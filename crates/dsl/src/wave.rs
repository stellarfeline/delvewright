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
