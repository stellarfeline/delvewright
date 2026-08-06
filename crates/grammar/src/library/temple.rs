//! The Greek temple — the Parthenon-class typology of the Box-Split Grammars
//! paper.
//!
//! Ported from `MakeTemple.py` and `Tetrastyle.py` (`yawgmoth/GDMC25`,
//! BSD-3-Clause; author Markus Eger). Upstream's `temple`, `temple1`, `temple2`
//! and `temple3` are four near-identical top rules that differ only in what
//! caps the colonnade; here they are four guarded alternatives of one `temple`
//! rule selected by the `roof` parameter — the same four buildings, now a knob.
//!
//! One rule changed shape: upstream's `columns` splits into a hard-coded nine
//! pieces, i.e. exactly four columns, so the peristyle does not follow the
//! building. Ours is the same alternating gap/column pattern under `repeat`, so
//! column *count* follows the depth of the box and `column_size` sets their
//! thickness. Four columns across a nine-deep box reproduces upstream exactly.
//!
//! Smallest region that expands: X ≥ `6 + 2*column_size`, Y ≥ `1 +
//! column_height + roof height` (5 pitched, 3 flat, 1 capped, 0 open), Z ≥ 7.
//! Below that the rule's absolute sizes do not fit and expansion fails loudly;
//! `tests/library.rs` holds those numbers to the code.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{CmpOp, Program};

use super::{abs, absp, alt_when, call, cmp, fill, int, par, rel, split, split_repeat, void};

/// Roof kinds, the `roof` parameter's values.
pub mod roof {
    /// The stepped pitched roof of `MakeTemple.temple` (5 blocks tall).
    pub const PITCHED: i64 = 0;
    /// The stepped flat roof of `MakeTemple.temple3` (3 blocks tall).
    pub const FLAT: i64 = 1;
    /// A single capping course, `MakeTemple.temple1`.
    pub const CAPPED: i64 = 2;
    /// No roof at all — a ruin, `MakeTemple.temple2`.
    pub const OPEN: i64 = 3;
}

/// The temple program.
///
/// Parameters: `column_size` (column thickness), `column_height` (the order's
/// height), `roof` (see [`roof`]). Palette roles: `marble` (everything the
/// temple is built of).
pub fn temple() -> Program {
    Program::new("temple", "temple")
        .param("column_size", 1)
        .param("column_height", 8)
        .param("roof", roof::PITCHED)
        .role("marble", BlockState::simple("quartz_block"))
        // --- the four buildings -------------------------------------------
        .rule_alts(
            "temple",
            vec![
                alt_when(
                    cmp(par("roof"), CmpOp::Eq, int(roof::PITCHED)),
                    split(
                        Axis::Y,
                        vec![abs(1), absp("column_height"), abs(5)],
                        vec![fill("marble"), call("floorplan"), call("temple_roof")],
                    ),
                ),
                alt_when(
                    cmp(par("roof"), CmpOp::Eq, int(roof::FLAT)),
                    split(
                        Axis::Y,
                        vec![abs(1), absp("column_height"), abs(3)],
                        vec![fill("marble"), call("floorplan"), call("temple_roof_flat")],
                    ),
                ),
                alt_when(
                    cmp(par("roof"), CmpOp::Eq, int(roof::CAPPED)),
                    split(
                        Axis::Y,
                        vec![abs(1), absp("column_height"), abs(1), rel(1)],
                        vec![fill("marble"), call("floorplan"), fill("marble"), void()],
                    ),
                ),
                alt_when(
                    cmp(par("roof"), CmpOp::Eq, int(roof::OPEN)),
                    split(
                        Axis::Y,
                        vec![abs(1), absp("column_height"), rel(1)],
                        vec![fill("marble"), call("floorplan"), void()],
                    ),
                ),
            ],
        )
        // --- floor plan ----------------------------------------------------
        .rule(
            "floorplan",
            split(
                Axis::X,
                vec![
                    abs(1),
                    absp("column_size"),
                    abs(1),
                    rel(1),
                    abs(1),
                    absp("column_size"),
                    abs(1),
                ],
                vec![
                    void(),
                    call("columns"),
                    void(),
                    call("naos"),
                    void(),
                    call("columns"),
                    void(),
                ],
            ),
        )
        .rule(
            "naos",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![call("columns"), call("chamber"), call("back_wall")],
            ),
        )
        .rule(
            "chamber",
            split(
                Axis::Z,
                vec![abs(1), abs(1), rel(1), abs(1), abs(1)],
                vec![void(), fill("marble"), void(), fill("marble"), void()],
            ),
        )
        .rule(
            "back_wall",
            split(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                vec![void(), fill("marble"), void()],
            ),
        )
        // A colonnade of any depth: gap, column, gap, column, ... clamped at the
        // far end. Upstream fixes this at four columns.
        .rule(
            "columns",
            split_repeat(
                Axis::Z,
                vec![abs(1), absp("column_size")],
                vec![void(), fill("marble")],
            ),
        )
        // --- pitched roof: four stepped courses, then a ridge ---------------
        .rule(
            "temple_roof",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![fill("marble"), call("level2roof")],
            ),
        )
        .rule(
            "level2roof",
            split(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                vec![void(), call("temple_roof2"), void()],
            ),
        )
        .rule(
            "temple_roof2",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![fill("marble"), call("level3roof")],
            ),
        )
        .rule(
            "level3roof",
            split(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                vec![void(), call("temple_roof3"), void()],
            ),
        )
        .rule(
            "temple_roof3",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![fill("marble"), call("level4roof")],
            ),
        )
        .rule(
            "level4roof",
            split(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                vec![void(), call("temple_roof4"), void()],
            ),
        )
        .rule(
            "temple_roof4",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![fill("marble"), call("level5roof")],
            ),
        )
        .rule(
            "level5roof",
            split(
                Axis::Z,
                vec![rel(1), abs(1), rel(1)],
                vec![void(), fill("marble"), void()],
            ),
        )
        // --- flat roof: two courses, stepped in by two -----------------------
        .rule(
            "temple_roof_flat",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![fill("marble"), call("level2roof_flat")],
            ),
        )
        .rule(
            "level2roof_flat",
            split(
                Axis::Z,
                vec![abs(2), rel(1), abs(2)],
                vec![void(), call("temple_roof2_flat"), void()],
            ),
        )
        .rule(
            "temple_roof2_flat",
            split(
                Axis::Y,
                vec![abs(1), rel(1)],
                vec![fill("marble"), call("level3roof_flat")],
            ),
        )
        .rule(
            "level3roof_flat",
            split(
                Axis::Z,
                vec![abs(2), rel(1), abs(2)],
                vec![void(), fill("marble"), void()],
            ),
        )
}
