//! **Which cameras a campaign has** is one question with one implementation:
//! the render plan `delvec build` emits (`render-plan.json`), derived over the
//! build's final world. `delvec snapshot --shot` is its other consumer, and it
//! reads that plan from the build (`--build`) rather than deriving one of its
//! own. This holds the two to one answer: every shot the build plans is one the
//! snapshot frames, from the plan's own camera, on the gallery primary (whose
//! critical path is the longest any campaign here walks) and on the
//! keep-vertical fixture. A snapshot that re-derived the plan over a world of
//! its own numbered the gallery's later legs differently and refused 21 of the
//! build's shots.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// Build `campaign` against `prefabs` into a fresh tree beside it.
fn build(campaign: &Path, prefabs: &Path) -> PathBuf {
    let out = campaign.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let o = Command::new(BIN)
        .arg("build")
        .arg(campaign)
        .arg("--out")
        .arg(&out)
        .arg("--prefabs")
        .arg(prefabs)
        .output()
        .expect("delvec runs");
    assert!(
        o.status.success(),
        "{} does not build:\n{}",
        campaign.display(),
        String::from_utf8_lossy(&o.stderr)
    );
    out
}

/// What one consumer said about one shot.
enum Verdict {
    Same,
    Refused(String),
    Differs(String),
}

/// Ask the snapshot for every shot of the build's plan and compare the camera
/// it frames with the plan's: position to the manifest's three decimals, and
/// the aim the plan's `look_at` gives.
fn agree(name: &str, campaign: &Path, prefabs: &Path) -> (usize, Vec<(String, String)>) {
    let out = build(campaign, prefabs);
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("render-plan.json")).unwrap()).unwrap();
    let shots: Vec<serde_json::Value> = plan["shots"].as_array().cloned().unwrap_or_default();
    let frames = campaign.with_extension("frames");
    let _ = std::fs::remove_dir_all(&frames);
    std::fs::create_dir_all(&frames).unwrap();
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = shots.len().div_ceil(workers.max(1)).max(1);
    let verdicts: Vec<(String, Verdict)> = std::thread::scope(|scope| {
        let handles: Vec<_> = shots
            .chunks(chunk)
            .map(|part| {
                let (out, frames) = (&out, &frames);
                scope.spawn(move || {
                    part.iter()
                        .map(|shot| {
                            let id = shot["id"].as_str().unwrap().to_string();
                            (id.clone(), ask(campaign, prefabs, out, frames, &id, shot))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a worker finishes"))
            .collect()
    });
    let same = verdicts
        .iter()
        .filter(|(_, v)| matches!(v, Verdict::Same))
        .count();
    let mut bad = Vec::new();
    let (mut refused, mut differ) = (0, 0);
    for (id, v) in verdicts {
        match v {
            Verdict::Same => {}
            Verdict::Refused(why) => {
                refused += 1;
                bad.push((id, format!("refused: {why}")));
            }
            Verdict::Differs(why) => {
                differ += 1;
                bad.push((id, format!("differs: {why}")));
            }
        }
    }
    eprintln!(
        "render-plan agreement: {name}: {} shot(s) the build plans, {same} framed from the \
         plan's camera, {refused} refused by the snapshot, {differ} framed from another",
        shots.len()
    );
    (shots.len(), bad)
}

fn ask(
    campaign: &Path,
    prefabs: &Path,
    out: &Path,
    frames: &Path,
    id: &str,
    shot: &serde_json::Value,
) -> Verdict {
    let png = frames.join(format!("{}.png", id.replace('/', "_")));
    let o = Command::new(BIN)
        .arg("--prefabs")
        .arg(prefabs)
        .arg("snapshot")
        .arg(campaign)
        .arg("--shot")
        .arg(id)
        .arg("--build")
        .arg(out)
        .arg("-o")
        .arg(&png)
        .args(["--width", "32", "--height", "18"])
        .output()
        .expect("delvec runs");
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        return Verdict::Refused(err.lines().last().unwrap_or("").chars().take(160).collect());
    }
    let m: serde_json::Value =
        serde_json::from_slice(&std::fs::read(png.with_extension("manifest.json")).unwrap())
            .unwrap();
    let read3 = |v: &serde_json::Value| -> [f64; 3] {
        [0, 1, 2].map(|i| v[i].as_f64().expect("a coordinate"))
    };
    let pos = read3(&shot["camera"]["pos"]);
    let look = read3(&shot["camera"]["look_at"]);
    let fov = shot["camera"]["fov"]
        .as_f64()
        .unwrap_or(delvec::compiler::snapshot::DEFAULT_FOV);
    let want = delvec::compiler::snapshot::Camera::looking_at(pos, look, fov);
    let got = &m["camera"];
    let near = |a: f64, b: &serde_json::Value| b.as_f64().is_some_and(|b| (a - b).abs() < 2e-3);
    let same = (0..3).all(|i| near(want.pos[i], &got["pos"][i]))
        && near(want.yaw, &got["yaw"])
        && near(want.pitch, &got["pitch"]);
    if same {
        Verdict::Same
    } else {
        Verdict::Differs(format!(
            "plan pos {:?} yaw {:.3} pitch {:.3}, snapshot {}",
            want.pos, want.yaw, want.pitch, got
        ))
    }
}

#[test]
fn every_shot_the_build_plans_is_the_shot_the_snapshot_frames() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("render-plan-agreement");
    let gallery = root.join("gallery");
    let gallery_prefabs = common::gallery_primary_at(&gallery);
    let keep = root.join("keep-vertical");
    let _ = std::fs::remove_dir_all(&keep);
    common::copy_dir_all(&common::keep_vertical_dir(), &keep);
    let mut planned = 0;
    let mut bad = Vec::new();
    for (name, campaign, prefabs) in [
        ("the gallery primary", &gallery, gallery_prefabs.clone()),
        ("keep-vertical", &keep, common::prefabs_dir()),
    ] {
        let (n, b) = agree(name, campaign, &prefabs);
        assert!(
            n > 0,
            "{name}: the build plans no shot, so nothing was compared"
        );
        planned += n;
        bad.extend(
            b.into_iter()
                .map(|(id, why)| format!("{name}: {id}: {why}")),
        );
    }
    assert!(
        bad.is_empty(),
        "of {planned} planned shot(s), {} are not framed from the plan's camera:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
