//! v0.8 dialogue-option `tooltip` emission: the button keeps a caption, the hover
//! box carries the full line (owner design 2026-08-04).
//!
//! The vanilla contract this rides on, read off the pinned 1.21.11 client jar
//! rather than assumed: a dialog action button is
//! `ActionButton(CommonButtonData, Optional<DialogAction>)`, and
//! `CommonButtonData`'s `MapCodec` is `fieldOf("label")` +
//! `optionalFieldOf("tooltip")` + `optionalFieldOf("width", 150)`. `tooltip` is
//! therefore a **sibling of `label` inside each `actions[]` entry**, which is the
//! shape asserted here. The compiler has in fact shipped this field since the
//! class-selection dialog (`class_select` puts each class's `blurb` in it), so
//! tier 2's "the delve loads with zero errors on the pinned vanilla server" step
//! already proves the codec accepts it on every PR; this test pins the *dialogue*
//! surface's use of it.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

/// The wine beat's shape: a caption on the button, the whole spoken line hovering.
const FULL_LINE: &str =
    "And who are you, to come knocking at a door that has stayed shut for thirty winters?";

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
    .expect("build succeeds")
}

/// hello-world materialized at `tag`, with its dialogue stage raised to 0.8.0 and
/// `mutate` applied to the parsed dialogue document.
fn hello_world_with_dialogue(
    tag: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> std::path::PathBuf {
    let tmp = std::env::temp_dir().join(format!("dw-opt-tooltip-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    common::materialize_from(&common::hello_world_dir(), &serde_json::json!({}), &tmp);
    let dpath = tmp.join("dialogue.json");
    let mut dlg: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&dpath).unwrap()).unwrap();
    dlg["dsl_version"] = serde_json::json!("0.8.0");
    mutate(&mut dlg);
    std::fs::write(&dpath, serde_json::to_string_pretty(&dlg).unwrap()).unwrap();
    tmp
}

fn greeting_dialog(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("datapack/data/hello-world/dialog/keeper_greeting.json")
            .expect("keeper_greeting dialog emitted"),
    )
    .unwrap()
}

/// An authored tooltip lands on that option's button and nowhere else: a sibling
/// of `label` in the `actions[]` entry, with the caption left alone.
#[test]
fn an_authored_tooltip_rides_its_own_button() {
    let dir = hello_world_with_dialogue("present", |dlg| {
        dlg["content"]["dialogues"][0]["nodes"][0]["options"][0]["tooltip"] =
            serde_json::json!(FULL_LINE);
    });
    let dlg = greeting_dialog(&build_campaign_dir(&dir));
    assert_eq!(dlg["type"], "minecraft:multi_action");
    let actions = dlg["actions"].as_array().unwrap();
    // i18n v2 (spec-0029): a caption/tooltip is emitted as a text COMPONENT. This
    // campaign is built straight from its stage docs (no translation tagging), so
    // the component is the literal `{"text": …}` form; the shipped `delvec build`
    // path emits `{"translate": …, "fallback": …}` for the same strings.
    assert_eq!(
        actions[0]["label"]["text"], "Who are you?",
        "the caption is untouched"
    );
    assert_eq!(
        actions[0]["tooltip"]["text"], FULL_LINE,
        "the full line rides the same button as a `tooltip`"
    );
    // The button still does what it did: the tooltip is decoration on the action,
    // never a replacement for it.
    assert_eq!(actions[0]["action"]["type"], "minecraft:run_command");
    // Siblings are unaffected — a tooltip is per-option, not per-node.
    assert!(
        actions[1].get("tooltip").is_none(),
        "an option that authored none gets none: {:#?}",
        actions[1]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// ADR-0006 byte-identity: raising the dialogue stage to 0.8.0 without authoring a
/// tooltip emits exactly the bytes it emitted before the field existed.
#[test]
fn no_tooltip_emits_no_key_at_all() {
    let dir = hello_world_with_dialogue("absent", |_| {});
    let bumped = build_campaign_dir(&dir);
    let baseline = build_campaign_dir(&common::hello_world_dir());
    let key = "datapack/data/hello-world/dialog/keeper_greeting.json";
    assert_eq!(
        bumped.get(key),
        baseline.get(key),
        "{key} must be byte-identical when no tooltip is authored"
    );
    let dlg = greeting_dialog(&bumped);
    assert!(
        dlg["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a.get("tooltip").is_none()),
        "no authored tooltip means no emitted key"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The class-selection dialog has carried a button `tooltip` since v0.1 — the
/// live-server precedent this feature stands on. Pinned so the evidence cannot
/// quietly disappear from under the dialogue surface that now cites it.
#[test]
fn class_select_has_always_shipped_a_button_tooltip() {
    let out = build_campaign_dir(&common::hello_world_dir());
    let dlg: serde_json::Value = serde_json::from_slice(
        out.get("datapack/data/hello-world/dialog/class_select.json")
            .expect("class_select dialog emitted"),
    )
    .unwrap();
    for action in dlg["actions"].as_array().unwrap() {
        assert!(
            action["tooltip"]["text"].is_string(),
            "every class button carries its blurb as a tooltip: {action:#?}"
        );
    }
}
