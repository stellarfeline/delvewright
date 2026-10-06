//! Deterministic generator for the content library's assembly rigs
//! (spec-0082 §3.1).
//!
//! A rig is the parts and clips of an assembly — thousands of keyframe numbers,
//! a procedural derivation and never campaign JSON. This program writes them
//! into a prefab library the way a tileset generator writes `.nbt`:
//!
//! ```text
//! cargo run --release --manifest-path prefabs/rig-generator/Cargo.toml -- <library>
//! ```
//!
//! writes `<library>/rigs/<name>/rig.json` for every rig below. `<library>` is
//! the content repository's `prefabs/`; the output is committed there, and this
//! program, like every generator, stays here.
//!
//! ## `rig/tentacle`
//!
//! The research spike's 34-segment tentacle (`tools/spike-display-assembly/gen.py`,
//! itself re-cut from `research/eldritch-visuals`): 34 `block_display`
//! segments, sculk banded with crying obsidian, thick from the base to a club
//! of 1.6 blocks and then tapering to a 0.32 tip over its last five, and five
//! clips — `rise` (36 frames at 5 ticks, out of a pit), `idle` (60 at 5, a
//! looping sway), `windup` (4 at 3, a lean back of its upper body: 10 ticks),
//! `strike` (8 at 2, a slam: 17 ticks until the last frame is drawn) and
//! `retract` (`rise` reversed, back into the pit).
//!
//! The slam's last frame is built joint by joint: the base stands straight up
//! out of the pit for five segments, arches forward over head height, comes
//! down, and lies along the floor for nine segments — the club — before its
//! tip curls up off the floor. What a body standing in front of the pit is
//! struck by is that club: a long area along where the limb lands
//! (spec-0082 §5.4). The base's five segments hold the same pose through the
//! whole wind-up and strike, so where the limb already stood is never read as
//! where it came down.
//!
//! The spike's tentacle leans south and strikes north. A rig's front is `+z`
//! (spec-0082 §3.1), so every frame is turned 180° about the mark here, once,
//! by the engine's own `Transform::faced` — the blow lands in front, the
//! wind-up leans back.
//!
//! ADR-0006: no clock, no RNG; every number is rounded to six decimals before
//! it is written, so the file is the same on every platform's `libm`.

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::path::Path;

use delvewright_dsl::rig::{self, Clip, PartKind, Rig, RigPart, RigProvenance, Transform};
use delvewright_dsl::Facing;

/// Segments.
const N_SEG: usize = 34;
/// Each segment's length along the limb.
const SEG_L: f64 = 0.8;
/// The base segment's width, the club's (the last thick segment's), and the
/// tip's; the club ends at [`CLUB_END`], and the taper to the tip takes the rest.
const W_BASE: f64 = 2.4;
const W_CLUB: f64 = 1.0;
const W_TIP: f64 = 0.32;
const CLUB_END: usize = 28;
/// How deep the base sits when the limb is fully hidden, and when it is up.
const D_HIDDEN: f64 = N_SEG as f64 * SEG_L + 1.5;
const D_UP: f64 = 0.6;
/// Frame counts.
const RISE: usize = 36;
const IDLE: usize = 60;
const WINDUP: usize = 4;
const STRIKE: usize = 8;
/// The wind-up's lean, in radians spread over the joints above the base.
const WINDUP_BEND: f64 = 0.9;
/// The segments that stand straight up out of the pit through the wind-up and
/// the strike.
const BASE_SEGS: usize = 5;
/// The keyframe cadences, ticks per frame: the slow clips, the wind-up, the
/// strike.
const CADENCE: u32 = 5;
const WINDUP_CADENCE: u32 = 3;
const STRIKE_CADENCE: u32 = 2;
/// The body block and the band every fourth segment wears.
const BODY: &str = "minecraft:sculk";
const BAND: &str = "minecraft:crying_obsidian";

type Quat = [f64; 4];

fn qmul(a: Quat, b: Quat) -> Quat {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn qaxis(axis: [f64; 3], ang: f64) -> Quat {
    let s = (ang / 2.0).sin();
    [axis[0] * s, axis[1] * s, axis[2] * s, (ang / 2.0).cos()]
}

fn qrot(q: Quat, v: [f64; 3]) -> [f64; 3] {
    let r = qmul(
        qmul(q, [v[0], v[1], v[2], 0.0]),
        [-q[0], -q[1], -q[2], q[3]],
    );
    [r[0], r[1], r[2]]
}

/// Which joints a bend is spread over: `Low` toward the base (a lean),
/// `Uniform` evenly (an arc).
#[derive(Clone)]
enum Profile {
    Low,
    /// Each joint's own bend, in radians (the slam and the wind-up's lean).
    Joints(Vec<f64>),
}

/// One keyframe: a transform per segment, in the spike's frame (`gen.py`'s
/// `pose`, number for number).
fn pose(depth: f64, amp: f64, tau: f64, bend: f64, profile: Profile) -> Vec<Transform> {
    let mut q = qaxis([1.0, 0.0, 0.0], -0.12 * amp * amp);
    let mut j = [0.0, -depth, 0.0];
    let us: Vec<f64> = (0..N_SEG).map(|i| i as f64 / (N_SEG - 1) as f64).collect();
    let w: Vec<f64> = match &profile {
        Profile::Low => {
            let raw: Vec<f64> = us.iter().map(|u| 1.0 - u).collect();
            let tot: f64 = raw.iter().sum();
            raw.iter().map(|x| bend * x / tot).collect()
        }
        Profile::Joints(j) => j.clone(),
    };
    let mut out = Vec::with_capacity(N_SEG);
    for i in 0..N_SEG {
        let u = us[i];
        let wd = width(i);
        let twist = 0.35 * i as f64 + 0.4 * tau.sin();
        let l = qmul(q, qaxis([0.0, 1.0, 0.0], twist));
        let off = qrot(l, [wd / 2.0, -0.06 * SEG_L, wd / 2.0]);
        let t = [j[0] - off[0], j[1] - off[1], j[2] - off[2]];
        let nxt = qrot(q, [0.0, SEG_L, 0.0]);
        out.push(Transform {
            translation: t,
            left_rotation: l,
            scale: [wd, SEG_L * 1.12, wd],
            right_rotation: [0.0, 0.0, 0.0, 1.0],
        });
        j = [j[0] + nxt[0], j[1] + nxt[1], j[2] + nxt[2]];
        let curl = -0.11 * (0.5 + 0.5 * (tau - 0.8).sin()) * u * u;
        let ax = amp * (0.075 * (tau - 2.2 * PI * u).sin() * u + curl) + w[i];
        let az = amp * (0.06 * (tau + 1.3 - 1.8 * PI * u).sin() * u);
        q = qmul(
            q,
            qmul(qaxis([1.0, 0.0, 0.0], ax), qaxis([0.0, 0.0, 1.0], az)),
        );
    }
    out
}

/// Segment `i`'s width: from the base to the club, then down to the tip.
fn width(i: usize) -> f64 {
    if i <= CLUB_END {
        W_BASE + (W_CLUB - W_BASE) * i as f64 / CLUB_END as f64
    } else {
        W_CLUB + (W_TIP - W_CLUB) * (i - CLUB_END) as f64 / (N_SEG - 1 - CLUB_END) as f64
    }
}

/// The wind-up's last lean: `bend` spread evenly over the joints above the
/// base, positive (back, away from the blow).
fn lean(bend: f64) -> Vec<f64> {
    (0..N_SEG)
        .map(|i| {
            if i < BASE_SEGS {
                0.0
            } else {
                bend / (N_SEG - BASE_SEGS) as f64
            }
        })
        .collect()
}

/// The slam's joints, negative bending toward the blow: up out of the pit,
/// over head height, down, along the floor, and the tip curled up.
fn slam() -> Vec<f64> {
    let q = PI / 2.0;
    let mut j: Vec<f64> = Vec::with_capacity(N_SEG);
    let mut run = |n: usize, each: f64| j.extend(std::iter::repeat_n(each, n));
    run(BASE_SEGS - 1, 0.0); // straight up; the last base joint starts the arch
    run(3, -q / 3.0); // over, to horizontal
    run(8, 0.0); // across, over head height
    run(3, -q / 3.0); // down
    run(1, 0.0);
    run(3, q / 3.0); // along the floor
    run(4, 0.0); // the club, on the floor
    run(2, q / 2.5); // the tip curls up and back
    run(N_SEG, 0.0);
    j.truncate(N_SEG);
    j
}

/// Round to six decimals, `-0` folded to `0`.
fn r6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// A spike frame turned to the rig's front and rounded.
fn fronted(frame: Vec<Transform>) -> Vec<Transform> {
    frame
        .into_iter()
        .map(|t| {
            let f = t.faced(Facing::North);
            Transform {
                translation: f.translation.map(r6),
                left_rotation: f.left_rotation.map(r6),
                scale: f.scale.map(r6),
                right_rotation: f.right_rotation.map(r6),
            }
        })
        .collect()
}

fn clip(ticks_per_frame: u32, looping: bool, frames: Vec<Vec<Transform>>) -> Clip {
    Clip {
        ticks_per_frame,
        looping,
        frames: frames.into_iter().map(fronted).collect(),
    }
}

/// `rig/tentacle`.
fn tentacle() -> Rig {
    let rise: Vec<Vec<Transform>> = (0..RISE)
        .map(|f| {
            let p = f as f64 / (RISE - 1) as f64;
            let e = 1.0 - (1.0 - p).powi(3);
            let depth = D_HIDDEN + (D_UP - D_HIDDEN) * e;
            pose(depth, 0.2 + 0.8 * p, 0.0, 0.0, Profile::Low)
        })
        .collect();
    let idle: Vec<Vec<Transform>> = (0..IDLE)
        .map(|f| {
            pose(
                D_UP,
                1.0,
                2.0 * PI * f as f64 / IDLE as f64,
                0.0,
                Profile::Low,
            )
        })
        .collect();
    let windup: Vec<Vec<Transform>> = (0..WINDUP)
        .map(|f| {
            let p = (f + 1) as f64 / WINDUP as f64;
            pose(D_UP, 0.0, 0.0, 0.0, Profile::Joints(lean(WINDUP_BEND * p)))
        })
        .collect();
    let back = lean(WINDUP_BEND);
    let down = slam();
    let strike: Vec<Vec<Transform>> = (0..STRIKE)
        .map(|f| {
            let p = (f + 1) as f64 / STRIKE as f64;
            let e = p * p;
            let joints = back
                .iter()
                .zip(&down)
                .map(|(a, b)| a + (b - a) * e)
                .collect();
            pose(D_UP, 0.0, 0.0, 0.0, Profile::Joints(joints))
        })
        .collect();
    let mut retract = rise.clone();
    retract.reverse();
    let mut clips = BTreeMap::new();
    clips.insert("rise".to_string(), clip(CADENCE, false, rise));
    clips.insert("idle".to_string(), clip(CADENCE, true, idle));
    clips.insert("windup".to_string(), clip(WINDUP_CADENCE, false, windup));
    clips.insert("strike".to_string(), clip(STRIKE_CADENCE, false, strike));
    clips.insert("retract".to_string(), clip(CADENCE, false, retract));
    Rig {
        rig_version: rig::RIG_VERSION,
        parts: (0..N_SEG)
            .map(|i| RigPart {
                id: format!("seg-{i}"),
                kind: PartKind::Block,
                block: if i % 4 == 2 { BAND } else { BODY }.to_string(),
                rest: None,
            })
            .collect(),
        clips,
        provenance: RigProvenance {
            generator: "prefabs/rig-generator (after tools/spike-display-assembly/gen.py)"
                .to_string(),
            source: "original".to_string(),
            spdx: "GPL-3.0-or-later".to_string(),
        },
    }
}

fn write(out: &Path, name: &str, r: &Rig) {
    let issues = rig::check(r);
    assert!(
        issues.is_empty(),
        "rig `{name}` breaks a rig rule the engine refuses with DW0935: {issues:?}"
    );
    let dir = out.join(rig::RIGS_DIR).join(name);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    let path = dir.join(rig::RIG_FILE);
    let mut text = serde_json::to_string_pretty(r).expect("a rig serializes");
    text.push('\n');
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    print!(
        "{}",
        rig::describe(&format!("rig/{name}"), r, Facing::South)
    );
    println!("wrote {}", path.display());
}

fn main() {
    let Some(out) = std::env::args().nth(1) else {
        eprintln!("usage: rig-gen <library>  (the content repository's prefabs/ directory)");
        std::process::exit(2);
    };
    let out = Path::new(&out);
    if !out.is_dir() {
        eprintln!(
            "rig-gen: {} is not an existing directory. Point this at the prefab library \
             the rigs belong beside; a path this program created is a path nothing reads.",
            out.display()
        );
        std::process::exit(2);
    }
    write(out, "tentacle", &tentacle());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two generations are byte-identical (ADR-0006).
    #[test]
    fn the_rig_is_deterministic() {
        let a = serde_json::to_string(&tentacle()).unwrap();
        let b = serde_json::to_string(&tentacle()).unwrap();
        assert_eq!(a, b);
    }

    /// The blow lands in front, along the floor: the strike's last frame
    /// meets the floor course (y 0 and 1, a standing body's space) along a run
    /// of cells in front of the mark, and none behind it.
    #[test]
    fn the_blow_lands_in_front_along_the_floor() {
        let r = tentacle();
        let last = rig::last_frame_footprint(&r.clips["strike"], Facing::South);
        let floor: Vec<i32> = last
            .iter()
            .filter(|c| c[0] == 0 && (0..=1).contains(&c[1]) && c[2] > 2)
            .map(|c| c[2])
            .collect();
        assert!(floor.len() >= 7, "{last:?}");
        assert!(last.iter().all(|c| c[2] >= -3), "{last:?}");
    }

    /// The base holds its pose through the wind-up's end and the whole strike.
    #[test]
    fn the_base_stands_still_through_the_blow() {
        let r = tentacle();
        let first = &r.clips["strike"].frames[0][..BASE_SEGS];
        for f in &r.clips["strike"].frames {
            assert_eq!(&f[..BASE_SEGS], first);
        }
        assert_eq!(
            &r.clips["windup"].frames.last().unwrap()[..BASE_SEGS],
            first
        );
    }
}
