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
pub const SUPPORTED_DSL_VERSION: &str = "0.11.0";

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
/// from sibling PRs; v0.7 (spec-0020) adds the per-quest `cast` ledger; v0.8
/// (spec-0025) adds declared stage-4 `branch_points`, the per-node `happening`
/// declaration and the named `campaign-complete` `ending`, and (spec-0016 §1
/// owner rulings) the bonfire rest interaction — the `bonfire` effect's
/// authorable option strings and the class-kit `flask`; v0.9 adds
/// declared elite/boss `drops[]` and the `collect` `dropped_by`; v0.10
/// (spec-0031) adds **runtime state** — the stage-5 `state[]` declaration, the
/// `set-state`/`add-state`/`clear-state` verbs and the `requires_state` numeric
/// comparison on every gate consumer — the campaign-wide `on_death` effect
/// root, the bundle that runs at the moment a player dies, and the stage-5
/// `lethal_volumes` declaration; v0.11 adds two surfaces and one obligation —
/// (spec-0034) the per-body `traversal` declaration, what a body can do when it
/// moves, on the stage-2 NPC and the stage-5 actor; the **press-answer lift**, a
/// `narrate` `actionbar` style and a trigger `audience: presser`; and with the
/// lift the one obligation of the version, `DW0429`.
/// Older campaigns remain valid and compile byte-identically. A construct
/// introduced in a later version is rejected with `DW0141` in an earlier one.
pub const SUPPORTED_DSL_VERSIONS: &[&str] = &[
    "0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0", "0.7.0", "0.8.0", "0.9.0", "0.10.0", "0.11.0",
];

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
///
/// Public as [`minor_ordinal`]: the obligation fence ([`crate::fence`]) compares
/// a rule's [`Binds::Since`](crate::Binds::Since) against exactly this number, so
/// "version 0.8.0" means the same thing to a fence as it does to `is_v08`.
fn ordinal(version: &str) -> u32 {
    match version {
        "0.2.0" => 2,
        "0.3.0" => 3,
        "0.4.0" => 4,
        "0.5.0" => 5,
        "0.6.0" => 6,
        "0.7.0" => 7,
        "0.8.0" => 8,
        "0.9.0" => 9,
        "0.10.0" => 10,
        "0.11.0" => 11,
        _ => 0,
    }
}

/// The minor-version ordinal of a supported `dsl_version` (`0.8.0` → `8`); `0`
/// for anything this crate does not accept. The number every `is_v0*` predicate
/// below compares, and the number [`crate::fence`] grandfathers against.
pub fn minor_ordinal(version: &str) -> u32 {
    ordinal(version)
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

/// True if `version` enables the DSL v0.8 surface. Two specs land in it:
/// spec-0025's stage-4 `branch_points` declaration, per-node `happening` and
/// named `ending` on `campaign-complete`; and spec-0016 §1's bonfire **rest
/// interaction** — the `bonfire` effect's authorable
/// `prompt` / `rest_label` / `save_label` strings and the stage-3 kit item
/// `flask` marker a rest replenishes. Additive over v0.7 — a campaign that
/// declares none of it compiles byte-identically, and any use of the surface in
/// an earlier campaign is rejected with `DW0141`. The **requirement** side (a
/// story node without a `happening` is `DW0481`; an undeclared fork is `DW0480`)
/// fires only at 0.8.0 and above, which is what keeps every 0.6/0.7 campaign's
/// datapack byte-for-byte unchanged.
pub fn is_v08(version: &str) -> bool {
    ordinal(version) >= 8
}

/// True if `version` enables the DSL v0.9 surface: declared **drops** on an
/// elite/boss — the `drops[]` list on a
/// wave mob and on an actor, and the `collect` `dropped_by` that turns a boss's
/// quest token into a proved link in the quest graph. Additive over v0.8: a
/// campaign that declares none of it compiles byte-identically (every
/// undeclared slot keeps drop chance `0.0`, which is exactly what pre-0.9
/// emission wrote), and any use of the surface in an earlier campaign is
/// rejected with `DW0141`.
pub fn is_v09(version: &str) -> bool {
    ordinal(version) >= 9
}

/// True if `version` enables the DSL v0.10 surface. **Two spec-0031 surfaces
/// land in it**, and they are additive over v0.9 and over each other:
///
/// * §"the missing primitive" — **runtime state**: the stage-5 `state[]`
///   declaration of named, scoped, integer-valued data; the `set-state` /
///   `add-state` / `clear-state` verbs that write one; and `requires_state`, the
///   numeric comparison carried by **every** gate consumer beside
///   `requires_flags` / `forbids_flags`. The datum is what `FlagId` is not: it
///   clears, it counts, and its multiplayer scope is declared rather than
///   assumed. The comparison lives in the gate and not in any one verb, because
///   its consumers are exactly the gate's consumers — a door that opens at 500,
///   a line withheld below 200, a lever inert while a ride is in progress.
///   Generality is decided at the first site (CLAUDE.md); a second bespoke field
///   would be the defect, not the fix.
/// * §`on_death` — the stage-5 campaign-wide `on_death` bundle: the seventh
///   effect root, and the only one that runs while the player who fired it is
///   still a corpse.
/// * §"Lethal volume" — the stage-5 `lethal_volumes[]` declaration: a box that
///   kills whatever enters it, worded by the campaign's own strings. Geometry,
///   so the completability model owns it (`DW0510`/`DW0511`); an ordinary
///   `/damage`, so the death edge above needs no second detector.
///
/// A campaign that declares none of it compiles byte-identically (no new scoreboard
/// objective, no new guard clause, no new function, and the whole
/// `dw.death_seen` half of the death edge absent; no lethal tick call and no
/// navigation cell), and any use of any of it in an earlier campaign is rejected
/// with `DW0141`.
///
/// **The version is `0.10.0`, not `0.9.1`.** `ordinal` matches the literal
/// string, so the ledger is a sequence of minors and a patch would sort nowhere.
pub fn is_v10(version: &str) -> bool {
    ordinal(version) >= 10
}

/// True if `version` enables the DSL v0.11 surface. **Two surfaces land in this
/// version and one obligation rides with them**, and one predicate carries all
/// three.
///
/// # The per-body `traversal` declaration (spec-0034)
///
/// What a body can do when it moves, carried by the stage-2 NPC and the stage-5
/// actor through one shared [`crate::stages::BodyTraversal`] type.
///
/// Spiders really do climb, so the traversal proof's rules cannot be absolute;
/// what was missing was the author's side of that. A declaration is not an
/// exemption: the compiler compares the verdicts the body earns under the
/// declared class against the ones it earns under its species' derived class,
/// and a declaration that changes none of them is `DW0454`. It can never reach
/// the error tier (`DW0452`), because that rule is a collision-and-interaction
/// question with no authorable exemption.
///
/// Purely additive: nothing obliges a body to declare traversal, a campaign that
/// declares none routes exactly as it did before (the derived class is what
/// every pre-0.11 build used), and declaring it in an earlier campaign is
/// rejected with `DW0141`.
///
/// # The press-answer lift
///
/// Two additions, and they are one lift — each alone leaves the general
/// mechanism unable to say what `close-gate`'s private copy said:
///
/// * `narrate` gains the **`actionbar`** style — the reply strip every string the
///   compiler writes itself already used, and the one channel the general effect
///   could not reach;
/// * an environment trigger gains **`audience: presser`** — dispatch by the
///   `player_interacted_with_entity` advancement, so the bundle runs as the
///   player who right-clicked instead of addressing the whole party.
///
/// With both, "a pressable thing answers the player who pressed it" is an
/// ordinary trigger with an ordinary effect, and `close-gate.sealed_hint` stops
/// being a mechanism and becomes what it always was — a wording. Additive: a
/// campaign that declares neither keeps every verdict and every line it showed,
/// and any use of either below 0.11.0 is `DW0141`.
///
/// The surface is additive; the version also carries **one requirement**, and it
/// is fenced rather than reserved. At 0.11.0 and above a sealed body nothing
/// answers is `DW0429` — a `shortcut` door and a `close-gate` wall alike, one
/// rule over the class. That obligation declares itself on its own code
/// ([`crate::Binds::Since`] 11) and is carried by [`crate::fence`], so a campaign
/// below 0.11.0 is grandfathered: its sealed gate answers exactly as it did
/// before, and its silent door stays silent.
///
/// # What "additive" does and does not promise
///
/// A campaign that declares none of the new surface keeps every verdict and
/// every player-facing string it had. It does **not** follow that its datapack
/// is byte-identical: a `close-gate` seal's answer is now emitted through the
/// general trigger path rather than through a private one, so the set of emitted
/// files and identifiers moves even where the line the player reads does not.
/// The fence grandfathers the verdict and the wording; it does not grandfather
/// emitted identifiers. Reproduction of a released delve is the pinned engine's
/// job (`versions.toml` + OCI), not eternal byte-stable emission.
pub fn is_v11(version: &str) -> bool {
    ordinal(version) >= 11
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

/// Parse then validate, **through the obligation fence**.
///
/// Convenience over [`parse_campaign`], [`crate::validate::validate_campaign`]
/// and [`crate::fence::Fenced`], and the fence is not optional here: a caller with
/// a raw document in hand has no campaign to fence against afterwards, so an
/// unfenced list handed out from this function is one nothing downstream could
/// correct. Every diagnostic returned is one the campaign's own declared
/// `dsl_version` makes it answerable for; a [`crate::Binds::Since`] rule raised
/// against a stage below its version is grandfathered and never appears.
///
/// A document that does not parse cannot be fenced — there is no declared
/// version to read — so that path takes [`crate::fence::Fenced::structural`],
/// which refuses to carry anything version-scoped.
pub fn check_campaign(raw: &RawCampaign) -> Vec<Diagnostic> {
    match parse_campaign(raw) {
        Ok(campaign) => {
            let diags = crate::validate::validate_campaign(&campaign);
            crate::fence::Fenced::apply(&campaign, diags)
                .reported()
                .to_vec()
        }
        Err(diags) => crate::fence::Fenced::structural(diags).reported().to_vec(),
    }
}
