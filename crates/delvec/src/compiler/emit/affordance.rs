//! The compiler-owned interact affordances: the subjects of the `DW0420` / `DW0421` proofs.

use super::*;

/// Every compiler-owned interact affordance in this campaign, in deterministic
/// order — the subjects of the `DW0420` / `DW0421` proofs.
///
/// The list is the definition of the class: a point the delve asks the player to
/// right-click. Adding a new such verb means adding it here, which is what makes
/// the proof total rather than a spot check.
pub(super) fn affordances(plan: &Plan) -> Vec<crate::compiler::affordance::Affordance> {
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        out.push(crate::compiler::affordance::Affordance {
            id: sc.id.clone(),
            kind: "shortcut unlock",
            tag: format!("dw_sc_{}", sc.safe),
            // Opening the door spends the affordance.
            retired_by: Some(format!("shortcut_open_{}", sc.safe)),
        });
    }
    for t in &plan.traps {
        if t.disarm.is_some() {
            out.push(crate::compiler::affordance::Affordance {
                id: t.id.clone(),
                kind: "trap disarm",
                tag: format!("dw_trapdis_{}", t.safe),
                // Throwing the lever spends the affordance.
                retired_by: Some(format!("trap_disarm_{}", t.safe)),
            });
        }
    }
    for g in &plan.timed_gates {
        if g.disarm.is_some() {
            out.push(crate::compiler::affordance::Affordance {
                id: g.id.clone(),
                kind: "timed-gate disarm",
                tag: format!("dw_tgdis_{}", g.safe),
                // Jamming the gate spends the affordance.
                retired_by: Some(format!("tgate_disarm_{}", g.safe)),
            });
        }
    }
    for bf in plan.bonfires() {
        out.push(crate::compiler::affordance::Affordance {
            id: bf.anchor.clone(),
            kind: "bonfire",
            tag: format!("dw_bonfire_{}", bf.index),
            // A bonfire is rested at, never used up.
            retired_by: None,
        });
    }
    // spec-0032. A shop is furniture — traded with, never used up. A stake IS
    // consumed, and `stk_gc_<s>` is the single function permitted to retire it:
    // every other path (a collection, an eviction under the `replace` policy)
    // clears the per-player ledger and lets the reference count decide, which is
    // what keeps one killer for one piece of hardware (`DW0421`).
    for (i, sh, _) in shops(plan) {
        out.push(crate::compiler::affordance::Affordance {
            id: sh.id.as_str().to_string(),
            kind: "shop",
            tag: format!("dw_shop_{i}"),
            retired_by: None,
        });
    }
    // One entry, because there is one piece of hardware: the marker is a PLACE and
    // every stake a death forfeits leaves its wager at the same one. Registering it
    // per stake would name four owners for one entity and four retirers for one
    // `kill`, which is precisely the bookkeeping `DW0421` exists to refuse.
    let marking: Vec<String> = stakes(plan)
        .into_iter()
        .filter(|(st, _)| st.max_live() > 0)
        .map(|(st, _)| st.id.as_str().to_string())
        .collect();
    if !marking.is_empty() {
        out.push(crate::compiler::affordance::Affordance {
            id: marking.join(", "),
            kind: "recovery stake marker",
            tag: stk_tag(),
            retired_by: Some(STK_GC_FN.to_string()),
        });
    }
    out
}

/// The visible hardware for a compiler-owned interact affordance: a glowing,
/// collision-free `item_display` at the affordance's own cell, carrying the
/// derived `dw_hw_<tag>` so [`crate::compiler::affordance`] can pair the two.
///
/// An `item_display` (not a block) because the affordance's cell must stay
/// walkable and the interaction hitbox unobstructed — the same reasoning that
/// made the interact objective's marker a display. It is deliberately
/// **nameless**: the glow says "use me" without inventing a player-facing
/// string that no campaign authored and no `l10n` sidecar could translate.
pub(super) fn affordance_hardware(pos: [String; 3], tag: &str, item: &str) -> String {
    format!(
        "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{}\"],billboard:\"center\",item:{{id:\"{item}\",count:1}}}}",
        pos[0],
        pos[1],
        pos[2],
        crate::compiler::affordance::hardware_tag(tag)
    )
}
