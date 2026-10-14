//! `close-gate` gate-block validation (DSL v0.6): the physical dual of `open-gate`
//! fills a gate anchor's region with the block the anchor declares (a basalt
//! boulder sealing a cave mouth, iron bars dropping across a doorway). A
//! `close-gate` on an anchor that declares **no** fill `block` — or that is not a
//! gate region at all — cannot be sealed, so it is rejected at validate-time
//! (`DW0343`). This lives in the compiler (not `dsl::validate`) because the fill
//! `block` is prefab metadata, which the DSL's anchor-registry surface does not
//! carry; it runs in the compiler's validate stage alongside the atmos sound/art
//! checks, so `validate`/`analyze`/`build` all catch it (validation tier, exit 1).

use delvewright_dsl::{Campaign, Diagnostic};

use crate::compiler::registry::PrefabRegistry;
use delvewright_dsl::{DwCode, ExitTier};
use std::collections::{BTreeMap, BTreeSet};

delvewright_dsl::dw_code! {
    /// `DW0343`: a `close-gate` targets a gate anchor that declares no fill `block` in
    /// its prefab metadata (or is not a gate region), so the compiler cannot seal it.
    pub const DW_GATE_NO_BLOCK: DwCode = DwCode::new("DW0343", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0423`: two `close-gate` effects seal the **same** gate anchor with
    /// different `sealed_hint` wordings.
    ///
    /// The seal's answer belongs to the place, not to the firing: one anchor gets one
    /// set of `dw_seal_<anchor>` hitboxes and one reward function, so a second
    /// wording has nowhere to live and would be silently dropped. Rejected instead —
    /// a line an author wrote and a player can never read is the same silence class
    /// as the finding this verb exists to close.
    pub const DW_SEAL_HINT_CONFLICT: DwCode = DwCode::new("DW0423", ExitTier::Build);
}

/// Which of this campaign's bound pieces provide `anchor` as a gate the
/// compiler can fill, keyed by the site area, paired with the refusals the
/// authority raised while asking.
///
/// The denominator is **this campaign's own pieces** — the `details[]` rows —
/// and never the loaded library. A piece no place binds cannot be placed, so it
/// has no standing to answer a question about what this campaign's gates are
/// made of. Asking the library is what let two pieces the campaign did not
/// place satisfy `DW0343` on a shortcut they had nothing to do with.
fn gate_providers(
    c: &Campaign,
    prefabs: &PrefabRegistry,
    anchor: &str,
) -> (BTreeMap<String, BTreeSet<String>>, Vec<String>) {
    let mut areas: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut refusals = Vec::new();
    if c.site_plan.is_some() {
        let area = delvewright_dsl::SITE_AREA;
        for piece in prefabs.campaign_pieces(c) {
            match prefabs.gate_anchor_in(&piece, anchor) {
                Ok(Some(_)) => {
                    areas
                        .entry(area.to_string())
                        .or_default()
                        .insert(piece.clone());
                }
                Ok(None) => {}
                Err(why) => refusals.push(format!("`{piece}`: {why}")),
            }
        }
    }
    (areas, refusals)
}

/// Validate every verb that needs a gate anchor's **fill block** references an
/// anchor that declares one (`DW0343`): `close-gate` (which fills the region back
/// in) and a stage-5 `shortcut` (spec-0016 §2, whose unlock clears the region
/// `replace <block>` and whose gate is sealed from world-load by that very
/// block). Descends every nested effect list (`sequence` steps / lifecycle
/// bundles) so a `close-gate` buried in a timeline is checked too, and a
/// `timed-gate` (spec-0016 §4), whose clock fills and clears the region with that
/// block twice a cycle.
///
/// A gate station a `details[]` row binds is the bound piece's gate, so its
/// block is the piece's: a piece anchor that declares a region and no `block`
/// is refused here, rather than filled with the derivation's bar.
pub fn check_close_gates(c: &Campaign, prefabs: &PrefabRegistry) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let diag = |anchor: &str, path: String| -> Vec<Diagnostic> {
        if let Some((piece, bound)) = bound_piece_anchor(c, anchor) {
            let declared = prefabs
                .get(piece)
                .and_then(|m| m.anchors.get(bound))
                .is_some_and(|a| a.region.is_none() || a.block.is_some());
            if declared {
                return Vec::new();
            }
            return vec![Diagnostic::error(
                DW_GATE_NO_BLOCK,
                "quests",
                path,
                format!(
                    "gate anchor `{anchor}` is bound to `{bound}` of `{piece}`, which declares a \
                     gate region and no fill `block`. `close-gate` fills the region with the \
                     anchor's declared block (the dual of `open-gate`), and a `shortcut` clears \
                     exactly that block on unlock. Prescription: declare the `block` on \
                     `{bound}` in `{piece}`'s metadata — or remove the verb."
                ),
            )];
        }
        // A DERIVED seam gate declares its block in the derivation, not in prefab
        // metadata — a site-plan campaign has no prefab to ask. Asking only the
        // prefab registry made this check measure a smaller world than the
        // campaign has: it refused a `shortcut` naming the very
        // `anchor/seam-<edge>` the derivation seals, and nothing was red, because
        // a check resolving against a truncated input refuses CONTENT. The
        // question is "can the compiler fill and clear this", and here it
        // demonstrably can — the block is the one the derivation writes.
        if delvewright_dsl::synthesized_gate_block(c, anchor).is_some() {
            return Vec::new();
        }
        // And a derivation with no `layout-graph.json` to read names no gate at
        // all, so the question above cannot be answered rather than answered no.
        // The same silence the DSL tier keeps over every other anchor reference
        // in that state, kept here for the same reason and from the same
        // authority: `DW0824` is the finding, and this gate is re-judged the
        // moment the graph exists.
        if delvewright_dsl::anchor_vocabulary_unknowable(c) {
            return Vec::new();
        }
        let (areas, refusals) = gate_providers(c, prefabs, anchor);
        if areas.len() == 1 && refusals.is_empty() {
            return Vec::new();
        }
        let why = if refusals.is_empty() {
            String::new()
        } else {
            format!(" — {}", refusals.join("; "))
        };
        vec![Diagnostic::error(
            DW_GATE_NO_BLOCK,
            "quests",
            path,
            format!(
                "gate anchor `{anchor}` declares no fill `block` in the prefab metadata of any \
                 piece this campaign's areas can place (or is not a gate region){why}. \
                 `close-gate` fills the region with the anchor's declared block (the dual of \
                 `open-gate`), and a `shortcut` clears exactly that block on unlock. \
                 Prescription: {} — or remove the verb.",
                delvewright_dsl::anchor_remedy(c)
            ),
        )]
    };
    // Every root, every depth. Its own file's `check_seal_hints` (`DW0423`, twenty
    // lines below) already carried the corrected reasoning; this half was never
    // back-ported and still hand-listed three of the five roots. The walk also
    // reports the *nested* effect's own pointer now rather than its top-level
    // ancestor's, because `for_each_campaign_effect` threads the path down —
    // a `close-gate` inside a `sequence` step used to be blamed on the sequence.
    delvewright_dsl::for_each_campaign_effect(c, &mut |path, _site, e| {
        if let Some(a) = e.close_gate_anchor() {
            d.extend(diag(a.as_str(), format!("{path}/anchor")));
        }
    });
    // spec-0016 §2: a shortcut's gate is cleared `replace <block>` on unlock and is
    // sealed by that same block at world-load, so it carries the identical
    // fill-block obligation as `close-gate`.
    for (si, sc) in c.quests.content.shortcuts.iter().enumerate() {
        d.extend(diag(
            sc.gate.as_str(),
            format!("/content/shortcuts/{si}/gate"),
        ));
    }
    // spec-0016 §4: a timed gate's clock fills and clears the region with exactly
    // that block, twice a cycle — the same obligation, third verb.
    for (gi, g) in c.quests.content.timed_gates.iter().enumerate() {
        d.extend(diag(
            g.gate.as_str(),
            format!("/content/timed_gates/{gi}/gate"),
        ));
    }
    d
}

/// The `(piece, piece anchor)` a `details[]` row binds the campaign anchor
/// `anchor` to, if any row does.
fn bound_piece_anchor<'c>(c: &'c Campaign, anchor: &str) -> Option<(&'c str, &'c str)> {
    c.detail_plan
        .as_ref()?
        .content
        .details
        .iter()
        .find_map(|row| {
            row.anchors
                .get(anchor)
                .map(|bound| (row.piece.as_str(), bound.as_str()))
        })
}

/// Prove every gate anchor has ONE answer (`DW0423`, DSL v0.8).
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
///
/// Walks [`crate::compiler::plan::for_each_gate_effect`] — the **same** traversal the seal
/// planner uses, so the check and the emission can never disagree about which
/// firings exist. `dsl::for_each_campaign_effect` is deliberately not used: it
/// stops at the quests stage and would miss a `close-gate` inside a dialogue
/// option's `set-checkpoint` `on_respawn` bundle, which really does emit a fill.
pub fn check_seal_hints(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    // anchor → (first authored wording, its path)
    let mut seen: std::collections::BTreeMap<String, (String, String)> = Default::default();
    crate::compiler::plan::for_each_gate_effect(c, &mut |site, e| {
        let (Some(anchor), Some(hint)) = (e.close_gate_anchor(), e.close_gate_sealed_hint()) else {
            return;
        };
        let path = &site.path;
        let key = anchor.as_str().to_string();
        match seen.get(&key) {
            None => {
                seen.insert(key, (hint.to_string(), format!("{path}/sealed_hint")));
            }
            Some((first, first_path)) if first != hint => {
                d.push(Diagnostic::error(
                    DW_SEAL_HINT_CONFLICT,
                    site.stage,
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
