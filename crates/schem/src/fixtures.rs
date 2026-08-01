//! Hand-crafted reference schematics, built in code (no network fetches).
//!
//! These back the round-trip tests and double as living documentation of the
//! Sponge v2/v3 byte layouts the parser accepts. Everything is deterministic.

use std::collections::BTreeMap;

use flate2::{Compression, GzBuilder};
use std::io::Write as _;

use crate::nbt::Nbt;

/// A block entity to embed in a fixture.
pub struct FixtureBe {
    pub id: &'static str,
    pub pos: [i32; 3],
    pub data: BTreeMap<String, Nbt>,
}

fn encode_varint(mut value: u32, out: &mut Vec<i8>) {
    loop {
        let mut b = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            b |= 0x80;
        }
        out.push(b as i8);
        if value == 0 {
            break;
        }
    }
}

/// Varint-pack palette indices in Sponge cell order `(y*L + z)*W + x`.
fn pack_block_data(size: [i32; 3], block_at: &dyn Fn(i32, i32, i32) -> usize) -> Vec<i8> {
    let [w, h, l] = size;
    let mut out = Vec::new();
    for y in 0..h {
        for z in 0..l {
            for x in 0..w {
                encode_varint(block_at(x, y, z) as u32, &mut out);
            }
        }
    }
    out
}

fn palette_compound(states: &[&str]) -> Nbt {
    let mut map = BTreeMap::new();
    for (i, s) in states.iter().enumerate() {
        map.insert((*s).to_string(), Nbt::Int(i as i32));
    }
    Nbt::Compound(map)
}

fn gzip(nbt: &Nbt) -> Vec<u8> {
    let raw = fastnbt::to_bytes(nbt).expect("fixture serializes to NBT");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&raw).expect("gzip write");
    gz.finish().expect("gzip finish")
}

/// Build a Sponge **v2** schematic (flat root, inline block-entity data).
pub fn build_v2(
    size: [i32; 3],
    palette: &[&str],
    block_at: &dyn Fn(i32, i32, i32) -> usize,
    bes: &[FixtureBe],
) -> Vec<u8> {
    build_v2_versioned(4671, size, palette, block_at, bes)
}

/// Like [`build_v2`], but with an explicit source `DataVersion` — used to
/// synthesize a schematic whose `DataVersion` differs from the pinned MC
/// 1.21.11 target (`DW0702`).
pub fn build_v2_versioned(
    data_version: i32,
    size: [i32; 3],
    palette: &[&str],
    block_at: &dyn Fn(i32, i32, i32) -> usize,
    bes: &[FixtureBe],
) -> Vec<u8> {
    let mut root = BTreeMap::new();
    root.insert("Version".to_string(), Nbt::Int(2));
    root.insert("DataVersion".to_string(), Nbt::Int(data_version));
    root.insert("Width".to_string(), Nbt::Short(size[0] as i16));
    root.insert("Height".to_string(), Nbt::Short(size[1] as i16));
    root.insert("Length".to_string(), Nbt::Short(size[2] as i16));
    root.insert("Offset".to_string(), Nbt::IntArray(vec![0, 0, 0]));
    root.insert("PaletteMax".to_string(), Nbt::Int(palette.len() as i32));
    root.insert("Palette".to_string(), palette_compound(palette));
    root.insert(
        "BlockData".to_string(),
        Nbt::ByteArray(pack_block_data(size, block_at)),
    );
    if !bes.is_empty() {
        let list = bes
            .iter()
            .map(|be| {
                // v2: `Id` + `Pos` + inline payload.
                let mut c = be.data.clone();
                c.insert("Id".to_string(), Nbt::String(be.id.to_string()));
                c.insert("Pos".to_string(), Nbt::IntArray(be.pos.to_vec()));
                Nbt::Compound(c)
            })
            .collect();
        root.insert("BlockEntities".to_string(), Nbt::List(list));
    }
    gzip(&Nbt::Compound(root))
}

/// Build a Sponge **v3** schematic (root wraps `Schematic`; block data under
/// `Blocks`; block-entity payload nested under `Data`).
pub fn build_v3(
    size: [i32; 3],
    palette: &[&str],
    block_at: &dyn Fn(i32, i32, i32) -> usize,
    bes: &[FixtureBe],
) -> Vec<u8> {
    let mut blocks = BTreeMap::new();
    blocks.insert("Palette".to_string(), palette_compound(palette));
    blocks.insert(
        "Data".to_string(),
        Nbt::ByteArray(pack_block_data(size, block_at)),
    );
    if !bes.is_empty() {
        let list = bes
            .iter()
            .map(|be| {
                let mut c = BTreeMap::new();
                c.insert("Id".to_string(), Nbt::String(be.id.to_string()));
                c.insert("Pos".to_string(), Nbt::IntArray(be.pos.to_vec()));
                c.insert("Data".to_string(), Nbt::Compound(be.data.clone()));
                Nbt::Compound(c)
            })
            .collect();
        blocks.insert("BlockEntities".to_string(), Nbt::List(list));
    }

    let mut schem = BTreeMap::new();
    schem.insert("Version".to_string(), Nbt::Int(3));
    schem.insert("DataVersion".to_string(), Nbt::Int(4671));
    schem.insert("Width".to_string(), Nbt::Short(size[0] as i16));
    schem.insert("Height".to_string(), Nbt::Short(size[1] as i16));
    schem.insert("Length".to_string(), Nbt::Short(size[2] as i16));
    schem.insert("Offset".to_string(), Nbt::IntArray(vec![0, 0, 0]));
    schem.insert("Blocks".to_string(), Nbt::Compound(blocks));

    let mut root = BTreeMap::new();
    root.insert("Schematic".to_string(), Nbt::Compound(schem));
    gzip(&Nbt::Compound(root))
}

// --- Named reference fixtures -------------------------------------------------

/// A 3x3x3 room: stone shell, air interior. Palette `[air, stone]`.
pub fn basic_block_at(x: i32, y: i32, z: i32) -> usize {
    if x == 0 || x == 2 || y == 0 || y == 2 || z == 0 || z == 2 {
        1 // stone
    } else {
        0 // air
    }
}

const BASIC_PALETTE: [&str; 2] = ["minecraft:air", "minecraft:stone"];

pub fn v2_basic() -> Vec<u8> {
    build_v2([3, 3, 3], &BASIC_PALETTE, &basic_block_at, &[])
}

/// The basic room, but with a source `DataVersion` that is not the pinned MC
/// 1.21.11 target (`DW0702`).
pub fn v2_wrong_data_version() -> Vec<u8> {
    build_v2_versioned(3700, [3, 3, 3], &BASIC_PALETTE, &basic_block_at, &[])
}

pub fn v3_basic() -> Vec<u8> {
    build_v3([3, 3, 3], &BASIC_PALETTE, &basic_block_at, &[])
}

/// Palette for the block-entity fixture: air, a facing chest, a command block.
const BE_PALETTE: [&str; 3] = [
    "minecraft:air",
    "minecraft:chest[facing=north]",
    "minecraft:command_block[conditional=false]",
];

/// A flat 3x1x3 slab: chest at (0,0,0), command block at (2,0,0), else air.
pub fn be_block_at(x: i32, _y: i32, _z: i32) -> usize {
    match x {
        0 => 1, // chest
        2 => 2, // command block
        _ => 0, // air
    }
}

fn be_fixtures() -> Vec<FixtureBe> {
    // A chest holding one item (carried through opaquely).
    let mut item = BTreeMap::new();
    item.insert(
        "id".to_string(),
        Nbt::String("minecraft:diamond".to_string()),
    );
    item.insert("count".to_string(), Nbt::Int(3));
    item.insert("Slot".to_string(), Nbt::Byte(0));
    let mut chest = BTreeMap::new();
    chest.insert("Items".to_string(), Nbt::List(vec![Nbt::Compound(item)]));

    // A command block (must be stripped: forbidden block, forbidden BE id, and a
    // `Command` payload — all three strip paths).
    let mut cmd = BTreeMap::new();
    cmd.insert("Command".to_string(), Nbt::String("say pwned".to_string()));
    cmd.insert("auto".to_string(), Nbt::Byte(0));

    vec![
        FixtureBe {
            id: "minecraft:chest",
            pos: [0, 0, 0],
            data: chest,
        },
        FixtureBe {
            id: "minecraft:command_block",
            pos: [2, 0, 0],
            data: cmd,
        },
    ]
}

pub fn v2_block_entities() -> Vec<u8> {
    build_v2([3, 1, 3], &BE_PALETTE, &be_block_at, &be_fixtures())
}

pub fn v3_block_entities() -> Vec<u8> {
    build_v3([3, 1, 3], &BE_PALETTE, &be_block_at, &be_fixtures())
}

/// A 60x10x60 checker of stone/air — larger than the 48-cube cap on x/z.
pub fn oversize_block_at(x: i32, y: i32, z: i32) -> usize {
    ((x + y + z) % 2 == 0) as usize
}

pub fn v2_oversize() -> Vec<u8> {
    build_v2([60, 10, 60], &BASIC_PALETTE, &oversize_block_at, &[])
}
