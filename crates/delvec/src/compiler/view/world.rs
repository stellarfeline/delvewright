//! **The world the engine writes** (spec-0089 §5, spec-0112 §5): a
//! configuration written as an Anvil world a frame is drawn from.
//!
//! A frame stands in a configuration, and the bytes of that configuration are
//! the engine's — the model the proofs read ([`crate::compiler::nav::Configuration::blocks`]),
//! never a server's save — so a frame that disagrees with a proof shows a
//! defect of the model instead of hiding it, and two writes of one build agree.
//! A server's save of the same build is the cross-check
//! (`tools/ci/check-written-world.py`, `DW0955`), never the source.
//!
//! What a written world holds: `level.dat` (`DataVersion`, `LevelName`, the
//! spawn, `version`, and the configuration's clock as `DayTime`);
//! `region/r.<x>.<z>.mca`, every chunk of the loaded extent and every chunk a
//! cell stands in, each with `DataVersion` (the pinned game's), `xPos`, `zPos`,
//! `yPos`, `Status: minecraft:full`, every section from the lowest to the
//! highest any cell needs — `Y`, `block_states{palette, data}`,
//! `biomes{palette, data}`, `BlockLight` and `SkyLight` — and the chunk's
//! `block_entities`; and `entities/r.<x>.<z>.mca`, one entity chunk
//! (`DataVersion`, `Position`, `Entities`) per chunk a body stands in. Packed
//! by the minecraft.wiki *Chunk format* rules: indices at `max(4, ceil(log2
//! n))` bits for blocks and `ceil(log2 n)` for biomes, never across a long,
//! `data` absent for a one-entry palette; light at 4 bits a cell, `y`-`z`-`x`
//! order, the even index in the low nibble. Every region timestamp is zero,
//! zlib at one fixed level, chunks in index order, sectors contiguous.
//!
//! **The light** is the engine's one flood ([`crate::compiler::light::LightArrays`])
//! over every written cell: block light from the emitters the configuration
//! holds, sky light seeded at 15 — the two terms before the maximum the
//! game's lightmap is drawn from.
//!
//! Deterministic (ADR-0006): no clock, a fixed compression level, palettes in
//! first-seen order over the section's own index order, every compound the
//! writer builds in a fixed key order and every decoded one sorted by key.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::compiler::blockstate::BlockMap;
use delvewright_dsl::{DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// `DW0955`: the world the engine writes for a camera is not the world the
    /// pinned server builds from the same datapack (spec-0089 §5.4). Raised by
    /// `tools/ci/check-written-world.py`, which compares the written load world
    /// with a `validation/world-save.sh` save cell by cell inside the layout box
    /// and reds on any differing cell no named class (a gravity settle, a fluid
    /// flow, a state re-derived on a block update, a clock's phase) explains —
    /// declared here, beside the writer it holds to the game, so the code has
    /// one home.
    pub const DW_CAMERA_STEP_WORLD: DwCode = DwCode::new("DW0955", ExitTier::Build);
}

/// The `DataVersion` a written chunk carries: the pinned game's (ADR-0009).
pub const DATA_VERSION: i32 = delvewright_dsl::blocks::PIN_DATA_VERSION;

/// The lowest section of the pinned overworld (`min_y` −64 / 16): every
/// written chunk's `yPos`, as the server writes it.
pub const MIN_SECTION: i32 = -4;

/// The zlib level every chunk is compressed at — fixed, so two writes agree.
pub const ZLIB_LEVEL: u32 = 6;

/// The region-file sector, bytes.
const SECTOR: usize = 4096;

/// **What a cell no build write reached holds**: the level generator's own
/// block, which the server keeps wherever the datapack writes nothing.
///
/// Under `horizon: ocean` that is the pinned superflat stack
/// ([`crate::compiler::plan::ocean_layer_at`]: bedrock, stone to the sea floor,
/// water to sea level); under `void` and `valley` the stack is empty and every
/// such cell is air. Which cells a write reached is the assembled world's
/// [`crate::compiler::assembled::Reached`], widened by the caller with what the
/// configuration's own writes and the shipped setup writes reach.
#[derive(Clone)]
pub struct Ground {
    /// The generator's block at a height, or `None` where it lays air.
    layer: Option<fn(i32) -> Option<&'static str>>,
    /// The cells a write reached: the generator does not show through them.
    reached: crate::compiler::assembled::Reached,
}

impl Ground {
    /// A generator that lays nothing (`void`, `valley`): every cell no write
    /// reached is air.
    pub fn void() -> Ground {
        Ground {
            layer: None,
            reached: Default::default(),
        }
    }

    /// The ground a horizon base's generator lays, under the cells `reached`.
    pub fn of_base(
        base: delvewright_dsl::HorizonBase,
        reached: crate::compiler::assembled::Reached,
    ) -> Ground {
        let layer: Option<fn(i32) -> Option<&'static str>> = match base {
            delvewright_dsl::HorizonBase::Ocean => Some(crate::compiler::plan::ocean_layer_at),
            delvewright_dsl::HorizonBase::Void | delvewright_dsl::HorizonBase::Valley => None,
        };
        Ground { layer, reached }
    }

    /// Mark a cell a write reached (a configuration's laid write, a setup write).
    pub fn mark(&mut self, c: [i32; 3]) {
        self.reached.mark(c);
    }

    /// Mark every cell of the inclusive box `a..=b` reached.
    pub fn mark_box(&mut self, a: [i32; 3], b: [i32; 3]) {
        self.reached.mark_box(a, b);
    }

    /// The generator's block at `c`, when no write reached it and the
    /// generator lays a block there.
    pub fn at(&self, c: [i32; 3]) -> Option<&'static str> {
        let layer = self.layer?;
        if self.reached.contains(c) {
            return None;
        }
        layer(c[1])
    }

    /// The sections, bottom up, the generator can lay a block in: every
    /// written chunk carries them, as every chunk of the server's world does.
    fn sections(&self) -> Vec<i32> {
        match self.layer {
            Some(_) => (MIN_SECTION..=crate::compiler::assembled::Reached::CEILING.div_euclid(16))
                .collect(),
            None => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------- NBT

/// An NBT tag as this writer emits it. A compound keeps its entries in the
/// order they were pushed, so every compound the writer builds is in a fixed
/// key order and two writes agree (ADR-0006); a compound taken from a decoded
/// template ([`Nbt::from_value`]) is sorted by key, since its source is a hash
/// map.
#[derive(Debug, Clone, PartialEq)]
pub enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    Str(String),
    List(Vec<Nbt>),
    Compound(Vec<(String, Nbt)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Nbt {
    fn id(&self) -> u8 {
        match self {
            Nbt::Byte(_) => 1,
            Nbt::Short(_) => 2,
            Nbt::Int(_) => 3,
            Nbt::Long(_) => 4,
            Nbt::Float(_) => 5,
            Nbt::Double(_) => 6,
            Nbt::ByteArray(_) => 7,
            Nbt::Str(_) => 8,
            Nbt::List(_) => 9,
            Nbt::Compound(_) => 10,
            Nbt::IntArray(_) => 11,
            Nbt::LongArray(_) => 12,
        }
    }

    fn write_payload(&self, out: &mut Vec<u8>) {
        match self {
            Nbt::Byte(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::ByteArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                out.extend(v.iter().map(|b| *b as u8));
            }
            Nbt::Str(s) => write_str(out, s),
            Nbt::IntArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                for l in v {
                    out.extend_from_slice(&l.to_be_bytes());
                }
            }
            Nbt::LongArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                for l in v {
                    out.extend_from_slice(&l.to_be_bytes());
                }
            }
            Nbt::List(items) => {
                // An empty list is written with element type End, as the game does.
                out.push(items.first().map_or(0, Nbt::id));
                out.extend_from_slice(&(items.len() as i32).to_be_bytes());
                for t in items {
                    t.write_payload(out);
                }
            }
            Nbt::Compound(entries) => {
                for (k, v) in entries {
                    out.push(v.id());
                    write_str(out, k);
                    v.write_payload(out);
                }
                out.push(0);
            }
        }
    }

    /// A decoded tag, with every compound's keys sorted.
    pub fn from_value(v: &fastnbt::Value) -> Nbt {
        use fastnbt::Value as V;
        match v {
            V::Byte(b) => Nbt::Byte(*b),
            V::Short(s) => Nbt::Short(*s),
            V::Int(i) => Nbt::Int(*i),
            V::Long(l) => Nbt::Long(*l),
            V::Float(f) => Nbt::Float(*f),
            V::Double(d) => Nbt::Double(*d),
            V::String(s) => Nbt::Str(s.clone()),
            V::ByteArray(a) => Nbt::ByteArray(a.iter().copied().collect()),
            V::IntArray(a) => Nbt::IntArray(a.iter().copied().collect()),
            V::LongArray(a) => Nbt::LongArray(a.iter().copied().collect()),
            V::List(items) => Nbt::List(items.iter().map(Nbt::from_value).collect()),
            V::Compound(m) => {
                let mut entries: Vec<(String, Nbt)> = m
                    .iter()
                    .map(|(k, v)| (k.clone(), Nbt::from_value(v)))
                    .collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                Nbt::Compound(entries)
            }
        }
    }

    /// The entry `key` of a compound.
    pub fn get(&self, key: &str) -> Option<&Nbt> {
        match self {
            Nbt::Compound(e) => e.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Set `key` in a compound, replacing an entry of that name or appending.
    pub fn set(&mut self, key: &str, value: Nbt) {
        if let Nbt::Compound(e) = self {
            match e.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => e.push((key.to_string(), value)),
            }
        }
    }

    /// The tag as an uncompressed root compound named `""`.
    pub fn to_root_bytes(&self) -> Vec<u8> {
        nbt_root(self.clone())
    }
}

/// The writer's own name for [`Nbt`], kept short.
type Tag = Nbt;

fn write_str(out: &mut Vec<u8>, s: &str) {
    // Modified UTF-8 matches UTF-8 for every character outside the null and
    // the supplementary planes, which nothing this writer emits carries.
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
fn paletted<S: AsRef<str>>(cells: impl Iterator<Item = S>, floor: u32) -> Tag {
    let mut palette: Vec<String> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut indices = Vec::new();
    for c in cells {
        let c = c.as_ref();
        let i = match index.get(c) {
            Some(i) => *i,
            None => {
                palette.push(c.to_string());
                index.insert(c.to_string(), palette.len() - 1);
                palette.len() - 1
            }
        };
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

/// One body the world holds (spec-0112 §5.5): the summon's whole compound
/// (`nbt`, a compound, without `id`, `Pos`, `Rotation`, `Tags` or `UUID`,
/// which the writer adds), its kind, where it stands, its yaw and its tags.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityRecord {
    /// Namespaced id, `minecraft:mannequin`.
    pub kind: String,
    pub pos: [f64; 3],
    pub yaw: f32,
    pub tags: Vec<String>,
    pub nbt: Nbt,
}

impl EntityRecord {
    /// The UUID written for this body: derived from its tags and position, so
    /// two writes agree (ADR-0006) and two bodies apart differ.
    pub fn uuid(&self) -> [i32; 4] {
        let mut h = Sha256::new();
        h.update(self.kind.as_bytes());
        for t in &self.tags {
            h.update([0u8]);
            h.update(t.as_bytes());
        }
        for p in self.pos {
            h.update(p.to_bits().to_be_bytes());
        }
        let d = h.finalize();
        std::array::from_fn(|i| {
            i32::from_be_bytes([d[4 * i], d[4 * i + 1], d[4 * i + 2], d[4 * i + 3]])
        })
    }

    /// The compound as the entity file holds it.
    fn compound(&self) -> Nbt {
        let mut c = match &self.nbt {
            Nbt::Compound(_) => self.nbt.clone(),
            _ => Nbt::Compound(Vec::new()),
        };
        c.set("id", Nbt::Str(self.kind.clone()));
        c.set(
            "Pos",
            Nbt::List(self.pos.iter().map(|v| Nbt::Double(*v)).collect()),
        );
        c.set(
            "Rotation",
            Nbt::List(vec![Nbt::Float(self.yaw), Nbt::Float(0.0)]),
        );
        let mut tags: Vec<Nbt> = match c.get("Tags") {
            Some(Nbt::List(t)) => t.clone(),
            _ => Vec::new(),
        };
        for t in &self.tags {
            if !tags.iter().any(|x| *x == Nbt::Str(t.clone())) {
                tags.push(Nbt::Str(t.clone()));
            }
        }
        if !tags.is_empty() {
            c.set("Tags", Nbt::List(tags));
        }
        c.set("UUID", Nbt::IntArray(self.uuid().to_vec()));
        c
    }
}

/// Everything a written world is made of beyond its blocks.
pub struct WorldInput<'a> {
    pub blocks: &'a BlockMap,
    /// Which biome a 4×4×4 cell stands in, asked at its centre.
    pub biome: &'a dyn Fn([i32; 3]) -> String,
    pub spawn: [i32; 3],
    pub ground: &'a Ground,
    /// The inclusive box every chunk of which is written, its generator's
    /// ground included, whether or not a block of the map stands in it
    /// (spec-0112 §5.3) — the build's loaded extent. `None`: the chunks the
    /// map's cells stand in.
    pub extent: Option<([i32; 3], [i32; 3])>,
    /// Per cell, the block entity's compound (its `id` included; the writer
    /// adds `x`, `y`, `z`). Written only where the cell holds a block.
    pub block_entities: &'a BTreeMap<[i32; 3], Nbt>,
    /// Every body the world holds.
    pub entities: &'a [EntityRecord],
    /// `level.dat`'s `DayTime`: the configuration's clock.
    pub day_time: Option<i64>,
}

/// What a write produced, for the binding line (spec-0089 §7, spec-0112 §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenWorld {
    /// Chunks written.
    pub chunks: usize,
    /// Non-air cells written from the block map.
    pub cells: usize,
    /// Cells holding the generator's block ([`Ground`]): no write reached them.
    pub ground: usize,
    /// Block entities written.
    pub block_entities: usize,
    /// Bodies written.
    pub entities: usize,
    /// Cells the light arrays cover (4096 per written section).
    pub light_cells: usize,
    /// sha-256 over every section's `SkyLight` then `BlockLight`, in chunk and
    /// section order.
    pub light_sha256: String,
    /// sha-256 over the region and entity files' bytes, in file-name order.
    pub sha256: String,
    /// `(relative path, bytes)` of every file, `level.dat` first then the
    /// region files then the entity files, each in name order.
    pub files: Vec<(String, Vec<u8>)>,
}

/// Pack 4096 light levels (`y`-`z`-`x` order) into the 2048 bytes a section
/// stores: two cells a byte, the even index in the low nibble.
pub fn pack_nibbles(levels: &[u8]) -> Vec<i8> {
    levels
        .chunks(2)
        .map(|p| ((p[0] & 0xf) | ((p.get(1).copied().unwrap_or(0) & 0xf) << 4)) as i8)
        .collect()
}

/// Encode `blocks` as a world in memory with no block entity, no body and no
/// clock: [`encode_world`] over the map alone.
pub fn encode(
    blocks: &BlockMap,
    biome: &dyn Fn([i32; 3]) -> String,
    spawn: [i32; 3],
    ground: &Ground,
) -> WrittenWorld {
    encode_world(&WorldInput {
        blocks,
        biome,
        spawn,
        ground,
        extent: None,
        block_entities: &BTreeMap::new(),
        entities: &[],
        day_time: None,
    })
}

/// **Encode a world in memory**: `level.dat` with the spawn and the clock, one
/// region file per region a written chunk stands in, and one entity file per
/// region a body stands in.
///
/// Every written chunk carries every section from the lowest to the highest
/// any cell of the chunk set needs, each with its blocks, its biomes and its
/// two light arrays (the engine's flood over every written cell,
/// [`crate::compiler::light::LightArrays`]), and the chunk's block entities.
pub fn encode_world(input: &WorldInput<'_>) -> WrittenWorld {
    let WorldInput {
        blocks,
        biome,
        spawn,
        ground,
        ..
    } = input;
    // The chunks written: every chunk a cell stands in, and every chunk of the
    // extent.
    let mut chunk_set: std::collections::BTreeSet<(i32, i32)> = Default::default();
    let mut lo_y = i32::MAX;
    let mut hi_y = i32::MIN;
    let mut cells = 0usize;
    for (c, state) in blocks.iter() {
        if state.as_str() == AIR {
            continue;
        }
        cells += 1;
        chunk_set.insert((c[0].div_euclid(16), c[2].div_euclid(16)));
        lo_y = lo_y.min(c[1]);
        hi_y = hi_y.max(c[1]);
    }
    if let Some((lo, hi)) = input.extent {
        for cx in lo[0].div_euclid(16)..=hi[0].div_euclid(16) {
            for cz in lo[2].div_euclid(16)..=hi[2].div_euclid(16) {
                chunk_set.insert((cx, cz));
            }
        }
        lo_y = lo_y.min(lo[1]);
        hi_y = hi_y.max(hi[1]);
    }
    let gsecs = ground.sections();
    if let (Some(a), Some(b)) = (gsecs.first(), gsecs.last()) {
        lo_y = lo_y.min(a * 16);
        hi_y = hi_y.max(b * 16 + 15);
    }
    if chunk_set.is_empty() {
        lo_y = 0;
        hi_y = 0;
    }
    let (sec_lo, sec_hi) = (lo_y.div_euclid(16), hi_y.div_euclid(16));
    // The block at a cell of a written chunk: the map's, else the generator's.
    let at = |c: [i32; 3]| -> Option<&str> {
        match blocks.get(&c) {
            Some(s) => Some(s.as_str()),
            None => ground.at(c),
        }
    };
    // The light of the written box: one flood over every written cell.
    let (box_lo, box_hi) = match (chunk_set.first(), chunk_set.last()) {
        (Some(_), Some(_)) => {
            let (mut x0, mut x1, mut z0, mut z1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
            for (cx, cz) in &chunk_set {
                x0 = x0.min(cx * 16);
                x1 = x1.max(cx * 16 + 15);
                z0 = z0.min(cz * 16);
                z1 = z1.max(cz * 16 + 15);
            }
            ([x0, sec_lo * 16, z0], [x1, sec_hi * 16 + 15, z1])
        }
        _ => ([0, 0, 0], [0, 0, 0]),
    };
    let mut ground_cells: Vec<([i32; 3], &str)> = Vec::new();
    for (cx, cz) in &chunk_set {
        for sy in &gsecs {
            for i in 0..4096 {
                let c = [
                    cx * 16 + (i % 16),
                    sy * 16 + i / 256,
                    cz * 16 + ((i / 16) % 16),
                ];
                if blocks.contains_key(&c) {
                    continue;
                }
                if let Some(g) = ground.at(c) {
                    ground_cells.push((c, g));
                }
            }
        }
    }
    let light = crate::compiler::light::LightArrays::over(
        box_lo,
        box_hi,
        blocks
            .iter()
            .map(|(c, s)| (*c, s.as_str()))
            .chain(ground_cells.iter().copied()),
    );
    let ground_count = ground_cells.len();
    drop(ground_cells);
    let mut light_hash = Sha256::new();
    let mut light_cells = 0usize;
    let mut block_entities = 0usize;
    // region (rx, rz) -> chunk index -> payload.
    let mut regions: BTreeMap<(i32, i32), BTreeMap<usize, Vec<u8>>> = BTreeMap::new();
    for (cx, cz) in &chunk_set {
        let secs: Vec<Tag> = (sec_lo..=sec_hi)
            .map(|sy| {
                let (x0, y0, z0) = (cx * 16, sy * 16, cz * 16);
                let cell = |i: i32| [x0 + (i % 16), y0 + i / 256, z0 + ((i / 16) % 16)];
                let block_cells: Vec<&str> =
                    (0..4096).map(|i| at(cell(i)).unwrap_or(AIR)).collect();
                let biome_cells = (0..64).map(|i| {
                    biome([
                        x0 + 4 * (i % 4) + 2,
                        y0 + 4 * (i / 16) + 2,
                        z0 + 4 * ((i / 4) % 4) + 2,
                    ])
                });
                let sky: Vec<u8> = (0..4096).map(|i| light.sky(cell(i))).collect();
                let blk: Vec<u8> = (0..4096).map(|i| light.block(cell(i))).collect();
                let (sky, blk) = (pack_nibbles(&sky), pack_nibbles(&blk));
                light_hash.update(sky.iter().map(|b| *b as u8).collect::<Vec<u8>>());
                light_hash.update(blk.iter().map(|b| *b as u8).collect::<Vec<u8>>());
                light_cells += 4096;
                compound(vec![
                    // A section's `Y` is a byte, as the game writes it.
                    ("Y", Tag::Byte(sy as i8)),
                    ("block_states", paletted(block_cells.into_iter(), 4)),
                    ("biomes", paletted(biome_cells, 0)),
                    ("BlockLight", Tag::ByteArray(blk)),
                    ("SkyLight", Tag::ByteArray(sky)),
                ])
            })
            .collect();
        // The chunk's block entities, in cell order.
        let bes: Vec<Tag> = input
            .block_entities
            .range([cx * 16, i32::MIN, i32::MIN]..=[cx * 16 + 15, i32::MAX, i32::MAX])
            .filter(|(c, _)| c[2].div_euclid(16) == *cz && at(**c).is_some())
            .map(|(c, nbt)| {
                let mut t = match nbt {
                    Nbt::Compound(_) => nbt.clone(),
                    _ => Nbt::Compound(Vec::new()),
                };
                t.set("x", Tag::Int(c[0]));
                t.set("y", Tag::Int(c[1]));
                t.set("z", Tag::Int(c[2]));
                t
            })
            .collect();
        block_entities += bes.len();
        let mut root = vec![
            ("DataVersion", Tag::Int(DATA_VERSION)),
            ("xPos", Tag::Int(*cx)),
            ("zPos", Tag::Int(*cz)),
            ("yPos", Tag::Int(sec_lo)),
            ("Status", Tag::Str("minecraft:full".to_string())),
            ("sections", Tag::List(secs)),
        ];
        if !bes.is_empty() {
            root.push(("block_entities", Tag::List(bes)));
        }
        let index = (cz.rem_euclid(32) * 32 + cx.rem_euclid(32)) as usize;
        regions
            .entry((cx.div_euclid(32), cz.div_euclid(32)))
            .or_default()
            .insert(index, zlib(&nbt_root(compound(root))));
    }
    // The bodies, one entity chunk per chunk a body stands in.
    let mut by_chunk: BTreeMap<(i32, i32), Vec<Tag>> = BTreeMap::new();
    for e in input.entities {
        let (cx, cz) = (
            (e.pos[0].floor() as i32).div_euclid(16),
            (e.pos[2].floor() as i32).div_euclid(16),
        );
        by_chunk.entry((cx, cz)).or_default().push(e.compound());
    }
    let mut entity_regions: BTreeMap<(i32, i32), BTreeMap<usize, Vec<u8>>> = BTreeMap::new();
    for ((cx, cz), list) in by_chunk {
        let root = compound(vec![
            ("DataVersion", Tag::Int(DATA_VERSION)),
            ("Position", Tag::IntArray(vec![cx, cz])),
            ("Entities", Tag::List(list)),
        ]);
        let index = (cz.rem_euclid(32) * 32 + cx.rem_euclid(32)) as usize;
        entity_regions
            .entry((cx.div_euclid(32), cz.div_euclid(32)))
            .or_default()
            .insert(index, zlib(&nbt_root(root)));
    }
    let mut files = vec![("level.dat".to_string(), level_dat(*spawn, input.day_time))];
    let mut hasher = Sha256::new();
    let mut named: Vec<(String, Vec<u8>)> = regions
        .iter()
        .map(|((rx, rz), payloads)| (format!("region/r.{rx}.{rz}.mca"), region_file(payloads)))
        .chain(entity_regions.iter().map(|((rx, rz), payloads)| {
            (format!("entities/r.{rx}.{rz}.mca"), region_file(payloads))
        }))
        .collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, bytes) in &named {
        hasher.update(bytes);
    }
    files.extend(named);
    WrittenWorld {
        chunks: chunk_set.len(),
        cells,
        ground: ground_count,
        block_entities,
        entities: input.entities.len(),
        light_cells,
        light_sha256: format!("{:x}", light_hash.finalize()),
        sha256: format!("{:x}", hasher.finalize()),
        files,
    }
}

/// zlib at [`ZLIB_LEVEL`], as every chunk is compressed.
fn zlib(raw: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(ZLIB_LEVEL));
    z.write_all(raw).expect("zlib into memory");
    z.finish().expect("zlib into memory")
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

/// The minimal `level.dat` (spec-0089 §2.3, spec-0112 §5.8): gzip (mtime 0) of
/// `Data{DataVersion, LevelName, SpawnX, SpawnY, SpawnZ, version, DayTime}`,
/// `DayTime` present when the world states a clock.
fn level_dat(spawn: [i32; 3], day_time: Option<i64>) -> Vec<u8> {
    let mut entries = vec![
        ("DataVersion", Tag::Int(DATA_VERSION)),
        ("LevelName", Tag::Str("delve".to_string())),
        ("SpawnX", Tag::Int(spawn[0])),
        ("SpawnY", Tag::Int(spawn[1])),
        ("SpawnZ", Tag::Int(spawn[2])),
        // The Anvil format's own version number.
        ("version", Tag::Int(19133)),
    ];
    if let Some(t) = day_time {
        entries.push(("DayTime", Tag::Long(t)));
    }
    let raw = nbt_root(compound(vec![("Data", compound(entries))]));
    let mut g = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::new(ZLIB_LEVEL));
    g.write_all(&raw).expect("gzip into memory");
    g.finish().expect("gzip into memory")
}

/// [`encode_world`], written under `dir`: the directory is emptied of a
/// previous write's `level.dat`, region and entity files first, so a world
/// never carries a file a later configuration does not hold.
pub fn write_world(input: &WorldInput<'_>, dir: &Path) -> std::io::Result<WrittenWorld> {
    let w = encode_world(input);
    write_files(&w, dir)?;
    Ok(w)
}

/// Write an encoded world's files under `dir`, emptying it of a previous
/// write's region and entity files first.
pub fn write_files(w: &WrittenWorld, dir: &Path) -> std::io::Result<()> {
    for sub in ["region", "entities"] {
        let d = dir.join(sub);
        if d.is_dir() {
            for e in std::fs::read_dir(&d)? {
                let p = e?.path();
                if p.extension().is_some_and(|x| x == "mca") {
                    std::fs::remove_file(p)?;
                }
            }
        }
    }
    std::fs::create_dir_all(dir.join("region"))?;
    for (rel, bytes) in &w.files {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

/// [`encode`], written under `dir` ([`write_world`] over the map alone).
pub fn write(
    blocks: &BlockMap,
    biome: &dyn Fn([i32; 3]) -> String,
    spawn: [i32; 3],
    ground: &Ground,
    dir: &Path,
) -> std::io::Result<WrittenWorld> {
    write_world(
        &WorldInput {
            blocks,
            biome,
            spawn,
            ground,
            extent: None,
            block_entities: &BTreeMap::new(),
            entities: &[],
            day_time: None,
        },
        dir,
    )
}

/// How many cells of `blocks` hold a block whose picture is drawn from
/// block-entity NBT — a sign's text, a banner's pattern, a head's profile, a
/// lectern's book — and whose compound `block_entities` does not carry
/// (spec-0112 §5.4). The count is a measured zero on a campaign that places
/// none.
pub fn block_entities_omitted(
    blocks: &BlockMap,
    block_entities: &BTreeMap<[i32; 3], Nbt>,
) -> usize {
    blocks
        .iter()
        .filter(|(c, s)| {
            let id = s.as_str().split('[').next().unwrap_or("");
            let id = id.rsplit(':').next().unwrap_or(id);
            (id.ends_with("_sign")
                || id.ends_with("_banner")
                || id.ends_with("_head")
                || id.ends_with("_skull")
                || id == "lectern")
                && !block_entities.contains_key(*c)
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
        assert_eq!((longs[1] as u64) & 0x1f, 12);
    }

    #[test]
    fn the_ocean_ground_shows_through_only_where_no_write_reached() {
        let mut reached = crate::compiler::assembled::Reached::default();
        reached.mark([0, 58, 0]);
        reached.mark_box([2, 40, 2], [3, 41, 3]);
        let g = Ground::of_base(delvewright_dsl::HorizonBase::Ocean, reached);
        assert_eq!(g.at([0, 58, 0]), None, "a written cell keeps its write");
        assert_eq!(g.at([0, 57, 0]), Some("minecraft:water"));
        assert_eq!(g.at([0, 62, 0]), Some("minecraft:water"));
        assert_eq!(g.at([0, 63, 0]), None, "air above the stack");
        assert_eq!(g.at([0, 54, 0]), Some("minecraft:stone"));
        assert_eq!(g.at([0, -64, 0]), Some("minecraft:bedrock"));
        assert_eq!(g.at([3, 41, 2]), None);
        assert_eq!(g.at([3, 42, 2]), Some("minecraft:stone"));
        let void = Ground::of_base(delvewright_dsl::HorizonBase::Void, Default::default());
        assert_eq!(void.at([0, 57, 0]), None);
        // The writer: one cell written at y 70 carries the stack under it, down
        // to the build floor, everywhere no write reached.
        let mut m = BlockMap::new();
        m.insert([1, 70, 1], BlockState::new("minecraft:stone"));
        let w = encode(&m, &|_| "minecraft:ocean".to_string(), [0, 71, 0], &g);
        // 16x16 columns of 127 layers, less the three cells reached above.
        assert_eq!(w.ground, 256 * 127 - 1 - 8, "{}", w.ground);
        let v = encode(
            &m,
            &|_| "minecraft:ocean".to_string(),
            [0, 71, 0],
            &Ground::void(),
        );
        assert_eq!(v.ground, 0);
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
        let a = encode(&m, &b, [0, 65, 0], &Ground::void());
        let c = encode(&m, &b, [0, 65, 0], &Ground::void());
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
