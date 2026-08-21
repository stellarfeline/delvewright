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
//! `delvewright_compiler::blockout`'s module docs): inversion is not forbidden,
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
use crate::siteplan::PlacedBox;

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

/// **How many courses of floor a piece owns under its play space** — the whole
/// of the fabric split (spec-0050 §3), as one number.
///
/// A floor's material is the place's own voice, so the course the walk plane
/// stands on belongs to the piece; everything else the derivation writes around
/// a box — walls, ceiling, seam frames, party planes — is structure, and
/// structure is the whole's. Named rather than spelled `1` at each site so that
/// [`Frame::of`], [`Frame::datum_y`] and every reader of either move together.
pub const FLOOR_COURSE: i64 = 1;

/// **What a piece owns**: the box's play space, grown [`FLOOR_COURSE`] downward
/// to take in the floor the walk plane stands on.
///
/// One derivation, [`Frame::of`], and four readers that must not disagree: the
/// exactness check (`DW0843`), the face check (`DW0844`), the placement inside
/// `Plan::build`, and `delvec allocation`. Two of them computing "where does
/// this piece go" independently is how a builder and its observer come to agree
/// about a world neither describes — the failure `PlacedBox`'s own note records
/// one layer down.
///
/// What is **not** in here is as load-bearing as what is: every vertical party
/// plane, every unshared shell face, every seam frame, every derived stair in an
/// unbound host and every bar in a vertical-plane seam stay whole-owned. The
/// piece dresses its side of a wall from within its own frame; the party plane
/// is structure, and structure is the whole's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The place this frames.
    pub node: NodeId,
    /// Inclusive low corner in world cells — the floor course.
    pub lo: [i64; 3],
    /// Inclusive high corner in world cells — the top of the play space.
    pub hi: [i64; 3],
}

impl Frame {
    /// The frame a detailed place's piece must exactly fill.
    ///
    /// The play space plus the floor course under it, and nothing else. A
    /// floor's material is the place's own voice — the datum convention already
    /// says the walk plane is the plan's, so the handing states the datum in
    /// piece-local coordinates and the seam-rise proofs hold it.
    #[must_use]
    pub fn of(b: &PlacedBox) -> Frame {
        let (lo, hi) = b.space();
        Frame {
            node: b.node.clone(),
            lo: [lo[0], lo[1] - FLOOR_COURSE, lo[2]],
            hi,
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
    /// surface must be.
    ///
    /// It is [`FLOOR_COURSE`] because the frame is the play space grown exactly
    /// that many courses downward, so the number is a consequence of
    /// [`Frame::of`] and moves with it rather than being a literal repeated at
    /// four call sites.
    #[must_use]
    pub fn datum_y(&self) -> i64 {
        FLOOR_COURSE
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

/// The synthesized anchor names **this place owes** (spec-0050 §6).
///
/// Exactly the synthesized names whose bearer is this box: its own
/// `anchor/node-…`; `spawn` when it is the entry node; each `anchor/unlock-…`
/// whose `opens_from` side it is. A gate region (`anchor/seam-…`) is never owed
/// — it is whole fabric, and one over a bound upper box's floor course resolves
/// to the seam's allocated cells as ever, which `DW0844`'s barred row has
/// already required the piece to bar.
///
/// Derived from the same two documents [`crate::siteplan::synthesized_anchors`]
/// reads, and **proven to partition it**: a test walks every node, unions the
/// owed sets with the seam anchors, and asserts equality with the one authority.
/// A name that stopped being owed by anyone would otherwise be a name the piece
/// is never asked for and the campaign still resolves.
#[must_use]
pub fn owed_anchors(c: &Campaign, node: &NodeId) -> BTreeSet<String> {
    crate::siteplan::owed_anchors(c, node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::NodeId;
    use crate::siteplan::PlacedBox;

    fn a_box() -> PlacedBox {
        PlacedBox {
            node: NodeId("node/hall".into()),
            foot: [10, 25, 4, 19],
            floor: 64,
            clearance: 8,
            open: false,
        }
    }

    #[test]
    fn a_frame_is_the_play_space_plus_one_course_below() {
        let b = a_box();
        let (lo, hi) = b.space();
        let f = Frame::of(&b);
        assert_eq!(
            f.lo,
            [lo[0], lo[1] - 1, lo[2]],
            "one course below the walk plane"
        );
        assert_eq!(f.hi, hi, "and nothing above the play space");
        assert_eq!(
            f.extent(),
            [16, 9, 16],
            "footprint by clearance + the floor course"
        );
        assert_eq!(
            f.datum_y(),
            1,
            "the walk plane sits one course up, piece-local"
        );
        assert_eq!(f.to_local([lo[0], b.floor, lo[2]]), [0, 1, 0]);
        assert!(
            f.contains([lo[0], b.floor - 1, lo[2]]),
            "the floor course is the piece's"
        );
        assert!(
            !f.contains([lo[0], b.floor - 2, lo[2]]),
            "and nothing under it is"
        );
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
