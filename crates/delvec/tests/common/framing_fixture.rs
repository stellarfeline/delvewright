//! spec-0110's fixture: the spec-0082 assembly keep (`assembly_fixture`) with
//! one `cutscene` appended to the beat that spawns the assembly. The keep is a
//! 10 × 10 room: the exit (and the assembly on it) at `[5, 65, 8]`, the keeper
//! at `[5, 65, 4]`, the barred door across `z = 6` between them.
#![allow(dead_code)]

use delvec::compiler::emit::{BuildFailure, BuildOutput};
use serde_json::{Value, json};

use super::assembly_fixture as af;

/// A static shot from `offset` off the exit, framing `subject`.
pub fn shot(offset: [i32; 3], subject: Value) -> Value {
    json!({"seconds": 2, "subject": subject,
           "path": [{"anchor": "anchor/exit", "offset": offset}]})
}

/// The fixture's quests with `cutscene` appended to `obj/talk`'s bundle,
/// after the `spawn-assembly`.
pub fn quests_with_cutscene(cutscene: Value) -> Value {
    af::quests_with(|q| {
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(cutscene);
    })
}

/// A one-shot cutscene of `shots`.
pub fn cutscene(shots: Vec<Value>) -> Value {
    json!({"type": "cutscene", "shots": shots})
}

/// Build `q`.
pub fn build(q: &Value) -> Result<BuildOutput, BuildFailure> {
    af::try_build(&af::campaign(q), &af::prefabs_with(af::rig()))
}

/// Build `q`, which must build.
pub fn green(q: &Value) -> BuildOutput {
    match build(q) {
        Ok(o) => o,
        Err(BuildFailure::Diagnostic { code, message }) => panic!("{code}: {message}"),
        Err(BuildFailure::Validation(e)) => panic!("{} invalid command(s): {:?}", e.len(), e),
    }
}

/// `validation/shot-subjects.json` of a build.
pub fn ledger(out: &BuildOutput) -> Value {
    serde_json::from_slice(&out["validation/shot-subjects.json"]).unwrap()
}

/// The text of the one generated function whose name starts with `prefix`.
pub fn function(out: &BuildOutput, prefix: &str) -> String {
    let hits: Vec<&String> = out
        .keys()
        .filter(|k| {
            k.starts_with("datapack/data/")
                && k.rsplit('/').next().is_some_and(|n| n.starts_with(prefix))
        })
        .collect();
    assert_eq!(hits.len(), 1, "one function named {prefix}*: {hits:?}");
    String::from_utf8(out[hits[0]].clone()).unwrap()
}

/// Every keyframe `tp` of the cutscene driver: `(tick, pos, yaw, pitch)`.
pub fn keyframes(out: &BuildOutput) -> Vec<(i32, [f64; 3], f64, f64)> {
    function(out, "cs_tick_")
        .lines()
        .filter_map(|l| {
            let (head, tp) = l.split_once(" run tp @e[tag=")?;
            let tick: i32 = head.rsplit(' ').next()?.parse().ok()?;
            let nums: Vec<f64> = tp
                .split_once("] ")?
                .1
                .split(' ')
                .map(|n| n.parse().unwrap())
                .collect();
            Some((tick, [nums[0], nums[1], nums[2]], nums[3], nums[4]))
        })
        .collect()
}

/// Minecraft `(yaw, pitch)` from `from` toward `to`, as the emitter rounds it.
pub fn aim(from: [f64; 3], to: [f64; 3]) -> (f64, f64) {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let horiz = (d[0] * d[0] + d[2] * d[2]).sqrt();
    let r = |v: f64| {
        let r = (v * 1000.0).round() / 1000.0;
        if r == 0.0 { 0.0 } else { r }
    };
    (
        r((-d[0]).atan2(d[2]).to_degrees()),
        r((-d[1]).atan2(horiz).to_degrees()),
    )
}

/// The refusal code of `q`, or `None` when it builds.
pub fn code(q: &Value) -> Option<String> {
    match build(q) {
        Ok(_) => None,
        Err(BuildFailure::Diagnostic { code, .. }) => Some(code.id().to_string()),
        Err(BuildFailure::Validation(e)) => panic!("{} invalid command(s): {:?}", e.len(), e),
    }
}

/// The refusal of `q`, which must be refused with `want`.
pub fn refusal(q: &Value, want: &str) -> String {
    match build(q) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code.id(), want, "{message}");
            message
        }
        Err(BuildFailure::Validation(e)) => panic!("{} invalid command(s): {:?}", e.len(), e),
        Ok(_) => panic!("expected {want}, and it built"),
    }
}
