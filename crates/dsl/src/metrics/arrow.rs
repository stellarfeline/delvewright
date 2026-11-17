//! **An arrow's flight** in the pinned version, read from the jar
//! (`docs/reference/vehicles-animals-and-targets.md` §14): a full draw
//! launches it at [`LAUNCH_SPEED`] blocks a tick along the shooter's look, from
//! the shooter's feet `x`, `z` and its eye less [`LAUNCH_DROP`]; each tick it
//! moves by its velocity (the straight segment from its position to position
//! plus velocity is what it strikes), then the velocity is scaled by [`DRAG`]
//! and loses [`GRAVITY`] downward.
//!
//! The one solver of the low arc through a point: the compiler's shot proof
//! follows the flight it gives (`delvec::compiler::nav::shot`), and the bot's
//! aim (`harness/src/shot.ts`) is held to it by the case file
//! `crates/dsl/tests/fixtures/movement/arrow-arcs-1.21.11.json`, which the
//! harness writes and this module replays within `1e-9`.

/// Blocks a tick a full-drawn bow launches an arrow at (`BowItem`: power
/// `1.0` × `3.0`).
pub const LAUNCH_SPEED: f64 = 3.0;

/// The scale applied to the velocity after each move, in air
/// (`AbstractArrow.tick`, `applyInertia(0.99F)`).
pub const DRAG: f64 = 0.99;

/// Blocks a tick taken from the vertical velocity after the drag
/// (`AbstractArrow.getDefaultGravity`).
pub const GRAVITY: f64 = 0.05;

/// How far under the shooter's eye the arrow spawns (`AbstractArrow`'s
/// shooter constructor, `getEyeY() - 0.1F`: the float widened).
pub const LAUNCH_DROP: f64 = 0.100_000_001_490_116_12;

/// The half-width of the triangular draw each component of the look's unit
/// vector is moved by before it is scaled to the speed
/// (`Projectile.getMovementToShoot`, inaccuracy `1.0`).
pub const SPREAD: f64 = 0.017_227_5;

/// The ticks a flight is followed before it is given up as out of reach.
pub const FLIGHT_TICKS: usize = 200;

const PITCH_LO: f64 = -std::f64::consts::FRAC_PI_2 + 1e-3;
const PITCH_HI: f64 = std::f64::consts::FRAC_PI_2 - 1e-3;

/// The height over its launch point at which an arrow loosed at `pitch`
/// radians above level crosses `distance` blocks of horizontal travel; `None`
/// when the flight window ends first.
#[must_use]
pub fn height_at(pitch: f64, distance: f64) -> Option<f64> {
    let mut vx = LAUNCH_SPEED * pitch.cos();
    let mut vy = LAUNCH_SPEED * pitch.sin();
    let (mut x, mut y) = (0.0, 0.0);
    for _ in 0..FLIGHT_TICKS {
        if x + vx >= distance {
            let f = if vx == 0.0 { 0.0 } else { (distance - x) / vx };
            return Some(y + f * vy);
        }
        x += vx;
        y += vy;
        vx *= DRAG;
        vy = vy * DRAG - GRAVITY;
    }
    None
}

fn height_or(pitch: f64, distance: f64) -> f64 {
    height_at(pitch, distance).unwrap_or(f64::NEG_INFINITY)
}

/// The pitch (radians above level) of the low arc that carries a full-drawn
/// arrow `distance` blocks out and `rise` blocks up from its launch point;
/// `None` when no arc reaches that point.
#[must_use]
pub fn low_arc_pitch(distance: f64, rise: f64) -> Option<f64> {
    // The crossing height climbs with the pitch up to the arc of farthest
    // reach, then falls: find that peak, then bisect the climbing side.
    let (mut lo, mut hi) = (PITCH_LO, PITCH_HI);
    for _ in 0..100 {
        let a = lo + (hi - lo) / 3.0;
        let b = hi - (hi - lo) / 3.0;
        if height_or(a, distance) < height_or(b, distance) {
            lo = a;
        } else {
            hi = b;
        }
    }
    let peak = (lo + hi) / 2.0;
    if height_or(peak, distance) < rise {
        return None;
    }
    lo = PITCH_LO;
    hi = peak;
    if height_or(lo, distance) >= rise {
        return Some(lo);
    }
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if height_or(mid, distance) < rise {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo + hi) / 2.0)
}

/// **The flight** of a full-drawn arrow loosed from `eye` on the low arc
/// through `point`, with no spread: its position at launch and after each
/// tick, up to the first tick at or past `point`'s column. `None` when no arc
/// reaches `point`, or `point` stands straight over or under the eye.
#[must_use]
pub fn flight(eye: [f64; 3], point: [f64; 3]) -> Option<Vec<[f64; 3]>> {
    let launch = [eye[0], eye[1] - LAUNCH_DROP, eye[2]];
    let (dx, dz) = (point[0] - launch[0], point[2] - launch[2]);
    let d = dx.hypot(dz);
    if d == 0.0 {
        return None;
    }
    let pitch = low_arc_pitch(d, point[1] - launch[1])?;
    let (ux, uz) = (dx / d, dz / d);
    let mut vh = LAUNCH_SPEED * pitch.cos();
    let mut vy = LAUNCH_SPEED * pitch.sin();
    let mut p = launch;
    let mut out = vec![p];
    let mut run = 0.0;
    for _ in 0..FLIGHT_TICKS {
        p = [p[0] + vh * ux, p[1] + vy, p[2] + vh * uz];
        run += vh;
        out.push(p);
        if run >= d {
            return Some(out);
        }
        vh *= DRAG;
        vy = vy * DRAG - GRAVITY;
    }
    None
}

#[cfg(test)]
mod tests;
