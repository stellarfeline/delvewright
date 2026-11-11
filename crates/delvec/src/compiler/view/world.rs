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
delvewright_dsl::dw_code! {
    /// `DW1040`: **the objects the engine writes are not the objects the pinned
    /// server places** (spec-0112 §8.2). Raised by
    /// `tools/ci/check-written-world.py`, which compares every block entity of
    /// the written load world by cell and compound, and every entity by kind,
    /// cell and tag set, against a `validation/world-save.sh` save of the same
    /// build, and reds on a difference no named class (the server's own
    /// bookkeeping keys, a re-rolled loot table, a body present only once a
    /// player has played) explains — declared beside the writer, as `DW0955`.
    pub const DW_WRITTEN_OBJECTS: DwCode = DwCode::new("DW1040", ExitTier::Build);
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
/// On an `ocean` base that is the pinned superflat stack
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
                // A list of more than one tag type (a sign's messages: a
                // plain string beside a `translate` compound) is written as
                // the 1.21.11 game writes one (`ListTag.wrapIfNeeded`): a
                // list of compounds, every element that is not a compound —
                // or is a compound whose one key is `""` — wrapped as
                // `{"": element}`.
                if items.iter().any(|t| t.id() != items[0].id()) {
                    out.push(10);
                    out.extend_from_slice(&(items.len() as i32).to_be_bytes());
                    for t in items {
                        let bare =
                            matches!(t, Nbt::Compound(e) if !(e.len() == 1 && e[0].0.is_empty()));
                        if bare {
                            t.write_payload(out);
                        } else {
                            Nbt::Compound(vec![(String::new(), t.clone())]).write_payload(out);
                        }
                    }
                    return;
                }
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
        // The pitch is the compound's own (a frame on a floor or a ceiling
        // looks up or down), 0 where it states none.
        let pitch = match c.get("Rotation") {
            Some(Nbt::List(r)) => match r.get(1) {
                Some(Nbt::Float(f)) => *f,
                Some(Nbt::Double(d)) => *d as f32,
                _ => 0.0,
            },
            _ => 0.0,
        };
        c.set(
            "Rotation",
            Nbt::List(vec![Nbt::Float(self.yaw), Nbt::Float(pitch)]),
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
    pub biome: &'a (dyn Fn([i32; 3]) -> String + Sync),
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
    biome: &(dyn Fn([i32; 3]) -> String + Sync),
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
    // The ground's cells — every cell of the written chunks' ground sections
    // that no block holds — are walked, never gathered: under an ocean they
    // outnumber the placed cells three to one (58 M for a 95-place town), and
    // held as a list at 32 bytes each they were the largest single allocation
    // of the build. The light flood reads the walk twice and the count once,
    // in the same order a list would have given, so what it writes does not
    // move.
    let ground_cells = || {
        chunk_set.iter().flat_map(|(cx, cz)| {
            gsecs.iter().flat_map(move |sy| {
                (0..4096).filter_map(move |i| {
                    let c = [
                        cx * 16 + (i % 16),
                        sy * 16 + i / 256,
                        cz * 16 + ((i / 16) % 16),
                    ];
                    if blocks.contains_key(&c) {
                        return None;
                    }
                    ground.at(c).map(|g| (c, g))
                })
            })
        })
    };
    let ground_count = ground_cells().count();
    crate::compiler::notes::mark("encode: chunk set and ground counted");
    // The light is flooded strip by strip of [`LIGHT_STRIP`] chunk columns,
    // each over its own box grown by [`LIGHT_REACH`] on both `x` sides
    // ([`strip_light`]): what the chunks of a strip carry is the whole box's
    // light, and only one strip's arrays are held at a time (ADR-0037 §2).
    // The chunks are taken in the same order as before, so the light hash
    // and every payload are the same.
    let gsecs_ref = &gsecs;
    let strip_cells = |x0: i32, x1: i32| {
        let generated = chunk_set
            .range((x0.div_euclid(16), i32::MIN)..=(x1.div_euclid(16), i32::MAX))
            .flat_map(move |(cx, cz)| {
                gsecs_ref.iter().flat_map(move |sy| {
                    (0..4096).filter_map(move |i| {
                        let c = [
                            cx * 16 + (i % 16),
                            sy * 16 + i / 256,
                            cz * 16 + ((i / 16) % 16),
                        ];
                        if c[0] < x0 || c[0] > x1 || blocks.contains_key(&c) {
                            return None;
                        }
                        ground.at(c).map(|g| (c, g))
                    })
                })
            });
        blocks
            .in_box([x0, i32::MIN, i32::MIN], [x1, i32::MAX, i32::MAX])
            .map(|(c, s)| (c, s.as_str()))
            .chain(generated)
    };
    // The strips are lit and their chunks encoded in parallel
    // (`par::try_for_each_ordered`), each strip from the map, the ground and
    // its own light alone, and folded here in chunk order: the light hash is
    // fed the same bytes in the same order, and every payload lands at the
    // same index, at any thread count. At most `threads` strips' light fields
    // and `2 × threads` strips' encoded chunks (each a compressed chunk and
    // its 4,096 light bytes per section) are held at once, where the
    // sequential walk held one of each (ADR-0037; `docs/reference/compiler.md`,
    // "Parallel work merges in a fixed order").
    let chunks: Vec<(i32, i32)> = chunk_set.iter().copied().collect();
    let strips: Vec<&[(i32, i32)]> = chunks
        .chunk_by(|a, b| a.0.div_euclid(LIGHT_STRIP) == b.0.div_euclid(LIGHT_STRIP))
        .collect();
    let encode_strip = |strip: &&[(i32, i32)]| -> Vec<EncodedChunk> {
        let light = strip_light(
            box_lo,
            box_hi,
            strip[0].0.div_euclid(LIGHT_STRIP),
            &strip_cells,
        );
        strip
            .iter()
            .map(|&(cx, cz)| encode_chunk(input, &at, &light, (cx, cz), (sec_lo, sec_hi)))
            .collect()
    };
    let mut light_hash = Sha256::new();
    let mut light_cells = 0usize;
    let mut block_entities = 0usize;
    // region (rx, rz) -> chunk index -> payload.
    let mut regions: BTreeMap<(i32, i32), BTreeMap<usize, Vec<u8>>> = BTreeMap::new();
    let folded: Result<(), std::convert::Infallible> =
        crate::par::try_for_each_ordered(&strips, encode_strip, |encoded| {
            for c in encoded {
                light_hash.update(&c.light);
                light_cells += c.light_cells;
                block_entities += c.block_entities;
                let (cx, cz) = c.chunk;
                let index = (cz.rem_euclid(32) * 32 + cx.rem_euclid(32)) as usize;
                regions
                    .entry((cx.div_euclid(32), cz.div_euclid(32)))
                    .or_default()
                    .insert(index, c.payload);
            }
            Ok(())
        });
    let Ok(()) = folded;
    crate::compiler::notes::mark("encode: chunks lit, encoded and compressed");
    let mut files = vec![("level.dat".to_string(), level_dat(*spawn, input.day_time))];
    let mut hasher = Sha256::new();
    let mut named: Vec<(String, Vec<u8>)> = regions
        .iter()
        .map(|((rx, rz), payloads)| (format!("region/r.{rx}.{rz}.mca"), region_file(payloads)))
        .chain(entity_files(input.entities))
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

/// **The entity files of a world holding `entities`**: one entity chunk per
/// chunk a body stands in, `entities/r.<x>.<z>.mca` per region, in name
/// order.
fn entity_files(entities: &[EntityRecord]) -> Vec<(String, Vec<u8>)> {
    let mut by_chunk: BTreeMap<(i32, i32), Vec<Tag>> = BTreeMap::new();
    for e in entities {
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
    entity_regions
        .iter()
        .map(|((rx, rz), payloads)| (format!("entities/r.{rx}.{rz}.mca"), region_file(payloads)))
        .collect()
}

impl WrittenWorld {
    /// **This world holding `entities` in place of its own bodies**: the same
    /// `level.dat` and region files, its entity files written again — a
    /// picture whose bodies differ from another's over the same blocks (a
    /// body that turns to face the lens) without encoding the blocks twice.
    pub fn with_entities(&self, entities: &[EntityRecord]) -> WrittenWorld {
        let mut files: Vec<(String, Vec<u8>)> = self
            .files
            .iter()
            .filter(|(n, _)| !n.starts_with("entities/"))
            .cloned()
            .collect();
        let level = files.remove(0);
        files.extend(entity_files(entities));
        files.sort_by(|a, b| a.0.cmp(&b.0));
        let mut hasher = Sha256::new();
        for (_, bytes) in &files {
            hasher.update(bytes);
        }
        files.insert(0, level);
        WrittenWorld {
            entities: entities.len(),
            sha256: format!("{:x}", hasher.finalize()),
            files,
            ..self.clone()
        }
    }
}

/// One chunk as [`encode_world`] folds it: its payload, the light bytes the
/// light hash is fed for it, in section order, and its counts.
struct EncodedChunk {
    chunk: (i32, i32),
    payload: Vec<u8>,
    light: Vec<u8>,
    light_cells: usize,
    block_entities: usize,
}

/// **One written chunk**: every section from `secs.0` to `secs.1` with its
/// blocks (`at`), its biomes, and its two light arrays read from its strip's
/// `light`; its block entities in cell order; zlib-compressed as the region
/// file holds it.
fn encode_chunk<'b>(
    input: &WorldInput<'_>,
    at: &impl Fn([i32; 3]) -> Option<&'b str>,
    light: &crate::compiler::light::LightArrays,
    (cx, cz): (i32, i32),
    (sec_lo, sec_hi): (i32, i32),
) -> EncodedChunk {
    let biome = input.biome;
    let mut light_bytes = Vec::with_capacity(((sec_hi - sec_lo + 1) as usize) * 4096);
    let mut light_cells = 0usize;
    let secs: Vec<Tag> = (sec_lo..=sec_hi)
        .map(|sy| {
            let (x0, y0, z0) = (cx * 16, sy * 16, cz * 16);
            let cell = |i: i32| [x0 + (i % 16), y0 + i / 256, z0 + ((i / 16) % 16)];
            let block_cells: Vec<&str> = (0..4096).map(|i| at(cell(i)).unwrap_or(AIR)).collect();
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
            light_bytes.extend(sky.iter().map(|b| *b as u8));
            light_bytes.extend(blk.iter().map(|b| *b as u8));
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
        .filter(|(c, _)| c[2].div_euclid(16) == cz && at(**c).is_some())
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
    let block_entities = bes.len();
    let mut root = vec![
        ("DataVersion", Tag::Int(DATA_VERSION)),
        ("xPos", Tag::Int(cx)),
        ("zPos", Tag::Int(cz)),
        ("yPos", Tag::Int(sec_lo)),
        ("Status", Tag::Str("minecraft:full".to_string())),
        ("sections", Tag::List(secs)),
    ];
    if !bes.is_empty() {
        root.push(("block_entities", Tag::List(bes)));
    }
    EncodedChunk {
        chunk: (cx, cz),
        payload: zlib(&nbt_root(compound(root))),
        light: light_bytes,
        light_cells,
        block_entities,
    }
}

/// The chunk columns one strip of a world's light is flooded over.
const LIGHT_STRIP: i32 = 4;

/// How far light reaches: a seed of 15 lights nothing 15 steps away, so the
/// light of a cell is decided by the cells within 14 of it.
const LIGHT_REACH: i32 = 15;

/// **The light of strip `g`** — the chunk columns `g * LIGHT_STRIP ..` — of a
/// world whose written box is `box_lo..=box_hi`: one flood over the strip's
/// columns grown by [`LIGHT_REACH`] on both `x` sides, clipped to the box,
/// holding what `cells(x0, x1)` gives for those columns. Every cell of the
/// strip's own columns holds the light the whole box's flood gives it: the
/// field is the pointwise maximum over every seed of `seed − distance`, a
/// seed lights only cells within 14 steps of it, and every path of 14 steps
/// into the strip lies inside the grown box; the box's own faces are the
/// whole box's.
fn strip_light<'c, I>(
    box_lo: [i32; 3],
    box_hi: [i32; 3],
    g: i32,
    cells: &impl Fn(i32, i32) -> I,
) -> crate::compiler::light::LightArrays
where
    I: Iterator<Item = ([i32; 3], &'c str)> + Clone,
{
    let x0 = (g * LIGHT_STRIP * 16 - LIGHT_REACH).max(box_lo[0]);
    let x1 = ((g + 1) * LIGHT_STRIP * 16 - 1 + LIGHT_REACH).min(box_hi[0]);
    crate::compiler::light::LightArrays::over(
        [x0, box_lo[1], box_lo[2]],
        [x1, box_hi[1], box_hi[2]],
        cells(x0, x1),
    )
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

/// **A world `from` wrote, holding `entities` in place of its bodies**,
/// written under `dir`: its `level.dat` and region files copied as they are,
/// its entity files encoded from `entities`
/// ([`WrittenWorld::with_entities`]'s files, without encoding the blocks
/// again). Returns how many bodies it holds.
pub fn copy_with_entities(
    from: &Path,
    entities: &[EntityRecord],
    dir: &Path,
) -> std::io::Result<usize> {
    if dir.is_dir() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir.join("region"))?;
    std::fs::copy(from.join("level.dat"), dir.join("level.dat"))?;
    for e in std::fs::read_dir(from.join("region"))? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "mca")
            && let Some(name) = p.file_name()
        {
            std::fs::copy(&p, dir.join("region").join(name))?;
        }
    }
    for (rel, bytes) in entity_files(entities) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
    }
    Ok(entities.len())
}

/// [`encode`], written under `dir` ([`write_world`] over the map alone).
pub fn write(
    blocks: &BlockMap,
    biome: &(dyn Fn([i32; 3]) -> String + Sync),
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
                && !block_entities.contains_key(c)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    /// **A world's light, flooded strip by strip, is the whole box's**
    /// (ADR-0037 §2, the review world): on generated worlds — opaque blocks,
    /// glass, emitters of 15, 14 and 7, open sky and overhangs, spread over
    /// boxes whose `x` extent spans several strips and starts off a strip's
    /// edge — every cell of every strip's own columns holds, in both terms,
    /// the light one flood over the whole box gives it. Red on any cell;
    /// perturbed by a reach of 13.
    #[test]
    fn a_strip_of_light_is_lit_as_the_whole_box_lights_it() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let palette = [
            "minecraft:stone",
            "minecraft:glass",
            "minecraft:glowstone",
            "minecraft:torch",
            "minecraft:redstone_torch",
            "minecraft:oak_leaves",
        ];
        let (mut cells, mut lit, mut strips) = (0usize, 0usize, 0usize);
        for (lo, hi) in [
            ([-40, 0, -8], [150, 30, 23]),
            ([5, -10, 0], [200, 12, 15]),
            ([-130, 60, -20], [-1, 80, 11]),
        ] {
            // A roof with a few holes in it, a floor, sparse emitters and
            // pillars in open air: light carries its full reach under the
            // roof, and the sky's spills in sideways through the holes.
            let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
            for x in lo[0]..=hi[0] {
                for z in lo[2]..=hi[2] {
                    blocks.insert([x, lo[1], z], palette[0].to_string());
                    if next(40) != 0 {
                        blocks.insert([x, hi[1] - 1, z], palette[0].to_string());
                    }
                }
            }
            for _ in 0..((hi[0] - lo[0]) * (hi[2] - lo[2]) / 40) {
                let c = [
                    lo[0] + next((hi[0] - lo[0] + 1) as u64) as i32,
                    lo[1] + 1 + next((hi[1] - lo[1] - 2) as u64) as i32,
                    lo[2] + next((hi[2] - lo[2] + 1) as u64) as i32,
                ];
                let b = palette[match next(10) {
                    0 => 2,
                    1 => 3,
                    2 => 4,
                    3 => 1,
                    4 => 5,
                    _ => 0,
                }];
                blocks.insert(c, b.to_string());
            }
            let all = || blocks.iter().map(|(c, b)| (*c, b.as_str()));
            let whole = crate::compiler::light::LightArrays::over(lo, hi, all());
            let cells_in = |x0: i32, x1: i32| {
                blocks
                    .range([x0, i32::MIN, i32::MIN]..=[x1, i32::MAX, i32::MAX])
                    .map(|(c, b)| (*c, b.as_str()))
            };
            let per = LIGHT_STRIP * 16;
            for g in lo[0].div_euclid(per)..=hi[0].div_euclid(per) {
                strips += 1;
                let part = strip_light(lo, hi, g, &cells_in);
                for x in (g * per).max(lo[0])..=((g + 1) * per - 1).min(hi[0]) {
                    for y in lo[1]..=hi[1] {
                        for z in lo[2]..=hi[2] {
                            let c = [x, y, z];
                            assert_eq!(part.sky(c), whole.sky(c), "{c:?}: sky light");
                            assert_eq!(part.block(c), whole.block(c), "{c:?}: block light");
                            cells += 1;
                            if whole.block(c) > 0 && whole.block(c) < 15 {
                                lit += 1;
                            }
                        }
                    }
                }
            }
        }
        eprintln!(
            "strip light: {cells} cell(s) over {strips} strip(s), {lit} lit by a spread of block \
             light"
        );
        assert!(cells > 0 && lit > 0 && strips > 3, "the cases bind nothing");
    }

    /// A list of two tag types — a sign's messages, a string beside a
    /// `translate` compound — is written as the game writes one: a list of
    /// compounds, the string wrapped under the empty key, the compound bare.
    #[test]
    fn a_list_of_two_tag_types_is_written_wrapped() {
        let list = Nbt::List(vec![
            Nbt::Str(String::new()),
            Nbt::Compound(vec![("translate".into(), Nbt::Str("k".into()))]),
        ]);
        let mut out = Vec::new();
        list.write_payload(&mut out);
        let mut want = vec![10u8, 0, 0, 0, 2];
        // {"": ""}
        want.extend([8, 0, 0, 0, 0, 0]);
        // {translate: "k"}
        want.extend([8, 0, 9]);
        want.extend(b"translate");
        want.extend([0, 1, b'k', 0]);
        assert_eq!(out, want);
        let mut same = Vec::new();
        Nbt::List(vec![Nbt::Str("a".into()), Nbt::Str("b".into())]).write_payload(&mut same);
        assert_eq!(same[0], 8, "a list of one type is written as it is");
    }

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

    /// **A world encoded strip by strip in parallel is the sequential
    /// world** (`encode_world`, `par::try_for_each_ordered`): a generated
    /// world spanning eight light strips — stone floors and roofs with holes,
    /// emitters, chests carrying block entities, an ocean ground under it —
    /// encoded at one thread and at four gives the same files, counts and
    /// light hash, and both are the bytes the one-strip-at-a-time walk wrote
    /// before the strips were parallel (the pinned hashes, taken from that
    /// implementation on this case). A fold out of strip order moves the
    /// light hash; one out of chunk order moves a region file.
    #[test]
    fn a_world_encoded_in_parallel_is_the_sequential_world() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let (lo, hi) = ([-150, 40, -20], [290, 70, 40]);
        let mut m = BlockMap::new();
        let mut bes: BTreeMap<[i32; 3], Nbt> = BTreeMap::new();
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                m.insert([x, lo[1], z], BlockState::new("minecraft:stone"));
                if next(30) != 0 {
                    m.insert([x, hi[1], z], BlockState::new("minecraft:stone"));
                }
            }
        }
        for _ in 0..2_000 {
            let c = [
                lo[0] + next((hi[0] - lo[0] + 1) as u64) as i32,
                lo[1] + 1 + next((hi[1] - lo[1] - 1) as u64) as i32,
                lo[2] + next((hi[2] - lo[2] + 1) as u64) as i32,
            ];
            let b = match next(6) {
                0 => "minecraft:glowstone",
                1 => "minecraft:torch",
                2 => "minecraft:glass",
                3 => {
                    bes.insert(
                        c,
                        Nbt::Compound(vec![("id".to_string(), Nbt::Str("minecraft:chest".into()))]),
                    );
                    "minecraft:chest[facing=north]"
                }
                _ => "minecraft:oak_planks",
            };
            m.insert(c, BlockState::new(b));
        }
        let mut reached = crate::compiler::assembled::Reached::default();
        for c in m.keys() {
            reached.mark(c);
        }
        let ground = Ground::of_base(delvewright_dsl::HorizonBase::Ocean, reached);
        let biome = |c: [i32; 3]| {
            if c[0] < 0 {
                "minecraft:ocean".to_string()
            } else {
                "minecraft:plains".to_string()
            }
        };
        let input = WorldInput {
            blocks: &m,
            biome: &biome,
            spawn: [0, 41, 0],
            ground: &ground,
            extent: None,
            block_entities: &bes,
            entities: &[],
            day_time: Some(6000),
        };
        let one = crate::par::with_threads(1, || encode_world(&input));
        let four = crate::par::with_threads(4, || encode_world(&input));
        let strips = {
            let per = LIGHT_STRIP * 16;
            (lo[0].div_euclid(per)..=hi[0].div_euclid(per)).count()
        };
        eprintln!(
            "parallel encode: {} chunk(s) over {strips} strip(s), {} block entit(ies), light {}, \
             world {}",
            one.chunks, one.block_entities, one.light_sha256, one.sha256
        );
        assert!(
            strips >= 8 && one.block_entities > 0,
            "the case binds nothing"
        );
        assert_eq!(one, four, "the world at one thread and at four");
        assert_eq!(
            one.light_sha256, "8a96c7f2959845280fecafa80e9978eaab8b1d676e82e90aa4ebfd51d107cc3b",
            "the sequential walk's light hash"
        );
        assert_eq!(
            one.sha256, "be064630957e0b6da67c20ec25d22ccfcd148fcece0bede25c889e1d490c19b4",
            "the sequential walk's world hash"
        );
    }
}
