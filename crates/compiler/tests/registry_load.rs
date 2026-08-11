//! `PrefabRegistry::load_dir` load-failure reporting (task #62, `DW0346`):
//! an unparseable prefab metadata file — e.g. NEWER metadata (an unknown field
//! under `deny_unknown_fields`) met by an OLDER delvec — must yield a real
//! diagnostic naming the file and the serde error, not a silent skip that
//! resurfaces later as a baffling `DW0300` "prefab not found". Loading is
//! report-all: every other file still loads.

mod common;

use delvewright_compiler::registry::{DW_PREFAB_META_INVALID, PrefabRegistry};

/// A private, mutable copy of the prefab library (the real content repo is
/// read-only for tests).
fn prefab_copy(tag: &str) -> std::path::PathBuf {
    let tmp = std::env::temp_dir().join(format!("dw-registry-load-{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    common::copy_dir_all(&common::prefabs_dir(), &tmp);
    tmp
}

/// The clean library loads with zero diagnostics — DW0346 is a tripwire, not a
/// tax on the happy path.
#[test]
fn clean_library_has_no_load_diagnostics() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    assert!(
        prefabs.load_diagnostics().is_empty(),
        "the real prefab library must load clean: {:#?}",
        prefabs.load_diagnostics()
    );
}

/// Newer-schema metadata (an unknown field) yields `DW0346` naming the file
/// and the serde error, with the upgrade-or-fix prescription — and every other
/// prefab still loads (report-all, not fail-fast).
#[test]
fn unknown_field_yields_dw0346_naming_file_and_others_still_load() {
    let tmp = prefab_copy("unknown-field");
    // Inject a field this delvec's PrefabMeta does not know (deny_unknown_fields).
    let hello = tmp.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hello).unwrap()).unwrap();
    meta.as_object_mut()
        .unwrap()
        .insert("from_the_future".to_string(), serde_json::json!(true));
    std::fs::write(&hello, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let prefabs = PrefabRegistry::load_dir(&tmp).unwrap();
    let d = prefabs.load_diagnostics();
    assert_eq!(d.len(), 1, "exactly one failing file: {d:#?}");
    assert_eq!(d[0].code, DW_PREFAB_META_INVALID);
    assert!(
        d[0].path == "hello-room.json" && d[0].message.contains("hello-room.json"),
        "the diagnostic must name the failing file: {d:#?}"
    );
    assert!(
        d[0].message.contains("from_the_future"),
        "the serde error (naming the unknown field) must be surfaced: {d:#?}"
    );
    assert!(
        d[0].message.contains("upgrade delvec"),
        "the prescription must say upgrade-or-fix: {d:#?}"
    );
    // The broken file is skipped; the rest of the library still loads.
    assert!(prefabs.get("prefab/hello-room").is_none());
    assert!(
        prefabs.get("prefab/keep-gate-room").is_some(),
        "other prefabs must still load (report-all, not fail-fast)"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Outright JSON syntax garbage is reported the same way, and a broken
/// `pools.json` is covered too (pools silently vanishing strands every
/// `prefab_pool` area).
#[test]
fn syntax_garbage_and_broken_pools_yield_dw0346() {
    let tmp = prefab_copy("garbage");
    std::fs::write(tmp.join("cave-den.json"), "{ not json").unwrap();
    std::fs::write(tmp.join("pools.json"), "[]").unwrap();

    let prefabs = PrefabRegistry::load_dir(&tmp).unwrap();
    let d = prefabs.load_diagnostics();
    let named: Vec<&str> = d.iter().map(|x| x.path.as_str()).collect();
    assert!(
        named.contains(&"cave-den.json") && named.contains(&"pools.json"),
        "both broken files must be reported: {d:#?}"
    );
    assert!(d.iter().all(|x| x.code == DW_PREFAB_META_INVALID));
    // Report-all: the rest of the library is intact.
    assert!(prefabs.get("prefab/hello-room").is_some());
    let _ = std::fs::remove_dir_all(&tmp);
}

/// End-to-end: `delvec validate` over a library with one future-schema file
/// exits 1 (validation tier) and prints the `DW0346` — the compiler-version
/// mismatch is surfaced at the front door, not as a downstream `DW0300`.
#[test]
fn cli_validate_reports_dw0346_at_exit_1() {
    let tmp = prefab_copy("cli");
    let hello = tmp.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hello).unwrap()).unwrap();
    meta.as_object_mut()
        .unwrap()
        .insert("from_the_future".to_string(), serde_json::json!(true));
    std::fs::write(&hello, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "validate",
            common::hello_world_dir().to_str().unwrap(),
            "--prefabs",
            tmp.to_str().unwrap(),
        ])
        .output()
        .expect("run delvec");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a broken prefab metadata file is a validation failure (exit 1): {stdout}"
    );
    assert!(
        stdout.contains(DW_PREFAB_META_INVALID.id()) && stdout.contains("hello-room.json"),
        "DW0346 naming the file must be printed: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
