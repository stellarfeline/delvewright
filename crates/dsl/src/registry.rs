//! Registries for data that lives outside the DSL: the vanilla item registry and
//! prefab anchor metadata.
//!
//! v0 ships small vendored lists covering only what the M1 hello-world fixtures
//! use. The **full** 1.21.11 item registry and the real prefab anchor metadata
//! are vendored/loaded by the compiler (spec-0002 / ADR-0004); the compiler
//! injects them via [`crate::validate::validate_campaign_with`].

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::PrefabId;

/// Membership test for vanilla item ids (`minecraft:iron_sword`, …).
pub trait ItemRegistry {
    /// True if `item_id` is a known item in the pinned MC version.
    fn contains(&self, item_id: &str) -> bool;
}

/// The named anchors a prefab declares.
pub trait AnchorRegistry {
    /// The DSL anchor ids (`anchor/exit`, …) the prefab provides, or `None` if
    /// the prefab is unknown to this registry (defer to the compiler).
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>>;
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
}

impl VendoredAnchorRegistry {
    /// The anchors declared by the M1 hello-world prefab(s).
    pub fn hello_world() -> Self {
        let raw = include_str!("../data/anchors.json");
        let by_prefab: BTreeMap<String, BTreeSet<String>> =
            serde_json::from_str(raw).expect("embedded anchor metadata is valid JSON");
        Self { by_prefab }
    }
}

impl AnchorRegistry for VendoredAnchorRegistry {
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>> {
        self.by_prefab.get(prefab.as_str())
    }
}
