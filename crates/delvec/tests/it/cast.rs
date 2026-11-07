//! The NPC scene ledger (spec-0020): the four build proofs, the `"unchanged"`
//! sugar, the staleness lint and the pre-0.7 deprecation window.
//!
//! Fixture shape: hello-world's world/classes, plus two NPCs and a two-quest DAG
//! (`quest/one` → `quest/two`) so a ledger has a *previous* node to be consistent
//! with. Every test drives `cast::check_cast` on a parsed campaign, so the proofs
//! are exercised exactly as `delvec validate` runs them.

use crate::common;

use delvec::compiler::cast;
use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign};
use std::collections::BTreeMap;
use std::sync::LazyLock;

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        common::repo_root()
            .join("crates/dsl/fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

/// Two NPCs: the keeper at his stand, a scout at the exit.
static NPCS: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%", "campaign_id": "hello-world", "stage": "npcs",
  "content": { "npcs": [
    { "id": "npc/keeper", "name": "The Keeper", "role": "quest-giver",
      "area": "area/keep", "anchor": "anchor/keeper-stand", "base_entity": "minecraft:villager",
      "persona": { "archetype": "stoic gatekeeper", "speech_style": "Terse.", "motivation": "Guard the gate." } },
    { "id": "npc/scout", "name": "The Scout", "role": "flavor",
      "area": "area/keep", "anchor": "anchor/exit", "base_entity": "minecraft:villager",
      "persona": { "archetype": "restless scout", "speech_style": "Clipped.", "motivation": "Get out." } }
  ] }
}"#,
    )
});

static QUEST_PLAN: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%", "campaign_id": "hello-world", "stage": "quest-plan",
  "content": { "quests": [
    { "id": "quest/one", "goal": "Speak with the Keeper.", "area": "area/keep",
      "npcs": ["npc/keeper"], "depends_on": [], "mandatory": true, "act": 1 },
    { "id": "quest/two", "goal": "Leave the keep.", "area": "area/keep",
      "npcs": ["npc/scout"], "depends_on": ["quest/one"], "mandatory": true, "act": 1 }
  ], "finale": "quest/two" }
}"#,
    )
});

static DIALOGUE: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%", "campaign_id": "hello-world", "stage": "dialogue",
  "content": { "dialogues": [
    { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
      { "id": "dlg/greeting", "text": "Halt.", "options": [
        { "label": "Open the door.", "effects": [{ "type": "complete-objective", "objective": "obj/talk" }] } ] },
      { "id": "dlg/after", "text": "You still here?", "options": [] } ] },
    { "npc": "npc/scout", "root": "dlg/scout-root", "nodes": [
      { "id": "dlg/scout-root", "text": "Quiet, now.", "options": [] },
      { "id": "dlg/scout-later", "text": "We are clear.", "options": [] } ] }
  ] }
}"#,
    )
});

/// A two-quest stage-5 document whose `cast` blocks are supplied per test.
fn quests(cast_one: &str, cast_two: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}", "campaign_id": "hello-world", "stage": "quests",
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
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
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

// --- DW0858: an objective that cannot be completed is refused --------------
//
// `quest/one` carries `obj/talk` on `npc/keeper`, and `quest/two` only opens
// once `quest/one` completes — so the only scenes live while `obj/talk` is
// pending are `quest/one`'s own.

/// Every one of these fixtures keeps `DW0123` green: the completing option
/// really is in the keeper's tree, reachable from the stage-6 `root`. That is
/// the point — the coverage check measures the tree, and only the ledger check
/// measures what right-click opens during the beat.
fn dsl_codes(cast_one: &str, cast_two: &str) -> Vec<String> {
    delvewright_dsl::check_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests(cast_one, cast_two),
        dialogue: DIALOGUE.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .iter()
    .map(|d| d.code.clone())
    .collect()
}

/// The defect: the quest that asks the player to talk declares that very NPC
/// silent, so right-click consumes the interaction and opens nothing.
#[test]
fn dw0858_a_silent_scene_on_the_asking_quest_is_refused() {
    let one = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "none" },
      "npc/scout":  { "at": "anchor/exit", "doing": "watching the road", "dialogue": { "barks": ["Nothing yet."] } }
    }"#;
    let d = cast::check_cast(&campaign(one, FULL_TWO))
        .into_iter()
        .find(|d| d.code == "DW0858")
        .expect("a talk-to whose scene opens nothing must be refused");
    assert!(d.message.contains("obj/talk"), "{}", d.message);
    assert!(d.message.contains("npc/keeper"), "{}", d.message);

    // Only this repair can catch it: every neighbouring check is green.
    let dsl = dsl_codes(one, FULL_TWO);
    assert!(
        !dsl.contains(&"DW0123".to_string()) && !dsl.contains(&"DW0120".to_string()),
        "the tree still covers obj/talk from its root, so DW0120/DW0123 must stay green: {dsl:?}"
    );
    assert!(
        !codes(&campaign(one, FULL_TWO)).contains(&"DW0467".to_string()),
        "the keeper's scene changes between quests, so the staleness lint says nothing"
    );
}

/// A bark pool is a scene too, and it advances nothing — the same refusal.
#[test]
fn dw0858_a_bark_pool_cannot_answer_an_objective() {
    let one = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": { "barks": ["Halt.", "Still no."] } },
      "npc/scout":  { "at": "anchor/exit", "doing": "watching the road", "dialogue": { "barks": ["Nothing yet."] } }
    }"#;
    let d = cast::check_cast(&campaign(one, FULL_TWO))
        .into_iter()
        .find(|d| d.code == "DW0858")
        .expect("a bark pool cannot complete a talk-to");
    assert!(d.message.contains("bark pool"), "{}", d.message);
}

/// A real scene root that simply does not reach the completing option is the
/// same defect wearing a root's clothes — the message names the root it read.
#[test]
fn dw0858_a_scene_root_that_never_reaches_the_option_is_refused() {
    let one = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "watching the road", "dialogue": { "barks": ["Nothing yet."] } }
    }"#;
    let d = cast::check_cast(&campaign(one, FULL_TWO))
        .into_iter()
        .find(|d| d.code == "DW0858")
        .expect("a scene root with no completing option must be refused");
    assert!(d.message.contains("dlg/after"), "{}", d.message);
}

/// The ordinary silent scene stays legal: an NPC with nothing to say during a
/// quest whose beats do not go through them is common and correct. Here the
/// scout is silent in BOTH quests and no objective asks for them.
#[test]
fn dw0858_does_not_fire_on_an_ordinary_silent_scene() {
    let one = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" },
      "npc/scout":  { "at": "anchor/exit", "doing": "asleep at his post", "dialogue": "none" }
    }"#;
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "still asleep", "dialogue": "none" }
    }"#;
    let diags = cast::check_cast(&campaign(one, two));
    assert!(
        !diags.iter().any(|d| d.code == "DW0858"),
        "a silent scene on an npc no objective names must stay legal: {diags:#?}"
    );
    // And the clean ledger the rest of this file uses raises nothing either.
    assert!(
        !codes(&campaign(FULL_ONE, FULL_TWO)).contains(&"DW0858".to_string()),
        "the complete consistent ledger must stay clean"
    );
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
    assert_ne!(dialogue, DIALOGUE.as_str(), "the branch patch must apply");
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests(cast_one, cast_two),
        dialogue,
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
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
        gated.scene,
        Some(2),
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
        vec![("quest/one", Some(1)), ("quest/two", None)],
        "`unchanged` claims no scene of its own"
    );
    let both: std::collections::BTreeSet<String> =
        ["quest/one", "quest/two"].map(String::from).into();
    assert_eq!(
        cast::eval_ladder(scout, &both, &Default::default(), &Default::default()),
        1,
        "with both quests begun, the scene carried forward from quest/one governs"
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

// ---------------------------------------------------------------------------
// DW0846: a clause no runtime state can select (the ladder solver's refusal)
// ---------------------------------------------------------------------------

/// A gated branch listed BEFORE an ungated fallback of the same quest is dead:
/// the fallback always passes and, being later, always overrides. This is the
/// per-branch ordering rule ("fallback first, branches after") hardened from a
/// doc line into a refusal.
#[test]
fn a_branch_listed_before_its_ungated_fallback_is_dead() {
    let two = r#"{
      "npc/keeper": [
        { "at": "anchor/keeper-stand", "doing": "standing the watch with you",
          "dialogue": "dlg/after", "requires_flags": ["flag/wait"] },
        { "at": "anchor/keeper-stand", "doing": "watching you go",
          "dialogue": { "barks": ["Go on, then."] } }
      ],
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    let diags = cast::check_cast(&campaign(FULL_ONE, two));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0846")
        .expect("the shadowed branch must be refused");
    assert!(
        d.message.contains("placement 0") && d.message.contains("fallback FIRST"),
        "the message names the dead placement and the reorder prescription: {}",
        d.message
    );
}

/// The same entry with the fallback FIRST is the documented shape and is clean:
/// the branch is selectable (its flag on), and the fallback is selectable (the
/// branch's flag off).
#[test]
fn fallback_first_then_branches_is_alive() {
    let two = r#"{
      "npc/keeper": [
        { "at": "anchor/keeper-stand", "doing": "watching you go",
          "dialogue": { "barks": ["Go on, then."] } },
        { "at": "anchor/keeper-stand", "doing": "standing the watch with you",
          "dialogue": "dlg/after", "requires_flags": ["flag/wait"] }
      ],
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    assert!(
        !codes(&campaign(FULL_ONE, two)).contains(&"DW0846".to_string()),
        "fallback-first is the documented per-branch shape and must be clean"
    );
}

/// The state axis: a clause whose numeric gate is ENTAILED by a later sibling's
/// (`at-least 500` before `at-least 400`) is dead — every value that selects it
/// also selects the later clause, which overrides. A flag pin cannot even see
/// this case; the solver's `DatumSet` arithmetic is what catches it.
#[test]
fn a_clause_entailed_by_a_later_siblings_state_gate_is_dead() {
    let two = r#"{
      "npc/keeper": [
        { "at": "anchor/keeper-stand", "doing": "counting a high toll",
          "dialogue": "dlg/after",
          "requires_state": [ { "state": "state/patience", "op": "at-least", "value": 500 } ] },
        { "at": "anchor/keeper-stand", "doing": "counting a low toll",
          "dialogue": { "barks": ["Nearly there."] },
          "requires_state": [ { "state": "state/patience", "op": "at-least", "value": 400 } ] }
      ],
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    assert!(
        codes(&campaign(FULL_ONE, two)).contains(&"DW0846".to_string()),
        "an entailed state gate must be refused as dead"
    );
}

/// Disjoint state windows are both selectable and clean — the check is
/// reachability of each clause, never a stylistic limit on how many there are.
#[test]
fn disjoint_state_windows_are_alive() {
    let two = r#"{
      "npc/keeper": [
        { "at": "anchor/keeper-stand", "doing": "counting a low toll",
          "dialogue": { "barks": ["Nearly there."] },
          "requires_state": [ { "state": "state/patience", "op": "at-most", "value": 399 } ] },
        { "at": "anchor/keeper-stand", "doing": "counting a high toll",
          "dialogue": "dlg/after",
          "requires_state": [ { "state": "state/patience", "op": "at-least", "value": 400 } ] }
      ],
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    assert!(
        !codes(&campaign(FULL_ONE, two)).contains(&"DW0846".to_string()),
        "disjoint windows are each selectable and must be clean"
    );
}

/// A clause whose OWN gate is self-contradictory is `DW0847`'s finding at the
/// gate site, not a second report here — one defect, one code.
#[test]
fn a_self_contradictory_clause_is_not_double_reported_as_dead() {
    let two = r#"{
      "npc/keeper": [
        { "at": "anchor/keeper-stand", "doing": "watching you go",
          "dialogue": { "barks": ["Go on, then."] } },
        { "at": "anchor/keeper-stand", "doing": "counting an impossible toll",
          "dialogue": "dlg/after",
          "requires_state": [
            { "state": "state/patience", "op": "at-least", "value": 5 },
            { "state": "state/patience", "op": "at-most", "value": 3 }
          ] }
      ],
      "npc/scout":  { "at": "anchor/exit", "doing": "waving you through", "dialogue": "dlg/scout-later" }
    }"#;
    assert!(
        !codes(&campaign(FULL_ONE, two)).contains(&"DW0846".to_string()),
        "a contradictory gate is DW0847's finding at its own site"
    );
}

/// The solver itself, at the emitter's contract: every live clause yields a
/// drive that its own model scores as selecting that clause — the guarantee the
/// generated `cast_ladder_<npc>` assert rests on.
#[test]
fn every_live_clause_has_a_drive_its_model_confirms() {
    let c = campaign(FULL_ONE, FULL_TWO);
    let casts = cast::npc_casts(&c);
    let mut proved = 0usize;
    for cast_ in casts.values() {
        for n in 0..cast_.by_quest.len() {
            let drive = cast::distinguishing_drive(cast_, n).expect("live ledger");
            assert_eq!(
                Some(cast::eval_ladder(
                    cast_,
                    &drive.begun,
                    &drive.flags,
                    &drive.datums
                )),
                cast_.by_quest[n].scene,
            );
            proved += 1;
        }
    }
    assert_eq!(proved, 4, "two NPCs x two quests of clauses were solved");
}

// --- quests open together: whose clause governs ----------------------------
//
// `quest/one` → { `quest/two`, `quest/three` }, both opened by `quest/one`
// completing, so they begin in the same tick; `quest/two` ranks first.
// `quest/two` asks the player to talk to the scout. An optional `quest/four`
// opens once its parent completes: `quest/three` (a later opening) or
// `quest/one` (a third sibling, ranked last).

/// The sibling fixture's quest plan: the two siblings, plus `quest/four` under
/// `four`'s parent when given.
fn sibling_plan(four: Option<&str>) -> String {
    let tail = four
        .map(|parent| {
            format!(
                r#",
    {{ "id": "quest/four", "goal": "Report back.", "area": "area/keep",
      "npcs": ["npc/scout"], "depends_on": ["{parent}"], "mandatory": false, "act": 1 }}"#
            )
        })
        .unwrap_or_default();
    common::at_dsl_version(&format!(
        r#"{{
  "dsl_version": "%dsl_version%", "campaign_id": "hello-world", "stage": "quest-plan",
  "content": {{ "quests": [
    {{ "id": "quest/one", "goal": "Speak with the Keeper.", "area": "area/keep",
      "npcs": ["npc/keeper"], "depends_on": [], "mandatory": true, "act": 1 }},
    {{ "id": "quest/two", "goal": "Ask the scout.", "area": "area/keep",
      "npcs": ["npc/scout"], "depends_on": ["quest/one"], "mandatory": true, "act": 1 }},
    {{ "id": "quest/three", "goal": "Leave the keep.", "area": "area/keep",
      "npcs": ["npc/keeper"], "depends_on": ["quest/one"], "mandatory": true, "act": 1 }}{tail}
  ], "finale": "quest/three" }}
}}"#
    ))
}

/// Three (or four) quests; the scout's tree gains `dlg/scout-ask`, the one node
/// whose option completes `obj/ask`. `four` is `quest/four`'s parent and cast.
fn siblings(two: &str, three: &str, four: Option<(&str, &str)>) -> Campaign {
    let four_q = four
        .map(|(parent, c)| {
            format!(
                r#",
    {{ "id": "quest/four", "trigger": {{ "type": "quest-complete", "quest": "{parent}" }},
       "objectives": [ {{ "type": "reach-anchor", "id": "obj/report", "anchor": "anchor/exit", "radius": 2 }} ],
       "on_complete": [], "cast": {c} }}"#
            )
        })
        .unwrap_or_default();
    let quests = format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}", "campaign_id": "hello-world", "stage": "quests",
  "content": {{ "quests": [
    {{ "id": "quest/one", "trigger": {{ "type": "campaign-start" }},
       "objectives": [ {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }} ],
       "on_complete": [], "cast": {FULL_ONE} }},
    {{ "id": "quest/two", "trigger": {{ "type": "quest-complete", "quest": "quest/one" }},
       "objectives": [ {{ "type": "talk-to", "id": "obj/ask", "npc": "npc/scout" }} ],
       "on_complete": [], "cast": {two} }},
    {{ "id": "quest/three", "trigger": {{ "type": "quest-complete", "quest": "quest/one" }},
       "objectives": [ {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2 }} ],
       "on_complete": [ {{ "type": "campaign-complete" }} ], "cast": {three} }}{four_q}
  ] }}
}}"#
    );
    let dialogue = DIALOGUE.replace(
        r#"{ "id": "dlg/scout-later", "text": "We are clear.", "options": [] }"#,
        r#"{ "id": "dlg/scout-later", "text": "We are clear.", "options": [] },
      { "id": "dlg/scout-ask", "text": "Ask, then.", "options": [
        { "label": "Which way is clear?", "effects": [{ "type": "complete-objective", "objective": "obj/ask" }] } ] }"#,
    );
    assert_ne!(dialogue, DIALOGUE.as_str(), "the dialogue patch must apply");
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: sibling_plan(four.map(|(parent, _)| parent)),
        quests,
        dialogue,
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("sibling fixture must parse")
}

/// `quest/two` gives the scout the root that completes `obj/ask`.
const TWO_ASKS: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
  "npc/scout":  { "at": "anchor/exit", "doing": "waiting to be asked", "dialogue": "dlg/scout-ask" }
}"#;

/// A sibling ranked after `quest/two` that gives the scout a bark pool.
const THREE_BARKS: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "unchanged" },
  "npc/scout":  { "at": "anchor/exit", "doing": "busy at the gate", "dialogue": { "barks": ["Busy."] } }
}"#;

/// The same sibling, with no claim on the scout.
const THREE_UNCHANGED: &str = r#"{
  "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "unchanged" },
  "npc/scout":  { "at": "anchor/exit", "doing": "waiting to be asked", "dialogue": "unchanged" }
}"#;

/// Every quest of the sibling fixture begun, no flag set.
fn begun(ids: &[&str]) -> std::collections::BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

/// The shape that stranded a playtest: two quests open in one tick, and the one
/// ranked later declares a bark pool for the NPC the other one needs. At run
/// time the later clause governs for as long as both are open, so the
/// objective's own root is never what right-click opens. DW0858 judges the
/// clause that wins, not every clause of every open quest.
#[test]
fn dw0858_a_sibling_ranked_later_that_overrides_the_npc_is_refused() {
    let c = siblings(TWO_ASKS, THREE_BARKS, None);
    let scout = &cast::npc_casts(&c)["npc/scout"];
    let all = begun(&["quest/one", "quest/two", "quest/three"]);
    let bark = scout
        .scenes
        .iter()
        .find(|s| matches!(s.action, cast::SceneAction::Barks(_)) && s.declared_by == "quest/three")
        .expect("quest/three's pool is a scene")
        .index;
    assert_eq!(
        cast::eval_ladder(scout, &all, &Default::default(), &Default::default()),
        bark,
        "the model must agree the later sibling's bark pool is what right-click opens"
    );
    let d = cast::check_cast(&c)
        .into_iter()
        .find(|d| d.code == "DW0858")
        .expect("an objective whose NPC a same-tick sibling overrides must be refused");
    assert!(d.message.contains("obj/ask"), "{}", d.message);
    assert!(d.message.contains("`quest/three`"), "{}", d.message);
    assert!(d.message.contains("bark pool"), "{}", d.message);
}

/// The repair the refusal names: the sibling declares `"unchanged"`, claims
/// nothing, and `quest/two`'s own root governs.
#[test]
fn dw0858_a_sibling_that_carries_the_npc_forward_passes() {
    let c = siblings(TWO_ASKS, THREE_UNCHANGED, None);
    let diags = cast::check_cast(&c);
    assert!(
        !diags.iter().any(|d| d.code == "DW0858"),
        "an `unchanged` sibling claims nothing: {diags:#?}"
    );
    let scout = &cast::npc_casts(&c)["npc/scout"];
    let ask = scout
        .scenes
        .iter()
        .find(|s| s.action == cast::SceneAction::Root("dlg/scout-ask".into()))
        .unwrap()
        .index;
    assert_eq!(
        cast::eval_ladder(
            scout,
            &begun(&["quest/one", "quest/two", "quest/three"]),
            &Default::default(),
            &Default::default()
        ),
        ask
    );
}

/// An objective answerable only once a later quest opens is a legal design:
/// the scout is busy while `quest/three` runs, and `quest/four` (opened by
/// `quest/three` completing) hands her the root. Some reachable state answers
/// it, so nothing is refused.
#[test]
fn dw0858_an_objective_a_later_opening_answers_passes() {
    let four = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "unchanged" },
      "npc/scout":  { "at": "anchor/exit", "doing": "free at last", "dialogue": "dlg/scout-ask" }
    }"#;
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
      "npc/scout":  { "at": "anchor/exit", "doing": "waiting", "dialogue": "dlg/scout-later" }
    }"#;
    let diags = cast::check_cast(&siblings(two, THREE_BARKS, Some(("quest/three", four))));
    assert!(
        !diags.iter().any(|d| d.code == "DW0858"),
        "quest/four's opening answers obj/ask: {diags:#?}"
    );
    // Without quest/four nothing ever answers it.
    assert!(
        codes(&siblings(two, THREE_BARKS, None)).contains(&"DW0858".to_string()),
        "with no later opening the objective is unanswerable"
    );
}

/// `"unchanged"` is resolved at run time, never to the clause ranked before it:
/// inside a per-branch list it does not copy its own quest's other placement,
/// and after a gated clause it does not carry that clause's scene ungated.
#[test]
fn unchanged_puts_back_what_governed_before_its_quest() {
    let all = begun(&["quest/one", "quest/two", "quest/three"]);
    let warned: BTreeMap<String, i32> = [("flag/warned".to_string(), 1)].into();
    let scene_of = |cast_: &cast::NpcCast, root: &str| {
        cast_
            .scenes
            .iter()
            .find(|s| s.action == cast::SceneAction::Root(root.into()))
            .unwrap()
            .index
    };

    // The list shape: barks unless warned, and no claim once warned.
    let three = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "unchanged" },
      "npc/scout": [
        { "at": "anchor/exit", "doing": "busy at the gate", "dialogue": { "barks": ["Busy."] },
          "forbids_flags": ["flag/warned"] },
        { "at": "anchor/exit", "doing": "waiting to be asked", "dialogue": "unchanged",
          "requires_flags": ["flag/warned"] }
      ]
    }"#;
    let c = siblings(TWO_ASKS, three, None);
    let scout = &cast::npc_casts(&c)["npc/scout"];
    assert_eq!(
        cast::eval_ladder(scout, &all, &warned, &Default::default()),
        scene_of(scout, "dlg/scout-ask"),
        "warned: quest/three claims nothing, so quest/two's root governs"
    );

    // The gated shape: quest/two splits on the flag, quest/three carries it.
    let two = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "dlg/after" },
      "npc/scout": [
        { "at": "anchor/exit", "doing": "waiting", "dialogue": "dlg/scout-later" },
        { "at": "anchor/exit", "doing": "waiting to be asked", "dialogue": "dlg/scout-ask",
          "requires_flags": ["flag/warned"] }
      ]
    }"#;
    let c = siblings(two, THREE_UNCHANGED, None);
    let scout = &cast::npc_casts(&c)["npc/scout"];
    assert_eq!(
        cast::eval_ladder(scout, &all, &Default::default(), &Default::default()),
        scene_of(scout, "dlg/scout-later"),
        "not warned: the carried scene is quest/two's fallback, not its gated branch"
    );
    assert_eq!(
        cast::eval_ladder(scout, &all, &warned, &Default::default()),
        scene_of(scout, "dlg/scout-ask"),
    );
}

/// The shape that stranded the playtest, exactly: `quest/three` gives the scout
/// a bark pool only once `quest/two` is done (`flag/asked`), and a third sibling
/// opened in the same tick, ranked last, carries her forward. `"unchanged"` once
/// resolved to the clause ranked before it and was emitted ungated, so the third
/// sibling set the bark pool for everyone the moment the three quests opened.
/// It claims nothing now: `quest/two`'s root governs until the flag is set, and
/// DW0858, reading the clause that wins, has nothing to refuse.
#[test]
fn a_last_ranked_sibling_that_carries_the_npc_does_not_override_it() {
    let three = r#"{
      "npc/keeper": { "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "unchanged" },
      "npc/scout":  { "at": "anchor/exit", "doing": "busy at the gate once asked", "dialogue": { "barks": ["Busy."] },
                      "requires_flags": ["flag/asked"] }
    }"#;
    let c = siblings(TWO_ASKS, three, Some(("quest/one", THREE_UNCHANGED)));
    let scout = &cast::npc_casts(&c)["npc/scout"];
    let all = begun(&["quest/one", "quest/two", "quest/three", "quest/four"]);
    let ask = scout
        .scenes
        .iter()
        .find(|s| s.action == cast::SceneAction::Root("dlg/scout-ask".into()))
        .unwrap()
        .index;
    assert_eq!(
        cast::eval_ladder(scout, &all, &Default::default(), &Default::default()),
        ask,
        "before `flag/asked`, quest/two's own root must govern"
    );
    let asked: BTreeMap<String, i32> = [("flag/asked".to_string(), 1)].into();
    assert_ne!(
        cast::eval_ladder(scout, &all, &asked, &Default::default()),
        ask,
        "once asked, quest/three's gated pool takes over, through quest/four's carry"
    );
    let diags = cast::check_cast(&c);
    assert!(
        !diags.iter().any(|d| d.code == "DW0858"),
        "the governing scene answers obj/ask: {diags:#?}"
    );
}
