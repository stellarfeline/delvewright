---
name: new-delve
description: Generate a complete playable Minecraft delve from a creative prompt — staged DSL authoring with validation-loop self-repair, deterministic compile, machine validation, joinable output. Use when the user asks to create/generate a new delve or campaign. Args = the creative prompt (theme one-liner or detailed brief).
version: 1.3.0
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
**`delvewright-campaigns` git repo**, reached through the `campaigns/` symlink
at the engine repo root (real path `../delvewright-campaigns/`). The symlink is
the only way there — there is no environment-variable override. Create
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

### D. HARD RULE — a name spelled the same way IS the same name

Owner ruling, 2026-08-06, after playing a delve in Chinese. Every name you write
over a body is translated, and **whether two bodies share one translation is
decided by whether you spelled them identically** — not by whether you meant the
same character. Apply this while you are naming, because it is unrecoverable
later: by the time a translator sees the list, your intent is gone and only the
spelling is left.

**Bodies that are one character: spell the name byte-identically.** A character
usually occupies more than one declaration — an NPC that stands and talks, plus
one actor puppet per cutscene pose it is staged in. Written identically, all of
them are one name: the translator is asked once and every body renders the same
way, in every language.

```
GOOD  npc/polyphemus            "Polyphemus"
      actor/polyphemus-walker   "Polyphemus"      ← same character, same spelling
      actor/polyphemus-roused   "Polyphemus"
      actor/polyphemus-blinded  "Polyphemus"

BAD   npc/polyphemus            "Polyphemus"
      actor/polyphemus-roused   "Polyphemus "     ← a trailing space is a second
      actor/polyphemus-blinded  "polyphemus"        character, and the giant is
                                                    renamed mid-cutscene
```

Differ by a space, a case, or a `the` and the player meets two characters — one
of whom may be called something else entirely in Chinese. Copy the NPC's name;
do not retype it.

**Bodies that are genuinely different: spell them differently.** The rule runs
both ways. Two unrelated NPCs you both called `Guard` are one name and will be
translated once, so if they must read as two people, write two names.

**Wave mobs are the exception, and it is the one to plan around.** A wave mob's
name is *not* pooled with anything: three waves whose mobs you both named
`Drowned of Poseidon` are three separate names, asked of the translator three
times, and free to come back as three different Chinese strings — the same squad
under three names, in one delve. So:

- If several waves really are **one creature**, still write the identical string
  — it is the honest source, and the localization stage carries a glossary that
  holds proper nouns steady across batches. Then **say so in the campaign's
  posture note**, so the localization stage knows those rows must agree.
- If they are **not** one creature, give them names that differ. Do not reuse a
  name for flavour across waves that the fiction treats as distinct — you get the
  cost of a shared name with none of the benefit.

Fewer distinct names is the cheaper delve in every language. A name you reuse
deliberately is free; a name you reuse accidentally is a defect the English build
can never show you.

### E. Plain-prose baseline (Strunk 1918, public domain)

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
   - **A number the world remembers is `state`, not a flag.** A flag is boolean,
     party-wide and one-way — nothing clears one — so it says "this happened" and
     nothing else. When a beat needs a *quantity* that goes down as well as up (a
     toll still owed, a floor a lift is at, whether a ride is in progress),
     declare it in the stage-5 `state` list: `{"id": "state/<kebab>", "scope":
     "party" | "player", "initial": <n>, "note": "<what the number means>"}`.
     `scope` is required and never guessed — `party` is one shared value, `player`
     gives each member their own. Write it with `set-state` / `add-state` (signed:
     a negative `amount` counts down) / `clear-state` (back to `initial`).
     - **Read it in the gate, never in a verb.** `requires_state: [{"state": …,
       "op": "equals"|"not-equals"|"at-least"|"at-most", "value": <n>}]` is
       accepted everywhere `requires_flags` is — an objective, any gatable effect,
       a trigger, a trap, a dialogue option, a cast placement — so "the door opens
       at zero" and "this line is withheld below two" are the same construct.
     - Every datum must be both **written somewhere and read somewhere**: a gate
       reading a datum nothing writes is `DW0501`, and a datum no gate reads is
       `DW0502`. Both mean the mechanism is decoration.
     - A `player`-scoped datum can only be touched where a player is acting — a
       dialogue option, a cast placement, an `on_death` beat, or an effect on a
       quest beat a player completes. These have **no** acting player and reject
       one (`DW0503`): an objective/trigger/trap *gate*, a trigger's `effects`, a
       trap's `payload`, a shortcut's `on_unlock`, a `sequence` step and a
       `move-npc`/`move-actor` `on_arrive`. Use `party` scope there.
     - Needs `dsl_version` 0.10.0 on the stage that declares or reads it.
   - **What happens when a player dies is content, not engine behaviour.** The
     quests stage takes a campaign-wide `on_death`: a bundle of ordinary effects
     that runs at the moment a player dies, for that player. One per campaign —
     it is not a field on a checkpoint, because dying is true everywhere in the
     delve; put `requires_flags`/`forbids_flags` on the effects inside it if the
     beat should only land in some phase of the story. Do NOT write a death beat
     the mainline depends on: nothing inside it is credited as a flag producer,
     deliberately, so a door it alone opens is a door only a corpse can open.
     Needs `dsl_version` 0.10.0 on the quests stage.
   - **A region can be filled or cleared while the delve runs, and no gate need
     be involved.** `fill-region {region{anchor,extent}, block}` writes a block
     over a declared box; `clear-region {region{anchor,extent}}` empties it.
     `open-gate`/`close-gate` are the same operation with the box and the block
     read off a prefab gate anchor — reach for those when a prefab already
     declares the threshold, and for these when the box is yours: a bridge that
     materialises, a floor that sinks, a wall that opens, a platform summoned
     under the party. Both need `dsl_version` 0.10.0 on the quests stage. The
     completability proof honours them from the point in the quest DAG where the
     effect fires: a fill the only route must cross afterwards fails the build
     (`DW0311`), and a clear is credited as passable, so a route may legitimately
     depend on one. Two things it will not model, so do not build on them: a clear
     that opens a box into water (the water flows back in and the proof does not
     know), and a clear over rubble another mechanism dropped there (a `collapse`
     debris field, a shut timed gate) — those stay solid.
   - **A place that kills is DECLARED, never faked with the art.** A cliff whose
     fall must be fatal, a lava pit, an acid pool, an out-of-bounds plane: all one
     declaration, `lethal_volumes[] {id, region{anchor,extent}, message,
     damage_type?}` on the quests stage (`dsl_version` 0.10.0). Never obtain the
     behaviour by changing the world instead — making the horizon `void` so the
     fall kills is the move this exists to replace, and it serves exactly one
     fiction. `message` is REQUIRED and is what the player reads as they die
     (blank is `DW0512`); `damage_type` words vanilla's own broadcast (`fall`,
     `on_fire`, …). The volume is geometry the completability proof honours: if the
     party's only route to an objective crosses it the build fails naming the
     volume (`DW0510`), and nothing the campaign POSTS — the entry spawn, a
     checkpoint, a bonfire, an NPC's anchor, a `cast` placement, an actor — may sit
     inside one (`DW0511`). Put the volume where a player can SEE what will happen
     before they commit to it; a killing box nobody can read is 初见杀 with no
     lesson in it.
   - **A status effect is a verb now — and it ends by expiring, never by being
     cleared.** `give-effect {effect, seconds, amplifier?, hide_particles?, in?}`
     grants any pinned-1.21.11 status effect; `in {anchor, extent}` narrows it to
     the players inside a box, so "blind whoever is riding" does not blind the
     delve. `seconds` is REQUIRED and there is no infinite form, on purpose: an
     effect whose only removal is a later step is one the player keeps forever
     whenever that step does not run — a logout, a crash, a death mid-chain. So
     **do not write "grant, then clear at the end"**; write a duration that covers
     the beat plus slack and let it expire. Pairing a live grant with a
     `clear-effect` of the same effect in the same bundle is `DW0540`.
     `clear-effect {effect?, in?}` exists for effects the campaign did NOT grant
     (a potion the player drank, a `wither` a mob applied); omit `effect` to clear
     everything. Needs `dsl_version` 0.10.0 on the quests stage.
   - **A teleport selects a REGION, never a block.** `teleport {from {anchor,
     extent}, to}` moves **everything** inside the box to the destination anchor —
     players and entities alike, which is what makes a cargo platform the same
     mechanism as a passenger one. Nothing is exempt, so do not draw the volume
     over an affordance the engine anchors to a block (an interact objective, a
     click trigger, a bonfire, a shortcut lever, a disarm, a sealed gate): the
     hitbox would ride and the hardware would stay, and the build refuses it
     (`DW0542`). Two things to design AROUND rather than against, both measured on
     the pinned server: **a teleport is not a rescue** — accumulated fall distance
     carries across it unchanged and is charged in full at the destination, so a
     platform arriving under a falling player past ~20 blocks is the surface they
     die on; and **nav does not know about it** — a route that exists only through
     a teleport still fails the completability proof, so keep a walked route to
     anything the critical path needs. Needs `dsl_version` 0.10.0 on the quests
     stage.
   - **A currency is a NAMED datum, and a price is a GATE.** There is no
     `currencies` section and no `price` field, on purpose. Give a `state[]` datum
     a `name` and it becomes a purse the player reads: the engine states
     `<name>: <value>` on that player's action bar on every write, translated like
     any other line. A shop is `shops[] {id, anchor, title, marker_item?,
     offers[{label, tooltip?, effects[], + the ordinary gate}]}` on the quests stage
     (`dsl_version` 0.10.0), and its prices are `requires_state` comparisons —
     exactly the ones a door or a dialogue line would use. **Write the refusal
     yourself**: put the purchase behind `at-least <price>` and an apology
     `narrate` behind `at-most <price − 1>`, both as gated effects of the same
     offer, so a player who cannot afford something is told rather than left
     pressing a dead button. An offer with no effects at all is `DW0523`.
     **Order matters and the compiler will tell you (`DW0527`):** put the refusal
     and any confirmation BEFORE the debit. Sibling effects are consecutive
     commands, so a gate written after the debit reads the balance the debit just
     produced — buy your last coin and you are charged and apologised to in the
     same breath.
   - **A death that costs something leaves a stake, and the engine decides where.**
     `stakes[] {id, state, forfeit?, max_live?, on_full?, collect_by?,
     collected_message, marker_item?}` on the quests stage, dropped by a
     `drop-stake` effect in `on_death`. The datum must be `player`-scoped
     (`DW0520`) — a stake is one player's wager, never the party's. You do **not**
     choose where it lands: the compiler computes the point, on the walkable way
     back from the respawn point in force, nearest to where they died, so a death
     in a lethal volume leaves its stake at the near lip rather than inside the
     hazard, and a death on a lift car leaves it on solid ground. If your geometry
     can strand one — a one-way drop with no shortcut back — the build fails
     naming the place (`DW0525`), and the fix is a route back or a
     `lethal_volume`, never deleting the stake. Souls behaviour is
     `max_live: 1, on_full: "replace"`; no death cost at all is `max_live: 0`; a
     memorial at every death site is a larger `max_live` with `on_full: "keep"`.
   - **A body that moves unlike its species DECLARES it, and the build holds it to
     the claim.** `traversal { locomotion: ground|climber|flier }` on a stage-2 NPC
     or a stage-5 actor (`dsl_version` 0.11.0 on that body's own stage). By default
     the compiler reads locomotion off the entity id — spiders climb, ghasts fly,
     everything else walks and is checked — so a walked leg that goes OVER a wall
     line instead of round to its opening is `DW0453`. If that is your fiction (a
     sheep that climbs), declare it and the advisory is answered. It is **not** an
     off switch: a declaration that changes no verdict is refused (`DW0454`), so
     you may only claim a climber where the route really climbs; `aquatic` is
     refused outright (`DW0455`) because nothing in the model could hold a body to
     it; and no declaration touches the error tier — a declared climber still
     cannot walk through a closed fence gate (`DW0452`). Declare it on the body,
     never on the beat.
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
3. `delvec validate <campaign-dir>` — fix by diagnostic code (DW####; the
   complete catalogue is `docs/reference/compiler.md` §5). Loop until clean.
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
actually reviews in. She does not read long documents (the review protocol); a
design she cannot see is a design she cannot approve, and every problem it would
have caught gets paid for twice once stages 5–6 are written against it.

- **Which images these are** (owner correction, 2026-08-07): at THIS gate they are
  **reference images** — concept art drawn from the scene description *before any
  prefab exists*, so she is confirming the design, not a build. A **render** is a
  candidate prefab imaged by `delve-render`, and belongs to contact-sheet curation
  later. Two stages, two producers; building prefabs first and rendering them
  inverts the gate. `tools/refimg.py` draws reference images when a provider is
  configured (`[refimg]` in `delvewright.local.toml`) — advisory, and it needs a
  human in the loop for prompt iteration.
  When candidate prefabs DO exist and the owner is choosing between them, that
  later step has its own tool: `delve-render contact-sheet <renders> -o <png>`
  puts them all on one page, optionally ordered by similarity to this gate's
  reference image (`tools/refscore.py`) — advisory, human-in-the-loop, and the
  score only ORDERS the page, it never removes a candidate from it.
  A still image cannot answer where the way in is or how a room reads from
  standing height; when that is the question,
  `delve-render viewer <nbt|dir|manifest.json> -o <page.html>` gives her one
  self-contained page she drives — orbit, plan, a player point of view at every
  anchor, and a cutaway for roofed interiors. Every block is drawn from the
  pinned version's own model and textures, so a wall is a wall and a stair is a
  stair. Advisory, human-in-the-loop.

- **Near view** = the scene as a player stands in it. **Far view** = the same
  scene in its surroundings, so staging and sightlines read.
- Prefer the **player-POV** shots (below) for near view. An orbit render answers
  "is the set pretty"; only an eye-height frame on the walk answers "what does a
  player walking in experience", and the second question is the one the review
  exists for.
- **The moment she confirms, the approved images become campaign files.** Copy
  them to `campaigns/<id>/design/concept/`, one per scene, named for the scene,
  and write `campaigns/<id>/design/README.md` carrying the approval date, the
  approved names, and the sentence that every later round is held to: *author
  from the image, judge against it, present every choice beside it.* Commit them
  with the campaign. `tools/refimg.py` writes to a gitignored working directory,
  which is right for a draft and wrong for an approved one — **an approval that
  lives only in a published page is bound to nothing.**
- **Every later step that asks the owner to choose reads `design/` FIRST**, and
  presents the choice beside that scene's image, under the approved name, saying
  which element of the image the thing on offer corresponds to. A round that
  cannot say that is not ready to ask. This binds hardest on contact-sheet
  curation, which is the step most likely to run in a later session that never
  saw the gate.
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
- **The layout needs a prefab the library doesn't have**: follow
  `docs/reference/prefab-procedure.md` — it is the procedure, and these are its
  mandatory steps, in order. Do not improvise around them.
  1. **Write the scene description first** (one or two sentences: what a body
     does in the space, the material feeling, what the campaign will attach).
     Written after the render, it is a description of the render.
  2. **Choose the palette by measurement, never from memory** — and it is
     three steps, not one. A block's name is not its appearance (`packed_mud`
     is orange, 142/107/80).
     **Screen** the shelf by constraints rather than by a guessed hex:
     `python3 tools/block-appearance.py --screen --where full_cube --where
     'L>=0.75' --where 'C_mean<0.02' --where 'texture_range<=0.30'` takes 1146
     blocks to a handful (`L` = Oklab lightness, `C_mean` = how coloured,
     `texture_range` = how loud the pattern; `form=`, `family=`, `not tinted`,
     `not gravity` are facets too). Then **measure the mix**:
     `--mix 'a=3,b=3,c=4'` or `--program p.json` reports `chroma_mass`,
     `chromatic_area`, the **named** `loudest_member` with its area share, and
     `dominant_hue` — never a mean as the verdict, because a mean cannot see
     that 60% of a wall is one loud family when the craft rule gives it 10%.
     Then **LOOK**: `--sheet` writes `.sheets/palette/swatches.png`, every
     survivor tiled and every mix rendered as its seeded weighted tiling —
     **read that PNG before binding anything.** A shortlist is not a choice,
     and the screen will hand you blocks that are right on every measured axis
     and wrong for the job (a light source, a gravity block, wool). Record the
     measured hex beside each role.
     The tool needs the pinned block registry from `crates/compiler/data/`
     **and** a 1.21.11 client jar, and refuses by name when either is
     absent. That does not make the step optional: take role names from the
     corpus instead (`delve-grammar list`, then `delve-grammar show
     --program <nearest>`), which is a palette somebody already measured,
     and record where each name came from. Never invent one — a block that
     does not exist is refused at export, and one that exists but looks
     nothing like its name is caught only by eye at step 5.
  3. **Author a grammar program.** Read the **idiom index** first
     (`docs/reference/grammar.md` §2c): ten techniques with a runnable program
     each — repetition, `otherwise`, taper/arch/gable (one recursion),
     air-in-a-mix erosion, graded erosion, surface detail, symmetry without
     reflection, `skip`, light, and arguments (`bind` — one rule called with
     different content). It is the part of the language no type signature
     shows, and a scene that looks impossible is usually one of the ten.
     **Never copy a rule to change its paint, its size or its axis**: a caller
     passes a paint or a size with `bind`, an axis with `reorient`, and anything
     derivable from the box with an expression over `dim` — a copied rule family
     is one nothing keeps in step and no gate reads.
     `delve-grammar show --program idiom-shape` prints one. Then start from the
     corpus: `delve-grammar list`, `delve-grammar show --program <nearest> >
     p.json`, edit, and `delve-grammar check --file p.json` after every edit.
     You write JSON — never Rust, and never blocks by hand. Four traps the
     procedure names: two guards that can both hold are a **probability, not a
     priority** (the "none of the above" arm is `otherwise`, and it is also what
     stops a recursion); **`rounding` is owed by every surface, not only
     floors** — the default truncates and an unwritten cell is air, which no
     gate reads; a palette role may be a **weighted list with `minecraft:air` in
     it**, which is the whole of decay and the cure for a piece that renders as
     one flat material; and a `facing=` block state **does not turn when the frame
     turns and does not flip when it reflects** — `oriented-fills` (`DW0736`)
     refuses the piece rather than shipping it facing the wrong way. Say which
     axes the state is written in: wrap it as
     `{"local": "minecraft:iron_bars[east=true,…]"}` and its directions mean the
     scope's own, so one palette role gives the right state at every frame,
     reflections included. Where the whole rule BODY differs by frame, use an
     `orientation` guard instead — one alternative per frame, naming the
     reflection as well as the axes.
     **Decide the split order before the first rule** (`grammar.md` §2c, the
     section before the ten). A split's children copy the parent box on the two
     axes it does not cut, so siblings of a split are the only two things
     guaranteed to line up, and there is no way to say "this opening is the same
     cells as that one". Hence: **the last axis you split is the only axis on
     which two things are guaranteed to meet — split last on the axis your
     openings run through**, and write a hole as a piece of that split whose
     siblings are the two things that must meet (best as the *absence* of a
     sibling, which cannot be misaligned). Within one axis, pin a course to a
     band's end and not to a height: `[relative 1, absolute 1]` is *the last
     course of this band* at any band height, where `[absolute 5, absolute 1]`
     is a computed height that also refuses a short band. Every constant you do
     not eliminate this way fails silently.
     One more refusal to expect: **`repeat` clamps the last tile but does not
     rescue a box too short for the first one** — one pass of the pattern is
     resolved before any tiling, so a repeat whose absolutes sum to 8 across a
     7-deep box is a hard refusal. Guard the extent and give the short box an
     `otherwise` arm.
  4. **Expand and let the machine judge**:
     `delve-grammar expand --file p.json --region XxYxZ --seed N --traversable
     --reachable-floor -o out/`. Pass `--traversable` for any passage, stair or
     route; pass `--reachable-floor` for any piece with an inside a body is meant
     to walk around. A red gate writes no `.nbt` (exit 4). **Read the `findings`
     in the report** — a gate that bound to zero objects, or a program that
     declared no anchors, is a finding, not a pass.
     Three of the always-on gates are about how a block state is SPELLED —
     `shape-complete` (`DW0735`), `states-complete` (`DW0737`) and
     `oriented-fills` (`DW0736`). Write every property of every block state you
     paint, including the ones whose default looks obvious: a state that omits
     one means whatever a running server decides, and the render you are about
     to check the piece against cannot know which. Where a property names a
     direction — a bar's connections, a stair's facing, a skull's yaw — write
     the state in the scope's own frame (`{"local": …}`) rather than guessing
     which way the zone will hand your piece its box.
     **Read the `reachability` line too**, which prints whether you asked or not:
     `traversable` joins two ground-level faces and says nothing about the
     storeys above, so a building can pass every gate with half its floor
     stranded. Unreachable floor **under a roof** is a room with no way in, and
     the report gives you the box to go and look at. Unreachable floor open to
     the sky is a roof, and is nobody's defect.
     If the piece is one of a campaign's **zones**, its program belongs to the
     campaign: put it in `campaigns/<campaign>/design/programs/` and name it in
     `zones.json` there with the region, seed and gate claims it is built at
     (`traversable`, `allow_falls`, `reachable_floor`, `symmetric`).
     `delve-grammar audit --campaign-root <content repo>` judges every zone a
     campaign declares, and CI in both repos runs it — a program that directory
     carries and the manifest does not name is a red.

     **One design the gate cannot be told about: a one-way descent.** A level a
     body drops into and does not climb back out of is unreachable on foot on
     purpose, and nothing in the CLI, the report or the metadata can state that
     claim. So do **not** pass `--reachable-floor` on such a piece — it fails
     (`drop-shaft` 9×12×9 seed 1: 28 of 63 roofed cells unreached) and a red gate
     writes no `.nbt`, so the flag ships nothing rather than shipping a known
     red. Expand without it, read the always-on reachability line, and record in
     the campaign's `GENERATION.md` that the `unreachable_sheltered` pocket it
     names is the drop and not a room with no way in. That verdict is bounded by
     the instrument, and this is the step at which to say so.
  5. **Look at it**: `delve-render piece out/<id>.nbt -o shots/`, and compare
     against step 1. The gates prove it is buildable and walkable; they say
     nothing about whether it is the scene you asked for. If the expand wrote a
     tile set instead of one `.nbt`, pass the manifest — `delve-render piece
     out/<id>.json` — which renders the assembled zone as one scene, eye shots
     included. Never review a single tile; the command refuses one anyway.
     **Open the `eye-<anchor>.png` frames FIRST.** They are the only cameras
     inside the piece — a body's eye at 1.62, at each declared anchor, looking
     the way that anchor faces. The orbit shots (`ext-*`, `top`, `door-*`,
     `anchor-*`) are fitted from outside, and on a roofed piece they are all the
     same picture of the same rock. Read `<id>-shots.json` beside the images for
     which cell each body is standing in: a camera whose anchor cell held a gate
     or a barrel steps back along the facing and says so (`DW0727`), and an
     anchor with no body cell gets no eye shot at all — the run states that count.
     A flat grey frame is outside the piece, and a shot that is *only* that
     is reported as an empty frame: the camera is aimed at nothing.
     **When the piece is a building whose identity is one elevation** — a west
     front, a gatehouse, an approach face — add the camera for it: `--view
     name=west-front,face=north` (repeatable) appends a level, square-on shot of
     that face of the model, and no planned camera is square-on at a face. `of=`
     aims it at a declared anchor instead of the whole model; `zoom=` tightens or
     backs off. Do not build a forecourt and stand an anchor on it: a 70° eye
     camera reaches only ≈0.7 × its distance above eye height, so it looks
     through the doorway instead of at the façade, and the forecourt shrinks the
     building in every exterior frame. Keys: `docs/reference/tools.md` §4.
  6. **Admit it**: the whole `delve-admit` chain (`audit` → `socket` →
     `anchor` → `lighting --write` → `catalog validate`), then `audit` again.
     For a tile set, `audit` and `lighting` both take the **manifest** and
     answer about one zone; handing any command a single tile is `DW0739`, and
     so is handing it a tile copied away from its manifest.
     A grammar prefab has **no connectors and no lighting** until this step, so
     it cannot enter a `prefab_pool` and will be dark, until you do it.
     `lighting` measures the roofed floor a body can walk to from outside and
     reports the count it bound to. Two refusals to expect and not work around:
     `DW0752` means the probe bound to **zero** cells — usually a piece whose
     only way in is a socket that has not been carved yet, so run `socket`
     first; `DW0753` means there is no metadata to write into, and the fix is to
     create it, never to let the tool invent a `spdx: UNKNOWN` one.

  What the grammar cannot express — **escalate, do not work around**: block
  entities of any kind (chest loot, sign text, spawners — bind those in the
  campaign against an anchor the piece declares), **smooth** curves, diagonals,
  a profile step that varies independently of the box, a vault bending on two
  axes at once, and terrain. **Neither a stepped arch nor a symmetric shape is
  on this list** — the first is idiom 3 (one recursion whose step is arithmetic
  on the remaining dimension, and the same program inverted is the opening), the
  second is idiom 7 (a rule body written mirrored, since `reorient` permutes and
  never mirrors). Check §2c before escalating. **Size is not on this list**
  either: a region of any extent expands, and one past the 48-per-axis
  structure-template cap is written as a tile set plus a manifest at
  `<id>.json`. Never shrink a scene to fit a file format.

  A piece that comes from **outside** (a community schematic) instead enters via
  `delve-schem convert` and then the same admission chain with
  `resolve-jigsaw` before `socket`. Never place an un-audited piece: `audit` is
  the ADR-0013 licence/code-injection gate and the `DW0733` check that the blocks
  in it exist at all. Flags in `docs/reference/tools.md` §2a and §3.
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
- **Several candidate prefabs for one slot, and she has to pick**: mention
  `delve-render contact-sheet <renders> -o <png>` — all the candidates on one
  page, each labelled with its rank and id, with `tools/refscore.py` optionally
  ordering the page by similarity to the design-gate reference image. One line,
  human-optional. Say plainly that the score only orders the page: every
  candidate is on it, and the low scorer is present, last — she is the selector,
  the number is not.
- **She cannot tell from a picture what a prefab is like to be inside**: mention
  `delve-render viewer <nbt|dir|manifest.json> -o <page.html>` — one
  self-contained HTML page with a camera she drives: exterior, plan, and a player
  point of view at eye height (1.62) standing at every declared anchor and
  doorway, plus a cutaway slider that takes the roof off. Blocks are drawn from
  the pinned client jar's own models and textures, so a wall reads as a wall. A
  zone that ships as several tiles and a manifest shows as one building. Pass a
  directory to put a whole library on one page. One line, human-optional.

  **Read its fidelity list before showing her the page.** It names every
  blockstate the page cannot draw as the game draws it: a block the pinned
  version does not have (`DW0790`), and — the one that reads as fine and is not
  — a palette entry that leaves shape-carrying properties unwritten (`DW0791`),
  where the shape comes from the version's default state rather than from the
  file. That is a defect in the prefab, not in the page: fix it by writing the
  property at the value the message names, then rebuild. Showing her a page whose
  walls are the wrong shape spends her hour on the tool instead of on the
  building.

### Localization stage (only when the prompt asks for other languages)

If the prompt requests one or more languages — or the user prompts in a
non-English language **and asks for localized in-game text** (中文文本 etc.) — add a
**final generation stage after `dialogue`**, once the English campaign is complete:

1. Declare the codes in `world.json`: `"languages": ["zh-cn", …]` (BCP-47-style;
   `en` is implicit/canonical and is **never** listed). Stage docs stay English.
   Each code must be one the compiler can map to a Minecraft lang-file name
   (`zh-cn` → `zh_cn`); an unmapped code is `DW0184` at validate time, never a
   language quietly missing from the shipped pack.
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
4. Re-`validate` until zero `DW0180`/`DW0181`. **The default build ships every
   declared language and the client picks its own** (i18n v2): `delvec build`
   emits each authored string as `{"translate": key, "fallback": English}` and
   writes `assets/delvewright/lang/<mc_code>.json` per language into the delve's
   resource pack. A player whose locale you do not ship — or who declines the
   resource-pack prompt — reads the English fallback. Nothing extra to run.
   `delvec build --lang <code>` still produces the single-language bake for local
   dev; the release path does not use it. `critical-path.json` is language-neutral
   either way, so the ladder is unchanged.

Then:

**`delvec fmt <campaign-dir>` — MANDATORY, before `analyze` and again after every
later DSL fix, including every playtest-round repair.** It rewrites every stage
document and l10n sidecar in canonical form: object keys sorted, two-space
indent, non-ASCII raw, one trailing newline. It exists because a three-key
insertion into a non-canonical `zh-cn.json` once produced a 103-insertion /
100-deletion diff, which is unreviewable and conflicts with every other edit in
flight. **Array order is semantic and it never touches it** (`quests[]`,
`objectives[]`, `effects[]` are ordered), and it proves that on every file it
writes — so running it is never a risk to the campaign.

```
cargo run -q -p delvec --bin delvec -- fmt campaigns/campaigns/<id>
```

Exit 1 means something is wrong with the JSON itself, not with its layout:
`DW0770` unparseable (it prints `line:col`), `DW0771` a duplicate object key —
which means one of the two values is already being silently discarded, so fix the
document rather than the formatter. Never hand-sort a file, and never "fix" a
`DW0773` by editing: re-run `fmt`. Full canonical form:
`docs/reference/compiler.md` §9.

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
   - `cargo run -q --manifest-path crates/render/Cargo.toml --bin delve-render -- batch campaigns/prefabs -o <workspace>/renders`
     (`--manifest-path`, not `-p`: the render crate is its own cargo workspace)
     (needs the 1.21.11 client jar via `--textures`/`$DELVEWRIGHT_CLIENT_JAR`;
     skip with a note if unavailable locally).
   - `delve-render fidelity-gate` must exit 0 before trusting any render.
   - Open the exterior/top/interior/anchor PNGs and check each against its
     `expect` line (marker visible? room not dark? NPC faces camera and its name
     is text not JSON? seam clean?). Findings are **DSL-level** — fix the campaign
     (lighting profile, anchor, NPC facing, name string) and rebuild; never
     hand-edit output.

   **Judge the player's eye first, and the set second** (owner concern, recorded
   during the nobodys-cave QA rounds). A per-prefab eye shot is a body inside one
   piece; it cannot see the route, the seams between pieces, or the world's real
   light. The question a playtest asks is *"what does a player walking in
   experience"*, and only a first-person frame on the actual assembled route
   answers it. The compiler emits those shots — a `pov` camera at eye height on
   every corner-thinned
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
   version stamp, not prose; a translated gloss may follow on the next line, but
   it may not restate the numbers (a mistranslated version number is a wrong
   version number — the island's zh-cn gloss drifted a whole minor behind the
   stamp directly above it).

   **Write no other version number anywhere in the storybook.** The marker is
   the only one a check can keep true; every other is hand-typed and goes stale
   in silence. The v1.1.0 island release had a correct marker and still told a
   host to run `:v1.0.0`, because the `docker run` line and a `**vX.Y.Z**`
   campaign stamp were bound to nothing. So: no campaign-version stamp, and the
   host command names `:latest` — that IS this storybook's claim — with one
   sentence sending a reader who wants an exact version to the release page,
   where the tag is machine-written. Then prove it:

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
4. **Append every finding to the engine repo's `docs/playtest-findings.json`**,
   the same day, with its general form and the check that carries it — this is
   the cross-campaign ledger, and `GENERATION.md`'s table is the per-campaign
   view of it. A finding recorded only in the campaign is a finding the NEXT
   campaign learns nothing from.
5. **Audit the FULL ledger from round 1 before staging any build** — never from
   the last round, and never by reading. You do not have to remember to: the
   staging paths REQUIRE it. `tools/playtest-server.sh up` runs the gate between
   the build and the container and refuses to serve a red build; the compose
   owner-play path requires the admission token the gate mints. Run it yourself
   first so the red list is in the round summary before she is invited:

       python3 tools/staging-gate.py --campaign <dir> --build <out> --report round-N-gate.md

   The gate answers the question rules 3 and 4 are about — for every finding
   ever reported, on any campaign, does its general form exist and does it BIND,
   non-zero, on THIS build — and it distinguishes the ways a green has lied
   here: never built, check gone, matched nothing, the campaign has none of the
   objects, or the campaign's `dsl_version` never reached the surface the check
   keys off. **A red is not permission to stop**: it is the list of defect
   classes she is not protected from, and it goes into the round summary item by
   item. Never backfill a weak diagnostic to turn a row green. To show her a red
   build deliberately (a framing check, not a QC round), the override is
   `--stage-anyway "<reason>" --acknowledge-red <N>` — it prints every class
   being overridden and the server announces it at boot.
6. **Pre-flight, in this order, before the invitation**: full ladder green
   (PackTest → bot critical path + die-retry → every branch run) → staging gate
   (step 5) → localized builds + double-build byte-identical → server boots and
   self-checks → then invite. Not "the build compiled, come look".
7. **Update `DESIGN.md` in the same round and run its conformance review.** The
   island's design record went eight rounds unupdated and the audit that caught
   up found seven changes no one had asked for.
8. **Close the round in `GENERATION.md` with its machine record**, not just
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
- **Commit only canonically formatted JSON.** The last thing you do before any
  `git add` of a stage document or sidecar is `delvec fmt <campaign-dir>`; CI
  runs `delvec fmt --check`. A diff that rewrites a file nobody edited is the
  defect this closes.
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
