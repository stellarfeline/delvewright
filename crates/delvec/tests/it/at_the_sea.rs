//! A place at the sea meets the sea and its seabed as an open side meets open
//! ground: the ground under the sea is the ground the world holds, and the
//! open sea outside every claim is the commons (`DW0838`).
//!
//! The fixture is `shore-plan` (an `ocean` horizon, an open fill), its plan
//! edited per test. Its corners, read off `delvec validate`'s placing lines:
//! `node/landing` at `[4, 8]`, 8 × 8; `node/hall` at `[13, 4]`, 12 × 16;
//! `node/loft` at `[26, 8]`; `node/cell` at `[13, 21]`; `node/exit` at
//! `[4, 21]`, 8 × 8. The landing's south ring (z 16) and the exit's north
//! ring (z 20) face each other across three rows of the commons, z 17..19,
//! and no seam joins the two.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::blockout;
use delvec::compiler::blockstate::BlockState;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::Campaign;
use delvewright_dsl::siteplan::SitePlan;
use serde_json::{Value, json};

/// The shore fixture with its site plan edited: every place but the loft at a
/// walk plane of 63 (its floor course the sea's top course, 62), the terrain
/// flat at `seabed − 1`, and no massif between the sea floor and the grade.
fn at_the_sea(tag: &str, seabed: i64, edit: impl FnOnce(&mut Value)) -> Campaign {
    let dir = std::env::temp_dir().join(format!("dw-at-the-sea-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(
        &common::repo_root().join("crates/delvec/tests/fixtures/shore-plan"),
        &dir,
    );
    common::patch_file(&dir.join("site-plan.json"), |v| {
        let c = &mut v["content"];
        c["region"] = json!({"min": [0, 48, 0], "extent": [64, 40, 64]});
        c["datums"] = json!([
            {"id": "datum/grade", "y": 63},
            {"id": "datum/loft", "y": 69},
            {"id": "datum/seabed", "y": seabed},
        ]);
        c["fill"]["terrain"] = json!({"kind": "flat", "datum": "datum/seabed"});
        let volumes = c["volumes"].as_array_mut().expect("volumes");
        volumes.retain(|x| x["role"] != "massif");
        edit(v);
    });
    let c = common::campaign_at(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    c
}

fn library() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).expect("the prefab library loads")
}

fn place<'a>(v: &'a mut Value, node: &str) -> &'a mut Value {
    v["content"]["boxes"]
        .as_array_mut()
        .expect("boxes")
        .iter_mut()
        .find(|b| b["node"] == node)
        .expect("the place")
}

/// **The ground under the sea is the ground the world holds.** Over a terrain
/// drawn at y 50, four courses under the `ocean` superflat's sea floor: the
/// pinned generator holds stone to y 54 in every cell no write reaches, and
/// the fill writes ground, never water — so in every whole column the
/// highest solid cell of the world the server holds stands at
/// `Ground::top`, which reads the sea floor, and so does every place's
/// handed ground. Red before: `Ground::top` read 50 under a world holding
/// 54, and a place's handed ground followed it four courses under the
/// commons' seabed.
#[test]
fn the_ground_under_the_sea_is_the_ground_the_world_holds() {
    let c = at_the_sea("seabed", 51, |_| {});
    let sp = SitePlan::of(&c);
    let ground = &sp.ground;
    let plan = Plan::build(&c, &library()).expect("plans");
    let a = delvec::compiler::assembled::assemble(&plan, &BTreeMap::new());
    let held = |cell: [i32; 3]| -> bool {
        match a.blocks.get(&cell) {
            Some(b) => b.as_str() != "minecraft:air" && !b.as_str().starts_with("minecraft:water"),
            None => {
                !a.reached.contains(cell)
                    && delvec::compiler::plan::ocean_layer_at(cell[1])
                        .is_some_and(|b| b != "minecraft:water")
            }
        }
    };
    let shells: Vec<[i64; 4]> = sp
        .boxes
        .iter()
        .map(|b| [b.foot[0] - 1, b.foot[1] + 1, b.foot[2] - 1, b.foot[3] + 1])
        .collect();
    let (lo, hi) = ground.region();
    let mut compared = 0usize;
    for x in lo[0]..=hi[0] {
        for z in lo[2]..=hi[2] {
            if shells
                .iter()
                .any(|s| x >= s[0] && x <= s[1] && z >= s[2] && z <= s[3])
            {
                continue;
            }
            let top = ground.top(x, z).expect("a top");
            let built = (lo[1] - 8..=hi[1])
                .rev()
                .find(|y| held([x as i32, *y as i32, z as i32]))
                .expect("a column of ground");
            assert_eq!(built, top, "the seabed the world holds at [{x}, {z}]");
            compared += 1;
        }
    }
    assert!(compared > 3000, "whole columns compared: {compared}");
    assert_eq!(
        ground.top(40, 40),
        Some(i64::from(delvewright_dsl::OCEAN_SEA_FLOOR)),
        "the terrain under the sea floor reads the sea floor"
    );
    // Every place's handed ground meets the same seabed.
    for node in ["node/landing", "node/exit"] {
        let h = delvec::compiler::detail::allocation(&c, &delvewright_dsl::NodeId(node.into()))
            .expect("an allocation");
        let h = serde_json::to_value(&h).expect("serialises");
        let base = h["world_min"][1].as_i64().expect("world_min");
        for col in h["ground"]["columns"].as_array().expect("columns") {
            assert_eq!(
                col["top"].as_i64().unwrap() + base,
                i64::from(delvewright_dsl::OCEAN_SEA_FLOOR),
                "{node}'s handed ground at {col}"
            );
        }
        assert_eq!(
            h["ground"]["sea_y"].as_i64().map(|y| y + base),
            Some(i64::from(delvewright_dsl::OCEAN_SEA_LEVEL)),
            "{node} is handed the sea"
        );
    }
}

/// The battery over the shore at the sea: the landing's south side and the
/// exit's north side left open (`sheer`), each place's own basin of water
/// carried under its ring to the open sea — a piece's water, written over the
/// stand-in as a detailed place's piece writes it, the two places taken as
/// detailed. A body in the landing swims out under its ring, across the open
/// sea, and up into the exit.
fn swim_battery() -> blockout::Battery {
    let c = at_the_sea("swim", 57, |v| {
        place(v, "node/landing")["sides"]["south"] = json!("sheer");
        place(v, "node/exit")["sides"]["north"] = json!("sheer");
    });
    let mut plan = Plan::build(&c, &library()).expect("plans");
    let mut blocks = delvec::compiler::assembled::assembled_blocks(&plan, &BTreeMap::new());
    let water = BlockState::new("minecraft:water[level=0]");
    for x in 5..=10 {
        for y in 57..=62 {
            for z in (12..=16).chain(20..=24) {
                blocks.insert([x, y, z], water);
            }
        }
    }
    plan.blockout
        .as_mut()
        .expect("a blockout")
        .massed
        .retain(|n| n != "node/landing" && n != "node/exit");
    blockout::check(&plan, &blocks).expect("a site-plan campaign has a blockout")
}

fn dw0838(b: &blockout::Battery) -> Vec<String> {
    b.findings
        .iter()
        .filter(|(_, d)| d.code == "DW0838")
        .map(|(_, d)| d.message.clone())
        .collect()
}

/// **The open sea is the commons** (`DW0838`). Two places that both open onto
/// the sea are joined through it, as two that open onto open ground are: the
/// swim from the landing to the exit is no unallocated join, and the binding
/// counts the pair among those the commons joins and the water among the
/// commons' cells. Red before: the commons was standable ground alone, so the
/// flood that keeps off it swam across the sea and the pair was refused.
#[test]
fn two_places_open_to_the_sea_are_joined_through_the_commons() {
    let b = swim_battery();
    assert!(dw0838(&b).is_empty(), "{:#?}", dw0838(&b));
    assert!(
        b.binding.unclaimed_water > 1000,
        "the open sea is counted: {}",
        b.binding.line()
    );
    assert!(
        b.binding.commons_pairs >= 1,
        "the landing and the exit are joined through the commons: {}",
        b.binding.line()
    );
}
