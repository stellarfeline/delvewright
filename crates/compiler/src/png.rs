//! A minimal, deterministic PNG writer (8-bit RGBA, colour type 6).
//!
//! Hand-rolled for the same reason [`crate::resourcepack`]'s ZIP/SHA-1 is: the
//! output must be **byte-stable** under ADR-0006, and every byte a compressor
//! chooses is a byte we would otherwise have to trust a third party to keep
//! stable across versions. Two encoders share one chunk/CRC/checksum core:
//!
//! - [`encode_rgba_stored`] — stored (BTYPE=00) DEFLATE, i.e. no compression at
//!   all. Used by the `delve:art` font atlas ([`crate::atmos`]), whose PNG bytes
//!   are hashed into a shipped resource pack; keeping it uncompressed keeps that
//!   hash a pure function of this repo's source.
//! - [`encode_rgba`] — real DEFLATE via `flate2` at a **pinned** compression
//!   level. Used by the visual-authoring-loop renders ([`crate::snapshot`]),
//!   which are review artifacts measured in megapixels: a 960×540 frame is ~2 MB
//!   stored and ~100 KB deflated. `flate2`'s miniz_oxide backend is a pure-Rust
//!   deterministic compressor (the same property the pinned-mtime gzip emission
//!   already relies on), so a given pixel buffer always yields the same bytes.
//!
//! Neither encoder is a general-purpose image library: no palettes, no
//! interlacing, no ancillary chunks. Filter type 0 (none) on every scanline —
//! filtering would only trade determinism-neutral bytes for size.

use std::io::Write;

/// The pinned DEFLATE level for [`encode_rgba`]. Fixed (never `default()`, which
/// is a `flate2` policy knob that could move) so the compressed bytes are a
/// function of this repo alone.
const DEFLATE_LEVEL: u32 = 6;

/// Encode an 8-bit RGBA buffer as a PNG using real DEFLATE compression.
///
/// `rgba` must hold `width * height * 4` bytes in row-major order.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    encode(width, height, rgba, zlib_deflate)
}

/// Encode an 8-bit RGBA buffer as a PNG using stored (uncompressed) DEFLATE.
///
/// `rgba` must hold `width * height * 4` bytes in row-major order.
pub fn encode_rgba_stored(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    encode(width, height, rgba, zlib_store)
}

/// Shared encoder: PNG signature, `IHDR`, one `IDAT` produced by `zlib`, `IEND`.
fn encode(width: u32, height: u32, rgba: &[u8], zlib: fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
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

    write_chunk(&mut out, b"IDAT", &zlib(&raw));
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

/// Wrap `data` as a zlib stream compressed with DEFLATE at [`DEFLATE_LEVEL`].
fn zlib_deflate(data: &[u8]) -> Vec<u8> {
    let mut enc =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(DEFLATE_LEVEL));
    // Writing to a `Vec` cannot fail, and neither can the encoder's finish.
    enc.write_all(data).expect("in-memory zlib write");
    enc.finish().expect("in-memory zlib finish")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// Decode a PNG this module wrote back to raw RGBA: verify the signature,
    /// walk the chunks checking every CRC, inflate the `IDAT` stream and strip
    /// the (always zero) per-scanline filter bytes. A real round-trip is the only
    /// honest test of a hand-rolled encoder.
    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "signature");
        let mut i = 8;
        let (mut w, mut h) = (0u32, 0u32);
        let mut idat = Vec::new();
        let mut saw_iend = false;
        while i < png.len() {
            let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = png[i + 4..i + 8].try_into().unwrap();
            let data = &png[i + 8..i + 8 + len];
            let crc = u32::from_be_bytes(png[i + 8 + len..i + 12 + len].try_into().unwrap());
            let mut check = kind.to_vec();
            check.extend_from_slice(data);
            assert_eq!(crc, crc32fast::hash(&check), "chunk CRC for {kind:?}");
            match &kind {
                b"IHDR" => {
                    w = u32::from_be_bytes(data[0..4].try_into().unwrap());
                    h = u32::from_be_bytes(data[4..8].try_into().unwrap());
                    assert_eq!((data[8], data[9]), (8, 6), "8-bit RGBA");
                }
                b"IDAT" => idat.extend_from_slice(data),
                b"IEND" => saw_iend = true,
                _ => {}
            }
            i += 12 + len;
        }
        assert!(saw_iend, "IEND present");
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(&idat[..])
            .read_to_end(&mut raw)
            .expect("IDAT inflates");
        let stride = w as usize * 4;
        let mut rgba = Vec::with_capacity(stride * h as usize);
        for y in 0..h as usize {
            let row = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
            assert_eq!(row[0], 0, "filter type 0");
            rgba.extend_from_slice(&row[1..]);
        }
        (w, h, rgba)
    }

    /// A small deterministic gradient buffer.
    fn sample(w: u32, h: u32) -> Vec<u8> {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                px.extend_from_slice(&[(x * 7) as u8, (y * 11) as u8, (x ^ y) as u8, 255]);
            }
        }
        px
    }

    #[test]
    fn deflate_encoder_round_trips() {
        let (w, h) = (37u32, 19u32);
        let px = sample(w, h);
        let (dw, dh, back) = decode(&encode_rgba(w, h, &px));
        assert_eq!((dw, dh), (w, h));
        assert_eq!(back, px);
    }

    #[test]
    fn stored_encoder_round_trips() {
        let (w, h) = (37u32, 19u32);
        let px = sample(w, h);
        let (dw, dh, back) = decode(&encode_rgba_stored(w, h, &px));
        assert_eq!((dw, dh), (w, h));
        assert_eq!(back, px);
    }

    #[test]
    fn stored_blocks_span_more_than_one_deflate_block() {
        // A buffer larger than 0xFFFF forces multiple stored blocks — the loop
        // boundary the art atlas never exercises but the raycaster would.
        let (w, h) = (256u32, 128u32); // 256*128*4 + 128 filter bytes ≈ 131 KB
        let px = sample(w, h);
        let png = encode_rgba_stored(w, h, &px);
        let (_, _, back) = decode(&png);
        assert_eq!(back, px);
    }

    #[test]
    fn encoders_are_byte_stable_across_calls() {
        // ADR-0006: the same pixels always encode to the same bytes.
        let px = sample(64, 64);
        assert_eq!(encode_rgba(64, 64, &px), encode_rgba(64, 64, &px));
        assert_eq!(
            encode_rgba_stored(64, 64, &px),
            encode_rgba_stored(64, 64, &px)
        );
    }

    #[test]
    fn deflate_is_smaller_than_stored_for_a_flat_image() {
        // The reason the snapshot path compresses at all: a mostly-flat frame is
        // an order of magnitude smaller deflated.
        let px = vec![17u8; 256 * 256 * 4];
        let d = encode_rgba(256, 256, &px).len();
        let s = encode_rgba_stored(256, 256, &px).len();
        assert!(d * 8 < s, "deflate {d} should be far under stored {s}");
    }
}
