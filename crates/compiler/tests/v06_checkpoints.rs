//! DSL v0.6 (spec-0012 checkpoints + spec-0014 stealth) end-to-end emission
//! tests, driven by the `v06-checkpoints` fixture: it adds a `set-checkpoint`
//! (with `on_respawn`) and a `begin-stealth`/`end-stealth` beat to the v0.4
//! showcase, so the whole pipeline — validation, the DW0315/DW0316/DW0327 proofs,
//! and emission — is exercised on a real campaign.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "v06-checkpoints";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// Build the v0.6 checkpoints fixture, returning the build output. A clean build
/// also proves the DW0315 (no-stranding), DW0316 (placement) and DW0327
/// (stealth-zone) obligations pass on the fixture.
fn build_fixture() -> BuildOutput {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("v06-checkpoints parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();

    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "v06-checkpoints must validate clean: {diags:#?}"
    );

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(dir.join("skins").join(format!("{}.png", skin.texture_id)))
                .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &skins,
    )
    .expect("every emitted command validates (DW0315/DW0316/DW0327 proofs pass)")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(out.get(&path).unwrap_or_else(|| panic!("missing fn {name}"))).unwrap()
}

/// Every shipped-datapack function body concatenated (search convenience). The
/// `packtest-datapack/` suite is excluded so counts reflect the delve itself.
fn all_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

/// `set-checkpoint` emits the party-wide `spawnpoint @a`, the `dw:cp pos` mirror,
/// and — because a checkpoint carries `on_respawn` — the active-checkpoint marker.
/// Exactly one `spawnpoint` is emitted for the single declared checkpoint.
#[test]
fn set_checkpoint_emits_spawnpoint_storage_and_marker() {
    let out = build_fixture();
    let all = all_functions(&out);
    let spawnpoints = all.matches("spawnpoint @a ").count();
    assert_eq!(spawnpoints, 1, "exactly one spawnpoint for one checkpoint");
    assert!(
        all.contains("data modify storage dw:cp pos set value ["),
        "checkpoint mirrors coords into storage dw:cp"
    );
    assert!(
        all.contains("scoreboard players set #cp dw.sys 0"),
        "active-checkpoint marker set for checkpoint index 0"
    );
}

/// Setup initialises the `dw:cp pos` mirror to the spawn cell, declares the
/// vanilla `deathCount` respawn-detection score, and the stealth scores.
#[test]
fn setup_initialises_checkpoint_and_stealth_scores() {
    let out = build_fixture();
    let setup = fn_body(&out, "setup");
    assert!(
        setup.contains("scoreboard objectives add dw.deaths deathCount"),
        "deathCount respawn detector declared"
    );
    assert!(
        setup.contains(
            "scoreboard objectives add dw.st_sneak minecraft.custom:minecraft.sneak_time"
        ),
        "sneak_time stat objective declared"
    );
    assert!(
        setup.contains("scoreboard players set #stealth dw.sys 0"),
        "stealth session marker initialised inactive"
    );
    let finish = fn_body(&out, "setup_finish");
    assert!(
        finish.contains("data modify storage dw:cp pos set value ["),
        "dw:cp initialised to the spawn cell in setup_finish"
    );
}

/// The respawn dispatcher fires per-player `on_respawn` on the death-count edge,
/// gated by the active checkpoint.
#[test]
fn on_respawn_dispatch_is_emitted() {
    let out = build_fixture();
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(&format!("execute as @a run function {NS}:cp_respawn_check")),
        "tick drives the respawn check"
    );
    let check = fn_body(&out, "cp_respawn_check");
    assert!(
        check.contains("execute if score @s dw.deaths > @s dw.death_ack"),
        "respawn fires on the death-count edge"
    );
    let fire = fn_body(&out, "cp_respawn_fire");
    assert!(
        fire.contains("execute if score #cp dw.sys matches 0 run function"),
        "dispatch keys on the active checkpoint"
    );
    let hook = fn_body(&out, "cp_on_respawn_0");
    assert!(
        hook.contains("You steady yourself at the shrine"),
        "on_respawn narration emitted"
    );
}

/// `begin-stealth` arms a per-tick judge that requires sneaking (the `sneak_time`
/// stat rose) AND zone membership, tracks grace, and fires `on_caught`.
#[test]
fn stealth_beat_emits_per_tick_judge() {
    let out = build_fixture();
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(&format!(
            "execute if score #stealth dw.sys matches 1 run function {NS}:stealth_tick_1"
        )),
        "tick runs the active beat's judge"
    );
    let begin = fn_body(&out, "stealth_begin_1");
    assert!(
        begin.contains("scoreboard players set #stealth dw.sys 1"),
        "begin activates the session"
    );
    let eval = fn_body(&out, "stealth_eval_1");
    assert!(
        eval.contains("if score @s dw.st_sneak > @s dw.st_sneakack if entity @s[x="),
        "judge requires sneaking-this-tick AND zone membership"
    );
    assert!(
        eval.contains(&format!(
            "execute if score @s dw.st_grace matches 20.. run function {NS}:stealth_caught_1"
        )),
        "grace of 20 ticks before on_caught"
    );
    let caught = fn_body(&out, "stealth_caught_1");
    assert!(
        caught.contains("The wardens spot you!"),
        "on_caught consequence emitted"
    );
    // end-stealth clears the session marker somewhere in the pack.
    assert!(
        all_functions(&out).contains("scoreboard players set #stealth dw.sys 0"),
        "end-stealth clears the session"
    );
}

/// PackTest coverage: a respawn-at-checkpoint test and a stealth kill/spare test
/// are emitted under the packtest datapack.
#[test]
fn packtest_checkpoint_and_stealth_tests_emitted() {
    let out = build_fixture();
    assert!(
        out.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/v06_checkpoint_respawn.mcfunction"
        )),
        "checkpoint respawn packtest emitted"
    );
    assert!(
        out.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/v06_stealth.mcfunction"
        )),
        "stealth packtest emitted"
    );
}

/// Determinism (ADR-0006): a double build is byte-identical, including all v0.6
/// checkpoint/stealth output.
#[test]
fn double_build_is_byte_identical() {
    let a = build_fixture();
    let b = build_fixture();
    assert_eq!(a.len(), b.len(), "same file set");
    for (path, bytes) in &a {
        assert_eq!(b.get(path), Some(bytes), "byte-identical: {path}");
    }
}
