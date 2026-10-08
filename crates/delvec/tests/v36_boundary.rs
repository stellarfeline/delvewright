//! spec-0092 §10: the boundary and the world agree (`DW0960`), the clock skips a
//! player watching a cutscene and a creator flying out of the body, and a
//! boundary that does not return starts no clock.
//!
//! The rule is asked of `bound::judge` over real places and regions; the
//! emission is read off a hello-world build with a boundary declared.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvec::compiler::bound::{BoundGate, judge};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

const REGION: ([i32; 3], [i32; 3]) = ([0, 50, 0], [20, 1024, 20]);

fn place(what: &str, c: [i32; 3]) -> (String, [i32; 3]) {
    (what.to_string(), c)
}

#[test]
fn a_place_outside_a_region_that_returns_is_dw0960() {
    let places = [
        place("a link lands", [30, 64, 5]),
        place("step 1", [5, 64, 5]),
    ];
    let (gate, f) = judge(&places, Some(REGION), true, &BTreeSet::new(), None);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].code.to_string(), "DW0960");
    assert!(
        f[0].message.contains("a link lands at [30, 64, 5]"),
        "{}",
        f[0].message
    );
    assert_eq!((gate.places, gate.outside), (2, 1));
    let inside = [place("step 1", [5, 64, 5])];
    assert!(
        judge(&inside, Some(REGION), true, &BTreeSet::new(), None)
            .1
            .is_empty()
    );
}

#[test]
fn a_region_that_does_not_return_round_a_world_a_body_leaves_is_dw0960() {
    let walk_out: BTreeSet<[i32; 3]> = [[5, 64, 5], [25, 64, 5]].into_iter().collect();
    let (_, f) = judge(&[], Some(REGION), false, &walk_out, None);
    assert_eq!(f[0].code.to_string(), "DW0960");
    assert!(f[0].message.contains("[25, 64, 5]"), "{}", f[0].message);
    let sealed: BTreeSet<[i32; 3]> = [[5, 64, 5]].into_iter().collect();
    let (_, f) = judge(&[], Some(REGION), false, &sealed, Some([5, 64, 5]));
    assert!(
        f[0].message.contains("open sea"),
        "the sea is a way out too"
    );
    let (gate, f) = judge(
        &[place("far", [30, 64, 5])],
        Some(REGION),
        false,
        &sealed,
        None,
    );
    assert!(
        f.is_empty(),
        "a region that does not return does not hold a place to it"
    );
    assert_eq!(gate.reached, 1);
}

#[test]
fn the_binding_line_states_what_was_examined() {
    assert_eq!(
        BoundGate::default().line(),
        "boundary binding: no boundary declared — 0 place(s) examined, 0 refused"
    );
    let (gate, _) = judge(
        &[place("s", [5, 64, 5])],
        Some(REGION),
        true,
        &BTreeSet::new(),
        None,
    );
    assert_eq!(
        gate.line(),
        "boundary binding: the region returns; 1 place(s) a body is put examined, 0 outside; 0 \
         reachable cell(s) examined for a way out; 0 refused"
    );
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build_with_boundary(who: &str, boundary: serde_json::Value) -> Result<BuildOutput, String> {
    let dir = tmp(&format!("v36-boundary-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(common::hello_world_dir().join(f), dir.join(f)).unwrap();
    }
    let mut world: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::hello_world_dir().join("world.json")).unwrap(),
    )
    .unwrap();
    world["content"]["boundary"] = boundary;
    std::fs::write(
        dir.join("world.json"),
        serde_json::to_string_pretty(&world).unwrap(),
    )
    .unwrap();
    let prefab_dir = common::prefabs_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("parses");
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                structures.insert(
                    t.structure_file.clone(),
                    std::fs::read(prefab_dir.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map_err(|e| format!("{e:?}"))
}

fn text(out: &BuildOutput, path_end: &str) -> Option<String> {
    out.iter()
        .find(|(p, _)| p.ends_with(path_end))
        .map(|(_, b)| String::from_utf8_lossy(b).to_string())
}

/// The clock skips a player watching a cutscene and a creator flying out of
/// the body, and the PackTest templates prove each tag on its own.
#[test]
fn the_clock_skips_the_cutscene_and_the_free_camera() {
    let out = build_with_boundary("returns", serde_json::json!({ "margin": 4 })).expect("builds");
    let tick = text(&out, "function/boundary_tick.mcfunction")
        .expect("a boundary that returns has a clock");
    assert!(
        tick.contains("execute as @a[tag=!dw_cutscene,tag=!dw_free] unless entity @s["),
        "{tick}"
    );
    for t in [
        "boundary_exempt_cutscene.mcfunction",
        "boundary_exempt_free.mcfunction",
    ] {
        let body = text(&out, t).unwrap_or_else(|| panic!("{t} is emitted"));
        assert!(
            body.contains("function hello-world:boundary_tick"),
            "{body}"
        );
    }
}

/// `returns: false` keeps the region and starts no clock.
#[test]
fn a_boundary_that_does_not_return_starts_no_clock() {
    let out = build_with_boundary("off", serde_json::json!({ "margin": 4, "returns": false }))
        .expect("the hall cannot be left");
    assert!(text(&out, "function/boundary_tick.mcfunction").is_none());
    assert!(text(&out, "test/v06_boundary_return.mcfunction").is_none());
    let setup = text(&out, "function/setup_finish.mcfunction").unwrap_or_default()
        + &text(&out, "function/setup.mcfunction").unwrap_or_default();
    assert!(
        setup.contains("data modify storage dw:region bounds"),
        "the region is kept"
    );
    assert!(
        !setup.contains("boundary_tick"),
        "and no clock is scheduled"
    );
}
