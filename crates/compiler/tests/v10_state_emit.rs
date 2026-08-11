//! DSL v0.10 (spec-0031) end-to-end emission: a declared runtime datum becomes a
//! scoreboard objective on its declared holder, the three verbs write it, and the
//! numeric comparison reaches **every** gate consumer's emitted guard.
//!
//! The point of the file is the last clause. The comparison lives in the shared
//! gate rather than in the verb that first wanted it, and a surface that is
//! general in the type but reaches only one emitter is general on paper — so this
//! builds one campaign that gates on a datum at every consumer class the engine
//! has, and asserts the clause in each emitted guard. The binding count is
//! printed and asserted: `GateConsumer::COUNT` classes, all of them bound.
//!
//! Built on the `cast-ledger` fixture (two quests, a cast ledger, a dialogue
//! tree) with the stages raised to 0.10.0, a `state` section added, and — via a
//! private prefab copy, exactly as `v06_traps` does — an `anchor/trap` so the
//! trap consumer can be bound too.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::gate::{GateConsumer, for_each_gate};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "cast-ledger";

/// A private working directory per CALLER, not per file: the tests run in
/// parallel threads of one binary, and two of them sharing a scratch directory
/// is a race whose symptom is a missing file — an intermittent red, which is a
/// finding rather than something to re-run (CLAUDE.md).
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A private prefab copy whose `hello-room.json` gains an `anchor/trap` carrying
/// a restorable `trigger_block` — a gated trap's hardware obligation (`DW0363`)
/// holds for a numeric gate exactly as it does for a flag one.
///
/// Materialized **once per process** behind a `OnceLock`, not once per test. The
/// library is 76 files, the four tests here are read-only over it, and copying it
/// four times in parallel is IO this binary does not need — IO that is enough, on
/// a CI disk, to widen a time-of-check/time-of-use window in a sibling test
/// binary doing the same thing (see `effect_root_walkers::prefabs_with_trap`).
/// One initializer, every other caller blocking on it, nothing destructive
/// running beside a read.
fn patched_prefabs() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tmp("v10-state-prefabs");
        common::copy_dir_all(&common::prefabs_dir(), &dir);
        let path = dir.join("hello-room.json");
        let mut meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let anchors = meta
            .get_mut("anchors")
            .and_then(|a| a.as_object_mut())
            .unwrap();
        anchors.insert(
            "anchor/trap".to_string(),
            serde_json::json!({
                "pos": [5, 1, 6],
                "dispenser": [4, 1, 6],
                "trigger_block": "minecraft:stone_pressure_plate"
            }),
        );
        std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
        dir
    })
    .clone()
}

const WORLD: &str = r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "cast-ledger",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "difficulty": "normal",
    "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
  }
}"#;

/// Two data — one shared by the party, one held per player — read by a gate at
/// every consumer class the engine has, and written by all three verbs.
const QUESTS: &str = r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "cast-ledger",
  "stage": "quests",
  "content": {
    "state": [
      { "id": "state/toll", "scope": "party", "initial": 3,
        "note": "coins the keeper still wants before he stands aside" },
      { "id": "state/nerve", "scope": "player", "initial": 2,
        "note": "how much of your own nerve is left" }
    ],
    "shops": [
      {
        "id": "shop/keeper-wares",
        "anchor": "spawn",
        "title": "What the keeper will part with",
        "offers": [
          { "label": "A coin's worth of nerve",
            "tooltip": "Costs one coin.",
            "requires_state": [ { "state": "state/toll", "op": "at-least", "value": 1 } ],
            "effects": [
              { "type": "add-state", "state": "state/toll", "amount": -1 },
              { "type": "add-state", "state": "state/nerve", "amount": 1 }
            ] }
        ]
      }
    ],
    "traps": [
      {
        "id": "trap/step",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "lethality": "nonlethal",
        "requires_state": [ { "state": "state/toll", "op": "at-least", "value": 1 } ],
        "payload": [ { "type": "narrate", "text": "The plate clicks under your boot." } ]
      }
    ],
    "triggers": [
      {
        "id": "trigger/coin-slot",
        "at": "anchor/door",
        "on": { "on": "use" },
        "requires_state": [ { "state": "state/toll", "op": "at-least", "value": 1 } ],
        "effects": [ { "type": "add-state", "state": "state/toll", "amount": -1 } ]
      }
    ],
    "quests": [
      {
        "id": "quest/ask",
        "trigger": { "type": "campaign-start" },
        "objectives": [ { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" } ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "set-state", "state": "state/toll", "value": 0 },
            { "type": "add-state", "state": "state/nerve", "amount": -1 },
            { "type": "open-gate", "anchor": "anchor/door",
              "requires_state": [ { "state": "state/toll", "op": "at-most", "value": 0 } ] }
          ]
        },
        "on_complete": [ { "type": "narrate", "text": "The bolt slides back." } ],
        "cast": {
          "npc/keeper": [
            { "at": "anchor/keeper-stand", "doing": "barring the door with his body",
              "dialogue": "dlg/greeting" },
            { "at": "anchor/keeper-stand", "doing": "counting the coins you handed over",
              "dialogue": "dlg/greeting",
              "requires_state": [ { "state": "state/nerve", "op": "at-most", "value": 0 } ] }
          ],
          "npc/sleeper": {
            "at": "anchor/exit", "doing": "wine-drowned sleep against the wall",
            "dialogue": "none"
          }
        }
      },
      {
        "id": "quest/leave",
        "trigger": { "type": "quest-complete", "quest": "quest/ask" },
        "objectives": [
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "requires_state": [ { "state": "state/toll", "op": "at-most", "value": 0 } ] }
        ],
        "on_complete": [
          { "type": "clear-state", "state": "state/toll" },
          { "type": "campaign-complete" }
        ],
        "cast": {
          "npc/keeper": { "at": "anchor/keeper-stand", "doing": "standing aside, watching you go",
            "dialogue": "dlg/farewell" },
          "npc/sleeper": { "at": "anchor/exit", "doing": "dead asleep, past waking",
            "dialogue": "none" }
        }
      }
    ]
  }
}"#;

const DIALOGUE: &str = r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "cast-ledger",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      {
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {
            "id": "dlg/greeting",
            "text": "Halt. The door stays shut until the toll is paid.",
            "options": [
              { "label": "Stare him down.",
                "requires_state": [ { "state": "state/nerve", "op": "at-least", "value": 1 } ],
                "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] },
              { "label": "Pay what he asks.",
                "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] }
            ]
          },
          {
            "id": "dlg/farewell",
            "text": "Go on, then. The road is yours.",
            "options": [ { "label": "Farewell." } ]
          }
        ]
      },
      {
        "npc": "npc/sleeper",
        "root": "dlg/snore",
        "nodes": [ { "id": "dlg/snore", "text": "...mm. Not my watch.", "options": [] } ]
      }
    ]
  }
}"#;

/// Materialize the campaign and build it, returning `(output, binding ledger)`.
fn build(who: &str) -> (BuildOutput, delvewright_dsl::GateBinding) {
    let dir = tmp(&format!("v10-state-campaign-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(
            common::compiler_fixtures_dir().join("cast-ledger").join(f),
            dir.join(f),
        )
        .unwrap();
    }
    std::fs::write(dir.join("world.json"), WORLD).unwrap();
    std::fs::write(dir.join("quests.json"), QUESTS).unwrap();
    std::fs::write(dir.join("dialogue.json"), DIALOGUE).unwrap();

    let prefab_dir = patched_prefabs();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("v10-state parses");
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "v10-state must validate clean: {diags:#?}"
    );

    // The binding ledger for the assertions below: how many gates this campaign
    // actually has, per consumer class. A class at zero would make every
    // assertion about it vacuous, which is why it is checked rather than assumed.
    let mut per: BTreeMap<&'static str, usize> = BTreeMap::new();
    let binding = for_each_gate(&campaign, &mut |site, gate| {
        if !gate.requires_state.is_empty() {
            *per.entry(site.consumer.label()).or_default() += 1;
        }
    });
    println!("gate binding: {}", binding.summary());
    let unbound: Vec<&str> = GateConsumer::ALL
        .iter()
        .filter(|k| !per.contains_key(k.label()))
        .map(|k| k.label())
        .collect();
    assert!(
        unbound.is_empty(),
        "this fixture must carry a numeric gate at EVERY consumer class, or the assertions \
         below prove nothing about the classes it misses. Unbound: {unbound:?}"
    );

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(prefab_dir.join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let tree = CommandTree::v1_21_11();
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates");
    (out, binding)
}

fn body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// Everything emitted, as one string — for the assertions that only care that a
/// clause reached the datapack somewhere.
fn all_functions(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .map(|(p, b)| format!("### {p}\n{}\n", String::from_utf8_lossy(b)))
        .collect()
}

/// A declared datum becomes one scoreboard objective; a `party` datum is seeded
/// at world init and a `player` datum on each player's first tick.
#[test]
fn a_declared_datum_gets_an_objective_and_its_initial() {
    let (out, _) = build("seed");
    let setup = body(&out, "setup");
    assert!(
        setup.contains("scoreboard objectives add dw.s_toll dummy"),
        "the party datum needs its objective:\n{setup}"
    );
    assert!(
        setup.contains("scoreboard objectives add dw.s_nerve dummy"),
        "the per-player datum needs its objective too:\n{setup}"
    );
    // Party scope: seeded once, at world init, on the party holder.
    assert!(
        setup.contains("scoreboard players set #party dw.s_toll 3"),
        "a party datum is seeded in setup:\n{setup}"
    );
    assert!(
        !setup.contains("dw.s_nerve 2"),
        "a per-player datum cannot be seeded at world init — no player exists yet:\n{setup}"
    );
    // Player scope: seeded per player, once, driven from the tick.
    let seed = body(&out, "state_seed");
    assert!(
        seed.contains("scoreboard players set @s dw.s_nerve 2")
            && seed.contains("tag @s add dw_state"),
        "the per-player seed writes the initial then tags the player:\n{seed}"
    );
    assert!(
        body(&out, "tick")
            .contains("execute as @a[tag=!dw_state] run function cast-ledger:state_seed"),
        "the seed runs for any player who has not had it"
    );
}

/// The three verbs lower to `scoreboard players set/add/remove` against the
/// datum's declared holder. `clear-state` writes the declared initial rather than
/// resetting the score — an absent score would satisfy a `not-equals` gate.
#[test]
fn the_three_verbs_write_the_declared_holder() {
    let (out, _) = build("verbs");
    let all = all_functions(&out);
    assert!(
        all.contains("scoreboard players set #party dw.s_toll 0"),
        "set-state writes an absolute value:\n{all}"
    );
    assert!(
        all.contains("scoreboard players remove #party dw.s_toll 1"),
        "a negative add-state lowers by `remove`:\n{all}"
    );
    assert!(
        all.contains("scoreboard players set #party dw.s_toll 3"),
        "clear-state returns the datum to its declared initial (3), not to an absent score"
    );
}

/// **The generality claim, at the emitter.** The comparison reaches the emitted
/// guard of every gate consumer — not only the one that first wanted it.
#[test]
fn the_comparison_reaches_every_consumers_guard() {
    let (out, binding) = build("guards");
    println!("emission binding: {}", binding.summary());

    // 1. objective — the activation guard on the tick, a party predicate.
    let all = all_functions(&out);
    assert!(
        all.contains("if score #party dw.s_toll matches ..0"),
        "an objective's activation guard must carry the comparison:\n{all}"
    );

    // 2. effect — the per-effect gate wrapping the `open-gate` fill.
    let talk = body(&out, "complete_o_talk");
    assert!(
        talk.contains("if score #party dw.s_toll matches ..0 run fill"),
        "a gated effect's command must be wrapped in the comparison:\n{talk}"
    );

    // 3. trigger — the arming gate on the tick.
    assert!(
        all.contains(
            "if score #party dw.s_toll matches 1.. run function cast-ledger:trig_coin_slot"
        ),
        "a trigger's arming gate must carry the comparison:\n{all}"
    );

    // 4. trap — armed and disarmed on the numeric transition.
    assert!(
        all.contains("trap_gate_on_step") && all.contains("trap_gate_off_step"),
        "a numerically gated trap gets the same arming machinery a flag-gated one does"
    );
    assert!(
        all.contains(
            "unless score #party dw.s_toll matches 1.. run function cast-ledger:trap_gate_off_step"
        ),
        "the trap disarms the moment the comparison stops holding:\n{all}"
    );

    // 5. dialogue option — per-player availability, so a `player`-scoped datum
    //    is read from the acting player rather than from the party holder.
    let dmask = body(&out, "dmask_keeper_greeting");
    assert!(
        dmask.contains("if score @s dw.s_nerve matches 1.."),
        "a dialogue option's availability must carry the comparison, on `@s`:\n{dmask}"
    );
    let talk_fn = all_functions(&out);
    assert!(
        talk_fn.contains("unless score @s dw.s_nerve matches 1.. run return fail"),
        "the option's `/trigger` handler must be inert while the comparison fails"
    );

    // 6. cast placement — per-player scene selection.
    let cast = body(&out, "cast_keeper");
    assert!(
        cast.contains("if score @s dw.s_nerve matches ..0"),
        "a cast placement's branch gate must carry the comparison:\n{cast}"
    );
}

/// The negation is the same range under the other keyword, so a gate and its
/// shut-condition can never disagree about what the number means.
#[test]
fn a_shut_condition_is_the_same_range_negated() {
    let (out, _) = build("negate");
    let all = all_functions(&out);
    assert!(
        all.contains("if score #party dw.s_toll matches 1..")
            && all.contains("unless score #party dw.s_toll matches 1.."),
        "both readings of `at-least 1` must be the same range:\n{all}"
    );
}
