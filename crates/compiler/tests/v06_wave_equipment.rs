//! DSL v0.6 wave-mob `equipment` emission (task #65): explicit slots land in the
//! summon NBT as the component-era `equipment`/`drop_chances` compounds (1.21.11
//! silently ignores legacy `ArmorItems`/`HandItems` on `/summon`), every emitted
//! slot at drop chance 0 (no-grind: wave gear is never lootable). Explicit
//! `main_hand` overrides the armed-mob default; a mob without the field is
//! byte-identical to the pre-equipment emission.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};

/// A v0.6 hello-world quests doc: obj/talk spawns the wave, a kill objective
/// clears it, then the exit. `{mob}` is the one wave-mob JSON object under test.
fn quests_doc(mob: &str) -> String {
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
          {{ "type": "kill", "id": "obj/clear", "wave": "wave/ambush", "after": ["obj/talk"] }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/clear"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "spawn-wave", "wave": "wave/ambush" }},
                        {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "waves": [
      {{
        "id": "wave/ambush",
        "anchor": "anchor/keeper-stand",
        "mobs": [ {mob} ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
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
        prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

/// Build the hello-world campaign with the given wave mob; the campaign must
/// first validate clean against the FULL 1.21.11 registries (equipment items
/// resolve through the same item registry `give-item` uses).
fn build_hw(mob: &str) -> BuildOutput {
    let c = parse_hw(&quests_doc(mob));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&c, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "campaign must validate clean: {diags:#?}");
    build(&c, &prefabs)
}

fn spawn_fn(out: &BuildOutput) -> &str {
    let path = "datapack/data/hello-world/function/spawn_ambush.mcfunction";
    std::str::from_utf8(out.get(path).expect("spawn_ambush emitted")).unwrap()
}

/// Armor slots land as component-era `equipment` with per-slot zero
/// `drop_chances` — the helmeted zombie survives daylight and drops nothing.
#[test]
fn equipment_emits_component_form_with_zero_drop_chances() {
    let out = build_hw(
        r#"{ "entity": "minecraft:zombie", "count": 1,
             "equipment": { "head": "minecraft:iron_helmet", "chest": "minecraft:leather_chestplate" } }"#,
    );
    let spawn = spawn_fn(&out);
    assert!(
        spawn.contains(
            "equipment:{head:{id:\"minecraft:iron_helmet\",count:1},\
             chest:{id:\"minecraft:leather_chestplate\",count:1}},\
             drop_chances:{head:0.0f,chest:0.0f}"
        ),
        "expected component-era equipment with zero drop chances; got:\n{spawn}"
    );
    // Never the legacy summon NBT (silently ignored by 1.21.11).
    assert!(
        !spawn.contains("ArmorItems") && !spawn.contains("HandItems"),
        "legacy ArmorItems/HandItems must never be emitted:\n{spawn}"
    );
}

/// A helmeted skeleton keeps its default bow: explicit slots merge OVER the
/// armed-mob main-hand default rather than replacing it.
#[test]
fn armor_merges_over_default_mainhand() {
    let out = build_hw(
        r#"{ "entity": "minecraft:skeleton", "count": 1,
             "equipment": { "head": "minecraft:golden_helmet" } }"#,
    );
    let spawn = spawn_fn(&out);
    assert!(
        spawn.contains(
            "equipment:{mainhand:{id:\"minecraft:bow\",count:1},\
             head:{id:\"minecraft:golden_helmet\",count:1}},\
             drop_chances:{mainhand:0.0f,head:0.0f}"
        ),
        "helmeted skeleton must keep its default bow; got:\n{spawn}"
    );
}

/// An explicit `main_hand` overrides the armed-mob default.
#[test]
fn explicit_main_hand_overrides_default() {
    let out = build_hw(
        r#"{ "entity": "minecraft:wither_skeleton", "count": 1,
             "equipment": { "main_hand": "minecraft:iron_sword" } }"#,
    );
    let spawn = spawn_fn(&out);
    assert!(
        spawn.contains("equipment:{mainhand:{id:\"minecraft:iron_sword\",count:1}}"),
        "explicit main_hand must override the stone-sword default; got:\n{spawn}"
    );
    assert!(
        !spawn.contains("minecraft:stone_sword"),
        "the overridden default must not leak; got:\n{spawn}"
    );
}

/// A wave mob WITHOUT the field emits exactly the pre-equipment default
/// fragment (byte-identity for existing campaigns), and the whole build is
/// deterministic: building twice yields identical bytes (ADR-0006).
#[test]
fn no_equipment_is_byte_identical_default_and_deterministic() {
    let mob = r#"{ "entity": "minecraft:skeleton", "count": 1 }"#;
    let out = build_hw(mob);
    let spawn = spawn_fn(&out);
    assert!(
        spawn.contains(
            "equipment:{mainhand:{id:\"minecraft:bow\",count:1}},drop_chances:{mainhand:0.0f}"
        ),
        "equipment-less skeleton must keep the exact M2-fix-5 default fragment; got:\n{spawn}"
    );

    let again = build_hw(mob);
    assert_eq!(out, again, "same DSL must build byte-identical output");
}
