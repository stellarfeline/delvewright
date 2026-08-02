//! `strike` triggers on an NPC's anchor (round-4 + round-6 island QA).
//!
//! A `strike` trigger reads the `attack` record off a `minecraft:interaction`
//! entity. When the trigger's anchor is also where an NPC stands, the NPC's
//! hitbox is the entity a click actually reaches, and the NPC's body is
//! `Invulnerable` — so a trigger listening on an entity of its own could simply
//! never fire (round-4). Round-4 shared the trigger's tag onto the NPC hitbox
//! but kept both entities; the two exactly co-located hitboxes then made the
//! *right*-click pick ambiguous, and when the standalone won, the dialogue
//! advancement never fired (round-6 soft-lock). The NPC's hitbox is now the
//! trigger's SOLE carrier: one cell, one hitbox.

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
        world_edits: None,
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

/// One cell, one hitbox (round-6 island QA): the NPC's hitbox is the trigger's
/// SOLE carrier — the trigger's own world-init summon is suppressed. Two exactly
/// co-located interaction entities made the client's right-click pick ambiguous;
/// when the standalone won the tie, the dialogue advancement (keyed on
/// `Tags:["dw_npc_<n>"]`) never fired and the delve soft-locked (Polyphemus
/// untalkable after the boulder seal, proven on a live server: the click landed
/// on the world-init entity, the same click on the NPC's hitbox opened the
/// dialog).
#[test]
fn strike_trigger_on_an_npc_anchor_summons_no_standalone_hitbox() {
    let all = all_functions(&build(STRIKE_ON_NPC));
    for line in all.lines() {
        if line.starts_with("summon minecraft:interaction") && line.contains("dw_trig_wake") {
            assert!(
                line.contains("dw_npc_keeper"),
                "the strike trigger must not get an interaction entity of its own — \
                 only the NPC's hitbox may wear its tag:\n{line}"
            );
        }
    }
    assert_eq!(
        all.lines()
            .filter(|l| l.starts_with("summon minecraft:interaction") && l.contains("dw_trig_wake"))
            .count(),
        1,
        "exactly one summon wears the trigger tag (the NPC hitbox):\n{all}"
    );
}

/// A strike trigger with no NPC on its anchor keeps its own hitbox — there is
/// nothing else at that cell for the click to be stolen from.
#[test]
fn strike_trigger_off_an_npc_anchor_keeps_its_standalone_hitbox() {
    let away = r#"{
      "id": "trigger/ward",
      "at": "anchor/exit",
      "on": { "on": "strike" },
      "once": true,
      "effects": [ { "type": "narrate", "style": "chat", "text": "Wards flare." } ]
    }"#;
    let all = all_functions(&build(away));
    assert!(
        all.lines()
            .any(|l| l.starts_with("summon minecraft:interaction")
                && l.contains(r#"Tags:["dw_trig_ward"]"#)),
        "an off-NPC strike trigger still summons its own hitbox:\n{all}"
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
        "exactly one consume line (it clears the sole, shared hitbox):\n{all}"
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
/// does not share the hitbox. (Since round-6 the conflict is rejected outright at
/// validate time, `DW0350`; emission never sees it through the CLI. This pins
/// the emission-level scoping regardless.)
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
        body.contains("assert score #rec_stnp dw.sys matches 0")
            && body.contains("assert score #trig_wake dw.sys matches 0"),
        "packtest must assert the record was consumed and cannot re-fire:\n{body}"
    );
    // Batch model: `setup_finish` summons are unguarded and the world init has
    // already run them, so the test must clear every planned NPC tag before its
    // own `setup_finish` — else duplicated hitboxes break the exact-count
    // routing assert.
    let clear = body
        .find("kill @e[tag=dw_npc_keeper]")
        .expect("clears the NPC tag before re-running setup_finish");
    let finish = body.find(":setup_finish").expect("runs setup_finish");
    assert!(
        clear < finish,
        "the NPC-tag clear must precede setup_finish:\n{body}"
    );
}

/// No co-located strike trigger → no PackTest (and no behaviour to test).
#[test]
fn no_strike_packtest_without_the_collision() {
    let out = build("");
    for t in ["v04_strike_npc", "v04_strike_talk"] {
        let path = format!("packtest-datapack/data/{NS}/test/{t}.mcfunction");
        assert!(
            !out.contains_key(&path),
            "{t} is emitted only for the collision"
        );
    }
}

/// The round-6 regression PackTest: after the NPC is in place — and again after
/// an attack record lands and a tick consumes it — the NPC's hitbox is the ONE
/// interaction entity wearing the trigger's tag, and no entity wears it without
/// also being the NPC's. The pre-fix emission (standalone summon + shared tag)
/// trips both counts.
#[test]
fn strike_talk_packtest_pins_the_single_hitbox_invariant() {
    let out = build(STRIKE_ON_NPC);
    let path = format!("packtest-datapack/data/{NS}/test/v04_strike_talk.mcfunction");
    let body = std::str::from_utf8(out.get(&path).expect("strike-talk packtest emitted")).unwrap();
    assert!(
        body.contains("if entity @e[type=minecraft:interaction,tag=dw_trig_wake]")
            && body.contains("assert score #one_stlk dw.sys matches 1"),
        "exactly one entity wears the trigger tag:\n{body}"
    );
    assert!(
        body.contains(
            "if entity @e[type=minecraft:interaction,tag=dw_trig_wake,tag=!dw_npc_keeper]"
        ) && body.contains("assert score #orph_stlk dw.sys matches 0"),
        "no orphan (non-NPC) entity wears the trigger tag:\n{body}"
    );
    assert!(
        body.contains("attack set value {player:[I;0,0,0,0],timestamp:1L}"),
        "the owner's left-click is simulated:\n{body}"
    );
    // The post-attack re-check comes after the record lands, and the template
    // consumes the record itself (running `tick` could fire the trigger's
    // content effects when a sibling's dummy holds its gate flag — batch
    // templates are interleaving-independent).
    let attack = body.find("attack set value").unwrap();
    let recheck = body
        .rfind("assert score #one2_stlk dw.sys matches 1")
        .expect("re-asserts the single click target after the attack");
    assert!(
        recheck > attack,
        "the single-hitbox invariant is re-asserted after the attack lands:\n{body}"
    );
    assert!(
        !body.contains(":tick"),
        "no full tick — sibling dummies may arm the trigger's flag gate:\n{body}"
    );
    assert!(
        body.contains("run data remove entity @s attack"),
        "the hand-written record is consumed (no residue):\n{body}"
    );
}
