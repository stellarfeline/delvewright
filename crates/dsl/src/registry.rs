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
///
/// The first three are **measurements**, taken by the live probe loop. The
/// fourth, [`LightingProfile::Unmeasured`], is the honest absence of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LightingProfile {
    /// Floor light ≥ 7 (default requirement).
    Lit,
    /// Floor light 3–6, with an atmosphere rationale.
    Dim,
    /// Floor light < 3, only usable with a proven mitigation.
    Dark,
    /// The piece has never been probed, and says so.
    ///
    /// A generated prefab (spec-0027's grammar back end) knows where it put
    /// blocks and nothing about the light they end up in, so it declares this
    /// rather than fabricating a number. It is *not* a synonym for "no
    /// `lighting` block at all": absence means legacy metadata that predates the
    /// field, while this is a positive statement that the measurement is owed.
    /// An unmeasured piece carries no measurement fields — a claim and its
    /// absence cannot both be true — and is treated as not-`lit` everywhere a
    /// profile is consumed.
    Unmeasured,
}

impl LightingProfile {
    /// True for the three profiles that assert a measured light level.
    pub fn is_measurement(self) -> bool {
        !matches!(self, LightingProfile::Unmeasured)
    }
}

/// A prefab's declared `lighting` metadata block (measured once at library
/// admission). Field names match `prefabs/<name>.json`.
///
/// `measured_min_light` / `measured` are `Option` in the type but **not**
/// optional in the data: a measured profile without them, or an `unmeasured`
/// profile with them, is refused at deserialisation (see the hand-written
/// `Deserialize` below). The optionality expresses which profile is being
/// declared, never a licence to omit a measurement that was claimed.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Lighting {
    /// The declared profile.
    pub profile: LightingProfile,
    /// The measured minimum floor light level. Present iff the profile is a
    /// measurement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_min_light: Option<i64>,
    /// The date the level was measured (`YYYY-MM-DD`). Present iff the profile
    /// is a measurement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured: Option<String>,
    /// Why `dim`/`dark` was chosen (required for `dim`/`dark` by review).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// How the minimum was measured (provenance breadcrumb; optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

impl Lighting {
    /// A never-probed declaration — what a generated prefab exports.
    pub fn unmeasured() -> Lighting {
        Lighting {
            profile: LightingProfile::Unmeasured,
            measured_min_light: None,
            measured: None,
            rationale: None,
            method: None,
        }
    }
}

/// The wire shape of [`Lighting`], before the profile/measurement agreement is
/// checked. Split out so the check runs on every deserialisation path there is.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLighting {
    profile: LightingProfile,
    #[serde(default)]
    measured_min_light: Option<i64>,
    #[serde(default)]
    measured: Option<String>,
    #[serde(default)]
    rationale: Option<String>,
    #[serde(default)]
    method: Option<String>,
}

impl<'de> Deserialize<'de> for Lighting {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Lighting, D::Error> {
        use serde::de::Error as _;
        let raw = RawLighting::deserialize(de)?;
        let has_measurement = raw.measured_min_light.is_some() && raw.measured.is_some();
        if raw.profile.is_measurement() {
            // Exactly the requirement the previously-mandatory fields enforced:
            // claiming `lit`/`dim`/`dark` means claiming a probe ran.
            if !has_measurement {
                return Err(D::Error::custom(
                    "a `lit`/`dim`/`dark` lighting profile is a measurement and must carry both \
                     `measured_min_light` and `measured`; declare `\"profile\": \"unmeasured\"` \
                     if the piece has not been probed yet",
                ));
            }
        } else if raw.measured_min_light.is_some() || raw.measured.is_some() {
            return Err(D::Error::custom(
                "an `unmeasured` lighting profile cannot carry `measured_min_light` or \
                 `measured` — declare the profile the probe actually found instead",
            ));
        }
        Ok(Lighting {
            profile: raw.profile,
            measured_min_light: raw.measured_min_light,
            measured: raw.measured,
            rationale: raw.rationale,
            method: raw.method,
        })
    }
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

/// A status-effect id in its canonical namespaced form: `blindness` and
/// `minecraft:blindness` are the same effect to vanilla and must be the same
/// effect to the compiler.
///
/// One spelling for one fact. This normalization existed twice — inside
/// [`VendoredEffectRegistry::contains`] and inside
/// [`crate::stages::MobEffect::is_instant`] — before `DW0540` needed a third
/// copy to decide whether a `clear-effect` removes the effect a `give-effect`
/// granted. Two ids that the registry accepts as the same must not be two ids to
/// a rule that pairs them, and a private third copy is how they drift apart.
pub fn namespaced_effect_id(id: &str) -> String {
    if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    }
}

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
        self.ids.contains(namespaced_effect_id(effect_id).as_str())
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

#[cfg(test)]
mod lighting_tests {
    use super::*;

    /// A prefab's lighting profile is a claim about a measurement, so the
    /// measurement must be there. This was enforced by two mandatory fields
    /// until `unmeasured` needed them absent; the rule itself did not move.
    #[test]
    fn a_measured_profile_still_cannot_omit_its_measurement() {
        for json in [
            r#"{ "profile": "lit" }"#,
            r#"{ "profile": "lit", "measured_min_light": 9 }"#,
            r#"{ "profile": "lit", "measured": "2026-07-30" }"#,
            r#"{ "profile": "dim", "measured": "2026-07-30" }"#,
            r#"{ "profile": "dark", "measured_min_light": 1 }"#,
        ] {
            let err = serde_json::from_str::<Lighting>(json).unwrap_err();
            assert!(
                err.to_string().contains("must carry both"),
                "{json} was accepted or misreported: {err}"
            );
        }
    }

    /// ...and the converse, which is the new way to lie: declaring the probe
    /// never ran while quoting what it found.
    #[test]
    fn an_unmeasured_profile_cannot_quote_a_measurement() {
        for json in [
            r#"{ "profile": "unmeasured", "measured_min_light": 9 }"#,
            r#"{ "profile": "unmeasured", "measured": "2026-07-30" }"#,
            r#"{ "profile": "unmeasured", "measured_min_light": 9, "measured": "2026-07-30" }"#,
        ] {
            let err = serde_json::from_str::<Lighting>(json).unwrap_err();
            assert!(err.to_string().contains("cannot carry"), "{json}: {err}");
        }
    }

    /// The two shapes that are true: a hand-probed piece and a generated one.
    #[test]
    fn both_honest_declarations_round_trip() {
        let measured: Lighting = serde_json::from_str(
            r#"{ "profile": "lit", "measured_min_light": 8, "measured": "2026-07-30",
                 "method": "live 1.21.11 probe" }"#,
        )
        .unwrap();
        assert_eq!(measured.profile, LightingProfile::Lit);
        assert_eq!(measured.measured_min_light, Some(8));
        assert!(measured.profile.is_measurement());

        let unmeasured: Lighting = serde_json::from_str(r#"{ "profile": "unmeasured" }"#).unwrap();
        assert_eq!(unmeasured, Lighting::unmeasured());
        assert!(!unmeasured.profile.is_measurement());

        // Serialisation omits what is absent rather than writing `null`, so a
        // re-serialised file is the file that was read.
        for l in [measured, unmeasured] {
            let text = serde_json::to_string(&l).unwrap();
            assert!(!text.contains("null"), "{text}");
            assert_eq!(serde_json::from_str::<Lighting>(&text).unwrap(), l);
        }
    }

    /// An unknown profile string is still refused outright — `unmeasured` is a
    /// named state, not an escape hatch for anything the reader does not know.
    #[test]
    fn an_unknown_profile_is_still_refused() {
        assert!(serde_json::from_str::<Lighting>(r#"{ "profile": "unknown" }"#).is_err());
        assert!(
            serde_json::from_str::<Lighting>(
                r#"{ "profile": "lit", "measured_min_light": 8, "measured": "x", "extra": 1 }"#
            )
            .is_err(),
            "deny_unknown_fields must survive the hand-written Deserialize"
        );
    }
}
