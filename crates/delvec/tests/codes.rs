//! `delvec codes`: the DW-code registry the binary prints is exactly the codes
//! the source declares.
//!
//! The comparison is `tools/ci/check-dw-codes.py --delvec`, run against the
//! binary cargo built for this test — the one reading of a declaration, never a
//! second parse rule here. A declaration written outside `dw_code!` is in the
//! source and missing from the registry, and this test reds.

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
