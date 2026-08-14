//! The far-side bar — a doorway that stays shut until someone reaches it from
//! the other side (W4 entry F, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. The
//! grammar half of a souls shortcut (spec-0016 §2): the short route between two
//! rest points starts sealed, the far side earns it, and pulling the mechanism
//! opens it forever. This rule builds the sealed half — a wall, one doorway,
//! and a physical bar filling it — plus the far-side cell a campaign's
//! `shortcut.unlock` binds to.
//!
//! ```text
//!  local Z:   0 .......... far room ...... wall  near room .......... Z-1
//!                                           ^bar    ^ travel starts here
//!                                    fills the door opening
//!                                   travel: local Z-max -> Z-min
//! ```
//!
//! `ambush_door` is the closest relative — same wall-across-the-box shape, same
//! one opening — but there the opening is open. Here it is filled with bars
//! (`bar_cell`): not a narrower door, a **barred** one. Unbarring it
//! (the `unbarred` test knob) turns the fill back to air and nothing else
//! changes, which is what proves the bar — and not some second gap in the
//! wall — is what was blocking the room.
//!
//! # The gate
//!
//! The near side cannot reach `anchor/unlock` while the bar stands: near-room
//! and far-room standable cells are simply **not connected** at all (the one
//! opening between them is solid), proved the same way `cliff_path` and
//! `ambush_door` prove their claims — graph connectivity over standable cells.
//! Teeth: `unbarred = 1` swaps the fill for void, and the same check must find
//! the two rooms connected through exactly that doorway — demonstrating both
//! that the wall has no other gap, and that the bar is what was sealing it.
//!
//! # Anchors
//!
//! * `anchor/gate` — the barred opening's own cell (its floor, if `door_height`
//!   spans more than one). A point, not a region: region anchors (the shape a
//!   `close-gate` / `shortcut` fill `block` actually needs) are not yet
//!   expressible by a rule (`docs/reference/grammar.md` §7), and this rule does
//!   not invent one — the same limitation `watch_bay`'s `anchor/gate` already
//!   accepted.
//! * `anchor/unlock` — the far room's floor centre, where a campaign's
//!   `shortcut.unlock` binds the interaction that opens the bar for good.
//!
//! Smallest region that expands: **3 wide, `head + 2` tall, 3 long** (one cell
//! of floor on each side of the wall) — and at least as long as it is wide.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, absp, all_of, alt_when, call, cmp, dim, fill, fill_block, int, marked, oriented, par, rel,
    reoriented, split, split_exact, void,
};

/// The far-side bar.
///
/// Parameters: `head` (interior headroom, both rooms), `door_height` (how tall
/// the barred opening is, at most `head`), `unbarred` — a test knob, off by
/// default, that swaps the bar for air so the no-route gate can be shown to
/// fail when it should. Palette roles: `rock` (the shell). The bars that
/// seal the doorway are not a role: their connections depend on the scope's
/// orientation, so they are per-orientation guarded inline states
/// (`bar_cell` below).
pub fn far_side_bar() -> Program {
    Program::new("far_side_bar", "threshold")
        .param("head", 3)
        .param("door_height", 2)
        .param("unbarred", 0)
        .role("rock", BlockState::simple("stone_bricks"))
        // --- frame -----------------------------------------------------------
        .rule(
            "threshold",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("threshold_plan"),
            ),
        )
        // One alternative, no `otherwise`: a box too small for a room on each
        // side of a real wall is not a smaller shortcut, it is not one at all.
        .rule_alts(
            "threshold_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(3)),
                    cmp(par("door_height"), CmpOp::Ge, int(1)),
                    cmp(par("head"), CmpOp::Ge, par("door_height")),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                ]),
                split_exact(
                    Axis::Z,
                    vec![rel(1), abs(1), rel(1)],
                    vec![call("far_room"), call("wall"), call("near_room")],
                ),
            )],
        )
        // --- the two rooms -------------------------------------------------------
        .rule(
            "far_room",
            plain_room(marked("unlock", MarkAt::FloorCenter, void())),
        )
        .rule("near_room", plain_room(void()))
        // --- the wall and its bar -------------------------------------------------
        .rule(
            "wall",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("rock"), call("door_column"), fill("rock")],
            ),
        )
        .rule(
            "door_column",
            split(
                Axis::Y,
                vec![abs(1), absp("door_height"), rel(1)],
                vec![
                    fill("rock"),
                    marked("gate", MarkAt::FloorCenter, call("bar_or_open")),
                    fill("rock"),
                ],
            ),
        )
        .rule_alts(
            "bar_or_open",
            vec![
                alt_when(cmp(par("unbarred"), CmpOp::Le, int(0)), call("bar_cell")),
                alt_when(cmp(par("unbarred"), CmpOp::Ge, int(1)), void()),
            ],
        )
        // The bars that seal the doorway. They span the opening along the
        // wall's own axis — the local X — and an `iron_bars` connection
        // property names a WORLD direction that a reorientation does not
        // rewrite, so the bars cannot be one palette role: one alternative per
        // orientation, guarded with the `orientation` cond (the `DW0736`
        // mechanism). The root pins local Y to world Y, so these two are the
        // only reachable orientations. A bare `iron_bars` role shipped here
        // for as long as this rule existed, and every doorway it sealed read
        // as a line of isolated posts (`DW0735`).
        .rule_alts(
            "bar_cell",
            vec![
                alt_when(
                    oriented(Axis::X, Axis::Y, Axis::Z),
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
                alt_when(
                    oriented(Axis::Z, Axis::Y, Axis::X),
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
            ],
        )
}

/// A plain floor-to-ceiling room, walled at both ends of the width, with
/// `interior` expanded into the standable band. Shared by both rooms because
/// they are identical shells — only what a campaign hangs inside differs.
fn plain_room(interior: crate::ir::Node) -> crate::ir::Node {
    split(
        Axis::X,
        vec![abs(1), rel(1), abs(1)],
        vec![
            fill("rock"),
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("rock"), interior, fill("rock")],
            ),
            fill("rock"),
        ],
    )
}
