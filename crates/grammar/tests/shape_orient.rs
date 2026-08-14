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
use delvewright_grammar::ir::{Cond, Material, Node, Paint, Program};
use delvewright_grammar::library::broken_grate::broken_grate;
use delvewright_grammar::library::far_side_bar::far_side_bar;
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

// ---------------------------------------------------------------------------
// The axis frame — the missing word, on the object that was missing it
// ---------------------------------------------------------------------------
//
// `Cond::Orientation` selects a rule ALTERNATIVE by orientation. It is the only
// mechanism there was for an oriented block, so a rule that needed one had to
// spell out a variant per orientation — and a variant is an inline state, so
// the piece lost its palette role: one name binds one state, and the state
// differed by orientation.
//
// `Paint::Local` says the other thing: not "under this orientation, write this"
// but "these properties are named in MY axes". One binding, resolved at fill
// time through the transform the `DW0736` predicate already runs to decide that
// an unframed literal landed wrong. `far_side_bar`'s bar is a role again.

/// The far-side bar in the two orientations its root can produce: a box longer
/// in world Z keeps identity, a box longer in world X turns the piece.
const BAR_ALONG_Z: Box3 = Box3::at_origin([5, 6, 11]);
const BAR_ALONG_X: Box3 = Box3::at_origin([11, 6, 5]);

fn bar_states(program: &Program, region: Box3) -> Vec<String> {
    let out = expand(program, region, &ExpandOptions::seeded(3)).unwrap();
    let mut states: Vec<String> = out
        .model
        .palette()
        .iter()
        .filter(|s| s.name == "minecraft:iron_bars")
        .map(ToString::to_string)
        .collect();
    states.sort();
    states
}

/// **GREEN, in both orientations, from ONE binding.** The role is written in
/// the scope's own axes — the bars span the wall's local X — and the piece is
/// turned underneath it. Nothing in the program mentions either orientation.
#[test]
fn one_local_frame_role_writes_the_right_bar_in_every_orientation() {
    let program = far_side_bar();
    assert!(
        program.palette.contains_key("bar"),
        "the bar is a palette role, so a campaign can restyle it"
    );
    assert!(program.palette["bar"].is_local());

    assert_eq!(
        bar_states(&program, BAR_ALONG_Z),
        ["minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]"],
        "local X is world X here"
    );
    assert_eq!(
        bar_states(&program, BAR_ALONG_X),
        ["minecraft:iron_bars[east=false,north=true,south=true,waterlogged=false,west=false]"],
        "local X is world Z here, so the same binding turns with the piece"
    );

    for region in [BAR_ALONG_Z, BAR_ALONG_X] {
        let out = expand(&program, region, &ExpandOptions::seeded(3)).unwrap();
        assert_eq!(out.oriented.resolved, 1, "the frame's binding count");
        assert!(out.oriented.carrying >= 1, "and it is still examined");
        let report = gates::judge(&out, gates::Options::default());
        assert!(gate(&report, "oriented-fills").pass);
        assert!(gate(&report, "shape-complete").pass);
        assert!(gate(&report, "states-complete").pass);
        export_zone(&program, region, &ExpandOptions::seeded(3), "bar")
            .expect("a framed piece exports in either orientation");
    }
}

/// **RED, the same piece, the mechanism misused.** Rebinding the role to the
/// identical state in the WORLD frame is the whole difference: it is right
/// where nothing turned and wrong the moment the piece does, which is the
/// `DW0736` shape the frame exists to remove. The gate still binds — same
/// fill, same population — and now it fails.
#[test]
fn dropping_the_frame_reds_the_same_bar_under_a_turned_box() {
    let mut program = far_side_bar();
    let state = program.palette["bar"].states().each()[0].clone();
    program
        .set_role("bar", Paint::block(state))
        .expect("the role is bound");

    let flat = expand(&program, BAR_ALONG_Z, &ExpandOptions::seeded(3)).unwrap();
    assert_eq!(flat.oriented.resolved, 0, "no frame left to resolve");
    assert!(
        gates::judge(&flat, gates::Options::default())
            .gates
            .iter()
            .find(|g| g.id == "oriented-fills")
            .unwrap()
            .pass,
        "a world literal is correct while nothing turns"
    );

    let turned = expand(&program, BAR_ALONG_X, &ExpandOptions::seeded(3)).unwrap();
    let report = gates::judge(&turned, gates::Options::default());
    let g = gate(&report, "oriented-fills");
    assert!(!g.pass, "{}", g.detail);
    assert_eq!(g.bound, flat.oriented.fills as usize, "the same population");
    assert!(g.detail.contains("DW0736"), "{}", g.detail);
    let err = export_zone(&program, BAR_ALONG_X, &ExpandOptions::seeded(3), "bar").unwrap_err();
    assert!(
        matches!(err, ExportError::UnguardedOrientedFills { .. }),
        "{err}"
    );
}

/// **RED the other way: the frame asked for an image that does not exist.** A
/// yaw is stated against a fixed vertical, so a scope that calls a horizontal
/// axis its local `Y` leaves `rotation` nothing to mean. The expansion refuses
/// with `DW0738` and names the property — it never writes a plausible skull.
#[test]
fn a_local_frame_state_with_no_image_refuses_instead_of_guessing() {
    use delvewright_grammar::ir::{Alternative, AxisSpec, Reorient};
    let program = Program::new("tipped", "start")
        .role_local(
            "corpse",
            "minecraft:skeleton_skull[powered=false,rotation=8]"
                .parse::<BlockState>()
                .unwrap(),
        )
        .rule_alts(
            "start",
            vec![Alternative::new(Node::Reorient {
                // Local Y onto world Z: the scope's own "up" is a horizontal
                // world axis, so nothing in the yaw vocabulary survives.
                orient: Reorient::KEEP.y(AxisSpec::WorldZ).z(AxisSpec::WorldY),
                body: Box::new(Node::fill("corpse")),
            })],
        );
    let err = expand(
        &program,
        Box3::at_origin([3, 3, 3]),
        &ExpandOptions::seeded(0),
    )
    .unwrap_err();
    let text = err.to_string();
    assert!(text.contains("DW0738"), "{text}");
    assert!(text.contains("rotation=8"), "{text}");
    assert!(text.contains("x->X,y->Z,z->Y"), "{text}");

    // Same program, same role, a scope that keeps its vertical: resolved, not
    // refused. The refusal is about the orientation, never about the frame.
    let upright = Program::new("upright", "start")
        .role_local(
            "corpse",
            "minecraft:skeleton_skull[powered=false,rotation=8]"
                .parse::<BlockState>()
                .unwrap(),
        )
        .rule("start", Node::fill("corpse"));
    let out = expand(
        &upright,
        Box3::at_origin([3, 3, 3]),
        &ExpandOptions::seeded(0),
    )
    .unwrap();
    assert_eq!(out.oriented.resolved, 1);
}

/// **Determinism, and a control that says the comparison discriminates.** The
/// frame is resolved from the scope, never from a draw, so the same program at
/// the same seed is byte-identical — and a different seed is not, which is what
/// proves the equality above is measuring something.
#[test]
fn a_framed_program_is_byte_stable_at_a_seed_and_moves_with_one() {
    let program = far_side_bar();
    let bytes = |seed: u64, region: Box3| {
        expand(&program, region, &ExpandOptions::seeded(seed))
            .unwrap()
            .model
            .canonical_bytes()
    };
    for region in [BAR_ALONG_Z, BAR_ALONG_X] {
        assert_eq!(bytes(3, region), bytes(3, region));
    }
    // The negative control is the ORIENTATION, because the seed does not reach
    // this piece: `far_side_bar` has no weighted alternative, so seed 3 and
    // seed 9 give the same model by construction, and pinning them equal would
    // be a comparison that discriminates nothing. Turning the box does move
    // the bytes, and that is the axis this test is about.
    assert_eq!(bytes(3, BAR_ALONG_Z), bytes(9, BAR_ALONG_Z));
    assert_ne!(bytes(3, BAR_ALONG_Z), bytes(3, BAR_ALONG_X));
}

/// A `--role` override is a restyle: it says which material, and its syntax has
/// no word for a frame. So it keeps the frame of the binding it replaces —
/// otherwise swapping the bar's material would silently re-point every
/// connection in the piece.
#[test]
fn a_restyle_of_a_framed_role_keeps_the_frame() {
    let mut program = far_side_bar();
    program
        .set_role(
            "bar",
            Paint::local_block(
                "minecraft:iron_chain[axis=x,waterlogged=false]"
                    .parse::<BlockState>()
                    .unwrap(),
            ),
        )
        .expect("the role is bound");
    let out = expand(&program, BAR_ALONG_X, &ExpandOptions::seeded(3)).unwrap();
    assert!(
        out.model
            .palette()
            .iter()
            .any(|s| s.to_string() == "minecraft:iron_chain[axis=z,waterlogged=false]"),
        "the restyled material turns with the piece too: {:?}",
        out.model
            .palette()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
}
