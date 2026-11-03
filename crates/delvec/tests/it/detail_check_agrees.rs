//! **`delvec detail --check` and the commit are two runs of one verb**
//! (spec-0122 §6.1). `--check` is the commit's own `prepare`, `judge_run`,
//! write and battery, the write landing in a scratch copy of the campaign and
//! the library, so over every place and every refusal the two must say the
//! same thing, code for code and exit for exit, and the check must leave the
//! library and the campaign byte-identical.
//!
//! What is compared: the DW codes each run prints up to the point the commit
//! writes — the commit's `detail: N place(s) detailed` line — and the exit of
//! the whole run, the battery's included. A refusal only the battery raises
//! (`DW1002`, a falling block over air) is `--check`'s too, at the commit's
//! exit.

use crate::common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

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
    let p = std::env::temp_dir().join(format!("dw-detail-check-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Every file under `dir`, recursively, with its bytes' hash.
fn snapshot(dir: &Path) -> BTreeSet<(String, String)> {
    use sha2::{Digest, Sha256};
    let mut out = BTreeSet::new();
    // The campaign's `.out/` is the runs' own, and no copy of the campaign
    // carries it (`detail`'s scratch copy leaves it behind): it holds the
    // reach graph's node bank, which a check keeps there by design.
    let own = dir.join(".out");
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p == own {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                let h = format!("{:x}", Sha256::digest(std::fs::read(&p).unwrap()));
                out.insert((p.strip_prefix(dir).unwrap().display().to_string(), h));
            }
        }
    }
    out
}

/// Every `DWnnnn` in `text`, in order.
fn dw_codes(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 6 <= bytes.len() {
        if &bytes[i..i + 2] == b"DW" && bytes[i + 2..i + 6].iter().all(u8::is_ascii_digit) {
            found.push(text[i..i + 6].to_string());
            i += 6;
        } else {
            i += 1;
        }
    }
    found
}

/// What a run said, as two code lists: the diagnostics it printed on stdout,
/// in order, and the codes its stderr names up to the commit's write line —
/// and whether it reached that line (wrote).
fn codes(out: &Output) -> (Vec<String>, Vec<String>, bool) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let diags: Vec<String> = stdout
        .lines()
        .filter_map(|l| l.get(..7))
        .filter(|head| head.ends_with(' '))
        .flat_map(|head| dw_codes(&head[..6]))
        .collect();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut said = Vec::new();
    let mut wrote = false;
    for line in stderr.lines() {
        if (line.starts_with("detail: ") && line.contains("place(s) detailed of"))
            || line.starts_with("detail --check: the commit written into a scratch copy")
        {
            wrote = true;
            break;
        }
        said.extend(dw_codes(line));
    }
    (diags, said, wrote)
}

/// One place, checked then committed: the check writes nothing, and the two
/// print the same codes. Returns the shared code list.
fn agree(campaign: &Path, prefabs: &Path, place: &str) -> (Vec<String>, i32, i32, String) {
    let cs = campaign.to_str().unwrap();
    let ps = prefabs.to_str().unwrap();
    let before = (snapshot(prefabs), snapshot(campaign));
    let check = delvec(&["--prefabs", ps, "detail", "--check", cs, place]);
    assert_eq!(
        before,
        (snapshot(prefabs), snapshot(campaign)),
        "`--check {place}` wrote something: {}",
        text(&check)
    );
    let commit = delvec(&["--prefabs", ps, "detail", cs, place]);
    let ((c_diags, c_said, _), (k_diags, k_said, wrote)) = (codes(&check), codes(&commit));
    // stdout is one stream in print order: the shared stage's diagnostics
    // come first, and a commit that wrote adds only the battery's after them.
    let k_shared = if wrote {
        &k_diags[..c_diags.len().min(k_diags.len())]
    } else {
        &k_diags[..]
    };
    assert!(
        c_said == k_said && c_diags.as_slice() == k_shared,
        "{place}: `--check` and the commit disagree\n--check: {}\ncommit: {}",
        text(&check),
        text(&commit)
    );
    let mut c = c_diags.clone();
    c.extend(c_said);
    let check_exit = check.status.code().unwrap_or(-1);
    let commit_exit = commit.status.code().unwrap_or(-1);
    if check_exit == 0 {
        assert!(
            text(&check).contains("peak rss: ") && text(&check).contains("the commit's battery: "),
            "the check states its battery and its peak: {}",
            text(&check)
        );
    }
    // Every refusal the commit gives, before its write or in its battery,
    // the check gives at the same exit.
    assert_eq!(
        check_exit,
        commit_exit,
        "{place}: --check: {}\ncommit: {}",
        text(&check),
        text(&commit)
    );
    let shown = format!("--check: {}\ncommit: {}", text(&check), text(&commit));
    (c, check_exit, commit_exit, shown)
}

fn stems(campaign: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(campaign.join("programs"))
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            (p.extension().and_then(|x| x.to_str()) == Some("json"))
                .then(|| p.file_stem().unwrap().to_string_lossy().to_string())
        })
        .collect();
    out.sort();
    out
}

fn patch(path: &Path, f: impl FnOnce(&mut Value)) {
    common::patch_file(path, f);
}

/// One test per program place of the gallery's site-plan point: the place is
/// checked, then committed, and the two agree. The places are named here so
/// that each is its own process with its own time (spec-0126 §4.3, §5), and
/// the last test holds the list to the point: a program added to the gallery
/// reds there until it has a test of its own.
macro_rules! place_agrees {
    ($($name:ident => $place:literal),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                let tmp = tempdir(concat!("agree-", $place));
                let (campaign, prefabs) = common::gallery_site_plan_detailed(&tmp);
                let places = stems(&campaign);
                assert!(
                    places.iter().any(|s| s == $place),
                    "node/{} is not a program place of the gallery's site-plan point: {places:?}",
                    $place
                );
                let (_, check, commit, shown) =
                    agree(&campaign, &prefabs, concat!("node/", $place));
                assert_eq!((check, commit), (0, 0), "node/{}: {shown}", $place);
                eprintln!("agreement binding: node/{} checked and committed", $place);
            }
        )*

        #[test]
        fn every_program_place_of_the_gallery_has_its_agreement_test() {
            let tmp = tempdir("agree-places");
            let (campaign, _) = common::gallery_site_plan_detailed(&tmp);
            let places = stems(&campaign);
            assert!(places.len() >= 4, "the binding: {places:?}");
            let named: Vec<String> = vec![$($place.to_string()),*];
            assert_eq!(
                places, named,
                "the program places of the gallery's site-plan point and the places tested above differ"
            );
            eprintln!(
                "agreement binding: {n} of {n} program place(s) have a check-and-commit test",
                n = places.len()
            );
        }
    };
}

place_agrees! {
    check_and_commit_agree_on_annex => "annex",
    check_and_commit_agree_on_beacon => "beacon",
    check_and_commit_agree_on_causeway => "causeway",
    check_and_commit_agree_on_far_hall => "far-hall",
    check_and_commit_agree_on_gantry => "gantry",
    check_and_commit_agree_on_loft => "loft",
    check_and_commit_agree_on_near_hall => "near-hall",
    check_and_commit_agree_on_perch => "perch",
}

/// Each refusal over its own copy of the committed whole (the fixture is this
/// process's own copy already).
fn committed_copy(name: &str) -> (PathBuf, PathBuf) {
    let tmp = tempdir(name);
    common::gallery_site_plan_detailed(&tmp)
}

// DW0843 — an undersize piece: the perch's box shrinks under its
// committed piece, and the annex's check judges the whole that holds it.
#[test]
fn check_and_commit_agree_on_an_undersize_piece_dw0843() {
    let (c, p) = committed_copy("dw0843");
    patch(&c.join("site-plan.json"), |v| {
        let boxes = v["content"]["boxes"].as_array_mut().unwrap();
        let b = boxes
            .iter_mut()
            .find(|b| b["node"] == "node/perch")
            .unwrap();
        b["extent"] = json!([8, 6]);
    });
    let (k, _, _, shown) = agree(&c, &p, "node/annex");
    assert!(k.contains(&"DW0843".to_string()), "{k:?}\n{shown}");
}

// DW0844 — an unanswered seam: the annex's arch cut one cell along the
// wall from where the plan hands it.
#[test]
fn check_and_commit_agree_on_an_unanswered_seam_dw0844() {
    let (c, p) = committed_copy("dw0844");
    patch(&c.join("programs/annex.json"), |v| {
        let size = &mut v["rules"]["east_band"][0]["body"]["sizes"][0]["blocks"];
        *size = json!({"expr": "arith", "lhs": size.clone(), "op": "add",
                       "rhs": {"expr": "int", "value": 1}});
    });
    let (k, _, _, shown) = agree(&c, &p, "node/annex");
    assert!(k.contains(&"DW0844".to_string()), "{k:?}\n{shown}");
}

// DW0845 — an owed name unbound: the annex's own place mark removed.
#[test]
fn check_and_commit_agree_on_an_owed_name_unbound_dw0845() {
    let (c, p) = committed_copy("dw0845");
    patch(&c.join("programs/annex.json"), |v| {
        let body = v["rules"]["room"][0]["body"]["body"].clone();
        v["rules"]["room"][0]["body"] = body;
    });
    let (k, _, _, shown) = agree(&c, &p, "node/annex");
    assert!(k.contains(&"DW0845".to_string()), "{k:?}\n{shown}");
}

// DW0773 — a program not in canonical form: the annex's program written
// on one line, the same document. Refused where it enters, by both runs.
#[test]
fn check_and_commit_agree_on_a_non_canonical_program_dw0773() {
    let (c, p) = committed_copy("dw0773");
    let path = c.join("programs/annex.json");
    let doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_string(&doc).unwrap()).unwrap();
    let (k, check, _, shown) = agree(&c, &p, "node/annex");
    assert!(
        k.contains(&"DW0773".to_string()) && check != 0,
        "{k:?}\n{shown}"
    );
}

// DW1002 — a refusal only the commit's battery raises: the annex's
// ceiling laid in gravel, which falls into the room the moment the load
// writes it. Its own gates pass; the battery over the whole refuses.
#[test]
fn check_and_commit_agree_on_the_battery_s_own_refusal_dw1002() {
    let (c, p) = committed_copy("dw1002");
    patch(&c.join("programs/annex.json"), |v| {
        v["palette"]["ceiling"] = json!("minecraft:gravel");
    });
    let (_, check, commit, shown) = agree(&c, &p, "node/annex");
    assert!(check != 0 && check == commit, "{shown}");
    let said = shown.split("\ncommit: ").next().unwrap_or_default();
    assert!(
        said.contains("DW1002"),
        "`--check` names the battery's refusal: {shown}"
    );
}

#[test]
fn check_beside_all_is_refused_naming_the_pair_and_writes_nothing() {
    let tmp = tempdir("check-all");
    let campaign = tmp.join("site-plan");
    common::gallery_site_plan_at(&campaign);
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let before = (snapshot(&prefabs), snapshot(&campaign));
    let out = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        "--check",
        "--all",
        campaign.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let t = text(&out);
    assert!(t.contains("`--check` and `--all`"), "{t}");
    assert_eq!(before, (snapshot(&prefabs), snapshot(&campaign)));
}
