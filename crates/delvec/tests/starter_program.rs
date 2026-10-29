//! **A place author starts from a program that already passes** (spec-0122
//! §6.3, §8, criterion 18). `scripts/starter.py`, the starter generator
//! `handout.py` copies beside every handout, is run UNEDITED on the allocation
//! of every place the gallery's site-plan points allocate a box to; its program,
//! standing as that place's program in a scratch copy, passes `delvec detail
//! --check`. A place class the starter cannot answer reds here, naming the
//! place and the verdict — it is fixed in the starter, never excluded.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-starter-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The interpreter the skill names (`DELVEWRIGHT_PYTHON`), else `python3`.
fn python() -> String {
    std::env::var("DELVEWRIGHT_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

fn starter() -> PathBuf {
    common::repo_root().join(".claude/skills/delvewright/skills/new-delve/scripts/starter.py")
}

/// The `site-plan-solid` point, materialised by the gallery's own domain tool
/// and detailed into `prefabs` beside the open point's pieces, as
/// `common::gallery_site_plan_detailed` details the open one.
fn solid_point_detailed(root: &Path, prefabs: &Path) -> PathBuf {
    let campaign = root.join("site-plan-solid");
    let out = Command::new(python())
        .arg(common::repo_root().join("tools/ci/gallery_domain.py"))
        .args(["materialise", "--point", "site-plan-solid"])
        .arg(&campaign)
        .output()
        .expect("run gallery_domain.py");
    assert!(out.status.success(), "{}", text(&out));
    let stems: Vec<String> = std::fs::read_dir(campaign.join("programs"))
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension().and_then(|x| x.to_str()) == Some("json"))
                .then(|| p.file_stem().unwrap().to_string_lossy().to_string())
        })
        .collect();
    common::patch_file(&campaign.join("detail-plan.json"), |v| {
        v["content"]["details"].as_array_mut().unwrap().retain(|r| {
            let place = r["place"].as_str().unwrap();
            !stems.iter().any(|s| place == format!("node/{s}"))
        });
    });
    let all = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        campaign.to_str().unwrap(),
        "--all",
    ]);
    assert!(all.status.success(), "{}", text(&all));
    campaign
}

/// Every place `campaign` allocates a box to, its starter program run unedited
/// and checked. Returns `(passed, places, failures)`.
fn judge_every_place(tmp: &Path, campaign: &Path, prefabs: &Path) -> (usize, usize, Vec<String>) {
    let cs = campaign.to_str().unwrap();
    let all = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "allocation",
        cs,
        "--all",
    ]);
    assert!(all.status.success(), "{}", text(&all));
    let allocations: Vec<serde_json::Value> = serde_json::from_slice(&all.stdout).unwrap();
    assert!(!allocations.is_empty(), "the point allocates no box");
    let mut passed = 0;
    let mut failures = Vec::new();
    for a in &allocations {
        let place = a["place"].as_str().unwrap();
        let stem = place.strip_prefix("node/").unwrap();
        let alloc_file = tmp.join(format!(
            "{}-{stem}.allocation.json",
            campaign.file_name().unwrap().to_string_lossy()
        ));
        std::fs::write(&alloc_file, serde_json::to_vec_pretty(a).unwrap()).unwrap();
        let program = campaign.join("programs").join(format!("{stem}.json"));
        std::fs::create_dir_all(program.parent().unwrap()).unwrap();
        let before = std::fs::read(&program).ok();
        let run = Command::new(python())
            .arg(starter())
            .arg("--allocation")
            .arg(&alloc_file)
            .arg("--out")
            .arg(&program)
            .output()
            .expect("run the starter");
        let verdict = if run.status.success() {
            let check = delvec(&[
                "--prefabs",
                prefabs.to_str().unwrap(),
                "detail",
                "--check",
                cs,
                place,
            ]);
            if !check.status.success() {
                Err(format!(
                    "`detail --check` exited {:?}:\n{}",
                    check.status.code(),
                    text(&check)
                ))
            } else {
                // The commit too, battery included, in a scratch copy of the
                // campaign and the library. The empty place is not the
                // building, so the battery's whole-map verdicts on it (a vista,
                // a crossing, a gate the skeleton does not yet separate) are
                // the author's to answer; what the starter must never hand an
                // author is a refusal of its OWN declarations — `DW0885`, a
                // shown side it leaves empty or an open-air side it does not
                // declare.
                let scratch = tmp.join(format!(
                    "commit-{}-{stem}",
                    campaign.file_name().unwrap().to_string_lossy()
                ));
                let _ = std::fs::remove_dir_all(&scratch);
                common::copy_dir_all(campaign, &scratch.join("campaign"));
                common::copy_dir_all(prefabs, &scratch.join("prefabs"));
                let commit = delvec(&[
                    "--prefabs",
                    scratch.join("prefabs").to_str().unwrap(),
                    "detail",
                    scratch.join("campaign").to_str().unwrap(),
                    place,
                ]);
                let _ = std::fs::remove_dir_all(&scratch);
                let said = text(&commit);
                let own: Vec<&str> = said
                    .lines()
                    .filter(|l| l.starts_with("DW0885 [error]"))
                    .collect();
                if !said.contains("written from") {
                    Err(format!(
                        "`detail --check` passed and the commit wrote nothing (exit {:?}):\n{said}",
                        commit.status.code()
                    ))
                } else if !own.is_empty() {
                    Err(format!(
                        "the commit's battery refuses the starter's own `shown_faces`:\n{}",
                        own.join("\n")
                    ))
                } else {
                    Ok(())
                }
            }
        } else {
            Err(format!(
                "the starter exited {:?}:\n{}",
                run.status.code(),
                text(&run)
            ))
        };
        // The committed program goes back: each place is judged with every
        // other place standing as the gallery commits it.
        match before {
            Some(bytes) => std::fs::write(&program, bytes).unwrap(),
            // A starter that refused wrote nothing to take back.
            None if !program.exists() => {}
            None => std::fs::remove_file(&program).unwrap(),
        }
        match verdict {
            Ok(()) => {
                passed += 1;
                eprintln!(
                    "starter: {place} of `{cs}` passes `detail --check`, and its commit writes it with no `DW0885`"
                );
            }
            Err(why) => failures.push(format!("{place}: {why}")),
        }
    }
    (passed, allocations.len(), failures)
}

#[test]
fn the_starter_passes_the_check_unedited_on_every_place_of_the_gallery_site_plans() {
    let tmp = tempdir("gallery");
    let (open, prefabs) = common::gallery_site_plan_detailed(&tmp);
    let solid = solid_point_detailed(&tmp, &prefabs);
    let mut passed = 0;
    let mut places = 0;
    let mut failures = Vec::new();
    for campaign in [&open, &solid] {
        let (p, n, f) = judge_every_place(&tmp, campaign, &prefabs);
        passed += p;
        places += n;
        failures.extend(f);
    }
    eprintln!(
        "starter binding: {passed} of {places} place(s) pass --check, committed with no DW0885"
    );
    assert!(places > 0, "zero places judged: the binding is vacuous");
    assert!(
        failures.is_empty(),
        "{} of {places} place(s) refused the starter's program:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// The two-box `atmosphere-threshold` plan with `node/far` made a plot of
/// three parts — a walked court, a walked house, a bed seen and never
/// entered — and the starter run unedited on its allocation: the program
/// binds the three, and `detail --check` passes.
fn two_box_plot(root: &Path) -> (PathBuf, PathBuf) {
    let campaign = root.join("campaign");
    let prefabs = root.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    common::copy_dir_all(
        &common::compiler_fixtures_dir().join("atmosphere-threshold"),
        &campaign,
    );
    common::patch_file(&campaign.join("layout-graph.json"), |v| {
        for n in v["content"]["nodes"].as_array_mut().unwrap() {
            if n["id"] == "node/far" {
                n["holds"] = serde_json::json!([
                    {"id": "part/court", "kind": "ground", "use": "the court a body crosses", "cells": 60},
                    {"id": "part/house", "kind": "structure", "use": "a house on the court", "cells": 30, "height": 4},
                    {"id": "part/bed", "kind": "ground", "use": "a bed nobody walks", "cells": 8, "reached": false}
                ]);
            }
        }
    });
    let cs = campaign.to_str().unwrap();
    let a = delvec(&["allocation", cs, "node/far"]);
    assert!(a.status.success(), "{}", text(&a));
    let alloc: serde_json::Value = serde_json::from_slice(&a.stdout).unwrap();
    let ids: Vec<&str> = alloc["parts"]
        .as_array()
        .expect("the allocation hands the plot's parts")
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["part/court", "part/house", "part/bed"]);
    let alloc_file = root.join("far.allocation.json");
    std::fs::write(&alloc_file, &a.stdout).unwrap();
    let program = campaign.join("programs").join("far.json");
    std::fs::create_dir_all(program.parent().unwrap()).unwrap();
    let run = Command::new(python())
        .arg(starter())
        .arg("--allocation")
        .arg(&alloc_file)
        .arg("--out")
        .arg(&program)
        .output()
        .expect("run the starter");
    assert!(run.status.success(), "{}", text(&run));
    (campaign, prefabs)
}

fn check_far(campaign: &Path, prefabs: &Path) -> Output {
    delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        "--check",
        campaign.to_str().unwrap(),
        "node/far",
    ])
}

/// **spec-0125 criterion 5**: over a two-box plan whose plot's piece binds
/// three parts, `delvec detail --check` passes with the parts line — the seen
/// bed no flood reaches is not refused — and raises `DW1074` for each shape
/// of §5.3: a part no region names (*unbuilt*), a `part` on a piece whose
/// node declares no `holds[]` (*undeclared*), a walked part none of whose
/// columns the piece's flood reaches (*not walked*), and a declaration far
/// from the columns the piece claims (*out of size*).
#[test]
fn a_two_box_plots_piece_binds_three_parts_and_each_shape_is_refused() {
    let tmp = tempdir("two-box-plot");
    let (campaign, prefabs) = two_box_plot(&tmp);
    let green = check_far(&campaign, &prefabs);
    let t = text(&green);
    assert!(green.status.success(), "{t}");
    assert!(
        t.contains("node/far: parts: 3 of 3 bound, 2 of 2 walked"),
        "{t}"
    );
    assert!(!t.contains("DW1074"), "{t}");

    let graph = |c: &Path, f: &dyn Fn(&mut serde_json::Value)| {
        common::patch_file(&c.join("layout-graph.json"), |v| {
            for n in v["content"]["nodes"].as_array_mut().unwrap() {
                if n["id"] == "node/far" {
                    f(&mut n["holds"]);
                }
            }
        });
    };
    type Edit = Box<dyn Fn(&Path)>;
    let cases: Vec<(&str, Edit, &str)> = vec![
        (
            "unbuilt",
            Box::new(|c: &Path| {
                common::patch_file(&c.join("programs/far.json"), |v| {
                    let s = v["contract"]["spaces"]["part-house"]
                        .as_object_mut()
                        .unwrap();
                    assert!(s.remove("part").is_some());
                });
            }),
            "unbuilt: `node/far` declares the part `part/house` (a house on the court, 30 cell(s))",
        ),
        (
            "undeclared",
            Box::new(|c: &Path| {
                common::patch_file(&c.join("layout-graph.json"), |v| {
                    for n in v["content"]["nodes"].as_array_mut().unwrap() {
                        if n["id"] == "node/far" {
                            n.as_object_mut().unwrap().remove("holds");
                        }
                    }
                });
            }),
            "names the part `part/court`",
        ),
        (
            "not-walked",
            Box::new(move |c: &Path| {
                graph(c, &|h| {
                    h[2].as_object_mut().unwrap().remove("reached");
                })
            }),
            "not walked: `node/far`'s part `part/bed` (a bed nobody walks)",
        ),
        (
            "out-of-size",
            Box::new(move |c: &Path| graph(c, &|h| h[0]["cells"] = serde_json::json!(120))),
            "out of size: `node/far`'s part `part/court` (the court a body crosses) declares 120 cell(s)",
        ),
    ];
    for (tag, edit, shape) in cases {
        let dir = tmp.join(tag);
        common::copy_dir_all(&campaign, &dir);
        edit(&dir);
        let out = check_far(&dir, &prefabs);
        let t = text(&out);
        assert!(!out.status.success(), "{tag}: {t}");
        assert!(t.contains("DW1074"), "{tag}: {t}");
        assert!(t.contains(shape), "{tag}: {t}");
    }
}

/// A generator drawn into the starter's `Model` (the template of its section
/// *A building drawn as cells*): a court, a house with a door, a fenced bed of
/// poppies nobody walks. `DELVEWRIGHT_SKILL` names the skill root, as a
/// creator's session does.
const FAR_GENERATOR: &str = r#"
import os, sys
sys.path.insert(0, os.path.join(os.environ["DELVEWRIGHT_SKILL"], "scripts"))
from starter import Model, run
from starter import lantern as lantern_state

EDIT = os.environ.get("FAR_EDIT", "")


def build(place):
    m = Model(place)
    m.palette.update({
        "boards": "minecraft:spruce_planks",
        "fence": "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]",
        "soil": "minecraft:grass_block[snowy=false]",
        "bloom": "minecraft:poppy",
        "lantern": "minecraft:lantern[hanging=true,waterlogged=false]",
        "candle": "minecraft:candle[candles=1,lit=false,waterlogged=false]",
    })
    m.hollow(3, 8, 1, 3, 10, 14, "boards")
    m.box(3, 8, 4, 4, 10, 14, "boards")
    if EDIT != "no-door":
        m.clear(5, 5, 1, 2, 10, 10)
    m.set(5, 3, 12, "lantern")
    if EDIT == "lantern-in-air":
        m.set(12, 3, 4, "lantern")
    if EDIT == "candle":
        m.set(4, 1, 13, "candle")
    if EDIT == "no-such-block":
        m.set(4, 1, 13, "minecraft:tuff_bricks_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]")
    if EDIT == "no-such-property":
        m.set(4, 1, 13, "minecraft:muddy_mangrove_roots[waterlogged=false]")
    m.part(3, 8, 10, 14, "house")
    m.hollow(11, 16, 1, 1, 14, 17, "fence")
    for x in range(12, 16):
        for z in (15, 16):
            m.set(x, 0, z, "soil")
            m.set(x, 1, z, "bloom")
    m.part(12, 15, 15, 16, "bed")
    m.part(9, 16, 2, 9, "court")
    if EDIT != "roof-undeclared":
        m.show("up")   # the house's flat roof stands on the top course, in the open air
    if EDIT == "raised-court":
        m.box(9, 16, 1, 1, 2, 9, "boards")
    if EDIT == "lantern-by-way":
        # a lantern standing on the floor beside a seam's opening, inside
        s = place.seams[0]
        lo, hi = [b.value for b in s["lo"]], [b.value for b in s["hi"]]
        (sx0, _, sz0), (sx1, _, sz1) = place.space
        spot = next((x + dx, z + dz) for x in range(lo[0], hi[0]) for z in range(lo[2], hi[2])
                    for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1))
                    if sx0 <= x + dx <= sx1 and sz0 <= z + dz <= sz1
                    and not (lo[0] <= x + dx < hi[0] and lo[2] <= z + dz < hi[2]))
        m.set(spot[0], place.datum.value, spot[1], lantern_state())
    m.finish()
    return m


raise SystemExit(run(build))
"#;

fn run_generator(root: &Path, campaign: &Path, edit: &str) -> Output {
    let generator = root.join("far_generator.py");
    std::fs::write(&generator, FAR_GENERATOR).unwrap();
    let a = delvec(&["allocation", campaign.to_str().unwrap(), "node/far"]);
    assert!(a.status.success(), "{}", text(&a));
    let alloc_file = root.join("far.allocation.json");
    std::fs::write(&alloc_file, &a.stdout).unwrap();
    Command::new(python())
        .arg(&generator)
        .arg("--allocation")
        .arg(&alloc_file)
        .arg("--out")
        .arg(campaign.join("programs").join("far.json"))
        .arg("--delvec")
        .arg(BIN)
        .env(
            "DELVEWRIGHT_SKILL",
            starter().parent().unwrap().parent().unwrap(),
        )
        .env("FAR_EDIT", edit)
        .output()
        .expect("run the generator")
}

/// **A plot with a building in it passes as drawn**: the generator names its
/// parts' columns and nothing else; the starter claims every cell from the
/// engine's survey, and `detail --check` passes with the parts bound. With
/// the door left out, the house's floor is claimed out of walk — what the
/// survey says — and the check refuses the walked part no flood reaches
/// (`DW1074`). A lantern hung in air and a refused candle are refused by the
/// starter before it writes, naming the block.
#[test]
fn a_building_drawn_into_the_starter_s_model_is_claimed_from_the_engine_s_survey() {
    let tmp = tempdir("model");
    let (campaign, prefabs) = two_box_plot(&tmp);

    let run = run_generator(&tmp, &campaign, "");
    assert!(run.status.success(), "{}", text(&run));
    let green = check_far(&campaign, &prefabs);
    let t = text(&green);
    assert!(green.status.success(), "{t}");
    assert!(
        t.contains("node/far: parts: 3 of 3 bound, 2 of 2 walked"),
        "{t}"
    );

    let run = run_generator(&tmp, &campaign, "no-door");
    assert!(run.status.success(), "{}", text(&run));
    assert!(
        text(&run).contains("part/house has no walkable column"),
        "{}",
        text(&run)
    );
    let program: serde_json::Value =
        serde_json::from_slice(&std::fs::read(campaign.join("programs").join("far.json")).unwrap())
            .unwrap();
    assert_eq!(
        program["contract"]["no_body"]["part-house"]["part"], "part/house",
        "the part no body reaches is declared seen: {}",
        program["contract"]
    );
    let red = check_far(&campaign, &prefabs);
    let t = text(&red);
    assert!(!red.status.success(), "{t}");
    assert!(
        t.contains("not walked: `node/far`'s part `part/house`"),
        "{t}"
    );

    // The house's roof on the frame's top course, its side left undeclared:
    // the check names the exposure with its cell, as the build would.
    let run = run_generator(&tmp, &campaign, "roof-undeclared");
    assert!(run.status.success(), "{}", text(&run));
    let red = check_far(&campaign, &prefabs);
    let t = text(&red);
    assert_eq!(red.status.code(), Some(3), "{t}");
    assert!(
        t.contains("DW0885")
            && t.contains("its up side standing in open air")
            && t.contains("the first is at ["),
        "{t}"
    );

    // A lantern standing beside a seam's opening: the starter claims it in the
    // space the way opens into, so the way's cells touch that space and the
    // check passes (`contract-well-formed`).
    let run = run_generator(&tmp, &campaign, "lantern-by-way");
    assert!(run.status.success(), "{}", text(&run));
    let green = check_far(&campaign, &prefabs);
    assert!(green.status.success(), "{}", text(&green));

    // A court a course above the house's floor: the walk between the two
    // carries the rise the check measures, and the check passes.
    let run = run_generator(&tmp, &campaign, "raised-court");
    assert!(run.status.success(), "{}", text(&run));
    let program: serde_json::Value =
        serde_json::from_slice(&std::fs::read(campaign.join("programs").join("far.json")).unwrap())
            .unwrap();
    let risen: Vec<&serde_json::Value> = program["contract"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["class"] == "walk" && e.get("rise").is_some())
        .collect();
    assert!(
        !risen.is_empty(),
        "a walk between floors that differ carries its rise: {}",
        program["contract"]
    );
    let green = check_far(&campaign, &prefabs);
    assert!(green.status.success(), "{}", text(&green));

    let run = run_generator(&tmp, &campaign, "lantern-in-air");
    assert!(!run.status.success());
    assert!(
        text(&run).contains(
            "minecraft:lantern[hanging=true,waterlogged=false] at (12, 3, 4): the server drops it"
        ),
        "{}",
        text(&run)
    );

    let run = run_generator(&tmp, &campaign, "candle");
    assert!(!run.status.success());
    assert!(
        text(&run).contains("minecraft:candle: the palette refuses it (DW0730)"),
        "{}",
        text(&run)
    );

    // A state the pinned game does not have is refused before anything is
    // written: an unknown block, and an unknown property of a real one.
    let program = campaign.join("programs").join("far.json");
    for (edit, says) in [
        (
            "no-such-block",
            "minecraft:tuff_bricks_stairs is not a block in Minecraft 1.21.11",
        ),
        (
            "no-such-property",
            "minecraft:muddy_mangrove_roots has no property \"waterlogged\"",
        ),
    ] {
        let before = std::fs::read(&program).unwrap();
        let run = run_generator(&tmp, &campaign, edit);
        assert!(!run.status.success(), "{edit}: {}", text(&run));
        assert!(text(&run).contains(says), "{edit}: {}", text(&run));
        assert_eq!(
            before,
            std::fs::read(&program).unwrap(),
            "{edit}: nothing written"
        );
    }
}

/// **Every block constructor the starter offers writes a full state of the
/// pin**: each one's output, asked of the engine's survey, is a block of
/// Minecraft 1.21.11 with every property written — what `blocks-exist` and
/// `states-complete` judge in the finished piece. A misspelt state beside them
/// is named, so the question is the engine's and not this test's.
#[test]
fn every_block_constructor_writes_a_full_state_of_the_pin() {
    let tmp = tempdir("constructors");
    let script = tmp.join("constructors.py");
    std::fs::write(
        &script,
        r#"
import json, os, sys
sys.path.insert(0, os.path.join(os.environ["DELVEWRIGHT_SKILL"], "scripts"))
import starter as s
states = [
    s.stairs("spruce", "north"), s.stairs("stone_brick", "east", "top", "outer_left"),
    s.slab("deepslate_tile"), s.slab("spruce", "top"), s.pillar("spruce_log", "x"),
    s.pillar("stripped_spruce_wood"), s.pillar("hay_block"),
    s.door("spruce", "east", "lower"), s.door("spruce", "east", "upper", "right", True),
    s.trapdoor("dark_oak", "north", "bottom", True), s.fence("spruce"),
    s.fence_gate("oak", "south"), s.wall("cobblestone"), s.pane(), s.pane("iron_bars"),
    s.lantern(), s.lantern(True, True), s.leaves("azalea"), s.bed("red", "east", "foot"),
    s.candle(), s.candle(3, "white", True), s.torch(), s.wall_torch("west"), s.campfire(),
    s.ladder("north"), s.chain("z"), s.barrel(), s.button("stone", "north"),
    s.grass_block(), s.tall_plant("rose_bush", "lower"),
]
bad = "minecraft:oak_stairs[facing=east]"
a = s.ask_engine(sys.argv[1], {"size": [1, 1, 1], "blocks": [], "palette": states + [bad]})
print(json.dumps({"n": len(states), "malformed": a["malformed"]}))
"#,
    )
    .unwrap();
    let out = Command::new(python())
        .arg(&script)
        .arg(BIN)
        .env(
            "DELVEWRIGHT_SKILL",
            starter().parent().unwrap().parent().unwrap(),
        )
        .output()
        .expect("run the constructors");
    assert!(out.status.success(), "{}", text(&out));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let malformed = v["malformed"].as_array().unwrap();
    assert_eq!(v["n"].as_u64(), Some(30), "{v}");
    assert_eq!(
        malformed.len(),
        1,
        "only the planted state is malformed: {v}"
    );
    assert_eq!(malformed[0]["block"], "minecraft:oak_stairs[facing=east]");
    assert!(
        malformed[0]["why"]
            .as_str()
            .unwrap()
            .contains("leaves out half, shape, waterlogged"),
        "{v}"
    );
}
