# Step 5 — quests and dialogue


This is the long step. **What the DSL can express is in *Reference: what a quest
can do*** — read it before writing, not after a refusal. **How the prose has to
be written is in *Reference: writing craft*** — run its section A over every
line before calling this step done.

The order inside `quests.json` matters: **write the `cast` block first, before
the objectives.** Every quest declares, for every NPC live in it,
`{at, doing, dialogue}` — position first, story second.

Then `dialogue.json`. Two rules that are not style preferences:

- **A dialogue option is a button caption, not a sentence.** Vanilla draws each
  option on a fixed 150-GUI-px button and *scrolls* a label that does not fit.
  Author to **≤20 Latin / ≤12 Han characters**; the compiler refuses over-long
  ones (`DW0331`). What does not fit belongs in the node's body text, which
  wraps, in the option's `tooltip`, or in the NPC's reply — never in the button.
- **Re-derive every node's option list from that node's situation.** Never carry
  an option list forward from an earlier node.

Loop `delvec --prefabs "$DELVEWRIGHT_PREFABS" validate <campaign-dir>` until clean after each.
**This is the step where clean is reachable**, and it is where the codes step 3
ended holding go away: writing `quests.json` clears `DW0150` for every quest at
once and `DW0818` with it, and the effects it carries clear `DW0172`, `DW0112`
on a branch's `leads_to`, `DW0482` and `DW0197`; `dialogue.json` clears
`DW0152`. If any of them
survives a written stage 5 it has stopped being the expected state and is a real
finding — a `DW0150` here means an id in the plan and an id in `quests.json` do
not match, and the message says how many quests stage 5 declares, which is how
you tell that apart from the step-3 state.

**One code arrives rather than goes away, and this is the first run that can
raise it.** `DW0890` refuses when the approved design and the built world do not
agree about the sky: the rows step 4 wrote into `design.json` state `night` and
`world.json` declares `noon`, or an effect reaches a `dawn` no row states, or a
row names a picture that is not under `design/` — or a picture there has no row.
Every run prints its binding line first (`design record: N reference(s) recorded
over M image file(s) …`), refusing or not, so read that line and check the two
counts are the two you approved. The remedy is always a document edit on
whichever side is wrong, and *never* moving the hour to satisfy a mob — that is
`DW0496`'s rule, and *Reference: authoring pitfalls* says why.

If the campaign declares other languages, the localization stage is a **final
document stage after `dialogue`** — see *Reference: other languages*. It does
not change anything below.
