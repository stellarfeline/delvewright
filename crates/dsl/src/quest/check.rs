//! The quest object's validation (ADR-0031): what `dsl::validate` checks over a
//! stage-5 quest — its objectives' ordering, references, anchors and props — and
//! over the verbs its effects carry at every effect root (status effects, the
//! `sequence` nesting rule, a `carrier: "one"` give, `give-item` enchantments).

use crate::cutscene::check_cutscene_shape;
use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::{AnchorRegistry, BlockRegistry, EffectRegistry, ItemRegistry};
use crate::validate::{
    AnchorProviders, MIN_CONTAINER_SLOTS, check_block_field, check_enchantments, check_stack_count,
    for_each_effect_deep, graph_has_cycle, produced_flags, station_kind_diag,
};
use crate::{Objective, PlannedQuest, QuestEffect, Verb};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn after_ordering(c: &Campaign, d: &mut Vec<Diagnostic>) {
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

/// DSL v0.10 status-effect checks (spec-0031): a real effect id, a duration that
/// is actually a duration, and the one *pattern* that reintroduces the hazard the
/// mandatory duration exists to remove.
///
/// ## Why the duration is not enough on its own
///
/// `give-effect` has no infinite form: `seconds` is required and bounded, so a
/// grant always ends by itself. That is the whole design, and it can still be
/// defeated by two effects that are individually fine — grant blindness for an
/// hour, clear it four ticks later. The clear is then the real removal, and a
/// bundle that does not reach it (a logout, a crash, a death mid-chain, a
/// `sequence` whose remaining `schedule` never runs) leaves the player blind for
/// the rest of the hour. `DW0540` is that pattern, and it fires on exactly the
/// grants that are still LIVE when their clear arrives: where the duration
/// expires first, the duration is the removal and there is nothing to say.
///
/// ## What "the same sequence" means, mechanically
///
/// A bundle's own timeline: every effect it fires, at the tick offset it fires
/// on. A plain member runs at offset 0; a directly-nested `sequence`'s members
/// run at their step's `at_ticks` (nested sequences are `DW0329`, so the
/// expansion terminates). Conditional continuations — `on_arrive`, `on_caught`,
/// `on_respawn`, `on_rest` — are NOT folded in: they are separate bundles with
/// their own timelines, and each is walked as one. A clear that hangs off an
/// arrival is strictly more fragile than one on a fixed tick, and it is the
/// mandatory duration, not this rule, that keeps that case survivable.
pub(crate) fn status_effect_checks(
    c: &Campaign,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        status_effect_bundle(site.stage, &site.path, list, effects, d);
    });
}

/// One effect bundle: its own timeline is checked for the grant/clear pattern,
/// then each member is checked individually and its nested bundles are walked.
fn status_effect_bundle(
    stage: &'static str,
    path: &str,
    list: &[QuestEffect],
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let tl = effect_timeline(path, list);
    check_grant_removal(stage, &tl, d);
    for (_, p, eff) in &tl {
        check_one_status_effect(stage, p, eff, effects, d);
        for (pseg, _key, inner) in eff.nested_effect_lists_labeled() {
            status_effect_bundle(stage, &format!("{p}/{pseg}"), inner, effects, d);
        }
    }
}

/// A bundle's timeline: `(tick offset, json pointer, effect)` for every effect
/// that runs on it, in declaration order.
///
/// A `sequence` is REPLACED by its steps' effects at their `at_ticks`, so the
/// timeline is what actually happens rather than what is written — which is the
/// only view in which "the grant is still live when the clear fires" is a
/// question with an answer.
fn effect_timeline<'a>(path: &str, list: &'a [QuestEffect]) -> Vec<(u32, String, &'a QuestEffect)> {
    let mut out = Vec::new();
    for (i, eff) in list.iter().enumerate() {
        match &eff.verb {
            Verb::Sequence { steps } => {
                for (s, step) in steps.iter().enumerate() {
                    for (k, inner) in step.effects.iter().enumerate() {
                        out.push((
                            step.at_ticks,
                            format!("{path}/{i}/steps/{s}/effects/{k}"),
                            inner,
                        ));
                    }
                }
            }
            _ => out.push((0, format!("{path}/{i}"), eff)),
        }
    }
    out
}

/// `DW0540`: a grant that is still live when a clear in the same bundle removes
/// it.
///
/// Each grant is paired with the EARLIEST clear that could remove it: the one
/// smallest in `(tick, declaration position)` among those strictly after the
/// grant, that either names the same effect or names none at all (vanilla's
/// `effect clear <targets>` with no id clears everything, and so does this verb).
///
/// **Ordered by the key, never by the loop index.** A bundle's steps are declared
/// in whatever order the author wrote them, not in tick order, so "the next clear
/// in the list" and "the clear that actually fires first" are different effects
/// the moment a `sequence` declares `at_ticks: 40` above `at_ticks: 5`. Taking
/// the first in declaration order made the rule miss exactly the live removal it
/// exists to find — and, symmetrically, miss a clear declared *above* a grant but
/// scheduled after it.
fn check_grant_removal(
    stage: &'static str,
    timeline: &[(u32, String, &QuestEffect)],
    d: &mut Vec<Diagnostic>,
) {
    for (gi, (g_tick, g_path, g_eff)) in timeline.iter().enumerate() {
        let Some((id, seconds, _, _, _)) = g_eff.give_effect() else {
            continue;
        };
        // `blindness` and `minecraft:blindness` are one effect to vanilla and to
        // the registry, so they are one effect here (`namespaced_effect_id`) —
        // an exact string compare would have made the rule silently miss the
        // pair whenever the two sites spelled the id differently.
        let granted = crate::registry::namespaced_effect_id(id);
        let removal = timeline
            .iter()
            .enumerate()
            .filter(|(ci, (t, _, _))| (*t, *ci) > (*g_tick, gi))
            .filter_map(|(ci, (t, p, e))| match e.clear_effect() {
                Some((None, _)) => Some(((*t, ci), t, p, "clears every effect")),
                Some((Some(c), _)) if crate::registry::namespaced_effect_id(c) == granted => {
                    Some(((*t, ci), t, p, "clears it"))
                }
                _ => None,
            })
            .min_by_key(|(key, _, _, _)| *key)
            .map(|(_, t, p, what)| (*t, p, what));
        let Some((c_tick, c_path, what)) = removal else {
            continue;
        };
        let live_ticks = seconds.saturating_mul(20);
        let elapsed = c_tick - g_tick;
        if elapsed >= live_ticks {
            // The duration ended first, so the duration IS the removal and the
            // clear is inert. That is a different (and much milder) finding than
            // this rule makes, so this rule says nothing about it.
            continue;
        }
        d.push(Diagnostic::error(
            codes::EFFECT_CLEARED_LIVE,
            stage,
            g_path.clone(),
            format!(
                "this `give-effect` grants `{id}` for {seconds}s ({live_ticks} ticks) and the \
                 `clear-effect` at `{c_path}`, {elapsed} tick(s) later in the same bundle, {what} \
                 while it is still live — so the CLEAR is what ends this grant, not its duration. \
                 Every path that does not reach that clear (a logout, a crash, a death mid-chain, \
                 a `sequence` whose remaining schedule never runs) leaves the player holding \
                 `{id}` for the rest of the {seconds}s. Set `seconds` to how long the effect \
                 should actually last — {elapsed} tick(s) here, so {} — and drop the \
                 `clear-effect`: a duration expires with no cooperation from anything. \
                 `clear-effect` is for effects this campaign did not grant.",
                if elapsed == 0 {
                    "the grant is doing nothing at all".to_string()
                } else {
                    format!("`seconds`: {}", elapsed.div_ceil(20).max(1))
                }
            ),
        ));
    }
}

/// The per-effect half: a real status-effect id (`DW0192`, the same registry
/// wave-mob effects and kit potions validate against) and a duration/amplifier
/// inside vanilla's own field widths (`DW0541`).
fn check_one_status_effect(
    stage: &'static str,
    path: &str,
    eff: &QuestEffect,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    for (sub, id) in eff.status_effect_refs() {
        if !effects.contains(id) {
            d.push(Diagnostic::error(
                codes::EFFECT_UNKNOWN,
                stage,
                format!("{path}/{sub}"),
                format!(
                    "`{}` names status effect `{id}`, which is not in the pinned 1.21.11 \
                     `mob_effect` registry — use a valid namespaced effect id (e.g. \
                     `minecraft:blindness`)",
                    eff.verb.tag()
                ),
            ));
        }
    }
    let Some((_, seconds, amplifier, _, _)) = eff.give_effect() else {
        return;
    };
    if seconds == 0 || seconds > crate::MAX_EFFECT_SECONDS {
        d.push(Diagnostic::error(
            codes::EFFECT_GRANT_BOUNDS,
            stage,
            format!("{path}/seconds"),
            format!(
                "`give-effect` duration {seconds}s is out of range — it must be between 1 and {} \
                 seconds. {}",
                crate::MAX_EFFECT_SECONDS,
                if seconds == 0 {
                    "Zero grants nothing at all: the effect is applied and gone before the next \
                     tick, so the beat reports green and the player sees nothing."
                } else {
                    "The ceiling is vanilla's own field width (past the 10-hour delve ceiling), \
                     so a value above it is a duration typed in ticks or milliseconds."
                }
            ),
        ));
    }
    if amplifier > crate::MAX_POTION_AMPLIFIER {
        d.push(Diagnostic::error(
            codes::EFFECT_GRANT_BOUNDS,
            stage,
            format!("{path}/amplifier"),
            format!(
                "`give-effect` amplifier {amplifier} is out of range — vanilla stores it in an \
                 unsigned byte, so {} is the end of the field, not a policy",
                crate::MAX_POTION_AMPLIFIER
            ),
        ));
    }
}

/// True if `e` is (or transitively reaches, via a `move-actor` / `move-npc`
/// `on_arrive`) a `sequence` — the recursion `DW0329` forbids inside another
/// sequence's steps.
fn reaches_sequence(e: &QuestEffect) -> bool {
    match &e.verb {
        Verb::Sequence { .. } => true,
        Verb::MoveActor { on_arrive, .. } | Verb::MoveNpc { on_arrive, .. } => {
            on_arrive.iter().any(reaches_sequence)
        }
        _ => false,
    }
}

/// Reject a `sequence` nested inside another `sequence` (`DW0329`). Recurses into a
/// `move-actor` / `move-npc` `on_arrive` (a sequence there is legal — not yet
/// inside a sequence) but not into an already-flagged sequence's steps (avoids
/// double-reporting).
pub(crate) fn check_no_nested_sequence(effs: &[QuestEffect], path: &str, d: &mut Vec<Diagnostic>) {
    for e in effs {
        match &e.verb {
            Verb::Sequence { steps } => {
                for s in steps {
                    for inner in &s.effects {
                        if reaches_sequence(inner) {
                            d.push(Diagnostic::error(
                                codes::NESTED_SEQUENCE,
                                "quests",
                                path.to_string(),
                                "a `sequence` effect is nested inside another `sequence` — \
                                 timelines do not recurse (spec-0014). Flatten the inner steps \
                                 into the outer timeline (shift their `at_ticks` by the inner \
                                 sequence's start), or drive the second beat from a flag/objective"
                                    .to_string(),
                            ));
                        }
                    }
                }
            }
            Verb::MoveActor { on_arrive, .. } | Verb::MoveNpc { on_arrive, .. } => {
                check_no_nested_sequence(on_arrive, path, d);
            }
            _ => {}
        }
    }
}

/// Reject a `carrier: "one"` `give-item` inside a **scheduler-only** bundle
/// (`DW0357`, spec-0018).
///
/// `carrier: "one"` means "hand this one quest prop to the player whose action
/// earned it". A bundle the vanilla scheduler re-invokes with the **server**
/// command source has no acting player, so the effect has no defensible
/// recipient. The party-wide default (absent `carrier`) is always fine — it
/// addresses `@a`.
///
/// Which nested bundle has an acting player is the DSL's one statement of it,
/// [`QuestEffect::nested_effect_dispatch`]: a `move-npc`/`move-actor`
/// `on_arrive` and a `bonfire`'s `on_rest` never do; a `set-checkpoint`'s
/// `on_respawn` and a `begin-stealth`'s `on_caught` always do; a `sequence`
/// step has one exactly where the timeline was started with one, because the
/// timeline carries its actor across its `schedule`s (spec-0085 §3.2). The
/// root's own top level is not refused here — a polled root lowers a
/// `carrier: "one"` give to the party, which is what an absent carrier says.
fn check_carrier_one_not_scheduled(
    effs: &[QuestEffect],
    path: &str,
    has_actor: bool,
    top: bool,
    d: &mut Vec<Diagnostic>,
) {
    for e in effs {
        if !has_actor && !top && e.gives_to_one() {
            d.push(Diagnostic::error(
                codes::PARTY_CARRIER_SCHEDULED,
                "quests",
                path.to_string(),
                "a `give-item` with `carrier: \"one\"` sits in a bundle only the scheduler ever \
                 runs with no acting player (a `move-npc`/`move-actor` `on_arrive`, a `bonfire`'s \
                 `on_rest`, or a `sequence` step of a timeline started where nobody acted — a \
                 trigger's effects, a trap's payload, a shortcut's `on_unlock`). Those run with \
                 the server command source — there is no acting player to hand the prop to, so \
                 the give would silently reach nobody. Drop `carrier` to arm the whole party, or \
                 move the hand-off onto the beat a player completes"
                    .to_string(),
            ));
        }
        for (list, how) in e.nested_effect_dispatch() {
            check_carrier_one_not_scheduled(list, path, how.has_actor(has_actor), false, d);
        }
    }
}

/// `give-item` enchantments (`DW0433`/`DW0434`) at **every effect root**, at
/// any nesting depth: the same checks a `loot` stack and an equipped piece get,
/// because it is the same field on the same object — an item stack.
pub(crate) fn give_item_enchantment_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let reg = crate::registry::VendoredEnchantmentRegistry::v1_21_11();
    fn walk(
        stage: &'static str,
        path: &str,
        list: &[QuestEffect],
        reg: &dyn crate::registry::EnchantmentRegistry,
        d: &mut Vec<Diagnostic>,
    ) {
        for (n, eff) in list.iter().enumerate() {
            let p = format!("{path}/{n}");
            if let Verb::GiveItem {
                item, enchantments, ..
            } = &eff.verb
            {
                check_enchantments(
                    enchantments,
                    &format!("`give-item` item `{item}`"),
                    stage,
                    &format!("{p}/enchantments"),
                    reg,
                    d,
                );
            }
            for (seg, _key, inner) in eff.nested_effect_lists_labeled() {
                walk(stage, &format!("{p}/{seg}"), inner, reg, d);
            }
        }
    }
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        walk(site.stage, &site.path, list, &reg, d);
    });
}

/// Validate a v0.4-relevant [`QuestEffect`]'s refs: `set-block` block id
/// (`DW0193`), `despawn-npc`/`move-npc` npc ids (`DW0112`), `move-npc` speed
/// positivity, `cutscene` shape (`DW0199`). Item/wave/flag refs are covered by
/// the shared v0.3 checks.
pub(crate) fn check_effect_v04(
    eff: &QuestEffect,
    blocks: &dyn BlockRegistry,
    _declared_waves: &BTreeSet<&str>,
    base_path: &str,
    npc_ids: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    check_cutscene_shape(eff, base_path, d);
    match &eff.verb {
        Verb::SetBlock { block, .. } => {
            check_block_field(
                blocks,
                block,
                format!("{base_path}/block"),
                "set-block",
                "minecraft:air",
                d,
            );
        }
        // The v0.10 region write (spec-0031) places the same kind of block over a
        // box instead of a cell, so it is the same `DW0193` check — the registry
        // belongs to "a block id an author wrote", not to `set-block`.
        Verb::FillRegion { block, .. } => {
            check_block_field(
                blocks,
                block,
                format!("{base_path}/block"),
                "fill-region",
                "minecraft:stone",
                d,
            );
        }
        // NPC-lifecycle refs. `spawn-npc` (v0.6) joins the v0.4 `despawn-npc`/
        // `move-npc` family: an unknown npc id is the same `DW0112` dangling ref.
        Verb::DespawnNpc { npc, .. } | Verb::MoveNpc { npc, .. } | Verb::SpawnNpc { npc, .. }
            if !npc_ids.contains(npc.as_str()) =>
        {
            let verb = eff
                .v04_effect()
                .or_else(|| eff.v06_effect())
                .unwrap_or("lifecycle");
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

/// Push `DW0142` if `anchor` is not provided by the quest's (known single-prefab)
/// area, or `DW0871` if it is provided and is the wrong SHAPE.
///
/// `None` set = pool area or unknown prefab → deferred to the compiler.
///
/// Every objective that names an anchor names a cell — `reach-anchor` walks a
/// body to it, `interact` presses something at it, `collect` opens a container
/// standing on it — so the demand is [`crate::layout::StationKind::Point`] for
/// all three and is stated once here rather than at each caller.
fn anchor_resolves(
    providers: &AnchorProviders,
    set: Option<&BTreeSet<String>>,
    anchor: &crate::ids::AnchorId,
    qi: usize,
    oi: usize,
    field: &str,
    d: &mut Vec<Diagnostic>,
) {
    if let Some(f) = station_kind_diag(
        providers,
        anchor.as_str(),
        crate::layout::StationKind::Point,
        &format!("an objective's `{field}`"),
        "quests",
        format!("/content/quests/{qi}/objectives/{oi}/{field}"),
    ) {
        d.push(f);
        return;
    }
    if let Some(set) = set
        && !set.contains(anchor.as_str())
    {
        d.push(Diagnostic::error(
            codes::ANCHOR_UNRESOLVED,
            "quests",
            format!("/content/quests/{qi}/objectives/{oi}/{field}"),
            format!(
                "objective `{field}` anchor `{anchor}` is not provided by the prefab bound to \
                 this quest's area — {remedy}",
                remedy = providers.anchor_remedy(
                    "use an anchor the prefab exposes (anchor names come from prefab metadata; \
                     do NOT invent one)"
                ),
            ),
        ));
    }
}

/// `DW0150` for the state a campaign is in when the stage-4 plan is written and
/// stage 5 is not: **every** planned quest is unexpanded, because stage 5
/// declares no quests at all.
///
/// **The state this names is the one the authoring page ends its story-document
/// step in.** The plan is written before the quests that fill it — that is the
/// order the stages are numbered in — so between the two there is necessarily a
/// campaign that carries a plan and no expansion, and this code fires once for
/// every quest in it. Per-quest, with the two ordinary remedies attached, that
/// reads as one fault per planned quest with a repair the author is expected to
/// perform now, and both of the repairs it names are wrong here: writing the
/// expansions *is* the next authoring step and the plan is not a mistake to
/// delete.
///
/// **There is no third, cheaper remedy, and the message says so rather than
/// leaving the author to find out.** A stage-5 quest carrying only what the
/// schema requires — a trigger and two empty arrays — is refused again by
/// `DW0481`, which demands every quest say what it does to the story, and by
/// `DW0460` once for every NPC live in it: measured on a five-quest plan,
/// writing the five minimal expansions took the campaign from 5 errors to 15.
/// A diagnostic that offered "stub it" here would be sending the author to a
/// state three times worse than the one they are in.
///
/// So what changes is what the refusal SAYS, never whether it refuses. A plan
/// with no expansion emits nothing and cannot build, so the error and the exit
/// stand exactly as before; this is [`crate::diagnostic::codes::QUEST_NOT_EXPANDED`]
/// telling the truth about where the author is standing. It is grouped into one
/// diagnostic for the same reason `DW0874` names all five missing documents in
/// one run: N copies of a sentence about one state is a count the reader has to
/// discount, not information.
fn plan_awaiting_expansion(unexpanded: &[&PlannedQuest]) -> Diagnostic {
    let names = unexpanded
        .iter()
        .map(|q| format!("`{}`", q.id))
        .collect::<Vec<_>>()
        .join(", ");
    Diagnostic::error(
        codes::QUEST_NOT_EXPANDED,
        "quest-plan",
        "/content/quests",
        format!(
            "stage 5 declares no quests at all, so none of the {n} planned quest(s) has an \
             expansion: {names}. That is an authoring state, not a fault — the plan is written \
             before the quests that fill it, and a campaign is written a document at a time. The \
             refusal still stands, because a planned quest with no expansion has no trigger, no \
             objective and no completion, so nothing of it is emitted and no campaign in this \
             state can build. What there is NO route to is a cheaper way out: a stage-5 quest \
             carrying only what its schema requires — a trigger and two empty arrays — is refused \
             again by `DW0481`, because every quest must say what it does to the story, and by \
             `DW0460` once for every NPC live in it, so writing empty expansions raises the error \
             count instead of lowering it. Do not try to clear this code from the plan, and do \
             not re-run validation expecting it to move. The two things that clear it are \
             writing stage 5, which clears it for every quest at once, and dropping the quests \
             from the stage-4 plan. Until stage 5 is written this is the expected state of a \
             plan-only campaign and this code is what marks it.",
            n = unexpanded.len(),
        ),
    )
}

pub(crate) fn cross_stage(c: &Campaign, d: &mut Vec<Diagnostic>) {
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

    // `DW0150` has two readings and only one of them is a mistake. When stage 5
    // declares quests and this plan entry is not among them, the two remedies
    // below are both available and one of them is what the author meant. When
    // stage 5 declares NO quests at all, neither remedy is: the author is
    // between writing the plan and writing the quests, every planned quest is
    // unexpanded by construction, and the message has to say so — see
    // [`plan_awaiting_expansion`].
    let unexpanded: Vec<&PlannedQuest> = c
        .quest_plan
        .content
        .quests
        .iter()
        .filter(|q| !expanded_ids.contains(q.id.as_str()))
        .collect();
    if !unexpanded.is_empty() && c.quests.content.quests.is_empty() {
        d.push(plan_awaiting_expansion(&unexpanded));
    } else {
        for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
            if !expanded_ids.contains(q.id.as_str()) {
                d.push(Diagnostic::error(
                    codes::QUEST_NOT_EXPANDED,
                    "quest-plan",
                    format!("/content/quests/{i}"),
                    format!(
                        "planned quest `{}` has no stage-5 expansion — stage 5 declares {n} \
                         quest(s) and none of them carries this id, so this is a mismatch rather \
                         than an unwritten stage: add a stage-5 quest with id `{}` \
                         (objectives/effects), or drop it from the stage-4 plan",
                        q.id,
                        q.id,
                        n = c.quests.content.quests.len(),
                    ),
                ));
            }
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
}

/// `DW0357` at every quests-stage effect root: a `carrier: "one"` give where the
/// bundle has no acting player ([`check_carrier_one_not_scheduled`]).
pub(crate) fn carrier_one_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // `DW0357`, per root SITE: whether its bundle has an acting player is the
    // site's own answer (a `presser` trigger does, every other trigger does not).
    crate::effects::for_each_effect_root(c, &mut |site, effs| {
        if site.stage == "quests" {
            check_carrier_one_not_scheduled(
                effs,
                &site.path,
                site.runs_with_acting_player(),
                true,
                d,
            );
        }
    });
}

/// Every objective and effect anchor of a quest (`DW0142`/`DW0871`), resolved
/// against the quest's planned area — a cutscene's camera against every area —
/// at any nesting depth, through the single anchor authority
/// ([`QuestEffect::anchor_refs`]).
pub(crate) fn quest_anchor_checks(
    c: &Campaign,
    providers: &AnchorProviders,
    d: &mut Vec<Diagnostic>,
) {
    // planned quest id -> its area.
    let quest_area: BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();

    // Objective / effect anchors, resolved against the quest's planned area.
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        let set = quest_area
            .get(q.id.as_str())
            .and_then(|area| providers.for_area(area));
        let Some(set) = set else { continue };

        for (j, obj) in q.objectives.iter().enumerate() {
            if let crate::Objective::ReachAnchor { anchor, .. } = obj
                && let Some(f) = station_kind_diag(
                    providers,
                    anchor.as_str(),
                    crate::layout::StationKind::Point,
                    "a `reach-anchor` objective",
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/anchor"),
                )
            {
                d.push(f);
            } else if let crate::Objective::ReachAnchor { anchor, .. } = obj
                && !set.contains(anchor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/anchor"),
                    format!(
                        "objective anchor `{anchor}` is not provided by the prefab bound to \
                         this quest's area — {}",
                        providers.anchor_remedy(
                            "use an anchor the prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
        }
        // Effect anchors — **deep** and driven by the single anchor authority
        // ([`QuestEffect::anchor_refs`]), so every anchor-bearing effect at any
        // nesting depth resolves or is named. This scan used to be shallow
        // ([`for_each_effect`]) and enumerate variants by hand: a typo'd
        // `open-gate`/`set-block`/`set-checkpoint` anchor one level down (a
        // `sequence` step, an `on_respawn`/`on_caught`/`on_arrive` bundle)
        // validated clean and then emitted *nothing* — the silent-drop class of
        // bug the compiler's build-time seal (`DW0360`) now backstops.
        for_each_effect_deep(q, |path, eff| {
            // A `cutscene`'s camera anchors are legitimately cross-area (the
            // camera flies wherever the shot needs), so they resolve against the
            // union of every known area's anchors; every other effect anchor names
            // a world position the quest's own area must provide.
            let cross_area = matches!(&eff.verb, Verb::Cutscene { .. });
            for (suffix, anchor, demands) in eff.anchor_refs() {
                if let Some(f) = station_kind_diag(
                    providers,
                    anchor.as_str(),
                    demands,
                    &format!("`{}`", eff.verb.tag()),
                    "quests",
                    format!("/content/quests/{i}/{path}/{suffix}"),
                ) {
                    d.push(f);
                    continue;
                }
                let resolves = if cross_area {
                    // An unknown/pool area makes the union incomplete — defer to
                    // the compiler's build-time seal rather than guess.
                    !providers.all_areas_known() || providers.union().contains(anchor.as_str())
                } else {
                    set.contains(anchor.as_str())
                };
                if resolves {
                    continue;
                }
                let scope = if cross_area {
                    "any area's prefab"
                } else {
                    "the prefab bound to this quest's area"
                };
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/{suffix}"),
                    format!(
                        "`{verb}` anchor `{anchor}` is not provided by {scope} — {}",
                        providers.anchor_remedy(
                            "use an anchor a prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                        verb = eff.verb.tag(),
                    ),
                ));
            }
        });
    }
}

/// A quest's references (spec-0001 v0.3): `kill.wave` / `spawn-wave.wave` /
/// `collect.dropped_by` name a declared wave (`DW0170`) and a killed wave is
/// spawned (`DW0171`); `requires_flags` / `forbids_flags` on objectives and
/// effects name a produced flag (`DW0172`); item ids on `collect` /
/// `interact.requires_item` / `give-item` are in the registry (`DW0143`);
/// `collect` / `interact` anchors resolve against the quest's area (`DW0142`).
pub(crate) fn quest_reference_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;

    let declared_waves: BTreeSet<&str> = quests.waves.iter().map(|w| w.id.as_str()).collect();

    // Flags declared by `set-flag`, from the ONE producer inventory
    // ([`produced_flags`]); waves spawned by `spawn-wave`.
    let declared_flags: BTreeSet<String> = produced_flags(c);
    let mut spawned_waves: BTreeSet<&str> = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_path, _site, e| {
        if let Some(w) = e.spawn_wave() {
            spawned_waves.insert(w.as_str());
        }
    });

    // area id -> its single-prefab anchor set (pool areas deferred to compiler).
    let providers = AnchorProviders::build(c, anchors);
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
            .and_then(|area| providers.for_area(area));

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
                Objective::Collect {
                    id: oid,
                    item,
                    count,
                    anchor,
                    container,
                    dropped_by,
                    fill_count,
                    ..
                } => {
                    // v0.9: the wave a drop-gated collect names must
                    // exist — the same dangling-reference rule a `kill` follows.
                    if let Some(wave) = dropped_by
                        && !declared_waves.contains(wave.as_str())
                    {
                        d.push(Diagnostic::error(
                            codes::WAVE_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/dropped_by"),
                            format!(
                                "`collect` `dropped_by` references unknown wave `{wave}` — \
                                 declare it in the stage-5 `waves` section or correct the \
                                 reference"
                            ),
                        ));
                    }
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
                    // The objective's props are placed with a single-slot `item
                    // replace … container.0` fill, so an over-cap count leaves the
                    // chest empty and the objective uncompletable (`DW0436`).
                    check_stack_count(
                        item,
                        *count,
                        &format!("collect objective `{oid}`"),
                        format!("/content/quests/{i}/objectives/{j}/count"),
                        items,
                        d,
                    );
                    anchor_resolves(&providers, set, anchor, i, j, "anchor", d);
                    // v0.8: the adopted container's anchor must exist
                    // too. Whether its CELL really holds a chest/barrel needs the
                    // assembled world and is the build-tier `DW0438`.
                    if let Some(cont) = container {
                        anchor_resolves(&providers, set, cont, i, j, "container", d);
                    }
                    // v0.8: the fill is positional — the required stack
                    // in `container.0` plus one padding stack per slot after it —
                    // so it obeys the same 27-slot ceiling a `loot` declaration
                    // does, and for the same reason: every stack past the last slot
                    // is dropped without a word.
                    let slots = 1usize + *fill_count as usize;
                    if slots > MIN_CONTAINER_SLOTS {
                        d.push(Diagnostic::error(
                            codes::LOOT_TOO_MANY_ITEMS,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/fill_count"),
                            format!(
                                "collect objective `{oid}` fills {slots} slots (its own stack \
                                 plus `fill_count` {fill_count}), more than the \
                                 {MIN_CONTAINER_SLOTS} slots a vanilla chest or barrel has. \
                                 Slots are assigned positionally, so every stack past the \
                                 {MIN_CONTAINER_SLOTS}th would be dropped silently. Lower \
                                 `fill_count` to at most {max} — a container that reads full \
                                 does not need to overflow.",
                                max = MIN_CONTAINER_SLOTS - 1
                            ),
                        ));
                    }
                }
                Objective::Interact {
                    anchor,
                    requires_item,
                    missing_item_hint,
                    ..
                } => {
                    // v0.7: the empty-hand narration answers the held-item gate;
                    // without a gate there is no missing hand to narrate to, and
                    // the authored line would be dead content that never fires.
                    if missing_item_hint.is_some() && requires_item.is_none() {
                        d.push(Diagnostic::error(
                            codes::MISSING_ITEM_HINT_WITHOUT_ITEM,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/missing_item_hint"),
                            "`interact.missing_item_hint` narrates the click that arrives \
                             without the required item in hand, but this objective declares no \
                             `requires_item` — add the `requires_item` this hint is about, or \
                             drop the hint"
                                .to_string(),
                        ));
                    }
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
                    anchor_resolves(&providers, set, anchor, i, j, "anchor", d);
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
            // v0.6: `forbids_flags` gets the same unknown-flag treatment as
            // `requires_flags` — a never-produced flag can never suppress anything,
            // so the reference is dead (a typo until proven otherwise).
            for (m, f) in obj.forbids_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/forbids_flags/{m}"),
                        format!(
                            "objective `forbids_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — the gate can never suppress anything; add the \
                             producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag \
                             name"
                        ),
                    ));
                }
            }
        }

        for_each_effect_deep(q, |path, eff| {
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
                        format!("/content/quests/{i}/{path}/when/requires_flags/{n}"),
                        format!(
                            "effect `requires_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                             earlier, or correct the flag name"
                        ),
                    ));
                }
            }
            // v0.6: per-effect `forbids_flags` — same unknown-flag treatment.
            for (n, f) in eff.forbids_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/{path}/when/forbids_flags/{n}"),
                        format!(
                            "effect `forbids_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — the gate can never suppress anything; add the \
                             producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag \
                             name"
                        ),
                    ));
                }
            }
        });
    }
}

/// An `interact` objective's `prop` (`DW0193`, `DW0957`) and every quest effect's
/// block, npc and cutscene references ([`check_effect_v04`]), at any nesting
/// depth.
pub(crate) fn quest_prop_checks(c: &Campaign, blocks: &dyn BlockRegistry, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let declared_waves: BTreeSet<&str> = quests.waves.iter().map(|w| w.id.as_str()).collect();

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
                if crate::fires_on_step(&prop.block) {
                    d.push(Diagnostic::error(
                        codes::INTERACT_PROP_STEPPED,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/prop/block"),
                        format!(
                            "`interact` objective `{}` uses `{}` as its prop, a block a player \
                             fires by stepping on it — but an `interact` completes on a \
                             right-click, so walking onto it does nothing. Prescription: give the \
                             objective a block a hand works (a lever, a button), or make the step \
                             the act: a `trigger` with `on: step` at an anchor whose cell holds the \
                             plate, whose effects do what completing the objective did",
                            o.id(),
                            prop.block
                        ),
                    ));
                }
            }
        }
        for_each_effect_deep(q, |path, eff| {
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
}
