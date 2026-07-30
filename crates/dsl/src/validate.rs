//! Campaign validation: all six rule groups from spec-0001.
//!
//! [`validate_campaign`] uses the vendored v0 registries; the compiler injects
//! full registries via [`validate_campaign_with`].

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::{Campaign, SUPPORTED_DSL_VERSIONS, Stage, is_supported_version, is_v03};
use crate::registry::{
    AnchorRegistry, EntityRegistry, ItemRegistry, VendoredAnchorRegistry, VendoredEntityRegistry,
    VendoredItemRegistry,
};
use crate::stages::{Objective, QuestEffect};

/// Validate a campaign against all spec-0001 rules using the vendored v0
/// registries (subset item + entity registries + hello-world anchor metadata).
pub fn validate_campaign(c: &Campaign) -> Vec<Diagnostic> {
    let items = VendoredItemRegistry::v1_21_11();
    let entities = VendoredEntityRegistry::v1_21_11();
    let anchors = VendoredAnchorRegistry::hello_world();
    validate_campaign_with(c, &items, &anchors, &entities)
}

/// Validate a campaign with caller-supplied registries. The `entities` registry
/// (DSL v0.3) validates stage-5 wave mobs; the compiler injects the full
/// 1.21.11 item/entity registries and real prefab metadata.
pub fn validate_campaign_with(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    entities: &dyn EntityRegistry,
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
    prefab_binding(c, anchors, &mut d);
    anchors_and_items(c, items, anchors, &mut d);
    cross_stage(c, &mut d);
    // DSL v0.3: the new stage-5 verbs, waves and flags. Gated on the quests
    // stage's version so a v0.2 campaign is unaffected (its uses of these verbs
    // are still rejected as reserved by `reserved`, above).
    if is_v03(c.quests.dsl_version.as_str()) {
        v03_checks(c, items, anchors, entities, &mut d);
    }

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
        (
            Stage::Dialogue,
            c.dialogue.stage,
            c.dialogue.dsl_version.as_str(),
        ),
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
        if !is_supported_version(version) {
            d.push(Diagnostic::error(
                codes::DSL_VERSION,
                expected.name(),
                "/dsl_version",
                format!(
                    "unsupported dsl_version `{version}`, expected one of {SUPPORTED_DSL_VERSIONS:?}"
                ),
            ));
        }
    }

    let ids = [
        (Stage::World, &c.world.campaign_id),
        (Stage::Npcs, &c.npcs.campaign_id),
        (Stage::Classes, &c.classes.campaign_id),
        (Stage::QuestPlan, &c.quest_plan.campaign_id),
        (Stage::Quests, &c.quests.campaign_id),
        (Stage::Dialogue, &c.dialogue.campaign_id),
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
        if let Some(prefab) = &a.prefab {
            chk!(prefab, "world", format!("/content/areas/{i}/prefab"));
        }
        if let Some(pool) = &a.prefab_pool {
            chk!(pool, "world", format!("/content/areas/{i}/prefab_pool"));
        }
    }
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        chk!(npc.id, "npcs", format!("/content/npcs/{i}/id"));
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
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            chk!(
                node.id,
                "dialogue",
                format!("/content/dialogues/{i}/nodes/{j}/id")
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
    // Dialogue trees: at most one per NPC (a duplicate tree is a duplicate npc
    // binding within the stage-6 dialogue namespace).
    dup_check(
        c.dialogue
            .content
            .dialogues
            .iter()
            .enumerate()
            .map(|(i, t)| (t.npc.as_str(), format!("/content/dialogues/{i}/npc"))),
        "dialogue",
        "dialogue tree for npc",
        d,
    );
    // Dialogue node ids: unique within each tree.
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        dup_check(
            tree.nodes.iter().enumerate().map(|(j, node)| {
                (
                    node.id.as_str(),
                    format!("/content/dialogues/{i}/nodes/{j}/id"),
                )
            }),
            "dialogue",
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
        // Persona relationships are same-stage NPC refs (validated within stage 2).
        for (k, rel) in npc.persona.relationships.iter().enumerate() {
            dangling(
                d,
                npc_ids.contains(rel.npc.as_str()),
                "npcs",
                format!("/content/npcs/{i}/persona/relationships/{k}/npc"),
                format!("persona relationship references unknown npc `{}`", rel.npc),
            );
        }
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
// Rule group 3 — stage-6 dialogue graph
// ---------------------------------------------------------------------------

fn dialogue(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::stages::{DialogueEffect, Objective};

    // Stage-5 objective facts: which are `talk-to`, and (for those) their npc.
    let mut all_objectives: BTreeSet<&str> = BTreeSet::new();
    let mut talk_npc: BTreeMap<&str, &str> = BTreeMap::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            all_objectives.insert(o.id().as_str());
            if let Objective::TalkTo { id, npc, .. } = o {
                talk_npc.insert(id.as_str(), npc.as_str());
            }
        }
    }

    // npc id -> objective ids completed by an option reachable from that tree's
    // root (feeds the DW0123 coverage check).
    let mut reachable_completes: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();

    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        let node_ids: BTreeSet<&str> = tree.nodes.iter().map(|n| n.id.as_str()).collect();

        // `next` / effect references.
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                if let Some(next) = &opt.next
                    && !node_ids.contains(next.as_str())
                {
                    d.push(Diagnostic::error(
                        codes::DIALOGUE_BAD_REF,
                        "dialogue",
                        format!("/content/dialogues/{i}/nodes/{j}/options/{k}/next"),
                        format!("option `next` references unknown node `{next}`"),
                    ));
                }
                for (m, eff) in opt.effects.iter().enumerate() {
                    let DialogueEffect::CompleteObjective { objective } = eff;
                    let oid = objective.as_str();
                    let path = format!(
                        "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/objective"
                    );
                    let msg = if !all_objectives.contains(oid) {
                        Some(format!(
                            "dialogue effect references unknown objective `{objective}`"
                        ))
                    } else if let Some(owner) = talk_npc.get(oid) {
                        if *owner == tree.npc.as_str() {
                            None
                        } else {
                            Some(format!(
                                "dialogue effect completes `talk-to` objective `{objective}`, \
                                 which belongs to npc `{owner}`, not this tree's npc `{}`",
                                tree.npc
                            ))
                        }
                    } else {
                        Some(format!(
                            "dialogue effect targets objective `{objective}`, which is not a \
                             `talk-to` objective"
                        ))
                    };
                    if let Some(msg) = msg {
                        d.push(Diagnostic::error(
                            codes::DIALOGUE_BAD_OBJECTIVE,
                            "dialogue",
                            path,
                            msg,
                        ));
                    }
                }
            }
        }

        // Root existence.
        if !node_ids.contains(tree.root.as_str()) {
            d.push(Diagnostic::error(
                codes::DIALOGUE_BAD_REF,
                "dialogue",
                format!("/content/dialogues/{i}/root"),
                format!("dialogue root references unknown node `{}`", tree.root),
            ));
            continue; // reachability is undefined without a root
        }

        // Reachability from root.
        let adj: BTreeMap<&str, Vec<&str>> = tree
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
        let mut stack = vec![tree.root.as_str()];
        while let Some(cur) = stack.pop() {
            if seen.insert(cur)
                && let Some(neis) = adj.get(cur)
            {
                stack.extend(neis.iter().copied());
            }
        }
        for (j, node) in tree.nodes.iter().enumerate() {
            if !seen.contains(node.id.as_str()) {
                d.push(Diagnostic::error(
                    codes::DIALOGUE_UNREACHABLE,
                    "dialogue",
                    format!("/content/dialogues/{i}/nodes/{j}"),
                    format!("dialogue node `{}` is unreachable from root", node.id),
                ));
            }
        }

        // Objectives completed by reachable options (for the coverage check).
        let completes = reachable_completes.entry(tree.npc.as_str()).or_default();
        for node in &tree.nodes {
            if !seen.contains(node.id.as_str()) {
                continue;
            }
            for opt in &node.options {
                for eff in &opt.effects {
                    let DialogueEffect::CompleteObjective { objective } = eff;
                    completes.insert(objective.as_str());
                }
            }
        }
    }

    // Every `talk-to` objective must have ≥ 1 reachable completing option in its
    // own npc's tree (the static half of the compiler's DW0203 guarantee).
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Objective::TalkTo { id, npc, .. } = o {
                let covered = reachable_completes
                    .get(npc.as_str())
                    .is_some_and(|s| s.contains(id.as_str()));
                if !covered {
                    d.push(Diagnostic::error(
                        codes::DIALOGUE_UNCOVERED,
                        "dialogue",
                        format!("/content/quests/{qi}/objectives/{oi}"),
                        format!(
                            "`talk-to` objective `{id}` has no reachable dialogue option in npc \
                             `{npc}`'s tree that completes it"
                        ),
                    ));
                }
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
    // The v0.3 verbs (`kill`/`collect`/`interact`, `give-item`/`set-flag`/
    // `spawn-wave`) are implemented under dsl_version 0.3.0 and reserved under
    // 0.2.0. NPC roles `vendor`/`boss` remain reserved in both.
    let v03 = is_v03(c.quests.dsl_version.as_str());

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
            if let Some(name) = obj.v03_verb()
                && !v03
            {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/type"),
                    format!("objective type `{name}` is reserved (requires dsl_version 0.3.0)"),
                ));
            }
        }
        for_each_effect(q, |path, eff| {
            if let Some(name) = eff.v03_effect()
                && !v03
            {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/type"),
                    format!("effect `{name}` is reserved (requires dsl_version 0.3.0)"),
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
    // area id -> anchor set of its bound prefab (only for single-prefab areas
    // whose prefab is known; pool areas resolve anchors in M2 task #9).
    let mut area_anchors: BTreeMap<&str, &BTreeSet<String>> = BTreeMap::new();
    for a in &c.world.content.areas {
        if let Some(prefab) = &a.prefab
            && let Some(set) = anchors.anchors_for(prefab)
        {
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
// Rule group 6 — prefab / prefab_pool binding (stage 1)
// ---------------------------------------------------------------------------

fn prefab_binding(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    for (i, a) in c.world.content.areas.iter().enumerate() {
        // Exactly one of `prefab` / `prefab_pool`.
        match (&a.prefab, &a.prefab_pool) {
            (Some(_), Some(_)) => d.push(Diagnostic::error(
                codes::PREFAB_BINDING,
                "world",
                format!("/content/areas/{i}"),
                format!(
                    "area `{}` binds both `prefab` and `prefab_pool`; bind exactly one",
                    a.id
                ),
            )),
            (None, None) => d.push(Diagnostic::error(
                codes::PREFAB_BINDING,
                "world",
                format!("/content/areas/{i}"),
                format!(
                    "area `{}` binds neither `prefab` nor `prefab_pool`; bind exactly one",
                    a.id
                ),
            )),
            _ => {}
        }
        // A bound pool must resolve against the prefab-metadata surface.
        if let Some(pool) = &a.prefab_pool
            && pool.is_valid_syntax()
            && !anchors.has_pool(pool)
        {
            d.push(Diagnostic::error(
                codes::POOL_UNKNOWN,
                "world",
                format!("/content/areas/{i}/prefab_pool"),
                format!("prefab pool `{pool}` is not declared in the prefab metadata"),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 6 — cross-stage 1:1 (quest plan↔expansion, npc↔dialogue tree)
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

    // Stage-2 NPC ↔ stage-6 dialogue tree, 1:1 both directions.
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let tree_npcs: BTreeSet<&str> = c
        .dialogue
        .content
        .dialogues
        .iter()
        .map(|t| t.npc.as_str())
        .collect();
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if !tree_npcs.contains(npc.id.as_str()) {
            d.push(Diagnostic::error(
                codes::NPC_WITHOUT_TREE,
                "dialogue",
                format!("/content/npcs/{i}"),
                format!("npc `{}` has no stage-6 dialogue tree", npc.id),
            ));
        }
    }
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        if !npc_ids.contains(tree.npc.as_str()) {
            d.push(Diagnostic::error(
                codes::TREE_WITHOUT_NPC,
                "dialogue",
                format!("/content/dialogues/{i}/npc"),
                format!(
                    "dialogue tree references npc `{}`, which is not declared in stage 2",
                    tree.npc
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// DSL v0.3 — waves, flags, and the new stage-5 verbs
// ---------------------------------------------------------------------------

/// The v0.3 semantic checks (run only for `dsl_version` 0.3.0):
///
/// - wave declarations: id syntax (`DW0110`), uniqueness (`DW0111`), mob entity
///   ids (`DW0173`);
/// - `kill.wave` / `spawn-wave.wave` reference a declared wave (`DW0170`);
/// - a killed wave is spawned by some `spawn-wave` effect (`DW0171`);
/// - `requires_flags` reference a flag some `set-flag` effect produces
///   (`DW0172`);
/// - item ids on `collect` / `interact.requires_item` / `give-item` are in the
///   registry (`DW0143`, reused);
/// - `collect` / `interact` anchors resolve against the quest's single-prefab
///   area (`DW0142`, reused; pool-area anchors are resolved by the compiler,
///   as for `reach-anchor`).
fn v03_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;

    // Wave declarations.
    let mut declared_waves: BTreeSet<&str> = BTreeSet::new();
    let mut seen_waves: BTreeSet<&str> = BTreeSet::new();
    for (i, w) in quests.waves.iter().enumerate() {
        if !w.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/waves/{i}/id"),
                format!("malformed wave id `{}`", w.id),
            ));
        }
        if !seen_waves.insert(w.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/waves/{i}/id"),
                format!("duplicate wave id `{}`", w.id),
            ));
        }
        declared_waves.insert(w.id.as_str());
        for (k, m) in w.mobs.iter().enumerate() {
            if !entities.contains(&m.entity) {
                d.push(Diagnostic::error(
                    codes::ENTITY_UNKNOWN,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/entity"),
                    format!("`{}` is not a known 1.21.11 entity id", m.entity),
                ));
            }
        }
    }

    // Flags declared by `set-flag`; waves spawned by `spawn-wave` (first pass —
    // needed before the reference checks below).
    let mut declared_flags: BTreeSet<&str> = BTreeSet::new();
    let mut spawned_waves: BTreeSet<&str> = BTreeSet::new();
    for q in &quests.quests {
        let effs = q
            .on_objective_complete
            .values()
            .flatten()
            .chain(q.on_complete.iter());
        for eff in effs {
            if let Some(f) = eff.set_flag() {
                declared_flags.insert(f.as_str());
            }
            if let Some(w) = eff.spawn_wave() {
                spawned_waves.insert(w.as_str());
            }
        }
    }

    // area id -> its single-prefab anchor set (pool areas deferred to compiler).
    let mut area_anchors: BTreeMap<&str, &BTreeSet<String>> = BTreeMap::new();
    for a in &c.world.content.areas {
        if let Some(prefab) = &a.prefab
            && let Some(set) = anchors.anchors_for(prefab)
        {
            area_anchors.insert(a.id.as_str(), set);
        }
    }
    let quest_area: BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();

    // Reference checks.
    for (i, q) in quests.quests.iter().enumerate() {
        let set = quest_area
            .get(q.id.as_str())
            .and_then(|area| area_anchors.get(*area).copied());

        for (j, obj) in q.objectives.iter().enumerate() {
            match obj {
                Objective::Kill { wave, .. } => {
                    if !declared_waves.contains(wave.as_str()) {
                        d.push(Diagnostic::error(
                            codes::WAVE_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/wave"),
                            format!("kill objective references unknown wave `{wave}`"),
                        ));
                    } else if !spawned_waves.contains(wave.as_str()) {
                        d.push(Diagnostic::error(
                            codes::WAVE_NEVER_SPAWNED,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/wave"),
                            format!(
                                "wave `{wave}` is killed but never spawned by any `spawn-wave` \
                                 effect; a wave must be spawned before its kill objective is \
                                 reachable"
                            ),
                        ));
                    }
                }
                Objective::Collect { item, anchor, .. } => {
                    if !items.contains(item) {
                        d.push(Diagnostic::error(
                            codes::ITEM_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/item"),
                            format!("item `{item}` is not in the 1.21.11 registry"),
                        ));
                    }
                    anchor_resolves(set, anchor, i, j, "anchor", d);
                }
                Objective::Interact {
                    anchor,
                    requires_item,
                    ..
                } => {
                    if let Some(it) = requires_item
                        && !items.contains(it)
                    {
                        d.push(Diagnostic::error(
                            codes::ITEM_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/requires_item"),
                            format!("requires_item `{it}` is not in the 1.21.11 registry"),
                        ));
                    }
                    anchor_resolves(set, anchor, i, j, "anchor", d);
                }
                Objective::TalkTo { .. } | Objective::ReachAnchor { .. } => {}
            }

            for (m, f) in obj.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/requires_flags/{m}"),
                        format!(
                            "requires_flags references flag `{f}`, which no `set-flag` effect \
                             ever produces"
                        ),
                    ));
                }
            }
        }

        for_each_effect(q, |path, eff| {
            if let Some(w) = eff.spawn_wave()
                && !declared_waves.contains(w.as_str())
            {
                d.push(Diagnostic::error(
                    codes::WAVE_UNKNOWN,
                    "quests",
                    format!("/content/quests/{i}/{path}/wave"),
                    format!("spawn-wave effect references unknown wave `{w}`"),
                ));
            }
            if let Some(it) = eff.give_item()
                && !items.contains(it)
            {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "quests",
                    format!("/content/quests/{i}/{path}/item"),
                    format!("give-item effect item `{it}` is not in the 1.21.11 registry"),
                ));
            }
        });
    }
}

/// Push `DW0142` if `anchor` is not provided by the quest's (known single-prefab)
/// area. `None` set = pool area or unknown prefab → deferred to the compiler.
fn anchor_resolves(
    set: Option<&BTreeSet<String>>,
    anchor: &crate::ids::AnchorId,
    qi: usize,
    oi: usize,
    field: &str,
    d: &mut Vec<Diagnostic>,
) {
    if let Some(set) = set
        && !set.contains(anchor.as_str())
    {
        d.push(Diagnostic::error(
            codes::ANCHOR_UNRESOLVED,
            "quests",
            format!("/content/quests/{qi}/objectives/{oi}/{field}"),
            format!("anchor `{anchor}` is not provided by the quest's prefab"),
        ));
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
