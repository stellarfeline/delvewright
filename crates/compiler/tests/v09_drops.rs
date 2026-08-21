//! DSL v0.9 declared-drop emission.
//!
//! What this pins down, against the pinned 1.21.11 NBT surface:
//!
//! * a **declared** slot carries drop chance `2.0f` — vanilla's own
//!   `DropChances.withGuaranteedDrop` constant, which is also the only value
//!   that makes the drop deterministic (`isPreserved` = `chance > 1.0f`, and a
//!   preserved slot skips the durability randomization a chance of `≤ 1.0`
//!   applies, and drops even when the killing blow was not a player's);
//! * every **undeclared** slot keeps `0.0f`, exactly as before v0.9 — an
//!   ordinary kit is never farmable;
//! * a quest-item drop rides the entity's own `DeathLootTable`, pointed at a
//!   compiler-emitted single-roll, single-entry table;
//! * a removal the COMPILER performs (unleash, despawn) strips the declaration
//!   off the body first, so only a player's kill yields the prize;
//! * a campaign that declares no drops emits no `loot_table` directory, no strip
//!   line and the same `0.0f` chances it always did.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};

/// A v0.9 hello-world quests doc. `{wave_extra}` splices into the boss wave's
/// single mob; `{actors}` splices an optional `actors` section, `{collect}` an
/// optional drop-gated collect objective before the exit.
fn quests_doc(mob: &str, actors: &str, collect: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.9.0",
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
          {collect}
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
    {actors}
    "waves": [
      {{
        "id": "wave/ambush",
        "anchor": "anchor/keeper-stand",
        "tier": "boss",
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
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
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
        prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn build_hw(mob: &str, actors: &str, collect: &str) -> BuildOutput {
    let c = parse_hw(&quests_doc(mob, actors, collect));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&c, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "campaign must validate clean: {diags:#?}");
    build(&c, &prefabs)
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(out.get(path).unwrap_or_else(|| panic!("{path} emitted"))).unwrap()
}

fn spawn_fn(out: &BuildOutput) -> &str {
    text(
        out,
        "datapack/data/hello-world/function/spawn_ambush.mcfunction",
    )
}

/// The bell remake's cliff elite: wears helm and sword, leaves **only** the
/// sword. The declared slot carries `2.0f`; the helm keeps `0.0f`.
#[test]
fn declared_slot_is_guaranteed_and_the_rest_stays_zero() {
    let out = build_hw(
        r#"{ "entity": "minecraft:zombie", "count": 1,
             "equipment": { "head": "minecraft:iron_helmet",
                            "main_hand": {"item": "minecraft:iron_sword",
                                          "enchantments": {"minecraft:knockback": 2}} },
             "drops": [ { "slot": "main_hand" } ] }"#,
        "",
        "",
    );
    let spawn = spawn_fn(&out);
    assert!(
        spawn.contains("drop_chances:{mainhand:2.0f,head:0.0f}"),
        "the declared slot must be guaranteed (2.0f) and the rest zero:\n{spawn}"
    );
    // Never 1.0f: at exactly 1.0 vanilla still rolls the item's durability down,
    // so the "deterministic drop" would be a die-roll of remaining damage.
    assert!(
        !spawn.contains("mainhand:1.0f"),
        "1.0f is not the guaranteed-drop value:\n{spawn}"
    );
}

/// No `drops[]` → the pre-0.9 string, every slot at `0.0f`, and no loot table
/// anywhere in the datapack.
#[test]
fn undeclared_drops_are_byte_identical_to_pre_0_9() {
    let out = build_hw(
        r#"{ "entity": "minecraft:zombie", "count": 1,
             "equipment": { "head": "minecraft:iron_helmet", "main_hand": "minecraft:iron_sword" } }"#,
        "",
        "",
    );
    let spawn = spawn_fn(&out);
    assert!(
        spawn.contains("drop_chances:{mainhand:0.0f,head:0.0f}"),
        "a wave that declares no drops must keep every chance at 0.0f:\n{spawn}"
    );
    assert!(
        !spawn.contains("DeathLootTable"),
        "a wave mob without a quest-item drop keeps vanilla's own table:\n{spawn}"
    );
    assert!(
        out.keys().all(|k| !k.contains("/loot_table/")),
        "no drop declared → no loot_table directory: {:?}",
        out.keys().collect::<Vec<_>>()
    );
}

/// The quest-token half: a declared `{item}` drop points the mob's own
/// `DeathLootTable` at a compiler-emitted table with one roll and one entry, and
/// the declared display name lands as the same `custom_name` component a
/// container-provided quest item carries.
#[test]
fn quest_item_drop_rides_the_death_loot_table() {
    let out = build_hw(
        r#"{ "entity": "minecraft:zombie", "count": 1,
             "drops": [ { "item": "minecraft:tripwire_hook", "name": "Gate Key" } ] }"#,
        "",
        "",
    );
    let spawn = spawn_fn(&out);
    assert!(
        spawn.contains("DeathLootTable:\"hello-world:dw_drop/wave_ambush_0\""),
        "the mob must point at its emitted table:\n{spawn}"
    );
    let table = text(
        &out,
        "datapack/data/hello-world/loot_table/dw_drop/wave_ambush_0.json",
    );
    let v: serde_json::Value = serde_json::from_str(table).expect("loot table is json");
    assert_eq!(v["type"], "minecraft:entity");
    assert_eq!(v["pools"][0]["rolls"], 1);
    assert_eq!(v["pools"][0]["entries"][0]["type"], "minecraft:item");
    assert_eq!(
        v["pools"][0]["entries"][0]["name"],
        "minecraft:tripwire_hook"
    );
    let f = &v["pools"][0]["entries"][0]["functions"][0];
    assert_eq!(f["function"], "minecraft:set_name");
    assert_eq!(f["target"], "custom_name");
    assert_eq!(f["name"]["text"], "Gate Key");
}

const BOSS_ACTOR: &str = r#""actors": [
      { "id": "actor/warden", "entity": "minecraft:wither_skeleton",
        "anchor": "anchor/keeper-stand", "tier": "boss", "vulnerable": true,
        "equipment": { "head": "minecraft:iron_helmet", "main_hand": "minecraft:iron_sword" },
        "drops": [ { "slot": "head" }, { "item": "minecraft:tripwire_hook", "name": "Barrow Seal" } ] }
    ],"#;

/// The barrow warden: wears a helm and a sword, leaves **only** the helm — on
/// the puppet and on the unleashed twin alike, because the drop belongs to the
/// body rather than to one of its two lifecycles.
#[test]
fn actor_drops_ride_both_bodies() {
    let out = build_hw(
        r#"{ "entity": "minecraft:zombie", "count": 1 }"#,
        BOSS_ACTOR,
        "",
    );
    let spawn = text(
        &out,
        "datapack/data/hello-world/function/spawn_actor_warden.mcfunction",
    );
    let unleash = text(
        &out,
        "datapack/data/hello-world/function/unleash_warden.mcfunction",
    );
    for (what, body) in [("puppet", spawn), ("twin", unleash)] {
        assert!(
            body.contains("drop_chances:{mainhand:0.0f,head:2.0f}"),
            "{what}: only the declared helm is guaranteed:\n{body}"
        );
        assert!(
            body.contains("DeathLootTable:\"hello-world:dw_drop/actor_warden\""),
            "{what}: the quest token rides the death loot table:\n{body}"
        );
    }
}

/// A removal the compiler performs is not a death the player earned: `unleash`
/// strips the cage's declaration before killing it, so standing the elite up
/// never showers the party with its own helm.
#[test]
fn unleash_strips_the_cage_before_killing_it() {
    let out = build_hw(
        r#"{ "entity": "minecraft:zombie", "count": 1 }"#,
        BOSS_ACTOR,
        "",
    );
    let unleash = text(
        &out,
        "datapack/data/hello-world/function/unleash_warden.mcfunction",
    );
    let strip = unleash
        .lines()
        .position(|l| l.contains("data merge entity @s") && l.contains("drop_chances"))
        .expect("the strip line is emitted");
    let kill = unleash
        .lines()
        .position(|l| l.starts_with("kill @e[tag=dw_pup_warden]"))
        .expect("the cage is killed");
    assert!(strip < kill, "the strip must precede the kill:\n{unleash}");
    let line = unleash.lines().nth(strip).unwrap();
    assert!(
        line.contains("mainhand:0.0f") && line.contains("head:0.0f"),
        "the strip zeroes every slot:\n{line}"
    );
    assert!(
        line.contains("DeathLootTable:\"minecraft:empty\""),
        "the strip empties the death loot table too:\n{line}"
    );
}

/// A drop-gated `collect` provisions nothing: no chest is placed and no fill is
/// written, because the item exists only once the fight is over. Its
/// critical-path step names the wave instead and points at that wave's ground.
#[test]
fn drop_gated_collect_places_no_container() {
    let collect = r#"{ "type": "collect", "id": "obj/key", "item": "minecraft:tripwire_hook",
             "count": 1, "anchor": "anchor/exit", "dropped_by": "wave/ambush",
             "after": ["obj/clear"] },"#;
    let out = build_hw(
        r#"{ "entity": "minecraft:zombie", "count": 1,
             "drops": [ { "item": "minecraft:tripwire_hook", "name": "Gate Key" } ] }"#,
        "",
        collect,
    );
    // Nothing anywhere in the datapack conjures a chest for this item or fills
    // one with it: the fight is the provisioning.
    for (path, bytes) in out.iter() {
        if !path.starts_with("datapack/") {
            continue;
        }
        let body = String::from_utf8_lossy(bytes);
        assert!(
            !body.contains("item replace block"),
            "a drop-gated collect fills no container ({path}):\n{body}"
        );
        assert!(
            !body.contains("minecraft:chest"),
            "a drop-gated collect places no chest ({path}):\n{body}"
        );
    }
    let cp: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").expect("cp emitted")).unwrap();
    let step = cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/key")
        .expect("the collect step is on the path");
    assert_eq!(step["action"], "collect");
    assert_eq!(step["dropped_by"], "wave/ambush");
}

/// The whole surface is deterministic: the same DSL builds byte-identically
/// twice, file for file (ADR-0006).
#[test]
fn declared_drops_build_deterministically() {
    let mob = r#"{ "entity": "minecraft:zombie", "count": 1,
             "equipment": { "head": "minecraft:iron_helmet", "main_hand": "minecraft:iron_sword" },
             "drops": [ { "slot": "main_hand" },
                        { "item": "minecraft:tripwire_hook", "name": "Gate Key" } ] }"#;
    let a = build_hw(mob, BOSS_ACTOR, "");
    let b = build_hw(mob, BOSS_ACTOR, "");
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "the emitted file set must be stable"
    );
    for (k, v) in a.iter() {
        assert_eq!(b.get(k.as_str()), Some(v), "file `{k}` is not reproducible");
    }
}
