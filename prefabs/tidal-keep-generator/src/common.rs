//! Shared machinery for the tidal-keep generator: deterministic noise, palettes,
//! the cell grid, vanilla-structure NBT emission, `tk:socket` geometry, metadata
//! JSON, the derived block-light estimate, the gravity-substrate invariant, and
//! the redstone-dust wiring helper the trap pieces use.
//!
//! The primitive family (splitmix64 hashing, trilinear value noise, the `.nbt`
//! writer, keep-socket geometry, `assert_no_unsupported_gravity`) is Delvewright's
//! own, carried over from `prefabs/island-terrain-generator`. No third-party
//! material ingested (ADR-0013).

use std::collections::BTreeMap;

use serde::Serialize;

/// MC 1.21.11 (ADR-0009).
pub const DATA_VERSION: i32 = 4671;
pub const GENERATOR: &str = "prefabs/tidal-keep-generator (tidal-keep-gen)";
pub const MEASURED_DATE: &str = "2026-08-02";

/// The tidal-keep socket vocabulary (keep-socket-v1 geometry, `tk` namespace).
pub const SOCKET_NAME: &str = "tk:socket";
pub const SOCKET_POOL: &str = "tk:pool";

/// Shore datum (shared with `prefabs/island-tileset.md`): waterline local y=2,
/// walk plane local y=3, sockets at `floor_y = 2`.
pub const SHORE_FLOOR_Y: i32 = 2;
pub const SHORE_WALK: i32 = 3;
/// Keep datum: the courtyard plinth. Solid to local y=10, walk plane y=11,
/// sockets at `floor_y = 10`.
pub const KEEP_FLOOR_Y: i32 = 10;
pub const KEEP_WALK: i32 = 11;

// ---------------------------------------------------------------------------
// Deterministic hashing / value noise
// ---------------------------------------------------------------------------

pub fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn piece_seed(id: &str, salt: u64) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    mix64(h ^ salt)
}

pub fn hash01(seed: u64, x: i32, y: i32, z: i32, salt: u64) -> f64 {
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

/// Trilinearly-interpolated value noise in [0,1] — smooth, so palette picks
/// cluster into weather-stained bands instead of per-cell speckle.
pub fn value_noise(seed: u64, x: i32, y: i32, z: i32, freq: f64, salt: u64) -> f64 {
    let (fx, fy, fz) = (x as f64 * freq, y as f64 * freq, z as f64 * freq);
    let (x0, y0, z0) = (fx.floor(), fy.floor(), fz.floor());
    let (tx, ty, tz) = (fade(fx - x0), fade(fy - y0), fade(fz - z0));
    let (ix, iy, iz) = (x0 as i32, y0 as i32, z0 as i32);
    let c = |dx: i32, dy: i32, dz: i32| hash01(seed, ix + dx, iy + dy, iz + dz, salt);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), tx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), tx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), tx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), tx);
    lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz)
}

// ---------------------------------------------------------------------------
// Palettes
// ---------------------------------------------------------------------------

pub type Props = Option<Vec<(&'static str, &'static str)>>;

pub struct Recipe {
    pub name: &'static str,
    pub weight: f64,
}
pub fn r(name: &'static str, weight: f64) -> Recipe {
    Recipe { name, weight }
}

pub fn pick(palette: &[Recipe], n: f64) -> &'static str {
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

/// Keep masonry: stone brick with a heavy salt-weathered fraction (cracked,
/// mossy) — the tidal keep has stood in spray for a long time.
pub fn keep_wall() -> Vec<Recipe> {
    vec![
        r("minecraft:stone_bricks", 0.40),
        r("minecraft:mossy_stone_bricks", 0.21),
        r("minecraft:cracked_stone_bricks", 0.18),
        r("minecraft:cobblestone", 0.11),
        r("minecraft:andesite", 0.06),
        r("minecraft:chiseled_stone_bricks", 0.04),
    ]
}
/// Keep floor: the same masonry, gritted with worn cobble and gravel.
pub fn keep_floor() -> Vec<Recipe> {
    vec![
        r("minecraft:stone_bricks", 0.32),
        r("minecraft:cobblestone", 0.24),
        r("minecraft:mossy_stone_bricks", 0.16),
        r("minecraft:cracked_stone_bricks", 0.15),
        r("minecraft:andesite", 0.09),
        r("minecraft:mossy_cobblestone", 0.04),
    ]
}
/// The plinth the keep stands on (solid mass below the courtyard datum).
pub fn plinth() -> Vec<Recipe> {
    vec![
        r("minecraft:stone", 0.38),
        r("minecraft:andesite", 0.24),
        r("minecraft:tuff", 0.22),
        r("minecraft:cobblestone", 0.16),
    ]
}
/// Undercroft masonry: sea-rotted, prismarine bloom on old brick.
pub fn tide_wall() -> Vec<Recipe> {
    vec![
        r("minecraft:mossy_stone_bricks", 0.24),
        r("minecraft:cracked_stone_bricks", 0.20),
        r("minecraft:mossy_cobblestone", 0.18),
        r("minecraft:prismarine", 0.14),
        r("minecraft:tuff", 0.14),
        r("minecraft:dark_prismarine", 0.10),
    ]
}
/// Undercroft floor, above the waterline.
pub fn tide_floor() -> Vec<Recipe> {
    vec![
        r("minecraft:cobblestone", 0.26),
        r("minecraft:mossy_cobblestone", 0.22),
        r("minecraft:prismarine", 0.18),
        r("minecraft:cracked_stone_bricks", 0.16),
        r("minecraft:gravel", 0.10),
        r("minecraft:moss_block", 0.08),
    ]
}
/// Shore sand (always laid over the solid plinth — never over air).
pub fn shore_sand() -> Vec<Recipe> {
    vec![
        r("minecraft:sand", 0.56),
        r("minecraft:gravel", 0.26),
        r("minecraft:coarse_dirt", 0.12),
        r("minecraft:stone", 0.06),
    ]
}
/// Barrow-field turf.
pub fn turf() -> Vec<Recipe> {
    vec![
        r("minecraft:grass_block", 0.70),
        r("minecraft:coarse_dirt", 0.14),
        r("minecraft:podzol", 0.08),
        r("minecraft:moss_block", 0.08),
    ]
}
/// Burial-mound stone.
pub fn barrow_stone() -> Vec<Recipe> {
    vec![
        r("minecraft:mossy_cobblestone", 0.40),
        r("minecraft:cobblestone", 0.28),
        r("minecraft:andesite", 0.20),
        r("minecraft:mossy_stone_bricks", 0.12),
    ]
}
/// The WORN lane of the boulder stair: the brick face is gone, polished smooth by
/// a century of stone rolling over it. This is the 初见杀 tell, in plain sight.
pub fn tread_worn() -> Vec<Recipe> {
    vec![
        r("minecraft:smooth_stone", 0.46),
        r("minecraft:polished_andesite", 0.34),
        r("minecraft:stone", 0.20),
    ]
}
/// The UNTRODDEN flanks of the same stair: masonry the boulder never touches.
pub fn tread_unworn() -> Vec<Recipe> {
    vec![
        r("minecraft:mossy_stone_bricks", 0.38),
        r("minecraft:cracked_stone_bricks", 0.32),
        r("minecraft:stone_bricks", 0.20),
        r("minecraft:mossy_cobblestone", 0.10),
    ]
}

// ---------------------------------------------------------------------------
// Cell grid
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum Cell {
    Air,
    Block(String, Props),
    Jigsaw(&'static str),
}

pub struct Grid {
    pub size: [i32; 3],
    pub cells: Vec<Cell>,
}

impl Grid {
    pub fn new(size: [i32; 3]) -> Self {
        let n = (size[0] * size[1] * size[2]) as usize;
        Grid {
            size,
            cells: vec![Cell::Air; n],
        }
    }
    pub fn idx(&self, x: i32, y: i32, z: i32) -> usize {
        ((x * self.size[1] + y) * self.size[2] + z) as usize
    }
    pub fn inb(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < self.size[0] && y < self.size[1] && z < self.size[2]
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, c: Cell) {
        if self.inb(x, y, z) {
            let i = self.idx(x, y, z);
            self.cells[i] = c;
        }
    }
    pub fn get(&self, x: i32, y: i32, z: i32) -> &Cell {
        &self.cells[self.idx(x, y, z)]
    }
    pub fn blk(&mut self, x: i32, y: i32, z: i32, name: &str, props: Props) {
        self.set(x, y, z, Cell::Block(name.to_string(), props));
    }
    pub fn air(&mut self, x: i32, y: i32, z: i32) {
        self.set(x, y, z, Cell::Air);
    }
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.inb(x, y, z) && matches!(self.get(x, y, z), Cell::Block(_, _))
    }
    pub fn is_air(&self, x: i32, y: i32, z: i32) -> bool {
        self.inb(x, y, z) && matches!(self.get(x, y, z), Cell::Air)
    }
    pub fn name_at(&self, x: i32, y: i32, z: i32) -> Option<&str> {
        match self.get(x, y, z) {
            Cell::Block(n, _) => Some(n.as_str()),
            _ => None,
        }
    }

    /// Fill an inclusive box with one block.
    pub fn fill(&mut self, b: [i32; 6], name: &str, props: Props) {
        for x in b[0]..=b[1] {
            for y in b[2]..=b[3] {
                for z in b[4]..=b[5] {
                    self.blk(x, y, z, name, props.clone());
                }
            }
        }
    }
    /// Fill an inclusive box from a noise palette.
    pub fn fill_pal(&mut self, b: [i32; 6], pal: &[Recipe], seed: u64, freq: f64, salt: u64) {
        for x in b[0]..=b[1] {
            for y in b[2]..=b[3] {
                for z in b[4]..=b[5] {
                    let n = value_noise(seed, x, y, z, freq, salt);
                    self.blk(x, y, z, pick(pal, n), None);
                }
            }
        }
    }
    /// Carve an inclusive box to air.
    pub fn carve(&mut self, b: [i32; 6]) {
        for x in b[0]..=b[1] {
            for y in b[2]..=b[3] {
                for z in b[4]..=b[5] {
                    self.air(x, y, z);
                }
            }
        }
    }
}

/// An inclusive box helper: `bx(x0,x1,y0,y1,z0,z1)`.
pub fn bx(x0: i32, x1: i32, y0: i32, y1: i32, z0: i32, z1: i32) -> [i32; 6] {
    [x0, x1, y0, y1, z0, z1]
}

// ---------------------------------------------------------------------------
// NBT serialization (vanilla structure format)
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, PartialEq)]
pub struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}
#[derive(Serialize)]
pub struct BlockEntry {
    pos: [i32; 3],
    state: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    nbt: Option<JigsawBE>,
}
#[derive(Serialize, Clone)]
pub struct JigsawBE {
    id: String,
    name: String,
    target: String,
    pool: String,
    final_state: String,
    joint: String,
}
#[derive(Serialize)]
pub struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<Entity>,
}
#[derive(Serialize)]
pub struct Entity {}

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

/// The flattened view the shared `invariants` gates read: exactly the blocks this
/// piece is about to write, palette already resolved. Lives here because
/// `Structure`'s fields are private to this module.
pub fn invariant_cells(s: &Structure) -> crate::invariants::Cells {
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

pub fn serialize(grid: &Grid) -> Structure {
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
// Sockets (keep-socket-v1 geometry, `tk` vocabulary)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    North,
    South,
    West,
    East,
}
impl Side {
    pub fn orientation(self) -> &'static str {
        match self {
            Side::North => "north_up",
            Side::South => "south_up",
            Side::West => "west_up",
            Side::East => "east_up",
        }
    }
    pub fn facing(self) -> &'static str {
        match self {
            Side::North => "north",
            Side::South => "south",
            Side::West => "west",
            Side::East => "east",
        }
    }
}

/// The socket's jigsaw cell for a given side. `along` is the coordinate along the
/// face (x for N/S faces, z for W/E faces); the socket is a 3-wide × 3-tall
/// opening based one cell above `floor_y`.
pub fn door_center(size: [i32; 3], side: Side, floor_y: i32, along: i32) -> [i32; 3] {
    let (x, z) = (size[0], size[2]);
    let y = floor_y + 1;
    match side {
        Side::North => [along, y, 0],
        Side::South => [along, y, z - 1],
        Side::West => [0, y, along],
        Side::East => [x - 1, y, along],
    }
}

/// Carve the 3×3 opening and set the jigsaw marker. Returns the jigsaw cell.
pub fn cut_socket(g: &mut Grid, side: Side, floor_y: i32, along: i32) -> [i32; 3] {
    let jc = door_center(g.size, side, floor_y, along);
    let base = floor_y + 1;
    for dy in 0..3 {
        for d in -1..=1 {
            let p = match side {
                Side::North | Side::South => [jc[0] + d, base + dy, jc[2]],
                Side::West | Side::East => [jc[0], base + dy, jc[2] + d],
            };
            g.air(p[0], p[1], p[2]);
        }
    }
    g.set(jc[0], jc[1], jc[2], Cell::Jigsaw(side.orientation()));
    jc
}

// ---------------------------------------------------------------------------
// Metadata JSON (the exact shape `compiler::registry::PrefabMeta` deserializes;
// every struct there is `deny_unknown_fields`, so no field may drift)
// ---------------------------------------------------------------------------

/// What a POINT anchor is *for*, and therefore what "correct" means for it.
///
/// Until spec-0021/0022 every point anchor was somewhere a player, mob or marker
/// STANDS, so one invariant (standability) covered them all. Two of the new
/// stage-5 surfaces name cells that are deliberately not footings, and each is
/// the exact opposite of standable — a volley slot must be EMPTY, a loot anchor
/// must be FULL. Declaring the class lets each get the check that is actually
/// true of it, instead of one check that is wrong for two of them.
/// Generator-side only: the emitted metadata JSON is unchanged, because the
/// compiler learns an anchor's role from the DSL that references it.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum AnchorKind {
    /// A footing: standable (air at feet + head, solid dry floor below).
    #[default]
    Footing,
    /// A firing slot (spec-0022 `volley.from_anchor`): clear and dry, so a
    /// summoned projectile actually leaves it. Mirrors `DW0446`.
    Slot,
    /// A container the prefab already placed (spec-0021 `loot[].anchor`): the
    /// compiler FILLS it and never places it, so the furniture has to be really
    /// there. Mirrors `DW0431`.
    Container,
}

/// The containers `item replace block … container.<n>` can address, and that
/// `DW0431` accepts. Double chests are excluded there (two block entities make
/// `container.<n>` ambiguous) and so are they here.
pub const FILLABLE: [&str; 3] = [
    "minecraft:chest",
    "minecraft:trapped_chest",
    "minecraft:barrel",
];

#[derive(Serialize, Clone, Default)]
pub struct AnchorJson {
    /// Generator-side classification; never serialised (the compiler reads the
    /// anchor's ROLE from the DSL that references it, not from the metadata).
    #[serde(skip)]
    pub kind: AnchorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<RegionJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispenser: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_block: Option<String>,
}
#[derive(Serialize, Clone)]
pub struct RegionJson {
    pub from: [i32; 3],
    pub to: [i32; 3],
}
#[derive(Serialize)]
pub struct ConnectorJson {
    pub name: &'static str,
    pub target: &'static str,
    pub local_pos: [i32; 3],
    pub facing: String,
    pub opening: [i32; 2],
    pub joint: &'static str,
}
#[derive(Serialize)]
pub struct StructureJson {
    pub file: String,
    pub id: String,
    pub size: [i32; 3],
    pub data_version: i32,
    pub generator: String,
}
#[derive(Serialize)]
pub struct LightingJson {
    pub profile: &'static str,
    pub measured_min_light: i32,
    pub measured: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    pub method: &'static str,
}
#[derive(Serialize)]
pub struct LicenseJson {
    pub source: &'static str,
    pub spdx: &'static str,
    pub note: &'static str,
    pub provenance: &'static str,
}
#[derive(Serialize)]
pub struct MetaJson {
    pub prefab_id: String,
    pub structure: StructureJson,
    /// The piece's authored walk-plane datum (spec-0026 §2): the feet-cell
    /// local y of its **lowest** socket floor — shore pieces 3
    /// (`SHORE_FLOOR_Y + 1`), plinth pieces 11 (`KEEP_FLOOR_Y + 1`). Consumed
    /// by the compiler's per-area placement datum (`walk_ref_y − walk_y`;
    /// missing in a non-void horizon is `DW0367`).
    pub walk_y: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waterline_y: Option<i32>,
    pub anchors: BTreeMap<String, AnchorJson>,
    pub connectors: Vec<ConnectorJson>,
    pub lighting: LightingJson,
    pub license: LicenseJson,
}

pub fn a_pos(pos: [i32; 3], facing: &str) -> AnchorJson {
    AnchorJson {
        pos: Some(pos),
        facing: Some(facing.to_string()),
        ..Default::default()
    }
}
/// A volley firing slot (spec-0022 `from_anchor`): the open cell a summoned
/// projectile spawns in, NOT a place anything stands. `DW0446` rejects a solid
/// or flooded one; `assert_anchors_sane` holds the generator to the same rule.
pub fn a_slot(pos: [i32; 3], facing: &str) -> AnchorJson {
    AnchorJson {
        kind: AnchorKind::Slot,
        pos: Some(pos),
        facing: Some(facing.to_string()),
        ..Default::default()
    }
}
/// A loot container (spec-0021 `loot[].anchor`): the cell of a chest, trapped
/// chest or barrel the PIECE places. The compiler only ever fills it, so a
/// generator that moves the furniture without moving the anchor is caught here
/// rather than by `DW0431` two layers later — and `item replace block` against a
/// non-container fails SILENTLY, which is why both layers check.
pub fn a_container(pos: [i32; 3], facing: &str) -> AnchorJson {
    AnchorJson {
        kind: AnchorKind::Container,
        pos: Some(pos),
        facing: Some(facing.to_string()),
        ..Default::default()
    }
}
pub fn a_region(from: [i32; 3], to: [i32; 3], block: &str) -> AnchorJson {
    AnchorJson {
        region: Some(RegionJson { from, to }),
        block: Some(block.to_string()),
        ..Default::default()
    }
}
/// A trap marker: `pos` is the trigger cell the compiler models as the hazard,
/// `dispenser` the pre-wired (empty) dispenser socket it loads, `trigger_block`
/// the plate blockstate a flag-gated trap must be able to restore verbatim
/// (`DW0363`).
pub fn a_trap(pos: [i32; 3], facing: &str, dispenser: [i32; 3], trigger_block: &str) -> AnchorJson {
    AnchorJson {
        pos: Some(pos),
        facing: Some(facing.to_string()),
        dispenser: Some(dispenser),
        trigger_block: Some(trigger_block.to_string()),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Derived block-light estimate (authoring value, not a live probe)
// ---------------------------------------------------------------------------

pub fn emission(name: &str) -> i32 {
    match name {
        "minecraft:campfire" => 15,
        "minecraft:soul_campfire" => 10,
        "minecraft:lantern" => 15,
        "minecraft:soul_lantern" => 10,
        "minecraft:torch" | "minecraft:wall_torch" => 14,
        "minecraft:glowstone" | "minecraft:shroomlight" | "minecraft:sea_lantern" => 15,
        "minecraft:glow_lichen" => 7,
        "minecraft:magma_block" => 3,
        _ => 0,
    }
}

pub fn transparent(cell: &Cell) -> bool {
    match cell {
        Cell::Air | Cell::Jigsaw(_) => true,
        Cell::Block(name, _) => matches!(
            name.as_str(),
            "minecraft:water"
                | "minecraft:campfire"
                | "minecraft:soul_campfire"
                | "minecraft:lantern"
                | "minecraft:soul_lantern"
                | "minecraft:torch"
                | "minecraft:wall_torch"
                | "minecraft:chain"
                | "minecraft:iron_bars"
                | "minecraft:oak_fence"
                | "minecraft:stone_brick_wall"
                | "minecraft:cobblestone_wall"
                | "minecraft:glow_lichen"
                | "minecraft:vine"
                | "minecraft:pointed_dripstone"
                | "minecraft:moss_carpet"
                | "minecraft:short_grass"
                | "minecraft:dead_bush"
                | "minecraft:seagrass"
                | "minecraft:redstone_wire"
                | "minecraft:stone_pressure_plate"
                | "minecraft:ladder"
                | "minecraft:bell"
        ),
    }
}

/// Static flood-fill block-light estimate. Returns the minimum block light over
/// every **standable** cell inside the box (air cell, air above, solid below) —
/// the same shape as the compiler's reachable-walkable set, restricted to a
/// region the caller declares as the piece's interior. Sky light is NOT counted
/// (conservative for roofed volumes; meaningless for open-air pieces, which
/// declare `lit` 15 directly).
pub fn estimate_min_light(grid: &Grid, region: [i32; 6]) -> i32 {
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
            if l - 1 > light[i] {
                light[i] = l - 1;
                queue.push_back((nx, ny, nz, l - 1));
            }
        }
    }
    let mut min = i32::MAX;
    let mut worst = [0i32; 3];
    let mut hist = [0usize; 16];
    for x in region[0]..=region[1] {
        for y in region[2]..=region[3] {
            for z in region[4]..=region[5] {
                if standable(grid, [x, y, z]) {
                    let l = light[grid.idx(x, y, z)];
                    hist[l.clamp(0, 15) as usize] += 1;
                    if l < min {
                        min = l;
                        worst = [x, y, z];
                    }
                }
            }
        }
    }
    if std::env::var("TK_DEBUG_LIGHT").is_ok() {
        let dim: usize = hist[..7].iter().sum();
        eprintln!(
            "  region {region:?}: min {} at {worst:?}  ({dim} cells below 7)",
            if min == i32::MAX { 0 } else { min }
        );
    }
    if min == i32::MAX {
        0
    } else {
        min
    }
}

pub fn classify(min_light: i32) -> &'static str {
    if min_light >= 7 {
        "lit"
    } else if min_light >= 3 {
        "dim"
    } else {
        "dark"
    }
}

// ---------------------------------------------------------------------------
// Generator invariants
// ---------------------------------------------------------------------------

fn is_falling(name: &str) -> bool {
    let id = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        id,
        "sand" | "red_sand" | "gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg"
    ) || id.ends_with("_concrete_powder")
}

/// No gravity block may sit over air in a finished piece (it would fall/despawn).
/// The compiler's `DW0313` is the authoritative gate; this fails generation first
/// so the pitfall never leaves the tooling layer.
pub fn assert_no_unsupported_gravity(id: &str, g: &Grid) {
    let [sx, sy, sz] = g.size;
    let mut bad = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if matches!(g.get(x, y, z), Cell::Block(name, _) if is_falling(name))
                    && !(y > 0 && g.is_solid(x, y - 1, z))
                {
                    bad.push([x, y, z]);
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "tidal-keep-gen invariant: piece `{id}` has {} unsupported gravity block(s): {:?}",
        bad.len(),
        &bad[..bad.len().min(8)]
    );
}

/// Whether (x,z) is within `rad` of any anchor's point cell (dressing keeps off
/// anchors and their immediate surround, so a marker never lands under a tuft).
pub fn near_anchor(anchors: &[(&'static str, AnchorJson)], x: i32, z: i32, rad: i32) -> bool {
    anchors.iter().any(|(_, a)| {
        a.pos
            .map(|p| (p[0] - x).abs() <= rad && (p[2] - z).abs() <= rad)
            .unwrap_or(false)
    })
}

/// Whether a cell is PASSABLE to a walker under the current nav model
/// (`crates/compiler/src/assembled.rs`): air, or one of the blocks the model
/// deliberately leaves walk-through — the trap triggers (so nav can route a
/// player ONTO a plate; load-bearing for `DW0342`) and sub-half-block decoration.
/// Everything else, including stairs and every unlisted block, is a full cube.
pub fn passable(g: &Grid, c: [i32; 3]) -> bool {
    match g.get(c[0], c[1], c[2]) {
        Cell::Air | Cell::Jigsaw(_) => true,
        Cell::Block(n, _) => {
            let id = n.strip_prefix("minecraft:").unwrap_or(n);
            id.ends_with("_pressure_plate")
                || id == "tripwire"
                || id == "tripwire_hook"
                || id.ends_with("_carpet")
                || id == "moss_carpet"
        }
    }
}

/// Every declared route cell must be standable under the CURRENT nav model
/// (`crates/compiler/src/nav.rs`): the feet cell and the cell above are passable,
/// with a **solid** block directly below (water is never a floor).
pub fn standable(g: &Grid, c: [i32; 3]) -> bool {
    g.inb(c[0], c[1], c[2])
        && passable(g, c)
        && g.inb(c[0], c[1] + 1, c[2])
        && passable(g, [c[0], c[1] + 1, c[2]])
        && g.is_solid(c[0], c[1] - 1, c[2])
        && g.name_at(c[0], c[1] - 1, c[2]) != Some("minecraft:water")
}

/// Generator-side walkability proof for an authored route (debug doctrine: the
/// lesson is pinned as an invariant, not prose). Every listed cell must be
/// standable, and consecutive cells must be a legal nav step: cardinal-adjacent
/// with |dy| ≤ 1, and a full-block rise needs the head-sweep cell clear
/// (`MAX_AUTO_STEP_16 = 9/16` — a full 16/16 rise is a jump).
pub fn assert_route_walkable(id: &str, label: &str, g: &Grid, route: &[[i32; 3]]) {
    for (i, &c) in route.iter().enumerate() {
        assert!(
            standable(g, c),
            "{id}: route `{label}` cell #{i} {c:?} is not standable (needs air at feet+head, \
             solid floor below — water is never a floor in the nav model)"
        );
    }
    for w in route.windows(2) {
        let (a, b) = (w[0], w[1]);
        let man = (a[0] - b[0]).abs() + (a[2] - b[2]).abs();
        let dy = b[1] - a[1];
        assert!(
            man == 1 && dy.abs() <= 1,
            "{id}: route `{label}` step {a:?} -> {b:?} is not a nav edge (cardinal, |dy| <= 1)"
        );
        if dy == 1 {
            assert!(
                passable(g, [a[0], a[1] + 2, a[2]]),
                "{id}: route `{label}` rise {a:?} -> {b:?} is a full block (a jump) with no head \
                 clearance at {:?}",
                [a[0], a[1] + 2, a[2]]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Redstone wiring (trap hardware — the prefab owns it; the compiler emits none)
// ---------------------------------------------------------------------------

fn dust_dir(from: [i32; 3], to: [i32; 3]) -> (&'static str, &'static str) {
    // (property on `from` pointing at `to`, value)
    let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
    let dir = if dx > 0 {
        "east"
    } else if dx < 0 {
        "west"
    } else if dz > 0 {
        "south"
    } else {
        "north"
    };
    // A dust cell sees a HIGHER neighbour as `up`, a level or lower one as `side`.
    let val = if to[1] > from[1] { "up" } else { "side" };
    (dir, val)
}

/// Lay an explicit redstone-dust chain, authoring each cell's connection
/// properties (structure placement must not depend on a post-placement redstone
/// re-shape). Asserts the vanilla preconditions for a dust "staircase": solid
/// support under every dust cell, air above it, and — for an up-step — air above
/// the LOWER dust so the connection is not cut.
pub fn wire_dust(id: &str, g: &mut Grid, path: &[[i32; 3]]) {
    for (i, &c) in path.iter().enumerate() {
        assert!(
            g.is_solid(c[0], c[1] - 1, c[2]),
            "{id}: redstone dust at {c:?} has no solid support below"
        );
        let mut props: Vec<(&'static str, &'static str)> = vec![
            ("north", "none"),
            ("south", "none"),
            ("east", "none"),
            ("west", "none"),
            ("power", "0"),
        ];
        let set = |k: &'static str, v: &'static str, p: &mut Vec<(&'static str, &'static str)>| {
            for e in p.iter_mut() {
                if e.0 == k {
                    e.1 = v;
                }
            }
        };
        if i > 0 {
            let (d, v) = dust_dir(c, path[i - 1]);
            set(d, v, &mut props);
        }
        if i + 1 < path.len() {
            let (d, v) = dust_dir(c, path[i + 1]);
            set(d, v, &mut props);
            if path[i + 1][1] > c[1] {
                // the higher neighbour only connects if the cell above THIS dust is clear
                assert!(
                    g.is_air(c[0], c[1] + 1, c[2]),
                    "{id}: dust up-step from {c:?} is cut by a block at {:?}",
                    [c[0], c[1] + 1, c[2]]
                );
            }
        }
        props.sort_by_key(|e| e.0);
        g.blk(c[0], c[1], c[2], "minecraft:redstone_wire", Some(props));
    }
}

/// A pressure-plate blockstate, authored verbatim so a flag-gated trap can
/// restore it (`trigger_block`, `DW0363`).
pub const PLATE_BLOCK: &str = "minecraft:stone_pressure_plate[powered=false]";

pub fn plate(g: &mut Grid, x: i32, y: i32, z: i32) {
    g.blk(
        x,
        y,
        z,
        "minecraft:stone_pressure_plate",
        Some(vec![("powered", "false")]),
    );
}

pub fn dispenser(g: &mut Grid, c: [i32; 3], facing: &'static str) {
    g.blk(
        c[0],
        c[1],
        c[2],
        "minecraft:dispenser",
        Some(vec![("facing", facing), ("triggered", "false")]),
    );
}

/// Hang a lamp on a chain from a solid ceiling: `ceil_solid` is the solid ceiling
/// layer, the chain drops `drop` cells and the lamp hangs under it. Skipped if the
/// ceiling is not solid or the shaft is not clear, so a lattice can be applied
/// blindly over an irregular room.
pub fn chandelier(g: &mut Grid, x: i32, z: i32, ceil_solid: i32, drop: i32, lamp: &str) {
    if !g.is_solid(x, ceil_solid, z) {
        return;
    }
    for dy in 1..=drop {
        if !g.is_air(x, ceil_solid - dy, z) {
            return;
        }
    }
    if !g.is_air(x, ceil_solid - drop - 1, z) {
        return;
    }
    // The stem is masonry, not chain: Nucleation (the per-piece review renderer)
    // has no blockstate for `minecraft:chain`, so a chain-hung lamp reads as a
    // lamp floating in mid-air in every review shot. A corbel post renders, and
    // reads as a bracket. Chain survives only where it IS the subject (the bell
    // ropes), with the gap recorded in the tileset doc.
    for dy in 1..=drop {
        g.blk(x, ceil_solid - dy, z, "minecraft:stone_brick_wall", None);
    }
    g.blk(
        x,
        ceil_solid - drop - 1,
        z,
        lamp,
        Some(vec![("hanging", "true")]),
    );
}

/// Wall sconces around a rectangular room at `y`, every `spacing` cells, INCLUDING
/// the four corners (an unlit corner is exactly where the measured minimum lands).
#[allow(clippy::too_many_arguments)]
pub fn sconces(
    g: &mut Grid,
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    y: i32,
    spacing: i32,
    skip: &dyn Fn(i32, i32) -> bool,
) {
    let put = |g: &mut Grid, c: [i32; 3], wall: [i32; 3], facing: &'static str| {
        if !skip(c[0], c[2]) && g.is_air(c[0], c[1], c[2]) && g.is_solid(wall[0], wall[1], wall[2])
        {
            g.blk(
                c[0],
                c[1],
                c[2],
                "minecraft:wall_torch",
                Some(vec![("facing", facing)]),
            );
        }
    };
    let xs: Vec<i32> = (x0..=x1).step_by(spacing as usize).chain([x1]).collect();
    let zs: Vec<i32> = (z0..=z1).step_by(spacing as usize).chain([z1]).collect();
    for &x in &xs {
        put(g, [x, y, z0], [x, y, z0 - 1], "south");
        put(g, [x, y, z1], [x, y, z1 + 1], "north");
    }
    for &z in &zs {
        put(g, [x0, y, z], [x0 - 1, y, z], "east");
        put(g, [x1, y, z], [x1 + 1, y, z], "west");
    }
}

/// Light an enclosed room to the spec-0001 `lit` bar without ever intruding on the
/// walk envelope: wall sconces sit at `walk + 3` — above the head cell AND above
/// the jump head-sweep cell the nav model checks on a full-block rise — and the
/// hanging lamps drop from the ceiling. (Every unlisted block, torches included,
/// is a full cube to the nav model, so "decorative" lighting placed at head height
/// would silently wall off a route.)
#[allow(clippy::too_many_arguments)]
pub fn light_room(
    g: &mut Grid,
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    walk: i32,
    ceil_solid: i32,
    spacing: i32,
    lamp: &str,
) {
    light_room_ex(
        g,
        x0,
        x1,
        z0,
        z1,
        walk,
        ceil_solid,
        spacing,
        lamp,
        &|_, _| false,
    )
}

/// [`light_room`] with an exclusion predicate on (x,z) — used to keep the ceiling
/// lattice out of stairwell columns, where a hanging lamp would land inside the
/// climb and wall the route off at head height.
#[allow(clippy::too_many_arguments)]
pub fn light_room_ex(
    g: &mut Grid,
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    walk: i32,
    ceil_solid: i32,
    spacing: i32,
    lamp: &str,
    skip: &dyn Fn(i32, i32) -> bool,
) {
    let y = walk + 3;
    let sconce = |g: &mut Grid, cell: [i32; 3], wall: [i32; 3], facing: &'static str| {
        if g.is_air(cell[0], cell[1], cell[2]) && g.is_solid(wall[0], wall[1], wall[2]) {
            g.blk(
                cell[0],
                cell[1],
                cell[2],
                "minecraft:wall_torch",
                Some(vec![("facing", facing)]),
            );
        }
    };
    let mut x = x0 + 1;
    while x <= x1 {
        if !skip(x, z0) {
            sconce(g, [x, y, z0], [x, y, z0 - 1], "south");
        }
        if !skip(x, z1) {
            sconce(g, [x, y, z1], [x, y, z1 + 1], "north");
        }
        x += spacing;
    }
    let mut z = z0 + 1;
    while z <= z1 {
        if !skip(x0, z) {
            sconce(g, [x0, y, z], [x0 - 1, y, z], "east");
        }
        if !skip(x1, z) {
            sconce(g, [x1, y, z], [x1 + 1, y, z], "west");
        }
        z += spacing;
    }
    let mut x = x0 + spacing / 2;
    while x <= x1 {
        let mut z = z0 + spacing / 2;
        while z <= z1 {
            if !skip(x, z) {
                chandelier(g, x, z, ceil_solid, (ceil_solid - walk - 4).max(1), lamp);
            }
            z += spacing;
        }
        x += spacing;
    }
}

pub fn stairs(g: &mut Grid, x: i32, y: i32, z: i32, kind: &str, facing: &'static str) {
    g.blk(
        x,
        y,
        z,
        kind,
        Some(vec![
            ("facing", facing),
            ("half", "bottom"),
            ("shape", "straight"),
            ("waterlogged", "false"),
        ]),
    );
}

/// The `facing` of a bottom-half stair at a cell, if one is there.
fn stair_facing(g: &Grid, x: i32, y: i32, z: i32) -> Option<&'static str> {
    match g.get(x, y, z) {
        Cell::Block(n, Some(props)) if n.ends_with("_stairs") => {
            props.iter().find(|(k, _)| *k == "facing").map(|(_, v)| *v)
        }
        _ => None,
    }
}

/// The two lateral offsets of a climb, i.e. the axis a flight is *wide* on.
fn flank_offsets(facing: &str) -> [(i32, i32); 2] {
    match facing {
        "north" | "south" => [(-1, 0), (1, 0)],
        _ => [(0, -1), (0, 1)],
    }
}

/// Seal every **lateral step-up onto a stair tread** with a newel course, so a
/// flight can only be entered at its foot.
///
/// The drowned-bell playtest lesson (2026-08-03), pinned as tooling rather than
/// prose: a stair block carries exactly ONE climb direction, but where a room
/// floor sits flush one block below a mid-flight tread, the nav model reads a
/// perfectly legal side-step onto that tread. The tread then serves two climbs
/// at once — the flight's, and the side-step's — and whichever `facing` it
/// takes, one of them is backwards. That is `DW0430` with no literal to blame:
/// the geometry, not the generator's facing, is over-determined. Closing the
/// flank removes the second climb and leaves the tread with the one job a stair
/// can actually do.
///
/// Deterministic (ADR-0006): the grid is scanned in fixed `x,y,z` order and the
/// whole fix set is collected before any of it is applied, so no seal can
/// change whether a later cell is detected.
pub fn seal_stair_flanks(g: &mut Grid, block: &str) -> Vec<[i32; 3]> {
    let [sx, sy, sz] = g.size;
    let mut open = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let Some(facing) = stair_facing(g, x, y, z) else {
                    continue;
                };
                for (dx, dz) in flank_offsets(facing) {
                    // The tread's walk plane is `y + 1`; a neighbour standable at
                    // `y` is exactly one below it, i.e. a legal lateral rise.
                    let n = [x + dx, y, z + dz];
                    if standable(g, n) {
                        open.push(n);
                    }
                }
            }
        }
    }
    for n in &open {
        g.blk(n[0], n[1], n[2], block, None);
    }
    if std::env::var("TK_DEBUG_STAIRS").is_ok() {
        eprintln!("seal_stair_flanks: {} cell(s) {open:?}", open.len());
    }
    open
}

/// Proof that `seal_stair_flanks` left nothing behind (and that no later pass
/// re-opened a flank): no stair tread anywhere in the piece is reachable by a
/// one-block rise from its side.
pub fn assert_stair_flanks_sealed(id: &str, g: &Grid) {
    let [sx, sy, sz] = g.size;
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let Some(facing) = stair_facing(g, x, y, z) else {
                    continue;
                };
                for (dx, dz) in flank_offsets(facing) {
                    let n = [x + dx, y, z + dz];
                    assert!(
                        !standable(g, n),
                        "{id}: stair tread [{x}, {y}, {z}] (facing={facing}) can be entered \
                         SIDEWAYS from {n:?} — a one-block lateral rise onto a tread whose climb \
                         runs the other way. A route that takes that side-step makes the tread \
                         carry two climbs at once and `DW0430` reports it with no wrong literal \
                         to blame. Close the flight's flank instead of turning the tread."
                    );
                }
            }
        }
    }
}
