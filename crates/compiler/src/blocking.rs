//! `delvec blocking-chart` — per-elevation cutaway floor plans (spec-0015
//! pillar 3).
//!
//! The viewport ([`crate::snapshot`]) answers "what does it look like from
//! here". This answers the question a viewport structurally cannot: **is there
//! room**. Crowding, an NPC standing in a doorway, a stealth zone that overlaps
//! the only walk corridor, a fire pit with no clearance around it — those are
//! *plan* defects, and a plan is what you read them off.
//!
//! ## Why a cutaway, and why per elevation
//!
//! There is no in-world camera that can see a roofed cavern from above, so an
//! orthographic top-down render simply **excludes everything above the cut**:
//! a dollhouse view, generated straight from the voxel model rather than
//! staged in game. And one plan per area is not enough — the island's mountain
//! has a cavern floor and, four blocks up, a ramp and a sheep pen. Flattening
//! them into one image hides whichever loses the depth test.
//!
//! So the elevations are **found, not declared**: the walkable cells of the area
//! are histogrammed by Y, and the populated plateaus of that histogram become
//! the bands ([`bands_of`]). A ramp contributes a few cells at each intervening
//! Y and does not become a band of its own; the floor and the pen do.
//!
//! ## What a slice shows
//!
//! For band floor `Y`, every column of the area is drawn from the topmost block
//! in `[Y-1, Y+3)` — the floor the player stands on plus anything up to head
//! height — flat-shaded with [`crate::snapshot::block_color`] and lightened with
//! height so a step reads as a step. Over that: a walkable wash on standable
//! cells, the DW0311-proven critical-path corridor as a tint, and a labelled
//! marker for every anchor, gate, NPC/actor post, interact marker, stealth zone
//! and trigger region whose elevation falls in the band.
//!
//! ## Determinism (ADR-0006)
//!
//! Band detection reads a `BTreeMap` histogram with fixed tie-breaks, targets
//! arrive pre-sorted from [`crate::snapshot::collect_targets`], label placement
//! is the deterministic greedy placer, and the PNG encoder pins its DEFLATE
//! level. Two runs produce byte-identical PNGs and index.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::nav::World;
use crate::plan::Plan;
use crate::raster::{Canvas, GLYPH_ROWS, LabelPlacer, ScreenBox, kind_color, text_width};
use crate::snapshot::{Target, block_color};

/// Minimum separation, in blocks, between two detected elevation bands. Below
/// this two "floors" are really one floor with a step in it, and merging them
/// keeps the chart set short.
pub const MIN_BAND_GAP: i32 = 3;

/// How far a candidate elevation must stand out from the elevations either side
/// of it to be a band: its walkable-cell count must be at least this multiple of
/// its larger neighbour's.
///
/// **Relief, not share.** An earlier rule ("hold ≥4% of the area's walkable
/// cells") is wrong in principle and wrong in fact: it makes a level's status
/// depend on how big the rest of the *area* is, so the island's sheep pen —
/// unambiguously a second storey — failed to be a level purely because the
/// beach and meadow below it are large. Relief asks the local question instead:
/// does walkable area *concentrate* here relative to its immediate surroundings?
/// A floor and a mezzanine both do; a ramp, which contributes one or two transit
/// cells per Y, never does, no matter how small the area around it is.
pub const BAND_RELIEF: usize = 3;

/// A band must hold at least this many walkable cells outright — the floor that
/// keeps single-cell noise (a lone ledge, a snapped anchor) out of the chart set.
pub const BAND_MIN_CELLS: usize = 6;

/// At most this many slices per area, largest bands first (then re-sorted by
/// elevation). A hard cap so a pathological layout cannot emit hundreds of PNGs.
pub const MAX_BANDS: usize = 8;

/// Blocks above the band floor that are drawn (the cut plane sits here). Chosen
/// as the standing cell + head + one: enough to show a doorway lintel and a
/// waist-high obstacle, low enough that a ceiling never sneaks in.
pub const BAND_HEIGHT: i32 = 3;

/// Target pixel width for a slice; the per-block scale is derived from it and
/// then clamped by [`MIN_SCALE`]/[`MAX_SCALE`].
const TARGET_PIXELS: i32 = 1000;
/// Smallest / largest pixels-per-block. The floor keeps a big area legible; the
/// ceiling keeps a tiny test area from producing an enormous image.
const MIN_SCALE: i32 = 4;
const MAX_SCALE: i32 = 14;

/// Height of the title bar above the plan.
const TITLE_BAR: i32 = 22;

/// Background (void / outside the cut) colour.
const BACKDROP: [u8; 3] = [16, 17, 22];
/// Walkable-cell wash.
const WALKABLE_TINT: [u8; 3] = [96, 220, 130];
/// Critical-path corridor tint.
const CORRIDOR_TINT: [u8; 3] = [255, 128, 64];

/// One detected elevation band of one area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Band {
    /// The band's walkable floor Y (the cell a player's feet occupy).
    pub floor_y: i32,
    /// How many walkable cells sit in this band.
    pub cells: usize,
}

/// Detect the walkable elevation bands of a set of standable cells.
///
/// The histogram of walkable-cell counts per Y has a plateau at every real
/// floor and a thin smear across the ramps between them. A band is a local
/// maximum of that histogram that stands out from its neighbours by
/// [`BAND_RELIEF`] and holds at least [`BAND_MIN_CELLS`] cells; maxima closer
/// than [`MIN_BAND_GAP`] are merged into the larger one, and at most
/// [`MAX_BANDS`] survive.
///
/// The maximum test is `n > count(y-1) && n >= count(y+1)`, deliberately
/// asymmetric: on a plateau it reports the **lowest** Y, which is the floor a
/// player stands on rather than the top of the step.
///
/// Deterministic: the histogram is a `BTreeMap`, ties break toward the lower Y.
pub fn bands_of(walkable: &BTreeSet<[i32; 3]>) -> Vec<Band> {
    let mut hist: BTreeMap<i32, usize> = BTreeMap::new();
    for c in walkable {
        *hist.entry(c[1]).or_default() += 1;
    }
    if hist.is_empty() {
        return Vec::new();
    }
    let count = |y: i32| hist.get(&y).copied().unwrap_or(0);
    let stands_out =
        |y: i32, n: usize| n >= BAND_MIN_CELLS && n >= BAND_RELIEF * count(y - 1).max(count(y + 1));

    let mut peaks: Vec<Band> = hist
        .iter()
        .filter(|&(&y, &n)| n > count(y - 1) && n >= count(y + 1) && stands_out(y, n))
        .map(|(&y, &n)| Band {
            floor_y: y,
            cells: n,
        })
        .collect();
    // Nothing stood out (a lone ramp, a one-cell test world): fall back to the
    // most populated Y, ties to the lowest, so a chart always exists.
    if peaks.is_empty()
        && let Some((&y, &n)) = hist.iter().max_by_key(|&(&y, &n)| (n, -y))
    {
        peaks.push(Band {
            floor_y: y,
            cells: n,
        });
    }

    // Merge peaks closer than MIN_BAND_GAP, keeping the more populated one
    // (ties → the lower elevation, which is the floor rather than the step).
    peaks.sort_by_key(|b| (std::cmp::Reverse(b.cells), b.floor_y));
    let mut kept: Vec<Band> = Vec::new();
    for b in peaks {
        if kept
            .iter()
            .any(|k| (k.floor_y - b.floor_y).abs() < MIN_BAND_GAP)
        {
            continue;
        }
        kept.push(b);
        if kept.len() == MAX_BANDS {
            break;
        }
    }
    kept.sort_by_key(|b| b.floor_y);

    // Coverage pass — the invariant that makes the chart set trustworthy:
    // **every populated elevation is drawn on some slice**. Relief finds the
    // storeys, but rolling outdoor ground (the island's meadow climbing from
    // beach to cave mouth) has no storeys at all: no Y stands out, so relief
    // alone would leave a walkable stretch on no chart, and a reviewer would
    // have no way to know. Any populated elevation left outside every band's cut
    // gets a band of its own, lowest first, until the layout is covered or
    // [`MAX_BANDS`] is reached. These fill-in bands deliberately bypass the
    // MIN_BAND_GAP merge: coverage outranks tidiness.
    while kept.len() < MAX_BANDS {
        let uncovered = hist.iter().find(|&(&y, &n)| {
            n >= BAND_MIN_CELLS
                && !kept
                    .iter()
                    .any(|b| y >= b.floor_y - 1 && y < b.floor_y + BAND_HEIGHT)
        });
        let Some((&y, &n)) = uncovered else {
            break;
        };
        kept.push(Band {
            floor_y: y,
            cells: n,
        });
        kept.sort_by_key(|b| b.floor_y);
    }
    kept
}

/// One rendered slice: its PNG bytes and the metadata the index records.
pub struct Slice {
    /// Output file name (`<area>-band<i>-y<Y>.png`).
    pub file: String,
    /// The area this slice belongs to.
    pub area_id: String,
    /// The band it cuts at.
    pub band: Band,
    /// Inclusive world Y range drawn.
    pub y_range: (i32, i32),
    /// Image size in pixels.
    pub size: (u32, u32),
    /// Ids of the targets labelled on it, in draw order.
    pub labelled: Vec<String>,
    /// The PNG bytes.
    pub png: Vec<u8>,
}

/// Everything a chart run produces: the slices plus the index document.
pub struct Chart {
    /// Rendered slices, area order then ascending elevation.
    pub slices: Vec<Slice>,
    /// `blocking-chart.json` — the machine-readable index.
    pub index: Value,
}

/// Render the blocking chart for a whole campaign: every area, every detected
/// elevation band.
pub fn chart(
    plan: &Plan,
    blocks: &BTreeMap<[i32; 3], String>,
    world: &World,
    targets: &[Target],
    corridor: &BTreeSet<[i32; 3]>,
) -> Chart {
    let mut slices = Vec::new();
    let mut areas_json = Vec::new();
    for area in &plan.areas {
        let (min, max) = area.bounds();
        let walkable = area_walkable(plan, world, &area.area_id, min, max);
        let bands = bands_of(&walkable);
        let mut band_json = Vec::new();
        for (i, band) in bands.iter().enumerate() {
            let slice = render_slice(
                &area.area_id,
                i,
                band,
                min,
                max,
                blocks,
                &walkable,
                targets,
                corridor,
            );
            band_json.push(json!({
                "index": i,
                "floor_y": band.floor_y,
                "walkable_cells": band.cells,
                "y_range": [slice.y_range.0, slice.y_range.1],
                "file": slice.file,
                "width": slice.size.0,
                "height": slice.size.1,
                "labelled": slice.labelled,
            }));
            slices.push(slice);
        }
        areas_json.push(json!({
            "area": area.area_id,
            "bounds": { "min": min, "max": max },
            "walkable_cells": walkable.len(),
            "bands": band_json,
        }));
    }
    let index = json!({
        "chart_version": 1,
        "campaign_id": plan.campaign.world.campaign_id.as_str(),
        "delvec": crate::DELVEC_VERSION,
        "orientation": "top-down orthographic; +X right, +Z down (north up)",
        "cut": format!(
            "each slice draws world Y in [floor-1, floor+{BAND_HEIGHT}); everything above the \
             cut plane is excluded (dollhouse view)"
        ),
        "areas": areas_json,
    });
    Chart { slices, index }
}

/// The connected walkable cells of one area: a BFS from every point anchor the
/// area declares, clipped to the area's own AABB.
///
/// Rooting the search at the anchors (rather than taking every standable cell in
/// the box) is what makes a band mean "floor the delve actually uses" — a sealed
/// void pocket inside the rock is standable and irrelevant.
fn area_walkable(
    plan: &Plan,
    world: &World,
    area_id: &str,
    min: [i32; 3],
    max: [i32; 3],
) -> BTreeSet<[i32; 3]> {
    let starts: Vec<[i32; 3]> = plan
        .anchors
        .iter()
        .filter(|((a, _), _)| a == area_id)
        .map(|(_, r)| match r {
            crate::plan::ResolvedAnchor::Point { pos, .. } => *pos,
            crate::plan::ResolvedAnchor::Gate { from, to, .. } => [
                (from[0] + to[0]) / 2,
                (from[1] + to[1]) / 2,
                (from[2] + to[2]) / 2,
            ],
        })
        .collect();
    world
        .reachable_walkable(&starts)
        .into_iter()
        .filter(|c| (0..3).all(|a| c[a] >= min[a] && c[a] <= max[a]))
        .collect()
}

/// The `(x, z)` extent one band's slice is cropped to: everything the slice
/// draws (blocks inside the cut, walkable cells on the band floor, and the
/// targets it labels), padded by [`VIEW_MARGIN`] and clamped to the area.
/// Degenerate (nothing at all in the band) falls back to the full area.
#[allow(clippy::too_many_arguments)]
fn view_bounds(
    band: &Band,
    y_lo: i32,
    y_hi: i32,
    min: [i32; 3],
    max: [i32; 3],
    blocks: &BTreeMap<[i32; 3], String>,
    walkable: &BTreeSet<[i32; 3]>,
    targets: &[Target],
) -> ([i32; 2], [i32; 2]) {
    let mut lo = [i32::MAX; 2];
    let mut hi = [i32::MIN; 2];
    fn note(lo: &mut [i32; 2], hi: &mut [i32; 2], x: i32, z: i32) {
        lo[0] = lo[0].min(x);
        lo[1] = lo[1].min(z);
        hi[0] = hi[0].max(x);
        hi[1] = hi[1].max(z);
    }
    // The extent is driven by the band's WALKABLE cells and the markers on it —
    // the things the slice is about — not by every block inside the cut. On an
    // island whose single area runs from beach to mountain, terrain exists at
    // almost every Y across the whole footprint, so cropping to blocks crops to
    // nothing; cropping to the floor gives each storey its own frame.
    for c in walkable.iter().filter(|c| c[1] == band.floor_y) {
        note(&mut lo, &mut hi, c[0], c[2]);
    }
    for t in targets
        .iter()
        .filter(|t| target_in_band(t, y_lo, y_hi, min, max))
    {
        note(&mut lo, &mut hi, t.min[0].max(min[0]), t.min[2].max(min[2]));
        note(&mut lo, &mut hi, t.max[0].min(max[0]), t.max[2].min(max[2]));
    }
    // Nothing walkable and nothing marked: fall back to the drawn blocks, then
    // to the whole area, so a slice is never empty.
    if lo[0] > hi[0] {
        for (cell, _) in blocks.range([min[0], y_lo, min[2]]..=[max[0], y_hi, max[2]]) {
            if cell[1] >= y_lo && cell[1] <= y_hi && in_box(*cell, min, max) {
                note(&mut lo, &mut hi, cell[0], cell[2]);
            }
        }
    }
    if lo[0] > hi[0] {
        return ([min[0], min[2]], [max[0], max[2]]);
    }
    (
        [
            (lo[0] - VIEW_MARGIN).max(min[0]),
            (lo[1] - VIEW_MARGIN).max(min[2]),
        ],
        [
            (hi[0] + VIEW_MARGIN).min(max[0]),
            (hi[1] + VIEW_MARGIN).min(max[2]),
        ],
    )
}

/// Blocks of padding around a slice's cropped content.
const VIEW_MARGIN: i32 = 5;

/// Render one elevation slice.
#[allow(clippy::too_many_arguments)]
fn render_slice(
    area_id: &str,
    index: usize,
    band: &Band,
    min: [i32; 3],
    max: [i32; 3],
    blocks: &BTreeMap<[i32; 3], String>,
    walkable: &BTreeSet<[i32; 3]>,
    targets: &[Target],
    corridor: &BTreeSet<[i32; 3]>,
) -> Slice {
    let (y_lo, y_hi) = (band.floor_y - 1, band.floor_y + BAND_HEIGHT - 1);
    // Crop to what this band actually contains. A campaign area is the union of
    // its pieces, so the island's single area spans beach to mountain-top — and
    // an uncropped cavern slice would be a small drawing in a large field of
    // void, at the smallest scale the widest band needs. Cropping gives every
    // band its own extent, and therefore its own (larger) scale.
    let (view_min, view_max) = view_bounds(band, y_lo, y_hi, min, max, blocks, walkable, targets);
    let (nx, nz) = (view_max[0] - view_min[0] + 1, view_max[1] - view_min[1] + 1);
    let min = [view_min[0], min[1], view_min[1]];
    let max = [view_max[0], max[1], view_max[1]];
    let scale = (TARGET_PIXELS / nx.max(nz).max(1)).clamp(MIN_SCALE, MAX_SCALE);
    let width = (nx * scale) as u32;
    let height = (nz * scale + TITLE_BAR) as u32;
    let mut c = Canvas::filled(width, height, BACKDROP);

    // --- terrain: topmost block of each column within the cut ---------------
    for zi in 0..nz {
        for xi in 0..nx {
            let (x, z) = (min[0] + xi, min[2] + zi);
            let mut top = None;
            for y in (y_lo..=y_hi).rev() {
                if let Some(name) = blocks.get(&[x, y, z]) {
                    top = Some((y, name));
                    break;
                }
            }
            let Some((y, name)) = top else {
                continue;
            };
            let (base, emissive) = block_color(name);
            // Height relief: the higher within the cut, the brighter. A step,
            // a ledge and a lintel all read without any shading model.
            let rel = (y - y_lo) as f64 / (y_hi - y_lo).max(1) as f64;
            let f = if emissive { 1.0 } else { 0.62 + 0.38 * rel };
            let color = [
                (base[0] as f64 * f).round() as u8,
                (base[1] as f64 * f).round() as u8,
                (base[2] as f64 * f).round() as u8,
            ];
            fill_cell(&mut c, min, scale, x, z, color, 1.0);
        }
    }

    // --- overlays: walkable wash, then the proven critical-path corridor -----
    for cell in walkable.iter().filter(|c| c[1] == band.floor_y) {
        fill_cell(&mut c, min, scale, cell[0], cell[2], WALKABLE_TINT, 0.22);
    }
    for cell in corridor
        .iter()
        .filter(|c| c[1] >= y_lo && c[1] <= y_hi && in_box(**c, min, max))
    {
        fill_cell(&mut c, min, scale, cell[0], cell[2], CORRIDOR_TINT, 0.45);
    }

    // --- markers + labels ---------------------------------------------------
    let on_band: Vec<&Target> = targets
        .iter()
        .filter(|t| target_in_band(t, y_lo, y_hi, min, max))
        .collect();
    // Double-size text only when the slice has both the pixels for it and few
    // enough labels that they will not smother the plan they annotate.
    let text_scale = if scale >= 8 && on_band.len() <= 16 {
        2
    } else {
        1
    };
    let th = GLYPH_ROWS * text_scale;
    let mut placer = LabelPlacer::default();
    let mut labelled = Vec::new();
    for t in &on_band {
        let color = kind_color(t.kind);
        let b = cell_box(min, scale, t.min[0], t.min[2], t.max[0], t.max[2]);
        c.stroke_rect(b, color, 0.9);
        if t.is_point() {
            c.fill_rect(b, color, 0.35);
        }
        let text = t.label();
        let tw = text_width(&text, text_scale);
        let want = ScreenBox {
            x: (b.x).min(width as i64 - tw - 2).max(2),
            y: (b.y - th - 2).max(TITLE_BAR as i64 + 2),
            w: tw + 2,
            h: th + 2,
        };
        // Nudged labels must stay on the plan: a name pushed past the bottom
        // edge is worse than no name, and the index records which ids were
        // dropped.
        if let Some(placed) = placer.place(want, th + 4, 20)
            && placed.y + placed.h < height as i64
        {
            c.stamp_text(want.x, placed.y, &text, text_scale, color);
            labelled.push(t.id.clone());
        }
    }

    // --- title bar ----------------------------------------------------------
    c.fill_rect(
        ScreenBox {
            x: 0,
            y: 0,
            w: width as i64,
            h: TITLE_BAR as i64,
        },
        [10, 11, 15],
        1.0,
    );
    c.stamp_text(
        6,
        7,
        &format!(
            "{} BAND {} FLOOR Y{} CUT Y{}-{} N-UP",
            local_of(area_id).to_ascii_uppercase(),
            index,
            band.floor_y,
            y_lo,
            y_hi
        ),
        1,
        [220, 226, 235],
    );

    let png = crate::png::encode_rgba(width, height, &c.rgba);
    Slice {
        file: format!("{}-band{index}-y{}.png", local_of(area_id), band.floor_y),
        area_id: area_id.to_string(),
        band: band.clone(),
        y_range: (y_lo, y_hi),
        size: (width, height),
        labelled,
        png,
    }
}

/// Whether a target belongs on this slice: its cell box overlaps the cut's Y
/// range and the area's footprint.
fn target_in_band(t: &Target, y_lo: i32, y_hi: i32, min: [i32; 3], max: [i32; 3]) -> bool {
    t.min[1] <= y_hi
        && t.max[1] >= y_lo
        && t.min[0] <= max[0]
        && t.max[0] >= min[0]
        && t.min[2] <= max[2]
        && t.max[2] >= min[2]
}

/// Whether a cell is inside an inclusive box.
fn in_box(c: [i32; 3], min: [i32; 3], max: [i32; 3]) -> bool {
    (0..3).all(|a| c[a] >= min[a] && c[a] <= max[a])
}

/// The pixel rectangle of the inclusive cell span `[x0..x1] × [z0..z1]`.
fn cell_box(min: [i32; 3], scale: i32, x0: i32, z0: i32, x1: i32, z1: i32) -> ScreenBox {
    let (lx, hx) = (x0.min(x1), x0.max(x1));
    let (lz, hz) = (z0.min(z1), z0.max(z1));
    ScreenBox {
        x: ((lx - min[0]) * scale) as i64,
        y: ((lz - min[2]) * scale) as i64 + TITLE_BAR as i64,
        w: ((hx - lx + 1) * scale) as i64,
        h: ((hz - lz + 1) * scale) as i64,
    }
}

/// Blend one world cell's square.
fn fill_cell(
    c: &mut Canvas,
    min: [i32; 3],
    scale: i32,
    x: i32,
    z: i32,
    color: [u8; 3],
    alpha: f64,
) {
    c.fill_rect(cell_box(min, scale, x, z, x, z), color, alpha);
}

/// The local (post-`/`) part of an id.
fn local_of(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat walkable plate of `n × n` cells at elevation `y`.
    fn plate(y: i32, n: i32, x0: i32, z0: i32) -> BTreeSet<[i32; 3]> {
        let mut s = BTreeSet::new();
        for x in x0..x0 + n {
            for z in z0..z0 + n {
                s.insert([x, y, z]);
            }
        }
        s
    }

    #[test]
    fn a_single_floor_is_one_band() {
        let b = bands_of(&plate(69, 10, 0, 0));
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].floor_y, 69);
        assert_eq!(b[0].cells, 100);
    }

    #[test]
    fn two_floors_joined_by_a_ramp_are_two_bands_not_five() {
        // The island's mountain in miniature: a big cavern floor at 69, a pen at
        // 73, and a four-cell ramp bridging them. Naive "every distinct Y is a
        // level" would emit five slices; the populated-plateau rule emits two.
        let mut w = plate(69, 12, 0, 0);
        w.extend(plate(73, 6, 20, 0));
        for (i, y) in (70..=72).enumerate() {
            w.insert([12 + i as i32, y, 0]);
        }
        let b = bands_of(&w);
        assert_eq!(
            b.iter().map(|b| b.floor_y).collect::<Vec<_>>(),
            vec![69, 73],
            "cavern floor and pen, no ramp bands: {b:?}"
        );
    }

    #[test]
    fn levels_closer_than_the_gap_merge_into_the_more_populated_one() {
        // A floor at 69 with a wide step at 70 is one floor with a step in it.
        let mut w = plate(69, 12, 0, 0);
        w.extend(plate(70, 8, 0, 0));
        let b = bands_of(&w);
        assert_eq!(b.len(), 1, "one merged band: {b:?}");
        assert_eq!(b[0].floor_y, 69, "the more populated elevation wins");
    }

    #[test]
    fn a_thin_ramp_alone_still_yields_a_band() {
        // Degenerate input must never produce zero charts.
        let mut w = BTreeSet::new();
        for y in 60..70 {
            w.insert([0, y, 0]);
        }
        assert!(!bands_of(&w).is_empty());
    }

    #[test]
    fn empty_walkable_space_yields_no_bands() {
        assert!(bands_of(&BTreeSet::new()).is_empty());
    }

    #[test]
    fn a_small_upper_storey_survives_a_large_ground_floor() {
        // The island's sheep pen, and the reason band detection measures RELIEF
        // rather than share: a 36-cell pen four blocks above a 1000-cell beach +
        // meadow is unambiguously a second storey, but it is only 3% of the area.
        let mut w = plate(63, 32, 0, 0); // 1024-cell ground level
        w.extend(plate(73, 6, 100, 0)); // 36-cell pen far above
        for (i, y) in (64..=72).enumerate() {
            w.insert([90 + i as i32, y, 0]); // the ramp between them
        }
        assert_eq!(
            bands_of(&w).iter().map(|b| b.floor_y).collect::<Vec<_>>(),
            vec![63, 73],
            "the pen must be its own band"
        );
    }

    #[test]
    fn rolling_ground_with_no_storeys_is_still_fully_covered() {
        // The island's meadow: walkable ground climbing one block at a time from
        // the beach to the cave mouth. No elevation stands out, so relief alone
        // finds at most one band and leaves the rest of the climb on no chart.
        // The coverage pass is what makes "interiors fully covered by
        // construction" true of exteriors too.
        let mut w = BTreeSet::new();
        for (i, y) in (63..=72).enumerate() {
            for k in 0..40 {
                w.insert([i as i32 * 4 + k % 4, y, k / 4]);
            }
        }
        let bands = bands_of(&w);
        for y in 63..=72 {
            assert!(
                bands
                    .iter()
                    .any(|b| y >= b.floor_y - 1 && y < b.floor_y + BAND_HEIGHT),
                "elevation {y} appears on no slice: {bands:?}"
            );
        }
    }

    #[test]
    fn band_count_is_capped() {
        // Ten well-separated floors → at most MAX_BANDS slices.
        let mut w = BTreeSet::new();
        for k in 0..10 {
            w.extend(plate(60 + k * 5, 6, 0, 0));
        }
        assert!(bands_of(&w).len() <= MAX_BANDS);
    }

    #[test]
    fn bands_are_sorted_by_elevation() {
        let mut w = plate(80, 8, 0, 0);
        w.extend(plate(60, 10, 0, 0));
        w.extend(plate(70, 9, 0, 0));
        let ys: Vec<i32> = bands_of(&w).iter().map(|b| b.floor_y).collect();
        let mut sorted = ys.clone();
        sorted.sort_unstable();
        assert_eq!(ys, sorted, "ascending elevation: {ys:?}");
    }

    #[test]
    fn cell_boxes_tile_without_gaps_or_overlap() {
        let min = [-4, 0, -7];
        let a = cell_box(min, 6, -4, -7, -4, -7);
        let right = cell_box(min, 6, -3, -7, -3, -7);
        let below = cell_box(min, 6, -4, -6, -4, -6);
        assert_eq!((a.x, a.w), (0, 6));
        assert_eq!(a.y, TITLE_BAR as i64, "the plan starts under the title bar");
        assert_eq!(right.x, a.x + a.w);
        assert_eq!(below.y, a.y + a.h);
        // A span box covers exactly its cells.
        let span = cell_box(min, 6, -4, -7, -2, -5);
        assert_eq!((span.w, span.h), (18, 18));
    }

    #[test]
    fn a_target_is_placed_on_the_band_its_elevation_falls_in() {
        let (min, max) = ([0, 0, 0], [31, 127, 31]);
        let t = Target {
            id: "anchor/pen".into(),
            kind: "anchor",
            area: "area/island".into(),
            min: [10, 73, 10],
            max: [10, 73, 10],
        };
        // Band at 73 (cut 72..75) contains it; the band at 69 (cut 68..71) does not.
        assert!(target_in_band(&t, 72, 75, min, max));
        assert!(!target_in_band(&t, 68, 71, min, max));
        // A tall region target straddling both appears on both — correct: a
        // stealth zone that spans two floors is a fact about both floors.
        let tall = Target {
            min: [10, 69, 10],
            max: [10, 74, 10],
            ..t
        };
        assert!(target_in_band(&tall, 68, 71, min, max));
        assert!(target_in_band(&tall, 72, 75, min, max));
    }

    #[test]
    fn a_target_outside_the_area_footprint_is_excluded() {
        let (min, max) = ([0, 0, 0], [31, 127, 31]);
        let t = Target {
            id: "anchor/elsewhere".into(),
            kind: "anchor",
            area: "area/other".into(),
            min: [400, 69, 10],
            max: [400, 69, 10],
        };
        assert!(!target_in_band(&t, 68, 71, min, max));
    }
}
