# spec-0016 — Souls-mode mechanics (M4)

- **Status**: Draft (planner-personal, 2026-08-01; owner's souls direction
  from spec-0011/0012 closing notes; consumes the jump-arc spike
  (docs/notes/jump-arc-model.md) and the TD-routing spike (pending))
- **Vision**: death is a legitimate teacher. Delves may be punishing when the
  punishment is legible, retry is fast, and every kill the world scores on
  the player was telegraphed. The engine's job is to make "fair" provable.

## Mechanics

### 1. Bonfires (rest points)
`set-checkpoint` gains sibling verb **`bonfire{anchor}`**: places a rest
interact at the checkpoint anchor (campfire prop affordance). Resting fires
**`on_rest[]`** hooks (superset of today's `on_respawn`): reset scene state —
re-arm `reset: rearm` traps, re-seat cleared waves (declared per wave:
`respawns_on_rest: true`), restore deferred-NPC/actor postures. Death =
respawn at last-rested bonfire with the same hooks. `on_respawn` remains for
non-bonfire checkpoints (island-style). Proofs: bonfire placement inherits
DW0316 (standable) + DW0315 (no stranding); a wave marked respawns_on_rest
on the critical path re-runs its completability proof post-rest.

### 2. One-way doors
Existing primitives compose: `close-gate` fired behind the player = a point
of no return. New proof obligation only: the forward path from every one-way
crossing must still reach a bonfire before any lethal hazard (no
death-warp-to-stale-bonfire traps) — extends the DW0315 machinery with the
gate-causal model from #108.

### 3. Ambushes
Deferred NPC/actor + `approach`/`strike` trigger + `spawn-actor`/`unleash`
at corner/doorway anchors — all landed (v0.6). Missing sugar, added here:
**`ambush{at, actors[], trigger, telegraph}`** — one declaration expanding
to the deferred spawns + trigger + a REQUIRED `telegraph` (sound or sight
line beat ≥1.5 s before contact; the fairness rule made structural). New
diagnostic: an ambush without a telegraph is an error, not a style choice.

### 4. Timing gates
`schedule`-driven oscillating gates: **`timed-gate{gate, open_ticks,
closed_ticks, phase?}`** emitting a deterministic clock over the gate region
(open-gate/close-gate alternation). Proof: the passage window admits a
walking player (window ≥ crossing time at walk speed over the gate span,
computed from the nav model); required-path timed gates must be passable
from EVERY phase within one full cycle (no unwinnable phase).

### 5. Lethal parkour
Jump edges per the measured model (docs/notes/jump-arc-model.md): cardinal
jump edges, sprint-runway requirement, clearance 3/3/2, content envelope =
measured-max − 1 (flat gap ≤ 2, +1 rise gap ≤ 1). Area opt-in
`parkour: true`; two diagnostics (required-but-unjumpable; final-world jump
self-check). Fall-damage routes death to the bonfire; checkpoint-density
rule: ≤ 45 s of traversal between a lethal jump sequence and its bonfire.

### 6. TD lanes (routed-then-feral)
(Pending the W4 spike verdict — mechanism, parameters, DSL surface land
here; owner rule fixed in advance: mobs march the lane while distant and
fight with NATIVE AI once aggroed, never brushing past.)

### 7. Death-as-learning pacing rules (design contract, linted)
Retry cost: bonfire → point of failure ≤ 60 s traversal on the proven path.
Telegraph rule: every lethal source (trap, ambush, gate, fall) carries a
declared telegraph. Escalation rule: a mechanic kills gently before it kills
hard (first instance of each hazard class on a path must be survivable at
full health). These are warning-tier lints, not hard errors — taste stays
with the author, the compiler keeps the receipts.

## Acceptance

1. Planner-built multi-level souls campaign using every mechanic above,
   full ladder green, owner-played.
2. **Skill-completeness prompt suite** (owner directive): planner-authored
   prompts the OWNER runs through /new-delve — per genre a one-line
   creative-freedom prompt AND a detailed fidelity brief with checkable
   criteria. Genres: tower defense; one-map siege (attacker + defender
   campaigns); stealth heist; horror escape; detective mystery; moving
   escort; time-attack parkour; boss rush; puzzle dungeon; horde survival;
   branching moral drama; negotiation adventure. Suite ships as an appendix
   to this spec before M4 close.

## Non-goals
Difficulty settings (a delve is tuned once); PvP; procedural difficulty
scaling; any runtime LLM.
