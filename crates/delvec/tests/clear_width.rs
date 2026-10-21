//! **A way is whole** (spec-0120) over a built world: the site-plan fixture's
//! stand-ins, judged by the clear-width check (`DW1057`, `DW1058`) the way a
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
use delvewright_dsl::siteplan::{BoxWay, Offset, PlaceNarrowing, PlanAxis};

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
        line.contains("6 portal(s), 2 stair run(s), 1 contact(s), 0 way place(s), 0 piece way(s)"),
        "{line}"
    );
    assert!(
        line.contains("7 way(s) whole, 0 narrowed undeclared"),
        "{line}"
    );
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
/// along its run is refused naming the station; declared, it is green; one
/// column outside the declared width it is green; the box with `way` removed
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
        f.message.contains("\"along\": 8, \"across\": [1, 1]"),
        "{}",
        f.message
    );
    assert_eq!(b.to_json()["way_places"], 1);

    let mut c = fixture();
    hall_way(&mut c, 2, 0);
    {
        let plan = &mut c.site_plan.as_mut().unwrap().content;
        let hall = plan
            .boxes
            .iter_mut()
            .find(|b| b.node.0 == "node/hall")
            .unwrap();
        hall.narrowed = vec![PlaceNarrowing {
            along: 8,
            across: [1, 1],
            block: "minecraft:stone_brick_wall".to_string(),
            what: "a lamp post half-way down the hall".to_string(),
        }];
    }
    let (_, _, b, refused) = judged(&c, |m| post(m, 14));
    assert!(refused.is_empty(), "{:?}", refused);
    assert!(
        b.line()
            .contains("1 narrowed as declared (1 column(s) excused by 1 declaration(s))"),
        "{}",
        b.line()
    );

    // One column outside the declared width: green.
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

fn seam_mut<'a>(c: &'a mut Campaign, edge: &str) -> &'a mut delvewright_dsl::siteplan::Seam {
    c.site_plan
        .as_mut()
        .unwrap()
        .content
        .seams
        .iter_mut()
        .find(|s| s.edge.0 == edge)
        .unwrap()
}

fn seam_entry(
    side: delvewright_dsl::siteplan::NarrowSide,
    across: [u32; 2],
    what: &str,
) -> delvewright_dsl::siteplan::SeamNarrowing {
    delvewright_dsl::siteplan::SeamNarrowing {
        side,
        across,
        course: None,
        block: "minecraft:stone_bricks".to_string(),
        what: what.to_string(),
    }
}

/// **Criterion 1, the refusals.** Each shape §4 names, refused at validation
/// with its code, and the well-formed entry beside it accepted.
#[test]
fn the_surface_refuses_each_shape_with_its_code() {
    use delvewright_dsl::siteplan::NarrowSide;
    let c = fixture();
    assert!(plan_refusals(&c).is_empty(), "{:?}", plan_refusals(&c));

    // Well formed: the landing arch's hall-side approach, its second column.
    let mut c = fixture();
    seam_mut(&mut c, "edge/landing-hall").narrowed =
        vec![seam_entry(NarrowSide::B, [1, 1], "a post")];
    assert!(plan_refusals(&c).is_empty(), "{:?}", plan_refusals(&c));

    for (entry, says) in [
        (seam_entry(NarrowSide::B, [1, 1], " "), "`what` is empty"),
        (
            seam_entry(NarrowSide::B, [1, 2], "a post"),
            "of a way 2 column(s) wide",
        ),
        (
            seam_entry(NarrowSide::Plane, [0, 0], "a post"),
            "`side: plane` names a portal's opening",
        ),
        (
            seam_entry(NarrowSide::Run, [0, 0], "a post"),
            "`side: run` names a stair's course",
        ),
    ] {
        let mut c = fixture();
        seam_mut(&mut c, "edge/landing-hall").narrowed = vec![entry];
        let r = plan_refusals(&c);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].0, "DW0824", "{r:?}");
        assert!(r[0].1.contains(says), "{says}: {}", r[0].1);
    }

    // A box: a way wider than its place, and a narrowing on a room.
    let mut c = fixture();
    hall_way(&mut c, 12, 1);
    let r = plan_refusals(&c);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].0, "DW0826");
    assert!(r[0].1.contains("the way leaves its place"), "{}", r[0].1);

    let mut c = fixture();
    {
        let hall = c
            .site_plan
            .as_mut()
            .unwrap()
            .content
            .boxes
            .iter_mut()
            .find(|b| b.node.0 == "node/hall")
            .unwrap();
        hall.narrowed = vec![PlaceNarrowing {
            along: 0,
            across: [0, 0],
            block: "minecraft:stone_bricks".to_string(),
            what: "a post".to_string(),
        }];
    }
    let r = plan_refusals(&c);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].0, "DW0826");
    assert!(
        r[0].1.contains("a box without `way` is a room"),
        "{}",
        r[0].1
    );
}

/// **Criterion 1, the export.** `delvec schema --stage site-plan` exports the
/// box's `way` and both `narrowed` lists with their fields and closed sets,
/// and the prefab metadata export the contract edge's `narrowed`.
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
    for needle in [
        "\"BoxWay\"",
        "\"PlaceNarrowing\"",
        "\"SeamNarrowing\"",
        "\"NarrowSide\"",
        "\"along\"",
        "\"across\"",
        "\"course\"",
        "\"clear\"",
        "\"plane\"",
        "\"run\"",
    ] {
        assert!(plan.contains(needle), "site-plan schema lacks {needle}");
    }
    let meta = run("prefab-metadata").to_string();
    assert!(
        meta.contains("\"EdgeNarrowing\""),
        "prefab-metadata schema lacks EdgeNarrowing"
    );
    assert!(meta.contains("\"cells\""));
}
