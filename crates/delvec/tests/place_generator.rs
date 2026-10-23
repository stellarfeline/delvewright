//! **A place from its allocation** (spec-0122 §8): the gallery's worked
//! generator, `gallery/overlays/site-plan/design/generator/annex.py`, held to
//! the built binary. `delvec allocation <point> node/annex` → the script → the
//! emitted program, after `delvec fmt`, is the committed `programs/annex.json`
//! byte for byte, and the program passes `delvec detail --check`. A committed
//! program that stops being the script's output reds here; so does a script
//! whose output stops passing the check.

mod common;

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-place-gen-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The interpreter the skill names (`DELVEWRIGHT_PYTHON`), else `python3`.
fn python() -> String {
    std::env::var("DELVEWRIGHT_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

#[test]
fn the_annex_generator_emits_the_committed_program_and_it_passes_the_check() {
    let tmp = tempdir("annex");
    let (campaign, prefabs) = common::gallery_site_plan_detailed(&tmp);
    let cs = campaign.to_str().unwrap();

    // The allocation, saved by the caller: the script runs no binary.
    let alloc = delvec(&["allocation", cs, "node/annex"]);
    assert!(alloc.status.success(), "{}", text(&alloc));
    let alloc_file = tmp.join("annex-allocation.json");
    std::fs::write(&alloc_file, &alloc.stdout).unwrap();

    let script = common::repo_root().join("gallery/overlays/site-plan/design/generator/annex.py");
    let emitted = tmp.join("emitted/annex.json");
    std::fs::create_dir_all(emitted.parent().unwrap()).unwrap();
    let run = Command::new(python())
        .arg(&script)
        .arg("--allocation")
        .arg(&alloc_file)
        .arg("--out")
        .arg(&emitted)
        .output()
        .expect("run the generator");
    assert!(run.status.success(), "{}", text(&run));

    let fmt = delvec(&["fmt", emitted.to_str().unwrap()]);
    assert!(fmt.status.success(), "{}", text(&fmt));
    let committed = std::fs::read(
        common::repo_root().join("gallery/overlays/site-plan/programs/annex.json"),
    )
    .unwrap();
    assert!(
        std::fs::read(&emitted).unwrap() == committed,
        "the committed program is not the generator's output"
    );

    // The emitted program, standing where the campaign keeps it, passes.
    std::fs::copy(&emitted, campaign.join("programs/annex.json")).unwrap();
    let check = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        "--check",
        cs,
        "node/annex",
    ]);
    assert!(check.status.success(), "{}", text(&check));
}
