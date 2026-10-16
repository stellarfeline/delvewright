//! `delvec codes`: the DW-code registry the binary prints is exactly the codes
//! the source declares.
//!
//! The comparison is `tools/ci/check-dw-codes.py --delvec`, run against the
//! binary cargo built for this test — the one reading of a declaration, never a
//! second parse rule here. A declaration written outside `dw_code!` is in the
//! source and missing from the registry, and this test reds.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn rows() -> Vec<serde_json::Value> {
    let out = Command::new(BIN)
        .arg("codes")
        .output()
        .expect("run delvec codes");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("utf-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect()
}

#[test]
fn every_line_is_a_code_with_its_properties_in_code_order() {
    let rows = rows();
    assert!(rows.len() > 100, "{} code(s) listed", rows.len());
    let mut previous = String::new();
    for row in &rows {
        let code = row["code"].as_str().expect("code");
        assert!(
            code.len() == 6
                && code.starts_with("DW")
                && code[2..].bytes().all(|b| b.is_ascii_digit()),
            "{row}"
        );
        assert!(code >= previous.as_str(), "{code} after {previous}");
        previous = code.to_string();
        assert!(row["name"].as_str().is_some_and(|n| !n.is_empty()), "{row}");
        assert!(
            row["module"].as_str().is_some_and(|m| !m.is_empty()),
            "{row}"
        );
        let tier = &row["tier"];
        assert!(
            tier.is_null() || tier == "Analysis" || tier == "Build",
            "{row}"
        );
        assert_eq!(tier.is_null(), row["subject"].is_null(), "{row}");
    }
}

#[test]
fn the_registry_is_exactly_the_declared_codes() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the repository root");
    let out = Command::new("python3")
        .arg(repo.join("tools/ci/check-dw-codes.py"))
        .args(["--delvec", BIN])
        .current_dir(repo)
        .output()
        .expect("python3 is required to read the declarations");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stdout}{stderr}");
    assert!(
        stdout.contains(&format!(
            "registry: `delvec codes` lists {} code(s)",
            rows().len()
        )),
        "{stdout}"
    );
}

/// **A build states the codes its engine carries.** `manifest.json` `dw_codes`
/// is what `tools/creator/staging-gate.py` judges a ledger row's `dw` carrier
/// against — the engine that built the tree, never the source the gate reads —
/// so it must be exactly this binary's registry: sorted, once each, nothing
/// added or lost.
#[test]
fn a_build_manifest_states_exactly_the_registry() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("codes-manifest");
    let _ = std::fs::remove_dir_all(&out);
    let r = Command::new(BIN)
        .arg("build")
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .expect("run delvec build");
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("manifest.json")).expect("manifest"))
            .expect("manifest parses");
    let stated: Vec<&str> = manifest["dw_codes"]
        .as_array()
        .expect("`dw_codes` is a list")
        .iter()
        .map(|c| c.as_str().expect("a code is a string"))
        .collect();
    let registry: BTreeSet<String> = rows()
        .iter()
        .map(|row| row["code"].as_str().expect("code").to_string())
        .collect();
    assert_eq!(
        stated,
        registry.iter().map(String::as_str).collect::<Vec<_>>(),
        "manifest `dw_codes` is not the registry, sorted and once each"
    );
}
