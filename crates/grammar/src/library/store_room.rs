//! The container mix — a barrel line with exactly one wrong barrel in it (W2
//! entry C, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. DS1's
//! Depths hides giant rats inside the first breakable boxes past the stairs
//! (`docs/notes/souls-design-language.md` §4.1); the mimic is the one ambush in
//! the whole catalogue with a *genuine* tell, and the wiki's own advice is that
//! the reliable read is the one that needs no waiting — the chain, not the
//! breath (§2.3). This rule builds that: a tell you can see, from a distance, by
//! looking.
//!
//! ```text
//!  local X:   0     1 .......... X-3      X-2        X-1
//!            wall   open floor            barrels    wall
//!
//!  the line, along local Z:   [ barrel | barrel | TELL | barrel | ... ]
//!                                                        ^ Z-max = the end the
//!                                                          player walks in from
//!                                   travel: local Z-max -> Z-min
//! ```
//!
//! # One tell, and the rule cannot miscount
//!
//! A grammar rule has no memory, so "exactly one of these is different" cannot
//! be a counter — it has to be in the *shape of the derivation*. It is: the line
//! is walked by two rules whose names are the state. `line_before_tell` either
//! lays a plain barrel and recurses (still looking) or spends its draw here and
//! hands the rest to `line_after_tell`, which is a plain fill and can never
//! produce another. When the remaining run is one cell long neither guarded
//! alternative applies and the `otherwise` places the tell outright. So every
//! derivation places exactly one, wherever the seed put it, and no path can
//! place two or none.
//!
//! The draw is weighted 3:1 toward carrying on, which is what puts the tell a
//! few barrels in rather than always at the near end — far enough that the
//! player has already read "this is a row of barrels" before the odd one
//! arrives.
//!
//! # Two palette roles, on purpose
//!
//! `barrel` and `barrel_unbanded` are separate roles so a campaign restyles
//! *both* and the tell survives the restyle as a tell. The default binding keeps
//! them one material family and changes only the variant: a spruce barrel, and a
//! spruce log — the same wood with its iron bands missing. The dossier is blunt
//! that a souls ambush's fairness budget is spent on silhouette, not on sound
//! (§4.3), and a missing band is a silhouette.
//!
//! # Anchors
//!
//! * `anchor/store-line` — the barrel at the approach end of the row, facing
//!   down the row. Where a campaign hangs the line's narration or its first prop.
//! * `anchor/tell` — the odd barrel's own cell, facing out into the room across
//!   the line (derived through a `reorient` naming the across-room axis as local
//!   `Z`, which is why the row is against the `X`-max wall and not the other
//!   one). A container anchor names the container's block, not the air beside it.
//!
//! Smallest region that expands: X ≥ 5, Y ≥ 5, Z ≥ 3 — three barrels is the
//! shortest row in which the odd one always has a neighbour to be odd against.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{Alternative, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient, Side};

use super::{
    abs, all_of, alt_else, alt_when, call, cmp, dim, face, fill, int, marked, rel, reoriented,
    split, void,
};

/// Weight on carrying down the line rather than spending the tell here. The
/// tell's position is then geometric with p = 1/4, which lands it a few barrels
/// in without ever pinning it to one cell.
const CARRY_ON: u32 = 3;

/// The shortest row the rule will lay: with three barrels the odd one always has
/// a neighbour on at least one side, whichever cell the draw picks.
pub const MIN_LINE: i64 = 3;

/// The storeroom with its barrel line.
///
/// No size parameters: the row is as long as the box and the tell's position is
/// the seed's. Palette roles: `stone` (the room), `barrel`, `barrel_unbanded`.
pub fn store_room() -> Program {
    Program::new("store_room", "stores")
        .role("stone", BlockState::simple("stone_bricks"))
        .role("barrel", BlockState::with("barrel", [("facing", "up")]))
        // The same spruce, without the bands. A different block rather than a
        // different block *state* on purpose: `barrel[open=true]` is the obvious
        // mimic-breath pun and vanilla closes it again the moment the structure
        // loads, so the tell would last exactly as long as nobody looked.
        .role(
            "barrel_unbanded",
            BlockState::with("spruce_log", [("axis", "y")]),
        )
        // --- frame -----------------------------------------------------------
        .rule(
            "stores",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("store_plan"),
            ),
        )
        // Wall, floor, the barrel row against the far wall, wall. The row is at
        // `X`-max so that the `tell` anchor's derived facing — always the
        // negative direction of the axis a scope calls local `Z` — points into
        // the room rather than into the wall.
        .rule_alts(
            "store_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(5)),
                    cmp(dim(DimRef::Y), CmpOp::Ge, int(5)),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(MIN_LINE)),
                ]),
                split(
                    Axis::X,
                    vec![abs(1), rel(1), abs(1), abs(1)],
                    vec![
                        fill("stone"),
                        call("floor_column"),
                        call("store_lane"),
                        fill("stone"),
                    ],
                ),
            )],
        )
        .rule(
            "floor_column",
            split(
                Axis::Y,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        // Floor, the one course of barrels, headroom over them, ceiling.
        .rule(
            "store_lane",
            split(
                Axis::Y,
                vec![abs(1), abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("barrel_line"), void(), fill("stone")],
            ),
        )
        // --- the line ----------------------------------------------------------
        .rule(
            "barrel_line",
            marked(
                "store-line",
                face(Axis::Z, Side::Max),
                call("line_before_tell"),
            ),
        )
        // The state machine, spelled as two rule names. Both guarded
        // alternatives can hold at once, which here is exactly right: this is a
        // distribution over *where* the tell goes, not a priority order.
        .rule_alts(
            "line_before_tell",
            vec![
                Alternative::new(step(call("line_before_tell"), fill("barrel")))
                    .when(cmp(dim(DimRef::Z), CmpOp::Ge, int(2)))
                    .weight(CARRY_ON),
                Alternative::new(step(call("line_after_tell"), call("tell_cell")))
                    .when(cmp(dim(DimRef::Z), CmpOp::Ge, int(2)))
                    .weight(1),
                // One cell left and the tell still unspent: it goes here. This
                // is what makes "exactly one" a property of the grammar rather
                // than a hope about the draw.
                alt_else(call("tell_cell")),
            ],
        )
        .rule("line_after_tell", fill("barrel"))
        .rule(
            "tell_cell",
            reoriented(
                Reorient::KEEP.z(AxisSpec::LocalX),
                marked("tell", MarkAt::CornerMin, fill("barrel_unbanded")),
            ),
        )
}

/// One step down the line: everything deeper into the room, then the near cell.
///
/// The near cell is the high-`Z` piece because travel runs toward local `Z`-min
/// (the W1 frame), so recursing into the low-`Z` remainder walks the row in the
/// order the player meets it.
fn step(deeper: crate::ir::Node, near: crate::ir::Node) -> crate::ir::Node {
    split(Axis::Z, vec![rel(1), abs(1)], vec![deeper, near])
}
