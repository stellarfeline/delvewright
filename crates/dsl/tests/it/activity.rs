//! spec-0131: the activity census is a function of the documents and the
//! metrics table alone, and every act is priced. The pacing projection's
//! reading of it (`DW0822`) is asserted in the engine crate's tests, over a
//! fixture that carries a layout graph.

use crate::common;

use delvewright_dsl::metrics::{Metrics, Reads};
use delvewright_dsl::{Act, ActivityCensus, ActivityRates, parse_campaign};

/// Every act of the vocabulary has a price in the table, under the one name
/// the census reads it by; a new act owes its entry in the change that adds it.
#[test]
fn every_act_is_priced_under_its_own_name() {
    let table = Metrics::table();
    for act in Act::ALL {
        let name = ActivityRates::metric_name(act);
        assert!(
            table
                .resolve(delvewright_dsl::metrics::MetricKind::Pacing, &name)
                .is_ok(),
            "the table prices no `{name}`"
        );
    }
    assert!(ActivityRates::read(&table, &mut Reads::new()).is_some());
}

/// hello-world: one line, a conversation and a walk, priced at the table.
#[test]
fn hello_world_is_one_line_of_a_talk_and_a_walk() {
    let c = parse_campaign(&common::valid_raw()).expect("hello-world parses");
    let table = Metrics::table();
    let mut reads = Reads::new();
    let census = ActivityCensus::of(&c, &table, &mut reads).expect("priced");
    assert_eq!(census.lines.len(), 1);
    assert_eq!(census.objectives, 2);
    assert_eq!(census.acts.get(&Act::Talk), Some(&1));
    assert_eq!(census.acts.get(&Act::Reach), Some(&1));
    let r = &census.rates;
    assert_eq!(
        census.minutes,
        u64::from(r.of(Act::Talk)) + u64::from(r.of(Act::Reach))
    );
    assert!(census.lines[0].talk_or_reach_only());
    // Every price was read, so the provisional notice can name each seed.
    assert_eq!(reads.binding().read, Act::ALL.len());
    assert_eq!(census.lines().len(), 2, "a header and one line");
}

/// Two censuses of one campaign are equal: no clock, no order, no environment.
#[test]
fn the_census_is_deterministic() {
    let c = parse_campaign(&common::valid_raw()).expect("hello-world parses");
    let table = Metrics::table();
    let a = ActivityCensus::of(&c, &table, &mut Reads::new());
    let b = ActivityCensus::of(&c, &table, &mut Reads::new());
    assert_eq!(a, b);
}
