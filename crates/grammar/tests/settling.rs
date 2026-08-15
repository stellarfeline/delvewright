//! **The two gates that judge a piece as the world will hold it, not as it was
//! written** — `stair-shape` (`DW0801`) and `fluid-contained` (`DW0800`).
//!
//! Both rules are measured against the pinned server rather than reasoned out
//! (`tools/spike-block-settling/`, replayed cell for cell by
//! `crates/schem/tests/stair_shape_measured.rs`). What this file covers is the
//! other half: that the gates BIND — that they examine the objects a piece
//! actually contains, red in the direction that ships, and green one course
//! away.
//!
//! # The real artifacts these fixtures stand in for
//!
//! Both rules were owed by a zone round that found them by hand, and both have
//! live instances in campaign content rather than here:
//!
//! - the bell campaign's Z2 gate ward carries a kerbed water channel — 36
//!   sources, 25 stairs, both gates green at its own region and seed — and its
//!   mitred kerbs are the reason the stair rule exists: pointed across the run
//!   instead of along it, the same solid survives every render and flattens to
//!   `straight` in the world;
//! - `island-beach-camp` and `island-galley` are shoreline pieces whose water
//!   is the sea (1200 and 672 sources), green with their at-the-face counts
//!   stated;
//! - `cave-shore` is red on seven cells where the tideline runs into beach air.
//!
//! Campaign content lives in the other repository, so the fixtures below are
//! the same geometry at test scale; the campaign-side proofs are run against
//! those pieces directly.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::gates;
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{Material, Node, Program, Reorient, Rounding, Size, Split};
use delvewright_grammar::{Box3, ExpandOptions, Expansion, expand};

fn fill(state: &str) -> Node {
    Node::Fill {
        material: Material::block(state.parse::<BlockState>().unwrap()),
    }
}

fn split(axis: Axis, sizes: Vec<i64>, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes: sizes.into_iter().map(Size::abs).collect(),
        rounding: Rounding::Truncate,
        repeat: false,
        orient: Reorient::KEEP,
        children,
    })
}

fn judge(program: &Program, region: Box3) -> gates::Report {
    let out: Expansion =
        expand(program, region, &ExpandOptions::seeded(1)).expect("the fixture expands");
    gates::judge(&out, gates::Options::default())
}

fn gate<'r>(report: &'r gates::Report, id: &str) -> &'r gates::Gate {
    report
        .gates
        .iter()
        .find(|g| g.id == id)
        .unwrap_or_else(|| panic!("no `{id}` gate in {:?}", report.gates))
}

// ---------------------------------------------------------------------------
// A basin, and the same basin one course short
// ---------------------------------------------------------------------------

const BASIN: Box3 = Box3::at_origin([5, 3, 5]);

/// A 3x1x3 pool of sources in a stone box. `west_wall` is the one course this
/// pair differs by: with it the body is walled, without it the body runs.
fn basin(west_wall: bool) -> Program {
    let pool = split(
        Axis::Z,
        vec![1, 3, 1],
        vec![
            fill("minecraft:stone"),
            fill("minecraft:water[level=0]"),
            fill("minecraft:stone"),
        ],
    );
    let middle = split(
        Axis::X,
        vec![1, 3, 1],
        vec![
            if west_wall {
                fill("minecraft:stone")
            } else {
                fill("minecraft:air")
            },
            pool,
            fill("minecraft:stone"),
        ],
    );
    Program::new("basin", "piece").rule(
        "piece",
        split(
            Axis::Y,
            vec![1, 1, 1],
            vec![fill("minecraft:stone"), middle, fill("minecraft:stone")],
        ),
    )
}

#[test]
fn a_walled_basin_holds_its_water_and_says_how_many_cells_it_examined() {
    let report = judge(&basin(true), BASIN);
    let g = gate(&report, "fluid-contained");
    assert!(g.pass, "{}", g.detail);
    assert_eq!(g.bound, 9, "3x1x3 sources: {}", g.detail);
}

#[test]
fn one_missing_wall_course_reds_the_basin_and_names_the_cells() {
    let report = judge(&basin(false), BASIN);
    let g = gate(&report, "fluid-contained");
    assert!(
        !g.pass,
        "a basin with an open west wall must red: {}",
        g.detail
    );
    assert!(g.detail.contains("DW0800"), "{}", g.detail);
    assert_eq!(g.bound, 9, "the same nine cells are examined: {}", g.detail);
    assert!(
        g.detail.contains("runs into"),
        "the finding must name the cell the body runs into: {}",
        g.detail
    );
    assert!(!report.is_pass(), "a red gate must fail the report");
}

/// A body written mid-flow. `level` is a value vanilla derives and re-derives,
/// so a piece cannot pin one; this is the other half of the same rule.
#[test]
fn water_written_mid_flow_is_not_a_body() {
    let flowing = Program::new("flowing", "piece").rule(
        "piece",
        split(
            Axis::Y,
            vec![1, 1, 1],
            vec![
                fill("minecraft:stone"),
                split(
                    Axis::X,
                    vec![1, 3, 1],
                    vec![
                        fill("minecraft:stone"),
                        split(
                            Axis::Z,
                            vec![1, 3, 1],
                            vec![
                                fill("minecraft:stone"),
                                fill("minecraft:water[level=3]"),
                                fill("minecraft:stone"),
                            ],
                        ),
                        fill("minecraft:stone"),
                    ],
                ),
                fill("minecraft:stone"),
            ],
        ),
    );
    let report = judge(&flowing, BASIN);
    let g = gate(&report, "fluid-contained");
    assert!(!g.pass, "{}", g.detail);
    assert!(g.detail.contains("DW0800"), "{}", g.detail);
    assert!(g.detail.contains("mid-flow"), "{}", g.detail);
}

/// A block written `waterlogged=true` is wet and — measured on the pinned
/// server — spreads nothing. It owes no containment, and it is counted so that
/// "no fluid here" and "still fluid here" are different lines in the report.
#[test]
fn a_waterlogged_block_beside_open_air_is_counted_and_not_red() {
    let program = Program::new("wet_kerb", "piece").rule(
        "piece",
        split(
            Axis::X,
            vec![1, 4],
            vec![
                fill(
                    "minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=true]",
                ),
                fill("minecraft:air"),
            ],
        ),
    );
    let report = judge(&program, BASIN);
    assert!(
        !report.gates.iter().any(|g| g.id == "fluid-contained"),
        "a still cell is not a body that runs, so there is no verdict to claim: {:?}",
        report.gates
    );
    assert_eq!(report.measurements.fluid_cells, 0);
    assert!(
        report.measurements.fluid_held_cells > 0,
        "the still cells must be counted: {:?}",
        report.measurements
    );
    assert!(report.is_pass());
}

// ---------------------------------------------------------------------------
// A mitred kerb, and the same kerb with its corner pointed the other way
// ---------------------------------------------------------------------------

const KERB: Box3 = Box3::at_origin([3, 1, 3]);

/// Two stairs in a line along Z. The cell at `z=1` claims an outer corner; the
/// cell in front of it (north, `z=0`) is the one the claim depends on. Turned
/// `across` the run — perpendicular — the corner is what vanilla derives;
/// turned along it, vanilla derives `straight` and the piece is claiming a
/// shape the world will take away.
fn kerb(across: bool) -> Program {
    let front = if across {
        "minecraft:stone_brick_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]"
    } else {
        "minecraft:stone_brick_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]"
    };
    let corner =
        "minecraft:stone_brick_stairs[facing=north,half=bottom,shape=outer_left,waterlogged=false]";
    Program::new("kerb", "piece").rule(
        "piece",
        split(
            Axis::X,
            vec![1, 1, 1],
            vec![
                fill("minecraft:air"),
                split(
                    Axis::Z,
                    vec![1, 1, 1],
                    vec![fill(front), fill(corner), fill("minecraft:air")],
                ),
                fill("minecraft:air"),
            ],
        ),
    )
}

#[test]
fn a_corner_the_neighbours_derive_is_green_and_binds_to_the_stairs() {
    let report = judge(&kerb(true), KERB);
    let g = gate(&report, "stair-shape");
    assert!(g.pass, "{}", g.detail);
    assert_eq!(g.bound, 2, "both stairs are examined: {}", g.detail);
}

#[test]
fn the_same_corner_pointed_along_the_run_reds_and_names_both_shapes() {
    let report = judge(&kerb(false), KERB);
    let g = gate(&report, "stair-shape");
    assert!(!g.pass, "{}", g.detail);
    assert!(g.detail.contains("DW0801"), "{}", g.detail);
    assert_eq!(g.bound, 2, "the same two stairs are examined: {}", g.detail);
    assert!(
        g.detail.contains("shape=outer_left") && g.detail.contains("shape=straight"),
        "the finding must name what was written and what the game derives: {}",
        g.detail
    );
    assert!(!report.is_pass(), "a red gate must fail the report");
}

// ---------------------------------------------------------------------------
// The vacuity rule, asserted rather than trusted
// ---------------------------------------------------------------------------

/// **A gate that examined nothing is not a pass, so neither gate is emitted
/// over a piece that holds nothing for it to judge.** Most buildings hold no
/// water; a green `fluid-contained` line on every one of them would be a
/// verdict about nothing, which the next reader takes for a proof. The counts
/// are carried as measurements instead, where a zero is a fact and not a pass.
#[test]
fn a_piece_with_no_stairs_and_no_fluid_claims_no_verdict_over_either() {
    let plain = Program::new("plain", "piece").rule("piece", fill("minecraft:stone"));
    let report = judge(&plain, BASIN);
    for id in ["stair-shape", "fluid-contained"] {
        assert!(
            !report.gates.iter().any(|g| g.id == id),
            "`{id}` claimed a verdict over nothing: {:?}",
            report.gates
        );
    }
    assert_eq!(report.measurements.stairs, 0);
    assert_eq!(report.measurements.fluid_cells, 0);
    assert!(report.is_pass());
}
