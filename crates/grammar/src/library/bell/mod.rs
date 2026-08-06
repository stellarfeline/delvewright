//! The drowned-bell remake's **zone programs** — the staging vocabulary
//! composed into the zones the remake design names (REMAKE §3, build-sequence
//! step 3).
//!
//! A zone is one grammar program (REMAKE §2). These programs contain no
//! encounter geometry of their own: they lay out boxes and `call` the staging
//! vocabulary of [`super`], brought in by [`crate::compose::include`]. The only
//! blocks a zone program writes itself are the **mass** a zone is carved out of
//! and the **absence** beside it — the crag under the cliff road, and the gulf
//! the road is cut into — because those are facts about the zone's box that no
//! piece of vocabulary can know. Four of the five zones here write nothing at
//! all.
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
//! REMAKE §3 names eight zones. Five are programmed here; the other three need
//! something that does not exist yet, and a zone program that faked it would be
//! worse than one that does not exist. Each gap names what it waits on.
//!
//! | Zone | State | Composed from | Missing |
//! |---|---|---|---|
//! | Z0 Barrow Shore | [`barrow_shore`] | `elite_ground` | — (**E** is the whole of Z0's vocabulary) |
//! | Z1 Cliff Road | [`cliff_road`] | `cliff_path` + the zone's gulf | switchback landing (no catalogue entry — see below) |
//! | Z2 Gatehouse | [`gate_ward`] (partial) | `watch_bay` + `ambush_door` | boulder stair with worn-tread lane (**W**), hazard-run safe pockets (**S**), boulder jam (**D**), sally-port far-side bar (**F**), spill shaft (**L**), boss-threshold motif (**M**) |
//! | Z3 Drowned Lower Ward | **not programmed** | — | nothing in the *vocabulary*: **T**, **E** and **F** are all built rules. One composition blocker left (`causeway` has no exit); the other two are closed — see below |
//! | Z4 Chapel Ward (hub) | **not programmed** | — | the hub's own shape: hearth ward + the landing every later shortcut arrives at (no catalogue entry; the hub is topology, and `L`/`F` are its hardware) |
//! | Z5 Great Hall + Keep | [`hall_keep`] | `rafter_hall` + `ambush_door` + `store_room` | bait-item gallery (**B**), kitchen dumbwaiter (**L**), boss-threshold motif (**M**) |
//! | Z6 Cistern Deep | [`cistern_deep`] | `drop_shaft` + `watch_bay` + `broken_grate` + `elite_ground` | the sally-port far-side bar (**F**) — no longer blocked; it waits on a zone-program round, see below |
//! | Z7 Bell Tower | **not programmed** | — | counterweight lift (**L**), which is not built and cannot be with today's IR (`docs/reference/grammar.md` §5b). Its loft is `rafter_hall` and its boss ring is `elite_ground` |
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
//! 2. **`causeway` has no exit past its guard post.** Its far end is the post's
//!    own plinth — solid from the ward floor up to `rise + tower_rise`, with the
//!    post's floor an island the berm cannot reach (deliberately: "not a
//!    landing"). The piece is a *terminus*, so any chain through it is severed
//!    at that face, whichever end of the zone it is placed at. Z3 waits on an
//!    exit lane past the post, which is a change to the §5b rule and not
//!    something a zone may write.
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
//! write. What is programmed is the owner-mandated set piece itself: the
//! one-wide ledge, the niches, and the drop beside them.

pub mod barrow_shore;
pub mod cistern_deep;
pub mod cliff_road;
pub mod gate_ward;
pub mod hall_keep;

pub use barrow_shore::barrow_shore;
pub use cistern_deep::cistern_deep;
pub use cliff_road::cliff_road;
pub use gate_ward::gate_ward;
pub use hall_keep::hall_keep;

use crate::compose::include;
use crate::ir::Program;

/// Include every named piece of vocabulary into a zone plan.
///
/// A clash here is an authoring mistake in this module — two pieces given the
/// same prefix — and it is caught by [`crate::ir::Program::validate`] in the
/// library suite either way; it panics rather than widening every zone
/// constructor's return type into a `Result` no caller could act on.
fn composed(zone: Program, parts: &[(&str, &Program)]) -> Program {
    let name = zone.name.clone();
    let mut out = zone;
    for (prefix, source) in parts {
        out = include(out, source, prefix)
            .unwrap_or_else(|e| panic!("{name} cannot include {prefix:?}: {e}"));
    }
    out
}
