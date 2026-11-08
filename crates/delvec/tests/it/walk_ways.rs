//! **A staged walk follows the declared ways** over a planned site: an NPC's
//! `move-npc` between two places keeps to the roads the site plan declares
//! (`boxes[].way`) beside a shorter way across a place that is not one, and
//! still walks where no way joins its ends.
//!
//! The site, on open flat ground at grade 64, every place walled and every
//! connection an arch three wide, one wall course between neighbours
//! (footprints as `delvec validate` places them):
//!
//! ```text
//!   x 2..12        14..21     23..27
//!   ┌──────────┐ ┌────────┐ ┌─────┐  z 2..6
//!   │home      ├─┤ road-a ├─┤     │
//!   └─┬────────┘ └────────┘ │     │
//!   ┌─┴─┐                   │road │
//!   │grn│ (13 long)         │ -b  │  z 8..20
//!   └─┬─┘                   │     │
//!   ┌─┴────────┐ ┌────────┐ │     │
//!   │market    ├─┤ road-c ├─┤     │  z 22..26
//!   └──────────┘ └────────┘ └─────┘
//! ```
//!
//! The keeper walks from home to the market: through the green is about
//! twenty-six cells, round the three roads about fifty. The green stands
//! clear of the roads by more than a sprint jump, so the site validates and
//! is a legal blockout (`DW0838` is asked of it before any walk is read).

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
    let mut home = place("node/home", [11, 5], None);
    home["min"] = serde_json::json!([2, 2]);
    write(
        "site-plan.json",
        "site-plan",
        serde_json::json!({
            "region": {"min": [0, 56, 0], "extent": [32, 24, 32]},
            "datums": [{"id": "datum/grade", "y": 64}],
            "boxes": [
                home,
                place("node/green", [5, 13], None),
                place("node/market", [11, 5], None),
                place("node/road-a", [8, 5], Some("x")),
                place("node/road-b", [5, 25], far_road_way.then_some("z")),
                place("node/road-c", [8, 5], Some("x"))
            ],
            "seams": [
                seam("edge/home-green", "south", 1, 1),
                seam("edge/green-market", "south", 1, 1),
                seam("edge/home-road-a", "east", 1, 1),
                seam("edge/road-a-road-b", "east", 1, 1),
                seam("edge/road-c-road-b", "east", 1, 21),
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

/// A place's footprint `[x0, x1, z0, z1]`, as the derivation placed it.
type Foot = [i32; 4];

/// What the walk is judged against, read off the planned site: the green's
/// footprint, and each road's footprint with its three clear columns — the
/// middle three of its five across, the declared `clear: 3` centred.
struct Site {
    green: Foot,
    roads: Vec<(Foot, Foot)>,
}

/// The keeper's planned walk over the site: the site, the route cells and
/// the per-tick positions.
fn keeper_walk(far_road_way: bool) -> (Site, Vec<[i32; 3]>, Vec<[f64; 3]>) {
    let root = tempdir(if far_road_way { "joined" } else { "broken" });
    let dir = root.join("campaign");
    campaign(&dir, far_road_way);
    let c = common::campaign_at(&dir);
    let reg = empty_library(&root);
    let refused: Vec<_> = common::validation_diagnostics(
        &c,
        &delvec::compiler::registry::FullItemRegistry::v1_21_11(),
        &reg,
        &delvec::compiler::registry::FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .filter(|d| d.severity == delvewright_dsl::Severity::Error)
    .map(|d| format!("{}: {}", d.code, d.message))
    .collect();
    assert!(
        refused.is_empty(),
        "the site does not validate: {refused:#?}"
    );
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
    let foot = |node: &str| -> Foot {
        let b = plan
            .blockout
            .as_ref()
            .expect("a blockout")
            .boxes
            .iter()
            .find(|b| b.node.0 == node)
            .expect("the place is placed");
        b.foot.map(|v| i32::try_from(v).unwrap())
    };
    let site = Site {
        green: foot("node/green"),
        roads: ["node/road-a", "node/road-b", "node/road-c"]
            .iter()
            .map(|n| {
                let f = foot(n);
                let clear = if *n == "node/road-b" {
                    [f[0] + 1, f[1] - 1, f[2], f[3]]
                } else {
                    [f[0], f[1], f[2] + 1, f[3] - 1]
                };
                (f, clear)
            })
            .collect(),
    };
    let world = delvec::compiler::nav::World::from_plan(&plan, &BTreeMap::new());
    let moves = delvec::compiler::nav::plan_moves(&plan, &world).expect("the walk routes");
    let m = moves
        .iter()
        .find(|m| m.npc == "npc/keeper")
        .expect("the keeper walks");
    (site, m.cells.clone(), m.waypoints.clone())
}

fn in_foot(foot: Foot, x: i32, z: i32) -> bool {
    x >= foot[0] && x <= foot[1] && z >= foot[2] && z <= foot[3]
}

fn column(p: &[f64; 3]) -> (i32, i32) {
    (p[0].floor() as i32, p[2].floor() as i32)
}

/// **The walk keeps to the roads beside the shortcut through the green**:
/// no route cell and no per-tick position stands in the green, every route
/// cell on a road is on its clear columns, and the walk goes the whole way
/// round.
#[test]
fn the_keeper_walks_the_roads_and_not_across_the_green() {
    let (site, cells, ticks) = keeper_walk(true);
    let in_green: Vec<_> = cells
        .iter()
        .filter(|c| in_foot(site.green, c[0], c[2]))
        .collect();
    assert!(
        in_green.is_empty(),
        "the route crosses the green at {in_green:?}"
    );
    // The far road is entered across its west column, which is outside its
    // clear width: the one cell of each crossing from a near road, in that
    // road's own clear rows, is the only off-way cell a road owes.
    let far = site.roads[1].0;
    let crossing = |c: &[i32; 3]| {
        c[0] == far[0]
            && [site.roads[0].1, site.roads[2].1]
                .iter()
                .any(|clear| c[2] >= clear[2] && c[2] <= clear[3])
    };
    let off_road: Vec<_> = cells
        .iter()
        .filter(|c| {
            site.roads.iter().any(|(f, _)| in_foot(*f, c[0], c[2]))
                && !site
                    .roads
                    .iter()
                    .any(|(_, clear)| in_foot(*clear, c[0], c[2]))
                && !crossing(c)
        })
        .collect();
    assert!(
        off_road.is_empty(),
        "the route leaves the roads' clear width at {off_road:?}"
    );
    let crossed = cells.iter().filter(|c| crossing(c)).count();
    assert_eq!(crossed, 2, "the far road is entered once and left once");
    assert!(
        cells.iter().any(|c| in_foot(site.roads[1].0, c[0], c[2])),
        "the route never reaches the far road"
    );
    let drawn: Vec<_> = ticks
        .iter()
        .map(column)
        .filter(|(x, z)| in_foot(site.green, *x, *z))
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
    let (site, cells, _) = keeper_walk(false);
    assert!(cells.len() > 1, "the walk is a walk: {cells:?}");
    assert!(
        cells.iter().any(|c| in_foot(site.green, c[0], c[2])),
        "the walk takes the green once no way joins its ends"
    );
}
