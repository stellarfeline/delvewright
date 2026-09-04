//! Interactions that existed on **no branch** — and so could be tested from
//! none of them.
//!
//! Three changes landed on the compiler at once: a zone past the vanilla
//! 48-per-axis cap is now placed as one piece out of several templates
//! (`DW0803`); fluid that leaves the built volume is a finding under a void
//! horizon (`DW0318`); and the set of `delve-admit` commands that owe a lone
//! tile a refusal is discovered from the parser (`DW0739`). Each was correct
//! and tested alone. Each is also a claim about an object one of the others
//! touches, and the claims only meet here:
//!
//! - **Two of them now have an opinion about where a piece ends.** Tiling made
//!   a piece's blocks arrive in several files; the fluid proof asks whether a
//!   cell is inside "every placed piece". If those two ever disagreed, a tiled
//!   zone's own water would read as having escaped the world.
//! - **The fluid proof must run before the boundary proof**, and the tiling
//!   change inserted a third gate into the same function. Nothing but source
//!   order held that constraint, and source order is what a merge moves — so
//!   the sequence moved inside the proof and this pins that it stayed there.
//! - **The door enumeration walks the parser; the tiling change altered what a
//!   door does with a malformed manifest.** The enumeration hands every door a
//!   lone tile. Nobody hands them a manifest that lies.
//!
//! The first two are below. The third is in the admit crate's
//! `tests/fragment_doors.rs`, beside the enumeration it has to read from rather
//! than restate — a door list copied into a second crate is the same defect the
//! enumeration exists to end.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_compiler::{emit, nav};
use delvewright_dsl::parse_campaign;

/// A scratch dir of this test's own, named for the case.
fn scratch(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-pairs-{case}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The corridor's anchors, in whole-zone coordinates; `exit` is past the cut.
fn corridor_anchors() -> serde_json::Value {
    serde_json::json!({
        "spawn": { "pos": [4, 1, 2], "facing": "south" },
        "anchor/keeper-stand": { "pos": [4, 1, 4], "facing": "north" },
        "anchor/door": {
            "region": { "from": [4, 1, 30], "to": [4, 3, 30] },
            "block": "minecraft:iron_bars"
        },
        "anchor/exit": { "pos": [4, 1, 55] }
    })
}

/// Plan a campaign and read every structure file it names.
fn plan_and_structures(
    campaign_dir: &Path,
    prefabs_dir: &Path,
) -> (
    &'static delvewright_dsl::Campaign,
    Plan<'static>,
    PrefabRegistry,
    BTreeMap<String, Vec<u8>>,
    &'static delvewright_compiler::load::LoadedCampaign,
) {
    let loaded = Box::leak(Box::new(load_campaign_dir(campaign_dir).unwrap()));
    let campaign = Box::leak(Box::new(parse_campaign(&loaded.raw).unwrap()));
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for t in plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .flat_map(|p| &p.templates)
    {
        let bytes = std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap();
        structures.insert(t.structure_file.clone(), bytes);
    }
    (campaign, plan, prefabs, structures, loaded)
}

// ---------------------------------------------------------------------------
// Pair 1 — where a piece ends
// ---------------------------------------------------------------------------

/// **A tiled zone's own water has not escaped anywhere.**
///
/// `nav::built_volume` is the fluid proof's answer to "where does the content
/// end", and it reads `PiecePlacement::bbox()`. The tiling change made a piece's
/// blocks arrive in several templates while deliberately keeping `size` — and
/// therefore `bbox()` — the whole zone's. That is the load-bearing agreement
/// between the two features, and it is invisible: both branches pass every test
/// they own whether it holds or not.
///
/// So this puts water in a tiled corridor **past the cut** and builds under the
/// void horizon, where escaped fluid is fatal. If the built volume ever became
/// one tile — the first, the largest, the one whose metadata was read — every
/// one of those cells would read as water that ran out of the world, and the
/// build would fail with `DW0318` pointing at a pond that is indoors.
#[test]
fn a_tiled_zones_own_water_is_inside_the_built_volume() {
    let root = scratch("tiled-water");
    let prefabs_dir = root.join("prefabs");

    // A sealed basin in the ceiling of the SECOND tile: stone floor at y=2 and
    // a rim, water at y=3. Past the cut at z=48, and clear of the walk plane at
    // y=1 so the corridor stays passable and this test stays about the water.
    let mut extra: Vec<([i32; 3], &str)> = Vec::new();
    for z in 49..55 {
        for x in 1..4 {
            extra.push(([x, 2, z], "minecraft:stone"));
            extra.push(([x, 3, z], "minecraft:water"));
        }
    }
    for z in 49..55 {
        extra.push(([4, 3, z], "minecraft:stone"));
    }
    for x in 1..5 {
        extra.push(([x, 3, 48], "minecraft:stone"));
        extra.push(([x, 3, 55], "minecraft:stone"));
    }
    common::write_tiled_zone(&prefabs_dir, "tiled-cistern", corridor_anchors(), &extra);

    let campaign_path = common::campaign_bound_to(&root.join("campaign"), "tiled-cistern");
    let (_c, plan, prefabs, structures, loaded) = plan_and_structures(&campaign_path, &prefabs_dir);

    // The agreement, stated directly: one piece, and its built extent is the
    // union of its templates' extents rather than any one of them.
    let built = nav::built_volume(&plan);
    assert_eq!(
        built.len(),
        1,
        "a tiled zone is ONE piece in the built volume"
    );
    let (_, (min, max)) = &built[0];
    let piece = &plan.areas[0].pieces[0];
    assert_eq!(piece.templates.len(), 2, "the fixture is genuinely tiled");
    for t in &piece.templates {
        for axis in 0..3 {
            assert!(
                t.pos[axis] >= min[axis] && t.pos[axis] + t.size[axis] - 1 <= max[axis],
                "template {} runs outside the built volume on axis {axis}: \
                 {:?}+{:?} not within {min:?}..{max:?}",
                t.structure_file,
                t.pos,
                t.size
            );
        }
    }
    assert_eq!(
        max[2] - min[2] + 1,
        common::TILED_ZONE_SIZE[2],
        "the built volume spans the whole zone, not one tile"
    );

    // And the proof itself agrees, over the real occupancy flood.
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("a tiled zone whose water is indoors builds under the void horizon");

    let ledger: serde_json::Value =
        serde_json::from_slice(&out["validation/fluid-escape.json"]).unwrap();
    assert_eq!(
        ledger["horizon"], "void",
        "the default horizon is the strict one"
    );
    assert_eq!(ledger["verdict"], "pass");
    assert_eq!(
        ledger["pieces_examined"], 1,
        "the fluid proof counts PIECES, not templates: {ledger}"
    );
    // A binding count that can disagree with reality: the fixture authored
    // water, so a zero here would mean the test proved nothing.
    let fluid = ledger["fluid_cells_examined"].as_u64().unwrap();
    assert!(fluid > 0, "the fixture must actually hold water: {ledger}");
    assert_eq!(
        ledger["cells_outside_built_volume"], 0,
        "a tiled zone's own water is inside its own zone: {ledger}"
    );
}

// ---------------------------------------------------------------------------
// Pair 2 — the order of two proofs, and why it is no longer an order
// ---------------------------------------------------------------------------

/// **A world whose water is running out of it never gets a boundary verdict.**
///
/// The boundary proof's per-column fall-arrest scan counts a flooded cell as
/// arrest, so a bottomless column with a waterfall in it reads as *supported*
/// and the proof goes quiet on exactly the columns the water escaped through.
/// Escaping fluid is a false premise of that proof, the way an unsettled gravity
/// block is of everything downstream of `DW0313`.
///
/// The constraint arrived held by the ORDER OF TWO STATEMENTS at each of two
/// call sites plus a comment saying why — and this merge inserted a third gate
/// into one of those functions, which is exactly the edit that moves one. Source
/// order is a checklist item, and `CLAUDE.md` is explicit that a checklist is not
/// a mechanism.
///
/// It cannot be pinned by a fixture either, and the reason is worth stating
/// because it is what makes the structural fix necessary rather than tidy: under
/// a void horizon the flood model has no floor to stop it, so escaped water
/// reaches essentially every column and masks essentially every boundary hit. A
/// world with both defects therefore reports `DW0318` under EITHER order — the
/// first attempt at this test built one and it could not tell them apart. There
/// is no fixture that can.
///
/// So the sequence moved inside `verify_boundary_safety`, and what this test
/// binds is that it stayed there: the leaky world below has no boundary verdict
/// to be had, through the entry point every caller uses.
#[test]
fn a_leaking_world_gets_no_boundary_verdict_through_the_entry_point() {
    let root = scratch("order");
    let prefabs_dir = root.join("prefabs");

    // A plate a body can stand on, and a shelf holding water over nothing. The
    // water falls past the bottom of the piece and out of the built volume.
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 2..9 {
        for z in 2..9 {
            cells.push(([x, 0, z], "minecraft:stone"));
        }
    }
    for x in 0..2 {
        for z in 0..2 {
            cells.push(([x, 6, z], "minecraft:stone"));
            cells.push(([x, 7, z], "minecraft:water"));
        }
    }
    common::write_single_prefab(
        &prefabs_dir,
        "leaky-plate",
        [9, 8, 9],
        &cells,
        serde_json::json!({
            "spawn": { "pos": [5, 1, 5], "facing": "south" },
            "anchor/keeper-stand": { "pos": [5, 1, 6], "facing": "north" },
            "anchor/door": {
                "region": { "from": [4, 1, 4], "to": [4, 1, 4] },
                "block": "minecraft:iron_bars"
            },
            "anchor/exit": { "pos": [6, 1, 6] }
        }),
    );

    let campaign_path = common::campaign_bound_to(&root.join("campaign"), "leaky-plate");
    let (_c, plan, prefabs, structures, loaded) = plan_and_structures(&campaign_path, &prefabs_dir);
    let world = nav::World::from_plan(&plan, &structures);

    // The leak is real, and it is the binding this test rests on.
    let escape = nav::measure_fluid_escape(&world);
    assert!(
        escape.finding().is_some(),
        "the fixture must leak, or this test proves nothing: {} fluid cell(s), {} outside",
        escape.fluid_cells,
        escape.outside.len()
    );

    // The entry point every caller uses reports the leak rather than a boundary
    // verdict. If the sequence ever moves back out to the call sites, this is
    // `Ok(())` — the masking — and the assertion names what that means.
    let err = nav::verify_boundary_safety(
        &world,
        &delvewright_compiler::edit::anchor_starts(&plan),
    )
    .expect_err(
        "a world the water is still running out of must not yield a boundary verdict at all: \
             an `Ok` here is the masking, not a pass",
    );
    assert_eq!(err.code, "DW0318");

    // …and the build agrees, which is the same fact one layer out.
    let err = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect_err("a leaking world does not build");
    let emit::BuildFailure::Diagnostic { code, .. } = err else {
        panic!("expected a diagnostic");
    };
    assert_eq!(code, "DW0318");
}
