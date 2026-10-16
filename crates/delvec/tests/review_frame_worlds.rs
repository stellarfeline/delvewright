//! **The build writes the worlds its review frames stand in, with their light,
//! their block entities and their bodies** (spec-0112 §5; criteria 3 and 4).
//!
//! Every world is read back through `tools/lib/anvil.py`, the one reader every
//! gate reads a world save with, so what is asserted is what a gate sees.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn build(campaign: &Path, prefabs: &Path, out: &Path) -> String {
    let o = Command::new(BIN)
        .arg("build")
        .arg(campaign)
        .arg("-o")
        .arg(out)
        .arg("--prefabs")
        .arg(prefabs)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
    stderr(&o)
}

fn json(p: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

/// `python3 -I tools/lib/anvil.py <args>`, its stdout as JSON.
fn anvil(args: &[&str]) -> serde_json::Value {
    let o = Command::new("python3")
        .arg("-I")
        .arg(common::repo_root().join("tools/lib/anvil.py"))
        .args(args)
        .output()
        .expect("python3 runs");
    assert!(o.status.success(), "anvil.py {args:?}: {}", stderr(&o));
    serde_json::from_slice(&o.stdout).unwrap()
}

/// Every file under `dir`, by its path relative to `dir`.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().to_string_lossy().into_owned();
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

/// Criterion 3: one world per distinct key the plan's shots state, each file
/// recorded in `manifest.json`, the binding line per world with its counts,
/// its light arrays, its hash and its wall time.
#[test]
fn the_build_writes_every_world_its_shots_stand_in() {
    let dir = tmp("rfw-keys");
    let out = dir.join("build");
    let log = build(&common::hello_world_dir(), &common::prefabs_dir(), &out);
    let plan = json(&out.join("render-plan.json"));
    let keys: std::collections::BTreeSet<String> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["world"].as_str().unwrap().to_string())
        .collect();
    assert!(
        keys.len() >= 2,
        "hello-world's shots stand at load and after the talk: {keys:?}"
    );
    let manifest = json(&out.join("manifest.json"));
    let outputs = manifest["outputs"].to_string();
    let worlds = out.join("validation/worlds");
    let written: std::collections::BTreeSet<String> = std::fs::read_dir(&worlds)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        written, keys,
        "one world per key the shots state, and no other"
    );
    for key in &keys {
        for (rel, _) in tree(&worlds.join(key)) {
            let path = format!("validation/worlds/{key}/{rel}");
            assert!(outputs.contains(&path), "{path} is not in manifest.json");
        }
        let line = log
            .lines()
            .find(|l| l.starts_with(&format!("world: {key} ")))
            .unwrap_or_else(|| panic!("no binding line for `{key}`:\n{log}"));
        for part in [
            " chunk(s), ",
            " cell(s), ",
            " ground cell(s), ",
            " block entit(ies), ",
            " entit(ies), light arrays over ",
            " sha256 ",
            &format!(" -> validation/worlds/{key}, written in "),
        ] {
            assert!(line.contains(part), "`{part}` in `{line}`");
        }
        assert!(line.ends_with(" ms"), "{line}");
    }
    let record = json(&out.join("validation/review-frames.json"));
    let recorded: std::collections::BTreeSet<String> = record["worlds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["key"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(recorded, keys);
}

/// Criterion 3: two builds write byte-identical worlds and review record
/// (ADR-0006).
#[test]
fn two_builds_write_byte_identical_worlds() {
    let dir = tmp("rfw-twice");
    let (a, b) = (dir.join("a"), dir.join("b"));
    build(&common::hello_world_dir(), &common::prefabs_dir(), &a);
    build(&common::hello_world_dir(), &common::prefabs_dir(), &b);
    let (ta, tb) = (
        tree(&a.join("validation/worlds")),
        tree(&b.join("validation/worlds")),
    );
    assert!(!ta.is_empty());
    assert_eq!(ta, tb, "the worlds differ between two builds");
    assert_eq!(
        std::fs::read(a.join("validation/review-frames.json")).unwrap(),
        std::fs::read(b.join("validation/review-frames.json")).unwrap()
    );
}

/// Criterion 3: `validation/worlds/at-load` is `delvec written-world`'s output
/// byte for byte.
#[test]
fn the_load_world_is_the_written_world_byte_for_byte() {
    let dir = tmp("rfw-written");
    let out = dir.join("build");
    build(&common::hello_world_dir(), &common::prefabs_dir(), &out);
    let written = dir.join("written");
    let o = Command::new(BIN)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .arg("written-world")
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&written)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
    let (load, ww) = (tree(&out.join("validation/worlds/at-load")), tree(&written));
    assert!(!load.is_empty());
    assert_eq!(load, ww, "the load world and `written-world` differ");
}

/// Criterion 3: every written section carries `BlockLight` and `SkyLight`;
/// the lantern's cell reads block light 15, a cell under the roof with no
/// emitter sky light 0, a cell over the roof sky light 15.
#[test]
fn every_section_carries_its_light() {
    let dir = tmp("rfw-light");
    let out = dir.join("build");
    build(&common::hello_world_dir(), &common::prefabs_dir(), &out);
    let world = out.join("validation/worlds/at-load");
    let w = world.to_str().unwrap();
    let census = anvil(&["light-census", w]);
    let sections = census["sections"].as_u64().unwrap();
    assert!(sections > 0, "{census}");
    assert_eq!(census["with_both"], sections, "{census}");
    // hello-room's lanterns hang at template (3, 4, 3) — world (3, 68, 3).
    assert_eq!(anvil(&["light", w, "3", "68", "3"])["block"], 15);
    let inside = anvil(&["light", w, "5", "66", "4"]);
    assert_eq!(inside["sky"], 0, "under the roof: {inside}");
    assert!(
        inside["block"].as_u64().unwrap() > 0,
        "lit by the lanterns: {inside}"
    );
    assert_eq!(
        anvil(&["light", w, "5", "70", "4"])["sky"],
        15,
        "over the roof"
    );
}

/// Criterion 3: an `ocean` horizon's world carries the generator's sea, water
/// to `SEA_LEVEL`, beside the piece where no write reached — the shore room
/// of `common::ocean_prefabs_dir`, eleven wide in a sixteen-wide chunk.
#[test]
fn the_ocean_horizon_writes_its_sea() {
    let dir = tmp("rfw-ocean");
    let pf = common::ocean_prefabs_dir("rfw-ocean-prefabs", common::OceanRoom::Shore);
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("world.json"), |v| {
        v["content"]["horizon"] = serde_json::json!("ocean");
        v["content"]["boundary"] = serde_json::json!({});
    });
    let out = dir.join("build");
    let log = build(&camp, &pf, &out);
    let line = log
        .lines()
        .find(|l| l.starts_with("world: at-load "))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(!line.contains(" 0 ground cell(s)"), "{line}");
    let w = out.join("validation/worlds/at-load");
    let cells = anvil(&[
        "cells",
        w.to_str().unwrap(),
        "12",
        "55",
        "5",
        "12",
        "70",
        "5",
    ]);
    let at = |y: i32| {
        cells[format!("12,{y},5")]
            .as_str()
            .map(|s| s.split('[').next().unwrap().to_string())
    };
    let sea = delvec::compiler::horizon::SEA_LEVEL;
    for y in 55..=sea {
        assert_eq!(at(y).as_deref(), Some("minecraft:water"), "y {y}: {cells}");
    }
    assert_eq!(at(sea + 1), None, "air over the sea: {cells}");
}

/// Criterion 3: the delve's image copies nothing under `validation/`, so no
/// written world ships.
#[test]
fn the_delve_image_copies_no_written_world() {
    let df =
        std::fs::read_to_string(common::repo_root().join("validation/Dockerfile.delve")).unwrap();
    let copies: Vec<&str> = df
        .lines()
        .filter(|l| l.trim_start().starts_with("COPY") || l.trim_start().starts_with("ADD"))
        .collect();
    assert!(!copies.is_empty(), "the Dockerfile copies nothing at all");
    for c in copies {
        assert!(!c.contains("validation/"), "{c}");
    }
}

/// Criterion 4: every body the ledger places at setup stands in the load
/// world's entity file with its whole summon compound, `Pos`, `Rotation`,
/// `Tags` and a `UUID`.
#[test]
fn every_ledger_body_stands_in_the_entity_file() {
    let dir = tmp("rfw-bodies");
    let out = dir.join("build");
    build(&common::hello_world_dir(), &common::prefabs_dir(), &out);
    let ledger = json(&out.join("validation/placed.json"));
    let rows: Vec<&serde_json::Value> = ledger["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["fired"] == "setup" && r["object"]["entity"].is_string())
        .collect();
    assert!(!rows.is_empty(), "hello-world's keeper is a setup body");
    let ents = anvil(&[
        "entities",
        out.join("validation/worlds/at-load").to_str().unwrap(),
    ]);
    let ents = ents.as_array().unwrap();
    assert_eq!(
        ents.len(),
        rows.len(),
        "one entity per setup body: {ents:?}"
    );
    for r in rows {
        let kind = r["object"]["entity"].as_str().unwrap();
        let pos: Vec<f64> = r["object"]["pos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().parse().unwrap())
            .collect();
        let found = ents
            .iter()
            .find(|x| {
                x["id"] == kind
                    && x["Pos"]
                        .as_array()
                        .is_some_and(|p| p.iter().zip(&pos).all(|(a, b)| a.as_f64() == Some(*b)))
            })
            .unwrap_or_else(|| panic!("no `{kind}` at {pos:?} in {ents:?}"));
        for key in ["Pos", "Rotation", "Tags", "UUID"] {
            assert!(!found[key].is_null(), "`{key}` on {found}");
        }
        // Every key of the summon's compound stands on the written body.
        let command = r["command"].as_str().unwrap();
        let compound = &command[command.find('{').unwrap()..];
        for key in compound_keys(compound) {
            assert!(!found[&key].is_null(), "`{key}` of `{command}` on {found}");
        }
    }
}

/// The top-level keys of an SNBT compound, by a reading of its own: the bare
/// word before each `:` at depth one, outside a string.
fn compound_keys(snbt: &str) -> Vec<String> {
    let (mut depth, mut quote, mut escaped) = (0i32, None::<char>, false);
    let (mut keys, mut word, mut in_key) = (Vec::new(), String::new(), false);
    for ch in snbt.chars() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' | '[' => {
                depth += 1;
                if depth == 1 {
                    in_key = true;
                    word.clear();
                }
            }
            '}' | ']' => depth -= 1,
            ',' if depth == 1 => {
                in_key = true;
                word.clear();
            }
            ':' if depth == 1 && in_key => {
                keys.push(word.trim().to_string());
                in_key = false;
            }
            c if depth == 1 && in_key => word.push(c),
            _ => {}
        }
    }
    keys
}

/// Write hello-room with a lettered oak sign standing at template `(8, 1, 8)`,
/// into a private copy of the pinned library.
fn library_with_a_sign(dir: &Path) -> PathBuf {
    use fastnbt::Value;
    use std::io::{Read, Write};
    let lib = dir.join("prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &lib);
    let path = lib.join("hello-room.nbt");
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(&path).unwrap())
        .read_to_end(&mut raw)
        .unwrap();
    let mut root: Value = fastnbt::from_bytes(&raw).unwrap();
    let Value::Compound(top) = &mut root else {
        panic!("a template is a compound")
    };
    let Some(Value::List(palette)) = top.get_mut("palette") else {
        panic!("a palette")
    };
    let mut sign = std::collections::HashMap::new();
    sign.insert(
        "Name".to_string(),
        Value::String("minecraft:oak_sign".into()),
    );
    let mut props = std::collections::HashMap::new();
    props.insert("rotation".to_string(), Value::String("8".into()));
    props.insert("waterlogged".to_string(), Value::String("false".into()));
    sign.insert("Properties".to_string(), Value::Compound(props));
    // The sign's state is judged against the pinned registry before it is
    // written into the library copy: an id the pin lacks would load as air.
    let registry = delvewright_dsl::blocks::BlockRegistry::v1_21_11();
    let sign_props = std::collections::BTreeMap::from([
        ("rotation".to_string(), "8".to_string()),
        ("waterlogged".to_string(), "false".to_string()),
    ]);
    let judged = registry.validate("minecraft:oak_sign", &sign_props);
    judged.expect("the sign is a block the pin has");
    palette.push(Value::Compound(sign));
    let state = (palette.len() - 1) as i32;
    let Some(Value::List(blocks)) = top.get_mut("blocks") else {
        panic!("blocks")
    };
    let at = |b: &Value| match b {
        Value::Compound(c) => match c.get("pos") {
            Some(Value::List(p)) => p
                .iter()
                .map(|v| match v {
                    Value::Int(i) => *i,
                    _ => -1,
                })
                .collect::<Vec<_>>(),
            _ => vec![],
        },
        _ => vec![],
    };
    let cell = blocks
        .iter_mut()
        .find(|b| at(b) == [8, 1, 8])
        .expect("the template lists (8, 1, 8)");
    let Value::Compound(c) = cell else { panic!() };
    c.insert("state".to_string(), Value::Int(state));
    let line = |t: &str| Value::String(t.to_string());
    let mut front = std::collections::HashMap::new();
    front.insert(
        "messages".to_string(),
        Value::List(vec![line("KEEP"), line("OUT"), line(""), line("")]),
    );
    front.insert("color".to_string(), Value::String("black".into()));
    front.insert("has_glowing_text".to_string(), Value::Byte(0));
    let mut back = front.clone();
    back.insert(
        "messages".to_string(),
        Value::List(vec![line(""), line(""), line(""), line("")]),
    );
    let mut nbt = std::collections::HashMap::new();
    nbt.insert("id".to_string(), Value::String("minecraft:sign".into()));
    nbt.insert("front_text".to_string(), Value::Compound(front));
    nbt.insert("back_text".to_string(), Value::Compound(back));
    nbt.insert("is_waxed".to_string(), Value::Byte(0));
    c.insert("nbt".to_string(), Value::Compound(nbt));
    let bytes = fastnbt::to_bytes(&root).unwrap();
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&bytes).unwrap();
    std::fs::write(&path, enc.finish().unwrap()).unwrap();
    lib
}

/// Criterion 4: a template's lettered sign is written with its compound, and a
/// frame of it draws its glyphs (the renderer's own block-entity statistics).
#[test]
fn a_template_sign_is_written_with_its_text_and_drawn() {
    let dir = tmp("rfw-sign");
    let lib = library_with_a_sign(&dir);
    let out = dir.join("build");
    build(&common::hello_world_dir(), &lib, &out);
    let bes = anvil(&[
        "block-entities",
        out.join("validation/worlds/at-load").to_str().unwrap(),
    ]);
    let sign = bes
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == "minecraft:sign")
        .unwrap_or_else(|| panic!("no sign compound in {bes}"));
    assert_eq!(sign["front_text"]["messages"][0], "KEEP", "{sign}");
    assert_eq!([&sign["x"], &sign["y"], &sign["z"]], [8, 65, 8], "{sign}");
    // A frame looking at the sign from the north, a few blocks off.
    let png = dir.join("frame.png");
    let o = Command::new(BIN)
        .args(["frames", "--campaign"])
        .arg(common::hello_world_dir())
        .arg("--prefabs")
        .arg(&lib)
        .arg("-o")
        .arg(&png)
        .args([
            "--camera=8.5,66.2,6.0,0,10,60",
            "--size",
            "128",
            "--backend",
            "cpu",
        ])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", stderr(&o));
    let m = json(&png.with_extension("manifest.json"));
    let glyphs = m["renderer_stats"]["block_entities"]["glyphs"]
        .as_u64()
        .unwrap();
    assert!(glyphs > 0, "the sign's letters were drawn: {m}");
}

/// **An `interact` frame stands where the player meets the thing** (spec-0112
/// §3 item 3, §7.3): the gallery's `obj/press-the-case` places its `prop`
/// when its quest activates, so its frame stands in a world after that
/// activation — the world holds the prop at the anchor, the frame's `shows`
/// lists that cell, and its `expect` names the prop's block and never a
/// lantern marker the objective does not have.
#[test]
fn a_prop_objectives_interact_frame_stands_in_the_world_that_holds_the_prop() {
    const OBJ: &str = "obj/press-the-case";
    let dir = tmp("rfw-interact-prop");
    let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
    let out = dir.join("build");
    build(&campaign, &prefabs, &out);
    let quests = json(&campaign.join("quests.json"));
    let block = quests["content"]["quests"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|q| q["objectives"].as_array().unwrap())
        .find(|o| o["id"] == OBJ)
        .and_then(|o| o["prop"]["block"].as_str())
        .expect("the gallery's prop objective")
        .to_string();
    // The prop's cell, as the placements ledger records the activation laying
    // it.
    let ledger = json(&out.join("validation/placed.json"));
    let row = ledger["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["object"]["block"] == block.as_str())
        .unwrap_or_else(|| panic!("a ledger row places `{block}`: {ledger}"));
    let cell: Vec<i64> = row["object"]["cell"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();

    let plan = json(&out.join("render-plan.json"));
    let shot = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "interact" && s["objective"] == OBJ)
        .expect("the objective's interact frame");
    let key = shot["world"].as_str().unwrap();
    assert_ne!(
        key, "at-load",
        "the prop is placed when its quest activates: {shot}"
    );

    // The world the frame stands in holds the prop at its cell.
    let w = out.join(format!("validation/worlds/{key}"));
    let (x, y, z) = (
        cell[0].to_string(),
        cell[1].to_string(),
        cell[2].to_string(),
    );
    let got = anvil(&["cells", w.to_str().unwrap(), &x, &y, &z, &x, &y, &z]);
    let id = |s: &str| s.split('[').next().unwrap().to_string();
    assert_eq!(
        got[format!("{x},{y},{z}")].as_str().map(id),
        Some(id(&block)),
        "world `{key}` at {cell:?}: {got}"
    );

    // `shows` lists the prop's cell.
    let record = json(&out.join("validation/review-frames.json"));
    let row = record["shots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == shot["id"])
        .expect("the frame's review row");
    assert!(
        row["shows"].as_array().unwrap().iter().any(|t| {
            t["kind"] == "interact" && t["id"] == OBJ && t["pos"] == serde_json::json!(cell)
        }),
        "{row}"
    );

    // `expect` names the prop's block, and no marker the objective lacks.
    let expect: Vec<&str> = shot["expect"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_str().unwrap())
        .collect();
    assert!(expect.iter().any(|e| e.contains(&block)), "{expect:?}");
    assert!(expect.iter().all(|e| !e.contains("lantern")), "{expect:?}");
}
