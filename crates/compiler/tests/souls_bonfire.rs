//! spec-0016 §1 (souls-mode bonfires) end-to-end tests, driven by the
//! `souls-bonfire` fixture: the v0.6 checkpoint showcase with its
//! `set-checkpoint` replaced by a `bonfire` (with `on_rest`) and its critical
//! wave marked `respawns_on_rest`. A clean build proves the bonfire inherits the
//! DW0315 (no-stranding) and DW0316 (standable placement) obligations — a
//! bonfire IS a checkpoint to those proofs.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-bonfire";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// Build the fixture. A clean build is itself the DW0315/DW0316 proof for the
/// bonfire (the checkpoint proofs run over `plan.checkpoints`, which a bonfire
/// joins).
fn build_fixture() -> BuildOutput {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-bonfire parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();

    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "souls-bonfire must validate clean: {diags:#?}"
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
    .expect("every emitted command validates (DW0315/DW0316 hold for the bonfire)")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

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

/// A `bonfire` does NOT move the respawn point when its beat fires — it only
/// ARMS the rest affordance. This is the whole difference from `set-checkpoint`
/// (spec-0016 §1): the checkpoint moves when the party rests.
#[test]
fn bonfire_arms_a_rest_affordance_and_does_not_move_the_checkpoint() {
    let out = build_fixture();
    let all = all_functions(&out);
    // The arming line: a guarded interaction summon, never a bare one.
    assert!(
        all.contains(
            "execute unless entity @e[tag=dw_bonfire_0] run summon minecraft:interaction "
        ),
        "the bonfire beat summons its rest affordance, guarded on absence"
    );
    assert!(
        all.contains("Tags:[\"dw_bonfire_0\"]"),
        "the affordance carries the bonfire's stable content-ordered tag"
    );
    // The beat that arms the bonfire must NOT itself carry `spawnpoint @a` — that
    // is the rest function's job.
    let arming = fn_body(&out, "complete_o_slay");
    assert!(
        !arming.contains("spawnpoint @a"),
        "arming a bonfire must not move the respawn point: {arming}"
    );
}

/// Resting moves the party respawn point and mirrors it into `dw:cp` — the same
/// shared contract `set-checkpoint` writes (spec-0013's boundary return reads it).
#[test]
fn resting_moves_the_party_respawn_point() {
    let out = build_fixture();
    let rest = fn_body(&out, "bonfire_rest_0");
    assert!(
        rest.lines().any(|l| l.starts_with("spawnpoint @a ")),
        "rest sets the party spawnpoint: {rest}"
    );
    assert!(
        rest.contains("data modify storage dw:cp pos set value ["),
        "rest mirrors the cell into dw:cp: {rest}"
    );
    assert!(
        rest.contains("scoreboard players set #cp dw.sys 0"),
        "rest marks itself the active checkpoint: {rest}"
    );
}

/// The rest interaction is polled every tick and is deliberately REPEATABLE — a
/// bonfire is rested at many times over a delve (unlike the one-shot trap disarm).
#[test]
fn rest_detection_is_per_tick_and_repeatable() {
    let out = build_fixture();
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(&format!(
            "execute if entity @e[tag=dw_bonfire_0,nbt={{interaction:{{}}}}] run function {NS}:bonfire_rest_0"
        )),
        "tick polls the bonfire's interaction record: {tick}"
    );
    assert!(
        tick.contains("execute as @e[tag=dw_bonfire_0] run data remove entity @s interaction"),
        "the interaction record is consumed so the next rest re-fires: {tick}"
    );
    // No one-shot sentinel guards the dispatch (contrast `#trapdis_<id>`).
    assert!(
        !tick.contains("unless score #bonfire_0"),
        "resting must not be one-shot: {tick}"
    );
}

/// One authored `on_rest` bundle, two audiences (spec-0018). Resting is a PARTY
/// event dispatched from the tick, so its player-facing effects address `@a` —
/// the party rests together. A respawn belongs to the ONE player who died, so the
/// same bundle addresses `@s` there. Party state (`set-flag`) names no player on
/// either path and fires exactly once.
#[test]
fn on_rest_runs_at_the_right_audience_on_both_paths() {
    let out = build_fixture();
    let rest = fn_body(&out, "bonfire_rest_0");
    let respawn = fn_body(&out, "cp_on_respawn_0");
    assert!(
        rest.contains("tellraw @a {\"text\":\"You rest at the shrine fire.\"}"),
        "the whole party sees the rest: {rest}"
    );
    assert!(
        respawn.contains("tellraw @s {\"text\":\"You rest at the shrine fire.\"}"),
        "only the player who died sees it on the respawn path: {respawn}"
    );
    for (label, body) in [("rest", rest), ("respawn", respawn)] {
        assert!(
            body.contains("scoreboard players set #party dw.f_rested 1"),
            "the on_rest set-flag is party state on the {label} path: {body}"
        );
    }
}

/// A `respawns_on_rest` wave is re-seated on every rest and on every respawn at a
/// bonfire — but only once the party has actually met it (the seated sentinel).
#[test]
fn respawns_on_rest_wave_is_reseated_by_rest_and_respawn() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_guards");
    assert!(
        spawn.contains("scoreboard players set #wseat_guards dw.sys 1"),
        "spawning the wave marks it seated: {spawn}"
    );
    let reseat = fn_body(&out, "wave_reseat_guards");
    assert_eq!(
        reseat.lines().collect::<Vec<_>>(),
        vec![
            "kill @e[tag=dw_wave_guards]",
            &format!("function {NS}:spawn_guards")
        ],
        "the re-seat clears survivors then re-runs the authored spawn"
    );
    let guard = format!(
        "execute if score #wseat_guards dw.sys matches 1 run function {NS}:wave_reseat_guards"
    );
    assert!(
        fn_body(&out, "bonfire_rest_0").contains(&guard),
        "a rest re-seats the wave"
    );
    assert!(
        fn_body(&out, "cp_on_respawn_0").contains(&guard),
        "a respawn at the bonfire re-seats it too"
    );
    // An unmarked wave is never re-seated.
    assert!(
        !all_functions(&out).contains("wave_reseat_ambush"),
        "only a `respawns_on_rest` wave gets a re-seat function"
    );
}

/// The generated PackTest suite covers both runtime behaviours the bonfire adds:
/// a rest moves the party checkpoint, and a rest re-seats a met wave (and only a
/// met one). Batch-model compliant: each template clears its own entity/score
/// residue at entry and exit.
#[test]
fn bonfire_runtime_behaviour_is_packtested() {
    let out = build_fixture();
    let rest = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_bonfire_rest.mcfunction"
        ))
        .expect("bonfire rest PackTest emitted"),
    )
    .unwrap();
    assert!(
        rest.contains(&format!("function {NS}:bonfire_rest_0")),
        "the template drives the REAL rest function: {rest}"
    );
    assert!(
        rest.contains("data modify storage dw:cp pos set value [0, 0, 0]")
            && rest.matches("assert score").count() == 3,
        "the mirror is scrubbed then asserted on all three axes: {rest}"
    );

    let reseat = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_bonfire_reseat.mcfunction"
        ))
        .expect("bonfire re-seat PackTest emitted"),
    )
    .unwrap();
    assert!(
        reseat.contains("assert score #bu_bfs dw.sys matches 0"),
        "an unmet wave is not conjured by a rest: {reseat}"
    );
    assert!(
        reseat.contains("assert score #br_bfs dw.sys matches 2"),
        "a met, wiped wave stands again at its authored count after a rest: {reseat}"
    );
    assert!(
        reseat
            .trim_end()
            .ends_with("scoreboard players set #wseat_guards dw.sys 0"),
        "the template leaves no residue for the shared batch: {reseat}"
    );
}
