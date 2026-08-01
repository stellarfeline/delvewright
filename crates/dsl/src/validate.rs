//! Campaign validation: all six rule groups from spec-0001.
//!
//! [`validate_campaign`] uses the vendored v0 registries; the compiler injects
//! full registries via [`validate_campaign_with`].

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::{
    Campaign, SUPPORTED_DSL_VERSIONS, Stage, is_supported_version, is_v03, is_v04, is_v05, is_v06,
};
use crate::ids::is_kebab;
use crate::registry::{
    AnchorRegistry, BlockRegistry, EffectRegistry, EntityRegistry, ItemBackedBlockRegistry,
    ItemRegistry, VendoredAnchorRegistry, VendoredEffectRegistry, VendoredEntityRegistry,
    VendoredItemRegistry,
};
use crate::stages::{Objective, QuestEffect, TriggerOn};

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
    // DSL v0.4: props/set-block/narrate/triggers/skins/lifecycle/cutscene. The
    // status-effect registry is a fixed vanilla list; the block registry derives
    // from the item registry (see [`ItemBackedBlockRegistry`]), so no new
    // caller-supplied registry is needed. Gated on the quests stage version.
    if is_v04(c.quests.dsl_version.as_str()) {
        let blocks = ItemBackedBlockRegistry::new(items);
        let effects_reg = VendoredEffectRegistry::v1_21_11();
        v04_checks(c, anchors, &blocks, &effects_reg, &mut d);
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
                    "`stage` is `{}` but this is the `{}` stage document — set `stage` to `{}` (or \
                     move this content into the `{}` document it belongs to)",
                    actual.name(),
                    expected.name(),
                    expected.name(),
                    actual.name(),
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
                format!(
                    "campaign_id `{id}` differs from `{canonical}` (the world stage's id) — set \
                     every stage's `campaign_id` to `{canonical}` so all six documents name one \
                     campaign"
                ),
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
                    format!(
                        "malformed id `{}` — ids must be lowercase kebab-case with their type \
                         prefix (e.g. `area/keep`, `npc/keeper`, `quest/find-key`)",
                        $id
                    ),
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
                format!("duplicate {what} id `{id}` — rename one so every {what} id is unique"),
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
            format!(
                "npc references unknown area `{}` — declare it in stage-1 `world.areas` or \
                 correct the reference",
                npc.area
            ),
        );
        // Persona relationships are same-stage NPC refs (validated within stage 2).
        for (k, rel) in npc.persona.relationships.iter().enumerate() {
            dangling(
                d,
                npc_ids.contains(rel.npc.as_str()),
                "npcs",
                format!("/content/npcs/{i}/persona/relationships/{k}/npc"),
                format!(
                    "persona relationship references unknown npc `{}` — declare that npc in \
                     stage 2 or correct the reference",
                    rel.npc
                ),
            );
        }
    }

    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(q.area.as_str()),
            "quest-plan",
            format!("/content/quests/{i}/area"),
            format!(
                "quest references unknown area `{}` — declare it in stage-1 `world.areas` or \
                 correct the reference",
                q.area
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

    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if let crate::stages::Trigger::QuestComplete { quest } = &q.trigger {
            dangling(
                d,
                expanded_ids.contains(quest.as_str()),
                "quests",
                format!("/content/quests/{i}/trigger/quest"),
                format!(
                    "quest trigger `quest-complete` references unknown quest `{quest}` — declare \
                     that quest in stage 5 or correct the reference"
                ),
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
                    format!(
                        "`talk-to` objective references unknown npc `{npc}` — declare it in \
                         stage 2 or correct the reference"
                    ),
                );
            }
            for (m, aft) in obj.after().iter().enumerate() {
                dangling(
                    d,
                    local_objs.contains(aft.as_str()),
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/after/{m}"),
                    format!(
                        "objective `after` references unknown objective `{aft}` — `after` may \
                         only name another objective in the same quest; declare it or correct \
                         the reference"
                    ),
                );
            }
        }
        for key in q.on_objective_complete.keys() {
            dangling(
                d,
                local_objs.contains(key.as_str()),
                "quests",
                format!("/content/quests/{i}/on_objective_complete/{key}"),
                format!(
                    "`on_objective_complete` is keyed by unknown objective `{key}` — the key must \
                     name an objective declared in this quest; declare it or correct the key"
                ),
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
                        format!(
                            "dialogue option `next` references unknown node `{next}` — add a node \
                             with that id to this tree or correct the reference"
                        ),
                    ));
                }
                for (m, eff) in opt.effects.iter().enumerate() {
                    let DialogueEffect::CompleteObjective { objective } = eff else {
                        continue;
                    };
                    let oid = objective.as_str();
                    let path = format!(
                        "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/objective"
                    );
                    let msg = if !all_objectives.contains(oid) {
                        Some(format!(
                            "dialogue `complete-objective` effect references unknown objective \
                             `{objective}` — it must name a `talk-to` objective on this tree's \
                             npc; declare it or correct the reference"
                        ))
                    } else if let Some(owner) = talk_npc.get(oid) {
                        if *owner == tree.npc.as_str() {
                            None
                        } else {
                            Some(format!(
                                "dialogue effect completes `talk-to` objective `{objective}`, \
                                 which belongs to npc `{owner}`, not this tree's npc `{}` — a tree \
                                 may only complete its own npc's objectives; move the effect into \
                                 `{owner}`'s tree",
                                tree.npc
                            ))
                        }
                    } else {
                        Some(format!(
                            "dialogue `complete-objective` effect targets objective `{objective}`, \
                             which is not a `talk-to` objective — only `talk-to` objectives are \
                             completed through dialogue; retarget it or change the objective's type"
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
                format!(
                    "dialogue tree `root` references unknown node `{}` — add a node with that id \
                     or point `root` at an existing node",
                    tree.root
                ),
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
                    format!(
                        "dialogue node `{}` is unreachable from `root` — add an option whose \
                         `next` leads here from a reachable node, or remove this node",
                        node.id
                    ),
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
                    if let DialogueEffect::CompleteObjective { objective } = eff {
                        completes.insert(objective.as_str());
                    }
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
                             `{npc}`'s tree that completes it — add an option (reachable from \
                             `root`) with a `complete-objective` effect for `{id}`, else the \
                             objective can never finish"
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
                format!(
                    "quest `{}` sets `mandatory: false`, which is reserved until M3 — set \
                     `mandatory: true` (every v0 quest is on the critical path)",
                    q.id
                ),
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
            "stage-4 quest `depends_on` graph contains a cycle — the plan must be a DAG; remove a \
             `depends_on` edge so the quests form an acyclic order",
        ));
        return; // reachability is meaningless with a cycle
    }

    // Finale must be declared.
    if !planned_ids.contains(plan.finale.as_str()) {
        d.push(Diagnostic::error(
            codes::FINALE_UNKNOWN,
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
                    "quest `{}` is not a (transitive) dependency of finale `{}`, so the plan does \
                     not converge on the finale — add a `depends_on` chain so `{}` eventually \
                     depends on `{}` (or drop `{}` if it is not part of this delve)",
                    q.id, plan.finale, plan.finale, q.id, q.id
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
                    "objective `after` ordering in quest `{}` contains a cycle — the `after` \
                     edges must form a DAG; remove one `after` entry to break the cycle",
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
                format!(
                    "npc role `{name}` is reserved and not implemented in v0 — use role \
                     `quest-giver` or `flavor`. Do NOT raise `dsl_version` to try to enable it: \
                     `vendor`/`boss` are not implemented at any version yet"
                ),
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
                    format!(
                        "objective type `{name}` requires dsl_version 0.3.0 — raise this stage's \
                         `dsl_version` to at least 0.3.0, or remove the objective"
                    ),
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
                    format!(
                        "effect `{name}` requires dsl_version 0.3.0 — raise this stage's \
                         `dsl_version` to at least 0.3.0, or remove the effect"
                    ),
                ));
            }
        });
    }

    reserved_v04(c, d);
    reserved_v05(c, d);
    reserved_v06(c, d);
}

/// DSL v0.6 reserved-feature gating + validation (spec-0013 + task #55): the
/// stage-1 `horizon`/`boundary` world fields (gated on the world stage) and the
/// per-effect `requires_flags` flag gate (gated on the quests stage) are each
/// rejected with `DW0141` before 0.6.0. All default to absent/empty, so a v0.5
/// campaign that uses none is byte-identical. Under a v0.6 campaign the world
/// fields are validated: `horizon: "ocean"` requires a `boundary` (`DW0320`),
/// and `boundary.margin` must lie in `0..=64` (`DW0321`).
fn reserved_v06(c: &Campaign, d: &mut Vec<Diagnostic>) {
    reserved_v06_world(c, d);
    reserved_v06_effect_flags(c, d);
}

/// Per-effect `requires_flags` (task #55) is a v0.6 quests-stage surface: any
/// quest effect (`on_objective_complete` / `on_complete`) or environment-trigger
/// effect that carries a non-empty `requires_flags` under a pre-0.6 quests stage
/// is reserved (`DW0141`). `campaign-complete` cannot carry the field at all.
fn reserved_v06_effect_flags(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if is_v06(c.quests.dsl_version.as_str()) {
        return;
    }
    let msg = "effect `requires_flags` (per-effect flag gating) requires dsl_version 0.6.0 — raise \
               this stage's `dsl_version` to 0.6.0, or remove the field";
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for_each_effect(q, |path, eff| {
            if !eff.requires_flags().is_empty() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/requires_flags"),
                    msg.to_string(),
                ));
            }
        });
    }
    for (i, t) in c.quests.content.triggers.iter().enumerate() {
        for (m, eff) in t.effects.iter().enumerate() {
            if !eff.requires_flags().is_empty() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/triggers/{i}/effects/{m}/requires_flags"),
                    msg.to_string(),
                ));
            }
        }
    }
}

/// Stage-1 `horizon`/`boundary` gating + validation (spec-0013).
fn reserved_v06_world(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::stages::Horizon;

    if !is_v06(c.world.dsl_version.as_str()) {
        if c.world.content.horizon.is_some() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "world",
                "/content/horizon".to_string(),
                "world `horizon` requires dsl_version 0.6.0 — raise this stage's `dsl_version` to \
                 0.6.0, or remove the construct"
                    .to_string(),
            ));
        }
        if c.world.content.boundary.is_some() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "world",
                "/content/boundary".to_string(),
                "world `boundary` requires dsl_version 0.6.0 — raise this stage's `dsl_version` to \
                 0.6.0, or remove the construct"
                    .to_string(),
            ));
        }
        return;
    }

    // `horizon: "ocean"` without a return rule strands wanderers in an infinite sea.
    if matches!(c.world.content.horizon, Some(Horizon::Ocean)) && c.world.content.boundary.is_none()
    {
        d.push(Diagnostic::error(
            codes::OCEAN_NO_BOUNDARY,
            "world",
            "/content/horizon".to_string(),
            "`horizon: \"ocean\"` needs a `boundary` — an infinite swimmable sea with no return \
             rule lets players wander off the map. Add a `boundary` (a bare `{}` uses the default \
             margin), or set `horizon` to `void`"
                .to_string(),
        ));
    }
    // `margin` range check (0..=64).
    if let Some(b) = &c.world.content.boundary
        && !(0..=64).contains(&b.margin)
    {
        d.push(Diagnostic::error(
            codes::BOUNDARY_MARGIN,
            "world",
            "/content/boundary/margin".to_string(),
            format!(
                "`boundary.margin` = {} is out of range — set it to a value in 0..=64 (16 is the \
                 default)",
                b.margin
            ),
        ));
    }
}

/// DSL v0.5 reserved-feature gating (spec-0010): declared world `time`/`weather`
/// and per-area `lighting` (stage 1), plus the `set-time`/`set-weather` effect
/// verbs (stage 5 quests, stage 6 dialogue), are rejected with `DW0141` in a
/// pre-0.5.0 campaign, gated on the stage that carries them. Additive fields
/// default to empty, so a v0.4 campaign that uses none is byte-identical. When
/// `lighting` is present under a v0.5 campaign, `min_light` is range-checked
/// (1..=14, `DW0196`).
fn reserved_v05(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let v05_world = is_v05(c.world.dsl_version.as_str());
    let v05_quests = is_v05(c.quests.dsl_version.as_str());
    let v05_dialogue = is_v05(c.dialogue.dsl_version.as_str());
    let res = |d: &mut Vec<Diagnostic>, stage, path: String, what: &str| {
        d.push(Diagnostic::error(
            codes::RESERVED,
            stage,
            path,
            format!(
                "{what} requires dsl_version 0.5.0 — raise this stage's `dsl_version` to 0.5.0, \
                 or remove the construct"
            ),
        ));
    };

    // Stage 1 — declared time / weather / per-area lighting.
    if !v05_world {
        if c.world.content.time.is_some() {
            res(d, "world", "/content/time".to_string(), "world `time`");
        }
        if c.world.content.weather.is_some() {
            res(
                d,
                "world",
                "/content/weather".to_string(),
                "world `weather`",
            );
        }
        for (i, area) in c.world.content.areas.iter().enumerate() {
            if area.lighting.is_some() {
                res(
                    d,
                    "world",
                    format!("/content/areas/{i}/lighting"),
                    "area `lighting`",
                );
            }
        }
    } else {
        // Range-check min_light (1..=14) where a lighting block is declared.
        for (i, area) in c.world.content.areas.iter().enumerate() {
            if let Some(lighting) = &area.lighting
                && !(1..=14).contains(&lighting.min_light)
            {
                d.push(Diagnostic::error(
                    codes::LIGHTING_RANGE,
                    "world",
                    format!("/content/areas/{i}/lighting/min_light"),
                    format!(
                        "area `{}` `lighting.min_light` = {} is out of range — set it to a value \
                         in 1..=14 (7 is the default)",
                        area.id, lighting.min_light
                    ),
                ));
            }
        }
    }

    // Stage 5 — set-time / set-weather quest effects.
    if !v05_quests {
        for (i, q) in c.quests.content.quests.iter().enumerate() {
            for_each_effect(q, |path, eff| {
                if let Some(name) = eff.v05_effect() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/{path}/type"),
                        &format!("effect `{name}`"),
                    );
                }
            });
        }
    }

    // Stage 6 — set-time / set-weather dialogue effects.
    if !v05_dialogue {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, node) in t.nodes.iter().enumerate() {
                for (k, opt) in node.options.iter().enumerate() {
                    for (m, eff) in opt.effects.iter().enumerate() {
                        if let Some(name) = eff.v05_effect() {
                            res(
                                d,
                                "dialogue",
                                format!(
                                    "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/type"
                                ),
                                &format!("dialogue effect `{name}`"),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// DSL v0.4 reserved-feature gating: every v0.4 construct is rejected with
/// `DW0141` in a pre-0.4.0 campaign, gated on the stage that carries it (mirroring
/// the v0.3 verb gate above). Additive fields default to empty, so a v0.3
/// campaign that uses none is byte-identical.
fn reserved_v04(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let v04_npcs = is_v04(c.npcs.dsl_version.as_str());
    let v04_quests = is_v04(c.quests.dsl_version.as_str());
    let v04_dialogue = is_v04(c.dialogue.dsl_version.as_str());
    let res = |d: &mut Vec<Diagnostic>, stage, path: String, what: &str| {
        d.push(Diagnostic::error(
            codes::RESERVED,
            stage,
            path,
            format!(
                "{what} requires dsl_version 0.4.0 — raise this stage's `dsl_version` to at least \
                 0.4.0, or remove the construct"
            ),
        ));
    };

    // Stage 2 — mannequin skins.
    if !v04_npcs {
        for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
            if npc.skin.is_some() {
                res(d, "npcs", format!("/content/npcs/{i}/skin"), "npc `skin`");
            }
        }
    }

    // Stage 5 — objective stealth/prop, wave attributes/effects, v0.4 effects,
    // named give-item, environment triggers.
    if !v04_quests {
        for (i, q) in c.quests.content.quests.iter().enumerate() {
            for (j, o) in q.objectives.iter().enumerate() {
                if o.stealth() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/stealth"),
                        "objective `stealth`",
                    );
                }
                if o.prop().is_some() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/prop"),
                        "objective `prop`",
                    );
                }
            }
            for_each_effect(q, |path, eff| {
                if let Some(name) = eff.v04_effect() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/{path}/type"),
                        &format!("effect `{name}`"),
                    );
                }
                if eff.give_item_named() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/{path}/name"),
                        "give-item `name`",
                    );
                }
            });
        }
        for (i, w) in c.quests.content.waves.iter().enumerate() {
            for (k, m) in w.mobs.iter().enumerate() {
                if m.attributes.is_some() {
                    res(
                        d,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/attributes"),
                        "wave-mob `attributes`",
                    );
                }
                if !m.effects.is_empty() {
                    res(
                        d,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/effects"),
                        "wave-mob `effects`",
                    );
                }
            }
        }
        if !c.quests.content.triggers.is_empty() {
            res(
                d,
                "quests",
                "/content/triggers".to_string(),
                "environment `triggers`",
            );
        }
    }

    // Stage 6 — dialogue requires_flags + set-flag effect.
    if !v04_dialogue {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, node) in t.nodes.iter().enumerate() {
                for (k, opt) in node.options.iter().enumerate() {
                    if !opt.requires_flags.is_empty() {
                        res(
                            d,
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/requires_flags"),
                            "dialogue option `requires_flags`",
                        );
                    }
                    for (m, eff) in opt.effects.iter().enumerate() {
                        if let Some(name) = eff.v04_effect() {
                            res(
                                d,
                                "dialogue",
                                format!(
                                    "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/type"
                                ),
                                &format!("dialogue effect `{name}`"),
                            );
                        }
                    }
                }
            }
        }
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
                    "npc anchor `{}` is not provided by the prefab bound to area `{}` — use an \
                     anchor the prefab exposes, or bind a prefab/pool that carries `{}`. Anchor \
                     names come from prefab metadata; do NOT invent one",
                    npc.anchor, npc.area, npc.anchor
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
                    format!(
                        "objective anchor `{anchor}` is not provided by the prefab bound to this \
                         quest's area — use an anchor the prefab exposes (anchor names come from \
                         prefab metadata; do NOT invent one)"
                    ),
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
                    format!(
                        "`open-gate` anchor `{anchor}` is not provided by the prefab bound to \
                         this quest's area — use a gate anchor the prefab exposes (anchor names \
                         come from prefab metadata; do NOT invent one)"
                    ),
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
                    format!(
                        "kit item `{}` is not in the pinned 1.21.11 item registry — use a valid \
                         namespaced item id (e.g. `minecraft:iron_sword`)",
                        it.item
                    ),
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
                format!(
                    "area `prefab_pool` `{pool}` is not declared in the prefab metadata — bind a \
                     pool that exists in the prefabs dir, or add `{pool}` to the prefab library. \
                     This is a prefab-library/naming issue, not a quest-logic one"
                ),
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
                format!(
                    "planned quest `{}` has no stage-5 expansion — add a stage-5 quest with id \
                     `{}` (objectives/effects), or drop it from the stage-4 plan",
                    q.id, q.id
                ),
            ));
        }
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if !planned_ids.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                codes::QUEST_NOT_PLANNED,
                "quests",
                format!("/content/quests/{i}"),
                format!(
                    "stage-5 quest `{}` is not planned in stage 4 — add a stage-4 plan entry with \
                     id `{}` (every quest must be planned), or remove this expansion",
                    q.id, q.id
                ),
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
                format!(
                    "npc `{}` has no stage-6 dialogue tree — every stage-2 npc needs exactly one \
                     tree; add a dialogue tree for `{}`, or remove the npc",
                    npc.id, npc.id
                ),
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
                    "dialogue tree targets npc `{}`, which is not declared in stage 2 — declare \
                     that npc in stage 2, or point this tree at an existing npc",
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
                format!(
                    "malformed wave id `{}` — wave ids must be lowercase kebab-case with the \
                     `wave/` prefix (e.g. `wave/ambush`)",
                    w.id
                ),
            ));
        }
        if !seen_waves.insert(w.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/waves/{i}/id"),
                format!(
                    "duplicate wave id `{}` — rename one so every wave id is unique",
                    w.id
                ),
            ));
        }
        declared_waves.insert(w.id.as_str());
        for (k, m) in w.mobs.iter().enumerate() {
            if !entities.contains(&m.entity) {
                d.push(Diagnostic::error(
                    codes::ENTITY_UNKNOWN,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/entity"),
                    format!(
                        "wave-mob entity `{}` is not a known 1.21.11 entity id — use a valid \
                         namespaced entity id (e.g. `minecraft:zombie`)",
                        m.entity
                    ),
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
    // v0.4: flags/waves may also come from dialogue `set-flag` effects and
    // environment-trigger effects. Empty for v0.2/v0.3 campaigns (no such
    // constructs), so their flag resolution is unchanged.
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some(f) = eff.set_flag() {
                        declared_flags.insert(f.as_str());
                    }
                }
            }
        }
    }
    for t in &quests.triggers {
        for eff in &t.effects {
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
                            format!(
                                "`kill` objective references unknown wave `{wave}` — declare it \
                                 in the stage-5 `waves` section or correct the reference"
                            ),
                        ));
                    } else if !spawned_waves.contains(wave.as_str()) {
                        d.push(Diagnostic::error(
                            codes::WAVE_NEVER_SPAWNED,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/wave"),
                            format!(
                                "wave `{wave}` is killed but never spawned by any `spawn-wave` \
                                 effect — a wave must be spawned before its `kill` objective is \
                                 reachable; add a `spawn-wave` effect for `{wave}` on an earlier \
                                 objective/quest"
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
                            format!(
                                "collect item `{item}` is not in the pinned 1.21.11 item \
                                 registry — use a valid namespaced item id (e.g. \
                                 `minecraft:emerald`)"
                            ),
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
                            format!(
                                "`interact.requires_item` `{it}` is not in the pinned 1.21.11 \
                                 item registry — use a valid namespaced item id (e.g. \
                                 `minecraft:tripwire_hook`)"
                            ),
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
                            "objective `requires_flags` references flag `{f}`, which no \
                             `set-flag` effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` \
                             effect on an earlier objective/quest, or correct the flag name"
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
                    format!(
                        "`spawn-wave` effect references unknown wave `{w}` — declare it in the \
                         stage-5 `waves` section or correct the reference"
                    ),
                ));
            }
            if let Some(it) = eff.give_item()
                && !items.contains(it)
            {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "quests",
                    format!("/content/quests/{i}/{path}/item"),
                    format!(
                        "`give-item` item `{it}` is not in the pinned 1.21.11 item registry — use \
                         a valid namespaced item id (e.g. `minecraft:golden_apple`)"
                    ),
                ));
            }
            // v0.6: per-effect `requires_flags` must resolve to a produced flag,
            // mirroring the objective/trigger `requires_flags` check (DW0172).
            for (n, f) in eff.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/{path}/requires_flags/{n}"),
                        format!(
                            "effect `requires_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                             earlier, or correct the flag name"
                        ),
                    ));
                }
            }
        });
    }

    // v0.6: environment-trigger effect `requires_flags` resolution (DW0172).
    for (i, t) in quests.triggers.iter().enumerate() {
        for (m, eff) in t.effects.iter().enumerate() {
            for (n, f) in eff.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/triggers/{i}/effects/{m}/requires_flags/{n}"),
                        format!(
                            "effect `requires_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                             earlier, or correct the flag name"
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// DSL v0.4 — props, set-block, narrate, wave tuning, lifecycle, skins, triggers
// ---------------------------------------------------------------------------

/// The v0.4 semantic checks (run only for `dsl_version` 0.4.0):
///
/// - mannequin skins: `texture_id` syntax + uniqueness (`DW0190`);
/// - wave-mob `effects[].effect` ids in the effect registry (`DW0192`);
/// - `set-block` / `interact.prop` block ids in the block registry (`DW0193`);
/// - environment triggers: id syntax + uniqueness (`DW0194`), `at` anchor
///   resolves against some area (`DW0142`), `requires_flags` resolve (`DW0172`),
///   effect refs (item/block/wave) validated;
/// - lifecycle: `despawn-npc` / `move-npc` npc refs resolve (`DW0112`);
/// - dialogue `requires_flags` resolve (`DW0172`), and a `talk-to` with only
///   flag-gated completing options is a potential deadlock (`DW0191`);
/// - a `talk-to` targeting an NPC despawned earlier on a dependency path
///   (`DW0195`).
fn v04_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    blocks: &dyn BlockRegistry,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();

    // area anchor sets (single-prefab areas only) + whether any pool area exists.
    let mut area_anchors: BTreeMap<&str, &BTreeSet<String>> = BTreeMap::new();
    let mut has_pool_area = false;
    for a in &c.world.content.areas {
        if let Some(prefab) = &a.prefab {
            if let Some(set) = anchors.anchors_for(prefab) {
                area_anchors.insert(a.id.as_str(), set);
            }
        } else if a.prefab_pool.is_some() {
            has_pool_area = true;
        }
    }
    let known_anchor: BTreeSet<&str> = area_anchors
        .values()
        .flat_map(|s| s.iter().map(String::as_str))
        .collect();
    // Lenient anchor resolution: flag only when the name is provided by no known
    // area AND there are no pool areas (which the compiler resolves later).
    let anchor_resolvable = |name: &str| has_pool_area || known_anchor.contains(name);

    // Declared flags across quest / dialogue / trigger `set-flag` effects.
    let mut flags: BTreeSet<&str> = BTreeSet::new();
    for q in &quests.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            if let Some(f) = e.set_flag() {
                flags.insert(f.as_str());
            }
        }
    }
    for t in &quests.triggers {
        for e in &t.effects {
            if let Some(f) = e.set_flag() {
                flags.insert(f.as_str());
            }
        }
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for e in &opt.effects {
                    if let Some(f) = e.set_flag() {
                        flags.insert(f.as_str());
                    }
                }
            }
        }
    }
    let declared_waves: BTreeSet<&str> = quests.waves.iter().map(|w| w.id.as_str()).collect();

    // --- skins (spec-0009) ---
    let mut seen_skins: BTreeSet<&str> = BTreeSet::new();
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(skin) = &npc.skin {
            if !is_kebab(&skin.texture_id) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "npcs",
                    format!("/content/npcs/{i}/skin/texture_id"),
                    format!(
                        "skin `texture_id` `{}` is malformed — it must be a bare kebab token \
                         (e.g. `keeper-armor`), matching the `skins/<texture_id>.png` filename",
                        skin.texture_id
                    ),
                ));
            }
            if !seen_skins.insert(skin.texture_id.as_str()) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "npcs",
                    format!("/content/npcs/{i}/skin/texture_id"),
                    format!(
                        "duplicate skin `texture_id` `{}` — each mannequin needs a distinct \
                         texture; rename one (and its `skins/<id>.png`)",
                        skin.texture_id
                    ),
                ));
            }
        }
    }

    // --- wave-mob effects + attributes ---
    for (i, w) in quests.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            for (e, eff) in m.effects.iter().enumerate() {
                if !effects.contains(&eff.effect) {
                    d.push(Diagnostic::error(
                        codes::EFFECT_UNKNOWN,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/effects/{e}/effect"),
                        format!(
                            "wave-mob effect `{}` is not a known 1.21.11 status-effect id — use a \
                             valid namespaced effect id (e.g. `minecraft:strength`)",
                            eff.effect
                        ),
                    ));
                }
            }
        }
    }

    // --- block ids: interact props + set-block effects (quest + trigger) ---
    for (i, q) in quests.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            if let Some(prop) = o.prop() {
                check_block_field(
                    blocks,
                    &prop.block,
                    format!("/content/quests/{i}/objectives/{j}/prop/block"),
                    "interact.prop",
                    "minecraft:lever",
                    d,
                );
            }
        }
        for_each_effect(q, |path, eff| {
            check_effect_v04(
                eff,
                blocks,
                &declared_waves,
                &format!("/content/quests/{i}/{path}"),
                &npc_ids,
                d,
            );
        });
    }

    // --- environment triggers ---
    let mut seen_triggers: BTreeSet<&str> = BTreeSet::new();
    for (i, t) in quests.triggers.iter().enumerate() {
        if !t.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "malformed trigger id `{}` — trigger ids must be lowercase kebab-case with \
                     the `trigger/` prefix (e.g. `trigger/pressure-plate`)",
                    t.id
                ),
            ));
        }
        if !seen_triggers.insert(t.id.as_str()) {
            d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "duplicate trigger id `{}` — rename one so every trigger id is unique",
                    t.id
                ),
            ));
        }
        if !anchor_resolvable(t.at.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `at` anchor `{}` is not provided by any area's prefab — set `at` to \
                     an anchor some area's prefab exposes (anchor names come from prefab metadata; \
                     do NOT invent one)",
                    t.at
                ),
            ));
        }
        if let TriggerOn::Approach { range } = &t.on
            && *range == 0
        {
            d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/on/range"),
                "`approach` trigger `range` must be > 0 — set a positive block radius (e.g. 3)"
                    .to_string(),
            ));
        }
        for (m, f) in t.requires_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/triggers/{i}/requires_flags/{m}"),
                    format!(
                        "trigger `requires_flags` references flag `{f}`, which no `set-flag` \
                         effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                         somewhere, or correct the flag name"
                    ),
                ));
            }
        }
        for (m, eff) in t.effects.iter().enumerate() {
            check_effect_v04(
                eff,
                blocks,
                &declared_waves,
                &format!("/content/triggers/{i}/effects/{m}"),
                &npc_ids,
                d,
            );
        }
    }

    // --- dialogue requires_flags resolution + flag-deadlock guard ---
    dialogue_v04(c, &flags, d);

    // --- despawned-npc references (DW0195) ---
    despawned_ref_check(c, &npc_ids, d);
}

/// Split a block field into its base id and (optional) blockstate suffix,
/// validating the suffix's syntax (DSL v0.6, task #55). Returns the base id to
/// check against the block registry; `Err(reason)` when a `[...]` suffix is
/// present but malformed (unbalanced brackets, empty, or a token that is not a
/// lowercase `key=value`). A well-formed state string is passed through verbatim
/// to `setblock` — vanilla validates the property names/values against the
/// block's own state definition, so the compiler only guards the surface syntax.
fn split_blockstate(block: &str) -> Result<&str, String> {
    let Some(open) = block.find('[') else {
        return Ok(block);
    };
    let rest = &block[open..];
    if !rest.ends_with(']') {
        return Err(format!(
            "malformed blockstate in `{block}` — the `[...]` suffix must close with `]` \
             (e.g. `minecraft:grindstone[face=floor]`)"
        ));
    }
    let inner = &rest[1..rest.len() - 1];
    if inner.trim().is_empty() {
        return Err(format!(
            "malformed blockstate in `{block}` — the `[...]` suffix is empty; drop the brackets or \
             add a `key=value` property"
        ));
    }
    let is_token = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    };
    for prop in inner.split(',') {
        let mut kv = prop.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        match kv.next().map(str::trim) {
            Some(val) if is_token(key) && is_token(val) => {}
            _ => {
                return Err(format!(
                    "malformed blockstate in `{block}` — each property must be `key=value` with \
                     lowercase `[a-z0-9_]` tokens (e.g. `face=floor`)"
                ));
            }
        }
    }
    Ok(&block[..open])
}

/// Validate a block field (interact prop / set-block) allowing an optional
/// verbatim blockstate suffix (DSL v0.6). The base id must be in the block
/// registry (`DW0193`); a malformed `[...]` suffix reuses `DW0193` with a clear
/// message. Always a quests-stage diagnostic.
fn check_block_field(
    blocks: &dyn BlockRegistry,
    block: &str,
    path: String,
    kind: &str,
    example: &str,
    d: &mut Vec<Diagnostic>,
) {
    match split_blockstate(block) {
        Ok(base) => {
            if !blocks.contains(base) {
                d.push(Diagnostic::error(
                    codes::BLOCK_UNKNOWN,
                    "quests",
                    path,
                    format!(
                        "`{kind}` block `{block}` is not a known 1.21.11 block id — use a valid \
                         namespaced block id (e.g. `{example}`)"
                    ),
                ));
            }
        }
        Err(reason) => {
            d.push(Diagnostic::error(
                codes::BLOCK_UNKNOWN,
                "quests",
                path,
                reason,
            ));
        }
    }
}

/// Validate a v0.4-relevant [`QuestEffect`]'s refs: `set-block` block id
/// (`DW0193`), `despawn-npc`/`move-npc` npc ids (`DW0112`), `move-npc` speed
/// positivity. Item/wave/flag refs are covered by the shared v0.3 checks.
fn check_effect_v04(
    eff: &QuestEffect,
    blocks: &dyn BlockRegistry,
    _declared_waves: &BTreeSet<&str>,
    base_path: &str,
    npc_ids: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    match eff {
        QuestEffect::SetBlock { block, .. } => {
            check_block_field(
                blocks,
                block,
                format!("{base_path}/block"),
                "set-block",
                "minecraft:air",
                d,
            );
        }
        QuestEffect::DespawnNpc { npc, .. } | QuestEffect::MoveNpc { npc, .. }
            if !npc_ids.contains(npc.as_str()) =>
        {
            let verb = eff.v04_effect().unwrap_or("lifecycle");
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("{base_path}/npc"),
                format!(
                    "`{verb}` references unknown npc `{npc}` — declare it in stage 2 or correct \
                     the reference"
                ),
            ));
        }
        _ => {}
    }
}

/// Dialogue v0.4: option `requires_flags` resolve against declared flags
/// (`DW0172`); a `talk-to` whose completing options are all flag-gated is a
/// potential deadlock (`DW0191`, spec-0008 §1).
fn dialogue_v04(c: &Campaign, flags: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) {
    use crate::stages::DialogueEffect;
    // Option requires_flags resolution.
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                for (m, f) in opt.requires_flags.iter().enumerate() {
                    if !flags.contains(f.as_str()) {
                        d.push(Diagnostic::error(
                            codes::FLAG_UNKNOWN,
                            "dialogue",
                            format!(
                                "/content/dialogues/{i}/nodes/{j}/options/{k}/requires_flags/{m}"
                            ),
                            format!(
                                "dialogue option `requires_flags` references flag `{f}`, which no \
                                 `set-flag` effect ever produces — add a `set-flag {{ flag: \
                                 \"{f}\" }}` effect somewhere, or correct the flag name"
                            ),
                        ));
                    }
                }
            }
        }
    }
    // Per-NPC: objectives completed by an UNGATED option in that npc's tree.
    let mut ungated_completes: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut any_completes: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for tree in &c.dialogue.content.dialogues {
        let npc = tree.npc.as_str();
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let DialogueEffect::CompleteObjective { objective } = eff {
                        any_completes
                            .entry(npc)
                            .or_default()
                            .insert(objective.as_str());
                        if opt.requires_flags.is_empty() {
                            ungated_completes
                                .entry(npc)
                                .or_default()
                                .insert(objective.as_str());
                        }
                    }
                }
            }
        }
    }
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Objective::TalkTo { id, npc, .. } = o {
                let npc = npc.as_str();
                let oid = id.as_str();
                let completed = any_completes.get(npc).is_some_and(|s| s.contains(oid));
                let ungated = ungated_completes.get(npc).is_some_and(|s| s.contains(oid));
                // Only when it IS completed somewhere (else DW0123 fires) but every
                // completing option is flag-gated.
                if completed && !ungated {
                    d.push(Diagnostic::error(
                        codes::DIALOGUE_FLAG_DEADLOCK,
                        "quests",
                        format!("/content/quests/{qi}/objectives/{oi}"),
                        format!(
                            "`talk-to` objective `{id}` has no ungated completing dialogue option \
                             in npc `{npc}`'s tree — every completing option is `requires_flags`- \
                             gated, so it can deadlock the moment it activates; keep at least one \
                             completing option with no `requires_flags`"
                        ),
                    ));
                }
            }
        }
    }
}

/// DW0195: a `talk-to` targeting an NPC despawned by an effect that runs strictly
/// before it on the quest dependency graph. Conservative: quest-ancestor despawn
/// (via `on_complete`) or same-quest earlier-objective despawn (via
/// `on_objective_complete` on a prerequisite `after` objective).
fn despawned_ref_check(c: &Campaign, _npc_ids: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) {
    // Quest transitive ancestors (a quest completes before its dependents start).
    let deps: BTreeMap<&str, &Vec<crate::ids::QuestId>> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), &q.depends_on))
        .collect();
    let ancestors = |q: &str| -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        let mut stack = vec![q];
        while let Some(cur) = stack.pop() {
            if let Some(ds) = deps.get(cur) {
                for dep in ds.iter() {
                    if out.insert(dep.as_str()) {
                        stack.push(dep.as_str());
                    }
                }
            }
        }
        out
    };

    // Where each npc is despawned: quests that despawn it on completion.
    let mut despawn_quest: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for q in &c.quests.content.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            if let Some(npc) = e.despawn_npc() {
                despawn_quest
                    .entry(npc.as_str())
                    .or_default()
                    .insert(q.id.as_str());
            }
        }
    }
    if despawn_quest.is_empty() {
        return;
    }
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let anc = ancestors(q.id.as_str());
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Objective::TalkTo { npc, .. } = o
                && let Some(dq) = despawn_quest.get(npc.as_str())
                && dq.iter().any(|dqid| anc.contains(dqid))
            {
                d.push(Diagnostic::error(
                    codes::NPC_DESPAWNED_REF,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}/npc"),
                    format!(
                        "`talk-to` targets npc `{npc}`, which a prerequisite quest despawns — the \
                         npc is gone by the time this objective activates; talk to `{npc}` before \
                         the quest that despawns it, or drop the `despawn-npc`"
                    ),
                ));
            }
        }
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
            format!(
                "objective `{field}` anchor `{anchor}` is not provided by the prefab bound to \
                 this quest's area — use an anchor the prefab exposes (anchor names come from \
                 prefab metadata; do NOT invent one)"
            ),
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
