//! **Where a campaign's places and anchor names come from — asked once.**
//!
//! A campaign's map is its `site-plan.json` (ADR-0032, ADR-0035): there is no
//! second placement. Before the plan exists the campaign has no map yet, and
//! `DW0824` is the finding. What every rule that refuses an anchor name needs
//! from that one fact is the sentence the author reads afterwards — the names
//! the derivation places, or the documents that would place them — and that
//! sentence is chosen here, once, rather than beside each refusal.
//!
//! Nothing here decides whether a reference is wrong; only what the author is
//! told to write instead.

use crate::envelope::Campaign;

/// **What to write instead of an anchor name that does not resolve.**
///
/// On a campaign with a site plan: the names its derivation places and the
/// stations its nodes declare. On one with none yet: the documents that would
/// give it a map.
#[must_use]
pub fn anchor_remedy(c: &Campaign) -> &'static str {
    if c.site_plan.is_some() {
        SITE_PLAN_ANCHORS
    } else {
        NO_MAP_ANCHORS
    }
}

const SITE_PLAN_ANCHORS: &str = "this campaign's map is its site plan, so its anchor names are \
     the ones the derivation places: `anchor/node-<place>` for each place the layout graph \
     declares, `anchor/seam-<edge>` over each barred connection, `anchor/unlock-<edge>` on the \
     far side of a one-sided one, and `spawn` for the entry — plus every `stations[]` name its \
     nodes declare, each answered by the piece bound to its place. Write one of those, or \
     declare this name as a station on the node it belongs to";

const NO_MAP_ANCHORS: &str = "this campaign has no map yet: there is no `site-plan.json`, so \
     nothing places an anchor and no name can resolve. Write the map pipeline \
     (`geometry-brief.json`, then `layout-graph.json`, then `site-plan.json`) and write one of \
     the names its derivation places (`anchor/node-<place>`, `anchor/seam-<edge>`, \
     `anchor/unlock-<edge>`, `spawn`, or a `stations[]` name a node declares)";

/// **Where this campaign writes a lighting declaration**, as a field name: the
/// plan's own `lighting` for the derived interiors, or a place's
/// `boxes[].lighting` for the piece bound to it (spec-0106 §3.2). Bare, with no
/// stage qualifier, because `DW0210` says "declare …" and `DW0211` says "Fix
/// in …".
pub const LIGHTING_FIELD: &str = "the site plan's `lighting` (or, for a bound piece, its place's \
     `boxes[].lighting`)";

/// **What declares how big this campaign's map is** — asked once, by both tiers
/// that need the answer.
///
/// A horizon that BUILDS terrain has to ring a stated extent: validation
/// refuses `DW0855` on a campaign that states none, before a block is placed,
/// and the compiler derives the rectangle from the same answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// **The site plan's `region`**: the whole map's design of record, the
    /// brief's number flowing down. A box outside it is `DW0826`, so nothing a
    /// part does can grow it.
    Region,
    /// **Nothing states one**: the campaign has no site plan.
    Unstated,
}

impl Extent {
    /// Ask the campaign, once.
    #[must_use]
    pub fn of(c: &Campaign) -> Self {
        if c.site_plan.is_some() {
            Self::Region
        } else {
            Self::Unstated
        }
    }

    /// Whether the campaign states an extent at all — the predicate `DW0855`
    /// refuses on.
    #[must_use]
    pub fn is_stated(self) -> bool {
        matches!(self, Self::Region)
    }
}

/// **Whether this campaign's anchor vocabulary can be known at all** — asked
/// before any rule refuses a name for not being in it.
///
/// A site plan's anchor names are DERIVED from `layout-graph.json`. With that
/// document absent the derived set is not empty, it is **unknown**, and
/// `DW0824` is the finding; refusing names against it would print a remedy no
/// author can take.
#[must_use]
pub fn anchor_vocabulary_unknowable(c: &Campaign) -> bool {
    c.site_plan.is_some() && c.layout_graph.is_none()
}
