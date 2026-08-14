//! The broken grate — a wall's own vent row, with exactly one cell wrong (W3
//! entry X, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. The same
//! move `store_room` makes for a container row, applied to a wall instead: a
//! secret cue has to be *findable by looking*, not by waiting
//! (`docs/notes/souls-design-language.md` §2.3), and a grate that has visibly
//! given way is a tell that costs nothing to notice and nothing to miss.
//!
//! ```text
//!  local X:   0     1 .......... X-2      X-1
//!            wall   open floor            grate wall (embeds the row)
//!
//!  the row, along local Z:  [ grate | grate | BROKEN | grate | ... ]
//!                                                       ^ Z-max = the end the
//!                                                         player walks in from
//!                                    travel: local Z-max -> Z-min
//! ```
//!
//! # The rule cannot miscount
//!
//! Exactly the state-machine `store_room` uses, applied to a wall band instead
//! of a floor row: `line_before_tell` either lays a plain grate and recurses
//! (still looking) or spends its draw here and hands the rest to
//! `line_after_tell`, a plain fill that can never produce another. With one
//! cell of row left and the draw still unspent, the `otherwise` places the
//! break outright — so every derivation breaks exactly one grate, wherever the
//! seed put it, and no path can break two or none.
//!
//! # The gate spec-0027 §4 exists to check
//!
//! `grate` and `grate_broken` must read as one material family at two
//! distress levels, the same claim `boulder_stair` makes for its tread —
//! proved the same way, with the same **test-local mirror** of the not-yet-
//! built §4 diagnostic (`docs/reference/grammar.md` §7), scoped to the grate
//! row's own cells so the room's incidental stone does not dilute a claim that
//! is specifically about the row.
//!
//! # Anchors
//!
//! * `anchor/grate-secret` — the broken cell's own position, facing out into
//!   the room across the row (derived through a `reorient` naming the
//!   across-room axis as local `Z`, which is why the row sits at `X`-max and
//!   not the other one — the same construction `store_room`'s tell uses).
//!
//! Smallest region that expands: X ≥ 3, Y ≥ `head` + 2, Z ≥ `MIN_LINE` — three
//! grates is the shortest row in which the odd one always has a neighbour to
//! be odd against, the same proof `store_room` makes for its barrels.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{Alternative, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, absp, all_of, alt_else, alt_when, call, cmp, dim, fill, fill_block, int, marked, oriented,
    par, rel, reoriented, split, void,
};

/// Weight on carrying down the row rather than spending the break here — the
/// same distribution `store_room` draws its tell's position from.
const CARRY_ON: u32 = 3;

/// The shortest row the rule will lay: with three grates the odd one always
/// has a neighbour on at least one side.
pub const MIN_LINE: i64 = 3;

/// The wall with its broken-grate row.
///
/// Parameters: `head` (room headroom), `grate_height` (how tall the row is;
/// must be ≤ `head`). Palette roles: `stone` (the room) and `grate_broken`
/// (the break). The plain bars are NOT a role: their connection properties
/// depend on the scope's orientation, so they are per-orientation guarded
/// inline states (see `grate_bars` below) rather than a role in the scope's own
/// axes; the guard is kept here because the corpus needs a demonstration of it.
pub fn broken_grate() -> Program {
    Program::new("broken_grate", "broken_grate")
        .param("head", 3)
        .param("grate_height", 2)
        .role("stone", BlockState::simple("cobblestone"))
        // The same family, distressed: mossy variant of the same bars, not a
        // different material.
        .role("grate_broken", BlockState::simple("mossy_cobblestone"))
        // --- frame -------------------------------------------------------
        .rule(
            "broken_grate",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("wall_plan"),
            ),
        )
        // One alternative, no `otherwise`: a box too narrow to hold wall,
        // floor and grate wall, or too short for a three-grate row, is a
        // refusal naming the rule, never a shorter row.
        .rule_alts(
            "wall_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(crate::ir::ArithOp::Add, int(2)),
                    ),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(MIN_LINE)),
                    cmp(par("grate_height"), CmpOp::Le, par("head")),
                ]),
                split(
                    Axis::X,
                    vec![abs(1), rel(1), abs(1)],
                    vec![fill("stone"), call("floor_column"), call("grate_wall")],
                ),
            )],
        )
        .rule(
            "floor_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        // --- the far wall, embedding the row -------------------------------
        .rule(
            "grate_wall",
            split(
                Axis::Y,
                vec![absp("grate_height"), rel(1)],
                vec![call("grate_row"), fill("stone")],
            ),
        )
        .rule("grate_row", call("line_before_tell"))
        // The state machine, spelled as two rule names, identical in shape to
        // `store_room`'s. Both guarded alternatives can hold at once, which
        // here is exactly right: this is a distribution over *where* the
        // break goes, not a priority order.
        .rule_alts(
            "line_before_tell",
            vec![
                Alternative::new(step(call("line_before_tell"), call("grate_bars")))
                    .when(cmp(dim(DimRef::Z), CmpOp::Ge, int(2)))
                    .weight(CARRY_ON),
                Alternative::new(step(call("line_after_tell"), call("broken_cell")))
                    .when(cmp(dim(DimRef::Z), CmpOp::Ge, int(2)))
                    .weight(1),
                // One cell left and the break still unspent: it goes here.
                // This is what makes "exactly one" a property of the grammar
                // rather than a hope about the draw.
                alt_else(call("broken_cell")),
            ],
        )
        .rule("line_after_tell", call("grate_bars"))
        // The bars themselves. `iron_bars` connects along the row — the local
        // `Z` — and a connection property names a WORLD direction, which a
        // reorientation does not rewrite (`crate::orient` permutes and reflects
        // geometry only). Two constructs answer that. A role written in the
        // scope's own axes says it in ONE binding, which is what `far_side_bar`
        // does; this piece keeps the other — one alternative per frame, each
        // guarded with the `orientation` cond and writing the connections that
        // match — because the guard is a live construct and the corpus needs a
        // program that demonstrates it. Either way, a bare `iron_bars` role was
        // a `DW0735` isolated-post defect for as long as this rule shipped one.
        // The root pins local `Y` to world `Y`, so these two orientations are
        // the only reachable ones; a third would refuse loudly.
        .rule_alts(
            "grate_bars",
            vec![
                alt_when(
                    oriented(Axis::X, Axis::Y, Axis::Z),
                    fill_block(BlockState::with(
                        "iron_bars",
                        [
                            ("east", "false"),
                            ("north", "true"),
                            ("south", "true"),
                            ("waterlogged", "false"),
                            ("west", "false"),
                        ],
                    )),
                ),
                alt_when(
                    oriented(Axis::Z, Axis::Y, Axis::X),
                    fill_block(BlockState::with(
                        "iron_bars",
                        [
                            ("east", "true"),
                            ("north", "false"),
                            ("south", "false"),
                            ("waterlogged", "false"),
                            ("west", "true"),
                        ],
                    )),
                ),
            ],
        )
        .rule(
            "broken_cell",
            reoriented(
                Reorient::KEEP.z(AxisSpec::LocalX),
                marked("grate-secret", MarkAt::CornerMin, fill("grate_broken")),
            ),
        )
}

/// One step down the row: everything deeper into the wall, then the near
/// cell. The near cell is the high-`Z` piece because travel runs toward local
/// `Z`-min, so recursing into the low-`Z` remainder walks the row in the order
/// the player meets it — the same reasoning `store_room`'s own `step` uses.
fn step(deeper: crate::ir::Node, near: crate::ir::Node) -> crate::ir::Node {
    split(Axis::Z, vec![rel(1), abs(1)], vec![deeper, near])
}
