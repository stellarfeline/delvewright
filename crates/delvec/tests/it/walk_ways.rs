//! **A staged walk follows the declared ways** over a planned site: an NPC's
//! `move-npc` between two places keeps to the roads the site plan declares
//! (`boxes[].way`) beside a shorter way across a place that is not one, and
//! still walks where no way joins its ends.
//!
//! The site, on open flat ground at grade 64, every place walled and every
//! connection an arch three wide:
//!
//! ```text
//!   x 2..6      7..16       17..21
//!   ┌─────┐┌────────────┐┌─────┐  z 2..6
//!   │home ├┤   road-a   ├┤     │
//!   └──┬──┘└────────────┘│     │
//!   ┌──┴──┐              │road │
//!   │green│  (13 long)   │ -b  │  z 7..19
//!   └──┬──┘              │     │
//!   ┌──┴──┐┌────────────┐│     │
//!   │mkt  ├┤   road-c   ├┤     │  z 20..24
//!   └─────┘└────────────┘└─────┘
//! ```
//!
//! The keeper walks from home to the market: straight through the green is
//! about eighteen cells, round the three roads about fifty.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-walk-ways-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn empty_library(root: &Path) -> PrefabRegistry {
    let dir = root.join("prefabs");
    std::fs::create_dir_all(&dir).unwrap();
    PrefabRegistry::load_dir(&dir).expect("an empty library loads")
}

/// The green's footprint, `[x0, x1, z0, z1]`.
const GREEN: [i32; 4] = [2, 6, 7, 19];

/// The three roads' footprints and their clear columns: `[x0, x1, z0, z1]`.
const ROADS: [(&str, [i32; 4], &str, [i32; 4]); 3] = [
    ("node/road-a", [7, 16, 2, 6], "x", [7, 16, 3, 5]),
    ("node/road-b", [17, 21, 2, 24], "z", [18, 20, 2, 24]),
    ("node/road-c", [7, 16, 20, 24], "x", [7, 16, 21, 23]),
];

fn place(node: &str, extent: [i32; 2], way: Option<&str>) -> serde_json::Value {
    let mut b = serde_json::json!({
        "node": node, "extent": extent,
        "floor": {"datum": "datum/grade"}, "ceiling": {"clearance": 4},
        "sides": {"east": "wall", "west": "wall", "south": "wall", "north": "wall"}
    });
    if let Some(axis) = way {
        b["way"] = serde_json::json!({"axis": axis, "clear": 3});
    }
    b
}

fn seam(edge: &str, face: &str, at: i32, meets: i32) -> serde_json::Value {
    serde_json::json!({
        "edge": edge, "face": face, "at": at, "meets": meets,
        "form": "a stone arch", "opening": {"width": 3, "height": 3}
    })
}

fn walk(id: &str, a: &str, b: &str) -> serde_json::Value {
    serde_json::json!({"id": id, "a": a, "b": b, "class": "walk"})
}

/// The campaign: the blockout fixture's cast and words over six places, the
/// keeper walking from home to the market once he is asked. `far_road_way`
/// says whether the far road declares itself a way.
fn campaign(dir: &Path, far_road_way: bool) {
    common::copy_dir_all(
        &common::repo_root().join("crates/delvec/tests/fixtures/blockout"),
        dir,
    );
    let v = delvewright_dsl::DSL_VERSION;
    let write = |name: &str, stage: &str, content: serde_json::Value| {
        let doc = serde_json::json!({
            "campaign_id": "blockout", "content": content, "dsl_version": v, "stage": stage,
        });
        std::fs::write(
            dir.join(name),
            delvewright_dsl::to_canonical_string(&doc).unwrap(),
        )
        .unwrap();
    };
    write(
        "layout-graph.json",
        "layout-graph",
        serde_json::json!({
            "nodes": [
                {"id": "node/home", "intent": "arrival"},
                {"id": "node/green", "intent": "hub"},
                {"id": "node/market", "intent": "goal-chamber"},
                {"id": "node/road-a", "intent": "service-way"},
                {"id": "node/road-b", "intent": "service-way"},
                {"id": "node/road-c", "intent": "service-way"}
            ],
            "edges": [
                walk("edge/home-green", "node/home", "node/green"),
                walk("edge/green-market", "node/green", "node/market"),
                walk("edge/home-road-a", "node/home", "node/road-a"),
                walk("edge/road-a-road-b", "node/road-a", "node/road-b"),
                walk("edge/road-c-road-b", "node/road-c", "node/road-b"),
                walk("edge/market-road-c", "node/market", "node/road-c")
            ],
            "entry": "node/home",
            "goal": "node/market",
            "critical_path": ["node/home", "node/green", "node/market"],
            "beats": [
                {"node": "node/home", "objective": "obj/ask", "quest": "quest/walk-the-whole"},
                {"node": "node/market", "objective": "obj/walk-out", "quest": "quest/walk-the-whole"}
            ]
        }),
    );
    let mut home = place("node/home", [5, 5], None);
    home["min"] = serde_json::json!([2, 2]);
    write(
        "site-plan.json",
        "site-plan",
        serde_json::json!({
            "region": {"min": [0, 56, 0], "extent": [24, 24, 28]},
            "datums": [{"id": "datum/grade", "y": 64}],
            "boxes": [
                home,
                place("node/green", [5, 13], None),
                place("node/market", [5, 5], None),
                place("node/road-a", [10, 5], Some("x")),
                place("node/road-b", [5, 23], far_road_way.then_some("z")),
                place("node/road-c", [10, 5], Some("x"))
            ],
            "seams": [
                seam("edge/home-green", "south", 1, 1),
                seam("edge/green-market", "south", 1, 1),
                seam("edge/home-road-a", "east", 1, 1),
                seam("edge/road-a-road-b", "east", 1, 1),
                seam("edge/road-c-road-b", "east", 1, 19),
                seam("edge/market-road-c", "east", 1, 1)
            ],
            "fill": {
                "kind": "open", "below": "minecraft:stone", "surface": "minecraft:grass_block",
                "terrain": {"kind": "flat", "datum": "datum/grade"}
            }
        }),
    );
    common::patch_file(&dir.join("npcs.json"), |v| {
        v["content"]["npcs"][0]["anchor"] = serde_json::json!("anchor/node-home");
    });
    common::patch_file(&dir.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        q["cast"]["npc/keeper"]["at"] = serde_json::json!("anchor/node-home");
        let objs = q["objectives"].as_array_mut().unwrap();
        objs.retain(|o| o["id"] != "obj/enter-the-cell");
        let out = objs.iter_mut().find(|o| o["id"] == "obj/walk-out").unwrap();
        out["after"] = serde_json::json!(["obj/ask"]);
        out["anchor"] = serde_json::json!("anchor/node-market");
        let ask = q["on_objective_complete"]["obj/ask"]
            .as_array_mut()
            .unwrap();
        ask.retain(|e| e["type"] != "open-gate");
        ask.push(serde_json::json!({
            "type": "move-npc", "npc": "npc/keeper", "to": {"anchor": "anchor/node-market"}
        }));
    });
}

/// The keeper's planned walk: its route cells and its per-tick positions.
fn keeper_walk(far_road_way: bool) -> (Vec<[i32; 3]>, Vec<[f64; 3]>) {
    let root = tempdir(if far_road_way { "joined" } else { "broken" });
    let dir = root.join("campaign");
    campaign(&dir, far_road_way);
    let c = common::campaign_at(&dir);
    let reg = empty_library(&root);
    let plan = Plan::build(&c, &reg).expect("the site plans");
    let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &BTreeMap::new());
    let battery = delvec::compiler::blockout::check(&plan, &blocks).expect("a blockout");
    let errors: Vec<_> = battery
        .findings
        .iter()
        .filter(|(_, d)| d.severity == delvewright_dsl::Severity::Error)
        .map(|(_, d)| format!("{}: {}", d.code, d.message))
        .collect();
    assert!(
        errors.is_empty(),
        "the site is not a legal blockout: {errors:#?}"
    );
    let world = delvec::compiler::nav::World::from_plan(&plan, &BTreeMap::new());
    let moves = delvec::compiler::nav::plan_moves(&plan, &world).expect("the walk routes");
    let m = moves
        .iter()
        .find(|m| m.npc == "npc/keeper")
        .expect("the keeper walks");
    (m.cells.clone(), m.waypoints.clone())
}

fn in_foot(foot: [i32; 4], x: i32, z: i32) -> bool {
    x >= foot[0] && x <= foot[1] && z >= foot[2] && z <= foot[3]
}

fn column(p: &[f64; 3]) -> (i32, i32) {
    (p[0].floor() as i32, p[2].floor() as i32)
}

/// **The walk keeps to the roads beside the shortcut through the green**:
/// no route cell and no per-tick position stands in the green, every route
/// cell outside home and the market is on a road's clear columns, and the
/// walk goes the whole way round.
#[test]
fn the_keeper_walks_the_roads_and_not_across_the_green() {
    let (cells, ticks) = keeper_walk(true);
    let in_green: Vec<_> = cells
        .iter()
        .filter(|c| in_foot(GREEN, c[0], c[2]))
        .collect();
    assert!(
        in_green.is_empty(),
        "the route crosses the green at {in_green:?}"
    );
    let off_road: Vec<_> = cells
        .iter()
        .filter(|c| {
            c[0] > 6
                && !ROADS
                    .iter()
                    .any(|(_, _, _, clear)| in_foot(*clear, c[0], c[2]))
        })
        .collect();
    assert!(
        off_road.is_empty(),
        "the route leaves the roads' clear width at {off_road:?}"
    );
    assert!(
        cells.iter().any(|c| in_foot(ROADS[1].1, c[0], c[2])),
        "the route never reaches the far road"
    );
    let drawn: Vec<_> = ticks
        .iter()
        .map(column)
        .filter(|(x, z)| in_foot(GREEN, *x, *z))
        .collect();
    assert!(
        drawn.is_empty(),
        "{} of {} per-tick position(s) are drawn in the green, first {:?}",
        drawn.len(),
        ticks.len(),
        drawn.first()
    );
}

/// **Where no way joins the ends, the walk still exists**: with the far road
/// a plain place, no way joins home to the market, and the keeper walks —
/// through the green, the shorter way.
#[test]
fn with_the_roads_broken_the_keeper_still_walks() {
    let (cells, _) = keeper_walk(false);
    assert!(cells.len() > 1, "the walk is a walk: {cells:?}");
    assert!(
        cells.iter().any(|c| in_foot(GREEN, c[0], c[2])),
        "the walk takes the green once no way joins its ends"
    );
}
