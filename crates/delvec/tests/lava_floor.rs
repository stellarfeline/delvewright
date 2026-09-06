//! Prefab-authored lava is not floor — the static half of the fluid-is-not-floor
//! defect, over the real `emit::build` path.
//!
//! The runtime half (a `fill-region` whose block is a fluid) is `DW0544`. This is
//! the other site: a fluid that arrives from a prefab palette rather than from a
//! verb. `occupancy_of` classified it by asking `is_water`, so `minecraft:lava`
//! fell through to the final `else` and became a full-cube solid — floor the
//! completability proof walks the party across.
//!
//! No prefab in the library contains lava, which is why it never surfaced and why
//! the fixture here is synthetic. The geometry is `boundary_assembled`'s box: a
//! closed stone room the size of `hello-room`, so hello-world's own anchors land
//! inside it (spawn `[5,65,2]`, the keeper `[5,65,4]`, the exit `[5,65,8]`), and
//! the party's only route from the keeper to the exit runs along the floor.
//!
//! One course of that floor — the `z == 7` row, the full width of the interior —
//! is the variable. Filled with stone the room is an ordinary hall; filled with
//! lava it is a hall with a moat, and a moat is not a bridge. Nothing else in the
//! two worlds differs, which is what makes the red a statement about the block.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};

/// The canonical hello-world campaign: one area bound to `prefab/hello-room`,
/// whose critical path is talk-to-the-keeper then reach-the-exit.
fn hello_world() -> Campaign {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    parse_campaign(&loaded.raw).expect("hello-world parses")
}

/// An 11×6×11 stone room, lit from inside, whose floor course at local `z == 7`
/// is made of `moat` instead of stone — with a stone sub-course beneath it so the
/// moat is a basin and not a hole. Both properties matter:
///
/// - **A basin, not a hole.** Without the course below, the columns under the moat
///   are bottomless and the void-boundary proof (`DW0322`) fires first on the same
///   geometry, which would make the red a statement about void rather than about
///   lava.
/// - **Sealed on every side.** The moat's cells are ringed by stone (the floor
///   fore and aft, the shell left and right, the sub-course below), so the flood
///   is exactly the nine authored cells and the two worlds differ only where they
///   are meant to.
fn room_with_moat(moat: &str) -> Vec<u8> {
    use fastnbt::Value;
    let (sx, sy, sz) = (11i32, 6i32, 11i32);
    let mut blocks: Vec<Value> = Vec::new();
    let mut push = |x: i32, y: i32, z: i32, state: i32| {
        let mut c = std::collections::HashMap::new();
        c.insert(
            "pos".to_string(),
            Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
        );
        c.insert("state".to_string(), Value::Int(state));
        blocks.push(Value::Compound(c));
    };
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1 {
                    // The moat replaces the floor course across the interior width.
                    let is_moat = y == 0 && z == 7 && (1..sx - 1).contains(&x);
                    push(x, y, z, if is_moat { 3 } else { 1 });
                }
            }
        }
    }
    // The sub-course under the moat: a floor for the basin, one below the room's.
    for x in 1..sx - 1 {
        push(x, -1, 7, 1);
    }
    // Interior glowstone: the lighting gate is not what this file is about.
    for x in [2, sx / 2, sx - 3] {
        for z in [2, sz / 2, sz - 3] {
            push(x, sy - 2, z, 2);
        }
    }
    let palette = Value::List(vec![
        pal_entry("minecraft:air"),
        pal_entry("minecraft:stone"),
        pal_entry("minecraft:glowstone"),
        pal_entry(moat),
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

fn pal_entry(name: &str) -> fastnbt::Value {
    let mut c = std::collections::HashMap::new();
    c.insert("Name".to_string(), fastnbt::Value::String(name.to_string()));
    fastnbt::Value::Compound(c)
}

/// Build hello-world through the real `emit::build` path against the synthetic
/// room, so every static proof the shipped engine runs sees this geometry.
fn build_with(moat: &str) -> Result<BuildOutput, BuildFailure> {
    let campaign = hello_world();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let nbt = room_with_moat(moat);
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.clone());
        }
    }
    emit::build(
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

/// The binding case: the party cannot walk across lava, so a lava moat across the
/// only route makes the delve incompletable and the build is refused.
///
/// Before the classification fix this campaign built **green**: the lava course
/// was `solid`, the cells above it were standable, and the critical-path walk
/// crossed the moat as if it were a paved floor.
#[test]
fn a_lava_moat_across_the_only_route_refuses_the_build() {
    match build_with("minecraft:lava") {
        // A `BuildOutput` here is the defect itself, and printing it prints a whole
        // datapack — say what happened instead.
        Ok(_) => panic!(
            "the build SUCCEEDED: the lava course was taken for floor and the \
             critical-path walk crossed the moat"
        ),
        Err(BuildFailure::Diagnostic { code, message }) => assert_eq!(
            code, "DW0311",
            "the critical path is what the moat breaks: {message}"
        ),
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
    }
}

/// The counter-case that keeps the red from being a statement about the geometry:
/// the same room, the same course, the same build path — one solid block instead
/// of a fluid — and the party walks across it.
#[test]
fn the_same_room_with_a_stone_course_builds() {
    build_with("minecraft:stone").expect("a stone floor is a floor");
}

/// And the same for water, which was already classified correctly: the two fluids
/// now reach `flooded` by one route, so this asserts the answer does not depend on
/// which fluid the prefab happens to hold.
#[test]
fn a_water_moat_refuses_the_build_identically() {
    match build_with("minecraft:water") {
        Ok(_) => panic!("the build SUCCEEDED over a water moat"),
        Err(BuildFailure::Diagnostic { code, message }) => assert_eq!(code, "DW0311", "{message}"),
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
    }
}
