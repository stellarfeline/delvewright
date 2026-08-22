# spec-0051: Optional is a claim about the proof — the mainline neither needs it nor fears it

- **Status**: Proposed
- **Question**: the engine refuses `mandatory: false` unconditionally ("every v0
  quest is on the critical path"), and its own catalog row says the surface is
  *reserved*, not rejected — so elective content cannot be expressed at all, and
  a campaign in flight has strands (fights, side routes, keyed doors) that have
  no legal spelling. The question the reservation deferred is not "what surface"
  — the field exists in stage 4 — but **what `optional` means to the machine
  completability proof**, which currently proves exactly one thing: the DAG is
  the critical path because nothing may be off it.
- **ADRs**: 0005 (two-layer validation — the static half carries the new
  proofs, the runtime half walks the new routes), 0006 (determinism), 0016
  (three-layer versioning — the acceptance is fenced)
- **Specs**: 0025 (extended — "provably completable by machine" already
  quantifies over branches; this spec makes it quantify over participation),
  0031/0032 (gates and state — producers and consumers this spec partitions),
  0012 (checkpoints — the re-rooted stranding proof this spec's leaveability
  demand generalises), 0049 (the layout binding already derives and reports the
  mandatory spine)
- **Non-goals**: objective-level optionality inside one quest; any model of
  difficulty, balance or reward value; any player-facing marking of content as
  optional; enumerating participation-order perturbations of runtime state
  (§7); any campaign's strand design.

## 1. The measured ground

Read at engine `ada73be9` — the validator, the flow/plan doctrine in
`docs/reference/compiler.md` (CI-enforced against the diagnostics catalog in
both directions), and the committed content. The instruments are named because
every claim below is a premise of the design.

1. **The refusal and the reservation.** `crates/dsl/src/validate.rs` refuses
   `mandatory: false` on every version; the catalog row reads *"Non-mandatory
   quest (`mandatory:false`), reserved until M3"*. The stage-4 field exists
   (`PlannedQuest.mandatory`), documented *"v0 requires `true`; optional quests
   are reserved."*
2. **The convergence rule occupies the same ground.** The plan validator
   requires every quest to be a transitive dependency of the `finale` — its
   message even offers *"drop `<q>` if it is not part of this delve"*. So an
   optional quest is refused twice today: once by name, once because the only
   DAG position it could occupy (off the finale's closure) is itself illegal.
3. **The proof already has a derived notion of optional.** The analyze doctrine
   states it verbatim: *"Optionality is not a DSL declaration — it is derived
   ... the mainline must be completable with zero optional participation."* The
   exported critical path is replayed as the **participation-minimal walk**,
   crediting only the mainline's own producers; a mainline objective gated on a
   flag only an off-path act sets fails the replay, and a completing button on
   screen too early is the ordering refusal. Today that derived notion reaches
   dialogue options, elective triggers, traps and shop offers — everything
   *below* quest granularity, because at quest granularity items 1–2 forbid the
   case.
4. **The skippable-root stance exists as written policy.** The `on_death[]`
   row: *"Optional in the strongest sense the completability model has — nobody
   is forced to die — so it registers `close-gate`s only, never an `open-gate`
   the proof could lean on, and nothing inside it is credited as a flag
   producer."* The `fill-region` row carries the same stance for every root
   the party can skip — a skippable firing **seals** (the proof must survive
   it) and never lays footing a forced leg may stand on.
5. **The re-rooted stranding proof exists.** A checkpoint that strands the
   party is refused by re-rooting reachability at the checkpoint cell and
   asking whether the remaining critical-path anchors are still walkable.
   Its binding is checkpoints only.
6. **The spine is already derived and reported.** `crates/dsl/src/layout.rs`
   computes `mandatory_quests` as the finale's `depends_on` closure and states
   how many layout beats sit on it; a graph with none is a called-out finding.
7. **The compatibility set is concrete.** Two committed stage-4 documents
   exist across the content trees (15 `"mandatory"` occurrences); zero carry
   `false` — structurally, since the value was always refused. The only
   `mandatory: false` in either repository is the refusal's own red fixture.

## 2. The decision

`mandatory: false` on a stage-4 quest becomes legal at the adopting
`dsl_version`, and it means exactly this to the completability proof:

**The mainline neither needs this quest nor fears it, and the quest keeps its
own promises.** Four clauses, each a proof obligation:

1. **Not needed.** The exported playthrough contains no step of an optional
   quest and credits nothing optional participation produces — no flag, no
   state write, no wave drop, no `open-gate`, no footing, no
   `quest-complete` trigger. Every guarantee the machine makes about the
   mainline is proven in the **skip world**: the ordinary replay in which no
   optional objective is ever completed, and everything mechanical still
   happens (§5).
2. **Not feared.** What optional participation *can* fire is held to the
   skippable-root stance of §1.4: its seals count against every mainline leg,
   and its opens and its footing are never credited to one. A player who
   participates, in any order, still owns a completable mainline.
3. **Own promises.** An optional quest is **enterable** (its activation chain
   fires and its first posted place is reachable from the mainline),
   **completable** (its objectives complete in some world where the player
   participates — the existing all-quest reachability fixpoint keeps ranging
   over it), and **leaveable** (from every place it posts the party, the
   remaining mainline is walkable, under the seal states reachable there).
   Marking a quest optional is therefore not an escape hatch from the proof:
   the properties it demands are exactly the ones a broken strand cannot
   supply, and a dead strand stays a refusal.
4. **Declared and verified, both directions.** The declaration partitions the
   quest set; the derivation keeps it honest. The mandatory set must equal the
   finale's dependency closure: a quest declared mandatory that the closure
   does not reach keeps today's convergence refusal (the wiring mistake stays
   a refusal instead of silently becoming optional content), and a quest
   declared optional that the closure reaches is refused (the finale cannot
   fire without it, so "optional" would be a lie). This is the
   claim-the-machine-verifies pattern the authored critical path and the
   empty `branch_points` already follow.

`optional` is a property of a quest's relationship to the completion proof —
nothing else. It is not a genre word: the same declaration is a souls elite, a
detective's red herring, a museum wing, a lore vault. Anything that **forks**
the mainline is a branch and already has its mechanism (spec-0025); anything
the mainline cannot complete without is mandatory; optional is precisely what
remains — content whose absence and whose presence the mainline both survive.

## 3. No new mechanism — the bindings that widen

The constitution's third review shape, applied before any surface: every half
of this design already exists as a general mechanism whose binding stops one
step short of quest granularity. The work is widening five bindings; a new
authoring section (a `side_quests[]`, an `optional_strands[]`) would be a
fourth mechanism strictly weaker than what is already there, and is rejected.

| Existing mechanism | Binding today | Widened to |
|---|---|---|
| Derived optionality + participation-minimal replay | every elective act below quest granularity | a quest whose `mandatory: false` puts all its acts in the elective set |
| Convergence rule (finale closure) | all quests, so the closure is everything | the **mandatory** quests exactly, cross-checked against the declaration both ways |
| Skippable-root stance (seals count; opens, footing, producers do not) | `on_death`, trap payloads, shop offers, the shortcut far side | effects rooted in an optional quest's objectives and `on_complete` |
| Re-rooted stranding proof | checkpoint cells | every place an optional quest posts the party (§6) |
| Branch-set verification, per-branch walks and waypoints | declared story branches | optional strands join the walked set (§6); optional-produced flags that select a declared branch are proven by the branch machinery unchanged |

One derivation, one authority: the spine (finale closure) is currently
computed independently by the plan validator and by the layout binding. Once
the partition carries proof weight, that computation is a single function both
read — two agreeing copies of one rule is the recorded merge defect waiting to
happen.

## 4. The DAG position of an optional quest

- The `finale` is mandatory. Refused otherwise.
- A **mandatory** quest's `depends_on` entries and its stage-5 `trigger:
  quest-complete` source are mandatory quests. Refused otherwise — a mainline
  that hangs off elective content is clause 1 violated in the plan's own
  skeleton, and it is refused there, at the edge, where the message can name
  the edge.
- An **optional** quest's `depends_on` and trigger source may name either kind.
  Optional-on-optional edges order a strand; optional-on-mandatory edges are
  the strand's attachment to the spine.
- The acyclicity, finale-declared and intra-quest ordering rules are untouched.

A **strand** — a weakly-connected component of optional quests — is a derived
reporting unit only. No new field names it; proofs quantify per quest, walks
and chronicle lines group by strand for legibility.

## 5. Skipped is not absent — the skip world, precisely

The skip world is not the campaign with optional quests deleted. A skipped
optional quest still **activates** when its trigger fires (a trigger chained
to a mandatory completion fires for every party); its quest-active score is
set; its cast entries enter the cast ladder; its goal line exists. What never
happens is **participation**: no optional objective completes, so its
`on_objective_complete` and `on_complete` bundles never fire and nothing
downstream of them exists.

The replay therefore walks the mainline **under the activated-but-uncompleted
state of every optional quest whose trigger has fired by that step**. This is
the clause that keeps the cast ladder honest: an optional quest that casts an
NPC re-points that NPC's right-click for skippers too — later-begun wins — and
if that retires the tree carrying a mainline completing option, the replay
fails at that step exactly as it fails any other unreachable option. The
ordering refusal (a completing button on screen too early) likewise evaluates
under this state; its derived participation set widens by every optional
objective with no re-derivation, because it was never a list of anything but
"acts off the exported path".

Emission owes the terminal state: `campaign-complete` with optional quests
active and incomplete is a **legal end of a delve**, and nothing emitted may
assume quest completion is universal. Activation, HUD and l10n surfaces are
unchanged — no new player-facing string exists, and whether a campaign tells
the player a thing is skippable is content, authored in the strings it
already has.

## 6. What optional content owes about itself

- **Enterable.** The strand's attachment point activates off the spine, and
  its first posted place is reachable from the mainline position at which it
  opens, gate-aware. The existing anchor-reachability and all-quest fixpoint
  proofs already range here; they keep doing so. A quest no world can enter or
  complete is refused today and stays refused — this is what makes
  `mandatory: false` an opt-out *secured by properties the defect cannot
  supply* rather than a laundering hatch.
- **Leaveable.** From **every place an optional quest posts the party** — its
  objectives' anchors and completion boxes, its effects' teleport
  destinations, its spawn and interaction cells — the remaining mainline
  anchors are walkable, judged under the seal states reachable at the steps
  where that place is live, including seals other skippable roots can have
  fired. The one-way drop into a crypt with no way back is the shape this
  refuses; the checkpoint case is already refused by the same re-rooted proof
  at its current binding, and a `set-checkpoint` inside an optional quest is
  covered by that existing binding unchanged. Where a strand *wants* a one-way
  entry, the shortcut mechanism already spells a provable loop-back.
- **Walked.** Machine-playable quantifies over participation as it already
  quantifies over branches: each strand exports a route — enter from the
  spine at its activation point, complete its objectives, return to the next
  mainline anchor — joining the per-branch waypoint and bot-walk set. Tier
  per the existing ladder: static proofs on every build, full walks where
  full walks already run.
- **Fights, structurally only.** Combat proofs whose premise is "the party
  cannot walk away" keep binding to the forced path. An optional fight is one
  the party can always walk away from — leaveability proves the retreat — so
  the threshold proofs (time-to-kill against best kit) do not reach it, and
  that is deliberate: past the structural floor, "harder than the kit
  affords" on elective content is a design choice the machine cannot
  distinguish from a mistake without a difficulty policy, and a difficulty
  policy is genre. The structural floor still binds through the strand's own
  completability: a kill objective in an optional quest still requires its
  wave to spawn, be damageable and be fought from somewhere.

## 7. What the engine deliberately does not know

**Reward weight.** If a strand grants gear the mandatory path is balanced
around, the skip case is a different difficulty curve. The engine refuses
provability defects and models no balance: which run is *harder* is the
owner's-hour question, unanswerable by machine without encoding a design
policy. What the engine owes instead is legibility, by the decompilation
principle: the analysis states the partition as a binding count (N mandatory,
M optional quests; zero optional is a plain fact, not a finding) and the
chronicle names, per strand, what participation grants — every item of which
the partition proof has already shown nothing mandatory consumes. The
reviewer judges balance in the reviewer's medium; no diagnostic does.

**Participation-ordered state arithmetic.** The proof does not enumerate the
values a datum can take under every participation order. The skip world's
arithmetic is exact; flag edges are refused at the partition (§8) because
flags are monotone and the edge is decidable; a datum is not monotone, so
"an elective debit can spend the party below a mainline price" is not decided
by this spec. That hazard exists today without optional quests — any elective
shop beside a state-priced mainline gate carries it — and is recorded as a
finding for the ledger, not parked here as a pretend obligation.

## 8. The refusals, by what they refuse

No code numbers — this spec allocates none; the implementation round consumes
them from the planner. Each refusal names the campaign shape that trips it.

1. **The finale leans on it.** An optional quest inside the finale's
   dependency closure. Trips: `finale` transitively `depends_on` a quest
   marked `mandatory: false`. Prescription in the message: mark it mandatory
   or cut the edge — never "the proof will sort it out".
2. **The mainline hangs off it.** A mandatory quest whose `depends_on` or
   `quest-complete` trigger names an optional quest. Trips: a strand's last
   quest used to open the finale act.
3. **A mainline key behind participation.** A mandatory element — objective,
   activation-chain effect, forced-leg gate — whose `requires_flags`,
   `requires_state` or `dropped_by` chain is satisfiable only through
   optional participation. Trips: the shape that surfaced this spec — a gate
   whose key drops from a wave only an optional quest spawns. Refused at the
   edge, naming the gate, the flag or item, and the optional-only producers;
   the participation-minimal replay remains the compensating stronger check
   behind it, exactly as it already backstops the negative-gate fixpoint.
   (The dual defect — a lock **nothing** opens — is already refused by the
   unknown-flag and deadlock family and is unchanged.)
4. **Participation forks the mainline undeclared.** An optional-produced flag
   in a mandatory element's `forbids_flags`, or selecting mandatory casts or
   staging, outside a declared branch point. This is the existing
   undeclared-story-fork refusal reaching the new producer set, plus its
   prescription: declare the branch point — the branch machinery then proves
   both sides — or move the producer onto the spine.
5. **A one-way door into elective content.** A place an optional quest posts
   the party from which the remaining mainline is not walkable under a
   reachable seal state. Trips: the drop-entered crypt with no return; the
   optional chamber another skippable firing can seal with the party inside.
6. **Forced footing from an elective hand.** A mainline leg whose only
   footing or only opening comes from an optional quest's firing — the
   existing skippable-root rule with its root class widened; same refusal
   family, wider denominator.
7. **The mismatch pair.** A mandatory-declared quest the closure does not
   reach keeps today's convergence refusal verbatim; below the fence,
   `mandatory: false` keeps today's reserved refusal verbatim.

## 9. Fence and compatibility

The acceptance of `mandatory: false` is fenced at the `dsl_version` the
implementation round consumes (ADR-0016); below it, the existing refusal
fires byte-for-byte as today. No committed campaign document carries the
value (§1.7 — it was always refused), so every existing document compiles
byte-identically by construction, and no adoption round is *forced*; the
campaign that surfaced the gap adopts because it wants the surface, on its
own branch, as its own proof-carrying round. The refusal fixture stays red
at its declared version forever.

Every engine surface owes a gallery element in the same PR: the landing PR
binds a gallery optional quest that exercises the partition — at least one
producer nothing mandatory consumes, one posted place with a proven return,
one strand walk in the walked set — or the coverage gate reds the unbound
unit the moment the field's acceptance lands. The mechanic's demo-level row
is queued in `docs/demo-levels.md` by the same PR.

## 10. Acceptance criteria — each stating what would make it vacuous

Assertions are in-repo and machine-checkable unless marked otherwise; none is
claimed true of the current tree — this is a Proposed spec, and these are the
implementation's gates.

1. **The fence pair.** The existing red fixture (`mandatory: false` at its
   old version) stays red with the reserved-refusal message; the same
   document at the adopting version, with the quest taken off the finale and
   a mandatory finale added, compiles green. *Vacuous if* the green half's
   quest is still the finale or still in its closure — it would red on §8.1
   or finale-mandatory, and the fence would never be the thing tested.
2. **The skip world is the exported path.** A fixture with one mandatory
   spine and one optional quest: the exported critical path contains no step
   of the optional quest, the replay is green, and the partition binding
   count states 1 optional against the 2-quest denominator. *Vacuous if* the
   optional quest has no objectives — there would be nothing to omit.
3. **A key behind participation is refused at the edge.** A mandatory
   objective gated on a flag set only in an optional quest's `on_complete`
   reds, naming gate, flag and producer; moving the `set-flag` to a mandatory
   quest greens the same document. *Vacuous if* the flag has a second,
   mandatory producer in the red half — the edge under test would not exist.
4. **Direction of dependency.** A mandatory quest triggered by
   `quest-complete` of an optional quest reds (§8.2); the reversed direction
   greens. *Vacuous if* either quest's trigger is `campaign-start`.
5. **The mismatch pair, both directions.** An optional quest in the finale's
   closure reds (§8.1); a mandatory quest outside the closure still reds with
   the existing convergence code, asserted by code. *Vacuous if* either
   fixture's quest is simultaneously in the other illegal position — one
   refusal could mask the other.
6. **Leaveability.** A strand entered over a one-way drop with no return
   route reds naming the posted place and the first unreachable mainline
   anchor; adding the return route greens the same geometry. *Vacuous if*
   the mainline itself crosses the drop — the existing stranding proof would
   red first and the new binding would never be exercised.
7. **Seals count, footing does not.** An optional quest's region fill across
   a mainline leg reds the leg's route proof; an optional fill that is the
   leg's only footing reds the forced-footing rule. *Vacuous if* the fill
   does not intersect any leg's route in the first half, or if the leg has
   independent footing in the second.
8. **Skipped is not absent.** A fixture where an auto-activated optional
   quest's cast retires the tree holding a mainline completing option reds
   the replay at that step; the same campaign with the cast entry
   `"unchanged"` greens. *Vacuous if* the optional quest casts no NPC the
   mainline talks to.
9. **Dead elective content stays dead.** An optional quest gated on a flag
   nothing produces is still refused by the existing reachability family —
   the opt-out cannot launder a broken strand. *Vacuous if* the gate is
   producible.
10. **The terminal state.** A generated PackTest completes the mainline with
    an optional quest active and incomplete: `campaign-complete` fires, every
    live command passes the shared rejection rule, and no emitted function
    errors on the incomplete quest. *Vacuous if* no optional quest is active
    at the finale step.
11. **The strand is walked.** Each optional strand exports an
    enter-complete-return route in the waypoint set, and the harness walks it
    at the tier where branch walks already run. The full bot walk is **not
    evaluable in-repo per-PR** — it binds where the ladder already runs full
    walks, and this criterion is satisfied per-PR by the exported route plus
    the static route proof, in those words.
12. **Byte-identity and coverage.** Every committed campaign and the gallery
    compile byte-identical under the implementation with the surface unused;
    the gallery binds the new unit in the landing PR; every new refusal arm
    is test-asserted per the DW-coverage convention (no code named here — the
    implementation round allocates). *Vacuous if* the gallery element's
    optional quest produces nothing and posts nothing — it would bind the
    field and exercise no proof.

## 11. Not covered

- **Objective-level optionality.** A skippable objective inside a mandatory
  quest is expressed by splitting the quest; a second optionality granularity
  would be a second mechanism for the same meaning.
- **Difficulty, balance, reward value** — §7, deliberately unmodelled.
- **Participation-ordered state arithmetic** — §7; pre-existing, recorded as
  a ledger finding, not solved or pretended-solved here.
- **Player-facing presentation** of optionality — content's job, in strings
  content already owns.
- **Cross-strand order enumeration** beyond the conservative stances stated
  (seal union against the mainline and against leaveability); two strands
  whose interaction is a designed exclusivity declare a branch point, which
  is the mechanism that proves such things.

## 12. Order of work

1. The partition and DAG rules (§4, §8.1–2, §8.7) with the one-authority
   spine function; fixtures of criteria 1, 4, 5 in the same PR.
2. The flow/replay widening (§5, §8.3–4) and the skippable-root class
   widening (§8.6); criteria 2, 3, 7, 8, 9.
3. The leaveability binding (§6, §8.5); criterion 6.
4. The walked-strand export and terminal-state emission; criteria 10–11.
5. Docs in the same PRs per the tooling-sync rule: `compiler.md` stage-4/
   analyze/nav rows, the diagnostics catalog, the gallery element and the
   demo-level row (criterion 12).
