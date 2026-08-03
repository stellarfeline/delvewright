//! `DW0410` — the effect-timeline gate proof (round-8 island playtest).
//!
//! The defect: one `sequence` sealed the island's boulder gate at `at_ticks: 460`
//! and walked the giant across the sealed region at `at_ticks: 700`. The compiler
//! proved that walk on the *open* world — the occupancy model treats every gate
//! as passable — so it shipped green and the giant stepped through solid basalt
//! on the live server.
//!
//! Inside one timeline the gate state is statically knowable, so it is now
//! proven. These tests pin both halves: the error fires when the timeline's own
//! `close-gate` makes the walk impossible, and it stays silent in every case
//! where the ordering does *not* establish a seal — the no-false-certainty stance
//! `crate::timeline` inherits from `crate::continuity`.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit;
use delvewright_compiler::nav;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_compiler::timeline;
use delvewright_dsl::{Campaign, QuestEffect, RawCampaign, parse_campaign};

/// A hello-world `quests` doc carrying a stage-5 actor plus a caller-supplied
/// `on_complete` body (raw JSON array contents, no surrounding brackets).
///
/// The actor stands at `anchor/keeper-stand` (room side, local z=4); the walks
/// below all target `anchor/exit` (local z=8), which is only reachable through
/// the `anchor/door` gate region (local x=4..5, z=6). Sealing the door therefore
/// disconnects the two, which is exactly the island's geometry in miniature.
fn quests_doc(on_complete: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "actors": [
      {{ "id": "actor/ram", "entity": "minecraft:sheep", "anchor": "anchor/keeper-stand" }}
    ],
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
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "set-flag", "flag": "flag/sealed" }}
          ]
        }},
        "on_complete": [ {on_complete} ]
      }}
    ]
  }}
}}"#
    )
}

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
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap()
}

/// Run the full build over a campaign whose `on_complete` is `body`, returning the
/// diagnostic code on failure and `None` on a clean build.
fn build_code(body: &str) -> Option<String> {
    let c = parse_hw(&quests_doc(body));
    let prefabs = prefabs();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let tree = CommandTree::v1_21_11();
    match emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    ) {
        Ok(_) => None,
        Err(emit::BuildFailure::Diagnostic { code, .. }) => Some(code.to_string()),
        Err(other) => panic!("unexpected non-diagnostic build failure: {other:?}"),
    }
}

/// The campaign validates cleanly (so a build failure below is the nav proof
/// talking, never a schema slip in the fixture text).
fn assert_validates(body: &str) {
    let c = parse_hw(&quests_doc(body));
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let d = delvewright_dsl::validate_campaign_with(&c, &items, &prefabs(), &entities);
    assert!(d.is_empty(), "fixture must validate cleanly: {d:#?}");
}

// --- DW0410: the island defect ---------------------------------------------

/// The round-8 island shape: `close-gate` at a lower `at_ticks` than a
/// `move-actor` whose only route crosses that gate. The walk is impossible from
/// the moment the gate shuts, and the compiler now says so instead of proving it
/// on the open world.
#[test]
fn walk_after_close_gate_in_the_same_sequence_is_dw0410() {
    let body = r#"{ "type": "sequence", "steps": [
          { "at_ticks": 460, "effects": [ { "type": "close-gate", "anchor": "anchor/door" } ] },
          { "at_ticks": 700, "effects": [
              { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" } ] }
       ] },
       { "type": "campaign-complete" }"#;
    assert_validates(body);
    assert_eq!(
        build_code(body).as_deref(),
        Some("DW0410"),
        "a walk across a gate this timeline already sealed must fail the build"
    );
}

/// The same conflict spelled as plain effect-list order inside one bundle: every
/// effect in a bundle runs in declared order in one tick, so a `close-gate` at a
/// lower index has provably fired before a later `move-actor` starts walking.
#[test]
fn walk_after_close_gate_in_the_same_effect_list_is_dw0410() {
    let body = r#"{ "type": "close-gate", "anchor": "anchor/door" },
       { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" },
       { "type": "campaign-complete" }"#;
    assert_validates(body);
    assert_eq!(build_code(body).as_deref(), Some("DW0410"));
}

/// `move-npc` carries the identical obligation — same planner, same proof.
#[test]
fn move_npc_after_close_gate_is_dw0410() {
    let body = r#"{ "type": "close-gate", "anchor": "anchor/door" },
       { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" },
       { "type": "campaign-complete" }"#;
    assert_validates(body);
    assert_eq!(build_code(body).as_deref(), Some("DW0410"));
}

/// The diagnostic has to be *actionable*: it names the verb, the mover and the
/// gate anchor the author has to look at, not a bare pair of coordinates.
#[test]
fn dw0410_message_names_the_verb_mover_and_gate() {
    let c = parse_hw(&quests_doc(
        r#"{ "type": "close-gate", "anchor": "anchor/door" },
           { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" },
           { "type": "campaign-complete" }"#,
    ));
    let prefabs = prefabs();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let world = nav::World::from_plan(&plan, &structures);
    let err =
        nav::plan_actor_moves(&plan, &world).expect_err("the sealed door leaves the walk no route");
    assert_eq!(err.code, nav::DW_GATE_TIMELINE);
    for needle in ["move-actor", "actor/ram", "anchor/door", "close-gate"] {
        assert!(
            err.message.contains(needle),
            "DW0410 message must name `{needle}`: {}",
            err.message
        );
    }
}

// --- silence where the ordering proves nothing -------------------------------

/// Walk first, seal after: the walk is complete before the gate shuts, so there
/// is nothing to complain about. (This is the ordering the island *meant*.)
#[test]
fn walk_before_close_gate_builds_clean() {
    let body = r#"{ "type": "sequence", "steps": [
          { "at_ticks": 100, "effects": [
              { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" } ] },
          { "at_ticks": 700, "effects": [ { "type": "close-gate", "anchor": "anchor/door" } ] }
       ] },
       { "type": "campaign-complete" }"#;
    assert_validates(body);
    assert_eq!(build_code(body), None);
}

/// Declaration order is NOT timeline order inside a `sequence` — the tick offsets
/// are. The seal is declared first but fires *later*, so the walk is clear.
/// Getting this backwards would resurrect the bug in mirror image.
#[test]
fn sequence_order_follows_at_ticks_not_declaration_order() {
    let body = r#"{ "type": "sequence", "steps": [
          { "at_ticks": 700, "effects": [ { "type": "close-gate", "anchor": "anchor/door" } ] },
          { "at_ticks": 100, "effects": [
              { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" } ] }
       ] },
       { "type": "campaign-complete" }"#;
    assert_validates(body);
    assert_eq!(build_code(body), None);
}

/// A later `open-gate` reopens the region: the walk relies on a gate an earlier
/// effect opened, which is exactly the symmetric case the model must permit.
#[test]
fn close_then_open_then_walk_builds_clean() {
    let body = r#"{ "type": "sequence", "steps": [
          { "at_ticks": 0,   "effects": [ { "type": "close-gate", "anchor": "anchor/door" } ] },
          { "at_ticks": 200, "effects": [ { "type": "open-gate",  "anchor": "anchor/door" } ] },
          { "at_ticks": 700, "effects": [
              { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" } ] }
       ] },
       { "type": "campaign-complete" }"#;
    assert_validates(body);
    assert_eq!(build_code(body), None);
}

/// A **conditional** `close-gate` may never fire, so it proves no seal. Treating
/// it as one would invent a build error for a campaign that works — the failure
/// mode the no-false-certainty rule exists to prevent.
#[test]
fn conditional_close_gate_seals_nothing() {
    let body = r#"{ "type": "close-gate", "anchor": "anchor/door", "requires_flags": ["flag/sealed"] },
       { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" },
       { "type": "campaign-complete" }"#;
    assert_validates(body);
    assert_eq!(build_code(body), None);
}

/// Cross-bundle order is unknowable and is never guessed: a `close-gate` fired
/// from a *different* bundle (here the objective bundle) says nothing about a
/// walk in `on_complete`. The DAG-causal model (`DW0311`) is what covers the
/// player's forced route; this proof deliberately stays quiet.
#[test]
fn close_gate_in_another_bundle_does_not_seal_this_timeline() {
    let quests = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "actors": [
      { "id": "actor/ram", "entity": "minecraft:sheep", "anchor": "anchor/keeper-stand" }
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
          "obj/talk": [ { "type": "close-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [
          { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" },
          { "type": "campaign-complete" }
        ]
      }
    ]
  }
}"#;
    let c = parse_hw(quests);
    let prefabs = prefabs();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let world = nav::World::from_plan(&plan, &structures);
    assert!(
        nav::plan_actor_moves(&plan, &world).is_ok(),
        "a seal in another bundle must not be attributed to this timeline"
    );
}

// --- the timeline state machine itself ---------------------------------------

/// Every effect the walk yields, paired with whether its timeline had sealed
/// anything at that point — the raw material both planners consume.
fn seal_flags(c: &Campaign) -> Vec<(String, bool)> {
    let prefabs = prefabs();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    timeline::walk(&plan)
        .into_iter()
        .map(|(e, state)| {
            let name = match e {
                QuestEffect::CloseGate { .. } => "close-gate",
                QuestEffect::OpenGate { .. } => "open-gate",
                QuestEffect::MoveActor { .. } => "move-actor",
                QuestEffect::Sequence { .. } => "sequence",
                QuestEffect::CampaignComplete { .. } => "campaign-complete",
                _ => "other",
            };
            (name.to_string(), !state.is_empty())
        })
        .collect()
}

/// An effect never sees its own seal — only earlier ones. The `close-gate` itself
/// is reported unsealed; everything after it in the same list is sealed.
#[test]
fn state_is_as_of_the_effect_not_after_it() {
    let c = parse_hw(&quests_doc(
        r#"{ "type": "close-gate", "anchor": "anchor/door" },
           { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" },
           { "type": "campaign-complete" }"#,
    ));
    let flags = seal_flags(&c);
    let close = flags.iter().position(|(n, _)| n == "close-gate").unwrap();
    let mv = flags.iter().position(|(n, _)| n == "move-actor").unwrap();
    assert!(close < mv, "declared order preserved");
    assert!(!flags[close].1, "the close-gate itself sees no seal yet");
    assert!(flags[mv].1, "the following walk sees the seal");
}

/// The walk yields exactly the canonical effect pre-order `nav::all_effects` used
/// to build by hand — the two are one traversal, so alignment cannot drift.
#[test]
fn walk_yields_every_effect_including_nested_ones() {
    let c = parse_hw(&quests_doc(
        r#"{ "type": "sequence", "steps": [
              { "at_ticks": 0, "effects": [ { "type": "close-gate", "anchor": "anchor/door" } ] },
              { "at_ticks": 40, "effects": [
                  { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" } ] }
           ] },
           { "type": "campaign-complete" }"#,
    ));
    let flags = seal_flags(&c);
    let names: Vec<&str> = flags.iter().map(|(n, _)| n.as_str()).collect();
    // The objective bundle (open-gate, set-flag), then `on_complete`: the sequence
    // node, its two nested steps in declaration order, then campaign-complete.
    assert_eq!(
        names,
        vec![
            "open-gate",
            "other", // set-flag
            "sequence",
            "close-gate",
            "move-actor",
            "campaign-complete"
        ],
        "pre-order: each effect ahead of its nested effects"
    );
    // The sequence node itself starts its timeline unsealed, and so does its
    // `close-gate` step; only the walk that follows the seal sees it.
    assert!(!flags[2].1 && !flags[3].1 && flags[4].1);
}
