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
pub mod chrome;
pub mod diagnostic;
pub mod effects;
pub mod envelope;
pub mod gate;
pub mod ids;
pub mod l10n;
pub mod mclang;
pub mod registry;
pub mod schema;
pub mod stages;
pub mod validate;

pub use canonical::to_canonical_string;
pub use chrome::{Chrome, ChromeString, validate_chrome_namespace};
pub use diagnostic::{Diagnostic, Severity, codes};
pub use effects::{
    EffectRootKind, EffectRootOwner, EffectRootSite, RootBinding, for_each_effect_root,
    for_each_effect_root_mut,
};
pub use envelope::{
    Campaign, Envelope, RawCampaign, SUPPORTED_DSL_VERSION, SUPPORTED_DSL_VERSIONS, Stage,
    check_campaign, is_supported_version, is_v03, is_v04, is_v05, is_v06, is_v07, is_v08, is_v09,
    is_v10, is_v11, parse_campaign,
};
pub use gate::{Gate, GateBinding, GateConsumer, GateSite, for_each_gate};
pub use ids::{
    ActorId, AmbushId, AnchorId, AreaId, BranchId, BranchPointId, CampaignId, ClassId, DialogueId,
    EditBatchId, EndingId, FlagId, LethalVolumeId, NpcId, ObjectiveId, PoolId, PrefabId, QuestId,
    RegionId, ShortcutId, StateId, TimedGateId, TriggerId, WaveId,
};
pub use l10n::{
    ArtNarrate, CANONICAL_LANG, L10nDoc, L10nKind, MARKER_SIGIL, OptionLabel, ScreenNarrate,
    SoundRef, TR_SIGIL, art_narrates, bonfire_option_labels, declared_mc_codes,
    dialogue_option_labels, each_string, has_tr_sigil, inventory as l10n_inventory, key_speaker,
    local_id, localize, on_screen_narrates, plain as l10n_plain, play_sound_actor_refs, sound_refs,
    tag_translatables, untag as l10n_untag, validate_l10n, validate_l10n_provenance,
    validate_marker_channel, validate_tr_sigil,
};
pub use mclang::mc_lang_code;
pub use registry::{
    AnchorRegistry, BlockRegistry, EffectRegistry, EntityRegistry, ItemBackedBlockRegistry,
    ItemRegistry, Lighting, LightingProfile, VendoredAnchorRegistry, VendoredEffectRegistry,
    VendoredEntityRegistry, VendoredItemRegistry, is_potion_id, is_technical_block,
};
pub use schema::stage_schema;
pub use stages::for_each_campaign_effect;
pub use stages::{
    Actor, Ambush, Area, AreaLighting, AreaMitigation, BONFIRE_PROMPT_EN, BONFIRE_REST_LABEL_EN,
    BONFIRE_SAVE_LABEL_EN, BonfireLabels, Boundary, BranchDecl, BranchPoint, CameraShot,
    CameraSubject, CameraTarget, CameraWaypoint, Carrier, CastAbsence, CastBarks, CastDialogue,
    CastDialogueKeyword, CastEntry, CastPlace, CastPlacement, Class, ClassesContent, CollectBy,
    CompareOp, DamageKind, DespawnStyle, DialogueContent, DialogueEffect, DialogueNode,
    DialogueOption, EffectSite, EnchantedItem, EncounterTier, EnvTrigger, EquipItem, EquipSlot,
    Facing, Fixture, Forfeit, Happening, HappeningVerb, Horizon, ItemDrop, KitItem, LethalVolume,
    Lethality, Loot, LootItem, MAX_POTION_AMPLIFIER, MAX_POTION_DURATION_TICKS, MobAttributes,
    MobDrop, MobEffect, MobEquipment, NarrateStyle, Npc, NpcDialogue, NpcSkin, NpcsContent,
    Objective, OnFull, Persona, Pieces, PlannedQuest, PotionContents, PotionEffect, Prop, Quest,
    QuestEffect, QuestPlanContent, QuestsContent, Relationship, Role, SequenceStep, Shop,
    ShopOffer, Shortcut, ShotStyle, SkinModel, SlotDrop, SoundAt, Stake, StateCompare, StateDecl,
    StateScope, StateWrite, StealthZone, TimedGate, Trap, TrapDisarm, TrapEffect, TrapReset,
    TrapTrigger, Trigger, TriggerAudience, TriggerOn, Wave, WaveLane, WaveMob, WaveSummon,
    WorldContent, WorldDifficulty, WorldTime, WorldWeather, is_potion_bearing_item,
};
pub use stages::{
    EditBatch, EditFrame, FragmentRotation, MorphOp, PaletteBlock, PaletteRecipe, RegionShape,
    SocketState, TreeKind, WorldEdit, WorldEditsContent,
};
pub use validate::{validate_campaign, validate_campaign_with};
