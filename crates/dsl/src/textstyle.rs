//! Inline text styles (spec-0096): a span of any player-facing line carries a
//! vanilla text style.
//!
//! A span is written inside the line as `[[<styles>|<text>]]`, where `<styles>`
//! is one or more of `obfuscated`, `bold`, `italic`, `underlined`,
//! `strikethrough` and `color=<colour>`, comma-separated, each at most once.
//! A doubled square bracket occurs in no prose and in no other DSL surface, so a
//! line carrying one is markup or a defect, never ambiguous; a single bracket is
//! ordinary prose.
//!
//! The span belongs to the **player-facing string**, the object class
//! [`crate::l10n::each_string`] defines, so this module is the one rule every
//! reader of such a string shares:
//!
//! - [`parse`] — the grammar, refused by [`validate_inline_styles`] (`DW0975`);
//! - [`visible`] — what a reader that does not draw the line sees;
//! - [`lower`] / [`lower_aligned`] — the format string and span list the emitter
//!   writes as a `translate` component with `with` arguments, and the language
//!   files write under `<key>` and `<key>.span.<i>`;
//! - [`validate_inline_styles`] — the English and every sidecar, and a
//!   translation whose spans are not the English's (`DW0976`).

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::l10n::{L10nDoc, inventory};

/// Opens a span.
pub const OPEN: &str = "[[";
/// Closes a span.
pub const CLOSE: &str = "]]";
/// Separates a span's styles from its text.
pub const SEP: char = '|';

/// The five vanilla text-component flags, in the order a signature lists them.
pub const FLAGS: [&str; 5] = [
    "bold",
    "italic",
    "underlined",
    "strikethrough",
    "obfuscated",
];

/// The sixteen named text colours of the pinned game (Minecraft Wiki, *Text
/// component format*: `color`).
pub const NAMED_COLORS: [&str; 16] = [
    "black",
    "dark_blue",
    "dark_green",
    "dark_aqua",
    "dark_red",
    "dark_purple",
    "gold",
    "gray",
    "dark_gray",
    "blue",
    "green",
    "aqua",
    "red",
    "light_purple",
    "yellow",
    "white",
];

/// The suffix the language files key a span's own text under:
/// `<line key>.span.<index>`.
pub fn span_key(line_key: &str, index: usize) -> String {
    format!("{line_key}.span.{index}")
}

/// The style a span sets on top of the line it sits in.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpanStyle {
    /// The flags set, in [`FLAGS`] order.
    pub flags: Vec<&'static str>,
    /// `color`, lower-case: a vanilla name or `#rrggbb`.
    pub color: Option<String>,
}

impl SpanStyle {
    /// The component keys this style sets, as `(key, value)`.
    pub fn fields(&self) -> Vec<(&'static str, serde_json::Value)> {
        let mut out: Vec<(&'static str, serde_json::Value)> = self
            .flags
            .iter()
            .map(|f| (*f, serde_json::Value::Bool(true)))
            .collect();
        if let Some(c) = &self.color {
            out.push(("color", serde_json::Value::String(c.clone())));
        }
        out
    }

    /// The canonical spelling of this style: flags in [`FLAGS`] order, then
    /// `color=<c>`. Two spans have the same style iff their signatures are equal.
    pub fn signature(&self) -> String {
        let mut parts: Vec<String> = self.flags.iter().map(|f| (*f).to_string()).collect();
        if let Some(c) = &self.color {
            parts.push(format!("color={c}"));
        }
        parts.join(",")
    }
}

/// One piece of a parsed line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    /// Unstyled prose.
    Text(&'a str),
    /// A styled span.
    Span(SpanStyle, &'a str),
}

/// Why a line's markup does not parse: the character offset and what was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkupError {
    /// Character offset into the line.
    pub at: usize,
    /// What is wrong, in a clause.
    pub what: String,
}

fn char_at(s: &str, byte: usize) -> usize {
    s[..byte].chars().count()
}

/// Whether `s` contains any markup delimiter at all — the fast path every reader
/// takes for the common line with no span.
pub fn has_markup(s: &str) -> bool {
    s.contains(OPEN) || s.contains(CLOSE)
}

fn parse_styles(spec: &str) -> Result<SpanStyle, String> {
    if spec.is_empty() {
        return Err("a span names no style before its `|`".to_string());
    }
    let mut style = SpanStyle::default();
    for part in spec.split(',') {
        if let Some(c) = part.strip_prefix("color=") {
            if style.color.is_some() {
                return Err("`color` is written twice in one span".to_string());
            }
            let lower = c.to_ascii_lowercase();
            if !(NAMED_COLORS.contains(&c) || crate::color::is_hex(c)) {
                return Err(format!(
                    "`{c}` is not a text colour: write one of {} or `#rrggbb`",
                    NAMED_COLORS.join(", ")
                ));
            }
            style.color = Some(lower);
        } else if let Some(f) = FLAGS.iter().find(|f| **f == part) {
            if style.flags.contains(f) {
                return Err(format!("`{part}` is written twice in one span"));
            }
            style.flags.push(f);
        } else {
            return Err(format!(
                "`{part}` is not a style: write {} or `color=<colour>`, comma-separated, \
                 with no spaces",
                FLAGS.join(", ")
            ));
        }
    }
    style
        .flags
        .sort_by_key(|f| FLAGS.iter().position(|g| g == f));
    Ok(style)
}

/// Parse `s` into its segments (spec-0096 §3.1). A line with no markup is one
/// [`Segment::Text`] (or none, if empty).
pub fn parse(s: &str) -> Result<Vec<Segment<'_>>, MarkupError> {
    let mut out = Vec::new();
    let mut rest = 0usize;
    loop {
        let tail = &s[rest..];
        let open = tail.find(OPEN);
        let close = tail.find(CLOSE);
        match (open, close) {
            (None, None) => {
                if !tail.is_empty() {
                    out.push(Segment::Text(tail));
                }
                return Ok(out);
            }
            (o, Some(c)) if o.is_none_or(|o| c < o) => {
                return Err(MarkupError {
                    at: char_at(s, rest + c),
                    what: "`]]` closes no span".to_string(),
                });
            }
            (Some(o), _) => {
                if o > 0 {
                    out.push(Segment::Text(&tail[..o]));
                }
                let start = rest + o;
                let body_at = start + OPEN.len();
                let body = &s[body_at..];
                let Some(end) = body.find(CLOSE) else {
                    return Err(MarkupError {
                        at: char_at(s, start),
                        what: "`[[` opens a span that is never closed with `]]`".to_string(),
                    });
                };
                let inner = &body[..end];
                if let Some(nested) = inner.find(OPEN) {
                    return Err(MarkupError {
                        at: char_at(s, body_at + nested),
                        what: "`[[` inside a span: spans do not nest".to_string(),
                    });
                }
                let Some(bar) = inner.find(SEP) else {
                    return Err(MarkupError {
                        at: char_at(s, start),
                        what: "a span has no `|` between its styles and its text".to_string(),
                    });
                };
                let style = parse_styles(&inner[..bar]).map_err(|what| MarkupError {
                    at: char_at(s, body_at),
                    what,
                })?;
                let text = &inner[bar + 1..];
                if text.trim().is_empty() {
                    return Err(MarkupError {
                        at: char_at(s, body_at + bar + 1),
                        what: "a span's text is empty".to_string(),
                    });
                }
                out.push(Segment::Span(style, text));
                rest = body_at + end + CLOSE.len();
            }
            (None, Some(_)) => unreachable!("covered by the close-first arm"),
        }
    }
}

/// The text a reader sees when it does not draw the line: each span replaced by
/// its text. A line that does not parse is returned unchanged (`DW0975` is the
/// refusal; a reader is not where it is raised).
pub fn visible(s: &str) -> Cow<'_, str> {
    if !has_markup(s) {
        return Cow::Borrowed(s);
    }
    match parse(s) {
        Ok(segs) => Cow::Owned(
            segs.iter()
                .map(|g| match g {
                    Segment::Text(t) | Segment::Span(_, t) => *t,
                })
                .collect(),
        ),
        Err(_) => Cow::Borrowed(s),
    }
}

/// The span signatures of `s`, sorted — the multiset `DW0976` compares. Empty
/// for a line with no span or one that does not parse.
pub fn signatures(s: &str) -> Vec<String> {
    let mut out: Vec<String> = parse(s)
        .map(|segs| {
            segs.iter()
                .filter_map(|g| match g {
                    Segment::Span(st, _) => Some(st.signature()),
                    Segment::Text(_) => None,
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// A styled line lowered for a `translate` component (spec-0096 §3.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lowered {
    /// The line with span `i` replaced by `%<i+1>$s` and every other `%`
    /// doubled: the component's `fallback` and the language file's value.
    pub format: String,
    /// Each span's style and its text (every `%` doubled), by index: the
    /// `with` arguments, and the language file's `<key>.span.<i>` values.
    pub spans: Vec<(SpanStyle, String)>,
}

fn escape_percent(s: &str) -> String {
    s.replace('%', "%%")
}

/// Lower `s`, its spans indexed in its own order. `None` for a line with no
/// span, or one that does not parse (validation refuses it before emission).
pub fn lower(s: &str) -> Option<Lowered> {
    if !has_markup(s) {
        return None;
    }
    let segs = parse(s).ok()?;
    let order: Vec<usize> = (0..segs.len()).collect();
    lower_in(&segs, &order)
}

fn lower_in(segs: &[Segment<'_>], index_of: &[usize]) -> Option<Lowered> {
    let n = segs
        .iter()
        .filter(|g| matches!(g, Segment::Span(..)))
        .count();
    if n == 0 {
        return None;
    }
    let mut format = String::new();
    let mut spans: Vec<Option<(SpanStyle, String)>> = vec![None; n];
    let mut k = 0usize;
    for g in segs {
        match g {
            Segment::Text(t) => format.push_str(&escape_percent(t)),
            Segment::Span(st, t) => {
                let i = index_of[k];
                format.push_str(&format!("%{}$s", i + 1));
                spans[i] = Some((st.clone(), escape_percent(t)));
                k += 1;
            }
        }
    }
    Some(Lowered {
        format,
        spans: spans
            .into_iter()
            .map(|s| s.expect("every index filled"))
            .collect(),
    })
}

/// Lower a translation `translated` with each span written under the index of
/// the English span it answers (spec-0096 §3.3): spans are matched by style, in
/// order of appearance among spans of that style. `Ok(None)` when neither line
/// carries a span; `Err` (the `DW0976` reason) when the two span multisets
/// differ or either line does not parse.
pub fn lower_aligned(english: &str, translated: &str) -> Result<Option<Lowered>, String> {
    let en = parse(english).map_err(|e| format!("the English does not parse: {}", e.what))?;
    let tr =
        parse(translated).map_err(|e| format!("the translation does not parse: {}", e.what))?;
    let en_sig: Vec<String> = en
        .iter()
        .filter_map(|g| match g {
            Segment::Span(st, _) => Some(st.signature()),
            Segment::Text(_) => None,
        })
        .collect();
    let tr_sig: Vec<String> = tr
        .iter()
        .filter_map(|g| match g {
            Segment::Span(st, _) => Some(st.signature()),
            Segment::Text(_) => None,
        })
        .collect();
    let (mut a, mut b) = (en_sig.clone(), tr_sig.clone());
    a.sort();
    b.sort();
    if a != b {
        return Err(format!(
            "the English carries span(s) [{}] and the translation [{}]",
            en_sig.join("; "),
            tr_sig.join("; ")
        ));
    }
    if en_sig.is_empty() {
        return Ok(None);
    }
    // The k-th span of a style in the translation answers the k-th span of that
    // style in the English.
    let mut taken: BTreeMap<&str, usize> = BTreeMap::new();
    let mut index_of = Vec::with_capacity(tr_sig.len());
    for sig in &tr_sig {
        let nth = taken.entry(sig.as_str()).or_insert(0);
        let i = en_sig
            .iter()
            .enumerate()
            .filter(|(_, s)| *s == sig)
            .nth(*nth)
            .map(|(i, _)| i)
            .expect("equal multisets");
        *nth += 1;
        index_of.push(i);
    }
    Ok(lower_in(&tr, &index_of))
}

/// What [`validate_inline_styles`] examined, for the binding line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineStyleBinding {
    /// English lines in the inventory.
    pub lines: usize,
    /// English lines carrying at least one span.
    pub styled: usize,
    /// Spans across the English.
    pub spans: usize,
    /// Sidecar rows checked against their English.
    pub sidecar_rows: usize,
}

impl InlineStyleBinding {
    /// Count what the checks bind over `c` and `sidecars`.
    pub fn of(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Self {
        let inv = inventory(c);
        let mut b = InlineStyleBinding {
            lines: inv.len(),
            ..Default::default()
        };
        for v in inv.values() {
            let n = signatures(v).len();
            if n > 0 {
                b.styled += 1;
                b.spans += n;
            }
        }
        b.sidecar_rows = sidecars
            .values()
            .map(|d| d.content.keys().filter(|k| inv.contains_key(*k)).count())
            .sum();
        b
    }

    /// The one-line binding statement `delvec validate` prints.
    pub fn line(&self) -> String {
        format!(
            "inline-style binding: {} of {} player-facing line(s) carry {} span(s); {} sidecar \
             row(s) held to their English's spans",
            self.styled, self.lines, self.spans, self.sidecar_rows
        )
    }
}

const GRAMMAR: &str = "a span is `[[<styles>|<text>]]`, <styles> one or more of bold, italic, \
     underlined, strikethrough, obfuscated, color=<name or #rrggbb>, comma-separated with no \
     spaces";

/// `DW0975` over every English line and every sidecar row, and `DW0976` over
/// every sidecar row whose spans are not its English's (spec-0096 §4).
pub fn validate_inline_styles(
    c: &Campaign,
    sidecars: &BTreeMap<String, L10nDoc>,
) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let inv = inventory(c);
    let malformed = |stage: &str, path: String, key: &str, text: &str, e: MarkupError| {
        Diagnostic::error(
            codes::INLINE_STYLE_MALFORMED,
            stage,
            path,
            format!(
                "player-visible string `{key}` has malformed style markup at character {}: {} \
                 — {GRAMMAR}. Correct the span, or remove the doubled brackets. Text: `{text}`",
                e.at, e.what
            ),
        )
    };
    for (key, text) in &inv {
        if !has_markup(text) {
            continue;
        }
        if let Err(e) = parse(text) {
            d.push(malformed("l10n", format!("#/{key}"), key, text, e));
        }
    }
    for (lang, doc) in sidecars {
        for (key, text) in &doc.content {
            let path = format!("l10n/{lang}.json#/content/{key}");
            if has_markup(text)
                && let Err(e) = parse(text)
            {
                d.push(malformed("l10n", path, key, text, e));
                continue;
            }
            let Some(en) = inv.get(key) else {
                continue;
            };
            if parse(en).is_err() {
                continue;
            }
            if !has_markup(text) && !has_markup(en) {
                continue;
            }
            if let Err(why) = lower_aligned(en, text) {
                d.push(Diagnostic::error(
                    codes::INLINE_STYLE_UNMATCHED,
                    "l10n",
                    path,
                    format!(
                        "`{lang}` row `{key}` does not carry the English's styled spans: {why}. \
                         Keep every span of the English with its style unchanged and its text \
                         transcreated; it may move within the line. English: `{en}`; \
                         translation: `{text}`"
                    ),
                ));
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    const BARK: &str =
        "The ledger is kept by [[obfuscated|someone else]] at [[italic,color=dark_purple|night]].";

    #[test]
    fn a_line_with_no_markup_is_one_text_segment() {
        assert_eq!(
            parse("Plain words.").unwrap(),
            vec![Segment::Text("Plain words.")]
        );
        assert_eq!(parse("a [bracket] or two").unwrap().len(), 1);
        assert!(parse("").unwrap().is_empty());
        assert_eq!(lower("Plain words."), None);
    }

    #[test]
    fn every_style_key_parses() {
        for f in FLAGS {
            let line = format!("x [[{f}|y]] z");
            let segs = parse(&line).unwrap();
            assert_eq!(
                segs[1],
                Segment::Span(
                    SpanStyle {
                        flags: vec![f],
                        color: None
                    },
                    "y"
                )
            );
        }
        for c in NAMED_COLORS {
            let line = format!("[[color={c}|y]]");
            assert_eq!(signatures(&line), vec![format!("color={c}")]);
        }
        assert_eq!(
            signatures("[[color=#AbCdEf|y]]"),
            vec!["color=#abcdef".to_string()]
        );
        assert_eq!(
            signatures("[[obfuscated,bold,color=red|y]]"),
            vec!["bold,obfuscated,color=red".to_string()]
        );
    }

    #[test]
    fn each_refusal_shape_names_its_offset() {
        for (line, at, needle) in [
            ("ab [[bold|c", 3, "never closed"),
            ("ab ]] c", 3, "closes no span"),
            ("[[bold c]]", 0, "no `|`"),
            ("[[|c]]", 2, "names no style"),
            ("[[shiny|c]]", 2, "not a style"),
            ("[[bold,bold|c]]", 2, "twice"),
            ("[[color=crimson|c]]", 2, "not a text colour"),
            ("[[color=red,color=blue|c]]", 2, "twice"),
            ("[[bold|  ]]", 7, "empty"),
            ("[[bold|a [[b]]", 9, "do not nest"),
            ("[[bold, italic|c]]", 2, "not a style"),
        ] {
            let e = parse(line).expect_err(line);
            assert_eq!(e.at, at, "{line}: {}", e.what);
            assert!(e.what.contains(needle), "{line}: {}", e.what);
        }
    }

    #[test]
    fn the_visible_text_drops_the_markup() {
        assert_eq!(
            visible(BARK),
            "The ledger is kept by someone else at night."
        );
        assert_eq!(visible("no spans"), "no spans");
        assert_eq!(visible("[[bold|broken"), "[[bold|broken");
    }

    #[test]
    fn lowering_numbers_the_spans_and_escapes_percent() {
        let l = lower("A 50% [[bold|chance]] of [[obfuscated|100%]].").unwrap();
        assert_eq!(l.format, "A 50%% %1$s of %2$s.");
        assert_eq!(l.spans[0].1, "chance");
        assert_eq!(l.spans[1].1, "100%%");
        assert_eq!(l.spans[1].0.signature(), "obfuscated");
    }

    #[test]
    fn a_translation_is_aligned_to_the_english_by_style() {
        let zh = "[[italic,color=dark_purple|夜里]]，账本由[[obfuscated|别的什么人]]记着。";
        let l = lower_aligned(BARK, zh).unwrap().unwrap();
        assert_eq!(l.format, "%2$s，账本由%1$s记着。");
        assert_eq!(l.spans[0].1, "别的什么人");
        assert_eq!(l.spans[0].0.signature(), "obfuscated");
        assert_eq!(l.spans[1].1, "夜里");
        // Same-style spans keep their relative order.
        let l = lower_aligned("[[bold|a]] [[bold|b]]", "[[bold|甲]][[bold|乙]]")
            .unwrap()
            .unwrap();
        assert_eq!(l.format, "%1$s%2$s");
    }

    #[test]
    fn a_translation_with_other_spans_is_refused() {
        assert!(lower_aligned(BARK, "账本由别的什么人记着。").is_err());
        assert!(lower_aligned(BARK, "[[bold|夜里]]，账本由[[obfuscated|某人]]记着。").is_err());
        assert!(lower_aligned("plain", "[[bold|加粗]]").is_err());
        assert_eq!(lower_aligned("plain", "平"), Ok(None));
    }
}
