//! **A place is built to read as what it is** (spec-0123 Part B): the
//! threshold shots the review reads blind (§3.4, criterion 6) and the piece
//! census it reads beside them (§3.5, criteria 24 and 25), over the gallery's
//! site-plan point as built.
//!
//! The expected numbers are read off the plan by hand, never off the code
//! under test: the point's site plan cuts nine portal seams in vertical faces
//! (`annex-arch`, `annex-chute`, the three hall doors, `loft-stair`,
//! `loft-drop`, `exit-arch`, `perch-walk`), each a door for both of its places
//! — 18 — and declares five open sides (the yard's east, south and west, the
//! causeway's east and west), none under a contact — 5; ten boxes. The annex's
//! street face is a three-wide counter window over a one-course sill and an
//! iron door, both under one head course; the yard's trough is two sources in
//! the paving course; the far hall's screen holds an oak door in a wall, and
//! the near hall ships the centre door's oak leaf in its seam (spec-0124).

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-place-reads-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn build(campaign: &Path, prefabs: &Path, out: &Path) -> Output {
    let run = Command::new(BIN)
        .arg("--prefabs")
        .arg(prefabs)
        .arg("build")
        .arg(campaign)
        .arg("-o")
        .arg(out)
        .output()
        .expect("run delvec");
    assert!(run.status.success(), "{}", text(&run));
    run
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// The site-plan point, detailed and built once for every assertion below.
struct Point {
    campaign: PathBuf,
    prefabs: PathBuf,
    out: PathBuf,
    stderr: String,
}

/// Built once per test process: every test below reads the same point.
fn point(_name: &str) -> &'static Point {
    static POINT: std::sync::OnceLock<Point> = std::sync::OnceLock::new();
    POINT.get_or_init(|| built_point("point"))
}

fn built_point(name: &str) -> Point {
    let tmp = tempdir(name);
    let (campaign, prefabs) = common::gallery_site_plan_detailed(&tmp);
    let out = tmp.join("out");
    let run = build(&campaign, &prefabs, &out);
    Point {
        campaign,
        prefabs,
        out,
        stderr: String::from_utf8_lossy(&run.stderr).to_string(),
    }
}

/// **Criterion 6**: a `threshold` shot per portal seam on a vertical face and
/// per open side, each eye at eye height over its place's walk
/// plane, the plan stating the count and the build printing it.
#[test]
fn the_site_plan_point_draws_a_threshold_shot_per_door_and_open_side() {
    let p = point("threshold");
    assert!(
        p.stderr
            .contains("threshold shots: 23 over 10 place(s), 18 door(s), 5 open side(s)"),
        "{}",
        p.stderr
    );
    let plan = read_json(&p.out.join("render-plan.json"));
    assert_eq!(
        plan["threshold_shots"],
        serde_json::json!({"shots": 23, "places": 10, "doors": 18, "open_sides": 5})
    );
    let shots: Vec<&serde_json::Value> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "threshold")
        .collect();
    assert_eq!(shots.len(), 23);
    // The yard's open east side: the eye one cell outside the ring at the
    // side's middle, at eye height over the yard's walk plane (y 64).
    let east = shots
        .iter()
        .find(|s| s["id"] == "threshold/exit/east")
        .expect("the yard's east side has a shot");
    let y = east["camera"]["pos"][1].as_f64().unwrap();
    assert!((y - 65.62).abs() < 1e-6, "{east}");
    assert!(
        east["expect"][0]
            .as_str()
            .unwrap()
            .contains("the place reads as its intent: goal-chamber"),
        "{east}"
    );
    // The annex's two doors look into the annex, the near hall's arch into
    // the near hall: one shot per place a portal serves.
    for id in ["threshold/annex/door0", "threshold/near_hall/door0"] {
        assert!(shots.iter().any(|s| s["id"] == id), "{id}");
    }
}

/// **Criterion 6, the zero**: a plan with no portal and no open side — the
/// ferry site, two boxes a link joins — writes `0 over 2`.
#[test]
fn a_plan_with_no_portal_and_no_open_side_writes_zero_threshold_shots() {
    let tmp = tempdir("ferry");
    let dir = tmp.join("ferry-site");
    common::copy_dir_all(&common::compiler_fixtures_dir().join("ferry-site"), &dir);
    let out = tmp.join("out");
    let run = build(&dir, &common::prefabs_dir(), &out);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("threshold shots: 0 over 2 place(s), 0 door(s), 0 open side(s)"),
        "{stderr}"
    );
}

/// **Criteria 24 and 25 over the built point**: one entry per bound piece;
/// the annex's street face lists its counter opening and its door opening
/// with their heads at one course; every door is framed but the smith's
/// bench, an iron door standing free by its `decorative` declaration; the yard's water
/// stands one course under its walk plane; the undercroft, with no shown
/// face, no open side and no door, writes a zero entry; the line states every
/// count.
#[test]
fn the_census_states_the_annex_front_the_doors_and_the_court_water() {
    let p = point("census");
    assert!(
        p.stderr.contains(
            "piece census: 10 piece(s), 30 face(s), 19 opening(s), 2 water cell(s); doors framed: 6 of 7"
        ),
        "{}",
        p.stderr
    );
    let c = read_json(&p.out.join("validation/piece-census.json"));
    let piece = |node: &str| {
        c["pieces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["node"] == node)
            .unwrap_or_else(|| panic!("no entry for {node}"))
            .clone()
    };
    let annex = piece("node/annex");
    let south = annex["faces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["face"] == "south")
        .expect("the annex's street face")
        .clone();
    let openings: Vec<(i64, i64, i64)> = south["openings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            (
                o["sill"].as_i64().unwrap(),
                o["head"].as_i64().unwrap(),
                o["width"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(openings, vec![(1, 1, 3), (0, 1, 1)], "{south}");
    // The annex: the street door, the loft door and the smith's bench; the
    // far hall: the screen wall's two doors and the yard gate.
    for (node, n) in [
        ("node/annex", 3),
        ("node/far-hall", 3),
        ("node/near-hall", 1),
    ] {
        let doors = piece(node)["doors"].clone();
        assert_eq!(doors.as_array().unwrap().len(), n, "{node}: {doors}");
        for d in doors.as_array().unwrap() {
            let bench = d["cell"] == serde_json::json!([11, 64, 20]);
            assert_eq!(d["framed"], !bench, "{node}: {d}");
        }
    }
    let water = piece("node/exit")["water"].clone();
    assert_eq!(
        water,
        serde_json::json!({"cells": 2, "highest": -1, "lowest": -1})
    );
    let under = piece("node/undercroft");
    assert_eq!(under["faces"], serde_json::json!([]));
    assert_eq!(under["doors"], serde_json::json!([]));
    assert_eq!(under["water"]["cells"], 0);
}

/// The point rebuilt with one piece's bytes edited in a copy of its library:
/// the reading the build takes, perturbed by one cell. Returns the build's
/// stderr and its census.
fn rebuilt_with(
    p: &Point,
    name: &str,
    file: &str,
    edit: impl FnOnce(&mut delvec::admit::structure::Structure),
) -> (String, serde_json::Value) {
    let tmp = tempdir(name);
    let prefabs = tmp.join("prefabs");
    common::copy_dir_all(&p.prefabs, &prefabs);
    let path = prefabs.join(file);
    let mut s = delvec::admit::structure::Structure::read(&std::fs::read(&path).unwrap())
        .expect("the piece reads");
    edit(&mut s);
    std::fs::write(&path, s.write()).unwrap();
    let out = tmp.join("out");
    let run = build(&p.campaign, &prefabs, &out);
    (
        String::from_utf8_lossy(&run.stderr).to_string(),
        read_json(&out.join("validation/piece-census.json")),
    )
}

fn entry(c: &serde_json::Value, node: &str) -> serde_json::Value {
    c["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["node"] == node)
        .unwrap_or_else(|| panic!("no entry for {node}"))
        .clone()
}

/// The piece's world origin, as `delvec allocation` hands its frame.
fn frame_origin(p: &Point, place: &str) -> [i64; 3] {
    let out = Command::new(BIN)
        .arg("allocation")
        .arg(&p.campaign)
        .arg(place)
        .output()
        .expect("run delvec");
    assert!(out.status.success(), "{}", text(&out));
    let a: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let m = a["world_min"].as_array().unwrap();
    [
        m[0].as_i64().unwrap(),
        m[1].as_i64().unwrap(),
        m[2].as_i64().unwrap(),
    ]
}

/// **Criterion 25's perturbation**: raising the court's water one course in
/// the yard's bytes moves `highest` by one, and the build still exits 0 — the
/// census refuses nothing.
#[test]
fn raising_the_court_water_one_course_moves_highest_by_one() {
    use delvec::admit::structure::PaletteEntry;
    let p = point("water");
    let (stderr, census) = rebuilt_with(p, "water", "gallery-yard.nbt", |s| {
        // The trough's two sources at the paving course (local y 4, at x 2,
        // z 5..=6), raised one course with a rim of stone round them at that
        // course, so the raised water stands where it is put; the paving is
        // closed under it.
        let stone = || PaletteEntry::with_props("minecraft:stone_bricks", &[]);
        for z in 5..=6 {
            s.set_cell(
                [2, 5, z],
                PaletteEntry::with_props("minecraft:water", &[("level", "0")]),
                None,
            );
            s.set_cell(
                [2, 4, z],
                PaletteEntry::with_props("minecraft:polished_andesite", &[]),
                None,
            );
            s.set_cell([1, 5, z], stone(), None);
            s.set_cell([3, 5, z], stone(), None);
        }
        s.set_cell([2, 5, 4], stone(), None);
        s.set_cell([2, 5, 7], stone(), None);
    });
    assert_eq!(
        entry(&census, "node/exit")["water"],
        serde_json::json!({"cells": 2, "highest": 0, "lowest": 0}),
        "{stderr}"
    );
}

/// **Criterion 24's perturbation**: air over the annex door's upper half, in
/// the annex's bytes, moves the line from `N of N` to `N-1 of N` and marks
/// that head absent; the build still exits 0.
#[test]
fn air_over_the_annex_door_unframes_it_and_moves_the_line() {
    use delvec::admit::structure::PaletteEntry;
    let p = point("door");
    let before = read_json(&p.out.join("validation/piece-census.json"));
    // The street door, not the bench or the loft door the annex also holds.
    let street = entry(&before, "node/annex")["doors"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["cell"] == serde_json::json!([12, 64, 29]))
        .expect("the annex's street door");
    let door = entry(&before, "node/annex")["doors"][street]["cell"].clone();
    let origin = frame_origin(p, "node/annex");
    let head = [
        (door[0].as_i64().unwrap() - origin[0]) as i32,
        (door[1].as_i64().unwrap() + 2 - origin[1]) as i32,
        (door[2].as_i64().unwrap() - origin[2]) as i32,
    ];
    let (stderr, after) = rebuilt_with(p, "door", "gallery-annex.nbt", |s| {
        s.set_cell(head, PaletteEntry::with_props("minecraft:air", &[]), None);
    });
    assert!(stderr.contains("doors framed: 5 of 7"), "{stderr}");
    assert_eq!(after["census"]["doors_framed"], 5);
    let framing = entry(&after, "node/annex")["doors"][street]["framing"].clone();
    let h = framing
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["role"] == "head")
        .unwrap()
        .clone();
    assert_eq!(h["present"], false, "{framing}");
}

/// **spec-0124 criteria 4, 10 and 11, on the point as built**: the centre
/// door is a door-shut seam whose key, anchored on the seam, stands its box
/// over the door's lower cell at load; the key's completion releases the door
/// — the box comes down and no `fill` is written — where the west door, a bar
/// in a seam of the same wall, is cleared by a `fill` to air; the device
/// ledger and the four binding lines state what they read.
#[test]
fn the_door_shut_seam_is_released_where_a_bar_is_cleared() {
    let p = point("device");
    let f = p.out.join("datapack/data/gallery/function");
    let read = |name: &str| std::fs::read_to_string(f.join(name)).unwrap();
    let setup = read("setup_finish.mcfunction");
    assert!(
        setup.lines().any(
            |l| l.contains("summon minecraft:interaction 30.5 63.99 20.5")
                && l.contains("\"dw_i_unlock_the_centre_door\"")
        ),
        "the key's box stands over the door's lower cell at load"
    );
    let key = read("complete_o_unlock_the_centre_door.mcfunction");
    assert!(
        key.lines()
            .any(|l| l == "kill @e[tag=dw_i_unlock_the_centre_door]"),
        "the released door's box comes down: {key}"
    );
    assert!(
        !key.lines().any(|l| l.starts_with("fill ")),
        "a released door is not cleared: {key}"
    );
    let bar = read("complete_q_near_hall.mcfunction");
    assert!(
        bar.lines()
            .any(|l| l.starts_with("fill 23 64 20 23 65 20 minecraft:air replace ")),
        "the west door's bar is cleared: {bar}"
    );
    let ledger = read_json(&p.out.join("validation/devices.json"));
    assert_eq!(ledger["way_blocks"]["door_seams"], 1, "{ledger}");
    assert_eq!(ledger["way_blocks"]["way_blocks"], 1, "{ledger}");
    assert_eq!(ledger["way_blocks"]["refused"], 0, "{ledger}");
    assert_eq!(ledger["keys"]["keys"], 1, "{ledger}");
    assert_eq!(ledger["keys"]["own_release"], 1, "{ledger}");
    assert_eq!(ledger["doors"]["refused"], 0, "{ledger}");
    for line in [
        "device binding (DW1069): 1 way-block(s) among 8 pressable(s); 1 door-shut seam(s) among 8 gated connection(s); 0 refused",
        "key binding (DW1071): 1 key objective(s) among 8; 1 write(s) judged, 0 in sight, 0 shown by a shot, 1 the key's own door released; 0 refused",
        "happening binding (DW1070): 9 opens/seals happening(s) among 18; 0 refused",
        "DW1072 binding: 8 door(s) and gate(s) in the assembled world (7 in bound pieces, the census's N); 2 declared decorative [[11, 64, 20] (no hand opens), [11, 72, 29] (out of reach)]; 6 judged over 7 configuration(s); 0 refused",
    ] {
        assert!(p.stderr.contains(line), "`{line}` in:\n{}", p.stderr);
    }
}

/// Every file under `root`, by its path relative to `root`, with its bytes.
fn tree(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

/// **spec-0124 criterion 2, the bytes.** The centre door's seam is owned by
/// no bound piece, so the derivation stands its door (the stand-in's oak, in
/// `tests/gated_connection.rs`); the world-load seal measures the gate region
/// `anchor/seam-centre-door` as the leaf's two cells, both blocked at load;
/// and the point built a second time into another directory is the same
/// bytes, file for file.
#[test]
fn the_derivations_door_is_sealed_at_load_and_built_twice_the_same() {
    let p = point("door-twice");
    let seal = read_json(&p.out.join("validation/gate-seal.json"));
    let gate = seal["gates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["anchor"] == "anchor/seam-centre-door")
        .expect("the seal measures the door-shut seam");
    assert_eq!(gate["from"], serde_json::json!([30, 64, 20]), "{gate}");
    assert_eq!(gate["to"], serde_json::json!([30, 65, 20]), "{gate}");
    assert_eq!(gate["cells"], 2, "{gate}");
    assert_eq!(gate["blocked_at_world_load"], 2, "{gate}");
    assert_eq!(gate["sealed"], true, "{gate}");
    let again = tempdir("door-twice").join("out");
    build(&p.campaign, &p.prefabs, &again);
    let (a, b) = (tree(&p.out), tree(&again));
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "the same files"
    );
    let differ: Vec<&PathBuf> = a.keys().filter(|k| a[*k] != b[*k]).collect();
    assert!(differ.is_empty(), "files whose bytes differ: {differ:?}");
}

/// **spec-0124 criterion 4, the close.** A `close-gate` on the door-shut seam
/// at the far hall's end lays the leaf shut again — both halves, `open=false`
/// — and stands the key's box over it once more, in the same function.
#[test]
fn a_close_gate_on_a_door_shut_seam_shuts_the_leaf_and_restands_the_box() {
    let (ok, said, out) = common::gallery_site_plan_quests_variant(
        &tempdir("close-door"),
        &[("fx.trig.centre-door-shut.0.narrate", "中门又锁上了。")],
        |v| {
            // A sealed door answers a press (`DW0429`): the close's own answer.
            v["content"]["triggers"].as_array_mut().unwrap().push(serde_json::json!({
            "at": "anchor/seam-centre-door", "audience": "presser",
            "effects": [{"style": "actionbar", "text": "The centre door is locked again.", "type": "narrate"}],
            "id": "trigger/centre-door-shut", "on": {"on": "use"}, "once": false,
            "serves": {"gate": "anchor/seam-centre-door"}
        }));
            v["content"]["quests"][1]["on_complete"]
                .as_array_mut()
                .unwrap()
                .insert(
                    0,
                    serde_json::json!({"type": "close-gate", "anchor": "anchor/seam-centre-door",
                "happening": {"subject": "anchor/seam-centre-door", "verb": "seals",
                    "text": "The centre door is shut and locked behind the party."}}),
                );
        },
    );
    assert!(ok, "the close builds:\n{said}");
    let f = out.join("datapack/data/gallery/function");
    let mut hits = Vec::new();
    for e in std::fs::read_dir(&f).unwrap() {
        let p = e.unwrap().path();
        let body = std::fs::read_to_string(&p).unwrap();
        if p.file_name().unwrap() == "setup_finish.mcfunction" {
            continue;
        }
        if body.lines().any(|l| {
            l.starts_with("summon minecraft:interaction 30.5 63.99 20.5")
                && l.contains("\"dw_i_unlock_the_centre_door\"")
        }) {
            hits.push((p, body));
        }
    }
    assert_eq!(
        hits.len(),
        1,
        "one function restands the box: {:?}",
        hits.iter().map(|h| &h.0).collect::<Vec<_>>()
    );
    let body = &hits[0].1;
    // The door-prop pair, re-bound on a seam's shutter: the leaf is laid as
    // one door, its lower half in the plane's lower cell under its upper half
    // — never the declared lower state written into each cell, which the
    // server drops and `DW1002` refuses on the world the build judges.
    let leaf: Vec<&str> = body
        .lines()
        .filter(|l| l.contains("minecraft:oak_door["))
        .collect();
    assert_eq!(leaf.len(), 2, "the leaf's two halves: {leaf:?}");
    for (cell, half) in [("30 64 20", "lower"), ("30 65 20", "upper")] {
        assert!(
            leaf.iter().any(|l| l.contains(cell)
                && l.contains(&format!("half={half}"))
                && l.contains("open=false")),
            "the {half} half is laid shut at {cell}: {leaf:?}"
        );
    }
}
