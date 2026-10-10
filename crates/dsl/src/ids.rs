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

            /// The form THIS id has to take, for the diagnostic that rejected
            /// one — `` `npc/<kebab>` ``, `` `dlg/<kebab>` ``, and so on.
            ///
            /// A refusal that says only "ids must be lowercase kebab-case with
            /// their type prefix" and then lists three examples of other types
            /// has told the author the general rule and withheld the one fact
            /// they were missing: which prefix THIS field takes. The prefix is
            /// a property of the id type, so the answer is derived from the
            /// type at every site rather than hand-written per site — the
            /// per-section refusals that already name their own prefix
            /// (`wave/`, `trap/`, `loot/`, …) are that fact copied by hand,
            /// which is exactly why the general path never had it.
            pub fn syntax_form(&self) -> String {
                format!("`{}/<kebab>`", Self::PREFIX)
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
    /// Assembly id: `assembly/<kebab>` (stage-5 `assemblies` section, spec-0082).
    /// A fixed thing built of display entities that plays clips, can be struck
    /// and strikes back. Its root, parts and hitbox are tagged `dw_asm_<kebab>`.
    AssemblyId, "assembly");
prefixed_id!(
    /// Rig id: `rig/<kebab>` (spec-0082 §3.1). Resolved against the library's
    /// `rigs/<kebab>/rig.json`, the way `prefab/<kebab>` resolves against a
    /// piece's metadata: a rig is a library artefact a generator writes, never
    /// campaign JSON.
    RigId, "rig");
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
    /// Loop id: `loop/<kebab>` (stage-5 `loops` section, spec-0086). Unique within
    /// the stage-5 loop namespace; it names the loop's emitted functions, its
    /// PackTest pair, its `on_cross` l10n keys and the loop a seamlessness or route
    /// finding blames.
    LoopId, "loop");
prefixed_id!(
    /// Pulse id: `pulse/<kebab>` (stage-5 `pulses` section, spec-0102). Unique
    /// within the stage-5 pulse namespace; it names the pulse's emitted
    /// function, its latch holder, its PackTest templates and its ledger row.
    PulseId, "pulse");
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
prefixed_id!(
    /// Site-plan datum id: `datum/<kebab>` (spec-0049 §4.1). A named ground
    /// plane a box's floor sits on. Unique within the site plan.
    DatumId, "datum");
prefixed_id!(
    /// Site-plan volume id: `volume/<kebab>` (spec-0049 §4.1). A mass the WHOLE
    /// owns — the mountain a cave system is inside, the ground under a village,
    /// the sky a silhouette needs kept empty. Unique within the site plan.
    VolumeId, "volume");
prefixed_id!(
    /// Atmosphere id: `atmosphere/<kebab>` (stage-1 `atmospheres[]`, spec-0080).
    /// Unique within the campaign; it names the datapack biome the atmosphere
    /// ships as (`<ns>:atmosphere/<kebab>`), which a place carries and a
    /// `set-atmosphere` paints.
    AtmosphereId, "atmosphere");
prefixed_id!(
    /// Site-plan view id: `view/<kebab>` (spec-0049 §4.1). A named exterior
    /// vantage the walk judges the silhouette from. Unique within the site plan.
    ViewId, "view");

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

// ---------------------------------------------------------------------------
// Validation: the three rules every id collection obeys
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, codes};

/// **`DW0110`: refuse a malformed id**, pushing onto `$d`.
///
/// The form is taken from the id's own type (`syntax_form`), never written
/// into this message. This macro is the ONE path every id type's syntax
/// refusal goes through, and it used to answer all of them with the same
/// three examples — `area/keep`, `npc/keeper`, `quest/find-key` — so a
/// rejected dialogue node id was refused by a sentence that never spelled
/// `dlg/<kebab>`, and the rule it needed lived only in the schema
/// description. The prefix belongs to the id type, so every site gets it
/// from the type: the general mechanism was here all along, and only its
/// message was too narrow to reach what it was rejecting.
macro_rules! id_syntax {
    ($d:expr, $id:expr, $stage:expr, $path:expr) => {
        if !$id.is_valid_syntax() {
            $d.push($crate::diagnostic::Diagnostic::error(
                $crate::diagnostic::codes::ID_SYNTAX,
                $stage,
                $path,
                format!(
                    "malformed id `{}` — this field takes {}: the type prefix, a `/`, and \
                     one lowercase kebab-case segment after it ([a-z0-9] and `-`, no second \
                     `/`, no capitals, no underscores)",
                    $id,
                    $id.syntax_form()
                ),
            ));
        }
    };
}
pub(crate) use id_syntax;

/// **`DW0111`: refuse the second of two equal ids** in one namespace, at the
/// later one's path.
pub(crate) fn dup_check<'a>(
    ids: impl Iterator<Item = (&'a str, String)>,
    stage: &'static str,
    what: &str,
    d: &mut Vec<Diagnostic>,
) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (id, path) in ids {
        if !seen.insert(id) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                stage,
                path,
                format!("duplicate {what} id `{id}` — rename one so every {what} id is unique"),
            ));
        }
    }
}

/// **`DW0112`: refuse a reference that does not resolve** — `ok` is whether it
/// does, and `msg` is the refusal's own wording.
pub(crate) fn dangling(
    d: &mut Vec<Diagnostic>,
    ok: bool,
    stage: &'static str,
    path: String,
    msg: String,
) {
    if !ok {
        d.push(Diagnostic::error(codes::DANGLING_REF, stage, path, msg));
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
