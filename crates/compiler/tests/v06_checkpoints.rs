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
use delvewright_dsl::{
    AnchorId, QuestEffect, SequenceStep, parse_campaign, validate_campaign_with,
};

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
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
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
        setup.contains("scoreboard objectives add dw.st_grace dummy"),
        "stealth grace objective declared"
    );
    // Owner ruling 2026-08-01: zone presence alone = hidden. No sneak stat is
    // tracked anywhere in the pack — the objectives must be gone, not just unused.
    assert!(
        !setup.contains("dw.st_sneak"),
        "no sneak stat objective survives the zone-presence stealth model:\n{setup}"
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
        check.contains("if score @s dw.deaths > @s dw.death_ack"),
        "respawn fires on the death-count edge"
    );
    // task #145: `deathCount` ticks on the DEATH, not on the respawn, so both the
    // fire and the acknowledgement wait for a living player — otherwise the whole
    // bundle would land on the corpse and the edge would be spent.
    //
    // Scoped to the lines that READ the counter, because task #68 added one that
    // deliberately does not: the acknowledgement has to be seeded unconditionally
    // (see below). The invariant this test exists for is unchanged — nothing that
    // touches `dw.deaths` may land on a corpse.
    assert!(
        check
            .lines()
            .filter(|l| l.contains("dw.deaths"))
            .all(|l| l.contains("unless data entity @s {Health:0.0f}")),
        "the death edge is held until the player is alive again:\n{check}"
    );
    // task #68, found live by the bot tier's death-loop stage: `dw.death_ack` is a
    // `dummy` objective, so a player who has never died has NO score in it — and
    // `execute if score @s A > @s B` with B unset does not fire (measured on the
    // pinned 1.21.11 server). The whole respawn dispatch was therefore dead on a
    // player's FIRST death, and worked from the second onward, which is why it
    // survived every manual test that dies twice. The seed must come BEFORE the
    // comparison, so the order is asserted, not merely the presence.
    let seed = check
        .lines()
        .position(|l| l.trim() == "scoreboard players add @s dw.death_ack 0")
        .expect("the acknowledgement is seeded so it EXISTS before it is compared to");
    let read = check
        .lines()
        .position(|l| l.contains("if score @s dw.deaths > @s dw.death_ack"))
        .expect("the edge reads the acknowledgement");
    assert!(
        seed < read,
        "the acknowledgement must be seeded before the edge reads it, or the first \
         death still compares against an unset score:\n{check}"
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

/// task #145 (owner playtest, tide-mill). `spawnpoint` is a hint, not a promise:
/// vanilla re-validates the recorded cell on death and silently falls back to the
/// world spawn — the campaign entrance — whenever the cell or the cell above it is
/// solid or liquid. Past a one-way transport that is an unrecoverable softlock, so
/// the delve re-seats the respawned player on the checkpoint itself.
#[test]
fn respawn_re_seats_the_player_on_the_checkpoint_cell() {
    let out = build_fixture();
    let fire = fn_body(&out, "cp_respawn_fire");
    assert!(
        fire.contains(&format!(
            "execute if score #cp dw.sys matches 0 run function {NS}:cp_seat_0"
        )),
        "the death edge dispatches the re-seat for the active checkpoint:\n{fire}"
    );
    // …and it runs BEFORE the authored `on_respawn` beats, which narrate to a
    // player who is supposed to be standing on the mark.
    let seat = fire.find("cp_seat_0").expect("re-seat dispatched");
    let hook = fire.find("cp_on_respawn_0").expect("hook dispatched");
    assert!(seat < hook, "the re-seat precedes the hooks:\n{fire}");

    // The re-seat lands on the CENTRE of the checkpoint cell — vanilla's own
    // respawn lands at `cell + (0.5, 0.1, 0.5)`, so a correct respawn must not
    // visibly twitch. Coordinates are compiled in: no macro, no storage read.
    let body = fn_body(&out, "cp_seat_0");
    let cell: Vec<i32> = out
        .iter()
        .filter(|(k, _)| k.ends_with(".mcfunction"))
        .filter_map(|(_, v)| String::from_utf8(v.clone()).ok())
        .flat_map(|s| {
            s.lines()
                .filter(|l| l.starts_with("spawnpoint @a "))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .next()
        .expect("a checkpoint records a spawnpoint")
        .trim_start_matches("spawnpoint @a ")
        .split(' ')
        .map(|n| n.parse::<i32>().unwrap())
        .collect();
    assert_eq!(
        body.trim(),
        format!(
            "tp @s {:.1} {} {:.1}",
            cell[0] as f64 + 0.5,
            cell[1],
            cell[2] as f64 + 0.5
        ),
        "the re-seat targets the checkpoint cell's centre"
    );
}

/// The environmental-death PackTest (task #145): the runtime half of the proof —
/// a `deathCount` edge from the campaign entrance must end ON the checkpoint, and
/// must not fire a second time without a second death.
#[test]
fn reseat_packtest_drives_the_death_edge_from_the_entrance() {
    let out = build_fixture();
    let t = std::str::from_utf8(
        &out[&format!("packtest-datapack/data/{NS}/test/v06_checkpoint_reseat.mcfunction")],
    )
    .unwrap();
    assert!(
        t.contains("scoreboard players set @a[tag=dw_t_cpseat,limit=1] dw.deaths 1"),
        "the test drives a real death-count edge:\n{t}"
    );
    assert!(
        t.contains("run function v06-checkpoints:cp_respawn_check"),
        "…through the shipped respawn check:\n{t}"
    );
    assert_eq!(
        t.matches("assert score").count(),
        5,
        "landing (x/y/z) + the ack + the no-second-re-seat guard:\n{t}"
    );
    assert!(
        t.contains("tag @p add dw_t_cpseat") && t.matches("@p").count() == 1,
        "the template pins its own dummy exactly once:\n{t}"
    );
}

/// `begin-stealth` arms a per-tick judge that requires zone membership alone
/// (owner ruling 2026-08-01: no sneak requirement — holding sneak collided with
/// the spectator cutscene camera), tracks grace, and fires `on_caught`.
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
        eval.contains("execute if entity @s[x="),
        "judge tests zone membership (a pure position selector)"
    );
    assert!(
        !eval.contains("dw.st_sneak"),
        "the judge must not read any sneak state — zone presence alone = hidden:\n{eval}"
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
    let stealth = std::str::from_utf8(
        &out[&format!("packtest-datapack/data/{NS}/test/v06_stealth.mcfunction")],
    )
    .unwrap();
    // The test drives `stealth_eval` explicitly, so it must DISARM the live session
    // marker after each `stealth_begin` — otherwise the world `tick` loop fires a
    // second judge pass in the same tick, double-counting exposure and corrupting
    // the grace the asserts read (the failure that surfaced on the first live run).
    assert_eq!(
        stealth
            .matches("scoreboard players set #stealth dw.sys 0")
            .count(),
        2,
        "each stealth_begin is followed by a session disarm: {stealth}"
    );
    for (begin, disarm) in stealth
        .match_indices(&format!("function {NS}:stealth_begin_1"))
        .zip(stealth.match_indices("scoreboard players set #stealth dw.sys 0"))
    {
        assert!(
            disarm.0 > begin.0,
            "disarm follows its stealth_begin: {stealth}"
        );
    }
    // PackTest spawns one dummy PER test and runs the whole suite as one batch
    // on one shared server (round-5 island red): the template must pin its own
    // dummy by tag as its first act and never address a player through bare
    // `@p`/`@a` again — after the template tp's its dummy to campaign
    // coordinates, `@p` retargets to a neighbor test's dummy.
    assert!(
        stealth.contains("tag @p add dw_sttest"),
        "stealth test pins its dummy: {stealth}"
    );
    assert_eq!(
        stealth.matches("@p").count(),
        1,
        "the pin is the only `@p` in the stealth test: {stealth}"
    );
    assert!(
        stealth.contains("assert score @a[tag=dw_sttest,limit=1] dw.st_grace matches 0"),
        "asserts read the pinned dummy: {stealth}"
    );
    // The caught trip runs the campaign's real `on_caught` — arbitrary content,
    // possibly lethal to the dummy — so it must be the LAST driven action: the
    // spare (safe-player) section runs first, and after the final trip eval the
    // only remaining line is its assert.
    let trip = stealth
        .match_indices(":stealth_eval_1")
        .last()
        .expect("eval driven")
        .0;
    let tail = &stealth[trip..];
    assert_eq!(
        tail.lines().count(),
        2,
        "nothing state-dependent follows the on_caught trip: {tail}"
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

/// Regression (FIX: sequence-nested producers): a `set-checkpoint` nested inside a
/// `sequence` step is a real checkpoint. The plan must collect it with its own
/// content-ordered index, and emission must bind `#cp` to that index — never the
/// silent fallback to `0`. Before the fix the collector scanned only top-level
/// effects, so a nested checkpoint was never registered, `checkpoint_for` returned
/// `None`, and `emit_set_checkpoint` mis-bound its marker to checkpoint 0 (running
/// the wrong `on_respawn` hook on respawn).
#[test]
fn set_checkpoint_nested_in_sequence_binds_its_own_index() {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut campaign = parse_campaign(&loaded.raw).expect("parses");

    // Locate the fixture's existing top-level checkpoint: its anchor is a proven
    // standable, reachable cell we can reuse for a second, distinct checkpoint.
    let (anchor, oid, qi): (AnchorId, _, usize) = {
        let mut found = None;
        for (qi, q) in campaign.quests.content.quests.iter().enumerate() {
            for (oid, effs) in &q.on_objective_complete {
                for e in effs {
                    if let Some((a, _)) = e.set_checkpoint() {
                        found = Some((a.clone(), oid.clone(), qi));
                    }
                }
            }
        }
        found.expect("fixture has a top-level set-checkpoint")
    };

    // A distinct on_respawn hook makes this a distinct checkpoint (index 1).
    let nested_hook = vec![QuestEffect::Narrate {
        text: "You steady yourself again, deeper in.".to_string(),
        style: None,
        sound: None,
        requires_flags: vec![],
        forbids_flags: vec![],
        requires_state: vec![],
    }];
    let nested = QuestEffect::Sequence {
        steps: vec![SequenceStep {
            at_ticks: 0,
            effects: vec![QuestEffect::SetCheckpoint {
                anchor: anchor.clone(),
                on_respawn: nested_hook.clone(),
            }],
        }],
    };
    campaign.quests.content.quests[qi]
        .on_objective_complete
        .get_mut(&oid)
        .unwrap()
        .push(nested);

    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "nested-checkpoint campaign still validates clean: {diags:#?}"
    );

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    // Both checkpoints are collected; the nested one keeps its own index (1).
    assert_eq!(
        plan.checkpoints.len(),
        2,
        "both the top-level and sequence-nested checkpoints are collected"
    );
    let cp = plan
        .checkpoint_for(anchor.as_str(), &nested_hook)
        .expect("nested checkpoint is registered (not dropped)");
    assert_eq!(
        cp.index, 1,
        "the nested checkpoint keeps its content-ordered index, never a silent 0"
    );

    // Emission: the marker + dispatch use index 1.
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
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &skins,
    )
    .expect("nested-checkpoint build emits valid commands");

    let all = all_functions(&out);
    assert_eq!(
        all.matches("scoreboard players set #cp dw.sys 1").count(),
        1,
        "the nested checkpoint emits its own #cp index 1 exactly once"
    );
    // The per-index dispatch function for index 1 exists and carries its own hook.
    let hook = fn_body(&out, "cp_on_respawn_1");
    assert!(
        hook.contains("You steady yourself again, deeper in."),
        "cp_on_respawn_1 runs the nested checkpoint's own on_respawn hook"
    );
}
