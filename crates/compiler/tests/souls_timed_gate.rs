//! spec-0016 §4 (timed gates) end-to-end tests, driven by the `souls-timed-gate`
//! fixture: hello-world's inner door put on a 60-open / 40-closed clock, with the
//! same stage-7 `carve` bypass the shortcut fixture uses (so sealing the door for
//! half of every cycle never strands the critical path). A clean build is the
//! `DW0378` ≥20%-of-cycle proof on real prefab geometry.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-timed-gate";

fn build_fixture() -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-timed-gate parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        diags.is_empty(),
        "souls-timed-gate must validate clean: {diags:#?}"
    );
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates (DW0378 holds on the fixture)")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// The clock is a self-sustaining two-function ping-pong: each half does its
/// world edit and schedules the other. No per-tick polling, no state to drift,
/// and `schedule` is replace-mode so it can never double up.
#[test]
fn the_clock_is_a_self_sustaining_ping_pong() {
    let out = build_fixture();
    assert_eq!(
        fn_body(&out, "tgate_open_inner_door")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "fill 4 65 6 5 67 6 minecraft:air replace minecraft:iron_bars",
            &format!("schedule function {NS}:tgate_close_inner_door 60t"),
        ],
        "open clears the region with the anchor's declared block, then arms the close"
    );
    assert_eq!(
        fn_body(&out, "tgate_close_inner_door")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "fill 4 65 6 5 67 6 minecraft:iron_bars",
            &format!("schedule function {NS}:tgate_open_inner_door 40t"),
        ],
        "close seals it back, then arms the open"
    );
    // Neither half is on the tick — the chain carries itself.
    let tick = fn_body(&out, "tick");
    assert!(
        !tick.contains("tgate_"),
        "a timed gate costs nothing per tick: {tick}"
    );
}

/// The gate is sealed by the prefab at world-load, so the clock's first act is an
/// OPEN. A `phase` of 0 opens immediately; a larger one holds it shut first.
#[test]
fn setup_starts_the_clock_on_an_open() {
    let out = build_fixture();
    let setup = fn_body(&out, "setup_finish");
    assert!(
        setup.contains(&format!("function {NS}:tgate_open_inner_door")),
        "the clock starts on an open: {setup}"
    );
    assert!(
        !setup.contains("schedule function souls-timed-gate:tgate_open_inner_door"),
        "phase 0 opens immediately rather than scheduling: {setup}"
    );
}

/// The generated PackTest drives both halves of the real clock on a live server
/// and asserts the region's block after each — the machine-checkable half of
/// "a deterministic clock over the gate region".
#[test]
fn clock_runtime_behaviour_is_packtested() {
    let out = build_fixture();
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_timed_gate.mcfunction"
        ))
        .expect("timed-gate PackTest emitted"),
    )
    .unwrap();
    assert!(
        t.contains(&format!("function {NS}:tgate_open_inner_door"))
            && t.contains(&format!("function {NS}:tgate_close_inner_door")),
        "the template drives BOTH halves of the real clock: {t}"
    );
    for score in ["#tg_sealed", "#tg_open", "#tg_shut"] {
        assert!(
            t.contains(&format!("assert score {score} dw.sys matches 1")),
            "missing the {score} assertion: {t}"
        );
    }
}

/// The validation metadata the RUNTIME rung needs (task #81). The static proof
/// (`DW0378`) already shows the window is readable; the harness bot could not act on
/// it, because nothing told it a gate existed — so the portcullis filling mid-approach
/// aborted the pathfinder and failed the leg as if the geometry were broken. The
/// waypoints artifact therefore carries the gate table (region + clock) and marks every
/// leg whose proven route walks through one.
#[test]
fn waypoints_artifact_exports_the_gate_table_and_marks_the_crossing_leg() {
    let out = build_fixture();
    let raw = out
        .get("validation/critical-path-waypoints.json")
        .expect("waypoints artifact emitted (the fixture walks a leg)");
    let v: serde_json::Value = serde_json::from_slice(raw).expect("valid JSON");

    let gates = v["timed_gates"].as_array().expect("gate table exported");
    assert_eq!(gates.len(), 1, "one declared timed gate: {gates:#?}");
    let g = &gates[0];
    assert_eq!(g["id"], "timed-gate/inner-door");
    assert_eq!(g["open_ticks"], 60);
    assert_eq!(g["closed_ticks"], 40);
    assert_eq!(g["phase"], 0);
    assert_eq!(g["block"], "minecraft:iron_bars");
    // Canonical inclusive bbox, min ≤ max componentwise — the harness reads every
    // cell of it to observe the open/closed edge.
    let min = g["region"]["min"].as_array().expect("region min");
    let max = g["region"]["max"].as_array().expect("region max");
    for axis in 0..3 {
        assert!(
            min[axis].as_i64() <= max[axis].as_i64(),
            "region min must not exceed max on axis {axis}: {g:#?}"
        );
    }

    // The fixture's walked leg runs straight through the door column, so it is marked
    // — and the mark names a gate the table declares.
    let legs = v["legs"].as_array().expect("legs");
    let crossing: Vec<&serde_json::Value> = legs
        .iter()
        .filter(|l| l.get("timed_gates").is_some())
        .collect();
    assert!(
        !crossing.is_empty(),
        "the fixture's critical path walks through the timed door: {legs:#?}"
    );
    for leg in crossing {
        assert_eq!(
            leg["timed_gates"],
            serde_json::json!(["timed-gate/inner-door"]),
            "a marked leg names declared gates, in declared order: {leg:#?}"
        );
    }
}
