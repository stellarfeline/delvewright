//! **The ballistic instrument, by hand** (spec-0113 §8.2).
//!
//! Not a check: `#[ignore]`d, so no `cargo test` and no CI job runs it
//! (CLAUDE.md — a check guards only what our toolchain owns; vanilla behaviour
//! is established once against the pinned jar and recorded). It writes a build
//! tree holding the instrument, to be run against the pinned server:
//!
//! ```text
//! cargo test -p delvec --test movement_ballistics_instrument -- --ignored
//! EULA=TRUE validation/packtest-run.sh --project dw-ballistics \
//!   --output ./delve-output-ballistics
//! ```
//!
//! The tree is the hello-world fixture's build with
//! [`delvec::compiler::ballistics::instrument_files`] laid into its PackTest
//! datapack. The result and the revision it was measured at live in
//! `docs/reference/body-movement.md`.

mod common;

#[test]
#[ignore = "a one-time instrument against the pinned jar, run by hand"]
fn write_the_ballistic_instrument() {
    let out = common::repo_root().join("validation/delve-output-ballistics");
    let _ = std::fs::remove_dir_all(&out);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .expect("delvec runs");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let files = delvec::compiler::ballistics::instrument_files("hello-world", "The Keeper's Door");
    assert_eq!(files.len(), 2, "the test and its chunk wait");
    for (rel, bytes) in files {
        let p = out.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        eprintln!("wrote {}", p.display());
    }
}
