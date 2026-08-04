//! `close-gate` gate-block validation (DSL v0.6): the physical dual of `open-gate`
//! fills a gate anchor's region with the block the anchor declares (a basalt
//! boulder sealing a cave mouth, iron bars dropping across a doorway). A
//! `close-gate` on an anchor that declares **no** fill `block` — or that is not a
//! gate region at all — cannot be sealed, so it is rejected at validate-time
//! (`DW0343`). This lives in the compiler (not `dsl::validate`) because the fill
//! `block` is prefab metadata, which the DSL's anchor-registry surface does not
//! carry; it runs in the compiler's validate stage alongside the atmos sound/art
//! checks, so `validate`/`analyze`/`build` all catch it (validation tier, exit 1).

use delvewright_dsl::{Campaign, Diagnostic, QuestEffect};

use crate::registry::PrefabRegistry;

/// `DW0343`: a `close-gate` targets a gate anchor that declares no fill `block` in
/// its prefab metadata (or is not a gate region), so the compiler cannot seal it.
pub const DW_GATE_NO_BLOCK: &str = "DW0343";

/// `DW0423`: two `close-gate` effects seal the **same** gate anchor with
/// different `sealed_hint` wordings.
///
/// The seal's answer belongs to the place, not to the firing: one anchor gets one
/// set of `dw_seal_<anchor>` hitboxes and one reward function, so a second
/// wording has nowhere to live and would be silently dropped. Rejected instead —
/// a line an author wrote and a player can never read is the same silence class
/// as the finding this verb exists to close.
pub const DW_SEAL_HINT_CONFLICT: &str = "DW0423";

/// Validate every verb that needs a gate anchor's **fill block** references an
/// anchor that declares one (`DW0343`): `close-gate` (which fills the region back
/// in) and a stage-5 `shortcut` (spec-0016 §2, whose unlock clears the region
/// `replace <block>` and whose gate is sealed from world-load by that very
/// block). Descends every nested effect list (`sequence` steps / lifecycle
/// bundles) so a `close-gate` buried in a timeline is checked too, and a
/// `timed-gate` (spec-0016 §4), whose clock fills and clears the region with that
/// block twice a cycle.
pub fn check_close_gates(c: &Campaign, prefabs: &PrefabRegistry) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let diag = |anchor: &str, path: String| -> Option<Diagnostic> {
        // `Some(true)` = every prefab providing this gate anchor declares a block;
        // `Some(false)` = a region-provider omits `block`; `None` = not a gate.
        if prefabs.gate_anchor_block(anchor) == Some(true) {
            return None;
        }
        Some(Diagnostic::error(
            DW_GATE_NO_BLOCK,
            "quests",
            path,
            format!(
                "gate anchor `{anchor}` declares no fill `block` in its prefab metadata (or is \
                 not a gate region), so the compiler cannot fill or clear it — `close-gate` fills \
                 the region with the anchor's declared block (the dual of `open-gate`), and a \
                 `shortcut` clears exactly that block on unlock. Declare a `block` on the gate \
                 anchor in the prefab metadata, or remove the verb."
            ),
        ))
    };
    let scan = |eff: &QuestEffect, base: &str, d: &mut Vec<Diagnostic>| {
        eff.visit_deep(&mut |e| {
            if let Some(a) = e.close_gate_anchor()
                && let Some(diagnostic) = diag(a.as_str(), format!("{base}/anchor"))
            {
                d.push(diagnostic);
            }
        });
    };
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oid, effs) in &q.on_objective_complete {
            for (i, eff) in effs.iter().enumerate() {
                let base = format!(
                    "/content/quests/{qi}/on_objective_complete/{}/{i}",
                    oid.as_str()
                );
                scan(eff, &base, &mut d);
            }
        }
        for (i, eff) in q.on_complete.iter().enumerate() {
            let base = format!("/content/quests/{qi}/on_complete/{i}");
            scan(eff, &base, &mut d);
        }
    }
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        for (i, eff) in t.effects.iter().enumerate() {
            let base = format!("/content/triggers/{ti}/effects/{i}");
            scan(eff, &base, &mut d);
        }
    }
    // spec-0016 §2: a shortcut's gate is cleared `replace <block>` on unlock and is
    // sealed by that same block at world-load, so it carries the identical
    // fill-block obligation as `close-gate`.
    for (si, sc) in c.quests.content.shortcuts.iter().enumerate() {
        if let Some(diagnostic) = diag(sc.gate.as_str(), format!("/content/shortcuts/{si}/gate")) {
            d.push(diagnostic);
        }
    }
    // spec-0016 §4: a timed gate's clock fills and clears the region with exactly
    // that block, twice a cycle — the same obligation, third verb.
    for (gi, g) in c.quests.content.timed_gates.iter().enumerate() {
        if let Some(diagnostic) = diag(g.gate.as_str(), format!("/content/timed_gates/{gi}/gate")) {
            d.push(diagnostic);
        }
    }
    d
}

/// Prove every gate anchor has ONE answer (`DW0423`, DSL v0.8 / task #142).
///
/// A `close-gate`'s `sealed_hint` is the line the sealed region answers a
/// right-click with. The hitboxes that carry it are named after the **anchor**,
/// so all the `close-gate`s that seal one anchor share them — and therefore share
/// one wording. Two firings that disagree are rejected here rather than resolved
/// by declaration order.
///
/// A firing that authors no hint is compatible with anything: it asks for the
/// compiler's canonical English, which any authored wording refines. Only two
/// *authored* and *different* lines conflict.
pub fn check_seal_hints(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    // anchor → (first authored wording, its path)
    let mut seen: std::collections::BTreeMap<String, (String, String)> = Default::default();
    delvewright_dsl::for_each_campaign_effect(c, &mut |path, _site, e| {
        let (Some(anchor), Some(hint)) = (e.close_gate_anchor(), e.close_gate_sealed_hint()) else {
            return;
        };
        let key = anchor.as_str().to_string();
        match seen.get(&key) {
            None => {
                seen.insert(key, (hint.to_string(), format!("{path}/sealed_hint")));
            }
            Some((first, first_path)) if first != hint => {
                d.push(Diagnostic::error(
                    DW_SEAL_HINT_CONFLICT,
                    "quests",
                    format!("{path}/sealed_hint"),
                    format!(
                        "gate anchor `{}` is sealed with two different `sealed_hint` lines — \
                         `{first}` at `{first_path}`, and `{hint}` here. A seal's answer belongs \
                         to the PLACE: one anchor carries one set of `dw_seal_…` hitboxes and one \
                         reward function, so the second wording would never reach a player. Give \
                         both firings the same line, or seal two different gate anchors.",
                        anchor.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    });
    d
}
