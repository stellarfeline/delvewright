//! Delvewright DSL: serde types, validation, canonical serialization and JSON
//! Schema export for the staged campaign DSL (spec-0001).
//!
//! Entry points:
//! - [`parse_campaign`] / [`check_campaign`]: parse the five raw stage documents.
//! - [`validate_campaign`]: run all six spec-0001 rule groups on a parsed
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
pub mod registry;
pub mod schema;
pub mod stages;
pub mod validate;

pub use canonical::to_canonical_string;
pub use diagnostic::{Diagnostic, Severity, codes};
pub use envelope::{
    Campaign, Envelope, RawCampaign, SUPPORTED_DSL_VERSION, Stage, check_campaign, parse_campaign,
};
pub use ids::{
    AnchorId, AreaId, CampaignId, ClassId, DialogueId, NpcId, ObjectiveId, PrefabId, QuestId,
};
pub use registry::{AnchorRegistry, ItemRegistry, VendoredAnchorRegistry, VendoredItemRegistry};
pub use schema::stage_schema;
pub use stages::{
    Area, Class, ClassesContent, Dialogue, DialogueEffect, DialogueNode, DialogueOption, KitItem,
    Npc, NpcsContent, Objective, PlannedQuest, PrefabPool, Quest, QuestEffect, QuestPlanContent,
    QuestsContent, Role, Trigger, WorldContent,
};
pub use validate::{validate_campaign, validate_campaign_with};
