//! Round-6 staging primitives, emission side:
//!
//! * `move-npc.on_arrive` — a `mv_arrive_<key>` bundle fired on the walk
//!   driver's final-waypoint tick, exact parity with `ma_arrive_<key>`;
//! * `forbids_flags` — the negative flag gate emitted as `unless score`
//!   guards (per-player sites) and `unless entity @a[scores=…]` fire
//!   conditions (trigger arming), plus the `verb_forbid_gate` PackTest.
//!
//! Built on the hello-world fixture with a v0.6 quests/dialogue overlay.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A v0.6 quests doc: the keeper's walk carries an `on_arrive` (set-flag), an
/// effect and two objectives carry `forbids_flags`, and an approach trigger is
/// armed by `flag/arrived` but stood down by `flag/blocked`.
const QUESTS_V06: &str = r#"{
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
          { "type": "interact", "id": "obj/lever", "anchor": "anchor/keeper-stand",
            "after": ["obj/talk"], "forbids_flags": ["flag/blocked"] },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/lever"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "narrate", "text": "The way is open.",
              "forbids_flags": ["flag/blocked"] },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
              "on_arrive": [ { "type": "set-flag", "flag": "flag/arrived" } ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "triggers": [
      {
        "id": "trigger/retaliate",
        "at": "anchor/keeper-stand",
        "on": { "on": "approach", "range": 4 },
        "requires_flags": ["flag/arrived"],
        "forbids_flags": ["flag/blocked"],
        "effects": [ { "type": "set-flag", "flag": "flag/blocked" } ]
      }
    ]
  }
}"#;

/// hello-world's dialogue with the completing option `forbids_flags`-gated.
fn dialogue_with_forbids() -> String {
    read_hw("dialogue.json")
        .replacen("\"0.2.0\"", "\"0.6.0\"", 1)
        .replacen(
            "\"effects\": [",
            "\"forbids_flags\": [\"flag/blocked\"], \"effects\": [",
            1,
        )
}

fn parse_hw() -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: QUESTS_V06.to_string(),
        dialogue: dialogue_with_forbids(),
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
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
    .expect("every emitted command validates")
}

fn file(out: &BuildOutput, path: &str) -> String {
    out.iter()
        .find(|(p, _)| p.as_str() == path)
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("expected build output file `{path}`"))
}

fn built() -> BuildOutput {
    let campaign = parse_hw();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    build(&campaign, &prefabs)
}

const FN_DIR: &str = "datapack/data/hello-world/function";

/// `move-npc.on_arrive` emits a `mv_arrive_<key>` bundle, and the walk driver
/// fires it on the final-waypoint tick — the same arrival detection as
/// `ma_tick`/`ma_arrive` (parity contract).
#[test]
fn move_npc_on_arrive_fires_on_final_waypoint_tick() {
    let out = built();
    let arrive = file(&out, &format!("{FN_DIR}/mv_arrive_keeper_exit.mcfunction"));
    assert!(
        arrive.contains("scoreboard players set @s dw.f_arrived 1"),
        "the on_arrive set-flag must be emitted in the arrive bundle: {arrive}"
    );
    let tick = file(&out, &format!("{FN_DIR}/mv_tick_keeper_exit.mcfunction"));
    assert!(
        tick.contains("run function hello-world:mv_arrive_keeper_exit")
            && tick.contains("execute if score #mt_keeper_exit dw.sys matches"),
        "the driver must call the arrive bundle on the final-waypoint tick: {tick}"
    );
    // The hook fires exactly on the final waypoint index (ticks() == the last
    // waypoint), i.e. strictly before the counter passes the end.
    let hook = tick
        .lines()
        .find(|l| l.contains("mv_arrive_keeper_exit"))
        .unwrap();
    let stop = tick
        .lines()
        .find(|l| l.contains("run scoreboard players set #mrun_keeper_exit dw.sys 0"))
        .unwrap();
    let hook_tick: u32 = hook
        .split("matches ")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let stop_tick: u32 = stop
        .split("matches ")
        .nth(1)
        .unwrap()
        .split("..")
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(
        hook_tick + 1,
        stop_tick,
        "arrive fires on the final waypoint tick, one before the driver stops"
    );
}

/// A `forbids_flags`-gated effect is wrapped in per-player `unless score`
/// guards (unset-safe: an unset score counts as "not set").
#[test]
fn forbids_gated_effect_emits_unless_score_guard() {
    let out = built();
    let complete = file(&out, &format!("{FN_DIR}/complete_o_talk.mcfunction"));
    assert!(
        complete
            .lines()
            .any(|l| l.starts_with("execute unless score @s dw.f_blocked matches 1 run tellraw")),
        "the forbids-gated narrate must be guarded `unless score @s dw.f_blocked matches 1`: {complete}"
    );
}

/// A trigger-level `forbids_flags` suppresses the fire condition while ANY
/// player holds the flag: `unless entity @a[scores={…=1..}]` — a positive
/// selector inside a negation, so unset scores never suppress.
#[test]
fn forbids_gated_trigger_emits_unless_entity_condition() {
    let out = built();
    let tick = file(&out, &format!("{FN_DIR}/tick.mcfunction"));
    let fire = tick
        .lines()
        .find(|l| l.contains("run function hello-world:trig_retaliate"))
        .expect("trigger fire condition emitted");
    assert!(
        fire.contains("unless entity @a[scores={dw.f_blocked=1..}]"),
        "trigger arming must carry the any-player forbid guard: {fire}"
    );
    assert!(
        fire.contains("dw.f_arrived=1.."),
        "the requires gate stays on the same condition: {fire}"
    );
}

/// An objective-level `forbids_flags` joins the pending/activation guard as an
/// `unless score` clause.
#[test]
fn forbids_gated_objective_pending_guard_has_unless() {
    let out = built();
    let tick = file(&out, &format!("{FN_DIR}/tick.mcfunction"));
    assert!(
        tick.lines()
            .any(|l| l.contains("dw.i_lever")
                && l.contains("unless score @s dw.f_blocked matches 1")),
        "the interact objective's tick guard must carry the forbid clause: {tick}"
    );
}

/// A `forbids_flags`-gated dialogue option is inert to a direct `/trigger` once
/// the flag is set (`return fail`), mirroring the `requires_flags` guard.
#[test]
fn forbids_gated_dialogue_option_handler_fails_fast() {
    let out = built();
    let gated = out
        .iter()
        .filter(|(p, _)| p.contains("/function/dlg_keeper_"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .find(|s| s.contains("execute if score @s dw.f_blocked matches 1 run return fail"));
    assert!(
        gated.is_some(),
        "some dlg_keeper_<n> handler must fail fast on the forbidden flag"
    );
}

/// The `verb_forbid_gate` PackTest template is emitted for the first
/// `forbids_flags`-gated collect/interact objective: suppressed while the flag
/// is set, completing once it is cleared.
#[test]
fn verb_forbid_gate_packtest_emitted() {
    let out = built();
    let test = file(
        &out,
        "packtest-datapack/data/hello-world/test/verb_forbid_gate.mcfunction",
    );
    assert!(
        test.contains("forbids_flags suppresses objective `obj/lever`"),
        "template header names the objective: {test}"
    );
    // Phase order: set the forbidden flag → assert suppressed (0); clear it →
    // assert completed (1).
    let set = test
        .find("dw.f_blocked 1")
        .expect("suppression phase sets the flag");
    let suppressed = test
        .find("assert score @a[tag=dw_fbdtest,limit=1] dw.o_lever matches 0")
        .expect("suppressed assert present");
    let clear = test[set..]
        .find("dw.f_blocked 0")
        .map(|i| set + i)
        .expect("release phase clears the flag");
    let completed = test
        .rfind("assert score @a[tag=dw_fbdtest,limit=1] dw.o_lever matches 1")
        .expect("completed assert present");
    assert!(
        set < suppressed && suppressed < clear && clear < completed,
        "phases in order (set → assert 0 → clear → assert 1): {test}"
    );
}

/// Chained move origins (round-6, live-server proven): a SECOND consecutive
/// `move-actor` on the same actor must plan from the actor's current staged
/// location (the previous move's target), not its declared anchor. The island
/// case: the walker moved to the mouth at t=0; the t=260 move BACK to the
/// fire-pit — its declared anchor — used to plan fire-pit→fire-pit and
/// degenerate into a single-waypoint instant teleport, so the giant snapped
/// instead of visibly walking on camera. Both legs must be real multi-waypoint
/// walks; the second driver must also re-tp through intermediate cells.
#[test]
fn chained_moves_plan_from_last_staged_location() {
    // Actor anchored at the keeper stand; leg 1 → exit, leg 2 → back home.
    let quests = QUESTS_V06
        .replacen(
            r#""quests": ["#,
            r#""actors": [ { "id": "actor/walker", "entity": "minecraft:villager", "anchor": "anchor/keeper-stand" } ],
    "quests": ["#,
            1,
        )
        .replacen(
            r#"{ "type": "open-gate", "anchor": "anchor/door" },"#,
            r#"{ "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-actor", "actor": "actor/walker" },
            { "type": "move-actor", "actor": "actor/walker", "to_anchor": "anchor/exit" },
            { "type": "move-actor", "actor": "actor/walker", "to_anchor": "anchor/keeper-stand" },"#,
            1,
        );
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests,
        dialogue: dialogue_with_forbids(),
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&campaign, &prefabs);

    // Leg 2 (exit → keeper-stand) is a REAL walk: its per-tick driver carries
    // multiple waypoint teleports, not the degenerate single tp of a
    // start==target plan.
    let tick2 = file(
        &out,
        &format!("{FN_DIR}/ma_tick_walker_keeper_stand.mcfunction"),
    );
    let tp_lines = tick2
        .lines()
        .filter(|l| l.contains("run tp @e[tag=dw_pup_walker]"))
        .count();
    assert!(
        tp_lines > 1,
        "the second chained move-actor must be a multi-waypoint walk (planned from \
         the previous leg's target), got {tp_lines} waypoint tp(s):\n{tick2}"
    );

    // Same contract for chained move-npc legs (the shared-planner parity): the
    // keeper's second walk (exit → back to its stand) is a real walk too.
    let quests_npc = QUESTS_V06.replacen(
        r#"{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
              "on_arrive": [ { "type": "set-flag", "flag": "flag/arrived" } ] }"#,
        r#"{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
              "on_arrive": [ { "type": "set-flag", "flag": "flag/arrived" } ] },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/keeper-stand" }"#,
        1,
    );
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests_npc,
        dialogue: dialogue_with_forbids(),
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let out = build(&campaign, &prefabs);
    let tick2 = file(
        &out,
        &format!("{FN_DIR}/mv_tick_keeper_keeper_stand.mcfunction"),
    );
    let tp_lines = tick2
        .lines()
        .filter(|l| l.contains("run tp @e[tag=dw_npc_keeper]"))
        .count();
    assert!(
        tp_lines > 1,
        "the second chained move-npc must be a multi-waypoint walk, got {tp_lines} \
         waypoint tp(s):\n{tick2}"
    );
}
