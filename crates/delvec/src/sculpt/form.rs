//! **The form document** — what `delvec sculpt` reads (spec-0087 §3.2).
//!
//! A form states a body as implicit solids in blocks, the ground it lies on,
//! the material it is made of by tone, the light placed in it and the anchors
//! a campaign binds. It is a library-asset document beside prefab metadata,
//! exported by `delvec schema --stage sculpt-form` and absent from
//! `--stage all`, so no stage document changes shape.
//!
//! # Two frames
//!
//! **Anchors are in the piece's frame** — they are prefab metadata, written
//! into the output as declared. **Solids and lights are in the body's frame**,
//! which is the piece's frame lowered so that body `y = 0` lies `sink` courses
//! below the apron's top surface: `piece y = body y + ground.top + 1 - sink`.
//! `x` and `z` are the same in both. So `sink` is one number that says how
//! deep the body lies in its ground, and changing it moves the body and its
//! light together.
//!
//! # What is refused here (`DW0951`, before anything is fitted)
//!
//! [`Form::check`] runs over the parsed document and refuses, naming the field:
//! a version this engine does not know; no `ground` (a floating body is
//! follow-up A); no anchor with `role: entry`; a palette, ground, family,
//! shelf-material or light block the pinned registry does not hold (the
//! `blocks-exist` gate, run over the declared states); a palette, ground or
//! shelf-material block that emits light (read from the relight pass's own
//! table, [`crate::compiler::light::emission`]); a `lights[]` block that emits
//! none; and a `shelf` whose knots rise more than one block per block of path.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use delvewright_dsl::prefab::{AnchorRole, Region};

use crate::grammar::block::BlockState;

/// The surface's version, fenced like a grammar program's
/// ([`crate::grammar::version`]): an engine meeting a version it does not know
/// refuses rather than reading the parts it recognises.
pub const LATEST_FORM_VERSION: &str = "1.0.0";

/// The least `spacing` a `hull` light may declare, in blocks.
pub const MIN_HULL_SPACING: f64 = 2.0;

/// Every form version this engine reads.
pub const SUPPORTED_FORM_VERSIONS: &[&str] = &["1.0.0"];

/// A body stated as solids over its own ground (spec-0087 §3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Form {
    /// The anchors a campaign binds, by name (`anchor/<stem>`), in the piece's
    /// frame. At least one carries `role: entry`.
    pub anchors: BTreeMap<String, FormAnchor>,
    /// The piece's extent in blocks, `[x, y, z]`. The ground fills its footprint.
    #[serde(rename = "box")]
    pub extent: [u32; 3],
    /// The form surface's version (`1.0.0`).
    pub form_version: String,
    /// The ground the body lies on. Required: a form with none is refused
    /// (a floating body is follow-up A of spec-0087).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground: Option<Ground>,
    /// The prefab id the output carries, `prefab/<kebab>`.
    pub id: String,
    /// Light placed where the room is designed, in the body's frame. Each
    /// replaces whatever the fit put at its cell.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lights: Vec<Light>,
    /// The seeded low-frequency weathering every `noisy` solid gets.
    pub noise: Noise,
    /// The material by tone, bleached (first) to weathered (last).
    pub palette: [Tone; 4],
    /// How many courses of the body lie below the apron's top: body `y = 0`
    /// lies at piece `y = ground.top + 1 - sink`.
    pub sink: u32,
    /// The body, stamped in order.
    pub solids: Vec<Solid>,
    /// Sub-voxels per block per axis for the octant fit: `2` or `4`.
    pub sub: u32,
}

/// The apron: full blocks of `block` at piece `y` in `[0, top]` wherever the
/// fitted body leaves air.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ground {
    /// A full block that emits no light.
    pub block: String,
    /// The apron's top course, piece `y`. A body stands on it at `top + 1`.
    pub top: u32,
}

/// The weathering noise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Noise {
    /// How far the surface moves, in blocks, at one standard deviation.
    pub amplitude: f64,
    /// The noise lattice's spacing, in blocks: the size of a weathering patch.
    pub cell: f64,
}

/// One tone of material: the full blocks it is built of, by weight, and the
/// family its slabs and stairs come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tone {
    /// The stair/slab stem: `<family>_stairs` and `<family>_slab` must both
    /// exist (`diorite`, `stone_brick`, `mossy_cobblestone`).
    pub family: String,
    /// Full blocks and their weights, `[[block, weight], …]`.
    pub full: Vec<(String, u32)>,
}

/// Light placed where the room is designed: either one block at a cell the
/// form names (`at`), or a distribution the sculpt derives over the body's
/// inside surface (`hull`). Exactly one of the two.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Light {
    /// The cell, body frame — a light placed by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[i32; 3]>,
    /// A block that emits light (`minecraft:soul_lantern[hanging=false]`).
    pub block: String,
    /// Sources the sculpt places itself, in the body's inside surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hull: Option<Hull>,
}

/// **Light set into the body's inside surface** (spec-0087 §9): the sculpt
/// finds every body block inside `within` whose face meets air with the body
/// over it, keeps those on the surfaces `on` names, and draws sources from
/// them by a seeded Poisson-disk distribution — no two closer than `spacing`
/// blocks — so the sources are staggered and irregular, never a grid, over
/// walls and vault alike.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hull {
    /// The partial block a `recessed` source sits behind: a `<family>_stairs`
    /// or `<family>_slab` id, oriented by the sculpt. Absent for `embedded`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    /// How a source sits in the surface.
    pub mode: LightMode,
    /// The surfaces sources may sit in: `wall`, `vault`, `floor` (a body
    /// block facing up; the ground is never a host).
    pub on: Vec<Surface>,
    /// The least distance between two sources of this entry, in blocks — the
    /// density, stated as the Poisson-disk radius.
    pub spacing: f64,
    /// The cells the surface is taken from, body frame, inclusive.
    pub within: CellBox,
}

/// An inclusive box of cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CellBox {
    /// The low corner.
    pub from: [i32; 3],
    /// The high corner.
    pub to: [i32; 3],
}

/// How a hull source sits in the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LightMode {
    /// The source replaces a block of the surface, flush with it — a cave's
    /// own light. The block must be a full cube.
    Embedded,
    /// The source sits one block behind the surface, hidden behind the
    /// `cover` set in the surface in front of it, with a one-block slot beside
    /// the cover through which the light reaches the room.
    Recessed,
}

/// A kind of surface, by the way its block faces the air.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// Faces air sideways.
    Wall,
    /// Faces air below it.
    Vault,
    /// Faces air above it.
    Floor,
}

impl Surface {
    /// The schema word.
    pub fn as_str(self) -> &'static str {
        match self {
            Surface::Wall => "wall",
            Surface::Vault => "vault",
            Surface::Floor => "floor",
        }
    }
}

/// An anchor, in prefab metadata's own anchor shape (the four fields a form
/// can state), in the piece's frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FormAnchor {
    /// Cardinal facing: `north`, `south`, `east` or `west`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    /// The anchor's cell.
    pub pos: [i32; 3],
    /// Local cell range (a gate, or furniture's own blocks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// What the anchor is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<AnchorRole>,
}

/// Whether a solid adds material or takes it away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    /// Fill the shape.
    Add,
    /// Empty the shape.
    Cut,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One implicit solid, body frame, in continuous block units (a cell `[x, y, z]`
/// spans `x..x+1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum Solid {
    /// A tapered capsule from `from` to `to`, its cross-section stretched
    /// `stretch_y` times vertically.
    Capsule {
        /// Add or cut.
        op: Op,
        /// One end of the axis.
        from: [f64; 3],
        /// The other end.
        to: [f64; 3],
        /// Radius at `from`.
        radius_from: f64,
        /// Radius at `to`.
        radius_to: f64,
        /// Vertical stretch of the cross-section (1 when absent).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stretch_y: Option<f64>,
        /// This solid's own material: every block whose centre it contains
        /// takes this tone instead of the palette's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        material: Option<Tone>,
        /// Weathered by the form's noise.
        #[serde(default, skip_serializing_if = "is_false")]
        noisy: bool,
    },
    /// An axis-aligned ellipsoid.
    Ellipsoid {
        /// Add or cut.
        op: Op,
        /// The centre.
        centre: [f64; 3],
        /// The three semi-axes.
        radii: [f64; 3],
        /// This solid's own material: every block whose centre it contains
        /// takes this tone instead of the palette's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        material: Option<Tone>,
        /// Weathered by the form's noise.
        #[serde(default, skip_serializing_if = "is_false")]
        noisy: bool,
    },
    /// A cylinder of `radius` and full `height` along `axis` through `centre`.
    Disc {
        /// Add or cut.
        op: Op,
        /// The centre.
        centre: [f64; 3],
        /// The cylinder's axis (any non-zero vector).
        axis: [f64; 3],
        /// The radius.
        radius: f64,
        /// The full height along the axis.
        height: f64,
        /// This solid's own material: every block whose centre it contains
        /// takes this tone instead of the palette's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        material: Option<Tone>,
        /// Weathered by the form's noise.
        #[serde(default, skip_serializing_if = "is_false")]
        noisy: bool,
    },
    /// An axis-aligned box between two corners.
    Box {
        /// Add or cut.
        op: Op,
        /// One corner.
        from: [f64; 3],
        /// The opposite corner.
        to: [f64; 3],
        /// This solid's own material: every block whose centre it contains
        /// takes this tone instead of the palette's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        material: Option<Tone>,
        /// Weathered by the form's noise.
        #[serde(default, skip_serializing_if = "is_false")]
        noisy: bool,
    },
    /// A walkway: solid for `depth` blocks under a feet surface interpolated
    /// along `path`, and clear for `clearance` blocks over it. Each knot is
    /// `[x, y_feet, z]`; a knot may rise at most one block per block of path.
    Shelf {
        /// The knots, in order.
        path: Vec<[f64; 3]>,
        /// The walkway's width, in blocks.
        width: f64,
        /// Clear headroom over the feet surface, in blocks.
        clearance: f64,
        /// Solid under the feet surface, in blocks.
        depth: f64,
        /// The walkway's own material; the body's palette when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        material: Option<Tone>,
    },
}

impl Solid {
    /// The solid's own material, if it declares one.
    pub fn material(&self) -> Option<&Tone> {
        match self {
            Solid::Capsule { material, .. }
            | Solid::Ellipsoid { material, .. }
            | Solid::Disc { material, .. }
            | Solid::Box { material, .. }
            | Solid::Shelf { material, .. } => material.as_ref(),
        }
    }

    /// The schema name of the shape, for a message.
    pub fn shape_name(&self) -> &'static str {
        match self {
            Solid::Capsule { .. } => "capsule",
            Solid::Ellipsoid { .. } => "ellipsoid",
            Solid::Disc { .. } => "disc",
            Solid::Box { .. } => "box",
            Solid::Shelf { .. } => "shelf",
        }
    }
}

/// **The form document's JSON Schema** — `delvec schema --stage sculpt-form`.
pub fn schema() -> serde_json::Value {
    let mut v = serde_json::to_value(schemars::schema_for!(Form))
        .expect("the form schema serializes to JSON");
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "title".into(),
            serde_json::Value::String("sculpt form".into()),
        );
        obj.insert(
            "description".into(),
            serde_json::Value::String(
                "The form document `delvec sculpt` reads (spec-0087): a body stated as implicit \
                 solids over its own ground, its material by tone, its light and its anchors. A \
                 library asset, not a stage document."
                    .into(),
            ),
        );
        obj.insert(
            crate::compiler::view::camera::SCHEMA_FILE_KEY.into(),
            serde_json::Value::String(FORMS_DIR_GLOB.into()),
        );
    }
    v
}

/// Where a campaign or the gallery keeps its forms — named by the schema export
/// so a tool reading it (the gallery's coverage gate) finds every form without a
/// second list of paths.
pub const FORMS_DIR_GLOB: &str = "forms/*.json";

/// The canonical bytes a form's hash is taken over: its serde JSON form, which
/// depends on the form's content and nothing else.
pub fn canonical_bytes(form: &Form) -> Vec<u8> {
    serde_json::to_vec(form).expect("a Form serialises to JSON")
}

/// `sha256:<64 hex digits>` over [`canonical_bytes`].
pub fn form_hash(form: &Form) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(canonical_bytes(form));
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Parse a block-state string as the grammar does, namespacing a bare id.
pub(crate) fn parse_state(s: &str) -> Result<BlockState, String> {
    s.parse::<BlockState>().map_err(|e| format!("{s:?}: {e}"))
}

impl Form {
    /// The piece `y` of body `y = 0`.
    pub fn body_floor(&self) -> i64 {
        match &self.ground {
            Some(g) => g.top as i64 + 1 - self.sink as i64,
            None => 0,
        }
    }

    /// The prefab id's stem — `prefab/<stem>`.
    pub fn stem(&self) -> Option<&str> {
        self.id.strip_prefix("prefab/")
    }

    /// **Every `DW0951` refusal**, in the order a reader meets the fields, or
    /// `Ok` when the document can be fitted. Runs before any fitting, so a
    /// refusal writes nothing.
    pub fn check(&self) -> Result<(), Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        let registry = crate::schem::blocks::BlockRegistry::v1_21_11();

        if !SUPPORTED_FORM_VERSIONS.contains(&self.form_version.as_str()) {
            out.push(format!(
                "`form_version` {:?} is not a form version this engine reads (it reads {}); \
                 nothing in the document was read past it",
                self.form_version,
                SUPPORTED_FORM_VERSIONS.join(", ")
            ));
            return Err(out);
        }
        match self.stem() {
            Some(stem) if crate::grammar::export::is_valid_id(stem) => {}
            _ => out.push(format!(
                "`id` {:?} is not `prefab/<id>` with an id of lowercase letters, digits and \
                 hyphens",
                self.id
            )),
        }
        if self.extent.contains(&0) {
            out.push(format!(
                "`box` {:?} is empty on an axis; a piece needs at least one cell on every axis",
                self.extent
            ));
        }
        if self.sub != 2 && self.sub != 4 {
            out.push(format!(
                "`sub` is {}; the octant fit samples 2 or 4 sub-voxels per block per axis",
                self.sub
            ));
        }
        match &self.ground {
            None => out.push(
                "the form declares no `ground`. A sculpted body lies on its own ground, which is \
                 what lets every walk and light instrument seed from grade; a body with nothing \
                 under it is follow-up A of spec-0087 (entry-seeded piece instruments) and is not \
                 built. Declare `ground: {block, top}`"
                    .to_string(),
            ),
            Some(g) => {
                if g.top as i64 >= self.extent[1] as i64 - 2 {
                    out.push(format!(
                        "`ground.top` is {} in a box {} tall; a body standing on the ground at \
                         y {} needs two courses of clearance inside the box",
                        g.top,
                        self.extent[1],
                        g.top + 1
                    ));
                }
            }
        }
        if self.sink as i64 > self.ground.as_ref().map_or(0, |g| g.top as i64 + 1) {
            out.push(format!(
                "`sink` is {}, deeper than the ground's {} course(s): the body would start below \
                 the piece",
                self.sink,
                self.ground.as_ref().map_or(0, |g| g.top + 1)
            ));
        }

        // --- anchors -----------------------------------------------------------
        if !self
            .anchors
            .values()
            .any(|a| a.role == Some(AnchorRole::Entry))
        {
            out.push(
                "no anchor declares `role: entry`. The entry is where a body arrives, where the \
                 sculpt's walk and pocket proofs start, and the one anchor a campaign does not \
                 name; declare one on the ground at grade"
                    .to_string(),
            );
        }
        for (name, a) in &self.anchors {
            if !name.starts_with("anchor/") || name.len() == "anchor/".len() {
                out.push(format!("anchor {name:?} is not named `anchor/<stem>`"));
            }
            if let Some(f) = &a.facing
                && crate::schem::stairs::Facing::parse(f).is_none()
            {
                out.push(format!(
                    "anchor {name:?} faces {f:?}; a facing is north, south, east or west"
                ));
            }
            if !self.inside(a.pos) {
                out.push(format!(
                    "anchor {name:?} at {:?} is outside the box {:?}",
                    a.pos, self.extent
                ));
            }
        }

        // --- blocks: they exist, and the families have both shapes ---------------
        let mut declared: Vec<(String, BlockState)> = Vec::new();
        let mut materials: Vec<(String, &Tone)> = self
            .palette
            .iter()
            .enumerate()
            .map(|(i, t)| (format!("palette[{i}]"), t))
            .collect();
        for (i, s) in self.solids.iter().enumerate() {
            if let Some(t) = s.material() {
                materials.push((format!("solids[{i}] ({}) material", s.shape_name()), t));
            }
        }
        let mut unparsed = Vec::new();
        for (field, tone) in &materials {
            if tone.full.is_empty() {
                out.push(format!("{field} lists no full block"));
            }
            for (block, weight) in &tone.full {
                if *weight == 0 {
                    out.push(format!("{field} gives {block:?} weight 0"));
                }
                match parse_state(block) {
                    Ok(s) => declared.push((format!("{field} full block"), s)),
                    Err(e) => unparsed.push(format!("{field} full block {e}")),
                }
            }
            for (shape, suffix) in [("stair", "_stairs"), ("slab", "_slab")] {
                let id = format!("minecraft:{}{suffix}", tone.family);
                if !registry.has(&id) {
                    out.push(format!(
                        "{field} family {:?} has no {shape}: `{id}` is not a block in Minecraft \
                         {}. A family is the stem both shapes share (`stone_brick` for \
                         `stone_brick_stairs` and `stone_brick_slab`)",
                        tone.family,
                        crate::schem::blocks::MC_VERSION
                    ));
                } else {
                    declared.push((
                        format!("{field} family {shape}"),
                        BlockState {
                            name: id,
                            properties: BTreeMap::new(),
                        },
                    ));
                }
            }
        }
        if let Some(g) = &self.ground {
            match parse_state(&g.block) {
                Ok(s) => declared.push(("ground block".to_string(), s)),
                Err(e) => unparsed.push(format!("ground block {e}")),
            }
        }
        for (i, l) in self.lights.iter().enumerate() {
            match parse_state(&l.block) {
                Ok(s) => declared.push((format!("lights[{i}] block"), s)),
                Err(e) => unparsed.push(format!("lights[{i}] block {e}")),
            }
            if let Some(cover) = l.hull.as_ref().and_then(|h| h.cover.as_ref()) {
                match parse_state(cover) {
                    Ok(s) => declared.push((format!("lights[{i}] (hull) cover"), s)),
                    Err(e) => unparsed.push(format!("lights[{i}] (hull) cover {e}")),
                }
            }
        }
        out.extend(unparsed);
        let states: Vec<BlockState> = declared.iter().map(|(_, s)| s.clone()).collect();
        let gate = crate::grammar::gates::gate_blocks_exist_over(&states);
        if gate.failed() {
            out.push(format!(
                "gate `{}` (examined {} declared block state(s)): {} — every block the form \
                 names must exist in the pinned version",
                gate.id, gate.bound, gate.detail
            ));
        }

        // --- what a full block must be, and what may and must emit light ---------
        for (field, state) in &declared {
            let id = crate::schem::convert::strip_ns(&state.name);
            if !registry.has(&state.name) {
                continue; // the gate above named it
            }
            let emits = crate::compiler::light::emission(&state.to_string());
            if field.ends_with("(hull) cover") {
                let stem = id
                    .strip_suffix("_stairs")
                    .or_else(|| id.strip_suffix("_slab"));
                if stem.is_none() || !state.properties.is_empty() {
                    out.push(format!(
                        "{field} `{state}` is not a bare `<family>_stairs` or `<family>_slab` id: \
                         the cover is a partial block the sculpt orients in front of the source, \
                         so it names the block and the sculpt writes its facing, half or type"
                    ));
                }
                if emits > 0 {
                    out.push(format!(
                        "{field} `{state}` emits light {emits}; a cover hides a source, it is not one"
                    ));
                }
                continue;
            }
            let is_light = field.starts_with("lights[");
            let is_full = field.ends_with("full block") || field == "ground block";
            if is_light {
                if emits == 0 {
                    out.push(format!(
                        "{field} `{state}` emits no light; a `lights[]` entry places light, so \
                         its block must emit some (the relight pass's own table)"
                    ));
                }
                continue;
            }
            if emits > 0 {
                out.push(format!(
                    "{field} `{state}` emits light {emits}: the floor and body of a sculpted \
                     piece are never paved with a glowing block. Light enters through `lights[]`, \
                     placed where the room is designed, and through cuts that reach the sky"
                ));
            }
            if is_full {
                if !registry.shape_carrying(&state.name).is_empty() {
                    out.push(format!(
                        "{field} `{id}` assembles its shape from its neighbours ({}); a fitted \
                         full block cannot state that, so it cannot be a full block here",
                        registry.shape_carrying(&state.name).join(", ")
                    ));
                }
                if delvewright_dsl::blockshape::collision_class(&state.name)
                    != delvewright_dsl::blockshape::Collision::FullCube
                {
                    out.push(format!(
                        "{field} `{id}` is not a full cube to a body, so it cannot be the full \
                         block the fit places"
                    ));
                }
            }
        }

        // --- solids ---------------------------------------------------------------
        for (i, s) in self.solids.iter().enumerate() {
            let finite = |v: f64| v.is_finite();
            match s {
                Solid::Capsule {
                    from,
                    to,
                    radius_from,
                    radius_to,
                    stretch_y,
                    ..
                } => {
                    if !(from.iter().chain(to).all(|v| finite(*v))
                        && *radius_from >= 0.0
                        && *radius_to >= 0.0
                        && finite(*radius_from)
                        && finite(*radius_to)
                        && stretch_y.is_none_or(|y| y > 0.0 && finite(y)))
                    {
                        out.push(format!(
                            "solids[{i}] (capsule) needs finite ends, radii >= 0 and stretch_y > 0"
                        ));
                    }
                }
                Solid::Ellipsoid { centre, radii, .. } => {
                    if !(centre.iter().all(|v| finite(*v))
                        && radii.iter().all(|r| *r > 0.0 && finite(*r)))
                    {
                        out.push(format!(
                            "solids[{i}] (ellipsoid) needs a finite centre and radii > 0"
                        ));
                    }
                }
                Solid::Disc {
                    centre,
                    axis,
                    radius,
                    height,
                    ..
                } => {
                    let len2: f64 = axis.iter().map(|v| v * v).sum();
                    if !(centre.iter().chain(axis).all(|v| finite(*v))
                        && len2 > 0.0
                        && *radius > 0.0
                        && *height > 0.0
                        && finite(*radius)
                        && finite(*height))
                    {
                        out.push(format!(
                            "solids[{i}] (disc) needs a finite centre, a non-zero axis, radius > 0 \
                             and height > 0"
                        ));
                    }
                }
                Solid::Box { from, to, .. } => {
                    if !from.iter().chain(to).all(|v| finite(*v)) {
                        out.push(format!("solids[{i}] (box) needs finite corners"));
                    }
                }
                Solid::Shelf {
                    path,
                    width,
                    clearance,
                    depth,
                    ..
                } => {
                    if path.len() < 2 {
                        out.push(format!(
                            "solids[{i}] (shelf) needs at least two knots in `path`"
                        ));
                    }
                    if !(*width > 0.0
                        && *clearance > 0.0
                        && *depth > 0.0
                        && finite(*width)
                        && finite(*clearance)
                        && finite(*depth)
                        && path.iter().flatten().all(|v| finite(*v)))
                    {
                        out.push(format!(
                            "solids[{i}] (shelf) needs finite knots and width, clearance and \
                             depth > 0"
                        ));
                    }
                    for (k, w) in path.windows(2).enumerate() {
                        let run =
                            ((w[1][0] - w[0][0]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
                        let rise = (w[1][1] - w[0][1]).abs();
                        if rise > run {
                            out.push(format!(
                                "solids[{i}] (shelf) rises {rise} block(s) over {run} block(s) of \
                                 path between knots {k} and {}: steeper than one block per block, \
                                 so its fit is a wall, not a stair. A route that genuinely needs a \
                                 ladder or a gap is follow-up B of spec-0087; lengthen the path, \
                                 or add knots that turn it",
                                k + 1
                            ));
                        }
                    }
                }
            }
        }
        if !(self.noise.amplitude >= 0.0
            && self.noise.amplitude.is_finite()
            && self.noise.cell > 0.0
            && self.noise.cell.is_finite())
        {
            out.push("`noise` needs amplitude >= 0 and cell > 0".to_string());
        }
        let inside_piece = |c: [i32; 3]| {
            let p = [c[0] as i64, c[1] as i64 + self.body_floor(), c[2] as i64];
            ((0..3).all(|a| p[a] >= 0 && p[a] < self.extent[a] as i64), p)
        };
        for (i, l) in self.lights.iter().enumerate() {
            match (&l.at, &l.hull) {
                (Some(_), Some(_)) | (None, None) => out.push(format!(
                    "lights[{i}] declares {}; a light is either placed by hand at a cell (`at`) \
                     or distributed over the body's inside surface (`hull`), exactly one",
                    if l.at.is_some() {
                        "both `at` and `hull`"
                    } else {
                        "neither `at` nor `hull`"
                    }
                )),
                _ => {}
            }
            if let Some(at) = l.at {
                let (inside, p) = inside_piece(at);
                if !inside {
                    out.push(format!(
                        "lights[{i}] at body {at:?} is piece {p:?}, outside the box {:?}",
                        self.extent
                    ));
                }
            }
            if let Some(h) = &l.hull {
                out.extend(self.check_hull(i, &l.block, h));
            }
        }
        if out.is_empty() { Ok(()) } else { Err(out) }
    }

    /// The refusals of one `hull` light (spec-0087 §9).
    fn check_hull(&self, i: usize, block: &str, h: &Hull) -> Vec<String> {
        let mut out = Vec::new();
        let field = format!("lights[{i}] (hull)");
        if !(h.spacing.is_finite() && h.spacing >= MIN_HULL_SPACING) {
            out.push(format!(
                "{field} `spacing` is {}; the least distance between two sources is a finite \
                 number of blocks, at least {MIN_HULL_SPACING}: two sources nearer than that \
                 touch, and the surface between them is gone",
                h.spacing
            ));
        }
        if h.on.is_empty() {
            out.push(format!(
                "{field} `on` names no surface; name `wall`, `vault` or `floor`"
            ));
        }
        let mut seen = h.on.clone();
        seen.sort();
        seen.dedup();
        if seen.len() != h.on.len() {
            out.push(format!("{field} `on` names a surface twice"));
        }
        let floor = self.body_floor();
        for (name, c) in [("from", h.within.from), ("to", h.within.to)] {
            let p = [c[0] as i64, c[1] as i64 + floor, c[2] as i64];
            if !(0..3).all(|a| p[a] >= 0 && p[a] < self.extent[a] as i64) {
                out.push(format!(
                    "{field} `within.{name}` at body {c:?} is piece {p:?}, outside the box {:?}",
                    self.extent
                ));
            }
        }
        if (0..3).any(|a| h.within.from[a] > h.within.to[a]) {
            out.push(format!(
                "{field} `within` runs from {:?} to {:?}: `from` is the low corner on every axis",
                h.within.from, h.within.to
            ));
        }
        let name = parse_state(block).map(|s| s.name).unwrap_or_default();
        match h.mode {
            LightMode::Embedded => {
                if h.cover.is_some() {
                    out.push(format!(
                        "{field} declares a `cover` with `mode: embedded`; an embedded source is \
                         flush with the surface and has nothing in front of it — a hidden source \
                         is `recessed`"
                    ));
                }
                if delvewright_dsl::blockshape::collision_class(&name)
                    != delvewright_dsl::blockshape::Collision::FullCube
                {
                    out.push(format!(
                        "{field} embeds `{block}`, which is not a full cube: set into the \
                         surface it would leave a hole in the hull. A small source (a lantern) \
                         is `recessed` behind a `cover`"
                    ));
                }
            }
            LightMode::Recessed => {
                if h.cover.is_none() {
                    out.push(format!(
                        "{field} is `recessed` with no `cover`; name the `<family>_stairs` or \
                         `<family>_slab` the source sits behind"
                    ));
                }
                if h.on.contains(&Surface::Floor) {
                    out.push(format!(
                        "{field} recesses into a `floor`: the slot beside the cover would be a \
                         hole in a surface a body walks. Recess into `wall` and `vault`; embed \
                         in a floor"
                    ));
                }
            }
        }
        out
    }

    fn inside(&self, pos: [i32; 3]) -> bool {
        (0..3).all(|a| pos[a] >= 0 && (pos[a] as i64) < self.extent[a] as i64)
    }
}
