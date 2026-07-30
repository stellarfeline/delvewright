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
