# Step 3 — npcs, classes and the quest plan

## Contents

- [The three documents, in order](#the-three-documents-in-order)
- [Where step 3 ends](#where-step-3-ends)


## The three documents, in order

Write them in that order; each conditions the next. Get each one's shape from
`delvec schema --stage <name>` first, and a **worked example** of it from
`delvec metrics --gym <dir>`, which writes nine complete documents that build —
these three among them. Reach for the gym whenever the schema alone is slow to
read: `--stage all` is half a megabyte in which every stage's `content` is a
`$ref`, so one document's required fields are several dereferences away, while
the gym hands you that document filled in. The schema stays the authority on
form; the gym is what shows you one. Then run
`delvec --prefabs "$DELVEWRIGHT_PREFABS" validate <campaign-dir>`
after each, fixing by diagnostic code. **Three failed repairs on the same code
means stop and look at the design**, not at the syntax.

**That loop does not end on a green run, and it is not supposed to** — read
*Where step 3 ends*, below, before you start it. Once the quest plan is on disk
the campaign carries refusals that are correct and that only step 5 can clear,
and looping against them is the one way to spend an afternoon here.

- **NPCs**: personas per the schema — `archetype`, `speech_style` and
  `motivation` are required, and step 5 honours them in every line.
- **Classes**: pre-provided gear, no grind. If the campaign places a bonfire,
  **every class kit must declare a flask** (`DW0476`) — see *Reference:
  authoring pitfalls*.
- **Quest plan**: acts, dependencies, which area each quest belongs to,
  mandatory-only quests, paced to `target_minutes`.
- **Declare every story fork here.** If a choice forks who lives, where the
  party ends up, or which ending plays, it is a `branch_points` entry:
  `{id, opens_at, forks_on:[flags], branches:[{id, flags, leads_to}]}`.
  `leads_to` is one field — a `quest/…` the branches converge at, or an
  `ending/…` this branch runs to; the id prefix says which. Name each ending on
  the `campaign-complete` that fires it (`"ending": "ending/<slug>"`). A flag
  that gates casts, staging or quest structure and is not set on every
  playthrough must belong to a declared point (`DW0480`). Every declared branch
  must reach an ending (`DW0482`) and must be exclusive: no sibling's flag may
  be producible on it (`DW0484`).


## Where step 3 ends

**The campaign does not validate when this step is done, and no amount of
looping will make it.** The quest plan names quests, stage 5 is step 5, and the
compiler is right to refuse a campaign that plans a quest nothing expands. So
the finish line for this step is not a green run — it is a run whose every
remaining error the rule below accounts for.

Run `delvec --prefabs "$DELVEWRIGHT_PREFABS" validate <campaign-dir>` and sort the output by
**one rule, which is the whole of this step**:

> A refusal whose message names something only `quests.json` or `dialogue.json`
> can supply is the step-3 state, and is not a repair you owe. Every other code
> is a repair you owe now.

Those two documents are step 5, so nothing you write here can produce what they
hold. **Read the message, not the code.** Each of these says in its own words
what is missing and where it comes from — a flag no `set-flag` produces, an
ending no `campaign-complete` declares, a body no `spawn-npc` summons. Every one
of them is a stage-5 effect or a stage-6 dialogue option, every one of them
clears at step 5, and looping on any of them here is the way to lose an
afternoon.

The codes below are the **instances a campaign hits today**, not the rule. A
code that is not in this list is judged by the rule above — by what its message
says is missing — never by its absence here.

**`DW0890` is not one of them and cannot be**, which is worth knowing before you
meet it: the design record does not exist until step 4 approves one, so at this
step `validate` prints `0 reference(s) recorded over 0 image file(s)` and says
nothing further. It becomes live at step 5, on the first `validate` after the
gate, and it is a repair you owe there like any other.

| code | what it is saying | what to do |
|---|---|---|
| `DW0150` | the plan is written and stage 5 is not. **One** diagnostic naming every planned quest — it says so itself: *an authoring state, not a fault*. | nothing. It clears at step 5, for every quest at once. |
| `DW0818` | site-plan campaigns only: every layout-graph beat and quest-gated way names a quest stage 5 has not written. The message says so and points at `DW0150`. | nothing. Same state, same clearing. |
| `DW0152` | one per NPC with no dialogue tree. | **stubbable, unlike the rest.** A tree of one node with an empty `options` is accepted and raises nothing new, so clear it here if you want a shorter list to read at step 4; leaving it is equally fine. Step 5 replaces the stub either way. |
| `DW0172` | an objective gate, or a `branch_points` `forks_on`, names a flag no `set-flag` produces — and every `set-flag` lives in a stage-5 quest effect or a stage-6 dialogue option. | nothing, once you have checked the flag is one step 5 is going to set. A flag no beat in the plan is ever going to produce is a plan defect and you fix it here. |
| `DW0112` | a branch's `leads_to` names an `ending/…` no `campaign-complete` declares, and `campaign-complete` is a stage-5 effect. | nothing. **But `DW0112` is the generic unresolved-reference code** — on any other path (an npc's `area`, a `datum`, a trigger's target) it is a genuinely broken reference and you fix it now. The path in the message is what tells the two apart. |
| `DW0482` | a declared branch reaches no ending, because nothing sets its flags yet. The same absence `DW0172` names, one diagnostic per branch. | nothing. It clears with the `set-flag`s at step 5. |
| `DW0197` | a `deferred: true` NPC that no `spawn-npc` summons, and `spawn-npc` is a stage-5 effect. | nothing, if the character is meant to walk in at a beat step 5 will write. If it was never meant to be deferred, drop `deferred` now — that is a stage-2 repair, not a stage-5 one. |
| `DW0816` | site-plan campaigns only: a place the graph's closure never reaches from `entry` under gating and one-way direction. The message names the place, the nearest place a body can stand, and how many of the graph's places are reachable at all. | **read the caveat before you decide.** Where the graph gates on a flag and stage 5 declares no quests, the message says so and points at `DW0150` — the way may be one a stage-5 `set-flag` opens, and the check is the same as `DW0172`'s: is a beat in the plan going to produce that flag? Where the message carries no such caveat, the place is unreached for a reason the mission has nothing to do with, and it is a graph repair you owe now. |
| `DW0817` | site-plan campaigns only: the authored `critical_path[]` does not hold — four faults under one code. It does not run `entry` → `goal`; a step names two places no connection joins, or joins them the other way; a step crosses a connection not open yet at that point in the walk; or it never visits a place where a beat of the mandatory quest spine happens. | **three of the four are yours now.** Only the *not-open-yet* fault carries the unwritten-mission caveat, and only when that connection waits on a flag — the other three are judgements about the graph by itself, which an absent mission has nothing to say about, so they are repairs to the graph or the path here. Read which fault the message names. |

Measured on a twenty-four-place site-plan campaign at the end of this step: 42
errors over seven of those codes — `DW0818` ×28, `DW0152` ×6, `DW0482` ×2,
`DW0172` ×2, `DW0112` ×2, `DW0197` ×1, `DW0150` ×1 — plus the two ordinary
warnings (`DW0813`, `DW0822`) that stand on every site-plan campaign. A list
that long is the expected shape here, not evidence that something went wrong.

**Do not try to clear `DW0150` by writing empty stage-5 quests.** It is the
obvious move and it is strictly worse: a quest carrying only what the schema
requires is refused again by `DW0481`, because every quest must say what it does
to the story, and by `DW0460` once for every NPC live in it. Measured on a
five-quest plan, writing the five minimal expansions took the campaign from 5
errors to 15. `DW0150` says this in its own message; believe it.

**And the "three failed repairs" rule does not fire on these**, because there is
nothing to repair. It fires on a code you have genuinely tried three times to
fix.

If someone else gave you the brief, show them a 3–6 line summary of each
document — the summary, not the JSON — and wait, unless they asked for an
uninterrupted run.
