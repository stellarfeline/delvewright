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
    /// Environment-trigger id: `trigger/<kebab>` (stage-5 `triggers` section,
    /// DSL v0.4). Unique within the stage-5 triggers namespace.
    TriggerId, "trigger");
prefixed_id!(
    /// Scripted-actor id: `actor/<kebab>` (stage-5 `actors` section, DSL v0.6,
    /// spec-0014). Unique within the stage-5 actors namespace; the puppet body
    /// is tagged `dw_actor_<kebab>`.
    ActorId, "actor");

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
