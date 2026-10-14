//! **The teaching line** (spec-0104 §5.4): for each input class the campaign
//! gives a line, the line is shown once per player, in chat, the first time an
//! act of that class is the next thing that player can do.
//!
//! The sites are [`crate::compiler::acts::teach_sites`]'s: per walk, the first
//! act of each class. For an objective the line is shown from its activation
//! function; for a `use`/`strike` trigger, when a player first stands within
//! [`crate::compiler::crosshair::INTERACTION_REACH`] of its anchor while the
//! trigger is armed — the per-tick position test an `approach` trigger uses.
//! Either way it runs `teach_<class>` as each player not yet taught, which
//! shows the line to that player, marks them taught on `dw.taught_<class>`,
//! and counts the line on the party holder `#taught_<class>` — the count a
//! PackTest reads to assert one line was shown.
//!
//! A campaign with no `teaching` line emits nothing here, byte for byte.

use super::*;

/// The per-player score that records a class taught.
pub(super) fn taught_score(class: delvewright_dsl::InputClass) -> String {
    format!("dw.taught_{}", class.as_str())
}

/// The function that shows a class's line to `@s`.
pub(super) fn teach_fn(class: delvewright_dsl::InputClass) -> String {
    format!("teach_{}", class.as_str().replace('-', "_"))
}

/// The function a trigger's firing cells run its teaching from.
pub(super) fn teach_at_fn(trigger_id: &str) -> String {
    format!("teach_at_{}", plan::safe_local(trigger_id))
}

/// The classes the campaign shows a line for at some site, in class order.
pub(super) fn taught_classes(plan: &Plan) -> Vec<delvewright_dsl::InputClass> {
    let sites = crate::compiler::acts::teach_sites(plan);
    let mut out: Vec<delvewright_dsl::InputClass> = sites
        .objectives
        .values()
        .chain(sites.triggers.values().map(|(c, _)| c))
        .flatten()
        .copied()
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The `setup` lines: one score per class taught.
pub(super) fn teaching_setup(plan: &Plan) -> Vec<String> {
    taught_classes(plan)
        .into_iter()
        .map(|cl| format!("scoreboard objectives add {} dummy", taught_score(cl)))
        .collect()
}

/// The lines an objective's activation runs to teach its classes.
pub(super) fn teaching_activation(plan: &Plan, objective_id: &str) -> Vec<String> {
    let ns = &plan.namespace;
    let sites = crate::compiler::acts::teach_sites(plan);
    sites
        .objectives
        .get(objective_id)
        .map(|classes| {
            classes
                .iter()
                .map(|cl| {
                    format!(
                        "execute as @a unless score @s {} matches 1 run function {ns}:{}",
                        taught_score(*cl),
                        teach_fn(*cl)
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The `tick` lines: each teaching trigger's armed poll.
pub(super) fn teaching_tick(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<String> {
    let ns = &plan.namespace;
    let sites = crate::compiler::acts::teach_sites(plan);
    let mut out = Vec::new();
    for t in &plan.emitted_triggers(chrome) {
        if !sites.triggers.contains_key(t.id.as_str()) {
            continue;
        }
        let (once_guard, forbid_guard, flag_guard) = trigger::trigger_poll_guards(plan, t);
        out.push(format!(
            "execute {once_guard}{forbid_guard}{flag_guard}run function {ns}:{}",
            teach_at_fn(t.id.as_str())
        ));
    }
    out
}

/// The teaching functions: `teach_<class>` per class taught, and
/// `teach_at_<trigger>` per teaching trigger.
pub(super) fn teaching_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let teaching = &plan.campaign.quests.content.guidance.teaching;
    let sites = crate::compiler::acts::teach_sites(plan);
    let mut out = Vec::new();
    for cl in taught_classes(plan) {
        let Some(line) = teaching.get(cl).and_then(|t| t.line()) else {
            continue;
        };
        out.push((
            teach_fn(cl),
            lines(&[
                format!("tellraw @s {}", tr(line)),
                format!("scoreboard players set @s {} 1", taught_score(cl)),
                format!("scoreboard players add #taught_{} dw.sys 1", cl.as_str()),
            ]),
        ));
    }
    let reach = crate::compiler::crosshair::INTERACTION_REACH;
    for (tid, (classes, p)) in &sites.triggers {
        let body: Vec<String> = classes
            .iter()
            .map(|cl| {
                format!(
                    "execute positioned {}.5 {} {}.5 as @a[distance=..{reach}{}] unless score @s \
                     {} matches 1 run function {ns}:{}",
                    p[0],
                    p[1],
                    p[2],
                    respawn::observer_guard(plan),
                    taught_score(*cl),
                    teach_fn(*cl)
                )
            })
            .collect();
        out.push((teach_at_fn(tid), lines(&body)));
    }
    out
}
