//! The blocking chart's corridor and the build's routes are one answer.
//!
//! `delvec build` proves the critical path over its final world and writes the
//! routes' thinned waypoints to `validation/critical-path-waypoints.json`;
//! `delvec blocking-chart` states the corridor it tints in its index. Both are
//! asked of the same campaign through the process, and the corridor must hold
//! every cell the build's routes name: a leg count that differs, or a waypoint,
//! use-gate or climb end the corridor lacks, is a chart drawn over a
//! route the build never proved.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str], prefabs: &Path) {
    let out = Command::new(BIN)
        .args(args)
        .arg("--prefabs")
        .arg(prefabs)
        .output()
        .expect("run delvec");
    assert!(
        out.status.success(),
        "delvec {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap_or_else(|e| panic!("{path:?}: {e}")))
        .expect("JSON")
}

fn cell(v: &serde_json::Value) -> [i64; 3] {
    let a = v.as_array().expect("a cell is an array");
    [
        a[0].as_i64().unwrap(),
        a[1].as_i64().unwrap(),
        a[2].as_i64().unwrap(),
    ]
}

/// What the chart's corridor holds against the build's routes for one campaign:
/// `(legs, cells held to the corridor)`. Panics on any difference.
fn agree(tag: &str, campaign: &Path, prefabs: &Path) -> (usize, usize) {
    let build = scratch(&format!("chart-routes-build-{tag}"));
    let chart = scratch(&format!("chart-routes-chart-{tag}"));
    run(
        &[
            "build",
            campaign.to_str().unwrap(),
            "-o",
            build.to_str().unwrap(),
        ],
        prefabs,
    );
    run(
        &[
            "blocking-chart",
            campaign.to_str().unwrap(),
            "-o",
            chart.to_str().unwrap(),
        ],
        prefabs,
    );

    let index = json(&chart.join("blocking-chart.json"));
    let corridor: BTreeSet<[i64; 3]> = index["corridor"]["cells"]
        .as_array()
        .expect("the index states its corridor")
        .iter()
        .map(cell)
        .collect();
    assert_eq!(
        index["corridor"]["cell_count"].as_u64().unwrap() as usize,
        corridor.len(),
        "{tag}: the index's own count disagrees with its cells"
    );

    let waypoints = build.join("validation/critical-path-waypoints.json");
    // A build that walks no leg writes no waypoint file: the chart then draws
    // no corridor, and says so.
    if !waypoints.exists() {
        assert_eq!(index["corridor"]["legs"], 0, "{tag}");
        assert!(corridor.is_empty(), "{tag}: a corridor with no route");
        return (0, 0);
    }
    let doc = json(&waypoints);
    let legs = doc["legs"].as_array().expect("legs");
    assert_eq!(
        index["corridor"]["legs"].as_u64().unwrap() as usize,
        legs.len(),
        "{tag}: the chart's routes have a different number of legs than the build's"
    );
    let mut held = 0;
    let mut hold = |what: &str, v: &serde_json::Value| {
        let c = cell(v);
        assert!(
            corridor.contains(&c),
            "{tag}: the build's {what} {c:?} is not in the chart's corridor"
        );
        held += 1;
    };
    for leg in legs {
        for w in leg["waypoints"].as_array().unwrap() {
            hold("waypoint", w);
        }
        for g in leg["use_gates"].as_array().into_iter().flatten() {
            hold("use-gate", g);
        }
        for c in leg["climbs"].as_array().into_iter().flatten() {
            hold("climb start", &c["from"]);
            hold("climb end", &c["to"]);
        }
    }
    (legs.len(), held)
}

#[test]
fn the_charts_corridor_holds_every_cell_the_builds_routes_name() {
    let mut legs = 0;
    let mut held = 0;
    let mut campaigns = 0;
    for (tag, dir) in [
        ("keep-vertical", common::keep_vertical_dir()),
        ("keep-crawl", common::keep_crawl_dir()),
        ("keep-trial", common::keep_trial_dir()),
        ("hello-world", common::hello_world_dir()),
    ] {
        let (l, h) = agree(tag, &dir, &common::prefabs_dir());
        legs += l;
        held += h;
        campaigns += 1;
    }
    eprintln!(
        "chart-routes binding: {legs} leg(s), {held} cell(s) held to the corridor over {campaigns} campaign(s)"
    );
    // The denominator: a run that compared nothing is not an agreement.
    assert!(legs > 0 && held > 0, "no route was compared");
}

/// A leg that ends at a holder `collect` ends at the stance the build's
/// stance pass chose (spec-0111 §5.2), so the chart's corridor is walked in
/// the same stanced world: hello-world with its exit objective a `collect`
/// adopting the glow frame the room carries.
#[test]
fn a_leg_ending_at_a_holder_ends_at_the_builds_stance_in_the_chart() {
    let dir = scratch("chart-routes-holder");
    let prefabs = common::holder_prefabs(&dir.join("prefabs"), vec![common::frame_entity()]);
    let camp = dir.join("camp");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |q| {
        q["content"]["quests"][0]["objectives"][1] = serde_json::json!({
            "type": "collect", "id": "obj/exit", "item": "minecraft:map", "count": 1,
            "anchor": "anchor/exit", "holder": "anchor/frame", "after": ["obj/talk"],
            "title": "Take the map", "item_name": "Keeper's Map",
            "happening": {"text": "the party takes the map", "verb": "gains"}
        });
    });
    let (legs, held) = agree("holder", &camp, &prefabs);
    eprintln!("chart-routes holder binding: {legs} leg(s), {held} cell(s) held to the corridor");
    assert!(legs > 0 && held > 0, "no route was compared");
}
