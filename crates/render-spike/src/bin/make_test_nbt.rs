//! SPIKE helper: emit a gzip-framed vanilla structure `.nbt` packed with
//! 1.21.x-era blocks, to probe Nucleation texture fidelity for the newest
//! Minecraft content. Same serialization shape as prefabs/generator.
//!
//! Layout: a stone-brick floor with a row of "showcase" blocks on top, each
//! separated by air, so every block face is visible from an angle.
//!
//! Usage: make-test-nbt <out.nbt>

use std::collections::BTreeMap;
use std::io::Write as _;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

const DATA_VERSION: i32 = 4671; // MC 1.21.11

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
}
#[derive(Serialize)]
struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<()>,
}

struct Palette {
    entries: Vec<PaletteEntry>,
}
impl Palette {
    fn idx(&mut self, name: &str, props: &[(&str, &str)]) -> i32 {
        let properties = if props.is_empty() {
            None
        } else {
            Some(
                props
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<BTreeMap<_, _>>(),
            )
        };
        let e = PaletteEntry {
            name: name.to_string(),
            properties,
        };
        if let Some(i) = self.entries.iter().position(|x| *x == e) {
            return i as i32;
        }
        self.entries.push(e);
        (self.entries.len() - 1) as i32
    }
}

fn main() {
    let out = std::env::args().nth(1).expect("usage: make-test-nbt <out.nbt>");

    // The showcase row: newest-era blocks first, then common keep blocks.
    // (name, properties) — properties chosen to exercise stateful models.
    let showcase: &[(&str, &[(&str, &str)])] = &[
        // --- 1.21.x-era ("newest") ---
        ("minecraft:pale_oak_planks", &[]),
        ("minecraft:pale_oak_log", &[("axis", "y")]),
        ("minecraft:crafter", &[("orientation", "north_up"), ("crafting", "false"), ("triggered", "false")]),
        ("minecraft:copper_bulb", &[("lit", "true"), ("powered", "false")]),
        ("minecraft:copper_grate", &[]),
        ("minecraft:waxed_copper_grate", &[]),
        ("minecraft:tuff_bricks", &[]),
        ("minecraft:chiseled_tuff", &[]),
        ("minecraft:polished_tuff", &[]),
        ("minecraft:tuff_brick_stairs", &[("facing", "east"), ("half", "bottom"), ("shape", "straight")]),
        ("minecraft:trial_spawner", &[]),
        ("minecraft:vault", &[]),
        ("minecraft:heavy_core", &[]),
        ("minecraft:chiseled_copper", &[]),
        ("minecraft:copper_door", &[("facing", "north"), ("half", "lower"), ("hinge", "left"), ("open", "false"), ("powered", "false")]),
        // --- common keep blocks (sanity baseline) ---
        ("minecraft:stone_bricks", &[]),
        ("minecraft:chiseled_stone_bricks", &[]),
        ("minecraft:glowstone", &[]),
        ("minecraft:iron_bars", &[]),
    ];

    let depth = showcase.len() as i32;
    let size = [3, 2, depth];

    let mut pal = Palette { entries: vec![] };
    let air = pal.idx("minecraft:air", &[]);
    let floor = pal.idx("minecraft:stone_bricks", &[]);

    let mut blocks: Vec<BlockEntry> = Vec::new();
    // Fill everything with air first (y=0 floor, y=1 showcase).
    for z in 0..depth {
        for x in 0..3 {
            // floor
            blocks.push(BlockEntry { pos: [x, 0, z], state: floor });
            // default air above
            let _ = air;
        }
        // showcase block at center column x=1, y=1
        let (name, props) = showcase[z as usize];
        let s = pal.idx(name, props);
        blocks.push(BlockEntry { pos: [1, 1, z], state: s });
    }

    let structure = Structure {
        data_version: DATA_VERSION,
        size,
        palette: pal.entries,
        blocks,
        entities: vec![],
    };

    let nbt = fastnbt::to_bytes(&structure).expect("serialize nbt");
    let mut gz = GzBuilder::new().mtime(0).write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip");
    let bytes = gz.finish().expect("finish gzip");
    std::fs::write(&out, &bytes).expect("write");
    eprintln!(
        "wrote {out} ({} bytes, {} showcase blocks)",
        bytes.len(),
        showcase.len()
    );
}
