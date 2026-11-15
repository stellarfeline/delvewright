//! spec-0138 over a built world: the assembled ground is the graded ground,
//! a declared-open side over a drop stands open, and a campaign with nothing
//! to grade moves no byte by its grade.
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

/// One named edit of the fixture's plan.
type Edit = Box<dyn Fn(&mut Value)>;

// ---------------------------------------------------------------- criterion 7

/// In every whole column no volume covers, the assembled world's highest
/// terrain block stands at `Ground::top`; and every consumer that reads the
/// ground — the site plan's ground, the daylight survey's (`Ground::of`), the
/// trees' — reads that one height.
#[test]
fn the_built_ground_is_the_graded_ground() {
    let plans: Vec<(&str, Edit)> = vec![
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
                    x >= a[0]
                        && x <= b[0]
                        && z >= a[2]
                        && z <= b[2]
                        && a[1] <= top + 1
                        && b[1] >= top - 1
                }) {
                    continue;
                }
                assert_eq!(
                    other.top(x, z),
                    Some(top),
                    "{name}: Ground::of agrees at [{x}, {z}]"
                );
                let at = |y: i64| {
                    blocks
                        .get(&[x as i32, y as i32, z as i32])
                        .map(|b| b.as_str())
                };
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
        let open_to_compare = grade
            .regraded
            .iter()
            .filter(|r| {
                !vols.iter().any(|(a, b)| {
                    r.x >= a[0]
                        && r.x <= b[0]
                        && r.z >= a[2]
                        && r.z <= b[2]
                        && a[1] <= r.graded + 1
                        && b[1] >= r.graded - 1
                })
            })
            .count();
        assert!(moved > 0, "{name}: regraded columns compared");
        assert_eq!(
            moved, open_to_compare,
            "{name}: every regraded column no volume covers"
        );
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
    assert!(
        shut[0].contains("loft"),
        "the loft's side alone: {}",
        shut[0]
    );
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
    assert!(
        shut[0].contains("tunnel"),
        "the tunnel's side alone: {}",
        shut[0]
    );
}

// ---------------------------------------------------------------- criterion 10

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

/// The emitted paths whose bytes differ between two builds, both sides'
/// paths included.
fn moved(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    a.keys()
        .chain(b.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|p| a.get(*p) != b.get(*p))
        .cloned()
        .collect()
}

/// Every place of the plan given the steepest bank the surface admits.
fn every_place_steep(v: &mut Value) {
    for b in boxes(v) {
        b["grade"] = json!({"rise": 5, "run": 1});
    }
}

/// Three campaigns with nothing to grade: the grade moves no byte of their
/// build. Measured on this revision by varying only the grade's own input —
/// every place's bank made the steepest the surface admits — so a later
/// change to anything else the build emits is not read as the grade's
/// (CLAUDE.md, *Measurement doctrine*). With a plan, the two builds are equal
/// file by file but the grade's own ledger, which states `C = 0`; with no
/// plan, nothing of the grade is resolved and no ledger is written.
#[test]
fn a_campaign_with_nothing_to_grade_moves_no_byte_by_its_grade() {
    let no_plan = {
        let loaded = delvec::compiler::load::load_campaign_dir(
            &common::repo_root().join("crates/delvec/tests/fixtures/and-join"),
        )
        .expect("readable");
        delvewright_dsl::parse_campaign(&loaded.raw).expect("parses")
    };
    assert!(no_plan.site_plan.is_none());
    assert!(
        delvewright_dsl::siteplan::grade::GradeBinding::of(&no_plan).is_none(),
        "no site plan: no grade is resolved"
    );
    assert!(Ground::of(&no_plan).grade().is_none());
    let out = emitted(&no_plan);
    assert!(
        !out.contains_key("validation/grade.json"),
        "no site plan: no grade ledger"
    );
    let own: Value =
        serde_json::from_slice(&out["validation/packtest-own-cells.json"]).expect("json");
    assert_eq!(own["laying_count"], 0, "no site plan: {own}");

    let solid = |v: &mut Value| {
        v["content"]["fill"] = json!({"kind": "solid", "block": "minecraft:stone"});
    };
    let cases: [(&str, Edit); 2] = [
        ("a solid fill", Box::new(solid)),
        ("every side walled", Box::new(|_: &mut Value| {})),
    ];
    let mut files = 0;
    for (name, edit) in &cases {
        let as_written = campaign_with(|v| edit(v));
        let steep = campaign_with(|v| {
            edit(v);
            every_place_steep(v);
        });
        assert_ne!(
            as_written.site_plan.as_ref().map(|p| &p.content),
            steep.site_plan.as_ref().map(|p| &p.content),
            "{name}: the perturbation reaches the plan"
        );
        let (a, b) = (emitted(&as_written), emitted(&steep));
        for (which, out) in [("as written", &a), ("steep", &b)] {
            let ledger: Value =
                serde_json::from_slice(out.get("validation/grade.json").expect("the ledger"))
                    .expect("parses");
            assert_eq!(
                ledger["regraded_columns"],
                json!(0),
                "{name}, {which}: C = 0"
            );
            let own: Value =
                serde_json::from_slice(&out["validation/packtest-own-cells.json"]).expect("json");
            assert_eq!(own["laying_count"], 0, "{name}, {which}: {own}");
        }
        let moved: Vec<String> = moved(&a, &b)
            .into_iter()
            .filter(|p| p != "validation/grade.json")
            .collect();
        assert!(
            moved.is_empty(),
            "{name}: a bank of another slope moved {} emitted file(s) where nothing \
             is graded: {moved:?}",
            moved.len()
        );
        files += a.len();
    }
    println!(
        "nothing-to-grade binding: 3 of the 4 fixture campaigns the criterion names; \
         {files} emitted file(s) compared across 2 builds each"
    );
    // The fourth campaign the spec names — unwalled sides all at the
    // terrain's height — is not among these: on the blockout fixture an
    // unwalled side at the grade lets a body walk off the region (`DW0322`).
    // Recorded as a debt of criterion 10.
    assert!(files > 0);
}
