//! The pinned-content cache (`common::pinned`) survives the two states that
//! broke it, each in a cache directory of its own so the shared copy every
//! other test reads is never touched:
//!
//! * the tree `Swatinem/rust-cache` leaves behind — every file under the
//!   materialised directory deleted, the directories kept, the manifest gone —
//!   is replaced, not judged;
//! * several materialisations of one pin at once, from threads holding their
//!   own lock handles, all return the one verified tree.

mod common;

use std::path::{Path, PathBuf};

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
fn a_tree_the_cache_cleaner_emptied_is_materialised_again() {
    let cache = fresh("pinned-content-cleaned");
    let dest = common::pinned::materialise_into(&cache);
    let before = files(&dest).len();
    assert!(before > 1, "{before} file(s) materialised");

    // What `rust-cache` does to a non-profile directory of `target/` before it
    // saves: every file removed, every directory kept.
    for f in files(&dest) {
        std::fs::remove_file(f).unwrap();
    }
    assert!(files(&dest).is_empty() && dest.is_dir());

    let again = common::pinned::materialise_into(&cache);
    assert_eq!(again, dest);
    assert_eq!(files(&again).len(), before);
}

#[test]
fn concurrent_materialisations_of_one_pin_agree() {
    let cache = fresh("pinned-content-concurrent");
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let cache = cache.clone();
            std::thread::spawn(move || common::pinned::materialise_into(&cache))
        })
        .collect();
    let roots: Vec<PathBuf> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(roots.windows(2).all(|w| w[0] == w[1]), "{roots:?}");
    let leftovers: Vec<_> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".partial-"))
        .collect();
    assert!(leftovers.is_empty(), "staging left behind: {leftovers:?}");
}
