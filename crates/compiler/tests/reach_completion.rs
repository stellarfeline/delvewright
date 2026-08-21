//! **One authority on where a `reach-anchor` completes**, asserted on a built
//! campaign rather than remembered.
//!
//! `radius` is authored once and had two readers that stopped agreeing. The M2
//! repair for a completion sphere too tight to stand in (`hv-01`) replaced the
//! sphere with a fixed ±1 cube at DSL v0.3 — and *replaced* is the defect: the
//! authored number stopped reaching the datapack entirely, while the harness went
//! on deriving its walk goal from it and aiming `radius - 1` blocks out. For any
//! `radius` of 3 or more that goal is outside the box the server tests, so the
//! bot was entitled to stop short and then hang on a completion that could not
//! fire. It stayed green because a `GoalNear` usually overshoots inward, which
//! made the failure intermittent — and an intermittent failure is an
//! under-specified test, never a flake.
//!
//! Two things are asserted here, and the second is the one that keeps the first
//! from rotting:
//!
//! 1. the volume **honours the authored radius** at v0.3+ (half-extent
//!    `max(1, radius)` — a floor over the ±1 that closed `hv-01`, not a constant
//!    instead of it), and stays the pre-v0.3 sphere for a v0.2 campaign;
//! 2. the region the tick line adjudicates with and the region
//!    `critical-path.json` hands the bot are **the same string**, for every reach
//!    objective of every campaign built here. That is the agreement itself, and
//!    it is checked against emitted bytes — not against the fact that both
//!    currently call one function, which is a property of today's source.
//!
//! Process-level (the real `delvec` binary), like `tests/cli.rs`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// `plan::safe_local`'s shape, as the emitted function name carries it:
/// `obj/reach-the-counter` → `o_reach_the_counter`.
fn obj_fn(obj_id: &str) -> String {
    let local = obj_id.rsplit('/').next().unwrap_or(obj_id);
    format!(
        "o_{}",
        local
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
    )
}

/// Build a fixture and return `(critical-path.json, every tick line)`.
fn build(fixture: &str, out_name: &str) -> (serde_json::Value, Vec<String>) {
    let dir = common::compiler_fixtures_dir().join(fixture);
    let out = tmp(out_name);
    let r = Command::new(BIN)
        .args([
            "build",
            dir.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            &common::prefabs_dir().display().to_string(),
        ])
        .current_dir(common::repo_root())
        .output()
        .expect("run delvec");
    assert!(
        r.status.success(),
        "{fixture} builds:\n{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    let path: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("critical-path.json")).expect("path json"))
            .expect("parse path");
    let mut ticks = Vec::new();
    let mut stack = vec![out.join("datapack")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("read dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == "tick.mcfunction") {
                ticks.extend(
                    std::fs::read_to_string(&p)
                        .expect("read tick")
                        .lines()
                        .map(str::to_string),
                );
            }
        }
    }
    assert!(!ticks.is_empty(), "{fixture} emitted a tick function");
    (path, ticks)
}

/// The exported reach steps of a built campaign, as `(objective, step)`.
fn reach_steps(path: &serde_json::Value) -> Vec<serde_json::Value> {
    path["steps"]
        .as_array()
        .expect("steps array")
        .iter()
        .filter(|s| s["action"] == "reach")
        .cloned()
        .collect()
}

/// The `@s[...]` selector arguments a completion volume denotes — the string the
/// tick line must carry. Written out here from the exported JSON rather than
/// borrowed from the compiler, so this test is a second reader of the artifact
/// and not the emitter agreeing with itself.
fn expected_selector(completion: &serde_json::Value) -> String {
    let v = |k: &str, i: usize| completion[k][i].as_i64().expect("coord");
    match completion["kind"].as_str().expect("kind") {
        "cube" => format!(
            "x={},dx={},y={},dy={},z={},dz={}",
            v("lo", 0),
            v("hi", 0) - v("lo", 0),
            v("lo", 1),
            v("hi", 1) - v("lo", 1),
            v("lo", 2),
            v("hi", 2) - v("lo", 2)
        ),
        "sphere" => format!(
            "x={},y={},z={},distance=..{}",
            v("pos", 0),
            v("pos", 1),
            v("pos", 2),
            completion["radius"].as_u64().expect("radius")
        ),
        other => panic!("unknown completion kind {other}"),
    }
}

/// The agreement, on emitted bytes: for every reach objective, the tick line that
/// completes it adjudicates in exactly the region `critical-path.json` exports.
///
/// The failure this refuses is silent by construction — the datapack keeps
/// compiling, the artifact keeps parsing, and the only symptom is a bot that
/// stops somewhere the objective cannot see it.
fn assert_agreement(fixture: &str, path: &serde_json::Value, ticks: &[String]) -> usize {
    let steps = reach_steps(path);
    for step in &steps {
        let obj = step["objective"].as_str().expect("objective");
        let want = expected_selector(&step["completion"]);
        let needle = format!("complete_{}", obj_fn(obj));
        let lines: Vec<&String> = ticks
            .iter()
            .filter(|l| l.ends_with(&needle) && l.contains("if entity @s["))
            .collect();
        assert_eq!(
            lines.len(),
            1,
            "{fixture}: {obj} should have exactly one adjudicating tick line, got {}",
            lines.len()
        );
        let line = lines[0];
        let start = line.find("if entity @s[").expect("selector") + "if entity @s[".len();
        let end = start + line[start..].find(']').expect("selector close");
        assert_eq!(
            &line[start..end],
            want,
            "{fixture}: {obj} — the datapack adjudicates a different region from the one \
             `critical-path.json` hands the bot. These are two readers of one authored \
             `radius`, and this is exactly the drift that let a bot stop outside the box \
             and hang. Fix the emitter, never this assertion.\n  line: {line}"
        );
    }
    steps.len()
}

/// v0.2 emission is untouched: the pre-v0.3 sphere, both in the artifact and in
/// the line. `hello-world` / `keep-crawl` stay byte-identical.
#[test]
fn a_v02_campaign_keeps_the_pre_v03_sphere() {
    let (path, ticks) = build("v06-edits", "reach-completion-v02");
    let steps = reach_steps(&path);
    assert_eq!(steps.len(), 1, "the fixture has one reach objective");
    assert_eq!(steps[0]["completion"]["kind"], "sphere");
    assert_eq!(steps[0]["completion"]["radius"], steps[0]["radius"]);
    assert_eq!(assert_agreement("v06-edits", &path, &ticks), 1);
}

/// v0.3+ honours the authored radius. `v04-showcase` authors `radius: 2`, so the
/// volume is a cube of half-extent 2 — not the ±1 the emitter used to hard-code,
/// and not tighter than it either.
#[test]
fn a_v04_campaign_gets_the_radius_it_authored() {
    let (path, ticks) = build("v04-showcase", "reach-completion-v04");
    let steps = reach_steps(&path);
    assert_eq!(steps.len(), 1);
    let (c, pos) = (&steps[0]["completion"], &steps[0]["pos"]);
    assert_eq!(c["kind"], "cube");
    assert_eq!(steps[0]["radius"], 2);
    for i in 0..3 {
        let p = pos[i].as_i64().unwrap();
        assert_eq!(c["lo"][i].as_i64().unwrap(), p - 2, "half-extent 2 below");
        assert_eq!(c["hi"][i].as_i64().unwrap(), p + 2, "half-extent 2 above");
    }
    assert_eq!(assert_agreement("v04-showcase", &path, &ticks), 1);
}

/// The same rule at the current `dsl_version`, driven on a document the newest
/// pipeline built: a proof that is only shown at the version its surface was
/// introduced at has demonstrated the fence, not the rule. `blockout` authors
/// `radius: 3` — the live shape, and the one the old ±1 box was three times too
/// small for.
#[test]
fn the_newest_dsl_version_honours_the_radius_too() {
    let (path, ticks) = build("blockout", "reach-completion-v014");
    let steps = reach_steps(&path);
    assert!(!steps.is_empty(), "the fixture has reach objectives");
    for step in &steps {
        let (c, pos) = (&step["completion"], &step["pos"]);
        let r = step["radius"].as_i64().expect("radius");
        assert_eq!(c["kind"], "cube");
        assert_eq!(r, 3, "the fixture authors radius 3");
        for i in 0..3 {
            let p = pos[i].as_i64().unwrap();
            assert_eq!(c["lo"][i].as_i64().unwrap(), p - r);
            assert_eq!(c["hi"][i].as_i64().unwrap(), p + r);
        }
    }
    let n = assert_agreement("blockout", &path, &ticks);
    assert_eq!(n, steps.len());
    assert!(n >= 2, "binding: {n} reach objective(s) examined");
}

/// The floor the whole shape rests on: `hv-01` was a completion volume too tight
/// for a body standing on the anchor cell, and the answer to it must survive
/// every radius an author can write. `radius` is validated positive, so the
/// smallest volume the engine can produce is the ±1 cube — never smaller.
#[test]
fn the_smallest_authorable_volume_is_still_the_cube_that_closed_hv01() {
    use delvewright_compiler::plan::{ReachCompletion, reach_completion};
    for r in 1..=8u32 {
        let ReachCompletion::Cube { lo, hi } = reach_completion([10, 20, 30], r, true) else {
            panic!("v0.3+ is a cube");
        };
        let half = (hi[0] - lo[0]) / 2;
        assert!(half >= 1, "radius {r} must never be tighter than ±1");
        assert_eq!(half, r as i32, "radius {r} means radius {r}");
    }
    assert!(matches!(
        reach_completion([10, 20, 30], 4, false),
        ReachCompletion::Sphere { radius: 4, .. }
    ));
}
