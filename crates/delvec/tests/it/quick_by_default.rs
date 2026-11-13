//! **Every picture drawn while working is a quick frame; a path-traced picture
//! is asked for by name.** The two verbs that path trace — `delvec cameras` and
//! `delvec panorama` — draw quick with no flag, and `--hq` draws exactly what
//! the verb drew with no flag before quick became the default: its frame
//! manifests are compared byte for byte with the ones that default wrote over
//! the same build (`tests/fixtures/view/hq-by-name/`), the engine's version
//! the only field set aside. Every frame is drawn on the CPU backend.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// The manifests the old default (no flag) wrote, `"delvec"` set to
/// [`VERSION_SLOT`].
const FIXTURE: &str = "crates/delvec/tests/fixtures/view/hq-by-name";
const VERSION_SLOT: &str = "\"delvec\": \"<delvec>\"";

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delvec-quick-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn log(r: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    )
}

/// hello-world built into `<dir>/build`, and a copy of it carrying two approved
/// pictures and a camera for each, at `<dir>/campaign`.
fn fixture(dir: &Path) -> (PathBuf, PathBuf) {
    let built = dir.join("build");
    let r = Command::new(BIN)
        .arg("build")
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&built)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", log(&r));
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    std::fs::create_dir_all(camp.join("design")).unwrap();
    std::fs::write(
        camp.join("design.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "stage": "design",
            "content": {"references": [
                {"name": "concept/gate", "shows": "the gate", "time": "dusk", "weather": "clear"},
                {"name": "concept/hall", "shows": "the hall", "time": "dusk", "weather": "clear"}
            ]}
        }))
        .unwrap(),
    )
    .unwrap();
    let camera = |name: &str, answers: &str| {
        serde_json::json!({
            "answers": answers, "fov": 55.0, "height": 90, "name": name,
            "pitch": 12.0, "pos": [9.5, 72.0, -6.0], "source": "estimated", "spp": 4,
            "width": 160, "yaw": 20.0
        })
    };
    std::fs::write(
        camp.join("design/cameras.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "campaign_id": "hello-world",
            "cameras": [camera("gate", "concept/gate"), camera("hall", "concept/hall")]
        }))
        .unwrap(),
    )
    .unwrap();
    (built, camp)
}

fn cameras(built: &Path, camp: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(BIN)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .arg("cameras")
        .arg(built)
        .arg("--campaign")
        .arg(camp)
        .arg("-o")
        .arg(out)
        .args(["--backend", "cpu"])
        .args(extra)
        .output()
        .unwrap()
}

fn panorama(built: &Path, out: &Path, extra: &[&str]) -> Output {
    Command::new(BIN)
        .arg("panorama")
        .arg(built)
        .arg("-o")
        .arg(out)
        .args(["--width", "64", "--height", "36", "--backend", "cpu"])
        .args(extra)
        .output()
        .unwrap()
}

fn manifest(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// A manifest's bytes with the engine's version set to [`VERSION_SLOT`]:
/// asserted present exactly once, so a manifest that lost the field reds here.
fn versionless(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let field = format!("\"delvec\": \"{}\"", delvec::compiler::DELVEC_VERSION);
    assert_eq!(
        text.matches(&field).count(),
        1,
        "{}: {text}",
        path.display()
    );
    text.replace(&field, VERSION_SLOT)
}

/// **With no flag, both verbs draw quick**: the camera frames are the quick
/// frames at half the stated size, as `<stem>_preview.png`; the panorama is a
/// quick frame as `<stem>_preview.png`, with no samples; no path-traced
/// picture is written and no run says it spends samples. `--spp` without
/// `--hq`, and `--hq` beside `--draft`, are refused at the flag.
#[test]
fn a_showcase_verb_with_no_flag_draws_quick() {
    let dir = tmp("default");
    let (built, camp) = fixture(&dir);

    let out = dir.join("cameras");
    let r = cameras(&built, &camp, &out, &[]);
    let said = log(&r);
    assert_eq!(r.status.code(), Some(0), "{said}");
    let mut drawn = 0;
    for name in ["gate", "hall"] {
        let m = manifest(&out.join(format!("hello-world_camera_{name}_preview.manifest.json")));
        assert_eq!(m["mode"], "quick", "{name}: {m}");
        assert_eq!(
            (m["image"]["width"].as_u64(), m["image"]["height"].as_u64()),
            (Some(80), Some(45)),
            "{name}: {m}"
        );
        assert!(
            !out.join(format!("hello-world_camera_{name}.png")).exists(),
            "{name}: a path-traced frame was written with no flag"
        );
        drawn += 1;
    }
    assert_eq!(drawn, 2);
    assert!(!said.contains("path traced at"), "{said}");
    assert!(said.contains("previewed 2 camera frame(s)"), "{said}");
    assert!(
        said.contains("`--hq` path traces the delivered pictures"),
        "{said}"
    );

    let out = dir.join("panorama");
    let r = panorama(&built, &out, &[]);
    let said = log(&r);
    assert_eq!(r.status.code(), Some(0), "{said}");
    let m = manifest(&out.join("hello-world_panorama_se_45_preview.manifest.json"));
    assert_eq!(m["mode"], "quick", "{m}");
    assert_eq!(m["spp"], serde_json::Value::Null, "{m}");
    assert_eq!(m["image"]["path"], "hello-world_panorama_se_45_preview.png");
    assert!(out.join("hello-world_panorama_se_45_preview.png").is_file());
    assert!(!out.join("hello-world_panorama_se_45.png").exists());
    assert!(!said.contains("path traced at"), "{said}");

    for (verb, r) in [
        (
            "panorama --spp",
            panorama(&built, &dir.join("refused-spp"), &["--spp", "1"]),
        ),
        (
            "cameras --hq --draft",
            cameras(
                &built,
                &camp,
                &dir.join("refused-draft"),
                &["--hq", "--draft"],
            ),
        ),
    ] {
        assert_eq!(r.status.code(), Some(2), "{verb}: {}", log(&r));
    }
}

/// **`--hq` draws what no flag drew before quick was the default**, and says
/// before it draws that it is the delivery mode, with the frame count and the
/// samples it will spend: each frame manifest is byte-identical to the old
/// default's over the same build, the engine's version set aside.
#[test]
fn hq_by_name_draws_what_the_old_default_drew() {
    let dir = tmp("hq");
    let (built, camp) = fixture(&dir);
    let fixtures = common::repo_root().join(FIXTURE);

    let out = dir.join("cameras");
    let r = cameras(&built, &camp, &out, &["--hq"]);
    let said = log(&r);
    assert_eq!(r.status.code(), Some(0), "{said}");
    // 2 frames of 160 x 90 at 4 samples: 115200.
    assert!(
        said.contains(
            "cameras: hq, the delivery mode: 2 frame(s) path traced at 4 spp each, 115200 \
             samples in all"
        ),
        "{said}"
    );
    let mut compared = 0;
    for name in ["gate", "hall"] {
        let file = format!("hello-world_camera_{name}.manifest.json");
        assert!(out.join(format!("hello-world_camera_{name}.png")).is_file());
        assert_eq!(
            versionless(&out.join(&file)),
            std::fs::read_to_string(fixtures.join(&file)).unwrap(),
            "{file}"
        );
        compared += 1;
    }

    let out = dir.join("panorama");
    let r = panorama(&built, &out, &["--hq", "--spp", "1"]);
    let said = log(&r);
    assert_eq!(r.status.code(), Some(0), "{said}");
    // 64 x 36 at 1 sample: 2304.
    assert!(
        said.contains(
            "panorama: hq, the delivery mode: 1 frame(s) path traced at 1 spp each, 2304 samples \
             in all"
        ),
        "{said}"
    );
    let file = "hello-world_panorama_se_45.manifest.json";
    assert!(out.join("hello-world_panorama_se_45.png").is_file());
    assert_eq!(
        versionless(&out.join(file)),
        std::fs::read_to_string(fixtures.join(file)).unwrap(),
        "{file}"
    );
    compared += 1;
    assert_eq!(compared, 3, "every manifest of the fixture is compared");
    let held = std::fs::read_dir(&fixtures).unwrap().count();
    assert_eq!(held, compared, "a fixture manifest no run compares");
}
