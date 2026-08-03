//! Cast-ledger emission (spec-0020 proof 3): the declaration IS the gate.
//!
//! Asserts on the emitted `.mcfunction` text, where a silent scene's defining
//! property — that it emits **no** action clause — is directly observable and
//! race-free (the PackTest suite cannot assert an absence across interleaved
//! templates; see `emit_cast_packtests`).

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

fn build(dir: &std::path::Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
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
    .expect("emission succeeds")
}

fn ledger() -> BuildOutput {
    build(&common::compiler_fixtures_dir().join("cast-ledger"))
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("missing {path}"))
            .clone(),
    )
    .unwrap()
}

fn func(out: &BuildOutput, name: &str) -> String {
    text(out, &format!("datapack/data/hello-world/function/{name}"))
}

/// The retirement mechanism: right-click resolves through the scene selector, and
/// each beat's declared root is the one that beat shows.
#[test]
fn declared_root_swaps_between_beats() {
    let out = ledger();
    let talk = func(&out, "talk_keeper.mcfunction");
    assert!(
        talk.contains("function hello-world:cast_keeper"),
        "talk must dispatch through the scene selector:\n{talk}"
    );
    // Scene 1 = the premise root; scene 2 = the farewell root.
    assert!(
        talk.contains(
            "execute if score @s dw.cast matches 2 run dialog show @s hello-world:keeper_farewell"
        ),
        "the later beat must show the later root:\n{talk}"
    );
    let sel = func(&out, "cast_keeper.mcfunction");
    assert_eq!(
        sel.trim(),
        "scoreboard players set @s dw.cast 0\n\
         execute if score #party dw.qa_ask matches 1 run scoreboard players set @s dw.cast 1\n\
         execute if score #party dw.qa_leave matches 1 run scoreboard players set @s dw.cast 2",
        "the selector must be pure scoreboard math in quest-DAG order (later beat wins)"
    );
}

/// A `"none"` scene emits no action clause at all: the interaction record is
/// still consumed by the `advancement revoke`, and nothing opens.
#[test]
fn a_silent_scene_emits_no_action_clause() {
    let talk = func(&ledger(), "talk_sleeper.mcfunction");
    assert!(
        talk.contains("advancement revoke @s only hello-world:sleeper_interact"),
        "the interaction record must still be consumed:\n{talk}"
    );
    // Scene 2 is the sleeper's `"none"`: no clause may mention it.
    assert!(
        !talk.contains("dw.cast matches 2"),
        "a silent scene must emit no action clause:\n{talk}"
    );
    // ...while its live sibling scenes do.
    assert!(
        talk.contains(
            "execute if score @s dw.cast matches 1 run function hello-world:bark_sleeper_1"
        ),
        "the bark scene must still dispatch:\n{talk}"
    );
}

/// Barks cycle by an explicit clause ladder — deterministic, no RNG.
#[test]
fn bark_pool_cycles_deterministically() {
    let bark = func(&ledger(), "bark_sleeper_1.mcfunction");
    assert!(bark.contains("scoreboard players add #bk_sleeper_1 dw.sys 1"));
    assert!(
        bark.contains(
            "execute if score #bk_sleeper_1 dw.sys matches 4.. run scoreboard players set #bk_sleeper_1 dw.sys 1"
        ),
        "a 3-line pool must wrap after the third line:\n{bark}"
    );
    for line in [
        "...mm. Not my watch.",
        "...tell the captain I never left.",
        "...mm.",
    ] {
        assert!(bark.contains(line), "missing bark line `{line}`:\n{bark}");
    }
    assert!(
        !bark.contains("random"),
        "bark cycling must never use RNG:\n{bark}"
    );
}

/// The selector objective is declared exactly when a ledger exists.
#[test]
fn cast_objective_is_declared_only_when_a_ledger_exists() {
    assert!(
        func(&ledger(), "setup.mcfunction").contains("scoreboard objectives add dw.cast dummy"),
        "a campaign with a ledger must declare the selector"
    );
    assert!(
        !func(&build(&common::hello_world_dir()), "setup.mcfunction").contains("dw.cast"),
        "a campaign with no ledger must not declare the selector"
    );
}

/// A campaign that declares no `cast` emits exactly what it always did: the
/// single-line root show, with no selector and no bark machinery.
#[test]
fn no_ledger_means_byte_identical_talk_functions() {
    let out = build(&common::hello_world_dir());
    let talk = func(&out, "talk_keeper.mcfunction");
    assert_eq!(
        talk.trim(),
        "advancement revoke @s only hello-world:keeper_interact\n\
         dialog show @s hello-world:keeper_greeting",
        "pre-0.7 emission must be unchanged"
    );
    assert!(
        !out.keys()
            .any(|k| k.contains("/cast_") || k.contains("/bark_")),
        "no ledger must emit no ledger artifacts"
    );
}

/// The three cast PackTest templates are emitted for a ledger campaign.
#[test]
fn cast_packtests_are_emitted() {
    let out = ledger();
    for name in ["cast_root_swap", "cast_bark_cycle", "cast_none_silent"] {
        let p = format!("packtest-datapack/data/hello-world/test/{name}.mcfunction");
        assert!(out.contains_key(&p), "missing packtest template {name}");
    }
}
