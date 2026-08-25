//! Stamps the engine revision into the binary at compile time.
//!
//! `detail::engine_revision()` reads `DELVEC_ENGINE_REVISION` through
//! `option_env!` and honestly answers `unstamped` when nobody supplied it.
//! Nobody did: the variable had no writer anywhere in the repository, so the
//! release recipe, CI and a source build all printed `unstamped` — and
//! `unstamped` is exactly the field `walk-record.json` asks a campaign author
//! to copy out of the build output. A field that always holds one constant is
//! not a measurement, and the `DW0842` drift advisory that reads it could only
//! ever say the record was taken on engine `unstamped` and the current engine
//! is `unstamped` too.
//!
//! **A source build is the one path this project guarantees** — clone the repo
//! and build from source is the floor every authoring need has to be runnable
//! on — and a source build is a git checkout by construction. So it is the one
//! case that can answer the question, and this asks it.
//!
//! Three sources, in order, and each is a claim only when it can be made:
//!
//! 1. **An explicit `DELVEC_ENGINE_REVISION` in the environment** wins
//!    unchanged. That is the spelling a release recipe or a container build
//!    uses when it has the revision and the source tree does not carry `.git`.
//! 2. **`git rev-parse HEAD`**, run from this crate's own directory so git
//!    resolves the enclosing repository itself — worktrees included, which the
//!    workers on this project build in. A tree with uncommitted changes is
//!    stamped `<sha>-dirty`, because a measurement taken behind an uncommitted
//!    edit is not a measurement of that revision and a stamp that hides the
//!    difference is worse than none.
//! 3. **Nothing** — no git, no repository, or a source tarball such as the one
//!    crates.io serves, where there is no `.git` to read. Then no variable is
//!    set, `option_env!` yields `None`, and the engine goes on saying
//!    `unstamped` rather than claiming a revision it does not have. That is
//!    the case spec-0050 §2's fallback was written for, and it is preserved
//!    exactly.
//!
//! This never fails a build. A revision it cannot establish is a revision it
//! declines to name.

use std::path::Path;
use std::process::Command;

fn main() {
    // An explicit stamp is a decision someone made; never second-guess it.
    println!("cargo:rerun-if-env-changed=DELVEC_ENGINE_REVISION");
    if let Ok(explicit) = std::env::var("DELVEC_ENGINE_REVISION") {
        let explicit = explicit.trim();
        if !explicit.is_empty() {
            println!("cargo:rustc-env=DELVEC_ENGINE_REVISION={explicit}");
            return;
        }
    }

    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let Some(head) = git(&manifest, &["rev-parse", "HEAD"]) else {
        return; // No git, or no repository: say nothing.
    };

    // Re-run when the checked-out revision moves. `--git-common-dir` rather
    // than `--git-dir` so a linked worktree watches the SHARED refs its HEAD
    // resolves into, and `--git-path HEAD` so the worktree's own HEAD file is
    // the one watched.
    if let Some(head_file) = git(&manifest, &["rev-parse", "--git-path", "HEAD"]) {
        watch(&manifest, &head_file);
    }
    if let Some(common) = git(&manifest, &["rev-parse", "--git-common-dir"]) {
        // A branch checkout's HEAD names a ref whose file moves under it; a
        // detached HEAD names none and the HEAD file above is the whole story.
        if let Some(reference) = git(&manifest, &["symbolic-ref", "--quiet", "HEAD"]) {
            watch(&manifest, &format!("{common}/{reference}"));
            watch(&manifest, &format!("{common}/packed-refs"));
        }
    }

    // Dirty is part of the name, not a footnote. Deliberately NOT bound by a
    // `rerun-if-changed`: no such instruction can cover "any tracked file in
    // the workspace", and pretending otherwise would be a freshness claim this
    // cannot keep. The stamp is context for a diagnostic, never a freshness
    // key — `walk-record.json` keys on the document hashes, which are computed
    // per run from the bytes on disk.
    let dirty = git(&manifest, &["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|s| !s.is_empty());
    let suffix = if dirty { "-dirty" } else { "" };
    println!("cargo:rustc-env=DELVEC_ENGINE_REVISION={head}{suffix}");
}

/// Run a git command in `dir`, returning trimmed stdout when it succeeded and
/// said something.
fn git(dir: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git").current_dir(dir).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    Some(s)
}

/// Ask cargo to re-run this script when `path` changes, if it exists.
///
/// `rerun-if-changed` on a path that does not exist makes cargo re-run the
/// script on every build, so an absent ref file is skipped rather than watched.
fn watch(dir: &str, path: &str) {
    let p = Path::new(path);
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(dir).join(p)
    };
    if p.exists() {
        println!("cargo:rerun-if-changed={}", p.display());
    }
}
