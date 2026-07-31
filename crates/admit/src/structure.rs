//! An editable vanilla structure template (`.nbt`), read from and written back to
//! the same gzip-framed Java NBT the compiler/generator/`delve-schem` emit.
//!
//! `delve-render`/`delve-schem` each read structures for their own purpose;
//! admission needs one editable model it can both **inspect** (palette audit,
//! light probe) and **mutate** (socket carving) and re-emit deterministically.
//! Determinism (ADR-0006): compounds are `BTreeMap`-ordered via the schem crate's
//! [`Nbt`] type, gzip mtime is pinned to 0, and the block/palette order is
//! preserved on read → mutate → write, so the same input + the same edits produce
//! byte-identical output.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read as _, Write as _};

use delvewright_schem::convert::{self, DATA_VERSION};
use delvewright_schem::nbt::Nbt;
use flate2::{Compression, GzBuilder};
use serde::Serialize;

/// One palette entry: a block name plus its (sorted) block-state properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}

impl PaletteEntry {
    pub fn simple(name: impl Into<String>) -> Self {
        PaletteEntry {
            name: name.into(),
            properties: BTreeMap::new(),
        }
    }

    pub fn with_props(name: impl Into<String>, props: &[(&str, &str)]) -> Self {
        PaletteEntry {
            name: name.into(),
            properties: props
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

/// One placed block: its local position, palette index, and optional block-entity
/// NBT (carried verbatim).
#[derive(Debug, Clone)]
pub struct Block {
    pub pos: [i32; 3],
    pub state: i32,
    pub nbt: Option<Nbt>,
}

/// An editable structure template.
#[derive(Debug, Clone)]
pub struct Structure {
    pub data_version: i32,
    pub size: [i32; 3],
    pub palette: Vec<PaletteEntry>,
    pub blocks: Vec<Block>,
    /// `pos -> index into blocks`, for O(1) cell lookup/mutation.
    index: BTreeMap<[i32; 3], usize>,
}

impl Structure {
    /// Parse a gzip-framed structure `.nbt`.
    pub fn read(bytes: &[u8]) -> Result<Structure, String> {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut raw)
            .map_err(|e| format!("gzip decode: {e}"))?;
        let value: fastnbt::Value = fastnbt::from_bytes(&raw).map_err(|e| format!("nbt: {e}"))?;
        let root = Nbt::from(value);
        let root = root.as_compound().ok_or("root not compound")?;

        let data_version = root
            .get("DataVersion")
            .and_then(Nbt::as_i32)
            .ok_or("no DataVersion")?;
        let size = read_ivec3(root.get("size").ok_or("no size")?).ok_or("bad size")?;

        let palette = root
            .get("palette")
            .and_then(Nbt::as_list)
            .ok_or("no palette")?
            .iter()
            .map(read_palette_entry)
            .collect::<Result<Vec<_>, String>>()?;

        let mut blocks = Vec::new();
        let mut index = BTreeMap::new();
        for (i, b) in root
            .get("blocks")
            .and_then(Nbt::as_list)
            .ok_or("no blocks")?
            .iter()
            .enumerate()
        {
            let c = b.as_compound().ok_or("block not compound")?;
            let pos = read_ivec3(c.get("pos").ok_or("no pos")?).ok_or("bad pos")?;
            let state = c.get("state").and_then(Nbt::as_i32).ok_or("no state")?;
            if state < 0 || state as usize >= palette.len() {
                return Err(format!("block state {state} out of palette range"));
            }
            let nbt = c.get("nbt").cloned();
            index.insert(pos, i);
            blocks.push(Block { pos, state, nbt });
        }

        Ok(Structure {
            data_version,
            size,
            palette,
            blocks,
            index,
        })
    }

    /// The distinct block **names** in the palette (namespaced), sorted.
    pub fn block_names(&self) -> BTreeSet<String> {
        self.palette.iter().map(|p| p.name.clone()).collect()
    }

    /// The block at `pos`, if present.
    pub fn block_at(&self, pos: [i32; 3]) -> Option<&Block> {
        self.index.get(&pos).map(|&i| &self.blocks[i])
    }

    /// The palette entry backing `pos`, if the cell exists.
    pub fn entry_at(&self, pos: [i32; 3]) -> Option<&PaletteEntry> {
        self.block_at(pos).map(|b| &self.palette[b.state as usize])
    }

    /// True when `pos` is inside the structure bounds.
    pub fn in_bounds(&self, pos: [i32; 3]) -> bool {
        (0..3).all(|a| pos[a] >= 0 && pos[a] < self.size[a])
    }

    /// Find the palette index for `entry`, appending it if new.
    pub fn palette_idx(&mut self, entry: PaletteEntry) -> i32 {
        if let Some(i) = self.palette.iter().position(|e| *e == entry) {
            return i as i32;
        }
        self.palette.push(entry);
        (self.palette.len() - 1) as i32
    }

    /// Overwrite the cell at `pos` with `entry` + optional block-entity `nbt`.
    /// Adds the cell if it did not exist (preserving deterministic block order).
    pub fn set_cell(&mut self, pos: [i32; 3], entry: PaletteEntry, nbt: Option<Nbt>) {
        let state = self.palette_idx(entry);
        if let Some(&i) = self.index.get(&pos) {
            self.blocks[i].state = state;
            self.blocks[i].nbt = nbt;
        } else {
            let i = self.blocks.len();
            self.blocks.push(Block { pos, state, nbt });
            self.index.insert(pos, i);
        }
    }

    /// Drop palette entries no block references, remapping every block's `state`.
    /// Entries are kept in **first-use order** (deterministic; the block list is
    /// itself deterministic). Used after an edit that can orphan an entry (e.g.
    /// resolving away every `minecraft:jigsaw` cell) so the audit palette reflects
    /// only blocks that are actually placed.
    pub fn prune_palette(&mut self) {
        let mut remap: Vec<Option<i32>> = vec![None; self.palette.len()];
        let mut kept: Vec<PaletteEntry> = Vec::new();
        for b in &self.blocks {
            let old = b.state as usize;
            if remap[old].is_none() {
                remap[old] = Some(kept.len() as i32);
                kept.push(self.palette[old].clone());
            }
        }
        for b in &mut self.blocks {
            b.state = remap[b.state as usize].expect("every referenced state is kept");
        }
        self.palette = kept;
    }

    /// Serialize back to a gzip-framed structure `.nbt`, deterministically
    /// (mtime 0, fixed compression) — matching the generator/schem emitters.
    pub fn write(&self) -> Vec<u8> {
        let palette: Vec<OutPaletteEntry> = self
            .palette
            .iter()
            .map(|p| OutPaletteEntry {
                name: &p.name,
                properties: if p.properties.is_empty() {
                    None
                } else {
                    Some(&p.properties)
                },
            })
            .collect();
        let blocks: Vec<OutBlock> = self
            .blocks
            .iter()
            .map(|b| OutBlock {
                pos: b.pos,
                state: b.state,
                nbt: b.nbt.as_ref(),
            })
            .collect();
        let out = OutStructure {
            data_version: self.data_version,
            size: self.size,
            palette,
            blocks,
            entities: Vec::new(),
        };
        let nbt = fastnbt::to_bytes(&out).expect("structure serializes to NBT");
        let mut gz = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::new(6));
        gz.write_all(&nbt).expect("gzip write");
        gz.finish().expect("gzip finish")
    }
}

/// The pinned target data version (re-exported for callers building fresh cells).
pub const TARGET_DATA_VERSION: i32 = DATA_VERSION;

fn read_ivec3(v: &Nbt) -> Option<[i32; 3]> {
    let l = v.as_list()?;
    if l.len() != 3 {
        return None;
    }
    Some([l[0].as_i32()?, l[1].as_i32()?, l[2].as_i32()?])
}

fn read_palette_entry(v: &Nbt) -> Result<PaletteEntry, String> {
    let c = v.as_compound().ok_or("palette entry not compound")?;
    let name = c
        .get("Name")
        .and_then(Nbt::as_str)
        .ok_or("palette entry has no Name")?
        .to_string();
    let mut properties = BTreeMap::new();
    if let Some(p) = c.get("Properties").and_then(Nbt::as_compound) {
        for (k, val) in p {
            properties.insert(k.clone(), val.as_str().unwrap_or("").to_string());
        }
    }
    Ok(PaletteEntry { name, properties })
}

// Serialization mirrors the generator/convert output shape exactly.
#[derive(Serialize)]
struct OutStructure<'a> {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<OutPaletteEntry<'a>>,
    blocks: Vec<OutBlock<'a>>,
    entities: Vec<Nbt>,
}

#[derive(Serialize)]
struct OutPaletteEntry<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    properties: Option<&'a BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct OutBlock<'a> {
    pos: [i32; 3],
    state: i32,
    #[serde(rename = "nbt", skip_serializing_if = "Option::is_none")]
    nbt: Option<&'a Nbt>,
}

/// Build a minimal single-cell test structure programmatically (used by tests and
/// as a helper for callers that want to synthesize fixtures without a server).
pub fn synth(size: [i32; 3], cells: &[([i32; 3], PaletteEntry, Option<Nbt>)]) -> Structure {
    let mut s = Structure {
        data_version: DATA_VERSION,
        size,
        palette: Vec::new(),
        blocks: Vec::new(),
        index: BTreeMap::new(),
    };
    // Fill every cell with air first (dense structure, matching real templates).
    let air = s.palette_idx(PaletteEntry::simple("minecraft:air"));
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                let pos = [x, y, z];
                let i = s.blocks.len();
                s.blocks.push(Block {
                    pos,
                    state: air,
                    nbt: None,
                });
                s.index.insert(pos, i);
            }
        }
    }
    for (pos, entry, nbt) in cells {
        s.set_cell(*pos, entry.clone(), nbt.clone());
    }
    s
}

/// Round-trip a structure to bytes and back — helper for tests.
pub fn roundtrip(s: &Structure) -> Result<Structure, String> {
    Structure::read(&s.write())
}

// Re-export the convert-side readback so callers verifying against the schem
// reader can, without a second dependency edge.
pub use convert::read_structure as read_structure_view;
