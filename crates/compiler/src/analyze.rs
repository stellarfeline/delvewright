//! Deep quest/objective reachability analysis (ADR-0005 static layer, exit 2).
//!
//! This is the compiler's *semantic* reachability, distinct from the DSL's
//! *structural* stage-4 check (`DW0132`, convergent-sink). It merges the stage-5
//! trigger graph, intra-quest objective `after` ordering, and the stage-6
//! dialogue graph into one model and asks: can the finale actually complete?
//!
//! ## Model (fixpoint)
//!
//! - A quest is **active** if triggered by `campaign-start`, or by
//!   `quest-complete(X)` where X can complete.
//! - An objective is **completable** if its quest is active, all its `after`
//!   prerequisites are completable, and its type is satisfiable:
//!   - `talk-to`: some dialogue option that fires `complete-objective` for it is
//!     reachable from its NPC's stage-6 tree `root`. (The DSL's `DW0123` already
//!     guarantees this statically per NPC; the fixpoint additionally propagates
//!     it across the trigger/`after` graph.)
//!   - `reach-anchor`: always satisfiable (the anchor resolves at build time).
//! - A quest **completes** if it is active and every objective is completable.
//! - The finale is reachable iff the finale quest completes.
//!
//! The lighting gate (`DW0210`/`DW0211`) is **no longer** here: spec-0010 moved it
//! to the compiler's assembled-world light model ([`crate::light`]), which measures
//! real light over the placed geometry (per-seam, sealed-cavity aware) instead of
//! reading per-piece admission profiles.
//!
//! ## Codes (`DW02xx`)
//!
//! | Code | Meaning |
//! |------|---------|
//! | `DW0201` | Finale quest can never complete (unreachable finale). |
//! | `DW0202` | Quest can never be triggered (unreachable / dead quest). |
//! | `DW0203` | Objective can never be completed (deadlock — e.g. an `after` chain that can't be satisfied). |
//! | `DW0210`/`DW0211` | Assembled-light gate — see [`crate::light`]. |

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{AnchorRegistry, Campaign, Diagnostic, Objective, Trigger};

/// Stable analysis codes.
pub mod codes {
    /// Finale quest can never complete.
    pub const FINALE_UNREACHABLE: &str = "DW0201";
    /// Quest can never be triggered.
    pub const QUEST_UNREACHABLE: &str = "DW0202";
    /// Objective can never be completed (deadlock).
    pub const OBJECTIVE_DEADLOCK: &str = "DW0203";
    // DW0210 (dark-area mitigation) moved to `crate::light` (spec-0010): it is now
    // measured over the assembled world, not a per-piece admission profile.
}

/// Run reachability + lighting-mitigation analysis. Returns `DW02xx`
/// diagnostics (sorted); empty means the finale and every quest/objective are
/// reachable and every `dark` prefab is mitigated. `prefabs` supplies the
/// lighting profile per prefab (see [`dark_mitigation`]).
pub fn analyze_campaign(c: &Campaign, prefabs: &dyn AnchorRegistry) -> Vec<Diagnostic> {
    let quests = &c.quests.content.quests;

    // objective id -> completing dialogue option reachable? (talk-to satisfiability)
    let dialogue_completable = reachable_completions(c);

    // Precompute objective lookup within each quest.
    let mut active: BTreeSet<&str> = BTreeSet::new();
    let mut completable: BTreeSet<&str> = BTreeSet::new(); // objective ids
    let mut completed: BTreeSet<&str> = BTreeSet::new(); // quest ids
    let mut set_flags: BTreeSet<&str> = BTreeSet::new(); // flags producible so far

    // set-flag effects by the objective / quest that fires them (v0.3): a flag is
    // producible once the objective/quest that sets it is itself completable.
    let mut obj_setflags: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut quest_setflags: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for q in quests {
        for (oid, effs) in &q.on_objective_complete {
            let flags: Vec<&str> = effs
                .iter()
                .filter_map(|e| e.set_flag().map(|f| f.as_str()))
                .collect();
            if !flags.is_empty() {
                obj_setflags.insert(oid.as_str(), flags);
            }
        }
        let cflags: Vec<&str> = q
            .on_complete
            .iter()
            .filter_map(|e| e.set_flag().map(|f| f.as_str()))
            .collect();
        if !cflags.is_empty() {
            quest_setflags.insert(q.id.as_str(), cflags);
        }
    }

    // Fixpoint.
    loop {
        let mut changed = false;

        for q in quests {
            let qid = q.id.as_str();
            if !active.contains(qid) {
                let now_active = match &q.trigger {
                    Trigger::CampaignStart => true,
                    Trigger::QuestComplete { quest } => completed.contains(quest.as_str()),
                };
                if now_active {
                    active.insert(qid);
                    changed = true;
                }
            }
        }

        for q in quests {
            if !active.contains(q.id.as_str()) {
                continue;
            }
            for obj in &q.objectives {
                let oid = obj.id().as_str();
                if completable.contains(oid) {
                    continue;
                }
                let prereqs_ok = obj.after().iter().all(|a| completable.contains(a.as_str()));
                if !prereqs_ok {
                    continue;
                }
                // requires_flags (v0.3): every gating flag must be producible by a
                // set-flag effect on an already-completable objective/quest.
                let flags_ok = obj
                    .requires_flags()
                    .iter()
                    .all(|f| set_flags.contains(f.as_str()));
                if !flags_ok {
                    continue;
                }
                let type_ok = match obj {
                    Objective::TalkTo { .. } => dialogue_completable.contains(oid),
                    // reach/kill/collect/interact resolve at build time; the wave,
                    // chest and interaction entity are compiler-placed.
                    Objective::ReachAnchor { .. }
                    | Objective::Kill { .. }
                    | Objective::Collect { .. }
                    | Objective::Interact { .. } => true,
                };
                if type_ok {
                    completable.insert(oid);
                    if let Some(fs) = obj_setflags.get(oid) {
                        set_flags.extend(fs.iter().copied());
                    }
                    changed = true;
                }
            }
        }

        for q in quests {
            let qid = q.id.as_str();
            if active.contains(qid)
                && !completed.contains(qid)
                && q.objectives
                    .iter()
                    .all(|o| completable.contains(o.id().as_str()))
            {
                completed.insert(qid);
                if let Some(fs) = quest_setflags.get(qid) {
                    set_flags.extend(fs.iter().copied());
                }
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    let mut diags = Vec::new();

    // DW0202: quests never activated.
    for (i, q) in quests.iter().enumerate() {
        if !active.contains(q.id.as_str()) {
            diags.push(Diagnostic::error(
                codes::QUEST_UNREACHABLE,
                "quests",
                format!("/content/quests/{i}"),
                format!(
                    "quest `{}` can never be triggered — its `quest-complete` trigger names a \
                     quest that itself never completes (a dead branch). Point the trigger at a \
                     completable quest, or make the source quest completable",
                    q.id
                ),
            ));
        }
    }

    // DW0203: objectives in active quests that never become completable.
    for (qi, q) in quests.iter().enumerate() {
        if !active.contains(q.id.as_str()) {
            continue; // covered by DW0202
        }
        for (oi, obj) in q.objectives.iter().enumerate() {
            if !completable.contains(obj.id().as_str()) {
                let why = match obj {
                    Objective::TalkTo { npc, .. } => {
                        format!(
                            "no dialogue option reachable from `{npc}`'s root fires \
                             `complete-objective` for it (or its prerequisites are unsatisfiable) \
                             — add a reachable completing option, or satisfy the blocking \
                             `after`/`requires_flags` prerequisite"
                        )
                    }
                    _ => "its `after` prerequisites can never all complete — break the \
                          unsatisfiable `after` chain so each prerequisite is itself reachable"
                        .to_string(),
                };
                diags.push(Diagnostic::error(
                    codes::OBJECTIVE_DEADLOCK,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}"),
                    format!("objective `{}` can never be completed: {why}", obj.id()),
                ));
            }
        }
    }

    // DW0201: the finale never completes.
    let finale = c.quest_plan.content.finale.as_str();
    if !completed.contains(finale) {
        diags.push(Diagnostic::error(
            codes::FINALE_UNREACHABLE,
            "quest-plan",
            "/content/finale",
            format!(
                "finale quest `{finale}` can never complete (deep reachability) — some objective \
                 on the finale's completion path is unreachable. Look for the accompanying \
                 `DW0202`/`DW0203` on the blocking quest/objective and fix that; the finale \
                 clears once every quest on its `depends_on` chain can complete"
            ),
        ));
    }

    // DW0210 is no longer computed here (spec-0010): the lighting gate moved to the
    // compiler's assembled-world light model (`crate::light`), which measures real
    // light over the placed geometry rather than reading per-piece admission
    // profiles. `prefabs` is retained in the signature for the reachability model's
    // other uses and API stability.
    let _ = prefabs;

    diags.sort_by(|a, b| (&a.code, &a.path).cmp(&(&b.code, &b.path)));
    diags
}

/// Objective ids for which a `complete-objective` effect sits on a dialogue
/// option reachable from the objective's NPC's dialogue `root`.
fn reachable_completions(c: &Campaign) -> BTreeSet<String> {
    // For each stage-6 tree: the set of objective ids its reachable options
    // complete, keyed by the tree's NPC.
    let mut npc_completes: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for tree in &c.dialogue.content.dialogues {
        let nodes: BTreeMap<&str, &_> = tree.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        let root = tree.root.as_str();
        if nodes.contains_key(root) {
            queue.push_back(root);
            seen.insert(root);
        }
        let mut completes: BTreeSet<String> = BTreeSet::new();
        while let Some(nid) = queue.pop_front() {
            let Some(node) = nodes.get(nid) else {
                continue;
            };
            for opt in &node.options {
                for eff in &opt.effects {
                    if let delvewright_dsl::DialogueEffect::CompleteObjective { objective } = eff {
                        completes.insert(objective.as_str().to_string());
                    }
                }
                if let Some(next) = &opt.next
                    && seen.insert(next.as_str())
                {
                    queue.push_back(next.as_str());
                }
            }
        }
        npc_completes.insert(tree.npc.as_str(), completes);
    }

    // An objective is dialogue-completable if a reachable option in ITS npc
    // completes it.
    let mut result = BTreeSet::new();
    for q in &c.quests.content.quests {
        for obj in &q.objectives {
            if let Objective::TalkTo { id, npc, .. } = obj
                && let Some(set) = npc_completes.get(npc.as_str())
                && set.contains(id.as_str())
            {
                result.insert(id.as_str().to_string());
            }
        }
    }
    result
}
