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

/// The cast PackTest templates emitted for a ledger campaign: one ladder proof
/// PER LEDGER NPC (never an exemplar — the search that picks an exemplar is
/// the search that skips the rest), plus the bark-cycle and silent files.
#[test]
fn cast_packtests_are_emitted_per_npc() {
    let out = ledger();
    for name in [
        "cast_ladder_keeper",
        "cast_ladder_sleeper",
        "cast_bark_cycle",
        "cast_none_silent",
    ] {
        let p = format!("packtest-datapack/data/cast-ledger/test/{name}.mcfunction");
        assert!(out.contains_key(&p), "missing packtest template {name}");
    }
}

/// The ladder proof asserts scene 0 (no declaring quest begun), then every
/// clause's own scene in ladder order with the earlier quests still active —
/// which is the retirement mechanism proven per clause, not on one exemplar
/// pair.
#[test]
fn the_ladder_proof_asserts_every_clause_and_the_default() {
    let out = ledger();
    let t = text(
        &out,
        "packtest-datapack/data/cast-ledger/test/cast_ladder_keeper.mcfunction",
    );
    // Phase order carries the claim: default 0, then scene 1 (only `ask`
    // begun), then scene 2 with BOTH begun (`dw.qa_*` is never cleared).
    let i0 = t.find("dw.cast matches 0").expect("default phase");
    let i1 = t.find("dw.cast matches 1").expect("clause 1 phase");
    let i2 = t.find("dw.cast matches 2").expect("clause 2 phase");
    assert!(i0 < i1 && i1 < i2, "phases in ladder order:\n{t}");
    let both_active = t
        .rfind("scoreboard players set #party dw.qa_ask 1")
        .expect("ask still active in the later phase");
    assert!(
        both_active > i1 && both_active < i2,
        "the later phase keeps the earlier quest active — retirement, not reset:\n{t}"
    );
    // Every ledger NPC gets its own file; the sleeper's ladder is not the
    // keeper's.
    let s = text(
        &out,
        "packtest-datapack/data/cast-ledger/test/cast_ladder_sleeper.mcfunction",
    );
    assert!(
        s.contains("cast-ledger:cast_sleeper") && !s.contains("cast_keeper"),
        "each NPC's ladder drives its own selector:\n{s}"
    );
}

/// A cast template's dispatch assertion must be batch-order-free (island r15):
/// every phase pins EVERY flag the ladder reads — the branch being asserted to
/// 1, the sibling branch to 0 — before the dispatch runs, and each branch
/// clause is asserted under its own drive.
#[test]
fn the_ladder_pins_every_flag_and_proves_each_branch() {
    let out = build(&common::compiler_fixtures_dir().join("branch-two-endings"));
    let t = text(
        &out,
        "packtest-datapack/data/hello-world/test/cast_ladder_keeper.mcfunction",
    );
    // The wait branch's phase: wait 1, flee 0, then the dispatch, then scene 3.
    let wait_on = t
        .find("scoreboard players set #party dw.f_wait 1")
        .expect("wait branch pinned on");
    let wait_assert = t.find("dw.cast matches 3").expect("wait branch asserted");
    assert!(wait_on < wait_assert, "pin precedes the assert:\n{t}");
    let flee_zero_before = t[..wait_assert]
        .rfind("scoreboard players set #party dw.f_flee 0")
        .expect("sibling branch pinned OFF in the same phase — the r15 poison");
    assert!(flee_zero_before > wait_on.saturating_sub(200));
    // The flee branch's phase: flee 1, wait 0, scene 4.
    let flee_on = t
        .find("scoreboard players set #party dw.f_flee 1")
        .expect("flee branch pinned on");
    let flee_assert = t.find("dw.cast matches 4").expect("flee branch asserted");
    assert!(wait_assert < flee_on && flee_on < flee_assert);
    // The pin must precede the phase's dispatch drive, not merely its assert.
    let last_drive = t[..flee_assert]
        .rfind("run function hello-world:cast_")
        .expect("dispatch driven");
    assert!(
        flee_on < last_drive,
        "pins land before the dispatch runs:\n{t}"
    );
    // And each required term is broken on its own, falling back to the bark
    // scene the model names.
    assert!(
        t.contains("# required `flag/wait` broken: the ladder must fall to scene 2"),
        "the wait term's negative phase names the model's answer:\n{t}"
    );
}

/// Two clauses of ONE quest differing only by `requires_state` — the case a
/// flag pin cannot distinguish, and the reason the drive is SOLVED: the
/// fallback clause is asserted under the value that VIOLATES the sibling's
/// comparison, the gated clause under the boundary that satisfies it, and the
/// broken-term phase must fall back to the fallback's scene.
#[test]
fn state_only_siblings_are_distinguished_by_a_solved_drive() {
    let out = build(&common::compiler_fixtures_dir().join("cast-state-ladder"));
    let t = text(
        &out,
        "packtest-datapack/data/cast-ledger/test/cast_ladder_keeper.mcfunction",
    );
    // Fallback clause (scene 2): patience driven to 499 — the solver violating
    // the LATER sibling's `at-least 500`, which no flag pin could do.
    let fb_pin = t
        .find("scoreboard players set #party dw.s_patience 499")
        .expect("fallback phase violates the sibling's comparison");
    let fb_assert = t.find("dw.cast matches 2").expect("fallback asserted");
    assert!(fb_pin < fb_assert, "drive precedes assert:\n{t}");
    // Gated clause (scene 3): patience at the satisfying boundary.
    let on_pin = t
        .find("scoreboard players set #party dw.s_patience 500")
        .expect("gated phase satisfies its comparison");
    let on_assert = t.find("dw.cast matches 3").expect("gated clause asserted");
    assert!(
        fb_assert < on_pin && on_pin < on_assert,
        "phase order:\n{t}"
    );
    // The negative direction: the state term broken on its own must fall back
    // to the fallback's scene — a selector that lost the `requires_state`
    // condition still answers 3 here and reds on the live server.
    assert!(
        t.contains("at-least 500 broken (driven to 499): the ladder must fall to scene 2"),
        "the broken-term phase names the model's answer:\n{t}"
    );
    let broken = t
        .rfind("scoreboard players set #party dw.s_patience 499")
        .expect("broken-term drive present");
    assert!(
        broken > on_assert,
        "negative phase follows the positive one:\n{t}"
    );
}

/// A ledger with no branch-gated clause emits no flag pins at all — the
/// ladder files and the silent file for an ungated ledger touch no `dw.f_*`.
#[test]
fn unbranched_cast_templates_emit_no_flag_pins() {
    let out = ledger();
    for name in [
        "cast_ladder_keeper",
        "cast_ladder_sleeper",
        "cast_none_silent",
    ] {
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

/// Every bark pool is cycled — both of an NPC's pools, not the first that fit
/// a template — and every silent scene is proven, from the one authored
/// ledger.
#[test]
fn every_pool_and_every_silent_scene_is_driven() {
    let out = ledger();
    let barks = text(
        &out,
        "packtest-datapack/data/cast-ledger/test/cast_bark_cycle.mcfunction",
    );
    assert!(
        barks.contains("function cast-ledger:bark_sleeper_1"),
        "the sleeper's pool is driven:\n{barks}"
    );
    let silent = text(
        &out,
        "packtest-datapack/data/cast-ledger/test/cast_none_silent.mcfunction",
    );
    assert!(
        silent.contains("talk_sleeper") && silent.contains("dw.cast matches 2"),
        "the sleeper's `\"none\"` scene is selected and proven:\n{silent}"
    );
}

/// The suite's claims over the cast families are registered and discharged:
/// `cast-ladder` declares every ledger NPC (from the AUTHORED ledger, so a
/// walk that skips one still declares it — the DW0811 refusal), `cast-bark`
/// declares every bark scene per NPC, and on this fixture nothing is breached.
#[test]
fn cast_claims_are_registered_and_discharged() {
    let out = ledger();
    let claims: serde_json::Value = serde_json::from_slice(
        out.get("validation/watch-claims.json")
            .expect("claims ledger"),
    )
    .unwrap();
    let t = text(
        &out,
        "packtest-datapack/data/cast-ledger/test/cast_ladder_keeper.mcfunction",
    );
    assert!(t.contains("run function cast-ledger:cast_keeper"));
    assert_eq!(
        claims["breaches"].as_array().unwrap().len(),
        0,
        "no cast body may be written and undriven: {claims}"
    );
    // The binding is nonzero — this fixture declares two ledger NPCs and a
    // bark scene, so a zero here would be the vacuous-green shape.
    assert!(
        claims["bodies_judged"].as_u64().unwrap() >= 3,
        "the claim judged the cast bodies: {claims}"
    );
}
