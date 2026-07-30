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
    /// The v0 subset of the 1.21.11 item registry used by the M1 fixtures.
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
