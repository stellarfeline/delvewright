//! The stage envelope, the assembled [`Campaign`], and parsing from raw JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::ids::CampaignId;
use crate::stages::{
    ClassesContent, DialogueContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
    WorldEditsContent,
};

/// The latest `dsl_version` this crate implements (identity / tooling default).
pub const SUPPORTED_DSL_VERSION: &str = "0.7.0";

/// Every `dsl_version` this crate accepts. Each version is an **additive
/// superset** of the previous: v0.3 added the stage-5 verbs/waves/flags; v0.4
/// (spec-0008) adds dialogue state, props, narration, live-threat tuning, NPC
/// lifecycle + skins, environment triggers, cutscenes and named given items;
/// v0.5 (spec-0010) adds declared world `time`/`weather`, per-area `lighting`
/// (deterministic relight), and the `set-time`/`set-weather` effect verbs; v0.6
/// (spec-0012/spec-0013/spec-0014) adds checkpoints (`set-checkpoint` +
/// `on_respawn`), the stealth-zone verbs (`begin-stealth`/`end-stealth`), the
/// stage-1 `horizon` (`ocean` backdrop) + `boundary` (playable region +
/// return-to-checkpoint enforcement), and the staging surface — the `play-sound`
/// effect and the `narrate` `art` style — alongside the actors/sequence surface
/// from sibling PRs; v0.7 (spec-0020) adds the per-quest `cast` ledger.
/// Older campaigns remain valid and compile byte-identically. A construct
/// introduced in a later version is rejected with `DW0141` in an earlier one.
pub const SUPPORTED_DSL_VERSIONS: &[&str] = &["0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0", "0.7.0"];

/// True if `version` is a `dsl_version` this crate accepts.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_DSL_VERSIONS.contains(&version)
}

/// The minor-version ordinal of a supported `dsl_version` (`0.4.0` → 4); `0` for
/// anything this crate does not accept.
///
/// Every version predicate below is `ordinal(version) >= n`, so adding a version
/// to [`SUPPORTED_DSL_VERSIONS`] is the *only* edit a version bump needs. The
/// hand-written `version == "0.4.0" || version == "0.5.0" || …` chains this
/// replaced had to be extended in lockstep in five places; forgetting one made
/// the newest campaigns silently lose an older version's surface.
fn ordinal(version: &str) -> u32 {
    match version {
        "0.2.0" => 2,
        "0.3.0" => 3,
        "0.4.0" => 4,
        "0.5.0" => 5,
        "0.6.0" => 6,
        "0.7.0" => 7,
        _ => 0,
    }
}

/// True if `version` enables the DSL v0.3 verbs (`kill`/`collect`/`interact`,
/// `give-item`/`set-flag`/`spawn-wave`, waves, flags). v0.4 is an additive
/// superset, so it enables the whole v0.3 surface too.
pub fn is_v03(version: &str) -> bool {
    ordinal(version) >= 3
}

/// True if `version` enables the DSL v0.4 surface (spec-0008): dialogue
/// `set-flag` + `requires_flags`, props (`interact.prop`, `set-block`),
/// `narrate`, wave `attributes`/`effects`, `despawn-npc`/`move-npc`, `cutscene`,
/// NPC `skin`, stage-5 `triggers`, named `give-item`, objective `stealth`.
pub fn is_v04(version: &str) -> bool {
    ordinal(version) >= 4
}

/// True if `version` enables the DSL v0.5 surface (spec-0010): declared world
/// `time`/`weather`, per-area `lighting` (deterministic relight fixtures), and
/// the `set-time`/`set-weather` effect verbs. v0.6 is an additive superset.
pub fn is_v05(version: &str) -> bool {
    ordinal(version) >= 5
}

/// True if `version` enables the DSL v0.6 surface: the `set-checkpoint` effect
/// (with its optional `on_respawn` hook) and the `begin-stealth`/`end-stealth`
/// verbs (spec-0012 checkpoints + spec-0014 stealth zones), the stage-1 `horizon`
/// (`ocean`) and `boundary` (playable region) world fields (spec-0013), the
/// `play-sound` effect + `narrate` `art` style (spec-0014), and the stage-5
/// scripted `actors` + staging effects (`spawn`/`despawn`/`move`/`unleash-actor`,
/// `sequence`, spec-0014). Additive over v0.5; a campaign that uses none is
/// byte-identical, and a use of the v0.6 surface in an earlier campaign is
/// rejected with `DW0141`.
pub fn is_v06(version: &str) -> bool {
    ordinal(version) >= 6
}

/// True if `version` enables the DSL v0.7 surface (spec-0020): the per-quest
/// `cast` ledger — for every live stage-2 NPC, where it stands, what it is doing,
/// and what its right-click offers during that quest. Additive over v0.6: a
/// campaign that declares no `cast` compiles byte-identically, and a pre-0.7
/// campaign that declares none keeps building with the `DW0465` deprecation
/// warning for one version window.
pub fn is_v07(version: &str) -> bool {
    ordinal(version) >= 7
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
    /// Stage 7 (optional; DSL v0.6, spec-0017): the map-editor edit script.
    WorldEdits,
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
                     JSON to match the schema — run `delvec schema --stage <1..7>` to see the \
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
    // The optional stage-7 edit script (spec-0017): absent = `None`; present but
    // unparseable = a `DW0100` like any other stage.
    let mut world_edits: Result<Option<Envelope<WorldEditsContent>>, ()> = Ok(None);
    if let Some(src) = &raw.world_edits {
        let mut parsed = Err(());
        parse_stage(src, Stage::WorldEdits, &mut parsed, &mut diags);
        world_edits = parsed.map(Some);
    }

    match (
        world,
        npcs,
        classes,
        quest_plan,
        quests,
        dialogue,
        world_edits,
    ) {
        (
            Ok(world),
            Ok(npcs),
            Ok(classes),
            Ok(quest_plan),
            Ok(quests),
            Ok(dialogue),
            Ok(world_edits),
        ) => {
            let mut campaign = Campaign {
                world,
                npcs,
                classes,
                quest_plan,
                quests,
                dialogue,
                world_edits,
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

/// Parse then validate. Convenience over [`parse_campaign`] +
/// [`crate::validate::validate_campaign`].
pub fn check_campaign(raw: &RawCampaign) -> Vec<Diagnostic> {
    match parse_campaign(raw) {
        Ok(campaign) => crate::validate::validate_campaign(&campaign),
        Err(diags) => diags,
    }
}
