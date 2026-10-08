# spec-0096: A span of text carries a style — inline markup in every player-facing line, lowered to vanilla text components, kept through transcreation

- **Status**: Approved
- **Ground**: written against engine `dabe45cc2` (`origin/main`), read only — `each_string`, `inventory`, `tag`, `untag`, `plain`, `tag_translatables`, `TextKind` and `key_kind` in `crates/dsl/src/l10n.rs`; `tr`, `tr_with`, `snbt_component`, `snbt_text_component`, `snbt_translate`, `lang_assets` and `check_untranslated_literals` (`DW0185`) in `crates/delvec/src/compiler/emit.rs`; `check_text_fits` (`DW0330`) and `check_option_labels` (`DW0331`) in `crates/delvec/src/compiler/textfit.rs`; `check_art` (`DW0328`) in `crates/delvec/src/compiler/atmos.rs`; `worded` in `crates/delvec/src/compiler/deathplan.rs`; the `SURFACE` table of `crates/dsl/tests/l10n_surface.rs`; `cast_bark_fns` in `emit.rs`; `PLACEHOLDER_RE`, `check_row` and the prompts in `tools/creator/i18n-translate.py`.
- **What it is for**: a creator needs part of a line to render with a vanilla text style — above all `obfuscated`, the shifting glyphs. The first user is an NPC whose mind has been touched: one bark in the rotation carries an obfuscated span. Today no authored line can carry any style; the emitter styles whole components only (a bark is italic, a speaker name is yellow), and nothing an author writes reaches a style field.
- **Research**: each statement is marked **cited** (a line of the tree at `dabe45cc2`, or a published source) or **authored** (this spec chooses). An unsupported claim is named as one.
- **Numbers**: spec `0096`. Two diagnostics, `DW0975` (malformed markup, §4.1) and `DW0976` (a translation whose spans differ from the English, §4.2). `DW0977` was handed to this spec and is not consumed (§4.3). The surface is a stage-content change inside the unpublished `dsl_version`, which unreleased format changes share; no version literal moves.
- **Non-goals**: styles on compiler chrome (the engine's own strings in `dsl::chrome`, which carry no authored text); nested spans; a span that changes font, inserts a click or hover event, or carries anything but the six style keys of §3.2; a written book (the DSL has no book surface at `dabe45cc2` — `grep -i book crates/dsl/src/stages.rs` names only enchanted books, an item id — so "books" in the motivating issue binds nothing; a book surface would take this markup when it lands, because it would be an inventoried string).

## 1. The object it belongs to

**Cited** (`l10n.rs`, `each_string`): every player-facing string of a campaign is visited by one traversal, which is the inventory (`inventory`), the translation swap (`localize`) and the tagger (`tag_translatables`) at once. **Cited** (`l10n_surface.rs`, `SURFACE`): the schema export's string properties are each classified, and the rows classified `Inventoried` are exactly the strings that traversal reaches — 36 rows at `dabe45cc2` (`Actor.name`, `Area.name`, `Boundary.message`, `CastBarks.barks`, `Class.blurb`, `Class.name`, `DialogueNode.text`, `DialogueOption.label`, `DialogueOption.tooltip`, `HealthBar.title`, `ItemDrop.name`, `KitItem.name`, `LethalVolume.message`, `Shop.title`, `ShopOffer.label`, `ShopOffer.tooltip`, `Stake.collected_message`, `LootItem.name`, `Npc.name`, `Objective.hint`, `Objective.item_name`, `Objective.missing_item_hint`, `Objective.title`, `PlannedQuest.goal`, `QuestEffect.name`, `QuestEffect.prompt`, `QuestEffect.rest_label`, `QuestEffect.rest_tooltip`, `QuestEffect.save_label`, `QuestEffect.save_tooltip`, `QuestEffect.sealed_hint`, `QuestEffect.text`, `StateDecl.name`, `WaveMob.name`, `WorldContent.outro`, `WorldContent.title`), spanning the 13 classes of `TextKind`. **Cited** (`emit.rs`): every tagged string reaches the built tree through one of three functions — `tr`/`tr_with` (JSON component), `snbt_component` and `snbt_text_component` (SNBT component) — and `DW0185` refuses a build whose bytes still carry a tag, so no fourth path exists.

**Authored**: the style is a property of the **player-facing string**, the object class the inventory already defines, not of any one verb. The markup is accepted in every inventoried string, checked over the inventory and every sidecar, and lowered in the three functions above. No per-class field is added.

## 2. What vanilla offers

- **Cited** (Minecraft Wiki, *Text component format*, Java Edition 1.21.5+): a component carries the style keys `color` (one of sixteen names, or `#RRGGBB`), `bold`, `italic`, `underlined`, `strikethrough` and `obfuscated`; a `translate` component carries `with` (the arguments substituted for `%s` / `%1$s` in the translated text) and `fallback` (used in place of a missing translation); `%%` is a literal percent sign; a child inherits its parent's style and overrides it key by key. In NBT (`CustomName`) the same component is written as SNBT, with booleans as `true`/`false`.
- **Cited** (same page, *Formatting codes*): obfuscated text draws each glyph as a random glyph of the same width, changing every frame.
- **Unsupported claim** (recalled from Mojang's decompiled `TranslatableContents.decompose`, not re-read off the pinned jar for this spec): a translated text whose format is malformed — a lone `%` not followed by `s`, `d` or `%`, or an argument index past the end of `with` — is drawn verbatim, its arguments lost. The emitter therefore escapes every `%` of a styled line (§3.3) so the claim cannot bite; the demo level (§7) is where the rendered result is looked at.
- **Unsupported claim** (Minecraft Wiki, *Formatting codes*, not measured): bold widens a glyph by one font pixel.

## 3. The surface

### 3.1 Grammar

A span is written inside any player-facing string as

```
[[<styles>|<text>]]
```

- `<styles>` is one or more of `obfuscated`, `bold`, `italic`, `underlined`, `strikethrough`, `color=<colour>`, separated by `,`, with no whitespace, each at most once, in any order. `<colour>` is one of the sixteen vanilla names (`black`, `dark_blue`, `dark_green`, `dark_aqua`, `dark_red`, `dark_purple`, `gold`, `gray`, `dark_gray`, `blue`, `green`, `aqua`, `red`, `light_purple`, `yellow`, `white`) or a `#rrggbb` literal (`dsl::color::is_hex`, the rule every other colour literal in the DSL reads).
- `<text>` runs from the first `|` to the first `]]`, is non-empty, holds a character that is not whitespace, and contains no `[[`.
- Outside a span, `[[` and `]]` do not occur. A single `[` or `]` is ordinary prose.

**Authored**, and why this spelling: a doubled square bracket does not occur in English or Chinese prose, in the game's chat, or in any surface the DSL has, so a line that carries one is markup or is a defect — never ambiguous; a single bracket stays free for prose. The style list is spelled with the component's own key names, so the line says what the emitted JSON will say.

Example (the gallery's bark): `The ledger is kept by [[obfuscated|someone else]] at [[italic,color=dark_purple|night]].`

### 3.2 What a span means

**Authored**: the span renders its text with each named key set (`true` for a flag, the colour for `color`) on top of the style the emitter gives the whole line. A key the span does not name is inherited from the line. `color=#RRGGBB` is emitted lower-case.

### 3.3 Emission

**Authored**: a tagged line `K` whose English carries spans lowers to

```
{"translate": K, "fallback": F, "with": [S_1, …, S_n], …line style}
S_i = {"translate": K.span.<i-1>, "fallback": T_i, …span style}
```

where `F` is the line with each span replaced by `%<i>$s` and every other `%` doubled, and `T_i` is the span's text with every `%` doubled. A line with no span emits exactly what it did at `dabe45cc2`. The SNBT form is the same compound, keys in the same (alphabetical) order. The language files carry `K → F` and `K.span.<i-1> → T_i` for every declared language, with `en_us` from the English. An untagged string carrying spans lowers to `{"text": "", "extra": […]}`: that is every string of a `--lang` bake, whose lines are swapped to the target language and never tagged; no compiler literal carries a span.

A translation's spans are **aligned** to the English by style: the translated line's spans are matched to the English spans of the same style in order of appearance, and each is written under the English span's index — so `F` in another language may carry `%2$s` before `%1$s`, and the style always rides on the component, never in the language file. A `--lang` bake (no language files) lowers the translated line in its own order.

### 3.4 Everything that reads a line without drawing it

**Authored**: every consumer that reads an authored string as text rather than as a component reads its **visible text** — the line with each span replaced by its text: `dsl::l10n::plain` (the manifest, the render plan, the named exclusions of `DW0185`), the death plan's `message` (what the bot watches chat for, `deathplan::worded`), the art-font glyph check (`DW0328`) and both width checks (`DW0330`, `DW0331`).

**Width.** **Authored**: an obfuscated glyph has its source glyph's width (§2), so obfuscation adds nothing; a bold span adds one font pixel per character, every character counted, spaces included. The bold measure is the unsupported claim of §2, rounded in the direction that can only turn a width check red, never let an overrun ship.

**Bark rotation.** **Cited** (`cast_bark_fns`): a bark pool is a scoreboard ladder over line indices with no RNG; a styled line is one more component on its own rung, so the rotation is unchanged.

## 4. Refusals

### 4.1 `DW0975` — markup that does not parse

Validation tier (exit 1), `dsl::textstyle::validate_inline_styles`, over every inventoried English string and every row of every sidecar. Raised for: an unclosed `[[`; a `]]` with no opening; a span with no `|`; an empty style list; an unknown style; a style written twice; a colour that is neither a vanilla name nor `#rrggbb`; an empty or all-whitespace text; a `[[` inside a span's text. The message names the key, the character offset, what was found and the grammar of §3.1. Prescription: correct the span, or remove the brackets.

### 4.2 `DW0976` — a translation whose spans are not the English's

Validation tier (exit 1), same function, over every sidecar row whose English and translation both parse: the multiset of span styles differs (a span dropped, added, or restyled). The message names the key, the English styles and the translated styles. Prescription: keep every span of the English, with its style unchanged and its text transcreated; it may move within the line. The build re-proves the alignment when it writes the language files and refuses with the same code.

### 4.3 `DW0977` — not consumed

No third refusal is needed: a span key `K.span.<i>` cannot collide with an inventory key (no inventory key has a `span` segment followed by an index — the key scheme of `l10n.rs`), and the build asserts it rather than diagnosing an input that cannot produce it.

## 5. The translator

**Authored**: `tools/creator/i18n-translate.py` instructs every step to keep each `[[…|` opener byte for byte and each `]]`, transcreating only the text inside; its fact check compares the openers and closers of the English and the translation as it compares placeholders (`PLACEHOLDER_RE`), and a row that loses or changes one goes back through the fix step and is refused if it still does. The authority stays the compiler: the tool's closing `delvec validate` runs `DW0975`/`DW0976` over the sidecar it wrote.

## 6. Gallery

The gallery's counter (`npc/factor`) gains a third bark carrying two spans, one `obfuscated`, one `italic,color=dark_purple`, with its zh-cn row carrying the same spans in Chinese order. Two probes are committed, each the primary plus one declared edit: an unclosed span in that bark (`DW0975`), and the zh-cn row with its obfuscated span dropped (`DW0976`). The markup is not a schema unit (it lives inside strings the schema already declares), so `check-gallery-coverage.py` binds no new unit and both probes declare none.

## 7. Demo level

A row in `docs/demo-levels.md`: **The Touched Clerk** — one room, one NPC whose bark rotation carries a line with an obfuscated span, a dialogue node with a coloured span, an objective title with a bold span; walked in English and in zh-cn. The look is the thing no machine decides: whether the shifting glyphs read as a mind that has been touched, and whether the span sits where the Chinese line puts it.

## Acceptance criteria

Each is checked against the tree at `dabe45cc2`, where none holds; the implementation lands them in one pull request.

1. **Grammar.** Unit tests in `crates/dsl/src/textstyle.rs` assert: a line with no `[[`/`]]` parses as one text segment; each of the six style keys parses; `#AbCdEf` is accepted and emitted `#abcdef`; each refusal shape of §4.1 is an error naming its offset; `visible()` of the gallery bark is `The ledger is kept by someone else at night.`
2. **Every class.** A test builds a campaign in which every `TextKind` that the fixture campaign exercises carries a span, runs `validate_inline_styles` and finds no diagnostic, and asserts the inventory keys carrying a span cover every `TextKind` present in that inventory.
3. **Emission.** A build test asserts, on the emitted bytes: the styled bark's `tellraw` component carries `translate`, a `fallback` with `%1$s` and `%2$s`, `"italic":true` on the line, and a `with` array whose first element carries `"obfuscated":true` and a `translate` ending `.span.0`; a `%` in a styled line is emitted `%%`; an unstyled line's component is byte-identical to its form without the feature (no `with`); the SNBT form of a styled NPC name carries `with:[{…obfuscated:true…}]`.
4. **Language files.** A build test with a zh-cn sidecar asserts `en_us.json` and `zh_cn.json` both carry `K`, `K.span.0` and `K.span.1`, and that a zh-cn row whose two spans appear in the opposite order to the English writes `%2$s` before `%1$s` with each span text under the English span's index.
5. **Refusals.** `DW0975` is raised by `delvec validate` for an unclosed span in English and for a malformed span in a sidecar row; `DW0976` for a sidecar row that drops a span and for one that restyles it; both asserted by tests and by the two gallery probes under `tools/ci/check-gallery-coverage.py`.
6. **Readers of visible text.** Tests assert `l10n::plain` and the death plan's `message` carry no `[[`; `DW0331` measures a label `[[bold|…]]` one pixel per character wider than the same label unstyled, and an `obfuscated` label exactly as wide; `DW0328` does not flag the markup characters of a styled art title.
7. **Rotation.** The gallery's `bark_factor_1` function (the counter's scene 1) holds three rungs, the styled line on the third; two builds are byte-identical (the determinism test).
8. **Translator.** `pytest tools/tests` asserts `check_row` refuses a translation that drops a span opener or closer and passes one that moves a span.
9. **Gallery.** The gallery bark and its zh-cn row are committed; the regenerated `gallery/baseline/` moves and the commit body attributes every moved row; a perturbation of the bark's `obfuscated` to `bold` moves an emitted byte of `bark_factor_1`.
10. **Docs and skill, same change.** `docs/reference/compiler.md` carries the surface row, the emission rule and the `DW0975`/`DW0976` rows (`tools/ci/check-dw-codes.py` green); `docs/reference/i18n.md` states the span keys; `docs/reference/tools.md` states the translator's span rule; `docs/reference/game-writing.md` §2 carries the rule on when a style is right; the `/new-delve` skill's writing references state the grammar and point to it; `docs/specs/README.md` carries this spec's row; `docs/demo-levels.md` carries §7's row as pending.
