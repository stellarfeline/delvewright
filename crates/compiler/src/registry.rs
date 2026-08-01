//! The full pinned-MC item registry and prefab/anchor metadata, injected into
//! DSL validation via `validate_campaign_with` (spec-0002 / ADR-0004).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvewright_dsl::{AnchorRegistry, EntityRegistry, ItemRegistry, Lighting, PoolId, PrefabId};
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

/// The complete 1.21.11 entity-type registry (157 ids), vendored under `data/`
/// from the same `misode/mcmeta` summary as the item registry (see
/// `data/PROVENANCE.md`). Validates v0.3 wave mobs (`DW0173`).
#[derive(Debug, Clone)]
pub struct FullEntityRegistry {
    ids: BTreeSet<String>,
}

impl FullEntityRegistry {
    /// Load the vendored 1.21.11 entity registry (embedded at compile time).
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/entities-1.21.11.json");
        let ids: Vec<String> =
            serde_json::from_str(raw).expect("vendored entity registry is valid JSON");
        Self {
            ids: ids.into_iter().collect(),
        }
    }
}

impl EntityRegistry for FullEntityRegistry {
    fn contains(&self, entity_id: &str) -> bool {
        if self.ids.contains(entity_id) {
            return true;
        }
        if !entity_id.contains(':') {
            return self.ids.contains(&format!("minecraft:{entity_id}"));
        }
        false
    }
}

/// The complete 1.21.11 `sound_event` registry (1838 ids), vendored under `data/`
/// from the same `misode/mcmeta` summary as the item/entity registries (see
/// `data/PROVENANCE.md`; regenerate with `tools/extract-sound-registry.py`).
/// Validates v0.6 `play-sound` / v0.4 `narrate.sound` ids (`DW0326`, spec-0014).
#[derive(Debug, Clone)]
pub struct FullSoundRegistry {
    ids: BTreeSet<String>,
}

impl FullSoundRegistry {
    /// Load the vendored 1.21.11 sound-event registry (embedded at compile time).
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/sounds-1.21.11.json");
        let ids: Vec<String> =
            serde_json::from_str(raw).expect("vendored sound registry is valid JSON");
        Self {
            ids: ids.into_iter().collect(),
        }
    }

    /// True if `sound_id` is a known sound event in the pinned MC version. An
    /// un-namespaced id is resolved under the default `minecraft:` namespace, as
    /// the `playsound` command accepts.
    pub fn contains(&self, sound_id: &str) -> bool {
        if self.ids.contains(sound_id) {
            return true;
        }
        if !sound_id.contains(':') {
            return self.ids.contains(&format!("minecraft:{sound_id}"));
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
    /// keep-socket-v1 connectors (jigsaw sockets). Empty for single-piece prefabs
    /// (e.g. `hello-room`); the layout solver (`crate::solver`) mates these when
    /// assembling a `prefab_pool` area (M2 task #9). Optional so single-piece
    /// metadata without them still loads.
    #[serde(default)]
    pub connectors: Vec<Connector>,
    /// Declared lighting profile (spec-0001 "Lighting contract"). Typed as the
    /// DSL [`Lighting`] block; consumed by the `dark`-needs-mitigation analysis
    /// (`DW0210`, `analyze`). Optional so legacy metadata without it still loads.
    #[serde(default)]
    pub lighting: Option<Lighting>,
    /// License/provenance (opaque here; validated by review + `LICENSE-ASSETS.md`).
    #[serde(default)]
    pub license: serde_json::Value,
}

/// One keep-socket-v1 connector (a jigsaw doorway) declared by a prefab. See
/// `prefabs/keep-tileset.md` "Connection convention". `local_pos` is the socket's
/// wall cell (bottom-centre of the 3×3 opening) in the prefab's local coordinates;
/// `facing` is the cardinal direction the opening faces outward. The solver mates
/// two sockets by placing the child so its socket sits one block beyond the
/// parent socket, facing the opposite way (see `crate::solver`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connector {
    /// Jigsaw `name` (uniform `keep:socket` in keep-socket-v1).
    pub name: String,
    /// Jigsaw `target` (uniform `keep:socket`).
    pub target: String,
    /// The socket's wall cell, local coords `[x, y, z]`.
    pub local_pos: [i32; 3],
    /// Cardinal direction the opening faces (`north`/`south`/`east`/`west`).
    pub facing: String,
    /// Opening extent `[width, height]` (3×3 in keep-socket-v1).
    pub opening: [i32; 2],
    /// Jigsaw joint (`aligned` in keep-socket-v1).
    pub joint: String,
}

/// One member of a prefab pool: a prefab id, a weight, and a layout role. Roles
/// (`entry`, `connector`, `room`, `terminal`) steer the solver — `entry` seeds
/// the layout, `connector` fills the spine, `room`/`terminal` carry anchors and
/// are placed only when a campaign-referenced anchor requires them.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolMember {
    /// The member prefab id (`prefab/<name>`).
    pub prefab: String,
    /// Selection weight (higher = more likely). Defaults to 1.
    #[serde(default = "one")]
    pub weight: u32,
    /// Layout role: `entry` | `connector` | `room` | `terminal`.
    pub role: String,
}

fn one() -> u32 {
    1
}

/// The on-disk `prefabs/pools.json` shape: pool id → members.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolsFile {
    pools: BTreeMap<String, PoolDef>,
}

/// One pool definition (its member list).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolDef {
    members: Vec<PoolMember>,
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
    /// Declared prefab-pool ids (the keys of `pools.json`). A `prefab_pool` ref
    /// to a pool absent here is reported unknown (`DW0161`).
    pools: BTreeSet<String>,
    /// Pool id → its member pieces (weights + roles), consumed by the solver.
    pool_members: BTreeMap<String, Vec<PoolMember>>,
}

impl PrefabRegistry {
    /// Load every `*.json` prefab metadata file in `dir`. Files that do not parse
    /// as [`PrefabMeta`] are skipped (e.g. unrelated docs). An optional
    /// `pools.json` (`{ "pools": { "pool/<name>": { "members": [...] } } }`)
    /// declares prefab pools and their member pieces. Returns the loaded
    /// registry; errors only on an unreadable directory.
    pub fn load_dir(dir: &Path) -> std::io::Result<Self> {
        let mut by_id: BTreeMap<String, PrefabMeta> = BTreeMap::new();
        let mut anchor_names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut pools: BTreeSet<String> = BTreeSet::new();
        let mut pool_members: BTreeMap<String, Vec<PoolMember>> = BTreeMap::new();
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
                if let Ok(file) = serde_json::from_str::<PoolsFile>(&raw) {
                    for (id, def) in file.pools {
                        pools.insert(id.clone());
                        pool_members.insert(id, def.members);
                    }
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
            pool_members,
        })
    }

    /// The metadata for a prefab id (`prefab/<name>`), if loaded.
    pub fn get(&self, prefab_id: &str) -> Option<&PrefabMeta> {
        self.by_id.get(prefab_id)
    }

    /// The member pieces of a prefab pool (`pool/<name>`), if declared.
    pub fn pool(&self, pool_id: &str) -> Option<&[PoolMember]> {
        self.pool_members.get(pool_id).map(|v| v.as_slice())
    }

    /// The prefab ids in `pool_id` that declare `anchor_name` in their metadata.
    pub fn pool_prefabs_with_anchor(&self, pool_id: &str, anchor_name: &str) -> Vec<String> {
        let Some(members) = self.pool_members.get(pool_id) else {
            return Vec::new();
        };
        members
            .iter()
            .filter(|m| {
                self.by_id
                    .get(&m.prefab)
                    .is_some_and(|meta| meta.anchors.contains_key(anchor_name))
            })
            .map(|m| m.prefab.clone())
            .collect()
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
