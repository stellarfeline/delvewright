//! **The place kind a standalone audit judges a piece as** (spec-0098 §14).
//!
//! `delvec detail` judges a piece's spatial contract *sealed* when the place it
//! fills is scenery (`reached: false` in the layout graph): a place built to be
//! seen and never entered owes no way in, no reachable floor and no exterior
//! face. The audit has to reach the same verdict over the same bytes, or it
//! refuses pieces their writer passed.
//!
//! The kind relaxes obligations, so it is an opt-out, and the piece's own
//! document is never asked for it: a field a piece writes about itself is a
//! property a defective piece can supply. The kind is derived here from the
//! authority that decides it — each campaign's layout graph — reached through
//! the one name the piece already carries, its `prefab_id`, which each
//! campaign's `detail-plan` binds to a place. A piece is judged sealed only
//! when at least one campaign places it and **every** campaign that places it
//! places it at a scenery node, through [`is_scenery`],
//! the rule `delvec detail` seals on.
//!
//! Where the campaigns are: a prefab library is `<root>/prefabs/`, and the
//! campaigns built against it are `<root>/campaigns/<name>/` — the layout a
//! content repository has, and the one the default `--prefabs`
//! (`campaigns/prefabs`) resolves through. The piece is corroborated as the one
//! a campaign resolves for the id: its document's file name must be the id's
//! stem. A document claiming another piece's id under its own file name is
//! judged strictly.
//!
//! Every way of not finding the evidence lands on the strict judgement, which
//! can only refuse: no `campaigns/` beside the library, no campaign placing the
//! piece, a placing campaign with no layout graph or no node of that name. A
//! campaign whose `detail-plan.json` does not parse under this engine places
//! nothing in any world this engine builds, so it owns nothing; it is named in
//! the evidence line rather than passed over in silence.

use std::path::Path;

use delvewright_dsl::detailplan::DetailPlanContent;
use delvewright_dsl::layout::LayoutGraphContent;
use delvewright_dsl::{Envelope, NodeId};

use crate::compiler::load::{DETAIL_PLAN_FILE, LAYOUT_GRAPH_FILE};
use crate::schem::prefab::PrefabMeta;

/// **Whether `node` is scenery in `graph`** (spec-0098 §14): a place the graph
/// names with `reached: false`, built to be seen and never entered. A node the
/// graph does not name is not scenery — absent means reached.
///
/// The one rule every judge of a piece reads the place's kind through:
/// `delvec detail` seals the piece's contract on it before writing, and
/// [`place_kind`] derives the audit's kind through it, so the writer and the
/// audit cannot judge one piece two ways.
#[must_use]
pub fn is_scenery(graph: &LayoutGraphContent, node: &NodeId) -> bool {
    graph.nodes.iter().any(|n| &n.id == node && !n.reached)
}

/// The directory beside a prefab library that holds the campaigns built
/// against it.
pub const CAMPAIGNS_DIR: &str = "campaigns";

/// One `detail-plan` row placing the audited piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// The campaign directory's name.
    pub campaign: String,
    /// The layout-graph node the row binds the piece to.
    pub place: String,
    /// Whether that node is scenery in the campaign's layout graph.
    pub scenery: bool,
}

/// The place kind derived for one piece, with the evidence it was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlaceKind {
    /// Judge the contract sealed.
    pub sealed: bool,
    /// Every row, across every campaign read, that places this piece.
    pub owners: Vec<Owner>,
    /// Campaign directories examined.
    pub campaigns: usize,
    /// Campaigns whose detail plan does not parse, by directory name.
    pub unread: Vec<String>,
    /// Why no campaign was consulted, when none was.
    pub not_consulted: Option<String>,
}

impl PlaceKind {
    /// The evidence, as one line a reader of the audit can check.
    pub fn line(&self) -> String {
        if let Some(why) = &self.not_consulted {
            return format!(
                "place kind: reached (strict) — no campaign was consulted: {why}; a piece is \
                 judged as scenery only when a campaign places it at a `reached: false` node"
            );
        }
        let rows = if self.owners.is_empty() {
            "no campaign's detail plan places this piece".to_string()
        } else {
            self.owners
                .iter()
                .map(|o| {
                    format!(
                        "`{}` at `{}` ({})",
                        o.campaign,
                        o.place,
                        if o.scenery {
                            "`reached: false`"
                        } else {
                            "reached"
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let unread = if self.unread.is_empty() {
            String::new()
        } else {
            format!(
                "; {} campaign(s) whose detail plan this engine cannot parse place nothing it \
                 builds: {}",
                self.unread.len(),
                self.unread
                    .iter()
                    .map(|c| format!("`{c}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        format!(
            "place kind: {} — {} detail-plan row(s) of {} campaign(s) read place this piece: \
             {rows}{unread}",
            if self.sealed {
                "scenery (sealed: built to be seen and never entered)"
            } else {
                "reached (strict)"
            },
            self.owners.len(),
            self.campaigns,
        )
    }
}

/// Derive the place kind of the piece whose declaration document is
/// `meta_path` (a single template's `.json`, or a tile set's manifest).
pub fn place_kind(meta_path: &Path, meta: &PrefabMeta) -> PlaceKind {
    let strict = |why: String| PlaceKind {
        not_consulted: Some(why),
        ..PlaceKind::default()
    };
    let Some(stem) = meta.prefab_id.strip_prefix("prefab/") else {
        return strict(format!(
            "the document's `prefab_id` {:?} is not a `prefab/<stem>` id",
            meta.prefab_id
        ));
    };
    let file_stem = meta_path.file_stem().and_then(|s| s.to_str());
    if file_stem != Some(stem) {
        return strict(format!(
            "the document's file name does not carry its `prefab_id` `{}`, so it is not the \
             piece a campaign resolves for that id",
            meta.prefab_id
        ));
    }
    let library = match meta_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => Path::new(".").to_path_buf(),
    };
    let Some(root) = std::fs::canonicalize(&library)
        .ok()
        .and_then(|l| l.parent().map(Path::to_path_buf))
    else {
        return strict("the library directory has no parent".to_string());
    };
    let campaigns_dir = root.join(CAMPAIGNS_DIR);
    let mut dirs: Vec<_> = match std::fs::read_dir(&campaigns_dir) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => {
            return strict(format!(
                "there is no `{CAMPAIGNS_DIR}/` beside the piece's library"
            ));
        }
    };
    dirs.sort();

    let mut kind = PlaceKind::default();
    for dir in dirs {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let raw = match std::fs::read_to_string(dir.join(DETAIL_PLAN_FILE)) {
            Ok(raw) => raw,
            // Not a detailed campaign: it places no piece.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                kind.campaigns += 1;
                kind.unread.push(name);
                continue;
            }
        };
        kind.campaigns += 1;
        let Ok(plan) = serde_json::from_str::<Envelope<DetailPlanContent>>(&raw) else {
            kind.unread.push(name);
            continue;
        };
        let rows: Vec<_> = plan
            .content
            .details
            .iter()
            .filter(|d| d.piece.as_str() == meta.prefab_id)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let graph = std::fs::read_to_string(dir.join(LAYOUT_GRAPH_FILE))
            .ok()
            .and_then(|raw| serde_json::from_str::<Envelope<LayoutGraphContent>>(&raw).ok());
        for row in rows {
            kind.owners.push(Owner {
                campaign: name.clone(),
                place: row.place.as_str().to_string(),
                scenery: graph
                    .as_ref()
                    .is_some_and(|g| is_scenery(&g.content, &row.place)),
            });
        }
    }
    kind.sealed = !kind.owners.is_empty() && kind.owners.iter().all(|o| o.scenery);
    kind
}
