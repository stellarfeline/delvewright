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
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(out.get(path).unwrap_or_else(|| panic!("missing {path}"))).unwrap()
}

/// Build any campaign directory (generalizes `build_hello_world`).
fn build_campaign_dir(dir: &std::path::Path) -> BuildOutput {
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
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

/// gap 10 regression: NO emitted dialog carries an empty `actions` list. A
/// `minecraft:multi_action` with `"actions": []` is rejected by the 1.21.11 dialog
/// codec ("List must have contents") and aborts registry load at server boot — a
/// terminal (option-less) node must instead ship as `minecraft:notice`.
#[test]
fn no_emitted_dialog_has_empty_actions() {
    for dir in [
        common::hello_world_dir(),
        common::keep_crawl_dir(),
        common::keep_trial_dir(),
        common::keep_vertical_dir(),
    ] {
        let out = build_campaign_dir(&dir);
        for (path, bytes) in &out {
            let Some(rest) = path.strip_prefix("datapack/") else {
                continue;
            };
            if !(rest.contains("/dialog/") && rest.ends_with(".json")) {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            if let Some(actions) = v.get("actions") {
                assert!(
                    !actions.as_array().expect("actions is a list").is_empty(),
                    "dialog {path} has an empty `actions` list (server will not boot)"
                );
            }
        }
    }
}

/// gap 10: a dialogue node with empty options (schema: "empty closes the dialog")
/// is emitted as `minecraft:notice` — an implicit single close button — NOT a
/// `multi_action` with an empty `actions` list. Materializes a keep-trial variant
/// whose `dlg/lore` node is made terminal and asserts the emitted shape.
#[test]
fn terminal_dialogue_node_emits_notice() {
    let tmp = std::env::temp_dir().join(format!("dw-notice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &tmp);
    common::make_english_only(&tmp); // drop the zh-cn sidecar; build English-only

    // Make `dlg/lore` terminal: empty its options.
    let dpath = tmp.join("dialogue.json");
    let mut dlg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&dpath).unwrap()).unwrap();
    let mut found = false;
    for tree in dlg["content"]["dialogues"].as_array_mut().unwrap() {
        for node in tree["nodes"].as_array_mut().unwrap() {
            if node["id"] == "dlg/lore" {
                node["options"] = serde_json::json!([]);
                found = true;
            }
        }
    }
    assert!(found, "dlg/lore node present in keep-trial");
    std::fs::write(&dpath, serde_json::to_string_pretty(&dlg).unwrap()).unwrap();

    let out = build_campaign_dir(&tmp);
    let key = "datapack/data/keep-trial/dialog/keeper_lore.json";
    let v: serde_json::Value =
        serde_json::from_slice(out.get(key).expect("keeper_lore dialog emitted")).unwrap();
    assert_eq!(
        v["type"], "minecraft:notice",
        "an option-less node ships as a notice"
    );
    assert!(
        v.get("actions").is_none(),
        "a notice carries no `actions` list"
    );
    // A node that still has options remains a multi_action (control case).
    let greet: serde_json::Value = serde_json::from_slice(
        out.get("datapack/data/keep-trial/dialog/keeper_greet.json")
            .expect("keeper_greet dialog emitted"),
    )
    .unwrap();
    assert_eq!(greet["type"], "minecraft:multi_action");
    assert!(!greet["actions"].as_array().unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&tmp);
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

    // Player-POV tier: first-person cameras along the walked critical path.
    let pov: Vec<&serde_json::Value> = shots.iter().filter(|s| s["kind"] == "pov").collect();
    assert!(
        !pov.is_empty(),
        "hello-world has a walked leg, so it emits player-POV shots: {kinds:?}"
    );
    for s in &pov {
        let cam = &s["camera"];
        // Eye is at 1.62 above the standing cell floor (feet cell Y + 1.62).
        let y = cam["pos"][1].as_f64().unwrap();
        let cell_y = s["standing_cell"][1].as_f64().unwrap();
        assert!(
            (y - (cell_y + 1.62)).abs() < 1e-6,
            "POV eye at cell_y+1.62: eye_y={y} cell_y={cell_y}"
        );
        // POV shots carry the first-person FOV and a leg/objective.
        assert_eq!(cam["fov"].as_f64().unwrap(), 70.0);
        assert!(s["leg"].is_number());
        assert!(s["objective"].is_string(), "POV names the served objective");
        // The first expect entry is the one-sentence first-person description.
        let line = s["expect"][0].as_str().unwrap();
        assert!(
            line.starts_with("First-person view"),
            "POV expect leads with the description: {line}"
        );
    }
    // POV shots sort after the overhead/orbit kinds (deterministic suffix).
    let first_pov = shots.iter().position(|s| s["kind"] == "pov").unwrap();
    assert!(
        shots[first_pov..].iter().all(|s| s["kind"] == "pov"),
        "POV shots form the trailing block"
    );
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

    // The completion objective exists but is NOT put on the sidebar: a raw
    // internal id (`dw.campaign`) must never surface to players (task #54
    // addendum). The bot observes completion via the chat token, not the sidebar.
    let setup = text(&out, "datapack/data/hello-world/function/setup.mcfunction");
    assert!(setup.contains("scoreboard objectives add dw.campaign dummy"));
    assert!(
        !setup.contains("setdisplay sidebar"),
        "no raw-id sidebar display leaks to players: {setup}"
    );
    // Completion still sets the objective (chat-token marker + PackTest assert).
    let complete = text(
        &out,
        "datapack/data/hello-world/function/campaign_complete.mcfunction",
    );
    assert!(complete.contains("scoreboard players set @s dw.campaign 1"));
    assert!(complete.contains("[dw:complete hello-world campaign]"));
}

/// The bot-completion oracle (AUDIT-P0). Two halves that must agree:
///   * `critical-path.json` names, on every objective-bearing step, the `obj/<id>`
///     that step must prove — and declares `format_version` so a pre-oracle path
///     (position-only, unverifiable) can never be run;
///   * the datapack broadcasts exactly one anchored marker line per objective, on
///     the objective's own completion, plus the `campaign` token at the end.
///
/// Pinned literally: this string IS the wire format the harness parses, so any
/// drift in it must break here rather than silently in a live run.
#[test]
fn completion_marker_channel_is_anchored_and_per_objective() {
    let out = build_hello_world();
    let cp: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();

    // The contract version is explicit and independent of the DSL version.
    assert_eq!(cp["format_version"], 2);
    assert_eq!(cp["version"], "0.2.0");

    let steps = cp["steps"].as_array().unwrap();
    // Every objective-bearing step names its objective; the framing steps do not.
    assert_eq!(steps[1]["objective"], "obj/talk");
    assert_eq!(steps[2]["objective"], "obj/exit");
    assert!(
        steps[0]["objective"].is_null(),
        "select-class proves no objective"
    );
    assert!(
        steps[3]["objective"].is_null(),
        "assert-complete is proved by the campaign token, not an objective"
    );

    // Each named objective's completion function broadcasts exactly its own
    // anchored marker — campaign id included, whole line, no other objective's.
    for (obj, func) in [
        ("obj/talk", "complete_o_talk"),
        ("obj/exit", "complete_o_exit"),
    ] {
        let body = text(
            &out,
            &format!("datapack/data/hello-world/function/{func}.mcfunction"),
        );
        assert!(
            body.contains(&format!(
                r#"tellraw @a {{"color":"dark_gray","text":"[dw:complete hello-world {obj}]"}}"#
            )),
            "{func} must broadcast the anchored marker for {obj}: {body}"
        );
        // The marker fires as the score flips, before any effect that could
        // teleport the player or complete the campaign.
        let marker_at = body.find("[dw:complete").expect("marker present");
        let score_at = body
            .find("scoreboard players set @s dw.o_")
            .expect("score set");
        assert!(
            score_at < marker_at,
            "marker follows its own score set: {body}"
        );
    }

    // The unanchored legacy token is gone: it was matchable as a substring of any
    // chat line, which is exactly what made a step passable without completing.
    let all: String = out
        .values()
        .filter_map(|v| String::from_utf8(v.clone()).ok())
        .collect();
    assert!(
        !all.contains("[Delvewright] complete"),
        "the unanchored completion token must not survive anywhere in the build"
    );
}

/// task #38: the compiler exports the DW0311-proven critical-path routes as a
/// deterministic validation artifact (`validation/critical-path-waypoints.json`) so
/// the harness can navigate the bot leg-by-leg. It is validation metadata, cleanly
/// separated from the shipped datapack (lives under `validation/`, not `datapack/`),
/// and each leg's `to` matches a `critical-path.json` step position so the harness
/// can key a leg by its destination.
#[test]
fn critical_path_waypoints_artifact_shape() {
    let out = build_hello_world();
    let wp: serde_json::Value = serde_json::from_slice(
        out.get("validation/critical-path-waypoints.json")
            .expect("waypoints artifact emitted (hello-world walks talk→exit)"),
    )
    .unwrap();

    assert_eq!(wp["version"], "0.2.0");
    assert_eq!(wp["campaign_id"], "hello-world");
    let legs = wp["legs"].as_array().expect("legs is an array");
    assert!(!legs.is_empty(), "hello-world has at least one walked leg");

    // The set of leg destinations is a subset of the critical-path step positions
    // (so the harness matches a leg by its target anchor).
    let cp: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();
    let step_positions: std::collections::BTreeSet<Vec<i64>> = cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s.get("pos"))
        .map(|p| {
            p.as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap())
                .collect()
        })
        .collect();
    for leg in legs {
        let to: Vec<i64> = leg["to"]
            .as_array()
            .expect("leg.to is an array")
            .iter()
            .map(|n| n.as_i64().unwrap())
            .collect();
        assert!(
            step_positions.contains(&to),
            "leg destination {to:?} is not a critical-path step position"
        );
        let wps = leg["waypoints"].as_array().expect("waypoints is an array");
        assert!(!wps.is_empty(), "a walked leg has at least its endpoints");
        // The last waypoint is the leg's snapped goal near `to` (within snap radius).
        let last: Vec<i64> = wps
            .last()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap())
            .collect();
        let d = (0..3).map(|i| (last[i] - to[i]).abs()).sum::<i64>();
        assert!(d <= 9, "final waypoint {last:?} far from leg target {to:?}");
    }

    // The artifact is NOT part of the shipped datapack (it is validation metadata).
    assert!(
        !out.keys().any(|p| p.starts_with("datapack/validation")),
        "waypoints artifact must not live inside the datapack"
    );
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
        complete.contains("[dw:complete hello-world campaign]"),
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
        test.contains("tag @p add dw_t_camp"),
        "pins its own dummy (batch model: one dummy per test, all coexisting)"
    );
    assert!(
        test.contains("assert score @a[tag=dw_t_camp,limit=1] dw.campaign matches 1"),
        "asserts the campaign objective is set on the pinned dummy"
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
        "gamerule respawn_radius 0",                   // was spawnRadius (spawn scatter off)
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
        "spawnRadius",
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

/// Datapack-owned FIRST-JOIN placement (singleplayer parity). The integrated
/// (singleplayer) server does not reliably honour the emitted level.dat spawn and
/// drops the first join at the superflat floor — inside stone. No rung of the
/// validation ladder runs an integrated server, so this is asserted statically:
/// the tick function must drive a once-per-player `join_place`, and `join_place`
/// must teleport to the campaign entry point (the same cell `class_apply_*` uses)
/// and then mark the player, so a relog never re-teleports.
#[test]
fn first_join_placement_emitted() {
    let out = build_hello_world();
    let tick = text(&out, "datapack/data/hello-world/function/tick.mcfunction");
    let join = text(
        &out,
        "datapack/data/hello-world/function/join_place.mcfunction",
    );

    // Driver: gated on placement being verified (so the teleport lands on real
    // geometry) and on the absence of the per-player tag (so it fires once).
    let driver = "execute if score #placed dw.sys matches 1 as @a[tag=!dw_joined] \
                  run function hello-world:join_place";
    assert!(
        tick.lines().any(|l| l.trim() == driver),
        "tick must drive first-join placement: `{driver}`\ntick:\n{tick}"
    );

    // The teleport target is the campaign entry point — identical to the cell the
    // class-apply handler teleports to.
    let tp = join
        .lines()
        .find(|l| l.starts_with("teleport @s "))
        .expect("join_place teleports the player");
    let class_apply_path = out
        .keys()
        .find(|p| p.contains("/function/class_apply_"))
        .expect("a class-apply handler is emitted")
        .clone();
    let class_apply = text(&out, &class_apply_path);
    assert!(
        class_apply.lines().any(|l| l.trim() == tp),
        "first-join placement must use the campaign entry point (`{tp}`)\n\
         {class_apply_path}:\n{class_apply}"
    );
    assert!(
        join.lines().any(|l| l.trim() == "tag @s add dw_joined"),
        "join_place must mark the player so a relog never re-teleports\n{join}"
    );

    // Both lines are real 1.21.11 commands.
    let tree = CommandTree::v1_21_11();
    for line in join.lines().filter(|l| !l.trim().is_empty()) {
        assert!(
            tree.validate_line(line).is_ok(),
            "join_place line fails the 1.21.11 command-tree validator: `{line}`"
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
        test.contains("assert score #sealtime_sealed dw.sys matches 6000"),
        "asserts time is noon (daytime 6000) via the test-unique holder"
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

/// The gravity-despawn diagnostic (DW0313, task #42) fires at build time for a
/// prefab whose gravity floor is unsupported over the void, and passes once a
/// non-falling substrate is added — exercised against a real plan (real piece
/// AABBs for attribution) with synthetic structure bytes.
mod gravity_despawn {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;

    /// Gzip-framed structure NBT from `(local pos, block id)` cells.
    fn structure_nbt(cells: &[([i32; 3], &str)]) -> Vec<u8> {
        use fastnbt::Value;
        let mut names: Vec<String> = Vec::new();
        let mut blocks: Vec<Value> = Vec::new();
        for (p, n) in cells {
            let state = names.iter().position(|x| x == n).unwrap_or_else(|| {
                names.push((*n).to_string());
                names.len() - 1
            });
            let mut b = HashMap::new();
            b.insert(
                "pos".to_string(),
                Value::List(p.iter().map(|v| Value::Int(*v)).collect()),
            );
            b.insert("state".to_string(), Value::Int(state as i32));
            blocks.push(Value::Compound(b));
        }
        let palette = Value::List(
            names
                .iter()
                .map(|n| {
                    let mut c = HashMap::new();
                    c.insert("Name".to_string(), Value::String(n.clone()));
                    Value::Compound(c)
                })
                .collect(),
        );
        let mut root = HashMap::new();
        root.insert("palette".to_string(), palette);
        root.insert("blocks".to_string(), Value::List(blocks));
        let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn unsupported_sand_floor_is_dw0313_and_substrate_passes() {
        let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
        let campaign = parse_campaign(&loaded.raw).unwrap();
        let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
        let plan = Plan::build(&campaign, &prefabs).unwrap();

        // Unsupported: one sand cell at each piece's local origin → over void.
        let mut bad: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        // Supported: a stone substrate under the sand surface → nothing despawns.
        let mut good: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for area in &plan.areas {
            for piece in &area.pieces {
                bad.entry(piece.structure_file.clone())
                    .or_insert_with(|| structure_nbt(&[([0, 0, 0], "minecraft:sand")]));
                good.entry(piece.structure_file.clone()).or_insert_with(|| {
                    structure_nbt(&[
                        ([0, 0, 0], "minecraft:stone"),
                        ([0, 1, 0], "minecraft:sand"),
                    ])
                });
            }
        }

        let msg = delvewright_compiler::assembled::gravity_despawn_error(&plan, &bad)
            .expect("unsupported sand floor must raise DW0313");
        assert!(msg.contains("despawn") && msg.contains("substrate"));
        assert!(msg.contains("Do NOT swap the floor palette"));

        assert!(
            delvewright_compiler::assembled::gravity_despawn_error(&plan, &good).is_none(),
            "a substrate-supported sand floor must pass"
        );
    }
}

/// Singleplayer pause-freeze parity: a dialogue handler must RE-ARM the trigger it
/// consumes, in the same function. `scoreboard players reset` re-locks a trigger;
/// the per-tick `scoreboard players enable @a` cannot close the window, because the
/// handler's last act is to show the next dialog node and the integrated
/// (singleplayer) server freezes ticking while a screen is open — the player's next
/// click is then executed before the tick ever runs again and vanilla rejects it.
/// A dedicated server never pauses, so the validation ladder cannot see this.
#[test]
fn dialogue_handler_rearms_its_own_trigger() {
    let out = build_hello_world();
    let handlers: Vec<String> = out
        .keys()
        .filter(|p| p.contains("/function/dlg_"))
        .cloned()
        .collect();
    assert!(!handlers.is_empty(), "hello-world emits dialogue handlers");

    for path in &handlers {
        let body = text(&out, path);
        let lines: Vec<&str> = body.lines().map(str::trim).collect();
        let reset = lines
            .iter()
            .position(|l| l.starts_with("scoreboard players reset @s dw.dlg_"))
            .unwrap_or_else(|| panic!("{path} must consume its trigger:\n{body}"));
        let obj = lines[reset]
            .rsplit(' ')
            .next()
            .expect("reset names an objective");
        // Immediately after the reset — before any `return fail` gate can
        // short-circuit the rest of the handler, and before any `dialog show`.
        assert_eq!(
            lines.get(reset + 1).copied(),
            Some(format!("scoreboard players enable @s {obj}").as_str()),
            "{path} must re-arm `{obj}` immediately after resetting it:\n{body}"
        );
    }

    // Belt-and-braces: the per-tick re-enable is still there.
    let tick = text(&out, "datapack/data/hello-world/function/tick.mcfunction");
    assert!(
        tick.lines()
            .any(|l| l.trim().starts_with("scoreboard players enable @a dw.dlg_")),
        "the per-tick re-enable stays as belt-and-braces:\n{tick}"
    );

    // The emitted PackTest drives the real freeze scenario: use the trigger, run
    // the handler, use it again — with the tick function never running.
    let pt = text(
        &out,
        "packtest-datapack/data/hello-world/test/dialogue_trigger_rearm.mcfunction",
    );
    assert!(
        !pt.contains("hello-world:tick"),
        "the re-arm PackTest must NOT run the tick function (that is the freeze):\n{pt}"
    );
    assert_eq!(
        pt.lines()
            .filter(|l| l
                .trim()
                .starts_with("execute as @a[tag=dw_t_rearm,limit=1] run trigger "))
            .count(),
        2,
        "the re-arm PackTest uses the trigger twice, on its pinned dummy:\n{pt}"
    );
    assert_eq!(
        pt.lines()
            .filter(|l| l
                .trim()
                .starts_with("assert score @a[tag=dw_t_rearm,limit=1] "))
            .count(),
        2,
        "both uses are asserted on the pinned dummy:\n{pt}"
    );
}
