//! The worn-tread tell — a hazard lane whose own floor tells you where the
//! danger runs (W3 entry W, drowned-bell remake step 2). Its side pockets are
//! entry S: see the module note below on why they land here rather than as a
//! second exported program.
//!
//! **Original Delvewright content**, not a port: licence `original`. The
//! reference is Sen's Fortress ("Sen's palette telegraph" — the vocabulary
//! doc's own name for this entry), whose rolling boulder needs no dialogue to
//! say which lane is dangerous: centuries of feet and stone have already worn
//! a smoother track down the middle of the stair than the sides. This rule
//! builds that tell as **paint, not shape** — the lane the boulder runs and
//! the lane beside it are the same floor, one course, differing only in which
//! material variant they take.
//!
//! ```text
//!  local X:  0..a      a..a+1        a+1..b       b..b+1   b+1..b+2
//!           far lane   RUN (smooth)  near lane    pocket   backing
//!           (rough)     1 wide       (rough)      (S)      (always solid)
//!
//!  along local Z:  |pocket-slot(1)|plain(pocket_period-1)|pocket-slot(1)|...
//!                                                     travel: Z-max -> Z-min
//! ```
//!
//! # Why this is a palette rule, not a geometry rule
//!
//! `docs/reference/grammar.md` §7 already files the gap this vocabulary keeps
//! meeting: a rule cannot ask a repeated slice to climb a block per iteration
//! — there is no index the IR exposes to a `Size` or a `Cond` — so a *true*
//! rising staircase is not composable from today's verbs without inventing a
//! primitive. The vocabulary doc does not actually ask for that: it asks for a
//! floor whose **material** changes down the hazard lane, which is squarely a
//! `fill` decision. "Stair" names the encounter — a rolling boulder down a Sen's
//! Fortress typology — not a Y-changing shape this rule has to build. A flat
//! lane, painted, is therefore what the doc describes, and it keeps the rule
//! inside composition-only.
//!
//! # The gate spec-0027 §4 exists to check
//!
//! The smooth centre lane and the rough side lane must read as **one material
//! family at two distress levels**, not as an accent colour: `tests/staging.rs`
//! proves it with a **mirror** of the not-yet-built §4 diagnostic
//! (`docs/reference/grammar.md` §7 / `crates/grammar/src/lib.rs`: the craft
//! diagnostics are a later phase of spec-0027) — the same move `watch_bay`'s
//! sightline gate already makes against `DW0388`: a test-local
//! reimplementation of the *described* rule (60/30/10, accent < 10%, grouped by
//! family), scoped to the lane's own floor course so incidental wall stone
//! elsewhere in the model does not dilute a claim that is specifically about
//! the tread. Nothing here mints a diagnostic code; that stays planner-owned.
//!
//! # Anchors
//!
//! * `anchor/stair-run` — the run's floor centre (`FloorCenter`, so it holds at
//!   any lane width), for the campaign to bind the boulder's path to.
//! * `anchor/volley-slot` — the vault rib directly over the run's midpoint, for
//!   a dart/arrow trap. Not itself a trap declaration
//!   (`docs/reference/grammar.md` §7: trigger/dispenser anchors are not yet
//!   expressible by a rule) — a point the campaign spends on one; the cell it
//!   names is ordinary vault stone until something binds it.
//! * `anchor/pocket-<i>` — see S, below.
//!
//! # S — the side pockets, and why they are not a second program
//!
//! `docs/notes/private/grammar-staging-vocabulary.md` gives "S: safe pockets"
//! no grammar of its own anywhere in its catalogue — the only place the
//! pockets are described at all is inside W's own entry, as "side `alcove`
//! splits every 8 units (entry S safe pockets)". Read plainly, S *is* the name
//! for a sub-feature of this rule's own split, not a box grammar with its own
//! minimum region the way K/R/O/W each get; the dispatch line's "W3 = W+S+M+X"
//! is the only place that treats S as a fourth peer letter, and it is silent
//! on shape.
//!
//! **This is filed as an open question for the planner, not decided here.**
//! What is built: `pocket_niche`, a properly named, fully gated rule inside
//! this program (the same factoring `ambush_door`'s `alcove_row` uses for its
//! own pocket) — real fixture, indexed anchors, a shown-red control, all of
//! it — but *inside* `boulder_stair`, not as a second exported [`Program`]
//! (the IR has no cross-program `call`, so a standalone `safe_pocket` program
//! could not literally be what this rule's own split uses; it would have to be
//! a second, independently-composed program sharing only Rust-level helper
//! functions). If that is the honest factoring the planner wants, this file
//! does not attempt to guess its shape.
//!
//! Gate: a pocket is a knee-deep dodge off the rough lane, not a room — one
//! cell deep, standable, with a solid lintel and a solid backing wall behind
//! it so the notch does not open onto the model's own edge. Unlike
//! `ambush_door`'s blind alcove, a pocket is meant to be seen: a dodge you
//! cannot spot coming is not an escape, so there is no blindness proof here.
//!
//! Smallest region that expands: **`MIN_X` × (`head` + 2) × `MIN_DEPTH`** — see
//! the constants below; `MIN_X` is `RUN_WIDTH` plus the two 1-cell lane
//! minimums plus the two pocket-band cells, and at least as long as it is wide
//! (the frame turns length onto the longer horizontal axis, so this holds by
//! construction, not by an extra guard).

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, marked_each, par, rel,
    reoriented, split, split_exact, split_repeat, void,
};

/// Cells the smooth run is wide.
const RUN_WIDTH: i64 = 1;

/// The smallest interior width: one cell of rough lane on each side of the
/// run, plus the pocket cell and its always-solid backing.
pub const MIN_X: i64 = RUN_WIDTH + 4;

/// The shortest box the rule will lay a lane in. The frame turns length onto
/// the *longer* horizontal axis (§7's `Largest`), so a box's depth can never
/// end up smaller than its width after reorientation — pinning this to
/// anything under `MIN_X` would describe a minimum the frame can never reach.
pub const MIN_DEPTH: i64 = MIN_X;

/// The worn-tread hazard lane, with its side pockets.
///
/// Parameters: `head` (lane headroom, and the height the vault rib sits at),
/// `pocket_height` (how tall a pocket is; must be ≤ `head`), `pocket_period`
/// (cells between pockets; the entry asks for 8). Palette roles: `rough` (the
/// side lane and every wall), `smooth` (the worn centre — same family,
/// different distress level).
pub fn boulder_stair() -> Program {
    Program::new("boulder_stair", "boulder_stair")
        .param("head", 4)
        .param("pocket_height", 2)
        .param("pocket_period", 8)
        .role("rough", BlockState::simple("cobblestone"))
        .role("smooth", BlockState::simple("stone"))
        // --- frame -------------------------------------------------------
        .rule(
            "boulder_stair",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("stair_plan"),
            ),
        )
        // One alternative and no `otherwise`: a box too narrow for a lane and
        // its pocket band, or too short for the vault to clear a pocket, is a
        // refusal naming the rule, never a thinner lane.
        .rule_alts(
            "stair_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(MIN_X)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(MIN_DEPTH)),
                    cmp(par("pocket_height"), CmpOp::Le, par("head")),
                    cmp(par("pocket_period"), CmpOp::Ge, int(2)),
                ]),
                split_exact(
                    Axis::X,
                    vec![rel(1), abs(RUN_WIDTH), rel(1), abs(1), abs(1)],
                    vec![
                        call("rough_column"),
                        call("smooth_column"),
                        call("rough_column"),
                        call("pocket_column"),
                        call("pocket_backing"),
                    ],
                ),
            )],
        )
        // --- plain columns, one course of floor, open headroom, a vault rib -
        .rule(
            "rough_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("rough"), void(), fill("rough")],
            ),
        )
        .rule(
            "smooth_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![
                    marked("stair-run", MarkAt::FloorCenter, fill("smooth")),
                    void(),
                    marked("volley-slot", MarkAt::FloorCenter, fill("rough")),
                ],
            ),
        )
        // Always solid, full height: what backs a pocket so the notch never
        // opens onto the model's own edge.
        .rule("pocket_backing", fill("rough"))
        // --- the pocket band: alternates niche / plain wall along Z ---------
        // A `split_repeat` pattern has to fit at least once before it can be
        // tiled — `make_split` checks the un-repeated pattern first — so a box
        // shorter than one full `pocket_period` cannot ask for even one
        // pocket. That box is still legal: the same shape `rafter_hall` uses
        // for a hall too short for its truss, a solid pocket band and no
        // `anchor/pocket-*` is a variant, not an error.
        .rule_alts(
            "pocket_column",
            vec![
                alt_when(
                    cmp(dim(DimRef::Z), CmpOp::Ge, par("pocket_period")),
                    split_repeat(
                        Axis::Z,
                        vec![abs(1), gap()],
                        vec![call("pocket_niche"), call("pocket_wall")],
                    ),
                ),
                alt_when(
                    cmp(dim(DimRef::Z), CmpOp::Lt, par("pocket_period")),
                    call("pocket_wall"),
                ),
            ],
        )
        .rule("pocket_wall", fill("rough"))
        .rule(
            "pocket_niche",
            split(
                Axis::Y,
                vec![abs(1), absp("pocket_height"), rel(1)],
                vec![
                    fill("rough"),
                    // Turn the scope so its local Z is the across-lane axis:
                    // the derived facing is then the negative direction of
                    // that axis, i.e. out of the pocket at the near lane —
                    // the same construction `cliff_path`'s recess uses.
                    reoriented(
                        Reorient::KEEP.z(AxisSpec::LocalX),
                        marked_each("pocket", MarkAt::CornerMin, void()),
                    ),
                    fill("rough"),
                ],
            ),
        )
}

/// The gap between pocket slots: the period less the slot itself.
fn gap() -> crate::ir::Size {
    abse(par("pocket_period").arith(ArithOp::Sub, int(1)))
}
