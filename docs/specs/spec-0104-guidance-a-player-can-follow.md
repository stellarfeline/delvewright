# spec-0104: Guidance a player can follow — every act is in sight or told, the text names the act its type completes on, the opening teaches each input, and a pressable stands alone among its kind

- **Status**: Proposed
- **Ground**: written against engine `5961145d8` (`origin/main`), read only — `Objective`, `Guidance`, `Visibility` in `crates/dsl/src/quest/objective.rs` and `quest/mod.rs`; `EnvTrigger`, `TriggerOn` in `crates/dsl/src/trigger.rs`; `Wave` in `crates/dsl/src/wave.rs`; `TextKind`, `each_string`, `plain` in `crates/dsl/src/l10n.rs`; the span grammar of `crates/dsl/src/textstyle.rs` (spec-0096); `ChromeString` in `crates/dsl/src/chrome.rs`; `check_objective_prompts` (`DW0862`, `DW0961`, `DW0962`), `check_pressables_visible` (`DW0963`, `PressableBinding`), `check_fight_signposts` (`DW0863`, `FightBinding`, `firing_sites`, `act_places`, `objective_place`) in `crates/delvec/src/compiler/promise.rs`; `check_names_told_bound` (`DW0982`, `NameBinding`, `is_use`, `introduces`) in `crates/delvec/src/compiler/telling.rs`; `Step`, `Step::pos` in `crates/delvec/src/compiler/plan/path/mod.rs`; `Flow::journal`, `PathStep`, `Playthrough` in `crates/delvec/src/compiler/flow.rs`; `walk_cells`, `World::blocks_camera`, `cell_center` in `crates/delvec/src/compiler/nav/world/body.rs`; `INTERACTION_REACH` in `crates/delvec/src/compiler/crosshair.rs`; `DEFAULT_FOLLOW_RANGE` and `wave_aggro_radius` in `crates/delvec/src/compiler/nav/wave.rs`; `is_hand_pressed` in `crates/dsl/src/blockshape.rs`; `PLACEHOLDER_RE` and `check_row` in `tools/creator/i18n-translate.py`; `docs/reference/game-writing.md` §1, §3, §5 and §8; `docs/reference/web-narrative.md` §5; spec-0093. The campaign that motivated this spec was read at the content repository's `campaign/stranding` head (`campaigns/the-stranding/quests.json`, `dsl_version 0.39.0`).
- **What it is for**: a player who finishes one thing and can tell what to do next. In the first full playtest of a quiet delve the party could not: 18 of 30 objectives carried no `hint` and steps went unseen; a title said *go* where talking completes the step and a hint said *walk past* a thing that must be right-clicked; nothing in the opening said that the world's blocks are right-clicked, and a sign started a cutscene unannounced; interact props did not read as the thing their text names and one stood among identical blocks; a fight's bodies were seated out of sight. The campaign's own choice stands and this spec serves it: **no highlight guidance** — routes, text, props and scenes carry it, never a glowing marker. Today `hint` is optional on every objective kind and no rule asks for one; `DW0863` asks only of a `kill`, accepts aggro reach in place of a line and never asks about sight; no check reads what act a title names (`compiler::promise`'s module doc says so in those words, and `game-writing.md` §5 lists hint-for-statement as human review); no field and no check concerns teaching an input, and a `use` trigger has no prompt; `DW0963` refuses only a prop-less click on open air and nothing compares a prop with its neighbours.
- **Research**: §2 is this spec's record. Each statement is marked **cited** (a line of the tree at `5961145d8`, a reading of a document, a published source listed under Sources, or a constitution rule) or **authored** (this spec chooses). An unsupported claim is named as one. Every outside source is used for its ideas only (ADR-0013).
- **Numbers**: spec `0104`. Three new diagnostics: `DW1015` (a title or hint that names another act, §4), `DW1016` (an input the opening does not teach, §5), `DW1017` (a pressable among its own kind, §6). `DW0863` and `DW0982` change what they judge under their own codes (§3, §7). The surface change is stage 5 and the implementation takes the next minor `dsl_version`, stated once, by `crates/dsl/Cargo.toml`.
- **Non-goals**: a marker, glow, beam or floating label of any kind (spec-0093 keeps the lantern as an opt-in; nothing here adds to it); a journal or quest log surface (the hint is shown once, at activation, as today; a re-readable journal is a capability of its own and is recorded as a gap in §12); reading prose for meaning (§2.4 is why); an `approach` or `step` trigger as an act the party must find (it is a place on the route, not a thing to press); a quest's `goal` (a heading, not an act); whether a block **reads as** the object its text names (human QC, §6); a compiler-emitted teaching line (§2.3 is why the line is authored).

## 1. The defect, and the object class

**Cited** (`promise.rs`, module doc): *what the game tells the party and what the machine actually requires are not the same thing* is one defect class, judged in one module over one object class — the player-facing promise attached to an objective. **Cited** (`promise.rs`, `check_fight_signposts`): the one rule that asks whether the party can *find* a thing is `DW0863`, and it quantifies over `kill` objectives alone, because a fight is the one kind the compiler puts nothing in the world for. **Cited** (`objective.rs`): `hint` is `Option<String>` on all five kinds; `EnvTrigger` carries no player-facing string.

**Authored — the object class is the act.** An **act** is a thing the party must do at a place to advance a walked path: every objective of every kind, and every `use` or `strike` trigger the path performs (`Step::Trigger`). What the party is owed for each act is the same five things whatever the kind: it can be found (§3); its text names what the hands do (§4); the input it needs has been taught before it is first needed (§5); the thing to press is the only one of its kind in reach (§6); the names its text uses have been told (§7). Each is a rule over the act class, keyed to the act's **kind** only where vanilla gives the kinds different inputs. None is a field on a verb.

## 2. Research record

### 2.1 Where guidance comes from

- **Cited** (`game-writing.md` §8 L1): the level carries the information and the text restates it; a destination is recognised by its building; a glowing entity, floating label or beam is the defect. **Cited** (`web-narrative.md` §5): Lynch's landmarks, paths, edges, nodes and districts (via Totten); a landmark that draws from afar, hidden and revealed again (Rogers); light and props lead instead of markers; players look where they travel.
- **Cited** (Nielsen Norman Group, *10 Usability Heuristics*, #6 *Recognition rather than recall*): "minimize the user's memory load by making elements, actions, and options visible"; "information required to use the design … should be visible or easily retrievable when needed." **Authored**, from it: an act the party can see from where it stands needs no line; an act it cannot see owes one, shown when the act becomes the next thing to do.

### 2.2 Text that names the act

- **Cited** (`game-writing.md` §1 F1, itself citing the Microsoft Writing Style Guide and the Game Accessibility Guidelines' *Use simple clear language*): functional text starts with the verb and names the thing and the place. **Cited** (Nielsen Norman Group, heuristic #4 *Consistency and standards*): "users should not have to wonder whether different words, situations, or actions mean the same thing." **Authored**, from both: the verb a title opens with is a claim about what the hands will do, and the objective's `type` is the engine's record of what the hands must do; two authored fields that disagree are the `DW0862` shape — a contradiction between declarations, judged without reading either for meaning.

### 2.3 Teaching an input

- **Cited** (Game Accessibility Guidelines): *Include tutorials* (Cognitive, Basic) — "guided interactive tuition is still far more useful for all players than a simple instructions screen"; *Include contextual in-game help / guidance / tips* (Cognitive, Intermediate) — introducing concepts gradually during play suits players with short-term memory limits better than upfront screens; *Give a clear indication that interactive elements are interactive* (Cognitive and Vision, Intermediate) — players "can have difficulty distinguishing which UI elements or in-game items are intended to be interactive", and some do not know the conventions others take for granted.
- **Cited** (Minecraft Wiki, *Tutorial hints*): the pinned game's own teaching is a toast in the top right, one per basic control, each shown when the control is first relevant, naming the bound key (*Movement*, *Destroy the tree*, *Open your inventory*, *Social interactions* name theirs), dismissed once the act is done, and never shown again once the sequence completes.
- **Cited** (Minecraft Wiki, *Text component format*): a component's `keybind` field holds "a keybind identifier, to be displayed as the name of the button that is currently bound to that action"; it is displayed by the client from the viewer's own settings, so "each will see their own keybind"; `{keybind: "key.inventory"}` shows `e` under the default scheme. Vanilla's identifiers for the two inputs an act needs are `key.use` and `key.attack`.
- **Cited** (`chrome.rs`): a compiler-authored line is baked English with a per-language table the engine maintains, declared unreviewed; the diegetic ones each carry an authored override. **Authored**, from it: the teaching line is in the campaign's voice and the campaign's language, so it is authored prose carrying the one thing only the engine knows — the key — as a span, never a compiler sentence; the moment it is shown is derived, never typed.

### 2.4 What a machine can read in prose, measured

- **Cited** (`game-writing.md` §5): *Functional text without action or place* (a functional key lacking an imperative, a direction or a named object) measured **8 of 26 (31 %)** precision on the audited campaign — "not usable as a refusal"; *Hint-for-statement* **6 of 16 (38 %)** — "human review only". **Authored**, from it: no rule in this spec reads a hint for whether it names a place or for what it means. The machine's half of "a findable place" is two closed-set facts: the names the hint uses are told (§7), and the act is either in sight or carries the hint at all (§3). The rest is QC, and the spec says so.
- **Cited** (`telling.rs`, module doc): `DW0982` dates every string at the first state of the play order at which the player can read it — an objective's title and hint "once its `after` is done" — and judges a **use** only in an NPC's line or an option's label or tooltip. So titles and hints are already placed on the walk as tellers and are not yet judged as users; §7 is that widening.

### 2.5 Sight, and how far it reaches

- **Cited** (`body.rs`, `blocks_camera`): a line of sight is stopped by a full-cube solid, a 1.5-tall fence or wall, or a fence gate; water does not stop it; `walk_cells` is the exact voxel traversal (Amanatides–Woo) a declared sightline (`DW0821`) and the cutscene clip are proven with — one traversal, so a second predicate for what a fence does to a view would be a second opinion.
- **Cited** (spec-0091 §2.5): the server stops tracking a body for a client between 10 and 11 chunks whatever the view distance — a body past 176 blocks is never sent. **Cited, secondary** (Mojang bug tracker MC-157896, quoting the client's mapped `shouldRenderAtSqrDistance`): the client draws a body only within `64 × size × viewScale` blocks, `size` its bounding box's average edge clamped to at least 1 and `viewScale` 1.0 at the default *Entity Distance*; so a body of a block or less is drawn to 64 blocks. **Authored**: `SIGHT_BLOCKS = 64` is the one reach this spec gives sight — the farthest a one-block body is drawn at default settings, used for blocks as well, because a one-block prop farther than that is not a thing a player picks out of a scene (unsupported claim: no reading of a player at a screen was taken; the demo level is where it is looked at).

### 2.6 The thing the player presses

- **Cited** (spec-0093 §6.5): the hitbox alone is never the object; a quiet act sits on a visible authored object at its cell; where vanilla reports the object's use (`is_hand_pressed`: the lever, every button, the bell) the block is the whole body. **Cited** (`promise.rs`, `DW0861`): four identical barrels, one of them the objective's, with nothing to say which — repaired at the time by a `title` and an `item_name`, which name the thing and still do not say which box. **Cited** (the Portal post-mortem as reported live at GDC 2008): playtesters did not understand a "shimmery force field" in the first room, and the team changed it to glass — the object was changed until it read, no marker was added. **Authored**, from both: the machine's half of "the prop reads as the thing" is that nothing of the same kind stands within the click's reach of it; whether the block reads as the named object is QC.

### 2.7 The motivating campaign, measured

**Cited** (`campaigns/the-stranding/quests.json` at `campaign/stranding`): `guidance` is `{announcements: shown, markers: hidden}`; 30 objectives — 16 `interact`, 8 `talk-to`, 6 `kill`; 18 carry a `title`, 12 a `hint`; the first objective is an untitled, unhinted `interact` on a `spruce_sign`; 5 of the 6 kills are untitled; 44 triggers, of which 13 are `use` (9 with a prop: two `dark_oak_fence` tillers, two bells, a banner, three identical `red_mushroom_block` valves, a chiseled-stone rubbing) and 8 `strike-assembly`; every wave is anchored at a place node's centre. Against the §4 lexicon as seeded, **9 of the 18 titles** name no act of their type or another type's act (*Look round*, *Look through*, *Follow*, *Go on*, *Go in*, *Find*, *Cut*, *Get back*, *Take* on `interact` and `talk-to` objectives) — a hand count the implementation re-measures with the check itself (§11).

## 3. An act the party can find — `DW0863`, widened to every act

**Authored — the rule.** On every walked path — the exported critical path and each enumerated world's own path, the walks `Flow::journal` already takes for `DW0982` — every act is **found** when either holds:

1. **it is announced with a `hint`** (`Objective::announced` and a non-empty `hint`; a trigger has no lines and cannot take this arm); or
2. **it is in sight from where the party stands**: a clear line, by `walk_cells` over `World::blocks_camera`, from the eye of a body standing on the **previous act's cell** — the `pos()` of the preceding positioned `Step` on that path, or where that step's completion carries the party when it carries it (`transport_by_step`: an inter-area crossing, a link's landing), or the entry cell the path departs from for the first act — to the act's **target**, in the world that walk holds when the act is next (every earlier step of the walk has happened, so a door an earlier step opened is open), within `SIGHT_BLOCKS`; and, for a `kill`, the wave's anchor is also within the wave's reach of that cell (`wave_aggro_radius`: the lane's `aggro_radius`, else the mobs' largest declared `follow_range`, else `DEFAULT_FOLLOW_RANGE`), so the bodies the party sees also come to it.

The **target** per kind: an `interact`'s anchor cell; a `reach-anchor`'s anchor cell; a `talk-to`'s NPC station on that path (`plan.body_point`, the cast ladder's answer); a `collect`'s container cell, else the compiler's chest at its anchor, else the dropping wave's anchor; a `kill`'s wave anchor; a `use`/`strike` trigger's `at` cell, or the struck NPC's or assembly's cell. The eye is `cell_center(prev) + (0, 1.62, 0)`; the target point is `cell_center(target) + (0, 1.0, 0)` for a body or a wave, the block's centre for a block; the eye cell and the target cell are not blocking cells. A kill fired by several acts (the `firing_sites` of today's rule) is judged from each firing act's cell, all of which must see it, exactly as every site must be in reach today.

An act that is neither is refused with `DW0863`, naming the path, the previous act and its cell, the target and its distance, the first blocking cell, and both remedies — give it a `title` and a `hint` (an objective), or move the act or its target into sight (any act; a trigger's only remedy, or make it an `interact` with a hint). **This is a tightening, in those words**: a `kill` found by reach alone (the bodies come, unseen, round a wall) is no longer found; an objective of another kind out of sight with no hint, which nothing refused, is refused. What stands behind it: the campaign that found the defect had every announcement shown, so its remedy is the hint it already meant to write.

**Pairs.** `DW0862` (a hint on an unannounced objective) is unchanged and is this rule's other half: an author who takes arm 1 under hidden announcements meets `DW0862`, whose remedy — announce it — is this rule's. A hint written to satisfy arm 1 is itself judged by §4 and §7, so the three rules on one hint are read together: it exists, it names this act, and it names only told things.

**Binding line** (`FightBinding`, renamed `ActBinding`): `DW0863 binding: N act(s) over W walk(s) — A announced with a hint, S in sight (K kill(s) also in reach), T trigger(s) judged by sight alone; R refused`. Zero acts is stated.

## 4. A title names the act its type completes on — `DW1015`

**Authored — the lexicon.** `crates/dsl/data/act-verbs.json` holds, per objective kind and per language, the verbs that name the act vanilla completes the kind on. One verb belongs to one kind in a language (a unit test holds the five sets disjoint). The seed, English, each entry a whole word or phrase matched case-insensitively on the string's visible text (`l10n::plain`):

| kind | the hands do | verbs |
|---|---|---|
| `talk-to` | right-click a person | talk, speak, ask, tell, question, greet, meet, wake, warn, persuade, confess, report |
| `interact` | right-click a block, hand or held item | use, press, pull, push, ring, light, open, unlock, turn, read, touch, place, hold, present, show, offer, insert, set, lower, raise, wind, crank, hang, knock |
| `reach-anchor` | walk to a place | go, reach, climb, cross, enter, follow, descend, walk, run, head, return, leave, escape, find, get to, come, step, pass, arrive |
| `kill` | fight bodies to the last | kill, slay, defeat, destroy, fight, clear, drive off, fend off, repel, hunt, beat, put down, cut down, break |
| `collect` | take an item | take, collect, gather, pick up, bring, fetch, retrieve, recover, loot, grab, carry, search, steal |

Chinese (`zh-cn`), matched as a substring of the visible text: `talk-to` 谈, 说, 问, 告诉, 聊, 商量, 请教, 禀报, 找; `interact` 使用, 按, 拉, 推, 敲, 摇, 点亮, 打开, 拧, 转动, 放上, 举, 出示, 插, 挂, 放下, 升起, 读; `reach-anchor` 去, 到, 前往, 走到, 爬, 登, 穿过, 进, 进入, 跟随, 下到, 回到, 离开, 找到, 抵达; `kill` 杀, 击杀, 消灭, 击败, 打败, 清除, 赶走, 击退, 斩, 除掉; `collect` 拿, 取, 收集, 捡, 带回, 取回, 找回, 搜, 拾, 夺. The seed is **authored**; a verb a campaign needs that is not here is an engine edit to the data file, and the message prints the kind's set so the author sees the choice. A language with no table is reported in the binding line as unjudged, with its string count.

**Authored — the rule.** For every objective, in the English document and in every sidecar whose language has a table:

- the **title** contains a verb of its own kind's set, else `DW1015` — it names another act (the verb found is in another kind's set) or no act;
- the **hint** that contains a verb of another kind's set and none of its own is `DW1015`; a hint with no lexicon verb at all passes (a hint may be a place: *Past the bell, left of the arch.*).

A title or hint with an own-kind verb passes whatever else it says (*Go up the stair and ring the bell* is an `interact`). The check is membership in a closed set, never a reading of meaning (§2.4); its recall is bounded by the seed and stated as such. Tier: validation (`promise`), one walk of the objective surface beside `DW0862`. Binding line: `DW1015 binding: N title(s) and H hint(s) over L language(s) judged (U string(s) in languages with no table); R refused`.

## 5. The opening teaches each input — the key span and `DW1016`

### 5.1 The key span

**Authored.** The span grammar of spec-0096 §3.1 admits one more style: `[[key|<input>]]`, `<input>` one of `use`, `attack`, `jump`, `sneak`, `sprint`, `inventory`, `drop`, `swap` — each naming a key mapping the pinned client knows: `key.<input>` for the first seven, and `key.swapOffhand` for `swap` (the pinned client has no `key.swap`). A key span has no prose: its text **is** the identifier, and it lowers to `{"keybind": "key.<input>"}` (`key.swapOffhand` for `swap`) as a `with` argument of the line exactly as a styled span lowers to its component (spec-0096 §3.3). Its visible text, for every reader that measures or watches a line, is the identifier in brackets (`[use]`); its width is measured as the longest default key name of the pinned client for that input (`Right Button` for `use`, `Left Button` for `attack`), rounded in the direction that can only turn a width check red. A translation keeps the span byte for byte: `DW0976` already refuses a translation whose spans differ from the English, and the translator's fact check (`PLACEHOLDER_RE`, which keeps `[[…|` openers and `]]` closers) is extended to keep a key span whole, identifier included (§10). An unknown `<input>` is `DW0975`.

### 5.2 Input classes

**Authored — a derivation, never typed.** Every act has an **input class**, read off its kind and body:

| class | the hands do | acts |
|---|---|---|
| `use-block` | right-click a block | `interact` with a prop or standing on an authored block; `collect` from a container or the compiler's chest; `use` trigger |
| `use-body` | right-click a body | `talk-to`; `interact` whose body is a hitbox beside a marker |
| `attack-body` | left-click a body | `kill`; `strike-npc`; `strike-assembly` |
| `attack-block` | left-click a block | `strike` trigger |

A `reach-anchor` and an `approach`/`step` trigger need no input beyond walking and belong to no class. The class of an `interact` is the body `compiler::pressable` already resolves for it (`Body::Block` and a prop or authored block are `use-block`; a hitbox is `use-body`).

### 5.3 The surface

Stage 5, in the `guidance` block spec-0093 owns:

```json
{
  "guidance": {
    "teaching": {
      "use-block": "Things in this town answer to a touch: [[key|use]] on them.",
      "use-body": "Stand close and [[key|use]] to speak to someone.",
      "attack-body": null
    }
  }
}
```

`teaching` maps each input class to a line, or to `null`. A line is authored prose carrying its class's key span (`use` for the two `use-` classes, `attack` for the two `attack-` classes), inventoried as `guidance.teaching.<class>` (`TextKind::Narration`), transcreated like any line. `null` declares that the level teaches this class without a line — a declaration of zero, legal, printed. A class absent from the map is undeclared.

### 5.4 Emission

**Authored.** For each class with a line, the compiler shows the line **once per player**, in chat, at the first moment an act of that class is the next thing that player can do: for an objective, in `activate_<obj>` of the first act of that class on any path, under a per-player `dw.taught_<class>` score; for a `use`/`strike` trigger, when a player first stands in one of the trigger's **firing cells** (spec-0093 §6.2: the standable cells within `INTERACTION_REACH` of its anchor) while it is armed, by the same per-tick position test an `approach` trigger uses. The line precedes the act: a teaching shown at the completion of the act it teaches is the defect `DW0860` names for a clock, and this spec does not emit that shape. Nothing about the bot's channel moves.

### 5.5 `DW1016` — teaching and need disagree

Build tier (`promise`, from `emit::build`: which `use`/`strike` triggers a walked path performs is decided when the plan is built, `plan::path::path_triggers`, so a document-level check cannot know them), three shapes under one code: **A.** a class some walked path requires and `teaching` neither names nor nulls — an input the opening never teaches; **B.** a class `teaching` gives a line and no walked path requires — a line nobody is owed, the inert declaration; **C.** a line without its class's key span — a line that does not say the key. The message names the class, the first act of that class on the path that needs it, and the remedy per shape (write the line with `[[key|…]]`, write `null`, remove the line). Binding line: `DW1016 binding: C class(es) required over W walk(s) — L taught by a line, Z declared taught by the level, R refused`. A campaign whose paths require no class and declares no `teaching` is `0 required, 0 declared`, stated.

## 6. A pressable stands alone among its kind — `DW1017`

**Authored — the rule.** Build tier (`promise`, over the settled block map, beside `DW0963`): for every **pressable** — an `interact`'s prop or authored block, a `use`/`strike` trigger's prop or authored block, a `collect`'s adopted container — no cell within `INTERACTION_REACH` (3.0 blocks, cell centre to cell centre) of the pressed cell holds a block of the **same id** (blockstate ignored: a lever facing the other way is a lever), unless the act is an `interact` with no prop whose marker is shown — spec-0093's lantern picks it out of its twins, and it is the one opt-in a quiet campaign does not take (a prop never carries a marker, `DW0962`). Refused with `DW1017`, naming the act, the block, every same-kind cell in reach with its distance, and the remedies: move the twins, change the prop to a block the scene has one of, or mark the act. **Do not** satisfy it by moving the anchor into a wall (`DW0963`'s warning, repeated).

`DW0861` is unchanged (an adopted container still owes its `title` and `item_name`: the stack names itself); the half of its finding that was *which box* now lives here, over every pressable rather than the one verb that first met it. A reach-anchor, a kill and an NPC are not pressables. Binding line (`PressableBinding`, extended): `DW1017 binding: P pressable(s) — Q alone within 3.0 blocks, M marked; R refused among T twin cell(s)`.

## 7. A hint's names are told — `DW0982`, widened

**Authored.** `TextKind::Objective` strings — an objective's `title` and `hint` — are **uses** as well as tellers: a declared name (the inventory's `name` and item-name rows) occurring in one, as the name or as a definite reference, is judged against when the play order told it, at the state the objective is announced (`after` done, quest active — the state `telling.rs` already dates them at). A title that introduces a name by `telling.rs`'s own `introduces` rule (`the Undertide, the grey water under the castle`) tells and is not refused. The message is `DW0982`'s, naming the objective. This is `game-writing.md` §8 L1's sentence *it never names a place, person or thing the player has not been told*, bound to the rows a machine can hold; a hint's landmark, when it is not a declared name, stays QC (§2.4).

## 8. Refusals

| Code | Shape | Tier |
|---|---|---|
| `DW0863` | an act on a walked path neither announced with a hint nor in sight (and, for a kill, in reach) from the previous act's cell (§3) | build (`promise`) |
| `DW1015` | a title with no verb of its kind, or a title or hint whose only act verb is another kind's (§4) | validation (`promise`) |
| `DW1016` | an input class required and not taught, taught and not required, or taught by a line with no key span (§5.5) | build (`promise`) |
| `DW1017` | a pressable with a block of its own kind within the click's reach, unmarked (§6) | build (`promise`) |
| `DW0982` | a declared name in an objective's title or hint before the play order told it (§7) | build (`telling`) |
| `DW0975` | a key span naming an input the client has no identifier for (§5.1) | validation (`textstyle`) |

Every rule judges what the author declared: a kind, a title, a map of lines, a prop, a name. None infers an intent the document does not state; a campaign with no acts of a class, no pressables or no names states its zeroes.

## 9. Gallery and demo level

**Gallery.** `check-gallery-coverage.py` enumerates the new units `Guidance.teaching` and `Teaching.use-block`, `Teaching.use-body`, `Teaching.attack-body`, `Teaching.attack-block` (the last two as the gallery's paths require them, else refusal-proven); the key span is markup inside strings the schema already declares and binds no unit, as spec-0096's spans do. The gallery's objectives each take arm 1 or arm 2 of §3, its titles carry their kind's verb in both languages, its `teaching` block names every class its paths require. Probes, each the primary plus one declared edit: `a-title-that-names-another-act` (`DW1015`: an interact's title verb replaced by `Go to`, the zh-cn row following); `an-input-the-opening-never-teaches` (`DW1016` A: the `use-block` line removed); `a-line-that-does-not-say-the-key` (`DW1016` C: the span removed from the line); `a-lever-in-a-row-of-levers` (`DW1017`: a world edit placing a second lever two blocks from the gallery's lever); `a-step-out-of-sight` (`DW0863`: the hint of a talk-to whose NPC stands round a corner from the previous act removed); `a-hint-that-names-the-untold` (`DW0982`: a hint rewritten to use the name a later NPC introduces). `a-fight-nobody-points-at` stays a refusal and its message now also states whether the muster line is in sight.

**Demo level.** A row in `docs/demo-levels.md`: **The Ferry Bell** — one shore, one jetty, one ferry house: a talk, a sign to read, a lever that opens the ferry house, a bell that calls a fight that forms up in sight across the water, every title in both languages, the `teaching` block written in the ferryman's voice, no marker anywhere. Built once with the lever among its rail's fence posts of the same block, to be refused, then moved. The look is the thing no machine decides: whether the key span reads in a line, whether the prop reads as the thing, and whether a stranger who has never right-clicked a block does so at the sign.

## 10. Pages

**Reference.** `docs/reference/compiler.md`: the `guidance` row gains `teaching`; the Objective rows state that a title names the act and that every act is found; the §5 shared rules; the DW rows on `docs/reference/delvec/compiler/promise.md` (`DW1015`, `DW1016`, `DW1017`, the re-grounded `DW0863`) and `telling.md` (`DW0982`'s widened population); `docs/reference/dsl/textstyle.md` (the key span). `docs/reference/game-writing.md`: F1 gains its machine half (the lexicon, by pointer to the data file); §5's table gains the four rows built here and moves *Hint-for-statement* beside them as the row that stays human; §8 L1 gains the sight rule as its machine half. `docs/reference/i18n.md`: the key span and the per-language lexicon. `docs/reference/tools.md`: the translator keeps a key span whole.

**Skill, at the re-pin** (`tools/ci/check-skill-page.py` rule 17 refuses a page naming a code the pinned release predates, as spec-0093 §11 records): `references/quest-capabilities.md` §Objectives replaces its first bullet with the five obligations of §1 in the author's terms — every act is in sight of the last or carries a `title` and `hint`; the title opens with the kind's verb, and the page lists the five sets; the `teaching` block and the key span; a prop with no twin in three blocks; a hint names only what the player has been told — each with its code; `references/content.md` adds the `teaching` block to the order of `quests.json` (after `cast`, before the objectives) and names the three codes that arrive with a written stage 5; `references/writing-craft.md` §F gives functional text the verb rule and the key span beside the styled spans. Text that names no new code may land with the implementation; the rest lands with the pin that carries it.

## 11. Measurements this spec states

Taken on the tree the implementation lands with, written into the commit body that lands them: the `DW1015` count on the motivating campaign's 18 titles and 12 hints against the hand count of §2.7 (9 refused), per language; the `DW0863` binding on that campaign (acts, announced, in sight, refused) and on the gallery; the gallery's `DW1017` binding and the distance of the demo level's lever to its nearest fence post before and after the move; the gallery coverage line (units enumerated, bound, refusal-proven, 0 in neither state); the width the key span is measured at for `use` and `attack`.

## 12. What stays open

- **A journal.** The hint is shown once, at activation. A player who missed it has nothing to re-read; `game-writing.md` §8 L1 speaks of a journal the engine does not have. Recorded as a gap; a re-readable surface is a spec of its own.
- **A hint's landmark.** Whether a hint names a place the player can find is prose (§2.4): the demo level and QC read it.
- **A sign as a prop.** A sign is right-clicked to read in vanilla only where the game lets the player edit it; in adventure mode the click reaches the engine's hitbox over it and nothing else. Whether a sign *reads as pressable* is QC; `DW1017` only keeps it alone among signs.
- **The skill's own ordering defects** — a reference view refused before the gate that would record its approval, a build run before the detail pass it needs — are defects of the creator's procedure, not of the player's guidance, and are fixed on their own pull requests; this spec touches neither.

## Acceptance criteria

Each is checked against the tree at `5961145d8`, where none holds; the implementation lands them on this branch in one pull request, with the next minor `dsl_version` moved through `tools/lib/version_sites.py`'s rows and every document in the repository.

1. **Surface.** `delvec schema --stage quests` exports `Guidance.teaching` and the four `Teaching` class fields, each `string | null`; `tools/ci/check-gallery-coverage.py` reports every one bound or refusal-proven, and the key span binds no unit. A document declaring no `teaching` canonicalises byte-identically (`crates/dsl` round-trip test).
2. **`DW0863`, every act.** Over a synthetic world: (a) an untitled `interact` in a straight corridor from the previous act passes and the binding line counts it in sight; (b) the same with a wall between is refused naming the first blocking cell; (c) the same at 65 blocks with a clear line is refused naming the distance; (d) the same announced with a hint passes; (e) a `use` trigger the path performs behind a wall is refused with the trigger's remedy; (f) a `kill` in sight at 20 blocks with reach 16 is refused naming the reach, and passes at 12; (g) a `kill` in reach behind a wall is refused — the tightening, asserted. The gallery probe `a-fight-nobody-points-at` is still refused with `DW0863`; `a-step-out-of-sight` is refused with it. The first act of a path is judged from the entry cell (a test moves the entry and the verdict moves).
3. **`DW1015`.** Each row of §4's seed is in `act-verbs.json`; a test holds every language's five sets pairwise disjoint; `Talk to the miller` passes on a `talk-to` and is refused on a `reach-anchor` naming `talk` as a `talk-to` verb; `Go to the mill` is refused on a `talk-to`; `The Mill` is refused on every kind; the hint `Past the bell, left of the arch.` passes on every kind; the hint `Walk past the slate.` is refused on an `interact` and passes on a `reach-anchor`; the zh-cn title 去磨坊 is refused on a `talk-to` and 找磨坊主 passes; a sidecar language with no table is counted unjudged in the binding line. The motivating campaign's 18 titles measure as §2.7 says, or the spec's count is corrected in the commit body.
4. **The key span.** `[[key|use]]` parses; `[[key|fly]]` is `DW0975`; a line carrying it lowers to a `with` argument `{"keybind":"key.use"}` and a `fallback` with `%1$s`; `l10n::plain` renders it `[use]`; `DW0330` measures it at the pinned default key name's width; `DW0976` refuses a zh-cn row that drops it; `pytest tools/tests` asserts `check_row` refuses a translation that alters the identifier inside a key span.
5. **Teaching emission.** On a fixture whose first `interact` has a prop: `activate_<obj>` carries the `use-block` line under `dw.taught_use-block` and a second `use-block` act's activation does not; a `use` trigger that is the first `use-block` act emits a firing-cell poll that shows the line and sets the score; a PackTest drives the poll with a dummy on a firing cell and asserts one line and the score; a class with `null` emits nothing for it; two builds are byte-identical.
6. **`DW1016`.** Shape A raised for a fixture whose path performs a `use` trigger with no `use-block` entry; silent with the line; silent with `null` and the binding line counts it declared taught by the level; shape B raised for an `attack-block` line on a campaign with no `strike` trigger; shape C raised for a `use-block` line with no span and for one carrying `[[key|attack]]`; the gallery probes for A and C are refused with the code.
7. **`DW1017`.** A lever prop with a second lever at distance 2 is refused naming both cells; at distance 4 it passes; a lever beside a lever `[facing=north]` is refused (state ignored); an adopted barrel with a barrel at 1 is refused; an unmarked prop-less `interact` standing on an authored lamp beside a second lamp is refused and passes with `marker: shown`; a `strike` trigger over an authored block among its twins is refused. The gallery probe `a-lever-in-a-row-of-levers` is refused with the code.
8. **`DW0982`, widened.** A hint naming an NPC whose first line is later on the walk is refused naming the objective; the same hint after the NPC has spoken passes; a title that introduces the name by apposition passes; the probe `a-hint-that-names-the-untold` is refused. The binding line's population counts objective strings.
9. **Binding lines.** Every build prints the lines of §3, §4, §5.5 and §6 with counts computed from the objects; a campaign with no objectives prints zeroes, not nothing.
10. **Demo level.** `docs/demo-levels.md` carries *The Ferry Bell*; the level on content branch `demo/the-ferry-bell` builds, passes PackTest, the bot walk and the staging gate, and its commit body records the lever's distance before and after the move.
11. **Pages, same change.** `docs/reference/compiler.md`, `promise.md`, `telling.md`, `textstyle.md`, `game-writing.md`, `i18n.md` and `tools.md` carry §10's text (`tools/ci/check-dw-codes.py` and `check-doc-dupes.py` green); `docs/specs/README.md` carries this spec's row; the skill pages carry the text of §10 that names no new code, and the text that does lands at the re-pin (`tools/ci/check-skill-page.py` green at both points).

## Sources

- Game Accessibility Guidelines. *Include tutorials*; *Include contextual in-game help / guidance / tips*; *Give a clear indication that interactive elements are interactive*. gameaccessibilityguidelines.com.
- Minecraft Wiki. *Text component format* (the `keybind` content); *Tutorial hints*.
- Mojang bug tracker. MC-157896 (the client's `shouldRenderAtSqrDistance`, as quoted there).
- Nielsen Norman Group. *10 Usability Heuristics for User Interface Design*, heuristics 4 and 6.
- Engadget. *GDC08: Live from the Portal post-mortem* (the force field changed to glass).
- `docs/reference/game-writing.md` and `docs/reference/web-narrative.md`, with the sources they list (Microsoft Writing Style Guide; Lynch via Totten; Rogers via Doctorow).
