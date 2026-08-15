//! Boundary safety over the **assembled world** (`DW0322`) — the
//! stage-10 call site in `emit::build`, as opposed to the per-batch one inside
//! the stage-7 edit replay.
//!
//! Every other `DW0322` test drives a campaign that HAS an edit script and so
//! proves the per-batch call (`tests/edit.rs`). That is exactly the gap the
//! unconditional proof was written to close: a campaign that only places pieces
//! skips stage 8 entirely, and used to ship a reachable walkable cell one step
//! from a void drop with nothing having looked. These tests bind to a campaign
//! with **no `world-edits.json` at all**, so the only call site that can raise
//! the diagnostic is the one over the finished world.
//!
//! The hazard is geometry, not an edit: hello-world assembled against a
//! synthetic prefab whose outer wall is missing on one side, which is what a
//! stripped berm or an unclosed pool seam looks like to the compiler.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};

/// The canonical hello-world campaign: one prefab area, and — the point of this
/// file — **no stage-7 edit script**, so nothing here can reach the per-batch
/// boundary proof.
fn hello_world() -> Campaign {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    parse_campaign(&loaded.raw).expect("hello-world parses")
}

/// A synthetic gzipped structure `.nbt` for an `[sx,sy,sz]` stone box, lit from
/// inside so the darkness gate (`DW0210`) never pre-empts the boundary proof.
///
/// `open_x0` drops the whole `x == 0` slab — floor, wall and ceiling together —
/// which is what makes this a *boundary* fixture rather than a doorway one: the
/// column beside the interior floor is then bottomless, and a player standing on
/// that floor is one step from leaving the world.
fn box_nbt(size: [i32; 3], open_x0: bool) -> Vec<u8> {
    use fastnbt::Value;
    let [sx, sy, sz] = size;
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
        if open_x0 && x == 0 {
            continue;
        }
        for y in 0..sy {
            for z in 0..sz {
                if y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1 {
                    push(x, y, z, 1); // stone
                }
            }
        }
    }
    // Interior glowstone: the lighting gate is not what these tests are about.
    for x in [2, sx / 2, sx - 3] {
        for z in [2, sz / 2, sz - 3] {
            push(x, sy - 2, z, 2);
        }
    }
    let palette = Value::List(vec![
        pal_entry("minecraft:air"),
        pal_entry("minecraft:stone"),
        pal_entry("minecraft:glowstone"),
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

/// Build hello-world through the real `emit::build` path against a synthetic
/// structure. No edit script exists, so `edit_replay` is `None` and the ONLY
/// boundary proof that runs is the stage-10 one over the assembled world.
fn build_with_structure(campaign: &Campaign, nbt: Vec<u8>) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
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
        "unpinned",
        &BTreeMap::new(),
    )
}

/// The binding case: an edit-less campaign whose assembled geometry exposes
/// reachable floor to the void fails the build with `DW0322` at **error tier**
/// (a `BuildFailure::Diagnostic` aborts emission — exit 3 — as opposed to a
/// warning that would ship the hazard).
///
/// The reported edge is the room's own FLOOR (`y == 65`, one block above the
/// prefab's local floor at world `y == 64`), never the roof at `y == 70`. That
/// distinction is the fix this test is paired with: an anchor is seated inside
/// the piece that declares it, so the walk region this proof examines is the
/// space the party occupies. A roof-rooted region would flag the wrong cells and
/// would be unsatisfiable for any free-standing prefab in a void world.
#[test]
fn an_editless_campaign_with_a_walkable_edge_over_void_is_dw0322() {
    let c = hello_world();
    let err = build_with_structure(&c, box_nbt([11, 6, 11], true))
        .expect_err("an open-sided room is a walkable edge over the void");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a diagnostic failure");
    };
    assert_eq!(code, "DW0322", "{message}");
    assert!(message.contains("void drop"), "names the hazard: {message}");
    assert!(
        message.contains(", 65,"),
        "flags the room's floor, the cells the party walks: {message}"
    );
    assert!(
        !message.contains(", 70,"),
        "and never the roof, which no anchor seats on: {message}"
    );
}

/// The control that keeps the case above from being vacuously red: the SAME
/// campaign, the same synthetic prefab, the same stage-10 call site — with the
/// wall put back. A closed room has no boundary to breach, and the build is
/// clean. Without this, "the box fails" would prove nothing about the boundary.
#[test]
fn the_same_campaign_with_the_wall_intact_is_clean() {
    let c = hello_world();
    match build_with_structure(&c, box_nbt([11, 6, 11], false)) {
        Ok(_) => {}
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_ne!(code, "DW0322", "an enclosed room has a boundary: {message}");
        }
        Err(other) => panic!("unexpected {other:?}"),
    }
}
