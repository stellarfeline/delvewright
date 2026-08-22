//! The full pinned-MC item registry and prefab/anchor metadata, injected into
//! DSL validation via `validate_campaign_with` (spec-0002 / ADR-0004).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvewright_dsl::DwCode;
use delvewright_dsl::{
    AnchorRegistry, Diagnostic, EntityRegistry, ItemRegistry, Lighting, PoolId, PrefabId,
};
use serde::Deserialize;

/// `DW0346`: a prefab metadata `*.json` (or `pools.json`) in the prefabs dir
/// failed to read or parse. Silently skipping it made a bad file surface only as
/// a baffling downstream `DW0300` "prefab not found"; the parse failure itself is
/// the information. Reported at **validation tier (exit 1)**; loading continues
/// for the other files (report-all, not fail-fast).
///
/// A key this delvec does not model is deliberately **not** one of these: it is
/// kept and reported as [`DW_PREFAB_META_UNKNOWN_KEY`].
pub const DW_PREFAB_META_INVALID: DwCode = DwCode::every_version("DW0346");

/// `DW0543`: a prefab metadata file carries a key this delvec does not model.
///
/// A **warning**, and the severity is the decision. Refusing the document was
/// the previous behaviour and it was wrong in exactly one direction: a consumer
/// that is not a document's owner meets new keys as a matter of course — the
/// content library and the engine version independently — and every forward
/// addition became a hard failure at the layer with the least context. Ignoring
/// the key is wrong in the other direction, because the same observation is also
/// what a misspelled key looks like. So the piece loads, the key survives a
/// rewrite ([`delvewright_dsl::prefab`]), and the reader says what it saw.
pub const DW_PREFAB_META_UNKNOWN_KEY: DwCode = DwCode::every_version("DW0543");

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

/// The vendored `entity_type` **tag** table: tag id → its member entity ids,
/// from Mojang's own generated reports (`data/entity-tags-1.21.11.json`,
/// regenerated per MC pin by `tools/extract-entity-tags.py`; `data/PROVENANCE.md`).
///
/// Vanilla's answer to every question of the form "which entity types do X" that
/// vanilla itself answers. Lives here, beside the other vendored tables, because
/// more than one proof needs it — `DW0496` asks which bodies burn in daylight,
/// `DW0452`/`DW0453` ask which bodies are aquatic — and a second copy is how a
/// hand table gets started.
pub fn entity_tags() -> &'static BTreeMap<String, BTreeSet<String>> {
    static TAGS: std::sync::LazyLock<BTreeMap<String, BTreeSet<String>>> =
        std::sync::LazyLock::new(|| {
            let raw = include_str!("../data/entity-tags-1.21.11.json");
            let parsed: BTreeMap<String, Vec<String>> =
                serde_json::from_str(raw).expect("vendored entity-type tags are valid JSON");
            parsed
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().collect()))
                .collect()
        });
    &TAGS
}

/// Normalize a DSL entity id to its namespaced form (`zombie` →
/// `minecraft:zombie`), which is how the vendored tags spell every member.
pub fn namespaced_entity(entity: &str) -> String {
    if entity.contains(':') {
        entity.to_string()
    } else {
        format!("minecraft:{entity}")
    }
}

/// Whether `entity` is a member of the vanilla `entity_type` tag `tag`.
///
/// Membership only — this deliberately does **not** expand a nested `#tag`
/// reference (a tag's member list may name another tag). Every caller so far
/// reads a flat tag, and silently under-expanding would be the dangerous
/// direction for a proof that grants exemptions, so the narrowing is stated
/// rather than assumed: a nested reference reads as "not a member", which puts
/// the entity in the *checked* class.
pub fn entity_in_tag(entity: &str, tag: &str) -> bool {
    let id = namespaced_entity(entity);
    entity_tags()
        .get(tag)
        .is_some_and(|members| members.contains(&id))
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

/// Per-item combat & sustain numbers for the pinned MC version — Mojang's own
/// `minecraft:attribute_modifiers` / `minecraft:food` default components,
/// vendored under `data/` (see `data/PROVENANCE.md`; regenerate with
/// `tools/extract-item-combat-stats.py`). Read by the spec-0023 winnability
/// arithmetic (`compiler::combat`).
///
/// **Absence is a fact, not a gap.** An item with no entry has no combat
/// *attribute* in Mojang's data, which is emphatically not "deals no damage": a
/// bow's damage is projectile code and appears in no vanilla data at all. Callers
/// must treat `None` as *unknown*, never as zero — that distinction is the whole
/// reason `DW0472` (a bound that failed) and `DW0475` (a bound that could not be
/// computed) are two different diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct ItemCombat {
    /// The item's `minecraft:attack_damage` modifier — what it ADDS to the
    /// wielder's base attack damage, not the total.
    pub attack_damage: f64,
    /// The item's `minecraft:attack_speed` modifier (negative on every weapon:
    /// it subtracts from the player's base 4.0 attacks/second).
    pub attack_speed: f64,
    /// Armour points contributed when worn.
    pub armor: f64,
    /// Armour toughness contributed when worn.
    pub armor_toughness: f64,
    /// `minecraft:food`'s `nutrition`; 0 for anything that is not food.
    pub nutrition: f64,
}

/// The vendored `item id -> ItemCombat` table.
#[derive(Debug, Clone)]
pub struct ItemCombatRegistry {
    stats: BTreeMap<String, ItemCombat>,
}

impl ItemCombatRegistry {
    /// Load the vendored 1.21.11 combat/sustain table (embedded at compile time).
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/item-combat-1.21.11.json");
        let stats: BTreeMap<String, ItemCombat> =
            serde_json::from_str(raw).expect("vendored item combat table is valid JSON");
        Self { stats }
    }

    /// Mojang's numbers for `item_id`, or `None` when the item contributes none.
    /// An un-namespaced id resolves under the default `minecraft:` namespace.
    pub fn get(&self, item_id: &str) -> Option<ItemCombat> {
        if let Some(s) = self.stats.get(item_id) {
            return Some(*s);
        }
        if !item_id.contains(':') {
            return self.stats.get(&format!("minecraft:{item_id}")).copied();
        }
        None
    }
}

/// How a damage type behaves, straight from Mojang's data: whether armour
/// applies at all (`#minecraft:bypasses_armor` membership) and its `scaling`
/// field, which decides whether the difficulty multipliers touch it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DamageTypeFacts {
    /// True when armour points do not reduce this type at all.
    pub bypasses_armor: bool,
    /// `never` | `when_caused_by_living_non_player` | `always`, verbatim.
    pub scaling: String,
}

impl DamageTypeFacts {
    /// Does a bare `/damage <target> <amount> <type>` — which has **no
    /// attacker** — get scaled by the world difficulty?
    ///
    /// Only `always` does. This is the trap the table exists to close: eight of
    /// the nine types the DSL exposes are `when_caused_by_living_non_player`, so
    /// a scripted `damage-players` is NOT halved on Easy, whatever the
    /// `WorldDifficulty` doc-comment formula reads like in isolation.
    pub fn scales_without_attacker(&self) -> bool {
        self.scaling == "always"
    }
}

/// The vendored `damage type -> DamageTypeFacts` table.
#[derive(Debug, Clone)]
pub struct DamageTypeRegistry {
    facts: BTreeMap<String, DamageTypeFacts>,
}

impl DamageTypeRegistry {
    /// Load the vendored 1.21.11 damage-type table (embedded at compile time).
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/damage-types-1.21.11.json");
        let facts: BTreeMap<String, DamageTypeFacts> =
            serde_json::from_str(raw).expect("vendored damage type table is valid JSON");
        Self { facts }
    }

    /// Mojang's facts for `type_id`, or `None` if the pinned registry has no
    /// such type.
    pub fn get(&self, type_id: &str) -> Option<&DamageTypeFacts> {
        self.facts.get(type_id)
    }
}

// ---------------------------------------------------------------------------
// Prefab metadata (`prefabs/<name>.json`)
// ---------------------------------------------------------------------------

/// The prefab metadata document, defined once in [`delvewright_dsl::prefab`] and
/// re-exported here under the names this crate has always called it by.
///
/// **This crate does not define the shape and must not.** It used to: a private
/// `PrefabMeta` with `deny_unknown_fields`, which meant that the first
/// grammar-exported prefab carrying a key this engine predated would have failed
/// every campaign build — not one piece, and not a degraded render. The
/// duplication was the defect; adding the missing field names to the copy each
/// time is what made it look like a fixed one.
///
/// The compiler consumes a narrow part of the document (anchors, sockets, the
/// lighting profile, the declared waterline, the face contract). A narrow VIEW
/// is not a reason to re-declare the fields — it is a reason for the accessors
/// below to read only what they read.
pub use delvewright_dsl::prefab::{
    Anchor as AnchorMeta, AnchorRole, Connector, GateAnchor, PrefabMeta, Region,
    SpatialContract as SpatialContractMeta, StructureMeta,
};
/// A face of the piece's face contract, and its opening. The opening is an
/// ordinary [`Region`]; assembly reads it as one.
pub use delvewright_dsl::prefab::{ContractFace as ContractFaceMeta, Region as FaceOpening};

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
                        "prefab metadata `{file}` {what} — the document is malformed for this \
                         delvec: a required block is absent, or a value is of the wrong type. \
                         (A key this delvec has never heard of is NOT this: unknown keys are \
                         kept and reported as `DW0543`.) The file is skipped; every other \
                         prefab still loads."
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
            if file == delvewright_dsl::prefab::POOLS_FILE {
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
            // Both packagings — one template, or a tile set past the vanilla
            // 48-per-axis cap — are the same document and are read by the one
            // reader that defines it (`PrefabMeta::from_json`, which also
            // refuses a manifest that does not tile its own zone). A private
            // shape test here was what made a tiled zone unbuildable: the
            // registry knew what the document was and answered `DW0346`, so a
            // campaign could produce a zone and not build a world out of it.
            match PrefabMeta::from_json(&raw) {
                Ok(meta) => {
                    // A key this delvec does not model is kept, not refused —
                    // and not silent either. It is one of exactly two things: a
                    // library newer than this engine, or a typo, and the reader
                    // cannot tell them apart. Saying so is the whole of what a
                    // consumer is entitled to do about it; refusing was the
                    // defect this diagnostic replaces.
                    let unknown = meta.unknown_keys();
                    if !unknown.is_empty() {
                        let named = unknown
                            .iter()
                            .map(|(owner, key)| {
                                if owner.is_empty() {
                                    format!("`{key}`")
                                } else {
                                    format!("`{key}` (on anchor `{owner}`)")
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        load_diagnostics.push(Diagnostic::warning(
                            DW_PREFAB_META_UNKNOWN_KEY,
                            "prefabs",
                            file.clone(),
                            format!(
                                "prefab metadata `{file}` carries {} key(s) this delvec does not \
                                 model: {named}. The piece loads and the key is preserved on any \
                                 rewrite, so this is not a failure — but it is one of two things \
                                 and this delvec cannot tell which: the library is NEWER than \
                                 this engine (upgrade delvec to consume the key), or the key is \
                                 a misspelling of one this document does define, in which case \
                                 whatever it was meant to say is not being said.",
                                unknown.len()
                            ),
                        ));
                    }
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

    /// **What one piece says about a gate anchor**, asked of the ONE authority
    /// ([`PrefabMeta::gate_anchor`]) so that this crate never reads
    /// `region`/`block` off an anchor itself.
    ///
    /// `Ok(None)` covers both "this piece does not declare the name" and "it
    /// declares it, and it is not a gate" — neither is something content can
    /// fill, and no caller has ever needed to tell them apart.
    pub fn gate_anchor_in(
        &self,
        prefab_id: &str,
        anchor_name: &str,
    ) -> Result<Option<GateAnchor>, String> {
        match self.by_id.get(prefab_id) {
            Some(meta) => meta.gate_anchor(anchor_name),
            None => Ok(None),
        }
    }

    /// The prefab ids `area` can place: its bare `prefab`, or every member of
    /// its `prefab_pool`.
    ///
    /// **The denominator every campaign-scoped question about pieces owes.** The
    /// gate-anchor check used to scan `by_id` — the whole library, every `*.json`
    /// in the prefabs dir — and so answered about pieces this campaign cannot
    /// place. Two pieces belonging to no area of the campaign declared an anchor
    /// of the name a shortcut addressed, and `DW0343` passed on their word: a
    /// gate check that answered `yes` about a different building. The rule the
    /// old doc comment stated — *whichever member the solver places can be
    /// sealed* — was right, and the code asked it of a larger world than the
    /// solver draws from.
    pub fn area_pieces(&self, area: &delvewright_dsl::Area) -> Vec<String> {
        if let Some(prefab) = &area.prefab {
            return vec![prefab.as_str().to_string()];
        }
        match area
            .prefab_pool
            .as_ref()
            .and_then(|p| self.pool(p.as_str()))
        {
            Some(members) => members.iter().map(|m| m.prefab.clone()).collect(),
            None => Vec::new(),
        }
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

    /// This registry IS the library — every `*.json` in the prefabs dir the run
    /// was pointed at — so it answers, and `Some(false)` means the piece is not
    /// there (`DW0856`). A file that failed to parse is `DW0346` and is absent
    /// from `by_id`, so it answers `Some(false)` too; that is the intended
    /// order, since the campaign's binding really cannot be honoured and both
    /// diagnostics are reported together.
    fn has_prefab(&self, prefab: &PrefabId) -> Option<bool> {
        Some(self.by_id.contains_key(prefab.as_str()))
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
