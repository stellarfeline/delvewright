//! Per-piece shot planner for `delve-render piece`.
//!
//! Two camera kinds, answering two different questions.
//!
//! **Orbit** cameras fit themselves to the model and look inward. They answer
//! *"is the set well made"*: massing, silhouette, floor plan, where a socket
//! sits. Nucleation fits them to the model bounds and optionally aims them at a
//! `target`.
//!
//! - **4 exterior corner-isometric** shots (yaw 45/135/225/315, pitch 30).
//! - **1 top-down** floor plan (pitch 90) on a ceiling-stripped cutaway.
//! - **1 doorway** shot per socket, ceiling-stripped, aimed just inside the
//!   opening from the socket's outward side.
//! - **1 surroundings** shot per declared anchor, ceiling-stripped, aimed down at
//!   it — where the anchor sits in the piece.
//!
//! **Eye** cameras stand inside the piece, at a body's eye height, looking the
//! way the anchor faces ([`plan_piece`] → [`Framing::Eye`]). They answer the
//! only question the gates cannot: *is this the scene that was asked for* —
//! doorway shape and proportion, what is in front of the body, how the walls
//! read, whether an anchor is looking at the thing it is about. An orbit camera
//! cannot answer it, because from outside a roofed piece every anchor shot is
//! the same picture of the same rock.
//!
//! The eye point is **resolved, not assumed** (`crate::occupancy`): a prefab is
//! mostly solid, so an anchor cell may hold a gate, a barrel or a wall. The
//! resolution is reported per shot and raises `DW0727`; it is never applied
//! silently.

use crate::diag::{DW_ANCHOR_EYE, Diagnostic};
use crate::meta::PrefabMeta;
use crate::nbt::Structure;
use crate::occupancy::{Clearance, Facing, Occupancy, Placement, Standing};

/// Field of view for the orbit shots (degrees) — a long lens that keeps a piece
/// readable without perspective stretch.
pub const ORBIT_FOV_DEG: f32 = 45.0;

/// Field of view for the eye shots (degrees): Minecraft's own default
/// first-person FOV, so the frame is proportioned like what a player sees rather
/// than like a photograph of a model.
pub const PLAYER_FOV_DEG: f32 = 70.0;

/// How a shot's camera is positioned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Framing {
    /// Nucleation's orbit/turntable fit: the camera backs off far enough to
    /// frame the model and looks at `target` (or the model centroid).
    Orbit {
        /// >1 pushes the camera closer.
        zoom: f32,
        target: Option<[f32; 3]>,
    },
    /// A free camera standing at `pos`, looking along the shot's yaw/pitch.
    Eye { pos: [f32; 3] },
}

/// The resolved body an eye shot is taken from — everything a reviewer needs to
/// know *where they are standing* in the frame they are looking at.
#[derive(Debug, Clone, PartialEq)]
pub struct EyeCamera {
    /// The anchor this shot serves (full name, e.g. `anchor/gate`).
    pub anchor: String,
    /// The anchor's declared cell.
    pub anchor_cell: [i32; 3],
    /// The anchor's declared facing — the direction the camera looks.
    pub facing: Facing,
    /// The cell the body's feet occupy.
    pub cell: [i32; 3],
    /// The camera point (cell centre at eye height).
    pub pos: [f32; 3],
    /// How `cell` was chosen from `anchor_cell`.
    pub placement: Placement,
    /// Whether that cell has a solid floor under it.
    pub supported: bool,
    /// What the eye meets looking straight ahead — a measurement, no verdict.
    pub clearance: Clearance,
}

/// One planned per-piece shot.
#[derive(Debug, Clone, PartialEq)]
pub struct PieceShot {
    /// Output file stem (e.g. `ext-ne`, `top`, `door-0`, `around-gate`,
    /// `eye-gate`).
    pub name: String,
    /// Machine tag for the shot manifest: `exterior`, `plan`, `doorway`,
    /// `surroundings`, `eye`.
    pub kind: &'static str,
    /// Orbit yaw / pitch (degrees), Nucleation `CameraConfig` convention:
    /// `dir = -[cos(pitch)·sin(yaw), sin(pitch), cos(pitch)·cos(yaw)]`.
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    /// Field of view (degrees).
    pub fov_deg: f32,
    /// How the camera is placed.
    pub framing: Framing,
    /// Strip the top Y layer before meshing (dollhouse cutaway) so an orbit
    /// camera can see a roofed interior. Never set on an eye shot: the camera is
    /// already inside, and a body sees its own ceiling.
    pub cutaway: bool,
    /// Present exactly when `framing` is [`Framing::Eye`].
    pub eye: Option<EyeCamera>,
}

/// How many anchors the eye-shot pass actually bound to. A validation artifact
/// states its binding count; a zero binding is a finding, not a pass (CLAUDE.md).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnchorBinding {
    /// Anchors in the prefab metadata.
    pub declared: usize,
    /// Anchors carrying both a `pos` and a cardinal `facing` — the ones an eye
    /// shot is even expressible for.
    pub eligible: usize,
    /// Eye shots planned.
    pub eye_shots: usize,
    /// Eligible anchors with no body cell within reach, by name.
    pub unplaceable: Vec<String>,
}

/// The planned shot set plus what the planner found while making it.
#[derive(Debug, Clone, PartialEq)]
pub struct PiecePlan {
    pub shots: Vec<PieceShot>,
    pub diagnostics: Vec<Diagnostic>,
    pub binding: AnchorBinding,
}

fn centre_of(pos: [i32; 3]) -> [f32; 3] {
    [
        pos[0] as f32 + 0.5,
        pos[1] as f32 + 0.5,
        pos[2] as f32 + 0.5,
    ]
}

/// `anchor/branch-door` → `branch_door`.
fn anchor_stem(name: &str) -> String {
    name.rsplit('/')
        .next()
        .unwrap_or(name)
        .replace(['-', '.'], "_")
}

fn orbit(
    name: String,
    kind: &'static str,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    target: Option<[f32; 3]>,
    cutaway: bool,
) -> PieceShot {
    PieceShot {
        name,
        kind,
        yaw_deg: yaw,
        pitch_deg: pitch,
        fov_deg: ORBIT_FOV_DEG,
        framing: Framing::Orbit { zoom, target },
        cutaway,
        eye: None,
    }
}

/// Plan the deterministic shot set for `st`, using `meta` when present (sockets
/// → doorway shots, anchors → surroundings + eye shots).
pub fn plan_piece(st: &Structure, meta: Option<&PrefabMeta>) -> PiecePlan {
    let mut shots = Vec::new();
    let mut diagnostics = Vec::new();
    let mut binding = AnchorBinding::default();

    // 4 exterior corner-isometric.
    for (name, yaw) in [
        ("ext-ne", 45.0),
        ("ext-se", 135.0),
        ("ext-sw", 225.0),
        ("ext-nw", 315.0),
    ] {
        shots.push(orbit(
            name.to_string(),
            "exterior",
            yaw,
            30.0,
            1.0,
            None,
            false,
        ));
    }

    // Top-down floor plan (ceiling stripped).
    shots.push(orbit("top".to_string(), "plan", 0.0, 90.0, 1.0, None, true));

    let Some(meta) = meta else {
        return PiecePlan {
            shots,
            diagnostics,
            binding,
        };
    };

    // Interior doorway shots, one per socket (connector order).
    for (i, c) in meta.connectors.iter().enumerate() {
        let Some(facing) = Facing::parse(&c.facing) else {
            continue;
        };
        // Aim a couple of blocks inside the socket (opposite its outward facing).
        let inward = facing.unit();
        let base = centre_of(c.local_pos);
        let target = [
            base[0] - (inward[0] * 2) as f32,
            base[1] + 1.0,
            base[2] - (inward[2] * 2) as f32,
        ];
        shots.push(orbit(
            format!("door-{i}"),
            "doorway",
            facing.behind_yaw_deg(),
            20.0,
            1.6,
            Some(target),
            true,
        ));
    }

    // Per-anchor shots (sorted by name for determinism): the orbit that shows
    // where the anchor sits, then the eye that shows what it looks at.
    let occ = Occupancy::new(st);
    for (name, a) in &meta.anchors {
        binding.declared += 1;
        let stem = anchor_stem(name);

        // Surroundings orbit. Gate anchors use the region centre; point anchors
        // their position.
        let around = if let Some(p) = a.pos {
            Some(centre_of(p))
        } else {
            a.region.as_ref().map(|r| {
                [
                    (r.from[0] + r.to[0]) as f32 / 2.0 + 0.5,
                    (r.from[1] + r.to[1]) as f32 / 2.0 + 1.0,
                    (r.from[2] + r.to[2]) as f32 / 2.0 + 0.5,
                ]
            })
        };
        if let Some(target) = around {
            shots.push(orbit(
                format!("anchor-{stem}"),
                "surroundings",
                45.0,
                55.0,
                1.4,
                Some(target),
                true,
            ));
        }

        // Eye shot: needs a point and a cardinal facing.
        let (Some(cell), Some(facing)) = (a.pos, a.facing.as_deref().and_then(Facing::parse))
        else {
            continue;
        };
        binding.eligible += 1;
        let Some(standing) = occ.resolve_standing(cell, facing) else {
            binding.unplaceable.push(name.clone());
            diagnostics.push(Diagnostic::warning(
                DW_ANCHOR_EYE,
                format!(
                    "anchor `{name}` at {cell:?} facing {}: no cell within {} block(s) fits a \
                     standing body with the anchor still in front of it, so NO eye-level shot was \
                     taken for it. The remaining shots of this anchor are orbit cameras outside \
                     the piece; nothing in this render set shows what a body there would see",
                    facing.as_str(),
                    crate::occupancy::SEARCH_RADIUS
                ),
            ));
            continue;
        };
        if let Some(d) = placement_diagnostic(name, cell, facing, &standing, occ.state(cell)) {
            diagnostics.push(d);
        }
        binding.eye_shots += 1;
        shots.push(PieceShot {
            name: format!("eye-{stem}"),
            kind: "eye",
            yaw_deg: facing.view_yaw_deg(),
            pitch_deg: 0.0,
            fov_deg: PLAYER_FOV_DEG,
            framing: Framing::Eye {
                pos: standing.eye(),
            },
            cutaway: false,
            eye: Some(EyeCamera {
                anchor: name.clone(),
                anchor_cell: cell,
                facing,
                cell: standing.cell,
                pos: standing.eye(),
                placement: standing.placement,
                supported: standing.supported,
                clearance: occ.forward_clearance(standing.cell, facing),
            }),
        });
    }

    if binding.eligible > 0 && binding.eye_shots == 0 {
        diagnostics.push(Diagnostic::warning(
            DW_ANCHOR_EYE,
            format!(
                "{} anchor(s) could carry an eye-level shot and NONE of them could be placed — \
                 this render set contains no interior view at all, and cannot show whether the \
                 piece is the scene it was authored from",
                binding.eligible
            ),
        ));
    }

    PiecePlan {
        shots,
        diagnostics,
        binding,
    }
}

/// The `DW0727` a non-trivial placement owes the reviewer: the image is not
/// taken where the metadata says the anchor is, and the frame cannot say so.
fn placement_diagnostic(
    name: &str,
    cell: [i32; 3],
    facing: Facing,
    s: &Standing,
    occupied_by: Option<&str>,
) -> Option<Diagnostic> {
    let held = occupied_by.unwrap_or("nothing (the cell above has no head room)");
    let detail = match s.placement {
        Placement::AnchorCell => {
            return (!s.supported).then(|| {
                Diagnostic::warning(
                    DW_ANCHOR_EYE,
                    format!(
                        "anchor `{name}` at {cell:?} has no solid floor under it — the eye shot is \
                         taken as if a body hovered there"
                    ),
                )
            });
        }
        Placement::SteppedBack { blocks } => format!(
            "its own cell holds `{held}`, so the camera stands {blocks} block(s) back along {} at \
             {:?}",
            facing.as_str(),
            s.cell
        ),
        Placement::Nearest { offset } => format!(
            "its own cell holds `{held}` and nothing directly behind it is open, so the camera \
             stands at {:?} (offset {offset:?} from the anchor)",
            s.cell
        ),
    };
    Some(Diagnostic::warning(
        DW_ANCHOR_EYE,
        format!(
            "anchor `{name}` at {cell:?} facing {}: {detail}. The eye shot is a body's view from \
             that cell, not from the anchor cell",
            facing.as_str()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 7×5×9 shell: solid floor/ceiling/walls, open interior at y=1..3.
    fn shell(size: [i32; 3]) -> Structure {
        let mut blocks = Vec::new();
        for x in 0..size[0] {
            for y in 0..size[1] {
                for z in 0..size[2] {
                    let solid = x == 0
                        || x == size[0] - 1
                        || z == 0
                        || z == size[2] - 1
                        || y == 0
                        || y == size[1] - 1;
                    blocks.push(([x, y, z], usize::from(solid)));
                }
            }
        }
        Structure {
            size,
            palette: vec!["minecraft:air".to_string(), "minecraft:stone".to_string()],
            blocks,
        }
    }

    fn names(plan: &PiecePlan) -> Vec<&str> {
        plan.shots.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn exterior_and_topdown_without_metadata() {
        let plan = plan_piece(&shell([7, 5, 9]), None);
        assert_eq!(plan.shots.len(), 5);
        assert_eq!(
            names(&plan),
            ["ext-ne", "ext-se", "ext-sw", "ext-nw", "top"]
        );
        assert!(plan.shots.iter().find(|s| s.name == "top").unwrap().cutaway);
        assert!(
            !plan
                .shots
                .iter()
                .find(|s| s.name == "ext-ne")
                .unwrap()
                .cutaway
        );
        assert!(plan.diagnostics.is_empty());
    }

    #[test]
    fn sockets_and_anchors_add_interior_shots() {
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{
                "anchors": {
                    "anchor/gate": { "region": { "from": [2,1,4], "to": [4,3,4] } },
                    "anchor/keeper": { "pos": [3,1,2], "facing": "north" }
                },
                "connectors": [
                    { "name": "s", "target": "s", "local_pos": [3,1,0], "facing": "north",
                      "opening": [3,3], "joint": "aligned" },
                    { "name": "s", "target": "s", "local_pos": [3,1,8], "facing": "south",
                      "opening": [3,3], "joint": "aligned" }
                ]
            }"#,
        )
        .unwrap();
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta));
        let n = names(&plan);
        assert!(n.contains(&"door-0"));
        assert!(n.contains(&"door-1"));
        assert!(n.contains(&"anchor-gate"));
        assert!(n.contains(&"anchor-keeper"));
        assert!(n.contains(&"eye-keeper"));
        // A region anchor has no facing, so no eye shot is even expressible.
        assert!(!n.contains(&"eye-gate"));
        assert_eq!(plan.binding.declared, 2);
        assert_eq!(plan.binding.eligible, 1);
        assert_eq!(plan.binding.eye_shots, 1);
        // Every orbit interior shot keeps its cutaway + explicit target.
        for s in plan
            .shots
            .iter()
            .filter(|s| s.kind == "doorway" || s.kind == "surroundings")
        {
            assert!(s.cutaway, "{} lost its cutaway", s.name);
            assert!(
                matches!(
                    s.framing,
                    Framing::Orbit {
                        target: Some(_),
                        ..
                    }
                ),
                "{} lost its target",
                s.name
            );
        }
        // north socket → orbit yaw 180 (camera on the outward/−Z side, looking in).
        assert_eq!(
            plan.shots
                .iter()
                .find(|s| s.name == "door-0")
                .unwrap()
                .yaw_deg,
            180.0
        );
    }

    /// The obligation this whole module exists for: an anchor's declared
    /// `facing` decides where its eye camera LOOKS. Two anchors on one cell
    /// with opposite facings must differ in the only thing that can make their
    /// images differ — the view direction.
    #[test]
    fn opposite_facings_on_one_cell_aim_opposite_ways() {
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{"anchors": {
                    "anchor/a": { "pos": [3,1,4], "facing": "north" },
                    "anchor/b": { "pos": [3,1,4], "facing": "south" }
                }}"#,
        )
        .unwrap();
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta));
        let a = plan.shots.iter().find(|s| s.name == "eye-a").unwrap();
        let b = plan.shots.iter().find(|s| s.name == "eye-b").unwrap();
        // Same body, same eye point — so nothing but `facing` can distinguish them.
        assert_eq!(a.framing, b.framing);
        assert_eq!(a.pitch_deg, b.pitch_deg);
        // …and they look opposite ways. A planner that ignored `facing` would
        // give both the same yaw and fail here.
        assert_eq!(a.yaw_deg, 0.0, "north looks toward −Z");
        assert_eq!(b.yaw_deg, 180.0, "south looks toward +Z");
        assert_eq!((a.yaw_deg - b.yaw_deg).abs(), 180.0);
        // The facing also rides the manifest, so the image can be read back.
        assert_eq!(a.eye.as_ref().unwrap().facing, Facing::North);
        assert_eq!(b.eye.as_ref().unwrap().facing, Facing::South);
    }

    #[test]
    fn every_cardinal_facing_produces_its_own_direction() {
        // All four at one cell: four distinct yaws, one per facing.
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{"anchors": {
                    "anchor/n": { "pos": [3,1,4], "facing": "north" },
                    "anchor/s": { "pos": [3,1,4], "facing": "south" },
                    "anchor/e": { "pos": [3,1,4], "facing": "east"  },
                    "anchor/w": { "pos": [3,1,4], "facing": "west"  }
                }}"#,
        )
        .unwrap();
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta));
        let mut yaws: Vec<f32> = plan
            .shots
            .iter()
            .filter(|s| s.kind == "eye")
            .map(|s| s.yaw_deg)
            .collect();
        assert_eq!(yaws.len(), 4);
        yaws.sort_by(f32::total_cmp);
        yaws.dedup();
        assert_eq!(yaws.len(), 4, "four facings must give four view directions");
    }

    #[test]
    fn the_eye_stands_at_body_height_inside_the_piece() {
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{"anchors": {"anchor/k": { "pos": [3,1,4], "facing": "north" }}}"#,
        )
        .unwrap();
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta));
        let s = plan.shots.iter().find(|s| s.name == "eye-k").unwrap();
        assert_eq!(
            s.framing,
            Framing::Eye {
                pos: [3.5, 1.0 + crate::occupancy::EYE_HEIGHT, 4.5]
            }
        );
        assert!(!s.cutaway, "an eye shot never strips the ceiling");
        assert_eq!(s.fov_deg, PLAYER_FOV_DEG);
        let eye = s.eye.as_ref().unwrap();
        assert_eq!(eye.placement, Placement::AnchorCell);
        assert!(eye.supported);
    }

    /// An anchor whose own cell is solid: the camera steps back, and the shot
    /// SAYS it did — `DW0727`, plus the offset in the manifest.
    #[test]
    fn a_solid_anchor_cell_is_reported_as_dw0727() {
        let mut st = shell([7, 5, 9]);
        // Fill the anchor's own column with stone (palette index 1).
        for b in st.blocks.iter_mut() {
            if b.0 == [3, 1, 4] || b.0 == [3, 2, 4] {
                b.1 = 1;
            }
        }
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{"anchors": {"anchor/bars": { "pos": [3,1,4], "facing": "west" }}}"#,
        )
        .unwrap();
        let plan = plan_piece(&st, Some(&meta));
        assert_eq!(plan.binding.eye_shots, 1);
        let d = plan
            .diagnostics
            .iter()
            .find(|d| d.code == "DW0727")
            .expect("a stepped-back eye camera must be reported");
        assert!(d.message.contains("anchor/bars"), "{}", d.message);
        let eye = plan
            .shots
            .iter()
            .find(|s| s.name == "eye-bars")
            .unwrap()
            .eye
            .as_ref()
            .unwrap();
        assert_eq!(eye.anchor_cell, [3, 1, 4]);
        assert_eq!(eye.cell, [4, 1, 4], "one block back along west");
        assert_eq!(eye.placement, Placement::SteppedBack { blocks: 1 });
    }

    /// A piece with declared, facing-carrying anchors and no interior at all:
    /// zero eye shots is a finding, stated under the same code.
    #[test]
    fn a_piece_with_no_body_room_reports_a_zero_binding() {
        let mut st = shell([7, 5, 9]);
        for b in st.blocks.iter_mut() {
            b.1 = 1;
        }
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{"anchors": {"anchor/k": { "pos": [3,1,4], "facing": "north" }}}"#,
        )
        .unwrap();
        let plan = plan_piece(&st, Some(&meta));
        assert_eq!(plan.binding.eligible, 1);
        assert_eq!(plan.binding.eye_shots, 0);
        assert_eq!(plan.binding.unplaceable, vec!["anchor/k".to_string()]);
        let codes: Vec<&str> = plan.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, [DW_ANCHOR_EYE, DW_ANCHOR_EYE]);
    }

    #[test]
    fn plan_is_deterministic() {
        let st = shell([9, 5, 9]);
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{"anchors": {
                    "anchor/a": { "pos": [3,1,4], "facing": "north" },
                    "anchor/b": { "pos": [5,1,4], "facing": "east" },
                    "anchor/c": { "pos": [4,1,6], "facing": "west" }
                }}"#,
        )
        .unwrap();
        let a = plan_piece(&st, Some(&meta));
        let b = plan_piece(&st, Some(&meta));
        assert_eq!(a, b);
    }
}
