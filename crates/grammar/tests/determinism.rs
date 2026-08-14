//! ADR-0006 for the grammar back end: same program + same region + same seed
//! gives byte-identical output, and the parameters are real controls rather
//! than decoration (spec-0027 acceptance criteria 1 and 2).

use std::collections::BTreeSet;

use delvewright_grammar::ir::Program;
use delvewright_grammar::library::temple::roof;
use delvewright_grammar::library::{
    ambush_door, castle, causeway, church, cliff_path, drop_shaft, dumbwaiter, elite_ground,
    far_side_bar, lift_shaft, rafter_hall, stair_flight, store_room, tee_passage, temple,
    watch_bay,
};
use delvewright_grammar::{Box3, ExpandOptions, expand};
// W3: the palette/prop family (W + S + M + X).
use delvewright_grammar::library::{boulder_stair, broken_grate, threshold_motif};
// The mechanism family (task #182 zone round): the rest point, the lure and the
// hazard control.
use delvewright_grammar::library::{bait_stand, disarm_stand, hearth_ward};

const TEMPLE_REGION: Box3 = Box3::at_origin([13, 14, 21]);
const CASTLE_REGION: Box3 = Box3::at_origin([41, 14, 25]);
const CHURCH_REGION: Box3 = Box3::at_origin([15, 16, 30]);
/// The original staging rules (spec-0027 W1, W2 and W4) travel with the
/// ports: they owe the same structural validity, JSON round trip and
/// determinism — anchors included, which for `rafter_hall` means the
/// generated `perch-<i>` numbering, for `store_room` means the seeded
/// position of the one odd barrel, and for the W4 rules means the notched
/// `rescue_ladder`/`obstruct`/`seal_flank` knobs never move a byte on their
/// own.
const CLIFF_REGION: Box3 = Box3::at_origin([3, 6, 30]);
const PASSAGE_REGION: Box3 = Box3::at_origin([7, 7, 24]);
const HALL_REGION: Box3 = Box3::at_origin([13, 6, 25]);
const DOOR_REGION: Box3 = Box3::at_origin([11, 5, 13]);
const STORE_REGION: Box3 = Box3::at_origin([7, 5, 14]);
/// W3: the palette/prop family (spec-0027 W + S + M + X).
const STAIR_REGION: Box3 = Box3::at_origin([9, 6, 27]);
const THRESHOLD_REGION: Box3 = Box3::at_origin([9, 6, 13]);
const GRATE_REGION: Box3 = Box3::at_origin([3, 5, 14]);
/// The topology family (task #182): vertical links, one-way bars, elite ground.
const SHAFT_REGION: Box3 = Box3::at_origin([4, 8, 6]);
const DUCT_REGION: Box3 = Box3::at_origin([6, 8, 8]);
const BAR_REGION: Box3 = Box3::at_origin([5, 5, 7]);
const TEE_REGION: Box3 = Box3::at_origin([5, 5, 12]);
const CAUSEWAY_REGION: Box3 = Box3::at_origin([7, 10, 9]);
const ARENA_REGION: Box3 = Box3::at_origin([19, 5, 25]);
/// The vertical family's two-way member: five across (two walls and a
/// three-wide lane), fourteen tall, twenty-two long.
const FLIGHT_REGION: Box3 = Box3::at_origin([5, 14, 22]);
/// The vertical family's stationary member: five across, sixteen tall (a sill
/// and two whole storeys), seven deep — so the `lift-station-<i>` and
/// `lift-call-<i>` numbering a campaign binds is part of what is pinned here.
const LIFT_REGION: Box3 = Box3::at_origin([5, 16, 7]);
/// The mechanism family: a rest point's nook, a lure with its watcher, and a
/// hazard's control at the head of its run.
const HEARTH_REGION: Box3 = Box3::at_origin([8, 6, 14]);
const BAIT_REGION: Box3 = Box3::at_origin([9, 8, 14]);
const DISARM_REGION: Box3 = Box3::at_origin([9, 7, 16]);

fn cases() -> Vec<(Program, Box3)> {
    vec![
        (temple(), TEMPLE_REGION),
        (castle(), CASTLE_REGION),
        (church(), CHURCH_REGION),
        (cliff_path(), CLIFF_REGION),
        (watch_bay(), PASSAGE_REGION),
        (rafter_hall(), HALL_REGION),
        (ambush_door(), DOOR_REGION),
        (store_room(), STORE_REGION),
        (boulder_stair::boulder_stair(), STAIR_REGION),
        (threshold_motif::threshold_motif(), THRESHOLD_REGION),
        (broken_grate::broken_grate(), GRATE_REGION),
        (drop_shaft(), SHAFT_REGION),
        (dumbwaiter(), DUCT_REGION),
        (far_side_bar(), BAR_REGION),
        (tee_passage(), TEE_REGION),
        (causeway(), CAUSEWAY_REGION),
        (elite_ground(), ARENA_REGION),
        (stair_flight(), FLIGHT_REGION),
        (lift_shaft(), LIFT_REGION),
        (hearth_ward(), HEARTH_REGION),
        (bait_stand(), BAIT_REGION),
        (disarm_stand(), DISARM_REGION),
    ]
}

/// A grammar program with a genuinely probabilistic rule, authored the way the
/// pipeline will author one: as JSON. Doubles as the IR's schema-shape example.
const CAIRN_JSON: &str = r#"{
  "version": "1.2.0",
  "name": "cairn",
  "start": "cairn",
  "params": { "course": 1 },
  "palette": {
    "rock": [
      { "weight": 5, "block": "minecraft:cobblestone" },
      { "weight": 2, "block": "minecraft:mossy_cobblestone" },
      { "weight": 1, "block": "minecraft:andesite" }
    ]
  },
  "rules": {
    "cairn": [
      {
        "weight": 3,
        "when": { "cond": "cmp", "lhs": { "expr": "dim", "dim": "y" },
                  "op": "gt", "rhs": { "expr": "int", "value": 1 } },
        "body": {
          "op": "split",
          "axis": "y",
          "sizes": [
            { "size": "absolute", "blocks": { "expr": "param", "name": "course" } },
            { "size": "relative", "weight": { "expr": "int", "value": 1 } }
          ],
          "children": [
            { "op": "fill", "material": { "role": "rock" } },
            { "op": "call", "symbol": "cairn" }
          ]
        }
      },
      {
        "weight": 1,
        "body": { "op": "fill", "material": { "role": "rock" } }
      },
      {
        "weight": 1,
        "when": { "cond": "cmp", "lhs": { "expr": "dim", "dim": "x" },
                  "op": "gt", "rhs": { "expr": "int", "value": 1 } },
        "body": {
          "op": "split",
          "axis": "x",
          "sizes": [
            { "size": "absolute", "blocks": { "expr": "int", "value": 1 } },
            { "size": "relative", "weight": { "expr": "int", "value": 1 } }
          ],
          "children": [{ "op": "void" }, { "op": "call", "symbol": "cairn" }]
        }
      }
    ]
  }
}"#;

fn cairn() -> Program {
    serde_json::from_str(CAIRN_JSON).expect("the JSON IR parses")
}

#[test]
fn expanding_twice_gives_the_same_bytes() {
    for (program, region) in cases() {
        for seed in [0u64, 1, 7, 4_294_967_296, u64::MAX] {
            let a = expand(&program, region, &ExpandOptions::seeded(seed)).unwrap();
            let b = expand(&program, region, &ExpandOptions::seeded(seed)).unwrap();
            assert_eq!(
                a.model.canonical_bytes(),
                b.model.canonical_bytes(),
                "{} is not reproducible at seed {seed}",
                program.name
            );
            assert_eq!(a.stats, b.stats, "{}", program.name);
            // Anchors are output too: same program, same seed, same names on the
            // same cells — including the per-stem numbering, which is a fact
            // about the derivation and not about the run.
            assert_eq!(
                a.anchors, b.anchors,
                "{}'s anchors are not reproducible at seed {seed}",
                program.name
            );
        }
    }
    assert!(
        !expand(&castle(), CASTLE_REGION, &ExpandOptions::seeded(0))
            .unwrap()
            .anchors
            .is_empty(),
        "the castle declares an anchor, so the assertion above is not vacuous"
    );
}

#[test]
fn a_probabilistic_program_follows_its_seed_and_only_its_seed() {
    let program = cairn();
    let region = Box3::at_origin([9, 12, 9]);
    let mut shapes = BTreeSet::new();
    for seed in 0..24u64 {
        let a = expand(&program, region, &ExpandOptions::seeded(seed)).unwrap();
        let b = expand(&program, region, &ExpandOptions::seeded(seed)).unwrap();
        assert_eq!(
            a.model.canonical_bytes(),
            b.model.canonical_bytes(),
            "seed {seed} is not reproducible"
        );
        shapes.insert(a.model.canonical_bytes());
    }
    assert!(
        shapes.len() > 8,
        "24 seeds gave only {} distinct models — the seed is not reaching the \
         rule choice",
        shapes.len()
    );
}

#[test]
fn expansions_do_not_leak_into_each_other() {
    // Upstream keeps rules, materials and the derivation stack in module-level
    // dicts, so what you expanded last can change what you expand next. This is
    // the regression that shape can never come back.
    let region = Box3::at_origin([9, 12, 9]);
    let alone = expand(&cairn(), region, &ExpandOptions::seeded(5)).unwrap();

    let mut interleaved = None;
    for (program, other_region) in cases() {
        let _ = expand(&program, other_region, &ExpandOptions::seeded(99));
        let again = expand(&cairn(), region, &ExpandOptions::seeded(5)).unwrap();
        interleaved = Some(again.model.canonical_bytes());
    }
    assert_eq!(alone.model.canonical_bytes(), interleaved.unwrap());
}

#[test]
fn the_parameter_sweep_produces_distinct_valid_models() {
    // spec-0027 acceptance 2: kind / size / style are real controls.
    let mut models = BTreeSet::new();
    let mut count = 0;
    for kind in [roof::PITCHED, roof::FLAT, roof::CAPPED, roof::OPEN] {
        for column_height in [6, 8, 11] {
            for column_size in [1, 2] {
                for depth in [17u32, 21, 27] {
                    let mut program = temple();
                    program.set_param("roof", kind).unwrap();
                    program.set_param("column_height", column_height).unwrap();
                    program.set_param("column_size", column_size).unwrap();
                    let region = Box3::at_origin([13, 1 + column_height as u32 + 5, depth]);
                    let out =
                        expand(&program, region, &ExpandOptions::seeded(1)).unwrap_or_else(|e| {
                            panic!("{kind}/{column_height}/{column_size}/{depth}: {e}")
                        });
                    assert!(
                        out.model.filled_cells() > 100,
                        "{kind}/{column_height}/{column_size}/{depth} built almost nothing"
                    );
                    models.insert(out.model.canonical_bytes());
                    count += 1;
                }
            }
        }
    }
    assert_eq!(
        models.len(),
        count,
        "{count} parameter combinations collapsed to {} models",
        models.len()
    );
}

#[test]
fn the_region_is_a_control_too() {
    // Moving the box moves the building and nothing else.
    let program = temple();
    let here = expand(&program, TEMPLE_REGION, &ExpandOptions::seeded(2)).unwrap();
    let there = expand(
        &program,
        Box3::new([-104, 62, 813], TEMPLE_REGION.size),
        &ExpandOptions::seeded(2),
    )
    .unwrap();
    assert_eq!(here.model.filled_cells(), there.model.filled_cells());
    assert_eq!(here.model.palette(), there.model.palette());
    assert_ne!(
        here.model.canonical_bytes(),
        there.model.canonical_bytes(),
        "the origin is part of the model"
    );
    for pos in TEMPLE_REGION.positions() {
        let moved = [pos[0] - 104, pos[1] + 62, pos[2] + 813];
        assert_eq!(here.model.get(pos), there.model.get(moved));
    }
}

#[test]
fn runaway_recursion_stops_deterministically() {
    // A rule that calls itself without shrinking anything: the budget turns an
    // authoring mistake into a diagnostic instead of a hang.
    let program: Program = serde_json::from_str(
        r#"{
          "version": "1.2.0", "name": "runaway", "start": "loop",
          "rules": { "loop": [{ "body": { "op": "call", "symbol": "loop" } }] }
        }"#,
    )
    .unwrap();
    let err = expand(
        &program,
        Box3::at_origin([4, 4, 4]),
        &ExpandOptions::seeded(0),
    )
    .unwrap_err();
    assert!(err.to_string().contains("depth limit"), "{err}");
}
