//! **A shrieker's answer is predicted only on a leg no free body can take its
//! sensor's click on** (spec-0100 §2.4, §4.6). A mob's step sets a sensor off
//! and never reaches a shrieker, and a sensor that clicked is deaf for its
//! busy span; so a leg walked while a wave or an actor's free twin can stand
//! within the sensor's listener radius may never hear the shriek it walks for.
//!
//! Asked of the gallery primary. Its listening floor's sensor is predicted on
//! three legs: the walk to the Marshal (before the muster forms up and before
//! the rafters' ambush can be sprung), the climb onto the cabin and the walk
//! to the exit (after both). The first keeps its shrieker; the two after it
//! carry the sensor, withhold the shrieker, and name the bodies — the muster,
//! seated beside the floor, and the rafter spider, a climber its ambush looses
//! after the despawn the muster's clearing fires.

use crate::common;

use std::path::Path;
use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_gallery_withholds_the_shriek_on_the_legs_a_free_body_can_reach() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sculk-free-bodies");
    let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
    let out = dir.join("build");
    let _ = std::fs::remove_dir_all(&out);
    let r = Command::new(BIN)
        .arg("build")
        .arg(&campaign)
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(&prefabs)
        .output()
        .expect("delvec runs");
    assert!(
        r.status.success(),
        "the gallery primary builds:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let path = read(&out.join("critical-path.json"));
    let legs = read(&out.join("validation/critical-path-waypoints.json"));
    // The position each named step walks to, read off the exported path.
    let pos_of = |key: &str, value: &str| -> Value {
        path["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s[key] == value)
            .unwrap_or_else(|| panic!("a step with {key} {value}"))["pos"]
            .clone()
    };
    let leg_to = |pos: &Value| -> Value {
        legs["legs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["to"] == *pos)
            .unwrap_or_else(|| panic!("a leg to {pos}"))
            .clone()
    };
    // The floor's sensor is the one with a shrieker in reach.
    let floor = |leg: &Value| -> Value {
        leg["vibrations"]
            .as_array()
            .unwrap_or_else(|| panic!("the leg predicts vibrations: {leg}"))
            .iter()
            .find(|v| {
                !v["shriekers"].as_array().unwrap().is_empty()
                    || v["withheld"]["shriekers"].as_array().is_some()
            })
            .unwrap_or_else(|| panic!("the leg carries the listening floor's sensor: {leg}"))
            .clone()
    };

    let marshal = floor(&leg_to(&pos_of("npc", "npc/marshal")));
    assert_eq!(
        marshal["shriekers"].as_array().unwrap().len(),
        1,
        "the walk to the Marshal keeps its shriek: {marshal}"
    );
    assert!(marshal.get("withheld").is_none(), "{marshal}");

    for anchor in ["anchor/cabin-roof", "anchor/exit"] {
        let v = floor(&leg_to(&pos_of("anchor", anchor)));
        assert_eq!(v["sensor"], marshal["sensor"], "{anchor}: the same sensor: {v}");
        assert!(
            v["shriekers"].as_array().unwrap().is_empty(),
            "{anchor}: a free body can take the click, so no shriek is predicted: {v}"
        );
        assert_eq!(v["withheld"]["shriekers"], marshal["shriekers"], "{anchor}: {v}");
        let by: Vec<&str> = v["withheld"]["by"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b.as_str().unwrap())
            .collect();
        for body in ["wave/muster", "actor/rafter-spider"] {
            assert!(by.contains(&body), "{anchor}: withheld by {body}: {by:?}");
        }
    }
}
