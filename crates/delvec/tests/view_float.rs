//! A site-plan view stands where a body's eye stands (spec-0122 §7.3): `eye`
//! and `look_at` take floats, the far-view proof (`DW0956`) measures them, the
//! handed `delvec frames` invocation draws the float pose, and a whole-number
//! view keeps its bytes through `delvec fmt` and through the typed writer.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, parse_campaign, validate_campaign_with, viewdistance};

const EYE: [f64; 3] = [54.5, 85.62, 101.5];

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn log(o: &Output) -> String {
    format!(
        "exit {:?}\n{}{}",
        o.status.code(),
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// The blockout fixture, copied, with its view's eye at a half block and a
/// body's eye height.
fn half_block_campaign(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("view-float-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::compiler_fixtures_dir().join("blockout"), &dir);
    common::patch_file(&dir.join("site-plan.json"), |p| {
        p["content"]["views"][0]["eye"] = serde_json::json!(EYE);
    });
    dir
}

fn parsed(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).unwrap();
    parse_campaign(&loaded.raw).expect("the campaign parses")
}

fn codes(c: &Campaign) -> Vec<String> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .map(|d| d.code.to_string())
    .collect()
}

#[test]
fn a_half_block_eye_validates_and_is_judged_by_the_far_view_proof() {
    let dir = half_block_campaign("judged");
    let r = delvec(&[
        "validate",
        dir.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(0), "{}", log(&r));

    let mut c = parsed(&dir);
    let view = &c.site_plan.as_ref().unwrap().content.views[0];
    assert_eq!(view.eye, EYE);
    let b = viewdistance::checks(&c, &mut Vec::new());
    assert_eq!((b.views, b.beyond), (1, 0), "the view is judged and near");
    assert!(!codes(&c).contains(&"DW0956".to_string()));

    // Moved 400.25 blocks further along z, the proof measures the float length.
    let view = &mut c.site_plan.as_mut().unwrap().content.views[0];
    view.look_at = [EYE[0], EYE[1], EYE[2] + 400.25];
    let mut d = Vec::new();
    let b = viewdistance::checks(&c, &mut d);
    assert_eq!((b.views, b.beyond), (1, 1));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0956" && x.message.contains("reaches 400.2 blocks")),
        "{d:?}"
    );
}

#[test]
fn the_handed_frame_draws_the_float_pose() {
    let dir = half_block_campaign("render");
    let r = delvec(&["allocation", dir.to_str().unwrap(), "--all"]);
    assert_eq!(r.status.code(), Some(0), "{}", log(&r));
    let stdout = String::from_utf8_lossy(&r.stdout).to_string();
    let needle = "--camera 54.5,85.62,101.5,";
    let at = stdout
        .find(needle)
        .unwrap_or_else(|| panic!("no place is handed the half-block view: {stdout}"));
    let pose: String = stdout[at + "--camera ".len()..]
        .chars()
        .take_while(|c| *c != '"' && !c.is_whitespace())
        .collect();
    let png = dir.join("view.png");
    let r = delvec(&[
        "frames",
        "--campaign",
        dir.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
        "-o",
        png.to_str().unwrap(),
        "--size",
        "96",
        "--backend",
        "cpu",
        "--camera",
        &pose,
    ]);
    assert_eq!(r.status.code(), Some(0), "{pose}: {}", log(&r));
    let m: serde_json::Value =
        serde_json::from_slice(&std::fs::read(png.with_extension("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(m["camera"]["pos"], serde_json::json!(EYE), "{m}");
}

#[test]
fn whole_number_views_keep_their_bytes() {
    let src = common::compiler_fixtures_dir().join("blockout/site-plan.json");
    let text = std::fs::read_to_string(&src).unwrap();
    assert!(
        text.contains("\"eye\": [\n          32,"),
        "fixture as authored"
    );
    // `delvec fmt --check` passes over it unchanged.
    let r = delvec(&["fmt", "--check", src.to_str().unwrap()]);
    assert_eq!(r.status.code(), Some(0), "{}", log(&r));
    // The typed writer writes the same bytes.
    let env: delvewright_dsl::Envelope<delvewright_dsl::siteplan::SitePlanContent> =
        serde_json::from_str(&text).unwrap();
    assert_eq!(delvewright_dsl::to_canonical_string(&env).unwrap(), text);
    // And a half-block view keeps its own spelling through both.
    let dir = half_block_campaign("fmt");
    let half = dir.join("site-plan.json");
    let r = delvec(&["fmt", "--check", half.to_str().unwrap()]);
    assert_eq!(r.status.code(), Some(0), "{}", log(&r));
    let text = std::fs::read_to_string(&half).unwrap();
    assert!(
        text.contains("54.5,\n          85.62,\n          101.5"),
        "{text}"
    );
    let env: delvewright_dsl::Envelope<delvewright_dsl::siteplan::SitePlanContent> =
        serde_json::from_str(&text).unwrap();
    assert_eq!(delvewright_dsl::to_canonical_string(&env).unwrap(), text);
}
