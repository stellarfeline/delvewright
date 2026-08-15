//! Chunky's per-scene derived caches, and why emission deletes them.
//!
//! Beside every `<scene>.json` Chunky writes derived files keyed to the chunk
//! set it loaded: the voxel octree (`<scene>.octree2`), the emitter grid
//! (`<scene>.emittergrid`) and the accumulated sample buffer (`<scene>.dump`,
//! plus its `.dump.backup`). On the next `-render`, Chunky loads those instead
//! of re-reading the world — **silently**. Change a scene's `chunkList`, camera,
//! sun or water settings, re-render, and the frame comes back rendered from the
//! stale cache with none of the edits applied and no warning anywhere (a whole
//! debugging session was paid for this on 2026-08-06).
//!
//! Debug doctrine says automate the pitfall out of existence rather than
//! document it: emitting a scene file **invalidates** those caches, so emission
//! deletes them. Only the caches of the scenes being written are touched — an
//! unrelated scene's in-progress render in the same directory survives.

use std::path::{Path, PathBuf};

use crate::view::diag::{DW_OUTPUT, Diagnostic};

/// The extensions Chunky derives from a scene's chunk set, all invalidated the
/// moment the scene description changes.
pub const CACHE_SUFFIXES: [&str; 4] = ["octree2", "dump", "dump.backup", "emittergrid"];

/// Delete the derived caches of the scene named by `scene_json` (a bare file
/// name such as `spawn.json`) in `dir`. Returns the files actually removed, in
/// [`CACHE_SUFFIXES`] order. A cache that is not there is not an error; one that
/// cannot be removed is (leaving it would re-introduce the stale-render trap).
pub fn purge_scene_caches(dir: &Path, scene_json: &str) -> Result<Vec<PathBuf>, Diagnostic> {
    let stem = scene_json.strip_suffix(".json").unwrap_or(scene_json);
    let mut removed = Vec::new();
    for suffix in CACHE_SUFFIXES {
        let path = dir.join(format!("{stem}.{suffix}"));
        match std::fs::remove_file(&path) {
            Ok(()) => removed.push(path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Diagnostic::error(
                    DW_OUTPUT,
                    format!(
                        "remove stale Chunky cache {}: {e} — re-rendering this scene would \
                         silently reuse the cached chunks and ignore the new scene settings",
                        path.display()
                    ),
                ));
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("dw-render-cache-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn purges_every_derived_cache_of_that_scene_only() {
        let dir = tmp("purge");
        for f in [
            "spawn.octree2",
            "spawn.dump",
            "spawn.dump.backup",
            "spawn.emittergrid",
        ] {
            std::fs::write(dir.join(f), b"stale").unwrap();
        }
        // Same prefix, different scene: must survive (`spawn2` is not `spawn`).
        std::fs::write(dir.join("spawn2.octree2"), b"keep").unwrap();
        std::fs::write(dir.join("other.dump"), b"keep").unwrap();
        // The scene description itself is being (re)written, never deleted here.
        std::fs::write(dir.join("spawn.json"), b"{}").unwrap();

        let removed = purge_scene_caches(&dir, "spawn.json").unwrap();
        assert_eq!(removed.len(), 4, "{removed:?}");
        for f in [
            "spawn.octree2",
            "spawn.dump",
            "spawn.dump.backup",
            "spawn.emittergrid",
        ] {
            assert!(!dir.join(f).exists(), "{f} survived");
        }
        assert!(dir.join("spawn2.octree2").exists());
        assert!(dir.join("other.dump").exists());
        assert!(dir.join("spawn.json").exists());
    }

    #[test]
    fn absent_caches_are_not_an_error() {
        let dir = tmp("absent");
        assert!(purge_scene_caches(&dir, "fresh.json").unwrap().is_empty());
    }

    #[test]
    fn a_cache_that_cannot_be_removed_is_dw0722() {
        // A directory where the cache file should be: `remove_file` refuses,
        // and emission must refuse too rather than leave a stale render behind.
        let dir = tmp("blocked");
        std::fs::create_dir_all(dir.join("blocked.octree2")).unwrap();
        let err = purge_scene_caches(&dir, "blocked.json").unwrap_err();
        assert_eq!(err.code, DW_OUTPUT, "expected DW0722: {err:?}");
    }
}
