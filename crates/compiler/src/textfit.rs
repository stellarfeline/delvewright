//! On-screen text that does not fit what draws it: narrate titles that overrun the
//! screen (`DW0330`) and dialogue option labels that overrun their button
//! (`DW0331`).
//!
//! Vanilla draws a `title`, a `subtitle` and an art title **centred, on one line,
//! with no wrapping and no shrink-to-fit**: text wider than the screen simply runs
//! off both edges. Nothing in the game clips or warns, so an over-long string ships
//! looking broken. The owner hit this in QA round 4 with Simplified-Chinese titles,
//! where the same sentence is far wider than its English source.
//!
//! This module measures a string's **rendered width in font pixels** and compares it
//! against the per-style budget. Measuring beats counting characters: `i` and `W`
//! differ by 3× in the vanilla font, and a Han glyph is 1.5× a Latin one — a
//! character count is unfair to whichever script it was not tuned for.
//!
//! ## Geometry (vanilla, 1.21.11)
//!
//! `Gui.renderTitle` pushes a **×4** pose scale for the title and **×2** for the
//! subtitle, then draws centred at `guiWidth / 2`. An art title is a `title` in the
//! `delve:art` font, so it takes the ×4 title scale on top of that font's own
//! provider scale ([`crate::atmos::ART_SCALE`]). Those two multiply — which is why
//! the art font renders at source size (×1) and still draws a title-sized banner.
//! Through v0.6 the provider scale was ×4, and ×4 × ×4 left room for four glyphs;
//! see `ART_SCALE` for why it is 1.
//!
//! So a string fits when `width_in_font_px * style_scale <= usable_gui_width`.
//!
//! ## Severity: warning, not error
//!
//! The true limit depends on the player's window size and GUI scale, which the
//! compiler cannot know. [`REF_GUI_WIDTH`] is a defensible reference point, not a
//! fact about the player's screen — a wide window fits more, a small one less.
//! Rejecting a build on it would dress a judgement call as a certainty, and would
//! hard-block a translation for being honestly longer than its English source.
//! `DW0330` therefore reports at **advisory tier**: it is printed like any other
//! diagnostic and shows up in `--json`, but does not fail `validate`/`analyze`/
//! `build`. It is the first warning-tier code in the compiler; `delvec` exits
//! non-zero only on `Severity::Error`.
//!
//! ## Dialogue option labels (`DW0331`) — the same measurement, a harder limit
//!
//! Owner directive, 2026-08-03. A dialogue option is a **button caption**, not a
//! sentence. [`crate::emit`]'s `build_node_dialog` emits each node as a
//! `minecraft:multi_action` dialog with `columns: 1` and **no `width` override**, so
//! every option button is vanilla's default [`DIALOG_BUTTON_WIDTH`]. Vanilla draws a
//! button's label with `AbstractWidget::renderScrollingString`, inset
//! [`BUTTON_LABEL_INSET`] px on each side: a label wider than what is left does not
//! wrap and does not shrink — it **scrolls back and forth**, and a shelf of sliding
//! captions is unreadable to pick from.
//!
//! Two things make this a stricter check than `DW0330`, not a copy of it:
//!
//! - **Pose scale ×1.** A dialog button draws at the identity pose, so one font pixel
//!   is one GUI pixel — no ×4/×2 division as for titles.
//! - **Error tier, not advisory.** `DW0330` warns because [`REF_GUI_WIDTH`] is a
//!   guess about the *player's window*, which the compiler cannot know; rejecting a
//!   build on a guess dresses a judgement call as a certainty. That reasoning does
//!   not transfer. A dialog button is 150 GUI px because the compiler emitted no
//!   `width`, on every window at every GUI scale — the widget's own geometry, fixed
//!   by the datapack this compiler writes. `width > 146` therefore *is* "this label
//!   scrolls in game", a fact, so `DW0331` rejects. Following the precedent means
//!   following its **reason** (warn about what you cannot know, reject what you
//!   emitted), not copying its tier.
//!
//! The remedy is never a wider button — the fix is to move the content into the
//! node's body text, which wraps, or into the NPC's reply.
//!
//! ### What `DW0331` deliberately does NOT measure: the option `tooltip` (v0.8)
//!
//! An option's `tooltip` is a *sibling* of `label` in vanilla's `CommonButtonData`
//! codec, but it is not drawn on the button. The client's `DialogControlSet` wraps
//! it in `Tooltip.create(…)`, and `Tooltip` splits its component with
//! `Font.split(message, 170)` — it **wraps at 170 px into a hover box**. Wrapping
//! is the whole difference: the failure `DW0331` exists to reject is *scrolling*,
//! which is what `renderScrollingString` does when a caption overruns a fixed
//! button. Nothing overruns a tooltip, so there is no budget to enforce and no
//! diagnostic to raise — measuring one would be inventing a limit vanilla does not
//! declare. (Both facts read off the pinned 1.21.11 client jar.) This is the
//! authored shape of the wine-beat pattern: **button = caption, tooltip = the full
//! line.**

use std::collections::BTreeMap;

use delvewright_dsl::{
    Campaign, Diagnostic, L10nDoc, NarrateStyle, OptionLabel, bonfire_option_labels,
    dialogue_option_labels, on_screen_narrates,
};

use crate::atmos::{ART_GLYPH_ADVANCE, ART_SPACE_ADVANCE};

/// `DW0330`: an on-screen `narrate` string (`title` / `subtitle` / `art`), in the
/// English source or a sidecar translation, is wider than the screen renders.
pub const DW_TEXT_OVERRUNS_SCREEN: &str = "DW0330";

/// `DW0331`: a dialogue option label, in the English source or a sidecar
/// translation, is wider than the dialog button vanilla draws it on, so the caption
/// scrolls instead of sitting still.
pub const DW_OPTION_LABEL_SCROLLS: &str = "DW0331";

// ---------------------------------------------------------------------------
// Screen geometry
// ---------------------------------------------------------------------------

/// Reference GUI width in **scaled** pixels.
///
/// Minecraft's auto GUI scale (`Window.calculateScale`) picks the largest integer
/// scale that keeps the scaled framebuffer at least 320×240, so the scaled width is
/// never below 320 and in practice lands at 426 (1280×720 and 2560×1440 both scale
/// to 426×240) or 480 (1920×1080 → ×4). 426 is the realistic low end of modern
/// setups: strict enough to catch what a 720p or high-DPI player sees, without
/// budgeting for the 320 floor that only a deliberately tiny window produces.
pub const REF_GUI_WIDTH: u32 = 426;

/// Percentage of [`REF_GUI_WIDTH`] on-screen text may occupy. The remainder is side
/// margin: text that reaches the very edge reads as overflow even when it technically
/// fits, and the margin absorbs the difference between this reference and a slightly
/// narrower window.
pub const SAFE_PERCENT: u32 = 85;

/// The usable width in scaled pixels: [`REF_GUI_WIDTH`] minus the side margin.
const USABLE_WIDTH: u32 = REF_GUI_WIDTH * SAFE_PERCENT / 100;

/// The pose scale vanilla renders `style` at, and the font it uses.
fn style_scale(style: NarrateStyle) -> u32 {
    match style {
        // `Gui.renderTitle`: pose.scale(4) for the title, pose.scale(2) for the
        // subtitle. An art title is a title, so it too is ×4.
        NarrateStyle::Title | NarrateStyle::Art => 4,
        NarrateStyle::Subtitle => 2,
        // Chat wraps and scrolls — no width budget. The actionbar draws at ×1 and
        // vanilla neither wraps nor truncates it, but it is a reply strip rather
        // than a banner and is not width-policed. Neither reaches here
        // (`narrate_on_screen` excludes both), but keep the match total and honest.
        NarrateStyle::Chat | NarrateStyle::Actionbar => 1,
    }
}

/// The width budget in **font pixels** for `style`.
pub fn budget(style: NarrateStyle) -> u32 {
    USABLE_WIDTH / style_scale(style)
}

/// The width, in GUI pixels, of one dialog action button.
///
/// Vanilla's dialog action codec defaults `width` to 150 (range 1..=1024), and
/// [`crate::emit`]'s `build_node_dialog` emits no `width` — so this is the width of
/// every button this compiler ships, on every window at every GUI scale. Unlike
/// [`REF_GUI_WIDTH`] this is not a reference point: it is a property of the datapack
/// the compiler writes, which is why `DW0331` can reject rather than advise.
pub const DIALOG_BUTTON_WIDTH: u32 = 150;

/// The horizontal inset, per side, between a button's edge and its label.
/// `AbstractButton::renderWidget` calls `renderScrollingString(…, this.getX() + 2, …,
/// this.getX() + this.width - 2, …)`: two pixels of padding at each end, and the
/// helper *scrolls* whatever does not fit between them.
const BUTTON_LABEL_INSET: u32 = 2;

/// The usable label width on a dialog button, in font pixels. Buttons draw at the
/// identity pose (×1), so a font pixel is a GUI pixel and no scale divides this —
/// contrast [`budget`], where the ×4 title pose costs three quarters of the screen.
pub const BUTTON_LABEL_BUDGET: u32 = DIALOG_BUTTON_WIDTH - 2 * BUTTON_LABEL_INSET;

// ---------------------------------------------------------------------------
// Font metrics
// ---------------------------------------------------------------------------

/// Advance of a space in the vanilla default font (the `space` provider; the ASCII
/// bitmap cell for `0x20` is blank, so the bitmap provider gives it 0).
const SPACE_ADVANCE: u32 = 4;

/// The advance of a full-width glyph — Han/Kana/Hangul and full-width punctuation.
/// The `unihex` provider derives an advance from the glyph's ink extents,
/// `(right - left + 1) / 2 + 1`, but `default.json`'s `size_overrides` pin the CJK
/// blocks to the full 16-column cell, so every Han glyph lands on 9 regardless of how
/// much ink it actually has. Against a Latin letter's 6 that is **1.5×** — not the 2×
/// a naive "CJK counts double" rule assumes, which is exactly why this check measures
/// rather than counting characters.
///
/// The override makes this exact for Han, Kana and full-width punctuation. Hangul
/// syllables are pinned `left=1` instead, so they are really 8 — a 1 px overestimate
/// on a script no delve ships in, not worth a second branch.
///
/// Caveat, quantified rather than guessed: with the client's **Force Unicode Font**
/// option on, the provider stack collapses to `[space, unihex]`, Latin drops to mostly
/// 4 and the ratio becomes 9:4. This check budgets against the vanilla default, which
/// is the stricter side for CJK and the setting essentially every player is on.
const FULL_WIDTH_ADVANCE: u32 = 9;

/// The advance assumed for any other codepoint: accented Latin, Greek, Cyrillic and
/// the rest of the BMP. Those are served by the `accented` / `nonlatin_european`
/// bitmap sheets at `advance = ink + 1` like ASCII, where a typical letter is 6.
const TYPICAL_ADVANCE: u32 = 6;

/// Advance widths for printable ASCII in the vanilla default font, indexed by
/// `ch - 0x20`. Vanilla's `BitmapProvider` derives these from `font/ascii.png`:
/// `advance = round(inkWidth * height / cellHeight) + 1`, and for the ASCII sheet
/// `height == cellHeight == 8`, so the advance is simply the glyph's ink width + 1.
///
/// **Measured**, not recited: `tools/extract-font-metrics.py` reads the sheet out of
/// the 1.21.11 client jar and applies that rule (see `data/PROVENANCE.md`). The jar is
/// EULA-bound and never vendored, so the result is committed here as a constant and
/// the script is the reproduce path. 68 of the 95 are 6; the measured table matches
/// the widely-cited community one exactly, which is the cross-check that the decode is
/// right — recollection of that table is what was wrong (7 values) before measuring.
///
/// Only `font/ascii.png` serves printable ASCII: the `nonlatin_european` and
/// `accented` sheets cover none of it, so there is no provider-priority subtlety here.
#[rustfmt::skip]
const ASCII_ADVANCE: [u8; 95] = [
    4, 2, 4, 6, 6, 6, 6, 2, 4, 4, 4, 6, 2, 6, 2, 6, // ' ' ! " # $ % & ' ( ) * + , - . /
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 2, 2, 5, 6, 5, 6, // 0-9 : ; < = > ?
    7, 6, 6, 6, 6, 6, 6, 6, 6, 4, 6, 6, 6, 6, 6, 6, // @ A-O
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 4, 6, 4, 6, 6, // P-Z [ \ ] ^ _
    3, 6, 6, 6, 6, 6, 5, 6, 6, 2, 6, 5, 3, 6, 6, 6, // ` a-o
    6, 6, 6, 6, 4, 6, 6, 6, 6, 6, 6, 4, 2, 4, 7,    // p-z { | } ~
];

/// Prose punctuation outside ASCII, measured from the same jar.
///
/// These are a trap worth pinning: they sit in General Punctuation, right next to the
/// CJK blocks and heavily used *in* CJK copy, but `default.json` declares the
/// `nonlatin_european` bitmap sheet **before** the `unihex` provider — so they resolve
/// to a bitmap glyph, not to a full-width one. An em dash is 9 px (as wide as a Han
/// glyph); an ellipsis is 8; curly quotes are 5 or 3. Treating them as generic
/// non-ASCII would under-measure every line that uses one, and this campaign's prose
/// uses em dashes constantly.
#[rustfmt::skip]
const PUNCT_ADVANCE: [(char, u32); 8] = [
    ('\u{00B7}', 2), // · middle dot
    ('\u{2013}', 7), // – en dash
    ('\u{2014}', 9), // — em dash
    ('\u{2018}', 3), // ‘ left single quote
    ('\u{2019}', 3), // ’ right single quote
    ('\u{201C}', 5), // “ left double quote
    ('\u{201D}', 5), // ” right double quote
    ('\u{2026}', 8), // … ellipsis
];

/// True for codepoints the font renders at full (double) width: the CJK blocks and
/// their full-width punctuation / forms.
fn is_full_width(ch: char) -> bool {
    let c = ch as u32;
    matches!(c,
        0x1100..=0x115F     // Hangul Jamo
        | 0x2E80..=0x303E   // CJK radicals, Kangxi, CJK symbols & punctuation
        | 0x3041..=0x33FF   // Kana, Hangul Compatibility Jamo, CJK compat
        | 0x3400..=0x4DBF   // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF   // CJK Unified Ideographs
        | 0xA000..=0xA4CF   // Yi
        | 0xAC00..=0xD7A3   // Hangul syllables
        | 0xF900..=0xFAFF   // CJK compatibility ideographs
        | 0xFE30..=0xFE6F   // CJK compatibility forms
        | 0xFF00..=0xFF60   // full-width forms
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD // CJK extensions B+
    )
}

/// The rendered width of `text` in the vanilla **default** font, in font pixels.
pub fn default_font_width(text: &str) -> u32 {
    text.chars()
        .map(|ch| match ch {
            ' ' => SPACE_ADVANCE,
            '\u{21}'..='\u{7E}' => ASCII_ADVANCE[ch as usize - 0x20] as u32,
            _ if is_full_width(ch) => FULL_WIDTH_ADVANCE,
            _ => PUNCT_ADVANCE
                .iter()
                .find(|(c, _)| *c == ch)
                .map_or(TYPICAL_ADVANCE, |(_, a)| *a),
        })
        .sum()
}

/// The rendered width of `text` in the `delve:art` font, in font pixels. Every art
/// glyph advances the same [`ART_GLYPH_ADVANCE`]; the space comes from the font's own
/// `space` provider. Both are the constants that *emit* the font (they derive from
/// `atmos::ART_SCALE`), so the lint and the shipped font cannot drift.
pub fn art_font_width(text: &str) -> u32 {
    text.chars()
        .map(|ch| {
            if ch == ' ' {
                ART_SPACE_ADVANCE as u32
            } else {
                ART_GLYPH_ADVANCE as u32
            }
        })
        .sum()
}

/// The rendered width of `text` in the font `style` renders in, in font pixels.
pub fn width_for(style: NarrateStyle, text: &str) -> u32 {
    match style {
        NarrateStyle::Art => art_font_width(text),
        _ => default_font_width(text),
    }
}

// ---------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------

/// Check every on-screen `narrate` string — the canonical English source **and**
/// every declared-language sidecar rendition — against its style's width budget
/// (`DW0330`). Sidecar findings are reported under the locale and the l10n key, so a
/// `zh-cn` overflow points at the exact string to shorten rather than at its English
/// source. Advisory: see the module docs for why this warns rather than rejects.
pub fn check_text_fits(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let narrates = on_screen_narrates(c);
    for n in &narrates {
        if let Some(diag) = over_budget(n.stage, n.path.clone(), n.style, &n.text) {
            d.push(diag);
        }
    }
    // Translations: only the declared languages, in a fixed order.
    for lang in &c.world.content.languages {
        let Some(doc) = sidecars.get(lang) else {
            continue; // absence is DW0180's job, not ours.
        };
        for n in &narrates {
            if let Some(translated) = doc.content.get(&n.key)
                && let Some(diag) = over_budget(
                    "l10n",
                    format!("l10n/{lang}.json#/content/{}", n.key),
                    n.style,
                    translated,
                )
            {
                d.push(diag);
            }
        }
    }
    d
}

/// The `DW0330` diagnostic for `text` under `style`, or `None` if it fits.
fn over_budget(stage: &str, path: String, style: NarrateStyle, text: &str) -> Option<Diagnostic> {
    let width = width_for(style, text);
    let budget = budget(style);
    if width <= budget {
        return None;
    }
    let scale = style_scale(style);
    let token = style.token();
    Some(Diagnostic::warning(
        DW_TEXT_OVERRUNS_SCREEN,
        stage,
        path,
        format!(
            "`{token}` text renders {width} font px wide, over the {budget} px that fits \
             on screen ({} scaled px at ×{scale} of a {REF_GUI_WIDTH}-px GUI, \
             {SAFE_PERCENT}% usable) — vanilla draws titles centred on ONE line with no \
             wrapping, so the extra {} px runs off both edges. Shorten the line to about \
             {} of its width. Do NOT switch the line to `chat` to silence this if it is \
             meant to be a title, and do NOT assume a wider monitor fixes it — the \
             overflow scales with the player's GUI scale, not away from it. Text: `{text}`",
            width * scale,
            width - budget,
            percent_of(budget, width),
        ),
    ))
}

/// `budget/width` as a rounded percentage, for the remediation hint.
fn percent_of(budget: u32, width: u32) -> String {
    format!("{}%", budget * 100 / width)
}

// ---------------------------------------------------------------------------
// Dialogue option labels (DW0331)
// ---------------------------------------------------------------------------

/// Check every dialogue option label — the canonical English source **and** every
/// declared-language sidecar rendition — against the dialog button it is drawn on
/// (`DW0331`). Sidecar findings name the locale and the l10n key, so a `zh-cn` label
/// that overflows where its English source fits points at the string to shorten.
///
/// Error tier: see the module docs. The budget is the widget's geometry, not a guess
/// about the player's screen.
pub fn check_option_labels(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    // spec-0016 §1 (owner ruling 2026-08-03): a bonfire's two rest options are
    // drawn on exactly the same 150-GUI-px `multi_action` button, so they carry
    // exactly the same budget. The check follows the widget, not the stage the
    // string happened to be authored in.
    let labels: Vec<OptionLabel> = dialogue_option_labels(c)
        .into_iter()
        .chain(bonfire_option_labels(c))
        .collect();
    for l in &labels {
        if let Some(diag) = label_over_budget(l.stage, l.path.clone(), &l.text) {
            d.push(diag);
        }
    }
    // Translations: only the declared languages, in a fixed order.
    for lang in &c.world.content.languages {
        let Some(doc) = sidecars.get(lang) else {
            continue; // absence is DW0180's job, not ours.
        };
        for l in &labels {
            if let Some(translated) = doc.content.get(&l.key)
                && let Some(diag) = label_over_budget(
                    "l10n",
                    format!("l10n/{lang}.json#/content/{}", l.key),
                    translated,
                )
            {
                d.push(diag);
            }
        }
    }
    d
}

/// The `DW0331` diagnostic for an option `label`, or `None` if it fits its button.
fn label_over_budget(stage: &str, path: String, text: &str) -> Option<Diagnostic> {
    let width = default_font_width(text);
    if width <= BUTTON_LABEL_BUDGET {
        return None;
    }
    Some(Diagnostic::error(
        DW_OPTION_LABEL_SCROLLS,
        stage,
        path,
        format!(
            "dialogue option label renders {width} font px wide, over the \
             {BUTTON_LABEL_BUDGET} px a dialog button fits ({DIALOG_BUTTON_WIDTH} GUI px, \
             vanilla's default since the compiler sets no `width`, less \
             {BUTTON_LABEL_INSET} px of inset each side; buttons draw at pose scale ×1, \
             so a font px is a GUI px). Vanilla neither wraps nor shrinks an over-wide \
             caption — it SCROLLS it back and forth, on every window at every GUI scale, \
             and a shelf of sliding captions is unreadable to choose from. An option is a \
             button caption, not a sentence: cut it to about {} of its width (roughly \
             {BUTTON_LABEL_BUDGET} px ≈ 24 Latin or 16 Han characters, so author to ~20 / \
             ~12 to leave a translation room to grow) and move what it was carrying into \
             the node's body text, which wraps, or into the NPC's reply. Label: `{text}`",
            percent_of(BUTTON_LABEL_BUDGET, width),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vanilla ASCII advances the table encodes: the common letter is 6 px, and
    /// the narrow glyphs are genuinely narrower (a character count would call `iii`
    /// and `WWW` the same size — they are 6 px and 18 px).
    #[test]
    fn ascii_advances_match_the_vanilla_font() {
        assert_eq!(default_font_width("A"), 6);
        assert_eq!(default_font_width("i"), 2);
        assert_eq!(default_font_width("l"), 3);
        assert_eq!(default_font_width(" "), 4);
        assert_eq!(default_font_width("iii"), 6);
        assert_eq!(default_font_width("WWW"), 18);
    }

    /// A Han glyph is 9 px against a Latin letter's 6 — 1.5×, not the 2× a naive
    /// "count CJK double" rule assumes.
    #[test]
    fn han_glyphs_are_one_and_a_half_latin_letters() {
        assert_eq!(default_font_width("无"), FULL_WIDTH_ADVANCE);
        assert_eq!(default_font_width("无人之洞"), 4 * FULL_WIDTH_ADVANCE);
        assert_eq!(default_font_width("，"), FULL_WIDTH_ADVANCE);
        assert_eq!(default_font_width("AA"), 12);
        // 1.5×, exactly: three Han glyphs measure the same as four ASCII letters.
        assert_eq!(default_font_width("无人之"), default_font_width("AAAA") + 3);
    }

    /// Prose punctuation that sits beside the CJK blocks still resolves to the
    /// `nonlatin_european` bitmap sheet, which `default.json` declares **before**
    /// `unihex` — so an em dash is a full 9 px, not a generic non-ASCII glyph. The
    /// campaign's prose is full of them; getting this wrong under-measures every line
    /// that uses one.
    #[test]
    fn prose_punctuation_uses_its_measured_bitmap_advance() {
        assert_eq!(default_font_width("—"), 9);
        assert_eq!(default_font_width("–"), 7);
        assert_eq!(default_font_width("…"), 8);
        assert_eq!(default_font_width("“”"), 10);
        assert_eq!(default_font_width("‘’"), 6);
        assert_eq!(default_font_width("·"), 2);
        // An em dash is as wide as a Han glyph and 4.5× an `i`.
        assert_eq!(default_font_width("—"), default_font_width("无"));
    }

    /// The art font's advance is derived from the constants that emit the font, so a
    /// glyph-size change cannot leave the budget stale.
    #[test]
    fn art_glyphs_are_derived_from_the_font_it_emits() {
        assert_eq!(art_font_width("A"), ART_GLYPH_ADVANCE as u32);
        assert_eq!(art_font_width("A"), 6);
        assert_eq!(art_font_width("A A"), 6 + 4 + 6);
    }

    /// The reason `ART_SCALE` is 1: the ×4 title pose leaves 90 font px, and an
    /// ending banner needs at least a dozen glyphs in it. This is the acceptance the
    /// scale was chosen against — if the font grows again, this fails first.
    #[test]
    fn the_art_budget_fits_a_real_banner() {
        let fits = budget(NarrateStyle::Art) / ART_GLYPH_ADVANCE as u32;
        assert_eq!(fits, 15, "art fits 15 glyphs at the ×4 title pose");
        assert!(fits >= 12, "an ending banner needs ≥12 glyphs");
        // The strings the owner hit on screen, both renditions of each.
        assert!(art_font_width("NOBODY") <= budget(NarrateStyle::Art));
        assert!(art_font_width("HOMEWARD") <= budget(NarrateStyle::Art));
        // …and the check still bites past the budget.
        assert!(art_font_width("HOMEWARD BOUND AGAIN") > budget(NarrateStyle::Art));
    }

    /// The budgets follow from the reference GUI width and the vanilla pose scales.
    #[test]
    fn budgets_follow_the_vanilla_pose_scales() {
        assert_eq!(budget(NarrateStyle::Title), 90);
        assert_eq!(budget(NarrateStyle::Subtitle), 181);
        assert_eq!(budget(NarrateStyle::Art), 90);
        // A subtitle gets twice a title's budget because it renders at half the scale.
        assert_eq!(
            budget(NarrateStyle::Subtitle),
            2 * budget(NarrateStyle::Title) + 1
        );
    }
}
