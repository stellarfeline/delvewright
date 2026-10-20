//! spec-0105: every container is loot, a device or scenery — the census, its
//! binding line and ledger, the arrival notice, and each refusal shape
//! (`DW1018`, `DW1019`, `DW1024`) on a fixture built through the binary.
//!
//! The fixture is hello-world furnished by a stage-7 batch
//! ([`common::container_campaign`]): hello-room carries no container of its
//! own, so every container a test counts is one the test placed.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BARREL: &str = "minecraft:barrel";

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("containers-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Build the fixture; return the exit code, what it said and the output dir.
fn build(
    tag: &str,
    blocks: &[([i32; 3], &str)],
    collect: Option<&str>,
    notice: bool,
) -> (i32, String, PathBuf) {
    let dir = scratch(tag);
    let camp = common::container_campaign(&dir.join("camp"), blocks, collect, notice);
    let out = dir.join("out");
    let prefabs = common::prefabs_dir();
    let o = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (o.status.code().unwrap_or(-1), said(&o), out)
}

fn census_line(log: &str) -> &str {
    log.lines()
        .find(|l| l.starts_with("container census:"))
        .unwrap_or_else(|| panic!("no census line in:\n{log}"))
}

fn class_apply_bodies(out: &Path) -> Vec<String> {
    let dir = out.join("datapack/data/hello-world/function");
    let mut bodies = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("class_apply_") {
            bodies.push(std::fs::read_to_string(&p).unwrap());
        }
    }
    assert!(
        !bodies.is_empty(),
        "no class_apply_* under {}",
        dir.display()
    );
    bodies
}

/// spec-0105 criterion 7: a campaign with no container prints the line with
/// a measured zero, and still writes the ledger.
#[test]
fn a_campaign_with_no_container_prints_a_measured_zero() {
    let (code, log, out) = build("none", &[], None, false);
    assert_eq!(code, 0, "{log}");
    let line = census_line(&log);
    assert!(line.contains("0 container cell(s)"), "{line}");
    assert!(line.contains("notice none"), "{line}");
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/containers.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["cells"], 0);
    assert!(ledger["notice"].is_null());
    // criterion 3: no notice, no line in any class apply.
    for body in class_apply_bodies(&out) {
        assert!(!body.contains("tellraw"), "{body}");
    }
}

/// `DW1018`, first arm: scenery with no notice. The notice is the remedy, and
/// with it the build is green, the ledger carries the row and the notice, and
/// every class apply tells it after `dw.classed` and after the teleport, its
/// sound played `at` the player — at the arrival, never at the command source.
#[test]
fn scenery_with_no_notice_is_dw1018_and_the_notice_builds() {
    let (code, log, _) = build("bare", &[([-3, 0, -2], BARREL)], None, false);
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1018"), "{log}");
    assert!(log.contains("`minecraft:barrel` at"), "{log}");
    assert!(log.contains("area/keep"), "{log}");

    let (code, log, out) = build("told", &[([-3, 0, -2], BARREL)], None, true);
    assert_eq!(code, 0, "{log}");
    let line = census_line(&log);
    assert!(
        line.contains("1 container cell(s) in the assembled world (minecraft:barrel: 1)"),
        "{line}"
    );
    assert!(
        line.contains("0 loot, 0 collect, 0 device, 1 decoration; notice declared"),
        "{line}"
    );
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/containers.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["decoration"], 1);
    assert_eq!(
        ledger["notice"]["text"],
        "The keep was stripped bare before you came."
    );
    assert_eq!(ledger["notice"]["style"], "chat");
    assert_eq!(ledger["notice"]["sound"], "minecraft:block.barrel.close");
    let row = &ledger["rows"][0];
    assert_eq!(row["block"], BARREL);
    assert_eq!(row["slots"], 27);
    assert_eq!(row["place"], "area/keep");
    assert_eq!(row["claims"], serde_json::json!([]));
    assert_eq!(ledger["census_input_sha256"].as_str().unwrap().len(), 64);
    for body in class_apply_bodies(&out) {
        let lines: Vec<&str> = body.lines().collect();
        let classed = lines
            .iter()
            .position(|l| *l == "scoreboard players set @s dw.classed 1")
            .expect("dw.classed line");
        let told = lines
            .iter()
            .position(|l| l.starts_with("tellraw @s") && l.contains("stripped bare"))
            .expect("the notice line");
        let warp = lines
            .iter()
            .position(|l| l.starts_with("teleport @s"))
            .expect("teleport line");
        let sound = lines
            .iter()
            .position(|l| {
                *l == "execute as @s at @s run playsound minecraft:block.barrel.close player @s"
            })
            .expect("the notice's sound, played at the player");
        assert_eq!(
            lines.iter().filter(|l| l.contains("playsound")).count(),
            1,
            "the sound is played once, at the player, and nowhere else: {body}"
        );
        assert!(classed < warp && warp < told && told < sound, "{body}");
    }
}

/// `DW1018`, second arm: a notice over a world whose every container is
/// claimed (here a `collect`'s own chest, counted from its declaration).
#[test]
fn a_notice_over_a_stocked_world_is_dw1018() {
    let (code, log, _) = build("stocked", &[], Some("anchor/exit"), true);
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1018"), "{log}");
    assert!(
        log.contains("Delete `containers.decoration_notice`"),
        "{log}"
    );
    let line = census_line(&log);
    assert!(line.contains("(minecraft:chest: 1)"), "{line}");
    assert!(
        line.contains("0 loot, 1 collect, 0 device, 0 decoration"),
        "{line}"
    );
}

/// `DW1024`, mixed: the collect's chest stands in the same place as a
/// scenery barrel.
#[test]
fn a_container_that_matters_among_scenery_is_dw1024() {
    let (code, log, _) = build("mixed", &[([-3, 0, -2], BARREL)], Some("anchor/exit"), true);
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1024"), "{log}");
    assert!(log.contains("MIXED — `area/keep`"), "{log}");
    assert!(census_line(&log).contains("1 mixed place(s)"), "{log}");
}

/// `DW1024`, storeroom: three scenery cells in one place, over the cap of two.
#[test]
fn a_storeroom_of_nothing_is_dw1024() {
    let (code, log, _) = build(
        "storeroom",
        &[
            ([-3, 0, -2], BARREL),
            ([3, 0, -2], BARREL),
            ([-3, 0, -1], BARREL),
        ],
        None,
        true,
    );
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1024"), "{log}");
    assert!(log.contains("STOREROOM — `area/keep` holds 3"), "{log}");
    assert!(census_line(&log).contains("1 over the cap of 2"), "{log}");
}

fn validate(tag: &str, f: impl FnOnce(&mut serde_json::Value)) -> (i32, String) {
    let dir = scratch(tag);
    let camp = common::container_campaign(&dir.join("camp"), &[], None, false);
    common::patch_file(&camp.join("quests.json"), f);
    let prefabs = common::prefabs_dir();
    let o = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (o.status.code().unwrap_or(-1), said(&o))
}

/// `DW1019`: a `loot` fill with no stack.
#[test]
fn an_empty_loot_fill_is_dw1019() {
    let (code, log) = validate("empty-loot", |q| {
        q["content"]["loot"] =
            serde_json::json!([{ "id": "loot/nothing", "anchor": "anchor/exit", "items": [] }]);
    });
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("DW1019"), "{log}");
    assert!(log.contains("/content/loot/0/items"), "{log}");
}

/// `DW1019`: a `containers` object that declares nothing.
#[test]
fn an_empty_containers_object_is_dw1019() {
    let (code, log) = validate("empty-containers", |q| {
        q["content"]["containers"] = serde_json::json!({});
    });
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("DW1019"), "{log}");
    assert!(log.contains("/content/containers"), "{log}");
}

/// `DW1019`: a runtime `set-block` of a container, reached inside a nested
/// effect list as well as at the top of a bundle.
#[test]
fn a_container_set_at_runtime_is_dw1019() {
    let (code, log) = validate("runtime-chest", |q| {
        let effs = q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        effs.push(serde_json::json!({
            "type": "set-block", "anchor": "anchor/exit", "block": "minecraft:smoker[facing=north]"
        }));
    });
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("DW1019"), "{log}");
    assert!(
        log.contains("places `minecraft:smoker[facing=north]` at runtime"),
        "{log}"
    );
}
