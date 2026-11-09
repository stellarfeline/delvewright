# Script review — a second reading before the build

For the agent that reviews a campaign's script at `/new-delve` step 5b, and for
whoever maintains that step. The **script** is everything the player meets in
play: the title and premise, the class blurbs, every objective's title, hint and
answers, every line of dialogue and narration, every sign, shop label and
tooltip — read in play order, as a player meets it, with nothing of the design
record beside it. The engine's checks prove a delve can be finished; this
reading asks whether it is worth playing. §1 is what established practice does
when a script, a quest design or a level is reviewed; §2 is the questions the
reviewer asks, which `scripts/script-review.py` reads into the reviewer's
handout; §3 is the form the review takes; §4 is what the review is not.

Every rule is marked **cited** (with its source, listed at the end) or
**authored** (this project's rule, with the reason it exists). Sources are used
for their ideas and paraphrased; a short phrase is quoted only for criticism
(ADR-0013, `docs/ACKNOWLEDGEMENTS.md`).

## Contents

- [1. How scripts, quests and levels are reviewed elsewhere](#1-how-scripts-quests-and-levels-are-reviewed-elsewhere)
- [2. The questions](#2-the-questions)
- [3. The review](#3-the-review)
- [4. What the review is not](#4-what-the-review-is-not)
- [Sources](#sources)

## 1. How scripts, quests and levels are reviewed elsewhere

- **A screenplay is read by a reader, not its author, and the report has a
  fixed shape.** Script coverage opens with a neutral synopsis of what the
  script does, then comments on premise, structure, character and dialogue,
  then a grid that rates each, then one of three verdicts — pass, consider,
  recommend. The synopsis is kept free of opinion so the author can see what
  reached the reader. **Cited:** trade descriptions of script coverage
  (glcoverage.com; Filmustage; writing-world.com).
- **A script is heard the way its audience meets it.** A table read exposes
  what a silent reading smooths over — a scene that drags, a joke that does not
  land, a line nobody would say — because the listener meets the lines in order
  and at speed. **Cited:** Celtx and Casting Networks on table reads.
- **A note says what is wrong, what is missing, what is unclear and what makes
  no sense; it is specific, it is timely, and it is not a demand.** A proposed
  fix only illustrates; the author decides. Catmull names "I'm writhing with
  boredom" as the shape of a bad note, and gives the Braintrust no authority
  over the director. **Cited:** Catmull, *Creativity, Inc.*
- **A critique opens with what worked.** Lerman's Critical Response Process
  begins with statements of meaning — what was striking, evocative or
  interesting — before any question or opinion; questions are neutral, with no
  opinion couched in them; opinions are offered with the artist's permission.
  **Cited:** Lerman, Critical Response Process.
- **A heuristic evaluation walks the artifact with a list and records each
  violation.** Desurvire, Caplan and Toth's heuristics for playability put
  beside each other, under Game Play: vary activities and pacing; keep the
  elements consistent with the setting and story; present the overriding goal
  early and short-term goals throughout; let the player discover the story as
  part of play; make the world remember the player's passage, so what they
  changed is still changed when they come back; and under Game Story: one
  consistent vision; a story the player is interested in and thinks about;
  fairness of outcomes; characters that interest the player and develop as the
  action goes. Six of the eight story heuristics found real issues on a
  prototype that had no play in it yet, which is the state a script is in.
  **Cited:** Desurvire, Caplan & Toth (CHI 2004).
- **Severity is a second pass, after the finding pass, and one rater's number
  orders the work rather than deciding it.** Nielsen rates each problem 0–4 —
  not a problem; cosmetic; minor; major; a catastrophe to fix before release —
  from three factors: how often it is met, how hard it is to get past, and
  whether it keeps hurting after the first time. Ratings are collected after the
  evaluation, because an evaluator who is finding problems rates them badly;
  one evaluator's ratings are too unreliable to be trusted alone, and are used
  to put the fixes in order. **Cited:** Nielsen, *Severity Ratings for
  Usability Problems*.
- **A quest with little or no story around it is a fetch quest, and an errand
  is not one when it leads to a conversation, a choice or a secret.** The
  Witcher 3's designers declared war on the first kind and kept the second;
  a character who needs a whole story is given one or removed. **Cited:**
  Tomaszkiewicz and Sasko, in MCV/Develop.
- **Fun is learning a pattern; a pattern mastered and then repeated is
  boredom.** Koster's test of a good game is that it teaches everything it has
  before the player stops; the same deed again, with nothing new to learn, is
  the signal of failure. **Cited:** Koster, *A Theory of Fun*.
- **A decision is interesting when it is a trade-off, depends on the situation,
  lasts, and is made with enough information**; a set of options one of which
  everyone picks is not a decision. **Cited:** Meier, *Interesting Decisions*
  (GDC 2012), as reported by Game Developer.
- **A mystery plays fair.** The reader has the detective's opportunity: every
  clue plainly stated; the truth apparent on a second reading; the culprit
  found by deduction, never by accident or an unmotivated confession; motives
  personal. **Cited:** Van Dine, *Twenty Rules* (1928), rules 1, 5, 15 and 19.
- **A marker supports exploration and never replaces it; a detective quest
  marks no clue.** **Cited:** TES Renewal: Skywind, *Quest Design Principles*
  (ideas only, no licence stated).
- **What pulls a player is competence, autonomy and relatedness** — doing a
  thing well, choosing, and mattering to someone — so "was it fun" is answered
  by naming which of the three a stretch of play gave. **Cited:** Ryan, Rigby &
  Przybylski (2006).
- **A return to a place is worth the walk when the place has changed or the way
  carries something new**; the spatial rule and its sources are
  [place-craft.md §11](place-craft.md#11-the-way-between-places). **Cited** there.

## 2. The questions

Ask these of the script in play order, then of the whole. Each is answered with
findings — where, what the player meets, why it hurts — never with a yes.
`scripts/script-review.py` reads this section's bullets, each cut before its
**Cited** or **Authored** tail, into the reviewer's handout, so a question edited
here is the question asked.

- **Q1 — What is this for?** Every task the player is set has a reason in the
  story they have been told before it is set, and finishing it changes
  something they can see. A task whose only purpose is to be finished — three
  things found so that three things have been found — or whose result the story
  never picks up again, is unmotivated, however pleasant the doing. **Cited:**
  Tomaszkiewicz (a fetch quest is one with little or no story around it; an
  errand that leads somewhere is not one); Desurvire et al., Game Play 2 and
  Game Story 1. The "told before it is set" clause is **authored**: a reason
  the player learns afterwards did not move them.
- **Q2 — What is new here?** Each line's body is a different kind of deed from
  the lines before it, and a deed repeated inside a line asks something new of
  the player each time — a new way to find it, a new thing to read, a rule
  learnt on the first that the second uses. The same deed again with nothing new
  to learn is boredom, and three talks in a row are one deed three times.
  **Cited:** Koster; Desurvire et al., Game Play 1. Reading a line as its kinds
  of deed is **authored**, on Doran & Parberry's atomic actions.
- **Q3 — Where does it send me, and what is there?** For every leg the script
  walks the party, a place reached again carries something new — a change the
  party made, a person moved, a thing that was not there — and a leg that only
  carries the party back to where the task was set, to say it is done, is a
  defect when the deed could be closed where it was done. Count the crossings
  of the map a line asks for and ask what each one is for. **Cited:** Desurvire
  et al., Game Play 12 (the world remembers the player's passage); place-craft
  §11. The hand-in clause is **authored**.
- **Q4 — How do I find it, and is that how the story would have me find it?**
  Where the story has given the player a reason to look — a sound, a thing
  described, a clue in someone's words — the compass and the announcement are
  not what finds it, and a compass pointing at the thing the story set the
  player to search for is a defect. Where nothing has told the player, the text
  does; a search with nothing to go on is the other defect. **Cited:** TES
  Renewal: Skywind (markers support exploration, never replace it; a detective
  quest marks no clue); the information rule is
  [game-writing.md L1](game-writing.md#8-the-level-is-what-the-player-reads).
  The two-sided form is **authored**.
- **Q5 — Does the word match the thing?** Every title, hint and answer names
  the object the player will meet by the word a player would use for what is in
  front of them, and names one object where look-alikes stand; a word the level
  shows as something else sends the player to the wrong thing. The rules are
  [game-writing.md F1 and L2](game-writing.md#1-two-registers-never-mixed) and
  the content page's rule that a line names a thing as the player sees it; the
  question is whether the script keeps them at every object. **Authored**,
  from a playtest.
- **Q6 — Why does this person do that?** Every act of a character the player
  meets — a refusal, a lie, a forgetting, a favour, a change of subject — has a
  motive the player can read from the lines or the events by the time it
  matters, and no character's own lines contradict it. An act with no motive is
  a hole; a character who exists for one errand is cut or given a reason.
  **Cited:** Van Dine, rules 5 and 19; Desurvire et al., Game Story 8; Sasko.
  The "by the time it matters" clause is **authored**.
- **Q7 — Does each conversation earn its place?** A conversation changes what
  the player knows, has or can do, and two conversations of one shape in a row
  — the same question put to everyone — are padding. Where one person's manner
  alone would give the answer, the round of asking is the defect, not the
  manner. **Cited:** Slabinski (concision; dialogue is for the player); the
  table-read finding that a scene drags. The "one shape in a row" reading is
  **authored**.
- **Q8 — Is this a choice?** A set of options is a choice when it is a
  trade-off, depends on the situation, lasts, and is made with enough
  information; a set with one obvious pick, or whose options come to the same,
  is not one and is not dressed as one. **Cited:** Meier; Desurvire et al.,
  Game Story 6 (fairness of outcomes).
- **Q9 — Could I have solved it from what I was shown?** For a puzzle or an
  investigation: every clue the answer needs is shown before the answer is
  asked; the answer follows from the clues and from nothing else; and the game
  takes the answer however the player reached it. **Cited:** Van Dine, rules 1
  and 15; the knowledge-gate rule is
  [web-narrative.md S4](web-narrative.md#2-structures).
- **Q10 — Is it one story, and the one that was asked for?** Everything the
  player does belongs to the premise they were given at the start; the ending
  reads what they did; and what the idea pinned — a tone, a thing that must be
  absent — holds in every line. **Cited:** Desurvire et al., Game Story 1;
  [web-narrative.md §3](web-narrative.md#3-endings-that-read-the-run). The
  pinned-idea clause is **authored**.
- **Q11 — What would a player tell a friend?** Before any finding, name what
  struck: the deed that was new, the line that landed, the moment a player
  would retell. What works is kept only if somebody names it. **Cited:**
  Lerman, step 1.

## 3. The review

The reviewer writes one document, in this order. `scripts/script-review.py`
reads this section into the handout too.

1. **What works** — Q11's statements, three to six lines, each naming where.
2. **The run, retold** — what a player would say happened, in a few neutral
   lines with no judgement in them, so the author sees what reached a reader.
3. **Findings** — one per problem. Each names **where** (the file and the id:
   `quests.json` `quest/…` `obj/…`; `dialogue.json` `dlg/…`; a trigger, shop or
   sign id), quotes **what the player meets** there, names **the question** it
   fails, and says **why it hurts play** in one or two sentences. A fix may be
   suggested to illustrate, never to prescribe. "The middle stretch is dull" is
   not a finding; the objective that makes it dull, quoted, is.
4. **Severity**, written after every finding is down and never while finding,
   one per finding: **0** not a problem · **1** a line to mend if there is time
   · **2** dulls a beat · **3** a stretch of play not worth playing · **4** the
   delve is not worth playing until this is fixed. Weigh how many players meet
   it and how often, whether it stops the fun or only dulls it, and whether it
   keeps hurting after the first time.
5. **The score** — a table with one row per question Q1–Q10: findings counted,
   worst severity. Never one number for the whole delve.
6. **The verdict** — one word, derived from the table: **build** when nothing
   stands at 3 or 4; **revise** when something stands at 3; **rethink** when
   anything stands at 4, or three findings stand at 3 under one question.

The form is **authored**, from the cited practice: the order of 1–3 is script
coverage's with Lerman's opening; the note shape is Catmull's; the scale and
its second pass are Nielsen's; the per-question table is heuristic evaluation's
record.

## 4. What the review is not

- **Not a proof.** Reachability, deadlock and every DW code are the engine's
  (`delvec analyze`, `delvec build`); the reviewer reads what those cannot.
- **Not a line edit.** The rules for how a line is written are game-writing.md
  F1–F7, S1–S6 and N1–N6; the reviewer names a line only where it fails a
  question above.
- **Not the user's yes.** The design gate is the user's; a reviewer's `build`
  is admission to step 6, nothing more (`references/design-gate.md`, the stop).
- **Not a measurement of fun.** One reader's severities order the author's work
  (Nielsen); the playtest measures the rest.
- **The design record is not a defence.** The reviewer reads what the player
  meets and nothing the author meant; an intent the script does not carry is a
  finding, and the remedy is in the script.

All **authored**.

## Sources

- Catmull, E. with Wallace, A. *Creativity, Inc.* (Random House, 2014): the Braintrust and "good notes", as excerpted in the author's blog and in First Round Review's interview coverage. Copyrighted; one sentence quoted for criticism.
- Celtx Blog, *What Is a Table Read?*; Casting Networks, *What Are Table Reads?* Trade guides; ideas only.
- Desurvire, H., Caplan, M. & Toth, J. A. *Using Heuristics to Evaluate the Playability of Games.* CHI 2004 Extended Abstracts. ACM; heuristics paraphrased.
- Doran, J. & Parberry, I. *A Prototype Quest Generator Based on a Structural Analysis of Quests from Four MMORPGs.* PCGames 2011 (as cited in game-writing.md).
- glcoverage.com, *What Is Script Coverage?*; Filmustage, *The Basics of Script Coverage*; writing-world.com, *Coverage*. Trade descriptions; ideas only.
- Koster, R. *A Theory of Fun for Game Design* (2nd ed., O'Reilly, 2013). Copyrighted; ideas only.
- Lerman, L. & Borstel, J. *Critical Response Process*, lizlerman.com. Copyrighted; the four step names paraphrased.
- MCV/Develop. *The Witcher 3 quest designers talk balancing story with gameplay and declaring war on fetch quests* (Tomaszkiewicz, Sasko). Press interview; ideas only.
- Meier, S. *Interesting Decisions*, GDC 2012, as reported in Game Developer, *GDC 2012: Sid Meier on how to see games as sets of interesting decisions*. Ideas only.
- Nielsen, J. *Severity Ratings for Usability Problems.* Nielsen Norman Group, 1994. Copyrighted; the scale paraphrased.
- Ryan, R. M., Rigby, C. S. & Przybylski, A. *The Motivational Pull of Video Games: A Self-Determination Theory Approach.* Motivation and Emotion 30(4), 2006. Ideas only.
- Slabinski, M. *8 Key Principles of Writing Effective Game Dialogue.* Game Developer (as cited in game-writing.md).
- TES Renewal: Skywind. *Quest Design Principles.* handbook.tesrskywind.com/dev/Principles. No licence stated; ideas only.
- Van Dine, S. S. *Twenty Rules for Writing Detective Stories.* The American Magazine, September 1928. Public domain in the United States.
