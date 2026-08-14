//! One or more prefabs → a self-contained interactive HTML page.
//!
//! A still render answers "is the set pretty". Only a camera the reviewer drives
//! answers "what is it like to stand in here" — where the way in is, which face
//! the party walks on, what the interior reads as from eye height. This emits
//! that: the voxels, a derived colour per blockstate, the declared anchors, and a
//! small WebGL renderer, in one file with **no external references of any kind**.
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
//! Meshing happens in the browser, from that grid: a face is emitted only where
//! the neighbouring cell does not hide it, so interior faces — the overwhelming
//! majority — never become triangles.
//!
//! # Determinism
//!
//! Same inputs → byte-identical page (ADR-0006). Every map is a `BTreeMap`, the
//! encoder is a pure function of the parsed structure, and nothing reads the
//! clock, the environment, or an absolute path.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::blockcolor::{Appearance, Appearances, PaletteTable, Unresolved, is_air};
use crate::meta::PrefabMeta;
use crate::nbt::Structure;

/// Schema id of the model block embedded in the page.
pub const SCHEMA: &str = "delvewright.prefab-viewer/1";

/// A player's eye height above the floor of the cell they stand in, in blocks.
/// A "player POV" preset that floats is not a player POV.
pub const EYE_HEIGHT: f32 = 1.62;

/// Anchor name stems that mean "the party's way in", in precedence order. The
/// first one a prefab declares becomes the page's default point of view, because
/// the first question a reviewer asks is what the place looks like on arrival.
pub const WAY_IN_STEMS: [&str; 4] = ["spawn", "entry", "entrance", "threshold"];

/// One block's appearance as the page carries it.
#[derive(Debug, Clone, Serialize)]
struct PaletteEntry {
    /// Blockstate string, for the legend and the hover readout.
    name: String,
    rgb: [u8; 3],
    /// Mean alpha, 0–255.
    cov: u8,
    /// Model bounds in sixteenths: `[x0, y0, z0, x1, y1, z1]`.
    #[serde(rename = "box")]
    shape: [u8; 6],
    /// How many cells of this model carry this state.
    count: u32,
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

/// A blockstate the colour deriver could not resolve, with how many cells it
/// covers. Reported on the page rather than silently drawn grey: a block that
/// cannot be resolved is a finding about the prefab or the pinned version.
#[derive(Debug, Clone, Serialize)]
struct UnresolvedOut {
    reason: Unresolved,
    detail: String,
    count: u32,
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
    anchors: Vec<AnchorOut>,
    unresolved: BTreeMap<String, UnresolvedOut>,
}

/// The whole page payload.
#[derive(Debug, Clone, Serialize)]
struct PageData {
    schema: &'static str,
    biome: String,
    /// `eye_height` travels with the data so the page and this crate cannot
    /// disagree about what a player POV is.
    eye_height: f32,
    way_in_stems: Vec<String>,
    models: Vec<ModelOut>,
}

/// Everything one prefab contributes to a page.
pub struct ViewerModel {
    id: String,
    structure: Structure,
    meta: Option<PrefabMeta>,
}

impl ViewerModel {
    /// Read a prefab `.nbt` and its `<basename>.json` metadata sidecar when
    /// present. The sidecar is where anchors live — for hand-built prefabs today
    /// and for grammar snapshots — so a missing one costs anchors and nothing
    /// else.
    pub fn load(nbt_path: &Path) -> Result<ViewerModel, String> {
        let structure = crate::nbt::parse_structure(nbt_path).map_err(|e| e.to_string())?;
        let meta = PrefabMeta::beside_nbt(nbt_path)?;
        let id = nbt_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "prefab".to_string());
        Ok(ViewerModel {
            id,
            structure,
            meta,
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
}

/// What a built page measured, for the caller's diagnostics.
#[derive(Debug, Clone, Default)]
pub struct BuildStats {
    /// Bytes of the emitted page.
    pub bytes: usize,
    /// Anchors plus sockets over every model.
    pub anchors: usize,
    /// Distinct blockstates that could not be resolved, over every model.
    pub unresolved: BTreeMap<String, (Unresolved, u32)>,
}

/// Build the page for one or more prefabs.
///
/// `deriver` supplies colours; `title` names the page. The result is a complete
/// HTML document with every byte inline.
pub fn build_page(
    models: &[ViewerModel],
    colors: &dyn Appearances,
    title: &str,
) -> Result<(String, BuildStats), String> {
    if models.is_empty() {
        return Err("no prefabs to show".to_string());
    }
    let mut stats = BuildStats::default();
    let mut out = Vec::with_capacity(models.len());

    for m in models {
        let built = build_model(m, colors)?;
        stats.anchors += built.anchors.len();
        for (state, u) in &built.unresolved {
            let e = stats
                .unresolved
                .entry(state.clone())
                .or_insert((u.reason, 0));
            e.1 += u.count;
        }
        out.push(built);
    }

    let data = PageData {
        schema: SCHEMA,
        biome: colors.biome().to_string(),
        eye_height: EYE_HEIGHT,
        way_in_stems: WAY_IN_STEMS.iter().map(|s| s.to_string()).collect(),
        models: out,
    };
    let json = serde_json::to_string(&data).map_err(|e| format!("serialise page data: {e}"))?;
    let html = render_html(title, &json);
    stats.bytes = html.len();
    Ok((html, stats))
}

/// Build one model's payload: palette, packed grid, anchors.
fn build_model(m: &ViewerModel, colors: &dyn Appearances) -> Result<ModelOut, String> {
    let st = &m.structure;
    let [sx, sy, sz] = st.size;
    if sx <= 0 || sy <= 0 || sz <= 0 {
        return Err(format!("{}: structure size {:?} is empty", m.id, st.size));
    }
    let (sxu, syu, szu) = (sx as usize, sy as usize, sz as usize);
    let cells = sxu
        .checked_mul(syu)
        .and_then(|v| v.checked_mul(szu))
        .ok_or_else(|| format!("{}: structure size {:?} overflows", m.id, st.size))?;

    // Palette index 0 is air. Every other blockstate gets an index in the
    // structure's own palette order, so the mapping is stable per input.
    let mut index_of: Vec<u16> = vec![0; st.palette.len()];
    let mut entries: Vec<Option<PaletteEntry>> = vec![None];
    let mut unresolved: BTreeMap<String, UnresolvedOut> = BTreeMap::new();
    let mut counts: Vec<u32> = vec![0];

    for (i, state) in st.palette.iter().enumerate() {
        if is_air(state) {
            index_of[i] = 0;
            continue;
        }
        let appearance = match colors.appearance(state) {
            Ok(a) => a,
            Err(reason) => {
                unresolved.entry(state.clone()).or_insert(UnresolvedOut {
                    reason,
                    detail: reason.to_string(),
                    count: 0,
                });
                // An unresolved block is still SOMETHING the reviewer must see —
                // drawing nothing would hide a hole in the building. It gets the
                // missing-texture magenta the fidelity gate already means by
                // "this did not resolve", and is listed on the page.
                Appearance {
                    rgb: [255, 0, 255],
                    coverage: 255,
                    shape: [0, 0, 0, 16, 16, 16],
                }
            }
        };
        if entries.len() >= u16::MAX as usize {
            return Err(format!("{}: more than 65534 distinct blockstates", m.id));
        }
        index_of[i] = entries.len() as u16;
        entries.push(Some(PaletteEntry {
            name: state.clone(),
            rgb: appearance.rgb,
            cov: appearance.coverage,
            shape: appearance.shape,
            count: 0,
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
        }
    }
    for (state, u) in unresolved.iter_mut() {
        // Count cells, not palette entries: "2 blocks of chain" is the number a
        // reviewer can act on.
        if let Some(pi) = st.palette.iter().position(|p| p == state) {
            u.count = counts[index_of[pi] as usize];
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
        anchors: collect_anchors(m.meta.as_ref()),
        unresolved,
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

/// The page shell. Styles and script are compiled in, so the emitted file
/// references nothing outside itself — the Artifact CSP blocks every external
/// host, and a viewer that needs a CDN is a viewer the owner cannot open.
fn render_html(title: &str, data_json: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<style>\n{css}</style>\n</head>\n<body>\n\
         {body}\n\
         <script type=\"application/json\" id=\"delve-model\">{data}</script>\n\
         <script>\n{controls}</script>\n\
         <script>\n{js}</script>\n</body>\n</html>\n",
        title = escape_html(title),
        css = include_str!("viewer/page.css"),
        body = include_str!("viewer/page.html"),
        // The payload sits in a `application/json` block, so the only sequence
        // that could break out of it is a literal `</script`.
        data = data_json.replace("</", "<\\/"),
        // The control mapping loads first and in its own block: it is shared
        // with whatever else drives this viewer, and it depends on nothing the
        // renderer defines. Which key does what is decided in exactly one file.
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
) -> PaletteTable {
    let mut states: Vec<&str> = Vec::new();
    for m in models {
        for s in &m.structure.palette {
            states.push(s.as_str());
        }
    }
    PaletteTable::derive(deriver, states)
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

    /// The page must reference nothing outside itself: the Artifact CSP blocks
    /// every external host, so a single `src`/`href` to one is a broken page.
    #[test]
    fn the_page_shell_is_self_contained() {
        let html = render_html("t", "{}");
        for probe in ["http://", "https://", "//cdn", "src=\"", "href=\""] {
            assert!(!html.contains(probe), "page references {probe}");
        }
    }
}
