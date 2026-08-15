//! Deterministic generator for the Delvewright **tidal-keep** tileset — the six
//! prefabs that stage "The Drowned Bell" (a souls campaign: barrow shore →
//! gatehouse → wall walk → courtyard/chapel hub → cistern undercroft → bell
//! tower). A sibling of `prefabs/island-terrain-generator` and
//! `prefabs/cave-generator`: its own `[workspace]`, outside `crates/`, so it never
//! enters the shipped `delvec` binary and no existing `.nbt` output moves
//! (ADR-0006).
//!
//! Convention: `tk:socket` — keep-socket-v1 geometry (3×3 opening, one jigsaw
//! block at the bottom-centre wall cell, `joint=aligned`, `final_state=air`) in a
//! `tk` vocabulary, at two datums: **`floor_y = 2`** on the shore (the island
//! convention's walk plane, local y=3) and **`floor_y = 10`** on the keep plinth
//! (walk plane local y=11). Every level change is authored INSIDE a piece — the
//! solver has no vertical socket, so a piece's rise is the difference between its
//! two sockets' local y, exactly as `keep-stair` does it.
//!
//! Determinism (ADR-0006): every stream is seeded from a per-piece PRNG + value
//! noise; no wall clock (gzip mtime pinned 0), no unseeded RNG, no hash-order
//! iteration, no absolute paths in output. Same seed → byte-identical `.nbt`.
//!
//! Usage: tidal-keep-gen <out_dir>

mod barrow;
mod belltower;
mod cistern;
mod common;
mod courtyard;
mod gatehouse;
mod wallwalk;

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

use common::*;

/// A socket declaration: which face, at which floor datum, centred where along it.
type Door = (Side, i32, i32);

struct Spec {
    id: &'static str,
    size: [i32; 3],
    doors: Vec<Door>,
    /// Only the pieces that author sea declare `waterline_y` — the barrow shore
    /// and (r5) the bell tower's ferry pier, both on the shore datum. Every
    /// other piece omits it so `DW0344` does not demand it land at sea level
    /// (the keep RISES).
    waterline_y: Option<i32>,
    build: fn(&mut Grid, u64),
    anchors: fn() -> Vec<(&'static str, AnchorJson)>,
    light: Light,
    salt: u64,
}

enum Light {
    /// Roofless: sky-lit, no static block-light estimate is meaningful.
    OpenAir,
    /// Enclosed: measure over these interior volumes.
    Measured(fn() -> Vec<[i32; 6]>, Option<&'static str>),
}

fn specs() -> Vec<Spec> {
    use Side::*;
    vec![
        Spec {
            id: "tk-barrow-field",
            size: [barrow::SX, barrow::SY, barrow::SZ],
            doors: vec![(North, SHORE_FLOOR_Y, 24)],
            waterline_y: Some(SHORE_FLOOR_Y),
            build: barrow::build,
            anchors: barrow::anchors,
            light: Light::OpenAir,
            salt: 1,
        },
        Spec {
            id: "tk-gatehouse",
            size: [gatehouse::SX, gatehouse::SY, gatehouse::SZ],
            doors: vec![(South, SHORE_FLOOR_Y, 14), (North, KEEP_FLOOR_Y, 14)],
            waterline_y: None,
            build: gatehouse::build,
            anchors: gatehouse::anchors,
            light: Light::Measured(gatehouse::light_regions, None),
            salt: 2,
        },
        Spec {
            id: "tk-wall-walk",
            size: [wallwalk::SX, wallwalk::SY, wallwalk::SZ],
            doors: vec![(South, KEEP_FLOOR_Y, 7), (North, KEEP_FLOOR_Y, 7)],
            waterline_y: None,
            build: wallwalk::build,
            anchors: wallwalk::anchors,
            light: Light::OpenAir,
            salt: 3,
        },
        Spec {
            id: "tk-courtyard-chapel",
            size: [courtyard::SX, courtyard::SY, courtyard::SZ],
            doors: vec![(South, KEEP_FLOOR_Y, 23), (East, KEEP_FLOOR_Y, 23)],
            waterline_y: None,
            build: courtyard::build,
            anchors: courtyard::anchors,
            light: Light::Measured(courtyard::light_regions, None),
            salt: 4,
        },
        Spec {
            id: "tk-cistern",
            size: [cistern::SX, cistern::SY, cistern::SZ],
            doors: vec![(West, KEEP_FLOOR_Y, 19), (East, KEEP_FLOOR_Y, 19)],
            waterline_y: None,
            build: cistern::build,
            anchors: cistern::anchors,
            light: Light::Measured(
                cistern::light_regions,
                Some(
                    "the drowned undercroft — lamplit at the causeways and the secret cell, gloomy \
                     over the flooded bays BY DESIGN (the TEST ambush behind the pillars reads as a \
                     shape before it reads as a warden). Declared `dim`, not `dark`: the measured \
                     floor minimum stays at or above the compiler's DARK_THRESHOLD, so the piece \
                     needs no night-vision grant and the campaign is free to leave the area \
                     undeclared and keep the gloom (spec-0010 mitigation hierarchy).",
                ),
            ),
            salt: 5,
        },
        Spec {
            id: "tk-bell-tower",
            size: [belltower::SX, belltower::SY, belltower::SZ],
            doors: vec![(West, KEEP_FLOOR_Y, 13)],
            waterline_y: Some(SHORE_FLOOR_Y),
            build: belltower::build,
            anchors: belltower::anchors,
            light: Light::Measured(belltower::light_regions, None),
            salt: 6,
        },
    ]
}

fn write_piece(out: &Path, spec: &Spec) {
    let seed = piece_seed(spec.id, spec.salt);
    let mut g = Grid::new(spec.size);
    (spec.build)(&mut g, seed);
    // Uniform over EVERY piece, including the four that have no stairs today: a
    // future flight that forgets to seal its flanks fails here rather than in a
    // playtest. The sealing itself belongs to the piece that authors the flight
    // (see `seal_stair_flanks`), so it runs before that piece's route proofs.
    assert_stair_flanks_sealed(spec.id, &g);
    assert_no_unsupported_gravity(spec.id, &g);
    if let Ok(probe) = std::env::var("TK_PROBE") {
        let v: Vec<i32> = probe
            .split(',')
            .filter_map(|t| t.trim().parse().ok())
            .collect();
        if v.len() == 4 && spec.salt == v[0] as u64 {
            for dy in -2..=4 {
                for dz in -2..=2 {
                    let mut row = String::new();
                    for dx in -3..=6 {
                        let c = [v[1] + dx, v[2] + dy, v[3] + dz];
                        let n = match g.get(c[0], c[1], c[2]) {
                            Cell::Air => "air".to_string(),
                            Cell::Jigsaw(_) => "jig".to_string(),
                            Cell::Block(b, _) => b.replace("minecraft:", ""),
                        };
                        row.push_str(&format!("{:>14}", &n[..n.len().min(14)]));
                    }
                    eprintln!("y{:+} z{:+} {row}", dy, dz);
                }
                eprintln!();
            }
        }
    }
    assert_anchors_sane(spec, &g);

    let (profile, min_light, method): (&'static str, i32, &'static str) = match &spec.light {
        Light::OpenAir => (
            "lit",
            15,
            "roofless piece: sky-lit by construction (a static block-light BFS is not applicable \
             to an open-air structure). Braziers and lanterns supplement after dusk; the compiler \
             re-measures the assembled world under its darkest reachable sky (spec-0010)",
        ),
        Light::Measured(regions, _) => {
            let m = (regions)()
                .into_iter()
                .map(|r| estimate_min_light(&g, r))
                .min()
                .unwrap_or(0);
            (
                classify(m),
                m,
                "static flood-fill block-light estimate over the standable cells of the declared \
                 interior volumes (block light only; sky light through belfry openings and the \
                 open yard is NOT counted). An authoring estimate, not a live probe — the \
                 compiler re-measures the assembled world (spec-0010)",
            )
        }
    };
    let rationale = match &spec.light {
        Light::Measured(_, r) => r.map(|s| s.to_string()),
        _ => None,
    };

    let mut structure = serialize(&g);
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
    invariants::assert_fluid_is_contained(spec.id, spec.size, &cells);
    let nbt = fastnbt::to_bytes(&structure).expect("nbt");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gz");
    let framed = gz.finish().expect("finish");
    std::fs::write(out.join(format!("{}.nbt", spec.id)), &framed).expect("write nbt");

    let connectors: Vec<ConnectorJson> = spec
        .doors
        .iter()
        .map(|&(side, fy, along)| ConnectorJson {
            name: SOCKET_NAME,
            target: SOCKET_NAME,
            local_pos: door_center(spec.size, side, fy, along),
            facing: side.facing().into(),
            opening: [3, 3],
            joint: "aligned",
        })
        .collect();

    let anchors: BTreeMap<String, AnchorJson> = (spec.anchors)()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
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
        waterline_y: spec.waterline_y,
        anchors,
        connectors,
        lighting: LightingJson {
            profile,
            measured_min_light: min_light,
            measured: MEASURED_DATE,
            rationale,
            method,
        },
        license: LicenseJson {
            source: "original",
            spdx: "GPL-3.0-or-later",
            note: "Original Delvewright project asset (pipeline-code license per \
                   prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            provenance: "Generated deterministically by prefabs/tidal-keep-generator \
                         (tidal-keep-gen), ADR-0006; regenerating yields byte-identical NBT.",
        },
    };
    let json = serde_json::to_string_pretty(&meta).expect("json") + "\n";
    std::fs::write(out.join(format!("{}.json", spec.id)), json).expect("write json");
    println!(
        "wrote {:<22} {:>3}x{:>3}x{:>3}  {:>8} nbt bytes  profile {:<4} min-light {:>2}  \
         {} sockets  {} anchors",
        spec.id,
        spec.size[0],
        spec.size[1],
        spec.size[2],
        framed.len(),
        profile,
        min_light,
        spec.doors.len(),
        (spec.anchors)().len(),
    );
}

/// Anchor hygiene, proved per piece rather than discovered at campaign-build:
/// every POINT anchor resolves to a standable cell (so `DW0316` / wave-spawn /
/// bonfire placement can never fail on prefab geometry), every anchor id is a
/// legal `anchor/<kebab>` (`DW0110`) or the one reserved `spawn` key, and every
/// trap marker's dispenser really is a dispenser (`DW0352`'s hardware is only
/// meaningful if it exists).
fn assert_anchors_sane(spec: &Spec, g: &Grid) {
    for (name, a) in (spec.anchors)() {
        let legal = name == "spawn"
            || name == "entry"
            || (name.starts_with("anchor/")
                && name["anchor/".len()..].split('-').all(|s| {
                    !s.is_empty()
                        && s.chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                }));
        assert!(
            legal,
            "{}: anchor id `{name}` is not `anchor/<kebab>`",
            spec.id
        );
        if let Some(p) = a.pos {
            match a.kind {
                AnchorKind::Footing => assert!(
                    standable(g, p),
                    "{}: anchor `{name}` at {p:?} is not standable (air at feet+head, solid floor \
                     below) — an unstandable anchor is a build failure waiting to happen",
                    spec.id
                ),
                // spec-0022: a firing slot is an opening, not a footing. Same
                // condition `DW0446` enforces, checked one layer earlier.
                AnchorKind::Slot => assert!(
                    passable(g, p) && g.name_at(p[0], p[1], p[2]) != Some("minecraft:water"),
                    "{}: volley slot `{name}` at {p:?} is solid or flooded — a projectile \
                     summoned there never leaves the block it spawned in (`DW0446`)",
                    spec.id
                ),
                // spec-0021: the compiler fills a container, never places one.
                AnchorKind::Container => assert!(
                    g.name_at(p[0], p[1], p[2])
                        .is_some_and(|n| FILLABLE.contains(&n)),
                    "{}: loot anchor `{name}` at {p:?} holds {:?}, not one of {FILLABLE:?} — \
                     `item replace block … container.<n>` fails SILENTLY against a \
                     non-container, so the delve would ship with an empty wall (`DW0431`)",
                    spec.id,
                    g.name_at(p[0], p[1], p[2])
                ),
            }
        }
        if let Some(d) = a.dispenser {
            assert_eq!(
                g.name_at(d[0], d[1], d[2]),
                Some("minecraft:dispenser"),
                "{}: trap anchor `{name}` declares a dispenser socket at {d:?} that is not a \
                 dispenser — `item replace block … container.0` fails SILENTLY on a non-container",
                spec.id
            );
        }
        if let Some(r) = &a.region {
            for i in 0..3 {
                assert!(
                    r.from[i] <= r.to[i] && r.to[i] < g.size[i],
                    "{}: anchor `{name}` region is malformed or out of bounds",
                    spec.id
                );
            }
        }
    }
}

/// The pool block the content repo's `pools.json` needs. Printed rather than
/// written, because every `*.json` in the prefab directory is parsed as prefab
/// metadata — a stray snippet file would be `DW0346`.
fn print_pool() {
    println!(
        "\n--- merge into <content-repo>/prefabs/pools.json under \"pools\" ---\n{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "pool/tidal-keep": {
                "members": [
                    { "prefab": "prefab/tk-barrow-field",     "weight": 1, "role": "entry" },
                    { "prefab": "prefab/tk-gatehouse",        "weight": 1, "role": "room" },
                    { "prefab": "prefab/tk-wall-walk",        "weight": 1, "role": "room" },
                    { "prefab": "prefab/tk-courtyard-chapel", "weight": 1, "role": "room" },
                    { "prefab": "prefab/tk-cistern",          "weight": 1, "role": "room" },
                    { "prefab": "prefab/tk-bell-tower",       "weight": 1, "role": "terminal" }
                ]
            }
        }))
        .unwrap()
    );
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: tidal-keep-gen <out_dir>");
    let out = Path::new(&out);
    std::fs::create_dir_all(out).expect("mkdir");
    let specs = specs();
    for spec in &specs {
        write_piece(out, spec);
    }
    println!("{} pieces", specs.len());
    print_pool();
}
