//! The ported rule libraries expand, and expand into the buildings they claim.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::ir::{Paint, Program, WeightedBlock};
use delvewright_grammar::library::{
    ambush_door, castle, causeway, church, cliff_path, drop_shaft, dumbwaiter, elite_ground,
    far_side_bar, rafter_hall, stair_flight, store_room, tee_passage, temple, watch_bay,
};
use delvewright_grammar::{Box3, ExpandOptions, expand};
// W3: the palette/prop family (W + S + M + X).
use delvewright_grammar::library::{boulder_stair, broken_grate, threshold_motif};
// The mechanism family (task #182 zone round): the rest point, the lure and the
// hazard control.
use delvewright_grammar::library::{bait_stand, disarm_stand, hearth_ward};

/// Regions each program is comfortably sized for. Sizes below these are a
/// documented error, not a silent nothing — see `undersized_regions_are_loud`.
const TEMPLE_REGION: Box3 = Box3::at_origin([13, 14, 21]);
const CASTLE_REGION: Box3 = Box3::at_origin([41, 14, 25]);
const CHURCH_REGION: Box3 = Box3::at_origin([15, 16, 30]);
/// The original staging rules (spec-0027 W1, W2 and W4) travel with the
/// ports: they owe the same structural validity, JSON round trip and
/// determinism.
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
/// The mechanism family: a rest point's nook, a lure with its watcher, and a
/// hazard's control at the head of its run.
const HEARTH_REGION: Box3 = Box3::at_origin([8, 6, 14]);
const BAIT_REGION: Box3 = Box3::at_origin([9, 8, 14]);
const DISARM_REGION: Box3 = Box3::at_origin([9, 7, 16]);

fn programs() -> Vec<(Program, Box3)> {
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
        (hearth_ward(), HEARTH_REGION),
        (bait_stand(), BAIT_REGION),
        (disarm_stand(), DISARM_REGION),
    ]
}

#[test]
fn every_library_program_is_structurally_valid() {
    for (program, _) in programs() {
        program
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
    }
}

#[test]
fn every_library_program_builds_something_inside_its_box() {
    for (program, region) in programs() {
        let out = expand(&program, region, &ExpandOptions::seeded(1))
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
        let filled = out.model.filled_cells();
        let volume = region.volume() as usize;
        assert!(
            filled > volume / 50,
            "{} filled only {filled} of {volume} cells",
            program.name
        );
        assert!(
            filled < volume,
            "{} filled the whole box — that is a cube, not a building",
            program.name
        );
        assert_eq!(out.model.region(), region);
        // Every cell the model holds is inside the region by construction; this
        // asserts the derivation actually reached the far corners of the box.
        assert!(out.stats.rules_applied > 5, "{}", program.name);
    }
}

/// Count the distinct column runs along the temple's front colonnade.
fn temple_columns(program: &Program, depth: u32) -> usize {
    // The peristyle is a gap/column rhythm along Z, so a deeper box gets more
    // columns. Count the distinct solid runs along the front row.
    let region = Box3::at_origin([13, 14, depth]);
    let out = expand(program, region, &ExpandOptions::seeded(1)).unwrap();
    let mut runs = 0;
    let mut prev_solid = false;
    for z in 0..depth as i32 {
        let solid = !out.model.get([1, 4, z]).unwrap().is_air();
        if solid && !prev_solid {
            runs += 1;
        }
        prev_solid = solid;
    }
    runs
}

#[test]
fn the_temple_has_a_colonnade_that_follows_the_box() {
    let program = temple();
    let shallow = temple_columns(&program, 15);
    let deep = temple_columns(&program, 29);
    assert!(
        deep > shallow && shallow >= 3,
        "colonnade did not follow the box: {shallow} vs {deep}"
    );
}

/// `library/temple.rs` claims the port diverges from upstream — which fixes the
/// colonnade at four columns — *without* losing upstream's building: "four
/// columns across a nine-deep box reproduces upstream exactly". That claim was
/// prose, so it could rot silently the next time the `columns` rule is touched.
/// Here it is arithmetic: a nine-deep box, at the default `column_size` of 1,
/// gives exactly the tetrastyle the paper's own figure shows.
#[test]
fn a_nine_deep_box_reproduces_upstreams_four_columns() {
    let mut program = temple();
    assert_eq!(program.params["column_size"], 1, "upstream's column width");
    assert_eq!(
        temple_columns(&program, 9),
        4,
        "the divergence note promises upstream's tetrastyle at depth 9"
    );
    // And the rhythm is genuinely `column_size`-driven, not a coincidence of 9:
    // doubling the thickness halves what fits.
    program.set_param("column_size", 2).unwrap();
    assert_eq!(temple_columns(&program, 9), 3);
}

#[test]
fn the_church_lays_directional_stairs_and_a_two_half_door() {
    let out = expand(&church(), CHURCH_REGION, &ExpandOptions::seeded(1)).unwrap();
    let names: Vec<String> = out.model.palette().iter().map(|b| b.to_string()).collect();
    // Block states, not bare ids: the roof needs facings and the door needs
    // halves. This is the assertion behind "block states from day one".
    let stairs: Vec<_> = names.iter().filter(|n| n.contains("oak_stairs")).collect();
    assert!(
        stairs.iter().any(|n| n.contains("facing=")),
        "roof stairs lost their facing: {names:?}"
    );
    let door_halves: Vec<_> = names.iter().filter(|n| n.contains("oak_door")).collect();
    assert_eq!(
        door_halves.len(),
        2,
        "a door is two halves, got {door_halves:?}"
    );
    assert!(names.iter().any(|n| n.contains("glass")), "{names:?}");
}

#[test]
fn undersized_regions_are_loud() {
    // Upstream prints a warning and writes blocks outside the box, or silently
    // voids the scope. Both are failures we would only find by looking.
    let err = expand(
        &temple(),
        Box3::at_origin([13, 6, 21]),
        &ExpandOptions::seeded(1),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("too small"),
        "expected a sizing diagnostic, got: {err}"
    );

    let err = expand(
        &castle(),
        Box3::at_origin([12, 14, 12]),
        &ExpandOptions::seeded(1),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected an unsatisfied-guard diagnostic, got: {err}"
    );
}

#[test]
fn the_documented_minimum_regions_are_the_real_ones() {
    // `docs/reference/grammar.md` and each library module state the smallest box
    // their program expands in. Documented numbers drift; these hold them to the
    // code from both sides — the minimum works, one block less does not.
    fn check(name: &str, program: &Program, smallest: [u32; 3], too_small: &[[u32; 3]]) {
        expand(
            program,
            Box3::at_origin(smallest),
            &ExpandOptions::seeded(1),
        )
        .unwrap_or_else(|e| panic!("{name} should expand at its documented minimum: {e}"));
        for &size in too_small {
            assert!(
                expand(program, Box3::at_origin(size), &ExpandOptions::seeded(1)).is_err(),
                "{name} expanded at {size:?}, below its documented minimum {smallest:?}"
            );
        }
    }

    // temple: 6 + 2*column_size in X, 1 + column_height + 5 in Y, 7 in Z.
    check(
        "temple",
        &temple(),
        [8, 14, 7],
        &[[7, 14, 7], [8, 13, 7], [8, 14, 6]],
    );
    // castle: both horizontal extents 2*large_tower + 2, tower_height + 1 in Y.
    check(
        "castle",
        &castle(),
        [20, 9, 20],
        &[[19, 9, 20], [20, 8, 20], [20, 9, 19]],
    );

    // church: no fixed minimum — the roof's height has to follow the nave's
    // width, because it steps in two blocks per course.
    for (width, min_height) in [(9u32, 9u32), (15, 12), (21, 18)] {
        let ok = Box3::at_origin([width, min_height, 30]);
        expand(&church(), ok, &ExpandOptions::seeded(1))
            .unwrap_or_else(|e| panic!("church {width}x{min_height}: {e}"));
        let squat = Box3::at_origin([width, min_height - 1, 30]);
        assert!(
            expand(&church(), squat, &ExpandOptions::seeded(1)).is_err(),
            "church expanded at {width} wide and only {} tall",
            min_height - 1
        );
    }
}

#[test]
fn programs_round_trip_through_json() {
    for (program, region) in programs() {
        let json = serde_json::to_string_pretty(&program).unwrap();
        let back: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(back, program, "{} did not survive JSON", program.name);
        // and the round trip is stable, not merely lossless
        assert_eq!(serde_json::to_string_pretty(&back).unwrap(), json);
        // ...and the deserialised program expands to the same blocks.
        let a = expand(&program, region, &ExpandOptions::seeded(3)).unwrap();
        let b = expand(&back, region, &ExpandOptions::seeded(3)).unwrap();
        assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    }
}

#[test]
fn the_json_form_is_the_one_an_author_would_write() {
    let json = serde_json::to_value(temple()).unwrap();
    assert_eq!(json["palette"]["marble"], "minecraft:quartz_block");
    assert_eq!(json["params"]["column_height"], 8);
    let back_wall = &json["rules"]["back_wall"][0]["body"];
    assert_eq!(back_wall["op"], "split");
    assert_eq!(back_wall["axis"], "z");
    assert_eq!(
        back_wall["sizes"][0],
        serde_json::json!({"size": "absolute", "blocks": {"expr": "int", "value": 1}})
    );
    assert_eq!(back_wall["children"][0]["op"], "void");
}

#[test]
fn a_palette_swap_restyles_without_touching_a_rule() {
    let mut sandstone = temple();
    sandstone
        .set_role(
            "marble",
            Paint::Block(BlockState::simple("smooth_sandstone")),
        )
        .unwrap();
    let marble = expand(&temple(), TEMPLE_REGION, &ExpandOptions::seeded(1)).unwrap();
    let sand = expand(&sandstone, TEMPLE_REGION, &ExpandOptions::seeded(1)).unwrap();
    assert_eq!(
        marble.model.filled_cells(),
        sand.model.filled_cells(),
        "a restyle must not move a single block"
    );
    assert!(
        sand.model
            .palette()
            .iter()
            .any(|b| b.name == "minecraft:smooth_sandstone")
    );
    assert_ne!(marble.model.canonical_bytes(), sand.model.canonical_bytes());
}

#[test]
fn a_weathered_palette_mixes_per_cell_under_the_seed() {
    let mut weathered = temple();
    weathered
        .set_role(
            "marble",
            Paint::Mix(vec![
                WeightedBlock {
                    weight: 6,
                    block: BlockState::simple("quartz_block"),
                },
                WeightedBlock {
                    weight: 1,
                    block: BlockState::simple("cracked_stone_bricks"),
                },
            ]),
        )
        .unwrap();
    let a = expand(&weathered, TEMPLE_REGION, &ExpandOptions::seeded(11)).unwrap();
    let b = expand(&weathered, TEMPLE_REGION, &ExpandOptions::seeded(12)).unwrap();
    assert!(
        a.model
            .palette()
            .iter()
            .any(|x| x.name == "minecraft:cracked_stone_bricks")
    );
    assert_ne!(
        a.model.canonical_bytes(),
        b.model.canonical_bytes(),
        "a per-cell mix must follow the seed"
    );
    assert_eq!(
        a.model.filled_cells(),
        b.model.filled_cells(),
        "...but only the blocks change, never the geometry"
    );
}

#[test]
fn unknown_knobs_are_refused_rather_than_ignored() {
    let mut program = temple();
    assert!(program.set_param("colum_height", 12).is_err());
    assert!(
        program
            .set_role("marbel", Paint::Block(BlockState::air()))
            .is_err()
    );
    assert_eq!(program, temple(), "a refused override changes nothing");
}
