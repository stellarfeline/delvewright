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
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
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

/// The same build, keeping the failure instead of unwrapping it.
fn try_build(triggers: &str) -> Result<BuildOutput, emit::BuildFailure> {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests_doc(triggers),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
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
        line.contains(r#"Tags:["dw_borne","dw_npc_keeper","dw_trig_wake"]"#),
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
                && l.contains(r#"Tags:["dw_fixture","dw_trig_ward"]"#)),
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
        line.ends_with(r#"Tags:["dw_borne","dw_npc_keeper"]}"#),
        "an unrelated strike trigger must not touch the npc hitbox:\n{line}"
    );
}

/// Scope: right-click on an NPC already belongs to the dialogue advancement, so a
/// co-located `use` trigger is an authoring conflict, not a detection bug — it
/// does not share the hitbox and it does not ship.
///
/// The conflict is rejected twice over, and this pins the second: `DW0350`
/// (validate tier, symbolic — same anchor name) is what the CLI reaches first,
/// and `DW0359` (build tier, geometric — the trigger's own interaction entity
/// sits inside the NPC's body box) catches it here, where a caller reaching
/// `emit::build` directly bypasses validation. Before round-7 this test asserted
/// the *emitted* hitbox line instead; the campaign no longer emits at all, which
/// is the stronger statement of the same rule (one cell, one hitbox) and the
/// reason the assertion moved.
#[test]
fn use_trigger_on_an_npc_anchor_is_rejected() {
    let use_trigger = r#"{
      "id": "trigger/press",
      "at": "anchor/keeper-stand",
      "on": { "on": "use" },
      "once": true,
      "effects": [ { "type": "narrate", "style": "chat", "text": "He nods." } ]
    }"#;
    let err = try_build(use_trigger).expect_err("a `use` trigger in the NPC's cell must not ship");
    let emit::BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic, got {err:?}");
    };
    assert_eq!(code, "DW0359", "{message}");
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

// ---------------------------------------------------------------------------
// `on: strike-npc` (DSL v0.6) — the first-class body-targeting form
// ---------------------------------------------------------------------------

/// A `strike-npc` trigger targeting `npc/keeper`, spelled with **no** `at`: its
/// target is a character, not a cell.
const STRIKE_NPC: &str = r#"{
  "id": "trigger/wake",
  "on": { "on": "strike-npc", "npc": "npc/keeper" },
  "once": true,
  "effects": [ { "type": "narrate", "style": "chat", "text": "He stirs." } ]
}"#;

/// The v0.6 spelling routes exactly like the pre-0.6 collision it replaces: the
/// NPC's own interaction hitbox carries the trigger's tag, and nothing else is
/// summoned. It is the *only* form that can express "hit the giant" — a
/// `strike` at a world anchor summons a second entity the giant's body
/// eclipses (`DW0359`), whereas this one has no cell to be eclipsed.
#[test]
fn strike_npc_rides_the_npc_hitbox_and_summons_nothing() {
    let out = build(STRIKE_NPC);
    let line = npc_hitbox_line(&out);
    assert!(
        line.ends_with(r#"Tags:["dw_borne","dw_npc_keeper","dw_trig_wake"]}"#),
        "the NPC's hitbox must carry the strike-npc trigger's tag:\n{line}"
    );
    let standalone = all_functions(&out)
        .lines()
        .filter(|l| l.starts_with("summon minecraft:interaction ") && l.contains("dw_trig_wake"))
        .filter(|l| !l.contains("dw_npc_keeper"))
        .count();
    assert_eq!(
        standalone, 0,
        "a strike-npc trigger must summon no entity of its own"
    );
}

/// `strike-npc` needs no anchor to work, so it keeps working when the NPC is
/// nowhere near any authored trigger anchor — the property that makes it the
/// fix rather than a rename. The tick's detection is the same single selector
/// reading the `attack` record; the dialogue's right-click record
/// (`interaction`) is a different field on the same entity and is never
/// consumed by it.
#[test]
fn strike_npc_detection_reads_attack_and_leaves_interaction_alone() {
    let fns = all_functions(&build(STRIKE_NPC));
    assert!(
        fns.contains("if entity @e[tag=dw_trig_wake,nbt={attack:{}}]"),
        "strike-npc must fire off the `attack` record:\n{fns}"
    );
    assert!(
        fns.contains("execute as @e[tag=dw_trig_wake] run data remove entity @s attack"),
        "…and consume exactly that record:\n{fns}"
    );
    assert!(
        !fns.contains("dw_trig_wake] run data remove entity @s interaction"),
        "the right-click record belongs to the NPC's dialogue and must be left alone:\n{fns}"
    );
}

/// The PackTest the collision emits now covers `strike-npc` too, including the
/// separability leg: a right-click record on the shared hitbox must leave the
/// left-click trigger unfired.
#[test]
fn strike_npc_packtest_proves_the_two_click_streams_are_separate() {
    let out = build(STRIKE_NPC);
    let path = format!("packtest-datapack/data/{NS}/test/v04_strike_npc.mcfunction");
    let body = std::str::from_utf8(out.get(&path).expect("strike packtest emitted")).unwrap();
    assert!(
        body.contains("tag=dw_npc_keeper,tag=dw_trig_wake"),
        "packtest must assert the routing:\n{body}"
    );
    assert!(
        body.contains("interaction set value {player:[I;0,0,0,0],timestamp:1L}"),
        "packtest must drive a right-click record:\n{body}"
    );
    let after_rc = body
        .split_once("interaction set value")
        .expect("right-click leg present")
        .1;
    assert!(
        after_rc.contains("assert score #rc_stnp dw.sys matches 0")
            && after_rc.contains("assert score #trig_wake dw.sys matches 0"),
        "…and prove the right-click neither wrote `attack` nor fired the trigger:\n{body}"
    );
}
