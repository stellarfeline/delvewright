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
use delvewright_grammar::split::SplitError;
use delvewright_grammar::{Box3, ExpandError, ExpandOptions, GuardLeaf, Limits, expand};

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
        "rule \"all\": division or remainder by zero, evaluating (4 / param:zero)\n  at: all\n  \
         scope: local 4x4x4 (x\u{2192}world x, y\u{2192}world y, z\u{2192}world z; world box \
         corner 0,0,0 size 4x4x4)",
        "an error must not print a Debug struct inside a sentence — and it must name the \
         expression that failed and the scope it was evaluated against"
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

// ---------------------------------------------------------------------------
// Guard-exhaustion forensics.
//
// A refusal used to say only which rule refused. On a real campaign zone that
// cost a brute-force region sweep: three hand-guessed regions each named a
// different rule, and the scope reaching each rule had been through two
// reorientations and a chain of splits, so the dimensions at the failure site
// were not the dimensions on the command line. These tests hold the refusal to
// its claim: every rejected alternative names the comparisons that failed,
// with BOTH operands as evaluated at that scope — asserted on the structured
// values AND on the prose, because a message is a claim and an untested claim
// rots.
// ---------------------------------------------------------------------------

/// The exhaustion report of one rule, or a panic with the error's own prose.
fn exhaustion(
    err: &ExpandError,
) -> (
    &str,
    &delvewright_grammar::ScopeAt,
    &[delvewright_grammar::RejectedAlternative],
    &[String],
) {
    match err {
        ExpandError::NoApplicableRule {
            symbol,
            scope,
            rejected,
            path,
        } => (symbol, scope, rejected, path),
        other => panic!("expected guard exhaustion, got: {other}"),
    }
}

/// **Every failed conjunct of an `all` is reported, not just the first.** An
/// author handed one constraint at a time re-runs into the next; the point is
/// the whole system of constraints in one refusal.
#[test]
fn guard_exhaustion_reports_every_failed_comparison_with_its_operands() {
    let p = program(
        r#"{ "name": "forensics", "start": "plan",
             "params": { "run": 10, "margin": 4 },
             "rules": { "plan": [
               { "when": { "cond": "all", "of": [
                   { "cond": "cmp", "lhs": { "expr": "dim", "dim": "x" }, "op": "ge",
                     "rhs": { "expr": "int", "value": 6 } },
                   { "cond": "cmp", "lhs": { "expr": "param", "name": "run" }, "op": "le",
                     "rhs": { "expr": "arith",
                              "lhs": { "expr": "dim", "dim": "z" }, "op": "sub",
                              "rhs": { "expr": "param", "name": "margin" } } },
                   { "cond": "cmp", "lhs": { "expr": "dim", "dim": "y" }, "op": "ge",
                     "rhs": { "expr": "int", "value": 3 } } ] },
                 "body": { "op": "fill", "material": "minecraft:stone" } } ] } }"#,
    );
    // dim:x = 5 < 6 fails; param:run = 10 > dim:z - param:margin = 9 - 4 = 5
    // fails; dim:y = 4 >= 3 holds. Two of three conjuncts must be reported.
    let err = expand(&p, Box3::at_origin([5, 4, 9]), &ExpandOptions::seeded(0)).unwrap_err();
    let (symbol, scope, rejected, path) = exhaustion(&err);
    assert_eq!(symbol, "plan");
    assert_eq!(scope.local, [5, 4, 9]);
    assert_eq!(path, &["plan".to_string()]);
    assert_eq!(rejected.len(), 1, "one non-otherwise alternative");
    assert_eq!(rejected[0].index, 1);
    assert_eq!(
        rejected[0].failed.len(),
        2,
        "both failed conjuncts, and only those: {err}"
    );
    // The operands the message prints, asserted as values.
    assert_eq!(
        rejected[0].failed[0],
        GuardLeaf::Cmp {
            rendered: "dim:x >= 6".to_string(),
            lhs: 5,
            rhs: 6,
            bindings: vec![],
            held: false,
        }
    );
    assert_eq!(
        rejected[0].failed[1],
        GuardLeaf::Cmp {
            rendered: "param:run <= (dim:z - param:margin)".to_string(),
            lhs: 10,
            rhs: 5,
            bindings: vec![("dim:z".to_string(), 9), ("param:margin".to_string(), 4)],
            held: false,
        }
    );
    // ...and as prose: expression, both evaluated operands, and the bindings
    // inside the composite operand.
    let text = err.to_string();
    for want in [
        "required dim:x >= 6; at this scope left = 5, right = 6",
        "required param:run <= (dim:z - param:margin); at this scope left = 10, right = 5",
        "[dim:z = 9, param:margin = 4]",
        "local 5x4x9",
        "not the region as given",
    ] {
        assert!(text.contains(want), "missing {want:?} in:\n{text}");
    }
}

/// **The reported dimensions are the failure site's, not the region as
/// typed.** A reorientation between the command line and the failing rule is
/// exactly what made the old refusal unactionable.
#[test]
fn guard_exhaustion_reports_the_scope_after_reorientation_with_its_path() {
    let p = program(
        r#"{ "name": "turned", "start": "outer",
             "rules": {
               "outer": [ { "body": { "op": "reorient",
                 "orient": { "z": "world_x" },
                 "body": { "op": "call", "symbol": "inner" } } } ],
               "inner": [
                 { "when": { "cond": "cmp", "lhs": { "expr": "dim", "dim": "z" }, "op": "ge",
                             "rhs": { "expr": "int", "value": 100 } },
                   "body": { "op": "fill", "material": "minecraft:stone" } } ] } }"#,
    );
    // Region 7x4x9. "My Z is the old X" swaps X and Z, so inner reads
    // dim:z = 7 (world x) and dim:x = 9 (world z).
    let err = expand(&p, Box3::at_origin([7, 4, 9]), &ExpandOptions::seeded(0)).unwrap_err();
    let (symbol, scope, rejected, path) = exhaustion(&err);
    assert_eq!(symbol, "inner");
    assert_eq!(
        scope.local,
        [9, 4, 7],
        "local extents read through the turn"
    );
    assert_eq!(scope.size, [7, 4, 9], "world extents unmoved");
    assert_eq!(path, &["outer".to_string(), "inner".to_string()]);
    assert_eq!(
        rejected[0].failed[0],
        GuardLeaf::Cmp {
            rendered: "dim:z >= 100".to_string(),
            lhs: 7,
            rhs: 100,
            bindings: vec![],
            held: false,
        },
        "dim:z must evaluate through the reorientation: 7, not the typed 9"
    );
    let text = err.to_string();
    assert!(
        text.contains("local 9x4x7") && text.contains("z\u{2192}world x"),
        "{text}"
    );
}

/// **The path names the split piece that produced the failing scope**, because
/// the rule name alone does not locate one piece among its siblings.
#[test]
fn guard_exhaustion_path_names_the_split_piece() {
    let p = program(
        r#"{ "name": "pieces", "start": "lane",
             "rules": {
               "lane": [ { "body": { "op": "split", "axis": "z",
                 "sizes": [ { "size": "absolute", "blocks": { "expr": "int", "value": 3 } },
                            { "size": "relative", "weight": { "expr": "int", "value": 1 } } ],
                 "children": [ { "op": "skip" },
                               { "op": "call", "symbol": "inner" } ] } } ],
               "inner": [
                 { "when": { "cond": "cmp", "lhs": { "expr": "dim", "dim": "z" }, "op": "ge",
                             "rhs": { "expr": "int", "value": 100 } },
                   "body": { "op": "void" } } ] } }"#,
    );
    let err = expand(&p, Box3::at_origin([4, 4, 9]), &ExpandOptions::seeded(0)).unwrap_err();
    let (_, scope, _, path) = exhaustion(&err);
    assert_eq!(
        path,
        &[
            "lane".to_string(),
            "split z\u{2192}z piece 2/2".to_string(),
            "inner".to_string()
        ]
    );
    assert_eq!(
        scope.origin,
        [0, 0, 3],
        "the second piece starts after the absolute 3"
    );
    assert_eq!(scope.local, [4, 4, 6]);
}

/// **A `none_of` rejection names the comparison that HELD**, with the same
/// operands — and an `orientation` guard reports required against actual.
#[test]
fn guard_exhaustion_explains_none_of_and_orientation_leaves() {
    let p = program(
        r#"{ "name": "negated", "start": "plan",
             "rules": { "plan": [
               { "when": { "cond": "none_of", "of": [
                   { "cond": "cmp", "lhs": { "expr": "dim", "dim": "x" }, "op": "eq",
                     "rhs": { "expr": "int", "value": 4 } } ] },
                 "body": { "op": "void" } },
               { "when": { "cond": "orientation", "x": "z", "y": "y", "z": "x" },
                 "body": { "op": "void" } } ] } }"#,
    );
    let err = expand(&p, Box3::at_origin([4, 4, 4]), &ExpandOptions::seeded(0)).unwrap_err();
    let (_, _, rejected, _) = exhaustion(&err);
    assert_eq!(rejected.len(), 2);
    assert_eq!(
        rejected[0].failed[0],
        GuardLeaf::Cmp {
            rendered: "dim:x == 4".to_string(),
            lhs: 4,
            rhs: 4,
            bindings: vec![],
            held: true,
        }
    );
    assert!(
        matches!(
            &rejected[1].failed[0],
            GuardLeaf::Orientation { held: false, .. }
        ),
        "{err}"
    );
    let text = err.to_string();
    assert!(
        text.contains(
            "forbidden (under none_of) dim:x == 4; at this scope it held, left = 4, right = 4"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "required orientation x\u{2192}z, y\u{2192}y, z\u{2192}x; this scope has \
             x\u{2192}x, y\u{2192}y, z\u{2192}z"
        ),
        "{text}"
    );
}

/// **A conjunct the short-circuiting test never reached is reported as
/// unevaluable, not silently dropped** — the exhaustion is still the story,
/// and the report stays total.
#[test]
fn guard_exhaustion_reports_an_unevaluable_conjunct_by_name() {
    let p = program(
        r#"{ "name": "half_dark", "start": "plan", "params": { "zero": 0 },
             "rules": { "plan": [
               { "when": { "cond": "all", "of": [
                   { "cond": "cmp", "lhs": { "expr": "dim", "dim": "x" }, "op": "ge",
                     "rhs": { "expr": "int", "value": 6 } },
                   { "cond": "cmp",
                     "lhs": { "expr": "arith", "lhs": { "expr": "int", "value": 1 },
                              "op": "div", "rhs": { "expr": "param", "name": "zero" } },
                     "op": "gt", "rhs": { "expr": "int", "value": 0 } } ] },
                 "body": { "op": "void" } } ] } }"#,
    );
    let err = expand(&p, Box3::at_origin([5, 4, 4]), &ExpandOptions::seeded(0)).unwrap_err();
    let (_, _, rejected, _) = exhaustion(&err);
    assert_eq!(rejected[0].failed.len(), 2);
    assert_eq!(
        rejected[0].failed[1],
        GuardLeaf::Unevaluable {
            rendered: "(1 / param:zero) > 0".to_string(),
            error: "division or remainder by zero".to_string(),
        }
    );
}

/// **The sibling refusals carry the same sight**: a split overflow names its
/// axis, its evaluated size pattern, the scope and the path; a bad size names
/// the expression it came from.
#[test]
fn split_refusals_name_their_pattern_scope_and_path() {
    let p = program(
        r#"{ "name": "tight", "start": "outer",
             "params": { "need": 10 },
             "rules": {
               "outer": [ { "body": { "op": "call", "symbol": "band" } } ],
               "band": [ { "body": { "op": "split", "axis": "x",
                 "sizes": [ { "size": "absolute",
                              "blocks": { "expr": "param", "name": "need" } } ],
                 "children": [ { "op": "void" } ] } } ] } }"#,
    );
    let err = expand(&p, Box3::at_origin([4, 4, 4]), &ExpandOptions::seeded(0)).unwrap_err();
    match &err {
        ExpandError::Split {
            symbol,
            error,
            pattern,
            scope,
            path,
            ..
        } => {
            assert_eq!(symbol, "band");
            assert_eq!(
                error,
                &SplitError::Overflow {
                    absolute: 10,
                    extent: 4
                }
            );
            assert_eq!(pattern, "abs param:need = 10");
            assert_eq!(scope.local, [4, 4, 4]);
            assert_eq!(path, &["outer".to_string(), "band".to_string()]);
        }
        other => panic!("expected a split overflow, got: {other}"),
    }
    let text = err.to_string();
    assert!(
        text.contains("needs 10 blocks along local x (world x)")
            && text.contains("sizes: abs param:need = 10")
            && text.contains("at: outer \u{203a} band"),
        "{text}"
    );

    // A weight that evaluates to zero names the expression that produced it.
    let p = program(
        r#"{ "name": "flat", "start": "band", "params": { "w": 0 },
             "rules": { "band": [ { "body": { "op": "split", "axis": "x",
               "sizes": [ { "size": "relative",
                            "weight": { "expr": "param", "name": "w" } } ],
               "children": [ { "op": "void" } ] } } ] } }"#,
    );
    let err = expand(&p, Box3::at_origin([4, 4, 4]), &ExpandOptions::seeded(0)).unwrap_err();
    assert!(
        matches!(&err, ExpandError::BadSize { value: 0, expr, .. } if expr == "param:w"),
        "{err}"
    );
    assert!(
        err.to_string()
            .contains("0 is not a usable split size \u{2014} from param:w"),
        "{err}"
    );
}
