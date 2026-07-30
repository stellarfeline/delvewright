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

fn build_hello_world() -> BuildOutput {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");

    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        let bytes = std::fs::read(common::prefabs_dir().join(&area.structure_file)).unwrap();
        structures.insert(area.structure_file.clone(), bytes);
    }
    let tree = CommandTree::v1_21_11();
    emit::build(&plan, &loaded.inputs, &structures, &tree).expect("emission succeeds")
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
fn critical_path_shape_and_commands() {
    let out = build_hello_world();
    let cp: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();

    assert_eq!(cp["version"], "0.1.0");
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

    // 3. setup must forceload the prefab chunks before placement (else place/
    //    summon/fill silently no-op at load time) and set world spawn onto the
    //    prefab floor (else players fall through the void before class select).
    let setup = text(&out, "datapack/data/hello-world/function/setup.mcfunction");
    let force_idx = setup.find("forceload add").expect("forceload emitted");
    let place_idx = setup.find("place template").expect("place emitted");
    assert!(
        force_idx < place_idx,
        "forceload must precede place template"
    );
    assert!(setup.contains("setworldspawn "), "setworldspawn emitted");

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
