//! Type-prefixed, kebab-case identifier newtypes (spec-0001 "IDs").
//!
//! IDs deserialize permissively from any JSON string so that *syntax* violations
//! surface as validation diagnostics (`DW0110`) rather than opaque parse errors.
//! Call [`is_valid_syntax`](AreaId::is_valid_syntax) during validation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// True if `s` is a single kebab-case token: `[a-z0-9]+(-[a-z0-9]+)*`.
pub(crate) fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// True if `s` is exactly `<prefix>/<kebab>`.
pub(crate) fn is_prefixed(s: &str, prefix: &str) -> bool {
    match s.strip_prefix(prefix).and_then(|r| r.strip_prefix('/')) {
        Some(rest) => is_kebab(rest),
        None => false,
    }
}

macro_rules! prefixed_id {
    ($(#[$m:meta])* $name:ident, $prefix:literal) => {
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(transparent)]
        #[schemars(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// The required type prefix (`area`, `npc`, …).
            pub const PREFIX: &'static str = $prefix;

            /// Borrow the raw id string.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// True if the id is well-formed: `<prefix>/<kebab>`.
            pub fn is_valid_syntax(&self) -> bool {
                is_prefixed(&self.0, Self::PREFIX)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

prefixed_id!(
    /// Area id: `area/<kebab>` (stage 1).
    AreaId, "area");
prefixed_id!(
    /// NPC id: `npc/<kebab>` (stage 2).
    NpcId, "npc");
prefixed_id!(
    /// Class id: `class/<kebab>` (stage 3).
    ClassId, "class");
prefixed_id!(
    /// Quest id: `quest/<kebab>` (stages 4 & 5).
    QuestId, "quest");
prefixed_id!(
    /// Prefab id: `prefab/<kebab>` (resolved against `prefabs/` at compile time).
    PrefabId, "prefab");
prefixed_id!(
    /// Prefab-pool id: `pool/<kebab>` (jigsaw multi-piece assembly, stage 1;
    /// resolved against `prefabs/` metadata at compile time).
    PoolId, "pool");
prefixed_id!(
    /// Dialogue node id: `dlg/<kebab>` (stage-local to an NPC's dialogue graph).
    DialogueId, "dlg");
prefixed_id!(
    /// Objective id: `obj/<kebab>` (stage-local to stage 5).
    ObjectiveId, "obj");
prefixed_id!(
    /// Anchor id: `anchor/<kebab>` (resolved against prefab metadata).
    AnchorId, "anchor");
prefixed_id!(
    /// Wave id: `wave/<kebab>` (stage-5 `waves` section, DSL v0.3).
    WaveId, "wave");
prefixed_id!(
    /// Flag id: `flag/<kebab>` (declared by `set-flag` effects, read by
    /// `requires_flags`, DSL v0.3). No separate declaration list — the set of
    /// flags is exactly those produced by some `set-flag` effect.
    FlagId, "flag");
prefixed_id!(
    /// Runtime-state id: `state/<kebab>` (stage-5 `state` section, DSL v0.10,
    /// spec-0031). A named, scoped, integer-valued datum.
    ///
    /// Unlike [`FlagId`] this one **is** declared. A datum's scope (per-player or
    /// party-wide) is a multiplayer semantic that no inference can supply, and a
    /// counter — unlike a monotonic "this happened" flag — has an initial value
    /// that only a declaration can state.
    StateId, "state");
prefixed_id!(
    /// Environment-trigger id: `trigger/<kebab>` (stage-5 `triggers` section,
    /// DSL v0.4). Unique within the stage-5 triggers namespace.
    TriggerId, "trigger");
prefixed_id!(
    /// Scripted-actor id: `actor/<kebab>` (stage-5 `actors` section, DSL v0.6,
    /// spec-0014). Unique within the stage-5 actors namespace; the puppet body
    /// is tagged `dw_actor_<kebab>`.
    ActorId, "actor");
prefixed_id!(
    /// Trap id: `trap/<kebab>` (stage-5 `traps` section, DSL v0.6, spec-0011).
    /// Unique within the stage-5 traps namespace.
    TrapId, "trap");
prefixed_id!(
    /// Timed-gate id: `timed-gate/<kebab>` (stage-5 `timed_gates` section,
    /// spec-0016 §4). Unique within the stage-5 timed-gate namespace.
    TimedGateId, "timed-gate");
prefixed_id!(
    /// Ambush id: `ambush/<kebab>` (stage-5 `ambushes` section, spec-0016 §3).
    /// Unique within the stage-5 ambushes namespace; the derived environment
    /// trigger is named `trigger/<kebab>` from the same local id.
    AmbushId, "ambush");
prefixed_id!(
    /// Shortcut id: `shortcut/<kebab>` (stage-5 `shortcuts` section, spec-0016 §2).
    /// Unique within the stage-5 shortcuts namespace.
    ShortcutId, "shortcut");
prefixed_id!(
    /// Branch-point id: `branch-point/<kebab>` (stage-4 `branch_points`, DSL v0.8,
    /// spec-0025). One declared fork in the story: the flags it forks on, the quest
    /// it opens at, and the branches it offers.
    BranchPointId, "branch-point");
prefixed_id!(
    /// Branch id: `branch/<kebab>` (stage-4 `branch_points[].branches`, DSL v0.8,
    /// spec-0025). One alternative of a branch point. Unique campaign-wide, because
    /// it names the emitted `validation/branch-chronicle-<kebab>.md`.
    BranchId, "branch");
prefixed_id!(
    /// Ending id: `ending/<kebab>` (DSL v0.8, spec-0025). Declared on the
    /// `campaign-complete` effect that ends the delve that way, and referenced by
    /// the branch that runs to it. There is no separate declaration list — the set
    /// of endings is exactly those named by some `campaign-complete`, the same rule
    /// [`FlagId`] follows.
    EndingId, "ending");
prefixed_id!(
    /// Loot-fill id: `loot/<kebab>` (stage-5 `loot` section, spec-0021). Unique
    /// within the stage-5 loot namespace.
    LootId, "loot");
prefixed_id!(
    /// Lethal-volume id: `lethal/<kebab>` (stage-5 `lethal_volumes` section, DSL
    /// v0.10, spec-0031). Unique within the stage-5 lethal-volume namespace; it
    /// names the volume's emitted tick function, its l10n key, and the volume a
    /// completability finding blames.
    LethalVolumeId, "lethal");
prefixed_id!(
    /// Shop id: `shop/<kebab>` (stage-5 `shops` section, DSL v0.10, spec-0032).
    /// Unique within the stage-5 shop namespace; it names the shop's interaction
    /// affordance, its dialog, its `/trigger` routing value and its l10n keys.
    ShopId, "shop");
prefixed_id!(
    /// Recovery-stake id: `stake/<kebab>` (stage-5 `stakes` section, DSL v0.10,
    /// spec-0032). Unique within the stage-5 stake namespace; it names the
    /// per-player ledger objectives, the marker hardware's tag and the l10n key of
    /// the line a collection says.
    StakeId, "stake");
prefixed_id!(
    /// Edit-batch id: `batch/<kebab>` (stage-7 `world-edits` batches, DSL v0.6,
    /// spec-0017). Unique within the edit script; also the batch's snapshot name
    /// and its seed-stream label, so renaming a batch deliberately reseeds it.
    EditBatchId, "batch");
prefixed_id!(
    /// Named edit region: `region/<kebab>` (stage-7 `select` verb, DSL v0.6,
    /// spec-0017). Scoped to its batch; later edits in the batch refer back to it.
    RegionId, "region");
prefixed_id!(
    /// Layout-graph node id: `node/<kebab>` (spec-0049 §3.1). A **place** — a
    /// room, a courtyard, a stretch of shore, a cavern — named before any
    /// coordinate exists. Unique within the layout graph.
    NodeId, "node");
prefixed_id!(
    /// Layout-graph edge id: `edge/<kebab>` (spec-0049 §3.1). A connection
    /// between two places, of a declared class. Unique within the layout graph.
    EdgeId, "edge");
prefixed_id!(
    /// Geometry-brief fact id: `fact/<kebab>` (spec-0049 §4.2). A number with a
    /// name, taken from the whole map's written brief. Unique within the brief;
    /// a site plan's `identities[]` bind to these.
    FactId, "fact");

/// Campaign id: a bare kebab-case token (no type prefix).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct CampaignId(pub String);

impl CampaignId {
    /// Borrow the raw id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True if the id is a bare kebab-case token.
    pub fn is_valid_syntax(&self) -> bool {
        is_kebab(&self.0)
    }
}

impl std::fmt::Display for CampaignId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_rules() {
        assert!(is_kebab("open-the-door"));
        assert!(is_kebab("keep"));
        assert!(is_kebab("area1"));
        assert!(!is_kebab("Open"));
        assert!(!is_kebab("open_the_door"));
        assert!(!is_kebab("open--the"));
        assert!(!is_kebab("-open"));
        assert!(!is_kebab(""));
    }

    #[test]
    fn prefixed_rules() {
        assert!(AreaId("area/keep".into()).is_valid_syntax());
        assert!(!AreaId("area/Keep".into()).is_valid_syntax());
        assert!(!AreaId("keep".into()).is_valid_syntax());
        assert!(!AreaId("npc/keeper".into()).is_valid_syntax());
        assert!(NpcId("npc/keeper".into()).is_valid_syntax());
    }
}
