//! The stage envelope, the assembled [`Campaign`], and parsing from raw JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::ids::CampaignId;
use crate::stages::{
    ClassesContent, DialogueContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
};

/// The latest `dsl_version` this crate implements (identity / tooling default).
pub const SUPPORTED_DSL_VERSION: &str = "0.6.0";

/// Every `dsl_version` this crate accepts. Each version is an **additive
/// superset** of the previous: v0.3 added the stage-5 verbs/waves/flags; v0.4
/// (spec-0008) adds dialogue state, props, narration, live-threat tuning, NPC
/// lifecycle + skins, environment triggers, cutscenes and named given items;
/// v0.5 (spec-0010) adds declared world `time`/`weather`, per-area `lighting`
/// (deterministic relight), and the `set-time`/`set-weather` effect verbs; v0.6
/// adds the stage-1 `horizon` (`ocean` backdrop) + `boundary` (spec-0013) and the
/// staging surface — the `play-sound` effect and the `narrate` `art` style
/// (spec-0014), alongside the actors/stealth/sequence surface from sibling PRs.
/// Older campaigns remain valid and compile byte-identically. A construct
/// introduced in a later version is rejected with `DW0141` in an earlier one.
pub const SUPPORTED_DSL_VERSIONS: &[&str] = &["0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0"];

/// True if `version` is a `dsl_version` this crate accepts.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_DSL_VERSIONS.contains(&version)
}

/// True if `version` enables the DSL v0.3 verbs (`kill`/`collect`/`interact`,
/// `give-item`/`set-flag`/`spawn-wave`, waves, flags). v0.4 is an additive
/// superset, so it enables the whole v0.3 surface too.
pub fn is_v03(version: &str) -> bool {
    version == "0.3.0" || version == "0.4.0" || version == "0.5.0" || version == "0.6.0"
}

/// True if `version` enables the DSL v0.4 surface (spec-0008): dialogue
/// `set-flag` + `requires_flags`, props (`interact.prop`, `set-block`),
/// `narrate`, wave `attributes`/`effects`, `despawn-npc`/`move-npc`, `cutscene`,
/// NPC `skin`, stage-5 `triggers`, named `give-item`, objective `stealth`.
pub fn is_v04(version: &str) -> bool {
    version == "0.4.0" || version == "0.5.0" || version == "0.6.0"
}

/// True if `version` enables the DSL v0.5 surface (spec-0010): declared world
/// `time`/`weather`, per-area `lighting` (deterministic relight fixtures), and
/// the `set-time`/`set-weather` effect verbs. v0.6 is an additive superset.
pub fn is_v05(version: &str) -> bool {
    version == "0.5.0" || version == "0.6.0"
}

/// True if `version` enables the DSL v0.6 surface: the stage-1 `horizon`/`boundary`
/// world fields (spec-0013) and the `play-sound` effect + `narrate` `art` style
/// (spec-0014). Additive over v0.5; a campaign that uses none is byte-identical,
/// and a use of the v0.6 surface in an earlier campaign is rejected with `DW0141`.
pub fn is_v06(version: &str) -> bool {
    version == "0.6.0"
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
                format!(
                    "`{}` stage document does not conform to its schema: {e}. Fix the offending \
                     field (unknown field, wrong type, or missing required one) in the campaign \
                     JSON to match the schema — run `delvec schema --stage <1..6>` to see the \
                     exact shape.",
                    stage.name()
                ),
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
