# Trial 0002 — a castle's first wall section

The owner set the subject, 2026-08-12, when she asked the question trial 0001
could not answer: **can this engine produce scenery with content in it, or only
a handsomely decorated single-storey box?**

Trial 0001 measured whether a recognisable *building* could be stated. This one
measures whether a *playable* one can — rooms a party fights in, a route that
loops, a level below reachable only one way.

**This brief is written down before any run, and no run may change it.** A brief
adjusted after seeing a result measures nothing.

## The brief given to the agent

> Build the first wall section of a castle, at playable scale — a party of one
> to four walks it in adventure mode. It is a section of curtain wall and the
> works behind it, not a whole fortress.
>
> It contains **a dozen or more rooms**. Not corridors with doors: places a
> party stops in, fights in, or takes something from.
>
> The wall carries **a breach**, and through the breach a **plank walkway on the
> outside face** that a body can walk. Going out and coming back in are both
> possible.
>
> Somewhere there is a **broken floor**, and through it a **lower level
> reachable one way only** — a body that goes down does not come back up the
> way it fell.
>
> Fine detail is out of scope by decision. What must survive is that the thing
> reads as a castle wall from outside, and that everything named above is
> somewhere a body can actually get to.

## Why this brief and not another

Every clause is a mechanism the corpus already names, so the trial cannot fail
merely for lack of vocabulary: `store-room`, `rafter-hall`, `watch-bay` and
`castle` for rooms; `idiom-erosion` for the breach; `stair-flight` (which carries
a `broken_step`) and `drop-shaft` (whose `rescue_ladder=0` *is* a one-way
descent). What is untested is whether they compose into one building that holds
together.

Each clause is also a **hole that must line up with something on the other
side** — stair to floor opening, breach to walkway, broken floor to the room
below. That is the class of defect trial 0001 found no way to state and nothing
to check, and it is the reason this subject was chosen over a prettier one.

## The two runs

| Run | What the agent has | Measures |
|---|---|---|
| **0** | the reader-facing documents and the toolchain as they stand now | what the language and tools can do |
| **1** | the same, plus the spatial contract once its obligations land | whether the contract catches what run 0 shipped, and what it costs to write |

Run 0 is not a control in the sense trial 0001's was — it is the honest current
answer to the owner's question. Run 1 is the spatial contract's own falsifier,
and the design names in advance what result would prove it wrong.

## What the agent may read

The reader-facing documents only — `docs/reference/grammar.md`,
`prefab-procedure.md`, `tools.md`, the `/new-delve` skill — plus the corpus
through `delve-grammar list` and `show`, and the tools' own `--help`.

**Not** `crates/**`. The sandbox is arranged so the sources are absent rather
than merely forbidden, because the trial measures what an authoring agent can do
from the material an authoring agent actually has.

## Rubric

| # | Question | Answer |
|---|---|---|
| R1 | Does it read as a castle wall from outside? | yes / partial / no |
| R2 | Is every named thing — the dozen rooms, the walkway, the lower level — somewhere a body can reach? | measured, not judged |
| R3 | Do the machine gates pass, with non-zero bindings? | yes / no |
| R4 | What was missing? | the idiom or primitive, named |
| R5 | Which of the four holes lined up by construction, and which by a hand-computed constant nothing checks? | per hole |

**R2 is a measurement, not an opinion.** `delve-grammar expand` now reports how
much of a piece's floor a body reaches from the entrance and names the pockets it
cannot; a claim that a room is reachable is checked against that report.

**R5 is the question trial 0001 could not ask.** A hole that lines up because
the geometry forces it is sound; a hole that lines up because two rules were
given matching literals is a defect waiting for the next edit, and the count of
each is the trial's most transferable number.

**R1 is judged against a square-on elevation**, which trial 0001 could not
render. A verdict bounded by the instrument rather than by the artifact must say
so.

## What is recorded, per run

The program, region and seed, the full gate report, the shots, the five rubric
answers, the contract if the run has one, and the agent's own account of where
it got stuck — including, asked in the brief rather than afterwards, **what the
toolchain was like to use.** Trial 0001 lost that answer because the question was
put to the agent after it had finished.

## Run 0 — result

Not yet run.

## Run 1 — result

Not yet run.
