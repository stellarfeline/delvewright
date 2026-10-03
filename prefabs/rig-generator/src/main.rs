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
//! itself re-cut from `research/eldritch-visuals`), ported number for number:
//! 34 `block_display` segments tapering from 2.4 to 0.32 blocks, sculk banded
//! with crying obsidian, and five clips at a 5-tick cadence — `rise` (36
//! frames, out of a pit), `idle` (60, a looping sway), `windup` (8, a lean
//! back), `strike` (10, an arc over and down onto the floor) and `retract`
//! (`rise` reversed, back into the pit).
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
/// The base segment's width, and the tip's.
const W_BASE: f64 = 2.4;
const W_TIP: f64 = 0.32;
/// How deep the base sits when the limb is fully hidden, and when it is up.
const D_HIDDEN: f64 = N_SEG as f64 * SEG_L + 1.5;
const D_UP: f64 = 0.6;
/// Frame counts.
const RISE: usize = 36;
const IDLE: usize = 60;
const WINDUP: usize = 8;
const STRIKE: usize = 10;
/// The wind-up's lean, and the strike's arc, in radians spread over the joints.
const WINDUP_BEND: f64 = 0.9;
const STRIKE_BEND: f64 = -3.1;
/// The keyframe cadence, ticks per frame.
const CADENCE: u32 = 5;
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
#[derive(Clone, Copy)]
enum Profile {
    Low,
    Uniform,
}

/// One keyframe: a transform per segment, in the spike's frame (`gen.py`'s
/// `pose`, number for number).
fn pose(depth: f64, amp: f64, tau: f64, bend: f64, profile: Profile) -> Vec<Transform> {
    let mut q = qaxis([1.0, 0.0, 0.0], -0.12 * amp * amp);
    let mut j = [0.0, -depth, 0.0];
    let us: Vec<f64> = (0..N_SEG).map(|i| i as f64 / (N_SEG - 1) as f64).collect();
    let raw: Vec<f64> = us
        .iter()
        .map(|u| match profile {
            Profile::Low => 1.0 - u,
            Profile::Uniform => 1.0,
        })
        .collect();
    let tot: f64 = raw.iter().sum();
    let w: Vec<f64> = raw.iter().map(|x| x / tot).collect();
    let mut out = Vec::with_capacity(N_SEG);
    for i in 0..N_SEG {
        let u = us[i];
        let wd = W_BASE + (W_TIP - W_BASE) * u;
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
        let ax = amp * (0.075 * (tau - 2.2 * PI * u).sin() * u + curl) + bend * w[i];
        let az = amp * (0.06 * (tau + 1.3 - 1.8 * PI * u).sin() * u);
        q = qmul(
            q,
            qmul(qaxis([1.0, 0.0, 0.0], ax), qaxis([0.0, 0.0, 1.0], az)),
        );
    }
    out
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

fn clip(looping: bool, frames: Vec<Vec<Transform>>) -> Clip {
    Clip {
        ticks_per_frame: CADENCE,
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
            pose(
                D_UP,
                0.3,
                0.0,
                WINDUP_BEND * (f + 1) as f64 / WINDUP as f64,
                Profile::Low,
            )
        })
        .collect();
    let strike: Vec<Vec<Transform>> = (0..STRIKE)
        .map(|f| {
            let p = (f + 1) as f64 / STRIKE as f64;
            let e = p * p;
            let b = WINDUP_BEND + (STRIKE_BEND - WINDUP_BEND) * e;
            pose(D_UP, 0.3, 0.0, b, Profile::Uniform)
        })
        .collect();
    let mut retract = rise.clone();
    retract.reverse();
    let mut clips = BTreeMap::new();
    clips.insert("rise".to_string(), clip(false, rise));
    clips.insert("idle".to_string(), clip(true, idle));
    clips.insert("windup".to_string(), clip(false, windup));
    clips.insert("strike".to_string(), clip(false, strike));
    clips.insert("retract".to_string(), clip(false, retract));
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
            generator: "prefabs/rig-generator (port of tools/spike-display-assembly/gen.py)"
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

    /// The blow lands in front: the strike's last frame reaches cells at `+z`
    /// beyond the mark, and none far behind it.
    #[test]
    fn the_blow_lands_in_front() {
        let r = tentacle();
        let last = rig::last_frame_footprint(&r.clips["strike"], Facing::South);
        assert!(last.iter().any(|c| c[2] >= 10), "{last:?}");
        assert!(last.iter().all(|c| c[2] >= -4), "{last:?}");
    }
}
