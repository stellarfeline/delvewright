# spec-0025: Branch-complete narrative verification — every branch is played, not just declared

- **Status**: Approved (this bug class must be machine-caught, never swept)
- **ADRs**: 0005 (two-layer validation — extends both layers from one critical
  path to the branch set), 0006 (determinism)
- **Depends on**: spec-0020 (cast ledger, per-branch casts), spec-0012
  (checkpoints), spec-0023 (combat proofs apply per branch)

## Problem

The validation ladder proves ONE critical path. Narrative branches — a choice
that forks who lives, three endings — are declared in the DSL, reachability-
checked as a graph, and then never played. The island round-13 defect shows the
resulting blind class end to end: the flee branch's ledger said Antiphos lives,
but the staging still belonged to the death branch — an NPC despawned himself,
another held a cave the party had left, a third mourned a man standing beside
him. Structurally: **the fork moved the ledger but never moved the bodies**,
and no check owned the gap. The bot walked the wait branch; the cast ledger
proved declarations exist, not that they hold on every branch a player can
take.

## Ruling

"Provably completable by machine" quantifies over branches, not paths: a
campaign is verified when **every reachable narrative branch** has been proven
— structurally by the compiler, playably by the bot, narratively by the
generation-time review of the compiler-assembled branch chronicle. Each layer
asserts only what it can honestly own.

**Decompilation principle**: the generation workflow is
natural language → design doc → DSL. Whether the DSL matches the design is
NOT visible to an LLM — checking would mean simulating compilation in its
head, which is unreliable by construction. So the compiler compiles the DSL
*back* into natural language (the chronicle) and the reviewer compares like
with like, NL against NL. This is the narrative instance of the doctrine the
visual loop already follows (spec-0015: snapshot/blocking-chart render the
compiled world back into the reviewer's medium); expect future verification
surfaces to take the same shape.

## Design

### 1. Branches become first-class (DSL, stage 4)

The campaign plan declares its **branch points**: the flag (or flag set) that
forks the story, where the fork opens (quest id), where branches converge (or
that they run to distinct endings). The compiler VERIFIES the declaration
covers reality: any flag that gates casts, staging effects, or quest structure
downstream of where it is set, and is not part of a declared branch point, is
an error — undeclared story fork. Enumerated branches are the product of
declared branch points, so the set is authored and small, never a combinatorial
sweep of all flags.

Additionally, every story node — quest, objective, staging effect, wave,
gate/ending, story-weight dialogue beat — REQUIRES a name and a
**`happening` declaration**: one line stating what this node does to the
story, as a structured event verb from a small vocabulary (`dies`,
`survives`, `departs`, `arrives`, `learns`, `believes`, `gains`, `loses`,
`opens`, `seals`, …) plus free text. This is the forcing function (the
spec-0020 `doing` pattern generalized from NPC presence to event flow): a
design that never got written down node by node cannot compile. The
declaration is node-local on purpose — there is NO parallel per-branch
script document that could itself drift from the graph.

### 2. Compile-time branch proofs (static layer)

Per enumerated branch, existing proofs re-run under that branch's flag
assignment — reachability, cast selection, staging:

- **Terminality**: every branch reaches an ending (flow reachability with the
  branch's flags pinned).
- **Cast continuity** (the island class): from the quest where a branch opens
  through every later quest, each NPC present on multiple branches must have a
  cast selection valid under that branch — spec-0020's proof 4 extended from
  "the declaration exists" to "the selector resolves to it on this branch at
  every quest after the fork" (later-declaration-wins makes the whole suffix
  load-bearing, which is exactly where round 13 broke).
- **Exclusive-content leakage**: content gated on branch A's flags must be
  unreachable under branch B's assignment, and vice versa — a mourning scene
  reachable on the branch with no death is a build error, not a review note.
- **Hard event contradictions**: the structured verbs make a subset of
  narrative errors machine-decidable per branch — an entity that `dies` and
  later speaks/moves/`departs` on the same branch, a gate that `seals` and
  is later walked through, an item `loses`d and later spent. Diagnostic
  shows the branch and both chronicle lines.
- Spec-0023 combat arithmetic runs per branch where branches change encounters
  or kits.

Failures are new DW diagnostics (block assigned at implementation dispatch),
each showing the branch assignment that breaks.

### 3. The branch plan artifact + bot tier (dynamic layer)

The compiler emits `validation/branch-plan.json`: per branch, its flag
assignment, its critical path (the existing per-path artifact, computed under
that branch), and the dialogue choices the bot must make to enter it. The
harness gains branch runs: same machinery as today's critical path, plus
scripted dialogue choices at declared branch points.

Tiering (cost honesty — a full run is ~20 min):

- **Release tier**: every enumerated branch, full run.
- **PR tier**: branches whose content the diff touches (the compiler maps
  changed quests/casts/effects to the branches they participate in); minimum
  one branch. The plan artifact records which branches ran and which were
  skipped — a skipped branch is named, never silent.

### 4. The branch chronicle + narrative review (generation-time layer)

Dialogue text carries meaning no compiler can check ("Where is Antiphos,
Captain" is only wrong because Antiphos is alive HERE). The rubber-stamp risk
is real: a reviewer handed a pile of per-node data has no way to know the
authoritative answer, and will nod. The design:

- **The compiler assembles a per-branch chronicle** (流水账): for each
  enumerated branch, every reachable story node's `happening` line, in the
  order the compiled graph actually plays them — a pseudo-natural-language
  account of that storyline from first beat to ending. The SKELETON
  (ordering, reachability, which nodes appear) is derived machine truth;
  only the flesh (each line's text) is authored, node-locally. Emitted
  deterministically alongside `branch-plan.json`. Narrative incoherence
  becomes a readable contradiction in sequence: on the flee chronicle,
  "Antiphos survives" is followed pages later by "Elpenor mourns Antiphos"
  — visible in one linear read.
- **Review is chronicle vs design, with citations.** The `/new-delve` skill
  gains a mandatory pass per branch: read the chronicle end to end against
  the campaign's DESIGN.md (the intent document that already exists and is
  already conformance-reviewed) and against the dialogue reachable on that
  branch. Every finding or clearance must cite chronicle lines; a dialogue
  line touching branch-divergent state must be licensed by a chronicle line
  or the pass fails mechanically. The citation table in the generation log
  is the artifact of record — "reviewed" is checkable, never folklore.

## Out of scope

- No semantic NLP in the compiler — text-vs-facts stays a generation-time
  review against the emitted chronicle.
- No branch-coverage requirement on bark pools (spec-0020 exempts them: a bark
  never claims history).
- No exhaustive flag-combination sweep — branches are declared, verified
  complete, then enumerated.

## Acceptance criteria

- [ ] A flag that forks casts/staging/structure downstream without a declared
      branch point fails the build (undeclared-fork DW code + test).
- [ ] Island round-13's flee-branch desync, replayed as a fixture, fails the
      cast-continuity proof with the branch assignment shown; round-14's
      corrected content passes.
- [ ] A branch-A-exclusive scene reachable under branch B's assignment fails
      (leakage DW code + test); each branch reaches an ending or the build
      fails (terminality DW code + test).
- [ ] `branch-plan.json` is emitted deterministically (byte-identical per
      ADR-0006) and names every branch with flags, path, and entry choices.
- [ ] Harness completes a scripted-choice branch run on a two-branch fixture;
      release tier runs all branches; PR tier runs diff-touched branches and
      the artifact names skipped ones.
- [ ] A story node (quest, objective, staging effect, wave, gate/ending,
      story-weight dialogue beat) without a `happening` declaration fails
      the build (missing-happening DW code + test).
- [ ] A hard event contradiction — `dies` then acts, `seals` then walked
      through — on any enumerated branch fails the build with both
      chronicle lines shown (contradiction DW code + test).
- [ ] The compiler emits a per-branch chronicle, deterministically: every
      reachable node's `happening` line in compiled play order, readable
      start to ending; the r13 flee fixture's chronicle contains the
      survives/mourns contradiction in sequence.
- [ ] SKILL.md gains the mandatory per-branch chronicle review vs DESIGN.md
      with a citation table in the generation log (uncited branch-divergent
      dialogue fails the pass) — tooling-sync in the same PR.
- [ ] Verification changes touch no shipped campaign bytes.
