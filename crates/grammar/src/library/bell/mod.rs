//! The drowned-bell remake's **zone programs** — the staging vocabulary
//! composed into the zones the remake design names (REMAKE §3, build-sequence
//! step 3).
//!
//! A zone is one grammar program (REMAKE §2). These programs contain no
//! encounter geometry of their own: they lay out boxes and `call` the staging
//! vocabulary of [`super`], brought in by [`crate::compose::include`]. The only
//! blocks a zone program writes itself are the **mass** a zone is carved out of
//! and the **absence** beside it — the crag under the cliff road, the gulf the
//! road is cut into, and the inert rock filling the strip a branch is parked in
//! — because those are facts about the zone's box that no piece of vocabulary
//! can know. Three of the six zones here write nothing at all; the other three
//! write only that.
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
//! REMAKE §3 names eight zones. Six are programmed here; the other two need
//! something that does not exist yet, and a zone program that faked it would be
//! worse than one that does not exist. Each gap names what it waits on.
//!
//! A **partial** zone is one whose spine is composed and whose remaining §4
//! entries are named below. That a letter is missing from a row no longer means
//! its rule is missing — since the W3/W4 families landed, every letter in this
//! table except **B** and **D** exists as a rule, and what those rows wait on is
//! a zone-program round composing them. `counterweight_lift` used to be a third
//! entry on that list and has been **struck**: see Z7 below.
//!
//! | Zone | State | Composed from | Missing |
//! |---|---|---|---|
//! | Z0 Barrow Shore | [`barrow_shore`] | `elite_ground` | — (**E** is the whole of Z0's vocabulary) |
//! | Z1 Cliff Road | [`cliff_road`] | `cliff_path` + the zone's gulf | switchback landing (no catalogue entry — see below) |
//! | Z2 Gatehouse | [`gate_ward`] (partial) | `watch_bay` + `ambush_door` | **W**+**S** (`boulder_stair`), **F** (`far_side_bar`), **L** (`drop_shaft`), **M** (`threshold_motif`) — all built rules, awaiting a zone round — and the boulder jam (**D**), which has no rule |
//! | Z3 Drowned Lower Ward | **not programmed** | — | nothing in the *vocabulary*: **T**, **E** and **F** are all built rules. One composition blocker left (`causeway` has no exit past its post); the other two are closed — see below |
//! | Z4 Chapel Ward (hub) | [`chapel_ward`] (partial) | `dumbwaiter` + `tee_passage` + `far_side_bar` + the zone's branch strip | the hearth: a rest point is `bonfire{anchor}` (spec-0016 §1) and no rule declares an anchor for one. The smallest honest form is a `hearth_ward` rule, which is §5b's business and not a zone's |
//! | Z5 Great Hall + Keep | [`hall_keep`] (partial) | `rafter_hall` + `ambush_door` + `store_room` | **L** (`dumbwaiter`) and **M** (`threshold_motif`) — both built rules, awaiting a zone round — and the bait-item gallery (**B**), which has no rule |
//! | Z6 Cistern Deep | [`cistern_deep`] | `drop_shaft` + `watch_bay` + `broken_grate` + `elite_ground` + `tee_passage` + `far_side_bar` + the zone's branch strip | — |
//! | Z7 Bell Tower | **not programmed** | — | a zone round, and one rule. Both recorded blockers are closed or struck — [`crate::library::stair_flight`] ascends, and the counterweight lift is no longer grammar's problem. See below. Its loft is `rafter_hall` and its boss ring is `elite_ground` |
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
//! One caveat for whoever writes Z7. A straight flight climbs at most about
//! `Z / tread`, so a tall tower over a small footprint wants a **switchback** —
//! and a switchback is a *rule body*, not an orientation. "A permutation cannot
//! reflect" is true and is the right answer to a different question: a rule that
//! peels its treads off the other end of its own split climbs the other way,
//! and two such lanes side by side in `X` are a dogleg. Not built.
//!
//! **The counterweight lift (**L**) — STRUCK, not blocked.** The lift shipped as
//! a first-class DSL construct (spec-0031): runtime state, region fill/clear and
//! teleport-by-region composed into one `sequence` in campaign JSON. Nothing
//! moves, so this crate never has to express motion, and a `counterweight_lift`
//! *rule* would build a thing that no longer exists as geometry. What a lift
//! wants from the grammar is a **walled shaft with a station per floor** — a
//! `lift_shaft` §5b rule, ordinary static work, and the anchors it needs are
//! exactly the point anchors `mark` already declares. See
//! `docs/reference/grammar.md` §5b.
//!
//! ## The seam's limits: two closed, one open
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
//! 2. **`causeway` has no exit past its guard post — STILL OPEN, re-measured.**
//!    Its far end is the post's own plinth — solid from the ward floor up to
//!    `rise + tower_rise`, with the post's floor an island the berm cannot reach
//!    (deliberately: "not a landing"). The piece is a *terminus*, so any chain
//!    through it is severed at that face, whichever end of the zone it is placed
//!    at, and no orientation helps: a grammar orientation is a permutation
//!    without reflection, so the post cannot be turned to the entry end either.
//!    Z3 waits on an exit lane past the post — a change to the §5b rule, and not
//!    something a zone may write. The smallest honest form is a `berm_gate`
//!    parameter, off by default so every existing expansion stays byte-identical,
//!    that carries the berm's own column through `guard_support` and
//!    `guard_cantilever` at berm height: the post becomes a gatehouse you pass
//!    *under* rather than a plug, and its sightline gate is unaffected because
//!    the lane is `tower_rise` blocks below the guard's own floor.
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
pub mod chapel_ward;
pub mod cistern_deep;
pub mod cliff_road;
pub mod gate_ward;
pub mod hall_keep;

pub use barrow_shore::barrow_shore;
pub use chapel_ward::chapel_ward;
pub use cistern_deep::cistern_deep;
pub use cliff_road::cliff_road;
pub use gate_ward::gate_ward;
pub use hall_keep::hall_keep;

use crate::compose::{AnchorRenames, include_renaming};
use crate::ir::Program;

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
