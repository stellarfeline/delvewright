//! A `talk-to` step stands where the **cast ledger** stations the body, not at
//! the NPC's static stage-2 anchor (island-release blocker).
//!
//! ## The defect
//!
//! `npc/perimedes` declares `anchor: anchor/mouth` in stage 2 and is cast at
//! `anchor/alcove-2` for his `obj/the-stone` beat. The emitted runtime cast is
//! correct — the body stands in the alcove and a human clicks it — but
//! `critical-path.json` carried the *mouth*, so the eye-ray bot
//! walked to the mouth and could not acquire the target through the sealed
//! boulder region's wall of interaction entities. Two sources of truth (the
//! static anchor and the ledger) that emission and the bot contract read
//! differently; the old click-by-entity-id bot never noticed.
//!
//! ## The fixtures
//!
//! * `talkto-cast-pos` — the mainline shape: the Keeper is declared at
//!   `anchor/keeper-stand`, a `move-npc` walks him to `anchor/exit`, and the
//!   second `talk-to` is cast there. One NPC, one move, no branch.
//! * `talkto-cast-pos-branch` — the per-branch shape: one fork decides whether
//!   the Keeper keeps his post or walks out to the road, and **both** branches
//!   walk the same `talk-to` afterwards. Each branch path must carry its own
//!   cast position.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::Value;

/// Build a fixture campaign end to end and return its output tree.
fn build(fixture: &str) -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(fixture);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
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
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

fn json(out: &BuildOutput, path: &str) -> Value {
    serde_json::from_slice(out.get(path).unwrap_or_else(|| panic!("missing {path}")))
        .expect("valid json")
}

/// The `pos` of the step proving `objective` in an exported path artifact.
fn step_pos(path: &Value, objective: &str) -> Vec<i64> {
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == objective)
        .unwrap_or_else(|| panic!("no step proves {objective}: {path:#}"))["pos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect()
}

/// Every leg destination of a waypoint artifact.
fn leg_destinations(wp: &Value) -> Vec<Vec<i64>> {
    wp["legs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            l["to"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap())
                .collect()
        })
        .collect()
}

/// Every `pos` an exported path visits.
fn all_positions(path: &Value) -> Vec<Vec<i64>> {
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s.get("pos"))
        .map(|p| {
            p.as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap())
                .collect()
        })
        .collect()
}

/// The world cell an area anchor resolves to, read off the plan the same build
/// used — so the assertion names the anchor, never a hand-copied literal.
fn anchor_cell(fixture: &str, anchor: &str) -> Vec<i64> {
    let dir = common::compiler_fixtures_dir().join(fixture);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    plan.point("area/keep", anchor)
        .unwrap_or_else(|| panic!("anchor {anchor} does not resolve"))
        .iter()
        .map(|n| *n as i64)
        .collect()
}

/// The mainline defect: the Keeper's ledger walks him to `anchor/exit` before
/// the farewell beat, so the bot contract must send the party to the exit — not
/// to the stage-2 anchor he vacated two quests ago.
#[test]
fn talk_to_step_stands_where_the_cast_ledger_stations_the_body() {
    let out = build("talkto-cast-pos");
    let cp = json(&out, "critical-path.json");

    let stand = anchor_cell("talkto-cast-pos", "anchor/keeper-stand");
    let exit = anchor_cell("talkto-cast-pos", "anchor/exit");
    assert_ne!(stand, exit, "the fixture must move the body somewhere else");

    // Beat 1: cast at the declared anchor — unchanged behavior.
    assert_eq!(
        step_pos(&cp, "obj/ask"),
        stand,
        "the first beat is cast where the NPC was declared"
    );
    // Beat 2: cast at `anchor/exit`. The static anchor is stale by now.
    assert_eq!(
        step_pos(&cp, "obj/farewell"),
        exit,
        "the talk-to step must carry the CAST position, not the stage-2 anchor"
    );
}

/// The waypoint polyline is derived from the step positions, so it has to move
/// with them: a leg the harness walks must end on a position of its own path.
#[test]
fn waypoints_follow_the_cast_positions() {
    let out = build("talkto-cast-pos");
    let cp = json(&out, "critical-path.json");
    let wp = json(&out, "validation/critical-path-waypoints.json");
    let positions = all_positions(&cp);
    let legs = leg_destinations(&wp);
    assert!(!legs.is_empty(), "the fixture walks at least one leg");
    for to in &legs {
        assert!(
            positions.contains(to),
            "leg destination {to:?} is not one of the path's step positions {positions:?}"
        );
    }
    assert!(
        legs.contains(&anchor_cell("talkto-cast-pos", "anchor/exit")),
        "the walked legs must reach the exit the ledger stations the Keeper at: {legs:?}"
    );
}

/// Per branch: the same `talk-to`, two different bodies-in-place. Each branch
/// path artifact must carry the position ITS ledger row declares — the shape
/// `Plan::branch_critical_path` inherits from the shared builder.
#[test]
fn each_branch_path_carries_its_own_cast_position() {
    let out = build("talkto-cast-pos-branch");
    let hold = json(&out, "validation/branch-path-hold.json");
    let bolt = json(&out, "validation/branch-path-bolt.json");

    let stand = anchor_cell("talkto-cast-pos-branch", "anchor/keeper-stand");
    let exit = anchor_cell("talkto-cast-pos-branch", "anchor/exit");

    // The fork itself is pre-branch: both paths meet him at his post.
    assert_eq!(step_pos(&hold, "obj/decide"), stand);
    assert_eq!(step_pos(&bolt, "obj/decide"), stand);

    // After it, the ledger diverges — and so must the two contracts.
    assert_eq!(
        step_pos(&hold, "obj/parley"),
        stand,
        "the hold branch keeps him at his post"
    );
    assert_eq!(
        step_pos(&bolt, "obj/parley"),
        exit,
        "the bolt branch's ledger walked him to the road; its path must say so"
    );
    assert_ne!(
        step_pos(&hold, "obj/parley"),
        step_pos(&bolt, "obj/parley"),
        "two branches that stage the body differently cannot share one position"
    );
}

/// Each branch's waypoints are built from its own path, so they diverge too.
#[test]
fn branch_waypoints_follow_each_branch_cast_positions() {
    let out = build("talkto-cast-pos-branch");
    for slug in ["hold", "bolt"] {
        let cp = json(&out, &format!("validation/branch-path-{slug}.json"));
        let wp = json(&out, &format!("validation/branch-waypoints-{slug}.json"));
        let positions = all_positions(&cp);
        for to in &leg_destinations(&wp) {
            assert!(
                positions.contains(to),
                "{slug}: leg destination {to:?} is not one of ITS OWN path's step positions \
                 {positions:?}"
            );
        }
    }
    let bolt_wp = json(&out, "validation/branch-waypoints-bolt.json");
    assert!(
        leg_destinations(&bolt_wp).contains(&anchor_cell("talkto-cast-pos-branch", "anchor/exit")),
        "the bolt branch walks out to the road it cast the Keeper onto"
    );
}
