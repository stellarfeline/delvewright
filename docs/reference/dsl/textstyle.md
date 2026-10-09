# `delvewright_dsl::textstyle`

The reference page for `crates/dsl/src/textstyle.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0975/DW0976 — a span of text carries a style (`dsl::textstyle`; error; exit 1)

The surface, its lowering and its language-file rows are in [`compiler.md`, *Inline styles*](../compiler.md#inline-styles--a-span-of-a-line-carries-a-style-spec-0096). Both codes are validation-tier and read every player-facing line the l10n inventory holds and every sidecar row.

| Code | Meaning |
|------|---------|
| `DW0975` | (spec-0096) **Style markup that does not parse.** A player-visible string — authored English (every inventory row) or any sidecar row — carries a `[[` or `]]` that is not one well-formed span `[[<styles>|<text>]]`: an unclosed `[[`, a `]]` closing nothing, no `|`, no style, an unknown or repeated style, a colour neither a vanilla name nor `#rrggbb`, blank text, or a `[[` inside a span. `dsl::textstyle::validate_inline_styles`. The message names the key, the character offset and the grammar. Prescription: correct the span, or remove the doubled brackets. |
| `DW0976` | (spec-0096) **A translation whose styled spans are not the English's.** A sidecar row drops, adds or restyles a span of its English line (the multisets of span styles differ; `dsl::textstyle::lower_aligned`). The style rides on the component, so a translation only places and words each span. Raised at validate over every sidecar row whose English and translation both parse; the build's language-file writer (`emit::server::styled_rows`) re-proves the alignment and refuses with the same code. Prescription: keep every span of the English with its style unchanged and its text transcreated; it may move within the line. |
