# spec-0044: A respawn that resets the scene

- **Status**: Proposed
- **Question**: the respawn safe-zone proof (spec-0016 §1) judges geometry
  against the world **as staged**, and never reads the checkpoint's own
  `on_respawn` bundle — the DSL's first-class answer to exactly the hazard the
  proof guards. On `nobodys-cave-island` a checkpoint whose bundle despawns all
  four giant bodies before contact is possible was still counted against one of
  them, and clearing the proof cost the campaign its designed seats: all three
  checkpoints now share one alcove. This spec makes the proof measure the world
  the reset **provably leaves**, bounds trigger-staged onsets by their own flag
  gates, and names — as refusals, with reasons — everything it deliberately
  does not credit. The geometry demanded of a compared pair does not change by
  one block.
- **ADRs**: 0006 (determinism — the post-reset world is a fold over declared
  effects, no simulation), 0008 (CI arbitration)
- **Specs**: 0016 §1 (amended — the safe-zone rule's comparison set; the
  clearance rule is verbatim), 0012 (checkpoints — surface untouched), 0014
  (stealth — cited as the expressible spelling of one refused beat)
- **Non-goals**: any new DSL surface, field, or verb (no `dsl_version` moves;
  the held `0.12.0` belongs to its holder); a wave-removal effect; line-of-sight
  or occlusion modelling; any per-species perception data; any change to
  `follow_range` reading or default; the non-raider wave-delivery capability
  (the reason one wave declares `follow_range: 48` — a separate engine
  question this spec neither fixes nor excuses); campaign content.

## 1. The measured ground

Instruments: `delvec` at engine `ba437282`; content `b2ad117` (the island's
files are identical at `f28f153`, which lags only in `demos/`). Every claim
below was re-demonstrated on that pair before this document was written.

- The released island builds **green**: exit 0, `respawn-safety.json` reports
  3 examined, 8 pairs, 0 violations. It is green because a prior content fix
  seated all three checkpoints on `anchor/alcove-3` and moved `wave/storm-shore`
  off the mountain foot — the whole-campaign compromise this spec exists to
  make unnecessary where it can, and to justify where it cannot.
- Restoring the two design-intent placements (checkpoint 3 at
  `anchor/checkpoint-3`, `wave/storm-shore` at `anchor/mountain-foot`) reds the
  safe zone at the first pair: seat `[14, 69, -60]`, wave seated cell
  `[11, 63, -32]`, 28.8 blocks inside a 48.0 radius. The remaining design-seat
  pairs, by the same probe and by arithmetic over the resolved anchors:
  walker 14.8/16, roused 8.9/16, blinded 6.4/16 against checkpoint 3;
  roused 9.5/16 against checkpoint 1's design seat, 5.0/16 against
  checkpoint 2's. Zero anchors on the island clear all six hostile sources;
  the only anchors anywhere that do are the departure galley's deck, ~250
  blocks offshore. The legal placement set for a last checkpoint at design
  positions is empty.
- The proof never reads `on_respawn`: its inputs are checkpoint positions,
  onsets, and staged aggro sources. Checkpoint 2's bundle unconditionally
  despawns all four warden bodies; the pair was counted anyway.
- The respawn dispatch is edge-held until the player is **alive again**: the
  scene reset and the re-seat run in the arriving player's first live tick.
  The residual window between rematerialisation and that tick is the subject
  of acceptance criterion 6, which is a live measurement, not an argument.
- The diagnostic returns at the **first** violating pair; a campaign's
  violation count is invisible from one build (the content round needed a
  radius-shrinking probe tree to enumerate six).
- Force onsets root every trigger-staged body at step 0, ignoring the
  trigger's own `requires_flags`. The roused warden is staged only by triggers
  gated on `flag/sealed` / `flag/asleep`, both first settable at or after the
  step where checkpoint 1's reign ends — yet it is compared against
  checkpoint 1.

## 2. Decision — the proof measures the world the reset leaves

For each compared pair (respawn point R, force F), the occupied cells of F are
those of the **post-reset world**: the fold, in bundle order, of the
**unconditional** staging effects of R's own `on_respawn` (and, for a bonfire,
its re-seat plus `on_rest`) over F's staged state.

- An unconditional `despawn-actor F` with no later re-stage: F has no cells;
  the pair is recorded in the ledger as **credited**, with the effect and the
  resulting state as the reason.
- A re-stage (`despawn-actor F` then `spawn-actor F`, or a bare `spawn-wave`):
  F is measured at its re-staged cells — the same seated-cell / staging-anchor
  resolution the proof already uses. Re-staging inside the radius stays red.
- An effect carrying `requires_flags` / `forbids_flags` is **never** credited:
  the post-reset world must hold in every state a death can occur in.
- Waves at a plain checkpoint are unaffected: no verb removes a wave, so no
  wave pair can be credited. A bonfire's re-seat already returns waves to
  their initial stations, which is the state the proof measures today —
  unchanged, now by this rule instead of by coincidence.
- The clearance rule for whatever the post-reset world contains is spec-0016
  §1 verbatim: distance must exceed `follow_range` (plus lane drift for lane
  path cells). Nothing about the demand moves.

This is the binding question of CLAUDE.md's third review shape answered for
this proof: the general mechanism — one effect walk, one staging semantics,
one seated-cell resolution — already exists and already computes everything
the credit needs; the proof's binding simply stopped at the staged world and
never entered the one bundle that runs at the exact moment the proof is about.

## 3. Decision — a trigger's flag gate bounds its onset

A force staged only from a trigger with `requires_flags` cannot exist before
the earliest critical-path step at which all its required flags can be set.
Its onset becomes the maximum over required flags of the minimum step of that
flag's producers — where a producer is itself step-less (another gated
trigger, a trap payload, a death bundle), its contribution is resolved by the
same rule, with **0 on any cycle or unresolvable producer**. `forbids_flags`
never widens the bound. The fallback in every doubtful direction is 0 — the
existing conservative answer.

Like the reign window this joins, it narrows **what is compared**, never what
is demanded of a compared pair, and every skip states its reason (the flag and
the step bound) in the ledger.

## 4. Decision — the red states every pair, and the ledger states every credit

One build reports **every** violating pair (same code, same tier, same
first-pair message shape, then the full list). `respawn-safety.json` gains a
per-rest-point `credited` list — `(force, reason, post-reset state)` — beside
`compared` and `skipped`, so a credit is as auditable as a skip and a zero is
as loud as before. `docs/reference/compiler.md`'s diagnostic row and artifact
row update in the same PR.

## 5. The refusals, each with its reason

- **A bundle's existence is never credited.** Only its computed, unconditional
  post-reset state is. A script racing a warden's swing is not a proof; a
  world in which the warden does not exist is.
- **No declared "blind" / reduced-perception property, on any object class.**
  The engine's one perception number is `follow_range` (declared, else the
  documented default), and its own prescription forbids shrinking it to buy
  clearance. A property whose emitted consequence vanilla does not enforce
  would be a paper claim the proof believes — and a warden's real
  vibration-based senses are species folklore vanilla publishes no data for.
  A body whose fiction is that it cannot see is therefore the **same object
  class with no new property**: until vanilla provides a perception primitive,
  "the party respawns inside the blind warden's documented radius" is
  inexpressible, deliberately. The expressible spelling of that beat already
  exists and is already proven: seat the respawn beyond the documented radius,
  inside a zone of the live stealth session — spec-0014's onset proof already
  guarantees respawn-into-session cover in grace time, and the island's
  current seat is literally one of the escape session's declared zones.
- **No line-of-sight or occlusion credit.** The nav world could compute it,
  but *which* perceivers require sight is a per-species fact (a drowned does;
  a warden does not), and the compiler does not invent per-species tables.
  The 28.8-blocks-through-rock pair stays red, answered by placement or by
  retiring the oversized radius when the wave-delivery capability it
  substitutes for exists.

## 6. The opt-out analysis

What Decision 2 demands of an author who wants a pair credited: **change the
shipped post-respawn world so the force is absent, or stands out of range, on
the arrival tick** — established from the same emitted commands the datapack
runs, not from a claim.

Could a mis-placed checkpoint itself produce that demand? The sealed-region
failure was an opt-out whose proof obligation (unreachability) was *entailed*
by the defect, so the defect always qualified. Here the relation is inverted:
the defect is "F perceives the arrival cell when the party arrives", and the
credit demands "F is not there when the party arrives". The two cannot be true
of the same shipped world — supplying the credit **is** removing the hazard,
not disguising it. The residual routes around that, each closed and each
tested:

1. *A conditional despawn* — true in the author's head, false in some flag
   state. Refused: unconditional effects only (criterion 2).
2. *Despawn, then re-stage within range* — the island's own checkpoint 3 does
   exactly this. The post-reset fold measures the re-staged cell: still red
   (criterion 3).
3. *The arrival-tick window* — the reset runs in the first live tick, so the
   force coexists with the arriving player for less than one tick. Whether
   that window admits contact is a fact about the pinned server and is
   **measured** (criterion 6). If the measurement finds contact, the credit
   is withheld entirely — geometry-only, as today — never weakened into a
   margin.
4. *Despawn with no re-stage, purely to silence the pair* — credited, and
   rightly: the body is genuinely gone on every retry. This is a real design
   decision with loud consequences (the encounter is deleted on first death;
   a kill objective over it still answers to the existing liveness proofs),
   not a paper acknowledgement. It is the one route by which an author trades
   a fight for a placement, and the trade is visible in play and in the
   ledger's credit reason — the opposite of the invisible hatch.
5. *Gating the staging trigger on a late flag* (Decision 3's opt-out) — the
   gate genuinely prevents the body existing during the earlier reign; the
   demand is again the hazard's negation. The unresolvable direction falls
   back to 0, and the cycle fixture asserts the fallback (criterion 4).

## 7. What this gives back, demonstrated

Re-run on the island's design-seat probe (the two anchor edits of §1), the
amended proof must yield exactly: checkpoint 1 × roused **skipped** (flag
bound ≥ reign end); checkpoint 2 × roused **credited** (unconditional
despawn); checkpoint 3 red against storm-shore, walker, roused, blinded. Of
those four: roused becomes creditable by the one-line despawn its own fiction
already owes (the roused body must not coexist with the blinded one); walker
is the open content finding it already was; storm-shore and blinded are §5's
two named refusals. Two of three design seats return; what does not return is
refused for a stated reason rather than compromised silently.

## Acceptance criteria

Each criterion is a test asserting the diagnostic's code or the ledger's
content, and each names its vacuous reading.

1. **Credit, red→green.** A fixture campaign with a checkpoint strictly inside
   an unleashed actor's radius, whose `on_respawn` unconditionally despawns
   it: red on the pre-amendment engine, green after — **and** the ledger
   records that (rest point, force) pair as `credited` with the despawn named.
   *Vacuous when*: green via an onset/reign skip instead of a credit — the
   assertion must read the `credited` entry, never just the exit code.
2. **A conditional effect is never credited.** The same fixture with
   `requires_flags` on the despawn stays red, and the message names the same
   pair. *Vacuous when*: red for a different first pair — the pair identity is
   asserted.
3. **A re-stage is measured where it lands.** Despawn + re-stage inside the
   radius stays red; the same re-stage at an out-of-range anchor is green with
   the re-staged cell in the credit reason. *Vacuous when*: green because the
   body stopped being a fighter — the fixture asserts the actor appears in the
   ledger's `hostiles`.
4. **The flag bound, both directions.** A force staged only by a trigger gated
   on a flag first settable at step s, against a checkpoint whose reign ends
   at e ≤ s: skipped, with the flag and the bound in the skip reason. A twin
   whose flag is settable only from another gated trigger (a cycle): onset 0,
   **compared**. *Vacuous when*: the skip reason is the generic onset text
   (must name the flag), or the cycle twin is skipped (the fallback must be
   the conservative direction).
5. **Every violating pair in one build.** A fixture with two violating pairs
   across distinct forces emits both in one run; the count is asserted.
   *Vacuous when*: the two violations collapse into one emission — both pair
   identities are asserted.
6. **The arrival window, measured on the pinned server.** A live fixture
   (PackTest tier): a hostile seated strictly inside its radius from a
   checkpoint whose bundle despawns it; a control run first proves the same
   body damages a player when *not* despawned; the credit run then asserts the
   body is gone and the respawned player takes zero damage. *Vacuous when*:
   the control is absent — a body that could never hurt anyone proves nothing
   about the window. A failed measurement withdraws Decision 2's credit
   entirely; it is a precondition, not a tunable.
7. **The island demonstration.** §7 re-run verbatim against the design-seat
   probe: the exact skip/credit/red set stated there, byte-anchored to the two
   probe edits. *Vacuous when*: the probe's anchors drift — the two edited
   lines are pinned in the fixture.

No version fence rides any of this: the amendment adds no authoring surface
and no obligation — it narrows comparisons and credits declarations that
already exist, so it binds every version, exactly as the reign window does.
