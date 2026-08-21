//! Branch-aware walk-origin chaining (`DW0488`, island round 16).
//!
//! A scripted walk starts where the body's *previous* walk left it. That chain
//! used to be one flat sequence per body, walked in campaign effect order with
//! no regard for which branch each leg belonged to — so a leg the player's
//! branch can never reach still handed its destination to the next leg as an
//! origin.
//!
//! Owner playtest, island round 15: choosing to WAIT teleported `npc/eurylochus`
//! out of the cave down to the beach and walked him 35 seconds back up, because
//! the `flag/flee`-gated walk to the gangplank had overwritten the origin the
//! `flag/wait`-gated walk to the alcove inherited. `npc/perimedes` had the same
//! defect on the same branch, unreported, and a third body — Eurylochus again —
//! shared ONE driver between two beats that reach the gangplank from different
//! places, so the wait branch teleported him into the cave before walking him
//! down.
//!
//! The three shapes are one defect and are pinned here:
//!
//! 1. a gated leg must not poison a leg on a different branch;
//! 2. two beats that walk one body to one mark from different branches get their
//!    own drivers, each planned from its own origin;
//! 3. a campaign that gates no walk emits exactly the names it always did.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// Three walks for one body, in campaign effect order:
///
/// * `obj/talk` — **unconditional** to `anchor/exit`;
/// * `obj/talk` — `requires flag/flee` back to `anchor/keeper-stand`;
/// * `on_complete` — `requires flag/wait` to `anchor/door`.
///
/// The third leg's origin must be `anchor/exit` (the last leg its own branch
/// can prove ran), never `anchor/keeper-stand` (the flee leg's destination).
const QUESTS_BRANCHED: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/keeper-stand",
              "requires_flags": ["flag/flee"] }
          ]
        },
        "on_complete": [
          { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/door",
            "requires_flags": ["flag/wait"] },
          { "type": "campaign-complete" }
        ]
      }
    ]
  }
}"#;

/// Two beats walk one body to ONE mark from two different places — the island's
/// `anchor/gangplank` shape. Leg 1 is unconditional to `anchor/exit`; leg 2 is
/// `flag/flee`-gated to `anchor/door` (so the flee branch stands at the door);
/// leg 3 is `flag/flee`-gated to `anchor/exit` (from the door) and leg 4 is
/// unconditional to `anchor/exit` (from `anchor/exit` itself is degenerate, so
/// leg 1 goes to `keeper-stand` instead). Kept deliberately small: the point is
/// that `(body, destination)` alone cannot key a driver.
const QUESTS_SHARED_MARK: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/door",
              "requires_flags": ["flag/flee"] },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
              "requires_flags": ["flag/flee"] },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// The pre-branch baseline: one unconditional walk, nothing gated anywhere.
const QUESTS_UNGATED: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

fn parse_with(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json").replacen("\"0.2.0\"", "\"0.6.0\"", 1),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build_with(quests: &str) -> BuildOutput {
    let campaign = parse_with(quests);
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
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

const FN_DIR: &str = "datapack/data/hello-world/function";

/// Every emitted function path, for name-shape assertions.
fn names(out: &BuildOutput) -> Vec<String> {
    out.keys()
        .filter(|p| p.starts_with(FN_DIR))
        .map(|p| p.rsplit('/').next().unwrap().to_string())
        .collect()
}

fn body(out: &BuildOutput, file: &str) -> String {
    out.iter()
        .find(|(p, _)| p.as_str() == format!("{FN_DIR}/{file}"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("expected `{file}`; have {:?}", names(out)))
}

/// The `x y z` of a walk driver's first `tp` — the cell the body is standing on
/// when the driver starts, i.e. the planned origin.
fn first_waypoint(out: &BuildOutput, tick_fn: &str) -> String {
    let text = body(out, tick_fn);
    let line = text
        .lines()
        .find(|l| l.contains("matches 0 run tp "))
        .unwrap_or_else(|| panic!("no waypoint 0 in `{tick_fn}`:\n{text}"));
    line.split(" run tp ")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .skip(1)
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The single `mv_tick_*` driver whose name starts with `stem`.
fn driver_named(out: &BuildOutput, stem: &str) -> String {
    let mut hits: Vec<String> = names(out)
        .into_iter()
        .filter(|n| n.starts_with(stem) && n.ends_with(".mcfunction"))
        .collect();
    hits.sort();
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one driver starting `{stem}`, got {hits:?}"
    );
    hits.pop().unwrap()
}

/// **The regression.** A `flag/wait`-gated walk must start where the last leg
/// its own branch proves ran left the body — never at the destination of a
/// `flag/flee`-gated leg it can never co-occur with.
#[test]
fn a_gated_leg_does_not_poison_another_branchs_origin() {
    let out = build_with(QUESTS_BRANCHED);

    // Leg 1 (unconditional) is the only staging the wait branch can prove, so
    // both the flee leg and the wait leg start where it ended.
    let exit_end = {
        let t = body(&out, "mv_tick_keeper_exit.mcfunction");
        let last = t.lines().rfind(|l| l.contains(" run tp ")).unwrap();
        last.split(" run tp ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .skip(1)
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    };

    let wait_leg = driver_named(&out, "mv_tick_keeper_door_b");
    let flee_leg = driver_named(&out, "mv_tick_keeper_keeper_stand_b");

    assert_eq!(
        first_waypoint(&out, &wait_leg),
        exit_end,
        "the wait-branch walk must begin where the unconditional leg left the body"
    );
    assert_eq!(
        first_waypoint(&out, &flee_leg),
        exit_end,
        "the flee-branch walk begins from the same proven staging"
    );

    // The precise defect: the wait leg must NOT start at the flee leg's target.
    let flee_end = {
        let t = body(&out, &flee_leg);
        let last = t.lines().rfind(|l| l.contains(" run tp ")).unwrap();
        last.split(" run tp ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .skip(1)
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_ne!(
        first_waypoint(&out, &wait_leg),
        flee_end,
        "the wait branch inherited the flee branch's destination — the island \
         round-15 teleport"
    );
}

/// Two beats reaching ONE mark from two branches get **two** drivers, each
/// planned from its own origin. Before this, `(body, destination)` keyed a
/// single driver and the second beat silently ran the first beat's polyline.
#[test]
fn one_mark_two_branches_emits_two_drivers() {
    let out = build_with(QUESTS_SHARED_MARK);
    let all = names(&out);
    let to_exit: Vec<&String> = all
        .iter()
        .filter(|n| n.starts_with("mv_tick_keeper_exit"))
        .collect();
    assert_eq!(
        to_exit.len(),
        2,
        "one driver per branch reaching `anchor/exit`, got {to_exit:?}"
    );

    // The unconditional one keeps the historical, suffix-free name; the gated
    // one carries the branch key.
    assert!(
        all.iter().any(|n| n == "mv_tick_keeper_exit.mcfunction"),
        "the unconditional walk keeps its unsuffixed driver name: {all:?}"
    );
    let gated = driver_named(&out, "mv_tick_keeper_exit_b");
    assert_ne!(
        first_waypoint(&out, "mv_tick_keeper_exit.mcfunction"),
        first_waypoint(&out, &gated),
        "the two beats reach the same mark from different places — that is why \
         they cannot share one driver"
    );
}

/// A campaign that gates no walk emits exactly the names it always did: the
/// branch key is the empty string when there is no branch, so every pre-existing
/// campaign's output is untouched.
#[test]
fn an_ungated_campaign_keeps_its_historical_driver_names() {
    let out = build_with(QUESTS_UNGATED);
    let all = names(&out);
    assert!(
        all.iter().any(|n| n == "mv_keeper_exit.mcfunction"),
        "start function keeps its name: {all:?}"
    );
    assert!(
        all.iter().any(|n| n == "mv_tick_keeper_exit.mcfunction"),
        "driver keeps its name: {all:?}"
    );
    assert!(
        !all.iter().any(|n| n.contains("_b") && n.starts_with("mv_")),
        "no branch suffix anywhere in an ungated campaign: {all:?}"
    );
}

/// Planning is deterministic: the branch key is derived from sorted flag names,
/// so two builds of the same DSL name the same functions (ADR-0006).
#[test]
fn branch_keyed_driver_names_are_deterministic() {
    let a = names(&build_with(QUESTS_BRANCHED));
    let b = names(&build_with(QUESTS_BRANCHED));
    assert_eq!(a, b);
}

/// The residual case the branch key cannot separate, and therefore reports:
/// two occurrences on the **same** branch walk one body to one mark from two
/// different places. They share a driver by construction, so one of them would
/// open with a teleport — `DW0488` refuses to ship it.
///
/// This is the island's `anchor/gangplank` shape with the branch gates removed:
/// keeper walks to `anchor/exit`, then to `anchor/door`, then to `anchor/exit`
/// again. The third leg stands at the door; the driver was planned from the
/// keeper's stand.
#[test]
fn two_origins_on_one_branch_is_dw0488() {
    const QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;
    let campaign = parse_with(QUESTS);
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
    let err = emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect_err("a shared driver with two origins must not build");
    match err {
        emit::BuildFailure::Diagnostic { code, message } => {
            assert_eq!(code, "DW0488", "message was: {message}");
            assert!(
                message.contains("npc/keeper") && message.contains("anchor/exit"),
                "the diagnostic names the body and the mark: {message}"
            );
        }
        other => panic!("unexpected non-diagnostic build failure: {other:?}"),
    }
}
