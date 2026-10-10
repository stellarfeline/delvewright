//! Waves: the mobs a fight spawns, their lanes, equipment, drops and attributes.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::is_false;
use crate::{AnchorId, OnKill, WaveId};

/// A combat wave (DSL v0.3): a bundle of mobs spawned at an anchor and slain to
/// complete a `kill` objective. Emission (spec-0002): a `spawn-wave` effect
/// summons the mobs tagged `dw_wave_<id>` (AI enabled — they fight); a
/// `player_killed_entity` advancement per tag decrements a scoreboard countdown,
/// and the `kill` objective completes when the count reaches zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wave {
    /// Unique wave id.
    pub id: WaveId,
    /// The anchor the wave's mobs spawn at.
    pub anchor: AnchorId,
    /// The mobs that make up the wave (1..N).
    pub mobs: Vec<WaveMob>,
    /// Re-seat this wave every time the party rests at (or respawns from) a
    /// bonfire (spec-0016 §1) — the souls contract: progress is kept, the
    /// enemies come back. The compiler kills any survivor carrying the wave tag
    /// and re-runs the wave's own spawn function, so the room is restored to its
    /// authored composition and spawn cells.
    ///
    /// Inert without a `bonfire` in the campaign, which is a compile error
    /// (`DW0370`) rather than a silent no-op.
    #[serde(default, skip_serializing_if = "is_false")]
    pub respawns_on_rest: bool,
    /// Tower-defense lane routing (spec-0016 §6): march this wave along a
    /// waypoint polyline while distant, hand it to native AI the instant a
    /// player is inside `aggro_radius`. Absent = today's behaviour (spawn and
    /// stand), byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<WaveLane>,
    /// Where the wave's mobs materialize (spec-0016 §6). Absent =
    /// [`WaveSummon::Anchor`], the pre-0.6 behaviour: standable cells around the
    /// wave `anchor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summon: Option<WaveSummon>,
    /// How hard this encounter is *meant* to be (DSL v0.7, spec-0023). Absent =
    /// [`EncounterTier::Ordinary`], byte-identical to every pre-0.7 campaign.
    ///
    /// This is a **declaration, not a knob**: the compiler never scales content
    /// from it (spec-0023 "Out of scope"). It exists because the validation
    /// ladder's inverted floor gate needs to know which fights the content
    /// *claims* are hard — an `elite`/`boss` encounter the unassisted bot beats
    /// on its first attempt is reported as too easy for its billing. Marking it
    /// is how the author opts into that scrutiny; the alternative — inferring
    /// "elite" from how tuned a stack looks — is exactly the downstream folklore
    /// CLAUDE.md's no-hack rule forbids.
    ///
    /// **It does reach emission in exactly one place** (spec-0016 §1): in a
    /// campaign with a `bonfire`, a billed `elite`/
    /// `boss` wave that does not declare `respawns_on_rest` is refreshed by a
    /// rest *while it is still standing* — deleted and re-seated at full count
    /// and full health, so chipping it down one life at a time is never a path.
    /// Beat it and it stays beaten. `DW0499` forbids billing a wave `boss` and
    /// `respawns_on_rest` at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<EncounterTier>,
    /// A health bar over this wave's bodies (DSL v0.31, spec-0073): a named bar
    /// over their total health, drawn for every player within `range` blocks of a
    /// live one. Absent = no bar, byte-identical. Declared, never derived from
    /// `tier`; a `boss`-billed wave without one is advised (`DW0912`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_bar: Option<crate::healthbar::HealthBar>,
    /// What happens each time a player is credited with killing one of this
    /// wave's bodies (spec-0074) — effect root R9, the same
    /// [`OnKill`] an actor declares. Absent = no bundle, and the
    /// wave's emission is byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_kill: Option<OnKill>,
}

/// What a wave is billed as (DSL v0.7, spec-0023). Consumed by the validation
/// ladder (the run's combat plan) and — since spec-0016 §1's undefeated re-seat
/// — by one emission site: a bonfire refreshes a billed wave that is still
/// standing. Nothing about the encounter itself is scaled from it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EncounterTier {
    /// Trash / pressure. No floor expectation: a bot that wins it cold proves
    /// nothing either way. The default.
    #[default]
    Ordinary,
    /// A set-piece the content bills as a hard fight (spec-0016's optional-elite
    /// and spawn-and-unleash vocabulary).
    Elite,
    /// A campaign's named fight. Same floor rule as `elite`; the distinction is
    /// for the run report a human reads.
    Boss,
}

impl EncounterTier {
    /// The kebab tag, as it appears in the DSL and in the emitted combat plan.
    pub fn token(self) -> &'static str {
        match self {
            EncounterTier::Ordinary => "ordinary",
            EncounterTier::Elite => "elite",
            EncounterTier::Boss => "boss",
        }
    }

    /// Does the inverted floor gate (spec-0023) apply to this tier? A fight the
    /// content bills as hard carries an expectation the bot can measure; an
    /// ordinary one does not.
    pub fn has_floor_expectation(self) -> bool {
        matches!(self, EncounterTier::Elite | EncounterTier::Boss)
    }
}

/// Where a wave's mobs materialize (spec-0016 §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WaveSummon {
    /// Standable cells around the wave `anchor`, nearest first — the pre-0.6
    /// behaviour and the default.
    Anchor,
    /// **Spirit-summoned at the edge of perception**:
    /// each mob appears on the ring at its own `attributes.follow_range` from
    /// the wave `anchor`, so it acquires a target the instant it exists and
    /// closes under pure native AI. Species without patrol AI never march a
    /// lane; this is what they do instead — never a spawn on top of the party,
    /// never a mob that brushes past.
    ///
    /// With this mode the wave `anchor` is the **defended point** (what the ring
    /// is drawn around), not the spawn point. Each mob stack must declare its
    /// own `attributes.follow_range` (`DW0385`) — the ring radius is authored,
    /// never guessed from a vanilla defaults table the compiler cannot verify.
    AggroEdge,
}

/// Tower-defense lane routing for a wave (spec-0016 §6), built on vanilla's
/// **Raider patrol system** — the intended primitive, live-verified on 1.21.11
/// (`docs/notes/td-routing-spike.md`).
///
/// The squad spawns `Patrolling:1b` with one `PatrolLeader:1b` and a snake_case
/// `patrol_target` int-array; a compiler-emitted clock walks the shared waypoint
/// index forward, and per mob a player-proximity check releases `Patrolling:0b`.
/// From that instant the mob is a plain native hostile. "Combat preempts
/// routing" is engine semantics — vanilla's patrol goal is hard-gated on having
/// no target — so the owner's rule (march while distant, fight with NATIVE AI
/// once aggroed, never brush past) falls out of the primitive unforced.
///
/// Lanes are raider-family only (`DW0382`), squad ≥ 2 (`DW0383`), and a lane
/// pillager must keep its crossbow (`DW0384`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaveLane {
    /// The lane polyline, in march order: anchors some area's prefab provides.
    /// At least one; consecutive legs (counting the wave `anchor` as the origin)
    /// must be more than 10 blocks apart — vanilla re-rolls a patrol target
    /// within 10 blocks of arrival, so a tighter lane is a lane the engine
    /// quietly stops following (`DW0386`).
    pub waypoints: Vec<AnchorId>,
    /// The release radius, in blocks: the compiler sets every lane mob's
    /// `follow_range` attribute to exactly this and releases `Patrolling:0b`
    /// at the same distance. The two MUST be equal — a patrolling raider that
    /// targets a player it cannot engage holds ground instead of marching, so a
    /// per-mob `attributes.follow_range` that disagrees is `DW0381`.
    pub aggro_radius: u32,
}

/// One mob stack in a [`Wave`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaveMob {
    /// Vanilla entity id, validated against the pinned 1.21.11 registry.
    pub entity: String,
    /// How many to spawn.
    pub count: u32,
    /// Optional custom name (shown above the mob).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional attribute overrides (DSL v0.4), emitted as 1.21.11 attribute
    /// components. Enables e.g. a weakened live warden as a survivable stealth
    /// threat. Omitted = vanilla defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<MobAttributes>,
    /// Optional permanent, ambient status effects (DSL v0.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<MobEffect>,
    /// Optional worn/held equipment (DSL v0.6). A helmet is the
    /// sanctioned fix for daylight-burning undead — never
    /// `set-time`. Item ids validate against the pinned 1.21.11 item registry
    /// (`DW0143`, the give-item family); every emitted slot carries drop
    /// chance 0 so players can never farm wave gear (no-grind constitution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<MobEquipment>,
    /// What this mob leaves behind when it dies (DSL v0.9). Only an `elite`/`boss` wave may declare it (`DW0491`)
    /// — an ordinary mob's kit is never farmable. Empty = drop chance 0 on
    /// every slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drops: Vec<MobDrop>,
}

/// Worn/held equipment for a wave mob (DSL v0.6). Each field is a vanilla item
/// id for the matching vanilla equipment slot; an unset slot stays empty —
/// except `main_hand`, where the compiler's armed-mob default (skeleton bow,
/// wither-skeleton sword) still applies unless overridden. Emitted as the
/// component-era `equipment`/`drop_chances` summon NBT (1.21.11 silently
/// ignores legacy `ArmorItems`/`HandItems` on `/summon`), all drop chances 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MobEquipment {
    /// Head slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<EquipItem>,
    /// Chest slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chest: Option<EquipItem>,
    /// Legs slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<EquipItem>,
    /// Feet slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feet: Option<EquipItem>,
    /// Main-hand slot. Overrides the compiler's armed-mob default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_hand: Option<EquipItem>,
    /// Off-hand slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_hand: Option<EquipItem>,
    /// Body slot: horse armour, wolf armour, a llama's carpet, a nautilus's
    /// armour, a happy ghast's harness (spec-0067). Shown only on a body whose
    /// entity type draws it (`DW0898`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<EquipItem>,
    /// Saddle slot (spec-0067). Shown only on a body whose entity type draws a
    /// saddle (`DW0898`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saddle: Option<EquipItem>,
}

impl MobEquipment {
    /// Every slot as `(dsl_field_name, piece)`, in [`EquipSlot::ALL`]'s order —
    /// the single iteration source for validation paths and emission.
    pub fn slots(&self) -> [(&'static str, Option<&EquipItem>); EquipSlot::ALL.len()] {
        EquipSlot::ALL.map(|s| (s.field(), self.filled(s)))
    }

    /// Every slot as `(slot, piece)`, in [`EquipSlot::ALL`]'s order.
    pub fn pieces(&self) -> [(EquipSlot, Option<&EquipItem>); EquipSlot::ALL.len()] {
        EquipSlot::ALL.map(|s| (s, self.filled(s)))
    }

    /// The piece this equipment declaration puts in `slot`, if any. The single
    /// question a `drops[]` `slot` entry asks: a mob can only drop a piece it
    /// actually wears (`DW0490`).
    pub fn filled(&self, slot: EquipSlot) -> Option<&EquipItem> {
        match slot {
            EquipSlot::Head => self.head.as_ref(),
            EquipSlot::Chest => self.chest.as_ref(),
            EquipSlot::Legs => self.legs.as_ref(),
            EquipSlot::Feet => self.feet.as_ref(),
            EquipSlot::MainHand => self.main_hand.as_ref(),
            EquipSlot::OffHand => self.off_hand.as_ref(),
            EquipSlot::Body => self.body.as_ref(),
            EquipSlot::Saddle => self.saddle.as_ref(),
        }
    }
}

/// One vanilla equipment slot, named exactly as the [`MobEquipment`] field that
/// fills it. The DSL name and the summon-NBT key differ (`main_hand` vs
/// `mainhand`), so both live here and nowhere else.
///
/// **The set is the pinned game's equipment-slot set** (spec-0067 §2), a
/// [`crate::metrics::Provenance::VanillaRule`]: the eight values the
/// `minecraft:equippable` component's `slot` field takes, per the Minecraft
/// Wiki page *Data component format/equippable* for Java 1.21.11, which are the
/// serialised names of the client's `EquipmentSlot` enum. The pinned item data
/// is the cross-check, not the source: every `slot` value an item declares is
/// asserted to be one of these, and no item declares `mainhand`, because a hand
/// takes anything.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlot {
    /// Head slot.
    Head,
    /// Chest slot.
    Chest,
    /// Legs slot.
    Legs,
    /// Feet slot.
    Feet,
    /// Main-hand slot.
    MainHand,
    /// Off-hand slot.
    OffHand,
    /// Body slot (horse armour, wolf armour, carpet, harness).
    Body,
    /// Saddle slot.
    Saddle,
}

impl EquipSlot {
    /// Every slot, in emission order: the two hands, the four armour slots,
    /// then `body` and `saddle`. The one enumeration every slot list derives
    /// from — `MobEquipment::slots()`, the summon `equipment` / `drop_chances`
    /// compounds and the drop-strip line.
    pub const ALL: [EquipSlot; 8] = [
        EquipSlot::MainHand,
        EquipSlot::OffHand,
        EquipSlot::Head,
        EquipSlot::Chest,
        EquipSlot::Legs,
        EquipSlot::Feet,
        EquipSlot::Body,
        EquipSlot::Saddle,
    ];

    /// The DSL field name (`main_hand`), for diagnostics and JSON pointers.
    pub fn field(self) -> &'static str {
        match self {
            EquipSlot::Head => "head",
            EquipSlot::Chest => "chest",
            EquipSlot::Legs => "legs",
            EquipSlot::Feet => "feet",
            EquipSlot::MainHand => "main_hand",
            EquipSlot::OffHand => "off_hand",
            EquipSlot::Body => "body",
            EquipSlot::Saddle => "saddle",
        }
    }

    /// The 1.21.11 `equipment` / `drop_chances` NBT key (`mainhand`) — also the
    /// game's own name for the slot, as an `equippable` component spells it.
    pub fn nbt(self) -> &'static str {
        match self {
            EquipSlot::Head => "head",
            EquipSlot::Chest => "chest",
            EquipSlot::Legs => "legs",
            EquipSlot::Feet => "feet",
            EquipSlot::MainHand => "mainhand",
            EquipSlot::OffHand => "offhand",
            EquipSlot::Body => "body",
            EquipSlot::Saddle => "saddle",
        }
    }

    /// The slot the game names `name` (`mainhand`), if it is one.
    pub fn from_nbt(name: &str) -> Option<EquipSlot> {
        EquipSlot::ALL.into_iter().find(|s| s.nbt() == name)
    }

    /// Whether this is a hand: a hand takes any item, so an item's own declared
    /// slot never contradicts it.
    pub fn is_hand(self) -> bool {
        matches!(self, EquipSlot::MainHand | EquipSlot::OffHand)
    }
}

/// One declared drop of an elite or boss (DSL v0.9).
///
/// A mob may wear many pieces; what it *leaves behind* is a **declared subset**,
/// usually one piece and never automatically everything. Two forms, told apart
/// by which field is present:
///
/// ```json
/// { "slot": "main_hand" }                                  // its axe
/// { "item": "minecraft:tripwire_hook", "name": "Gate Key" } // a quest item
/// ```
///
/// A `slot` entry must name a slot the same entity's `equipment` really fills
/// (`DW0490`) — the drop is the piece the player has been *looking at* through
/// the whole fight, so the two declarations cannot disagree. An `item` entry is
/// a token the fight *yields* rather than wears, and rides the entity's own
/// death loot table.
///
/// Undeclared slots keep drop chance `0.0` — byte-for-byte today's behaviour, and
/// the no-grind constitution's guarantee that an ordinary kit is never farmable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum MobDrop {
    /// A worn/held piece, named by its equipment slot.
    Slot(SlotDrop),
    /// A quest token the fight yields (not worn).
    Item(ItemDrop),
}

/// The worn-piece form of a [`MobDrop`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlotDrop {
    /// The equipment slot whose piece drops. Must be filled by the entity's own
    /// `equipment` declaration (`DW0490`).
    pub slot: EquipSlot,
}

/// The quest-token form of a [`MobDrop`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemDrop {
    /// Vanilla item id, validated against the pinned 1.21.11 registry
    /// (`DW0143`, the give-item family).
    pub item: String,
    /// Display name the dropped stack carries as a `custom_name` component.
    /// Player-visible, so it enters the l10n string inventory and translates
    /// like any other line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl MobDrop {
    /// The equipment slot this entry names, or `None` for a quest-item drop.
    pub fn slot(&self) -> Option<EquipSlot> {
        match self {
            MobDrop::Slot(s) => Some(s.slot),
            MobDrop::Item(_) => None,
        }
    }

    /// The item id this entry names, or `None` for a worn-piece drop.
    pub fn item(&self) -> Option<&str> {
        match self {
            MobDrop::Slot(_) => None,
            MobDrop::Item(i) => Some(&i.item),
        }
    }

    /// The declared display name of a quest-item drop, when set.
    pub fn name(&self) -> Option<&String> {
        match self {
            MobDrop::Slot(_) => None,
            MobDrop::Item(i) => i.name.as_ref(),
        }
    }

    /// Mutable access to the display name — the l10n traversal's hook.
    pub fn name_mut(&mut self) -> Option<&mut String> {
        match self {
            MobDrop::Slot(_) => None,
            MobDrop::Item(i) => i.name.as_mut(),
        }
    }
}

/// One equipped item: either a bare item id, or an id carrying enchantments.
///
/// The plain form is the common case and stays a plain JSON string, which is
/// what keeps every campaign written before enchantments existed byte-identical
/// on re-serialisation:
///
/// ```json
/// "main_hand": "minecraft:netherite_sword"
/// "head": { "item": "minecraft:netherite_helmet",
///           "enchantments": { "minecraft:protection": 4 } }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EquipItem {
    /// A bare item id — no enchantments.
    Plain(String),
    /// An item id plus its enchantments.
    Enchanted(EnchantedItem),
}

impl EquipItem {
    /// The item id, whichever form was authored.
    pub fn item(&self) -> &str {
        match self {
            EquipItem::Plain(s) => s,
            EquipItem::Enchanted(e) => &e.item,
        }
    }

    /// The enchantments on this piece — empty for the plain form. `BTreeMap`
    /// ordered, so emission order is the id order and never hash order
    /// (ADR-0006).
    pub fn enchantments(&self) -> &BTreeMap<String, u32> {
        static EMPTY: std::sync::LazyLock<BTreeMap<String, u32>> =
            std::sync::LazyLock::new(BTreeMap::new);
        match self {
            EquipItem::Plain(_) => &EMPTY,
            EquipItem::Enchanted(e) => &e.enchantments,
        }
    }
}

/// **The item component a stack's enchantments are written to** — vanilla's own
/// rule (`EnchantmentHelper.getComponentType` at 1.21.11): an enchanted book
/// STORES its enchantments (`minecraft:stored_enchantments`, what an anvil
/// applies to the item it is combined with), and every other item CARRIES them
/// (`minecraft:enchantments`). Both are in the pinned 1.21.11
/// `data_component_type` registry.
///
/// A property of the item, not of the surface that writes the stack: a book in
/// a `loot` chest, a book handed over by `give-item` and a book on an equipped
/// piece are one object, so every emitter asks this one function. An
/// enchantment map written to the other component is a book that glints and
/// an anvil ignores, which the game accepts without a word.
pub fn enchantment_component(item: &str) -> &'static str {
    match item.strip_prefix("minecraft:").unwrap_or(item) {
        "enchanted_book" => "minecraft:stored_enchantments",
        _ => "minecraft:enchantments",
    }
}

/// The enchanted form of [`EquipItem`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnchantedItem {
    /// Item id (e.g. `minecraft:netherite_chestplate`).
    pub item: String,
    /// Enchantment id → level (e.g. `{"minecraft:protection": 4}`). Emitted as
    /// the 1.21 `minecraft:enchantments` item component.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub enchantments: BTreeMap<String, u32>,
}

/// Attribute overrides for a wave mob (DSL v0.4). Each field maps to a 1.21.11
/// `minecraft:` attribute component; an unset field keeps the vanilla base value.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MobAttributes {
    /// `minecraft:max_health` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_health: Option<f64>,
    /// `minecraft:attack_damage` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_damage: Option<f64>,
    /// `minecraft:movement_speed` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement_speed: Option<f64>,
    /// `minecraft:follow_range` base value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_range: Option<f64>,
}

/// One permanent status effect on a wave mob (DSL v0.4), emitted as an ambient,
/// non-expiring `effect give`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MobEffect {
    /// Vanilla effect id (e.g. `minecraft:slowness`), validated against the
    /// pinned registry (`DW0192`).
    pub effect: String,
    /// Amplifier (0 = level I).
    pub amplifier: u32,
}

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::Objective;
use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::{AnchorRegistry, EffectRegistry, EntityRegistry, ItemRegistry};
use crate::validate::{AnchorProviders, declares_bonfire, quest_ancestors, station_kind_diag};
use std::collections::BTreeSet;

crate::dw_code! {
    /// (spec-0016 §1) A wave declares `respawns_on_rest: true` but the campaign
    /// declares no `bonfire` — nothing can ever re-seat it, so the field is a
    /// silent no-op. Either add the bonfire the re-seat is meant to hang off, or
    /// drop the field.
    pub const REST_RESEAT_NO_BONFIRE: DwCode = DwCode::new("DW0370", ExitTier::Build);
}

crate::dw_code! {
    /// A `drops[]` `slot` entry does not
    /// name a distinct slot the same entity's `equipment` actually fills — the
    /// slot is empty, or the same slot is declared twice. A mob can only leave
    /// behind a piece it wears, and it can only leave it behind once.
    pub const DROP_SLOT_UNFILLED: DwCode = DwCode::new("DW0490", ExitTier::Build);
}

crate::dw_code! {
    /// `drops[]` on an encounter that is
    /// not billed `elite` or `boss`. Only a named fight leaves anything behind;
    /// an ordinary mob's kit is never farmable (no-grind constitution), so the
    /// declaration is refused rather than silently making rank-and-file gear
    /// lootable.
    pub const DROP_NOT_TIERED: DwCode = DwCode::new("DW0491", ExitTier::Build);
}

crate::dw_code! {
    /// A `collect` `dropped_by` is not backed by the wave it names:
    /// the wave declares no `{item}` drop of this objective's item, the count
    /// asks for more copies than the wave's mobs can yield, or the objective
    /// also declares a `container` (the item cannot come out of a box *and* off
    /// a body).
    pub const DROP_COLLECT_UNSOURCED: DwCode = DwCode::new("DW0492", ExitTier::Build);
}

crate::dw_code! {
    /// A `collect` `dropped_by` is not ordered after the fight that
    /// produces it: no `kill` objective for that wave precedes this collect in
    /// the objective graph. Without that edge "kill the boss, take its key" is
    /// an authoring intention the quest graph cannot prove, and the collect
    /// reads as reachable from the campaign's first tick.
    pub const DROP_COLLECT_UNORDERED: DwCode = DwCode::new("DW0493", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §6) A wave's TD `lane` / `summon` declaration is structurally
    /// invalid or internally contradictory: an empty `waypoints` list, a
    /// waypoint anchor no area's prefab provides, a repeated consecutive
    /// waypoint, an `aggro_radius` outside `4..=64`, a mob whose
    /// `attributes.follow_range` disagrees with `aggro_radius` (they MUST be
    /// equal — a patrolling raider holds ground against a target it cannot
    /// engage), or `lane` together with `summon: aggro-edge` (a lane IS the
    /// routing; aggro-edge is its opposite).
    pub const LANE_INVALID: DwCode = DwCode::new("DW0381", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §6) A lane wave contains a non-raider species. `Patrolling` /
    /// `patrol_target` are Raider NBT: on anything else they are dropped and the
    /// mob simply stands where it spawned. The admitted set is vanilla's own
    /// `#minecraft:raiders` tag, read from the vendored tag table — never a
    /// species list this engine keeps. Non-raiders use `summon: aggro-edge`
    /// instead.
    pub const LANE_NOT_RAIDER: DwCode = DwCode::new("DW0382", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §6) A lane wave fields fewer than 2 mobs. A lone patroller
    /// sets `Patrolling:0b` on itself when it finds no companion within its
    /// follow range (vanilla), so a one-mob lane cancels itself.
    pub const LANE_SQUAD_TOO_SMALL: DwCode = DwCode::new("DW0383", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §6) A lane `pillager` is not holding a crossbow. Its only
    /// attack goal is the crossbow goal, so a pillager that acquires a target it
    /// has no runnable attack for freezes in place indefinitely — patrol blocked
    /// by the target, nothing to run instead (live-verified deadlock).
    pub const LANE_UNARMED: DwCode = DwCode::new("DW0384", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §6) A `summon: aggro-edge` wave mob declares no
    /// `attributes.follow_range`. That radius IS the summon ring — the distance
    /// at which the mob perceives the party — so it is authored, never guessed
    /// from a vanilla defaults table the compiler cannot verify.
    pub const AGGRO_EDGE_NO_RANGE: DwCode = DwCode::new("DW0385", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.6) A campaign stages actors meant to **fight** — unleashed, or
    /// declared `vulnerable` — but declares no `world.difficulty`, and the
    /// engine's derivation ([`crate::derived_difficulty`]) ships `peaceful`
    /// because it fields no wave and stages no body peaceful discards. Every
    /// such fighter is a species peaceful keeps, and on peaceful every hit it
    /// lands on a player whose damage scales with difficulty — a mob's melee and
    /// its projectiles — is zero. Advisory (warning, exit 0): declaring
    /// `world.difficulty` settles it.
    pub const DIFFICULTY_UNDECLARED_ACTORS: DwCode = DwCode::new("DW0469", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §1, spec-0023, souls ruling 5/7: "stage bosses never respawn
    /// on rest") A wave declares BOTH `tier: boss` and `respawns_on_rest: true`.
    /// `tier` and `respawns_on_rest` are two fields on the same [`Wave`]
    /// declaration — the only place a "boss" billing and a "re-seat on rest"
    /// contract can land on one another; an [`Actor`] carries `tier` too but has
    /// no `respawns_on_rest` field at all (it is killed by hand, never re-seated
    /// by a bonfire), so this is the sole structurally expressible violation of
    /// the ruling. A rest-respawning boss re-fight breaks the retry economy the
    /// ruling exists to protect: a boss is the campaign's named fight, not
    /// trash pressure the party grinds back down every rest. Validation-tier
    /// (exit 1), `dsl::validate`. Prescription: drop `tier: boss` if the
    /// encounter really is meant to re-seat (bill it `elite` instead), or drop
    /// `respawns_on_rest` if it really is the boss.
    ///
    /// [`Wave`]: crate::Wave
    /// [`Actor`]: crate::Actor
    pub const BOSS_RESPAWNS_ON_REST: DwCode = DwCode::new("DW0499", ExitTier::Build);
}

/// The vanilla `entity_type` tag whose members honour `Patrolling` /
/// `patrol_target`: `#minecraft:raiders`.
///
/// On anything outside it the keys are inert — the mob stands where it spawned —
/// which is the silent no-op class `DW0382` exists to make loud.
///
/// **The species list is Mojang's, never ours**, the same rule `DW0496` follows
/// for `#minecraft:burn_in_daylight`. In the pinned game these are the same six
/// types by two independent routes: the data branch publishes them as this tag,
/// and the code branch makes exactly them subclasses of `PatrollingMonster` —
/// whose own `registerGoals` adds the `LongDistancePatrolGoal` every one of them
/// inherits. A hand-written table is how the two come apart, and had: it named
/// five, omitting `minecraft:illusioner`, so a lane of illusioners was refused a
/// march the game would have walked.
const LANE_RAIDER_TAG: &str = "minecraft:raiders";

/// Whether `entity` may be fielded in a lane — membership of [`LANE_RAIDER_TAG`].
///
/// `#minecraft:raiders` names only concrete types in the pinned game (no nested
/// `#tag` member), so [`crate::registry::entity_in_tag`]'s deliberate
/// non-expansion cannot narrow this set.
fn is_lane_raider(entity: &str) -> bool {
    crate::registry::entity_in_tag(entity, LANE_RAIDER_TAG)
}

/// Species whose ONLY attack goal is gated on holding a specific weapon: they
/// acquire a target, find no runnable attack goal, and freeze — while the patrol
/// goal stays blocked by the very target they cannot hit (`DW0384`). A pillager
/// is a crossbow mob and nothing else, so this table has exactly one row.
///
/// The near miss is `minecraft:illusioner`, whose ranged goal is bow-gated the
/// same way — but it also carries two spell goals that are gated on nothing but
/// a target, so a bare-handed illusioner has something runnable and does not
/// freeze. Every other raider melees or casts bare-handed.
const LANE_WEAPON_GATED: [(&str, &str); 1] = [("pillager", "minecraft:crossbow")];

/// The bare entity id (`minecraft:pillager` → `pillager`).
fn bare_entity(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

/// The advisory half of the difficulty surface (`DW0469`): a campaign that
/// stages a **fighting** actor, declares no `world.difficulty`, and is derived
/// `peaceful` ([`crate::derived_difficulty`]: no wave, no staged body peaceful
/// discards).
///
/// A fighter of a species peaceful discards never reaches this check: the
/// derivation already ships `easy` for it. What remains is a fighter peaceful
/// keeps — and on peaceful, `Player#hurtServer` scales every damage source that
/// `scalesWithDifficulty()` (a living non-player attacker's melee and
/// projectiles) to zero, so the fight lands no blow.
///
/// "Fighting" is read off the campaign's own declarations: an `unleash-actor`
/// at any effect root ([`crate::fight::unleashed_actors`]) or `vulnerable: true`.
pub(crate) fn difficulty_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if c.world.content.difficulty.is_some()
        || crate::derived_difficulty(c) != crate::WorldDifficulty::Peaceful
    {
        return;
    }
    let unleashed = crate::fight::unleashed_actors(c);
    let fighters: Vec<String> = c
        .quests
        .content
        .actors
        .iter()
        .filter(|a| a.vulnerable || unleashed.contains(a.id.as_str()))
        .map(|a| format!("{} ({})", a.id.as_str(), a.entity))
        .collect();
    if fighters.is_empty() {
        return;
    }
    d.push(Diagnostic::warning(
        DIFFICULTY_UNDECLARED_ACTORS,
        "world",
        "/content/difficulty".to_string(),
        format!(
            "this campaign stages {} actor(s) meant to FIGHT — unleashed, or declared \
             `vulnerable` — [{}], but declares no `world.difficulty`, and with no wave and no \
             body peaceful discards the engine derives `difficulty=peaceful`. These species \
             survive peaceful, but on peaceful every hit they land on a player whose damage \
             scales with difficulty (a mob's melee and its projectiles) is zero, so the fight \
             lands no blow. Declare `world.difficulty` on the world stage: `easy` halves \
             incoming damage, `normal` is the vanilla baseline.",
            fighters.len(),
            fighters.join(", ")
        ),
    ));
}

/// Validate the spec-0016 §6 wave `lane` / `summon` surface.
///
/// Five rules, five codes, each pinned to a live-verified 1.21.11 failure mode:
/// * `DW0381` — the declaration does not resolve or contradicts itself;
/// * `DW0382` — a lane species outside the raider family (the NBT is inert);
/// * `DW0383` — a lane squad below 2 (a lone patroller self-cancels);
/// * `DW0384` — a lane pillager without its crossbow (target-acquisition deadlock);
/// * `DW0385` — an aggro-edge mob with no authored `follow_range` (no ring radius).
///
/// Anchor resolution stays lenient for pool areas the compiler resolves later —
/// the same policy as the trap, trigger and shortcut checks. Waypoint *geometry*
/// (standable, reachable, spaced > 10) is a build-tier proof over the assembled
/// world (`DW0386`), not a validation-tier one.
pub(crate) fn lane_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    if quests
        .waves
        .iter()
        .all(|w| w.lane.is_none() && w.summon.is_none())
    {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);

    for (i, w) in quests.waves.iter().enumerate() {
        let aggro_edge = w.summon == Some(crate::WaveSummon::AggroEdge);
        if aggro_edge {
            if w.lane.is_some() {
                d.push(Diagnostic::error(
                    LANE_INVALID,
                    "quests",
                    format!("/content/waves/{i}/summon"),
                    format!(
                        "wave `{}` declares BOTH a `lane` and `summon: aggro-edge` (spec-0016 §6) \
                         — a lane IS the routing (march while distant, native AI once aggroed), \
                         and aggro-edge is its opposite (materialize already at the edge of \
                         perception, no routing at all). Pick one.",
                        w.id
                    ),
                ));
            }
            for (k, m) in w.mobs.iter().enumerate() {
                if m.attributes.and_then(|a| a.follow_range).is_none() {
                    d.push(Diagnostic::error(
                        AGGRO_EDGE_NO_RANGE,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/attributes"),
                        format!(
                            "`summon: aggro-edge` mob `{}` in wave `{}` declares no \
                             `attributes.follow_range` (spec-0016 §6). That radius IS the summon \
                             ring — the distance at which this mob perceives the party — so it is \
                             authored, never guessed: the compiler will not fabricate a vanilla \
                             default it cannot verify against the pinned server.",
                            m.entity, w.id
                        ),
                    ));
                }
            }
        }
        let Some(lane) = &w.lane else { continue };

        if lane.waypoints.is_empty() {
            d.push(Diagnostic::error(
                LANE_INVALID,
                "quests",
                format!("/content/waves/{i}/lane/waypoints"),
                format!(
                    "wave `{}` declares a `lane` with no waypoints (spec-0016 §6) — a lane is a \
                     polyline the squad marches; give it at least one waypoint anchor",
                    w.id
                ),
            ));
        }
        for (k, wp) in lane.waypoints.iter().enumerate() {
            if let Some(f) = station_kind_diag(
                &providers,
                wp.as_str(),
                crate::layout::StationKind::Point,
                "a lane waypoint",
                "quests",
                format!("/content/waves/{i}/lane/waypoints/{k}"),
            ) {
                d.push(f);
            }
            if !providers.resolvable(wp.as_str()) {
                d.push(Diagnostic::error(
                    LANE_INVALID,
                    "quests",
                    format!("/content/waves/{i}/lane/waypoints/{k}"),
                    format!(
                        "lane waypoint anchor `{wp}` is not provided by any area's prefab — {}",
                        providers.anchor_remedy(
                            "use an anchor a prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
            if k > 0 && lane.waypoints[k - 1] == *wp {
                d.push(Diagnostic::error(
                    LANE_INVALID,
                    "quests",
                    format!("/content/waves/{i}/lane/waypoints/{k}"),
                    format!(
                        "lane waypoint `{wp}` repeats the one before it — the squad would be told \
                         to march to where it already stands, and vanilla re-rolls a patrol \
                         target on arrival. Remove the repeat."
                    ),
                ));
            }
        }
        if !(4..=64).contains(&lane.aggro_radius) {
            d.push(Diagnostic::error(
                LANE_INVALID,
                "quests",
                format!("/content/waves/{i}/lane/aggro_radius"),
                format!(
                    "lane `aggro_radius` {} on wave `{}` is outside 4..=64 (spec-0016 §6). It is \
                     emitted verbatim as the mobs' `follow_range` attribute AND as the release \
                     radius; below 4 the squad walks into contact before it can see anyone, and \
                     past 64 it aggroes across the whole delve.",
                    lane.aggro_radius, w.id
                ),
            ));
        }
        if w.mobs.iter().map(|m| m.count).sum::<u32>() < 2 {
            d.push(Diagnostic::error(
                LANE_SQUAD_TOO_SMALL,
                "quests",
                format!("/content/waves/{i}/mobs"),
                format!(
                    "lane wave `{}` fields fewer than 2 mobs (spec-0016 §6). A lone patroller \
                     sets `Patrolling:0b` on ITSELF when it finds no companion within its follow \
                     range — vanilla behaviour, live-verified — so a one-mob lane cancels its own \
                     routing and just stands there. Field a squad of at least 2.",
                    w.id
                ),
            ));
        }
        for (k, m) in w.mobs.iter().enumerate() {
            let bare = bare_entity(&m.entity);
            if !is_lane_raider(&m.entity) {
                d.push(Diagnostic::error(
                    LANE_NOT_RAIDER,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/entity"),
                    format!(
                        "lane wave `{}` fields `{}`, which is not raider-family (spec-0016 §6). \
                         `Patrolling`/`patrol_target` are Raider NBT: on any other species they \
                         are simply dropped and the mob stands where it spawned. Lane species \
                         (vanilla's own `#{LANE_RAIDER_TAG}` tag): {}. For anything else use \
                         `summon: aggro-edge`, which needs no patrol AI.",
                        w.id,
                        m.entity,
                        crate::registry::entity_tag_members_bare(LANE_RAIDER_TAG).join(" / ")
                    ),
                ));
            }
            if let Some((_, weapon)) = LANE_WEAPON_GATED.iter().find(|(s, _)| *s == bare) {
                let held = m
                    .equipment
                    .as_ref()
                    .and_then(|e| e.main_hand.as_ref())
                    .map_or(*weapon, |p| p.item());
                if held != *weapon {
                    d.push(Diagnostic::error(
                        LANE_UNARMED,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/equipment/main_hand"),
                        format!(
                            "lane `{bare}` in wave `{}` holds `{held}` instead of `{weapon}` \
                             (spec-0016 §6). Its ONLY attack goal is the crossbow goal, so on \
                             acquiring a target it has nothing runnable to do — and the patrol \
                             goal is meanwhile blocked BY that target. The mob freezes in place \
                             indefinitely (live-verified deadlock). Give it the crossbow, or drop \
                             the `main_hand` override and take the compiler's default.",
                            w.id
                        ),
                    ));
                }
            }
        }
        if let Some(bad) = w.mobs.iter().enumerate().find(|(_, m)| {
            m.attributes
                .and_then(|a| a.follow_range)
                .is_some_and(|r| r != f64::from(lane.aggro_radius))
        }) {
            let (k, m) = bad;
            d.push(Diagnostic::error(
                LANE_INVALID,
                "quests",
                format!("/content/waves/{i}/mobs/{k}/attributes/follow_range"),
                format!(
                    "lane mob `{}` in wave `{}` declares `follow_range` {} but the lane's \
                     `aggro_radius` is {} (spec-0016 §6). They MUST be equal: the release radius \
                     is where routing hands over to native AI, and a patrolling raider that \
                     targets a player outside its engagement range HOLDS GROUND instead of \
                     marching — the squad stalls mid-lane. Drop the override (the compiler sets \
                     `follow_range` from `aggro_radius`) or make the two agree.",
                    m.entity,
                    w.id,
                    m.attributes
                        .and_then(|a| a.follow_range)
                        .unwrap_or_default(),
                    lane.aggro_radius
                ),
            ));
        }
    }
}

/// Declared drops: what an elite or boss
/// leaves behind is a **declared subset**, never automatically everything.
///
/// Four rules, all of them about the gap between what a campaign says and what
/// the world can actually produce:
///
/// * `DW0491` — only an `elite`/`boss` encounter may declare drops. Rank-and-file
///   gear stays unfarmable by construction (no-grind constitution).
/// * `DW0490` — a `slot` entry must name a **distinct** slot the same entity's
///   own `equipment` fills. A body cannot leave behind a piece it never wore.
/// * `DW0492` — a `dropped_by` collect must be backed by the wave it names: the
///   wave really declares that item, in at least the count the objective asks
///   for, and the objective does not also adopt a container.
/// * `DW0493` — that collect must be **ordered after** the fight, so the chain
///   "kill the boss → take its key → open the door" is a proof rather than an
///   intention.
pub(crate) fn check_drops(
    c: &Campaign,
    quests: &crate::QuestsContent,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    use crate::{EncounterTier, MobDrop};

    // --- the declaration side: waves and actors ---------------------------
    let tiered =
        |t: Option<EncounterTier>| matches!(t, Some(EncounterTier::Elite | EncounterTier::Boss));
    for (i, w) in quests.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            if m.drops.is_empty() {
                continue;
            }
            if !tiered(w.tier) {
                d.push(Diagnostic::error(
                    DROP_NOT_TIERED,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/drops"),
                    format!(
                        "wave `{}` declares drops but is not billed `elite` or `boss` — only a \
                         named fight leaves anything behind; an ordinary mob's kit is never \
                         farmable. Declare the wave's `tier`, or remove the `drops`",
                        w.id
                    ),
                ));
            }
            check_drop_list(
                &m.drops,
                m.equipment.as_ref(),
                &format!("wave `{}` mob {k}", w.id),
                &format!("/content/waves/{i}/mobs/{k}/drops"),
                items,
                d,
            );
        }
    }
    for (i, a) in quests.actors.iter().enumerate() {
        if a.drops.is_empty() {
            continue;
        }
        if !tiered(a.tier) {
            d.push(Diagnostic::error(
                DROP_NOT_TIERED,
                "quests",
                format!("/content/actors/{i}/drops"),
                format!(
                    "actor `{}` declares drops but is not billed `elite` or `boss` — only a named \
                     fight leaves anything behind; a staged puppet's kit is never farmable. \
                     Declare the actor's `tier`, or remove the `drops`",
                    a.id
                ),
            ));
        }
        check_drop_list(
            &a.drops,
            a.equipment.as_ref(),
            &format!("actor `{}`", a.id),
            &format!("/content/actors/{i}/drops"),
            items,
            d,
        );
    }

    // --- the consumption side: `collect.dropped_by` ------------------------
    // How many copies of each item every wave can yield: one per declaring mob
    // in the stack, so a pair of elites each dropping a sword yields two.
    let mut yielded: BTreeMap<&str, BTreeMap<&str, u32>> = BTreeMap::new();
    for w in &quests.waves {
        let per = yielded.entry(w.id.as_str()).or_default();
        for m in &w.mobs {
            for dr in &m.drops {
                if let MobDrop::Item(it) = dr {
                    *per.entry(it.item.as_str()).or_default() += m.count;
                }
            }
        }
    }
    let anc = quest_ancestors(c);
    // Which quests hold a `kill` objective for each wave, and which objective ids.
    let mut kills: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for q in &quests.quests {
        for o in &q.objectives {
            if let Objective::Kill { wave, id, .. } = o {
                kills
                    .entry(wave.as_str())
                    .or_default()
                    .push((q.id.as_str(), id.as_str()));
            }
        }
    }
    for (i, q) in quests.quests.iter().enumerate() {
        let after_anc = objective_ancestors(q);
        for (j, o) in q.objectives.iter().enumerate() {
            let Objective::Collect {
                id,
                item,
                count,
                container,
                dropped_by: Some(wave),
                ..
            } = o
            else {
                continue;
            };
            let path = format!("/content/quests/{i}/objectives/{j}/dropped_by");
            if container.is_some() {
                d.push(Diagnostic::error(
                    DROP_COLLECT_UNSOURCED,
                    "quests",
                    path.clone(),
                    format!(
                        "`collect` `{id}` declares both `dropped_by` (wave `{wave}`) and a \
                         `container` — the item comes off a body or out of a box, not both; drop \
                         whichever provisioning this beat does not use"
                    ),
                ));
            }
            let Some(per) = yielded.get(wave.as_str()) else {
                // Unknown wave: the ordinary dangling-reference diagnostic
                // (`DW0170`) already names it; nothing to add here.
                continue;
            };
            match per.get(item.as_str()).copied() {
                None => d.push(Diagnostic::error(
                    DROP_COLLECT_UNSOURCED,
                    "quests",
                    path.clone(),
                    format!(
                        "`collect` `{id}` takes `{item}` off wave `{wave}`, but no mob of that \
                         wave declares a `{{\"item\": \"{item}\"}}` drop — {}. Declare the drop on \
                         the wave's mob, or point `dropped_by` at the wave that really carries it",
                        if per.is_empty() {
                            "the wave declares no item drops at all".to_string()
                        } else {
                            format!(
                                "it declares {}",
                                per.keys().cloned().collect::<Vec<_>>().join(", ")
                            )
                        }
                    ),
                )),
                Some(have) if have < *count => d.push(Diagnostic::error(
                    DROP_COLLECT_UNSOURCED,
                    "quests",
                    path.clone(),
                    format!(
                        "`collect` `{id}` asks for {count} × `{item}`, but wave `{wave}` yields \
                         only {have} — a body drops its declared item once. Lower the `count`, or \
                         raise the declaring mob's `count`"
                    ),
                )),
                Some(_) => {}
            }
            // The ordering proof: some `kill` objective for this wave must
            // strictly precede this collect — in the same quest through the
            // `after` graph, or in a quest this one transitively depends on.
            let ordered = kills.get(wave.as_str()).is_some_and(|ks| {
                ks.iter().any(|(kq, ko)| {
                    if *kq == q.id.as_str() {
                        after_anc
                            .get(id.as_str())
                            .is_some_and(|set| set.contains(ko))
                    } else {
                        anc.get(q.id.as_str()).is_some_and(|set| set.contains(kq))
                    }
                })
            });
            if !ordered {
                d.push(Diagnostic::error(
                    DROP_COLLECT_UNORDERED,
                    "quests",
                    path,
                    format!(
                        "`collect` `{id}` takes `{item}` off wave `{wave}`, but no `kill` \
                         objective for `{wave}` is proven to run first — the item would be \
                         unreachable while the objective reads as active from the campaign's \
                         first tick. Add a `kill` objective for `{wave}` and list it in this \
                         objective's `after`, or put the kill in a quest this one `depends_on`"
                    ),
                ));
            }
        }
    }
}

/// One entity's `drops[]` list: distinct, really-worn slots (`DW0490`) and
/// registry-valid quest items (`DW0143`). Shared by wave mobs and actors so the
/// two surfaces cannot drift.
fn check_drop_list(
    drops: &[crate::MobDrop],
    equipment: Option<&crate::MobEquipment>,
    what: &str,
    base_path: &str,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    use crate::MobDrop;

    let mut seen_slots: BTreeSet<&'static str> = BTreeSet::new();
    for (n, dr) in drops.iter().enumerate() {
        match dr {
            MobDrop::Slot(s) => {
                let field = s.slot.field();
                let filled: Vec<&str> = equipment
                    .map(|eq| {
                        eq.slots()
                            .into_iter()
                            .filter(|(_, p)| p.is_some())
                            .map(|(name, _)| name)
                            .collect()
                    })
                    .unwrap_or_default();
                if equipment.is_none_or(|eq| eq.filled(s.slot).is_none()) {
                    d.push(Diagnostic::error(
                        DROP_SLOT_UNFILLED,
                        "quests",
                        format!("{base_path}/{n}/slot"),
                        format!(
                            "{what} declares a `{field}` drop, but its `equipment` puts nothing \
                             in `{field}` — {}. A body can only leave behind a piece it wears: \
                             equip the slot, or drop a slot it fills",
                            if filled.is_empty() {
                                "it declares no equipment at all".to_string()
                            } else {
                                format!("it fills {}", filled.join(", "))
                            }
                        ),
                    ));
                } else if !seen_slots.insert(field) {
                    d.push(Diagnostic::error(
                        DROP_SLOT_UNFILLED,
                        "quests",
                        format!("{base_path}/{n}/slot"),
                        format!(
                            "{what} declares the `{field}` drop twice — a body leaves each piece \
                             behind once; remove the duplicate entry"
                        ),
                    ));
                }
            }
            MobDrop::Item(it) => {
                if !items.contains(&it.item) {
                    d.push(Diagnostic::error(
                        codes::ITEM_UNKNOWN,
                        "quests",
                        format!("{base_path}/{n}/item"),
                        format!(
                            "{what} declares a drop of `{}`, which is not in the pinned 1.21.11 \
                             item registry — use a valid namespaced item id (e.g. \
                             `minecraft:tripwire_hook`)",
                            it.item
                        ),
                    ));
                }
            }
        }
    }
}

/// Per-objective transitive `after` ancestors within one quest: `obj -> {every
/// objective that must complete before it}`. Acyclicity is guaranteed by
/// `DW0140`; a cyclic quest simply yields a partial set and the cycle's own
/// diagnostic fires.
fn objective_ancestors(q: &crate::Quest) -> BTreeMap<&str, BTreeSet<&str>> {
    let direct: BTreeMap<&str, Vec<&str>> = q
        .objectives
        .iter()
        .map(|o| {
            (
                o.id().as_str(),
                o.after().iter().map(|a| a.as_str()).collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut out = BTreeMap::new();
    for o in &q.objectives {
        let mut anc: BTreeSet<&str> = BTreeSet::new();
        let mut stack = vec![o.id().as_str()];
        while let Some(cur) = stack.pop() {
            if let Some(ds) = direct.get(cur) {
                for dep in ds {
                    if anc.insert(dep) {
                        stack.push(dep);
                    }
                }
            }
        }
        out.insert(o.id().as_str(), anc);
    }
    out
}

/// Validate one [`MobEquipment`] block — item ids against the pinned registry
/// (`DW0143`) and every piece's enchantments against the pinned enchantment
/// registry (`DW0433`) and level range (`DW0434`).
///
/// Shared verbatim by wave mobs and actors so the two surfaces cannot drift:
/// they are the same schema type and therefore must be the same rules.
pub(crate) fn check_equipment(
    eq: &crate::MobEquipment,
    what: &str,
    base_path: &str,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let ench_reg = crate::registry::VendoredEnchantmentRegistry::v1_21_11();
    for (slot, piece) in eq.slots() {
        let Some(piece) = piece else { continue };
        let it = piece.item();
        if !items.contains(it) {
            d.push(Diagnostic::error(
                codes::ITEM_UNKNOWN,
                "quests",
                format!("{base_path}/{slot}"),
                format!(
                    "{what} equipment `{slot}` item `{it}` is not in the pinned 1.21.11 \
                     item registry — use a valid namespaced item id (e.g. \
                     `minecraft:iron_helmet`)"
                ),
            ));
        }
        check_enchantments(
            piece.enchantments(),
            &format!("{what} equipment `{slot}`"),
            "quests",
            &format!("{base_path}/{slot}/enchantments"),
            &ench_reg,
            d,
        );
    }
}

/// Wave-mob `equipment` (spec-0014): item ids and enchantments
/// ([`check_equipment`]).
pub(crate) fn wave_equipment_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    // Wave-mob `equipment` item ids: every present slot must name a
    // pinned-1.21.11 item — the same registry and DW family as `give-item`
    // (`DW0143`).
    for (i, w) in quests.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            let Some(eq) = &m.equipment else { continue };
            check_equipment(
                eq,
                "wave-mob",
                &format!("/content/waves/{i}/mobs/{k}/equipment"),
                items,
                d,
            );
        }
    }
}

/// A wave's `respawns_on_rest` (spec-0016 §1): inert with no `bonfire`
/// (`DW0370`), and forbidden on a `boss` (`DW0499`).
pub(crate) fn rest_reseat_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    // spec-0016 §1: `respawns_on_rest` is re-seating *by a bonfire*. With no
    // `bonfire` anywhere in the campaign nothing can ever fire the re-seat, so
    // the field is a silent no-op — the class of defect this compiler always
    // turns loud (`DW0370`).
    let has_bonfire = declares_bonfire(c);
    if !has_bonfire {
        for (i, w) in quests.waves.iter().enumerate() {
            if w.respawns_on_rest {
                d.push(Diagnostic::error(
                    REST_RESEAT_NO_BONFIRE,
                    "quests",
                    format!("/content/waves/{i}/respawns_on_rest"),
                    format!(
                        "wave `{}` declares `respawns_on_rest: true` but this campaign declares \
                         no `bonfire` — nothing can ever re-seat it, so the field is inert. Add \
                         the `bonfire` the re-seat hangs off (spec-0016 §1), or drop the field; \
                         do NOT leave a silently dead declaration in the DSL.",
                        w.id.as_str()
                    ),
                ));
            }
        }
    }

    // spec-0016 §1 + spec-0023, souls ruling 5/7 ("stage bosses never respawn
    // on rest"): `tier` and
    // `respawns_on_rest` are two fields on the SAME wave declaration — the only
    // place a "boss" billing and a "re-seat on rest" contract can land on one
    // another (an actor carries `tier` too, but has no `respawns_on_rest` field
    // at all, so it cannot express this violation). A rest-respawning boss
    // re-fight breaks the retry economy the ruling protects. Checked
    // unconditionally of `has_bonfire`: the combination is forbidden on its own
    // terms, not merely inert like `DW0370`.
    for (i, w) in quests.waves.iter().enumerate() {
        if w.respawns_on_rest && w.tier == Some(EncounterTier::Boss) {
            d.push(Diagnostic::error(
                BOSS_RESPAWNS_ON_REST,
                "quests",
                format!("/content/waves/{i}/respawns_on_rest"),
                format!(
                    "wave `{}` declares `tier: boss` AND `respawns_on_rest: true` — souls \
                     ruling 5/7 is that stage bosses never respawn on rest, since a \
                     rest-respawning boss re-fight breaks the retry economy the ruling \
                     protects. Drop `respawns_on_rest` if this really is the boss, or drop \
                     `tier: boss` (bill it `elite` instead) if the encounter is meant to \
                     re-seat.",
                    w.id.as_str()
                ),
            ));
        }
    }
}

/// Wave declarations (spec-0001 v0.3): id syntax (`DW0110`), uniqueness
/// (`DW0111`) and every mob's entity id (`DW0173`).
pub(crate) fn wave_decl_checks(
    c: &Campaign,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;

    // Wave declarations.
    let mut seen_waves: BTreeSet<&str> = BTreeSet::new();
    for (i, w) in quests.waves.iter().enumerate() {
        if !w.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/waves/{i}/id"),
                format!(
                    "malformed wave id `{}` — wave ids must be lowercase kebab-case with the \
                     `wave/` prefix (e.g. `wave/ambush`)",
                    w.id
                ),
            ));
        }
        if !seen_waves.insert(w.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/waves/{i}/id"),
                format!(
                    "duplicate wave id `{}` — rename one so every wave id is unique",
                    w.id
                ),
            ));
        }
        for (k, m) in w.mobs.iter().enumerate() {
            if !entities.contains(&m.entity) {
                d.push(Diagnostic::error(
                    codes::ENTITY_UNKNOWN,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/entity"),
                    format!(
                        "wave-mob entity `{}` is not a known 1.21.11 entity id — use a valid \
                         namespaced entity id (e.g. `minecraft:zombie`)",
                        m.entity
                    ),
                ));
            }
        }
    }
}

/// Every wave mob's status effects name a 1.21.11 effect (`DW0192`).
pub(crate) fn mob_effect_checks(
    c: &Campaign,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;

    // --- wave-mob effects + attributes ---
    for (i, w) in quests.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            for (e, eff) in m.effects.iter().enumerate() {
                if !effects.contains(&eff.effect) {
                    d.push(Diagnostic::error(
                        codes::EFFECT_UNKNOWN,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/effects/{e}/effect"),
                        format!(
                            "wave-mob effect `{}` is not a known 1.21.11 status-effect id — use a \
                             valid namespaced effect id (e.g. `minecraft:strength`)",
                            eff.effect
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

crate::dw_code! {
    /// (spec-0021) An `equipment` or `loot` enchantment id is not in the pinned
    /// 1.21.11 enchantment registry.
    pub const ENCHANTMENT_UNKNOWN: DwCode = DwCode::new("DW0433", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0021) An enchantment level is outside the 1..=255 range vanilla's
    /// `minecraft:enchantments` component can carry.
    pub const ENCHANTMENT_LEVEL: DwCode = DwCode::new("DW0434", ExitTier::Build);
}

/// Validate an enchantment map: known ids (`DW0433`), legal levels (`DW0434`).
///
/// Levels are checked against what the `minecraft:enchantments` **component**
/// can carry (1..=255), not against each enchantment's survival max. Exceeding
/// the survival max from a command is legal vanilla and is a legitimate way to
/// build a set-piece elite, so refusing it would be the compiler overruling a
/// design decision it cannot second-guess; 0 and >255 are simply not
/// representable and would be silently dropped by the game.
pub(crate) fn check_enchantments(
    ench: &std::collections::BTreeMap<String, u32>,
    what: &str,
    stage: &'static str,
    path: &str,
    reg: &dyn crate::registry::EnchantmentRegistry,
    d: &mut Vec<Diagnostic>,
) {
    for (id, level) in ench {
        if !reg.contains(id) {
            d.push(Diagnostic::error(
                ENCHANTMENT_UNKNOWN,
                stage,
                format!("{path}/{id}"),
                format!(
                    "{what} enchantment `{id}` is not in the pinned 1.21.11 enchantment \
                     registry — use a valid namespaced enchantment id (e.g. \
                     `minecraft:protection`, `minecraft:sharpness`). Note the vanilla \
                     ids for curses are `minecraft:binding_curse` and \
                     `minecraft:vanishing_curse`, NOT `curse_of_binding`."
                ),
            ));
        }
        if *level == 0 || *level > 255 {
            d.push(Diagnostic::error(
                ENCHANTMENT_LEVEL,
                stage,
                format!("{path}/{id}"),
                format!(
                    "{what} enchantment `{id}` has level {level}, outside the 1..=255 range \
                     the `minecraft:enchantments` component stores. Levels above an \
                     enchantment's survival maximum ARE allowed (that is how a set-piece \
                     elite is built) — but 0 means \"not enchanted\" and is silently \
                     dropped by the game, so declare the level you want or remove the entry."
                ),
            ));
        }
    }
}
