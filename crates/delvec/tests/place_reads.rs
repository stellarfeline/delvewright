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
//! the paving course; the far hall's screen holds an oak door in a wall.

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
        p.stderr.contains(
            "threshold shots: 26 over 10 place(s), 18 door(s), 5 open side(s), 3 part(s)"
        ),
        "{}",
        p.stderr
    );
    let plan = read_json(&p.out.join("render-plan.json"));
    assert_eq!(
        plan["threshold_shots"],
        serde_json::json!({"shots": 26, "places": 10, "doors": 18, "open_sides": 5, "parts": 3})
    );
    let shots: Vec<&serde_json::Value> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "threshold")
        .collect();
    assert_eq!(shots.len(), 26);
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
        stderr.contains("threshold shots: 0 over 2 place(s), 0 door(s), 0 open side(s), 0 part(s)"),
        "{stderr}"
    );
}

/// **Criteria 24 and 25 over the built point**: one entry per bound piece;
/// the annex's street face lists its counter opening and its door opening
/// with their heads at one course; both doors are framed; the yard's water
/// stands one course under its walk plane; the undercroft, with no shown
/// face, no open side and no door, writes a zero entry; the line states every
/// count, `D` equal to `N`.
#[test]
fn the_census_states_the_annex_front_the_doors_and_the_court_water() {
    let p = point("census");
    assert!(
        p.stderr.contains(
            "piece census: 10 piece(s), 30 face(s), 19 opening(s), 2 water cell(s); doors framed: 2 of 2"
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
    for node in ["node/annex", "node/far-hall"] {
        let doors = piece(node)["doors"].clone();
        assert_eq!(doors.as_array().unwrap().len(), 1, "{node}: {doors}");
        assert_eq!(doors[0]["framed"], true, "{node}: {doors}");
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
    let door = entry(&before, "node/annex")["doors"][0]["cell"].clone();
    let origin = frame_origin(p, "node/annex");
    let head = [
        (door[0].as_i64().unwrap() - origin[0]) as i32,
        (door[1].as_i64().unwrap() + 2 - origin[1]) as i32,
        (door[2].as_i64().unwrap() - origin[2]) as i32,
    ];
    let (stderr, after) = rebuilt_with(p, "door", "gallery-annex.nbt", |s| {
        s.set_cell(head, PaletteEntry::with_props("minecraft:air", &[]), None);
    });
    assert!(stderr.contains("doors framed: 1 of 2"), "{stderr}");
    assert_eq!(after["census"]["doors_framed"], 1);
    let framing = entry(&after, "node/annex")["doors"][0]["framing"].clone();
    let h = framing
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["role"] == "head")
        .unwrap()
        .clone();
    assert_eq!(h["present"], false, "{framing}");
}

// ---------------------------------------------------------------------------
// spec-0125 — the plot program and the part frames
// ---------------------------------------------------------------------------

/// **spec-0125 criterion 9**: the court is a plot of three parts and one
/// crossing (the far hall's arch; the gantry's ladder is a seam through its
/// sky, not a crossing a body enters by), so it draws three part frames, each
/// eye on the court's side of the arch's sill at eye height and each `expect`
/// naming the part's use.
#[test]
fn the_court_draws_one_part_frame_per_part_from_its_crossing() {
    let p = point("parts");
    let plan = read_json(&p.out.join("render-plan.json"));
    let parts: Vec<&serde_json::Value> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "threshold" && s.get("part").is_some())
        .collect();
    let ids: Vec<&str> = parts.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [
            "threshold/exit/court/exit-arch",
            "threshold/exit/well_house/exit-arch",
            "threshold/exit/bed/exit-arch"
        ]
    );
    for s in &parts {
        assert_eq!(s["crossing"], "edge/exit-arch", "{s}");
        let y = s["camera"]["pos"][1].as_f64().unwrap();
        assert!((y - 65.62).abs() < 1e-6, "{s}");
        assert!(
            s["expect"][0]
                .as_str()
                .unwrap()
                .starts_with("the part reads as its use: "),
            "{s}"
        );
    }
    assert!(
        parts[1]["expect"][0]
            .as_str()
            .unwrap()
            .contains("a small stone well-house"),
        "{}",
        parts[1]
    );
}

/// **spec-0125 criterion 6, the ledger**: the court's three parts with their
/// declared and built cells, the well-house's ridge, clearance and setbacks,
/// the plot's shares, and the index line.
#[test]
fn the_ledger_states_the_courts_three_parts_built_inside_the_tolerance() {
    let p = point("ledger");
    assert!(
        p.stderr.contains(
            "plot program: 1 plot(s), 3 part(s) built 3 of 3; coverage 2–2 %, remainder 13–13 %"
        ),
        "{}",
        p.stderr
    );
    let l = read_json(&p.out.join("validation/plot-program.json"));
    let plots = l["plots"].as_array().unwrap();
    assert_eq!(plots.len(), 1);
    let court = &plots[0];
    assert_eq!(court["node"], "node/exit");
    assert_eq!(court["play_space"], 64);
    assert_eq!(court["remainder"], 8);
    let rows: Vec<(String, i64, i64)> = court["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["id"].as_str().unwrap().to_string(),
                x["declared"].as_i64().unwrap(),
                x["built"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("part/court".to_string(), 52, 52),
            ("part/well-house".to_string(), 1, 1),
            ("part/bed".to_string(), 3, 3)
        ]
    );
    let well = &court["parts"][1];
    assert_eq!(well["ridge"], 3);
    assert_eq!(well["regions"][0]["clearance"], 2);
    assert_eq!(
        well["setbacks"],
        serde_json::json!({"east": 2, "north": 1, "south": 6, "west": 5})
    );
    assert_eq!(well["roofed_share"], 1.0);
    assert_eq!(court["parts"][2]["reached"], false);
}

/// **spec-0125 criterion 6, the perturbation**: one claimed column moved
/// from the court to the bed — in the yard's contract, and in the plan's
/// declaration so the bed stays inside its quarter — moves the court's
/// `built` from 52 to 51 and the bed's from 3 to 4, and nothing else.
#[test]
fn moving_one_claimed_cell_from_the_court_to_the_bed_moves_two_built_values_by_one() {
    let p = point("ledger-move");
    let tmp = tempdir("ledger-move");
    let prefabs = tmp.join("prefabs");
    common::copy_dir_all(&p.prefabs, &prefabs);
    let meta_path = prefabs.join("gallery-yard.json");
    let mut meta = read_json(&meta_path);
    let c = &mut meta["spatial_contract"];
    // The court's southern box (x 1..8, z 6..7) gives up its column x 1, z 6:
    // split into the row z 6 without it and the row z 7 whole.
    let boxes = c["spaces"]["yard"]["boxes"].as_array_mut().unwrap();
    let south = boxes
        .iter()
        .position(|b| b["from"][2] == 6)
        .expect("the court's southern box");
    let b = boxes.remove(south);
    let (f, t) = (b["from"].clone(), b["to"].clone());
    boxes.push(serde_json::json!({"from": [2, f[1], 6], "to": [t[0], t[1], 6]}));
    boxes.push(serde_json::json!({"from": [1, f[1], 7], "to": [t[0], t[1], 7]}));
    c["no_body"]["bed"]["boxes"][0]["to"][2] = serde_json::json!(6);
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta).unwrap()).unwrap();
    let campaign = tmp.join("campaign");
    common::copy_dir_all(&p.campaign, &campaign);
    let gp = campaign.join("layout-graph.json");
    let mut g = read_json(&gp);
    for n in g["content"]["nodes"].as_array_mut().unwrap() {
        if n["id"] == "node/exit" {
            n["holds"][0]["cells"] = serde_json::json!(51);
            n["holds"][2]["cells"] = serde_json::json!(4);
        }
    }
    std::fs::write(&gp, serde_json::to_vec_pretty(&g).unwrap()).unwrap();
    let out = tmp.join("out");
    let run = build(&campaign, &prefabs, &out);
    assert!(run.status.success(), "{}", text(&run));
    let l = read_json(&out.join("validation/plot-program.json"));
    let built: Vec<i64> = l["plots"][0]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["built"].as_i64().unwrap())
        .collect();
    assert_eq!(built, [51, 1, 4]);
}
