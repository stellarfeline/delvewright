//! Deterministic generator for the Delvewright "nobodys-cave island" SET-PIECE
//! prefabs (spec-0013 remake): the beach camp (`island-beach-camp`, the sea-level
//! entry) and the ancient-Greek galley (`island-galley`, the showcase visible from
//! spawn).
//!
//! A SIBLING of `prefabs/cave-generator` and `prefabs/generator`: its own
//! `[workspace]`, outside `crates/`, so it never enters the shipped `delvec` binary
//! and no other tileset's `.nbt` output changes (ADR-0006). It emits the same
//! artifact shape — a gzip-framed vanilla structure `.nbt` (`mtime` pinned to 0) +
//! a metadata `.json` per piece — reusing the proven cave-generator NBT/socket/
//! lighting machinery, but the pieces are open-air island scenery, not enclosed
//! rock.
//!
//! ISLAND CONVENTION (documented so the terrain worker's greenfield/mountain
//! pieces align — see prefabs/island-tileset.md):
//!   * The world horizon is `ocean` (spec-0013): a superflat with **sea level
//!     y=62**; the compiler places ocean areas at y=60 (`sea_level-2`), the datum
//!     this convention assumes.
//!   * Every island piece authors its own local geometry with a **base at local
//!     y=0** and a **waterline at local y=2** (top water block). Placed with its
//!     base at world `sea_level-2` (y=60), the authored water meets the world
//!     ocean seamlessly.
//!   * The walkable land plane is **local y=3** — ONE block above the waterline.
//!     Water (the conservative compiler flood, `crates/compiler/src/assembled.rs`)
//!     spreads horizontally and downward but never CLIMBS, so a y=3 walk surface
//!     over solid-at-y=2 land can never flood: dry standable cells are dry by
//!     construction, not by distance. That is why the tutorial `surf-wave` anchor
//!     and the whole camp are flood-safe.
//!   * Sockets are keep-socket geometry under the `island:socket` vocabulary with
//!     **floor_y=2** (3x3 opening based at the y=3 walk plane, one jigsaw block at
//!     the bottom-centre). The solver reads socket geometry only, so island pieces
//!     mate with the same machinery as cave/keep pieces.
//!
//! The galley hull is authored once by `build_galley` and used TWO ways:
//!   1. `island-galley` — the standalone set-piece (zero connectors, one
//!      `anchor/deck`), the admitted `hero-galleon-oak` pattern. Output byte-
//!      identical to before this merge; it stays a reusable offshore-ship asset.
//!   2. `island-beach-camp` — the same hull is STAMPED into authored ocean water
//!      south of the sand, so the camp piece contains its own moored galley,
//!      visible offshore from spawn and boardable via a walkable gangplank
//!      (`anchor/deck`, `anchor/prow`).
//!
//! Why merged (not offset at assembly): the DSL has no scenery-offset primitive
//! and the solver spaces areas 256 blocks apart, so a *standalone* galley area can
//! never be "anchored just offshore" from the beach — the two would sit a quarter-
//! kilometre apart with no way to bridge them. The merged-piece fallback was the
//! reserved plan for exactly this (design brief §5, tileset-doc "merged vs
//! separate"). The stamp copies only the hull's SOLID cells over the beach's own
//! sea, so the shared waterline (local y=2) and seabed stay a single water body —
//! no cross-seam flood interaction, the concern that first motivated "separate".
//!
//! Usage: island-prefab-gen <out_dir>

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

/// Cross-tileset generator invariants, shared by source include so a lesson
/// learned in one tileset does not have to be re-learned in the other four
/// (the generators are separate Cargo workspaces on purpose).
#[path = "../../invariants.rs"]
mod invariants;

/// The connection derivation, shared the same way: what a fence, a wall, a pane
/// or a lichen joins is computed from the blocks beside it, at the emitter.
#[path = "../../connections.rs"]
mod connections;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

const DATA_VERSION: i32 = 4671; // MC 1.21.11
const GENERATOR: &str = "prefabs/island-generator (island-prefab-gen)";
const MEASURED_DATE: &str = "2026-08-01";
/// The island convention's waterline (`../island-tileset.md`): the top authored
/// water block is local y=2, the walkable land plane local y=3. Declared in every
/// piece's metadata as `waterline_y`, which the compiler pins to world sea level
/// when it places an ocean-horizon area (`DW0344`).
const WATERLINE_Y: i32 = 2;

// ---------------------------------------------------------------------------
// Deterministic hashing / value noise (ADR-0006) — same primitives as
// cave-generator (splitmix64 finalizer + trilinear value noise).
// ---------------------------------------------------------------------------

fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn piece_seed(id: &str, salt: u64) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    mix64(h ^ salt)
}

fn hash01(seed: u64, x: i32, y: i32, z: i32, salt: u64) -> f64 {
    let mut h = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = mix64(h ^ (x as i64 as u64).wrapping_mul(0x0000_0100_0000_01B3));
    h = mix64(h ^ (y as i64 as u64).wrapping_mul(0xFF51_AFD7_ED55_8CCD));
    h = mix64(h ^ (z as i64 as u64).wrapping_mul(0xC4CE_B9FE_1A85_EC53));
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn fade(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn value_noise(seed: u64, x: i32, y: i32, z: i32, freq: f64, salt: u64) -> f64 {
    let (fx, fy, fz) = (x as f64 * freq, y as f64 * freq, z as f64 * freq);
    let (x0, y0, z0) = (fx.floor(), fy.floor(), fz.floor());
    let (tx, ty, tz) = (fade(fx - x0), fade(fy - y0), fade(fz - z0));
    let (ix, iy, iz) = (x0 as i32, y0 as i32, z0 as i32);
    let c = |dx: i32, dy: i32, dz: i32| hash01(seed, ix + dx, iy + dy, iz + dz, salt);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), tx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), tx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), tx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), tx);
    let y0i = lerp(x00, x10, ty);
    let y1i = lerp(x01, x11, ty);
    lerp(y0i, y1i, tz)
}

// ---------------------------------------------------------------------------
// Cell grid + NBT serialization (structure format), mirrored from cave-generator.
// ---------------------------------------------------------------------------

type Props = Option<Vec<(&'static str, &'static str)>>;

#[derive(Clone)]
enum Cell {
    Air,
    Block(String, Props),
    Jigsaw(&'static str),
}

struct Grid {
    size: [i32; 3],
    cells: Vec<Cell>,
}
impl Grid {
    fn new(size: [i32; 3]) -> Self {
        let n = (size[0] * size[1] * size[2]) as usize;
        Grid {
            size,
            cells: vec![Cell::Air; n],
        }
    }
    fn idx(&self, x: i32, y: i32, z: i32) -> usize {
        ((x * self.size[1] + y) * self.size[2] + z) as usize
    }
    fn inb(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < self.size[0] && y < self.size[1] && z < self.size[2]
    }
    fn set(&mut self, x: i32, y: i32, z: i32, c: Cell) {
        if self.inb(x, y, z) {
            let i = self.idx(x, y, z);
            self.cells[i] = c;
        }
    }
    fn get(&self, x: i32, y: i32, z: i32) -> &Cell {
        &self.cells[self.idx(x, y, z)]
    }
    fn is_air(&self, x: i32, y: i32, z: i32) -> bool {
        !self.inb(x, y, z) || matches!(self.get(x, y, z), Cell::Air)
    }
    fn blk(&mut self, x: i32, y: i32, z: i32, name: &'static str, props: Props) {
        self.set(x, y, z, Cell::Block(name.to_string(), props));
    }
    /// Place only into an empty (air) cell — never overwrite structure already
    /// placed. Keeps ordering-independent scene layers from clobbering each other.
    fn fill_air(&mut self, x: i32, y: i32, z: i32, name: &'static str, props: Props) {
        if self.is_air(x, y, z) {
            self.blk(x, y, z, name, props);
        }
    }
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
struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<Entity>,
}
#[derive(Serialize)]
struct Entity {}

struct Palette {
    entries: Vec<PaletteEntry>,
}
impl Palette {
    fn new() -> Self {
        Palette { entries: vec![] }
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

fn props_map(props: &Props) -> Option<BTreeMap<String, String>> {
    props.as_ref().map(|v| {
        v.iter()
            .map(|(k, val)| (k.to_string(), val.to_string()))
            .collect()
    })
}

fn serialize(grid: &Grid) -> Structure {
    let [sx, sy, sz] = grid.size;
    let mut pal = Palette::new();
    pal.idx("minecraft:air", None); // reserve index 0
    let mut blocks = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let cell = grid.get(x, y, z);
                let (state, nbt) = match cell {
                    Cell::Air => (pal.idx("minecraft:air", None), None),
                    Cell::Block(name, props) => (pal.idx(name, props_map(props)), None),
                    Cell::Jigsaw(o) => {
                        let mut p = BTreeMap::new();
                        p.insert("orientation".to_string(), o.to_string());
                        (
                            pal.idx("minecraft:jigsaw", Some(p)),
                            Some(JigsawBE {
                                id: "minecraft:jigsaw".into(),
                                name: "island:socket".into(),
                                target: "island:socket".into(),
                                pool: "island:pool".into(),
                                final_state: "minecraft:air".into(),
                                joint: "aligned".into(),
                            }),
                        )
                    }
                };
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state,
                    nbt,
                });
            }
        }
    }
    Structure {
        data_version: DATA_VERSION,
        size: grid.size,
        palette: pal.entries,
        blocks,
        entities: vec![],
    }
}

// ---------------------------------------------------------------------------
// Sockets (keep-socket geometry, island vocabulary). floor_y=2 (walk plane y=3).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)] // full 4-side socket vocabulary; current pieces use North only
enum Side {
    North,
    South,
    West,
    East,
}
impl Side {
    fn orientation(self) -> &'static str {
        match self {
            Side::North => "north_up",
            Side::South => "south_up",
            Side::West => "west_up",
            Side::East => "east_up",
        }
    }
    fn facing(self) -> &'static str {
        match self {
            Side::North => "north",
            Side::South => "south",
            Side::West => "west",
            Side::East => "east",
        }
    }
}

fn door_center(size: [i32; 3], side: Side, floor_y: i32) -> [i32; 3] {
    let (x, z) = (size[0], size[2]);
    let y = floor_y + 1;
    match side {
        Side::North => [x / 2, y, 0],
        Side::South => [x / 2, y, z - 1],
        Side::West => [0, y, z / 2],
        Side::East => [x - 1, y, z / 2],
    }
}
fn doorway_cells(size: [i32; 3], side: Side, floor_y: i32) -> (Vec<[i32; 3]>, [i32; 3]) {
    let jc = door_center(size, side, floor_y);
    let base = floor_y + 1;
    let mut cells = vec![];
    for dy in 0..3 {
        for d in -1..=1 {
            let p = match side {
                Side::North | Side::South => [jc[0] + d, base + dy, jc[2]],
                Side::West | Side::East => [jc[0], base + dy, jc[2] + d],
            };
            cells.push(p);
        }
    }
    (cells, jc)
}

// ---------------------------------------------------------------------------
// Metadata JSON (identical shape to cave-generator).
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct AnchorJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pos: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    facing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<RegionJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block: Option<String>,
}
#[derive(Serialize)]
struct RegionJson {
    from: [i32; 3],
    to: [i32; 3],
}
#[derive(Serialize)]
struct ConnectorJson {
    name: &'static str,
    target: &'static str,
    local_pos: [i32; 3],
    facing: String,
    opening: [i32; 2],
    joint: &'static str,
}
#[derive(Serialize)]
struct StructureJson {
    file: String,
    id: String,
    size: [i32; 3],
    data_version: i32,
    generator: String,
}
#[derive(Serialize)]
struct LightingJson {
    profile: &'static str,
    measured_min_light: i32,
    measured: &'static str,
    method: &'static str,
}
#[derive(Serialize)]
struct LicenseJson {
    source: &'static str,
    spdx: &'static str,
    note: &'static str,
    provenance: &'static str,
}
#[derive(Serialize)]
struct MetaJson {
    prefab_id: String,
    structure: StructureJson,
    /// Local y of the top authored water block (the island convention's waterline).
    /// The compiler's ocean-horizon placement invariant (`DW0344`) requires this to
    /// land at world sea level (y=62) — see `prefabs/island-tileset.md`.
    waterline_y: i32,
    anchors: BTreeMap<String, AnchorJson>,
    connectors: Vec<ConnectorJson>,
    lighting: LightingJson,
    license: LicenseJson,
}

fn a_pos(pos: [i32; 3], facing: Option<&str>) -> AnchorJson {
    AnchorJson {
        pos: Some(pos),
        facing: facing.map(|s| s.to_string()),
        region: None,
        block: None,
    }
}

// ---------------------------------------------------------------------------
// Gravity-floor invariant: no falling block over air. Both island
// pieces author a solid base under every sand cell, so this is belt-and-braces
// (the compiler's DW0313 is the authoritative gate).
// ---------------------------------------------------------------------------

fn is_falling(name: &str) -> bool {
    let id = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        id,
        "sand" | "red_sand" | "gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg"
    ) || id.ends_with("_concrete_powder")
}

fn assert_no_unsupported_gravity(id: &str, g: &Grid) {
    let [sx, sy, sz] = g.size;
    let mut bad = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if matches!(g.get(x, y, z), Cell::Block(name, _) if is_falling(name))
                    && !(y > 0 && matches!(g.get(x, y - 1, z), Cell::Block(_, _)))
                {
                    bad.push([x, y, z]);
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "island-generator invariant: piece `{id}` has {} unsupported gravity block(s) over air \
         (would despawn in the_void — add a substrate): {:?}",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
}

/// Stamp `src`'s SOLID cells into `dst` at offset `(ox,oy,oz)`, skipping air and
/// water. Used to merge the galley hull into the beach camp's authored ocean:
/// skipping water leaves the beach's own seabed + waterline intact (one water
/// body, no second flood volume), while the hull, deck, mast, oars and dressing
/// are copied in. Deterministic — a straight cell copy, no RNG.
fn stamp_solids(dst: &mut Grid, src: &Grid, ox: i32, oy: i32, oz: i32) {
    let [sx, sy, sz] = src.size;
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let cell = src.get(x, y, z);
                let skip = matches!(cell, Cell::Air)
                    || matches!(cell, Cell::Block(name, _) if name == "minecraft:water");
                if !skip {
                    dst.set(x + ox, y + oy, z + oz, cell.clone());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PIECE 1 — island-beach-camp (open-air sea-level entry).
//   Base stone y=0; sand beach y=1..2 (walk y=3); sea to the south (high Z) with
//   waterline top at y=2; one inland (north) island:socket to greenfield. The
//   moored galley is stamped into that southern sea and a walkable gangplank
//   bridges the beach to its deck (see the jetty/galley section near the end).
// ---------------------------------------------------------------------------

/// Ragged tide line: the first (south-most) sea row for column `x`. Dry beach is
/// z < coast; sea is z >= coast. Jittered per-column so the coast is uneven.
fn beach_coast(seed: u64, x: i32) -> i32 {
    let j = (value_noise(seed, x, 0, 0, 0.30, 131) * 3.0).floor() as i32 - 1; // -1..+1
    (12 + j).clamp(11, 14)
}

fn build_beach_camp(size: [i32; 3], seed: u64) -> Grid {
    let [sx, _sy, sz] = size;
    let mut g = Grid::new(size);
    let surf = WATERLINE_Y; // top water block (waterline)

    // Solid base under everything (the sand/gravel substrate + the seabed).
    for x in 0..sx {
        for z in 0..sz {
            g.blk(x, 0, z, "minecraft:stone", None);
        }
    }
    // Surface layer: dry beach (sand y=1,2 -> top y=3) vs sea.
    for x in 0..sx {
        let coast = beach_coast(seed, x);
        for z in 0..sz {
            if z < coast {
                // Dry beach: sand with clustered gravel patches (noise). Two sand
                // blocks (y=1,2) so the walk plane sits at y=3, ONE above the
                // waterline — dry by construction (water never climbs).
                let n = value_noise(seed, x, 1, z, 0.22, 101);
                let dry_top = if n > 0.80 {
                    "minecraft:gravel"
                } else {
                    "minecraft:sand"
                };
                g.blk(x, 1, z, "minecraft:sand", None);
                g.blk(x, 2, z, dry_top, None);
            } else if z == coast {
                // Shallow: sand seabed (y=1), 1 block of water (y=2) — a clean
                // 1-block tide step down from the dry beach.
                g.blk(x, 1, z, "minecraft:sand", None);
                g.blk(x, surf, z, "minecraft:water", Some(vec![("level", "0")]));
                if value_noise(seed, x, 2, z, 0.5, 105) > 0.80 {
                    g.blk(x, 2, z, "minecraft:seagrass", None); // over sand, at surface
                }
            } else {
                // Open sea: stone seabed (y=0 already), 2 blocks of water.
                for wy in 1..=surf {
                    g.blk(x, wy, z, "minecraft:water", Some(vec![("level", "0")]));
                }
            }
        }
    }

    // Loose rock scatter + driftwood on the dry beach (noise-clustered greeble).
    for x in 2..sx - 2 {
        let coast = beach_coast(seed, x);
        for z in 1..coast - 1 {
            let n = value_noise(seed, x, 3, z, 0.5, 111);
            if n > 0.93 && g.is_air(x, 3, z) {
                g.blk(x, 3, z, "minecraft:cobblestone", None);
            }
        }
    }
    for (lx, lz, ax) in [(3, 8, "x"), (18, 6, "z")] {
        if lz < beach_coast(seed, lx) {
            g.fill_air(
                lx,
                3,
                lz,
                "minecraft:stripped_oak_log",
                Some(vec![("axis", ax)]),
            );
        }
    }
    for (bx, bz) in [(2, 4), (19, 9)] {
        if bz < beach_coast(seed, bx) {
            g.fill_air(bx, 3, bz, "minecraft:dead_bush", None);
        }
    }

    // --- Camp: campfire ring with log benches (the relight fixture). ---
    let (cx, cz) = (10, 5);
    g.blk(
        cx,
        3,
        cz,
        "minecraft:campfire",
        Some(vec![("lit", "true"), ("facing", "north")]),
    );
    // Log benches around the fire (one cell out on the four sides).
    for bx in cx - 1..=cx + 1 {
        g.fill_air(
            bx,
            3,
            cz - 2,
            "minecraft:stripped_oak_log",
            Some(vec![("axis", "x")]),
        );
        g.fill_air(
            bx,
            3,
            cz + 2,
            "minecraft:stripped_oak_log",
            Some(vec![("axis", "x")]),
        );
    }
    for bz in cz - 1..=cz + 1 {
        g.fill_air(
            cx - 2,
            3,
            bz,
            "minecraft:stripped_oak_log",
            Some(vec![("axis", "z")]),
        );
        g.fill_air(
            cx + 2,
            3,
            bz,
            "minecraft:stripped_oak_log",
            Some(vec![("axis", "z")]),
        );
    }
    // A stack of firewood beside the hearth.
    g.fill_air(
        cx + 3,
        3,
        cz + 2,
        "minecraft:oak_log",
        Some(vec![("axis", "y")]),
    );

    // --- Two tents (fence frame + wool A-frame roof), open toward the fire. ---
    build_tent(&mut g, 3, 3, "minecraft:white_wool");
    build_tent(&mut g, 15, 3, "minecraft:light_gray_wool");

    // --- Supply barrels (crates). ---
    for (bx, by, bz) in [(17, 3, 7), (18, 3, 7), (17, 4, 7), (17, 3, 8)] {
        g.blk(bx, by, bz, "minecraft:barrel", Some(vec![("facing", "up")]));
    }

    // --- Class-selection post: fence + lantern beacon near the shore. ---
    g.blk(6, 3, 9, "minecraft:oak_fence", None);
    g.blk(6, 4, 9, "minecraft:oak_fence", None);
    g.blk(
        6,
        5,
        9,
        "minecraft:lantern",
        Some(vec![("hanging", "false")]),
    );

    // --- Jetty: a short pier reaching south over the shallows (walk y=3). ---
    // Bounded to z=11..=16 (NOT run to the south wall): the moored galley and its
    // boarding gangplank occupy the water beyond it. anchor/gangplank is [10,3,15].
    let jetty_x = 10;
    for z in 11..=16 {
        g.blk(jetty_x, 1, z, "minecraft:oak_fence", None); // pile support
        g.blk(jetty_x, 2, z, "minecraft:oak_planks", None); // deck plank (walk y=3)
    }

    // --- Moored Greek galley, stamped into the authored ocean south of the sand.
    // Native orientation (long axis +Z, pointed stern toward the beach); the SAME
    // hull `build_galley` authors for the standalone `island-galley`. Only solid
    // cells are copied (stamp_solids) so the beach's sea stays one water body, and
    // the hull sits at merged x=7..15 / z=14..42, fully afloat (all z >= coast).
    const GALLEY_OX: i32 = 7; // hull centreline (local cx=4) lands on merged x=11
    const GALLEY_OZ: i32 = 14; // stern clears the coast (max 14); hull is offshore
    let galley = build_galley([9, 15, 29], 0);
    stamp_solids(&mut g, &galley, GALLEY_OX, 0, GALLEY_OZ);

    // --- Boarding gangplank: jetty head (y=3) up onto the galley deck (y=5).
    // The route hugs x=10 past the pointed stern (whose centreline stem posts at
    // merged (11,·,17) and (11,·,18) form the support column), then steps east onto
    // the deck spine at x=11. Every stand cell rises <=1 with >=2 air overhead, so
    // it is DW0311-walkable from the jetty to the deck:
    //   (10,3,16) -> (10,4,17) -> (10,5,18) -> (11,5,18) -> (11,5,19) deck spine
    // spruce treads on oak-fence piles read as the ship's own gangplank.
    // z=17 tread (stand y=4), on a pile down to the seabed.
    g.blk(10, 1, 17, "minecraft:oak_fence", None);
    g.blk(10, 2, 17, "minecraft:spruce_planks", None);
    g.blk(10, 3, 17, "minecraft:spruce_planks", None);
    // z=18 tread (stand y=5), on a pile down to the seabed.
    g.blk(10, 1, 18, "minecraft:oak_fence", None);
    g.blk(10, 2, 18, "minecraft:spruce_planks", None);
    g.blk(10, 3, 18, "minecraft:spruce_planks", None);
    g.blk(10, 4, 18, "minecraft:spruce_planks", None);
    // Step east onto the deck: this tread rests on the stern stem post (11,3,18).
    g.blk(11, 4, 18, "minecraft:spruce_planks", None);

    // --- Deck lanterns (merged piece only — the galley appears in a dusk beat).
    // Set on the bulwark caps fore & aft, clear of the deck walk lane (x=10..12).
    g.blk(
        13,
        6,
        23,
        "minecraft:lantern",
        Some(vec![("hanging", "false")]),
    );
    g.blk(
        8,
        6,
        31,
        "minecraft:lantern",
        Some(vec![("hanging", "false")]),
    );

    g
}

/// A small gable tent: fence corner posts + a wool A-frame roof, front open toward
/// +z (the fire). Origin `(x0,z0)` is the front-left corner; footprint 3x3.
fn build_tent(g: &mut Grid, x0: i32, z0: i32, wool: &'static str) {
    let (x1, z1, zc) = (x0 + 2, z0 + 2, z0 + 1);
    // A-frame tent: a ridge along X at the peak (y=5), the roof sloping down to the
    // sand at z0/z1, the back gable (x1) closed and the front gable (x0) open.
    for xx in x0..=x1 {
        g.fill_air(xx, 5, zc, wool, None); // ridge
        g.fill_air(xx, 4, z0, wool, None); // slope faces
        g.fill_air(xx, 4, z1, wool, None);
        g.fill_air(xx, 3, z0, wool, None); // walls to the ground
        g.fill_air(xx, 3, z1, wool, None);
    }
    // Back gable fill (x1) + two tent-poles flanking the open front (x0).
    g.fill_air(x1, 4, zc, wool, None);
    g.fill_air(x1, 3, zc, wool, None);
    g.fill_air(x0, 3, z0, "minecraft:oak_fence", None);
    g.fill_air(x0, 3, z1, "minecraft:oak_fence", None);
}

// ---------------------------------------------------------------------------
// PIECE 2 — island-galley (ancient-Greek galley, standalone set-piece).
//   Curved plank hull, single mast + white wool sail, oar rows (spruce
//   trapdoors/buttons), Greek eye (ophthalmos) on the prow. Floats on its own
//   authored water (waterline y=2), prow at high Z, stern at low Z.
// ---------------------------------------------------------------------------

/// Half-beam (hull half-width at the gunwale) for length position `z`: a sine
/// bulge, max amidships, tapering to a point (0) at bow and stern.
fn half_beam(z: i32) -> i32 {
    let (lo, hi) = (3.0_f64, 25.0_f64);
    let t = ((z as f64 - lo) / (hi - lo)).clamp(0.0, 1.0);
    (3.2 * (std::f64::consts::PI * t).sin()).round() as i32
}

fn build_galley(size: [i32; 3], _seed: u64) -> Grid {
    let [sx, _sy, sz] = size;
    let mut g = Grid::new(size);
    let cx = sx / 2; // beam centre (x=4)

    // Sea pad: 3 blocks of water (y=0..2) across the footprint; the hull displaces
    // it. Renders as "on the water" in isolation and blends with the world ocean
    // when placed (base at world sea_level-2).
    for x in 0..sx {
        for z in 0..sz {
            for y in 0..=2 {
                g.blk(x, y, z, "minecraft:water", Some(vec![("level", "0")]));
            }
        }
    }

    // --- Hull: keel + flared plank skin + deck sole + bulwark cap. ---
    for z in 3..=25 {
        let hb = half_beam(z);
        // Keel timber along the centreline (below the waterline).
        g.blk(
            cx,
            1,
            z,
            "minecraft:stripped_spruce_log",
            Some(vec![("axis", "z")]),
        );
        if hb <= 0 {
            // Pointed bow/stern: just a stem post rising from the keel.
            g.blk(cx, 2, z, "minecraft:spruce_planks", None);
            g.blk(cx, 3, z, "minecraft:spruce_planks", None);
            continue;
        }
        // Garboard (bottom planking) at y=1, tucked narrower than the beam.
        let bw = (hb - 1).max(0);
        for x in cx - bw..=cx + bw {
            g.blk(x, 1, z, "minecraft:spruce_planks", None);
        }
        // Side skin y=2..5. y=2 tucks in (turn of the bilge); y=3 is a dark wale
        // (waterline strake); y=4..5 topsides + bulwark cap.
        for y in 2..=5 {
            let hw = if y <= 2 { (hb - 1).max(0) } else { hb };
            let strake = if y == 3 {
                "minecraft:dark_oak_planks"
            } else {
                "minecraft:spruce_planks"
            };
            g.blk(cx - hw, y, z, strake, None);
            g.blk(cx + hw, y, z, strake, None);
        }
        // Deck sole at y=4 (walk plane y=5), oak for a lighter deck.
        for x in cx - hb + 1..=cx + hb - 1 {
            g.fill_air(x, 4, z, "minecraft:oak_planks", None);
        }
        // Clear the cockpit (rower space) at walk height so the deck reads open.
        for x in cx - hb + 1..=cx + hb - 1 {
            if g.inb(x, 6, z) {
                g.set(x, 6, z, Cell::Air);
            }
        }
    }

    // --- Oar rows: spruce trapdoors jutting from both sides, buttons as ports. ---
    for z in (6..=22).step_by(2) {
        let hb = half_beam(z);
        if hb < 2 {
            continue;
        }
        // Port (-x) and starboard (+x): trapdoor in the water cell just outside the
        // hull, hinged to the hull, open + bottom -> angled down like an oar.
        g.fill_air(
            cx - hb - 1,
            3,
            z,
            "minecraft:spruce_trapdoor",
            Some(vec![
                ("facing", "east"),
                ("half", "bottom"),
                ("open", "true"),
            ]),
        );
        g.fill_air(
            cx + hb + 1,
            3,
            z,
            "minecraft:spruce_trapdoor",
            Some(vec![
                ("facing", "west"),
                ("half", "bottom"),
                ("open", "true"),
            ]),
        );
        // Oar-port studs on the topside skin.
        g.blk(
            cx - hb,
            4,
            z,
            "minecraft:spruce_button",
            Some(vec![("face", "wall"), ("facing", "west")]),
        );
        g.blk(
            cx + hb,
            4,
            z,
            "minecraft:spruce_button",
            Some(vec![("face", "wall"), ("facing", "east")]),
        );
    }

    // --- Mast + square sail + rigging (amidships, z=14). ---
    let mz = 14;
    for y in 5..=12 {
        g.blk(cx, y, mz, "minecraft:spruce_log", Some(vec![("axis", "y")]));
    }
    // Yard across the beam at y=11 (just below the masthead).
    for x in cx - 3..=cx + 3 {
        g.fill_air(x, 11, mz, "minecraft:spruce_log", Some(vec![("axis", "x")]));
    }
    // White wool square sail: a SOLID 5x5 sheet on one plane, set one cell forward
    // of the mast so the mast reads behind it (not bisecting it), hung from the yard.
    for x in cx - 2..=cx + 2 {
        for y in 6..=10 {
            g.fill_air(x, y, mz + 1, "minecraft:white_wool", None);
        }
    }
    // Braces: furled-line stubs (spruce trapdoors) at the yard tips.
    g.fill_air(
        cx - 3,
        10,
        mz,
        "minecraft:spruce_trapdoor",
        Some(vec![
            ("facing", "south"),
            ("half", "top"),
            ("open", "false"),
        ]),
    );
    g.fill_air(
        cx + 3,
        10,
        mz,
        "minecraft:spruce_trapdoor",
        Some(vec![
            ("facing", "south"),
            ("half", "top"),
            ("open", "false"),
        ]),
    );
    // Masthead pennant lantern.
    g.fill_air(
        cx,
        13,
        mz,
        "minecraft:lantern",
        Some(vec![("hanging", "false")]),
    );

    // --- Greek eye (ophthalmos) on both bows: white sclera + black pupil. ---
    for &side in &[-1i32, 1] {
        for zz in 21..=23 {
            let hb = half_beam(zz);
            if hb < 2 {
                continue;
            }
            let sxx = cx + side * hb;
            for y in 3..=5 {
                g.blk(sxx, y, zz, "minecraft:white_wool", None);
            }
        }
        let hb22 = half_beam(22);
        g.blk(cx + side * hb22, 4, 22, "minecraft:black_wool", None);
    }

    // --- Prow: rising stempost + a low ram (embolos) at the waterline. ---
    g.fill_air(cx, 5, 25, "minecraft:spruce_planks", None);
    g.fill_air(cx, 6, 25, "minecraft:spruce_planks", None);
    g.fill_air(
        cx,
        6,
        26,
        "minecraft:spruce_stairs",
        Some(vec![
            ("facing", "north"),
            ("half", "bottom"),
            ("shape", "straight"),
        ]),
    );
    g.blk(
        cx,
        2,
        26,
        "minecraft:stripped_spruce_log",
        Some(vec![("axis", "z")]),
    );
    g.blk(
        cx,
        2,
        27,
        "minecraft:stripped_spruce_log",
        Some(vec![("axis", "z")]),
    );

    // --- Stern: curled aphlaston ornament. ---
    g.fill_air(cx, 5, 3, "minecraft:spruce_planks", None);
    g.fill_air(cx, 6, 2, "minecraft:spruce_planks", None);
    g.fill_air(
        cx,
        7,
        2,
        "minecraft:spruce_stairs",
        Some(vec![
            ("facing", "south"),
            ("half", "top"),
            ("shape", "straight"),
        ]),
    );
    // Steering oars at the stern quarters.
    g.fill_air(
        cx - 2,
        3,
        4,
        "minecraft:spruce_trapdoor",
        Some(vec![
            ("facing", "east"),
            ("half", "bottom"),
            ("open", "true"),
        ]),
    );
    g.fill_air(
        cx + 2,
        3,
        4,
        "minecraft:spruce_trapdoor",
        Some(vec![
            ("facing", "west"),
            ("half", "bottom"),
            ("open", "true"),
        ]),
    );

    // --- Deck dressing: amphorae (decorated pots) + a lashed barrel. ---
    g.fill_air(cx - 1, 5, 10, "minecraft:decorated_pot", None);
    g.fill_air(
        cx + 1,
        5,
        18,
        "minecraft:barrel",
        Some(vec![("facing", "up")]),
    );

    g
}

// ---------------------------------------------------------------------------
// Piece specs, emit.
// ---------------------------------------------------------------------------

enum Kind {
    BeachCamp,
    Galley,
}

struct Spec {
    id: &'static str,
    size: [i32; 3],
    kind: Kind,
    /// (side, floor_y) sockets. Empty for standalone set-pieces.
    doors: Vec<(Side, i32)>,
    anchors: Vec<(&'static str, AnchorJson)>,
    salt: u64,
}

fn build(spec: &Spec) -> Grid {
    let seed = piece_seed(spec.id, spec.salt);
    let mut g = match spec.kind {
        Kind::BeachCamp => build_beach_camp(spec.size, seed),
        Kind::Galley => build_galley(spec.size, seed),
    };
    // Carve sockets last so their opening columns are clear air + jigsaw seat.
    for &(side, fy) in &spec.doors {
        let (cells, jc) = doorway_cells(spec.size, side, fy);
        for c in &cells {
            g.set(c[0], c[1], c[2], Cell::Air);
        }
        g.set(jc[0], jc[1], jc[2], Cell::Jigsaw(side.orientation()));
    }
    g
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

fn write_piece(out: &Path, spec: &Spec) {
    let grid = build(spec);
    assert_no_unsupported_gravity(spec.id, &grid);

    let mut structure = serialize(&grid);
    // Connections before the gates: what a fence, a wall, a pane or a lichen
    // joins is derived from the blocks beside it, never left to the defaults.
    resolve_connections(spec.id, &mut structure);
    let cells = invariant_cells(&structure);
    invariants::assert_distress_never_stacks(spec.id, &cells);
    // Spelling, at the emitter: an unknown block id loads as AIR.
    invariants::assert_blocks_are_real(spec.id, &cells);
    // Shape, at the emitter: an omitted connection property ships a post.
    connections::assert_shape_is_stated(spec.id, &cells);
    connections::assert_attachments_are_supported(spec.id, &cells);
    // Settling, at the emitter: a body of fluid written with a way out of it
    // is not where it was put — the world moves it on the first tick, before
    // anyone arrives, and no other gate here looks (`DW0800`).
    invariants::assert_fluid_is_contained(spec.id, structure.size, &cells);
    let nbt = fastnbt::to_bytes(&structure).expect("nbt");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gz");
    let framed = gz.finish().expect("finish");
    std::fs::write(out.join(format!("{}.nbt", spec.id)), &framed).expect("write nbt");

    let mut connectors: Vec<ConnectorJson> = Vec::new();
    for &(side, fy) in &spec.doors {
        let (_, jc) = doorway_cells(spec.size, side, fy);
        connectors.push(ConnectorJson {
            name: "island:socket",
            target: "island:socket",
            local_pos: jc,
            facing: side.facing().into(),
            opening: [3, 3],
            joint: "aligned",
        });
    }

    let anchors: BTreeMap<String, AnchorJson> = spec
        .anchors
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                AnchorJson {
                    pos: v.pos,
                    facing: v.facing.clone(),
                    region: v.region.as_ref().map(|r| RegionJson {
                        from: r.from,
                        to: r.to,
                    }),
                    block: v.block.clone(),
                },
            )
        })
        .collect();

    let meta = MetaJson {
        prefab_id: format!("prefab/{}", spec.id),
        structure: StructureJson {
            file: format!("{}.nbt", spec.id),
            id: spec.id.into(),
            size: spec.size,
            data_version: DATA_VERSION,
            generator: GENERATOR.into(),
        },
        waterline_y: WATERLINE_Y,
        anchors,
        connectors,
        lighting: LightingJson {
            profile: "lit",
            measured_min_light: 15,
            measured: MEASURED_DATE,
            method: "open-air island set-piece: sky-lit, daylight 15 on the exposed \
                     land/deck (static block-light BFS not applicable to a roofless \
                     structure); campfire/lanterns supplement at night.",
        },
        license: LicenseJson {
            source: "original",
            spdx: "GPL-3.0-or-later",
            note: "Original Delvewright project asset (pipeline-code license per \
                   prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            provenance: "Generated deterministically by prefabs/island-generator \
                         (island-prefab-gen), ADR-0006; regenerating yields \
                         byte-identical NBT.",
        },
    };
    let json = serde_json::to_string_pretty(&meta).expect("json") + "\n";
    std::fs::write(out.join(format!("{}.json", spec.id)), json).expect("write json");
    println!("wrote {} ({} nbt bytes)", spec.id, framed.len());
}

fn specs() -> Vec<Spec> {
    use Side::*;
    vec![
        Spec {
            // Beach camp + its moored galley (stamped in the southern sea) + the
            // boarding gangplank. Extended +Z (open-sea side) and +Y (mast height);
            // every pre-galley beach anchor keeps its original local coordinate.
            id: "island-beach-camp",
            size: [21, 15, 44],
            kind: Kind::BeachCamp,
            doors: vec![(North, 2)], // inland island:socket to greenfield (walk y=3)
            anchors: vec![
                ("entry", a_pos([10, 3, 9], Some("south"))),
                ("anchor/camp-fire", a_pos([10, 3, 5], Some("north"))),
                ("anchor/class-post", a_pos([7, 3, 9], Some("north"))),
                ("anchor/crew-a", a_pos([7, 3, 5], Some("east"))),
                ("anchor/crew-b", a_pos([13, 3, 5], Some("west"))),
                ("anchor/surf-wave", a_pos([8, 3, 10], Some("south"))),
                ("anchor/gangplank", a_pos([10, 3, 15], Some("south"))),
                // Galley deck (boarding target) + bow (scenic ending beat), both on
                // the merged hull's walk plane y=5, facing to read from the camp.
                ("anchor/deck", a_pos([11, 5, 22], Some("north"))),
                ("anchor/prow", a_pos([11, 5, 35], Some("south"))),
            ],
            salt: 1,
        },
        Spec {
            id: "island-galley",
            size: [9, 15, 29],
            kind: Kind::Galley,
            doors: vec![],
            anchors: vec![("anchor/deck", a_pos([4, 5, 11], Some("north")))],
            salt: 2,
        },
    ]
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: island-prefab-gen <out_dir>");
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("mkdir");
    for spec in &specs() {
        write_piece(out, spec);
    }
    println!("{} pieces", specs().len());
}
