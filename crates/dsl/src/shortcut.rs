//! Shortcuts: a route that unlocks from one side and stays open.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, QuestEffect, ShortcutId};

/// A stage-5 **shortcut door** (spec-0016 §2) — the souls loop-back.
///
/// The owner's definition of the pattern: between two rest points there are two
/// routes. The **short** one starts sealed and holds nothing; the **long** one is
/// full of enemies and mechanisms. You earn the far side the hard way, pull one
/// mechanism, and the short route opens **forever**. That moment is the design.
///
/// The compiler owns three obligations, none of them optional:
/// 1. the `unlock` affordance is reachable while the gate is still sealed — the
///    long route genuinely exists (`DW0373`);
/// 2. opening the gate genuinely shortens the trip across it — a shortcut that
///    pays nothing is a leak, not a shortcut (`DW0360`);
/// 3. permanence is **structural**: no `close-gate` may target a shortcut gate
///    (`DW0372`). There is no re-sealing verb to reach for.
///
/// `close-gate` on a NON-shortcut gate (the point-of-no-return staging beat) is
/// untouched by this — the two verbs are deliberately disjoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Shortcut {
    /// Unique shortcut id (`shortcut/<kebab>`).
    pub id: ShortcutId,
    /// The gate anchor this shortcut opens. Sealed from world-load (the prefab
    /// carries the physical fill), and its metadata must declare the fill `block`
    /// the compiler clears — the same requirement `close-gate` has.
    pub gate: AnchorId,
    /// The FAR-side anchor whose interaction fires the permanent open. The
    /// compiler summons the affordance there and polls it, reusing the v0.4
    /// interaction-entity `use` primitive.
    pub unlock: AnchorId,
    /// Effects fired once, when the shortcut opens — the bar lifting, the
    /// elevator descending, the sound of a door you will never have to earn
    /// again. Emitted server-source-safe (the poll lives on the tick).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_unlock: Vec<QuestEffect>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{
    AnchorProviders, for_each_effect_deep, for_each_trigger_effect_deep, station_kind_diag,
};

crate::dw_code! {
    /// (spec-0016 §2) A `shortcut` declaration is structurally invalid: a
    /// malformed or duplicate `shortcut/<id>`, a `gate`/`unlock` anchor no area's
    /// prefab provides, or a `gate` that IS the `unlock` (the mechanism must sit
    /// on the far side, not in the doorway).
    pub const SHORTCUT_INVALID: DwCode = DwCode::new("DW0371", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §2) A `close-gate` effect targets a gate a `shortcut` owns.
    /// A shortcut opens **permanently** — that is the whole pattern — so its
    /// permanence is structural: there is no verb that can put it back. Use a
    /// different gate for the point-of-no-return beat.
    pub const SHORTCUT_RESEALED: DwCode = DwCode::new("DW0372", ExitTier::Build);
}

/// Validate the stage-5 `shortcuts` section (spec-0016 §2).
///
/// Two rules, two codes:
/// * `DW0371` — the declaration must resolve: a well-formed, unique
///   `shortcut/<id>`, a `gate` and an `unlock` some area's prefab provides, and
///   the two must be different anchors (the mechanism sits on the FAR side, not
///   in the doorway it opens).
/// * `DW0372` — no `close-gate` anywhere may target a gate a shortcut owns. A
///   shortcut opens permanently; making that structural is cheaper and safer than
///   trusting every author to never reach for the re-seal verb. `close-gate` on
///   any other gate (the point-of-no-return beat) is untouched.
///
/// Anchor resolution stays lenient for pool areas the compiler resolves later —
/// the same policy as the trap and trigger checks.
pub(crate) fn shortcut_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    if quests.shortcuts.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, sc) in quests.shortcuts.iter().enumerate() {
        if !sc.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                SHORTCUT_INVALID,
                "quests",
                format!("/content/shortcuts/{i}/id"),
                format!(
                    "malformed shortcut id `{}` — shortcut ids must be lowercase kebab-case with \
                     the `shortcut/` prefix (e.g. `shortcut/keep-lift`)",
                    sc.id
                ),
            ));
        }
        if !seen.insert(sc.id.as_str()) {
            d.push(Diagnostic::error(
                SHORTCUT_INVALID,
                "quests",
                format!("/content/shortcuts/{i}/id"),
                format!(
                    "duplicate shortcut id `{}` — rename one so every shortcut id is unique",
                    sc.id
                ),
            ));
        }
        // A shortcut's two anchors are two SHAPES: the `gate` is the region it
        // clears on unlock, and the `unlock` is the cell its far-side affordance
        // stands on. They travelled together through one resolvable() call
        // because a name was all either needed.
        for (field, anchor, demands) in [
            ("gate", &sc.gate, crate::layout::StationKind::Gate),
            ("unlock", &sc.unlock, crate::layout::StationKind::Point),
        ] {
            if let Some(f) = station_kind_diag(
                &providers,
                anchor.as_str(),
                demands,
                &format!("a shortcut's `{field}`"),
                "quests",
                format!("/content/shortcuts/{i}/{field}"),
            ) {
                d.push(f);
                continue;
            }
            if !providers.resolvable(anchor.as_str()) {
                d.push(Diagnostic::error(
                    SHORTCUT_INVALID,
                    "quests",
                    format!("/content/shortcuts/{i}/{field}"),
                    format!(
                        "shortcut `{field}` anchor `{anchor}` is not provided by any area's \
                         prefab — {}",
                        providers.anchor_remedy(
                            "use an anchor a prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
        }
        if sc.gate == sc.unlock {
            d.push(Diagnostic::error(
                SHORTCUT_INVALID,
                "quests",
                format!("/content/shortcuts/{i}/unlock"),
                format!(
                    "shortcut `{}` unlocks at its own gate anchor `{}` — the mechanism belongs on \
                     the FAR side of the door you have not opened yet, which is the entire point \
                     of the pattern (spec-0016 §2)",
                    sc.id, sc.gate
                ),
            ));
        }
    }

    // `close-gate` may never target a shortcut gate: permanence is structural.
    let owned: BTreeSet<&str> = quests.shortcuts.iter().map(|s| s.gate.as_str()).collect();
    let report = |path: String, anchor: &str, d: &mut Vec<Diagnostic>| {
        d.push(Diagnostic::error(
            SHORTCUT_RESEALED,
            "quests",
            path,
            format!(
                "`close-gate` targets `{anchor}`, a gate a `shortcut` owns — a shortcut opens \
                 PERMANENTLY (spec-0016 §2), so nothing may re-seal it. Use a different gate for \
                 the point-of-no-return beat, or drop the shortcut declaration."
            ),
        ));
    };
    for (qi, q) in quests.quests.iter().enumerate() {
        for_each_effect_deep(q, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && owned.contains(a.as_str())
            {
                report(format!("/content/quests/{qi}/{path}/anchor"), a.as_str(), d);
            }
        });
    }
    for (ti, t) in quests.triggers.iter().enumerate() {
        for_each_trigger_effect_deep(t, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && owned.contains(a.as_str())
            {
                report(
                    format!("/content/triggers/{ti}/{path}/anchor"),
                    a.as_str(),
                    d,
                );
            }
        });
    }
}
