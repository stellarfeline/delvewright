//! Process-level CLI tests: exit-code + diagnostic matrix, and the ADR-0006
//! double-build byte-identity determinism gate.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn version_line() {
    let out = delvec(&["--version"]);
    assert_eq!(code(&out), 0);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("delvec 0.1.0"), "{s}");
    assert!(s.contains("dsl 0.4.0"), "{s}");
    assert!(s.contains("mc 1.21.11"), "{s}");
}

#[test]
fn valid_campaign_validates_and_analyzes() {
    let hw = common::hello_world_dir();
    let pf = common::prefabs_dir();
    let v = delvec(&[
        "validate",
        hw.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate: {}",
        String::from_utf8_lossy(&v.stdout)
    );
    let a = delvec(&[
        "analyze",
        hw.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&a),
        0,
        "analyze: {}",
        String::from_utf8_lossy(&a.stdout)
    );
}

#[test]
fn build_is_byte_identical_across_runs() {
    let hw = common::hello_world_dir();
    let pf = common::prefabs_dir();
    let out_a = tmp("det-a");
    let out_b = tmp("det-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            hw.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
    // Sanity: the tree actually contains the key contract outputs.
    assert!(a.contains_key("manifest.json"));
    assert!(a.contains_key("critical-path.json"));
    assert!(a.contains_key("datapack/pack.mcmeta"));
    assert!(a.contains_key("server/server.properties"));
}

#[test]
fn invalid_fixtures_exit_1_with_expected_code() {
    let dir = common::dsl_invalid_dir();
    let pf = common::prefabs_dir();
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let patch: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let expect = patch["expect"].as_str().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let camp = tmp(&format!("inv-{stem}"));
        common::materialize(&patch, &camp);

        let out = delvec(&[
            "validate",
            camp.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
            "--json",
        ]);
        let stdout = String::from_utf8_lossy(&out.stdout);

        // DW0143 is registry-dependent: the fixture's "unknown" item
        // (`minecraft:diamond_hoe`) is rejected by the DSL's 5-item vendored
        // subset but is a REAL 1.21.11 item. The compiler injects the full
        // registry, so it correctly accepts it (exit 0) — proving the full
        // registry is wired. See `full_item_registry_accepts_real_items`.
        if expect == "DW0143" {
            assert_eq!(
                code(&out),
                0,
                "DW0143 fixture should pass against the full registry"
            );
            count += 1;
            continue;
        }

        assert_eq!(
            code(&out),
            1,
            "fixture {stem} should exit 1 (validation failure)"
        );
        assert!(
            stdout.contains(expect),
            "fixture {stem}: expected code {expect} in diagnostics:\n{stdout}"
        );
        count += 1;
    }
    assert!(
        count >= 20,
        "expected the full invalid-fixture matrix, saw {count}"
    );
}

#[test]
fn keep_crawl_builds_and_double_build_is_byte_identical() {
    let kc = common::keep_crawl_dir();
    let pf = common::prefabs_dir();
    let out_a = tmp("kc-det-a");
    let out_b = tmp("kc-det-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            kc.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(
            code(&r),
            0,
            "keep-crawl build: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "keep-crawl byte mismatch in {path}");
    }
    // The pool area shipped several distinct structures.
    let structures = a.keys().filter(|p| p.contains("/structure/keep-")).count();
    assert!(
        structures >= 3,
        "multiple keep pieces shipped, saw {structures}"
    );
}

/// The two `DW03xx` build/solver diagnostics: an unsatisfiable required anchor
/// (`DW0302`) and a `pieces` range too small for the required roles (`DW0303`).
/// Both pass validate + analyze (the DSL cannot see pool-area anchors) and fail
/// at build with exit 3 and their exact code in `--json`.
#[test]
fn pool_build_diagnostics_exit_3_with_dw03xx() {
    let pf = common::prefabs_dir();
    for (fixture, expect) in [
        ("keep-unsatisfiable-anchor.json", "DW0302"),
        ("keep-range-too-small.json", "DW0303"),
    ] {
        let patch: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(common::compiler_fixtures_dir().join(fixture)).unwrap(),
        )
        .unwrap();
        let camp = tmp(&format!("pool-{expect}"));
        common::materialize_from(&common::keep_crawl_dir(), &patch, &camp);

        // validate + analyze pass (pool anchors are invisible to the DSL layer).
        let v = delvec(&[
            "analyze",
            camp.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&v), 0, "{fixture}: analyze should pass");

        let out = tmp(&format!("pool-{expect}-out"));
        let b = delvec(&[
            "build",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(code(&b), 3, "{fixture}: build should exit 3");
        let stdout = String::from_utf8_lossy(&b.stdout);
        assert!(
            stdout.contains(expect),
            "{fixture}: expected {expect} in build diagnostics:\n{stdout}"
        );
    }
}

/// Gate-aware reachability (M2 fix 7, DW0306): a layout where an objective's
/// anchor is sealed behind a gate that only a later objective opens passes
/// validate + analyze (neither models sealed gates) but fails at build with exit 3
/// and DW0306. The clean keep-trial fixture (gate opened when the keeper is
/// greeted) builds fine — the same solver/seed, only the open-gate timing differs.
#[test]
fn gate_deadlock_exits_3_with_dw0305() {
    let pf = common::prefabs_dir();
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::compiler_fixtures_dir().join("keep-trial-gate-deadlock.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let camp = tmp("gate-deadlock");
    common::materialize_from(&common::keep_trial_dir(), &patch, &camp);
    // This build-error fixture alters keep-trial's strings; it is not an i18n test,
    // so drop the language declaration + sidecar (would otherwise fail coverage).
    common::make_english_only(&camp);

    // validate + analyze pass — sealed-gate reachability is invisible to them.
    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&a),
        0,
        "analyze should pass: {}",
        String::from_utf8_lossy(&a.stdout)
    );

    // build fails at DW0306.
    let out = tmp("gate-deadlock-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "gate deadlock should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0306"), "expected DW0306:\n{stdout}");

    // The clean keep-trial (gate opened at the greeting) builds fine.
    let clean = tmp("gate-clean-out");
    let r = delvec(&[
        "build",
        common::keep_trial_dir().to_str().unwrap(),
        "-o",
        clean.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&r),
        0,
        "clean keep-trial builds: {}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn unreachable_finale_exits_2_with_dw0201() {
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("unreachable-finale.json"))
            .unwrap(),
    )
    .unwrap();
    let pf = common::prefabs_dir();
    let camp = tmp("unreachable-finale");
    common::materialize(&patch, &camp);

    // validate passes (no DSL rule violated) ...
    let v = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate should pass: {}",
        String::from_utf8_lossy(&v.stdout)
    );

    // ... but analyze rejects it with exit 2 and DW0201.
    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&a), 2, "analyze should exit 2");
    let stdout = String::from_utf8_lossy(&a.stdout);
    assert!(stdout.contains("DW0201"), "expected DW0201:\n{stdout}");

    // build also stops at analysis (exit 2), never emitting.
    let out = tmp("unreachable-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(code(&b), 2, "build should exit 2 on unreachable finale");
}

// ---------------------------------------------------------------------------
// i18n (spec-0001/0002 addendum): --lang build, l10n sidecar coverage
// ---------------------------------------------------------------------------

/// Overwrite `camp/l10n/zh-cn.json`'s `content` after `mutate` edits it, keeping
/// the rest of the envelope. Used to synthesize DW0180/DW0181 fixtures from the
/// real keep-trial sidecar.
fn mutate_sidecar(
    camp: &Path,
    mutate: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) {
    let path = camp.join("l10n/zh-cn.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let content = doc["content"].as_object_mut().unwrap();
    mutate(content);
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}

/// keep-trial declares `zh-cn`; a full zh-cn sidecar makes both an `en` build and a
/// `--lang zh-cn` build succeed. The zh-cn build is deterministic, records the
/// language in the manifest, differs from `en` only in string-bearing files, and
/// keeps `critical-path.json` byte-identical (the bot contract is language-neutral).
#[test]
fn lang_build_localizes_only_strings_and_is_deterministic() {
    let kt = common::keep_trial_dir();
    let pf = common::prefabs_dir();
    let en = tmp("kt-lang-en");
    let zh_a = tmp("kt-lang-zh-a");
    let zh_b = tmp("kt-lang-zh-b");
    let build = |out: &Path, lang: Option<&str>| {
        let mut args = vec!["build", kt.to_str().unwrap(), "-o", out.to_str().unwrap()];
        args.extend(["--prefabs", pf.to_str().unwrap()]);
        if let Some(l) = lang {
            args.extend(["--lang", l]);
        }
        let r = delvec(&args);
        assert_eq!(
            code(&r),
            0,
            "build {lang:?}: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    };
    build(&en, None); // default = en
    build(&zh_a, Some("zh-cn"));
    build(&zh_b, Some("zh-cn"));

    let en_t = read_tree(&en);
    let zh = read_tree(&zh_a);
    let zh2 = read_tree(&zh_b);

    // Deterministic: same DSL + seed + lang → byte-identical.
    assert_eq!(
        zh.keys().collect::<Vec<_>>(),
        zh2.keys().collect::<Vec<_>>()
    );
    for (p, b) in &zh {
        assert_eq!(b, &zh2[p], "zh-cn double-build mismatch in {p}");
    }

    // Same file set; the difference is confined to string-bearing files + manifest.
    assert_eq!(
        en_t.keys().collect::<Vec<_>>(),
        zh.keys().collect::<Vec<_>>()
    );
    let differing: Vec<&String> = en_t
        .iter()
        .filter(|(p, b)| *b != &zh[*p])
        .map(|(p, _)| p)
        .collect();
    assert!(
        differing.iter().any(|p| p.contains("/dialog/")),
        "expected localized dialog files to differ, got {differing:?}"
    );
    // Language-neutral outputs stay byte-identical between en and zh-cn.
    assert_eq!(
        en_t["critical-path.json"], zh["critical-path.json"],
        "critical-path.json must be language-neutral"
    );
    for p in &differing {
        assert!(
            p.contains("/dialog/")
                || p.contains("/function/")
                || p.contains("/advancement/")
                || p.contains("packtest-datapack/")
                // render-plan.json embeds each NPC's display name in its
                // `expect` checklist ("NPC named X faces the camera"); the vision
                // tier verifies the localized in-game name tag, so it localizes
                // with the build (unlike the bot-facing critical-path.json).
                || p.as_str() == "render-plan.json"
                || p.as_str() == "manifest.json",
            "unexpected non-string file differs between en and zh-cn: {p}"
        );
    }

    // The manifest records the non-canonical language; en's manifest does not.
    let zh_manifest = String::from_utf8(zh["manifest.json"].clone()).unwrap();
    assert!(
        zh_manifest.contains("\"language\": \"zh-cn\""),
        "{zh_manifest}"
    );
    let en_manifest = String::from_utf8(en_t["manifest.json"].clone()).unwrap();
    assert!(
        !en_manifest.contains("\"language\""),
        "en manifest must omit language"
    );
}

/// An undeclared `--lang` is a validation-class rejection of the requested build
/// (exit 1); `--lang en` always works even when the campaign declares no languages.
#[test]
fn undeclared_lang_exits_1() {
    let pf = common::prefabs_dir();
    let out = tmp("kt-lang-fr-out");
    let b = delvec(&[
        "build",
        common::keep_trial_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--lang",
        "fr",
    ]);
    assert_eq!(code(&b), 1, "undeclared --lang should exit 1");

    // --lang en on an English-only campaign is a no-op success.
    let hw_out = tmp("hw-lang-en-out");
    let r = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        hw_out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--lang",
        "en",
    ]);
    assert_eq!(code(&r), 0, "--lang en must always succeed");
}

/// A declared language whose sidecar is missing an inventory key fails validation
/// with `DW0180` (under-coverage); an orphan key fails with `DW0181`.
#[test]
fn l10n_coverage_gaps_fire_dw0180_and_dw0181() {
    let pf = common::prefabs_dir();

    // DW0180: drop a required key.
    let miss = tmp("l10n-missing");
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &miss);
    mutate_sidecar(&miss, |c| {
        c.remove("world.title")
            .expect("key present in the real sidecar");
    });
    let out = delvec(&[
        "validate",
        miss.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&out), 1, "missing key should exit 1");
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("DW0180"), "expected DW0180:\n{s}");

    // DW0181: add an orphan key not in the inventory.
    let orph = tmp("l10n-orphan");
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &orph);
    mutate_sidecar(&orph, |c| {
        c.insert("world.subtitle".into(), serde_json::json!("多余的键"));
    });
    let out = delvec(&[
        "validate",
        orph.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&out), 1, "orphan key should exit 1");
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("DW0181"), "expected DW0181:\n{s}");
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, map);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                map.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    walk(root, root, &mut map);
    map
}
