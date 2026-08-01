//! Sound + art-title surface (DSL v0.6, spec-0014): sound-event validation
//! (`DW0326`), the deferred `play-sound at: actor` gate (`DW0335`), and the
//! large-glyph "art" title font (`delve:art`) with its compile-time glyph-coverage
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
                "quests",
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
            "quests",
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
            d.push(art_diag("quests", a.path.clone(), &a.text, bad));
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
const CELL: usize = 8;
/// Glyph bitmap dimensions.
const GW: usize = 5;
const GH: usize = 7;

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
    let v = serde_json::json!({
        "providers": [
            { "type": "space", "advances": { " ": 16 } },
            {
                "type": "bitmap",
                "file": "delve:font/art.png",
                "ascent": 28,
                "height": 32,
                "chars": chars
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
    encode_png_rgba(width as u32, height as u32, &px)
}

/// Minimal deterministic RGBA PNG encoder (8-bit, color type 6). Uses stored
/// (uncompressed) DEFLATE so no compressor is needed — hand-rolled to stay
/// dependency-free and byte-stable, matching `resourcepack`'s hand-rolled ZIP/SHA-1.
fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = vec![137, 80, 78, 71, 13, 10, 26, 10];

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_chunk(&mut out, b"IHDR", &ihdr);

    // Raw image data: each scanline is a filter byte (0) followed by its pixels.
    let stride = (width as usize) * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0); // filter type 0 (none)
        raw.extend_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }

    write_chunk(&mut out, b"IDAT", &zlib_store(&raw));
    write_chunk(&mut out, b"IEND", &[]);
    out
}

/// Write one PNG chunk: length, type, data, CRC-32 over (type ++ data).
fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32fast::hash(&crc_input).to_be_bytes());
}

/// Wrap `data` as a zlib stream using stored (BTYPE=00) DEFLATE blocks.
fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78u8, 0x01]; // zlib header (CMF=0x78, FLG=0x01; %31 == 0)
    let mut i = 0;
    let n = data.len();
    loop {
        let end = (i + 0xFFFF).min(n);
        let block = &data[i..end];
        let len = block.len() as u16;
        let final_block = end == n;
        out.push(if final_block { 1 } else { 0 }); // BFINAL, BTYPE=00
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
        i = end;
        if final_block {
            break;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// Adler-32 checksum (RFC 1950).
fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
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
