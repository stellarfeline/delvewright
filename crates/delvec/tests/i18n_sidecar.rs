//! What `tools/i18n-translate.py` writes is in canonical form.
//!
//! `delvec fmt --check` refused **every** sidecar that tool had ever produced —
//! `DW0773`, an error tier — because the writer laid the envelope out in the
//! order it happened to build the dict and carried an existing sidecar's
//! `dsl_version` forward, while canonical form sorts every object's keys and
//! `delvec fmt` stamps the version this engine implements (ADR-0024). Two
//! authorities for one canonical form, and only a hand-run `delvec fmt` ever
//! reconciled them.
//!
//! The repair is that the tool has stopped being the second authority: it hands
//! the finished file to `delvec fmt`. This test is what binds that, and it is
//! deliberately not a string comparison inside the Python suite — that suite
//! has no compiler, so it could only ever assert what the writer *intended*.
//! Here the real writer writes a real file and the real formatter judges it.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn repo_root() -> PathBuf {
    // crates/delvec -> crates -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root above crates/delvec")
        .to_path_buf()
}

#[test]
fn the_sidecar_the_translation_tool_writes_is_canonical() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("i18n-sidecar");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let out = dir.join("zh-cn.json");

    let driver = repo_root().join("tools/tests/fixtures/i18n-sidecar/write_sidecar.py");
    assert!(driver.is_file(), "driver missing: {}", driver.display());

    let written = Command::new("python3")
        .arg(&driver)
        .arg(&out)
        .arg(BIN)
        .output()
        .expect("run the sidecar writer");
    assert!(
        written.status.success(),
        "the writer failed:\n{}{}",
        String::from_utf8_lossy(&written.stdout),
        String::from_utf8_lossy(&written.stderr)
    );
    assert!(out.is_file(), "no sidecar was written");

    // The binding: the formatter is the authority, and it is asked about the
    // bytes on disk, not about a string the test built.
    let checked = Command::new(BIN)
        .args(["fmt", "--check"])
        .arg(&out)
        .output()
        .expect("run delvec fmt --check");
    assert_eq!(
        checked.status.code().unwrap_or(-1),
        0,
        "the tool wrote a sidecar its own formatter refuses:\n{}{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );

    // And the two specific disagreements are named, so a reader can see which
    // properties this is protecting rather than only that some check is green.
    let text = std::fs::read_to_string(&out).expect("read the sidecar");
    let keys: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("  \""))
        .filter_map(|l| l.split('"').next())
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "envelope keys are not in canonical order");
    assert!(
        text.contains(&format!(
            "\"dsl_version\": \"{}\"",
            delvewright_dsl::DSL_VERSION
        )),
        "the sidecar declares a dsl_version this engine does not implement:\n{text}"
    );
}
