# Game writing — evidence, audit and check design

Evidence behind [game-writing.md](game-writing.md), for the same reader and for
whoever builds the check. Nothing here is a rule; the rules are on that page.

## 1. What was measured, and with what

- **Corpus.** `stellarfeline/delvewright-campaigns` at `04159aa461310bb0f5f66b67f32070997f5a3cad`.
  Vesperhold's sidecar `campaigns/vesperhold/l10n/zh-cn.json` (blob `69d60222a0fb`)
  holds every inventoried player-facing string: its `source` map is the English,
  its `content` map the Chinese.
- **Denominator.** 385 keys, 323 unique English strings. Twelve of the thirteen
  `quest.<q>.goal` rows are inventoried but never displayed (at engine revision
  `1ef29efa` the only reader of a goal is `campaign_outro` in
  `crates/delvec/src/compiler/emit.rs`, which shows the finale's), so the
  denominator is **311 displayed unique strings (373 displayed keys)**: 172
  functional (objective titles and hints, item and refusal hints, buttons,
  tooltips, class blurbs, shop text, system messages) and 139 story (dialogue
  text, barks, narration).
- **Rater.** One rater (the author of this page) labelled every displayed unique
  string. The counts are one rater's judgement, not a consensus; the protocol in
  game-writing.md §6 is the cross-check. The README is outside the sidecar and
  outside these counts.
- **Classes.** English: **H** a detail standing in for the statement it implies;
  **A** an aphoristic, echoing or symmetrical ending (including tricolon
  fragments); **W** a gag or non sequitur that carries no function; **O**
  functional text that does not say what to do, what it does, or what is wrong.
  Chinese-only (on top of what the English carries): **Zr** semi-classical
  register mixed into speech; **Zo** set-phrase or slogan parallelism; **Zt**
  calque; **Zn** one name rendered two ways.

## 2. Counts (Vesperhold, displayed unique strings, n = 311)

| Class | Unique strings | Displayed keys | Of which functional |
|---|---|---|---|
| H hint-for-statement | 21 (6.8%) | 21 | 1 |
| A aphoristic / symmetrical ending | 23 (7.4%) | 37 | 5 |
| W gag / non sequitur | 16 (5.1%) | 30 | 11 |
| O functional text that does not instruct | 15 (4.8%; 14 of 172 functional, 8.1%) | 28 | 14 |
| Any English class | **65 (20.9%)** | 99 of 373 | — |
| Zr archaic register (zh) | 7 | 7 | — |
| Zo slogan / set phrase (zh) | 4 | 4 | — |
| Zt calque (zh) | 23 | 44 | — |
| Zn name rendered two ways (zh) | 4 | 5 | — |
| Any Chinese-only class | **34 (10.9%)** | — | — |
| Any class, either language | **80 (25.7%)** | — | — |

Against the 139 story strings alone, H is 20 (14%) and A is 18 (13%). Six of
Zr's seven are one character's lines (Halvard), whose register the campaign's
generation record says was set to "archaic and courteous" on purpose; nine of
W's sixteen are shop tooltips (nine of the ten).

## 3. Lines and fixes

Fixes keep every fact of the original. EN fixes are English rewrites; ZH fixes
are rewrites of the Chinese (from the fixed English where the English changed).

| # | Key | Class | Original | Fix |
|---|---|---|---|---|
| 1 | `cast.the-wardens-ledger.halvard.0.bark.0` | H, N1 | The scriptorium. The warden went there every morning. | Try the scriptorium, south of the cloister. The bell-warden worked there; her ledger should still be on the desk. |
| 2 | `fx.the-last-vesper.oc.choose.0.seq.1.0.narrate` | H | Down in the ward, a soldier reads the name on his bunk out loud. | The court remembers who it is. Down in the ward, a soldier reads his own name off his bunk, out loud. |
| 3 | `fx.the-last-vesper.oc.choose.1.seq.1.1.narrate` | H | On the road you try to remember what the castle was called. | On the road you try to remember the castle's name, and cannot. The grey water has taken it from you too. |
| 4 | `fx.the-wardens-key.oc.hear-the-warden.2.seq.0.8.narrate` | H | The key is still on the hook. | The echo fades, but the key is real. It still hangs on the hook; take it. |
| 5 | `fx.trig.sally-tally.0.narrate` | H | Forty strokes cut into the lectern: … None of them is struck through. | … Not one is struck through: none of them came back. |
| 6 | `fx.the-wardens-ledger.done.0.narrate` | H | A cold draught comes up the stair. | A cold draught comes from behind the psalter shelves. There is a space behind them. |
| 7 | `dlg.tamsin.tamsin-who.text` | H, A | "I carried the warden's lantern once. That's all you need." | "I was the bell-warden's apprentice. I carried her lantern. I'd rather not say more yet." |
| 8 | `dlg.ilse-hesk.hesk-cost.text` | H, A | "A day for every ringing. I write down which one, so something keeps it." … "I don't know the song any more. Only that there was one." | "Each time I ring the bell, it takes one day out of my memory. I write down which day, so the ledger keeps it for me." … "I don't remember the song now." |
| 9 | `cast.the-bell-road.tamsin.0.bark.0` | H | You saw her. Don't tell me what she said. | Name who "her" is: "You saw my mother in the echo", then the request. |
| 10 | `dlg.tamsin.tamsin-tongue.text` | H | "… The chapel will remember what it was like when she still rang. …" | "… Hold the tongue to the shard and the chapel will show you the last morning the bell rang. …" |
| 11 | `fx.the-king-who-kept-the-watch.done.0.narrate` | H | The king says, very clearly: "Keep the gates shut." Then he does not say anything. | The king says, very clearly: "Keep the gates shut." Then he dies. |
| 12 | `dlg.halvard.halvard-dont-know.text` | H, A | "… We walked it hard against the outer curtain, a hand on the stone. Never down the middle. I never thought to ask why, and now there is nobody left to ask." | "… Walk the east wall the way we did: against the outer parapet, a hand on the stone, never down the middle. I never learned why." |
| 13 | `dlg.pellam.pellam-sold.text` | A | "I sell everything, friend. That's the one true thing about me." | "They paid more than you, friend. I sell to whoever pays most." |
| 14 | `dlg.halvard.halvard-door.text` | A | "Every morning I choose one and stand at it. It seems the least I can do." | Keep the first sentence; end on "I don't know what else to do." |
| 15 | `dlg.halvard.halvard-what.text` | A | "… I have been choosing a door every morning for ten years. I am tired of choosing." | "… I have chosen a door every morning for ten years. I want someone else to decide." |
| 16 | `dlg.young-tamsin.apprentice-hammer.text` | A | "If there's no bell there's nothing to ring. That's all. That's the whole plan." | "If there's no bell, she can't ring it, and she can't forget me." |
| 17 | `lethal.undertide.message` | A, O | The grey water takes your name first, and then the rest of you. | You drowned in the Undertide's grey water. |
| 18 | `class.wall-breaker.blurb` | A, O | A heavy axe and heavier armour. Slow, and hard to stop. | Front-line fighter. Heavy axe, heavy armour; slow, but very hard to kill. |
| 19 | `dlg.ilse-hesk.hesk-why.text` | A | "Because if I stop, the grey comes up and takes everyone's days at once instead of mine one by one." … | "Because if I stop ringing, the grey water rises and takes everyone's memories at once. This way it only takes mine, a day at a time." … |
| 20 | `cast.the-ringer-unmade.tamsin.0.bark.1` | A | I hear it from here. I always hear it from here. | I can hear the bell from here. I have heard it every night for ten years. |
| 21 | `cast.beneath-the-psalter.pellam.0.bark.0` | W, A | Buying or lying, friend? I do both. | Buying, friend? Everything on the blanket is for sale. Tallow only. |
| 22 | `cast.beneath-the-psalter.pellam.0.bark.2` | W | I was a monk. Or I knew one. | I was a monk, once. Or so I tell pilgrims. |
| 23 | `shop.pellam.offer.0.tooltip` | W, O | Sixteen arrows, fletched with something that was a goose once. | Sixteen arrows. |
| 24 | `shop.pellam.offer.6.tooltip` | W, A, O | "Proof against any blade," he says. Against some. | Protection II: less damage from every attack. Apply it at the anvil. His boast may follow, without the punchline. |
| 25 | `stake.tallow.collected` | W | Your tallow is where you left it, still warm. | You pick up the tallow you dropped when you died. |
| 26 | `fx.beneath-the-psalter.oc.find-the-pool.1.narrate` | W, N1 | The water is singing without a voice. | This is the Undertide: grey water that hums, low and steady, and takes memories. |
| 27 | `fx.shop.pellam.9.1.narrate` | W, O | "The bell tower door opens for anyone who knocks three times." He says it with great confidence. | A paid secret is true and actionable: the shortcut it sells, in one sentence. |
| 28 | `fx.the-bell-road.oc.clear-the-rampart.2.save_label` | O | Only warm up | Save, don't rest |
| 29 | `obj.the-bell-road.open-the-wardens-door.missing_item_hint` | O | Knocking does nothing. The keyhole is old and large. | Locked. You need the Warden's Key. |
| 30 | `obj.the-chapel-echo.wake-the-shard.missing_item_hint` | O | The shard hums. It wants the rest of its bell. | Hold the bell's tongue to wake the shard. |
| 31 | `obj.the-ringer-unmade.ring-the-vesper.missing_item_hint` | O | There is a bell frame and no bell. You carried its tongue up from the pool. | Hold up the bell's tongue under the frame to ring it. |
| 32 | `obj.the-cloister-fire.find-the-cloister.title` | O | Find shelter | Reach the cloister |
| 33 | `obj.the-bell-road.survive-the-knives.hint` | H, O | Somebody paid them to wait here. | On the high walk west of the watch tower. Pellam sold you out. |
| 34 | `dlg.tamsin.tamsin-throne.opt.0.tooltip` | A, O | The castle remembers. Tamsin forgets. | Tamsin rings the bell. The court remembers; she will forget you. |
| 35 | `dlg.tamsin.tamsin-throne.opt.1.tooltip` | A, O | The watch ends. The castle forgets itself. | The grey water rises. The castle and its people are forgotten. |
| 36 | `fx.shop.pellam.0.0.narrate` | O | Pellam spreads his hands. "Tallow first, friend." | Not enough tallow. Pellam spreads his hands: "Tallow first, friend." |
| 37 | `class.sellsword.blurb` | O | Sword and board. Holds a doorway while the others work. | Sword and shield. Holds the front so the others can work. |
| 38 | `dlg.halvard.halvard-cloister.text` (zh) | Zr | 守望的朋友们。在下哈尔瓦德·德雷爵士，誓守国王之门。是哪一扇，我已记不起了。诸位可曾见到国王？他还活着吗？ | 几位好。我是哈尔瓦德·德雷爵士，发过誓替国王守门——可守的是哪扇门，我记不清了。你们见到国王了吗？他还活着吗？ |
| 39 | `dlg.halvard.halvard-tower.text` (zh) | Zr, Zt | 哈尔瓦德把他的守望挪到了城墙顶上。“守望的朋友们。这段城墙还不安全。等安全了，我有一事相询。” | 哈尔瓦德换到城墙顶上守着了。“这段城墙还不安全。等清干净了，我有件事想问你们。” |
| 40 | `dlg.halvard.halvard-door.text` (zh) | Zr, Zt | 每天清晨我择一扇门，守在那里。这是我至少能做的。 | 每天早上我挑一扇门，在那儿守着。我也不知道还能做什么。 |
| 41 | `class.wall-breaker.blurb` (zh) | Zt (meaning reversed) | 重斧，更重的甲。慢，但挡不住。 | 前排。重斧重甲，走得慢，但很难被打倒。 |
| 42 | `dlg.tamsin.tamsin-throne.opt.0.tooltip` (zh) | Zo | 城堡记起一切，塔姆辛开始遗忘。 | 塔姆辛去敲钟：城里的人会想起一切，她会忘了你们。 |
| 43 | `quest.the-last-vesper.goal` (zh) | Zo | 与塔姆辛一起决定：这口钟，和被它锁住的城堡，将何去何从。 | 和塔姆辛一起决定：敲不敲这口钟，城堡还要不要封着。 |
| 44 | `cast.the-ringer-unmade.tamsin.0.bark.0` (zh) | Zt | 要敲她，就只敲一下。她受不住更多了。 | 要敲，就只敲一下。再敲她就撑不住了。 |
| 45 | `shop.pellam.offer.0.tooltip` (zh) | Zt | 十六支箭，箭羽来自某个曾经是鹅的东西。 | 十六支箭。 |
| 46 | `obj.the-cliff-path.put-down-the-grooms.title` (zh) | Zt | 从马夫手里活下来 | 打退马夫 |
| 47 | `cast.beneath-the-psalter.pellam.0.bark.2` (zh) | Zt | 我当过修士。要么就是认识一个。 | 我当过修士。也可能只是认识个修士。 |
| 48 | `dlg.tamsin.tamsin-want.text` (zh) | Zt | 城堡底下有钟的一块东西。钟舌。… | 钟有一块落在城堡底下了，是钟舌。… |
| 49 | `fx.the-causeway-fire.oc.hear-tamsin.4.sealed_hint` (zh) | Zn | 守卫长之门锁着。钥匙在守卫长手里。 | 守钟人之门锁着。钥匙在守钟人那里。 |
| 50 | `fx.the-causeway-fire.oc.hear-tamsin.2.sealed_hint` (zh) | Zn | 边门锁着。看门人身上有钥匙。 | 侧门锁着。门卫身上有钥匙。 (and `gate-warden` gets its own word, e.g. 守门官) |
| 51 | `fx.trig.casting-ledger.0.narrate` (zh) | Zn | 应守钟人之请，为晚祷堡铸造。… | 应守钟人之请，为维斯珀堡铸造。… |

## 4. Dangling references

### Play order

Earliest display order is derived statically from the stage documents: class
pick and world title; quests in `depends_on` topological order; inside a quest,
objectives in `after` order, each followed by its `on_objective_complete`
effects; then `on_complete`. A `spawn-wave` or `spawn-actor` places the names it
spawns. NPC lines are placed at the quest where the NPC is first reachable (its
first `talk-to` objective or dialogue entry node, checked against the campaign's
route in `DESIGN.md`), not at the first `cast` row, because a cast row stands an
NPC at an anchor the party may not reach for several quests. A `close-gate`'s
`sealed_hint` is placed where the gate is first reachable on the route.

Cross-check: the static objective order for Vesperhold equals the objective
order of the compiler's own branch chronicle
(`validation/branch-chronicle-halvard-kept-oath+ring.md`, written by `delvec
1.7.1` built at `1ef29efa`, the campaign's `dsl_version` raised in a scratch copy
only so the current engine would read it): 37 of 37 objectives in the same
order. The other three campaigns are at older `dsl_version`s the current engine
refuses, so their order is the static derivation alone.

### Verdicts

A name counts when it occurs inside a sentence at least once (not only as the
label on its own body or item). **Introduced**: by its first such occurrence the
player has seen a body or item carrying it, or a sentence has said what it is.
**Bridgeable**: the sentence itself makes the kind evident (a head noun, "son
of", "runs into"). **Dangling**: neither.

| Campaign | Names | Introduced | Bridgeable | Dangling |
|---|---|---|---|---|
| Vesperhold | 21 | 14 | 4 — Vesper ("Rang the Vesper"), Keep Doors, Warden's Door, Thaw ("the eighth of Thaw") | **3** — *Warden* (first in the objective `Read the Warden's Ledger` and Halvard's bark, before anything says the warden rings the bell); *Undertide* (objective `Find the Undertide`, never glossed); *Tallow* (a sidebar counter from the start, earned from the first kills, explained only when Pellam says "Tallow only" in the eighth quest) |
| The Hollow Vigil | 6 | 4 | 1 — Maren ("the one Maren sent for") | **1** — *First Warden*: the player's own option `Who is the First Warden?` before anyone has named him |
| Doune Castle tour | 15 | 9 | 4 — Ardoch, Teith ("runs into"), Stirling, Highlands | **2** — *the Jacobites*; *Montrose* ("Montrose quartered here in 1645, in the middle of the wars") |
| Nobody's Isle | 16 | 11 | 4 — Troy, Athena (item name plus "the grey-eyed goddess"), Laertes ("son of"), Crete | **1** — *Maron* ("The strong wine of Maron" in the class blurb; item `Skin of Maron's Wine`) |
| **Total** | **58** | **38** | **13** | **7 (12%)** |

Definite references that act as names, outside the counts above: Vesperhold
*the grey* (its identity with the Undertide is never stated) and *the watch*
("Friends of the watch"); The Hollow Vigil *the order* (class blurb, before any
order is mentioned); Doune *the coconuts* (a film joke never explained) and *the
wars*, and *the duke*, introduced only on an optional branch; Nobody's Isle *the
scouts* and "what they saw in the cave".

Consistency findings beside the audit (N4): Vesperhold names one hazard three
ways (the Undertide, the grey, the grey water) without equating them; its Chinese
renders one name two ways four times, and two English names as one Chinese
word once (§2 Zn, §3 rows 49–51); its shop label `Book of Smite III` delivers an
item named `Litany for the Unquiet`.

## 5. Check design (not built)

### Dangling-reference check

1. **Order.** Use the compiler's play order, not a re-derivation: `Flow::journal`
   (the order `validation/branch-chronicle-*.md` prints and `Flow::replay`
   proves), per reachable branch. Every l10n key has a JSON pointer into its stage
   document; join each key to the first journal step whose quest, objective,
   effect or dialogue choice owns that pointer. Positional strings (a
   `sealed_hint`, a `use`/`approach` trigger, loot, shop text) take the first step
   at which their anchor is reachable under that step's gate seals — the same
   per-step seal set `nav::check_branch_path` already routes over.
2. **Names.** A campaign glossary: every name the documents declare (NPC, actor,
   mob, item, class, bonfire prompt, shop title, state) plus every invented term
   the author declares, each with a one-line gloss and the event that introduces
   it (a body, an item, or a string key whose text glosses it). A capitalised
   mid-sentence term absent from the glossary is reported as a candidate for the
   author to declare.
3. **Rule.** A glossary term's first reference (a mention inside a string that is
   not its own label) must not precede its introducing event on any branch. A
   dialogue option label that mentions a term must come after the term's
   introduction in that NPC's tree.
4. **Chinese.** Every string whose English contains a glossary term contains
   that term's one Chinese rendering; no two glossary terms share a rendering.

Measured on the static order, the capitalisation sweep as a candidate generator
over all four campaigns returned 39 candidates (after dropping title-case
objective fragments and contractions), of which 6 are real; it missed *Montrose*
because the name opens its sentence. That is 15% precision and 6 of 7 recall: a
list for the author, not a refusal. The glossary rule itself has no
precision problem — it checks an obligation the author stated — and its recall
is the glossary's completeness.

### Tell detectors

Scored on Vesperhold's 311 displayed strings against §2's labels. TP/FP/FN are
string counts.

| Detector | Rule | Hits | TP | FP | FN | Precision | Recall |
|---|---|---|---|---|---|---|---|
| A — echo closer, symmetry | last sentence ≤ 7 words repeating a content word of an earlier sentence, or a closer phrase (`that's all`, `the one true`, `only that`, `the least I can do`); a tricolon fragment `a X, a Y, a Z.`; two adjacent sentences of 2–6 words starting with the same word | 14 | 9 | 5 | 14 | 64% | 39% |
| H — detail | `still`, `remembers`, `without a (bell\|voice)`, `is empty` | 16 | 6 | 10 | 15 | 38% | 29% |
| O — functional without action or place | functional key with no sentence opening on an imperative, no direction, no named object, no number | 26 | 8 | 18 | 7 | 31% | 53% |
| W — item tooltip without function | shop/item tooltip with no mechanical word (`damage`, `heal`, `restore`, `undead`, `anvil`) | 9 | 8 | 1 | 8 | 89% | 50% |
| Zr — archaic markers | 诸位、在下、可曾、相询、直言相告、若、便、择 | 9 | 7 | 2 | 0 | 78% | 100% |
| Zt — calque markers | 对此、颇为、没有任何、某个、曾经是、被 (not 被遗忘)、受不住更多、的其余部分、一块东西、手里活下来 | 11 | 9 | 2 | 14 | 82% | 39% |
| Zo — slogan parallelism | two clauses of equal length 4–8, or 何去何从/宣誓效忠 | 5 | 1 | 4 | 3 | 20% | 25% |
| Zn — glossary consistency | glossary term in English, its rendering absent in Chinese | 4 | 3 | 1 | 1 | 75% | 75% |

False positives worth seeing: A flags `The postern is locked. The Porter had a
key.` (a repeated word in a plain refusal); H flags `The king still needs his
door held.` (a literal "still"); O flags every combat hint that names the enemy
but not a direction (`Archers hold the wall walk.`); Zr flags 然后便再没有说话
(a natural 便) and the warden's diary 若 (a register the diary may carry); Zt
flags 何去何从's 被 (a real defect, but of class Zo); Zn flags 卫队 for 卫兵 (a
near-miss that is itself a small inconsistency). The missed Zn is the reverse
collision (`Porter` and `gate-warden` both 门卫), which needs the
no-shared-rendering rule of §5 step 4.

**What to build, if anything.** Refusal-grade (precision ≥ 75% here, and the
pattern is mechanical): the glossary rules (dangling, one rendering per name,
no shared rendering), Zr and Zt markers, the item-tooltip-without-function rule.
Advisory only: A, Zo and O, which fire on good lines often enough that a
refusal would train authors to dodge the regex. Not mechanical: H. These
estimates come from one campaign and one rater; they are the number to beat,
not a guarantee.

## 6. Research notes

- **LLM style, English.** Instruction-tuned models overuse present participial
  clauses, nominalisations and phrasal coordination and favour words such as
  "tapestry" (Reinhart et al., PNAS 2025); "It's not X, it's Y" is 6.3× more
  prevalent than in human writing in some models, and some patterns over 1,000×
  (Paech et al., *Antislop*); the skill's StoryScope evidence locates the deeper
  tell in narrative structure (explicit themes, somatic emotion, quiet endings).
  None of these sources studies very short game strings; the H, A and W classes
  are this audit's own, read off the Vesperhold text.
- **LLM style, Chinese.** HC3 (Guo et al.) compares human and ChatGPT answers in
  both languages: ChatGPT is "typically formal" where people are colloquial,
  less emotional, and uses more conjunctions. Chinese reporting on "AI味"
  (Tencent News / 半月谈) lists high function-word density, uniform sentence
  length, habitual 排比 and metaphor, and tidy 总分总 structure. No peer-reviewed
  study of Chinese LLM fiction style was found; the Zo rule rests on these two.
- **Translationese.** Translated Chinese measured against native Chinese
  (balanced corpora ZCTC and LCMC): lower lexical density (61.59% vs 66.93%),
  more conjunctions (306.42 vs 243.23 per 10,000 tokens), more 被-passives
  overall but less marked in imaginative writing (Xiao, IJCL 2010). LLM
  translations show translationese that supervised fine-tuning introduces (Li et
  al., ACL 2025). LLMs writing Chinese directly still show English-influenced
  lexical and syntactic patterns (Guo et al., ACL 2025, "English accent").
  Regenerating from a meaning representation reduces translationese (Wein &
  Schneider, EACL 2024); eliciting native writing from storyboards instead of
  source text gives better fluency and worse accuracy (Kuwanto et al.,
  LREC-COLING 2024).
- **Game localisation.** Mangiron & O'Hagan (JoSTrans 6, 2006), on Final
  Fantasy: the brief is a version players "experience … as if it were
  originally developed in their own language"; translators "are often given
  carte blanche to modify, adapt, and remove" references, with "quasi absolute
  freedom"; "transcreation, rather than just translation, takes place". The
  search for independent per-language authoring as standard practice found
  none; sim-ship localisation runs in parallel with development but still
  localises one source.
- **FromSoftware.** Item descriptions mix lore with a plain functional sentence
  (Estus Flask: "Fill with Estus at bonfires, and drink to restore HP"; Flask of
  Crimson Tears: "this flask restores HP with use. Rest at a site of grace to
  replenish"), and one-way doors say "does not open from this side" (as players
  quote it). The obliqueness is in what the world withholds, not in how a
  prompt is worded.
