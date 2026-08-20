# spec-0044: A respawn that resets the scene

- **Status**: Proposed
- **Question**: `DW0478` (spec-0016 §1) asks *"does any hostile's declared
  perception radius cover the respawn cell?"* — and never the question that
  separates a soft-lock from an ordinary retry: **is the respawn cell worse
  than the beat it returns the player to?** On the project's one released
  campaign the criterion reported six violating pairs; an adjudication has now
  measured all six with two instruments whose failure modes are unrelated, and
  **all six are false positives** — every cell is strictly safer than a
  position the campaign's own critical path already requires the party to
  occupy, against the same body, at the same beat. The compiler already holds
  both sides of that comparison. This spec repairs the criterion: the proof
  measures the world the reset provably leaves, compares only instants a force
  can perceive, bounds trigger-staged onsets by their gates **and their
  bearers**, and credits a pair the campaign's own forced path dominates. The
  geometry demanded of a compared, uncredited pair does not change by one
  block.
- **ADRs**: 0006 (determinism — every fold is over declared effects, no
  simulation), 0008 (CI arbitration)
- **Specs**: 0016 §1 (amended — the safe-zone rule's comparison set and credit
  set; the clearance rule for a compared, uncredited pair is verbatim), 0012
  (checkpoints — surface untouched), 0014 (stealth — the expressible spelling
  of one refused beat), 0023 (mandatory-fight semantics — the proof the
  dominance credit is anchored to)
- **Non-goals**: any new DSL surface, field, or verb (no `dsl_version` moves);
  a wave-removal effect; line-of-sight or occlusion modelling; any per-species
  perception data; any change to `follow_range` reading or default; the
  non-raider wave-delivery capability (the reason one wave declares
  `follow_range: 48` — a separate engine question this spec neither fixes nor
  excuses); campaign content — **no released campaign is edited to satisfy
  this or any criterion**; a version fence (§8 refuses one, with the reason);
  a tier change (a recommendation rides separately at the end and is not part
  of the repair).

## 1. The measured ground, adjudicated

The six design-seat pairs were first recorded at engine `ba437282` over
content `b2ad117`. They have since been measured again by two instruments
whose failure modes are unrelated: the engine over the frozen release tree
(`release/nobodys-cave-island/v1.1.0`), and the coordinates the **shipped
datapack itself contains** — emitted by the release's own pinned engine
`91a9a843` (`delvec` v1.1.0: a different revision, a different tree, its own
build). The two agree on every distance to 0.000 blocks. **All six pairs are
false positives; nothing is wrong with the shipped campaign.**

1. `checkpoint-1 × polyphemus-roused`, 9.49/16 — every staging trigger is
   flag-gated, and the flag that would arm one is set fourteen effects before
   the **same bundle** re-seats the spawn to checkpoint 2. No tick exists on
   which the force is live while checkpoint 1 reigns. Red because onsets root
   a trigger-staged force at step 0 and never read `requires_flags`.
2. `checkpoint-2 × polyphemus-roused`, 5.00/16 — an unconditional `on_respawn`
   despawns the body before the player arrives. The proof never reads
   `on_respawn`.
3. `checkpoint-3 × wave/storm-shore`, 28.79/48 — the forced path walks the
   party straight through the wave's own seated cells; the respawn returns
   them 28.79 blocks away from a beat the path opens at contact range.
4. `checkpoint-3 × polyphemus-walker`, 14.77/16 — the compared body is
   `NoAI:1b, Invulnerable:1b, Silent:1b` for every instant the comparison
   covers; it gains AI only when the party's own forced approach fires a
   proximity trigger inside the trigger's declared range of the body. Red
   because force-hood is decided from `unleash-actor` while the force is
   *dated* from `spawn-actor` — a deliberate, documented split the proof
   cannot see across.
5. `checkpoint-3 × polyphemus-roused`, 8.94/16 — the staging triggers' bearer
   is killed in the same bundle that seats checkpoint 3, so nothing can stage
   that force again, ever. Both gate flags are long since set, so a flag rule
   alone does not catch it.
6. `checkpoint-3 × polyphemus-blinded`, 6.40/16 — the geometry is accurate and
   the pair is still false: the campaign's own forced critical path opens the
   identical encounter at 2.00 blocks, the body unleashed in the same tick
   with no head start. The respawn hands the player the same actor, freshly
   re-staged with no target, at 3.2× the distance. If that loop were
   unwinnable, the mandatory beat would be unwinnable and the campaign
   uncompletable.

Two more measured facts about the criterion itself. On content main the
compromise build is green by **0.12 blocks** against a 16.0 radius — a seat
moved five to nineteen blocks, changing nothing a player experiences, flipped
the verdict: the flagship's green rests on placement noise, not on safety.
And enumerating the six violations at all required a patched binary, because
the diagnostic returns at the first pair — which is itself the argument for §5.

## 2. What a red honestly claims

An error condemning six placements and right about none was claiming too
much. Whether a retry loop is *winnable* from a seat is not decidable from
declarations — it is a combat question, and the compiler refuses to simulate
combat (ADR-0006). What **is** decidable is whether the campaign supplies
evidence that the retry is no worse than play the party already owes. So the
amended criterion's claim shrinks to what it can prove: **red asserts that
nothing the campaign declares separates this respawn from a soft-lock** — and
it enumerates the evidence it accepts, each a fact the defect cannot supply
(§7). The diagnostic's message is reworded to match: it names the pair, the
distance, and the three evidence routes (the reset removes or re-places the
force; its staging cannot meet the reign; a forced beat already dominates
it), and prescribes those before it prescribes moving anything. The
prescription never includes shrinking `follow_range`, exactly as before.

This reframing is what makes the criterion repairable. A criterion that kept
asserting "this is a soft-lock" would be unrepairable, having been measured
wrong on six of six instances; a criterion that demands declared evidence is
a design demand, and a design demand can be met.

## 3. Decision — the proof measures the world the reset leaves, at emission order

For each compared pair (respawn point R, force F), the cells and state of F
are those of the **post-reset world**: the fold, in emitted order, of the
**unconditional** effects of R's own `on_respawn` (and, for a bonfire, its
re-seat plus `on_rest`) over F's state at R's reign.

- An unconditional `despawn-actor F` with no later re-stage: F has no cells;
  the pair is **credited**, with the effect and resulting state as the reason.
- A re-stage (`despawn-actor F` then `spawn-actor F`, with or without an
  `unleash-actor F`): F is measured at its re-staged cells **in its re-staged
  state** — the same seated-cell resolution the proof already uses. A
  re-stage inside the radius is compared there; its verdict then passes
  through §6, never around it.
- An effect carrying `requires_flags` / `forbids_flags` is **never** credited:
  the post-reset world must hold in every state a death can occur in.
- Waves at a plain checkpoint are unaffected: no verb removes a wave. A
  bonfire's re-seat returns waves to their initial stations, which is the
  state the proof measures — unchanged, now by this rule instead of by
  coincidence.
- **The fold is ordered at emission granularity**: line order within a bundle,
  step order across bundles. Ground pair 1 is why this is stated: an arming
  and a reign end emitted in one bundle are ordered facts, and a trigger armed
  at or after the line that ends a reign never meets that reign — the bundle
  runs in one tick, and no firing can land between two lines of one function.
- **The world at R's reign start** is likewise a fold: the ordered
  unconditional effects of the critical-path prefix up to and including R's
  seating bundle. A force that prefix unconditionally removes, and that
  cannot be staged again during the reign (§4a), has no cells against R.

The clearance rule for whatever a compared, uncredited pair contains is
spec-0016 §1 verbatim: distance must exceed `follow_range` (plus lane drift
for lane path cells). Nothing about the demand moves.

## 4. Decision — perception is compared, and an onset is bounded by gates and bearer

Two bounds, both narrowing *what is compared*, never what is demanded of a
compared pair. Every skip states its bound in the ledger.

**(a) A trigger's onset is bounded by its flags and by its bearer.** A force
staged only from a trigger with `requires_flags` cannot exist before the
earliest instant all its required flags can be set — resolved recursively
over each flag's producers, at §3's emission granularity, with **0 on any
cycle or unresolvable producer**; `forbids_flags` never widens the bound.
And a trigger keyed to an entity (`strike-npc`, an actor's own hitbox)
structurally cannot fire without its bearer: the staging window **closes** at
the bearer's unconditional removal. A pair is skipped on the bearer bound
only when the whole obligation holds: every staging trigger's bearer is
unconditionally removed at or before R's seat, the bearer is never re-staged
during the reign, and no instance staged *before* the reign survives into it
on any route — where a route includes every death route, each resolved
through the then-reigning respawn point's own §3 fold. Where any part cannot
be established, the pair is compared; where the pair is skipped, the ledger
names the bearer, its removal site, and the route closure.

**(b) A puppet is not a perceiver.** The proof cannot currently tell a puppet
from a warden: force-hood is decided from `unleash-actor` while the force is
dated from `spawn-actor` — a deliberate split (an unleash is not an onset,
because it summons nothing) that leaves the compared body possibly `NoAI` for
the whole interval compared. The repair keys the capability to the object:
a force is compared only over instants it is **staged and can acquire a
target**. For an actor staged as a puppet, the perception onset is
`max(staging onset, unleash bound)`, where the unleash bound is resolved by
the same machinery as (a) — a step-rooted unleash at its step, a flag-gated
one at its flag bound, a **proximity-triggered** one at the earliest
critical-path entry into the trigger's own declared region, and 0 whenever
nothing resolves. The 0 fallback makes this strictly narrowing: `max(s, 0)`
is today's answer, so no pair today skipped is widened, and the documented
reason `unleash-actor` was excluded from onsets (a proximity unleash dragging
a five-quests-away body onto step 0) is preserved by construction.

## 5. Decision — the red states every pair, and the ledger states every credit

One build reports **every** violating pair (same code, same tier, same
first-pair message shape, then the full list). `respawn-safety.json` gains a
per-rest-point `credited` list — `(force, kind, reason, post-reset state)` —
beside `compared` and `skipped`, where `kind` is one of `reset`, `dominated`
and a skip's is one of `onset`, `flag-bound`, `bearer-bound`, `puppet`,
`reign` — **computed from the object, never selected by the author** — so a
credit is as auditable as a skip and a zero binding is as loud as before.
`docs/reference/compiler.md`'s diagnostic row and artifact row update in the
same PR.

## 6. Decision — a forced beat dominates the respawn it precedes

This is the load-bearing repair, and it is the general defect's negation
made a rule. A pair (R, F) still red after §3–§4 is **credited** when the
campaign's forced critical path contains a step, **inside R's own reign
window** — the very segment a death at R re-walks — at which the party must
stand at distance ≤ `dist(R, F's measured cells)` from the same force, in a
state no more advantaged than the state F holds at R's arrival (per §3's
fold). The retry then delivers an encounter the path already delivers
no-more-gently: same body, no closer, no angrier.

Bounds, each stated so the credit cannot drift:

- **Same force, by id.** Never same species; no per-species table exists or
  is invented (`DW0475`'s rule).
- **Stationary cells only.** Seated spawn cells, staging anchors, re-staged
  cells. A lane wave's smeared march corridor **never dominates**: the
  corridor is every cell the squad sweeps over time, and the path crossing it
  is not a proven meeting — crediting on it would re-ship the criterion's
  own motivating death (a re-seated lane squad killing the party beside the
  fire it had just rested at). Lane cells stay red-side only.
- **State is emitted state.** Cells, count, puppet-hood, aggro lock — read
  from the same bytes both sides of the comparison already use. Runtime
  drift (a body pursued off its station, accumulated anger) is out of model
  on both sides symmetrically, exactly as the base criterion already holds
  (it measures seated cells, and terrain is out of model in both directions —
  the 28.79-blocks-through-rock red was the proof). At a bonfire the
  stationed and undefeated re-seats make the emitted state the arrival state
  by construction; at a plain checkpoint it is the criterion's standing
  model, unchanged here.
- **The dominating step must fall inside R's reign.** A close encounter in a
  different reign proves nothing about this retry.

The credit's ledger entry names the dominating step, both distances, and the
state comparison.

What anchors the credit is the oldest invariant the product has: a delve is
**provably completable by machine** before it ships, and spec-0023 makes
every fight the path forces a proven fight. A respawn dominated by a forced
beat can only be a soft-lock if that beat is unwinnable — which is the
campaign being uncompletable, which the machine playthrough refuses on
evidence (a finished run) no defect can supply. The credit therefore adds no
new trust; it re-uses the one this project is founded on, and it is exactly
why "the compiler already holds both sides of the comparison" is true.

## 7. The opt-out analysis

Each credit or skip, what it demands, and why the defect cannot supply it.
The effective obligation is the disjunction of these kinds, the kind is
computed from the object (§5), and the weakest member — dominance — carries
its model boundary in §6 rather than in an author's choice.

1. **Reset credit** (§3): demands the force be absent, or stand re-placed,
   in the emitted post-reset world. The defect is "F perceives the arrival";
   the credit is "F is not there at the arrival" — contradictory facts about
   the same emitted bytes. Conditional effects are never credited, so "true
   in the author's head, false in some flag state" has no route in.
2. **Flag bound** (§4a): demands the staging be unarmable until the reign has
   ended, read from emission order. A force that meets the reign cannot
   supply a gate that provably kept it out of the reign. Cycles and
   unresolvables fall to 0 — the conservative direction.
3. **Bearer bound** (§4a): demands the staging trigger's bearer be gone and
   every route closed. Vanilla structurally cannot fire an entity-keyed
   trigger with no entity; a force existing during the reign cannot supply a
   world in which it could not have been staged.
4. **Puppet bound** (§4b): demands `NoAI` at every compared instant, read
   from the staged bytes, with the unleash bound resolved conservatively. A
   body that acquires the arriving player as a target cannot supply
   puppet-hood at that instant — vanilla enforces the contradiction.
5. **Dominance** (§6): demands a forced, in-reign, no-farther, no-fresher
   meeting with the same force. A soft-locked respawn dominated by such a
   beat entails an unwinnable mandatory beat, i.e. an uncompletable campaign
   — refused by the machine-playthrough gate on evidence the defect cannot
   produce. The one honest trade inside it: an author may seat a mandatory
   fight close on purpose and thereby buy nearby seats — and that is not a
   hatch, because the close fight is itself proven winnable and played, the
   loudest possible form of the claim.
6. **Despawn with no re-stage purely to silence a pair** — credited, and
   rightly: the body is genuinely gone on every retry. A real design
   decision with loud consequences (the encounter is deleted on first death;
   a kill objective over it still answers the liveness proofs), visible in
   play and in the credit reason — the opposite of an invisible hatch.
7. **The arrival-tick window**: the reset runs in the arriving player's
   first live tick, so a reset-credited force coexists with the player for
   less than one tick. Whether that window admits contact is a fact about
   the pinned server and is **measured** (criterion 9). If the measurement
   finds contact, the reset credit is withheld entirely — never weakened
   into a margin.

## 8. Classification, and the fence refused

`DW0478` binds `EveryVersion`, on a two-part argument. The first half — the
rule asks for nothing to be written, and this amendment moves verdicts only
green-ward, so no campaign reds on unchanged documents *by this change* —
survives and now carries the classification alone. The second half — *"a
campaign that trips it was always soft-locked"* — is **measured false on six
of six instances** and is withdrawn as justification. What that does to the
classification: `EveryVersion` remains correct, but it is no longer a claim
about the tripped campaign; it is a claim about the rule's inputs and the
amendment's direction, and the spec says so where the next reader will look.

A fence is refused, with the reason stated: fencing the criterion would
grandfather exactly the six false verdicts — old campaigns keeping wrong
reds waived at their declared versions — while every future campaign faced
the unrepaired criterion. That treats a correctness defect as a
compatibility question. The repair is the criterion; nothing is fenced.

The 0.12-block fact lands here too: under the amendment the flagship's
verdict rests on structural evidence — a flag bound, a bearer bound, a reset,
a dominating beat — and not on a margin a cosmetic seat-move flips. A
criterion whose verdict cannot be toggled by 0.12 blocks of nothing is the
point.

## 9. The island, demonstrated — and the revert answered

Re-run on the island's design-seat probe (the two anchor edits: checkpoint 3
at `anchor/checkpoint-3`, `wave/storm-shore` at `anchor/mountain-foot`), the
amended proof must yield exactly, with these ledger kinds:

- `checkpoint-1 × roused` — **skipped, flag-bound**: the arming line precedes
  the reign-ending re-seat inside one bundle; no tick admits a firing.
- `checkpoint-2 × roused` — **credited, reset**: unconditional despawn.
- `checkpoint-3 × storm-shore` — **credited, dominated**: the forced path
  crosses the wave's seated cells inside checkpoint 3's reign; 28.79 ≥ ~0.
- `checkpoint-3 × walker` — **credited, dominated**: every perceiving instant
  begins at the forced proximity beat that unleashes it, inside the trigger's
  own declared range of the body's cells; 14.77 ≥ that range.
- `checkpoint-3 × roused` — **skipped, bearer-bound**: every staging trigger's
  bearer is unconditionally removed by the seating bundle (and by each
  trigger's own firing), never re-staged; every death route before the seat
  resolves through a fold that removes the force.
- `checkpoint-3 × blinded` — **credited, dominated** over the post-reset
  state: the reset re-stages the body fresh at 6.40 blocks; the path's own
  beat inside the same reign opens the identical encounter at 2.00, unleashed
  the same tick.

Zero red. All three design seats return; no refusal costs a placement; and —
answering the standing question explicitly — **reverting the released
campaign's compromise edit does not re-red the build under this criterion**,
because the design-seat probe *is* that revert. Whether the revert happens is
a plan question outside this spec; the criterion no longer has a vote against
it. Noted for the record: the prior revision of this spec still owed the
flagship a content edit to clear pair 5 — an edit the released-campaign rule
forbids — which is itself evidence the prior cure was incomplete.

## 10. The refusals, each with its reason

- **A bundle's existence is never credited.** Only its computed,
  unconditional post-reset state is.
- **No declared "blind" / reduced-perception property, on any object class.**
  The engine's one perception number is `follow_range`; a property whose
  emitted consequence vanilla does not enforce would be a paper claim the
  proof believes. The expressible spelling of the blind-warden beat exists
  and is proven: seat the respawn beyond the documented radius, inside a
  zone of the live stealth session (spec-0014's onset proof).
- **No line-of-sight or occlusion credit.** *Which* perceivers require sight
  is a per-species fact vanilla publishes no data for, and the compiler does
  not invent per-species tables. Unchanged — and no longer load-bearing: the
  through-rock pair this refusal used to condemn is credited by §6 on
  grounds that need no sight model, so the refusal now costs nothing.
- **No winnability simulation.** The criterion never decides whether a
  compared, uncredited pair is truly a soft-lock; it decides that no declared
  evidence separates it from one (§2). That residue is genuinely undecidable
  from declarations, and this spec says so rather than manufacturing a proof.

## Acceptance criteria

Each criterion is a test asserting the diagnostic's code or the ledger's
content, and each names its vacuous reading.

1. **Reset credit, red→green.** A fixture with a checkpoint strictly inside
   an unleashed actor's radius, whose `on_respawn` unconditionally despawns
   it: red on the pre-amendment engine, green after — and the ledger records
   the pair as `credited` kind `reset` with the despawn named. *Vacuous
   when*: green via any skip instead of a credit — the assertion reads the
   `credited` entry, never just the exit code.
2. **A conditional effect is never credited.** The same fixture with
   `requires_flags` on the despawn stays red, naming the same pair. *Vacuous
   when*: red for a different first pair — pair identity is asserted.
3. **A re-stage is measured where and as it lands.** Despawn + re-stage at an
   out-of-range anchor: green, re-staged cell in the reason. Despawn +
   re-stage inside the radius with no dominating beat: red. The same in-range
   re-stage with a forced in-reign beat at a shorter distance: green,
   `credited` kind `dominated`, the step named. *Vacuous when*: green because
   the body stopped being a fighter — the actor is asserted present in the
   ledger's hostiles.
4. **The flag bound, three directions.** (i) A force staged only by a trigger
   gated on a flag first settable at step s, against a checkpoint whose reign
   ends at e ≤ s: skipped, the flag and bound in the reason. (ii) A twin
   whose arming line precedes the reign-ending line **inside one bundle**:
   still skipped — no tick admits a firing between two lines of one function.
   (iii) A twin whose flag is settable only from another gated trigger (a
   cycle): onset 0, **compared**. *Vacuous when*: a generic reason (must name
   the flag), or the cycle twin skipped (the fallback must be conservative).
5. **The bearer bound, both directions.** A force staged only by an
   entity-keyed trigger whose bearer is unconditionally removed by the
   seating prefix, never re-staged, with no staging surviving into the reign:
   skipped, bearer and removal named. Twins: a conditional bearer removal,
   and a prior staged instance no fold removes — both **compared**. *Vacuous
   when*: the skip fires on the surviving-instance twin — route closure is
   the assertion.
6. **The puppet bound, both directions.** A staged `NoAI` actor whose unleash
   resolves to a beat after the reign ends: skipped, the puppet state and the
   unleash bound named. A twin whose unleash nothing resolves: onset 0,
   **compared**. *Vacuous when*: the skip reads the actor's declaration kind
   instead of the staged bytes, or the unresolvable twin is skipped.
7. **Dominance, three directions.** (i) A mandatory path step inside R's
   reign through a stationary force's cells, nearer than the seat: credited,
   kind `dominated`, step and both distances in the reason. (ii) A twin whose
   close step lies in a different checkpoint's reign: red. (iii) A lane twin
   where the path crosses the smeared march corridor: **red** — lane cells
   never dominate; this pins the criterion's own motivating death. *Vacuous
   when*: the credit fires on either twin.
8. **Every violating pair in one build.** A fixture with two violating pairs
   across distinct forces emits both in one run; both pair identities
   asserted. *Vacuous when*: the violations collapse into one emission.
9. **The arrival window, measured on the pinned server.** A live fixture
   (PackTest tier): a hostile seated strictly inside its radius from a
   checkpoint whose bundle despawns it; a control run first proves the same
   body damages a player when *not* despawned; the credit run asserts the
   body is gone and the respawned player takes zero damage. *Vacuous when*:
   the control is absent. A failed measurement withdraws the reset credit
   entirely; it is a precondition, not a tunable.
10. **The island demonstration.** §9 re-run verbatim against the design-seat
    probe: the exact kind per pair as stated there, zero violations,
    byte-anchored to the two probe edits. *Vacuous when*: any pair reaches
    its verdict by geometry margin instead of the named kind — kinds are
    asserted, not exit codes.

## Recommendation (separate — not part of the repair, and the owner's call)

The adjudication observes that a criterion condemning six placements and
right about none has the failure profile of an analysis-tier lint. That
observation is correct **about the unrepaired criterion**, whose only
released measurement scored zero of six. This spec's position: with the
repair and its fixtures landed, error tier stands — a pair red under the
amended criterion is one the campaign supplies no evidence for, all three
evidence routes are first-class and auditable, and the demand on such a pair
is the same demand as before, so the tier's original justification holds
again where it holds at all. If the repair is rejected, the criterion must
not keep error tier on its record; that choice — repair, demote, or both —
is a check-weakening question and belongs to the owner, never to this spec.
