//! The grammar half of the blockstate diagnostic family: `DW0735` (a state
//! omitting a shape-carrying property), `DW0736` (an oriented state filled into
//! a reoriented scope with no `orientation` guard) and `DW0737` (a state
//! omitting any property at all).
//!
//! Both are demonstrated red→green on the **real** defect this family was
//! ruled from: `broken_grate`'s bars. The shipped rule filled a bare
//! `iron_bars` — an isolated-post row, the `DW0735` shape — and the naive
//! repair (write the connections) is itself the `DW0736` shape the moment the
//! zone hands the piece a box whose long axis is world X, because a
//! reorientation permutes geometry and never rewrites properties. The shipped
//! fix is one alternative per orientation under `Cond::Orientation`, and the
//! green half of each test proves the guard *selects* the matching variant
//! rather than merely silencing the gate.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::export::{ExportError, export_zone};
use delvewright_grammar::gates;
use delvewright_grammar::ir::{Cond, Material, Node, Program};
use delvewright_grammar::library::broken_grate::broken_grate;
use delvewright_grammar::{Box3, ExpandOptions, expand};

/// The row along local Z (identity at this region): bars connect north/south.
const ROW_ALONG_Z: Box3 = Box3::at_origin([3, 5, 14]);
/// The same box turned: `z: largest` maps local Z onto world X.
const ROW_ALONG_X: Box3 = Box3::at_origin([14, 5, 3]);

fn fill_state(state: &str) -> Node {
    Node::Fill {
        material: Material::block(state.parse::<BlockState>().unwrap()),
    }
}

fn gate<'r>(report: &'r gates::Report, id: &str) -> &'r gates::Gate {
    report
        .gates
        .iter()
        .find(|g| g.id == id)
        .unwrap_or_else(|| panic!("no `{id}` gate"))
}

/// `broken_grate` exactly as it shipped before the fix: the bars a bare
/// `iron_bars` with no properties at all.
fn grate_with_bare_bars() -> Program {
    let mut program = broken_grate();
    let bars = vec![delvewright_grammar::ir::Alternative::new(fill_state(
        "iron_bars",
    ))];
    program.rules.insert("grate_bars".to_string(), bars);
    program
}

/// The naive repair: connections written for the identity orientation, no
/// `orientation` guard.
fn grate_with_unguarded_connections() -> Program {
    let mut program = broken_grate();
    let bars = vec![delvewright_grammar::ir::Alternative::new(fill_state(
        "iron_bars[east=false,north=true,south=true,west=false]",
    ))];
    program.rules.insert("grate_bars".to_string(), bars);
    program
}

// ---------------------------------------------------------------------------
// DW0735 — the shape defect
// ---------------------------------------------------------------------------

/// RED: the pre-fix rule. The bars carry no connection properties, so every
/// cell places as an isolated post — the gate reds and names `DW0735`, and the
/// export refuses for callers that never read a gate report.
#[test]
fn a_bare_bars_fill_reds_the_shape_gate_and_the_export() {
    let program = grate_with_bare_bars();
    let out = expand(&program, ROW_ALONG_Z, &ExpandOptions::seeded(1)).unwrap();
    let report = gates::judge(&out, gates::Options::default());
    let g = gate(&report, "shape-complete");
    assert!(!g.pass);
    assert!(g.bound > 0, "the gate bound nothing");
    assert!(g.detail.contains("DW0735"), "{}", g.detail);
    assert!(
        g.detail.contains("east, north, south, west"),
        "{}",
        g.detail
    );

    let err = export_zone(&program, ROW_ALONG_Z, &ExpandOptions::seeded(1), "grate").unwrap_err();
    assert!(matches!(err, ExportError::ShapeOmissions { .. }), "{err}");
    assert!(err.to_string().contains("DW0735"), "{err}");
}

/// GREEN: the shipped rule writes the full connection state and passes — and
/// the benign class stays benign: the row's cobblestone and the break's mossy
/// cobblestone have no shape-carrying properties to write.
#[test]
fn the_shipped_grate_writes_its_connections_and_passes() {
    let out = expand(&broken_grate(), ROW_ALONG_Z, &ExpandOptions::seeded(1)).unwrap();
    let report = gates::judge(&out, gates::Options::default());
    let g = gate(&report, "shape-complete");
    assert!(g.pass, "{}", g.detail);
    assert!(g.bound >= 4, "bound {} states", g.bound);
    assert!(
        out.model
            .palette()
            .iter()
            .any(|s| s.to_string()
                == "minecraft:iron_bars[east=false,north=true,south=true,waterlogged=false,west=false]"),
        "the row along Z must connect north/south"
    );
}

// ---------------------------------------------------------------------------
// DW0737 — the under-specification defect, of which DW0735 is the hard half
// ---------------------------------------------------------------------------

/// RED: a state that omits a property whose default is benign for the MODEL is
/// still a state no reader upstream of a running server can resolve.
///
/// `oak_stairs[facing=east]` is the live shape and it is deliberately not a
/// `DW0735`: `half`, `shape` and `waterlogged` are `variants` properties, so
/// the game picks one complete model and nothing falls off. What no document
/// then states is WHICH model — and vanilla recomputes `shape` from the stair's
/// neighbours on every block update, so the reviewer's render, the navigation
/// walk and the server can each hold a different stair. The church's roof
/// shipped four of these for as long as the port existed.
#[test]
fn a_state_that_omits_a_property_reds_the_completeness_gate() {
    let mut program = broken_grate();
    program.rules.insert(
        "grate_bars".to_string(),
        vec![delvewright_grammar::ir::Alternative::new(fill_state(
            "oak_stairs[facing=east]",
        ))],
    );
    let out = expand(&program, ROW_ALONG_Z, &ExpandOptions::seeded(1)).unwrap();
    let report = gates::judge(&out, gates::Options::default());

    let g = gate(&report, "states-complete");
    assert!(!g.pass, "{}", g.detail);
    assert!(g.bound > 0, "the gate bound nothing");
    assert!(g.detail.contains("DW0737"), "{}", g.detail);
    assert!(
        g.detail.contains("half, shape, waterlogged"),
        "{}",
        g.detail
    );

    // ...and the class line holds: this is NOT a shape defect, so the harder
    // gate stays green. A `DW0737` that also reds `DW0735` would mean the two
    // are one rule and one of them is redundant.
    assert!(gate(&report, "shape-complete").pass);
}

/// GREEN: the same stair with every property written passes, and the corpus it
/// came from passes with it.
#[test]
fn a_fully_written_state_passes_the_completeness_gate() {
    let mut program = broken_grate();
    program.rules.insert(
        "grate_bars".to_string(),
        vec![delvewright_grammar::ir::Alternative::new(fill_state(
            "oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]",
        ))],
    );
    let out = expand(&program, ROW_ALONG_Z, &ExpandOptions::seeded(1)).unwrap();
    let report = gates::judge(&out, gates::Options::default());
    let g = gate(&report, "states-complete");
    assert!(g.pass, "{}", g.detail);
    assert!(g.bound >= 4, "bound {} states", g.bound);
}

/// A bare wall is BOTH: the shape half fires and so does the whole class. This
/// is what pins `DW0735` as a strict subset rather than a sibling rule.
#[test]
fn a_bare_wall_reds_both_halves_of_the_family() {
    let program = grate_with_bare_bars();
    let out = expand(&program, ROW_ALONG_Z, &ExpandOptions::seeded(1)).unwrap();
    let report = gates::judge(&out, gates::Options::default());
    assert!(!gate(&report, "shape-complete").pass);
    let whole = gate(&report, "states-complete");
    assert!(!whole.pass);
    assert!(whole.detail.contains("DW0737"), "{}", whole.detail);
    // The whole class names every omitted property; the shape half names four.
    assert!(whole.detail.contains("waterlogged"), "{}", whole.detail);
}

// ---------------------------------------------------------------------------
// DW0736 — the orientation defect
// ---------------------------------------------------------------------------

/// The naive repair is correct at the identity region…
#[test]
fn unguarded_connections_pass_where_nothing_is_reoriented() {
    let program = grate_with_unguarded_connections();
    let out = expand(&program, ROW_ALONG_Z, &ExpandOptions::seeded(1)).unwrap();
    let report = gates::judge(&out, gates::Options::default());
    let g = gate(&report, "oriented-fills");
    assert!(g.pass, "{}", g.detail);
    assert!(g.bound > 0);
}

/// …and RED the moment the zone hands the piece a box whose long axis is
/// world X: the root's `z: largest` reorients the scope, the literal
/// north/south connections land unrotated, and the row reads as a line of
/// crosses instead of a grate. The gate names `DW0736`, the rule and the
/// offending property; the export refuses.
#[test]
fn unguarded_connections_red_the_orientation_gate_under_a_turned_box() {
    let program = grate_with_unguarded_connections();
    let out = expand(&program, ROW_ALONG_X, &ExpandOptions::seeded(1)).unwrap();
    let report = gates::judge(&out, gates::Options::default());
    let g = gate(&report, "oriented-fills");
    assert!(!g.pass);
    assert!(g.detail.contains("DW0736"), "{}", g.detail);
    assert!(g.detail.contains("grate_bars"), "{}", g.detail);
    assert!(g.detail.contains("orientation"), "{}", g.detail);

    let err = export_zone(&program, ROW_ALONG_X, &ExpandOptions::seeded(1), "grate").unwrap_err();
    assert!(
        matches!(err, ExportError::UnguardedOrientedFills { .. }),
        "{err}"
    );
    assert!(err.to_string().contains("DW0736"), "{err}");
}

/// GREEN, and not by silencing: under the turned box the shipped rule's
/// `orientation` guard selects the OTHER variant — east/west connections — so
/// the grate still reads as a run of bars. The pin only stands while the
/// orientation the guard proved still holds, which is what licenses the fill.
#[test]
fn the_shipped_grate_selects_the_matching_variant_under_a_turned_box() {
    let out = expand(&broken_grate(), ROW_ALONG_X, &ExpandOptions::seeded(1)).unwrap();
    assert!(
        out.oriented.carrying > 0,
        "the guarded fill must be examined, not skipped"
    );
    let report = gates::judge(&out, gates::Options::default());
    let g = gate(&report, "oriented-fills");
    assert!(g.pass, "{}", g.detail);
    assert!(
        out.model
            .palette()
            .iter()
            .any(|s| s.to_string()
                == "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]"),
        "the row along X must connect east/west: {:?}",
        out.model
            .palette()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
    export_zone(
        &broken_grate(),
        ROW_ALONG_X,
        &ExpandOptions::seeded(1),
        "grate",
    )
    .expect("the guarded piece exports");
}

/// The pin is the *guard's* orientation, not a blanket licence: a guard that
/// passes and is then reoriented underneath no longer covers the fill.
#[test]
fn a_reorientation_below_the_guard_voids_the_pin() {
    use delvewright_grammar::geom::Axis;
    use delvewright_grammar::ir::{Alternative, AxisSpec, Reorient};
    let program = Program::new("turned_below_guard", "start")
        .rule(
            "start",
            Node::Reorient {
                // Identity request resolved over a box longer in Z keeps
                // identity; the guard below then passes…
                orient: Reorient::KEEP,
                body: Box::new(Node::call("guarded")),
            },
        )
        .rule_alts(
            "guarded",
            vec![
                Alternative::new(Node::Reorient {
                    // …but the body turns the scope again before the fill.
                    orient: Reorient::KEEP.z(AxisSpec::LocalX),
                    body: Box::new(fill_state("oak_stairs[facing=north,half=bottom]")),
                })
                .when(Cond::Orientation {
                    x: Axis::X,
                    y: Axis::Y,
                    z: Axis::Z,
                }),
            ],
        );
    let out = expand(
        &program,
        Box3::at_origin([4, 3, 8]),
        &ExpandOptions::seeded(0),
    )
    .unwrap();
    assert_eq!(
        out.oriented.unguarded.len(),
        1,
        "{:?}",
        out.oriented.unguarded
    );
    assert_eq!(out.oriented.unguarded[0].property, "facing=north");
}
