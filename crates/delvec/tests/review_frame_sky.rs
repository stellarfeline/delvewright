//! **Every shot states its world, its clock and its mode, and the sky a frame
//! is drawn under is the engine's** (spec-0112 §7.6, §6.1; criterion 6).

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn json(p: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn build(campaign: &Path, out: &Path) {
    let o = Command::new(BIN)
        .arg("build")
        .arg(campaign)
        .arg("-o")
        .arg(out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// hello-world with a `set-time` to dusk in the talk's completion.
fn hello_dusk_after_talk(dir: &Path) -> PathBuf {
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |v| {
        v["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"type": "set-time", "time": "dusk"}));
    });
    camp
}

/// Every shot carries `world`, `sky` (with `set_by`) and `mode`; a forced
/// `set-time` on step `X` states the new time on every shot standing at or
/// after `X + 1`, and the old one on every shot before.
#[test]
fn a_set_time_states_its_hour_on_every_later_shot() {
    let dir = tmp("rfs-set-time");
    let camp = hello_dusk_after_talk(&dir);
    let out = dir.join("build");
    build(&camp, &out);
    let plan = json(&out.join("render-plan.json"));
    let shots = plan["shots"].as_array().unwrap();
    assert!(!shots.is_empty());
    let (mut after, mut before) = (0, 0);
    for s in shots {
        assert!(s["world"].is_string(), "{s}");
        assert_eq!(s["mode"], "quick", "{s}");
        let sky = &s["sky"];
        assert!(sky.get("set_by").is_some(), "{s}");
        assert!(sky["daytime_ticks"].is_i64(), "{s}");
        if s["world"] == "at-load" {
            before += 1;
            assert_eq!(sky["time"], "noon", "{s}");
            assert!(sky["set_by"].is_null(), "{s}");
        } else {
            after += 1;
            assert_eq!(sky["time"], "dusk", "{s}");
            assert_eq!(sky["daytime_ticks"], 12000, "{s}");
            assert_eq!(sky["set_by"], "obj/talk", "{s}");
        }
    }
    assert!(after > 0 && before > 0, "{after} after, {before} before");
}

/// One sky path: the frame's sky is the engine's (`scene::sky_attributes`
/// over `sky_cell`, the sun from `sun_at`), and the engine calls none of
/// Lumenwright's own sky or celestial functions.
#[test]
fn the_sky_is_the_engines_and_never_the_renderers() {
    let src = common::repo_root().join("crates/delvec/src");
    let frames = std::fs::read_to_string(src.join("compiler/view/frames.rs")).unwrap();
    assert!(
        frames.contains("scene::sky_attributes("),
        "frames.rs reads the engine's sky"
    );
    assert!(
        frames.contains("scene::sun_at("),
        "frames.rs reads the engine's sun"
    );
    let mut stack = vec![src];
    let mut read = 0usize;
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                read += 1;
                let text = std::fs::read_to_string(&p).unwrap();
                for (n, line) in text.lines().enumerate() {
                    if line.contains("lumenwright")
                        && (line.contains("celestial_angle") || line.contains("sky_color"))
                    {
                        panic!("{}:{}: {line}", p.display(), n + 1);
                    }
                }
            }
        }
    }
    eprintln!("sky path: {read} source file(s) read, no call of the renderer's own sky");
    assert!(read > 100, "the walk read {read} files");
}

/// The mean luma (Rec. 709) of one of this engine's PNGs: one `IDAT`, filter 0
/// on every row (`compiler::png`).
fn mean_luma(png: &[u8]) -> f64 {
    use std::io::Read;
    let w = u32::from_be_bytes(png[16..20].try_into().unwrap()) as usize;
    let mut i = 8;
    let mut idat = Vec::new();
    while i + 8 <= png.len() {
        let n = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        if &png[i + 4..i + 8] == b"IDAT" {
            idat.extend_from_slice(&png[i + 8..i + 8 + n]);
        }
        i += 12 + n;
    }
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&idat[..])
        .read_to_end(&mut raw)
        .unwrap();
    let stride = w * 4 + 1;
    let (mut sum, mut n) = (0.0, 0usize);
    for row in raw.chunks(stride) {
        assert_eq!(row[0], 0, "filter 0");
        for p in row[1..].chunks(4) {
            sum += 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]);
            n += 1;
        }
    }
    sum / n as f64
}

/// A camera over hello-world's roof, looking at the horizon, previewed at noon
/// and at midnight: the midnight frame is darker by a factor this test prints
/// and holds above 3 — the shape a renderer drawing its own day over every
/// hour would fail.
#[test]
fn midnight_is_darker_than_noon() {
    let dir = tmp("rfs-luma");
    let built = dir.join("build");
    build(&common::hello_world_dir(), &built);
    let camp = dir.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    std::fs::write(
        camp.join("design.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "stage": "design",
            "content": {"references": [
                {"name": "concept/sky", "shows": "the keep's roof", "time": "dusk", "weather": "clear"}
            ]}
        }))
        .unwrap(),
    )
    .unwrap();
    let mut lumas = Vec::new();
    for time in ["noon", "midnight"] {
        std::fs::create_dir_all(camp.join("design")).unwrap();
        std::fs::write(
            camp.join("design/cameras.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"campaign_id": "hello-world", "cameras": [{
                    "answers": "concept/sky", "fov": 70.0, "height": 96, "name": "sky",
                    "pitch": -10.0, "pos": [5.5, 75.0, 5.5], "source": "estimated", "spp": 4,
                    "width": 96, "yaw": 0.0, "sky": {"time": time, "weather": "clear"}
                }]}),
            )
            .unwrap(),
        )
        .unwrap();
        let out = dir.join(format!("frames-{time}"));
        let o = Command::new(BIN)
            .arg("--prefabs")
            .arg(common::prefabs_dir())
            .arg("cameras")
            .arg(&built)
            .arg("--campaign")
            .arg(&camp)
            .arg("-o")
            .arg(&out)
            .args(["--preview", "--backend", "cpu"])
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let png = std::fs::read(out.join("hello-world_camera_sky_preview.png")).unwrap();
        lumas.push(mean_luma(&png));
    }
    let factor = lumas[0] / lumas[1].max(1e-6);
    eprintln!(
        "sky luma: noon {:.2}, midnight {:.2}, factor {factor:.2}",
        lumas[0], lumas[1]
    );
    assert!(factor > 3.0, "noon {} vs midnight {}", lumas[0], lumas[1]);
}
