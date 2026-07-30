//! The stage envelope, the assembled [`Campaign`], and parsing from raw JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::ids::CampaignId;
use crate::stages::{
    ClassesContent, DialogueContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
};

/// The latest `dsl_version` this crate implements (identity / tooling default).
pub const SUPPORTED_DSL_VERSION: &str = "0.3.0";

/// Every `dsl_version` this crate accepts. v0.3 is an **additive superset** of
/// v0.2 (new stage-5 verbs, waves, flags); v0.2 campaigns remain valid and
/// compile byte-identically. The reserved verbs (`kill`/`collect`/`interact`,
/// `give-item`/`set-flag`/`spawn-wave`) are implemented only under `0.3.0`; a
/// `0.2.0` document using them is still rejected with `DW0141`.
pub const SUPPORTED_DSL_VERSIONS: &[&str] = &["0.2.0", "0.3.0"];

/// True if `version` is a `dsl_version` this crate accepts.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_DSL_VERSIONS.contains(&version)
}

/// True if `version` enables the DSL v0.3 verbs (`kill`/`collect`/`interact`,
/// `give-item`/`set-flag`/`spawn-wave`, waves, flags).
pub fn is_v03(version: &str) -> bool {
    version == "0.3.0"
}

/// Which stage a document belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Stage 1.
    World,
    /// Stage 2.
    Npcs,
    /// Stage 3.
    Classes,
    /// Stage 4.
    QuestPlan,
    /// Stage 5.
    Quests,
    /// Stage 6.
    Dialogue,
}

impl Stage {
    /// The wire/filename name (`world`, `npcs`, `classes`, `quest-plan`,
    /// `quests`, `dialogue`).
    pub fn name(self) -> &'static str {
        match self {
            Stage::World => "world",
            Stage::Npcs => "npcs",
            Stage::Classes => "classes",
            Stage::QuestPlan => "quest-plan",
            Stage::Quests => "quests",
            Stage::Dialogue => "dialogue",
        }
    }
}

/// A stage document: `{ dsl_version, campaign_id, stage, content }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    /// DSL version string.
    pub dsl_version: String,
    /// Owning campaign id.
    pub campaign_id: CampaignId,
    /// The stage this document is for.
    pub stage: Stage,
    /// The stage payload.
    pub content: T,
}

/// The six parsed stage documents that make up one campaign.
#[derive(Clone, Debug, PartialEq)]
pub struct Campaign {
    /// Stage 1.
    pub world: Envelope<WorldContent>,
    /// Stage 2.
    pub npcs: Envelope<NpcsContent>,
    /// Stage 3.
    pub classes: Envelope<ClassesContent>,
    /// Stage 4.
    pub quest_plan: Envelope<QuestPlanContent>,
    /// Stage 5.
    pub quests: Envelope<QuestsContent>,
    /// Stage 6.
    pub dialogue: Envelope<DialogueContent>,
}

/// The six stage documents as raw JSON strings (compiler input).
#[derive(Clone, Debug, PartialEq)]
pub struct RawCampaign {
    /// `world.json`.
    pub world: String,
    /// `npcs.json`.
    pub npcs: String,
    /// `classes.json`.
    pub classes: String,
    /// `quest-plan.json`.
    pub quest_plan: String,
    /// `quests.json`.
    pub quests: String,
    /// `dialogue.json`.
    pub dialogue: String,
}

fn parse_stage<T: for<'de> Deserialize<'de>>(
    src: &str,
    stage: Stage,
    out: &mut Result<Envelope<T>, ()>,
    diags: &mut Vec<Diagnostic>,
) {
    match serde_json::from_str::<Envelope<T>>(src) {
        Ok(env) => *out = Ok(env),
        Err(e) => {
            *out = Err(());
            diags.push(Diagnostic::error(
                codes::SCHEMA,
                stage.name(),
                "",
                format!("schema violation: {e}"),
            ));
        }
    }
}

/// Parse all six stage documents.
///
/// On any schema/parse failure returns every `DW0100` diagnostic collected
/// (validation cannot run on unparseable input).
pub fn parse_campaign(raw: &RawCampaign) -> Result<Campaign, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let mut world = Err(());
    let mut npcs = Err(());
    let mut classes = Err(());
    let mut quest_plan = Err(());
    let mut quests = Err(());
    let mut dialogue = Err(());

    parse_stage(&raw.world, Stage::World, &mut world, &mut diags);
    parse_stage(&raw.npcs, Stage::Npcs, &mut npcs, &mut diags);
    parse_stage(&raw.classes, Stage::Classes, &mut classes, &mut diags);
    parse_stage(
        &raw.quest_plan,
        Stage::QuestPlan,
        &mut quest_plan,
        &mut diags,
    );
    parse_stage(&raw.quests, Stage::Quests, &mut quests, &mut diags);
    parse_stage(&raw.dialogue, Stage::Dialogue, &mut dialogue, &mut diags);

    match (world, npcs, classes, quest_plan, quests, dialogue) {
        (Ok(world), Ok(npcs), Ok(classes), Ok(quest_plan), Ok(quests), Ok(dialogue)) => {
            Ok(Campaign {
                world,
                npcs,
                classes,
                quest_plan,
                quests,
                dialogue,
            })
        }
        _ => Err(diags),
    }
}

/// Parse then validate. Convenience over [`parse_campaign`] +
/// [`crate::validate::validate_campaign`].
pub fn check_campaign(raw: &RawCampaign) -> Vec<Diagnostic> {
    match parse_campaign(raw) {
        Ok(campaign) => crate::validate::validate_campaign(&campaign),
        Err(diags) => diags,
    }
}
