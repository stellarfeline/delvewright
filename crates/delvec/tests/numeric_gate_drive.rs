//! **The path drives a numeric gate that only presses move** (`DW0985`).
//!
//! An objective gated on a datum that nothing but `use` triggers writes — a
//! valve puzzle — used to stall the bot: the plan performs a trigger for the
//! way it opens or the flag it pays, and a valve does neither, so the path
//! walked up to the gated beat and waited. The plan now replays every press in
//! the order the datapack runs it and schedules the shortest sequence that
//! makes the gate hold, or refuses the build when no sequence within its bound
//! can.
//!
//! On the in-repo `hello-world` fixture: the talk beat opens the door, and the
//! exit beyond it waits on `state/round at-least 2`. Two valves write the
//! round — one on the `spawn` stone, one at the exit — and the one declared
//! FIRST is the one that must be pressed SECOND, so a plan that pressed the
//! writers in declaration order would not open the gate.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, DwCode, RawCampaign, parse_campaign, validate_campaign_with};

/// The hello-world `quests` doc with a datum, the exit gated on it, and the two
/// valves; `second` and `first` are the two triggers' effect lists.
fn valves_doc(second: &str, first: &str) -> String {
    common::at_dsl_version(&format!(
        r#"{{
  "dsl_version": "%dsl_version%",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "state": [
      {{ "id": "state/round", "scope": "party", "note": "right valves in a row" }}
    ],
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"],
             "requires_state": [ {{ "state": "state/round", "op": "at-least", "value": 2 }} ] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": []
      }}
    ],
    "triggers": [
      {{ "id": "trigger/second-valve", "at": "anchor/exit", "on": {{ "on": "use" }},
         "once": false, "prop": {{ "block": "minecraft:oak_sign[rotation=0]" }},
         "effects": [ {second} ] }},
      {{ "id": "trigger/first-valve", "at": "spawn", "on": {{ "on": "use" }},
         "once": false, "prop": {{ "block": "minecraft:oak_sign[rotation=0]" }},
         "effects": [ {first} ] }}
    ]
  }}
}}"#
    ))
}

/// The second valve answers only after the first: at 1 it makes 2; pressed
/// first, it is the wrong valve and puts the round back to 0.
const SECOND: &str = r#"
  { "type": "set-state", "state": "state/round", "value": 2,
    "when": { "requires_state": [ { "state": "state/round", "op": "equals", "value": 1 } ] } },
  { "type": "narrate", "text": "Wrong valve.", "style": "actionbar",
    "when": { "requires_state": [ { "state": "state/round", "op": "equals", "value": 0 } ] } }"#;

/// The first valve starts the round.
const FIRST: &str = r#"
  { "type": "set-state", "state": "state/round", "value": 1,
    "when": { "requires_state": [ { "state": "state/round", "op": "equals", "value": 0 } ] } }"#;

/// The first valve with its own reset written AFTER its increment: the
/// increment makes 1, and the line after it — read against the value the
/// line before produced — puts it straight back to 0. No press ever leaves
/// the round at 1.
const FIRST_UNDONE: &str = r#"
  { "type": "add-state", "state": "state/round", "amount": 1,
    "when": { "requires_state": [ { "state": "state/round", "op": "equals", "value": 0 } ] } },
  { "type": "set-state", "state": "state/round", "value": 0,
    "when": { "requires_state": [ { "state": "state/round", "op": "equals", "value": 1 } ] } }"#;

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
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).expect("prefab library loads")
}

fn validated(quests: &str, prefabs: &PrefabRegistry) -> Campaign {
    let c = parse_hw(quests);
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let d = validate_campaign_with(&c, &items, prefabs, &entities);
    let errors: Vec<_> = d
        .iter()
        .filter(|x| x.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "the campaign under test must be valid: {errors:#?}"
    );
    c
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, String> {
    let plan = Plan::build(campaign, prefabs).map_err(|e| format!("{e:?}"))?;
    let mut structures = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        prefabs,
        None,
        &BTreeMap::new(),
    )
    .map_err(|e| format!("{e:?}"))
}

fn critical_path(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("critical-path.json")
            .expect("a critical path ships"),
    )
    .expect("critical-path.json parses")
}

/// `(action, trigger-or-objective)` per step.
fn acts(path: &serde_json::Value) -> Vec<(String, String)> {
    path["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| {
            let what = s["trigger"]
                .as_str()
                .or(s["objective"].as_str())
                .unwrap_or("");
            (
                s["action"].as_str().unwrap_or("").to_string(),
                what.to_string(),
            )
        })
        .collect()
}

/// **The presses, in the order that opens the gate.** The first valve, then
/// the second, between the beat that opens the door and the exit they gate —
/// although the second is declared first.
#[test]
fn a_gate_only_presses_move_is_driven_by_the_presses_in_order() {
    let p = prefabs();
    let c = validated(&valves_doc(SECOND, FIRST), &p);
    let out = build(&c, &p).expect("the valves open the gate");
    let path = critical_path(&out);
    let steps = acts(&path);
    let s = |a: &str, w: &str| (a.to_string(), w.to_string());
    assert_eq!(
        steps,
        [
            s("select-class", ""),
            s("talk-to", "obj/talk"),
            s("trigger", "trigger/first-valve"),
            s("trigger", "trigger/second-valve"),
            s("reach", "obj/exit"),
            s("assert-complete", ""),
        ],
        "the path presses the first valve, then the second, before the exit: {path:#}"
    );
    let second = &path["steps"][3];
    assert_eq!(second["on"], "use");
    assert_eq!(second["anchor"], "anchor/exit");
}

/// **Each press prints the line its step waits on.** A valve neither opens a
/// way nor sets a flag; it is performed because a gate owes it, so its bundle
/// carries the fired marker the bot passes the step on.
#[test]
fn a_press_a_gate_owes_broadcasts_its_fired_marker() {
    let p = prefabs();
    let c = validated(&valves_doc(SECOND, FIRST), &p);
    let out = build(&c, &p).expect("builds");
    for t in ["first_valve", "second_valve"] {
        let f = out
            .get(&format!(
                "datapack/data/hello-world/function/trig_{t}.mcfunction"
            ))
            .unwrap_or_else(|| panic!("the bundle of {t}"));
        let text = String::from_utf8(f.clone()).unwrap();
        let id = t.replace('_', "-");
        assert!(
            text.contains(&format!("[dw:complete hello-world trigger/{id}]")),
            "no fired marker in {t}: {text}"
        );
    }
}

/// **No sequence, no build.** With the first valve's reset written after its
/// own increment, every press of it leaves the round at 0, and the second valve
/// answers only at 1: nothing the party can press opens the exit. The build is
/// refused, naming the gate, the presses and the values they reach.
#[test]
fn a_gate_no_press_sequence_can_open_is_dw0985() {
    let p = prefabs();
    let c = validated(&valves_doc(SECOND, FIRST_UNDONE), &p);
    let err = build(&c, &p).expect_err("an exit no press can open is a delve nobody finishes");
    let code: DwCode = delvec::compiler::plan::DW_GATE_UNDRIVABLE;
    assert!(err.contains(code.id()), "{err}");
    assert!(err.contains("obj/exit"), "{err}");
    assert!(err.contains("state/round at-least 2"), "{err}");
    assert!(err.contains("trigger/first-valve"), "{err}");
    assert!(err.contains("`state/round` ∈ {0}"), "{err}");
}
