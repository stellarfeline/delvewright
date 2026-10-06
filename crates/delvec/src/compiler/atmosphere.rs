//! A place has its own sky (spec-0080): a campaign declares atmospheres, a place
//! carries one from the first tick, and a beat repaints a volume with another.
//!
//! An atmosphere is a **datapack biome** the delve ships
//! (`datapack/data/<ns>/worldgen/biome/atmosphere/<kebab>.json`), built from the
//! engine's void definition (`horizon`) with the creator's environment
//! attributes, tint and precipitation. Biomes load only when a world opens, so
//! every declared atmosphere is emitted whether a place carries it or only a
//! beat paints it. Which biome stands where is
//! [`crate::compiler::horizon::biome_map`]'s answer; this module owns the
//! surface — what a campaign may write (`DW0928`, `DW0929`'s document arm,
//! `DW0930`), how it becomes a biome file, and the one writer of `fillbiome`.
//!
//! # What a biome can set in the overworld
//!
//! Read from the pinned jar's data (`data/minecraft/timeline/*.json`): attributes
//! stack dimension < biome < timeline < weather, and the overworld's `day`
//! timeline multiplies the colours and `sky_light_factor` (a biome's value
//! survives, darkened at night), takes the maximum of `star_brightness` (a biome
//! can force stars at noon), and **overrides** `sun_angle`, `moon_angle`,
//! `star_angle`, `sunrise_sunset_color` and `moon_phase`. Those five are refused,
//! because a biome that writes them is overruled every tick. The `gameplay/` ids
//! are out of scope: `monsters_burn` stacks by `or` under the day timeline, so a
//! biome can force burning and never stop it, and `sky_light_level` would owe a
//! per-cell reading to three proofs before it could be admitted honestly.
//!
//! # The registry is data
//!
//! `crates/delvec/data/environment-attributes-1.21.11.json` is read out of the
//! pinned jar's bytecode by `tools/maintenance/extract-environment-attributes.py`
//! (`data/PROVENANCE.md`): the ids, their scope, the value shape and the range
//! the codec rejects outside of. Where the codec bounds nothing, the engine
//! bounds nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use delvewright_dsl::{
    Atmosphere, Campaign, Diagnostic, DwCode, ExitTier, Precipitation, QuestEffect, Verb,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// `DW0928`: an attribute line the pinned game does not accept here — an id it
/// does not register, one the overworld day cycle overrides, a `gameplay/` id, a
/// value not in the id's shape or outside its codec's range, a sound or particle
/// the pinned registries lack; or a tint colour that is not `#rrggbb`.
pub const DW_ATMOSPHERE_ATTRIBUTE: DwCode = DwCode::new("DW0928", ExitTier::Build);
/// `DW0929`: a paint that reaches cells it may not — a `set-atmosphere` naming
/// neither or both of `region` / `place`, a repaint volume outside the map's
/// extent, or two carried places whose painted cells meet with different
/// atmospheres.
pub const DW_ATMOSPHERE_PAINT: DwCode = DwCode::new("DW0929", ExitTier::Build);
/// `DW0930`: an atmosphere declared against itself or against nothing — one no
/// place carries and no beat paints, a `climate` that contradicts its
/// `precipitation`, or a duplicate id.
pub const DW_ATMOSPHERE_DECL: DwCode = DwCode::new("DW0930", ExitTier::Build);

/// The path segment every atmosphere biome lives under in the delve's
/// namespace: `<ns>:atmosphere/<kebab>`, emitted at
/// `data/<ns>/worldgen/biome/atmosphere/<kebab>.json`.
pub const BIOME_DIR: &str = "atmosphere";

/// Vanilla's snow line: a biome whose temperature is below 0.15 snows where a
/// warmer one rains (`horizon::void_biome_definition`'s own note).
pub const SNOW_LINE: f64 = 0.15;

/// The default `max_block_modifications` gamerule, which caps one `fillbiome`'s
/// volume (`FillBiomeCommand.fill`: the product of the spans of the box its two
/// corners quantize to, compared with the gamerule).
pub const FILLBIOME_CAP: i64 = 32768;

// ---------------------------------------------------------------------------
// The vendored registry
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RegistryDoc {
    attributes: Vec<Row>,
    records: BTreeMap<String, BTreeMap<String, Field>>,
}

/// One environment attribute of the pinned game.
#[derive(Deserialize, Clone, Debug)]
pub struct Row {
    /// `visual/sky_color`.
    pub id: String,
    /// `admitted`, `overridden` or `gameplay`.
    pub scope: String,
    /// The value shape a campaign writes; admitted ids only.
    pub shape: Option<String>,
    /// `[min, max]` the codec validates, `max` `None` for unbounded above.
    pub range: Option<(f64, Option<f64>)>,
    /// The modifier the overworld's timelines key it with, when they do.
    pub overworld_timeline: Option<String>,
}

#[derive(Deserialize, Clone, Copy, Debug)]
struct Field {
    required: bool,
    range: Option<(f64, Option<f64>)>,
}

/// The pinned registries an atmosphere is held to.
pub struct Registry {
    rows: BTreeMap<String, Row>,
    records: BTreeMap<String, BTreeMap<String, Field>>,
    sounds: crate::compiler::registry::FullSoundRegistry,
}

impl Registry {
    /// The vendored 1.21.11 tables, parsed once.
    pub fn v1_21_11() -> &'static Registry {
        static REG: OnceLock<Registry> = OnceLock::new();
        REG.get_or_init(|| {
            let doc: RegistryDoc = serde_json::from_str(include_str!(
                "../../data/environment-attributes-1.21.11.json"
            ))
            .expect("the vendored environment-attribute registry parses");
            Registry {
                rows: doc
                    .attributes
                    .into_iter()
                    .map(|r| (r.id.clone(), r))
                    .collect(),
                records: doc.records,
                sounds: crate::compiler::registry::FullSoundRegistry::v1_21_11(),
            }
        })
    }

    /// Every attribute row, in id order.
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.values()
    }

    /// The row for an id written with or without its `minecraft:` prefix.
    pub fn row(&self, key: &str) -> Option<&Row> {
        self.rows.get(canonical_id(key))
    }

    fn field(&self, record: &str, name: &str) -> Field {
        *self
            .records
            .get(record)
            .and_then(|r| r.get(name))
            .unwrap_or_else(|| panic!("the vendored registry reads record `{record}.{name}`"))
    }

    fn record_names(&self, record: &str) -> BTreeSet<&str> {
        self.records
            .get(record)
            .map(|r| r.keys().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// The registered ids nearest `key` by edit distance, for the refusal.
    fn nearest(&self, key: &str) -> Vec<&str> {
        let key = canonical_id(key);
        let mut ranked: Vec<(usize, &str)> = self
            .rows
            .keys()
            .map(|id| (edit_distance(key, id), id.as_str()))
            .collect();
        ranked.sort();
        let best = ranked.first().map(|r| r.0).unwrap_or(0);
        ranked
            .into_iter()
            .take_while(|r| r.0 == best)
            .take(3)
            .map(|r| r.1)
            .collect()
    }
}

/// An attribute id without its optional `minecraft:` prefix.
pub fn canonical_id(key: &str) -> &str {
    key.strip_prefix("minecraft:").unwrap_or(key)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

// ---------------------------------------------------------------------------
// Value shapes: each validates and returns the canonical value it emits
// ---------------------------------------------------------------------------

fn namespaced(id: &str) -> String {
    if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    }
}

fn colour(v: &Value, digits: usize) -> Result<Value, String> {
    let shape = if digits == 6 { "#rrggbb" } else { "#aarrggbb" };
    match v.as_str() {
        Some(s)
            if s.len() == digits + 1
                && s.starts_with('#')
                && s[1..].chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            Ok(Value::String(s.to_ascii_lowercase()))
        }
        _ => Err(format!("a colour written `{shape}`, got `{v}`")),
    }
}

fn in_range(x: f64, range: Option<(f64, Option<f64>)>, what: &str) -> Result<(), String> {
    if let Some((lo, hi)) = range
        && (x < lo || hi.is_some_and(|hi| x > hi))
    {
        let hi = hi.map_or("unbounded".to_string(), |h| h.to_string());
        return Err(format!(
            "{what} {x} is outside the range the pinned codec accepts, [{lo}, {hi}]"
        ));
    }
    Ok(())
}

fn float(v: &Value, range: Option<(f64, Option<f64>)>, what: &str) -> Result<Value, String> {
    let x = v
        .as_f64()
        .ok_or_else(|| format!("{what} is a number, got `{v}`"))?;
    in_range(x, range, what)?;
    Ok(json!(x))
}

fn int(v: &Value, range: Option<(f64, Option<f64>)>, what: &str) -> Result<Value, String> {
    let x = v
        .as_i64()
        .ok_or_else(|| format!("{what} is a whole number, got `{v}`"))?;
    in_range(x as f64, range, what)?;
    Ok(json!(x))
}

/// The float modifiers whose argument is a plain float
/// (`FloatModifier$Simple.argumentCodec` is `Codec.FLOAT`), plus `override`,
/// whose argument is the attribute's own codec and so carries its range.
/// `alpha_blend` takes a `FloatWithAlpha` and is not admitted.
const FLOAT_MODIFIERS: [&str; 6] = [
    "add", "subtract", "multiply", "minimum", "maximum", "override",
];

fn float_or_modifier(v: &Value, row: &Row) -> Result<Value, String> {
    let Some(obj) = v.as_object() else {
        return float(v, row.range, "the value");
    };
    let keys: BTreeSet<&str> = obj.keys().map(String::as_str).collect();
    if keys != BTreeSet::from(["argument", "modifier"]) {
        return Err(format!(
            "a number, or the modifier form `{{\"argument\": <number>, \"modifier\": <one of {}>}}`, \
             got `{v}`",
            FLOAT_MODIFIERS.join(", ")
        ));
    }
    let modifier = obj["modifier"].as_str().unwrap_or_default();
    if !FLOAT_MODIFIERS.contains(&modifier) {
        return Err(format!(
            "modifier `{}` is not one of {}",
            obj["modifier"],
            FLOAT_MODIFIERS.join(", ")
        ));
    }
    let range = (modifier == "override").then_some(row.range).flatten();
    Ok(json!({
        "argument": float(&obj["argument"], range, "the modifier's argument")?,
        "modifier": modifier,
    }))
}

impl Registry {
    fn sound(&self, v: &Value, what: &str) -> Result<Value, String> {
        let s = v
            .as_str()
            .ok_or_else(|| format!("{what} is a sound-event id, got `{v}`"))?;
        if !self.sounds.contains(s) {
            return Err(format!(
                "{what} `{s}` is not a pinned 1.21.11 sound event (the registry `DW0326` reads)"
            ));
        }
        Ok(Value::String(namespaced(s)))
    }

    fn particle(&self, v: &Value) -> Result<Value, String> {
        let obj = v
            .as_object()
            .filter(|o| o.len() == 1 && o.contains_key("type"))
            .ok_or_else(|| format!("a particle written `{{\"type\": <id>}}`, got `{v}`"))?;
        let id = obj["type"]
            .as_str()
            .map(namespaced)
            .ok_or_else(|| format!("a particle type id, got `{}`", obj["type"]))?;
        // The one particle table (`crates/dsl/data/particles-1.21.11.json`),
        // read by the `particle` verb's `DW0941` and by this arm of `DW0928`.
        match delvewright_dsl::perception::particle_takes_options(&id) {
            None => Err(format!(
                "particle type `{id}` is not in the pinned `particle_type` registry"
            )),
            Some(true) => Err(format!(
                "particle type `{id}` takes options, and an atmosphere admits only a simple \
                 particle written `{{\"type\": <id>}}`"
            )),
            Some(false) => Ok(json!({ "type": id })),
        }
    }

    /// A record value: the declared fields, each through its own shape, every
    /// required field present and nothing else.
    fn record(
        &self,
        v: &Value,
        record: &str,
        each: &dyn Fn(&str, &Value, Field) -> Result<Value, String>,
    ) -> Result<Value, String> {
        let obj = v
            .as_object()
            .ok_or_else(|| format!("an object (`{record}`), got `{v}`"))?;
        let names = self.record_names(record);
        if let Some(extra) = obj.keys().find(|k| !names.contains(k.as_str())) {
            return Err(format!(
                "`{extra}` is not a field of `{record}` (its fields: {})",
                names.iter().copied().collect::<Vec<_>>().join(", ")
            ));
        }
        let mut out = Map::new();
        for name in names {
            let field = self.field(record, name);
            match obj.get(name) {
                Some(x) => {
                    out.insert(name.to_string(), each(name, x, field)?);
                }
                None if field.required => {
                    return Err(format!("`{record}` requires `{name}`"));
                }
                None => {}
            }
        }
        Ok(Value::Object(out))
    }

    fn music(&self, v: &Value) -> Result<Value, String> {
        self.record(v, "music", &|name, x, f| match name {
            "sound" => self.sound(x, "`music.sound`"),
            "replace_current_music" => x
                .as_bool()
                .map(Value::Bool)
                .ok_or_else(|| format!("`replace_current_music` is true or false, got `{x}`")),
            _ => int(x, f.range, &format!("`music.{name}`")),
        })
    }

    fn additions(&self, v: &Value) -> Result<Value, String> {
        self.record(v, "ambient_additions", &|name, x, f| match name {
            "sound" => self.sound(x, "`additions.sound`"),
            _ => float(x, f.range, &format!("`additions.{name}`")),
        })
    }

    /// Validate one attribute value and return the canonical form emitted.
    pub fn value(&self, row: &Row, v: &Value) -> Result<Value, String> {
        match row.shape.as_deref() {
            Some("rgb") => colour(v, 6),
            Some("argb") => colour(v, 8),
            Some("float") => float_or_modifier(v, row),
            Some("boolean") => v
                .as_bool()
                .map(Value::Bool)
                .ok_or_else(|| format!("true or false, got `{v}`")),
            Some("particle") => self.particle(v),
            Some("ambient_particles") => {
                let list = v
                    .as_array()
                    .ok_or_else(|| format!("a list of `{{particle, probability}}`, got `{v}`"))?;
                let out: Result<Vec<Value>, String> = list
                    .iter()
                    .map(|e| {
                        self.record(e, "ambient_particle", &|name, x, f| match name {
                            "particle" => self.particle(x),
                            _ => float(x, f.range, &format!("`{name}`")),
                        })
                    })
                    .collect();
                Ok(Value::Array(out?))
            }
            Some("background_music") => {
                self.record(v, "background_music", &|_, x, _| self.music(x))
            }
            Some("ambient_sounds") => self.record(v, "ambient_sounds", &|name, x, _| match name {
                "loop" => self.sound(x, "`loop`"),
                "mood" => self.record(x, "ambient_mood", &|n, y, f| match n {
                    "sound" => self.sound(y, "`mood.sound`"),
                    "offset" => float(y, f.range, "`mood.offset`"),
                    _ => int(y, f.range, &format!("`mood.{n}`")),
                }),
                // `AmbientSounds.additions` is a list; vanilla data writes one
                // entry as a bare object, and both spellings are read.
                _ => match x {
                    Value::Array(list) => Ok(Value::Array(
                        list.iter()
                            .map(|e| self.additions(e))
                            .collect::<Result<_, _>>()?,
                    )),
                    _ => self.additions(x),
                },
            }),
            other => Err(format!("no shape is recorded for `{}` ({other:?})", row.id)),
        }
    }
}

// ---------------------------------------------------------------------------
// The document checks: DW0928, DW0929 (exclusivity), DW0930
// ---------------------------------------------------------------------------

/// What the overworld day cycle does to an overridden attribute, for the
/// refusal.
fn overridden_reason(id: &str) -> &'static str {
    match id {
        "visual/sun_angle" | "visual/moon_angle" | "visual/star_angle" => {
            "the overworld's `minecraft:day` timeline sets the sun, moon and star angles every \
             tick, so the party would see the ordinary sky's course"
        }
        "visual/sunrise_sunset_color" => {
            "the overworld's `minecraft:day` timeline sets the sunrise colour every tick, so the \
             party would see the ordinary sunrise"
        }
        _ => {
            "an overworld timeline sets it every tick (`minecraft:moon` keys the phase), so the \
             party would see the ordinary value"
        }
    }
}

/// Validate every atmosphere a campaign declares and every `set-atmosphere`
/// it writes, at the document (`delvec validate`).
pub fn check(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let reg = Registry::v1_21_11();
    let declared = &c.world.content.atmospheres;
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, a) in declared.iter().enumerate() {
        let base = format!("/content/atmospheres/{i}");
        if !seen.insert(a.id.as_str()) {
            d.push(Diagnostic::error(
                DW_ATMOSPHERE_DECL,
                "world",
                format!("{base}/id"),
                format!(
                    "atmosphere `{}` is declared twice. An atmosphere ships as one biome, \
                     `<ns>:{}`, so a second declaration under the same id is a second answer \
                     to what that biome is. Merge the two, or give one its own id.",
                    a.id, a.id
                ),
            ));
        }
        attribute_checks(reg, a, &base, &mut d);
        climate_check(a, &base, &mut d);
    }
    paint_exclusivity(c, &mut d);
    unbound(c, &mut d);
    d
}

fn attribute_checks(reg: &Registry, a: &Atmosphere, base: &str, d: &mut Vec<Diagnostic>) {
    let mut keys: BTreeSet<&str> = BTreeSet::new();
    for (key, value) in &a.attributes {
        let path = format!(
            "{base}/attributes/{}",
            key.replace('~', "~0").replace('/', "~1")
        );
        let refuse = |d: &mut Vec<Diagnostic>, msg: String| {
            d.push(Diagnostic::error(
                DW_ATMOSPHERE_ATTRIBUTE,
                "world",
                path.clone(),
                format!("atmosphere `{}` attribute `{key}`: {msg}", a.id),
            ));
        };
        if !keys.insert(canonical_id(key)) {
            refuse(
                d,
                "written twice, once with and once without its `minecraft:` prefix — keep one"
                    .to_string(),
            );
            continue;
        }
        let Some(row) = reg.row(key) else {
            refuse(
                d,
                format!(
                    "the pinned game registers no such environment attribute. Nearest: {}. \
                     The registry is `crates/delvec/data/environment-attributes-1.21.11.json`.",
                    reg.nearest(key)
                        .iter()
                        .map(|n| format!("`{n}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            continue;
        };
        match row.scope.as_str() {
            "overridden" => refuse(
                d,
                format!(
                    "a biome cannot set this in the overworld: {}. Remove the line. Moving the \
                     sky's course is world-wide in vanilla — a custom dimension or an edited \
                     timeline — and no place can do it.",
                    overridden_reason(&row.id)
                ),
            ),
            "gameplay" => refuse(
                d,
                "gameplay attributes are out of scope for an atmosphere (spec-0080 §2.4): \
                 `monsters_burn` stacks by `or` under the day timeline so a biome can never \
                 stop daylight burning, `sky_light_level` is read by the light, engagement and \
                 burn proofs, and the rest act on things a delve has none of. Remove the line."
                    .to_string(),
            ),
            _ => {
                if let Err(e) = reg.value(row, value) {
                    refuse(
                        d,
                        format!(
                            "{e}. The pinned codec would refuse the biome, and a biome that \
                             fails to load stops the world from opening."
                        ),
                    );
                }
            }
        }
    }
    if let Some(t) = &a.tint {
        for (field, v) in [
            ("grass", &t.grass),
            ("foliage", &t.foliage),
            ("dry_foliage", &t.dry_foliage),
            ("water", &t.water),
        ] {
            if let Some(v) = v
                && let Err(e) = colour(&Value::String(v.clone()), 6)
            {
                d.push(Diagnostic::error(
                    DW_ATMOSPHERE_ATTRIBUTE,
                    "world",
                    format!("{base}/tint/{field}"),
                    format!("atmosphere `{}` tint `{field}`: {e}", a.id),
                ));
            }
        }
    }
}

fn climate_check(a: &Atmosphere, base: &str, d: &mut Vec<Diagnostic>) {
    let Some(climate) = a.climate else {
        return;
    };
    let wrong = match a.precipitation {
        Precipitation::Snow => (climate.temperature >= SNOW_LINE).then_some(
            "declares `precipitation: snow`, and vanilla snows only below temperature 0.15",
        ),
        Precipitation::Rain => (climate.temperature < SNOW_LINE).then_some(
            "declares `precipitation: rain`, and vanilla turns rain to snow below temperature 0.15",
        ),
        Precipitation::None => None,
    };
    if let Some(why) = wrong {
        d.push(Diagnostic::error(
            DW_ATMOSPHERE_DECL,
            "world",
            format!("{base}/climate/temperature"),
            format!(
                "atmosphere `{}` {why}, but its `climate.temperature` is {}. The biome would let \
                 the other thing fall, and the proofs that read `precipitation` would be \
                 reasoning about weather the party never stands in. Change the temperature, \
                 or the precipitation, or drop `climate` and let the compiler derive it.",
                a.id, climate.temperature
            ),
        ));
    }
}

/// Every `set-atmosphere` in the campaign, at every root and every depth, with
/// its stage and JSON path.
pub fn set_atmospheres(c: &Campaign) -> Vec<(String, String, &QuestEffect)> {
    let mut out = Vec::new();
    delvewright_dsl::for_each_campaign_effect(c, &mut |path, site, e| {
        if matches!(e.verb, Verb::SetAtmosphere { .. }) {
            let stage = match site {
                delvewright_dsl::EffectSite::DialogueRespawn { .. } => "dialogue",
                _ => "quests",
            };
            out.push((stage.to_string(), path.to_string(), e));
        }
    });
    out
}

fn paint_exclusivity(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (stage, path, e) in set_atmospheres(c) {
        let Verb::SetAtmosphere { region, place, .. } = &e.verb else {
            continue;
        };
        let msg = match (region, place) {
            (Some(_), Some(_)) => "names both a `region` and a `place`",
            (None, None) => "names neither a `region` nor a `place`",
            _ => continue,
        };
        d.push(Diagnostic::error(
            DW_ATMOSPHERE_PAINT,
            stage,
            path,
            format!(
                "this `set-atmosphere` {msg}. A repaint covers exactly one volume: a `region` \
                 (an anchor-centred box) for a volume inside a place, or a `place` (an \
                 `area/…` or a site-plan box's `node/…`) for the whole of one — whose bounds \
                 the compiler reads from the placement and nobody types. Keep exactly one."
            ),
        ));
    }
}

/// The atmosphere ids something stands in or paints: carried by an area or a
/// site-plan box, or named by a `set-atmosphere`.
pub fn used_ids(c: &Campaign) -> BTreeSet<String> {
    let mut used: BTreeSet<String> = BTreeSet::new();
    for a in &c.world.content.areas {
        if let Some(id) = &a.atmosphere {
            used.insert(id.as_str().to_string());
        }
    }
    if let Some(sp) = &c.site_plan {
        for b in &sp.content.boxes {
            if let Some(id) = &b.atmosphere {
                used.insert(id.as_str().to_string());
            }
        }
    }
    for (_, _, e) in set_atmospheres(c) {
        if let Verb::SetAtmosphere {
            atmosphere: Some(id),
            ..
        } = &e.verb
        {
            used.insert(id.as_str().to_string());
        }
    }
    used
}

fn unbound(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let used = used_ids(c);
    for (i, a) in c.world.content.atmospheres.iter().enumerate() {
        if !used.contains(a.id.as_str()) {
            d.push(Diagnostic::error(
                DW_ATMOSPHERE_DECL,
                "world",
                format!("/content/atmospheres/{i}/id"),
                format!(
                    "atmosphere `{}` is declared and nothing stands in it: no area or site-plan \
                     box carries it (`atmosphere`) and no `set-atmosphere` paints it. It would \
                     ship as a biome no cell is ever painted with. Carry it on the place it \
                     belongs to, paint it from the beat that turns the air, or delete it.",
                    a.id
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

/// The biome id an atmosphere ships as in namespace `ns`.
pub fn biome_id(ns: &str, id: &str) -> String {
    format!("{ns}:{id}")
}

/// The datapack path an atmosphere's biome is written to.
pub fn biome_path(ns: &str, id: &str) -> String {
    format!("datapack/data/{ns}/worldgen/biome/{id}.json")
}

/// The three vanilla fields `precipitation` derives (§3.1.4), or the declared
/// `climate`'s pair in their place.
pub fn climate_of(a: &Atmosphere) -> (bool, f64, f64) {
    let (temperature, downfall) = match a.precipitation {
        Precipitation::Snow => (0.0, 0.5),
        Precipitation::Rain | Precipitation::None => (0.5, 0.5),
    };
    let (temperature, downfall) = a
        .climate
        .map_or((temperature, downfall), |c| (c.temperature, c.downfall));
    (
        a.precipitation != Precipitation::None,
        temperature,
        downfall,
    )
}

/// Whether declared rain or snow falls in an atmosphere's biome.
pub fn precipitates(a: &Atmosphere) -> bool {
    a.precipitation != Precipitation::None
}

/// An atmosphere's biome definition: the engine's void biome with
/// `features: []` (a painted biome never generates), the derived climate, the
/// tint as `effects`, and every attribute under its `minecraft:` id in its
/// canonical form. An attribute that fails its shape is left out: `DW0928` has
/// refused the document before any build reaches here.
pub fn biome_definition(a: &Atmosphere) -> Value {
    let reg = Registry::v1_21_11();
    let mut def = crate::compiler::horizon::void_biome_definition();
    let (has_precipitation, temperature, downfall) = climate_of(a);
    let mut attributes = Map::new();
    for (key, v) in &a.attributes {
        if let Some(row) = reg.row(key)
            && row.scope == "admitted"
            && let Ok(canon) = reg.value(row, v)
        {
            attributes.insert(format!("minecraft:{}", row.id), canon);
        }
    }
    let mut effects = Map::new();
    effects.insert("water_color".to_string(), json!("#3f76e4"));
    if let Some(t) = &a.tint {
        for (field, v) in [
            ("grass_color", &t.grass),
            ("foliage_color", &t.foliage),
            ("dry_foliage_color", &t.dry_foliage),
            ("water_color", &t.water),
        ] {
            if let Some(v) = v {
                effects.insert(field.to_string(), json!(v.to_ascii_lowercase()));
            }
        }
    }
    let obj = def
        .as_object_mut()
        .expect("the void definition is an object");
    obj.insert("attributes".to_string(), Value::Object(attributes));
    obj.insert("effects".to_string(), Value::Object(effects));
    obj.insert("features".to_string(), json!([]));
    obj.insert("has_precipitation".to_string(), json!(has_precipitation));
    obj.insert("temperature".to_string(), json!(temperature));
    obj.insert("downfall".to_string(), json!(downfall));
    def
}

/// **The one writer of `fillbiome`** under `crates/delvec/src/`: one command
/// over one block box. Every caller — the surround's bands, the bootstrap
/// paint of a carried place, a `set-atmosphere` — comes through here or through
/// [`fillbiome_lines`].
pub fn fillbiome_line(min: [i32; 3], max: [i32; 3], biome: &str) -> String {
    format!(
        "fillbiome {} {} {} {} {} {} {biome}",
        min[0], min[1], min[2], max[0], max[1], max[2]
    )
}

/// Floor to the 4-block biome cell (`QuartPos.toBlock(QuartPos.fromBlock(x))`).
pub fn quantize(x: i32) -> i32 {
    x.div_euclid(4) * 4
}

/// The volume `fillbiome` measures a box by: the spans of the box its corners
/// quantize to (`FillBiomeCommand.fill`).
fn command_volume(min: [i32; 3], max: [i32; 3]) -> i64 {
    (0..3)
        .map(|i| i64::from(quantize(max[i]) - quantize(min[i]) + 1))
        .product()
}

/// The cells a `fillbiome` over `min..=max` paints: every 4-cell the range
/// touches, so the enclosing 4-aligned box.
pub fn painted_box(min: [i32; 3], max: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    (
        [quantize(min[0]), quantize(min[1]), quantize(min[2])],
        [
            quantize(max[0]) + 3,
            quantize(max[1]) + 3,
            quantize(max[2]) + 3,
        ],
    )
}

/// `fillbiome` lines painting `min..=max` with `biome`, split at 4-cell
/// boundaries so that no command exceeds the default `max_block_modifications`
/// ([`FILLBIOME_CAP`]) — a repaint fires mid-play, where raising a gamerule
/// around it would be a second world-wide write. Deterministic: the longest
/// axis is halved at a 4-cell boundary until every piece fits.
pub fn fillbiome_lines(min: [i32; 3], max: [i32; 3], biome: &str) -> Vec<String> {
    let mut out = Vec::new();
    split(min, max, &mut |a, b| out.push(fillbiome_line(a, b, biome)));
    out
}

fn split(min: [i32; 3], max: [i32; 3], f: &mut dyn FnMut([i32; 3], [i32; 3])) {
    if command_volume(min, max) <= FILLBIOME_CAP {
        f(min, max);
        return;
    }
    let axis = (0..3)
        .max_by_key(|&i| (quantize(max[i]) - quantize(min[i]), std::cmp::Reverse(i)))
        .expect("three axes");
    let cells = (quantize(max[axis]) - quantize(min[axis])) / 4 + 1;
    let cut = quantize(min[axis]) + (cells / 2) * 4;
    let (mut lo_max, mut hi_min) = (max, min);
    lo_max[axis] = cut - 1;
    hi_min[axis] = cut;
    split(min, lo_max, f);
    split(hi_min, max, f);
}

/// What the atmosphere surface bound to, printed with every build (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Atmospheres declared.
    pub declared: usize,
    /// Places carrying one.
    pub carried: usize,
    /// Places in the campaign (areas, or site-plan boxes).
    pub places: usize,
    /// `set-atmosphere` effects.
    pub repaints: usize,
    /// Distinct repaint volumes.
    pub volumes: usize,
    /// 4-cells painted at bootstrap.
    pub bootstrap_cells: i64,
    /// Paints in the map (bands and places).
    pub paints: usize,
    /// Biome files emitted for atmospheres.
    pub biome_files: usize,
}

impl Binding {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "atmosphere binding: {} declared; {} of {} place(s) carry one; {} repaint effect(s) \
             over {} volume(s); {} quart cells painted at bootstrap of {} paint(s) in the map; \
             {} biome file(s) emitted",
            self.declared,
            self.carried,
            self.places,
            self.repaints,
            self.volumes,
            self.bootstrap_cells,
            self.paints,
            self.biome_files
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_counts_are_the_pinned_games() {
        let reg = Registry::v1_21_11();
        let count = |s: &str| reg.rows().filter(|r| r.scope == s).count();
        assert_eq!(reg.rows().count(), 45);
        assert_eq!(count("admitted"), 20);
        assert_eq!(count("overridden"), 5);
        assert_eq!(count("gameplay"), 20);
        for r in reg.rows().filter(|r| r.scope == "admitted") {
            assert!(r.shape.is_some(), "{} has a shape", r.id);
        }
    }

    #[test]
    fn a_split_paint_stays_inside_the_cap_and_covers_the_box() {
        let lines = fillbiome_lines([-37, 0, 5], [300, 120, 260], "x:y");
        assert!(lines.len() > 1);
        let mut cells = 0i64;
        for l in &lines {
            let n: Vec<i32> = l
                .split(' ')
                .skip(1)
                .take(6)
                .map(|t| t.parse().unwrap())
                .collect();
            let (a, b) = ([n[0], n[1], n[2]], [n[3], n[4], n[5]]);
            assert!(command_volume(a, b) <= FILLBIOME_CAP, "{l}");
            let (pa, pb) = painted_box(a, b);
            cells += (0..3)
                .map(|i| i64::from((pb[i] - pa[i] + 1) / 4))
                .product::<i64>();
        }
        let (pa, pb) = painted_box([-37, 0, 5], [300, 120, 260]);
        let whole: i64 = (0..3).map(|i| i64::from((pb[i] - pa[i] + 1) / 4)).product();
        assert_eq!(cells, whole, "the pieces tile the painted box exactly");
    }

    #[test]
    fn the_nearest_id_is_offered() {
        let reg = Registry::v1_21_11();
        assert_eq!(reg.nearest("visual/sky_colour"), vec!["visual/sky_color"]);
    }
}
