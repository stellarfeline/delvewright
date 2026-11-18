//! An `interact` pressed over an area (`region`) instead of one block.
//!
//! On hello-world (the room seated at y 64), an interact over the air on the
//! floor by the latch, three cells along x on the spawn side of the door wall:
//!
//! * the green: it builds; nothing is placed; its body is three tiles of
//!   `ClickBox::tiling` summoned by `area_obj_sweep` at setup, each wearing the
//!   fixture class, the area class and the objective's tag; the objective's
//!   advancement is the box's `player_interacted_with_entity`; the exported
//!   step carries the stance the build proved and the tiles; the press ledger
//!   names the area;
//! * the perturbation: no extent along x is one tile, not three;
//! * the reds: a region that reaches the door wall has a block on its top face
//!   (`DW1143`); `prop` and `region` both (`DW1059`); `release` on an area
//!   (`DW1051`); a region over the keeper's own cell encloses his body
//!   (`DW0359`).

use crate::common;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// hello-world with `obj/sweep` between the keeper's talk and the exit,
/// `patch` applied to the objective.
fn fixture(tag: &str, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("interact-area-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    let mut o = json!({
        "type": "interact", "id": "obj/sweep", "anchor": "anchor/latch",
        "region": {"anchor": "anchor/latch", "extent": [1, 0, 0]},
        "after": ["obj/talk"],
        "title": "Sweep the floor by the latch", "hint": "Anywhere along the floor by the latch.",
        "answers": {"not_yet": "Not yet.", "done": "Swept already."},
        "happening": {"text": "the party sweeps the floor", "verb": "opens"}
    });
    patch(&mut o);
    common::patch_file(&dir.join("quests.json"), |q| {
        let objs = q["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap();
        let exit = objs.pop().unwrap();
        assert_eq!(exit["id"], "obj/exit");
        objs.push(o);
        let mut exit = exit;
        exit["after"] = json!(["obj/sweep"]);
        objs.push(exit);
    });
    dir
}

fn build(dir: &Path) -> (i32, String, PathBuf) {
    let out = dir.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "build",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .expect("delvec runs");
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        out,
    )
}

/// The emitted file under `out` whose path ends with `suffix`, as text.
fn emitted(out: &Path, suffix: &str) -> Option<String> {
    let mut stack = vec![out.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if !p.to_string_lossy().contains("packtest") {
                    stack.push(p);
                }
            } else if p.to_string_lossy().ends_with(suffix) {
                return Some(std::fs::read_to_string(&p).unwrap_or_default());
            }
        }
    }
    None
}

fn sweep_step(out: &Path) -> Value {
    let path: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("critical-path.json")).unwrap())
            .unwrap();
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/sweep")
        .cloned()
        .expect("the critical path sweeps")
}

#[test]
fn an_area_interact_is_tiled_placed_nowhere_and_clicked_from_its_stance() {
    let (code, log, out) = build(&fixture("green", |_| {}));
    assert_eq!(code, 0, "{log}");
    assert!(
        log.contains("area binding (DW1143): 1 area interact(s) judged: `obj/sweep`"),
        "{log}"
    );

    // The tiles: the box x 1..=3, y 1, z 5 cut into three squares one cell
    // wide, every face a margin past the grid, each wearing both classes and
    // the objective's tag.
    let tiles = emitted(&out, "/function/area_obj_sweep.mcfunction").expect("the area's function");
    let summons: Vec<&str> = tiles.lines().collect();
    assert_eq!(summons.len(), 3, "{tiles}");
    for (i, line) in summons.iter().enumerate() {
        let x = 1.5 + i as f64;
        assert!(
            line.starts_with(&format!(
                "summon minecraft:interaction {x:.1} 64.99 5.5 {{width:1.02f,height:1.02f,"
            )),
            "{line}"
        );
        assert!(
            line.contains("Tags:[\"dw_fixture\",\"dw_area\",\"dw_i_sweep\""),
            "{line}"
        );
    }

    // Summoned once at setup; nothing placed over the area.
    let setup = emitted(&out, "/function/setup_finish.mcfunction").expect("setup_finish");
    assert!(
        setup.lines().any(|l| l
            .starts_with("execute unless entity @e[tag=dw_i_sweep] run function ")
            && l.ends_with(":area_obj_sweep")),
        "{setup}"
    );
    for l in setup.lines() {
        assert!(
            !["setblock 1 65 5 ", "setblock 2 65 5 ", "setblock 3 65 5 "]
                .iter()
                .any(|p| l.starts_with(p)),
            "an area places no block: {l}"
        );
    }

    // The act is the box's own report, as the presser.
    let adv: Value = serde_json::from_str(
        &emitted(&out, "/advancement/i_sweep.json").expect("the interact's advancement"),
    )
    .unwrap();
    let crit = &adv["criteria"]["interact"];
    assert_eq!(
        crit["trigger"], "minecraft:player_interacted_with_entity",
        "{crit}"
    );
    assert_eq!(
        crit["conditions"]["entity"]["nbt"], "{Tags:[\"dw_i_sweep\"]}",
        "{crit}"
    );

    // The bot is handed the stance, the cell it clicks and the tiles.
    let step = sweep_step(&out);
    let stand = step["stand"].as_array().expect("a stance").clone();
    let stand: Vec<i64> = stand.iter().map(|v| v.as_i64().unwrap()).collect();
    assert!(
        !(stand[2] == 5 && (1..=3).contains(&stand[0])),
        "the stance is off the area's columns: {step}"
    );
    let cell: Vec<i64> = step["area"]["cell"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert!(
        cell[1] == 65 && cell[2] == 5 && (1..=3).contains(&cell[0]),
        "{step}"
    );
    assert_eq!(step["area"]["tiles"].as_array().unwrap().len(), 3, "{step}");

    // The ledger names the area and its tiles.
    let ledger: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/press-bodies.json")).unwrap(),
    )
    .unwrap();
    let row = ledger["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["owner"] == "obj-sweep")
        .expect("the area's row")
        .clone();
    assert_eq!(
        row["area"]["region"],
        json!([[1, 65, 5], [3, 65, 5]]),
        "{row}"
    );
    assert_eq!(row["form"], "boxed: unobservable", "{row}");
}

/// The extent is what the tiles are cut from: none along x is the one tile
/// over the latch's own cell (two more each way reaches the west wall, a lid).
#[test]
fn the_extent_moves_the_tiles() {
    let (code, log, out) = build(&fixture("wider", |o| {
        o["region"]["extent"] = json!([0, 0, 0]);
    }));
    assert_eq!(code, 0, "{log}");
    let tiles = emitted(&out, "/function/area_obj_sweep.mcfunction").expect("the area's function");
    let xs: Vec<&str> = tiles
        .lines()
        .map(|l| l.split(' ').nth(2).unwrap())
        .collect();
    assert_eq!(xs, ["2.5"], "{tiles}");
}

/// The region reaches the door wall at z 6: the wall stands on its top face
/// there, a lid over that part of the declared surface.
#[test]
fn a_block_on_the_area_top_face_is_refused() {
    let (code, log, _) = build(&fixture("lid", |o| {
        o["region"]["extent"] = json!([1, 0, 1]);
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.contains("DW1143")
            && log.contains("`obj/sweep`")
            && log.contains("directly on its top face"),
        "{log}"
    );
}

#[test]
fn a_prop_and_a_region_both_are_refused() {
    let (code, log, _) = build(&fixture("both", |o| {
        o["prop"] = json!({"block": "minecraft:stone_button[face=floor,facing=north]"});
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.lines().any(|l| l.starts_with("DW1059 [error]")
            && l.contains("`obj/sweep`")
            && l.contains("names both a `prop` and a `region`")),
        "{log}"
    );
}

#[test]
fn neither_a_prop_nor_a_region_names_both_remedies() {
    let (code, log, _) = build(&fixture("neither", |o| {
        o.as_object_mut().unwrap().remove("region");
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.lines().any(|l| l.starts_with("DW1059 [error]")
            && l.contains("`obj/sweep`")
            && l.contains("declare `\"region\"")),
        "{log}"
    );
}

#[test]
fn a_released_area_is_refused() {
    let (code, log, _) = build(&fixture("release", |o| {
        o["release"] = json!(true);
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.lines().any(|l| l.starts_with("DW1051 [error]")
            && l.contains("`obj/sweep`")
            && l.contains("releases its area")),
        "{log}"
    );
}

/// A region over the keeper's own cell encloses his body: the area eclipses
/// the man, and he it.
#[test]
fn an_area_over_a_standing_body_is_refused() {
    let (code, log, _) = build(&fixture("keeper", |o| {
        o["region"] = json!({"anchor": "anchor/keeper-stand", "extent": [0, 0, 0]});
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.contains("DW0359") && log.contains("`obj/sweep`") && log.contains("npc/keeper"),
        "{log}"
    );
}
