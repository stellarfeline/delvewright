//! Shared test helpers: locate fixtures and materialize a campaign directory
//! from the DSL's patch-style invalid fixtures (base hello-world + overrides).
//!
//! Compiled once per integration-test binary; not every helper is used by each,
//! so unused-warnings here are expected.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The five stage filenames (matching `delvewright_compiler::load::STAGE_FILES`).
pub const STAGE_FILES: [&str; 5] = [
    "world.json",
    "npcs.json",
    "classes.json",
    "quest-plan.json",
    "quests.json",
];

/// Repo root (two levels up from `crates/compiler`).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

/// The canonical valid hello-world campaign directory.
pub fn hello_world_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/hello-world")
}

/// The repo `prefabs/` directory.
pub fn prefabs_dir() -> PathBuf {
    repo_root().join("prefabs")
}

/// The DSL invalid-fixture directory (patch files).
pub fn dsl_invalid_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/invalid")
}

/// This crate's own test fixtures.
pub fn compiler_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Materialize a full campaign directory at `dst` = the valid hello-world base
/// with each stage in `patch["documents"]` overwritten by its replacement
/// envelope. Returns nothing; panics on IO error (tests).
pub fn materialize(patch: &serde_json::Value, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    let base = hello_world_dir();
    for f in STAGE_FILES {
        std::fs::copy(base.join(f), dst.join(f)).unwrap();
    }
    if let Some(docs) = patch.get("documents").and_then(|d| d.as_object()) {
        for (stage, doc) in docs {
            let file = dst.join(format!("{stage}.json"));
            let text = serde_json::to_string_pretty(doc).unwrap();
            std::fs::write(file, text).unwrap();
        }
    }
}
