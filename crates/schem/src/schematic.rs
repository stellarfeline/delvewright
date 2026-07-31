//! Sponge schematic (`.schem`) reader — versions 2 and 3.
//!
//! Both versions are gzip-framed Java NBT. The layouts differ:
//!
//! - **v2**: fields live at the top of the root compound. `Palette` maps
//!   block-state strings to indices, `BlockData` is a varint-packed byte array,
//!   `BlockEntities` is a list of `{Id, Pos, ..inline data}` compounds.
//! - **v3**: the root wraps a `Schematic` compound; block data moves under a
//!   `Blocks` compound (`Palette` + `Data` + `BlockEntities`), and each block
//!   entity nests its data under a `Data` compound.
//!
//! Biomes and entities are intentionally ignored in this tool version.

use std::collections::BTreeMap;
use std::io::Read as _;

use crate::nbt::Nbt;

/// A parsed block state: `minecraft:oak_stairs[facing=north,half=bottom]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockState {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}

impl BlockState {
    /// Canonical `name[k=v,...]` form (sorted keys — `BTreeMap`).
    pub fn to_state_string(&self) -> String {
        if self.properties.is_empty() {
            self.name.clone()
        } else {
            let props: Vec<String> = self
                .properties
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            format!("{}[{}]", self.name, props.join(","))
        }
    }

    pub fn air() -> Self {
        BlockState {
            name: "minecraft:air".to_string(),
            properties: BTreeMap::new(),
        }
    }
}

/// A carried-through block entity (position-keyed).
#[derive(Debug, Clone, PartialEq)]
pub struct SchemBlockEntity {
    /// Normalized (namespaced) block-entity id, e.g. `minecraft:chest`.
    pub id: String,
    pub pos: [i32; 3],
    /// The entity's payload (Sponge `Id`/`Pos` stripped; v3 `Data` unwrapped).
    pub data: BTreeMap<String, Nbt>,
}

/// A fully parsed schematic in a version-independent shape.
#[derive(Debug, Clone)]
pub struct ParsedSchematic {
    pub version: i32,
    pub source_data_version: Option<i32>,
    /// `[width, height, length]`.
    pub size: [i32; 3],
    pub offset: [i32; 3],
    pub palette: Vec<BlockState>,
    /// Palette index per cell, indexed by [`ParsedSchematic::at_index`].
    pub blocks: Vec<i32>,
    pub block_entities: Vec<SchemBlockEntity>,
}

impl ParsedSchematic {
    /// x-major flat index: `(x*H + y)*L + z`.
    pub fn at_index(&self, x: i32, y: i32, z: i32) -> usize {
        let [_, h, l] = self.size;
        (((x * h) + y) * l + z) as usize
    }

    /// Palette index at a local cell.
    pub fn at(&self, x: i32, y: i32, z: i32) -> i32 {
        self.blocks[self.at_index(x, y, z)]
    }
}

/// Parse error with a stable, human-readable message.
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseError {}

fn err<T>(msg: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(msg.into()))
}

/// Decompress (gzip if framed, else raw) and parse a Sponge schematic.
pub fn parse_schematic(bytes: &[u8]) -> Result<ParsedSchematic, ParseError> {
    let raw = decompress(bytes)?;
    let value: fastnbt::Value =
        fastnbt::from_bytes(&raw).map_err(|e| ParseError(format!("NBT decode failed: {e}")))?;
    let root = Nbt::from(value);
    let root = root
        .as_compound()
        .ok_or_else(|| ParseError("root NBT tag is not a compound".to_string()))?;

    // v3 wraps everything under a `Schematic` compound; v2 is flat. Descend when
    // the wrapper is present.
    let schem = match root.get("Schematic").and_then(Nbt::as_compound) {
        Some(inner) => inner,
        None => root,
    };

    let version = schem
        .get("Version")
        .and_then(Nbt::as_i32)
        .ok_or_else(|| ParseError("missing `Version` field".to_string()))?;

    match version {
        2 => parse_v2(schem),
        3 => parse_v3(schem),
        other => err(format!(
            "unsupported schematic version {other} (only Sponge v2 and v3 are supported)"
        )),
    }
}

fn decompress(bytes: &[u8]) -> Result<Vec<u8>, ParseError> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .map_err(|e| ParseError(format!("gzip decode failed: {e}")))?;
        Ok(out)
    } else {
        // Tolerate an already-decompressed input.
        Ok(bytes.to_vec())
    }
}

/// Read `Width`/`Height`/`Length` (unsigned shorts) as an `i32` extent.
fn read_dims(schem: &BTreeMap<String, Nbt>) -> Result<[i32; 3], ParseError> {
    let dim = |k: &str| -> Result<i32, ParseError> {
        let raw = schem
            .get(k)
            .and_then(Nbt::as_i32)
            .ok_or_else(|| ParseError(format!("missing/invalid `{k}`")))?;
        // Sponge stores dimensions as unsigned shorts.
        Ok((raw as u16) as i32)
    };
    Ok([dim("Width")?, dim("Height")?, dim("Length")?])
}

fn read_offset(schem: &BTreeMap<String, Nbt>) -> [i32; 3] {
    schem
        .get("Offset")
        .and_then(Nbt::as_i32_array)
        .filter(|a| a.len() == 3)
        .map(|a| [a[0], a[1], a[2]])
        .unwrap_or([0, 0, 0])
}

/// Build the index->state palette from a Sponge `Palette` compound (state ->
/// index). Missing indices (a sparse palette) are rejected.
fn read_palette(pal: &BTreeMap<String, Nbt>) -> Result<Vec<BlockState>, ParseError> {
    let mut entries: Vec<(i32, BlockState)> = Vec::with_capacity(pal.len());
    let mut max = -1;
    for (state, idx) in pal {
        let i = idx
            .as_i32()
            .ok_or_else(|| ParseError(format!("palette index for `{state}` is not an int")))?;
        if i < 0 {
            return err(format!("negative palette index {i} for `{state}`"));
        }
        max = max.max(i);
        entries.push((i, parse_block_state(state)));
    }
    let mut out: Vec<Option<BlockState>> = vec![None; (max + 1) as usize];
    for (i, bs) in entries {
        let slot = &mut out[i as usize];
        if slot.is_some() {
            return err(format!("duplicate palette index {i}"));
        }
        *slot = Some(bs);
    }
    out.into_iter()
        .enumerate()
        .map(|(i, e)| e.ok_or_else(|| ParseError(format!("palette has a gap at index {i}"))))
        .collect()
}

/// Parse `minecraft:oak_stairs[facing=north,half=bottom]` into name + properties.
fn parse_block_state(s: &str) -> BlockState {
    match s.split_once('[') {
        Some((name, rest)) => {
            let inner = rest.strip_suffix(']').unwrap_or(rest);
            let mut properties = BTreeMap::new();
            if !inner.is_empty() {
                for kv in inner.split(',') {
                    if let Some((k, v)) = kv.split_once('=') {
                        properties.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
            }
            BlockState {
                name: name.to_string(),
                properties,
            }
        }
        None => BlockState {
            name: s.to_string(),
            properties: BTreeMap::new(),
        },
    }
}

/// Decode the varint-packed block-data byte array into palette indices, then
/// re-lay it into x-major order. Sponge stores cell `i` in `(y*L + z)*W + x`.
fn decode_blocks(
    packed: &[i8],
    size: [i32; 3],
    palette_len: usize,
) -> Result<Vec<i32>, ParseError> {
    let [w, h, l] = size;
    let count = (w * h * l) as usize;
    let sponge = decode_varints(packed, count)?;
    if sponge.len() != count {
        return err(format!(
            "block data holds {} entries, expected {count} ({w}x{h}x{l})",
            sponge.len()
        ));
    }
    let mut blocks = vec![0i32; count];
    for y in 0..h {
        for z in 0..l {
            for x in 0..w {
                let si = (((y * l) + z) * w + x) as usize;
                let idx = sponge[si];
                if idx < 0 || idx as usize >= palette_len {
                    return err(format!(
                        "block index {idx} out of palette range at {x},{y},{z}"
                    ));
                }
                let dst = (((x * h) + y) * l + z) as usize;
                blocks[dst] = idx;
            }
        }
    }
    Ok(blocks)
}

/// LEB128 unsigned varint decode.
fn decode_varints(bytes: &[i8], expect: usize) -> Result<Vec<i32>, ParseError> {
    let mut out = Vec::with_capacity(expect);
    let mut i = 0;
    while i < bytes.len() {
        let mut value: i32 = 0;
        let mut shift = 0u32;
        loop {
            if i >= bytes.len() {
                return err("truncated varint in block data".to_string());
            }
            let b = bytes[i] as u8;
            i += 1;
            value |= ((b & 0x7f) as i32) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 32 {
                return err("varint too long in block data".to_string());
            }
        }
        out.push(value);
    }
    Ok(out)
}

/// Normalize a block-entity id to `namespace:path` (default `minecraft`).
fn normalize_id(id: &str) -> String {
    if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    }
}

fn read_pos(be: &BTreeMap<String, Nbt>) -> Result<[i32; 3], ParseError> {
    be.get("Pos")
        .and_then(Nbt::as_i32_array)
        .filter(|a| a.len() == 3)
        .map(|a| [a[0], a[1], a[2]])
        .ok_or_else(|| ParseError("block entity missing a 3-int `Pos`".to_string()))
}

fn parse_v2(schem: &BTreeMap<String, Nbt>) -> Result<ParsedSchematic, ParseError> {
    let size = read_dims(schem)?;
    let offset = read_offset(schem);
    let source_data_version = schem.get("DataVersion").and_then(Nbt::as_i32);
    let palette = read_palette(
        schem
            .get("Palette")
            .and_then(Nbt::as_compound)
            .ok_or_else(|| ParseError("v2 schematic missing `Palette`".to_string()))?,
    )?;
    let packed = schem
        .get("BlockData")
        .and_then(Nbt::as_byte_array)
        .ok_or_else(|| ParseError("v2 schematic missing `BlockData`".to_string()))?;
    let blocks = decode_blocks(packed, size, palette.len())?;

    let mut block_entities = Vec::new();
    if let Some(list) = schem.get("BlockEntities").and_then(Nbt::as_list) {
        for entry in list {
            let be = entry
                .as_compound()
                .ok_or_else(|| ParseError("BlockEntities entry is not a compound".to_string()))?;
            let id = be
                .get("Id")
                .and_then(Nbt::as_str)
                .ok_or_else(|| ParseError("v2 block entity missing `Id`".to_string()))?;
            let pos = read_pos(be)?;
            // v2 stores the payload inline; strip the schematic-only keys.
            let mut data = be.clone();
            data.remove("Id");
            data.remove("Pos");
            block_entities.push(SchemBlockEntity {
                id: normalize_id(id),
                pos,
                data,
            });
        }
    }

    Ok(ParsedSchematic {
        version: 2,
        source_data_version,
        size,
        offset,
        palette,
        blocks,
        block_entities,
    })
}

fn parse_v3(schem: &BTreeMap<String, Nbt>) -> Result<ParsedSchematic, ParseError> {
    let size = read_dims(schem)?;
    let offset = read_offset(schem);
    let source_data_version = schem.get("DataVersion").and_then(Nbt::as_i32);
    let blocks_c = schem
        .get("Blocks")
        .and_then(Nbt::as_compound)
        .ok_or_else(|| ParseError("v3 schematic missing `Blocks`".to_string()))?;
    let palette = read_palette(
        blocks_c
            .get("Palette")
            .and_then(Nbt::as_compound)
            .ok_or_else(|| ParseError("v3 schematic missing `Blocks.Palette`".to_string()))?,
    )?;
    let packed = blocks_c
        .get("Data")
        .and_then(Nbt::as_byte_array)
        .ok_or_else(|| ParseError("v3 schematic missing `Blocks.Data`".to_string()))?;
    let blocks = decode_blocks(packed, size, palette.len())?;

    let mut block_entities = Vec::new();
    if let Some(list) = blocks_c.get("BlockEntities").and_then(Nbt::as_list) {
        for entry in list {
            let be = entry
                .as_compound()
                .ok_or_else(|| ParseError("BlockEntities entry is not a compound".to_string()))?;
            let id = be
                .get("Id")
                .and_then(Nbt::as_str)
                .ok_or_else(|| ParseError("v3 block entity missing `Id`".to_string()))?;
            let pos = read_pos(be)?;
            // v3 nests the payload under `Data`.
            let data = be
                .get("Data")
                .and_then(Nbt::as_compound)
                .cloned()
                .unwrap_or_default();
            block_entities.push(SchemBlockEntity {
                id: normalize_id(id),
                pos,
                data,
            });
        }
    }

    Ok(ParsedSchematic {
        version: 3,
        source_data_version,
        size,
        offset,
        palette,
        blocks,
        block_entities,
    })
}
