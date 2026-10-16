# spec-0114: An objective says the act that completes it — the title's head word is read against the act the completion performs, a disagreement is refused, and an item laid at a place leaves the hand

- **Status**: Proposed
- **Ground**: written against engine `e7bd149428a6768b71d9f6e08746bfec917fc0ac` (`origin/main`), read only — `Objective` and its five kinds in `crates/dsl/src/quest/objective.rs`; `Quest.on_objective_complete` in `crates/dsl/src/quest/mod.rs`; `DialogueEffect::CompleteObjective` and its `talk-to`-only rule in `crates/dsl/src/dialogue.rs`; `DialogueOption` (no item gate) in the same file; `Verb::GiveItem` in `crates/dsl/src/quest/verb.rs`; the per-kind completion lines of `crates/delvec/src/compiler/emit/functions.rs` (the `interact` hand guard `if items entity @s weapon.mainhand <item>`, the `collect` inventory count, the `kill` counter, the `reach-anchor` volume); `Happening`/`HappeningVerb` in `crates/dsl/src/quest_plan.rs`; `key_situations` in `crates/dsl/src/l10n.rs`; the `telling` module and its page `docs/reference/delvec/compiler/telling.md`; `docs/reference/game-writing.md` §1 F1 and §8 L2; `docs/reference/i18n.md` (the sidecar's `source` map, `DW0187`/`DW0188`); the stage-5 rows of `docs/reference/compiler.md`. The unit vocabulary of §4 is derived from `delvec schema --stage all` of that revision, built from source in the tree that wrote this spec. Content read: `stellarfeline/delvewright-campaigns` at `7a67e92d77a3831cd3aca26005b84a1862e1c02e` (content `main`, five campaigns) and at `fa5e9246` (`campaigns/the-muster-loft`), and the gallery with its four quest-carrying overlays at the engine revision.
- **What it is for**: a player is told to do one thing and the game rewards another. In The Muster Loft, `obj/lay-the-roll` is titled *Lay the roll in the loft* and is a `reach-anchor`: it completes when a body stands in the loft, and nothing is laid. The same shape stands in four of the five campaigns on content `main` and in the engine's own gallery overlays (§1). No check reads it; the title is prose the compiler carries and never judges. Every objective kind completes on one act a player performs, and the title states one act; this spec makes the engine decide, deterministically, whether they are the same act, and refuse when they are not. It also closes the one capability the motivating shape needs for its honest repair: an item presented at a place that stays there, so that *lay*, *hang* and *give* can be true.
- **Research**: §2 is the record. Each statement is marked **cited** (a published source, a line of the tree at the ground revision, a reading taken by running that tree, or a constitution rule) or **authored** (this spec chooses). The published sources are listed under *Sources* and entered in `docs/reference/game-writing.md`.
- **Numbers**: spec `0114`. One new diagnostic, `DW1045` (§7). One more is needed and is not allocated here: `DW-next` (`takes_item` with no `requires_item`, §6.3). The surface change (§6.3) is stage 5 and takes the unreleased `dsl_version` minor other unreleased surface changes share; this spec states no version literal.
- **Non-goals**: judging the `hint` (it names the place, not the act, and §8 says why the place is a separate question); judging a quest's `goal`; judging the words of a non-English line (§5 states what binds them); composing titles from templates (§3); a lexicon the campaign extends (§3); a declared `act` field (§3); an item handed to or taken from a body through dialogue (§6.4 records both as gaps); any change to what the five kinds complete on.

## 1. The defect, measured

**Cited** (content `fa5e9246`, `campaigns/the-muster-loft/quests.json`): `quest/the-roll` ends in `{"type": "reach-anchor", "anchor": "anchor/node-loft", "radius": 2, "title": "Lay the roll in the loft", "happening": {"verb": "loses", "subject": "item/the-levy-roll", …}}`. **Cited** (`emit/functions.rs`, the `reach-anchor` line; `compiler.md` stage-5 row): a `reach-anchor` completes when a player's hitbox is inside the box of half-extent `max(1, radius)` at the anchor. The roll is `minecraft:paper` taken by `obj/take-the-roll`; no command in the build moves, removes or reads it after that. The `happening` says the party *loses* the roll and the chronicle believes it; the player keeps it. Its sidecar row `obj.the-roll.lay-the-roll.title` is 把名册放进阁楼, transcreated from the English and carrying the same act.

**Cited — a reading of the corpus** (the instrument is §4's rule over §4's seed lexicon, run by hand over every `quests.json` named under *Ground*; the numbers are repeated in the commit that lands this spec): 127 objectives; 117 carry a `title`, 10 do not; 7 have a `give-item` effect in `on_objective_complete`. Every titled objective's head word is in the lexicon (0 unknown). 105 titles state an act their completion performs. 12 do not:

| Title | Where | Kind | The title says | The completion does |
|---|---|---|---|---|
| Speak the Ending | hollow-vigil | `reach-anchor` | talk | reach |
| Get Under a Ram | nobodys-cave-island | `interact` | reach | use |
| Climb up to Oru | the-treehouse-camp | `talk-to` | reach | talk |
| Bring both to Oru | the-treehouse-camp | `talk-to` | give | talk |
| Show Neve the lit lantern | the-treehouse-camp | `talk-to` | present | talk |
| Hang the lantern on the hook | the-treehouse-camp | `interact` + `requires_item` | give | use, present |
| Show Tamsin the tongue | vesperhold | `talk-to` | present | talk |
| Lay the roll in the loft | the-muster-loft | `reach-anchor` | give | reach |
| Reach the counter | gallery `overlays/site-plan` and `overlays/site-plan-solid` | `talk-to` | reach | talk |
| Climb the loft | gallery `overlays/site-plan` and `overlays/site-plan-solid` | `talk-to` | reach | talk |

A second reading, by hand and without the lexicon, of all 117 titles against the question *does the completion perform what the head word says*, finds the same 12 and no other. One title tells a different kind of lie the act check does not see: *Climb the cellar ladder* (the-muster-loft) is a `reach-anchor` at the ladder's **foot**; the act agrees (a movement) and the place does not (§8). Both readings are one reader's; the lexicon is the method that can be re-run.

## 2. What practice does

- **Cited** (Doran & Parberry 2011, §3 and Table 5): over 750 quests from four MMORPGs reduce to sequences of **atomic actions** — `capture`, `damage`, `defend`, `escort`, `exchange`, `experiment`, `explore`, `gather`, `give`, `goto`, `kill`, `listen`, `read`, `repair`, `report`, `spy`, `stealth`, `take`, `use` — each with a pre-condition and a **post-condition** that is the state the game checks: `goto` → *You are there*; `take`/`gather` → *You have it*; `give` → *They have it, and you don't*; `kill` → *They're dead*; `listen` → *You have some of their information*; `use` → *It has affected characters or environment*. An NPC's quest text is a "verb-noun pair" ("kill pests", "steal supplies") whose verb is one of these actions. The act and its completion are one object in this model.
- **Cited** (Aarseth 2005, as read by Karlsen 2008): quests are **place-oriented** ("getting to a specific location"), **time-oriented**, or **objective-oriented**, and combine. Getting somewhere is a kind of its own, distinct from doing something there.
- **Cited** (Minecraft wiki, *Advancement definition*, the pinned game's own quest primitive): an advancement's `display.title` and `display.description` are free text components; its `criteria` are typed `trigger`s with `conditions` — `minecraft:location`, `minecraft:inventory_changed`, `minecraft:player_killed_entity`, `minecraft:default_block_use`, `minecraft:player_interacted_with_entity`, `minecraft:item_used_on_block`. Nothing in the format binds the title to the trigger; the author binds them or does not.
- **Cited** (Creation Kit documentation, UESP's *Bethesda Tutorial Quest Objectives* and *SetObjectiveCompleted*): a quest objective is an index, a **display text**, and targets; it is completed by a stage's script calling `SetObjectiveCompleted(index)`. The display text is set in the objective's data, the completion in the stage; nothing holds the two together.
- **Cited** (World of Warcraft API, `GetQuestObjectiveInfo`): a quest objective is one record returning its text (`"0/3 Monsters slain"`), a typed token (`item`, `object`, `monster`, `reputation`, `log`, `event`, `player`, `progressbar`), and its `fulfilled`/`required` counts "without need to parse objective text manually". The tracker line is derived from the condition; the type is the condition's.
- **Cited** (Xbox Accessibility Guideline 109): "Descriptions of tasks and objectives should be clear and straightforward"; the examples it praises are prescriptive subtasks — *Unblock the obstructed laser* — and quests "broken down to clearly describe to players what tasks are needed", in language that is "concise and straightforward".
- **Cited** (Skywind development handbook, *Quest Design Principles*): "The quest must be fully completable without quest markers. This means descriptive objectives"; "markers are a subset of objectives, not the objective itself".
- **Cited** (`docs/reference/game-writing.md` F1, itself citing the Microsoft Writing Style Guide): functional text "says the action, the object and the place. Start with the verb". **Cited** (L2): "whatever the text says of the world is true".

**The synthesis — authored.** Practice knows two shapes. Where the objective line is **derived** from the condition (the MMO tracker, the quest grammar), text and completion cannot disagree, and the line has no voice. Where the line is **authored** (advancements, the Creation Kit), it has voice and nothing binds it; the lie in §1 is this shape's ordinary failure. Delvewright keeps the authored line — the objective title is story text in the campaign's own words, and the Chinese is transcreated from it, which a template cannot survive (§3) — and adds the binding the derived shape has by construction: the act is a closed vocabulary (Doran & Parberry's atomic actions, cut to what the pinned game can adjudicate), each completion shape performs a known set of acts, and the title's head word must name one of them. The place/deed distinction is Aarseth's.

## 3. The decision: the act is derived from the completion, and the title is read against it

Two designs were put to this spec.

**A declared act** — the author names `act` on the objective from the vocabulary, the title is judged against it, and the completion is derived from it. Rejected, **authored**: a third declaration beside the title and the kind is a third thing that can disagree, and it removes nothing — the title must still be read to judge it, and the kind must still be read to see what the act is; the author it is meant to bind is the same author who wrote *lay* over a `reach-anchor`, who would write `act: reach` beside it as readily. The DSL already states the player's act, once, in the kind and its fields: a `reach-anchor` *is* going somewhere, a `collect` *is* taking. Stating it again is the second bespoke field.

**A derived act** — the kind and the fields that shape its completion fix the set of acts the objective performs, and the title must state one of them. Chosen, **authored**, for the reason above and because it fits the constitution's rule that a capability belongs to the object class it acts on: the objective is the object, the act is a property of what it completes on, and every kind, present and future, derives one without a new field.

**Also rejected, authored.** *A composed title*: the engine renders `<act> <object phrase>` per language from compiler chrome, as a tracker does. English composes; 把名册放进阁楼 puts the verb in the middle and is not a composition of an act word and an object phrase, so the Chinese would be a translation, which the conventions forbid. *A campaign-extensible lexicon*: `{"acts": {"reach": ["lay"]}}` is an opt-out secured by a property the defect can supply, in the words of the vacuity rule; the lexicon is the engine's, extended by an engine change with the corpus re-read (§10).

**How the title is read — authored.** F1 is the convention the check stands on: an objective title opens on its verb. The **head** of a title is the longest lexicon phrase, up to four words, that the title begins with, after case-folding and treating every character outside letters, apostrophes and spaces as a space; `take your place` is one head and `take` is another, and the longer wins. A title whose head is in the lexicon states the acts the lexicon gives that phrase. A title with no lexicon head states **no act the engine knows**: it is counted and listed in the binding line, never refused — refusing a word the engine does not know would be the check inferring an undeclared intent, which the constitution forbids. An objective with no `title` states nothing and is counted apart.

**What agrees — authored.** The objective's completion performs a set of acts (§4). The title agrees when the head's act set and the completion's act set intersect; it disagrees — `DW1045` — when they are disjoint. The object of the verb is never read: *find* may be a place, a person or a thing, so its set is `{reach, talk, take}` and the completion decides which it was. Intersection, not equality, is the rule because a richer completion performs more acts: an `interact` that takes the presented item is a use, a presentation and a gift at once, and any of the three words is true of it.

## 4. The vocabulary, derived

**Cited** (`delvec schema --stage all`, `quests.$defs.Objective.oneOf`, five variants by their `type` constant; the own fields are those outside the set every variant carries): `talk-to {npc}`, `reach-anchor {anchor, radius, marker}`, `kill {wave}`, `collect {item, count, anchor, container, dropped_by, fill_count, item_name}`, `interact {anchor, marker, prop, requires_item, missing_item_hint}`. **Cited** (the same export): `complete-objective` occurs at exactly one site, `dialogue.$defs.DialogueEffect`; `dialogue.rs` admits it only for a `talk-to` of the tree's own NPC. `QuestEffect` has 45 variants; the one that changes what the player holds is `give-item`. `HappeningVerb` has ten members (`dies`, `survives`, `departs`, `arrives`, `learns`, `believes`, `gains`, `loses`, `opens`, `seals`); it is the story's verb about a subject, not the player's act — *learns* is reached by walking and by talking, *loses* by laying and by giving — and is not the vocabulary this spec reads.

**The acts, authored from the completion shapes.** Seven, named by what the pinned game adjudicates:

| Act | What completes it | Objective shapes that perform it | Place or deed |
|---|---|---|---|
| `reach` | a body inside a volume at a place | `reach-anchor` | **place** |
| `talk` | a dialogue option chosen on a body | `talk-to` | deed |
| `kill` | a wave's counter at zero | `kill` | deed |
| `take` | the item in the player's inventory | `collect`; any kind whose `on_objective_complete` effects include `give-item` | deed |
| `use` | a thing at a place used — the hitbox or the hand-pressed block | `interact` (with or without `requires_item`) | deed |
| `present` | the item held in the main hand at the thing when it is used | `interact` with `requires_item` | deed |
| `give` | one of the presented item leaves the player at the thing | `interact` with `requires_item` and `takes_item: true` (§6.3) | deed |

So: `reach-anchor` → `{reach}`; `talk-to` → `{talk}`; `kill` → `{kill}`; `collect` → `{take}`; `interact` → `{use}`; `interact` + `requires_item` → `{use, present}`; `interact` + `requires_item` + `takes_item` → `{use, present, give}`; and `give-item` among the objective's completion effects adds `take` to any of these. `reach` is the only place act; the rest are deeds. A new kind derives its set from what its completion adjudicates, in the same table, in the PR that adds it.

**The lexicon — authored, English, seeded in the appendix.** 230 head phrases over 223 distinct heads; six heads carry more than one act because their object decides it: `find {reach, take, talk}`, `read {reach, use}`, `try {reach, use}`, `board {reach, use}`, `return {reach, give}`, `break {kill, use}`. The lexicon is a committed data file of the engine (`crates/delvec/data/acts-en.json`), read by the check and printed by `delvec codes`-adjacent tooling as the implementation sees fit; it is the one authority, and no gate or tool holds a copy. A word is added with the corpus reading of §10 repeated.

## 5. Languages: what the check binds, and what binds the rest

**Cited** (`i18n.md`; `DW0187`/`DW0188`): a sidecar row records the canonical English it was made from in its `source` map; a row whose `source` differs from the current English is refused, and rows with no `source` are counted. **Cited** (`key_situations`, `l10n.rs`): the inventory hands the transcreator each title's situation — the quest, the objective, the `happening` — beside the English.

**Authored.** The check reads English. Its binding line states the denominator it judged: `act binding (DW1045): <judged> of <titled> English title(s) read against <total> objective(s); <unknown> head(s) the lexicon does not know; <untitled> untitled; <refused> refused`. A sidecar's `obj.<q>.<o>.title` rows are bound by **provenance**, not by words: each is transcreated from an English title the check has passed, and `DW0187` refuses the row the moment that English moves, so a non-English title can state an act the completion does not perform only by a transcreation that departs from its source's act. To make that departure visible where it is made, the objective's derived act set joins the title row's `situation` as `act: <acts>` (the same place the `happening` already rides), so the transcreator and the fact check of `i18n-translate.py` read the act beside the words. The check does not read Chinese; a lexicon for a second language is a second data file under the same rule, and this spec seeds none. The binding line therefore counts English titles, and the sidecar rows are counted on their own line by `DW0187`'s coverage, as today.

## 6. The remedy

`DW1045` names both ways out, and both must be reachable.

### 6.1 Change the text to the act that completes it

Re-head the title with a word of the completion's act set, in the campaign's voice. *Lay the roll in the loft* on a `reach-anchor` becomes *Climb to the loft with the roll*; *Climb up to Oru* on a `talk-to` becomes *Speak to Oru up the tree*; *Reach the counter* on a `talk-to` becomes *Hear the counter*. The message prints the completion's act set and three of its words. Always reachable: the lexicon has at least six heads per act.

### 6.2 Change the completion to the act the text names

Re-kind the objective so the completion performs the stated act. *Speak the Ending* becomes a `talk-to`; *Get Under a Ram* becomes a `reach-anchor` under the ram; *Lay the roll in the loft* becomes an `interact` at a `prop` in the loft (a lectern, a chest) with `requires_item: minecraft:paper` and `takes_item: true` (§6.3); *Hang the lantern on the hook* keeps its kind and adds `takes_item: true`. Reachable for `reach`, `talk`, `kill`, `take`, `use`, `present` on the tree today; reachable for `give` once §6.3 lands; not reachable for the two shapes §6.4 records.

### 6.3 The capability this spec closes: an item laid at a place leaves the hand

**Cited** (`emit/functions.rs`): an `interact` with `requires_item` completes when the clicking player holds the item in `weapon.mainhand`; nothing is removed, and no verb of the DSL removes an item from a player (`clear` is emitted only by the class kit's flask handling). **Cited** (the pinned command tree, `crates/delvec/data/commands-1.21.11.json`): `clear <targets> <item> <maxCount>` removes up to `maxCount` matching items from a player. Vanilla provides the primitive, so the DSL exposes it first-class.

**Authored — the surface.** `interact.takes_item: bool`, default `false`, absent when false (a document without it canonicalises byte-identically). It requires `requires_item`; `takes_item: true` with no `requires_item` is `DW-next` (there is nothing to take). It is a property of the presented item's fate on the object that presents it, carried by `interact` because `interact` is the one kind that reads a hand; a dialogue option reads no hand (§6.4).

**Authored — the emission.** The `complete_<obj>` function of a `takes_item` interact opens with `clear @s <item> 1`: the function is already run `as` the clicking player under the hand guard, so `@s` is the player who presented the item, and one of it leaves their inventory at the moment of completion. The item's appearance in the world is the author's, composed from what exists — a `set-block` effect in `on_objective_complete` that puts a written book on the lectern or fills the chest — never a second mechanism here. The exported critical-path step keeps its shape (the bot already holds the item to click); the generated `verb_interact` PackTest asserts the player's count of the item is one lower after the click, the perturbation that only this field could move. The l10n inventory is unchanged. Gallery: `probes/` gains the `DW-next` probe, and the primary gains the element of §9.3.

### 6.4 Recorded gaps

Two acts the text can name and no completion performs, recorded here and in the findings ledger, not closed by this spec:

- **Show a thing to a body** (`present` on `talk-to`): *Show Tamsin the tongue*, *Show Neve the lit lantern*. The dialogue option would need a held-item gate (`requires_item` on `DialogueOption`, read where `option_display_conditions` reads `requires_state`). Until then the honest title is a `talk` word (*Tell Tamsin of the tongue*).
- **Give a thing to a body** (`give` on `talk-to`): *Bring both to Oru*. The option would need the gate above and a take. Until then: *Tell Oru you have both*.

Receiving a thing from a body is not a gap: a `give-item` in the objective's completion effects adds `take` (*Fetch the lantern from Neve* passes today for exactly this reason).

## 7. Diagnostics

**`DW1045` — an objective's title names an act its completion does not perform** (`compiler::telling`; error; validation tier, exit 1). Quantifier: every objective of every quest, in the primary document, that carries a `title` whose head is in the lexicon. Path: the objective's `/content/quests/<q>/objectives/<o>/title`. Message: the title, its head word and the act(s) the lexicon gives it, the kind and fields the completion was derived from and the act(s) it performs, then the two remedies of §6 with the completion's act words named and, where the text's act is `give` or `present`, the field or the gap (§6.3, §6.4) that would make it true. Judged at `delvec validate`, where the line is written; nothing reaches the datapack. The binding line of §5 prints on every run.

**`DW-next` — `takes_item` with nothing to take** (`compiler::promise` or wherever `DW0437`, its sibling rule for `missing_item_hint`, is declared; error; validation tier, exit 1). Path `…/objectives/<o>/takes_item`. Prescription: state `requires_item`, or drop `takes_item`.

Both get their catalog row on the declaring module's page, each with a test asserting its code, and `tools/ci/check-dw-codes.py` holds the pair bidirectionally.

## 8. What the check does not judge, by name

The **place**. *Climb the cellar ladder* completing at the ladder's foot is a movement that ends where the title does not say; the act agrees and the destination lies. The `hint` names the place; whether the volume the completion draws is where the title's noun says it is would need the title's object read against the anchor's place name (spec-0052), which this spec does not do. Recorded as the next question, not answered here.

The **count**. *Take the emeralds* over a `collect` of `count: 1` is not read for number.

The **hint**, the **goal**, dialogue, barks: F1 governs them; nothing here reads them.

## 9. Acceptance criteria

Each is a test or a gate run, named.

1. **Surface.** `delvec schema --stage quests` exports `takes_item` on `interact` and nowhere else; `tools/ci/check-gallery-coverage.py` reports it bound. A document without it canonicalises byte-identically (`crates/dsl` round-trip test).
2. **`DW1045` raised and silent.** A fixture of five objectives, one per kind, each titled with a head of its own act set, validates green; changing any one title's head to a word of a disjoint act set — including the four `interact` shapes of §4 — raises `DW1045` naming that path and no other. A title whose head is not in the lexicon is not refused and is counted in the binding line's `unknown`. A `talk-to` with `give-item` in its completion effects accepts a `take` head; the same objective without the effect refuses it. An `interact` with `requires_item` and `takes_item: true` accepts a `give` head; without `takes_item` it refuses the same head. The six object-decided heads each pass on every kind in their set and refuse on one outside it.
3. **Gallery element and probe.** The primary gains an `interact` with `requires_item` and `takes_item: true` titled with a `give` head (an item laid on the counter or hung at the hearth, in the gallery's own words), so `takes_item`, the `give` act and the `clear` emission are bound; `gallery/baseline/` moves by exactly that element. A probe `an-objective-that-says-one-act-and-rewards-another` patches an existing gallery title's head to `Lay` and is refused with `DW1045`; a probe for `DW-next` sets `takes_item: true` on an `interact` with no `requires_item` and is refused with that code. Both overlays' `talk-to` titles *Reach the counter* and *Climb the loft* are re-headed (§6.1), and `delvec validate` of each overlay is green.
4. **The Muster Loft shape, refused and repaired.** A fixture reproducing `obj/lay-the-roll` exactly (a `reach-anchor` titled *Lay the roll in the loft*, after a `collect` of `minecraft:paper`) is refused with `DW1045`, the message naming `lay → give` and `reach-anchor → reach`. Two repairs of that fixture are green: (a) the title re-headed to a `reach` word; (b) the kind changed to `interact` at the same anchor with a `prop`, `requires_item: minecraft:paper`, `takes_item: true`. Repair (b) builds, its `verb_interact` PackTest passes and asserts one paper fewer, and the exported critical path carries the interact step holding the paper.
5. **A perturbation that reds.** On the gallery primary, changing the head word of any one agreeing title to a disjoint act's word turns the coverage run red with `DW1045` and names that title; the gate's own test performs this perturbation on one title and asserts the red.
6. **The binding line.** `delvec validate` prints `act binding (DW1045): …` with the counts of §5 on every campaign, and the gallery coverage report carries it; a run that judged zero English titles on a campaign that has titles is red.
7. **Translator context.** `delvec l10n-inventory` lists `act: <acts>` in the `situation` of every `obj.<q>.<o>.title` row; `tools/creator/i18n-translate.py` passes it through unchanged (its test asserts the field reaches the prompt).
8. **Catalog and tests.** `DW1045` and `DW-next` each have a row on their declaring module's page and at least one test asserting the code; `tools/ci/check-dw-codes.py` is green with neither in `PENDING`.
9. **The lexicon is one file.** `crates/delvec/data/acts-en.json` is the only place a head phrase is written; a test asserts every phrase in it is lowercase, has at most four words, and that the file sorted equals the file as committed.
10. **Pages.** `docs/reference/game-writing.md` carries rule F7 (this spec's rule, cited and authored as written there) and the sources of §2; `docs/reference/game-writing-evidence.md` carries the §1 reading; `references/quest-capabilities.md` of the `/new-delve` skill carries §11 at the re-pin; `tools/ci/check-skill-page.py` is green.
11. **Demo level.** `docs/demo-levels.md` carries the row *An item laid at a place* when the mechanic lands; the level on its content branch builds, passes PackTest, the bot walk and the staging gate, and holds one `takes_item` interact whose title is a `give` word and whose prop then shows the item.
12. **Findings.** `docs/playtest-findings.json` carries a row for the Muster Loft title with a verified non-zero binding (the fixture of criterion 4), closed by the `DW1045` re-run; and one row each for the two gaps of §6.4, open.

## 10. Measurements this spec states

Written into the commit body that lands the implementation, taken on that tree: the corpus reading of §1 repeated with the engine's own check over content `main` at its then revision, the Muster Loft branch and the gallery (objectives, titled, unknown heads, agreeing, refused, and each refused title named), cross-checked against the hand reading of §1 as the second method; the gallery coverage line; the `verb_interact` PackTest's before/after item counts for the §9.3 element.

## 11. What the creator's page says, at the re-pin

`tools/ci/check-skill-page.py` holds the `/new-delve` page to the engine release it pins, and at this tree that release predates this spec; the page therefore changes with the re-pin that carries it. The text it takes, for `references/quest-capabilities.md` §Objectives, after the first bullet:

- **The title's first word is the act that completes the objective, and the engine checks it (`DW1045`).** A `reach-anchor` is reached: open on *Go*, *Climb*, *Enter*, *Cross*, *Find*, *Look into*. A `talk-to` is a conversation: *Speak to*, *Ask*, *Hear*, *Meet*, *Tell*. A `kill` is a fight: *Put down*, *Clear*, *Silence*. A `collect` is a taking: *Take*, *Fetch*, *Gather*. An `interact` is a use — *Pull*, *Ring*, *Open*, *Light* — and with `requires_item` also a presentation — *Show*, *Hold up*. *Lay*, *Hang*, *Place*, *Give*, *Bring* are true only of an `interact` with `requires_item` and `takes_item: true`, which takes one of the presented item from the hand; nothing a `talk-to` completes gives or shows anything, so write *Tell Oru you have both*, not *Bring both to Oru*. A word the engine does not know is listed, not refused; a word of another act is refused with both repairs named. The `hint` says where; the title says what.
- **`takes_item: true` makes a laid, hung or given thing leave the hand.** On an `interact` with `requires_item` only. The world showing the thing afterwards is yours: a `set-block` in `on_objective_complete` puts the book on the lectern.

## Sources

- Doran, J. & Parberry, I. *A Prototype Quest Generator Based on a Structural Analysis of Quests from Four MMORPGs.* PCGames 2011, Bordeaux. ianparberry.com/pubs/pcg2011.pdf — §3 (strategies as verb–noun pairs; quests as sequences of atomic actions), Table 5 (atomic actions with pre- and post-conditions).
- Aarseth, E. *From Hunt the Wumpus to EverQuest: Introduction to Quest Theory.* ICEC 2005, LNCS 3711, pp. 496–506 — the place/time/objective typology, as read in Karlsen, F. *Quests in Context: A Comparative Analysis of Discworld and World of Warcraft.* Game Studies 8(1), 2008.
- Minecraft Wiki. *Advancement definition* (`display.title`, `display.description`, `criteria[].trigger`, the trigger list). minecraft.wiki/w/Advancement_definition.
- UESP Creation Kit wiki. *Bethesda Tutorial Quest Objectives*; *SetObjectiveCompleted – Quest*. ck.uesp.net.
- Warcraft Wiki. *API GetQuestObjectiveInfo*. warcraft.wiki.gg/wiki/API:GetQuestObjectiveInfo.
- Microsoft. *Xbox Accessibility Guideline 109: Objective clarity.* learn.microsoft.com/gaming/accessibility/xbox-accessibility-guidelines/109.
- TES Renewal: Skywind. *Quest Design Principles.* handbook.tesrskywind.com/dev/Principles.
- `docs/reference/game-writing.md` F1 (citing the Microsoft Writing Style Guide, *Top 10 tips*) and L2.

## Appendix — the lexicon seed (`crates/delvec/data/acts-en.json`)

Head phrases, lowercase, matched longest-first at the title's head. 230 entries, 223 distinct heads. Object-decided heads appear under every act their object can take.

| Act | n | Heads |
|---|---|---|
| `reach` | 70 | `approach`, `arrive`, `ascend`, `board`, `climb`, `climb down`, `climb to`, `climb toward`, `climb towards`, `climb up`, `come`, `come to`, `cross`, `descend`, `down to`, `enter`, `escape`, `exit`, `explore`, `find`, `follow`, `get into`, `get onto`, `get out`, `get to`, `get under`, `go`, `go back`, `go down`, `go through`, `go to`, `go up`, `head`, `head for`, `hide`, `leave`, `look`, `look at`, `look down`, `look for`, `look into`, `look over`, `make for`, `out to`, `pass`, `reach`, `read`, `return`, `run`, `run for`, `scout`, `see`, `set off`, `set out`, `slip`, `sneak`, `stand`, `step`, `step into`, `survey`, `take cover`, `take your place`, `travel`, `try`, `up to`, `view`, `visit`, `walk`, `wander`, `watch` |
| `talk` | 22 | `answer`, `ask`, `call on`, `confer with`, `consult`, `decide with`, `find`, `greet`, `hear`, `listen`, `listen to`, `meet`, `question`, `report`, `report to`, `speak`, `speak to`, `speak with`, `talk`, `talk to`, `talk with`, `tell` |
| `kill` | 26 | `beat`, `best`, `break`, `clear`, `cull`, `cut down`, `defeat`, `destroy`, `drive back`, `drive off`, `end`, `face`, `fight`, `fight off`, `finish`, `hold the line`, `kill`, `overcome`, `purge`, `put down`, `quiet`, `repel`, `silence`, `slaughter`, `slay`, `survive` |
| `take` | 17 | `acquire`, `claim`, `collect`, `fetch`, `find`, `gather`, `get`, `grab`, `loot`, `obtain`, `pick up`, `pocket`, `recover`, `retrieve`, `seize`, `steal`, `take` |
| `use` | 62 | `activate`, `bar`, `board`, `break`, `burn`, `char`, `check`, `close`, `crank`, `cut`, `dim`, `douse`, `draw`, `examine`, `extinguish`, `flip`, `grind`, `hit`, `ignite`, `inspect`, `investigate`, `kindle`, `kneel at`, `knock`, `lift`, `light`, `lock`, `lower`, `open`, `operate`, `play`, `pray at`, `press`, `pull`, `push`, `put out`, `raise`, `read`, `rest at`, `ring`, `rouse`, `seal`, `search`, `sharpen`, `shut`, `shutter`, `smash`, `snuff`, `sound`, `strike`, `throw`, `toll`, `touch`, `try`, `turn`, `unbar`, `unlock`, `unseal`, `use`, `wake`, `wind`, `work` |
| `present` | 6 | `bring word of`, `hold out`, `hold up`, `present`, `prove`, `show` |
| `give` | 27 | `affix`, `attach`, `bring`, `bury`, `deliver`, `deposit`, `drop`, `feed`, `fit`, `give`, `hand`, `hand over`, `hang`, `insert`, `install`, `lay`, `leave behind`, `mount`, `offer`, `pay`, `place`, `plant`, `pour`, `put`, `return`, `set`, `tie` |
