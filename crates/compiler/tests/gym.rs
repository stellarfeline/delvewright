//! The metrics gym (spec-0049 §2.3, acceptance 9): a site-plan campaign
//! generated FROM the table, and the coverage line that says what it missed.
//!
//! # What is worth asserting here, and what is not
//!
//! Asserting the gym's bay count or its region extent would be asserting the
//! table's current numbers a second time, in a second place — the drift a single
//! authority exists to prevent. So these tests assert **properties that must hold
//! whatever the table says**: that the gym reads the table rather than deciding
//! for itself, that its coverage claim is measured rather than declared, that the
//! documents it writes are the ones a campaign directory needs, and that it is a
//! function of the table alone.
//!
//! The one thing they cannot prove is that the generated campaign compiles —
//! that needs a build, which is `delvec build <dir>` and is measured in the
//! round's own record rather than asserted in a unit test: the top rung of the
//! ladder is a 128×128 place and the derivation over it takes minutes, which is
//! itself a finding and not something to hide inside a test's runtime.

use std::collections::BTreeSet;

use delvewright_compiler::gym;
use delvewright_dsl::metrics::Metrics;

/// `DW0840` fires, names its denominator, and names exactly the entries the
/// generation did not read.
#[test]
fn dw0840_names_every_building_metric_the_gym_is_built_from_nothing_of() {
    let table = Metrics::table();
    let g = gym::generate(&table, "metrics-gym");

    let d = g
        .unwalked(&table)
        .expect("this table defines entries no bay is built from");
    assert_eq!(d.code.to_string(), "DW0840");

    let unread: BTreeSet<&str> = table
        .building
        .keys()
        .copied()
        .filter(|k| !g.read.contains(k))
        .collect();
    assert!(
        !unread.is_empty(),
        "the diagnostic fired, so something is unread"
    );
    for key in &unread {
        assert!(
            d.message.contains(key),
            "`{key}` is unread and the line does not name it: {}",
            d.message
        );
    }
    // The denominator, stated: a coverage count without one is about a smaller
    // world than the tool claims to cover.
    assert!(
        d.message.contains(&table.building.len().to_string()),
        "the line does not state how many entries the table defines: {}",
        d.message
    );
}

/// The coverage numerator is the READ ledger, not a list beside the generator.
///
/// This is the property that makes `DW0840` falsifiable rather than decorative:
/// every entry it reports as instantiated was actually consumed to decide
/// something. The failure it guards is the one the round hit — the generator
/// deciding a host pair from a hard-coded ratio while the table published the
/// pitches, which made `pitch.ramp` and `pitch.stair` honestly come back unread.
#[test]
fn every_entry_the_gym_claims_was_read_through_the_tables_own_accessor() {
    let table = Metrics::table();
    let g = gym::generate(&table, "metrics-gym");
    for key in &g.read {
        assert!(
            table.building.contains_key(key),
            "`{key}` is in the read ledger and not in the table"
        );
    }
    assert_eq!(g.entries, table.building.len());
    assert!(
        g.read.len() < g.entries,
        "if the gym ever reaches every entry, delete the `DW0840` expectation in \
         the test above rather than loosening this one — that is the end state"
    );
}

/// The gym is a function of the table alone: same table, same bytes.
#[test]
fn the_gym_is_deterministic() {
    let table = Metrics::table();
    let a = gym::generate(&table, "metrics-gym");
    let b = gym::generate(&table, "metrics-gym");
    assert_eq!(a.documents, b.documents);
}

/// Every document a campaign directory needs, and each in canonical form.
#[test]
fn the_gym_writes_a_whole_campaign_in_canonical_form() {
    let table = Metrics::table();
    let g = gym::generate(&table, "metrics-gym");
    for name in [
        "world.json",
        "npcs.json",
        "classes.json",
        "quest-plan.json",
        "quests.json",
        "dialogue.json",
        "geometry-brief.json",
        "layout-graph.json",
        "site-plan.json",
    ] {
        let text = g
            .documents
            .get(name)
            .unwrap_or_else(|| panic!("the gym writes no `{name}`"));
        // `delvec fmt --check` is a required gate over the whole tree, and a
        // generator that emits documents it would refuse hands the creator a
        // red the moment they commit what it wrote.
        let canonical = delvewright_dsl::fmt::format_text(text).expect("parses");
        assert_eq!(&canonical, text, "`{name}` is not in canonical form");
    }
}

/// The gym is a SITE-PLAN campaign: it declares no `areas[]` entry, so it cannot
/// be the two-placement-authorities refusal (`DW0839`) by construction.
#[test]
fn the_gym_has_one_placement_authority() {
    let table = Metrics::table();
    let g = gym::generate(&table, "metrics-gym");
    let world: serde_json::Value =
        serde_json::from_str(&g.documents["world.json"]).expect("the world document parses");
    assert_eq!(
        world["content"]["areas"].as_array().map(Vec::len),
        Some(0),
        "the gym places its geometry with a site plan and nothing else"
    );
    assert!(g.documents.contains_key("site-plan.json"));
}
