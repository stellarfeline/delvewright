//! Wave support machinery is emitted for EVERY wave the pack calls, and no
//! emitted `function <ns>:…` call may dangle (`DW0497`).
//!
//! **The island incident (round 21, `nobodys-cave-island`).** `wave/storm-surf`
//! was fired from a top-level `on_objective_complete` bundle and got its full
//! machinery. `wave/storm-shore` and `wave/storm-fire` were fired from step 7 of
//! a `sequence` — and got *nothing*: no `spawn_…`, no census, no brand, no kill
//! reward. The `seq_…` function still emitted `function <ns>:spawn_storm_shore`,
//! so the calls silently no-op'd at runtime and two of the three storm waves
//! never spawned. The compiler's own generated census PackTest — which walks
//! `waves[]`, not the effect tree — was the only thing that noticed
//! (`Expected #wcen_d to have a score on tick 0`, 41/42).
//!
//! Two halves, both pinned here: the emission set is derived from the same deep
//! traversal that emits the calls, and a build-integrity pass reads the finished
//! tree so the whole class (any future emitter split-brain, not only waves) is a
//! loud build error instead of a missing enemy.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The hello-world campaign with the named stage documents replaced.
fn parse_hw_with(overrides: &[(&str, String)]) -> Campaign {
    let get = |name: &str| {
        overrides
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| read_hw(name))
    };
    parse_campaign(&RawCampaign {
        world: get("world.json"),
        npcs: get("npcs.json"),
        classes: get("classes.json"),
        quest_plan: get("quest-plan.json"),
        quests: get("quests.json"),
        dialogue: get("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("campaign parses")
}

fn try_build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
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
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn build_ok(campaign: &Campaign) -> BuildOutput {
    try_build(campaign).expect("build succeeds")
}

fn fn_body(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/hello-world/function/{name}.mcfunction");
    let bytes = out
        .get(&path)
        .unwrap_or_else(|| panic!("no emitted function `{name}`"));
    String::from_utf8(bytes.clone()).unwrap()
}

// --- the fixture: a wave fired ONLY from inside a `sequence` step ------------

/// hello-world's quests with a **live-threat** wave whose only `spawn-wave` sits
/// inside a `sequence` step — the island's `wave/storm-shore` shape, minimized.
///
/// Deliberately carries **no `kill` objective**: `plan::wave_area`'s defensive
/// third resolution rule (a wave named by a `kill`) would otherwise resolve the
/// area anyway and hide the defect. A kill-less wave is first-class content
/// (spec-0008 §4 live threat), and it is exactly the shape that lost its
/// machinery.
const SEQ_WAVE_QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "waves": [
      { "id": "wave/ambush", "anchor": "anchor/exit",
        "mobs": [ { "entity": "minecraft:zombie", "count": 1 } ] }
    ],
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
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "sequence", "steps": [
              { "at_ticks": 20, "effects": [
                { "type": "spawn-wave", "wave": "wave/ambush" }
              ] }
            ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// The control: the same wave fired from the TOP-LEVEL bundle. This shape always
/// worked; it is here so the pair isolates the nesting as the variable.
fn top_level_quests() -> String {
    SEQ_WAVE_QUESTS.replace(
        r#"{ "type": "sequence", "steps": [
              { "at_ticks": 20, "effects": [
                { "type": "spawn-wave", "wave": "wave/ambush" }
              ] }
            ] }"#,
        r#"{ "type": "spawn-wave", "wave": "wave/ambush" }"#,
    )
}

/// Every per-wave support function the emitter owns (`docs/reference/compiler.md`,
/// the `kill`/`spawn-wave` row + the census probe).
const MACHINERY: [&str; 6] = [
    "spawn_ambush",
    "wave_census_ambush",
    "wave_census_one_ambush",
    "wave_brand_ambush",
    "wave_unbrand_ambush",
    "k_reward_ambush",
];

#[test]
fn top_level_wave_gets_its_machinery() {
    let out = build_ok(&parse_hw_with(&[("quests.json", top_level_quests())]));
    for name in MACHINERY {
        assert!(
            out.contains_key(&format!(
                "datapack/data/hello-world/function/{name}.mcfunction"
            )),
            "expected `{name}` in the emitted pack"
        );
    }
}

/// **The defect.** A wave fired from a `sequence` step is a wave the pack calls,
/// so it must get the same machinery as one fired top-level.
#[test]
fn sequence_fired_wave_gets_its_machinery() {
    let out = build_ok(&parse_hw_with(&[(
        "quests.json",
        SEQ_WAVE_QUESTS.to_string(),
    )]));
    // The call really is compiled — that is what makes the missing target fatal.
    let seq = out
        .iter()
        .filter(|(p, _)| p.contains("/function/seq_"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        seq.contains("function hello-world:spawn_ambush"),
        "the sequence step must compile a spawn call:\n{seq}"
    );
    for name in MACHINERY {
        assert!(
            out.contains_key(&format!(
                "datapack/data/hello-world/function/{name}.mcfunction"
            )),
            "wave fired from a `sequence` step is missing `{name}` — the emitted \
             `function hello-world:spawn_ambush` call dangles and the wave never spawns"
        );
    }
}

/// The census probe the harness reads must name the sequence-fired wave, not just
/// the top-level ones: the generated PackTest walks `waves[]`, so a wave in
/// `waves[]` with no census function is a red the content cannot fix.
#[test]
fn sequence_fired_wave_census_walks_its_own_tag() {
    let out = build_ok(&parse_hw_with(&[(
        "quests.json",
        SEQ_WAVE_QUESTS.to_string(),
    )]));
    let census = fn_body(&out, "wave_census_ambush");
    assert!(
        census.contains(
            "execute as @e[tag=dw_wave_ambush] run function \
                         hello-world:wave_census_one_ambush"
        ),
        "the census must walk the wave's own tag:\n{census}"
    );
}

/// **Attribution.** The countdown is a measurement of what still stands, so it
/// clears whether a body was cut down or fell into a lethal volume. That is the
/// point of it, and it is also why nothing downstream could tell those two apart:
/// on the gallery's own bot ladder, two of `wave/muster`'s three bodies died to
/// the world — one withered in `lethal/east-pit`, one fell — and the inverted
/// floor gate reported that the unassisted bot had beaten an `elite` encounter on
/// its first attempt.
///
/// `minecraft:player_killed_entity` is the only credit vanilla issues, and
/// `k_reward_<wave>` is where it lands, so the ledger is the advancement's own
/// count. Four sites carry it and all four are asserted here, because three of
/// them are silent when they are wrong: a missing `setup` seed renders the census
/// line with an empty field (a line no parser matches, which reads as "no
/// answer"), a missing `spawn_<wave>` zero carries the previous cohort's credits
/// into the new one, and a census that does not state the number is a measurement
/// nobody can read.
#[test]
fn credited_kills_are_ledgered_per_seating_and_stated_on_the_census() {
    let out = build_ok(&parse_hw_with(&[("quests.json", top_level_quests())]));
    let holder = "#wcred_ambush";

    let setup = fn_body(&out, "setup");
    assert!(
        setup.contains(&format!("scoreboard players set {holder} dw.sys 0")),
        "world init must seed the credited-kill ledger — the census renders it as a \
         `score` component, and a holder with no score renders as the empty \
         string:\n{setup}"
    );

    let spawn = fn_body(&out, "spawn_ambush");
    assert!(
        spawn.contains(&format!("scoreboard players set {holder} dw.sys 0")),
        "a fresh seating must start a fresh attribution, or a re-seat's own \
         uncredited `kill @e[tag=…]` sweep is counted against the next cohort:\n{spawn}"
    );

    let reward = fn_body(&out, "k_reward_ambush");
    assert!(
        reward.contains(&format!("scoreboard players add {holder} dw.sys 1")),
        "the credit is recorded where vanilla issues it — the \
         `player_killed_entity` advancement's reward:\n{reward}"
    );

    let census = fn_body(&out, "wave_census_ambush");
    assert!(
        census.contains(&format!("\"name\":\"{holder}\"")),
        "the census summary line must state the credited count, or the floor gate \
         has no way to tell a cohort the party felled from one the world \
         killed:\n{census}"
    );
    // Field order is the wire format the harness parses whole: the credited count
    // is LAST, after `damaged`.
    let seq = census.find("\"#wcen_seq\"").expect("seq field");
    let dam = census.find("\"#wcen_d\"").expect("damaged field");
    let cred = census
        .find(&format!("\"{holder}\""))
        .expect("credited field");
    assert!(
        seq < dam && dam < cred,
        "the census line's fields are positional; credited comes last:\n{census}"
    );
}
