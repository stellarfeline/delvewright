//! **The claim `DW0150` makes about there being no cheaper way out, measured.**
//!
//! The grouped `DW0150` tells an author, in so many words, not to reach for a
//! stub: *a stage-5 quest carrying only what its schema requires — a trigger
//! and two empty arrays — is refused again by `DW0481` … and by `DW0460` once
//! for every NPC live in it, so writing empty expansions raises the error count
//! instead of lowering it.*
//!
//! That is a measurement, and a diagnostic asserting a measurement owes one.
//! Without this file the sentence is a claim about the compiler written into
//! the compiler, with nothing anywhere able to disagree with it — and it is
//! exactly the kind of sentence that stays in the message long after the two
//! codes it names have moved.
//!
//! `DW0460` and `DW0481` live in this crate (`compiler::cast`,
//! `compiler::branch`) rather than in the DSL, which is why the claim is
//! established here and the wording is asserted in
//! `crates/dsl/tests/dw0150_plan_awaiting_expansion.rs`.

mod common;

use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world's world and cast, with `quest/side-trip` added to the plan so
/// there are two planned quests and the count below is not one.
const QUEST_PLAN: &str = r#"{
  "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "quest-plan",
  "content": { "quests": [
    { "id": "quest/open-the-door", "goal": "Get the Keeper to open the door.", "area": "area/keep",
      "npcs": ["npc/keeper"], "depends_on": [], "mandatory": true, "act": 1 },
    { "id": "quest/side-trip", "goal": "A prerequisite nobody has written yet.", "area": "area/keep",
      "npcs": [], "depends_on": [], "mandatory": true, "act": 1 }
  ], "finale": "quest/open-the-door" }
}"#;

/// Stage 5 exactly as `DW0874`'s stubbing recipe would have it.
const EMPTY_STAGE_FIVE: &str = r#"{
  "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "quests",
  "content": { "quests": [] }
}"#;

/// Stage 5 as an author would write it if a diagnostic told them to stub their
/// way out: one quest per planned quest, carrying **only** what the schema
/// requires — `id`, `trigger`, `objectives`, `on_complete`.
const MINIMAL_STAGE_FIVE: &str = r#"{
  "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "quests",
  "content": { "quests": [
    { "id": "quest/open-the-door", "trigger": { "type": "campaign-start" },
      "objectives": [], "on_complete": [] },
    { "id": "quest/side-trip", "trigger": { "type": "campaign-start" },
      "objectives": [], "on_complete": [] }
  ] }
}"#;

fn campaign(quests: &str) -> Campaign {
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("fixture must parse")
}

/// Every code the two tiers `delvec validate` runs would report, for the pieces
/// of it that decide this measurement: the DSL's own rules, the NPC scene
/// ledger (`DW0460`) and the story-node check (`DW0481`).
fn codes(quests: &str) -> Vec<String> {
    let c = campaign(quests);
    delvewright_dsl::validate_campaign(&c)
        .into_iter()
        .chain(delvewright_compiler::cast::check_cast(&c))
        .chain(delvewright_compiler::branch::check_branches(&c))
        .map(|d| d.code)
        .collect()
}

fn count(codes: &[String], want: &str) -> usize {
    codes.iter().filter(|c| c.as_str() == want).count()
}

/// **The measurement.** A plan awaiting its expansion carries ONE `DW0150`.
/// Stubbing the expansions clears it and raises more errors than it cleared —
/// one `DW0481` per quest, and one `DW0460` per NPC live in each of them.
///
/// So a diagnostic that offered "stub it" here would be sending an author to a
/// state strictly worse than the one they are in, which is why the grouped
/// message says the opposite in as many words.
#[test]
fn stubbing_the_expansions_raises_the_error_count() {
    let awaiting = codes(EMPTY_STAGE_FIVE);
    assert_eq!(
        count(&awaiting, "DW0150"),
        1,
        "a plan awaiting expansion is one grouped DW0150: {awaiting:?}"
    );

    let stubbed = codes(MINIMAL_STAGE_FIVE);
    assert_eq!(
        count(&stubbed, "DW0150"),
        0,
        "the stub clears DW0150, which is the whole temptation: {stubbed:?}"
    );
    assert_eq!(
        count(&stubbed, "DW0481"),
        2,
        "one per quest that never said what it does to the story: {stubbed:?}"
    );
    assert!(
        count(&stubbed, "DW0460") >= 1,
        "at least one per NPC live in a quest whose cast does not account for it: {stubbed:?}"
    );

    let before = count(&awaiting, "DW0150");
    let after = count(&stubbed, "DW0481") + count(&stubbed, "DW0460");
    assert!(
        after > before,
        "the message claims stubbing raises the count; measured {before} -> {after}"
    );
}

/// And the refusal itself is unchanged in both directions: neither state
/// builds, so nothing above is green because a check stopped binding.
#[test]
fn neither_state_is_accepted() {
    assert!(!codes(EMPTY_STAGE_FIVE).is_empty());
    assert!(!codes(MINIMAL_STAGE_FIVE).is_empty());
}
