//! Schematic region -> vanilla structure `.nbt`.
//!
//! Emits the same structure-template shape the compiler's `gen_hello_room`
//! writes (`DataVersion`, `size`, `palette`, `blocks`, `entities`), so the
//! output drops straight into the prefab library. The safety strip
//! (spec-0007 community contract) runs here: command/structure/jigsaw blocks and
//! NBT-bearing spawners are replaced with air and reported.

use std::collections::BTreeMap;
use std::io::Write as _;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

use crate::diag::{DW_STRIP, Diagnostic};
use crate::nbt::Nbt;
use crate::schematic::{BlockState, ParsedSchematic};

/// MC 1.21.11 structure `DataVersion` (ADR-0009; verified against
/// `crates/compiler/data/PROVENANCE.md` and the committed `hello-room.nbt`).
pub const DATA_VERSION: i32 = 4671;

/// Block names (namespace-stripped) removed unconditionally: the three command
/// blocks, structure/jigsaw blocks, and the spawner family.
const FORBIDDEN_BLOCKS: &[&str] = &[
    "command_block",
    "chain_command_block",
    "repeating_command_block",
    "structure_block",
    "jigsaw",
    "spawner",
    "trial_spawner",
    "vault",
];

/// Block-entity ids removed unconditionally.
const FORBIDDEN_BE: &[&str] = &[
    "command_block",
    "chain_command_block",
    "repeating_command_block",
    "structure_block",
    "jigsaw",
    "mob_spawner",
    "trial_spawner",
    "vault",
];

fn strip_ns(id: &str) -> &str {
    id.split_once(':').map(|(_, p)| p).unwrap_or(id)
}

/// If a block-entity payload carries a command or a spawner definition anywhere
/// in its NBT, return a human reason (the code-injection audit surface).
fn forbidden_nbt(data: &BTreeMap<String, Nbt>) -> Option<&'static str> {
    for (k, v) in data {
        match k.as_str() {
            "Command" => return Some("embedded command"),
            "SpawnData" | "SpawnPotentials" | "spawn_data" | "spawn_potentials" => {
                return Some("spawner definition");
            }
            _ => {}
        }
        if let Some(reason) = scan_value(v) {
            return Some(reason);
        }
    }
    None
}

fn scan_value(v: &Nbt) -> Option<&'static str> {
    match v {
        Nbt::Compound(m) => forbidden_nbt(m),
        Nbt::List(items) => items.iter().find_map(scan_value),
        _ => None,
    }
}

#[derive(Serialize)]
struct OutStructure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<OutPaletteEntry>,
    blocks: Vec<OutBlock>,
    entities: Vec<Nbt>,
}

#[derive(Serialize)]
struct OutPaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct OutBlock {
    pos: [i32; 3],
    state: i32,
    #[serde(rename = "nbt", skip_serializing_if = "Option::is_none")]
    nbt: Option<Nbt>,
}

/// Build one gzip-framed structure `.nbt` for a `size`-extent region whose local
/// origin maps to `origin` in the schematic. Strip diagnostics (position rebased
/// to the region-local frame) are appended to `diags`.
pub fn build_region(
    schem: &ParsedSchematic,
    origin: [i32; 3],
    size: [i32; 3],
    diags: &mut Vec<Diagnostic>,
) -> Vec<u8> {
    let [w, h, l] = size;

    // Position-keyed block entities for O(1) lookup by source cell.
    let be_by_pos: std::collections::HashMap<[i32; 3], &crate::schematic::SchemBlockEntity> =
        schem.block_entities.iter().map(|be| (be.pos, be)).collect();

    // First pass: decide the final state + carried nbt for each local cell,
    // running the strip. Iterate x -> y -> z (matches the compiler's prefab
    // generator) for a deterministic block list.
    struct Cell {
        state: BlockState,
        nbt: Option<Nbt>,
    }
    let air = BlockState::air();
    let mut cells: Vec<Cell> = Vec::with_capacity((w * h * l) as usize);

    for x in 0..w {
        for y in 0..h {
            for z in 0..l {
                let sx = origin[0] + x;
                let sy = origin[1] + y;
                let sz = origin[2] + z;
                let idx = schem.at(sx, sy, sz);
                let state = &schem.palette[idx as usize];
                let be = be_by_pos.get(&[sx, sy, sz]).copied();

                // Strip decision.
                let mut strip_reason: Option<String> = None;
                if FORBIDDEN_BLOCKS.contains(&strip_ns(&state.name)) {
                    strip_reason = Some(format!("block {}", state.name));
                } else if let Some(be) = be {
                    if FORBIDDEN_BE.contains(&strip_ns(&be.id)) {
                        strip_reason = Some(format!("block entity {}", be.id));
                    } else if let Some(reason) = forbidden_nbt(&be.data) {
                        strip_reason = Some(format!("{} in {}", reason, be.id));
                    }
                }

                if let Some(reason) = strip_reason {
                    diags.push(
                        Diagnostic::warning(DW_STRIP, format!("stripped {reason}")).at([x, y, z]),
                    );
                    cells.push(Cell {
                        state: air.clone(),
                        nbt: None,
                    });
                    continue;
                }

                // Carry a surviving block entity through, rebasing to structure
                // form: drop schematic-only keys, set the lowercase `id`.
                let nbt = be.map(|be| {
                    let mut data = be.data.clone();
                    data.remove("x");
                    data.remove("y");
                    data.remove("z");
                    data.insert("id".to_string(), Nbt::String(be.id.clone()));
                    Nbt::Compound(data)
                });
                cells.push(Cell {
                    state: state.clone(),
                    nbt,
                });
            }
        }
    }

    // Build a deterministic output palette: distinct states sorted by canonical
    // string, remapped to indices.
    let mut states: Vec<BlockState> = Vec::new();
    {
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for c in &cells {
            if seen.insert(c.state.to_state_string()) {
                states.push(c.state.clone());
            }
        }
        states.sort_by_key(|s| s.to_state_string());
    }
    let index_of: BTreeMap<String, i32> = states
        .iter()
        .enumerate()
        .map(|(i, s)| (s.to_state_string(), i as i32))
        .collect();

    let palette: Vec<OutPaletteEntry> = states
        .iter()
        .map(|s| OutPaletteEntry {
            name: s.name.clone(),
            properties: if s.properties.is_empty() {
                None
            } else {
                Some(s.properties.clone())
            },
        })
        .collect();

    // Emit blocks in the same x -> y -> z order they were built.
    let mut blocks: Vec<OutBlock> = Vec::with_capacity(cells.len());
    let mut ci = 0usize;
    for x in 0..w {
        for y in 0..h {
            for z in 0..l {
                let c = &cells[ci];
                ci += 1;
                blocks.push(OutBlock {
                    pos: [x, y, z],
                    state: index_of[&c.state.to_state_string()],
                    nbt: c.nbt.clone(),
                });
            }
        }
    }

    let structure = OutStructure {
        data_version: DATA_VERSION,
        size,
        palette,
        blocks,
        entities: Vec::new(),
    };

    let nbt = fastnbt::to_bytes(&structure).expect("structure serializes to NBT");
    gzip_frame(&nbt)
}

/// gzip-frame NBT deterministically (mtime 0, fixed level) — matches the
/// compiler's prefab emitter for byte-identity (ADR-0006).
fn gzip_frame(nbt: &[u8]) -> Vec<u8> {
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(nbt).expect("gzip write");
    gz.finish().expect("gzip finish")
}

/// Full input palette as canonical block-state strings, sorted and de-duplicated
/// — the `--palette-report` audit feed.
pub fn palette_report(schem: &ParsedSchematic) -> Vec<String> {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for bs in &schem.palette {
        set.insert(bs.to_state_string());
    }
    set.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Readback (verification / reassembly) — parses a structure `.nbt` we emitted.
// ---------------------------------------------------------------------------

/// A parsed vanilla structure template, for tests and split reassembly.
#[derive(Debug, Clone)]
pub struct StructureView {
    pub data_version: i32,
    pub size: [i32; 3],
    /// State string per cell in x-major order (`(x*H + y)*L + z`).
    pub cells: Vec<String>,
    /// Carried block-entity nbt per cell (present only where one exists).
    pub block_entities: BTreeMap<[i32; 3], Nbt>,
}

impl StructureView {
    pub fn at_index(&self, x: i32, y: i32, z: i32) -> usize {
        let [_, h, l] = self.size;
        (((x * h) + y) * l + z) as usize
    }

    pub fn state_at(&self, x: i32, y: i32, z: i32) -> &str {
        &self.cells[self.at_index(x, y, z)]
    }
}

/// Parse a gzip-framed structure `.nbt` (as this tool emits it).
pub fn read_structure(bytes: &[u8]) -> Result<StructureView, String> {
    let mut raw = Vec::new();
    {
        use std::io::Read as _;
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut raw)
            .map_err(|e| format!("gzip decode: {e}"))?;
    }
    let value: fastnbt::Value = fastnbt::from_bytes(&raw).map_err(|e| format!("nbt: {e}"))?;
    let root = Nbt::from(value);
    let root = root.as_compound().ok_or("root not compound")?;
    let data_version = root
        .get("DataVersion")
        .and_then(Nbt::as_i32)
        .ok_or("no DataVersion")?;
    let size = root
        .get("size")
        .and_then(Nbt::as_list)
        .and_then(|l| {
            if l.len() == 3 {
                Some([l[0].as_i32()?, l[1].as_i32()?, l[2].as_i32()?])
            } else {
                None
            }
        })
        .ok_or("bad size")?;
    let [_, h, l] = size;

    let palette: Vec<String> = root
        .get("palette")
        .and_then(Nbt::as_list)
        .ok_or("no palette")?
        .iter()
        .map(|e| {
            let c = e.as_compound().ok_or("palette entry not compound")?;
            let name = c.get("Name").and_then(Nbt::as_str).ok_or("no Name")?;
            let mut props: BTreeMap<String, String> = BTreeMap::new();
            if let Some(p) = c.get("Properties").and_then(Nbt::as_compound) {
                for (k, v) in p {
                    props.insert(k.clone(), v.as_str().unwrap_or("").to_string());
                }
            }
            Ok(BlockState {
                name: name.to_string(),
                properties: props,
            }
            .to_state_string())
        })
        .collect::<Result<_, String>>()?;

    let count = (size[0] * h * l) as usize;
    let mut cells = vec![String::new(); count];
    let mut block_entities = BTreeMap::new();
    for b in root
        .get("blocks")
        .and_then(Nbt::as_list)
        .ok_or("no blocks")?
    {
        let c = b.as_compound().ok_or("block not compound")?;
        let pos = c
            .get("pos")
            .and_then(Nbt::as_list)
            .and_then(|p| {
                if p.len() == 3 {
                    Some([p[0].as_i32()?, p[1].as_i32()?, p[2].as_i32()?])
                } else {
                    None
                }
            })
            .ok_or("bad pos")?;
        let state = c.get("state").and_then(Nbt::as_i32).ok_or("no state")? as usize;
        let idx = (((pos[0] * h) + pos[1]) * l + pos[2]) as usize;
        cells[idx] = palette.get(state).cloned().ok_or("state out of range")?;
        if let Some(nbt) = c.get("nbt") {
            block_entities.insert(pos, nbt.clone());
        }
    }

    Ok(StructureView {
        data_version,
        size,
        cells,
        block_entities,
    })
}
