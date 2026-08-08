---
name: new-delve
description: Generate a complete playable Minecraft delve from a creative prompt — staged DSL authoring with validation-loop self-repair, deterministic compile, machine validation, joinable output. Use when the user asks to create/generate a new delve or campaign. Args = the creative prompt (theme one-liner or detailed brief).
version: 1.1.0
requires:
  delvec: ">=1.0.0 <2.0.0"
verified_with: 1.1.0
---

# /new-delve — the Delvewright generation front-end (ADR-0012)

You are authoring a delve campaign as staged DSL documents. You NEVER write
mcfunction, dialogs, or datapack files — `delvec` compiles everything (ADR-0001).
Read `CLAUDE.md` first if you haven't; forbidden zones apply in full.

## Inputs

The user's prompt is a **constraint set over the DSL stages**: honor everything it
pins down (theme, specific levels, plot beats, NPCs, homages); invent the rest.
Ask 2–3 clarifying questions ONLY if the prompt is too thin to pick a theme and a
target length; otherwise proceed.

Mode: default **interactive** (pause after each stage for the user to review a
short summary — not the raw JSON — and confirm/adjust). Switch to **e2e** and run
straight through whenever the user asks for an uninterrupted end-to-end run — in
English ("e2e", "don't stop") or in the equivalent Chinese shorthand.

### Showcase mode (thin prompts)

When the prompt pins down little — a one-line theme, no detailed brief — treat it
as a **SHOWCASE** brief: a thin prompt is creative license, and the goal is to
deliberately exercise the breadth of the currently-supported feature set so a
stranger playing the result discovers what the engine can do (this is our primary
marketing artifact). Do NOT hardcode a feature list here — v0.4 is landing soon and
any list would rot; instead **query the live schema** (`delvec schema --stage <n>`
across stages) for the available verbs/effects, then aim to include, wherever the
story can carry them coherently: multi-area transport as a narrative beat,
flag-gated dialogue consequences, real props / set-dressing, at least one tuned
combat or stealth encounter, narration beats, and varied NPC presentation. **Rule
of thumb: coherence and pacing always win over feature count** — never bolt on a
mechanic the story can't motivate. A **detailed brief is the opposite**: honor
exactly what it pins down and showcase nothing extra.

## Execution architecture (delegation + models)

The **main (authoring) agent** does HIGH-LEVEL creative work ONLY: theme, beats,
personas, quest-plan intent, the stage summaries, ALL user interaction, and
visual-review judgment. The **mechanical writing of each stage's JSON and the
`delvec validate` repair loop** are delegated to a **dev subagent** — hand it the
creative brief for the stage plus the schema command (`delvec schema --stage <n>`);
it returns valid stage JSON and a short summary of the choices it made, which you
fold into your stage summary. The **validation ladder** stays on a **test
subagent** (step 8). The **branch chronicle review** (step 7) is the authoring
agent's own: it is narrative judgment against `DESIGN.md`, and delegating it
would hand the design's intent to somebody who never held it.

Model policy for subagents: **dev subagents run `opus`; test / validation subagents
run `sonnet`.** A subagent must **NEVER run a higher tier than the main agent
itself** — if you are running on a lower tier, clamp every subagent down to your own
model (e.g. main agent on `sonnet` → all subagents `sonnet`).

## Campaign workspace (artifact of record — NEVER skip)

Campaigns do not live in this repo (CLAUDE.md forbidden zone) — they live in the
**`delvewright-campaigns` git repo** (symlinked at `campaigns/`, real path
`../delvewright-campaigns/`; override: `$DELVEWRIGHT_CAMPAIGNS_DIR`). Create
`campaigns/campaigns/<campaign-id>/` with the six stage JSONs, a
`GENERATION.md` (prompt verbatim, date, dsl_version, decisions made, and the
campaign's **posture note** — see *Writing craft* §B), and a
`DESIGN.md` (the authoritative design record: layout, dramaturgy beats,
branch/ending table); build
output goes beside them (gitignored there). After validation passes, **commit the
campaign in that repo** (conventional message; do not push unless asked). The DSL documents are the artifact
of record: the delve must be rebuildable byte-identically from them without any
LLM (ADR-0006/0012).

**Iteration protocol (owner ruling 2026-08-03 — NEVER skip on any revision
round):** `DESIGN.md` is the single authoritative design document. Every
iteration round, however mechanical its trigger, (1) changes ONLY what the
user asked to change — a mechanics fix must not incidentally rewrite story,
staging, or dialogue; (2) updates `DESIGN.md` in the same commit when an
approved change moves the design; (3) ends with a **conformance review**:
diff the campaign's current behavior against `DESIGN.md` beat by beat, and
report any deviation the user did not request instead of shipping it. Drift
discovered during review is restored to the design or escalated — never
silently kept.

## Writing craft — pattern warnings, not technique bans

Everything a player reads is prose: dialogue, objective titles and hints,
narration beats, item and area names, the storybook. This section is the craft
checklist for all of it. **Hand it to the dev subagent verbatim with every
stage-5 and stage-6 brief**, and run section A over every line before a stage is
called done.

These are **pattern warnings, not technique bans.** They govern *automatic*
writing — the phrasing that arrives before you have decided anything, the hand
reaching before the mind does — not the device itself. A simile is not
forbidden; the simile you did not choose is. Banning a technique outright
produces stilted avoidance, which is its own tell.

### A. Automatic-phrasing tells

1. **Observation + verdict.** A line, then the text grading it — "*…, more
   statement than question*", "*…, and it was not a request*". The verdict
   instructs the player how to hear what they just read. Cut the verdict; if the
   line cannot stand without it, the line is wrong.
2. **Standalone simile fragments.** "*Like a blow to the chest.*" — a comparison
   set alone as though it were the feeling. Test every simile: does it make the
   player see the **thing** more sharply, or make them notice the author? The
   second kind goes.
3. **Stock intensity moves.** The air growing thick or heavy; time slowing;
   silence stretching; words left hanging in the air; a breath the character
   did not know they were holding. These are the default gestures at "this
   moment matters", and every generated delve reaches for them unprompted.
4. **Repetition as intensity.** Saying it again, louder — "*more than tired:
   hollow*"; three-beat lists where two beats carry all the meaning.
5. **Correction pairs.** Naming a false label in order to knock it down — "*not
   a warning, a promise*"; "*it stopped being a door and became a mouth*". Once
   per campaign is a rhetorical choice; three times is a signature.
6. **Purposeless gesture.** A nod, a tightening jaw, a hand moving to a hilt,
   costing nothing to delete. A gesture signifies by contrast with what that
   character usually does. If it can be cut with no loss, cut it.
7. **Explaining your own subtext.** An NPC says the hard thing, then the next
   line paraphrases what it meant. Trust the player; they have already read it.

Applies hardest where our text is shortest: `hint`, `title`, bark pools, and
`missing_item_hint` have no room to recover from a wasted clause.

### B. Convergence is the real tell — vary the posture per campaign

StoryScope (arXiv:2604.03136) separates human from AI fiction at **93.2%
macro-F1 from narrative structure alone, with every stylistic signal withheld**,
and span-level style editing of the prose moves that number by 1.6 points. So
the AI tell is not a phrase you can scrub. It is **convergence**: five different
models occupy one tight region of narrative space while human stories are
dispersed around it (mean rarity 0.49 vs 0.71). Section A is hygiene; this
section is the actual defence.

Measured gaps worth authoring against (AI vs human in that corpus):

| axis | the machine default |
|---|---|
| thematic explicitness | the narrator states the story's point — 77% vs 52% |
| emotion rendering | somatic: tight chest, cold sweat — 81% vs 38%. **Humans name the feeling outright 29% of the time; AI 8%.** |
| plot shape | no subplots 79% vs 57%; protagonist-driven resolution 69% vs 46% |
| resolution | closes on internal understanding or acceptance, 47% vs 27% |
| time order | strictly chronological; humans jump, flash back, withhold |
| morality | morally ambivalent protagonist 38% vs 59% |
| address | humans break the fourth wall (67% vs 39%) and address the audience (28% vs 7%) |

**Claude specifically** — us — is the most distinctive of the models measured,
and its fingerprint is restraint: *the flattest event escalation of any source*,
the most uniform narrative voice, epilogues over avalanche endings, and
reverence toward genre convention rather than subversion (62% vs 39–56%). Read
that as a standing instruction: **our default delve escalates too evenly and
ends too quietly.** Give a campaign a beat that is disproportionate to what came
before, and let at least one thing end badly or unresolved.

Operationally, per campaign:

- Pick **at least three** axes above and push them off the default *for this
  campaign* — a delve told out of order; a cast whose antagonist is right; an
  ending that refuses to explain itself; an NPC who names their fear in plain
  words instead of clenching a fist.
- Record the choice as a one-line **posture note** in `GENERATION.md`: which
  three axes, and how. It is a design commitment, not a report.
- Vary them **between** campaigns. A fixed counter-recipe applied every time
  just builds a second cluster — dispersion is the human signal, not any
  particular pole.
- Corollary, and it inverts the usual advice: **"show, don't tell" is a machine
  default here.** Somatic rendering is what the pole looks like. Sometimes let a
  character say they are afraid.

### C. HARD RULE — dialogue options are labels, not sentences

Owner ruling, 2026-08-03. **A dialogue option is a button caption.** Vanilla
draws each option as a fixed-width button; a label wider than the button
*scrolls* rather than wrapping or shrinking, and a shelf of scrolling captions is
a miserable thing to read and pick from. This is not a style preference — it is
the widget.

The geometry, so the budget is arithmetic and not taste. The compiler emits each
node as a `minecraft:multi_action` dialog with `columns: 1` and **no `width`
override**, so every option button is vanilla's default **150 GUI px**, leaving
roughly **146 px** for the label after the widget's inset. Dialog buttons draw at
pose scale ×1, so one font pixel is one GUI pixel — unlike `narrate` titles,
which `DW0330` budgets at ×4/×2 (`crates/compiler/src/textfit.rs`).

Mirror that module's reasoning: **width is measured in font pixels, not
characters**, because `i` and `W` differ by 3× and a Han glyph (advance 9) is 1.5×
a Latin one (typical advance 6), so any character count is unfair to whichever
script it was not tuned for. Character counts below are the authoring rule of
thumb derived from those advances — the pixel budget is the real rule:

| | scroll threshold | **author to** |
|---|---|---|
| English | ~24 characters (146 px ÷ ~6 px average advance) | **≤ 20 characters** |
| Chinese (`zh-*`) | ~16 characters (146 px ÷ 9 px Han advance) | **≤ 12 characters** |

Author to the target, not the threshold: the English is the source a translation
grows from, and a label at the English limit has nowhere to go in `zh-cn`.

```
BAD   "I don't know — are you sure there isn't another way out of the cave?"
GOOD  "Another way out?"

BAD   "我不太确定，你是说这座洞窟还有别的出口吗？"     (20 chars ≈ 180 px — scrolls)
GOOD  "还有别的出口吗？"                              (8 chars ≈ 72 px)
```

The content that does not fit belongs in the node's body text, which wraps
normally, or in the NPC's reply — not in the button. This applies to every
`.opt.<n>.label`, in the English source **and** in every l10n sidecar; the
localization stage's critique pass already checks that translated labels stay
short and scannable.

A compiler diagnostic for this is being added separately (engine task #110), on
the same font-pixel measurement `DW0330` uses. Until it lands the rule is
enforced here, by you, at authoring time — and when it lands it will be telling
you the same thing this section does.

### D. Plain-prose baseline (Strunk 1918, public domain)

Two rules carry most of the load for text rendered into a chat line:

- Rule 12, "Use definite, specific, concrete language" — the objective hint that
  names a landmark beats the one that names a mood.
- Rule 13, "Omit needless words": *"Vigorous writing is concise. A sentence
  should contain no unnecessary words, a paragraph no unnecessary sentences…"*
  His substitutions are still live — `owing to the fact that` → since, `in spite
  of the fact that` → though, `he is a man who` → he, `in a hasty manner` →
  hastily. And: *"In especial the expression `the fact that` should be revised
  out of every sentence in which it occurs."*

Concision is not the same as flatness. Cut the padding, keep the beat.

## The loop

`delvec` below means the compiler binary. **In a pipeline-repo checkout — which
is where this skill runs today — build it from source**: `cargo build -p delvec
--bin delvec` (or `cargo run -q -p delvec --bin delvec -- …`). Plain `cargo
build` is the right call — the workspace's dev profile is optimized enough for a
real campaign. Do **not** reach for `--release` mid-loop: it is ~20s slower to
rebuild after every edit and the output is byte-identical either way
(`docs/reference/tools.md`).

Two other paths now exist and are equally real, so do not assume a `delvec` on
`PATH` was built from this tree (ADR-0017): `cargo install delvec`, and the
per-target archives on the `v<version>` GitHub Release. If you are handed a
`delvec` rather than building one, run `delvec --version` and check it against
`versions.toml [engine].version` before trusting any output — a campaign is
reproducible only against a named engine (ADR-0006/0016). Full comparison:
`docs/reference/tools.md`.

For each stage in order — world → npcs → classes → quest-plan → quests → dialogue:

1. `cargo run -q -p delvec --bin delvec -- schema --stage <n>` —
   generate AGAINST the live schema, never from memory.
2. **Delegate the mechanical write + validate repair loop (steps 2–3) to a dev
   subagent** (see *Execution architecture*): hand it this stage's creative brief +
   the schema command; it returns valid JSON and a summary of choices. The brief you
   hand it carries these craft constraints:
   - **Prose**: the *Writing craft* section above, verbatim, on every stage that
     writes player-facing text (5 and 6 above all) — plus this campaign's posture
     note, so the subagent writes toward the three axes you committed to.
   - Areas: prefer `prefab_pool` (stone-keep tileset) for real layouts; check
     `campaigns/prefabs/pools.json` + prefab metadata for available pools/anchors/lighting
     profiles. Respect the lighting contract — darkness only as declared design
     with a mitigation the quest DAG provides.
   - NPCs: personas per schema (archetype/speech_style/motivation required);
     honor them in every stage-6 line. Dialogue: branching options; flavor NPCs
     get real trees too.
   - **Stage 6: an option label is a button caption, not a sentence** — the
     compiler rejects over-long ones (`DW0331`, error). Vanilla draws each option
     on a fixed 150-GUI-px button and *scrolls* a label that does not fit. The
     budget is 146 font px ≈ 24 Latin / 16 Han characters; author to **≤20 Latin,
     ≤12 Han** so a translation has room to grow (a `zh-cn` sidecar is checked
     under its own key). What does not fit belongs in the node's body text, which
     wraps, in the option's `tooltip`, or in the NPC's reply — never in the button.
   - **Stage 6: `button = caption, tooltip = the full line.`** (Owner design,
     2026-08-04.) When the caption cannot carry what the character actually
     says — the wine beat, where "Pour it out." stands for a whole sentence —
     author the option's optional `tooltip`: vanilla shows it in a hover box
     beside the button. It **wraps** (no `DW0331`, no width budget), so it takes
     a full sentence. Use it for the *said line*, not for hints or mechanics;
     the button still has to be readable on its own, since a player on a
     controller or reading fast never hovers. Needs `dsl_version 0.8.0` on the
     dialogue stage, and it translates under its own key
     (`dlg.<npc>.<node>.opt.<i>.tooltip`). Full geometry rationale: *Writing
     craft* §C.
   - **Stage 6: re-derive every node's option list from that node's situation.**
     (Owner ruling, 2026-08-03.) Never carry an option list forward from an
     earlier node. Before shipping a node, check each option for semantic fit
     with what has *just happened* in the story — "would a survivor say this line
     right now?" Premise and exposition options must retire once their moment
     passes, via the cast ledger's dialogue swap (declare a later root) or a flag
     gate. A "who are we" / "what is that thing" option must be **impossible** at
     the finale. The motivating playtest defect: after the climactic escape, a
     crew NPC still offered "Tell me what he is." and "Is there another way
     out?" — questions the character had already lived through the answers to.
   - Pace to `target_minutes`; no grind (forbidden zone); mandatory-only quests.
   - Objectives (v0.3): author `title` (short player-facing name, e.g. "Unbar the
     Deep Gate") and `hint` (one-line location/direction guidance, e.g. "Past the
     entrance hall, take the left passage to the barred door") for **every
     non-`talk-to` objective**. The compiler surfaces them in-game when the
     objective activates (chat + sound); without them the player gets no guidance
     and cannot find interact/collect/reach targets. For `talk-to`, `title`+`hint`
     are **REQUIRED whenever the target NPC is not already visible from where the
     previous objective completed** (a different room, down a corridor, across an
     area) — the player otherwise gets a silent objective and wanders. Only omit
     them when the NPC is in plain sight of where the player just was; read the "may
     omit" allowance narrowly (playtest lesson: an off-screen NPC 60 blocks away
     through an unfamiliar cave left the player with no guidance at all).
   - **`interact.requires_item` is HELD, not carried** (owner ruling, 2026-08-03):
     the player must have the item in their **main hand** when they click —
     presenting it is the action. Author `missing_item_hint` whenever the
     empty-handed click deserves diegetic feedback (a sleeping giant mumbles in
     its sleep; a locked door rattles and holds) — it is narrated in chat to that
     player, only while the objective is open, and without it the click is met
     with total silence, which reads as a broken affordance.
   - **Furnish the containers.** A prefab's chests and barrels are empty until a
     stage-5 `loot[]` entry fills them (`{id, anchor, items:[{item, count?, name?,
     enchantments?}]}`), and an empty chest reads as a bug to the player. Give
     every reachable container consumables or props; a named `name` is how a
     prop becomes a story object. The container must already exist in the piece
     at that anchor — the compiler fills furniture, it never places it
     (`DW0431`). Elites and set-piece actors take `equipment` in the same shape
     wave mobs use, enchantments included.
   - **A `collect` takes its item from the room's own furniture, and the item has
     a name.** (Owner ruling, island playtest rounds 1-2.) Point the objective's
     `container` at the anchor of a chest/barrel the prefab already placed — the
     compiler then fills THAT container and places no chest of its own; a floating
     chest conjured beside the barrel the player has been walking past is the
     defect this replaced. Give the item an `item_name` ("Cheese", "Tide Ledger"):
     it is what the player reads on the stack, it translates like every other
     player-visible string, and an unnamed generic item says nothing about what
     the quest asked for. Set `fill_count` so the container reads plausibly full
     (it counts padding SLOTS after the objective's own stack — a barrel with one
     lonely wheel of cheese in it reads as a bug); `1 + fill_count` must fit the
     container's 27 slots. The container must really be there in the piece
     (`DW0438`), must not also be filled by a `loot` entry or another `collect`
     (`DW0435`), and the fields need `dsl_version` 0.8.0 on the quests stage.
   - **An elite or boss leaves ONE thing behind, and you say which.** (Owner
     ruling, 2026-08-04.) Give the fight's `drops[]` a declared subset — a
     `{"slot": "main_hand"}` for the axe the player watched swing, or a
     `{"item": …, "name": …}` for a quest token — never the whole kit. Only an
     `elite`/`boss` encounter may declare drops (`DW0491`); a slot must be one the
     same body's `equipment` really fills (`DW0490`). If that token is what opens
     the next door, take it with a `collect` that names `dropped_by: <wave>`
     instead of a container: the compiler then places no chest and PROVES the
     chain — that the wave really yields the item (`DW0492`) and that its `kill`
     objective runs first (`DW0493`). `dropped_by` names a wave, never an actor
     (an actor's death is observable by no objective). Needs `dsl_version` 0.9.0
     on the quests stage.
   - Hint wording: give landmark-relative directions from places the player already
     knows (the entrance hall, the gate, a named NPC) — never room-shape jargon
     ("corner room", "L-shaped hall") or solver-internal terms (anchor/piece/socket
     ids).
   - **A beat that can FAIL the player must not arm before they could have read
     it.** (Owner ruling, 2026-08-03.) Any fail-able beat — follow-an-NPC, escort,
     timed escape, stealth onset — arms only after a grace window long enough to
     read the on-screen prompt that explains it: the player must never be failable
     before they could have understood what is being asked. Where the DSL has an
     explicit knob, set it consciously rather than inheriting the default
     (`begin-stealth`'s `grace_ticks`); where the pacing is authored, put the first
     enforcing step late enough in the `sequence`'s `at_ticks`. Budget the window
     from the prompt's length, not from a habit — a two-line chat prompt is several
     seconds of reading before the first step is taken. The motivating playtest
     defect: the flock the player was told to follow left while they were still
     reading the instruction, and the beat then failed them for it.
   - **Stage 4: declare every story fork** (spec-0025, DSL v0.8). If a choice
     forks who lives, where the party ends up, or which ending plays, it is a
     `branch_points` entry: `{id, opens_at, forks_on:[flags], branches:[{id,
     flags, leads_to}]}`. `leads_to` is one field — a `quest/…` the branches
     converge at, or an `ending/…` this branch runs to (the id prefix says
     which). Name each ending on the `campaign-complete` that fires it
     (`"ending": "ending/<slug>"`). A flag that gates casts, staging or quest
     structure and is *not* set on every playthrough must belong to a declared
     point, or the build fails (`DW0480`). Every declared branch must reach an
     ending (`DW0482`) and must be exclusive: no sibling's flag may be producible
     on it (`DW0484`).
   - **Every story node declares a `happening`** (spec-0025). One line saying
     what the node does to the story: `{verb, text, subject?}`, where `verb` is
     one of `dies` / `survives` / `departs` / `arrives` / `learns` / `believes` /
     `gains` / `loses` / `opens` / `seals`. Required on every quest, every
     objective, every staging / wave / gate / `campaign-complete` effect, and
     every dialogue option that sets a flag — a missing one is `DW0481`. It is
     the event-flow twin of the cast ledger's `doing`: you cannot fill it without
     deciding what the beat *is*. Keep `subject` accurate (`npc/…`, `actor/…`,
     `wave/…`, `anchor/…`, or an `item/…` label) — the compiler reads only the
     verb and the subject, and uses them to catch a dead character who later acts
     or a sealed gate later walked through, per branch (`DW0485`).
   - **Post-fork casts are per branch, every quest.** After a fork opens, an
     NPC whose situation differs by branch declares a **list** of placements,
     each gated by the flags of the branch it belongs to — in *every* later
     quest, not just the first. Leaving one ungated as a fallback is `DW0483`:
     later declarations win, so the fallback keeps governing the branch that
     already has its own. This is the island round-13 defect ("the fork moved the
     ledger but never moved the bodies").
   - **Stage 5: write the `cast` block FIRST, before the objectives** (spec-0020).
     Every quest declares, for every NPC live in it, `{at, doing, dialogue}` —
     position first, story second. `at` is an anchor (or `"offstage"`/`"dead"`,
     which must match a real `despawn-npc` — declaring a position does not move
     anybody, `DW0461`). `doing` is free prose and is the point: you cannot fill
     it without deciding the character's business in this beat, and stage 6
     writes their lines against it. `dialogue` is a stage-6 root id,
     `{"barks": [...]}`, `"unchanged"`, or `"none"`.
     - **A sleeping, working or background NPC gets a `barks` pool**, not
       `"none"`. Right-click then yields one inconsequential in-character line
       (the sleeping giant murmurs; a camp's off-duty crew make small talk)
       instead of dead silence. Use `"none"` only when the silence is itself the
       statement.
     - **Write `"unchanged"` when you are deliberately carrying dialogue
       forward** — never re-spell the same root id, and never omit `dialogue`
       hoping it defaults (it does not: `DW0463`). `"unchanged"` at an NPC's
       first appearance is `DW0466`.
     - **Treat the `DW0467` staleness warning as a design smell, not a
       nuisance.** It means an NPC's right-click never learns that the story
       moved. Give it a scene that changes, or make it a bark-pool background
       character — do not silence it by shuffling spellings.
     - Omitting a live NPC is `DW0460`: an unaccounted NPC is how two crew
       members ended up standing forgotten in the alcoves while the player
       escaped the cave.
3. `delvec validate <campaign-dir>` — fix by diagnostic code (DW####; see
   `crates/dsl/README.md` + `crates/compiler/README.md` tables). Loop until clean.
   Three failed repairs on the same code → stop and think about the design instead
   of patching syntax.
4. Interactive mode: present a 3–6 line summary of the stage; wait.

### 4b. The design-alignment Artifact — MANDATORY between the plan and the content

**Owner directive, 2026-08-04.** Stages 1–4 settle *what the delve is*; stages 5–6
are where the expensive authoring happens. Between them, when the design is
settled and the pieces it needs exist, **you deliver an Artifact and stop.**

The Artifact tells the **complete story** and walks through **every scene's
design**, and each scene carries images at **both near view and far view**. Not a
document with pictures in it — a visual walkthrough, in the medium the owner
actually reviews in. She does not read long documents (CLAUDE.md PR policy); a
design she cannot see is a design she cannot approve, and every problem it would
have caught gets paid for twice once stages 5–6 are written against it.

- **Near view** = the scene as a player stands in it. **Far view** = the same
  scene in its surroundings, so staging and sightlines read.
- Prefer the **player-POV** shots (below) for near view. An orbit render answers
  "is the set pretty"; only an eye-height frame on the walk answers "what does a
  player walking in experience", and the second question is the one the review
  exists for.
- **Do not begin stage 5 until she has confirmed it.** A confirmation is her
  words in chat, not the absence of an objection.
- In **e2e mode** the Artifact is still produced and still shown — e2e removes
  the per-stage pauses, not the one gate whose whole purpose is her judgment.

This is the same principle as the branch chronicle in step 7, applied one layer
earlier: the compiler renders compiled reality back into the reviewer's own
medium, and the review compares like with like. Whenever you are tempted to add
a review step, ask first — *what does the compiler emit that shows the reviewer
the compiled reality in their medium?* If the answer is "they read the DSL", the
step is designed wrong.

### 4c. A device enters a campaign only behind a green machine gate

**Owner ruling, 2026-08-05.** If a structural device — a shortcut loop, a one-way
drop, an ambush reversal, a multi-path interlock — has no machine gate proving
its class, it does not go in the campaign yet. Never "author it now and prove it
later": the owner's QA hour is the scarce resource this whole pipeline exists to
protect, and an unproven device spends it on something a test should have caught.

When a design wants a device whose gate does not exist, that is a **capability
gap**: report it, and either the gate lands first or the design does without it.

### Supported techniques

Load-bearing patterns proven on real runs — reuse rather than rediscover:

- **`base_entity` accepts any entity id, and NPCs are inert by construction.** Every
  NPC is summoned `NoAI,Invulnerable,Silent,NoGravity,PersistenceRequired` plus a
  separate interaction hitbox, and there is no registry validation on the field — so
  any mob id becomes a talking statue that cannot move or hurt anyone. This is how a
  villager-sized cast can include a giant: e.g. `minecraft:warden` as a Cyclops you
  must slip past. *Caveat:* `Silent:1b` also suppresses that entity's ambient sounds
  (the Warden's heartbeat), and the emitted `VillagerData` tag is inert on a
  non-villager.
- **Multi-area + automatic inter-area transport is a physically enforced point of no
  return.** Placing beats in separate areas (256 blocks apart across void, no
  walkable link) makes the compiler emit a one-way teleport on the objective that
  crosses areas — the player *cannot* walk back, so "the boulder seals the cave" is
  enforced by geometry, not merely asserted. The return trip is the same mechanism
  in reverse.
- **A sealed gate answers a right-click by itself — never build the hint by hand.**
  `close-gate` arms the sealed region so pressing it puts a line on the presser's
  actionbar; `sealed_hint` on the effect is only the *wording* (unauthored, the
  compiler says "The way is sealed."). Do **not** add a `use` trigger on the gate
  anchor to get this — that is the co-located second hitbox the compiler now
  rejects (`DW0422`). A `strike`/`use` trigger anchored on the gate anchor is still
  legitimate for a *different* line (it rides the seal's own hitboxes and is live
  only while the gate is sealed); two `close-gate`s on one anchor must agree on the
  wording (`DW0423`).

### Authoring tools (know these exist; reach for them by symptom)

Two classes, one rule each (owner, 2026-08-02):

- **LLM-facing tools are workflow steps, not options.** Where a step below says
  "always", skipping it is skipping validation.
- **Human-in-the-loop tools are offered, never required.** When the flow reaches
  the marked point, tell the user in one line that the tool exists and what it
  would catch — then keep going. Never block or wait on a use/don't-use answer.

The full inventory — every binary, script and flag that exists today — is
`docs/reference/tools.md`. Check it before assuming a capability is missing.

Symptom → tool:

- **Judging any visual outcome** (cutscene framing, set dressing, terrain):
  `delvec snapshot` (`--camera x,y,z,yaw,pitch`, `--at <anchor> --dist`,
  `--shot <render-plan id>`, `--labels`) and `delvec blocking-chart` (per-floor
  cutaways). *Always* for cutscenes: render start/mid/end of every dolly
  segment and **look at the frames** before calling the stage done — `DW0308`
  proves the path is air, not that the shot is pointed at the subject
  (round-6 shipped an inside-out cinematic that was fully DW-green).
- **Terrain/visual fixes beyond swapping prefabs**: `delvec edit` — the
  spec-0017 map editor loop (edit script batch → replay → snapshot). Never
  hand-patch `.nbt` or invent block edits outside it.
- **Handing a build to the owner to play**: mention the playtest note flow
  (spec-0006: `/trigger dw.note` in-game, then `delve-harvest` →
  `playtest-report.json`) — one line, human-optional.
- **Delivering or revising a cutscene**: mention spec-0019 shot calibration —
  in-game `/trigger dw.mark set <s>` (stand where the camera should be),
  `dw.aim set <s>` (look at the subject), `dw.faster`/`dw.slower set <s>`,
  then `/trigger dw.done` once; `delve-harvest` writes
  `rehearsal-report.json` and `delvec calibrate <report> --layout
  <out>/creator-datapack/layout.json` turns it into an anchor+offset patch you
  apply and rebuild. One line, human-optional. (Beat replay — `dw.beat` /
  `dw.shot` / `dw.free` — is not landed yet; do not promise it. See
  `docs/reference/compiler.md` §8.)
- **Declared non-English languages**: `delvec l10n-inventory` +
  `tools/i18n-translate.py` per `docs/reference/i18n.md` — workflow step,
  see the Localization stage below.
- **The layout needs a prefab the library doesn't have**: import it with
  `delve-schem convert`, then run the **whole** `delve-admit` admission chain
  (`audit` → `resolve-jigsaw` → `socket` → `anchor` → `lighting --write` →
  `catalog validate`; that order — `resolve-jigsaw` before `socket`). Never place
  an un-audited piece: `audit` is the ADR-0013 licence/code-injection gate, and
  an unadmitted piece has no anchors or lighting profile for the DSL to name.
  Flags in `docs/reference/tools.md` §3.
- **An NPC needs a look no vanilla mob gives you**: the skin toolchain
  (`tools/skin`, spec-0009) composes an original 64×64 skin from a cast-sheet
  entry and renders previews — `python -m delve_skin all <cast.json>
  --skins-dir … --preview-dir … --catalog-dir …` in its own venv. **Look at the
  previews**, and always set `model` (`wide`/`slim`) — an omitted model renders
  slim and distorts a wide skin. The compiler bakes the PNG into the delve's
  resource pack from `campaigns/<id>/skins/`.
- **Whole-scene or player-POV shots for review**: `validation/render-shots.sh
  <build-dir>` — emits the Chunky scene set + shot index from the build's
  `render-plan.json`. First-person POV shots only exist on this path (the
  per-prefab renderer is an orbit renderer and cannot stand inside a room).
  Chunky is the official renderer for these frames; render + extract commands,
  the pinned core and the parallel/tiered-SPP doctrine are in
  `docs/reference/tools.md` §4a.
- **A picture of the whole map** (storybook hero image, release asset):
  `delve-render panorama <build-dir> -o <dir> [--bearing se|sw|ne|nw] [--spp N]`
  — a 45° oblique scene framing the entire layout, computed from the plan. Never
  hand-edit a scene JSON to get one.
- **Re-running the machine ladder after a fix**: the ladder entry scripts
  (`validation/bot-run.sh` / `packtest-run.sh` / `branch-runs.sh`, all
  `--project <id>`) fresh-volume their own project before and after every run,
  so a persisted world volume can no longer keep completed objectives completed
  and fail a "fresh" playthrough for reasons that have nothing to do with the
  delve. To clean up by hand: `validation/fresh-volumes.sh --project <id>`.
  `--project` is required everywhere and there is no daemon-wide mode.
- **A `talk-to` / `interact` step that times out with "objective … did not
  complete"**: read the rest of that line before touching the campaign. The bot
  now reports the server's own answer to the `/trigger` it sent — *the server
  ANSWERED …* means the trigger reached the delve and a datapack guard consumed
  it (a re-used world whose scoreboard already carries the objective does
  exactly this: run `fresh-volumes.sh --project <id>` and re-run before believing
  the content is at fault), while *the server never answered …* means the command
  never got there and the failure is the harness's, not the delve's.
- **A prefab library needing owner taste, not machine checks**: mention
  `delve-admit gallery` (browse world) → owner walks it and leaves notes →
  `delve-admit curate` / `curate-merge` fold them into the catalog cards — one
  line, human-optional.

### Localization stage (only when the prompt asks for other languages)

If the prompt requests one or more languages — or the user prompts in a
non-English language **and asks for localized in-game text** (中文文本 etc.) — add a
**final generation stage after `dialogue`**, once the English campaign is complete:

1. Declare the codes in `world.json`: `"languages": ["zh-cn", …]` (BCP-47-style;
   `en` is implicit/canonical and is **never** listed). Stage docs stay English.
2. **Who translates** — if the repo's `delvewright.toml`/`delvewright.local.toml`
   has an `[i18n]` section AND the env var it names (`api_key_env`) is set, run
   `python3 tools/i18n-translate.py <campaign-dir> --lang <code> --reflect`
   (external LLM API; `--reflect` is the three-step translate → critique →
   revise pass and is where translationese actually dies — always pass it. It
   writes and validates the sidecar for you, then go to step 4).
   Otherwise translate in-agent, steps 3–4. Generation-time only either way —
   shipped delves never call an LLM. See `docs/reference/i18n.md`.
3. In-agent: `delvec l10n-inventory <campaign-dir> --lang <code>` gives the exact
   key inventory as JSON (key, English, speaking NPC, existing translation).
   **Translate FROM the finished English** (never author a language natively) —
   honor each NPC's `persona.speech_style`, keep a Minecraft-appropriate register,
   cover the inventory **exactly**. Run the **same three-step pass the tool
   runs** (`docs/reference/i18n.md`): draft, then re-read draft against English
   and write down what is wrong on accuracy / fluency (incl. the target
   language's translationese habits — for zh: 名词化, 弱动词, 的的不休,
   over-marked 被, front-loaded modifiers) / style-register / terminology, then
   revise — leaving lines that were already right byte-identical. Write
   `l10n/<code>.json`:
   `{ dsl_version, campaign_id, kind: "l10n", lang: "<code>", content: { <key>: … } }`.
4. Re-`validate` until zero `DW0180`/`DW0181`. The default build stays English;
   `delvec build --lang <code>` emits the localized delve (same layout, strings
   swapped; `critical-path.json` is language-neutral so the ladder is unchanged).

Then:

5. `delvec analyze <campaign-dir>` — reachability/deadlock/dark-mitigation. Fix in
   the DSL (never by weakening the campaign; a dead quest is a design bug).
6. `delvec build <campaign-dir> -o <workspace>/out` — must exit 0.
7. **Branch chronicle review (spec-0025 §4) — MANDATORY, per branch, whenever the
   stage-4 plan declares `branch_points`.** Yours, not a subagent's: this is
   narrative judgment. Skip it and the campaign is not verified, however green the
   ladder is.

   The compiler has compiled the DSL **back into natural language**:
   `<workspace>/out/validation/branch-chronicle-<branch>.md` is one branch's
   storyline in compiled play order — every reachable node's `happening` line,
   first beat to ending — and `validation/branch-plan.json` lists the branches.
   Whether the DSL matches the design is not something you can check by simulating
   compilation in your head, so you compare like with like: NL against NL (the
   decompilation principle, spec-0025). Dialogue text carries meaning no compiler
   can check — "Where is Antiphos, Captain" is wrong only because Antiphos is
   alive HERE.

   For **each** branch in `branch-plan.json`:
   a. Read its chronicle **end to end, in order, in one pass.** Do not skim and do
      not sample: what this catches are contradictions in SEQUENCE ("Antiphos
      survives" at line 12, "Elpenor mourns Antiphos" at line 31).
   b. Read it against `DESIGN.md` — the intent document, already conformance-
      reviewed. Every beat the design promises on this branch must appear in the
      chronicle; every beat in the chronicle must be one the design licenses on
      this branch.
   c. Read it against the dialogue **reachable on that branch** (the stage-5 cast
      ledger's roots for this branch, and the trees they reach under its flags).
      **Every dialogue line touching branch-divergent state — who is alive, who is
      where, what was sealed, opened, lost or gained — must be LICENSED by a
      chronicle line of that branch.** An unlicensed line is a finding, not a
      matter of taste.
   d. Write the **citation table into `GENERATION.md`** — it is the artifact of
      record, and "reviewed" is checkable, never folklore. Every finding AND every
      clearance cites chronicle lines by number:

      | branch | claim reviewed (dialogue/design beat) | chronicle line(s) | verdict |
      |---|---|---|---|
      | `branch/flee` | Elpenor: "We lost him at the mouth." | 14 `departs` | cleared |
      | `branch/flee` | Kalliope: "Antiphos is dead." | — | **FINDING** — no chronicle line licenses a death on this branch |

   The pass **fails** if any branch-divergent dialogue line has no citation, if a
   branch has no table rows at all, or if any row's verdict is a finding. A
   finding is fixed in the DSL (move the line behind the right flag, swap the
   cast's dialogue root for that branch, or fix the branch the beat is on) and the
   review re-run — never argued away, and never left for the owner's QA hour.
8. Machine validation ladder — **delegate to a `sonnet` subagent** (owner policy
   2026-07-30: execution is mechanical, no creativity needed; also keeps long
   server logs out of the authoring context). Spawn an Agent
   (`subagent_type: general-purpose`, `model: sonnet`) instructed to, from repo
   root (docker required):
   - copy/point `validation/delve-output` at the build output
   - **pick a compose project id for this ladder** — `dw-<campaign>-r<round>` or
     anything unique — and pass it to every command below. It is REQUIRED: the
     validation stack pins no container name and publishes no host port (task
     #185), so the compose project is the only name the stack has, and two
     ladders with distinct ids run side by side on one host with no lock and no
     queueing. There is no mutex to take. An entry script invoked without
     `--project` fails loudly rather than landing in a shared default.
   - `EULA=TRUE validation/packtest-run.sh --project <id>`
   - `EULA=TRUE validation/bot-run.sh --project <id>`
     — each fresh-volumes its own project before and after (a stale world carries
     the scoreboard: completed objectives stay completed and the bot reports a
     false CONTENT failure), and tears down only that project. The bot ladder has
     two labelled stages once the delve has mandatory combat (spec-0023):
     `critical-path` and `die-retry`. The die-retry stage adds two scripted
     deaths per encounter, so a combat-heavy delve needs a larger
     `DELVEWRIGHT_RUN_TIMEOUT_MS` than the 20-minute default — set it on the
     command (`DELVEWRIGHT_RUN_TIMEOUT_MS=2400000 EULA=TRUE validation/bot-run.sh
     --project <id>`); compose forwards it to the bot. Read
     `validation/run-out/<id>/run-report.json` afterwards — project-scoped, so two
     ladders can never overwrite each other's. It names every combat-assist
     window, every death trial, and any inverted-floor-gate finding. An EMPTY
     `assist_windows` is not evidence of anything on its own — read the
     `encounters` block beside it, which states each encounter's assist policy
     and the phase the run actually reached (no assist is taken on a billed
     fight's honest first attempt, nor for the scripted death itself). Expect
     SEVERAL windows per encounter: the die-retry stage is assisted into melee
     range and back out again, so bot fencing skill never decides whether that
     stage can run — only the death loop is under test there. **Read the
     `floor_gate` block every time**: it is the compiler's
     coverage ledger, and `not_covered` names each fight the delve bills
     `elite`/`boss` that the gate cannot measure, with the reason — an empty
     findings list over an uncovered elite is silence, not a pass. **`covered`,
     `not_covered` and `actors[]` all empty is the worst case, not the best**: it
     means no body in the campaign declares a tier, so the gate examined nothing
     and would have been green no matter what you shipped. The island's floor
     gate sat in exactly that state, green, for nineteen rounds. A campaign with
     hostile bodies and no tiered actor or wave has an **unbound** gate — report
     it as unbound, never as a pass, and fix it by tiering the fight
     (`docs/reference/playtest-methodology.md`, rule 1). The `actors[]`
     block beside it does the same for tier-declaring stage-5 actors: one row per
     actor, fought (with its outcome) or not (with why). An actor unleashed only
     by an ambient trigger is reported unexercised by design — if you want the
     ladder to measure that fight, unleash it from an objective on the critical
     path, and stage it where the party already stands (an actor anchored inside a
     later objective's zone completes that objective during the fight). A red
     `die-retry` stage is a CONTENT bug of the most serious kind — the delve is
     completable but dying is not safe. Never set `DELVEWRIGHT_DIE_RETRY=0` to
     get green; the report records a skipped stage as skipped, not as passed.
     Reading one trial: `respawn_pos` is where the bot actually came back and
     `at_checkpoint` is derived from it; `returned` is the walk back from exactly
     there. `re_engaged` and `outcome` are observed ONLY when `returned` — a trial
     that never got back reads `outcome: unproven`, which means the loop was never
     in a position to be judged, not that the fight vanished; fix the route from
     that checkpoint first. `kit_kept: false` means the kit did not survive the
     death, which is a broken world seal, not a difficulty knob.
   - **branch runs (spec-0025 §3) — required whenever the build emitted
     `validation/branch-plan.json`.** One critical-path run proves ONE storyline;
     a campaign that forks must have EVERY branch walked. Run
     `EULA=TRUE validation/branch-runs.sh --project <id>` (release tier: every
     enumerated branch, each in its own fresh world — party progress only moves
     forward, so a second branch needs a second world). It writes
     `validation/run-out/<id>/branch-runs.json`: per branch, ran/skipped-with-reason
     and the result. `DELVEWRIGHT_BRANCHES=<ids>` narrows the tier for local
     iteration; a narrowed run is NOT a validated campaign, and the report says
     which branches it skipped. `from-diff` is not available yet and refuses.
   - tear down containers, and report ONLY: per-command exit codes, failed
     PackTest names, the bot's failed step (if any), any die-retry finding, and
     ≤20 relevant log lines.
   Both must exit 0. Re-runs after fixes go through the same subagent. On any
   red, **triage before touching anything** (debug doctrine, CLAUDE.md):
   - *Content bug* (your DSL declares something wrong/unreachable/unlit): fix
     the campaign in the DSL. Repair judgment stays with the authoring agent.
   - *Toolchain bug* (compiler/harness/tileset misbehaves on a campaign the
     diagnostics accept): **stop content work and report it** with evidence —
     never hand-edit compiler output, never restructure the campaign to dodge
     the bug, never weaken a check or reroll a seed to get green. A workaround
     that turns a toolchain bug green is itself a quality defect: it ships the
     bug to every future campaign. Escalating is success.
9. Visual review (spec-0003 visual tier) — **you** (the authoring agent, not a
   subagent; visual judgment is the point). The build output already contains
   `render-plan.json` (deterministic shots + per-shot `expect` checklists derived
   from the DSL). Render the per-prefab sets with Nucleation and read them against
   each shot's `expect`:
   - `cargo run -q -p delvewright-render --bin delve-render -- batch campaigns/prefabs -o <workspace>/renders`
     (needs the 1.21.11 client jar via `--textures`/`$DELVEWRIGHT_CLIENT_JAR`;
     skip with a note if unavailable locally).
   - `delve-render fidelity-gate` must exit 0 before trusting any render.
   - Open the exterior/top/interior/anchor PNGs and check each against its
     `expect` line (marker visible? room not dark? NPC faces camera and its name
     is text not JSON? seam clean?). Findings are **DSL-level** — fix the campaign
     (lighting profile, anchor, NPC facing, name string) and rebuild; never
     hand-edit output.

   **Judge the player's eye first, and the set second** (owner concern, recorded
   during the nobodys-cave QA rounds). The per-prefab renders are orbit cameras:
   they answer *"is the set well made"*, which is not the question a playtest
   asks. The question is *"what does a player walking in experience"*, and only a
   first-person frame on the actual route answers it. The compiler already emits
   those shots — a `pov` camera at eye height on every corner-thinned
   critical-path waypoint, looking along the walk and, at each leg's end, toward
   the objective it arrives at, each with its own machine `expect` line. Every POV
   eye sits on a proven-standable waypoint, so the camera is provably in open air.

   So: read the **POV sequence in route order** before you open a single orbit
   render, and treat it as the primary evidence. A scene that photographs well
   from outside and reads as a corridor of grey stone from the doorway is a
   finding, not a pass. Whole-scene and player-POV shots come from
   `validation/render-shots.sh <build-dir>` (`delve-render scene` + `index`);
   path-tracing those scenes is Chunky, run as a separate process
   (`docs/reference/tools.md` §4a) — not wired into CI. The best of these frames
   are also what the design-alignment Artifact (step 4b) should have been built
   from.
10. **Storybook** (spec-0007): write `campaigns/campaigns/<id>/README.md` — the
   reader-facing intro. Background/setting ONLY: premise, lore, public NPC
   introductions (never persona `secret`), classes, playtime, build/play
   commands. NO puzzle solutions, quest structure, or endings. Images (relative
   links into `media/`, small JPEGs): exterior / starting-scene renders only,
   picked from the visual-review set — never interiors or late-game locations.
   Localized `README.<code>.md` per declared language. The render-set images are
   the default — the author may later replace them with hand-crafted shots
   (shaders, staged compositions); media ships with the campaign PR.

   **Storybook art is Chunky, in two passes** (owner decision, 2026-08-06).
   Draft with `delvec snapshot` — fast, disposable, for judging *layout*: is the
   right thing in frame, from the right side, at the right distance. Then produce
   the shipped image with Chunky: emit the scene set with
   `validation/render-shots.sh <build-dir>` and pick your scene, plus
   `delve-render panorama <build-dir> -o <dir>` for the whole-map hero shot every
   release owes (`--bearing` picks the corner). Render each scene as its own
   `java -jar ChunkyLauncher.jar … -render` process — parallel, `-target 64` for
   a look, ~300 for the shipped frame — then `-snapshot <scene> <out>.png`;
   commands, the pinned core and the cache/water/progress gotchas are in
   `docs/reference/tools.md` §4a. Never hand-edit a scene JSON: if the frame you
   want is not emittable, that is a `delve-render` gap to report, not a file to
   patch.

   **Every edition opens with the engine-version marker**, on its own line
   directly under the title (task #147). This is the ONE piece of internal
   machinery a storybook may carry — it is what a server host needs before
   running the delve — so it stays in this exact host-facing form and nothing
   else internal joins it:

   ```
   > **Requires delve engine <max per-stage dsl_version> or newer** — last verified with delvec <version>.
   ```

   The first number is the MAX `dsl_version` over the campaign's six stage
   documents; the second is `delvec --version`'s, from the build that just went
   green. The line is byte-identical in every localized edition — it is a
   version stamp, not prose; a translated gloss may follow on the next line.
   Then prove it:

   ```
   python3 tools/check-storybook-version.py --campaigns campaigns/campaigns
   ```

   Green before you report. A stale marker waves a host on an old engine
   straight into a delve their engine cannot run.
11. Report to the user: campaign summary, playtime estimate, validation results,
    and the two commands they care about:
    - play: `EULA=TRUE docker compose -f validation/compose.yaml -f
      validation/owner-play.yaml --profile play up` — `owner-play.yaml` is what
      publishes `localhost:25565`; the base compose file publishes nothing
    - playtest with notes: same with `--profile playtest` (+ `CREATOR_NAME=<mc name>`)

## Playtest rounds (iterating with the owner)

Generation is round 1. Everything after it is an iteration round against the
owner's findings, and the owner's playtest hour is the scarcest resource in the
pipeline. Full derivation from the 22-round island run:
`docs/reference/playtest-methodology.md`. Mandatory here:

1. **Keep a findings ledger in `GENERATION.md`** — one row per owner finding:
   number, her wording, the round it was reported, status. Status is `fixed@rN`,
   `open`, `engine` (blocked on a capability gap), or `ruled` (she closed it with
   no code change). This table is the campaign's memory; a finding that lives
   only in chat is a finding that will be reported to you twice.
2. **Triage every finding the day it arrives**, as *content* or *capability gap*.
   A capability gap — the DSL has no way to express what she asked for — is
   never patched downstream (CLAUDE.md forbids it) and is therefore a **staging
   blocker**: either the engine work lands before the next playtest, or the round
   summary tells her, per item, that it is still open and not to test it. Every
   island finding that survived more than one round was a capability gap, and
   staging builds while those rows were open is what made her see the same
   defect twice.
3. **Close each finding twice: the instance, and the general form.** After fixing
   the instance, ask what rule it is an instance *of*, and file that rule as a
   diagnostic (planner mints the DW code — never mint one yourself). When the
   diagnostic exists, **re-run it against the current build**: that sweep is the
   deliverable, not the code. `DW0489` found a second live instance the moment it
   landed — one the owner had already lost a click to. Where no diagnostic is
   possible, write that down; it becomes a risk item at the next staging review.
4. **Audit the FULL ledger from round 1 before staging any build** — never from
   the last round. Nothing she has reported may survive into a build you hand
   her.
5. **Pre-flight, in this order, before the invitation**: full ladder green
   (PackTest → bot critical path + die-retry → every branch run) → ledger audit →
   localized builds + double-build byte-identical → server boots and self-checks
   → then invite. Not "the build compiled, come look".
6. **Update `DESIGN.md` in the same round and run its conformance review.** The
   island's design record went eight rounds unupdated and the audit that caught
   up found seven changes no one had asked for.
7. **Close the round in `GENERATION.md` with its machine record**, not just
   prose: how many validation-loop iterations it took to reach green, and every
   DW code the round hit **with its count** (`DW0205 x3, DW0483 x3, DW0450 x1`).
   Write it even when the count is zero — a round that hit nothing is the
   datum that says the gates had nothing to say. This is the campaign's own
   record of where its difficulty lives, and it is the only source from which
   rounds-to-green can be read afterwards; a round summarised in prose alone
   is a round whose cost cannot be recovered.

## Hard rules

- Persist the DSL workspace before validation, not after — a crash must never
  lose the campaign.
- **Apply an owner ruling at the scope it was given.** If a wider rule seems
  right, propose it in one line and wait — generalizing a ruling is a design
  decision, not an inference to make silently. (A one-beat pacing ruling was
  read as a campaign-wide ceiling and had to be corrected the next day.)
- **Unrequested change is a rejection cause on its own**, independent of whether
  the change is good. Author what the round asked for; anything else you believe
  the campaign needs is a proposal in the round summary.
- **Open-air by default** (owner directive 2026-08-04): stage scenes in the
  open unless a beat NEEDS enclosure (a cave passage, an interior puzzle,
  a reveal). The horizon — surround terrain, sky, backdrop (spec-0026) — is
  part of the composition; a campaign of enclosed boxes wastes it. When an
  enclosed beat is necessary, prefer routing the player back into the open
  between beats over chaining interiors.
- Every player-visible string in the **stage docs stays English** — always. Other
  languages are delivered as `l10n/<code>.json` sidecars (the Localization stage
  above), never by writing non-English into the stage docs. Owner prompts in
  Chinese still yield English stage docs; add a `zh-cn` sidecar only when the user
  asks for localized in-game text (中文文本).
- Homages: original text only, cultural reference never asset ingestion
  (ADR-0007).
- If a mechanic the prompt wants has no DSL verb, do NOT fake it with adjacent
  verbs silently — tell the user what's missing and offer the closest authorable
  alternative (spec change requests go to the planning session, not this skill).

## Authoring pitfalls (learned on the nobodys-cave-island run, 2026-08-01)

- **`delvec validate` is whole-campaign**: it hard-errors unless all six stage
  files exist. When authoring stages incrementally, stub the later stages
  (clearly marked) so validate can run — and remember every stage-2 NPC needs a
  stage-6 tree (DW0152), and a declared language needs a covering sidecar
  (DW0180) even at the stub phase.
- **l10n `fx.` keys are POSITION-derived** (`fx.<quest>.oc.<obj>.<index>…`).
  Inserting an effect into a list SHIFTS every sibling's key and silently
  re-attaches old translations to the wrong lines. When editing effect lists on
  a localized campaign, APPEND rather than insert where order allows, and after
  any structural edit re-check every shifted key's translation against its new
  English source — exact-key coverage (DW0180/0181) cannot see a stale value.
- **Difficulty is declarable** (`world.difficulty`: `easy`/`normal`/`hard`).
  Absent, the compiler derives `easy` for a wave campaign — which HALVES the
  damage players take (`min(dmg/2+1, dmg)`), the setting behind "the enemies are
  too weak". A souls-style brief almost certainly wants `normal` or `hard`; when
  you change it, retune the combat arithmetic (mob `attributes`, class gear,
  wave sizes) rather than only flipping the keyword. `peaceful` is rejected
  (DW0468) — it deletes every hostile. Scripted `actors` take the same
  `attributes` block wave mobs do, so an elite can be tuned on both its staged
  puppet and its unleashed twin.
- **The machine proves the LOOP, not the win** (spec-0023). Three things are
  checked about every mandatory encounter, and it is worth authoring toward
  them rather than discovering them as red builds:
  1. *Winnability arithmetic* (build errors `DW0470`-`DW0473`): a required
     hostile must be damageable (Resistance amplifier 4 is total immunity —
     use at most 3, or put the durability in `attributes.max_health`), must
     have a standable cell beside it to be fought from, must fall inside the
     time-to-kill budget, and no `damage-players` in a quest bundle may land
     >= 20 (a full-health player) — that is a scripted death, not difficulty.
     A hit the party can dodge (a trap payload, a stealth `on_caught`, a
     `damage-players` with a `within` zone) is deliberately outside the check.
  2. **Declare `attributes.max_health` on every mandatory wave stack.** Vanilla
     publishes no per-entity default attributes, so an undeclared stack gets no
     numeric bound at all and the build warns `DW0475`. A souls campaign wants
     the arithmetic proven, so declare the health you tuned against.
  3. *The die-retry ladder stage*: the bot rests at every bonfire on the path
     (a fire only ARMS on arrival — the respawn point moves when the party
     RESTS), then deliberately dies twice at every encounter and proves respawn
     at the governing checkpoint -> the route back
     -> the encounter is still finishable -> no completed objective was lost.
     Author with that in mind: every encounter needs a checkpoint/bonfire that
     governs it, and a wave the party must be able to re-fight wants
     `respawns_on_rest`. Leaving it off is legitimate — a won fight stays won,
     and the stage records that as `cleared-before-retry` and passes it. What it
     reds is `stranded`: nothing left to fight AND the objective unfinished, so
     the party can neither complete the encounter nor re-fight it. Turning
     `respawns_on_rest` ON buys a stricter check: the wave must come back WHOLE
     — declared count, all-new mobs, full health — because a retry must never let
     the party grind a fight down one swing per death.
  4. *The inverted floor gate*: mark a set-piece fight `tier: "elite"` or
     `"boss"` — on the **wave** (DSL v0.7) or on the **actor** (DSL v0.8), same
     three keywords. The ladder then gives it one UNASSISTED bot attempt; if the
     bot — a poor fencer by design — wins cold, the run reports the fight as too
     easy for its billing. Leave ordinary pressure waves unmarked: they carry no
     such expectation. Marking is how you opt into the scrutiny, so mark
     honestly. **Mark the actor when the elite IS an actor** — the kneeling
     armoured thing that stands up when struck is a `spawn-actor` +
     `unleash-actor` beat, not a wave, and an unmarked one is a boss no proof
     ever looks at.
  5. *A tier the gate cannot measure is said out loud, not swallowed*: the gate
     warns on a first-try win and is silent otherwise, so an encounter nobody
     fought would look exactly like one that was fought and lost. The compiler
     therefore warns `DW0477` — and records `floor-gate: not covered (reason)`
     in `validation/combat-plan.json` — for a tiered actor no `unleash-actor`
     beat ever wakes (an `Invulnerable` puppet is scenery; a `vulnerable` one is
     `NoAI` and never swings back), and for a tiered wave no critical-path
     `kill` objective names. If you meant it as a fight, add the unleash or the
     `kill` objective; if you meant it as set dressing, drop the tier.
  Ordinary fights run the ladder under a bounded, logged combat assist, so bot
  fencing skill never caps how hard the delve is allowed to be — read
  `validation/run-out/<id>/run-report.json` after a `bot-run.sh` ladder for the
  assist windows, the death trials and any floor findings.
- **Bonfires owe the party a flask.** Right-clicking a `bonfire` opens exactly
  two options — *rest and save* (full restore: health, hunger, negative effects
  cleared, flask refilled, checkpoint moved, `respawns_on_rest` waves re-seated,
  `on_rest[]` fired) and *save only* (the checkpoint, nothing else). The
  replenished item is a class-kit entry marked `"flask": true`, and **every
  class kit in a campaign that places a bonfire must declare one** — a bonfire
  campaign with a flaskless kit is the build error `DW0476`. Author it as a real
  recovery consumable with the per-rest budget you tuned against as its `count`:
  resting sets the stack back to exactly that number, up or down, so the flask is
  a budget and never a stockpile.
  **A potion must say what is in it.** A `minecraft:potion` (or splash/lingering
  potion, or tipped arrow) with no `contents` is vanilla's *Uncraftable Potion* —
  it heals nothing however you name it — so at 0.8.0 declaring one is the build
  error `DW0487`. Either name a vanilla brew or list the effects:

  ```json
  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": { "potion": "minecraft:strong_healing" } }

  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": {
      "effects": [
        { "effect": "minecraft:instant_health", "amplifier": 1 },
        { "effect": "minecraft:regeneration", "duration": 200, "amplifier": 0 }
      ],
      "color": "#ff9c30"
    } }
  ```

  `potion` is a 1.21.11 potion id, where strength and duration are part of the id
  (`minecraft:strong_healing`, `minecraft:long_night_vision`) rather than separate
  fields. `duration` is in **ticks** (20 = one second) and is required for every
  lasting effect — and forbidden on the instantaneous ones
  (`instant_health`/`instant_damage`), which land once on drinking. `amplifier` is
  0 = level I. Anything vanilla cannot pour is `DW0486`. The
  bonfire's three dialog strings default to canonical English; author
  `prompt`/`rest_label`/`save_label` only when the fiction wants its own words,
  and keep the two labels button captions (`DW0331`: ~20 Latin / ~12 Han).
  Both the flask and the labels need `dsl_version 0.8.0` on their stage.
- **Place a bonfire OUT of every hostile's reach.** A rest point is where the
  party respawns and where every `respawns_on_rest` wave is put back on its
  feet, so a fire inside a hostile's `follow_range` delivers the party straight
  into combat on arrival — the build error `DW0478`. The clearance is measured
  against where the force actually IS: a wave's seated spawn cells, and for a
  `lane` wave the whole marched polyline (a lane wave walks its corridor while
  you are elsewhere, so a fire beside the far end of a lane is a fire in the
  lane). Fighting actors — anything `unleash-actor`ed, or staged `vulnerable` —
  count too, at their staging anchor. Put fires in side rooms, past the
  threshold, or beyond the end of the lane; never buy the clearance by
  shrinking `follow_range`, which retunes the fight to hide the placement.
  A re-seated wave always comes back **stationed** — a lane wave at its lane
  start, a plain wave at its anchor — so the safe zone stays true across every
  rest and every death.

- **Wave tuning**: `follow_range` below ~16 means distant wave mobs never
  engage; a kill objective whose mobs idle is unfinishable-in-practice even
  though machine-valid. Undead waves burn in daylight — the ONLY sanctioned fix
  (owner ruling) is a helmet on the mob: `equipment.head`, any head item, on
  every burning stack the party is asked to fight. **Never `set-time`**; the
  delve's hour is a pacing decision, and moving it to save a mob spends a beat.
  The compiler enforces this now (`DW0496`): a species in vanilla's
  `#minecraft:burn_in_daylight` staged for a `kill`-adjudicated fight whose
  walkable ground reaches open sky, under a pinned clear daytime hour, with an
  empty head slot, is a build error naming the sunlit cell. Roofing the arena
  clears it too. One species the helmet does not save — a phantom burns through
  it — so an open-air phantom fight has to be roofed or restaged.
  Never route wave mobs like actors: waves
  are native AI; if a future beat needs lane-then-fight movement, that is the
  routed-then-feral primitive (M4, task #66), not a follow_range trick.
- **Player-POV review is live**: the build's `render-plan.json` POV shots
  render through Chunky (see `validation/render-shots.sh`, delve-render scene
  emission, camera mapping fixed in #111). Review the corrected set against
  each shot's `expect` before handing the delve over. Declared-dark interiors
  render faithfully dark — review those in-game (night-vision mitigation);
  do not brighten scenes to pass review.
- **Anchor ambiguity**: if the jigsaw can place a pool piece twice, its anchors
  are DW0305-ambiguous — don't hang objectives on connector-piece anchors
  unless the pool guarantees uniqueness; use them as hint landmarks instead.
  You no longer have to guess which those are: the build now says so at the
  pool declaration. `DW0498` (advisory) names, per pool area, every prefab the
  draw seated twice and every anchor that repeat makes ambiguous. Read it
  before placing anything — an anchor it lists resolves silently to the first
  copy for actors and edits, and is a hard `DW0305` the moment an objective,
  NPC stand, gate or wave spawn references it. The fix is a wider pool
  (distinct variant members in the repeated role) or placements moved off
  those names — never a reseed.
