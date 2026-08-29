//! `DW0150`'s two readings: a stage-4 plan whose stage 5 is **not written yet**,
//! and a stage-4 plan whose stage 5 is written and does not carry this id.
//!
//! # What these tests are written against
//!
//! The authoring page's story-document step tells an author to loop
//! `delvec validate` "fixing by diagnostic code", and adds that three failed
//! repairs on the same code means stop and look at the design. Once
//! `quest-plan.json` is on disk that loop cannot terminate: every planned quest
//! is unexpanded, and the two remedies the per-quest message names are the step
//! after next, or deleting the document just written.
//!
//! So the discriminator these tests defend is **whether stage 5 declares any
//! quests at all**. Both readings still refuse, at the same code and the same
//! exit — a plan with no expansion emits nothing and cannot build — and every
//! assertion here is about what the refusal SAYS. Nothing in this file asserts
//! that a campaign is accepted; if one ever were, the gate would have been
//! weakened and these tests would not notice, which is why
//! [`both_readings_refuse`] states the refusal itself.
//!
//! The claim the grouped message makes about there being no cheaper way out —
//! that the schema-minimal stage-5 quest is refused again by `DW0481` and
//! `DW0460` — is a claim about the compiler tier and is established there:
//! `crates/compiler/tests/plan_awaiting_expansion.rs`.

mod common;

use delvewright_dsl::{Diagnostic, RawCampaign, check_campaign};
use serde_json::{Value, json};

/// A second planned quest, unexpanded in every fixture below.
fn second_planned_quest() -> Value {
    json!({
        "act": 1,
        "area": "area/keep",
        "depends_on": [],
        "goal": "A prerequisite nobody has written yet.",
        "id": "quest/side-trip",
        "mandatory": true,
        "npcs": []
    })
}

/// hello-world's plan with `quest/side-trip` appended, so exactly one of the two
/// planned quests is expanded in the untouched stage 5.
fn two_quest_plan() -> String {
    common::patch_doc(&common::read_valid("quest-plan.json"), |v| {
        v["content"]["quests"]
            .as_array_mut()
            .expect("planned quests")
            .push(second_planned_quest());
    })
}

/// The stage-5 document as `DW0874`'s stubbing recipe would have it: the
/// envelope, and a `content` carrying only the field the schema requires.
fn empty_stage_five() -> String {
    common::patch_doc(&common::read_valid("quests.json"), |v| {
        v["content"] = json!({ "quests": [] });
    })
}

fn diagnostics(quest_plan: String, quests: String) -> Vec<Diagnostic> {
    check_campaign(&RawCampaign {
        quest_plan,
        quests,
        ..common::valid_raw()
    })
}

fn dw0150(quest_plan: String, quests: String) -> Vec<Diagnostic> {
    diagnostics(quest_plan, quests)
        .into_iter()
        .filter(|d| d.code == "DW0150")
        .collect()
}

// ---------------------------------------------------------------------------
// Reading one: stage 5 is not written yet
// ---------------------------------------------------------------------------

/// A plan with no expansion at all is **one** diagnostic about one state, on the
/// plan's own `quests` array rather than on an entry of it, naming every planned
/// quest.
///
/// The count is the point: per-quest, this is N copies of a sentence about a
/// state, and N is the campaign's quest count rather than anything the author
/// can act on.
#[test]
fn an_unwritten_stage_five_is_one_diagnostic_naming_every_planned_quest() {
    let got = dw0150(two_quest_plan(), empty_stage_five());
    assert_eq!(got.len(), 1, "expected one grouped DW0150, got: {got:#?}");
    let d = &got[0];
    assert_eq!(d.path, "/content/quests", "{d:#?}");
    assert_eq!(d.stage, "quest-plan", "{d:#?}");
    for id in ["quest/open-the-door", "quest/side-trip"] {
        assert!(
            d.message.contains(id),
            "the grouped message must name every planned quest, and it omitted `{id}`: {}",
            d.message
        );
    }
}

/// It says the state is an authoring state, says why the refusal stands anyway,
/// and names the two codes that refuse the stub somebody would otherwise reach
/// for.
///
/// Each of these is load-bearing on its own. Without the first the author reads
/// a fault they did not commit; without the second the message reads as an
/// apology for a check that should have been a warning; without the third they
/// spend the next hour writing empty expansions and end with more errors than
/// they started with.
#[test]
fn the_unwritten_stage_five_message_names_the_state_and_the_absent_route() {
    let got = dw0150(two_quest_plan(), empty_stage_five());
    let m = &got[0].message;
    assert!(m.contains("authoring state, not a fault"), "{m}");
    assert!(m.contains("no campaign in this state can build"), "{m}");
    assert!(m.contains("DW0481"), "{m}");
    assert!(m.contains("DW0460"), "{m}");
    assert!(m.contains("writing stage 5"), "{m}");
}

// ---------------------------------------------------------------------------
// Reading two: stage 5 is written and this id is not in it
// ---------------------------------------------------------------------------

/// With stage 5 written, the per-quest refusal and its two ordinary remedies are
/// exactly right, and it points at the plan entry that is wrong.
#[test]
fn a_written_stage_five_missing_one_id_is_still_reported_per_quest() {
    let got = dw0150(two_quest_plan(), common::read_valid("quests.json"));
    assert_eq!(got.len(), 1, "{got:#?}");
    let d = &got[0];
    assert_eq!(d.path, "/content/quests/1", "{d:#?}");
    assert!(d.message.contains("quest/side-trip"), "{}", d.message);
    assert!(
        d.message.contains("drop it from the stage-4 plan"),
        "{}",
        d.message
    );
}

/// And it says how many quests stage 5 DOES declare, which is the fact that
/// separates "you mistyped an id" from "you have not written this document".
#[test]
fn a_written_stage_five_says_it_is_a_mismatch_rather_than_an_unwritten_stage() {
    let got = dw0150(two_quest_plan(), common::read_valid("quests.json"));
    let m = &got[0].message;
    assert!(m.contains("stage 5 declares 1 quest(s)"), "{m}");
    assert!(m.contains("mismatch rather than an unwritten stage"), "{m}");
}

// ---------------------------------------------------------------------------
// The discriminator, and the refusal itself
// ---------------------------------------------------------------------------

/// The two readings are not two spellings of one message: each says what the
/// other must not.
///
/// This is the half that would go red if the discriminator were removed and one
/// message were used for both — which is the shape a later edit is most likely
/// to reach for.
#[test]
fn the_two_readings_do_not_share_a_message() {
    let unwritten = dw0150(two_quest_plan(), empty_stage_five())[0]
        .message
        .clone();
    let mismatch = dw0150(two_quest_plan(), common::read_valid("quests.json"))[0]
        .message
        .clone();
    assert!(
        !unwritten.contains("mismatch rather than an unwritten stage"),
        "{unwritten}"
    );
    assert!(
        !mismatch.contains("authoring state, not a fault"),
        "{mismatch}"
    );
}

/// **Both readings refuse.** The campaign is rejected in each case, and a
/// campaign with every planned quest expanded raises no `DW0150` at all — so
/// this file cannot go green by the check having stopped binding.
#[test]
fn both_readings_refuse() {
    assert_eq!(dw0150(two_quest_plan(), empty_stage_five()).len(), 1);
    assert_eq!(
        dw0150(two_quest_plan(), common::read_valid("quests.json")).len(),
        1
    );
    assert!(
        dw0150(
            common::read_valid("quest-plan.json"),
            common::read_valid("quests.json"),
        )
        .is_empty(),
        "the untouched valid campaign must raise no DW0150"
    );
}
