//! The full pinned-MC item registry and prefab/anchor metadata, injected into
//! DSL validation via `validate_campaign_with` (spec-0002 / ADR-0004).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvewright_dsl::{AnchorRegistry, ItemRegistry, Lighting, PoolId, PrefabId};
use serde::Deserialize;

/// The complete 1.21.11 item registry (1505 ids), vendored under `data/`.
#[derive(Debug, Clone)]
pub struct FullItemRegistry {
    ids: BTreeSet<String>,
}

impl FullItemRegistry {
    /// Load the vendored 1.21.11 item registry (embedded at compile time).
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/items-1.21.11.json");
        let ids: Vec<String> =
            serde_json::from_str(raw).expect("vendored item registry is valid JSON");
        Self {
            ids: ids.into_iter().collect(),
        }
    }
}

impl ItemRegistry for FullItemRegistry {
    fn contains(&self, item_id: &str) -> bool {
        if self.ids.contains(item_id) {
            return true;
        }
        // Accept an un-namespaced id by assuming the default `minecraft:` namespace.
        if !item_id.contains(':') {
            return self.ids.contains(&format!("minecraft:{item_id}"));
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Prefab metadata (`prefabs/<name>.json`)
// ---------------------------------------------------------------------------

/// A prefab's `.json` metadata: structure reference + declared anchors + license.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefabMeta {
    /// The DSL prefab id (`prefab/<name>`).
    pub prefab_id: String,
    /// The structure-template reference.
    pub structure: StructureMeta,
    /// Named anchors, keyed by DSL anchor name (`spawn`, `anchor/…`).
    pub anchors: BTreeMap<String, AnchorMeta>,
    /// Declared lighting profile (spec-0001 "Lighting contract"). Typed as the
    /// DSL [`Lighting`] block; consumed by the `dark`-needs-mitigation analysis
    /// (`DW0210`, `analyze`). Optional so legacy metadata without it still loads.
    #[serde(default)]
    pub lighting: Option<Lighting>,
    /// License/provenance (opaque here; validated by review + `LICENSE-ASSETS.md`).
    #[serde(default)]
    pub license: serde_json::Value,
}

/// The structure-template reference of a prefab.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureMeta {
    /// The `.nbt` filename, relative to the prefab metadata file.
    pub file: String,
    /// The datapack structure id (path segment, e.g. `hello-room`).
    pub id: String,
    /// Structure extent `[x, y, z]`.
    pub size: [i32; 3],
    /// The MC data version the structure targets.
    pub data_version: i32,
    /// How the `.nbt` was generated (provenance breadcrumb).
    #[serde(default)]
    pub generator: Option<String>,
}

/// One anchor: either a point (`pos`, optional `facing`) or a gate (`region` of
/// `block`). Field presence distinguishes the two.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorMeta {
    /// Local point position (offset from the structure origin).
    #[serde(default)]
    pub pos: Option<[i32; 3]>,
    /// Facing keyword (`north`/`south`/`east`/`west`) for point anchors.
    #[serde(default)]
    pub facing: Option<String>,
    /// Local region (two inclusive corners) for gate anchors.
    #[serde(default)]
    pub region: Option<Region>,
    /// The block filling a gate region (e.g. `minecraft:iron_bars`).
    #[serde(default)]
    pub block: Option<String>,
}

/// An inclusive local block region (two corners).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    /// One corner (local coords).
    pub from: [i32; 3],
    /// The opposite corner (local coords).
    pub to: [i32; 3],
}

/// Loads and caches prefab metadata from a `prefabs/` directory, and answers
/// anchor / pool / lighting queries for DSL validation and analysis.
#[derive(Debug, Clone)]
pub struct PrefabRegistry {
    by_id: BTreeMap<String, PrefabMeta>,
    anchor_names: BTreeMap<String, BTreeSet<String>>,
    /// Declared prefab-pool ids. Populated from `pools.json` if present; jigsaw
    /// pool assembly lands fully in M2 task #9, so today this is typically empty
    /// (and any `prefab_pool` ref is reported unknown, `DW0161`).
    pools: BTreeSet<String>,
}

impl PrefabRegistry {
    /// Load every `*.json` prefab metadata file in `dir`. Files that do not parse
    /// as [`PrefabMeta`] are skipped (e.g. unrelated docs). An optional
    /// `pools.json` (a JSON array of `pool/<name>` ids) declares prefab pools.
    /// Returns the loaded registry; errors only on an unreadable directory.
    pub fn load_dir(dir: &Path) -> std::io::Result<Self> {
        let mut by_id: BTreeMap<String, PrefabMeta> = BTreeMap::new();
        let mut anchor_names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut pools: BTreeSet<String> = BTreeSet::new();
        // Sort entries for deterministic load order.
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            if path.file_name().and_then(|n| n.to_str()) == Some("pools.json") {
                if let Ok(ids) = serde_json::from_str::<Vec<String>>(&raw) {
                    pools.extend(ids);
                }
                continue;
            }
            if let Ok(meta) = serde_json::from_str::<PrefabMeta>(&raw) {
                let names: BTreeSet<String> = meta.anchors.keys().cloned().collect();
                anchor_names.insert(meta.prefab_id.clone(), names);
                by_id.insert(meta.prefab_id.clone(), meta);
            }
        }
        Ok(Self {
            by_id,
            anchor_names,
            pools,
        })
    }

    /// The metadata for a prefab id (`prefab/<name>`), if loaded.
    pub fn get(&self, prefab_id: &str) -> Option<&PrefabMeta> {
        self.by_id.get(prefab_id)
    }
}

impl AnchorRegistry for PrefabRegistry {
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>> {
        self.anchor_names.get(prefab.as_str())
    }

    fn has_pool(&self, pool: &PoolId) -> bool {
        self.pools.contains(pool.as_str())
    }

    fn lighting_for(&self, prefab: &PrefabId) -> Option<Lighting> {
        self.by_id
            .get(prefab.as_str())
            .and_then(|m| m.lighting.clone())
    }
}
