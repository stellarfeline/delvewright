//! Teleports.

use super::*;

/// The generated function name for a `teleport` effect (DSL v0.10, spec-0031).
///
/// A named function rather than an inline line, for the same reason `volley` and
/// `collapse` have one: **the body is compiler-PROVEN geometry** — a box resolved
/// through `Plan::zone_box` and a destination resolved to a literal cell — and a
/// body that only ever exists spliced into a `seq_<root>_<n>` beside four other
/// effects is a body no runtime test can call. The generated PackTest calls
/// exactly this function, so the runtime proof of totality binds to the emission
/// rather than to a command the test re-typed for itself.
pub(super) fn teleport_fn(eff: &QuestEffect) -> String {
    format!("teleport_{}", payload_verb_key(eff))
}

/// The one emitted line of a `teleport`: move everything in the volume.
///
/// `None` when either anchor is unresolved — `check_effect_anchors` (`DW0360`)
/// owns that failure, and an invalid selector emitted here would report it as
/// something else.
pub(super) fn teleport_command(plan: &Plan, eff: &QuestEffect) -> Option<String> {
    let (from, to) = eff.teleport()?;
    let (lo, hi) = plan.zone_box(from)?;
    let d = ent_xyz(to.cell(anchor_point_any(plan, to.anchor.as_str())?));
    Some(format!(
        "tp @e[{}] {} {} {}",
        entity_box_selector(lo, hi),
        d[0],
        d[1],
        d[2]
    ))
}

/// One function per distinct `teleport` (deduped by content key). Empty for a
/// campaign that declares none, so pre-0.10 output is byte-identical.
pub(super) fn teleport_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        if eff.teleport().is_none() {
            continue;
        }
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(cmd) = teleport_command(plan, eff) else {
            continue;
        };
        out.push((name, lines(&[cmd])));
    }
    out
}
