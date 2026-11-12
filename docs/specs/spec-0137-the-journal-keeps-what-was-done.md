# spec-0137: The journal keeps what was done — a finished objective's page says what the task was and which way it ended, the story writes the way where it decides it, and a task nobody announced is recorded once it is over

- **Status**: Proposed
- **Ground**: written against `feature/festival-day` at `d54e27128` and held against `af68169c4`, which changes no file named here (`delvec 1.16.0`, `dsl 0.75.0`), read for: `Objective` and its `title`, `hint`, `announcement`, `limit` and `Limit::on_expire` in `crates/dsl/src/quest/objective.rs`; `Verb` in `quest/verb.rs` and the envelope (`when`, `audience`, `in`) in `quest/effect.rs`; `DialogueOption` and `DialogueEffect` in `crates/dsl/src/dialogue.rs`; the key table and the position-derived `fx.…` keys in `crates/dsl/src/l10n.rs`; the chrome strings `journal.*` in `chrome.rs`; `Guide`, `item_modifiers`, `journal_ledger`, `journal_rewrite` and `journal_write` in `crates/delvec/src/compiler/emit/guidance.rs`; `told_guard` and `pending_guard` in `emit/quest.rs`; `expired_score` in `emit/limit.rs`; `check_book_pages` and `page_lines` in `compiler/textfit.rs`; the story worlds and `Elective` in `compiler/flow`; `harness/src/journal.ts` and `harness/src/executor/journal.ts`; spec-0104 §8, spec-0133 §3 and §5, spec-0039.
- **What it is for**: a player who opens the journal an hour after a task and wants to know what it was and what the party did about it. A finished objective is one page holding its title struck through, and nothing else; a task that can be done two ways — a horse rented for coin, or taken after its keeper was talked into leaving — reads the same on the page whichever was done, and a task the campaign never announced leaves no page at all. This spec gives a finished page the words for what the task was and for the way it ended, written by the creator and chosen by what the story did.
- **Research**: `docs/reference/journal-records.md`, sources S1–S4. Every practice below is **[cited]** from it; every rule is **[authored]** here.
- **Numbers**: spec `0137`; `dsl_version` `0.76.0`; `DW1104`, used (§8). `DW1105` and `DW1106` were handed and are not used: §10 says why the first has no rule left to carry and names the rule the second would carry.
- **Amended at the demo's rulings**: three open questions were answered before the implementation landed, and the text below is the answered one. (1) A task that runs out of time is a branch like any other: its way is a `record-outcome` in its `on_expire`, and its finished entry is the done entry — no heading or page of its own. (2) Every line the story wrote is kept, in the order the story enforces; nothing is replaced. (3) A hidden task is hidden only before it starts: once it is under way its authored hint is on its journal page, and its completion is told as any task's is (§3).
- **Non-goals**: a journal that narrates the story (a page says what was done, in the creator's words, and no more); a quest-level summary (the journal's entry is the objective, spec-0104 §8.2); any change to the compass or to the announcement of an announced objective; a line the engine composes from what it observed (§10).

## 1. The entry, and who owns each line **[authored, on cited practice]**

**[cited]** (spec-0104 §8.2; `item_modifiers`): the journal's entry is the objective. An announced objective has an open page while it is pending and a done page, `journal_done_<obj>`, once the party's completion score is 1: the chrome heading and the title struck through.

**[cited]** (S1, S3): an established journal keeps two kinds of words for a finished task. The task's own — what it was. And the way it ended — one entry per ending, of which the player sees the one their play reached ("Multiple stage items are often conditionalized so that only one is actually written to the player's log"; two stages that both finish one quest, each with its own entry). The entry is written by the story at the moment it turns: "Setting a quest stage … adds the Log Entry to the player's quest log", and a stage is set by the answer or the event that reached it.

**[authored]** The two kinds have two owners here.

- **What the task was** is a fact about the objective, the same on every playthrough. It is a field of the objective: `record`.
- **Which way it ended** is decided by an event of the story — an answer chosen, a trigger fired, the objective completed while the party holds some state, a clock run out. The line belongs to that event. Every one of those events already carries an effect bundle, so the line is an effect in it: `record-outcome`. The story writes the line by happening.

**Rejected: a list of gated lines on the objective** (`outcomes: [{when, line}]`, the first whose gate holds). The story already decided the way at the answer; a gate on the objective asks the creator to mint a flag only so the journal can read it, and then to state a second time, in flags, which line goes with which answer. Two statements of one fact can disagree, and the disagreement is a wrong sentence in the player's book. **Rejected: a `journal` field on the dialogue option, on the branch and on the limit.** Three fields on three classes for one capability; the effect bundle is the one thing all three already carry. **Rejected: the line named on the objective and pointed at from the story** (S1's own shape, a table of stages set by number): it puts the words in one place and the decision in another, joined by an id the creator types twice; the page in §9 that lists every objective's lines gives the reviewer the table without the second id.

## 2. The surface **[authored]**

### 2.1 `record`, on every objective kind

```json
{ "type": "reach-anchor", "id": "get-a-horse", "title": "Find a horse",
  "announcement": "hidden",
  "record": "The road north was too long to walk, and the stable had one horse left." }
```

`record` is one line of the span grammar: what the task was, written to be read after it is over. Optional. Inventoried as `obj.<quest>.<obj>.record` (narration) beside `.title` and `.hint`. It needs no `title` and no announcement.

### 2.2 `record-outcome {objective, line}`, an effect

```json
{ "type": "record-outcome", "objective": "get-a-horse",
  "line": "We paid the keeper his twelve coins and rode out by the gate." }
```

It writes `line` as the way `objective` ended. It is a verb like any other: it sits in any effect bundle — a quest's `on_complete` and `on_objective_complete`, a trigger's effects, a `limit`'s `on_expire`, a step of a `sequence` — and carries the envelope's `when`, so a completion bundle may hold two of them behind two gates on the state the party holds. `DialogueEffect` gains the same arm, as it mirrors `set-flag`: an answer writes the way it chose.

It addresses no player and changes nothing a proof reads: `audience` and `in` are refused on it as on every verb that addresses no player, and it keeps no cadence (`DW1102`). `objective` names any objective of the campaign, in any quest (`DW0112` when none has that id). `line` is inventoried under the effect's position-derived key with the leaf `.outcome` (`fx.….outcome`); on a dialogue option, `dlg.<npc>.<node>.opt.<i>.fx.<j>.outcome`.

### 2.3 The owner's example, whole

The stable task has one `reach-anchor` objective, unannounced. The keeper's dialogue has an answer that pays him: its effects take the coins and carry `record-outcome` *"We paid the keeper his twelve coins…"*. A hidden answer elsewhere sends him away: its effects carry `record-outcome` *"We sent the keeper off on an errand, and took the horse while he was gone."*. Nothing else is authored. Whichever answer the party gave is the line on its page once the horse is reached — both, in the order the story enforces, if it gave both.

## 3. Which objectives the journal records **[authored, on a cited principle]**

An objective is **recorded** when it carries `record` or any `record-outcome` names it. The journal keeps a finished entry for an objective that has an open page or is recorded.

**A hidden task is hidden only before it starts.** An unannounced objective — no `title`, or a resolved `announcement` of `hidden` — is told nothing before it starts: no announcement in chat, no compass, no page. **[cited]** (`game-writing.md` §8 L1): the journal "never names a place, person or thing the player has not been told". Once it has started (its told guard holds, as an announced objective's open page does), a hidden task with a `title` and a `hint` has an open page holding both, and nothing in chat and nothing on the compass: the creator wrote the hint for a task under way, and L1 admits what the play has begun. When it is completed, every objective with a `title` tells its completion — the `Objective complete` line and its cue — announced or not. After it is over the party has been told everything on its finished entry by doing it, so L1 admits that too. **[cited]** (S3): "which entries appear and which entries do not depends on the manner in which the quest is done." **[authored]**, and named as such: no source read gives a rule for a task the player was never told about (`journal-records.md` §3); this one is derived from L1. `DW0862` refuses a `hint` only on an objective with no `title`, which has no page to put it on. `DW0863` owes a hidden task nothing (spec-0104 §4 as amended): before it starts nothing is told, and after it starts its hint, where one is authored, is what makes it legible, so the rule never asks one to be announced.

An unannounced objective with no hint that is not recorded has no page, as now. A campaign carries a journal when any objective has an open page or is recorded, or `guidance.manual` is declared.

## 4. Which lines are shown **[authored, on cited practice]**

**Every line the story wrote, in the order the story enforces.** **[cited]** (S1): an entry is written by the stage that reached it, and (S3) a finished quest "may still receive new entries" — a log keeps what it was given. A task with several checkpoints has a line for each branch at each checkpoint, and the finished entry shows every line its playthrough wrote. Each `record-outcome` is one **site**, and each site has one party value, its **held** score: 0, or 1 once the site has fired. A site's ordinal is its 1-based position among the sites naming that objective, in the order of the one campaign effect walk, a dialogue option's after the quest effects, in document order (`dsl::record::outcome_sites`); it names the site and never orders the page.

**The order is the story's.** A finished entry shows its lines in the objective's **display order**: site `a` before site `b` when some story world's walk fires `a` before `b` and none fires `b` before `a`; the document's order between sites the story does not order. Within one step of a walk, a dialogue option's lines come before the bundle its completion fires.

So a line may be written before its objective opens (an answer given in an earlier scene), while it is open, as it settles, or after (the horse brought back): it is held from the moment it is written, and the entry shows it from the moment the objective settles. Nothing clears it. It is the party's, as the objective's completion is: one player's answer writes the entry every player reads.

**No line written is a legitimate way.** A recorded objective that settles with no line held shows its title and its `record` and nothing more.

## 5. The entry **[authored]**

A finished entry is a **head page** and one **line page** per line held, in display order. The head page holds, separated by a blank line: the chrome heading `journal.done`; the title struck through, when the objective has one; the `record`, when it has one. A head page that would hold the heading alone is not written. A line page holds the line alone.

**Settled is done or expired.** An objective settles when its completion score is 1, or — for an objective with a `limit` — when its settled score (spec-0133, done or expired) is 1. An expired task is a finished entry like any other: the same heading, the same struck title, and the creator's `on_expire` line, if one is written, says how it was lost. There is no expired heading and no expired page.

**Every page is a committed file.** **[cited]** (spec-0104 §8.3): the book is rewritten by static item modifiers, one whole page each, applied under guards. The head page is `journal_done_<obj>`; line `k`'s page is `journal_done_<obj>.<k>`. `.` is never in an objective's emitted name, so no two objectives' files meet. An objective with *n* sites has at most 1 + *n* files, whatever subset of its lines a play writes, and each is measured alone (§6).

**The rewrite.** `journal_write` appends, after the open pages and in document order, each journaled objective's head page under its settle guard, then each line page under the settle guard and its held score (`if score #jr_<obj>.<k> dw.guide matches 1`), in display order. An announced objective with no `record`, no site and no `limit` emits the bytes it emits now. `record-outcome` is two lines: set `#jr_<obj>.<k> dw.guide` to 1, and add 1 to `#epoch dw.guide`, so every player's book is rewritten on the next scan whether the objective settled before or after. Every held score is set to 0 at `setup` (`DW0495` holds the read to it).

## 6. The words: language and fit **[authored, on cited records]**

**[cited]** (`i18n.md`; `l10n.rs`): every authored line is inventoried under a key, transcreated per declared language, and lowered through `emit::tr` to a translate component with its English fallback. `record` and each `line` are two more kinds of authored narration under the keys of §2.

**[cited]** (spec-0104 §8.8; `check_book_pages`): `DW0330` measures every page the engine writes against a book page — 114 px wide, 14 lines — in the source and every declared language. Its population gains every page of a recorded objective's finished entry: the head page, measured as composed (heading, title, `record`, the blank lines between), and each line page alone. A page over the box is a `DW0330` warning naming the objective, for a line its site, and the language. The measured set is returned with the others, so its count is stated.

## 7. How the lines are written — a rule for the skill **[authored, on cited practice]**

Lands in the `/new-delve` page, not in a check. **[cited]** (S3): a finishing entry states the deed, in the past, in a sentence or two, and names what was done and to whom. **[authored]**: `record` says what the task was and never how it went; an outcome line says what the party did and never what it will lead to; each stands alone, because the page shows one outcome and the reader may have forgotten the scene; the person and voice are the campaign's (**[cited]** `journal-records.md` §3: no source fixes them for a party). A task with one way needs no outcome line. A line is written where the way is decided, which is where the creator is already writing the answer.

## 8. What is proven, and at which layer **[authored]**

**`DW1104` — a way no story reaches.** A `record-outcome` whose line no story world can show: in every enumerated world (the worlds `compiler::flow` enumerates, expiry witnesses included), either the site cannot fire or the objective it names never settles. A site can fire in a world when that world's walk fires it, when it stands at an ambient root the walk drives out of order, when it is a bundle of an act the world performs whose `when` flags the world holds, or when it is a dialogue option whose `requires_flags` the world holds. The creator declared a line for a way the task ends; the check confirms such a way exists. A build-tier refusal naming the site and the objective, bound in the validation funnel with its line (`records binding: …`). It does not ask that every way have a line (§4). With every line kept, no line is replaced, so a line that fires is a line shown.

**The ledger.** A build whose campaign has a recorded objective writes `validation/journal-records.json`: `party`; `objectives`, one per recorded objective in document order — `objective`, `announced`, `done` and `expired` (its settle scores; `expired` null without a limit), `title_key`, `record_key`, and `outcomes` in display order (each `{ordinal, key, site, held}`); `mainline`, the world the critical path walks; and `worlds`, one per coherent story world — `world`, and `finished`, one row per recorded objective: `settles` (`done`, `expired` or `open`, as that world's walk leaves it) and `written` (the ordinals that world's walk leaves held, in display order, `null` where a site of the objective stands at an ambient root the walk drives out of order). Keys are `null` in a `--lang` bake, as `journal-starts.json`'s are. It carries the counts: recorded objectives, sites, rows, and rows with a `null`.

**The bot.** `harness/src/journal.ts` reads the ledger beside the path it walks. At the boundary after the walk's last step, inside the settle window it already keeps, for each row of its world: a row that `settles` must show the objective's `record_key` (when it has one) and every written ordinal's key on some page, and no page may carry an unwritten ordinal's key; a row that stays `open` must have no page carrying its `record_key` or any of its outcome keys. The world a run walks is named by its path file: `world-path-<k>.json` walks `world/<k>`, `critical-path.json` the ledger's `mainline`. The expectation comes from the compiler's world, never from the scores the page was selected by: a `record-outcome` that failed to write would leave the score at 0, and a bot that read the score would agree with the page. The run prints its binding: rows judged of rows owed, and the rows unjudged with the reason (`null` written, a `--lang` bake). `critical-path.json` and its format are unchanged.

**PackTest.** Generated `guide_journal_record_<obj>`, one per recorded objective, beside `guide_journal_pages`: as the dummy, the settle score and the held scores are set as the emitted functions set them, `journal_rewrite` runs, and the book holds exactly the pages of that state — unsettled with every line held (no page of the entry), settled with none held (the head alone), settled with each line held alone, and settled with every line held.

| check | judges | layer |
|---|---|---|
| a written book takes appended pages from guarded item modifiers; a translate component in a page | vanilla behaviour | established (spec-0104 §8, read sites in `compiler.md`'s `journal` row); nothing recurring is added |
| the schema, the ordinals, each page file's composition, `journal_write`'s guards, the two lines of `record-outcome`, the unchanged bytes of an unrecorded campaign, the ledger | the compiler's output | `crates/delvec/tests/it/journal_records.rs` |
| `DW1104`; `DW0330` over the finished pages | the compiler's output | `journal_records.rs`, and the gallery probe |
| on this campaign's server, the book shows the page of the state held | a campaign's content | PackTest `guide_journal_record_<obj>` |
| on each walked world, the page the party ends with is the way that world took | a campaign's content | the bot, against `journal-records.json` |

## 9. The rules, the gallery and the pages **[authored, under spec-0039]**

| code | module | refuses |
|---|---|---|
| `DW1104` | `compiler::recording` | a `record-outcome` that can fire in no story world in which its objective settles |

Reused: `DW0112` (an objective no quest declares), `DW0100` (an empty `record` or `line`), `DW0942`'s rule for `audience` and `in` on a verb that addresses no player, `DW1102` (a cadence on it), `DW0330` (a finished page over the box), `DW0495` (every held score is seeded), and `DW0862` narrowed to a hint with no `title` (§3).

**The gallery** binds every unit the schema gains: `record` on an objective of each of the five kinds; `record-outcome` with each of its two fields in a completion bundle, behind a `when`, in a trigger's effects, in an `on_expire`, and as a dialogue option's effect; one objective that is unannounced and recorded; one that is limited and recorded. Each bound element moves an emitted byte when perturbed: a changed line moves its page file; a removed site removes its page file and renumbers the guards after it. One probe for `DW1104`, `a-way-the-story-never-takes`: the primary plus one `record-outcome` behind a flag no world holds when its bundle fires.

**The demo** is the town under construction (`foot-of-the-mountain`): its unannounced horse task carries the two lines of §2.3, and its limited tasks carry a `record`, a line for the way they are done and an `on_expire` line for the way they are lost. No demo level is added.

**The reviewer's table.** `delvec analyze` prints, per recorded objective, its title, its `record`, and each line, in display order, with the site that writes it and the count of story worlds whose walk ends holding it, of the worlds in which the objective settles — and the count that end holding none. A way with no line is seen there as a number, never refused.

**Pages**: `compiler.md` (the objective and effect rows, the `journal` and guidance-refresh rows, the artifact list, `DW1104`, the versions line); `i18n.md` (the keys); `tools.md` (the harness's second ledger); `game-writing.md` §8 L1 (a pointer to §7 here for the finished page); `docs/reference/journal-records.md` (the research); the `/new-delve` page `references/quest-capabilities.md` (*What the journal keeps*, with §7's rule as a mandatory step of quest expansion).

## 10. What it leaves out, by name **[authored]**

**A line that is always replaced.** With every line kept (§4) no line is replaced, so there is no such line and no rule for `DW1105` to carry. **A sentence that names what the party has not met.** A finished entry is read after the fact, so no telling check (`DW0982`) is run on `record` or `line`; a line written on an early answer for an objective far ahead could name something the scene between has not shown. It is the rule `DW1106` would carry. **A hidden task's hint judged by the telling check**: `DW0982` reads an announced objective's title and hint; a hidden task's hint on its open page is not yet judged by it. **An outcome on an open page** (what the party has done so far in a task still open). **A line the engine writes itself** from the branch taken or the items held: the engine has no words. **Telling a player that the book gained a page** for an unannounced task: unguided stays silent. **A per-player outcome**: the journal's sections are the party's. **A finished page per quest.** **Whether the words read well**, and whether a player opens the book: the playtest judges both. **`DialogueEffect` as a second, narrower effect list**: this spec adds its one arm there and does not merge the two.

## Acceptance criteria

1. **Surfaces.** `delvec schema --stage all` exports `record` on each of the 5 objective kinds, the verb `record-outcome` with `objective` and `line`, and the same arm on the dialogue effect, at `dsl_version` 0.76.0; every document the repository holds parses. *Judge*: `journal_records.rs::the_schema_carries_the_record_and_the_outcome`; `version_sites.py verify`. *Gallery*: every unit bound (criterion 9).
2. **Ordinals and files.** For a campaign with one objective that has a `record` and a limit and is named by 4 sites (a completion bundle, the quest's `on_complete`, its `on_expire`, a dialogue option): one head page file and 4 line page files, ordinals 1–4 in walk order with the option last, each file's page composed as §5 states, and no expired file. *Judge*: `journal_records.rs::each_state_of_a_finished_entry_is_one_committed_page`. *Denominator*: 5 of 5 files read. *Gallery*: the limited recorded objective.
3. **The rewrite and the write.** `journal_write` holds one guarded line per page of the entry: the head under the settle guard, each line page under the settle guard and its own held score; `record-outcome` emits exactly the set and the epoch line, from a quest bundle and from a dialogue option alike; `setup` seeds each held score. *Judge*: `journal_records.rs::the_story_writes_the_way_and_the_book_follows`. *Gallery*: the dialogue option's outcome.
4. **Nothing moves for a campaign that writes no line.** A fixture campaign with announced objectives and no `limit`, no `record` and no `record-outcome` emits `journal_write`'s finished lines and each `journal_done_<obj>` equal to the strings the test holds, and no `#jr_` score, no `.<k>` page and no `journal-records.json`; perturbation — adding one `record` adds exactly that objective's page bytes, the key and the ledger. *Judge*: `journal_records.rs::an_unrecorded_campaign_emits_what_it_emitted`, and the gallery baseline's diff, every moved row attributed. *Denominator*: the baseline's row count, stated in the commit body.
5. **A hidden task is hidden only before it starts.** An unannounced objective with a `record`: no open page file, no announcement, a done page, and its completion told; the same objective without `record`, site or hint: no page at all; with a hint: an open page and still no announcement. *Judge*: `journal_records.rs::a_task_nobody_announced_is_kept_once_done`. *Gallery*: the unannounced recorded objective.
6. **`DW1104`.** Refused: a site behind a flag nothing sets; a site on an `on_expire` of an objective whose expiry no world performs; a site naming an objective that settles in no world. Passing: the same sites reachable; a recorded objective one of whose ways has no line. *Judge*: `journal_records.rs::dw1104_a_way_no_story_reaches` (3 refused shapes, 2 passing); the gallery probe `a-way-the-story-never-takes` refused with `DW1104`.
7. **Fit and language.** `check_book_pages` returns one measurement per page of each recorded objective's entry per language shown, and an overlong `record` and an overlong line each warn `DW0330` naming the objective, for the line its site, and the language; the l10n inventory lists `obj.<quest>.<obj>.record`, each `fx.….outcome` and each option's `.outcome` key, and a sidecar's line reaches the page. *Judge*: `journal_records.rs::a_finished_page_fits_in_every_language`; `crates/dsl/tests/it/l10n_surface.rs`. *Denominator*: pages measured = Σ over recorded objectives of (1 + sites) × languages, asserted as a number.
8. **The server shows the pages held.** PackTest `guide_journal_record_<obj>` is generated for every recorded objective and passes on the gallery; the generator's count equals the ledger's `objectives` length. Perturbation: with one `journal_write` guard's ordinal changed in the built pack, that objective's test reds and no other test does. *Judge*: the PackTest tier on the gallery. *Denominator*: tests generated of recorded objectives.
9. **Each walked world ends on its own way.** The bot's journal judgement over the gallery reports every row judged or named unjudged with its reason, 0 failed, and a non-zero judged count; in the gallery two story worlds end one objective holding two different lines and each walk reads its own. Perturbation: with a `record-outcome`'s set line removed from the built pack, the walk of the world that takes that way reds on that row. *Judge*: the gallery bot job; `harness` unit tests over `journal.ts` (a missing page, a sibling's key present, an open unannounced row with a page, a `null` row counted). *Gallery*: `check-gallery-coverage.py` reports 0 in neither state with the units of §9 bound.
10. **Codes and pages.** `check-dw-codes.py`, `check-diagnostic-messages.py`, `check-capability-ownership.py`, `check-effect-roots.py`, `check-skill-page.py`, `check-stated-counts.py` and `version_sites.py verify` are green; `compiler.md` carries `DW1104` and no row for `DW1105` or `DW1106`.

## Measurements this spec states

Written into the commit bodies that land the implementation: the gallery baseline's rows moved, each attributed; the count of finished-entry pages measured by `DW0330` with its formula's terms; the bot's rows judged of rows owed per world; on the town, the recorded objectives and sites, and the reviewer's table's worlds-per-line.
