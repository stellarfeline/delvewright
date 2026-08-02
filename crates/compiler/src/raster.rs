//! The 2D drawing surface the visual-authoring-loop renderers share
//! (spec-0015): an RGBA byte canvas plus the handful of primitives both the
//! perspective viewport ([`crate::snapshot`]) and the orthographic blocking
//! chart ([`crate::blocking`]) need — alpha blending, rectangles, and bitmap
//! text stamped from the project's own `delve:art` glyph table.
//!
//! Kept deliberately tiny. It is not a graphics library: no transforms, no
//! clipping stack, no anti-aliasing. Everything is integer pixels and every
//! operation is a pure function of its inputs, so both renderers inherit
//! byte-identity (ADR-0006) for free.

/// Glyph cell advance in source pixels (a 5-wide glyph + a 1 px gap).
pub const GLYPH_ADVANCE: i64 = 6;
/// Glyph ink height in source pixels.
pub const GLYPH_ROWS: i64 = 7;

/// A rectangle in whole pixels, origin top-left. Doubles as a target's
/// screen-space bbox in the scene manifest and as a label's occupancy box in
/// the collision-avoiding label placer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenBox {
    /// Left edge (px).
    pub x: i64,
    /// Top edge (px).
    pub y: i64,
    /// Width (px, ≥ 1).
    pub w: i64,
    /// Height (px, ≥ 1).
    pub h: i64,
}

impl ScreenBox {
    /// Whether two boxes intersect.
    pub fn overlaps(self, other: ScreenBox) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

/// A row-major RGBA8 pixel buffer with alpha-blended drawing. Out-of-bounds
/// coordinates are silently dropped, so callers never have to clip.
#[derive(Clone)]
pub struct Canvas {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major RGBA8 pixels (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

impl Canvas {
    /// An opaque canvas filled with `color`.
    pub fn filled(width: u32, height: u32, color: [u8; 3]) -> Canvas {
        let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for _ in 0..(width as usize) * (height as usize) {
            rgba.extend_from_slice(&[color[0], color[1], color[2], 255]);
        }
        Canvas {
            width,
            height,
            rgba,
        }
    }

    /// Set a pixel to an opaque colour (no blending).
    pub fn set(&mut self, x: i64, y: i64, color: [u8; 3]) {
        if let Some(o) = self.offset(x, y) {
            self.rgba[o] = color[0];
            self.rgba[o + 1] = color[1];
            self.rgba[o + 2] = color[2];
            self.rgba[o + 3] = 255;
        }
    }

    /// Blend `color` into pixel `(x, y)` with alpha `a` in `0.0..=1.0`.
    pub fn blend(&mut self, x: i64, y: i64, color: [u8; 3], a: f64) {
        if a <= 0.0 {
            return;
        }
        let Some(o) = self.offset(x, y) else {
            return;
        };
        for (k, &c) in color.iter().enumerate() {
            let old = self.rgba[o + k] as f64;
            self.rgba[o + k] = (old + (c as f64 - old) * a.min(1.0)).round() as u8;
        }
    }

    /// Blend a filled rectangle.
    pub fn fill_rect(&mut self, b: ScreenBox, color: [u8; 3], alpha: f64) {
        for y in b.y..b.y + b.h {
            for x in b.x..b.x + b.w {
                self.blend(x, y, color, alpha);
            }
        }
    }

    /// Draw a 1 px rectangle outline.
    pub fn stroke_rect(&mut self, b: ScreenBox, color: [u8; 3], alpha: f64) {
        for x in b.x..b.x + b.w {
            self.blend(x, b.y, color, alpha);
            self.blend(x, b.y + b.h - 1, color, alpha);
        }
        for y in b.y..b.y + b.h {
            self.blend(b.x, y, color, alpha);
            self.blend(b.x + b.w - 1, y, color, alpha);
        }
    }

    /// Stamp `text` with its ink's top-left at `(x, y)`, over a dark plate so it
    /// stays readable on both a bright sky and a dark cavern floor.
    ///
    /// Glyphs come from [`crate::atmos::glyph`] — the same original bitmap font
    /// the shipped `delve:art` banner atlas uses. Characters the caps-only font
    /// does not cover are skipped rather than substituted, so an id with odd
    /// punctuation still reads.
    pub fn stamp_text(&mut self, x: i64, y: i64, text: &str, scale: i64, color: [u8; 3]) {
        let w = text_width(text, scale);
        let h = GLYPH_ROWS * scale;
        self.fill_rect(
            ScreenBox {
                x: x - scale,
                y: y - scale,
                w: w + 2 * scale,
                h: h + 2 * scale,
            },
            PLATE_COLOR,
            PLATE_ALPHA,
        );
        let mut pen = x;
        for ch in text.chars() {
            if ch != ' '
                && let Some(g) = crate::atmos::glyph(ch)
            {
                for (gy, row) in g.iter().enumerate() {
                    for (gx, c) in row.chars().enumerate() {
                        if c != '#' {
                            continue;
                        }
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.blend(
                                    pen + gx as i64 * scale + sx,
                                    y + gy as i64 * scale + sy,
                                    color,
                                    1.0,
                                );
                            }
                        }
                    }
                }
            }
            pen += GLYPH_ADVANCE * scale;
        }
    }

    /// Byte offset of pixel `(x, y)`, or `None` when it is off-canvas.
    fn offset(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return None;
        }
        Some(((y as usize) * self.width as usize + x as usize) * 4)
    }
}

/// The label plate's colour and opacity.
const PLATE_COLOR: [u8; 3] = [12, 12, 16];
const PLATE_ALPHA: f64 = 0.78;

/// Rendered width of `text` in pixels at `scale` (the trailing inter-glyph gap
/// of the last character is not counted).
pub fn text_width(text: &str, scale: i64) -> i64 {
    (text.chars().count() as i64 * GLYPH_ADVANCE - 1).max(1) * scale
}

/// The colour a manifest target's marker and label are drawn in, per kind. One
/// table for both renderers, so a stealth zone is the same lilac in a viewport
/// frame and in a blocking chart.
pub fn kind_color(kind: &str) -> [u8; 3] {
    match kind {
        "anchor" => [255, 214, 84],
        "gate" => [214, 128, 255],
        "npc-post" => [104, 232, 255],
        "actor-post" => [255, 128, 128],
        "interact" => [124, 255, 148],
        "stealth-zone" => [180, 180, 255],
        "trigger" => [255, 168, 92],
        _ => [230, 230, 230],
    }
}

/// A greedy, deterministic label placer: keeps every box it has accepted and
/// nudges each new candidate straight down (by `step` px, at most `nudges`
/// times) until it clears them all.
///
/// Deterministic by construction — the result depends only on the order labels
/// are offered, which every caller derives from a sorted target list.
#[derive(Default)]
pub struct LabelPlacer {
    placed: Vec<ScreenBox>,
}

impl LabelPlacer {
    /// Try to place `want`; returns the accepted box (possibly nudged down) or
    /// `None` when it could not be fitted within `nudges` attempts.
    pub fn place(&mut self, want: ScreenBox, step: i64, nudges: usize) -> Option<ScreenBox> {
        let mut probe = want;
        for _ in 0..=nudges {
            if !self.placed.iter().any(|p| p.overlaps(probe)) {
                self.placed.push(probe);
                return Some(probe);
            }
            probe.y += step;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_outside_the_canvas_is_a_no_op() {
        let mut c = Canvas::filled(4, 4, [0, 0, 0]);
        let before = c.rgba.clone();
        c.blend(-5, 2, [255, 255, 255], 1.0);
        c.blend(2, 99, [255, 255, 255], 1.0);
        c.set(-1, -1, [255, 255, 255]);
        c.stamp_text(-100, -100, "OFF", 2, [255, 255, 255]);
        assert_eq!(c.rgba, before);
    }

    #[test]
    fn blend_interpolates_and_alpha_zero_does_nothing() {
        let mut c = Canvas::filled(1, 1, [0, 0, 0]);
        c.blend(0, 0, [200, 100, 50], 0.5);
        assert_eq!(&c.rgba[..3], &[100, 50, 25]);
        let before = c.rgba.clone();
        c.blend(0, 0, [255, 255, 255], 0.0);
        assert_eq!(c.rgba, before);
    }

    #[test]
    fn stroke_rect_draws_only_the_border() {
        let mut c = Canvas::filled(5, 5, [0, 0, 0]);
        c.stroke_rect(
            ScreenBox {
                x: 0,
                y: 0,
                w: 5,
                h: 5,
            },
            [255, 255, 255],
            1.0,
        );
        let px = |x: usize, y: usize| c.rgba[(y * 5 + x) * 4];
        assert_eq!(px(0, 0), 255);
        assert_eq!(px(4, 4), 255);
        assert_eq!(px(2, 2), 0, "the interior stays untouched");
    }

    #[test]
    fn text_width_matches_what_is_stamped() {
        // The plate is sized from `text_width`, so a mismatch would clip glyphs.
        assert_eq!(text_width("A", 1), GLYPH_ADVANCE - 1);
        assert_eq!(text_width("AB", 2), (2 * GLYPH_ADVANCE - 1) * 2);
        let mut c = Canvas::filled(64, 16, [0, 0, 0]);
        c.stamp_text(2, 2, "AB", 1, [255, 255, 255]);
        // Ink appears inside the reported width and never past it.
        let ink_at = |x: usize| (0..16).any(|y| c.rgba[(y * 64 + x) * 4] > 100);
        assert!(ink_at(2), "first glyph column inked");
        assert!(
            !ink_at((2 + text_width("AB", 1) + 2) as usize),
            "no ink past the reported width"
        );
    }

    #[test]
    fn uncovered_characters_are_skipped_but_still_advance() {
        // `#` is not in the caps-only art font: it must not panic, and the next
        // glyph must still land where the width model says.
        let mut c = Canvas::filled(64, 16, [0, 0, 0]);
        c.stamp_text(2, 2, "#A", 1, [255, 255, 255]);
        let ink_at = |x: usize| (0..16).any(|y| c.rgba[(y * 64 + x) * 4] > 100);
        assert!(!ink_at(2), "the uncovered glyph inks nothing");
        assert!(
            ink_at((2 + GLYPH_ADVANCE) as usize),
            "the next glyph advanced"
        );
    }

    #[test]
    fn label_placer_nudges_then_gives_up() {
        let mut p = LabelPlacer::default();
        let b = ScreenBox {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        assert_eq!(p.place(b, 12, 3), Some(b), "first placement is unchanged");
        // The same box collides and is nudged clear.
        let second = p.place(b, 12, 3).expect("nudged into the clear");
        assert_eq!(second.y, 12);
        // With no nudges allowed, a colliding box is refused.
        assert_eq!(p.place(b, 12, 0), None);
    }

    #[test]
    fn overlaps_is_exclusive_at_the_edges() {
        let a = ScreenBox {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        let touching = ScreenBox {
            x: 10,
            y: 0,
            w: 10,
            h: 10,
        };
        let crossing = ScreenBox {
            x: 9,
            y: 0,
            w: 10,
            h: 10,
        };
        assert!(!a.overlaps(touching), "edge-adjacent boxes do not overlap");
        assert!(a.overlaps(crossing));
    }
}
