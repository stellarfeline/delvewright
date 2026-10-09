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
//! * The plan's `lighting` is [`crate::AreaLighting`], the engine's
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

use crate::AreaLighting;
use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::ids::{DatumId, EdgeId, FactId, NodeId, ViewId, VolumeId};
use crate::layout::{Edge, LayoutGraphContent, StationKind};
use crate::metrics::{
    MAX_JUMP_RISE_16, MetricKind, MetricValue, Metrics, Pitch, Reads, SizeClass, WayClass,
    passable_clearance_cells, passable_width_cells,
};

mod check;
mod claim;
mod fillcheck;
mod ground;
mod measure;
mod pack;
mod place;
mod region;
mod seam;

pub use check::*;
pub use claim::*;
pub use ground::*;
use measure::*;
pub use pack::*;
pub use place::*;
use region::*;
pub use seam::*;

crate::dw_code! {
    /// `DW0824`: the graph and the plan do not agree exactly.
    pub const DW_PLAN_AGREEMENT: DwCode = DwCode::new("DW0824", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0826`: a box leaves the region.
    pub const DW_BOX_LEAVES_REGION: DwCode = DwCode::new("DW0826", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0827`: two boxes overlap.
    pub const DW_BOXES_OVERLAP: DwCode = DwCode::new("DW0827", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0828`: a seam is not on a shared face.
    pub const DW_SEAM_NOT_SHARED: DwCode = DwCode::new("DW0828", ExitTier::Build);
}

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

crate::dw_code! {
    /// `DW0829`: a seam's opening is not a standard, or does not fit.
    pub const DW_SEAM_OPENING: DwCode = DwCode::new("DW0829", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0992`: a climb that climbs nothing (spec-0098 §2c).
    pub const DW_CLIMB_RISES_NOTHING: DwCode = DwCode::new("DW0992", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0830`: a stair seam cannot be built at standard pitch.
    pub const DW_STAIR_PITCH: DwCode = DwCode::new("DW0830", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0831`: a drop seam falls the wrong way, past the survivable fall, or
    /// past the plan's declared `max_drop`.
    pub const DW_DROP_POLICY: DwCode = DwCode::new("DW0831", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0876`: a seam does not declare a connection this engine builds
    /// (spec-0053 §6).
    ///
    /// **One code, three shapes of one claim** — the claim being that this seam
    /// states a crossing the derivation can build and the observer can measure:
    ///
    /// 1. it declares neither an `opening` nor a `contact`, or both;
    /// 2. its contact's span leaves the shared face `DW0828` established;
    /// 3. it is a contact on a `stair`, `barred` or `vision` connection.
    ///
    /// They are one code rather than three because the author's next action is
    /// the same in every case — say which kind of hand-off this is and give it a
    /// shape the engine has — and because a seam exhibiting one of them has no
    /// crossing for any rule below to judge. A contact's width is the author's:
    /// a front one cell wide is as legal as one fifty-five wide.
    pub const DW_CONTACT: DwCode = DwCode::new("DW0876", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0833`: a brief identity does not hold.
    pub const DW_IDENTITY_FALSE: DwCode = DwCode::new("DW0833", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0834`: the identity gate binds nothing. Warning — see [`identities`].
    pub const DW_IDENTITY_EMPTY: DwCode = DwCode::new("DW0834", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0835`: a whole-owned volume enters a box.
    pub const DW_VOLUME_IN_BOX: DwCode = DwCode::new("DW0835", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0839`: two placement authorities in one campaign — a `site-plan.json` and
    /// a non-empty `areas[]` both present.
    pub const DW_TWO_AUTHORITIES: DwCode = DwCode::new("DW0839", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0988`: a roof the plan has no room for (spec-0098 §7) — declared on
    /// a sky-open box, or rising into another place.
    pub const DW_ROOF_NO_ROOM: DwCode = DwCode::new("DW0988", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0883`: a box is not placed exactly once (spec-0059 §5). Two shapes of one
    /// claim: a connected component of the seam graph in which no box is pinned, so
    /// nothing places it; and a pinned box the packing also reaches, at a different
    /// corner, so two things place it.
    pub const DW_UNPLACED: DwCode = DwCode::new("DW0883", ExitTier::Build);
}

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
/// stands that affordance in; and each `barred` seam's gate region
/// (`anchor/seam-…`) whose plane this place owns (spec-0098 §2), because the
/// piece that owns a plane ships the gate standing in it.
///
/// Here rather than in `crate::detailplan` because the answer is a fact about
/// the graph, and [`synthesized_anchors`] is the one authority for what a
/// site-plan campaign provides — a second module deciding which names belong to
/// which place is exactly the two-functions-agreeing-about-spelling drift that
/// note exists to remove.
///
/// `crates/delvec/tests/blockout.rs`'s
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
    // Scenery (`reached: false`) owes no place to stand: nothing visits it
    // (spec-0098 §14).
    if n.reached {
        out.insert(node_anchor(node));
    }
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
    // The gate region over a `barred` seam is owed by the place that owns the
    // plane it stands in (spec-0098 §2): every seam lies in a plane some place
    // owns, so its shut state is that piece's to ship.
    let resolved = SitePlan::of(c);
    let site = resolved.site();
    for s in &resolved.seams {
        if s.class != "barred" || (&s.a != node && &s.b != node) {
            continue;
        }
        if site.owner(s.opening.0) == Owner::Place(node.clone()) {
            out.insert(seam_anchor(&s.edge));
        }
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
    /// **What every cell no place claims and no volume covers becomes**
    /// (spec-0098 §2b): `solid` rock for an enclosed site (a dungeon, a cave),
    /// `open` ground under sky over a declared terrain for an open one (a town).
    ///
    /// Required, with no default: either default would be a judgement about
    /// what kind of site this is, and that judgement is the author's. Per
    /// region it is overridden by `volumes[]`, exactly as before.
    pub fill: Fill,
    /// **The deepest fall a designed drop in this plan may take**, in blocks —
    /// the author's own policy, when they have one (`DW0831` confirms every
    /// drop seam falls no further). Absent, no policy cap applies; the
    /// survivable fall of an unarmoured body holds every drop either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_drop: Option<NonZeroU32>,
}

/// What undeclared space becomes (spec-0098 §2b).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Fill {
    /// Every unclaimed cell holds `block` — the enclosed site, whose places are
    /// carved out of rock.
    Solid {
        /// The block state the rock is.
        block: String,
    },
    /// A natural ground surface under sky: at the terrain's height the
    /// `surface` block, under it `below`, above it air.
    Open {
        /// The ground's shape: one flat height, or a heightmap.
        terrain: Terrain,
        /// The block state of the top course.
        surface: String,
        /// The block state under the top course.
        below: String,
    },
}

/// **The site's terrain: a declared heightfield** (spec-0098 §2c).
///
/// Every height here is the `y` of the **surface block** — the ground's top
/// cell — so a body walks one above it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Terrain {
    /// One height everywhere: the named datum is the terrain's walk plane, so
    /// the surface block stands at the datum's `y − 1` — exactly where a box
    /// standing on that datum has its floor course.
    Flat {
        /// The plane (`DW0112` if the plan declares no such datum).
        datum: DatumId,
    },
    /// A greyscale image exactly the region's `x × z` pixels, in the campaign:
    /// pixel `(px, pz)` is column `(region.min.x + px, region.min.z + pz)`,
    /// and its surface block stands at `base_y + value × range / 255` (integer
    /// division). The creator's own artifact — drawn, or generated by a tool
    /// whose seed is written down.
    Heightmap {
        /// The image's campaign-relative path.
        heightmap: String,
        /// The surface `y` a black pixel stands for.
        base_y: i64,
        /// How many blocks a white pixel stands above `base_y`.
        range: u32,
    },
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
/// like they say: `extent` is the play space the author declared; and a plan
/// never
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
    /// **A pin**: the low corner `[x, z]` in world coordinates, when the author
    /// chooses where this box stands. Optional (spec-0059 §2): a box with no pin
    /// stands where the packing puts it — one cell beyond the face of the box
    /// its first seam in document order hangs it off. At least one box of every
    /// connected component of the seam graph is pinned, or nothing places the
    /// component (`DW0883`); a pinned box the packing also reaches at a
    /// different corner is refused naming both (`DW0883`). Two horizontal
    /// numbers, never three — the vertical position is `floor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<[i64; 2]>,
    /// Interior footprint `[dx, dz]`, in blocks — any whole number on either axis.
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
    /// **The sky this place stands under from the first tick** (spec-0080
    /// §3.2): one of `world.atmospheres[]`, painted at world setup over the
    /// box's play space grown as far as the client's biome blend reads — the
    /// same capability `areas[].atmosphere` is, on the
    /// other class of place with a world box. Absent: the horizon's biome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<crate::ids::AtmosphereId>,
    /// **The roof the whole reserves over this place** (spec-0098 §3): how
    /// many courses it rises above the ceiling course and how far it overhangs
    /// the shell on each horizontal side. Massed solid at stage 5 so a walker
    /// sees the volume the building will take; drawn by the place's own piece
    /// once detailed. Absent: a flat lid one course thick, which the piece owns
    /// too. Refused on a sky-open box, and where its courses rise into another
    /// place (`DW0988`); its eaves stop at a neighbour's wall.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<Roof>,
}

/// The roof a roofed place carries above its lid (spec-0098 §3).
///
/// Both numbers are judgements the plan states against the research record
/// (`docs/reference/roof-and-facade-craft.md`): a 45° gable over a roof span of
/// `W` cells rises `⌈(W − 1) / 2⌉` courses, and an eave of one cell is the
/// idiom. Neither is inferred by the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Roof {
    /// Courses the roof rises above the ceiling course. `0` is a flat roof whose
    /// only course is the lid.
    pub courses: u32,
    /// Cells the roof zone overhangs the shell footprint on every horizontal
    /// side, from the ceiling course up. `0` is a roof flush with the walls.
    pub eaves: u32,
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

/// A crossing's position on one box's face, from that box's own low corner.
///
/// One integer on a wall face (cells along the face's horizontal axis), a pair
/// through a floor or ceiling (cells along `x` and `z`). Untagged, so the
/// document writes `"at": 2` or `"at": [2, 3]`; the face decides which shape is
/// a position and the other is `DW0828`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Offset {
    /// Cells along a vertical face's horizontal axis.
    Along(i64),
    /// Cells along `x` and `z` on a horizontal face.
    Plane([i64; 2]),
}

/// One traversal edge, allocated: an opening on a face the two boxes share.
///
/// The seam carries **no rise** and **no sill**. A rise is `floor(b) − floor(a)`, which the plan
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
    /// Where the crossing sits on **`a`'s** face: an offset from `a`'s own low
    /// corner along the face, never a world coordinate (spec-0059 §2).
    ///
    /// * on a vertical face (`east`/`west`/`north`/`south`) — one integer, cells
    ///   along the face's horizontal axis (`z` for east/west, `x` for
    ///   north/south). The **sill is not written**: it is
    ///   `max(floor(a), floor(b))`, which the plan has already stated.
    /// * on a horizontal face (`up`/`down`) — `[dx, dz]`, cells along `x` and
    ///   `z`.
    ///
    /// Omitted, the crossing is **centred** on the face: `(extent - width) div
    /// 2`. An offset that leaves the face, or of the wrong shape for the face,
    /// is `DW0828`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Offset>,
    /// Where the same crossing sits on **`b`'s** face — the same offset, taken
    /// from `b`'s own low corner. Omitted, centred on `b`'s face. The packing
    /// places `b` so that the two agree: `corner(b) = corner(a) + at - meets`
    /// along the face (spec-0059 §3). On a seam whose two boxes both already
    /// stand, the two must name the same cells (`DW0828`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meets: Option<Offset>,
    /// **A PORTAL**: a named opening from the metrics table's standard set
    /// (`DW0812` on a name the table does not define), or a size the seam
    /// declares itself, `{"width": w, "height": h}` — the author's own opening,
    /// a rope bridge's end one cell wide included. `DW0829` confirms either fits
    /// the shared face. A body crosses at exactly the cells `at` and the opening
    /// allocate.
    ///
    /// **Exactly one of this and [`Seam::contact`]** (`DW0876`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<OpeningSpec>,
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
    /// **What the crossing is, in a few words** (spec-0098 §2c) — "a wooden
    /// arch bridge, 3 wide", "a stone stair, down 4". A creative judgement the
    /// plan states once and `delvec allocation` hands to BOTH places the seam
    /// joins, so each designs its side of the interface knowing what meets it.
    /// Never player-facing, so never translated.
    pub form: String,
}

/// **A portal's opening**: a named standard, or a size the seam declares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OpeningSpec {
    /// A named standard from the metrics table's opening set.
    Named(String),
    /// A size the seam declares itself.
    Declared(DeclaredOpening),
}

/// An opening the seam sizes itself: cells on the face's own two in-plane
/// axes — along the wall and up it on a vertical face, `x` and `z` through a
/// floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeclaredOpening {
    /// Clear width, in cells.
    pub width: NonZeroU32,
    /// Clear height, in cells.
    pub height: NonZeroU32,
}

impl OpeningSpec {
    /// The opening's size: the named standard's, through [`Metrics::resolve`]
    /// (the one path from a name to an entry), or the declared one.
    ///
    /// # Errors
    ///
    /// [`crate::metrics::UnknownMetric`] for a name the table does not define,
    /// which the caller turns into `DW0812`.
    pub fn resolve(
        &self,
        table: &Metrics,
        reads: &mut Reads,
    ) -> Result<crate::metrics::Opening, crate::metrics::UnknownMetric> {
        match self {
            OpeningSpec::Named(name) => {
                let entry = table.resolve(MetricKind::Opening, name)?;
                match entry.value(reads) {
                    MetricValue::Opening(o) => Ok(*o),
                    _ => unreachable!("an opening entry carries an opening"),
                }
            }
            OpeningSpec::Declared(o) => Ok(crate::metrics::Opening {
                width: o.width.get(),
                height: o.height.get(),
            }),
        }
    }

    /// How the opening reads in a message: the standard's name, or `WxH`.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            OpeningSpec::Named(name) => format!("`{name}`"),
            OpeningSpec::Declared(o) => format!("declared {}x{}", o.width, o.height),
        }
    }
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
    /// The block state this volume is made of. Absent, the block of its kind
    /// comes from the plan's `fill` (spec-0098 §2b): a `massif` is the
    /// `solid` fill's block, or the `open` fill's `below`; a `ground` is the
    /// `open` fill's `surface` over `below`, or the `solid` fill's block. A
    /// `clearance` is air and names no block (`DW0193` if it does).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
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
