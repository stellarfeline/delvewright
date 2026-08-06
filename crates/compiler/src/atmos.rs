//! Sound + art-title surface (DSL v0.6, spec-0014): sound-event validation
//! (`DW0326`), the deferred `play-sound at: actor` gate (`DW0335`), and the
//! pixel-banner "art" title font (`delve:art`) with its compile-time glyph-coverage
//! check (`DW0328`).
//!
//! ## Art font provenance / license
//!
//! `art.png` is an **original** 5×7 pixel font authored for Delvewright (the
//! [`ART_GLYPHS`] table below). It is not derived from any third-party font, so it
//! carries the repository's own license (GPL-3.0) and needs no `ACKNOWLEDGEMENTS`
//! entry — OFL and other non-ADR-0013 licenses are avoided entirely by authoring
//! the glyphs here. The PNG is generated deterministically (a hand-rolled encoder,
//! like `resourcepack`'s hand-rolled ZIP/SHA-1), so the resource pack stays
//! byte-identical across builds (ADR-0006).
//!
//! ## Covered glyph set (explicit)
//!
//! Space (via a `space` provider) plus the 48 bitmap glyphs: `A`–`Z`, `0`–`9`, and
//! the punctuation `! " ' ( ) , - . / : ; ?`. Lowercase input is rendered through
//! the uppercase glyphs (art titles are uppercase). Any other character — accented
//! Latin, CJK, emoji — is **uncovered** and rejected at compile time (`DW0328`).
//! This is what forces per-language art titles to stay ASCII/Latin: a `zh-cn`
//! sidecar translation of an art-styled `narrate` must itself be an ASCII rendition
//! (romanization or a Latin equivalent), or the build fails.

use std::collections::BTreeMap;

use delvewright_dsl::{
    Campaign, Diagnostic, L10nDoc, art_narrates, play_sound_actor_refs, sound_refs,
};

use crate::registry::FullSoundRegistry;

/// `DW0326`: a `play-sound` / `narrate.sound` id is not a known 1.21.11 sound
/// event (validated against the vendored `sound_event` registry).
pub const DW_SOUND_UNKNOWN: &str = "DW0326";
/// `DW0328`: an art-styled `narrate` string (source or a sidecar translation) uses
/// a character outside the `delve:art` font's glyph inventory.
pub const DW_ART_GLYPH_UNCOVERED: &str = "DW0328";
/// `DW0335`: a `play-sound` targets `at: actor`, which is accepted by the schema
/// but not yet wired — the actors surface (spec-0014 `actors[]`) has not landed.
pub const DW_PLAYSOUND_ACTOR_DEFERRED: &str = "DW0335";

// ---------------------------------------------------------------------------
// Sound-event validation (DW0326) + deferred actor gate (DW0335)
// ---------------------------------------------------------------------------

/// Validate every referenced sound-event id against the pinned 1.21.11 registry
/// (`DW0326`), and reject the deferred `play-sound at: actor` target (`DW0335`).
/// Runs at validate-time (exit 1) so bad sounds never reach a build.
pub fn check_sounds(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let reg = FullSoundRegistry::v1_21_11();
    for r in sound_refs(c) {
        if !reg.contains(&r.sound) {
            d.push(Diagnostic::error(
                DW_SOUND_UNKNOWN,
                r.stage,
                r.path,
                format!(
                    "sound `{}` is not a known 1.21.11 sound event — use a vanilla \
                     `minecraft:` sound-event id (validated against the vendored \
                     `sound_event` registry)",
                    r.sound
                ),
            ));
        }
    }
    for r in play_sound_actor_refs(c) {
        d.push(Diagnostic::error(
            DW_PLAYSOUND_ACTOR_DEFERRED,
            r.stage,
            r.path,
            format!(
                "`play-sound` `at: {{actor: {}}}` is not yet supported — the actors \
                 surface (spec-0014 `actors[]`) has not landed. Use `at: {{anchor: …}}` \
                 or `at: players` for now",
                r.sound
            ),
        ));
    }
    d
}

// ---------------------------------------------------------------------------
// Art-title glyph coverage (DW0328)
// ---------------------------------------------------------------------------

/// True if the art font can render `ch`: a space, or (case-folded to uppercase) a
/// character with a bitmap glyph in [`ART_GLYPHS`].
pub fn covers(ch: char) -> bool {
    ch == ' ' || glyph_for(ch).is_some()
}

fn glyph_for(ch: char) -> Option<&'static [&'static str; 7]> {
    let up = ch.to_ascii_uppercase();
    ART_GLYPHS.iter().find(|(c, _)| *c == up).map(|(_, g)| g)
}

/// The first character of `text` the art font cannot render, if any.
fn first_uncovered(text: &str) -> Option<char> {
    text.chars().find(|&ch| !covers(ch))
}

/// Validate that every art-styled `narrate` string — the English source and every
/// declared-language sidecar translation — renders in the `delve:art` font
/// (`DW0328`). Art narrate text is inventoried like other narrate text, so its
/// translations live in the sidecars; a `zh-cn` string that cannot render must be
/// authored as an ASCII/Latin rendition or this fails.
pub fn check_art(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let arts = art_narrates(c);
    for a in &arts {
        // Source (canonical English).
        if let Some(bad) = first_uncovered(&a.text) {
            d.push(art_diag(a.stage, a.path.clone(), &a.text, bad));
        }
    }
    // Translations: only the declared languages, in a fixed order.
    for lang in &c.world.content.languages {
        let Some(doc) = sidecars.get(lang) else {
            continue; // absence is DW0180's job, not ours.
        };
        for a in &arts {
            if let Some(translated) = doc.content.get(&a.key)
                && let Some(bad) = first_uncovered(translated)
            {
                d.push(art_diag(
                    "l10n",
                    format!("l10n/{lang}.json#/content/{}", a.key),
                    translated,
                    bad,
                ));
            }
        }
    }
    d
}

fn art_diag(stage: &str, path: String, text: &str, bad: char) -> Diagnostic {
    Diagnostic::error(
        DW_ART_GLYPH_UNCOVERED,
        stage,
        path,
        format!(
            "art-title text `{text}` contains `{bad}` (U+{:04X}), which the `delve:art` \
             font cannot render — the art font covers only A–Z, 0–9, space, and \
             `! \" ' ( ) , - . / : ; ?`. Keep art titles ASCII/Latin (uppercase); for a \
             non-Latin language, the sidecar must supply an ASCII rendition",
            bad as u32
        ),
    )
}

// ---------------------------------------------------------------------------
// The delve:art resource-pack font (bitmap provider) — original asset
// ---------------------------------------------------------------------------

/// The bitmap-atlas grid width (glyphs per row). 48 glyphs → 6 rows of 8.
const COLS: usize = 8;
/// Per-glyph cell size in source pixels (5×7 glyph in the top-left of an 8×8 cell).
pub const CELL: usize = 8;
/// Glyph bitmap dimensions.
pub const GW: usize = 5;
const GH: usize = 7;

/// The integer factor the bitmap provider renders each source pixel at.
///
/// A vanilla bitmap provider scales its atlas by `height / cellHeight`, so this is
/// the one knob that sets how physically large an art title draws. It must stay an
/// **integer**: the font atlas is sampled nearest-neighbour, so a fractional factor
/// splits a source pixel across screen pixels and the glyph edges go ragged.
///
/// It is **1** (source size, 1 texel = 1 font px). It was 4 through v0.6, which drew
/// a 21-font-px advance per glyph — and since an art `narrate` renders in the vanilla
/// **title** slot, at that slot's ×4 pose scale, the two multiplied: `budget(Art)` is
/// 90 font px, so only *four* glyphs fit and every real banner ran off both edges
/// (owner-confirmed on screen, QA round 5: `NOBODY` at 126 px, `HOMEWARD` at 168 px).
/// Halving to 2 was not enough — 11 px/glyph fits 8, which `HOMEWARD` exactly
/// exhausts. 1 is the largest integer factor that fits **15** glyphs, clearing the
/// ≥12 an ending banner needs with headroom. The glyph is still drawn ×4 by the title
/// pose, so it reads as a title-sized blocky banner, not as body text; only its
/// oversize relative to the slot is gone.
pub const ART_SCALE: usize = 1;

/// The `delve:art` bitmap provider's rendered glyph height in font pixels, and the
/// baseline offset. Both derive from [`ART_SCALE`] so the provider JSON, the atlas
/// and [`crate::textfit`]'s width model cannot drift apart.
pub const ART_HEIGHT: usize = CELL * ART_SCALE;
/// The baseline offset: the glyph ink is [`GH`] of the [`CELL`] rows, sitting on the
/// baseline, so the ascent scales with the glyph.
const ART_ASCENT: usize = GH * ART_SCALE;

/// The advance a single art glyph occupies, in font pixels.
///
/// Vanilla's `BitmapProvider` derives a glyph advance as
/// `round(inkWidth * height / cellHeight) + 1`. Every letter and digit in
/// [`ART_GLYPHS`] inks the full [`GW`] columns, so this is exact for them; the few
/// narrow punctuation glyphs (`'`, `(`, `!`) ink fewer columns and really advance
/// less, which makes this model **conservative** — it never under-measures a line.
/// This is the single source both the font emission and `DW0330` read.
pub const ART_GLYPH_ADVANCE: usize = GW * ART_SCALE + 1;

/// The advance the art font's `space` provider gives a space, in font pixels — the
/// vanilla default font's 4 px, scaled with the glyphs so word gaps stay proportional.
pub const ART_SPACE_ADVANCE: usize = 4 * ART_SCALE;

/// True if the campaign uses the art title style anywhere (so the font is only
/// baked into the pack when needed — a non-art campaign's pack is byte-identical).
pub fn uses_art(c: &Campaign) -> bool {
    !art_narrates(c).is_empty()
}

/// The `delve:art` font assets keyed by resource-pack archive path: the bitmap
/// provider definition and the atlas PNG. Deterministic.
pub fn art_font_assets() -> BTreeMap<String, Vec<u8>> {
    let mut m = BTreeMap::new();
    m.insert("assets/delve/font/art.json".to_string(), font_json());
    m.insert(
        "assets/delve/textures/font/art.png".to_string(),
        atlas_png(),
    );
    m
}

/// The bitmap font provider JSON. `chars` rows are derived from [`ART_GLYPHS`] order
/// so the atlas and the char map cannot drift.
fn font_json() -> Vec<u8> {
    let chars: Vec<String> = ART_GLYPHS
        .chunks(COLS)
        .map(|row| row.iter().map(|(c, _)| *c).collect::<String>())
        .collect();
    // i18n v2 (spec-0029): the same atlas, addressed a second time by the
    // LOWERCASE letters. `emit_narrate` used to `to_ascii_uppercase()` an art
    // string on its way into the title command — a transform a `{"translate": …}`
    // component cannot express, because the client resolves the lang file after
    // the compiler is gone. Covering lowercase in the font instead moves the
    // fold from emission to rendering, where translation can reach it: a
    // lowercase letter renders through its uppercase bitmap, so the banner looks
    // exactly as it always did, in every language.
    //
    // A cell with no lowercase form (digits, punctuation) is `\u0000`, vanilla's
    // "this cell is unused" marker, so the two providers never claim one char
    // twice.
    let lower: Vec<String> = chars
        .iter()
        .map(|row| {
            row.chars()
                .map(|c| {
                    if c.is_ascii_uppercase() {
                        c.to_ascii_lowercase()
                    } else {
                        '\u{0}'
                    }
                })
                .collect()
        })
        .collect();
    let v = serde_json::json!({
        "providers": [
            { "type": "space", "advances": { " ": ART_SPACE_ADVANCE } },
            {
                "type": "bitmap",
                "file": "delve:font/art.png",
                "ascent": ART_ASCENT,
                "height": ART_HEIGHT,
                "chars": chars
            },
            {
                "type": "bitmap",
                "file": "delve:font/art.png",
                "ascent": ART_ASCENT,
                "height": ART_HEIGHT,
                "chars": lower
            }
        ]
    });
    let mut b = serde_json::to_vec_pretty(&v).expect("font json serializes");
    b.push(b'\n');
    b
}

/// Render the glyph atlas as a deterministic RGBA PNG (white glyphs on transparent).
fn atlas_png() -> Vec<u8> {
    let rows = ART_GLYPHS.len().div_ceil(COLS);
    let width = COLS * CELL;
    let height = rows * CELL;
    // RGBA pixel buffer.
    let mut px = vec![0u8; width * height * 4];
    for (k, (_, glyph)) in ART_GLYPHS.iter().enumerate() {
        let cx = (k % COLS) * CELL;
        let cy = (k / COLS) * CELL;
        for (gy, line) in glyph.iter().enumerate().take(GH) {
            for (gx, ch) in line.chars().enumerate().take(GW) {
                if ch == '#' {
                    let x = cx + gx;
                    let y = cy + gy;
                    let o = (y * width + x) * 4;
                    px[o] = 255;
                    px[o + 1] = 255;
                    px[o + 2] = 255;
                    px[o + 3] = 255;
                }
            }
        }
    }
    crate::png::encode_rgba_stored(width as u32, height as u32, &px)
}

/// The [`GW`]×7 pixel rows of one art glyph, or `None` if the font does not cover
/// `ch`. Lower-case input is folded to upper case (the font is caps-only), so a
/// caller may pass raw ids straight in.
///
/// Exposed for the visual-authoring-loop label stamper
/// ([`crate::snapshot`]/[`crate::blocking`]), which burns the *same* original
/// bitmap font into draft renders that the shipped `delve:art` atlas uses. One
/// glyph table, two consumers — a second hand-drawn font would be a second thing
/// to keep in sync for no gain.
pub fn glyph(ch: char) -> Option<&'static [&'static str; 7]> {
    let up = ch.to_ascii_uppercase();
    ART_GLYPHS.iter().find(|(c, _)| *c == up).map(|(_, g)| g)
}

/// The original 5×7 uppercase pixel font (48 glyphs). Order defines the atlas
/// layout and the font provider's `chars` rows — the single source of truth for
/// both the PNG and the coverage check. `#` = opaque, `.` = transparent.
#[rustfmt::skip]
const ART_GLYPHS: &[(char, [&str; 7])] = &[
    ('A', [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', [".####", "#....", "#....", "#....", "#....", "#....", ".####"]),
    ('D', ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."]),
    ('E', ["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', ["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', [".####", "#....", "#....", "#..##", "#...#", "#...#", ".####"]),
    ('H', ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"]),
    ('J', ["..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##.."]),
    ('K', ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', ["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', ["#...#", "##..#", "#.#.#", "#.#.#", "#..##", "#...#", "#...#"]),
    ('O', [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', [".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"]),
    ('X', ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('0', [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', [".###.", "#...#", "....#", "..##.", ".#...", "#....", "#####"]),
    ('3', ["####.", "....#", "....#", ".###.", "....#", "....#", "####."]),
    ('4', ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."]),
    ('!', ["..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#.."]),
    ('"', [".#.#.", ".#.#.", ".#.#.", ".....", ".....", ".....", "....."]),
    ('\'', ["..#..", "..#..", "..#..", ".....", ".....", ".....", "....."]),
    ('(', ["...#.", "..#..", ".#...", ".#...", ".#...", "..#..", "...#."]),
    (')', [".#...", "..#..", "...#.", "...#.", "...#.", "..#..", ".#..."]),
    (',', [".....", ".....", ".....", ".....", "..#..", "..#..", ".#..."]),
    ('-', [".....", ".....", ".....", "#####", ".....", ".....", "....."]),
    ('.', [".....", ".....", ".....", ".....", ".....", "..#..", "..#.."]),
    ('/', ["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."]),
    (':', [".....", "..#..", "..#..", ".....", "..#..", "..#..", "....."]),
    (';', [".....", "..#..", "..#..", ".....", "..#..", "..#..", ".#..."]),
    ('?', [".###.", "#...#", "....#", "..##.", "..#..", ".....", "..#.."]),
];
