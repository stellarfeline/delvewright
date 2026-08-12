# Trial 0001 — Notre-Dame de Paris

The spec-0033 §4.5 authoring trial. Owner set the subject, 2026-08-12: a
building whose fine structure is so much of its identity that the usual
Minecraft answer is to build it at several blocks per real metre. That is the
answer this project does not take (DEC-0071), which is what makes it the right
probe — it asks whether the back end can carry a recognisable cathedral with
the detail deliberately thrown away.

**This brief is written down before either run, and neither run may change it.**
A brief adjusted after seeing a result measures nothing. Both runs use this
file verbatim.

## The two runs

| Run | Documents the agent may read | Measures |
|---|---|---|
| **0** | the reader-facing docs as they stand *before* the spec-0033 idioms land | today's ceiling |
| **1** | the same documents *after* the idioms land | what the idioms bought |

Run 0 is the red half. Running only run 1 would show a result and prove nothing
about its cause.

## What the agent may read

The reader-facing documents only — `docs/reference/grammar.md`,
`docs/reference/prefab-procedure.md`, `docs/reference/tools.md`, the
`/new-delve` skill — plus the library corpus through `delve-grammar list` and
`delve-grammar show`, and the tools' own `--help`.

**Not** `crates/grammar/src/**` or any other engine source. The trial exists to
measure what an authoring agent can do from the material an authoring agent
actually has; the planner's own arcade probe read `ir.rs`, which is exactly the
advantage under test. The sandbox is arranged so the sources are absent rather
than merely forbidden.

## The brief given to the agent

> Build Notre-Dame de Paris as a grammar program, at playable scale — a party
> of one to four walks through it in adventure mode. It is a cathedral-sized
> cathedral, not an enlarged model: fine detail is out of scope by decision.
> What must survive is the silhouette and the interior's character.
>
> The silhouette: a twin-towered west front, the two towers of equal height and
> flat-topped, with a great round window between them above the portals; a long
> nave under a steep roof running east from that front; a transept crossing it;
> a spire over the crossing; an apse closing the east end; buttressing along the
> flanks.
>
> The interior: a tall central vessel flanked by lower aisles, separated by an
> arcade of pointed arches; a band of windows above the arcade; a stone floor
> walkable from the west doors to the east end.

No reference image is supplied. The referent is among the most documented
buildings in the world, and a text brief keeps the trial clear of third-party
image licensing (ADR-0013). The palette is therefore chosen from the fiction
and measured against the ranked list, which `prefab-procedure.md` §2 already
allows.

## Rubric — identical for both runs

| # | Question | Answer |
|---|---|---|
| R1 | From an exterior shot alone, and without being told, would a viewer name the building? | yes / partial / no |
| R2 | Does an eye-level interior shot read as the inside of that building? | yes / partial / no |
| R3 | Do the machine gates pass, `traversable` included, with non-zero bindings? | yes / no |
| R4 | What was missing? | the idiom or primitive, named |

R4 is the deliverable that matters most. Under spec-0033 §4.6 a failed trial is
the **only** thing that justifies adding a tenth idiom, so an unnamed failure
buys nothing. "It could not do it" is not a finding; "there is no way to state
X, and the nearest workaround is Y" is.

R1 and R2 are judged by the planner, and the judgement is recorded with the
shots so a later reader can disagree with it.

## What is recorded, per run

The program, the region and seed, the full gate report, the shots, the four
rubric answers, and the agent's own account of where it got stuck. Results land
in this file as a section per run.

## Run 0 — result

Not yet run.

## Run 1 — result

Not yet run.
