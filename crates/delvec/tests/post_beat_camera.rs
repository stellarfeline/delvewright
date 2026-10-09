//! **A camera pictures the world as a story beat leaves it** (spec-0089).
//!
//! The fixture is hello-world with three more region writes, each in a corner
//! of `hello-room` no leg walks:
//!
//! - completing `obj/talk` (forced) opens the door **and** fills a pillar of
//!   gold at `anchor/pillar`;
//! - completing `obj/exit` (forced, the last objective) fills a second cell
//!   of the pillar's column one higher (`anchor/pillar-top`), so the end state
//!   holds a write no earlier configuration does;
//! - a trapped chest's payload (a beat nobody has to play) lays a plank at
//!   `anchor/plank`.
//!
//! Every world here is written by [`delvec::compiler::view::world`] and read
//! back by `tools/lib/anvil.py`, the reader `tools/ci/check-written-world.py`
//! uses, so what is asserted is what the renderer is handed.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use delvec::compiler::blockstate::BlockMap;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvec::compiler::view::{beat, camera, world};
use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign};

fn tmp(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("post-beat-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The pinned library with three more point anchors on `hello-room`: a floor
/// corner for the pillar, a ceiling cell for the collapse, a far floor corner
/// for the plank (local coordinates; the room's origin is the world's).
fn prefabs(tag: &str) -> PathBuf {
    let dir = tmp(&format!("{tag}-prefabs"));
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["anchors"]["anchor/pillar"] = serde_json::json!({ "pos": [1, 1, 1] });
    meta["anchors"]["anchor/pillar-top"] = serde_json::json!({ "pos": [1, 3, 1] });
    meta["anchors"]["anchor/plank"] = serde_json::json!({ "pos": [9, 1, 9] });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

const PILLAR: &str = r#"{ "type": "fill-region",
    "region": { "anchor": "anchor/pillar", "extent": [0, 1, 0] },
    "block": "minecraft:gold_block" }"#;

const CAP: &str = r#"{ "type": "fill-region",
    "region": { "anchor": "anchor/pillar-top", "extent": [0, 0, 0] },
    "block": "minecraft:copper_block" }"#;

const PLANK: &str = r#"{ "type": "fill-region",
    "region": { "anchor": "anchor/plank", "extent": [0, 0, 0] },
    "block": "minecraft:oak_planks" }"#;

fn quests() -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}, {PILLAR} ],
          "obj/exit": [ {CAP} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "traps": [ {{
      "id": "trap/plank-drop",
      "at": "anchor/exit",
      "trigger": "trapped-chest",
      "lethality": "nonlethal",
      "payload": [ {PLANK} ]
    }} ]
  }}
}}"#
    )
}

fn campaign() -> Campaign {
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("campaign parses")
}

fn cam(name: &str, after: Option<&str>) -> camera::Camera {
    camera::Camera {
        after: after.map(|s| camera::CameraAfter::parse(s).unwrap()),
        answers: "concept/keep".into(),
        exposure: 1.0,
        fov: 70.0,
        height: 90,
        name: name.into(),
        pitch: 10.0,
        pos: [5.5, 66.62, 2.5],
        sky: None,
        source: camera::Source::Estimated,
        spp: 16,
        width: 160,
        yaw: 0.0,
    }
}

/// Where every camera stands, and the worlds they stand in, for the fixture.
fn stands(tag: &str, cams: &[camera::Camera]) -> Result<(beat::Stands, BlockMap), String> {
    let lib = prefabs(tag);
    let c = campaign();
    let reg = PrefabRegistry::load_dir(&lib).unwrap();
    let plan = Plan::build(&c, &reg).expect("plan builds");
    let structures = common::plan_structures_with_trap_triggers(&plan, &lib);
    let assembled = delvec::compiler::assembled::assemble(&plan, &structures);
    let world = delvec::compiler::nav::World::from_occupancy(
        delvec::compiler::assembled::occupancy_over(&assembled.blocks, &assembled.open_gates),
        delvec::compiler::nav::Premises::of_plan(&plan, assembled.gate_seals.clone()),
    );
    let relight = delvec::compiler::light::relight_over(&plan, &assembled);
    let base = beat::picture_base(&plan, &assembled, &relight.placements);
    beat::stands(&plan, &world, &base, cams).map(|s| (s, base))
}

/// Read a written world back through `tools/lib/anvil.py`: every non-air cell
/// as `"x,y,z" -> state`.
fn read_back(dir: &Path) -> BTreeMap<String, String> {
    let lib = common::repo_root().join("tools/lib");
    let script = format!(
        "import sys, json, pathlib\nsys.path.insert(0, {lib:?})\nimport anvil\n\
         cells = anvil.cells(pathlib.Path({dir:?}))\n\
         print(json.dumps({{f'{{x}},{{y}},{{z}}': s for (x, y, z), s in cells.items()}}))\n",
        lib = lib.display().to_string(),
        dir = dir.display().to_string(),
    );
    let path = dir.with_extension("read.py");
    std::fs::write(&path, script).unwrap();
    let o = Command::new("python3")
        .arg(&path)
        .output()
        .expect("python3 runs");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).unwrap()
}

fn write(
    tag: &str,
    key: &str,
    blocks: &BlockMap,
) -> (world::WrittenWorld, BTreeMap<String, String>) {
    let dir = tmp(&format!("{tag}-{key}"));
    let w = world::write(
        blocks,
        &|_| "minecraft:plains".to_string(),
        [5, 65, 2],
        &dir,
    )
    .unwrap();
    (w, read_back(&dir))
}

fn base_name(s: Option<&String>) -> Option<&str> {
    s.map(|s| s.split('[').next().unwrap())
}

/// **Criterion 2 — the configuration is the proofs'.**
#[test]
fn a_camera_after_a_step_holds_what_the_proofs_hold_there() {
    let cams = [
        cam("before", None),
        cam("after-talk", Some("obj/talk")),
        cam("after-exit", Some("obj/exit")),
    ];
    let (stood, _) = stands("c2", &cams).expect("every camera stands somewhere");
    let keys: Vec<&str> = stood.stands.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, ["at-load", "after-obj-talk", "after-obj-exit"]);
    let map = |k: &str| &stood.worlds.iter().find(|(key, _)| key == k).unwrap().1;
    let (_, load) = write("c2", "at-load", map("at-load"));
    let (_, talk) = write("c2", "after-talk", map("after-obj-talk"));
    let (_, exit) = write("c2", "after-exit", map("after-obj-exit"));

    // The forced fill: in the world after `obj/talk`, not at load.
    for y in 64..=66 {
        let cell = format!("1,{y},1");
        if y == 64 {
            continue; // the floor
        }
        assert_eq!(
            base_name(talk.get(&cell)),
            Some("minecraft:gold_block"),
            "{cell}"
        );
        assert_ne!(
            base_name(load.get(&cell)),
            Some("minecraft:gold_block"),
            "{cell}"
        );
    }
    // The door the same beat opens: shut at load, open after.
    assert_eq!(base_name(load.get("4,65,6")), Some("minecraft:iron_bars"));
    assert_eq!(talk.get("4,65,6"), None, "the door is open after obj/talk");
    // After the last step: every forced write — the pillar, the door, and
    // the cap only the last step lays.
    assert_eq!(base_name(exit.get("1,65,1")), Some("minecraft:gold_block"));
    assert_eq!(exit.get("4,65,6"), None);
    assert_eq!(
        base_name(exit.get("1,67,1")),
        Some("minecraft:copper_block")
    );
    assert_ne!(
        base_name(talk.get("1,67,1")),
        Some("minecraft:copper_block")
    );
    // The unforced plank is in no picture.
    for (what, w) in [("load", &load), ("talk", &talk), ("exit", &exit)] {
        assert_ne!(
            base_name(w.get("9,65,9")),
            Some("minecraft:oak_planks"),
            "{what}"
        );
    }
    let after_exit = &stood.stands[2];
    assert_eq!(after_exit.unforced, 1, "the plank is counted as not laid");
    assert!(after_exit.cells_moved > 0);
}

/// **Criterion 2's second half**: no site of `crates/delvec/src` lays a region
/// write's block over a block map but `RegionState::blocks_over`.
#[test]
fn one_function_lays_a_configuration() {
    let src = common::repo_root().join("crates/delvec/src");
    let mut sites = Vec::new();
    let mut stack = vec![src];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                // The only iteration over a state's laid writes.
                for (i, line) in text.lines().enumerate() {
                    let t = line.trim_start();
                    if t.starts_with("for ") && t.contains(" in &") && t.contains("laid") {
                        sites.push(format!("{}:{}", p.display(), i + 1));
                    }
                }
            }
        }
    }
    eprintln!("laid-write sites: {} — {sites:?}", sites.len());
    // `blocks_over` lays them; `moved_from` counts the same cells without
    // laying. Two readers of one list, one writer.
    assert_eq!(sites.len(), 2, "{sites:?}");
    assert!(
        sites.iter().all(|s| s.contains("compiler/nav/")),
        "{sites:?}"
    );
}

/// **Criterion 3 — refusals**, each under `DW0721`'s rules with the remedy
/// stated, through the one reader every caller asks.
#[test]
fn a_camera_s_after_is_refused_where_it_names_nothing_to_picture() {
    let cases = [
        (
            "obj/nowhere",
            "is no step of the critical path",
            "Steps: obj/talk, obj/exit",
        ),
        (
            "obj/talk@branch/none",
            "names no branch the build declares",
            "Branches",
        ),
    ];
    for (after, said, remedy) in cases {
        let err = stands("c3", &[cam("x", Some(after))]).unwrap_err();
        assert!(err.contains(said) && err.contains(remedy), "{after}: {err}");
        assert!(
            err.contains("camera `x`"),
            "{after}: the camera is named: {err}"
        );
    }
    // `--after` that does not parse.
    for bad in ["talk", "obj/", "obj/talk@", "step/3"] {
        let err = camera::CameraAfter::parse(bad).unwrap_err();
        assert!(err.contains("<step>[@<path>]"), "{bad}: {err}");
    }
}

/// A plain hello-room campaign whose `on_objective_complete` is `bundles`,
/// planned, assembled and asked where `cams` stand.
fn plain_stands(
    bundles: serde_json::Value,
    cams: &[camera::Camera],
) -> Result<beat::Stands, String> {
    let lib = common::prefabs_dir();
    let mut q: serde_json::Value = serde_json::from_str(&quests()).unwrap();
    q["content"]["quests"][0]["on_objective_complete"] = bundles;
    // A first step before the keeper, so a step exists before any beat fires.
    q["content"]["quests"][0]["objectives"] = serde_json::json!([
        { "type": "reach-anchor", "id": "obj/look", "anchor": "anchor/keeper-stand", "radius": 2 },
        { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper", "after": ["obj/look"] },
        { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
          "after": ["obj/talk"] }
    ]);
    q["content"]["traps"] = serde_json::json!([]);
    let c = parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: q.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .unwrap();
    let reg = PrefabRegistry::load_dir(&lib).unwrap();
    // The route proofs are the build's; the plan is what a camera is asked of.
    let plan = Plan::build(&c, &reg).unwrap();
    let structures = common::plan_structures_with_trap_triggers(&plan, &lib);
    let assembled = delvec::compiler::assembled::assemble(&plan, &structures);
    let world = delvec::compiler::nav::World::from_occupancy(
        delvec::compiler::assembled::occupancy_over(&assembled.blocks, &assembled.open_gates),
        delvec::compiler::nav::Premises::of_plan(&plan, assembled.gate_seals.clone()),
    );
    let base = beat::picture_base(&plan, &assembled, &[]);
    beat::stands(&plan, &world, &base, cams)
}

/// **Criterion 3 — an after equal to load**: a step after which no block moves
/// is refused, naming the first step after which one does — or saying the path
/// moves none.
#[test]
fn a_step_after_which_nothing_moves_is_refused_naming_the_first_that_does() {
    // After `obj/look` nothing has fired, so the world is the load world;
    // `obj/talk` opens the door.
    let door = serde_json::json!({
        "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
    });
    let err = plain_stands(door.clone(), &[cam("t", Some("obj/look"))]).unwrap_err();
    assert!(
        err.contains("is the world at load block for block")
            && err.contains("REMOVE `after` from camera `t`")
            && err.contains("the first on the critical path is `obj/talk`"),
        "{err}"
    );
    let ok = plain_stands(door, &[cam("e", Some("obj/talk"))]).unwrap();
    assert_eq!(ok.stands[0].cells_moved, 6, "the door's six cells");
    // A path that moves nothing at all says so.
    let err = plain_stands(serde_json::json!({}), &[cam("t", Some("obj/look"))]).unwrap_err();
    assert!(
        err.contains("the critical path moves no block at all"),
        "{err}"
    );
}

/// **Criterion 4 — the writer is whole and deterministic.**
#[test]
fn the_writer_is_whole_and_deterministic() {
    let (stood, _) = stands("c4", &[cam("x", Some("obj/talk"))]).unwrap();
    let blocks = &stood.worlds[0].1;
    let biome = |c: [i32; 3]| -> String {
        if c[0] < 4 {
            "minecraft:plains".into()
        } else {
            "minecraft:desert".into()
        }
    };
    let a = world::encode(blocks, &biome, [5, 65, 2]);
    let b = world::encode(blocks, &biome, [5, 65, 2]);
    assert_eq!(a, b, "two writes of one map are byte-identical");
    let dir = tmp("c4-x");
    let w = world::write(blocks, &biome, [5, 65, 2], &dir).unwrap();
    assert_eq!(w, a, "the files written are the bytes encoded");
    let back = read_back(&dir);
    // Read back cell for cell equal, every state completed by the pinned
    // defaults the writer fills.
    let reg = delvewright_dsl::blocks::BlockRegistry::v1_21_11();
    let mut compared = 0usize;
    for (c, s) in blocks {
        let key = format!("{},{},{}", c[0], c[1], c[2]);
        let got = back
            .get(&key)
            .unwrap_or_else(|| panic!("{key} not read back"));
        let (name, _) = s.as_str().split_once('[').unwrap_or((s.as_str(), ""));
        assert!(got.starts_with(name), "{key}: wrote {s}, read {got}");
        for (k, v) in s
            .as_str()
            .split_once('[')
            .map(|(_, p)| p.trim_end_matches(']'))
            .unwrap_or("")
            .split(',')
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.split_once('='))
        {
            assert!(got.contains(&format!("{k}={v}")), "{key}: {s} vs {got}");
        }
        if let Some(d) = reg.default_state(name) {
            for k in d.keys() {
                assert!(
                    got.contains(&format!("{k}=")),
                    "{key}: {got} lacks default {k}"
                );
            }
        }
        compared += 1;
    }
    assert_eq!(compared, blocks.len());
    assert_eq!(
        back.len(),
        blocks.len(),
        "nothing read back that was not written"
    );
    // The bytes: zero timestamps, the pinned DataVersion, full status, sections
    // only where cells are, packing at max(4, ceil(log2 n)) / ceil(log2 n).
    let o = Command::new("python3")
        .arg("-c")
        .arg(format!(
            "import sys, pathlib, struct\nsys.path.insert(0, {lib:?})\nimport anvil\n\
             d = pathlib.Path({dir:?})\n\
             for f in sorted((d / 'region').glob('r.*.mca')):\n\
             \x20   raw = f.read_bytes()\n\
             \x20   assert raw[4096:8192] == bytes(4096), 'timestamps'\n\
             \x20   for cx, cz, ts, comp, root in anvil.region_chunks(f):\n\
             \x20       r = root.value\n\
             \x20       assert ts == 0 and comp == 2\n\
             \x20       assert r['DataVersion'].value == {dv} and r['Status'].value == 'minecraft:full', r\n\
             \x20       for s in r['sections'].value:\n\
             \x20           assert s.value['Y'].kind == anvil.T_BYTE, 'a section Y is a byte'\n\
             \x20           bs = s.value['block_states'].value; n = len(bs['palette'].value)\n\
             \x20           assert any(p.value['Name'].value != 'minecraft:air' for p in bs['palette'].value)\n\
             \x20           if n == 1: assert 'data' not in bs\n\
             \x20           else: assert len(bs['data'].value) == -(-4096 // (64 // max(4, (n - 1).bit_length())))\n\
             \x20           bi = s.value['biomes'].value; m = len(bi['palette'].value)\n\
             \x20           if m == 1: assert 'data' not in bi\n\
             \x20           else: assert len(bi['data'].value) == -(-64 // (64 // anvil.biome_bits(m)))\n\
             print('ok')\n",
            lib = common::repo_root().join("tools/lib").display().to_string(),
            dir = dir.display().to_string(),
            dv = world::DATA_VERSION,
        ))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

// ---------------------------------------------------------------------------
// Through the binary: hello-world, whose `obj/talk` opens the door
// ---------------------------------------------------------------------------

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

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

/// A hello-world copy with one approved picture and `cameras` as its record
/// (the fixture's lens, `tests/fixtures/post-beat-camera/cameras.json`, with
/// `after` per camera as given).
fn hello(tag: &str, cameras: &[(&str, Option<&str>)]) -> PathBuf {
    let camp = tmp(&format!("{tag}-camp"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
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

fn built(tag: &str, camp: &Path) -> PathBuf {
    let out = tmp(&format!("{tag}-out"));
    let (code, log) = run(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    out
}

fn cameras(build: &Path, camp: &Path, out: &Path, extra: &[&str]) -> (i32, String) {
    let mut args = vec![
        "--prefabs",
        common::prefabs_dir().to_str().unwrap().to_owned().leak(),
        "cameras",
        build.to_str().unwrap(),
        "--campaign",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    run(&args)
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// **Criterion 8 — the binding line**, on a record with no `after` and on one
/// with, the count of moved cells equal to the cells `blocks_over` changed
/// (hello-world's door: its six cells), and the step's position read off the
/// build's own path.
#[test]
fn cameras_print_where_every_camera_stands() {
    let camp = hello("c8-plain", &[("hero", None)]);
    let out = built("c8-plain", &camp);
    let scenes = tmp("c8-plain-scenes");
    let (code, log) = cameras(&out, &camp, &scenes, &[]);
    assert_eq!(code, 0, "{log}");
    assert!(log.contains("after: hero at load"), "{log}");
    assert!(
        log.contains("configurations: 1 written for 1 camera(s), 0 after a step, 1 at load"),
        "{log}"
    );
    assert_eq!(
        log.matches("\nworld: ").count() + usize::from(log.starts_with("world: ")),
        1,
        "{log}"
    );
    assert!(log.contains("world: at-load "), "{log}");

    let camp = hello("c8-door", &[("hero", None), ("door", Some("obj/talk"))]);
    let out = built("c8-door", &camp);
    let steps = json(&out.join("critical-path.json"))["steps"]
        .as_array()
        .unwrap()
        .len();
    let talk = json(&out.join("critical-path.json"))["steps"]
        .as_array()
        .unwrap()
        .iter()
        .position(|s| s["objective"] == "obj/talk")
        .unwrap();
    let scenes = tmp("c8-door-scenes");
    let (code, log) = cameras(&out, &camp, &scenes, &[]);
    assert_eq!(code, 0, "{log}");
    let want = format!(
        "after: door after obj/talk (step {} of {steps} on the critical path): 6 cells moved from load, 0 unforced write(s) not laid, 0 block entit(ies) omitted; biomes: at load; clock: as the picture",
        talk + 1
    );
    assert!(log.contains(&want), "`{want}` in:\n{log}");
    assert!(
        log.contains("configurations: 2 written for 2 camera(s), 1 after a step, 1 at load"),
        "{log}"
    );
    assert!(log.contains("world: after-obj-talk "), "{log}");
    let door = json(&scenes.join("hello-world_camera_door.json"));
    assert!(
        door["world"]["path"]
            .as_str()
            .unwrap()
            .ends_with("worlds/after-obj-talk"),
        "{door}"
    );
    let hero = json(&scenes.join("hello-world_camera_hero.json"));
    assert!(
        hero["world"]["path"]
            .as_str()
            .unwrap()
            .ends_with("worlds/at-load"),
        "{hero}"
    );
    // The six cells are the door's, read back from the two written worlds.
    let load = read_back(&scenes.join("worlds/at-load"));
    let after = read_back(&scenes.join("worlds/after-obj-talk"));
    let moved: Vec<&String> = load
        .keys()
        .chain(after.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|k| load.get(*k) != after.get(*k))
        .collect();
    assert_eq!(moved.len(), 6, "{moved:?}");
}

/// **Criterion 7 — byte identity across the pin.** Two runs over one record,
/// plan and prefabs write byte-identical scenes and worlds; and a camera with
/// no `after` writes the scene the engine wrote before this change for the same
/// record, but for `world.path` (`tests/fixtures/post-beat-camera/
/// base-scene-hero.json`, emitted by the engine at the merge base, its path
/// masked).
#[test]
fn a_scene_is_the_scene_it_was_but_for_its_world() {
    let camp = hello("c7", &[("hero", None)]);
    let out = built("c7", &camp);
    let mut runs = Vec::new();
    for r in ["a", "b"] {
        let scenes = tmp(&format!("c7-{r}"));
        let (code, log) = cameras(&out, &camp, &scenes, &[]);
        assert_eq!(code, 0, "{log}");
        let mut scene = json(&scenes.join("hello-world_camera_hero.json"));
        assert!(
            scene["world"]["path"]
                .as_str()
                .unwrap()
                .ends_with(&format!("post-beat-c7-{r}/worlds/at-load"))
        );
        scene["world"]["path"] = serde_json::json!("<masked>");
        runs.push((
            scene,
            std::fs::read(scenes.join("worlds/at-load/level.dat")).unwrap(),
            std::fs::read(scenes.join("worlds/at-load/region/r.0.0.mca")).unwrap(),
        ));
    }
    assert_eq!(runs[0], runs[1], "two runs, one set of bytes");
    let mut base = json(
        &common::repo_root()
            .join("crates/delvec/tests/fixtures/post-beat-camera/base-scene-hero.json"),
    );
    base["world"]["path"] = serde_json::json!("<masked>");
    assert_eq!(runs[0].0, base, "the scene moved by more than its world");
}

/// **Criterion 1 — the surface.** `delvec schema cameras` exports `after`;
/// `place-camera --after` writes it; `--candidates --pick` carries a
/// candidate's `after` verbatim; `--bracket` writes every candidate with its
/// camera's `after`.
#[test]
fn the_record_carries_after_through_every_writer() {
    let (code, schema) = run(&["schema", "--stage", "cameras"]);
    assert_eq!(code, 0, "{schema}");
    let v: serde_json::Value = serde_json::from_str(&schema).unwrap();
    let defs = v.to_string();
    assert!(
        defs.contains("\"after\"") && defs.contains("CameraAfter"),
        "{schema}"
    );

    let camp = hello("c1", &[("hero", None)]);
    let out = built("c1", &camp);
    // --bracket: every candidate of a camera taken after a step carries it.
    common::patch_file(&camp.join("design/cameras.json"), |v| {
        v["cameras"][0]["after"] = serde_json::json!({"step": "obj/talk"});
    });
    let scenes = tmp("c1-scenes");
    let (code, log) = cameras(&out, &camp, &scenes, &["--bracket", "yaw=8", "--draft"]);
    assert_eq!(code, 0, "{log}");
    let cands = json(&scenes.join("candidates.json"));
    let afters: Vec<&serde_json::Value> = cands["cameras"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| &c["after"])
        .collect();
    assert_eq!(afters.len(), 3);
    assert!(afters.iter().all(|a| a["step"] == "obj/talk"), "{cands}");
    // --candidates --pick: verbatim.
    std::fs::remove_file(camp.join("design/cameras.json")).unwrap();
    let pick = |extra: &[&str]| {
        let mut args = vec![
            "--prefabs",
            common::prefabs_dir().to_str().unwrap().to_owned().leak(),
            "place-camera",
            camp.to_str().unwrap().to_owned().leak(),
            "--name",
            "hero",
            "--answers",
            "concept/keep",
            "--candidates",
            scenes
                .join("candidates.json")
                .to_str()
                .unwrap()
                .to_owned()
                .leak(),
            "--pick",
            "hero.yaw+8",
        ];
        args.extend_from_slice(extra);
        run(&args)
    };
    let (code, log) = pick(&[]);
    assert_eq!(code, 0, "{log}");
    assert!(log.contains("after: hero after obj/talk"), "{log}");
    assert_eq!(
        json(&camp.join("design/cameras.json"))["cameras"][0]["after"]["step"],
        "obj/talk"
    );
    // --after writes it, and keys stay alphabetical: `after` first.
    let (code, log) = pick(&["--after", "obj/exit"]);
    assert_eq!(code, 0, "{log}");
    let text = std::fs::read_to_string(camp.join("design/cameras.json")).unwrap();
    assert!(
        text.contains("\"after\": {\n        \"step\": \"obj/exit\"\n      },\n      \"answers\""),
        "{text}"
    );
}

/// **Criterion 10 — the plan states each shot's configuration.** Every POV
/// shot carries `after: {step, cells_moved}`; the legs walked once the door is
/// open stand in its configuration; `delvec scene` prints the limit with the
/// two counts the plan states.
#[test]
fn the_plan_states_where_every_review_frame_stands() {
    let camp = hello("c10", &[("hero", None)]);
    let out = built("c10", &camp);
    let plan = json(&out.join("render-plan.json"));
    let pov: Vec<&serde_json::Value> = plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "pov")
        .collect();
    assert!(!pov.is_empty());
    assert!(
        pov.iter().all(|s| s["after"]["cells_moved"].is_u64()),
        "every POV shot states it"
    );
    let moved = pov
        .iter()
        .filter(|s| s["after"]["cells_moved"].as_u64() > Some(0))
        .count();
    // The leg to the exit is walked through the open door: six cells moved,
    // after `obj/talk`; the legs before it stand at load.
    for s in &pov {
        if s["objective"] == "obj/exit" {
            assert_eq!(
                s["after"],
                serde_json::json!({"step": "obj/talk", "cells_moved": 6}),
                "{s}"
            );
        } else {
            assert_eq!(s["after"]["cells_moved"], 0, "{s}");
        }
    }
    assert!(moved > 0 && moved < pov.len(), "{moved} of {}", pov.len());
    let world = out.join("world");
    std::fs::create_dir_all(world.join("region")).unwrap();
    std::fs::write(world.join("level.dat"), b"stub").unwrap();
    std::fs::write(world.join("region/r.0.0.mca"), b"stub").unwrap();
    let (code, log) = run(&[
        "scene",
        out.to_str().unwrap(),
        "-o",
        tmp("c10-scenes").to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    let want = format!(
        "review frames render the world at load; {moved} of {} POV shots stand in a configuration other than load",
        pov.len()
    );
    assert!(log.contains(&want), "`{want}` in:\n{log}");
}

/// **`DW0955` — the cross-check reds on a world that is not the server's.**
/// The gate is the instrument (`tools/ci/check-written-world.py`); this holds
/// its code to the engine's declaration and its two perturbations: one cell
/// moved in a copy of the written world reds it, a gravel column the server
/// settled lands in the gravity class and does not.
#[test]
fn the_written_world_cross_check_reds_on_a_cell_the_server_does_not_hold() {
    let camp = hello("dw0955", &[("hero", None)]);
    let out = built("dw0955", &camp);
    let scenes = tmp("dw0955-scenes");
    let (code, log) = cameras(&out, &camp, &scenes, &[]);
    assert_eq!(code, 0, "{log}");
    let written = scenes.join("worlds/at-load");
    let gate = common::repo_root().join("tools/ci/check-written-world.py");
    let check = |server: &Path| {
        let o = Command::new("python3")
            .arg(&gate)
            .arg(&out)
            .arg(&written)
            .arg(server)
            .output()
            .unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).to_string(),
        )
    };
    // The written world against itself: green, every class zero.
    let (code, said) = check(&written);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("model: 0 differing cell(s)"), "{said}");
    // A "server" world with one cell moved: red, under the engine's own code.
    let mut blocks: BlockMap = read_back(&written)
        .into_iter()
        .map(|(k, v)| {
            let c: Vec<i32> = k.split(',').map(|n| n.parse().unwrap()).collect();
            (
                [c[0], c[1], c[2]],
                delvec::compiler::blockstate::BlockState::new(&v),
            )
        })
        .collect();
    let floor = [3, 64, 3];
    assert!(blocks.contains_key(&floor));
    blocks.insert(
        floor,
        delvec::compiler::blockstate::BlockState::new("minecraft:gold_block"),
    );
    let moved = tmp("dw0955-moved");
    world::write(&blocks, &|_| "minecraft:plains".into(), [5, 65, 2], &moved).unwrap();
    let (code, said) = check(&moved);
    assert_eq!(code, 1, "{said}");
    assert!(
        said.contains(world::DW_CAMERA_STEP_WORLD.id()) && said.contains("DW0955"),
        "{said}"
    );
    assert!(said.contains("model: 1 differing cell(s)"), "{said}");
    // A settled gravel the server holds and the model does not: the gravity
    // class, counted, not red.
    blocks.insert(
        floor,
        delvec::compiler::blockstate::BlockState::new(read_back(&written)["3,64,3"].as_str()),
    );
    blocks.insert(
        [3, 65, 3],
        delvec::compiler::blockstate::BlockState::new("minecraft:gravel"),
    );
    let settled = tmp("dw0955-settled");
    world::write(
        &blocks,
        &|_| "minecraft:plains".into(),
        [5, 65, 2],
        &settled,
    )
    .unwrap();
    let (code, said) = check(&settled);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("gravity: 1 differing cell(s)"), "{said}");

    // A `structure_void` the server holds where the model has air — what a
    // template shipped with its voids writes into the world — is a model cell,
    // red; and `--record` writes the verdict named by the build's manifest.
    blocks.remove(&[3, 65, 3]);
    let over = [3, 65, 3];
    blocks.insert(
        over,
        delvec::compiler::blockstate::BlockState::new("minecraft:structure_void"),
    );
    let voided = tmp("dw0955-voided");
    world::write(&blocks, &|_| "minecraft:plains".into(), [5, 65, 2], &voided).unwrap();
    let record = tmp("dw0955-record").join("written-world.json");
    let o = Command::new("python3")
        .arg(&gate)
        .arg(&out)
        .arg(&written)
        .arg(&voided)
        .arg("--record")
        .arg(&record)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&o.stdout).to_string();
    assert_eq!(o.status.code(), Some(1), "{said}");
    assert!(said.contains("model: 1 differing cell(s)"), "{said}");
    assert!(said.contains("server minecraft:structure_void"), "{said}");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&record).unwrap()).unwrap();
    assert_eq!(doc["verdict"], "fail", "{doc}");
    assert_eq!(doc["classes"]["model"], 1, "{doc}");
    assert_eq!(doc["model_sample"][0]["server"], "minecraft:structure_void");
    let manifest = std::fs::read(out.join("manifest.json")).unwrap();
    let sha = {
        use sha2::Digest;
        format!("{:x}", sha2::Sha256::digest(&manifest))
    };
    assert_eq!(doc["manifest_sha256"], sha.as_str(), "{doc}");
    // The same record on a green.
    let o = Command::new("python3")
        .arg(&gate)
        .arg(&out)
        .arg(&written)
        .arg(&written)
        .arg("--record")
        .arg(&record)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&record).unwrap()).unwrap();
    assert_eq!(doc["verdict"], "pass", "{doc}");
    assert_eq!(doc["classes"]["model"], 0, "{doc}");
}

/// spec-0089 meets spec-0092 and spec-0080's repaints on one beat: a camera
/// after a step that repaints a volume and strikes a bolt states what its
/// world does not carry. The bolt writes no block (its fire is the game's,
/// not the configuration's), so the moved cells are the door's six alone;
/// the repaint is spec-0089's recorded debt (its §6.3, decision 7) — the
/// written world takes the biome map at load — and the line says so rather
/// than picturing a sky the beat changed as unchanged without a word.
#[test]
fn a_camera_after_a_repaint_and_a_strike_names_the_biomes_it_does_not_lay() {
    let camp = hello("c-repaint", &[("door", Some("obj/talk"))]);
    common::patch_file(&camp.join("world.json"), |w| {
        w["content"]["atmospheres"] = serde_json::json!([{
            "id": "atmosphere/hush", "precipitation": "none",
            "attributes": { "visual/fog_end_distance": 26.0, "visual/sky_color": "#3b4a1e" }
        }]);
    });
    common::patch_file(&camp.join("quests.json"), |q| {
        let beat = q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        beat.push(serde_json::json!({
            "type": "set-atmosphere", "atmosphere": "atmosphere/hush",
            "region": { "anchor": "anchor/door", "extent": [2, 2, 2] },
            "happening": { "verb": "opens", "text": "the air at the door goes still" }
        }));
        beat.push(serde_json::json!({
            "type": "lightning", "at": { "anchor": "anchor/exit" },
            "happening": { "verb": "survives", "text": "a bolt strikes the road outside" }
        }));
    });
    let out = built("c-repaint", &camp);
    let scenes = tmp("c-repaint-scenes");
    let (code, log) = cameras(&out, &camp, &scenes, &[]);
    assert_eq!(code, 0, "{log}");
    let line = log
        .lines()
        .find(|l| l.starts_with("after: door after obj/talk"))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(
        line.contains(": 6 cells moved from load, 0 unforced write(s) not laid,"),
        "the bolt writes no block: {line}"
    );
    assert!(
        line.contains("; biomes: at load;"),
        "the repaint is not laid, and the line says so: {line}"
    );
}
