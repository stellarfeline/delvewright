# Playtest methodology — how an owner round is run

The owner's playtest hour is the scarcest resource in the pipeline. This document
records what 22 rounds on `nobodys-cave-island` (2026-08-01 → 2026-08-06) actually
taught about spending it, derived from that campaign's findings ledger and round
records (content repo, `campaigns/nobodys-cave-island/GENERATION.md`).

Audience: agents. It governs every campaign iteration round; `/new-delve` carries
its mandatory steps inline.

## The measured record

Owner findings per playtest, by the round they were **first** reported in (the
ledger's `Reported` column; a re-report of an already-open finding is not counted
again):

| r3 | r4 | r5 | r7 | r8 | r11 | r12 | r13 | r14 | r15 | r16–r22 |
|----|----|----|----|----|-----|-----|-----|-----|-----|---------|
| 8  | 9  | 4  | 5  | 4  | 2   | 9   | 7   | 1   | 3   | **0**   |

Three eras, each ended by giving the machine a model of a dimension it previously
could not see — not by fixing more carefully:

1. **r3–r6, missing primitives.** Cutscenes could not aim; strikes did not
   register; text overran its box; night vision was a renamed water bottle; the
   ocean sat below the shoreline; singleplayer had no entry point. Ended by new
   DSL verbs and the diagnostics that guard them.
2. **r7–r11, no spatial model.** Walls, seams, a giant standing inside the
   mountain, sheep scattered across a cavern. Ended by geometry proofs
   (`DW0450`/`DW0451`/`DW0359`), the world-edit loop, and render review.
3. **r12–r15, no narrative-structure model.** Premise questions offered after the
   finale; beats armed before their prompt could be read; one ending for three
   names; a half-built branch. Ended by the cast ledger (r13, spec-0020),
   happenings (r18), branch points (r19), actor tiers (r20).

The r12/r13 spike is not a regression: it is the moment a *new dimension* became
visible to the owner, before the machine could see it at all.

Engine-first root-causing was NOT the differentiator — round 3 alone produced
eight engine PRs, and the "fix the twin, not just the instance the owner stood
on" lesson was already written down in round 9. Both were in place while the
churn continued.

## Rule 1 — a green gate that binds to nothing must report VACUOUS, not pass

Most of the early "green" was vacuous. Three distinct ways this happens, all
observed on the island:

- **Unbound** — the gate ran and matched zero objects. Before round 20 added one
  `actors[].tier` field, `validation/combat-plan.json` had `floor_gate.covered`,
  `floor_gate.not_covered` **and** `actors[]` all empty: the bot's combat floor
  gate examined zero enemies for nineteen rounds and was green every time.
- **Unfenced** — the campaign's `dsl_version` had not reached the surface the gate
  keys off, so the whole proof was inert. Branch reachability, the chronicle and
  the six branch proofs did not exist for this campaign until round 19 declared
  `branch_points`. "All four branch runs green" was physically impossible before
  then. This is the failure mode CLAUDE.md's **version-adoption discipline**
  already forbids; the island quantifies its cost.
- **Unemitted** — declared, compiled green, and never emitted. `wave/storm-shore`
  and `wave/storm-fire` silently never spawned until engine PR #280 closed a
  wave-machinery emission gap; the round-22 bot run was the first time that fight
  existed at all, for machine or human. Guarded now by the dangling-function
  check (`DW0497`: no emitted `function <ns>:<name>` may point at a function that
  was never emitted).

**Obligation.** Every validation artifact states its binding count, and a zero
binding is a finding. Reading a report is not enough — an empty coverage set and
a clean coverage set look identical to a reader who is not counting. When a
campaign has hostile bodies but no tiered actor or wave, the floor gate is
unbound; say so in the round summary rather than reporting a pass.

## Rule 2 — a finding is not closed until its general form is a diagnostic

An instance fix leaves every other instance of the same defect in the build,
waiting for the owner to hit one. Measured latency on the island:

- The owner reported clicks landing on the wrong entity at the fire pit in **r7**.
  Fixed in **r10** by moving one anchor and adding a `strike-npc` trigger — the
  instance.
- The general rule became `DW0489` ("the crosshair is a ray") in engine task #190,
  **eleven rounds later**. On its first run against the real build it immediately
  found a second instance: Antiphos at the cave mouth, separation `0.00` — which
  the owner had by then independently lost a click to.
- Same shape for `DW0205`: its test table's first row is the owner's
  muster/surf softlock verbatim, and on arrival it found **three** live instances
  in the campaign.

**Obligation.** Every owner finding produces two deliverables: the instance fix,
and the general form as a diagnostic (or a declared, justified reason none is
possible). When the diagnostic lands it is **re-run against the current build** —
that sweep is the point, not the code. A finding closed with only an instance fix
is recorded as such, and that record is a risk item at the next staging review.

## Rule 3 — declare the machine-readable structure before authoring content

The four declaration surfaces that let the machine see the story arrived at
rounds 13, 18, 19 and 20 of a 22-round campaign. Everything they proved was
unprovable before them.

**Obligation.** A new campaign declares the cast ledger, happenings, branch
points and actor/wave tiers as it authors each stage — never as a later adoption
round. `/new-delve` requires the first three outright, and the compiler fails a
build that omits one (`DW0460` / `DW0481` / `DW0480`). The fourth it can only
**ask** for: no diagnostic demands a tier, so an untiered set-piece fight
compiles green and rule 1's unbound-gate report is its whole backstop. A campaign
missing one is not a campaign whose gates mean anything.

## Rule 4 — a capability-gap finding blocks staging, not just the backlog

Every island finding that stayed open across more than one round was blocked on a
missing first-class primitive. None was a forgotten task:

| Finding | Reported → closed | Blocked on |
|---|---|---|
| Cheese must fill the room's OWN barrel, and be named | r12 → r18 | `collect` had no `container`/`item_name`/`fill_count`; the compiler stamped its own chest |
| Boulder hint should answer right-click too | r12 → engine #142 | co-located click triggers had to merge onto one hitbox |
| Wait branch: a body vanishes and walks back | r15 → PR #244 | a walk must start where ITS branch left the body (`DW0488`) |
| Ending night-vision expires and flickers | r15 → PR #246 | granted sight must outlast the camera it has to survive |

Refusing to hack these downstream was correct (CLAUDE.md, *No hacks at any
layer*); round 13 explicitly STOPped on two of them rather than shipping a
workaround. The mistake was staging builds for the owner while those rows were
still open — which is how she saw the same defect twice and, in round 16, said so.

**Obligation.** Triage every finding as *content* / *capability gap* on the day it
is reported. A capability gap is a **staging blocker**: either the engine work
lands before the next playtest, or the round summary tells the owner, per item,
that it is still open and not to test it. The full findings ledger is audited from
round 1 — never from the last round — before any build is staged.

## Rule 5 — the design record is authoritative, and unrequested changes are rejected

`DESIGN.md` went unupdated from round 3 to round 11 while the implementation
drifted; the round-12 audit that made it v2 found **seven deviations traceable to
no owner request at all**. The protocol adopted then — every round updates it,
every round ends with a conformance review, unrequested changes are forbidden —
was first enforced in round 21, when a worker's entire round was **rejected
wholesale and never merged** for carrying unrequested extras.

**Obligation.** Unrequested change is a rejection cause on its own, independent of
whether the change is good. Re-do the round from the sanctioned mechanisms.

## Rule 6 — execute an owner ruling as stated; generalizing it is a proposal

Round 16 read the owner's ruling on one beat as a campaign-wide 3–4 second
ceiling and set the blinding beat to 4 s. Her correction: that beat gets **no
pause at all** — the giant standing up blind *is* the signal. The ceiling governs
places where a reading pause exists; it was never a target.

**Obligation.** Apply a ruling at the scope it was given. If a wider rule seems
right, propose it in one line and wait — a generalization is a design decision,
not an inference to make silently.

## Rule 7 — the ledger is a machine-readable artifact, and a gate reads it

Rules 2 and 4 were written down and obeyed by hand, which meant they were obeyed
exactly as well as whoever remembered them. Both are now enforced:
`docs/playtest-findings.json` is the ledger — **every finding the owner has
reported, on any campaign, from the first M2 dress rehearsal (2026-07-30)
onward** — and `tools/staging-gate.py` refuses to stage a build while any row's
general form is not a live, binding check on THAT build.

The gate asks a question no other check in this repo asks, and it is the reason
a green ladder does not discharge the owner's standing directive (2026-08-09:
*her playtest is content QC only*). Most island findings were things **no check
existed for at the time**, so "everything is green" and "she will not find a
mechanical bug" are different claims and only the first was ever measurable.
The gate re-runs nothing. Per row it asks: does a general-form check exist, and
does it BIND — non-zero — here.

**Six reds, one per way a green has really lied**, because folding them together
would be the seventh:

| Verdict | What it means | Its real instance |
|---|---|---|
| `NO-GENERAL-FORM` | the instance was fixed, the class never built | rule 2's `DW0489`, eleven rounds late |
| `MISSING-CHECK` | the ledger names a check this engine no longer has (absent from source, undocumented, or asserted by no test) | four rows in the ledger's own first run named invariants that did not exist under those names |
| `UNBOUND` | the check matched zero objects | rule 1's floor gate, nineteen rounds |
| `INAPPLICABLE` | zero binding **and** zero precondition — the campaign declares none of the objects the class needs | the island has no trap, so no volley-saturation proof can say anything about it |
| `UNFENCED` | the campaign's `dsl_version` never reached the surface the check keys off | rule 1's branch proofs before round 19 |
| `NO-SOURCE` | the campaign has no stage JSON, so nothing can be measured | the drowned-bell remake today |

`INAPPLICABLE` is a **red**, not an exemption. The temptation is to let a row
excuse itself by declaring its own binding class as its own precondition, which
is not a gate; and "this build cannot exercise the class" is exactly what the
round summary must say rather than fold away. The `applies_when` probe names
*which* zero a zero is; it never changes the verdict.

**The one permitted non-red** is rule 2's own escape, no wider: a row may close
`DECLARED-UNCOVERABLE` with a `disposition` (`no-machine-form` / `owner-ruled`)
**and** a substantive justification. Sixteen island rows qualify and every one is
a judgement — prose register, pacing, whether a space reads as open. A bare
label buys nothing; the gate checks the justification is there and says
something. Their count is in the headline because rule 4 makes each a standing
risk item at that staging review.

**Obligation.** Every playtest APPENDS its findings to the ledger, the same day,
with the triage rule 4 requires. No build is handed to the owner until
`tools/staging-gate.py` has been run against that exact campaign and build tree,
and its red list is carried into the round summary item by item — a red is not
permission to stop, it is the list of classes she is not protected from. The
gate is deliberately **not** a CI status check: it is red today by design, and
making an honest red list blocking would force the one move CLAUDE.md forbids.
Its falsification suite is in CI instead (`tools/tests/test_staging_gate.py`).

### What this ledger is reconstructed from, and what is missing

Stated because a findings ledger that silently starts at round 12 is precisely
the defect the gate exists to prevent. Sources: the island's own 52-row ledger
and round records (`GENERATION.md`, rounds 3–22); the private notes (gitignored,
`docs/notes/private/`) — the island ledger audit and the evidence log, read end
to end — and the two session handoffs; `hollow-vigil`'s `GENERATION.md`; the bell's
records on `campaign/the-drowned-bell-r3` and `REMAKE.md`, the on-disk task
archive (`~/.claude/tasks/`, 281 cards, 2026-07-30 → 2026-08-10), and the
diagnostics catalogue in `compiler.md`, whose `(owner playtest, …)` attributions
turned out to be the single best finding→diagnostic index in the repo.

Known gaps, each a reason a row may be missing rather than closed:

- **hollow-vigil's round-1 findings are not enumerated anywhere.** Four are
  recoverable from `spec-0002`; PRs #28–#36 may hold more.
- **The island ledger audit's r17 addendum was lost** and is recorded as lost.
- **The bell's round-6 batch of eight owner findings** is carried as pending
  work with no diagnostic coverage for any of the eight; only those with a clear
  general form are in the ledger as rows.
- **`the-wake` and `the-toll-road` were never staged for the owner**, so their
  records hold worker findings only — deliberately not in this ledger.
- The record contained one **wrong DW number** for a real finding (the
  branch-aware walk origin was written as `DW0486`, which is a different rule;
  it is `DW0488`). The catalogue is authoritative over any narrative log, and a
  ledger row citing a code must be checked against it — the gate reports the
  mismatch as `MISSING-CHECK` only when the cited code is absent entirely, so a
  wrong-but-existing code passes silently. That is the gate's own known blind
  spot.

## Why the final round was clean

Not because one round fixed everything. Because:

1. Rounds 17–21 ran with no owner exposure at all (prose, happenings, branch
   declaration, version adoption, the terminal round). Round 16 *was* an owner
   playtest — four items, every one of them a finding she had already reported,
   which is the rebuke rule 4 comes from.
2. By r21 all four declaration surfaces had landed, so the *existing* gates
   finally had something to bite.
3. Round 21 is recorded as "red frames, in the order the machine produced them —
   every fix admitted by a red first": a campaign green for twenty rounds
   produced **eight classes of machine red in one round** (`DW0205`×3, `DW0469`,
   `DW0483`×3, `DW0489`, `DW0132`, `DW0180`/`DW0181`, `DW0310`, `DW0450`). Those
   eight are the findings the owner would otherwise have reported.
4. Round 22's bot died in the storm gauntlet — the machine making a difficulty
   judgement that had previously required a human.
5. Only then: full ledger audit, localized build, determinism, server self-check,
   and the invitation.

The owner found nothing because everything machine-findable had already been
found. That is the target state for every campaign, and rules 1–6 are how a round
gets there without spending twenty-two of her hours discovering them again.
