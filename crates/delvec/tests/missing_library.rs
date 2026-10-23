//! **A verb that reads pieces, pointed at no library, says so** (spec-0122
//! §7.5): with no `--prefabs` from a directory with no `campaigns/prefabs` —
//! a creator's working directory — every piece-reading verb refuses naming
//! `--prefabs` and `DELVEWRIGHT_PREFABS`, at exit 1 on the compiler's surface
//! and inside the admission surface's input diagnostic at its own exit, and
//! never reports `internal error: cannot read prefabs dir`.

mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-missing-lib-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn assert_named(out: &Output, label: &str) {
    let t = text(out);
    assert!(t.contains("--prefabs"), "{label}: {t}");
    assert!(t.contains("DELVEWRIGHT_PREFABS"), "{label}: {t}");
    assert!(!t.contains("internal error"), "{label}: {t}");
}

#[test]
fn every_piece_reading_verb_refuses_an_absent_library_by_name() {
    let tmp = tempdir("cwd");
    let campaign = tmp.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &campaign);
    let site = tmp.join("site-plan");
    common::gallery_site_plan_at(&site);
    let out_dir = tmp.join("out");
    assert!(!tmp.join("campaigns/prefabs").exists());

    let run = |args: &[&str]| {
        Command::new(BIN)
            .current_dir(&tmp)
            .args(args)
            .output()
            .unwrap()
    };
    let cs = campaign.to_str().unwrap();
    let ss = site.to_str().unwrap();
    let os = out_dir.to_str().unwrap();
    let compiler: [(&str, Vec<&str>); 5] = [
        ("validate", vec!["validate", cs]),
        ("analyze", vec!["analyze", cs]),
        ("build", vec!["build", cs, "-o", os]),
        (
            "detail --check",
            vec!["detail", "--check", ss, "node/annex"],
        ),
        ("detail", vec!["detail", ss, "node/annex"]),
    ];
    for (label, args) in &compiler {
        let out = run(args);
        assert_eq!(out.status.code(), Some(1), "{label}: {}", text(&out));
        assert_named(&out, label);
    }
    // Neither the check nor the commit made a library of its own.
    assert!(!tmp.join("campaigns").exists());

    // The admission surface keeps its own exit table: its input diagnostic.
    let out = run(&["prefab", "anchors"]);
    assert_ne!(out.status.code(), Some(0), "{}", text(&out));
    assert!(text(&out).contains("DW0732"), "{}", text(&out));
    assert_named(&out, "prefab anchors");
}
