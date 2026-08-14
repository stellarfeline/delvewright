# spec-0033: The grammar's idioms are what an author is missing

- **Status**: Proposed (owner ruling 2026-08-12 — when an approved concept and
  the back end disagree, grow the back end; do not cut the concept down to
  what the tooling happens to say. Re-scoped twice the same day: first by
  measurement, when most of what looked like engine work turned out to be
  authoring material; then by the owner's question — *how would you ever know
  which structures the library needs?* — which is the right question and the
  reason §3 buys techniques instead of buildings.)
- **Specs**: 0027 (the back end this is the authoring material for)
- **ADRs**: 0004 (prefabs+jigsaw), 0006 (determinism), 0013 (licensing)
- **Non-goals**: curve primitives (integer arithmetic has no root, so arches
  are stepped-linear), reflection in `reorient`, per-cell probability that
  varies continuously with position. All three are real limits; none is shown
  to hurt yet, and §5 says how we will find out.

## 1. The finding

A zone was produced end to end through `prefab-procedure.md`. Every machine
gate passed with non-zero bindings, the bytes reproduce, and the piece is a
solid rectangular block of stone that shares nothing with the concept it was
authored from — a roofless ruined cloister of pointed arches. The gates bind
to buildability. Nothing binds to fidelity, and the one step that judges it
(§5) could not see inside a piece at all until PR #403.

The reflex is to call the missing arch an engine gap. It is not. A ruined
Gothic arcade — repeated bays, piers, stepped pointed heads, a ragged mossy
crest — was written in ~40 lines of program JSON with **no engine change**, at
silhouette complexity 1.96 against the produced zone's 1.03. `minecraft:air`
is a legal member of a weighted palette role, which is erosion, and voided 55
of 486 cells on the first try.

Measured across all eight drowned-bell zone programs, ~1300 IR nodes:

| Construct | Uses in the corpus |
|---|---|
| `skip` | 0 |
| weighted-mix palette role (the only per-cell material variation there is) | 0 of 43 roles |
| `minecraft:air` inside a mix (rubble, erosion, decay) | 0 |
| `otherwise` (the language's own "no other alternative matched") | 4 of 353 conditions |
| distinct block states, across eight zones | 19 |

Every role in the campaign is a single block, which is the whole explanation
for why every zone renders as monoculture. An author following the procedure
exactly could not have arrived at an arcade: there is no arch anywhere in the
library to start from, and §3 of the procedure tells them to start from the
library.

## 2. What the arcade actually required

Not an example of an arch. Four techniques, none of which is a building:

- repetition is **guarded self-recursion** that peels one slab and calls itself
  on the remainder — the language has no `repeat`;
- a **shape** is that same recursion with each step's extent computed
  arithmetically from the remaining dimension. This one technique is
  simultaneously the arch, the gable, the ramp, the vault, the spire and the
  tapered tower;
- **erosion** is `air` weighted into a role;
- **priority between alternatives** is `otherwise`, not hand-negated guards.

None of the four is discoverable from the IR's type signatures, and all four
were read out of `crates/grammar/src/ir.rs` — a source file no authoring agent
reads. That is the actual gap, and it is small.

**This is why the deliverable is not a list of structures.** Enumerating
`arcade`, `vault`, `spiral` would cap authorship at what we thought of: the
next creator wants a ziggurat, a headframe, a gantry, finds no entry, and
concludes the back end cannot. It is also the failure CLAUDE.md names — a
catalogue of buildings is authored content wearing a primitive's clothes, and
the list is unbounded, so no version of it is ever complete. Techniques
compose; catalogues do not.

## 3. Deliverables

**A. Nine idioms, documented in `grammar.md`, indexed by technique.** Each with
one minimal runnable program that exists to teach the technique and says so.

1. **Repetition** — guarded self-recursion peeling one slab per step.
2. **Priority** — `otherwise`; two guards that can both hold are a probability.
3. **Shape** — recursion whose per-step extent is arithmetic on the remaining
   dimension (arches, gables, ramps, vaults, spires, batter).
4. **Erosion** — `air` as a weighted member of a role.
5. **Graded erosion** — banded splits, a different mix per band.
6. **Surface detail** — the rule that built a surface splits off the layer
   against it and paints it (vegetation, sconces, rubble, silt).
7. **Symmetry without reflection** — peel from the low end and the high end
   with the same arithmetic, because `reorient` permutes and never mirrors.
8. **Show-through** — `skip`, so an earlier fill survives under later structure.
9. **Light** — any role may be a light-emitting block; a one-cell split is a
   sconce. (Closes task #96: every bell zone currently probes `dark` because no
   rule in the library exposes a lit role, and binding a floor to a lamp would
   be the downstream hack the doctrine forbids.)

Plus **one composite** program that uses several idioms at once, labelled a
composition demonstration rather than a catalogue entry — the ruined arcade,
since it already exists and its provenance is a real session.

**B. The two undocumented constructs**, `otherwise` and air-in-a-mix, enter
`grammar.md` and `prefab-procedure.md` §3, and the skill in the same PR
(tooling-sync rule). Both cost a measured wrong turn: the first arcade attempt
opened one bay of four.

**C. A demonstration-coverage report.** Which IR constructs no example
exercises. This was measured by hand once; unmeasured next time is dark again.

## 4. Acceptance criteria

1. `grammar.md` documents each idiom in §3A with a program that runs as
   written, and `prefab-procedure.md` §3 sends an author to the idiom index
   before the corpus.
2. `delve-grammar` reports, over the library, a binding count per IR
   construct: every `Node` variant, every `Cond` variant, each palette paint
   kind. A construct at zero is printed as a **finding**, in the shape the
   expansion gates already use.
3. The construct list is derived from the IR by an **exhaustive match**, not a
   hand-kept list. Adding a variant to `Node` or `Cond` fails to compile until
   someone classifies it, and it begins life at zero bindings — a surface
   nothing demonstrates is a finding on the day it lands.
4. **The report measures demonstration, not expressiveness, and says so.** A
   green report means no part of the language is undemonstrated. It does not
   mean an author can build a cathedral, and no document, PR or review may
   cite it as evidence that they can. Conflating the two would be this
   project's own recurring vacuity, one level up.
5. **The authoring trial is what binds §3A to reality.** An agent given only
   the reader-facing documents — `grammar.md`, `prefab-procedure.md`, the
   skill; **not** `crates/grammar/src/*` — and a concept image covered by none
   of the examples, produces a program that expands green and that a reviewer
   judges to be that concept. The trial records the concept, the program, the
   eye-level shots and the verdict.
6. **A failed trial is the only thing that grows the idiom list.** When a trial
   fails, the report names the missing idiom or the missing primitive; that
   name, not a planner's guess about typologies, is what justifies the tenth
   entry. An idiom added without a failed trial behind it is a catalogue entry
   in disguise.
7. The report is bound to the events that a corpus or an IR change is pushed,
   never to a line in a document (CLAUDE.md: a gate nothing invokes is unrun).
   Both bindings are compile- or test-level inside the existing **required**
   Rust job. There is no separate grammar CI job and this spec does not ask for
   one: every job name here is a required status context, so a new job is
   advisory until branch protection is edited — the exact shape CLAUDE.md
   rejects.
8. **The idiom index and the coverage corpus are two different sets, and they
   grow by different rules.** The index (§3A) is a curated set of *techniques*
   and grows only by a failed trial, per §4.6. The corpus is the
   *demonstration* set, and every IR construct owes it at least one example
   reachable from `delve-grammar list`. A minimal program showing what a
   construct looks like is **not** a claim that it is a technique — `Cond::NoneOf`
   is negation of guards, a language feature and not a way of building
   anything, and it earns a corpus example without earning an index entry.
   Without this distinction §4.3 and §4.6 contradict each other the first time
   the report reds on a construct no idiom covers, which happened immediately.
9. No campaign content is authored against this spec. Zone programs are a
   separate, later round; this spec ends when the idioms, their examples and
   the measurement exist, and one authoring trial has run.

## 5. How the non-goals get revisited

Curves, reflection and continuous erosion are excluded because nothing has yet
paid for them. Each is revisited the same way the idiom list grows: a trial or
a corpus program that cannot be written without it, with the workaround named
in that program's own doc comment. Three such comments open a spec. Excluding
a feature until the work proves it necessary is the no-hacks rule pointed at
ourselves.
