//! **The detail plan: a place is detailed inside the box the whole gave it**
//! (spec-0050) — pipeline stage 6.
//!
//! One campaign stage document, `detail-plan.json`, and its whole surface is
//! *which piece stands in which place, and which of the piece's anchors answers
//! each name the campaign already bound to that place*.
//!
//! # What the schema deliberately cannot say
//!
//! There is **no coordinate, no region, no extent, no datum, no seam and no
//! offset** below — absent fields, not optional ones. A detail document is
//! therefore *structurally unable* to move its box, its datum or its seams,
//! because the schema has no spelling for any of them; the only path from a
//! [`Detail`] row to placed bytes runs through the compiler computing the frame
//! ([`Frame::of`]) from the site plan, inside `Plan::build`, which is the only
//! constructor every world-reaching verb goes through.
//!
//! This is the same tooth the blockout's is (`crate::siteplan`, and
//! `delvec::compiler::blockout`'s module docs): inversion is not forbidden,
//! it is **uncompilable**. A part that wants different traversal takes the one
//! escalation path there is — a site-plan revision, which moves the plan hash,
//! which re-opens the walk gate, which re-runs the whole's walk.
//!
//! # Partial by construction
//!
//! Detail is per-place. The derivation masses every *unbound* box exactly as it
//! did at stage 5, so a campaign with one detailed place builds, walks, renders
//! and reds like any other — the broken intermediate is a real, lookable object
//! at every point between "no detail" and "fully detailed".

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::Campaign;
use crate::ids::{NodeId, PrefabId};
#[cfg(doc)]
use crate::owed_anchors;
use crate::siteplan::{PlacedBox, Site, SitePlan};

// ---------------------------------------------------------------------------
// The document (spec-0050 §1)
// ---------------------------------------------------------------------------

/// The `detail-plan` stage document's payload.
///
/// Two fields, and the second is the whole mechanism. See the module docs for
/// what is deliberately absent and why that absence is the design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DetailPlanContent {
    /// **The whole's material vocabulary**: role name → block, handed into every
    /// allocation (spec-0050 §4).
    ///
    /// Style surface, and **gated by nothing** — deliberately, and the reason is
    /// a standing decision rather than an omission: materials are style, style
    /// authority is rank-only (spec-0028), and a piece exported against a stale
    /// palette is a render finding rather than a machine one. The provenance row
    /// in the piece's own metadata already freezes what it was actually built
    /// from.
    ///
    /// Absent means the whole states no vocabulary, which is a different claim
    /// from an empty one — an empty map is the positive statement that the roles
    /// are the piece's own business.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<BTreeMap<String, String>>,
    /// One row per **detailed place**. A place with no row is massed by the
    /// derivation exactly as it was at stage 5.
    pub details: Vec<Detail>,
}

/// One place, detailed: the piece that stands in it and the anchor re-binding
/// that keeps the campaign's own names working.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    /// The layout-graph node whose box this piece fills.
    ///
    /// A node, not a box and not a region: the ordering tooth at type level, the
    /// same one `PlanBox::node` is. There is no other way to say *where*.
    pub place: NodeId,
    /// The piece — a prefab: frozen bytes plus metadata carrying a resolved
    /// spatial contract, faces and anchors.
    ///
    /// The engine consumes the object class, never the tool that made it
    /// (spec-0050 §1): a grammar program's export and a hand-admitted kit piece
    /// are the same object here, and every gate below reads metadata and bytes,
    /// indifferent to provenance.
    pub piece: PrefabId,
    /// Each synthesized anchor name this place **owes** ([`owed_anchors`]) →
    /// an anchor of the piece.
    ///
    /// The re-binding is what lets a kit piece keep its own vocabulary while the
    /// campaign keeps its own: the quest layer bound `anchor/node-…` to this
    /// place at stage 3, before any detail existed, so detailing must never
    /// force a quest edit.
    #[serde(default)]
    pub anchors: BTreeMap<String, String>,
}

impl DetailPlanContent {
    /// The row that binds `node`, if any.
    #[must_use]
    pub fn detail_of(&self, node: &NodeId) -> Option<&Detail> {
        self.details.iter().find(|d| &d.place == node)
    }
}

// ---------------------------------------------------------------------------
// The frame (spec-0050 §3) — ONE derivation, four readers
// ---------------------------------------------------------------------------

/// **What a piece owns**: the place's claim — the ground under its plot, its
/// floor course, its play space, the one-cell ring its walls may stand in and,
/// roofed, its lid and the roof zone the plan declares — minus the ring's fixed
/// ground and every cell spec-0098 §2's ownership rule awards to another place
/// or to nobody (`siteplan::Site::owner`).
///
/// One derivation, [`Frame::of`], and every reader that must not disagree: the
/// exactness check (`DW0843`), the face check (`DW0844`), the void and ring
/// checks (`DW0987`, `DW0990`), the blockout's stand-ins and holes, the
/// placement inside `Plan::build`, and `delvec allocation`. Two of them
/// computing "whose cell is this" independently is how a builder and its
/// observer come to agree about a world neither describes.
///
/// The frame is the **bounding box** of the owned cells, because a piece is a
/// structure template and a template is a box. Cells inside it the place does
/// not own are its [`Frame::voids`]: the piece holds `structure_void` there and
/// the owner's block shows through, in the game and in the model alike.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The place this frames.
    pub node: NodeId,
    /// Inclusive low corner in world cells.
    pub lo: [i64; 3],
    /// Inclusive high corner in world cells.
    pub hi: [i64; 3],
    /// The walk plane's world `y`.
    pub floor: i64,
    /// The claim's bottom: the lowest course of ground under the plot.
    pub bottom: i64,
    /// The cells the place owns, merged into disjoint AABBs.
    pub owned: Vec<crate::siteplan::Aabb>,
    /// How many cells the place owns.
    pub owned_cells: usize,
    /// Every cell of the frame the place does not own, with who does.
    pub voids: Vec<crate::siteplan::Void>,
    /// The ring's fixed ground inside the frame, each cell with the block the
    /// whole writes there (spec-0098 §2 rule 0).
    pub fixed: Vec<([i64; 3], String)>,
    /// Eaves cells the plan clipped at a neighbour's shell.
    pub clipped: Vec<(crate::siteplan::Aabb, NodeId)>,
}

impl Frame {
    /// The frame of the plan's `i`th place, derived from the whole plan:
    /// who owns a party plane depends on what stands on its other side, and the
    /// ring's ground on the site's terrain.
    #[must_use]
    pub fn of(site: &Site<'_>, i: usize) -> Frame {
        let b = &site.boxes[i];
        let o = site.ownership(i);
        Frame {
            node: b.node.clone(),
            lo: o.lo,
            hi: o.hi,
            floor: b.floor,
            bottom: site.bottom(i),
            owned: o.owned,
            owned_cells: o.owned_cells,
            voids: o.voids,
            fixed: o.fixed,
            clipped: o.clipped,
        }
    }

    /// The frame's size in cells, `[x, y, z]` — what a piece's structure size
    /// must equal on every axis (`DW0843`).
    #[must_use]
    pub fn extent(&self) -> [i64; 3] {
        [
            self.hi[0] - self.lo[0] + 1,
            self.hi[1] - self.lo[1] + 1,
            self.hi[2] - self.lo[2] + 1,
        ]
    }

    /// The walk plane's **piece-local** `y` — where the piece's own floor
    /// surface must be: one over the floor course, plus every course of ground
    /// the claim reaches below it (spec-0098 §2).
    #[must_use]
    pub fn datum_y(&self) -> i64 {
        self.floor - self.lo[1]
    }

    /// A world cell in this frame's local coordinates.
    #[must_use]
    pub fn to_local(&self, world: [i64; 3]) -> [i64; 3] {
        [
            world[0] - self.lo[0],
            world[1] - self.lo[1],
            world[2] - self.lo[2],
        ]
    }

    /// True when `world` is inside the frame, inclusive.
    #[must_use]
    pub fn contains(&self, world: [i64; 3]) -> bool {
        (0..3).all(|i| world[i] >= self.lo[i] && world[i] <= self.hi[i])
    }

    /// True when `world` is a cell the place owns — inside the frame and in no
    /// void.
    #[must_use]
    pub fn owns(&self, world: [i64; 3]) -> bool {
        self.owned
            .iter()
            .any(|(lo, hi)| (0..3).all(|i| world[i] >= lo[i] && world[i] <= hi[i]))
    }

    /// Every frame of a campaign's plan, in plan document order.
    #[must_use]
    pub fn all(c: &Campaign) -> Vec<(Frame, PlacedBox)> {
        let plan = SitePlan::of(c);
        let site = plan.site();
        (0..plan.boxes.len())
            .map(|i| (Frame::of(&site, i), plan.boxes[i].clone()))
            .collect()
    }

    /// True when `world` is fixed ground inside this frame.
    #[must_use]
    pub fn is_fixed(&self, world: [i64; 3]) -> bool {
        self.fixed.iter().any(|(c, _)| *c == world)
    }
}

// ---------------------------------------------------------------------------
// What a detail plan binds, for the derivation and for every gate
// ---------------------------------------------------------------------------

/// The layout-graph nodes a campaign's detail plan binds, by name.
///
/// Read by the blockout derivation, which stops massing what a binding owns —
/// so the answer must come from the document rather than from a second opinion
/// about which rows are "valid": a row naming a node the graph does not have is
/// `DW0842`'s finding, and a derivation that quietly disagreed with the gate
/// about which places are bound would be a world neither describes.
///
/// Empty for a campaign with no detail plan, which is every campaign that
/// existed before this version — and is why such a campaign's output does not
/// move by a byte.
#[must_use]
pub fn bound_places(c: &Campaign) -> BTreeSet<String> {
    c.detail_plan
        .as_ref()
        .map(|e| {
            e.content
                .details
                .iter()
                .map(|d| d.place.0.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// True when `node`'s box is bound by a `details[]` row.
#[must_use]
pub fn is_bound(c: &Campaign, node: &NodeId) -> bool {
    c.detail_plan
        .as_ref()
        .is_some_and(|e| e.content.detail_of(node).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::NodeId;
    use crate::siteplan::{Ground, Owner, PlacedBox, Roof, Site};

    fn a_box() -> PlacedBox {
        PlacedBox {
            node: NodeId("node/hall".into()),
            foot: [10, 25, 4, 19],
            floor: 64,
            clearance: 8,
            open: false,
            roof: None,
        }
    }

    fn frame(b: &PlacedBox, ground: &Ground) -> Frame {
        let boxes = vec![b.clone()];
        let site = Site::new(&boxes, &[], ground);
        Frame::of(&site, 0)
    }

    /// Criterion 2: the expected extents are literals worked out here from the
    /// box's own numbers, never the frame compared with itself.
    #[test]
    fn a_frame_is_the_shell_when_nothing_stands_beside_it() {
        let b = a_box();
        let f = frame(&b, &Ground::solid("minecraft:stone"));
        // Footprint x 10..25, z 4..19; play y 64..71; lid 72; floor course 63.
        assert_eq!(f.lo, [9, 63, 3], "the floor course and the ring");
        assert_eq!(f.hi, [26, 72, 20], "the ring and the lid");
        assert_eq!(f.extent(), [18, 10, 18]);
        assert_eq!(f.datum_y(), 1, "the walk plane sits one course up");
        assert!(f.owns([9, 64, 4]), "the wall is the piece's");
        assert!(!f.owns([9, 63, 4]), "the ring's ground is fixed");
        assert!(f.is_fixed([9, 63, 4]));
        assert!(!f.contains([8, 64, 4]), "and nothing beyond it is");
        // 18 × 10 × 18 minus the ring's 68 ground cells.
        assert_eq!(f.owned_cells, 18 * 10 * 18 - 68);
        assert_eq!(f.fixed.len(), 68);
        assert!(f.voids.iter().all(|v| v.owner == Owner::Ground));
    }

    #[test]
    fn an_open_frame_has_no_lid_and_a_roofed_one_rises_by_its_roof() {
        let mut open = a_box();
        open.open = true;
        open.clearance = 3;
        let f = frame(&open, &Ground::solid("minecraft:stone"));
        assert_eq!(f.extent(), [18, 4, 18], "floor course + 3, no lid");
        let mut roofed = a_box();
        roofed.roof = Some(Roof {
            courses: 4,
            eaves: 1,
        });
        let f = frame(&roofed, &Ground::solid("minecraft:stone"));
        assert_eq!(
            f.extent(),
            [20, 14, 20],
            "eaves 1 each side; floor + 8 + lid + 4"
        );
        assert_eq!(f.lo, [8, 63, 2]);
        assert_eq!(f.hi, [27, 76, 21]);
        assert_eq!(f.datum_y(), 1);
        // The columns beyond the walls under the eaves are nobody's: voids.
        assert!(f.voids.iter().any(|v| v.owner == Owner::Nobody));
    }

    /// On an open site whose terrain stands lower than the floor, the claim
    /// reaches down to the terrain's surface, so the frame grows downward.
    #[test]
    fn a_plinth_over_low_ground_claims_down_to_the_ground() {
        let b = a_box();
        let f = frame(
            &b,
            &Ground::open_flat(60, "minecraft:grass_block", "minecraft:dirt"),
        );
        assert_eq!(f.bottom, 60);
        assert_eq!(f.lo[1], 60);
        assert_eq!(f.datum_y(), 4);
        assert!(f.is_fixed([9, 60, 4]), "the ring's surface cell is fixed");
        assert!(
            f.owns([9, 61, 4]),
            "above the ground the ring is the piece's"
        );
        assert!(
            f.owns([12, 61, 6]),
            "the ground under the plot is the piece's"
        );
        let fixed: Vec<&String> = f.fixed.iter().map(|(_, b)| b).collect();
        assert!(fixed.iter().all(|b| b.as_str() == "minecraft:grass_block"));
    }

    /// The schema's absence is the design, so it is asserted rather than
    /// described: a document naming a coordinate does not parse.
    #[test]
    fn a_detail_row_cannot_state_where_anything_goes() {
        for extra in [
            r#""min": [0, 0, 0]"#,
            r#""at": [1, 2]"#,
            r#""region": {"min": [0, 0, 0], "extent": [1, 1, 1]}"#,
            r#""datum": "datum/grade""#,
            r#""offset": [0, 1, 0]"#,
            r#""extent": [4, 4, 4]"#,
            r#""seams": []"#,
        ] {
            let src = format!(
                r#"{{"place": "node/hall", "piece": "prefab/hall", "anchors": {{}}, {extra}}}"#
            );
            let err = serde_json::from_str::<Detail>(&src)
                .expect_err("a detail row has no spelling for where anything goes");
            assert!(
                err.to_string().contains("unknown field"),
                "the refusal is the schema's, not a check's: {err}"
            );
        }
    }

    #[test]
    fn a_detail_plan_cannot_state_where_anything_goes() {
        for extra in [
            r#""region": {"min": [0, 0, 0], "extent": [1, 1, 1]}"#,
            r#""datums": []"#,
            r#""boxes": []"#,
            r#""seams": []"#,
            r#""origin": [0, 0, 0]"#,
        ] {
            let src = format!(r#"{{"details": [], {extra}}}"#);
            let err = serde_json::from_str::<DetailPlanContent>(&src)
                .expect_err("a detail plan has no spelling for geometry");
            assert!(
                err.to_string().contains("unknown field"),
                "the refusal is the schema's, not a check's: {err}"
            );
        }
    }

    #[test]
    fn the_document_round_trips_and_defaults_to_nothing_bound() {
        let d: DetailPlanContent = serde_json::from_str(r#"{"details": []}"#).unwrap();
        assert!(d.palette.is_none(), "absent is not empty");
        assert!(d.details.is_empty());
        let d: DetailPlanContent = serde_json::from_str(
            r#"{"palette": {"role/wall": "minecraft:stone_bricks"},
                "details": [{"place": "node/hall", "piece": "prefab/hall"}]}"#,
        )
        .unwrap();
        assert_eq!(d.details[0].anchors.len(), 0, "`anchors` defaults to empty");
        assert!(d.detail_of(&NodeId("node/hall".into())).is_some());
        assert!(d.detail_of(&NodeId("node/annex".into())).is_none());
    }
}
