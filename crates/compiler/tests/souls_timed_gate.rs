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
