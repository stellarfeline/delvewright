//! The seal answers.

use super::*;

/// The seal plan for a gate anchor, if the campaign ever seals it.
pub(super) fn seal_hint_for<'a>(plan: &'a Plan, anchor: &str) -> Option<&'a plan::SealHintPlan> {
    plan.seal_hints.iter().find(|s| s.anchor == anchor)
}

/// The `seal_arm_<safe>` function name: what a `close-gate` calls to give the
/// stone a voice.
pub(super) fn seal_arm_fn(safe: &str) -> String {
    format!("seal_arm_{safe}")
}

/// The `dw_trig_<id>` tags every click trigger anchored **on this gate** rides,
/// in campaign declaration order (deterministic).
///
/// The round-6 rule, one layer out: one cell, one hitbox. A `strike`/`use`
/// trigger whose `at` is the gate anchor is asking the player to hit *the gate* —
/// and once the gate is sealed the gate's own hitboxes are what a click reaches.
/// Summoning the trigger a second, co-located entity is the exact ray-pick tie
/// that made the island's boulder unshippable, so the trigger's tag rides these
/// entities and [`env_trigger_setup`] summons nothing for it. The consequence is
/// also its meaning: such a trigger is live exactly while the gate is sealed.
pub(super) fn seal_rider_tags(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    anchor: &str,
) -> Vec<String> {
    plan.emitted_triggers(chrome)
        .iter()
        .filter(|t| t.on.is_click())
        .filter(|t| t.at_anchor() == Some(anchor))
        .map(|t| format!("dw_trig_{}", plan::safe_local(t.id.as_str())))
        .collect()
}

/// The `seal_arm_<safe>` functions: one `minecraft:interaction` per
/// clickable cell of each sealed region, so the wall answers a press wherever the
/// party presses it.
///
/// Only the region's **shell** is armed ([`plan::SealHintPlan::shell_cells`]) —
/// a cell buried inside the seal has no face a crosshair can reach. Each entity
/// is one block plus [`crate::compiler::clickbox::CLICK_MARGIN`] on every face, built by
/// [`crate::compiler::clickbox::ClickBox`] so its box brackets its cell on every axis.
///
/// Empty for a campaign that never seals a gate → byte-identical output.
pub(super) fn seal_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for s in &plan.seal_hints {
        let mut tags = vec![format!("dw_seal_{}", s.safe)];
        tags.extend(seal_rider_tags(plan, chrome, &s.anchor));
        let tag_list = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",");
        let body: Vec<String> = s
            .shell_cells()
            .into_iter()
            .map(|c| {
                crate::compiler::clickbox::ClickBox::column(c, 1).summon(&format!(
                    "response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}{tag_list}]"
                ))
            })
            .collect();
        out.push((seal_arm_fn(&s.safe), lines(&body)));
    }
    out
}
