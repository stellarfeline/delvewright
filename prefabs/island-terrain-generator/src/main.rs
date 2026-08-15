//! Deterministic generator for the Delvewright "nobody's-cave island" TERRAIN
//! prefabs — the mountain terminal (shell + tall-wide cavern interior, switchback
//! slope on the face) and the greenfield connectors (grass meadow + empty sheep
//! fold). A SIBLING of `prefabs/cave-generator` and `prefabs/generator`: its own
//! `[workspace]`, outside `crates/`, so it never enters the shipped `delvec`
//! binary and the keep/cave `.nbt` output stays byte-identical (ADR-0006).
//!
//! It shares the cave generator's proven primitives (splitmix64 PRNG, trilinear
//! value-noise palette field, vanilla-structure `.nbt` emit, keep-socket geometry,
//! the gravity-substrate invariant, a derived static block-light estimate) but the
//! two builders are bespoke terrain, not the cave box-room grammar:
//!   * `build_greenfield` — an open-air, sky-lit bounded meadow (箱庭) connector:
//!     grass floor on solid earth, scattered oaks, poppy/daisy/cornflower flowers,
//!     a worn dirt path spine, and a low mossy-cobblestone empty sheep fold.
//!   * `build_mountain` — a solid rock massif, CARVED (fill-then-subtract): a
//!     terraced switchback path climbs the south face to a cave-mouth ledge (a
//!     boulder gate region beside it), opening into ONE tall-wide cavern hall
//!     (≥30×14×24 interior) with a cheese store by the entry, a central fire pit,
//!     a rock-shelf ramp (no ladders) up to an empty upper sheep pen, four dark
//!     shadow alcoves, dripstone + moss dressing, and two ceiling light shafts
//!     open to the sky.
//!
//! Connection convention reuses keep-socket geometry (3×3 opening, one jigsaw
//! block at the bottom-centre wall cell) under the `island:socket` / `island:pool`
//! vocabulary — the compiler solver reads socket geometry only (names are a
//! connectivity vocabulary), so the island pieces mate with the same machinery as
//! keep/cave pieces and share a pool with the beach-camp / galley set pieces.
//!
//! Determinism (ADR-0006): every stream is seeded from a per-piece PRNG + value
//! noise (no wall clock — gzip mtime pinned 0 —, no unseeded RNG, no hash-order
//! iteration, no absolute paths in output). Same seed → byte-identical `.nbt`.
//!
//! Usage: island-terrain-gen <out_dir>

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
const GENERATOR: &str = "prefabs/island-terrain-generator (island-terrain-gen)";
const MEASURED_DATE: &str = "2026-08-01";
/// The island convention's waterline datum (`../island-tileset.md`): sea surface at
/// local y=2, walk plane at local y=3. Inland terrain pieces author no water, but
/// they are lifted onto the same datum (`lift_substrate`) so their walk plane mates
/// with the beach camp's — declaring it makes the compiler's ocean-horizon
/// placement invariant (`DW0344`) cover the whole tileset, not just the shore.
const WATERLINE_Y: i32 = 2;
const SOCKET_NAME: &str = "island:socket";
const SOCKET_POOL: &str = "island:pool";

// ---------------------------------------------------------------------------
// Deterministic hashing / value noise (shared primitive family with cave-gen)
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

/// Trilinearly-interpolated value noise in [0,1] — smooth so palette picks cluster
/// into strata/moss patches instead of per-cell speckle.
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
// Palette recipes
// ---------------------------------------------------------------------------

type Props = Option<Vec<(&'static str, &'static str)>>;

/// A neighbouring cell as `(block id, properties)`, or `None` when it is
/// outside the grid or holds no block. The shape [`connections`] asks about.
fn neighbour_state(g: &Grid, x: i32, y: i32, z: i32) -> Option<(String, BTreeMap<String, String>)> {
    if !g.inb(x, y, z) {
        return None;
    }
    match g.get(x, y, z) {
        Cell::Block(name, props) => Some((
            name.clone(),
            props
                .iter()
                .flatten()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )),
        _ => None,
    }
}

struct Recipe {
    name: &'static str,
    weight: f64,
}
fn r(name: &'static str, weight: f64) -> Recipe {
    Recipe { name, weight }
}

/// Cave/mountain wall rock: cobble-dominant with andesite/tuff/stone bands + moss.
fn wall_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:cobblestone", 0.32),
        r("minecraft:andesite", 0.20),
        r("minecraft:tuff", 0.16),
        r("minecraft:stone", 0.14),
        r("minecraft:mossy_cobblestone", 0.11),
        r("minecraft:cracked_stone_bricks", 0.07),
    ]
}
/// Cavern ceiling: darker/greyer with a little dripstone.
fn ceiling_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:stone", 0.28),
        r("minecraft:andesite", 0.26),
        r("minecraft:tuff", 0.22),
        r("minecraft:cobblestone", 0.16),
        r("minecraft:dripstone_block", 0.08),
    ]
}
/// Cave floor: gravel/cobble/stone with sand + coarse-dirt + moss patches.
fn floor_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:gravel", 0.26),
        r("minecraft:cobblestone", 0.22),
        r("minecraft:stone", 0.18),
        r("minecraft:andesite", 0.12),
        r("minecraft:coarse_dirt", 0.10),
        r("minecraft:moss_block", 0.08),
        r("minecraft:sand", 0.04),
    ]
}
/// Boulder / seal mass: heavy mossy cobble + basalt (the sealing rock).
fn boulder_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:cobblestone", 0.34),
        r("minecraft:mossy_cobblestone", 0.28),
        r("minecraft:andesite", 0.16),
        r("minecraft:basalt", 0.12),
        r("minecraft:tuff", 0.10),
    ]
}
/// Bare upper mountain rock (exterior massif surface, higher = greyer stone).
fn massif_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:stone", 0.34),
        r("minecraft:andesite", 0.24),
        r("minecraft:cobblestone", 0.20),
        r("minecraft:tuff", 0.14),
        r("minecraft:mossy_cobblestone", 0.08),
    ]
}
/// Meadow ground: grass over a little coarse-dirt / podzol dappling.
fn meadow_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:grass_block", 0.78),
        r("minecraft:coarse_dirt", 0.10),
        r("minecraft:podzol", 0.07),
        r("minecraft:moss_block", 0.05),
    ]
}
/// Worn dirt path. NB: no gravity blocks — the meadow surface sits at the piece
/// floor (y=0), so a falling block here would rest over the void; the dirt family
/// is non-falling (the generator gravity invariant enforces this).
fn path_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:coarse_dirt", 0.5),
        r("minecraft:dirt", 0.34),
        r("minecraft:rooted_dirt", 0.16),
    ]
}

fn pick(palette: &[Recipe], n: f64) -> &'static str {
    let total: f64 = palette.iter().map(|e| e.weight).sum();
    let mut acc = 0.0;
    let target = n.clamp(0.0, 0.999_999) * total;
    for e in palette {
        acc += e.weight;
        if target < acc {
            return e.name;
        }
    }
    palette.last().unwrap().name
}

// ---------------------------------------------------------------------------
// Cell grid
// ---------------------------------------------------------------------------

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
    fn blk(&mut self, x: i32, y: i32, z: i32, name: &str, props: Props) {
        self.set(x, y, z, Cell::Block(name.to_string(), props));
    }
    fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.inb(x, y, z) && matches!(self.get(x, y, z), Cell::Block(_, _))
    }
    fn is_air(&self, x: i32, y: i32, z: i32) -> bool {
        self.inb(x, y, z) && matches!(self.get(x, y, z), Cell::Air)
    }
}

// ---------------------------------------------------------------------------
// NBT serialization (vanilla structure format)
// ---------------------------------------------------------------------------

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
                                name: SOCKET_NAME.into(),
                                target: SOCKET_NAME.into(),
                                pool: SOCKET_POOL.into(),
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
// Sockets (keep-socket geometry, island vocabulary)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)] // West completes the cardinal-socket vocabulary; unused by the current terrain set.
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
// Metadata JSON
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
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
#[derive(Serialize, Clone)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    rationale: Option<String>,
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
    /// Local y of the island waterline datum (see [`WATERLINE_Y`]); the compiler
    /// pins it to world sea level when placing an ocean-horizon area (`DW0344`).
    waterline_y: i32,
    anchors: BTreeMap<String, AnchorJson>,
    connectors: Vec<ConnectorJson>,
    lighting: LightingJson,
    license: LicenseJson,
}

fn a_pos(pos: [i32; 3], facing: &str) -> AnchorJson {
    AnchorJson {
        pos: Some(pos),
        facing: Some(facing.to_string()),
        region: None,
        block: None,
    }
}
fn a_region(from: [i32; 3], to: [i32; 3], block: &str) -> AnchorJson {
    AnchorJson {
        pos: None,
        facing: None,
        region: Some(RegionJson { from, to }),
        block: Some(block.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Light estimation (derived, block-light only — conservative authoring estimate)
// ---------------------------------------------------------------------------

fn emission(name: &str) -> i32 {
    match name {
        "minecraft:campfire" => 15,
        "minecraft:lantern" => 15,
        "minecraft:soul_lantern" => 10,
        "minecraft:torch" | "minecraft:wall_torch" => 14,
        "minecraft:glowstone" | "minecraft:shroomlight" | "minecraft:sea_lantern" => 15,
        "minecraft:glow_lichen" => 7,
        "minecraft:magma_block" => 3,
        "minecraft:fire" => 15,
        _ => 0,
    }
}
fn transparent(cell: &Cell) -> bool {
    match cell {
        Cell::Air | Cell::Jigsaw(_) => true,
        Cell::Block(name, _) => matches!(
            name.as_str(),
            "minecraft:water"
                | "minecraft:campfire"
                | "minecraft:lantern"
                | "minecraft:soul_lantern"
                | "minecraft:torch"
                | "minecraft:iron_chain"
                | "minecraft:oak_fence"
                | "minecraft:oak_fence_gate"
                | "minecraft:glow_lichen"
                | "minecraft:vine"
                | "minecraft:pointed_dripstone"
                | "minecraft:moss_carpet"
                | "minecraft:short_grass"
                | "minecraft:poppy"
                | "minecraft:oxeye_daisy"
                | "minecraft:cornflower"
                | "minecraft:dandelion"
        ),
    }
}

/// Static flood-fill block-light estimate over a bounded floor region. `floor_top`
/// is the solid-floor Y; standable cells are the air at `floor_top+1` inside
/// [x0,x1]×[z0,z1]. Returns the min block light over those cells (a conservative
/// authoring estimate — NOT a live probe, and sky light / light shafts are not
/// counted here; the compiler re-measures the assembled world, spec-0010).
fn estimate_min_floor_light(
    grid: &Grid,
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    floor_top: i32,
) -> i32 {
    let [sx, sy, sz] = grid.size;
    let n = (sx * sy * sz) as usize;
    let mut light = vec![0i32; n];
    let mut queue: std::collections::VecDeque<(i32, i32, i32, i32)> =
        std::collections::VecDeque::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if let Cell::Block(name, _) = grid.get(x, y, z) {
                    let e = emission(name);
                    if e > 0 {
                        let i = grid.idx(x, y, z);
                        if e > light[i] {
                            light[i] = e;
                            queue.push_back((x, y, z, e));
                        }
                    }
                }
            }
        }
    }
    while let Some((x, y, z, l)) = queue.pop_front() {
        if l <= 1 {
            continue;
        }
        for (dx, dy, dz) in [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if !grid.inb(nx, ny, nz) || !transparent(grid.get(nx, ny, nz)) {
                continue;
            }
            let i = grid.idx(nx, ny, nz);
            let nl = l - 1;
            if nl > light[i] {
                light[i] = nl;
                queue.push_back((nx, ny, nz, nl));
            }
        }
    }
    let mut min = i32::MAX;
    let wy = floor_top + 1;
    for x in x0..=x1 {
        for z in z0..=z1 {
            let stand = grid.is_air(x, wy, z);
            let head = grid.is_air(x, wy + 1, z);
            let floor_solid = grid.is_solid(x, floor_top, z);
            if stand && head && floor_solid {
                min = min.min(light[grid.idx(x, wy, z)]);
            }
        }
    }
    if min == i32::MAX {
        0
    } else {
        min
    }
}

fn classify(min_light: i32) -> &'static str {
    if min_light >= 7 {
        "lit"
    } else if min_light >= 3 {
        "dim"
    } else {
        "dark"
    }
}

// ---------------------------------------------------------------------------
// Gravity-floor substrate invariant (mirrors cave-generator / compiler DW0313)
// ---------------------------------------------------------------------------

fn is_falling(name: &str) -> bool {
    let id = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        id,
        "sand" | "red_sand" | "gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg"
    ) || id.ends_with("_concrete_powder")
}

/// Belt-and-braces: no gravity block may sit over air in a finished piece (it would
/// despawn in the `the_void` delve world). Both terrain builders back every gravity
/// surface with solid rock/earth by construction, so this must pass with zero fixes;
/// it fails generation if any remain (automating the pitfall out of existence — the
/// compiler's `DW0313` is the authoritative gate).
fn assert_no_unsupported_gravity(id: &str, g: &Grid) -> usize {
    let [sx, sy, sz] = g.size;
    let mut gravity = 0usize;
    let mut bad = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if matches!(g.get(x, y, z), Cell::Block(name, _) if is_falling(name)) {
                    gravity += 1;
                    if !(y > 0 && g.is_solid(x, y - 1, z)) {
                        bad.push([x, y, z]);
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "island-terrain-gen invariant: piece `{id}` has {} unsupported gravity block(s) over air \
         (would despawn in the_void — add a substrate): {:?}",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
    gravity
}

/// The non-falling substrate laid beneath every piece to reach the shared island
/// datum.
const SUBSTRATE: &str = "minecraft:stone";

/// Lift a piece by `dy` and fill the new base layers (y 0..dy) with a solid
/// [`SUBSTRATE`] under any column that carries a block at the lifted floor, so the
/// walk plane lands on the sibling-established datum (island-tileset.md, 2026-08-01:
/// waterline y=2, walk plane y=3, sockets `island:socket` at floor_y=2) with every
/// surface solidly supported (and no gravity block left over the void). Uniform, so
/// all socket/anchor Ys shift by the same `dy`; deterministic (a pure grid map).
fn lift_substrate(g: &Grid, dy: i32) -> Grid {
    let [sx, sy, sz] = g.size;
    let mut out = Grid::new([sx, sy + dy, sz]);
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                out.set(x, y + dy, z, g.get(x, y, z).clone());
            }
        }
    }
    for x in 0..sx {
        for z in 0..sz {
            // fill the base under any column that has a solid or gravity block at the
            // lifted floor (keeps ground supported; leaves genuinely empty sky columns
            // as air so the open-air pieces stay open below their berms).
            if matches!(out.get(x, dy, z), Cell::Block(_, _)) {
                for y in 0..dy {
                    out.blk(x, y, z, SUBSTRATE, None);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Piece spec
// ---------------------------------------------------------------------------

enum Kind {
    Greenfield,
    Mountain,
}

struct Spec {
    id: &'static str,
    kind: Kind,
    size: [i32; 3],
    /// Jigsaw sockets (side, floor_y). Recorded into `connectors[]`.
    doors: Vec<(Side, i32)>,
    anchors: Vec<(&'static str, AnchorJson)>,
    /// Floor region + top for the derived light estimate (x0,x1,z0,z1,floor_top).
    /// Only meaningful for enclosed interiors (mountain); greenfield is sky-lit.
    light_region: Option<(i32, i32, i32, i32, i32)>,
    profile_override: Option<&'static str>,
    rationale: Option<&'static str>,
    salt: u64,
}

// ===========================================================================
// GREENFIELD (open-air meadow connector)
// ===========================================================================

/// The greenfield walk plane (piece-local, before `lift_substrate`): the meadow floor
/// is solid to y=0, so a player on the corridor stands with feet at y=1, head at y=2.
const G_WALK_Y: i32 = 1;

/// How far above the walk plane dressing must clear a walk-corridor cell. The nav model
/// needs 2 cells (player height 1.8); the third keeps an overhanging canopy reading as a
/// natural ceiling instead of a hat brushing the player's head, and leaves margin for
/// the compiler's `DW0311` walk. Corridor dressing is lifted to this height as a whole
/// — never cut off at it (see `place_oak`).
const G_CANOPY_CLEARANCE: i32 = 3;

/// A worn dirt path spine from the south socket to the north/side socket, 3 wide.
/// Returns the path-centre X at a given Z (piece-relative), for the straight and
/// corner variants alike.
fn greenfield_path_center(size: [i32; 3], doors: &[(Side, i32)], z: i32) -> Option<i32> {
    let [sx, _, sz] = size;
    // Every greenfield has a South socket (from the beach side). The second socket
    // is North (straight) or East (bend). The path runs from the south door up to
    // the far door; on the bend it elbows at the meadow centre.
    let has_east = doors.iter().any(|(s, _)| *s == Side::East);
    let sxc = sx / 2;
    if !has_east {
        // straight: centre column the whole way.
        Some(sxc)
    } else {
        // bend: run north on the centre column until the elbow (z = sz/2), then the
        // east leg is drawn separately in build; here just the north leg spine.
        if z >= sz / 2 {
            Some(sxc)
        } else {
            None
        }
    }
}

fn build_greenfield(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, _sy, sz] = spec.size;
    let has_east = spec.doors.iter().any(|(s, _)| *s == Side::East);

    // 1. Ground: a solid earth body (dirt) capped by a grass surface at y=0, with a
    //    low bounding berm rising at the piece edges so no interior walkable cell
    //    borders the void (a bounded box-garden 箱庭, sky-lit; cf. cave-shore). The
    //    berm dips to floor level only in the 3-wide socket lanes.
    for x in 0..sx {
        for z in 0..sz {
            let surf = greenfield_surface(spec, x, z, seed); // top solid Y of the ground/berm
            for y in 0..=surf {
                let top = y == surf;
                let name = if top {
                    if let Some(pc) = greenfield_path_center(spec.size, &spec.doors, z) {
                        if (x - pc).abs() <= 1 && surf == 0 {
                            pick(&path_palette(), value_noise(seed, x, y, z, 0.4, 11))
                        } else {
                            pick(&meadow_palette(), value_noise(seed, x, y, z, 0.16, 21))
                        }
                    } else {
                        pick(&meadow_palette(), value_noise(seed, x, y, z, 0.16, 21))
                    }
                } else {
                    // sub-surface: dirt body, stone deeper.
                    if surf - y >= 3 {
                        "minecraft:stone"
                    } else {
                        "minecraft:dirt"
                    }
                };
                g.blk(x, y, z, name, None);
            }
        }
    }
    // East leg of the bend path (grass→dirt) along z = sz/2 .. running east.
    if has_east {
        let zc = sz / 2;
        for x in (sx / 2)..sx {
            for dz in -1..=1 {
                let z = zc + dz;
                if g.inb(x, 0, z)
                    && g.is_solid(x, 0, z)
                    && greenfield_surface(spec, x, z, seed) == 0
                {
                    g.blk(
                        x,
                        0,
                        z,
                        pick(&path_palette(), value_noise(seed, x, 0, z, 0.4, 12)),
                        None,
                    );
                }
            }
        }
    }

    // 2. Carve the socket lanes through the berm (3 wide, floor-level) + jigsaw.
    for &(side, fy) in &spec.doors {
        let (cells, jc) = doorway_cells(spec.size, side, fy);
        for c in &cells {
            g.set(c[0], c[1], c[2], Cell::Air);
        }
        g.set(jc[0], jc[1], jc[2], Cell::Jigsaw(side.orientation()));
    }

    // 3. Scatter: oaks, flowers, tufts, and the empty sheep fold. All off the path
    //    spine and the socket lanes so the walk envelope stays clear.
    greenfield_scatter(spec, g, seed);

    // 4. Prove the walk envelope survived the dressing.
    assert_greenfield_corridor_clear(spec, g, seed);
}

/// Top solid Y of the greenfield ground at (x,z): 0 across the meadow, rising into
/// a low bounding berm toward the piece edges (except in the socket lanes).
fn greenfield_surface(spec: &Spec, x: i32, z: i32, seed: u64) -> i32 {
    let [sx, sy, sz] = spec.size;
    // distance to the nearest edge
    let de = x.min(sx - 1 - x).min(z.min(sz - 1 - z));
    // socket lanes stay at floor level so the doorway opens cleanly
    for &(side, _fy) in &spec.doors {
        let jc = door_center(spec.size, side, 0);
        let in_lane = match side {
            Side::North | Side::South => (x - jc[0]).abs() <= 1 && (z - jc[2]).abs() <= 2,
            Side::West | Side::East => (z - jc[2]).abs() <= 1 && (x - jc[0]).abs() <= 2,
        };
        if in_lane {
            return 0;
        }
    }
    // A low grassy bank rings the dell (bounds walk-off at the very edge — no
    // void-adjacent walk cell — without walling the meadow into a pit); the interior
    // is a flat open meadow. Socket lanes (handled above) dip to floor level.
    let _ = sy;
    match de {
        0 => 3,                                                       // outer bank
        1 => 1 + (value_noise(seed, x, 0, z, 0.3, 31) > 0.55) as i32, // rolling foot
        2 => (value_noise(seed, x, 0, z, 0.35, 33) > 0.7) as i32,     // occasional tussock
        _ => 0,
    }
}

/// The greenfield walk corridor: the 3-wide path spine (both legs of the bend) and the
/// socket lanes. The single authority on "keep this clear" — shared by the dressing
/// scatter, the canopy shape rule, and `assert_greenfield_corridor_clear`.
fn greenfield_on_walk(spec: &Spec, x: i32, z: i32) -> bool {
    let [sx, _sy, sz] = spec.size;
    // path spine or socket lane — keep clear
    if let Some(pc) = greenfield_path_center(spec.size, &spec.doors, z) {
        if (x - pc).abs() <= 1 {
            return true;
        }
    }
    let has_east = spec.doors.iter().any(|(s, _)| *s == Side::East);
    if has_east && (z - sz / 2).abs() <= 1 && x >= sx / 2 {
        return true;
    }
    for &(side, _fy) in &spec.doors {
        let jc = door_center(spec.size, side, 0);
        let lane = match side {
            Side::North | Side::South => (x - jc[0]).abs() <= 1 && (z - jc[2]).abs() <= 3,
            Side::West | Side::East => (z - jc[2]).abs() <= 1 && (x - jc[0]).abs() <= 3,
        };
        if lane {
            return true;
        }
    }
    false
}

/// Generator invariant (debug doctrine): dressing may never intrude on the greenfield
/// walk corridor below `G_WALK_Y + G_CANOPY_CLEARANCE`, and the corridor itself is flat
/// at the meadow datum. Fails generation rather than letting a sheared canopy or a stray
/// prop reach the compiler's `DW0311` gate — or the owner's QA hour.
fn assert_greenfield_corridor_clear(spec: &Spec, g: &Grid, seed: u64) {
    let [sx, _sy, sz] = spec.size;
    for x in 0..sx {
        for z in 0..sz {
            if !greenfield_on_walk(spec, x, z) {
                continue;
            }
            let surf = greenfield_surface(spec, x, z, seed);
            assert_eq!(
                surf, 0,
                "{}: corridor cell ({x},{z}) is not at the meadow datum (surface {surf})",
                spec.id
            );
            // Air or the socket's jigsaw marker (vanilla replaces it at placement) only —
            // any block, collidable or not, is dressing that does not belong here.
            for y in G_WALK_Y..(G_WALK_Y + G_CANOPY_CLEARANCE) {
                assert!(
                    !g.is_solid(x, y, z),
                    "{}: corridor cell ({x},{y},{z}) is obstructed — dressing must clear \
                     the corridor by {G_CANOPY_CLEARANCE} as a whole shape, never be cut off at it",
                    spec.id
                );
            }
        }
    }
}

fn greenfield_scatter(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, _sy, sz] = spec.size;
    let on_walk = |x: i32, z: i32| -> bool { greenfield_on_walk(spec, x, z) };
    // Anchor cells + their forward sightline stay clear of tree/flower dressing so the
    // meadow/fold anchors read clean and their facing view is unobstructed.
    let protected = |x: i32, z: i32| -> bool {
        for (_, a) in &spec.anchors {
            let Some(p) = a.pos else { continue };
            let (dx, dz) = match a.facing.as_deref() {
                Some("north") => (0, -1),
                Some("south") => (0, 1),
                Some("east") => (1, 0),
                Some("west") => (-1, 0),
                _ => (0, 0),
            };
            for k in 0..=2 {
                if x == p[0] + dx * k && z == p[2] + dz * k {
                    return true;
                }
            }
        }
        false
    };
    // Low mossy-cobblestone empty sheep fold (foreshadowing — the sheep are his):
    // a 1-tall wall rectangle with a gap "gate", in the west meadow away from path.
    let fold = fold_rect(spec);
    if let Some((fx0, fz0, fx1, fz1)) = fold {
        for x in fx0..=fx1 {
            for &z in &[fz0, fz1] {
                if g.is_solid(x, 0, z) {
                    g.blk(x, 1, z, "minecraft:mossy_cobblestone", None);
                }
            }
        }
        for z in fz0..=fz1 {
            for &x in &[fx0, fx1] {
                if g.is_solid(x, 0, z) {
                    g.blk(x, 1, z, "minecraft:cobblestone_wall", None);
                }
            }
        }
        // a gap gate on the south fold edge
        let gx = (fx0 + fx1) / 2;
        g.set(gx, 1, fz1, Cell::Air);
        // a tuft of hay by the fold
        if g.is_solid(fx0 + 1, 0, fz0 + 1) {
            g.blk(fx0 + 1, 1, fz0 + 1, "minecraft:short_grass", None);
        }
    }
    // Oaks: 2–3 small hand-shaped oaks per piece, chosen deterministically from the
    // highest-noise off-corridor meadow cells with a spacing rule so they spread out.
    // Trunks are strictly off the walk corridor, the fold, and the anchor sightlines. A
    // canopy that would reach the corridor leans away from it, or — failing that — grows
    // tall enough to arch over it whole (`place_oak`); it is never sliced, and never
    // intrudes below G_WALK_Y + G_CANOPY_CLEARANCE, so the socket-to-socket path keeps
    // full headroom (the compiler's DW0311 gate is the authority at assembly).
    let mut cand: Vec<(f64, i32, i32)> = Vec::new();
    for x in 2..sx - 2 {
        for z in 2..sz - 2 {
            if on_walk(x, z) || in_fold(spec, x, z) || protected(x, z) {
                continue;
            }
            if greenfield_surface(spec, x, z, seed) != 0 {
                continue;
            }
            if g.is_solid(x, 0, z) && g.is_air(x, 1, z) {
                cand.push((value_noise(seed, x, 5, z, 0.5, 41), x, z));
            }
        }
    }
    cand.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut planted: Vec<(i32, i32)> = Vec::new();
    for (_, x, z) in cand {
        if planted.len() >= 3 {
            break;
        }
        // reject only when close on BOTH axes so the oaks stay spread across the dell
        if planted
            .iter()
            .any(|&(px, pz)| (px - x).abs() < 4 && (pz - z).abs() < 4)
        {
            continue;
        }
        place_oak(g, x, z, seed, &on_walk);
        planted.push((x, z));
    }

    // Poppies + daisies (the design-named meadow flowers) with an occasional cornflower
    // accent and a short-grass dusting, scattered on the open grass only (never on the
    // worn dirt path — restricted to true grass cells — never in the fold pen, on an
    // anchor, or under a trunk). Noise-thresholded to ~10% flower density so the meadow
    // reads as a wildflower pasture, not a bare courtyard. The array is weighted (poppy
    // 2 / daisy 2 / cornflower 1) so the two named flowers dominate on small samples.
    let flowers = [
        "minecraft:poppy",
        "minecraft:oxeye_daisy",
        "minecraft:poppy",
        "minecraft:oxeye_daisy",
        "minecraft:cornflower",
    ];
    for x in 1..sx - 1 {
        for z in 1..sz - 1 {
            if on_walk(x, z) || in_fold(spec, x, z) || protected(x, z) {
                continue;
            }
            if greenfield_surface(spec, x, z, seed) != 0 {
                continue;
            }
            // grass only — the dirt path, podzol/moss dapples, and trunks stay bare
            if !matches!(g.get(x, 0, z), Cell::Block(name, _) if name == "minecraft:grass_block") {
                continue;
            }
            if !g.is_air(x, 1, z) {
                continue;
            }
            if hash01(seed, x, 1, z, 151) < 0.10 {
                let f = flowers
                    [(hash01(seed, x, 3, z, 155) * flowers.len() as f64) as usize % flowers.len()];
                g.blk(x, 1, z, f, None);
            } else if hash01(seed, x, 2, z, 153) < 0.15 {
                g.blk(x, 1, z, "minecraft:short_grass", None);
            }
        }
    }
}

/// Horizontal radius of the widest canopy layer, and the squared radius that rounds
/// the leaf ball off. The blob footprint is `dx² + dz² <= CANOPY_R2`.
const CANOPY_RAD: i32 = 2;
const CANOPY_R2: i32 = 5;

/// Whether a canopy blob centred at (cx,cz) would cover any walk-corridor cell.
fn canopy_over_corridor(cx: i32, cz: i32, on_walk: &impl Fn(i32, i32) -> bool) -> bool {
    (-CANOPY_RAD..=CANOPY_RAD).any(|dx| {
        (-CANOPY_RAD..=CANOPY_RAD)
            .any(|dz| dx * dx + dz * dz <= CANOPY_R2 && on_walk(cx + dx, cz + dz))
    })
}

/// The one-block step that leans a canopy directly away from the nearest walk-corridor
/// cell within `reach` of a trunk at (x,z); `(0,0)` when no corridor is in reach.
/// Deterministic: nearest by squared distance, ties broken by the fixed scan order.
fn corridor_lean(x: i32, z: i32, reach: i32, on_walk: &impl Fn(i32, i32) -> bool) -> (i32, i32) {
    let mut best: Option<(i32, i32, i32)> = None;
    for dx in -reach..=reach {
        for dz in -reach..=reach {
            if (dx, dz) == (0, 0) || !on_walk(x + dx, z + dz) {
                continue;
            }
            let d2 = dx * dx + dz * dz;
            if best.is_none_or(|(b, _, _)| d2 < b) {
                best = Some((d2, dx, dz));
            }
        }
    }
    best.map_or((0, 0), |(_, dx, dz)| (-dx.signum(), -dz.signum()))
}

/// Place one hand-shaped oak rooted at (x,z) on the meadow floor (y=0 solid, y=1 air).
///
/// The corridor rule is **structural, never a cut**. The trunk is always off the walk
/// corridor (the caller's planting filter), and the leaf ball is only ever grown whole:
///
/// 1. **Lean.** An oak whose blob would reach over the corridor first leans one block
///    directly away from it — oaks crowd off trodden ground, and the offset blob still
///    caps the trunk. If that clears the corridor the oak keeps its natural small
///    height (3–4 logs).
/// 2. **Grow.** If leaning is not enough, the oak grows tall instead: the trunk is
///    raised so the *entire* canopy sits at `G_WALK_Y + G_CANOPY_CLEARANCE`, arching
///    over the path as a ceiling with full headroom beneath it.
///
/// No leaf is ever skipped for being over the corridor, so no oak is left vertically
/// sheared. `assert_greenfield_corridor_clear` is the generator-side proof.
fn place_oak(g: &mut Grid, x: i32, z: i32, seed: u64, on_walk: &impl Fn(i32, i32) -> bool) {
    // 1. Lean away from the corridor; keep the small-oak height when that clears it.
    let (lean_x, lean_z) = corridor_lean(x, z, CANOPY_RAD + 1, on_walk);
    // 2. Otherwise centre the blob back on the trunk and lift it clear of the corridor.
    let (cx, cz, base) = if canopy_over_corridor(x + lean_x, z + lean_z, on_walk) {
        (x, z, G_WALK_Y + G_CANOPY_CLEARANCE)
    } else {
        let natural = 2 + (value_noise(seed, x, 0, z, 0.6, 43) > 0.5) as i32; // base 2 or 3
        (x + lean_x, z + lean_z, natural)
    };
    let h = base + 1; // trunk top sits inside the blob's mid layer
    for y in G_WALK_Y..=h {
        g.blk(x, y, z, "minecraft:oak_log", Some(vec![("axis", "y")]));
    }
    // compact leaf ball: a rounded 5-wide band at the trunk top, narrowing upward.
    for dy in base..=(base + 2) {
        let rad = if dy == base + 2 { 1 } else { CANOPY_RAD };
        for dx in -rad..=rad {
            for dz in -rad..=rad {
                let (lx, lz) = (cx + dx, cz + dz);
                let r2 = dx * dx + dz * dz + (dy - h) * (dy - h);
                if r2 <= CANOPY_R2 && g.inb(lx, dy, lz) && g.is_air(lx, dy, lz) {
                    g.blk(
                        lx,
                        dy,
                        lz,
                        "minecraft:oak_leaves",
                        Some(vec![("persistent", "true")]),
                    );
                }
            }
        }
    }
    // a single crown leaf caps the ball
    if g.inb(cx, base + 3, cz) && g.is_air(cx, base + 3, cz) {
        g.blk(
            cx,
            base + 3,
            cz,
            "minecraft:oak_leaves",
            Some(vec![("persistent", "true")]),
        );
    }
}

/// The empty sheep-fold rectangle (inclusive corners) in the west meadow, or None.
fn fold_rect(spec: &Spec) -> Option<(i32, i32, i32, i32)> {
    let [sx, _, sz] = spec.size;
    if sx < 11 || sz < 11 {
        return None;
    }
    // west side, a 5×5-ish pen clear of the central path spine and edges
    let fx0 = 2;
    let fx1 = 6.min(sx / 2 - 2);
    let fz0 = sz / 2 - 2;
    let fz1 = sz / 2 + 2;
    if fx1 - fx0 >= 3 && fz1 - fz0 >= 3 {
        Some((fx0, fz0, fx1, fz1))
    } else {
        None
    }
}
fn in_fold(spec: &Spec, x: i32, z: i32) -> bool {
    if let Some((a, b, c, d)) = fold_rect(spec) {
        x >= a && x <= c && z >= b && z <= d
    } else {
        false
    }
}

// ===========================================================================
// MOUNTAIN (solid massif, carved: switchback face + tall-wide cavern)
// ===========================================================================

// Fixed interior geometry (local coords, floor_y=0 convention → walk = solid_top+1).
const M_FLOOR_TOP: i32 = 6; // cavern floor solid top; walk at y=7
const M_WALK: i32 = 7;
const M_CEIL: i32 = 21; // first ceiling rock layer
const M_CAV_X0: i32 = 3;
const M_CAV_X1: i32 = 32; // interior width 30 (x 3..32)
const M_CAV_Z0: i32 = 3;
const M_CAV_Z1: i32 = 26; // interior depth 24 (z 3..26)
const M_MOUTH_WALL_Z0: i32 = 27;
const M_MOUTH_WALL_Z1: i32 = 29;
const M_SLOPE_Z0: i32 = 30; // slope region z 30..sz-1 (south face, open to sky)
const M_SHELF_X0: i32 = 20;
const M_SHELF_X1: i32 = 31;
const M_SHELF_Z0: i32 = 4;
const M_SHELF_Z1: i32 = 10;
const M_SHELF_TOP: i32 = 10; // upper pen platform solid top; walk at y=11
const M_MOUTH_XC: i32 = 17; // mouth centre x

/// The exterior mountain surface (top solid Y) at (x,z) BEFORE carving. The massif
/// caps the cavern to a rough peak and the south face descends as terraces to the
/// base; sides stay full-height to bound the slot.
fn mountain_surface(sx: i32, sy: i32, sz: i32, x: i32, z: i32, seed: u64) -> i32 {
    let peak = sy - 1;
    let side = !(M_CAV_X0..=M_CAV_X1).contains(&x);
    if z < M_SLOPE_Z0 {
        // Massif over the cavern body + mouth wall: a domed rocky cap (rounded, not a
        // flat plateau) with a ragged crown. Radial dome from the massif centre.
        let cx = (M_CAV_X0 + M_CAV_X1) as f64 / 2.0;
        let cz = (M_CAV_Z0 + M_CAV_Z1) as f64 / 2.0;
        let dx = (x as f64 - cx) / (sx as f64 / 2.0);
        let dz = (z as f64 - cz) / (sz as f64 / 2.0);
        let r = (dx * dx + dz * dz).sqrt().min(1.0);
        let dome = ((1.0 - r) * (peak - M_CEIL) as f64).round() as i32;
        let rough = (value_noise(seed, x, 0, z, 0.28, 61) * 3.0).floor() as i32;
        (M_CEIL + dome - rough).clamp(M_CEIL, peak)
    } else if side {
        // slope-region side walls: full-height bank so the face is a bounded slot
        let rough = (value_noise(seed, x, 0, z, 0.30, 63) * 3.0).floor() as i32;
        (peak - rough).max(M_WALK + 3)
    } else {
        // The terraced south face: flat walkable terraces (solid top = walk−1) across
        // the full width; risers between terraces are dressed with stairs so the bot
        // climbs natively. A cosmetic switchback trail is painted on top separately.
        terrace_walk(z) - 1
    }
}

/// Terraced walk height on the south face at depth z: rises 1 (base, z=sz-1) → 7
/// (ledge, z=M_SLOPE_Z0) in ~0.55/step floor bands (each riser a 1-block step the
/// bot walks natively via a stair).
fn terrace_walk(z: i32) -> i32 {
    let span = (41 - M_SLOPE_Z0) as f64; // depth of the slope run
    let t = (41 - z) as f64;
    (1 + (t * 6.0 / span).floor() as i32).clamp(1, M_WALK)
}

/// The cosmetic 3-wide switchback trail centre X at face depth z (a zig then a zag).
/// Purely visual — the whole terraced face is walkable, so a trail discontinuity
/// cannot break traversal; it just draws the eye up the mountainside.
fn mountain_path_center(sx: i32, z: i32) -> i32 {
    let west = 6;
    let east = sx - 6; // 30
    if z >= 37 {
        let t = (41 - z) as f64 / 4.0;
        (M_MOUTH_XC as f64 + (west as f64 - M_MOUTH_XC as f64) * t).round() as i32
    } else if z >= 32 {
        let t = (37 - z) as f64 / 5.0;
        (west as f64 + (east as f64 - west as f64) * t).round() as i32
    } else {
        let t = (32 - z) as f64 / 2.0;
        (east as f64 + (M_MOUTH_XC as f64 - east as f64) * t).round() as i32
    }
}

/// Dress the terraced south face: a stair tread at every riser (native bot walking),
/// a grass→stone gradient (grassy low terraces, bare rock high), and the cosmetic
/// coarse-dirt switchback trail. Runs after the massif fill.
fn slope_finalize(g: &mut Grid, seed: u64) {
    let [sx, _sy, sz] = g.size;
    for x in M_CAV_X0..=M_CAV_X1 {
        for z in M_SLOPE_Z0..sz {
            let w = terrace_walk(z);
            let top = w - 1;
            if !g.inb(x, top, z) {
                continue;
            }
            let riser = z + 1 < sz && terrace_walk(z) > terrace_walk(z + 1);
            let on_trail = (x - mountain_path_center(sx, z)).abs() <= 1;
            if riser {
                // stair tread, facing north (ascending northward, cf. cave-descent)
                g.blk(
                    x,
                    top,
                    z,
                    "minecraft:cobblestone_stairs",
                    Some(vec![
                        ("facing", "north"),
                        ("half", "bottom"),
                        ("shape", "straight"),
                        ("waterlogged", "false"),
                    ]),
                );
            } else if on_trail {
                g.blk(
                    x,
                    top,
                    z,
                    pick(&path_palette(), value_noise(seed, x, top, z, 0.4, 65)),
                    None,
                );
            } else if w <= 3 {
                // low terraces green over (grass-to-stone gradient)
                let n = value_noise(seed, x, top, z, 0.22, 67);
                let b = if n > 0.35 {
                    "minecraft:grass_block"
                } else if n > 0.18 {
                    "minecraft:coarse_dirt"
                } else {
                    pick(&massif_palette(), n)
                };
                g.blk(x, top, z, b, None);
            } else if w <= 5 && value_noise(seed, x, top, z, 0.3, 68) > 0.7 {
                g.blk(x, top, z, "minecraft:coarse_dirt", None);
            } else {
                g.blk(
                    x,
                    top,
                    z,
                    pick(&massif_palette(), value_noise(seed, x, top, z, 0.2, 69)),
                    None,
                );
            }
            // keep 2 air above every terrace tread so the walk envelope is clear
            g.set(x, top + 1, z, Cell::Air);
            g.set(x, top + 2, z, Cell::Air);
        }
    }
}

/// Break the boxy massif silhouette: erode the crown + upper vertical faces of the
/// perimeter (A5-style), only removing outer shell rock where an inner cell stays
/// solid (never breaches the cavern/void), and only above the cavern ceiling so the
/// enclosure and the walkable slope are untouched. Skips the mouth/socket columns.
fn roughen_massif(g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = g.size;
    let perim = |x: i32, z: i32| x <= 1 || x >= sx - 2 || z <= 1 || z >= sz - 2;
    // crown erosion: nibble the top few solid cells of perimeter columns over the body
    for x in 0..sx {
        for z in 0..M_SLOPE_Z0 {
            if !perim(x, z) {
                continue;
            }
            let n = value_noise(seed, x, 0, z, 0.5, 201);
            let depth = (n * 3.4).floor() as i32;
            let mut removed = 0;
            let mut y = sy - 1;
            while removed < depth && y > M_CEIL {
                if g.is_solid(x, y, z) && g.is_solid(x, y - 1, z) {
                    g.set(x, y, z, Cell::Air);
                    removed += 1;
                }
                y -= 1;
            }
        }
    }
    // vertical-face divots down the full body height (only where a cell one step
    // inward stays solid, so an inner wall layer always remains — the 3-thick walls
    // guarantee no breach to the cavern/void). Breaks the flat cube faces into rock.
    for y in 1..(sy - 1) {
        for x in 0..sx {
            for z in 0..M_SLOPE_Z0 {
                let outer = x == 0 || x == sx - 1 || z == 0;
                if !outer || !g.is_solid(x, y, z) {
                    continue;
                }
                let (ix, iz) = if x == 0 {
                    (1, 0)
                } else if x == sx - 1 {
                    (-1, 0)
                } else if z == 0 {
                    (0, 1)
                } else {
                    (0, -1)
                };
                if g.is_solid(x + ix, y, z + iz) && value_noise(seed, x, y, z, 0.45, 211) > 0.82 {
                    g.set(x, y, z, Cell::Air);
                }
            }
        }
    }
}

fn build_mountain(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = spec.size;

    // 1. Fill the solid massif up to the exterior surface, palette by role.
    for x in 0..sx {
        for z in 0..sz {
            let surf = mountain_surface(sx, sy, sz, x, z, seed);
            for y in 0..=surf {
                let name = mountain_block(x, y, z, surf, seed);
                g.blk(x, y, z, name, None);
            }
        }
    }

    // 1b. Dress the terraced switchback face (stair treads + grass gradient + trail)
    //     and break the boxy massif silhouette (crown + face erosion, enclosure-safe).
    slope_finalize(g, seed);
    roughen_massif(g, seed);

    // 2. Carve the cavern hall (air y=7..20 across the 30×24 interior).
    for x in M_CAV_X0..=M_CAV_X1 {
        for z in M_CAV_Z0..=M_CAV_Z1 {
            for y in (M_FLOOR_TOP + 1)..M_CEIL {
                g.set(x, y, z, Cell::Air);
            }
            // (re)lay the floor top so carving leaves a clean palette surface
            g.blk(
                x,
                M_FLOOR_TOP,
                z,
                pick(
                    &floor_palette(),
                    value_noise(seed, x, M_FLOOR_TOP, z, 0.16, 71),
                ),
                None,
            );
        }
    }

    // 3. Upper sheep-pen shelf: a raised rock platform (solid to y=10) in the NE.
    for x in M_SHELF_X0..=M_SHELF_X1 {
        for z in M_SHELF_Z0..=M_SHELF_Z1 {
            for y in (M_FLOOR_TOP + 1)..=M_SHELF_TOP {
                let name = if y == M_SHELF_TOP {
                    pick(&floor_palette(), value_noise(seed, x, y, z, 0.16, 72))
                } else {
                    pick(&wall_palette(), value_noise(seed, x, y, z, 0.17, 73))
                };
                g.blk(x, y, z, name, None);
            }
        }
    }

    // 4. Rock-shelf ramp (no ladders) from the cavern floor up to the shelf, 3 wide.
    //    Solid steps + a stair tread each riser (bot-walkable, cf. keep/cave stair).
    build_mountain_ramp(g, seed);

    // 5. The mouth: carve a 3×3 opening through the mouth wall at the ledge (walk 7).
    for x in (M_MOUTH_XC - 1)..=(M_MOUTH_XC + 1) {
        for y in M_WALK..=(M_WALK + 2) {
            for z in M_MOUTH_WALL_Z0..=M_MOUTH_WALL_Z1 {
                g.set(x, y, z, Cell::Air);
            }
        }
    }

    // 6. Shadow alcoves: carve dark recesses into the walls at floor level (no light).
    carve_alcove(g, 1, 2, 7, 9, 8, 8); // west, z8
    carve_alcove(g, 1, 2, 7, 9, 19, 20); // west, z19-20
    carve_alcove(g, sx - 3, sx - 2, 7, 9, 17, 18); // east, z17-18
    carve_alcove(g, 9, 11, 7, 9, 1, 2); // north, x9-11

    // 7. Two ceiling light shafts: 2×2 columns from the cavern up through the cap
    //    to the sky (sky-open in the assembled world → beam accents, spec-0010).
    carve_shaft(g, sy, 10, 11, 10, 11);
    carve_shaft(g, sy, 24, 25, 18, 19);

    // 8. Dressing: dripstone from the ceiling, moss patches on the floor, vines +
    //    sparse glow-lichen on the walls (kept minimal so the hall reads dark).
    mountain_dressing(spec, g, seed);

    // 9. Modules: fire pit, cheese store, upper pen, decorative Chekhov boulder.
    mountain_modules(g, seed);

    // 10. The base entry socket (South, floor_y 0) into the greenfield.
    let (cells, jc) = doorway_cells(spec.size, Side::South, 0);
    for c in &cells {
        g.set(c[0], c[1], c[2], Cell::Air);
    }
    g.set(jc[0], jc[1], jc[2], Cell::Jigsaw(Side::South.orientation()));
}

fn mountain_block(x: i32, y: i32, z: i32, surf: i32, seed: u64) -> &'static str {
    // exterior surface skin: grass→stone on the lower south face, else rock.
    if y == surf && z >= M_SLOPE_Z0 && surf <= M_WALK + 1 {
        // lower slope face greens over (grass-to-stone transition)
        let greeny = value_noise(seed, x, y, z, 0.22, 81);
        if surf <= 3 && greeny > 0.35 {
            return "minecraft:grass_block";
        }
        if surf <= 5 && greeny > 0.7 {
            return "minecraft:coarse_dirt";
        }
        return pick(&massif_palette(), value_noise(seed, x, y, z, 0.2, 83));
    }
    if y == M_CEIL {
        // cavern ceiling underside: darker rock + a little dripstone
        return pick(&ceiling_palette(), value_noise(seed, x, y, z, 0.18, 84));
    }
    if y > M_CEIL {
        return pick(&massif_palette(), value_noise(seed, x, y, z, 0.2, 85));
    }
    // NB: no gravity-prone floor palette in the massif body — the cavern floor and
    // the pen shelf lay their own (solid-backed) floor surface after carving. Keeping
    // the body rock means the silhouette erosion can never strand a falling block.
    pick(&wall_palette(), value_noise(seed, x, y, z, 0.17, 87))
}

fn build_mountain_ramp(g: &mut Grid, seed: u64) {
    // ascends northward (−z) from z=14 (walk 7, cavern floor) to z=10 (walk 11,
    // shelf). Tread tops: z14→6(=floor), z13→7, z12→8, z11→9, z10→10(=shelf).
    let tops = [(13, 7), (12, 8), (11, 9)];
    for x in 24..=26 {
        // solid fill below every ramp column
        for &(z, top) in &tops {
            for y in (M_FLOOR_TOP + 1)..top {
                g.blk(
                    x,
                    y,
                    z,
                    pick(&wall_palette(), value_noise(seed, x, y, z, 0.17, 91)),
                    None,
                );
            }
            g.blk(
                x,
                top,
                z,
                "minecraft:cobblestone_stairs",
                Some(vec![
                    ("facing", "north"),
                    ("half", "bottom"),
                    ("shape", "straight"),
                    ("waterlogged", "false"),
                ]),
            );
            // keep 2 air above each tread
            g.set(x, top + 1, z, Cell::Air);
            g.set(x, top + 2, z, Cell::Air);
        }
        // approach cell (z14) is cavern floor; landing onto shelf at z10 already solid.
        g.set(x, M_WALK + 1, 14, Cell::Air);
        g.set(x, M_SHELF_TOP + 1, 10, Cell::Air);
        g.set(x, M_SHELF_TOP + 2, 10, Cell::Air);
    }
}

/// Carve a dark recess into a wall: air over [x0,x1]×[z0,z1] at y[ylo,yhi], leaving
/// the outer boundary column solid (never opens to the void), floor solid below.
fn carve_alcove(g: &mut Grid, x0: i32, x1: i32, ylo: i32, yhi: i32, z0: i32, z1: i32) {
    for x in x0..=x1 {
        for z in z0..=z1 {
            for y in ylo..=yhi {
                if g.inb(x, y, z) {
                    g.set(x, y, z, Cell::Air);
                }
            }
            // ensure a solid floor under the recess (it is massif rock; guarantee it)
            if g.inb(x, ylo - 1, z) && !g.is_solid(x, ylo - 1, z) {
                g.blk(x, ylo - 1, z, "minecraft:cobblestone", None);
            }
        }
    }
}

/// Carve a 2×2 light shaft from the cavern ceiling up through the cap to the top.
fn carve_shaft(g: &mut Grid, sy: i32, x0: i32, x1: i32, z0: i32, z1: i32) {
    for x in x0..=x1 {
        for z in z0..=z1 {
            for y in M_CEIL..sy {
                g.set(x, y, z, Cell::Air);
            }
        }
    }
}

fn mountain_dressing(_spec: &Spec, g: &mut Grid, seed: u64) {
    // hanging dripstone from the ceiling
    for x in M_CAV_X0..=M_CAV_X1 {
        for z in M_CAV_Z0..=M_CAV_Z1 {
            if !g.is_air(x, M_CEIL - 1, z) {
                continue;
            }
            let n = value_noise(seed, x, M_CEIL - 1, z, 0.34, 101);
            if n > 0.80 && g.is_solid(x, M_CEIL, z) {
                g.blk(
                    x,
                    M_CEIL - 1,
                    z,
                    "minecraft:pointed_dripstone",
                    Some(vec![
                        ("vertical_direction", "down"),
                        ("thickness", if n > 0.9 { "middle" } else { "tip" }),
                        ("waterlogged", "false"),
                    ]),
                );
                if n > 0.9 && g.is_air(x, M_CEIL - 2, z) {
                    g.blk(
                        x,
                        M_CEIL - 2,
                        z,
                        "minecraft:pointed_dripstone",
                        Some(vec![
                            ("vertical_direction", "down"),
                            ("thickness", "tip"),
                            ("waterlogged", "false"),
                        ]),
                    );
                }
            }
        }
    }
    // moss patches + carpet on the cavern floor (edges), away from the centre spine
    for x in M_CAV_X0..=M_CAV_X1 {
        for z in M_CAV_Z0..=M_CAV_Z1 {
            if !g.is_air(x, M_WALK, z) || !g.is_solid(x, M_FLOOR_TOP, z) {
                continue;
            }
            let edge =
                x <= M_CAV_X0 + 1 || x >= M_CAV_X1 - 1 || z <= M_CAV_Z0 + 1 || z >= M_CAV_Z1 - 1;
            if edge && value_noise(seed, x, M_FLOOR_TOP, z, 0.4, 103) > 0.8 {
                g.blk(x, M_FLOOR_TOP, z, "minecraft:moss_block", None);
                if value_noise(seed, x, M_WALK, z, 0.5, 104) > 0.6 {
                    g.blk(x, M_WALK, z, "minecraft:moss_carpet", None);
                }
            }
        }
    }
    // vines + sparse glow-lichen on upper wall faces (start y=10 so nothing lands in
    // the walk envelope; lichen kept rare so the hall stays dark for the stealth beats)
    for x in M_CAV_X0..=M_CAV_X1 {
        for y in 10..M_CEIL {
            for z in M_CAV_Z0..=M_CAV_Z1 {
                if !g.is_air(x, y, z) {
                    continue;
                }
                // What would grow here is decided first, because which faces a
                // decal can hold on to is a fact about the block: a vine has
                // five, a lichen six.
                let ln = value_noise(seed, x, y, z, 0.5, 105);
                let decal = if ln > 0.95 {
                    "minecraft:glow_lichen"
                } else if ln < 0.08 && y >= M_CEIL - 3 {
                    "minecraft:vine"
                } else {
                    continue;
                };
                // Where it may hold on is `connections`' question, not this
                // scan's: the module owns which faces the block has and pairs
                // each with the direction it looks in, so this pass can neither
                // name a face pointing away from the rock nor forget that rock
                // overhead is rock. The first answer is the best one — a wall
                // if there is one, the ceiling if there is not.
                let Some(face) = connections::attachable_faces(decal, [x, y, z], |p| {
                    neighbour_state(g, p[0], p[1], p[2])
                })
                .first()
                .copied() else {
                    continue;
                };
                g.blk(x, y, z, decal, Some(vec![(face, "true")]));
            }
        }
    }
}

fn mountain_modules(g: &mut Grid, seed: u64) {
    // Central fire pit: a stone ring + lit campfire + a leaning log.
    let (cx, cz) = (M_MOUTH_XC, 14);
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            g.blk(
                cx + dx,
                M_WALK,
                cz + dz,
                pick(
                    &boulder_palette(),
                    value_noise(seed, cx + dx, M_WALK, cz + dz, 0.5, 111),
                ),
                None,
            );
        }
    }
    g.blk(
        cx,
        M_WALK,
        cz,
        "minecraft:campfire",
        Some(vec![("lit", "true"), ("facing", "north")]),
    );
    g.blk(
        cx - 2,
        M_WALK,
        cz + 1,
        "minecraft:oak_log",
        Some(vec![("axis", "x")]),
    );

    // Cheese store by the entry: a plank shelf with barrels + hay (interact spot).
    for z in 24..=25 {
        g.blk(
            21,
            M_WALK,
            z,
            "minecraft:barrel",
            Some(vec![("facing", "up")]),
        );
        g.blk(
            23,
            M_WALK,
            z,
            "minecraft:barrel",
            Some(vec![("facing", "up")]),
        );
    }
    g.blk(
        21,
        M_WALK + 1,
        25,
        "minecraft:oak_slab",
        Some(vec![("type", "bottom")]),
    );
    g.blk(
        23,
        M_WALK + 1,
        25,
        "minecraft:oak_slab",
        Some(vec![("type", "bottom")]),
    );
    g.blk(
        22,
        M_WALK,
        25,
        "minecraft:hay_block",
        Some(vec![("axis", "y")]),
    );

    // Upper pen (empty): oak fence rectangle + gate + hay on the shelf.
    let (px0, pz0, px1, pz1) = (22, 4, 30, 9);
    for x in px0..=px1 {
        g.blk(x, M_SHELF_TOP + 1, pz0, "minecraft:oak_fence", None);
        g.blk(x, M_SHELF_TOP + 1, pz1, "minecraft:oak_fence", None);
    }
    for z in pz0..=pz1 {
        g.blk(px0, M_SHELF_TOP + 1, z, "minecraft:oak_fence", None);
        g.blk(px1, M_SHELF_TOP + 1, z, "minecraft:oak_fence", None);
    }
    // The gate stands in the pz1 rail, which runs along X, and a fence gate is
    // joinable only from the two sides its panel spans — vanilla's
    // `FenceGateBlock.connectsToDirection`, i.e. across `facing.getClockWise()`.
    // So `facing` must be north or south for the rail to reach it: facing east
    // spans Z, leaves both rail ends unjoined, and opens a permanent gap in the
    // pen with daylight on either side of the gate. Of the two, north is the
    // one vanilla places for a player who walks in off the shelf apron at
    // pz1 + 1 — the pen's only approach, since the rails close every other side.
    let gx = (px0 + px1) / 2;
    g.blk(
        gx,
        M_SHELF_TOP + 1,
        pz1,
        "minecraft:oak_fence_gate",
        Some(vec![("facing", "north"), ("open", "false")]),
    );
    g.blk(
        px0 + 1,
        M_SHELF_TOP + 1,
        pz0 + 1,
        "minecraft:hay_block",
        Some(vec![("axis", "y")]),
    );

    // Decorative Chekhov boulder on the ledge, just east of the mouth. Its mass
    // belongs in the mouth wall; where the blob reaches out over the ledge terrace
    // it must WEATHER the tread, not litter it — `distress_blk`.
    for x in 20..=22 {
        for z in 29..=30 {
            for y in M_WALK..=(M_WALK + 2) {
                if g.inb(x, y, z) && (value_noise(seed, x, y, z, 0.5, 121) > 0.25) {
                    distress_blk(
                        g,
                        x,
                        y,
                        z,
                        pick(&boulder_palette(), value_noise(seed, x, y, z, 0.5, 123)),
                    );
                }
            }
        }
    }
}

/// **Distress embeds, it never stacks** (owner playtest, island round 13: stray
/// stone sitting on the stair treads at the cave mouth).
///
/// Writes `name` at `(x, y, z)` — unless that cell is the air a body walks in on
/// top of a surface, in which case the wear is baked INTO the surface instead: the
/// block below becomes its weathered variant ([`invariants::weathered`]) keeping
/// its block state verbatim, so a stair stays the same stair in a damaged material
/// and the geometry every nav proof walked does not move. The walk envelope is left
/// clear either way — a surface with no weathered form keeps its own face and the
/// distress is dropped rather than stacked.
///
/// Unsupported cells are skipped: this pass paints wear onto rock, it never hangs a
/// rock in the air.
fn distress_blk(g: &mut Grid, x: i32, y: i32, z: i32, name: &str) {
    if !g.inb(x, y, z) {
        return;
    }
    let headroom = !g.inb(x, y + 1, z) || g.is_air(x, y + 1, z);
    if !g.is_air(x, y, z) || !headroom {
        // Inside the mass, or roofed: a normal placement, nothing rests on anything.
        g.blk(x, y, z, name, None);
        return;
    }
    if !g.is_solid(x, y - 1, z) {
        return; // nothing under it — a floating lump is debris too
    }
    let Cell::Block(surface, props) = g.get(x, y - 1, z).clone() else {
        return;
    };
    if let Some(worn) = invariants::weathered(&surface) {
        g.blk(x, y - 1, z, worn, props);
    }
}

// ===========================================================================
// Emit
// ===========================================================================

fn build(spec: &Spec) -> Grid {
    let seed = piece_seed(spec.id, spec.salt);
    let mut g = Grid::new(spec.size);
    match spec.kind {
        Kind::Greenfield => build_greenfield(spec, &mut g, seed),
        Kind::Mountain => build_mountain(spec, &mut g, seed),
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
    // Build at the ground datum (walk = 1), measure light there, then lift every
    // piece to the shared island datum (walk = 3, socket floor_y = 2) with a solid
    // substrate under the base (island-tileset.md). `yoff` tracks the emitted Ys.
    let grid0 = build(spec);
    let yoff = 2;

    let min_light = spec
        .light_region
        .map(|(a, b, c, d, ft)| estimate_min_floor_light(&grid0, a, b, c, d, ft))
        .unwrap_or(15);
    let profile: &'static str = spec.profile_override.unwrap_or_else(|| classify(min_light));

    let grid = lift_substrate(&grid0, yoff);
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
    let nbt = fastnbt::to_bytes(&structure).expect("nbt");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gz");
    let framed = gz.finish().expect("finish");
    std::fs::write(out.join(format!("{}.nbt", spec.id)), &framed).expect("write nbt");

    let mut connectors: Vec<ConnectorJson> = Vec::new();
    for &(side, fy) in &spec.doors {
        let (_, mut jc) = doorway_cells(spec.size, side, fy);
        jc[1] += yoff; // socket sits at floor_y=2 on the shared island datum
        connectors.push(ConnectorJson {
            name: SOCKET_NAME,
            target: SOCKET_NAME,
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
                    pos: v.pos.map(|p| [p[0], p[1] + yoff, p[2]]),
                    facing: v.facing.clone(),
                    region: v.region.as_ref().map(|r| RegionJson {
                        from: [r.from[0], r.from[1] + yoff, r.from[2]],
                        to: [r.to[0], r.to[1] + yoff, r.to[2]],
                    }),
                    block: v.block.clone(),
                },
            )
        })
        .collect();
    let emitted_size = [spec.size[0], spec.size[1] + yoff, spec.size[2]];

    let method = match spec.kind {
        Kind::Greenfield => {
            "open-air meadow: sky-lit (block-light estimate not applicable); floor daylight 15"
        }
        Kind::Mountain => {
            "static flood-fill block-light estimate over the cavern floor (block light only); authoring estimate, not a live probe. Sky light from the two ceiling shafts is NOT counted here — the compiler re-measures the assembled world (spec-0010) and relights declared areas at the abundant rock fixture sites"
        }
    };

    let meta = MetaJson {
        prefab_id: format!("prefab/{}", spec.id),
        structure: StructureJson {
            file: format!("{}.nbt", spec.id),
            id: spec.id.into(),
            size: emitted_size,
            data_version: DATA_VERSION,
            generator: GENERATOR.into(),
        },
        waterline_y: WATERLINE_Y,
        anchors,
        connectors,
        lighting: LightingJson {
            profile,
            measured_min_light: min_light,
            measured: MEASURED_DATE,
            rationale: spec.rationale.map(|s| s.to_string()),
            method,
        },
        license: LicenseJson {
            source: "original",
            spdx: "GPL-3.0-or-later",
            note: "Original Delvewright project asset (pipeline-code license per prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            provenance: "Generated deterministically by prefabs/island-terrain-generator (island-terrain-gen), ADR-0006; regenerating yields byte-identical NBT.",
        },
    };
    let json = serde_json::to_string_pretty(&meta).expect("json") + "\n";
    std::fs::write(out.join(format!("{}.json", spec.id)), json).expect("write json");
    println!(
        "wrote {} ({} nbt bytes, profile {}, min-light {})",
        spec.id,
        framed.len(),
        profile,
        min_light
    );
}

fn specs() -> Vec<Spec> {
    use Side::*;
    vec![
        // ---- GREENFIELD connectors (open-air meadow) ----
        Spec {
            id: "island-greenfield",
            kind: Kind::Greenfield,
            size: [17, 8, 15],
            doors: vec![(South, 0), (North, 0)],
            anchors: vec![
                ("anchor/meadow", a_pos([8, 1, 7], "north")),
                ("anchor/fold", a_pos([4, 1, 7], "south")),
            ],
            light_region: None,
            profile_override: Some("lit"),
            rationale: None,
            salt: 1,
        },
        Spec {
            id: "island-greenfield-bend",
            kind: Kind::Greenfield,
            size: [17, 8, 15],
            doors: vec![(South, 0), (East, 0)],
            anchors: vec![
                ("anchor/meadow", a_pos([8, 1, 9], "north")),
                ("anchor/fold", a_pos([4, 1, 7], "south")),
            ],
            light_region: None,
            profile_override: Some("lit"),
            rationale: None,
            salt: 2,
        },
        // ---- MOUNTAIN terminal (massif + tall-wide cavern + switchback face) ----
        Spec {
            id: "island-mountain",
            kind: Kind::Mountain,
            size: [36, 26, 42],
            doors: vec![(South, 0)],
            anchors: vec![
                ("anchor/mouth", a_pos([17, 7, 28], "south")),
                ("anchor/boulder", a_region([16, 7, 27], [18, 9, 29], "minecraft:basalt")),
                ("anchor/cheese-store", a_pos([22, 7, 26], "north")),
                ("anchor/fire-pit", a_pos([17, 7, 16], "north")),
                ("anchor/ramp-top", a_pos([25, 11, 10], "north")),
                ("anchor/pen", a_pos([26, 11, 6], "south")),
                ("anchor/alcove-1", a_pos([2, 7, 8], "east")),
                ("anchor/alcove-2", a_pos([2, 7, 19], "east")),
                ("anchor/alcove-3", a_pos([33, 7, 18], "west")),
                ("anchor/alcove-4", a_pos([10, 7, 2], "south")),
                ("anchor/checkpoint-1", a_pos([17, 7, 25], "north")),
                ("anchor/checkpoint-2", a_pos([17, 7, 12], "north")),
                ("anchor/checkpoint-3", a_pos([22, 7, 12], "east")),
                ("anchor/shaft-1", a_pos([10, 7, 10], "north")),
                ("anchor/shaft-2", a_pos([24, 7, 18], "north")),
            ],
            // cavern floor light region: x 3..32, z 3..26, floor top 6
            light_region: Some((M_CAV_X0, M_CAV_X1, M_CAV_Z0, M_CAV_Z1, M_FLOOR_TOP)),
            profile_override: None,
            rationale: Some(
                "the Cyclops' hall — firelit at the central pit, dark at the vault edges and in the four shadow alcoves BY DESIGN (the stealth beats). Fixture sites (rock walls/ceiling within relight radius of every cell) and two sky-open ceiling shafts exist; the campaign relights minimally (spec-0010) or grants night-vision, keeping the hall dark.",
            ),
            salt: 3,
        },
    ]
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: island-terrain-gen <out_dir>");
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("mkdir");
    let specs = specs();
    for spec in &specs {
        write_piece(out, spec);
    }
    println!("{} pieces", specs.len());
}
