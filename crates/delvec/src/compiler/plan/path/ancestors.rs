//! Strict ancestor steps: which steps are guaranteed to precede which.

use super::*;

/// Transitive-reachability closure over a `node → direct successors` adjacency,
/// seeded by `start` (exclusive of the seeds' own membership only insofar as they
/// re-enter via the graph). Shared by the quest-`depends_on` and objective-`after`
/// ancestor computations.
fn transitive_closure<'a>(
    start: &[&'a str],
    next: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut seen: BTreeSet<&'a str> = BTreeSet::new();
    let mut stack: Vec<&'a str> = start.to_vec();
    while let Some(x) = stack.pop() {
        if seen.insert(x)
            && let Some(nx) = next.get(x)
        {
            stack.extend(nx.iter().copied());
        }
    }
    seen
}

/// For each `critical_path` **arrival** step, the set of steps of its **strict DAG
/// ancestors** (see [`Plan::strict_ancestor_steps`]).
///
/// An objective's own step takes the transitive `after`-closure within its quest,
/// plus every objective of every transitive `depends_on`-ancestor quest — a quest
/// completes, all its objectives, before any dependent quest starts.
///
/// An arrival **past the last objective** — the completion assertion, and the
/// one-past-the-end sentinel a sweep to `steps` inclusive finishes on — takes every
/// objective on the path. The exported path is rooted at the finale
/// (`flow::finale_order`), so it holds exactly the quests campaign completion
/// depends on, and the assertion cannot hold until all of them do.
///
/// Pure DAG structure, so it is deterministic and independent of the lineariser's
/// choice among valid orders. `steps` is the critical path's length.
///
/// A `trigger` step (`trigger_step`) is a path act rather than a DAG node: it
/// takes what precedes the objective it is performed in front of, and it
/// precedes every later step on the path — the arrivals past the last objective
/// included.
pub(in crate::compiler::plan) fn compute_strict_ancestor_steps(
    campaign: &Campaign,
    obj_step: &BTreeMap<String, usize>,
    trigger_step: &BTreeMap<String, usize>,
    steps: usize,
) -> BTreeMap<usize, BTreeSet<usize>> {
    // Quest direct `depends_on`, then its transitive-ancestor closure.
    let quest_deps: BTreeMap<&str, Vec<&str>> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| {
            (
                q.id.as_str(),
                q.depends_on.iter().map(|d| d.as_str()).collect(),
            )
        })
        .collect();
    let quest_anc: BTreeMap<&str, BTreeSet<&str>> = quest_deps
        .iter()
        .map(|(q, deps)| (*q, transitive_closure(deps, &quest_deps)))
        .collect();

    // Objective structure from stage 5: quest→objectives, objective→quest, `after`.
    let mut quest_objs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut obj_quest: BTreeMap<&str, &str> = BTreeMap::new();
    let mut obj_after: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for q in &campaign.quests.content.quests {
        let qid = q.id.as_str();
        for o in &q.objectives {
            let oid = o.id().as_str();
            quest_objs.entry(qid).or_default().push(oid);
            obj_quest.insert(oid, qid);
            obj_after.insert(oid, o.after().iter().map(|a| a.as_str()).collect());
        }
    }
    let after_closure: BTreeMap<&str, BTreeSet<&str>> = obj_after
        .iter()
        .map(|(o, a)| (*o, transitive_closure(a, &obj_after)))
        .collect();

    // Assemble the step-level ancestor sets.
    let mut out: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (&oid, &qid) in &obj_quest {
        let Some(&s) = obj_step.get(oid) else {
            continue;
        };
        let mut anc: BTreeSet<usize> = BTreeSet::new();
        let add = |name: &str, anc: &mut BTreeSet<usize>| {
            if let Some(&st) = obj_step.get(name) {
                anc.insert(st);
            }
        };
        if let Some(cl) = after_closure.get(oid) {
            for a in cl {
                add(a, &mut anc);
            }
        }
        if let Some(aq) = quest_anc.get(qid) {
            for q2 in aq {
                if let Some(objs) = quest_objs.get(*q2) {
                    for a in objs {
                        add(a, &mut anc);
                    }
                }
            }
        }
        out.insert(s, anc);
    }
    // A `trigger` step is a path act, not a DAG node: nothing orders it but the
    // path that performs it. So it precedes every step after it on that path —
    // the walk the proofs judge IS this path — and what precedes it is what
    // precedes the objective it is performed in front of, since the party
    // arrives at it on the way to that objective.
    let mut trigger_anc: Vec<(usize, BTreeSet<usize>)> = Vec::new();
    for &t in trigger_step.values() {
        let next = obj_step.values().copied().filter(|&o| o > t).min();
        let mut anc = next.and_then(|n| out.get(&n)).cloned().unwrap_or_default();
        anc.retain(|&a| a < t);
        trigger_anc.push((t, anc));
    }
    out.extend(trigger_anc);

    // ---- the arrivals past the last objective ----
    //
    // The critical path is `[select-class, objective…, assert-complete]`, and its
    // consumers sweep arrivals to `steps` INCLUSIVE — one past the end, the
    // sentinel `crate::compiler::nav::reachable_under_every_quest_state` finishes
    // on. Neither the completion assertion nor that sentinel is an objective, so
    // these rows are what keeps a lookup from answering "nothing has fired yet" at
    // the two arrivals where EVERYTHING has fired. The exported path is rooted at
    // the finale (`flow::finale_order`), so it holds exactly the quests campaign
    // completion depends on: every objective on it is done before `dw.campaign`
    // can be asserted, in every valid play order — which is what a strict ancestor
    // is. `DW0525` reads both arrivals, and a wrong answer there runs both ways —
    // a door the party is FORCED to open reads shut again (refusing a rest point
    // behind it), and a `close-gate` the last objective fires goes missing
    // (admitting a rest point sealed in). The trigger steps the path performs
    // before them are added by the sweep below, like every later step's.
    //
    // **Forcedness is not re-decided here, and the set is every objective on the
    // path rather than the mandatory ones** (spec-0051). `collect_region_events`
    // owns that reading and applies it one layer earlier: a write that does not
    // FILL is dropped outright when its root is unforced, so an `open-gate`
    // (`RegionWrite::Unseal`) hanging off a quest nobody has to play is not in
    // `region_events` for any relation to credit, while an unforced FILL is kept —
    // a wall the party may find standing, which the proof must survive. Asking the
    // question a second time here would give a second authority, and for a fill it
    // would be the answer that ships.
    //
    // A **branch** path (`Plan::branch_gate_model`) is ordered by
    // `flow::dag_order` over what its world completes rather than by the finale's
    // closure, so it can carry a quest that world's player may skip. It is sound
    // for the same reason: the branch's own `collect_region_events` has already
    // dropped that quest's unseals. Its consumers — `nav::check_branch_path` and
    // `nav::branch_path_routes` — arrive only at objective steps, so these rows
    // answer nothing there either way.
    if let Some(&last) = obj_step.values().max() {
        let every: BTreeSet<usize> = obj_step.values().copied().collect();
        for s in last + 1..=steps {
            out.entry(s).or_insert_with(|| every.clone());
        }
    }

    // Every step after a trigger step has it as an ancestor.
    for (&s, anc) in out.iter_mut() {
        anc.extend(trigger_step.values().copied().filter(|&t| t < s));
    }
    out
}
