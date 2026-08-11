//! The hearth ward — a lane with a sheltered nook a body can stop in, and one
//! declared focus inside it (the rest point Z4 was waiting on).
//!
//! **Original Delvewright content**, not a port: licence `original`.
//!
//! # What this rule is a mechanism *for*, and what it deliberately is not
//!
//! A rest point in the campaign DSL is `bonfire{anchor}` (spec-0016 §1): a verb
//! that needs a cell to bind. No rule in the vocabulary declared one, and a zone
//! program may not mint an anchor of its own — an anchor name is the campaign's
//! contract with a **rule**. So what was missing was never "a bonfire"; it was a
//! *mechanism*: **somewhere off the road, approachable from one direction only,
//! with one cell declared as its focus.** A checkpoint binds there. So does a
//! shrine, a vendor's stall, a save crystal, a lore stone or a locked reliquary
//! — none of which this rule has any opinion about. `hearth` is the stem the
//! anchor carries; the geometry knows nothing about fire.
//!
//! ```text
//!  local X:  0      1 .. 2      3         4 .. X-2      X-1
//!           wall    the nook   divider    the lane      wall
//!
//!  seen from above, travel running down the page:
//!
//!      │###################│
//!      │### ▓▓▓▓ ##########│   the nook's back wall is at local Z-max, so its
//!      │### ▓FF▓ #  lane  #│   one open face looks down-travel: a body resting
//!      │###  ^^  #        #│   in it watches the way it is about to go
//!      │###################│   travel: local Z-max -> Z-min
//!                ^ the mouth, the nook's only opening
//! ```
//!
//! # Why not a `watch_bay` with a different anchor on it
//!
//! The shape is deliberately the bay's — a pocket walled on three sides beside a
//! lane that runs past it — and that is worth saying out loud rather than
//! rediscovering. What is *not* shareable is the claim. [`super::watch_bay`]
//! exists to prove a **sightline to a hazard span it builds itself**; a rest ward
//! has no span, so composing a bay here would drag in a hazard the zone does not
//! want and bind that rule's only gate to zero cells — a green that measures
//! nothing (`docs/reference/playtest-methodology.md` rule 1). This rule's gates
//! are about **shelter and detour** instead: one way in, and the road still runs
//! without it.
//!
//! That two rules now build a pocket-off-a-lane (three, with
//! [`super::disarm_stand`]) is recorded as an open question in
//! `docs/reference/grammar.md` §7: the shape has no owner, and unifying it would
//! move the bytes of shipped zones, so it is named rather than done.
//!
//! # The gates (`tests/staging.rs`)
//!
//! 1. **The lane is still a chain segment** — standable end to end along local
//!    `Z`, so a rest ward dropped into a zone's piece run does not sever it. Red:
//!    the refusal a box under [`MIN_WIDTH`] gets, with the same box one cell
//!    wider as the control.
//! 2. **The focus is reachable, and it is a detour** — the lane connects to
//!    `anchor/hearth`, and deleting every nook cell leaves the lane connected end
//!    to end. A rest you have to walk *through* is a corridor with a campfire in
//!    it. Teeth: `mouth_sealed = 1` fills the nook's one open cell — the hearth
//!    goes unreachable while the lane walks exactly as before.
//! 3. **Exactly one way in** — the nook's standable neighbours are counted, and
//!    they are the mouth's own lane cells and nothing else. This is the property
//!    that makes a rest point defensible: you can only be come at from the
//!    direction you are already looking. Teeth: `back_door = 1` opens the outer
//!    wall behind the nook, and the neighbour count rises.
//!
//! # Anchors
//!
//! * `anchor/hearth` — the floor cell at the centre of the nook's inner half,
//!   facing out through the mouth (the derived facing, i.e. the negative local
//!   `Z`, which is why the back wall is at `Z`-max and not the other end).
//!
//! Smallest region that expands: **[`MIN_WIDTH`] wide, `head + 2` tall,
//! `nook_len + 3` long** — and at least as long as it is wide, which the frame's
//! `z(Largest)` guarantees by construction.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, abse, absp, all_of, alt_when, call, cmp, dim, fill, int, marked, par, rel, reoriented,
    split, split_exact, void,
};

/// Cells across the lane the nook itself takes. Two, so a body can step aside
/// into it rather than plug it.
pub const NOOK_WIDTH: i64 = 2;

/// The narrowest box the rule will build in: outer wall, the nook, the divider
/// that leaves the nook one open face, a cell of lane, and the far wall.
pub const MIN_WIDTH: i64 = NOOK_WIDTH + 4;

/// The shortest nook: the mouth cell, and at least one cell behind it for the
/// focus to sit in. A one-deep nook is a doorway, not a shelter.
pub const MIN_NOOK: i64 = 2;

/// The hearth ward.
///
/// Parameters: `head` (lane headroom), `nook_len` (how deep the nook runs along
/// the lane, at least [`MIN_NOOK`]), `nook_height` (the nook's own interior
/// height, which must be under `head` so the pocket reads as a pocket), and two
/// test knobs, both off by default: `mouth_sealed` fills the nook's one opening,
/// and `back_door` opens the outer wall behind it. Palette roles: `rock` (the
/// shell), `hearth_floor` (the nook's own floor course — a separate role so a
/// campaign can make the rest point read as one from across the room, and so
/// that restyling it cannot move a block).
pub fn hearth_ward() -> Program {
    Program::new("hearth_ward", "hearth_ward")
        .param("head", 3)
        .param("nook_len", 3)
        .param("nook_height", 2)
        .param("mouth_sealed", 0)
        .param("back_door", 0)
        .role("rock", BlockState::simple("stone_bricks"))
        .role("hearth_floor", BlockState::simple("polished_andesite"))
        // --- frame -----------------------------------------------------------
        .rule(
            "hearth_ward",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("ward_plan"),
            ),
        )
        // One alternative, no `otherwise`: a box too narrow for a nook beside a
        // lane, or too short to leave the lane running past both ends of it, is
        // not a smaller rest ward — it is a corridor with a dead end, which is a
        // different rule. A refusal naming this one is the honest answer.
        .rule_alts(
            "ward_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(MIN_WIDTH)),
                    cmp(
                        dim(DimRef::Z),
                        CmpOp::Ge,
                        par("nook_len").arith(ArithOp::Add, int(3)),
                    ),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(par("nook_len"), CmpOp::Ge, int(MIN_NOOK)),
                    cmp(par("nook_height"), CmpOp::Lt, par("head")),
                    cmp(par("nook_height"), CmpOp::Ge, int(2)),
                ]),
                // Low `Z` to high `Z` is the reverse of travel, so the lane the
                // player arrives along is declared last and the nook's own band
                // sits between two runs of plain corridor.
                split_exact(
                    Axis::Z,
                    vec![
                        rel(1),
                        abse(par("nook_len").arith(ArithOp::Add, int(1))),
                        rel(1),
                    ],
                    vec![call("corridor"), call("nook_band"), call("corridor")],
                ),
            )],
        )
        // --- the plain lane ----------------------------------------------------
        .rule(
            "corridor",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("rock"), call("lane_column"), fill("rock")],
            ),
        )
        .rule(
            "lane_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![fill("rock"), void(), fill("rock")],
            ),
        )
        // --- the nook's band: outer wall, nook, divider, and the lane past it ---
        // The divider is what leaves the nook exactly one open face. It is a
        // whole column of the band rather than a wall segment, which is what
        // makes "one way in" a fact about the split instead of an arithmetic
        // claim nobody re-checks.
        .rule(
            "nook_band",
            split(
                Axis::X,
                vec![abs(1), abs(NOOK_WIDTH), abs(1), rel(1), abs(1)],
                vec![
                    call("outer_wall"),
                    call("nook_column"),
                    fill("rock"),
                    call("lane_column"),
                    fill("rock"),
                ],
            ),
        )
        // Solid, except when `back_door` opens it — the one defect that would
        // give a rest point a second approach, kept expressible so the gate can
        // be watched catching it.
        .rule_alts(
            "outer_wall",
            vec![
                alt_when(cmp(par("back_door"), CmpOp::Le, int(0)), fill("rock")),
                alt_when(
                    cmp(par("back_door"), CmpOp::Ge, int(1)),
                    split(
                        Axis::Y,
                        vec![abs(1), absp("nook_height"), rel(1)],
                        vec![fill("rock"), void(), fill("rock")],
                    ),
                ),
            ],
        )
        // Along the lane: the nook, then its back wall at local `Z`-max, so the
        // one open face looks down-travel and the derived facing of the anchor
        // inside points out of it.
        .rule(
            "nook_column",
            split(
                Axis::Z,
                vec![absp("nook_len"), abs(1)],
                vec![call("nook_room"), fill("rock")],
            ),
        )
        .rule(
            "nook_room",
            split(
                Axis::Y,
                vec![abs(1), absp("nook_height"), rel(1)],
                vec![fill("hearth_floor"), call("nook_air"), fill("rock")],
            ),
        )
        // The mouth is the one cell of the nook the lane can be entered from, and
        // it is a piece of the split rather than an offset: `mouth_sealed` fills
        // exactly it, and nothing else in the nook changes.
        .rule(
            "nook_air",
            split_exact(
                Axis::Z,
                vec![abs(1), rel(1)],
                vec![call("mouth"), marked("hearth", MarkAt::FloorCenter, void())],
            ),
        )
        .rule_alts(
            "mouth",
            vec![
                alt_when(cmp(par("mouth_sealed"), CmpOp::Le, int(0)), void()),
                alt_when(cmp(par("mouth_sealed"), CmpOp::Ge, int(1)), fill("rock")),
            ],
        )
}
