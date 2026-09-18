//! **The path performs the triggers it depends on.**
//!
//! An environment trigger is a party action nothing on the quest DAG orders:
//! somebody has to strike it, use it, walk up to it or hit the NPC it watches.
//! Two proofs credit what it does — the region-write model credits the way it
//! opens, the flow replay credits the flags it sets — and until this file the
//! exported `critical-path.json` contained no step that did the act. The
//! vesperhold ladder walked into exactly that: `anchor/gate-psalter-wall` is
//! opened only by a `strike` trigger gated on `flag/ledger-read`, the region
//! model credited the open at step 0 (before the flag could even be set), the
//! build was green, and the bot stopped in front of a wall nobody struck.
//!
//! The pair below is on the in-repo `hello-world` fixture: `hello-room`'s
//! `anchor/door` bars six cells of the doorway between the keeper and the exit,
//! and the only thing that lifts them is a `strike` trigger on the bars.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};

/// A hello-world `quests` doc: talk to the keeper (which sets `flag/told`), then
/// reach `anchor/exit` beyond the barred door. `trigger` is the one environment
/// trigger; `on_complete` is spliced into the quest's `on_complete`.
fn quests_doc(trigger: &str, on_complete: &str) -> String {
    common::at_dsl_version(&format!(
        r#"{{
  "dsl_version": "%dsl_version%",
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
          "obj/talk": [ {{ "type": "set-flag", "flag": "flag/told" }} ]
        }},
        "on_complete": [ {on_complete} ]
      }}
    ],
    "triggers": [ {trigger} ]
  }}
}}"#
    ))
}

/// The bars fall to a strike once the keeper has spoken.
const STRIKE_THE_BARS: &str = r#"{
  "id": "trigger/break-the-bars", "at": "anchor/door", "on": { "on": "strike" },
  "requires_flags": ["flag/told"],
  "effects": [ { "type": "open-gate", "anchor": "anchor/door" } ]
}"#;

/// The same trigger armed only by a flag the quest sets on completion — after
/// the exit, so no point of the path both holds the flag and precedes the leg
/// through the door.
const STRIKE_THE_BARS_TOO_LATE: &str = r#"{
  "id": "trigger/break-the-bars", "at": "anchor/door", "on": { "on": "strike" },
  "requires_flags": ["flag/out"],
  "effects": [ { "type": "open-gate", "anchor": "anchor/door" } ]
}"#;

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

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                out.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    out
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, String> {
    let plan = Plan::build(campaign, prefabs).map_err(|e| format!("{e:?}"))?;
    let structures = structures(&plan);
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
    .map_err(|e| format!("{e:?}"))
}

fn validated(quests: &str, prefabs: &PrefabRegistry) -> Campaign {
    let c = parse_hw(quests);
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let d = validate_campaign_with(&c, &items, prefabs, &entities);
    assert!(
        d.is_empty(),
        "the campaign under test must be valid: {d:#?}"
    );
    c
}

fn critical_path(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("critical-path.json")
            .expect("a critical path ships"),
    )
    .expect("critical-path.json parses")
}

fn actions(path: &serde_json::Value) -> Vec<String> {
    path["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| s["action"].as_str().unwrap_or("").to_string())
        .collect()
}

/// **The step.** The path talks to the keeper, then strikes the bars, then walks
/// through them — the strike is a step of its own, naming the trigger, the kind
/// of act, and the cell it is done at.
#[test]
fn a_path_through_a_struck_gate_strikes_it() {
    let p = prefabs();
    let c = validated(&quests_doc(STRIKE_THE_BARS, ""), &p);
    let out = build(&c, &p).expect("the struck gate opens the way");
    let path = critical_path(&out);
    assert_eq!(
        actions(&path),
        [
            "select-class",
            "talk-to",
            "trigger",
            "reach",
            "assert-complete"
        ],
        "the strike sits between the beat that arms it and the leg through the door: {path:#}"
    );
    let strike = &path["steps"][2];
    assert_eq!(strike["trigger"], "trigger/break-the-bars");
    assert_eq!(strike["on"], "strike");
    assert_eq!(strike["anchor"], "anchor/door");
    assert!(
        strike["pos"].is_array(),
        "a strike is done somewhere: {strike}"
    );
    assert!(
        strike.get("objective").is_none(),
        "a trigger step proves no objective: {strike}"
    );
}

/// **The marker it passes on.** The trigger's bundle broadcasts the anchored
/// fired line with its own id as the token, before its effects run — the
/// strike landing is not the proof, the bundle running is.
#[test]
fn a_performable_trigger_broadcasts_its_fired_marker() {
    let p = prefabs();
    let c = validated(&quests_doc(STRIKE_THE_BARS, ""), &p);
    let out = build(&c, &p).expect("builds");
    let f = out
        .get("datapack/data/hello-world/function/trig_break_the_bars.mcfunction")
        .expect("the trigger's bundle");
    let text = String::from_utf8(f.clone()).unwrap();
    let marker = text
        .lines()
        .position(|l| l.contains("[dw:complete hello-world trigger/break-the-bars]"))
        .unwrap_or_else(|| panic!("no fired marker in the bundle: {text}"));
    let open = text
        .lines()
        .position(|l| l.contains("minecraft:air replace minecraft:iron_bars"))
        .unwrap_or_else(|| panic!("no open in the bundle: {text}"));
    assert!(marker < open, "the marker precedes the effects: {text}");
}

/// **The model no longer credits an open nobody makes.** The trigger's gate holds
/// only after the exit has been reached, so no point of the path can strike the
/// bars before walking through them. The region-write model used to fire every
/// trigger's open at step 0 — before `flag/out` could possibly be set — and this
/// campaign built green with a path that walks into iron bars.
#[test]
fn a_gate_only_a_trigger_the_path_cannot_perform_opens_is_dw0317() {
    let p = prefabs();
    let c = validated(
        &quests_doc(
            STRIKE_THE_BARS_TOO_LATE,
            r#"{ "type": "set-flag", "flag": "flag/out" }"#,
        ),
        &p,
    );
    let err = build(&c, &p).expect_err("a wall nobody on the path strikes is a wall");
    assert!(err.contains("DW0317"), "{err}");
    assert!(err.contains("anchor/door"), "{err}");
}

/// **The flag half.** The door is opened by the talk beat, and the exit is gated
/// on `flag/lit`, which only a `use` trigger on the spawn stone sets. The flow
/// replay credits a trigger's flag the moment its gate holds; the path has to
/// contain the press that sets it, before the step that reads it.
#[test]
fn a_flag_only_a_trigger_sets_is_paid_by_a_trigger_step() {
    let p = prefabs();
    let quests = common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
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
            "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/lit"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": []
      }
    ],
    "triggers": [
      { "id": "trigger/light-the-stone", "at": "spawn", "on": { "on": "use" },
        "effects": [ { "type": "set-flag", "flag": "flag/lit" } ] }
    ]
  }
}"#,
    );
    let c = validated(&quests, &p);
    let out = build(&c, &p).expect("builds");
    let path = critical_path(&out);
    let acts = actions(&path);
    let press = acts
        .iter()
        .position(|a| a == "trigger")
        .unwrap_or_else(|| panic!("the path never presses the stone: {path:#}"));
    let exit = acts.iter().position(|a| a == "reach").expect("the exit");
    assert!(
        press < exit,
        "the press precedes the step that reads its flag: {path:#}"
    );
    assert_eq!(path["steps"][press]["trigger"], "trigger/light-the-stone");
    assert_eq!(path["steps"][press]["on"], "use");
}

/// A hello-world `quests` doc whose only `open-gate` is a `strike` trigger on the
/// `spawn` stone, armed by the talk beat. The stone stands on the keeper's side
/// of the barred door, so the leg from the stone to `anchor/exit` crosses the
/// door — and is walkable only if the strike, performed at its own step, counts
/// as having happened before the arrival at the exit.
fn strike_the_stone_doc() -> String {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
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
          "obj/talk": [ { "type": "set-flag", "flag": "flag/told" } ]
        },
        "on_complete": []
      }
    ],
    "triggers": [
      { "id": "trigger/strike-the-stone", "at": "spawn", "on": { "on": "strike" },
        "requires_flags": ["flag/told"],
        "effects": [ { "type": "open-gate", "anchor": "anchor/door" } ] }
    ]
  }
}"#,
    )
}

/// A hello-world `quests` doc where the talk beat itself opens the door (and arms
/// the lamp), and the exit reads `flag/lit`, which only a `use` on the far-side
/// `anchor/exit` sets.
/// The leg INTO that trigger step crosses the door, so it is walkable only if
/// the talk beat — an ancestor of the objective the press is performed in front
/// of — counts as having fired before the arrival at the trigger step.
fn press_beyond_the_door_doc() -> String {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
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
            "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/lit"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "set-flag", "flag": "flag/told" }
          ]
        },
        "on_complete": []
      }
    ],
    "triggers": [
      { "id": "trigger/light-the-lamp", "at": "anchor/exit", "on": { "on": "use" },
        "requires_flags": ["flag/told"],
        "effects": [ { "type": "set-flag", "flag": "flag/lit" } ] }
    ]
  }
}"#,
    )
}

fn step_index(plan: &Plan, pred: impl Fn(&delvec::compiler::plan::Step) -> bool) -> usize {
    plan.critical_path
        .iter()
        .position(pred)
        .expect("the step is on the path")
}

/// **The ordering, asked directly.** A trigger step precedes every later step of
/// its path: a gate it opens has fired before the arrival at the exit, and the
/// step it is performed at inherits what precedes the objective after it.
#[test]
fn a_trigger_step_has_fired_before_every_later_arrival() {
    let p = prefabs();
    let c = validated(&strike_the_stone_doc(), &p);
    let plan = Plan::build(&c, &p).expect("plan builds");
    let t = step_index(&plan, |s| s.trigger() == Some("trigger/strike-the-stone"));
    let exit = step_index(&plan, |s| s.objective() == Some("obj/exit"));
    assert!(t < exit, "the strike is performed before the exit");
    assert!(
        plan.gate_fired_before(t, exit),
        "the strike at step {t} has fired before the arrival at step {exit}"
    );
    assert!(
        !plan.gate_fired_before(exit, t),
        "and not the other way round"
    );

    let c = validated(&press_beyond_the_door_doc(), &p);
    let plan = Plan::build(&c, &p).expect("plan builds");
    let talk = step_index(&plan, |s| s.objective() == Some("obj/talk"));
    let t = step_index(&plan, |s| s.trigger() == Some("trigger/light-the-lamp"));
    assert!(talk < t);
    assert!(
        plan.gate_fired_before(talk, t),
        "the talk beat (step {talk}) precedes the press performed in front of its successor \
         (step {t})"
    );
}

/// **A verdict that turns on the sweep.** The strike opens the door from the
/// keeper's side, and the walk from the stone to the exit goes through the door.
/// Crediting the opening to every later arrival is what makes that leg walkable;
/// without it the build is `DW0317` on the door.
#[test]
fn a_door_a_trigger_step_opened_is_open_for_the_legs_after_it() {
    let p = prefabs();
    let c = validated(&strike_the_stone_doc(), &p);
    let out = build(&c, &p).expect("the struck door is open for the walk to the exit");
    assert_eq!(
        actions(&critical_path(&out)),
        [
            "select-class",
            "talk-to",
            "trigger",
            "reach",
            "assert-complete"
        ]
    );
}

/// **A verdict that turns on the trigger step's own row.** The talk beat opens
/// the door; the press stands beyond it. The leg into the press crosses the
/// door, so it is walkable only because the press inherits the talk beat as a
/// predecessor; without that row the build is `DW0317` on the door.
#[test]
fn the_leg_into_a_trigger_step_sees_what_its_objective_inherits() {
    let p = prefabs();
    let c = validated(&press_beyond_the_door_doc(), &p);
    let out = build(&c, &p).expect("the door the talk opened is open for the walk to the press");
    assert_eq!(
        actions(&critical_path(&out)),
        [
            "select-class",
            "talk-to",
            "trigger",
            "reach",
            "assert-complete"
        ]
    );
}

// ---------------------------------------------------------------------------
// The refusals the ordering makes possible.
//
// A leg whose start is not an ancestor of its arrival is judged over the OPEN
// world (`World::leg_region_state`: a lineariser artifact the party never walks
// is not sealed). So a trigger step with no place in the ancestry is a step whose
// legs are never judged against a shut gate at all — the direction that ships.
// Each fixture below crosses a door nothing opens, on a leg only the trigger
// ordering makes causal, and must be refused.
// ---------------------------------------------------------------------------

/// The talk arms a stone on the keeper's side; the stone pays the exit's flag;
/// the door between the stone and the exit is never opened.
fn stone_then_barred_exit_doc() -> String {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
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
            "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/lit"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "set-flag", "flag": "flag/told" } ]
        },
        "on_complete": []
      }
    ],
    "triggers": [
      { "id": "trigger/light-the-stone", "at": "spawn", "on": { "on": "use" },
        "requires_flags": ["flag/told"],
        "effects": [ { "type": "set-flag", "flag": "flag/lit" } ] }
    ]
  }
}"#,
    )
}

/// The talk arms a lamp BEYOND the door; the door is never opened.
fn lamp_beyond_a_barred_door_doc() -> String {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
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
            "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/lit"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "set-flag", "flag": "flag/told" } ]
        },
        "on_complete": []
      }
    ],
    "triggers": [
      { "id": "trigger/light-the-lamp", "at": "anchor/exit", "on": { "on": "use" },
        "requires_flags": ["flag/told"],
        "effects": [ { "type": "set-flag", "flag": "flag/lit" } ] }
    ]
  }
}"#,
    )
}

/// **The sweep.** The leg out of the trigger step, from the stone to the exit,
/// crosses a door nothing opens. It is judged only because the stone precedes
/// the exit in the ancestry; without that the leg is read over the open world
/// and the delve builds with a path that walks into iron bars.
#[test]
fn the_leg_out_of_a_trigger_step_is_judged_against_a_shut_door() {
    let p = prefabs();
    let c = validated(&stone_then_barred_exit_doc(), &p);
    let err = build(&c, &p).expect_err("the walk from the stone to the exit meets the bars");
    assert!(err.contains("DW0317"), "{err}");
    assert!(err.contains("anchor/door"), "{err}");
}

/// **The trigger step's own row.** The leg INTO the press, from the keeper to
/// the lamp beyond the door, crosses a door nothing opens. It is judged only
/// because the press inherits the talk beat as a predecessor; without that row
/// the leg is read over the open world and the delve builds.
#[test]
fn the_leg_into_a_trigger_step_is_judged_against_a_shut_door() {
    let p = prefabs();
    let c = validated(&lamp_beyond_a_barred_door_doc(), &p);
    let err = build(&c, &p).expect_err("the walk from the keeper to the lamp meets the bars");
    assert!(err.contains("DW0317"), "{err}");
    assert!(err.contains("anchor/door"), "{err}");
}
