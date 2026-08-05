//! Registries for data that lives outside the DSL: the vanilla item registry and
//! prefab anchor metadata.
//!
//! v0 ships small vendored lists covering only what the M1 hello-world fixtures
//! use. The **full** 1.21.11 item registry and the real prefab anchor metadata
//! are vendored/loaded by the compiler (spec-0002 / ADR-0004); the compiler
//! injects them via [`crate::validate::validate_campaign_with`].

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{PoolId, PrefabId};

/// Membership test for vanilla item ids (`minecraft:iron_sword`, …).
pub trait ItemRegistry {
    /// True if `item_id` is a known item in the pinned MC version.
    fn contains(&self, item_id: &str) -> bool;

    /// The item's `minecraft:max_stack_size` in the pinned MC version, if this
    /// registry carries stack-size data for it.
    ///
    /// Needed because a single-slot fill (`item replace block … container.<n> with
    /// <item> <count>`) fails **silently** above the cap — rabbit stew caps at 1, so
    /// `count: 2` puts nothing in the chest (`DW0436`). `None` means "this registry
    /// does not know", and the check is then skipped rather than guessed: the small
    /// vendored DSL-side subset carries ids only, while the compiler injects the full
    /// 1.21.11 table (`crates/compiler/data/item-stack-sizes-1.21.11.json`).
    fn max_stack_size(&self, _item_id: &str) -> Option<u32> {
        None
    }
}

/// Membership test for vanilla entity ids (`minecraft:zombie`, …), used to
/// validate stage-5 wave mobs (DSL v0.3). Like [`ItemRegistry`], the crate ships
/// a small vendored subset; the compiler injects the full 1.21.11 entity registry
/// via [`crate::validate::validate_campaign_with`].
pub trait EntityRegistry {
    /// True if `entity_id` is a known entity in the pinned MC version.
    fn contains(&self, entity_id: &str) -> bool;
}

/// A prefab lighting profile (spec-0001 "Lighting contract"). `lit` = floor
/// light ≥ 7; `dim` = 3–6 (needs a rationale); `dark` = < 3 (valid only where
/// analysis proves a night-vision mitigation — the compiler's `DW0210` check).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LightingProfile {
    /// Floor light ≥ 7 (default requirement).
    Lit,
    /// Floor light 3–6, with an atmosphere rationale.
    Dim,
    /// Floor light < 3, only usable with a proven mitigation.
    Dark,
}

/// A prefab's declared `lighting` metadata block (measured once at library
/// admission). Field names match `prefabs/<name>.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Lighting {
    /// The declared profile.
    pub profile: LightingProfile,
    /// The measured minimum floor light level.
    pub measured_min_light: i64,
    /// The date the level was measured (`YYYY-MM-DD`).
    pub measured: String,
    /// Why `dim`/`dark` was chosen (required for `dim`/`dark` by review).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// How the minimum was measured (provenance breadcrumb; optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

/// The prefab-metadata surface DSL validation resolves refs against: declared
/// anchors, prefab-pool existence, and lighting profiles. v0 ships a small
/// vendored implementation; the compiler injects the real one from `prefabs/`.
pub trait AnchorRegistry {
    /// The DSL anchor ids (`anchor/exit`, …) the prefab provides, or `None` if
    /// the prefab is unknown to this registry (defer to the compiler).
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>>;

    /// True if a prefab pool with this id exists in the metadata surface.
    fn has_pool(&self, _pool: &PoolId) -> bool {
        false
    }

    /// The declared lighting for a prefab, if known.
    fn lighting_for(&self, _prefab: &PrefabId) -> Option<Lighting> {
        None
    }
}

/// Vendored item registry loaded from an embedded JSON array of item ids.
#[derive(Debug, Clone)]
pub struct VendoredItemRegistry {
    ids: BTreeSet<String>,
}

impl VendoredItemRegistry {
    /// The v0 subset of the 1.21.11 item registry used by the M1 fixtures, plus
    /// the four [`crate::stages::POTION_BEARING_ITEMS`] — a kit item's potion
    /// `contents` is DSL surface with its own rules (`DW0486`/`DW0487`), so the
    /// crate's own tests must be able to name the items those rules are about
    /// without the compiler's full injected registry.
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/items-1.21.11.json");
        let ids: Vec<String> =
            serde_json::from_str(raw).expect("embedded item registry is valid JSON");
        Self {
            ids: ids.into_iter().collect(),
        }
    }
}

impl ItemRegistry for VendoredItemRegistry {
    fn contains(&self, item_id: &str) -> bool {
        self.ids.contains(item_id)
    }
}

/// Vendored entity registry loaded from an embedded JSON array of entity ids
/// (the common hostile mobs used by v0.3 wave fixtures).
#[derive(Debug, Clone)]
pub struct VendoredEntityRegistry {
    ids: BTreeSet<String>,
}

impl VendoredEntityRegistry {
    /// The v0 subset of the 1.21.11 entity registry used by wave fixtures.
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/entities-1.21.11.json");
        let ids: Vec<String> =
            serde_json::from_str(raw).expect("embedded entity registry is valid JSON");
        Self {
            ids: ids.into_iter().collect(),
        }
    }
}

impl EntityRegistry for VendoredEntityRegistry {
    fn contains(&self, entity_id: &str) -> bool {
        self.ids.contains(entity_id)
    }
}

// ---------------------------------------------------------------------------
// DSL v0.4 registries: blocks (props / set-block) and status effects
// ---------------------------------------------------------------------------

/// Membership test for vanilla block ids (`minecraft:lever`, …), used to
/// validate DSL v0.4 `set-block` / `interact.prop` block ids (`DW0193`).
pub trait BlockRegistry {
    /// True if `block_id` is a known placeable block in the pinned MC version.
    fn contains(&self, block_id: &str) -> bool;
}

/// The complete 1.21.11 **enchantment** id list. Same rationale as
/// [`EFFECT_IDS_1_21_11`]: 43 ids, stable across the pinned MC version, so it is
/// inlined rather than vendored as a data file and injected. Extracted from the
/// `enchantment` registry of the same misode/mcmeta 1.21.11 registries summary
/// the item/entity registries come from (SHA-256
/// `7efb184902cfef62b431bc9826ebcbcde2c23746e5624326ffcf922e15cf28f9`, pinned in
/// `crates/compiler/data/PROVENANCE.md`) — Mojang's own generated data, not a
/// third-party reconstruction.
pub const ENCHANTMENT_IDS_1_21_11: &[&str] = &[
    "minecraft:aqua_affinity",
    "minecraft:bane_of_arthropods",
    "minecraft:binding_curse",
    "minecraft:blast_protection",
    "minecraft:breach",
    "minecraft:channeling",
    "minecraft:density",
    "minecraft:depth_strider",
    "minecraft:efficiency",
    "minecraft:feather_falling",
    "minecraft:fire_aspect",
    "minecraft:fire_protection",
    "minecraft:flame",
    "minecraft:fortune",
    "minecraft:frost_walker",
    "minecraft:impaling",
    "minecraft:infinity",
    "minecraft:knockback",
    "minecraft:looting",
    "minecraft:loyalty",
    "minecraft:luck_of_the_sea",
    "minecraft:lunge",
    "minecraft:lure",
    "minecraft:mending",
    "minecraft:multishot",
    "minecraft:piercing",
    "minecraft:power",
    "minecraft:projectile_protection",
    "minecraft:protection",
    "minecraft:punch",
    "minecraft:quick_charge",
    "minecraft:respiration",
    "minecraft:riptide",
    "minecraft:sharpness",
    "minecraft:silk_touch",
    "minecraft:smite",
    "minecraft:soul_speed",
    "minecraft:sweeping_edge",
    "minecraft:swift_sneak",
    "minecraft:thorns",
    "minecraft:unbreaking",
    "minecraft:vanishing_curse",
    "minecraft:wind_burst",
];

/// Membership test for vanilla enchantment ids (`minecraft:sharpness`, …), used
/// to validate actor/wave `equipment` and `loot` enchantments (`DW0433`).
pub trait EnchantmentRegistry {
    /// True if `enchantment_id` is a known enchantment in the pinned MC version.
    fn contains(&self, enchantment_id: &str) -> bool;
}

/// Vendored enchantment registry built from [`ENCHANTMENT_IDS_1_21_11`].
#[derive(Debug, Clone)]
pub struct VendoredEnchantmentRegistry {
    ids: BTreeSet<&'static str>,
}

impl VendoredEnchantmentRegistry {
    /// The full 1.21.11 enchantment registry.
    pub fn v1_21_11() -> Self {
        Self {
            ids: ENCHANTMENT_IDS_1_21_11.iter().copied().collect(),
        }
    }
}

impl Default for VendoredEnchantmentRegistry {
    fn default() -> Self {
        Self::v1_21_11()
    }
}

impl EnchantmentRegistry for VendoredEnchantmentRegistry {
    fn contains(&self, enchantment_id: &str) -> bool {
        let norm = if enchantment_id.contains(':') {
            enchantment_id.to_string()
        } else {
            format!("minecraft:{enchantment_id}")
        };
        self.ids.contains(norm.as_str())
    }
}

/// The complete 1.21.11 **potion** id list — the `potion` registry a
/// `minecraft:potion_contents` component's `potion` field names
/// (`minecraft:strong_healing`, …). 46 ids, extracted from the same
/// misode/mcmeta 1.21.11 registries summary the item/entity/enchantment lists
/// come from (SHA-256
/// `7efb184902cfef62b431bc9826ebcbcde2c23746e5624326ffcf922e15cf28f9`, pinned in
/// `crates/compiler/data/PROVENANCE.md`) — Mojang's own generated data.
///
/// Complete and stable for the pinned version, so unlike the item/entity
/// registries there is **nothing for the compiler to inject**: this const IS the
/// registry, and [`is_potion_id`] is the whole membership test (same treatment as
/// [`TECHNICAL_BLOCK_IDS`]).
pub const POTION_IDS_1_21_11: &[&str] = &[
    "minecraft:awkward",
    "minecraft:fire_resistance",
    "minecraft:harming",
    "minecraft:healing",
    "minecraft:infested",
    "minecraft:invisibility",
    "minecraft:leaping",
    "minecraft:long_fire_resistance",
    "minecraft:long_invisibility",
    "minecraft:long_leaping",
    "minecraft:long_night_vision",
    "minecraft:long_poison",
    "minecraft:long_regeneration",
    "minecraft:long_slow_falling",
    "minecraft:long_slowness",
    "minecraft:long_strength",
    "minecraft:long_swiftness",
    "minecraft:long_turtle_master",
    "minecraft:long_water_breathing",
    "minecraft:long_weakness",
    "minecraft:luck",
    "minecraft:mundane",
    "minecraft:night_vision",
    "minecraft:oozing",
    "minecraft:poison",
    "minecraft:regeneration",
    "minecraft:slow_falling",
    "minecraft:slowness",
    "minecraft:strength",
    "minecraft:strong_harming",
    "minecraft:strong_healing",
    "minecraft:strong_leaping",
    "minecraft:strong_poison",
    "minecraft:strong_regeneration",
    "minecraft:strong_slowness",
    "minecraft:strong_strength",
    "minecraft:strong_swiftness",
    "minecraft:strong_turtle_master",
    "minecraft:swiftness",
    "minecraft:thick",
    "minecraft:turtle_master",
    "minecraft:water",
    "minecraft:water_breathing",
    "minecraft:weakness",
    "minecraft:weaving",
    "minecraft:wind_charged",
];

/// True if `id` (optionally un-namespaced) is a 1.21.11 potion id.
pub fn is_potion_id(id: &str) -> bool {
    let norm = if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    };
    POTION_IDS_1_21_11.contains(&norm.as_str())
}

/// Membership test for vanilla status-effect ids (`minecraft:slowness`, …),
/// used to validate DSL v0.4 wave-mob `effects` (`DW0192`) and a kit item's
/// potion `contents` effects (`DW0486`).
pub trait EffectRegistry {
    /// True if `effect_id` is a known status effect in the pinned MC version.
    fn contains(&self, effect_id: &str) -> bool;
}

/// The complete 1.21.11 mob status-effect id list (the canonical source both the
/// vendored and full [`EffectRegistry`]s build from). Small, stable registry, so
/// it is inlined rather than a data file.
pub const EFFECT_IDS_1_21_11: &[&str] = &[
    "minecraft:speed",
    "minecraft:slowness",
    "minecraft:haste",
    "minecraft:mining_fatigue",
    "minecraft:strength",
    "minecraft:instant_health",
    "minecraft:instant_damage",
    "minecraft:jump_boost",
    "minecraft:nausea",
    "minecraft:regeneration",
    "minecraft:resistance",
    "minecraft:fire_resistance",
    "minecraft:water_breathing",
    "minecraft:invisibility",
    "minecraft:blindness",
    "minecraft:night_vision",
    "minecraft:hunger",
    "minecraft:weakness",
    "minecraft:poison",
    "minecraft:wither",
    "minecraft:health_boost",
    "minecraft:absorption",
    "minecraft:saturation",
    "minecraft:glowing",
    "minecraft:levitation",
    "minecraft:luck",
    "minecraft:unluck",
    "minecraft:slow_falling",
    "minecraft:conduit_power",
    "minecraft:dolphins_grace",
    "minecraft:bad_omen",
    "minecraft:hero_of_the_village",
    "minecraft:darkness",
    "minecraft:trial_omen",
    "minecraft:raid_omen",
    "minecraft:wind_charged",
    "minecraft:weaving",
    "minecraft:oozing",
    "minecraft:infested",
    // Registry-fidelity fix (2026-08-03): the pinned 1.21.11 `mob_effect`
    // registry has 40 entries and this list had 39 — the one 1.21.11 addition
    // was missing, so a campaign naming it was rejected as unknown by a check
    // that was simply out of date. Re-derived from the same pinned summary
    // (SHA-256 `7efb1849…`) the potion list above comes from.
    "minecraft:breath_of_the_nautilus",
];

/// Technical / fluid blocks that are NOT items but are valid `set-block` targets
/// (`air` clears a cell; fluids fill one). Block validation otherwise reuses the
/// item registry — every placeable *affordance* block a prop/set-block would use
/// (lever, button, chest, door, pressure plate, …) is an item in 1.21.11, so an
/// item-registry membership test is a sound, false-reject-free block check; this
/// allowlist covers the handful of placeable blocks that have no item form.
pub const TECHNICAL_BLOCK_IDS: &[&str] = &[
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
    "minecraft:water",
    "minecraft:lava",
];

/// True if `id` (optionally un-namespaced) is a technical/fluid block.
pub fn is_technical_block(id: &str) -> bool {
    let norm = if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    };
    TECHNICAL_BLOCK_IDS.contains(&norm.as_str())
}

/// A [`BlockRegistry`] backed by an [`ItemRegistry`] plus the technical-block
/// allowlist. Blocks that have an item form (the placeable affordances) resolve
/// through the item registry; `air`/fluids resolve through the allowlist.
pub struct ItemBackedBlockRegistry<'a> {
    items: &'a dyn ItemRegistry,
}

impl<'a> ItemBackedBlockRegistry<'a> {
    /// Wrap an item registry as a block registry.
    pub fn new(items: &'a dyn ItemRegistry) -> Self {
        Self { items }
    }
}

impl BlockRegistry for ItemBackedBlockRegistry<'_> {
    fn contains(&self, block_id: &str) -> bool {
        is_technical_block(block_id) || self.items.contains(block_id)
    }
}

/// Vendored status-effect registry built from [`EFFECT_IDS_1_21_11`].
#[derive(Debug, Clone)]
pub struct VendoredEffectRegistry {
    ids: BTreeSet<&'static str>,
}

impl VendoredEffectRegistry {
    /// The full 1.21.11 status-effect registry.
    pub fn v1_21_11() -> Self {
        Self {
            ids: EFFECT_IDS_1_21_11.iter().copied().collect(),
        }
    }
}

impl Default for VendoredEffectRegistry {
    fn default() -> Self {
        Self::v1_21_11()
    }
}

impl EffectRegistry for VendoredEffectRegistry {
    fn contains(&self, effect_id: &str) -> bool {
        let norm = if effect_id.contains(':') {
            effect_id.to_string()
        } else {
            format!("minecraft:{effect_id}")
        };
        self.ids.contains(norm.as_str())
    }
}

/// Vendored anchor registry loaded from embedded prefab metadata.
#[derive(Debug, Clone)]
pub struct VendoredAnchorRegistry {
    by_prefab: BTreeMap<String, BTreeSet<String>>,
    pools: BTreeSet<String>,
}

impl VendoredAnchorRegistry {
    /// The anchors declared by the M1 hello-world prefab(s), plus a fixture pool
    /// so prefab-pool existence checks are exercised (pools land fully in M2
    /// task #9).
    pub fn hello_world() -> Self {
        let raw = include_str!("../data/anchors.json");
        let by_prefab: BTreeMap<String, BTreeSet<String>> =
            serde_json::from_str(raw).expect("embedded anchor metadata is valid JSON");
        let pools_raw = include_str!("../data/pools.json");
        let pools: BTreeSet<String> =
            serde_json::from_str(pools_raw).expect("embedded pool metadata is valid JSON");
        Self { by_prefab, pools }
    }
}

impl AnchorRegistry for VendoredAnchorRegistry {
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>> {
        self.by_prefab.get(prefab.as_str())
    }

    fn has_pool(&self, pool: &PoolId) -> bool {
        self.pools.contains(pool.as_str())
    }
}
