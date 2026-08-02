//! Cutscene camera geometry (task #64): the arc-length-parameterized, eased
//! dolly with display-entity `teleport_duration` keyframe cadence. Shared
//! between emission ([`crate::emit`]) and validation ([`crate::nav`]) so the
//! checks prove exactly the camera path the client renders.
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
//! ## Determinism
//!
//! Everything here is a pure function of the resolved shot geometry: no RNG, no
//! wall clock, no iteration over unordered maps. Emitted poses are rounded to 3
//! decimals ([`round3`]) and threshold comparisons round first, so a borderline
//! shot cannot flip a diagnostic across platforms on a libm ulp (ADR-0006).

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
/// a pan is nausea-tier at 20 Hz and the shot fails `DW0346`. The camera
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

/// Plan a shot's keyframes: arc-length parameterize the waypoint polyline,
/// apply ease-in/ease-out, pick the widest chord-safe cadence, and bake the
/// per-keyframe aim (at `subject`, else along the direction of travel).
///
/// A single-waypoint / zero-length path — and the degenerate 1-tick shot,
/// which has no room to move — is a static shot: one snap frame, cadence 0.
pub fn plan_shot(pts: &[[f64; 3]], subject: Option<[f64; 3]>, ticks: i32) -> ShotFrames {
    let path = Path::new(pts);
    let snap_pos = path.pos_at_arc(0.0);
    if path.total == 0.0 || ticks <= 1 {
        let (yaw, pitch) = path.aim(snap_pos, 0.0, subject);
        return ShotFrames {
            cadence: 0,
            frames: vec![Frame {
                tick: 0,
                pos: round3_pos(snap_pos),
                yaw,
                pitch,
            }],
        };
    }
    let cadence = CADENCES
        .iter()
        .copied()
        .find(|&n| n < ticks && chords_within_tolerance(&path, subject, ticks, n))
        .unwrap_or(1);
    let mut frames = Vec::new();
    for (tick, tau) in keyframe_grid(ticks, cadence) {
        let arc = ease(tau) * path.total;
        let pos = path.pos_at_arc(arc);
        let (yaw, pitch) = path.aim(pos, arc, subject);
        frames.push(Frame {
            tick,
            pos: round3_pos(pos),
            yaw,
            pitch,
        });
    }
    ShotFrames { cadence, frames }
}

/// The peak angular rate (degrees per tick, 3-decimal rounded) of the exact
/// eased aim across the shot — the `DW0346` budget metric. The client-rendered
/// rate is a per-interval average of this, so the exact maximum bounds what a
/// player can ever see, independent of the cadence chosen.
pub fn max_aim_deg_per_tick(pts: &[[f64; 3]], subject: Option<[f64; 3]>, ticks: i32) -> f64 {
    let path = Path::new(pts);
    if ticks <= 0 {
        return 0.0;
    }
    let mut max = 0.0_f64;
    let mut prev = exact_aim_dir(&path, subject, 0, ticks);
    for t in 1..=ticks {
        let cur = exact_aim_dir(&path, subject, t, ticks);
        if let (Some(a), Some(b)) = (prev, cur) {
            max = max.max(angle_deg(a, b));
        }
        prev = cur;
    }
    round3(max)
}

/// The keyframe issue schedule for a moving shot: the tick-0 snap (τ = 0), then
/// a `tp` every `cadence` ticks starting at tick 1, each targeting the pose the
/// camera should hold when the tween lands `cadence` ticks later (τ clamped to
/// 1 — ease-out makes the clipped tail sub-perceptual, and the next shot's snap
/// or the restore overrides it).
fn keyframe_grid(ticks: i32, cadence: i32) -> Vec<(i32, f64)> {
    let mut out = vec![(0, 0.0)];
    let mut s = 1;
    while s < ticks {
        let tau = ((s + cadence) as f64 / ticks as f64).min(1.0);
        out.push((s, tau));
        s += cadence;
    }
    out
}

/// True if, at cadence `n`, every client-rendered tick pose (linear position
/// lerp + shortest-arc yaw/pitch lerp between keyframes — measurement 3) stays
/// within [`CHORD_POS_TOLERANCE`] / [`CHORD_AIM_TOLERANCE_DEG`] of the exact
/// eased path. Comparisons round first so the choice is platform-stable.
fn chords_within_tolerance(path: &Path, subject: Option<[f64; 3]>, ticks: i32, n: i32) -> bool {
    let grid = keyframe_grid(ticks, n);
    // Client arrival nodes: the snap lands at tick 0; a keyframe issued at s
    // lands at s + n.
    let nodes: Vec<(i32, f64)> = grid
        .iter()
        .map(|&(s, tau)| if s == 0 { (0, 0.0) } else { (s + n, tau) })
        .collect();
    for w in nodes.windows(2) {
        let ((t0, tau0), (t1, tau1)) = (w[0], w[1]);
        let a_pos = path.pos_at_arc(ease(tau0) * path.total);
        let b_pos = path.pos_at_arc(ease(tau1) * path.total);
        let a_aim = path.aim(a_pos, ease(tau0) * path.total, subject);
        let b_aim = path.aim(b_pos, ease(tau1) * path.total, subject);
        for t in (t0 + 1)..t1.min(ticks) {
            let f = (t - t0) as f64 / (t1 - t0) as f64;
            // Corridor bound: the exact path point at tick t must lie within
            // tolerance of the rendered chord *segment* (perpendicular /
            // corner-cut distance — along-path lag is allowed, see
            // `CHORD_POS_TOLERANCE`).
            let tau = t as f64 / ticks as f64;
            let exact = path.pos_at_arc(ease(tau) * path.total);
            if round3(dist_to_segment(exact, a_pos, b_pos)) > CHORD_POS_TOLERANCE {
                return false;
            }
            let rend_yaw = lerp_angle(a_aim.0, b_aim.0, f);
            let rend_pitch = a_aim.1 + (b_aim.1 - a_aim.1) * f;
            if let Some(exact_dir) = exact_aim_dir(path, subject, t, ticks) {
                let rend_dir = dir_from_aim(rend_yaw, rend_pitch);
                if round3(angle_deg(rend_dir, exact_dir)) > CHORD_AIM_TOLERANCE_DEG {
                    return false;
                }
            }
        }
    }
    true
}

/// The exact eased aim at tick `t` as a unit direction vector, or `None` for a
/// degenerate (no-direction) pose.
fn exact_aim_dir(path: &Path, subject: Option<[f64; 3]>, t: i32, ticks: i32) -> Option<[f64; 3]> {
    let tau = (t as f64 / ticks as f64).clamp(0.0, 1.0);
    let arc = ease(tau) * path.total;
    let pos = path.pos_at_arc(arc);
    let d = match subject {
        Some(s) => [s[0] - pos[0], s[1] - pos[1], s[2] - pos[2]],
        None => path.tangent_at_arc(arc)?,
    };
    normalize(d)
}

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

    /// Unit tangent of the segment containing `arc`; a zero-length segment (or
    /// path) falls back to the overall first → last direction, `None` if that
    /// is zero too.
    fn tangent_at_arc(&self, arc: f64) -> Option<[f64; 3]> {
        if self.total == 0.0 {
            return None;
        }
        let arc = arc.clamp(0.0, self.total);
        let mut i = 0;
        while i + 1 < self.cum.len() - 1 && self.cum[i + 1] <= arc {
            i += 1;
        }
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        if d == [0.0, 0.0, 0.0] {
            let (f, l) = (self.pts[0], self.pts[self.pts.len() - 1]);
            return normalize([l[0] - f[0], l[1] - f[1], l[2] - f[2]]);
        }
        normalize(d)
    }

    /// The emitted aim at a pose: at `subject` if given, else along the
    /// direction of travel. Minecraft entity rotation degrees, 3-decimal
    /// rounded (`yaw = atan2(-dx, dz)`, 0 = south; `pitch` positive = down).
    fn aim(&self, pos: [f64; 3], arc: f64, subject: Option<[f64; 3]>) -> (f64, f64) {
        let d = match subject {
            Some(s) => [s[0] - pos[0], s[1] - pos[1], s[2] - pos[2]],
            None => match self.tangent_at_arc(arc) {
                Some(t) => t,
                None => return (0.0, 0.0),
            },
        };
        aim_from_dir(d)
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
        let subject = Some([0.0, 0.0, 0.0]);
        let close = max_aim_deg_per_tick(&[[-10.0, 0.0, 2.0], [10.0, 0.0, 2.0]], subject, 40);
        let far = max_aim_deg_per_tick(&[[-10.0, 0.0, 40.0], [10.0, 0.0, 40.0]], subject, 40);
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
}
