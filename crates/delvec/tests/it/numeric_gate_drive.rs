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

use crate::common;

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

/// The hello-world `quests` doc with a latch: the first stone (at `spawn`)
/// sets `state/order` to `first_sets`, and the last stone (at the exit, awake
/// once the keeper has spoken and the door stands open) is `last` — the
/// trigger whose bundle sets `flag/latch` behind a gate on the order the first
/// stone wrote. The exit reads the flag, never the datum, so nothing but the
/// flag owes the first stone a press.
fn latch_doc(first_sets: i32, last: &str) -> String {
    let first = format!(
        r#"{{ "id": "trigger/first-stone", "at": "spawn", "on": {{ "on": "use" }},
         "once": false, "prop": {{ "block": "minecraft:oak_sign[rotation=0]" }},
         "effects": [
           {{ "type": "set-state", "state": "state/order", "value": {first_sets},
              "when": {{ "requires_state": [ {{ "state": "state/order", "op": "equals", "value": 0 }} ] }} }}
         ] }}"#
    );
    latch_doc_with(&first, last)
}

/// [`latch_doc`] with the trigger at `spawn` written whole as `first`.
fn latch_doc_with(first: &str, last: &str) -> String {
    common::at_dsl_version(&format!(
        r#"{{
  "dsl_version": "%dsl_version%",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "state": [
      {{ "id": "state/order", "scope": "party", "note": "stones pressed in the right order" }}
    ],
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/latch"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "set-flag", "flag": "flag/told" }}
          ]
        }},
        "on_complete": []
      }}
    ],
    "triggers": [
      {last},
      {first}
    ]
  }}
}}"#
    ))
}

/// The last stone sets the latch behind its line's own `when`: pressed at any
/// order but 1 it runs, prints its marker, and sets nothing.
const LAST_WHEN: &str = r#"{ "id": "trigger/last-stone", "at": "anchor/exit", "on": { "on": "use" },
   "once": false, "prop": { "block": "minecraft:oak_sign[rotation=0]" },
   "requires_flags": ["flag/told"],
   "answers": { "not_yet": "The stone is cold." },
   "effects": [
     { "type": "set-flag", "flag": "flag/latch",
       "when": { "requires_state": [ { "state": "state/order", "op": "equals", "value": 1 } ] } },
     { "type": "set-state", "state": "state/order", "value": 2,
       "when": { "requires_state": [ { "state": "state/order", "op": "equals", "value": 1 } ] } }
   ] }"#;

/// The last stone answers only at order 1: its own `requires_state`.
const LAST_REQUIRES: &str = r#"{ "id": "trigger/last-stone", "at": "anchor/exit", "on": { "on": "use" },
   "once": false, "prop": { "block": "minecraft:oak_sign[rotation=0]" },
   "requires_flags": ["flag/told"],
   "requires_state": [ { "state": "state/order", "op": "equals", "value": 1 } ],
   "answers": { "not_yet": "The stone is cold." },
   "effects": [
     { "type": "set-flag", "flag": "flag/latch" },
     { "type": "set-state", "state": "state/order", "value": 2 }
   ] }"#;

/// The path a latch doc builds, as `(action, trigger-or-objective)`.
fn latch_path(first_sets: i32, last: &str) -> Result<Vec<(String, String)>, String> {
    let p = prefabs();
    let c = validated(&latch_doc(first_sets, last), &p);
    build(&c, &p).map(|out| acts(&critical_path(&out)))
}

/// The path that pays the latch: the first stone, then the last, between the
/// beat that opens the door and the exit that reads the flag.
fn paid_path() -> Vec<(String, String)> {
    let s = |a: &str, w: &str| (a.to_string(), w.to_string());
    vec![
        s("select-class", ""),
        s("talk-to", "obj/talk"),
        s("trigger", "trigger/first-stone"),
        s("trigger", "trigger/last-stone"),
        s("reach", "obj/exit"),
        s("assert-complete", ""),
    ]
}

/// **A flag behind a `when` on state another press writes is paid by the
/// presses that make the `when` hold.** The latch line waits on the order the
/// first stone writes; the path presses the first stone, then the last.
#[test]
fn a_flag_behind_a_when_on_another_press_s_state_is_paid_by_both_presses() {
    let steps = latch_path(1, LAST_WHEN).expect("the two stones pay the latch");
    assert_eq!(steps, paid_path(), "{steps:#?}");
}

/// **The same, when the gate is the paying trigger's own `requires_state`.**
/// Pressed at order 0 the last stone does not run at all and prints no
/// marker; the path presses the first stone ahead of it.
#[test]
fn a_flag_behind_its_trigger_s_requires_state_is_paid_by_both_presses() {
    let steps = latch_path(1, LAST_REQUIRES).expect("the two stones pay the latch");
    assert_eq!(steps, paid_path(), "{steps:#?}");
}

/// **No sequence, no credit.** The first stone sets the order to 5, a value
/// the last stone never answers, so no press the party can make sets the
/// latch. The proof does not credit the flag: `DW1080`, naming the reader,
/// the flag, the payer and the values the presses reach.
#[test]
fn a_flag_no_press_sequence_can_pay_is_dw1080() {
    for last in [LAST_WHEN, LAST_REQUIRES] {
        let err =
            latch_path(5, last).expect_err("a latch no press sets is a delve nobody finishes");
        let code: DwCode = delvec::compiler::plan::DW_FLAG_GATE_UNPAID;
        assert!(err.contains(code.id()), "{err}");
        assert!(err.contains("`obj/exit` reads `flag/latch`"), "{err}");
        assert!(err.contains("trigger `trigger/last-stone`"), "{err}");
        assert!(err.contains("state/order equals 1"), "{err}");
        assert!(err.contains("`state/order` ∈ {0, 5}"), "{err}");
    }
}

/// **A latch another declared trigger sets without a gate is paid by that
/// trigger.** The last stone, declared first, sets the latch only at order 1,
/// and nothing ever makes the order 1; the spare stone at `spawn`, declared
/// after it, sets the latch on any press. The flow model names the first
/// declared producer as the payer, and a gate it cannot open is no reason to
/// refuse while another trigger can pay: the path presses the spare stone,
/// never the last.
#[test]
fn a_flag_a_later_ungated_trigger_sets_is_paid_by_it_not_refused() {
    const SPARE: &str = r#"{ "id": "trigger/spare-stone", "at": "spawn", "on": { "on": "use" },
       "once": false, "prop": { "block": "minecraft:oak_sign[rotation=0]" },
       "effects": [ { "type": "set-flag", "flag": "flag/latch" } ] }"#;
    let p = prefabs();
    let c = validated(&latch_doc_with(SPARE, LAST_WHEN), &p);
    let out = build(&c, &p).expect("the spare stone pays the latch");
    let steps = acts(&critical_path(&out));
    let s = |a: &str, w: &str| (a.to_string(), w.to_string());
    assert_eq!(
        steps,
        [
            s("select-class", ""),
            s("talk-to", "obj/talk"),
            s("trigger", "trigger/spare-stone"),
            s("reach", "obj/exit"),
            s("assert-complete", ""),
        ],
        "{steps:#?}"
    );
}

/// **A lift's latch is paid by the call its car can answer.** The car stands at
/// floor 1 (`initial`), and the only writes to its floor are the two calls'
/// `sequence` lines. The call declared first answers only away from floor 1,
/// so pressed from where the path stands it does nothing; the other call, at
/// the exit, answers away from floor 2 and sets the latch as its ride ends.
/// The floor is a datum only presses move, even though they move it in a
/// nested line, so the plan follows it from its declared start and presses
/// the call that answers — never refusing the latch as unknowable.
#[test]
fn a_latch_a_lift_call_sets_in_its_ride_is_paid_by_the_call_that_answers() {
    const DOWN: &str = r#"{ "id": "trigger/call-down", "at": "spawn", "on": { "on": "use" },
       "once": false, "prop": { "block": "minecraft:oak_sign[rotation=0]" },
       "answers": { "not_yet": "The car is already here." },
       "requires_state": [ { "state": "state/order", "op": "not-equals", "value": 1 } ],
       "effects": [ { "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [
         { "type": "set-state", "state": "state/order", "value": 1 },
         { "type": "set-flag", "flag": "flag/latch" } ] } ] } ] }"#;
    const UP: &str = r#"{ "id": "trigger/call-up", "at": "anchor/exit", "on": { "on": "use" },
       "once": false, "prop": { "block": "minecraft:oak_sign[rotation=0]" },
       "requires_flags": ["flag/told"],
       "answers": { "not_yet": "The call is dead." },
       "requires_state": [ { "state": "state/order", "op": "not-equals", "value": 2 } ],
       "effects": [ { "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [
         { "type": "set-state", "state": "state/order", "value": 2 },
         { "type": "set-flag", "flag": "flag/latch" } ] } ] } ] }"#;
    let doc = latch_doc_with(UP, DOWN).replace(
        r#""note": "stones pressed in the right order" }"#,
        r#""note": "the floor the car stands at", "initial": 1 }"#,
    );
    assert!(doc.contains(r#""initial": 1"#), "the car starts at floor 1");
    let p = prefabs();
    let c = validated(&doc, &p);
    let out = build(&c, &p).expect("the call that answers pays the latch");
    let steps = acts(&critical_path(&out));
    let s = |a: &str, w: &str| (a.to_string(), w.to_string());
    assert_eq!(
        steps,
        [
            s("select-class", ""),
            s("talk-to", "obj/talk"),
            s("trigger", "trigger/call-up"),
            s("reach", "obj/exit"),
            s("assert-complete", ""),
        ],
        "{steps:#?}"
    );
}

/// **The presses that pay a flag are made in front of the step that reads
/// it.** The last stone here wakes at once (no `flag/told`), so the path could
/// first perform it ahead of the talk, while the door between the party and
/// the exit is still shut; pressed there it would not pay, and the presses
/// that do pay it are owed only by the exit. They are made after the talk that
/// opens the door, in front of the exit, as an objective's own numeric gate is
/// driven.
#[test]
fn a_flag_s_presses_are_made_in_front_of_the_step_that_reads_it() {
    let last = LAST_WHEN
        .replace(r#""requires_flags": ["flag/told"],"#, "")
        .replace(r#""answers": { "not_yet": "The stone is cold." },"#, "");
    assert_ne!(
        last, LAST_WHEN,
        "the last stone no longer waits on the talk"
    );
    let steps = latch_path(1, &last).expect("the two stones pay the latch after the talk");
    assert_eq!(steps, paid_path(), "{steps:#?}");
}
