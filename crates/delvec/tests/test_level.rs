//! `world.test_level` (spec-0122 §7.2): a level built to try something, with no
//! approved design record, says so; its build carries the declaration in
//! `manifest.json` for the staging gate to read; a record of its own cameras
//! answers no `design.json` row; and a test level that also carries a design
//! record is refused `DW1061`.
//!
//! One campaign is built twice, once as written and once with the declaration,
//! and the two manifests are compared: the declared build carries
//! `test_level: true`, the other carries no such key, and every emitted path
//! hashes the same — nothing a player meets moves.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("delvec runs")
}

fn log(o: &Output) -> String {
    format!(
        "exit {:?}\n{}{}",
        o.status.code(),
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn tmp(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("test-level-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build_out(campaign: &Path, out: &Path) -> Output {
    let _ = std::fs::remove_dir_all(out);
    delvec(&[
        "build",
        campaign.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ])
}

fn build(campaign: &Path, out: &Path) -> serde_json::Value {
    let r = build_out(campaign, out);
    assert!(r.status.success(), "{}", log(&r));
    serde_json::from_slice(&std::fs::read(out.join("manifest.json")).expect("manifest"))
        .expect("manifest parses")
}

fn campaign_at(dir: &Path, test_level: Option<bool>) {
    let _ = std::fs::remove_dir_all(dir);
    common::copy_dir_all(&common::hello_world_dir(), dir);
    if let Some(v) = test_level {
        common::patch_file(&dir.join("world.json"), |w| {
            w["content"]["test_level"] = serde_json::json!(v);
        });
    }
}

#[test]
fn a_declared_test_level_is_carried_in_the_manifest_and_moves_nothing_else() {
    let tmp = tmp("manifest");
    let (plain, test) = (tmp.join("plain"), tmp.join("test"));
    campaign_at(&plain, None);
    campaign_at(&test, Some(true));
    let a = build(&plain, &tmp.join("plain.out"));
    let b = build(&test, &tmp.join("test.out"));

    assert!(a.get("test_level").is_none(), "{a}");
    assert_eq!(b["test_level"], serde_json::json!(true), "{b}");
    assert_eq!(
        a["outputs"], b["outputs"],
        "a test level emits the same world"
    );
    let (ai, bi) = (
        a["inputs"].as_object().unwrap(),
        b["inputs"].as_object().unwrap(),
    );
    assert_eq!(ai.keys().collect::<Vec<_>>(), bi.keys().collect::<Vec<_>>());
    let moved: Vec<&String> = ai.keys().filter(|k| ai[*k] != bi[*k]).collect();
    assert_eq!(
        moved,
        vec!["world.json"],
        "only the declaring document moves"
    );
}

#[test]
fn test_level_false_is_the_same_build_as_no_declaration() {
    let tmp = tmp("false");
    let (plain, off) = (tmp.join("plain"), tmp.join("off"));
    campaign_at(&plain, None);
    campaign_at(&off, Some(false));
    let a = build(&plain, &tmp.join("plain.out"));
    let b = build(&off, &tmp.join("off.out"));
    assert!(b.get("test_level").is_none(), "{b}");
    assert_eq!(a["outputs"], b["outputs"]);
}

/// A `design.json` of one approved row, and its image.
fn design_record(camp: &Path) {
    std::fs::create_dir_all(camp.join("design/concept")).unwrap();
    std::fs::write(camp.join("design/concept/keep.png"), b"not read").unwrap();
    let doc = serde_json::json!({
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "design",
        "content": { "references": [{
            "name": "concept/keep", "shows": "the keep at its best",
            "time": "noon", "weather": "clear"
        }]},
    });
    std::fs::write(
        camp.join("design.json"),
        serde_json::to_vec_pretty(&doc).unwrap(),
    )
    .unwrap();
}

/// **`DW1061`.** A test level that carries `design.json` is refused at
/// validate, naming the campaign and both ways out; dropping the declaration,
/// or the record, validates.
#[test]
fn a_test_level_with_a_design_record_is_refused_dw1061() {
    let tmp = tmp("dw1061");
    let camp = tmp.join("camp");
    campaign_at(&camp, Some(true));
    design_record(&camp);
    let validate = |c: &Path| {
        delvec(&[
            "validate",
            c.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
    };
    let r = validate(&camp);
    let said = log(&r);
    assert_eq!(r.status.code(), Some(1), "{said}");
    assert!(said.contains("DW1061"), "{said}");
    assert!(
        said.contains("declares `test_level: true` and carries `design.json`")
            && said.contains("hello-world")
            && said.contains("drop one"),
        "{said}"
    );
    // Either way out validates.
    let no_decl = tmp.join("no-decl");
    campaign_at(&no_decl, None);
    design_record(&no_decl);
    let r = validate(&no_decl);
    assert!(!log(&r).contains("DW1061"), "{}", log(&r));
    let no_record = tmp.join("no-record");
    campaign_at(&no_record, Some(true));
    let r = validate(&no_record);
    assert_eq!(r.status.code(), Some(0), "{}", log(&r));
}

/// The eye of hello-world's first player-POV shot, which the build proved
/// clear.
fn proven_eye(built: &Path) -> serde_json::Value {
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(built.join("render-plan.json")).unwrap()).unwrap();
    plan["shots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "pov")
        .expect("hello-world has a walked POV")["camera"]["pos"]
        .clone()
}

fn record(camp: &Path, cam: serde_json::Value) {
    std::fs::create_dir_all(camp.join("design")).unwrap();
    std::fs::write(
        camp.join("design/cameras.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "campaign_id": "hello-world", "cameras": [cam]
        }))
        .unwrap(),
    )
    .unwrap();
}

/// **A test level's camera answers no approved image.** Its row states no
/// `answers` and names what it frames in `note`, with a stated sky: the build
/// passes with no `design.json`, and `delvec cameras --preview` draws it. The
/// same row on a campaign that is not a test level is refused `DW0721`, by the
/// build and by `delvec cameras`, naming the camera.
#[test]
fn a_test_levels_camera_answers_no_row_and_is_drawn() {
    let tmp = tmp("cameras");
    let plain = tmp.join("plain");
    campaign_at(&plain, Some(true));
    let built = tmp.join("plain.out");
    build(&plain, &built);
    let cam = serde_json::json!({
        "fov": 70.0, "height": 90, "name": "hero", "note": "the keep's door from the yard",
        "pitch": 10.0, "pos": proven_eye(&built), "source": "estimated",
        "sky": {"time": "noon", "weather": "clear"}, "spp": 16, "width": 160, "yaw": 0.0
    });

    let test = tmp.join("test");
    campaign_at(&test, Some(true));
    record(&test, cam.clone());
    let test_out = tmp.join("test.out");
    let r = build_out(&test, &test_out);
    assert!(r.status.success(), "{}", log(&r));
    let shots = tmp.join("shots");
    let r = delvec(&[
        "cameras",
        test_out.to_str().unwrap(),
        "--campaign",
        test.to_str().unwrap(),
        "-o",
        shots.to_str().unwrap(),
        "--preview",
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    let said = log(&r);
    assert_eq!(r.status.code(), Some(0), "{said}");
    assert!(said.contains("previewed 1 camera frame(s)"), "{said}");
    assert!(said.contains("sky: hero noon+clear stated"), "{said}");

    // Not a test level: refused, by the build and by the camera command.
    let campaign = tmp.join("campaign");
    campaign_at(&campaign, None);
    record(&campaign, cam);
    let r = build_out(&campaign, &tmp.join("campaign.out"));
    let said = log(&r);
    assert_eq!(r.status.code(), Some(3), "{said}");
    assert!(
        said.contains("DW0721") && said.contains("camera `hero` states no `answers`"),
        "{said}"
    );
    design_record(&campaign);
    let r = delvec(&[
        "cameras",
        test_out.to_str().unwrap(),
        "--campaign",
        campaign.to_str().unwrap(),
        "-o",
        tmp.join("shots-campaign").to_str().unwrap(),
        "--preview",
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    let said = log(&r);
    assert_eq!(r.status.code(), Some(2), "{said}");
    assert!(
        said.contains("DW0721") && said.contains("camera `hero` states no `answers`"),
        "{said}"
    );
}
