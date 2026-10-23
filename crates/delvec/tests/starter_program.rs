//! **A place author starts from a program that already passes** (spec-0122
//! §6.3, §8, criterion 18). `scripts/starter.py`, the starter generator
//! `handout.py` copies beside every handout, is run UNEDITED on the allocation
//! of every place the gallery's site-plan points allocate a box to; its program,
//! standing as that place's program in a scratch copy, passes `delvec detail
//! --check`. A place class the starter cannot answer reds here, naming the
//! place and the verdict — it is fixed in the starter, never excluded.

mod common;

use std::path::{Path, PathBuf};
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
    let p = std::env::temp_dir().join(format!("dw-starter-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The interpreter the skill names (`DELVEWRIGHT_PYTHON`), else `python3`.
fn python() -> String {
    std::env::var("DELVEWRIGHT_PYTHON").unwrap_or_else(|_| "python3".to_string())
}

fn starter() -> PathBuf {
    common::repo_root().join(".claude/skills/delvewright/skills/new-delve/scripts/starter.py")
}

/// The `site-plan-solid` point, materialised by the gallery's own domain tool
/// and detailed into `prefabs` beside the open point's pieces, as
/// `common::gallery_site_plan_detailed` details the open one.
fn solid_point_detailed(root: &Path, prefabs: &Path) -> PathBuf {
    let campaign = root.join("site-plan-solid");
    let out = Command::new(python())
        .arg(common::repo_root().join("tools/ci/gallery_domain.py"))
        .args(["materialise", "--point", "site-plan-solid"])
        .arg(&campaign)
        .output()
        .expect("run gallery_domain.py");
    assert!(out.status.success(), "{}", text(&out));
    let stems: Vec<String> = std::fs::read_dir(campaign.join("programs"))
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension().and_then(|x| x.to_str()) == Some("json"))
                .then(|| p.file_stem().unwrap().to_string_lossy().to_string())
        })
        .collect();
    common::patch_file(&campaign.join("detail-plan.json"), |v| {
        v["content"]["details"].as_array_mut().unwrap().retain(|r| {
            let place = r["place"].as_str().unwrap();
            !stems.iter().any(|s| place == format!("node/{s}"))
        });
    });
    let all = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        campaign.to_str().unwrap(),
        "--all",
    ]);
    assert!(all.status.success(), "{}", text(&all));
    campaign
}

/// Every place `campaign` allocates a box to, its starter program run unedited
/// and checked. Returns `(passed, places, failures)`.
fn judge_every_place(tmp: &Path, campaign: &Path, prefabs: &Path) -> (usize, usize, Vec<String>) {
    let cs = campaign.to_str().unwrap();
    let all = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "allocation",
        cs,
        "--all",
    ]);
    assert!(all.status.success(), "{}", text(&all));
    let allocations: Vec<serde_json::Value> = serde_json::from_slice(&all.stdout).unwrap();
    assert!(!allocations.is_empty(), "the point allocates no box");
    let mut passed = 0;
    let mut failures = Vec::new();
    for a in &allocations {
        let place = a["place"].as_str().unwrap();
        let stem = place.strip_prefix("node/").unwrap();
        let alloc_file = tmp.join(format!(
            "{}-{stem}.allocation.json",
            campaign.file_name().unwrap().to_string_lossy()
        ));
        std::fs::write(&alloc_file, serde_json::to_vec_pretty(a).unwrap()).unwrap();
        let program = campaign.join("programs").join(format!("{stem}.json"));
        std::fs::create_dir_all(program.parent().unwrap()).unwrap();
        let before = std::fs::read(&program).ok();
        let run = Command::new(python())
            .arg(starter())
            .arg("--allocation")
            .arg(&alloc_file)
            .arg("--out")
            .arg(&program)
            .output()
            .expect("run the starter");
        let verdict = if run.status.success() {
            let check = delvec(&[
                "--prefabs",
                prefabs.to_str().unwrap(),
                "detail",
                "--check",
                cs,
                place,
            ]);
            if !check.status.success() {
                Err(format!(
                    "`detail --check` exited {:?}:\n{}",
                    check.status.code(),
                    text(&check)
                ))
            } else {
                // The commit too, battery included, in a scratch copy of the
                // campaign and the library. The empty place is not the
                // building, so the battery's whole-map verdicts on it (a vista,
                // a crossing, a gate the skeleton does not yet separate) are
                // the author's to answer; what the starter must never hand an
                // author is a refusal of its OWN declarations — `DW0885`, a
                // shown side it leaves empty or an open-air side it does not
                // declare.
                let scratch = tmp.join(format!(
                    "commit-{}-{stem}",
                    campaign.file_name().unwrap().to_string_lossy()
                ));
                let _ = std::fs::remove_dir_all(&scratch);
                common::copy_dir_all(campaign, &scratch.join("campaign"));
                common::copy_dir_all(prefabs, &scratch.join("prefabs"));
                let commit = delvec(&[
                    "--prefabs",
                    scratch.join("prefabs").to_str().unwrap(),
                    "detail",
                    scratch.join("campaign").to_str().unwrap(),
                    place,
                ]);
                let _ = std::fs::remove_dir_all(&scratch);
                let said = text(&commit);
                let own: Vec<&str> = said
                    .lines()
                    .filter(|l| l.starts_with("DW0885 [error]"))
                    .collect();
                if !said.contains("written from") {
                    Err(format!(
                        "`detail --check` passed and the commit wrote nothing (exit {:?}):\n{said}",
                        commit.status.code()
                    ))
                } else if !own.is_empty() {
                    Err(format!(
                        "the commit's battery refuses the starter's own `shown_faces`:\n{}",
                        own.join("\n")
                    ))
                } else {
                    Ok(())
                }
            }
        } else {
            Err(format!(
                "the starter exited {:?}:\n{}",
                run.status.code(),
                text(&run)
            ))
        };
        // The committed program goes back: each place is judged with every
        // other place standing as the gallery commits it.
        match before {
            Some(bytes) => std::fs::write(&program, bytes).unwrap(),
            None => std::fs::remove_file(&program).unwrap(),
        }
        match verdict {
            Ok(()) => {
                passed += 1;
                eprintln!("starter: {place} of `{cs}` passes `detail --check`, and its commit writes it with no `DW0885`");
            }
            Err(why) => failures.push(format!("{place}: {why}")),
        }
    }
    (passed, allocations.len(), failures)
}

#[test]
fn the_starter_passes_the_check_unedited_on_every_place_of_the_gallery_site_plans() {
    let tmp = tempdir("gallery");
    let (open, prefabs) = common::gallery_site_plan_detailed(&tmp);
    let solid = solid_point_detailed(&tmp, &prefabs);
    let mut passed = 0;
    let mut places = 0;
    let mut failures = Vec::new();
    for campaign in [&open, &solid] {
        let (p, n, f) = judge_every_place(&tmp, campaign, &prefabs);
        passed += p;
        places += n;
        failures.extend(f);
    }
    eprintln!("starter binding: {passed} of {places} place(s) pass --check, committed with no DW0885");
    assert!(places > 0, "zero places judged: the binding is vacuous");
    assert!(
        failures.is_empty(),
        "{} of {places} place(s) refused the starter's program:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
