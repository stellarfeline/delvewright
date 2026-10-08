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
