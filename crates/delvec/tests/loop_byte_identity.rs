//! spec-0086 acceptance criterion 1 — **byte identity.** A campaign that
//! declares no loop builds as it did before loops existed.
//!
//! `tests/fixtures/loop-byte-identity.json` is a frozen measurement: every
//! fixture directory under `tests/fixtures/` built by the engine at revision
//! `c0f22c51` (the instrument is named in the document, binary digest
//! included), and `long-gallery` with its `loops[]` and `state[]` removed built
//! over the synthesised corridor ([`common::corridor`]). Here each is built again
//! by this engine through the same entry point and compared:
//!
//! * the exit status is the same;
//! * every output file but three is the same, digested as one tree;
//! * the three that move do so by design and are compared field by field:
//!   `validation/effect-roots.json` gains exactly the `loop on_cross` root (the
//!   ledger enumerates every root, so a tenth root is a tenth row, bound to
//!   nothing here), `validation/lethal-gate.json` gains exactly
//!   `danger_visibility.roots`, and `manifest.json` hashes both.
//!
//! That is a loosening of the criterion's "byte-identically", declared in the
//! spec's report as such: the datapack and every other artifact are identical,
//! and the ledgers that count what the engine examined count one more thing.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use sha2::{Digest, Sha256};

fn files(tree: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().to_string();
                let sum = Sha256::digest(std::fs::read(&p).unwrap());
                out.insert(rel, format!("{sum:x}"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(tree, tree, &mut out);
    out
}

fn stable_digest(files: &BTreeMap<String, String>, moving: &[&str]) -> String {
    let mut h = Sha256::new();
    for (rel, sum) in files {
        if moving.contains(&rel.as_str()) {
            continue;
        }
        h.update(format!("{rel}\t{sum}\n").as_bytes());
    }
    format!("{:x}", h.finalize())
}

/// This engine's effect-root ledger with the one root spec-0086 adds taken
/// back out — what the frozen ledger must equal.
fn without_loop_root(mut v: Value) -> Value {
    let o = v.as_object_mut().unwrap();
    for k in ["roots_enumerated", "roots_total"] {
        let n = o[k].as_u64().unwrap();
        o.insert(k.to_string(), Value::from(n - 1));
    }
    let sites = o["sites"].as_object_mut().unwrap();
    assert_eq!(sites.remove("loop on_cross"), Some(Value::from(0)));
    let unbound = o["unbound_roots"].as_array_mut().unwrap();
    let before = unbound.len();
    unbound.retain(|r| r != "loop on_cross");
    assert_eq!(unbound.len(), before - 1, "the new root is unbound here");
    v
}

fn build(dir: &Path, prefabs: &Path, name: &str) -> (i32, std::path::PathBuf, String) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("loop-identity-{name}"));
    let _ = std::fs::remove_dir_all(&out);
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(dir)
        .arg("--out")
        .arg(&out)
        .arg("--prefabs")
        .arg(prefabs)
        .output()
        .expect("delvec runs");
    (
        o.status.code().unwrap_or(-1),
        out,
        String::from_utf8_lossy(&o.stderr).to_string(),
    )
}

#[test]
fn every_campaign_without_a_loop_builds_as_it_did_before_loops() {
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::repo_root().join("crates/delvec/tests/fixtures/loop-byte-identity.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        golden["pinned_content"].as_str().unwrap(),
        common::pinned::sha(),
        "the golden digests were measured against another content pin; re-measure them with the \
         instrument the document names before comparing"
    );
    let moving: Vec<&str> = golden["moving"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let fixtures = golden["fixtures"].as_object().unwrap();
    assert!(
        fixtures.len() >= 30,
        "{} fixture(s) in the golden",
        fixtures.len()
    );
    let pinned = common::prefabs_dir();
    let mut compared = 0usize;
    for (name, want) in fixtures {
        let (dir, prefabs) = if name == "long-gallery (loops removed)" {
            let src = common::compiler_fixtures_dir().join("long-gallery");
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("loop-identity-gallery-src");
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            for f in common::STAGE_FILES {
                std::fs::copy(src.join(f), dir.join(f)).unwrap();
            }
            common::patch_file(&dir.join("quests.json"), |q| {
                q["content"].as_object_mut().unwrap().remove("loops");
                q["content"].as_object_mut().unwrap().remove("state");
            });
            let prefabs = common::corridor::gallery_prefabs(
                "loop-identity",
                &common::corridor::Cuts::default(),
            );
            (dir, prefabs)
        } else {
            (common::compiler_fixtures_dir().join(name), pinned.clone())
        };
        let tag = name.replace([' ', '(', ')'], "-");
        let (status, out, stderr) = build(&dir, &prefabs, &tag);
        assert_eq!(
            i64::from(status),
            want["status"].as_i64().unwrap(),
            "{name}: exit status moved:\n{stderr}"
        );
        compared += 1;
        if status != 0 {
            continue;
        }
        let got = files(&out);
        assert_eq!(
            got.len() as u64,
            want["files"].as_u64().unwrap(),
            "{name}: the file set moved"
        );
        // A row may name a file the test itself writes unordered — the
        // synthesised piece's structure, which the build copies verbatim — and
        // that file is then held to the bytes this run wrote, not to a digest.
        let mut moving = moving.clone();
        for extra in want["also_moving"].as_array().into_iter().flatten() {
            let rel = extra.as_str().unwrap();
            moving.push(rel);
            let name = Path::new(rel).file_name().unwrap();
            assert_eq!(
                std::fs::read(out.join(rel)).unwrap(),
                std::fs::read(prefabs.join(name)).unwrap(),
                "{rel} is the piece, copied verbatim"
            );
        }
        assert_eq!(
            stable_digest(&got, &moving),
            want["stable_tree_sha256"].as_str().unwrap(),
            "{name}: a campaign with no loop built to a different tree"
        );
        let roots: Value = serde_json::from_str(
            &std::fs::read_to_string(out.join("validation/effect-roots.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            without_loop_root(roots),
            want["effect_roots"],
            "{name}: the effect-root ledger moved beyond its new root"
        );
        if let Some(frozen) = want.get("lethal_gate") {
            let mut ledger: Value = serde_json::from_str(
                &std::fs::read_to_string(out.join("validation/lethal-gate.json")).unwrap(),
            )
            .unwrap();
            ledger["danger_visibility"]
                .as_object_mut()
                .unwrap()
                .remove("roots")
                .expect("danger_visibility.roots");
            assert_eq!(
                &ledger, frozen,
                "{name}: the lethal ledger moved beyond `roots`"
            );
        }
    }
    assert_eq!(compared, fixtures.len());
}
