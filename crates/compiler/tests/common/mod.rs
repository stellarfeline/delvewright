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

/// The v0.6 cutscene fixture (`cutscene-shots`): hello-world's world and cast
/// with a two-shot `cutscene` on its exit beat. The campaign the spec-0019
/// tier-3 flow (`validation/rehearsal-flow.sh`) plays, kept here so tier 1
/// fails first if the fixture ever stops producing the proposal that flow
/// asserts against.
pub fn cutscene_shots_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/cutscene-shots")
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
///
/// **A fixture may only bind prefabs that exist at the PINNED content SHA**
/// (`versions.toml` `[content].sha`, which `.github/actions/checkout-content`
/// checks out). The local `campaigns/` symlink usually points at a working
/// checkout that is far AHEAD of the pin, so a fixture written against a newer
/// prefab passes locally and fails CI with `DW0300` ("no matching prefab
/// metadata") — the classic works-on-my-machine shape, and the reason this note
/// exists. To reproduce CI exactly, point the symlink at a clone checked out at
/// the pinned SHA. Bumping the pin to make a fixture build is a content-repo
/// decision, never a fix for a test.
///
/// # This asserts the library PARSES, and that is not decoration
///
/// The note above documented the hazard and the hazard kept happening — three
/// separate rounds lost to it on 2026-08-08 alone — because the failure does
/// not look like what it is. `PrefabRegistry::load_dir` reports a metadata file
/// this `delvec` cannot parse as `DW0346` in `load_diagnostics()`, and **the
/// CLI drains that list** (`main::validate_loaded`) so `delvec` users get the
/// real message. Integration tests build a `Plan` directly and never drain it,
/// so the prefab is simply absent from the registry and the first thing anyone
/// sees is `DW0300` "no matching prefab metadata" — a message that then states,
/// confidently and wrongly, "this is a prefab-library/naming issue".
///
/// The live case is an engine/content pair mid-flight: `PrefabMeta` is
/// `deny_unknown_fields`, so an engine that predates a metadata field drops
/// **every** prefab carrying it. Thirty-seven files, silently, reported as a
/// naming problem.
///
/// So this checks it once and says what actually happened. Docs are the weakest
/// form a lesson can take (CLAUDE.md debug doctrine); a tooling default that
/// makes the pitfall impossible is stronger, and this is the one place all 72
/// call sites already go through.
pub fn prefabs_dir() -> PathBuf {
    static CHECKED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let dir = repo_root().join("campaigns/prefabs");
    CHECKED.get_or_init(|| {
        let Ok(reg) = delvewright_compiler::registry::PrefabRegistry::load_dir(&dir) else {
            // An unreadable directory is the caller's own problem and every call
            // site already fails clearly on it; only the PARSE case impersonates
            // something else.
            return;
        };
        let diags = reg.load_diagnostics();
        assert!(
            diags.is_empty(),
            "the prefab library at {} has {} file(s) this delvec cannot parse, so those \
             prefabs are ABSENT from the registry and every fixture binding one will fail \
             as DW0300 \"no matching prefab metadata\" — which is not what went wrong.\n\n\
             Almost always: the `campaigns/` symlink points at a content checkout NEWER \
             than this engine (PrefabMeta is deny_unknown_fields, so one unknown field \
             drops the whole file). Point it at the SHA `versions.toml` [content].sha \
             pins, which is what CI builds against.\n\n{}",
            dir.display(),
            diags.len(),
            diags
                .iter()
                .map(|d| format!("  {} {}", d.code, d.message))
                .collect::<Vec<_>>()
                .join("\n")
        );
    });
    dir
}

/// The DSL invalid-fixture directory (patch files).
pub fn dsl_invalid_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/invalid")
}

/// This crate's own test fixtures.
pub fn compiler_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Recursively copy a directory tree (used to make a private, mutable copy of
/// `prefabs_dir()` for tests that corrupt prefab metadata/structures — the real
/// `campaigns/prefabs` is a checkout of the separate content repo and must never
/// be written to by a test).
pub fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        let to = dst.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir_all(&path, &to);
        } else {
            std::fs::copy(&path, &to).unwrap();
        }
    }
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

/// The build-input map for a campaign directory: the stage documents and, under
/// i18n v2 (spec-0029), every `l10n/<code>.json` sidecar — the resource pack now
/// carries a lang file per declared language, so a build of a multilingual
/// campaign that is handed no sidecars fails (`DW0180`) rather than silently
/// shipping one language. A test that builds a campaign declaring `languages`
/// must pass this instead of an empty map.
pub fn campaign_inputs(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    delvewright_compiler::load::load_campaign_dir(dir)
        .expect("campaign dir loads")
        .inputs
}

/// Patch a JSON document **structurally**: parse the text, hand the closure the
/// parsed value, and return it re-rendered in canonical form.
///
/// Tests used to splice fixtures with `str::replace` over exact indented text.
/// That coupling is invisible when it breaks: `str::replace` matching nothing
/// returns the input unchanged, so the test goes on to assert against an
/// **unpatched** campaign and passes for the wrong reason. Reformatting every
/// fixture into canonical form (task #52) exposed four such silent no-ops at
/// once — including the `DW0307` unroutable-move test, which had been asserting
/// against a campaign with no `move-npc` in it. A structural patch cannot miss:
/// an absent key is a panic, not a quiet pass.
pub fn patch_doc(text: &str, f: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(text).expect("fixture is valid JSON");
    f(&mut v);
    delvewright_dsl::to_canonical_string(&v).expect("patched fixture serializes")
}

/// [`patch_doc`] against a file, in place.
pub fn patch_file(path: &Path, f: impl FnOnce(&mut serde_json::Value)) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    std::fs::write(path, patch_doc(&text, f)).unwrap();
}

/// The effect list a quest runs when one of its objectives completes —
/// `content.quests[<quest>].on_objective_complete[<objective>]`. Panics if the
/// path is absent, which is the whole point (see [`patch_doc`]).
pub fn objective_effects<'a>(
    doc: &'a mut serde_json::Value,
    quest: usize,
    objective: &str,
) -> &'a mut Vec<serde_json::Value> {
    doc["content"]["quests"][quest]["on_objective_complete"][objective]
        .as_array_mut()
        .unwrap_or_else(|| panic!("quests[{quest}].on_objective_complete[{objective}] is an array"))
}
