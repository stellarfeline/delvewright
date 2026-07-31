//! In-process emission tests: every emitted command validates, and
//! `critical-path.json` matches its documented shape with its `/trigger`
//! commands backed by enabled trigger objectives.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

/// The i18n key inventory is derived from the stage docs (player-visible strings
/// only), and `localize` swaps exactly those strings — English-only campaigns have
/// no inventory beyond their strings, and authoring-context fields are excluded.
#[test]
fn l10n_inventory_and_localize() {
    let loaded = load_campaign_dir(&common::keep_trial_dir()).unwrap();
    let mut campaign = parse_campaign(&loaded.raw).expect("valid keep-trial parses");

    let inv = delvewright_dsl::l10n_inventory(&campaign);
    // Player-visible keys are present with their canonical English values.
    assert_eq!(
        inv.get("world.title").map(String::as_str),
        Some("Trial of the Keep")
    );
    assert_eq!(
        inv.get("npc.keeper.name").map(String::as_str),
        Some("The Keeper")
    );
    assert_eq!(
        inv.get("obj.trial.slay.title").map(String::as_str),
        Some("Clear the Guard")
    );
    assert!(inv.contains_key("wave.guards.mob.0.name"));
    // Authoring-context fields are NOT player-visible → never inventoried.
    assert!(!inv.keys().any(|k| k.contains("theme")
        || k.contains("premise")
        || k.contains("persona")
        || k.contains("archetype")));

    // Localize with the shipped zh-cn sidecar and confirm the swap.
    let doc: delvewright_dsl::L10nDoc =
        serde_json::from_slice(&loaded.l10n["zh-cn"]).expect("sidecar parses");
    delvewright_dsl::localize(&mut campaign, &doc.content);
    assert_eq!(campaign.world.content.title, "要塞的试炼");
    // Non-inventoried fields are untouched by localization (stay English/source).
    assert_eq!(campaign.world.content.theme, loaded_theme());
}

fn loaded_theme() -> &'static str {
    "A ruined keep on the moor, guarded by the restless dead."
}

fn build_hello_world() -> BuildOutput {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
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
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
    )
    .expect("emission succeeds")
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(out.get(path).unwrap_or_else(|| panic!("missing {path}"))).unwrap()
}

#[test]
fn every_emitted_command_validates() {
    let out = build_hello_world();
    let tree = CommandTree::v1_21_11();
    let errors = emit::validate_emitted(&out, &tree);
    assert!(
        errors.is_empty(),
        "emitted commands failed validation: {:#?}",
        errors
    );
}

#[test]
fn render_plan_shape_and_expect_vocabulary() {
    let out = build_hello_world();
    let rp: serde_json::Value =
        serde_json::from_slice(out.get("render-plan.json").unwrap()).unwrap();

    assert_eq!(rp["campaign_id"], "hello-world");
    assert!(rp["layout_aabb"]["min"].is_array());
    assert!(rp["layout_aabb"]["max"].is_array());
    let shots = rp["shots"].as_array().unwrap();
    assert!(!shots.is_empty(), "at least one shot");

    // Every shot carries a camera (pos/yaw/pitch) and a non-empty expect list.
    for s in shots {
        let cam = &s["camera"];
        assert_eq!(cam["pos"].as_array().unwrap().len(), 3);
        assert!(cam["yaw"].is_number());
        assert!(cam["pitch"].is_number());
        assert!(!s["expect"].as_array().unwrap().is_empty());
    }

    let kinds: Vec<&str> = shots.iter().map(|s| s["kind"].as_str().unwrap()).collect();
    // hello-world: a spawn, the keeper NPC, an interior, and the door gate.
    assert!(kinds.contains(&"spawn"), "spawn shot present: {kinds:?}");
    assert!(kinds.contains(&"npc"), "npc shot present: {kinds:?}");
    assert!(
        kinds.contains(&"interior"),
        "interior shot present: {kinds:?}"
    );
    assert!(kinds.contains(&"gate"), "gate shot present: {kinds:?}");

    // The NPC shot names the keeper in its expect checklist (localizable string).
    let npc = shots.iter().find(|s| s["kind"] == "npc").unwrap();
    let expect = npc["expect"].as_array().unwrap();
    assert!(
        expect
            .iter()
            .any(|e| e.as_str().unwrap().contains("faces the camera")),
        "npc expect names the NPC + facing: {expect:?}"
    );

    // The spawn shot is first (deterministic ordering).
    assert_eq!(shots[0]["kind"], "spawn");
}

#[test]
fn critical_path_shape_and_commands() {
    let out = build_hello_world();
    let cp: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();

    assert_eq!(cp["version"], "0.2.0");
    assert_eq!(cp["campaign_id"], "hello-world");
    let steps = cp["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0]["action"], "select-class");
    assert_eq!(steps[1]["action"], "talk-to");
    assert_eq!(steps[2]["action"], "reach");
    assert_eq!(steps[3]["action"], "assert-complete");
    // amended contract fields
    assert_eq!(steps[0]["command"], "/trigger dw.class set 1");
    assert_eq!(steps[1]["command"], "/trigger dw.dlg_keeper set 2");
    assert_eq!(steps[3]["scoreboard"]["objective"], "dw.campaign");
    assert_eq!(steps[3]["scoreboard"]["value"], 1);

    // Each interactive step's /trigger objective must be enabled every tick, so
    // both the dialog buttons and the bot's chat command work.
    let tick = text(&out, "datapack/data/hello-world/function/tick.mcfunction");
    for step in steps {
        if let Some(cmd) = step["command"].as_str() {
            // command form: "/trigger <objective> set <n>"
            let objective = cmd.split_whitespace().nth(1).unwrap();
            assert!(
                tick.contains(&format!("scoreboard players enable @a {objective}")),
                "trigger objective {objective} is not enabled in tick"
            );
        }
    }

    // The completion objective is displayed in the sidebar for the bot to read.
    let setup = text(&out, "datapack/data/hello-world/function/setup.mcfunction");
    assert!(setup.contains("scoreboard objectives setdisplay sidebar dw.campaign"));
}

/// Regressions for bugs found during the live 1.21.11 load shakeout (M1).
#[test]
fn live_load_shakeout_fixes() {
    let out = build_hello_world();

    // 1. pack.mcmeta: 1.21.11 rejects a bare `pack_format` for formats >81 —
    //    it requires min_format/max_format. We emit both as [94, 1].
    let mcmeta: serde_json::Value =
        serde_json::from_slice(out.get("datapack/pack.mcmeta").unwrap()).unwrap();
    assert_eq!(mcmeta["pack"]["min_format"], serde_json::json!([94, 1]));
    assert_eq!(mcmeta["pack"]["max_format"], serde_json::json!([94, 1]));
    assert!(
        mcmeta["pack"].get("pack_format").is_none(),
        "bare pack_format must not be emitted (rejected on 1.21.11)"
    );

    // 2. The interaction advancement's `entity` condition must be the single
    //    sub-predicate object form; the loot-condition list form failed to load
    //    ("No key entity in MapLike").
    let adv: serde_json::Value = serde_json::from_slice(
        out.get("datapack/data/hello-world/advancement/keeper_interact.json")
            .unwrap(),
    )
    .unwrap();
    let entity = &adv["criteria"]["interact"]["conditions"]["entity"];
    assert!(
        entity.is_object(),
        "entity condition must be an object, not a list"
    );
    assert_eq!(entity["type"], "minecraft:interaction");

    // 3. Placement is verification-driven (2026-07-31): setup forceloads and
    //    defers placement to the tick-retried `place_all`/`place_verify` pair
    //    (freshly-forceloaded far chunks are not reliably loaded in-tick, so a
    //    same-function `place template` can silently no-op). `setup_finish` runs
    //    once all sentinels verify and owns everything that needs real blocks,
    //    including setworldspawn.
    let setup = text(&out, "datapack/data/hello-world/function/setup.mcfunction");
    assert!(
        setup.contains("forceload add"),
        "forceload emitted in setup"
    );
    assert!(
        !setup.contains("place template"),
        "placement must not run in setup (unverifiable in-tick)"
    );
    let place_all = text(
        &out,
        "datapack/data/hello-world/function/place_all.mcfunction",
    );
    assert!(place_all.contains("place template"), "place_all places");
    let verify = text(
        &out,
        "datapack/data/hello-world/function/place_verify.mcfunction",
    );
    assert!(
        verify.contains("execute if block") && verify.contains("setup_finish"),
        "place_verify checks sentinels and gates setup_finish"
    );
    let finish = text(
        &out,
        "datapack/data/hello-world/function/setup_finish.mcfunction",
    );
    assert!(finish.contains("setworldspawn "), "setworldspawn in finish");
    let tick = text(&out, "datapack/data/hello-world/function/tick.mcfunction");
    assert!(
        tick.contains("place_all") && tick.contains("place_verify"),
        "tick retries placement until verified"
    );

    // 4. campaign_complete broadcasts the machine-readable completion marker the
    //    validation bot reads (mineflayer cannot read 1.21.11 scoreboard scores).
    let complete = text(
        &out,
        "datapack/data/hello-world/function/campaign_complete.mcfunction",
    );
    assert!(
        complete.contains("[Delvewright] complete dw.campaign 1"),
        "completion marker must be broadcast for the bot"
    );
}

/// The generated PackTest suite registers at the path PackTest auto-discovers
/// (`data/<ns>/test/`, NOT under `function/`) and is a real test (directives +
/// `assert`) that drives the completion chain — verified live on Fabric +
/// PackTest 2.4.0.
#[test]
fn packtest_suite_is_a_real_test() {
    let out = build_hello_world();
    let test = text(
        &out,
        "packtest-datapack/data/hello-world/test/campaign.mcfunction",
    );
    assert!(test.contains("# @dummy"), "declares a dummy player");
    assert!(
        test.contains("function hello-world:setup"),
        "runs the real generated init"
    );
    assert!(
        test.contains("run function hello-world:complete_o_talk"),
        "drives the talk objective completion"
    );
    assert!(
        test.contains("run function hello-world:complete_o_exit"),
        "drives the reach objective completion"
    );
    assert!(
        test.contains("assert score @p dw.campaign matches 1"),
        "asserts the campaign objective is set"
    );
    // The old provisional path under function/ must be gone — PackTest would not
    // discover a test there.
    assert!(
        out.keys()
            .all(|p| !p.contains("packtest-datapack") || !p.contains("/function/")),
        "no packtest function under /function/ (PackTest scans /test/)"
    );
}

/// Environment sealing (spec-0002 "Environment sealing"): the bootstrap seals the
/// box garden with the exact 1.21.11 gamerule commands (verified live — 1.21.11
/// renamed the legacy camelCase rules to a snake_case registry) plus a fixed time
/// of day. This is the authoritative sealing assertion (gamerule values have no
/// vanilla read-back path, so PackTest cannot assert them in-game).
#[test]
fn environment_sealing_emitted() {
    let out = build_hello_world();
    let setup = text(&out, "datapack/data/hello-world/function/setup.mcfunction");

    // Exact 1.21.11 forms — the old camelCase spellings are rejected live.
    let expected = [
        "gamerule spawn_mobs false",                   // was doMobSpawning
        "gamerule advance_time false",                 // was doDaylightCycle
        "gamerule advance_weather false",              // was doWeatherCycle
        "gamerule fire_spread_radius_around_player 0", // was doFireTick (now an int radius)
        "gamerule mob_griefing false",                 // was mobGriefing
        "time set noon",                               // fixed authored time (v0 default)
    ];
    for cmd in expected {
        assert!(
            setup.lines().any(|l| l.trim() == cmd),
            "sealing command missing / wrong form: `{cmd}`\nsetup:\n{setup}"
        );
    }

    // The legacy camelCase identifiers must never be emitted (they don't parse on
    // 1.21.11) — guards against a regression to the pre-1.21.11 spelling.
    for legacy in [
        "doMobSpawning",
        "doDaylightCycle",
        "doWeatherCycle",
        "doFireTick",
        "mobGriefing",
    ] {
        assert!(
            !setup.contains(legacy),
            "legacy gamerule name `{legacy}` must not be emitted (rejected on 1.21.11)"
        );
    }

    // Sealing is part of the idempotent, `#init`-guarded bootstrap — it must run
    // before the `#init` flag is set (so it fires exactly once per world).
    let seal_idx = setup.find("gamerule ").expect("gamerule emitted");
    let init_idx = setup
        .find("scoreboard players set #init dw.sys 1")
        .expect("init flag set");
    assert!(
        seal_idx < init_idx,
        "sealing must precede the #init guard set"
    );

    // Every emitted sealing command still validates against the vendored 1.21.11
    // command tree (covered broadly by `every_emitted_command_validates`, asserted
    // here per-line for a precise failure).
    let tree = CommandTree::v1_21_11();
    for cmd in expected {
        assert!(
            tree.validate_line(cmd).is_ok(),
            "sealing command fails the 1.21.11 command-tree validator: `{cmd}`"
        );
    }
}

/// The compiler-generated PackTest suite includes a sealed-state test that asserts
/// the one sealing value with a vanilla read-back path: the pinned time of day.
#[test]
fn packtest_sealed_state_test_emitted() {
    let out = build_hello_world();
    let test = text(
        &out,
        "packtest-datapack/data/hello-world/test/sealed_state.mcfunction",
    );
    assert!(test.contains("# @dummy"), "declares a dummy player");
    assert!(
        test.contains("function hello-world:setup"),
        "runs the real generated bootstrap (which applies sealing)"
    );
    assert!(
        test.contains("run time query daytime"),
        "queries the world time"
    );
    assert!(
        test.contains("assert score #sealtime dw.sys matches 6000"),
        "asserts time is noon (daytime 6000)"
    );
}

/// The creator overlay (spec-0006 M2) is a self-contained `creator-datapack/`
/// subtree: a `dw.note` trigger + a per-tick stamp handler that macro-emits one
/// machine-readable `[DelveNote]` line, plus a `layout.json` for the harvester. Its
/// `.mcfunction`s are plain vanilla (validated by `every_emitted_command_validates`)
/// and its bytes ride the determinism gate (`build_is_byte_identical_across_runs`).
#[test]
fn creator_overlay_emitted() {
    let out = build_hello_world();

    // pack.mcmeta uses the same [94,1] format contract.
    let mcmeta: serde_json::Value =
        serde_json::from_slice(out.get("creator-datapack/pack.mcmeta").unwrap()).unwrap();
    assert_eq!(mcmeta["pack"]["min_format"], serde_json::json!([94, 1]));

    // load/tick tags add the overlay functions (they MERGE with the main datapack's
    // same-named tags at load time).
    let load: serde_json::Value = serde_json::from_slice(
        out.get("creator-datapack/data/minecraft/tags/function/load.json")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(load["values"][0], "hello-world:creator/init");
    let tick: serde_json::Value = serde_json::from_slice(
        out.get("creator-datapack/data/minecraft/tags/function/tick.json")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(tick["values"][0], "hello-world:creator/tick");

    // The trigger is registered and armed every tick, then dispatched on fire.
    let init = text(
        &out,
        "creator-datapack/data/hello-world/function/creator/init.mcfunction",
    );
    assert!(init.contains("scoreboard objectives add dw.note trigger"));
    let tickf = text(
        &out,
        "creator-datapack/data/hello-world/function/creator/tick.mcfunction",
    );
    assert!(tickf.contains("scoreboard players enable @a dw.note"));
    assert!(tickf.contains("scores={dw.note=1..}"));
    assert!(tickf.contains("run function hello-world:creator/stamp"));

    // The stamp reads the player position off entity NBT (macro source) and
    // invokes the emit macro `with storage`.
    let stamp = text(
        &out,
        "creator-datapack/data/hello-world/function/creator/stamp.mcfunction",
    );
    assert!(stamp.contains("run data get entity @s Pos[0]"));
    assert!(stamp.contains("data modify storage hello-world:note area set value \"area/keep\""));
    assert!(stamp.contains("if entity @s[tag=dw_npc_keeper]"));
    assert!(stamp.contains("run scoreboard players get @s dw.o_talk"));
    assert!(stamp.contains("function hello-world:creator/emit with storage hello-world:note"));

    // The emit line is a single `say` macro (server-log-reachable, unlike a
    // tellraw system message) carrying the spec's `[DelveNote]` fields, with real
    // DSL objective ids baked in and live values macro-substituted.
    let emit = text(
        &out,
        "creator-datapack/data/hello-world/function/creator/emit.mcfunction",
    );
    let emit = emit.trim();
    assert!(emit.starts_with("$say [DelveNote] "), "emit line: {emit}");
    assert!(emit.contains("pos=[$(x),$(y),$(z)]"));
    assert!(emit.contains("area=$(area)"));
    assert!(emit.contains("obj/talk:$(o_talk)"));
    assert!(emit.contains("obj/exit:$(o_exit)"));
    assert!(emit.contains("nearest_npc=$(npc)"));

    // layout.json is the harvester's only campaign input: area→prefab and
    // objective→quest.
    let layout: serde_json::Value =
        serde_json::from_slice(out.get("creator-datapack/layout.json").unwrap()).unwrap();
    assert_eq!(layout["campaign_id"], "hello-world");
    assert_eq!(layout["areas"][0]["id"], "area/keep");
    assert_eq!(layout["areas"][0]["prefab"], "prefab/hello-room");
    assert_eq!(layout["objectives"][0]["id"], "obj/talk");
    assert_eq!(layout["objectives"][0]["quest"], "quest/open-the-door");
}

/// The overlay is playtest-only and must never enter the shipped delve datapack:
/// the two subtrees are strictly separate, and no `dw.note`/creator machinery
/// leaks into `datapack/` (the CI image-exclusion check is the runtime backstop).
#[test]
fn creator_overlay_absent_from_shipped_datapack() {
    let out = build_hello_world();
    for (path, bytes) in &out {
        if path.starts_with("datapack/") {
            let body = std::str::from_utf8(bytes).unwrap_or("");
            assert!(
                !path.contains("creator")
                    && !body.contains("dw.note")
                    && !body.contains("DelveNote"),
                "creator overlay leaked into shipped datapack at {path}"
            );
        }
    }
}

#[test]
fn dialog_buttons_run_the_trigger_commands() {
    let out = build_hello_world();
    // The keeper greeting dialog's "open the door" button runs the same command
    // the critical path records (set 2).
    let dlg: serde_json::Value = serde_json::from_slice(
        out.get("datapack/data/hello-world/dialog/keeper_greeting.json")
            .unwrap(),
    )
    .unwrap();
    let actions = dlg["actions"].as_array().unwrap();
    let commands: Vec<&str> = actions
        .iter()
        .map(|a| a["action"]["command"].as_str().unwrap())
        .collect();
    assert!(commands.contains(&"/trigger dw.dlg_keeper set 2"));
    for a in actions {
        assert_eq!(a["action"]["type"], "minecraft:run_command");
    }
}
