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
    quests_doc_with(loot, "")
}

fn quests_doc_with(loot: &str, actors: &str) -> String {
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
    "loot": [ {loot} ],
    "actors": [ {actors} ]
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
    try_build_doc(&quests_doc(loot))
}

fn try_build_doc(doc: &str) -> Result<emit::BuildOutput, emit::BuildFailure> {
    let c = parse_hw(doc);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&c, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "campaign must validate clean: {diags:#?}");
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
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

/// Validate only (no build), against the FULL 1.21.11 registry — the tier that
/// carries the stack-size table `DW0436` reads.
fn validate_doc(doc: &str) -> Vec<delvewright_dsl::Diagnostic> {
    let c = parse_hw(doc);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        &c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

/// `DW0436`: a `loot` count above the item's max stack size. `item replace …
/// container.<n>` fails SILENTLY above the cap — the-drowned-bell round 2 shipped
/// an empty chest slot from `rabbit_stew` × 2 (cap 1). The message must name the
/// item, the declared count and the cap.
#[test]
fn a_loot_count_above_the_items_stack_size_is_dw0436() {
    let diags = validate_doc(&quests_doc(
        r#"{ "id": "loot/stores", "anchor": "anchor/exit",
             "items": [ { "item": "minecraft:rabbit_stew", "count": 2 } ] }"#,
    ));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0436")
        .unwrap_or_else(|| panic!("an over-cap loot count must fire DW0436: {diags:#?}"));
    assert_eq!(d.path, "/content/loot/0/items/0/count");
    assert!(
        d.message.contains("minecraft:rabbit_stew")
            && d.message.contains("2")
            && d.message.contains("at most 1"),
        "the message must name the item, the declared count and the cap: {}",
        d.message
    );
    // An ender pearl caps at 16, not 1 — the table is per-item, not a 1-vs-64 rule.
    let diags = validate_doc(&quests_doc(
        r#"{ "id": "loot/stores", "anchor": "anchor/exit",
             "items": [ { "item": "minecraft:ender_pearl", "count": 17 } ] }"#,
    ));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0436" && d.message.contains("at most 16")),
        "{diags:#?}"
    );
}

/// A count exactly at the cap — and any count under it — is clean. The check is a
/// cap, never a nudge toward smaller stacks.
#[test]
fn a_loot_count_at_or_below_the_cap_is_clean() {
    for (item, count) in [
        ("minecraft:rabbit_stew", 1),
        ("minecraft:ender_pearl", 16),
        ("minecraft:cooked_cod", 64),
    ] {
        let diags = validate_doc(&quests_doc(&format!(
            r#"{{ "id": "loot/stores", "anchor": "anchor/exit",
                 "items": [ {{ "item": "{item}", "count": {count} }} ] }}"#
        )));
        assert!(
            !diags.iter().any(|d| d.code == "DW0436"),
            "{item} x{count} must be clean: {diags:#?}"
        );
    }
}

/// The same silent `item replace … container.<n>` failure reaches a `collect`
/// objective's prop chest, so `DW0436` covers it too — an over-cap count would
/// leave the chest empty and the objective uncompletable.
#[test]
fn a_collect_objective_count_above_the_stack_size_is_dw0436() {
    let doc = quests_doc("").replace(
        r#"{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }"#,
        r#"{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
           { "type": "collect", "id": "obj/get", "item": "minecraft:rabbit_stew",
             "count": 3, "anchor": "anchor/exit", "after": ["obj/talk"] }"#,
    );
    let diags = validate_doc(&doc);
    let d = diags
        .iter()
        .find(|d| d.code == "DW0436")
        .unwrap_or_else(|| panic!("an over-cap collect count must fire DW0436: {diags:#?}"));
    assert!(d.path.ends_with("/count"), "{}", d.path);
    assert!(d.message.contains("obj/get"), "{}", d.message);
}

/// No `loot` at all: the campaign builds exactly as before.
#[test]
fn no_loot_still_builds() {
    assert!(
        try_build("").is_ok(),
        "a loot-free campaign must still build"
    );
}

/// An equipped actor: the gear reaches the emitted puppet AND the twin, and the
/// generated PackTest asserts the handoff keeps it — the regression that would
/// otherwise be invisible (unleash summons a fresh entity).
#[test]
fn an_equipped_actor_emits_gear_and_its_packtest() {
    const ACTOR: &str = r#"{
      "id": "actor/elite",
      "entity": "minecraft:wither_skeleton",
      "anchor": "anchor/keeper-stand",
      "equipment": {
        "head": { "item": "minecraft:netherite_helmet",
                  "enchantments": { "minecraft:protection": 4 } },
        "main_hand": { "item": "minecraft:netherite_sword",
                       "enchantments": { "minecraft:sharpness": 5 } }
      }
    }"#;
    let out = try_build_doc(&quests_doc_with("", ACTOR)).expect("builds");
    let f = |p: &str| {
        String::from_utf8(out.get(p).unwrap_or_else(|| panic!("{p} emitted")).clone()).unwrap()
    };

    let spawn = f("datapack/data/hello-world/function/spawn_actor_elite.mcfunction");
    let unleash = f("datapack/data/hello-world/function/unleash_elite.mcfunction");
    for (what, body) in [("puppet", &spawn), ("twin", &unleash)] {
        assert!(
            body.contains(
                "mainhand:{id:\"minecraft:netherite_sword\",count:1,\
                           components:{\"minecraft:enchantments\":{\"minecraft:sharpness\":5}}}"
            ),
            "{what} must carry the enchanted sword:\n{body}"
        );
        assert!(
            body.contains("drop_chances:{mainhand:0.0f,head:0.0f}"),
            "{what} must drop nothing:\n{body}"
        );
    }

    let pt = f("packtest-datapack/data/hello-world/test/v06_actor_equipment.mcfunction");
    assert!(
        pt.contains("function hello-world:spawn_actor_elite"),
        "{pt}"
    );
    assert!(pt.contains("function hello-world:unleash_elite"), "{pt}");
    // Both halves of the handoff are asserted, on the puppet and on the twin.
    assert!(
        pt.contains("tag=dw_pup_elite") && pt.contains("tag=!dw_pup_elite"),
        "the packtest must probe the puppet AND the twin:\n{pt}"
    );
    assert_eq!(
        pt.matches("assert score").count(),
        2,
        "two assertions:\n{pt}"
    );
}

/// No equipped actor -> no equipment PackTest (byte-identity for old campaigns).
#[test]
fn an_unequipped_campaign_emits_no_equipment_packtest() {
    let out = try_build("").expect("builds");
    assert!(
        !out.keys().any(|k| k.contains("v06_actor_equipment")),
        "the equipment packtest must only appear when an actor is equipped"
    );
}
