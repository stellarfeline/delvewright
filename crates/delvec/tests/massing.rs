//! L2 massing verbs (spec-0017): the `v06-massing` fixture's five-verb
//! net-identity tour, the ADR-0006 determinism gate over massing, `DW0324`
//! application failures, and the full-assembly-revalidation proof (a massing
//! edit that seals the critical path fails `DW0311`, not QA).
//!
//! Process-level (the real `delvec` binary), like `tests/edit.rs`.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(common::repo_root())
        .output()
        .expect("run delvec")
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .expect("relative")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).expect("read file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn massing_fixture_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("v06-massing")
}

fn prefabs_arg() -> String {
    common::prefabs_dir().display().to_string()
}

fn massing_copy(name: &str) -> PathBuf {
    let dst = tmp(name);
    common::copy_dir_all(&massing_fixture_dir(), &dst);
    dst
}

fn set_batches(dir: &Path, campaign_id: &str, batches: serde_json::Value) {
    let doc = serde_json::json!({
        "dsl_version": "0.19.0",
        "campaign_id": campaign_id,
        "stage": "world-edits",
        "content": { "batches": batches }
    });
    std::fs::write(
        dir.join("world-edits.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .unwrap();
}

fn build(dir: &Path, out: &Path) -> Output {
    delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ])
}

fn combined(r: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    )
}

/// ADR-0006 over L2 massing: the five-verb fixture double-builds
/// byte-identically (the reseed draw is a named stream off the campaign seed
/// and the verb's script position — nothing else).
#[test]
fn v06_massing_double_build_is_byte_identical() {
    let dir = massing_fixture_dir();
    let (out_a, out_b) = (tmp("massing-det-a"), tmp("massing-det-b"));
    for out in [&out_a, &out_b] {
        let r = build(&dir, out);
        assert!(r.status.success(), "massed build failed:\n{}", combined(&r));
    }
    let (a, b) = (read_tree(&out_a), read_tree(&out_b));
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
}

/// The five-verb tour is a NET IDENTITY (remove + re-insert the shrine,
/// reseed + swap-back the gate room, seal + reopen a doorway) — so the
/// massaged layout's placement function must be byte-identical to the
/// unmassaged keep-crawl solve's. This is the composition proof: every verb
/// really applied (a failed application is a loud DW0324), and their
/// round-trips restore the exact solver state (positions, rotations, order).
#[test]
fn net_identity_massing_tour_restores_the_unmassaged_layout() {
    let massed = tmp("massing-net-a");
    let r = build(&massing_fixture_dir(), &massed);
    assert!(r.status.success(), "massed build failed:\n{}", combined(&r));

    // The same campaign without the edit script = the pristine solve.
    let plain_dir = massing_copy("massing-net-plain");
    std::fs::remove_file(plain_dir.join("world-edits.json")).unwrap();
    let plain = tmp("massing-net-b");
    let r = build(&plain_dir, &plain);
    assert!(r.status.success(), "plain build failed:\n{}", combined(&r));

    let rel = "datapack/data/keep-crawl/function/place_all.mcfunction";
    let a = std::fs::read_to_string(massed.join(rel)).expect("massed place_all");
    let b = std::fs::read_to_string(plain.join(rel)).expect("plain place_all");
    assert_eq!(
        a, b,
        "net-identity massing must restore the exact placement"
    );
}

/// A swap whose replacement cannot re-mate the piece's sockets (a one-socket
/// shrine standing in for a two-socket corridor) is a loud `DW0324`.
#[test]
fn swap_that_cannot_remate_is_dw0324() {
    let dir = massing_copy("massing-bad-swap");
    set_batches(
        &dir,
        "keep-crawl",
        serde_json::json!([{
            "id": "batch/bad-swap",
            "area": "area/keep",
            "edits": [
                { "verb": "swap-piece", "piece": 1,
                  "prefab": "prefab/keep-corridor-straight",
                  "with": "prefab/keep-shrine" }
            ]
        }]),
    );
    let r = build(&dir, &tmp("massing-bad-swap-out"));
    assert_eq!(r.status.code(), Some(3));
    let all = combined(&r);
    assert!(all.contains("DW0324"), "expected DW0324:\n{all}");
    assert!(all.contains("batch/bad-swap"), "names the batch:\n{all}");
}

/// Removing a mid-chain piece (two mated sockets) would orphan its children —
/// only leaves are removable (`DW0324`).
#[test]
fn remove_of_a_non_leaf_is_dw0324() {
    let dir = massing_copy("massing-bad-remove");
    set_batches(
        &dir,
        "keep-crawl",
        serde_json::json!([{
            "id": "batch/bad-remove",
            "area": "area/keep",
            "edits": [
                { "verb": "remove-piece", "piece": 2, "prefab": "prefab/keep-gate-room" }
            ]
        }]),
    );
    let r = build(&dir, &tmp("massing-bad-remove-out"));
    assert_eq!(r.status.code(), Some(3));
    let all = combined(&r);
    assert!(all.contains("DW0324"), "expected DW0324:\n{all}");
    assert!(all.contains("leaf"), "explains the leaf rule:\n{all}");
}

/// Massing verbs targeting a single-`prefab` area are `DW0324` — there is no
/// jigsaw layout to mass.
#[test]
fn massing_a_single_prefab_area_is_dw0324() {
    let dir = massing_copy("massing-single");
    set_batches(
        &dir,
        "keep-crawl",
        serde_json::json!([{
            "id": "batch/single",
            "area": "area/gatehouse",
            "edits": [
                { "verb": "reseed-piece", "piece": 0, "prefab": "prefab/hello-room" }
            ]
        }]),
    );
    let r = build(&dir, &tmp("massing-single-out"));
    assert_eq!(r.status.code(), Some(3));
    let all = combined(&r);
    assert!(all.contains("DW0324"), "expected DW0324:\n{all}");
    assert!(
        all.contains("single"),
        "explains the pool requirement:\n{all}"
    );
}

/// The full-assembly-revalidation proof: a rewire that permanently seals a
/// spine doorway walls off the critical path — the downstream `DW0311` proof
/// re-runs over the massaged world and fails the build, never QA.
#[test]
fn rewiring_a_spine_doorway_sealed_fails_the_walkability_proof() {
    let dir = massing_copy("massing-seal");
    set_batches(
        &dir,
        "keep-crawl",
        serde_json::json!([{
            "id": "batch/wall-it-up",
            "area": "area/keep",
            "edits": [
                { "verb": "rewire-socket", "piece": 1,
                  "prefab": "prefab/keep-corridor-straight",
                  "socket": 1, "state": "sealed" }
            ]
        }]),
    );
    let r = build(&dir, &tmp("massing-seal-out"));
    assert!(!r.status.success(), "a sealed spine must not build green");
    let all = combined(&r);
    assert!(
        all.contains("DW0311") || all.contains("DW0306"),
        "the reachability/walkability proofs catch the sealed doorway:\n{all}"
    );
}

/// Phase ordering is validation-tier: a massing batch after a detailing batch
/// is `DW0162` (exit 1) — the replay applies all massing first, so an
/// interleaved script would misrepresent its own order.
#[test]
fn massing_after_detailing_is_dw0162() {
    let dir = massing_copy("massing-order");
    set_batches(
        &dir,
        "keep-crawl",
        serde_json::json!([
            {
                "id": "batch/dress",
                "area": "area/gatehouse",
                "edits": [
                    { "verb": "select", "name": "region/floor", "shape": {
                        "kind": "box",
                        "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                        "min": [1, 0, 1], "max": [2, 0, 2] }},
                    { "verb": "carve", "region": "region/floor" }
                ]
            },
            {
                "id": "batch/late-massing",
                "area": "area/keep",
                "edits": [
                    { "verb": "reseed-piece", "piece": 2, "prefab": "prefab/keep-gate-room" }
                ]
            }
        ]),
    );
    let r = build(&dir, &tmp("massing-order-out"));
    assert_eq!(r.status.code(), Some(1), "validation-tier rejection");
    let all = combined(&r);
    assert!(all.contains("DW0162"), "expected DW0162:\n{all}");
    assert!(
        all.contains("precede"),
        "explains the ordering rule:\n{all}"
    );
}
