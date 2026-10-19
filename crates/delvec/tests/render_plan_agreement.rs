//! **Which cameras a campaign has** is one question with one implementation:
//! the render plan `delvec build` emits (`render-plan.json`), derived over the
//! build's final world. `delvec snapshot --shot` is its other consumer, and it
//! reads that plan from the build (`--build`) rather than deriving one of its
//! own. This holds the two to one answer on the gallery primary (whose
//! critical path is the longest any campaign here walks) and on the
//! keep-vertical fixture: a shot the build plans is one the snapshot frames,
//! from the plan's own camera.
//!
//! A snapshot assembles the world it draws, which on the gallery takes tens of
//! seconds a frame, so the snapshot is asked for a cover of the plan rather
//! than all of it: every leg's last POV shot (a leg whose waypoints a second
//! derivation numbered differently ends elsewhere, or is gone) and the first
//! shot of every other kind. The count asked is printed beside the count
//! planned. Asked of every shot, the snapshot that re-derived the plan over a
//! world of its own refused 21 of the gallery's 185 and framed 6 from other
//! cameras; asked of this cover, it refuses the last shot of each of legs 19 to
//! 24.

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
    let planned: Vec<serde_json::Value> = plan["shots"].as_array().cloned().unwrap_or_default();
    let shots = cover(&planned);
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
        "render-plan agreement: {name}: {} shot(s) the build plans, {} asked of the snapshot \
         (every leg's last POV shot, every other kind's first), {same} framed from the plan's \
         camera, {refused} refused, {differ} framed from another",
        planned.len(),
        shots.len()
    );
    (shots.len(), bad)
}

/// Every leg's last POV shot, and the first shot of every other kind, in the
/// plan's order.
fn cover(planned: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut last: std::collections::BTreeMap<String, (u64, serde_json::Value)> =
        std::collections::BTreeMap::new();
    let mut first: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();
    for shot in planned {
        let id = shot["id"].as_str().expect("a shot id");
        let mut parts = id.split('/');
        let kind = parts.next().unwrap_or("").to_string();
        if kind == "pov" {
            let leg = parts.next().unwrap_or("").to_string();
            let wp: u64 = parts
                .next()
                .and_then(|w| w.strip_prefix("wp"))
                .and_then(|w| w.parse().ok())
                .expect("a POV id names its waypoint");
            if last.get(&leg).is_none_or(|(w, _)| wp > *w) {
                last.insert(leg, (wp, shot.clone()));
            }
        } else {
            first.entry(kind).or_insert_with(|| shot.clone());
        }
    }
    last.into_values()
        .map(|(_, s)| s)
        .chain(first.into_values())
        .collect()
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
fn a_shot_the_build_plans_is_the_shot_the_snapshot_frames() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("render-plan-agreement");
    let gallery = root.join("gallery");
    let gallery_prefabs = common::gallery_primary_at(&gallery);
    let keep = root.join("keep-vertical");
    let _ = std::fs::remove_dir_all(&keep);
    common::copy_dir_all(&common::keep_vertical_dir(), &keep);
    let mut asked = 0;
    let mut bad = Vec::new();
    for (name, campaign, prefabs) in [
        ("the gallery primary", &gallery, gallery_prefabs.clone()),
        ("keep-vertical", &keep, common::prefabs_dir()),
    ] {
        let (n, b) = agree(name, campaign, &prefabs);
        assert!(n > 0, "{name}: no shot was asked, so nothing was compared");
        asked += n;
        bad.extend(
            b.into_iter()
                .map(|(id, why)| format!("{name}: {id}: {why}")),
        );
    }
    assert!(
        bad.is_empty(),
        "of {asked} shot(s) asked of the snapshot, {} are not framed from the plan's camera:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
