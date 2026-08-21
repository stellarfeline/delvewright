//! `delvec fmt` — the canonical-form formatter and its `--check` gate.
//!
//! The task's hard constraint is that **sorting an array changes the game**, so
//! the tests that matter are not about layout. In order of weight:
//!
//! 1. [`every_authored_json_survives_formatting_unchanged_in_meaning`] — a sweep
//!    over every authored JSON in the tree, asserting semantic identity through
//!    an **independent** parser (`serde_json::Value`, whose equality is exactly
//!    "objects order-insensitive, arrays order-sensitive"). This is the array
//!    rule, checked against every real document the repo has.
//! 2. [`an_ordered_array_keeps_its_order_element_for_element`] — the same claim
//!    stated legibly on `quests[]`/`objectives[]`/`effects[]` by name, so a
//!    reader can see what is being protected.
//! 3. [`formatting_a_campaign_changes_only_the_manifest_input_hashes`] — the
//!    end-to-end proof: build a campaign, format its sources, build again, and
//!    every emitted byte is identical. The one exception is stated rather than
//!    smoothed: `manifest.json` records the sha256 of the SOURCE bytes as
//!    provenance, so it must move when the sources are rewritten — a formatter
//!    that left it alone would have broken the provenance record instead.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvewright_dsl::fmt;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let p = entry.unwrap().path();
        let to = dst.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_dir(&p, &to);
        } else {
            std::fs::copy(&p, &to).unwrap();
        }
    }
}

/// Every JSON document this repository holds, **derived** — `git ls-files`,
/// minus the one exemption.
///
/// This used to name two roots by hand while its doc comment claimed to be
/// "every authored JSON document in this repository". It was not: it saw 240 of
/// the 335 files git tracks, and 50 of the 95 it never opened were out of
/// canonical form. That is the same enumeration-somebody-remembered shape the CI
/// sweep had, one layer down and wearing a truthful-sounding sentence — and it
/// is why there is now ONE derivation rather than two lists that can disagree.
/// `tools/check-json-canonical.py` is its sibling and takes its population the
/// same way.
///
/// `crates/compiler/tests/golden/` is excluded for the reason
/// `view::scene::golden_scene_matches` gives: those bytes are pinned to emitter
/// output, and `every_golden_is_emitter_output` closes the directory so nothing
/// authored can hide there.
///
/// Fails loudly if git cannot answer. A fallback to a hand-written list here
/// would reintroduce exactly the defect above, at the moment nobody is looking.
fn all_authored_files() -> Vec<PathBuf> {
    let root = common::repo_root();
    let out = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "ls-files",
            "-z",
            "--",
            "*.json",
        ])
        .output()
        .expect("run `git ls-files` — this sweep derives its corpus and never lists it");
    assert!(
        out.status.success(),
        "`git ls-files` exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files: Vec<PathBuf> = String::from_utf8(out.stdout)
        .unwrap()
        .split('\0')
        .filter(|p| !p.is_empty() && !p.starts_with("crates/compiler/tests/golden/"))
        .map(|p| root.join(p))
        .collect();
    files.sort();
    files
}

/// THE array test. `serde_json::Value` compares objects as maps (key order
/// irrelevant) and arrays as `Vec` (order relevant), so equality here says
/// exactly: keys may be reordered, elements may not. The oracle is a different
/// parser from the one under test, so a bug shared between writer and reader
/// cannot hide behind it.
#[test]
fn every_authored_json_survives_formatting_unchanged_in_meaning() {
    let files = all_authored_files();
    // Vacuity guard: a sweep that swept nothing is not a pass.
    assert!(
        files.len() > 100,
        "expected the authored-JSON sweep to bind to the repo's fixture corpus, \
         found {} file(s) — did a fixture directory move?",
        files.len()
    );
    let mut arrays_seen = 0usize;
    for path in &files {
        let original = std::fs::read_to_string(path).unwrap();
        let formatted =
            fmt::format_text(&original).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let before: serde_json::Value = serde_json::from_str(&original).unwrap();
        let after: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        assert_eq!(
            before,
            after,
            "formatting changed the meaning of {}",
            path.display()
        );
        arrays_seen += count_arrays(&before);
        // Idempotence: the canonical form is a fixed point, so a second run is
        // never a second diff.
        assert_eq!(
            fmt::format_text(&formatted).unwrap(),
            formatted,
            "not idempotent: {}",
            path.display()
        );
    }
    assert!(
        arrays_seen > 500,
        "the sweep examined {arrays_seen} arrays; too few to call the array rule proven"
    );
    eprintln!(
        "fmt sweep: {} authored JSON files, {arrays_seen} arrays, all semantically unchanged",
        files.len()
    );
}

fn count_arrays(v: &serde_json::Value) -> usize {
    match v {
        serde_json::Value::Array(items) => 1 + items.iter().map(count_arrays).sum::<usize>(),
        serde_json::Value::Object(map) => map.values().map(count_arrays).sum(),
        _ => 0,
    }
}

/// The same guarantee, said out loud on the arrays the DSL's semantics live in.
/// A deliberately anti-sorted input: every array here is in an order that a
/// naive sort would visibly disturb.
#[test]
fn an_ordered_array_keeps_its_order_element_for_element() {
    let src = r#"{
      "quests": [
        {"id": "quest/zulu",  "objectives": [{"id": "obj/c"}, {"id": "obj/a"}, {"id": "obj/b"}]},
        {"id": "quest/alpha", "objectives": [{"id": "obj/z"}, {"id": "obj/y"}]}
      ],
      "effects": ["give", "announce", "advance"],
      "numbers": [3, 1, 2]
    }"#;
    let out = fmt::format_text(src).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();

    let quests = v["quests"].as_array().unwrap();
    assert_eq!(quests[0]["id"], "quest/zulu");
    assert_eq!(quests[1]["id"], "quest/alpha");
    let objs = quests[0]["objectives"].as_array().unwrap();
    assert_eq!(
        objs.iter()
            .map(|o| o["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["obj/c", "obj/a", "obj/b"]
    );
    assert_eq!(
        v["effects"].as_array().unwrap(),
        &vec![
            serde_json::json!("give"),
            serde_json::json!("announce"),
            serde_json::json!("advance")
        ]
    );
    assert_eq!(v["numbers"].to_string(), "[3,1,2]");

    // …and the object keys DID sort, so the test is not passing because nothing
    // happened at all.
    let keys: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("  \""))
        .map(|l| l.trim().trim_start_matches('"').split('"').next().unwrap())
        .collect();
    assert_eq!(keys, ["effects", "numbers", "quests"]);
}

/// The end-to-end semantics proof: same campaign, sources reformatted, every
/// emitted byte identical — with the one honest exception named and asserted.
#[test]
fn formatting_a_campaign_changes_only_the_manifest_input_hashes() {
    let pf = common::prefabs_dir();
    let src_a = tmp("fmt-campaign-src");
    let src_b = tmp("fmt-campaign-fmt");
    copy_dir(&common::keep_trial_dir(), &src_a);
    copy_dir(&common::keep_trial_dir(), &src_b);

    // The in-repo fixtures are canonical (CI keeps them so), so de-canonicalize
    // `src_a` first — otherwise `fmt` would have nothing to do and this test
    // would prove nothing. Compact one-line JSON is semantically identical and
    // maximally far from the canonical form: different key order, different
    // whitespace, no trailing newline.
    let mut scrambled = 0;
    for name in common::STAGE_FILES {
        let path = src_a.join(name);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_string(&v).unwrap()).unwrap();
        scrambled += 1;
    }
    assert_eq!(
        scrambled, 6,
        "binding count: stage documents de-canonicalized"
    );

    let r = delvec(&["fmt", src_b.to_str().unwrap()]);
    assert_eq!(code(&r), 0, "fmt: {}", String::from_utf8_lossy(&r.stderr));
    // The two source trees must really differ byte-wise, or this test would be
    // comparing a build with itself.
    assert_ne!(
        std::fs::read_to_string(src_a.join("quests.json")).unwrap(),
        std::fs::read_to_string(src_b.join("quests.json")).unwrap(),
        "the two source trees are identical; nothing would be proven"
    );

    let out_a = tmp("fmt-build-src");
    let out_b = tmp("fmt-build-fmt");
    for (src, out) in [(&src_a, &out_a), (&src_b, &out_b)] {
        let r = delvec(&[
            "build",
            src.to_str().unwrap(),
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
    let mut differing: Vec<&String> = Vec::new();
    for (path, bytes) in &a {
        if bytes != &b[path] {
            differing.push(path);
        }
    }
    assert_eq!(
        differing,
        vec!["manifest.json"],
        "formatting the sources changed emitted output beyond the provenance manifest"
    );
    assert!(
        a.len() > 50,
        "binding count: only {} emitted files",
        a.len()
    );

    // And inside `manifest.json`, only the `inputs` provenance hashes moved:
    // every `outputs` hash — i.e. every byte a player can reach — is identical.
    let ma: serde_json::Value = serde_json::from_slice(&a["manifest.json"]).unwrap();
    let mb: serde_json::Value = serde_json::from_slice(&b["manifest.json"]).unwrap();
    assert_eq!(ma["outputs"], mb["outputs"], "emitted-output hashes moved");
    assert_ne!(ma["inputs"], mb["inputs"], "source provenance did not move");
    for (key, _) in ma.as_object().unwrap() {
        if key != "inputs" {
            assert_eq!(ma[key], mb[key], "manifest key `{key}` moved");
        }
    }
}

#[test]
fn check_reports_then_fmt_fixes_and_the_second_check_is_clean() {
    let dir = tmp("fmt-check-cycle");
    copy_dir(&common::hello_world_dir(), &dir);
    // Scramble one file into a non-canonical but semantically identical shape.
    let quests = dir.join("quests.json");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&quests).unwrap()).unwrap();
    std::fs::write(&quests, serde_json::to_string(&v).unwrap()).unwrap();

    let r = delvec(&["fmt", "--check", dir.to_str().unwrap()]);
    assert_eq!(code(&r), 1);
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(stdout.contains("DW0773"), "{stdout}");
    assert!(stdout.contains("quests.json"), "{stdout}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(
        stderr.contains("examined"),
        "binding count missing: {stderr}"
    );

    let r = delvec(&["fmt", dir.to_str().unwrap()]);
    assert_eq!(code(&r), 0, "{}", String::from_utf8_lossy(&r.stderr));
    let r = delvec(&["fmt", "--check", dir.to_str().unwrap()]);
    assert_eq!(code(&r), 0, "{}", String::from_utf8_lossy(&r.stdout));
}

#[test]
fn a_duplicate_key_is_refused_rather_than_silently_collapsed() {
    let dir = tmp("fmt-dup-key");
    std::fs::write(dir.join("a.json"), "{\n  \"id\": 1,\n  \"id\": 2\n}\n").unwrap();
    let r = delvec(&["fmt", dir.to_str().unwrap()]);
    assert_eq!(code(&r), 1);
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(stdout.contains("DW0771"), "{stdout}");
    // Nothing was written.
    assert_eq!(
        std::fs::read_to_string(dir.join("a.json")).unwrap(),
        "{\n  \"id\": 1,\n  \"id\": 2\n}\n"
    );
}

#[test]
fn unparseable_json_is_located_not_guessed() {
    let dir = tmp("fmt-bad-json");
    std::fs::write(dir.join("a.json"), "{\n  \"a\": 1,\n}\n").unwrap();
    let r = delvec(&["fmt", "--check", dir.to_str().unwrap()]);
    assert_eq!(code(&r), 1);
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(stdout.contains("DW0770"), "{stdout}");
    assert!(stdout.contains("a.json:3:"), "{stdout}");
}

/// A gate that binds to nothing is vacuous, not a pass — so `fmt` says so and
/// exits 1 rather than reporting a clean run over zero files.
#[test]
fn matching_no_files_is_a_finding() {
    let dir = tmp("fmt-empty");
    let r = delvec(&["fmt", "--check", dir.to_str().unwrap()]);
    assert_eq!(code(&r), 1);
    assert!(String::from_utf8_lossy(&r.stdout).contains("DW0774"));
}

/// Emitted trees are not authored content: a `delvec build` output root is
/// marked by the `manifest.json` the compiler writes there, and discovery stops
/// at one. Some of those trees are checked in (`campaigns/*/out/`), and
/// rewriting one would break the byte-identity record it exists to hold.
#[test]
fn build_output_trees_and_dot_directories_are_skipped() {
    let dir = tmp("fmt-skips");
    std::fs::write(dir.join("authored.json"), "{}\n").unwrap();
    std::fs::create_dir_all(dir.join("out")).unwrap();
    std::fs::write(dir.join("out/manifest.json"), "{}\n").unwrap();
    std::fs::write(dir.join("out/other.json"), "{}\n").unwrap();
    std::fs::create_dir_all(dir.join(".hidden")).unwrap();
    std::fs::write(dir.join(".hidden/x.json"), "{}\n").unwrap();

    let found = fmt::discover(&dir).unwrap();
    assert_eq!(found, vec![dir.join("authored.json")]);

    // …but an explicit file argument is honoured: you pointed at it.
    assert_eq!(
        fmt::discover(&dir.join("out/other.json")).unwrap(),
        vec![dir.join("out/other.json")]
    );
}

/// Discovery order is sorted, never `read_dir` order (ADR-0006): the report a
/// `--check` prints must be the same report on every machine.
#[test]
fn discovery_is_deterministically_ordered() {
    let dir = tmp("fmt-order");
    for name in ["m.json", "a.json", "z.json", "b.json"] {
        std::fs::write(dir.join(name), "{}\n").unwrap();
    }
    let found = fmt::discover(&dir).unwrap();
    let names: Vec<String> = found
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["a.json", "b.json", "m.json", "z.json"]);
}

/// The two-repo reality: campaigns carry zh-cn sidecars, and an escaping choice
/// that mangles or churns them defeats the point of the exercise.
#[test]
fn non_ascii_content_is_written_raw_and_is_a_fixed_point() {
    let src = "{\"content\":{\"b.name\":\"洞中公羊\",\"a.name\":\"\\u65e0\\u4eba\\u4e4b\\u5c9b\"}}";
    let out = fmt::format_text(src).unwrap();
    assert!(out.contains("无人之岛"), "{out}");
    assert!(out.contains("洞中公羊"), "{out}");
    assert!(
        !out.contains("\\u"),
        "escaped non-ASCII would triple every sidecar: {out}"
    );
    assert_eq!(fmt::format_text(&out).unwrap(), out);
}
