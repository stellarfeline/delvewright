//! `common::built` builds a fixture once across processes and again when an
//! input changes (spec-0126 §4.2, acceptance criterion 11).

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

use common::built::{Inputs, built_at, builds, key};

const CHILD: &str = "DW_BUILT_CHILD";

/// This test's own name as the runner lists it, so the probe below can run
/// exactly it in a child process wherever the file is compiled.
fn child_name() -> String {
    match module_path!().split_once("::") {
        Some((_, rest)) => format!("{rest}::built_child"),
        None => "built_child".to_string(),
    }
}

/// A child process's half of the probe: a no-op unless the test below spawned it.
#[test]
fn built_child() {
    let Ok(spec) = std::env::var(CHILD) else {
        return;
    };
    let (cache, key) = spec.split_once('|').unwrap();
    let dir = built_at(Path::new(cache), "probe", key, |d| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::write(d.join("built"), key).unwrap();
    });
    println!("DIR={}", dir.display());
}

fn spawn(cache: &Path, key: &str) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &child_name(), "--nocapture", "--test-threads", "1"])
        .env(CHILD, format!("{}|{key}", cache.display()))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("the test executable runs")
}

fn dir_of(child: std::process::Child) -> PathBuf {
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "the child probe failed");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text
        .lines()
        .find_map(|l| l.strip_prefix("DIR="))
        .unwrap_or_else(|| panic!("the child printed no DIR line:\n{text}"));
    PathBuf::from(line)
}

#[test]
fn two_processes_with_equal_inputs_build_once_and_a_changed_input_builds_again() {
    let cache = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("built-probe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    let a = key(Inputs::new().text("probe input a"));
    let b = key(Inputs::new().text("probe input b"));
    assert_ne!(a, b, "a changed input changes the key");

    let (first, second) = (spawn(&cache, &a), spawn(&cache, &a));
    let (d1, d2) = (dir_of(first), dir_of(second));
    assert_eq!(d1, d2, "two processes with equal inputs read one directory");
    assert_eq!(builds(&cache, "probe", &a), 1, "and it was built once");
    assert_eq!(std::fs::read_to_string(d1.join("built")).unwrap(), a);

    let d3 = dir_of(spawn(&cache, &b));
    assert_ne!(d3, d1, "a changed input is a different fixture");
    assert_eq!(builds(&cache, "probe", &b), 1, "built once for the changed input");
    assert_eq!(builds(&cache, "probe", &a), 1, "and the first was not rebuilt");
    println!(
        "built: 3 calls in 3 processes over 2 inputs, {} builds",
        builds(&cache, "probe", &a) + builds(&cache, "probe", &b)
    );
}
