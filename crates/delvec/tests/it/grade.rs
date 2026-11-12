//! spec-0138 over a built world: the assembled ground is the graded ground,
//! a declared-open side over a drop stands open, and a campaign with nothing
//! to grade emits what it emitted.
//!
//! The fixture is the blockout campaign (`tests/fixtures/blockout`): an open
//! fill, flat, its surface at y 63, every place walled. Its corners, read off
//! `delvec validate`'s placing lines: `node/loft` at `[26, 8]`, 8 × 8, floor 69
//! (its surface 68, five over the terrain); `node/tunnel` at `[22, 21]`, 4 × 8,
//! floor 59 (its surface 58, five under). Each test edits the plan by one or
//! two declarations and builds it.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit;
use delvec::compiler::nav::width;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::Campaign;
use delvewright_dsl::siteplan::{Ground, SitePlan};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn fixture_dir() -> std::path::PathBuf {
    common::repo_root().join("crates/delvec/tests/fixtures/blockout")
}

/// The blockout fixture with its site plan edited.
fn campaign_with(edit: impl FnOnce(&mut Value)) -> Campaign {
    let mut loaded = delvec::compiler::load::load_campaign_dir(&fixture_dir())
        .expect("the blockout fixture is readable");
    let mut plan: Value =
        serde_json::from_str(loaded.raw.site_plan.as_deref().expect("a plan")).expect("parses");
    edit(&mut plan);
    loaded.raw.site_plan = Some(plan.to_string());
    delvewright_dsl::parse_campaign(&loaded.raw).expect("the edited fixture parses")
}

fn boxes(v: &mut Value) -> &mut Vec<Value> {
    v["content"]["boxes"].as_array_mut().expect("boxes")
}

fn place<'a>(v: &'a mut Value, node: &str) -> &'a mut Value {
    boxes(v)
        .iter_mut()
        .find(|b| b["node"] == node)
        .expect("the place")
}

/// The loft's east side and the tunnel's south side declared `open`: a
/// raised and a lowered graded side.
fn two_open_sides(v: &mut Value) {
    place(v, "node/loft")["sides"]["east"] = json!("open");
    place(v, "node/tunnel")["sides"]["south"] = json!("open");
}

fn library() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).expect("the prefab library loads")
}

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    common::plan_structures_with_trap_triggers(plan, &common::prefabs_dir())
}

// ---------------------------------------------------------------- criterion 7

/// In every whole column no volume covers, the assembled world's highest
/// terrain block stands at `Ground::top`; and every consumer that reads the
/// ground — the site plan's ground, the daylight survey's (`Ground::of`), the
/// trees' — reads that one height.
#[test]
fn the_built_ground_is_the_graded_ground() {
    let plans: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
        ("two open sides", Box::new(two_open_sides)),
        (
            "the loft steep",
            Box::new(|v: &mut Value| {
                two_open_sides(v);
                place(v, "node/loft")["grade"] = json!({"rise": 5, "run": 1});
            }),
        ),
        (
            "the loft unwalled round",
            Box::new(|v: &mut Value| {
                two_open_sides(v);
                let loft = place(v, "node/loft");
                loft["sides"] = json!({"west": "wall"});
            }),
        ),
    ];
    let reg = library();
    for (name, edit) in plans {
        let c = campaign_with(edit);
        let sp = SitePlan::of(&c);
        let ground = &sp.ground;
        let grade = ground.grade().expect("a plan grades");
        assert!(!grade.regraded.is_empty(), "{name}: something is regraded");
        let other = Ground::of(&c);
        let plan = Plan::build(&c, &reg).expect("plans");
        let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures(&plan));
        let vols: Vec<([i64; 3], [i64; 3])> = c
            .site_plan
            .as_ref()
            .unwrap()
            .content
            .volumes
            .iter()
            .map(|v| (v.region.min, v.region.max()))
            .collect();
        let (lo, hi) = ground.region();
        let shells: Vec<[i64; 4]> = sp
            .boxes
            .iter()
            .map(|b| [b.foot[0] - 1, b.foot[1] + 1, b.foot[2] - 1, b.foot[3] + 1])
            .collect();
        let (mut compared, mut moved) = (0usize, 0usize);
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                if shells
                    .iter()
                    .any(|s| x >= s[0] && x <= s[1] && z >= s[2] && z <= s[3])
                {
                    continue;
                }
                let top = ground.top(x, z).expect("a top");
                if vols.iter().any(|(a, b)| {
                    x >= a[0] && x <= b[0] && z >= a[2] && z <= b[2] && a[1] <= top + 1 && b[1] >= top - 1
                }) {
                    continue;
                }
                assert_eq!(other.top(x, z), Some(top), "{name}: Ground::of agrees at [{x}, {z}]");
                let at = |y: i64| blocks.get(&[x as i32, y as i32, z as i32]).map(|b| b.as_str());
                let built = (lo[1]..=hi[1])
                    .rev()
                    .find(|y| {
                        at(*y).is_some_and(|b| {
                            b != "minecraft:air" && !b.contains("leaves") && !b.contains("log")
                        })
                    })
                    .expect("a column of ground");
                assert_eq!(built, top, "{name}: the built surface at [{x}, {z}]");
                compared += 1;
                if grade.regraded_at(x, z).is_some() {
                    moved += 1;
                }
            }
        }
        assert!(compared > 2000, "{name}: columns compared {compared}");
        assert_eq!(moved, grade.regraded.len(), "{name}: every regraded column compared");
    }
}

// ---------------------------------------------------------------- criterion 9

/// A side declared `open` over a drop stands open: the grade lays its
/// shoulder, so `DW1065` is silent; a clearance over the raised side's
/// shoulder, or a massif over the lowered side's, brings it back for that
/// side alone.
#[test]
fn a_declared_open_side_over_a_drop_stands_open() {
    let reg = library();
    let judge = |c: &Campaign| {
        let plan = Plan::build(c, &reg).expect("plans");
        let assembled = delvec::compiler::assembled::assemble(&plan, &structures(&plan));
        let (b, refused) = width::check(&plan, &reg, &assembled, &[]);
        let shut: Vec<String> = refused
            .iter()
            .filter(|f| f.code.id() == "DW1065")
            .map(|f| f.message.clone())
            .collect();
        (b.to_json(), shut)
    };
    let c = campaign_with(two_open_sides);
    let (j, shut) = judge(&c);
    assert_eq!(j["open_sides"], json!(2), "two open-side ways judged");
    assert!(shut.is_empty(), "both stand open: {shut:?}");
    // The loft's shoulder: x 35, z 8..15, its surface at y 68.
    let c = campaign_with(|v| {
        two_open_sides(v);
        v["content"]["volumes"].as_array_mut().unwrap().push(json!({
            "id": "volume/no-shoulder", "role": "clearance",
            "region": {"min": [35, 60, 8], "extent": [1, 9, 8]}
        }));
    });
    let (_, shut) = judge(&c);
    assert_eq!(shut.len(), 1, "{shut:?}");
    assert!(shut[0].contains("loft"), "the loft's side alone: {}", shut[0]);
    // The tunnel's shoulder: z 30, x 22..25, its surface at y 58.
    let c = campaign_with(|v| {
        two_open_sides(v);
        v["content"]["volumes"].as_array_mut().unwrap().push(json!({
            "id": "volume/fill-the-shoulder", "role": "massif",
            "region": {"min": [22, 59, 30], "extent": [4, 4, 1]}
        }));
    });
    let (_, shut) = judge(&c);
    assert_eq!(shut.len(), 1, "{shut:?}");
    assert!(shut[0].contains("tunnel"), "the tunnel's side alone: {}", shut[0]);
}

// ---------------------------------------------------------------- criterion 10

/// SHA-256 over every emitted file but the grade's own ledger, path and bytes.
fn digest(out: &BTreeMap<String, Vec<u8>>) -> String {
    let mut h = Sha256::new();
    for (path, bytes) in out {
        if path == "validation/grade.json" {
            continue;
        }
        h.update(path.as_bytes());
        h.update([0]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn emitted(c: &Campaign) -> BTreeMap<String, Vec<u8>> {
    let reg = library();
    let plan = Plan::build(c, &reg).expect("plans");
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures(&plan),
        &CommandTree::v1_21_11(),
        &reg,
        None,
        &BTreeMap::new(),
    )
    .expect("builds")
}

/// Four campaigns with nothing to grade emit exactly the bytes the base
/// revision emitted (the digests were measured there, by this same
/// function), and their ledger says `C = 0`.
#[test]
fn a_campaign_with_nothing_to_grade_emits_what_it_emitted() {
    let no_plan = {
        let loaded = delvec::compiler::load::load_campaign_dir(
            &common::repo_root().join("crates/delvec/tests/fixtures/and-join"),
        )
        .expect("readable");
        delvewright_dsl::parse_campaign(&loaded.raw).expect("parses")
    };
    let solid = campaign_with(|v| {
        v["content"]["fill"] = json!({"kind": "solid", "block": "minecraft:stone"});
    });
    let walled = campaign_with(|_| {});
    let level = campaign_with(|v| {
        // The exit stands at the grade: its unwalled sides meet the terrain
        // at its own height.
        place(v, "node/exit")["sides"] = json!({"east": "wall"});
    });
    let cases = [
        ("no site plan", &no_plan, BASE_NO_PLAN),
        ("a solid fill", &solid, BASE_SOLID),
        ("every side walled", &walled, BASE_WALLED),
        ("unwalled at the terrain's height", &level, BASE_LEVEL),
    ];
    let mut bound = 0;
    for (name, c, want) in cases {
        let out = emitted(c);
        assert_eq!(digest(&out), want, "{name}: the bytes the base revision emitted");
        if c.site_plan.is_some() {
            let ledger: Value =
                serde_json::from_slice(out.get("validation/grade.json").expect("the ledger"))
                    .expect("parses");
            assert_eq!(ledger["regraded_columns"], json!(0), "{name}: C = 0");
        }
        bound += 1;
    }
    assert_eq!(bound, 4, "4 of 4 fixture campaigns");
}

const BASE_NO_PLAN: &str = "MEASURE";
const BASE_SOLID: &str = "MEASURE";
const BASE_WALLED: &str = "MEASURE";
const BASE_LEVEL: &str = "MEASURE";
