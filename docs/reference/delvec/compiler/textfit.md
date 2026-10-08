# `delvec::compiler::textfit`

The reference page for `crates/delvec/src/compiler/textfit.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0330 — on-screen text fit (`compiler::textfit`; **warning**; exit 0)

An **advisory** code. Vanilla draws a `title`, a `subtitle`
and an art title centred, on **one line, with no wrapping and no shrink-to-fit** —
text wider than the screen just runs off both edges, silently. `DW0330` measures
each on-screen `narrate` string's **rendered width in font pixels** and compares it
to the style's budget.

**Why measured, not counted.** `i` and `W` differ by 3× in the vanilla font, and a
Han glyph is 9 px against a Latin letter's 6 (1.5×, *not* the 2× a "CJK counts
double" rule assumes). A character count is unfair to whichever script it was not
tuned for, so the check sums real advances: the ASCII sheet's per-glyph widths, the
`unihex` full-width advance for CJK, and — for `art` — the `delve:art` font's own
glyph metrics, derived from the same constants that emit the font.

**Budget.** `Gui.renderTitle` renders a title at pose scale **×4** and a subtitle at
**×2**; an art title is a title, so it takes ×4 on top of the `delve:art` provider's
own scale (see [The `delve:art` font](atmos.md#the-delveart-font)). Against a reference GUI
width of **426** scaled px (what Minecraft's auto GUI scale yields at 1280×720 and
2560×1440; 1920×1080 gives 480, and 320 is the auto floor) at **85%** usable width,
the budgets are **90** font px for `title` and `art` and **181** for `subtitle`.
`chat` has no budget — it wraps and scrolls. At the art font's 6 px/glyph that is
**15 art glyphs**; the lint reads the same `ART_GLYPH_ADVANCE` / `ART_SPACE_ADVANCE`
constants the font emission does, so the two cannot drift.

**Why warning, not error.** The true limit is a property of the player's window and
GUI scale, which the compiler cannot know; rejecting on it would dress a judgement
call as a fact, and would hard-block a translation for being honestly longer than its
English source. It reports, and the author shortens.

**Scope.** The canonical English source **and** every declared-language sidecar
rendition, walked by the same `each_effect_ref` traversal and l10n keying as
`DW0326`/`DW0328` — so a sidecar finding is reported at
`l10n/<lang>.json#/content/<key>`, naming the exact string to shorten. Nested
effects are covered.

| Code | Meaning |
|------|---------|
| `DW0330` | An on-screen `narrate` string (`title` / `subtitle` / `art`) — English source or any declared-language sidecar rendition — renders wider than fits on screen. Advisory (exit 0): shorten the line. Do **not** demote a title to `chat` to silence it, and do not assume a wider monitor fixes it — the overflow scales with GUI scale, not away from it. |

### DW0331 — dialogue option button fit (`compiler::textfit`; **error**; exit 1)

Same font metrics as `DW0330`, a harder limit, and the opposite severity — for a
reason that is worth stating precisely, because "follow the precedent" here means
following its *reason*, not copying its tier.

**A dialogue option is a button caption.** `emit::build_node_dialog` emits each node
as a `minecraft:multi_action` dialog with `columns: 1` and **no `width` override**,
so every option button is vanilla's default **150 GUI px**. Vanilla draws a button's
label via `AbstractWidget::renderScrollingString`, inset **2 px** per side: a label
wider than the remaining **146 px** neither wraps nor shrinks — it **scrolls back and
forth**, and a shelf of sliding captions is unreadable to choose from.

**Budget: 146 font px, no scale divisor.** Dialog buttons draw at the identity pose
(**×1**), so one font pixel is one GUI pixel — unlike `DW0330`'s titles at ×4/×2.
Rules of thumb from the advances, for authoring: ~24 Latin or ~16 Han characters at
the threshold, so author to **~20 / ~12** and leave a translation room to grow
(`.claude/skills/delvewright/skills/new-delve/references/writing-craft.md` §C;
`docs/reference/i18n.md`).

**Why error, not warning.** `DW0330` warns because its reference GUI width is a guess
about the *player's window*, which the compiler cannot know, and rejecting a build on
a guess dresses a judgement call as a fact. That reasoning does not transfer: 150 px
is the button width because **this compiler emitted no `width`**, on every window at
every GUI scale. `width > 146` therefore *is* "this caption scrolls in game" — a
property of the datapack being built, so it rejects. The remedy is never a wider
button: move the content into the node's body text, which wraps, or into the NPC's
reply.

**Not the option `tooltip`.** A `tooltip` is a sibling of `label` in
vanilla's button codec but is never drawn on the button: the client wraps it with
`Tooltip.create(…)` → `Font.split(message, 170)` into its own hover box. Wrapping
is the whole difference — the defect `DW0331` rejects is a caption *scrolling*
inside a fixed button, and nothing overruns a box that wraps. So a tooltip carries
no width budget, and inventing one would forbid exactly the pattern the field
exists for ("button = caption, tooltip = the full line"). The label on an option
that also has a tooltip is measured like any other label.

**Scope.** Every `.opt.<n>.label` in the canonical English source **and** every
declared-language sidecar rendition, keyed by the same `dlg.<npc>.<node>.opt.<i>.label`
inventory keying as `validate_l10n` (`dsl::l10n::dialogue_option_labels`) — so a
`zh-cn` label that overflows where its English source fits is reported at
`l10n/<lang>.json#/content/<key>`, naming the language and the exact string. Display
gating (`requires_flags`/`forbids_flags`) decides *whether* a variant shows a label,
never how wide it renders, so gated options carry the same budget.

| Code | Meaning |
|------|---------|
| `DW0331` | A dialogue option `label` — English source or any declared-language sidecar rendition — renders wider than the 146 usable font px of the 150-GUI-px dialog button it is drawn on, so vanilla scrolls the caption instead of sitting it still. Error (exit 1): cut the label to a caption and move what it carried into the node's body text or the NPC's reply. Scope follows the **widget**, not the stage: a `bonfire`'s authored `rest_label` / `save_label` are drawn on exactly that button and are held to exactly that budget, reported under the `quests` stage. The compiler's own canonical English is measured once by a unit test rather than per campaign, since it cannot vary. |
