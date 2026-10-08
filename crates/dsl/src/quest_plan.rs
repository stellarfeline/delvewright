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

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::Verb;
use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::validate::{graph_has_cycle, produced_flags};

crate::dw_code! {
    /// Quest dependency cycle.
    pub const PLAN_CYCLE: DwCode = DwCode::new("DW0130", ExitTier::Build);
}

crate::dw_code! {
    /// `finale` is not a declared quest.
    pub const FINALE_UNKNOWN: DwCode = DwCode::new("DW0131", ExitTier::Build);
}

crate::dw_code! {
    /// `finale` is not the convergent sink of the plan: some declared quest is
    /// not a transitive dependency of it.
    ///
    /// **The name deliberately does not contain `FINALE_UNREACHABLE`, which
    /// belongs to `DW0201`.** That code says the finale can never complete; this
    /// one says nothing at all about the finale being reachable — in the fixture
    /// that raises it the finale completes perfectly well and a side trip hangs
    /// off the plan. Both are `DwCode`, so nothing but the name distinguishes
    /// them at a call site, and `tools/ci/check-dw-codes.py` credits a bare
    /// constant name mentioned in a crate's tests to **that crate's** code — so
    /// one shared name would buy coverage for whichever rule the file happens to
    /// sit next to.
    pub const PLAN_NOT_CONVERGENT: DwCode = DwCode::new("DW0132", ExitTier::Build);
}

crate::dw_code! {
    /// An optional quest inside the finale's dependency closure (spec-0051
    /// §8.1) — including a finale that declares itself optional.
    pub const OPTIONAL_ON_SPINE: DwCode = DwCode::new("DW0866", ExitTier::Build);
}

crate::dw_code! {
    /// A mandatory quest whose `depends_on` edge or stage-5 `quest-complete`
    /// trigger names an optional quest (spec-0051 §8.2).
    pub const MANDATORY_ON_OPTIONAL: DwCode = DwCode::new("DW0867", ExitTier::Build);
}

crate::dw_code! {
    /// A mandatory objective gated on a flag only an optional quest produces
    /// (spec-0051 §8.3) — the mainline key behind participation.
    ///
    /// The participation-minimal replay (`DW0204`) is the compensating stronger
    /// check behind it; this one refuses at the edge so the message can name
    /// the strand.
    pub const MAINLINE_KEY_OPTIONAL: DwCode = DwCode::new("DW0868", ExitTier::Build);
}

pub(crate) fn plan_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let plan = &c.quest_plan.content;
    let planned_ids: BTreeSet<&str> = plan.quests.iter().map(|q| q.id.as_str()).collect();

    // The partition (spec-0051). `spine()` is the ONE authority on which quests
    // the finale cannot fire without; `mandatory` is the author's claim about
    // the same set, and `optional()` is the ONE authority on the other half.
    let optional: BTreeSet<&str> = plan.optional();

    // Dependency edges (only to existing quests; dangling handled elsewhere).
    let edges: BTreeMap<&str, Vec<&str>> = plan
        .quests
        .iter()
        .map(|q| {
            let deps = q
                .depends_on
                .iter()
                .map(|x| x.as_str())
                .filter(|x| planned_ids.contains(x))
                .collect();
            (q.id.as_str(), deps)
        })
        .collect();
    let nodes: Vec<&str> = plan.quests.iter().map(|q| q.id.as_str()).collect();

    if graph_has_cycle(&nodes, &edges) {
        d.push(Diagnostic::error(
            PLAN_CYCLE,
            "quest-plan",
            "/content/quests",
            "stage-4 quest `depends_on` graph contains a cycle — the plan must be a DAG; remove a \
             `depends_on` edge so the quests form an acyclic order",
        ));
        return; // reachability is meaningless with a cycle
    }

    // Finale must be declared.
    if !planned_ids.contains(plan.finale.as_str()) {
        d.push(Diagnostic::error(
            FINALE_UNKNOWN,
            "quest-plan",
            "/content/finale",
            format!(
                "stage-4 `finale` `{}` is not a declared quest — set `finale` to the id of an \
                 existing planned quest (the one that ends the delve)",
                plan.finale
            ),
        ));
        return;
    }

    // Finale convergence: every quest must be a transitive dependency of the
    // finale (the plan converges on the finale). See README (spec ambiguity).
    //
    // The spine is asked of [`QuestPlanContent::spine`], which is the ONE
    // authority on it — the same function the layout binding and the
    // critical-path spine obligation read. This check used to derive the closure
    // itself, over `edges` (deps pruned to declared quests) rather than over the
    // raw `depends_on`; both derivations were correct and neither was named, so
    // nothing would have caught them drifting apart. The two sets differ only by
    // ids the plan does not declare, which is `DW0112`'s finding and not this
    // one's, and which cannot move this verdict because the membership below is
    // only ever asked about a DECLARED quest.
    let reach = plan.spine();
    for (i, q) in plan.quests.iter().enumerate() {
        // Below the fence `optional` is empty, so this is every quest and the
        // message is the one it has always been. At and above it, the rule is
        // the MANDATORY half of spec-0051 §2.4's mismatch pair: a quest that
        // claims to be on the critical path and is not reachable from the
        // finale is still the wiring mistake it always was — it does not
        // silently become optional content. The other half (an optional quest
        // the closure does reach) is `DW0866`, because those are opposite
        // errors and a shared message could prescribe neither.
        if optional.contains(q.id.as_str()) {
            continue;
        }
        if !reach.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                PLAN_NOT_CONVERGENT,
                "quest-plan",
                format!("/content/quests/{i}"),
                format!(
                    "quest `{}` is not a (transitive) dependency of finale `{}`, so the plan does \
                     not converge on the finale — add a `depends_on` chain so `{}` eventually \
                     depends on `{}` (or drop `{}` if it is not part of this delve)",
                    q.id, plan.finale, plan.finale, q.id, q.id
                ),
            ));
        }
    }

    partition(c, &optional, &reach, d);
}

/// The partition refusals of spec-0051 §8.1–2: the two ways a declared-optional
/// quest can be a lie about the completion proof.
///
/// Both are edge-shaped and both are refused where the author can see the edge.
/// They are separate codes because they prescribe opposite repairs — one says
/// *this is not really optional*, the other says *this dependency is not really
/// mandatory* — and a campaign can trip either without the other.
///
/// Inert on a campaign that declares no optional quest: `optional` is empty, so
/// every loop below ranges over nothing.
fn partition(
    c: &Campaign,
    optional: &BTreeSet<&str>,
    spine: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    if optional.is_empty() {
        return;
    }
    let plan = &c.quest_plan.content;

    // §8.1 — the finale leans on it. An optional quest the finale cannot fire
    // without is not optional; the declaration would be a lie the proof then
    // rests on. Covers the finale itself: `spine()` contains it, so a finale
    // declared `mandatory: false` lands here rather than needing its own rule.
    for (i, q) in plan.quests.iter().enumerate() {
        if !optional.contains(q.id.as_str()) || !spine.contains(q.id.as_str()) {
            continue;
        }
        let how = if q.id.as_str() == plan.finale.as_str() {
            "it IS the finale".to_string()
        } else {
            format!("finale `{}` transitively depends on it", plan.finale)
        };
        d.push(Diagnostic::error(
            OPTIONAL_ON_SPINE,
            "quest-plan",
            format!("/content/quests/{i}/mandatory"),
            format!(
                "quest `{}` declares `mandatory: false`, but {} — so the delve cannot be \
                 completed without it and calling it optional would be a claim the \
                 completability proof then rests on. Set `mandatory: true`, or cut the \
                 `depends_on` chain that puts it in the finale's closure. Do not leave it for \
                 the proof to sort out: the skip world is exactly the world in which this \
                 quest is never played, and the finale never fires there",
                q.id, how
            ),
        ));
    }

    // §8.2 — the mainline hangs off it. Refused at the EDGE, naming the edge,
    // for both edge kinds a quest has: the stage-4 `depends_on` graph and the
    // stage-5 `quest-complete` trigger. One rule ("a mandatory quest may not
    // wait on elective content"), so one code; the message names which edge.
    for (i, q) in plan.quests.iter().enumerate() {
        if optional.contains(q.id.as_str()) {
            continue; // optional-on-optional and optional-on-mandatory are legal (§4)
        }
        for (j, dep) in q.depends_on.iter().enumerate() {
            if !optional.contains(dep.as_str()) {
                continue;
            }
            d.push(Diagnostic::error(
                MANDATORY_ON_OPTIONAL,
                "quest-plan",
                format!("/content/quests/{i}/depends_on/{j}"),
                format!(
                    "mandatory quest `{}` declares `depends_on` `{}`, which is optional — a \
                     quest on the critical path cannot wait on content the party may never \
                     play, so this edge makes the mainline unreachable in the skip world. \
                     Either mark `{}` mandatory, or drop the edge and attach `{}` to the \
                     spine some other way",
                    q.id, dep, dep, dep
                ),
            ));
        }
    }

    // The same rule over the stage-5 activation edge. `depends_on` orders the
    // plan; the trigger is what actually arms the quest at runtime, and nothing
    // ties the two together (a `quest-complete` trigger is resolved against the
    // stage-5 quest set, never against stage 4). So a campaign can spell this
    // edge with the trigger alone, and the `depends_on` loop above would not
    // see it.
    let declared: BTreeSet<&str> = plan.quests.iter().map(|q| q.id.as_str()).collect();
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if optional.contains(q.id.as_str()) || !declared.contains(q.id.as_str()) {
            continue;
        }
        let crate::Trigger::QuestComplete { quest } = &q.trigger else {
            continue;
        };
        if !optional.contains(quest.as_str()) {
            continue;
        }
        d.push(Diagnostic::error(
            MANDATORY_ON_OPTIONAL,
            "quests",
            format!("/content/quests/{i}/trigger/quest"),
            format!(
                "mandatory quest `{}` is triggered by the completion of `{}`, which is \
                 optional — the party may never complete `{}`, so `{}` would never activate \
                 and the mainline would stop there. Trigger `{}` from a mandatory quest, or \
                 mark `{}` mandatory",
                q.id, quest, quest, q.id, q.id, quest
            ),
        ));
    }

    mainline_key(c, optional, d);
}

/// spec-0051 §8.3 — **a mainline key behind participation**: a mandatory
/// objective gated on a flag every producer of which is rooted in an optional
/// quest.
///
/// Refused **at the edge**, naming the objective, the flag and the optional-only
/// producers, because that is where an author can act. The
/// participation-minimal replay (`DW0204`) remains the compensating stronger
/// check behind it, exactly as it already backstops the negative-gate fixpoint:
/// the replay credits only the exported path's own producers, so this shape
/// fails there too. What the edge buys is a message that names the strand
/// instead of a walk that stops.
///
/// **The producer partition is conservative in the safe direction.** A flag is
/// optional-only when EVERY root that sets it is an optional quest's bundle;
/// a single producer anywhere else — a mandatory quest, an environment trigger,
/// a trap disarm, a dialogue option, `on_death` — takes the flag out of the set.
/// Dialogue is counted as non-optional deliberately: whether an option is
/// reachable only inside an optional quest's scene is a cast-ladder question
/// this rule cannot answer, and answering it wrongly here would refuse a
/// correct campaign. `DW0204` can answer it, and does.
///
/// **Not yet covered, and named rather than implied**: the `requires_state` and
/// `dropped_by` chains of §8.3. Both are real shapes — an item that drops only
/// from a wave an optional quest spawns is the example the spec gives — and
/// both are still caught by `DW0204`, one step later and with a worse message.
fn mainline_key(c: &Campaign, optional: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) {
    // flag -> the optional quests that set it, while nothing else does.
    let mut only_optional: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut disqualified: BTreeSet<&str> = BTreeSet::new();

    crate::for_each_campaign_effect(c, &mut |_path, site, eff| {
        let Verb::SetFlag { flag, .. } = &eff.verb else {
            return;
        };
        let flag = flag.as_str();
        let owner = match site {
            crate::EffectSite::Objective { quest, .. }
            | crate::EffectSite::QuestComplete { quest } => quest.as_str(),
            // Every other root is ambient or dialogue-hosted: not a quest, so
            // not "optional participation" in this rule's sense.
            _ => {
                disqualified.insert(flag);
                return;
            }
        };
        match optional.get(owner) {
            Some(q) => only_optional.entry(flag).or_default().insert(*q),
            None => disqualified.insert(flag),
        };
    });
    for t in &c.dialogue.content.dialogues {
        for n in &t.nodes {
            for o in &n.options {
                for e in &o.effects {
                    if let crate::DialogueEffect::SetFlag { flag } = e {
                        disqualified.insert(flag.as_str());
                    }
                }
            }
        }
    }
    for trap in &c.quests.content.traps {
        if let Some(dis) = &trap.disarm {
            disqualified.insert(dis.sets_flag.as_str());
        }
    }

    // A mandatory quest's objective gated on such a flag.
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if optional.contains(q.id.as_str()) {
            continue;
        }
        for (j, o) in q.objectives.iter().enumerate() {
            for (m, f) in o.requires_flags().iter().enumerate() {
                let flag = f.as_str();
                if disqualified.contains(flag) {
                    continue;
                }
                let Some(producers) = only_optional.get(flag) else {
                    continue; // never produced at all: `DW0172`'s finding, not this one's
                };
                let names = producers.iter().copied().collect::<Vec<_>>().join("`, `");
                d.push(Diagnostic::error(
                    MAINLINE_KEY_OPTIONAL,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/requires_flags/{m}"),
                    format!(
                        "objective `{}` of mandatory quest `{}` requires flag `{}`, and the \
                         only effect that ever sets `{}` is rooted in optional quest(s) \
                         `{}` — so a party that plays only the mainline can never open \
                         this beat, and the delve is not completable with zero optional \
                         participation. Move the `set-flag` onto a mandatory quest, mark \
                         the producing quest mandatory, or drop the gate",
                        o.id(),
                        q.id,
                        flag,
                        flag,
                        names
                    ),
                ));
            }
        }
    }
}

/// Structural validation of the stage-4 `branch_points` declaration (spec-0025).
///
/// Everything here reuses the DSL's existing structural codes on purpose — a
/// branch point is an ordinary declaration with ordinary ids, so a malformed id
/// is `DW0110`, a repeated one `DW0111`, and a reference to something that does
/// not exist `DW0112`. The `DW048x` block is reserved for what is genuinely new:
/// proofs *about* branches.
pub(crate) fn branch_point_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests: BTreeSet<&str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    let endings: BTreeSet<String> = declared_endings(c);
    let flags: BTreeSet<String> = produced_flags(c);
    let mut seen_points: BTreeSet<&str> = BTreeSet::new();
    let mut seen_branches: BTreeSet<&str> = BTreeSet::new();

    for (i, bp) in c.quest_plan.content.branch_points.iter().enumerate() {
        let base = format!("/content/branch_points/{i}");
        if !bp.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quest-plan",
                format!("{base}/id"),
                format!(
                    "`{}` is not a valid branch-point id — use `branch-point/<kebab-case>`",
                    bp.id.as_str()
                ),
            ));
        } else if !seen_points.insert(bp.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quest-plan",
                format!("{base}/id"),
                format!("duplicate branch-point id `{}`", bp.id.as_str()),
            ));
        }
        if !quests.contains(bp.opens_at.as_str()) {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quest-plan",
                format!("{base}/opens_at"),
                format!(
                    "branch point `{}` opens at `{}`, which is not a planned quest — name the \
                     quest at which the story actually forks",
                    bp.id.as_str(),
                    bp.opens_at.as_str()
                ),
            ));
        }
        for (j, f) in bp.forks_on.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quest-plan",
                    format!("{base}/forks_on/{j}"),
                    format!(
                        "branch point `{}` forks on `{}`, which no `set-flag` effect produces — a \
                         fork nothing can set is not a fork",
                        bp.id.as_str(),
                        f.as_str()
                    ),
                ));
            }
        }
        let fork_set: BTreeSet<&str> = bp.forks_on.iter().map(|f| f.as_str()).collect();
        for (j, b) in bp.branches.iter().enumerate() {
            let bpath = format!("{base}/branches/{j}");
            if !b.id.is_valid_syntax() {
                d.push(Diagnostic::error(
                    codes::ID_SYNTAX,
                    "quest-plan",
                    format!("{bpath}/id"),
                    format!(
                        "`{}` is not a valid branch id — use `branch/<kebab-case>`",
                        b.id.as_str()
                    ),
                ));
            } else if !seen_branches.insert(b.id.as_str()) {
                d.push(Diagnostic::error(
                    codes::ID_DUPLICATE,
                    "quest-plan",
                    format!("{bpath}/id"),
                    format!(
                        "duplicate branch id `{}` — branch ids are campaign-wide unique because \
                         each one names an emitted `validation/branch-chronicle-<id>.md`",
                        b.id.as_str()
                    ),
                ));
            }
            for (k, f) in b.flags.iter().enumerate() {
                if !fork_set.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::DANGLING_REF,
                        "quest-plan",
                        format!("{bpath}/flags/{k}"),
                        format!(
                            "branch `{}` holds `{}`, which its branch point does not list in \
                             `forks_on` — a branch may only pin flags its own fork owns",
                            b.id.as_str(),
                            f.as_str()
                        ),
                    ));
                }
            }
            match (b.converges_at(), b.ending()) {
                (Some(q), _) => {
                    if !quests.contains(q.as_str()) {
                        d.push(Diagnostic::error(
                            codes::DANGLING_REF,
                            "quest-plan",
                            format!("{bpath}/leads_to"),
                            format!(
                                "branch `{}` converges at `{}`, which is not a planned quest",
                                b.id.as_str(),
                                q.as_str()
                            ),
                        ));
                    }
                }
                (None, Some(e)) => {
                    if !endings.contains(e.as_str()) {
                        d.push(Diagnostic::error(
                            codes::DANGLING_REF,
                            "quest-plan",
                            format!("{bpath}/leads_to"),
                            format!(
                                "branch `{}` runs to `{}`, which no `campaign-complete` effect \
                                 declares — name the ending on the `campaign-complete` that ends \
                                 this branch",
                                b.id.as_str(),
                                e.as_str()
                            ),
                        ));
                    }
                }
                (None, None) => d.push(Diagnostic::error(
                    codes::ID_SYNTAX,
                    "quest-plan",
                    format!("{bpath}/leads_to"),
                    format!(
                        "`{}` is neither a `quest/<kebab>` (the branches converge there) nor an \
                         `ending/<kebab>` (this branch runs to it) — the prefix is what says which \
                         one a branch leads to",
                        b.leads_to
                    ),
                )),
            }
        }
    }
}

/// Dangling-subject check for every `happening` (spec-0025). A subject naming an
/// `npc/`, `actor/`, `wave/` or `anchor/` id must resolve; an `item/<kebab>`
/// label is a free namespace for a story token the campaign tracks by hand, and
/// anything else is a malformed id.
pub(crate) fn happening_subject_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let npcs: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let actors: BTreeSet<&str> = c
        .quests
        .content
        .actors
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    let waves: BTreeSet<&str> = c
        .quests
        .content
        .waves
        .iter()
        .map(|w| w.id.as_str())
        .collect();
    let check = |subject: &str, stage: &str, path: String, d: &mut Vec<Diagnostic>| {
        let known = match subject.split_once('/') {
            Some(("npc", _)) => npcs.contains(subject),
            Some(("actor", _)) => actors.contains(subject),
            Some(("wave", _)) => waves.contains(subject),
            // Anchors resolve against prefab metadata far downstream (pool areas
            // are drawn at build time), so the DSL only polices the namespace.
            Some(("anchor", _)) | Some(("item", _)) => true,
            _ => false,
        };
        if !known {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                stage,
                path,
                format!(
                    "`happening.subject` names `{subject}`, which is not a declared `npc/`, \
                     `actor/` or `wave/` id (`anchor/` and `item/` labels are also accepted). A \
                     subject the compiler cannot resolve cannot be reasoned about, so the \
                     contradiction proof would silently skip this beat"
                ),
            ));
        }
    };
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if let Some(h) = &q.happening
            && let Some(s) = &h.subject
        {
            check(
                s,
                "quests",
                format!("/content/quests/{i}/happening/subject"),
                d,
            );
        }
        for (j, o) in q.objectives.iter().enumerate() {
            if let Some(h) = o.happening()
                && let Some(s) = &h.subject
            {
                check(
                    s,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/happening/subject"),
                    d,
                );
            }
        }
    }
    let mut effect_subjects: Vec<(String, String)> = Vec::new();
    crate::for_each_campaign_effect(c, &mut |path, _site, eff| {
        // The one derivation (spec-0071 §3), read here exactly as the chronicle
        // reads it. Only a **stated** subject is policed: a derived one is the
        // effect's own `anchor`/`npc`/`actor`/`wave` reference, already refused
        // by kind where it is written, and a second report would point the
        // author at a `happening/subject` the document does not have.
        if let Some(s) = eff.happening_subject().filter(|s| !s.derived) {
            effect_subjects.push((format!("{path}/happening/subject"), s.id.to_string()));
        }
    });
    for (path, s) in effect_subjects {
        check(&s, "quests", path, d);
    }
    for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, n) in t.nodes.iter().enumerate() {
            for (k, o) in n.options.iter().enumerate() {
                if let Some(h) = &o.happening
                    && let Some(s) = &h.subject
                {
                    check(
                        s,
                        "dialogue",
                        format!("/content/dialogues/{i}/nodes/{j}/options/{k}/happening/subject"),
                        d,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Every ending id some `campaign-complete` declares. There is no separate
/// declaration list — the same rule flags follow.
pub fn declared_endings(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_p, _site, eff| {
        if let Verb::CampaignComplete {
            ending: Some(e), ..
        } = &eff.verb
        {
            out.insert(e.as_str().to_string());
        }
    });
    out
}

/// `DW0110` over the planned quests' ids.
pub(crate) fn plan_id_syntax(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        crate::ids::id_syntax!(d, q.id, "quest-plan", format!("/content/quests/{i}/id"));
    }
}

/// `DW0111` over the planned quests' ids.
pub(crate) fn plan_id_uniqueness(c: &Campaign, d: &mut Vec<Diagnostic>) {
    crate::ids::dup_check(
        c.quest_plan
            .content
            .quests
            .iter()
            .enumerate()
            .map(|(i, q)| (q.id.as_str(), format!("/content/quests/{i}/id"))),
        "quest-plan",
        "quest",
        d,
    );
}

/// `DW0112` over what a planned quest names: its area, its NPCs, and the
/// quests it `depends_on`.
pub(crate) fn plan_dangling_refs(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::ids::dangling;
    let area_ids = crate::world::declared_area_ids(c);
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let planned_ids: BTreeSet<&str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(q.area.as_str()),
            "quest-plan",
            format!("/content/quests/{i}/area"),
            format!(
                "quest references unknown area `{}` — {}",
                q.area,
                crate::placement::Placement::of(c).area_remedy(),
            ),
        );
        for (k, npc) in q.npcs.iter().enumerate() {
            dangling(
                d,
                npc_ids.contains(npc.as_str()),
                "quest-plan",
                format!("/content/quests/{i}/npcs/{k}"),
                format!(
                    "quest references unknown npc `{npc}` — declare it in stage 2 or correct the \
                     reference"
                ),
            );
        }
        for (k, dep) in q.depends_on.iter().enumerate() {
            dangling(
                d,
                planned_ids.contains(dep.as_str()),
                "quest-plan",
                format!("/content/quests/{i}/depends_on/{k}"),
                format!(
                    "quest depends on unknown quest `{dep}` — declare it in the stage-4 quest \
                     plan or correct the `depends_on` entry"
                ),
            );
        }
    }
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
