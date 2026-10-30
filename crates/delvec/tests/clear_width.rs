//! **A way is whole** (spec-0120) over a built world: the site-plan fixture's
//! stand-ins, judged by the clear-width check (`DW1057`) the way a
//! build judges them — the plan, the assembled world, every runtime write.
//!
//! Each test is one half of a red→green pair over one declaration or one
//! block, with the cells read off `delvec validate`'s placing lines for the
//! fixture (`node/hall` at `[13, 4]`, 12 × 16, grade 64; `node/undercroft` at
//! `[13, 21]`, floor 59), never from the check's own arithmetic.

mod common;

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use delvec::compiler::assembled::Assembled;
use delvec::compiler::blockstate::BlockState;
use delvec::compiler::failure::Failure;
use delvec::compiler::nav::width::{self, WidthBinding};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::Campaign;
use delvewright_dsl::siteplan::{BoxWay, Offset, PlanAxis};

fn fixture() -> Campaign {
    let dir = common::repo_root().join("crates/delvec/tests/fixtures/blockout");
    let loaded = delvec::compiler::load::load_campaign_dir(&dir).expect("readable");
    delvewright_dsl::parse_campaign(&loaded.raw).expect("the blockout fixture parses")
}

fn empty_library() -> PrefabRegistry {
    let dir = std::env::temp_dir().join(format!("dw-clear-width-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    PrefabRegistry::load_dir(&dir).expect("an empty library loads")
}

/// Plan, assemble, edit the assembled blocks, and judge.
fn judged(
    c: &Campaign,
    edit: impl FnOnce(&mut delvec::compiler::blockstate::BlockMap),
) -> (Plan<'_>, Assembled, WidthBinding, Vec<Failure>) {
    let reg = empty_library();
    let plan = Plan::build(c, &reg).expect("the fixture plans");
    let mut assembled = delvec::compiler::assembled::assemble(&plan, &BTreeMap::new());
    edit(std::sync::Arc::make_mut(&mut assembled.blocks));
    let (binding, refused) = width::check(&plan, &reg, &assembled, &[]);
    (plan, assembled, binding, refused)
}

fn codes(refused: &[Failure]) -> Vec<&'static str> {
    refused.iter().map(|f| f.code.id()).collect()
}

fn hall_way(c: &mut Campaign, clear: u32, at: u32) {
    let plan = &mut c.site_plan.as_mut().expect("a plan").content;
    let hall = plan
        .boxes
        .iter_mut()
        .find(|b| b.node.0 == "node/hall")
        .expect("the hall");
    hall.way = Some(BoxWay {
        axis: PlanAxis::Z,
        clear: NonZeroU32::new(clear).unwrap(),
        at: Some(at),
    });
}

/// **Criterion 2.** The fixture's ways, counted against their denominators:
/// six portals (two of them stairs, both laid) and one contact over seven
/// traversal seams, seven boxes, no contract edge — and a box declaring `way`
/// is one more way.
#[test]
fn the_fixture_is_whole_and_states_its_population() {
    let c = fixture();
    let (_, _, b, refused) = judged(&c, |_| {});
    assert!(refused.is_empty(), "{:?}", codes(&refused));
    let j = b.to_json();
    assert_eq!(
        (
            &j["ways"],
            &j["traversal_seams"],
            &j["boxes"],
            &j["contract_edges"]
        ),
        (&7.into(), &7.into(), &7.into(), &0.into())
    );
    assert_eq!(
        (
            &j["portals"],
            &j["contacts"],
            &j["way_places"],
            &j["piece_ways"]
        ),
        (&6.into(), &1.into(), &0.into(), &0.into())
    );
    let line = b.line();
    assert!(
        line.contains(
            "6 portal(s), 2 stair run(s), 1 contact(s), 0 way place(s), 0 open side(s), 0 piece way(s)"
        ),
        "{line}"
    );
    assert!(line.contains("7 way(s) whole, 0 narrowed;"), "{line}");
    assert!(
        line.contains("0 stair run(s) undeclared by their piece"),
        "{line}"
    );

    let mut c = fixture();
    hall_way(&mut c, 2, 0);
    let (_, _, b, refused) = judged(&c, |_| {});
    assert!(refused.is_empty(), "{:?}", codes(&refused));
    assert_eq!(b.to_json()["way_places"], 1);
    assert_eq!(b.to_json()["ways"], 8);
}

/// **Criterion 7, the pair over one build.** A portal with a solid cell in its
/// plane reds `DW0836` and this check counts the plane into the columns left
/// to `DW0836` and raises nothing; a contact with half its span walled stands
/// green under `DW0836` and is refused `DW1057` here.
#[test]
fn the_dw0836_pair_over_one_build() {
    let c = fixture();
    // `edge/landing-hall`: the arch in the hall's west wall, x 12, z 10..11,
    // y 64..66. `edge/loft-drop`: the front on the loft's west wall, x 25, z
    // 8..13 at the loft's floor, 69 — the loft stair's arch shares the wall at
    // z 10..11, so the walled third of the front is z 12..13.
    let (plan, assembled, b, refused) = judged(&c, |m| {
        m.insert([12, 64, 10], BlockState::new("minecraft:stone"));
        for z in 12..=13 {
            for y in 69..=71 {
                m.insert([25, y, z], BlockState::new("minecraft:stone"));
            }
        }
    });
    let battery = delvec::compiler::blockout::check(&plan, &assembled.blocks).expect("a battery");
    let battery_codes: Vec<&str> = battery
        .findings
        .iter()
        .map(|(_, d)| d.code.as_str())
        .collect();
    assert!(battery_codes.contains(&"DW0836"), "{battery_codes:?}");
    assert!(
        battery
            .findings
            .iter()
            .filter(|(_, d)| d.code == "DW0836")
            .all(|(_, d)| d.message.contains("edge/landing-hall")),
        "only the portal is DW0836's: {:?}",
        battery
            .findings
            .iter()
            .map(|(_, d)| d.message.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        !battery_codes.contains(&"DW0877"),
        "half a front still crosses: {battery_codes:?}"
    );
    assert_eq!(codes(&refused), vec!["DW1057"], "{refused:?}");
    assert!(
        refused[0].message.contains("edge/loft-drop"),
        "{}",
        refused[0].message
    );
    assert!(
        refused[0].message.contains("at the span"),
        "{}",
        refused[0].message
    );
    // The deferral is total and it is the set DW0836 proves: every portal's
    // allocated cells, counted once.
    let dw0836: usize = plan
        .blockout
        .as_ref()
        .unwrap()
        .seams
        .iter()
        .filter(|s| s.crossing == delvewright_dsl::siteplan::Crossing::Portal)
        .map(|s| delvec::compiler::blockout::cells_of(s.opening.0, s.opening.1).count())
        .sum();
    assert_eq!(b.census.plane_columns, dw0836);
    assert!(
        b.line()
            .contains(&format!("{dw0836} plane column(s) left to DW0836"))
    );
}

/// The undercroft's floor stair with its hole moved to the east edge of the
/// cell's floor, so the run the stand-in lays starts against the east wall:
/// `[5, 1]` puts course 0 — the full-height top tread — in the tunnel
/// approach's column x 20, across z 22..24, beside the tunnel's z 21..23.
fn hole_at(c: &mut Campaign, at: [i64; 2]) {
    let plan = &mut c.site_plan.as_mut().expect("a plan").content;
    let s = plan
        .seams
        .iter_mut()
        .find(|s| s.edge.0 == "edge/cell-undercroft")
        .expect("the undercroft stair");
    s.at = Some(Offset::Plane(at));
    s.meets = Some(Offset::Plane(at));
}

/// **Criterion 8.** A two-seam stand-in with no pieces, where one seam's laid
/// run enters the other's approach, is refused naming the tread cells; the
/// stair seam moved one cell along its face is whole.
#[test]
fn a_run_laid_across_part_of_a_door_is_refused_and_one_cell_over_is_whole() {
    let mut c = fixture();
    hole_at(&mut c, [5, 1]);
    let (_, _, _, refused) = judged(&c, |_| {});
    let f = refused
        .iter()
        .find(|f| f.code.id() == "DW1057" && f.message.contains("edge/undercroft-tunnel"))
        .unwrap_or_else(|| panic!("the tunnel's approach is narrowed: {refused:?}"));
    assert!(f.message.contains("1 of 3"), "{}", f.message);
    assert!(f.message.contains("[20, 59, 22]"), "{}", f.message);
    assert!(f.message.contains("[20, 59, 23]"), "{}", f.message);

    let mut c = fixture();
    hole_at(&mut c, [4, 1]);
    let (_, _, _, refused) = judged(&c, |_| {});
    assert!(
        !refused
            .iter()
            .any(|f| f.message.contains("edge/undercroft-tunnel")),
        "{refused:?}"
    );
}

/// **Criterion 10a.** A way place: one solid cell in a middle column half-way
/// along its run is refused naming the station; one column outside the
/// declared width, on the verge, it is green; the box with `way` removed
/// is a room and the cell is not judged.
#[test]
fn a_way_place_is_judged_along_its_whole_length() {
    // The hall, z 4..19, its way the two columns x 13..14 (`at` 0, `clear`
    // 2), well west of the loft stair's run along the east wall. Half-way
    // along is z 12, `along` 8.
    let post = |m: &mut delvec::compiler::blockstate::BlockMap, x: i32| {
        m.insert([x, 64, 12], BlockState::new("minecraft:stone_brick_wall"));
    };
    let mut c = fixture();
    hall_way(&mut c, 2, 0);
    let (_, _, b, refused) = judged(&c, |m| post(m, 14));
    let f = refused.first().expect("refused");
    assert_eq!(f.code.id(), "DW1057");
    assert!(
        f.message.contains("8 cell(s) along `node/hall`"),
        "{}",
        f.message
    );
    assert!(f.message.contains("1 of 2"), "{}", f.message);
    assert!(
        f.message
            .contains("[14, 64, 12] `minecraft:stone_brick_wall`"),
        "{}",
        f.message
    );
    assert_eq!(b.to_json()["way_places"], 1);

    // One column outside the declared width, on the verge: green.
    let mut c = fixture();
    hall_way(&mut c, 2, 0);
    let (_, _, _, refused) = judged(&c, |m| post(m, 15));
    assert!(refused.is_empty(), "{:?}", refused);

    // No `way`: a room, not judged — the ledger holds no row for it.
    let c = fixture();
    let (_, _, b, refused) = judged(&c, |m| post(m, 14));
    assert!(refused.is_empty(), "{:?}", refused);
    assert!(
        !b.to_json()["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["way"] == "node/hall")
    );
}

/// A station a seam's approach covers is judged once, under the seam: the
/// hall's way along its west edge meets the landing arch's approach at x 13
/// and the loft stair's foot at x 14, both across z 10..11, and those cells
/// are not way-place columns.
#[test]
fn a_station_a_seams_approach_covers_is_judged_under_the_seam() {
    let mut c = fixture();
    hall_way(&mut c, 2, 0);
    let (_, _, b, _) = judged(&c, |_| {});
    let j = b.to_json();
    let hall = j["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["way"] == "node/hall")
        .expect("the hall's row");
    let stations = hall["stations"].as_array().unwrap();
    // along 6 and 7 are z 10 and 11: x 13 is the arch's, x 14 the stair's.
    assert_eq!(stations[6]["columns"], 0, "{}", stations[6]);
    assert_eq!(stations[7]["columns"], 0, "{}", stations[7]);
    assert_eq!(stations[5]["columns"], 2, "{}", stations[5]);
}

/// The stage-4 refusals over the fixture, as `(code, message)`.
fn plan_refusals(c: &Campaign) -> Vec<(String, String)> {
    let mut d = Vec::new();
    let mut reads = delvewright_dsl::metrics::Reads::new();
    delvewright_dsl::siteplan::check(c, &mut reads, &mut d);
    d.into_iter()
        .filter(|x| x.severity == delvewright_dsl::Severity::Error)
        .map(|x| (x.code, x.message))
        .collect()
}

/// The fixture's raw documents, with `edit` applied to the site plan's JSON.
fn raw_with_plan(edit: impl FnOnce(&mut serde_json::Value)) -> delvewright_dsl::RawCampaign {
    let dir = common::repo_root().join("crates/delvec/tests/fixtures/blockout");
    let mut loaded = delvec::compiler::load::load_campaign_dir(&dir).expect("readable");
    let mut plan: serde_json::Value =
        serde_json::from_str(loaded.raw.site_plan.as_deref().expect("a plan")).unwrap();
    edit(&mut plan);
    loaded.raw.site_plan = Some(plan.to_string());
    loaded.raw
}

/// **Criterion 1, the refusals.** A way wider than its place is `DW0826`;
/// a `narrowed` written on a seam or on a box — the withdrawn surface — is
/// refused as a key the stage does not have (`DW0100`), before any world is
/// judged.
#[test]
fn the_surface_refuses_each_shape_with_its_code() {
    let c = fixture();
    assert!(plan_refusals(&c).is_empty(), "{:?}", plan_refusals(&c));

    let mut c = fixture();
    hall_way(&mut c, 12, 1);
    let r = plan_refusals(&c);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].0, "DW0826");
    assert!(r[0].1.contains("the way leaves its place"), "{}", r[0].1);

    let excuse = serde_json::json!([{
        "side": "b", "across": [1, 1],
        "block": "minecraft:stone_bricks", "what": "a post"
    }]);
    for key in ["seams", "boxes"] {
        let raw = raw_with_plan(|plan| {
            plan["content"][key][0]["narrowed"] = excuse.clone();
        });
        let Err(diags) = delvewright_dsl::parse_campaign(&raw) else {
            panic!("a `narrowed` on {key}[0] must not parse");
        };
        assert!(
            diags
                .iter()
                .any(|d| d.code == "DW0100" && d.message.contains("narrowed")),
            "{key}: {:?}",
            diags
                .iter()
                .map(|d| (d.code.clone(), d.message.clone()))
                .collect::<Vec<_>>()
        );
    }
}

/// **Criterion 6, no excuse.** Every shape of excuse the surface once
/// carried is refused before the world is judged — on a seam, on a box (the
/// test above), on a piece's contract edge (`grammar_contract_check.rs`) —
/// and with no excuse written, a structure in a way is still `DW1057`.
#[test]
fn a_structure_in_a_way_is_refused_with_no_excuse_written() {
    // The landing arch's hall-side approach, x 13, z 10..11 at grade 64: a
    // post in its second column.
    let c = fixture();
    let (_, _, _, refused) = judged(&c, |m| {
        m.insert([13, 64, 11], BlockState::new("minecraft:stone_bricks"));
    });
    let f = refused
        .iter()
        .find(|f| f.message.contains("edge/landing-hall"))
        .unwrap_or_else(|| panic!("the arch's approach is narrowed: {refused:?}"));
    assert_eq!(f.code.id(), "DW1057");
    assert!(f.message.contains("1 of 2"), "{}", f.message);
}

/// **Criterion 1, the export.** `delvec schema --stage site-plan` exports the
/// box's `way` with its fields and no `narrowed` on a box or a seam; the
/// prefab metadata export carries no `narrowed` on a contract edge.
#[test]
fn the_schema_exports_the_surface() {
    let run = |stage: &str| -> serde_json::Value {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
            .args(["schema", "--stage", stage])
            .output()
            .expect("delvec runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("a schema")
    };
    let plan = run("site-plan").to_string();
    for needle in ["\"BoxWay\"", "\"clear\"", "\"axis\""] {
        assert!(plan.contains(needle), "site-plan schema lacks {needle}");
    }
    for gone in ["narrowed", "Narrowing", "NarrowSide"] {
        assert!(
            !plan.contains(gone),
            "site-plan schema still carries {gone}"
        );
    }
    let meta = run("prefab-metadata").to_string();
    for gone in ["narrowed", "EdgeNarrowing"] {
        assert!(
            !meta.contains(gone),
            "prefab-metadata schema still carries {gone}"
        );
    }
}

/// **Criterion 2, the zero.** A campaign with no site plan and no contract
/// way — hello-world, one area bound to a piece that declares no spatial
/// contract — still writes `validation/clear-width.json`, every count `0`.
#[test]
fn a_campaign_with_no_way_writes_its_zeros() {
    let out = std::env::temp_dir().join(format!("dw-clear-width-hello-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let r = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["build"])
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .expect("delvec runs");
    let log = String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success(), "{log}");
    assert!(
        log.contains("clear width (DW1057): 0 way(s) of 0 traversal seam(s), 0 box(es) and 0 contract edge(s)"),
        "{log}"
    );
    let j: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/clear-width.json"))
            .expect("the ledger is written"),
    )
    .unwrap();
    for key in [
        "ways",
        "examined",
        "traversal_seams",
        "boxes",
        "contract_edges",
        "portals",
        "contacts",
        "way_places",
        "piece_ways",
        "plane_columns_left_to_dw0836",
        "stair_runs_undeclared",
    ] {
        assert_eq!(j[key], 0, "{key}: {j}");
    }
    assert_eq!(j["states"], 1, "the world as built is the one state judged");
    let _ = std::fs::remove_dir_all(&out);
}

/// **A way place climbs where its place climbs.** The hall's way, x 13..14,
/// built as a flight: at z 14 a stair tread in x 13 beside a ladder whose
/// foot stands on the hall's floor course in x 14, the floor raised a course
/// under both columns at z 15..16, and back down at z 17. A body climbs the
/// tread and passes the ladder at the treads' plane, so every station is
/// whole — the station that judged both columns at the hall's floor took the
/// tread and left only the ladder. A wall standing on the raised floor in
/// x 13 still narrows the way, at the plane the climb reached.
#[test]
fn a_way_place_built_as_a_flight_is_judged_at_the_plane_it_climbs_to() {
    let stair =
        "minecraft:stone_brick_stairs[facing=south,half=bottom,shape=straight,waterlogged=false]";
    let ladder = "minecraft:ladder[facing=south,waterlogged=false]";
    let flight = |m: &mut delvec::compiler::blockstate::BlockMap| {
        m.insert([13, 64, 14], BlockState::new(stair));
        m.insert([14, 64, 14], BlockState::new(ladder));
        m.insert([14, 65, 14], BlockState::new(ladder));
        for z in 15..=16 {
            for x in 13..=14 {
                m.insert([x, 64, z], BlockState::new("minecraft:stone_bricks"));
            }
        }
    };
    let mut c = fixture();
    hall_way(&mut c, 2, 0);
    let (_, _, b, refused) = judged(&c, |m| flight(m));
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(b.to_json()["way_places"], 1);

    // A wall on the raised floor in the way: narrowed, at the climbed plane.
    let mut c = fixture();
    hall_way(&mut c, 2, 0);
    let (_, _, _, refused) = judged(&c, |m| {
        flight(m);
        m.insert([13, 65, 15], BlockState::new("minecraft:stone_brick_wall"));
    });
    let f = refused.first().expect("a wall in the way is refused");
    assert_eq!(f.code.id(), "DW1057");
    assert!(
        f.message.contains("11 cell(s) along `node/hall`"),
        "{}",
        f.message
    );
    assert!(
        f.message
            .contains("[13, 65, 15] `minecraft:stone_brick_wall`"),
        "{}",
        f.message
    );
}
