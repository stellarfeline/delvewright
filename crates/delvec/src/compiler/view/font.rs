//! A 5×7 bitmap font, built in.
//!
//! The contact sheet is *curation* material: the owner looks at a page of
//! candidates and says "that one". A cell she cannot name is a cell she cannot
//! choose, so every cell carries its rank, its candidate id and its score as
//! pixels — not in a sidecar she has to hold open beside the image.
//!
//! Why a hand-rolled bitmap rather than a font crate: labels must be legible and
//! **identical on every machine**, and a TrueType rasterizer's hinting/AA is the
//! one part of this pipeline that would differ between a laptop and a runner —
//! which would break the double-compose byte-identity gate for no gain. Five by
//! seven pixels, integer-scaled, is exactly reproducible.
//!
//! Single-case: lowercase and uppercase share one glyph (prefab ids are
//! lowercase-kebab by repo convention; the header prints short caps like `STUB`).
//! Any character with no glyph renders as `?` — never as a blank, so a label can
//! never silently lose a character.

/// Glyph cell width in pixels, before scaling.
pub const GLYPH_W: u32 = 5;
/// Glyph cell height in pixels, before scaling.
pub const GLYPH_H: u32 = 7;
/// Blank columns between glyphs, before scaling.
pub const TRACKING: u32 = 1;

/// Advance of one character (glyph + tracking) at `scale`.
pub const fn advance(scale: u32) -> u32 {
    (GLYPH_W + TRACKING) * scale
}

/// Rendered width of `text` at `scale` (no trailing tracking).
pub fn text_width(text: &str, scale: u32) -> u32 {
    let n = text.chars().count() as u32;
    if n == 0 {
        return 0;
    }
    n * advance(scale) - TRACKING * scale
}

/// Rendered height of one line at `scale`.
pub const fn text_height(scale: u32) -> u32 {
    GLYPH_H * scale
}

/// Rows are top→bottom; within a row bit 4 (`0b10000`) is the leftmost pixel.
type Glyph = [u8; GLYPH_H as usize];

const SPACE: Glyph = [0, 0, 0, 0, 0, 0, 0];
const UNKNOWN: Glyph = [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04];

/// Look up the glyph for `c`, folding case. Unknown characters map to `?`.
pub fn glyph(c: char) -> Glyph {
    match c.to_ascii_lowercase() {
        ' ' => SPACE,
        'a' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'b' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'c' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'd' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'e' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'f' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'g' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'h' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'i' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'j' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'k' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'l' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'm' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'n' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'o' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'p' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'r' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        's' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        't' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'u' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'v' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'w' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'x' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x1C],
        '-' => [0x00, 0x00, 0x00, 0x0E, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x06],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x06, 0x06, 0x08],
        ':' => [0x00, 0x06, 0x06, 0x00, 0x06, 0x06, 0x00],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '%' => [0x11, 0x01, 0x02, 0x04, 0x08, 0x10, 0x11],
        '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        '*' => [0x00, 0x0A, 0x04, 0x1F, 0x04, 0x0A, 0x00],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '?' => UNKNOWN,
        _ => UNKNOWN,
    }
}

/// Draw `text` at `(x, y)` (top-left, pixels) into `img`, integer-scaled by
/// `scale`. Clipped at the image edges; never panics on an out-of-bounds label.
pub fn draw_text(
    img: &mut image::RgbaImage,
    x: u32,
    y: u32,
    text: &str,
    scale: u32,
    color: [u8; 4],
) {
    let scale = scale.max(1);
    let (iw, ih) = (img.width(), img.height());
    for (i, ch) in text.chars().enumerate() {
        let gx = x + i as u32 * advance(scale);
        if gx >= iw {
            return;
        }
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..GLYPH_W {
                if bits & (1 << (GLYPH_W - 1 - col)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = gx + col * scale + dx;
                        let py = y + row as u32 * scale + dy;
                        if px < iw && py < ih {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    }
                }
            }
        }
    }
}

/// Truncate `text` so it fits in `max_px` at `scale`, marking the cut with `…`
/// spelled as `.` (the font is ASCII). Returns the text unchanged when it fits.
pub fn fit(text: &str, max_px: u32, scale: u32) -> String {
    if text_width(text, scale) <= max_px {
        return text.to_string();
    }
    let per = advance(scale);
    if max_px < per {
        return String::new();
    }
    let room = ((max_px + TRACKING * scale) / per) as usize;
    if room <= 3 {
        return text.chars().take(room).collect();
    }
    let mut s: String = text.chars().take(room - 3).collect();
    s.push_str("...");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ascii_printable_has_a_glyph_and_unknowns_fall_back() {
        // No character may render as an accidental blank: only a literal space
        // is allowed to be empty, so a label can never silently lose a glyph.
        for c in ' '..='~' {
            let g = glyph(c);
            if c == ' ' {
                assert_eq!(g, SPACE);
            } else {
                assert_ne!(g, SPACE, "{c:?} renders blank");
            }
        }
        assert_eq!(glyph('\u{4e2d}'), UNKNOWN, "non-ASCII must fall back to ?");
    }

    #[test]
    fn case_folds_to_one_glyph() {
        assert_eq!(glyph('A'), glyph('a'));
        assert_eq!(glyph('Z'), glyph('z'));
    }

    #[test]
    fn metrics_and_fit() {
        assert_eq!(text_width("", 2), 0);
        assert_eq!(text_width("ab", 1), 11);
        assert_eq!(text_height(3), 21);
        assert_eq!(fit("short", 1000, 1), "short");
        let cut = fit("a-very-long-candidate-id", 60, 1);
        assert!(cut.ends_with("..."), "{cut}");
        assert!(text_width(&cut, 1) <= 60, "{cut}");
    }

    #[test]
    fn draw_is_clipped_not_panicking() {
        let mut img = image::RgbaImage::new(8, 8);
        draw_text(&mut img, 6, 6, "long text", 3, [255, 255, 255, 255]);
    }
}
