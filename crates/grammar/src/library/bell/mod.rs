//! The drowned-bell remake's **zone programs** — the staging vocabulary
//! composed into the zones the remake design names (REMAKE §3, build-sequence
//! step 3).
//!
//! A zone is one grammar program (REMAKE §2). These programs contain no
//! encounter geometry of their own: they lay out boxes and `call` the vocabulary
//! of [`super::cliff_path`], [`super::watch_bay`], [`super::rafter_hall`],
//! [`super::ambush_door`] and [`super::store_room`], brought in by
//! [`crate::compose::include`]. The only blocks a zone program writes itself are
//! the **mass** a zone is carved out of and the **absence** beside it — the
//! crag under the cliff road, and the gulf the road is cut into — because those
//! are facts about the zone's box that no piece of vocabulary can know.
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
//! REMAKE §3 names eight zones. Three are programmed here; the other five need
//! vocabulary that does not exist yet, and a zone program that faked it would be
//! worse than one that does not exist. Each gap names the §4 catalogue entry it
//! comes from.
//!
//! | Zone | State | Composed from | Missing rule (§4 entry) |
//! |---|---|---|---|
//! | Z0 Barrow Shore | **not programmed** | — | open elite ground with two proven flank lanes (**E**) |
//! | Z1 Cliff Road | [`cliff_road`] | `cliff_path` + the zone's gulf | switchback landing (no catalogue entry — see below) |
//! | Z2 Gatehouse | [`gate_ward`] (partial) | `watch_bay` + `ambush_door` | boulder stair with worn-tread lane (**W**), hazard-run safe pockets (**S**), boulder jam (**D**), sally-port far-side bar (**F**), spill shaft (**L**), boss-threshold motif (**M**) |
//! | Z3 Drowned Lower Ward | **not programmed** | — | flooded floor + raised causeway (**T**), elite ground (**E**), sluice far-side bar (**F**) |
//! | Z4 Chapel Ward (hub) | **not programmed** | — | the hub's own shape: hearth ward + the landing every later shortcut arrives at (no catalogue entry; the hub is topology, and `L`/`F` are its hardware) |
//! | Z5 Great Hall + Keep | [`hall_keep`] | `rafter_hall` + `ambush_door` + `store_room` | bait-item gallery (**B**), kitchen dumbwaiter (**L**), boss-threshold motif (**M**) |
//! | Z6 Cistern Deep | **not programmed** | — | broken-grate secret (**X**), stair bar (**F**/**L**), elite ground (**E**). Its dart gallery is `watch_bay`, so the composable half is [`gate_ward`]'s shape exactly; a second zone program asserting the same three gates over the same two rules would be a copy, not a proof |
//! | Z7 Bell Tower | **not programmed** | — | counterweight lift (**L**), boss ring / elite ground (**E**), threshold motif (**M**). Its loft is `rafter_hall`, on the same argument as Z6 |
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

pub mod cliff_road;
pub mod gate_ward;
pub mod hall_keep;

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
