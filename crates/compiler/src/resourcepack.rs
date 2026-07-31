//! The per-delve NPC-skin resource pack (spec-0009 "bake").
//!
//! A campaign with skinned (mannequin) NPCs ships an original PNG per skin in a
//! server resource pack: `pack.mcmeta` (bare `pack_format` 75 for 1.21.11) plus
//! `assets/delvewright/textures/npc/<id>.png` for each skin. The mannequin's
//! `profile.texture` resolves to `delvewright:npc/<id>`.
//!
//! The zip is **byte-deterministic** (ADR-0006): entries are sorted, timestamps
//! are pinned to zero, and the STORE method is used (no compressor state). Its
//! SHA-1 is what a client verifies against the itzg `RESOURCE_PACK_SHA1` env, so
//! it is recorded in the build manifest. SHA-1 is implemented here to avoid a new
//! crate dependency (offline-safe).

use std::collections::BTreeMap;

use serde_json::json;

/// The resource-pack `pack_format` for 1.21.11 (spec-0009: a bare field; the ≥81
/// `min_format`/`max_format` rule applies to datapacks, not resource packs).
pub const RESOURCE_PACK_FORMAT: u32 = 75;

/// Build the deterministic resource-pack zip for `skins` (texture id → PNG
/// bytes). Returns the zip bytes. `skins` must be non-empty (callers only build a
/// pack when a campaign has skinned NPCs).
pub fn build_pack(skins: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    // Assemble the entries: sorted by archive name for determinism. `assets/…`
    // sorts before `pack.mcmeta`, and `skins` is a BTreeMap (sorted ids).
    let mcmeta = {
        let mut b = serde_json::to_vec_pretty(&json!({
            "pack": {
                "pack_format": RESOURCE_PACK_FORMAT,
                "description": "Delvewright NPC skins"
            }
        }))
        .expect("json serializes");
        b.push(b'\n');
        b
    };
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for (id, png) in skins {
        entries.push((
            format!("assets/delvewright/textures/npc/{id}.png"),
            png.clone(),
        ));
    }
    entries.push(("pack.mcmeta".to_string(), mcmeta));
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    write_store_zip(&entries)
}

/// Write a minimal, deterministic ZIP (STORE method, zero timestamps, no data
/// descriptors) from `entries` (already in the desired order).
fn write_store_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    // (name bytes, crc, size, local-header offset) for the central directory.
    let mut central: Vec<(Vec<u8>, u32, u32, u32)> = Vec::new();

    for (name, data) in entries {
        let name_bytes = name.as_bytes();
        let crc = crc32fast::hash(data);
        let size = data.len() as u32;
        let offset = out.len() as u32;

        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes()); // signature
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: store
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time (pinned 0)
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date (pinned 0)
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes()); // compressed size
        out.extend_from_slice(&size.to_le_bytes()); // uncompressed size
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        central.push((name_bytes.to_vec(), crc, size, offset));
    }

    let cd_offset = out.len() as u32;
    for (name_bytes, crc, size, offset) in &central {
        out.extend_from_slice(&0x0201_4b50u32.to_le_bytes()); // signature
        out.extend_from_slice(&20u16.to_le_bytes()); // version made by
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: store
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(&0u16.to_le_bytes()); // comment len
        out.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        out.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(name_bytes);
    }
    let cd_size = out.len() as u32 - cd_offset;

    // End of central directory.
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk number
    out.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
    out.extend_from_slice(&(central.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
}

/// Lowercase-hex SHA-1 of `bytes` (RFC 3174). Self-contained so the resource-pack
/// hash the client verifies needs no extra dependency.
pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let ml = (bytes.len() as u64) * 8;

    let mut msg = bytes.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&ml.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut s = String::with_capacity(40);
    for word in h {
        s.push_str(&format!("{word:08x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1_hex(b"The quick brown fox jumps over the lazy dog"),
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        );
    }

    #[test]
    fn zip_is_deterministic_and_has_local_signature() {
        let mut skins = BTreeMap::new();
        skins.insert("a".to_string(), vec![1u8, 2, 3]);
        let z1 = build_pack(&skins);
        let z2 = build_pack(&skins);
        assert_eq!(z1, z2, "same input → byte-identical zip");
        assert_eq!(&z1[0..4], &0x0403_4b50u32.to_le_bytes());
    }
}
