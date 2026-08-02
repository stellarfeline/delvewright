# spec-0012 — Checkpoints (respawn anchors)

- **Status**: Proposed (split out of spec-0011 owner decision 2, 2026-07-31;
  drafted by the planning agent)
- **Motivation**: sealing emits `setworldspawn` = delve entrance, so any death
  (trap, wave, lava) restarts the run from the door. That caps how deep danger
  can sit and how punishing pacing may be. spec-0011's survivability proof
  currently must assume entrance respawn; checkpoints unbind it.
- **Vanilla primitive**: `/spawnpoint <targets> <pos>` — per-player respawn
  point, first-class command (no-hack compliant; no beds/respawn-anchor blocks
  involved).

## DSL surface

- Stage-5 effect verb **`set-checkpoint`** — usable wherever `set-flag` /
  `set-time` are (step completion, trigger, dialogue outcome). Payload: an
  anchor reference (`area` + `anchor/checkpoint` marker provided by the
  prefab, mirroring every other anchor kind).
- Explicit only: no implicit "checkpoint on area entry" magic. The author
  declares every checkpoint; the compiler proves it. (Open decision 2.)

## Semantics

- **Party-wide**: emission targets `@a` — the group shares one respawn point,
  consistent with a 1–4 player co-op box garden. No per-player divergence.
  (Open decision 1.)
- **Monotonic by construction**: checkpoints activate in quest order; a later
  `set-checkpoint` always replaces an earlier one. No toggling back.
- On death, vanilla respawns the player at the last activated checkpoint
  (entrance until the first one fires). `keep_inventory true` unchanged.

## Proof obligations (compile-time)

1. **Placement**: the checkpoint cell must be standable on the gravity-settled
   assembled model, not a trap trigger/hazard cell (spec-0011 model), and meet
   the area's `min_light` at the darkest declared (time, weather) state.
2. **No stranding** (the core proof): from every checkpoint, every remaining
   required anchor of the critical path must still be reachable/walkable
   (DW0311 machinery re-rooted at the checkpoint). A checkpoint behind a
   one-way drop that the forward path later leaves unreachable is a build
   error — new diagnostic **DW0315** (exit 2, prescription per the #73 rubric:
   move the checkpoint or add a return route; do NOT delete the checkpoint to
   silence the proof).
3. **spec-0011 coupling**: the survivability obligation for a lethal trap may
   assume respawn at the nearest dominating checkpoint instead of the
   entrance, once this spec is implemented.

## Emission

Deterministic: `spawnpoint @a <x> <y> <z>` in the step's completion function,
plus `data modify storage dw:cp pos set value [<x>,<y>,<z>]` — vanilla holds
the respawn point; the storage mirror is the readable "last checkpoint"
contract other features consume (spec-0013 boundary return uses a macro tp
from `dw:cp`). Setup initializes `dw:cp` to the spawn cell.

## Validation

- PackTest: after the checkpoint step fires, kill a fake player → asserts
  respawn at the checkpoint cell, not the entrance.
- Harness: no new bot logic (death-aware transport from PR #59 already
  re-acquires after respawn; waypoints re-route from the respawn cell).

## Acceptance criteria

1. Schema accepts `set-checkpoint` referencing a declared checkpoint anchor;
   unknown anchor → existing unknown-anchor diagnostic.
2. Emitted datapack contains exactly one `spawnpoint` command per declared
   checkpoint, at the declared trigger point, byte-identical across double
   builds.
3. A campaign with a checkpoint stranded behind a one-way drop fails DW0315
   (negative test); the same campaign with a return route passes.
4. PackTest respawn assertion green on the showcase campaign once one
   checkpoint is authored.

## Non-goals

- Per-player divergent checkpoints; manual player-set spawn (beds/respawn
  anchors are not placed in delves); checkpoint UI/toasts (a `narrate` line
  suffices); saving any state beyond the respawn point (delves are one
  sitting).

## Open decisions (owner)

1. **Party-wide `@a`** respawn (recommended) — confirm.
2. **Explicit `set-checkpoint` only**, no automatic per-area checkpoints
   (recommended: author intent, no magic) — confirm.

## Addendum — `on_respawn` hook (planner, 2026-08-01, island-remake driven)

`set-checkpoint` gains optional `on_respawn: [effects]`: a per-player effect
list run when a player respawns while this checkpoint is the active one
(detected via the vanilla respawn advancement/tick hook). Purpose: scene state
reset — e.g. re-caging an unleashed actor to its idle puppet (spec-0014) after
a death in an unwinnable fight. Effects must be idempotent (despawn-by-tag +
respawn patterns); the compiler orders them deterministically. This is the M3
substrate; full souls-mode "rest resets the world" stays M4.

## Note — souls-mode reframing (owner, 2026-07-31)

The owner's souls-mode direction (spec-0011 final section) reframes checkpoints
as **bonfires**: resting may reset traps/enemies, and death-driven retry is a
legitimate pattern. The open decisions above stay open and will be settled
inside the planner's souls-mode design; this spec's respawn mechanics are the
substrate it builds on.
