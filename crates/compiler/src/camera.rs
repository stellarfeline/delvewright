//! Cutscene camera geometry (task #64 + spec-0015 shot styles): the
//! arc-length-parameterized, eased dolly with display-entity
//! `teleport_duration` keyframe cadence, and the deterministic `shot_style`
//! expansion. Shared between emission ([`crate::emit`]) and validation
//! ([`crate::nav`]) so the checks prove exactly the camera path the client
//! renders.
//!
//! ## Why keyframes, not per-tick teleports
//!
//! The pre-#64 dolly teleported the camera pair every tick with no
//! interpolation: a hard 20 Hz staircase — at 60–144 fps each pose is held for
//! 3–7 frames. Display entities carry the vanilla-intended smoothing primitive
//! `teleport_duration` (camera dossier, `docs/notes/camera-dossier.md` §1): the
//! *client* tweens a teleported display across N ticks. Emitting a waypoint
//! every N ticks with `teleport_duration:N` makes the client draw the
//! in-betweens — smoother than 20 Hz, and ~N× fewer commands.
//!
//! ## Spike measurements this module is built on (live, vanilla 1.21.11,
//! itzg/minecraft-server:java21 + RCON driver + a mineflayer packet observer;
//! task #64 spike, 2026-08-01)
//!
//! 1. `teleport_duration` is clamped server-side to `0..=59` ticks (summoning
//!    with `100` stores `59`; `-5` stores `0`) and is synced to the client as
//!    display-entity metadata.
//! 2. The tween is **client-side only**: polling `Pos`/`Rotation` server-side
//!    across a 40-tick tween shows both jump to the target within one tick.
//!    Server geometry (selectors, `data get`) never sees an intermediate pose —
//!    so validation must check the client-rendered chords, not just what the
//!    server stores.
//! 3. Teleporting a display with `teleport_duration` set emits exactly ONE
//!    position-sync packet carrying position AND yaw/pitch, with no follow-up
//!    movement packets: the client alone draws the in-betweens. Rotation
//!    interpolates along with position — "Display entities now start updating
//!    their client-side position and rotation on the first tick after an
//!    update. Duration of this interpolation is controlled by the field
//!    `teleport_duration`" (minecraft.wiki, *Display entity*, 23w31a).
//! 4. The per-tick `spectate` bounce emits only camera-switch and
//!    player-position packets — nothing addressed to the display entities — so
//!    it **cannot** reset an in-flight tween (the client's lerp state is a pure
//!    function of packets addressed to the tweening entity). The bounce stays.
//! 5. Within one server tick the position sync is flushed BEFORE entity
//!    metadata, regardless of command order inside the function: a same-tick
//!    `data merge {teleport_duration:N}` + `tp` applies the OLD duration to
//!    that `tp`. Emission exploits this at each shot's first tick (merge the
//!    new cadence + snap in one tick: the snap lands instantly under the old
//!    duration `0`) and otherwise keeps duration changes at least one tick
//!    ahead of the teleports they govern.
//!
//! ## Shot styles (spec-0015; camera dossier §2)
//!
//! [`expand_shot`] turns a `shot_style` + `subject` into the same
//! [`ExpandedShot`] geometry an explicit `path` produces — a **pure function**
//! of the style, its parameters, and the subject's resolved position or
//! compiler-planned motion track (`move-npc`/`move-actor`). Explicit
//! `path`/`look_at`/`seconds` always override the corresponding expanded part.
//! Placement is rule-based and documented per style (no world-aware candidate
//! search — the dossier §4 "compile-time ClearShot" scoring is future work);
//! if a placement clips or over-pans, `DW0308`/`DW0347` fail the build and the
//! author adjusts `bearing`/`dist`/`seconds` or overrides the path.
//!
//! ## Determinism
//!
//! Everything here is a pure function of the resolved shot geometry: no RNG, no
//! wall clock, no iteration over unordered maps. Emitted poses are rounded to 3
//! decimals ([`round3`]) and threshold comparisons round first, so a borderline
//! shot cannot flip a diagnostic across platforms on a libm ulp (ADR-0006).

use delvewright_dsl::{CameraShot, CameraSubject, ShotStyle};

use crate::nav::{ActorMovePlan, MovePlan};
use crate::plan::Plan;

/// Max *perpendicular* distance (blocks) the client-rendered chord between two
/// keyframes may cut inside the exact eased dolly path — the corner-cutting /
/// corridor bound ("N bounded by path curvature", camera dossier §1). Matches
/// the ≤ 0.25-block sampling step of the `DW0308` air-corridor check, so a
/// passing chord stays within one sampling step of the authored corridor.
/// Along-path (temporal) lag is deliberately NOT bounded here: it never leaves
/// the corridor, and its only perceptible effect — mis-aimed framing — is
/// bounded by [`CHORD_AIM_TOLERANCE_DEG`].
pub const CHORD_POS_TOLERANCE: f64 = 0.25;

/// Max angle (degrees) the client-rendered aim may deviate from the exact eased
/// aim between keyframes — the dossier's "comfortable" rotation band (§1).
pub const CHORD_AIM_TOLERANCE_DEG: f64 = 2.0;

/// Angular-rate budget: above this many degrees per tick (6 °/tick = 120 °/s)
/// a pan is nausea-tier at 20 Hz and the shot fails `DW0347`. The camera
/// dossier (§1) proposes ≤ 2 °/tick as comfortable and > 6 °/tick as the hard
/// limit; the spike rig has no rendering client, so the numbers are the
/// dossier's (anchored on Cinemachine's 0.5 s damping defaults), not
/// footage-calibrated.
pub const MAX_AIM_DEG_PER_TICK: f64 = 6.0;

/// Candidate keyframe cadences, best first. All divide 20, so whole-second
/// shots get a uniform grid; all are well under the 59-tick clamp; 10 caps the
/// easing granularity at half a second. Selection takes the first cadence whose
/// rendered chords stay within [`CHORD_POS_TOLERANCE`] /
/// [`CHORD_AIM_TOLERANCE_DEG`] of the exact eased path — a curvy path degrades
/// toward per-tick (`1`), which still buys sub-tick client smoothing.
const CADENCES: [i32; 5] = [10, 5, 4, 2, 1];

/// One emitted camera keyframe: the `tp` issued at shot-relative `tick`,
/// carrying the pose the client reaches `cadence` ticks later (instantly for
/// the tick-0 snap).
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Shot-relative tick the `tp` is issued at.
    pub tick: i32,
    /// Camera position (3-decimal rounded).
    pub pos: [f64; 3],
    /// Minecraft entity yaw, degrees (3-decimal rounded).
    pub yaw: f64,
    /// Minecraft entity pitch, degrees (3-decimal rounded).
    pub pitch: f64,
}

/// A shot's full keyframe plan: what [`crate::emit`] turns into commands and
/// what [`crate::nav`] ray-checks.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotFrames {
    /// `teleport_duration` for the shot's tweened keyframes. `0` = static shot:
    /// a single snap frame and no tween.
    pub cadence: i32,
    /// The tick-0 snap followed by the tweened keyframes, in issue order.
    pub frames: Vec<Frame>,
}

/// A shot's length in driver ticks — the single clamp shared by emission,
/// validation and the critical-path hint.
pub fn shot_ticks(seconds: u32) -> i32 {
    (seconds as i32 * 20).clamp(1, 400)
}

// ---------------------------------------------------------------------------
// Expanded shots (explicit paths and styled shots meet here)
// ---------------------------------------------------------------------------

/// What the shot aims at, per tick.
#[derive(Clone, Debug, PartialEq)]
pub enum AimTrack {
    /// Face along the direction of travel (the unstyled default).
    Travel,
    /// A fixed world point (`look_at`, or a static subject).
    Static([f64; 3]),
    /// A moving subject: its position per shot tick (`len == ticks + 1`).
    Moving(Vec<[f64; 3]>),
}

/// The shot's camera geometry.
#[derive(Clone, Debug, PartialEq)]
pub enum CameraPath {
    /// A waypoint polyline traversed arc-length-eased (explicit `path`s and
    /// most styles).
    Polyline(Vec<[f64; 3]>),
    /// A per-tick camera track (`len == ticks + 1`) locked to a moving subject
    /// (`side-track` / `low-follow`) — the subject's own motion profile
    /// governs, no easing.
    Track(Vec<[f64; 3]>),
}

/// A fully-resolved shot: geometry + aim + duration. The single input to
/// keyframe planning ([`ExpandedShot::frames`]), the clip check and the
/// angular-budget metric — emission and validation both consume exactly this.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpandedShot {
    /// Driver ticks.
    pub ticks: i32,
    /// Camera geometry.
    pub path: CameraPath,
    /// Aim target.
    pub aim: AimTrack,
}

impl ExpandedShot {
    /// The polyline whose corridor the `DW0308` clip check ray-marches: the
    /// waypoint polyline, or the dense per-tick track.
    pub fn clip_polyline(&self) -> &[[f64; 3]] {
        match &self.path {
            CameraPath::Polyline(p) | CameraPath::Track(p) => p,
        }
    }

    /// Exact per-tick pose samples (`len == ticks + 1`).
    fn samples(&self) -> Vec<[f64; 3]> {
        match &self.path {
            CameraPath::Polyline(pts) => {
                let path = Path::new(pts);
                (0..=self.ticks)
                    .map(|t| {
                        let tau = t as f64 / self.ticks as f64;
                        path.pos_at_arc(ease(tau) * path.total)
                    })
                    .collect()
            }
            CameraPath::Track(track) => (0..=self.ticks).map(|t| track_at(track, t)).collect(),
        }
    }

    /// The aim point at tick `t`, or `None` for travel-aim.
    fn aim_point(&self, t: i32) -> Option<[f64; 3]> {
        match &self.aim {
            AimTrack::Travel => None,
            AimTrack::Static(p) => Some(*p),
            AimTrack::Moving(track) => Some(track_at(track, t)),
        }
    }

    /// The exact unit aim direction at tick `t` given the pose samples, or
    /// `None` for a degenerate (no-direction) pose.
    fn aim_dir(&self, samples: &[[f64; 3]], t: i32) -> Option<[f64; 3]> {
        let pos = samples[t as usize];
        let d = match self.aim_point(t) {
            Some(s) => [s[0] - pos[0], s[1] - pos[1], s[2] - pos[2]],
            None => travel_dir(samples, t)?,
        };
        normalize(d)
    }

    /// The emitted `(yaw, pitch)` at tick `t` (3-decimal rounded; the summon
    /// default `(0, 0)` for a degenerate pose).
    fn aim_rot(&self, samples: &[[f64; 3]], t: i32) -> (f64, f64) {
        let pos = samples[t as usize];
        match self.aim_point(t) {
            Some(s) => aim_from_dir([s[0] - pos[0], s[1] - pos[1], s[2] - pos[2]]),
            None => match travel_dir(samples, t) {
                Some(d) => aim_from_dir(d),
                None => (0.0, 0.0),
            },
        }
    }

    /// Plan the shot's keyframes: pick the widest chord-safe cadence and bake
    /// the per-keyframe pose. A zero-motion shot (single waypoint, or a 1-tick
    /// shot with no room to move) is a static snap: one frame, cadence 0.
    pub fn frames(&self) -> ShotFrames {
        let samples = self.samples();
        let motionless = samples.windows(2).all(|w| w[0] == w[1]);
        let aim_static = !matches!(&self.aim, AimTrack::Moving(_));
        if (motionless && aim_static) || self.ticks <= 1 {
            let (yaw, pitch) = self.aim_rot(&samples, 0);
            return ShotFrames {
                cadence: 0,
                frames: vec![Frame {
                    tick: 0,
                    pos: round3_pos(samples[0]),
                    yaw,
                    pitch,
                }],
            };
        }
        let cadence = CADENCES
            .iter()
            .copied()
            .find(|&n| n < self.ticks && self.chords_within_tolerance(&samples, n))
            .unwrap_or(1);
        let mut frames = Vec::new();
        for (issue, arrival) in keyframe_grid(self.ticks, cadence) {
            let pos = samples[arrival as usize];
            let (yaw, pitch) = self.aim_rot(&samples, arrival);
            frames.push(Frame {
                tick: issue,
                pos: round3_pos(pos),
                yaw,
                pitch,
            });
        }
        ShotFrames { cadence, frames }
    }

    /// The peak angular rate (degrees per tick, 3-decimal rounded) of the exact
    /// aim across the shot — the `DW0347` budget metric. The client-rendered
    /// rate is a per-interval average of this, so the exact maximum bounds what
    /// a player can ever see, independent of the cadence chosen.
    pub fn max_aim_deg_per_tick(&self) -> f64 {
        if self.ticks <= 0 {
            return 0.0;
        }
        let samples = self.samples();
        let mut max = 0.0_f64;
        let mut prev = self.aim_dir(&samples, 0);
        for t in 1..=self.ticks {
            let cur = self.aim_dir(&samples, t);
            if let (Some(a), Some(b)) = (prev, cur) {
                max = max.max(angle_deg(a, b));
            }
            prev = cur;
        }
        round3(max)
    }

    /// True if, at cadence `n`, every client-rendered tick pose (linear
    /// position lerp + shortest-arc yaw/pitch lerp between keyframes —
    /// measurement 3) stays within [`CHORD_POS_TOLERANCE`] /
    /// [`CHORD_AIM_TOLERANCE_DEG`] of the exact pose. Comparisons round first
    /// so the choice is platform-stable.
    fn chords_within_tolerance(&self, samples: &[[f64; 3]], n: i32) -> bool {
        let nodes = keyframe_grid(self.ticks, n)
            .into_iter()
            .map(|(issue, arrival)| if issue == 0 { 0 } else { arrival })
            .collect::<Vec<_>>();
        for w in nodes.windows(2) {
            let (t0, t1) = (w[0], w[1]);
            let a_pos = samples[t0 as usize];
            let b_pos = samples[t1 as usize];
            let a_aim = self.aim_rot(samples, t0);
            let b_aim = self.aim_rot(samples, t1);
            for t in (t0 + 1)..t1.min(self.ticks) {
                let f = (t - t0) as f64 / (t1 - t0) as f64;
                // Corridor bound: the exact pose at tick t must lie within
                // tolerance of the rendered chord *segment* (perpendicular /
                // corner-cut distance — along-path lag is allowed, see
                // `CHORD_POS_TOLERANCE`).
                let exact = samples[t as usize];
                if round3(dist_to_segment(exact, a_pos, b_pos)) > CHORD_POS_TOLERANCE {
                    return false;
                }
                let rend_yaw = lerp_angle(a_aim.0, b_aim.0, f);
                let rend_pitch = a_aim.1 + (b_aim.1 - a_aim.1) * f;
                if let Some(exact_dir) = self.aim_dir(samples, t) {
                    let rend_dir = dir_from_aim(rend_yaw, rend_pitch);
                    if round3(angle_deg(rend_dir, exact_dir)) > CHORD_AIM_TOLERANCE_DEG {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// Plan an explicit-path shot (the pre-style API, kept for unit tests):
/// arc-length parameterize, ease, cadence-select, bake aims.
pub fn plan_shot(pts: &[[f64; 3]], subject: Option<[f64; 3]>, ticks: i32) -> ShotFrames {
    ExpandedShot {
        ticks,
        path: CameraPath::Polyline(if pts.is_empty() {
            vec![[0.0, crate::plan::BASE_Y as f64, 0.0]]
        } else {
            pts.to_vec()
        }),
        aim: match subject {
            Some(p) => AimTrack::Static(p),
            None => AimTrack::Travel,
        },
    }
    .frames()
}

/// The keyframe issue schedule for a moving shot as `(issue, arrival)` tick
/// pairs: the tick-0 snap, then a `tp` every `cadence` ticks starting at tick
/// 1, each targeting the pose the camera should hold when the tween lands
/// `cadence` ticks later (arrival clamped to the shot end — ease-out makes the
/// clipped tail sub-perceptual, and the next shot's snap or the restore
/// overrides it).
fn keyframe_grid(ticks: i32, cadence: i32) -> Vec<(i32, i32)> {
    let mut out = vec![(0, 0)];
    let mut s = 1;
    while s < ticks {
        out.push((s, (s + cadence).min(ticks)));
        s += cadence;
    }
    out
}

/// A per-tick track sample, clamped at both ends (a subject that has not
/// started moving yet, or has already arrived, holds its terminal position).
fn track_at(track: &[[f64; 3]], t: i32) -> [f64; 3] {
    if track.is_empty() {
        return [0.0, crate::plan::BASE_Y as f64, 0.0];
    }
    track[(t.max(0) as usize).min(track.len() - 1)]
}

/// The direction of travel at tick `t` over pose samples: the first nonzero
/// step at or after `t`, else the last nonzero step before it, else the overall
/// first → last direction, else `None` (a fully static shot).
fn travel_dir(samples: &[[f64; 3]], t: i32) -> Option<[f64; 3]> {
    let n = samples.len();
    if n < 2 {
        return None;
    }
    let step = |i: usize| {
        let d = [
            samples[i + 1][0] - samples[i][0],
            samples[i + 1][1] - samples[i][1],
            samples[i + 1][2] - samples[i][2],
        ];
        if d == [0.0, 0.0, 0.0] { None } else { Some(d) }
    };
    let t = (t.max(0) as usize).min(n - 2);
    for i in t..n - 1 {
        if let Some(d) = step(i) {
            return normalize(d);
        }
    }
    for i in (0..t).rev() {
        if let Some(d) = step(i) {
            return normalize(d);
        }
    }
    normalize([
        samples[n - 1][0] - samples[0][0],
        samples[n - 1][1] - samples[0][1],
        samples[n - 1][2] - samples[0][2],
    ])
}

// ---------------------------------------------------------------------------
// Shot-style expansion (spec-0015)
// ---------------------------------------------------------------------------

/// A sibling `move-npc`/`move-actor` visible to a cutscene: who moves, where
/// to, and how many ticks the move had already been running when the cutscene
/// started (negative: the move starts `-delta` ticks *after* the cutscene).
#[derive(Clone, Debug, PartialEq)]
pub struct MoveCtx {
    /// `true` for a `move-actor`, `false` for a `move-npc`.
    pub is_actor: bool,
    /// The npc/actor id.
    pub id: String,
    /// The move's destination anchor (disambiguates multiple moves).
    pub to_anchor: String,
    /// Ticks between the move's start and the cutscene's start.
    pub delta: i32,
}

/// Expand one shot to its resolved geometry: explicit `path`/`look_at` win;
/// otherwise the `shot_style` builds the dolly and aim from the subject
/// (`shot_offset` = ticks into the cutscene at which this shot starts, so a
/// moving subject's track lines up with the shot's own window).
///
/// Total by construction: a shape-invalid styled shot (validation reports
/// `DW0348`/`DW0349`) degenerates to a static framing of whatever resolves, so
/// downstream passes never panic.
pub fn expand_shot(
    plan: &Plan,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
    shot: &CameraShot,
    ctx: &[MoveCtx],
    shot_offset: i32,
) -> ExpandedShot {
    let ticks = shot_ticks(shot.resolved_seconds());
    let subject = shot
        .subject
        .as_ref()
        .map(|s| resolve_subject(plan, moves, actor_moves, s, ctx, shot_offset, ticks));
    // Aim: explicit `look_at` wins; else the subject (static point or moving
    // track); else travel.
    let aim = match &shot.look_at {
        Some(t) => AimTrack::Static(crate::nav::camera_look_point(plan, t)),
        None => match &subject {
            Some(SubjectGeom::Static(p)) => AimTrack::Static(*p),
            Some(SubjectGeom::Moving(track)) => AimTrack::Moving(track.clone()),
            None => AimTrack::Travel,
        },
    };
    // Geometry: explicit `path` wins; else the style's construction.
    if !shot.path.is_empty() {
        return ExpandedShot {
            ticks,
            path: CameraPath::Polyline(crate::nav::camera_points(plan, &shot.path)),
            aim: match (&shot.look_at, &shot.shot_style) {
                // An unstyled explicit path with no look_at keeps the v0.4
                // travel-aim default (byte-identical pre-style emission).
                (None, None) => AimTrack::Travel,
                _ => aim,
            },
        };
    }
    let Some(style) = shot.shot_style else {
        // Shape-invalid (no path, no style): DW0199 already reported it.
        return ExpandedShot {
            ticks,
            path: CameraPath::Polyline(vec![[0.0, crate::plan::BASE_Y as f64, 0.0]]),
            aim,
        };
    };
    let bearing = shot.bearing.unwrap_or(0.0);
    let s0 = subject_pos(&subject, 0);
    let pts = match style {
        // Static close framing: one waypoint at `dist` (default 3), half a
        // block above the subject point.
        ShotStyle::Insert => {
            let d = shot.dist.unwrap_or(3.0);
            vec![add(
                s0,
                add(scale(bearing_dir(bearing), d), [0.0, 0.5, 0.0]),
            )]
        }
        // Static wide framing from abeam the subject's track midpoint (or the
        // subject point when static), aim tracking the subject.
        ShotStyle::LockedOff => {
            let d = shot.dist.unwrap_or(12.0);
            let mid = subject_pos(&subject, ticks / 2);
            vec![add(
                mid,
                add(scale(bearing_dir(bearing), d), [0.0, 2.0, 0.0]),
            )]
        }
        // Straight dolly toward the subject: `dist` (default 12) → a third of
        // it (min 2), one block above the subject point.
        ShotStyle::PushIn => {
            let d0 = shot.dist.unwrap_or(12.0);
            let d1 = (d0 / 3.0).max(2.0);
            let dir = bearing_dir(bearing);
            vec![
                add(s0, add(scale(dir, d0), [0.0, 1.0, 0.0])),
                add(s0, add(scale(dir, d1), [0.0, 1.0, 0.0])),
            ]
        }
        // The reverse: close (`dist`, default 4) → 4× it, revealing context.
        ShotStyle::PullBackReveal => {
            let d0 = shot.dist.unwrap_or(4.0);
            let d1 = d0 * 4.0;
            let dir = bearing_dir(bearing);
            vec![
                add(s0, add(scale(dir, d0), [0.0, 1.0, 0.0])),
                add(s0, add(scale(dir, d1), [0.0, 1.0, 0.0])),
            ]
        }
        // High and far, descending and closing: `dist` (default 24, +12 up) →
        // half of it (+4 up) — the dossier's 24 → 12, Δy −8.
        ShotStyle::EstablishingCrane => {
            let d0 = shot.dist.unwrap_or(24.0);
            let d1 = d0 / 2.0;
            let dir = bearing_dir(bearing);
            vec![
                add(s0, add(scale(dir, d0), [0.0, 12.0, 0.0])),
                add(s0, add(scale(dir, d1), [0.0, 4.0, 0.0])),
            ]
        }
        // Constant-radius arc: `degrees` (default 90) starting at `bearing`,
        // radius `dist` (default 12), 2 blocks up, one waypoint per ≤ 10° so
        // arc-length parameterization gives constant angular speed.
        ShotStyle::OrbitArc => {
            let r = shot.dist.unwrap_or(12.0);
            let sweep = shot.degrees.unwrap_or(90.0);
            let n = (sweep / 10.0).ceil().max(1.0) as i32;
            (0..=n)
                .map(|k| {
                    let b = bearing + sweep * (k as f64 / n as f64);
                    add(s0, add(scale(bearing_dir(b), r), [0.0, 2.0, 0.0]))
                })
                .collect()
        }
        // Parallel dolly abeam the moving subject at a constant world offset
        // (the Rockstar phantom-vehicle rig, dossier §6): offset = `dist`
        // (default 8) to the right of the overall travel direction (`bearing`
        // rotates around it; 180 = left), one block up. Camera track mirrors
        // the subject's per-tick motion — no easing, the subject's profile
        // governs.
        ShotStyle::SideTrack => {
            let d = shot.dist.unwrap_or(8.0);
            let path = match follow_track(&subject, ticks, |travel| {
                add(
                    scale(rotate_y(right_of(travel), bearing), d),
                    [0.0, 1.0, 0.0],
                )
            }) {
                Some(track) => CameraPath::Track(track),
                None => CameraPath::Polyline(vec![add(
                    s0,
                    add(scale(bearing_dir(bearing), d), [0.0, 1.0, 0.0]),
                )]),
            };
            return ExpandedShot { ticks, path, aim };
        }
        // Low, close, trailing the moving subject: offset = `dist` (default 4)
        // directly behind the overall travel direction (`bearing` rotates
        // around it), half a block up.
        ShotStyle::LowFollow => {
            let d = shot.dist.unwrap_or(4.0);
            let path = match follow_track(&subject, ticks, |travel| {
                add(scale(rotate_y(neg(travel), bearing), d), [0.0, 0.5, 0.0])
            }) {
                Some(track) => CameraPath::Track(track),
                None => CameraPath::Polyline(vec![add(
                    s0,
                    add(scale(bearing_dir(bearing), d), [0.0, 0.5, 0.0]),
                )]),
            };
            return ExpandedShot { ticks, path, aim };
        }
        // Two subjects on opposite thirds: a Toric-space-inspired closed-form
        // placement (Lino & Christie, "Intuitive and Efficient Camera Control
        // with the Toric Space", SIGGRAPH 2015, and "Efficient Composition for
        // Virtual Camera Control", SCA 2012 — ideas only, no code ported, per
        // the dossier's ledger). With both subjects on the camera's mid-plane,
        // an angular separation α between them puts them symmetrically about
        // centre-frame; α = one third of the default ~70° field puts them on
        // the outer thirds. d = (|AB|/2) / tan(α/2) from the midpoint, on the
        // perpendicular closest to `bearing`; `dist` overrides d; clamp 5..=9
        // (the dossier's two-shot band) keeps degenerate baselines sane.
        ShotStyle::TwoShot => {
            let a = s0;
            let b = shot
                .subject_b
                .as_ref()
                .map(|s| {
                    subject_pos(
                        &Some(resolve_subject(
                            plan,
                            moves,
                            actor_moves,
                            s,
                            ctx,
                            shot_offset,
                            ticks,
                        )),
                        0,
                    )
                })
                .unwrap_or(a);
            let mid = [
                (a[0] + b[0]) / 2.0,
                (a[1] + b[1]) / 2.0,
                (a[2] + b[2]) / 2.0,
            ];
            let base = ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            let alpha = (70.0_f64 / 3.0).to_radians();
            let d = shot
                .dist
                .unwrap_or(((base / 2.0) / (alpha / 2.0).tan()).clamp(5.0, 9.0));
            // Horizontal perpendicular to AB closest to the requested bearing.
            let perp = match normalize([-(b[2] - a[2]), 0.0, b[0] - a[0]]) {
                Some(p) => {
                    let want = bearing_dir(bearing);
                    if p[0] * want[0] + p[2] * want[2] >= 0.0 {
                        p
                    } else {
                        neg(p)
                    }
                }
                None => bearing_dir(bearing),
            };
            return ExpandedShot {
                ticks,
                path: CameraPath::Polyline(vec![add(mid, add(scale(perp, d), [0.0, 1.0, 0.0]))]),
                aim: match &shot.look_at {
                    Some(_) => aim,
                    // Frame the pair: aim at the midpoint, not subject A.
                    None => AimTrack::Static(mid),
                },
            };
        }
    };
    ExpandedShot {
        ticks,
        path: CameraPath::Polyline(pts),
        aim,
    }
}

/// A subject's resolved geometry for one shot window.
enum SubjectGeom {
    Static([f64; 3]),
    /// Per-tick positions for the shot window (`len == ticks + 1`).
    Moving(Vec<[f64; 3]>),
}

/// The subject position at shot tick `t` (start position when unresolved).
fn subject_pos(subject: &Option<SubjectGeom>, t: i32) -> [f64; 3] {
    match subject {
        Some(SubjectGeom::Static(p)) => *p,
        Some(SubjectGeom::Moving(track)) => track_at(track, t),
        None => [0.0, crate::plan::BASE_Y as f64, 0.0],
    }
}

/// Resolve a shot subject: an anchor is a fixed block-centre point; an
/// npc/actor is its compiler-planned motion track when a matching sibling move
/// exists (picked by [`MoveCtx`]: the move most recently started at cutscene
/// time, else the next to start), otherwise its declared anchor. Entity
/// subjects aim one block above the feet cell (torso height) before `offset`.
fn resolve_subject(
    plan: &Plan,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
    subject: &CameraSubject,
    ctx: &[MoveCtx],
    shot_offset: i32,
    ticks: i32,
) -> SubjectGeom {
    let (is_actor, id, offset) = match subject {
        CameraSubject::Anchor(s) => {
            return SubjectGeom::Static(crate::nav::anchor_offset_point(
                plan,
                s.anchor.as_str(),
                s.offset,
            ));
        }
        CameraSubject::Npc(s) => (false, s.npc.as_str(), s.offset),
        CameraSubject::Actor(s) => (true, s.actor.as_str(), s.offset),
    };
    let lift = [offset[0] as f64, offset[1] as f64 + 1.0, offset[2] as f64];
    // The governing sibling move: smallest non-negative delta (the move most
    // recently underway at cutscene start), else the largest negative one (the
    // next to start). Declaration order breaks ties.
    let chosen = ctx
        .iter()
        .filter(|m| m.is_actor == is_actor && m.id == id)
        .min_by_key(|m| {
            if m.delta >= 0 {
                (0, m.delta)
            } else {
                (1, -m.delta)
            }
        });
    if let Some(mv) = chosen {
        let track: Option<&[[f64; 3]]> = if is_actor {
            actor_moves
                .iter()
                .find(|p| p.actor == mv.id && p.to_anchor == mv.to_anchor)
                .map(|p| p.waypoints.as_slice())
        } else {
            moves
                .iter()
                .find(|p| p.npc == mv.id && p.to_anchor == mv.to_anchor)
                .map(|p| p.waypoints.as_slice())
        };
        if let Some(track) = track
            && !track.is_empty()
        {
            // Window the full move track to this shot: shot tick t sees the
            // move at `shot_offset + delta + t`, clamped at both ends.
            let window: Vec<[f64; 3]> = (0..=ticks)
                .map(|t| add(track_at(track, shot_offset + mv.delta + t), lift))
                .collect();
            return SubjectGeom::Moving(window);
        }
    }
    // Static: the entity's declared anchor.
    let anchor = if is_actor {
        plan.campaign
            .quests
            .content
            .actors
            .iter()
            .find(|a| a.id.as_str() == id)
            .map(|a| a.anchor.as_str())
    } else {
        plan.campaign
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == id)
            .map(|n| n.anchor.as_str())
    };
    let base = crate::nav::anchor_offset_point(plan, anchor.unwrap_or(""), [0, 0, 0]);
    SubjectGeom::Static(add(base, lift))
}

/// Build a follow-style camera track: subject per-tick position + a constant
/// world offset derived from the overall travel direction. `None` when the
/// subject never moves (validation reports `DW0349`; the caller degenerates to
/// a static placement).
fn follow_track(
    subject: &Option<SubjectGeom>,
    ticks: i32,
    offset_of: impl Fn([f64; 3]) -> [f64; 3],
) -> Option<Vec<[f64; 3]>> {
    let Some(SubjectGeom::Moving(track)) = subject else {
        return None;
    };
    let first = track_at(track, 0);
    let last = track_at(track, ticks);
    let travel = normalize([last[0] - first[0], 0.0, last[2] - first[2]])?;
    let off = offset_of(travel);
    Some((0..=ticks).map(|t| add(track_at(track, t), off)).collect())
}

/// The horizontal unit vector at placement bearing `b` (degrees): where a
/// camera at that bearing sits relative to its subject. `0` → south (+Z),
/// `90` → west (−X), `-90` → east (+X), `180` → north (−Z) — the same compass
/// convention as Minecraft yaw.
fn bearing_dir(b: f64) -> [f64; 3] {
    let r = b.to_radians();
    [-r.sin(), 0.0, r.cos()]
}

/// Rotate a horizontal vector by `deg` degrees about +Y (same handedness as
/// [`bearing_dir`]).
fn rotate_y(v: [f64; 3], deg: f64) -> [f64; 3] {
    let r = deg.to_radians();
    let (s, c) = (r.sin(), r.cos());
    [v[0] * c - v[2] * s, v[1], v[0] * s + v[2] * c]
}

/// The horizontal "right of travel" direction (90° clockwise from above).
fn right_of(travel: [f64; 3]) -> [f64; 3] {
    [-travel[2], 0.0, travel[0]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(v: [f64; 3], k: f64) -> [f64; 3] {
    [v[0] * k, v[1] * k, v[2] * k]
}

fn neg(v: [f64; 3]) -> [f64; 3] {
    [-v[0], -v[1], -v[2]]
}

// ---------------------------------------------------------------------------
// Cutscene discovery with move context
// ---------------------------------------------------------------------------

/// Every `cutscene` effect in the campaign (quests + triggers, transitively
/// nested), each with the sibling-move context its styled shots resolve
/// against. Traversal order matches `emit`'s effect walk, so the first
/// occurrence of a deduplicated cutscene is the one that plans it.
///
/// Scope rules (mirrored by validation's `DW0349`): moves in the same effect
/// list run from the cutscene's own start (`delta` 0); a `sequence` aligns by
/// `at_ticks` (moves in any step, and moves in the launching list, offset
/// accordingly); reaction lists (`on_arrive`/`on_caught`/`on_respawn`) fire at
/// a statically-unknowable time and start an empty scope.
pub fn cutscene_units(
    campaign: &delvewright_dsl::Campaign,
) -> Vec<(&delvewright_dsl::QuestEffect, Vec<MoveCtx>)> {
    use delvewright_dsl::QuestEffect;
    fn list_moves(list: &[QuestEffect], delta: i32, out: &mut Vec<MoveCtx>) {
        for e in list {
            match e {
                QuestEffect::MoveNpc { npc, to_anchor, .. } => out.push(MoveCtx {
                    is_actor: false,
                    id: npc.to_string(),
                    to_anchor: to_anchor.to_string(),
                    delta,
                }),
                QuestEffect::MoveActor {
                    actor, to_anchor, ..
                } => out.push(MoveCtx {
                    is_actor: true,
                    id: actor.to_string(),
                    to_anchor: to_anchor.to_string(),
                    delta,
                }),
                _ => {}
            }
        }
    }
    fn scan<'a>(
        list: &'a [QuestEffect],
        scope: &[MoveCtx],
        out: &mut Vec<(&'a QuestEffect, Vec<MoveCtx>)>,
    ) {
        let mut local = scope.to_vec();
        list_moves(list, 0, &mut local);
        for e in list {
            match e {
                QuestEffect::Cutscene { .. } => out.push((e, local.clone())),
                QuestEffect::Sequence { steps } => {
                    // Timeline moves as (step start tick, move) pairs.
                    let mut timed: Vec<(i32, MoveCtx)> = Vec::new();
                    for st in steps {
                        let mut tmp = Vec::new();
                        list_moves(&st.effects, 0, &mut tmp);
                        for m in tmp {
                            timed.push((st.at_ticks as i32, m));
                        }
                    }
                    for st in steps {
                        let at = st.at_ticks as i32;
                        // Context seen from this step's start: outer moves
                        // started `at` ticks earlier; timeline moves started at
                        // their own step's tick.
                        let mut step_scope: Vec<MoveCtx> = local
                            .iter()
                            .map(|m| MoveCtx {
                                delta: m.delta + at,
                                ..m.clone()
                            })
                            .collect();
                        for (b, m) in &timed {
                            step_scope.push(MoveCtx {
                                delta: at - b,
                                ..m.clone()
                            });
                        }
                        for e2 in &st.effects {
                            match e2 {
                                QuestEffect::Cutscene { .. } => out.push((e2, step_scope.clone())),
                                _ => {
                                    for inner in e2.nested_effect_lists() {
                                        scan(inner, &[], out);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    for inner in e.nested_effect_lists() {
                        scan(inner, &[], out);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    let c = campaign;
    for q in &c.quests.content.quests {
        for effs in q.on_objective_complete.values() {
            scan(effs, &[], &mut out);
        }
        scan(&q.on_complete, &[], &mut out);
    }
    for t in &c.quests.content.triggers {
        scan(&t.effects, &[], &mut out);
    }
    out
}

// ---------------------------------------------------------------------------
// Geometry primitives
// ---------------------------------------------------------------------------

/// Smoothstep ease-in/ease-out (`3u² − 2u³`): Cinemachine's default blend shape
/// (camera dossier §3), baked at emission — never a runtime spring.
fn ease(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// A waypoint polyline with its cumulative arc lengths: the shared answer to
/// "where is the camera after `s` blocks of travel" — by *distance*, not by
/// segment index, so a 3-block and a 30-block segment no longer get equal time
/// (the pre-#64 `lerp_polyline` bug).
struct Path {
    pts: Vec<[f64; 3]>,
    cum: Vec<f64>,
    total: f64,
}

impl Path {
    fn new(pts: &[[f64; 3]]) -> Self {
        let pts: Vec<[f64; 3]> = if pts.is_empty() {
            vec![[0.0, crate::plan::BASE_Y as f64, 0.0]]
        } else {
            pts.to_vec()
        };
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            let d = dist(w[0], w[1]);
            cum.push(cum.last().unwrap() + d);
        }
        let total = *cum.last().unwrap();
        Path { pts, cum, total }
    }

    /// Position after `arc` blocks of travel along the polyline.
    fn pos_at_arc(&self, arc: f64) -> [f64; 3] {
        if self.total == 0.0 {
            return self.pts[0];
        }
        let arc = arc.clamp(0.0, self.total);
        let mut i = 0;
        while i + 1 < self.cum.len() - 1 && self.cum[i + 1] <= arc {
            i += 1;
        }
        let seg = self.cum[i + 1] - self.cum[i];
        let f = if seg == 0.0 {
            0.0
        } else {
            (arc - self.cum[i]) / seg
        };
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        [
            a[0] + (b[0] - a[0]) * f,
            a[1] + (b[1] - a[1]) * f,
            a[2] + (b[2] - a[2]) * f,
        ]
    }
}

/// Minecraft entity rotation from a direction vector; `(0, 0)` (the summon
/// default: south, level) for a zero direction. This is the convention the
/// `tp <targets> <pos> <rot>` command and the `Rotation` NBT use:
///
/// - `yaw = atan2(-dx, dz)`: `0` faces +Z (south), `90` faces −X (west), `180`
///   faces −Z (north), `-90` faces +X (east).
/// - `pitch = atan2(-dy, hypot(dx, dz))`: positive looks **down**, `0` is level.
///
/// Note this is *not* the render-plan / Chunky convention
/// ([`crate::render_plan`], `yaw = atan2(-dz, dx)`, `0` = +X): pitch agrees, yaw
/// does not. Rotations are rounded to 3 decimals so emission is byte-stable
/// across platforms (the ADR-0006 gate compares bytes; repeatable rounding
/// avoids libm ulp drift).
fn aim_from_dir(d: [f64; 3]) -> (f64, f64) {
    let horiz = (d[0] * d[0] + d[2] * d[2]).sqrt();
    if horiz == 0.0 && d[1] == 0.0 {
        return (0.0, 0.0);
    }
    let yaw = (-d[0]).atan2(d[2]).to_degrees();
    let pitch = (-d[1]).atan2(horiz).to_degrees();
    (round3(yaw), round3(pitch))
}

/// The unit view direction of a Minecraft `(yaw, pitch)` rotation — the inverse
/// of [`aim_from_dir`], used to compare rendered vs exact aims as vectors (a
/// yaw delta near the poles overstates the visual angle; vectors do not).
fn dir_from_aim(yaw: f64, pitch: f64) -> [f64; 3] {
    let (yr, pr) = (yaw.to_radians(), pitch.to_radians());
    let cos_p = pr.cos();
    [-yr.sin() * cos_p, -pr.sin(), yr.cos() * cos_p]
}

/// Shortest-arc angular lerp in degrees — how the client tweens yaw across a
/// teleport (wrap-aware, so 179° → −179° travels 2°, not 358°).
fn lerp_angle(a: f64, b: f64, f: f64) -> f64 {
    let mut d = (b - a) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    }
    if d < -180.0 {
        d += 360.0;
    }
    a + d * f
}

/// Angle in degrees between two unit vectors.
fn angle_deg(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
    dot.acos().to_degrees()
}

fn normalize(d: [f64; 3]) -> Option<[f64; 3]> {
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if len == 0.0 {
        return None;
    }
    Some([d[0] / len, d[1] / len, d[2] / len])
}

/// Distance from point `p` to the closed segment `a → b`.
fn dist_to_segment(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    if len2 == 0.0 {
        return dist(p, a);
    }
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let t = ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / len2).clamp(0.0, 1.0);
    dist(p, [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t])
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
}

fn round3_pos(p: [f64; 3]) -> [f64; 3] {
    [round3(p[0]), round3(p[1]), round3(p[2])]
}

/// Round to 3 decimals, collapsing `-0.0` — the emission-wide float policy
/// (byte-stable across platforms; see `emit::round3`).
fn round3(v: f64) -> f64 {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 { 0.0 } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arc-length parameterization: on a polyline with a 9:1 segment-length
    /// ratio, the camera crosses the middle waypoint at the *distance* midpoint
    /// of its journey, not the segment midpoint (the pre-#64 bug gave both
    /// segments equal time).
    #[test]
    fn arc_length_beats_segment_index() {
        let pts = [[0.0, 0.0, 0.0], [18.0, 0.0, 0.0], [20.0, 0.0, 0.0]];
        let path = Path::new(&pts);
        assert_eq!(path.total, 20.0);
        // Halfway by arc is x=10, still on the long first segment.
        assert_eq!(path.pos_at_arc(10.0), [10.0, 0.0, 0.0]);
        // 90% of the arc is exactly the middle waypoint.
        assert_eq!(path.pos_at_arc(18.0), [18.0, 0.0, 0.0]);
    }

    /// Easing: first and last keyframe intervals travel less distance than the
    /// middle — motion ramps in and out instead of starting at full speed.
    #[test]
    fn ease_in_out_shapes_the_keyframes() {
        let pts = [[0.0, 0.0, 0.0], [40.0, 0.0, 0.0]];
        let sf = plan_shot(&pts, None, 80);
        assert!(
            sf.cadence > 1,
            "a straight line tweens at the widest cadence"
        );
        let d: Vec<f64> = sf
            .frames
            .windows(2)
            .map(|w| dist(w[0].pos, w[1].pos))
            .collect();
        let mid = d[d.len() / 2];
        assert!(d[0] < mid, "ease-in: {d:?}");
        assert!(d[d.len() - 1] < mid, "ease-out: {d:?}");
    }

    /// A sharp corner forces a tighter cadence than a straight line: the chord
    /// tolerance is the mechanism that bounds cadence by curvature.
    #[test]
    fn corner_tightens_cadence() {
        let straight = plan_shot(&[[0.0, 0.0, 0.0], [20.0, 0.0, 0.0]], None, 40);
        let corner = plan_shot(
            &[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 0.0, 10.0]],
            None,
            40,
        );
        assert_eq!(straight.cadence, 10);
        assert!(
            corner.cadence < straight.cadence,
            "corner cadence {} must be tighter than straight {}",
            corner.cadence,
            straight.cadence
        );
    }

    /// A static (single-waypoint) shot is one snap frame with no tween.
    #[test]
    fn static_shot_is_snap_only() {
        let sf = plan_shot(&[[3.0, 4.0, 5.0]], Some([3.0, 4.0, 15.0]), 60);
        assert_eq!(sf.cadence, 0);
        assert_eq!(sf.frames.len(), 1);
        assert_eq!(sf.frames[0].pos, [3.0, 4.0, 5.0]);
        assert_eq!((sf.frames[0].yaw, sf.frames[0].pitch), (0.0, 0.0));
    }

    /// The budget metric: a camera orbit-passing 2 blocks from its subject in
    /// 2 s spins far beyond 6 °/tick; the same pass at 40 blocks is calm.
    #[test]
    fn aim_rate_scales_with_subject_distance() {
        let subject = AimTrack::Static([0.0, 0.0, 0.0]);
        let close = ExpandedShot {
            ticks: 40,
            path: CameraPath::Polyline(vec![[-10.0, 0.0, 2.0], [10.0, 0.0, 2.0]]),
            aim: subject.clone(),
        }
        .max_aim_deg_per_tick();
        let far = ExpandedShot {
            ticks: 40,
            path: CameraPath::Polyline(vec![[-10.0, 0.0, 40.0], [10.0, 0.0, 40.0]]),
            aim: subject,
        }
        .max_aim_deg_per_tick();
        assert!(close > MAX_AIM_DEG_PER_TICK, "close flyby spins: {close}");
        assert!(
            far < CHORD_AIM_TOLERANCE_DEG,
            "distant pass stays in the comfortable band: {far}"
        );
    }

    /// Shortest-arc yaw lerp crosses the ±180° seam the short way.
    #[test]
    fn yaw_lerp_wraps() {
        assert_eq!(lerp_angle(179.0, -179.0, 0.5), 180.0);
        assert_eq!(lerp_angle(-179.0, 179.0, 0.5), -180.0);
    }

    /// A moving-aim static camera (`locked-off` shape): the frames pan with the
    /// subject track even though the camera never translates.
    #[test]
    fn moving_aim_pans_a_static_camera() {
        let track: Vec<[f64; 3]> = (0..=40)
            .map(|t| [-10.0 + 0.5 * t as f64, 0.0, 10.0])
            .collect();
        let sf = ExpandedShot {
            ticks: 40,
            path: CameraPath::Polyline(vec![[0.0, 2.0, 0.0]]),
            aim: AimTrack::Moving(track),
        }
        .frames();
        assert!(sf.cadence > 0, "a panning shot is not a snap-only shot");
        let first = sf.frames.first().unwrap();
        let last = sf.frames.last().unwrap();
        assert_eq!(first.pos, last.pos, "locked-off camera holds position");
        assert!(
            (first.yaw - last.yaw).abs() > 30.0,
            "the aim sweeps with the subject: {} -> {}",
            first.yaw,
            last.yaw
        );
    }

    /// A follow track mirrors the subject's per-tick motion at a constant
    /// world offset (no easing).
    #[test]
    fn follow_track_mirrors_subject() {
        let track: Vec<[f64; 3]> = (0..=20).map(|t| [t as f64, 0.0, 0.0]).collect();
        let sub = Some(SubjectGeom::Moving(track));
        // Travel +X (east); right of travel is +Z (south).
        let cam = follow_track(&sub, 20, |travel| {
            add(scale(right_of(travel), 4.0), [0.0, 1.0, 0.0])
        })
        .expect("moving subject builds a track");
        assert_eq!(cam.len(), 21);
        assert_eq!(cam[0], [0.0, 1.0, 4.0]);
        assert_eq!(cam[20], [20.0, 1.0, 4.0]);
    }

    /// The bearing compass: 0 = south (+Z), 90 = west (−X).
    #[test]
    fn bearing_compass() {
        let s = bearing_dir(0.0);
        assert!((s[2] - 1.0).abs() < 1e-12 && s[0].abs() < 1e-12);
        let w = bearing_dir(90.0);
        assert!((w[0] + 1.0).abs() < 1e-12 && w[2].abs() < 1e-12);
    }
}
