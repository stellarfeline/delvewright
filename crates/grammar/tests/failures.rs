//! Failure is loud, and legible.
//!
//! `docs/reference/grammar.md` §4 promises the interpreter has no silent
//! degradation. These are the cases the PR #266 review found where it either
//! degraded quietly, aborted the process, or reported something a human could
//! not read.

use delvewright_grammar::eval::EvalError;
use delvewright_grammar::expand::ExpandError;
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{Program, ProgramError};
use delvewright_grammar::library::bell::chapel_ward;
use delvewright_grammar::orient::OrientError;
use delvewright_grammar::{Box3, ExpandOptions, Limits, expand};

fn program(json: &str) -> Program {
    serde_json::from_str(json).expect("the test program parses")
}

/// A model is a dense grid allocated before the first rule runs. An absurd
/// region used to be an allocation the process might not survive — and a killed
/// process reports nothing at all, which is the one failure mode a machine gate
/// cannot work with.
#[test]
fn an_absurd_region_is_a_diagnostic_not_an_allocation() {
    let p = program(
        r#"{ "name": "solid", "start": "all",
             "rules": { "all": [{ "body": { "op": "fill",
               "material": "minecraft:stone" } }] } }"#,
    );
    // ~4.6e10 cells: two orders of magnitude past any machine's RAM.
    let huge = Box3::at_origin([3600, 3600, 3600]);
    let err = expand(&p, huge, &ExpandOptions::seeded(0)).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("volume limit"), "{text}");
    assert!(
        text.contains(&Limits::default().max_volume.to_string()),
        "the diagnostic must name the budget it hit: {text}"
    );

    // The budget is a deliberate control, never a silent clamp: a region that
    // fits the default is refused under a tighter one and built under it, and
    // in neither case does a model come back quietly truncated.
    let modest = Box3::at_origin([40, 40, 40]);
    let tight = ExpandOptions {
        limits: Limits {
            max_volume: 1_000,
            ..Limits::default()
        },
        ..ExpandOptions::seeded(0)
    };
    let err = expand(&p, modest, &tight).unwrap_err();
    assert!(err.to_string().contains("64000 cells"), "{err}");
    let out = expand(&p, modest, &ExpandOptions::seeded(0)).unwrap();
    assert_eq!(out.model.region(), modest);
    assert_eq!(out.model.filled_cells(), 64_000);
}

/// An orientation is a permutation by definition, but the `orientation` guard
/// spells one out field by field, so a non-permutation is expressible. It can
/// never match, and at expansion time that surfaced as a `NoApplicableRule`
/// about some *other* alternative — a diagnostic pointing away from the defect.
#[test]
fn an_orientation_guard_that_is_not_a_permutation_is_refused_where_it_is_written() {
    let p = program(
        r#"{ "name": "impossible", "start": "all",
             "rules": { "all": [
               { "when": { "cond": "orientation", "x": "z", "y": "z", "z": "z" },
                 "body": { "op": "fill", "material": "minecraft:stone" } },
               { "when": { "cond": "otherwise" }, "body": { "op": "void" } }
             ] } }"#,
    );
    let err = p.validate().unwrap_err();
    assert_eq!(
        err,
        ProgramError::OrientationCondNotAPermutation {
            symbol: "all".to_string(),
            axes: [Axis::Z, Axis::Z, Axis::Z],
        }
    );
    assert!(err.to_string().contains("not a permutation"), "{err}");
    // ...and expansion refuses it too, because `expand` validates first.
    let err = expand(&p, Box3::at_origin([4, 4, 4]), &ExpandOptions::seeded(0)).unwrap_err();
    assert!(err.to_string().contains("not a permutation"), "{err}");

    // A real permutation still passes, guard and all.
    let ok = program(
        r#"{ "name": "possible", "start": "all",
             "rules": { "all": [
               { "when": { "cond": "orientation", "x": "x", "y": "y", "z": "z" },
                 "body": { "op": "fill", "material": "minecraft:stone" } },
               { "when": { "cond": "otherwise" }, "body": { "op": "void" } }
             ] } }"#,
    );
    ok.validate().unwrap();
    let out = expand(&ok, Box3::at_origin([4, 4, 4]), &ExpandOptions::seeded(0)).unwrap();
    assert_eq!(out.model.filled_cells(), 64);
}

/// Every error the interpreter can report reaches a human as prose. These two
/// reached them as `Debug` structs printed inside an otherwise-English sentence.
#[test]
fn nested_errors_read_as_sentences_not_as_debug_structs() {
    assert_eq!(
        EvalError::DivideByZero.to_string(),
        "division or remainder by zero"
    );
    assert!(
        EvalError::UnknownParam {
            name: "column_height".to_string()
        }
        .to_string()
        .contains("\"column_height\" is not declared")
    );
    assert!(
        OrientError::Conflict { axis: Axis::Z }
            .to_string()
            .contains("permutation")
    );

    // ...and that is what an expansion failure actually prints.
    let p = program(
        r#"{ "name": "divzero", "start": "all", "params": { "zero": 0 },
             "rules": { "all": [{ "body": {
               "op": "split", "axis": "y",
               "sizes": [
                 { "size": "absolute", "blocks": { "expr": "arith",
                   "lhs": { "expr": "int", "value": 4 }, "op": "div",
                   "rhs": { "expr": "param", "name": "zero" } } },
                 { "size": "relative", "weight": { "expr": "int", "value": 1 } }
               ],
               "children": [{ "op": "fill", "material": "minecraft:stone" },
                            { "op": "void" }] } }] } }"#,
    );
    let err = expand(&p, Box3::at_origin([4, 4, 4]), &ExpandOptions::seeded(0)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "rule \"all\": division or remainder by zero",
        "an error must not print a Debug struct inside a sentence"
    );

    let p = program(
        r#"{ "name": "conflict", "start": "all",
             "rules": { "all": [{ "body": {
               "op": "reorient", "orient": { "x": "local_z", "y": "local_z" },
               "body": { "op": "fill", "material": "minecraft:stone" } } }] } }"#,
    );
    let err = expand(&p, Box3::at_origin([4, 4, 4]), &ExpandOptions::seeded(0)).unwrap_err();
    assert!(
        err.to_string()
            .starts_with("rule \"all\": the reorientation names"),
        "{err}"
    );
}

/// **A refusal is the most informative event in a sweep, and it used to carry
/// the least information of anything the tool printed.**
///
/// `bell:chapel-ward`'s frame guard is a four-clause conjunction, and candidates
/// breaking different clauses of it all printed the same sentence, because the
/// message named the rule and withheld the reading — an author could not even
/// tell those cases apart. This is the `great-hearth` candidate, held to what
/// they must be able to deduce the next candidate from.
#[test]
fn a_guard_refusal_states_the_clause_the_values_and_the_distance() {
    let mut zone = chapel_ward();
    // `great-hearth`: a rest ward long enough to starve the chute of its run.
    zone.set_param("hearth_run", 14).unwrap();
    let region = Box3::at_origin(chapel_ward::REGION);
    let err = expand(&zone, region, &ExpandOptions::seeded(11)).unwrap_err();

    let ExpandError::NoApplicableRule { refusal } = &err else {
        panic!("a guard refused, so the error is a refusal: {err}");
    };
    assert_eq!(refusal.symbol, "ward_plan");

    let text = err.to_string();
    // The sentence that was the whole message before is still its first line.
    assert_eq!(
        text.lines().next().unwrap(),
        "no alternative of rule \"ward_plan\" applies to this scope, and none is `otherwise`"
    );

    // The box, in the frame the rule reads it in — the zone opens with
    // `z(Largest)`, so `Dimension.Z` is the long axis and not the box's Z by
    // luck. An author who cannot see that cannot use any of the numbers below.
    assert!(text.contains("scope: 16x9x26 at 0,0,0"), "{text}");
    assert!(text.contains("Dimension.Z = 26 (world Z)"), "{text}");

    // The conjunct that was false, named, with both sides measured...
    assert!(
        text.contains(
            "FALSE  Dimension.Z - junction_run - hearth_run > Dimension.X - strip_depth  4 > 7"
        ),
        "{text}"
    );
    // ...the derived operands broken into the knobs that move them — the number
    // alone would say how far off it is and nothing about what to change...
    assert!(
        text.contains("left  = 4   from Dimension.Z = 26, junction_run = 8, hearth_run = 14"),
        "{text}"
    );
    assert!(
        text.contains("right = 7   from Dimension.X = 16, strip_depth = 9"),
        "{text}"
    );
    // ...and the distance, from both sides.
    assert!(
        text.contains("4 short: the left must rise to 8, or the right fall to 3"),
        "{text}"
    );
    // The three clauses that held are shown holding, with their numbers: an
    // author fixing one clause must be able to see the headroom on the others
    // rather than discovering it on the next run.
    assert!(text.contains("ok     strip_depth > junction_run"), "{text}");
    assert!(
        text.contains("every clause must hold; 1 of 4 does not"),
        "{text}"
    );

    // The deduction the report is for: `26 - 8 - hearth_run` must reach 8, so
    // `hearth_run <= 10`. Ten builds; eleven is the same refusal one block on.
    let mut deduced = chapel_ward();
    deduced.set_param("hearth_run", 10).unwrap();
    expand(&deduced, region, &ExpandOptions::seeded(11)).expect("the deduced candidate builds");

    let mut one_over = chapel_ward();
    one_over.set_param("hearth_run", 11).unwrap();
    let err = expand(&one_over, region, &ExpandOptions::seeded(11)).unwrap_err();
    assert!(
        err.to_string()
            .contains("1 short: the left must rise to 8, or the right fall to 6"),
        "{err}"
    );
}

/// The report belongs to guard evaluation, not to the sweep that surfaced it:
/// a guard refusing anywhere says the same thing, including deep inside a
/// derivation where the scope is a sub-box no caller ever named.
#[test]
fn a_refusal_inside_a_derivation_reports_the_sub_box_it_was_handed() {
    let p = program(
        r#"{ "name": "nested", "start": "all", "params": { "floor": 6 },
             "rules": {
               "all": [{ "body": {
                 "op": "split", "axis": "z",
                 "sizes": [{ "size": "absolute", "blocks": { "expr": "int", "value": 3 } },
                           { "size": "relative", "weight": { "expr": "int", "value": 1 } }],
                 "children": [{ "op": "call", "symbol": "wing" }, { "op": "void" }] } }],
               "wing": [{ "when": { "cond": "cmp",
                   "lhs": { "expr": "dim", "dim": "z" }, "op": "ge",
                   "rhs": { "expr": "param", "name": "floor" } },
                 "body": { "op": "fill", "material": "minecraft:stone" } }] } }"#,
    );
    let err = expand(&p, Box3::at_origin([8, 4, 20]), &ExpandOptions::seeded(0)).unwrap_err();
    let text = err.to_string();
    // The whole region is 20 deep and passes the guard; the piece the rule was
    // actually handed is 3 deep, and that is the number the report must state.
    assert!(text.contains("scope: 8x4x3 at 0,0,0"), "{text}");
    assert!(
        text.contains("FALSE  Dimension.Z >= floor  3 >= 6"),
        "{text}"
    );
    assert!(
        text.contains("3 short: the left must rise to 6, or the right fall to 3"),
        "{text}"
    );
    assert!(text.contains("its one clause does not hold"), "{text}");
}
