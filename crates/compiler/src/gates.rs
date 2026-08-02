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

/// Validate every `close-gate` effect (DSL v0.6) references a gate anchor that
/// declares a fill `block` (`DW0343`). Descends every nested effect list
/// (`sequence` steps / lifecycle bundles) so a `close-gate` buried in a timeline is
/// checked too.
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
                "`close-gate` anchor `{anchor}` declares no fill `block` in its prefab metadata \
                 (or is not a gate region), so the compiler cannot seal it — `close-gate` fills \
                 the gate region with the anchor's declared block (the dual of `open-gate`). \
                 Declare a `block` on the gate anchor in the prefab metadata, or remove the \
                 `close-gate`."
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
    d
}
