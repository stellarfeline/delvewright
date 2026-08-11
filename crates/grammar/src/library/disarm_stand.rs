//! The disarm stand — a hazard's control, put where the hazard cannot reach
//! (§4 entry **D**: "the boulder release can be jammed from the stair head").
//!
//! **Original Delvewright content**, not a port: licence `original`.
//!
//! # The mechanism: actuation from safety, the dual of the watch bay
//!
//! [`super::watch_bay`] is the *observation* half of the dossier's third rung —
//! somewhere to stand and read a hazard from, outside its span. This rule is the
//! *actuation* half: somewhere to stand and **switch it off** from, outside its
//! run. Stated without the fiction, it is a control cell whose every operator
//! position lies outside the thing it controls; a lever for a rolling boulder is
//! one campaign's use of it, a valve for a flooding room or a killswitch for a
//! conveyor is another's. Nothing here knows what is released.
//!
//! ```text
//!  local X:  0        1 .. 2      3         4 .. X-2     X-1
//!           wall      the stand   divider   the lane     wall     (stand zone only)
//!            ^ the release is set into this wall, at hand height
//!
//!  seen from above, travel running down the page:
//!
//!      │##########################│
//!      │##  ##  ###  the lane  ###│  the stand zone: the head, where the player
//!      │##  R▓  ###            ###│  arrives and where the release is
//!      │##  ▓▓  ###            ###│
//!      ├──────────────────────────┤
//!      │###   the hazard run    ##│  what the release governs, and what the next
//!      │###                     ##│  piece of the zone carries on
//!      └──────────────────────────┘   travel: local Z-max -> Z-min
//! ```
//!
//! # The claim that makes it a mechanism rather than a lever-shaped block
//!
//! The release is set into the stand's **outer** wall, not into the divider
//! between the stand and the lane. That is the whole design: a mechanism in the
//! divider would be reachable from inside the run — you could jam the boulder
//! while standing in its path, which is not a third rung, it is a coin flip with
//! extra steps. The `release_in_lane` knob builds exactly that mistake, and the
//! gate below is written to count it.
//!
//! A control cell is a *point*, and what a campaign hangs on it — an `EnvTrigger`
//! with `on: use`, a `timed-gate` disarm, a lever prop — is the campaign's
//! business. This rule declares no trap: trap and trigger anchors are not yet
//! expressible by a rule (`docs/reference/grammar.md` §7), and inventing one here
//! would be the downstream folklore the no-hack rule forbids. The same call
//! [`super::boulder_stair`]'s `volley-slot` already makes.
//!
//! # The gates (`tests/staging.rs`)
//!
//! 1. **The lane is a chain segment** — standable end to end, so a stand dropped
//!    into a zone's piece run does not sever it. Red: the refusal an undersized
//!    box gets.
//! 2. **The release cannot be worked from the run** — every standable cell of the
//!    hazard run is checked for adjacency to `anchor/release`, and none is. The
//!    binding is the run's own cell count, so the claim cannot go quietly vacuous
//!    on a shorter box. Teeth: `release_in_lane = 1` moves the mechanism into the
//!    divider and the count of in-run operator cells rises off zero.
//! 3. **...and it can be worked at all** — the operator cell beside the release
//!    is standable and reachable from the run. A control nobody can reach is not
//!    safer, it is absent. Teeth: `stand_sealed = 1` fills the stand's mouth, and
//!    the release goes unreachable while the lane still walks.
//!
//! # Anchors
//!
//! * `anchor/release` — the mechanism's own block, set into the stand's outer
//!   wall one cell over the floor.
//! * `anchor/run-head` — the floor cell at the head of the run, where whatever
//!   the release governs starts. A campaign binds the hazard's path to it; the
//!   cell is ordinary floor until something does.
//!
//! Smallest region that expands: **[`MIN_WIDTH`] wide, `head + 2` tall,
//! [`STAND_ZONE`] + 2 long** — and at least as long as it is wide, which the
//! frame's `z(Largest)` guarantees.

use crate::block::BlockState;
use crate::geom::Axis;
use crate::ir::{ArithOp, AxisSpec, CmpOp, DimRef, MarkAt, Program, Reorient};

use super::{
    abs, absp, all_of, alt_when, at_offset, call, cmp, dim, fill, int, marked, par, rel,
    reoriented, split, split_exact, void,
};

/// Cells across the lane the stand takes: the mouth's column and the one behind
/// it, so a body can step out of the run and still turn round.
pub const STAND_WIDTH: i64 = 2;

/// The narrowest box the rule will build in: outer wall, the stand, the divider
/// that keeps it out of the lane, a cell of lane, and the far wall.
pub const MIN_WIDTH: i64 = STAND_WIDTH + 4;

/// Cells the stand's zone takes along the lane: two of stand, one of back wall.
pub const STAND_ZONE: i64 = 3;

/// The disarm stand.
///
/// Parameters: `head` (lane headroom), `stand_height` (the stand's own interior
/// height, which must be under `head`), and two test knobs, both off by default:
/// `release_in_lane` moves the mechanism into the divider, where the run can
/// reach it, and `stand_sealed` fills the stand's mouth. Palette roles: `rock`
/// (the shell) and `mechanism` (the release's own block — its own role so a
/// campaign can make the control read as one, and so restyling it cannot move a
/// block).
pub fn disarm_stand() -> Program {
    Program::new("disarm_stand", "disarm_stand")
        .param("head", 4)
        .param("stand_height", 2)
        .param("release_in_lane", 0)
        .param("stand_sealed", 0)
        .role("rock", BlockState::simple("cobblestone"))
        .role("mechanism", BlockState::simple("polished_blackstone"))
        // --- frame -----------------------------------------------------------
        .rule(
            "disarm_stand",
            reoriented(
                Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                call("stand_plan"),
            ),
        )
        // One alternative, no `otherwise`: a box with no room for a stand beside
        // the lane, or no run for the stand to be at the head of, is not a
        // smaller version of this — it is a corridor, and the caller should ask
        // for one.
        .rule_alts(
            "stand_plan",
            vec![alt_when(
                all_of(vec![
                    cmp(dim(DimRef::X), CmpOp::Ge, int(MIN_WIDTH)),
                    cmp(dim(DimRef::Z), CmpOp::Ge, int(STAND_ZONE + 2)),
                    cmp(
                        dim(DimRef::Y),
                        CmpOp::Ge,
                        par("head").arith(ArithOp::Add, int(2)),
                    ),
                    cmp(par("stand_height"), CmpOp::Lt, par("head")),
                    cmp(par("stand_height"), CmpOp::Ge, int(2)),
                ]),
                // Low `Z` to high `Z` is the reverse of travel: the run is
                // declared first and the head the player arrives at last.
                split_exact(
                    Axis::Z,
                    vec![rel(1), abs(STAND_ZONE)],
                    vec![call("run_zone"), call("stand_zone")],
                ),
            )],
        )
        // --- the run the release governs ---------------------------------------
        .rule(
            "run_zone",
            split(
                Axis::X,
                vec![abs(1), rel(1), abs(1)],
                vec![fill("rock"), call("run_column"), fill("rock")],
            ),
        )
        // The head anchor sits on the air the run starts in, at its `Z`-max end —
        // the cell nearest the stand, which is where whatever is released begins
        // its journey.
        .rule(
            "run_column",
            split(
                Axis::Y,
                vec![abs(1), absp("head"), rel(1)],
                vec![
                    fill("rock"),
                    marked(
                        "run-head",
                        at_offset(
                            dim(DimRef::X)
                                .arith(ArithOp::Sub, int(1))
                                .arith(ArithOp::Div, int(2)),
                            int(0),
                            dim(DimRef::Z).arith(ArithOp::Sub, int(1)),
                        ),
                        void(),
                    ),
                    fill("rock"),
                ],
            ),
        )
        // --- the head: outer wall, the stand, the divider, the lane past it ----
        .rule(
            "stand_zone",
            split(
                Axis::X,
                vec![abs(1), abs(STAND_WIDTH), abs(1), rel(1), abs(1)],
                vec![
                    call("outer_wall"),
                    call("stand_column"),
                    call("divider"),
                    call("lane_column"),
                    fill("rock"),
                ],
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
        // The two walls that could carry the release, and the knob that decides
        // which one does. They are exact complements — the mechanism exists once,
        // wherever it is — so this is a decision, not a distribution
        // (`docs/reference/grammar.md` §2).
        .rule_alts(
            "outer_wall",
            vec![
                alt_when(
                    cmp(par("release_in_lane"), CmpOp::Le, int(0)),
                    call("release_wall"),
                ),
                alt_when(cmp(par("release_in_lane"), CmpOp::Ge, int(1)), fill("rock")),
            ],
        )
        .rule_alts(
            "divider",
            vec![
                alt_when(cmp(par("release_in_lane"), CmpOp::Le, int(0)), fill("rock")),
                alt_when(
                    cmp(par("release_in_lane"), CmpOp::Ge, int(1)),
                    call("release_wall"),
                ),
            ],
        )
        // A wall with the mechanism set into it at hand height, one cell into the
        // stand's own depth so the cell beside it is the stand's inner floor and
        // not its mouth.
        .rule(
            "release_wall",
            split(
                Axis::Y,
                vec![abs(1), abs(1), rel(1)],
                vec![fill("rock"), call("release_band"), fill("rock")],
            ),
        )
        .rule(
            "release_band",
            split_exact(
                Axis::Z,
                vec![abs(1), abs(1), rel(1)],
                vec![
                    fill("rock"),
                    marked("release", MarkAt::FloorCenter, fill("mechanism")),
                    fill("rock"),
                ],
            ),
        )
        // Along the lane: the stand's two cells at the low-`Z` end, open toward
        // the run, then the back wall.
        .rule(
            "stand_column",
            split(
                Axis::Z,
                vec![abs(2), abs(1)],
                vec![call("stand_room"), fill("rock")],
            ),
        )
        .rule(
            "stand_room",
            split(
                Axis::Y,
                vec![abs(1), absp("stand_height"), rel(1)],
                vec![fill("rock"), call("stand_air"), fill("rock")],
            ),
        )
        // The mouth is a piece of the split rather than an offset, so
        // `stand_sealed` fills exactly it and nothing else about the stand moves.
        .rule(
            "stand_air",
            split_exact(Axis::Z, vec![abs(1), rel(1)], vec![call("mouth"), void()]),
        )
        .rule_alts(
            "mouth",
            vec![
                alt_when(cmp(par("stand_sealed"), CmpOp::Le, int(0)), void()),
                alt_when(cmp(par("stand_sealed"), CmpOp::Ge, int(1)), fill("rock")),
            ],
        )
}
