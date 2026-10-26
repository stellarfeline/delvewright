//! **Space is a graph before it is a coordinate** (spec-0049 §3, §4.2) —
//! pipeline stages 2 and 3.
//!
//! Two campaign stage documents land here, and neither holds a single
//! coordinate:
//!
//! * **`geometry-brief`** — the machine-readable half of the whole map's written
//!   brief: `facts[]`, each a number with a name. Stage 4's identity checks bind
//!   a site plan to these (spec-0049 §4.2); this round lands the document and
//!   nothing reads it yet, which is stated rather than implied.
//! * **`layout-graph`** — the campaign's space as a graph: places, the
//!   connections between them, the authored critical path, and where each quest
//!   beat happens. The topology carries the global guarantees and is checked as
//!   an object of its own, cheaply, **before geometry exists to make it
//!   expensive**.
//!
//! # What makes the ordering structural rather than prose
//!
//! spec-0049 §7 enumerates four inversions the design must make uncompilable.
//! The one this module owns is *graph before mission*, and §7 is honest that it
//! is **representable**: a graph with no `beats[]` and no gating validates
//! against quest documents it never references. What it is not is silently
//! green. Two teeth, both here:
//!
//! 1. `DW0817` states its beat binding, and a **zero** beat binding is printed
//!    as a zero — a critical path over an unbound graph is a route through
//!    nothing and says so.
//! 2. `DW0818`'s **reverse** direction: the moment quests exist, every
//!    place-bound beat must bind to a node. A graph is free to arrive before the
//!    mission; it is not free to arrive after it and ignore it.
//!
//! # The gating vocabulary is deliberately NOT [`crate::gate::Gate`]
//!
//! An edge's [`EdgeGating`] names flags and a quest, which is what the campaign's
//! one gate names too — so the question is owed an answer rather than a
//! convention. A [`Gate`](crate::gate::Gate) is a **runtime** object: emission
//! evaluates it against an acting player, and
//! [`GateConsumer::evaluates_per_player`](crate::gate::GateConsumer) is what makes
//! a `player`-scoped datum's readability decidable. A layout-graph edge is
//! evaluated by nothing at run time — the graph emits no command, no function and
//! no scoreboard — so making it a gate consumer would push a never-emitted object
//! into machinery whose whole subject is emission, and every proof written about
//! "the gates of this campaign" would then be reasoning about a claim instead of
//! about a thing the server runs.
//!
//! It is also narrower on purpose. The closure below is **monotone** (§3.2), so a
//! negative flag term and a numeric comparison are terms it cannot decide; a
//! surface an author may write and no proof honours is worse than one that is not
//! there. What an edge states is therefore a *projection* of the campaign's
//! runtime gating into topology — and `DW0818` is what keeps it a projection
//! rather than a second vocabulary: every flag it names must be one some effect
//! really sets, and every quest it names must exist.
//!
//! Determinism (ADR-0006): every set and map here is a `BTreeSet`/`BTreeMap` and
//! every walk is over a slice in document order.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Objective;
use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::ids::{AnchorId, EdgeId, FactId, FlagId, NodeId, ObjectiveId, QuestId};
use crate::metrics::{MetricKind, Metrics, Reads};

crate::dw_code! {
    /// `DW0814`: the layout graph is not a graph — a duplicate id, an endpoint
    /// naming no place, a self-loop, an `entry` that is not a node.
    pub const DW_GRAPH_MALFORMED: DwCode = DwCode::new("DW0814", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0816`: a node the closure never reaches.
    pub const DW_NODE_UNREACHED: DwCode = DwCode::new("DW0816", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0817`: the authored critical path does not hold.
    pub const DW_CRITICAL_PATH: DwCode = DwCode::new("DW0817", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0818`: the graph names quest-side state that does not exist, or a
    /// place-bound beat has no place.
    pub const DW_GRAPH_MISSION: DwCode = DwCode::new("DW0818", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0819`: a one-way edge strands.
    pub const DW_ONE_WAY_STRANDS: DwCode = DwCode::new("DW0819", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0820`: a shortcut closes no loop.
    pub const DW_SHORTCUT_NO_LOOP: DwCode = DwCode::new("DW0820", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0822`: the pacing measurement — a projection, printed with no threshold.
    pub const DW_PACING: DwCode = DwCode::new("DW0822", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0869`: a station takes a name in the engine's own namespace (spec-0052 §7.1).
    pub const DW_STATION_RESERVED: DwCode = DwCode::new("DW0869", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0870`: two stations claim one name (spec-0052 §7.2).
    pub const DW_STATION_DUPLICATE: DwCode = DwCode::new("DW0870", ExitTier::Build);
}

crate::dw_code! {
    /// `DW0871`: a reference demands a shape the station is not (spec-0052 §7.3).
    ///
    /// Judged at the reference site from the DECLARATION, with zero pieces bound.
    pub const DW_STATION_KIND: DwCode = DwCode::new("DW0871", ExitTier::Build);
}

// ---------------------------------------------------------------------------
// Stage 2 — the geometry brief's machine-readable facts (spec-0049 §4.2)
// ---------------------------------------------------------------------------

/// The `geometry-brief` stage document's payload.
///
/// The brief's prose stays prose; only what is stated as a **fact** is
/// checkable, and a site plan's `identities[]` bind to exactly these. The
/// reference imagery keeps its standing — style authority, rank-only, never a
/// gate (spec-0028) — so an identity binds to the written brief's numbers and
/// never to a picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeometryBriefContent {
    /// The brief's numbers, each with a name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<BriefFact>,
}

/// One fact from the whole map's written brief: a number with a name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BriefFact {
    /// Fact id (`fact/<kebab>`), unique within the brief.
    pub id: FactId,
    /// The number itself.
    pub value: f64,
    /// What the number counts (`blocks`, `storeys`, a ratio's `none`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The sentence of the brief this number came from, so a reader of the plan
    /// can see what the identity is holding the map to.
    pub note: String,
}

// ---------------------------------------------------------------------------
// Stage 3 — the layout graph (spec-0049 §3.1)
// ---------------------------------------------------------------------------

/// The `layout-graph` stage document's payload: the campaign's space as a graph,
/// stated before any coordinate exists.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutGraphContent {
    /// The places.
    pub nodes: Vec<Node>,
    /// The connections between them.
    pub edges: Vec<Edge>,
    /// Where a body starts.
    pub entry: NodeId,
    /// Where the campaign ends.
    pub goal: NodeId,
    /// The authored node sequence from `entry` to `goal`.
    ///
    /// **Authored rather than derived** so that it is a claim the machine
    /// verifies (`DW0817`) and the walk sheet can print. A derived path would be
    /// an answer with no author to disagree with.
    pub critical_path: Vec<NodeId>,
    /// Where each quest beat happens. Empty is legal and is the *graph before
    /// mission* case — stated as a zero binding rather than passed over.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beats: Vec<Beat>,
}

impl LayoutGraphContent {
    /// **The place an anchor belongs to**, read from the graph alone: the
    /// entry anchor is the entry's, a place's own `anchor/node-…` and each of
    /// its stations are that place's, and a gated connection's
    /// `anchor/unlock-…` is the place on its `gating.opens_from` side. The names are
    /// [`crate::siteplan::synthesized_anchor_kinds`]'s spelling. A seam's gate
    /// region stands between two places and belongs to neither; it and any name
    /// the graph does not spell yield `None`.
    #[must_use]
    pub fn place_of(&self, anchor: &str) -> Option<&NodeId> {
        if anchor == crate::siteplan::ENTRY_ANCHOR {
            return Some(&self.entry);
        }
        if let Some(n) = self.nodes.iter().find(|n| {
            (n.reached && crate::siteplan::node_anchor(&n.id) == anchor)
                || n.stations.iter().any(|s| s.anchor.as_str() == anchor)
        }) {
            return Some(&n.id);
        }
        self.edges.iter().find_map(|e| {
            if crate::siteplan::seam_unlock_anchor(e.id()) != anchor {
                return None;
            }
            e.opening_side()
        })
    }
}

/// A **place**: a room, a courtyard, an arena, a stretch of shore, a cavern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Node id (`node/<kebab>`), unique within the graph.
    pub id: NodeId,
    /// What this place is for, in the author's own words.
    ///
    /// A free non-empty label — `arena`, `hub`, `vista`, `gate-house`,
    /// `shortcut-landing` — that **no check keys on**. It is recorded judgement
    /// for the reviewer and for the later per-place briefs, and it is kept
    /// free-form deliberately: an enum of intents would be this month's genre
    /// wearing a schema's clothes.
    pub intent: String,
    /// Anything the reviewer needs that `intent` does not carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// **The named places inside this one** (spec-0052 §3).
    ///
    /// A site-plan campaign's anchor vocabulary is otherwise exactly the
    /// synthesized set — `spawn`, one anchor per node, and the seam and unlock
    /// anchors of gated connections — which is complete only for the authoring order
    /// it was designed for: quests written against nodes before any piece
    /// exists. The inverse order is real, and its quest layer names places
    /// *inside* places: the fire pit in the camp, the aft deck of the galley.
    /// Those names are the campaign's design, and flattening them onto box
    /// centres un-writes its spatial script.
    ///
    /// A station is a **name and a shape, never a position** (§5): while the box
    /// is massed the derivation realizes each station at a stand-in of its own,
    /// and when a piece is bound the `detail-plan` `anchors` map re-binds it to
    /// an anchor of that piece, which is where the name gets its real place.
    /// There is no coordinate, offset or hint field here, and that absence is
    /// the design rather than an omission.
    ///
    /// Declared names join the campaign vocabulary at the same authority as
    /// every synthesized one — [`crate::siteplan::synthesized_anchors`] is still
    /// the single place the question is answered, so a name that validates
    /// cannot fail to exist in the built world.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stations: Vec<Station>,
    /// **Whether a body reaches this place** (spec-0098 §14, a ruling).
    /// `false` declares the place scenery — a box built to be seen and never
    /// entered, a tree's crown over a treehouse — and the checks confirm that
    /// intent both ways: the closure must not reach it (`DW0816`) and no body
    /// may get into it in the built world (`DW0837`), exactly as a reached
    /// place must be reached. Absent means reached.
    #[serde(default = "reached_default", skip_serializing_if = "is_true")]
    pub reached: bool,
    /// **Human-built references this place is built from** (spec-0123 §3.3).
    ///
    /// Each row names a picture or a page of a real build of the same kind of
    /// place, what to take from it, and the terms its source states. A
    /// reference is looked at, never copied: an unlicensed or non-commercial
    /// source is ideas only (ADR-0013). None of the three strings is player
    /// facing and none is inventoried. A node declaring none is legitimate;
    /// validation counts them and refuses nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<PlaceReference>,
}

/// **One human-built reference** a place is built from (spec-0123 §3.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceReference {
    /// Where the reference is: an `https://` URL, to an image or a page.
    #[schemars(pattern(r"^https://\S+$"))]
    pub url: String,
    /// One sentence: what the reference shows, and what to take from it.
    #[schemars(length(min = 1))]
    pub shows: String,
    /// The terms the source states — `"CC BY-SA 4.0"`, `"none stated"`.
    #[schemars(length(min = 1))]
    pub terms: String,
}

fn reached_default() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// **A named place inside a node** (spec-0052 §3).
///
/// Belongs to exactly one node. A connection *between* places is the edge's to
/// declare, as ever — an interior gate station seals a volume inside its own
/// place and can never gate node-to-node traversal, so topology stays the
/// graph's structurally rather than by convention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Station {
    /// The anchor id (`anchor/<kebab>`) quests reference.
    ///
    /// Unique across the whole graph, and refused in the engine's own namespace
    /// — see [`DW_STATION_RESERVED`] and [`DW_STATION_DUPLICATE`].
    pub anchor: AnchorId,
    /// The station's **shape** — never its purpose.
    pub kind: StationKind,
    /// Recorded judgement for the reviewer and the later per-place detail
    /// brief. **No check keys on it**, for the same reason none keys on
    /// [`Node::intent`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// **What shape a station is** (spec-0052 §3) — a cell to stand a body at, or a
/// volume with a block that seals and clears.
///
/// The kind is the station's shape, never its purpose: there is no enum of
/// bonfire / camera / shop, for the same reason [`Node::intent`] is free-form —
/// a purpose vocabulary would be this month's genre wearing a schema's clothes.
/// A bonfire, a camera subject and a shop counter are the same [`Self::Point`]
/// to every check in this engine.
///
/// # Why two shapes and not three
///
/// spec-0052 §3 describes three, "mirroring the three shapes a piece anchor can
/// take (a cell to stand a body at; a volume; a volume with a block that seals
/// and clears)". Those are the three shapes a piece anchor may be **declared**
/// in; they are not three shapes anything **consumes**. This engine resolves an
/// anchor to `ResolvedAnchor::Point` or `ResolvedAnchor::Gate` and to nothing
/// else, and a piece anchor declaring a bare `region` with no `block` is read as
/// a gate whose fill block is `minecraft:air`.
///
/// Every volume-shaped consumer — a `lethal_volumes[]` region, `damage-players`'s
/// `in`, a `volley` kill zone, `collapse`'s ceiling, `begin-stealth`,
/// `fill-region`, `clear-region` — is a [`crate::StealthZone`], an
/// **anchor-centred box** resolved from a *point* plus an extent. The engine
/// states the reason in its own words at `Verb::FillRegion::region`: an
/// anchor-centred box rather than a prefab `region` anchor, "because the
/// assembled model deletes every gate-region anchor's cells, so a slab declared
/// that way would already be gone".
///
/// So a third `region` variant would be a name a campaign could declare and bind
/// and **no reference site could consume** — an inert surface whose stand-in
/// would have to be a gate region the assembled world then deletes. It is left
/// out deliberately, and spec-0052 §11's falsifier is what decides it: the first
/// campaign brief that cannot state its place without one is the evidence, and
/// the answer is a first-class surface with a consumer, or a refused feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StationKind {
    /// A cell to stand a body at — where a quest step, an NPC, a wave seat, a
    /// cutscene subject, an affordance or a volume's centre lands.
    Point,
    /// A volume with a block that seals and clears — what `open-gate`,
    /// `close-gate`, a `shortcut` and a `timed-gate` address.
    Gate,
}

impl StationKind {
    /// The word a diagnostic prints for this kind.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Gate => "gate",
        }
    }
}

/// Which way a one-way connection runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    /// From the edge's `a` end to its `b` end.
    AToB,
    /// From the edge's `b` end to its `a` end.
    BToA,
}

/// Which side of a gated connection can open it (spec-0107 §5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OpensFrom {
    /// Only from the `a` end.
    A,
    /// Only from the `b` end.
    B,
    /// From either end.
    #[default]
    Either,
}

impl OpensFrom {
    /// True for the default, which a canonical document leaves unwritten.
    #[must_use]
    pub fn is_either(&self) -> bool {
        matches!(self, OpensFrom::Either)
    }
}

/// What a body must already hold for a traversal connection to be passable —
/// and, because the world keeps the story's order, what the derivation builds
/// shut until the story opens it (spec-0107).
///
/// A **projection** of the campaign's runtime gating into topology, not the
/// campaign's [`Gate`](crate::gate::Gate) — see the module docs for why the two
/// are deliberately different objects, and for why this one carries no negative
/// flag term and no numeric comparison. A gating that names a term is built as
/// a bar in the connection's seam (`anchor/seam-<edge>`), and `opens_from` says
/// which side can open it: the field lives here because a side can only be
/// named for a way that shuts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EdgeGating {
    /// Flags a body must already have, each one some `set-flag` effect really
    /// produces (`DW0818`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagId>,
    /// A quest that must already be complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quest: Option<QuestId>,
    /// Which side can open the shut way: one-sided, the derivation stands
    /// `anchor/unlock-<edge>` on that side for a `shortcuts[]` affordance.
    /// Absent means either side.
    #[serde(default, skip_serializing_if = "OpensFrom::is_either")]
    pub opens_from: OpensFrom,
    /// How the player learns this way is shut (spec-0118): `seen` from the
    /// junction where its branch leaves the route, `told` by a pressable answer
    /// at its face, `rewarded` by a pickup, interaction or fight on its branch,
    /// or `hidden` — not at all, until the shot that opens it shows it. Owed
    /// by `DW1053` wherever a body meets the way shut or an eye sees it, and
    /// confirmed over the world by `DW1054`. Like `opens_from`, it can only be
    /// named for a way that shuts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned: Option<Learned>,
    /// **The words that would give a hidden way away** (spec-0118 §4): present
    /// exactly when `learned` is `hidden`, a non-empty list of non-empty words
    /// or phrases. No player-facing string the play order places before the
    /// shot that opens the way may mention one, and some string at or after it
    /// must (`DW1060`). The author's to know: never read from a place's name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secret: Vec<String>,
}

/// How the player learns a shut way is shut (spec-0118 §4): the four ways the
/// engine confirms, and no fifth.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Learned {
    /// The bar is the first thing the player's eye meets from the junction
    /// where the branch to it leaves the open route.
    Seen,
    /// A `use` trigger answers a press at the way's seam,
    /// `anchor/seam-<edge>`.
    Told,
    /// A pickup, interaction or fight stands on the branch's ground.
    Rewarded,
    /// The player does not learn of the way until the shot that opens it
    /// shows it: no eye in the party's reach sees it while it is shut, the
    /// beat that opens it carries a `cutscene` of its seam, and no text before
    /// that shot tells its `secret`.
    Hidden,
}

impl Learned {
    /// The word as the document spells it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Learned::Seen => "seen",
            Learned::Told => "told",
            Learned::Rewarded => "rewarded",
            Learned::Hidden => "hidden",
        }
    }
}

/// What a `carry` connection waits on before its link is armed (spec-0083 §7).
/// The terms of [`EdgeGating`] and nothing else: a carry crosses no seam, so it
/// has no bar and no side to open one from, and `opens_from` here is an unknown
/// field (`DW0100`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CarryGating {
    /// Flags a body must already have, each one some `set-flag` effect really
    /// produces (`DW0818`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagId>,
    /// A quest that must already be complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quest: Option<QuestId>,
}

/// The terms a connection demands, read the same way whichever class carries
/// them — what the closure, `DW0818` and every ledger read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GatingTerms<'a> {
    /// Flags a body must already have.
    pub flags: &'a [FlagId],
    /// A quest that must already be complete.
    pub quest: Option<&'a QuestId>,
}

impl GatingTerms<'_> {
    /// True if this gating demands nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.flags.is_empty() && self.quest.is_none()
    }

    /// How many terms it has — the number a binding ledger reports.
    #[must_use]
    pub fn terms(&self) -> usize {
        self.flags.len() + usize::from(self.quest.is_some())
    }

    /// The terms in words, for a diagnostic: `` `flag/x` and `quest/y` complete ``.
    #[must_use]
    pub fn words(&self) -> String {
        let mut terms: Vec<String> = self.flags.iter().map(|f| format!("`{f}`")).collect();
        if let Some(q) = self.quest {
            terms.push(format!("`{q}` complete"));
        }
        terms.join(" and ")
    }
}

impl EdgeGating {
    /// The terms this gating demands.
    #[must_use]
    pub fn terms(&self) -> GatingTerms<'_> {
        GatingTerms {
            flags: &self.flags,
            quest: self.quest.as_ref(),
        }
    }
}

impl CarryGating {
    /// The terms this gating demands.
    #[must_use]
    pub fn terms(&self) -> GatingTerms<'_> {
        GatingTerms {
            flags: &self.flags,
            quest: self.quest.as_ref(),
        }
    }
}

/// A connection between two places.
///
/// Internally tagged on `class`, in the same shape every other tagged union in
/// this DSL uses. The tagging is load-bearing rather than stylistic: `falls`
/// belongs to a drop, and writing it on a walk would be a declaration nothing
/// reads. Under the tag, serde's
/// `deny_unknown_fields` refuses it as an ordinary `DW0100` — so the illegal
/// state is unrepresentable and no diagnostic has to police it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "class", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Edge {
    /// A way a body walks, at grade.
    Walk {
        /// Edge id (`edge/<kebab>`), unique within the graph.
        id: EdgeId,
        /// One end.
        a: NodeId,
        /// The other end.
        b: NodeId,
        /// Declared directionality; absent means a body passes both ways.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        one_way: Option<Direction>,
        /// This connection exists to close a loop (`DW0820`).
        #[serde(default, skip_serializing_if = "is_false")]
        shortcut: bool,
        /// What a body must hold to pass.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gating: Option<EdgeGating>,
    },
    /// A way a body climbs or descends on built treads.
    Stair {
        /// Edge id (`edge/<kebab>`), unique within the graph.
        id: EdgeId,
        /// One end.
        a: NodeId,
        /// The other end.
        b: NodeId,
        /// Declared directionality; absent means a body passes both ways.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        one_way: Option<Direction>,
        /// This connection exists to close a loop (`DW0820`).
        #[serde(default, skip_serializing_if = "is_false")]
        shortcut: bool,
        /// What a body must hold to pass.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gating: Option<EdgeGating>,
    },
    /// A way a body climbs, on a ladder or a vine (spec-0099): a hole through
    /// a floor, or a door high in a wall, reached by a climbable the lower
    /// place hangs (spec-0098 §2c). No treads and no sill: the climb carries
    /// the rise, and the proofs that move a body count it.
    Climb {
        /// Edge id (`edge/<kebab>`), unique within the graph.
        id: EdgeId,
        /// One end.
        a: NodeId,
        /// The other end.
        b: NodeId,
        /// Declared directionality; absent means a body passes both ways.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        one_way: Option<Direction>,
        /// This connection exists to close a loop (`DW0820`).
        #[serde(default, skip_serializing_if = "is_false")]
        shortcut: bool,
        /// What a body must hold to pass.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gating: Option<EdgeGating>,
    },
    /// A fall. One-way **by construction**, which is why the direction is
    /// required here and optional on its siblings: a body that has dropped
    /// cannot climb back up the way it came.
    Drop {
        /// Edge id (`edge/<kebab>`), unique within the graph.
        id: EdgeId,
        /// One end.
        a: NodeId,
        /// The other end.
        b: NodeId,
        /// Which way it falls.
        falls: Direction,
        /// This connection exists to close a loop (`DW0820`).
        #[serde(default, skip_serializing_if = "is_false")]
        shortcut: bool,
        /// What a body must hold to pass.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gating: Option<EdgeGating>,
    },
    /// A connection a body is **carried** over (spec-0083 §7): no seam, no sill,
    /// no door check and no body of its own. The derivation writes nothing for
    /// it; at stage 5 every direction it allows is owed a link — a repeatable
    /// trigger whose `teleport` leaves the `a` place's station for the `b`
    /// place's mark — and every link is owed the edge (`DW0934`).
    Carry {
        /// Edge id (`edge/<kebab>`), unique within the graph.
        id: EdgeId,
        /// One end.
        a: NodeId,
        /// The other end.
        b: NodeId,
        /// Declared directionality; absent means a body is carried both ways,
        /// and each direction is then owed a link.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        one_way: Option<Direction>,
        /// What carries it. **Required to say something** (`DW0818`): a carry
        /// live from world load is a hole in the graph's own claim.
        gating: CarryGating,
    },
    /// A line of sight between two places, in either direction. Carries no body,
    /// so it is not a traversal edge and the reachability closure never walks
    /// it; stage 4 gives it a sightline rather than a seam (spec-0049 §4.4).
    Vision {
        /// Edge id (`edge/<kebab>`), unique within the graph.
        id: EdgeId,
        /// One end.
        a: NodeId,
        /// The other end.
        b: NodeId,
    },
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Edge {
    /// The edge's id.
    #[must_use]
    pub fn id(&self) -> &EdgeId {
        match self {
            Edge::Walk { id, .. }
            | Edge::Stair { id, .. }
            | Edge::Climb { id, .. }
            | Edge::Drop { id, .. }
            | Edge::Carry { id, .. }
            | Edge::Vision { id, .. } => id,
        }
    }

    /// The `a` end.
    #[must_use]
    pub fn a(&self) -> &NodeId {
        match self {
            Edge::Walk { a, .. }
            | Edge::Stair { a, .. }
            | Edge::Climb { a, .. }
            | Edge::Drop { a, .. }
            | Edge::Carry { a, .. }
            | Edge::Vision { a, .. } => a,
        }
    }

    /// The `b` end.
    #[must_use]
    pub fn b(&self) -> &NodeId {
        match self {
            Edge::Walk { b, .. }
            | Edge::Stair { b, .. }
            | Edge::Climb { b, .. }
            | Edge::Drop { b, .. }
            | Edge::Carry { b, .. }
            | Edge::Vision { b, .. } => b,
        }
    }

    /// The class name as the document spells it.
    #[must_use]
    pub fn class(&self) -> &'static str {
        match self {
            Edge::Walk { .. } => "walk",
            Edge::Stair { .. } => "stair",
            Edge::Climb { .. } => "climb",
            Edge::Drop { .. } => "drop",
            Edge::Carry { .. } => "carry",
            Edge::Vision { .. } => "vision",
        }
    }

    /// True if a body passes along this edge. A `vision` edge does not.
    #[must_use]
    pub fn is_traversal(&self) -> bool {
        !matches!(self, Edge::Vision { .. })
    }

    /// True if the geometry owes this edge a **seam** — an opening on a shared
    /// face. A `vision` edge is owed a sightline instead, and a `carry` edge is
    /// owed nothing: a body crosses it by being put down on the far side.
    #[must_use]
    pub fn has_seam(&self) -> bool {
        !matches!(self, Edge::Vision { .. } | Edge::Carry { .. })
    }

    /// Which way a body may pass, or `None` for both ways (and for a `vision`
    /// edge, which carries none).
    #[must_use]
    pub fn direction(&self) -> Option<Direction> {
        match self {
            Edge::Walk { one_way, .. }
            | Edge::Stair { one_way, .. }
            | Edge::Climb { one_way, .. }
            | Edge::Carry { one_way, .. } => *one_way,
            Edge::Drop { falls, .. } => Some(*falls),
            Edge::Vision { .. } => None,
        }
    }

    /// True if this edge is marked as closing a loop.
    #[must_use]
    pub fn shortcut(&self) -> bool {
        match self {
            Edge::Walk { shortcut, .. }
            | Edge::Stair { shortcut, .. }
            | Edge::Climb { shortcut, .. }
            | Edge::Drop { shortcut, .. } => *shortcut,
            Edge::Carry { .. } | Edge::Vision { .. } => false,
        }
    }

    /// What a body must hold to pass, or `None` where the edge demands nothing.
    #[must_use]
    pub fn gating(&self) -> Option<GatingTerms<'_>> {
        match self {
            Edge::Walk { gating, .. }
            | Edge::Stair { gating, .. }
            | Edge::Climb { gating, .. }
            | Edge::Drop { gating, .. } => gating.as_ref().map(EdgeGating::terms),
            Edge::Carry { gating, .. } => Some(gating.terms()),
            Edge::Vision { .. } => None,
        }
    }

    /// A traversal connection's own gating as written, `opens_from` included —
    /// `None` on a `carry`, a `vision` and an ungated way.
    #[must_use]
    pub fn way_gating(&self) -> Option<&EdgeGating> {
        match self {
            Edge::Walk { gating, .. }
            | Edge::Stair { gating, .. }
            | Edge::Climb { gating, .. }
            | Edge::Drop { gating, .. } => gating.as_ref(),
            Edge::Carry { .. } | Edge::Vision { .. } => None,
        }
    }

    /// **True if the world builds this connection shut** (spec-0107 §3): it
    /// crosses a seam and its `gating` names a term. The derivation stands a
    /// bar in its seam, `anchor/seam-<edge>` names it, and the world-load seal
    /// measures it shut until the story opens it. The one predicate every
    /// reader of "is this way shut" asks.
    #[must_use]
    pub fn shuts(&self) -> bool {
        self.way_gating().is_some_and(|g| !g.terms().is_empty())
    }

    /// Which side can open a shut connection, or `None` for one that never
    /// shuts.
    #[must_use]
    pub fn opens_from(&self) -> Option<OpensFrom> {
        self.way_gating()
            .filter(|g| !g.terms().is_empty())
            .map(|g| g.opens_from)
    }

    /// How a shut connection declares it is learned shut, or `None` for one
    /// that never shuts or declares nothing (spec-0118).
    #[must_use]
    pub fn learned(&self) -> Option<Learned> {
        self.way_gating()
            .filter(|g| !g.terms().is_empty())
            .and_then(|g| g.learned)
    }

    /// The words a hidden shut connection keeps (`gating.secret`), empty for
    /// every other connection (spec-0118 §4).
    #[must_use]
    pub fn secret(&self) -> &[String] {
        self.way_gating()
            .filter(|g| !g.terms().is_empty() && g.learned == Some(Learned::Hidden))
            .map_or(&[], |g| g.secret.as_slice())
    }

    /// The place on the side a one-sided shut connection opens from, where its
    /// `anchor/unlock-<edge>` stands; `None` for either side or a way that
    /// never shuts.
    #[must_use]
    pub fn opening_side(&self) -> Option<&NodeId> {
        match self.opens_from()? {
            OpensFrom::A => Some(self.a()),
            OpensFrom::B => Some(self.b()),
            OpensFrom::Either => None,
        }
    }
}

/// Where one quest beat happens.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Beat {
    /// The quest the objective belongs to.
    pub quest: QuestId,
    /// The objective.
    pub objective: ObjectiveId,
    /// The place it happens in.
    pub node: NodeId,
}

// ---------------------------------------------------------------------------
// The closure (spec-0049 §3.2)
// ---------------------------------------------------------------------------

/// One thing a body can be holding: a produced flag, or a completed quest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grant {
    /// A flag some effect set.
    Flag(String),
    /// A quest that completed.
    Quest(String),
}

/// The monotone reachability closure of a layout graph.
///
/// Exactly Dormans' loop, and exactly §3.2: places grant, edges demand. What a
/// place grants is [`Grants::of`]'s reading of the campaign's one producer
/// table.
/// Deterministic, linear in the number of edges times the number of rounds, and
/// **optimistic in every direction it cannot decide** — which is one property,
/// stated once, rather than a list of exceptions:
///
/// * it is **branch-blind**, so a campaign whose branch points set mutually
///   exclusive flags can reach a node no single playthrough reaches;
/// * a beat's grants include everything the campaign fires when that objective
///   completes, including a `talk-to`'s whole dialogue tree, because talking is
///   what a body does at the place the speaker stands.
///
/// The optimism can only under-report at graph time; the stage-5 battery is
/// branch-aware over bytes and is what stops it shipping a broken world. If a
/// campaign's graph-stage green turns into a repeated stage-5 red on
/// branch-gated nodes, this closure gains branch awareness from
/// `compiler::flow`'s existing branch enumeration — that is the trigger, and
/// before it fires the simple closure is the cheaper instrument.
#[derive(Debug, Clone, Default)]
pub struct Closure {
    /// Every node a body can be at.
    pub reached: BTreeSet<String>,
    /// Everything a body can be holding once the fixpoint settles.
    pub obtained: BTreeSet<Grant>,
    /// Per node, what was already obtained when that node was **first** reached.
    /// The set `DW0819` judges a strand against.
    pub obtained_when: BTreeMap<String, BTreeSet<Grant>>,
}

impl Closure {
    /// True if `gating` is satisfied by `held`.
    #[must_use]
    pub fn satisfied(gating: Option<GatingTerms<'_>>, held: &BTreeSet<Grant>) -> bool {
        let Some(g) = gating else { return true };
        g.flags
            .iter()
            .all(|f| held.contains(&Grant::Flag(f.0.clone())))
            && g.quest
                .is_none_or(|q| held.contains(&Grant::Quest(q.0.clone())))
    }

    /// Run the closure from `entry`, with `grants` saying what a reached node
    /// hands a body and `quest_grants` what completing a quest does.
    #[must_use]
    pub fn run(graph: &LayoutGraphContent, grants: &Grants) -> Closure {
        let mut c = Closure::default();
        c.reached.insert(graph.entry.0.clone());
        c.obtained_when
            .insert(graph.entry.0.clone(), BTreeSet::new());
        loop {
            let before = (c.reached.len(), c.obtained.len());
            // (a) + (b): every edge whose demand is met carries a body, in the
            // direction it allows.
            for e in &graph.edges {
                if !e.is_traversal() || !Closure::satisfied(e.gating(), &c.obtained) {
                    continue;
                }
                let (a, b) = (e.a().0.as_str(), e.b().0.as_str());
                let forward = e.direction() != Some(Direction::BToA);
                let backward = e.direction() != Some(Direction::AToB);
                if forward && c.reached.contains(a) && !c.reached.contains(b) {
                    c.reached.insert(b.to_string());
                    c.obtained_when.insert(b.to_string(), c.obtained.clone());
                }
                if backward && c.reached.contains(b) && !c.reached.contains(a) {
                    c.reached.insert(a.to_string());
                    c.obtained_when.insert(a.to_string(), c.obtained.clone());
                }
            }
            // (c): every beat bound to a reached node hands over what it grants.
            for (node, given) in &grants.by_node {
                if c.reached.contains(node.as_str()) {
                    c.obtained.extend(given.iter().cloned());
                }
            }
            // A quest completes once every one of its beats is somewhere a body
            // can stand.
            for (quest, (nodes, given)) in &grants.by_quest {
                if nodes.iter().all(|n| c.reached.contains(n.as_str())) {
                    c.obtained.insert(Grant::Quest(quest.clone()));
                    c.obtained.extend(given.iter().cloned());
                }
            }
            // A producer whose chain demands flags fires once its holder is held
            // and the body holds what it demands.
            for gg in &grants.gated {
                let held = match &gg.holder {
                    Holder::Node(n) => c.reached.contains(n.as_str()),
                    Holder::Quest(q) => c.obtained.contains(&Grant::Quest(q.clone())),
                };
                if held && gg.requires.is_subset(&c.obtained) {
                    c.obtained.insert(gg.grant.clone());
                }
            }
            if (c.reached.len(), c.obtained.len()) == before {
                return c;
            }
        }
    }
}

/// What each place hands a body, derived from the campaign's own quest
/// documents — the *grant* half of §3.2's loop.
#[derive(Debug, Clone, Default)]
pub struct Grants {
    /// node id -> what standing there yields, unconditionally.
    pub by_node: BTreeMap<String, BTreeSet<Grant>>,
    /// quest id -> (the nodes its beats sit in, what completing it yields
    /// unconditionally).
    pub by_quest: BTreeMap<String, (BTreeSet<String>, BTreeSet<Grant>)>,
    /// What a place or a completed quest yields only once a body already holds
    /// the flags the producing chain requires.
    pub gated: Vec<GatedGrant>,
}

/// One producer whose chain demands flags of its own: credited once its
/// [`Holder`] is held and every flag in `requires` is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatedGrant {
    /// What must be reached or completed for the producer to fire.
    pub holder: Holder,
    /// The flags the producing chain requires.
    pub requires: BTreeSet<Grant>,
    /// What it yields.
    pub grant: Grant,
}

/// One producer row as the graph reads it: what its chain requires, and what
/// it yields.
type ProducerRow = (BTreeSet<Grant>, Grant);

/// Where a [`GatedGrant`] fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holder {
    /// A body stands at this node.
    Node(String),
    /// This quest completed.
    Quest(String),
}

impl Grants {
    /// Derive the grants a graph implies, from the campaign's one producer
    /// table ([`crate::producers::flag_producers`]) and its dialogue.
    ///
    /// Every row of the table is credited where it happens:
    ///
    /// * an objective's row, at the place its beat is bound to;
    /// * a quest's row, once the quest completes;
    /// * an ambient row — a trigger, a trap's payload or disarm, a timed gate's
    ///   disarm, a shortcut's unlock, a shop offer — at the place its anchor
    ///   belongs to ([`LayoutGraphContent::place_of`]). A row whose anchor names
    ///   no place of the graph is credited nowhere.
    ///
    /// A row whose chain requires flags is credited only once a body holds
    /// them. A `talk-to` beat additionally grants every flag the spoken-to NPC's
    /// dialogue options set, because the conversation happens where the speaker
    /// stands.
    #[must_use]
    pub fn of(c: &Campaign, graph: &LayoutGraphContent) -> Grants {
        let mut g = Grants::default();
        let flags_of = |req: &[String]| -> BTreeSet<Grant> {
            req.iter().map(|f| Grant::Flag(f.clone())).collect()
        };
        // (quest, objective) -> the rows its completion fires.
        let mut on_objective: BTreeMap<(&str, &str), Vec<ProducerRow>> = BTreeMap::new();
        let mut on_quest: BTreeMap<&str, Vec<ProducerRow>> = BTreeMap::new();
        // A story-only row (an `on_kill`) is credited by no place: no kill is
        // forced, and the graph is not a story world.
        for p in crate::producers::flag_producers(c)
            .into_iter()
            .filter(|p| !p.story_only)
        {
            let row = (flags_of(&p.requires), Grant::Flag(p.flag.to_string()));
            match p.site {
                crate::producers::ProducerSite::ObjectiveComplete { quest, objective } => {
                    on_objective
                        .entry((quest, objective))
                        .or_default()
                        .push(row);
                }
                crate::producers::ProducerSite::QuestComplete { quest } => {
                    on_quest.entry(quest).or_default().push(row);
                }
                crate::producers::ProducerSite::Ambient { at, .. } => {
                    if let Some(node) = at.and_then(|a| graph.place_of(a.as_str())) {
                        g.credit(Holder::Node(node.0.clone()), row);
                    }
                }
            }
        }
        // NPC -> the flags anything in their dialogue tree can set.
        let mut npc_flags: BTreeMap<&str, BTreeSet<Grant>> = BTreeMap::new();
        for tree in &c.dialogue.content.dialogues {
            let set = npc_flags.entry(tree.npc.0.as_str()).or_default();
            for node in &tree.nodes {
                for opt in &node.options {
                    for eff in &opt.effects {
                        if let Some(f) = eff.set_flag() {
                            set.insert(Grant::Flag(f.0.clone()));
                        }
                    }
                }
            }
        }
        let mut talks: BTreeMap<(&str, &str), &str> = BTreeMap::new();
        for q in &c.quests.content.quests {
            for obj in &q.objectives {
                if let Objective::TalkTo { id, npc, .. } = obj {
                    talks.insert((q.id.0.as_str(), id.0.as_str()), npc.0.as_str());
                }
            }
        }
        for beat in &graph.beats {
            let node = beat.node.0.clone();
            let key = (beat.quest.0.as_str(), beat.objective.0.as_str());
            for row in on_objective.get(&key).into_iter().flatten() {
                g.credit(Holder::Node(node.clone()), row.clone());
            }
            if let Some(flags) = talks.get(&key).and_then(|npc| npc_flags.get(npc)) {
                g.by_node
                    .entry(node.clone())
                    .or_default()
                    .extend(flags.iter().cloned());
            }
            g.by_quest
                .entry(beat.quest.0.clone())
                .or_insert_with(|| (BTreeSet::new(), BTreeSet::new()))
                .0
                .insert(node);
        }
        for (quest, rows) in on_quest {
            if !g.by_quest.contains_key(quest) {
                continue; // a quest with no beat here never completes in this proof
            }
            for row in rows {
                g.credit(Holder::Quest(quest.to_string()), row);
            }
        }
        g
    }

    /// Record one producer row at its holder: unconditional when its chain
    /// requires nothing, gated otherwise.
    fn credit(&mut self, holder: Holder, (requires, grant): ProducerRow) {
        if !requires.is_empty() {
            self.gated.push(GatedGrant {
                holder,
                requires,
                grant,
            });
            return;
        }
        match holder {
            Holder::Node(n) => {
                self.by_node.entry(n).or_default().insert(grant);
            }
            Holder::Quest(q) => {
                self.by_quest
                    .entry(q)
                    .or_insert_with(|| (BTreeSet::new(), BTreeSet::new()))
                    .1
                    .insert(grant);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The binding ledger
// ---------------------------------------------------------------------------

/// What a run's **map-pipeline** checks bound to — stages 2, 3 and 4.
///
/// Stated on every run whether or not anything was found, because a count only
/// means something when the run that found nothing prints it too (CLAUDE.md). It
/// is one struct with one constructor, so the number the CLI prints, the number
/// the build ledger records and the number a diagnostic quotes cannot disagree.
///
/// One ledger for three documents rather than one per document, and that is the
/// point: a site plan's boxes are counted beside the graph's places, so a run
/// that embeds five of six places states both numbers on one line and the
/// mismatch is visible without arithmetic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LayoutBinding {
    /// Places declared — what `DW0816` examines.
    pub nodes: usize,
    /// Connections declared.
    pub edges: usize,
    /// Of those, connections a body passes along.
    pub traversal_edges: usize,
    /// Of those, connections marked as closing a loop — what `DW0820` examines.
    pub shortcut_edges: usize,
    /// Connections a body passes one way only — what `DW0819` examines.
    pub one_way_edges: usize,
    /// Connections that demand something before a body may pass.
    pub gated_edges: usize,
    /// Connections a body is carried over rather than walks (spec-0083 §7) —
    /// what `DW0934` matches against the campaign's links.
    pub carry_edges: usize,
    /// **Named places inside places** — stations declared across the whole graph
    /// (spec-0052 §4).
    ///
    /// A station no quest references is legal mid-authoring, so this is the
    /// denominator rather than a count of what is used, and a **zero is stated**
    /// like every other: a site-plan campaign with no stations names its places
    /// at node granularity, which is a fact about that campaign and not a
    /// silence.
    pub stations: usize,
    /// Of those, stations declared as a gate — the ones `open-gate`,
    /// `close-gate`, a `shortcut` and a `timed-gate` may address. The remainder
    /// are points, which is what every other consumer resolves.
    pub gate_stations: usize,
    /// Quest beats bound to a place.
    pub beats: usize,
    /// Of those, beats on the **mandatory quest spine** — the number `DW0817`'s
    /// obligation to visit them actually quantifies over.
    ///
    /// It is carried separately because `beats` is not the binding: a graph can
    /// declare a dozen beats and still ask `DW0817` to check nothing, if none of
    /// their quests is one the finale depends on. A zero here is the *graph
    /// before mission* case and is reported as a finding.
    pub spine_beats: usize,
    /// Steps of the authored critical path — what `DW0817` and `DW0822`
    /// examine.
    pub path_steps: usize,
    /// Facts the geometry brief states — what a site plan's identities bind to.
    pub brief_facts: usize,
    /// Reference rows declared across the graph (spec-0123 §3.3).
    pub references: usize,
    /// Of the nodes, those declaring at least one reference.
    pub referenced_nodes: usize,
    /// What the site plan offers the stage-4 checks.
    pub plan: crate::siteplan::PlanBinding,
}

impl LayoutBinding {
    /// Count what a campaign's layout documents offer the checks.
    #[must_use]
    pub fn of(c: &Campaign) -> LayoutBinding {
        let mut b = LayoutBinding {
            brief_facts: c
                .geometry_brief
                .as_ref()
                .map_or(0, |g| g.content.facts.len()),
            ..LayoutBinding::default()
        };
        let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
            b.plan = crate::siteplan::PlanBinding::of(c);
            return b;
        };
        b.plan = crate::siteplan::PlanBinding::of(c);
        b.nodes = graph.nodes.len();
        b.edges = graph.edges.len();
        b.beats = graph.beats.len();
        let spine = c.quest_plan.content.spine();
        b.spine_beats = graph
            .beats
            .iter()
            .filter(|beat| spine.contains(beat.quest.0.as_str()))
            .count();
        b.path_steps = graph.critical_path.len().saturating_sub(1);
        for n in &graph.nodes {
            b.references += n.references.len();
            b.referenced_nodes += usize::from(!n.references.is_empty());
            b.stations += n.stations.len();
            b.gate_stations += n
                .stations
                .iter()
                .filter(|s| s.kind == StationKind::Gate)
                .count();
        }
        for e in &graph.edges {
            if e.is_traversal() {
                b.traversal_edges += 1;
            }
            if e.shortcut() {
                b.shortcut_edges += 1;
            }
            if e.is_traversal() && e.direction().is_some() {
                b.one_way_edges += 1;
            }
            if e.gating().is_some_and(|g| !g.is_empty()) {
                b.gated_edges += 1;
            }
            if matches!(e, Edge::Carry { .. }) {
                b.carry_edges += 1;
            }
        }
        b
    }

    /// One line, for stderr and for the round summary.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "layout-graph binding: {n} node(s), {e} edge(s) ({t} traversal, {ow} one-way, \
             {s} shortcut, {g} gated, {c} carry), {st} station(s) of which {gs} gate(s), {b} beat(s) \
             of which {sb} on the mandatory spine, {p} critical-path step(s); \
             geometry-brief binding: {f} fact(s).",
            n = self.nodes,
            e = self.edges,
            t = self.traversal_edges,
            ow = self.one_way_edges,
            s = self.shortcut_edges,
            g = self.gated_edges,
            c = self.carry_edges,
            st = self.stations,
            gs = self.gate_stations,
            b = self.beats,
            sb = self.spine_beats,
            p = self.path_steps,
            f = self.brief_facts,
        )
    }

    /// The references a layout graph carries (spec-0123 §3.3), zeroes stated:
    /// `references binding: R reference(s) over N of M node(s)`.
    #[must_use]
    pub fn references_line(&self) -> String {
        format!(
            "references binding: {r} reference(s) over {n} of {m} node(s)",
            r = self.references,
            n = self.referenced_nodes,
            m = self.nodes,
        )
    }

    /// The site-plan half, when the campaign carries one.
    #[must_use]
    pub fn plan_line(&self) -> String {
        self.plan.line()
    }
}

// ---------------------------------------------------------------------------
// Validation tier (spec-0049 §3.3): DW0814, DW0818, DW0820, DW0822, DW0812,
// and — since the reachability battery moved here — DW0816, DW0817, DW0819
// ---------------------------------------------------------------------------

/// Every check the layout documents owe at **validation** tier.
///
/// Invoked from [`crate::validate::validate_campaign_with`] whenever the
/// campaign carries either document — the same event-bound shape stage 7's edit
/// script uses. There is no separate entry point to remember and no flag to
/// pass: a campaign directory holding a `layout-graph.json` cannot be validated
/// without running this.
pub fn check(c: &Campaign, reads: &mut Reads, d: &mut Vec<Diagnostic>) {
    let table = Metrics::table();
    if let Some(brief) = &c.geometry_brief {
        brief_checks(&brief.content, d);
    }
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return;
    };
    // Referential wellformedness first: every check below reads node ids, and a
    // dangling one would make each of them answer a question about a place that
    // is not there.
    let known: BTreeSet<&str> = graph.nodes.iter().map(|n| n.id.0.as_str()).collect();
    let malformed = wellformed(graph, &known, d);
    stations(graph, d);
    references(graph, d);
    if malformed {
        return;
    }
    mission(c, graph, d);
    shortcut_loops(graph, d);
    pacing(c, graph, &table, reads, d);
    // The reachability proofs run HERE and nowhere else — see [`reachability`]
    // for why they are no longer raised from the analysis pass. The page tells
    // an author to loop `validate` at the graph step; this is what makes that
    // instruction true.
    d.extend(reachability(c));
}

/// **A reference row conforms to its own schema** (spec-0123 §3.3): an
/// `https://` URL with no blank in it, and a non-empty `shows` and `terms`.
/// The schema states the three bounds and serde enforces none of them, so they
/// are restated here at the schema tier (`DW0100`).
fn references(graph: &LayoutGraphContent, d: &mut Vec<Diagnostic>) {
    for (i, n) in graph.nodes.iter().enumerate() {
        for (j, r) in n.references.iter().enumerate() {
            let path = format!("/content/nodes/{i}/references/{j}");
            let url_ok = r.url.strip_prefix("https://").is_some_and(|rest| {
                !rest.is_empty() && !rest.chars().any(char::is_whitespace)
            });
            if !url_ok {
                d.push(Diagnostic::error(
                    crate::codes::SCHEMA,
                    "layout-graph",
                    format!("{path}/url"),
                    format!(
                        "`{}` reference {j} has the url `{}`, which is not an `https://` address: \
                         a reference is a page or a picture a person and the fetch tool can both \
                         open, so it is named by an `https://` URL with no blank in it",
                        n.id, r.url
                    ),
                ));
            }
            for (field, value) in [("shows", &r.shows), ("terms", &r.terms)] {
                if value.trim().is_empty() {
                    d.push(Diagnostic::error(
                        crate::codes::SCHEMA,
                        "layout-graph",
                        format!("{path}/{field}"),
                        format!(
                            "`{}` reference {j} states an empty `{field}`: a reference says what it \
                             shows and the terms its source states (`\"none stated\"` where it states none)",
                            n.id
                        ),
                    ));
                }
            }
        }
    }
}

/// **The two refusals a declared station owes** (spec-0052 §7.1 and §7.2).
///
/// Runs before the malformed-graph return, because a station's name is judged
/// against the engine's namespace and against the other stations — neither of
/// which needs the node ids to resolve. A campaign with a dangling edge still
/// gets told its station name is taken.
fn stations(graph: &LayoutGraphContent, d: &mut Vec<Diagnostic>) {
    // ---- §7.1: a station in the engine's namespace.
    //
    // The PREFIX is the rule, not the collision: `anchor/seam-vestry-door` is
    // refused even where no such edge exists, so that adding the edge later
    // cannot turn a legal graph into two claims on one name.
    //
    // Reserving `spawn` by its exact name rather than by a prefix is not an
    // inconsistency — `spawn` is one name the derivation synthesizes, and it has
    // no family to reserve.
    let entry = crate::siteplan::ENTRY_ANCHOR;
    for (i, n) in graph.nodes.iter().enumerate() {
        for (j, s) in n.stations.iter().enumerate() {
            let name = s.anchor.as_str();
            let reserved = ["anchor/node-", "anchor/seam-", "anchor/unlock-"]
                .into_iter()
                .find(|p| name.starts_with(p));
            let why = if let Some(prefix) = reserved {
                format!("its name begins `{prefix}`")
            } else if name == entry {
                format!("`{entry}` is the name the entry place stands under")
            } else {
                continue;
            };
            d.push(Diagnostic::error(
                DW_STATION_RESERVED,
                "layout-graph",
                format!("/content/nodes/{i}/stations/{j}/anchor"),
                format!(
                    "station `{name}` takes a name the engine derives: {why}. The derivation \
                     synthesizes this campaign's whole spatial vocabulary — `{entry}`, an \
                     `anchor/node-…` for each place, and an `anchor/seam-…` plus an \
                     `anchor/unlock-…` for each gated connection — so those three prefixes and \
                     `{entry}` are reserved whether or not this graph happens to produce the \
                     name today. Name the station something of your own, and the quest layer \
                     references it exactly as it references a derived one."
                ),
            ));
        }
    }

    // ---- §7.2: two claims on one name.
    //
    // **The scope of uniqueness is the area**, which is the scope every anchor
    // reference already resolves in — unchanged from the standing rule, and the
    // reason two pieces may both declare `anchor/door` and collide with nothing.
    // A site-plan campaign has one area, so this is the whole graph.
    let mut first: BTreeMap<&str, &NodeId> = BTreeMap::new();
    for (i, n) in graph.nodes.iter().enumerate() {
        for (j, s) in n.stations.iter().enumerate() {
            let name = s.anchor.as_str();
            if let Some(other) = first.get(name) {
                d.push(Diagnostic::error(
                    DW_STATION_DUPLICATE,
                    "layout-graph",
                    format!("/content/nodes/{i}/stations/{j}/anchor"),
                    format!(
                        "station `{name}` is declared by `{here}` and by `{other}`. A station \
                         name is unique within the AREA — the scope every anchor reference \
                         resolves in — and a site-plan campaign has exactly one, so the \
                         campaign's whole vocabulary shares it. Two places cannot both answer \
                         to one name, because a quest naming it would have two answers and \
                         nothing would say which. Rename one of them, or declare it on one \
                         place only: a quest in either place may name a station of the other.",
                        here = n.id,
                    ),
                ));
            } else {
                first.insert(name, &n.id);
            }
        }
    }
}

/// `fact/<kebab>` ids, unique, and every fact carries the sentence it came from.
fn brief_checks(brief: &GeometryBriefContent, d: &mut Vec<Diagnostic>) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, f) in brief.facts.iter().enumerate() {
        if !f.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                crate::codes::ID_SYNTAX,
                "geometry-brief",
                format!("/content/facts/{i}/id"),
                format!(
                    "malformed fact id `{}` — a brief fact is named `fact/<kebab-case>`, so that \
                     a site plan's identity can bind to it by name.",
                    f.id
                ),
            ));
        }
        if !seen.insert(f.id.0.as_str()) {
            d.push(Diagnostic::error(
                crate::codes::ID_DUPLICATE,
                "geometry-brief",
                format!("/content/facts/{i}/id"),
                format!(
                    "duplicate fact id `{}` — rename one, because an identity binding to this \
                     name would otherwise hold the map to whichever number was written last.",
                    f.id
                ),
            ));
        }
    }
}

/// `DW0814` and the id-syntax rules. Returns true if the graph is malformed
/// enough that the semantic checks below it would be answering about places that
/// are not there.
fn wellformed(graph: &LayoutGraphContent, known: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) -> bool {
    let before = d.len();
    let mut seen_nodes: BTreeSet<&str> = BTreeSet::new();
    for (i, n) in graph.nodes.iter().enumerate() {
        if !n.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                crate::codes::ID_SYNTAX,
                "layout-graph",
                format!("/content/nodes/{i}/id"),
                format!(
                    "malformed node id `{}` — a place is named `node/<kebab-case>`.",
                    n.id
                ),
            ));
        }
        if !seen_nodes.insert(n.id.0.as_str()) {
            d.push(Diagnostic::error(
                DW_GRAPH_MALFORMED,
                "layout-graph",
                format!("/content/nodes/{i}/id"),
                format!(
                    "duplicate node id `{}` — two places cannot share a name, because every \
                     edge, beat and critical-path step that names it would then name both.",
                    n.id
                ),
            ));
        }
        if n.intent.trim().is_empty() {
            d.push(Diagnostic::error(
                DW_GRAPH_MALFORMED,
                "layout-graph",
                format!("/content/nodes/{i}/intent"),
                format!(
                    "place `{}` declares an empty `intent` — no check keys on this label, which \
                     is exactly why it has to be written: it is the recorded judgement a \
                     reviewer and the later per-place brief read.",
                    n.id
                ),
            ));
        }
    }
    let mut seen_edges: BTreeSet<&str> = BTreeSet::new();
    for (i, e) in graph.edges.iter().enumerate() {
        if !e.id().is_valid_syntax() {
            d.push(Diagnostic::error(
                crate::codes::ID_SYNTAX,
                "layout-graph",
                format!("/content/edges/{i}/id"),
                format!(
                    "malformed edge id `{}` — a connection is named `edge/<kebab-case>`.",
                    e.id()
                ),
            ));
        }
        if !seen_edges.insert(e.id().0.as_str()) {
            d.push(Diagnostic::error(
                DW_GRAPH_MALFORMED,
                "layout-graph",
                format!("/content/edges/{i}/id"),
                format!(
                    "duplicate edge id `{}` — rename one, because a seam is allocated per edge \
                     and two edges of one name would allocate one seam between them.",
                    e.id()
                ),
            ));
        }
        for (end, node) in [("a", e.a()), ("b", e.b())] {
            if !known.contains(node.0.as_str()) {
                d.push(Diagnostic::error(
                    DW_GRAPH_MALFORMED,
                    "layout-graph",
                    format!("/content/edges/{i}/{end}"),
                    format!(
                        "connection `{}` ends at `{node}`, which is not a declared place — \
                         declare that node, or point the end at one of the {n} that exist.",
                        e.id(),
                        n = known.len(),
                    ),
                ));
            }
        }
        if e.a() == e.b() {
            d.push(Diagnostic::error(
                DW_GRAPH_MALFORMED,
                "layout-graph",
                format!("/content/edges/{i}"),
                format!(
                    "connection `{}` has both ends in `{}` — a self-loop states nothing a place \
                     does not already state, at every class, so it is refused rather than \
                     silently carried into a seam allocation with no face to sit on.",
                    e.id(),
                    e.a(),
                ),
            ));
        }
    }
    for (field, node) in [("entry", &graph.entry), ("goal", &graph.goal)] {
        if !known.contains(node.0.as_str()) {
            d.push(Diagnostic::error(
                DW_GRAPH_MALFORMED,
                "layout-graph",
                format!("/content/{field}"),
                format!(
                    "`{field}` names `{node}`, which is not a declared place — every proof over \
                     this graph starts or ends there, so it cannot be a name nothing defines."
                ),
            ));
        }
    }
    for (i, node) in graph.critical_path.iter().enumerate() {
        if !known.contains(node.0.as_str()) {
            d.push(Diagnostic::error(
                DW_GRAPH_MALFORMED,
                "layout-graph",
                format!("/content/critical_path/{i}"),
                format!("the critical path steps through `{node}`, which is not a declared place."),
            ));
        }
    }
    for (i, beat) in graph.beats.iter().enumerate() {
        if !known.contains(beat.node.0.as_str()) {
            d.push(Diagnostic::error(
                DW_GRAPH_MALFORMED,
                "layout-graph",
                format!("/content/beats/{i}/node"),
                format!(
                    "beat `{q}` / `{o}` happens in `{n}`, which is not a declared place.",
                    q = beat.quest,
                    o = beat.objective,
                    n = beat.node,
                ),
            ));
        }
    }
    d.len() != before
}

/// `DW0818`: the graph and the mission agree, in both directions.
fn mission(c: &Campaign, graph: &LayoutGraphContent, d: &mut Vec<Diagnostic>) {
    let quests: BTreeMap<&str, BTreeSet<&str>> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| {
            (
                q.id.0.as_str(),
                q.objectives.iter().map(|o| o.id().0.as_str()).collect(),
            )
        })
        .collect();
    let produced = crate::validate::produced_flags(c);

    // **Stage 5 being empty is not a graph mistake.** It is the ordinary state
    // of a campaign whose plan is written and whose quests are not — the state
    // `DW0150` names — and in it EVERY name the graph borrows from the mission
    // is absent, so every such refusal says the same thing and none of them is
    // the finding. Without this clause a finished layout graph reports a fault
    // per beat and per gated way, in a document the author completed two steps
    // ago, at exactly the moment they are being told to loop validation until
    // it is clean.
    //
    // It is attached to every refusal in this function that reads the stage-5
    // quest list, not to the one that was noticed: a beat and a gated
    // connection are the same borrowing, and a clause on one of them would be
    // the narrow binding this codebase keeps finding.
    let unwritten = if quests.is_empty() {
        " Stage 5 declares no quests at all here, so everything this graph borrows from the \
         mission is missing, every one of these lines says the same thing, and none of them is \
         the finding: see `DW0150`, which names that state. This clears when stage 5 is written."
    } else {
        ""
    };

    // Direction one: nothing the graph names may be absent from the mission.
    let mut bound: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (i, beat) in graph.beats.iter().enumerate() {
        match quests.get(beat.quest.0.as_str()) {
            None => d.push(Diagnostic::error(
                DW_GRAPH_MISSION,
                "layout-graph",
                format!("/content/beats/{i}/quest"),
                format!(
                    "beat names quest `{}`, which the quest documents do not declare — the graph \
                     says where the mission happens, so it can only name beats the mission \
                     has.{unwritten}",
                    beat.quest,
                ),
            )),
            Some(objectives) if !objectives.contains(beat.objective.0.as_str()) => {
                d.push(Diagnostic::error(
                    DW_GRAPH_MISSION,
                    "layout-graph",
                    format!("/content/beats/{i}/objective"),
                    format!(
                        "beat names objective `{o}`, which quest `{q}` does not declare.",
                        o = beat.objective,
                        q = beat.quest,
                    ),
                ));
            }
            Some(_) => {
                *bound
                    .entry((beat.quest.0.as_str(), beat.objective.0.as_str()))
                    .or_default() += 1;
            }
        }
    }
    for (i, e) in graph.edges.iter().enumerate() {
        let Some(g) = e.gating() else { continue };
        for (k, f) in g.flags.iter().enumerate() {
            if !produced.contains(f.0.as_str()) {
                d.push(Diagnostic::error(
                    DW_GRAPH_MISSION,
                    "layout-graph",
                    format!("/content/edges/{i}/gating/flags/{k}"),
                    format!(
                        "connection `{e_id}` waits on flag `{f}`, which no `set-flag` effect ever \
                         produces — a body could never hold it, so the connection is a wall \
                         wearing a gate's clothes. Produce the flag, or gate on one the campaign \
                         really sets.",
                        e_id = e.id(),
                    ),
                ));
            }
        }
        if let Some(q) = &g.quest
            && !quests.contains_key(q.0.as_str())
        {
            d.push(Diagnostic::error(
                DW_GRAPH_MISSION,
                "layout-graph",
                format!("/content/edges/{i}/gating/quest"),
                format!(
                    "connection `{e_id}` waits on quest `{q}`, which the quest documents do not \
                     declare.{unwritten}",
                    e_id = e.id(),
                ),
            ));
        }
        if matches!(e, Edge::Carry { .. }) && g.is_empty() {
            d.push(Diagnostic::error(
                DW_GRAPH_MISSION,
                "layout-graph",
                format!("/content/edges/{i}/gating"),
                format!(
                    "carry connection `{e_id}` says nothing about what makes it carry. A carry \
                     with an empty `gating` is live from world load, which leaves a hole in the \
                     graph's own claim; name the flag or the quest whose completion arms the \
                     link that realises it.",
                    e_id = e.id(),
                ),
            ));
        }
        if !matches!(e, Edge::Carry { .. }) && g.is_empty() {
            let mut side = match e.way_gating().map(|w| w.opens_from) {
                Some(OpensFrom::A) => " It names a side (`opens_from: a`) for a way that never \
                                       shuts, which is a door with no bar in it."
                    .to_string(),
                Some(OpensFrom::B) => " It names a side (`opens_from: b`) for a way that never \
                                       shuts, which is a door with no bar in it."
                    .to_string(),
                _ => String::new(),
            };
            if let Some(l) = e.way_gating().and_then(|w| w.learned) {
                side.push_str(&format!(
                    " It says how the way is learned shut (`learned: {}`) beside `opens_from`, \
                     for a way that never shuts: only a way with a bar in it can be learned shut.",
                    l.word()
                ));
            }
            d.push(Diagnostic::error(
                DW_GRAPH_MISSION,
                "layout-graph",
                format!("/content/edges/{i}/gating"),
                format!(
                    "{class} connection `{e_id}` carries a `gating` that names no term. A \
                     `gating` is what builds the way shut until the story opens it, and an empty \
                     one is passable from world load, which is not gated.{side} Name the flag or \
                     the quest whose completion opens it, and `DW0818` then holds that name to \
                     something the campaign really produces — or delete the `gating`.",
                    class = e.class(),
                    e_id = e.id(),
                ),
            ));
        }
        // spec-0118 §4: a hidden way names the words that would give it away,
        // and only a hidden way does.
        if let Some(w) = e.way_gating() {
            let at = format!("/content/edges/{i}/gating");
            let hidden = w.learned == Some(Learned::Hidden);
            if hidden && w.secret.is_empty() {
                d.push(Diagnostic::error(
                    DW_GRAPH_MISSION,
                    "layout-graph",
                    at.clone(),
                    format!(
                        "connection `{e_id}` declares `learned: hidden` and names no `secret`. A \
                         hidden way is one the player does not know of until the shot that opens \
                         it, and the engine holds the text before that shot to the words that \
                         would give it away — which only the author can name. Write them in \
                         `gating.secret` (`[\"passage\", \"behind the hearth\"]`), or write the \
                         word that is true.",
                        e_id = e.id(),
                    ),
                ));
            }
            if !hidden && !w.secret.is_empty() {
                let word = w.learned.map_or_else(
                    || "no `learned`".to_string(),
                    |l| format!("`learned: {}`", l.word()),
                );
                d.push(Diagnostic::error(
                    DW_GRAPH_MISSION,
                    "layout-graph",
                    format!("{at}/secret"),
                    format!(
                        "connection `{e_id}` names a `secret` beside {word}. A secret is the \
                         words a hidden way keeps until the shot that opens it, and only a way \
                         declared `learned: hidden` keeps one: a way the player sees, is told \
                         of or is paid for has nothing to keep. Remove `secret`, or declare the \
                         way `hidden`.",
                        e_id = e.id(),
                    ),
                ));
            }
            if let Some(k) = w.secret.iter().position(|s| s.trim().is_empty()) {
                d.push(Diagnostic::error(
                    DW_GRAPH_MISSION,
                    "layout-graph",
                    format!("{at}/secret/{k}"),
                    format!(
                        "connection `{e_id}`'s `secret[{k}]` is empty: a word nothing can mention \
                         keeps nothing. Write the word or remove the entry.",
                        e_id = e.id(),
                    ),
                ));
            }
        }
    }

    // Direction two — the ordering tooth. Once a mission exists, every beat of
    // it has a place. This is what stops a graph arriving after the quests and
    // ignoring them, and it is why *graph before mission* is representable
    // without being silently green (spec-0049 §7).
    for q in &c.quests.content.quests {
        for (oi, obj) in q.objectives.iter().enumerate() {
            let n = bound
                .get(&(q.id.0.as_str(), obj.id().0.as_str()))
                .copied()
                .unwrap_or(0);
            if n == 0 {
                d.push(Diagnostic::error(
                    DW_GRAPH_MISSION,
                    "quests",
                    format!(
                        "/content/quests/{qi}/objectives/{oi}",
                        qi = quest_index(c, q)
                    ),
                    format!(
                        "objective `{o}` of quest `{q}` happens somewhere and the layout graph \
                         does not say where. Every objective is place-bound — a body has to be \
                         standing somewhere to talk, to reach, to fight or to take — so add a \
                         `beats[]` entry binding it to a node. This is the direction that keeps \
                         space and mission from silently disagreeing: a graph may be authored \
                         before the quests, never in ignorance of them.",
                        o = obj.id(),
                        q = q.id,
                    ),
                ));
            } else if n > 1 {
                d.push(Diagnostic::error(
                    DW_GRAPH_MISSION,
                    "layout-graph",
                    "/content/beats",
                    format!(
                        "objective `{o}` of quest `{q}` is bound to {n} places. A beat happens in \
                         exactly one place; two bindings make every proof over this graph pick \
                         one of them and no rule says which.",
                        o = obj.id(),
                        q = q.id,
                    ),
                ));
            }
        }
    }
}

/// The index of a quest in the stage-5 document, for a diagnostic's path.
fn quest_index(c: &Campaign, q: &crate::Quest) -> usize {
    c.quests
        .content
        .quests
        .iter()
        .position(|x| std::ptr::eq(x, q))
        .unwrap_or(0)
}

/// `DW0820`: a shortcut lies on a cycle.
fn shortcut_loops(graph: &LayoutGraphContent, d: &mut Vec<Diagnostic>) {
    for (i, e) in graph.edges.iter().enumerate() {
        if !e.shortcut() {
            continue;
        }
        if connected_without(graph, e.id(), e.a(), e.b()) {
            continue;
        }
        d.push(Diagnostic::error(
            DW_SHORTCUT_NO_LOOP,
            "layout-graph",
            format!("/content/edges/{i}/shortcut"),
            format!(
                "connection `{e_id}` is marked a shortcut and closes no loop: with it removed, \
                 `{a}` and `{b}` are no longer connected at all. A shortcut is the way back into \
                 ground a body has already crossed, so an edge that is the ONLY way between its \
                 two places is a corridor wearing a shortcut's name. Either drop the mark, or add \
                 the long way round it is meant to shorten.",
                e_id = e.id(),
                a = e.a(),
                b = e.b(),
            ),
        ));
    }
}

/// Undirected connectivity between two places over every traversal edge but
/// `skip`. Direction-blind and gating-blind on purpose: the loop a shortcut
/// closes is **spatial**, and a long way round that is gated is still the long
/// way round.
fn connected_without(graph: &LayoutGraphContent, skip: &EdgeId, a: &NodeId, b: &NodeId) -> bool {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![a.0.as_str()];
    seen.insert(a.0.as_str());
    while let Some(at) = stack.pop() {
        if at == b.0.as_str() {
            return true;
        }
        for e in &graph.edges {
            if !e.is_traversal() || e.id() == skip {
                continue;
            }
            let (x, y) = (e.a().0.as_str(), e.b().0.as_str());
            let other = if x == at {
                y
            } else if y == at {
                x
            } else {
                continue;
            };
            if seen.insert(other) {
                stack.push(other);
            }
        }
    }
    false
}

/// `DW0822`: the pacing projection, printed with **no threshold**.
///
/// # A leg is measured, never looked up
///
/// A place's traverse is **its box's long horizontal extent** — a real number
/// the author declared by drawing the box — and nothing else is consulted.
///
/// Where the campaign carries **no site plan**, a place has no geometry yet and
/// the projection says so: the leg is counted as **unprojected** in the line's
/// own binding rather than being given an invented number. A campaign with a
/// graph and no plan is a real and intended state — the graph is authored
/// before the embedding — so this is the ordinary case and not a fault.
fn pacing(
    c: &Campaign,
    graph: &LayoutGraphContent,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) {
    // The plan's boxes, when there is a plan. `DW0824` is what holds this to one
    // box per place; a lookup that finds nothing here is a campaign the graph
    // stage is looking at before the plan stage exists, which is legal.
    let runs: BTreeMap<&str, u64> = c
        .site_plan
        .as_ref()
        .map(|p| {
            p.content
                .boxes
                .iter()
                .map(|b| {
                    let (dx, dz) = (u64::from(b.extent[0].get()), u64::from(b.extent[1].get()));
                    (b.node.0.as_str(), dx.max(dz))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut blocks: u64 = 0;
    let mut legs = 0usize;
    let mut unprojected: Vec<String> = Vec::new();
    for node_id in &graph.critical_path {
        if !graph.nodes.iter().any(|n| &n.id == node_id) {
            continue;
        }
        match runs.get(node_id.0.as_str()) {
            Some(run) => {
                blocks += *run;
                legs += 1;
            }
            None => unprojected.push(format!("`{node_id}`")),
        }
    }
    let Ok(per_minute) = table.resolve(MetricKind::Pacing, "route-blocks-per-minute") else {
        return;
    };
    let crate::metrics::MetricValue::Count(rate) = per_minute.value(reads) else {
        return;
    };
    let rate = u64::from(*rate).max(1);
    d.push(Diagnostic::warning(
        DW_PACING,
        "layout-graph",
        "/content/critical_path",
        format!(
            "the critical path crosses {legs} place(s) over {steps} step(s), a measured \
             {blocks} blocks of route, which at {rate} blocks of route per minute of play \
             projects to about {minutes} minute(s) against this world's `target_minutes` of \
             {target}{un}. It carries no threshold and refuses nothing — see `DW0822` in \
             `docs/reference/dsl/layout.md` for what the number is worth.",
            steps = graph.critical_path.len().saturating_sub(1),
            minutes = blocks.div_ceil(rate),
            target = c.world.content.target_minutes,
            un = if unprojected.is_empty() {
                String::new()
            } else {
                format!(
                    ", with {n} leg(s) UNPROJECTED and not in that total ({names}) — what \
                     crossing a place costs is its box's long extent, and this campaign has no \
                     site plan to read it from yet. Embedding the graph is what projects them; \
                     nothing is wrong here",
                    n = unprojected.len(),
                    names = unprojected.join(", "),
                )
            },
        ),
    ));
}

// ---------------------------------------------------------------------------
// Reachability (spec-0049 §3.3): DW0816, DW0817, DW0819
// ---------------------------------------------------------------------------

/// The graph's reachability proofs — a place a body can never reach
/// (`DW0816`), an authored critical path that does not hold (`DW0817`), and a
/// one-way connection that strands (`DW0819`).
///
/// # Why this runs at VALIDATION tier
///
/// It used to be raised from `compiler::analyze::analyze_campaign`, on the
/// argument that a tier is decided by the pass that raises it and these are
/// reachability questions like `DW0202`–`DW0204`. The argument sorts the check
/// by its KIND; what decides a tier here is when the check can fire, and by
/// that measure it was unreachable at the step it exists for. `delvec analyze`
/// returns at the first validation-tier error, and a campaign at the graph step
/// carries `DW0150` by construction — the plan is written and stage 5 is not —
/// so **no verb raised these proofs until stage 5 was written**, which is after
/// the design gate and after the expensive authoring the gate exists to
/// protect. spec-0049 §3 is explicit that the graph is checked "cheaply, before
/// geometry exists to make it expensive"; a proof that first fires two steps
/// later is not that.
///
/// Nothing here reads geometry: the whole battery is a function of the campaign
/// documents, which is why it can move. Called from [`check`] and from nowhere
/// else, so there is one battery in one place and no second copy of any rule; a
/// campaign with no layout graph returns nothing and [`LayoutBinding`] states
/// that zero.
#[must_use]
pub fn reachability(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return d;
    };
    let known: BTreeSet<&str> = graph.nodes.iter().map(|n| n.id.0.as_str()).collect();
    if !known.contains(graph.entry.0.as_str()) {
        return d; // `DW0814` refused it; a closure from nowhere says nothing.
    }
    let grants = Grants::of(c, graph);
    let closure = Closure::run(graph, &grants);
    let caveat = unwritten_mission_caveat(c, graph);
    unreached(graph, &closure, caveat, &mut d);
    critical_path(c, graph, &grants, caveat, &mut d);
    strands(graph, &closure, caveat, &mut d);
    d
}

/// The caveat a reachability refusal owes while **stage 5 is unwritten**.
///
/// [`Closure`] reads the mission for its flag grants ([`Grants::of`]), so a
/// campaign between the plan and the quests has no `set-flag` anywhere and every
/// flag-gated way in its graph is shut to this proof. Now that the battery runs
/// at validation tier that state is the ordinary one at the graph step, and
/// without this the proof reports a fault in a document the author finished two
/// steps ago and cannot yet answer. `mission` says the same thing about
/// `DW0818` in its own words and for the same reason; the shared authority is
/// the PREDICATE below, not the wording, because the two consequences differ.
///
/// A quest-gated way is NOT affected: `Grants::of` credits a quest once every
/// one of its beats sits at a reached place, and beats are the graph's own
/// document. Only `gating.flags` needs an effect stage 5 has not written.
///
/// It is a CAVEAT and never a dismissal. A place can be unreached, a path can
/// fail to be a path and a drop can strand for reasons the mission has nothing
/// to do with, and every one of those is a finding now — so it is attached only
/// where the mission's absence could be the cause (a graph that gates on a flag
/// at all) and it says which half is which.
fn unwritten_mission_caveat(c: &Campaign, graph: &LayoutGraphContent) -> &'static str {
    let gates_on_a_flag = graph
        .edges
        .iter()
        .any(|e| e.gating().is_some_and(|g| !g.flags.is_empty()));
    if c.quests.content.quests.is_empty() && gates_on_a_flag {
        " Stage 5 declares no quests here, so no `set-flag` effect exists yet and every \
         flag-gated way in this graph is shut to this proof: a place behind one is closed off \
         for that reason alone and opens when stage 5 is written (see `DW0150`). Anything \
         unreached for any OTHER reason is a real finding now, and this line does not say which \
         of the two you are looking at — the graph does."
    } else {
        ""
    }
}

/// `DW0816`: a node the closure never reaches.
fn unreached(graph: &LayoutGraphContent, closure: &Closure, caveat: &str, d: &mut Vec<Diagnostic>) {
    for (i, n) in graph.nodes.iter().enumerate() {
        let reached = closure.reached.contains(n.id.0.as_str());
        // Scenery (`reached: false`): the intent is confirmed the other way.
        if !n.reached {
            if reached {
                d.push(Diagnostic::error(
                    DW_NODE_UNREACHED,
                    "layout-graph",
                    format!("/content/nodes/{i}/reached"),
                    format!(
                        "place `{id}` is declared `reached: false` — scenery, built to be seen \
                         and never entered — and the closure reaches it from `{entry}`: a \
                         connection leads a body into it. Remove the connections that reach \
                         it (a vision edge is how a place is seen), or take `reached: false` \
                         off and make it a place a body visits.{caveat}",
                        id = n.id,
                        entry = graph.entry,
                    ),
                ));
            }
            continue;
        }
        if reached {
            continue;
        }
        let near = graph
            .edges
            .iter()
            .filter(|e| e.is_traversal())
            .find_map(|e| {
                let (a, b) = (e.a().0.as_str(), e.b().0.as_str());
                if a == n.id.0 && closure.reached.contains(b) {
                    Some(b)
                } else if b == n.id.0 && closure.reached.contains(a) {
                    Some(a)
                } else {
                    None
                }
            });
        let hint = match near {
            Some(other) => format!(
                "the nearest place a body can stand is `{other}`, so the missing link is between \
                 those two — either its gating demands something no reached beat grants, or it \
                 runs one way and the wrong way"
            ),
            None => "no connection reaches it from anywhere a body can stand at all".to_string(),
        };
        d.push(Diagnostic::error(
            DW_NODE_UNREACHED,
            "layout-graph",
            format!("/content/nodes/{i}"),
            format!(
                "place `{id}` is never reached: {hint}. Of the {total} place(s) this graph \
                 declares, {n} are reachable from `{entry}` under the campaign's own \
                 gating.{caveat}",
                id = n.id,
                total = graph.nodes.len(),
                n = closure.reached.len(),
                entry = graph.entry,
            ),
        ));
    }
}

/// `DW0817`: the authored critical path holds.
fn critical_path(
    c: &Campaign,
    graph: &LayoutGraphContent,
    grants: &Grants,
    caveat: &str,
    d: &mut Vec<Diagnostic>,
) {
    let path = &graph.critical_path;
    let mut fault = |path_suffix: &str, msg: String| {
        d.push(Diagnostic::error(
            DW_CRITICAL_PATH,
            "layout-graph",
            format!("/content/critical_path{path_suffix}"),
            msg,
        ));
    };
    if path.first() != Some(&graph.entry) || path.last() != Some(&graph.goal) {
        fault(
            "",
            format!(
                "the critical path must run from `{entry}` to `{goal}`; it runs from {from} to \
                 {to}. It is authored rather than derived precisely so that it is a claim, and a \
                 claim that does not start where a body starts is not one.",
                entry = graph.entry,
                goal = graph.goal,
                from = path.first().map_or("nowhere".into(), |n| format!("`{n}`")),
                to = path.last().map_or("nowhere".into(), |n| format!("`{n}`")),
            ),
        );
    }
    // Stepwise: each step is a real connection, run the right way, and openable
    // with what the beats already visited have granted.
    let mut held: BTreeSet<Grant> = BTreeSet::new();
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut steps = 0usize;
    for (i, pair) in path.windows(2).enumerate() {
        let (from, to) = (&pair[0], &pair[1]);
        visited.insert(from.0.as_str());
        collect_grants(grants, &visited, &mut held);
        let edge = graph.edges.iter().find(|e| {
            e.is_traversal()
                && ((e.a() == from && e.b() == to && e.direction() != Some(Direction::BToA))
                    || (e.b() == from && e.a() == to && e.direction() != Some(Direction::AToB)))
        });
        match edge {
            None => fault(
                &format!("/{}", i + 1),
                format!(
                    "the critical path steps from `{from}` to `{to}` and no connection runs that \
                     way. Either the two places share no edge at all, or the one they share runs \
                     the other way."
                ),
            ),
            Some(e) if !Closure::satisfied(e.gating(), &held) => {
                // The caveat rides only the GATING fault. A step with no
                // connection at all, a path that does not run entry to goal and
                // a missed spine beat are judgements about the graph alone, and
                // an unwritten mission has nothing to say about any of them.
                let unwritten = if e.gating().is_some_and(|g| !g.flags.is_empty()) {
                    caveat
                } else {
                    ""
                };
                fault(
                    &format!("/{}", i + 1),
                    format!(
                        "the critical path steps from `{from}` to `{to}` over `{e_id}`, which is \
                         not open yet at that point in the walk: nothing bound to the {v} \
                         place(s) already visited grants what it waits on. Move the beat that \
                         opens it earlier on the path, or route the path through the place that \
                         grants it.{unwritten}",
                        e_id = e.id(),
                        v = visited.len(),
                    ),
                );
            }
            Some(_) => {}
        }
        steps += 1;
    }
    if let Some(last) = path.last() {
        visited.insert(last.0.as_str());
    }
    // The spine obligation, and the zero binding that goes with it.
    let spine = c.quest_plan.content.spine();
    let mut required = 0usize;
    for beat in &graph.beats {
        if !spine.contains(beat.quest.0.as_str()) {
            continue;
        }
        required += 1;
        if !visited.contains(beat.node.0.as_str()) {
            fault(
                "",
                format!(
                    "beat `{q}` / `{o}` happens in `{n}`, which the critical path never visits — \
                     and `{q}` is on the mandatory spine, so a body walking this path would reach \
                     the goal without doing it.",
                    q = beat.quest,
                    o = beat.objective,
                    n = beat.node,
                ),
            );
        }
    }
    // The binding is STATED, not raised. `delvec analyze` exits 2 on any reported
    // diagnostic, warning or error alike, so a "this bound to nothing" line here
    // would turn a green analyze red — a count is not a fault. It lives in
    // [`LayoutBinding`] instead, which every run prints and which flags the zero
    // as a finding, and `steps` and `required` are quoted in the faults above so
    // a red says what it examined.
    let _ = (steps, required);
}

/// Everything the places visited so far have granted.
fn collect_grants(grants: &Grants, visited: &BTreeSet<&str>, held: &mut BTreeSet<Grant>) {
    for (node, given) in &grants.by_node {
        if visited.contains(node.as_str()) {
            held.extend(given.iter().cloned());
        }
    }
    for (quest, (nodes, given)) in &grants.by_quest {
        if nodes.iter().all(|n| visited.contains(n.as_str())) {
            held.insert(Grant::Quest(quest.clone()));
            held.extend(given.iter().cloned());
        }
    }
    // A gated producer can enable another; settle them in rounds.
    loop {
        let before = held.len();
        for gg in &grants.gated {
            let at = match &gg.holder {
                Holder::Node(n) => visited.contains(n.as_str()),
                Holder::Quest(q) => held.contains(&Grant::Quest(q.clone())),
            };
            if at && gg.requires.is_subset(held) {
                held.insert(gg.grant.clone());
            }
        }
        if held.len() == before {
            return;
        }
    }
}

/// `DW0819`: a one-way edge strands.
fn strands(graph: &LayoutGraphContent, closure: &Closure, caveat: &str, d: &mut Vec<Diagnostic>) {
    let spine: BTreeSet<&str> = graph.critical_path.iter().map(|n| n.0.as_str()).collect();
    for (i, e) in graph.edges.iter().enumerate() {
        if !e.is_traversal() {
            continue;
        }
        let Some(dir) = e.direction() else { continue };
        let (from, to) = match dir {
            Direction::AToB => (e.a(), e.b()),
            Direction::BToA => (e.b(), e.a()),
        };
        // A body can only be at `to` having been at `from`, holding at most what
        // it held on arriving at `from`.
        let Some(held) = closure.obtained_when.get(from.0.as_str()) else {
            continue; // `from` is unreachable; `DW0816` owns that.
        };
        if rejoins(graph, to, held, &spine) {
            continue;
        }
        d.push(Diagnostic::error(
            DW_ONE_WAY_STRANDS,
            "layout-graph",
            format!("/content/edges/{i}"),
            format!(
                "connection `{e_id}` runs one way from `{from}` into `{to}`, and from `{to}` \
                 there is no way back to the critical path. A body can only be in `{to}` having \
                 taken this connection, so a walk that takes it is a softlock. Add a way out of \
                 `{to}` — the shortcut back is the usual one — or make the connection \
                 two-way.{caveat}",
                e_id = e.id(),
            ),
        ));
    }
}

/// Can a body standing at `at`, holding `held`, get back to the spine?
fn rejoins(
    graph: &LayoutGraphContent,
    at: &NodeId,
    held: &BTreeSet<Grant>,
    spine: &BTreeSet<&str>,
) -> bool {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![at.0.as_str()];
    seen.insert(at.0.as_str());
    while let Some(here) = stack.pop() {
        if spine.contains(here) {
            return true;
        }
        for e in &graph.edges {
            if !e.is_traversal() || !Closure::satisfied(e.gating(), held) {
                continue;
            }
            let (a, b) = (e.a().0.as_str(), e.b().0.as_str());
            let next = if a == here && e.direction() != Some(Direction::BToA) {
                b
            } else if b == here && e.direction() != Some(Direction::AToB) {
                a
            } else {
                continue;
            };
            if seen.insert(next) {
                stack.push(next);
            }
        }
    }
    false
}
