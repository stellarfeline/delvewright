//! **A body's vanilla variant** (DSL v0.83): the coat, colour, collar, kind or
//! trade a summoned body shows, declared on the body.
//!
//! A `/summon` that carries any NBT skips the entity's `finalizeSpawn`, which
//! is where vanilla picks a random coat; so every body the engine summons is
//! the type's default (a black cat with a red collar, a plains villager, a
//! temperate pig) unless its summon names the variant
//! (`docs/reference/body-variants.md`). This module is the one place that
//! knows which properties a type carries, which values each takes, and how a
//! summon writes them — read from the pinned jar into
//! `crates/dsl/data/entity-variants-1.21.11.json` by
//! `tools/maintenance/extract-entity-variants.py`.
//!
//! One field, [`BodyVariant`], on every object class that declares a body the
//! engine summons — a stage-2 NPC, a stage-5 actor, a wave mob, a wave mob's
//! mount and a vehicle — because the variant belongs to the body and not to the
//! verb that first needed it. One enumeration ([`body_variant_sites`]), one
//! rule set ([`variant_checks`]: `DW1131`, `DW1132`) and one emission rule
//! ([`variant_fields`] / [`render_fields`]) serve all five.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::{BodyRef, namespaced_entity};

/// **A body's vanilla variant** (DSL v0.83): property → value, e.g.
/// `{"variant": "minecraft:tabby", "collar": "blue"}` on a cat,
/// `{"variant": "minecraft:taiga", "profession": "minecraft:librarian"}` on a
/// villager, `{"variant": "black", "markings": "white_dots"}` on a horse.
///
/// The properties are the pinned game's own: every entity data component named
/// `<entity>/<property>` (`minecraft:cat/variant` → `variant`), plus a horse's
/// `markings`, a villager's `profession` and a panda's `gene`. A value is
/// written as the game writes that component: a registry id
/// (`minecraft:tabby`) or a name (`blue`, `white_dots`). A property the body's
/// entity does not carry is `DW1131`; a value it does not take is `DW1132`.
/// A property left out keeps the type's default; `{}` declares nothing and
/// changes nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct BodyVariant(pub BTreeMap<String, String>);

impl BodyVariant {
    /// Whether nothing is declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// How one NBT slot of a property is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VariantForm {
    /// The value's name, quoted.
    String,
    /// The value's numeric id as a byte (`11b`).
    Byte,
    /// The value's numeric id, shifted, OR-ed into an int several properties
    /// may share.
    Int,
}

/// One NBT slot a property is written to.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantSlot {
    /// The key: `Outer.inner` names a field of a compound.
    pub key: String,
    /// How the value is written there.
    pub form: VariantForm,
    /// The left shift of a numeric id within a packed key.
    pub shift: u32,
}

/// One variant property of one entity type.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantProperty {
    /// The value domain (a key of [`VariantTable::domains`]).
    pub domain: String,
    /// The data component the property is, when the game names one.
    #[serde(default)]
    pub component: Option<String>,
    /// Where a summon writes it.
    pub nbt: Vec<VariantSlot>,
}

/// One value a property takes.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantValue {
    /// How it is written in the DSL and in a string slot.
    pub name: String,
    /// Its numeric id, for a numeric slot.
    #[serde(default)]
    pub id: Option<i64>,
}

/// The pinned variant table.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantTable {
    /// Value domains by name.
    pub domains: BTreeMap<String, Vec<VariantValue>>,
    /// Entity id → property → how it is carried.
    pub entities: BTreeMap<String, BTreeMap<String, VariantProperty>>,
}

/// The pinned table (`crates/dsl/data/entity-variants-1.21.11.json`).
pub fn variant_table() -> &'static VariantTable {
    static TABLE: std::sync::LazyLock<VariantTable> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("../data/entity-variants-1.21.11.json"))
            .expect("vendored entity-variants table is valid JSON")
    });
    &TABLE
}

/// The variant properties a body of `entity` carries (bare or namespaced id);
/// empty for a type that carries none.
pub fn variant_properties(entity: &str) -> BTreeMap<&'static str, &'static VariantProperty> {
    variant_table()
        .entities
        .get(&namespaced_entity(entity))
        .map(|m| m.iter().map(|(k, v)| (k.as_str(), v)).collect())
        .unwrap_or_default()
}

/// **The one emission rule**: what a summon of `entity` carries for `v`, as
/// key path → SNBT value (`variant` → `"minecraft:tabby"`, `CollarColor` →
/// `11b`, `VillagerData.type` → `"minecraft:taiga"`). Numeric parts sharing a
/// key are OR-ed; a part left undeclared contributes nothing, which is the
/// type's default (id 0). A property or value the table does not hold — refused
/// by [`variant_checks`] before anything is emitted — writes nothing.
pub fn variant_fields(entity: &str, v: &BodyVariant) -> BTreeMap<String, String> {
    let t = variant_table();
    let props = variant_properties(entity);
    let mut numeric: BTreeMap<String, (VariantForm, i64)> = BTreeMap::new();
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for (prop, value) in &v.0 {
        let Some(p) = props.get(prop.as_str()) else {
            continue;
        };
        let Some(val) = t
            .domains
            .get(&p.domain)
            .and_then(|d| d.iter().find(|x| &x.name == value))
        else {
            continue;
        };
        for slot in &p.nbt {
            match slot.form {
                VariantForm::String => {
                    out.insert(slot.key.clone(), format!("\"{}\"", val.name));
                }
                VariantForm::Byte | VariantForm::Int => {
                    let id = val.id.unwrap_or(0) << slot.shift;
                    let e = numeric.entry(slot.key.clone()).or_insert((slot.form, 0));
                    e.1 |= id;
                }
            }
        }
    }
    for (key, (form, n)) in numeric {
        let s = match form {
            VariantForm::Byte => format!("{n}b"),
            _ => n.to_string(),
        };
        out.insert(key, s);
    }
    out
}

/// Render key-path → SNBT fields as compound entries (no braces, no leading
/// comma), in key order; `Outer.inner` paths are grouped into one
/// `Outer:{inner:…,…}` compound.
pub fn render_fields(fields: &BTreeMap<String, String>) -> String {
    let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut flat: BTreeMap<&str, String> = BTreeMap::new();
    for (k, v) in fields {
        match k.split_once('.') {
            Some((outer, inner)) => groups
                .entry(outer)
                .or_default()
                .push(format!("{inner}:{v}")),
            None => {
                flat.insert(k.as_str(), format!("{k}:{v}"));
            }
        }
    }
    for (outer, parts) in groups {
        flat.insert(outer, format!("{outer}:{{{}}}", parts.join(",")));
    }
    flat.into_values().collect::<Vec<_>>().join(",")
}

/// [`render_fields`] of [`variant_fields`], with a leading comma when anything
/// is written — the fragment a summon compound splices after its own fields.
/// Empty for no declaration, so a body that declares none is byte-identical.
pub fn variant_nbt(entity: &str, v: Option<&BodyVariant>) -> String {
    let Some(v) = v else {
        return String::new();
    };
    let s = render_fields(&variant_fields(entity, v));
    if s.is_empty() { s } else { format!(",{s}") }
}

/// One body that declares a variant.
#[derive(Clone, Debug)]
pub struct BodyVariantSite<'a> {
    /// The declaring stage (`npcs` / `quests`).
    pub stage: &'static str,
    /// JSON pointer of the `variant` object.
    pub path: String,
    /// `npc` / `actor` / `wave-mob` / `mount` / `vehicle`, for a diagnostic.
    pub what: &'static str,
    /// The declaring object's id (a wave's for a wave mob or its mount).
    pub id: &'a str,
    /// The entity the body ships as — a skinned body is a mannequin.
    pub entity: &'a str,
    /// The declaration.
    pub variant: &'a BodyVariant,
}

/// **Every body that declares a variant**, in stage order: NPCs, actors, wave
/// mobs, their mounts, vehicles. The one enumeration every reader walks.
pub fn body_variant_sites(c: &Campaign) -> Vec<BodyVariantSite<'_>> {
    let mut out = Vec::new();
    for (i, n) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(v) = &n.variant {
            out.push(BodyVariantSite {
                stage: "npcs",
                path: format!("/content/npcs/{i}/variant"),
                what: "npc",
                id: n.id.as_str(),
                entity: BodyRef::Npc(n).worn_entity(),
                variant: v,
            });
        }
    }
    let q = &c.quests.content;
    for (i, a) in q.actors.iter().enumerate() {
        if let Some(v) = &a.variant {
            out.push(BodyVariantSite {
                stage: "quests",
                path: format!("/content/actors/{i}/variant"),
                what: "actor",
                id: a.id.as_str(),
                entity: BodyRef::Actor(a).worn_entity(),
                variant: v,
            });
        }
    }
    for (wi, w) in q.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            if let Some(v) = &m.variant {
                out.push(BodyVariantSite {
                    stage: "quests",
                    path: format!("/content/waves/{wi}/mobs/{k}/variant"),
                    what: "wave-mob",
                    id: w.id.as_str(),
                    entity: m.entity.as_str(),
                    variant: v,
                });
            }
        }
    }
    for site in crate::mount::mount_sites(c) {
        if let Some(v) = &site.mount.variant {
            out.push(BodyVariantSite {
                stage: "quests",
                path: format!("{}/variant", site.path),
                what: "mount",
                id: site.wave.id.as_str(),
                entity: site.mount.entity.as_str(),
                variant: v,
            });
        }
    }
    for (i, v) in q.vehicles.iter().enumerate() {
        if let Some(var) = &v.variant {
            out.push(BodyVariantSite {
                stage: "quests",
                path: format!("/content/vehicles/{i}/variant"),
                what: "vehicle",
                id: v.id.as_str(),
                entity: v.entity.as_str(),
                variant: var,
            });
        }
    }
    out
}

crate::dw_code! {
    /// (DSL v0.83) **A variant property the body does not carry**: the body's
    /// entity — the one it ships as, so a skinned body is a mannequin — has no
    /// such property in the pinned game (a `collar` on a horse, any property on
    /// a zombie). Validation-tier (exit 1). Prescription: one of the properties
    /// the message lists for that entity, or no `variant`.
    pub const VARIANT_PROPERTY_UNKNOWN: DwCode = DwCode::new("DW1131", ExitTier::Build);
}

crate::dw_code! {
    /// (DSL v0.83) **A variant value the property does not take**: the value
    /// is not in the pinned game's registry or list for that property of that
    /// entity (`minecraft:tabby` on a wolf, `teal` as a collar). Validation-tier
    /// (exit 1). Prescription: one of the values the message lists.
    pub const VARIANT_VALUE_UNKNOWN: DwCode = DwCode::new("DW1132", ExitTier::Build);
}

/// `DW1131` / `DW1132`: every declared property is one the body's entity
/// carries, and every value one that property takes.
pub fn variant_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let t = variant_table();
    for s in body_variant_sites(c) {
        let props = variant_properties(s.entity);
        for (prop, value) in &s.variant.0 {
            let at = format!("{}/{}", s.path, json_pointer_escape(prop));
            let Some(p) = props.get(prop.as_str()) else {
                let carried = if props.is_empty() {
                    format!("`{}` carries no variant property", s.entity)
                } else {
                    format!(
                        "`{}` carries {}",
                        s.entity,
                        props
                            .keys()
                            .map(|k| format!("`{k}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                d.push(Diagnostic::error(
                    VARIANT_PROPERTY_UNKNOWN,
                    s.stage,
                    at,
                    format!(
                        "{} `{}` declares variant property `{prop}`, which its body does not carry: {carried}",
                        s.what, s.id
                    ),
                ));
                continue;
            };
            let domain = t.domains.get(&p.domain).map(Vec::as_slice).unwrap_or(&[]);
            if !domain.iter().any(|x| &x.name == value) {
                d.push(Diagnostic::error(
                    VARIANT_VALUE_UNKNOWN,
                    s.stage,
                    at,
                    format!(
                        "{} `{}` declares `{prop}` `{value}`, which a `{}` does not take; it takes {}",
                        s.what,
                        s.id,
                        s.entity,
                        domain.iter().map(|x| format!("`{}`", x.name)).collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
        }
    }
}

fn json_pointer_escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// What [`variant_checks`] examined, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VariantBinding {
    /// Bodies declaring a variant.
    pub bodies: usize,
    /// Properties declared across them.
    pub properties: usize,
    /// Distinct entity types among them.
    pub entity_types: usize,
}

impl VariantBinding {
    /// Count what [`variant_checks`] examines on `c`.
    pub fn of(c: &Campaign) -> Self {
        let sites = body_variant_sites(c);
        let types: std::collections::BTreeSet<&str> = sites.iter().map(|s| s.entity).collect();
        VariantBinding {
            bodies: sites.len(),
            properties: sites.iter().map(|s| s.variant.0.len()).sum(),
            entity_types: types.len(),
        }
    }
}
