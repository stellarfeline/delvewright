//! **A prefab is extracted from a built campaign and seated in another**
//! (spec-0106 §3.9, criteria 14–16 on a fixture CI builds).
//!
//! One fixture: three places in a row on an `open` flat site — `node/west`,
//! `node/mid`, `node/east`, each owning one course of ground under its floor —
//! joined by two `walk` seams through standard doors. Each place is bound to a
//! piece written here from the place's own allocation (`delvec allocation`),
//! so its faces answer its seams exactly: a floor course over its ground, a
//! wall ring with the seams' cells open, sky over it. The keeper stands in the
//! west room; the one quest walks to a station in each place.
//!
//! The real campaigns criteria 14 and 15 name are measured by the tests at
//! the bottom, which run only when the environment hands them a campaign
//! (`#[ignore]`d otherwise); this file's fixture tests are the mechanism every
//! CI run holds.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// The three places, west to east: `(node, min x, station)`.
const PLACES: [(&str, i64, &str); 3] = [
    ("west", 1, "hearth"),
    ("mid", 10, "bench"),
    ("east", 19, "settle"),
];
/// Every box's play-space footprint is `8 x 8`.
const SPAN: i64 = 8;
/// Every place's walk plane.
const FLOOR: i64 = 65;

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-extract-seat-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn write_doc(dir: &Path, name: &str, stage: &str, content: serde_json::Value) {
    let doc = serde_json::json!({
        "campaign_id": "blockout", "content": content,
        "dsl_version": delvewright_dsl::DSL_VERSION, "stage": stage,
    });
    std::fs::write(
        dir.join(name),
        delvewright_dsl::to_canonical_string(&doc).unwrap(),
    )
    .unwrap();
}

fn run(args: &[&str]) -> (Option<i32>, String) {
    let out = Command::new(BIN).args(args).output().unwrap();
    (
        out.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// The campaign's documents: the blockout fixture's cast and words, the three
/// places, and a quest walking a station in each.
fn campaign(dir: &Path) {
    common::copy_dir_all(
        &common::repo_root().join("crates/delvec/tests/fixtures/blockout"),
        dir,
    );
    let nodes: Vec<serde_json::Value> = PLACES
        .iter()
        .map(|(n, _, st)| {
            serde_json::json!({
                "id": format!("node/{n}"), "intent": "a room of the row",
                "stations": [{"anchor": format!("anchor/{st}"), "kind": "point"}]
            })
        })
        .collect();
    write_doc(
        dir,
        "layout-graph.json",
        "layout-graph",
        serde_json::json!({
            "nodes": nodes,
            "edges": [
                {"id": "edge/west-mid", "class": "walk", "a": "node/west", "b": "node/mid"},
                {"id": "edge/mid-east", "class": "walk", "a": "node/mid", "b": "node/east"}
            ],
            "entry": "node/west", "goal": "node/east",
            "critical_path": ["node/west", "node/mid", "node/east"],
            "beats": [
                {"node": "node/west", "objective": "obj/ask", "quest": "quest/walk-the-whole"},
                {"node": "node/mid", "objective": "obj/enter-the-cell", "quest": "quest/walk-the-whole"},
                {"node": "node/east", "objective": "obj/walk-out", "quest": "quest/walk-the-whole"}
            ]
        }),
    );
    let boxes: Vec<serde_json::Value> = PLACES
        .iter()
        .map(|(n, x, _)| {
            serde_json::json!({
                "node": format!("node/{n}"), "min": [x, 1], "extent": [SPAN, SPAN],
                "floor": {"y": FLOOR}, "ceiling": {"open": 4}, "base": {"owned": 1}
            })
        })
        .collect();
    write_doc(
        dir,
        "site-plan.json",
        "site-plan",
        serde_json::json!({
            "region": {"min": [0, 62, 0], "extent": [28, 10, 10]},
            "boxes": boxes,
            "seams": [
                {"edge": "edge/west-mid", "face": "east", "at": 3, "meets": 3,
                 "opening": "door", "form": "a door in the party wall"},
                {"edge": "edge/mid-east", "face": "east", "at": 3, "meets": 3,
                 "opening": "door", "form": "a door in the party wall"}
            ],
            "datums": [{"id": "datum/ground", "y": FLOOR}],
            "fill": {
                "kind": "open", "surface": "minecraft:grass_block", "below": "minecraft:dirt",
                "terrain": {"kind": "flat", "datum": "datum/ground"}
            }
        }),
    );
    common::patch_file(&dir.join("npcs.json"), |v| {
        v["content"]["npcs"][0]["anchor"] = serde_json::json!("anchor/hearth");
    });
    common::patch_file(&dir.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        q["cast"]["npc/keeper"]["at"] = serde_json::json!("anchor/hearth");
        q["objectives"][1]["anchor"] = serde_json::json!("anchor/bench");
        q["objectives"][2]["anchor"] = serde_json::json!("anchor/settle");
        let ask = q["on_objective_complete"]["obj/ask"]
            .as_array_mut()
            .unwrap();
        ask.retain(|e| e["type"] != "open-gate");
    });
    write_doc(
        dir,
        "geometry-brief.json",
        "geometry-brief",
        serde_json::json!({"facts": []}),
    );
}

/// One place's piece, written from its allocation: its frame, a floor course
/// over its ground, a wall ring with every seam's cells open, the voids it
/// does not own, sky over it — and a contract whose faces are the seams.
fn piece(dir: &Path, prefabs: &Path, node: &str, station: &str) {
    let (code, out) = run(&["allocation", s(dir), &format!("node/{node}")]);
    assert_eq!(code, Some(0), "{out}");
    let a: serde_json::Value = serde_json::from_str(&out).unwrap();
    let ext: Vec<i32> = a["extent"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as i32)
        .collect();
    let walk = (a["datum_y"].as_i64().unwrap()) as i32;
    let mut open: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut faces = Vec::new();
    for sm in a["seams"].as_array().unwrap() {
        let c = &sm["cells"];
        let lo: Vec<i32> = (0..3).map(|i| c[0][i].as_i64().unwrap() as i32).collect();
        let hi: Vec<i32> = (0..3).map(|i| c[1][i].as_i64().unwrap() as i32).collect();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    open.insert([x, y, z]);
                }
            }
        }
        faces.push(serde_json::json!({
            "space": "room", "class": sm["answer_with"][0], "dir": sm["face"],
            "opening": {"from": [lo[0], lo[1], lo[2]], "to": [hi[0], hi[1], hi[2]]}
        }));
    }
    let mut void: BTreeSet<[i32; 3]> = BTreeSet::new();
    let wmin: Vec<i64> = (0..3)
        .map(|i| a["world_min"][i].as_i64().unwrap())
        .collect();
    for v in a["voids"].as_array().into_iter().flatten() {
        let c = &v["cells"];
        for x in c[0][0].as_i64().unwrap()..=c[1][0].as_i64().unwrap() {
            for y in c[0][1].as_i64().unwrap()..=c[1][1].as_i64().unwrap() {
                for z in c[0][2].as_i64().unwrap()..=c[1][2].as_i64().unwrap() {
                    void.insert([x as i32, y as i32, z as i32]);
                }
            }
        }
    }
    let _ = wmin;
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..ext[0] {
        for y in 0..ext[1] {
            for z in 0..ext[2] {
                let p = [x, y, z];
                let ring = x == 0 || x == ext[0] - 1 || z == 0 || z == ext[2] - 1;
                let b = if void.contains(&p) {
                    "minecraft:structure_void"
                } else if y < walk - 1 {
                    "minecraft:dirt"
                } else if y == walk - 1 {
                    "minecraft:stone"
                } else if ring && !open.contains(&p) {
                    "minecraft:stone_bricks"
                } else {
                    "minecraft:air"
                };
                cells.push((p, b));
            }
        }
    }
    let size = [ext[0], ext[1], ext[2]];
    std::fs::write(
        prefabs.join(format!("{node}.nbt")),
        common::structure_nbt(size, &cells),
    )
    .unwrap();
    let c = [ext[0] / 2, walk, ext[2] / 2];
    let meta = serde_json::json!({
        "prefab_id": format!("prefab/{node}"),
        "structure": {
            "file": format!("{node}.nbt"), "id": node, "size": size, "data_version": 4671,
            "generator": "crates/delvec/tests/extract_seat.rs"
        },
        "anchors": {
            "seat": {"pos": c, "facing": "south", "role": "entry", "resolves_to": "space:room"},
            station: {"pos": [c[0] - 1, walk, c[2] + 1], "facing": "south", "resolves_to": "space:room"}
        },
        "connectors": [],
        "lighting": {"profile": "lit", "measured_min_light": 8, "measured": "2026-10-15"},
        "license": {
            "source": "original", "spdx": "GPL-3.0-or-later", "note": "Test fixture.",
            "provenance": "Written by crates/delvec/tests/extract_seat.rs."
        },
        "walk_y": walk,
        "shown_faces": ["north", "south", "east", "west", "up"],
        "spatial_contract": {
            "entry": "room",
            "spaces": {"room": {"envelope": "open", "boxes": [
                {"from": [1, walk, 1], "to": [ext[0] - 2, ext[1] - 1, ext[2] - 2]}
            ]}},
            "no_body": {},
            "edges": [{"a": "room", "b": "exterior", "class": "walk"}],
            "faces": faces
        }
    });
    std::fs::write(
        prefabs.join(format!("{node}.json")),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
}

/// The detail plan binding each place to its piece.
fn details(dir: &Path) {
    let rows: Vec<serde_json::Value> = PLACES
        .iter()
        .map(|(n, _, st)| {
            let mut anchors = serde_json::json!({
                format!("anchor/node-{n}"): "seat",
                format!("anchor/{st}"): st,
            });
            if *n == "west" {
                anchors["spawn"] = serde_json::json!("seat");
            }
            serde_json::json!({"place": format!("node/{n}"), "piece": format!("prefab/{n}"), "anchors": anchors})
        })
        .collect();
    write_doc(
        dir,
        "detail-plan.json",
        "detail-plan",
        serde_json::json!({"details": rows}),
    );
}

/// The whole fixture: `root/campaign` and its library `root/prefabs`.
fn fixture(tag: &str) -> (PathBuf, PathBuf) {
    let root = tempdir(tag);
    let dir = root.join("campaign");
    let prefabs = root.join("prefabs");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&prefabs).unwrap();
    campaign(&dir);
    for (n, _, st) in PLACES {
        piece(&dir, &prefabs, n, st);
    }
    details(&dir);
    (dir, prefabs)
}

fn extract(dir: &Path, prefabs: &Path, scope: &str, out: &Path, id: &str) -> (Option<i32>, String) {
    run(&[
        "--prefabs",
        s(prefabs),
        "prefab",
        "extract",
        s(dir),
        "--scope",
        scope,
        "-o",
        s(out),
        "--id",
        id,
    ])
}

fn meta(lib: &Path, id: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(lib.join(format!("{id}.json"))).unwrap()).unwrap()
}

fn build(dir: &Path, prefabs: &Path, out: &Path) -> (Option<i32>, String) {
    run(&["--prefabs", s(prefabs), "build", s(dir), "--out", s(out)])
}

#[test]
fn the_fixture_builds() {
    let (dir, prefabs) = fixture("builds");
    let (code, out) = build(&dir, &prefabs, &dir.parent().unwrap().join("out"));
    assert_eq!(code, Some(0), "{out}");
}

/// Criterion 15's mechanism: a place scope's openings are the plan's seams
/// leaving the set, each with its cells, kind and rise; a seam inside the set
/// is a contract edge; the stations, entry and ground are derived, the
/// residue is zero, and two extractions write the same bytes.
#[test]
fn a_place_scope_declares_its_boundary_seams_as_openings() {
    let (dir, prefabs) = fixture("place");
    let lib = dir.parent().unwrap().join("lib");
    let (code, out) = extract(&dir, &prefabs, "node/mid", &lib, "mid-x");
    assert_eq!(code, Some(0), "{out}");
    let m = meta(&lib, "mid-x");
    let x = &m["extraction"];
    let names: Vec<&str> = x["openings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["mid-east", "west-mid"],
        "in face then cell order: {x:#}"
    );
    for o in x["openings"].as_array().unwrap() {
        assert_eq!(o["kind"], "walk");
        assert_eq!(o["rise"], 0);
        assert_eq!(o["source"], "seam");
        assert_eq!(o["form"], "a door in the party wall");
    }
    assert_eq!(x["ground"], serde_json::json!({"owned": 1}));
    assert_eq!(x["residue"], serde_json::json!([]));
    assert!(
        out.contains("(the scope's boundary is all seams and commons)"),
        "{out}"
    );
    assert_eq!(
        x["stations"],
        serde_json::json!([{"anchor": "anchor/bench", "kind": "point"}])
    );
    assert_eq!(m["anchors"]["seat"]["role"], "entry");
    assert_eq!(m["walk_y"], 2);
    let lock = &m["license"]["extracted"];
    assert_eq!(lock["campaign"], "blockout");
    assert_eq!(lock["scope"], "node/mid");
    assert!(lock["manifest"].as_str().unwrap().starts_with("sha256:"));
    assert!(lock["digest"].as_str().unwrap().starts_with("sha256:"));

    // Two extractions of one build are byte-identical (ADR-0006).
    let again = dir.parent().unwrap().join("lib-again");
    let (code, out) = extract(&dir, &prefabs, "node/mid", &again, "mid-x");
    assert_eq!(code, Some(0), "{out}");
    for f in ["mid-x.json", "mid-x.nbt"] {
        assert_eq!(
            std::fs::read(lib.join(f)).unwrap(),
            std::fs::read(again.join(f)).unwrap(),
            "{f} differs between two extractions"
        );
    }

    // Two places: the seam between them is a contract edge, not an opening.
    let (code, out) = extract(&dir, &prefabs, "node/west,node/mid", &lib, "west-mid");
    assert_eq!(code, Some(0), "{out}");
    let m = meta(&lib, "west-mid");
    let names: Vec<&str> = m["extraction"]["openings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["mid-east"]);
    let edges = m["spatial_contract"]["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["a"] == "west-room" && e["b"] == "mid-room" && e["class"] == "walk"),
        "{edges:?}"
    );
    assert_eq!(m["extraction"]["stations"].as_array().unwrap().len(), 2);
    let entries = m["anchors"]
        .as_object()
        .unwrap()
        .values()
        .filter(|a| a["role"] == "entry")
        .count();
    assert_eq!(entries, 1, "one anchor keeps `role: entry`");
}

/// A fresh one-region campaign for a seated piece: the blockout cast, one
/// node with no station, a site plan with no box.
fn fresh(tag: &str, node: &str, station_name: &str) -> PathBuf {
    let root = tempdir(tag);
    let dir = root.join("campaign");
    std::fs::create_dir_all(&dir).unwrap();
    campaign(&dir);
    write_doc(
        &dir,
        "layout-graph.json",
        "layout-graph",
        serde_json::json!({
            "nodes": [{"id": format!("node/{node}"), "intent": "the seated piece"}],
            "edges": [],
            "entry": format!("node/{node}"), "goal": format!("node/{node}"),
            "critical_path": [format!("node/{node}")],
            "beats": [
                {"node": format!("node/{node}"), "objective": "obj/ask", "quest": "quest/walk-the-whole"},
                {"node": format!("node/{node}"), "objective": "obj/enter-the-cell", "quest": "quest/walk-the-whole"},
                {"node": format!("node/{node}"), "objective": "obj/walk-out", "quest": "quest/walk-the-whole"}
            ]
        }),
    );
    write_doc(
        &dir,
        "site-plan.json",
        "site-plan",
        serde_json::json!({
            "region": {"min": [0, 60, 0], "extent": [20, 14, 20]},
            "boxes": [], "seams": [],
            "datums": [{"id": "datum/ground", "y": 64}],
            "fill": {
                "kind": "open", "surface": "minecraft:grass_block", "below": "minecraft:dirt",
                "terrain": {"kind": "flat", "datum": "datum/ground"}
            }
        }),
    );
    for a in ["npcs.json", "quests.json"] {
        common::patch_file(&dir.join(a), |v| {
            let t = serde_json::to_string(v).unwrap();
            let t = t
                .replace("anchor/hearth", &format!("anchor/{station_name}"))
                .replace("anchor/settle", &format!("anchor/{station_name}"));
            *v = serde_json::from_str(&t).unwrap();
        });
    }
    // The commons a seated opening gives onto runs on into a valley the
    // horizon raises round the region, as Vesperhold's does.
    common::patch_file(&dir.join("world.json"), |v| {
        v["content"]["horizon"] = serde_json::json!({"base": "valley", "rim_height": 16});
        v["content"]["boundary"] =
            serde_json::json!({"margin": 2, "message": "The fog turns you back."});
    });
    let _ = std::fs::remove_file(dir.join("detail-plan.json"));
    dir
}

fn seat(dir: &Path, lib: &Path, prefab: &str, node: &str, extra: &[&str]) -> (Option<i32>, String) {
    let mut args = vec![
        "--prefabs",
        s(lib),
        "prefab",
        "seat",
        s(dir),
        prefab,
        "--node",
        node,
        "--at",
        "4,4",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

/// Criterion 15's seating: with every opening disposed to the commons the
/// fresh campaign validates and builds, its route reaching every station from
/// the seated `spawn`; one opening left undisposed is `DW1022` naming it and
/// both remedies; a station the campaign already holds is `DW0870`.
#[test]
fn a_seated_piece_answers_every_opening_or_is_refused() {
    let (dir, prefabs) = fixture("seat-src");
    let lib = dir.parent().unwrap().join("lib");
    let (code, out) = extract(&dir, &prefabs, "node/mid", &lib, "mid-x");
    assert_eq!(code, Some(0), "{out}");

    // One opening undisposed.
    let fresh_dir = fresh("seat-one", "hall", "bench");
    let (code, out) = seat(
        &fresh_dir,
        &lib,
        "prefab/mid-x",
        "node/hall",
        &["--commons", "west-mid"],
    );
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("DW1022") && out.contains("`mid-east`"),
        "{out}"
    );
    assert!(
        out.contains("--join mid-east=<edge>") && out.contains("--commons mid-east"),
        "{out}"
    );
    assert!(
        !fresh_dir.join("detail-plan.json").exists(),
        "a refused seating writes nothing"
    );

    // Every opening to the commons: it validates and builds.
    let (code, out) = seat(
        &fresh_dir,
        &lib,
        "prefab/mid-x",
        "node/hall",
        &["--commons", "west-mid", "--commons", "mid-east"],
    );
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("2 declared opening(s) examined (0 answered by a seam, 2 by the commons)"),
        "{out}"
    );
    let plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fresh_dir.join("site-plan.json")).unwrap())
            .unwrap();
    let b = &plan["content"]["boxes"][0];
    let size = meta(&lib, "mid-x")["structure"]["size"].clone();
    assert_eq!(
        b["min"],
        serde_json::json!([5, 5]),
        "the cuboid's low corner at `--at`, its ring outside the box"
    );
    assert_eq!(
        b["extent"],
        serde_json::json!([size[0].as_i64().unwrap() - 2, size[2].as_i64().unwrap() - 2]),
        "the box is the cuboid less its ring"
    );
    assert_eq!(b["base"], serde_json::json!({"owned": 1}));
    assert_eq!(
        b["floor"],
        serde_json::json!({"y": 64}),
        "the first commons opening's floor meets the ground outside it (surface 63)"
    );
    let (code, out) = build(&fresh_dir, &lib, &fresh_dir.parent().unwrap().join("out"));
    assert_eq!(code, Some(0), "{out}");

    // A station the campaign already holds.
    let held = fresh("seat-held", "hall", "bench");
    common::patch_file(&held.join("layout-graph.json"), |v| {
        let nodes = v["content"]["nodes"].as_array_mut().unwrap();
        nodes.push(serde_json::json!({
            "id": "node/porch", "intent": "another place",
            "stations": [{"anchor": "anchor/bench", "kind": "point"}]
        }));
    });
    let (code, out) = seat(
        &held,
        &lib,
        "prefab/mid-x",
        "node/hall",
        &["--commons", "west-mid", "--commons", "mid-east"],
    );
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("DW0870") && out.contains("--rename anchor/bench=<new>"),
        "{out}"
    );
}

/// Criterion 16's mechanism: a cuboid cut through a place declares the
/// candidate openings the walk model finds at its faces and a residue that is
/// exactly what the cut severs; the same box named as the place's contract
/// space derives the same bytes.
#[test]
fn a_cut_declares_its_candidates_and_its_residue() {
    let (dir, prefabs) = fixture("cut");
    let lib = dir.parent().unwrap().join("lib");
    // The mid room's west half, door to centre: x 9..13 cuts the room.
    let (code, out) = extract(&dir, &prefabs, "9,63,0..13,68,9", &lib, "mid-cut");
    assert_eq!(code, Some(0), "{out}");
    let m = meta(&lib, "mid-cut");
    let x = &m["extraction"];
    assert_eq!(x["kind"], "cut");
    let ops = x["openings"].as_array().unwrap();
    assert!(!ops.is_empty(), "{x:#}");
    assert!(ops.iter().all(|o| o["source"] == "cut"));
    // The west face meets the west room through the door; the east face runs
    // through the room's open floor.
    let by_face: BTreeMap<&str, usize> = ops.iter().fold(BTreeMap::new(), |mut m, o| {
        *m.entry(o["face"].as_str().unwrap()).or_default() += 1;
        m
    });
    assert!(
        by_face.contains_key("west") && by_face.contains_key("east"),
        "{by_face:?}"
    );
    let residue: Vec<&str> = x["residue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["object"].as_str().unwrap())
        .collect();
    assert!(
        residue.contains(&"openings") && residue.contains(&"lighting"),
        "{residue:?}"
    );
}
