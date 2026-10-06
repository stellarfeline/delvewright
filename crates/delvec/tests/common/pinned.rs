//! **The content repository at `versions.toml` `[content].sha`**, read out of
//! its object store and never out of a working tree.
//!
//! Every test that judges "the pinned content" — the prefab library, the only
//! part of it the engine judges — resolves it here and nowhere else. The `campaigns/` path
//! beside this repository (a dev symlink locally, a checkout in CI) is used only
//! to FIND the content repository's git object store; which revision of it a
//! working tree happens to sit on never reaches a test. So a test's answer is a
//! property of this repository's revision alone: the same engine commit judges
//! the same content bytes on every machine.
//!
//! The tree is materialised once per pin into `CARGO_TARGET_TMPDIR`, blob by
//! blob through `git cat-file`, with each git-lfs pointer replaced by the object
//! it names out of the repository's own LFS store (`<git-common-dir>/lfs/objects`),
//! checked against the pointer's sha256 and size. A manifest of every file's
//! sha256 is written beside the tree and the whole tree is checked against it on
//! every first use in a process, so a test that wrote into the shared copy reds
//! instead of changing what the next test reads.
//!
//! **The manifest is the completion marker, never the directory.** A tree is
//! staged beside the destination and renamed into place only once its manifest
//! is written, under a lock file every test process takes, so no process ever
//! sees a tree this module wrote without its manifest. A destination WITHOUT a
//! manifest is therefore not one this module completed, and it is replaced
//! rather than judged. That state is real: `Swatinem/rust-cache` caches
//! `target/`, and before saving it deletes every file under a directory of
//! `target/` that is not a cargo profile — `target/tmp/pinned-content/<sha>`
//! included — keeping the directories. The next run restored an empty tree with
//! no manifest, and every test that read it failed on the missing manifest.
//!
//! An absent content repository, a repository that does not carry the pinned
//! commit, and an LFS object that is not in the local store are failures with
//! the command that repairs them, never a skip and never a fall-back to the
//! working tree.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

const LFS_POINTER_HEAD: &[u8] = b"version https://git-lfs.github.com/spec/v1\n";
const MANIFEST: &str = ".pinned-content-manifest";

/// `versions.toml` `[content].sha` — the one commit every pinned-content test
/// reads. A line scan of the one key, with no TOML dependency.
pub fn sha() -> String {
    let path = super::repo_root().join("versions.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut in_content = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_content = line == "[content]";
            continue;
        }
        if in_content
            && let Some(rest) = line.strip_prefix("sha")
            && let Some(rest) = rest.trim_start().strip_prefix('=')
        {
            let val = rest
                .split('#')
                .next()
                .unwrap_or(rest)
                .trim()
                .trim_matches('"');
            assert!(
                val.len() == 40 && val.bytes().all(|b| b.is_ascii_hexdigit()),
                "{}: [content].sha = {val:?} is not a full commit id",
                path.display()
            );
            return val.to_ascii_lowercase();
        }
    }
    panic!("{}: no [content].sha", path.display())
}

/// The root of the content repository's tree at the pin.
pub fn root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        materialise_into(&PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pinned-content"))
    })
    .clone()
}

/// The prefab library at the pin (`prefabs/`).
pub fn prefabs() -> PathBuf {
    root().join("prefabs")
}

fn git(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} in {}: {e}", repo.display()))
}

fn git_text(repo: &Path, args: &[&str]) -> String {
    let out = git(repo, args);
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        repo.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// The content repository, found through `campaigns/` and refused unless that
/// path is the top level of a repository of its own — otherwise `git -C` would
/// walk up into THIS repository and read the wrong object store.
fn content_repo() -> PathBuf {
    let link = super::repo_root().join("campaigns");
    let canonical = link.canonicalize().unwrap_or_else(|e| {
        panic!(
            "{}: {e} — the content repository is missing. Clone \
             stellarfeline/delvewright-campaigns and point `campaigns` at it \
             (docs/reference/worktree-bootstrap.md); the pinned content is read out of its \
             object store, so an absent repository is a failure and never a skip.",
            link.display()
        )
    });
    let top = git(&canonical, &["rev-parse", "--show-toplevel"]);
    let top = PathBuf::from(String::from_utf8_lossy(&top.stdout).trim());
    assert!(
        top.canonicalize().ok().as_deref() == Some(canonical.as_path()),
        "{} is not the top level of a git repository of its own (git resolves it to {:?}), so \
         the pinned content cannot be read out of its object store",
        link.display(),
        top
    );
    canonical
}

/// Materialise the pinned tree under `cache` (once per pin) and return it,
/// verified. Every step that decides what is on disk runs under
/// `<cache>/<sha>.lock`, an exclusive file lock across processes.
pub fn materialise_into(cache: &Path) -> PathBuf {
    let sha = sha();
    let dest = cache.join(&sha);
    std::fs::create_dir_all(cache).unwrap();
    let lock_path = cache.join(format!("{sha}.lock"));
    let lock = std::fs::File::create(&lock_path)
        .unwrap_or_else(|e| panic!("{}: {e}", lock_path.display()));
    lock.lock()
        .unwrap_or_else(|e| panic!("{}: {e}", lock_path.display()));
    if dest.join(MANIFEST).is_file() {
        drop(lock);
        verify(&dest, &sha);
        return dest;
    }
    if dest.exists() {
        // Not a tree this module completed (see the module note): replace it.
        std::fs::remove_dir_all(&dest).unwrap_or_else(|e| panic!("{}: {e}", dest.display()));
    }
    let repo = content_repo();
    let has = git(&repo, &["cat-file", "-e", &format!("{sha}^{{commit}}")]);
    assert!(
        has.status.success(),
        "the content repository at {} does not carry the pinned commit {sha} \
         (versions.toml [content].sha). Fetch it: `git -C {} fetch origin`. The working tree \
         is never read in its place.",
        repo.display(),
        repo.display()
    );
    let lfs_store = PathBuf::from(git_text(
        &repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    ))
    .join("lfs/objects");

    let staging = cache.join(format!(
        "{sha}.partial-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).unwrap();

    // (mode, object id, path) for every entry of the pinned tree.
    let listing = git(&repo, &["ls-tree", "-r", "-z", "--full-tree", &sha]);
    assert!(listing.status.success(), "git ls-tree {sha}");
    let mut entries = Vec::new();
    for rec in listing.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let rec = std::str::from_utf8(rec).expect("tree path is UTF-8");
        let (meta, path) = rec.split_once('\t').expect("ls-tree record");
        let mut f = meta.split(' ');
        let (mode, kind, oid) = (f.next().unwrap(), f.next().unwrap(), f.next().unwrap());
        assert!(
            kind == "blob" && (mode == "100644" || mode == "100755"),
            "{path} at content {sha} is a {kind} of mode {mode}; the pinned-content reader \
             materialises regular files only"
        );
        entries.push((mode.to_string(), oid.to_string(), path.to_string()));
    }
    assert!(!entries.is_empty(), "the content tree at {sha} is empty");

    let mut child = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git cat-file --batch");
    let mut stdin = child.stdin.take().unwrap();
    let oids: Vec<String> = entries.iter().map(|(_, oid, _)| oid.clone()).collect();
    let writer = std::thread::spawn(move || {
        for oid in oids {
            writeln!(stdin, "{oid}").unwrap();
        }
    });
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut manifest = BTreeMap::new();
    for (mode, oid, path) in &entries {
        let mut header = String::new();
        out.read_line(&mut header).unwrap();
        let mut h = header.split_whitespace();
        assert_eq!(h.next(), Some(oid.as_str()), "cat-file header {header:?}");
        assert_eq!(h.next(), Some("blob"), "cat-file header {header:?}");
        let size: usize = h.next().unwrap().parse().unwrap();
        let mut bytes = vec![0u8; size];
        out.read_exact(&mut bytes).unwrap();
        let mut nl = [0u8; 1];
        out.read_exact(&mut nl).unwrap();
        if bytes.starts_with(LFS_POINTER_HEAD) {
            bytes = lfs_object(&bytes, path, &lfs_store, &repo, &sha);
        }
        let file = staging.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, &bytes).unwrap();
        #[cfg(unix)]
        if mode == "100755" {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(not(unix))]
        let _ = mode;
        manifest.insert(path.clone(), hex(&Sha256::digest(&bytes)));
    }
    writer.join().unwrap();
    assert!(child.wait().unwrap().success(), "git cat-file --batch");

    let mut text = format!("commit {sha}\n");
    for (path, digest) in &manifest {
        text.push_str(&format!("{digest} {path}\n"));
    }
    std::fs::write(staging.join(MANIFEST), text).unwrap();

    // Under the lock, nobody else can have put a tree here since the check
    // above, so a failed rename is a failure and never "someone was first".
    std::fs::rename(&staging, &dest)
        .unwrap_or_else(|e| panic!("{} -> {}: {e}", staging.display(), dest.display()));
    drop(lock);
    verify(&dest, &sha);
    dest
}

/// The bytes a git-lfs pointer names, out of the repository's own LFS store,
/// checked against the pointer's own sha256 and size.
fn lfs_object(pointer: &[u8], path: &str, store: &Path, repo: &Path, sha: &str) -> Vec<u8> {
    let text = std::str::from_utf8(pointer).expect("an LFS pointer is text");
    let mut oid = None;
    let mut size = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("oid sha256:") {
            oid = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("size ") {
            size = Some(v.trim().parse::<usize>().expect("LFS pointer size"));
        }
    }
    let (oid, size) = (
        oid.unwrap_or_else(|| panic!("{path}: LFS pointer without an oid")),
        size.unwrap_or_else(|| panic!("{path}: LFS pointer without a size")),
    );
    let object = store.join(&oid[0..2]).join(&oid[2..4]).join(&oid);
    let bytes = std::fs::read(&object).unwrap_or_else(|e| {
        panic!(
            "{path} at content {sha} is a git-lfs object ({oid}) the local LFS store does not \
             hold ({}: {e}). Fetch it: `git -C {} lfs fetch origin {sha}`.",
            object.display(),
            repo.display()
        )
    });
    assert!(
        bytes.len() == size && hex(&Sha256::digest(&bytes)) == oid,
        "{}: the LFS object for {path} does not match its pointer (size {}, want {size})",
        object.display(),
        bytes.len()
    );
    bytes
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The materialised tree, file for file and byte for byte, against the manifest
/// written when it was materialised.
fn verify(dest: &Path, sha: &str) {
    let manifest_path = dest.join(MANIFEST);
    let text = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("{}: {e}", manifest_path.display()));
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some(format!("commit {sha}").as_str()),
        "{}: manifest names another commit",
        manifest_path.display()
    );
    let want: BTreeMap<String, String> = lines
        .map(|l| {
            let (d, p) = l.split_once(' ').expect("manifest line");
            (p.to_string(), d.to_string())
        })
        .collect();
    let mut have = BTreeMap::new();
    walk(dest, dest, &mut have);
    have.remove(MANIFEST);
    assert!(
        have == want,
        "the materialised pinned content at {} no longer matches the commit it was read from \
         ({} file(s) on disk, {} in the manifest) — a test wrote into the shared copy. Delete \
         the directory to re-materialise it, and find the test.",
        dest.display(),
        have.len(),
        want.len()
    );
}

fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(base, &path, out);
        } else {
            let rel = path.strip_prefix(base).unwrap();
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let bytes = std::fs::read(&path).unwrap();
            out.insert(rel, hex(&Sha256::digest(&bytes)));
        }
    }
}
