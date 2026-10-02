//! **A delve wears its own textures** (spec-0084): a `world.textures[]` row
//! replaces one vanilla texture through the resource pack the delve already
//! ships.
//!
//! A row is resolved here against two things: the **census** of the pinned
//! client — every `assets/minecraft/textures/**.png` it ships, with its size,
//! its frame size where vanilla animates it, and the sha256 of vanilla's bytes,
//! vendored at `crates/delvec/data/textures-1.21.11.json` by
//! `tools/maintenance/derive-client-textures.py` so a build never reaches for a
//! jar — and the campaign's own file at `textures/<id>.png`. One function,
//! [`resolve`], is both the validation-tier refusal (`delvec validate` reports
//! every row's findings at once) and the build's source of truth for what the
//! pack carries, so the rule a row is judged by and the rule its bytes are
//! shipped by cannot drift.
//!
//! The refusals:
//!
//! - `DW0939` — `replaces` names no texture in the census (§6.1). The census is
//!   keyed by the resource location a campaign writes, so a `textures/` or
//!   `.png` left on, another namespace, another version's path and a
//!   misspelling are all one refusal, and the message names the nearest
//!   census paths.
//! - `DW0940` — the file is not an image that texture can be replaced by
//!   (§6.2): not a PNG; not `k·w₀ × k·h₀` of vanilla's frame for one integer
//!   `k`, or `k·w₀ × n·k·h₀` with an animation sidecar; a sidecar that is not
//!   vanilla's animation metadata; or bytes equal to vanilla's, which replace
//!   nothing.
//! - [`DW_IMAGE_MISSING`] (`DW0309`) — the row's `textures/<id>.png` is absent.
//!
//! The id and licence half of a row is judged in the DSL, where no file is
//! needed (`DW0190`, `DW0741`, and a second row replacing one texture,
//! `DW0939`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, codes};
use serde::Deserialize;
use serde_json::Value;

/// `DW0309`: an image a campaign declares has no file — a staged body's
/// `skin.texture_id` with no `skins/<id>.png`, or a `world.textures[]` row with
/// no `textures/<id>.png`. The message names the declaring object and the path.
/// Build-tier (exit 3) where a skin is baked; validation-tier where a texture
/// row is resolved.
pub const DW_IMAGE_MISSING: DwCode = DwCode::new("DW0309", ExitTier::Build);

/// The campaign-relative directory a row's image is read from.
pub const TEXTURES_DIR: &str = "textures";

const CENSUS_JSON: &str = include_str!("../../data/textures-1.21.11.json");

/// One texture the pinned client ships.
#[derive(Debug, Clone, Deserialize)]
pub struct CensusEntry {
    pub w: u32,
    pub h: u32,
    pub sha256: String,
    /// Vanilla ships a `.png.mcmeta` beside it.
    pub mcmeta: bool,
    /// That sidecar carries an `animation` object.
    #[serde(default)]
    pub animated: bool,
    /// One frame's width, for an animated texture.
    #[serde(default)]
    pub fw: Option<u32>,
    /// One frame's height, for an animated texture.
    #[serde(default)]
    pub fh: Option<u32>,
}

impl CensusEntry {
    /// The unit an override is scaled from: one frame where vanilla animates
    /// the texture, the whole image otherwise.
    pub fn frame(&self) -> (u32, u32) {
        match (self.animated, self.fw, self.fh) {
            (true, Some(w), Some(h)) => (w, h),
            _ => (self.w, self.h),
        }
    }
}

/// The census of the pinned client's textures.
#[derive(Debug, Deserialize)]
pub struct Census {
    /// The sha256 of the client jar the census was derived from.
    pub client_jar_sha256: String,
    /// The Minecraft version it describes.
    pub minecraft: String,
    /// Resource location (`minecraft:<path>`) → entry.
    pub textures: BTreeMap<String, CensusEntry>,
}

/// The vendored census, parsed once.
pub fn census() -> &'static Census {
    static CENSUS: OnceLock<Census> = OnceLock::new();
    CENSUS.get_or_init(|| {
        serde_json::from_str(CENSUS_JSON).expect("the vendored texture census parses")
    })
}

/// One row, resolved: what the pack carries for it and what the note says.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The authored id.
    pub id: String,
    /// The resource location, as authored.
    pub replaces: String,
    /// The vanilla path under `textures/`, without `.png`.
    pub path: String,
    /// The creator's PNG bytes, shipped verbatim.
    pub png: Vec<u8>,
    /// The creator's animation sidecar, shipped verbatim.
    pub mcmeta: Option<Vec<u8>>,
    /// The scale against vanilla's frame.
    pub k: u32,
    /// The frame count, where the row ships a sidecar.
    pub frames: Option<u32>,
    /// Vanilla animates this texture and the row ships no sidecar: it is
    /// replaced by a still image.
    pub still: bool,
    /// The census entry it was judged against.
    pub vanilla: CensusEntry,
    /// The licence as recorded.
    pub license: delvewright_dsl::license::LicenseEvidence,
}

impl Resolved {
    /// The archive path of the PNG in the resource pack.
    pub fn pack_path(&self) -> String {
        format!("assets/minecraft/textures/{}.png", self.path)
    }

    /// The block textures are the only class a review tool draws (spec-0084 §2.6).
    pub fn is_block(&self) -> bool {
        self.path.starts_with("block/")
    }
}

/// One refusal of one row: the code, the JSON pointer into `world.json`, and the
/// message. Kept as the code itself rather than a [`Diagnostic`] so the build
/// can refuse with it (`BuildFailure::Diagnostic` carries a [`DwCode`]).
#[derive(Debug, Clone)]
pub struct Finding {
    pub code: DwCode,
    pub path: String,
    pub message: String,
}

impl Finding {
    fn new(code: DwCode, path: String, message: String) -> Self {
        Finding {
            code,
            path,
            message,
        }
    }

    /// The validation-tier diagnostic for this finding.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.code, "world", self.path.clone(), self.message.clone())
    }
}

/// The file a row's image is read from, campaign-relative.
pub fn image_file(id: &str) -> String {
    format!("{TEXTURES_DIR}/{id}.png")
}

/// The file a row's animation sidecar is read from, campaign-relative.
pub fn sidecar_file(id: &str) -> String {
    format!("{TEXTURES_DIR}/{id}.png.mcmeta")
}

/// Resolve every `world.textures[]` row against the census and the campaign's
/// files. `file` answers a campaign-relative path (`textures/<id>.png`) with its
/// bytes, or `None` when the campaign holds no such file.
///
/// Returns the rows that resolved and one diagnostic per finding, every row
/// examined: a creator fixing three rows learns about all three at once.
pub fn resolve<'a>(
    campaign: &Campaign,
    file: impl Fn(&str) -> Option<&'a [u8]>,
) -> (Vec<Resolved>, Vec<Finding>) {
    let census = census();
    let mut out = Vec::new();
    let mut diags = Vec::new();
    for (i, row) in campaign.world.content.textures.iter().enumerate() {
        let pointer = format!("/content/textures/{i}");
        let Some(entry) = census.textures.get(&row.replaces) else {
            diags.push(Finding::new(
                codes::TEXTURE_PATH,
                format!("{pointer}/replaces"),
                path_message(&row.id, &row.replaces, census),
            ));
            continue;
        };
        let path = row
            .replaces
            .strip_prefix("minecraft:")
            .expect("every census key carries the minecraft namespace")
            .to_string();
        let image = image_file(&row.id);
        let Some(png) = file(&image) else {
            diags.push(Finding::new(
                DW_IMAGE_MISSING,
                format!("{pointer}/id"),
                format!(
                    "`world.textures[{i}]` (`{}`, replaces `{}`) has no file — the campaign \
                     directory holds no `{image}`. The row's image is what the resource pack \
                     ships at `assets/minecraft/textures/{path}.png`; add the PNG at that path, \
                     or remove the row",
                    row.id, row.replaces
                ),
            ));
            continue;
        };
        let mcmeta = file(&sidecar_file(&row.id)).map(<[u8]>::to_vec);
        match judge(&row.id, &row.replaces, entry, png, mcmeta.as_deref()) {
            Ok((k, frames)) => out.push(Resolved {
                id: row.id.clone(),
                replaces: row.replaces.clone(),
                path,
                png: png.to_vec(),
                still: entry.animated && mcmeta.is_none(),
                mcmeta,
                k,
                frames,
                vanilla: entry.clone(),
                license: row.license.clone(),
            }),
            Err(reason) => diags.push(Finding::new(
                codes::TEXTURE_IMAGE,
                format!("{pointer}/id"),
                format!(
                    "`world.textures[{i}]` (`{}`, replaces `{}`): `{image}` {reason}",
                    row.id, row.replaces
                ),
            )),
        }
    }
    (out, diags)
}

/// The `DW0939` message: what was asked for, why it is not in the census, and
/// the nearest paths the census holds.
fn path_message(id: &str, replaces: &str, census: &Census) -> String {
    let mut why = String::new();
    if !replaces.starts_with("minecraft:") {
        why.push_str(
            " A replaced texture is vanilla's, so it is named in the `minecraft` namespace \
             (`minecraft:<path>`).",
        );
    }
    let bare = replaces.strip_prefix("minecraft:").unwrap_or(replaces);
    if bare.starts_with("textures/") || bare.ends_with(".png") {
        why.push_str(
            " Name it as vanilla's models and atlases do — without `textures/` and without \
             `.png`.",
        );
    }
    let near = nearest(bare, census);
    let shown = if near.is_empty() {
        String::new()
    } else {
        format!(" The census holds, nearest by prefix: {}.", near.join(", "))
    };
    format!(
        "texture `{id}` replaces `{replaces}`, which the pinned client {} does not ship, so \
         an override written there would bind nothing.{why}{shown} Name a texture from the \
         census (`crates/delvec/data/textures-{}.json`), or remove the row",
        census.minecraft, census.minecraft
    )
}

/// Up to six census paths sharing the longest leading run of `/`-segments with
/// `asked` (after any `textures/` and `.png` are stripped, so a mis-spelled
/// prefix still finds its neighbours).
fn nearest(asked: &str, census: &Census) -> Vec<String> {
    let asked = asked
        .trim_start_matches("textures/")
        .trim_end_matches(".png");
    let segs: Vec<&str> = asked.split('/').collect();
    let shared = |key: &str| -> usize {
        let p = key.strip_prefix("minecraft:").unwrap_or(key);
        let mut n = 0;
        for (a, b) in p.split('/').zip(segs.iter()) {
            if a == *b {
                n += 1;
            } else {
                // A partly matching last segment still counts for order.
                if a.starts_with(&b[..b.len().min(3)]) && !b.is_empty() {
                    return n * 2 + 1;
                }
                break;
            }
        }
        n * 2
    };
    let best = census.textures.keys().map(|k| shared(k)).max().unwrap_or(0);
    if best == 0 {
        return Vec::new();
    }
    census
        .textures
        .keys()
        .filter(|k| shared(k) == best)
        .take(6)
        .map(|k| format!("`{k}`"))
        .collect()
}

/// Judge one file against its census entry: `Ok((k, frames))`, or the reason it
/// is refused, phrased to follow the file's name.
fn judge(
    id: &str,
    replaces: &str,
    entry: &CensusEntry,
    png: &[u8],
    mcmeta: Option<&[u8]>,
) -> Result<(u32, Option<u32>), String> {
    let _ = (id, replaces);
    if png.len() < 8 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("is not a PNG (its header does not read) — save the image as a PNG".into());
    }
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|e| format!("does not decode as a PNG ({e}) — re-save the image as a PNG"))?;
    let (w, h) = (img.width(), img.height());
    if sha256_hex(png) == entry.sha256 {
        return Err(
            "is byte-for-byte vanilla's own texture, so the override replaces nothing — draw \
             the image this delve wants there, or remove the row"
                .into(),
        );
    }
    let (w0, h0) = entry.frame();
    let unit = if entry.animated {
        format!(
            "{w0}×{h0} (one frame of vanilla's {}×{} animation)",
            entry.w, entry.h
        )
    } else {
        format!("{w0}×{h0}")
    };
    if w == 0 || w % w0 != 0 {
        return Err(format!(
            "is {w}×{h}, and vanilla's texture there is {unit}: the width must be a whole \
             multiple k of {w0} (k = 1 is vanilla's own resolution) and the height k × {h0}, \
             because a texture drawn at another proportion draws as noise — resize the image"
        ));
    }
    let k = w / w0;
    match mcmeta {
        None => {
            if h != k * h0 {
                return Err(format!(
                    "is {w}×{h}, and vanilla's texture there is {unit}: at {k}× its width the \
                     height must be {} — resize the image, or ship a \
                     `textures/{id}.png.mcmeta` if it is an animation strip of {w}×{} frames",
                    k * h0,
                    k * h0
                ));
            }
            Ok((k, None))
        }
        Some(raw) => {
            let frame_h = k * h0;
            if h == 0 || h % frame_h != 0 {
                return Err(format!(
                    "is {w}×{h} with an animation sidecar, and vanilla's texture there is \
                     {unit}: at {k}× its width every frame is {w}×{frame_h}, and {h} is not a \
                     whole number of frames — make the strip a whole number of frames tall"
                ));
            }
            let n = h / frame_h;
            check_sidecar(raw, n)
                .map_err(|why| format!("has a sidecar `textures/{id}.png.mcmeta` that {why}"))?;
            Ok((k, Some(n)))
        }
    }
}

/// Whether a sidecar is vanilla's animation metadata over `n` frames.
fn check_sidecar(raw: &[u8], n: u32) -> Result<(), String> {
    let v: Value = serde_json::from_slice(raw)
        .map_err(|e| format!("is not JSON ({e}) — write it as vanilla's animation metadata"))?;
    let Some(obj) = v.as_object() else {
        return Err("is not a JSON object — write `{\"animation\": {…}}`".into());
    };
    if let Some(other) = obj.keys().find(|k| k.as_str() != "animation") {
        return Err(format!(
            "carries `{other}`, and a texture's sidecar here may carry only its `animation` \
             — remove `{other}`"
        ));
    }
    let Some(anim) = obj.get("animation").and_then(Value::as_object) else {
        return Err("has no `animation` object — write `{\"animation\": {…}}`".into());
    };
    for (key, val) in anim {
        match key.as_str() {
            "frametime" => {
                if val.as_u64().is_none_or(|t| t == 0) {
                    return Err("has a `frametime` that is not a positive whole number of \
                                ticks — set it to one"
                        .into());
                }
            }
            "interpolate" => {
                if !val.is_boolean() {
                    return Err("has an `interpolate` that is not `true` or `false`".into());
                }
            }
            "frames" => {
                let Some(list) = val.as_array() else {
                    return Err("has a `frames` that is not a list".into());
                };
                for f in list {
                    let index = match f {
                        Value::Number(_) => f.as_u64(),
                        Value::Object(o) => {
                            if let Some(bad) = o
                                .keys()
                                .find(|k| k.as_str() != "index" && k.as_str() != "time")
                            {
                                return Err(format!(
                                    "has a frame carrying `{bad}` — a frame is an index, or \
                                     `{{\"index\": i, \"time\": t}}`"
                                ));
                            }
                            if o.get("time")
                                .is_some_and(|t| t.as_u64().is_none_or(|t| t == 0))
                            {
                                return Err("has a frame whose `time` is not a positive whole \
                                            number of ticks"
                                    .into());
                            }
                            o.get("index").and_then(Value::as_u64)
                        }
                        _ => None,
                    };
                    match index {
                        Some(i) if i < u64::from(n) => {}
                        Some(i) => {
                            return Err(format!(
                                "names frame {i}, and the strip holds {n} frame(s) (0..{})",
                                n - 1
                            ));
                        }
                        None => {
                            return Err("has a frame that is neither a whole-number index nor \
                                 `{\"index\": i, \"time\": t}`"
                                .into());
                        }
                    }
                }
            }
            "width" | "height" => {
                return Err(format!(
                    "states a frame `{key}` — an override's frame is vanilla's frame at the \
                     image's scale, read off the file; remove `{key}`"
                ));
            }
            other => {
                return Err(format!(
                    "carries `animation.{other}`, which is not vanilla's animation metadata \
                     (`frametime`, `interpolate`, `frames`) — remove it"
                ));
            }
        }
    }
    Ok(())
}

/// The resource-pack entries the resolved rows contribute: the PNG at the
/// vanilla path, and the sidecar beside it where the row ships one. Bytes are
/// the creator's, copied, never re-encoded.
pub fn pack_entries(rows: &[Resolved]) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for r in rows {
        out.insert(r.pack_path(), r.png.clone());
        if let Some(m) = &r.mcmeta {
            out.insert(format!("{}.mcmeta", r.pack_path()), m.clone());
        }
    }
    out
}

/// Whether a resource pack's entries replace anything vanilla ships — the fact
/// `manifest.json`'s `resource_pack_overrides_vanilla` states, read off the
/// archive paths the pack is built from.
pub fn overrides_vanilla<'a>(paths: impl IntoIterator<Item = &'a String>) -> bool {
    paths
        .into_iter()
        .any(|p| p.starts_with("assets/minecraft/"))
}

/// The review path of a row's comparison sheet (spec-0084 §5.2).
pub fn sheet_path(id: &str) -> String {
    format!("review/textures/{id}.png")
}

/// The width each half of a sheet is scaled to.
const SHEET_HALF: u32 = 256;

/// The comparison sheet for one row (spec-0084 §5.2): vanilla's texture on the
/// left and the override on the right, each scaled by nearest neighbour to 256
/// pixels wide, a one-pixel separator between, on a chequered ground that shows
/// alpha. Both halves show one frame — the first — of an animation. Deterministic:
/// a pure function of the two images, encoded through
/// [`crate::compiler::png::encode_rgba`].
pub fn sheet(vanilla: &[u8], replacement: &[u8], frame: (u32, u32), k: u32) -> Option<Vec<u8>> {
    let a = image::load_from_memory_with_format(vanilla, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let b = image::load_from_memory_with_format(replacement, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let (fw, fh) = frame;
    let half = |img: &image::RgbaImage, sw: u32, sh: u32| -> (u32, Vec<[u8; 4]>) {
        let out_h = (u64::from(SHEET_HALF) * u64::from(sh) / u64::from(sw.max(1))).max(1) as u32;
        let mut px = Vec::with_capacity((SHEET_HALF * out_h) as usize);
        for y in 0..out_h {
            for x in 0..SHEET_HALF {
                let sx = (u64::from(x) * u64::from(sw) / u64::from(SHEET_HALF)) as u32;
                let sy = (u64::from(y) * u64::from(sh) / u64::from(out_h)) as u32;
                let p = img
                    .get_pixel(sx.min(img.width() - 1), sy.min(img.height() - 1))
                    .0;
                px.push(p);
            }
        }
        (out_h, px)
    };
    let (ha, pa) = half(&a, fw, fh);
    let (hb, pb) = half(&b, fw * k, fh * k);
    let height = ha.max(hb);
    let width = SHEET_HALF * 2 + 1;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            // The chequer: 8-pixel cells of two greys.
            let g: u8 = if ((x / 8) + (y / 8)) % 2 == 0 {
                200
            } else {
                150
            };
            let mut c = [g, g, g, 255u8];
            let src = if x < SHEET_HALF {
                (y < ha).then(|| pa[(y * SHEET_HALF + x) as usize])
            } else if x == SHEET_HALF {
                c = [0, 0, 0, 255];
                None
            } else {
                let bx = x - SHEET_HALF - 1;
                (y < hb).then(|| pb[(y * SHEET_HALF + bx) as usize])
            };
            if let Some(p) = src {
                let al = u32::from(p[3]);
                for ch in 0..3 {
                    c[ch] = ((u32::from(p[ch]) * al + u32::from(c[ch]) * (255 - al)) / 255) as u8;
                }
            }
            let o = ((y * width + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&c);
        }
    }
    Some(crate::compiler::png::encode_rgba(width, height, &rgba))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
        crate::compiler::png::encode_rgba(w, h, &px)
    }

    fn entry(w: u32, h: u32) -> CensusEntry {
        CensusEntry {
            w,
            h,
            sha256: String::new(),
            mcmeta: false,
            animated: false,
            fw: None,
            fh: None,
        }
    }

    #[test]
    fn the_census_is_the_pinned_clients() {
        let c = census();
        assert_eq!(c.minecraft, "1.21.11");
        assert_eq!(c.textures.len(), 3517, "the rig's texture_census.png");
        let moon = &c.textures["minecraft:environment/celestial/moon/full_moon"];
        assert_eq!((moon.w, moon.h), (32, 32));
        let drowned = &c.textures["minecraft:entity/zombie/drowned"];
        assert_eq!((drowned.w, drowned.h), (64, 64));
        assert!(!c.textures.contains_key("minecraft:environment/moon_phases"));
        let water = &c.textures["minecraft:block/water_still"];
        assert!(water.animated);
        assert_eq!(water.frame(), (16, 16));
    }

    #[test]
    fn a_scaled_image_is_admitted_and_another_proportion_is_not() {
        let e = entry(32, 32);
        assert_eq!(
            judge("m", "x", &e, &png(32, 32, [255, 0, 0, 255]), None),
            Ok((1, None))
        );
        assert_eq!(
            judge("m", "x", &e, &png(64, 64, [255, 0, 0, 255]), None),
            Ok((2, None))
        );
        assert!(judge("m", "x", &e, &png(48, 48, [255, 0, 0, 255]), None).is_err());
        assert!(judge("m", "x", &e, &png(32, 64, [255, 0, 0, 255]), None).is_err());
        assert!(judge("m", "x", &e, b"not a png at all", None).is_err());
    }

    #[test]
    fn a_strip_needs_a_whole_number_of_frames_and_a_sidecar_vanilla_reads() {
        let e = entry(16, 16);
        let strip = png(16, 48, [0, 0, 255, 255]);
        let ok = br#"{"animation":{"frametime":4,"frames":[0,{"index":2,"time":8},1]}}"#;
        assert_eq!(judge("w", "x", &e, &strip, Some(ok)), Ok((1, Some(3))));
        let bad_index = br#"{"animation":{"frames":[3]}}"#;
        assert!(judge("w", "x", &e, &strip, Some(bad_index)).is_err());
        let extra = br#"{"animation":{},"texture":{"blur":true}}"#;
        assert!(judge("w", "x", &e, &strip, Some(extra)).is_err());
        assert!(judge("w", "x", &e, &png(16, 40, [0, 0, 255, 255]), Some(ok)).is_err());
        assert!(judge("w", "x", &e, &strip, Some(b"{")).is_err());
    }

    #[test]
    fn vanillas_own_bytes_replace_nothing() {
        let bytes = png(16, 16, [1, 2, 3, 255]);
        let mut e = entry(16, 16);
        e.sha256 = sha256_hex(&bytes);
        let err = judge("v", "x", &e, &bytes, None).unwrap_err();
        assert!(err.contains("replaces nothing"), "{err}");
    }

    #[test]
    fn a_sheet_is_a_pure_function_of_its_two_images() {
        let a = png(16, 16, [10, 20, 30, 255]);
        let b = png(32, 32, [200, 0, 0, 128]);
        let s1 = sheet(&a, &b, (16, 16), 2).unwrap();
        let s2 = sheet(&a, &b, (16, 16), 2).unwrap();
        assert_eq!(s1, s2);
        let img = image::load_from_memory(&s1).unwrap();
        assert_eq!((img.width(), img.height()), (513, 256));
    }

    #[test]
    fn a_path_off_the_census_names_its_neighbours() {
        let m = path_message("red-moon", "minecraft:environment/moon_phases", census());
        assert!(m.contains("environment/celestial"), "{m}");
        let m = path_message("x", "minecraft:textures/block/stone.png", census());
        assert!(m.contains("without `textures/`"), "{m}");
        assert!(m.contains("minecraft:block/stone"), "{m}");
    }
}
