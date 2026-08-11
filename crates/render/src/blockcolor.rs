//! Blockstate → appearance, **derived from the pinned client jar**, never typed
//! out by hand.
//!
//! For one `minecraft:foo[a=b]` palette entry this resolves, exactly as the game
//! does, `assets/…/blockstates/foo.json` → the matching variant (or every
//! satisfied `multipart` case) → the model's `parent` chain → that model's
//! `elements` and `textures` → the referenced `.png`s. From those it computes:
//!
//! - **colour** — the alpha-weighted mean of every texture the model's faces
//!   name. Alpha weighting is what stops a mostly-transparent texture (glass,
//!   a ladder, a sapling) reporting the colour of its empty pixels.
//! - **coverage** — mean alpha, which is how a translucent block is told from a
//!   solid one without a list of which blocks are glass.
//! - **shape** — the union of the model elements' `from`/`to`, with the variant's
//!   `x`/`y` rotation applied, in sixteenths of a block. This is what makes a
//!   slab a slab, a carpet a carpet and a chain a thin vertical post instead of
//!   everything being a full cube.
//!
//! **Biome tint is derived too.** Grass, foliage and water are greyscale or
//! flat in the atlas and get their colour at runtime, so a naive mean renders a
//! green island grey. The tint comes from `data/…/worldgen/biome/<id>.json` in
//! the same jar — its `temperature`/`downfall` index the `colormap/grass.png`
//! and `colormap/foliage.png` tables exactly as vanilla does, and its
//! `effects.water_color` colours water. The biome is a parameter
//! ([`Deriver::with_biome`]), so a creator whose fiction is a swamp, a nether
//! waste or a cherry grove gets their own tints rather than this project's.
//!
//! Nothing here needs a GPU, and the only version-specific knowledge is the jar
//! itself: point it at 1.22's jar and the table follows.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::assets::Assets;

/// The default biome whose tints colour a prefab when none is named. Plains is
/// vanilla's own reference point for the colormaps (temperature 0.8, downfall
/// 0.4) and is what a structure block preview shows.
pub const DEFAULT_BIOME: &str = "minecraft:plains";

/// How a block looks, reduced to what a voxel viewer can draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    /// Alpha-weighted mean texture colour, after biome tint.
    pub rgb: [u8; 3],
    /// Mean alpha over the sampled textures, 0–255. Below [`OPAQUE_COVERAGE`]
    /// the block is drawn see-through.
    pub coverage: u8,
    /// Union of the model elements, in sixteenths of a block:
    /// `[x0, y0, z0, x1, y1, z1]`, each 0–16.
    #[serde(rename = "box")]
    pub shape: [u8; 6],
}

/// Coverage at or above which a full-cube block occludes its neighbours' faces.
pub const OPAQUE_COVERAGE: u8 = 250;

impl Appearance {
    /// True when this block fills its cell and hides what is behind it — the
    /// test the mesher culls interior faces by.
    pub fn is_opaque_cube(&self) -> bool {
        self.coverage >= OPAQUE_COVERAGE && self.shape == [0, 0, 0, 16, 16, 16]
    }
}

/// Why a blockstate could not be given an appearance. Each is reported with its
/// count rather than silently dropped: a block the viewer cannot draw is a
/// finding about the prefab or the pinned version, not a cosmetic detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unresolved {
    /// No `blockstates/<id>.json` — the block does not exist in this version.
    NoBlockstate,
    /// The blockstate resolved but named no model that carries geometry.
    NoModel,
    /// The model named textures that are not in this source.
    NoTexture,
}

impl std::fmt::Display for Unresolved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Unresolved::NoBlockstate => "no blockstate definition in this version",
            Unresolved::NoModel => "no model with geometry",
            Unresolved::NoTexture => "no texture found for the model",
        })
    }
}

/// Air-like states, which carry no appearance and are not findings.
pub fn is_air(state: &str) -> bool {
    matches!(
        base_id(state).1.as_str(),
        "air" | "cave_air" | "void_air" | "structure_void" | "light"
    )
}

/// Split `ns:id[props]` into `(namespace, id)`, defaulting the namespace to
/// `minecraft`.
pub fn base_id(state: &str) -> (String, String) {
    let head = state.split('[').next().unwrap_or(state);
    match head.split_once(':') {
        Some((ns, id)) => (ns.to_string(), id.to_string()),
        None => ("minecraft".to_string(), head.to_string()),
    }
}

/// Parse the `[a=b,c=d]` tail of a blockstate string.
fn properties(state: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(start) = state.find('[') else {
        return out;
    };
    let Some(end) = state.rfind(']') else {
        return out;
    };
    for kv in state[start + 1..end].split(',') {
        if let Some((k, v)) = kv.split_once('=') {
            out.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

/// A model reference plus the variant rotation applied to it.
#[derive(Debug, Clone)]
struct ModelRef {
    ns: String,
    path: String,
    x_deg: i32,
    y_deg: i32,
}

/// Biome-derived tints.
#[derive(Debug, Clone, Copy)]
struct Tints {
    grass: [u8; 3],
    foliage: [u8; 3],
    water: [u8; 3],
}

/// Derives [`Appearance`]s from an [`Assets`] source.
pub struct Deriver<'a> {
    assets: &'a Assets,
    biome: String,
    tints: Tints,
}

impl<'a> Deriver<'a> {
    /// Derive with [`DEFAULT_BIOME`]'s tints.
    pub fn new(assets: &'a Assets) -> Self {
        Self::with_biome(assets, DEFAULT_BIOME)
    }

    /// Derive with the tints of a named biome (`ns:id`, or a bare id in
    /// `minecraft`). A biome absent from the source falls back to a neutral
    /// (untinted) table rather than failing: the tint is a refinement, and a
    /// prefab with no plants still deserves to render.
    pub fn with_biome(assets: &'a Assets, biome: &str) -> Self {
        let tints = read_tints(assets, biome);
        Deriver {
            assets,
            biome: biome.to_string(),
            tints,
        }
    }

    /// The biome whose tints this deriver applies.
    pub fn biome(&self) -> &str {
        &self.biome
    }

    /// Resolve one palette blockstate string.
    pub fn appearance(&self, state: &str) -> Result<Appearance, Unresolved> {
        let (ns, id) = base_id(state);
        let props = properties(state);
        let def = self
            .assets
            .blockstate(&ns, &id)
            .ok_or(Unresolved::NoBlockstate)?;
        let models = select_models(&def, &props, &ns);
        if models.is_empty() {
            return Err(Unresolved::NoModel);
        }

        let mut lo = [16.0f32; 3];
        let mut hi = [0.0f32; 3];
        let mut any_element = false;
        // Texture references in resolution order, de-duplicated but ORDER-STABLE
        // so the mean is the same on every run.
        let mut textures: Vec<(String, bool)> = Vec::new();
        let mut seen = BTreeMap::new();

        for m in &models {
            let Some(resolved) = self.resolve_model(&m.ns, &m.path) else {
                continue;
            };
            for el in &resolved.elements {
                any_element = true;
                let (f, t) = rotate_box(el.from, el.to, m.x_deg, m.y_deg);
                for i in 0..3 {
                    lo[i] = lo[i].min(f[i]);
                    hi[i] = hi[i].max(t[i]);
                }
                for (tex_ref, tinted) in &el.faces {
                    if let Some(p) = resolve_texture_ref(tex_ref, &resolved.textures)
                        && seen.insert(p.clone(), ()).is_none()
                    {
                        textures.push((p, *tinted));
                    }
                }
            }
            // A model with no elements of its own (a fluid, or a block whose
            // geometry the client supplies) still names its textures; take them
            // so the block gets a colour instead of vanishing.
            if resolved.elements.is_empty() {
                for key in ["all", "texture", "particle", "side", "top", "still"] {
                    if let Some(p) = resolve_texture_ref(&format!("#{key}"), &resolved.textures)
                        && seen.insert(p.clone(), ()).is_none()
                    {
                        textures.push((p, false));
                    }
                }
            }
        }

        if !any_element {
            // No geometry anywhere in the chain: occupy the whole cell.
            lo = [0.0; 3];
            hi = [16.0; 3];
        }

        let mut sum = [0f64; 3];
        let mut cov = 0f64;
        let mut n = 0f64;
        let mut tinted_any = false;
        for (path, tinted) in &textures {
            let Some((r, g, b, a)) = self.mean_texture(path) else {
                continue;
            };
            sum[0] += r;
            sum[1] += g;
            sum[2] += b;
            cov += a;
            n += 1.0;
            tinted_any |= *tinted;
        }
        if n == 0.0 {
            return Err(Unresolved::NoTexture);
        }

        let mut rgb = [
            (sum[0] / n).round().clamp(0.0, 255.0) as u8,
            (sum[1] / n).round().clamp(0.0, 255.0) as u8,
            (sum[2] / n).round().clamp(0.0, 255.0) as u8,
        ];
        if let Some(tint) = self.tint_for(&id, tinted_any) {
            rgb = multiply(rgb, tint);
        }

        Ok(Appearance {
            rgb,
            coverage: (cov / n).round().clamp(0.0, 255.0) as u8,
            shape: [
                clamp16(lo[0]),
                clamp16(lo[1]),
                clamp16(lo[2]),
                clamp16(hi[0]),
                clamp16(hi[1]),
                clamp16(hi[2]),
            ],
        })
    }

    /// Which biome tint, if any, multiplies this block's mean texture colour.
    ///
    /// A model face declares only *that* it is tinted (`tintindex`), never with
    /// which table — the client decides that per block. Two suffix rules cover
    /// every tinted block in vanilla: leaves and vines read the foliage table,
    /// everything else tinted reads the grass table. Water carries no
    /// `tintindex` at all (it has no model elements), so it is keyed by id.
    fn tint_for(&self, id: &str, tinted: bool) -> Option<[u8; 3]> {
        if id == "water" || id == "flowing_water" || id == "water_cauldron" {
            return Some(self.tints.water);
        }
        if !tinted {
            return None;
        }
        if id.ends_with("_leaves") || id == "vine" {
            Some(self.tints.foliage)
        } else {
            Some(self.tints.grass)
        }
    }

    /// Alpha-weighted mean `(r, g, b, mean_alpha)` of one texture.
    fn mean_texture(&self, path: &str) -> Option<(f64, f64, f64, f64)> {
        let (ns, p) = match path.split_once(':') {
            Some((ns, p)) => (ns.to_string(), p.to_string()),
            None => ("minecraft".to_string(), path.to_string()),
        };
        let (w, h, px) = self.assets.texture_rgba(&ns, &p)?;
        let count = (w as usize) * (h as usize);
        if count == 0 {
            return None;
        }
        let (mut r, mut g, mut b, mut a) = (0f64, 0f64, 0f64, 0f64);
        for i in 0..count {
            let q = &px[i * 4..i * 4 + 4];
            let al = q[3] as f64;
            r += q[0] as f64 * al;
            g += q[1] as f64 * al;
            b += q[2] as f64 * al;
            a += al;
        }
        if a == 0.0 {
            return None;
        }
        Some((r / a, g / a, b / a, a / count as f64))
    }

    /// Walk a model's `parent` chain, merging texture variables (child wins) and
    /// taking the nearest ancestor that actually has elements.
    fn resolve_model(&self, ns: &str, path: &str) -> Option<ResolvedModel> {
        let mut textures: BTreeMap<String, String> = BTreeMap::new();
        let mut elements: Vec<Element> = Vec::new();
        let mut have_elements = false;
        let mut cursor = Some((ns.to_string(), path.to_string()));
        let mut chain: Vec<String> = Vec::new();

        while let Some((cns, cpath)) = cursor.take() {
            let key = format!("{cns}:{cpath}");
            // A malformed pack could point a model at its own ancestor; stop
            // rather than spin.
            if chain.contains(&key) || chain.len() > 32 {
                break;
            }
            chain.push(key);

            let Some(json) = self.assets.model(&cns, &cpath) else {
                break;
            };
            if let Some(map) = json.get("textures").and_then(|t| t.as_object()) {
                for (k, v) in map {
                    if let Some(s) = v.as_str() {
                        // The child was inserted first and must win.
                        textures.entry(k.clone()).or_insert_with(|| s.to_string());
                    }
                }
            }
            if !have_elements
                && let Some(list) = json.get("elements").and_then(|e| e.as_array())
                && !list.is_empty()
            {
                have_elements = true;
                for e in list {
                    if let Some(el) = parse_element(e) {
                        elements.push(el);
                    }
                }
            }
            if let Some(parent) = json.get("parent").and_then(|p| p.as_str()) {
                let (pns, ppath) = match parent.split_once(':') {
                    Some((a, b)) => (a.to_string(), b.to_string()),
                    None => ("minecraft".to_string(), parent.to_string()),
                };
                cursor = Some((pns, ppath));
            }
        }

        if chain.is_empty() {
            return None;
        }
        Some(ResolvedModel { elements, textures })
    }
}

struct ResolvedModel {
    elements: Vec<Element>,
    textures: BTreeMap<String, String>,
}

struct Element {
    from: [f32; 3],
    to: [f32; 3],
    /// `(texture reference, is tinted)` per face, in a stable order.
    faces: Vec<(String, bool)>,
}

fn parse_element(v: &serde_json::Value) -> Option<Element> {
    let arr3 = |k: &str| -> Option<[f32; 3]> {
        let a = v.get(k)?.as_array()?;
        if a.len() != 3 {
            return None;
        }
        Some([
            a[0].as_f64()? as f32,
            a[1].as_f64()? as f32,
            a[2].as_f64()? as f32,
        ])
    };
    let from = arr3("from")?;
    let to = arr3("to")?;
    let mut faces = Vec::new();
    if let Some(map) = v.get("faces").and_then(|f| f.as_object()) {
        // Face keys are iterated in the JSON object's own order, which serde_json
        // preserves; sort so the mean does not depend on authoring order.
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort();
        for k in keys {
            let f = &map[k];
            let Some(t) = f.get("texture").and_then(|t| t.as_str()) else {
                continue;
            };
            let tinted = f
                .get("tintindex")
                .and_then(|t| t.as_i64())
                .is_some_and(|t| t >= 0);
            faces.push((t.to_string(), tinted));
        }
    }
    Some(Element { from, to, faces })
}

/// Follow `#name` indirection to a concrete `ns:path` texture reference.
fn resolve_texture_ref(reference: &str, textures: &BTreeMap<String, String>) -> Option<String> {
    let mut cur = reference.to_string();
    for _ in 0..16 {
        if let Some(key) = cur.strip_prefix('#') {
            cur = textures.get(key)?.clone();
        } else {
            return Some(cur);
        }
    }
    None
}

/// Pick the model(s) a blockstate definition gives these properties.
///
/// A structure palette entry omits every property left at its default, and the
/// jar carries no registry of defaults, so an omitted property cannot
/// contradict a variant — it simply does not constrain it. Variants are
/// therefore scored by how many of their constraints the declared properties
/// satisfy, rejecting any that are contradicted, and ties break on the
/// lexicographically smallest key so the choice is deterministic.
fn select_models(
    def: &serde_json::Value,
    props: &BTreeMap<String, String>,
    ns: &str,
) -> Vec<ModelRef> {
    if let Some(variants) = def.get("variants").and_then(|v| v.as_object()) {
        let mut keys: Vec<&String> = variants.keys().collect();
        keys.sort();
        let mut best: Option<(usize, &String)> = None;
        for k in keys {
            let (sat, contradicted) = score_variant(k, props);
            if contradicted {
                continue;
            }
            if best.is_none() || sat > best.unwrap().0 {
                best = Some((sat, k));
            }
        }
        if let Some((_, key)) = best {
            return model_refs(&variants[key], ns);
        }
        return Vec::new();
    }

    if let Some(cases) = def.get("multipart").and_then(|m| m.as_array()) {
        let mut out = Vec::new();
        for case in cases {
            let applies = match case.get("when") {
                None => true,
                Some(w) => when_matches(w, props),
            };
            if applies {
                out.extend(model_refs(
                    case.get("apply").unwrap_or(&serde_json::Value::Null),
                    ns,
                ));
            }
        }
        if out.is_empty() {
            // Nothing was satisfiable from the declared properties (they were
            // all defaults). Union every case so the block still gets a colour.
            for case in cases {
                out.extend(model_refs(
                    case.get("apply").unwrap_or(&serde_json::Value::Null),
                    ns,
                ));
            }
        }
        return out;
    }

    Vec::new()
}

/// `(constraints satisfied, any contradicted)` for a variant key.
fn score_variant(key: &str, props: &BTreeMap<String, String>) -> (usize, bool) {
    if key.is_empty() {
        return (0, false);
    }
    let mut sat = 0;
    for kv in key.split(',') {
        let Some((k, v)) = kv.split_once('=') else {
            continue;
        };
        match props.get(k) {
            None => {}
            Some(have) if have == v => sat += 1,
            Some(_) => return (0, true),
        }
    }
    (sat, false)
}

/// Vanilla `multipart` condition matching, including `OR` / `AND` and `a|b`
/// alternatives. A property the state does not declare is left at its default,
/// which this cannot know, so the condition does not match.
fn when_matches(when: &serde_json::Value, props: &BTreeMap<String, String>) -> bool {
    if let Some(list) = when.get("OR").and_then(|v| v.as_array()) {
        return list.iter().any(|w| when_matches(w, props));
    }
    if let Some(list) = when.get("AND").and_then(|v| v.as_array()) {
        return list.iter().all(|w| when_matches(w, props));
    }
    let Some(map) = when.as_object() else {
        return false;
    };
    for (k, v) in map {
        let want = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let Some(have) = props.get(k) else {
            return false;
        };
        if !want.split('|').any(|alt| alt == have) {
            return false;
        }
    }
    true
}

/// A variant value is either one model, or a weighted list of them (vanilla
/// picks at random). The first by name is taken so the page is deterministic.
fn model_refs(v: &serde_json::Value, ns: &str) -> Vec<ModelRef> {
    let one = |o: &serde_json::Value| -> Option<ModelRef> {
        let m = o.get("model")?.as_str()?;
        let (mns, mpath) = match m.split_once(':') {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (ns.to_string(), m.to_string()),
        };
        Some(ModelRef {
            ns: mns,
            path: mpath,
            x_deg: o.get("x").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
            y_deg: o.get("y").and_then(|y| y.as_i64()).unwrap_or(0) as i32,
        })
    };
    match v {
        serde_json::Value::Array(list) => {
            let mut best: Option<ModelRef> = None;
            for o in list {
                if let Some(r) = one(o)
                    && best
                        .as_ref()
                        .is_none_or(|b| (&r.ns, &r.path) < (&b.ns, &b.path))
                {
                    best = Some(r);
                }
            }
            best.into_iter().collect()
        }
        other => one(other).into_iter().collect(),
    }
}

/// Apply a variant's `x` then `y` rotation to an element box, about the cell
/// centre, and return the rotated axis-aligned bounds.
fn rotate_box(from: [f32; 3], to: [f32; 3], x_deg: i32, y_deg: i32) -> ([f32; 3], [f32; 3]) {
    if x_deg % 360 == 0 && y_deg % 360 == 0 {
        return (from, to);
    }
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for cx in [from[0], to[0]] {
        for cy in [from[1], to[1]] {
            for cz in [from[2], to[2]] {
                let p = rotate_point([cx, cy, cz], x_deg, y_deg);
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i]);
                    hi[i] = hi[i].max(p[i]);
                }
            }
        }
    }
    (lo, hi)
}

fn rotate_point(p: [f32; 3], x_deg: i32, y_deg: i32) -> [f32; 3] {
    let (mut x, mut y, mut z) = (p[0] - 8.0, p[1] - 8.0, p[2] - 8.0);
    let (sx, cx) = quarter_turn(x_deg);
    let (ny, nz) = (y * cx - z * sx, y * sx + z * cx);
    y = ny;
    z = nz;
    let (sy, cy) = quarter_turn(y_deg);
    let (nx, nz2) = (x * cy + z * sy, -x * sy + z * cy);
    x = nx;
    z = nz2;
    [x + 8.0, y + 8.0, z + 8.0]
}

/// `(sin, cos)` for a rotation that vanilla restricts to quarter turns, computed
/// exactly so a rotated box lands on integer sixteenths.
fn quarter_turn(deg: i32) -> (f32, f32) {
    match deg.rem_euclid(360) {
        90 => (1.0, 0.0),
        180 => (0.0, -1.0),
        270 => (-1.0, 0.0),
        _ => (0.0, 1.0),
    }
}

fn clamp16(v: f32) -> u8 {
    v.round().clamp(0.0, 16.0) as u8
}

fn multiply(base: [u8; 3], tint: [u8; 3]) -> [u8; 3] {
    [
        ((base[0] as u32 * tint[0] as u32) / 255) as u8,
        ((base[1] as u32 * tint[1] as u32) / 255) as u8,
        ((base[2] as u32 * tint[2] as u32) / 255) as u8,
    ]
}

/// Read a biome's grass/foliage/water tints out of the same source.
fn read_tints(assets: &Assets, biome: &str) -> Tints {
    let neutral = Tints {
        grass: [255, 255, 255],
        foliage: [255, 255, 255],
        water: [255, 255, 255],
    };
    let (ns, id) = match biome.split_once(':') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => ("minecraft".to_string(), biome.to_string()),
    };
    let Some(json) = assets.biome(&ns, &id) else {
        return neutral;
    };
    let temperature = json
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.8) as f32;
    let downfall = json.get("downfall").and_then(|v| v.as_f64()).unwrap_or(0.4) as f32;
    let effects = json.get("effects");

    let sample = |file: &str| -> [u8; 3] {
        match assets.texture_rgba("minecraft", file) {
            Some((w, h, px)) => colormap_sample(&px, w, h, temperature, downfall),
            None => [255, 255, 255],
        }
    };

    let hex = |key: &str| -> Option<[u8; 3]> {
        let v = effects?.get(key)?;
        parse_color(v)
    };

    Tints {
        grass: hex("grass_color").unwrap_or_else(|| sample("colormap/grass")),
        foliage: hex("foliage_color").unwrap_or_else(|| sample("colormap/foliage")),
        water: hex("water_color").unwrap_or([255, 255, 255]),
    }
}

/// Biome `effects` colours are either `"#rrggbb"` or a packed integer.
fn parse_color(v: &serde_json::Value) -> Option<[u8; 3]> {
    match v {
        serde_json::Value::String(s) => {
            let h = s.trim_start_matches('#');
            if h.len() != 6 {
                return None;
            }
            let n = u32::from_str_radix(h, 16).ok()?;
            Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
        }
        serde_json::Value::Number(n) => {
            let n = n.as_u64()? as u32;
            Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
        }
        _ => None,
    }
}

/// Index a 256×256 colormap the way vanilla does: `x` from temperature, `y` from
/// downfall scaled by temperature, both clamped into the table's lower triangle.
fn colormap_sample(px: &[u8], w: u32, h: u32, temperature: f32, downfall: f32) -> [u8; 3] {
    let t = temperature.clamp(0.0, 1.0);
    let d = (downfall.clamp(0.0, 1.0) * t).clamp(0.0, 1.0);
    let x = ((1.0 - t) * (w.saturating_sub(1)) as f32).round() as u32;
    let y = ((1.0 - d) * (h.saturating_sub(1)) as f32).round() as u32;
    let i = ((y.min(h - 1) * w + x.min(w - 1)) * 4) as usize;
    if i + 2 >= px.len() {
        return [255, 255, 255];
    }
    [px[i], px[i + 1], px[i + 2]]
}

/// A source of block appearances.
///
/// Two implement it: [`Deriver`], which reads a client jar, and [`PaletteTable`],
/// a table someone derived earlier. The viewer takes either, so a page can be
/// built on a machine with no jar — and so a creator can hand it a table of
/// their own resource pack's colours instead of vanilla's.
pub trait Appearances {
    /// Resolve one blockstate string.
    fn appearance(&self, state: &str) -> Result<Appearance, Unresolved>;
    /// The biome whose tints these appearances carry.
    fn biome(&self) -> &str;
}

impl Appearances for Deriver<'_> {
    fn appearance(&self, state: &str) -> Result<Appearance, Unresolved> {
        Deriver::appearance(self, state)
    }
    fn biome(&self) -> &str {
        Deriver::biome(self)
    }
}

impl Appearances for PaletteTable {
    fn appearance(&self, state: &str) -> Result<Appearance, Unresolved> {
        if let Some(a) = self.entries.get(state) {
            return Ok(a.clone());
        }
        // A state the table recorded as unresolved keeps its recorded reason; a
        // state the table never saw is missing from it, which is the same
        // problem one layer out and must not be reported as a resolved colour.
        Err(*self
            .unresolved
            .get(state)
            .unwrap_or(&Unresolved::NoBlockstate))
    }
    fn biome(&self) -> &str {
        &self.biome
    }
}

/// A derived appearance table, and the format `--palette` reads.
///
/// Keyed by full blockstate string so `oak_slab[type=top]` and
/// `oak_slab[type=bottom]` are different shapes. Serialization is a `BTreeMap`,
/// so the file is byte-identical for the same inputs (ADR-0006).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteTable {
    /// Format version of this file.
    pub version: u32,
    /// The biome whose tints were applied.
    pub biome: String,
    /// Blockstate string → appearance.
    pub entries: BTreeMap<String, Appearance>,
    /// Blockstate strings that could not be resolved, with the reason.
    #[serde(default)]
    pub unresolved: BTreeMap<String, Unresolved>,
}

/// Current [`PaletteTable::version`].
pub const PALETTE_VERSION: u32 = 1;

impl PaletteTable {
    /// Derive a table for a set of blockstate strings. Air-like states are
    /// skipped entirely — they are absence, not a failure to resolve.
    pub fn derive<'s>(
        deriver: &Deriver<'_>,
        states: impl IntoIterator<Item = &'s str>,
    ) -> PaletteTable {
        let mut entries = BTreeMap::new();
        let mut unresolved = BTreeMap::new();
        for s in states {
            if is_air(s) || entries.contains_key(s) || unresolved.contains_key(s) {
                continue;
            }
            match deriver.appearance(s) {
                Ok(a) => {
                    entries.insert(s.to_string(), a);
                }
                Err(e) => {
                    unresolved.insert(s.to_string(), e);
                }
            }
        }
        PaletteTable {
            version: PALETTE_VERSION,
            biome: deriver.biome().to_string(),
            entries,
            unresolved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_state_strings() {
        assert_eq!(
            base_id("minecraft:stone"),
            ("minecraft".into(), "stone".into())
        );
        assert_eq!(
            base_id("minecraft:oak_slab[type=top]"),
            ("minecraft".into(), "oak_slab".into())
        );
        assert_eq!(base_id("stone"), ("minecraft".into(), "stone".into()));
        let p = properties("minecraft:oak_slab[type=top,waterlogged=false]");
        assert_eq!(p.get("type").unwrap(), "top");
        assert_eq!(p.get("waterlogged").unwrap(), "false");
        assert!(properties("minecraft:stone").is_empty());
    }

    #[test]
    fn air_like_states_are_absence() {
        for s in [
            "minecraft:air",
            "minecraft:cave_air",
            "minecraft:void_air",
            "minecraft:structure_void",
        ] {
            assert!(is_air(s), "{s}");
        }
        assert!(!is_air("minecraft:stone"));
    }

    /// An omitted property is a default, and a default cannot contradict — this
    /// is what makes bare `minecraft:grass_block` resolve at all.
    #[test]
    fn an_omitted_property_does_not_contradict_a_variant() {
        let props = BTreeMap::new();
        assert_eq!(score_variant("snowy=false", &props), (0, false));
        let mut declared = BTreeMap::new();
        declared.insert("snowy".to_string(), "true".to_string());
        assert_eq!(score_variant("snowy=false", &declared), (0, true));
        assert_eq!(score_variant("snowy=true", &declared), (1, false));
    }

    #[test]
    fn variant_selection_prefers_the_most_constrained_match() {
        let def: serde_json::Value = serde_json::from_str(
            r#"{"variants":{
                 "facing=north,half=bottom":{"model":"block/a"},
                 "facing=north,half=top":{"model":"block/b"},
                 "facing=south,half=bottom":{"model":"block/c"}}}"#,
        )
        .unwrap();
        let mut props = BTreeMap::new();
        props.insert("facing".into(), "north".into());
        props.insert("half".into(), "top".into());
        let got = select_models(&def, &props, "minecraft");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, "block/b");
    }

    /// Every variant contradicted must not silently fall through to some other
    /// block's model.
    #[test]
    fn a_fully_contradicted_variant_set_resolves_to_nothing() {
        let def: serde_json::Value =
            serde_json::from_str(r#"{"variants":{"facing=north":{"model":"block/a"}}}"#).unwrap();
        let mut props = BTreeMap::new();
        props.insert("facing".into(), "south".into());
        assert!(select_models(&def, &props, "minecraft").is_empty());
    }

    #[test]
    fn multipart_falls_back_to_the_union_when_nothing_is_satisfiable() {
        let def: serde_json::Value = serde_json::from_str(
            r#"{"multipart":[
                 {"when":{"north":"true"},"apply":{"model":"block/side_n"}},
                 {"when":{"south":"true"},"apply":{"model":"block/side_s"}}]}"#,
        )
        .unwrap();
        // Nothing declared: both cases are unsatisfiable, so the union renders.
        let got = select_models(&def, &BTreeMap::new(), "minecraft");
        assert_eq!(got.len(), 2);
        // One declared: only that case applies.
        let mut props = BTreeMap::new();
        props.insert("north".into(), "true".into());
        let got = select_models(&def, &props, "minecraft");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, "block/side_n");
    }

    #[test]
    fn multipart_or_conditions_match() {
        let when: serde_json::Value =
            serde_json::from_str(r#"{"OR":[{"north":"true"},{"south":"true"}]}"#).unwrap();
        let mut props = BTreeMap::new();
        props.insert("south".into(), "true".into());
        assert!(when_matches(&when, &props));
        props.clear();
        props.insert("south".into(), "false".into());
        assert!(!when_matches(&when, &props));
    }

    #[test]
    fn alternative_property_values_match() {
        let when: serde_json::Value = serde_json::from_str(r#"{"facing":"north|south"}"#).unwrap();
        let mut props = BTreeMap::new();
        props.insert("facing".into(), "south".into());
        assert!(when_matches(&when, &props));
        props.insert("facing".into(), "east".into());
        assert!(!when_matches(&when, &props));
    }

    #[test]
    fn a_weighted_variant_list_picks_deterministically() {
        let v: serde_json::Value = serde_json::from_str(
            r#"[{"model":"block/z","weight":3},{"model":"block/a"},{"model":"block/m"}]"#,
        )
        .unwrap();
        let got = model_refs(&v, "minecraft");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, "block/a");
    }

    #[test]
    fn quarter_turns_move_a_box_exactly() {
        // A bottom slab rotated 180° about X becomes a top slab.
        let (f, t) = rotate_box([0.0, 0.0, 0.0], [16.0, 8.0, 16.0], 180, 0);
        assert_eq!([clamp16(f[0]), clamp16(f[1]), clamp16(f[2])], [0, 8, 0]);
        assert_eq!([clamp16(t[0]), clamp16(t[1]), clamp16(t[2])], [16, 16, 16]);
        // A vertical post rotated 90° about X becomes a horizontal one.
        let (f, t) = rotate_box([6.0, 0.0, 6.0], [10.0, 16.0, 10.0], 90, 0);
        assert_eq!([clamp16(f[0]), clamp16(f[1]), clamp16(f[2])], [6, 6, 0]);
        assert_eq!([clamp16(t[0]), clamp16(t[1]), clamp16(t[2])], [10, 10, 16]);
        // No rotation is the identity.
        let (f, t) = rotate_box([1.0, 2.0, 3.0], [4.0, 5.0, 6.0], 0, 0);
        assert_eq!(f, [1.0, 2.0, 3.0]);
        assert_eq!(t, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn texture_refs_follow_indirection() {
        let mut t = BTreeMap::new();
        t.insert("all".to_string(), "#texture".to_string());
        t.insert("texture".to_string(), "block/stone".to_string());
        assert_eq!(
            resolve_texture_ref("#all", &t).as_deref(),
            Some("block/stone")
        );
        assert_eq!(resolve_texture_ref("#missing", &t), None);
        // A cycle terminates instead of hanging.
        let mut c = BTreeMap::new();
        c.insert("a".to_string(), "#b".to_string());
        c.insert("b".to_string(), "#a".to_string());
        assert_eq!(resolve_texture_ref("#a", &c), None);
    }

    #[test]
    fn opacity_needs_both_coverage_and_a_full_cube() {
        let solid = Appearance {
            rgb: [1, 2, 3],
            coverage: 255,
            shape: [0, 0, 0, 16, 16, 16],
        };
        assert!(solid.is_opaque_cube());
        let slab = Appearance {
            shape: [0, 0, 0, 16, 8, 16],
            ..solid.clone()
        };
        assert!(!slab.is_opaque_cube());
        let glass = Appearance {
            coverage: 40,
            ..solid.clone()
        };
        assert!(!glass.is_opaque_cube());
    }

    #[test]
    fn colormap_sampling_is_the_vanilla_index() {
        // A 256x256 ramp where the red channel encodes x and green encodes y.
        let (w, h) = (256u32, 256u32);
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                px[i] = x as u8;
                px[i + 1] = y as u8;
                px[i + 3] = 255;
            }
        }
        // Plains: temperature 0.8, downfall 0.4 -> x = (1-0.8)*255 = 51,
        // y = (1 - 0.4*0.8)*255 = 173.
        let c = colormap_sample(&px, w, h, 0.8, 0.4);
        assert_eq!(c[0], 51);
        assert_eq!(c[1], 173);
    }

    #[test]
    fn biome_colors_parse_as_hex_or_packed_int() {
        assert_eq!(
            parse_color(&serde_json::json!("#3f76e4")).unwrap(),
            [0x3f, 0x76, 0xe4]
        );
        assert_eq!(
            parse_color(&serde_json::json!(4159204u32)).unwrap(),
            [0x3f, 0x76, 0xe4]
        );
        assert!(parse_color(&serde_json::json!("nope")).is_none());
    }
}
