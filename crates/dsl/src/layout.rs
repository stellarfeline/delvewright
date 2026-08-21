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

use crate::diagnostic::{Diagnostic, DwCode};
use crate::envelope::Campaign;
use crate::ids::{EdgeId, FactId, FlagId, NodeId, ObjectiveId, QuestId};
use crate::metrics::{MetricKind, Metrics, Reads};
use crate::stages::Objective;

/// `DW0814`: the layout graph is not a graph.
///
/// `every_version`: the rule judges what the document SAYS — a duplicate id, an
/// endpoint naming no place, a self-loop, an `entry` that is not a node. Its
/// verdict is a function of the campaign alone, and there is no field below
/// `dsl_version` 0.14.0 in which to write any of it.
pub const DW_GRAPH_MALFORMED: DwCode = DwCode::every_version("DW0814");

/// `DW0816`: a node the closure never reaches.
pub const DW_NODE_UNREACHED: DwCode = DwCode::every_version("DW0816");

/// `DW0817`: the authored critical path does not hold.
pub const DW_CRITICAL_PATH: DwCode = DwCode::every_version("DW0817");

/// `DW0818`: the graph names quest-side state that does not exist, or a
/// place-bound beat has no place.
pub const DW_GRAPH_MISSION: DwCode = DwCode::every_version("DW0818");

/// `DW0819`: a one-way edge strands.
pub const DW_ONE_WAY_STRANDS: DwCode = DwCode::every_version("DW0819");

/// `DW0820`: a shortcut closes no loop.
pub const DW_SHORTCUT_NO_LOOP: DwCode = DwCode::every_version("DW0820");

/// `DW0822`: the pacing measurement — a projection, printed with no threshold.
pub const DW_PACING: DwCode = DwCode::every_version("DW0822");

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
    /// The size class this place is built to, naming a rung of the metrics
    /// table's ladder (`alcove`, `room`, `hall`, …). A name the table does not
    /// define is `DW0812`.
    pub size_class: String,
    /// Anything the reviewer needs that `intent` does not carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
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

/// Which side of a barred connection can open it.
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

/// What a body must already hold for a connection to be passable.
///
/// A **projection** of the campaign's runtime gating into topology, not the
/// campaign's [`Gate`](crate::gate::Gate) — see the module docs for why the two
/// are deliberately different objects, and for why this one carries no negative
/// flag term and no numeric comparison.
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
}

impl EdgeGating {
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
}

/// A connection between two places.
///
/// Internally tagged on `class`, in the same shape every other tagged union in
/// this DSL uses. The tagging is load-bearing rather than stylistic: `falls`
/// belongs to a drop and `opens_from` belongs to a barred way, and writing
/// either on a walk would be a declaration nothing reads. Under the tag, serde's
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
    /// A sealed connection some effect opens.
    Barred {
        /// Edge id (`edge/<kebab>`), unique within the graph.
        id: EdgeId,
        /// One end.
        a: NodeId,
        /// The other end.
        b: NodeId,
        /// Which side can open it. The one-side-openable door is spelled here,
        /// and it is a property of the connection rather than of any campaign's
        /// fiction.
        #[serde(default)]
        opens_from: OpensFrom,
        /// Declared directionality once open; absent means both ways.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        one_way: Option<Direction>,
        /// This connection exists to close a loop (`DW0820`).
        #[serde(default, skip_serializing_if = "is_false")]
        shortcut: bool,
        /// What opens it. **Required to say something** (`DW0818`): a barred way
        /// nothing opens is a wall, and a barred way anything opens is not
        /// barred.
        gating: EdgeGating,
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
            | Edge::Drop { id, .. }
            | Edge::Barred { id, .. }
            | Edge::Vision { id, .. } => id,
        }
    }

    /// The `a` end.
    #[must_use]
    pub fn a(&self) -> &NodeId {
        match self {
            Edge::Walk { a, .. }
            | Edge::Stair { a, .. }
            | Edge::Drop { a, .. }
            | Edge::Barred { a, .. }
            | Edge::Vision { a, .. } => a,
        }
    }

    /// The `b` end.
    #[must_use]
    pub fn b(&self) -> &NodeId {
        match self {
            Edge::Walk { b, .. }
            | Edge::Stair { b, .. }
            | Edge::Drop { b, .. }
            | Edge::Barred { b, .. }
            | Edge::Vision { b, .. } => b,
        }
    }

    /// The class name as the document spells it.
    #[must_use]
    pub fn class(&self) -> &'static str {
        match self {
            Edge::Walk { .. } => "walk",
            Edge::Stair { .. } => "stair",
            Edge::Drop { .. } => "drop",
            Edge::Barred { .. } => "barred",
            Edge::Vision { .. } => "vision",
        }
    }

    /// True if a body passes along this edge. A `vision` edge does not.
    #[must_use]
    pub fn is_traversal(&self) -> bool {
        !matches!(self, Edge::Vision { .. })
    }

    /// Which way a body may pass, or `None` for both ways (and for a `vision`
    /// edge, which carries none).
    #[must_use]
    pub fn direction(&self) -> Option<Direction> {
        match self {
            Edge::Walk { one_way, .. }
            | Edge::Stair { one_way, .. }
            | Edge::Barred { one_way, .. } => *one_way,
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
            | Edge::Drop { shortcut, .. }
            | Edge::Barred { shortcut, .. } => *shortcut,
            Edge::Vision { .. } => false,
        }
    }

    /// What a body must hold to pass, or `None` where the edge demands nothing.
    #[must_use]
    pub fn gating(&self) -> Option<&EdgeGating> {
        match self {
            Edge::Walk { gating, .. } | Edge::Stair { gating, .. } | Edge::Drop { gating, .. } => {
                gating.as_ref()
            }
            Edge::Barred { gating, .. } => Some(gating),
            Edge::Vision { .. } => None,
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
/// Exactly Dormans' loop, and exactly §3.2: beats grant, edges demand.
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
    pub fn satisfied(gating: Option<&EdgeGating>, held: &BTreeSet<Grant>) -> bool {
        let Some(g) = gating else { return true };
        g.flags
            .iter()
            .all(|f| held.contains(&Grant::Flag(f.0.clone())))
            && g.quest
                .as_ref()
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
    /// node id -> what standing there eventually yields.
    pub by_node: BTreeMap<String, BTreeSet<Grant>>,
    /// quest id -> (the nodes its beats sit in, what completing it yields).
    pub by_quest: BTreeMap<String, (BTreeSet<String>, BTreeSet<Grant>)>,
}

impl Grants {
    /// Derive the grants a graph's `beats[]` imply, from the campaign's quests
    /// and dialogue.
    ///
    /// A beat bound to an objective grants every flag the campaign sets when
    /// that objective completes, plus — for a `talk-to` — every flag reachable
    /// in the spoken-to NPC's dialogue tree, because the conversation happens
    /// where the speaker stands. Both walks reach nested bundles, so a
    /// `set-flag` inside a `sequence` step or an `on_respawn` hook counts
    /// exactly as a top-level one does.
    #[must_use]
    pub fn of(c: &Campaign, graph: &LayoutGraphContent) -> Grants {
        let mut g = Grants::default();
        // objective id (scoped by quest) -> the flags its completion sets.
        let mut on_objective: BTreeMap<(&str, &str), BTreeSet<Grant>> = BTreeMap::new();
        let mut on_quest: BTreeMap<&str, BTreeSet<Grant>> = BTreeMap::new();
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
        for q in &c.quests.content.quests {
            let mut done: BTreeSet<Grant> = BTreeSet::new();
            for eff in &q.on_complete {
                eff.visit_deep(&mut |e| {
                    if let Some(f) = e.set_flag() {
                        done.insert(Grant::Flag(f.0.clone()));
                    }
                });
            }
            on_quest.insert(q.id.0.as_str(), done);
            for (obj, effects) in &q.on_objective_complete {
                let entry = on_objective
                    .entry((q.id.0.as_str(), obj.0.as_str()))
                    .or_default();
                for eff in effects {
                    eff.visit_deep(&mut |e| {
                        if let Some(f) = e.set_flag() {
                            entry.insert(Grant::Flag(f.0.clone()));
                        }
                    });
                }
            }
            for obj in &q.objectives {
                if let Objective::TalkTo { id, npc, .. } = obj
                    && let Some(flags) = npc_flags.get(npc.0.as_str())
                {
                    on_objective
                        .entry((q.id.0.as_str(), id.0.as_str()))
                        .or_default()
                        .extend(flags.iter().cloned());
                }
            }
        }
        for beat in &graph.beats {
            let node = beat.node.0.clone();
            let given = on_objective
                .get(&(beat.quest.0.as_str(), beat.objective.0.as_str()))
                .cloned()
                .unwrap_or_default();
            g.by_node.entry(node.clone()).or_default().extend(given);
            let q = g
                .by_quest
                .entry(beat.quest.0.clone())
                .or_insert_with(|| (BTreeSet::new(), BTreeSet::new()));
            q.0.insert(node);
            q.1 = on_quest
                .get(beat.quest.0.as_str())
                .cloned()
                .unwrap_or_default();
        }
        g
    }
}

// ---------------------------------------------------------------------------
// The binding ledger
// ---------------------------------------------------------------------------

/// What a run's layout-graph checks bound to.
///
/// Stated on every run whether or not anything was found, because a count only
/// means something when the run that found nothing prints it too (CLAUDE.md). It
/// is one struct with one constructor, so the number the CLI prints, the number
/// the build ledger records and the number a diagnostic quotes cannot disagree.
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
    /// Names resolved into the metrics table — what `DW0812` examines.
    pub metric_refs: usize,
    /// Facts the geometry brief states. Nothing reads them at this version, and
    /// the count says so rather than the absence being implied.
    pub brief_facts: usize,
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
            return b;
        };
        b.nodes = graph.nodes.len();
        b.edges = graph.edges.len();
        b.beats = graph.beats.len();
        let spine = mandatory_quests(c);
        b.spine_beats = graph
            .beats
            .iter()
            .filter(|beat| spine.contains(beat.quest.0.as_str()))
            .count();
        b.path_steps = graph.critical_path.len().saturating_sub(1);
        b.metric_refs = graph.nodes.len();
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
        }
        b
    }

    /// One line, for stderr and for the round summary.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "layout-graph binding: {n} node(s), {e} edge(s) ({t} traversal, {ow} one-way, \
             {s} shortcut, {g} gated), {b} beat(s) of which {sb} on the mandatory spine, \
             {p} critical-path step(s), {m} metrics reference(s); geometry-brief binding: \
             {f} fact(s).",
            n = self.nodes,
            e = self.edges,
            t = self.traversal_edges,
            ow = self.one_way_edges,
            s = self.shortcut_edges,
            g = self.gated_edges,
            b = self.beats,
            sb = self.spine_beats,
            p = self.path_steps,
            m = self.metric_refs,
            f = self.brief_facts,
        )
    }
}

// ---------------------------------------------------------------------------
// Validation tier (spec-0049 §3.3): DW0814, DW0818, DW0820, DW0822, DW0812
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
    metric_names(graph, &table, d);
    if malformed {
        return;
    }
    mission(c, graph, d);
    shortcut_loops(graph, d);
    pacing(graph, &table, reads, d);
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

/// `DW0812`: every `size_class` names a rung the metrics table defines.
fn metric_names(graph: &LayoutGraphContent, table: &Metrics, d: &mut Vec<Diagnostic>) {
    for (i, n) in graph.nodes.iter().enumerate() {
        if let Err(unknown) = table.resolve(MetricKind::SizeClass, &n.size_class) {
            d.push(unknown.diagnostic("layout-graph", &format!("/content/nodes/{i}/size_class")));
        }
    }
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
                     says where the mission happens, so it can only name beats the mission has.",
                    beat.quest
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
                     declare.",
                    e_id = e.id(),
                ),
            ));
        }
        if matches!(e, Edge::Barred { .. }) && g.is_empty() {
            d.push(Diagnostic::error(
                DW_GRAPH_MISSION,
                "layout-graph",
                format!("/content/edges/{i}/gating"),
                format!(
                    "barred connection `{e_id}` says nothing about what opens it. A barred way \
                     with an empty `gating` is passable from world load, which is not barred; \
                     name the flag or the quest whose completion opens it, and `DW0818` then \
                     holds that name to something the campaign really produces.",
                    e_id = e.id(),
                ),
            ));
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
fn quest_index(c: &Campaign, q: &crate::stages::Quest) -> usize {
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
fn pacing(graph: &LayoutGraphContent, table: &Metrics, reads: &mut Reads, d: &mut Vec<Diagnostic>) {
    let mut blocks: u64 = 0;
    let mut legs = 0usize;
    for node_id in &graph.critical_path {
        let Some(node) = graph.nodes.iter().find(|n| &n.id == node_id) else {
            continue;
        };
        let Ok(entry) = table.resolve(MetricKind::SizeClass, &node.size_class) else {
            continue; // `DW0812` already refused the name.
        };
        if let crate::metrics::MetricValue::SizeClass(sc) = entry.value(reads) {
            blocks += u64::from(sc.nominal_traverse_blocks);
            legs += 1;
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
            "the critical path crosses {legs} place(s) over {steps} step(s), a nominal \
             {blocks} blocks of route, which at {rate} blocks of route per minute of play \
             projects to about {minutes} minute(s). This figure carries NO threshold and \
             refuses nothing: the coefficient it rests on is uncalibrated until the first \
             walked blockout and the first full playtest, and a threshold on a number that \
             uncertain would be defending nothing. It is printed so that the projection and \
             the measurement taken over the built world can be set side by side, which is \
             how the coefficient gets calibrated at all.",
            steps = graph.critical_path.len().saturating_sub(1),
            minutes = blocks.div_ceil(rate),
        ),
    ));
}

// ---------------------------------------------------------------------------
// Analysis tier (spec-0049 §3.3): DW0816, DW0817, DW0819
// ---------------------------------------------------------------------------

/// Every check the layout graph owes at **analysis** tier (exit 2).
///
/// Invoked from `compiler::analyze::analyze_campaign`, which is the one pass
/// `delvec analyze` and `delvec build` both run — so there is no path to a built
/// world that skips it. A campaign with no layout graph returns nothing, and the
/// binding count says so.
#[must_use]
pub fn analyze(c: &Campaign) -> Vec<Diagnostic> {
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
    unreached(graph, &closure, &mut d);
    critical_path(c, graph, &grants, &mut d);
    strands(graph, &closure, &mut d);
    d
}

/// `DW0816`: a node the closure never reaches.
fn unreached(graph: &LayoutGraphContent, closure: &Closure, d: &mut Vec<Diagnostic>) {
    for (i, n) in graph.nodes.iter().enumerate() {
        if closure.reached.contains(n.id.0.as_str()) {
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
                 declares, {n} are reachable from `{entry}` under the campaign's own gating.",
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
            Some(e) if !Closure::satisfied(e.gating(), &held) => fault(
                &format!("/{}", i + 1),
                format!(
                    "the critical path steps from `{from}` to `{to}` over `{e_id}`, which is not \
                     open yet at that point in the walk: nothing bound to the {v} place(s) \
                     already visited grants what it waits on. Move the beat that opens it earlier \
                     on the path, or route the path through the place that grants it.",
                    e_id = e.id(),
                    v = visited.len(),
                ),
            ),
            Some(_) => {}
        }
        steps += 1;
    }
    if let Some(last) = path.last() {
        visited.insert(last.0.as_str());
    }
    // The spine obligation, and the zero binding that goes with it.
    let spine = mandatory_quests(c);
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
}

/// The quests a body cannot reach the finale without: the finale and everything
/// its `depends_on` chain demands.
fn mandatory_quests(c: &Campaign) -> BTreeSet<&str> {
    let deps: BTreeMap<&str, &[QuestId]> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.0.as_str(), q.depends_on.as_slice()))
        .collect();
    let mut spine: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![c.quest_plan.content.finale.0.as_str()];
    while let Some(q) = stack.pop() {
        if !spine.insert(q) {
            continue;
        }
        for dep in deps.get(q).copied().unwrap_or(&[]) {
            stack.push(dep.0.as_str());
        }
    }
    spine
}

/// `DW0819`: a one-way edge strands.
fn strands(graph: &LayoutGraphContent, closure: &Closure, d: &mut Vec<Diagnostic>) {
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
                 `{to}` — the shortcut back is the usual one — or make the connection two-way.",
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
