//! The castle — corner towers, curtain walls, crenellations.
//!
//! Ported rule for rule from `MakeCastle.py` (`yawgmoth/GDMC25`, BSD-3-Clause;
//! author Markus Eger). It is the library's exercise of the parts the temple
//! does not use: parity-guarded alternatives (`crennels` alternates block and
//! gap only when the run is even, and shortens the run by one when it is not),
//! `repeat` splits, mid-derivation `reorient`, and a size-guarded plan that
//! falls back to smaller towers on a smaller footprint.
//!
//! Smallest region that expands: **both** horizontal extents ≥ `2*large_tower +
//! 2` (the `castle` rule turns the plan onto the box's long side, so the guard
//! on the side walls ends up applying to the short one), and Y ≥ `tower_height +
//! 1`. An undersized box is refused loudly — upstream silently emits nothing
//! there, because a rule with no applicable alternative just voids the scope.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, Program, Reorient};

use super::{
    abs, absp, alt_else, alt_when, call, cmp, dim, fill, int, modulo_is, par, rel, reoriented,
    split, split_oriented, split_repeat, void,
};

/// The castle program.
///
/// Parameters: `large_tower`, `small_tower` (tower footprints), `great_hall`
/// (the courtyard width the large plan needs), `wall_height`, `wall_width`,
/// `tower_height`. Palette roles: `stone`.
pub fn castle() -> Program {
    Program::new("castle", "castle")
        .param("large_tower", 9)
        .param("small_tower", 5)
        .param("great_hall", 13)
        .param("wall_height", 5)
        .param("wall_width", 3)
        .param("tower_height", 8)
        .role("stone", BlockState::simple("stone"))
        // --- plan -----------------------------------------------------------
        .rule(
            "castle",
            reoriented(
                Reorient::KEEP.x(AxisSpec::Largest).y(AxisSpec::LocalY),
                call("castle_layout"),
            ),
        )
        .rule_alts(
            "castle_layout",
            vec![
                alt_when(
                    // Room for two large towers, the hall, and the walls between.
                    cmp(
                        dim(DimRef::X),
                        CmpOp::Gt,
                        int(2)
                            .arith(ArithOp::Mul, par("large_tower"))
                            .arith(ArithOp::Add, par("great_hall"))
                            .arith(ArithOp::Add, int(4)),
                    ),
                    split(
                        Axis::X,
                        vec![absp("large_tower"), rel(1), absp("large_tower")],
                        vec![call("side_wall"), call("castle_center"), call("side_wallr")],
                    ),
                ),
                alt_else(split(
                    Axis::X,
                    vec![absp("small_tower"), rel(1), absp("small_tower")],
                    vec![call("side_wall"), call("castle_center"), call("side_wallr")],
                )),
            ],
        )
        .rule(
            "castle_center",
            split(
                Axis::Z,
                vec![absp("large_tower"), rel(1), absp("large_tower")],
                vec![
                    reoriented(Reorient::KEEP.x(AxisSpec::LocalZ), call("carve_wall")),
                    void(),
                    reoriented(Reorient::KEEP.x(AxisSpec::LocalZ), call("carve_wallr")),
                ],
            ),
        )
        // --- side walls, tower to tower ---------------------------------------
        .rule_alts(
            "side_wall",
            vec![alt_when(
                cmp(
                    dim(DimRef::Z),
                    CmpOp::Gt,
                    int(2)
                        .arith(ArithOp::Mul, par("large_tower"))
                        .arith(ArithOp::Add, int(1)),
                ),
                split(
                    Axis::Z,
                    vec![absp("large_tower"), rel(1), absp("large_tower")],
                    vec![call("large_tower"), call("carve_wall"), call("large_tower")],
                ),
            )],
        )
        .rule_alts(
            "side_wallr",
            vec![alt_when(
                cmp(
                    dim(DimRef::Z),
                    CmpOp::Gt,
                    int(2)
                        .arith(ArithOp::Mul, par("large_tower"))
                        .arith(ArithOp::Add, int(1)),
                ),
                split(
                    Axis::Z,
                    vec![absp("large_tower"), rel(1), absp("large_tower")],
                    vec![
                        call("large_tower"),
                        call("carve_wallr"),
                        call("large_tower"),
                    ],
                ),
            )],
        )
        .rule(
            "carve_wall",
            split(
                Axis::X,
                vec![rel(1), absp("wall_width")],
                vec![void(), call("wall")],
            ),
        )
        .rule(
            "carve_wallr",
            split(
                Axis::X,
                vec![absp("wall_width"), rel(1)],
                vec![call("wall"), void()],
            ),
        )
        .rule(
            "wall",
            split(
                Axis::Y,
                vec![absp("wall_height"), abs(1), rel(1)],
                vec![fill("stone"), call("wallcrennels"), void()],
            ),
        )
        // --- towers ------------------------------------------------------------
        .rule(
            "large_tower",
            split(
                Axis::Y,
                vec![absp("tower_height"), abs(1), rel(1)],
                vec![fill("stone"), call("tower_crennels"), void()],
            ),
        )
        .rule(
            "tower_crennels",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![call("crennels"), call("mid_crennels"), call("crennels")],
            ),
        )
        .rule(
            "mid_crennels",
            // The two long sides of the tower cap: turn the scope so the
            // crenellation rhythm runs along them too.
            split_oriented(
                Axis::Z,
                vec![abs(1), rel(1), abs(1)],
                Reorient::KEEP.z(AxisSpec::LocalX),
                vec![call("gapcrennels"), void(), call("gapcrennels")],
            ),
        )
        // --- crenellation ------------------------------------------------------
        .rule_alts(
            "crennels",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::Z), 2, 0),
                    split_repeat(Axis::Z, vec![abs(1), abs(1)], vec![fill("stone"), void()]),
                ),
                alt_else(split(
                    Axis::Z,
                    vec![rel(1), abs(1)],
                    vec![call("crennels"), fill("stone")],
                )),
            ],
        )
        .rule_alts(
            "gapcrennels",
            vec![
                alt_when(
                    modulo_is(dim(DimRef::Z), 2, 0),
                    split_repeat(Axis::Z, vec![abs(1), abs(1)], vec![void(), fill("stone")]),
                ),
                alt_else(split(
                    Axis::Z,
                    vec![rel(1), abs(1)],
                    vec![call("gapcrennels"), void()],
                )),
            ],
        )
        .rule_alts(
            "wallcrennels",
            vec![
                alt_when(cmp(dim(DimRef::X), CmpOp::Lt, int(3)), call("crennels")),
                alt_else(split(
                    Axis::X,
                    vec![abs(1), rel(1), abs(1)],
                    vec![call("crennels"), void(), call("crennels")],
                )),
            ],
        )
}
