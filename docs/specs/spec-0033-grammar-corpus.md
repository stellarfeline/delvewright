# spec-0033: The corpus is the grammar's expressiveness surface

- **Status**: Proposed (owner ruling 2026-08-12 — when an approved concept and
  the back end disagree, grow the back end; do not cut the concept down to
  what the tooling happens to say. Re-scoped the same day by measurement: most
  of what looked like engine work is corpus work.)
- **Specs**: 0027 (the back end whose corpus this is)
- **ADRs**: 0004 (prefabs+jigsaw), 0006 (determinism), 0013 (licensing)
- **Non-goals**: curve primitives (integer arithmetic has no root, so arches
  are stepped-linear), mirroring in `reorient`, per-cell probability that
  varies continuously with position. All three are real limits; none is shown
  to hurt yet, and §5 says how we will know.

## 1. The finding

A zone was produced end to end through `prefab-procedure.md`. Every machine
gate passed with non-zero bindings, the bytes reproduce, and the piece is a
solid rectangular block of stone that shares nothing with the concept it was
authored from — a roofless ruined cloister of pointed arches. The gates bind
to buildability. Nothing binds to fidelity, and the one step that judges it
(§5) cannot see inside a piece at all (task #91).

The reflex is to call the missing arch an engine gap. It is not. A ruined
Gothic arcade — repeated bays, piers, stepped pointed heads, a ragged mossy
crest — was written in ~40 lines of program JSON with **no engine change**,
at silhouette complexity 1.96 against the produced zone's 1.03. Decay likewise:
`minecraft:air` is a legal member of a weighted palette role, which voids cells
at an authored rate and has been able to since the port landed.

What the language can state and what the library demonstrates are two different
sets, and only the second one is reachable. `prefab-procedure.md` §3 tells an
author to *start from the corpus, never from the schema* — correct advice,
because editing the nearest rule is what converges — and the consequence is
that the corpus **is** the expressiveness, whatever the IR supports.

Measured across all eight drowned-bell zone programs, ~1300 IR nodes:

| Construct | Uses in the corpus |
|---|---|
| `skip` | 0 |
| weighted-mix palette role (the only per-cell material variation there is) | 0 of 43 roles |
| `minecraft:air` inside a mix (rubble, decay, erosion) | 0 |
| `otherwise` (the language's own "no other alternative matched") | 4 of 353 conditions |
| distinct block states, across eight zones | 19 |

Every role in the campaign is a single block, which is the whole explanation
for why every zone renders as monoculture. And an author following the
procedure exactly could not have arrived at an arcade: there is no arch
anywhere in the library to start from.

Two of those rows are documentation defects with a measured cost. §3 warns that
"two guards that can both hold are a probability, not a priority" and never
mentions `otherwise`, which is the cure the language already ships — the first
arcade attempt opened one bay of four. That a palette role may mix in air is
written nowhere.

## 2. What is actually missing

| The concept needs | Expressible today? | What closes it |
|---|---|---|
| pointed arch, arcade, repeated bays | yes — recursion + guards | a corpus program |
| ruin, rubble, erosion | yes — `air` in a weighted role | a corpus program |
| decay stronger with height | yes, banded (a split per band) | a corpus program; continuous variation is a non-goal |
| vegetation on floors and ledges | yes where the program built the surface | a corpus program |
| light, so a zone is not `dark` | yes — any role may be a light-emitting block | a corpus program (closes task #96) |
| open-roof courtyard | yes — do not fill the top | a corpus program |
| spanned ceiling / vault | yes, stepped | a corpus program |
| water at a declared level | yes | a corpus program |
| true circular arch, dome | **no** — no root in the arithmetic | non-goal; revisit per §5 |
| mirrored detail without writing both halves | **no** — `reorient` permutes, never reflects | non-goal; revisit per §5 |

## 3. Deliverables

**A. Corpus.** Eight demonstration programs in the library, each teaching one
typology, each generic (a creator building something other than this campaign
must be able to use it): `arcade`, `ruin`, `overgrowth`, `brazier`, `cloister`,
`vault`, `flooded`, `spiral`. They are typologies, not zones — a zone program
calls them; naming them after the campaign that first needed them would be the
"keyed to the verb, not the object class" defect.

**B. Documentation.** `otherwise`, air-in-a-mix and recursion-as-repetition
enter `grammar.md` and `prefab-procedure.md` §3, and the skill in the same PR
(tooling-sync rule). `grammar.md` gains an index of what the corpus
demonstrates, so "start from the corpus" is a lookup and not a search.

**C. A coverage report.** Which IR constructs no library program exercises.
This measurement was made by hand once; unmeasured next time is dark again.

## 4. Acceptance criteria

1. `delve-grammar` reports, over the library corpus, a binding count per IR
   construct: every `Node` variant, every `Cond` variant, and each palette
   paint kind. A construct at zero is printed as a **finding**, in the same
   shape the expansion gates use.
2. The construct list is derived from the IR by an **exhaustive match**, not a
   hand-kept list. Adding a variant to `Node` or `Cond` therefore fails to
   compile until someone classifies it, and it begins life at zero bindings —
   a new surface nothing demonstrates is a finding on the day it lands.
3. A named required set has no zero bindings. Exemptions are an explicit
   allowlist, each with a reason string (the shape `tools/check-dw-codes.py`
   already uses).
4. Each of the eight programs in §3A is in the library, expands green at a
   declared region and seed with its gates bound non-zero, and declares at
   least one anchor. A test expands them; no report file is checked in, because
   a checked-in report goes stale silently.
5. The report runs as a step of the grammar CI job — bound to the event that a
   corpus or IR change is pushed, never to a line in a document (CLAUDE.md: a
   gate nothing invokes is unrun).
6. No campaign content is authored against this spec. Zone programs are a
   separate, later round; this spec ends when the corpus and its measurement
   exist.

## 5. How the non-goals get revisited

Curves, mirroring and continuous decay are excluded because nothing has yet
paid for them. Each is revisited when a corpus program cannot be written
without it and the workaround is named in that program's own doc comment —
that comment is the evidence, and three of them is the trigger for a spec.
Excluding a feature until vanilla or the corpus proves it necessary is the
no-hacks rule pointed at ourselves.
