//! The stage envelope, the assembled [`Campaign`], and parsing from raw JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::detailplan::DetailPlanContent;
use crate::diagnostic::{Diagnostic, codes};
use crate::ids::CampaignId;
use crate::layout::{GeometryBriefContent, LayoutGraphContent};
use crate::siteplan::SitePlanContent;
use crate::stages::{
    ClassesContent, DialogueContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
    WorldEditsContent,
};

/// **The one `dsl_version` this engine accepts** (ADR-0024).
///
/// Every envelope this engine reads — the stage documents, the map-pipeline
/// documents, an l10n sidecar — declares exactly this number; any other is
/// refused at the envelope with `DW0102`, which names this constant. The number
/// says which surface a document was written against, so a refusal can say
/// why, and it promises nothing about any other engine: a released campaign is
/// built by the engine it pins (`versions.toml`), and a surface change bumps
/// this number and moves every document in this repository with it.
pub const DSL_VERSION: &str = "0.19.0";

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
    /// Stage 7 (optional; DSL v0.6, spec-0017): the map-editor edit script.
    WorldEdits,
    /// The whole map's written brief, reduced to numbers (optional; DSL v0.13,
    /// spec-0049 §4.2). Named, never renumbered into the 1..7 sequence: it is a
    /// different pipeline's document and the two orderings are unrelated.
    GeometryBrief,
    /// The campaign's space as a graph, before any coordinate exists (optional;
    /// DSL v0.13, spec-0049 §3).
    LayoutGraph,
    /// The geometric embedding of that graph — the whole map's design of record
    /// (optional; DSL v0.14, spec-0049 §4).
    SitePlan,
    /// Which piece stands in which of the plan's places (optional; DSL v0.15,
    /// spec-0050). Named, never renumbered, for the reason `GeometryBrief`
    /// gives.
    DetailPlan,
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
            Stage::WorldEdits => "world-edits",
            Stage::GeometryBrief => "geometry-brief",
            Stage::LayoutGraph => "layout-graph",
            Stage::SitePlan => "site-plan",
            Stage::DetailPlan => "detail-plan",
        }
    }

    /// **Every stage, in document order.** The one enumeration.
    ///
    /// Hand-written stage lists are how a new document escapes a gate that was
    /// written before it existed: `crates/dsl/tests/gate_consumers.rs` walked
    /// seven stages by name, so a schema object declaring part of the gate in an
    /// eighth would have been invisible to the check whose whole subject is that
    /// no such object exists. Anything that means "over the stages" reads this.
    pub const ALL: [Stage; 11] = [
        Stage::World,
        Stage::Npcs,
        Stage::Classes,
        Stage::QuestPlan,
        Stage::Quests,
        Stage::Dialogue,
        Stage::WorldEdits,
        Stage::GeometryBrief,
        Stage::LayoutGraph,
        Stage::SitePlan,
        Stage::DetailPlan,
    ];
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

/// The parsed stage documents that make up one campaign: the six required
/// stages plus the optional stage-7 edit script (spec-0017).
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
    /// Stage 7 (optional; DSL v0.6, spec-0017): the map-editor edit script.
    /// `None` = no `world-edits.json` in the campaign directory — byte-identical
    /// to a campaign from before the stage existed.
    pub world_edits: Option<Envelope<WorldEditsContent>>,
    /// The whole map's brief as numbers (optional; DSL v0.13, spec-0049 §4.2).
    pub geometry_brief: Option<Envelope<GeometryBriefContent>>,
    /// The campaign's space as a graph (optional; DSL v0.13, spec-0049 §3).
    pub layout_graph: Option<Envelope<LayoutGraphContent>>,
    /// The geometric embedding of that graph (optional; DSL v0.14, spec-0049 §4).
    pub site_plan: Option<Envelope<SitePlanContent>>,
    /// Which piece fills which of the plan's places (optional; DSL v0.15,
    /// spec-0050 §1).
    pub detail_plan: Option<Envelope<DetailPlanContent>>,
}

/// The stage documents as raw JSON strings (compiler input): six required, the
/// stage-7 edit script optional.
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
    /// `world-edits.json` (optional stage 7, spec-0017); `None` when the
    /// campaign directory ships none.
    pub world_edits: Option<String>,
    /// `geometry-brief.json` (optional; spec-0049 §4.2).
    pub geometry_brief: Option<String>,
    /// `layout-graph.json` (optional; spec-0049 §3).
    pub layout_graph: Option<String>,
    /// `site-plan.json` (optional; spec-0049 §4).
    pub site_plan: Option<String>,
    /// `detail-plan.json` (optional; spec-0050 §1).
    pub detail_plan: Option<String>,
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
                    "`{name}` stage document does not conform to its schema: {e}. Fix the \
                     offending field (unknown field, wrong type, or missing required one) in the \
                     campaign JSON to match the schema — run `delvec schema --stage {name}` to \
                     see the exact shape of THIS document. The schema is the authority on the \
                     form; a spec that disagrees with it is the stale one.",
                    name = stage.name()
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
    // The optional stage-7 edit script (spec-0017): absent = `None`; present but
    // unparseable = a `DW0100` like any other stage.
    let mut world_edits: Result<Option<Envelope<WorldEditsContent>>, ()> = Ok(None);
    if let Some(src) = &raw.world_edits {
        let mut parsed = Err(());
        parse_stage(src, Stage::WorldEdits, &mut parsed, &mut diags);
        world_edits = parsed.map(Some);
    }
    // The spec-0049 map-pipeline documents, on the same terms: absent = `None`
    // and a campaign that ships neither is byte-identical to one from before
    // they existed; present = parsed, validated and hashed like any other stage.
    let mut geometry_brief: Result<Option<Envelope<GeometryBriefContent>>, ()> = Ok(None);
    if let Some(src) = &raw.geometry_brief {
        let mut parsed = Err(());
        parse_stage(src, Stage::GeometryBrief, &mut parsed, &mut diags);
        geometry_brief = parsed.map(Some);
    }
    let mut layout_graph: Result<Option<Envelope<LayoutGraphContent>>, ()> = Ok(None);
    if let Some(src) = &raw.layout_graph {
        let mut parsed = Err(());
        parse_stage(src, Stage::LayoutGraph, &mut parsed, &mut diags);
        layout_graph = parsed.map(Some);
    }
    let mut site_plan: Result<Option<Envelope<SitePlanContent>>, ()> = Ok(None);
    if let Some(src) = &raw.site_plan {
        let mut parsed = Err(());
        parse_stage(src, Stage::SitePlan, &mut parsed, &mut diags);
        site_plan = parsed.map(Some);
    }
    let mut detail_plan: Result<Option<Envelope<DetailPlanContent>>, ()> = Ok(None);
    if let Some(src) = &raw.detail_plan {
        let mut parsed = Err(());
        parse_stage(src, Stage::DetailPlan, &mut parsed, &mut diags);
        detail_plan = parsed.map(Some);
    }

    match (
        world,
        npcs,
        classes,
        quest_plan,
        quests,
        dialogue,
        world_edits,
        geometry_brief,
        layout_graph,
        site_plan,
        detail_plan,
    ) {
        (
            Ok(world),
            Ok(npcs),
            Ok(classes),
            Ok(quest_plan),
            Ok(quests),
            Ok(dialogue),
            Ok(world_edits),
            Ok(geometry_brief),
            Ok(layout_graph),
            Ok(site_plan),
            Ok(detail_plan),
        ) => {
            let mut campaign = Campaign {
                world,
                npcs,
                classes,
                quest_plan,
                quests,
                dialogue,
                world_edits,
                geometry_brief,
                layout_graph,
                site_plan,
                detail_plan,
            };
            // spec-0016 §3: expand the `ambush` sugar into real environment
            // triggers, ONCE, at the DSL boundary. Every downstream consumer —
            // validation, l10n, the flow producer scans, nav, emission — then
            // sees the same `triggers` list it always has, so the sugar has no
            // second code path to drift down and an ambush is exactly as
            // debuggable as the trigger an author would otherwise hand-write.
            campaign.quests.content.expand_ambushes();
            Ok(campaign)
        }
        _ => Err(diags),
    }
}

/// Parse then validate.
///
/// Convenience over [`parse_campaign`] and [`crate::validate::validate_campaign`]:
/// a document that does not parse yields its `DW0100` list; one that parses
/// yields every finding validation raises against it.
pub fn check_campaign(raw: &RawCampaign) -> Vec<Diagnostic> {
    match parse_campaign(raw) {
        Ok(campaign) => crate::validate::validate_campaign(&campaign),
        Err(diags) => diags,
    }
}
