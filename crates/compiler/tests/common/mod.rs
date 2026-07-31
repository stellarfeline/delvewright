//! Shared test helpers: locate fixtures and materialize a campaign directory
//! from the DSL's patch-style invalid fixtures (base hello-world + overrides).
//!
//! Compiled once per integration-test binary; not every helper is used by each,
//! so unused-warnings here are expected.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The six stage filenames (matching `delvewright_compiler::load::STAGE_FILES`).
pub const STAGE_FILES: [&str; 6] = [
    "world.json",
    "npcs.json",
    "classes.json",
    "quest-plan.json",
    "quests.json",
    "dialogue.json",
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

/// The multi-area / multi-piece keep-crawl campaign directory (M2 task #9).
pub fn keep_crawl_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/keep-crawl")
}

/// The v0.3 branching keep-trial campaign directory (all gameplay verbs).
pub fn keep_trial_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/keep-trial")
}

/// The v0.3 vertical keep-vertical campaign directory (3D stair layout).
pub fn keep_vertical_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/keep-vertical")
}

/// Materialize a full campaign directory at `dst` = the campaign in `base` with
/// each stage in `patch["documents"]` overwritten by its replacement envelope. Any
/// `l10n/` sidecar directory in `base` is copied verbatim (i18n coverage must hold
/// for a materialized campaign that declares languages).
pub fn materialize_from(base: &Path, patch: &serde_json::Value, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for f in STAGE_FILES {
        std::fs::copy(base.join(f), dst.join(f)).unwrap();
    }
    copy_l10n_dir(base, dst);
    if let Some(docs) = patch.get("documents").and_then(|d| d.as_object()) {
        for (stage, doc) in docs {
            let file = dst.join(format!("{stage}.json"));
            let text = serde_json::to_string_pretty(doc).unwrap();
            std::fs::write(file, text).unwrap();
        }
    }
}

/// Strip any i18n from a materialized campaign at `dir`: drop `world.languages`
/// and remove the `l10n/` directory. Used by build/solver fixtures derived from a
/// language-declaring campaign (e.g. keep-trial) that alter player-visible strings
/// and so would otherwise fail l10n coverage — they test build behavior, not i18n.
pub fn make_english_only(dir: &Path) {
    let world_path = dir.join("world.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&world_path).unwrap()).unwrap();
    if let Some(content) = world.get_mut("content").and_then(|c| c.as_object_mut()) {
        content.remove("languages");
    }
    std::fs::write(&world_path, serde_json::to_string_pretty(&world).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(dir.join("l10n"));
}

/// Copy `base/l10n/*.json` into `dst/l10n/` if the source directory exists.
pub fn copy_l10n_dir(base: &Path, dst: &Path) {
    let src = base.join("l10n");
    if !src.is_dir() {
        return;
    }
    let dst_l10n = dst.join("l10n");
    std::fs::create_dir_all(&dst_l10n).unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            std::fs::copy(&path, dst_l10n.join(path.file_name().unwrap())).unwrap();
        }
    }
}

/// The prefab library directory. The library lives in the content repo
/// (`delvewright-campaigns`), reached at `campaigns/prefabs` — the `campaigns/`
/// symlink locally, and a content-repo checkout at that path in CI (spec-0007
/// Step 0). Mirrors the compiler's default `--prefabs campaigns/prefabs`.
pub fn prefabs_dir() -> PathBuf {
    repo_root().join("campaigns/prefabs")
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
