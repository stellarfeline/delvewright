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

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::Campaign;

use crate::compiler::load::{DETAIL_PLAN_FILE, LAYOUT_GRAPH_FILE};
use crate::grammar::contract::Arrival;
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

/// **Whether `node` is reached only by being carried** (spec-0083 §7): a
/// reached place every one of whose body connections is a `carry` edge — at
/// least one, and no edge the geometry owes a seam ([`Edge::has_seam`]). Such a
/// place has no seam for its piece's entry to meet; a `vision` edge carries no
/// body and changes nothing.
///
/// [`Edge::has_seam`]: delvewright_dsl::layout::Edge::has_seam
#[must_use]
pub fn is_carried_only(graph: &LayoutGraphContent, node: &NodeId) -> bool {
    use delvewright_dsl::layout::Edge;
    if !graph.nodes.iter().any(|n| &n.id == node && n.reached) {
        return false;
    }
    let incident: Vec<&Edge> = graph
        .edges
        .iter()
        .filter(|e| e.a() == node || e.b() == node)
        .collect();
    incident.iter().any(|e| matches!(e, Edge::Carry { .. }))
        && !incident.iter().any(|e| e.has_seam())
}

/// **How a body arrives in `node`** ([`Arrival`]), in the campaign's own
/// vocabulary: scenery when the graph says `reached: false`; carried when every
/// body connection is a `carry` edge, set down at the `to` mark of each link
/// landing in it ([`crate::compiler::link::landings`]); walked otherwise. The
/// one rule `delvec detail` and the audit ([`place_kind`]) both read the kind
/// through. A campaign with no layout graph places by `areas[]`, and every
/// place of it is walked.
#[must_use]
pub fn arrival_of(campaign: &Campaign, node: &NodeId) -> Arrival {
    let Some(graph) = campaign.layout_graph.as_ref().map(|g| &g.content) else {
        return Arrival::Walked;
    };
    if is_scenery(graph, node) {
        Arrival::Never
    } else if is_carried_only(graph, node) {
        Arrival::Carried {
            landings: crate::compiler::link::landings(campaign, node),
        }
    } else {
        Arrival::Walked
    }
}

/// [`arrival_of`], with each landing renamed into the piece's vocabulary
/// through a row's `anchors` map (campaign name → piece anchor). A landing the
/// map does not bind keeps its campaign name, so the contract names the anchor
/// it looked for and did not find.
#[must_use]
pub fn arrival_in_piece(arrival: Arrival, anchors: &BTreeMap<String, String>) -> Arrival {
    match arrival {
        Arrival::Carried { landings } => Arrival::Carried {
            landings: landings
                .into_iter()
                .map(|l| anchors.get(&l).cloned().unwrap_or(l))
                .collect::<BTreeSet<String>>(),
        },
        other => other,
    }
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
    /// How a body arrives at that node, landings named in the piece's
    /// vocabulary through the row's `anchors` map.
    pub arrival: Arrival,
}

/// The place kind derived for one piece, with the evidence it was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlaceKind {
    /// Judge the contract sealed.
    pub sealed: bool,
    /// How the contract's entry is judged: the arrival every owner agrees on,
    /// or [`Arrival::Walked`] — the strict kind — when they disagree or none
    /// was consulted.
    pub arrival: Arrival,
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
                            "`reached: false`".to_string()
                        } else if let Arrival::Carried { .. } = o.arrival {
                            o.arrival.describe()
                        } else {
                            "reached".to_string()
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
            match &self.arrival {
                Arrival::Never =>
                    "scenery (sealed: built to be seen and never entered)".to_string(),
                Arrival::Carried { .. } => format!(
                    "carried (joined to the map only by `carry` edges; {})",
                    self.arrival.describe()
                ),
                Arrival::Walked => "reached (strict)".to_string(),
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
        // A carried place's landings are the quests' links, so the campaign is
        // read whole only when a row places the piece at one; a campaign that
        // does not load is named unread and its row judged strictly.
        let mut loaded: Option<Option<Campaign>> = None;
        for row in rows {
            let scenery = graph
                .as_ref()
                .is_some_and(|g| is_scenery(&g.content, &row.place));
            let carried = graph
                .as_ref()
                .is_some_and(|g| is_carried_only(&g.content, &row.place));
            let arrival = if scenery {
                Arrival::Never
            } else if carried {
                let campaign = loaded.get_or_insert_with(|| {
                    crate::compiler::load::load_campaign_dir(&dir)
                        .ok()
                        .and_then(|l| crate::compiler::load::parse_loaded(&l).ok())
                });
                match campaign {
                    Some(c) => arrival_in_piece(arrival_of(c, &row.place), &row.anchors),
                    None => {
                        if !kind.unread.contains(&name) {
                            kind.unread.push(name.clone());
                        }
                        Arrival::Walked
                    }
                }
            } else {
                Arrival::Walked
            };
            kind.owners.push(Owner {
                campaign: name.clone(),
                place: row.place.as_str().to_string(),
                scenery,
                arrival,
            });
        }
    }
    kind.sealed = !kind.owners.is_empty() && kind.owners.iter().all(|o| o.scenery);
    kind.arrival = match kind.owners.split_first() {
        Some((first, rest)) if rest.iter().all(|o| o.arrival == first.arrival) => {
            first.arrival.clone()
        }
        _ => Arrival::Walked,
    };
    kind
}
