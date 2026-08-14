//! Failure is loud, and legible.
//!
//! `docs/reference/grammar.md` §4 promises the interpreter has no silent
//! degradation. These are the cases the PR #266 review found where it either
//! degraded quietly, aborted the process, or reported something a human could
//! not read.

use delvewright_grammar::eval::EvalError;
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{Program, ProgramError};
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
        r#"{ "version": "1.2.0", "name": "solid", "start": "all",
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
        r#"{ "version": "1.2.0", "name": "impossible", "start": "all",
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
        r#"{ "version": "1.2.0", "name": "possible", "start": "all",
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
        r#"{ "version": "1.2.0", "name": "divzero", "start": "all", "params": { "zero": 0 },
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
        r#"{ "version": "1.2.0", "name": "conflict", "start": "all",
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
