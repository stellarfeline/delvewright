//! spec-0134 §6 — a fill waits for its beat. `loot[].filled: "by-effect"`
//! moves a fill out of `setup_finish` into its own `fill_<loot>` function,
//! which the `fill-loot` verb calls (emission is proven in `emit::loot`'s
//! unit tests, `a_fill_that_waits_is_its_own_function`); here, the document
//! tier's `DW1093` both ways and `DW0112` for a fill nobody declares.
//!
//! The world-edits half: `applied: "by-effect"` on a batch takes its writes
//! out of the world build into its own `edit_<batch>` function, which the
//! `apply-edits` verb calls; `DW1094` both ways; and the light proof judges
//! the batch applied (a batch that takes the hall's lamps is `DW0210`) as
//! well as the world without it (the final build).

use crate::common;

use delvewright_dsl::{DSL_VERSION, Diagnostic, RawCampaign, parse_campaign, validate_campaign};
use serde_json::{Value, json};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn diags(filled: Option<&str>, fill_loot: Option<&str>) -> Vec<Diagnostic> {
    let mut q: Value = serde_json::from_str(&read_hw("quests.json")).unwrap();
    q["dsl_version"] = json!(DSL_VERSION);
    let mut l = json!({"id": "loot/supplies", "anchor": "anchor/keeper-stand",
                       "items": [{"item": "minecraft:bread", "count": 3}]});
    if let Some(f) = filled {
        l["filled"] = json!(f);
    }
    q["content"]["loot"] = json!([l]);
    if let Some(id) = fill_loot {
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "fill-loot", "loot": id,
                         "happening": {"text": "the supplies are laid out", "verb": "learns"}}));
    }
    let c = parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: q.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("parses");
    validate_campaign(&c)
}

fn codes(d: &[Diagnostic], code: &str) -> Vec<String> {
    d.iter()
        .filter(|x| x.code == code)
        .map(|x| x.path.clone())
        .collect()
}

#[test]
fn a_fill_that_waits_and_its_beat_agree() {
    let d = diags(Some("by-effect"), Some("loot/supplies"));
    assert!(codes(&d, "DW1093").is_empty(), "{d:#?}");
    assert!(codes(&d, "DW0112").is_empty(), "{d:#?}");
}

#[test]
fn a_fill_nothing_fires_is_dw1093() {
    let d = diags(Some("by-effect"), None);
    assert_eq!(
        codes(&d, "DW1093"),
        vec!["/content/loot/0/filled"],
        "{d:#?}"
    );
}

#[test]
fn a_second_fill_of_a_filled_holder_is_dw1093() {
    let d = diags(None, Some("loot/supplies"));
    assert_eq!(
        codes(&d, "DW1093"),
        vec!["/content/quests/0/on_objective_complete/obj/talk/1/loot"],
        "{d:#?}"
    );
    let d = diags(Some("at-load"), Some("loot/supplies"));
    assert_eq!(codes(&d, "DW1093").len(), 1, "{d:#?}");
}

#[test]
fn a_fill_nobody_declares_is_dw0112() {
    let d = diags(Some("by-effect"), Some("loot/nowhere"));
    assert!(
        codes(&d, "DW0112")
            .contains(&"/content/quests/0/on_objective_complete/obj/talk/1/loot".to_string()),
        "{d:#?}"
    );
    assert_eq!(codes(&d, "DW1093"), vec!["/content/loot/0/filled"]);
}

// ---------------------------------------------------------------------------
// The world-edits half: a batch that waits for its beat
// ---------------------------------------------------------------------------

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn edits_copy(name: &str) -> std::path::PathBuf {
    let dst = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dst);
    common::copy_dir_all(&common::compiler_fixtures_dir().join("v06-edits"), &dst);
    dst
}

/// Mark `batch` `applied: by-effect` (or leave it), and push an `apply-edits`
/// of `apply` on the keeper's talk.
fn defer(dir: &std::path::Path, batch: Option<&str>, apply: Option<&str>) {
    let b = batch.map(str::to_string);
    common::patch_file(&dir.join("world-edits.json"), move |w| {
        if let Some(b) = b {
            for x in w["content"]["batches"].as_array_mut().unwrap() {
                if x["id"] == b.as_str() {
                    x["applied"] = json!("by-effect");
                }
            }
        }
    });
    let a = apply.map(str::to_string);
    common::patch_file(&dir.join("quests.json"), move |q| {
        if let Some(a) = a {
            q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type": "apply-edits", "batch": a,
                             "happening": {"text": "the keep is dressed", "verb": "learns"}}));
        }
    });
}

fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(BIN)
        .args(args)
        .current_dir(common::repo_root())
        .output()
        .expect("run delvec")
}

fn build_dir(dir: &std::path::Path, out: &std::path::Path) -> std::process::Output {
    let _ = std::fs::remove_dir_all(out);
    run(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &common::prefabs_dir().display().to_string(),
    ])
}

fn all_functions(out: &std::path::Path) -> std::collections::BTreeMap<String, String> {
    let fdir = out.join("datapack/data/hello-world/function");
    std::fs::read_dir(&fdir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_stem().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect()
}

/// A batch that waits for its beat is its own function, holding exactly the
/// writes the world build held for it, and the beat calls it; the world build
/// no longer writes them.
#[test]
fn a_batch_that_waits_is_its_own_function_and_its_beat_calls_it() {
    let at_build = edits_copy("deferred-edit-at-build");
    let out_a = at_build.with_extension("out");
    assert!(build_dir(&at_build, &out_a).status.success());
    let before = all_functions(&out_a);
    let dir = edits_copy("deferred-edit-by-effect");
    defer(&dir, Some("batch/lamplight"), Some("batch/lamplight"));
    let out = dir.with_extension("out");
    let r = build_dir(&dir, &out);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let fns = all_functions(&out);
    let edit = fns
        .get("edit_lamplight")
        .expect("edit_lamplight is emitted");
    assert!(!edit.trim().is_empty());
    for l in edit.lines() {
        assert!(l.starts_with("fill ") || l.starts_with("setblock "), "{l}");
    }
    let built = |fns: &std::collections::BTreeMap<String, String>| -> Vec<String> {
        fns.iter()
            .filter(|(k, _)| k.starts_with("world_build"))
            .flat_map(|(_, v)| v.lines().map(str::to_string).collect::<Vec<_>>())
            .collect()
    };
    let (was, now) = (built(&before), built(&fns));
    for l in edit.lines() {
        assert!(was.iter().any(|w| w == l), "the at-build world wrote `{l}`");
        assert!(
            !now.iter().any(|w| w == l),
            "the world build still writes `{l}`"
        );
    }
    assert!(
        fns.values().any(|b| b
            .lines()
            .any(|l| l == "function hello-world:edit_lamplight")),
        "the beat calls the batch"
    );
    assert!(!before.contains_key("edit_lamplight"));
}

/// The light proof judges the batch applied: a batch that takes the keep's
/// four lanterns waits for its beat and is still `DW0210`.
#[test]
fn a_batch_that_takes_the_lamps_is_dw0210_though_it_waits() {
    let dir = edits_copy("deferred-edit-dark");
    common::patch_file(&dir.join("world-edits.json"), |w| {
        w["content"]["batches"].as_array_mut().unwrap().push(json!({
            "id": "batch/lights-out", "area": "area/keep", "applied": "by-effect",
            "edits": [
                {"verb": "select", "name": "region/lamp-row",
                 "shape": {"kind": "box", "frame": {"kind": "piece-local", "piece": 0,
                           "prefab": "prefab/hello-room"}, "min": [1, 4, 1], "max": [9, 4, 9]}},
                {"verb": "select", "name": "region/lamps",
                 "shape": {"kind": "palette-match", "blocks": ["minecraft:lantern"],
                           "within": "region/lamp-row"}},
                {"verb": "carve", "region": "region/lamps"}
            ]
        }));
    });
    defer(&dir, None, Some("batch/lights-out"));
    let out = dir.with_extension("out");
    let r = build_dir(&dir, &out);
    let err = String::from_utf8_lossy(&r.stderr).to_string();
    assert!(!r.status.success(), "the dark batch built");
    assert!(err.contains("DW0210"), "{err}");
    assert!(err.contains("batch/lights-out"), "{err}");
}

#[test]
fn a_batch_nothing_applies_and_a_second_write_are_dw1094() {
    let dir = edits_copy("deferred-edit-unapplied");
    defer(&dir, Some("batch/lamplight"), None);
    let r = run(&["validate", dir.to_str().unwrap()]);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(!r.status.success());
    assert!(
        all.contains("DW1094") && all.contains("/content/batches/6/applied"),
        "{all}"
    );
    let dir = edits_copy("deferred-edit-twice");
    defer(&dir, None, Some("batch/lamplight"));
    let r = run(&["validate", dir.to_str().unwrap()]);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(!r.status.success());
    assert!(
        all.contains("DW1094")
            && all.contains("/content/quests/0/on_objective_complete/obj/talk/1/batch"),
        "{all}"
    );
}
