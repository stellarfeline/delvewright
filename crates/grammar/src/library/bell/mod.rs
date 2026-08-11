//! The drowned-bell remake's **zone programs** — the staging vocabulary
//! composed into the zones the remake design names (REMAKE §3, build-sequence
//! step 3).
//!
//! A zone is one grammar program (REMAKE §2). These programs contain no
//! encounter geometry of their own: they lay out boxes and `call` the staging
//! vocabulary of [`super`], brought in by [`crate::compose::include`]. The only
//! blocks a zone program writes itself are the **mass** a zone is carved out of
//! and the **absence** beside it — the crag under the cliff road, the gulf the
//! road is cut into, the inert rock filling the strip a branch is parked in, and
//! the plinth a zone stands on when it *leaves* one level down ([`gate_ward`],
//! [`hall_keep`]) or climbs one ([`bell_tower`]) — because those are facts about
//! the zone's box that no piece of vocabulary can know. One of the eight zones
//! here writes nothing at all; the other seven write only that.
//!
//! # The frame constrains composition, and it is worth saying out loud
//!
//! Every vocabulary rule opens with `z(Largest)`: it turns its length onto the
//! longer horizontal axis of whatever box it is handed. That makes a rule
//! reusable turned 90°, and it means a zone piece **shorter than the zone is
//! wide gets turned sideways** — its wall would run across the route instead of
//! along it. No composition can override it (a child rule reorients itself after
//! the parent's `orient` has been applied), so every zone program guards it: a
//! piece run shorter than the zone's width has no applicable alternative and the
//! expansion is refused, loudly, naming the rule. The price is that zone pieces
//! are long: a 11-wide keep cannot hold a 7-long threshold room, even though
//! `ambush_door` builds one happily in isolation.
//!
//! The primitive that would remove the constraint is a way for a caller to pin a
//! rule's travel axis — the same shape as the `local_*` facing spec
//! `docs/reference/grammar.md` §7 already files, one layer out. Not built here:
//! the red line for this round was compose-existing-rules-only.
//!
//! # Which zones exist, and what the rest are waiting for
//!
//! REMAKE §3 names eight zones, and **all eight are programmed here, with
//! nothing left in any Missing column.** Z7 was the last, and both of the
//! blockers its row recorded turned out to be stale rather than open — which is
//! the reason its section below is kept in full. Its own last gap, BF5, closed
//! one round later than the zone itself: [`crate::library::hearth_ward`] landed
//! for Z4's hearth and Z7 composes it rather than re-describing the hole.
//!
//! A **partial** zone is one whose spine is composed and whose remaining §4
//! entries are named below. **There are none left.** The three §4 entries that
//! had no rule at all — the bait gallery (**B**), the boulder jam (**D**) and the
//! hearth — are built as [`crate::library::bait_stand`],
//! [`crate::library::disarm_stand`] and [`crate::library::hearth_ward`], and the
//! letters that were only waiting for a zone round are composed below.
//! `counterweight_lift` was on that list too and has been **struck**: see Z7.
//!
//! | Zone | State | Composed from | Missing |
//! |---|---|---|---|
//! | Z0 Barrow Shore | [`barrow_shore`] | `elite_ground` | — (**E** is the whole of Z0's vocabulary) |
//! | Z1 Cliff Road | [`cliff_road`] | `cliff_path` + the zone's gulf | switchback landing (no catalogue entry — see below) |
//! | Z2 Gatehouse | [`gate_ward`] | `watch_bay` + `ambush_door` + `disarm_stand` + `boulder_stair` + `tee_passage` + `far_side_bar` + `threshold_motif` + `drop_shaft` + the zone's plinth and branch strip | — |
//! | Z3 Drowned Lower Ward | [`drowned_ward`] | `causeway` + `tee_passage` + `elite_ground` + `far_side_bar` + the zone's branch strip | — |
//! | Z4 Chapel Ward (hub) | [`chapel_ward`] | `dumbwaiter` + `hearth_ward` + `tee_passage` + `far_side_bar` + the zone's branch strip | — |
//! | Z5 Great Hall + Keep | [`hall_keep`] | `rafter_hall` + `ambush_door` + `store_room` + `bait_stand` + `threshold_motif` + `dumbwaiter` + the zone's plinth | — |
//! | Z6 Cistern Deep | [`cistern_deep`] | `drop_shaft` + `watch_bay` + `broken_grate` + `elite_ground` + `tee_passage` + `far_side_bar` + the zone's branch strip | — |
//! | Z7 Bell Tower | [`bell_tower`] | `stair_flight` + `hearth_ward` + `rafter_hall` + `tee_passage` + `threshold_motif` + `elite_ground` + `lift_shaft` + the zone's plinth and branch strip | — |
//!
//! ## The plinth: how a zone *leaves* one level down
//!
//! Every vertical piece builds its entry ledge `drop` blocks up and its landing
//! at the floor, so a zone that puts one anywhere but its own `Z`-max end has to
//! raise everything above the drop to meet that ledge. Z6 sidestepped it by being
//! *entered* by falling. Z2 and Z5 cannot: both are walked into and left down a
//! shaft, and that is what the design asks for.
//!
//! The construction is the branch strip's sibling and licensed by the same
//! clause: split the shaft's own slice off the `Z` end, and give the remainder a
//! `Y` split whose lower piece is inert `margin` rock. The upper ward's floor
//! then lands at exactly the shaft's entry-floor height and the seam is ordinary.
//! Two details make it honest rather than a coincidence:
//!
//! * the plinth's thickness is **read from the piece** (`par("shaft/drop")`,
//!   `par("duct/drop")`) rather than restated as a zone constant, so a campaign
//!   that dials the fall moves the floor with it — and the one-way gate's teeth,
//!   which shorten the drop, still describe a zone that builds. The tolerance is
//!   measured and written down at [`gate_ward`]: one block of mismatch is a step
//!   and every gate stays green, two is a one-way seam and five go red;
//! * the zone guards that a plinth leaves an upper ward at all
//!   ([`gate_ward::MIN_UPPER`]). A piece handed too little refuses for itself,
//!   loudly; a *remainder of zero* would be written silently, which is the one
//!   failure mode a guard is owed for.
//!
//! ## Z7's two blockers: one closed, one struck
//!
//! Both are recorded here in full, because each was believed for longer than it
//! was true and a stale blocker is worse than a missing feature.
//!
//! **"Nothing in the vocabulary climbs" — CLOSED.** Every vertical piece used to
//! be one-way **down** *by construction and by gate*:
//! [`crate::library::drop_shaft`] and [`crate::library::dumbwaiter`] both assert
//! that their landing does **not** reach their hatch under the plain step, and
//! [`crate::library::boulder_stair`] is flat.
//! [`crate::library::stair_flight`] is the way up: a walled shaft, a landing at
//! each end, a run of single-block treads, gated on the plain ±1-step walk in
//! **both** directions — the literal negation of `drop_shaft`'s gate, asserted
//! with the same predicate and cross-checked against it in one test.
//!
//! The open question was whether a climbing run is expressible without a
//! per-iteration index. **It is, and no IR change was made.** The index a stair
//! needs is not a counter but the box that is left, and a self-call already
//! carries it; `boulder_stair`'s note, which said otherwise, was right about
//! `split_repeat` and wrong about the IR, and is corrected at its own module.
//!
//! That the piece survives *composition* is asserted rather than assumed —
//! `tests/zones.rs::a_zone_can_compose_a_route_a_player_walks_up` walks a
//! composed model from a zone's entry face to `anchor/stair-head`, seven blocks
//! up. Nothing new was needed at the seam: a flight's foot landing sits on the
//! same floor course every flat piece uses.
//!
//! One caveat that was left for whoever wrote Z7, and how [`bell_tower`]
//! answered it. A straight flight climbs at most about `Z / tread`, so a tall
//! tower over a small footprint wants a **switchback** — and a switchback is a
//! *rule body*, not an orientation. "A permutation cannot reflect" is true and
//! is the right answer to a different question: a rule that peels its treads off
//! the other end of its own split climbs the other way, and two such lanes side
//! by side in `X` are a dogleg. Still not built, and Z7 did not need it: a
//! box-garden tower is a box like every other zone's, so the flight is simply
//! given a long enough run and the zone writes the **plinth** the four upper
//! pieces stand on. That is the second thing the round found out: the seam
//! between a climbing piece and a flat one is not a new node kind, it is the
//! same "mass no piece handed a sub-box can know about" `cliff_road`'s crag is.
//!
//! And the third, which cost a refusal to learn: that mass **cannot be derived
//! where it is cut**. A split's size is evaluated in the scope it is written in,
//! and `dim(Z)` inside the upper storey's own box is the upper run rather than
//! the zone's length, so the expression for the flight's rise evaluates to
//! nonsense there (the first draft cut a plinth of −4 courses). A zone in that
//! position declares the number and **guards the identity** at the one scope
//! where the whole box is visible — which is stronger than a derivation, not
//! weaker, because the guard is what a campaign's own dial runs into.
//!
//! **The counterweight lift (**L**) — STRUCK, not blocked.** The lift shipped as
//! a first-class DSL construct (spec-0031): runtime state, region fill/clear and
//! teleport-by-region composed into one `sequence` in campaign JSON. Nothing
//! moves, so this crate never has to express motion, and a `counterweight_lift`
//! *rule* would build a thing that no longer exists as geometry. What a lift
//! wants from the grammar is a **walled shaft with a station per floor** — a
//! `lift_shaft` §5b rule, ordinary static work, and the anchors it needs are
//! exactly the point anchors `mark` already declares. That rule is now
//! [`crate::library::lift_shaft`], written for this zone and against the shipped
//! lift's own contract rather than against a guess: spec-0031 records "a lift's
//! geometry is authored in NBT rather than in campaign JSON — and no prefab in
//! the library ships a shaft", and this is that prefab. See
//! `docs/reference/grammar.md` §5b.
//!
//! ## The seam's limits: all three closed
//!
//! Each of these is a *seam* limitation, not a missing shape, and each has a
//! test in `tests/zones.rs` that watches it happen rather than a paragraph
//! asserting it.
//!
//! 1. **Two pieces that declare the same anchor name — CLOSED.**
//!    [`crate::compose::include`] still never renames an anchor on its own,
//!    because an anchor name is the campaign's contract, so two pieces that
//!    share a stem still collide loudly (`causeway` and `elite_ground` on
//!    `anchor/elite`; `watch_bay` and `far_side_bar` on `anchor/gate`). What a
//!    zone can now do is say which is which:
//!    [`crate::compose::include_renaming`] takes an explicit per-anchor rename
//!    at the include site, and only the stems named there move. A ward with a
//!    causeway keeper and a dormant elite has two genuinely different elites,
//!    and the zone writes down the two names rather than a prefix deriving them.
//! 2. **`causeway` has no exit past its guard post — CLOSED.** Its far end was
//!    the post's own plinth — solid from the ward floor up to `rise +
//!    tower_rise`, with the post's floor an island the berm cannot reach
//!    (deliberately: "not a landing"). The piece was a *terminus*, so any chain
//!    through it was severed at that face, whichever end of the zone it was
//!    placed at, and no orientation helped: a grammar orientation is a
//!    permutation without reflection, so the post cannot be turned to the entry
//!    end either.
//!
//!    The rule now carries `berm_gate`, off by default: it runs the berm's own
//!    column through the guard station at berm height, so the post becomes a
//!    gatehouse the route passes *under* rather than a plug. It landed on the
//!    piece's **cross-section** rather than on either `Z`-slice of the post,
//!    because both slices need it and a capability keyed to the first consumer
//!    leaves the second with no surface. That the whole piece now shares one
//!    cross-section is what makes the lane expressible at all — and it closed a
//!    second defect nobody had reported: the post used to be centred by its own
//!    arithmetic rather than the berm's, so at every EVEN width it stood over the
//!    flood and went blind on 20 of the crossing's 22 cells. Both fixtures were
//!    odd. See [`crate::library::causeway`].
//! 3. **A shortcut is a branch, and the seam is a chain — CLOSED.** Pieces
//!    joined only along one axis, end to end, because every vocabulary rule
//!    walls its own two side faces; a `far_side_bar` laid in that chain
//!    therefore sealed the zone's own route rather than sitting beside it, which
//!    is the opposite of what a shortcut is (spec-0016 §2). The answer was
//!    vocabulary, not a new node kind: [`crate::library::tee_passage`] is a
//!    chain segment whose one side face carries a doorway — the same
//!    wall-with-one-opening construction `ambush_door` and `far_side_bar`
//!    already were, turned 90°. A zone splits off a side strip, walls its
//!    margins and hands the interior box to `far_side_bar` shaped
//!    deeper-than-wide, so the bar's own `z(Largest)` aims its travel at the
//!    chain.
//!
//! **Z1 is a single run, not a switchback**, and that is a finding rather than a
//! shortcut. A switchback alternates which side the drop is on, and a grammar
//! orientation is a permutation *without reflection* — so no `reorient` can
//! mirror a cliff run, and the two legs of a switchback cannot be the same rule
//! turned round. Building it needs either a mirroring orientation or a
//! `cliff_turn` landing rule that joins two runs at a corner; neither exists,
//! and inventing the landing inline is exactly the geometry this module does not
//! write. A third option was overlooked and is now **open rather than
//! answered**: what an orientation cannot do a *rule body* can, and
//! [`crate::library::stair_flight`] records the construction. Whether it reaches
//! `cliff_path`, whose lane and recesses are placed by `reorient` rather than by
//! split order, has not been checked. What is programmed is the owner-mandated set piece itself: the
//! one-wide ledge, the niches, and the drop beside them.

pub mod barrow_shore;
pub mod bell_tower;
pub mod chapel_ward;
pub mod cistern_deep;
pub mod cliff_road;
pub mod drowned_ward;
pub mod gate_ward;
pub mod hall_keep;

pub use barrow_shore::barrow_shore;
pub use bell_tower::bell_tower;
pub use chapel_ward::chapel_ward;
pub use cistern_deep::cistern_deep;
pub use cliff_road::cliff_road;
pub use drowned_ward::drowned_ward;
pub use gate_ward::gate_ward;
pub use hall_keep::hall_keep;

use crate::compose::{AnchorRenames, include_renaming};
use crate::ir::Program;

/// One row of [`ZONES`]: what a zone is called, how to build it, and the box it
/// is designed for.
pub struct Zone {
    /// The zone's stable id — what a sweep manifest and a contact sheet name it
    /// by, and what the owner says out loud. Kebab-case, so it is also a usable
    /// directory and structure-file name.
    pub id: &'static str,
    /// Builds the zone's grammar program.
    pub program: fn() -> Program,
    /// The zone's design box (`REGION` of its own module).
    pub region: [u32; 3],
}

/// Every zone of the remake, in ascent order (REMAKE §3).
///
/// A registry rather than eight names a caller has to know: a tool that sweeps
/// or renders zones enumerates *this*, so adding a zone reaches every such tool
/// without touching one of them. Ascent order rather than alphabetical because
/// that is the order the campaign — and the owner reviewing eight sheets — walks
/// them in.
pub static ZONES: &[Zone] = &[
    Zone {
        id: "barrow-shore",
        program: barrow_shore,
        region: barrow_shore::REGION,
    },
    Zone {
        id: "cliff-road",
        program: cliff_road,
        region: cliff_road::REGION,
    },
    Zone {
        id: "gate-ward",
        program: gate_ward,
        region: gate_ward::REGION,
    },
    Zone {
        id: "drowned-ward",
        program: drowned_ward,
        region: drowned_ward::REGION,
    },
    Zone {
        id: "chapel-ward",
        program: chapel_ward,
        region: chapel_ward::REGION,
    },
    Zone {
        id: "hall-keep",
        program: hall_keep,
        region: hall_keep::REGION,
    },
    Zone {
        id: "cistern-deep",
        program: cistern_deep,
        region: cistern_deep::REGION,
    },
    Zone {
        id: "bell-tower",
        program: bell_tower,
        region: bell_tower::REGION,
    },
];

/// Include every named piece of vocabulary into a zone plan.
///
/// A clash here is an authoring mistake in this module — two pieces given the
/// same prefix — and it is caught by [`crate::ir::Program::validate`] in the
/// library suite either way; it panics rather than widening every zone
/// constructor's return type into a `Result` no caller could act on.
fn composed(zone: Program, parts: &[(&str, &Program)]) -> Program {
    let renamed: Vec<(&str, &Program, AnchorRenames<'_>)> = parts
        .iter()
        .map(|(prefix, source)| (*prefix, *source, AnchorRenames::new()))
        .collect();
    composed_renaming(zone, &renamed)
}

/// [`composed`], plus the per-anchor renames a zone gives at each include site.
///
/// A zone that composes two pieces declaring one stem has to say which is which
/// — the seam deliberately never derives it (`docs/reference/grammar.md` §5c) —
/// and the map is written out beside the piece it renames so a reader of the
/// zone can see the contract the campaign will bind. A rename that names nothing
/// is refused by [`include_renaming`] and panics here for the same reason a
/// prefix clash does.
fn composed_renaming(zone: Program, parts: &[(&str, &Program, AnchorRenames<'_>)]) -> Program {
    let name = zone.name.clone();
    let mut out = zone;
    for (prefix, source, renames) in parts {
        out = include_renaming(out, source, prefix, renames)
            .unwrap_or_else(|e| panic!("{name} cannot include {prefix:?}: {e}"));
    }
    out
}
