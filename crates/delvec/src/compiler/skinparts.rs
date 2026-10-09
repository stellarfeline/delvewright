//! **A sheet is drawn to its own model's boxes** (spec-0097 §3–§4).
//!
//! An entity's geometry is fixed in the client by its layer definition, and a
//! texture can only fill the boxes that definition builds. The table of those
//! boxes — one row per box of every humanoid model a delve can dress — is
//! vendored at `crates/delvec/data/model-parts-1.21.11.json`, measured from the
//! pinned client by `tools/maintenance/extract-model-parts.py`, never typed.
//!
//! [`judge`] holds a sheet against its model and is the one rule both entry
//! points call: a `world.textures[]` row whose `replaces` the table binds to a
//! model ([`crate::compiler::textures::resolve`]), and a mannequin skin, against
//! `player` or `player_slim` by its `skin.model` (`read_skins` in `main.rs`).
//!
//! - [`DW_SKIN_UNSAMPLED`] (`DW0978`) — a pixel of non-zero alpha on no face
//!   any box of the model samples.
//! - [`DW_SKIN_UNSEEN`] (`DW0979`) — every opaque pixel on a face a body
//!   standing level with the model cannot see: the top of an unposed head or
//!   torso box above the standing eye, or the underside of one below it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use delvewright_dsl::{DwCode, ExitTier, SkinModel};
use serde::Deserialize;

delvewright_dsl::dw_code! {
    /// `DW0978` (spec-0097 §4.1): a sheet paints a pixel no box of its model
    /// samples — a mannequin skin against the player model it is worn on, or a
    /// `world.textures[]` row against the model the replaced texture is drawn
    /// with. The message names the model, the count, the first pixels, and the
    /// model whose layout the stray paint fits, where one does. Validation tier
    /// for a texture row and a skin alike, and again in the build.
    pub const DW_SKIN_UNSAMPLED: DwCode = DwCode::new("DW0978", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0979` (spec-0097 §4.2): every opaque pixel of a sheet lands on a face
    /// a body standing level with the model cannot see — the top of an unposed
    /// head or torso box above a standing player's eye, or the underside of one
    /// below it — so the sheet draws nothing anybody looks at.
    pub const DW_SKIN_UNSEEN: DwCode = DwCode::new("DW0979", ExitTier::Build);
}

const TABLE_JSON: &str = include_str!("../../data/model-parts-1.21.11.json");

/// One box a model builds.
#[derive(Debug, Clone, Deserialize)]
pub struct Cube {
    /// The part path (`head/hat`).
    pub part: String,
    pub u: u32,
    pub v: u32,
    pub w: u32,
    pub h: u32,
    pub d: u32,
    /// Outward growth on each side, in model pixels.
    pub grow: f64,
    pub mirror: bool,
    /// A rotation on the box's part chain at rest.
    pub posed: bool,
    /// Height of the box's top above the ground at rest, in model pixels.
    pub top: f64,
    /// Height of the box's bottom above the ground at rest, in model pixels.
    pub bottom: f64,
}

/// The six faces of the box unwrap, observer-relative.
pub const FACES: [&str; 6] = ["up", "down", "left", "front", "right", "back"];

impl Cube {
    /// Each face's rectangle on the sheet at scale 1: `(face, x, y, width, height)`.
    pub fn faces(&self) -> [(&'static str, u32, u32, u32, u32); 6] {
        let (u, v, w, h, d) = (self.u, self.v, self.w, self.h, self.d);
        [
            ("up", u + d, v, w, d),
            ("down", u + d + w, v, w, d),
            ("left", u, v + d, d, h),
            ("front", u + d, v + d, w, h),
            ("right", u + d + w, v + d, d, h),
            ("back", u + 2 * d + w, v + d, w, h),
        ]
    }

    /// Whether `face` of this box is one a body standing level with the model
    /// cannot see (spec-0097 §4.2).
    pub fn face_unseen(&self, face: &str, eye: f64) -> bool {
        if self.posed {
            return false;
        }
        let root = self.part.split('/').next().unwrap_or("");
        if root != "head" && root != "body" {
            return false;
        }
        match face {
            "up" => self.top > eye,
            "down" => self.bottom < eye,
            _ => false,
        }
    }
}

/// One model: the boxes a `ModelLayers` entry builds and the textures drawn with it.
#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    /// The `ModelLayers` field it was read from.
    pub layer: String,
    /// The renderer that binds its textures, cited.
    pub renderer: String,
    /// The texture size the model's UVs are in.
    pub texture_size: [u32; 2],
    /// The vanilla textures drawn with it, as resource locations.
    pub textures: Vec<String>,
    pub cubes: Vec<Cube>,
}

/// The mannequin's hidden-layers field, as read from the jar.
#[derive(Debug, Clone, Deserialize)]
pub struct Mannequin {
    pub field: String,
    pub layers: Vec<String>,
}

/// The instrument the table names.
#[derive(Debug, Clone, Deserialize)]
pub struct Instrument {
    pub extractor: String,
    pub dumper: String,
    pub revision: String,
}

/// The vendored table.
#[derive(Debug, Clone, Deserialize)]
pub struct Table {
    pub minecraft: String,
    pub client_jar_sha256: String,
    pub client_jar_sha1: String,
    pub client_mappings_sha1: String,
    pub instrument: Instrument,
    pub ground_y: f64,
    /// A standing player's eye, in model pixels above the ground.
    pub player_eye_height_px: f64,
    pub mannequin: Mannequin,
    pub models: BTreeMap<String, Model>,
}

/// The vendored table, parsed once.
pub fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(TABLE_JSON).expect("the vendored model-part table parses")
    })
}

/// The model a vanilla texture is drawn with, where the table binds one.
pub fn model_for_texture(replaces: &str) -> Option<(&'static str, &'static Model)> {
    table()
        .models
        .iter()
        .find(|(_, m)| m.textures.iter().any(|t| t == replaces))
        .map(|(k, m)| (k.as_str(), m))
}

/// The player model a mannequin skin is worn on.
pub fn model_for_skin(model: SkinModel) -> (&'static str, &'static Model) {
    let key = match model {
        SkinModel::Wide => "player",
        SkinModel::Slim => "player_slim",
    };
    let m = table()
        .models
        .get(key)
        .expect("the table carries both player models");
    (table().models.get_key_value(key).unwrap().0.as_str(), m)
}

/// Every cell a model's faces cover on a sheet `k` times its texture size, with
/// whether each is unseen: a cell is unseen only when every face covering it is.
fn footprint(model: &Model, k: u32) -> BTreeMap<(u32, u32), bool> {
    let eye = table().player_eye_height_px;
    let mut cells: BTreeMap<(u32, u32), bool> = BTreeMap::new();
    for c in &model.cubes {
        for (face, x0, y0, fw, fh) in c.faces() {
            let unseen = c.face_unseen(face, eye);
            for y in y0 * k..(y0 + fh) * k {
                for x in x0 * k..(x0 + fw) * k {
                    let e = cells.entry((x, y)).or_insert(true);
                    *e = *e && unseen;
                }
            }
        }
    }
    cells
}

/// Why a sheet is refused: the code and the reason, phrased to follow the
/// sheet's own description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: DwCode,
    pub reason: String,
}

/// Judge a decoded RGBA sheet against `model` (keyed `key` in the table).
///
/// `Ok(opaque)` with the count of opaque pixels judged, or the refusal. A sheet
/// whose size is not a whole multiple of the model's texture size is not judged
/// here (`DW0940` owns size for a texture row) and answers `Ok(0)`.
pub fn judge(key: &str, model: &Model, img: &image::RgbaImage) -> Result<usize, Refusal> {
    let [tw, th] = model.texture_size;
    let (w, h) = (img.width(), img.height());
    if w == 0 || w % tw != 0 || h * tw != w * th {
        return Ok(0);
    }
    let k = w / tw;
    let cells = footprint(model, k);
    let mut stray: Vec<(u32, u32)> = Vec::new();
    let mut opaque = 0usize;
    let mut seen = 0usize;
    for (x, y, px) in img.enumerate_pixels() {
        if px.0[3] == 0 {
            continue;
        }
        opaque += 1;
        match cells.get(&(x, y)) {
            None => stray.push((x, y)),
            Some(false) => seen += 1,
            Some(true) => {}
        }
    }
    if !stray.is_empty() {
        let first: Vec<String> = stray
            .iter()
            .take(4)
            .map(|(x, y)| format!("({x}, {y})"))
            .collect();
        let fits = other_layouts(key, model, &stray, k);
        let boxes: Vec<String> = model
            .cubes
            .iter()
            .map(|c| format!("`{}` at {},{} ({}×{}×{})", c.part, c.u, c.v, c.w, c.h, c.d))
            .collect();
        let hint = if fits.is_empty() {
            String::new()
        } else {
            let named: Vec<String> = fits.iter().map(|m| format!("`{m}`")).collect();
            format!(
                " Every one of them sits on the boxes of {}: the sheet was drawn to another \
                 model's layout, and `{key}` does not build those boxes.",
                named.join(", ")
            )
        };
        return Err(Refusal {
            code: DW_SKIN_UNSAMPLED,
            reason: format!(
                "paints {} opaque pixel(s) on no face the `{key}` model samples (first at {}), \
                 so they draw nothing.{hint} `{key}` builds {}. Clear those pixels, or draw \
                 them onto a box the model has",
                stray.len(),
                first.join(", "),
                boxes.join(", ")
            ),
        });
    }
    if opaque > 0 && seen == 0 {
        return Err(Refusal {
            code: DW_SKIN_UNSEEN,
            reason: format!(
                "paints its {opaque} opaque pixel(s) only on faces a body standing level with \
                 the `{key}` model cannot see — the top of a box above a standing player's eye \
                 ({} px), or the underside of one below it — so at eye level it draws \
                 nothing. Paint the faces that face the player: the front, the sides, the back",
                table().player_eye_height_px
            ),
        });
    }
    Ok(opaque)
}

/// Every other model of the same texture size whose footprint holds every stray
/// pixel, for the `DW0978` hint.
fn other_layouts(key: &str, model: &Model, stray: &[(u32, u32)], k: u32) -> Vec<&'static str> {
    table()
        .models
        .iter()
        .filter(|(other, m)| other.as_str() != key && m.texture_size == model.texture_size)
        .filter(|(_, m)| {
            let cells = footprint(m, k);
            stray.iter().all(|p| cells.contains_key(p))
        })
        .map(|(other, _)| other.as_str())
        .collect()
}

/// What the sheet judgement bound, for the run's binding line (spec-0097 §4.3):
/// how many sheets were declared that a model's boxes can judge, how many were
/// judged, against which model, and how many were refused.
#[derive(Debug, Clone, Default)]
pub struct Binding {
    /// Distinct `(texture_id, model)` pairs the campaign's bodies wear.
    pub skins_declared: usize,
    /// Of those, the ones whose `skins/<id>.png` decoded and was judged.
    pub skins_judged: usize,
    /// `world.textures[]` rows whose replaced texture the table binds to a model.
    pub rows_declared: usize,
    /// Of those, the ones that reached the model judgement (past `DW0939`,
    /// `DW0309` and `DW0940`).
    pub rows_judged: usize,
    /// Sheets judged per model key.
    pub by_model: BTreeMap<String, usize>,
    /// Sheets refused with `DW0978` or `DW0979`.
    pub refused: usize,
    /// The first skin refusal, for a build that reaches the bake unvalidated.
    pub first_refusal: Option<(DwCode, String)>,
}

impl Binding {
    /// `validation/sheet-gate.json`: the same counts, so a gate reading the
    /// build sees a judgement that stopped binding (`examined: 0`).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "examined": self.skins_judged + self.rows_judged,
            "declared": self.skins_declared + self.rows_declared,
            "skins_declared": self.skins_declared,
            "skins_judged": self.skins_judged,
            "rows_declared": self.rows_declared,
            "rows_judged": self.rows_judged,
            "by_model": self.by_model,
            "refused": self.refused,
            "table": format!("model-parts-{}.json", table().minecraft),
            "instrument_revision": table().instrument.revision,
        })
    }

    /// The line `delvec validate` and `delvec build` print under "what this run
    /// examined".
    pub fn line(&self) -> String {
        let models: Vec<String> = self
            .by_model
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect();
        format!(
            "sheet binding: {} of {} sheet(s) judged against their model's boxes — {} of {} \
             mannequin skin(s), {} of {} `world.textures[]` row(s) an entity model is drawn \
             with; by model: {}; {} refused (DW0978/DW0979)",
            self.skins_judged + self.rows_judged,
            self.skins_declared + self.rows_declared,
            self.skins_judged,
            self.skins_declared,
            self.rows_judged,
            self.rows_declared,
            if models.is_empty() {
                "none".to_string()
            } else {
                models.join(", ")
            },
            self.refused
        )
    }
}

/// Judge every mannequin skin the campaign's bodies wear (spec-0097 §4.3) —
/// against `player` for `wide`, `player_slim` for `slim` — once per
/// `(texture_id, model)` pair. `skins` maps an authored `texture_id` to the bytes
/// of `skins/<id>.png`; a missing file is `DW0309`'s at build and is counted
/// declared, not judged. Validation tier.
pub fn check_skins(
    campaign: &delvewright_dsl::Campaign,
    skins: &BTreeMap<String, Vec<u8>>,
    binding: &mut Binding,
) -> Vec<delvewright_dsl::Diagnostic> {
    let mut seen: BTreeSet<(String, &'static str)> = BTreeSet::new();
    let mut out = Vec::new();
    for site in delvewright_dsl::body_skin_sites(campaign) {
        let (key, model) = model_for_skin(site.skin.model);
        if !seen.insert((site.skin.texture_id.clone(), key)) {
            continue;
        }
        binding.skins_declared += 1;
        let Some(bytes) = skins.get(&site.skin.texture_id) else {
            continue;
        };
        let Ok(img) = image::load_from_memory_with_format(bytes, image::ImageFormat::Png) else {
            continue;
        };
        binding.skins_judged += 1;
        *binding.by_model.entry(key.to_string()).or_default() += 1;
        if let Err(r) = judge(key, model, &img.to_rgba8()) {
            binding.refused += 1;
            if binding.first_refusal.is_none() {
                binding.first_refusal = Some((
                    r.code,
                    format!(
                        "skin `{}` on `{}`: {}",
                        site.skin.texture_id,
                        site.body.id(),
                        r.reason
                    ),
                ));
            }
            out.push(delvewright_dsl::Diagnostic::error(
                r.code,
                site.body.stage(),
                format!("{}/texture_id", site.path),
                format!(
                    "skin `skins/{}.png` — `{}` wears it on the `{}` model — {}",
                    site.skin.texture_id,
                    site.body.id(),
                    site.skin.model.token(),
                    r.reason
                ),
            ));
        }
    }
    out
}

/// Count the `world.textures[]` half of the binding from what
/// [`crate::compiler::textures::resolve`] returned: a row bound to a model was
/// judged when it resolved or was refused by the model judgement itself.
pub fn count_texture_rows(
    campaign: &delvewright_dsl::Campaign,
    resolved: &[crate::compiler::textures::Resolved],
    findings: &[crate::compiler::textures::Finding],
    binding: &mut Binding,
) {
    for (i, row) in campaign.world.content.textures.iter().enumerate() {
        let Some((key, _)) = model_for_texture(&row.replaces) else {
            continue;
        };
        binding.rows_declared += 1;
        let pointer = format!("/content/textures/{i}/id");
        let refused = findings.iter().any(|f| {
            f.path == pointer && (f.code == DW_SKIN_UNSAMPLED || f.code == DW_SKIN_UNSEEN)
        });
        if refused || resolved.iter().any(|r| r.id == row.id) {
            binding.rows_judged += 1;
            binding.refused += usize::from(refused);
            *binding.by_model.entry(key.to_string()).or_default() += 1;
        }
    }
}

/// The model keys the table carries, sorted.
pub fn model_keys() -> BTreeSet<&'static str> {
    table().models.keys().map(String::as_str).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(model: &Model) -> image::RgbaImage {
        let [w, h] = model.texture_size;
        image::RgbaImage::new(w, h)
    }

    fn paint(img: &mut image::RgbaImage, x0: u32, y0: u32, w: u32, h: u32) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                img.put_pixel(x, y, image::Rgba([90, 120, 150, 255]));
            }
        }
    }

    fn cube<'a>(m: &'a Model, part: &str) -> &'a Cube {
        m.cubes.iter().find(|c| c.part == part).unwrap()
    }

    #[test]
    fn the_table_is_the_pinned_clients() {
        let t = table();
        assert_eq!(t.minecraft, "1.21.11");
        assert_eq!(
            t.client_jar_sha256,
            "1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd"
        );
        assert_eq!(t.models.len(), 17, "spec-0097 §3.1's model keys");
        assert_eq!(t.player_eye_height_px, 25.92);
        let player = &t.models["player"];
        let grown: Vec<(&str, u32, u32, f64)> = player
            .cubes
            .iter()
            .filter(|c| c.grow > 0.0)
            .map(|c| (c.part.as_str(), c.u, c.v, c.grow))
            .collect();
        assert_eq!(
            grown,
            vec![
                ("body/jacket", 16, 32, 0.25),
                ("head/hat", 32, 0, 0.5),
                ("left_arm/left_sleeve", 48, 48, 0.25),
                ("left_leg/left_pants", 0, 48, 0.25),
                ("right_arm/right_sleeve", 40, 32, 0.25),
                ("right_leg/right_pants", 0, 32, 0.25),
            ]
        );
        let zombie: Vec<&str> = t.models["zombie"]
            .cubes
            .iter()
            .filter(|c| c.grow > 0.0)
            .map(|c| c.part.as_str())
            .collect();
        assert_eq!(zombie, vec!["head/hat"], "a zombie's only second-layer box");
        let outer = &t.models["drowned_outer_layer"];
        for (part, u, v) in [
            ("head", 0, 0),
            ("body", 16, 16),
            ("right_arm", 40, 16),
            ("left_arm", 32, 48),
            ("right_leg", 0, 16),
            ("left_leg", 16, 48),
        ] {
            let c = cube(outer, part);
            assert_eq!(
                (c.u, c.v),
                (u, v),
                "the outer layer samples the BASE positions"
            );
            assert!(c.grow > 0.0);
        }
        let piglin = &t.models["piglin"];
        assert!(
            piglin.cubes.iter().all(|c| c.part != "head/hat"),
            "no piglin hat"
        );
        let villager = &t.models["villager"];
        assert_eq!(
            (
                cube(villager, "body/jacket").u,
                cube(villager, "body/jacket").v
            ),
            (0, 38)
        );
        assert_eq!(
            cube(villager, "body/jacket").h,
            20,
            "the robe hangs over the legs"
        );
        assert_eq!(
            t.mannequin.layers,
            vec![
                "cape",
                "jacket",
                "left_sleeve",
                "right_sleeve",
                "left_pants_leg",
                "right_pants_leg",
                "hat"
            ]
        );
    }

    #[test]
    fn the_stranding_sheet_is_refused_twice_over() {
        let (key, outer) =
            model_for_texture("minecraft:entity/zombie/drowned_outer_layer").unwrap();
        assert_eq!(key, "drowned_outer_layer");
        // Drawn to the player's overlay layout: a jacket at 16,32.
        let mut img = blank(outer);
        paint(&mut img, 20, 36, 8, 12);
        let r = judge(key, outer, &img).unwrap_err();
        assert_eq!(r.code, DW_SKIN_UNSAMPLED);
        assert!(r.reason.contains("`player`,"), "{}", r.reason);
        // Sixteen pixels on the top face of the outer hat: sampled, never seen.
        let mut img = blank(outer);
        paint(&mut img, 43, 0, 2, 8);
        let r = judge(key, outer, &img).unwrap_err();
        assert_eq!(r.code, DW_SKIN_UNSEEN, "{}", r.reason);
        // At the base positions, on the front of the body: admitted.
        let mut img = blank(outer);
        paint(&mut img, 20, 20, 8, 12);
        assert_eq!(judge(key, outer, &img), Ok(96));
        // An empty outer layer replaces the layer with nothing, and is a choice.
        assert_eq!(judge(key, outer, &blank(outer)), Ok(0));
    }

    #[test]
    fn a_skin_is_judged_on_the_model_it_is_worn_on() {
        let (wide_key, wide) = model_for_skin(SkinModel::Wide);
        let (slim_key, slim) = model_for_skin(SkinModel::Slim);
        // The last column of the wide right arm's back face (52..56): a 3-wide
        // slim arm's back face ends at 54.
        let mut img = blank(wide);
        paint(&mut img, 55, 20, 1, 12);
        paint(&mut img, 8, 8, 8, 8);
        assert!(judge(wide_key, wide, &img).is_ok());
        let r = judge(slim_key, slim, &img).unwrap_err();
        assert_eq!(r.code, DW_SKIN_UNSAMPLED);
        // A head-unwrap corner (0..8, 0..8) is no face of either model.
        let mut img = blank(wide);
        paint(&mut img, 8, 8, 8, 8);
        img.put_pixel(0, 0, image::Rgba([1, 2, 3, 255]));
        let r = judge(wide_key, wide, &img).unwrap_err();
        assert_eq!(r.code, DW_SKIN_UNSAMPLED);
        assert!(r.reason.contains("(0, 0)"), "{}", r.reason);
    }

    #[test]
    fn a_sheet_at_twice_the_resolution_is_judged_at_its_own_scale() {
        let (key, zombie) = model_for_texture("minecraft:entity/zombie/zombie").unwrap();
        let mut img = image::RgbaImage::new(128, 128);
        // The head front at 2×: (8..16, 8..16) → (16..32, 16..32).
        paint(&mut img, 16, 16, 16, 16);
        assert_eq!(judge(key, zombie, &img), Ok(256));
        // The zombie mirrors its left limbs, so 32,48 is never sampled.
        paint(&mut img, 72, 104, 2, 2);
        assert_eq!(
            judge(key, zombie, &img).unwrap_err().code,
            DW_SKIN_UNSAMPLED
        );
    }

    #[test]
    fn limbs_and_posed_boxes_are_never_unseen() {
        let (key, player) = model_for_skin(SkinModel::Wide);
        // The soles of both feet: the down faces of the legs.
        let mut img = blank(player);
        paint(&mut img, 8, 16, 4, 4);
        assert_eq!(judge(key, player, &img), Ok(16));
        // The underside of the head: unseen at eye level.
        let mut img = blank(player);
        paint(&mut img, 16, 0, 8, 8);
        assert_eq!(judge(key, player, &img).unwrap_err().code, DW_SKIN_UNSEEN);
        let villager = &table().models["villager"];
        let rim = cube(villager, "head/hat/hat_rim");
        assert!(rim.posed, "the rim is rotated");
        assert!(!rim.face_unseen("up", table().player_eye_height_px));
    }
}
