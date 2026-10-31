//! **`delvec frames --campaign --program <place>` draws a place from its
//! program as it stands** (spec-0128 §10): the program expanded at its frame as
//! `detail` expands it, in place of the committed piece, its gates passing or
//! not, and nothing of the campaign or the library written.
//!
//! The frame itself needs the pinned client's textures, which this test does
//! not ask for: the run is pointed at none, so it stops at the draw — after the
//! world it would draw is written. What is compared is that world: with
//! `--program`, an edit to the program that no commit has taken shows in it;
//! without, it does not.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::json;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-frames-program-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Every file under `dir`, with its bytes' hash.
fn snapshot(dir: &Path) -> BTreeSet<(String, String)> {
    use sha2::{Digest, Sha256};
    let mut out = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let h = format!("{:x}", Sha256::digest(std::fs::read(&p).unwrap()));
                out.insert((p.strip_prefix(dir).unwrap().display().to_string(), h));
            }
        }
    }
    out
}

/// `frames --campaign` with no textures anywhere, so it stops at the draw;
/// returns the run and the world it wrote.
fn frame(
    campaign: &Path,
    prefabs: &Path,
    out: &Path,
    extra: &[&str],
) -> (Output, BTreeSet<(String, String)>) {
    let home = out.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let run = std::process::Command::new(BIN)
        .env("HOME", &home)
        .env_remove("DELVEWRIGHT_CLIENT_JAR")
        .arg("--prefabs")
        .arg(prefabs)
        .arg("frames")
        .arg("--campaign")
        .arg(campaign)
        .args(extra)
        .arg("--textures")
        .arg(out.join("no-such-jar.jar"))
        .arg("-o")
        .arg(out.join("frame.png"))
        .output()
        .expect("run delvec");
    let world = out.join("world");
    assert!(world.is_dir(), "the world is written first: {}", text(&run));
    (run, snapshot(&world))
}

#[test]
fn a_place_is_drawn_from_its_program_as_it_stands_and_nothing_is_written() {
    let tmp = tempdir("look");
    let (campaign, prefabs) = common::gallery_site_plan_detailed(&tmp);
    // An edit no commit has taken: the annex's floor laid in gold.
    common::patch_file(&campaign.join("programs/annex.json"), |v| {
        v["palette"]["floor"] = json!("minecraft:gold_block");
    });
    let before = (snapshot(&campaign), snapshot(&prefabs));

    let (plain, committed) = frame(&campaign, &prefabs, &tmp.join("plain"), &[]);
    assert!(!text(&plain).contains("drawn from"), "{}", text(&plain));
    let (look, looked) = frame(
        &campaign,
        &prefabs,
        &tmp.join("look"),
        &["--program", "node/annex", "--at", "node/annex"],
    );
    let said = text(&look);
    assert!(
        said.contains("node/annex: drawn from") && said.contains("as it stands"),
        "{said}"
    );
    assert_ne!(
        committed, looked,
        "the edited program shows in the world drawn with --program, and not without"
    );
    assert_eq!(
        before,
        (snapshot(&campaign), snapshot(&prefabs)),
        "--program writes nothing to the campaign or the library"
    );

    // A program whose own gates refuse is still drawn, each refusal printed.
    common::patch_file(&campaign.join("programs/annex.json"), |v| {
        v["palette"]["floor"] = json!("minecraft:candle[candles=1,lit=false,waterlogged=false]");
    });
    let (broken, _) = frame(
        &campaign,
        &prefabs,
        &tmp.join("broken"),
        &["--program", "node/annex"],
    );
    assert!(
        text(&broken).contains("node/annex: drawn from"),
        "{}",
        text(&broken)
    );
}
