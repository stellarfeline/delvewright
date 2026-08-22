//! Contact sheet — many candidates, one page, the owner's eye is the selector
//! (spec-0027 §3 curation step; spec-0028 §3 ranking).
//!
//! # The one rule that governs this module
//!
//! **The score RANKS; it never GATES** (spec-0028 §3). Cross-domain
//! calibration between a painterly reference image and a voxel render is
//! unproven, so a similarity number may decide *where* a candidate sits on the
//! page and may never decide *whether* it is on the page. A low-scoring
//! candidate is still present, last. An **unscored** candidate is still present,
//! last, and is named as unscored — a missing measurement is not a bad one.
//!
//! That rule is not left to prose. Ordering is a *seam*: [`build_sheet`] takes
//! the order function, calls it, and then puts the result through
//! [`verify_total_order`], which refuses (`DW0725`) anything that is not a
//! permutation of the candidate set. A future ranker that filters — the drift
//! this rule exists to stop — fails there instead of quietly shipping a shorter
//! sheet. Promotion of the score to a threshold requires its own owner-approved
//! amendment backed by batch data; until then this guard is the amendment's
//! absence, spelled in code.
//!
//! # Two images, two stages, two producers
//!
//! A **reference image** is concept art drawn by an image model at the
//! design-alignment gate (`tools/refimg.py`), before any prefab exists. A
//! **render** is a candidate prefab imaged by `delve-render`, later, here. This
//! module consumes renders and, optionally, a score file measuring them against
//! a reference. It never draws either one.
//!
//! # Nothing here ships
//!
//! Sheets, like renders, are generation-time working material: local,
//! gitignored, never committed to the content repo, never shipped, so no
//! licensing of anything on this page can reach a delve's bytes (ADR-0013), and
//! nothing here can move them (ADR-0006).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::view::diag::{DW_BINDING, DW_INPUT, DW_RANK_ORDER, Diagnostic};
use crate::view::font;

/// One candidate on the sheet: an id the owner can name it by, and the render
/// that represents it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Stable name — the prefab/zone-expansion id. What she says out loud.
    pub id: String,
    /// The representative render (PNG) for this candidate.
    pub image: PathBuf,
}

/// How candidates were found on disk. Recorded in the manifest so a sheet can be
/// reproduced from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// One subdirectory per candidate (`delve-render batch` output).
    PerCandidateDir,
    /// One PNG per candidate, flat in the directory.
    Flat,
}

/// Discover candidates under `dir`.
///
/// Two layouts, chosen by what is actually there — `delve-render batch` writes
/// one subdirectory of shots per prefab, while a hand-assembled set of picks is
/// usually flat. If any immediate subdirectory holds a PNG, the sheet is built
/// from subdirectories (one representative shot each) and loose top-level PNGs
/// are ignored; otherwise every top-level PNG is a candidate.
///
/// `shot` selects the representative in the per-directory layout: the PNG whose
/// stem ends `-<shot>`. When `shot` is given explicitly and a candidate has no
/// such shot, discovery **refuses** rather than silently substituting a
/// different angle — a sheet whose cells are shot from different directions is
/// not a comparison. With `shot = None` the default `ext-se` is tried and
/// discovery falls back to the first PNG by name.
pub fn discover(dir: &Path, shot: Option<&str>) -> Result<(Vec<Candidate>, Layout), Diagnostic> {
    let entries = read_sorted(dir)?;
    let mut subdir_candidates: Vec<Candidate> = Vec::new();
    let mut missing_shot: Vec<String> = Vec::new();

    for path in entries.iter().filter(|p| p.is_dir()) {
        let pngs = pngs_in(path)?;
        if pngs.is_empty() {
            continue;
        }
        let id = file_name(path);
        let wanted = shot.unwrap_or(DEFAULT_SHOT);
        let pick = pngs
            .iter()
            .find(|p| stem(p).ends_with(&format!("-{wanted}")))
            .or(if shot.is_some() { None } else { pngs.first() });
        match pick {
            Some(image) => subdir_candidates.push(Candidate {
                id,
                image: image.clone(),
            }),
            None => missing_shot.push(id),
        }
    }

    if !missing_shot.is_empty() {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "--shot {} has no render for {} candidate(s): {} — a sheet whose cells are shot \
                 from different angles is not a comparison. Render the missing shot, or drop \
                 --shot to take each candidate's first render by name",
                shot.unwrap_or(DEFAULT_SHOT),
                missing_shot.len(),
                missing_shot.join(", ")
            ),
        ));
    }

    if !subdir_candidates.is_empty() {
        subdir_candidates.sort_by(|a, b| a.id.cmp(&b.id));
        return Ok((subdir_candidates, Layout::PerCandidateDir));
    }

    let flat: Vec<Candidate> = entries
        .iter()
        .filter(|p| is_png(p))
        .map(|p| Candidate {
            id: stem(p),
            image: p.clone(),
        })
        .collect();
    if flat.is_empty() {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "no candidates under {}: expected one subdirectory of renders per candidate \
                 (`delve-render batch` output) or a flat directory of `.png` renders",
                dir.display()
            ),
        ));
    }
    Ok((flat, Layout::Flat))
}

/// The representative angle taken when `--shot` is not given.
pub const DEFAULT_SHOT: &str = "ext-se";

fn read_sorted(dir: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let rd = std::fs::read_dir(dir)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("read dir {}: {e}", dir.display())))?;
    let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    v.sort();
    Ok(v)
}

fn pngs_in(dir: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    Ok(read_sorted(dir)?
        .into_iter()
        .filter(|p| is_png(p))
        .collect())
}

fn is_png(p: &Path) -> bool {
    p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("png")
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("candidate")
        .to_string()
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("candidate")
        .to_string()
}

// ---------------------------------------------------------------------------
// Scores
// ---------------------------------------------------------------------------

/// The score file written by `tools/refscore.py` — one number per candidate,
/// measured against a reference image (or a reference prompt, for a
/// text-conditioned metric).
#[derive(Debug, Clone)]
pub struct ScoreSet {
    /// Metric backend that produced the numbers (`stub`, `open-clip`, `vqascore`).
    pub backend: String,
    /// Model identifier the backend used.
    pub model: Option<String>,
    /// The reference the candidates were measured against.
    pub reference: Option<String>,
    /// The reference prompt, for a text-conditioned metric.
    pub prompt: Option<String>,
    /// `false` for a distance metric, where a *smaller* number is a better
    /// match. Carried explicitly so a future metric cannot silently invert the
    /// page.
    pub higher_is_better: bool,
    /// candidate id → score.
    pub by_id: BTreeMap<String, f64>,
}

const SCORE_SCHEMA: &str = "delvewright.refscore/1";

impl ScoreSet {
    /// Parse a `refscore` JSON document.
    ///
    /// Every failure is loud, and none of them is recovered from — a score file
    /// that cannot be read is not a page in id order, it is a stop. A non-finite
    /// score (a broken metric run) never reaches this type: JSON has no `NaN`
    /// or `Infinity` literal and `serde_json` refuses an out-of-range one, which
    /// is what [`rank_by_score`]'s total ordering rests on.
    pub fn parse(bytes: &[u8]) -> Result<ScoreSet, Diagnostic> {
        let v: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|e| Diagnostic::error(DW_INPUT, format!("scores: {e}")))?;
        let schema = v.get("schema").and_then(|s| s.as_str()).unwrap_or_default();
        if schema != SCORE_SCHEMA {
            return Err(Diagnostic::error(
                DW_INPUT,
                format!("scores: schema is {schema:?}, expected {SCORE_SCHEMA:?}"),
            ));
        }
        let backend = v
            .get("backend")
            .and_then(|s| s.as_str())
            .ok_or_else(|| Diagnostic::error(DW_INPUT, "scores: no `backend`"))?
            .to_string();
        let higher_is_better = v
            .get("higher_is_better")
            .and_then(|b| b.as_bool())
            .ok_or_else(|| {
                Diagnostic::error(
                    DW_INPUT,
                    "scores: no `higher_is_better` — the sort direction is never guessed",
                )
            })?;
        let rows = v
            .get("scores")
            .and_then(|s| s.as_array())
            .ok_or_else(|| Diagnostic::error(DW_INPUT, "scores: no `scores` array"))?;
        let mut by_id = BTreeMap::new();
        for row in rows {
            let id = row
                .get("id")
                .and_then(|s| s.as_str())
                .ok_or_else(|| Diagnostic::error(DW_INPUT, "scores: a row has no `id`"))?;
            let score = row.get("score").and_then(|s| s.as_f64()).ok_or_else(|| {
                Diagnostic::error(DW_INPUT, format!("scores: {id} has no numeric `score`"))
            })?;
            if by_id.insert(id.to_string(), score).is_some() {
                return Err(Diagnostic::error(
                    DW_INPUT,
                    format!("scores: {id} appears twice"),
                ));
            }
        }
        Ok(ScoreSet {
            backend,
            model: v.get("model").and_then(|s| s.as_str()).map(str::to_string),
            reference: v
                .get("reference")
                .and_then(|s| s.as_str())
                .map(str::to_string),
            prompt: v.get("prompt").and_then(|s| s.as_str()).map(str::to_string),
            higher_is_better,
            by_id,
        })
    }
}

// ---------------------------------------------------------------------------
// Binding
// ---------------------------------------------------------------------------

/// How much of the score set actually reached the candidates on this sheet.
///
/// Reported on every sheet, always, because **a green gate that binds to nothing
/// is vacuous, not a pass** (CLAUDE.md): a score file that matched zero
/// candidates orders nothing at all, and would otherwise look exactly like a
/// successful ranking run.
#[derive(Debug, Clone, Serialize)]
pub struct Binding {
    /// Candidates on the sheet.
    pub candidates: usize,
    /// Candidates that a score bound to.
    pub scored: usize,
    /// Candidate ids with no score — present, last, and named here.
    pub unscored: Vec<String>,
    /// Score rows that matched no candidate (usually an id typo or a stale run).
    pub unmatched_score_rows: Vec<String>,
}

/// Bind a score set to the candidates on this sheet.
pub fn bind(candidates: &[Candidate], scores: Option<&ScoreSet>) -> Binding {
    let ids: BTreeSet<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
    let Some(scores) = scores else {
        return Binding {
            candidates: candidates.len(),
            scored: 0,
            unscored: candidates.iter().map(|c| c.id.clone()).collect(),
            unmatched_score_rows: Vec::new(),
        };
    };
    let unscored: Vec<String> = candidates
        .iter()
        .filter(|c| !scores.by_id.contains_key(&c.id))
        .map(|c| c.id.clone())
        .collect();
    let unmatched: Vec<String> = scores
        .by_id
        .keys()
        .filter(|k| !ids.contains(k.as_str()))
        .cloned()
        .collect();
    Binding {
        candidates: candidates.len(),
        scored: candidates.len() - unscored.len(),
        unscored,
        unmatched_score_rows: unmatched,
    }
}

impl Binding {
    /// Diagnostics for this binding (`DW0726`).
    ///
    /// A score file that bound to **zero** candidates is an error: the run asked
    /// for a ranked sheet and produced an unranked one, and that must not read
    /// as success. A partial binding is a warning that names the counts — the
    /// unscored candidates are still on the sheet.
    pub fn diagnose(&self, scores_supplied: bool) -> Vec<Diagnostic> {
        if !scores_supplied {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.scored == 0 {
            out.push(Diagnostic::error(
                DW_BINDING,
                format!(
                    "score set bound to 0 of {} candidate(s) — the sheet is in id order and \
                     nothing was ranked. Score ids must be the candidate ids ({}); the score \
                     file names {}",
                    self.candidates,
                    preview(&self.unscored),
                    preview(&self.unmatched_score_rows),
                ),
            ));
        } else if !self.unscored.is_empty() {
            out.push(Diagnostic::warning(
                DW_BINDING,
                format!(
                    "score set bound to {} of {} candidate(s); unscored (placed last, never \
                     dropped): {}",
                    self.scored,
                    self.candidates,
                    preview(&self.unscored)
                ),
            ));
        }
        if !self.unmatched_score_rows.is_empty() && self.scored > 0 {
            out.push(Diagnostic::warning(
                DW_BINDING,
                format!(
                    "{} score row(s) matched no candidate on this sheet: {}",
                    self.unmatched_score_rows.len(),
                    preview(&self.unmatched_score_rows)
                ),
            ));
        }
        out
    }
}

fn preview(ids: &[String]) -> String {
    const N: usize = 6;
    if ids.is_empty() {
        return "(none)".to_string();
    }
    if ids.len() <= N {
        return ids.join(", ");
    }
    format!("{}, +{} more", ids[..N].join(", "), ids.len() - N)
}

// ---------------------------------------------------------------------------
// Ranking — rank-only, never a gate
// ---------------------------------------------------------------------------

/// Order the candidates for the sheet by similarity score.
///
/// Returns a **permutation** of `0..candidates.len()`: best match first,
/// unscored last, ties and unscored broken by id so the page is deterministic.
/// Nothing is ever removed — see the module docs, and [`verify_total_order`],
/// which enforces it on whatever ordering function `build_sheet` was handed.
pub fn rank_by_score(candidates: &[Candidate], scores: Option<&ScoreSet>) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..candidates.len()).collect();
    let Some(scores) = scores else {
        idx.sort_by(|&a, &b| candidates[a].id.cmp(&candidates[b].id));
        return idx;
    };
    idx.sort_by(|&a, &b| {
        let (sa, sb) = (
            scores.by_id.get(&candidates[a].id),
            scores.by_id.get(&candidates[b].id),
        );
        match (sa, sb) {
            (Some(x), Some(y)) => {
                // `partial_cmp` is `None` only for a `NaN`, which cannot come
                // out of `ScoreSet::parse` (JSON has no such literal). Falling
                // back to `Equal` keeps the comparator total either way — the
                // guard below would catch it, but a page is better than a stop.
                let ord = x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal);
                if scores.higher_is_better {
                    ord.reverse()
                } else {
                    ord
                }
            }
            // Unscored goes last — "not measured" is not "measured badly".
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| candidates[a].id.cmp(&candidates[b].id))
    });
    idx
}

/// The rank-never-gate guard: `order` must be a permutation of `0..n`.
///
/// This is the machine form of that rule (spec-0028 §3). Every way an
/// ordering can stop being rank-only shows up here as the same defect — a
/// threshold that drops a candidate shortens the order, a de-duplicating
/// "best of" repeats an index, an off-by-one loses the last cell. All of them
/// are refused with `DW0725`, so the sheet is never quietly shorter than the
/// candidate set it claims to show.
pub fn verify_total_order(n: usize, order: &[usize]) -> Result<(), Diagnostic> {
    let mut seen = vec![false; n];
    let mut dupes: Vec<usize> = Vec::new();
    for &i in order {
        if i >= n {
            return Err(Diagnostic::error(
                DW_RANK_ORDER,
                format!(
                    "ranking produced out-of-range index {i} for {n} candidate(s) — the score \
                     ORDERS the sheet, it never selects (spec-0028 §3)"
                ),
            ));
        }
        if std::mem::replace(&mut seen[i], true) {
            dupes.push(i);
        }
    }
    let dropped: Vec<usize> = seen
        .iter()
        .enumerate()
        .filter(|(_, s)| !**s)
        .map(|(i, _)| i)
        .collect();
    if dropped.is_empty() && dupes.is_empty() {
        return Ok(());
    }
    Err(Diagnostic::error(
        DW_RANK_ORDER,
        format!(
            "ranking is not a total order over the candidates: {} dropped, {} duplicated \
             ({} of {} placed). The score RANKS the contact sheet; it NEVER gates it — a \
             low-scoring candidate is still present, last (spec-0028 §3). \
             Promoting the score to a threshold needs its own owner-approved amendment \
             backed by batch data; do not add one here",
            dropped.len(),
            dupes.len(),
            order.len(),
            n
        ),
    ))
}

// ---------------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------------

/// Sheet layout knobs.
#[derive(Debug, Clone)]
pub struct SheetOptions {
    /// Cells per row. `None` → `ceil(sqrt(n))`, the squarest page.
    pub columns: Option<u32>,
    /// Thumbnail side, in pixels.
    pub thumb: u32,
    /// Header title.
    pub title: String,
}

impl Default for SheetOptions {
    fn default() -> Self {
        SheetOptions {
            columns: None,
            thumb: 256,
            title: "contact sheet".to_string(),
        }
    }
}

/// A built sheet: the page, and the manifest that names every cell on it.
pub struct Sheet {
    /// The composited page.
    pub image: image::RgbaImage,
    /// Pretty-printed manifest JSON (`delvewright.contact-sheet/1`).
    pub manifest: Vec<u8>,
    /// Binding counts, for the CLI's summary line.
    pub binding: Binding,
    /// Diagnostics raised while building (binding findings).
    pub diagnostics: Vec<Diagnostic>,
}

/// Hand-written so a failing `unwrap_err()` in a test prints the sheet's shape
/// rather than a megabyte of pixels.
impl std::fmt::Debug for Sheet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheet")
            .field("size", &(self.image.width(), self.image.height()))
            .field("manifest_bytes", &self.manifest.len())
            .field("binding", &self.binding)
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

#[derive(Serialize)]
struct Cell {
    rank: usize,
    row: u32,
    col: u32,
    id: String,
    image: String,
    score: Option<f64>,
}

#[derive(Serialize)]
struct RankSource {
    backend: String,
    model: Option<String>,
    reference: Option<String>,
    prompt: Option<String>,
    higher_is_better: bool,
}

#[derive(Serialize)]
struct Manifest {
    schema: &'static str,
    title: String,
    source: String,
    layout: Layout,
    shot: Option<String>,
    columns: u32,
    thumb: u32,
    /// `null` when no score file was supplied — then the page is in id order.
    rank_source: Option<RankSource>,
    /// Always true. The sheet records the ruling it was built under so a later
    /// reader of an archived sheet knows the score never removed anything.
    rank_only_never_gates: bool,
    binding: Binding,
    cells: Vec<Cell>,
}

const PAD: u32 = 10;
const TEXT_SCALE: u32 = 2;
const BG: [u8; 4] = [24, 24, 28, 255];
const CELL_BG: [u8; 4] = [40, 40, 46, 255];
const FG: [u8; 4] = [226, 226, 232, 255];
const DIM: [u8; 4] = [150, 150, 160, 255];
const WARN: [u8; 4] = [250, 196, 90, 255];

/// Build the contact sheet.
///
/// `order_fn` is the ordering seam — the CLI passes [`rank_by_score`]. Whatever
/// it returns goes through [`verify_total_order`] before a single pixel is
/// drawn, so an ordering that filters can never reach the page.
pub fn build_sheet(
    source: &Path,
    candidates: &[Candidate],
    scores: Option<&ScoreSet>,
    layout: Layout,
    shot: Option<&str>,
    opts: &SheetOptions,
    order_fn: impl Fn(&[Candidate], Option<&ScoreSet>) -> Vec<usize>,
) -> Result<Sheet, Diagnostic> {
    if candidates.is_empty() {
        return Err(Diagnostic::error(
            DW_INPUT,
            "no candidates — a sheet of nothing is not a curation page",
        ));
    }
    let binding = bind(candidates, scores);
    let mut diagnostics = binding.diagnose(scores.is_some());
    // An error-tier binding finding stops the page; warnings ride along and are
    // printed beside it. A zero binding ordered nothing, and a page in id order
    // must not be handed over as if a ranking had happened.
    if let Some(i) = diagnostics.iter().position(Diagnostic::is_error) {
        return Err(diagnostics.remove(i));
    }

    let order = order_fn(candidates, scores);
    verify_total_order(candidates.len(), &order)?;

    let n = candidates.len() as u32;
    let columns = opts
        .columns
        .unwrap_or_else(|| (n as f64).sqrt().ceil() as u32)
        .max(1);
    let rows = n.div_ceil(columns);
    let thumb = opts.thumb.max(32);

    let caption_h = font::text_height(TEXT_SCALE) * 2 + PAD * 2 + PAD / 2;
    let cell_w = thumb + PAD * 2;
    let cell_h = thumb + PAD * 2 + caption_h;
    let header = header_lines(scores, &binding, opts, n);
    let header_h = header.len() as u32 * (font::text_height(TEXT_SCALE) + PAD / 2) + PAD * 2;
    // The page is at least as wide as its own header. A truncated binding count
    // is a binding count nobody read, and the header is where every claim this
    // sheet makes about itself lives.
    let header_w = header
        .iter()
        .map(|(t, _)| font::text_width(t, 1))
        .max()
        .unwrap_or(0)
        + PAD * 2;
    let width = (columns * cell_w + PAD * 2).max(header_w);
    let height = header_h + rows * cell_h + PAD * 2;

    let mut img = image::RgbaImage::from_pixel(width, height, image::Rgba(BG));
    let mut y = PAD;
    for (text, color) in &header {
        draw_fitted(&mut img, PAD, y, text, width - PAD * 2, *color);
        y += font::text_height(TEXT_SCALE) + PAD / 2;
    }

    let mut cells = Vec::with_capacity(candidates.len());
    for (rank0, &ci) in order.iter().enumerate() {
        let c = &candidates[ci];
        let row = rank0 as u32 / columns;
        let col = rank0 as u32 % columns;
        let x = PAD + col * cell_w;
        let y = header_h + PAD + row * cell_h;
        fill_rect(&mut img, x, y, cell_w, cell_h, CELL_BG);

        let thumbnail = load_thumb(&c.image, thumb)?;
        let tx = x + PAD + (thumb - thumbnail.width()) / 2;
        let ty = y + PAD + (thumb - thumbnail.height()) / 2;
        image::imageops::overlay(&mut img, &thumbnail, tx as i64, ty as i64);

        let score = scores.and_then(|s| s.by_id.get(&c.id)).copied();
        let cap_y = y + PAD + thumb + PAD;
        let label = format!("{}. {}", rank0 + 1, c.id);
        draw_fitted(&mut img, x + PAD, cap_y, &label, thumb, FG);
        let (second, color) = match score {
            Some(v) => (format!("score {v:.4}"), DIM),
            None if scores.is_some() => ("unscored - kept, placed last".to_string(), WARN),
            None => ("unranked (id order)".to_string(), DIM),
        };
        draw_fitted(
            &mut img,
            x + PAD,
            cap_y + font::text_height(TEXT_SCALE) + PAD / 2,
            &second,
            thumb,
            color,
        );

        cells.push(Cell {
            rank: rank0 + 1,
            row,
            col,
            id: c.id.clone(),
            image: c.image.display().to_string(),
            score,
        });
    }

    let manifest = Manifest {
        schema: "delvewright.contact-sheet/1",
        title: opts.title.clone(),
        source: source.display().to_string(),
        layout,
        shot: shot.map(str::to_string),
        columns,
        thumb,
        rank_source: scores.map(|s| RankSource {
            backend: s.backend.clone(),
            model: s.model.clone(),
            reference: s.reference.clone(),
            prompt: s.prompt.clone(),
            higher_is_better: s.higher_is_better,
        }),
        rank_only_never_gates: true,
        binding: binding.clone(),
        cells,
    };
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| Diagnostic::error(DW_INPUT, e.to_string()))?;
    manifest_bytes.push(b'\n');

    Ok(Sheet {
        image: img,
        manifest: manifest_bytes,
        binding,
        diagnostics,
    })
}

/// The claims the sheet makes about itself, as (text, colour) lines. Pure, so
/// the page can be sized to fit them before anything is drawn.
fn header_lines(
    scores: Option<&ScoreSet>,
    binding: &Binding,
    opts: &SheetOptions,
    n: u32,
) -> Vec<(String, [u8; 4])> {
    const RULING: &str =
        "the score RANKS this page and never gates it - every candidate is here (spec-0028 s3)";
    let mut out = vec![(opts.title.clone(), FG)];
    match scores {
        Some(s) => {
            let model = s.model.clone().unwrap_or_else(|| "-".to_string());
            let reference = s
                .reference
                .as_deref()
                .map(base_name)
                .unwrap_or_else(|| "-".to_string());
            out.push((
                format!(
                    "{n} candidates - ranked by {} ({model}) vs {reference} - {}",
                    s.backend,
                    if s.higher_is_better {
                        "higher is better"
                    } else {
                        "lower is better"
                    }
                ),
                FG,
            ));
            out.push((
                format!(
                    "binding: {} of {} scored, {} unscored, {} unmatched score rows",
                    binding.scored,
                    binding.candidates,
                    binding.unscored.len(),
                    binding.unmatched_score_rows.len()
                ),
                if binding.unscored.is_empty() {
                    DIM
                } else {
                    WARN
                },
            ));
            if s.backend == STUB_BACKEND {
                out.push((
                    "STUB SCORES - NOT A SIMILARITY MEASURE - LOOP EXERCISE ONLY".to_string(),
                    WARN,
                ));
            } else {
                out.push((RULING.to_string(), DIM));
            }
        }
        None => {
            out.push((format!("{n} candidates - no score file - id order"), FG));
            out.push((
                "run tools/refscore.py to order this page by similarity to a reference".to_string(),
                DIM,
            ));
            out.push((RULING.to_string(), DIM));
        }
    }
    out
}

/// Draw `text` at the largest scale that fits `max_px`, truncating only when
/// even the smallest will not do. A label the owner cannot read is a candidate
/// she cannot name.
fn draw_fitted(
    img: &mut image::RgbaImage,
    x: u32,
    y: u32,
    text: &str,
    max_px: u32,
    color: [u8; 4],
) {
    for scale in (1..=TEXT_SCALE).rev() {
        if font::text_width(text, scale) <= max_px {
            font::draw_text(img, x, y, text, scale, color);
            return;
        }
    }
    font::draw_text(img, x, y, &font::fit(text, max_px, 1), 1, color);
}

/// The backend name reserved for the dependency-free loop-exercise scorer.
/// Named here too so the page can shout about it.
pub const STUB_BACKEND: &str = "stub";

fn base_name(s: &str) -> String {
    Path::new(s)
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or(s)
        .to_string()
}

fn fill_rect(img: &mut image::RgbaImage, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
    for py in y..(y + h).min(img.height()) {
        for px in x..(x + w).min(img.width()) {
            img.put_pixel(px, py, image::Rgba(color));
        }
    }
}

/// Load a candidate render and fit it into a `thumb`×`thumb` box, preserving
/// aspect ratio.
fn load_thumb(path: &Path, thumb: u32) -> Result<image::RgbaImage, Diagnostic> {
    let img = image::open(path)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("read {}: {e}", path.display())))?
        .to_rgba8();
    let (w, h) = (img.width().max(1), img.height().max(1));
    let scale = (thumb as f64 / w as f64).min(thumb as f64 / h as f64);
    let nw = ((w as f64 * scale).round() as u32).clamp(1, thumb);
    let nh = ((h as f64 * scale).round() as u32).clamp(1, thumb);
    Ok(image::imageops::resize(
        &img,
        nw,
        nh,
        image::imageops::FilterType::Triangle,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands(ids: &[&str]) -> Vec<Candidate> {
        ids.iter()
            .map(|id| Candidate {
                id: (*id).to_string(),
                image: PathBuf::from(format!("{id}.png")),
            })
            .collect()
    }

    fn score_json(rows: &[(&str, f64)], higher: bool) -> Vec<u8> {
        let scores: Vec<serde_json::Value> = rows
            .iter()
            .map(|(id, s)| serde_json::json!({ "id": id, "score": s }))
            .collect();
        serde_json::to_vec(&serde_json::json!({
            "schema": SCORE_SCHEMA,
            "backend": "stub",
            "higher_is_better": higher,
            "scores": scores,
        }))
        .unwrap()
    }

    /// spec-0028 §5 AC3, the load-bearing half: the LOW scorer is still there.
    #[test]
    fn low_scoring_candidate_is_present_and_last() {
        let c = cands(&["alpha", "bravo", "charlie"]);
        let s = ScoreSet::parse(&score_json(
            &[("alpha", 0.81), ("bravo", 0.02), ("charlie", 0.44)],
            true,
        ))
        .unwrap();
        let order = rank_by_score(&c, Some(&s));
        assert_eq!(
            order.len(),
            c.len(),
            "the score must never drop a candidate"
        );
        let ids: Vec<&str> = order.iter().map(|&i| c[i].id.as_str()).collect();
        assert_eq!(ids, ["alpha", "charlie", "bravo"]);
        assert_eq!(*ids.last().unwrap(), "bravo", "low scorer is last");
        assert!(ids.contains(&"bravo"), "low scorer is PRESENT");
        verify_total_order(c.len(), &order).unwrap();
    }

    #[test]
    fn a_negative_score_still_only_moves_a_candidate_last() {
        let c = cands(&["a", "b"]);
        let s = ScoreSet::parse(&score_json(&[("a", -99.0), ("b", 0.5)], true)).unwrap();
        let order = rank_by_score(&c, Some(&s));
        assert_eq!(order.len(), 2);
        assert_eq!(c[order[1]].id, "a");
    }

    #[test]
    fn lower_is_better_inverts_the_page_without_dropping_anything() {
        let c = cands(&["a", "b", "c"]);
        let s = ScoreSet::parse(&score_json(&[("a", 9.0), ("b", 1.0), ("c", 5.0)], false)).unwrap();
        let ids: Vec<&str> = rank_by_score(&c, Some(&s))
            .iter()
            .map(|&i| c[i].id.as_str())
            .collect();
        assert_eq!(ids, ["b", "c", "a"]);
    }

    #[test]
    fn unscored_candidates_sort_last_and_are_kept() {
        let c = cands(&["a", "b", "z"]);
        let s = ScoreSet::parse(&score_json(&[("b", 0.1)], true)).unwrap();
        let ids: Vec<&str> = rank_by_score(&c, Some(&s))
            .iter()
            .map(|&i| c[i].id.as_str())
            .collect();
        assert_eq!(ids, ["b", "a", "z"], "unscored keep id order, after scored");
    }

    #[test]
    fn ties_break_by_id_so_the_page_is_deterministic() {
        let c = cands(&["m", "a", "z"]);
        let s = ScoreSet::parse(&score_json(&[("m", 0.5), ("a", 0.5), ("z", 0.5)], true)).unwrap();
        let ids: Vec<&str> = rank_by_score(&c, Some(&s))
            .iter()
            .map(|&i| c[i].id.as_str())
            .collect();
        assert_eq!(ids, ["a", "m", "z"]);
    }

    /// The guard, exercised in the exact direction this rule erodes: someone
    /// adds a threshold to the ranker.
    #[test]
    fn a_filtering_ranker_is_refused_with_dw0725() {
        let c = cands(&["a", "b", "c"]);
        let filtered = vec![0usize, 2]; // a threshold dropped `b`
        let err = verify_total_order(c.len(), &filtered).unwrap_err();
        assert_eq!(err.code, "DW0725");
        assert!(err.message.contains("NEVER gates"), "{}", err.message);
    }

    #[test]
    fn a_duplicating_or_out_of_range_ranker_is_refused_too() {
        assert_eq!(
            verify_total_order(3, &[0, 0, 1, 2]).unwrap_err().code,
            "DW0725"
        );
        assert_eq!(verify_total_order(2, &[0, 5]).unwrap_err().code, "DW0725");
        verify_total_order(3, &[2, 0, 1]).unwrap();
        verify_total_order(0, &[]).unwrap();
    }

    /// A score too large for `f64` is a broken metric run. It must stop the
    /// page, not silently become an `Infinity` that sorts first.
    #[test]
    fn an_out_of_range_score_is_refused_at_parse() {
        let bytes = br#"{"schema":"delvewright.refscore/1","backend":"stub",
            "higher_is_better":true,"scores":[{"id":"a","score":1e400}]}"#;
        let err = ScoreSet::parse(bytes).unwrap_err();
        assert_eq!(err.code, "DW0721");
        assert!(err.message.contains("out of range"), "{}", err.message);
    }

    #[test]
    fn score_schema_direction_and_duplicates_are_refused() {
        let bad_schema = br#"{"schema":"other/1","backend":"stub","higher_is_better":true,
            "scores":[]}"#;
        assert_eq!(ScoreSet::parse(bad_schema).unwrap_err().code, "DW0721");

        let no_direction = br#"{"schema":"delvewright.refscore/1","backend":"stub","scores":[]}"#;
        let err = ScoreSet::parse(no_direction).unwrap_err();
        assert!(err.message.contains("higher_is_better"), "{}", err.message);

        let dupe = br#"{"schema":"delvewright.refscore/1","backend":"stub",
            "higher_is_better":true,"scores":[{"id":"a","score":1},{"id":"a","score":2}]}"#;
        assert!(
            ScoreSet::parse(dupe).unwrap_err().message.contains("twice"),
            "a duplicated id must be refused"
        );
    }

    #[test]
    fn zero_binding_is_a_finding_not_a_pass() {
        let c = cands(&["a", "b"]);
        let s = ScoreSet::parse(&score_json(&[("x", 0.5), ("y", 0.1)], true)).unwrap();
        let b = bind(&c, Some(&s));
        assert_eq!(b.scored, 0);
        assert_eq!(b.candidates, 2);
        let d = b.diagnose(true);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, "DW0726");
        assert!(d[0].is_error(), "a zero binding is an ERROR, not a warning");
    }

    #[test]
    fn partial_binding_warns_and_names_the_counts() {
        let c = cands(&["a", "b", "c"]);
        let s = ScoreSet::parse(&score_json(&[("a", 0.5), ("q", 0.1)], true)).unwrap();
        let b = bind(&c, Some(&s));
        assert_eq!((b.scored, b.candidates), (1, 3));
        let d = b.diagnose(true);
        assert_eq!(d.len(), 2, "unscored candidates AND an unmatched row");
        assert!(d.iter().all(|x| !x.is_error()));
        assert!(d.iter().all(|x| x.code == "DW0726"));
        assert!(d[0].message.contains("1 of 3"), "{}", d[0].message);
    }

    #[test]
    fn no_score_file_means_no_binding_finding() {
        let c = cands(&["a"]);
        assert!(bind(&c, None).diagnose(false).is_empty());
    }
}
