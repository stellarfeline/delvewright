# `delvec::compiler::continuity`

The reference page for `crates/delvec/src/compiler/continuity.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0351 — NPC location-continuity lint (`compiler::continuity`; **warning**; exit 0)

Tracks each NPC's **staged location history** through the campaign timeline —
the stage-2 anchor (or off-stage while `deferred`), every `move-npc`
destination, every `despawn-npc`/`spawn-npc` pair (a `spawn-npc` always places
at the NPC's declared anchor) — and warns when an NPC materializes or vanishes
at a location discontinuous with where it was last staged, with no movement in
between (an NPC popping into an alcove mid-story having never been staged
entering; an NPC "grabbed at the cave mouth" while its body vanishes at a camp
elsewhere).

Three shapes warn:

* **re-entry jump** — `spawn-npc` re-materializes an NPC at its declared anchor
  after it was last staged elsewhere;
* **unstaged entrance** — a never-yet-staged deferred NPC materializes
  mid-story with no staged arrival. The accepted staging shape is firing the
  `spawn-npc` from a `move-actor`/`move-npc` `on_arrive` whose destination IS
  the NPC's anchor (walk a stand-in to the spot, swap the npc in on arrival);
* **remote dismissal** — `despawn-npc` fires from a beat whose scene anchor
  (the completing objective's anchor; a `talk-to`'s scene is its target NPC's
  staged spot) differs from where the NPC's body stands.

**Conservative model (no temporal reasoning).** Locations are symbolic anchor
names (same name = same place; no geometry). The timeline is the quest-DAG
linearization (stage-4 `depends_on` topo order; objectives in `after` order;
bundles in declared order, descending into `sequence` steps and `on_arrive`
lists in place). Anything whose firing time is statically unknowable makes the
NPC **untracked** instead of guessed at: a lifecycle effect fired from an
environment trigger, a dialogue option, an `on_respawn`/`on_caught` reaction
bundle, or carrying a `requires_flags`/`forbids_flags` gate excludes that NPC
from the lint entirely.

**Why warning, not error.** Whether a jump reads as broken is authorial taste —
narrative cover ("he slipped away while you slept") legitimizes any of these.
The message names the discontinuity concretely and prescribes the remedy
(stage a walk / spawn at the last staged location / accept with narrative
cover); the author decides.

| Code | Meaning |
|------|---------|
| `DW0351` | An NPC materializes (`spawn-npc`) or vanishes (`despawn-npc`) at a location discontinuous with its last staged location, with no movement in between. Advisory (exit 0): stage the move, re-anchor, or accept with narrative cover. |
