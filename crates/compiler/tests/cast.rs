//! The NPC scene ledger (spec-0020): the four build proofs, the `"unchanged"`
//! sugar, the staleness lint and the pre-0.7 deprecation window.
//!
//! Fixture shape: hello-world's world/classes, plus two NPCs and a two-quest DAG
//! (`quest/one` → `quest/two`) so a ledger has a *previous* node to be consistent
//! with. Every test drives `cast::check_cast` on a parsed campaign, so the proofs
//! are exercised exactly as `delvec validate` runs them.

mod common;

use delvewright_compiler::cast;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        common::repo_root()
            .join("crates/dsl/fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

/// Two NPCs: the keeper at his stand, a scout at the exit.
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

/// A two-quest stage-5 document whose `cast` blocks are supplied per test.
fn quests(cast_one: &str, cast_two: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.7.0", "campaign_id": "hello-world", "stage": "quests",
  "content": {{ "quests": [
    {{ "id": "quest/one", "trigger": {{ "type": "campaign-start" }},
       "objectives": [ {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }} ],
       "on_complete": [],
       "cast": {cast_one} }},
    {{ "id": "quest/two", "trigger": {{ "type": "quest-complete", "quest": "quest/one" }},
       "objectives": [ {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2 }} ],
       "on_complete": [ {{ "type": "campaign-complete" }} ],
       "cast": {cast_two} }}
  ] }}
}}"#
    )
}

fn campaign(cast_one: &str, cast_two: &str) -> Campaign {
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests(cast_one, cast_two),
        dialogue: DIALOGUE.to_string(),
        world_edits: None,
    })
    .expect("fixture must parse")
}

fn codes(c: &Campaign) -> Vec<String> {
    cast::check_cast(c).iter().map(|d| d.code.clone()).collect()
}

/// Both NPCs stand where world init leaves them; both declare a scene.
const FULL_ONE: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" },
  "npc/scout":  { "at": "anchor/exit", "doing": "watching the road", "dialogue": { "barks": ["Nothing yet.", "Still nothing."] } }
}"#;
const FULL_TWO: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
  "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
}"#;

/// A complete, consistent ledger raises nothing.
#[test]
fn complete_consistent_ledger_is_clean() {
    let diags = cast::check_cast(&campaign(FULL_ONE, FULL_TWO));
    assert!(diags.is_empty(), "expected a clean ledger, got: {diags:#?}");
}

// --- proof 1: completeness -------------------------------------------------

/// A quest that omits a live NPC fails, naming the NPC and the quest.
#[test]
fn proof1_missing_live_npc_is_dw0460() {
    let one = r#"{ "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" } }"#;
    let c = campaign(one, FULL_TWO);
    let diags = cast::check_cast(&c);
    let d = diags
        .iter()
        .find(|d| d.code == "DW0460")
        .expect("unaccounted NPC must raise DW0460");
    assert!(d.message.contains("npc/scout"), "{}", d.message);
    assert!(d.message.contains("quest/one"), "{}", d.message);
}

// --- proof 2: placement consistency ---------------------------------------

/// A declared `at` the effect history contradicts fails, citing both anchors —
/// the round-8 "crew forgotten in the alcoves" defect, caught at compile time.
#[test]
fn proof2_placement_contradiction_is_dw0461() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/door", "doing": "gone ahead", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    let c = campaign(FULL_ONE, two);
    let diags = cast::check_cast(&c);
    let d = diags
        .iter()
        .find(|d| d.code == "DW0461")
        .expect("a contradicted placement must raise DW0461");
    assert!(d.message.contains("anchor/door"), "{}", d.message);
    assert!(d.message.contains("anchor/keeper-stand"), "{}", d.message);
    // The fixed declaration passes.
    assert!(
        !codes(&campaign(FULL_ONE, FULL_TWO)).contains(&"DW0461".to_string()),
        "the corrected declaration must pass"
    );
}

/// Declaring somebody `"dead"` while their body still stands is the same lie in
/// the other direction.
#[test]
fn proof2_declared_dead_while_still_staged_is_dw0461() {
    let two = r#"{
      "npc/keeper": "dead",
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    let c = campaign(FULL_ONE, two);
    let d = cast::check_cast(&c)
        .into_iter()
        .find(|d| d.code == "DW0461")
        .expect("a declared-dead-but-standing NPC must raise DW0461");
    assert!(d.message.contains("anchor/keeper-stand"), "{}", d.message);
}

// --- proof 4: branch honesty ----------------------------------------------

/// A campaign whose scout is despawned from a *dialogue option* — so its position
/// depends on what the player said — is branch-divergent.
fn branchy(cast_one: &str, cast_two: &str) -> Campaign {
    let dialogue = DIALOGUE.replace(
        r#"{ "id": "dlg/greeting", "text": "Halt.", "options": [
        { "label": "Open the door.", "effects": [{ "type": "complete-objective", "objective": "obj/talk" }] } ] }"#,
        r#"{ "id": "dlg/greeting", "text": "Halt.", "options": [
        { "label": "Open the door.", "effects": [{ "type": "complete-objective", "objective": "obj/talk" }, { "type": "spawn-npc", "npc": "npc/scout" }] } ] }"#,
    );
    assert_ne!(dialogue, DIALOGUE, "the branch patch must apply");
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests(cast_one, cast_two),
        dialogue,
        world_edits: None,
    })
    .expect("fixture must parse")
}

/// A branch-divergent position with a single flat declaration fails.
#[test]
fn proof4_flat_declaration_on_a_branchy_npc_is_dw0462() {
    let d = cast::check_cast(&branchy(FULL_ONE, FULL_TWO))
        .into_iter()
        .find(|d| d.code == "DW0462")
        .expect("a flat cast on a branch-divergent NPC must raise DW0462");
    assert!(d.message.contains("npc/scout"), "{}", d.message);
    assert!(d.message.contains("dialogue option"), "{}", d.message);
}

/// Per-branch casts pass. Note both quests need them: continuity's exclusion is
/// campaign-global, so an NPC whose lifecycle any branch touches is
/// branch-divergent for the whole story, not just after the fork.
#[test]
fn proof4_per_branch_casts_pass() {
    let one = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" },
      "npc/scout":  [
        { "at": "anchor/exit", "doing": "watching the road", "dialogue": "dlg/scout-root", "forbids_flags": [] },
        { "at": "offstage" }
      ]
    }"#;
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
      "npc/scout":  [
        { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later", "requires_flags": [] },
        { "at": "offstage" }
      ]
    }"#;
    let c = branchy(one, two);
    let codes = codes(&c);
    assert!(
        !codes.contains(&"DW0462".to_string()),
        "per-branch casts must satisfy proof 4, got: {codes:?}"
    );

    // Each on-stage branch gets its OWN selector clause carrying its own gate:
    // a per-branch cast that only ever dispatched its first branch would be a
    // ledger that lies at runtime while passing every build proof.
    let casts = cast::npc_casts(&c);
    let scout = &casts["npc/scout"];
    assert_eq!(
        scout.by_quest.len(),
        2,
        "one clause per on-stage branch (the `offstage` branch has no dialogue \
         and so no clause): {scout:#?}"
    );
    assert_eq!(scout.scenes.len(), 2, "two distinct roots, two scenes");
}

/// A branch gate on a placement reaches the emitted selector clause, so the
/// branch really is dispatched at runtime rather than declared and dropped.
#[test]
fn per_branch_gates_reach_the_selector() {
    let one = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" },
      "npc/scout":  [
        { "at": "anchor/exit", "doing": "watching the road", "dialogue": "dlg/scout-root" },
        { "at": "anchor/exit", "doing": "warned, and grim", "dialogue": "dlg/scout-later",
          "requires_flags": ["flag/warned"] }
      ]
    }"#;
    let c = branchy(one, FULL_TWO);
    let casts = cast::npc_casts(&c);
    let scout = &casts["npc/scout"];
    let gated = scout
        .by_quest
        .iter()
        .find(|cl| !cl.requires_flags.is_empty())
        .expect("the gated branch must produce its own clause");
    assert_eq!(gated.requires_flags, vec!["flag/warned".to_string()]);
    assert_eq!(
        gated.scene, 2,
        "the gated branch must select its own scene, not the fallback's"
    );
}

// --- DW0463 / DW0464: shape and refs --------------------------------------

/// An on-stage placement that says nothing about `doing`/`dialogue`.
#[test]
fn missing_forcing_function_fields_are_dw0463() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    let diags = cast::check_cast(&campaign(FULL_ONE, two));
    let hits: Vec<_> = diags.iter().filter(|d| d.code == "DW0463").collect();
    assert_eq!(
        hits.len(),
        2,
        "both `doing` and `dialogue` must be demanded"
    );
}

/// An absent NPC cannot answer a right-click.
#[test]
fn dialogue_on_an_absent_npc_is_dw0463() {
    let two = r#"{
      "npc/keeper": { "at": "offstage", "doing": "gone", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    assert!(codes(&campaign(FULL_ONE, two)).contains(&"DW0463".to_string()));
}

/// A dialogue root that is not a node of that NPC's tree.
#[test]
fn dangling_dialogue_root_is_dw0464() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching", "dialogue": "dlg/scout-later" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    let d = cast::check_cast(&campaign(FULL_ONE, two))
        .into_iter()
        .find(|d| d.code == "DW0464")
        .expect("a root from another NPC's tree must raise DW0464");
    assert!(d.message.contains("dlg/scout-later"), "{}", d.message);
}

/// An empty bark pool is silence dressed as an answer.
#[test]
fn empty_bark_pool_is_dw0464() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching", "dialogue": { "barks": [] } },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    assert!(codes(&campaign(FULL_ONE, two)).contains(&"DW0464".to_string()));
}

/// A cast entry for an NPC stage 2 never declared.
#[test]
fn unknown_npc_in_cast_is_dw0464() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" },
      "npc/ghost":  { "at": "anchor/exit", "doing": "haunting", "dialogue": "none" }
    }"#;
    let d = cast::check_cast(&campaign(FULL_ONE, two))
        .into_iter()
        .find(|d| d.code == "DW0464")
        .expect("an undeclared NPC must raise DW0464");
    assert!(d.message.contains("npc/ghost"), "{}", d.message);
}

// --- DW0465: the deprecation window ---------------------------------------

/// A pre-0.7 campaign with no ledger keeps building, with one warning.
#[test]
fn pre_07_campaign_without_a_ledger_warns_dw0465() {
    let c = parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: hw("quests.json"),
        dialogue: hw("dialogue.json"),
        world_edits: None,
    })
    .unwrap();
    let diags = cast::check_cast(&c);
    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(diags[0].code, "DW0465");
    assert_eq!(
        diags[0].severity,
        delvewright_dsl::Severity::Warning,
        "the window must warn, never fail the build"
    );
}

// --- DW0466 / DW0467: the `"unchanged"` sugar and the staleness lint -------

/// `"unchanged"` carries the previous scene forward.
#[test]
fn unchanged_carries_the_previous_scene_forward() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "still watching", "dialogue": "unchanged" }
    }"#;
    let c = campaign(FULL_ONE, two);
    assert!(
        !codes(&c).contains(&"DW0466".to_string()),
        "a second-appearance `unchanged` is legal"
    );
    let casts = cast::npc_casts(&c);
    let scout = &casts["npc/scout"];
    assert_eq!(
        scout.scenes.len(),
        1,
        "`unchanged` must emit no new scene: {scout:#?}"
    );
    assert_eq!(
        scout
            .by_quest
            .iter()
            .map(|c| (c.quest.as_str(), c.scene))
            .collect::<Vec<_>>(),
        vec![("quest/one", 1), ("quest/two", 1)],
        "both quests must resolve to the carried-forward scene"
    );
}

/// `"unchanged"` at an NPC's first appearance has nothing to carry.
#[test]
fn unchanged_at_first_appearance_is_dw0466() {
    let one = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" },
      "npc/scout":  { "at": "anchor/exit", "doing": "watching the road", "dialogue": "unchanged" }
    }"#;
    let d = cast::check_cast(&campaign(one, FULL_TWO))
        .into_iter()
        .find(|d| d.code == "DW0466")
        .expect("`unchanged` with no previous scene must raise DW0466");
    assert!(d.message.contains("npc/scout"), "{}", d.message);
}

/// An NPC whose dialogue never changes across the whole story is a design smell:
/// the "one tree from beginning to end" shape the ledger exists to surface.
#[test]
fn dialogue_that_never_changes_warns_dw0467() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/greeting" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    let diags = cast::check_cast(&campaign(FULL_ONE, two));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0467")
        .expect("an unchanging tree must raise DW0467");
    assert_eq!(d.severity, delvewright_dsl::Severity::Warning);
    assert!(d.message.contains("npc/keeper"), "{}", d.message);
    // The scout, whose root advances, is not flagged.
    assert!(
        !d.message.contains("npc/scout"),
        "an evolving NPC must not be flagged: {}",
        d.message
    );
}

/// A repeated root spelled as `"unchanged"` is the same staleness, and is caught.
#[test]
fn unchanged_does_not_launder_staleness_past_dw0467() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "unchanged" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    assert!(
        codes(&campaign(FULL_ONE, two)).contains(&"DW0467".to_string()),
        "`unchanged` must not hide an unchanging tree from the staleness lint"
    );
}

/// A bark pool is background business by construction, so it never goes stale.
#[test]
fn a_repeated_bark_pool_is_not_stale() {
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "still watching", "dialogue": "unchanged" }
    }"#;
    assert!(
        !codes(&campaign(FULL_ONE, two)).contains(&"DW0467".to_string()),
        "a carried-forward bark pool must not be flagged as stale"
    );
}
