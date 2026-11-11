//! Water in the route proof (spec-0134 §10), over the real `emit::build` path.
//!
//! The fixture is `lava_floor`'s: hello-world's anchors (spawn `[5,65,2]`, the
//! keeper `[5,65,4]`, the exit `[5,65,8]`) inside a closed stone room the size
//! of `hello-room`, so the party's only route from the keeper to the exit runs
//! along the floor. What varies is a moat across the full interior width of
//! that floor, between the keeper and the exit:
//!
//! - **a pool**: the floor course at local `z == 7` is water over a stone
//!   sub-course, level with the floor either side — a body steps down into it,
//!   swims one cell and climbs out over the one-block rim;
//! - **a walled pool**: the pool one course nearer (`z == 6`), its far rim a
//!   wall two blocks high — no swimmer climbs out of the water onto it.
//!
//! The dry walk crosses neither: a body does not stand on water. A leg under a
//! ceiling, dived and its submerged run stated, is `nav::route`'s unit test
//! (`a_pool_under_a_ceiling_states_its_submerged_run_in_the_water_ledger`):
//! every cell of this room lies inside the piece's box, and the box has no
//! room under its floor for water two deep.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, Diagnostic, parse_campaign};

#[derive(Clone, Copy)]
enum Moat {
    Pool,
    Walled,
}

fn hello_world() -> Campaign {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    parse_campaign(&loaded.raw).expect("hello-world parses")
}

/// Palette indices: air 0, stone 1, glowstone 2, water 3.
fn room(moat: Moat) -> Vec<u8> {
    room_of(moat, "minecraft:water")
}

/// [`room`] with `fluid` as the moat's block.
fn room_of(moat: Moat, fluid: &str) -> Vec<u8> {
    use fastnbt::Value;
    let (sx, sy, sz) = (11i32, 6i32, 11i32);
    let mut cells: BTreeMap<(i32, i32, i32), i32> = BTreeMap::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1 {
                    cells.insert((x, y, z), 1);
                }
            }
        }
    }
    let inner = 1..sx - 1;
    match moat {
        Moat::Pool => {
            for x in inner.clone() {
                cells.insert((x, 0, 7), 3);
                cells.insert((x, -1, 7), 1);
            }
        }
        Moat::Walled => {
            for x in inner.clone() {
                cells.insert((x, 0, 6), 3);
                cells.insert((x, -1, 6), 1);
                cells.insert((x, 1, 7), 1);
                cells.insert((x, 2, 7), 1);
            }
        }
    }
    for x in [2, sx / 2, sx - 3] {
        for z in [2, sz / 2, sz - 3] {
            cells.insert((x, sy - 2, z), 2);
        }
    }
    let mut blocks: Vec<Value> = Vec::new();
    for ((x, y, z), state) in cells {
        let mut c = std::collections::HashMap::new();
        c.insert(
            "pos".to_string(),
            Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
        );
        c.insert("state".to_string(), Value::Int(state));
        blocks.push(Value::Compound(c));
    }
    let palette = Value::List(vec![
        pal_entry("minecraft:air"),
        pal_entry("minecraft:stone"),
        pal_entry("minecraft:glowstone"),
        pal_entry(fluid),
    ]);
    let mut root = std::collections::HashMap::new();
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert(
        "size".to_string(),
        Value::List(vec![Value::Int(sx), Value::Int(sy), Value::Int(sz)]),
    );
    root.insert("palette".to_string(), palette);
    root.insert("blocks".to_string(), Value::List(blocks));
    root.insert("entities".to_string(), Value::List(vec![]));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gz, &raw).unwrap();
    gz.finish().unwrap()
}

/// One palette entry of the synthetic room. Every id the room is made of is
/// judged against the pinned registry before a byte is written: an id the
/// version lacks would load as air and quietly remove the cells it fills.
fn pal_entry(name: &str) -> fastnbt::Value {
    let registry = delvewright_dsl::blocks::BlockRegistry::v1_21_11();
    let verdict = registry.validate(name, &std::collections::BTreeMap::new());
    assert!(
        verdict.is_ok(),
        "the room's `{name}` is not a block: {verdict:?}"
    );
    let mut c = std::collections::HashMap::new();
    c.insert("Name".to_string(), fastnbt::Value::String(name.to_string()));
    fastnbt::Value::Compound(c)
}

fn build_with(moat: Moat) -> Result<(BuildOutput, Vec<Diagnostic>), BuildFailure> {
    let campaign = hello_world();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let nbt = room(moat);
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.clone());
        }
    }
    emit::build_with_warnings(
        &plan,
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn waypoints(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("validation/critical-path-waypoints.json")
            .expect("the build writes the waypoints"),
    )
    .unwrap()
}

/// The legs that swim, each with its `water` cells.
fn swimming_legs(doc: &serde_json::Value) -> Vec<serde_json::Value> {
    doc["legs"]
        .as_array()
        .expect("legs")
        .iter()
        .filter(|l| l.get("water").is_some())
        .cloned()
        .collect()
}

/// **A leg across a pool the walk cannot go round is proven**, and its
/// export carries the water it swims: every water cell is one of the leg's
/// waypoints (so the export self-check, `DW0314`, accepted each as afloat),
/// and lies in the moat's course.
#[test]
fn a_leg_across_a_pool_is_proven_and_exports_its_water() {
    let (out, warnings) = build_with(Moat::Pool).expect("the pool is swum");
    let doc = waypoints(&out);
    let legs = swimming_legs(&doc);
    assert_eq!(legs.len(), 1, "one leg crosses the moat: {doc}");
    let leg = &legs[0];
    let water = leg["water"].as_array().unwrap();
    assert!(!water.is_empty());
    let wps = leg["waypoints"].as_array().unwrap();
    for c in water {
        assert!(wps.contains(c), "water cell {c} is a waypoint of the leg");
    }
    // The swim entry was read to price the leg, and `DW0813` names it.
    let notice = warnings
        .iter()
        .find(|d| d.code == "DW0813")
        .expect("a swimming leg names the provisional swim entry");
    assert!(
        notice.message.contains("pacing.ticks-per-block-swimming"),
        "{}",
        notice.message
    );
}

/// The perturbation: the same pool, the leg's world dry — no `water` list on
/// any leg, and no `DW0813` notice for the swim entry.
#[test]
fn a_dry_room_swims_nothing_and_reads_no_swim_entry() {
    let campaign = hello_world();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let nbt = room_dry();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.clone());
        }
    }
    let (out, warnings) = emit::build_with_warnings(
        &plan,
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("the dry room builds");
    assert!(swimming_legs(&waypoints(&out)).is_empty());
    assert!(
        !warnings
            .iter()
            .any(|d| d.code == "DW0813" && d.message.contains("pacing.ticks-per-block-swimming")),
        "a route that swims nothing reads no swim entry"
    );
}

/// The pool's room with its moat course stone: every byte the same but the
/// moat's palette entry.
fn room_dry() -> Vec<u8> {
    room_of(Moat::Pool, "minecraft:stone")
}

/// **A pool walled on its far side is not crossed**: the swimmer cannot climb
/// out over a rim two blocks above the water, so the leg is refused (`DW0311`),
/// as the route proof refused every moat before water was reachable.
/// (`DW0322`'s swimmer stranding judges the ambient sea, under the reach graph,
/// and is unchanged by this spec; its tests are `nav::horizon`'s.)
#[test]
fn a_pool_walled_on_its_far_side_is_not_crossed() {
    match build_with(Moat::Walled) {
        Ok(_) => panic!("the build SUCCEEDED: a swimmer climbed a two-block wall out of the pool"),
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0311", "the leg is what the wall breaks: {message}")
        }
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
    }
}
