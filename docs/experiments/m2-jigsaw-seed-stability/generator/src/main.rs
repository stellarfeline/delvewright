//! Experiment jigsaw-piece generator (M2 seed-stability experiment).
//! Emits tiny 7x5x7 enclosed structure pieces with jigsaw connectors + a
//! distinct center marker block, into <out>/data/dwexp/structure/*.nbt.
//! Deterministic: fixed iteration, gzip mtime 0, fixed compression.

use std::collections::BTreeMap;
use std::io::Write as _;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

const DATA_VERSION: i32 = 4671; // MC 1.21.11
const SX: i32 = 7;
const SY: i32 = 5;
const SZ: i32 = 7;

#[derive(Serialize)]
struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<Entity>,
}

#[derive(Serialize, Clone, PartialEq)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct BlockEntry {
    pos: [i32; 3],
    state: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    nbt: Option<JigsawBE>,
}

#[derive(Serialize, Clone)]
struct JigsawBE {
    id: String,
    name: String,
    target: String,
    pool: String,
    final_state: String,
    joint: String,
}

#[derive(Serialize)]
struct Entity {}

/// A jigsaw connector to embed in a wall.
struct Jig {
    pos: [i32; 3],
    orientation: &'static str,
    name: &'static str,
    target: &'static str,
    pool: &'static str,
}

struct Palette {
    entries: Vec<PaletteEntry>,
}
impl Palette {
    fn new() -> Self {
        Palette { entries: Vec::new() }
    }
    fn idx(&mut self, name: &str, props: Option<BTreeMap<String, String>>) -> i32 {
        let e = PaletteEntry {
            name: name.to_string(),
            properties: props,
        };
        if let Some(i) = self.entries.iter().position(|x| *x == e) {
            return i as i32;
        }
        self.entries.push(e);
        (self.entries.len() - 1) as i32
    }
}

fn build_piece(marker: &str, jigs: &[Jig]) -> Structure {
    let mut pal = Palette::new();
    let air = pal.idx("minecraft:air", None);
    let stone = pal.idx("minecraft:stone", None);
    let marker_i = pal.idx(marker, None);
    let lapis_i = pal.idx("minecraft:lapis_block", None);
    // Ceiling-hung lantern to light interior (mirrors hello-room approach).
    let mut lprops = BTreeMap::new();
    lprops.insert("hanging".to_string(), "true".to_string());
    let lantern_i = pal.idx("minecraft:lantern", Some(lprops));

    // Pre-register jigsaw palette entries (one per orientation) and remember idx.
    let jig_idx: Vec<i32> = jigs
        .iter()
        .map(|j| {
            let mut p = BTreeMap::new();
            p.insert("orientation".to_string(), j.orientation.to_string());
            pal.idx("minecraft:jigsaw", Some(p))
        })
        .collect();

    let mut blocks = Vec::new();
    for x in 0..SX {
        for y in 0..SY {
            for z in 0..SZ {
                // Is this a jigsaw position? (overrides wall)
                if let Some(k) = jigs.iter().position(|j| j.pos == [x, y, z]) {
                    let j = &jigs[k];
                    blocks.push(BlockEntry {
                        pos: [x, y, z],
                        state: jig_idx[k],
                        nbt: Some(JigsawBE {
                            id: "minecraft:jigsaw".to_string(),
                            name: j.name.to_string(),
                            target: j.target.to_string(),
                            pool: j.pool.to_string(),
                            final_state: "minecraft:air".to_string(),
                            joint: "rollable".to_string(),
                        }),
                    });
                    continue;
                }
                let shell =
                    y == 0 || y == SY - 1 || x == 0 || x == SX - 1 || z == 0 || z == SZ - 1;
                let state = if x == 3 && y == 1 && z == 3 {
                    marker_i // center marker (piece identity)
                } else if x == 3 && y == 1 && z == 2 {
                    lapis_i // nose marker (orientation), toward north/entrance side
                } else if x == 3 && y == 3 && z == 3 {
                    lantern_i // ceiling-hung lantern
                } else if shell {
                    stone
                } else {
                    air
                };
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state,
                    nbt: None,
                });
            }
        }
    }

    Structure {
        data_version: DATA_VERSION,
        size: [SX, SY, SZ],
        palette: pal.entries,
        blocks,
        entities: Vec::new(),
    }
}

fn write_nbt(dir: &std::path::Path, id: &str, s: &Structure) {
    let nbt = fastnbt::to_bytes(s).expect("nbt");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gz");
    let framed = gz.finish().expect("finish");
    let out = dir.join(format!("{id}.nbt"));
    std::fs::write(&out, &framed).expect("write");
    println!("wrote {} ({} bytes)", out.display(), framed.len());
}

fn main() {
    let out = std::env::args().nth(1).expect("usage: gen <datapack_dir>");
    let dir = std::path::PathBuf::from(out).join("data/dwexp/structure");
    std::fs::create_dir_all(&dir).expect("mkdir");

    // Connector convention: entrance jigsaw on north wall (z=0) facing north,
    // name="dwexp:entrance", pool="minecraft:empty" (female, never expands).
    // Exit jigsaws face outward, name="dwexp:exit", target="dwexp:entrance",
    // pool="dwexp:main" (male, drives expansion). All at floor level y=1.
    let entrance = |()| Jig {
        pos: [3, 1, 0],
        orientation: "north_up",
        name: "dwexp:entrance",
        target: "minecraft:empty",
        pool: "minecraft:empty",
    };
    let exit_s = Jig {
        pos: [3, 1, SZ - 1],
        orientation: "south_up",
        name: "dwexp:exit",
        target: "dwexp:entrance",
        pool: "dwexp:main",
    };
    let exit_e = Jig {
        pos: [SX - 1, 1, 3],
        orientation: "east_up",
        name: "dwexp:exit",
        target: "dwexp:entrance",
        pool: "dwexp:main",
    };
    let exit_w = Jig {
        pos: [0, 1, 3],
        orientation: "west_up",
        name: "dwexp:exit",
        target: "dwexp:entrance",
        pool: "dwexp:main",
    };

    // room_iron: entrance + S + E exits
    write_nbt(
        &dir,
        "room_iron",
        &build_piece("minecraft:iron_block", &[entrance(()), exit_s, exit_e]),
    );
    // room_diamond: entrance + S + W exits (fresh exits; exit_s moved above)
    write_nbt(
        &dir,
        "room_diamond",
        &build_piece(
            "minecraft:diamond_block",
            &[
                entrance(()),
                Jig {
                    pos: [3, 1, SZ - 1],
                    orientation: "south_up",
                    name: "dwexp:exit",
                    target: "dwexp:entrance",
                    pool: "dwexp:main",
                },
                exit_w,
            ],
        ),
    );
    // corridor: entrance + S exit
    write_nbt(
        &dir,
        "corridor",
        &build_piece(
            "minecraft:redstone_block",
            &[
                entrance(()),
                Jig {
                    pos: [3, 1, SZ - 1],
                    orientation: "south_up",
                    name: "dwexp:exit",
                    target: "dwexp:entrance",
                    pool: "dwexp:main",
                },
            ],
        ),
    );
    // deadend: entrance only (terminal)
    write_nbt(
        &dir,
        "deadend",
        &build_piece("minecraft:emerald_block", &[entrance(())]),
    );
}
