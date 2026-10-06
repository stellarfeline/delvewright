//! spec-0078: every dialog button the engine presents carries the same optional
//! hover `tooltip`.
//!
//! The dialogue option and the shop offer already prove their own tooltips
//! (`v08_option_tooltip.rs`, `v10_economy.rs`); this file proves the bonfire's
//! two buttons, which gained theirs here, and that a campaign stating none emits
//! the same bytes it emitted before the field existed.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

const NS: &str = "souls-bonfire";
const DIALOG: &str = "datapack/data/souls-bonfire/dialog/bonfire_0.json";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// Add `fields` to every `bonfire` effect anywhere in `v`; returns how many.
fn patch_bonfires(v: &mut Value, fields: &Value) -> usize {
    match v {
        Value::Object(map) => {
            let mut n = 0;
            if map.get("type") == Some(&json!("bonfire")) {
                for (k, f) in fields.as_object().expect("fields is an object") {
                    map.insert(k.clone(), f.clone());
                }
                n += 1;
            }
            for child in map.values_mut() {
                n += patch_bonfires(child, fields);
            }
            n
        }
        Value::Array(items) => items.iter_mut().map(|c| patch_bonfires(c, fields)).sum(),
        _ => 0,
    }
}

/// Build the fixture with `fields` added to its one bonfire, through the default
/// (all-languages) path that tags every player-visible string with its key, as
/// `delvec build` does.
fn build_with(fields: Value) -> BuildOutput {
    let mut loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let mut patched = 0;
    loaded.raw.quests = common::patch_doc(&loaded.raw.quests, |q| {
        patched = patch_bonfires(q, &fields);
    });
    assert_eq!(patched, 1, "binding: the fixture holds exactly one bonfire");
    let mut c = parse_campaign(&loaded.raw).expect("souls-bonfire parses");
    delvewright_dsl::tag_translatables(&mut c);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &c.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(
                fixture_dir()
                    .join("skins")
                    .join(format!("{}.png", skin.texture_id)),
            )
            .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &skins,
    )
    .expect("every emitted command validates")
}

fn dialog(out: &BuildOutput) -> Value {
    serde_json::from_slice(out.get(DIALOG).expect("bonfire dialog emitted")).unwrap()
}

/// A stated tooltip lands on its own button, beside the label, under the key
/// `fx.….rest_tooltip` / `fx.….save_tooltip`; the action is untouched.
#[test]
fn a_bonfire_button_carries_its_stated_tooltip() {
    let out = build_with(json!({
        "rest_tooltip": "Rest: the dead rise again, and the fire keeps you.",
        "save_tooltip": "Save: the fire keeps you; the dead stay dead."
    }));
    let dlg = dialog(&out);
    let actions = dlg["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 2, "a bonfire offers exactly two buttons");
    for (i, (field, text, cmd)) in [
        (
            "rest_tooltip",
            "Rest: the dead rise again, and the fire keeps you.",
            "/trigger dw.rest set 2",
        ),
        (
            "save_tooltip",
            "Save: the fire keeps you; the dead stay dead.",
            "/trigger dw.rest set 1",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let tip = &actions[i]["tooltip"];
        assert_eq!(tip["fallback"], text, "button {i}: {:#?}", actions[i]);
        let key = tip["translate"]
            .as_str()
            .expect("a tagged tooltip translates");
        assert!(
            key.starts_with(&format!("delve.{NS}.fx.")) && key.ends_with(&format!(".{field}")),
            "button {i}'s tooltip key is `{key}`"
        );
        assert_eq!(actions[i]["action"]["command"], cmd);
        assert!(actions[i]["label"].is_object(), "the label is still there");
    }
}

/// One tooltip may be stated without the other: each is per button.
#[test]
fn a_bonfire_tooltip_is_per_button() {
    let dlg = dialog(&build_with(json!({ "save_tooltip": "Only this one." })));
    let actions = dlg["actions"].as_array().unwrap();
    assert!(actions[0].get("tooltip").is_none(), "{:#?}", actions[0]);
    assert_eq!(actions[1]["tooltip"]["fallback"], "Only this one.");
}

/// Perturbation: changing a declared tooltip moves an emitted byte of the
/// bonfire dialog, and only of the bonfire dialog among the dialogs.
#[test]
fn changing_a_bonfire_tooltip_moves_the_emitted_bytes() {
    let a = build_with(json!({ "rest_tooltip": "The fire is warm." }));
    let b = build_with(json!({ "rest_tooltip": "The fire is cold." }));
    assert_ne!(
        a.get(DIALOG),
        b.get(DIALOG),
        "the perturbation reached no byte"
    );
    let dialogs: Vec<&String> = a.keys().filter(|k| k.contains("/dialog/")).collect();
    assert!(!dialogs.is_empty(), "binding: no dialog emitted");
    for k in dialogs {
        if k != DIALOG {
            assert_eq!(a.get(k), b.get(k), "{k} moved with a bonfire tooltip");
        }
    }
}

/// ADR-0006 byte identity: a campaign that states no tooltip emits no tooltip
/// key, and stating one moves the bonfire dialog and no other datapack file.
/// (The gallery baseline and every fixture that states none are the
/// before/after half: their bytes do not move with this change.)
#[test]
fn no_stated_tooltip_emits_no_tooltip_key() {
    let out = build_with(json!({}));
    let dlg = dialog(&out);
    for a in dlg["actions"].as_array().unwrap() {
        assert!(a.get("tooltip").is_none(), "{a:#?}");
    }
    let stated = build_with(json!({ "rest_tooltip": "x" }));
    let moved: Vec<&String> = out
        .keys()
        .filter(|k| k.starts_with("datapack/") && out.get(*k) != stated.get(*k))
        .collect();
    assert_eq!(
        moved,
        vec![&DIALOG.to_string()],
        "stating a tooltip moves the bonfire dialog and nothing else in the datapack"
    );
}

/// Every button in every emitted dialog has the one shape the shared helper
/// builds: `label`, an optional `tooltip`, and a `/trigger` action — so no site
/// emits a button by hand. Bound over the fixture's dialogs.
#[test]
fn every_emitted_button_has_the_one_shape() {
    let out = build_with(json!({ "rest_tooltip": "r", "save_tooltip": "s" }));
    let mut buttons = 0;
    for (k, bytes) in out.iter().filter(|(k, _)| k.contains("/dialog/")) {
        let d: Value = serde_json::from_slice(bytes).unwrap();
        for a in d["actions"].as_array().into_iter().flatten() {
            let keys: Vec<&String> = a.as_object().unwrap().keys().collect();
            assert!(
                keys.iter()
                    .all(|k| ["label", "tooltip", "action"].contains(&k.as_str())),
                "{k}: {a:#?}"
            );
            assert_eq!(a["action"]["type"], "minecraft:run_command", "{k}");
            assert!(
                a["action"]["command"]
                    .as_str()
                    .unwrap()
                    .starts_with("/trigger "),
                "{k}"
            );
            buttons += 1;
        }
    }
    assert!(buttons >= 4, "binding: only {buttons} buttons emitted");
    println!("button shape binding: {buttons} buttons");
}
