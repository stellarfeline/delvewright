//! The exported player metrics are the navigation model's own constants
//! (spec-0049 §2.2, acceptance 1).
//!
//! # Why this measures behaviour and not two constants
//!
//! The obvious test — `assert_eq!(exported_walk_up, nav::MAX_AUTO_STEP_16)` —
//! is worth nothing here, because after this round they are the same `const`
//! item: it would compare a value with itself and pass on any number at all.
//! Rust already refuses the state it was meant to catch, and refuses it harder
//! than a test could: a module cannot both `use` a name and declare it (`E0252`),
//! so `compiler::nav` importing the step rule's bounds from `dsl::metrics` cannot
//! also keep private copies. "One definition, not two agreeing" is a compile
//! error, not an assertion.
//!
//! What is left unproven by that, and is what this file proves, is the join
//! between the number and the MODEL: that the bounds the table publishes to the
//! world are the bounds the routing model actually walks at. Each test builds
//! its geometry **from the exported figure** and asks the model what it reaches,
//! so the guarded failure is a step rule that stops honouring its own published
//! number — an inlined literal, a comparison flipped from `>` to `>=`, a
//! footprint change, a new guard in front of the rise test. That is the failure
//! that costs something: a creator sizing a stair to a figure `delvec metrics`
//! publishes and finding the engine will not walk it.
//!
//! **What these tests cannot see, stated because the first draft assumed
//! otherwise and was measured.** Moving the table alone does NOT red them, and
//! that was confirmed by doing it: the ceiling was perturbed to 21 and all four
//! stayed green, because the model imports the same constant, so the fixture and
//! the rule moved together. A second method that shares the first's calibration
//! is not a second method. Here it is also the correct behaviour rather than a
//! hole — a coordinated move IS the table being the authority, which is the
//! whole point — but the limit belongs on the page and not in an assumption.
//! Flipping the step rule's own comparison, which shares nothing with the
//! fixture, reds `the_published_jump_ceiling_is_where_the_model_stops` on the
//! spot.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_compiler::assembled::Occupancy;
use delvewright_compiler::nav::World;
use delvewright_dsl::metrics::{self, MAX_AUTO_STEP_16, MAX_JUMP_RISE_16, METRICS_VERSION, Metrics};

/// Two adjacent ledges whose walk planes differ by exactly `rise` sixteenths,
/// with air everywhere else — so the only thing that can stop a body crossing is
/// the step rule the table publishes.
///
/// The rise is built out of two **partial floors** one cell apart, and that is
/// forced rather than stylistic. The model's vertical candidates are `{0, −1,
/// +1}` **cells**, deliberately: a `+2`-cell move can be physically legal
/// between two very thin floors, and leaving it out only ever refuses a route,
/// never proves one. So every rise this fixture can express — one sixteenth to
/// nearly two blocks — is expressed within one cell of vertical travel, by
/// lowering the source's floor or raising the destination's. A slab, a snow
/// layer and a path lip are exactly this to the model.
fn two_ledges(rise: i64) -> (World, [i32; 3], [i32; 3]) {
    assert!(
        (1..=31).contains(&rise),
        "the fixture expresses a rise within one cell of travel: 1..=31 sixteenths"
    );
    // `rise = 16 + destination_floor − source_floor`, both floors in sixteenths
    // of their own cell. Below a block, drop the destination's floor; above one,
    // drop the source's.
    let (source_floor, dest_floor) = if rise <= 16 {
        (16, rise as u8)
    } else {
        ((32 - rise) as u8, 16)
    };

    let mut solid: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut partial: BTreeMap<[i32; 3], u8> = BTreeMap::new();

    // The near ledge: two cells so the body has somewhere to stand cardinally
    // adjacent to the far column rather than having to route around it.
    for z in 0..2 {
        let cell = [0, 63, z];
        solid.insert(cell);
        if source_floor < 16 {
            partial.insert(cell, source_floor);
        }
    }
    let start = [0, 64, 0];

    // The far ledge, one cell up.
    let cap = [1, 64, 1];
    solid.insert(cap);
    if dest_floor < 16 {
        partial.insert(cap, dest_floor);
    }
    let goal = [1, 65, 1];

    let world = World::from_occupancy(Occupancy {
        solid,
        tall: BTreeSet::new(),
        use_gates: BTreeSet::new(),
        flooded: BTreeSet::new(),
        partial,
    });
    (world, start, goal)
}

fn crosses(rise: i64) -> bool {
    let (world, start, goal) = two_ledges(rise);
    world.reachable_walkable(&[start]).contains(&goal)
}

/// `step.walk-up` is the largest rise the model crosses, and one sixteenth more
/// is a different move.
#[test]
fn the_published_walk_up_is_the_rise_the_model_actually_walks_up() {
    assert!(
        crosses(MAX_AUTO_STEP_16),
        "the table publishes a walk-up budget of {MAX_AUTO_STEP_16}/16 and the navigation model \
         will not cross it — a creator sizing a step to the published number would build a wall"
    );
}

/// `step.jump-rise` is the ceiling, and it is a real ceiling: one sixteenth over
/// it, the model refuses.
#[test]
fn the_published_jump_ceiling_is_where_the_model_stops() {
    assert!(
        crosses(MAX_JUMP_RISE_16),
        "the table publishes a jump ceiling of {MAX_JUMP_RISE_16}/16 and the model will not reach \
         it"
    );
    assert!(
        !crosses(MAX_JUMP_RISE_16 + 1),
        "one sixteenth over the published ceiling is reachable, so the ceiling the table publishes \
         is not the one the model enforces"
    );
}

/// The body the table describes is the body the model routes.
#[test]
fn the_published_body_is_the_body_the_model_routes() {
    let (w, h) = delvewright_compiler::nav::entity_dims("minecraft:player");
    assert_eq!(w, metrics::PLAYER_WIDTH);
    assert_eq!(h, metrics::PLAYER_HEIGHT);
    // And the eye every player-POV camera in this engine stands at.
    assert_eq!(
        delvewright_compiler::render_plan::EYE_HEIGHT,
        metrics::PLAYER_EYE_HEIGHT
    );
    assert_eq!(
        f64::from(delvewright_compiler::view::viewer::EYE_HEIGHT),
        f64::from(metrics::PLAYER_EYE_HEIGHT as f32),
    );
}

/// The export a tool outside the engine reads carries those same figures, and
/// carries them under the keys the reference names.
#[test]
fn the_export_publishes_what_the_model_uses() {
    let table = Metrics::table();
    let json = metrics::export(&table);

    // The CONSTANT, never its current value: a literal here is false the
    // moment the table moves, and the digest test is what holds the number to
    // the bytes.
    assert_eq!(json["metrics_version"], serde_json::json!(METRICS_VERSION));
    assert_eq!(json["mc_version"], serde_json::json!("1.21.11"));

    let player = &json["player"];
    assert_eq!(
        player["step.walk-up"]["value"],
        serde_json::json!(MAX_AUTO_STEP_16)
    );
    assert_eq!(
        player["step.jump-rise"]["value"],
        serde_json::json!(MAX_JUMP_RISE_16)
    );
    assert_eq!(
        player["body.width"]["value"],
        serde_json::json!(metrics::PLAYER_WIDTH)
    );
    assert_eq!(
        player["body.height"]["value"],
        serde_json::json!(metrics::PLAYER_HEIGHT)
    );
    assert_eq!(
        player["body.eye-height"]["value"],
        serde_json::json!(metrics::PLAYER_EYE_HEIGHT)
    );

    // Every player entry states where its number came from, and none of them
    // claims to be a project standard awaiting a walk: a fact of the game is not
    // something the metrics gym can rule on.
    for (key, entry) in player.as_object().expect("the player half is an object") {
        assert!(
            entry.get("calibrated").is_none(),
            "`{key}` carries a calibration flag, but a player metric is measured rather than chosen"
        );
        assert_ne!(
            entry["provenance"],
            serde_json::json!("provisional"),
            "`{key}` is a player metric and cannot be provisional"
        );
        assert!(
            !entry["note"].as_str().unwrap_or_default().is_empty(),
            "`{key}` states no provenance in prose, so its number is unsourced"
        );
    }

    // Every building entry carries one, and every one of them is false at this
    // version — the gym has not been walked.
    let building = json["building"]
        .as_object()
        .expect("the building half is an object");
    assert!(!building.is_empty());
    for (key, entry) in building {
        assert_eq!(
            entry["calibrated"],
            serde_json::json!(false),
            "`{key}` claims to be calibrated"
        );
    }
    assert_eq!(
        json["counts"]["uncalibrated"],
        serde_json::json!(building.len())
    );
}

/// `delvec metrics` exits 0, puts the table on stdout and its verdicts on
/// stderr, and states its binding count either way (spec-0049 acceptance 1).
///
/// The stdout/stderr split is load-bearing rather than tidy: the export is what
/// a tool outside the engine reads, so a diagnostic line mixed into it would
/// make the JSON unparseable for exactly the consumer the single-authority rule
/// exists to serve.
#[test]
fn the_cli_publishes_the_table_and_states_what_its_verdicts_bound_to() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("metrics")
        .output()
        .expect("`delvec metrics` runs");
    assert!(
        out.status.success(),
        "`delvec metrics` exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let table = Metrics::table();
    let mut expected = serde_json::to_string_pretty(&metrics::export(&table)).expect("serializes");
    expected.push('\n');
    assert_eq!(
        String::from_utf8(out.stdout).expect("stdout is UTF-8"),
        expected,
        "stdout is the table and nothing else"
    );

    let err = String::from_utf8(out.stderr).expect("stderr is UTF-8");
    assert!(
        err.contains("player metric(s)") && err.contains("building metric(s)"),
        "the run states what the table holds: {err}"
    );
    assert!(
        err.contains("self-check binding:") && err.contains("invariant(s)"),
        "the run states what its verdicts bound to: {err}"
    );
    assert!(
        err.contains("DW0813"),
        "every building metric is a seed at this version, so the run owes the notice: {err}"
    );
}
