//! **A showcase picture of a story moment is the world one play of its path
//! leaves** — through the binary, on hello-world copies, every world read
//! back by `tools/lib/anvil.py`, the reader the written-world gate uses.
//!
//! - the cast: a moment after a step that moves only the keeper is a moment
//!   (`DW0721` refused it as the load world), and its world stands the keeper
//!   where the cast ledger stations him then;
//! - the watch: a body that turns to the nearest player within its reach
//!   turns to the lens, where a player stands for the picture;
//! - the order: a write fired at a step the path played before the moment is
//!   in the moment's world even where the step is no causal ancestor of the
//!   next, and the printed moved-cell count is the count of cells the written
//!   world differs from load in;
//! - the renderer: a body of a kind it draws nothing of, in frame, refuses an
//!   hq picture under `DW1100` and is named on a preview.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_dsl::DSL_VERSION;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("story-moment-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(BIN).args(args).output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

/// The test library with a point anchor `anchor/pillar` on `hello-room`'s
/// floor corner (local `[1, 1, 1]`), a cell no leg walks.
fn library(tag: &str) -> PathBuf {
    let dir = tmp(&format!("{tag}-prefabs"));
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["anchors"]["anchor/pillar"] = serde_json::json!({ "pos": [1, 1, 1] });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

/// A hello-world copy with one approved picture, its quest edited by `edit`,
/// its keeper by `npc`, and `cameras` (`(name, after)`) as its record, each
/// at the fixture's lens.
fn campaign(
    tag: &str,
    edit: impl FnOnce(&mut serde_json::Value),
    npc: impl FnOnce(&mut serde_json::Value),
    cameras: &[(&str, Option<&str>)],
) -> PathBuf {
    let camp = tmp(&format!("{tag}-camp"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |q| {
        edit(&mut q["content"]["quests"][0])
    });
    common::patch_file(&camp.join("npcs.json"), |n| {
        npc(&mut n["content"]["npcs"][0])
    });
    std::fs::create_dir_all(camp.join("design/concept")).unwrap();
    std::fs::write(camp.join("design/concept/keep.png"), b"not read").unwrap();
    std::fs::write(
        camp.join("design.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "dsl_version": DSL_VERSION, "campaign_id": "hello-world", "stage": "design",
            "content": {"references": [
                {"name": "concept/keep", "shows": "x", "time": "noon", "weather": "clear"}
            ]}
        }))
        .unwrap(),
    )
    .unwrap();
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            common::repo_root().join("crates/delvec/tests/fixtures/post-beat-camera/cameras.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let base = fixture["cameras"][0].clone();
    let cams: Vec<serde_json::Value> = cameras
        .iter()
        .map(|(name, after)| {
            let mut c = base.clone();
            c["name"] = serde_json::json!(name);
            if let Some(a) = after {
                c["after"] = serde_json::json!({ "step": a });
            }
            c
        })
        .collect();
    std::fs::write(
        camp.join("design/cameras.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"campaign_id": "hello-world", "cameras": cams}),
        )
        .unwrap(),
    )
    .unwrap();
    camp
}

fn built(tag: &str, camp: &Path, lib: &Path) -> PathBuf {
    let out = tmp(&format!("{tag}-out"));
    let (code, log) = run(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        lib.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    out
}

fn cameras(build: &Path, camp: &Path, lib: &Path, out: &Path, extra: &[&str]) -> (i32, String) {
    let mut args = vec![
        "--prefabs",
        lib.to_str().unwrap(),
        "cameras",
        build.to_str().unwrap(),
        "--campaign",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--backend",
        "cpu",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

/// Run `tools/lib/anvil.py` over a written world: `what` is `entities` (every
/// body's compound) or `cells` (every non-air cell as `"x,y,z" -> state`).
fn anvil(world: &Path, what: &str) -> serde_json::Value {
    let lib = common::repo_root().join("tools/lib");
    let body = match what {
        "entities" => "print(json.dumps(anvil.entities(d)))",
        _ => "print(json.dumps({f'{x},{y},{z}': s for (x, y, z), s in anvil.cells(d).items()}))",
    };
    let script = format!(
        "import sys, json, pathlib\nsys.path.insert(0, {lib:?})\nimport anvil\n\
         d = pathlib.Path({dir:?})\n{body}\n",
        lib = lib.display().to_string(),
        dir = world.display().to_string(),
    );
    let path = world.with_extension(format!("{what}.py"));
    std::fs::write(&path, script).unwrap();
    let o = Command::new("python3").arg(&path).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).unwrap()
}

/// The keeper's body (`dw_npc` and `dw_npc_keeper`) in a written world: its
/// position and yaw.
fn keeper(world: &Path) -> ([f64; 3], f64) {
    let all = anvil(world, "entities");
    let body = all
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            let tags: Vec<&str> = e["Tags"]
                .as_array()
                .map(|t| t.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();
            tags.contains(&"dw_npc") && tags.contains(&"dw_npc_keeper")
        })
        .unwrap_or_else(|| panic!("no keeper in {}: {all}", world.display()));
    let p = &body["Pos"];
    (
        [
            p[0].as_f64().unwrap(),
            p[1].as_f64().unwrap(),
            p[2].as_f64().unwrap(),
        ],
        body["Rotation"][0].as_f64().unwrap(),
    )
}

/// `obj/look` first (reach the latch), `obj/talk` after it, `obj/exit` after
/// `obj/talk`; completing `obj/look` sets `flag/looked` and walks the keeper
/// two cells west of his stand, and the cast ledger stations him by the flag.
/// No block moves at `obj/look`.
fn the_keeper_steps_aside(q: &mut serde_json::Value) {
    q["cast"] = serde_json::json!({"npc/keeper": [
        {"at": "anchor/keeper-stand", "dialogue": "dlg/greeting", "doing": "keeping to his stand",
         "forbids_flags": ["flag/looked"]},
        {"at": {"anchor": "anchor/keeper-stand", "offset": [-2, 0, 0]}, "dialogue": "dlg/greeting",
         "doing": "stepped aside to watch the party", "requires_flags": ["flag/looked"]}
    ]});
    let objectives = q["objectives"].as_array_mut().unwrap();
    objectives.insert(
        0,
        serde_json::json!({
            "type": "reach-anchor", "id": "obj/look", "anchor": "anchor/latch", "radius": 2,
            "title": "Look around", "hint": "The room by the door.",
            "happening": {"text": "the party completes obj/look", "verb": "arrives"}
        }),
    );
    objectives[1]["after"] = serde_json::json!(["obj/look"]);
    q["on_objective_complete"]["obj/look"] = serde_json::json!([
        {"type": "set-flag", "flag": "flag/looked"},
        {"type": "move-npc", "npc": "npc/keeper",
         "to": {"anchor": "anchor/keeper-stand", "offset": [-2, 0, 0]},
         "when": {"requires_flags": ["flag/looked"]},
         "happening": {"text": "the keeper steps aside", "verb": "departs"}}
    ]);
}

/// **The cast at a moment, and the watch at the lens.** The keeper stands on
/// his stand at load (`[5.5, 65, 4.5]`, facing north) and, after `obj/look`,
/// two cells west where the cast ledger stations him: a moment whose only
/// difference from load is its cast is pictured, not refused. Each frame's
/// lens is within the keeper's default reach, so each frame stands in a copy
/// of its world whose keeper faces the lens.
#[test]
fn a_moment_holds_the_cast_the_ledger_stations_and_a_watcher_faces_the_lens() {
    let camp = campaign(
        "cast",
        the_keeper_steps_aside,
        |_| {},
        &[("hero", None), ("aside", Some("obj/look"))],
    );
    let lib = common::prefabs_dir();
    let out = built("cast", &camp, &lib);
    let scenes = tmp("cast-scenes");
    let (code, log) = cameras(&out, &camp, &lib, &scenes, &["--preview"]);
    assert_eq!(code, 0, "{log}");
    assert!(
        log.contains("after: aside after obj/look")
            && log.contains("0 cells moved from load, 1 of the cast moved, sky as at load"),
        "{log}"
    );
    let worlds = scenes.join("worlds");
    let (load, load_yaw) = keeper(&worlds.join("at-load"));
    let (aside, _) = keeper(&worlds.join("after-obj-look"));
    assert_eq!(load, [5.5, 65.0, 4.5], "the keeper on his stand at load");
    assert_eq!(load_yaw, 180.0, "facing his stand's facing, north, at rest");
    assert_eq!(aside, [3.5, 65.0, 4.5], "two cells west after obj/look");
    // The lens is a player within the keeper's reach: he turns to it.
    let lens = [5.5, 66.62, 2.5];
    for (frame, at) in [
        ("at-load.facing-hello-world_camera_hero", load),
        ("after-obj-look.facing-hello-world_camera_aside", aside),
    ] {
        let (pos, yaw) = keeper(&worlds.join(frame));
        assert_eq!(pos, at, "{frame}: the turn moves nobody");
        let want = delvec::compiler::watching::bearing(at, lens);
        assert!(
            (yaw - want).abs() < 1e-3,
            "{frame}: yaw {yaw}, the bearing to the lens {want}"
        );
    }
    assert!(
        log.contains("facing: 2 bod(ies) turned to the lens over 2 of 2 frame(s)"),
        "{log}"
    );
    // The frame's manifest names the world it stands in.
    let m: serde_json::Value = serde_json::from_slice(
        &std::fs::read(scenes.join("hello-world_camera_aside_preview.manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        m["world"]["path"],
        "worlds/after-obj-look.facing-hello-world_camera_aside"
    );
}

/// **The moment's blocks are laid in the path's order.** `obj/look` (no
/// `after`) fills a gold cell at `anchor/pillar`; `obj/talk` (no `after`)
/// opens the door; `obj/exit` follows `obj/talk` alone. The path plays
/// `obj/look` before `obj/talk`, so the world after `obj/talk` holds the gold
/// cell, though `obj/look` is no causal ancestor of the step after it; and
/// the printed count of moved cells is the count of cells the written world
/// differs from the written load world in.
#[test]
fn a_moment_lays_every_write_its_path_fired_and_counts_what_it_wrote() {
    let lib = library("order");
    let camp = campaign(
        "order",
        |q| {
            let objectives = q["objectives"].as_array_mut().unwrap();
            objectives.insert(
                0,
                serde_json::json!({
                    "type": "reach-anchor", "id": "obj/look", "anchor": "anchor/latch",
                    "radius": 2, "title": "Look around", "hint": "The room by the door.",
                    "happening": {"text": "the party completes obj/look", "verb": "arrives"}
                }),
            );
            q["on_objective_complete"]["obj/look"] = serde_json::json!([
                {"type": "fill-region", "block": "minecraft:gold_block",
                 "region": {"anchor": "anchor/pillar", "extent": [0, 0, 0]}}
            ]);
        },
        |_| {},
        &[("hero", None), ("open", Some("obj/talk"))],
    );
    let out = built("order", &camp, &lib);
    let scenes = tmp("order-scenes");
    let (code, log) = cameras(&out, &camp, &lib, &scenes, &["--preview"]);
    assert_eq!(code, 0, "{log}");
    let worlds = scenes.join("worlds");
    let read = |k: &str| -> BTreeMap<String, String> {
        serde_json::from_value(anvil(&worlds.join(k), "cells")).unwrap()
    };
    let (load, open) = (read("at-load"), read("after-obj-talk"));
    let gold = |w: &BTreeMap<String, String>| {
        w.get("1,65,1")
            .is_some_and(|s| s.starts_with("minecraft:gold_block"))
    };
    assert!(!gold(&load), "no gold at load");
    assert!(gold(&open), "the gold obj/look laid, after obj/talk");
    let differ = load
        .keys()
        .chain(open.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|c| load.get(*c) != open.get(*c))
        .count();
    let line = log
        .lines()
        .find(|l| l.starts_with("after: open after obj/talk"))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(
        line.contains(&format!(": {differ} cells moved from load,")),
        "{differ} cells differ in the written worlds: {line}"
    );
    assert!(differ > 6, "the door's six cells and the gold: {differ}");
}

/// **`DW1100`: a body the renderer draws nothing of, in frame.** The keeper is
/// an allay, a kind the pinned renderer has no model for, standing in the
/// frame of the fixture's lens turned south to him: the hq picture is refused (exit 5) and not written; a
/// preview names it and is written.
#[test]
fn a_frame_of_a_body_the_renderer_cannot_draw_is_refused() {
    let lib = common::prefabs_dir();
    let camp = campaign(
        "undrawn",
        |_| {},
        |n| n["base_entity"] = serde_json::json!("minecraft:allay"),
        &[("hero", None)],
    );
    // The fixture's lens turned south, to the keeper.
    common::patch_file(&camp.join("design/cameras.json"), |c| {
        c["cameras"][0]["yaw"] = serde_json::json!(0.0)
    });
    let out = built("undrawn", &camp, &lib);
    let scenes = tmp("undrawn-scenes");
    let (code, log) = cameras(&out, &camp, &lib, &scenes, &[]);
    assert_eq!(code, 5, "{log}");
    assert!(
        log.contains("DW1100") && log.contains("minecraft:allay dw_npc_keeper at [5.5, 65.0, 4.5]"),
        "{log}"
    );
    assert!(
        !scenes.join("hello-world_camera_hero.png").exists(),
        "the refused picture is not written"
    );
    let (code, log) = cameras(&out, &camp, &lib, &tmp("undrawn-preview"), &["--preview"]);
    assert_eq!(code, 0, "{log}");
    assert!(log.contains("DW1100"), "{log}");
    // The same keeper as a villager: drawn, nothing refused.
    let camp = campaign("drawn", |_| {}, |_| {}, &[("hero", None)]);
    common::patch_file(&camp.join("design/cameras.json"), |c| {
        c["cameras"][0]["yaw"] = serde_json::json!(0.0)
    });
    let out = built("drawn", &camp, &lib);
    let (code, log) = cameras(&out, &camp, &lib, &tmp("drawn-scenes"), &[]);
    assert_eq!(code, 0, "{log}");
    assert!(
        log.contains("undrawn: 0 of 1 frame(s) frame a body the renderer draws nothing of"),
        "{log}"
    );
}
