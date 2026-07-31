//! Missing-texture (magenta) detector — the fidelity gate's core check.
//!
//! Minecraft's "missing texture" is a magenta/black checker; an unresolved block
//! model (e.g. a bare `"texture":"all"` reference Nucleation cannot resolve)
//! meshes with that magenta placeholder instead of a real texture. We color-key
//! scan the rendered frame for magenta-ish pixels (high R, low G, high B, with
//! R≈B) and flag the frame when their share crosses a small threshold — a single
//! placeholder block face is thousands of pixels, far above the floor, while
//! stray anti-aliased edge pixels stay below it.
//!
//! Pure (no GPU); unit tested against synthetic frames and a committed real
//! `heavy_core` placeholder crop.

/// A missing-texture finding.
#[derive(Debug, Clone, PartialEq)]
pub struct MissingTexture {
    /// Magenta pixel count.
    pub count: u32,
    /// Fraction of total pixels that were magenta.
    pub fraction: f64,
    /// First (top-left-most) magenta pixel, as `(x, y)`.
    pub sample: (u32, u32),
}

/// True when `(r,g,b)` is a Minecraft missing-texture magenta. Deliberately wide
/// (200/80/200 with R≈B within 64) so it catches the placeholder whether it is
/// rendered as pure `#FF00FF`, the atlas's `#F800F8`, or a slightly shaded face,
/// without matching purples/pinks (which have R≫B or G≫80).
#[inline]
pub fn is_magenta(r: u8, g: u8, b: u8) -> bool {
    r >= 200 && b >= 200 && g <= 80 && (r as i32 - b as i32).abs() <= 64
}

/// Fraction floor: a frame is flagged only when magenta pixels exceed this share
/// of the total. One placeholder face at 1024² is ~0.3–3%; edge fringe stays well
/// under 0.05%.
pub const DEFAULT_THRESHOLD: f64 = 0.0005;

/// Scan an RGBA8 frame (`w*h*4` bytes) for the missing-texture placeholder.
/// Returns a finding when the magenta share exceeds `threshold`.
pub fn scan(rgba: &[u8], w: u32, h: u32, threshold: f64) -> Option<MissingTexture> {
    let total = (w as usize) * (h as usize);
    if total == 0 || rgba.len() < total * 4 {
        return None;
    }
    let mut count: u32 = 0;
    let mut sample: Option<(u32, u32)> = None;
    for i in 0..total {
        let p = &rgba[i * 4..i * 4 + 4];
        if is_magenta(p[0], p[1], p[2]) {
            count += 1;
            if sample.is_none() {
                let x = (i as u32) % w;
                let y = (i as u32) / w;
                sample = Some((x, y));
            }
        }
    }
    let fraction = count as f64 / total as f64;
    if fraction > threshold {
        Some(MissingTexture {
            count,
            fraction,
            sample: sample.unwrap_or((0, 0)),
        })
    } else {
        None
    }
}

/// [`scan`] with [`DEFAULT_THRESHOLD`].
pub fn scan_default(rgba: &[u8], w: u32, h: u32) -> Option<MissingTexture> {
    scan(rgba, w, h, DEFAULT_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, fill: [u8; 4]) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            v.extend_from_slice(&fill);
        }
        v
    }

    #[test]
    fn magenta_predicate() {
        assert!(is_magenta(255, 0, 255)); // pure
        assert!(is_magenta(248, 0, 248)); // atlas
        assert!(is_magenta(230, 40, 210)); // shaded face
        assert!(!is_magenta(120, 120, 120)); // stone gray
        assert!(!is_magenta(180, 80, 255)); // purple (R != B)
        assert!(!is_magenta(255, 150, 255)); // pink (G too high)
    }

    #[test]
    fn clean_frame_is_not_flagged() {
        let f = frame(256, 256, [130, 130, 130, 255]);
        assert!(scan_default(&f, 256, 256).is_none());
    }

    #[test]
    fn a_placeholder_face_is_flagged() {
        // A gray frame with a 32x32 magenta patch (0.4% of 256²) — above floor.
        let (w, h) = (256, 256);
        let mut f = frame(w, h, [130, 130, 130, 255]);
        for y in 10..42 {
            for x in 20..52 {
                let i = ((y * w + x) * 4) as usize;
                f[i] = 250;
                f[i + 1] = 0;
                f[i + 2] = 250;
                f[i + 3] = 255;
            }
        }
        let found = scan_default(&f, w, h).expect("placeholder detected");
        assert_eq!(found.count, 32 * 32);
        assert_eq!(found.sample, (20, 10));
    }

    #[test]
    fn a_few_stray_pixels_stay_under_threshold() {
        let (w, h) = (256, 256);
        let mut f = frame(w, h, [130, 130, 130, 255]);
        for i in 0..8 {
            let off = i * 4;
            f[off] = 255;
            f[off + 1] = 0;
            f[off + 2] = 255;
        }
        assert!(scan_default(&f, w, h).is_none());
    }

    /// The committed crop is a REAL Nucleation render of `minecraft:heavy_core`
    /// (bare `"texture":"all"` model, unresolved → magenta placeholder), lifted
    /// from the spike-render-fidelity evidence. Proves the detector catches an
    /// actual placeholder, not just a synthetic one — while heavy_core stays OUT
    /// of the gate fixture (expected-fail).
    #[test]
    fn catches_real_heavy_core_placeholder() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/heavy_core_placeholder.png"
        );
        let img = match image::open(p) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                panic!("real heavy_core placeholder fixture must be present: {e}");
            }
        };
        let (w, h) = img.dimensions();
        let found = scan_default(img.as_raw(), w, h)
            .expect("detector must flag the real heavy_core placeholder");
        assert!(found.count > 0);
    }
}
