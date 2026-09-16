## Contents

- [Reference: writing craft](#reference-writing-craft)
- [A. Automatic-phrasing tells](#a-automatic-phrasing-tells)
- [B. Convergence is the real tell — vary the posture per campaign](#b-convergence-is-the-real-tell--vary-the-posture-per-campaign)
- [C. HARD RULE — dialogue options are labels, not sentences](#c-hard-rule--dialogue-options-are-labels-not-sentences)
- [D. HARD RULE — a name spelled the same way IS the same name](#d-hard-rule--a-name-spelled-the-same-way-is-the-same-name)
- [E. Plain-prose baseline (Strunk 1918, public domain)](#e-plain-prose-baseline-strunk-1918-public-domain)

# Reference: writing craft

Everything a player reads is prose: dialogue, objective titles and hints,
narration beats, item and area names, the storybook. This is the craft checklist
for all of it, and section A is run over every line before step 5 is called done.

These are **pattern warnings, not technique bans.** They govern *automatic*
writing — the phrasing that arrives before you have decided anything, the hand
reaching before the mind does — not the device itself. A simile is not
forbidden; the simile you did not choose is. Banning a technique outright
produces stilted avoidance, which is its own tell.

## A. Automatic-phrasing tells

1. **Observation + verdict.** A line, then the text grading it — "*…, more
   statement than question*", "*…, and it was not a request*". The verdict
   instructs the player how to hear what they just read. Cut the verdict; if the
   line cannot stand without it, the line is wrong.
2. **Standalone simile fragments.** "*Like a blow to the chest.*" — a comparison
   set alone as though it were the feeling. Test every simile: does it make the
   player see the **thing** more sharply, or make them notice the author? The
   second kind goes.
3. **Stock intensity moves.** The air growing thick or heavy; time slowing;
   silence stretching; words left hanging in the air; a breath the character did
   not know they were holding. These are the default gestures at "this moment
   matters", and every generated delve reaches for them unprompted.
4. **Repetition as intensity.** Saying it again, louder — "*more than tired:
   hollow*"; three-beat lists where two beats carry all the meaning.
5. **Correction pairs.** Naming a false label in order to knock it down — "*not a
   warning, a promise*"; "*it stopped being a door and became a mouth*". Once per
   campaign is a rhetorical choice; three times is a signature.
6. **Purposeless gesture.** A nod, a tightening jaw, a hand moving to a hilt,
   costing nothing to delete. A gesture signifies by contrast with what that
   character usually does. If it can be cut with no loss, cut it.
7. **Explaining your own subtext.** An NPC says the hard thing, then the next
   line paraphrases what it meant. Trust the player; they have already read it.

Applies hardest where the text is shortest: `hint`, `title`, bark pools and
`missing_item_hint` have no room to recover from a wasted clause.

## B. Convergence is the real tell — vary the posture per campaign

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

**Claude specifically** is the most distinctive of the models measured, and its
fingerprint is restraint: *the flattest event escalation of any source*, the most
uniform narrative voice, epilogues over avalanche endings, and reverence toward
genre convention rather than subversion (62% vs 39–56%). Read that as a standing
instruction: **the default delve escalates too evenly and ends too quietly.**
Give a campaign a beat that is disproportionate to what came before, and let at
least one thing end badly or unresolved.

Operationally, per campaign:

- Pick **at least three** axes above and push them off the default *for this
  campaign* — a delve told out of order; a cast whose antagonist is right; an
  ending that refuses to explain itself; an NPC who names their fear in plain
  words instead of clenching a fist.
- Record the choice as a one-line **posture note** in `GENERATION.md`: which
  three axes, and how. It is a design commitment, not a report.
- Vary them **between** campaigns. A fixed counter-recipe applied every time just
  builds a second cluster — dispersion is the human signal, not any particular
  pole.
- Corollary, and it inverts the usual advice: **"show, don't tell" is a machine
  default here.** Somatic rendering is what the pole looks like. Sometimes let a
  character say they are afraid.

## C. HARD RULE — dialogue options are labels, not sentences

**A dialogue option is a button caption.** Vanilla draws each option as a
fixed-width button; a label wider than the button *scrolls* rather than wrapping
or shrinking, and a shelf of scrolling captions is a miserable thing to read and
pick from. This is not a style preference — it is the widget.

The geometry, so the budget is arithmetic and not taste. The compiler emits each
node as a `minecraft:multi_action` dialog with `columns: 1` and **no `width`
override**, so every option button is vanilla's default **150 GUI px**, leaving
roughly **146 px** for the label after the widget's inset. Dialog buttons draw at
pose scale ×1, so one font pixel is one GUI pixel — unlike `narrate` titles,
which `DW0330` budgets at ×4/×2.

**Width is measured in font pixels, not characters**, because `i` and `W` differ
by 3× and a Han glyph (advance 9) is 1.5× a Latin one (typical advance 6), so any
character count is unfair to whichever script it was not tuned for. The character
counts below are the authoring rule of thumb derived from those advances — the
pixel budget is the real rule:

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
`.opt.<n>.label`, in the English source **and** in every l10n sidecar.

`DW0331` enforces this at compile time on the same font-pixel measurement
`DW0330` uses. Author to the target and it never fires.

## D. HARD RULE — a name spelled the same way IS the same name

Every name you write over a body is translated, and **whether two bodies share
one translation is decided by whether you spelled them identically** — not by
whether you meant the same character. Apply this while you are naming, because it
is unrecoverable later: by the time a translator sees the list, your intent is
gone and only the spelling is left.

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
of whom may be called something else entirely in Chinese. Copy the NPC's name; do
not retype it.

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

## E. Plain-prose baseline (Strunk 1918, public domain)

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
