//! The full pinned-MC item registry and prefab/anchor metadata, injected into
//! DSL validation via `validate_campaign_with` (spec-0002 / ADR-0004).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvewright_dsl::{
    AnchorRegistry, Diagnostic, EntityRegistry, ItemRegistry, Lighting, PoolId, PrefabId,
};
use serde::Deserialize;

/// `DW0346`: a prefab metadata `*.json` (or `pools.json`) in the prefabs dir
/// failed to read or parse as prefab metadata. Silently skipping it made an
/// older `delvec` meeting newer metadata surface only as a baffling downstream
/// `DW0300` "prefab not found"; the parse failure itself is the information.
/// Reported at **validation tier (exit 1)**; loading continues for the other
/// files (report-all, not fail-fast).
pub const DW_PREFAB_META_INVALID: &str = "DW0346";

/// The complete 1.21.11 item registry (1505 ids) plus each item's
/// `minecraft:max_stack_size`, vendored under `data/`.
#[derive(Debug, Clone)]
pub struct FullItemRegistry {
    ids: BTreeSet<String>,
    /// Item id → default `minecraft:max_stack_size` (1, 16 or 64 in 1.21.11).
    /// Mojang's own data, regenerated per MC pin by
    /// `tools/extract-item-stack-sizes.py` — never a hand-maintained table.
    stack_sizes: BTreeMap<String, u32>,
}

impl FullItemRegistry {
    /// Load the vendored 1.21.11 item registry (embedded at compile time).
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/items-1.21.11.json");
        let ids: Vec<String> =
            serde_json::from_str(raw).expect("vendored item registry is valid JSON");
        let raw_sizes = include_str!("../data/item-stack-sizes-1.21.11.json");
        let stack_sizes: BTreeMap<String, u32> =
            serde_json::from_str(raw_sizes).expect("vendored item stack sizes are valid JSON");
        Self {
            ids: ids.into_iter().collect(),
            stack_sizes,
        }
    }

    /// Resolve an id to its canonical namespaced form, if known.
    fn canonical(&self, item_id: &str) -> Option<String> {
        if self.ids.contains(item_id) {
            return Some(item_id.to_string());
        }
        // Accept an un-namespaced id by assuming the default `minecraft:` namespace.
        if !item_id.contains(':') {
            let ns = format!("minecraft:{item_id}");
            if self.ids.contains(&ns) {
                return Some(ns);
            }
        }
        None
    }
}

impl ItemRegistry for FullItemRegistry {
    fn contains(&self, item_id: &str) -> bool {
        self.canonical(item_id).is_some()
    }

    fn max_stack_size(&self, item_id: &str) -> Option<u32> {
        self.canonical(item_id)
            .and_then(|id| self.stack_sizes.get(&id).copied())
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
    /// The local y of this piece's **top authored water block** — its waterline —
    /// for open-air island pieces built to the `prefabs/island-tileset.md`
    /// convention (waterline local y=2, walk plane local y=3). Consumed by the
    /// ocean-horizon placement invariant (`DW0344`, `plan::check_ocean_waterline`):
    /// in a `horizon: ocean` world the declared waterline must land at world sea
    /// level. Absent for pieces that author no sea (interiors, keep/cave tilesets),
    /// which are then not checked.
    #[serde(default)]
    pub waterline_y: Option<i32>,
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
    /// The pre-wired dispenser socket cell (local coords) for an `anchor/trap`
    /// marker (DSL v0.6, spec-0011). `pos` is the trap's trigger/hazard cell (the
    /// plate/tripwire/chest the compiler models as a hazard); `dispenser` is the
    /// separate cell holding the empty dispenser whose payload the compiler fills.
    /// Absent for every non-trap anchor (byte-identical for existing metadata).
    #[serde(default)]
    pub dispenser: Option<[i32; 3]>,
    /// The block the prefab wired as this `anchor/trap`'s **trigger** — the plate
    /// or tripwire sitting on `pos` (DSL v0.6). Declared with its full blockstate
    /// exactly as authored (`minecraft:oak_pressure_plate[powered=false]`), because
    /// flag-gating a trap physically removes and restores this block and must put
    /// back what was there. The gate-anchor `block` above is the same contract for
    /// `close-gate`. Absent for every non-trap anchor, and only *required* by a
    /// trap that declares a flag gate (`DW0363`).
    #[serde(default)]
    pub trigger_block: Option<String>,
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
    /// Per-file load failures (`DW0346`) collected by [`Self::load_dir`] —
    /// surfaced by the CLI at validation tier, never silently dropped.
    load_diagnostics: Vec<Diagnostic>,
}

impl PrefabRegistry {
    /// Load every `*.json` prefab metadata file in `dir`. An optional
    /// `pools.json` (`{ "pools": { "pool/<name>": { "members": [...] } } }`)
    /// declares prefab pools and their member pieces. Returns the loaded
    /// registry; errors only on an unreadable directory.
    ///
    /// A file that fails to read or parse is **not** silently skipped: it
    /// yields a `DW0346` in [`Self::load_diagnostics`] naming the file and the
    /// serde error, and loading continues for the other files (report-all, not
    /// fail-fast). Before this, an older `delvec` meeting newer metadata (an
    /// unknown field under `deny_unknown_fields`) dropped the prefab on the
    /// floor and only failed much later as a baffling `DW0300` "prefab not
    /// found".
    pub fn load_dir(dir: &Path) -> std::io::Result<Self> {
        let mut by_id: BTreeMap<String, PrefabMeta> = BTreeMap::new();
        let mut anchor_names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut pools: BTreeSet<String> = BTreeSet::new();
        let mut pool_members: BTreeMap<String, Vec<PoolMember>> = BTreeMap::new();
        let mut load_diagnostics: Vec<Diagnostic> = Vec::new();
        // Sort entries for deterministic load order (and diagnostic order).
        let mut paths: Vec<PathBuf> = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                paths.push(path);
            }
        }
        paths.sort();
        for path in paths {
            // The file name only (never an absolute path) — stable across
            // machines, and the prefabs dir is a single flat directory.
            let file = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<non-utf8 filename>")
                .to_string();
            let mut fail = |what: String| {
                load_diagnostics.push(Diagnostic::error(
                    DW_PREFAB_META_INVALID,
                    "prefabs",
                    file.clone(),
                    format!(
                        "prefab metadata `{file}` {what} — the library may use a newer metadata \
                         schema than this delvec understands: upgrade delvec, or fix the field. \
                         (The file is skipped; every other prefab still loads.)"
                    ),
                ));
            };
            let raw = match std::fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(e) => {
                    fail(format!("cannot be read: {e}"));
                    continue;
                }
            };
            if file == "pools.json" {
                match serde_json::from_str::<PoolsFile>(&raw) {
                    Ok(pf) => {
                        for (id, def) in pf.pools {
                            pools.insert(id.clone());
                            pool_members.insert(id, def.members);
                        }
                    }
                    Err(e) => fail(format!("does not parse as a pools file: {e}")),
                }
                continue;
            }
            match serde_json::from_str::<PrefabMeta>(&raw) {
                Ok(meta) => {
                    let names: BTreeSet<String> = meta.anchors.keys().cloned().collect();
                    anchor_names.insert(meta.prefab_id.clone(), names);
                    by_id.insert(meta.prefab_id.clone(), meta);
                }
                Err(e) => fail(format!("does not parse as prefab metadata: {e}")),
            }
        }
        Ok(Self {
            by_id,
            anchor_names,
            pools,
            pool_members,
            load_diagnostics,
        })
    }

    /// Per-file load failures (`DW0346`) from [`Self::load_dir`]. The CLI folds
    /// these into the validation diagnostics (exit 1) on every
    /// `validate`/`analyze`/`build`.
    pub fn load_diagnostics(&self) -> &[Diagnostic] {
        &self.load_diagnostics
    }

    /// The metadata for a prefab id (`prefab/<name>`), if loaded.
    pub fn get(&self, prefab_id: &str) -> Option<&PrefabMeta> {
        self.by_id.get(prefab_id)
    }

    /// The member pieces of a prefab pool (`pool/<name>`), if declared.
    pub fn pool(&self, pool_id: &str) -> Option<&[PoolMember]> {
        self.pool_members.get(pool_id).map(|v| v.as_slice())
    }

    /// Classify how the loaded prefabs provide a gate anchor for the `close-gate`
    /// block-declared check (`DW0343`). `close-gate` fills the region with the block
    /// the anchor declares, so a blockless (or non-region) anchor cannot be sealed:
    ///
    /// - `None` — no loaded prefab declares `anchor_name` as a **gate region**
    ///   (it is a point anchor, a trap anchor, or unknown): nothing to seal.
    /// - `Some(false)` — at least one region-provider declares the anchor but omits
    ///   `block`: the compiler cannot know what to fill with (and the solver may
    ///   place that blockless member).
    /// - `Some(true)` — every prefab that declares this anchor as a gate region also
    ///   declares a fill `block`.
    ///
    /// Gate anchors resolve globally (like `open-gate`), so the scan is over every
    /// prefab — the conservative "all region-providers must declare a block" rule
    /// guarantees whichever member the solver places can be sealed.
    pub fn gate_anchor_block(&self, anchor_name: &str) -> Option<bool> {
        let mut any_region = false;
        let mut all_have_block = true;
        for meta in self.by_id.values() {
            if let Some(am) = meta.anchors.get(anchor_name)
                && am.region.is_some()
            {
                any_region = true;
                if am.block.is_none() {
                    all_have_block = false;
                }
            }
        }
        any_region.then_some(all_have_block)
    }

    /// The trigger block declared by the `anchor/trap` marker `anchor_name`, if any
    /// prefab declares one. `None` when no prefab providing the anchor declares a
    /// `trigger_block` — which is what makes a flag gate on that trap `DW0363`.
    /// First match in prefab-id order (deterministic; a `BTreeMap` walk).
    pub fn trap_trigger_block(&self, anchor_name: &str) -> Option<&str> {
        self.by_id.values().find_map(|meta| {
            meta.anchors
                .get(anchor_name)
                .and_then(|am| am.trigger_block.as_deref())
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The vendored stack-size table must cover EXACTLY the vendored item
    /// registry. Both come from the same 1.21.11 summary, so a regeneration that
    /// updates one and not the other would silently turn `DW0436` off for the
    /// items it forgot — a check that stops checking without failing.
    #[test]
    fn the_stack_size_table_covers_exactly_the_item_registry() {
        let r = FullItemRegistry::v1_21_11();
        let sized: BTreeSet<&String> = r.stack_sizes.keys().collect();
        let known: BTreeSet<&String> = r.ids.iter().collect();
        assert_eq!(sized, known, "item registry and stack-size table disagree");
        assert_eq!(r.ids.len(), 1505);
    }

    /// Stack sizes are Mojang's, per item — not a 1-vs-64 folk rule. 1.21.11 uses
    /// exactly three caps.
    #[test]
    fn stack_sizes_are_the_pinned_vanilla_caps() {
        let r = FullItemRegistry::v1_21_11();
        assert_eq!(r.max_stack_size("minecraft:rabbit_stew"), Some(1));
        assert_eq!(r.max_stack_size("minecraft:ender_pearl"), Some(16));
        assert_eq!(r.max_stack_size("minecraft:cooked_cod"), Some(64));
        // Un-namespaced ids resolve under `minecraft:`, like `contains`.
        assert_eq!(r.max_stack_size("rabbit_stew"), Some(1));
        // An unknown item has no cap to report (its own diagnostic is `DW0143`).
        assert_eq!(r.max_stack_size("minecraft:not_an_item"), None);
        let caps: BTreeSet<u32> = r.stack_sizes.values().copied().collect();
        assert_eq!(caps, BTreeSet::from([1, 16, 64]));
    }
}
