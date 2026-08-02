//! The validation matrix: the valid campaign yields zero diagnostics, and every
//! invalid fixture yields exactly its expected diagnostic code.

mod common;

use std::collections::BTreeSet;

use delvewright_dsl::check_campaign;

#[test]
fn valid_campaign_has_no_diagnostics() {
    let diags = check_campaign(&common::valid_raw());
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the valid campaign, got: {diags:#?}"
    );
}

#[test]
fn invalid_fixtures_yield_exactly_their_code() {
    let fixtures = common::load_invalid();
    assert!(!fixtures.is_empty(), "no invalid fixtures found");

    for (name, fixture) in &fixtures {
        // Filename must start with the expected code.
        assert!(
            name.starts_with(&fixture.expect),
            "fixture {name} does not start with its expected code {}",
            fixture.expect
        );

        let raw = common::apply(fixture);
        let diags = check_campaign(&raw);
        let codes: BTreeSet<&str> = diags.iter().map(|d| d.code.as_str()).collect();
        let expected: BTreeSet<&str> = std::iter::once(fixture.expect.as_str()).collect();

        assert_eq!(
            codes, expected,
            "fixture {name} ({}) produced {diags:#?}",
            fixture.description
        );
    }
}

#[test]
fn matrix_covers_every_code() {
    // Every DW01xx code documented in the README must have at least one fixture.
    let fixtures = common::load_invalid();
    let covered: BTreeSet<String> = fixtures.iter().map(|(_, f)| f.expect.clone()).collect();
    let expected = [
        "DW0100", "DW0101", "DW0102", "DW0103", "DW0110", "DW0111", "DW0112", "DW0120", "DW0121",
        "DW0122", "DW0123", "DW0130", "DW0131", "DW0132", "DW0133", "DW0140", "DW0141", "DW0142",
        "DW0143", "DW0150", "DW0151", "DW0152", "DW0153", "DW0160", "DW0161", "DW0162", "DW0170",
        "DW0171", "DW0172", "DW0173",
    ];
    for code in expected {
        assert!(covered.contains(code), "no fixture covers {code}");
    }
}
