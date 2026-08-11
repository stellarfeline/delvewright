//! The two reds, in a form that COMPILES on `origin/main` as well as on the
//! branch — no new API is touched, only the shipped datapack and the DSL parser.
//!
//! Run it on either tree with:
//!   cp <this file> crates/compiler/tests/press_answer_red.rs
//!   cargo test -p delvec --test press_answer_red
mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, EnvTrigger, parse_campaign};

fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join("souls-shortcut");
    let loaded = load_campaign_dir(&dir).unwrap();
    parse_campaign(&loaded.raw).expect("souls-shortcut parses")
}

fn build(c: &Campaign) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn text(out: &BuildOutput, ext: &str) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(ext) {
            s.push_str(path);
            s.push('\n');
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

/// RED 1 — pressing the barred door from the wrong side answers nothing.
///
/// The door's hitboxes are tagged `dw_ws_inner_door` (task #50). For a press on
/// them to say anything, SOME advancement must watch a tag those bodies carry and
/// reward a function that writes to the presser's screen. On `origin/main` no
/// advancement in the whole shipped tree mentions the door at all.
#[test]
fn the_sealed_door_answers_a_right_click() {
    let out = build(&fixture());
    let arm = text(&out, ".mcfunction");
    let door_tags: Vec<String> = arm
        .lines()
        .filter(|l| l.starts_with("summon minecraft:interaction") && l.contains("dw_ws_inner_door"))
        .flat_map(|l| {
            l.split('"')
                .filter(|s| s.starts_with("dw_"))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(!door_tags.is_empty(), "the door must have bodies at all");

    let advs = text(&out, ".json");
    let watcher = door_tags.iter().find(|t| advs.contains(*t));
    let t = watcher.unwrap_or_else(|| {
        panic!(
            "NOTHING watches a press on the sealed shortcut door. Its bodies carry {door_tags:?} \
         and no advancement in the shipped tree names any of them, so a right-click on the \
         barred door produces silence — the finding."
        )
    });

    // …and what it rewards must address the PRESSER, on the reply strip.
    let reward = advs
        .lines()
        .find(|l| l.contains("\"function\"") && l.contains("press"))
        .map(|l| {
            l.rsplit(':')
                .next()
                .unwrap()
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
                .to_string()
        })
        .unwrap_or_else(|| panic!("the watcher on `{t}` rewards no press function:\n{advs}"));
    assert!(
        arm.contains("title @s actionbar"),
        "the reward `{reward}` must put the answer on the presser's actionbar:\n{arm}"
    );
}

/// RED 2 — a campaign cannot write that answer either.
///
/// The general verb is `EnvTrigger{on: use}` + `narrate`. To say what a wrong-side
/// answer says it needs the reply CHANNEL (`actionbar`) and the ADDRESSEE
/// (`presser`). On `origin/main` neither exists, so the document does not parse.
#[test]
fn the_campaign_can_write_a_wrong_side_answer() {
    let json = r#"{ "id": "trigger/from-the-wrong-side", "at": "anchor/door",
                    "on": { "on": "use" }, "once": false, "audience": "presser",
                    "effects": [ { "type": "narrate", "style": "actionbar",
                                   "text": "The door cannot be opened from this side." } ] }"#;
    let t = serde_json::from_str::<EnvTrigger>(json).unwrap_or_else(|e| {
        panic!("a campaign CANNOT express a wrong-side press answer on the general verb: {e}")
    });
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    c.quests.content.triggers.push(t);
    let out = build(&c);
    assert!(
        text(&out, ".mcfunction")
            .lines()
            .any(|l| l.starts_with("title @s actionbar")
                && l.contains("The door cannot be opened from this side.")),
        "the authored line must reach the presser's actionbar"
    );
}
