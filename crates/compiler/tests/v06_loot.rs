//! spec-0021 build tier: the container proof (`DW0431`) and loot emission.
//!
//! The DSL tier cannot know whether an anchor's cell holds a chest — that needs
//! the assembled world — so `DW0431` is a build error. hello-room furnishes no
//! container, which makes it the perfect negative fixture: a well-formed `loot`
//! entry pointed at a real, resolvable anchor must still fail the build rather
//! than emit an `item replace block` that would fail SILENTLY on the server.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};

fn quests_doc(loot: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{ "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ] }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "loot": [ {loot} ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str) -> Campaign {
    parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
    })
    .expect("campaign parses")
}

/// Build, returning the failure so the test can assert on its code.
fn try_build(loot: &str) -> Result<emit::BuildOutput, emit::BuildFailure> {
    let c = parse_hw(&quests_doc(loot));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&c, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "campaign must validate clean: {diags:#?}");
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        prefabs_ref(&prefabs),
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn prefabs_ref(p: &PrefabRegistry) -> &PrefabRegistry {
    p
}

/// A `loot` entry aimed at an anchor whose cell is not a container fails the
/// build with `DW0431` — never a silently-dropped `item replace block`.
#[test]
fn loot_on_a_non_container_anchor_is_dw0431() {
    let err = try_build(
        r#"{ "id": "loot/stores", "anchor": "anchor/exit",
             "items": [ { "item": "minecraft:cooked_cod", "count": 3 } ] }"#,
    )
    .expect_err("a non-container anchor must fail the build");
    match err {
        emit::BuildFailure::Diagnostic { code, message } => {
            assert_eq!(code, "DW0431");
            assert!(
                message.contains("loot/stores") && message.contains("anchor/exit"),
                "the message must name the offending loot and anchor: {message}"
            );
        }
        other => panic!("expected a DW0431 diagnostic, got {other:?}"),
    }
}

/// No `loot` at all: the campaign builds exactly as before.
#[test]
fn no_loot_still_builds() {
    assert!(
        try_build("").is_ok(),
        "a loot-free campaign must still build"
    );
}
