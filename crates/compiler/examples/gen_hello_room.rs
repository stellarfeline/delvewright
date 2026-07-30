//! Deterministic generator for the M1 `hello-room` prefab.
//!
//! Emits `prefabs/hello-room.nbt`: a vanilla structure template (gzip-framed
//! Java NBT) for a simple enclosed stone room with an inner dividing wall and a
//! 2-wide `minecraft:iron_bars` gate. Reproducible byte-for-byte (ADR-0006): no
//! wall-clock, fixed iteration order, gzip mtime pinned to 0.
//!
//! Run from the repo root:
//!
//! ```text
//! cargo run -p delvewright-compiler --example gen_hello_room
//! ```
//!
//! The anchors this structure provides are declared in `prefabs/hello-room.json`
//! and must be kept in sync with the geometry below.

use std::io::Write as _;
use std::path::PathBuf;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

/// MC 1.21.11 data version (ADR-0009); see `crates/compiler/data/PROVENANCE.md`.
const DATA_VERSION: i32 = 4671;

/// Structure extent: 11 (x) × 6 (y) × 11 (z).
const SIZE: [i32; 3] = [11, 6, 11];

// Palette indices (kept stable — `BlockEntry.state` references these).
const AIR: i32 = 0;
const STONE: i32 = 1;
const IRON_BARS: i32 = 2;

#[derive(Serialize)]
struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<Entity>,
}

#[derive(Serialize)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
}

#[derive(Serialize)]
struct BlockEntry {
    pos: [i32; 3],
    state: i32,
}

/// The structure carries no entities (the NPC is summoned by the datapack).
#[derive(Serialize)]
struct Entity {}

/// Which palette block occupies a local cell. Iteration is x→y→z ascending so
/// the `blocks` list order is fully determined.
fn block_at(x: i32, y: i32, z: i32) -> i32 {
    // Floor and ceiling: solid stone.
    if y == 0 || y == SIZE[1] - 1 {
        return STONE;
    }
    // Outer walls (perimeter of the footprint).
    if x == 0 || x == SIZE[0] - 1 || z == 0 || z == SIZE[2] - 1 {
        return STONE;
    }
    // Inner dividing wall at z == 6, splitting outer chamber (z<6, spawn + keeper)
    // from the inner chamber (z>6, the exit side).
    if z == 6 {
        // 2-wide gate opening at x∈{4,5}, 3 tall (y∈{1,2,3}) filled with iron bars.
        if (x == 4 || x == 5) && (1..=3).contains(&y) {
            return IRON_BARS;
        }
        return STONE;
    }
    AIR
}

fn main() {
    let palette = vec![
        PaletteEntry {
            name: "minecraft:air".to_string(),
        },
        PaletteEntry {
            name: "minecraft:stone".to_string(),
        },
        PaletteEntry {
            name: "minecraft:iron_bars".to_string(),
        },
    ];

    let mut blocks = Vec::with_capacity((SIZE[0] * SIZE[1] * SIZE[2]) as usize);
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: block_at(x, y, z),
                });
            }
        }
    }

    let structure = Structure {
        data_version: DATA_VERSION,
        size: SIZE,
        palette,
        blocks,
        entities: Vec::new(),
    };

    let nbt = fastnbt::to_bytes(&structure).expect("structure serializes to NBT");

    // gzip-frame it (MC reads structure files as gzip-compressed NBT). Pin mtime
    // to 0 and use a fixed compression level for byte-identity.
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip write");
    let framed = gz.finish().expect("gzip finish");

    // repo-root-relative: this example is invoked from the workspace root.
    let out = repo_root().join("prefabs").join("hello-room.nbt");
    std::fs::write(&out, &framed).expect("write prefabs/hello-room.nbt");
    println!(
        "wrote {} ({} blocks, {} gz bytes)",
        out.display(),
        SIZE[0] * SIZE[1] * SIZE[2],
        framed.len()
    );
}

/// The workspace root, resolved from this crate's manifest dir (`crates/compiler`).
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/compiler has a grandparent (repo root)")
        .to_path_buf()
}
