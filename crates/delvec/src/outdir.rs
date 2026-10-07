//! **An output directory holds exactly the tree that was last written into it.**
//!
//! `delvec build -o <dir>` and `delvec prefab gallery --out <dir>` write a tree
//! of files whose root carries a `manifest.json` naming every other file under
//! `outputs`. That manifest is the ownership record: a directory whose
//! `manifest.json` parses as one is a directory `delvec` wrote, and the files it
//! names are files `delvec` may remove. Writing a new tree into such a
//! directory removes every file the previous tree named and this one does not,
//! so nothing a previous build emitted (an advancement, the PackTest of an
//! objective since removed) survives beside the new emission.
//!
//! Anything else in the directory is **refused** (`DW0967`), never deleted:
//! a non-empty directory with no manifest, or a file the manifest does not
//! name, is a directory the caller may have pointed at by accident, and
//! deleting it would be the one mistake that cannot be undone.
//!
//! Three names at the root are **derived artifacts** that engine tools write
//! into a build directory from the build in it ([`DERIVED`]). Each is a
//! product of the previous build, so a new build removes it rather than refusing.
//!
//! The order of writes keeps the directory owned at every instant: stale files
//! are removed first, under the old manifest; the new `manifest.json` is
//! written next, naming every file that follows it; then the rest. A run that
//! stops part way leaves a directory the next run still owns.

use delvewright_dsl::diagnostic::{DwCode, ExitTier};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

delvewright_dsl::dw_code! {
    /// An output directory holds files no `delvec` tree wrote, or is not a
    /// directory: refused before anything in it is touched.
    pub const DW_OUTPUT_NOT_OWNED: DwCode = DwCode::new("DW0967", ExitTier::Build);
}

/// The ownership record at the root of every tree this module writes.
pub const MANIFEST: &str = "manifest.json";

/// Whether a [`Derived`] artifact is a directory or a single file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedKind {
    Dir,
    File,
}

/// A root entry an engine tool writes into a build directory from the build
/// it holds. A rebuild removes it: it describes the previous build.
#[derive(Debug, Clone, Copy)]
pub struct Derived {
    pub name: &'static str,
    pub kind: DerivedKind,
    /// The tool that writes it, for the line a rebuild prints.
    pub writer: &'static str,
}

/// Every derived artifact, by its root name. An entry of the same name and a
/// different kind is foreign.
pub const DERIVED: &[Derived] = &[
    Derived {
        name: "world",
        kind: DerivedKind::Dir,
        writer: "validation/world-save.sh",
    },
    Derived {
        name: "shots",
        kind: DerivedKind::Dir,
        writer: "validation/render-shots.sh",
    },
    Derived {
        name: "staging-admission.json",
        kind: DerivedKind::File,
        writer: "tools/creator/staging-gate.py",
    },
];

/// The `outputs` object of a [`MANIFEST`]: every file of `tree`, by path, to
/// the SHA-256 of its bytes. For a verb whose tree has no manifest of its own.
pub fn outputs_index(tree: &BTreeMap<String, Vec<u8>>) -> serde_json::Value {
    use sha2::{Digest, Sha256};
    let m: serde_json::Map<String, serde_json::Value> = tree
        .iter()
        .filter(|(k, _)| k.as_str() != MANIFEST)
        .map(|(k, v)| {
            let d = Sha256::digest(v);
            let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
            (k.clone(), serde_json::Value::String(hex))
        })
        .collect();
    serde_json::Value::Object(m)
}

/// What a write did to the directory.
#[derive(Debug, Default)]
pub struct Replaced {
    /// Files written.
    pub written: usize,
    /// Files the previous tree named that this tree does not, removed.
    pub stale_removed: Vec<String>,
    /// Derived artifacts removed, with the number of files each held.
    pub derived_removed: Vec<(&'static Derived, usize)>,
}

impl Replaced {
    /// The line a verb prints after a write that removed anything; `None`
    /// when the directory held nothing but what this tree overwrote.
    pub fn summary(&self, out: &Path) -> Option<String> {
        if self.stale_removed.is_empty() && self.derived_removed.is_empty() {
            return None;
        }
        let mut s = format!(
            "{}: wrote {} file(s); removed {} file(s) the previous tree emitted and this one does not",
            out.display(),
            self.written,
            self.stale_removed.len()
        );
        for (d, n) in &self.derived_removed {
            s.push_str(&format!(
                "; removed `{}` ({n} file(s), written by {} from the previous tree)",
                d.name, d.writer
            ));
        }
        Some(s)
    }
}

/// Why a write did not happen.
#[derive(Debug)]
pub enum OutError {
    /// `DW0967`: the directory is not one this module owns. Nothing was touched.
    NotOwned(String),
    /// The tree handed in does not name itself: an engine defect.
    Internal(String),
    /// The filesystem failed part way. The directory is still owned.
    Io(std::io::Error),
}

impl From<std::io::Error> for OutError {
    fn from(e: std::io::Error) -> Self {
        OutError::Io(e)
    }
}

/// An entry found under the output root, by relative `/`-joined path.
enum Found {
    File(String),
    /// A symlink, a non-UTF-8 name, a device: never owned.
    Foreign(String),
    Derived(&'static Derived, usize),
}

/// Write `tree` into `out` so that afterwards `out` holds exactly `tree`.
///
/// `tree` must carry a [`MANIFEST`] whose `outputs` names every other key, so
/// the directory it leaves is one the next write owns.
pub fn replace(out: &Path, tree: &BTreeMap<String, Vec<u8>>) -> Result<Replaced, OutError> {
    let new_owned = owned_set(tree.get(MANIFEST).map(Vec::as_slice)).map_err(|why| {
        OutError::Internal(format!(
            "the tree to write carries no usable {MANIFEST}: {why}"
        ))
    })?;
    let keys: BTreeSet<String> = tree.keys().cloned().collect();
    if new_owned != keys {
        let unnamed: Vec<_> = keys.difference(&new_owned).take(5).collect();
        let absent: Vec<_> = new_owned.difference(&keys).take(5).collect();
        return Err(OutError::Internal(format!(
            "the tree's {MANIFEST} does not name exactly its files (unnamed: {unnamed:?}; named and absent: {absent:?})"
        )));
    }
    for k in &keys {
        if !is_plain_relative(k) {
            return Err(OutError::Internal(format!(
                "the tree names `{k}`, which is not a plain relative path"
            )));
        }
    }

    let mut done = Replaced::default();
    match std::fs::metadata(out) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(out)?;
        }
        Err(e) => return Err(e.into()),
        Ok(m) if !m.is_dir() => {
            return Err(OutError::NotOwned(format!(
                "`{}` exists and is not a directory. Name a directory the build may own",
                out.display()
            )));
        }
        Ok(_) => {
            let found = walk(out)?;
            if !found.is_empty() {
                let old_owned = read_owned(out, &found)?;
                let foreign: Vec<&str> = found
                    .iter()
                    .filter_map(|f| match f {
                        Found::File(p) if !old_owned.contains(p) => Some(p.as_str()),
                        Found::Foreign(p) => Some(p.as_str()),
                        _ => None,
                    })
                    .collect();
                if !foreign.is_empty() {
                    return Err(OutError::NotOwned(format!(
                        "`{}` holds {} file(s) its {MANIFEST} does not name, so they were not \
                         written by the tree it records and are not the build's to remove: {}{}. \
                         Move them out, or delete the directory, and build again. Nothing was \
                         written or removed",
                        out.display(),
                        foreign.len(),
                        foreign
                            .iter()
                            .take(10)
                            .map(|p| format!("`{p}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        if foreign.len() > 10 { ", …" } else { "" }
                    )));
                }
                for f in &found {
                    match f {
                        Found::File(p) if !keys.contains(p) => {
                            std::fs::remove_file(out.join(p))?;
                            done.stale_removed.push(p.clone());
                        }
                        Found::Derived(d, n) => {
                            let at = out.join(d.name);
                            match d.kind {
                                DerivedKind::Dir => std::fs::remove_dir_all(&at)?,
                                DerivedKind::File => std::fs::remove_file(&at)?,
                            }
                            done.derived_removed.push((d, *n));
                        }
                        _ => {}
                    }
                }
                prune_empty_dirs(out, true)?;
            }
        }
    }

    // The manifest first: from here on every file on disk is one it names.
    write_one(out, MANIFEST, &tree[MANIFEST])?;
    done.written += 1;
    for (rel, bytes) in tree {
        if rel != MANIFEST {
            write_one(out, rel, bytes)?;
            done.written += 1;
        }
    }
    Ok(done)
}

fn write_one(out: &Path, rel: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = out.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

/// The set a manifest owns: every `outputs` key, plus the manifest itself.
fn owned_set(manifest: Option<&[u8]>) -> Result<BTreeSet<String>, String> {
    let bytes = manifest.ok_or_else(|| format!("there is no {MANIFEST}"))?;
    let v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("{MANIFEST} is not JSON: {e}"))?;
    let obj = v
        .as_object()
        .ok_or_else(|| format!("{MANIFEST} is not a JSON object"))?;
    if !obj.get("delvec_version").is_some_and(|x| x.is_string()) {
        return Err(format!("{MANIFEST} states no `delvec_version`"));
    }
    let outputs = obj
        .get("outputs")
        .and_then(|o| o.as_object())
        .ok_or_else(|| format!("{MANIFEST} has no `outputs` object"))?;
    let mut set = BTreeSet::new();
    for (k, h) in outputs {
        if !h.is_string() || !is_plain_relative(k) || k == MANIFEST {
            return Err(format!(
                "{MANIFEST} `outputs` names `{k}`, which no tree writes"
            ));
        }
        set.insert(k.clone());
    }
    set.insert(MANIFEST.to_string());
    Ok(set)
}

fn read_owned(out: &Path, found: &[Found]) -> Result<BTreeSet<String>, OutError> {
    let has_manifest = found
        .iter()
        .any(|f| matches!(f, Found::File(p) if p == MANIFEST));
    let bytes = if has_manifest {
        Some(std::fs::read(out.join(MANIFEST))?)
    } else {
        None
    };
    owned_set(bytes.as_deref()).map_err(|why| {
        OutError::NotOwned(format!(
            "`{}` is not empty and is not a tree delvec wrote ({why}); it holds {} entr(y/ies). \
             Name an empty or absent directory, or delete this one, and build again. Nothing \
             was written or removed",
            out.display(),
            found.len()
        ))
    })
}

/// A path the manifest may name: relative, `/`-joined, no `.`/`..`.
fn is_plain_relative(p: &str) -> bool {
    !p.is_empty()
        && !p.contains('\\')
        && Path::new(p)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Every entry under `root`, sorted, without following any symlink. Root
/// entries named in [`DERIVED`] with the right kind are reported whole.
fn walk(root: &Path) -> std::io::Result<Vec<Found>> {
    let mut found = Vec::new();
    walk_into(root, root, &mut found)?;
    Ok(found)
}

fn walk_into(root: &Path, dir: &Path, found: &mut Vec<Found>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let rel_os = path.strip_prefix(root).expect("walk stays under root");
        let rel = rel_os.to_str().map(|s| s.replace('\\', "/"));
        let ft = e.file_type()?;
        let Some(rel) = rel else {
            found.push(Found::Foreign(rel_os.to_string_lossy().into_owned()));
            continue;
        };
        if dir == root
            && let Some(d) = DERIVED.iter().find(|d| d.name == rel)
        {
            let kind_ok = match d.kind {
                DerivedKind::Dir => ft.is_dir(),
                DerivedKind::File => ft.is_file(),
            };
            if kind_ok {
                let n = if ft.is_dir() { count_files(&path)? } else { 1 };
                found.push(Found::Derived(d, n));
                continue;
            }
        }
        if ft.is_dir() {
            walk_into(root, &path, found)?;
        } else if ft.is_file() {
            found.push(Found::File(rel));
        } else {
            found.push(Found::Foreign(rel));
        }
    }
    Ok(())
}

fn count_files(dir: &Path) -> std::io::Result<usize> {
    let mut n = 0;
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let ft = e.file_type()?;
        if ft.is_dir() {
            n += count_files(&e.path())?;
        } else {
            n += 1;
        }
    }
    Ok(n)
}

/// Remove every directory under `dir` that holds nothing; `dir` itself only
/// when it is not the root. Returns whether `dir` is now empty.
fn prune_empty_dirs(dir: &Path, is_root: bool) -> std::io::Result<bool> {
    let mut empty = true;
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            if !prune_empty_dirs(&e.path(), false)? {
                empty = false;
            }
        } else {
            empty = false;
        }
    }
    if empty && !is_root {
        std::fs::remove_dir(dir)?;
    }
    Ok(empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        let mut t: BTreeMap<String, Vec<u8>> = files
            .iter()
            .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
            .collect();
        let outputs: serde_json::Map<String, serde_json::Value> = t
            .keys()
            .map(|k| (k.clone(), serde_json::Value::String("h".into())))
            .collect();
        let m = serde_json::json!({ "delvec_version": "x", "outputs": outputs });
        t.insert(MANIFEST.into(), serde_json::to_vec(&m).unwrap());
        t
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/outdir-unit")
            .join(name);
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn files(root: &Path) -> Vec<String> {
        walk(root)
            .unwrap()
            .into_iter()
            .map(|f| match f {
                Found::File(p) | Found::Foreign(p) => p,
                Found::Derived(d, _) => d.name.to_string(),
            })
            .collect()
    }

    #[test]
    fn a_rewrite_removes_what_the_previous_tree_named_and_this_one_does_not() {
        let d = scratch("rewrite");
        replace(&d, &tree(&[("a/x", "1"), ("a/y", "2"), ("b/z", "3")])).unwrap();
        let r = replace(&d, &tree(&[("a/x", "1")])).unwrap();
        assert_eq!(r.stale_removed, vec!["a/y".to_string(), "b/z".to_string()]);
        assert_eq!(files(&d), vec!["a/x".to_string(), MANIFEST.to_string()]);
        assert!(!d.join("b").exists(), "an emptied directory is pruned");
    }

    #[test]
    fn a_file_the_manifest_does_not_name_is_refused_and_nothing_moves() {
        let d = scratch("foreign");
        replace(&d, &tree(&[("a/x", "1")])).unwrap();
        std::fs::write(d.join("notes.txt"), "mine").unwrap();
        let e = replace(&d, &tree(&[("q", "2")])).unwrap_err();
        assert!(
            matches!(&e, OutError::NotOwned(m) if m.contains("`notes.txt`")),
            "{e:?}"
        );
        assert!(d.join("a/x").is_file(), "a refusal removes nothing");
        assert!(d.join("notes.txt").is_file());
        assert_eq!(DW_OUTPUT_NOT_OWNED.id(), "DW0967");
    }

    #[test]
    fn a_non_empty_directory_with_no_manifest_is_refused() {
        let d = scratch("unowned");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("thesis.tex"), "years").unwrap();
        let e = replace(&d, &tree(&[("a", "1")])).unwrap_err();
        assert!(
            matches!(&e, OutError::NotOwned(m) if m.contains("there is no manifest.json")),
            "{e:?}"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("thesis.tex")).unwrap(),
            "years"
        );
        assert!(!d.join("a").exists());
    }

    #[test]
    fn a_foreign_manifest_does_not_confer_ownership() {
        let d = scratch("foreign-manifest");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(MANIFEST), r#"{"name":"web app"}"#).unwrap();
        let e = replace(&d, &tree(&[("a", "1")])).unwrap_err();
        assert!(
            matches!(&e, OutError::NotOwned(m) if m.contains("delvec_version")),
            "{e:?}"
        );
        std::fs::write(
            d.join(MANIFEST),
            r#"{"delvec_version":"x","outputs":{"../escape":"h"}}"#,
        )
        .unwrap();
        let e = replace(&d, &tree(&[("a", "1")])).unwrap_err();
        assert!(
            matches!(&e, OutError::NotOwned(m) if m.contains("../escape")),
            "{e:?}"
        );
    }

    #[test]
    fn derived_artifacts_of_the_previous_tree_are_removed() {
        let d = scratch("derived");
        replace(&d, &tree(&[("a", "1")])).unwrap();
        std::fs::create_dir_all(d.join("world/region")).unwrap();
        std::fs::write(d.join("world/level.dat"), "old").unwrap();
        std::fs::write(d.join("world/region/r.0.0.mca"), "old").unwrap();
        std::fs::write(d.join("staging-admission.json"), "{}").unwrap();
        let r = replace(&d, &tree(&[("a", "1")])).unwrap();
        let names: Vec<_> = r
            .derived_removed
            .iter()
            .map(|(x, n)| (x.name, *n))
            .collect();
        assert_eq!(names, vec![("staging-admission.json", 1), ("world", 2)]);
        assert_eq!(files(&d), vec!["a".to_string(), MANIFEST.to_string()]);
    }

    #[test]
    fn a_derived_name_of_the_wrong_kind_is_foreign() {
        let d = scratch("derived-kind");
        replace(&d, &tree(&[("a", "1")])).unwrap();
        std::fs::write(d.join("world"), "a file, not a save").unwrap();
        assert!(matches!(
            replace(&d, &tree(&[("a", "1")])),
            Err(OutError::NotOwned(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_never_owned_even_when_named() {
        let d = scratch("symlink");
        replace(&d, &tree(&[("a", "1")])).unwrap();
        std::fs::remove_file(d.join("a")).unwrap();
        std::os::unix::fs::symlink("/", d.join("a")).unwrap();
        let e = replace(&d, &tree(&[("a", "1")])).unwrap_err();
        assert!(
            matches!(&e, OutError::NotOwned(m) if m.contains("`a`")),
            "{e:?}"
        );
    }

    #[test]
    fn a_tree_that_does_not_name_itself_is_an_engine_defect() {
        let d = scratch("internal");
        let mut t = tree(&[("a", "1")]);
        t.insert("b".into(), b"2".to_vec());
        assert!(matches!(replace(&d, &t), Err(OutError::Internal(_))));
        assert!(
            !d.exists(),
            "nothing is created for a tree that cannot be owned"
        );
    }

    #[test]
    fn a_file_is_not_a_directory() {
        let d = scratch("file");
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        std::fs::write(&d, "x").unwrap();
        assert!(matches!(
            replace(&d, &tree(&[("a", "1")])),
            Err(OutError::NotOwned(_))
        ));
        std::fs::remove_file(&d).unwrap();
    }
}
