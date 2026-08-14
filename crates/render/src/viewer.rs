//! One or more prefabs → a self-contained interactive HTML page.
//!
//! A still render answers "is the set pretty". Only a camera the reviewer drives
//! answers "what is it like to stand in here" — where the way in is, which face
//! the party walks on, what the interior reads as from eye height. This emits
//! that: the voxels, the pinned version's own block models and textures, the
//! declared anchors, and a renderer, in one file with **no external references
//! of any kind**.
//!
//! # What draws the blocks
//!
//! deepslate (MIT), vendored as `viewer/deepslate.bundle.js`. It reads the same
//! `blockstates` → `models` → `textures` chain the game does, so a wall is a
//! wall, a stair is a stair and a chest is a chest. [`resources`] extracts that
//! chain from the pinned client jar; this module packs it beside the geometry.
//!
//! # Packing
//!
//! A zone-sized box is 40×10×100 = 40,000 cells and a page must stay far below
//! 16 MB, so geometry is never JSON. The grid is run-length encoded over
//! `y → z → x` as `(palette index u16, run length u16)` pairs and base64'd, which
//! costs 4 bytes per *run* rather than per cell — a prefab is mostly long runs of
//! air and long runs of wall, so the encoded size tracks how complicated the
//! building is, not how big its box is.
//!
//! The page rebuilds the structure from that grid through the renderer's own
//! public `addBlock`, so a zone that ships as several tiles and one manifest is
//! one building on the page: reassembly happens here, in [`ViewerModel::load`],
//! and nothing downstream knows a tile existed.
//!
//! # Determinism
//!
//! Same inputs → byte-identical page (ADR-0006). Every map is a `BTreeMap`, the
//! encoder is a pure function of the parsed structure, and nothing reads the
//! clock, the environment, or an absolute path.

pub mod resources;

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::assets::Assets;
use crate::blockcolor::is_air;
use crate::meta::PrefabMeta;
use crate::nbt::Structure;
use resources::{Flags, PageResources, Texture, UnderSpecified, Unresolved};

/// Schema id of the model block embedded in the page.
///
/// `/2` carries resources — blockstate definitions, models, textures — where
/// `/1` carried one mean colour and one bounding box per state. Nothing about
/// the two payloads is compatible, so the id says so.
pub const SCHEMA: &str = "delvewright.prefab-viewer/2";

/// A player's eye height above the floor of the cell they stand in, in blocks.
/// A "player POV" preset that floats is not a player POV.
pub const EYE_HEIGHT: f32 = 1.62;

/// Anchor name stems that mean "the party's way in", in precedence order. The
/// first one a prefab declares becomes the page's default point of view, because
/// the first question a reviewer asks is what the place looks like on arrival.
pub const WAY_IN_STEMS: [&str; 4] = ["spawn", "entry", "entrance", "threshold"];

/// The vendored renderer, and the two texture ids it is patched to ask for.
///
/// deepslate 0.26.0 asks for `entity/banner/banner_base` and
/// `entity/shield/shield_base_nopattern`, paths no Minecraft version has ever
/// shipped; 1.21.11 has both at the jar's top level. `tools/build-deepslate-
/// bundle.sh` rewrites the two ids when it vendors the bundle. The check below
/// is bound to page emission rather than to a test, because a bundle that lost
/// the patch renders every banner and every shield as the missing-texture
/// checker and says nothing.
const BUNDLE: &str = include_str!("viewer/deepslate.bundle.js");
const PATCHED_TEXTURE_IDS: [&str; 2] = ["entity/banner_base", "entity/shield_base_nopattern"];
const UNPATCHED_TEXTURE_IDS: [&str; 2] = [
    "entity/banner/banner_base",
    "entity/shield/shield_base_nopattern",
];

/// One block's palette entry as the page carries it: what to place, not what
/// colour to paint.
#[derive(Debug, Clone, Serialize)]
struct PaletteEntry {
    /// Block id, `minecraft:oak_stairs`.
    name: String,
    /// The properties the palette entry writes, and only those. What the game
    /// would fill in is in [`PageData::defaults`], where the renderer reads it.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    props: BTreeMap<String, String>,
    /// How many cells of this model carry this state.
    count: u32,
    /// The blockstate string, for the legend and the findings list.
    state: String,
}

/// An anchor as the page draws it: either a point with a facing, or a region.
#[derive(Debug, Clone, Serialize)]
struct AnchorOut {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pos: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    facing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<[i32; 3]>,
    /// True for a jigsaw socket rather than a declared anchor. A socket's facing
    /// points *out* of the piece, so its point of view looks the other way.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    socket: bool,
}

/// One prefab in the page.
#[derive(Debug, Clone, Serialize)]
struct ModelOut {
    id: String,
    size: [i32; 3],
    /// Index 0 is always air; the rest are [`PaletteEntry`]s.
    palette: Vec<Option<PaletteEntry>>,
    /// Base64 of the run-length encoded grid.
    voxels: String,
    /// Run count, so the page can state what it decoded.
    runs: u32,
    /// Cells that are not air.
    filled: u32,
    /// How many structure templates this model was reassembled from. `1` for an
    /// ordinary prefab; more for a zone past the 48-per-axis template cap, which
    /// the page shows as one building.
    tiles: usize,
    anchors: Vec<AnchorOut>,
}

/// The whole page payload.
#[derive(Debug, Clone, Serialize)]
struct PageData {
    schema: &'static str,
    /// The version the resources were read from.
    mc_version: &'static str,
    /// `eye_height` travels with the data so the page and this crate cannot
    /// disagree about what a player POV is.
    eye_height: f32,
    way_in_stems: Vec<String>,
    models: Vec<ModelOut>,
    /// `minecraft:stone` → its blockstate definition.
    blockstates: BTreeMap<String, serde_json::Value>,
    /// `minecraft:block/stone` → its model, parents included.
    block_models: BTreeMap<String, serde_json::Value>,
    /// `minecraft:block/stone` → the `.png`.
    textures: BTreeMap<String, Texture>,
    /// Per-block render flags.
    flags: BTreeMap<String, Flags>,
    /// Per-block default state: what the renderer fills an unwritten property
    /// with, from the pinned registry rather than from a guess.
    defaults: BTreeMap<String, BTreeMap<String, String>>,
    /// Blockstates the page cannot draw as the game draws them.
    unresolved: Vec<Unresolved>,
    /// Palette entries that leave properties unwritten.
    under_specified: Vec<UnderSpecified>,
    /// How many block-entity texture ids the emitter resolved against the jar.
    /// Stated on the page: a check that examined nothing is not a pass.
    special_bound: usize,
}

/// Everything one prefab contributes to a page.
pub struct ViewerModel {
    id: String,
    structure: Structure,
    meta: Option<PrefabMeta>,
    tiles: usize,
}

impl ViewerModel {
    /// Read a prefab `.nbt` — or a tile-set manifest `.json` — and the metadata
    /// sidecar beside it, which is where anchors live. A missing sidecar costs
    /// anchors and nothing else.
    ///
    /// A zone past the structure-template cap ships as several `.nbt` files and
    /// one manifest. It is reassembled here, so the page shows the building and
    /// never a packaging boundary; and a lone tile of such a set is refused,
    /// because a page of a building sliced at an arbitrary plane is a review
    /// that passes and means nothing.
    pub fn load(input: &Path) -> Result<ViewerModel, String> {
        let (piece, meta_path) = crate::tileset::load_piece(input).map_err(|e| e.to_string())?;
        let tiles = match &piece {
            crate::tileset::PieceInput::Single(_) => 1,
            crate::tileset::PieceInput::Zone { tiles, .. } => *tiles,
        };
        let structure = piece.structure().clone();
        let meta = PrefabMeta::at_path(&meta_path)?;
        let id = input
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "prefab".to_string());
        Ok(ViewerModel {
            id,
            structure,
            meta,
            tiles,
        })
    }

    /// The prefab id (its file stem) — what the page's switcher shows.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// How many anchors and sockets this prefab declares. A page whose models all
    /// report zero is a finding worth printing, not a silent success.
    pub fn anchor_count(&self) -> usize {
        self.meta
            .as_ref()
            .map_or(0, |m| m.anchors.len() + m.connectors.len())
    }

    /// How many structure templates this model was reassembled from.
    pub fn tiles(&self) -> usize {
        self.tiles
    }
}

/// What a built page measured, for the caller's diagnostics.
#[derive(Debug, Clone, Default)]
pub struct BuildStats {
    /// Bytes of the emitted page.
    pub bytes: usize,
    /// Anchors plus sockets over every model.
    pub anchors: usize,
    /// Blockstates the page cannot draw faithfully.
    pub unresolved: Vec<Unresolved>,
    /// Palette entries that leave properties unwritten.
    pub under_specified: Vec<UnderSpecified>,
    /// Block-entity texture ids resolved against the jar — a binding count.
    pub special_bound: usize,
    /// Distinct blockstates on the page, air excluded. The binding count of
    /// every check above: zero means nothing was examined.
    pub states: usize,
    /// Distinct textures the page carries.
    pub textures: usize,
}

/// The page could not be built.
#[derive(Debug)]
pub enum BuildError {
    /// Something about the input.
    Input(String),
    /// The vendored renderer is not the one this crate was built against — the
    /// local texture-id patch is missing. Not an input error: the toolchain is
    /// wrong, and every banner and shield on every page it emits would be the
    /// missing-texture checker with nothing said.
    Bundle(String),
    /// A block-entity texture id the emitter asks for does not exist **in the
    /// pinned game**, which the asset source declared itself to be. The pinned
    /// jar is complete by definition, so this is the emitter's table and that
    /// version disagreeing, and no picture built from it can be trusted.
    SpecialTextures(Vec<(String, String)>),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Input(m) | BuildError::Bundle(m) => write!(f, "{m}"),
            BuildError::SpecialTextures(ids) => {
                write!(
                    f,
                    "the block-entity texture table and Minecraft {} disagree: ",
                    delvewright_schem::blocks::MC_VERSION
                )?;
                let body: Vec<String> = ids
                    .iter()
                    .map(|(block, id)| {
                        format!("{block} asks for {id}, which the jar does not have")
                    })
                    .collect();
                write!(f, "{}", body.join("; "))
            }
        }
    }
}

/// Refuse a bundle that does not carry the local texture-id patch.
///
/// Bound to page emission, not to a test: the operator building a page does not
/// run `cargo test`, and the failure this catches is invisible in the output.
fn check_bundle() -> Result<(), BuildError> {
    check_bundle_text(BUNDLE)
}

/// [`check_bundle`] over arbitrary text, so both verdicts are demonstrable.
fn check_bundle_text(bundle: &str) -> Result<(), BuildError> {
    for wrong in UNPATCHED_TEXTURE_IDS {
        if bundle.contains(wrong) {
            return Err(BuildError::Bundle(format!(
                "the vendored renderer still asks for `{wrong}`, a path no Minecraft version \
                 ships — every banner and shield would draw as the missing-texture checker. \
                 Rebuild it with tools/build-deepslate-bundle.sh, which applies the patch and \
                 refuses if upstream has moved the id again."
            )));
        }
    }
    for right in PATCHED_TEXTURE_IDS {
        if !bundle.contains(right) {
            return Err(BuildError::Bundle(format!(
                "the vendored renderer never mentions `{right}`, so it is not the bundle this \
                 crate was built against. Rebuild it with tools/build-deepslate-bundle.sh."
            )));
        }
    }
    Ok(())
}

/// Build the page for one or more prefabs.
pub fn build_page(
    models: &[ViewerModel],
    assets: &Assets,
    title: &str,
) -> Result<(String, BuildStats), BuildError> {
    check_bundle()?;
    if models.is_empty() {
        return Err(BuildError::Input("no prefabs to show".to_string()));
    }
    let mut stats = BuildStats::default();
    let mut out = Vec::with_capacity(models.len());

    // Cell counts per blockstate over the whole page, so a finding says how many
    // blocks a reviewer would be looking at rather than how many palette entries
    // mention it.
    let mut states: BTreeMap<String, u32> = BTreeMap::new();
    for m in models {
        let built = build_model(m, &mut states)?;
        stats.anchors += built.anchors.len();
        out.push(built);
    }
    states.retain(|s, _| !is_air(s));
    stats.states = states.len();

    let PageResources {
        blockstates,
        models: block_models,
        textures,
        flags,
        defaults,
        mut unresolved,
        under_specified,
        special_unresolved,
        special_bound,
    } = resources::extract(assets, &states);

    stats.under_specified = under_specified.clone();
    stats.special_bound = special_bound;
    stats.textures = textures.len();

    // What an absent block-entity texture MEANS depends on what the source is,
    // and the source says which it is. A jar that declares itself to be the
    // pinned game is complete by definition, so a texture it does not have is
    // the emitter's private table disagreeing with the version — an error, and
    // silent otherwise, since the block would simply render magenta. Any other
    // source is a resource pack, which is entitled to be partial, and the same
    // absence is the ordinary unresolved-resource finding.
    if !special_unresolved.is_empty() {
        if assets.declared_version().as_deref() == Some(delvewright_schem::blocks::MC_VERSION) {
            return Err(BuildError::SpecialTextures(special_unresolved));
        }
        for (block, id) in &special_unresolved {
            unresolved.push(Unresolved {
                state: block.clone(),
                reason: resources::Missing::Texture,
                detail: format!(
                    "{id} (a block-entity texture, named by the renderer and by no model file)"
                ),
                cells: *states.get(block).unwrap_or(&0),
            });
        }
        unresolved.sort_by(|a, b| (&a.state, &a.detail).cmp(&(&b.state, &b.detail)));
    }
    stats.unresolved = unresolved.clone();

    let data = PageData {
        schema: SCHEMA,
        mc_version: delvewright_schem::blocks::MC_VERSION,
        eye_height: EYE_HEIGHT,
        way_in_stems: WAY_IN_STEMS.iter().map(|s| s.to_string()).collect(),
        models: out,
        blockstates,
        block_models,
        textures,
        flags,
        defaults,
        unresolved,
        under_specified,
        special_bound,
    };
    let json = serde_json::to_string(&data)
        .map_err(|e| BuildError::Input(format!("serialise page data: {e}")))?;
    let html = render_html(title, &json);
    stats.bytes = html.len();
    Ok((html, stats))
}

/// Build one model's payload: palette, packed grid, anchors.
fn build_model(
    m: &ViewerModel,
    states: &mut BTreeMap<String, u32>,
) -> Result<ModelOut, BuildError> {
    let st = &m.structure;
    let [sx, sy, sz] = st.size;
    if sx <= 0 || sy <= 0 || sz <= 0 {
        return Err(BuildError::Input(format!(
            "{}: structure size {:?} is empty",
            m.id, st.size
        )));
    }
    let (sxu, syu, szu) = (sx as usize, sy as usize, sz as usize);
    let cells = sxu
        .checked_mul(syu)
        .and_then(|v| v.checked_mul(szu))
        .ok_or_else(|| {
            BuildError::Input(format!("{}: structure size {:?} overflows", m.id, st.size))
        })?;

    // Palette index 0 is air. Every other blockstate gets an index in the
    // structure's own palette order, so the mapping is stable per input.
    let mut index_of: Vec<u16> = vec![0; st.palette.len()];
    let mut entries: Vec<Option<PaletteEntry>> = vec![None];
    let mut counts: Vec<u32> = vec![0];

    for (i, state) in st.palette.iter().enumerate() {
        if is_air(state) {
            index_of[i] = 0;
            continue;
        }
        if entries.len() >= u16::MAX as usize {
            return Err(BuildError::Input(format!(
                "{}: more than 65534 distinct blockstates",
                m.id
            )));
        }
        index_of[i] = entries.len() as u16;
        let (ns, id) = crate::blockcolor::base_id(state);
        entries.push(Some(PaletteEntry {
            name: format!("{ns}:{id}"),
            props: resources::state_properties(state),
            count: 0,
            state: state.clone(),
        }));
        counts.push(0);
    }

    // Rasterise into a dense grid in `y → z → x` order.
    let mut grid = vec![0u16; cells];
    let mut filled = 0u32;
    for (pos, pi) in &st.blocks {
        let (x, y, z) = (pos[0], pos[1], pos[2]);
        if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz {
            continue;
        }
        let idx = index_of[*pi];
        if idx == 0 {
            continue;
        }
        let at = (y as usize * szu + z as usize) * sxu + x as usize;
        grid[at] = idx;
        counts[idx as usize] += 1;
        filled += 1;
    }
    for (i, e) in entries.iter_mut().enumerate() {
        if let Some(e) = e {
            e.count = counts[i];
            *states.entry(e.state.clone()).or_insert(0) += counts[i];
        }
    }

    let (packed, runs) = rle_encode(&grid);
    Ok(ModelOut {
        id: m.id.clone(),
        size: st.size,
        palette: entries,
        voxels: base64(&packed),
        runs,
        filled,
        tiles: m.tiles,
        anchors: collect_anchors(m.meta.as_ref()),
    })
}

/// Anchors and jigsaw sockets, in a stable order.
///
/// Degrades to an empty list when the prefab has no sidecar or declares none —
/// the page then simply offers no per-anchor point of view, and says so, rather
/// than inventing one.
fn collect_anchors(meta: Option<&PrefabMeta>) -> Vec<AnchorOut> {
    let Some(meta) = meta else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // `anchors` is a BTreeMap, so this is already name-sorted.
    for (name, a) in &meta.anchors {
        out.push(AnchorOut {
            name: name.clone(),
            pos: a.pos,
            facing: a.facing.clone(),
            from: a.region.as_ref().map(|r| r.from),
            to: a.region.as_ref().map(|r| r.to),
            socket: false,
        });
    }
    for (i, c) in meta.connectors.iter().enumerate() {
        out.push(AnchorOut {
            name: format!("socket-{}", i + 1),
            pos: Some(c.local_pos),
            facing: Some(c.facing.clone()),
            from: None,
            to: None,
            socket: true,
        });
    }
    out
}

/// Run-length encode the grid as `(palette index u16 LE, run length u16 LE)`.
/// Returns the bytes and the run count. Runs longer than `u16::MAX` are split,
/// which keeps every record fixed-width and the decoder branchless.
fn rle_encode(grid: &[u16]) -> (Vec<u8>, u32) {
    let mut out = Vec::new();
    let mut runs = 0u32;
    let mut i = 0usize;
    while i < grid.len() {
        let v = grid[i];
        let mut n = 1usize;
        while i + n < grid.len() && grid[i + n] == v && n < u16::MAX as usize {
            n += 1;
        }
        out.extend_from_slice(&v.to_le_bytes());
        out.extend_from_slice(&(n as u16).to_le_bytes());
        runs += 1;
        i += n;
    }
    (out, runs)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding. Hand-rolled rather than pulling a dependency in
/// for twenty lines that a test pins to RFC 4648's own vectors.
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

/// The page shell. Styles, renderer and script are compiled in, so the emitted
/// file references nothing outside itself — the Artifact CSP blocks every
/// external host, and a viewer that needs a CDN is a viewer the owner cannot
/// open.
fn render_html(title: &str, data_json: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<style>\n{css}</style>\n</head>\n<body>\n\
         {body}\n\
         <script type=\"application/json\" id=\"delve-model\">{data}</script>\n\
         <script>\n{bundle}</script>\n\
         <script>\n{controls}</script>\n\
         <script>\n{js}</script>\n</body>\n</html>\n",
        title = escape_html(title),
        css = include_str!("viewer/page.css"),
        body = include_str!("viewer/page.html"),
        // The payload sits in a `application/json` block, so the only sequence
        // that could break out of it is a literal `</script`.
        data = data_json.replace("</", "<\\/"),
        // deepslate (MIT), vendored and patched — see `check_bundle`.
        bundle = BUNDLE,
        // The control mapping loads before the page and in its own block: it is
        // shared with whatever else drives this viewer, and it depends on
        // nothing the renderer defines. Which key does what is decided in
        // exactly one file.
        controls = include_str!("viewer/controls.js"),
        js = include_str!("viewer/page.js"),
    )
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Derive a palette table covering every blockstate in these models — the
/// `palette` subcommand's payload.
pub fn palette_for(
    models: &[ViewerModel],
    deriver: &crate::blockcolor::Deriver<'_>,
) -> crate::blockcolor::PaletteTable {
    let mut states: Vec<&str> = Vec::new();
    for m in models {
        for s in &m.structure.palette {
            states.push(s.as_str());
        }
    }
    crate::blockcolor::PaletteTable::derive(deriver, states)
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
        // Every byte value round-trips through the alphabet.
        let all: Vec<u8> = (0..=255u8).collect();
        let enc = base64(&all);
        assert_eq!(enc.len(), 344);
        assert!(enc.ends_with('='));
    }

    #[test]
    fn rle_collapses_runs_and_splits_at_the_record_limit() {
        let (bytes, runs) = rle_encode(&[0, 0, 0, 7, 7, 1]);
        assert_eq!(runs, 3);
        assert_eq!(bytes.len(), 12);
        // (0, 3), (7, 2), (1, 1)
        assert_eq!(&bytes[0..4], &[0, 0, 3, 0]);
        assert_eq!(&bytes[4..8], &[7, 0, 2, 0]);
        assert_eq!(&bytes[8..12], &[1, 0, 1, 0]);

        // A run longer than u16::MAX splits into fixed-width records.
        let long = vec![5u16; 70_000];
        let (bytes, runs) = rle_encode(&long);
        assert_eq!(runs, 2);
        assert_eq!(bytes.len(), 8);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), u16::MAX);
        assert_eq!(
            u32::from(u16::from_le_bytes([bytes[6], bytes[7]])),
            70_000u32 - u32::from(u16::MAX)
        );

        assert_eq!(rle_encode(&[]), (Vec::new(), 0));
    }

    /// The packed form must be a faithful encoding of the grid, not merely a
    /// smaller one.
    #[test]
    fn rle_round_trips() {
        let grid: Vec<u16> = (0..5000u16).map(|i| (i / 37) % 11).collect();
        let (bytes, runs) = rle_encode(&grid);
        let mut back = Vec::new();
        for r in bytes.chunks(4) {
            let v = u16::from_le_bytes([r[0], r[1]]);
            let n = u16::from_le_bytes([r[2], r[3]]);
            for _ in 0..n {
                back.push(v);
            }
        }
        assert_eq!(back, grid);
        assert_eq!(runs as usize, bytes.len() / 4);
    }

    #[test]
    fn the_payload_cannot_close_the_script_block() {
        let html = render_html("t", r#"{"a":"</script><img>"}"#);
        assert!(!html.contains("</script><img>"));
        assert!(html.contains(r#"<\/script>"#));
    }

    #[test]
    fn the_title_is_escaped() {
        let html = render_html("<script>alert(1)</script>", "{}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<title><script>"));
    }

    /// The page must FETCH nothing outside itself: the Artifact CSP blocks every
    /// external host, so one external reference is a broken page.
    ///
    /// The property is about what the browser goes and gets, not about whether
    /// the letters `https` occur anywhere — the vendored renderer's bundled
    /// licence notices name their projects' repositories, and a URL inside a
    /// comment fetches nothing. So this looks for the attributes and functions
    /// that actually reach a host.
    #[test]
    fn the_page_fetches_nothing_from_outside_itself() {
        let html = render_html("t", "{}");
        for probe in [
            "src=\"http",
            "src='http",
            "href=\"http",
            "href='http",
            "<script src",
            "<link ",
            "@import",
            "url(http",
            "fetch(",
            "XMLHttpRequest",
            "importScripts",
            "new WebSocket",
        ] {
            assert!(
                !html.contains(probe),
                "page reaches outside itself via {probe}"
            );
        }
        // Every `<script>` and `<style>` is inline: as many opening tags as
        // there are closing ones, and no `src` on any of them.
        assert_eq!(
            html.matches("<script").count(),
            html.matches("</script>").count()
        );
    }

    /// The vendored renderer carries the local texture-id patch, and the check
    /// that says so runs on every page this crate emits.
    ///
    /// Both verdicts are demonstrated: a check that has only ever been seen to
    /// pass is a check nobody has watched fail.
    #[test]
    fn the_vendored_renderer_is_the_patched_one() {
        check_bundle().expect("the vendored bundle is unpatched or absent");
        for wrong in UNPATCHED_TEXTURE_IDS {
            assert!(!BUNDLE.contains(wrong));
        }
        for right in PATCHED_TEXTURE_IDS {
            assert!(BUNDLE.contains(right));
        }

        // Upstream's own ids, which are the paths the patch removes.
        let unpatched = "const t={0:\"entity/banner/banner_base\"},\
                         u={0:\"entity/shield/shield_base_nopattern\"};";
        match check_bundle_text(unpatched) {
            Err(BuildError::Bundle(m)) => assert!(m.contains("entity/banner/banner_base"), "{m}"),
            other => panic!("an unpatched bundle must be refused, got {other:?}"),
        }
        // And a bundle that mentions neither is not this crate's bundle at all.
        match check_bundle_text("var deepslate={};") {
            Err(BuildError::Bundle(m)) => assert!(m.contains("entity/banner_base"), "{m}"),
            other => panic!("a foreign bundle must be refused, got {other:?}"),
        }
    }

    /// deepslate's own `TextureAtlas.fromBlobs` sizes its canvas from
    /// `upperPowerOfTwo(sqrt(n + 1))` and then writes the first texture at index
    /// 1, so at a count whose square root is already a power of two the last
    /// texture is placed one row past the bottom edge and is silently lost. A
    /// jar-scale atlas is squarely in that range. The page packs its own atlas
    /// and hands the renderer a finished image; this pins that it never reaches
    /// for the broken constructor, which is the only way the defect could
    /// return.
    #[test]
    fn the_page_never_uses_the_upstream_atlas_builder() {
        let page = include_str!("viewer/page.js");
        assert!(
            !page.contains("fromBlobs("),
            "the page builds its own atlas; `fromBlobs` drops trailing textures at some counts"
        );
        // The page's own packer states what it placed, so a drop cannot be silent.
        assert!(page.contains("auditPacking("));
        assert!(
            page.contains("new D.TextureAtlas("),
            "the page must hand the renderer a finished atlas"
        );
    }
}
