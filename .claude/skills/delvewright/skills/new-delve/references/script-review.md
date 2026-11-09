# Step 5b — the script review

## Contents

- [When it runs, and when it does not](#when-it-runs-and-when-it-does-not)
- [The handout](#the-handout)
- [The reviewer](#the-reviewer)
- [What you do with the review](#what-you-do-with-the-review)
- [When the loop stops](#when-the-loop-stops)
- [What this step is not](#what-this-step-is-not)

A story you invented is read once more before anything is built, by a reader
who never held the design: a fresh agent handed nothing but what a player
meets. `delvec` proves the delve can be finished; this reading asks whether it
is worth playing, and it is cheapest here — after the documents exist and
before a world, its detail and its ladder are built on them.

## When it runs, and when it does not

**The step runs when you wrote the story, and not when the user gave it.** The
rule you apply, once, against the plan you wrote at step 3: go down the
mandatory quests of `quest-plan.json` and, for each, ask whether the prompt
itself said **what the player does in it and why**. A prompt that names the
acts, the people and what the party does in each — a brief that reads as a
synopsis — gave the story, and the step does not run. A prompt that gave a
theme, a tone, a place, a mechanic or a mood and left any mandatory quest's
deed or reason to you did not, and the step runs. One invented mandatory quest
is enough: the parts you invented are the parts nobody has read.

Write the decision into `GENERATION.md` in one line either way — *script review:
ran; the prompt pinned the place and the tone and left every quest's deed to
me*, or *script review: not run; the prompt gave each quest's deed and reason* —
so the next round knows which case this campaign is.

## The handout

Write the brief first: a file of a few lines, in your own words, saying what
the prompt pinned — the tone, the place, what must be absent, how long. It is
the one thing the reviewer learns about the idea, and it is yours to state; the
reviewer judges the script against it under Q10. Then:

```sh
"$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/script-review.py" <campaign-dir> \
    --brief <brief-file> --engine "$DELVEWRIGHT_ENGINE"
```

It writes `<campaign-dir>/.out/script-review/round-<n>/script.md` — the script
in play order: title and premise, the parts on offer, every line of play with
the places it walks the party to in order (a return named as one) and its
objectives in the order they open, each objective's title, hint, hidden
compass or announcement, what it needs and what it answers, every dialogue tree
as the cast shows it during that line (a completing option named with the beat
it closes, and the engine's rule that it is drawn only while that beat is
open), every narration and visible effect, then everything pressed or passed
in the world — and `handout.md` beside it, which
carries your brief, the questions the reviewer asks and the form the review
takes, both read from `$DELVEWRIGHT_ENGINE/docs/reference/script-review.md`
(§2 and §3) and never restated here. `n` counts the rounds already there. It
prints two lines: the handout's path, and the one-line prompt you launch the
agent with. Its stderr is the binding: quests, objectives, dialogue nodes
rendered of nodes declared, triggers, shops, signs and player-facing lines
counted, and the questions and steps read. A node no cast root reaches is said
by name on stderr and is not in the script; a tree the player cannot open is a
finding of your own.

The script carries what a player meets and nothing you meant: no `DESIGN.md`,
no persona, no `happening`, no note from `GENERATION.md`. **Do not read the
handout or the script yourself before the review is in**: a creator who reads
their own script as a player is the reader this step exists to replace.

A campaign with no `quests.json` or `dialogue.json` is exit 1 (step 5 is not
written); a missing or empty brief, or an engine tree without the page, is
exit 2. Nothing under `.out/` is committed.

## The reviewer

Launch one agent with the **Agent tool**: `subagent_type` `general-purpose`,
and as its prompt exactly the second line the script printed — `Read and
execute <handout path>. Report, then exit.` Nothing is added. **Never a fork**:
a fork inherits this session, the design and every reason you had, which is the
one thing the reviewer must not hold. The handout tells the agent to read the
script and nothing else, to write `review.md` beside it in the form the page
states — what works, the run retold, findings each with where, what the player
meets, the question it fails and why it hurts, severities rated after the
finding pass, the per-question table, the verdict — and to report its path and
verdict. One reviewer runs at a time; it changes nothing but its review.

## What you do with the review

Read every finding, in severity order, and do one of two things with each:

- **Fix it in the documents.** The remedy is in `quests.json`, `dialogue.json`
  or, where the finding reaches the plan — a line with no reason, a crossing
  with nothing on it — in `quest-plan.json` and the step-3 documents, and then in
  step 5's again. A fix that moves the design the user said yes to updates
  `DESIGN.md` in the same commit and is a departure the hand-over reports in
  player words (the rule is on the page under *Reference: turning a prompt
  into a campaign*).
- **Answer it.** A finding you judge wrong is a design decision, and you record
  it: one line in `GENERATION.md` under the round, naming the finding and the
  reason it stands. A reason is about the player's experience, never about the
  effort of the change or the engine.

Never weaken what the reviewer read to make a finding go away — a hint removed
so that a compass finding cannot be raised, a conversation cut so that Q7 has
nothing to count — unless the cut is the better delve.

Then `delvec --prefabs "$DELVEWRIGHT_PREFABS" validate <campaign-dir>` clean,
and `fmt` (step 6) before the next round's handout is written, so the reviewer
reads canonical documents.

## When the loop stops

A round is one handout, one fresh reviewer, one review, your revision. **Launch
a new round after every revision**, with a new agent on the new handout; a
reviewer never reads a second script, a previous review or your answers.

The loop stops at the first of these:

- a review with **no finding at 3 or 4**: the verdict is `build`, and step 6
  follows;
- a review whose every finding at 3 or 4 is one you **answered** in an earlier
  round, raised again in the same place: the answer stands, and step 6 follows;
- the **third** review: whatever still stands at 3 or 4 is recorded as open, and
  the hand-over report says so in player words.

Record every round in `GENERATION.md`: the round number, the verdict, the
per-question table as the review gave it, and one line per finding at 3 or 4
saying what was done — fixed (and where), answered (and why), or open. The
review files under `.out/` are not committed; the record is.

## What this step is not

It is not the design gate: the user said yes at step 4 and is not asked again
here, and a reviewer's `build` is admission to step 6, nothing more. It is not
the writing-craft pass, which you run over every line yourself at step 5. It is
not a measurement of fun: one reader's severities order your work, and the
playtest measures the rest.
