//! The threshold motif — a boss door hung with a bell-rope curtain (W3 entry
//! M, drowned-bell remake step 2).
//!
//! **Original Delvewright content**, not a port: licence `original`. A motif
//! is taught once, legibly, and then reused elsewhere without retraining the
//! player — so unlike the ambush vocabulary, this rule's whole job is to look
//! the *same* at whatever size a zone needs it, not to hide or to surprise.
//!
//! ```text
//!  local Z:   0 .......... inside ...... doorband ...... approach ...... Z-1
//!  local X:   0    1..X-2       X-1               travel: Z-max -> Z-min
//!            wall  interior     wall
//!
//!  doorband, local Y (bottom to top):
//!    floor (anchor/threshold-narrate)
//!    walk clearance (open)
//!    curtain band — strands tiled every `strand_period` cells across X
//!    ceiling
//! ```
//!
//! # Reusable at different box sizes without the motif degrading
//!
//! The doorway spans the box's **whole** interior width, so the curtain band
//! is a `split_repeat` over whatever width the box gives it — the same
//! technique `rafter_hall`'s truss uses for its beam period. A rule taught in
//! one zone and rebuilt wider for another therefore keeps the same strand
//! *density* automatically; nothing about the motif has to be retuned per
//! size. `tests/staging.rs` proves the density holds across two box widths,
//! and shows the claim has teeth with `single_strand`, a knob that forces
//! exactly one strand regardless of width — the one way to make the motif
//! degrade, kept expressible so the density gate can be watched catching it.
//!
//! The curtain hangs in the band **above** walking height and never intrudes
//! on it, so it restyles or re-sizes without ever touching whether the door is
//! passable — a `curtain` block state is free to be solid.
//!
//! # Anchors
//!
//! * `anchor/threshold-narrate` — the doorband's floor centre (`FloorCenter`,
//!   so it re-centres itself at any width), for the campaign to hang the beat
//!   that is taught in one zone and cued again in the others.
//!
//! Smallest region that expands: X ≥ 3, Y ≥ `head` + 2 (and `head` ≥
//! `curtain_height` + 2, so there are two full cells of walk clearance under
//! the curtain — one is not enough for `standable`, whose own definition asks
//! for the cell above the player's head too), Z ≥ 3 — and at least as long as
//! it is wide (the frame turns length onto the longer horizontal axis, so
//! this holds by construction).

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, par, rel, reoriented,
    split, split_exact, split_repeat, void,
};

/// The shortest box the rule will lay a threshold in.
pub const MIN_DEPTH: i64 = 3;

/// The boss-door threshold motif.
///
/// Parameters: `head` (room headroom), `curtain_height` (how tall the hanging
/// band is; must be under `head`), `strand_period` (cells between rope
/// strands — 1 is a dense curtain), and `single_strand` — a test knob, off by
/// default, that collapses the curtain to one strand regardless of width so
/// the density gate can be shown to fail when it should. Palette roles:
/// `stone`, `curtain`.
pub fn threshold_motif() -> Program {
    Program::new("threshold_motif", "threshold_motif")
        .param("head", 4)
        .param("curtain_height", 2)
        .param("strand_period", 1)
        .param("single_strand", 0)
        .role("stone", BlockState::simple("stone_bricks"))
        .role("curtain", BlockState::simple("chain"))
        // --- frame -------------------------------------------------------
        .rule(
            "threshold_motif",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("threshold_plan"),
            ),
        )
        .rule_alts(
            "threshold_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(3)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(MIN_DEPTH)),
                    // Two full cells of clearance under the curtain, or a
                    // player standing there fails the `standable` test
                    // itself: the block directly over their head would be
                    // the curtain band, not open air.
                    cmp(
                        par("head"),
                        CmpOp::Ge,
                        par("curtain_height").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(par("strand_period"), CmpOp::Ge, int(1)),
                ]),
                split_exact(
                    Axis::Z,
                    vec![rel(1), abs(1), rel(1)],
                    vec![call("room"), call("doorband"), call("room")],
                ),
            )],
        )
        // --- a plain room, both sides of the threshold ----------------------
        .rule(
            "room",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("room_column"), fill("stone")],
            ),
        )
        .rule(
            "room_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("stone"), void(), fill("stone")],
            ),
        )
        // --- the doorband: full-width opening, curtain hung above the walk -
        .rule(
            "doorband",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("stone"), call("door_column"), fill("stone")],
            ),
        )
        .rule(
            "door_column",
            split(
                Axis::Y,
                vec![abs(1), walk_clear(), absp("curtain_height"), rel(1)],
                vec![
                    marked("threshold-narrate", MarkAt::FloorCenter, fill("stone")),
                    void(),
                    call("curtain_row"),
                    fill("stone"),
                ],
            ),
        )
        .rule_alts(
            "curtain_row",
            vec![
                alt_when(
                    cmp(par("single_strand"), CmpOp::Le, int(0)),
                    split_repeat(Axis::X, vec![abs(1), gap()], vec![fill("curtain"), void()]),
                ),
                alt_when(
                    cmp(par("single_strand"), CmpOp::Ge, int(1)),
                    split(
                        Axis::X,
                        vec![rel(1), abs(1), rel(1)],
                        vec![void(), fill("curtain"), void()],
                    ),
                ),
            ],
        )
}

/// The room's walking clearance under the curtain: headroom less the curtain
/// band's own height.
fn walk_clear() -> crate::ir::Size {
    abse(par("head").arith(ArithOp::Sub, par("curtain_height")))
}

/// The gap between rope strands: the period less the strand itself.
fn gap() -> crate::ir::Size {
    abse(par("strand_period").arith(ArithOp::Sub, int(1)))
}
