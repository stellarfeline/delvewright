//! Campaign validation: all six rule groups from spec-0001.
//!
//! [`validate_campaign`] uses the vendored v0 registries; the compiler injects
//! full registries via [`validate_campaign_with`].

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::{Campaign, SUPPORTED_DSL_VERSION, Stage};
use crate::registry::{AnchorRegistry, ItemRegistry, VendoredAnchorRegistry, VendoredItemRegistry};
use crate::stages::QuestEffect;

/// Validate a campaign against all spec-0001 rules using the vendored v0
/// registries (subset item registry + hello-world anchor metadata).
pub fn validate_campaign(c: &Campaign) -> Vec<Diagnostic> {
    let items = VendoredItemRegistry::v1_21_11();
    let anchors = VendoredAnchorRegistry::hello_world();
    validate_campaign_with(c, &items, &anchors)
}

/// Validate a campaign with caller-supplied registries.
pub fn validate_campaign_with(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
) -> Vec<Diagnostic> {
    let mut d: Vec<Diagnostic> = Vec::new();

    envelope(c, &mut d);
    syntax(c, &mut d);
    uniqueness(c, &mut d);
    references(c, &mut d);
    dialogue(c, &mut d);
    plan(c, &mut d);
    after_ordering(c, &mut d);
    reserved(c, &mut d);
    anchors_and_items(c, items, anchors, &mut d);
    cross_stage(c, &mut d);

    d
}

// ---------------------------------------------------------------------------
// Rule group 1 — envelope
// ---------------------------------------------------------------------------

fn envelope(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let stages = [
        (Stage::World, c.world.stage, c.world.dsl_version.as_str()),
        (Stage::Npcs, c.npcs.stage, c.npcs.dsl_version.as_str()),
        (
            Stage::Classes,
            c.classes.stage,
            c.classes.dsl_version.as_str(),
        ),
        (
            Stage::QuestPlan,
            c.quest_plan.stage,
            c.quest_plan.dsl_version.as_str(),
        ),
        (Stage::Quests, c.quests.stage, c.quests.dsl_version.as_str()),
    ];
    for (expected, actual, version) in stages {
        if actual != expected {
            d.push(Diagnostic::error(
                codes::STAGE_MISMATCH,
                expected.name(),
                "/stage",
                format!(
                    "stage is `{}` but this document is the `{}` stage",
                    actual.name(),
                    expected.name()
                ),
            ));
        }
        if version != SUPPORTED_DSL_VERSION {
            d.push(Diagnostic::error(
                codes::DSL_VERSION,
                expected.name(),
                "/dsl_version",
                format!("unsupported dsl_version `{version}`, expected `{SUPPORTED_DSL_VERSION}`"),
            ));
        }
    }

    let ids = [
        (Stage::World, &c.world.campaign_id),
        (Stage::Npcs, &c.npcs.campaign_id),
        (Stage::Classes, &c.classes.campaign_id),
        (Stage::QuestPlan, &c.quest_plan.campaign_id),
        (Stage::Quests, &c.quests.campaign_id),
    ];
    let canonical = c.world.campaign_id.as_str();
    for (stage, id) in ids {
        if !id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                stage.name(),
                "/campaign_id",
                format!("malformed campaign_id `{id}` (expected kebab-case)"),
            ));
        }
        if id.as_str() != canonical {
            d.push(Diagnostic::error(
                codes::CAMPAIGN_ID_MISMATCH,
                stage.name(),
                "/campaign_id",
                format!("campaign_id `{id}` differs from `{canonical}` in the world stage"),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 2 — id syntax
// ---------------------------------------------------------------------------

fn syntax(c: &Campaign, d: &mut Vec<Diagnostic>) {
    macro_rules! chk {
        ($id:expr, $stage:expr, $path:expr) => {
            if !$id.is_valid_syntax() {
                d.push(Diagnostic::error(
                    codes::ID_SYNTAX,
                    $stage,
                    $path,
                    format!("malformed id `{}`", $id),
                ));
            }
        };
    }

    for (i, a) in c.world.content.areas.iter().enumerate() {
        chk!(a.id, "world", format!("/content/areas/{i}/id"));
        chk!(a.prefab, "world", format!("/content/areas/{i}/prefab"));
    }
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        chk!(npc.id, "npcs", format!("/content/npcs/{i}/id"));
        for (j, node) in npc.dialogue.nodes.iter().enumerate() {
            chk!(
                node.id,
                "npcs",
                format!("/content/npcs/{i}/dialogue/nodes/{j}/id")
            );
        }
    }
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        chk!(cl.id, "classes", format!("/content/classes/{i}/id"));
    }
    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        chk!(q.id, "quest-plan", format!("/content/quests/{i}/id"));
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        chk!(q.id, "quests", format!("/content/quests/{i}/id"));
        for (j, obj) in q.objectives.iter().enumerate() {
            chk!(
                obj.id(),
                "quests",
                format!("/content/quests/{i}/objectives/{j}/id")
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 2 — id uniqueness
// ---------------------------------------------------------------------------

fn dup_check<'a>(
    ids: impl Iterator<Item = (&'a str, String)>,
    stage: &'static str,
    what: &str,
    d: &mut Vec<Diagnostic>,
) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (id, path) in ids {
        if !seen.insert(id) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                stage,
                path,
                format!("duplicate {what} id `{id}`"),
            ));
        }
    }
}

fn uniqueness(c: &Campaign, d: &mut Vec<Diagnostic>) {
    dup_check(
        c.world
            .content
            .areas
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.as_str(), format!("/content/areas/{i}/id"))),
        "world",
        "area",
        d,
    );
    dup_check(
        c.npcs
            .content
            .npcs
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), format!("/content/npcs/{i}/id"))),
        "npcs",
        "npc",
        d,
    );
    dup_check(
        c.classes
            .content
            .classes
            .iter()
            .enumerate()
            .map(|(i, cl)| (cl.id.as_str(), format!("/content/classes/{i}/id"))),
        "classes",
        "class",
        d,
    );
    dup_check(
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
    dup_check(
        c.quests
            .content
            .quests
            .iter()
            .enumerate()
            .map(|(i, q)| (q.id.as_str(), format!("/content/quests/{i}/id"))),
        "quests",
        "quest",
        d,
    );
    // Objective ids: unique across all of stage 5 (so cross-stage dialogue refs
    // resolve unambiguously).
    dup_check(
        c.quests
            .content
            .quests
            .iter()
            .enumerate()
            .flat_map(|(i, q)| {
                q.objectives.iter().enumerate().map(move |(j, o)| {
                    (
                        o.id().as_str(),
                        format!("/content/quests/{i}/objectives/{j}/id"),
                    )
                })
            }),
        "quests",
        "objective",
        d,
    );
    // Dialogue node ids: unique within each NPC's graph.
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        dup_check(
            npc.dialogue.nodes.iter().enumerate().map(|(j, node)| {
                (
                    node.id.as_str(),
                    format!("/content/npcs/{i}/dialogue/nodes/{j}/id"),
                )
            }),
            "npcs",
            "dialogue node",
            d,
        );
    }
}

// ---------------------------------------------------------------------------
// Rule group 2 — dangling references (non-dialogue, non-finale)
// ---------------------------------------------------------------------------

fn references(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let area_ids: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let planned_ids: BTreeSet<&str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    let expanded_ids: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();

    let dangling = |d: &mut Vec<Diagnostic>, ok: bool, stage, path: String, msg: String| {
        if !ok {
            d.push(Diagnostic::error(codes::DANGLING_REF, stage, path, msg));
        }
    };

    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(npc.area.as_str()),
            "npcs",
            format!("/content/npcs/{i}/area"),
            format!("npc references unknown area `{}`", npc.area),
        );
    }

    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(q.area.as_str()),
            "quest-plan",
            format!("/content/quests/{i}/area"),
            format!("quest references unknown area `{}`", q.area),
        );
        for (k, npc) in q.npcs.iter().enumerate() {
            dangling(
                d,
                npc_ids.contains(npc.as_str()),
                "quest-plan",
                format!("/content/quests/{i}/npcs/{k}"),
                format!("quest references unknown npc `{npc}`"),
            );
        }
        for (k, dep) in q.depends_on.iter().enumerate() {
            dangling(
                d,
                planned_ids.contains(dep.as_str()),
                "quest-plan",
                format!("/content/quests/{i}/depends_on/{k}"),
                format!("quest depends on unknown quest `{dep}`"),
            );
        }
    }

    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if let crate::stages::Trigger::QuestComplete { quest } = &q.trigger {
            dangling(
                d,
                expanded_ids.contains(quest.as_str()),
                "quests",
                format!("/content/quests/{i}/trigger/quest"),
                format!("trigger references unknown quest `{quest}`"),
            );
        }
        let local_objs: BTreeSet<&str> = q.objectives.iter().map(|o| o.id().as_str()).collect();
        for (j, obj) in q.objectives.iter().enumerate() {
            if let crate::stages::Objective::TalkTo { npc, .. } = obj {
                dangling(
                    d,
                    npc_ids.contains(npc.as_str()),
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/npc"),
                    format!("objective references unknown npc `{npc}`"),
                );
            }
            for (m, aft) in obj.after().iter().enumerate() {
                dangling(
                    d,
                    local_objs.contains(aft.as_str()),
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/after/{m}"),
                    format!("objective `after` references unknown objective `{aft}`"),
                );
            }
        }
        for key in q.on_objective_complete.keys() {
            dangling(
                d,
                local_objs.contains(key.as_str()),
                "quests",
                format!("/content/quests/{i}/on_objective_complete/{key}"),
                format!("on_objective_complete references unknown objective `{key}`"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 3 — dialogue graph
// ---------------------------------------------------------------------------

fn dialogue(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let all_objectives: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .map(|o| o.id().as_str())
        .collect();

    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        let dlg = &npc.dialogue;
        let node_ids: BTreeSet<&str> = dlg.nodes.iter().map(|n| n.id.as_str()).collect();

        // `next` / effect references.
        for (j, node) in dlg.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                if let Some(next) = &opt.next
                    && !node_ids.contains(next.as_str())
                {
                    d.push(Diagnostic::error(
                        codes::DIALOGUE_BAD_REF,
                        "npcs",
                        format!("/content/npcs/{i}/dialogue/nodes/{j}/options/{k}/next"),
                        format!("option `next` references unknown node `{next}`"),
                    ));
                }
                for (m, eff) in opt.effects.iter().enumerate() {
                    let crate::stages::DialogueEffect::CompleteObjective { objective } = eff;
                    if !all_objectives.contains(objective.as_str()) {
                        d.push(Diagnostic::error(
                            codes::DIALOGUE_BAD_OBJECTIVE,
                            "npcs",
                            format!(
                                "/content/npcs/{i}/dialogue/nodes/{j}/options/{k}/effects/{m}/objective"
                            ),
                            format!("dialogue effect references unknown objective `{objective}`"),
                        ));
                    }
                }
            }
        }

        // Root existence.
        if !node_ids.contains(dlg.root.as_str()) {
            d.push(Diagnostic::error(
                codes::DIALOGUE_BAD_REF,
                "npcs",
                format!("/content/npcs/{i}/dialogue/root"),
                format!("dialogue root references unknown node `{}`", dlg.root),
            ));
            continue; // reachability is undefined without a root
        }

        // Reachability from root.
        let adj: BTreeMap<&str, Vec<&str>> = dlg
            .nodes
            .iter()
            .map(|n| {
                let outs = n
                    .options
                    .iter()
                    .filter_map(|o| o.next.as_ref().map(|x| x.as_str()))
                    .collect();
                (n.id.as_str(), outs)
            })
            .collect();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut stack = vec![dlg.root.as_str()];
        while let Some(cur) = stack.pop() {
            if seen.insert(cur)
                && let Some(neis) = adj.get(cur)
            {
                stack.extend(neis.iter().copied());
            }
        }
        for (j, node) in dlg.nodes.iter().enumerate() {
            if !seen.contains(node.id.as_str()) {
                d.push(Diagnostic::error(
                    codes::DIALOGUE_UNREACHABLE,
                    "npcs",
                    format!("/content/npcs/{i}/dialogue/nodes/{j}"),
                    format!("dialogue node `{}` is unreachable from root", node.id),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 4 — quest plan
// ---------------------------------------------------------------------------

fn plan(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let plan = &c.quest_plan.content;
    let planned_ids: BTreeSet<&str> = plan.quests.iter().map(|q| q.id.as_str()).collect();

    // Non-mandatory (reserved in v0).
    for (i, q) in plan.quests.iter().enumerate() {
        if !q.mandatory {
            d.push(Diagnostic::error(
                codes::NON_MANDATORY,
                "quest-plan",
                format!("/content/quests/{i}/mandatory"),
                format!("quest `{}` is non-mandatory (reserved until M3)", q.id),
            ));
        }
    }

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
            codes::PLAN_CYCLE,
            "quest-plan",
            "/content/quests",
            "quest dependency graph contains a cycle",
        ));
        return; // reachability is meaningless with a cycle
    }

    // Finale must be declared.
    if !planned_ids.contains(plan.finale.as_str()) {
        d.push(Diagnostic::error(
            codes::FINALE_UNKNOWN,
            "quest-plan",
            "/content/finale",
            format!("finale `{}` is not a declared quest", plan.finale),
        ));
        return;
    }

    // Finale convergence: every quest must be a transitive dependency of the
    // finale (the plan converges on the finale). See README (spec ambiguity).
    let mut reach: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![plan.finale.as_str()];
    while let Some(cur) = stack.pop() {
        if reach.insert(cur)
            && let Some(deps) = edges.get(cur)
        {
            stack.extend(deps.iter().copied());
        }
    }
    for (i, q) in plan.quests.iter().enumerate() {
        if !reach.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                codes::FINALE_UNREACHABLE,
                "quest-plan",
                format!("/content/quests/{i}"),
                format!(
                    "quest `{}` is not a (transitive) dependency of finale `{}`; \
                     the plan does not converge on the finale",
                    q.id, plan.finale
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 5 — intra-quest objective ordering (`after` DAG)
// ---------------------------------------------------------------------------

fn after_ordering(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        let local: BTreeSet<&str> = q.objectives.iter().map(|o| o.id().as_str()).collect();
        let edges: BTreeMap<&str, Vec<&str>> = q
            .objectives
            .iter()
            .map(|o| {
                let deps = o
                    .after()
                    .iter()
                    .map(|x| x.as_str())
                    .filter(|x| local.contains(x))
                    .collect();
                (o.id().as_str(), deps)
            })
            .collect();
        let nodes: Vec<&str> = q.objectives.iter().map(|o| o.id().as_str()).collect();
        if graph_has_cycle(&nodes, &edges) {
            d.push(Diagnostic::error(
                codes::AFTER_CYCLE,
                "quests",
                format!("/content/quests/{i}/objectives"),
                format!(
                    "objective `after` ordering in quest `{}` contains a cycle",
                    q.id
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 5 — reserved values / fields
// ---------------------------------------------------------------------------

fn reserved(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, a) in c.world.content.areas.iter().enumerate() {
        if a.prefab_pool.is_some() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "world",
                format!("/content/areas/{i}/prefab_pool"),
                "prefab_pool is reserved (jigsaw pools land in M2)",
            ));
        }
    }
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(name) = npc.role.reserved() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "npcs",
                format!("/content/npcs/{i}/role"),
                format!("role `{name}` is reserved (not implemented in v0)"),
            ));
        }
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, obj) in q.objectives.iter().enumerate() {
            if let Some(name) = obj.reserved() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/type"),
                    format!("objective type `{name}` is reserved (not implemented in v0)"),
                ));
            }
        }
        for_each_effect(q, |path, eff| {
            if let Some(name) = eff.reserved() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/type"),
                    format!("effect `{name}` is reserved (not implemented in v0)"),
                ));
            }
        });
    }
}

/// Visit every quest effect with a relative path fragment.
fn for_each_effect(q: &crate::stages::Quest, mut f: impl FnMut(String, &QuestEffect)) {
    for (key, effs) in &q.on_objective_complete {
        for (m, eff) in effs.iter().enumerate() {
            f(format!("on_objective_complete/{key}/{m}"), eff);
        }
    }
    for (m, eff) in q.on_complete.iter().enumerate() {
        f(format!("on_complete/{m}"), eff);
    }
}

// ---------------------------------------------------------------------------
// Rule group 5 — anchors and items
// ---------------------------------------------------------------------------

fn anchors_and_items(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    // area id -> anchor set of its bound prefab (only if the prefab is known).
    let mut area_anchors: BTreeMap<&str, &BTreeSet<String>> = BTreeMap::new();
    for a in &c.world.content.areas {
        if let Some(set) = anchors.anchors_for(&a.prefab) {
            area_anchors.insert(a.id.as_str(), set);
        }
    }
    // planned quest id -> its area.
    let quest_area: BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();

    // NPC anchors.
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(set) = area_anchors.get(npc.area.as_str())
            && !set.contains(npc.anchor.as_str())
        {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "npcs",
                format!("/content/npcs/{i}/anchor"),
                format!(
                    "anchor `{}` is not provided by the prefab bound to area `{}`",
                    npc.anchor, npc.area
                ),
            ));
        }
    }

    // Objective / effect anchors, resolved against the quest's planned area.
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        let set = quest_area
            .get(q.id.as_str())
            .and_then(|area| area_anchors.get(*area).copied());
        let Some(set) = set else { continue };

        for (j, obj) in q.objectives.iter().enumerate() {
            if let crate::stages::Objective::ReachAnchor { anchor, .. } = obj
                && !set.contains(anchor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/anchor"),
                    format!("anchor `{anchor}` is not provided by the quest's prefab"),
                ));
            }
        }
        for_each_effect(q, |path, eff| {
            if let Some(anchor) = eff.open_gate_anchor()
                && !set.contains(anchor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/anchor"),
                    format!("gate anchor `{anchor}` is not provided by the quest's prefab"),
                ));
            }
        });
    }

    // Kit items.
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        for (j, it) in cl.kit.iter().enumerate() {
            if !items.contains(&it.item) {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "classes",
                    format!("/content/classes/{i}/kit/{j}/item"),
                    format!("item `{}` is not in the 1.21.11 registry", it.item),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 6 — cross-stage 1:1 expansion
// ---------------------------------------------------------------------------

fn cross_stage(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let planned_ids: BTreeSet<&str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    let expanded_ids: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();

    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        if !expanded_ids.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                codes::QUEST_NOT_EXPANDED,
                "quest-plan",
                format!("/content/quests/{i}"),
                format!("planned quest `{}` has no stage-5 expansion", q.id),
            ));
        }
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if !planned_ids.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                codes::QUEST_NOT_PLANNED,
                "quests",
                format!("/content/quests/{i}"),
                format!("quest `{}` is expanded but not planned in stage 4", q.id),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Directed-graph cycle detection (three-color DFS).
fn graph_has_cycle<'a>(nodes: &[&'a str], edges: &BTreeMap<&'a str, Vec<&'a str>>) -> bool {
    let mut color: BTreeMap<&'a str, u8> = BTreeMap::new();
    for &n in nodes {
        if color.get(n).copied().unwrap_or(0) == 0 && dfs_cycle(n, edges, &mut color) {
            return true;
        }
    }
    false
}

fn dfs_cycle<'a>(
    node: &'a str,
    edges: &BTreeMap<&'a str, Vec<&'a str>>,
    color: &mut BTreeMap<&'a str, u8>,
) -> bool {
    color.insert(node, 1);
    if let Some(neis) = edges.get(node) {
        for &n in neis {
            match color.get(n).copied().unwrap_or(0) {
                0 => {
                    if dfs_cycle(n, edges, color) {
                        return true;
                    }
                }
                1 => return true,
                _ => {}
            }
        }
    }
    color.insert(node, 2);
    false
}
