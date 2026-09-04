//! **The whole owns the space and hands out boxes** (spec-0049 §4) — pipeline
//! stage 4, the geometric embedding of the layout graph.
//!
//! One campaign stage document, `site-plan.json`: the whole map's design of
//! record. It says where the region is, which plane each place stands on, what
//! footprint each place gets, where two places connect and through what opening,
//! what mass the whole itself owns, and which of the brief's numbers the plan is
//! held to.
//!
//! Everything here is decided **upstream of any geometry**. No block exists yet;
//! nothing in this module reads one. What is being judged is whether the plan is
//! a plan — whether the boxes fit in the region and not in each other, whether
//! two places that claim to connect really touch, and whether the numbers the
//! brief fixed still hold once the boxes are drawn.
//!
//! # Extent flows down, and it is unrepresentable for it to flow up
//!
//! The reset this stage answers was caused by parts choosing their own size and
//! the whole becoming whatever they added up to. So [`SitePlanContent::region`]
//! is a **required field with no derived spelling**: there is no
//! `"region": "fit"`, no default, and no constructor anywhere that computes one
//! from the boxes. A plan cannot state its extent as a consequence; it can only
//! state it, and `DW0826` then refuses a box that does not fit, naming the box
//! rather than the region. That is not a check — it is the absence of a way to
//! write the other thing.
//!
//! # Seams are allocated, not discovered
//!
//! A seam is placed by the plan, on a face the two boxes already share, at cells
//! the plan names. Two places therefore connect **by construction**: the
//! two-pieces-cannot-mate failure is resolved here, where both boxes are still
//! free to move, and never later between two finished buildings. `DW0828` and
//! `DW0829` are what make the allocation real rather than a claim.
//!
//! # One authority per fact
//!
//! Three places where the obvious shape would have carried two:
//!
//! * A box's floor is its **datum** and nothing else. The spec's `min` carried a
//!   `y` beside the declared floor, which is two numbers for one plane and no
//!   rule about which wins; here a box is a footprint (`min`/`extent` on `x`
//!   and `z`) standing at a [`Floor`]. §9 records the departure.
//! * A seam's **rise is derived** from the two boxes' floors. Authoring it would
//!   be authoring arithmetic — unlike the layout graph's `critical_path`, which
//!   is authored precisely because it is a *choice* among many, a rise is the
//!   consequence of where the plan already put the two places. §9 records it.
//! * The plan's `lighting` is [`crate::stages::AreaLighting`], the engine's
//!   existing "which fixture, to what light level" object, not a twin of it.
//!
//! # No opt-out exists
//!
//! Not one check here has an acknowledgement, an override or an exemption
//! field. That is deliberate and it is the cheapest possible answer to
//! `CLAUDE.md`'s question of every escape hatch — *could the defect this hatch
//! exists to catch supply the hatch's own proof obligation?* — because a hatch
//! that does not exist cannot be supplied.
//!
//! Determinism (ADR-0006): every set and map is a `BTreeSet`/`BTreeMap` and
//! every walk is over a slice in document order.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::ids::{DatumId, EdgeId, FactId, NodeId, ViewId, VolumeId};
use crate::layout::{Edge, LayoutGraphContent, StationKind};
use crate::metrics::{
    MAX_JUMP_RISE_16, MetricKind, MetricValue, Metrics, Reads, SizeClass, WayClass,
    passable_clearance_cells, passable_width_cells,
};
use crate::stages::AreaLighting;

/// `DW0824`: the graph and the plan do not agree exactly.
pub const DW_PLAN_AGREEMENT: DwCode = DwCode::every_version("DW0824", ExitTier::Build);

/// `DW0825`: a box leaves the kit grid.
pub const DW_BOX_OFF_GRID: DwCode = DwCode::every_version("DW0825", ExitTier::Build);

/// `DW0826`: a box leaves the region.
pub const DW_BOX_LEAVES_REGION: DwCode = DwCode::every_version("DW0826", ExitTier::Build);

/// `DW0827`: two boxes overlap.
pub const DW_BOXES_OVERLAP: DwCode = DwCode::every_version("DW0827", ExitTier::Build);

/// `DW0828`: a seam is not on a shared face.
pub const DW_SEAM_NOT_SHARED: DwCode = DwCode::every_version("DW0828", ExitTier::Build);

/// **How many cells stand between two connected boxes: the wall they share.**
///
/// The one number every geometric check in this stage is written against, and
/// the one an author has to know before the first box goes down. A box is the
/// **play space** of a place — the cells a body can be in — so the shell is not
/// inside it: it stands in this gap, and two places that connect leave exactly
/// this much room for it. Boxes placed flush have no wall to cut a seam through
/// and `DW0828` refuses them.
///
/// It is a constant rather than a literal because the authoring documents state
/// it: [`PlanBox`]'s schema description carries this value, and
/// `crates/dsl/tests/v14_site_plan.rs` asserts the exported description against
/// this constant, so the rule a person reads and the rule the checks enforce
/// cannot drift apart.
pub const SHARED_FACE_GAP_CELLS: i64 = 1;

/// `DW0829`: a seam's opening is not a standard, or does not fit.
pub const DW_SEAM_OPENING: DwCode = DwCode::every_version("DW0829", ExitTier::Build);

/// `DW0830`: a stair seam cannot be built at standard pitch.
pub const DW_STAIR_PITCH: DwCode = DwCode::every_version("DW0830", ExitTier::Build);

/// `DW0831`: a drop seam falls outside the drop policy.
pub const DW_DROP_POLICY: DwCode = DwCode::every_version("DW0831", ExitTier::Build);

/// `DW0876`: a seam does not declare a connection this engine builds
/// (spec-0053 §6).
///
/// **One code, four shapes of one claim** — the claim being that this seam
/// states a crossing the derivation can build and the observer can measure:
///
/// 1. it declares neither an `opening` nor a `contact`, or both;
/// 2. its contact's span leaves the shared face `DW0828` established;
/// 3. its contact's span is not **wider than the broadest standard opening**;
/// 4. it is a contact on a `stair`, `barred` or `vision` connection.
///
/// They are one code rather than four because the author's next action is the
/// same in every case — say which kind of hand-off this is and give it a shape
/// the engine has — and because a seam exhibiting one of them has no crossing
/// for any rule below to judge. It is the shape `DW0830` already carries for a
/// stair ("three shapes of one claim") and `DW0829` for an opening ("two halves
/// of one claim that the opening is usable").
///
/// Shape 3 is the floor that keeps the whole surface honest, and it is
/// **structural rather than seeded**: it is derived from the standard opening
/// set, so anything at or under it COULD have been a portal, and a doorway
/// declared a contact to dodge the standard set is refused by its own width.
/// That is the property `CLAUDE.md` demands of an escape hatch — the defect this
/// exists to catch is incapable of supplying the hatch's proof obligation.
///
/// `every_version` for the reason its siblings are: the rule judges what the
/// document SAYS, and a plan below [`crate::WAY_AND_CONTACT_SINCE`] has no
/// `contact` to judge — the per-stage fence has already refused one.
pub const DW_CONTACT: DwCode = DwCode::every_version("DW0876", ExitTier::Build);

/// `DW0832`: a box violates its node's size class.
pub const DW_SIZE_CLASS: DwCode = DwCode::every_version("DW0832", ExitTier::Build);

/// `DW0833`: a brief identity does not hold.
pub const DW_IDENTITY_FALSE: DwCode = DwCode::every_version("DW0833", ExitTier::Build);

/// `DW0834`: the identity gate binds nothing. Warning — see [`identities`].
pub const DW_IDENTITY_EMPTY: DwCode = DwCode::every_version("DW0834", ExitTier::Build);

/// `DW0835`: a whole-owned volume enters a box.
pub const DW_VOLUME_IN_BOX: DwCode = DwCode::every_version("DW0835", ExitTier::Build);

/// `DW0839`: two placement authorities in one campaign.
///
/// `every_version` for the reason its siblings are: the rule judges what the
/// campaign SAYS — that a `site-plan.json` and a non-empty `areas[]` are both
/// present — and a document below `dsl_version` 0.14.0 has no site plan to be
/// the second authority, so there is no earlier campaign the rule could reach.
pub const DW_TWO_AUTHORITIES: DwCode = DwCode::every_version("DW0839", ExitTier::Build);

// ---------------------------------------------------------------------------
// The vocabulary the derivation synthesizes (spec-0049 §5.2)
// ---------------------------------------------------------------------------

/// **The one area a site-plan campaign has.**
///
/// A campaign places its pieces either with `areas[]` or with a site plan, never
/// both (`DW0839`), so a site-plan campaign has exactly one place for an NPC to
/// stand in and one area for a quest to belong to. The name is fixed rather than
/// authored because there is nothing to choose: the site plan is the whole map,
/// and a second name for it would be a second way to spell one thing.
pub const SITE_AREA: &str = "area/site";

/// The anchor name the campaign's **entry** stands under.
///
/// A *name*, and only a name: what makes this anchor the entry is the declared
/// entry **role** (spec-0046) the derivation gives it, which is the one thing
/// the compiler's resolution consults. The spelling survives because a
/// site-plan campaign's quests and NPCs may address the entry cell like any
/// other anchor, and `spawn` is the word the rest of the vocabulary already
/// uses; nothing resolves through it.
pub const ENTRY_ANCHOR: &str = "spawn";

/// The anchor at a place's floor centre — where quests, NPCs and waves in a
/// site-plan campaign stand.
///
/// `node/near-hall` becomes `anchor/node-near-hall`, and the reshaping is not
/// cosmetic: a campaign reaches an anchor through [`crate::ids::AnchorId`],
/// which is `anchor/<kebab>`, so `node/<id>` — spec-0049 §5.2's spelling — is
/// not a name any document could write. The three families (`node-`, `seam-`,
/// `unlock-`) are disjoint by their first segment, so no two synthesized
/// anchors can collide however the graph is named.
#[must_use]
pub fn node_anchor(node: &NodeId) -> String {
    format!("anchor/node-{}", slug(node.0.as_str()))
}

/// The gate region over a `barred` seam's opening — what an `open-gate` or a
/// `shortcut` names.
#[must_use]
pub fn seam_anchor(edge: &EdgeId) -> String {
    format!("anchor/seam-{}", slug(edge.0.as_str()))
}

/// The anchor on the openable side of a one-sided `barred` seam, where a
/// shortcut's far-side affordance stands.
#[must_use]
pub fn seam_unlock_anchor(edge: &EdgeId) -> String {
    format!("anchor/unlock-{}", slug(edge.0.as_str()))
}

/// The part of an id after its kind prefix.
fn slug(id: &str) -> &str {
    id.split_once('/').map_or(id, |(_, rest)| rest)
}

/// **What a sealed `barred` seam stands in until content opens it.**
///
/// One definition, here rather than in the derivation that lays it, for the same
/// structural reason the metrics table owns the nav model's constants: two
/// parties need this block and they need the *same* one. The derivation writes it
/// into the gate region and declares it on the synthesized gate anchor; every
/// verb that needs a gate's fill block — `close-gate`, a `shortcut`'s clear, a
/// `timed-gate`'s clock — asks [`synthesized_gate_block`] whether this campaign
/// declares one. A copy in each place would be an agreement rather than a fact.
pub const SEAM_BAR: &str = "minecraft:iron_bars";

/// The fill block a **synthesized** gate anchor declares, or `None` when `anchor`
/// is not one of this campaign's derived seam gates.
///
/// This exists because `DW0343`'s question — *can the compiler fill and clear
/// this gate?* — used to be answered by one instrument only, the prefab registry,
/// and a derived world has no prefab. The answer came back honest and about a
/// smaller world than the campaign has: a `shortcut` naming the very
/// `anchor/seam-<edge>` the derivation seals with [`SEAM_BAR`] was refused for
/// declaring no fill block, while the block sat in the derivation's own
/// `AnchorSpec::Gate`. Nothing was red, because the check was refusing content.
///
/// `None` for a campaign with no site plan, and for any anchor the derivation
/// does not synthesize — those are the prefab registry's to answer for, and this
/// function never overrides it.
#[must_use]
pub fn synthesized_gate_block(c: &Campaign, anchor: &str) -> Option<&'static str> {
    // Asks the ONE kind authority rather than re-walking the edges, and that is
    // the whole repair: this used to enumerate `Edge::Barred` alone, so a
    // `close-gate`, `shortcut` or `timed-gate` naming a **gate station**
    // (spec-0052) would have been refused by `DW0343` for declaring no fill
    // block — a refusal whose message says "declare the gate on an anchor of a
    // piece an area binds", which a site-plan campaign cannot do at all
    // (`DW0839` refuses a campaign that carries both `areas[]` and a plan).
    // A narrow binding on the general mechanism, reading as a missing feature.
    matches!(
        synthesized_anchor_kinds(c).get(anchor),
        Some(StationKind::Gate)
    )
    .then_some(SEAM_BAR)
}

/// **Every anchor a site-plan campaign's blockout provides, and what SHAPE each
/// one is** — the single authority behind [`synthesized_anchors`].
///
/// The kind travels with the name because a kind is a property of the **anchor**,
/// not of the verb that first needed one: `synthesized_gate_block` needed to know
/// whether a name was a gate and answered by privately re-walking the edges, and
/// a second consumer wanting the same fact would have re-walked them again. One
/// function answers it, and everything that needs a shape asks here.
///
/// The mapping, and it is total:
///
/// * [`ENTRY_ANCHOR`] and every `anchor/node-…` — [`StationKind::Point`], the
///   floor centre a body stands on.
/// * every `anchor/unlock-…` — [`StationKind::Point`], where the shortcut's
///   far-side affordance stands.
/// * every `anchor/seam-…` — [`StationKind::Gate`], the region the derivation
///   fills with [`SEAM_BAR`] and content opens.
/// * every declared station — the kind its node declared (spec-0052 §3).
///
/// Empty for a campaign with no site plan — it has prefabs instead, and their
/// metadata is the authority.
#[must_use]
pub fn synthesized_anchor_kinds(c: &Campaign) -> BTreeMap<String, StationKind> {
    let mut out: BTreeMap<String, StationKind> = BTreeMap::new();
    if c.site_plan.is_none() {
        return out;
    }
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return out; // `DW0824` refused the plan; there is nothing to name.
    };
    out.insert(ENTRY_ANCHOR.to_string(), StationKind::Point);
    for n in &graph.nodes {
        out.insert(node_anchor(&n.id), StationKind::Point);
        // A station whose name collides with a synthesized one is `DW0869`, and
        // one that collides with another station is `DW0870`; both are errors,
        // so this insert never silently reinterprets a name a campaign builds
        // with. Inserting anyway keeps the set EXACT for the refused document
        // too, which is what lets the kind check name the declared kind rather
        // than a shape the author did not write.
        for s in &n.stations {
            out.insert(s.anchor.as_str().to_string(), s.kind);
        }
    }
    for e in &graph.edges {
        let Edge::Barred { id, opens_from, .. } = e else {
            continue;
        };
        out.insert(seam_anchor(id), StationKind::Gate);
        if !matches!(opens_from, crate::layout::OpensFrom::Either) {
            out.insert(seam_unlock_anchor(id), StationKind::Point);
        }
    }
    out
}

/// **Every anchor a site-plan campaign's blockout provides**, derived from the
/// documents alone.
///
/// One authority, and that is why it lives here rather than in the derivation
/// that places them: validation resolves a campaign's anchor references against
/// this set, the derivation creates exactly these anchors, and a name that
/// validated could therefore never fail to exist at build time. Two functions
/// agreeing about the spelling is the drift this one removes.
///
/// Empty for a campaign with no site plan — it has prefabs instead, and their
/// metadata is the authority.
#[must_use]
pub fn synthesized_anchors(c: &Campaign) -> BTreeSet<String> {
    // The names ARE the keys of the kind table, taken rather than re-derived:
    // two functions walking the graph and agreeing about the spelling is the
    // exact drift this module's note exists to remove, and a station made the
    // walk long enough that a second copy would eventually diverge.
    synthesized_anchor_kinds(c).into_keys().collect()
}

/// **The synthesized names one PLACE owes** (spec-0050 §6) — the subset of
/// [`synthesized_anchors`] whose bearer is this box.
///
/// Its own `anchor/node-…`; [`ENTRY_ANCHOR`] when it is the entry node; each
/// `anchor/unlock-…` whose `opens_from` side it is — the side the derivation
/// stands that affordance in. A gate region (`anchor/seam-…`) is **never** owed:
/// it is whole fabric, standing in a party plane the piece does not own.
///
/// Here rather than in `crate::detailplan` because the answer is a fact about
/// the graph, and [`synthesized_anchors`] is the one authority for what a
/// site-plan campaign provides — a second module deciding which names belong to
/// which place is exactly the two-functions-agreeing-about-spelling drift that
/// note exists to remove.
///
/// `crates/compiler/tests/blockout.rs`'s
/// `the_owed_anchors_partition_the_synthesized_set` proves the two PARTITION
/// rather than merely overlap: every synthesized name is owed by exactly one
/// place or is a gate region no place owes. A name in neither would be one a
/// campaign resolves and no piece is ever asked for; a name in both would be two
/// pieces claiming one anchor.
///
/// Empty for a campaign with no site plan, and for a node the graph does not
/// have.
#[must_use]
pub fn owed_anchors(c: &Campaign, node: &NodeId) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    if c.site_plan.is_none() {
        return out;
    }
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return out; // `DW0824` refused the plan; there is nothing to name.
    };
    let Some(n) = graph.nodes.iter().find(|n| &n.id == node) else {
        return out; // `DW0842` names a row whose place the graph does not have.
    };
    out.insert(node_anchor(node));
    if &graph.entry == node {
        out.insert(ENTRY_ANCHOR.to_string());
    }
    // Every station of this node (spec-0052 §6). The owed set grows **upstream**,
    // and that one widening is what carries the whole binding chain: the
    // `detail-plan` `anchors` map must now bind each of them, and it still
    // refuses every key outside this set, so a binding cannot invent vocabulary
    // and a typo cannot pass as intent.
    for s in &n.stations {
        out.insert(s.anchor.as_str().to_string());
    }
    for e in &graph.edges {
        let Edge::Barred { id, opens_from, .. } = e else {
            continue;
        };
        let side = match opens_from {
            crate::layout::OpensFrom::A => e.a(),
            crate::layout::OpensFrom::B => e.b(),
            crate::layout::OpensFrom::Either => continue,
        };
        if side == node {
            out.insert(seam_unlock_anchor(id));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The document (spec-0049 §4.1)
// ---------------------------------------------------------------------------

/// The `site-plan` stage document's payload: the geometric embedding of the
/// layout graph, and the whole map's design of record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SitePlanContent {
    /// **The whole map's one region, in world coordinates.**
    ///
    /// Required, with no way to omit it and no way to derive it: the schema has
    /// no "compute this from the boxes" spelling, so extent-flows-up is
    /// unrepresentable rather than merely forbidden. The number comes from the
    /// geometry brief and the identities hold the plan to it.
    ///
    /// The water plane is deliberately **not** site-plan surface: `horizon:
    /// ocean` in the stage-1 world document already fixes sea level, and the
    /// plan reads that single authority rather than restating it.
    pub region: WorldBox,
    /// Named ground planes the boxes stand on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datums: Vec<Datum>,
    /// **Exactly one box per graph node** (`DW0824`).
    pub boxes: Vec<PlanBox>,
    /// **Exactly one seam per traversal edge** (`DW0824`). A `vision` edge
    /// carries a [`Sightline`] instead — see [`Sightline`] for why.
    pub seams: Vec<Seam>,
    /// The mass the WHOLE owns: the mountain a cave system is inside, the ground
    /// under a village, the sky a silhouette needs kept empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<Volume>,
    /// The guarded comparisons binding this plan to the geometry brief's facts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identities: Vec<Identity>,
    /// One per `vision` edge (`DW0824`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sightlines: Vec<Sightline>,
    /// The named exterior vantages the walk judges the silhouette from. Optional;
    /// a plan with zero views has that zero stated in the binding line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<View>,
    /// One lighting setting applied to every enclosed box, so a blockout
    /// interior is walkable at night without per-box surface.
    ///
    /// **The engine's existing object**, not a twin of it: [`AreaLighting`] is
    /// already "which fixture, to what light level", and the relight pass that
    /// consumes it is the same pass either way. A second two-field struct here
    /// would be the private-copy defect `CLAUDE.md` names, and it would fork the
    /// range check the moment one of them grew a third field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<AreaLighting>,
}

/// A box of world cells: its low corner and its extent, in blocks.
///
/// The cells are `min[i] ..= min[i] + extent[i] - 1` on each axis. The extent is
/// [`NonZeroU32`] rather than a `u32` a check refuses: a zero-extent region is
/// not a small region, it is a document that does not describe a volume, and the
/// schema says so (`minimum: 1`) so the parse refuses it as an ordinary
/// `DW0100`. One fewer diagnostic to write and one fewer to forget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldBox {
    /// Low corner `[x, y, z]`, in world coordinates.
    pub min: [i64; 3],
    /// Extent `[dx, dy, dz]`, in blocks.
    pub extent: [NonZeroU32; 3],
}

impl WorldBox {
    /// The high corner (inclusive).
    #[must_use]
    pub fn max(&self) -> [i64; 3] {
        [
            self.min[0] + i64::from(self.extent[0].get()) - 1,
            self.min[1] + i64::from(self.extent[1].get()) - 1,
            self.min[2] + i64::from(self.extent[2].get()) - 1,
        ]
    }
}

/// A named ground plane a box's floor sits on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Datum {
    /// Datum id (`datum/<kebab>`), unique within the plan.
    pub id: DatumId,
    /// The world `y` of the walk plane.
    pub y: i64,
    /// What this plane is, for a reader of the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Where a place's walk plane is.
///
/// Two spellings of one number, and the second is not redundant: a plane several
/// places stand on is named once as a [`Datum`] and moved once, while a place
/// that stands alone at its own height has no plane to name. An `identities[]`
/// entry can only bind to a *named* one, which is the pressure toward naming.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Floor {
    /// A plane the plan names (`DW0112` if the plan declares no such datum).
    Datum(DatumId),
    /// A world `y` this place alone stands at.
    Y(i64),
}

/// What closes a place overhead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Ceiling {
    /// Cells of headroom over the walk plane. A body's feet are at the floor and
    /// the ceiling course sits at `floor + clearance`.
    Clearance(NonZeroU32),
    /// A sky-open place — a courtyard, a shore, a summit.
    ///
    /// The plan claims the ground and its size class's own minimum clearance,
    /// and **nothing above that**: an open place is precisely one that makes no
    /// claim on the air over it, so a `clearance` volume above a courtyard is
    /// the whole reserving sky rather than two authorities over one cell.
    Open,
}

/// One place, embedded: a footprint standing on a plane.
///
/// **A box is a plan, not a prism.** Its `min`/`extent` are the two horizontal
/// axes and its vertical position is [`PlanBox::floor`] — one authority for the
/// plane, where a `y` inside `min` beside a declared floor would have been two
/// numbers with no rule about which the derivation believes.
///
/// **A box is the PLAY SPACE, and connected boxes are separated by exactly one
/// cell.** `extent` is the interior a body can stand in; the shell the blockout
/// derivation builds is not inside it. That shell stands in the one-cell gap
/// between two neighbours, and on the course under the floor and over the
/// ceiling. So two places that connect are placed one cell apart on the face
/// they share — that cell is the wall they have in common, written once — and
/// two boxes placed flush have no wall for a seam to be cut through, which
/// `DW0828` refuses. Worked: a box at `min: [4, 4]` with `extent: [4, 4]`
/// occupies x 4..7, so its eastern neighbour's `min` x is 9, never 8.
///
/// Two consequences follow, and they are what make the checks say what they look
/// like they say: the size-class ladder judges `extent` directly (`DW0832`),
/// its smallest rung `4 × 4` being exactly one kit quantum; and a plan never
/// states a wall's thickness anywhere, because the gap is where the wall is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanBox {
    /// The graph node this box embeds.
    ///
    /// **The ordering tooth, at the type level**: there is no way to write a box
    /// that does not name a place, so a site plan cannot describe a space the
    /// layout graph has not declared (spec-0049 §7.1).
    pub node: NodeId,
    /// Low corner `[x, z]`, in world coordinates. Two horizontal numbers, never
    /// three — the vertical position is `floor`.
    pub min: [i64; 2],
    /// Interior footprint `[dx, dz]`, in blocks, on the kit grid (`DW0825`).
    /// Two horizontal numbers, never three — the vertical size is `ceiling`.
    ///
    /// **This is play space, not the building.** The box covers `min` to
    /// `min + extent - 1` inclusive, and the walls stand outside it, in the
    /// one-cell gap that separates connected places (`DW0828`).
    pub extent: [NonZeroU32; 2],
    /// The walk plane.
    pub floor: Floor,
    /// What closes it overhead.
    pub ceiling: Ceiling,
}

/// Which side of a box a seam sits on.
///
/// The engine's existing face vocabulary — the same six names a prefab's face
/// contract writes (`compiler::faces`), so a reader who knows one knows the
/// other and no translation table exists to disagree with itself.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Face {
    /// `+x`.
    East,
    /// `-x`.
    West,
    /// `+y`.
    Up,
    /// `-y`.
    Down,
    /// `+z`.
    South,
    /// `-z`.
    North,
}

impl Face {
    /// The unit vector this face points along.
    #[must_use]
    pub fn vector(self) -> [i64; 3] {
        match self {
            Face::East => [1, 0, 0],
            Face::West => [-1, 0, 0],
            Face::Up => [0, 1, 0],
            Face::Down => [0, -1, 0],
            Face::South => [0, 0, 1],
            Face::North => [0, 0, -1],
        }
    }

    /// True for a face whose plane is horizontal (a floor or a ceiling).
    #[must_use]
    pub fn is_horizontal_plane(self) -> bool {
        matches!(self, Face::Up | Face::Down)
    }

    /// The name a refusal prints.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Face::East => "east",
            Face::West => "west",
            Face::Up => "up",
            Face::Down => "down",
            Face::South => "south",
            Face::North => "north",
        }
    }
}

/// One traversal edge, allocated: an opening on a face the two boxes share.
///
/// The seam carries **no rise**. A rise is `floor(b) − floor(a)`, which the plan
/// has already stated by putting the two places where it put them; a second
/// declaration of it could only ever agree or be a refusal teaching nothing the
/// datums did not already say. `DW0830` and `DW0831` judge the derived number,
/// and the stage-5 observer of the built bytes judges the realized one against
/// the same derivation rather than against an author's copy of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Seam {
    /// The graph edge this seam allocates.
    pub edge: EdgeId,
    /// Which face **of the edge's `a` box** the seam sits on. The `b` box is the
    /// neighbour across it (`DW0828`).
    pub face: Face,
    /// The opening's low corner, in world coordinates, on the face's own two
    /// in-plane axes:
    ///
    /// * on a vertical face (`east`/`west`/`north`/`south`) — `[along, y]`,
    ///   where `along` is the horizontal axis in the plane (`z` for east/west,
    ///   `x` for north/south) and `y` is the **sill**. The opening's `width`
    ///   runs along the first, its `height` upward from the second.
    /// * on a horizontal face (`up`/`down`) — `[x, z]`, `width` along `x` and
    ///   `height` along `z`.
    pub at: [i64; 2],
    /// **A PORTAL**: a named opening from the metrics table's standard set
    /// (`DW0812` on a name the table does not define, `DW0829` on one that does
    /// not fit). A body crosses at exactly the cells `at` and this standard
    /// allocate.
    ///
    /// **Exactly one of this and [`Seam::contact`]** (`DW0876`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<String>,
    /// **A CONTACT**: the two places simply meet along a front, rather than
    /// through a doorway (spec-0053 §4).
    ///
    /// **Exactly one of this and [`Seam::opening`]** (`DW0876`).
    ///
    /// The width of a front where two places meet is a fact of those two boxes'
    /// shared face — per-campaign geometry, continuous — so it is never a named
    /// standard. A table that enumerated it would gain a new entry per campaign,
    /// which is the size ladder's own failure mode reproduced in the opening
    /// set: an `opening.gate-front` of 21×4 is content wearing a standard's
    /// clothes (spec-0053 §7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<Contact>,
    /// Which of the edge's two boxes hosts the stair massing. Required on a
    /// `stair` edge and refused on any other (`DW0830`, `DW0824`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stair_in: Option<NodeId>,
}

/// A mass the whole itself owns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    /// Volume id (`volume/<kebab>`), unique within the plan.
    pub id: VolumeId,
    /// The cells it covers.
    pub region: WorldBox,
    /// What the whole is doing with them.
    pub role: VolumeRole,
    /// What this mass is, for a reader of the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What a whole-owned volume is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum VolumeRole {
    /// Solid mass the places are cut into — the mountain around a cave system.
    Massif,
    /// The ground the places stand on.
    Ground,
    /// Air the whole keeps empty — the sky a silhouette needs, the drop a
    /// vista looks over.
    Clearance,
}

/// One guarded comparison binding the plan to a fact of the written brief.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The brief fact this holds the plan to (`DW0112` if the brief has no such
    /// fact).
    pub fact: FactId,
    /// What is measured off the plan.
    pub measure: Measure,
    /// How the measurement must stand to the fact's value.
    pub cmp: Cmp,
}

/// What an identity measures off the plan.
///
/// A small **fixed** vocabulary, spelled as a tagged union rather than as the
/// spec's `box(<node>).extent.x` string. The vocabulary is exactly the spec's
/// five; what changes is that a measure is parsed by serde instead of by a
/// grammar this module would have had to write, own and document — so an
/// unknown measure is an ordinary `DW0100`, a node it names is checked like
/// every other reference, and the growth the spec's marked judgement predicts is
/// a variant rather than a second escaping rule. §9 records the departure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "of", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Measure {
    /// The whole region's extent on one axis, in blocks.
    RegionExtent {
        /// Which axis.
        axis: Axis,
    },
    /// One place's footprint on one horizontal axis, in blocks.
    BoxExtent {
        /// The place.
        node: NodeId,
        /// Which horizontal axis.
        axis: PlanAxis,
    },
    /// One place's headroom over its walk plane, in blocks. A sky-open place
    /// measures its size class's own minimum clearance — the least air the
    /// ladder says such a place has.
    BoxHeight {
        /// The place.
        node: NodeId,
    },
    /// The horizontal distance between two places' footprint centres, in blocks.
    /// Euclidean on `x`/`z`; the standoff a brief states between two things.
    DistanceXz {
        /// One place.
        from: NodeId,
        /// The other.
        to: NodeId,
    },
    /// A named ground plane's world `y`.
    DatumY {
        /// The plane.
        datum: DatumId,
    },
}

/// One of the three world axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Axis {
    /// East–west.
    X,
    /// Up–down.
    Y,
    /// North–south.
    Z,
}

/// One of the two horizontal axes.
///
/// A separate type from [`Axis`] rather than a `y` some check refuses: a box is
/// a footprint, so its extent has no `y` to ask about, and an unrepresentable
/// state needs no diagnostic to police it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PlanAxis {
    /// East–west.
    X,
    /// North–south.
    Z,
}

/// How a measurement must stand to its fact's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Cmp {
    /// Exactly.
    Eq,
    /// Strictly under.
    Lt,
    /// At most.
    Le,
    /// Strictly over.
    Gt,
    /// At least.
    Ge,
}

impl Cmp {
    fn holds(self, measured: f64, fact: f64) -> bool {
        match self {
            Cmp::Eq => (measured - fact).abs() < 1e-9,
            Cmp::Lt => measured < fact,
            Cmp::Le => measured <= fact,
            Cmp::Gt => measured > fact,
            Cmp::Ge => measured >= fact,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Cmp::Eq => "exactly",
            Cmp::Lt => "under",
            Cmp::Le => "at most",
            Cmp::Gt => "over",
            Cmp::Ge => "at least",
        }
    }
}

/// A `vision` edge, embedded: the segment the stage-5 battery walks.
///
/// A vision edge gets a sightline rather than a seam because a vista's two ends
/// are routinely not adjacent — a bell tower seen from a shore shares no face
/// with it — so the seam construct cannot state the one thing a vision edge
/// asserts. A window between neighbours is simply a short sightline
/// (spec-0049 §4.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sightline {
    /// The `vision` edge this embeds.
    pub edge: EdgeId,
    /// The eye end, in world coordinates — inside the edge's `a` box
    /// (`DW0824`).
    pub from: [i64; 3],
    /// The seen end — inside the edge's `b` box (`DW0824`).
    pub to: [i64; 3],
}

/// A named exterior vantage the walk judges the silhouette from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct View {
    /// View id (`view/<kebab>`), unique within the plan.
    pub id: ViewId,
    /// Where the eye stands, in world coordinates.
    pub eye: [i64; 3],
    /// What it looks at.
    pub look_at: [i64; 3],
    /// What this view is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

// ---------------------------------------------------------------------------
// Resolution — the plan read once, so no check re-derives a number
// ---------------------------------------------------------------------------

/// One box with everything the checks below need already worked out.
///
/// **The model every geometric check depends on** is stated where an author
/// reads it — on [`PlanBox`], whose schema description carries it — and the
/// number itself is [`SHARED_FACE_GAP_CELLS`]. In short: a box is the **play
/// space** of a place, the shell stands in the one-cell gap between two
/// neighbours, `extent` is therefore the interior footprint the size-class
/// ladder judges directly (`DW0832`), and two connected places sit exactly
/// [`SHARED_FACE_GAP_CELLS`] apart on the face they share (`DW0828`).
#[derive(Debug, Clone)]
struct Placed<'a> {
    index: usize,
    plan: &'a PlanBox,
    /// Footprint, inclusive: `[x0, x1, z0, z1]`.
    foot: [i64; 4],
    /// The walk plane.
    floor: i64,
    /// Cells of headroom over the walk plane, or `None` when the place is
    /// sky-open and its classification did not resolve.
    clearance: Option<u32>,
    /// How the place is classified, when the name resolved.
    class: Option<PlaceClass>,
}

/// **How a place is classified** — the two kinds of standard a box is judged
/// against (spec-0053 §3).
///
/// The classification belongs to the PLACE, so it is one field of two kinds
/// rather than two fields. Written as a second `Option<WayClass>` beside the
/// first, every consumer would have had to remember to look at both, and the
/// one that forgot would silently judge a road against nothing — which is the
/// state this whole surface exists to end, reintroduced one layer down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceClass {
    /// A rung of the size ladder: both horizontal extents bounded.
    Size(SizeClass),
    /// A way: the cross-section bounded, the run free.
    Way(WayClass),
}

impl PlaceClass {
    /// The least interior clearance the class demands.
    ///
    /// The one question both kinds answer identically, which is why a sky-open
    /// box needs no arm: an open place claims exactly its class's minimum
    /// headroom and nothing above it, and that sentence is true of a road as it
    /// is of a hall.
    fn min_clearance(self) -> u32 {
        match self {
            PlaceClass::Size(c) => c.min_clearance,
            PlaceClass::Way(w) => w.min_clearance,
        }
    }
}

impl Placed<'_> {
    fn x0(&self) -> i64 {
        self.foot[0]
    }
    fn x1(&self) -> i64 {
        self.foot[1]
    }
    fn z0(&self) -> i64 {
        self.foot[2]
    }
    fn z1(&self) -> i64 {
        self.foot[3]
    }

    /// The inclusive vertical span of the play space, when it is bounded.
    fn y_span(&self) -> Option<(i64, i64)> {
        let c = i64::from(self.clearance?);
        Some((self.floor, self.floor + c - 1))
    }

    /// The centre of the footprint, in blocks.
    fn centre_xz(&self) -> (f64, f64) {
        (
            (self.x0() as f64 + self.x1() as f64) / 2.0,
            (self.z0() as f64 + self.z1() as f64) / 2.0,
        )
    }
}

// ---------------------------------------------------------------------------
// The resolved plan, in world cells — ONE authority, three readers
// ---------------------------------------------------------------------------

/// One place, resolved into world cells: the play space the plan gives it.
///
/// Public because three readers need the same answer and two of them are in
/// another crate: the stage-4 checks here, the **blockout derivation** that
/// builds the mass, and the **stage-5 battery** that judges the built bytes
/// against the plan. Two of those computing "where is this box" independently is
/// how a builder and its observer come to agree about a world neither of them
/// describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedBox {
    /// The place this embeds.
    pub node: NodeId,
    /// Inclusive footprint `[x0, x1, z0, z1]`.
    pub foot: [i64; 4],
    /// The walk plane's world `y`.
    pub floor: i64,
    /// Cells of headroom over the walk plane.
    pub clearance: u32,
    /// True when the plan declared no ceiling — a courtyard, a shore, a summit.
    /// The place still claims its size class's own minimum headroom (which is
    /// what [`PlacedBox::clearance`] holds); what it makes no claim on is the
    /// air above that.
    pub open: bool,
}

impl PlacedBox {
    /// The play space's inclusive world AABB.
    #[must_use]
    pub fn space(&self) -> ([i64; 3], [i64; 3]) {
        (
            [self.foot[0], self.floor, self.foot[2]],
            [
                self.foot[1],
                self.floor + i64::from(self.clearance) - 1,
                self.foot[3],
            ],
        )
    }

    /// The floor centre — where a body seated in this place stands.
    #[must_use]
    pub fn centre(&self) -> [i64; 3] {
        [
            (self.foot[0] + self.foot[1]) / 2,
            self.floor,
            (self.foot[2] + self.foot[3]) / 2,
        ]
    }
}

/// One connection, resolved into world cells: the wall the two places share and
/// the hole the plan cut in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedSeam {
    /// The connection this allocates.
    pub edge: EdgeId,
    /// Its class, as the graph spells it.
    pub class: &'static str,
    /// The `a` end.
    pub a: NodeId,
    /// The `b` end.
    pub b: NodeId,
    /// Which face **of `a`** the seam sits on.
    pub face: Face,
    /// The axis the shared wall is flat in: 0 = x, 1 = y, 2 = z.
    pub normal_axis: usize,
    /// The wall's coordinate on that axis — one cell thick, so one number.
    pub plane: i64,
    /// The opening's inclusive world AABB (flat in [`Self::normal_axis`]).
    pub opening: ([i64; 3], [i64; 3]),
    /// The whole rectangle the two boxes share on that wall, inclusive.
    pub shared: ([i64; 3], [i64; 3]),
    /// Which kind of connection this seam allocates (spec-0053 §4). A portal's
    /// `opening` is a standard's rectangle; a contact's is its span.
    pub crossing: Crossing,
    /// `floor(b) − floor(a)`, derived — never authored (see [`Seam`]).
    pub rise: i64,
    /// Which place hosts the stair massing, on a `stair`.
    pub stair_in: Option<NodeId>,
}

/// The plan's boxes, resolved by the code the stage-4 checks judge with.
///
/// A box whose floor names an undeclared datum is **absent** — `DW0112` has
/// refused it, and a place with no plane has no cells for any reader to work in.
/// A sky-open box whose size class did not resolve is absent for the same reason
/// (`DW0812` refused the class, so the plan states no headroom for it at all).
#[must_use]
pub fn placed_boxes(c: &Campaign, reads: &mut Reads) -> Vec<PlacedBox> {
    let (Some(plan), Some(graph)) = (
        c.site_plan.as_ref().map(|p| &p.content),
        c.layout_graph.as_ref().map(|g| &g.content),
    ) else {
        return Vec::new();
    };
    let table = Metrics::table();
    let mut sink = Vec::new();
    resolve(plan, graph, &table, reads, &mut sink)
        .into_iter()
        .filter_map(|p| {
            Some(PlacedBox {
                node: p.plan.node.clone(),
                foot: p.foot,
                floor: p.floor,
                clearance: p.clearance?,
                open: matches!(p.plan.ceiling, Ceiling::Open),
            })
        })
        .collect()
}

/// **The one place a seam's crossing rectangle is computed**, for either kind
/// of connection (spec-0053 §4).
///
/// A portal's rectangle is the named standard's `width × height` anchored at
/// `at`. A contact's is its span: `at` plus the declared `extent`, or `at` to
/// the far edge of the shared face when no extent is declared.
///
/// One function rather than one per kind, and one call rather than a copy in
/// each reader, because this rectangle is simultaneously the derivation's carve,
/// `DW0836`'s allocation, `DW0838`'s allocation set and `DW0877`'s span. Two
/// implementations of it would be a plan-time green and a byte-time green about
/// two different rectangles, which is the defect `shared_face` already has one
/// implementation to prevent.
///
/// `None` when the seam names an opening the table does not define — `DW0812`
/// refused it and there is no rectangle to build or measure.
fn crossing_rect(
    s: &Seam,
    face: &SharedFace,
    table: &Metrics,
    reads: &mut Reads,
) -> Option<(Crossing, [i64; 2])> {
    if s.contact.is_some() {
        return Some((Crossing::Contact, contact_extent(s, face)));
    }
    let named = s.opening.as_ref()?;
    let entry = table.resolve(MetricKind::Opening, named).ok()?;
    match entry.value(reads) {
        MetricValue::Opening(o) => {
            Some((Crossing::Portal, [i64::from(o.width), i64::from(o.height)]))
        }
        _ => None,
    }
}

/// **How big a contact's span is** — the one authority, read by
/// [`crossing_rect`] and by the refusal that judges it (`DW0876`).
///
/// A declared `extent` is taken as written. With none declared the span runs
/// from `at` to the far edge of the shared face on both axes, which is how a
/// contact along the whole of a face is spelled. An `at` already past that edge
/// would give a negative extent, so it is clamped to one cell: the rectangle
/// stays well-formed and `DW0876` describes it, rather than the arithmetic
/// producing a rectangle nothing downstream could reason about.
fn contact_extent(s: &Seam, face: &SharedFace) -> [i64; 2] {
    match s.contact.as_ref().and_then(|c| c.extent) {
        Some(e) => [i64::from(e[0].get()), i64::from(e[1].get())],
        None => [
            (face.u.1 - s.at[0] + 1).max(1),
            (face.v.1 - s.at[1] + 1).max(1),
        ],
    }
}

/// The plan's seams, resolved by the code the stage-4 checks judge with.
///
/// A seam whose face the two boxes do not share, or whose opening the table does
/// not define, is **absent**: `DW0828`/`DW0812` refused it, and there is no hole
/// for a reader to build or measure.
#[must_use]
pub fn placed_seams(c: &Campaign, boxes: &[PlacedBox], reads: &mut Reads) -> Vec<PlacedSeam> {
    let (Some(plan), Some(graph)) = (
        c.site_plan.as_ref().map(|p| &p.content),
        c.layout_graph.as_ref().map(|g| &g.content),
    ) else {
        return Vec::new();
    };
    let table = Metrics::table();
    let by_node: BTreeMap<&str, &PlacedBox> =
        boxes.iter().map(|b| (b.node.0.as_str(), b)).collect();
    let edges: BTreeMap<&str, &Edge> = graph.edges.iter().map(|e| (e.id().0.as_str(), e)).collect();
    let mut out = Vec::new();
    for s in &plan.seams {
        let Some(edge) = edges.get(s.edge.0.as_str()) else {
            continue;
        };
        if matches!(edge, Edge::Vision { .. }) {
            continue;
        }
        let (Some(a), Some(b)) = (
            by_node.get(edge.a().0.as_str()).copied(),
            by_node.get(edge.b().0.as_str()).copied(),
        ) else {
            continue;
        };
        let Ok(face) = shared_face_of(a, b, s.face) else {
            continue;
        };
        let Some((crossing, extent)) = crossing_rect(s, &face, &table, reads) else {
            continue;
        };
        let normal_axis = match s.face {
            Face::East | Face::West => 0,
            Face::Up | Face::Down => 1,
            Face::South | Face::North => 2,
        };
        // The face's two in-plane axes, in the order `at` names them.
        let (u_axis, v_axis) = in_plane_axes(s.face);
        let mut lo = [0i64; 3];
        let mut hi = [0i64; 3];
        lo[normal_axis] = face.plane;
        hi[normal_axis] = face.plane;
        lo[u_axis] = s.at[0];
        hi[u_axis] = s.at[0] + extent[0] - 1;
        lo[v_axis] = s.at[1];
        hi[v_axis] = s.at[1] + extent[1] - 1;
        let mut smin = [0i64; 3];
        let mut smax = [0i64; 3];
        smin[normal_axis] = face.plane;
        smax[normal_axis] = face.plane;
        smin[u_axis] = face.u.0;
        smax[u_axis] = face.u.1;
        smin[v_axis] = face.v.0;
        smax[v_axis] = face.v.1;
        out.push(PlacedSeam {
            edge: s.edge.clone(),
            class: edge.class(),
            a: edge.a().clone(),
            b: edge.b().clone(),
            face: s.face,
            normal_axis,
            plane: face.plane,
            opening: (lo, hi),
            shared: (smin, smax),
            crossing,
            rise: b.floor - a.floor,
            stair_in: s.stair_in.clone(),
        });
    }
    out
}

/// **A contact**: the span of a shared face along which two places simply meet
/// (spec-0053 §4).
///
/// # What a contact MEANS
///
/// The boundary is continuous ground. The derivation writes **no wall along the
/// span** — and wall as ever outside it — and crossing is legitimate anywhere
/// along it the step rule admits. It is not a wide door: `DW0829`'s standard-name
/// resolution and sill rule are portal checks and do not apply, because a
/// contact has no opening name to resolve and no single sill. Calling a 55-cell
/// front a door would make every downstream door check wrong.
///
/// # What the author allocates and what the engine measures
///
/// The author allocates **where** the places meet. The engine measures the
/// **crossing profile** from assembled bytes — which columns of the span a body
/// actually crosses under the step rule — and `DW0877` refuses a contact nothing
/// can cross. *"This face is fine"* is never a declaration this engine accepts.
///
/// Seams stay **allocated, never discovered**: the span is the edge's allocation
/// set for `DW0838`, so a crossing outside it is still a refusal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    /// The span, `[u, v]` in cells on the face's own two in-plane axes, anchored
    /// at the seam's `at`.
    ///
    /// **Omitted, the span runs from `at` to the far edge of the shared face on
    /// both axes** — which is how a contact along the whole of a face is
    /// written, by putting `at` at the face's low corner. `DW0828`'s refusal
    /// prints that corner, so the number an author needs is in the message they
    /// would already be reading.
    ///
    /// There is no `width` standard and no `length` here, and both absences are
    /// the design (spec-0053 §7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<[NonZeroU32; 2]>,
}

/// **Which kind of connection a seam allocates** (spec-0053 §4).
///
/// Carried on the resolved seam rather than re-derived from the authored one at
/// each reader, so that the derivation and the byte observer cannot disagree
/// about which kind a seam is — the same reason `shared_face` has one
/// implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Crossing {
    /// A standard opening. Every allocated cell must be passable (`DW0836`).
    Portal,
    /// A front where two places meet. No wall along the span, and **at least
    /// one** passable column of body width somewhere in it (`DW0877`) — not
    /// every cell, because a contact is ground rather than a hole and the
    /// massing standing on it is content.
    Contact,
}

/// The two world axes a face's `at` names, in that order.
fn in_plane_axes(face: Face) -> (usize, usize) {
    match face {
        // `[along, y]` — `z` for east/west, `x` for north/south.
        Face::East | Face::West => (2, 1),
        Face::South | Face::North => (0, 1),
        // `[x, z]`.
        Face::Up | Face::Down => (0, 2),
    }
}

/// A footprint's inclusive span on one WORLD axis (0 = x, 2 = z). Axis 1 has no
/// answer here — a footprint is horizontal — and no caller asks for it.
fn span(foot: [i64; 4], axis: usize) -> (i64, i64) {
    if axis == 0 {
        (foot[0], foot[1])
    } else {
        (foot[2], foot[3])
    }
}

/// Inclusive overlap of two ranges, or `None`.
fn overlap(a: (i64, i64), b: (i64, i64)) -> Option<(i64, i64)> {
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    (lo <= hi).then_some((lo, hi))
}

/// Is `[lo, hi]` inside `[within_lo, within_hi]`?
fn within(r: (i64, i64), w: (i64, i64)) -> bool {
    r.0 >= w.0 && r.1 <= w.1
}

/// The region's inclusive span on one axis.
fn region_span(region: &WorldBox, axis: usize) -> (i64, i64) {
    (region.min[axis], region.max()[axis])
}

// ---------------------------------------------------------------------------
// The binding ledger's site-plan half
// ---------------------------------------------------------------------------

/// What a run's site-plan checks bound to.
///
/// Carried inside [`crate::LayoutBinding`] rather than as a second ledger: the
/// map pipeline's documents are counted by one struct with one constructor, so
/// the number the CLI prints and the number a diagnostic quotes cannot disagree
/// about how many places there are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct PlanBinding {
    /// Places embedded — what `DW0825`, `DW0826` and `DW0832` examine.
    pub boxes: usize,
    /// Unordered box pairs — what `DW0827` examines. Zero at one box, which is
    /// the honest count and not a pass.
    pub box_pairs: usize,
    /// Connections allocated — what `DW0828` and `DW0829` examine.
    pub seams: usize,
    /// Of those, seams whose edge is a `stair` — what `DW0830` examines.
    pub stair_seams: usize,
    /// Of those, seams whose edge is a `drop` — what `DW0831` examines.
    pub drop_seams: usize,
    /// Named ground planes.
    pub datums: usize,
    /// Whole-owned masses — what `DW0835` examines.
    pub volumes: usize,
    /// Guarded comparisons against the brief — what `DW0833` examines.
    pub identities: usize,
    /// Sightlines embedded.
    pub sightlines: usize,
    /// Named exterior vantages the walk judges the silhouette from.
    pub views: usize,
}

impl PlanBinding {
    /// Count what a campaign's site plan offers the checks.
    #[must_use]
    pub fn of(c: &Campaign) -> PlanBinding {
        let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
            return PlanBinding::default();
        };
        let classes: BTreeMap<&str, &'static str> = c
            .layout_graph
            .as_ref()
            .map(|g| {
                g.content
                    .edges
                    .iter()
                    .map(|e| (e.id().0.as_str(), e.class()))
                    .collect()
            })
            .unwrap_or_default();
        let n = plan.boxes.len();
        PlanBinding {
            boxes: n,
            box_pairs: n * n.saturating_sub(1) / 2,
            seams: plan.seams.len(),
            stair_seams: plan
                .seams
                .iter()
                .filter(|s| classes.get(s.edge.0.as_str()) == Some(&"stair"))
                .count(),
            drop_seams: plan
                .seams
                .iter()
                .filter(|s| classes.get(s.edge.0.as_str()) == Some(&"drop"))
                .count(),
            datums: plan.datums.len(),
            volumes: plan.volumes.len(),
            identities: plan.identities.len(),
            sightlines: plan.sightlines.len(),
            views: plan.views.len(),
        }
    }

    /// The site-plan half of the binding line.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "site-plan binding: {b} box(es) ({p} pair(s) compared), {s} seam(s) ({st} stair, \
             {sd} drop), {d} datum(s), {v} whole-owned volume(s), {i} identity(ies), \
             {sl} sightline(s), {w} view(s).",
            b = self.boxes,
            p = self.box_pairs,
            s = self.seams,
            st = self.stair_seams,
            sd = self.drop_seams,
            d = self.datums,
            v = self.volumes,
            i = self.identities,
            sl = self.sightlines,
            w = self.views,
        )
    }
}

// ---------------------------------------------------------------------------
// Validation (spec-0049 §4.3) — every rule, all upstream of any geometry
// ---------------------------------------------------------------------------

/// Every check the site plan owes, at **validation** tier (exit 1).
///
/// # What invokes this, and what happens without it
///
/// [`crate::validate::validate_campaign_with`], whenever the campaign directory
/// holds a `site-plan.json` — the same event-bound shape stages 2, 3 and 7 use.
/// That function is what **every** `delvec` subcommand's validation stage calls,
/// so there is no path from a campaign directory to a verdict, a world or a
/// datapack that goes round it: `validate`, `analyze` and `build` all enter
/// through it, and a caller who somehow skipped it would have no `Campaign` to
/// hand anything downstream. There is no flag to pass, no step in a document to
/// remember, and no second entry point.
///
/// # Why every rule is here and none is at analysis tier
///
/// Round 2 put the layout graph's *reachability* proofs at analysis tier because
/// reachability is a question about a whole graph, the tier its quest-graph
/// siblings answer at. Nothing here is that question. Every rule below is a
/// property of the document in front of it — does this box fit, do these two
/// boxes touch, does this number match the brief's — so all of it is validation,
/// and a plan that is wrong is wrong before anything is analyzed.
///
/// # What is deliberately NOT here, and where it went
///
/// Three obligations of this stage are only decidable once the blockout exists,
/// and each is named rather than approximated:
///
/// * whether a built seam is the opening the plan allocated, whether every node's
///   floor is reached, and whether any crossing was *discovered* outside a seam;
/// * `DW0833`'s second call site, the identities recomputed from assembled bytes;
/// * whether a declared sightline is unobstructed.
///
/// All three read blocks. Writing a version of them here that read the plan
/// instead would be the derivation's arithmetic replayed against itself — the
/// opposite of an independent observer — so they belong to the round that builds
/// the blockout, and this module states the plan-side half they will be checked
/// against.
pub fn check(c: &Campaign, reads: &mut Reads, d: &mut Vec<Diagnostic>) {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return;
    };
    let table = Metrics::table();

    // Plan-internal wellformedness first: the ids every rule below quotes.
    ids(plan, d);
    one_authority(c, d);

    // ------------------------------------------------------------- the tooth
    // A site plan validates ONLY against a layout graph and a geometry brief.
    // Both are refused by name, and the graph's absence returns: every rule
    // below reads node ids, and without a graph each would be answering about a
    // place nothing declared.
    let brief_missing = c.geometry_brief.is_none();
    if brief_missing {
        d.push(Diagnostic::error(
            DW_PLAN_AGREEMENT,
            "site-plan",
            "",
            "this campaign carries a site plan and no `geometry-brief.json`. The plan is the \
             embedding of a design, and the brief is where that design's numbers are written \
             down; with no brief there is nothing for `identities[]` to hold the map to, and \
             the region's extent is a number with no author. Write the brief first — a plan \
             cannot reach green ahead of it."
                .to_string(),
        ));
    }
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        d.push(Diagnostic::error(
            DW_PLAN_AGREEMENT,
            "site-plan",
            "",
            "this campaign carries a site plan and no `layout-graph.json`. A site plan is the \
             geometric embedding OF a layout graph: every box names a place and every seam \
             names a connection, so with no graph there is nothing being embedded and every \
             name in this document resolves to nothing. Author the graph first — that ordering \
             is what this refusal exists to make uncompilable rather than merely advised."
                .to_string(),
        ));
        return;
    };

    openers(c, graph, d);
    let placed = resolve(plan, graph, &table, reads, d);
    agreement(plan, graph, &placed, d);
    grid(&placed, &table, reads, d);
    region(plan, &placed, d);
    disjoint(&placed, d);
    // The SITE PLAN stage's own declared version — what the contact fence is
    // judged against, exactly as the graph stage's is what the way fence reads.
    let version = c.site_plan.as_ref().map_or("", |p| p.dsl_version.as_str());
    seams(plan, graph, &placed, &table, version, reads, d);
    size_classes(&placed, d);
    volumes_outside_boxes(plan, &placed, d);
    identities(c, plan, &placed, d);
    lighting(plan, d);
}

/// `DW0839`: a campaign has ONE placement authority.
///
/// `areas[]` places pieces on the fixed stride; the site plan places the whole
/// map in its own region. A world carrying both has two owners for one question
/// and no rule to pick between them, so the answer is not to arbitrate but to
/// refuse. Both surfaces stay legal at 0.14.0 — one per campaign.
fn one_authority(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let n = c.world.content.areas.len();
    if n == 0 {
        return;
    }
    d.push(Diagnostic::error(
        DW_TWO_AUTHORITIES,
        "world",
        "/content/areas",
        format!(
            "this campaign declares {n} `areas[]` entry(ies) AND a site plan. Those are two \
             placement authorities for one world: `areas[]` seats prefab pieces on the compiler's \
             fixed stride, and the site plan seats the derived blockout inside its own declared \
             `region` — so every question about where something is has two answers and nothing \
             says which. Keep one. A campaign that places pieces keeps `areas[]` and drops \
             `site-plan.json`; a campaign whose map is the site plan declares an empty `areas` \
             list and lets the plan own the space. Both surfaces are legal — what is not legal \
             is one \
             campaign holding both."
        ),
    ));
}

/// `DW0818`'s **byte-side half** of the opener obligation, which round 3 could
/// not write and named as this round's.
///
/// The graph half already stands in [`crate::layout`]: a `barred` edge must
/// declare a `gating` that names a flag some effect really sets or a quest that
/// really exists. That says the way is *meant* to open; it does not say anything
/// in the campaign ever opens **this** way, because at stage 3 the region such
/// an effect would target does not exist yet. It exists here: the derivation
/// synthesizes [`seam_anchor`] over every `barred` seam's opening, so "something
/// opens `seam/<edge>`" is finally a question with a subject.
///
/// Raised under `DW0818` and against the layout graph, because the fault is the
/// graph's claim rather than the plan's geometry — the plan did everything asked
/// of it. Only reachable in a site-plan campaign, which is exactly the campaign
/// in which the seam anchor exists to be named.
fn openers(c: &Campaign, graph: &LayoutGraphContent, d: &mut Vec<Diagnostic>) {
    // Every gate region the campaign opens, however it opens it: an `open-gate`
    // at any nesting depth, or a `shortcut` whose far side lifts the bar.
    let mut opened: BTreeSet<&str> = BTreeSet::new();
    crate::stages::for_each_campaign_effect(c, &mut |_, _, eff| {
        eff.visit_deep(&mut |e| {
            if let Some(a) = e.open_gate_anchor() {
                opened.insert(a.0.as_str());
            }
        });
    });
    for s in &c.quests.content.shortcuts {
        opened.insert(s.gate.0.as_str());
    }

    for (i, e) in graph.edges.iter().enumerate() {
        let Edge::Barred { id, .. } = e else { continue };
        let want = seam_anchor(id);
        if opened.contains(want.as_str()) {
            continue;
        }
        d.push(Diagnostic::error(
            crate::layout::DW_GRAPH_MISSION,
            "layout-graph",
            format!("/content/edges/{i}"),
            format!(
                "`{id}` is barred and nothing in this campaign opens it. The derivation seals \
                 this seam's opening at world load and names the region `{want}`; for the way to \
                 ever be passable some effect has to address that name — an `open-gate` on the \
                 beat whose completion earns it, or a `shortcut` whose far side lifts the bar. \
                 The graph's `gating` says what a body must HOLD to pass, which is a different \
                 claim and is already checked: a way that is gated on a flag nobody spends is \
                 still a wall. This is the half of the obligation that could only be written once \
                 the region existed, so it is asked here rather than at stage 3."
            ),
        ));
    }
}

/// `datum/`, `volume/` and `view/` ids: well formed and unique, like every other
/// id in the DSL. An id is an id, so these are the ordinary `DW0110`/`DW0111`.
fn ids(plan: &SitePlanContent, d: &mut Vec<Diagnostic>) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut check_id = |ok: bool, id: String, kind: &str, path: String, d: &mut Vec<Diagnostic>| {
        if !ok {
            d.push(Diagnostic::error(
                crate::codes::ID_SYNTAX,
                "site-plan",
                path.clone(),
                format!("malformed {kind} id `{id}` — expected `{kind}/<kebab-case>`."),
            ));
        }
        if !seen.insert(format!("{kind}:{id}")) {
            d.push(Diagnostic::error(
                crate::codes::ID_DUPLICATE,
                "site-plan",
                path,
                format!(
                    "duplicate {kind} id `{id}` — rename one, because anything naming it would \
                     otherwise name both."
                ),
            ));
        }
    };
    for (i, dat) in plan.datums.iter().enumerate() {
        check_id(
            dat.id.is_valid_syntax(),
            dat.id.0.clone(),
            "datum",
            format!("/content/datums/{i}/id"),
            d,
        );
    }
    for (i, v) in plan.volumes.iter().enumerate() {
        check_id(
            v.id.is_valid_syntax(),
            v.id.0.clone(),
            "volume",
            format!("/content/volumes/{i}/id"),
            d,
        );
    }
    for (i, v) in plan.views.iter().enumerate() {
        check_id(
            v.id.is_valid_syntax(),
            v.id.0.clone(),
            "view",
            format!("/content/views/{i}/id"),
            d,
        );
    }
}

/// Resolve every box once: its footprint, its walk plane, its headroom and its
/// size class. A floor naming a datum the plan does not declare is the ordinary
/// dangling reference (`DW0112`) and the box is dropped, because a place with no
/// plane has no geometry for any rule below to judge.
fn resolve<'a>(
    plan: &'a SitePlanContent,
    graph: &LayoutGraphContent,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) -> Vec<Placed<'a>> {
    let datums: BTreeMap<&str, i64> = plan.datums.iter().map(|x| (x.id.0.as_str(), x.y)).collect();
    // Whichever of the two classifications the node declared. `DW0875` is what
    // refuses a node that declared both or neither; this map takes the size
    // class first so that a node which slipped past with both is judged against
    // one of the two rather than against neither — a refused campaign builds
    // nothing either way, and a check that quietly examines zero boxes is the
    // shape worth avoiding.
    let classes: BTreeMap<&str, (MetricKind, &str)> = graph
        .nodes
        .iter()
        .filter_map(|n| {
            let named = n
                .size_class
                .as_deref()
                .map(|x| (MetricKind::SizeClass, x))
                .or_else(|| n.way_class.as_deref().map(|x| (MetricKind::WayClass, x)))?;
            Some((n.id.0.as_str(), named))
        })
        .collect();
    let mut out = Vec::new();
    for (i, b) in plan.boxes.iter().enumerate() {
        let floor = match &b.floor {
            Floor::Y(y) => *y,
            Floor::Datum(id) => match datums.get(id.0.as_str()) {
                Some(y) => *y,
                None => {
                    d.push(Diagnostic::error(
                        crate::codes::DANGLING_REF,
                        "site-plan",
                        format!("/content/boxes/{i}/floor"),
                        format!(
                            "box for `{node}` stands on `{id}`, which this plan declares no \
                             `datums[]` entry for. Declare the plane, or give the box its own \
                             `y` — a place with no plane has no walk surface, so nothing below \
                             can say where it is.",
                            node = b.node,
                        ),
                    ));
                    continue;
                }
            },
        };
        let class = classes
            .get(b.node.0.as_str())
            .and_then(|(kind, name)| table.resolve(*kind, name).ok())
            .and_then(|entry| match entry.value(reads) {
                MetricValue::SizeClass(sc) => Some(PlaceClass::Size(*sc)),
                MetricValue::WayClass(w) => Some(PlaceClass::Way(*w)),
                _ => None,
            });
        let clearance = match b.ceiling {
            Ceiling::Clearance(c) => Some(c.get()),
            // A sky-open place claims its class's own minimum headroom and
            // nothing above it: an open place is precisely one that makes no
            // claim on the air over it. True of both kinds of class, which is
            // why the question is asked of the classification rather than of
            // one of its variants.
            Ceiling::Open => class.map(PlaceClass::min_clearance),
        };
        out.push(Placed {
            index: i,
            plan: b,
            foot: [
                b.min[0],
                b.min[0] + i64::from(b.extent[0].get()) - 1,
                b.min[1],
                b.min[1] + i64::from(b.extent[1].get()) - 1,
            ],
            floor,
            clearance,
            class,
        });
    }
    out
}

/// `DW0824`: the graph and the plan agree **exactly**, in both directions.
///
/// Six correspondences and three references, all one claim: everything the graph
/// declares is embedded exactly once, and everything the plan embeds is
/// something the graph declared.
///
/// This check is also the **two-artifact question's instrument** (spec-0049
/// §10): how often it fires *alone* — a graph edit with no plan edit or the
/// reverse — is the measurable evidence that decides whether the graph and the
/// plan stay two documents or merge into one.
///
/// It reads `plan.boxes` rather than the resolved list, deliberately: a box
/// whose floor named no datum is still a box the author wrote, and reporting it
/// as a place with no box as well would answer a question nobody asked.
fn agreement(
    plan: &SitePlanContent,
    graph: &LayoutGraphContent,
    placed: &[Placed<'_>],
    d: &mut Vec<Diagnostic>,
) {
    let fault = |path: String, msg: String, d: &mut Vec<Diagnostic>| {
        d.push(Diagnostic::error(DW_PLAN_AGREEMENT, "site-plan", path, msg));
    };

    // -------------------------------------------------------------- places
    let nodes: BTreeSet<&str> = graph.nodes.iter().map(|n| n.id.0.as_str()).collect();
    let mut boxed: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, b) in plan.boxes.iter().enumerate() {
        if !nodes.contains(b.node.0.as_str()) {
            fault(
                format!("/content/boxes/{i}/node"),
                format!(
                    "this box embeds `{n}`, which the layout graph declares no place for. A box \
                     is the geometry OF a place; one that names nothing is a room the map has \
                     no reason to contain. Declare the place, or delete the box.",
                    n = b.node,
                ),
                d,
            );
            continue;
        }
        boxed.entry(b.node.0.as_str()).or_default().push(i);
    }
    for (i, n) in graph.nodes.iter().enumerate() {
        match boxed.get(n.id.0.as_str()).map_or(0, Vec::len) {
            1 => {}
            0 => fault(
                "/content/boxes".to_string(),
                format!(
                    "place `{id}` has no box. Every place the graph declares is embedded exactly \
                     once — an unembedded place is a room the plan forgot, and nothing later can \
                     notice it, because every geometric rule quantifies over the boxes that \
                     exist. (Graph node {i} of {total}.)",
                    id = n.id,
                    total = graph.nodes.len(),
                ),
                d,
            ),
            k => fault(
                "/content/boxes".to_string(),
                format!(
                    "place `{id}` has {k} boxes. A place is one space; two boxes for it make \
                     every rule below pick one of them and no rule says which.",
                    id = n.id,
                ),
                d,
            ),
        }
    }

    // --------------------------------------------------------- connections
    let edges: BTreeMap<&str, &Edge> = graph.edges.iter().map(|e| (e.id().0.as_str(), e)).collect();
    let mut seamed: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, s) in plan.seams.iter().enumerate() {
        match edges.get(s.edge.0.as_str()) {
            None => fault(
                format!("/content/seams/{i}/edge"),
                format!(
                    "this seam allocates `{e}`, which the layout graph declares no connection \
                     for. A seam is an opening cut for a connection; one that names nothing is a \
                     hole in a wall for no reason.",
                    e = s.edge,
                ),
                d,
            ),
            Some(Edge::Vision { .. }) => fault(
                format!("/content/seams/{i}/edge"),
                format!(
                    "`{e}` is a `vision` connection and carries a **sightline**, not a seam. A \
                     seam is an opening on a shared face, and a vista's two ends are routinely \
                     not adjacent — a tower seen from a shore shares no face with it — so the \
                     seam construct cannot state the one thing a vision connection asserts. Move \
                     it to `sightlines[]`.",
                    e = s.edge,
                ),
                d,
            ),
            Some(_) => {
                seamed.entry(s.edge.0.as_str()).or_default().push(i);
            }
        }
    }
    let mut sighted: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, s) in plan.sightlines.iter().enumerate() {
        match edges.get(s.edge.0.as_str()) {
            None => fault(
                format!("/content/sightlines/{i}/edge"),
                format!(
                    "this sightline embeds `{e}`, which the layout graph declares no connection \
                     for.",
                    e = s.edge,
                ),
                d,
            ),
            Some(e) if !matches!(e, Edge::Vision { .. }) => fault(
                format!("/content/sightlines/{i}/edge"),
                format!(
                    "`{id}` is a `{class}` connection: a body passes along it, so it is allocated \
                     a **seam** on a shared face rather than a line of sight. Move it to \
                     `seams[]`.",
                    id = s.edge,
                    class = e.class(),
                ),
                d,
            ),
            Some(_) => {
                sighted.entry(s.edge.0.as_str()).or_default().push(i);
            }
        }
    }
    for (i, e) in graph.edges.iter().enumerate() {
        let (what, held, other) = if e.is_traversal() {
            ("seam", &seamed, "seams")
        } else {
            ("sightline", &sighted, "sightlines")
        };
        match held.get(e.id().0.as_str()).map_or(0, Vec::len) {
            1 => {}
            0 => fault(
                format!("/content/{other}"),
                format!(
                    "connection `{id}` ({class}) has no {what}. **Seams are allocated, not \
                     discovered**: two places connect because the plan cut an opening between \
                     them while both were still free to move, never because a wall happened to \
                     be low somewhere. A connection with nothing allocated is a promise the \
                     geometry has not been asked to keep. (Graph edge {i} of {total}.)",
                    id = e.id(),
                    class = e.class(),
                    total = graph.edges.len(),
                ),
                d,
            ),
            k => fault(
                format!("/content/{other}"),
                format!(
                    "connection `{id}` has {k} {what}s. One connection is one way through; two \
                     openings for it are two ways, and the graph declared one.",
                    id = e.id(),
                ),
                d,
            ),
        }
    }

    // ---------------------------------- what a seam says ABOUT its connection
    let by_node: BTreeMap<&str, &Placed<'_>> =
        placed.iter().map(|p| (p.plan.node.0.as_str(), p)).collect();
    for (i, s) in plan.seams.iter().enumerate() {
        let Some(e) = edges.get(s.edge.0.as_str()) else {
            continue;
        };
        let Some(host) = &s.stair_in else {
            continue;
        };
        if !matches!(e, Edge::Stair { .. }) {
            fault(
                format!("/content/seams/{i}/stair_in"),
                format!(
                    "this seam declares stair massing in `{host}`, and `{id}` is a `{class}` \
                     connection. Only a stair is built out of treads; on anything else the \
                     declaration is a fact about the geometry that the graph contradicts.",
                    id = s.edge,
                    class = e.class(),
                ),
                d,
            );
        } else if host != e.a() && host != e.b() {
            fault(
                format!("/content/seams/{i}/stair_in"),
                format!(
                    "this seam hosts its stair in `{host}`, which is neither end of `{id}` \
                     (`{a}` and `{b}`). A stair stands in one of the two places it joins.",
                    id = s.edge,
                    a = e.a(),
                    b = e.b(),
                ),
                d,
            );
        }
    }

    // ------------------------- what a sightline says ABOUT its two places
    for (i, s) in plan.sightlines.iter().enumerate() {
        let Some(e) = edges.get(s.edge.0.as_str()) else {
            continue;
        };
        for (end, point, node) in [("from", s.from, e.a()), ("to", s.to, e.b())] {
            let Some(p) = by_node.get(node.0.as_str()) else {
                continue;
            };
            if contains_point(p, point) {
                continue;
            }
            fault(
                format!("/content/sightlines/{i}/{end}"),
                format!(
                    "`{id}` is a line of sight between `{a}` and `{b}`, and its `{end}` end \
                     `[{x}, {y}, {z}]` is not inside `{node}`. The stage-5 proof walks exactly \
                     this segment and calls the result the vista's; a segment whose ends are \
                     somewhere else would be proving a different claim, green or red.",
                    id = s.edge,
                    a = e.a(),
                    b = e.b(),
                    x = point[0],
                    y = point[1],
                    z = point[2],
                ),
                d,
            );
        }
    }
}

/// Is a world cell inside a place's play space?
fn contains_point(p: &Placed<'_>, at: [i64; 3]) -> bool {
    if at[0] < p.x0() || at[0] > p.x1() || at[2] < p.z0() || at[2] > p.z1() {
        return false;
    }
    match p.y_span() {
        Some((lo, hi)) => at[1] >= lo && at[1] <= hi,
        // A sky-open place whose class did not resolve has no stated headroom;
        // `DW0812` already refused the name, and inventing a bound here would be
        // a second refusal for one defect.
        None => at[1] >= p.floor,
    }
}

/// `DW0825`: every box's footprint is a multiple of the kit grid's quantum.
fn grid(placed: &[Placed<'_>], table: &Metrics, reads: &mut Reads, d: &mut Vec<Diagnostic>) {
    let Some(grid) = table.grid(reads) else {
        return; // `Metrics::self_check` owns a table that defines no grid.
    };
    let q = grid.quantum;
    if q == 0 {
        return;
    }
    for p in placed {
        for (axis, name) in [(0usize, "x"), (1usize, "z")] {
            let e = p.plan.extent[axis].get();
            if !off_grid(e, q) {
                continue;
            }
            d.push(Diagnostic::error(
                DW_BOX_OFF_GRID,
                "site-plan",
                format!("/content/boxes/{}/extent/{axis}", p.index),
                format!(
                    "box for `{node}` is {e} blocks on {name}, and the kit grid's quantum is \
                     {q} — so it is not a multiple of it. Every box's footprint is a whole \
                     number of quanta on both horizontal axes, which is what lets a kit piece \
                     land in one without being cut. The nearest multiples are {lo} and {hi}.",
                    node = p.plan.node,
                    lo = e - e % q,
                    hi = e - e % q + q,
                ),
            ));
        }
    }
}

/// **`DW0825`'s own test, for one footprint on one axis.** One function so that
/// a verdict computed from a box can ask the SAME question the refusal asked,
/// rather than re-deriving the kit-grid rule beside it.
fn off_grid(extent: u32, quantum: u32) -> bool {
    quantum != 0 && !extent.is_multiple_of(quantum)
}

/// **The clause a stage-6 verdict owes when the allocation it measured against
/// is one this plan has already refused** — the DEFER shape of
/// [`crate::diagnostic`]'s "one cause, one line".
///
/// A `details[]` row is judged against a FRAME and a SEAM SET, and both are
/// computed from the site plan. When the plan's own refusals have already
/// touched them, the stage-6 line is a true, separate finding measured against a
/// number the map does not really have — and, worse, the primary is in another
/// document, so the reader has no way to see the relation. Measured: widening
/// one box by one block printed `DW0825` and `DW0828` in the site plan and then
/// `DW0843` and `DW0844` in the detail plan, five codes over three documents,
/// with nothing saying which was the edit.
///
/// So the stage-6 verdicts keep their own lines — each still names a real
/// mismatch, and suppressing them is how fixing one thing produces a fresh crop
/// of refusals nobody was shown — and gain a clause saying what they are
/// downstream of. Two things can be already-refused:
///
/// 1. the place's own box is off the kit grid (`DW0825`), so its frame is not a
///    frame this map will keep;
/// 2. the plan DECLARES a seam on this place that it did not resolve — a face
///    the two boxes do not share (`DW0828`) or an opening that does not fit
///    (`DW0829`) — so the allocated seam set this place answers is short of what
///    the author wrote.
///
/// Empty when the plan settled both, which is the ordinary case and costs one
/// pass over the plan's seams.
#[must_use]
pub fn refused_upstream(
    c: &Campaign,
    node: &NodeId,
    resolved: &[PlacedSeam],
    reads: &mut Reads,
) -> String {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();

    if let Some(q) = Metrics::table().grid(reads).map(|g| g.quantum)
        && let Some(b) = plan.boxes.iter().find(|b| &b.node == node)
    {
        let axes: Vec<&str> = [(0usize, "x"), (1usize, "z")]
            .into_iter()
            .filter(|(a, _)| off_grid(b.extent[*a].get(), q))
            .map(|(_, name)| name)
            .collect();
        if !axes.is_empty() {
            parts.push(format!(
                "this place's box is off the kit grid on {axes} and `DW0825` has already refused \
                 it, so the frame above is not one this map keeps",
                axes = axes.join(" and "),
            ));
        }
    }

    // A seam the plan wrote and did not resolve: the graph edge names this
    // place, the plan has a seam row for it, and nothing came out the other end.
    let graph_edges: BTreeMap<&str, &Edge> = c
        .layout_graph
        .as_ref()
        .map(|g| {
            g.content
                .edges
                .iter()
                .map(|e| (e.id().0.as_str(), e))
                .collect()
        })
        .unwrap_or_default();
    let unresolved: Vec<String> = plan
        .seams
        .iter()
        .filter(|s| {
            graph_edges
                .get(s.edge.0.as_str())
                .is_some_and(|e| e.a() == node || e.b() == node)
                && !resolved.iter().any(|r| r.edge == s.edge)
        })
        .map(|s| format!("`{}`", s.edge))
        .collect();
    if !unresolved.is_empty() {
        parts.push(format!(
            "the plan writes {n} seam(s) on this place that it does not resolve ({list}), which \
             `DW0828` or `DW0829` has already refused, so the allocation this piece is answering \
             is short of what the plan says",
            n = unresolved.len(),
            list = unresolved.join(", "),
        ));
    }

    if parts.is_empty() {
        return String::new();
    }
    format!(
        " This measurement stands downstream of a site-plan refusal: {parts}. Repair the plan \
         first — this line moves with it.",
        parts = parts.join("; "),
    )
}

/// `DW0826`: nothing the plan places leaves the region.
///
/// **The region is the brief's number flowing down.** A box is never grounds to
/// grow it: the prescription in the message is to move or shrink the box, or to
/// change the brief's fact and re-derive *visibly* — never to let the extent be
/// whatever the parts added up to, which is the failure this whole stage was
/// bought to end.
///
/// Whole-owned volumes answer to it too. A `massif` outside the region is the
/// whole owning mass beyond its own declared extent, which is the same
/// extent-flows-up defect arriving through the back door.
///
/// **One region number is one finding, however many boxes stand outside it**
/// (`crate::diagnostic`'s "one cause, one line"). The region is a single fact
/// handed down by the brief, so a region a course too short puts *every* box
/// over the same edge and each per-box refusal prints the same two numbers with
/// a different name in front: measured on a 24-place campaign, shortening the
/// region by five courses printed 24 identical paragraphs. When more than one
/// thing leaves it, this states the count, names every offender with its own
/// overrun, and prescribes once. A single offender still gets its own line
/// exactly as before — the fold is reachable only where the copies would have
/// been.
fn region(plan: &SitePlanContent, placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    let r = &plan.region;
    let spans = [region_span(r, 0), region_span(r, 1), region_span(r, 2)];
    // How the region reads once, for the folded arms: the per-item clause
    // repeats it, and repeating it N times is most of what made N copies
    // unreadable.
    let region_text = format!(
        "x {}..{}, y {}..{}, z {}..{}",
        spans[0].0, spans[0].1, spans[1].0, spans[1].1, spans[2].0, spans[2].1
    );

    // ---- boxes ----
    let mut boxes_out: Vec<Overrun<'_>> = Vec::new();
    for p in placed {
        let mut bad: Vec<(&'static str, i64, i64)> = Vec::new();
        if !within((p.x0(), p.x1()), spans[0]) {
            bad.push(("x", p.x0(), p.x1()));
        }
        if let Some(y) = p.y_span()
            && !within(y, spans[1])
        {
            bad.push(("y", y.0, y.1));
        }
        if !within((p.z0(), p.z1()), spans[2]) {
            bad.push(("z", p.z0(), p.z1()));
        }
        if !bad.is_empty() {
            boxes_out.push(Overrun {
                index: p.index,
                name: p.plan.node.0.as_str(),
                axes: bad,
            });
        }
    }
    if boxes_out.len() == 1 {
        let o = &boxes_out[0];
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            format!("/content/boxes/{}", o.index),
            format!(
                "box for `{node}` leaves the region: {bad}. The region is the whole map's \
                 extent, and it comes from the brief — a box is never grounds to grow it. Move \
                 the box, shrink it, or change the brief's fact and re-derive the region so the \
                 change is visible in the document that owns it.",
                node = o.name,
                bad = against_region(&o.axes, &spans),
            ),
        ));
    } else if boxes_out.len() > 1 {
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            "/content/boxes",
            format!(
                "{n} of the {total} box(es) this plan places leave the region, which is \
                 {region_text}: {list}. The region is the whole map's extent, and it comes from \
                 the brief — a box is never grounds to grow it. One region is the cause of all \
                 {n} of these, which is why they are one line and not {n}: move or shrink the \
                 boxes, or change the brief's fact and re-derive the region so the change is \
                 visible in the document that owns it.",
                n = boxes_out.len(),
                total = placed.len(),
                list = named_overruns(&boxes_out),
            ),
        ));
    }

    // ---- whole-owned volumes ----
    let mut volumes_out: Vec<Overrun<'_>> = Vec::new();
    for (i, v) in plan.volumes.iter().enumerate() {
        let vmax = v.region.max();
        let mut bad: Vec<(&'static str, i64, i64)> = Vec::new();
        for (axis, name) in [(0usize, "x"), (1, "y"), (2, "z")] {
            if !within((v.region.min[axis], vmax[axis]), spans[axis]) {
                bad.push((name, v.region.min[axis], vmax[axis]));
            }
        }
        if !bad.is_empty() {
            volumes_out.push(Overrun {
                index: i,
                name: v.id.0.as_str(),
                axes: bad,
            });
        }
    }
    if volumes_out.len() == 1 {
        let o = &volumes_out[0];
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            format!("/content/volumes/{}", o.index),
            format!(
                "whole-owned volume `{id}` leaves the region: {bad}. The region is the whole's \
                 own extent; mass outside it is the whole growing to fit what was put in it, \
                 which is the direction this stage exists to forbid.",
                id = o.name,
                bad = against_region(&o.axes, &spans),
            ),
        ));
    } else if volumes_out.len() > 1 {
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            "/content/volumes",
            format!(
                "{n} of the {total} whole-owned volume(s) leave the region, which is \
                 {region_text}: {list}. The region is the whole's own extent; mass outside it is \
                 the whole growing to fit what was put in it, which is the direction this stage \
                 exists to forbid. One region is the cause of all {n} of these, which is why \
                 they are one line and not {n}.",
                n = volumes_out.len(),
                total = plan.volumes.len(),
                list = named_overruns(&volumes_out),
            ),
        ));
    }
}

/// One offender's overrun, spelled against the region on each axis it leaves —
/// the wording a single finding carries, where the region's own numbers are
/// stated beside the box's because there is only one line to read them in.
fn against_region(bad: &[(&'static str, i64, i64)], spans: &[(i64, i64); 3]) -> String {
    bad.iter()
        .map(|(name, lo, hi)| {
            let axis = match *name {
                "x" => 0,
                "y" => 1,
                _ => 2,
            };
            format!(
                "{name} {lo}..{hi} against the region's {}..{}",
                spans[axis].0, spans[axis].1
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// One thing the plan places that stands outside the region, and where.
///
/// A named struct rather than a tuple because both arms read it: the single
/// finding addresses its own `index`, the folded one prints `name` and `axes`,
/// and boxes and volumes differ in nothing else.
struct Overrun<'a> {
    /// Position in its own array — the path a single finding is addressed at.
    index: usize,
    /// The id an author reads.
    name: &'a str,
    /// Each axis it leaves, with its own inclusive span on that axis.
    axes: Vec<(&'static str, i64, i64)>,
}

/// The offender list a folded finding carries: every name with its own overrun,
/// and no repeat of the region — the folded message states that once.
fn named_overruns(items: &[Overrun<'_>]) -> String {
    items
        .iter()
        .map(|o| {
            format!(
                "`{}` ({})",
                o.name,
                o.axes
                    .iter()
                    .map(|(ax, lo, hi)| format!("{ax} {lo}..{hi}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `DW0827`: the boxes are disjoint.
///
/// Shared **faces** are the only permitted contact, because a seam needs one —
/// and a shared face is a one-cell gap, not a touch (see [`Placed`]). Two boxes
/// whose play spaces meet are two authorities over one cell, which the
/// derivation would have to arbitrate and must never be asked to.
fn disjoint(placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    for (a_i, a) in placed.iter().enumerate() {
        for b in &placed[a_i + 1..] {
            let (Some(x), Some(z)) = (
                overlap((a.x0(), a.x1()), (b.x0(), b.x1())),
                overlap((a.z0(), a.z1()), (b.z0(), b.z1())),
            ) else {
                continue;
            };
            let y = match (a.y_span(), b.y_span()) {
                (Some(ya), Some(yb)) => match overlap(ya, yb) {
                    Some(y) => y,
                    None => continue,
                },
                // One of them is sky-open with an unresolved class; `DW0812`
                // owns that name, and the footprints alone are enough to say
                // the two places stand in each other.
                _ => (a.floor.min(b.floor), a.floor.max(b.floor)),
            };
            d.push(Diagnostic::error(
                DW_BOXES_OVERLAP,
                "site-plan",
                format!("/content/boxes/{}", b.index),
                format!(
                    "the boxes for `{a_n}` and `{b_n}` overlap, sharing x {x0}..{x1}, y \
                     {y0}..{y1}, z {z0}..{z1}. Two places may share a FACE — that is what a seam \
                     is cut through — but never a cell: overlapping boxes are two owners for one \
                     block, and the derivation would have to pick between them with no rule to \
                     pick by. Boxes that connect sit one cell apart, and the cell between them \
                     is the wall they have in common.",
                    a_n = a.plan.node,
                    b_n = b.plan.node,
                    x0 = x.0,
                    x1 = x.1,
                    y0 = y.0,
                    y1 = y.1,
                    z0 = z.0,
                    z1 = z.1,
                ),
            ));
        }
    }
}

/// `DW0835`: the whole's mass stands beside, under and over places — never
/// inside one.
fn volumes_outside_boxes(plan: &SitePlanContent, placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    for (i, v) in plan.volumes.iter().enumerate() {
        let vmax = v.region.max();
        for p in placed {
            let (Some(x), Some(z)) = (
                overlap((v.region.min[0], vmax[0]), (p.x0(), p.x1())),
                overlap((v.region.min[2], vmax[2]), (p.z0(), p.z1())),
            ) else {
                continue;
            };
            let Some(py) = p.y_span() else { continue };
            let Some(y) = overlap((v.region.min[1], vmax[1]), py) else {
                continue;
            };
            d.push(Diagnostic::error(
                DW_VOLUME_IN_BOX,
                "site-plan",
                format!("/content/volumes/{i}"),
                format!(
                    "whole-owned volume `{id}` ({role}) enters the box for `{node}`, sharing x \
                     {x0}..{x1}, y {y0}..{y1}, z {z0}..{z1}. The whole's mass may stand beside a \
                     place, under it and over it; inside it, the volume and the place are two \
                     authorities writing one cell, and the derivation must never be asked to \
                     arbitrate that. Pull the volume back to the place's face, or move the \
                     place.",
                    id = v.id,
                    role = v.role.as_str(),
                    node = p.plan.node,
                    x0 = x.0,
                    x1 = x.1,
                    y0 = y.0,
                    y1 = y.1,
                    z0 = z.0,
                    z1 = z.1,
                ),
            ));
        }
    }
}

/// `DW0832`: a box is built to its place's class — **either kind** (spec-0053
/// §3).
///
/// # The way branch, and why its third demand is structural
///
/// A size class bounds both horizontal extents and this is the one place that
/// becomes geometry. A way class bounds only the **cross-section**, which is the
/// box's *shorter* horizontal extent — the axis a body feels — and then demands
/// that the **run**, the longer extent, strictly EXCEED the class's
/// `max_width`.
///
/// That third demand is the elongation, and it is deliberately derived from the
/// class's own widest cross-section rather than seeded as a constant, because it
/// is exactly what a room cannot supply. A square box can never satisfy it: its
/// run equals its width, and one number cannot both be `<= max_width` and exceed
/// it. So "declare a room a way to escape the size ladder" is refused **by the
/// object's own shape** rather than by a rule the author could satisfy by
/// choosing differently — the property `CLAUDE.md` demands of an opt-out, since
/// the defect this branch exists to catch is structurally incapable of
/// producing its proof.
///
/// There is no maximum run and there is not going to be one: a route's length is
/// per-campaign geometry, never a standard (spec-0053 §7).
fn size_classes(placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    for p in placed {
        let Some(class) = p.class else {
            continue; // `DW0812` refused the name.
        };
        let (kind, mut bad) = match class {
            PlaceClass::Size(sc) => {
                let mut bad: Vec<String> = Vec::new();
                for (axis, name) in [(0usize, "x"), (1, "z")] {
                    let e = p.plan.extent[axis].get();
                    if e < sc.min_footprint[axis] || e > sc.max_footprint[axis] {
                        bad.push(format!(
                            "{e} blocks on {name}, outside the class's {}..{}",
                            sc.min_footprint[axis], sc.max_footprint[axis]
                        ));
                    }
                }
                ("size", bad)
            }
            PlaceClass::Way(w) => {
                let mut bad: Vec<String> = Vec::new();
                let (dx, dz) = (p.plan.extent[0].get(), p.plan.extent[1].get());
                let (width, run) = (dx.min(dz), dx.max(dz));
                let axis = if dx <= dz { "x" } else { "z" };
                if width < w.min_width || width > w.max_width {
                    bad.push(format!(
                        "a cross-section of {width} blocks (its shorter extent, on {axis}), \
                         outside the class's {}..{}",
                        w.min_width, w.max_width
                    ));
                }
                if run <= w.max_width {
                    bad.push(format!(
                        "a run of {run} blocks, which does not exceed the class's widest \
                         cross-section of {}. A way is a place that is longer than it is wide \
                         by kind and not by margin, so this box is a room — give it a \
                         `size_class` instead, or make it longer",
                        w.max_width
                    ));
                }
                ("way", bad)
            }
        };
        if let Ceiling::Clearance(c) = p.plan.ceiling
            && c.get() < class.min_clearance()
        {
            bad.push(format!(
                "{c} cells of headroom, under the class's minimum of {}",
                class.min_clearance()
            ));
        }
        if bad.is_empty() {
            continue;
        }
        d.push(Diagnostic::error(
            DW_SIZE_CLASS,
            "site-plan",
            format!("/content/boxes/{}", p.index),
            format!(
                "the box for `{node}` is not built to its declared {kind} class: {bad}. The \
                 class is the vocabulary the graph chose this place's scale in, and this is the \
                 one place it becomes geometry — either build the box to it, or declare the \
                 place a different class in the layout graph and say so there.",
                node = p.plan.node,
                bad = bad.join("; "),
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Seams: allocated on a face both boxes already have
// ---------------------------------------------------------------------------

/// The rectangle two boxes have in common on one face, in the face's own two
/// in-plane world axes, plus the plane the wall between them stands in.
#[derive(Debug, Clone, Copy)]
struct SharedFace {
    plane: i64,
    u: (i64, i64),
    v: (i64, i64),
    u_axis: &'static str,
    v_axis: &'static str,
}

/// Why two boxes do not share the declared face.
#[derive(Debug, Clone)]
enum NotShared {
    /// They are not neighbours across it: the gap is `gap` cells where the one
    /// wall they would have in common needs exactly 1.
    NotAdjacent { gap: i64 },
    /// They are neighbours, but the face they would share is empty because their
    /// spans miss each other on one of the two in-plane axes.
    NoCommonArea { axis: &'static str },
    /// One of them is sky-open with no stated headroom, so it has no ceiling or
    /// floor plane for a horizontal seam to sit in.
    NoPlane { which: &'static str },
}

/// Do these two boxes share `face` **of `a`**, and where?
///
/// A shared face is a **one-cell gap** — the wall the two places have in common,
/// which the derivation writes once. See [`Placed`] for why the box is the play
/// space rather than the play space plus its shell.
/// The geometry a shared-face question needs of one box: its footprint and its
/// vertical span, when it has one.
///
/// A tiny value rather than `&Placed` so that the **one** implementation of "do
/// these two boxes share this face" serves both readers of the resolved plan:
/// the stage-4 checks, which hold a partially-resolved box, and the derivation
/// and battery in the compiler, which hold a [`PlacedBox`]. A second copy of
/// this arithmetic is how a plan-time green and a byte-time green come to be
/// about different walls.
#[derive(Clone, Copy)]
struct FaceSide {
    foot: [i64; 4],
    y: Option<(i64, i64)>,
}

impl Placed<'_> {
    fn side(&self) -> FaceSide {
        FaceSide {
            foot: self.foot,
            y: self.y_span(),
        }
    }
}

impl PlacedBox {
    fn side(&self) -> FaceSide {
        let (lo, hi) = self.space();
        FaceSide {
            foot: self.foot,
            y: Some((lo[1], hi[1])),
        }
    }
}

/// [`shared_face`] over two fully resolved boxes.
fn shared_face_of(a: &PlacedBox, b: &PlacedBox, face: Face) -> Result<SharedFace, NotShared> {
    shared_face(a.side(), b.side(), face)
}

fn shared_face(a: FaceSide, b: FaceSide, face: Face) -> Result<SharedFace, NotShared> {
    let horizontal_pair = |plane: i64, u: (i64, i64), v: (i64, i64)| -> SharedFace {
        SharedFace {
            plane,
            u,
            v,
            u_axis: "x",
            v_axis: "z",
        }
    };
    match face {
        Face::East | Face::West | Face::South | Face::North => {
            // The axis the face's normal runs along, and the horizontal axis
            // that stays in the plane.
            let (normal, along) = match face {
                Face::East | Face::West => (0usize, 2usize),
                _ => (2usize, 0usize),
            };
            let a_span = span(a.foot, normal);
            let b_span = span(b.foot, normal);
            let positive = matches!(face, Face::East | Face::South);
            let (plane, gap) = if positive {
                (a_span.1 + 1, b_span.0 - a_span.1 - 1)
            } else {
                (a_span.0 - 1, a_span.0 - b_span.1 - 1)
            };
            if gap != SHARED_FACE_GAP_CELLS {
                return Err(NotShared::NotAdjacent { gap });
            }
            let a_along = span(a.foot, along);
            let b_along = span(b.foot, along);
            let u = overlap(a_along, b_along).ok_or(NotShared::NoCommonArea {
                axis: if along == 0 { "x" } else { "z" },
            })?;
            let (ya, yb) = match (a.y, b.y) {
                (Some(ya), Some(yb)) => (ya, yb),
                (None, _) => return Err(NotShared::NoPlane { which: "a" }),
                (_, None) => return Err(NotShared::NoPlane { which: "b" }),
            };
            let v = overlap(ya, yb).ok_or(NotShared::NoCommonArea { axis: "y" })?;
            Ok(SharedFace {
                plane,
                u,
                v,
                u_axis: if along == 0 { "x" } else { "z" },
                v_axis: "y",
            })
        }
        Face::Up | Face::Down => {
            let (Some(ya), Some(yb)) = (a.y, b.y) else {
                return Err(NotShared::NoPlane {
                    which: if a.y.is_none() { "a" } else { "b" },
                });
            };
            let (plane, gap) = if face == Face::Up {
                (ya.1 + 1, yb.0 - ya.1 - 1)
            } else {
                (ya.0 - 1, ya.0 - yb.1 - 1)
            };
            if gap != SHARED_FACE_GAP_CELLS {
                return Err(NotShared::NotAdjacent { gap });
            }
            let u = overlap(span(a.foot, 0), span(b.foot, 0))
                .ok_or(NotShared::NoCommonArea { axis: "x" })?;
            let v = overlap(span(a.foot, 2), span(b.foot, 2))
                .ok_or(NotShared::NoCommonArea { axis: "z" })?;
            Ok(horizontal_pair(plane, u, v))
        }
    }
}

/// One seam and everything already resolved about it: the two places it joins,
/// the connection it allocates, and the face they share. Carried as one value so
/// each rule below takes the seam and its context rather than eight positional
/// arguments — the shape a `clippy::too_many_arguments` allow would otherwise
/// have papered over.
struct SeamCtx<'a> {
    index: usize,
    seam: &'a Seam,
    edge: &'a Edge,
    a: &'a Placed<'a>,
    b: &'a Placed<'a>,
    face: SharedFace,
    /// The `dsl_version` the SITE PLAN stage declares — what the contact fence
    /// is judged against.
    version: &'a str,
}

/// `DW0828`–`DW0831`: every seam sits on a face its two boxes share, at cells
/// that face has, through a standard opening a body can use, and — where the two
/// places are on different planes — by a climb or a fall the standards allow.
fn seams(
    plan: &SitePlanContent,
    graph: &LayoutGraphContent,
    placed: &[Placed<'_>],
    table: &Metrics,
    version: &str,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) {
    let by_node: BTreeMap<&str, &Placed<'_>> =
        placed.iter().map(|p| (p.plan.node.0.as_str(), p)).collect();
    let edges: BTreeMap<&str, &Edge> = graph.edges.iter().map(|e| (e.id().0.as_str(), e)).collect();

    for (i, s) in plan.seams.iter().enumerate() {
        let Some(edge) = edges.get(s.edge.0.as_str()) else {
            continue; // `DW0824` refused the reference.
        };
        if matches!(edge, Edge::Vision { .. }) {
            continue; // `DW0824` said this carries a sightline.
        }
        let (Some(a), Some(b)) = (
            by_node.get(edge.a().0.as_str()).copied(),
            by_node.get(edge.b().0.as_str()).copied(),
        ) else {
            continue; // `DW0824` reported the missing box.
        };

        let face = match shared_face(a.side(), b.side(), s.face) {
            Ok(f) => f,
            Err(why) => {
                d.push(not_shared(i, s, edge, a, b, &why));
                continue;
            }
        };

        let ctx = SeamCtx {
            index: i,
            seam: s,
            edge,
            a,
            b,
            face,
            version,
        };

        // `DW0876`, first: a seam that does not state exactly one kind of
        // connection has no crossing for any rule below to judge, and telling
        // an author both that and what the crossing they did not state would
        // have meant prescribes two repairs for one mistake.
        if !contact_declaration(&ctx, table, reads, d) {
            continue;
        }

        let opening = if s.contact.is_some() {
            // A contact has no opening name to resolve and no single sill, so
            // `DW0829` does not run over it. That is stated rather than
            // shoehorned: calling a 55-cell front a door would make every
            // downstream door check wrong (spec-0053 §4).
            None
        } else {
            match table.resolve(
                MetricKind::Opening,
                s.opening.as_deref().unwrap_or_default(),
            ) {
                Ok(e) => match e.value(reads) {
                    MetricValue::Opening(o) => Some(*o),
                    _ => continue,
                },
                Err(unknown) => {
                    d.push(unknown.diagnostic("site-plan", &format!("/content/seams/{i}/opening")));
                    continue;
                }
            }
        };

        if let Some(opening) = opening {
            opening_fits(&ctx, opening, d);
        }
        match edge {
            Edge::Stair { .. } => stair(&ctx, table, reads, d),
            Edge::Drop { falls, .. } => drop_seam(&ctx, *falls, table, reads, d),
            Edge::Walk { .. } | Edge::Barred { .. } => {
                if let Some(opening) = opening {
                    sill(&ctx, opening, d);
                }
            }
            Edge::Vision { .. } => {}
        }
        if matches!(edge, Edge::Stair { .. }) && s.stair_in.is_none() {
            d.push(Diagnostic::error(
                DW_STAIR_PITCH,
                "site-plan",
                format!("/content/seams/{i}"),
                format!(
                    "the seam for stair `{id}` does not say which place hosts its treads. A \
                     stair is massing, and massing stands somewhere: name `{a}` or `{b}` in \
                     `stair_in`, so that the run it costs comes out of a footprint the plan has \
                     already allocated rather than out of whatever space happens to be left.",
                    id = s.edge,
                    a = edge.a(),
                    b = edge.b(),
                ),
            ));
        }
    }
}

/// `DW0828`, with the arithmetic that produced it.
fn not_shared(
    i: usize,
    s: &Seam,
    edge: &Edge,
    a: &Placed<'_>,
    b: &Placed<'_>,
    why: &NotShared,
) -> Diagnostic {
    let detail = match why {
        NotShared::NotAdjacent { gap } if *gap < 0 => format!(
            "they overlap by {} cell(s) across it rather than standing one apart",
            -gap
        ),
        NotShared::NotAdjacent { gap } => format!(
            "there are {gap} cells between them across that face where a shared wall is exactly \
             {SHARED_FACE_GAP_CELLS}"
        ),
        NotShared::NoCommonArea { axis } => format!(
            "they are neighbours across it, but their spans on {axis} miss each other entirely, \
             so the face they share has no area to cut an opening in"
        ),
        NotShared::NoPlane { which } => format!(
            "the `{which}` end is sky-open with no stated headroom, so it has no ceiling or floor \
             plane for a horizontal seam to sit in"
        ),
    };
    Diagnostic::error(
        DW_SEAM_NOT_SHARED,
        "site-plan",
        format!("/content/seams/{i}/face"),
        format!(
            "the seam for `{id}` is declared on the {face} face of `{an}`, and `{an}` and `{bn}` \
             do not share it: {detail}. **A seam is allocated on a face both boxes already \
             have** — that is the whole of why the plan places it while both are still free to \
             move, instead of two finished places discovering later that they cannot mate. \
             `{an}` is x {ax0}..{ax1}, z {az0}..{az1} at floor {af}; `{bn}` is x {bx0}..{bx1}, \
             z {bz0}..{bz1} at floor {bf}. Move one box against the other, or put the seam on \
             the face they really share.",
            id = s.edge,
            face = s.face.as_str(),
            an = edge.a(),
            bn = edge.b(),
            ax0 = a.x0(),
            ax1 = a.x1(),
            az0 = a.z0(),
            az1 = a.z1(),
            af = a.floor,
            bx0 = b.x0(),
            bx1 = b.x1(),
            bz0 = b.z0(),
            bz1 = b.z1(),
            bf = b.floor,
        ),
    )
}

/// **`DW0876` and the per-stage fence**: this seam states exactly one kind of
/// connection, and if it is a contact, one this engine builds (spec-0053 §4).
///
/// Returns `false` when the seam has no usable crossing, in which case the
/// caller stops: everything below reads the crossing rectangle.
fn contact_declaration(
    ctx: &SeamCtx<'_>,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) -> bool {
    let (i, s) = (ctx.index, ctx.seam);
    let mut refuse = |what: String, remedy: String| {
        d.push(Diagnostic::error(
            DW_CONTACT,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "the seam for `{edge}` {what}. To fix it, {remedy}.",
                edge = s.edge,
            ),
        ));
    };

    // ---- Shape 1: exactly one kind.
    match (s.opening.as_ref(), s.contact.as_ref()) {
        (Some(o), Some(_)) => {
            refuse(
                format!(
                    "declares BOTH an `opening` (`{o}`) and a `contact` — a hand-off is one \
                     kind or the other"
                ),
                "delete whichever this is not. A portal allocates the cells a body crosses \
                 at and every one of them must be passable; a contact is a front along which \
                 two places simply meet and needs only one crossable column. The derivation \
                 builds them differently and the byte observer measures them differently, so \
                 there is no world in which a seam is both"
                    .to_string(),
            );
            return false;
        }
        (None, None) => {
            refuse(
                "declares neither an `opening` nor a `contact`, so it states no way across"
                    .to_string(),
                format!(
                    "give it one. A doorway is `\"opening\": \"<name>\"` — defined \
                     standards: {names}. A front where the two places simply meet is \
                     `\"contact\": {{}}`, which spans from `at` to the far edge of the \
                     shared face",
                    names = table.names_of(MetricKind::Opening).join(", "),
                ),
            );
            return false;
        }
        (Some(_), None) => return true,
        (None, Some(_)) => {}
    }

    // ---- The fence. A WELLFORMEDNESS rule, judged against the version this
    // document declares. Below it there is nothing else to say about a contact.
    if !crate::is_v19(ctx.version) {
        d.push(Diagnostic::error(
            crate::codes::RESERVED,
            "site-plan",
            format!("/content/seams/{i}/contact"),
            format!(
                "the seam for `{edge}` declares a `contact`, which requires dsl_version \
                 {since} and this stage declares `{version}` — raise this stage's \
                 `dsl_version` to {since}, or give the seam a standard `opening` instead \
                 (below {since} two places meet only through a doorway a table names).",
                edge = s.edge,
                since = crate::WAY_AND_CONTACT_SINCE,
                version = ctx.version,
            ),
        ));
        return false;
    }

    // ---- Shape 4: the classes a contact may carry.
    //
    // `walk` and `drop` only. A rim falling to a lower court is a genuine broad
    // hand-off, so `drop` is in; `stair`, `barred` and `vision` are excluded
    // until a campaign brief demands one (spec-0053 §4, the falsifier re-armed).
    if !matches!(ctx.edge, Edge::Walk { .. } | Edge::Drop { .. }) {
        refuse(
            format!(
                "is a contact on a `{class}` connection, and a contact carries `walk` or \
                 `drop` only",
                class = ctx.edge.class(),
            ),
            "give the seam a standard `opening`, or declare the connection `walk` or \
             `drop` in the layout graph. A stair needs a run and a pitch, a barred door \
             needs a gate region that seals and clears, and a sightline is not a crossing \
             at all — none of the three is a thing a front can be, and this engine does \
             not have them as contacts until a campaign brief demands one"
                .to_string(),
        );
        return false;
    }

    // ---- Shape 3: wider than the broadest standard opening.
    //
    // The floor is derived from the standard set, so anything at or under it
    // could have been a portal. That is what makes it a demand the defect
    // cannot supply: a door declared a contact is refused by its own width.
    let Some(floor) = table.broadest_opening_width(reads) else {
        return false; // `Metrics::self_check` reports a table with no openings.
    };
    let (u_span, v_span) = (ctx.face.u, ctx.face.v);
    let (u_hi, v_hi) = crossing_hi(ctx);
    let width = u_hi - s.at[0] + 1;
    if width <= i64::from(floor) {
        refuse(
            format!(
                "is a contact {width} cell(s) wide, which is not wider than the broadest \
                 standard opening ({floor} cells)"
            ),
            format!(
                "widen the span, or declare it a portal — anything this narrow could have \
                 been one, and a doorway called a contact would dodge the standard set \
                 while every downstream door check went on being wrong about it. Defined \
                 openings: {names}",
                names = table.names_of(MetricKind::Opening).join(", "),
            ),
        );
        return false;
    }

    // ---- Shape 2: the span lies on the shared face.
    let mut off: Vec<String> = Vec::new();
    if s.at[0] < u_span.0 || u_hi > u_span.1 {
        off.push(format!(
            "{}..{} on {}, against the face's {}..{}",
            s.at[0], u_hi, ctx.face.u_axis, u_span.0, u_span.1
        ));
    }
    if s.at[1] < v_span.0 || v_hi > v_span.1 {
        off.push(format!(
            "{}..{} on {}, against the face's {}..{}",
            s.at[1], v_hi, ctx.face.v_axis, v_span.0, v_span.1
        ));
    }
    if !off.is_empty() {
        refuse(
            format!(
                "is a contact whose span leaves the face the two boxes share: {}",
                off.join("; ")
            ),
            "move `at` onto the shared face, or shorten `contact.extent` — the span is \
             where the derivation writes no wall, and a span running off the face would \
             ask it to open a wall that is not there. Omitting `contact.extent` runs the \
             span from `at` to the far edge of the face, which never leaves it"
                .to_string(),
        );
        return false;
    }
    true
}

/// The far corner of a seam's crossing rectangle, on the face's own two in-plane
/// axes — [`contact_extent`] resolved against the seam's own anchor, so this
/// rule and the derivation describe one rectangle.
fn crossing_hi(ctx: &SeamCtx<'_>) -> (i64, i64) {
    let e = contact_extent(ctx.seam, &ctx.face);
    (ctx.seam.at[0] + e[0] - 1, ctx.seam.at[1] + e[1] - 1)
}

/// `DW0828`'s anchor half and `DW0829`'s geometric half: the opening's cells are
/// cells the shared face has.
fn opening_fits(ctx: &SeamCtx<'_>, opening: crate::metrics::Opening, d: &mut Vec<Diagnostic>) {
    let (i, s, edge, face) = (ctx.index, ctx.seam, ctx.edge, &ctx.face);
    let anchor_in =
        s.at[0] >= face.u.0 && s.at[0] <= face.u.1 && s.at[1] >= face.v.0 && s.at[1] <= face.v.1;
    if !anchor_in {
        d.push(Diagnostic::error(
            DW_SEAM_NOT_SHARED,
            "site-plan",
            format!("/content/seams/{i}/at"),
            format!(
                "the seam for `{id}` is anchored at {ua} {u}, {va} {v}, which is not on the face \
                 `{an}` and `{bn}` share — that face runs {ua} {u0}..{u1} by {va} {v0}..{v1} in \
                 the plane at {plane}. `at` names the opening's low corner in the face's own two \
                 axes, so a corner off the face allocates the seam nowhere.",
                id = s.edge,
                an = edge.a(),
                bn = edge.b(),
                ua = face.u_axis,
                va = face.v_axis,
                u = s.at[0],
                v = s.at[1],
                u0 = face.u.0,
                u1 = face.u.1,
                v0 = face.v.0,
                v1 = face.v.1,
                plane = face.plane,
            ),
        ));
        return;
    }
    let u_hi = s.at[0] + i64::from(opening.width) - 1;
    let v_hi = s.at[1] + i64::from(opening.height) - 1;
    if u_hi <= face.u.1 && v_hi <= face.v.1 {
        return;
    }
    d.push(Diagnostic::error(
        DW_SEAM_OPENING,
        "site-plan",
        format!("/content/seams/{i}/opening"),
        format!(
            "the `{name}` opening ({w}x{h}) does not fit on the face `{an}` and `{bn}` share. \
             Anchored at {ua} {u}, {va} {v} it would run to {ua} {u_hi}, {va} {v_hi}, and the \
             shared face ends at {ua} {u1}, {va} {v1}. Move the anchor, choose a narrower \
             standard opening, or grow the overlap between the two boxes — the standard set is \
             the vocabulary, so the opening is never quietly cropped to fit.",
            name = s.opening.as_deref().unwrap_or_default(),
            w = opening.width,
            h = opening.height,
            an = edge.a(),
            bn = edge.b(),
            ua = face.u_axis,
            va = face.v_axis,
            u = s.at[0],
            v = s.at[1],
            u1 = face.u.1,
            v1 = face.v.1,
        ),
    ));
}

/// `DW0829`'s step-rule half: a body standing on the floor of a side it enters
/// from can get onto the sill.
fn sill(ctx: &SeamCtx<'_>, opening: crate::metrics::Opening, d: &mut Vec<Diagnostic>) {
    let (i, s, edge, a, b, face) = (ctx.index, ctx.seam, ctx.edge, ctx.a, ctx.b, &ctx.face);
    if face.v_axis != "y" {
        return; // a horizontal seam has no sill; the fall or the treads own it.
    }
    let sources: Vec<(&NodeId, &Placed<'_>)> = match edge.direction() {
        Some(crate::layout::Direction::AToB) => vec![(edge.a(), a)],
        Some(crate::layout::Direction::BToA) => vec![(edge.b(), b)],
        None => vec![(edge.a(), a), (edge.b(), b)],
    };
    let max_rise = MAX_JUMP_RISE_16 / crate::metrics::FULL_16;
    for (name, p) in sources {
        let rise = s.at[1] - p.floor;
        if rise <= max_rise {
            continue;
        }
        d.push(Diagnostic::error(
            DW_SEAM_OPENING,
            "site-plan",
            format!("/content/seams/{i}/at"),
            format!(
                "the seam for `{id}` has its sill at y {sill}, {rise} blocks over the floor of \
                 `{name}` at y {floor}, and a body reaches at most {max_rise} block(s) by \
                 jumping ({j}/16 of vanilla's apex). A body entering from `{name}` cannot get \
                 into the opening at all, so the connection the graph declares is not one. Drop \
                 the sill, or declare the connection a `stair` and let the treads carry the \
                 climb. (The opening is {w}x{h}.)",
                id = s.edge,
                sill = s.at[1],
                j = MAX_JUMP_RISE_16,
                floor = p.floor,
                w = opening.width,
                h = opening.height,
            ),
        ));
    }
}

/// `DW0830`: the stair the plan allocated can be built at a standard pitch,
/// inside the box the plan said hosts it.
fn stair(ctx: &SeamCtx<'_>, table: &Metrics, reads: &mut Reads, d: &mut Vec<Diagnostic>) {
    let (i, s, edge, a, b, face) = (ctx.index, ctx.seam, ctx.edge, ctx.a, ctx.b, &ctx.face);
    let rise = b.floor - a.floor;
    if rise == 0 {
        d.push(Diagnostic::error(
            DW_STAIR_PITCH,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "`{id}` is a stair, and `{an}` and `{bn}` are both on plane y {f} — so it climbs \
                 nothing. A stair's rise is not authored here: it is the difference between the \
                 two floors the plan has already chosen, which means a stair between two places \
                 at one level is a walk that has been called a stair. Move one floor, or declare \
                 the connection a `walk`.",
                id = s.edge,
                an = edge.a(),
                bn = edge.b(),
                f = a.floor,
            ),
        ));
        return;
    }
    let Some(host_id) = &s.stair_in else {
        return; // the missing declaration is reported by `seams`.
    };
    // **The treads stand in the LOWER place**, and that is geometry rather than
    // taste: a stair is a stack of courses rising off a walk plane, and the only
    // walk plane it can rise off is the lower of the two. Hosting it in the
    // upper place asks for a stack that starts at that place's floor and has to
    // reach a level *below* it, which is not a stair — it is a hole with treads
    // drawn in the air under it.
    //
    // Found by building. This code checked only that the host affords the RUN,
    // so a plan naming the upper place reached green at stage 4 and the
    // derivation then laid a mound on the wrong side of the opening; the
    // stage-5 observer caught it as a seam whose hole was still solid, which is
    // the right refusal for the wrong defect. `stair_in` stays authored rather
    // than derived because it says WHICH of the two footprints pays for the run
    // when both are candidates — but when only one can be, saying the other is
    // a refusal.
    let (low, high) = if b.floor > a.floor {
        (edge.a(), edge.b())
    } else {
        (edge.b(), edge.a())
    };
    if host_id == high {
        d.push(Diagnostic::error(
            DW_STAIR_PITCH,
            "site-plan",
            format!("/content/seams/{i}/stair_in"),
            format!(
                "the stair for `{id}` hosts its treads in `{high}`, which is the HIGHER of the two \
                 places (`{an}` stands at y {af}, `{bn}` at y {bf}). Treads rise off a walk plane, \
                 and the only plane this stair can rise off is the lower one — massing in the \
                 upper place would have to start at that place's floor and reach a level beneath \
                 it, which is not a stair. Host it in `{low}`, and check that `{low}` affords the \
                 run: a stair costs its footprint, and moving the host moves who pays.",
                id = s.edge,
                an = edge.a(),
                bn = edge.b(),
                af = a.floor,
                bf = b.floor,
            ),
        ));
        return;
    }
    let host = if host_id == edge.a() { a } else { b };
    // The run a stair needs is horizontal. Across a vertical face it is spent
    // along that face's normal; through a floor or ceiling it may run either
    // way, so the host's longer horizontal axis is what it has.
    let (available, run_axis) = if face.v_axis == "y" {
        match s.face {
            Face::East | Face::West => (i64::from(host.plan.extent[0].get()), "x"),
            _ => (i64::from(host.plan.extent[1].get()), "z"),
        }
    } else {
        let (ex, ez) = (
            i64::from(host.plan.extent[0].get()),
            i64::from(host.plan.extent[1].get()),
        );
        if ex >= ez { (ex, "x") } else { (ez, "z") }
    };

    let mut best: Option<(&'static str, i64)> = None;
    for name in table.names_of(MetricKind::Pitch) {
        let Ok(entry) = table.resolve(MetricKind::Pitch, name) else {
            continue;
        };
        let MetricValue::Pitch(p) = entry.value(reads) else {
            continue;
        };
        if p.rise == 0 {
            continue;
        }
        let (span, per) = (rise.abs() * i64::from(p.run), i64::from(p.rise));
        let needed = span / per + i64::from(span % per != 0);
        if needed <= available {
            return; // some standard pitch fits.
        }
        if best.is_none_or(|(_, b)| needed < b) {
            best = Some((name, needed));
        }
    }
    let Some((name, needed)) = best else {
        return; // the table defines no pitch; `Metrics::self_check` owns that.
    };
    d.push(Diagnostic::error(
        DW_STAIR_PITCH,
        "site-plan",
        format!("/content/seams/{i}"),
        format!(
            "the stair for `{id}` climbs {rise} block(s) between `{an}` (floor {af}) and `{bn}` \
             (floor {bf}), and no standard pitch fits inside `{host_id}`. The gentlest fit is \
             `{name}`, which needs {needed} block(s) of run, and `{host_id}` affords {available} \
             on {run_axis}. Give the host a longer footprint on that axis, host the stair in the \
             other place, or bring the two floors closer together — the pitches are standards, \
             so a steeper one is not on offer.",
            id = s.edge,
            an = edge.a(),
            bn = edge.b(),
            af = a.floor,
            bf = b.floor,
        ),
    ));
}

/// `DW0831`: a designed drop falls the way it says it falls, and no further than
/// the policy allows.
fn drop_seam(
    ctx: &SeamCtx<'_>,
    falls: crate::layout::Direction,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) {
    let (i, s, edge, a, b) = (ctx.index, ctx.seam, ctx.edge, ctx.a, ctx.b);
    let (from, from_p, to, to_p) = match falls {
        crate::layout::Direction::AToB => (edge.a(), a, edge.b(), b),
        crate::layout::Direction::BToA => (edge.b(), b, edge.a(), a),
    };
    let depth = from_p.floor - to_p.floor;
    if depth <= 0 {
        d.push(Diagnostic::error(
            DW_DROP_POLICY,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "`{id}` falls from `{from}` (floor {ff}) into `{to}` (floor {tf}), which is \
                 {what}. A drop is one-way because a body cannot climb back up the way it came, \
                 and that is only true going down — this one is a mislabelled stair. Swap the \
                 declared direction, move the floors, or declare the connection a `stair`.",
                id = s.edge,
                ff = from_p.floor,
                tf = to_p.floor,
                what = if depth == 0 {
                    "the same plane".to_string()
                } else {
                    format!("{} block(s) HIGHER", -depth)
                },
            ),
        ));
        return;
    }
    let Some(cap) = table.max_designed_drop_blocks(reads) else {
        return;
    };
    if depth <= i64::from(cap) {
        return;
    }
    d.push(Diagnostic::error(
        DW_DROP_POLICY,
        "site-plan",
        format!("/content/seams/{i}"),
        format!(
            "`{id}` drops {depth} blocks from `{from}` into `{to}`, and the designed-drop policy \
             caps a declared fall at {cap}. This is a **policy** cap and it is deliberately far \
             tighter than what a body survives: a drop is a decision about the shape of the map, \
             and it should not also be a decision about the party's health. Bring the two floors \
             closer, or break the fall with a place between them.",
            id = s.edge,
        ),
    ));
}

// ---------------------------------------------------------------------------
// Identities: the plan held to the brief's own numbers
// ---------------------------------------------------------------------------

/// What one measure came out at, or why it could not be taken.
enum Measured {
    Value(f64),
    Unresolved(Diagnostic),
}

/// `DW0833` and `DW0834`: the brief's numbers still hold once the boxes are
/// drawn, and the binding that holds them is not empty.
///
/// `DW0834` is a **warning**, not an error, and the difference is deliberate: a
/// deliberately minimal plan — a fixture, a first sketch — is a legitimate
/// thing to hold, and refusing it would make the smallest useful document
/// uncompilable. What it may not be is *silent*, so the empty side is named on
/// every run and is a finding for the round summary.
///
/// `DW0833` runs here over the plan. **Its second call site is the built
/// world**, where the same rule recomputes the same measures from assembled
/// bytes so that a derivation defect which moved a datum cannot hide behind a
/// plan-time green. That site belongs to the round that builds the blockout;
/// nothing here approximates it.
fn identities(
    c: &Campaign,
    plan: &SitePlanContent,
    placed: &[Placed<'_>],
    d: &mut Vec<Diagnostic>,
) {
    let facts: BTreeMap<&str, &crate::layout::BriefFact> = c
        .geometry_brief
        .as_ref()
        .map(|b| {
            b.content
                .facts
                .iter()
                .map(|f| (f.id.0.as_str(), f))
                .collect()
        })
        .unwrap_or_default();

    if facts.is_empty() || plan.identities.is_empty() {
        let empty = match (facts.is_empty(), plan.identities.is_empty()) {
            (true, true) => "the brief states no fact and the plan declares no identity",
            (true, false) => "the brief states no fact",
            _ => "the plan declares no identity",
        };
        d.push(Diagnostic::warning(
            DW_IDENTITY_EMPTY,
            "site-plan",
            "/content/identities",
            format!(
                "the identity gate binds nothing: {empty}. This is what holds the whole map to \
                 the design somebody wrote down — with either side empty, the plan may say \
                 anything at all and every check above will still pass, because none of them \
                 has an opinion about how big the map was meant to be. It is a warning rather \
                 than a refusal so that a deliberately minimal plan stays compilable; it is \
                 printed every run so that the emptiness is never quietly a pass."
            ),
        ));
    }

    let by_node: BTreeMap<&str, &Placed<'_>> =
        placed.iter().map(|p| (p.plan.node.0.as_str(), p)).collect();
    let datums: BTreeMap<&str, i64> = plan.datums.iter().map(|x| (x.id.0.as_str(), x.y)).collect();

    for (i, id) in plan.identities.iter().enumerate() {
        let Some(fact) = facts.get(id.fact.0.as_str()) else {
            d.push(Diagnostic::error(
                DW_PLAN_AGREEMENT,
                "site-plan",
                format!("/content/identities/{i}/fact"),
                format!(
                    "this identity holds the map to `{f}`, which the geometry brief states no \
                     fact for. An identity binds to a number the brief WROTE DOWN — that is what \
                     makes it a design being kept rather than an assertion the plan makes about \
                     itself.",
                    f = id.fact,
                ),
            ));
            continue;
        };
        let measured = measure(&id.measure, plan, &by_node, &datums, i);
        let value = match measured {
            Measured::Value(v) => v,
            Measured::Unresolved(diag) => {
                d.push(diag);
                continue;
            }
        };
        if id.cmp.holds(value, fact.value) {
            continue;
        }
        d.push(Diagnostic::error(
            DW_IDENTITY_FALSE,
            "site-plan",
            format!("/content/identities/{i}"),
            format!(
                "the plan does not keep `{f}`: {what} measures {value}, and the brief asks for \
                 {cmp} {want}{unit}. The brief's sentence was: \"{note}\". Either move the \
                 geometry until the number is true, or change the brief's fact — in the brief, \
                 where the design is written down, so that the change is a decision somebody \
                 took rather than a plan that drifted.",
                f = id.fact,
                what = describe(&id.measure),
                cmp = id.cmp.as_str(),
                want = fact.value,
                unit = fact
                    .unit
                    .as_ref()
                    .map(|u| format!(" {u}"))
                    .unwrap_or_default(),
                note = fact.note,
            ),
        ));
    }
}

/// Take one measure off the plan.
fn measure(
    m: &Measure,
    plan: &SitePlanContent,
    by_node: &BTreeMap<&str, &Placed<'_>>,
    datums: &BTreeMap<&str, i64>,
    i: usize,
) -> Measured {
    let missing_node = |node: &NodeId| {
        Measured::Unresolved(Diagnostic::error(
            DW_PLAN_AGREEMENT,
            "site-plan",
            format!("/content/identities/{i}/measure"),
            format!(
                "this identity measures `{node}`, which this plan embeds no box for. A measure \
                 is taken off the geometry, so it can only name a place the plan actually put \
                 somewhere."
            ),
        ))
    };
    match m {
        Measure::RegionExtent { axis } => {
            Measured::Value(f64::from(plan.region.extent[axis.index()].get()))
        }
        Measure::BoxExtent { node, axis } => match by_node.get(node.0.as_str()) {
            Some(p) => Measured::Value(f64::from(p.plan.extent[axis.index()].get())),
            None => missing_node(node),
        },
        Measure::BoxHeight { node } => match by_node.get(node.0.as_str()) {
            Some(p) => match p.clearance {
                Some(c) => Measured::Value(f64::from(c)),
                None => Measured::Unresolved(Diagnostic::error(
                    DW_PLAN_AGREEMENT,
                    "site-plan",
                    format!("/content/identities/{i}/measure"),
                    format!(
                        "this identity measures the height of `{node}`, which is sky-open and \
                         whose size class did not resolve — so the plan states no headroom for \
                         it at all. Fix the class name the layout graph declares (`DW0812` names \
                         it) and the height becomes the class's own minimum."
                    ),
                )),
            },
            None => missing_node(node),
        },
        Measure::DistanceXz { from, to } => {
            let (Some(a), Some(b)) = (by_node.get(from.0.as_str()), by_node.get(to.0.as_str()))
            else {
                return missing_node(if by_node.contains_key(from.0.as_str()) {
                    to
                } else {
                    from
                });
            };
            let (ax, az) = a.centre_xz();
            let (bx, bz) = b.centre_xz();
            Measured::Value(((bx - ax).powi(2) + (bz - az).powi(2)).sqrt())
        }
        Measure::DatumY { datum } => match datums.get(datum.0.as_str()) {
            Some(y) => Measured::Value(*y as f64),
            None => Measured::Unresolved(Diagnostic::error(
                crate::codes::DANGLING_REF,
                "site-plan",
                format!("/content/identities/{i}/measure"),
                format!(
                    "this identity measures `{datum}`, which this plan declares no `datums[]` \
                     entry for."
                ),
            )),
        },
    }
}

/// What a measure is, in a refusal's own words.
fn describe(m: &Measure) -> String {
    match m {
        Measure::RegionExtent { axis } => {
            format!("the region's extent on {}", axis.as_str())
        }
        Measure::BoxExtent { node, axis } => {
            format!("`{node}`'s footprint on {}", axis.as_str())
        }
        Measure::BoxHeight { node } => format!("`{node}`'s headroom"),
        Measure::DistanceXz { from, to } => {
            format!("the horizontal distance from `{from}` to `{to}`")
        }
        Measure::DatumY { datum } => format!("the plane `{datum}`"),
    }
}

impl Axis {
    fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }
}

impl PlanAxis {
    fn index(self) -> usize {
        match self {
            PlanAxis::X => 0,
            PlanAxis::Z => 1,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            PlanAxis::X => "x",
            PlanAxis::Z => "z",
        }
    }
}

impl VolumeRole {
    /// The keyword a refusal prints.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            VolumeRole::Massif => "massif",
            VolumeRole::Ground => "ground",
            VolumeRole::Clearance => "clearance",
        }
    }
}

/// The plan's one lighting setting, range-checked exactly as an area's is — the
/// same code, because it is the same object being asked the same question.
fn lighting(plan: &SitePlanContent, d: &mut Vec<Diagnostic>) {
    let Some(l) = &plan.lighting else { return };
    if (1..=14).contains(&l.min_light) {
        return;
    }
    d.push(Diagnostic::error(
        crate::codes::LIGHTING_RANGE,
        "site-plan",
        "/content/lighting/min_light",
        format!(
            "`lighting.min_light` = {} is out of range — set it to a value in 1..=14 (7 is the \
             default)",
            l.min_light
        ),
    ));
}

/// The floor a designed opening may never be chosen below: the cells a standing
/// body needs to pass at all.
///
/// Not a check of its own — [`Metrics::self_check`] already holds every standard
/// opening over it, and a second refusal here would be this module re-asking a
/// question the table has already answered about itself. It is re-exported so a
/// reader of `DW0829` can see what the standard set is bounded by.
#[must_use]
pub fn passable_opening_cells() -> (u32, u32) {
    (passable_width_cells(), passable_clearance_cells())
}
