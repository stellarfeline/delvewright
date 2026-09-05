//! `DW0489` — two bodies the party clicks may not contest one crosshair.
//!
//! The owner's island finding, in its exact shape: two crew NPCs staged onto one
//! cell at the cave mouth, so a human could not aim at the one carrying the
//! decision. The campaign's own ledger said so — `quest/follow-the-smoke`
//! declares `npc/eurylochus` and `npc/antiphos` both `at: anchor/mouth` — and
//! every proof was green, because `DW0359` compares a body against an
//! *affordance* and skips every walker, and nothing else looked at where two
//! NPCs stand relative to each other.
//!
//! The fixture reproduces the structure rather than the map: two NPCs, one of
//! them walked onto the other's cell by a `move-npc` (so `DW0359`'s parked-body
//! rule excludes it, exactly as it excluded the island's crew), and a cast ledger
//! that declares them sharing a scene.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::crosshair::{DW_CROSSHAIR_CONTEST, threshold};
use delvewright_compiler::emit::{self, BuildFailure};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Diagnostic, RawCampaign, Severity, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// Two NPCs in the hello-room: the keeper on his stand, a scout at the exit
/// (`anchor/keeper-stand` is local `[5, 1, 4]`, `anchor/exit` is `[5, 1, 8]`).
const NPCS: &str = r#"{
  "dsl_version": "0.7.0", "campaign_id": "hello-world", "stage": "npcs",
  "content": { "npcs": [
    { "id": "npc/keeper", "name": "The Keeper", "role": "quest-giver",
      "area": "area/keep", "anchor": "anchor/keeper-stand", "base_entity": "minecraft:villager",
      "persona": { "archetype": "stoic gatekeeper", "speech_style": "Terse.", "motivation": "Guard the gate." } },
    { "id": "npc/scout", "name": "The Scout", "role": "flavor",
      "area": "area/keep", "anchor": "anchor/exit", "base_entity": "minecraft:villager",
      "persona": { "archetype": "restless scout", "speech_style": "Clipped.", "motivation": "Get out." } }
  ] }
}"#;

const QUEST_PLAN: &str = r#"{
  "dsl_version": "0.7.0", "campaign_id": "hello-world", "stage": "quest-plan",
  "content": { "quests": [
    { "id": "quest/one", "goal": "Speak with the Keeper.", "area": "area/keep",
      "npcs": ["npc/keeper"], "depends_on": [], "mandatory": true, "act": 1 },
    { "id": "quest/two", "goal": "Leave the keep.", "area": "area/keep",
      "npcs": ["npc/scout"], "depends_on": ["quest/one"], "mandatory": true, "act": 1 }
  ], "finale": "quest/two" }
}"#;

const DIALOGUE: &str = r#"{
  "dsl_version": "0.7.0", "campaign_id": "hello-world", "stage": "dialogue",
  "content": { "dialogues": [
    { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
      { "id": "dlg/greeting", "text": "Halt.", "options": [
        { "label": "Open the door.", "effects": [{ "type": "complete-objective", "objective": "obj/talk" }] } ] },
      { "id": "dlg/after", "text": "You still here?", "options": [] } ] },
    { "npc": "npc/scout", "root": "dlg/scout-root", "nodes": [
      { "id": "dlg/scout-root", "text": "Quiet, now.", "options": [] },
      { "id": "dlg/scout-later", "text": "We are clear.", "options": [] } ] }
  ] }
}"#;

/// The stage-5 document. `scout_to` is where the scout is walked when quest/one
/// completes — the whole variable of this fixture — and `cast_two` is the scene
/// that walk produces.
///
/// The `open-gate` on quest/one is load-bearing and not decoration: `anchor/exit`
/// is on the far side of `hello-room`'s barred doorway, so without it the party
/// can never reach quest/two's objective and the delve is refused (`DW0317`). It
/// fires FIRST in the bundle so the scout's own walk is planned through an open
/// door.
fn quests(scout_to: &str, cast_two: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.7.0", "campaign_id": "hello-world", "stage": "quests",
  "content": {{ "quests": [
    {{ "id": "quest/one", "trigger": {{ "type": "campaign-start" }},
       "objectives": [ {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }} ],
       "on_complete": [ {{ "type": "open-gate", "anchor": "anchor/door" }},
                        {{ "type": "move-npc", "npc": "npc/scout", "to_anchor": "{scout_to}" }} ],
       "cast": {{
         "npc/keeper": {{ "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" }},
         "npc/scout":  {{ "at": "anchor/exit", "doing": "watching the road", "dialogue": {{ "barks": ["Nothing yet."] }} }}
       }} }},
    {{ "id": "quest/two", "trigger": {{ "type": "quest-complete", "quest": "quest/one" }},
       "objectives": [ {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2 }} ],
       "on_complete": [ {{ "type": "campaign-complete" }} ],
       "cast": {cast_two} }}
  ] }}
}}"#
    )
}

/// Build the fixture; `Ok` carries the advisory diagnostics.
fn build(scout_to: &str, cast_two: &str) -> Result<Vec<Diagnostic>, BuildFailure> {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests(scout_to, cast_two),
        dialogue: DIALOGUE.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build_with_warnings(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map(|(_, warnings)| warnings)
}

/// The scene where the scout has walked onto the keeper's cell and the keeper's
/// right-click still opens a consequential tree.
const SHARED_CELL_WITH_ROOT: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
  "npc/scout":  { "at": "anchor/keeper-stand", "doing": "crowding the keeper", "dialogue": { "barks": ["Move along."] } }
}"#;

/// The same collision with nothing riding on either click.
const SHARED_CELL_ALL_BARKS: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": { "barks": ["Go."] } },
  "npc/scout":  { "at": "anchor/keeper-stand", "doing": "crowding the keeper", "dialogue": { "barks": ["Move along."] } }
}"#;

/// The scout stays where he started: four cells from the keeper.
const APART: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
  "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
}"#;

// --- red -------------------------------------------------------------------

/// The island's shape: a walked body lands on a speaker's cell and the build stops.
#[test]
fn two_npcs_on_one_cell_is_dw0489() {
    let err = build("anchor/keeper-stand", SHARED_CELL_WITH_ROOT)
        .expect_err("two crosshair targets on one cell must fail the build");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic, got {err:?}");
    };
    assert_eq!(code, "DW0489", "{message}");
    // The message must name BOTH bodies — that pair is the whole content of the bug.
    assert!(
        message.contains("npc/keeper") && message.contains("npc/scout"),
        "the message must name both NPCs: {message}"
    );
    assert!(
        message.contains("quest/two"),
        "the message must name the scene: {message}"
    );
    assert!(
        message.contains("COINCIDENT"),
        "coincident boxes must be called what they are: {message}"
    );
    assert!(
        message.contains("intangible"),
        "the message must forbid the intangible-body 'fix': {message}"
    );
}

/// `DW0359` cannot see this pair — the proof that the new code is not a
/// re-spelling of the old one. The scout is a walker, so the parked-body rule
/// excludes him; and an NPC's dialogue hitbox is not in the affordance list.
#[test]
fn the_eclipse_proof_is_silent_on_the_same_fixture() {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests("anchor/keeper-stand", SHARED_CELL_WITH_ROOT),
        dialogue: DIALOGUE.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let eclipse = delvewright_compiler::eclipse::check_body_eclipse(&plan)
        .expect("DW0359 has nothing to say about two bodies");
    assert!(
        !eclipse.iter().any(|d| d.code == "DW0359"),
        "DW0359 must stay silent here — it is why DW0489 exists: {eclipse:#?}"
    );
}

// --- the advisory tier -----------------------------------------------------

/// The same collision with nothing consequential riding on it is a staging note,
/// not a build failure: the bodies are ambiguous, but no beat is lost.
#[test]
fn a_contest_between_two_barks_is_the_warning_tier() {
    let warnings =
        build("anchor/keeper-stand", SHARED_CELL_ALL_BARKS).expect("barks must not fail the build");
    let d = warnings
        .iter()
        .find(|d| d.code == DW_CROSSHAIR_CONTEST)
        .expect("an inconsequential contest must still be reported");
    assert_eq!(d.severity, Severity::Warning);
    assert_eq!(d.stage, "quests");
    assert!(
        d.message.contains("advisory") || d.message.contains("rather than a build failure"),
        "the warning must say why it is only a warning: {}",
        d.message
    );
}

// --- green -----------------------------------------------------------------

/// Bodies that keep their distance raise nothing — the no-false-positive guard.
#[test]
fn npcs_four_cells_apart_are_silent() {
    let warnings = build("anchor/exit", APART).expect("a well-staged scene must build");
    assert!(
        !warnings.iter().any(|d| d.code == DW_CROSSHAIR_CONTEST),
        "a clean scene must raise no contest: {warnings:#?}"
    );
}

/// The threshold two humanoid bodies have to clear, stated where a reader of the
/// fixtures will find it: 1.2 blocks, and the hello-room's two anchors are four
/// cells apart, which is why the green case is green.
#[test]
fn the_humanoid_threshold_is_one_point_two_blocks() {
    assert!((threshold(0.6, 0.6) - 1.2).abs() < 1e-9);
    assert!(4.0 > threshold(0.6, 0.6));
}
