//! The slice of the pinned client jar one review page needs, and the checks that
//! say whether it is complete.
//!
//! The page draws real block models, so it carries real resources: for every
//! blockstate a prefab contains, that block's `blockstates/*.json`, the whole
//! `parent` chain of every model it names, and every `.png` those models
//! reference — plus the block-entity textures the model files never mention,
//! because a chest, a sign, a bell and a banner are drawn by code rather than by
//! a model.
//!
//! # What is checked, and why each check has to be here
//!
//! Every failure this module reports is **silent by construction** downstream: a
//! block with no definition, a model whose parent is absent, a texture id that
//! resolves to nothing — all three render as the same magenta checker, which is
//! indistinguishable from a prefab that legitimately names a block the pinned
//! version dropped. So the resolution happens here, where the ids are still
//! separable, and each failure is named with its cell count.
//!
//! The one that cost the most to find is the **block-entity texture table**
//! ([`SPECIAL_TEXTURES`]). The renderer's own copy of it is private, so this is
//! a second copy by necessity; a wrong entry in it is invisible, because a
//! texture that does not exist and a texture that was never asked for look the
//! same in a finished picture. Every id the table produces is therefore resolved
//! against the jar at page-build time, with a binding count, and an id that
//! resolves to nothing is an error rather than a warning — it means this table
//! and the pinned version disagree, and no picture built from it can be trusted.
//!
//! # Determinism
//!
//! Every map is a `BTreeMap` and every list is sorted, so the same jar and the
//! same prefab produce the same bytes (ADR-0006).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::view::assets::Assets;
use crate::view::blockcolor::{base_id, is_air};

/// One texture as the page carries it.
#[derive(Debug, Clone, Serialize)]
pub struct Texture {
    /// Base64 of the `.png` file, exactly as the jar holds it.
    pub b64: String,
    /// Image width in pixels.
    pub w: u32,
    /// Image height in pixels.
    pub h: u32,
    /// Cell height the atlas packs. Equal to `h` for a still texture; equal to
    /// `w` for an animated one, whose file is a vertical strip of square frames
    /// and whose first frame is the resting image.
    pub ch: u32,
    /// Every pixel is fully opaque. Not sent to the page — it decides render
    /// flags here, and a page that carried it would be carrying a fact it never
    /// reads.
    #[serde(skip)]
    pub opaque: bool,
    /// Some pixel is partly transparent (as opposed to fully transparent, which
    /// is a cutout). Not sent to the page.
    #[serde(skip)]
    pub translucent: bool,
}

/// deepslate's per-block render flags. Derived from model geometry and texture
/// alpha, because the client jar does not carry them.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Flags {
    /// A full cube of fully opaque textures: it hides its neighbours' faces.
    pub opaque: bool,
    /// Drawn in the transparent pass.
    pub semi_transparent: bool,
    /// Hides its own internal faces against a like neighbour (water against
    /// water).
    pub self_culling: bool,
}

/// Why one blockstate will not be drawn as the game draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Missing {
    /// The pinned version has no `blockstates/<id>.json` — the block does not
    /// exist at this version at all.
    Blockstate,
    /// A model the definition names is absent, so some variant draws nothing.
    Model,
    /// A texture a model names is absent, so some face draws the checker.
    Texture,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Missing::Blockstate => write!(f, "no blockstate definition at this version"),
            Missing::Model => write!(f, "a model the definition names is absent"),
            Missing::Texture => write!(f, "a texture a model names is absent"),
        }
    }
}

/// A blockstate the page cannot draw faithfully, and why.
#[derive(Debug, Clone, Serialize)]
pub struct Unresolved {
    /// The blockstate string as the palette writes it.
    pub state: String,
    /// Which half of the resolution failed.
    pub reason: Missing,
    /// The resource id that was absent.
    pub detail: String,
    /// How many cells of the page carry this state.
    pub cells: u32,
}

/// A palette entry that leaves properties unwritten, with what the game fills
/// them with.
///
/// Legal, and the running server places the right block — but the file then
/// means something only a running server can work out. This is what stops the
/// page reporting a clean resolution over a building whose walls are the wrong
/// shape: a `cobblestone_wall` with nothing written matches no `multipart` case,
/// and a reader that unions every case draws a solid cube.
#[derive(Debug, Clone, Serialize)]
pub struct UnderSpecified {
    /// The blockstate string as the palette writes it.
    pub state: String,
    /// Property → the value the pinned version's default state supplies.
    pub filled: BTreeMap<String, String>,
    /// True when the block's definition is `multipart`, where an unwritten
    /// property does not merely pick a different model — it matches no case at
    /// all, and the block would not be drawn.
    pub multipart: bool,
    /// How many cells of the page carry this state.
    pub cells: u32,
}

/// The block-entity textures the renderer reaches for that no model file names.
///
/// The predicate is on the block id; the values are texture ids. This is a
/// second copy of a table that lives, private, inside the renderer — see the
/// module docs for why that is unavoidable and what is done about it.
const SPECIAL_TEXTURES: &[(&str, &[&str])] = &[
    // Chests, by their own variant name rather than by the block id.
    ("chest", &["entity/chest/normal"]),
    ("trapped_chest", &["entity/chest/trapped"]),
    ("ender_chest", &["entity/chest/ender"]),
    ("copper_chest", &["entity/chest/copper"]),
    ("exposed_copper_chest", &["entity/chest/copper_exposed"]),
    ("weathered_copper_chest", &["entity/chest/copper_weathered"]),
    ("oxidized_copper_chest", &["entity/chest/copper_oxidized"]),
    ("waxed_copper_chest", &["entity/chest/copper"]),
    (
        "waxed_exposed_copper_chest",
        &["entity/chest/copper_exposed"],
    ),
    (
        "waxed_weathered_copper_chest",
        &["entity/chest/copper_weathered"],
    ),
    (
        "waxed_oxidized_copper_chest",
        &["entity/chest/copper_oxidized"],
    ),
    // Skulls.
    ("skeleton_skull", &["entity/skeleton/skeleton"]),
    ("skeleton_wall_skull", &["entity/skeleton/skeleton"]),
    (
        "wither_skeleton_skull",
        &["entity/skeleton/wither_skeleton"],
    ),
    (
        "wither_skeleton_wall_skull",
        &["entity/skeleton/wither_skeleton"],
    ),
    ("zombie_head", &["entity/zombie/zombie"]),
    ("zombie_wall_head", &["entity/zombie/zombie"]),
    ("creeper_head", &["entity/creeper/creeper"]),
    ("creeper_wall_head", &["entity/creeper/creeper"]),
    ("dragon_head", &["entity/enderdragon/dragon"]),
    ("dragon_wall_head", &["entity/enderdragon/dragon"]),
    ("piglin_head", &["entity/piglin/piglin"]),
    ("piglin_wall_head", &["entity/piglin/piglin"]),
    ("player_head", &["entity/player/wide/steve"]),
    ("player_wall_head", &["entity/player/wide/steve"]),
    // Singletons.
    (
        "decorated_pot",
        &[
            "entity/decorated_pot/decorated_pot_side",
            "entity/decorated_pot/decorated_pot_base",
        ],
    ),
    ("bell", &["entity/bell/bell_body"]),
    ("conduit", &["entity/conduit/base"]),
    // Fluids: deepslate draws these itself from the still/flow textures, which
    // no model file references.
    ("water", &["block/water_still", "block/water_flow"]),
    ("lava", &["block/lava_still", "block/lava_flow"]),
];

/// Block-id suffixes whose texture id is derived from the wood/colour stem.
///
/// The banner and shield base textures sit at the jar's top level
/// (`entity/banner_base.png`), not inside the pattern folder; the vendored
/// renderer is patched to ask for them there. See
/// `tools/build-deepslate-bundle.sh`.
const SPECIAL_SUFFIXES: &[(&str, &str, &str)] = &[
    // (suffix, texture prefix, "" | the suffix to strip beyond the match)
    ("_wall_hanging_sign", "entity/signs/hanging/", ""),
    ("_hanging_sign", "entity/signs/hanging/", ""),
    ("_wall_sign", "entity/signs/", ""),
    ("_sign", "entity/signs/", ""),
    ("_bed", "entity/bed/", ""),
    ("_shulker_box", "entity/shulker/shulker_", ""),
];

/// Everything one page needs from the jar, plus what it could not find.
#[derive(Debug, Default)]
pub struct PageResources {
    /// `minecraft:stone` → its `blockstates/stone.json`.
    pub blockstates: BTreeMap<String, serde_json::Value>,
    /// `minecraft:block/stone` → its model JSON, parents included.
    pub models: BTreeMap<String, serde_json::Value>,
    /// `minecraft:block/stone` → the `.png`.
    pub textures: BTreeMap<String, Texture>,
    /// Per-block render flags.
    pub flags: BTreeMap<String, Flags>,
    /// Per-block default state, from the pinned registry.
    pub defaults: BTreeMap<String, BTreeMap<String, String>>,
    /// Blockstates that will not be drawn as the game draws them.
    pub unresolved: Vec<Unresolved>,
    /// Palette entries that leave properties unwritten.
    pub under_specified: Vec<UnderSpecified>,
    /// Block-entity texture ids this table asked for that the asset source does
    /// not have, as `(block, texture id)`.
    pub special_unresolved: Vec<(String, String)>,
    /// How many block-entity texture ids were resolved — the binding count of
    /// the check above. Zero over a library that contains a chest would mean the
    /// table stopped matching, which is the failure it exists to catch.
    pub special_bound: usize,
}

/// Extract everything the page needs for `states`, which are full blockstate
/// strings (`minecraft:oak_stairs[facing=north,...]`) with their cell counts.
pub fn extract(assets: &Assets, states: &BTreeMap<String, u32>) -> PageResources {
    let registry = delvewright_dsl::blocks::BlockRegistry::v1_21_11();
    let mut out = PageResources::default();

    // Distinct block ids, so a definition is read once however many states of it
    // the prefab contains.
    let mut names: BTreeSet<String> = BTreeSet::new();
    for state in states.keys() {
        if is_air(state) {
            continue;
        }
        let (ns, id) = base_id(state);
        names.insert(format!("{ns}:{id}"));
    }

    for name in &names {
        let (ns, id) = base_id(name);
        let Some(def) = assets.blockstate(&ns, &id) else {
            for (state, cells) in states {
                if &block_name(state) == name {
                    out.unresolved.push(Unresolved {
                        state: state.clone(),
                        reason: Missing::Blockstate,
                        detail: format!("assets/{ns}/blockstates/{id}.json"),
                        cells: *cells,
                    });
                }
            }
            continue;
        };
        for model in model_refs(&def) {
            load_model(assets, &qualify(&model), &mut out, name, states);
        }
        out.blockstates.insert(name.clone(), def);
        if let Some(default) = registry.default_state(name) {
            out.defaults.insert(name.clone(), default.clone());
        }
    }

    special_textures(assets, &names, &mut out);
    derive_flags(&names, &mut out);
    under_specified(registry, states, &mut out);

    out.unresolved.sort_by(|a, b| {
        (&a.state, a.reason as u8, &a.detail).cmp(&(&b.state, b.reason as u8, &b.detail))
    });
    out.unresolved
        .dedup_by(|a, b| a.state == b.state && a.detail == b.detail);
    out.under_specified.sort_by(|a, b| a.state.cmp(&b.state));
    out.special_unresolved.sort();
    out
}

/// The `ns:id` half of a blockstate string.
fn block_name(state: &str) -> String {
    let (ns, id) = base_id(state);
    format!("{ns}:{id}")
}

/// `block/stone` → `minecraft:block/stone`; an already-qualified id is left
/// alone.
fn qualify(reference: &str) -> String {
    if reference.contains(':') {
        reference.to_string()
    } else {
        format!("minecraft:{reference}")
    }
}

/// Every model a blockstate definition can select, variants and multipart alike.
fn model_refs(def: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |v: &serde_json::Value| match v {
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(m) = item.get("model").and_then(|m| m.as_str()) {
                    out.push(m.to_string());
                }
            }
        }
        other => {
            if let Some(m) = other.get("model").and_then(|m| m.as_str()) {
                out.push(m.to_string());
            }
        }
    };
    if let Some(variants) = def.get("variants").and_then(|v| v.as_object()) {
        for v in variants.values() {
            push(v);
        }
    }
    if let Some(parts) = def.get("multipart").and_then(|v| v.as_array()) {
        for part in parts {
            if let Some(apply) = part.get("apply") {
                push(apply);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Load a model, its `parent` chain, and every texture they name.
fn load_model(
    assets: &Assets,
    id: &str,
    out: &mut PageResources,
    owner: &str,
    states: &BTreeMap<String, u32>,
) {
    if out.models.contains_key(id) {
        return;
    }
    let (ns, path) = base_id(id);
    let Some(model) = assets.model(&ns, &path) else {
        note(
            out,
            owner,
            states,
            Missing::Model,
            &format!("assets/{ns}/models/{path}.json"),
        );
        return;
    };
    // Insert before recursing: a malformed jar with a parent cycle must not
    // recurse forever.
    out.models.insert(id.to_string(), model.clone());
    if let Some(parent) = model.get("parent").and_then(|p| p.as_str()) {
        load_model(assets, &qualify(parent), out, owner, states);
    }
    if let Some(textures) = model.get("textures").and_then(|t| t.as_object()) {
        for value in textures.values() {
            let Some(reference) = value.as_str() else {
                continue;
            };
            if reference.starts_with('#') {
                continue;
            }
            want_texture(assets, &qualify(reference), out, owner, states);
        }
    }
}

/// `minecraft:missingno` is vanilla's own sentinel: `models/block/air.json`
/// names it as a particle texture on a block that is never drawn. It is not a
/// finding.
const SENTINEL_TEXTURE: &str = "minecraft:missingno";

/// Read a texture and record it, or record that it is absent.
fn want_texture(
    assets: &Assets,
    id: &str,
    out: &mut PageResources,
    owner: &str,
    states: &BTreeMap<String, u32>,
) {
    if out.textures.contains_key(id) {
        return;
    }
    let (ns, path) = base_id(id);
    let Some(bytes) = assets.read(&format!("assets/{ns}/textures/{path}.png")) else {
        if id != SENTINEL_TEXTURE {
            note(
                out,
                owner,
                states,
                Missing::Texture,
                &format!("assets/{ns}/textures/{path}.png"),
            );
        }
        return;
    };
    let Some(texture) = decode(&bytes, assets, &ns, &path) else {
        note(
            out,
            owner,
            states,
            Missing::Texture,
            &format!("assets/{ns}/textures/{path}.png (unreadable PNG)"),
        );
        return;
    };
    out.textures.insert(id.to_string(), texture);
}

/// Decode a texture's dimensions, animation strip height and alpha character.
fn decode(bytes: &[u8], assets: &Assets, ns: &str, path: &str) -> Option<Texture> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    // An animated texture is a vertical film strip of square frames; the atlas
    // takes the first frame. The `.mcmeta` beside it is what declares the
    // animation, so a tall texture with no `.mcmeta` (a sign, a bed) keeps its
    // full height.
    let animated = assets
        .read(&format!("assets/{ns}/textures/{path}.png.mcmeta"))
        .is_some()
        && h > w
        && h % w == 0;
    // Fully opaque, versus partly-transparent (glass — the transparent pass),
    // versus fully-transparent-in-places (leaves, a ladder — a cutout, which is
    // drawn in the opaque pass but does not occlude).
    let mut opaque = true;
    let mut translucent = false;
    for pixel in img.pixels() {
        match pixel.0[3] {
            255 => {}
            0 => opaque = false,
            _ => {
                opaque = false;
                translucent = true;
            }
        }
    }
    Some(Texture {
        b64: base64(bytes),
        w,
        h,
        ch: if animated { w } else { h },
        opaque,
        translucent,
    })
}

/// Record a missing resource against every state of the block that needs it.
fn note(
    out: &mut PageResources,
    owner: &str,
    states: &BTreeMap<String, u32>,
    reason: Missing,
    detail: &str,
) {
    for (state, cells) in states {
        if block_name(state) == owner {
            out.unresolved.push(Unresolved {
                state: state.clone(),
                reason,
                detail: detail.to_string(),
                cells: *cells,
            });
        }
    }
}

/// Resolve the block-entity textures, and refuse an id the jar does not have.
fn special_textures(assets: &Assets, names: &BTreeSet<String>, out: &mut PageResources) {
    for name in names {
        let (_, id) = base_id(name);
        let mut wanted: Vec<String> = Vec::new();
        for (block, textures) in SPECIAL_TEXTURES {
            if &id == block {
                wanted.extend(textures.iter().map(|t| t.to_string()));
            }
        }
        for (suffix, prefix, _) in SPECIAL_SUFFIXES {
            if let Some(stem) = id.strip_suffix(suffix) {
                wanted.push(format!("{prefix}{stem}"));
            }
        }
        if id.ends_with("_banner") {
            wanted.push("entity/banner_base".to_string());
        }
        for texture in wanted {
            let qualified = qualify(&texture);
            let (ns, path) = base_id(&qualified);
            match assets.read(&format!("assets/{ns}/textures/{path}.png")) {
                Some(bytes) => {
                    out.special_bound += 1;
                    if !out.textures.contains_key(&qualified)
                        && let Some(t) = decode(&bytes, assets, &ns, &path)
                    {
                        out.textures.insert(qualified, t);
                    }
                }
                None => out
                    .special_unresolved
                    .push((name.clone(), qualified.clone())),
            }
        }
    }
}

/// Derive deepslate's per-block flags from model geometry and texture alpha.
///
/// The client jar does not carry them: misode's own app takes them from a
/// separate metadata repository. They are derivable from the two facts the jar
/// does carry — is every selectable model a full 16³ cube with six faces, and is
/// every texture those faces name fully opaque — which is exactly what `opaque`
/// means to the renderer (this block hides its neighbours' faces).
fn derive_flags(names: &BTreeSet<String>, out: &mut PageResources) {
    for name in names {
        let Some(def) = out.blockstates.get(name) else {
            continue;
        };
        let refs = model_refs(def);
        let mut full = !refs.is_empty();
        let mut translucent = false;
        let mut cutout = false;
        for reference in &refs {
            let (elements, textures) = flatten(&out.models, &qualify(reference), 0);
            if !is_full_cube(&elements) {
                full = false;
            }
            for element in &elements {
                let Some(faces) = element.get("faces").and_then(|f| f.as_object()) else {
                    continue;
                };
                for face in faces.values() {
                    let Some(reference) = face.get("texture").and_then(|t| t.as_str()) else {
                        continue;
                    };
                    let Some(id) = resolve_texture_ref(reference, &textures, 0) else {
                        continue;
                    };
                    let Some(texture) = out.textures.get(&qualify(&id)) else {
                        continue;
                    };
                    if texture.translucent {
                        translucent = true;
                    } else if !texture.opaque {
                        cutout = true;
                    }
                }
            }
        }
        // Fluids have no model elements at all — the renderer draws them itself
        // — so the geometry test says nothing about them and their kind does.
        let fluid = name == "minecraft:water" || name == "minecraft:lava";
        let semi = translucent || fluid;
        out.flags.insert(
            name.clone(),
            Flags {
                opaque: full && !translucent && !cutout && !fluid,
                semi_transparent: semi,
                // A like neighbour hides a like face: water against water, glass
                // against glass. Only meaningful for a block that fills its cell.
                self_culling: semi && (full || fluid),
            },
        );
    }
}

/// Is this the one 16³ element with all six faces that "full cube" means?
fn is_full_cube(elements: &[serde_json::Value]) -> bool {
    let [element] = elements else {
        return false;
    };
    let corner = |key: &str, want: f64| {
        element
            .get(key)
            .and_then(|v| v.as_array())
            .is_some_and(|a| a.len() == 3 && a.iter().all(|v| v.as_f64() == Some(want)))
    };
    corner("from", 0.0)
        && corner("to", 16.0)
        && element
            .get("faces")
            .and_then(|f| f.as_object())
            .is_some_and(|f| f.len() == 6)
}

/// A model's own `elements` and texture map, with its parents merged in.
fn flatten(
    models: &BTreeMap<String, serde_json::Value>,
    id: &str,
    depth: u8,
) -> (Vec<serde_json::Value>, BTreeMap<String, String>) {
    if depth > 8 {
        return (Vec::new(), BTreeMap::new());
    }
    let Some(model) = models.get(id) else {
        return (Vec::new(), BTreeMap::new());
    };
    let (parent_elements, mut textures) = match model.get("parent").and_then(|p| p.as_str()) {
        Some(parent) => flatten(models, &qualify(parent), depth + 1),
        None => (Vec::new(), BTreeMap::new()),
    };
    if let Some(own) = model.get("textures").and_then(|t| t.as_object()) {
        for (k, v) in own {
            if let Some(s) = v.as_str() {
                textures.insert(k.clone(), s.to_string());
            }
        }
    }
    let elements = match model.get("elements").and_then(|e| e.as_array()) {
        Some(e) => e.clone(),
        None => parent_elements,
    };
    (elements, textures)
}

/// Follow `#name` indirection to a concrete texture id.
fn resolve_texture_ref(
    reference: &str,
    textures: &BTreeMap<String, String>,
    depth: u8,
) -> Option<String> {
    if depth > 8 {
        return None;
    }
    match reference.strip_prefix('#') {
        Some(key) => resolve_texture_ref(textures.get(key)?, textures, depth + 1),
        None => Some(reference.to_string()),
    }
}

/// Properties a blockstate definition selects a model with — every key its
/// variant keys carry, and every key its `multipart` `when` clauses test.
///
/// This is the predicate that decides whether leaving a property out changes the
/// picture, and it is derived from the block's own definition rather than from a
/// list. Broader than the registry's shape-carrying class, which is the
/// `multipart` half alone: a stair's `facing` selects a rotated model through a
/// variant key and turning a stair the wrong way is exactly the kind of thing
/// this page exists to show. Narrower than "every property", which would report
/// a leaf block's `distance` — real, and invisible.
fn selecting_properties(def: &serde_json::Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Some(variants) = def.get("variants").and_then(|v| v.as_object()) {
        for key in variants.keys() {
            for pair in key.split(',') {
                if let Some((k, _)) = pair.split_once('=') {
                    out.insert(k.trim().to_string());
                }
            }
        }
    }
    if let Some(parts) = def.get("multipart").and_then(|v| v.as_array()) {
        for part in parts {
            collect_when(part.get("when"), &mut out);
        }
    }
    out
}

/// Every property key a `when` clause tests, `OR`/`AND` included.
fn collect_when(when: Option<&serde_json::Value>, out: &mut BTreeSet<String>) {
    let Some(when) = when else { return };
    let Some(map) = when.as_object() else { return };
    for (k, v) in map {
        if k == "OR" || k == "AND" {
            if let Some(list) = v.as_array() {
                for clause in list {
                    collect_when(Some(clause), out);
                }
            }
        } else {
            out.insert(k.clone());
        }
    }
}

/// Find palette entries that leave selecting properties unwritten.
fn under_specified(
    registry: &delvewright_dsl::blocks::BlockRegistry,
    states: &BTreeMap<String, u32>,
    out: &mut PageResources,
) {
    for (state, cells) in states {
        if is_air(state) {
            continue;
        }
        let name = block_name(state);
        let Some(def) = out.blockstates.get(&name) else {
            continue;
        };
        let selecting = selecting_properties(def);
        if selecting.is_empty() {
            continue;
        }
        let written = state_properties(state);
        let filled: BTreeMap<String, String> = registry
            .unwritten(&name, &written)
            .into_iter()
            .filter(|(k, _)| selecting.contains(k))
            .collect();
        if filled.is_empty() {
            continue;
        }
        out.under_specified.push(UnderSpecified {
            state: state.clone(),
            filled,
            multipart: def.get("multipart").is_some(),
            cells: *cells,
        });
    }
}

/// The `[a=b,c=d]` tail of a blockstate string as a map.
pub fn state_properties(state: &str) -> BTreeMap<String, String> {
    let Some(open) = state.find('[') else {
        return BTreeMap::new();
    };
    let inner = state[open + 1..].trim_end().trim_end_matches(']');
    inner
        .split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
fn base64(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_state_string_splits_into_its_properties() {
        let p = state_properties("minecraft:oak_stairs[facing=north,half=bottom]");
        assert_eq!(p["facing"], "north");
        assert_eq!(p["half"], "bottom");
        assert!(state_properties("minecraft:stone").is_empty());
    }

    /// Every id the special table can produce must be resolvable at the pin.
    /// This is the static half — that the table's own suffix rules produce the
    /// ids the emitter believes they do — with the jar half done at page build.
    #[test]
    fn the_special_table_derives_the_ids_it_claims() {
        let names: BTreeSet<String> = [
            "minecraft:oak_wall_sign",
            "minecraft:red_bed",
            "minecraft:blue_shulker_box",
            "minecraft:white_banner",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let mut derived: Vec<String> = Vec::new();
        for name in &names {
            let (_, id) = base_id(name);
            for (suffix, prefix, _) in SPECIAL_SUFFIXES {
                if let Some(stem) = id.strip_suffix(suffix) {
                    derived.push(format!("{prefix}{stem}"));
                    break;
                }
            }
            if id.ends_with("_banner") {
                derived.push("entity/banner_base".to_string());
            }
        }
        derived.sort();
        assert_eq!(
            derived,
            vec![
                "entity/banner_base",
                "entity/bed/red",
                "entity/shulker/shulker_blue",
                "entity/signs/oak",
            ]
        );
    }
}
