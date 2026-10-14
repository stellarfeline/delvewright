//! **A shot frames its subject** (spec-0110): every cutscene shot declares what
//! it shows, and the build proves the subject is in the frame, in sight,
//! present and drawn for the whole shot.
//!
//! The frame and the sight line are properties of a shot, so they live here and
//! not in the verb that first needed them: [`sight`] is the one function that
//! answers "is this inside the picture and in view", and `delvec snapshot`'s
//! manifest computes its `occluded` / `out_of_frame` fields by calling it.
//!
//! ## The proof frame (§5.1)
//!
//! Every shot is judged against the client the game ships: a vertical field of
//! view of [`PROOF_FOV_DEG`] on an 854 × 480 window ([`PROOF_ASPECT`]),
//! narrowed to [`PROOF_FOV_FLUID_DEG`] at any tick the lens's cell holds water
//! or lava; `entityDistanceScaling` 1.0 and the `viewScale` floor 1.0 for a body
//! with no `view_range`; `entity-broadcast-range-percentage` 100. Cited from the
//! pinned 1.21.11 jar (`Options`, `GameRenderer.getFov`, `Main`), spec-0110 §2.
//!
//! ## What is judged per shot, per tick (§5.2–§5.8)
//!
//! - **In frame** — the subject's reference point projects in front of the near
//!   plane and inside the action-safe rectangle, `|x|, |y| ≤` [`ACTION_SAFE`] in
//!   normalised device coordinates (EBU R 95). `DW1033`.
//! - **In sight** — of nine lines from the lens to the subject's sight box (its
//!   centre and its eight corners inset), at least one crosses no cell
//!   `World::blocks_camera` holds for, the subject's own box excluded. `DW1033`.
//! - **Present** — a body subject is placed before the cutscene on every path
//!   and not removed between: the one ordering rule,
//!   [`delvewright_dsl::ordering::Order`]. `DW1034`.
//! - **Sent** — the horizontal distance from the lens to the body is within its
//!   type's tracking range, capped by the served chunk distance. `DW1034`.
//! - **Drawn** — an NPC or actor body is within `size × 64` of the lens (its
//!   type's box, [`body_of`]); an assembly is drawn because the engine writes
//!   its `view_range` ([`view_ranges`]). `DW1034`.
//!
//! A `"landscape"` passes the frame and sight proofs with zero objects stated.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use delvewright_dsl::ordering::{Beat, Order, Presence};
use delvewright_dsl::{CameraSubject, DwCode, ExitTier};
use serde_json::{Value, json};

use crate::compiler::camera::{MoveCtx, SubjectGeom, expand_shot, resolve_subject};
use crate::compiler::failure::Failure;
use crate::compiler::nav::{ActorMovePlan, MovePlan, World, walk_cells};
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1033` (spec-0110 §6.1): a cutscene shot does not show its subject —
    /// at some judged tick the subject (or `subject_b`) is out of frame (its
    /// reference point outside the action-safe rectangle of the proof frame)
    /// or occluded (all nine sight lines blocked). Never fires on
    /// `"landscape"`.
    pub const DW_SHOT_UNFRAMED: DwCode = DwCode::new("DW1033", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1034` (spec-0110 §6.2): a body subject is not there to be seen — not
    /// present at the shot's first tick (no placing ordered before the
    /// cutscene, or an unconditional removal between), not sent to the viewer
    /// at some tick (farther than its type's tracking range), or, for an NPC or
    /// actor, not drawn at the proof frame (farther than its cull radius).
    pub const DW_SUBJECT_ABSENT: DwCode = DwCode::new("DW1034", ExitTier::Build);
}

/// The proof frame's vertical field of view, degrees: `options.fov`'s default.
pub const PROOF_FOV_DEG: f64 = 70.0;
/// The vertical field of view at a tick the lens's cell holds water or lava:
/// `70 × 6/7` (`GameRenderer.getFov`, `fovEffectScale` 1.0).
pub const PROOF_FOV_FLUID_DEG: f64 = 60.0;
/// The proof frame's aspect: the default 854 × 480 window.
pub const PROOF_ASPECT: f64 = 854.0 / 480.0;
/// The action-safe rectangle's half-extent in normalised device coordinates:
/// 3.5% in from each edge (EBU R 95).
pub const ACTION_SAFE: f64 = 0.93;
/// The client's near plane, blocks (`GameRenderer.getProjectionMatrix`).
pub const NEAR_PLANE: f64 = 0.05;
/// How far inside its box a corner sight line ends (the snapshot's rule,
/// clamped so a one-cell box still yields eight distinct points).
pub const SIGHT_INSET: f64 = 0.3;
/// Blocks per unit of `size` (or of `view_range`) a body is drawn within at
/// `viewScale` 1.0 (`Entity.shouldRenderAtSqrDistance`).
pub const CULL_PER_SIZE: f64 = 64.0;
/// The lowest `entityDistanceScaling` the client offers; an assembly's derived
/// `view_range` holds at it.
pub const ENTITY_DISTANCE_FLOOR: f64 = 0.5;
/// What the engine cannot control and does not check (§5.9).
pub const UNCHECKED: [&str; 8] = [
    "options.fov",
    "window aspect",
    "options.entityDistanceScaling",
    "options.fovEffectScale",
    "client render distance",
    "client occlusion culling of the section a body stands in",
    "a body hidden by another entity, a stand-in, a particle or weather",
    "the client's frame rate between ticks",
];

// ---------------------------------------------------------------------------
// The sight rule
// ---------------------------------------------------------------------------

/// An inclusive box of cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellBox {
    /// Inclusive minimum cell.
    pub min: [i32; 3],
    /// Inclusive maximum cell.
    pub max: [i32; 3],
}

impl CellBox {
    /// The box of one cell.
    pub fn cell(c: [i32; 3]) -> CellBox {
        CellBox { min: c, max: c }
    }

    /// The box spanning two corners in any order.
    pub fn spanning(a: [i32; 3], b: [i32; 3]) -> CellBox {
        CellBox {
            min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
            max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        }
    }

    /// The box's centre in world coordinates (cell centres).
    pub fn centre(&self) -> [f64; 3] {
        [
            (self.min[0] + self.max[0] + 1) as f64 / 2.0,
            (self.min[1] + self.max[1] + 1) as f64 / 2.0,
            (self.min[2] + self.max[2] + 1) as f64 / 2.0,
        ]
    }

    /// Whether `c` is inside the box.
    pub fn contains(&self, c: [i32; 3]) -> bool {
        (0..3).all(|a| c[a] >= self.min[a] && c[a] <= self.max[a])
    }

    /// The nine points sight lines end at: the centre, then the eight corners
    /// inset by [`SIGHT_INSET`] (clamped to 0.2 of a half-extent).
    pub fn sight_points(&self) -> [[f64; 3]; 9] {
        let c = self.centre();
        let half = |a: usize| ((self.max[a] - self.min[a] + 1) as f64 / 2.0 - SIGHT_INSET).max(0.2);
        let h = [half(0), half(1), half(2)];
        let mut out = [c; 9];
        for (i, p) in out.iter_mut().skip(1).enumerate() {
            *p = [
                c[0] + if i & 1 == 0 { -h[0] } else { h[0] },
                c[1] + if i & 2 == 0 { -h[1] } else { h[1] },
                c[2] + if i & 4 == 0 { -h[2] } else { h[2] },
            ];
        }
        out
    }
}

/// A camera pose in Minecraft degrees: `yaw 0` faces +Z, pitch positive looks
/// down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// The lens.
    pub pos: [f64; 3],
    /// Minecraft yaw, degrees.
    pub yaw: f64,
    /// Minecraft pitch, degrees.
    pub pitch: f64,
}

/// What one camera sees of one subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sight {
    /// The reference point lies in front of the near plane and inside the
    /// action-safe rectangle.
    pub in_frame: bool,
    /// How many of the nine sight lines cross no camera-blocking cell outside
    /// the subject's own box.
    pub lines_clear: u8,
    /// The first blocking cell on the centre line, when it is blocked.
    pub centre_block: Option<[i32; 3]>,
}

impl Sight {
    /// All nine lines blocked.
    pub fn occluded(&self) -> bool {
        self.lines_clear == 0
    }
}

/// **The one sight rule** (spec-0110 §5.3–§5.5): is `reference` inside the
/// picture a camera at `pose` takes with vertical field of view `fov_v` and
/// aspect `aspect`, and how many of the nine lines from the lens to `sight_box`
/// cross no cell `blocks` holds for — the box's own cells excluded, because a
/// mark that is a block must not hide itself and a place's walls must not hide
/// the place.
pub fn sight(
    pose: &Pose,
    fov_v: f64,
    aspect: f64,
    reference: [f64; 3],
    sight_box: &CellBox,
    blocks: &dyn Fn([i32; 3]) -> bool,
) -> Sight {
    let cam = crate::compiler::snapshot::Camera {
        pos: pose.pos,
        yaw: pose.yaw,
        pitch: pose.pitch,
        fov: fov_v,
    };
    let in_frame = ndc(&cam, aspect, reference)
        .is_some_and(|[x, y]| round6(x.abs()) <= ACTION_SAFE && round6(y.abs()) <= ACTION_SAFE);
    let blocked = |to: [f64; 3]| walk_cells(pose.pos, to, |c| blocks(c) && !sight_box.contains(c));
    let pts = sight_box.sight_points();
    let centre_block = blocked(pts[0]);
    let lines_clear = pts
        .iter()
        .enumerate()
        .filter(|(i, p)| {
            if *i == 0 {
                centre_block.is_none()
            } else {
                blocked(**p).is_none()
            }
        })
        .count() as u8;
    Sight {
        in_frame,
        lines_clear,
        centre_block,
    }
}

/// A point's normalised device coordinates through `cam`, or `None` when it is
/// not in front of the near plane.
fn ndc(cam: &crate::compiler::snapshot::Camera, aspect: f64, p: [f64; 3]) -> Option<[f64; 2]> {
    let (f, r, u) = cam.basis();
    let c = [p[0] - cam.pos[0], p[1] - cam.pos[1], p[2] - cam.pos[2]];
    let z = dot(c, f);
    if z <= NEAR_PLANE {
        return None;
    }
    let ty = (cam.fov.to_radians() * 0.5).tan();
    Some([dot(c, r) / (z * ty * aspect), dot(c, u) / (z * ty)])
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn round6(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

fn round3(v: f64) -> f64 {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 { 0.0 } else { r }
}

fn round3v(p: [f64; 3]) -> [f64; 3] {
    [round3(p[0]), round3(p[1]), round3(p[2])]
}

// ---------------------------------------------------------------------------
// Bodies: the pinned entity table
// ---------------------------------------------------------------------------

/// One entity type's body as the pinned server registers it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityBody {
    /// Bounding-box width, blocks.
    pub width: f64,
    /// Bounding-box height, blocks.
    pub height: f64,
    /// `clientTrackingRange`, chunks.
    pub tracking_range: u32,
}

impl EntityBody {
    /// `AABB.getSize()`: the mean of the box's three extents.
    pub fn size(&self) -> f64 {
        (2.0 * self.width + self.height) / 3.0
    }

    /// The distance beyond which the client does not draw the body at the
    /// proof frame: `size × 64`.
    pub fn cull_radius(&self) -> f64 {
        self.size() * CULL_PER_SIZE
    }

    /// The distance the server sends the body within, blocks, before the
    /// served chunk distance caps it.
    pub fn tracking_blocks(&self) -> f64 {
        f64::from(self.tracking_range) * 16.0
    }
}

/// The pinned entity table (`crates/delvec/data/entity-bodies-1.21.11.json`,
/// written by `tools/maintenance/extract-entity-bodies.py`).
pub fn body_of(entity: &str) -> Option<EntityBody> {
    static TABLE: OnceLock<BTreeMap<String, EntityBody>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let raw: BTreeMap<String, Value> =
            serde_json::from_str(include_str!("../../data/entity-bodies-1.21.11.json"))
                .expect("the vendored entity-body table parses");
        raw.into_iter()
            .map(|(k, v)| {
                (
                    k,
                    EntityBody {
                        width: v["width"].as_f64().unwrap_or(0.0),
                        height: v["height"].as_f64().unwrap_or(0.0),
                        tracking_range: v["tracking_range"].as_u64().unwrap_or(0) as u32,
                    },
                )
            })
            .collect()
    });
    let id = if entity.contains(':') {
        entity.to_string()
    } else {
        format!("minecraft:{entity}")
    };
    table.get(&id).copied()
}

// ---------------------------------------------------------------------------
// Subject geometry
// ---------------------------------------------------------------------------

/// The box a subject's sight lines end in, for every kind with a fixed box:
/// a mark's cell, an assembly's envelope, a place's box, a region's box, a
/// standing body's feet and head cells. `None` for a `"landscape"` and for a
/// reference that does not resolve (`DW0112`/`DW0360` report it).
pub fn subject_box(plan: &Plan, subject: &CameraSubject) -> Option<CellBox> {
    match subject {
        CameraSubject::Landscape(_) => None,
        CameraSubject::Anchor(m) => {
            let p = crate::compiler::nav::anchor_offset_point(plan, m.anchor.as_str(), m.offset);
            Some(CellBox::cell(cell_of(p)))
        }
        CameraSubject::Assembly(s) => assembly_envelope(plan, s.assembly.as_str()),
        CameraSubject::Place(s) => crate::compiler::horizon::place_bounds(plan, &s.place)
            .map(|(lo, hi)| CellBox::spanning(lo, hi)),
        CameraSubject::Region(s) => plan
            .zone_box(&s.region)
            .map(|(lo, hi)| CellBox::spanning(lo, hi)),
        CameraSubject::Npc(_) | CameraSubject::Actor(_) => {
            let feet = body_feet_static(plan, subject)?;
            Some(body_cells(feet))
        }
    }
}

/// An assembly's envelope (spec-0110 §4.1): the bounding box of the union of
/// its parts' cells over its spawn pose and every frame of every clip, turned
/// to its facing.
pub fn assembly_envelope(plan: &Plan, id: &str) -> Option<CellBox> {
    let placed = crate::compiler::assembly::placed(plan);
    let p = placed.iter().find(|p| p.decl.id.as_str() == id)?;
    let mut cells: BTreeSet<[i32; 3]> = p.world_cells(&p.spawn_pose());
    for clip in p.rig.clips.values() {
        for frame in &clip.frames {
            cells.extend(p.world_cells(frame));
        }
    }
    cells.insert(p.mark);
    let mut it = cells.iter();
    let first = *it.next()?;
    let mut b = CellBox::cell(first);
    for c in it {
        b = CellBox::spanning(
            [b.min[0].min(c[0]), b.min[1].min(c[1]), b.min[2].min(c[2])],
            [b.max[0].max(c[0]), b.max[1].max(c[1]), b.max[2].max(c[2])],
        );
    }
    Some(b)
}

/// An assembly's root position: the mark cell's centre at its floor plane.
fn assembly_root(plan: &Plan, id: &str) -> Option<[f64; 3]> {
    crate::compiler::assembly::placed(plan)
        .iter()
        .find(|p| p.decl.id.as_str() == id)
        .map(|p| {
            [
                p.mark[0] as f64 + 0.5,
                p.mark[1] as f64,
                p.mark[2] as f64 + 0.5,
            ]
        })
}

fn cell_of(p: [f64; 3]) -> [i32; 3] {
    [
        p[0].floor() as i32,
        p[1].floor() as i32,
        p[2].floor() as i32,
    ]
}

/// A standing body's two cells.
fn body_cells(feet: [i32; 3]) -> CellBox {
    CellBox {
        min: feet,
        max: [feet[0], feet[1] + 1, feet[2]],
    }
}

/// A static npc/actor body's feet cell: its declared mark.
fn body_feet_static(plan: &Plan, subject: &CameraSubject) -> Option<[i32; 3]> {
    let c = plan.campaign;
    match subject {
        CameraSubject::Npc(s) => c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id == s.npc)
            .and_then(|n| plan.body_point(delvewright_dsl::BodyRef::Npc(n))),
        CameraSubject::Actor(s) => c
            .quests
            .content
            .actors
            .iter()
            .find(|a| a.id == s.actor)
            .and_then(|a| plan.body_point(delvewright_dsl::BodyRef::Actor(a))),
        _ => None,
    }
}

/// The entity type a body subject wears.
fn body_entity(plan: &Plan, subject: &CameraSubject) -> Option<String> {
    let c = plan.campaign;
    match subject {
        CameraSubject::Npc(s) => c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id == s.npc)
            .map(crate::compiler::nav::npc_body_entity),
        CameraSubject::Actor(s) => c
            .quests
            .content
            .actors
            .iter()
            .find(|a| a.id == s.actor)
            .map(crate::compiler::nav::actor_body_entity),
        CameraSubject::Assembly(_) => Some("minecraft:item_display".to_string()),
        _ => None,
    }
}

/// A subject resolved over one shot window: per tick (`len == ticks + 1`, or
/// one entry for a fixed subject), its reference point, its sight box and —
/// for a body — the position its entity stands at.
struct Track {
    reference: Vec<[f64; 3]>,
    boxes: Vec<CellBox>,
    body: Vec<[f64; 3]>,
}

impl Track {
    fn at<T: Copy>(v: &[T], t: i32) -> T {
        v[(t.max(0) as usize).min(v.len() - 1)]
    }
}

#[allow(clippy::too_many_arguments)]
fn track_of(
    plan: &Plan,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
    subject: &CameraSubject,
    ctx: &[MoveCtx],
    shot_offset: i32,
    ticks: i32,
) -> Option<Track> {
    match subject {
        CameraSubject::Landscape(_) => None,
        CameraSubject::Npc(_) | CameraSubject::Actor(_) => {
            let geom = resolve_subject(plan, moves, actor_moves, subject, ctx, shot_offset, ticks)?;
            let o = subject.offset();
            let lift = [o[0] as f64, o[1] as f64 + 1.0, o[2] as f64];
            let refs = match geom {
                SubjectGeom::Static(p) => vec![p],
                SubjectGeom::Moving(track) => track,
            };
            let feet: Vec<[i32; 3]> = refs
                .iter()
                .map(|r| cell_of([r[0] - lift[0], r[1] - lift[1], r[2] - lift[2]]))
                .collect();
            Some(Track {
                boxes: feet.iter().map(|f| body_cells(*f)).collect(),
                body: feet
                    .iter()
                    .map(|f| [f[0] as f64 + 0.5, f[1] as f64, f[2] as f64 + 0.5])
                    .collect(),
                reference: refs,
            })
        }
        _ => {
            let b = subject_box(plan, subject)?;
            let reference = match subject {
                CameraSubject::Anchor(m) => {
                    crate::compiler::nav::anchor_offset_point(plan, m.anchor.as_str(), m.offset)
                }
                _ => b.centre(),
            };
            let body = match subject {
                CameraSubject::Assembly(s) => assembly_root(plan, s.assembly.as_str())?,
                _ => reference,
            };
            Some(Track {
                reference: vec![reference],
                boxes: vec![b],
                body: vec![body],
            })
        }
    }
}

// ---------------------------------------------------------------------------
// The proof
// ---------------------------------------------------------------------------

/// One judged subject of one shot: what the ledger keeps.
#[derive(Clone, Debug)]
pub struct SubjectRecord {
    /// `subject` or `subject_b`.
    pub field: &'static str,
    /// The subject's kind token.
    pub kind: &'static str,
    /// The subject's canonical rendering.
    pub canon: String,
    /// Its reference point at tick 0.
    pub reference: [f64; 3],
    /// Per tick: in the action-safe frame.
    pub in_frame: Vec<bool>,
    /// Per tick: clear sight lines of nine.
    pub lines_clear: Vec<u8>,
    /// For a body: the presence derivation.
    pub presence: Option<String>,
    /// For a body: the farthest horizontal distance (sent), blocks.
    pub farthest_horizontal: Option<f64>,
    /// For a body: the farthest lens-to-body distance (drawn), blocks.
    pub farthest: Option<f64>,
    /// For a body: the type's tracking range, capped by the served distance.
    pub tracking_range: Option<f64>,
    /// For an NPC or actor: its cull radius at the proof frame.
    pub cull_radius: Option<f64>,
    /// For a body: the entity type a client sees (an assembly's root is an
    /// `item_display`).
    pub entity: Option<String>,
    /// For a body: where its entity stands at the shot's first tick.
    pub body_at: Option<[f64; 3]>,
}

/// One judged shot.
#[derive(Clone, Debug)]
pub struct ShotRecord {
    /// Shot index within its cutscene.
    pub index: usize,
    /// Driver ticks into the cutscene at which the shot starts.
    pub start_tick: i32,
    /// Driver ticks.
    pub ticks: i32,
    /// The subjects judged (empty for a `"landscape"`).
    pub subjects: Vec<SubjectRecord>,
    /// The declared subject's kind token.
    pub kind: &'static str,
}

/// One cutscene's shots.
#[derive(Clone, Debug)]
pub struct CutsceneRecord {
    /// JSON pointer of the `cutscene` effect.
    pub path: String,
    /// The canonical hash of the shot list measured.
    pub digest: String,
    /// Where the lens stands at the cutscene's first tick — where a watching
    /// client is snapped to.
    pub start_lens: [f64; 3],
    /// The shots.
    pub shots: Vec<ShotRecord>,
}

/// Every judged cutscene, plus what the assemblies' `view_range` reads from it.
#[derive(Clone, Debug, Default)]
pub struct ShotLedger {
    /// The cutscenes, in emission order.
    pub cutscenes: Vec<CutsceneRecord>,
    /// Per assembly id: the greatest lens-to-root distance over every shot
    /// framing it, every tick.
    pub assembly_reach: BTreeMap<String, f64>,
}

/// One assembly's emitted `view_range` and the terms it is the maximum of.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblyRange {
    /// The assembly id.
    pub id: String,
    /// The emitted value (two decimals, rounded up).
    pub view_range: f64,
    /// `seen_from / 32`, when declared.
    pub seen_from: Option<f64>,
    /// The farthest shot distance / 32, when a shot frames it.
    pub shots: Option<f64>,
    /// The farthest standable cell of its place / 32.
    pub place: f64,
    /// Which term won: `floor`, `seen_from`, `shots` or `place`.
    pub won: &'static str,
}

impl ShotLedger {
    fn counts(&self) -> Counts {
        let mut n = Counts {
            cutscenes: self.cutscenes.len(),
            ..Counts::default()
        };
        for c in &self.cutscenes {
            for s in &c.shots {
                n.shots += 1;
                match s.kind {
                    "landscape" => n.landscape += 1,
                    "anchor" => n.marks += 1,
                    "npc" | "actor" => n.bodies += 1,
                    "assembly" => n.assemblies += 1,
                    "place" => n.places += 1,
                    "region" => n.regions += 1,
                    _ => {}
                }
                if s.kind != "none" {
                    n.declared += 1;
                }
                for r in &s.subjects {
                    let t = r.in_frame.len();
                    n.ticks += t;
                    n.out_of_frame += r.in_frame.iter().filter(|f| !**f).count();
                    n.occluded += r.lines_clear.iter().filter(|c| **c == 0).count();
                    n.clear += r.lines_clear.iter().map(|c| *c as usize).sum::<usize>();
                    if let Some(f) = r.farthest {
                        n.body_subjects += 1;
                        n.farthest = n.farthest.max(f);
                    }
                    if let (Some(f), Some(cr)) = (r.farthest, r.cull_radius) {
                        let s = f / cr;
                        n.slack = Some(n.slack.map_or(s, |x: f64| x.max(s)));
                    }
                }
            }
        }
        n
    }

    /// The binding line (spec-0110 §7), every figure computed from the
    /// declarations, zeroes included.
    pub fn line(&self) -> String {
        let n = self.counts();
        format!(
            "shot subjects: {} shot(s) in {} cutscene(s), {} declare a subject ({} landscape, {} \
             marks, {} bodies, {} assemblies, {} places, {} regions); {} tick(s) judged, {} out of \
             frame, {} occluded, clear lines {} of {}; bodies: {} present, sent, drawn — farthest \
             {:.1} blocks, hidden below entity distance {}",
            n.shots,
            n.cutscenes,
            n.declared,
            n.landscape,
            n.marks,
            n.bodies,
            n.assemblies,
            n.places,
            n.regions,
            n.ticks,
            n.out_of_frame,
            n.occluded,
            n.clear,
            9 * n.ticks,
            n.body_subjects,
            n.farthest,
            n.slack
                .map(|s| format!("{:.2}", (s * 100.0).ceil() / 100.0))
                .unwrap_or_else(|| "none (no npc or actor framed)".to_string()),
        )
    }

    /// `validation/shot-subjects.json`.
    pub fn to_json(&self, ranges: &[AssemblyRange]) -> Value {
        let n = self.counts();
        json!({
            "proof_frame": {
                "fov_vertical_degrees": PROOF_FOV_DEG,
                "fov_in_fluid_degrees": PROOF_FOV_FLUID_DEG,
                "window": [854, 480],
                "action_safe_ndc": ACTION_SAFE,
                "entity_distance_scaling": 1.0,
                "view_scale": 1.0,
                "entity_broadcast_range_percentage": 100,
            },
            "counts": {
                "cutscenes": n.cutscenes,
                "shots": n.shots,
                "declared": n.declared,
                "landscape": n.landscape,
                "marks": n.marks,
                "bodies": n.bodies,
                "assemblies": n.assemblies,
                "places": n.places,
                "regions": n.regions,
                "ticks_judged": n.ticks,
                "out_of_frame": n.out_of_frame,
                "occluded": n.occluded,
                "clear_lines": n.clear,
                "clear_lines_of": 9 * n.ticks,
            },
            "cutscenes": self.cutscenes.iter().map(|c| json!({
                "path": c.path,
                "cutscene_digest": c.digest,
                "start_lens": round3v(c.start_lens),
                "shots": c.shots.iter().map(|s| json!({
                    "index": s.index,
                    "start_tick": s.start_tick,
                    "ticks": s.ticks,
                    "kind": s.kind,
                    "subjects": s.subjects.iter().map(|r| json!({
                        "field": r.field,
                        "kind": r.kind,
                        "subject": r.canon,
                        "reference": round3v(r.reference),
                        "in_frame": r.in_frame,
                        "lines_clear": r.lines_clear,
                        "presence": r.presence,
                        "farthest_horizontal": r.farthest_horizontal.map(round3),
                        "farthest": r.farthest.map(round3),
                        "tracking_range": r.tracking_range,
                        "cull_radius": r.cull_radius.map(round3),
                        "entity": r.entity,
                        "body_at": r.body_at.map(round3v),
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "assemblies": ranges.iter().map(|r| json!({
                "id": r.id,
                "view_range": r.view_range,
                "terms": {
                    "floor": 1.0,
                    "seen_from": r.seen_from.map(round3),
                    "shots": r.shots.map(round3),
                    "place": round3(r.place),
                },
                "won": r.won,
            })).collect::<Vec<_>>(),
            "unchecked": UNCHECKED,
        })
    }
}

#[derive(Default)]
struct Counts {
    cutscenes: usize,
    shots: usize,
    declared: usize,
    landscape: usize,
    marks: usize,
    bodies: usize,
    assemblies: usize,
    places: usize,
    regions: usize,
    ticks: usize,
    out_of_frame: usize,
    occluded: usize,
    clear: usize,
    body_subjects: usize,
    farthest: f64,
    slack: Option<f64>,
}

/// Judge every cutscene shot (spec-0110 §5): returns the ledger, and every
/// refusal — the first per judged subject — when a shot does not show its
/// subject (`DW1033`) or a body subject is not there to be seen (`DW1034`).
/// The caller prints the binding line before taking the verdict.
pub fn judge(
    plan: &Plan,
    world: &World,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> (ShotLedger, Vec<Failure>) {
    let order = Order::of(plan.campaign);
    let served = delvewright_dsl::viewdistance::served_radius_blocks(
        delvewright_dsl::viewdistance::chunks(plan.campaign),
    );
    let blocks = |c: [i32; 3]| world.blocks_camera(c);
    let mut ledger = ShotLedger::default();
    let mut failures: Vec<Failure> = Vec::new();
    for (eff, ctx) in crate::compiler::camera::cutscene_units(plan.campaign) {
        let Some(shots) = eff.cutscene_shots() else {
            continue;
        };
        if shots.is_empty() {
            continue;
        }
        let path = order.path_of(eff).unwrap_or("").to_string();
        let mut record = CutsceneRecord {
            path: path.clone(),
            digest: crate::compiler::emit::cutscene_digest(&shots),
            start_lens: [0.0; 3],
            shots: Vec::new(),
        };
        let mut offset: i32 = 0;
        for (si, shot) in shots.iter().enumerate() {
            let ex = expand_shot(plan, moves, actor_moves, shot, &ctx, offset);
            let shot_offset = offset;
            offset += ex.ticks + 1;
            let poses = ex.client_poses();
            if si == 0 {
                record.start_lens = poses.first().map_or([0.0; 3], |p| p.0);
            }
            let mut sr = ShotRecord {
                index: si,
                start_tick: shot_offset,
                ticks: ex.ticks,
                subjects: Vec::new(),
                kind: shot.subject.as_ref().map_or("none", CameraSubject::kind),
            };
            for (field, subject) in [("subject", &shot.subject), ("subject_b", &shot.subject_b)] {
                let Some(subject) = subject else { continue };
                let Some(track) = track_of(
                    plan,
                    moves,
                    actor_moves,
                    subject,
                    &ctx,
                    shot_offset,
                    ex.ticks,
                ) else {
                    continue;
                };
                let rec = judge_subject(
                    plan,
                    world,
                    &order,
                    eff,
                    &path,
                    si,
                    &poses,
                    field,
                    subject,
                    &track,
                    served,
                    &blocks,
                    &mut failures,
                );
                if let (CameraSubject::Assembly(s), Some(f)) = (subject, rec.farthest) {
                    let e = ledger
                        .assembly_reach
                        .entry(s.assembly.as_str().to_string())
                        .or_insert(0.0);
                    *e = e.max(f);
                }
                sr.subjects.push(rec);
            }
            record.shots.push(sr);
        }
        ledger.cutscenes.push(record);
    }
    (ledger, failures)
}

/// Judge one subject of one shot. A body is asked first whether it is there
/// to be seen (`DW1034`: present, sent, drawn) and only then whether the frame
/// shows it (`DW1033`), because the frame of a body that is not there is not
/// the defect to name.
#[allow(clippy::too_many_arguments)]
fn judge_subject(
    plan: &Plan,
    world: &World,
    order: &Order<'_>,
    eff: &delvewright_dsl::QuestEffect,
    path: &str,
    si: usize,
    poses: &[([f64; 3], f64, f64)],
    field: &'static str,
    subject: &CameraSubject,
    track: &Track,
    served: f64,
    blocks: &dyn Fn([i32; 3]) -> bool,
    failures: &mut Vec<Failure>,
) -> SubjectRecord {
    let canon = subject.canon();
    let mut rec = SubjectRecord {
        field,
        kind: subject.kind(),
        canon: canon.clone(),
        reference: track.reference[0],
        in_frame: Vec::new(),
        lines_clear: Vec::new(),
        presence: None,
        farthest_horizontal: None,
        farthest: None,
        tracking_range: None,
        cull_radius: None,
        entity: None,
        body_at: None,
    };
    // The first refusal per judged subject: a subject that leaves the frame
    // for forty ticks is one finding, not forty.
    let mut failure: Option<Failure> = None;
    let mut fail = |code: DwCode, message: String| {
        if failure.is_none() {
            failure = Some(Failure { code, message });
        }
    };
    if subject.is_body() {
        // §5.6: present at the shot's first tick.
        let body_id = subject.id();
        match order.presence(body_id, Beat::Effect(eff)) {
            Presence::Present(p) => rec.presence = Some(format!("placed by {p}")),
            Presence::NeverPlaced(found) => {
                rec.presence = Some("never placed before the cutscene".to_string());
                let placing = match subject {
                    CameraSubject::Assembly(_) => "spawn-assembly",
                    CameraSubject::Actor(_) => "spawn-actor",
                    _ => "spawn-npc",
                };
                fail(
                    DW_SUBJECT_ABSENT,
                    format!(
                        "cutscene {path}: shot {si} frames `{body_id}`, which no beat places \
                         before the cutscene on every path ({}) — the shot shows an empty mark. \
                         Put a `{placing}` ahead of the cutscene: earlier in the same effect list, \
                         in an earlier `sequence` step, or in a beat the quest graph orders before \
                         it, outside any flag gate and any reaction bundle",
                        if found.is_empty() {
                            "nothing places it at all".to_string()
                        } else {
                            format!(
                                "placings found at {}, none ordered before",
                                found.join(", ")
                            )
                        }
                    ),
                );
            }
            Presence::Removed(p, r) => {
                rec.presence = Some(format!("placed by {p}, removed by {r}"));
                fail(
                    DW_SUBJECT_ABSENT,
                    format!(
                        "cutscene {path}: shot {si} frames `{body_id}`, placed by {p} but removed \
                         by {r}, which is ordered between that placing and the cutscene — the \
                         shot shows an empty mark. Order the removal after the cutscene, or place \
                         the body again before it"
                    ),
                );
            }
        }
        // §5.7 / §5.8: sent and drawn, every tick.
        let entity = body_entity(plan, subject).unwrap_or_default();
        rec.entity = Some(entity.clone());
        rec.body_at = Some(Track::at(&track.body, 0));
        if let Some(body) = body_of(&entity) {
            rec.tracking_range = Some(body.tracking_blocks().min(served));
            let drawn_by_type = !matches!(subject, CameraSubject::Assembly(_));
            if drawn_by_type {
                rec.cull_radius = Some(body.cull_radius());
            }
            let mut far_h: f64 = 0.0;
            let mut far: f64 = 0.0;
            for (t, (pos, ..)) in poses.iter().enumerate() {
                let b = Track::at(&track.body, t as i32);
                let (dh, d) = distances(*pos, b);
                far_h = far_h.max(dh);
                far = far.max(d);
                if let Err(why) =
                    sent_and_drawn(t as i32, *pos, b, &entity, body, served, drawn_by_type)
                {
                    fail(
                        DW_SUBJECT_ABSENT,
                        format!(
                            "cutscene {path}: shot {si} frames `{body_id}`, and {why}. \
                             Bring the camera nearer the subject"
                        ),
                    );
                }
            }
            rec.farthest_horizontal = Some(far_h);
            rec.farthest = Some(far);
        }
    }
    // §5.4–§5.5: in frame and in sight, every tick.
    let judged = frame_and_sight(
        poses,
        &track.reference,
        &track.boxes,
        &|c| world.is_flooded(c),
        blocks,
    );
    rec.in_frame = judged.in_frame;
    rec.lines_clear = judged.lines_clear;
    if let Some(m) = judged.first_failure {
        fail(
            DW_SHOT_UNFRAMED,
            format!("cutscene {path}: shot {si} does not show its {field} `{canon}` {m}"),
        );
    }
    failures.extend(failure);
    rec
}

/// What [`frame_and_sight`] measured over one shot's ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameJudgement {
    /// Per tick: the reference point is inside the action-safe frame.
    pub in_frame: Vec<bool>,
    /// Per tick: clear sight lines of nine.
    pub lines_clear: Vec<u8>,
    /// The first failing tick, worded: `at tick T — the lens stands at …,
    /// and the subject is out of frame / occluded … Move the camera …`.
    pub first_failure: Option<String>,
}

/// **In frame and in sight at every tick** (spec-0110 §5.4–§5.5): per client
/// pose (`poses`, `(lens, yaw, pitch)`), the subject's reference point and
/// sight box at that tick (a one-entry slice holds still), judged against the
/// proof frame — [`PROOF_FOV_DEG`], or [`PROOF_FOV_FLUID_DEG`] at a tick the
/// lens's cell is `fluid` — over the cells `blocks` holds for.
pub fn frame_and_sight(
    poses: &[([f64; 3], f64, f64)],
    references: &[[f64; 3]],
    boxes: &[CellBox],
    fluid: &dyn Fn([i32; 3]) -> bool,
    blocks: &dyn Fn([i32; 3]) -> bool,
) -> FrameJudgement {
    let mut out = FrameJudgement {
        in_frame: Vec::new(),
        lines_clear: Vec::new(),
        first_failure: None,
    };
    for (t, (pos, yaw, pitch)) in poses.iter().enumerate() {
        let t = t as i32;
        let fov = if fluid(cell_of(*pos)) {
            PROOF_FOV_FLUID_DEG
        } else {
            PROOF_FOV_DEG
        };
        let pose = Pose {
            pos: *pos,
            yaw: *yaw,
            pitch: *pitch,
        };
        let reference = Track::at(references, t);
        let sbox = Track::at(boxes, t);
        let s = sight(&pose, fov, PROOF_ASPECT, reference, &sbox, blocks);
        out.in_frame.push(s.in_frame);
        out.lines_clear.push(s.lines_clear);
        if (!s.in_frame || s.occluded()) && out.first_failure.is_none() {
            let what = if !s.in_frame {
                format!(
                    "out of frame: its reference point {:?} projects outside the action-safe \
                     rectangle (|x|, |y| <= {ACTION_SAFE} in normalised device coordinates) of the \
                     proof frame ({fov}° vertical, 854 × 480)",
                    round3v(reference)
                )
            } else {
                format!(
                    "occluded: all nine sight lines to it are blocked, the centre line first by \
                     the block at {:?}",
                    s.centre_block.unwrap_or_default()
                )
            };
            out.first_failure = Some(format!(
                "at tick {t} — the lens stands at {:?} aiming yaw {:.1}, pitch {:.1}, and the \
                 subject is {what}. Move the camera `path`, aim it with `look_at`, adjust \
                 `bearing`/`dist` on a styled shot, or cut the shot in two where the subject \
                 leaves it",
                round3v(*pos),
                yaw,
                pitch
            ));
        }
    }
    out
}

/// `(horizontal, straight)` distance from the lens to a body.
fn distances(lens: [f64; 3], body: [f64; 3]) -> (f64, f64) {
    let dh = ((body[0] - lens[0]).powi(2) + (body[2] - lens[2]).powi(2)).sqrt();
    (dh, (dh * dh + (body[1] - lens[1]).powi(2)).sqrt())
}

/// **Sent and drawn** at one tick (spec-0110 §5.7, §5.8): the body at `body`
/// is within its type's tracking range (capped by the `served` radius)
/// horizontally from the lens, and — when `judged_drawn`, an NPC or actor — it
/// is nearer the lens than its cull radius at the proof frame. `Err` says
/// which failed, with the distance and the bound.
pub fn sent_and_drawn(
    tick: i32,
    lens: [f64; 3],
    body: [f64; 3],
    entity: &str,
    kind: EntityBody,
    served: f64,
    judged_drawn: bool,
) -> Result<(), String> {
    let (dh, d) = distances(lens, body);
    let tracking = kind.tracking_blocks().min(served);
    if round3(dh) > tracking {
        return Err(format!(
            "at tick {tick} the body stands {} blocks from the lens horizontally — beyond its tracking range \
             of {tracking} blocks (`{entity}` is tracked {} chunks, capped by the served {served} \
             blocks), so the server never sends it to the player watching",
            round3(dh),
            kind.tracking_range
        ));
    }
    if judged_drawn && round3(d) >= round3(kind.cull_radius()) {
        return Err(format!(
            "at tick {tick} the body stands {} blocks from the lens — at or beyond its cull radius of {} blocks \
             (`{entity}`, box size {} × 64 at the default entity distance), so the client does \
             not draw it",
            round3(d),
            round3(kind.cull_radius()),
            round3(kind.size())
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The assemblies' view range (§5.8)
// ---------------------------------------------------------------------------

/// Per assembly id: the farthest a standing player's eye in the place the
/// assembly's mark stands in is from its root — the floor its `view_range`
/// is derived to. `population` is the walked population: the cells the party
/// can stand in.
pub fn place_reach(plan: &Plan, population: &BTreeSet<[i32; 3]>) -> BTreeMap<String, f64> {
    let eye = delvewright_dsl::metrics::PLAYER_EYE_HEIGHT;
    let mut out = BTreeMap::new();
    for p in crate::compiler::assembly::placed(plan) {
        let root = [
            p.mark[0] as f64 + 0.5,
            p.mark[1] as f64,
            p.mark[2] as f64 + 0.5,
        ];
        let reach = crate::compiler::horizon::place_bounds(plan, &p.area)
            .map(|(lo, hi)| {
                let b = CellBox::spanning(lo, hi);
                population
                    .iter()
                    .filter(|c| b.contains(**c))
                    .map(|c| {
                        let e = [c[0] as f64 + 0.5, c[1] as f64 + eye, c[2] as f64 + 0.5];
                        ((e[0] - root[0]).powi(2)
                            + (e[1] - root[1]).powi(2)
                            + (e[2] - root[2]).powi(2))
                        .sqrt()
                    })
                    .fold(0.0_f64, f64::max)
            })
            .unwrap_or(0.0);
        out.insert(p.decl.id.as_str().to_string(), reach);
    }
    out
}

/// Every placed assembly's emitted `view_range` (spec-0110 §5.8):
/// `max(1.0, seen_from / 32, d_max / 32)` rounded up to two decimals, `d_max`
/// the greatest of the shot reach ([`ShotLedger::assembly_reach`]) and the
/// place reach ([`place_reach`]). The divisor is `64 × 0.5`, so the range holds
/// at the client's lowest entity distance.
pub fn view_ranges(
    plan: &Plan,
    shot_reach: &BTreeMap<String, f64>,
    place: &BTreeMap<String, f64>,
) -> Vec<AssemblyRange> {
    let div = CULL_PER_SIZE * ENTITY_DISTANCE_FLOOR;
    crate::compiler::assembly::placed(plan)
        .iter()
        .map(|p| {
            let id = p.decl.id.as_str().to_string();
            let seen_from = p.decl.seen_from.map(|s| f64::from(s.blocks()) / div);
            let shots = shot_reach.get(&id).map(|d| d / div);
            let place = place.get(&id).copied().unwrap_or(0.0) / div;
            let (view_range, won) = derive_range(seen_from, shots, place);
            AssemblyRange {
                id,
                view_range,
                seen_from,
                shots,
                place,
                won,
            }
        })
        .collect()
}

/// `max(1.0, seen_from, shots, place)` over terms already divided by 32,
/// rounded up to two decimals, and the name of the term that won (`floor`
/// on a tie with it).
pub fn derive_range(seen_from: Option<f64>, shots: Option<f64>, place: f64) -> (f64, &'static str) {
    let mut won = "floor";
    let mut v = 1.0_f64;
    for (name, term) in [
        ("seen_from", seen_from),
        ("shots", shots),
        ("place", Some(place)),
    ] {
        if let Some(x) = term
            && round6(x) > round6(v)
        {
            v = x;
            won = name;
        }
    }
    ((round6(v * 100.0)).ceil() / 100.0, won)
}

#[cfg(test)]
mod tests;
