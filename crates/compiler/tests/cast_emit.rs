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
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
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

/// A function from the cast-ledger fixture's namespace.
fn func(out: &BuildOutput, name: &str) -> String {
    text(out, &format!("datapack/data/cast-ledger/function/{name}"))
}

/// A function from hello-world's namespace (the no-ledger control).
fn hw_func(out: &BuildOutput, name: &str) -> String {
    text(out, &format!("datapack/data/hello-world/function/{name}"))
}

/// The retirement mechanism: right-click resolves through the scene selector, and
/// each beat's declared root is the one that beat shows.
#[test]
fn declared_root_swaps_between_beats() {
    let out = ledger();
    let talk = func(&out, "talk_keeper.mcfunction");
    assert!(
        talk.contains("function cast-ledger:cast_keeper"),
        "talk must dispatch through the scene selector:\n{talk}"
    );
    // Scene 1 = the premise root; scene 2 = the farewell root.
    assert!(
        talk.contains(
            "execute if score @s dw.cast matches 2 run dialog show @s cast-ledger:keeper_farewell"
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
        talk.contains("advancement revoke @s only cast-ledger:sleeper_interact"),
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
            "execute if score @s dw.cast matches 1 run function cast-ledger:bark_sleeper_1"
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
        !hw_func(&build(&common::hello_world_dir()), "setup.mcfunction").contains("dw.cast"),
        "a campaign with no ledger must not declare the selector"
    );
}

/// A campaign that declares no `cast` emits exactly what it always did: the
/// single-line root show, with no selector and no bark machinery.
#[test]
fn no_ledger_means_byte_identical_talk_functions() {
    let out = build(&common::hello_world_dir());
    let talk = hw_func(&out, "talk_keeper.mcfunction");
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

/// The per-NPC `talk_` template asserts the cast dispatch **exactly when the
/// campaign authored a ledger for that NPC** — the general form of a live red.
///
/// `talk_<npc>` has two shapes (`cast_dispatch`): with a ledger it opens
/// `function <ns>:cast_<npc>`, and with none it is the single root line it always
/// was. `DW0811`'s `npc-talk` claim drives every NPC's body, and its second
/// assertion — that the dispatch really ran, read off `dw.cast` — has a subject
/// only in the first shape. Asserted unconditionally it is a claim about a line
/// this campaign's body neither has nor should have, and it reddened hello-world
/// on the live server: one NPC, no ledger, `dw.cast` never written.
///
/// The gate is the AUTHORED ledger (`cast::npc_casts`), never the emitted body.
/// Gating on the body would be an opt-out the defect itself supplies — a `talk_`
/// that lost its dispatch line would lose the assertion about it in the same
/// stroke — which is the vacuity mode `CLAUDE.md` names sixth. This test is what
/// keeps that gate honest in both directions: present for a ledger NPC, absent
/// for one without.
#[test]
fn the_talk_template_claims_a_cast_dispatch_only_where_one_is_emitted() {
    // No ledger: the template still proves the body loads and re-arms, and makes
    // no claim about a dispatch that does not exist.
    let hw = build(&common::hello_world_dir());
    let t = std::str::from_utf8(
        &hw["packtest-datapack/data/hello-world/test/npc_talk_keeper.mcfunction"],
    )
    .unwrap();
    assert!(
        t.contains("run function hello-world:talk_keeper"),
        "the body is still driven per NPC:\n{t}"
    );
    assert!(
        t.contains("assert score #tlk_keeper dw.sys matches 1"),
        "the re-arm claim survives without a ledger:\n{t}"
    );
    assert!(
        !t.contains("dw.cast"),
        "a campaign with no ledger must carry no dispatch claim:\n{t}"
    );

    // With a ledger: the dispatch claim is present, and reads `dw.cast` after
    // actively clearing it, so a score at all is the proof the selector ran.
    let led = ledger();
    let c = std::str::from_utf8(
        &led["packtest-datapack/data/cast-ledger/test/npc_talk_keeper.mcfunction"],
    )
    .unwrap();
    assert!(
        c.contains("scoreboard players reset")
            && c.contains("dw.cast")
            && c.contains("assert score #cst_keeper dw.sys matches 1"),
        "a ledger NPC's template clears `dw.cast` and then claims the dispatch \
         wrote it:\n{c}"
    );
}

/// The three cast PackTest templates are emitted for a ledger campaign.
#[test]
fn cast_packtests_are_emitted() {
    let out = ledger();
    for name in ["cast_root_swap", "cast_bark_cycle", "cast_none_silent"] {
        let p = format!("packtest-datapack/data/cast-ledger/test/{name}.mcfunction");
        assert!(out.contains_key(&p), "missing packtest template {name}");
    }
}

/// A cast template's dispatch assertion must be batch-order-free. Zeroing every
/// `dw.qa_*` a template reads is not enough on its own: leaving the ledger's
/// branch-gate flags (`requires_flags` / `forbids_flags`) to whatever the batch
/// has lets a sibling verb template that legitimately ends with a campaign flag
/// set to 1 make `cast_root_swap`'s later assert read the OTHER branch's clause
/// (expected `dw.cast 2`, got 3). The consumer pins every flag its ledger
/// reads to the value that selects the asserted scene — the generator-side
/// defense that holds against any future flag-setting template.
#[test]
fn cast_root_swap_pins_the_branch_gate_flags_it_asserts_under() {
    let out = build(&common::compiler_fixtures_dir().join("branch-two-endings"));
    let t = text(
        &out,
        "packtest-datapack/data/hello-world/test/cast_root_swap.mcfunction",
    );

    // Phase 1 asserts the ungated pre-fork root: every ledger flag pinned 0.
    // Phase 2 asserts the hold-branch root (`requires flag/wait`): wait pinned
    // 1, the sibling branch's flee pinned 0 — the exact poison of island r15.
    let first_assert = t.find("assert score").expect("first assert present");
    let wait_zero = t
        .find("scoreboard players set #party dw.f_wait 0")
        .expect("phase 1 pins wait to 0");
    let flee_zero = t
        .find("scoreboard players set #party dw.f_flee 0")
        .expect("phase 1 pins flee to 0");
    assert!(
        wait_zero < first_assert && flee_zero < first_assert,
        "phase 1's pins precede its assert:\n{t}"
    );
    let wait_one = t
        .find("scoreboard players set #party dw.f_wait 1")
        .expect("phase 2 pins wait to 1");
    let flee_zero_again = t
        .rfind("scoreboard players set #party dw.f_flee 0")
        .expect("phase 2 re-pins flee to 0");
    let last_assert = t.rfind("assert score").expect("second assert present");
    assert!(
        first_assert < wait_one && wait_one < last_assert,
        "phase 2's requires-pin sits between the two asserts:\n{t}"
    );
    assert!(
        first_assert < flee_zero_again && flee_zero_again < last_assert,
        "phase 2 re-pins the sibling branch's flag to 0:\n{t}"
    );
    // The pin must precede the phase's dispatch drive, not merely its assert.
    let last_drive = t
        .rfind("run function hello-world:cast_")
        .expect("dispatch driven");
    assert!(
        wait_one < last_drive && flee_zero_again < last_drive,
        "phase 2 pins land before the dispatch runs:\n{t}"
    );
}

/// A ledger with no branch-gated clause emits no pin lines at all.
#[test]
fn unbranched_cast_templates_emit_no_flag_pins() {
    let out = ledger();
    for name in ["cast_root_swap", "cast_none_silent"] {
        let t = text(
            &out,
            &format!("packtest-datapack/data/cast-ledger/test/{name}.mcfunction"),
        );
        assert!(
            !t.contains("dw.f_"),
            "{name} must not touch any flag for an ungated ledger:\n{t}"
        );
    }
}
