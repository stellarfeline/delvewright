//! **Every `delvec` command the engine's text names is a command the binary has.**
//!
//! A diagnostic's remedy that names a verb is a move its reader takes next; a
//! verb the binary does not have sends the reader nowhere. So every backticked
//! `` `delvec <sub> [<sub>]` `` in the workspace's Rust sources — diagnostic
//! messages, remedies, help text and doc comments alike — is resolved against the
//! command tree the built binary prints under `--help`, walked recursively.
//!
//! A source line is read the way its string is assembled: a `\`-continued string
//! literal and a comment that wraps onto the next line are joined first, so a
//! command split across two source lines is read whole.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The subcommands `delvec <path…> --help` lists, `help` excluded.
fn subcommands(path: &[String]) -> Vec<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(path)
        .arg("--help")
        .output()
        .expect("delvec runs");
    assert!(out.status.success(), "delvec {path:?} --help: {out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    let mut subs = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line == "Commands:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if !line.starts_with("  ") {
            break;
        }
        if line.starts_with("   ") {
            continue;
        }
        let name = line.split_whitespace().next().unwrap().to_string();
        if name != "help" {
            subs.push(name);
        }
    }
    subs
}

fn command_tree() -> BTreeSet<Vec<String>> {
    let mut tree = BTreeSet::new();
    let mut stack: Vec<Vec<String>> = vec![Vec::new()];
    while let Some(path) = stack.pop() {
        for sub in subcommands(&path) {
            let mut p = path.clone();
            p.push(sub);
            tree.insert(p.clone());
            stack.push(p);
        }
    }
    tree
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Join `\`-continued string literals and wrapped comments into one line: a
/// line ending in `\` takes the next line with its indent dropped, and a line
/// whose successor is a comment takes that comment's text after a space.
fn joined(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        out.push_str(line.strip_suffix('\\').unwrap_or(line));
        let mut continued = line.ends_with('\\');
        while let Some(next) = lines.peek() {
            let t = next.trim_start();
            if continued {
                out.push_str(t.strip_suffix('\\').unwrap_or(t));
                continued = t.ends_with('\\');
            } else if let Some(c) = ["//!", "///", "//"].iter().find(|c| t.starts_with(**c)) {
                out.push(' ');
                out.push_str(t[c.len()..].trim_start());
            } else {
                break;
            }
            lines.next();
        }
        out.push('\n');
    }
    out
}

/// Every backticked `delvec` in `text` with the (at most two) lower-case
/// identifier words that follow it.
fn named_commands(text: &str) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    for (i, _) in text.match_indices("`delvec ") {
        let rest = &text[i + "`delvec ".len()..];
        let mut words = Vec::new();
        for raw in rest.split(' ').take(2) {
            let w = raw.trim_end_matches(['`', ',', '.', ')', '\'', ';', ':']);
            let ok = w.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && w.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !ok {
                break;
            }
            words.push(w.to_string());
            // A word the closing backtick follows ends the command.
            if raw[w.len()..].starts_with('`') {
                break;
            }
        }
        if !words.is_empty() {
            found.push(words);
        }
    }
    found
}

#[test]
fn every_backticked_delvec_command_in_the_sources_exists() {
    let tree = command_tree();
    let tops: BTreeSet<&str> = tree.iter().map(|p| p[0].as_str()).collect();
    let groups: BTreeSet<&str> = tree
        .iter()
        .filter(|p| p.len() == 2)
        .map(|p| p[0].as_str())
        .collect();
    assert!(
        tree.len() > 20 && groups.contains("prefab"),
        "the walked command tree is too small to be the binary's: {} path(s)",
        tree.len()
    );

    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    rust_sources(crates, &mut files);

    let mut named = 0usize;
    let mut dead = Vec::new();
    for f in &files {
        let text = joined(&std::fs::read_to_string(f).unwrap());
        for words in named_commands(&text) {
            named += 1;
            let top = words[0].as_str();
            let resolves = tops.contains(top)
                && (!groups.contains(top)
                    || words.len() < 2
                    || tree.contains(&vec![words[0].clone(), words[1].clone()]));
            if !resolves {
                dead.push(format!(
                    "{}: `delvec {}`",
                    f.strip_prefix(crates).unwrap().display(),
                    words.join(" ")
                ));
            }
        }
    }
    eprintln!(
        "named-command binding: {named} backticked `delvec …` mention(s) over {} source file(s), \
         resolved against {} command path(s); {} dead",
        files.len(),
        tree.len(),
        dead.len()
    );
    assert!(
        named > 100,
        "only {named} mention(s) found: the scan binds to nothing"
    );
    assert!(
        dead.is_empty(),
        "these name a `delvec` command the binary does not have:\n{}",
        dead.join("\n")
    );
}
