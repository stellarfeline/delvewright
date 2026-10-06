//! **The world the engine writes** (spec-0089 §5): a block map written as an
//! Anvil world a Chunky scene can name.
//!
//! A Chunky scene names a world directory and nothing else will do, and the
//! only other producer of one is a server boot (`validation/world-save.sh`),
//! which stops at load by construction. So a showcase camera's world is written
//! here, from the configuration's block map ([`crate::compiler::nav::Configuration::blocks`]):
//! `level.dat` and `region/r.<x>.<z>.mca`, nothing else.
//!
//! Per chunk holding at least one cell of the map: `DataVersion` (the pinned
//! game's), `xPos`, `zPos`, `yPos`, `Status: minecraft:full`, and `sections[]`
//! for the sections holding a cell, each with `Y`, `block_states{palette, data}`
//! and `biomes{palette, data}`, packed by the minecraft.wiki *Chunk format*
//! rules: indices at `max(4, ceil(log2 n))` bits for blocks and
//! `ceil(log2 n)` for biomes, never across a long, `data` absent for a
//! one-entry palette. Every region timestamp is zero, zlib at one fixed level,
//! chunks in index order, sectors contiguous. No light arrays, heightmaps,
//! entities, block entities or ticks: the pinned core lights a scene itself and
//! draws none of the entities the engine summons (spec-0089 §2.2).
//!
//! Deterministic (ADR-0006): no clock, a fixed compression level, palettes in
//! first-seen order over the section's own index order, the NBT written by the
//! writer here in a fixed key order rather than through a hash map.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::compiler::blockstate::BlockMap;

/// The `DataVersion` a written chunk carries: the pinned game's (ADR-0009).
pub const DATA_VERSION: i32 = delvewright_dsl::blocks::PIN_DATA_VERSION;

/// The lowest section of the pinned overworld (`min_y` −64 / 16): every
/// written chunk's `yPos`, as the server writes it.
pub const MIN_SECTION: i32 = -4;

/// The zlib level every chunk is compressed at — fixed, so two writes agree.
pub const ZLIB_LEVEL: u32 = 6;

/// The region-file sector, bytes.
const SECTOR: usize = 4096;

/// What a write produced, for the binding line (spec-0089 §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenWorld {
    /// Chunks written.
    pub chunks: usize,
    /// Non-air cells written.
    pub cells: usize,
    /// sha-256 over the region files' bytes, in file-name order.
    pub sha256: String,
    /// `(relative path, bytes)` of every file, `level.dat` first then the
    /// region files in name order.
    pub files: Vec<(String, Vec<u8>)>,
}

// ---------------------------------------------------------------------- NBT

/// The tags this writer emits — the subset a minimal world needs, written in
/// the order they are pushed.
enum Tag {
    Int(i32),
    Str(String),
    LongArray(Vec<i64>),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Int(_) => 3,
            Tag::Str(_) => 8,
            Tag::List(_) => 9,
            Tag::Compound(_) => 10,
            Tag::LongArray(_) => 12,
        }
    }

    fn write_payload(&self, out: &mut Vec<u8>) {
        match self {
            Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
            Tag::Str(s) => write_str(out, s),
            Tag::LongArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                for l in v {
                    out.extend_from_slice(&l.to_be_bytes());
                }
            }
            Tag::List(items) => {
                // An empty list is written with element type End, as the game does.
                out.push(items.first().map_or(0, Tag::id));
                out.extend_from_slice(&(items.len() as i32).to_be_bytes());
                for t in items {
                    t.write_payload(out);
                }
            }
            Tag::Compound(entries) => {
                for (k, v) in entries {
                    out.push(v.id());
                    write_str(out, k);
                    v.write_payload(out);
                }
                out.push(0);
            }
        }
    }
}

fn write_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// A root compound named `""`, as chunk and `level.dat` payloads are.
fn nbt_root(root: Tag) -> Vec<u8> {
    let mut out = vec![root.id()];
    write_str(&mut out, "");
    root.write_payload(&mut out);
    out
}

fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

// ------------------------------------------------------------------ packing

/// Bits per index for a palette of `n` entries with a `floor` (4 for blocks,
/// 0 for biomes): `max(floor, ceil(log2 n))`. A one-entry palette writes no
/// `data` and is never asked.
pub fn bits_for(n: usize, floor: u32) -> u32 {
    let need = usize::BITS - (n.saturating_sub(1)).leading_zeros();
    need.max(floor).max(1)
}

/// Pack `indices` at `bits` per index, never across a long (the 1.16+ rule).
pub fn pack(indices: &[usize], bits: u32) -> Vec<i64> {
    let per = (64 / bits) as usize;
    indices
        .chunks(per)
        .map(|run| {
            let mut w: u64 = 0;
            for (j, v) in run.iter().enumerate() {
                w |= (*v as u64) << (j as u32 * bits);
            }
            w as i64
        })
        .collect()
}

/// A block state's text as the chunk palette entry: `Name` and `Properties`
/// completed with the pinned default of every property the text leaves out —
/// what the game itself resolves an unwritten property to when it loads a
/// template ([`delvewright_dsl::blocks::BlockRegistry::default_state`]). This
/// is a world, the state the server holds before any neighbour update, never a
/// template a library reads back, so the completion asserts nothing a later
/// shape rule would have to re-derive. Properties are written in name order.
fn palette_entry(text: &str) -> Tag {
    let (name, props) = match text.split_once('[') {
        Some((n, rest)) => (n, rest.trim_end_matches(']')),
        None => (text, ""),
    };
    let name = if name.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    };
    let mut full: BTreeMap<String, String> = delvewright_dsl::blocks::BlockRegistry::v1_21_11()
        .default_state(&name)
        .cloned()
        .unwrap_or_default();
    for (k, v) in props
        .split(',')
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.split_once('='))
    {
        full.insert(k.to_string(), v.to_string());
    }
    let mut entries = vec![("Name", Tag::Str(name))];
    if !full.is_empty() {
        entries.push((
            "Properties",
            Tag::Compound(full.into_iter().map(|(k, v)| (k, Tag::Str(v))).collect()),
        ));
    }
    compound(entries)
}

/// A palette and its packed indices: first-seen order over `cells`.
fn paletted(cells: impl Iterator<Item = String>, floor: u32) -> Tag {
    let mut palette: Vec<String> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut indices = Vec::new();
    for c in cells {
        let i = *index.entry(c.clone()).or_insert_with(|| {
            palette.push(c);
            palette.len() - 1
        });
        indices.push(i);
    }
    let mut entries: Vec<(&str, Tag)> = Vec::new();
    let is_blocks = floor > 0;
    entries.push((
        "palette",
        Tag::List(
            palette
                .iter()
                .map(|p| {
                    if is_blocks {
                        palette_entry(p)
                    } else {
                        Tag::Str(p.clone())
                    }
                })
                .collect(),
        ),
    ));
    if palette.len() > 1 {
        entries.push((
            "data",
            Tag::LongArray(pack(&indices, bits_for(palette.len(), floor))),
        ));
    }
    compound(entries)
}

// -------------------------------------------------------------------- write

const AIR: &str = "minecraft:air";

/// Encode `blocks` as a world in memory: `level.dat` with the spawn at
/// `spawn`, and one region file per region a cell stands in. `biome(cell)`
/// answers which biome a 4×4×4 cell stands in, asked at the cell's centre
/// (coordinates 2 mod 4), the point vanilla's own reading never jitters off.
pub fn encode(
    blocks: &BlockMap,
    biome: &dyn Fn([i32; 3]) -> String,
    spawn: [i32; 3],
) -> WrittenWorld {
    // chunk (cx, cz) -> section Y -> present.
    let mut chunks: BTreeMap<(i32, i32), BTreeMap<i32, ()>> = BTreeMap::new();
    let mut cells = 0usize;
    for (c, state) in blocks {
        if state.as_str() == AIR {
            continue;
        }
        cells += 1;
        chunks
            .entry((c[0].div_euclid(16), c[2].div_euclid(16)))
            .or_default()
            .insert(c[1].div_euclid(16), ());
    }
    // region (rx, rz) -> chunk index -> payload.
    let mut regions: BTreeMap<(i32, i32), BTreeMap<usize, Vec<u8>>> = BTreeMap::new();
    for ((cx, cz), sections) in &chunks {
        let secs: Vec<Tag> = sections
            .keys()
            .map(|sy| {
                let (x0, y0, z0) = (cx * 16, sy * 16, cz * 16);
                let block_cells = (0..4096).map(|i| {
                    let cell = [x0 + (i % 16), y0 + i / 256, z0 + ((i / 16) % 16)];
                    blocks
                        .get(&cell)
                        .map_or_else(|| AIR.to_string(), |s| s.as_str().to_string())
                });
                let biome_cells = (0..64).map(|i| {
                    biome([
                        x0 + 4 * (i % 4) + 2,
                        y0 + 4 * (i / 16) + 2,
                        z0 + 4 * ((i / 4) % 4) + 2,
                    ])
                });
                compound(vec![
                    ("Y", Tag::Int(*sy)),
                    ("block_states", paletted(block_cells, 4)),
                    ("biomes", paletted(biome_cells, 0)),
                ])
            })
            .collect();
        let root = compound(vec![
            ("DataVersion", Tag::Int(DATA_VERSION)),
            ("xPos", Tag::Int(*cx)),
            ("zPos", Tag::Int(*cz)),
            ("yPos", Tag::Int(MIN_SECTION)),
            ("Status", Tag::Str("minecraft:full".to_string())),
            ("sections", Tag::List(secs)),
        ]);
        let mut z =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(ZLIB_LEVEL));
        z.write_all(&nbt_root(root)).expect("zlib into memory");
        let comp = z.finish().expect("zlib into memory");
        let index = (cz.rem_euclid(32) * 32 + cx.rem_euclid(32)) as usize;
        regions
            .entry((cx.div_euclid(32), cz.div_euclid(32)))
            .or_default()
            .insert(index, comp);
    }
    let mut files = vec![("level.dat".to_string(), level_dat(spawn))];
    let mut hasher = Sha256::new();
    // Region files in name order, so the hash is over a fixed order.
    let mut named: Vec<(String, Vec<u8>)> = regions
        .iter()
        .map(|((rx, rz), payloads)| (format!("region/r.{rx}.{rz}.mca"), region_file(payloads)))
        .collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, bytes) in &named {
        hasher.update(bytes);
    }
    files.extend(named);
    WrittenWorld {
        chunks: chunks.len(),
        cells,
        sha256: format!("{:x}", hasher.finalize()),
        files,
    }
}

/// One region file: the location table, a zero timestamp table, and each
/// chunk's `length + compression (2 = zlib) + bytes` padded to whole sectors,
/// in index order.
fn region_file(payloads: &BTreeMap<usize, Vec<u8>>) -> Vec<u8> {
    let mut header = vec![0u8; 2 * SECTOR];
    let mut body = Vec::new();
    let mut sector = 2usize;
    for (index, comp) in payloads {
        let mut p = Vec::with_capacity(comp.len() + 5);
        p.extend_from_slice(&((comp.len() + 1) as i32).to_be_bytes());
        p.push(2);
        p.extend_from_slice(comp);
        p.resize(p.len().div_ceil(SECTOR) * SECTOR, 0);
        let count = p.len() / SECTOR;
        let loc = ((sector as u32) << 8) | (count as u32 & 0xff);
        header[4 * index..4 * index + 4].copy_from_slice(&loc.to_be_bytes());
        body.extend_from_slice(&p);
        sector += count;
    }
    header.extend_from_slice(&body);
    header
}

/// The minimal `level.dat` the pinned core loads (spec-0089 §2.3): gzip
/// (mtime 0) of `Data{DataVersion, LevelName, SpawnX, SpawnY, SpawnZ, version}`.
fn level_dat(spawn: [i32; 3]) -> Vec<u8> {
    let data = compound(vec![
        ("DataVersion", Tag::Int(DATA_VERSION)),
        ("LevelName", Tag::Str("delve".to_string())),
        ("SpawnX", Tag::Int(spawn[0])),
        ("SpawnY", Tag::Int(spawn[1])),
        ("SpawnZ", Tag::Int(spawn[2])),
        // The Anvil format's own version number.
        ("version", Tag::Int(19133)),
    ]);
    let raw = nbt_root(compound(vec![("Data", data)]));
    let mut g = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::new(ZLIB_LEVEL));
    g.write_all(&raw).expect("gzip into memory");
    g.finish().expect("gzip into memory")
}

/// [`encode`], written under `dir`: the directory is emptied of a previous
/// write's `level.dat` and region files first, so a world never carries a
/// region file a later configuration does not hold.
pub fn write(
    blocks: &BlockMap,
    biome: &dyn Fn([i32; 3]) -> String,
    spawn: [i32; 3],
    dir: &Path,
) -> std::io::Result<WrittenWorld> {
    let w = encode(blocks, biome, spawn);
    let region = dir.join("region");
    if region.is_dir() {
        for e in std::fs::read_dir(&region)? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "mca") {
                std::fs::remove_file(p)?;
            }
        }
    }
    std::fs::create_dir_all(&region)?;
    for (rel, bytes) in &w.files {
        std::fs::write(dir.join(rel), bytes)?;
    }
    Ok(w)
}

/// How many cells of `blocks` hold a block whose picture the pinned core
/// draws from block-entity NBT a block map does not carry — a sign's text, a
/// banner's pattern, a head's profile, a lectern's book (spec-0089 §6.2). The
/// written world draws each blank, and the binding line counts them, so the
/// count is a measured zero on a campaign that places none.
pub fn block_entities_omitted(blocks: &BlockMap) -> usize {
    blocks
        .values()
        .filter(|s| {
            let id = s.as_str().split('[').next().unwrap_or("");
            let id = id.rsplit(':').next().unwrap_or(id);
            id.ends_with("_sign")
                || id.ends_with("_banner")
                || id.ends_with("_head")
                || id.ends_with("_skull")
                || id == "lectern"
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    #[test]
    fn bits_follow_the_chunk_format() {
        assert_eq!(bits_for(2, 4), 4);
        assert_eq!(bits_for(16, 4), 4);
        assert_eq!(bits_for(17, 4), 5);
        assert_eq!(bits_for(2, 0), 1);
        assert_eq!(bits_for(3, 0), 2);
        assert_eq!(bits_for(5, 0), 3);
    }

    #[test]
    fn packing_never_spans_a_long() {
        // 5 bits: 12 per long, the last 4 bits of each long unused.
        let idx: Vec<usize> = (0..24).map(|i| i % 17).collect();
        let longs = pack(&idx, 5);
        assert_eq!(longs.len(), 2);
        assert_eq!((longs[1] as u64) & 0x1f, 12 % 17);
    }

    #[test]
    fn two_writes_are_byte_identical_and_hold_no_clock() {
        let mut m = BlockMap::new();
        m.insert([0, 64, 0], BlockState::new("minecraft:stone"));
        m.insert(
            [40, 70, -3],
            BlockState::new("minecraft:oak_stairs[facing=east,half=top]"),
        );
        let b = |_c: [i32; 3]| "minecraft:plains".to_string();
        let a = encode(&m, &b, [0, 65, 0]);
        let c = encode(&m, &b, [0, 65, 0]);
        assert_eq!(a, c);
        assert_eq!(a.chunks, 2);
        assert_eq!(a.cells, 2);
        for (rel, bytes) in &a.files {
            if rel.ends_with(".mca") {
                assert!(bytes[SECTOR..2 * SECTOR].iter().all(|b| *b == 0), "{rel}");
            }
        }
    }
}
