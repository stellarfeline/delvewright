# Game writing — how each kind of player-facing text reads naturally

For the agent writing a campaign's text: objective titles and hints, buttons,
tooltips, barks, dialogue, narration, item and shop text, and the Chinese
sidecar. The `/new-delve` skill's *Reference: writing craft* covers automatic
phrasing and narrative convergence; this page covers what each class of text is
for, how to introduce a name, and how to write the Chinese. Evidence, the
Vesperhold audit and the check design are in
[game-writing-evidence.md](game-writing-evidence.md).

§8 says where the player's information comes from at all (L1) and that the text
tells it plainly and truly (L2); §1-§3 say how each line of the text those rules
allow is written.

Every rule is marked **cited** (with its source) or **authored** (this project's
rule, with the reason it exists). An authored rule is a decision, not a finding.

## 1. Two registers, never mixed

Player-facing text is either **functional** (it tells the player what to do,
what a thing does, or why something failed) or **story** (it tells the player
what happened and who these people are). Functional text is never oblique, in
any game, however oblique its story.

- **F1 — Functional text says the action, the object and the place.** Start with
  the verb; name the thing; say where by a landmark the player can see and a
  position relative to it or to the player (past the bell, left of the arch,
  behind you), never by a compass point: Minecraft shows no compass, so
  `north` leaves the player guessing. `Hold the Warden's Key to the iron-bound
  door past the broken bell.` not `The keyhole is old and large.`, and not
  `the door at the west end of the walk`. The landmark rule is **authored**.
  **Cited:** Microsoft Writing Style Guide, *Top 10 tips* ("start each statement
  with a verb", "get to the point fast"); Game Accessibility Guidelines, *Use
  simple clear language* (basic level; about 14% of US/UK adults read below an
  11-year-old's level).
- **F2 — A refusal says what is wrong and what fixes it.** A `missing_item_hint`,
  a `sealed_hint`, a shop refusal: `Locked. You need the Warden's Key.` One line
  of flavour may follow the statement, never replace it: `Not enough tallow.
  Pellam spreads his hands: "Tallow first, friend."` **Cited:** Nielsen Norman
  Group, *Error-Message Guidelines* ("use human-readable language"; "offer
  constructive advice … offer some potential remedies").
- **F3 — Oblique lore lives only where nothing depends on it.** The model is
  FromSoftware: item descriptions carry lore, but the same description states
  the function in a plain sentence ("Fill with Estus at bonfires, and drink to
  restore HP"; "this flask restores HP with use. Rest at a site of grace to
  replenish"), and system messages are flat ("Door does not open from this
  side"). Lore is optional reading layered on text that already works.
  **Cited:** item descriptions as transcribed by the Fextralife wikis (Dark Souls
  Estus Flask; Elden Ring Flask of Crimson Tears); the door message as quoted by
  players on the Steam and GameFAQs forums; Miyazaki's stated preference for
  players finding the plot "from items or side-characters" (Wired interview,
  via Wikipedia). The application to Delvewright's classes is **authored**.
- **F4 — Button labels are the player's own words, at most 20 characters
  (12 in Chinese).** The budget is the widget's (skill §C, `DW0331`). A label
  that answers "what will happen if I press this": `Save, don't rest` beats
  `Only warm up`. A choice tooltip states the consequence: `Tamsin rings the
  bell. The court remembers; she will forget you.` **Authored** (the consequence
  rule); the width is measured.
- **F5 — A shop or item tooltip says what the item does first.** `Protection II:
  less damage from every attack. Apply it at the anvil.` The merchant's lie may
  follow. A paid secret is true and actionable. **Authored**, by F3's pattern.
- **F6 — A class blurb names the role in plain words.** `Front-line fighter.
  Heavy axe, heavy armour; slow, but very hard to kill.` not `Slow, and hard to
  stop.` **Authored.**

## 2. Story text: say it

- **S1 — State the fact; a detail may accompany a statement, never stand in for
  it.** The defect is a single image the reader must decode into the sentence
  it implies (`The key is still on the hook.` meaning *the echo was real, take
  the key*; `On the road you try to remember what the castle was called.`
  meaning *you have forgotten it too*). Write the statement, then keep the image
  if it earns its place: `The court remembers who it is. Down in the ward, a
  soldier reads his own name off his bunk, out loud.` **Cited:** Grice's maxim
  of Manner, "be perspicuous; so avoid obscurity and ambiguity" (Stanford
  Encyclopedia of Philosophy, *Implicature*); skill §B (StoryScope: somatic
  showing is the machine default, naming the feeling is the human one).
- **S2 — End on information.** A line does not close on a maxim, an echo of its
  own words, or a balanced antithesis: `That's the one true thing about me.`,
  `I am tired of choosing.`, `The castle remembers. Tamsin forgets.` Stop after
  the last fact. **Cited:** the "rule of three" and negative-parallelism entries
  of Wikipedia's *Signs of AI writing*; Paech et al., *Antislop* (some patterns
  over 1,000× more frequent in LLM output than in human text; "It's not X, it's
  Y" 6.3× more prevalent in some models). The "stop after the last fact" form is
  **authored**.
- **S3 — A joke carries the function too, or it goes.** A merchant may be funny;
  nine gag tooltips in a row that never say what the item does are a defect
  (F5). A non sequitur (`I was a monk. Or I knew one.`) is a quirk only when the
  quirk is the character's point and the scene has already told the player what
  they need. **Cited:** Slabinski, *8 Key Principles of Writing Effective Game
  Dialogue* (concision first; barks are feedback to the player). The test is
  **authored**.
- **S4 — Let people say what they want and fear.** `I'm afraid of it. Go.` works
  because it is plain. **Cited:** skill §B.
- **S5 — Lore is optional; the critical path is plain.** Exposition the player
  must have goes in a line they cannot miss; history goes in an optional branch
  or a lectern. **Cited:** Slabinski ("the treachery of forced lore").

## 3. Introduce a name before you use it

- **N1 — The first time a name reaches the player in play order, it is either a
  body or item carrying that name, or a sentence that says what it is.** `Find
  the Undertide` as an objective title, before any line has said that the
  Undertide is the pool of grey water that eats memory, leaves the reader
  nothing to attach it to. A definite reference (`the Warden`, `the grey`, `the
  order`) is the same case as a capitalised name. **Cited:** Haviland & Clark
  (1974), the given-new strategy: a definite noun phrase is processed by finding
  its antecedent, and comprehension is slower when there is none and the reader
  must build a bridge. The play-order rule is **authored**. The body that
  introduces a name is a person speaking to the player under it, or an item in
  the player's hands; a nameplate over a body the party fights, a counter, a
  bar's title and an area's name show a name without saying what it is, and a
  definite mention (`the last of the watch`) presupposes it — none of these
  introduces. **Authored**, from a playtest in which an option spoke of `the
  watch`, a name the party had only seen over the bodies it fought.
- **N2 — A head noun that names the kind is an introduction; an invented word is
  not.** `the Keep Doors`, `the Warden's Door`, `the eighth of Thaw` explain
  themselves. `the Undertide`, `Tallow` (a sidebar counter shown from the first
  minute and explained only when a merchant names it), `Maron` do not. Give an
  invented word one plain clause the first time: `the Undertide, the grey water
  under the castle`. **Authored**, from N1.
- **N3 — The player never asks about a name they have not heard.** A dialogue
  option `Who is the First Warden?` before anyone mentioned him is the player
  character knowing something the player does not. **Authored.**
- **N4 — One thing, one name; one name, one thing — in every language.** Vesperhold
  calls one hazard `the Undertide`, `the grey`, `the grey water`; its Chinese
  renders `the Porter` as both 门卫 and 看门人, uses 门卫 again for `gate-warden`,
  renders `Vesperhold` once as 晚祷堡 among 维斯珀堡, and `the Warden's Door`
  once as 守卫长之门 among 守钟人之门. Pick one rendering per name and copy it.
  **Authored**; it extends skill §D from bodies to every mention.
- **N5 — Real-world names in a tour get the same treatment.** A Chinese reader
  does not know who `Montrose` or `the Jacobites` are. One clause each.
  **Authored.**

- **N6 — A name tag marks a person.** A body wears a name only when the story
  treats it as someone: a boss, an elite, a named actor, an NPC. Ordinary bodies
  fight under their kind and wear nothing. Three zombies of one wave each tagged
  `The Watch` is the defect; three plain zombies, with a stated bar title `The
  Watch` over the fight if it needs a heading, is the form, and one `The Watch
  Captain` among them, alone under its name, is a character. **Authored.**

## 4. Chinese

These rules are what `tools/creator/i18n-translate.py` sends the transcreating
model: it reads this section's bullets, each cut before its **Cited** or
**Authored** tail, so a rule edited here is the rule sent. Who writes the
Chinese is §7.

- **C1 — Write the Chinese from the scene's intent, with the English as the fact
  reference.** Read what the line must do (the beat's `happening`, the speaker's
  persona, what the player must learn), then write the line a Chinese writer
  would write; keep every fact, name, number and direction of the English. Do
  not translate sentence by sentence. **Cited:** Mangiron & O'Hagan (2006):
  game localisation is "transcreation"; localisers are given "quasi absolute
  freedom to modify, omit, and even add" so players "experience the game as if
  it were originally developed in their own language". See §6 for the evidence
  on why this beats translation for an LLM.
- **C2 — Modern spoken Chinese at the character's register.** No
  semi-classical grammar mixed into speech (诸位、在下、可曾、若……便……、一事相询、
  直言相告) — an old knight is old by what he says, not by 文白夹杂. A word or two
  of period flavour is enough. **Cited, ideas only:** 思果《翻译研究》 against
  mixing classical particles (应、即、乃) with modern speech. The register rule
  for characters is **authored**.
- **C3 — No slogans.** No four-character set phrases strung into parallel
  clauses (城堡记起一切，塔姆辛开始遗忘), no 将何去何从, no 公文腔. **Cited:** the
  feature list in reporting on "AI味" (habitual 排比, uniform sentence length,
  high density of function words; Tencent News / 半月谈 article on AI writing);
  Guo et al. (HC3): ChatGPT answers in both English and Chinese are "typically
  formal" where people are colloquial. The register rule is **authored**.
- **C4 — No calques.** 受不住更多了, 从……手里活下来, 某个曾经是鹅的东西,
  这是我至少能做的, 一块东西 are English grammar in Chinese words. The translationese
  checklist sent with these rules (名词化, 弱动词, 的的不休, 被, front-loaded
  modifiers) applies. **Cited:** Xiao (2010): translated
  Chinese has lower lexical density (61.59% vs 66.93%) and more conjunctions
  (306.42 vs 243.23 per 10,000 tokens) than native Chinese, and more 被-passives
  overall; 余光中《怎样改进英式中文》 (ideas only).
- **C5 — Check meaning after rewriting.** 慢，但挡不住 reads as *slow, but cannot
  block* — the opposite of `hard to stop`. Reread every Chinese line for what it
  says on its own. **Authored.**

## 5. What a machine can check

Summary of the design in the evidence file §5. Four rows are built. One name,
one rendering is the transcreation tool's fact check, run on every line it writes
([i18n.md § The fact check](i18n.md#the-fact-check)). Dangling name, for the
names that reach dialogue, is `DW0982`: the declared names (the inventory's
`name` and item-name rows, the fact check's set) placed in the compiler's play
order against what has told them (N1, N3). A question without an answer is
`DW0981`: an option that asks and opens no line. A name tag on a crowd is
`DW0983` (N6, [compiler.md § DW0983](compiler.md#dw0983--a-name-tag-marks-a-person-compilertelling-error-exit-1)). All three refuse at `delvec validate` ([compiler.md § DW0981/DW0982](compiler.md#dw0981dw0982--what-the-dialogue-tells-the-player-compilertelling-error-exit-1)).
The rest is not built.

| Check | What it reads | Estimated precision on Vesperhold |
|---|---|---|
| Dangling name (`DW0982`, built for dialogue) | Every NPC line and option label or tooltip at its earliest display step in the compiler's play order (the replay's walk, on the critical path and every branch), against the declared names and what has told them: the person speaking, the class and kit, an item handed over, or a sentence that says what it is | Refused uses on content main: Vesperhold 1 of 4 (`Tallow`), and, order-free on the campaigns the current engine cannot read, Hollow Vigil 3 of 3 (`First Warden`) — each a name the evidence file's audit rates dangling; none rated introduced is refused. A place or thing the documents never name (an anchor) is out of its reach |
| Name tag on a crowd (`DW0983`, built) | Every wave entry and actor carrying a name that one wave spawns more than one body under | Content main: 17 of 21 waves refused (Hollow Vigil 2 of 2, Nobody's Isle 3 of 3, Vesperhold 12 of 16), 0 of 36 named actors; every wave left standing is a single named boss or elite |
| Question without an answer (`DW0981`, built) | Every dialogue option whose label or tooltip, in any language, ends in `?`/`？` | 0 of 73 question options on content main refused; in the gallery and its overlays 2 of 5, both in the site-plan overlay, since answered |
| One name, one rendering | Glossary names × the Chinese of every string that contains them | 3 of 4 hits real (75%); recall 3 of 4 |
| Archaic register (zh) | Marker list (诸位、在下、可曾、相询、若、便、择) | 7 of 9 (78%); recall 7 of 7 |
| Calque markers (zh) | Marker list | 9 of 11 (82%); recall 9 of 23 |
| Item tooltip without function | Shop/item tooltip with no mechanical word | 8 of 9 (89%); recall 8 of 16 |
| Echo closer / symmetry (en) | Last sentence repeats a content word, tricolon fragment, twin short sentences | 9 of 14 (64%); recall 9 of 23 |
| Functional text without action or place | Functional keys lacking an imperative, a direction or a named object | 8 of 26 (31%) — not usable as a refusal |
| Hint-for-statement (en) | No mechanical signal found | 6 of 16 (38%) — human review only |

Precision is against one rater's labels and is an estimate; §6 of this page is
the cross-check.

## 6. Native-reader rating protocol

Fifteen minutes per volunteer, three to five native readers per language.

1. **Pairs (10 min).** 20 slots per language, stratified across the defect
   classes plus 2 control slots whose two versions differ only trivially. Each
   slot shows the speaker and one line of situation, then the original and the
   rewrite, left/right randomised, origin hidden. Question: *Which reads more
   like something a person would write here?* — A / B / no difference.
   **Cited:** Callison-Burch et al. (2007): relative ranking gave higher
   inter-annotator agreement (κ = .373 for sentence ranking) than absolute
   5-point fluency scales (κ = .250).
2. **Names (3 min).** The first five minutes of play as a script; the reader
   marks every word they could not place.
3. **Functional (2 min).** Eight objective or refusal lines; the reader picks
   what they would do next from three options.

**Comparison.** Primary: per slot, the majority verdict; the rewrite wins a
language when it takes at least 15 of 20 slots (one-sided sign test, p = 0.021).
Secondary: all non-tie judgements pooled (with five raters, at least 59 of 100).
A control slot preferred by a clear majority flags position bias. Report per
class, never just the total. **Authored**, with the binomial thresholds computed.

## 7. Parallel authoring: the answer and its cost

**The established practice is neither independent parallel authoring nor
sentence translation: it is one source, localised by native writers licensed to
rewrite (transcreation).** No source found treats independent per-language
authoring as standard game practice. For an LLM the evidence points the same
way: its translations carry translationese traced to fine-tuning (Li et al.,
ACL 2025); text regenerated from a meaning representation reads more like
native text (Wein & Schneider, EACL 2024); writing from a non-text prompt
instead of translating gives better fluency and worse accuracy (Kuwanto et al.,
LREC-COLING 2024); and even an LLM writing Chinese directly shows English
patterns (Guo et al., ACL 2025), so authoring in parallel with the same model
does not remove the defect by itself.

**The practice.** C1: the other language is written from the intent of each
line, with the English kept as the fact source and the key of record, and name,
number and placeholder consistency checked mechanically. The English is still
the canonical source; the sidecar's `source` map, the staleness check and the
width gates are unchanged. Each inventory row carries its intent (`kind`,
`speaker`, `situation`) beside its English, derived from the documents.

**Who writes the Chinese.** Claude writes the English and, through the
documents, each line's intent; the Chinese is transcreated by DeepSeek through
`tools/creator/i18n-translate.py`. In a blind native-reader trial, DeepSeek
transcreating existing lines from English, speaker and situation was chosen in
9 of 12 decisive slots, against the original Chinese in 2 and Claude's
rule-guided Chinese in 1; writing fresh from a brief, no writer led, and rule
guidance did not change Claude's count (evidence §7). In-agent Chinese is the
fallback when no key is configured.

## 8. The level is what the player reads

- **L1 — The level carries the information; the text only restates it.** A
  closed door with a lever beside it needs no explanation. Story text comes
  from NPC dialogue and a few books kept in bookshelves. The objective journal
  restates what the story has already told the player, updating as they learn
  it (step one `Talk to the innkeeper`; once her dialogue has said where to go,
  step two `Go to the mill`), so a player who skimmed or forgot can look it up;
  it never names a place, person or thing the player has not been told. A
  destination is recognised by its building, and must read as what it is at
  playable scale (a tavern looks like a tavern). Guidance cues in the level
  (a path, a handhold) are diegetic and consistent across the design, blended
  into the environment and found on a close look, never a jarring highlight. No
  object exists only so that reading it opens something. **Authored.** A glowing
  entity, floating label or beam over a target is the defect; so is a sign
  pointing at a destination, or a note, plaque or lectern whose only job is to
  be read so that a gate opens.
- **L2 — Mystery comes from the telling and the events, never from obscured
  wording; whatever the text says of the world is true.** A line the player must
  decode to learn what they need is S1's defect, in every register. Dread comes
  from who is speaking, what they saw or did not see, and what they will not
  say: `Where the light forgets itself, the second door listens.` is the defect;
  `The second door is in the unlit cellar. The cook will not go down there
  again, and she will not say why.` is the form. The text never contradicts
  what the level shows: a creature that moves and strikes is not called a
  statue. An unknown thing may go unnamed or be mentioned obliquely
  (`something at the bottom of the well`), but every claim made of it is true
  in the story. **Authored.**
- A speaker never narrates the level's design. `Most walk past both.`, said by
  a keeper of a lever and a bell the player has just used, is the designer
  remarking on players through a character; the keeper says what this place is
  and what just happened, in his own voice. **Authored**, from a playtest; no
  machine check reads it.

## Sources

- Callison-Burch, Fordyce, Koehn, Monz, Schroeder. (Meta-) Evaluation of Machine Translation. WMT 2007.
- Fextralife wikis: Dark Souls *Estus Flask*; Elden Ring *Flask of Crimson Tears*.
- Game Accessibility Guidelines. *Use simple clear language*. gameaccessibilityguidelines.com.
- Grice, H. P. Logic and Conversation (1975), as summarised in the Stanford Encyclopedia of Philosophy, *Implicature*.
- Guo, Conia, Zhou, Li, Potdar, Xiao. Do Large Language Models Have an English Accent? ACL 2025. arXiv:2410.15956.
- Guo et al. How Close is ChatGPT to Human Experts? (HC3, English and Chinese). arXiv:2301.07597.
- Haviland, S. E. & Clark, H. H. What's new? Acquiring new information as a process in comprehension. JVLVB 13(5), 1974.
- Kuwanto et al. Mitigating Translationese in Low-resource Languages: The Storyboard Approach. LREC-COLING 2024. arXiv:2407.10152.
- Li, Zhang, Wang, Zhang, Cui, Yin, Xiao, Zhang. Lost in Literalism: How Supervised Training Shapes Translationese in LLMs. ACL 2025. arXiv:2503.04369.
- Mangiron, C. & O'Hagan, M. Game Localisation: Unleashing Imagination with "Restricted" Translation. JoSTrans 6, 2006.
- Microsoft Writing Style Guide. *Top 10 tips for Microsoft style and voice*.
- Nielsen Norman Group (Neusesser, Sunwall). *Error-Message Guidelines*.
- Paech, Roush, Goldfeder, Shwartz-Ziv. Antislop. arXiv:2510.15061.
- Reinhart et al. Do LLMs write like humans? Variation in grammatical and rhetorical styles. PNAS 122(8), 2025.
- Slabinski, M. 8 Key Principles of Writing Effective Game Dialogue. Game Developer.
- Tencent News / 半月谈. AI味儿到底是一种什么味儿？文字创作也有"预制菜".
- Wein, S. & Schneider, N. Lost in Translationese? Reducing Translation Effect Using Abstract Meaning Representation. EACL 2024. arXiv:2304.11501.
- Wikipedia. *Signs of AI writing* (ideas only, as in `docs/ACKNOWLEDGEMENTS.md`).
- Xiao, R. How different is translated Chinese from native Chinese? IJCL 15(1), 2010.
- 余光中《怎样改进英式中文？——论中文的常态与变态》; 思果《翻译研究》 (both copyrighted; ideas only).
