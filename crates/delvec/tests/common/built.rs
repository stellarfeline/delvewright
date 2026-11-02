//! **One build per world, on disk** (spec-0126 §4.2).
//!
//! An expensive fixture — a materialised campaign, a built world, an assembled
//! gallery point — is built once and read by every test that needs it, in any
//! process. The cache directory is keyed by the SHA-256 of the fixture's inputs,
//! of the `delvec` binary that builds it, and of the test executable whose code
//! is the recipe; determinism (ADR-0006) is what makes a hit correct: the same
//! inputs, the same binary and the same recipe give the same bytes.
//!
//! The build runs into a staging directory under an exclusive file lock and is
//! renamed into place whole, so no process ever reads a half-built fixture. Each
//! build appends a line to `<key>.builds` beside the cache, which is how a test
//! proves a second call did not build again. The cache lives under the target
//! directory, is never committed, and `cargo clean` clears it.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// What a fixture is built from, folded into its key in the order given.
#[derive(Default)]
pub struct Inputs {
    hasher: Sha256,
}

impl Inputs {
    pub fn new() -> Self {
        Self::default()
    }

    fn field(mut self, tag: &str, bytes: &[u8]) -> Self {
        self.hasher.update(tag.as_bytes());
        self.hasher.update((bytes.len() as u64).to_le_bytes());
        self.hasher.update(bytes);
        self
    }

    /// A literal the recipe depends on (a name, a flag, a seed).
    pub fn text(self, s: &str) -> Self {
        self.field("text", s.as_bytes())
    }

    /// One file's bytes, under its name.
    pub fn file(self, path: &Path) -> Self {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        self.field("file", path.file_name().unwrap().as_encoded_bytes())
            .field("bytes", &bytes)
    }

    /// Every file git tracks under `pathspec` in the repository at `root`, by
    /// path and working-tree bytes, in git's order: a fixture derived from the
    /// tree is keyed by what the tree holds, never by build output beside it.
    pub fn tracked(mut self, root: &Path, pathspec: &str) -> Self {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["ls-files", "-z", "--", pathspec])
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git ls-files {pathspec} failed");
        let listed: Vec<&[u8]> = out.stdout.split(|b| *b == 0).filter(|p| !p.is_empty()).collect();
        assert!(!listed.is_empty(), "git tracks nothing under {pathspec}");
        for rel in listed {
            let rel = String::from_utf8_lossy(rel).to_string();
            let path = root.join(&rel);
            // A tracked file deleted in the working tree is part of the state.
            let bytes = std::fs::read(&path).unwrap_or_default();
            self = self.field("path", rel.as_bytes()).field("bytes", &bytes);
        }
        self
    }

    /// Every file under `dir`, by relative path and bytes, in path order.
    pub fn dir(mut self, dir: &Path) -> Self {
        let mut files = Vec::new();
        walk(dir, &mut files);
        files.sort();
        for f in files {
            let rel = f.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
            let bytes = std::fs::read(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            self = self.field("path", rel.as_bytes()).field("bytes", &bytes);
        }
        self
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn file_sha(path: &Path) -> String {
    let mut h = Sha256::new();
    let mut f = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    std::io::copy(&mut f, &mut h).unwrap();
    hex(&h.finalize())
}

/// The SHA-256 of the `delvec` binary the tests run, once per process.
pub fn delvec_sha() -> &'static str {
    static SHA: OnceLock<String> = OnceLock::new();
    SHA.get_or_init(|| file_sha(Path::new(env!("CARGO_BIN_EXE_delvec"))))
}

/// The SHA-256 of this test executable — the recipe — once per process.
fn recipe_sha() -> &'static str {
    static SHA: OnceLock<String> = OnceLock::new();
    SHA.get_or_init(|| file_sha(&std::env::current_exe().expect("the test executable")))
}

/// Where every built fixture lives.
pub fn cache_root() -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("built")
}

/// The directory `build` fills for `label` from `inputs`: built on a miss,
/// returned as it stands on a hit. `build` receives an empty directory and
/// writes the fixture into it.
pub fn built(label: &str, inputs: Inputs, build: impl FnOnce(&Path)) -> PathBuf {
    built_at(&cache_root(), label, &key(inputs), build)
}

/// The cache key of `inputs`: their hash folded with the `delvec` binary's and
/// the recipe's.
pub fn key(inputs: Inputs) -> String {
    hex(&inputs
        .text(delvec_sha())
        .text(recipe_sha())
        .hasher
        .finalize())
}

/// [`built`] with the key already computed, under `cache` — the seam the
/// helper's own test drives across processes.
pub fn built_at(cache: &Path, label: &str, key: &str, build: impl FnOnce(&Path)) -> PathBuf {
    let name = format!("{label}-{}", &key[..16]);
    let dest = cache.join(&name);
    std::fs::create_dir_all(cache).unwrap();
    let lock_path = cache.join(format!("{name}.lock"));
    let lock = std::fs::File::create(&lock_path)
        .unwrap_or_else(|e| panic!("{}: {e}", lock_path.display()));
    lock.lock()
        .unwrap_or_else(|e| panic!("{}: {e}", lock_path.display()));
    let done = cache.join(format!("{name}.done"));
    if done.is_file() && dest.is_dir() {
        drop(lock);
        return dest;
    }
    if dest.exists() {
        std::fs::remove_dir_all(&dest).unwrap_or_else(|e| panic!("{}: {e}", dest.display()));
    }
    let staging = cache.join(format!("{name}.partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).unwrap();
    build(&staging);
    std::fs::rename(&staging, &dest)
        .unwrap_or_else(|e| panic!("{} -> {}: {e}", staging.display(), dest.display()));
    use std::io::Write;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cache.join(format!("{name}.builds")))
        .unwrap();
    writeln!(log, "built by process {}", std::process::id()).unwrap();
    std::fs::write(&done, key).unwrap();
    drop(lock);
    dest
}

/// How many times the fixture `label` with `key` has been built under `cache`.
pub fn builds(cache: &Path, label: &str, key: &str) -> usize {
    let name = format!("{label}-{}", &key[..16]);
    std::fs::read_to_string(cache.join(format!("{name}.builds")))
        .map(|s| s.lines().count())
        .unwrap_or(0)
}
