//! Stage 3 — classes: the kits a party chooses from, and the item grants and
//! potion contents a kit or a `give-item` carries.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ClassId;
use crate::serde_fields::is_false;

#[cfg(doc)]
use crate::Verb;

/// Who a granted item goes to (DSL v0.6, spec-0018).
///
/// Progression is a fact about the party, so the default for every `give-item` and
/// every class-kit entry is **all**: a quest beat that arms the party arms all of
/// it. `one` is the deliberate exception for a single quest prop (the wine-skin,
/// the stake): exactly one copy enters the party, handed to the player whose action
/// earned it, and the party passes it around physically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Carrier {
    /// Every party member receives the item (the default).
    #[default]
    All,
    /// Exactly one copy, to the player whose action fired the effect.
    One,
}

/// Stage 3 payload: 1..4 selectable classes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClassesContent {
    /// The selectable classes.
    pub classes: Vec<Class>,
}

/// A player class with a starting kit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Class {
    /// Unique class id.
    pub id: ClassId,
    /// Player-facing name.
    pub name: String,
    /// Selection-screen blurb.
    pub blurb: String,
    /// Granted items.
    pub kit: Vec<KitItem>,
}

/// One item in a class kit.
///
/// Note: `lore`, `enchantments` and `attributes` are reserved for M2/M3
/// (spec-0001). They are intentionally *not* defined as fields in v0, so a
/// document using them is rejected as an unknown field (`DW0100`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KitItem {
    /// Vanilla item id, validated against the pinned 1.21.11 registry.
    pub item: String,
    /// Stack count.
    pub count: u32,
    /// Optional display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Who gets it (DSL v0.6, spec-0018). Absent = [`Carrier::All`]. A class kit is
    /// per-player gear by construction — every player who picks the class gets the
    /// kit — so `carrier` here marks a **party-unique** kit item: exactly one copy
    /// enters the party, given to the first player to pick this class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrier: Option<Carrier>,
    /// **The flask** (DSL v0.8, spec-0016 §1): this kit
    /// entry is the class's recovery item, and resting at a bonfire replenishes
    /// it to exactly `count`. A campaign that places a `bonfire` and declares no
    /// flask anywhere in its kits is `DW0476` — the estus loop is what makes
    /// dying an investment, so a souls campaign without one is a build error, not
    /// a design choice. Absent on every pre-0.8 kit → emission byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flask: bool,
    /// **What is in the bottle** (DSL v0.8, spec-0016 §1): the vanilla
    /// `minecraft:potion_contents` component of a
    /// potion-bearing item ([`POTION_BEARING_ITEMS`]). Without it a
    /// `minecraft:potion` is the *Uncraftable Potion* — a bottle that heals
    /// nothing — which is exactly the placeholder flask this field exists to
    /// abolish, so at `dsl_version` 0.8.0 a potion-bearing kit item that declares
    /// no `contents` is `DW0487`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<PotionContents>,
}

/// The items whose vanilla item definition carries a `minecraft:potion_contents`
/// component — the only items a kit `contents` may be declared on (`DW0486`).
///
/// Read off the pinned 1.21.11 `item_components` summary (SHA-256
/// `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4`, the same
/// misode/mcmeta ref `crates/delvec/data/PROVENANCE.md` pins): exactly these
/// four items declare the component, and on any other item the game drops the
/// data on the floor.
pub const POTION_BEARING_ITEMS: &[&str] = &[
    "minecraft:lingering_potion",
    "minecraft:potion",
    "minecraft:splash_potion",
    "minecraft:tipped_arrow",
];

/// True if `item_id` (optionally un-namespaced) is one of the four
/// [`POTION_BEARING_ITEMS`].
pub fn is_potion_bearing_item(item_id: &str) -> bool {
    let norm = if item_id.contains(':') {
        item_id.to_string()
    } else {
        format!("minecraft:{item_id}")
    };
    POTION_BEARING_ITEMS.contains(&norm.as_str())
}

/// The two **instantaneous** status effects: they are applied once, on the tick
/// the potion is drunk (`PotionContents.applyToLivingEntity` branches on
/// `isInstantenous` before any effect instance is ever added), so a `duration` on
/// one is a statement the game never reads — `DW0486` says so rather than letting
/// an author believe they wrote a thirty-second heal.
pub const INSTANT_EFFECTS: &[&str] = &["minecraft:instant_health", "minecraft:instant_damage"];

/// A potion-bearing kit item's `minecraft:potion_contents` component (DSL v0.8),
/// modelled field for field on vanilla rather than invented: a **named** potion,
/// a list of **custom effects**, or both, plus the bottle-colour override.
///
/// Vanilla resolves a drink as the named potion's effects followed by the custom
/// ones, and derives the bottle colour from those effects unless `color`
/// overrides it — so `{"potion": "minecraft:strong_healing"}` is literally the
/// Potion of Healing II a player would brew, not an approximation of one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PotionContents {
    /// A named vanilla potion id (`minecraft:strong_healing`,
    /// `minecraft:long_night_vision`, …), validated against the pinned 1.21.11
    /// `potion` registry (`DW0486`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potion: Option<String>,
    /// Custom effects applied on top of (or instead of) the named potion — the
    /// escape hatch for a recovery item vanilla has no brew for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<PotionEffect>,
    /// Bottle-colour override, `#rrggbb` (vanilla `custom_color`). Absent → the
    /// colour vanilla derives from the effects themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// One entry of a potion's `custom_effects` list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PotionEffect {
    /// Vanilla status-effect id (e.g. `minecraft:regeneration`), validated
    /// against the pinned registry (`DW0486`).
    pub effect: String,
    /// How long it lasts, in **ticks** (20 = one second). Required for every
    /// effect except the two [`INSTANT_EFFECTS`], which must NOT declare one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// Amplifier, 0 = level I (vanilla's unsigned byte, so 0–255). Absent = 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amplifier: Option<u32>,
}

impl PotionEffect {
    /// True if this effect is applied once on drinking rather than over time.
    pub fn is_instant(&self) -> bool {
        let norm = crate::registry::namespaced_effect_id(&self.effect);
        INSTANT_EFFECTS.contains(&norm.as_str())
    }
}

/// The largest `duration` a potion effect may declare, in ticks: 1 000 000 ticks
/// ≈ 13.9 hours, past the 10-hour delve ceiling, so nothing a delve can legally
/// need is refused — while a duration typed in *milliseconds*, or one that would
/// overflow vanilla's int, is caught (`DW0486`).
pub const MAX_POTION_DURATION_TICKS: u32 = 1_000_000;

/// The largest `amplifier` a potion effect may declare: vanilla stores it in an
/// unsigned byte, so 255 is not a policy but the end of the field.
pub const MAX_POTION_AMPLIFIER: u32 = 255;

/// The largest `seconds` a [`Verb::GiveEffect`] may declare, derived from
/// [`MAX_POTION_DURATION_TICKS`] rather than chosen again: the two are the same
/// quantity in different units, and a second independently-picked ceiling is how
/// two limits for one fact drift apart. ≈13.9 hours, past the 10-hour delve
/// ceiling, so nothing a delve can legally need is refused — while a duration
/// typed in *ticks* or in milliseconds is caught (`DW0541`).
pub const MAX_EFFECT_SECONDS: u32 = MAX_POTION_DURATION_TICKS / 20;
