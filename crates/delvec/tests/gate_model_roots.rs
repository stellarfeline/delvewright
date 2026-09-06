//! The close-gate **completability model** sees every root emission can fill a
//! gate from.
//!
//! `plan::collect_region_events` feeds the nav proofs (`DW0311`/`DW0315`/`DW0342`/
//! `DW0410`) their picture of which gate regions are solid on which walked leg.
//! It used to walk three effect roots where emission reaches five, so a
//! `close-gate` inside a `traps[].payload` or a dialogue option's `set-checkpoint`
//! `on_respawn` bundle was **emitted but not modelled**: the datapack sealed a
//! wall the proof believed was open, and a delve could ship provably-completable
//! and be physically unfinishable.
//!
//! Both fixtures below seal `anchor/door` — which the hello-world critical path
//! must walk through — from a root the old scan could not see, and assert the
//! model now refuses the build.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

/// A hello-world `quests` doc that never opens `anchor/door` itself, with a raw
/// `traps` array body (no surrounding brackets) spliced in.
fn quests_doc(traps: &str) -> String {
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
        "on_objective_complete": {{}},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "traps": [ {traps} ]
  }}
}}"#
    )
}

/// A trap whose payload seals `anchor/door` — the gate the forced path crosses.
const TRAP_SEALS_THE_DOOR: &str = r#"{
  "id": "trap/spring-the-door",
  "at": "anchor/exit",
  "trigger": "trapped-chest",
  "lethality": "harmful",
  "payload": [ { "type": "close-gate", "anchor": "anchor/door" } ]
}"#;

/// A dialogue doc whose option sets a checkpoint whose `on_respawn` bundle seals
/// `anchor/door`. `DialogueEffect` carries no gate verb, but the bundle is a plain
/// `Vec<QuestEffect>` and its `close-gate` is really lowered (into
/// `cp_on_respawn_<i>`).
const DIALOGUE_SEALS_THE_DOOR: &str = r#"{
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

fn parse_hw(quests: &str, dialogue: Option<&str>) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue
            .map(str::to_string)
            .unwrap_or_else(|| read_hw("dialogue.json")),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn try_build(campaign: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, BuildFailure> {
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
        &BTreeMap::new(),
    )
}

/// The coded diagnostic a build failed with.
fn failure_code(err: BuildFailure) -> (String, String) {
    match err {
        BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a coded diagnostic, got {other:?}"),
    }
}

/// A `close-gate` in a **trap payload** seals the region for the completability
/// model. The forced path must cross `anchor/door`, so the build fails `DW0311`.
///
/// Red against `origin/main`: the build **succeeds**, because
/// `collect_region_events` never looked inside `traps[].payload`.
#[test]
fn a_trap_payload_close_gate_is_modelled() {
    let c = parse_hw(&quests_doc(TRAP_SEALS_THE_DOOR), None);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let err = try_build(&c, &prefabs)
        .expect_err("a trap payload that seals the forced path must fail the nav proof");
    let (code, message) = failure_code(err);
    assert_eq!(code, "DW0311", "{message}");
    assert!(
        message.contains("close-gate"),
        "the message must name the seal: {message}"
    );
}

/// …and so does one in a dialogue option's `set-checkpoint` `on_respawn` bundle.
///
/// Red against `origin/main`: the build **succeeds** — the old scan stopped at the
/// quests stage entirely, so this seal was invisible to every nav proof.
#[test]
fn a_dialogue_nested_close_gate_is_modelled() {
    let c = parse_hw(&quests_doc(""), Some(DIALOGUE_SEALS_THE_DOOR));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let err = try_build(&c, &prefabs)
        .expect_err("a dialogue-nested seal on the forced path must fail the nav proof");
    let (code, message) = failure_code(err);
    assert_eq!(code, "DW0311", "{message}");
    assert!(
        message.contains("close-gate"),
        "the message must name the seal: {message}"
    );
}

/// The widening is a **seal**, never an unseal: a trap payload that seals a gate
/// the quest line later opens still builds, because the causally-latest firing on
/// the region is the `open-gate`. (This is the shape the seal-arming tests use, and
/// it must keep passing — the model gained sight of a close, not a veto.)
#[test]
fn a_later_open_gate_still_wins() {
    let quests = r#"{
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
}"#;
    let c = parse_hw(quests, None);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    try_build(&c, &prefabs).expect("a reopened gate is not a sealed gate");
}
