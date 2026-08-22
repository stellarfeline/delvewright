//! JSON Schema export (one full envelope schema per stage) via `schemars`.
//!
//! The schemas are *derived* from the Rust types — they are the LLM-facing
//! authoring aid (spec-0001) and the source of truth is the Rust struct.

use schemars::schema_for;

use crate::detailplan::DetailPlanContent;
use crate::envelope::{Envelope, Stage};
use crate::layout::{GeometryBriefContent, LayoutGraphContent};
use crate::siteplan::SitePlanContent;
use crate::stages::{
    ClassesContent, DialogueContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
    WorldEditsContent,
};

/// The JSON Schema (draft 2020-12) for a stage's full envelope document.
pub fn stage_schema(stage: Stage) -> serde_json::Value {
    let schema = match stage {
        Stage::World => schema_for!(Envelope<WorldContent>),
        Stage::Npcs => schema_for!(Envelope<NpcsContent>),
        Stage::Classes => schema_for!(Envelope<ClassesContent>),
        Stage::QuestPlan => schema_for!(Envelope<QuestPlanContent>),
        Stage::Quests => schema_for!(Envelope<QuestsContent>),
        Stage::Dialogue => schema_for!(Envelope<DialogueContent>),
        Stage::WorldEdits => schema_for!(Envelope<WorldEditsContent>),
        Stage::GeometryBrief => schema_for!(Envelope<GeometryBriefContent>),
        Stage::LayoutGraph => schema_for!(Envelope<LayoutGraphContent>),
        Stage::SitePlan => schema_for!(Envelope<SitePlanContent>),
        Stage::DetailPlan => schema_for!(Envelope<DetailPlanContent>),
    };
    serde_json::to_value(schema).expect("schema serializes to JSON")
}
