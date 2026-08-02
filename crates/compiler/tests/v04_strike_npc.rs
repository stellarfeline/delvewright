//! `strike` triggers on an NPC's anchor (round-4 island QA).
//!
//! A `strike` trigger reads the `attack` record off a `minecraft:interaction`
//! entity. When the trigger's anchor is also where an NPC stands, two
//! interaction entities occupy the cell and a left-click reaches exactly one of
//! them — whichever the attack raycast finds first, which the compiler does not
//! control. The NPC's body is `Invulnerable`, so a swing that lands on the NPC's
//! hitbox used to be recorded where nothing was watching, and the trigger never
//! fired. The NPC's hitbox now wears the trigger's tag, so the trigger's single
//! selector watches both entities.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{RawCampaign, parse_campaign};

const NS: &str = "hello-world";

/// A v0.4 quests document with the hello-world quest plus `triggers`.
fn quests_doc(triggers: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.4.0",
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
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "triggers": [ {triggers} ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn build(triggers: &str) -> BuildOutput {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests_doc(triggers),
        dialogue: read_hw("dialogue.json"),
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
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
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

/// Every shipped-datapack function body concatenated.
fn all_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

/// The one `summon minecraft:interaction` line that places the NPC's hitbox.
fn npc_hitbox_line(out: &BuildOutput) -> String {
    all_functions(out)
        .lines()
        .find(|l| l.starts_with("summon minecraft:interaction ") && l.contains("dw_npc_keeper"))
        .expect("npc hitbox summon emitted")
        .to_string()
}

const STRIKE_ON_NPC: &str = r#"{
  "id": "trigger/wake",
  "at": "anchor/keeper-stand",
  "on": { "on": "strike" },
  "once": true,
  "effects": [ { "type": "narrate", "style": "chat", "text": "He stirs." } ]
}"#;

/// The routing fix: the NPC's interaction hitbox carries the co-located strike
/// trigger's tag, so the trigger's existing selector reaches the entity a
/// left-click actually lands on.
#[test]
fn npc_hitbox_wears_the_colocated_strike_trigger_tag() {
    let line = npc_hitbox_line(&build(STRIKE_ON_NPC));
    assert!(
        line.contains(r#"Tags:["dw_npc_keeper","dw_trig_wake"]"#),
        "npc hitbox must carry the strike trigger's tag:\n{line}"
    );
}

/// Detection itself is untouched — one selector, one consume line. The fix lives
/// entirely in which entities wear the tag.
#[test]
fn strike_detection_stays_a_single_selector() {
    let all = all_functions(&build(STRIKE_ON_NPC));
    assert_eq!(
        all.matches("if entity @e[tag=dw_trig_wake,nbt={attack:{}}]")
            .count(),
        1,
        "exactly one detection line:\n{all}"
    );
    assert_eq!(
        all.matches("execute as @e[tag=dw_trig_wake] run data remove entity @s attack")
            .count(),
        1,
        "exactly one consume line (it now clears both hitboxes):\n{all}"
    );
}

/// A strike trigger somewhere an NPC does not stand leaves the NPC hitbox
/// exactly as before — no collision, no tag, byte-identical output.
#[test]
fn strike_trigger_away_from_an_npc_changes_nothing() {
    let away = r#"{
      "id": "trigger/ward",
      "at": "anchor/exit",
      "on": { "on": "strike" },
      "once": true,
      "effects": [ { "type": "narrate", "style": "chat", "text": "Wards flare." } ]
    }"#;
    let line = npc_hitbox_line(&build(away));
    assert!(
        line.ends_with(r#"Tags:["dw_npc_keeper"]}"#),
        "an unrelated strike trigger must not touch the npc hitbox:\n{line}"
    );
}

/// Scope: right-click on an NPC already belongs to the dialogue advancement, so a
/// co-located `use` trigger is an authoring conflict, not a detection bug — it
/// does not share the hitbox.
#[test]
fn use_trigger_on_an_npc_anchor_is_left_alone() {
    let use_trigger = r#"{
      "id": "trigger/press",
      "at": "anchor/keeper-stand",
      "on": { "on": "use" },
      "once": true,
      "effects": [ { "type": "narrate", "style": "chat", "text": "He nods." } ]
    }"#;
    let line = npc_hitbox_line(&build(use_trigger));
    assert!(
        line.ends_with(r#"Tags:["dw_npc_keeper"]}"#),
        "a `use` trigger must not share the npc hitbox:\n{line}"
    );
}

/// The generated PackTest writes the vanilla `attack` record onto the NPC's own
/// hitbox — exactly what a left-click produces — and proves the trigger fires,
/// once, and that the record is consumed.
#[test]
fn strike_packtest_drives_the_record_and_asserts_one_fire() {
    let out = build(STRIKE_ON_NPC);
    let path = format!("packtest-datapack/data/{NS}/test/v04_strike_npc.mcfunction");
    let body = std::str::from_utf8(out.get(&path).expect("strike packtest emitted")).unwrap();
    assert!(
        body.contains("tag=dw_npc_keeper,tag=dw_trig_wake"),
        "packtest must assert the routing:\n{body}"
    );
    assert!(
        body.contains("attack set value {player:[I;0,0,0,0],timestamp:1L}"),
        "packtest must simulate the vanilla left-click record:\n{body}"
    );
    assert!(
        body.contains("assert score #trig_wake dw.sys matches 1"),
        "packtest must assert the trigger fired:\n{body}"
    );
    assert!(
        body.contains("assert score #rec dw.sys matches 0")
            && body.contains("assert score #trig_wake dw.sys matches 0"),
        "packtest must assert the record was consumed and cannot re-fire:\n{body}"
    );
}

/// No co-located strike trigger → no PackTest (and no behaviour to test).
#[test]
fn no_strike_packtest_without_the_collision() {
    let out = build("");
    let path = format!("packtest-datapack/data/{NS}/test/v04_strike_npc.mcfunction");
    assert!(
        !out.contains_key(&path),
        "the strike packtest is emitted only for the collision"
    );
}
