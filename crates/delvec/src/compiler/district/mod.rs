//! **A delve authored in parallel by district** (spec-0103).
//!
//! The layout graph partitions its places into districts (`districts[]`,
//! `DW1003` in `dsl::layout`). This module is everything the engine does with
//! that partition beyond declaring it:
//!
//! * [`fragment`] — a campaign directory may hold `districts/<x>/` for each
//!   declared district, and in it **fragments** of six stage documents. The
//!   loader reads them in sorted district order and unions each fragment's lists
//!   into the stage document's after the whole's own, so every check, every
//!   proof and every byte of the build is a function of the union and is
//!   indifferent to who wrote what (`DW1005` for a field the whole owns).
//! * [`located`] — every object a fragment carries stands in its district, and
//!   every reference it makes resolves to the whole or to its own district
//!   (`DW1004`).
//! * [`census`] — `delvec district list`: per district, what it holds, beside
//!   the guide's size advice (`DW1007`, which advises and never refuses).
//! * [`writes`] — the write set a district's worker may touch, derived from the
//!   declaration and the plan, and `delvec district writes`, which refuses a
//!   path outside it (`DW1008`).
//! * [`packet`] — `delvec district packet`: the worker's handover, derived from
//!   the documents on every invocation and an input to nothing.
//! * [`ledger`] — `validation/districts.json`, the object-to-district ledger
//!   `delvec build` writes, and the `[district/<x>]` routing of a diagnostic
//!   that names an object of one district.
//! * [`scope`] — `delvec validate --district <x>`: a worker's tree holds its
//!   siblings' planned quests unexpanded, and the refusals that stand only on
//!   that absence are reported under one line (`DW1006`; `DW1009` for a name no
//!   district carries).
//!
//! A campaign whose graph declares no district is the whole authored by one
//! agent: nothing here reads anything, nothing refuses, and every binding line
//! says `0 district(s)`.
//!
//! Determinism (ADR-0006): every map is a `BTreeMap`, every walk is over a
//! sorted directory listing or a document's own order.

pub mod census;
pub mod fragment;
pub mod ledger;
pub mod located;
pub mod packet;
pub mod scope;
pub mod writes;

use std::collections::BTreeMap;

use delvewright_dsl::{Campaign, DwCode, Edge, ExitTier, NodeId, OpensFrom, Stage};

delvewright_dsl::dw_code! {
    /// `DW1004`: an object a district's fragment carries stands outside its
    /// district, or reaches into a sibling district (spec-0103 §7.3).
    pub const DW_OUTSIDE_DISTRICT: DwCode = DwCode::new("DW1004", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1005`: a district fragment carries a field, or a whole document, that
    /// the whole owns (spec-0103 §7.2).
    pub const DW_WHOLE_OWNS: DwCode = DwCode::new("DW1005", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1006`: informational — under `validate --district <x>`, the planned
    /// quests of other districts are not expanded in this tree, and the
    /// refusals that stand only on that absence are reported beneath this line
    /// as advisories (spec-0103 §7.5).
    pub const DW_SIBLINGS_UNEXPANDED: DwCode = DwCode::new("DW1006", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1007`: advisory — a district outside the partitioning guide's size
    /// advice (spec-0103 §4.2). Never refuses.
    pub const DW_DISTRICT_SIZE: DwCode = DwCode::new("DW1007", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1008`: a path outside the district's write set (spec-0103 §7.6) —
    /// `delvec district writes`, at the commit hook and at integration.
    pub const DW_WRITE_OUTSIDE: DwCode = DwCode::new("DW1008", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1009`: `--district` names no district the layout graph declares
    /// (spec-0103 §7.5).
    pub const DW_NO_SUCH_DISTRICT: DwCode = DwCode::new("DW1009", ExitTier::Build);
}

/// The directory under a campaign that holds one directory per district.
pub const DISTRICTS_DIR: &str = "districts";

/// The prefix every district id carries.
pub const DISTRICT_PREFIX: &str = "district/";

/// **What a district fragment may carry** (spec-0103 §7.1) — one row per stage
/// document a district fragments.
#[derive(Debug, Clone, Copy)]
pub struct FragmentKind {
    /// The file name, the same as the whole's document.
    pub file: &'static str,
    /// The stage the fragment is a fragment of.
    pub stage: Stage,
    /// The `content` lists it may carry; every other key is the whole's.
    pub lists: &'static [&'static str],
}

/// The six documents a district fragments, and the lists each may carry.
/// A `.json` file under `districts/<x>/` with any other name is `DW1005`.
pub const FRAGMENTS: [FragmentKind; 6] = [
    FragmentKind {
        file: "npcs.json",
        stage: Stage::Npcs,
        lists: &["npcs"],
    },
    FragmentKind {
        file: "quests.json",
        stage: Stage::Quests,
        lists: &[
            "quests",
            "triggers",
            "waves",
            "actors",
            "assemblies",
            "lethal_volumes",
            "pulses",
        ],
    },
    FragmentKind {
        file: "dialogue.json",
        stage: Stage::Dialogue,
        lists: &["dialogues"],
    },
    FragmentKind {
        file: "layout-graph.json",
        stage: Stage::LayoutGraph,
        lists: &["nodes"],
    },
    FragmentKind {
        file: "detail-plan.json",
        stage: Stage::DetailPlan,
        lists: &["details"],
    },
    FragmentKind {
        file: "design.json",
        stage: Stage::Design,
        lists: &["references"],
    },
];

/// The row of [`FRAGMENTS`] for a file name.
#[must_use]
pub fn fragment_kind(file: &str) -> Option<&'static FragmentKind> {
    FRAGMENTS.iter().find(|k| k.file == file)
}

/// `district/<x>` → `<x>`, the directory name.
#[must_use]
pub fn dir_of(id: &str) -> &str {
    id.strip_prefix(DISTRICT_PREFIX).unwrap_or(id)
}

/// `node/<stem>` → `<stem>`.
#[must_use]
pub fn stem_of(node: &str) -> &str {
    node.strip_prefix("node/").unwrap_or(node)
}

/// **The partition as the engine reads it**: which district each place is in,
/// and which district each anchor the derivation provides stands in.
///
/// Built from the parsed campaign — the union of the whole and every fragment —
/// so a station a fragment declares is located by the node it was merged onto.
#[derive(Debug, Clone, Default)]
pub struct Partition {
    /// Declared district ids, in document order.
    pub ids: Vec<String>,
    /// place id → district id.
    pub node: BTreeMap<String, String>,
    /// anchor name → the place it stands in, for every anchor a site-plan
    /// campaign's derivation provides (`spawn`, `anchor/node-…`, stations,
    /// `anchor/unlock-…`). A barred seam's gate region (`anchor/seam-…`) stands
    /// in a place only when both its ends are in one district; a seam between
    /// two districts is the boundary and belongs to the whole.
    pub anchor_place: BTreeMap<String, String>,
    /// anchor name → district, for the seam regions inside one district.
    pub seam_district: BTreeMap<String, String>,
}

impl Partition {
    /// Read the partition off a parsed campaign. Empty when the graph declares
    /// no district.
    #[must_use]
    pub fn of(c: &Campaign) -> Partition {
        let mut p = Partition::default();
        let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
            return p;
        };
        if graph.districts.is_empty() {
            return p;
        }
        for d in &graph.districts {
            p.ids.push(d.id.as_str().to_string());
            for n in &d.nodes {
                p.node
                    .entry(n.0.clone())
                    .or_insert_with(|| d.id.as_str().to_string());
            }
        }
        p.anchor_place.insert(
            delvewright_dsl::ENTRY_ANCHOR.to_string(),
            graph.entry.0.clone(),
        );
        for n in &graph.nodes {
            p.anchor_place
                .insert(delvewright_dsl::node_anchor(&n.id), n.id.0.clone());
            for s in &n.stations {
                p.anchor_place
                    .insert(s.anchor.as_str().to_string(), n.id.0.clone());
            }
        }
        for e in &graph.edges {
            let Edge::Barred { id, opens_from, .. } = e else {
                continue;
            };
            let (a, b) = (e.a().0.as_str(), e.b().0.as_str());
            if let (Some(da), Some(db)) = (p.node.get(a), p.node.get(b))
                && da == db
            {
                p.seam_district
                    .insert(delvewright_dsl::seam_anchor(id), da.clone());
            }
            let side = match opens_from {
                OpensFrom::A => e.a(),
                OpensFrom::B => e.b(),
                OpensFrom::Either => continue,
            };
            p.anchor_place
                .insert(delvewright_dsl::seam_unlock_anchor(id), side.0.clone());
        }
        p
    }

    /// True when the graph declares at least one district.
    #[must_use]
    pub fn is_declared(&self) -> bool {
        !self.ids.is_empty()
    }

    /// The district a place is in.
    #[must_use]
    pub fn of_node(&self, node: &str) -> Option<&str> {
        self.node.get(node).map(String::as_str)
    }

    /// The district an anchor name stands in, or `None` when it stands in the
    /// whole (a boundary seam) or the name is not one the derivation provides.
    /// A name written `area/<a>:anchor/<x>` is read by its anchor part.
    #[must_use]
    pub fn of_anchor(&self, name: &str) -> Option<&str> {
        let bare = name.rsplit_once(':').map_or(name, |(_, a)| a);
        if let Some(d) = self.seam_district.get(bare) {
            return Some(d);
        }
        self.anchor_place
            .get(bare)
            .and_then(|place| self.of_node(place))
    }

    /// True when the name is one the derivation provides at all.
    #[must_use]
    pub fn knows_anchor(&self, name: &str) -> bool {
        let bare = name.rsplit_once(':').map_or(name, |(_, a)| a);
        self.anchor_place.contains_key(bare)
            || self.seam_district.contains_key(bare)
            || bare.starts_with("anchor/seam-")
    }

    /// The places of a district, in the graph's declared order of the district.
    #[must_use]
    pub fn places_of(&self, c: &Campaign, district: &str) -> Vec<NodeId> {
        c.layout_graph
            .as_ref()
            .and_then(|g| {
                g.content
                    .districts
                    .iter()
                    .find(|d| d.id.as_str() == district)
            })
            .map(|d| d.nodes.clone())
            .unwrap_or_default()
    }
}

/// Render `a`, `b` and `c` with backticks.
#[must_use]
pub fn ticked(items: &[String]) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    items
        .iter()
        .map(|i| format!("`{i}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
