//! **No test shares a scratch path with another run on the same machine.**
//!
//! `std::env::temp_dir()` is one directory for every `cargo test` on the host,
//! across worktrees. A test that joins a fixed name onto it, and removes the
//! result before writing, deletes the directory another run is using, and both
//! runs red with a missing-file error. The process id is the only component
//! that differs between two concurrent runs, so every `temp_dir().join(..)`
//! argument must contain `process::id()`. A directory under
//! `CARGO_TARGET_TMPDIR` is per target directory and needs nothing.
//!
//! The scan covers every `.rs` file under `crates/*/tests` and `crates/*/src`
//! except this one, and states how many sites it bound.

use std::path::{Path, PathBuf};

const NEEDLE: &str = concat!("temp_dir", "()");
const PID: &str = concat!("process::", "id()");

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The text of the `.join( .. )` argument that follows a `temp_dir()` at
/// `from`, or `None` when no join follows directly.
fn join_argument(text: &str, from: usize) -> Option<&str> {
    let rest = &text[from..];
    let open = rest.find(".join(")?;
    if open > 5 {
        return None;
    }
    let start = open + ".join(".len();
    let mut depth = 1usize;
    for (i, c) in rest[start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[start..start + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Every site in `text` that names a shared scratch directory without the
/// process id: `(line, argument)`; and the number of sites seen.
fn offenders(text: &str) -> (Vec<(usize, String)>, usize) {
    let mut bad = Vec::new();
    let mut seen = 0;
    for (at, _) in text.match_indices(NEEDLE) {
        seen += 1;
        let line = text[..at].matches('\n').count() + 1;
        match join_argument(text, at + NEEDLE.len()) {
            Some(arg) if arg.contains(PID) => {}
            Some(arg) => bad.push((line, arg.split_whitespace().collect::<Vec<_>>().join(" "))),
            None => bad.push((line, String::from("(no join: the bare shared directory)"))),
        }
    }
    (bad, seen)
}

#[test]
fn every_shared_temp_path_carries_the_process_id() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let me = Path::new(file!()).file_name().unwrap();
    let mut files = Vec::new();
    for member in std::fs::read_dir(crates).unwrap() {
        let member = member.unwrap().path();
        for sub in ["tests", "src"] {
            if member.join(sub).is_dir() {
                rs_files(&member.join(sub), &mut files);
            }
        }
    }
    let mut seen = 0;
    let mut report = Vec::new();
    for f in files.iter().filter(|f| f.file_name() != Some(me)) {
        let (bad, n) = offenders(&std::fs::read_to_string(f).unwrap());
        seen += n;
        for (line, arg) in bad {
            report.push(format!("{}:{line}: join({arg})", f.display()));
        }
    }
    eprintln!(
        "scratch paths: {seen} temp_dir() sites bound in {} files, {} without the process id",
        files.len(),
        report.len()
    );
    assert!(
        seen > 0,
        "the scan bound no temp_dir() site: it checked nothing"
    );
    assert!(
        report.is_empty(),
        "a fixed path under the shared temp directory is deleted by a concurrent run; \
         add `std::process::id()` to the name:\n{}",
        report.join("\n")
    );
}

#[test]
fn the_scan_reds_on_a_fixed_name_and_passes_on_a_pid_name() {
    let fixed = concat!("let d = std::env::", "temp_dir", "().join(\"dw-x\");");
    let pid = concat!(
        "let d = std::env::temp_dir",
        "().join(format!(\"dw-x-{}\", std::process::",
        "id()));"
    );
    assert_eq!(offenders(fixed).0.len(), 1);
    assert!(offenders(pid).0.is_empty());
    assert_eq!(offenders(pid).1, 1);
}
