//! Every `set-flag` emission lowers gets its scoreboard objective declared
//! (task #24 — the second of the two latent emission defects on the
//! effect-root drift list).
//!
//! `emit::declared_flags` decides which `dw.f_<flag>` objectives `setup` creates.
//! It is not a lint: a `set-flag` whose objective was never declared writes to
//! nothing at runtime. Vanilla answers `scoreboard players set … <undeclared> 1`
//! with a command error and moves on — no crash, no log a bot reads, and every
//! gate downstream simply never opens. That is the `DW0497` shape (a call with no
//! callee) reproduced one layer down, at the scoreboard.
//!
//! The inventory hand-listed the quests-stage roots plus a trap's
//! `disarm.sets_flag`, a timed gate's `disarm.sets_flag` and the *flat*
//! `DialogueEffect::SetFlag` list — three of the five roots
//! `plan::for_each_effect_root` enumerates. So a `set-flag` in a `traps[].payload`
//! or in a dialogue option's `set-checkpoint` `on_respawn` bundle emitted its
//! write against an objective nothing had created.
//!
//! Every assertion below states its binding: the write is located in the shipped
//! function body first, and only then is the declaration demanded. A test that
//! asserted the declaration alone would pass just as happily if the root stopped
//! being lowered at all.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

const NS: &str = "hello-world";

// ---------------------------------------------------------------------------
// fixture plumbing
// ---------------------------------------------------------------------------

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str, dialogue: Option<&str>) -> Campaign {
    parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue
            .map(str::to_string)
            .unwrap_or_else(|| read_hw("dialogue.json")),
        world_edits: None,
    })
    .expect("campaign parses")
}

fn build(quests: &str, dialogue: Option<&str>) -> BuildOutput {
    let campaign = parse_hw(quests, dialogue);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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

/// hello-world's stage 5 with a caller-supplied `content` prelude (a `traps`
/// array, trailing comma included) and a caller-supplied effect on `obj/talk`'s
/// completion.
fn quests_doc(prelude: &str, effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    {prelude}
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
          "obj/talk": [ {effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

/// A trap whose `payload` fires `effects`.
fn trap_prelude(effects: &str) -> String {
    format!(
        r#""traps": [
      {{ "id": "trap/alarm-chest", "at": "anchor/exit", "trigger": "trapped-chest",
         "lethality": "harmful",
         "payload": [ {effects} ] }}
    ],"#
    )
}

/// hello-world's dialogue with the "Open the door" option additionally hanging a
/// `set-checkpoint` on `anchor/exit`, whose `on_respawn` bundle fires `effects`.
fn respawn_dialogue(effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {{
    "dialogues": [
      {{ "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
        {{ "id": "dlg/greeting",
          "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
          "options": [
            {{ "label": "Open the door, please.",
              "effects": [
                {{ "type": "complete-objective", "objective": "obj/talk" }},
                {{ "type": "set-checkpoint", "anchor": "anchor/exit",
                  "on_respawn": [ {effects} ] }}
              ] }}
          ] }}
      ] }}
    ]
  }}
}}"#
    )
}

fn set_flag(flag: &str) -> String {
    format!(r#"{{ "type": "set-flag", "flag": "flag/{flag}" }}"#)
}

fn text(out: &BuildOutput, key: &str) -> String {
    String::from_utf8(
        out.get(key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .clone(),
    )
    .unwrap()
}

fn setup(out: &BuildOutput) -> String {
    text(
        out,
        &format!("datapack/data/{NS}/function/setup.mcfunction"),
    )
}

/// Every emitted `datapack/` function body, concatenated — what actually ships.
fn all_function_text(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .collect()
}

/// The binding-first assertion, in one place: the objective is WRITTEN somewhere
/// in the shipped pack, and therefore must also be DECLARED in `setup`.
///
/// Order matters. Demanding the declaration alone is a gate that binds to
/// nothing — it would stay green if the root stopped being lowered, which is the
/// opposite of what this file is for.
fn assert_written_and_declared(out: &BuildOutput, flag: &str, where_: &str) {
    let objective = format!("dw.f_{}", flag.replace('-', "_"));
    let write = format!("dw.f_{} 1", flag.replace('-', "_"));
    let all = all_function_text(out);
    assert!(
        all.contains(&write),
        "{where_}: expected a write to `{objective}` somewhere in the pack, \
         so this assertion binds to a real emission"
    );
    let decl = format!("scoreboard objectives add {objective} dummy");
    assert!(
        setup(out).contains(&decl),
        "{where_}: `{objective}` is written but never created — the write is a \
         runtime no-op. setup was:\n{}",
        setup(out)
    );
}

// ---------------------------------------------------------------------------
// the precedent the two new roots follow
// ---------------------------------------------------------------------------

/// A `set-flag` on a quest's `on_objective_complete` gets its objective. Green
/// throughout — the widening applies the rule already here to the roots that
/// were missing it, it does not invent one.
#[test]
fn a_set_flag_on_an_objective_gets_its_objective() {
    let out = build(&quests_doc("", &set_flag("alarm")), None);
    assert_written_and_declared(&out, "alarm", "quest on_objective_complete");
}

/// …including nested inside a `sequence` step, which `visit_deep` already
/// covered. The blind spot was never depth; it was which ROOTS the walk started
/// from.
#[test]
fn a_set_flag_nested_in_a_sequence_gets_its_objective() {
    let out = build(
        &quests_doc(
            "",
            &format!(
                r#"{{ "type": "sequence", "steps": [
                     {{ "at_ticks": 0, "effects": [ {} ] }} ] }}"#,
                set_flag("alarm")
            ),
        ),
        None,
    );
    assert_written_and_declared(&out, "alarm", "set-flag inside a sequence step");
}

// ---------------------------------------------------------------------------
// the two roots the inventory did not reach (task #24)
// ---------------------------------------------------------------------------

/// A `set-flag` in a `traps[].payload` gets its objective.
///
/// Red before task #24: `trap_fire_alarm_chest.mcfunction` shipped
/// `scoreboard players set @a[tag=dw_party] dw.f_alarm 1` while `setup` never
/// ran `scoreboard objectives add dw.f_alarm dummy`. The trap sprang, the write
/// failed, and every gate on `flag/alarm` stayed shut forever.
#[test]
fn a_set_flag_in_a_trap_payload_gets_its_objective() {
    let out = build(
        &quests_doc(
            &trap_prelude(&set_flag("alarm")),
            r#"{ "type": "narrate", "style": "chat", "text": "The bar lifts." }"#,
        ),
        None,
    );
    assert_written_and_declared(&out, "alarm", "set-flag in a trap payload");
}

/// The same, nested inside a `sequence` step inside the payload — the fix must
/// inherit the ROOT and keep descending, not merely glance at the payload's top
/// level.
#[test]
fn a_set_flag_nested_in_a_trap_payload_gets_its_objective() {
    let out = build(
        &quests_doc(
            &trap_prelude(&format!(
                r#"{{ "type": "sequence", "steps": [
                     {{ "at_ticks": 0, "effects": [ {} ] }} ] }}"#,
                set_flag("alarm")
            )),
            r#"{ "type": "narrate", "style": "chat", "text": "The bar lifts." }"#,
        ),
        None,
    );
    assert_written_and_declared(&out, "alarm", "set-flag nested in a trap payload");
}

/// A `set-flag` in a dialogue option's `set-checkpoint` `on_respawn` bundle gets
/// its objective.
///
/// The bundle is lowered into `cp_on_respawn_<i>`, so the write really ships;
/// before task #24 the objective behind it did not exist. Quieter than the trap
/// case, because nothing runs until somebody dies.
///
/// Note this root is a **non-producer** for the completability model
/// (`flow_effect_roots::a_dialogue_respawn_bundle_is_still_never_a_producer`) and
/// that is unrelated: whether the model may *credit* a firing is a question about
/// proofs, while whether the objective it writes to exists is a question about the
/// runtime. The scoreboard must be well-formed either way.
#[test]
fn a_set_flag_in_a_dialogue_respawn_bundle_gets_its_objective() {
    let out = build(
        &quests_doc(
            "",
            r#"{ "type": "narrate", "style": "chat", "text": "The bar lifts." }"#,
        ),
        Some(&respawn_dialogue(&set_flag("woke"))),
    );
    assert_written_and_declared(&out, "woke", "set-flag in a dialogue on_respawn bundle");
}

// ---------------------------------------------------------------------------
// the enumeration itself
// ---------------------------------------------------------------------------

/// One `set-flag` at each of the five roots, each flag named after its root:
/// every one is written, and every one is declared. Read off the shipped pack, so
/// a root dropped or the inventory re-hand-rolled is a diff here rather than a
/// silent runtime hole.
#[test]
fn every_effect_root_declares_the_objectives_it_writes() {
    let quests = format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "triggers": [
      {{ "id": "trigger/wake", "at": "anchor/keeper-stand", "on": {{ "on": "approach", "range": 3 }},
        "effects": [ {} ] }}
    ],
    {}
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{ "obj/talk": [ {} ] }},
        "on_complete": [ {}, {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#,
        set_flag("root-trigger"),
        trap_prelude(&set_flag("root-trap")),
        set_flag("root-objective"),
        set_flag("root-quest"),
    );
    let out = build(&quests, Some(&respawn_dialogue(&set_flag("root-respawn"))));
    for flag in [
        "root-objective",
        "root-quest",
        "root-trigger",
        "root-trap",
        "root-respawn",
    ] {
        assert_written_and_declared(&out, flag, flag);
    }
}
