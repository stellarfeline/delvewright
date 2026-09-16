//! Per-piece shot planner for `delvec render piece`.
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
//! The eye point is **resolved, not assumed** (`crate::render::occupancy`): a prefab is
//! mostly solid, so an anchor cell may hold a gate, a barrel or a wall. The
//! resolution is reported per shot and raises `DW0727`; it is never applied
//! silently.
//!
//! **Room** cameras (`room-<anchor>`) are eye cameras too, with the same facing
//! and the same field of view, standing **back along the facing** at the far side
//! of the space the anchor stands in ([`crate::compiler::view::sight::stand_back`]).
//! The eye shot answers *what does a body on the anchor see*; the room shot
//! answers *what is the room the anchor is in*, which the eye shot answers only
//! when the anchor happens to face into it. Both frames are measured for
//! blindness (`DW0893`) off the piece's bytes, and the manifest says how far the
//! room camera stood back and why it stopped.
//!
//! **Views** ([`crate::render::view`]) are the same shots, stated by the author instead
//! of derived: a bearing and a subject box arrive as input, and the planner
//! appends them to this plan. They are not a third camera kind and not a second
//! planner — the fixed set simply contains no square-on elevation, and an author
//! who needs one says so.

use crate::compiler::view::showing::Cells;
use crate::compiler::view::sight::{self, Sight, Stand};
use crate::render::detect::Featureless;
use crate::render::diag::{DW_ANCHOR_EYE, DW_INPUT, Diagnostic};
use crate::render::meta::PrefabMeta;
use crate::render::nbt::Structure;
use crate::render::occupancy::{Clearance, Facing, Occupancy, Placement, Standing};
use crate::render::view::View;

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

/// The camera a `room-<anchor>` shot stands at.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomCamera {
    /// The anchor this shot serves (full name).
    pub anchor: String,
    /// The anchor's declared cell.
    pub anchor_cell: [i32; 3],
    /// The anchor's declared facing — the direction the camera looks, unchanged.
    pub facing: Facing,
    /// Where the body stands, how far back, and why it went no further.
    pub stand: Stand,
}

/// One planned per-piece shot.
#[derive(Debug, Clone, PartialEq)]
pub struct PieceShot {
    /// Output file stem (e.g. `ext-ne`, `top`, `door-0`, `around-gate`,
    /// `eye-gate`, `room-gate`).
    pub name: String,
    /// Machine tag for the shot manifest: `exterior`, `plan`, `doorway`,
    /// `surroundings`, `eye`, `room`, `view`.
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
    /// Present exactly when this is an `eye` shot.
    pub eye: Option<EyeCamera>,
    /// Present exactly when this is a `room` shot.
    pub room: Option<RoomCamera>,
    /// What an eye-level frame's sampled rays met (`DW0893`) — present exactly
    /// on the `eye` and `room` shots.
    pub sight: Option<Sight>,
    /// Present exactly when the author declared this shot ([`crate::render::view`]) —
    /// what they asked for, so the frame can be re-asked for verbatim.
    pub view: Option<View>,
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

/// What the blindness measurement (`DW0893`) bound to, stated on every run with
/// its zeroes: a sight check that does not say how many frames it measured
/// cannot be told from one that measured none.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SightBinding {
    /// `eye` frames measured.
    pub eye_frames: usize,
    /// `room` frames measured.
    pub room_frames: usize,
    /// Standing `view` frames measured.
    pub view_frames: usize,
    /// Blind `eye` frames, by shot name.
    pub blind_eye: Vec<String>,
    /// Blind `room` frames, by shot name.
    pub blind_room: Vec<String>,
    /// Blind standing `view` frames, by shot name.
    pub blind_view: Vec<String>,
    /// Eye-eligible anchors with no cell within one course to stand a room
    /// camera on, by name.
    pub no_stand: Vec<String>,
    /// The furthest any room camera stood back, in blocks.
    pub max_back: u32,
}

/// How many author-declared views the run actually planned. Stated on every run
/// for the same reason [`AnchorBinding`] is: a shot set that does not say what it
/// bound to cannot be told from one that bound to nothing (CLAUDE.md).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewBinding {
    /// Views the author declared.
    pub declared: usize,
    /// Views that became shots. Equal to `declared` on any successful run — a
    /// view that cannot be resolved stops the run rather than being dropped.
    pub planned: usize,
}

/// The planned shot set plus what the planner found while making it.
#[derive(Debug, Clone, PartialEq)]
pub struct PiecePlan {
    pub shots: Vec<PieceShot>,
    pub diagnostics: Vec<Diagnostic>,
    pub binding: AnchorBinding,
    pub views: ViewBinding,
    pub sight: SightBinding,
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
        room: None,
        sight: None,
        view: None,
    }
}

/// Plan the deterministic shot set for `st`, using `meta` when present (sockets
/// → doorway shots, anchors → surroundings + eye shots), then append the
/// author's declared `views` in the order they were given.
///
/// A view that cannot be resolved — an unknown subject, a name that would
/// overwrite another shot's image — is an **error**, and it stops the run before
/// a single frame is rendered. The alternative is worse than nothing: a shot set
/// that quietly dropped the one camera the reviewer asked for still looks like a
/// full set in a directory listing.
pub fn plan_piece(
    st: &Structure,
    meta: Option<&PrefabMeta>,
    views: &[View],
) -> Result<PiecePlan, Diagnostic> {
    let mut plan = plan_fixed_set(st, meta);
    plan.views.declared = views.len();
    for v in views {
        if let Some(clash) = plan.shots.iter().find(|s| s.name == v.name) {
            return Err(Diagnostic::error(
                DW_INPUT,
                format!(
                    "view `{}` is already the name of a {} shot in this set — rendering it would \
                     overwrite that image. Give the view its own `name=`",
                    v.name, clash.kind
                ),
            ));
        }
        if let Some(standing) = &v.stand {
            let shot = plan_standing_view(st, meta, v, standing, &mut plan)?;
            plan.shots.push(shot);
            plan.views.planned += 1;
            continue;
        }
        let (fmin, fmax) = v
            .framed_box(st, meta)
            .map_err(|e| Diagnostic::error(DW_INPUT, e))?;
        let target = crate::render::view::midpoint(fmin, fmax);
        plan.shots.push(PieceShot {
            name: v.name.clone(),
            kind: "view",
            yaw_deg: v.yaw_deg,
            pitch_deg: v.pitch_deg,
            fov_deg: v.fov_deg,
            // Aimed at the framed box's own centre, which is also what gives a
            // declared view the tight projected-corner fit rather than the
            // sphere fit the target-less exterior shots use.
            framing: Framing::Orbit {
                zoom: solve_zoom(st, &fmin, &fmax, &target, v),
                target: Some(target),
            },
            cutaway: v.cutaway,
            eye: None,
            room: None,
            sight: None,
            view: Some(v.clone()),
        });
        plan.views.planned += 1;
    }
    Ok(plan)
}

/// A `stand=` view: a body in the anchor's space, backed off along `look` (or
/// the anchor's facing) to the far side of it, measured like every eye-level
/// frame. Anything that cannot be resolved stops the run, as for any view.
fn plan_standing_view(
    st: &Structure,
    meta: Option<&PrefabMeta>,
    v: &View,
    standing: &crate::render::view::Standing,
    plan: &mut PiecePlan,
) -> Result<PieceShot, Diagnostic> {
    let name = &standing.anchor;
    let refuse = |why: String| {
        Diagnostic::error(
            DW_INPUT,
            format!("view `{}` stands at `{name}`, {why}", v.spec),
        )
    };
    let meta = meta.ok_or_else(|| {
        refuse("but this piece has no metadata file, so it declares no anchors".to_string())
    })?;
    let a = meta.anchors.get(name).ok_or_else(|| {
        let declared: Vec<&str> = meta.anchors.keys().map(String::as_str).collect();
        refuse(format!(
            "which this piece does not declare. Declared anchors: {}",
            if declared.is_empty() {
                "none".to_string()
            } else {
                declared.join(", ")
            }
        ))
    })?;
    let cell = a
        .pos
        .ok_or_else(|| refuse("which declares no position for a body to stand at".to_string()))?;
    let facing = match standing.look {
        Some(l) => l,
        None => a.facing.as_deref().and_then(Facing::parse).ok_or_else(|| {
            refuse("which declares no cardinal facing, and the view states no `look=`".to_string())
        })?,
    };
    let grid = sight::grid_of(st);
    let cells = Cells::of(st);
    let step = facing.unit();
    let stand = sight::stand_back(&cells, &grid, cell, step).ok_or_else(|| {
        refuse(format!(
            "and no cell within one course of {cell:?} holds a standing body — there is \
             nowhere to stand"
        ))
    })?;
    let mut cam = sight::level_camera(stand.eye(), step);
    cam.pitch = f64::from(v.pitch_deg);
    cam.fov = f64::from(v.fov_deg);
    let seen = Sight::measure(&grid, &cam);
    plan.sight.view_frames += 1;
    if seen.is_blind() {
        plan.sight.blind_view.push(v.name.clone());
        plan.diagnostics.push(sight::blind_diagnostic(
            &v.name,
            name,
            facing.as_str(),
            stand.cell,
            &seen,
            Some((v.name.as_str(), &stand, &seen)),
        ));
    }
    let eye = stand.eye();
    Ok(PieceShot {
        name: v.name.clone(),
        kind: "view",
        yaw_deg: facing.view_yaw_deg(),
        pitch_deg: v.pitch_deg,
        fov_deg: v.fov_deg,
        framing: Framing::Eye {
            pos: [eye[0] as f32, eye[1] as f32, eye[2] as f32],
        },
        cutaway: false,
        eye: None,
        room: Some(RoomCamera {
            anchor: name.clone(),
            anchor_cell: cell,
            facing,
            stand,
        }),
        sight: Some(seen),
        view: Some(v.clone()),
    })
}

/// The Nucleation `zoom` that stands a view's camera at the distance which fits
/// the box it *framed*, rather than the distance which fits the whole model.
///
/// Nucleation's orbit camera has no distance field — it fits itself to the model
/// and divides by `zoom` — so the wanted standoff is expressed as a ratio of two
/// fits taken with the same replicated formula ([`crate::render::gpu::fit_distance`]):
/// the model's, which is what the renderer would do unasked, over the framed
/// box's, which is what the author asked for. The ratio cancels the constants,
/// so this stays correct if the fit formula is ever corrected.
///
/// Degenerate cases fall back to the author's `zoom` verbatim, which is the plain
/// orbit behaviour: a straight-down/straight-up bearing has no stable right/up
/// basis (`fit_distance`'s `cross(forward, +Y)` vanishes), and a subject box with
/// no extent at all across the view has no size to fit.
fn solve_zoom(
    st: &Structure,
    fmin: &[f32; 3],
    fmax: &[f32; 3],
    target: &[f32; 3],
    v: &View,
) -> f32 {
    // `fit_distance` builds its screen basis as `cross(forward, +Y)`, which
    // vanishes when the camera looks straight up or down. There is no framing
    // arithmetic to do there, and a ratio taken from the degenerate basis would
    // be a large number pointing the camera at its own target.
    let forward = crate::render::gpu::view_direction(v.yaw_deg, v.pitch_deg);
    if forward[0] * forward[0] + forward[2] * forward[2] < 1e-6 {
        return v.zoom;
    }
    let model_max = [st.size[0] as f32, st.size[1] as f32, st.size[2] as f32];
    let fit = |lo: [f32; 3], hi: [f32; 3]| {
        crate::render::gpu::fit_distance(lo, hi, 1.0, *target, v.yaw_deg, v.pitch_deg, v.fov_deg)
    };
    let framed = fit(*fmin, *fmax);
    let whole = fit([0.0, 0.0, 0.0], model_max);
    if !framed.is_finite() || !whole.is_finite() || framed <= 0.0 || whole <= 0.0 {
        return v.zoom;
    }
    whole / framed * v.zoom
}

/// The fixed set — every shot the renderer derives without being asked.
fn plan_fixed_set(st: &Structure, meta: Option<&PrefabMeta>) -> PiecePlan {
    let mut shots = Vec::new();
    let mut diagnostics = Vec::new();
    let mut binding = AnchorBinding::default();
    let mut sights = SightBinding::default();

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
            views: ViewBinding::default(),
            sight: sights,
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
    let grid = sight::grid_of(st);
    let cells = Cells::of(st);
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
                    crate::render::occupancy::SEARCH_RADIUS
                ),
            ));
            continue;
        };
        if let Some(d) = placement_diagnostic(name, cell, facing, &standing, occ.state(cell)) {
            diagnostics.push(d);
        }
        binding.eye_shots += 1;
        let step = facing.unit();
        let eye_name = format!("eye-{stem}");
        let eye_sight = Sight::measure(
            &grid,
            &sight::level_camera(sight::eye_of(standing.cell), step),
        );
        // The room camera: the same facing, stood back to the far side of the
        // space the anchor is in. Measured before either frame is reported, so
        // a blind eye frame can name the picture to open instead.
        let room = sight::stand_back(&cells, &grid, cell, step).map(|stand| {
            let rs = Sight::measure(&grid, &sight::level_camera(stand.eye(), step));
            (format!("room-{stem}"), stand, rs)
        });
        sights.eye_frames += 1;
        if eye_sight.is_blind() {
            sights.blind_eye.push(eye_name.clone());
            diagnostics.push(sight::blind_diagnostic(
                &eye_name,
                name,
                facing.as_str(),
                standing.cell,
                &eye_sight,
                room.as_ref().map(|(n, st, rs)| (n.as_str(), st, rs)),
            ));
        }
        shots.push(PieceShot {
            name: eye_name,
            kind: "eye",
            yaw_deg: facing.view_yaw_deg(),
            pitch_deg: 0.0,
            fov_deg: PLAYER_FOV_DEG,
            framing: Framing::Eye {
                pos: standing.eye(),
            },
            cutaway: false,
            view: None,
            room: None,
            sight: Some(eye_sight),
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
        let Some((room_name, stand, room_sight)) = room else {
            sights.no_stand.push(name.clone());
            continue;
        };
        sights.room_frames += 1;
        sights.max_back = sights.max_back.max(stand.back);
        if room_sight.is_blind() {
            sights.blind_room.push(room_name.clone());
            diagnostics.push(sight::blind_diagnostic(
                &room_name,
                name,
                facing.as_str(),
                stand.cell,
                &room_sight,
                Some((room_name.as_str(), &stand, &room_sight)),
            ));
        }
        let eye = sight::eye_of(stand.cell);
        shots.push(PieceShot {
            name: room_name,
            kind: "room",
            yaw_deg: facing.view_yaw_deg(),
            pitch_deg: 0.0,
            fov_deg: PLAYER_FOV_DEG,
            framing: Framing::Eye {
                pos: [eye[0] as f32, eye[1] as f32, eye[2] as f32],
            },
            cutaway: false,
            eye: None,
            room: Some(RoomCamera {
                anchor: name.clone(),
                anchor_cell: cell,
                facing,
                stand,
            }),
            sight: Some(room_sight),
            view: None,
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
        views: ViewBinding::default(),
        sight: sights,
    }
}

/// The `DW0727` a frame that shows nothing owes the reviewer.
///
/// The check is on the **pixels**, so it belongs to a rendered frame and not to
/// any one camera kind: an eye camera aimed out of the piece, a declared view
/// pushed inside a wall by its zoom, and a cutaway that stripped the only layer
/// there was all produce the same artifact — a rectangle of flat background that
/// reads, in a directory listing, as one more shot of the room. What differs is
/// only what the reviewer should do about it, which is what this message says.
pub fn empty_frame_diagnostic(stem: &str, shot: &PieceShot, f: &Featureless) -> Diagnostic {
    let (name, distinct) = (&shot.name, f.distinct);
    let message = if let Some(r) = &shot.room {
        format!(
            "{stem}/{name}: the room shot for `{}` is an EMPTY frame ({distinct} distinct \
             colour(s)) — a body standing {} block(s) back along {} at {:?} sees nothing but flat \
             background: the space behind the anchor opens onto nothing in the template",
            r.anchor,
            r.stand.back,
            r.facing.as_str(),
            r.stand.cell,
        )
    } else {
        match (&shot.eye, &shot.view) {
            (Some(e), _) => {
                // Two causes, two different things for the reader to do — the piece
                // is either aimed at nothing, or aimed out of itself, and only the
                // first is a defect.
                let cause = match &e.clearance {
                    crate::render::occupancy::Clearance::LeavesThePiece { open } => format!(
                        "The view runs {open} open cell(s) and then leaves the template. If this \
                     anchor is meant to face outward (an approach, a threshold), what it is about \
                     lives in the assembled world, and its real view is the campaign's own \
                     player-POV shot, not a per-piece render. Otherwise the piece is missing the \
                     thing the anchor names"
                    ),
                    crate::render::occupancy::Clearance::Blocked { open, state } => format!(
                        "The view runs {open} open cell(s) and then meets `{state}`, whose face fills \
                     the frame — the anchor is pressed against a surface"
                    ),
                };
                format!(
                    "{stem}/{name}: the eye shot for `{}` is an EMPTY frame ({distinct} distinct \
                 colour(s)) — a body standing at {:?} and looking {} sees nothing but flat \
                 background. {cause}. Whatever the cause, no image in this set shows what that \
                 anchor is about; the fix is the anchor or the geometry, never the camera",
                    e.anchor,
                    e.cell,
                    e.facing.as_str(),
                )
            }
            (None, Some(v)) => format!(
                "{stem}/{name}: the declared view `{}` is an EMPTY frame ({distinct} distinct \
             colour(s)) — a camera aimed at {} on bearing yaw {} pitch {} at zoom {} sees nothing \
             but flat background. A zoom past the fit distance puts the camera inside the model, \
             and a cutaway can strip the only layer there was. The picture this view was asked \
             for is NOT in this set; re-aim the view — never read the blank frame as the answer",
                v.spec,
                v.subject.tag(),
                v.yaw_deg,
                v.pitch_deg,
                v.zoom
            ),
            (None, None) => format!(
                "{stem}/{name}: the `{}` shot is an EMPTY frame ({distinct} distinct colour(s)). This \
             camera is fitted to the model, so an empty frame means there was nothing to fit — on \
             a cutaway shot, that the stripped layer was the only one the piece had",
                shot.kind
            ),
        }
    };
    Diagnostic::warning(DW_ANCHOR_EYE, message)
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
        let plan = plan_piece(&shell([7, 5, 9]), None, &[]).unwrap();
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
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta), &[]).unwrap();
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
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta), &[]).unwrap();
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
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta), &[]).unwrap();
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
        let plan = plan_piece(&shell([7, 5, 9]), Some(&meta), &[]).unwrap();
        let s = plan.shots.iter().find(|s| s.name == "eye-k").unwrap();
        assert_eq!(
            s.framing,
            Framing::Eye {
                pos: [3.5, 1.0 + crate::render::occupancy::EYE_HEIGHT, 4.5]
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
        let plan = plan_piece(&st, Some(&meta), &[]).unwrap();
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
        let plan = plan_piece(&st, Some(&meta), &[]).unwrap();
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
        let a = plan_piece(&st, Some(&meta), &[]).unwrap();
        let b = plan_piece(&st, Some(&meta), &[]).unwrap();
        assert_eq!(a, b);
    }

    // ---- room shots and blind frames (`DW0893`) ---------------------------

    fn one_anchor(facing: &str) -> PrefabMeta {
        serde_json::from_slice(
            format!(r#"{{"anchors": {{"anchor/k": {{ "pos": [3,1,1], "facing": "{facing}" }}}}}}"#)
                .as_bytes(),
        )
        .unwrap()
    }

    /// **The defect this surface exists for.** An anchor pressed against the
    /// wall it faces: its eye frame is a wall, which is reported as `DW0893`
    /// naming the room shot — the same facing, stood back to the far wall — and
    /// that frame is not blind.
    #[test]
    fn a_blind_eye_frame_is_reported_and_the_room_shot_stands_back_along_the_facing() {
        let st = shell([7, 5, 21]);
        let plan = plan_piece(&st, Some(&one_anchor("north")), &[]).unwrap();
        let eye = plan.shots.iter().find(|s| s.name == "eye-k").unwrap();
        let room = plan.shots.iter().find(|s| s.name == "room-k").unwrap();
        assert!(eye.sight.unwrap().is_blind(), "{:?}", eye.sight);
        assert_eq!(room.kind, "room");
        assert_eq!(room.yaw_deg, eye.yaw_deg, "the facing is never overridden");
        assert_eq!(room.pitch_deg, 0.0);
        assert_eq!(room.fov_deg, PLAYER_FOV_DEG);
        assert!(!room.cutaway);
        let r = room.room.as_ref().unwrap();
        assert_eq!(r.stand.start, [3, 1, 1]);
        assert_eq!(r.stand.cell, [3, 1, 19], "the far wall behind the anchor");
        assert_eq!(r.stand.back, 18);
        assert!(!room.sight.unwrap().is_blind(), "{:?}", room.sight);
        let d: Vec<&Diagnostic> = plan
            .diagnostics
            .iter()
            .filter(|d| d.code == "DW0893")
            .collect();
        assert_eq!(d.len(), 1, "{:?}", plan.diagnostics);
        assert!(!d[0].is_error(), "a report, never a refusal");
        assert!(d[0].message.contains("`room-k`"), "{}", d[0].message);
        assert_eq!(plan.sight.eye_frames, 1);
        assert_eq!(plan.sight.room_frames, 1);
        assert_eq!(plan.sight.blind_eye, ["eye-k"]);
        assert!(plan.sight.blind_room.is_empty());
        assert_eq!(plan.sight.max_back, 18);
    }

    /// The perturbation only the blindness check reads: the same body on the
    /// same cell, turned to face the room, is not blind and raises nothing.
    #[test]
    fn the_same_anchor_facing_into_the_room_is_not_blind() {
        let st = shell([7, 5, 21]);
        let plan = plan_piece(&st, Some(&one_anchor("south")), &[]).unwrap();
        let eye = plan.shots.iter().find(|s| s.name == "eye-k").unwrap();
        assert!(!eye.sight.unwrap().is_blind(), "{:?}", eye.sight);
        assert!(plan.diagnostics.iter().all(|d| d.code != "DW0893"));
        assert!(plan.sight.blind_eye.is_empty());
        assert_eq!(plan.sight.eye_frames, 1);
    }

    /// A standing view looks the way the author says, from the far side of the
    /// anchor's space, and a view that cannot be stood is refused before a frame.
    #[test]
    fn a_standing_view_looks_where_it_is_told_from_the_far_side() {
        let st = shell([7, 5, 21]);
        let meta = one_anchor("north");
        let v = View::parse("stand=anchor/k,look=south").unwrap();
        let plan = plan_piece(&st, Some(&meta), std::slice::from_ref(&v)).unwrap();
        let shot = plan.shots.last().unwrap();
        assert_eq!(shot.kind, "view");
        assert_eq!(shot.yaw_deg, Facing::South.view_yaw_deg());
        assert!(matches!(shot.framing, Framing::Eye { .. }));
        let r = shot.room.as_ref().unwrap();
        assert_eq!(r.facing, Facing::South);
        assert_eq!(r.stand.cell, [3, 1, 1], "backing north stops at the wall");
        assert_eq!(plan.sight.view_frames, 1);
        assert_eq!(plan.views.planned, 1);

        for (spec, needle) in [
            ("stand=anchor/nope", "does not declare"),
            ("name=x,stand=anchor/k,look=east", "nowhere to stand"),
        ] {
            let mut meta = meta.clone();
            if spec.contains("look=east") {
                // An anchor inside the wall has no body cell within one course.
                meta.anchors.get_mut("anchor/k").unwrap().pos = Some([0, 2, 5]);
            }
            let v = View::parse(spec).unwrap();
            let e = plan_piece(&st, Some(&meta), std::slice::from_ref(&v)).unwrap_err();
            assert_eq!(e.code, DW_INPUT);
            assert!(e.message.contains(needle), "{}", e.message);
        }
    }

    // ---- author-declared views -------------------------------------------

    fn view_meta() -> PrefabMeta {
        serde_json::from_slice(
            br#"{"anchors": {"anchor/keeper": { "pos": [3,1,2], "facing": "north" }}}"#,
        )
        .unwrap()
    }

    /// The obligation the whole surface exists for, and the one that decided a
    /// trial verdict: the fixed set contains **no** square-on elevation, and a
    /// declared `face=` view is one — level, on a cardinal bearing, aimed at the
    /// subject's own centre so the fit is the tight one.
    #[test]
    fn a_declared_face_view_is_the_square_on_elevation_the_fixed_set_lacks() {
        let st = shell([7, 5, 9]);
        let fixed = plan_piece(&st, Some(&view_meta()), &[]).unwrap();
        assert!(
            !fixed
                .shots
                .iter()
                .any(|s| s.pitch_deg == 0.0 && matches!(s.framing, Framing::Orbit { .. })),
            "the fixed set must contain no level orbit camera — that is the defect"
        );

        let v = View::parse("name=west-front,face=north").unwrap();
        let plan = plan_piece(&st, Some(&view_meta()), std::slice::from_ref(&v)).unwrap();
        let shot = plan.shots.last().unwrap();
        assert_eq!(shot.name, "west-front");
        assert_eq!(shot.kind, "view");
        assert_eq!(shot.pitch_deg, 0.0);
        assert_eq!(shot.yaw_deg, 180.0, "the −Z face is photographed from −Z");
        assert_eq!(shot.fov_deg, ORBIT_FOV_DEG);
        let Framing::Orbit { zoom, target } = shot.framing else {
            panic!("a view is an orbit camera");
        };
        assert_eq!(
            target,
            Some([3.5, 2.5, 0.0]),
            "a face view aims at the centre of THAT FACE, not of the box"
        );
        assert_eq!(zoom, 1.0, "no extra zoom is needed to fill a face head-on");
        assert!(shot.eye.is_none());
        assert_eq!(shot.view.as_ref().unwrap().name, "west-front");
        assert_eq!(
            plan.views,
            ViewBinding {
                declared: 1,
                planned: 1
            }
        );
    }

    /// Views are appended, never interleaved: every planned shot keeps its name,
    /// its parameters and its position in the set, so a review image taken
    /// before this surface existed is still the same image.
    #[test]
    fn declaring_views_leaves_the_fixed_set_byte_for_byte_alone() {
        let st = shell([7, 5, 9]);
        let meta = view_meta();
        let fixed = plan_piece(&st, Some(&meta), &[]).unwrap();
        let views = [
            View::parse("face=north").unwrap(),
            View::parse("name=plan-2,face=up,cutaway=true").unwrap(),
        ];
        let plan = plan_piece(&st, Some(&meta), &views).unwrap();
        assert_eq!(plan.shots.len(), fixed.shots.len() + 2);
        assert_eq!(&plan.shots[..fixed.shots.len()], &fixed.shots[..]);
        assert_eq!(plan.binding, fixed.binding);
        assert_eq!(plan.diagnostics, fixed.diagnostics);
        assert_eq!(
            plan.shots[fixed.shots.len()..]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["view-north", "plan-2"]
        );
    }

    /// A view named after a planned shot would write its PNG over that shot's —
    /// the one way this surface could regress the set it is added to. Refused
    /// before a frame is rendered.
    #[test]
    fn a_view_that_would_overwrite_a_planned_shot_is_refused() {
        let st = shell([7, 5, 9]);
        for name in ["top", "ext-ne", "eye-keeper", "anchor-keeper"] {
            let v = View::parse(&format!("name={name},face=north")).unwrap();
            let d = plan_piece(&st, Some(&view_meta()), std::slice::from_ref(&v)).unwrap_err();
            assert_eq!(d.code, DW_INPUT);
            assert!(d.message.contains("overwrite"), "{}", d.message);
        }
        // Two views under one name collide with each other for the same reason.
        let two = [
            View::parse("face=north").unwrap(),
            View::parse("face=north,fov=60").unwrap(),
        ];
        let d = plan_piece(&st, Some(&view_meta()), &two).unwrap_err();
        assert!(d.message.contains("overwrite"), "{}", d.message);
    }

    /// A bad aim is a refusal, not a picture of something else.
    #[test]
    fn a_view_aimed_at_a_subject_the_piece_does_not_have_is_refused() {
        let st = shell([7, 5, 9]);
        let v = View::parse("face=north,of=anchor/west-front").unwrap();
        let d = plan_piece(&st, Some(&view_meta()), std::slice::from_ref(&v)).unwrap_err();
        assert_eq!(d.code, DW_INPUT);
        assert!(d.message.contains("does not declare"), "{}", d.message);
        assert!(d.message.contains("anchor/keeper"), "{}", d.message);
    }

    #[test]
    fn a_view_aims_at_the_anchor_it_names() {
        let st = shell([7, 5, 9]);
        let v = View::parse("face=east,of=anchor/keeper").unwrap();
        let plan = plan_piece(&st, Some(&view_meta()), std::slice::from_ref(&v)).unwrap();
        let Framing::Orbit { target, .. } = plan.shots.last().unwrap().framing else {
            panic!("orbit");
        };
        // The anchor's own cell is [3,1,2]; its east face is the x=4 plane.
        assert_eq!(target, Some([4.0, 1.5, 2.5]));
    }

    /// What `of=` buys, and the reason the framed box is a box and not always
    /// the model: naming a smaller subject frames THAT subject. The whole-model
    /// face view needs no extra zoom (the face is the model's own silhouette
    /// head-on); an anchor's face view stands far closer.
    #[test]
    fn naming_a_smaller_subject_frames_that_subject() {
        let st = shell([31, 21, 41]);
        let meta = view_meta();
        let zoom_of = |spec: &str| {
            let v = View::parse(spec).unwrap();
            let plan = plan_piece(&st, Some(&meta), std::slice::from_ref(&v)).unwrap();
            match plan.shots.last().unwrap().framing {
                Framing::Orbit { zoom, .. } => zoom,
                Framing::Eye { .. } => panic!("orbit"),
            }
        };
        assert_eq!(zoom_of("face=north"), 1.0);
        assert!(
            zoom_of("face=north,of=anchor/keeper") > 5.0,
            "a one-cell subject must be a close-up, not the whole building again"
        );
    }

    /// The claim `zoom` rests on, stated as arithmetic rather than as a picture:
    /// Nucleation divides its own model fit by `zoom`, so the camera's distance
    /// from the target must come out equal to the fit of the box the view
    /// FRAMED — the face for a `face=` view — divided by the author's `zoom`.
    #[test]
    fn a_views_standoff_is_the_fit_of_the_box_it_framed() {
        let st = shell([7, 5, 21]);
        let meta = view_meta();
        for spec in [
            "face=north",
            "face=north,zoom=2",
            "face=south,fov=60",
            "face=west",
            "yaw=25,pitch=15",
        ] {
            let v = View::parse(spec).unwrap();
            let plan = plan_piece(&st, Some(&meta), std::slice::from_ref(&v)).unwrap();
            let shot = plan.shots.last().unwrap();
            let Framing::Orbit { zoom, target } = shot.framing else {
                panic!("orbit");
            };
            let target = target.unwrap();
            let fit = |lo, hi| {
                crate::render::gpu::fit_distance(
                    lo,
                    hi,
                    1.0,
                    target,
                    v.yaw_deg,
                    v.pitch_deg,
                    v.fov_deg,
                )
            };
            let (fmin, fmax) = v.framed_box(&st, Some(&meta)).unwrap();
            // Nucleation's own distance: its model fit, divided by our zoom.
            let distance = fit([0.0, 0.0, 0.0], [7.0, 5.0, 21.0]) / zoom;
            let wanted = fit(fmin, fmax) / v.zoom;
            assert!(
                (distance - wanted).abs() < 1e-3,
                "`{spec}`: camera at {distance} from {target:?}, wanted {wanted}"
            );
        }
    }

    /// A straight-down bearing has no stable screen basis, so the fit ratio is
    /// not computed there — the author's zoom is used verbatim, which is the
    /// plain orbit behaviour the `top` shot already has.
    #[test]
    fn a_vertical_bearing_falls_back_to_the_plain_orbit_fit() {
        let st = shell([7, 5, 9]);
        for spec in ["face=up", "face=down,zoom=1.5"] {
            let v = View::parse(spec).unwrap();
            let plan = plan_piece(&st, Some(&view_meta()), std::slice::from_ref(&v)).unwrap();
            let Framing::Orbit { zoom, .. } = plan.shots.last().unwrap().framing else {
                panic!("orbit");
            };
            assert_eq!(zoom, v.zoom, "`{spec}`");
        }
    }

    #[test]
    fn planning_with_views_is_deterministic() {
        let st = shell([7, 5, 9]);
        let views = [
            View::parse("face=north").unwrap(),
            View::parse("name=oblique,yaw=25,pitch=15,fov=60").unwrap(),
        ];
        let a = plan_piece(&st, Some(&view_meta()), &views).unwrap();
        let b = plan_piece(&st, Some(&view_meta()), &views).unwrap();
        assert_eq!(a, b);
    }

    /// An aimable camera is far easier to point at nothing than a derived one,
    /// so the empty-frame report must reach a declared view — and it must tell
    /// its reader something different from what it tells an anchor's reader.
    #[test]
    fn an_empty_frame_is_dw0727_for_every_camera_kind() {
        let st = shell([7, 5, 9]);
        let v = View::parse("name=west-front,face=north,zoom=90").unwrap();
        let plan = plan_piece(&st, Some(&view_meta()), std::slice::from_ref(&v)).unwrap();
        let f = crate::render::detect::Featureless { distinct: 1 };

        let view_shot = plan.shots.iter().find(|s| s.kind == "view").unwrap();
        let d = empty_frame_diagnostic("piece", view_shot, &f);
        assert_eq!(d.code, DW_ANCHOR_EYE);
        assert!(d.message.contains("EMPTY frame"), "{}", d.message);
        assert!(d.message.contains("west-front"), "{}", d.message);
        assert!(d.message.contains("zoom 90"), "{}", d.message);
        assert!(
            d.message.contains("is NOT in this set"),
            "a blank view must not read as an answer: {}",
            d.message
        );

        // The eye wording is the one that was already there, unchanged.
        let eye_shot = plan.shots.iter().find(|s| s.kind == "eye").unwrap();
        let d = empty_frame_diagnostic("piece", eye_shot, &f);
        assert_eq!(d.code, DW_ANCHOR_EYE);
        assert!(d.message.contains("the eye shot for"), "{}", d.message);
        assert!(d.message.contains("anchor/keeper"), "{}", d.message);

        // …and a fitted planned shot says the third thing: nothing to fit.
        let top = plan.shots.iter().find(|s| s.name == "top").unwrap();
        let d = empty_frame_diagnostic("piece", top, &f);
        assert_eq!(d.code, DW_ANCHOR_EYE);
        assert!(d.message.contains("nothing to fit"), "{}", d.message);
    }
}
