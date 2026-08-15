//! Deterministic generator for the M1 `hello-room` prefab.
//!
//! Emits `<out_dir>/hello-room.nbt`: a vanilla structure template (gzip-framed
//! Java NBT) for a simple enclosed stone room with an inner dividing wall and a
//! 2-wide `minecraft:iron_bars` gate. The interior is lit by ceiling-hung
//! `minecraft:lantern`s (two per chamber) so every walkable floor block clears
//! the lighting contract's `lit` bar (floor light >= 7); measured on a live
//! 1.21.11 server, see the piece's `hello-room.json` `lighting` block.
//! Reproducible byte-for-byte (ADR-0006): no wall-clock, fixed iteration order,
//! gzip mtime pinned to 0.
//!
//! ```text
//! cargo run --release --manifest-path prefabs/hello-room-generator/Cargo.toml -- <out_dir>
//! ```
//!
//! `<out_dir>` is the prefab library — the content repo's `prefabs/`. It must
//! already exist: a generator that creates its destination turns a mistyped path
//! into a directory nobody reads, which is how this piece spent six tilesets
//! writing to an engine-repo directory that has held no `.nbt` since the library
//! moved out.
//!
//! The anchors this structure provides are declared beside it in
//! `hello-room.json` and must be kept in sync with the geometry below.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

#[path = "../../invariants.rs"]
mod invariants;

/// The connection derivation, shared the same way: what a fence, a wall, a pane
/// or a lichen joins is computed from the blocks beside it, at the emitter.
#[path = "../../connections.rs"]
mod connections;

/// MC 1.21.11 data version (ADR-0009); see `crates/compiler/data/PROVENANCE.md`.
const DATA_VERSION: i32 = 4671;

/// The piece's id — the `.nbt` stem and what the invariants report against.
const ID: &str = "hello-room";

/// Structure extent: 11 (x) × 6 (y) × 11 (z).
const SIZE: [i32; 3] = [11, 6, 11];

/// Ceiling-hung lanterns (the lighting contract). Each sits at y == 4 with the
/// stone ceiling (y == 5) directly above so `hanging=true` is supported; two per
/// chamber (outer z<6 spawn/keeper side, inner z>6 exit side) light every
/// walkable floor block to >= 7 with margin (measured min recorded in the prefab
/// metadata). Kept in ascending x→y→z order for a deterministic block list.
const LANTERNS: [[i32; 3]; 4] = [[3, 4, 3], [3, 4, 8], [7, 4, 3], [7, 4, 8]];

#[derive(Serialize)]
struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<Entity>,
}

#[derive(Serialize, PartialEq, Eq, Clone)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    /// Blockstate properties. `BTreeMap` keeps property order deterministic
    /// (ADR-0006); `None` only for a block that has no properties at all.
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct BlockEntry {
    pos: [i32; 3],
    state: i32,
}

/// The structure carries no entities (the NPC is summoned by the datapack).
#[derive(Serialize)]
struct Entity {}

/// The palette, with its single insertion point — the same shape as every other
/// tileset generator's.
struct Palette {
    entries: Vec<PaletteEntry>,
}

impl Palette {
    fn new() -> Self {
        Palette { entries: vec![] }
    }

    fn idx(&mut self, name: &str, props: Option<&[(&str, &str)]>) -> i32 {
        let written: BTreeMap<String, String> = props
            .unwrap_or(&[])
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        let e = PaletteEntry {
            name: name.to_string(),
            properties: if written.is_empty() {
                None
            } else {
                Some(written)
            },
        };
        if let Some(i) = self.entries.iter().position(|x| *x == e) {
            return i as i32;
        }
        self.entries.push(e);
        (self.entries.len() - 1) as i32
    }
}

/// Which block occupies a local cell, as a name + the properties this generator
/// decides. Everything it leaves unsaid is filled by [`Palette::idx`].
fn block_at(
    x: i32,
    y: i32,
    z: i32,
) -> (
    &'static str,
    Option<&'static [(&'static str, &'static str)]>,
) {
    // Floor and ceiling: solid stone.
    if y == 0 || y == SIZE[1] - 1 {
        return ("minecraft:stone", None);
    }
    // Outer walls (perimeter of the footprint).
    if x == 0 || x == SIZE[0] - 1 || z == 0 || z == SIZE[2] - 1 {
        return ("minecraft:stone", None);
    }
    // Inner dividing wall at z == 6, splitting outer chamber (z<6, spawn + keeper)
    // from the inner chamber (z>6, the exit side).
    if z == 6 {
        // 2-wide gate opening at x∈{4,5}, 3 tall (y∈{1,2,3}) filled with iron bars.
        if (x == 4 || x == 5) && (1..=3).contains(&y) {
            return ("minecraft:iron_bars", None);
        }
        return ("minecraft:stone", None);
    }
    // Ceiling-hung lanterns light the interior.
    if LANTERNS.iter().any(|p| p == &[x, y, z]) {
        // `hanging=true` renders it suspended from the stone above (and is the
        // state that survives a block update in that position).
        return ("minecraft:lantern", Some(&[("hanging", "true")]));
    }
    ("minecraft:air", None)
}

fn build() -> Structure {
    let mut palette = Palette::new();
    // Registered up front, in this order, so the emitted palette keeps the
    // order the piece has always had: index 0 is air, and completion is then
    // the ONLY difference between this file and the one it replaces. Insertion
    // order is otherwise first use, which would renumber all four.
    for (name, props) in [
        ("minecraft:air", None),
        ("minecraft:stone", None),
        ("minecraft:iron_bars", None),
        ("minecraft:lantern", Some(&[("hanging", "true")][..])),
    ] {
        palette.idx(name, props);
    }
    let mut blocks = Vec::with_capacity((SIZE[0] * SIZE[1] * SIZE[2]) as usize);
    // Iteration is x→y→z ascending so the `blocks` list order is fully determined.
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                let (name, props) = block_at(x, y, z);
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: palette.idx(name, props),
                });
            }
        }
    }
    // Pinning the order above is only safe while every pinned entry is still
    // placed: an entry nothing references is a block the file claims to contain
    // and does not.
    for (i, e) in palette.entries.iter().enumerate() {
        assert!(
            blocks.iter().any(|b| b.state == i as i32),
            "{ID}: palette entry {i} ({}) is referenced by no cell",
            e.name
        );
    }
    Structure {
        data_version: DATA_VERSION,
        size: SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

/// The flattened view the shared [`invariants`] gates read: exactly the blocks
/// this piece is about to write, palette already resolved.
fn invariant_cells(s: &Structure) -> invariants::Cells {
    s.blocks
        .iter()
        .map(|b| {
            let p = &s.palette[b.state as usize];
            (
                b.pos,
                (p.name.clone(), p.properties.clone().unwrap_or_default()),
            )
        })
        .collect()
}

/// This piece's palette and block list, handed to the shared connection pass
/// and taken back. The rule lives in [`connections`]; only the conversion
/// between it and this workspace's own `Structure` types is local.
fn resolve_connections(id: &str, s: &mut Structure) {
    let mut piece = connections::Piece {
        palette: s
            .palette
            .iter()
            .map(|p| (p.name.clone(), p.properties.clone().unwrap_or_default()))
            .collect(),
        positions: s.blocks.iter().map(|b| b.pos).collect(),
        states: s.blocks.iter().map(|b| b.state as usize).collect(),
    };
    connections::resolve(id, &mut piece);
    s.palette = piece
        .palette
        .into_iter()
        .map(|(name, properties)| PaletteEntry {
            name,
            properties: (!properties.is_empty()).then_some(properties),
        })
        .collect();
    for (b, state) in s.blocks.iter_mut().zip(piece.states) {
        b.state = state as i32;
    }
}

fn write_piece(out: &Path) {
    let mut s = build();
    // Connections before the gates: what the door's bars join is derived from
    // the blocks beside them, never left to vanilla's defaults.
    resolve_connections(ID, &mut s);
    let cells = invariant_cells(&s);
    invariants::assert_distress_never_stacks(ID, &cells);
    // Spelling, at the emitter: an unknown block id loads as AIR.
    invariants::assert_blocks_are_real(ID, &cells);
    // Shape, at the emitter: an omitted connection property ships a post.
    connections::assert_shape_is_stated(ID, &cells);
    connections::assert_attachments_are_supported(ID, &cells);

    let nbt = fastnbt::to_bytes(&s).expect("structure serializes to NBT");
    // gzip-frame it (MC reads structure files as gzip-compressed NBT). Pin mtime
    // to 0 and use a fixed compression level for byte-identity.
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip write");
    let framed = gz.finish().expect("gzip finish");

    let path = out.join(format!("{ID}.nbt"));
    std::fs::write(&path, &framed).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!(
        "wrote {} ({} blocks, {} palette entries, {} gz bytes)",
        path.display(),
        s.blocks.len(),
        s.palette.len(),
        framed.len()
    );
}

fn main() {
    let Some(out) = std::env::args().nth(1) else {
        eprintln!("usage: hello-room-gen <out_dir>   (the prefab library, e.g. campaigns/prefabs)");
        std::process::exit(2);
    };
    let out = Path::new(&out);
    // Deliberately NOT `create_dir_all`: the destination is an existing library,
    // and a generator that makes its own destination cannot tell a fresh library
    // from a typo. Writing where nobody reads is indistinguishable from success.
    if !out.is_dir() {
        eprintln!(
            "hello-room-gen: {} is not an existing directory. Point this at the prefab library \
             you mean to re-export into (the content repo's `prefabs/`); this generator will not \
             create it, because a path it created is a path nothing reads.",
            out.display()
        );
        std::process::exit(2);
    }
    write_piece(out);
}
