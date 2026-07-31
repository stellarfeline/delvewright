//! Delvewright DSL: serde types, validation, canonical serialization and JSON
//! Schema export for the staged campaign DSL (spec-0001).
//!
//! Entry points:
//! - [`parse_campaign`] / [`check_campaign`]: parse the six raw stage documents.
//! - [`validate_campaign`]: run all spec-0001 v0.2 rule groups on a parsed
//!   [`Campaign`], returning [`Diagnostic`]s (spec-0002 `--json` shape).
//! - [`to_canonical_string`]: the single canonical writer.
//! - [`stage_schema`]: export a stage's JSON Schema.
//!
//! Determinism (ADR-0006): all iteration is over `BTreeMap`/`BTreeSet` or slices;
//! nothing depends on hash order, wall-clock, or absolute paths.

pub mod canonical;
pub mod diagnostic;
pub mod envelope;
pub mod ids;
pub mod l10n;
pub mod registry;
pub mod schema;
pub mod stages;
pub mod validate;

pub use canonical::to_canonical_string;
pub use diagnostic::{Diagnostic, Severity, codes};
pub use envelope::{
    Campaign, Envelope, RawCampaign, SUPPORTED_DSL_VERSION, SUPPORTED_DSL_VERSIONS, Stage,
    check_campaign, is_supported_version, is_v03, is_v04, parse_campaign,
};
pub use ids::{
    AnchorId, AreaId, CampaignId, ClassId, DialogueId, FlagId, NpcId, ObjectiveId, PoolId,
    PrefabId, QuestId, TriggerId, WaveId,
};
pub use l10n::{
    CANONICAL_LANG, L10nDoc, L10nKind, each_string, inventory as l10n_inventory, localize,
    validate_l10n,
};
pub use registry::{
    AnchorRegistry, BlockRegistry, EffectRegistry, EntityRegistry, ItemBackedBlockRegistry,
    ItemRegistry, Lighting, LightingProfile, VendoredAnchorRegistry, VendoredEffectRegistry,
    VendoredEntityRegistry, VendoredItemRegistry, is_technical_block,
};
pub use schema::stage_schema;
pub use stages::{
    Area, CameraWaypoint, Class, ClassesContent, DialogueContent, DialogueEffect, DialogueNode,
    DialogueOption, EnvTrigger, KitItem, MobAttributes, MobEffect, NarrateStyle, Npc, NpcDialogue,
    NpcSkin, NpcsContent, Objective, Persona, Pieces, PlannedQuest, Prop, Quest, QuestEffect,
    QuestPlanContent, QuestsContent, Relationship, Role, SkinModel, Trigger, TriggerOn, Wave,
    WaveMob, WorldContent,
};
pub use validate::{validate_campaign, validate_campaign_with};
