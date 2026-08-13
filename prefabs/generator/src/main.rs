//! Deterministic generator for the Delvewright "stone keep" prefab tileset (M2).
//!
//! Emits, for each piece, a gzip-framed vanilla structure `.nbt` and a metadata
//! `.json` (anchors + connectors + lighting + license) into <out>/.
//! Byte-deterministic (ADR-0006): fixed iteration order, gzip mtime 0, fixed
//! compression. Mirrors the hello-room generator style; lives outside crates/.
//!
//! Usage: keep-prefab-gen <out_dir>

use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;
use std::path::Path;

/// Cross-tileset generator invariants, shared by source include so a lesson
/// learned in one tileset does not have to be re-learned in the other four
/// (the generators are separate Cargo workspaces on purpose).
#[path = "../../invariants.rs"]
mod invariants;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

const DATA_VERSION: i32 = 4671; // MC 1.21.11
const GENERATOR: &str = "prefabs/generator (keep-prefab-gen)";
const MEASURED_DATE: &str = "2026-07-30";

// Block ids used by the tileset.
const FLOOR: &str = "minecraft:stone_bricks";
const WALL: &str = "minecraft:stone_bricks";
const CEIL: &str = "minecraft:stone_bricks";
const ACCENT: &str = "minecraft:chiseled_stone_bricks";
const LIGHT: &str = "minecraft:glowstone"; // embedded ceiling source, light 15
const GATE: &str = "minecraft:iron_bars";

#[derive(Clone, Copy, PartialEq)]
enum Side {
    North, // z = 0, faces -z
    South, // z = max, faces +z
    West,  // x = 0, faces -x
    East,  // x = max, faces +x
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
    fn idx(&mut self, name: &str, props: Option<Vec<(&str, &str)>>) -> i32 {
        let written = props
            .map(|v| {
                v.into_iter()
                    .map(|(k, val)| (k.to_string(), val.to_string()))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        // Every palette entry states every property the block has. Vanilla's
        // BlockState codec would fill an unwritten one from the block's default
        // state, so this changes no BlockState and decides no content — it stops
        // the file meaning something only a running 1.21.11 server can work out.
        // `invariants::assert_states_are_complete` is the post-condition.
        let full = invariants::complete_state(name, &written);
        let e = PaletteEntry {
            name: name.to_string(),
            properties: if full.is_empty() { None } else { Some(full) },
        };
        if let Some(i) = self.entries.iter().position(|x| *x == e) {
            return i as i32;
        }
        self.entries.push(e);
        (self.entries.len() - 1) as i32
    }
}

/// A cell override placed after the shell is computed.
#[derive(Clone)]
enum Cell {
    Block(&'static str, Option<Vec<(&'static str, &'static str)>>),
    Jigsaw(&'static str), // orientation
    Air,
}

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
    anchors: BTreeMap<String, AnchorJson>,
    connectors: Vec<ConnectorJson>,
    lighting: LightingJson,
    license: LicenseJson,
}

/// Declarative piece spec.
struct Spec {
    id: &'static str,
    size: [i32; 3],
    doors: Vec<Side>,
    lights: Vec<[i32; 3]>,         // glowstone cells (usually ceiling y = Y-1)
    extras: Vec<([i32; 3], Cell)>, // gate bars, accents, etc.
    anchors: Vec<(&'static str, AnchorJson)>,
}

/// Measured floor-light minimums: min block-light over all walkable (air) floor
/// cells, probed live on a pinned 1.21.11 server with the piece's doorways SEALED
/// (sky-light = 0, so pure block light — the conservative per-piece value; a
/// connected neighbour can only add light). Method recorded in each metadata JSON.
fn measured_min(id: &str) -> i32 {
    match id {
        "keep-spawn-hall" => 10,
        "keep-corridor-straight" => 9,
        "keep-corridor-corner" => 8,
        "keep-corridor-tee" => 8,
        "keep-room-small-a" => 8,
        "keep-room-small-b" => 9,
        "keep-room-small-c" => 9,
        "keep-gate-room" => 9,
        "keep-shrine" => 10,
        "keep-boss-hall" => 8,
        "keep-alcove" => 10,
        "keep-cross" => 8,
        "keep-stair" => 8,
        _ => 0,
    }
}

/// The jigsaw (wall) cell of a doorway on `side` whose floor sits at `floor_y`
/// (bottom-centre of the 3×3 opening = `[.., floor_y + 1, ..]`). Floor-level doors
/// pass `floor_y = 0`; a stair's raised door passes its higher floor.
fn door_center_at(size: [i32; 3], side: Side, floor_y: i32) -> [i32; 3] {
    let (x, z) = (size[0], size[2]);
    let y = floor_y + 1;
    match side {
        Side::North => [x / 2, y, 0],
        Side::South => [x / 2, y, z - 1],
        Side::West => [0, y, z / 2],
        Side::East => [x - 1, y, z / 2],
    }
}

/// Cells (3 wide x 3 tall) that make up a doorway on `side` at `floor_y`, plus the
/// jigsaw cell.
fn doorway_cells_at(size: [i32; 3], side: Side, floor_y: i32) -> (Vec<[i32; 3]>, [i32; 3]) {
    let jc = door_center_at(size, side, floor_y);
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

fn door_center(size: [i32; 3], side: Side) -> [i32; 3] {
    door_center_at(size, side, 0)
}

/// Cells (3 wide x 3 tall) that make up a floor-level doorway on `side`, plus the
/// jigsaw cell.
fn doorway_cells(size: [i32; 3], side: Side) -> (Vec<[i32; 3]>, [i32; 3]) {
    doorway_cells_at(size, side, 0)
}

/// Raised (non-floor-level) doorways of a piece, keyed by id: `(side, floor_y)`.
/// Only the vertical stair has one (its far socket sits +4 y). Every other piece
/// returns empty, so their output is unchanged.
fn raised_doors(id: &str) -> Vec<(Side, i32)> {
    match id {
        "keep-stair" => vec![(Side::North, 4)],
        _ => vec![],
    }
}

/// Top solid-block height of the stair's climbing floor at interior `z` (the low
/// door is on the south, z = max; the run climbs +1 per z northward to the +4 high
/// landing). Stand level = surface + 1, so it rises from y=1 (low) to y=5 (high).
fn stair_surface(z: i32) -> i32 {
    match z {
        9 => 0,
        8 => 1,
        7 => 2,
        6 => 3,
        _ => 4, // z <= 5: the high landing
    }
}

fn build(spec: &Spec) -> Structure {
    let [sx, sy, sz] = spec.size;
    let mut pal = Palette::new();
    // Reserve index 0 = air (nice-to-have, not required).
    let air = pal.idx("minecraft:air", None);
    let _ = air;

    // Overlay map: coordinate -> Cell.
    let mut overlay: HashMap<[i32; 3], Cell> = HashMap::new();
    // Doorways (air) + jigsaws.
    let mut connectors: Vec<([i32; 3], Side)> = vec![];
    for &side in &spec.doors {
        let (cells, jc) = doorway_cells(spec.size, side);
        for c in cells {
            overlay.entry(c).or_insert(Cell::Air);
        }
        overlay.insert(jc, Cell::Jigsaw(side.orientation()));
        connectors.push((jc, side));
    }
    // Raised doorways (stair far socket): carve the opening at its higher floor.
    for (side, floor_y) in raised_doors(spec.id) {
        let (cells, jc) = doorway_cells_at(spec.size, side, floor_y);
        for c in cells {
            overlay.entry(c).or_insert(Cell::Air);
        }
        overlay.insert(jc, Cell::Jigsaw(side.orientation()));
        connectors.push((jc, side));
    }
    // Lights.
    for &p in &spec.lights {
        overlay.insert(p, Cell::Block(LIGHT, None));
    }
    // Extras (highest precedence).
    for (p, c) in &spec.extras {
        overlay.insert(*p, c.clone());
    }

    let mut blocks = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let p = [x, y, z];
                let state = if let Some(cell) = overlay.get(&p) {
                    match cell {
                        Cell::Air => pal.idx("minecraft:air", None),
                        Cell::Block(b, props) => pal.idx(b, props.clone()),
                        Cell::Jigsaw(o) => {
                            pal.idx("minecraft:jigsaw", Some(vec![("orientation", o)]))
                        }
                    }
                } else {
                    let shell =
                        y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                    if y == 0 {
                        pal.idx(FLOOR, None)
                    } else if y == sy - 1 {
                        pal.idx(CEIL, None)
                    } else if shell {
                        pal.idx(WALL, None)
                    } else {
                        pal.idx("minecraft:air", None)
                    }
                };
                let nbt = if let Some(Cell::Jigsaw(_)) = overlay.get(&p) {
                    Some(JigsawBE {
                        id: "minecraft:jigsaw".into(),
                        name: "keep:socket".into(),
                        target: "keep:socket".into(),
                        pool: "keep:pool".into(),
                        final_state: "minecraft:air".into(),
                        joint: "aligned".into(),
                    })
                } else {
                    None
                };
                blocks.push(BlockEntry { pos: p, state, nbt });
            }
        }
    }
    Structure {
        data_version: DATA_VERSION,
        size: spec.size,
        palette: pal.entries,
        blocks,
        entities: vec![],
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

fn write_piece(out: &Path, spec: &Spec) {
    let s = build(spec);
    let cells = invariant_cells(&s);
    invariants::assert_distress_never_stacks(spec.id, &cells);
    // Spelling, at the emitter: an unknown block id loads as AIR.
    invariants::assert_blocks_are_real(spec.id, &cells);
    // Completeness, at the emitter: a state that omits a property means whatever
    // a 1.21.11 server would fill in, and only a 1.21.11 server can read that.
    invariants::assert_states_are_complete(spec.id, &cells);
    let nbt = fastnbt::to_bytes(&s).expect("nbt");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gz");
    let framed = gz.finish().expect("finish");
    std::fs::write(out.join(format!("{}.nbt", spec.id)), &framed).expect("write nbt");

    // Metadata.
    let mut connectors: Vec<ConnectorJson> = spec
        .doors
        .iter()
        .map(|&side| {
            let (_, jc) = doorway_cells(spec.size, side);
            ConnectorJson {
                name: "keep:socket",
                target: "keep:socket",
                local_pos: jc,
                facing: side.facing().into(),
                opening: [3, 3],
                joint: "aligned",
            }
        })
        .collect();
    for (side, floor_y) in raised_doors(spec.id) {
        let (_, jc) = doorway_cells_at(spec.size, side, floor_y);
        connectors.push(ConnectorJson {
            name: "keep:socket",
            target: "keep:socket",
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
        anchors,
        connectors,
        lighting: LightingJson {
            profile: "lit",
            measured_min_light: measured_min(spec.id),
            measured: MEASURED_DATE,
            method: "live 1.21.11 probe: min over walkable floor cells of location_check light predicate; sealed time, roofed piece so sky-light=0",
        },
        license: LicenseJson {
            source: "original",
            spdx: "GPL-3.0-or-later",
            note: "Original Delvewright project asset (pipeline-code license per prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            provenance: "Generated deterministically by prefabs/generator (keep-prefab-gen), ADR-0006; regenerating yields byte-identical NBT.",
        },
    };
    let json = serde_json::to_string_pretty(&meta).expect("json") + "\n";
    std::fs::write(out.join(format!("{}.json", spec.id)), json).expect("write json");
    println!("wrote {} ({} nbt bytes)", spec.id, framed.len());
}

// Convenience constructors for anchors.
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

/// Ceiling glowstone grid: place a source at ceiling (y=Y-1) at each (x,z) in the
/// interior on a step-4 lattice, guaranteeing floor light >= 7 (verified live).
fn ceiling_grid(size: [i32; 3]) -> Vec<[i32; 3]> {
    let [sx, sy, sz] = size;
    let mut v = vec![];
    let xs = axis_points(sx);
    let zs = axis_points(sz);
    for &x in &xs {
        for &z in &zs {
            v.push([x, sy - 1, z]);
        }
    }
    v
}
/// Interior lattice points along an axis of length n (walls at 0 and n-1):
/// centers spaced <=6 apart so a ceiling source covers the floor to >=7.
fn axis_points(n: i32) -> Vec<i32> {
    let inner_lo = 1;
    let inner_hi = n - 2;
    if inner_hi < inner_lo {
        return vec![n / 2];
    }
    let span = inner_hi - inner_lo;
    // number of segments so spacing <= 6
    let segs = (span / 6).max(0) + 1;
    let mut pts = vec![];
    for k in 0..=segs {
        let x = inner_lo + (span * k) / segs;
        pts.push(x);
    }
    pts.dedup();
    pts
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: keep-prefab-gen <out_dir>");
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("mkdir");

    let mut specs: Vec<Spec> = vec![];

    // 1. spawn hall 9x5x9, door S. spawn anchor center facing the door.
    specs.push(Spec {
        id: "keep-spawn-hall",
        size: [9, 5, 9],
        doors: vec![Side::South],
        lights: ceiling_grid([9, 5, 9]),
        extras: vec![
            ([1, 1, 1], Cell::Block(ACCENT, None)),
            ([7, 1, 1], Cell::Block(ACCENT, None)),
        ],
        anchors: vec![
            // `spawn` faces the exit door so the arriving player looks toward
            // where they will go. `anchor/exit` faces NORTH (back toward the
            // spawn) so that when an NPC stands here it greets players arriving
            // from the spawn instead of turning its back on them (M2 facing fix).
            ("spawn", a_pos([4, 1, 4], Some("south"))),
            ("anchor/exit", a_pos([4, 1, 8], Some("north"))),
        ],
    });

    // 2. straight corridor 5x5x7, doors N/S.
    specs.push(Spec {
        id: "keep-corridor-straight",
        size: [5, 5, 7],
        doors: vec![Side::North, Side::South],
        lights: vec![[2, 4, 3]],
        extras: vec![],
        anchors: vec![],
    });

    // 3. corner corridor 7x5x7, doors N + E.
    specs.push(Spec {
        id: "keep-corridor-corner",
        size: [7, 5, 7],
        doors: vec![Side::North, Side::East],
        lights: vec![[3, 4, 3]],
        extras: vec![],
        anchors: vec![],
    });

    // 4. tee corridor 7x5x7, doors N + E + W.
    specs.push(Spec {
        id: "keep-corridor-tee",
        size: [7, 5, 7],
        doors: vec![Side::North, Side::East, Side::West],
        lights: vec![[3, 4, 3]],
        extras: vec![],
        anchors: vec![],
    });

    // 5. small room A 7x5x7, door N. npc stand.
    specs.push(Spec {
        id: "keep-room-small-a",
        size: [7, 5, 7],
        doors: vec![Side::North],
        lights: ceiling_grid([7, 5, 7]),
        extras: vec![([5, 1, 5], Cell::Block(ACCENT, None))],
        // `anchor/chest` (v0.3 collect target) sits centred, facing the single north
        // entry door so arrivals see it head-on. `anchor/npc-stand` gets its OWN
        // clear-floor cell (west wall, mid-room), off the x=3 door→chest walking
        // line and clear of the accent — it used to collide with the chest cell,
        // which made an NPC + chest share one block and the anchor unusable (gap 12).
        anchors: vec![
            ("anchor/npc-stand", a_pos([1, 1, 3], Some("east"))),
            ("anchor/chest", a_pos([3, 1, 4], Some("north"))),
        ],
    });

    // 6. small room B 7x5x9, doors N/S. npc stand.
    specs.push(Spec {
        id: "keep-room-small-b",
        size: [7, 5, 9],
        doors: vec![Side::North, Side::South],
        lights: ceiling_grid([7, 5, 9]),
        extras: vec![
            ([1, 1, 4], Cell::Block(ACCENT, None)),
            ([5, 1, 4], Cell::Block(ACCENT, None)),
        ],
        // `anchor/wave` (v0.3 wave spawn) is centred, facing the north entry door.
        // `anchor/npc-stand` gets its own clear-floor cell (west wall, near the north
        // door), off the x=3 north↔south through-passage and clear of the accents —
        // it used to collide with the wave-spawn cell, making the anchor unusable
        // (gap 12).
        anchors: vec![
            ("anchor/npc-stand", a_pos([1, 1, 2], Some("east"))),
            ("anchor/wave", a_pos([3, 1, 4], Some("north"))),
        ],
    });

    // 7. small room C 9x5x7, doors N + E. npc stand.
    specs.push(Spec {
        id: "keep-room-small-c",
        size: [9, 5, 7],
        doors: vec![Side::North, Side::East],
        lights: ceiling_grid([9, 5, 7]),
        extras: vec![([4, 1, 5], Cell::Block(ACCENT, None))],
        // `anchor/door` (v0.3 interact target) faces the east entry door.
        // `anchor/npc-stand` gets its own clear-floor cell (north wall, east side),
        // off the x=4 north-door passage and the z=3 east-door passage and clear of
        // the accent — it used to collide with the door/interact cell, making the
        // anchor unusable (gap 12).
        anchors: vec![
            ("anchor/npc-stand", a_pos([6, 1, 1], Some("south"))),
            ("anchor/door", a_pos([2, 1, 3], Some("east"))),
        ],
    });

    // 8. gate room 7x5x9, doors N/S, iron-bars gate at z=4.
    //
    // The gate must seal the passage WALL-TO-WALL (M2 fix): the room interior
    // spans x=1..=5, so bars only across the 3-wide doorway (x=2..4) left the
    // floor at x=1 and x=5 open and the owner walked around the gate. The barred
    // row now fills the whole interior width (x=1..=5). The OPENABLE region
    // (`anchor/gate`) stays the 3-wide central passage (x=2..4) that a player
    // actually walks through; the x=1 and x=5 columns are permanent bar flanks
    // that keep the wall sealed after the gate opens. Iron bars are non-occluding
    // (light passes through), so the sealed-doorway floor-light probe is unchanged.
    let gate_extras: Vec<([i32; 3], Cell)> = (1..=5)
        .flat_map(|x| (1..=3).map(move |y| ([x, y, 4], Cell::Block(GATE, None))))
        .collect();
    specs.push(Spec {
        id: "keep-gate-room",
        size: [7, 5, 9],
        doors: vec![Side::North, Side::South],
        lights: ceiling_grid([7, 5, 9]),
        extras: gate_extras,
        anchors: vec![
            ("anchor/gate", a_region([2, 1, 4], [4, 3, 4], GATE)),
            // Keeper faces NORTH — toward the north entry door players arrive
            // through (he stands on the north side of the gate). Previously faced
            // south (into the sealed gate), turning his back on arrivals.
            ("anchor/keeper-stand", a_pos([3, 1, 2], Some("north"))),
        ],
    });

    // 9. shrine / objective room 9x5x9, door N. objective anchor center.
    specs.push(Spec {
        id: "keep-shrine",
        size: [9, 5, 9],
        doors: vec![Side::North],
        lights: ceiling_grid([9, 5, 9]),
        extras: vec![
            ([4, 1, 6], Cell::Block(ACCENT, None)),
            ([3, 1, 6], Cell::Block(ACCENT, None)),
            ([5, 1, 6], Cell::Block(ACCENT, None)),
        ],
        anchors: vec![("anchor/objective", a_pos([4, 1, 6], Some("north")))],
    });

    // 10. boss / finale hall 11x5x13, door N. boss + objective anchors.
    specs.push(Spec {
        id: "keep-boss-hall",
        size: [11, 5, 13],
        doors: vec![Side::North],
        lights: ceiling_grid([11, 5, 13]),
        extras: vec![
            ([1, 1, 1], Cell::Block(ACCENT, None)),
            ([9, 1, 1], Cell::Block(ACCENT, None)),
            ([1, 1, 11], Cell::Block(ACCENT, None)),
            ([9, 1, 11], Cell::Block(ACCENT, None)),
        ],
        anchors: vec![
            ("anchor/boss", a_pos([5, 1, 9], Some("north"))),
            ("anchor/objective", a_pos([5, 1, 11], Some("north"))),
        ],
    });

    // 11. dead-end alcove 5x5x5, door N. decorative.
    specs.push(Spec {
        id: "keep-alcove",
        size: [5, 5, 5],
        doors: vec![Side::North],
        lights: vec![[2, 4, 2]],
        extras: vec![([2, 1, 3], Cell::Block(ACCENT, None))],
        anchors: vec![],
    });

    // 12. cross junction 7x5x7, doors N/S/E/W.
    specs.push(Spec {
        id: "keep-cross",
        size: [7, 5, 7],
        doors: vec![Side::North, Side::South, Side::East, Side::West],
        lights: vec![[3, 4, 3]],
        extras: vec![],
        anchors: vec![],
    });

    // 13. vertical stair connector 5x9x11: low door South (floor 0), high door
    //     North (floor 4) — a +4 elevation rise between its two sockets, so mating
    //     it lifts the layout one level. Usable up OR down by orientation (the
    //     mating rule picks which socket meets the parent). The climb is a
    //     continuous run of real `stone_brick_stairs`, 3 wide, one step per z, with
    //     solid stone-brick fill beneath each step for support; glowstone is
    //     embedded in the side walls at head height along the run so every floor
    //     cell clears the `lit` bar.
    //
    // Stair facing (M2 round-2 fix 2): the player ascends NORTH (from the south low
    // door toward the north high door), so the stairs face **north** — a vanilla
    // stair's raised half-step sits on its `facing` side, so `facing = the
    // direction you ascend toward`. The prior `facing:"south"` put the raised step
    // on the downhill side, presenting a full-block riser to a north-bound climber
    // (the owner's "climbs on full blocks / wrong-facing stairs"). z=5..8 are the
    // four ascending stair steps (top y 1→4); z<=4 is the flat high landing (solid
    // to y4, stand y5); z=9 is the flat low threshold (stand y1). Verified live by
    // rendering the regenerated piece.
    let stair_props = || {
        Some(vec![
            ("facing", "north"),
            ("half", "bottom"),
            ("shape", "straight"),
            ("waterlogged", "false"),
        ])
    };
    let mut stair_extras: Vec<([i32; 3], Cell)> = vec![];
    for z in 1..=9 {
        let top = stair_surface(z);
        for x in 1..=3 {
            for y in 1..top {
                stair_extras.push(([x, y, z], Cell::Block(FLOOR, None)));
            }
            if top >= 1 {
                let cell = if (5..=8).contains(&z) {
                    Cell::Block("minecraft:stone_brick_stairs", stair_props())
                } else {
                    Cell::Block(FLOOR, None)
                };
                stair_extras.push(([x, top, z], cell));
            }
        }
    }
    for &z in &[2, 4, 6, 8] {
        let s = stair_surface(z);
        for &x in &[0, 4] {
            stair_extras.push(([x, s + 2, z], Cell::Block(LIGHT, None)));
        }
    }
    specs.push(Spec {
        id: "keep-stair",
        size: [5, 9, 11],
        doors: vec![Side::South],
        lights: vec![],
        extras: stair_extras,
        anchors: vec![],
    });

    for spec in &specs {
        write_piece(out, spec);
    }
    println!("{} pieces", specs.len());
}
