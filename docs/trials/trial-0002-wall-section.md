# Trial 0002 — a castle's first wall section

The subject is the question trial 0001 could not answer: **can this engine
produce scenery with content in it, or only a handsomely decorated single-storey
box?**

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

```sh
delve-grammar expand --file castle-wall.json --region 32x28x90 --seed 7 --id castle-wall
```

88 rules / 96 alternatives / 24 params / 6 roles; 29883 filled cells of 80640; 11 block
states; 42 anchors; 2 tiles past the 48-per-axis cap. Re-expanded into a second
directory: byte-identical.

| # | Answer |
|---|---|
| R1 | **yes** — judged against a square-on west elevation |
| R2 | 30 rooms against a bar of 12; 5815 of 6143 standable cells reachable (94.7%) |
| R3 | pass, no zero bindings — and one green gate that measures nothing |
| R4 | six gaps, one of them the report's centre |
| R5 | **10 by construction, 6 hand-computed** |

### R1 — it reads as a castle wall

A continuous crenellated parapet the full 90 blocks; a projecting mural tower rising
eight courses above the wall-walk with its own battlement; a battered plinth in a
separate greener tuff family; arrow loops on a six-cell rhythm; the timber gallery with
its corbel brackets; the breach with its parapet blown out above it. The silhouette
carries it, which is the standard. Two honest qualifications: the breach reads as a dark
patch at elevation scale and needs the close view, and 90 blocks of wall face has no
vertical articulation between the endcap pilasters and the tower.

### R2 — the count is independent of the author's own anchors

Rooms were counted by flooding each storey through cells with the full five-cell clear
height — a three-tall doorway is a wall to that flood — which yields 13 ground-storey
chambers, 13 first-storey, 3 in the tower and the undercroft. Floor areas 72 to 173
cells.

The undercroft is unreachable, and that is the brief being satisfied rather than a
defect: it is the one-way descent. `--reachable-floor` goes red on it and the run ships
that red, because the engine offers no way to state the claim. The agent calibrated its
own walker against the tool — reproducing 5815 / 114 / 188 exactly — and then measured
what the tool cannot be asked: with a fall edge, reachable rises to 5935 and the
undercroft flips to reached; walking out of it reaches 108 cells at y1 and nothing above,
under the plain step and under walk-and-fall alike. That is `drop_shaft`'s pair of gates,
reproduced by hand.

Two measurements off the exported bytes. The breach sill is one unbroken course from the
plank deck to the room floor. And the loop is real: cutting the breach alone leaves the
walkway reachable, cutting the postern alone leaves it reachable, cutting both strands
it — two independent ways out and back.

### R3 — the gate that is bound, honest, and empty

`--traversable` passes with **bound 64, and the route it certifies never enters the
building.** The approach and exit ends are the Z faces; the only qualifying cells there
are the outdoor apron and the bailey ground, so the walk goes round the outside. The
count is right. A binding count structurally cannot expose this, because the number is
not the thing that is wrong.

### R4 — what was missing

1. **A one-way descent cannot be stated, so the only gate that binds to it must be left
   red.** `nav::reachable_with_fall` is public and `drop_shaft` is gated on it in both
   directions — from outside `cargo test` there is no way to ask. An authoring agent had
   to build a second instrument to answer a clause the vocabulary already names.
2. **`delve-admit lighting` has no way to scope to sheltered floor**, so any piece with
   an outdoor half reports `dark` and adding lamps moves the number not at all. The
   distinction already exists one tool over: reachability separates `sheltered` from
   `open to the sky` and says out loud that it never gates on the second.
3. **A `repeat` split refuses a box shorter than one un-repeated pattern**, stated once
   inside `boulder_stair`'s entry — not in idiom 1, which is where §3 sends an author.
   Both of the run's expansion refusals were this.
4. **A split's size list cannot be shared between two rules.** The building's plan is one
   six-piece X split and every band needs it with different children, so it is written
   out ten times. Params make the widths co-vary; nothing makes the child *order*
   co-vary, and that is where hole C's risk sits.
5. **A hole's alignment is expressible only by making the two sides siblings in one
   split.** That works — it is the run's main positive finding — but it forces the whole
   building's decomposition order. There is no way to say "this opening is the same cells
   as that one".
6. **The mandatory palette step could not run.** `block-appearance.py` reads the block
   registry from the compiler crate; the sandbox omits `crates/` by design, and the one
   step the procedure marks non-negotiable is the one an authoring agent could not
   perform from the material it had.

### R5 — 10 by construction, 6 hand-computed

The structure that buys most of it: split Z into bays, then Y into storey bands, then X
into the plan, and let a hole be a piece of a split whose siblings are the two things
that must meet. `[relative 1, absolute 1]` — never `[absolute 5, absolute 1]` — puts a
floor slab on a band's last course whatever the storey pitch is.

| Hole | By construction | Hand-computed |
|---|---|---|
| A — stair to floor opening | 3 | 1 |
| B — breach to walkway | 2 | 0 |
| C — broken floor to the room below | 2 | 2 |
| D — room door to the room beyond | 3 | 0 |

Hole A's three: the flight's recursion terminates when the box is under two cells tall,
so the `otherwise` arm fills the band's last course, which *is* the floor above — nothing
states the rise; the opening is the absence of a sibling, so it cannot be misaligned; and
the upper flight is the lower one written mirrored, so both anchor to the same Z end. Its
one constant: the two flights occupy complementary X children of two *different* rules,
and if those ever disagree the upper flight has no foot with every gate green.

Hole B is the cleanest thing in the run — the deck is the last course of the apron and
the breach sill is the last course of the curtain, and apron and curtain are two children
of one X split of one Y band. They are literally the same course. No constant to get
wrong.

Hole C's two: the hole is cut in the cellar's own ceiling course, so it can only ever be
over the cellar; the five-block drop and the cellar's headroom both fall out of one
param. Its constants: the cellar is the *second* child of the base band's plan split and
the room above is the second child of bands A and B — same six sizes, same order, by the
author's discipline restated in ten rules. Put the cellar fourth and the hole opens into
solid rock with every gate green. And an unintended one — two independent "centre it"
rules landed on the same cell, so a room's anchor hovers in the hole. `DW0727` caught it
after the fact; nothing would have caught it had the room had no anchor.

Three further constants sit outside the named holes and all fail silently: the blown
parapet's span coincides with the breach's only because the arithmetic was done; nothing
states that the tower must project beyond the curtain face; and nothing checks that the
parapet depth is less than the wall thickness — set them equal and the wall-walk vanishes.

**The transferable number: all four of the brief's named holes line up by construction in
their essential claim. Every constant that could not be eliminated is in the layer
*around* the holes — the plan grid, the parapet, the tower's projection — and every one
of them fails silently.**

### What the toolchain was like to use

Asked in the brief, not afterwards.

The good part is the loop, not the language: `check` is instantaneous, `expand` on an
80,000-cell zone runs in 60 ms, the whole 97-shot render set takes about a minute. Never
waiting on a tool means being able to afford to be wrong, and that is most of what made
the run work.

The single best thing in the system is that `expand` prints reachability whether it was
asked for or not. The agent had already convinced itself the piece was fine; the report
named 114 cells of roofed floor with no route, one pocket of which was a wall ledge past
the tower it had not thought about. It works because it is a measurement with no
threshold — always on, and never something that can be satisfied.

What grated, in order: the mandatory palette tool that could not run; the plan grid
written out ten times, which produced a 4,500-line JSON artifact of record whose real
source is a 350-line generator script the toolchain has never heard of — a layering smell
that exists because the language has no way to name a shape; and `repeat`'s refusal being
filed where an author cannot find it, the only case where the docs set the agent up.

Two pleasant surprises, both about alignment: the `[relative 1, absolute 1]` idiom makes
a slab *the last course of a band* rather than *a course at height 11*, and once every
slab is written that way holes stop needing arithmetic — a language with no positional
index turned out to be excellent at alignment within one axis. And the stair landed its
last tread exactly on the floor above, at any pitch, for free.

And the design lesson the docs do not teach: **the decomposition order is the design.**
Choosing Z→Y→X decided which holes could line up, and it was chosen in the first ten
minutes with almost no information. `grammar.md` teaches nine techniques and none of them
is "what to split first". The rule the run earned: *the last axis you split is the only
axis on which two things are guaranteed to meet* — so split last on the axis your
openings run through.

### What this run put on the backlog

`--traversable`'s outside route; a one-way descent has no surface; the split-order
design rule; plus the doc corrections in R4.

## Instrument bounds

Every judged verdict declares what bounded it: `artifact-bound` when the
instrument could frame the thing being judged, so the answer is about the
artifact; `instrument-bound` when it could not, with the blocker named so the
verdict can be re-taken once it is fixed. Trial 0001 shipped a bounded answer as
a verdict with the disclaimer three paragraphs away, and later rounds cited the
verdict; `tools/check-trial-verdicts.py` is what makes this table exist.

| Verdict | Bound | Judged from |
|---|---|---|
| R1 run 0 | artifact-bound | `castle-wall-elevation-field.png`, a square-on west elevation at 900 px, plus the close view of the breach |

## Run 1 — result

Not yet run.
