//! DSL v0.8 `close-gate` seal answers (task #142 — owner island finding #34).
//!
//! The finding: right-clicking a sealed boulder answered with **silence**, three
//! rounds and two playtests running. A seal is a wall the party walks back to and
//! presses; the compiler now arms that wall to answer, with the authored
//! `sealed_hint` or its own canonical English.
//!
//! Covered here: the answer exists at all (the finding's own red), the authored
//! wording overrides the default, re-opening the gate takes the answer down, a
//! click trigger anchored on the gate rides the seal's hitboxes instead of
//! summoning a second co-located one, and the two new diagnostics (`DW0422`
//! hitbox collision, `DW0423` two wordings for one gate).

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::gates;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

/// A hello-world `quests` doc at `dsl_version`, opening `anchor/door` on the talk
/// objective and running `on_complete` after the exit is reached — where a
/// `close-gate` seals nothing still to be walked.
fn quests_doc(version: &str, on_complete: &str) -> String {
    quests_doc_with(version, on_complete, "")
}

/// As [`quests_doc`], plus a raw `triggers` array body (no surrounding brackets).
fn quests_doc_with(version: &str, on_complete: &str, triggers: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}",
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
        "on_complete": [ {on_complete} ]
      }}
    ],
    "triggers": [ {triggers} ]
  }}
}}"#
    )
}

/// A hello-world `quests` doc whose ONLY `close-gate` lives in a `traps[]`
/// payload — an effect root the quests stage owns but the older gate scans skip.
fn quests_doc_trap_payload() -> String {
    r#"{
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
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "traps": [
      {
        "id": "trap/spring-the-door",
        "at": "anchor/exit",
        "trigger": "trapped-chest",
        "lethality": "harmful",
        "payload": [ { "type": "close-gate", "anchor": "anchor/door" } ]
      }
    ]
  }
}"#
    .to_string()
}

/// A hello-world `dialogue` doc whose ONLY `close-gate` lives in a dialogue
/// option's `set-checkpoint` `on_respawn` bundle — a `Vec<QuestEffect>` hanging
/// off the *dialogue* stage, which `emit_quest_effect` really does lower (into
/// `cp_on_respawn_<i>`).
const DIALOGUE_SEALS_ON_RESPAWN: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
        { "id": "dlg/greeting",
          "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
          "options": [
            { "label": "Open the door, please.",
              "effects": [
                { "type": "complete-objective", "objective": "obj/talk" },
                { "type": "set-checkpoint", "anchor": "anchor/exit",
                  "on_respawn": [ { "type": "close-gate", "anchor": "anchor/door" } ] }
              ] }
          ] }
      ] }
    ]
  }
}"#;

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw_with_dialogue(quests: &str, dialogue: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue.to_string(),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
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

fn try_build(campaign: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, BuildFailure> {
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
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
    try_build(campaign, prefabs).expect("every emitted command validates")
}

/// One emitted `.mcfunction` body, by unqualified name.
fn function(out: &BuildOutput, name: &str) -> String {
    let suffix = format!("/function/{name}.mcfunction");
    out.iter()
        .find(|(p, _)| p.starts_with("datapack/") && p.ends_with(&suffix))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no shipped function `{name}` in {:?}", out.keys()))
}

/// Every shipped `.mcfunction` body, concatenated.
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

fn advancement(out: &BuildOutput, name: &str) -> String {
    let suffix = format!("/advancement/{name}.json");
    out.iter()
        .find(|(p, _)| p.starts_with("datapack/") && p.ends_with(&suffix))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no advancement `{name}` in {:?}", out.keys()))
}

const SEAL_IT: &str = r#"{ "type": "close-gate", "anchor": "anchor/door" },
                          { "type": "campaign-complete" }"#;

// --- the finding itself: a sealed gate must not answer with silence ---------

/// **Owner island finding #34, as a machine test.** A gate the campaign seals
/// arms a right-click answer: interaction hitboxes over the sealed region, a
/// `player_interacted_with_entity` advancement watching them, and a reward
/// function that puts a line on the presser's actionbar. Before task #142 the
/// datapack had none of the three and the stone said nothing.
#[test]
fn a_sealed_gate_answers_a_right_click() {
    let c = parse_hw(&quests_doc("0.6.0", SEAL_IT));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);

    let arm = function(&out, "seal_arm_door");
    assert!(
        arm.contains("summon minecraft:interaction")
            && arm.contains("\"dw_seal_door\"")
            && arm.contains("width:1.02f"),
        "the seal must arm hitboxes that protrude past the sealed block: {arm}"
    );

    // DSL v0.11: the answer is no longer `close-gate`'s own machinery. It is the
    // synthesized press-answer trigger — an ordinary `use` trigger with
    // `audience: presser`, riding the seal's hitboxes — so what the advancement
    // watches is the TRIGGER's tag, which `seal_arm_door` puts on those same
    // entities. Same three parts, none of them keyed to the verb any more.
    assert!(
        arm.contains("\"dw_trig_dw_press_seal_door\""),
        "the press answer must ride the seal's hitboxes, not summon its own: {arm}"
    );

    let adv = advancement(&out, "press_dw_press_seal_door");
    assert!(
        adv.contains("minecraft:player_interacted_with_entity")
            && adv.contains("dw_trig_dw_press_seal_door")
            && adv.contains("press_dw_press_seal_door"),
        "a right-click on the seal must dispatch the answer as the clicking player: {adv}"
    );

    let dispatch = function(&out, "press_dw_press_seal_door");
    assert!(
        dispatch.contains("advancement revoke @s only"),
        "the stone answers EVERY press, not only the first: {dispatch}"
    );

    let hint = function(&out, "trig_dw_press_seal_door");
    assert!(
        hint.contains("title @s actionbar") && hint.contains("The way is sealed."),
        "the answer must reach the presser's actionbar: {hint}"
    );
}

/// **The engine does not talk over the campaign.** Once the author answers the
/// press at that anchor themselves, the compiler supplies nothing — one press,
/// one answer. (For a `close-gate` the compiler still *may* speak; the owner's
/// 2026-08-10 ruling withdrew that licence for shortcut doors only, and
/// `plan::SilencePolicy` is where the two classes differ.)
#[test]
fn an_authored_trigger_replaces_the_compilers_seal_answer() {
    let c = parse_hw(&quests_doc_with(
        "0.11.0",
        SEAL_IT,
        r#"{ "id": "trigger/the-stone", "at": "anchor/door", "on": { "on": "use" },
             "once": false, "audience": "presser",
             "effects": [ { "type": "narrate", "style": "actionbar",
                            "text": "The stone does not care." } ] }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    let all = all_functions(&out);
    assert!(
        all.contains("The stone does not care."),
        "the author's line is what the seal says: {all}"
    );
    assert!(
        !all.contains("dw_press_seal_door"),
        "the compiler's own answer must stand down: {all}"
    );
    assert!(
        !all.contains("delvewright.ui.gate.sealed"),
        "…and its chrome must not ship either: {all}"
    );
}

/// The `close-gate` itself is what arms the answer, and it is idempotent: a beat
/// that fires twice must not stack a second set of hitboxes.
#[test]
fn closing_the_gate_arms_the_answer_idempotently() {
    let c = parse_hw(&quests_doc("0.6.0", SEAL_IT));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    assert!(
        all_functions(&out).contains(
            "execute unless entity @e[tag=dw_seal_door] run function hello-world:seal_arm_door"
        ),
        "the seal fill must arm the answer, guarded on absence: {}",
        all_functions(&out)
    );
}

/// …and re-opening it takes the answer down again. An opened threshold that still
/// says "the way is sealed" is a lie, and a hitbox left standing in a doorway
/// swallows right-clicks aimed through it.
#[test]
fn opening_the_gate_takes_the_answer_down() {
    let c = parse_hw(&quests_doc("0.6.0", SEAL_IT));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    assert!(
        all_functions(&out).contains("kill @e[tag=dw_seal_door]"),
        "`open-gate` must retire the seal's hitboxes: {}",
        all_functions(&out)
    );
}

/// An authored `sealed_hint` (v0.8) replaces the compiler's canonical English.
#[test]
fn an_authored_hint_replaces_the_canonical_english() {
    let c = parse_hw(&quests_doc(
        "0.8.0",
        r#"{ "type": "close-gate", "anchor": "anchor/door",
             "sealed_hint": "The bars will not lift for you.",
             "happening": { "verb": "seals", "text": "The bars come down." } },
           { "type": "campaign-complete",
             "happening": { "verb": "survives", "text": "The party is out." } }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    let hint = function(&out, "trig_dw_press_seal_door");
    assert!(
        hint.contains("The bars will not lift for you."),
        "the authored wording must be what the seal says: {hint}"
    );
    assert!(
        !hint.contains("The way is sealed."),
        "…instead of the default, not alongside it: {hint}"
    );
}

/// Only the region's **shell** is armed: `anchor/door` is a 2 × 3 × 1 slab, so
/// every one of its six cells is clickable and gets exactly one hitbox.
#[test]
fn the_seal_arms_one_hitbox_per_clickable_cell() {
    let c = parse_hw(&quests_doc("0.6.0", SEAL_IT));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    let arm = function(&out, "seal_arm_door");
    let summons = arm.lines().filter(|l| l.starts_with("summon ")).count();
    assert_eq!(
        summons, 6,
        "a 2 x 3 x 1 gate slab has six clickable cells: {arm}"
    );
}

// --- one cell, one hitbox --------------------------------------------------

/// A click trigger anchored on the gate **rides** the seal's hitboxes: they wear
/// its tag and `setup_finish` summons nothing for it. This is the island's
/// `trigger/boulder-wont-move` shape — the round-13 STOP was that a second,
/// exactly co-located hitbox makes the client's ray-pick a tie, so one of the two
/// silently stops receiving clicks.
#[test]
fn a_click_trigger_on_the_gate_rides_the_seal() {
    let c = parse_hw(&quests_doc_with(
        "0.6.0",
        SEAL_IT,
        r#"{ "id": "trigger/wont-budge", "at": "anchor/door", "on": { "on": "strike" },
             "once": false,
             "effects": [ { "type": "narrate", "style": "chat", "text": "It does not budge." } ] }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);

    let arm = function(&out, "seal_arm_door");
    assert!(
        arm.contains("\"dw_trig_wont_budge\""),
        "the seal's hitboxes must carry the riding trigger's tag: {arm}"
    );
    let setup = function(&out, "setup_finish");
    assert!(
        !setup.contains("dw_trig_wont_budge"),
        "…and the trigger must summon no second, co-located hitbox: {setup}"
    );
}

/// `DW0422`: any OTHER affordance inside the sealed region is a ray-pick tie with
/// the seal's own hitboxes, and which of the two dies is not decidable from the
/// campaign. The fixture adds a point anchor **inside** `anchor/door`'s region to
/// the prefab library (copied to a temp dir — the content repo is read-only) and
/// hangs a `use` trigger on it: a second interaction entity in a cell the seal
/// already answers for.
#[test]
fn dw0422_a_second_affordance_inside_the_seal() {
    let tmp = std::env::temp_dir().join("dw-seal-collision-prefabs");
    let _ = std::fs::remove_dir_all(&tmp);
    common::copy_dir_all(&common::prefabs_dir(), &tmp);
    let hello = tmp.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hello).unwrap()).unwrap();
    // `anchor/door` spans [4,1,6]..[5,3,6]; put the latch squarely in it.
    meta["anchors"]["anchor/latch"] = serde_json::json!({ "pos": [4, 1, 6] });
    std::fs::write(&hello, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let c = parse_hw(&quests_doc_with(
        "0.6.0",
        SEAL_IT,
        r#"{ "id": "trigger/latch", "at": "anchor/latch", "on": { "on": "use" },
             "once": false,
             "effects": [ { "type": "narrate", "style": "chat", "text": "The latch is cold." } ] }"#,
    ));
    let prefabs = PrefabRegistry::load_dir(&tmp).unwrap();
    let err = try_build(&c, &prefabs).expect_err("a contested seal hitbox must stop the build");
    let _ = std::fs::remove_dir_all(&tmp);
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded diagnostic, got {err:?}");
    };
    assert_eq!(code, "DW0422");
    assert!(message.contains("trigger/latch"), "{message}");
    assert!(message.contains("anchor/door"), "{message}");
}

/// `DW0423`: one gate anchor, one wording. The seal's answer belongs to the
/// place, so a second `sealed_hint` on the same anchor would never reach a
/// player — the same silence class the verb exists to close.
#[test]
fn dw0423_two_wordings_for_one_gate() {
    let c = parse_hw(&quests_doc_with(
        "0.8.0",
        r#"{ "type": "close-gate", "anchor": "anchor/door",
             "sealed_hint": "The bars will not lift.",
             "happening": { "verb": "seals", "text": "The bars come down." } },
           { "type": "campaign-complete",
             "happening": { "verb": "survives", "text": "The party is out." } }"#,
        r#"{ "id": "trigger/reseal", "at": "anchor/exit", "on": { "on": "approach", "range": 2 },
             "once": true,
             "effects": [ { "type": "close-gate", "anchor": "anchor/door",
                            "sealed_hint": "Nothing you do moves it.",
                            "happening": { "verb": "seals", "text": "It stays shut." } } ] }"#,
    ));
    let d = gates::check_seal_hints(&c);
    assert_eq!(d.len(), 1, "exactly one conflict: {d:#?}");
    assert_eq!(d[0].code, "DW0423");
    assert!(d[0].message.contains("anchor/door"), "{:?}", d[0].message);
}

/// A single wording repeated is not a conflict — only two *different* authored
/// lines are, and a firing that authors nothing asks for the default.
#[test]
fn one_wording_repeated_is_not_a_conflict() {
    let c = parse_hw(&quests_doc_with(
        "0.8.0",
        r#"{ "type": "close-gate", "anchor": "anchor/door",
             "sealed_hint": "The bars will not lift.",
             "happening": { "verb": "seals", "text": "The bars come down." } },
           { "type": "campaign-complete",
             "happening": { "verb": "survives", "text": "The party is out." } }"#,
        r#"{ "id": "trigger/reseal", "at": "anchor/exit", "on": { "on": "approach", "range": 2 },
             "once": true,
             "effects": [ { "type": "close-gate", "anchor": "anchor/door",
                            "sealed_hint": "The bars will not lift.",
                            "happening": { "verb": "seals", "text": "It stays shut." } } ] }"#,
    ));
    assert!(gates::check_seal_hints(&c).is_empty());
}

// --- every site that can FILL a gate must also ARM it ----------------------
//
// A seal the compiler fills but never arms is the finding again, one effect root
// further out. These two roots emit a `close-gate` fill but are invisible to the
// older quest/trigger-only gate scans, so the seal planner walks its own, wider
// traversal (`plan::for_each_gate_effect`) and these pin it.

/// A `close-gate` in a **trap payload** (spec-0022 — a payload is an effect root)
/// arms the seal like any other firing.
#[test]
fn a_trap_payload_seal_is_armed() {
    let c = parse_hw(&quests_doc_trap_payload());
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    let all = all_functions(&out);
    assert!(
        all.lines()
            .any(|l| l.starts_with("fill ") && l.ends_with(" minecraft:iron_bars")),
        "the trap payload must still seal the gate: {all}"
    );
    assert!(
        all.contains(
            "execute unless entity @e[tag=dw_seal_door] run function hello-world:seal_arm_door"
        ),
        "…and a filled seal must be an armed seal: {all}"
    );
    assert!(
        function(&out, "seal_arm_door").contains("summon minecraft:interaction"),
        "the seal's hitboxes must exist"
    );
}

/// …and so does one inside a **dialogue option's** `set-checkpoint` `on_respawn`
/// bundle. `DialogueEffect` carries no gate verb of its own — which is why the
/// quests-stage-only scans stop short — but this bundle is a plain
/// `Vec<QuestEffect>` and its `close-gate` really is lowered, into
/// `cp_on_respawn_<i>`.
#[test]
fn a_dialogue_nested_seal_is_armed() {
    let quests = quests_doc("0.6.0", r#"{ "type": "campaign-complete" }"#);
    let c = parse_hw_with_dialogue(&quests, DIALOGUE_SEALS_ON_RESPAWN);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    let respawn = function(&out, "cp_on_respawn_0");
    assert!(
        respawn
            .lines()
            .any(|l| l.starts_with("fill ") && l.ends_with(" minecraft:iron_bars")),
        "the dialogue-nested close-gate must still seal the gate: {respawn}"
    );
    assert!(
        respawn.contains(
            "execute unless entity @e[tag=dw_seal_door] run function hello-world:seal_arm_door"
        ),
        "…and a filled seal must be an armed seal: {respawn}"
    );
}

/// `DW0423` sees the same sites the planner does: a dialogue-nested wording that
/// disagrees with the quest-stage one is a conflict, reported at its dialogue
/// path rather than silently dropped.
#[test]
fn dw0423_reaches_a_dialogue_nested_wording() {
    let quests = quests_doc(
        "0.8.0",
        r#"{ "type": "close-gate", "anchor": "anchor/door",
             "sealed_hint": "The bars will not lift.",
             "happening": { "verb": "seals", "text": "The bars come down." } },
           { "type": "campaign-complete",
             "happening": { "verb": "survives", "text": "The party is out." } }"#,
    );
    let dialogue = DIALOGUE_SEALS_ON_RESPAWN
        .replacen("\"0.6.0\"", "\"0.8.0\"", 1)
        .replace(
            r#"{ "type": "close-gate", "anchor": "anchor/door" }"#,
            r#"{ "type": "close-gate", "anchor": "anchor/door",
                 "sealed_hint": "Nothing you do moves it.",
                 "happening": { "verb": "seals", "text": "It stays shut." } }"#,
        );
    let c = parse_hw_with_dialogue(&quests, &dialogue);
    let d = gates::check_seal_hints(&c);
    assert_eq!(d.len(), 1, "exactly one conflict: {d:#?}");
    assert_eq!(d[0].code, "DW0423");
    assert_eq!(d[0].stage, "dialogue", "reported at its real stage");
    assert!(
        d[0].path.contains("/on_respawn/"),
        "…and its real path: {}",
        d[0].path
    );
}

/// A campaign that seals no gate emits none of this machinery at all.
#[test]
fn no_seal_no_machinery() {
    let c = parse_hw(&quests_doc("0.6.0", r#"{ "type": "campaign-complete" }"#));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);
    assert!(
        !all_functions(&out).contains("dw_seal_"),
        "no close-gate, no seal hitboxes"
    );
}
