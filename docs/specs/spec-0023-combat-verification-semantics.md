# spec-0023: Combat verification semantics — the machine proves the loop, not the win

- **Status**: Accepted
- **ADRs**: 0005 (two-layer validation — refines the bot layer's contract at
  combat encounters), 0006 (determinism)
- **Depends on**: declarable `world.difficulty` (implementation in flight);
  spec-0012 (checkpoints), spec-0016 (souls mode)

## Problem

"Provably completable by machine" collides with souls design the moment
difficulty is real: the mineflayer bot is a poor fighter, so if the bot must
WIN every fight, bot skill becomes the ceiling on combat difficulty. Souls
design requires the opposite — a player whose technique isn't there must be
able to lose despite full effort. The gap between a skilled and unskilled
Minecraft fighter is enormous, and it is design material, not noise.

## Ruling

The founding invariant stands: every delve is machine-verified before the
owner sees it. What changes is **what the machine asserts about a combat
encounter**. "The average player can win" was never a provable claim and is
no longer pretended. The machine proves the fight is REACHABLE, RETRIABLE,
and STRUCTURALLY WINNABLE; human skill is the variable the design leaves
open, deliberately.

## The three combat proofs

### 1. Retry-loop soundness (bot, the load-bearing proof)

In a souls delve the sacred property is not winning — it is that dying is
always safe. For every combat encounter on the critical path, the harness
**deliberately dies** to it and proves the loop: death → respawn at the
governing checkpoint/bonfire → the route back to the encounter is walkable →
the encounter is re-engageable (waves re-seated per `respawns_on_rest`,
actors re-armed per their lifecycle, gates in their declared state) → no
progression flag/objective was corrupted by the death. N configurable deaths
per encounter (default 2: one mid-fight, one at first contact). This is a
new required ladder stage: **die-retry**.

### 2. Structural winnability (compile-time arithmetic)

The compiler proves, per encounter, from data it already owns (kit contents,
mob equipment/attributes, the declared `difficulty`):

- every hostile is **killable**: finite HP, no `Invulnerable`, hitbox
  reachable from standable cells (body-clearance machinery, DW0450 family);
- **time-to-kill is finite and bounded**: best-kit DPS against the mob's
  HP/armor yields TTK under a per-encounter budget (generous — this is a
  sanity bound, not a balance opinion);
- **incoming damage is survivable-with-play**: no single unavoidable hit
  exceeds max HP (one-shots must be dodgeable — the saturation/telegraph
  rules of spec-0022/0016 already govern avoidability), and the kit's
  sustain (food/potions) is nonzero where the fight budget exceeds base
  regen;
- victory is **wired**: the kill objective/flag chain from "last hostile
  dead" to the next node exists (existing flow proofs).

All arithmetic states the difficulty formula it used (the Easy halving
`min(dmg/2+1, dmg)` trap is documented; normal/hard use their own). Failures
are build errors in a new DW block; the diagnostic shows the arithmetic so
content can retune without re-deriving the formulas.

### 3. Orchestrated full run (bot, combat-assist, labeled)

The existing critical-path bot run keeps running the WHOLE delve — every
trigger, walk, dialogue, checkpoint — but at combat encounters it runs under
**combat assist** (temporary Resistance, applied and removed by the harness,
logged loudly in the run artifact). It proves orchestration end-to-end at
the shipped difficulty without asserting fencing skill. Assist windows are
per-encounter and minimal; everything between fights runs clean.

## The inverted gate: bot as difficulty FLOOR

Unassisted bot combat stops being a gate and becomes telemetry with one
teeth-bearing rule reversed: for encounters the content marks as elite/boss
(spec-0016 vocabulary), if the UNASSISTED bot wins on its first attempt, the
ladder emits a **warning** — a fight this bot beats cold is too easy to be
called an elite in a souls delve. Advisory tier; content decides. Ordinary
encounters carry no such expectation. (Bot melee competence is telemetry
quality, not a gate.)

## Out of scope

- No difficulty-scaling of content by the compiler (content declares, the
  compiler proves).
- No attempt to model human dodge/aim skill in the winnability arithmetic —
  the bounds are deliberately coarse; taste stays with the author and the
  owner's playtest.
- PvP, multiplayer combat balance.

## Acceptance criteria

- [ ] Ladder gains the die-retry stage: for each critical-path encounter,
      ≥2 scripted deaths each followed by proven respawn → return → re-engage
      → complete; a corrupted flag or unreachable return path is a red run.
- [ ] Compile-time winnability: a campaign with an `Invulnerable` hostile on
      the critical path, an unreachable hitbox, an unbounded TTK, or an
      unavoidable lethal hit fails the build with the arithmetic shown;
      each rule has a DW code, test, and catalog entry.
- [ ] Full-run bot passes at declared difficulty with assist windows logged;
      the run artifact names every assist window (encounter id, ticks).
- [ ] Elite-marked encounter beaten by the unassisted bot first-try emits
      the floor warning; a genuinely hard one does not.
- [ ] Byte-identity: verification changes touch no shipped campaign bytes.
- [ ] compiler.md + SKILL.md updated (tooling-sync): authors see the three
      proofs and the floor warning as part of the combat authoring contract.
