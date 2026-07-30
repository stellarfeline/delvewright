//! Deep quest/objective reachability analysis (ADR-0005 static layer, exit 2).
//!
//! This is the compiler's *semantic* reachability, distinct from the DSL's
//! *structural* stage-4 check (`DW0132`, convergent-sink). It merges the stage-5
//! trigger graph, intra-quest objective `after` ordering, and the stage-2
//! dialogue graph into one model and asks: can the finale actually complete?
//!
//! ## Model (fixpoint)
//!
//! - A quest is **active** if triggered by `campaign-start`, or by
//!   `quest-complete(X)` where X can complete.
//! - An objective is **completable** if its quest is active, all its `after`
//!   prerequisites are completable, and its type is satisfiable:
//!   - `talk-to`: some dialogue option that fires `complete-objective` for it is
//!     reachable from its NPC's dialogue `root`.
//!   - `reach-anchor`: always satisfiable (the anchor resolves at build time).
//! - A quest **completes** if it is active and every objective is completable.
//! - The finale is reachable iff the finale quest completes.
//!
//! ## Codes (`DW02xx`)
//!
//! | Code | Meaning |
//! |------|---------|
//! | `DW0201` | Finale quest can never complete (unreachable finale). |
//! | `DW0202` | Quest can never be triggered (unreachable / dead quest). |
//! | `DW0203` | Objective can never be completed (deadlock — e.g. a `talk-to` with no reachable completing dialogue option, or an `after` chain that can't be satisfied). |

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{Campaign, Diagnostic, Objective, Trigger};

/// Stable analysis codes.
pub mod codes {
    /// Finale quest can never complete.
    pub const FINALE_UNREACHABLE: &str = "DW0201";
    /// Quest can never be triggered.
    pub const QUEST_UNREACHABLE: &str = "DW0202";
    /// Objective can never be completed (deadlock).
    pub const OBJECTIVE_DEADLOCK: &str = "DW0203";
}

/// Run reachability analysis. Returns `DW02xx` diagnostics (sorted); empty means
/// the finale and every quest/objective are reachable.
pub fn analyze_campaign(c: &Campaign) -> Vec<Diagnostic> {
    let quests = &c.quests.content.quests;

    // objective id -> completing dialogue option reachable? (talk-to satisfiability)
    let dialogue_completable = reachable_completions(c);

    // Precompute objective lookup within each quest.
    let mut active: BTreeSet<&str> = BTreeSet::new();
    let mut completable: BTreeSet<&str> = BTreeSet::new(); // objective ids
    let mut completed: BTreeSet<&str> = BTreeSet::new(); // quest ids

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
                let type_ok = match obj {
                    Objective::TalkTo { .. } => dialogue_completable.contains(oid),
                    Objective::ReachAnchor { .. } => true,
                    // Reserved types are rejected by DSL validation before analyze.
                    _ => false,
                };
                if type_ok {
                    completable.insert(oid);
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
                    "quest `{}` can never be triggered (its trigger's source never completes)",
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
                            "no dialogue option reachable from `{npc}`'s root fires `complete-objective` for it (or its prerequisites are unsatisfiable)"
                        )
                    }
                    _ => "its `after` prerequisites can never all complete".to_string(),
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
            format!("finale quest `{finale}` can never complete (deep reachability)"),
        ));
    }

    diags.sort_by(|a, b| (&a.code, &a.path).cmp(&(&b.code, &b.path)));
    diags
}

/// Objective ids for which a `complete-objective` effect sits on a dialogue
/// option reachable from the objective's NPC's dialogue `root`.
fn reachable_completions(c: &Campaign) -> BTreeSet<String> {
    // For each NPC: the set of objective ids its reachable options complete.
    let mut npc_completes: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for npc in &c.npcs.content.npcs {
        let nodes: BTreeMap<&str, &_> = npc
            .dialogue
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        let root = npc.dialogue.root.as_str();
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
                    let delvewright_dsl::DialogueEffect::CompleteObjective { objective } = eff;
                    completes.insert(objective.as_str().to_string());
                }
                if let Some(next) = &opt.next
                    && seen.insert(next.as_str())
                {
                    queue.push_back(next.as_str());
                }
            }
        }
        npc_completes.insert(npc.id.as_str(), completes);
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
