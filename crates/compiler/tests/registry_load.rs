//! `PrefabRegistry::load_dir` reporting, in two kinds.
//!
//! **A malformed file is `DW0346`** — a real diagnostic naming the file and the
//! serde error, never a silent skip that resurfaces later as a baffling
//! `DW0300` "prefab not found". Loading is report-all: every other file still
//! loads.
//!
//! **A key this delvec does not model is `DW0543`, a warning, and the prefab
//! still loads.** That is the distinction these tests exist to hold. The
//! compiler used to read this document through a private copy of its shape with
//! `deny_unknown_fields`, so a library one key newer than the engine failed
//! every campaign build at the layer with the least context. The document has
//! one definition now (`delvewright_dsl::prefab`), it keeps what it does not
//! model, and the only thing a consumer does about an unknown key is say it saw
//! one.

mod common;

use delvewright_compiler::registry::{
    DW_PREFAB_META_INVALID, DW_PREFAB_META_UNKNOWN_KEY, PrefabRegistry,
};

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

/// Inject a key `delvec` does not model — at the top level and on an anchor —
/// into one prefab of a private copy of the library. Returns the copy.
///
/// Two levels on purpose: both are places this document has actually grown
/// (`waterline_y`, `spatial_contract`; `resolves_to`, `dispenser`,
/// `trigger_block`), and a reader that tolerates one and refuses the other is
/// the same defect with a smaller blast radius.
fn library_with_a_newer_key(tag: &str) -> std::path::PathBuf {
    let tmp = prefab_copy(tag);
    let hello = tmp.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hello).unwrap()).unwrap();
    let obj = meta.as_object_mut().unwrap();
    obj.insert("from_the_future".to_string(), serde_json::json!(true));
    let anchors = obj.get_mut("anchors").unwrap().as_object_mut().unwrap();
    let first = anchors.keys().next().unwrap().clone();
    anchors
        .get_mut(&first)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("acoustics".to_string(), serde_json::json!("reverberant"));
    std::fs::write(&hello, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    tmp
}

/// A key newer than this delvec is **kept, not refused**: the prefab loads, and
/// the only report is a `DW0543` warning naming every unknown key.
///
/// This is the assertion that would have caught the shipped defect. "The
/// current fields all parse" would not have: they did, on both sides, right up
/// until content added one key.
#[test]
fn a_key_this_delvec_does_not_model_loads_and_warns() {
    let tmp = library_with_a_newer_key("unknown-field");

    let prefabs = PrefabRegistry::load_dir(&tmp).unwrap();
    let d = prefabs.load_diagnostics();
    assert_eq!(
        d.len(),
        1,
        "exactly one file has anything to report: {d:#?}"
    );
    assert_eq!(d[0].code, DW_PREFAB_META_UNKNOWN_KEY);
    assert_eq!(
        d[0].severity,
        delvewright_dsl::Severity::Warning,
        "an unknown key is not a build failure: {d:#?}"
    );
    assert!(
        d[0].path == "hello-room.json" && d[0].message.contains("hello-room.json"),
        "the diagnostic must name the file: {d:#?}"
    );
    assert!(
        d[0].message.contains("from_the_future") && d[0].message.contains("acoustics"),
        "both the top-level and the anchor key must be named: {d:#?}"
    );
    // The whole point: the prefab is USABLE, not skipped.
    assert!(
        prefabs.get("prefab/hello-room").is_some(),
        "a prefab one key newer than this engine must still load"
    );
    assert!(prefabs.get("prefab/keep-gate-room").is_some());
    let _ = std::fs::remove_dir_all(&tmp);
}

/// The clean library binds this warning to **zero** files, which is what makes
/// the test above a tripwire rather than noise. A non-zero count here means the
/// pinned content library carries a key this engine does not model, which is a
/// finding about the pin, not about this test.
#[test]
fn the_pinned_library_carries_no_key_this_delvec_does_not_model() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let unknown: Vec<_> = prefabs
        .load_diagnostics()
        .iter()
        .filter(|d| d.code == DW_PREFAB_META_UNKNOWN_KEY)
        .collect();
    assert!(unknown.is_empty(), "{unknown:#?}");
}

/// **Reading is total, on the real library.** Every prefab document the pinned
/// content repo carries survives a parse-and-write round trip through the one
/// definition with no key lost and no value changed.
///
/// The unit tests prove this for documents this repo wrote; this proves it for
/// the ones a campaign actually ships, which is where the loss was live —
/// `waterline_y` sits on five of these files and the previous owner type did not
/// model it, so every admission step deleted it and `DW0344` quietly stopped
/// binding on that piece.
///
/// A written document may add a key it never drops one of: `connectors` has a
/// default and is always emitted, so a legacy piece that omitted it gains
/// `"connectors": []`. The invariant is therefore superset, not equality — and
/// it is asserted per key rather than per file so a failure names what was lost.
#[test]
fn every_shipped_prefab_document_round_trips_without_losing_a_key() {
    use delvewright_dsl::prefab::PrefabMeta;

    let dir = common::prefabs_dir();
    let mut checked = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    paths.sort();

    for path in paths {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        let before: serde_json::Value = serde_json::from_str(&text).unwrap();
        // `pools.json` is a different document, and a tile-set manifest names
        // `structure_set` instead of `structure`. Neither is this shape; both
        // are named rather than silently passed over.
        if name == "pools.json" || before.get("structure_set").is_some() {
            skipped.push(name);
            continue;
        }
        let meta = PrefabMeta::from_json(&text)
            .unwrap_or_else(|e| panic!("{name} must parse as prefab metadata: {e}"));
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();

        for (key, value) in before.as_object().unwrap() {
            assert_eq!(
                after.get(key),
                Some(value),
                "{name}: top-level key `{key}` did not survive the round trip"
            );
        }
        if let Some(anchors) = before.get("anchors").and_then(|a| a.as_object()) {
            for (anchor, body) in anchors {
                for (key, value) in body.as_object().unwrap() {
                    assert_eq!(
                        after["anchors"][anchor].get(key),
                        Some(value),
                        "{name}: anchor `{anchor}` key `{key}` did not survive the round trip"
                    );
                }
            }
        }
        checked += 1;
    }

    // Binding count: a green here over zero documents would prove nothing.
    assert!(
        checked >= 30,
        "only {checked} prefab document(s) were round-tripped (skipped: {skipped:?}) — the \
         pinned library carries 36, so this gate is examining almost nothing"
    );
    // And the field the loss was live on is really present to be checked.
    let with_waterline = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| std::fs::read_to_string(e.unwrap().path()).ok())
        .filter(|t| t.contains("\"waterline_y\""))
        .count();
    assert!(
        with_waterline > 0,
        "no shipped prefab declares `waterline_y`, so the round trip is not exercising the \
         field this gate exists for"
    );
    eprintln!(
        "round-tripped {checked} prefab document(s), {with_waterline} of them declaring \
         `waterline_y`; skipped {} non-prefab document(s): {skipped:?}",
        skipped.len()
    );
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

/// End-to-end, and the assertion the whole change is for: **a campaign still
/// BUILDS against a prefab library one key newer than this delvec.**
///
/// Not "validate exits 0" and not "the file parses" — a real `delvec build`,
/// producing a datapack, over a library carrying a key this engine has never
/// heard of, on the piece the campaign actually places. The old reader turned
/// that key into a failed load, a missing prefab and a `DW0300`, and the build
/// was over.
#[test]
fn a_campaign_builds_against_a_library_one_key_newer_than_this_delvec() {
    let tmp = library_with_a_newer_key("cli");
    let out_dir = std::env::temp_dir().join("dw-registry-load-cli-out");
    let _ = std::fs::remove_dir_all(&out_dir);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "build",
            common::hello_world_dir().to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
            "--prefabs",
            tmp.to_str().unwrap(),
        ])
        .output()
        .expect("run delvec");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a newer metadata key must not stop a campaign building:\n{stdout}\n{stderr}"
    );
    assert!(
        out_dir.join("datapack/pack.mcmeta").is_file(),
        "the datapack must actually be there"
    );
    assert!(
        stdout.contains(DW_PREFAB_META_UNKNOWN_KEY.id()) && stdout.contains("from_the_future"),
        "the unknown key must still be reported, by name: {stdout}"
    );

    // And the file is untouched: reading a document is not editing it.
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.join("hello-room.json")).unwrap())
            .unwrap();
    assert_eq!(after["from_the_future"], serde_json::json!(true));

    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// A file that is genuinely malformed for this delvec — a value of the wrong
/// type — is still `DW0346` at exit 1, and the prefab is still skipped. The
/// tolerance above is of unknown KEYS, not of unreadable documents.
#[test]
fn cli_validate_reports_dw0346_at_exit_1() {
    let tmp = prefab_copy("cli-bad");
    let hello = tmp.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hello).unwrap()).unwrap();
    meta.as_object_mut()
        .unwrap()
        .insert("connectors".to_string(), serde_json::json!("not a list"));
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

/// A tile-set manifest is refused with the QUEUED WORK named, not with the
/// generic newer-schema advice.
///
/// The distinction is the point. `structure_set` is a shape this delvec fully
/// understands and cannot yet place — compiler-side placement of a tile group
/// is chunked export phase 2 — so telling the operator to "upgrade delvec, or
/// fix the field" would send them after a fix that does not exist. And the zone
/// must be skipped loudly rather than half-placed. The pre-check that spots
/// `structure_set` is what carries this: a manifest names no `structure`, so
/// the parse would otherwise fail as a bare "missing field" — a true statement
/// about the bytes and a useless one about the situation.
#[test]
fn a_tile_set_manifest_is_refused_naming_the_queued_work() {
    let tmp = prefab_copy("tile-set");
    let hello = tmp.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&hello).unwrap()).unwrap();
    let obj = meta.as_object_mut().unwrap();
    obj.remove("structure");
    obj.insert(
        "structure_set".to_string(),
        serde_json::json!({
            "base": "hello-room", "size": [20, 10, 84], "part_max": 48,
            "grid": [1, 1, 2], "data_version": 4671, "generator": "crates/grammar",
            "parts": [
                { "file": "hello-room.x0y0z0.nbt", "id": "hello-room.x0y0z0",
                  "grid_index": [0, 0, 0], "offset": [0, 0, 0], "size": [20, 10, 48] },
                { "file": "hello-room.x0y0z1.nbt", "id": "hello-room.x0y0z1",
                  "grid_index": [0, 0, 1], "offset": [0, 0, 48], "size": [20, 10, 36] }
            ]
        }),
    );
    std::fs::write(&hello, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let prefabs = PrefabRegistry::load_dir(&tmp).unwrap();
    let d = prefabs.load_diagnostics();
    assert_eq!(d.len(), 1, "exactly one failing file: {d:#?}");
    assert_eq!(d[0].code, DW_PREFAB_META_INVALID);
    assert!(d[0].message.contains("TILE SET"), "{:?}", d[0].message);
    assert!(
        d[0].message.contains("phase 2"),
        "the refusal must name the queued work, not a fix that does not exist: {:?}",
        d[0].message
    );
    assert!(
        !d[0].message.contains("upgrade delvec"),
        "the newer-schema prescription is wrong here and must not be given: {:?}",
        d[0].message
    );
}
