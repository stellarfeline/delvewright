//! spec-0083 acceptance criterion 10 — **byte identity.** Taking a link only
//! where a walk fails is what keeps every campaign without one byte-identical,
//! and this file holds the build to it.
//!
//! `tests/golden/link-byte-identity.json` is a frozen measurement: every
//! fixture directory under `tests/fixtures/` built by the engine at revision
//! `e3a6dd36` (the instrument is named in the document), its output tree
//! digested. Here every fixture is built again by this engine through the same
//! entry point, over the same pinned prefab library, and compared:
//!
//! * a fixture that declares no `teleport` builds to the identical tree (or is
//!   refused with the identical exit status);
//! * a fixture that declares one — `teleport-volume`, `lift`, `lift-stake` —
//!   differs in `validation/teleport-gate.json`, whose new keys are exactly
//!   `teleports.links`, `teleports.gathers` and `legs_carried` over the frozen
//!   ledger, and in `manifest.json`, which hashes it; in nothing else.
//!
//! The document names the pinned content revision it was measured against; a
//! run against another pin is refused rather than compared, because a digest
//! over a different library says nothing about this engine.

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

fn tree_digest(files: &BTreeMap<String, String>) -> String {
    let mut h = Sha256::new();
    for (rel, sum) in files {
        h.update(format!("{rel}\t{sum}\n").as_bytes());
    }
    format!("{:x}", h.finalize())
}

#[test]
fn every_fixture_builds_as_it_did_before_links_and_the_teleport_ledgers_only_gain_keys() {
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::repo_root().join("crates/delvec/tests/golden/link-byte-identity.json"),
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
    let prefabs = common::prefabs_dir();
    let fixtures = golden["fixtures"].as_object().unwrap();
    assert!(
        fixtures.len() >= 25,
        "{} fixture(s) in the golden",
        fixtures.len()
    );
    let mut compared = 0usize;
    for (name, want) in fixtures {
        let dir = common::compiler_fixtures_dir().join(name);
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("byte-identity-{name}"));
        let _ = std::fs::remove_dir_all(&out);
        let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
            .arg("build")
            .arg(&dir)
            .arg("--out")
            .arg(&out)
            .arg("--prefabs")
            .arg(&prefabs)
            .output()
            .expect("delvec runs");
        let status = o.status.code().unwrap_or(-1);
        assert_eq!(
            i64::from(status),
            want["status"].as_i64().unwrap(),
            "{name}: exit status moved:\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
        compared += 1;
        if status != 0 {
            continue;
        }
        let got = files(&out);
        match want.get("per_file") {
            None => assert_eq!(
                tree_digest(&got),
                want["tree_sha256"].as_str().unwrap(),
                "{name}: a fixture with no teleport built to a different tree"
            ),
            Some(per_file) => {
                let per_file = per_file.as_object().unwrap();
                assert_eq!(got.len(), per_file.len(), "{name}: the file set moved");
                for (rel, sum) in per_file {
                    if rel == "validation/teleport-gate.json" || rel == "manifest.json" {
                        continue;
                    }
                    assert_eq!(
                        got.get(rel).map(String::as_str),
                        sum.as_str(),
                        "{name}: {rel} moved"
                    );
                }
                let mut ledger: Value = serde_json::from_str(
                    &std::fs::read_to_string(out.join("validation/teleport-gate.json")).unwrap(),
                )
                .unwrap();
                let t = ledger["teleports"].as_object_mut().unwrap();
                let links = t.remove("links").expect("teleports.links");
                let gathers = t.remove("gathers").expect("teleports.gathers");
                assert_eq!(
                    links.as_u64().unwrap() + gathers.as_u64().unwrap(),
                    t["declared"].as_u64().unwrap(),
                    "{name}: links + gathers = declared"
                );
                ledger
                    .as_object_mut()
                    .unwrap()
                    .remove("legs_carried")
                    .expect("legs_carried");
                assert_eq!(
                    ledger, want["teleport_gate"],
                    "{name}: the ledger moved beyond its new keys"
                );
            }
        }
    }
    assert_eq!(compared, fixtures.len());
}
