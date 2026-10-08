//! Stage 4 — the quest plan: the planned quests, their dependencies, branch
//! points and happenings, and the spine they form.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AreaId, BranchId, BranchPointId, EndingId, FlagId, NpcId, QuestId};

#[cfg(doc)]
use crate::QuestEffect;

/// Stage 4 payload: the quest dependency plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestPlanContent {
    /// Planned quests (expanded in stage 5).
    pub quests: Vec<PlannedQuest>,
    /// The quest whose completion ends the campaign.
    pub finale: QuestId,
    /// The campaign's declared **story forks** (DSL v0.8, spec-0025). Empty/absent = a campaign that claims to have no
    /// branch — which the compiler then *verifies* rather than assumes: any flag
    /// that gates casts, staging or structure and is set on some playthroughs and
    /// not others belongs to no declared point and is `DW0480`.
    ///
    /// Enumerated branches are the **product of the declared points**, so the
    /// branch set is authored and small — never a combinatorial sweep of every
    /// flag in the campaign.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_points: Vec<BranchPoint>,
}

impl QuestPlanContent {
    /// **The ONE authority on which quests are the spine**: the finale and every
    /// quest its `depends_on` chain transitively demands — the quests a body
    /// cannot reach the finale without.
    ///
    /// The capability belongs here, on the stage-4 document, because the spine is
    /// a fact about the quest plan and about nothing else. It had grown two
    /// derivations of the same closure in two files — one inline in
    /// [`crate::validate`]'s `DW0132` convergence check, one a private
    /// `mandatory_quests` in [`crate::layout`] read by the layout binding and by
    /// the critical-path spine obligation. Both were correct and neither said it
    /// was the authority, which is exactly the shape a later clean merge turns
    /// into two rules that disagree.
    ///
    /// **Why the closure is taken over the raw `depends_on` edges, unfiltered.**
    /// The `validate` copy first dropped every dep naming a quest the plan does
    /// not declare. That filtering is not this function's question: a dangling
    /// `depends_on` is `DW0112`'s finding, and silently pruning it here would
    /// make the set disagree with the document it is derived from. So an id the
    /// plan does not declare is reported in the spine and expands no further —
    /// and the one reader that could care, `DW0132`, only ever asks whether a
    /// **declared** quest is a member, so an undeclared member cannot change its
    /// verdict.
    ///
    /// Cycle-safe by construction (a quest already in the set is not expanded
    /// again), so a plan `DW0130` will refuse still yields a set rather than
    /// hanging.
    ///
    /// **Not the same question as the `mandatory` field**, and the name says so
    /// deliberately. Today the two sets always coincide, because `DW0132` demands
    /// every declared quest be a transitive dependency of the finale and `DW0866`
    /// demands every quest set `mandatory: true`. If `mandatory: false` ever
    /// becomes legal those coincide no longer, and this function keeps answering
    /// the graph question it has always answered.
    #[must_use]
    pub fn spine(&self) -> BTreeSet<&str> {
        let deps: BTreeMap<&str, &[QuestId]> = self
            .quests
            .iter()
            .map(|q| (q.id.as_str(), q.depends_on.as_slice()))
            .collect();
        let mut spine: BTreeSet<&str> = BTreeSet::new();
        let mut stack = vec![self.finale.as_str()];
        while let Some(q) = stack.pop() {
            if !spine.insert(q) {
                continue;
            }
            for dep in deps.get(q).copied().unwrap_or(&[]) {
                stack.push(dep.as_str());
            }
        }
        spine
    }

    /// **The ONE authority on which quests are elective** (spec-0051): the
    /// quests declaring `mandatory: false`.
    ///
    /// The counterpart to [`Self::spine`], and deliberately a *different*
    /// question. `spine` asks what the graph demands; this asks what the author
    /// claims. `DW0866`/`DW0867` are exactly the rules that keep the two
    /// answers honest about each other, and they can only do that while each
    /// side has one derivation — which is the defect the spine function was
    /// created to end, and which a second private `!q.mandatory` filter in the
    /// compiler would re-introduce on the other half.
    #[must_use]
    pub fn optional(&self) -> BTreeSet<&str> {
        self.quests
            .iter()
            .filter(|q| !q.mandatory)
            .map(|q| q.id.as_str())
            .collect()
    }
}

/// One declared story fork (DSL v0.8, spec-0025).
///
/// A branch point names the flag set the story forks on, the quest at which the
/// fork opens, and every branch it offers. Each branch pins the point's whole
/// flag set: the flags it lists are **set**, and every other flag of `forks_on`
/// is **not** — which is what makes exclusive-content leakage (`DW0484`) and
/// per-branch cast resolution (`DW0483`) decidable instead of hopeful.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BranchPoint {
    /// Unique branch-point id.
    pub id: BranchPointId,
    /// The quest at which the fork opens — every branch's divergent content is at
    /// or after it in the stage-4 DAG.
    pub opens_at: QuestId,
    /// The flag set the story forks on. Every branch's `flags` is a subset of
    /// this, and the flags it does not list are pinned **unset** on that branch.
    pub forks_on: Vec<FlagId>,
    /// The alternatives (≥ 2).
    pub branches: Vec<BranchDecl>,
}

/// One alternative of a [`BranchPoint`] (DSL v0.8, spec-0025).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BranchDecl {
    /// Unique branch id (campaign-wide — it names the emitted chronicle file).
    pub id: BranchId,
    /// The subset of the point's `forks_on` that is SET on this branch. The rest
    /// of `forks_on` is pinned unset. An empty list is legal — the "took neither
    /// option" branch — as long as it is genuinely reachable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<FlagId>,
    /// Where this branch goes: either the `quest/<kebab>` the branches converge
    /// at, or the `ending/<kebab>` this branch runs to.
    ///
    /// One field, not two mutually exclusive ones, because the **id prefix
    /// already says which it is** — the same convention every cross-stage
    /// reference in the DSL uses. That is what makes "exactly one of them" an
    /// unrepresentable state rather than a rule some diagnostic has to police: a
    /// value that is neither a syntactically valid `quest/…` nor `ending/…` is
    /// the ordinary malformed-id `DW0110`, and one that names nothing is the
    /// ordinary dangling-reference `DW0112`.
    pub leads_to: String,
}

impl BranchDecl {
    /// The convergence quest, if [`Self::leads_to`] names one.
    pub fn converges_at(&self) -> Option<QuestId> {
        let q = QuestId(self.leads_to.clone());
        q.is_valid_syntax().then_some(q)
    }

    /// The ending, if [`Self::leads_to`] names one.
    pub fn ending(&self) -> Option<EndingId> {
        let e = EndingId(self.leads_to.clone());
        e.is_valid_syntax().then_some(e)
    }
}

/// What a story node does to the story (DSL v0.8, spec-0025).
///
/// The generalization of spec-0020's `doing` from NPC presence to event flow: a
/// design that never got written down node by node cannot compile. It is
/// **node-local on purpose** — there is no parallel per-branch script document
/// that could itself drift from the graph.
///
/// `text` is authoring/validation metadata, never shown to a player, so it is
/// deliberately **excluded from the l10n inventory** exactly like `doing`. The
/// compiler reads only `verb` and `subject`; `text` is the flesh the per-branch
/// chronicle is assembled from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Happening {
    /// The structured event verb — the machine-decidable part.
    pub verb: HappeningVerb,
    /// One line of prose stating what this node does to the story.
    pub text: String,
    /// What the event happens TO: an `npc/`, `actor/`, `wave/` or `anchor/` id
    /// (validated — a dangling one is `DW0112`), or an `item/<kebab>` label for a
    /// story token the campaign tracks by hand. Optional, because not every beat
    /// is about somebody; the hard-contradiction proof (`DW0485`) reasons only
    /// over the beats that name one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// The subject a beat is about, and whether the document said so (spec-0071 §3).
///
/// Answered by [`QuestEffect::happening_subject`], the one derivation. `derived`
/// is carried rather than dropped because the two readers want different things
/// from it: the namespace check reports on what an author **wrote**, and the
/// chronicle reasons over what the beat **is about**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HappeningSubject<'a> {
    /// The subject id (`npc/`, `actor/`, `wave/`, `anchor/` or an `item/` label).
    pub id: &'a str,
    /// `true` when the effect's own single object supplied it, `false` when the
    /// `happening` states it.
    pub derived: bool,
}

/// The structured event vocabulary (DSL v0.8, spec-0025).
///
/// Deliberately small and closed. These ten verbs are what make a subset of
/// narrative errors machine-decidable per branch (`DW0485`); everything else a
/// beat means lives in [`Happening::text`], which the compiler never interprets.
/// Extend only when a real campaign cannot state its beat with what is here.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum HappeningVerb {
    /// The subject is killed / destroyed. Terminal: nothing the subject does may
    /// follow it on the same branch.
    Dies,
    /// The subject comes through alive — the explicit counterpart of `dies`,
    /// which is what lets a branch state that somebody *did not* die.
    Survives,
    /// The subject leaves the stage (offstage, not dead). Cleared by `arrives`.
    Departs,
    /// The subject enters the stage.
    Arrives,
    /// Somebody learns a fact — the true-information beat.
    Learns,
    /// Somebody comes to believe something (whether or not it is true) — the beat
    /// that carries a wrong belief forward, which is where branch drift shows.
    Believes,
    /// The party (or the subject) gains a thing.
    Gains,
    /// The party (or the subject) loses a thing. A second `loses` with no
    /// intervening `gains` is spending what is already spent (`DW0485`).
    Loses,
    /// A way is opened.
    Opens,
    /// A way is sealed. Cleared by `opens`.
    Seals,
}

/// One planned quest (dependency-graph node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedQuest {
    /// Unique quest id.
    pub id: QuestId,
    /// Human-readable goal.
    pub goal: String,
    /// Area this quest takes place in (stage-1 ref).
    pub area: AreaId,
    /// NPCs involved (stage-2 refs).
    pub npcs: Vec<NpcId>,
    /// Prerequisite quests; edges must form a DAG.
    pub depends_on: Vec<QuestId>,
    /// `false` declares an optional quest (spec-0051).
    pub mandatory: bool,
    /// Act number (informational).
    pub act: u32,
}

#[cfg(test)]
mod spine_tests {
    use super::QuestPlanContent;

    /// Build a plan from `(id, deps)` pairs plus a finale. JSON rather than a
    /// struct literal on purpose: a field added to `PlannedQuest` later must not
    /// red these tests for a reason that has nothing to do with the spine.
    fn plan(finale: &str, quests: &[(&str, &[&str])]) -> QuestPlanContent {
        let quests: Vec<serde_json::Value> = quests
            .iter()
            .map(|(id, deps)| {
                serde_json::json!({
                    "id": id,
                    "goal": "g",
                    "area": "area/keep",
                    "npcs": [],
                    "depends_on": deps,
                    "mandatory": true,
                    "act": 1,
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "finale": finale,
            "quests": quests,
        }))
        .expect("plan fixture parses")
    }

    fn sorted(p: &QuestPlanContent) -> Vec<String> {
        p.spine().into_iter().map(str::to_owned).collect()
    }

    #[test]
    fn a_chain_is_wholly_spine() {
        let p = plan(
            "quest/c",
            &[
                ("quest/a", &[]),
                ("quest/b", &["quest/a"]),
                ("quest/c", &["quest/b"]),
            ],
        );
        assert_eq!(sorted(&p), ["quest/a", "quest/b", "quest/c"]);
    }

    #[test]
    fn a_quest_the_finale_does_not_depend_on_is_off_the_spine() {
        // Exactly the `DW0132` shape: the plan does not converge, and the spine
        // is the half that does. The authority answers, it does not refuse — the
        // refusal is `validate`'s, built on this answer.
        let p = plan("quest/end", &[("quest/end", &[]), ("quest/side-trip", &[])]);
        assert_eq!(sorted(&p), ["quest/end"]);
    }

    #[test]
    fn a_diamond_counts_the_join_once() {
        let p = plan(
            "quest/d",
            &[
                ("quest/a", &[]),
                ("quest/b", &["quest/a"]),
                ("quest/c", &["quest/a"]),
                ("quest/d", &["quest/b", "quest/c"]),
            ],
        );
        assert_eq!(sorted(&p), ["quest/a", "quest/b", "quest/c", "quest/d"]);
    }

    #[test]
    fn a_cycle_terminates_and_yields_a_set() {
        // `DW0130` refuses this plan, but the authority is asked before that
        // verdict is known (the layout binding prints on an erroring campaign),
        // so it must terminate rather than hang.
        let p = plan(
            "quest/b",
            &[("quest/a", &["quest/b"]), ("quest/b", &["quest/a"])],
        );
        assert_eq!(sorted(&p), ["quest/a", "quest/b"]);
    }

    #[test]
    fn a_dangling_dependency_is_reported_and_expands_no_further() {
        // The deliberate difference between the two derivations this function
        // replaced. `validate` pruned undeclared ids before walking; the
        // authority does not, because pruning them would make the set disagree
        // with the document, and naming an id the plan does not declare is
        // `DW0112`'s finding rather than the spine's.
        let p = plan("quest/end", &[("quest/end", &["quest/ghost"])]);
        assert_eq!(sorted(&p), ["quest/end", "quest/ghost"]);
    }

    #[test]
    fn an_undeclared_finale_is_the_whole_spine() {
        // `DW0131`'s shape. The answer is honest about the document: nothing the
        // plan declares is on the spine of a finale it never declared.
        let p = plan(
            "quest/ghost",
            &[("quest/a", &[]), ("quest/b", &["quest/a"])],
        );
        assert_eq!(sorted(&p), ["quest/ghost"]);
    }
}
