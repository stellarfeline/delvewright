//! Deterministic generator for the Delvewright "Mediterranean cave/shore" prefab
//! tileset (the prefab-ceiling probe: can self-created prefabs reach showcase
//! quality via a render-critique loop?).
//!
//! A SIBLING of `prefabs/generator` (the stone-keep gen): it shares nothing with
//! it and never touches its output, so keep `.nbt` stays byte-identical
//! (ADR-0006). It emits the same artifact shape — a gzip-framed vanilla structure
//! `.nbt` + a metadata `.json` per piece — but the pieces are irregular natural
//! rock, not a uniform brick shell.
//!
//! Three deterministic design layers, all seeded from a per-piece PRNG + value
//! noise (no wall clock, no unseeded RNG, no hash-order iteration):
//!   1. PALETTE RECIPES  — weighted multi-block per surface role, sampled through
//!      a spatially-coherent value-noise field so blocks cluster into patches
//!      (real rock strata / moss) instead of salt-and-pepper "tiled noise".
//!   2. MODULE GRAMMAR   — irregular wall lining + niches, hearth, timber sheep
//!      pens, boulder-sealed cave mouth.
//!   3. DETAILING / AGING — hanging dripstone stalactites, floor rubble mounds,
//!      glow-lichen / vine patches, and (shore) a sand→water gradient with rock
//!      scatter + driftwood and waterline staining.
//!
//! Connection convention reuses keep-socket geometry (3×3 opening, one jigsaw
//! block at the bottom-centre wall cell) under the `cave:socket` vocabulary; the
//! compiler's solver reads socket geometry only (names are a vocabulary), so the
//! cave pool is a structural drop-in for `pool/stone-keep`.
//!
//! Lighting is DERIVED, not asserted: a static flood-fill block-light estimate
//! over walkable floor cells sets `measured_min_light`, and the profile
//! (`lit` ≥7 / `dim` 3–6) is classified from it — firelight pockets are declared
//! as `dim`, honestly, not hidden.
//!
//! Usage: cave-prefab-gen <out_dir>

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

/// Cross-tileset generator invariants, shared by source include so a lesson
/// learned in one tileset does not have to be re-learned in the other four
/// (the five generators are separate Cargo workspaces on purpose).
#[path = "../../invariants.rs"]
mod invariants;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

const DATA_VERSION: i32 = 4671; // MC 1.21.11
const GENERATOR: &str = "prefabs/cave-generator (cave-prefab-gen)";
const MEASURED_DATE: &str = "2026-07-31";

// ---------------------------------------------------------------------------
// Deterministic hashing / value noise (ADR-0006)
// ---------------------------------------------------------------------------

/// splitmix64 finalizer over a 64-bit lane — the one mixing primitive.
fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fold a piece id string into a 64-bit seed (FNV-1a then one mix step) so each
/// piece has an independent, reproducible randomness stream.
fn piece_seed(id: &str, salt: u64) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    mix64(h ^ salt)
}

/// Hash a 3D integer lattice point (+ salt) to a value in [0,1).
fn hash01(seed: u64, x: i32, y: i32, z: i32, salt: u64) -> f64 {
    let mut h = seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = mix64(h ^ (x as i64 as u64).wrapping_mul(0x0000_0100_0000_01B3));
    h = mix64(h ^ (y as i64 as u64).wrapping_mul(0xFF51_AFD7_ED55_8CCD));
    h = mix64(h ^ (z as i64 as u64).wrapping_mul(0xC4CE_B9FE_1A85_EC53));
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn fade(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t) // smoothstep
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Trilinearly-interpolated value noise in [0,1] at world (x,y,z) scaled by
/// `freq` (cells per lattice cell ≈ 1/freq). Smooth → blocks of similar palette
/// cluster into patches instead of per-cell speckle.
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
// Palette recipes (design layer 1)
// ---------------------------------------------------------------------------

type Props = Option<Vec<(&'static str, &'static str)>>;

/// A weighted palette entry: block id, optional block-state props, weight.
struct Recipe {
    name: &'static str,
    props: Props,
    weight: f64,
}
fn r(name: &'static str, weight: f64) -> Recipe {
    Recipe {
        name,
        props: None,
        weight,
    }
}

/// Cave wall rock: cobble-dominant with andesite/tuff/stone bands and sparse
/// moss + cracked-brick (old bronze-age masonry showing through the rock).
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
/// Ceiling rock: darker/greyer, a little dripstone.
fn ceiling_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:stone", 0.28),
        r("minecraft:andesite", 0.26),
        r("minecraft:tuff", 0.22),
        r("minecraft:cobblestone", 0.16),
        r("minecraft:dripstone_block", 0.08),
    ]
}
/// Cave floor: sand→gravel→stone gradient with cobble and coarse dirt.
fn floor_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:gravel", 0.30),
        r("minecraft:cobblestone", 0.22),
        r("minecraft:stone", 0.18),
        r("minecraft:sand", 0.14),
        r("minecraft:andesite", 0.08),
        r("minecraft:coarse_dirt", 0.08),
    ]
}
/// Boulder mass (sealed cave mouth): heavy mossy cobble.
fn boulder_palette() -> Vec<Recipe> {
    vec![
        r("minecraft:cobblestone", 0.40),
        r("minecraft:mossy_cobblestone", 0.30),
        r("minecraft:andesite", 0.18),
        r("minecraft:tuff", 0.12),
    ]
}

/// Pick a block from a weighted palette using a noise value in [0,1]; the
/// cumulative-weight partition means a smooth noise field yields spatial patches.
fn pick(palette: &[Recipe], n: f64) -> (&'static str, Props) {
    let total: f64 = palette.iter().map(|e| e.weight).sum();
    let mut acc = 0.0;
    let target = n.clamp(0.0, 0.999_999) * total;
    for e in palette {
        acc += e.weight;
        if target < acc {
            return (e.name, e.props.clone());
        }
    }
    let last = palette.last().unwrap();
    (last.name, last.props.clone())
}

/// Edge-distance / height weathering bias (algorithm A1, reimplemented from the
/// GDMC extraction dossier's description — see ACKNOWLEDGEMENTS; no upstream code
/// used). Our surface palettes are authored clean→weathered (mossy/cracked/coarse
/// last), so ADDING a positive bias to the noise sample before `pick` concentrates
/// the weathered variants where wear and water collect: low (near the floor) and
/// near the piece walls. Deterministic — a pure function of position + bounds.
fn weathering_bias(x: i32, y: i32, z: i32, size: [i32; 3], t: i32) -> f64 {
    let [sx, sy, sz] = size;
    // Height ramp: strongest at the floor, fading out by mid-height.
    let h = ((sy - 1) as f64).max(1.0);
    let floor_ramp = (1.0 - (y as f64 / h)).clamp(0.0, 1.0);
    // Edge ramp: strongest against the walls (min horizontal distance to the
    // interior edge), fading one cell in.
    let dx = (x - t).min(sx - 1 - t - x);
    let dz = (z - t).min(sz - 1 - t - z);
    let edge = (dx.min(dz)).max(0);
    let edge_ramp = match edge {
        0 => 1.0,
        1 => 0.45,
        _ => 0.0,
    };
    0.30 * floor_ramp + 0.22 * edge_ramp
}

// ---------------------------------------------------------------------------
// Cell grid
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Cell {
    Air,
    Block(String, Props),
    Jigsaw(&'static str), // orientation token
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
    fn blk(&mut self, x: i32, y: i32, z: i32, name: &'static str, props: Props) {
        self.set(x, y, z, Cell::Block(name.to_string(), props));
    }
}

// ---------------------------------------------------------------------------
// NBT serialization (structure format)
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
                                name: "cave:socket".into(),
                                target: "cave:socket".into(),
                                pool: "cave:pool".into(),
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
// Sockets (keep-socket geometry, cave vocabulary)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
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

/// Jigsaw (wall) cell of a doorway on `side` whose floor sits at `floor_y`.
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
/// The 3×3 opening cells + the jigsaw cell for a doorway.
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
fn a_region(from: [i32; 3], to: [i32; 3], block: &str) -> AnchorJson {
    AnchorJson {
        pos: None,
        facing: None,
        region: Some(RegionJson { from, to }),
        block: Some(block.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Light estimation (derived lighting declaration)
// ---------------------------------------------------------------------------

/// Block-light emission of a light-source id (0 if not a source). 1.21.11 values.
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
/// Whether a cell lets block light pass (for the flood estimate). Full opaque
/// rock blocks light; air, water, and small non-full blocks pass it.
fn transparent(cell: &Cell) -> bool {
    match cell {
        Cell::Air | Cell::Jigsaw(_) => true,
        Cell::Block(name, _) => matches!(
            name.as_str(),
            "minecraft:air"
                | "minecraft:water"
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
                | "minecraft:seagrass"
                | "minecraft:dead_bush"
        ),
    }
}

/// Static flood-fill block-light estimate: BFS from every source through
/// transparent cells, light −1 per step, take the max at each cell. Returns the
/// minimum over walkable floor cells (y=1 air with clearance and solid below).
/// NOT a live server probe — a conservative authoring estimate.
fn estimate_min_floor_light(grid: &Grid) -> i32 {
    let [sx, sy, sz] = grid.size;
    let n = (sx * sy * sz) as usize;
    let mut light = vec![0i32; n];
    let mut queue: std::collections::VecDeque<(i32, i32, i32, i32)> =
        std::collections::VecDeque::new();
    // Seed sources (fixed x,y,z order → deterministic).
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
    // Minimum over walkable *interior* floor cells. Cells on the piece boundary
    // (x/z at 0 or max) are doorway-mouth thresholds — the shell is solid wall
    // there except where a socket is carved — so a walkable boundary cell is
    // always an open doorway, which mates against the (lit) neighbouring piece at
    // assembly and is dark only in isolation. Measuring the interior is the honest
    // "does the room read as lit?" question; it can only raise the min, never mask
    // a genuinely dark interior.
    let mut min = i32::MAX;
    for x in 0..sx {
        for z in 0..sz {
            if x == 0 || x == sx - 1 || z == 0 || z == sz - 1 {
                continue;
            }
            let floor_solid = matches!(grid.get(x, 0, z), Cell::Block(_, _));
            let stand = matches!(grid.get(x, 1, z), Cell::Air);
            let head = sy > 2 && matches!(grid.get(x, 2, z), Cell::Air);
            if floor_solid && stand && head {
                min = min.min(light[grid.idx(x, 1, z)]);
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
// Piece specs & construction
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Module {
    /// Hearth: stone ring + lit campfire + firewood, centred at (x,z).
    Hearth(i32, i32),
    /// Timber sheep pen: fenced rectangle (inclusive x,z corners) with a gate on
    /// its south side and hay bales inside.
    Pen(i32, i32, i32, i32),
    /// Boulder-sealed cave mouth filling the doorway region on `side`.
    Boulder(Side),
}

struct Spec {
    id: &'static str,
    size: [i32; 3],
    /// (side, floor_y) doorways. floor_y>0 makes a raised (stair) socket.
    doors: Vec<(Side, i32)>,
    /// Wall thickness (1 corridor, 2 room — thicker walls host niches).
    wall_thickness: i32,
    open_air: bool,
    /// Hanging-lantern lattice (ceiling grid) for lit pieces; empty → rely on
    /// module firelight (dim).
    lantern_grid: bool,
    modules: Vec<Module>,
    /// Stair run: south low door → north high door climb (like keep-stair).
    stair: bool,
    anchors: Vec<(&'static str, AnchorJson)>,
    rationale: Option<&'static str>,
    salt: u64,
}

/// Stair top-of-solid height at interior z (mirror keep-stair: +4 rise over the
/// run, low door south at z=max).
fn stair_surface(z: i32) -> i32 {
    match z {
        9 => 0,
        8 => 1,
        7 => 2,
        6 => 3,
        _ => 4,
    }
}

fn build(spec: &Spec) -> Grid {
    let [sx, sy, sz] = spec.size;
    let seed = piece_seed(spec.id, spec.salt);
    let mut g = Grid::new(spec.size);

    if spec.stair {
        build_stair(spec, &mut g, seed);
        return g;
    }
    if spec.open_air {
        build_shore(spec, &mut g, seed);
        apply_anchoring_air(&mut g, spec);
        return g;
    }

    let t = spec.wall_thickness;
    // 1. Shell with palette recipes + irregular inner lining.
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let on_x = x < t || x >= sx - t;
                let on_z = z < t || z >= sz - t;
                let floor = y == 0;
                let ceil = y == sy - 1;
                if floor {
                    let n = value_noise(seed, x, y, z, 0.16, 11)
                        + 0.5 * weathering_bias(x, y, z, spec.size, t);
                    let (b, p) = pick(&floor_palette(), n);
                    g.blk(x, y, z, b, p);
                } else if ceil {
                    let n = value_noise(seed, x, y, z, 0.18, 23);
                    let (b, p) = pick(&ceiling_palette(), n);
                    g.blk(x, y, z, b, p);
                } else if on_x || on_z {
                    let n = value_noise(seed, x, y, z, 0.17, 31)
                        + weathering_bias(x, y, z, spec.size, t);
                    let (b, p) = pick(&wall_palette(), n);
                    g.blk(x, y, z, b, p);
                }
            }
        }
    }
    // 1b. Organic shaping (order matters — each pass reads the previous state):
    //   ca_inner_shape  carves alcoves / bumps the inner wall face (A8 CA),
    //   vault_ceiling   domes the roof inward one cell along the wall,
    //   roughen_silhouette erodes the outer crown + vertical faces (A5).
    // All three skip doorway columns so the sockets stay byte-identical, and none
    // touch y=0 (floor) or y=1 (walk height), so pathability + enclosure hold.
    ca_inner_shape(spec, &mut g, seed);
    vault_ceiling(spec, &mut g, seed);
    roughen_silhouette(spec, &mut g, seed);

    // 2. Carve doorways + jigsaw sockets through the full wall thickness.
    for &(side, fy) in &spec.doors {
        let (cells, jc) = doorway_cells(spec.size, side, fy);
        for depth in 0..t {
            for c in &cells {
                let cc = inset(*c, side, depth);
                g.set(cc[0], cc[1], cc[2], Cell::Air);
            }
        }
        g.set(jc[0], jc[1], jc[2], Cell::Jigsaw(side.orientation()));
    }

    // 3. Detailing pass: stalactites, floor rubble, lichen, vines.
    detail_pass(spec, &mut g, seed);

    // 4. Lighting: hanging lantern lattice for lit pieces. Large halls use a
    // coarser lattice (spacing 8) so they read as a firelit cavern, dimmer at the
    // vault edges, rather than an evenly-floodlit warehouse.
    if spec.lantern_grid {
        let spacing = if (sx - 2 * t).min(sz - 2 * t) > 8 {
            8
        } else {
            6
        };
        for (lx, lz) in lantern_points(spec.size, t, spacing) {
            // lantern hangs from the ceiling block just above it
            g.blk(
                lx,
                sy - 2,
                lz,
                "minecraft:lantern",
                Some(vec![("hanging", "true")]),
            );
        }
    }

    // 5. Modules.
    for m in &spec.modules {
        apply_module(&mut g, m, seed);
    }

    g
}

/// Inset a doorway cell `depth` blocks inward from `side` (to carry the opening
/// through a thick wall).
fn inset(c: [i32; 3], side: Side, depth: i32) -> [i32; 3] {
    match side {
        Side::North => [c[0], c[1], c[2] + depth],
        Side::South => [c[0], c[1], c[2] - depth],
        Side::West => [c[0] + depth, c[1], c[2]],
        Side::East => [c[0] - depth, c[1], c[2]],
    }
}

/// True if column (x,z) lies within a doorway's 3-wide opening (plus a one-cell
/// margin around the socket frame) for any door. The organic passes skip these
/// columns so each doorway's opening, jigsaw seat, and surrounding frame stay
/// byte-identical for the solver — sockets are keep-socket-v1, unchanged.
fn near_doorway(size: [i32; 3], doors: &[(Side, i32)], x: i32, z: i32) -> bool {
    for &(side, fy) in doors {
        let jc = door_center(size, side, fy);
        let hit = match side {
            Side::North | Side::South => (x - jc[0]).abs() <= 2 && (z - jc[2]).abs() <= 1,
            Side::West | Side::East => (z - jc[2]).abs() <= 2 && (x - jc[0]).abs() <= 1,
        };
        if hit {
            return true;
        }
    }
    false
}

/// Cellular-automata (4-5 rule) organic shaping of the inner wall face
/// (algorithm A8 — the classic public "cave-like levels" CA technique, our own
/// implementation from the description; no code ingested). A per-column field is
/// seeded ~45% rock inside the interior, the boundary pinned rock, then the 4-5
/// smoothing rule is iterated to blobby caverns. It is read back only on the ring
/// against the inner wall: "open" columns carve an alcove into the wall (widening
/// it, enclosure preserved because the outer boundary stays solid); "rock" columns
/// bump rock inward above walk height. Deterministic — one seeded value field,
/// double-buffered fixed scan order (ADR-0006). Skips doorway columns and never
/// touches y=0/y=1, so sockets, floor, and the walkable path are untouched.
fn ca_inner_shape(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = spec.size;
    let t = spec.wall_thickness;
    if t < 2 {
        return;
    }
    let interior = (sx - 2 * t).min(sz - 2 * t);
    let d = sz as usize;
    let at = |x: i32, z: i32| (x as usize) * d + (z as usize);
    let inside = |x: i32, z: i32| x >= t && x < sx - t && z >= t && z < sz - t;
    // Seed.
    let mut cur = vec![false; (sx * sz) as usize];
    for x in 0..sx {
        for z in 0..sz {
            cur[at(x, z)] = if inside(x, z) {
                value_noise(seed, x, 0, z, 0.55, 221) < 0.45
            } else {
                true
            };
        }
    }
    // Iterate the 4-5 rule (double-buffered, fixed order).
    for _ in 0..4 {
        let mut next = cur.clone();
        for x in 0..sx {
            for z in 0..sz {
                if !inside(x, z) {
                    next[at(x, z)] = true;
                    continue;
                }
                let mut walls = 0;
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let (nx, nz) = (x + dx, z + dz);
                        if nx < 0 || nz < 0 || nx >= sx || nz >= sz || cur[at(nx, nz)] {
                            walls += 1;
                        }
                    }
                }
                next[at(x, z)] = walls >= 5;
            }
        }
        cur = next;
    }
    // Read back on the inner ring only.
    for x in t..sx - t {
        for z in t..sz - t {
            let ring = x == t || x == sx - 1 - t || z == t || z == sz - 1 - t;
            if !ring || near_doorway(spec.size, &spec.doors, x, z) {
                continue;
            }
            if cur[at(x, z)] {
                // Bump rock inward above walk height (only when the room can spare
                // the width, so small rooms are not choked).
                if interior >= 5 {
                    for y in 2..sy - 1 {
                        if value_noise(seed, x, y, z, 0.30, 231) > 0.55 {
                            let n = value_noise(seed, x, y, z, 0.17, 31)
                                + weathering_bias(x, y, z, spec.size, t);
                            let (b, p) = pick(&wall_palette(), n);
                            g.blk(x, y, z, b, p);
                        }
                    }
                }
            } else {
                // Alcove: carve the inner wall layer behind this cell to air.
                let (wx, wz) = if x == t {
                    (t - 1, z)
                } else if x == sx - 1 - t {
                    (sx - t, z)
                } else if z == t {
                    (x, t - 1)
                } else {
                    (x, sz - t)
                };
                for y in 2..sy - 1 {
                    if value_noise(seed, x, y, z, 0.30, 241) > 0.55 {
                        g.set(wx, y, wz, Cell::Air);
                    }
                }
            }
        }
    }
}

/// Dome the interior ceiling: hang rock down one cell along the wall-adjacent ring
/// so the roof reads as an arched cavern instead of a flat lid. Skips doorway
/// columns; never touches the two lowest air layers, so head clearance holds.
fn vault_ceiling(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = spec.size;
    let t = spec.wall_thickness;
    if sy < 5 {
        return;
    }
    for x in t..sx - t {
        for z in t..sz - t {
            let ex = (x - t).min(sx - 1 - t - x);
            let ez = (z - t).min(sz - 1 - t - z);
            if ex.min(ez) > 0 || near_doorway(spec.size, &spec.doors, x, z) {
                continue;
            }
            if value_noise(seed, x, sy - 2, z, 0.40, 251) > 0.5
                && matches!(g.get(x, sy - 2, z), Cell::Air)
            {
                let n = value_noise(seed, x, sy - 2, z, 0.18, 23);
                let (b, p) = pick(&ceiling_palette(), n);
                g.blk(x, sy - 2, z, b, p);
            }
        }
    }
}

/// Silhouette / edge roughening (algorithm A5 — reimplemented from the dossier's
/// "stochastic thickening/erosion of an edge" description; no code ingested).
/// Breaks the rectangular outline that made round-1 pieces read as boxes, WITHOUT
/// moving the AABB or the sockets: it only removes shell rock, never adds beyond
/// the bounds. Two effects, both deterministic and enclosure-safe. First, a
/// ragged eroded crown along the roofline: nibble perimeter top cells only where
/// the cell below is solid (the interior ceiling, whose support is air, is never
/// opened), eroding deeper at the vertical corners to chamfer them. Second, divots
/// in the outer vertical faces of thick pieces: remove an outer-layer cell only
/// when the cell one step inward is still solid, so an inner wall layer always
/// remains and nothing breaches to the void. Doorway columns are skipped so socket
/// frames stay byte-identical.
fn roughen_silhouette(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = spec.size;
    let t = spec.wall_thickness;
    let perim = |x: i32, z: i32| x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
    let solid = |g: &Grid, x: i32, y: i32, z: i32| matches!(g.get(x, y, z), Cell::Block(_, _));

    // 1. Ragged eroded crown.
    for x in 0..sx {
        for z in 0..sz {
            if !perim(x, z) || near_doorway(spec.size, &spec.doors, x, z) {
                continue;
            }
            let at_corner = x.min(sx - 1 - x) == 0 && z.min(sz - 1 - z) == 0;
            let max_depth = if t >= 2 {
                if at_corner {
                    3
                } else {
                    2
                }
            } else if at_corner {
                2
            } else {
                1
            };
            let n = value_noise(seed, x, 0, z, 0.5, 201);
            let mut depth = (n * (max_depth as f64 + 0.4)).floor() as i32;
            if at_corner {
                depth += 1;
            }
            depth = depth.min(max_depth);
            let mut removed = 0;
            let mut y = sy - 1;
            while removed < depth && y >= 2 {
                if solid(g, x, y, z) && solid(g, x, y - 1, z) {
                    g.set(x, y, z, Cell::Air);
                    removed += 1;
                    y -= 1;
                } else {
                    break;
                }
            }
        }
    }

    // 2. Outer-face divots (thick pieces only — an inner wall layer always remains).
    if t >= 2 {
        for x in 0..sx {
            for y in 2..sy - 1 {
                for z in 0..sz {
                    if !perim(x, z) || near_doorway(spec.size, &spec.doors, x, z) {
                        continue;
                    }
                    if !solid(g, x, y, z) {
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
                    if solid(g, x + ix, y, z + iz) && value_noise(seed, x, y, z, 0.45, 211) > 0.80 {
                        g.set(x, y, z, Cell::Air);
                    }
                }
            }
        }
    }
}

/// Ceiling lantern lattice points. A tight interior (min inner span ≤ 4:
/// corridors, small rooms) gets a single central lantern — a grid there just
/// clutters the space. Larger rooms get a lattice spaced ≤ `spacing` (≤6 → floor
/// light ≥7; a coarser spacing leaves an honestly dimmer hall).
fn lantern_points(size: [i32; 3], t: i32, spacing: i32) -> Vec<(i32, i32)> {
    let [sx, _, sz] = size;
    let inner_x = sx - 2 * t;
    let inner_z = sz - 2 * t;
    if inner_x.min(inner_z) <= 4 {
        return vec![(sx / 2, sz / 2)];
    }
    let axis = |n: i32| -> Vec<i32> {
        let lo = t;
        let hi = n - 1 - t;
        let span = hi - lo;
        let segs = (span / spacing).max(0) + 1;
        let mut v = vec![];
        for k in 0..=segs {
            v.push(lo + span * k / segs);
        }
        v.dedup();
        v
    };
    let mut pts = vec![];
    for x in axis(sx) {
        for z in axis(sz) {
            pts.push((x, z));
        }
    }
    pts
}

/// Design layer 3: hanging dripstone, floor rubble mounds, glow-lichen and vine
/// patches — the aging / detailing pass. All noise-gated, none on the central
/// walkable spine so pieces stay pathable.
fn detail_pass(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = spec.size;
    let t = spec.wall_thickness;
    for x in t..sx - t {
        for z in t..sz - t {
            // hanging stalactite from the ceiling into the upper air
            let n = value_noise(seed, x, sy - 2, z, 0.34, 51);
            if n > 0.72 && matches!(g.get(x, sy - 2, z), Cell::Air) {
                g.blk(
                    x,
                    sy - 2,
                    z,
                    "minecraft:pointed_dripstone",
                    Some(vec![
                        ("vertical_direction", "down"),
                        ("thickness", if n > 0.86 { "middle" } else { "tip" }),
                        ("waterlogged", "false"),
                    ]),
                );
                if n > 0.86 && sy >= 6 && matches!(g.get(x, sy - 3, z), Cell::Air) {
                    g.blk(
                        x,
                        sy - 3,
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
            // floor rubble mound (edge band only, keep the centre clear)
            let edge = x < t + 1 || x >= sx - t - 1 || z < t + 1 || z >= sz - t - 1;
            if edge {
                let m = value_noise(seed, x, 1, z, 0.40, 61);
                if m > 0.74 && matches!(g.get(x, 1, z), Cell::Air) {
                    let (b, p) = pick(&floor_palette(), value_noise(seed, x, 1, z, 0.5, 63));
                    g.blk(x, 1, z, b, p);
                }
            }
        }
    }
    // glow-lichen + vines on interior wall faces (atmosphere; lichen emits 7).
    // Start at y=3 so no greeble ever lands in the WALK ENVELOPE (feet y=1, head
    // y=2 for a 2-tall entity standing on the y=0 floor): glow_lichen has no
    // collision box in-game, but a plant curtain drawn across a narrow doorway
    // throat still reads to a block-occupancy pathfinder (and to a mineflayer bot)
    // as a wall, which is exactly what wedged the round-2 seams shut. Decoration
    // hugs the upper wall / ceiling line instead, where it can never obstruct a
    // walker — the walk tube through every socket stays clear rock-and-air.
    for x in t..sx - t {
        for y in 3..sy - 1 {
            for z in t..sz - t {
                if !matches!(g.get(x, y, z), Cell::Air) {
                    continue;
                }
                // find an adjacent solid wall face
                for (dx, dz, face) in [
                    (-1, 0, "east"),
                    (1, 0, "west"),
                    (0, -1, "south"),
                    (0, 1, "north"),
                ] {
                    if matches!(g.get(x + dx, y, z + dz), Cell::Block(_, _)) {
                        let ln = value_noise(seed, x, y, z, 0.5, 71);
                        if ln > 0.88 {
                            g.blk(x, y, z, "minecraft:glow_lichen", Some(vec![(face, "true")]));
                        } else if ln < 0.05 && y >= sy - 2 {
                            g.blk(x, y, z, "minecraft:vine", Some(vec![(face, "true")]));
                        }
                        break;
                    }
                }
            }
        }
    }
}

fn apply_module(g: &mut Grid, m: &Module, seed: u64) {
    match m {
        Module::Hearth(cx, cz) => {
            // stone ring around the fire
            for dx in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let n = value_noise(seed, cx + dx, 1, cz + dz, 0.5, 81);
                    let (b, p) = pick(&boulder_palette(), n);
                    g.blk(cx + dx, 1, cz + dz, b, p);
                }
            }
            g.blk(
                *cx,
                1,
                *cz,
                "minecraft:campfire",
                Some(vec![("lit", "true"), ("facing", "north")]),
            );
            // firewood leaning nearby
            g.blk(
                cx - 1,
                1,
                cz + 1,
                "minecraft:oak_log",
                Some(vec![("axis", "x")]),
            );
        }
        Module::Pen(x0, z0, x1, z1) => {
            // fence perimeter
            for x in *x0..=*x1 {
                g.blk(x, 1, *z0, "minecraft:oak_fence", None);
                g.blk(x, 1, *z1, "minecraft:oak_fence", None);
            }
            for z in *z0..=*z1 {
                g.blk(*x0, 1, z, "minecraft:oak_fence", None);
                g.blk(*x1, 1, z, "minecraft:oak_fence", None);
            }
            // gate on the south edge, centre
            let gx = (x0 + x1) / 2;
            g.blk(
                gx,
                1,
                *z1,
                "minecraft:oak_fence_gate",
                Some(vec![("facing", "east"), ("open", "false")]),
            );
            // hay bales inside
            g.blk(
                x0 + 1,
                1,
                z0 + 1,
                "minecraft:hay_block",
                Some(vec![("axis", "y")]),
            );
            if x1 - x0 > 2 {
                g.blk(
                    x1 - 1,
                    1,
                    z1 - 1,
                    "minecraft:hay_block",
                    Some(vec![("axis", "z")]),
                );
            }
        }
        Module::Boulder(side) => {
            let size = g.size;
            let (cells, _) = doorway_cells(size, *side, 0);
            for c in &cells {
                let n = value_noise(seed, c[0], c[1], c[2], 0.4, 91);
                let (b, p) = pick(&boulder_palette(), n);
                g.blk(c[0], c[1], c[2], b, p);
            }
            // a mossy rim around the boulder
            let jc = door_center(size, *side, 0);
            g.blk(jc[0], jc[1], jc[2], "minecraft:mossy_cobblestone", None);
        }
    }
}

/// Anchor-free geometry helper for open-air pieces (no-op hook kept for symmetry).
fn apply_anchoring_air(_g: &mut Grid, _spec: &Spec) {}

/// The stair piece: mirror keep-stair's climbing run but in cave rock, with
/// wall-mounted lanterns along the ascent.
fn build_stair(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = spec.size;
    // shell
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = x == 0 || x == sx - 1 || z == 0 || z == sz - 1 || y == 0 || y == sy - 1;
                if shell {
                    let pal = if y == 0 {
                        floor_palette()
                    } else if y == sy - 1 {
                        ceiling_palette()
                    } else {
                        wall_palette()
                    };
                    let n = value_noise(seed, x, y, z, 0.17, 31);
                    let (b, p) = pick(&pal, n);
                    g.blk(x, y, z, b, p);
                } else {
                    g.set(x, y, z, Cell::Air);
                }
            }
        }
    }
    // climbing run (3 wide), cobblestone_stairs facing north, solid fill beneath
    for z in 1..=9 {
        let top = stair_surface(z);
        for x in 1..=3 {
            for y in 1..top {
                g.blk(x, y, z, "minecraft:cobblestone", None);
            }
            if top >= 1 {
                if (5..=8).contains(&z) {
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
                } else {
                    let n = value_noise(seed, x, top, z, 0.16, 11);
                    let (b, p) = pick(&floor_palette(), n);
                    g.blk(x, top, z, b, p);
                }
            }
        }
    }
    // Roughen the exterior crown (A5) so the stair shaft is not a clean box either
    // — t=1 here, so only the ragged roofline + corner chamfer apply. Runs before
    // doorway carving so the sockets stay byte-identical.
    roughen_silhouette(spec, g, seed);
    // doorways: low south (floor 0), high north (floor 4)
    for &(side, fy) in &spec.doors {
        let (cells, jc) = doorway_cells(spec.size, side, fy);
        for c in &cells {
            g.set(c[0], c[1], c[2], Cell::Air);
        }
        g.set(jc[0], jc[1], jc[2], Cell::Jigsaw(side.orientation()));
    }
    // wall lanterns at head height along the run
    for &z in &[2, 4, 6, 8] {
        let s = stair_surface(z);
        for &x in &[0, 4] {
            g.blk(
                x,
                s + 2,
                z,
                "minecraft:lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }
}

/// The open-air shore terminal: a rock cove open to sky and sea. Sand→water
/// gradient front-to-back, rock scatter, driftwood, glow-lichen on the cliff,
/// and one inland socket (north cliff) into the cave.
fn build_shore(spec: &Spec, g: &mut Grid, seed: u64) {
    let [sx, sy, sz] = spec.size;
    // base stone under everything
    for x in 0..sx {
        for z in 0..sz {
            g.blk(x, 0, z, "minecraft:stone", None);
        }
    }
    // Enclosing cliffs on ALL FOUR sides, each rising the full piece height, so the
    // cove is a bounded sky-lit lagoon (箱庭 box-garden): open upward to the sky,
    // walled against the void on every horizontal edge. Round-1/round-2 tapered the
    // side cliffs down toward the sea (`sy-1 - z/2`) and left the front (z=sz-1)
    // fully open. Packed against the enclosed cave pieces in the assembly, both
    // leaks let a wandering entity escape to the void: the tapered sides were a
    // climbable stair (≤1 block/step) up onto the piece roofscape and off an eroded
    // roof edge, and the open front let it walk/swim straight off the world's edge —
    // the sheep the bot watched fall to y=-163. Full-height walls seal every edge.
    // The graded shoreline, seabed depth profile, water, and scatter INSIDE the
    // basin are unchanged, so the round-2 soft-shore art is preserved; only the
    // escape routes are closed.
    for y in 1..sy {
        for x in 0..sx {
            for z in 0..sz {
                let cliff = z == 0 || z == sz - 1 || x == 0 || x == sx - 1;
                let height = sy - 1;
                if cliff && y <= height {
                    let n = value_noise(seed, x, y, z, 0.17, 31);
                    let (b, p) = pick(&wall_palette(), n);
                    g.blk(x, y, z, b, p);
                }
            }
        }
    }
    // Surface: a GRADED shoreline that laps in from the open front (south), not a
    // rectangular pool with a hard vertical water wall (the round-1 defect). The
    // tide line sits near the front and is ragged (per-column jitter → cove inlets)
    // so the coast is uneven; the spawn strip (x 4..8) is pinned dry so the player
    // lands on the beach at the water's edge, not waist-deep. The softening is in
    // the SEABED, not the water column: near shore the bed stays at sand (1-deep
    // shallow), then the bed STEPS DOWN to stone as it deepens (2-deep sea) while
    // the water surface stays flat at y=2. Sand → wet-gravel tide row → sand-bottom
    // shallow → stone-bottom sea, with pebbles and seagrass across the transition.
    let surf = 2; // flat calm-sea surface height (top water block at y=surf)
    let base_wl = sz - 2;
    let coast_at = |x: i32| -> i32 {
        let j = (value_noise(seed, x, 0, 0, 0.35, 131) * 4.0).floor() as i32 - 2; // −2..+1
        let mut c = (base_wl + j).clamp(sz - 4, sz - 1);
        if (4..=8).contains(&x) {
            c = c.max(sz - 2); // keep the spawn beach dry
        }
        c
    };
    for x in 1..sx - 1 {
        let coast = coast_at(x);
        for z in 1..sz {
            if z < coast - 1 {
                // dry beach: sand with clustered gravel patches, drier toward back.
                let n = value_noise(seed, x, 1, z, 0.24, 101);
                let dryness = z as f64 / coast as f64;
                let b = if n + dryness > 1.15 {
                    "minecraft:gravel"
                } else {
                    "minecraft:sand"
                };
                g.blk(x, 1, z, b, None);
            } else if z == coast - 1 {
                // wet tide row: darker damp gravel where the water just reaches.
                g.blk(x, 1, z, "minecraft:gravel", None);
            } else {
                // sea. Bed drops the further out we go so depth grows from the FLOOR
                // (natural beach slope), not from stacking water over a flat bed.
                let out = z - coast; // 0 at the shore
                if out <= 1 {
                    // shallow: sand bottom visible through 1 block of water; scatter
                    // wet pebbles so the bottom is not a clean plane.
                    let bed = if value_noise(seed, x, 1, z, 0.5, 107) > 0.78 {
                        "minecraft:gravel"
                    } else {
                        "minecraft:sand"
                    };
                    g.blk(x, 1, z, bed, None);
                    g.blk(x, surf, z, "minecraft:water", Some(vec![("level", "0")]));
                } else {
                    // deeper sea: bed steps down to stone, water fills 2 deep to the
                    // flat surface.
                    g.blk(x, 0, z, "minecraft:stone", None);
                    for wy in 1..=surf {
                        g.blk(x, wy, z, "minecraft:water", Some(vec![("level", "0")]));
                    }
                }
                // seagrass tufts across the shallow band.
                if out <= 2 && value_noise(seed, x, 2, z, 0.5, 105) > 0.82 {
                    g.blk(x, 2, z, "minecraft:seagrass", None);
                }
            }
        }
    }
    // Rock scatter (boulders) on the dry beach, noise-clustered.
    for x in 2..sx - 2 {
        for z in 2..base_wl {
            if z >= coast_at(x) - 1 {
                continue;
            }
            let n = value_noise(seed, x, 2, z, 0.5, 111);
            if n > 0.9 {
                let (b, p) = pick(&boulder_palette(), value_noise(seed, x, 2, z, 0.6, 113));
                g.blk(x, 2, z, b, p);
                if n > 0.96 {
                    let (b2, p2) = pick(&boulder_palette(), value_noise(seed, x, 3, z, 0.6, 114));
                    g.blk(x, 3, z, b2, p2);
                }
            }
        }
    }
    // Driftwood: weathered logs beached along the ragged tide line at varied
    // positions and axes — scattered, not paired.
    let logs: [(i32, i32, &str); 4] = [
        (sx / 4, -1, "x"),
        (sx / 2, 0, "z"),
        (3 * sx / 5, -2, "x"),
        (4 * sx / 5, -1, "x"),
    ];
    for (lx, dz, ax) in logs {
        let z = (coast_at(lx) + dz).clamp(2, sz - 2);
        if matches!(g.get(lx, 1, z), Cell::Block(_, _)) && matches!(g.get(lx, 2, z), Cell::Air) {
            g.blk(
                lx,
                2,
                z,
                "minecraft:stripped_oak_log",
                Some(vec![("axis", ax)]),
            );
            let (nx, nz) = if ax == "x" {
                (lx.saturating_sub(1), z)
            } else {
                (lx, z - 1)
            };
            if matches!(g.get(nx, 1, nz), Cell::Block(_, _))
                && matches!(g.get(nx, 2, nz), Cell::Air)
            {
                g.blk(nx, 2, nz, "minecraft:oak_log", Some(vec![("axis", ax)]));
            }
        }
    }
    // Dead bushes on the dry sand.
    for x in [sx / 5, 3 * sx / 4] {
        if matches!(g.get(x, 1, 2), Cell::Block(_, _)) {
            g.blk(x, 2, 2, "minecraft:dead_bush", None);
        }
    }
    // glow-lichen on the back cliff
    for x in 1..sx - 1 {
        for y in 2..sy - 1 {
            if matches!(g.get(x, y, 1), Cell::Air) && matches!(g.get(x, y, 0), Cell::Block(_, _)) {
                let n = value_noise(seed, x, y, 1, 0.5, 121);
                if n > 0.9 {
                    g.blk(
                        x,
                        y,
                        1,
                        "minecraft:glow_lichen",
                        Some(vec![("north", "true")]),
                    );
                }
            }
        }
    }
    // inland socket on the north cliff
    let (cells, jc) = doorway_cells(spec.size, Side::North, 0);
    for c in &cells {
        g.set(c[0], c[1], c[2], Cell::Air);
    }
    g.set(jc[0], jc[1], jc[2], Cell::Jigsaw(Side::North.orientation()));
}

// ---------------------------------------------------------------------------
// Gravity-floor substrate
// ---------------------------------------------------------------------------

/// Vanilla `FallingBlock`s — mirrors `crate::assembled::is_falling_block` in the
/// compiler. A cave floor may use these (sand/gravel are a first-class content
/// need), but in the delve's `the_void` world each one must rest on a solid block
/// or it despawns (compiler `DW0313`).
fn is_falling(name: &str) -> bool {
    let id = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        id,
        "sand" | "red_sand" | "gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg"
    ) || id.ends_with("_concrete_powder")
}

/// The hidden non-falling substrate laid directly beneath every gravity floor cell
/// (cave bedrock) so sand/gravel floors survive the void world, exactly as
/// cave-shore's beach seabed rests its sand on solid rock.
const SUBSTRATE: &str = "minecraft:stone";

/// Lift a built enclosed piece one block and fill a non-falling [`SUBSTRATE`] cell
/// directly beneath every gravity block that would otherwise sit over air: the
/// visible sand/gravel surface is preserved, and supported. Applied
/// uniformly to every enclosed piece, so all socket/anchor Ys shift by the same +1
/// and pieces still mate; the solver's socket mating absorbs the placement offset,
/// keeping assembled world coordinates stable. Deterministic (a pure grid map).
fn with_substrate(g: &Grid) -> Grid {
    let [sx, sy, sz] = g.size;
    let mut out = Grid::new([sx, sy + 1, sz]);
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                out.set(x, y + 1, z, g.get(x, y, z).clone());
            }
        }
    }
    for x in 0..sx {
        for y in 1..=sy {
            for z in 0..sz {
                let falling = matches!(out.get(x, y, z), Cell::Block(name, _) if is_falling(name));
                if falling && !matches!(out.get(x, y - 1, z), Cell::Block(_, _)) {
                    out.set(x, y - 1, z, Cell::Block(SUBSTRATE.to_string(), None));
                }
            }
        }
    }
    out
}

/// Generator invariant (belt-and-braces): every gravity block in a
/// finished piece must rest on a solid cell — a placement over air despawns in the
/// void world. Fails generation if any remain, automating the pitfall out of
/// existence at the tooling layer (the compiler's `DW0313` is the authoritative
/// gate). Returns the count of gravity cells checked (for logging).
fn assert_no_unsupported_gravity(id: &str, g: &Grid) -> usize {
    let [sx, sy, sz] = g.size;
    let mut gravity = 0usize;
    let mut bad = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if matches!(g.get(x, y, z), Cell::Block(name, _) if is_falling(name)) {
                    gravity += 1;
                    if !(y > 0 && matches!(g.get(x, y - 1, z), Cell::Block(_, _))) {
                        bad.push([x, y, z]);
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "cave-generator invariant: piece `{id}` has {} unsupported gravity block(s) over air \
         (would despawn in the_void — add a substrate): {:?}",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
    gravity
}

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

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

fn write_piece(out: &Path, spec: &Spec) {
    let grid0 = build(spec);
    // Light is measured over the authored walkable floor (y=0 frame), before the
    // substrate lift, so the estimate is unchanged by the hidden sub-floor.
    let min_light = if spec.open_air {
        15
    } else {
        estimate_min_floor_light(&grid0)
    };
    // Enclosed pieces get a non-falling substrate under every gravity floor cell,
    // which lifts their local geometry (floor/walls/ceiling/doorways/anchors) by 1
    // The open-air shore already builds a solid seabed under its beach,
    // so it is left as-authored. `yoff` is applied to the emitted metadata Ys so
    // sockets/anchors track the shifted grid; the solver's socket mating keeps the
    // assembled world coordinates stable across the uniform shift.
    let (grid, yoff) = if spec.open_air {
        (grid0, 0)
    } else {
        (with_substrate(&grid0), 1)
    };
    let size = [spec.size[0], spec.size[1] + yoff, spec.size[2]];
    // Belt-and-braces: no gravity block may sit over air in the shipped piece.
    assert_no_unsupported_gravity(spec.id, &grid);
    let structure = serialize(&grid);
    let cells = invariant_cells(&structure);
    invariants::assert_distress_never_stacks(spec.id, &cells);
    // Spelling, at the emitter: an unknown block id loads as AIR.
    invariants::assert_blocks_are_real(spec.id, &cells);
    let nbt = fastnbt::to_bytes(&structure).expect("nbt");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gz");
    let framed = gz.finish().expect("finish");
    std::fs::write(out.join(format!("{}.nbt", spec.id)), &framed).expect("write nbt");

    // connectors (jigsaw Y tracks the substrate lift)
    let mut connectors: Vec<ConnectorJson> = Vec::new();
    for &(side, fy) in &spec.doors {
        let (_, mut jc) = doorway_cells(spec.size, side, fy);
        jc[1] += yoff;
        connectors.push(ConnectorJson {
            name: "cave:socket",
            target: "cave:socket",
            local_pos: jc,
            facing: side.facing().into(),
            opening: [3, 3],
            joint: "aligned",
        });
    }
    // anchors (pos/region Y track the substrate lift)
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

    // derived lighting (profile from the pre-lift floor-light estimate above)
    let profile = if spec.open_air {
        "lit"
    } else {
        classify(min_light)
    };
    let method = if spec.open_air {
        "open-air cove: sky-lit (block-light estimate not applicable); floor daylight 15"
    } else {
        "static flood-fill block-light estimate over walkable floor cells (source emission −1/step through transparent cells); authoring estimate, not a live 1.21.11 probe"
    };

    let meta = MetaJson {
        prefab_id: format!("prefab/{}", spec.id),
        structure: StructureJson {
            file: format!("{}.nbt", spec.id),
            id: spec.id.into(),
            size,
            data_version: DATA_VERSION,
            generator: GENERATOR.into(),
        },
        anchors,
        connectors,
        lighting: LightingJson {
            profile,
            measured_min_light: min_light,
            measured: MEASURED_DATE,
            rationale: if profile == "dim" {
                Some(
                    spec.rationale
                        .unwrap_or("firelight pockets: a bronze-age cave lit by hearth/lanterns, dimmer at the rock edges by design")
                        .to_string(),
                )
            } else {
                None
            },
            method,
        },
        license: LicenseJson {
            source: "original",
            spdx: "GPL-3.0-or-later",
            note: "Original Delvewright project asset (pipeline-code license per prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            provenance: "Generated deterministically by prefabs/cave-generator (cave-prefab-gen), ADR-0006; regenerating yields byte-identical NBT.",
        },
    };
    let json = serde_json::to_string_pretty(&meta).expect("json") + "\n";
    std::fs::write(out.join(format!("{}.json", spec.id)), json).expect("write json");
    println!(
        "wrote {} ({} nbt bytes, profile {}, min-light {})",
        spec.id,
        framed.len(),
        profile,
        min_light,
    );
}

fn specs() -> Vec<Spec> {
    use Side::*;
    vec![
        // ENTRY: open-air shore cove (spawn vestibule + storybook shot).
        Spec {
            id: "cave-shore",
            size: [13, 6, 11],
            doors: vec![(North, 0)],
            wall_thickness: 1,
            open_air: true,
            lantern_grid: false,
            modules: vec![],
            stair: false,
            anchors: vec![
                ("spawn", a_pos([6, 2, 8], Some("north"))),
                ("anchor/exit", a_pos([6, 2, 1], Some("south"))),
            ],
            rationale: None,
            salt: 1,
        },
        // CONNECTORS
        Spec {
            id: "cave-passage-straight",
            size: [5, 5, 7],
            doors: vec![(North, 0), (South, 0)],
            wall_thickness: 1,
            open_air: false,
            lantern_grid: true,
            modules: vec![],
            stair: false,
            anchors: vec![],
            rationale: None,
            salt: 2,
        },
        Spec {
            id: "cave-passage-corner",
            size: [7, 5, 7],
            doors: vec![(North, 0), (East, 0)],
            wall_thickness: 1,
            open_air: false,
            lantern_grid: true,
            modules: vec![],
            stair: false,
            anchors: vec![],
            rationale: None,
            salt: 3,
        },
        Spec {
            id: "cave-passage-tee",
            size: [7, 5, 7],
            doors: vec![(North, 0), (East, 0), (West, 0)],
            wall_thickness: 1,
            open_air: false,
            lantern_grid: true,
            modules: vec![],
            stair: false,
            anchors: vec![],
            rationale: None,
            salt: 4,
        },
        Spec {
            id: "cave-passage-cross",
            size: [7, 5, 7],
            doors: vec![(North, 0), (South, 0), (East, 0), (West, 0)],
            wall_thickness: 1,
            open_air: false,
            lantern_grid: true,
            modules: vec![],
            stair: false,
            anchors: vec![],
            rationale: None,
            salt: 5,
        },
        // STAIR (vertical connector)
        Spec {
            id: "cave-descent",
            size: [5, 9, 11],
            doors: vec![(South, 0), (North, 4)],
            wall_thickness: 1,
            open_air: false,
            lantern_grid: false,
            modules: vec![],
            stair: true,
            anchors: vec![],
            rationale: None,
            salt: 6,
        },
        // ROOMS
        Spec {
            id: "cave-room-small",
            size: [7, 5, 7],
            doors: vec![(North, 0)],
            wall_thickness: 2,
            open_air: false,
            lantern_grid: true,
            modules: vec![Module::Hearth(3, 4)],
            stair: false,
            anchors: vec![
                ("anchor/npc-stand", a_pos([3, 1, 2], Some("north"))),
                ("anchor/chest", a_pos([3, 1, 2], Some("north"))),
            ],
            rationale: None,
            salt: 7,
        },
        Spec {
            id: "cave-den",
            size: [9, 5, 9],
            doors: vec![(North, 0), (South, 0)],
            wall_thickness: 2,
            open_air: false,
            lantern_grid: true,
            // cave-den is a 9×5×9 room whose 5×5 interior is fully consumed by the
            // door-to-door critical corridor (both N and S sockets open on the same
            // x=3..5 columns). It carries NO herd/wave anchor: any wave seated here
            // has no footing ≥3 Chebyshev off the proven path, so the flock would
            // wall the 1-wide return corridor. The flock is hosted by
            // cave-cavern, which has a genuine side alcove for it. Removing the
            // `anchor/wave` entirely makes that mistake unrepresentable — a
            // corridor-only piece exposes no wave seat (owner decision).
            modules: vec![],
            stair: false,
            anchors: vec![("anchor/npc-stand", a_pos([3, 1, 4], Some("north")))],
            rationale: None,
            salt: 8,
        },
        Spec {
            id: "cave-hollow",
            size: [9, 5, 7],
            doors: vec![(North, 0), (East, 0)],
            wall_thickness: 2,
            open_air: false,
            lantern_grid: true,
            modules: vec![],
            stair: false,
            anchors: vec![
                ("anchor/npc-stand", a_pos([3, 1, 3], Some("east"))),
                ("anchor/door", a_pos([3, 1, 3], Some("east"))),
            ],
            rationale: None,
            salt: 9,
        },
        // TERMINALS
        Spec {
            id: "cave-mouth",
            size: [7, 5, 9],
            doors: vec![(North, 0), (South, 0)],
            wall_thickness: 2,
            open_air: false,
            lantern_grid: true,
            modules: vec![Module::Boulder(South)],
            stair: false,
            anchors: vec![
                ("anchor/gate", a_region([2, 1, 8], [4, 3, 8], "minecraft:cobblestone")),
                ("anchor/keeper-stand", a_pos([3, 1, 5], Some("south"))),
            ],
            rationale: None,
            salt: 10,
        },
        Spec {
            id: "cave-hearth",
            size: [9, 5, 9],
            doors: vec![(North, 0)],
            wall_thickness: 2,
            open_air: false,
            lantern_grid: false,
            modules: vec![Module::Hearth(4, 5)],
            stair: false,
            anchors: vec![("anchor/objective", a_pos([4, 1, 6], Some("north")))],
            rationale: Some(
                "firelit shrine: a single hearth is the only light, bright at the fire and dim at the walls — the intended bronze-age atmosphere",
            ),
            salt: 11,
        },
        Spec {
            id: "cave-cavern",
            size: [13, 6, 15],
            doors: vec![(North, 0)],
            wall_thickness: 2,
            open_air: false,
            lantern_grid: true,
            // Sheep pen relocated to the front-west alcove (local x2..4, z2..4),
            // clear of the proven path — which runs straight down the x=6 centreline
            // from the north door (z0) to the boss/objective at z10..12 — and clear
            // of the door span (x5..7). The round-2 placement (x9..11, z3..5) hugged
            // that centreline and put a fence line on the wall (x11), flanking the
            // path and inflating the bot's A* search. Pen is dressing, not
            // stock; keeping it here preserves the good greeble off the corridor.
            modules: vec![Module::Hearth(6, 11), Module::Pen(2, 2, 3, 6)],
            stair: false,
            anchors: vec![
                ("anchor/boss", a_pos([6, 1, 10], Some("north"))),
                ("anchor/objective", a_pos([6, 1, 12], Some("north"))),
                // The flock's spawn-wave home (owner decision): a genuine
                // side alcove in the east of the 13×6×15 cavern. The proven path runs
                // down the x=6 centreline (north door z0 → boss z10..12); every cell
                // of the x≥9 strip is ≥3 Chebyshev from that line, and the open cavern
                // floor there seats ≥6 sheep on supported rock without walling the
                // boss approach or colliding with the front-west pen (x2..4).
                ("anchor/wave", a_pos([9, 1, 5], Some("west"))),
            ],
            rationale: Some(
                "boss cavern lit by scattered hearth-fire: dramatic firelight pockets, dark at the vault edges by design (the Cyclops' hall)",
            ),
            salt: 12,
        },
        Spec {
            id: "cave-niche",
            size: [5, 5, 5],
            doors: vec![(North, 0)],
            wall_thickness: 1,
            open_air: false,
            lantern_grid: true,
            modules: vec![],
            stair: false,
            anchors: vec![],
            rationale: None,
            salt: 13,
        },
    ]
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: cave-prefab-gen <out_dir>");
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("mkdir");
    let specs = specs();
    for spec in &specs {
        write_piece(out, spec);
    }
    println!("{} pieces", specs.len());
}
