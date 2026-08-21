//! The camera-coverage guarantee for granted vision (island round 16).
//!
//! Owner playtest: "the night-vision effect expires mid-ending-cutscene and
//! flickers." The island declares `mitigation: "night-vision"` on `area/island`
//! and nothing on `area/open-sea`, where the ending plays. Boarding transports
//! the party out of the mitigated box and immediately runs a 15-second camera —
//! so they arrived holding at most the 12-second lease, vanilla's `GameRenderer`
//! began ramping the brightness down 1.5 s later (it ramps below 200 ticks
//! remaining), and the effect died mid-shot.
//!
//! The rule the compiler now guarantees: **a vision effect it grants outlasts any
//! authored camera it can overlap, plus vanilla's flicker window.** Sized from the
//! campaign's own longest cutscene, because the compiler cannot know which camera
//! a player stepping out of a mitigated area will land in.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{RawCampaign, parse_campaign};

/// Vanilla's night-vision wind-down, in seconds (`GameRenderer` ramps below
/// 200 ticks remaining). Mirrored here so the test states the requirement in its
/// own terms rather than importing the constant it is checking.
const FLICKER_SECONDS: u32 = 10;

fn read(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

/// hello-world's world doc with the keep declaring the night-vision mitigation.
fn world_with_mitigation() -> String {
    let w = read(&common::hello_world_dir(), "world.json");
    let mut v: serde_json::Value = serde_json::from_str(&w).unwrap();
    v["dsl_version"] = serde_json::json!("0.6.0");
    v["content"]["areas"][0]["mitigation"] = serde_json::json!("night-vision");
    serde_json::to_string(&v).unwrap()
}

/// The `cutscene-shots` quests doc, with every shot's duration rewritten to
/// `seconds` so the campaign's longest camera is exactly `2 * seconds`.
fn quests_with_camera(seconds: u32) -> String {
    let q = read(&common::cutscene_shots_dir(), "quests.json");
    let mut v: serde_json::Value = serde_json::from_str(&q).unwrap();
    v["campaign_id"] = serde_json::json!("hello-world");
    let shots = v["content"]["quests"][0]["on_complete"][0]["shots"]
        .as_array_mut()
        .unwrap();
    for s in shots.iter_mut() {
        s["seconds"] = serde_json::json!(seconds);
    }
    serde_json::to_string(&v).unwrap()
}

/// hello-world's quests doc, unchanged: a campaign with no camera at all.
fn quests_without_camera() -> String {
    read(&common::hello_world_dir(), "quests.json")
}

fn night_vision_clock(quests: String, world: String) -> String {
    let hw = common::hello_world_dir();
    let raw = RawCampaign {
        world,
        npcs: read(&hw, "npcs.json"),
        classes: read(&hw, "classes.json"),
        quest_plan: read(&hw, "quest-plan.json"),
        quests,
        dialogue: read(&hw, "dialogue.json").replacen("\"0.2.0\"", "\"0.6.0\"", 1),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let out: BuildOutput = emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("build succeeds");
    let key = out
        .keys()
        .find(|k| k.ends_with("/function/night_vision_tick.mcfunction"))
        .expect("a declared mitigation emits night_vision_tick")
        .clone();
    String::from_utf8(out[&key].clone()).unwrap()
}

/// The seconds in `effect give … minecraft:night_vision <n> 0 true`.
fn lease_seconds(clock: &str) -> u32 {
    let line = clock
        .lines()
        .find(|l| l.contains("minecraft:night_vision"))
        .unwrap_or_else(|| panic!("no grant in clock:\n{clock}"));
    let after = line.split("minecraft:night_vision ").nth(1).unwrap();
    after.split_whitespace().next().unwrap().parse().unwrap()
}

/// **The guarantee.** The lease outlasts the campaign's longest camera by at
/// least vanilla's flicker window, so an effect cannot begin ramping down while
/// an authored camera is still running.
#[test]
fn the_lease_outlasts_the_longest_camera_plus_the_flicker_window() {
    for shot in [6_u32, 15, 20] {
        let camera = shot * 2; // the fixture has two shots
        let lease = lease_seconds(&night_vision_clock(
            quests_with_camera(shot),
            world_with_mitigation(),
        ));
        assert!(
            lease >= camera + FLICKER_SECONDS,
            "a {camera}s camera needs a lease of at least {} s to never ramp down \
             on screen; got {lease}",
            camera + FLICKER_SECONDS
        );
    }
}

/// The lease tracks the camera: a longer cutscene buys a longer lease. Pins that
/// the bound is *derived*, not a bigger hardcoded number that happens to pass.
#[test]
fn a_longer_camera_buys_a_longer_lease() {
    let short = lease_seconds(&night_vision_clock(
        quests_with_camera(15),
        world_with_mitigation(),
    ));
    let long = lease_seconds(&night_vision_clock(
        quests_with_camera(20),
        world_with_mitigation(),
    ));
    assert!(
        long > short,
        "the lease is derived from the longest camera ({short} vs {long})"
    );
}

/// The floor is unchanged: a campaign with no camera keeps the historical 12 s
/// lease, so every pre-existing campaign's `night_vision_tick` is byte-identical.
#[test]
fn a_campaign_with_no_camera_keeps_the_historical_lease() {
    let clock = night_vision_clock(quests_without_camera(), world_with_mitigation());
    assert_eq!(
        lease_seconds(&clock),
        12,
        "no camera means nothing to cover, so the pre-existing floor stands:\n{clock}"
    );
    assert!(
        clock.contains("minecraft:night_vision 12 0 true"),
        "byte-identical to the pre-round-16 emission:\n{clock}"
    );
}

/// Deterministic (ADR-0006): the lease is a pure function of the campaign.
#[test]
fn the_lease_is_deterministic() {
    let a = night_vision_clock(quests_with_camera(15), world_with_mitigation());
    let b = night_vision_clock(quests_with_camera(15), world_with_mitigation());
    assert_eq!(a, b);
}
