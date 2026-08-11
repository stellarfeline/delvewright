//! Lazy read access to a Minecraft asset source — the pinned 1.21.11 client jar,
//! a resource-pack `.zip`, or an unpacked directory.
//!
//! The GPU path ([`crate::render`]) hands the same path to Nucleation, whose
//! loader eagerly parses every `assets/` entry and exposes nothing under `data/`.
//! The colour derivation ([`crate::blockcolor`]) needs both halves — block models
//! and textures live in `assets/`, while the biome that decides grass, foliage and
//! water tint lives in `data/minecraft/worldgen/biome/` — and it needs perhaps a
//! hundred entries out of the jar's twenty-eight thousand. So it reads the archive
//! itself, by name, on demand.
//!
//! Reads are cached, so resolving a palette of 150 blockstates that share a dozen
//! parent models touches each file once.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// Where block models, textures and biome definitions are read from.
pub enum AssetSource {
    /// A `.jar` / `.zip` archive.
    Archive {
        path: PathBuf,
        zip: RefCell<zip::ZipArchive<std::fs::File>>,
    },
    /// An unpacked resource-pack directory (the root that contains `assets/`).
    Directory { path: PathBuf },
}

/// Failure to open or read an asset source.
#[derive(Debug)]
pub struct AssetError(pub String);

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for AssetError {}

impl AssetSource {
    /// Open an archive or a directory. Which one is decided by what is on disk,
    /// not by the extension: `--textures` already accepts either.
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        let meta = std::fs::metadata(path)
            .map_err(|e| AssetError(format!("open {}: {e}", path.display())))?;
        if meta.is_dir() {
            return Ok(AssetSource::Directory {
                path: path.to_path_buf(),
            });
        }
        let f = std::fs::File::open(path)
            .map_err(|e| AssetError(format!("open {}: {e}", path.display())))?;
        let zip = zip::ZipArchive::new(f)
            .map_err(|e| AssetError(format!("read archive {}: {e}", path.display())))?;
        Ok(AssetSource::Archive {
            path: path.to_path_buf(),
            zip: RefCell::new(zip),
        })
    }

    /// The path this source was opened from, for diagnostics.
    pub fn path(&self) -> &Path {
        match self {
            AssetSource::Archive { path, .. } | AssetSource::Directory { path } => path,
        }
    }

    /// Read one entry by its archive-relative path (e.g.
    /// `assets/minecraft/blockstates/stone.json`). `None` when absent — an absent
    /// entry is ordinary (it is how "this block does not exist in this version"
    /// presents), never an error at this layer.
    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            AssetSource::Archive { zip, .. } => {
                let mut zip = zip.borrow_mut();
                let mut entry = zip.by_name(name).ok()?;
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf).ok()?;
                Some(buf)
            }
            AssetSource::Directory { path } => std::fs::read(path.join(name)).ok(),
        }
    }
}

/// An [`AssetSource`] with a read-through cache, so a palette whose entries share
/// parent models and textures reads each file once.
pub struct Assets {
    source: AssetSource,
    cache: RefCell<BTreeMap<String, Option<Vec<u8>>>>,
}

impl Assets {
    pub fn open(path: &Path) -> Result<Self, AssetError> {
        Ok(Assets {
            source: AssetSource::open(path)?,
            cache: RefCell::new(BTreeMap::new()),
        })
    }

    pub fn path(&self) -> &Path {
        self.source.path()
    }

    /// Cached [`AssetSource::read`].
    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        if let Some(hit) = self.cache.borrow().get(name) {
            return hit.clone();
        }
        let v = self.source.read(name);
        self.cache.borrow_mut().insert(name.to_string(), v.clone());
        v
    }

    /// Read and parse a JSON entry.
    pub fn read_json(&self, name: &str) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.read(name)?).ok()
    }

    /// `assets/<ns>/blockstates/<id>.json` for a `ns:id` block id.
    pub fn blockstate(&self, ns: &str, id: &str) -> Option<serde_json::Value> {
        self.read_json(&format!("assets/{ns}/blockstates/{id}.json"))
    }

    /// `assets/<ns>/models/<path>.json` for a `ns:path` model reference.
    pub fn model(&self, ns: &str, path: &str) -> Option<serde_json::Value> {
        self.read_json(&format!("assets/{ns}/models/{path}.json"))
    }

    /// `data/<ns>/worldgen/biome/<id>.json` for a `ns:id` biome id.
    pub fn biome(&self, ns: &str, id: &str) -> Option<serde_json::Value> {
        self.read_json(&format!("data/{ns}/worldgen/biome/{id}.json"))
    }

    /// Decode `assets/<ns>/textures/<path>.png` to RGBA8 `(width, height, pixels)`.
    ///
    /// An animated texture is a vertical strip of square frames; only the first
    /// frame is returned, so a mean over the result is the block's resting colour
    /// rather than a blend of every animation frame.
    pub fn texture_rgba(&self, ns: &str, path: &str) -> Option<(u32, u32, Vec<u8>)> {
        let bytes = self.read(&format!("assets/{ns}/textures/{path}.png"))?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return None;
        }
        let mut px = img.into_raw();
        if h > w && h % w == 0 {
            px.truncate((w as usize) * (w as usize) * 4);
            return Some((w, w, px));
        }
        Some((w, h, px))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dw-assets-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn reads_a_directory_source_and_caches_misses() {
        let root = tmp("dir");
        std::fs::create_dir_all(root.join("assets/minecraft/blockstates")).unwrap();
        std::fs::write(
            root.join("assets/minecraft/blockstates/stone.json"),
            br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        )
        .unwrap();
        let a = Assets::open(&root).unwrap();
        assert!(a.blockstate("minecraft", "stone").is_some());
        // A miss is `None`, not an error, and is cached as a miss.
        assert!(a.blockstate("minecraft", "no_such_block").is_none());
        assert!(a.blockstate("minecraft", "no_such_block").is_none());
    }

    #[test]
    fn opening_a_missing_path_is_an_error() {
        let root = tmp("missing");
        assert!(Assets::open(&root.join("nope.jar")).is_err());
    }
}
