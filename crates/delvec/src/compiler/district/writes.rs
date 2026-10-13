//! **A district's write set, and the refusal of a write outside it**
//! (spec-0103 §7.6, `DW1008`).
//!
//! The documents are refused by the engine where they are read (`DW1004`,
//! `DW1005`); the **tree** is refused where it is committed. A district's write
//! set is derived from the declaration and the plan, never typed:
//!
//! * `<campaign>/districts/<x>/**` — its fragments;
//! * `<campaign>/programs/<stem>.json` and `<campaign>/design/places/<stem>/**`
//!   for each place `node/<stem>` of the district;
//! * the piece files `delvec detail` writes for those places under the prefab
//!   directory: `<piece>.nbt`, its tiles `<piece>.x<i>y<j>z<k>.nbt`,
//!   `<piece>.json` and `<piece>.report.json`, where `<piece>` is
//!   [`crate::compiler::detail::piece_id`] — and the piece a row of the
//!   district's names.
//!
//! One rule, one implementation: `delvec district writes` reads a list of
//! repository-relative paths and refuses every path outside the set, naming its
//! owner — the whole, or the district it belongs to. Its two call sites are the
//! worker's pre-commit hook and the planner's integration
//! (`tools/creator/district-hook.sh`, `tools/creator/district-integrate.sh`).

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use delvewright_dsl::{Campaign, NodeId};

use super::{DISTRICTS_DIR, Partition, dir_of, stem_of};

/// The repository a path lives in: the nearest ancestor holding `.git` (a
/// directory, or the file a worktree carries).
#[must_use]
pub fn repo_root(path: &Path) -> Option<PathBuf> {
    let start = std::fs::canonicalize(path).ok()?;
    let mut at: &Path = &start;
    loop {
        if at.join(".git").exists() {
            return Some(at.to_path_buf());
        }
        at = at.parent()?;
    }
}

/// `path` relative to `root`, with `/` separators — `None` outside it.
#[must_use]
pub fn relative(root: &Path, path: &Path) -> Option<String> {
    let abs = std::fs::canonicalize(path).ok()?;
    let rel = abs.strip_prefix(root).ok()?;
    Some(
        rel.components()
            .filter_map(|c| match c {
                Component::Normal(s) => s.to_str().map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// One district's write set, as path rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSet {
    /// `district/<x>`.
    pub district: String,
    /// Exact repository-relative files.
    pub files: BTreeSet<String>,
    /// Repository-relative directory prefixes (each ends in `/`): everything
    /// under them.
    pub trees: BTreeSet<String>,
    /// Repository-relative prefab directory and the piece ids whose files are
    /// in the set, or `None` when the prefab directory is outside the
    /// repository.
    pub pieces: Option<(String, BTreeSet<String>)>,
}

impl WriteSet {
    /// True when the set holds `path` (repository-relative, `/`-separated).
    #[must_use]
    pub fn holds(&self, path: &str) -> bool {
        if self.files.contains(path) || self.trees.iter().any(|t| path.starts_with(t.as_str())) {
            return true;
        }
        let Some((dir, ids)) = &self.pieces else {
            return false;
        };
        let name = if dir.is_empty() {
            path
        } else {
            match path.strip_prefix(&format!("{dir}/")) {
                Some(n) => n,
                None => return false,
            }
        };
        if name.contains('/') {
            return false;
        }
        piece_of_file(name).is_some_and(|id| ids.contains(id))
    }

    /// Every rule, as the lines a packet and a person read.
    #[must_use]
    pub fn rules(&self) -> Vec<String> {
        let mut out: Vec<String> = self.trees.iter().map(|t| format!("{t}**")).collect();
        out.extend(self.files.iter().cloned());
        if let Some((dir, ids)) = &self.pieces {
            let at = if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            };
            for id in ids {
                out.push(format!("{at}{id}.nbt"));
                out.push(format!("{at}{id}.x<i>y<j>z<k>.nbt"));
                out.push(format!("{at}{id}.json"));
                out.push(format!("{at}{id}.report.json"));
            }
        }
        out
    }
}

/// The piece a prefab-directory file belongs to, by the names `delvec detail`
/// writes: `<id>.json`, `<id>.report.json`, `<id>.nbt`, `<id>.x<i>y<j>z<k>.nbt`.
#[must_use]
pub fn piece_of_file(name: &str) -> Option<&str> {
    if let Some(id) = name.strip_suffix(crate::compiler::registry::REPORT_SUFFIX) {
        return Some(id);
    }
    if let Some((base, _)) = delvewright_dsl::split::tile_filename(name) {
        return Some(base);
    }
    name.strip_suffix(".nbt")
        .or_else(|| name.strip_suffix(".json"))
}

/// **Derive a district's write set** from the declaration and the plan.
///
/// `campaign_rel` is the campaign directory relative to the repository root;
/// `prefabs_rel` the prefab directory's, or `None` when it is outside.
#[must_use]
pub fn write_set(
    c: &Campaign,
    district: &str,
    campaign_rel: &str,
    prefabs_rel: Option<&str>,
) -> WriteSet {
    let partition = Partition::of(c);
    let at = |p: &str| {
        if campaign_rel.is_empty() {
            p.to_string()
        } else {
            format!("{campaign_rel}/{p}")
        }
    };
    let mut ws = WriteSet {
        district: district.to_string(),
        files: BTreeSet::new(),
        trees: BTreeSet::new(),
        pieces: None,
    };
    ws.trees
        .insert(at(&format!("{DISTRICTS_DIR}/{}/", dir_of(district))));
    let places: Vec<NodeId> = partition.places_of(c, district);
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for n in &places {
        let stem = stem_of(&n.0);
        ws.files.insert(at(&format!(
            "{}/{stem}.json",
            crate::compiler::detail::PROGRAMS_DIR
        )));
        ws.trees.insert(at(&format!(
            "design/{}/{stem}/",
            delvewright_dsl::design::PLACES_DIR
        )));
        ids.insert(crate::compiler::detail::piece_id(c, n));
    }
    for row in c.detail_plan.iter().flat_map(|p| p.content.details.iter()) {
        if partition.of_node(row.place.0.as_str()) == Some(district) {
            ids.insert(
                row.piece
                    .as_str()
                    .strip_prefix("prefab/")
                    .unwrap_or(row.piece.as_str())
                    .to_string(),
            );
        }
    }
    ws.pieces = prefabs_rel.map(|d| (d.to_string(), ids));
    ws
}

/// The owner of a path no district's set holds: the district whose set does,
/// or the whole.
#[must_use]
pub fn owner<'a>(sets: &'a [WriteSet], path: &str) -> &'a str {
    sets.iter()
        .find(|s| s.holds(path))
        .map_or("the whole", |s| s.district.as_str())
}

/// Normalise one input line into a repository-relative path: trimmed, `\`
/// read as `/`, a leading `./` dropped. An empty line is skipped.
#[must_use]
pub fn normalise(line: &str) -> Option<String> {
    let t = line.trim().replace('\\', "/");
    let t = t.strip_prefix("./").unwrap_or(&t).to_string();
    (!t.is_empty()).then_some(t)
}
