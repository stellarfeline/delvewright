//! The node bank across runs (`DELVEC_REACH_CACHE`): a build that reads what
//! an earlier run's bank holds emits the same bytes as one that flies every
//! node, a run that grew a bank leaves its file, a run that grew nothing
//! leaves the file as it found it, and a file that does not decode is read as
//! empty (ADR-0006).

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(out: &Path, cache: Option<&Path>) -> Output {
    let pf = common::prefabs_dir();
    let mut cmd = Command::new(BIN);
    cmd.args(["build"])
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(out)
        .arg("--prefabs")
        .arg(&pf)
        .env_remove("DELVEC_REACH_CACHE");
    if let Some(c) = cache {
        cmd.env("DELVEC_REACH_CACHE", c);
    }
    let o = cmd.output().expect("run delvec");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    o
}

/// Every file under `dir`, by its path relative to `dir`, with its bytes.
fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(at).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn banks(cache: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(cache)
        .map(|d| {
            d.map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|x| x == "bank"))
                .map(|p| {
                    let b = std::fs::read(&p).unwrap();
                    (p, b)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[test]
fn a_build_from_an_earlier_run_s_bank_emits_what_flying_emits() {
    let root = tmp("reach-cache");
    let cache = root.join("cache");
    let flown = root.join("flown");
    build(&flown, None);
    assert!(banks(&cache).is_empty(), "no cache named, none written");

    let cold = root.join("cold");
    build(&cold, Some(&cache));
    let written = banks(&cache);
    assert!(
        !written.is_empty(),
        "a run that grew a bank leaves its file"
    );
    assert!(written.iter().all(|(_, b)| b.len() > 8));

    let warm = root.join("warm");
    build(&warm, Some(&cache));
    assert_eq!(
        banks(&cache),
        written,
        "a run that took every node from the file grew nothing and wrote nothing"
    );

    // A file cut short is read as empty, and the run writes a whole one back.
    for (p, b) in &written {
        std::fs::write(p, &b[..b.len() / 2]).unwrap();
    }
    let cut = root.join("cut");
    build(&cut, Some(&cache));
    let rewritten = banks(&cache);
    assert_eq!(rewritten.len(), written.len());
    for ((p, b), (_, was)) in rewritten.iter().zip(&written) {
        assert!(
            b.len() > was.len() / 2,
            "{p:?}: the cut file is replaced by a whole one"
        );
    }

    let want = tree(&flown);
    assert!(!want.is_empty());
    for (name, dir) in [("cold", &cold), ("warm", &warm), ("cut", &cut)] {
        assert!(
            tree(dir) == want,
            "the {name} build's tree differs from the one that flew every node"
        );
    }
}

fn check(campaign: &Path, prefabs: &Path, cache: Option<&str>, threads: Option<&str>) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(["--prefabs"])
        .arg(prefabs)
        .args(["detail", "--check"])
        .arg(campaign)
        .arg("node/annex")
        .env_remove("DELVEC_REACH_CACHE")
        .env_remove("DELVEC_THREADS");
    if let Some(c) = cache {
        cmd.env("DELVEC_REACH_CACHE", c);
    }
    if let Some(t) = threads {
        cmd.env("DELVEC_THREADS", t);
    }
    cmd.output().expect("run delvec")
}

/// **`delvec detail` keeps the bank with the campaign** when the variable is
/// unset: a check leaves its banks under `<campaign>/.out/reach-bank/`, each
/// named for the executable that wrote it, and removes a bank file another
/// executable left there; a second check over the bank says what the first
/// said; the empty string writes nothing. The bank is the same at every
/// thread count: the check that writes nothing runs on one thread and says
/// what the warm check at the host's threads says, the `reach bank:` line
/// included, and a check on one thread into a directory of its own writes
/// the bank file the check at the host's threads wrote, byte for byte.
#[test]
fn detail_keeps_the_bank_with_the_campaign_and_reclaims_another_binary_s() {
    let root = tmp("reach-cache-detail");
    let (campaign, prefabs) = common::gallery_site_plan_detailed(&root);
    let bank = campaign.join(".out/reach-bank");
    // The fixture is made by `detail --all`, which keeps its own bank there.
    let _ = std::fs::remove_dir_all(&bank);

    let off = check(&campaign, &prefabs, Some(""), Some("1"));
    let said = String::from_utf8_lossy(&off.stderr).into_owned();
    assert_eq!(off.status.code(), Some(0), "{said}");
    assert!(banks(&bank).is_empty(), "the empty string writes nothing");
    // The validation's notes are counted, naming the campaign and the library
    // the check was given (never its scratch copy), and not printed.
    assert!(
        said.contains(&format!(
            "the campaign's census; `delvec --prefabs {} validate {}` prints them",
            prefabs.display(),
            campaign.display()
        )),
        "{said}"
    );
    assert!(!said.contains("site-plan placing"), "{said}");

    std::fs::create_dir_all(&bank).unwrap();
    let stale = bank.join("0000000000000000-stale.bank");
    std::fs::write(&stale, b"DWBANK01").unwrap();
    let cold = check(&campaign, &prefabs, None, None);
    assert_eq!(cold.status.code(), Some(0), "{}", text(&cold));
    let written = banks(&bank);
    assert!(
        !written.is_empty(),
        "the check left its bank with the campaign"
    );
    assert!(!stale.exists(), "another executable's bank file is removed");
    let exe = {
        use sha2::{Digest, Sha256};
        let h = Sha256::digest(std::fs::read(BIN).unwrap());
        h[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    for (p, _) in &written {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(&format!("{exe}-")), "{name}");
    }

    let warm = check(&campaign, &prefabs, None, None);
    assert_eq!(warm.status.code(), Some(0), "{}", text(&warm));
    let lines = |o: &Output| -> Vec<String> {
        String::from_utf8_lossy(&o.stderr)
            .lines()
            // What the run cost — its peak, its times, and what its reach
            // lists, snapshot and section stores spilled, read back and held
            // under the budget, and each raise of the stores' need — is the
            // machine's, not the check's.
            .filter(|l| {
                !l.contains("peak rss")
                    && !l.contains(" ms")
                    && !l.starts_with("reach lists:")
                    && !l.starts_with("reach snapshot:")
                    && !l.starts_with("reach budget:")
                    && !l.starts_with("world store pages:")
                    && !l.starts_with("world store need:")
            })
            .map(|l| {
                // The scratch is named for the process.
                l.split("detail-check-")
                    .enumerate()
                    .map(|(i, part)| {
                        if i == 0 {
                            part.to_string()
                        } else {
                            part.trim_start_matches(|c: char| c.is_ascii_digit())
                                .to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("detail-check-")
            })
            .collect()
    };
    assert_eq!(
        lines(&off),
        lines(&warm),
        "a check over the bank says what a check that flew every node says"
    );
    assert_eq!(off.stdout, warm.stdout);
    assert!(
        lines(&off).iter().any(|l| l.starts_with("reach bank:")),
        "the check states its bank"
    );

    let one = root.join("one-thread-bank");
    let alone = check(&campaign, &prefabs, Some(one.to_str().unwrap()), Some("1"));
    assert_eq!(alone.status.code(), Some(0), "{}", text(&alone));
    let name = |v: &[(PathBuf, Vec<u8>)]| -> Vec<(String, Vec<u8>)> {
        v.iter()
            .map(|(p, b)| {
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    b.clone(),
                )
            })
            .collect()
    };
    let (by_one, by_all) = (name(&banks(&one)), name(&written));
    assert_eq!(
        by_one.iter().map(|(n, b)| (n, b.len())).collect::<Vec<_>>(),
        by_all.iter().map(|(n, b)| (n, b.len())).collect::<Vec<_>>(),
        "the bank at one thread and at the host's threads"
    );
    assert!(
        by_one == by_all,
        "the bank file at one thread differs from the one at the host's threads"
    );
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}
