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
/// node with no station, a site plan with no box; the keeper stands at
/// `station_name` and the quest walks to it.
fn fresh(tag: &str, node: &str, station_name: &str) -> PathBuf {
    fresh_at(tag, node, [station_name; 3], [20, 14, 20])
}

/// [`fresh`], the keeper at `stations[0]`, the quest's two walks to
/// `stations[1]` and `stations[2]`, over a region of `extent`.
fn fresh_at(tag: &str, node: &str, stations: [&str; 3], extent: [i64; 3]) -> PathBuf {
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
            "region": {"min": [0, 60, 0], "extent": extent},
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
                .replace("anchor/hearth", &format!("anchor/{}", stations[0]))
                .replace("anchor/bench", &format!("anchor/{}", stations[1]))
                .replace("anchor/settle", &format!("anchor/{}", stations[2]));
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
    seat_at(dir, lib, prefab, node, "4,4", extra)
}

fn seat_at(
    dir: &Path,
    lib: &Path,
    prefab: &str,
    node: &str,
    at: &str,
    extra: &[&str],
) -> (Option<i32>, String) {
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
        at,
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

/// Every place's frame, from `delvec allocation --all`: `(node, lo, hi)`.
fn frames(dir: &Path) -> Vec<(String, [i64; 3], [i64; 3])> {
    let (code, out) = run(&["allocation", s(dir), "--all"]);
    assert_eq!(code, Some(0), "{out}");
    let all: serde_json::Value = serde_json::from_str(&out).unwrap();
    all.as_array()
        .unwrap()
        .iter()
        .map(|a| {
            let lo: Vec<i64> = (0..3)
                .map(|i| a["world_min"][i].as_i64().unwrap())
                .collect();
            let ext: Vec<i64> = (0..3).map(|i| a["extent"][i].as_i64().unwrap()).collect();
            (
                a["place"].as_str().unwrap().to_string(),
                [lo[0], lo[1], lo[2]],
                [lo[0] + ext[0] - 1, lo[1] + ext[1] - 1, lo[2] + ext[2] - 1],
            )
        })
        .collect()
}

/// **The residue a cut owes, computed from the documents alone** (spec-0106
/// criterion 16's second method): every station of a place whose frame the
/// cuboid reaches and whose anchor it leaves outside, and — the derivation's
/// own rows — the entry when the campaign's lies outside, the candidates
/// awaiting a neighbour, and the light at the cut. This fixture declares no
/// gate, volume, pulse or assembly, so those classes are empty here.
fn residue_from_documents(
    dir: &Path,
    prefabs: &Path,
    lo: [i64; 3],
    hi: [i64; 3],
) -> BTreeSet<String> {
    let inside = |c: [i64; 3]| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]);
    let mut out = BTreeSet::new();
    let rows: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("detail-plan.json")).unwrap())
            .unwrap();
    for (node, flo, fhi) in frames(dir) {
        if (0..3).any(|a| fhi[a] < lo[a] || flo[a] > hi[a]) {
            continue;
        }
        let row = rows["content"]["details"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["place"] == node.as_str())
            .unwrap();
        let id = row["piece"].as_str().unwrap().trim_start_matches("prefab/");
        let m = meta(prefabs, id);
        for (owed, key) in row["anchors"].as_object().unwrap() {
            let p = &m["anchors"][key.as_str().unwrap()]["pos"];
            let w = [
                flo[0] + p[0].as_i64().unwrap(),
                flo[1] + p[1].as_i64().unwrap(),
                flo[2] + p[2].as_i64().unwrap(),
            ];
            let station = PLACES
                .iter()
                .any(|(_, _, st)| owed == &format!("anchor/{st}"));
            if station && !inside(w) {
                out.insert(owed.clone());
            }
            if owed == "spawn" && !inside(w) {
                out.insert("entry".to_string());
            }
        }
    }
    out.insert("openings".to_string());
    out.insert("lighting".to_string());
    // No source side lies on this cut's sides, so none is claimed finished.
    out.insert("shown_faces".to_string());
    out
}

/// Criterion 16's mechanism: a cuboid cut through a place declares the
/// candidate openings the walk model finds at its faces and a residue that is
/// exactly what the cut severs; the same box named as the place's contract
/// space derives the same bytes.
#[test]
fn a_cut_declares_its_candidates_and_its_residue() {
    let (dir, prefabs) = fixture("cut");
    let lib = dir.parent().unwrap().join("lib");
    // The mid room's west half, door to its middle, west's party wall in it.
    let (lo, hi) = ([9, 63, 0], [13, 68, 9]);
    let scope = format!(
        "{},{},{}..{},{},{}",
        lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
    );
    let (code, out) = extract(&dir, &prefabs, &scope, &lib, "mid-cut");
    assert_eq!(code, Some(0), "{out}");
    let m = meta(&lib, "mid-cut");
    let x = &m["extraction"];
    assert_eq!(x["kind"], "cut");
    let ops = x["openings"].as_array().unwrap();
    assert!(ops.iter().all(|o| o["source"] == "cut"));
    // The west face meets the west room through the door; the east face runs
    // through the room's open floor.
    let by_face: BTreeMap<&str, Vec<&serde_json::Value>> =
        ops.iter().fold(BTreeMap::new(), |mut m, o| {
            m.entry(o["face"].as_str().unwrap()).or_default().push(o);
            m
        });
    assert_eq!(by_face.len(), 2, "{by_face:#?}");
    let west = &by_face["west"];
    assert_eq!(west.len(), 1, "one run through the door: {west:#?}");
    assert_eq!(
        (west[0]["kind"].as_str(), west[0]["rise"].as_i64()),
        (Some("walk"), Some(0))
    );
    assert_eq!(west[0]["width"], 1, "the door is one cell wide");
    let east = &by_face["east"];
    assert_eq!(east.len(), 1, "{east:#?}");
    assert_eq!(east[0]["kind"], "walk");
    assert!(
        east[0]["width"].as_u64().unwrap() > 1,
        "the open floor runs the room's depth"
    );
    // Every candidate is a face of the contract, so a seating must answer it.
    assert_eq!(
        m["spatial_contract"]["faces"].as_array().unwrap().len(),
        ops.len()
    );

    // The residue is exactly what the cut severs, computed a second way.
    let got: BTreeSet<String> = x["residue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["object"].as_str().unwrap().to_string())
        .collect();
    let want = residue_from_documents(&dir, &prefabs, lo, hi);
    assert_eq!(got, want);
    assert!(
        want.contains("anchor/hearth"),
        "the cut parts the west room's station: {want:?}"
    );

    // Seated with every candidate disposed to the commons, every residue row
    // is answered by the seated campaign's own checks: it validates and builds.
    let names: Vec<String> = ops
        .iter()
        .map(|o| o["name"].as_str().unwrap().to_string())
        .collect();
    let seated = fresh("cut-seat", "hall", "bench");
    let (code, out) = seat(
        &seated,
        &lib,
        "prefab/mid-cut",
        "node/hall",
        &["--commons", &names[0]],
    );
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("DW1022") && out.contains(&format!("`{}`", names[1])),
        "{out}"
    );
    let mut all: Vec<&str> = Vec::new();
    for n in &names {
        all.extend(["--commons", n.as_str()]);
    }
    // The cut's sides are a cross-section (the residue's `shown_faces` row):
    // standing it on open ground, the author answers `DW0885` by declaring
    // them finished as they stand — the piece's claim about itself, made by
    // the one who chose to show it.
    common::patch_file(&lib.join("mid-cut.json"), |v| {
        v["shown_faces"] = serde_json::json!(["north", "south", "east", "west", "up"]);
    });
    let (code, out) = seat(&seated, &lib, "prefab/mid-cut", "node/hall", &all);
    assert_eq!(code, Some(0), "{out}");
    let (code, out) = build(&seated, &lib, &seated.parent().unwrap().join("out"));
    assert_eq!(code, Some(0), "{out}");

    // The same box named as the mid piece's contract space — the space and
    // its shell — cuts the same bytes as those coordinates.
    let (code, out) = extract(&dir, &prefabs, "node/mid/room", &lib, "mid-room");
    assert_eq!(code, Some(0), "{out}");
    let room = meta(&lib, "mid-room");
    let lo2 = &room["license"]["provenance"];
    assert!(lo2.as_str().unwrap().contains("structure scope"), "{lo2}");
    let (code, out) = run(&["allocation", s(&dir), "node/mid"]);
    assert_eq!(code, Some(0), "{out}");
    let a: serde_json::Value = serde_json::from_str(&out).unwrap();
    let flo: Vec<i64> = (0..3)
        .map(|i| a["world_min"][i].as_i64().unwrap())
        .collect();
    let ext: Vec<i64> = (0..3).map(|i| a["extent"][i].as_i64().unwrap()).collect();
    let walk = a["datum_y"].as_i64().unwrap();
    // The room's cells grown by one course every side, inside the frame.
    let rlo = [flo[0], flo[1] + walk - 1, flo[2]];
    let rhi = [
        flo[0] + ext[0] - 1,
        flo[1] + ext[1] - 1,
        flo[2] + ext[2] - 1,
    ];
    let scope = format!(
        "{},{},{}..{},{},{}",
        rlo[0], rlo[1], rlo[2], rhi[0], rhi[1], rhi[2]
    );
    let (code, out) = extract(&dir, &prefabs, &scope, &lib, "mid-room-cut");
    assert_eq!(code, Some(0), "{out}");
    let cut = meta(&lib, "mid-room-cut");
    assert_eq!(
        std::fs::read(lib.join("mid-room.nbt")).unwrap(),
        std::fs::read(lib.join("mid-room-cut.nbt")).unwrap(),
        "the claim's box and the same cuboid cut the same bytes"
    );
    // The documents differ in the words that say which scope it was, and
    // nowhere else.
    let strip = |mut v: serde_json::Value| {
        v["prefab_id"] = serde_json::Value::Null;
        v["structure"]["file"] = serde_json::Value::Null;
        v["structure"]["id"] = serde_json::Value::Null;
        v["license"]["provenance"] = serde_json::Value::Null;
        v["license"]["extracted"]["scope"] = serde_json::Value::Null;
        v["license"]["extracted"]["digest"] = serde_json::Value::Null;
        v["extraction"]["kind"] = serde_json::Value::Null;
        v
    };
    assert_eq!(strip(room), strip(cut));
}

/// **Joining a seated piece's openings to neighbours** (spec-0106 §3.9): the
/// author names the connection each opening joins (`--join`), and the seam is
/// written whole — its face as the edge's `a` box sees it, its offsets on both
/// boxes from the opening's cells, its size from the opening — so the pieces
/// either side are detailed against it and the whole builds.
#[test]
fn a_seated_piece_joins_its_openings_to_neighbours() {
    let (dir, prefabs) = fixture("join-src");
    let lib = dir.parent().unwrap().join("lib");
    let (code, out) = extract(&dir, &prefabs, "node/mid", &lib, "mid-x");
    assert_eq!(code, Some(0), "{out}");
    let size: Vec<i64> = (0..3)
        .map(|i| {
            meta(&lib, "mid-x")["structure"]["size"][i]
                .as_i64()
                .unwrap()
        })
        .collect();

    // left | hall (seated) | right, the hall the `a` of both connections.
    let root = tempdir("join");
    let seated = root.join("campaign");
    std::fs::create_dir_all(&seated).unwrap();
    campaign(&seated);
    let at = [10i64, 0];
    let (left_hi, right_lo) = (at[0] - 1, at[0] + size[0]);
    write_doc(
        &seated,
        "layout-graph.json",
        "layout-graph",
        serde_json::json!({
            "nodes": [
                {"id": "node/left", "intent": "a room", "stations": [{"anchor": "anchor/hearth", "kind": "point"}]},
                {"id": "node/hall", "intent": "the seated piece"},
                {"id": "node/right", "intent": "a room", "stations": [{"anchor": "anchor/settle", "kind": "point"}]}
            ],
            "edges": [
                {"id": "edge/hall-left", "class": "walk", "a": "node/hall", "b": "node/left"},
                {"id": "edge/hall-right", "class": "walk", "a": "node/hall", "b": "node/right"}
            ],
            "entry": "node/left", "goal": "node/right",
            "critical_path": ["node/left", "node/hall", "node/right"],
            "beats": [
                {"node": "node/left", "objective": "obj/ask", "quest": "quest/walk-the-whole"},
                {"node": "node/hall", "objective": "obj/enter-the-cell", "quest": "quest/walk-the-whole"},
                {"node": "node/right", "objective": "obj/walk-out", "quest": "quest/walk-the-whole"}
            ]
        }),
    );
    common::patch_file(&seated.join("site-plan.json"), |v| {
        let c = &mut v["content"];
        c["seams"] = serde_json::json!([]);
        c["boxes"] = serde_json::json!([
            {"node": "node/left", "min": [left_hi - SPAN + 1, 1], "extent": [SPAN, SPAN],
             "floor": {"y": FLOOR}, "ceiling": {"open": 4}, "base": {"owned": 1}},
            {"node": "node/right", "min": [right_lo, 1], "extent": [SPAN, SPAN],
             "floor": {"y": FLOOR}, "ceiling": {"open": 4}, "base": {"owned": 1}}
        ]);
    });
    let at_s = format!("{},{}", at[0], at[1]);
    let (code, out) = seat_at(
        &seated,
        &lib,
        "prefab/mid-x",
        "node/hall",
        &at_s,
        &[
            "--floor",
            &FLOOR.to_string(),
            "--join",
            "west-mid=edge/hall-left",
            "--join",
            "mid-east=edge/hall-right",
        ],
    );
    assert_eq!(code, Some(0), "{out}");
    assert!(
        out.contains("2 declared opening(s) examined (2 answered by a seam, 0 by the commons)"),
        "{out}"
    );
    let plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(seated.join("site-plan.json")).unwrap())
            .unwrap();
    let seams = plan["content"]["seams"].as_array().unwrap();
    assert_eq!(seams.len(), 2);
    for sm in seams {
        assert_eq!(
            sm["form"], "a door in the party wall",
            "the form is the opening's"
        );
        assert!(sm.get("at").is_some() && sm.get("meets").is_some(), "{sm}");
    }
    // The neighbours, detailed against the seams the seating wrote.
    common::copy_dir_all(&lib, &prefabs);
    piece(&seated, &prefabs, "left", "hearth");
    piece(&seated, &prefabs, "right", "settle");
    common::patch_file(&seated.join("detail-plan.json"), |v| {
        let rows = v["content"]["details"].as_array_mut().unwrap();
        rows.push(
            serde_json::json!({"place": "node/left", "piece": "prefab/left",
            "anchors": {"anchor/node-left": "seat", "spawn": "seat", "anchor/hearth": "hearth"}}),
        );
        rows.push(
            serde_json::json!({"place": "node/right", "piece": "prefab/right",
            "anchors": {"anchor/node-right": "seat", "anchor/settle": "settle"}}),
        );
    });
    let (code, out) = run(&["fmt", s(&seated)]);
    assert_eq!(code, Some(0), "{out}");
    let (code, out) = build(&seated, &prefabs, &root.join("out"));
    assert_eq!(code, Some(0), "{out}");
}

/// `DW1023`: a declared object the derivation can neither carry nor drop by
/// a rule is refused, naming it and the field it could not fill, and nothing
/// is written — here a cut through the air over the rooms, which has no walk
/// plane to seat it by.
#[test]
fn what_the_derivation_cannot_place_is_dw1023() {
    let (dir, prefabs) = fixture("dw1023");
    let lib = dir.parent().unwrap().join("lib");
    let (code, out) = extract(&dir, &prefabs, "1,69,1..5,70,5", &lib, "air");
    assert_eq!(code, Some(3), "{out}");
    assert!(out.contains("DW1023") && out.contains("`walk_y`"), "{out}");
    assert!(
        !lib.join("air.json").exists(),
        "a refused extraction writes nothing"
    );
}

// ---------------------------------------------------------------------------
// The real campaigns criteria 14 and 16 name. Run when the environment hands
// them over:
//   DW_VESPERHOLD_REGION=<campaigns/vesperhold of campaign/vesperhold-region>
//   DW_VESPERHOLD_PREFABS=<that branch's prefabs/>
//   cargo test -p delvec --test extract_seat -- --ignored --nocapture
// ---------------------------------------------------------------------------

/// The campaign and library the environment names, or the reason the
/// measurement cannot run.
fn handed(campaign: &str, prefabs: &str) -> (PathBuf, PathBuf) {
    let get = |k: &str| {
        PathBuf::from(std::env::var(k).unwrap_or_else(|_| {
            panic!("{k} is unset: this measurement runs over a real campaign the environment names")
        }))
    };
    (get(campaign), get(prefabs))
}

/// A copy of `src` with `node`'s box, stations and row taken out — the
/// fresh campaign a seating fills.
fn without_place(src: &Path, dst: &Path, node: &str) {
    let _ = std::fs::remove_dir_all(dst);
    common::copy_dir_all(src, dst);
    common::patch_file(&dst.join("site-plan.json"), |v| {
        v["content"]["boxes"]
            .as_array_mut()
            .unwrap()
            .retain(|b| b["node"] != node);
    });
    common::patch_file(&dst.join("layout-graph.json"), |v| {
        for n in v["content"]["nodes"].as_array_mut().unwrap() {
            if n["id"] == node {
                n["stations"] = serde_json::json!([]);
            }
        }
    });
    common::patch_file(&dst.join("detail-plan.json"), |v| {
        v["content"]["details"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["place"] != node);
    });
    let (code, out) = run(&["fmt", s(dst)]);
    assert_eq!(code, Some(0), "{out}");
}

/// The relight binding line a build printed.
fn relight_line(out: &str) -> String {
    out.lines()
        .find(|l| l.starts_with("relight binding:"))
        .unwrap_or_else(|| panic!("no relight line in {out}"))
        .to_string()
}

/// **Criterion 14**: Vesperhold's one region, extracted and seated again,
/// builds the same output trees.
#[test]
#[ignore = "needs DW_VESPERHOLD_REGION and DW_VESPERHOLD_PREFABS"]
fn criterion_14_vesperhold_extracted_and_seated() {
    let (src, prefabs) = handed("DW_VESPERHOLD_REGION", "DW_VESPERHOLD_PREFABS");
    let root = tempdir("c14");
    let (lib, again) = (root.join("lib"), root.join("lib-again"));
    let (code, out) = extract(&src, &prefabs, "node/vesperhold", &lib, "vesperhold");
    assert_eq!(code, Some(0), "{out}");
    println!(
        "{}",
        out.lines().find(|l| l.starts_with("extracted")).unwrap()
    );
    let (code, out) = extract(&src, &prefabs, "node/vesperhold", &again, "vesperhold");
    assert_eq!(code, Some(0), "{out}");
    let files: BTreeSet<String> = std::fs::read_dir(&lib)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    for f in &files {
        assert_eq!(
            std::fs::read(lib.join(f)).unwrap(),
            std::fs::read(again.join(f)).unwrap(),
            "{f} differs between two extractions"
        );
    }
    println!("two extractions: {} file(s), byte-identical", files.len());

    let m = meta(&lib, "vesperhold");
    let anchors = m["anchors"].as_object().unwrap();
    assert_eq!(anchors.len(), 144);
    assert_eq!(
        anchors
            .values()
            .filter(|a| a.get("region").is_some())
            .count(),
        7
    );
    let x = &m["extraction"];
    assert_eq!(x["ground"], serde_json::json!({"owned": 7}));
    assert_eq!(
        x["relight"],
        serde_json::json!({"fixture": "torch", "min_light": 3})
    );
    assert_eq!(x["openings"], serde_json::json!([]), "zero openings");
    assert_eq!(x["residue"], serde_json::json!([]), "zero residue");
    assert_eq!(x["stations"].as_array().unwrap().len(), 114);
    let lock = &m["license"]["extracted"];
    assert_eq!(lock["campaign"], "vesperhold");
    assert_eq!(lock["scope"], "node/vesperhold");
    for k in ["documents", "manifest", "digest"] {
        assert!(
            lock[k].as_str().unwrap().starts_with("sha256:"),
            "{k}: {lock}"
        );
    }
    assert!(
        lock["engine"]
            .as_str()
            .unwrap()
            .contains(delvewright_dsl::DSL_VERSION)
    );
    assert!(
        m["license"]["provenance"]
            .as_str()
            .unwrap()
            .contains("holds: prefab/vesperhold (GPL-3.0-or-later, original;")
    );
    let mut same = 0;
    for f in files.iter().filter(|f| f.ends_with(".nbt")) {
        assert_eq!(
            std::fs::read(lib.join(f)).unwrap(),
            std::fs::read(prefabs.join(f)).unwrap(),
            "{f}: the extracted tile is not the source's"
        );
        same += 1;
    }
    println!("{same} extracted tile(s) byte-identical to the source tiles");

    // Seated again as the one region of a fresh campaign.
    let fresh_dir = root.join("vesperhold");
    without_place(&src, &fresh_dir, "node/vesperhold");
    let seated_lib = root.join("seated-lib");
    common::copy_dir_all(&prefabs, &seated_lib);
    common::copy_dir_all(&lib, &seated_lib);
    let (code, out) = seat_at(
        &fresh_dir,
        &seated_lib,
        "prefab/vesperhold",
        "node/vesperhold",
        "0,0",
        &["--floor", "64"],
    );
    assert_eq!(code, Some(0), "{out}");
    for f in ["site-plan.json", "layout-graph.json", "detail-plan.json"] {
        assert_eq!(
            std::fs::read(src.join(f)).unwrap(),
            std::fs::read(fresh_dir.join(f)).unwrap(),
            "{f}: the seating did not write §6's document"
        );
    }
    let (a, b) = (root.join("out-src"), root.join("out-seat"));
    let (code, out_a) = build(&src, &prefabs, &a);
    assert_eq!(code, Some(0), "{out_a}");
    let (code, out_b) = build(&fresh_dir, &seated_lib, &b);
    assert_eq!(code, Some(0), "{out_b}");
    println!(
        "source: {}\nseated: {}",
        relight_line(&out_a),
        relight_line(&out_b)
    );
    assert_eq!(relight_line(&out_a), relight_line(&out_b));
    let cmp = Command::new("python3")
        .arg(common::repo_root().join("tools/ci/compare-build-trees.py"))
        .args([s(&a), s(&b), "--report-only", "validation"])
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&cmp.stdout).into_owned() + &String::from_utf8_lossy(&cmp.stderr);
    println!("{text}");
    assert_eq!(cmp.status.code(), Some(0), "{text}");
}

/// **Criterion 16**: one tower of Vesperhold, cut out of its region by
/// coordinates — the Sally Tower, `design/generator/castle/pockets.py`'s
/// box 116..120 x 167..174 on the valley floor (feet 8) with its walls, its
/// floor course and the door through its south face, at the piece's own
/// origin `[0, 56, 0]`.
#[test]
#[ignore = "needs DW_VESPERHOLD_REGION and DW_VESPERHOLD_PREFABS"]
fn criterion_16_a_tower_cut_out_of_vesperhold() {
    let (src, prefabs) = handed("DW_VESPERHOLD_REGION", "DW_VESPERHOLD_PREFABS");
    let root = tempdir("c16");
    let lib = root.join("lib");
    let (lo, hi) = ([115, 63, 166], [121, 73, 176]);
    let scope = format!(
        "{},{},{}..{},{},{}",
        lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
    );
    let (code, out) = extract(&src, &prefabs, &scope, &lib, "sally-tower");
    assert_eq!(code, Some(0), "{out}");
    println!(
        "{}",
        out.lines().find(|l| l.starts_with("extracted")).unwrap()
    );
    let m = meta(&lib, "sally-tower");
    let x = &m["extraction"];
    let ops = x["openings"].as_array().unwrap();
    for o in ops {
        println!(
            "opening {}: {} {} rise {} {}x{} at {}",
            o["name"], o["face"], o["kind"], o["rise"], o["width"], o["height"], o["cells"]
        );
    }
    assert!(!ops.is_empty(), "the door in the south face meets the cut");

    // The residue, a second way: from the documents, every station of the
    // place the cut parts and every gate whose region straddles the cut
    // (Vesperhold declares no site-plan volume; its assemblies and pulses are
    // judged by the extraction's own rule and must be absent here).
    let inside = |c: [i64; 3]| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]);
    let mut want: BTreeSet<String> = BTreeSet::new();
    let piece = meta(&prefabs, "vesperhold");
    let graph: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("layout-graph.json")).unwrap())
            .unwrap();
    let rows: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("detail-plan.json")).unwrap())
            .unwrap();
    let row = &rows["content"]["details"][0]["anchors"];
    let origin = [0, 56, 0];
    let at = |p: &serde_json::Value| -> [i64; 3] {
        [
            origin[0] + p[0].as_i64().unwrap(),
            origin[1] + p[1].as_i64().unwrap(),
            origin[2] + p[2].as_i64().unwrap(),
        ]
    };
    for st in graph["content"]["nodes"][0]["stations"].as_array().unwrap() {
        let name = st["anchor"].as_str().unwrap();
        let a = &piece["anchors"][row[name].as_str().unwrap()];
        let p = a.get("pos").unwrap_or(&a["region"]["from"]);
        if !inside(at(p)) {
            want.insert(name.to_string());
        }
    }
    for (k, a) in piece["anchors"].as_object().unwrap() {
        if let Some(r) = a.get("region")
            && inside(at(&r["from"])) != inside(at(&r["to"]))
        {
            want.insert(k.clone());
        }
    }
    for d in ["entry", "openings", "lighting", "shown_faces"] {
        want.insert(d.to_string());
    }
    let got: BTreeSet<String> = x["residue"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["object"].as_str().unwrap().to_string())
        .collect();
    println!(
        "residue: {} row(s), the documents' computation {} row(s)",
        got.len(),
        want.len()
    );
    assert_eq!(got, want);

    // Seated in a fresh campaign with every residue row answered: the
    // candidates to the commons, the cross-sections declared shown.
    let names: Vec<String> = ops
        .iter()
        .map(|o| o["name"].as_str().unwrap().to_string())
        .collect();
    let seated = fresh_at(
        "c16-seat",
        "tower",
        ["sally-tower", "sally-tally", "sally-chest"],
        [24, 16, 24],
    );
    // The keeper stands at the tower's own mark, which the source named in
    // no station: the fresh campaign declares it, and the seating binds it to
    // the piece's anchor of the same name.
    common::patch_file(&seated.join("layout-graph.json"), |v| {
        v["content"]["nodes"][0]["stations"] =
            serde_json::json!([{"anchor": "anchor/sally-tower", "kind": "point"}]);
    });
    // A tower room is small: a reach of radius 3 would complete from the
    // valley floor outside (`DW0881`), so the walks reach their cell.
    common::patch_file(&seated.join("quests.json"), |v| {
        for o in v["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap()
        {
            if o.get("radius").is_some() {
                o["radius"] = serde_json::json!(1);
            }
        }
    });
    if names.len() > 1 {
        let (code, out) = seat_at(
            &seated,
            &lib,
            "prefab/sally-tower",
            "node/tower",
            "6,6",
            &["--commons", &names[0]],
        );
        assert_eq!(code, Some(1), "{out}");
        assert!(out.contains("DW1022"), "{out}");
    } else {
        let (code, out) = seat_at(
            &seated,
            &lib,
            "prefab/sally-tower",
            "node/tower",
            "6,6",
            &[],
        );
        assert_eq!(code, Some(1), "{out}");
        assert!(
            out.contains("DW1022") && out.contains(&format!("`{}`", names[0])),
            "{out}"
        );
    }
    common::patch_file(&lib.join("sally-tower.json"), |v| {
        v["shown_faces"] = serde_json::json!(["north", "south", "east", "west", "up"]);
    });
    let mut all: Vec<&str> = Vec::new();
    for n in &names {
        all.extend(["--commons", n.as_str()]);
    }
    let (code, out) = seat_at(
        &seated,
        &lib,
        "prefab/sally-tower",
        "node/tower",
        "6,6",
        &all,
    );
    assert_eq!(code, Some(0), "{out}");
    let (code, out) = build(&seated, &lib, &root.join("out"));
    assert_eq!(code, Some(0), "{out}");
    println!("{}", relight_line(&out));
}

/// The Stranding's creature: the body's inside places and the places on its
/// back, the scope criterion 15 names.
const CREATURE: [&str; 14] = [
    "node/mouth",
    "node/throat",
    "node/rib-cathedral",
    "node/stomach",
    "node/heart-chamber",
    "node/breach",
    "node/spine-stair",
    "node/spine-top",
    "node/crown",
    "node/brow",
    "node/back-upper",
    "node/back-middle",
    "node/back-lower",
    "node/tail-flank",
];

/// **Criterion 15**: The Stranding's creature, extracted as one place scope
/// and seated in a fresh one-region `open` campaign. Run with
/// `DW_STRANDING=<a copy of campaigns/the-stranding stamped to this engine>`
/// and `DW_STRANDING_PREFABS=<its library>`.
#[test]
#[ignore = "needs DW_STRANDING and DW_STRANDING_PREFABS"]
fn criterion_15_the_creature_extracted_and_seated() {
    let (src, prefabs) = handed("DW_STRANDING", "DW_STRANDING_PREFABS");
    let root = tempdir("c15");
    let lib = root.join("lib");
    let scope = CREATURE.join(",");
    let (code, out) = extract(&src, &prefabs, &scope, &lib, "the-fathomer");
    assert_ne!(
        code,
        Some(3),
        "DW1023 is a derivation finding, recorded, never a pass: {out}"
    );
    assert_eq!(code, Some(0), "{out}");
    println!(
        "{}",
        out.lines().find(|l| l.starts_with("extracted")).unwrap()
    );
    for l in out
        .lines()
        .filter(|l| l.trim_start().starts_with("residue:"))
    {
        println!("{l}");
    }
    let m = meta(&lib, "the-fathomer");
    let x = &m["extraction"];

    // The openings, a second way: every seam `delvec allocation --all` hands a
    // place of the set whose other place lies outside it, at the cells it
    // hands, with its class and rise.
    let (code, out) = run(&["allocation", s(&src), "--all"]);
    assert_eq!(code, Some(0), "{out}");
    let all: serde_json::Value = serde_json::from_str(&out).unwrap();
    let in_set = |p: &str| CREATURE.contains(&p);
    let mut lo = [i64::MAX; 3];
    for a in all.as_array().unwrap() {
        if in_set(a["place"].as_str().unwrap()) {
            for (i, v) in lo.iter_mut().enumerate() {
                *v = (*v).min(a["world_min"][i].as_i64().unwrap());
            }
        }
    }
    let mut want: BTreeSet<(String, String, i64, [i64; 6])> = BTreeSet::new();
    let mut internal: BTreeSet<(String, String, String)> = BTreeSet::new();
    for a in all.as_array().unwrap() {
        let place = a["place"].as_str().unwrap();
        if !in_set(place) {
            continue;
        }
        for sm in a["seams"].as_array().unwrap() {
            let other = sm["other"].as_str().unwrap();
            let edge = sm["edge"]
                .as_str()
                .unwrap()
                .trim_start_matches("edge/")
                .to_string();
            if in_set(other) {
                let (p, q) = if place < other {
                    (place, other)
                } else {
                    (other, place)
                };
                internal.insert((edge, p.to_string(), q.to_string()));
                continue;
            }
            let c = |k: usize, i: usize| {
                a["world_min"][i].as_i64().unwrap() + sm["cells"][k][i].as_i64().unwrap() - lo[i]
            };
            want.insert((
                edge,
                sm["class"].as_str().unwrap().to_string(),
                sm["rise"].as_i64().unwrap(),
                [c(0, 0), c(0, 1), c(0, 2), c(1, 0), c(1, 1), c(1, 2)],
            ));
        }
    }
    let got: BTreeSet<(String, String, i64, [i64; 6])> = x["openings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            let c = |k: usize, i: usize| o["cells"][k][i].as_i64().unwrap();
            (
                o["name"].as_str().unwrap().to_string(),
                o["kind"].as_str().unwrap().to_string(),
                o["rise"].as_i64().unwrap(),
                [c(0, 0), c(0, 1), c(0, 2), c(1, 0), c(1, 1), c(1, 2)],
            )
        })
        .collect();
    println!(
        "openings: {} derived, {} computed from the allocations",
        got.len(),
        want.len()
    );
    assert_eq!(got, want);
    // Every internal seam is a contract edge between the spaces either side.
    let edges = m["spatial_contract"]["edges"].as_array().unwrap();
    for (edge, p, q) in &internal {
        let (sp, sq) = (p.trim_start_matches("node/"), q.trim_start_matches("node/"));
        let joined = edges.iter().any(|e| {
            let (a, b) = (e["a"].as_str().unwrap(), e["b"].as_str().unwrap());
            (a.starts_with(&format!("{sp}-")) && b.starts_with(&format!("{sq}-")))
                || (a.starts_with(&format!("{sq}-")) && b.starts_with(&format!("{sp}-")))
        });
        assert!(
            joined,
            "the internal seam `{edge}` ({p}, {q}) is no contract edge: {edges:?}"
        );
    }
    println!(
        "internal seams: {} examined, each a contract edge",
        internal.len()
    );
    let entities = x["entities"].as_array().unwrap();
    for e in entities {
        println!("entity: {} {} at {}", e["kind"], e["id"], e["anchor"]);
    }
    assert!(
        entities.iter().any(|e| e["id"] == "npc/davey-coyle"),
        "{entities:?}"
    );

    // Seated in a fresh one-region `open` campaign.
    let stations: Vec<String> = x["stations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["anchor"].as_str().unwrap().to_string())
        .collect();
    let gates: Vec<String> = m["anchors"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, a)| a.get("region").is_some())
        .map(|(k, _)| k.clone())
        .collect();
    let local = |a: &str| a.trim_start_matches("anchor/").to_string();
    let seated = fresh_at(
        "c15-seat",
        "body",
        [
            local(&stations[0]).as_str(),
            local(&stations[1]).as_str(),
            local(&stations[2]).as_str(),
        ],
        [80, 60, 180],
    );
    // The body's own gates, named by the campaign that seats it and opened by
    // its first beat; then a walk to every station the piece declares.
    common::patch_file(&seated.join("layout-graph.json"), |v| {
        let g = &mut v["content"];
        g["nodes"][0]["stations"] = serde_json::Value::Array(
            gates
                .iter()
                .map(|k| serde_json::json!({"anchor": k, "kind": "gate"}))
                .collect(),
        );
        let beats = g["beats"].as_array_mut().unwrap();
        for i in 0..stations.len() {
            beats.push(serde_json::json!({
                "node": "node/body", "objective": format!("obj/station-{i}"),
                "quest": "quest/walk-the-whole"
            }));
        }
    });
    common::patch_file(&seated.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        let opens: Vec<serde_json::Value> = gates
            .iter()
            .map(|k| {
                serde_json::json!({"type": "open-gate", "anchor": k,
                "happening": {"text": "The way through the body opens.", "verb": "opens"}})
            })
            .collect();
        q["on_objective_complete"]["obj/ask"]
            .as_array_mut()
            .unwrap()
            .extend(opens);
        let objectives = q["objectives"].as_array_mut().unwrap();
        for o in objectives.iter_mut() {
            if o.get("radius").is_some() {
                o["radius"] = serde_json::json!(1);
            }
        }
        let mut after = "obj/walk-out".to_string();
        for (i, st) in stations.iter().enumerate() {
            let id = format!("obj/station-{i}");
            objectives.push(serde_json::json!({
                "id": id, "type": "reach-anchor", "anchor": st, "radius": 1, "after": [after],
                "happening": {"text": "The party reaches the next place in the body.", "verb": "arrives"}
            }));
            after = id;
        }
    });
    let (code, out) = run(&["fmt", s(&seated)]);
    assert_eq!(code, Some(0), "{out}");
    let names: Vec<String> = x["openings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap().to_string())
        .collect();
    let mut all_commons: Vec<&str> = Vec::new();
    for n in &names {
        all_commons.extend(["--commons", n.as_str()]);
    }
    let undisposed = &names[names.len() - 1];
    let (code, out) = seat_at(
        &seated,
        &lib,
        "prefab/the-fathomer",
        "node/body",
        "10,10",
        &all_commons[..all_commons.len() - 2],
    );
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("DW1022")
            && out.contains(&format!("`{undisposed}`"))
            && out.contains(&format!("--join {undisposed}=<edge>"))
            && out.contains(&format!("--commons {undisposed}")),
        "{out}"
    );
    // A station the campaign already holds is DW0870.
    let held = root.join("held");
    common::copy_dir_all(&seated, &held);
    common::patch_file(&held.join("layout-graph.json"), |v| {
        v["content"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "node/porch", "intent": "another place",
                "stations": [{"anchor": stations[0], "kind": "point"}]
            }));
    });
    let (code, out) = seat_at(
        &held,
        &lib,
        "prefab/the-fathomer",
        "node/body",
        "10,10",
        &all_commons,
    );
    assert_eq!(code, Some(1), "{out}");
    assert!(out.contains("DW0870"), "{out}");
    // Every opening to the commons: it validates and builds, and the route
    // walks to every station from the seated `spawn`.
    let (code, out) = seat_at(
        &seated,
        &lib,
        "prefab/the-fathomer",
        "node/body",
        "10,10",
        &all_commons,
    );
    assert_eq!(code, Some(0), "{out}");
    println!("{}", out.lines().find(|l| l.starts_with("seated")).unwrap());
    let (code, out) = build(&seated, &lib, &root.join("out"));
    assert_eq!(code, Some(0), "{out}");
    for l in out
        .lines()
        .filter(|l| l.starts_with("DW0311 binding") || l.starts_with("relight binding"))
    {
        println!("{l}");
    }
}
