//! Horizon surround generation (spec-0026): compiler-generated prefab tiles
//! that dress the world OUTSIDE the scene's placed pieces.
//!
//! This module owns the `valley` base (and its `cherry-valley` parameter row):
//! a mountain annulus of total footprint `ratio`× the scene's, with a flat gap
//! floor between the scene edge and the inner slopes, a radial ridged-noise rim,
//! and a seeded tree layer on the mountains (never in the gap floor or scene —
//! owner directive: 树在山上). Cherry-valley is **not a code path**: one
//! generator, with every species/decor id looked up through parallel tables, so
//! a same-seed cherry build differs from a valley build only in block ids and
//! the biome id (spec-0026 acceptance criterion 6).
//!
//! Design law (spec-0026 §1 valley row, §5; dossier §2.2):
//! - Surround tiles are structure NBT synthesized here at build time, placed by
//!   the same bootstrap `place template` path as scene prefabs, and present in
//!   the assembled voxel model (nav / DW0322 / gravity / snapshots see them).
//! - They are **excluded from boundary-region derivation** and carry no
//!   interior-lighting obligations (wired by the caller, not here).
//! - The inner slopes are proven un-climbable **empirically** (a nav flood from
//!   the gap floor must never cross the crest line), not by slope-angle
//!   promise. This module additionally guarantees it *by construction*: every
//!   surround surface height is quantized to even steps, so no two adjacent
//!   columns ever differ by exactly 1 — with vanilla's 1-block auto-step/jump
//!   there is no standable staircase anywhere on the annulus. The generator
//!   runs its own flood proof (`assert_inner_slopes_unclimbable`) so a
//!   violation dies here, long before the compiler's authoritative nav gate.
//! - Determinism (ADR-0006): everything derives from one `u64` stream seed via
//!   the in-house position-addressed value-noise family (`edit::value_noise`,
//!   `edit::hash01`) and [`crate::Splitmix64`]. No wall clock, no unseeded RNG,
//!   no hash-order iteration (BTreeMap/BTreeSet only), no trig (`sin`/`cos`
//!   go through platform libm and are NOT bit-stable across hosts — the
//!   Poisson sampler uses rejection sampling on squares instead).
//!
//! The tree scatter is a Bridson-style Poisson-disk sampler (Fast Poisson Disk
//! Sampling, SIGGRAPH 2007 sketch — ideas-only, re-implemented in-house; see
//! `docs/ACKNOWLEDGEMENTS.md`), chosen over the plain noise-threshold idiom so
//! tree spacing reads as a grove, not clumped speckle.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

use crate::edit::{hash01, value_noise};
use crate::solver::Splitmix64;

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Which flora dresses the annulus: biome paint + tree species + understory,
/// selected together (spec-0026 §1: "`flora` selects biome + tree species
/// together").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flora {
    /// Oak trees over `minecraft:windswept_forest` (the vanilla mountain-oak
    /// biome: muted grass, oak-bearing).
    Oak,
    /// Cherry trees over `minecraft:cherry_grove` (vanilla's own pink-tinted
    /// grass/ambience — the same channel vanilla uses, no resource pack).
    Cherry,
}

/// The surface palette row (spec-0026: valley default `stone-grass`;
/// cherry-valley shorthand selects `stone-petal`). Only understory/decor ids
/// differ between rows — rock and ground ids are shared, so the palettes stay
/// structurally parallel (criterion 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurroundPalette {
    StoneGrass,
    StonePetal,
}

/// Valley-base parameters (spec-0026 §1 table; defaults pinned there).
#[derive(Debug, Clone, Copy)]
pub struct ValleyParams {
    /// Total footprint as a multiple of the scene's (2..=3, default 2.5).
    pub ratio: f64,
    /// Rim height above the gap floor (default 48).
    pub rim_height: i32,
    pub flora: Flora,
    pub palette: SurroundPalette,
}

impl Default for ValleyParams {
    fn default() -> Self {
        ValleyParams {
            ratio: 2.5,
            rim_height: 48,
            flora: Flora::Oak,
            palette: SurroundPalette::StoneGrass,
        }
    }
}

/// The scene's XZ bounding rectangle in world coordinates (inclusive), i.e.
/// the union of the content areas' placed-piece AABBs. The annulus surrounds
/// this rectangle; nothing is ever generated inside it.
#[derive(Debug, Clone, Copy)]
pub struct SceneRect {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

/// One compiler-generated surround tile: gzip-framed vanilla structure NBT plus
/// its world placement. `size` obeys the library's ≤48×48 XZ safe-template
/// envelope; tall tiles are sliced vertically into ≤48-high stacked tiles.
pub struct SurroundTile {
    /// Datapack structure id (`horizon/valley/t<n>`), unique per tile.
    pub structure_id: String,
    /// Gzip-framed structure NBT (mtime pinned 0 — byte-identical per seed).
    pub bytes: Vec<u8>,
    /// World position of the tile's minimum corner (`place template` args).
    pub pos: [i32; 3],
    pub size: [i32; 3],
}

/// `DW0854` (build, exit 3): the surround's inner slope has grown a standable
/// staircase — a nav walk flood starting on the gap floor reached a column
/// outward of the crest line, so the landform no longer bounds the map.
///
/// Proven over the assembled bytes rather than argued from the generator's own
/// quantization, because everything between the two — gravity settling, a
/// stage-7 edit script, a palette change — can put back the riser the generator
/// never wrote.
pub const DW_VALLEY_CLIMB: delvewright_dsl::DwCode = delvewright_dsl::DwCode::since("DW0854", 16);

/// A biome-paint rectangle for the bootstrap `/fillbiome` pass (vanilla-native
/// tint/ambience channel; spec-0026 §1 layering paragraph).
#[derive(Clone)]
pub struct BiomeRect {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub biome: &'static str,
}

/// A generated valley surround, ready for the caller to wire into the plan
/// (placement + emission), the bootstrap (`fillbiome`), and the nav proof
/// (`gap_floor_starts` → `reachable must never satisfy `beyond_crest`).
pub struct ValleySurround {
    pub tiles: Vec<SurroundTile>,
    pub biome: Vec<BiomeRect>,
    /// Standable cells on the gap floor (feet position, world coords) — the
    /// start set for the compiler-side un-climbability nav flood.
    pub gap_floor_starts: Vec<[i32; 3]>,
    /// The world y of the gap floor's top solid block (walk plane − 1).
    pub floor_top_y: i32,
    seed: u64,
    scene: SceneRect,
    ratio: f64,
    rim_height: i32,
}

impl ValleySurround {
    /// Whether a column lies OUTWARD of the crest line — the "escaped" side of
    /// the un-climbability proof (uses the same warped distance the heightfield
    /// used, so the line is exactly the generated geometry's crest).
    pub fn beyond_crest(&self, x: i32, z: i32) -> bool {
        let d = warped_distance(self.seed, &self.scene, x, z);
        d > GAP_WIDTH + SLOPE_RUN
    }

    /// Whether a column is inside the annulus (outside the scene rect, inside
    /// the `ratio`-scaled outer rect).
    pub fn in_annulus(&self, x: i32, z: i32) -> bool {
        in_annulus(&self.scene, self.ratio, x, z)
    }

    /// The scene-rect interior MOAT fill (spec-0026 amendment, task #157
    /// rounds 3–4 — planner ruling): scene-rect columns no piece FLOORS
    /// receive ambient gap-floor ground at `floor_top_y` with the gap floor's
    /// own surface treatment (the noise is world-coordinate-keyed, so the
    /// dapple runs seamlessly from the annulus gap floor through the moat),
    /// making the box-garden floor continuous from gap floor to every piece
    /// footprint.
    ///
    /// `authored` is the caller's set of floored columns — those where a piece
    /// authors a block at or below `floor_top_y` ([`Plan::ground_columns`],
    /// which owns the rationale). Those columns are untouched: the piece owns
    /// its ground, holes and basements included. A column whose piece content
    /// is entirely ABOVE the gap floor (an elevated storey) is NOT floored and
    /// is filled — the valley floor runs on under it. Emitted as row-strip
    /// tiles (`horizon/valley/m<n>`) so no tile ever carries an air cell over
    /// a floored column (placed air would erase authored blocks).
    ///
    /// [`Plan::ground_columns`]: crate::plan::Plan
    ///
    /// Returns the strip tiles and their standable cells (extra starts for
    /// the un-climbability flood).
    pub fn moat(&self, authored: &BTreeSet<(i32, i32)>) -> (Vec<SurroundTile>, Vec<[i32; 3]>) {
        let mut tiles = Vec::new();
        let mut starts = Vec::new();
        let mut n = 0usize;
        let y_min = self.floor_top_y - SKIRT;
        for z in self.scene.min_z..=self.scene.max_z {
            let mut x = self.scene.min_x;
            while x <= self.scene.max_x {
                if authored.contains(&(x, z)) {
                    x += 1;
                    continue;
                }
                let run_start = x;
                while x <= self.scene.max_x
                    && !authored.contains(&(x, z))
                    && (x - run_start) < TILE_XZ
                {
                    x += 1;
                }
                let x1 = x - 1;
                let mut cells: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
                for cx in run_start..=x1 {
                    for y in y_min..=self.floor_top_y {
                        let name = if y == self.floor_top_y {
                            if value_noise(self.seed, cx, y, z, 0.16, SALT_GAP_DAPPLE) > 0.85 {
                                "minecraft:coarse_dirt"
                            } else {
                                "minecraft:grass_block"
                            }
                        } else if self.floor_top_y - y <= 3 {
                            "minecraft:dirt"
                        } else {
                            pick(
                                &ROCK,
                                value_noise(self.seed, cx, y, z, 0.13, SALT_ROCK_BAND),
                            )
                        };
                        cells.insert([cx, y, z], name);
                    }
                    starts.push([cx, self.floor_top_y + 1, z]);
                }
                let size = [x1 - run_start + 1, SKIRT + 1, 1];
                let bytes = serialize_tile(&cells, [run_start, y_min, z], size);
                tiles.push(SurroundTile {
                    structure_id: format!("horizon/valley/m{n}"),
                    bytes,
                    pos: [run_start, y_min, z],
                    size,
                });
                n += 1;
            }
        }
        (tiles, starts)
    }

    /// The spec-0026 §5 empirical proof, stated over the ASSEMBLED world (the
    /// authoritative geometry, surround tiles included): a nav walk flood from
    /// the gap floor must never stand on a column outward of the crest line.
    /// Not a slope-angle promise — if any palette/tree/settle change ever
    /// grows a standable staircase, this is the check that turns red.
    ///
    /// Returns the first escaped cell (for the caller's DW diagnostic).
    pub fn verify_unclimbable(&self, world: &crate::nav::World) -> Result<(), [i32; 3]> {
        let reached = world.reachable_walkable(&self.gap_floor_starts);
        for cell in reached {
            if self.beyond_crest(cell[0], cell[2]) {
                return Err(cell);
            }
        }
        Ok(())
    }

    /// The establishing-vista camera for the render plan (spec-0026 §6; task
    /// #157 rounds 2–3): eye at a WALKABLE cell inside the scene — the
    /// campaign spawn when the caller has one, else the scene centre — at
    /// player eye height, aimed at MID-RIM on the nearest crest line, with a
    /// per-shot vertical FOV derived from the geometry so the frame's bottom
    /// edge reaches the gap floor (elevation ≤ −15°) and its top edge clears
    /// the crest top by ≥ 8° of sky, clamped to ≤ 110° (planner ruling: a
    /// spawn eye 40 blocks from a 48-high rim subtends ~52° — no fixed FOV
    /// can frame floor + crest + sky). Deterministic: scene bbox + walk
    /// plane + the pinned band tables; nearest side wins, ties in the fixed
    /// order +x, −x, +z, −z; angles go through the same `atan2`+`round3`
    /// idiom the render plan's `aim` already uses.
    ///
    /// Returns `(eye, look_at, vertical_fov_degrees)`.
    pub fn vista_camera(&self, spawn: Option<[i32; 3]>) -> ([f64; 3], [f64; 3], f64) {
        let (cx, cz) = (
            f64::from(self.scene.min_x + self.scene.max_x) / 2.0 + 0.5,
            f64::from(self.scene.min_z + self.scene.max_z) / 2.0 + 0.5,
        );
        let eye = match spawn {
            Some(p) => [
                f64::from(p[0]) + 0.5,
                f64::from(p[1]) + 1.62, // player eye height over the feet cell
                f64::from(p[2]) + 0.5,
            ],
            None => [cx, f64::from(self.floor_top_y) + 1.0 + 1.62, cz],
        };
        // Nearest scene edge from the eye, fixed tie order.
        let sides: [(f64, f64, f64); 4] = [
            (f64::from(self.scene.max_x) - eye[0], 1.0, 0.0), // +x
            (eye[0] - f64::from(self.scene.min_x), -1.0, 0.0), // −x
            (f64::from(self.scene.max_z) - eye[2], 0.0, 1.0), // +z
            (eye[2] - f64::from(self.scene.min_z), 0.0, -1.0), // −z
        ];
        let mut best = sides[0];
        for side in &sides[1..] {
            if side.0 < best.0 {
                best = *side;
            }
        }
        let (dist_to_edge, ux, uz) = best;
        // The crest line sits gap + slope beyond the scene edge on every
        // side (the band floor guarantees the full rim on every axis).
        let crest_out = (dist_to_edge + GAP_WIDTH + SLOPE_RUN).max(1.0);
        let mid_rim_y = f64::from(self.floor_top_y) + f64::from(self.rim_height) / 2.0;
        let target = [eye[0] + ux * crest_out, mid_rim_y, eye[2] + uz * crest_out];
        // Vertical FOV from the frame requirements (elevations in degrees,
        // positive = up, at the crest-line distance).
        let elev = |y: f64| ((y - eye[1]).atan2(crest_out)).to_degrees();
        let center = elev(mid_rim_y);
        let crest_top = elev(f64::from(self.floor_top_y + self.rim_height));
        let need_down = center - VISTA_FLOOR_ELEV_DEG; // bottom edge ≤ −15°
        let need_up = (crest_top + VISTA_SKY_MARGIN_DEG) - center; // top clears crest
        let fov = (2.0 * need_down.max(need_up)).clamp(30.0, VISTA_MAX_FOV_DEG);
        (eye, target, (fov * 1000.0).round() / 1000.0)
    }
}

// ---------------------------------------------------------------------------
// Tunables (fixed, not DSL-exposed — spec-0026: horizons expose intent
// parameters; the compiler maps them to fixed tables)
// ---------------------------------------------------------------------------

/// Flat gap-floor width between the scene edge and the slope foot (blocks).
const GAP_WIDTH: f64 = 12.0;
/// Radial run of the inner slope from gap edge to crest line (blocks).
const SLOPE_RUN: f64 = 18.0;
/// Domain-warp amplitude on the radial distance, so the ring reads as
/// mountains, not a stadium wall (dossier §2.2).
const WARP_AMP: f64 = 5.0;
const WARP_FREQ: f64 = 0.045;
/// Ridged-multifractal base frequency for the rim relief.
const RIDGE_FREQ: f64 = 0.03;
/// Ridge floor: no rim segment degenerates below 0.3× `rim_height`
/// (dossier §2.2 — the ring never opens into a walkable gap).
const RIDGE_FLOOR: f64 = 0.3;
/// Normalized annulus progress at which the outer decay begins.
const DECAY_START: f64 = 0.5;
/// Fraction of the core height still standing at the outer tile edge.
const OUTER_KEEP: f64 = 0.2;
/// Solid skirt under the lowest surface of each tile band (blocks).
const SKIRT: i32 = 4;
/// Max tile extent (the prefab library's safe template envelope, per axis).
const TILE_XZ: i32 = 48;
const TILE_Y: i32 = 48;
/// Poisson-disk radius for the tree layer, in blocks — the density knob, and
/// the only one. Everything else about the flora layer is geometry.
///
/// Measured sweep on the `hello-room` valley fixture (task #176): canopy
/// coverage over the surround's occupied columns, with the multi-seed sampler
/// in place. Recorded here so the next change to this number starts from the
/// curve instead of re-deriving it.
///
/// | radius | trees | canopy |
/// |--------|-------|--------|
/// | 7.0    |    10 |  14.8% |
/// | 5.0    |    25 |  32.0% |
/// | 4.0    |    47 |  50.1% |
/// | 3.0    |    80 |  61.9% |
/// | 2.5    |   100 |  67.4% |
///
/// 4.0 is a PROPOSAL, not a proof: half-covered, so the crest silhouette still
/// reads and the grove still has gaps to walk through. Density is an aesthetic
/// call and belongs to the owner. What bounds it mechanically is
/// `DW_VALLEY_CLIMB`, and that stays green across this whole range — a denser
/// canopy grows no standable staircase, proven per build by the nav flood over
/// the emitted world rather than assumed here.
const TREE_SPACING: f64 = 4.0;
/// Bridson candidate attempts per active sample.
const POISSON_K: usize = 20;
/// Radial margin outward of the crest line that every cell of a tree's canopy
/// footprint must keep — trees live on the mountains, never where they could
/// shorten the un-climbable inner wall. Checked per canopy cell (exact),
/// not per trunk with a guessed slack.
const TREE_CREST_MARGIN: f64 = 1.0;
/// Understory decor density on grass cells (gap floor stays bare).
const DECOR_DENSITY: f64 = 0.12;
/// Vista frame requirements (task #157 round 3, planner ruling): the frame's
/// bottom edge must reach this elevation (gap floor in frame)…
const VISTA_FLOOR_ELEV_DEG: f64 = -15.0;
/// …its top edge must clear the crest top by this margin of sky…
const VISTA_SKY_MARGIN_DEG: f64 = 8.0;
/// …and the derived vertical FOV never exceeds this.
const VISTA_MAX_FOV_DEG: f64 = 110.0;
/// Per-axis FLOOR on the annulus band width (owner ruling, 2026-08-04): a
/// legal surround always contains a full gap + slope rim on every axis —
/// `ratio` controls spaciousness above this floor, never below it. The
/// valley simply grows to contain any scene shape (hollow-vigil's 94×27
/// short axis gets the floor while the long axis keeps its proportional
/// band; asymmetry is accepted and expected), so the rim profile runs
/// unchanged on every axis and the DW0322/DW0854 proofs hold purely by
/// construction — no degraded geometry, no second proof path.
const ANNULUS_BAND_FLOOR: f64 = GAP_WIDTH + SLOPE_RUN;
/// MC 1.21.11 build range (dossier §3).
const WORLD_MIN_Y: i32 = -64;
const WORLD_MAX_Y: i32 = 319;

// Noise stream salts (one stream seed, salted per concern — the generators'
// convention).
const SALT_WARP: u64 = 61;
const SALT_RIDGE: u64 = 63; // +octave index
const SALT_ROCK_SPECKLE: u64 = 71;
const SALT_ROCK_BAND: u64 = 73;
const SALT_GAP_DAPPLE: u64 = 75;
const SALT_DECOR_GATE: u64 = 151;
const SALT_DECOR_PICK: u64 = 155;
const SALT_TREE_HEIGHT: u64 = 43;
const SALT_TREE_KEEP: u64 = 45;

// ---------------------------------------------------------------------------
// Flora / palette tables (parallel by construction — criterion 6)
// ---------------------------------------------------------------------------

struct FloraTable {
    biome: &'static str,
    log: &'static str,
    leaves: &'static str,
}

fn flora_table(flora: Flora) -> FloraTable {
    match flora {
        Flora::Oak => FloraTable {
            biome: "minecraft:windswept_forest",
            log: "minecraft:oak_log[axis=y]",
            leaves: "minecraft:oak_leaves[persistent=true]",
        },
        Flora::Cherry => FloraTable {
            biome: "minecraft:cherry_grove",
            log: "minecraft:cherry_log[axis=y]",
            leaves: "minecraft:cherry_leaves[persistent=true]",
        },
    }
}

/// Understory decor: two weighted entries, positionally parallel across
/// palettes (the same seed picks the same index in both rows).
fn decor_table(palette: SurroundPalette) -> [(&'static str, f64); 2] {
    match palette {
        SurroundPalette::StoneGrass => [("minecraft:short_grass", 0.7), ("minecraft:fern", 0.3)],
        SurroundPalette::StonePetal => [
            ("minecraft:pink_petals", 0.7),
            ("minecraft:short_grass", 0.3),
        ],
    }
}

/// Bare mountain rock (shared by both palette rows — deliberately: the rows
/// differ only in understory, so the cherry diff stays id-minimal). No gravity
/// blocks: every surround column would otherwise need a settle-proof substrate.
const ROCK: [(&str, f64); 5] = [
    ("minecraft:stone", 0.34),
    ("minecraft:andesite", 0.24),
    ("minecraft:cobblestone", 0.20),
    ("minecraft:tuff", 0.14),
    ("minecraft:mossy_cobblestone", 0.08),
];

/// Cumulative-weight palette pick (the generators' `pick`, verbatim idiom).
fn pick(palette: &[(&'static str, f64)], n: f64) -> &'static str {
    let total: f64 = palette.iter().map(|e| e.1).sum();
    let mut acc = 0.0;
    let target = n.clamp(0.0, 0.999_999) * total;
    for e in palette {
        acc += e.1;
        if target < acc {
            return e.0;
        }
    }
    palette.last().expect("non-empty palette").0
}

// ---------------------------------------------------------------------------
// Geometry: smoothed-rectangle distance + warped radial profile
// ---------------------------------------------------------------------------

fn smoothstep01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Distance in blocks from the scene rectangle (0 inside), with rounded
/// corners (Euclidean over the per-axis excesses) so the ring follows the
/// scene's footprint, not a circle and not a square picture frame.
fn rect_distance(scene: &SceneRect, x: i32, z: i32) -> f64 {
    let dx = (scene.min_x - x).max(0).max(x - scene.max_x) as f64;
    let dz = (scene.min_z - z).max(0).max(z - scene.max_z) as f64;
    (dx * dx + dz * dz).sqrt()
}

/// Half-extents of the annulus per axis: `(ratio − 1)/2 ×` the scene extent,
/// floored at [`ANNULUS_BAND_FLOOR`] (owner ruling, 2026-08-04) so every
/// axis carries the full gap + slope rim regardless of the scene's shape.
fn annulus_sides(scene: &SceneRect, ratio: f64) -> (f64, f64) {
    let w = (scene.max_x - scene.min_x + 1) as f64;
    let d = (scene.max_z - scene.min_z + 1) as f64;
    (
        ((ratio - 1.0) / 2.0 * w).max(ANNULUS_BAND_FLOOR),
        ((ratio - 1.0) / 2.0 * d).max(ANNULUS_BAND_FLOOR),
    )
}

/// The outer rectangle (inclusive) of the annulus.
fn outer_rect(scene: &SceneRect, ratio: f64) -> SceneRect {
    let (sx, sz) = annulus_sides(scene, ratio);
    SceneRect {
        min_x: scene.min_x - sx.round() as i32,
        min_z: scene.min_z - sz.round() as i32,
        max_x: scene.max_x + sx.round() as i32,
        max_z: scene.max_z + sz.round() as i32,
    }
}

fn in_annulus(scene: &SceneRect, ratio: f64, x: i32, z: i32) -> bool {
    let o = outer_rect(scene, ratio);
    let inside_outer = x >= o.min_x && x <= o.max_x && z >= o.min_z && z <= o.max_z;
    let inside_scene = x >= scene.min_x && x <= scene.max_x && z >= scene.min_z && z <= scene.max_z;
    inside_outer && !inside_scene
}

/// Normalized annulus progress 0..~1.41 (0 at the scene edge, 1 at the outer
/// edge mid-side; rectangle corners exceed 1 and simply finish their decay).
fn annulus_progress(scene: &SceneRect, ratio: f64, x: i32, z: i32) -> f64 {
    let (sx, sz) = annulus_sides(scene, ratio);
    let dx = (scene.min_x - x).max(0).max(x - scene.max_x) as f64 / sx.max(1.0);
    let dz = (scene.min_z - z).max(0).max(z - scene.max_z) as f64 / sz.max(1.0);
    (dx * dx + dz * dz).sqrt()
}

/// The domain-warped radial distance the whole profile keys on.
fn warped_distance(seed: u64, scene: &SceneRect, x: i32, z: i32) -> f64 {
    let d = rect_distance(scene, x, z);
    if d <= 0.0 {
        return 0.0; // inside the scene: never warped into the annulus
    }
    let w = 2.0 * value_noise(seed, x, 0, z, WARP_FREQ, SALT_WARP) - 1.0;
    (d + WARP_AMP * w).max(0.0)
}

/// Ridged multifractal in [0, 1] (Musgrave-style ridge composition,
/// ideas-only; dossier §2.1) over the in-house value noise: 4 octaves,
/// lacunarity 2, gain 0.5, octave weight scaled by the previous ridge.
fn ridged(seed: u64, x: i32, z: i32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = RIDGE_FREQ;
    let mut weight = 1.0;
    let mut norm = 0.0;
    for octave in 0..4u64 {
        let n = value_noise(seed, x, 0, z, freq, SALT_RIDGE + octave);
        let mut r = 1.0 - (2.0 * n - 1.0).abs();
        r *= r;
        r *= weight;
        weight = r.clamp(0.0, 1.0);
        sum += r * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm).clamp(0.0, 1.0)
}

/// Column zone within the annulus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    /// Flat walkable floor between scene and slope foot.
    Gap,
    /// The inner slope, gap edge → crest line.
    Inner,
    /// Crest band and outer face (where the trees live).
    Outer,
}

/// Surface height ABOVE the gap floor at a column, plus its zone. Heights are
/// quantized to even steps — the by-construction half of the un-climbability
/// guarantee (see the module doc). A pure function of its arguments (ADR-0006).
fn column_profile(
    seed: u64,
    scene: &SceneRect,
    ratio: f64,
    rim_height: i32,
    x: i32,
    z: i32,
) -> (Zone, i32) {
    let dw = warped_distance(seed, scene, x, z);
    if dw < GAP_WIDTH {
        return (Zone::Gap, 0);
    }
    let zone = if dw <= GAP_WIDTH + SLOPE_RUN {
        Zone::Inner
    } else {
        Zone::Outer
    };
    let s = smoothstep01((dw - GAP_WIDTH) / SLOPE_RUN);
    let r = RIDGE_FLOOR + (1.0 - RIDGE_FLOOR) * ridged(seed, x, z);
    // Outer decay keys on how far BEYOND the crest the column sits, relative
    // to the band remaining past it. A band-floored axis (owner ruling,
    // 2026-08-04) has no room past the crest, so its rim ends full-height at
    // the tile edge — the perimeter is unreachable crest-top, never a
    // walkable shelf over the void.
    let d = rect_distance(scene, x, z);
    let p = annulus_progress(scene, ratio, x, z);
    let band = if p > 1e-9 { d / p } else { f64::MAX };
    let crest = GAP_WIDTH + SLOPE_RUN;
    let decay = if band > crest + 1.0 {
        let o = ((dw - crest) / (band - crest)).clamp(0.0, 1.0);
        1.0 - (1.0 - OUTER_KEEP) * smoothstep01(o)
    } else {
        1.0
    };
    let h = (f64::from(rim_height) * s * r * decay).max(0.0);
    let hq = 2 * (h / 2.0).floor() as i32;
    (zone, hq.max(0))
}

// ---------------------------------------------------------------------------
// Poisson-disk tree scatter (Bridson family, ideas-only, in-house)
// ---------------------------------------------------------------------------

/// Deterministic Bridson-style Poisson-disk sampling over integer columns.
/// No trig: ring candidates come from rejection-sampling the enclosing square
/// (bit-stable across hosts, unlike libm `sin`/`cos`). Iteration order is
/// fully determined by the seeded [`crate::Splitmix64`] stream.
fn poisson_columns(
    seed: u64,
    bounds: &SceneRect,
    r: f64,
    domain: &impl Fn(i32, i32) -> bool,
) -> Vec<(i32, i32)> {
    let cell = r / std::f64::consts::SQRT_2;
    let w = (bounds.max_x - bounds.min_x + 1).max(1);
    let d = (bounds.max_z - bounds.min_z + 1).max(1);
    let gw = (f64::from(w) / cell).ceil() as i32 + 1;
    let gd = (f64::from(d) / cell).ceil() as i32 + 1;
    let gidx = |px: f64, pz: f64| -> usize {
        let gx = ((px - f64::from(bounds.min_x)) / cell) as i32;
        let gz = ((pz - f64::from(bounds.min_z)) / cell) as i32;
        (gx.clamp(0, gw - 1) * gd + gz.clamp(0, gd - 1)) as usize
    };
    let mut grid: Vec<Option<(f64, f64)>> = vec![None; (gw * gd) as usize];
    let mut out: Vec<(i32, i32)> = Vec::new();
    let mut active: Vec<(f64, f64)> = Vec::new();
    let mut rng = Splitmix64::new(seed);
    let unit = |rng: &mut Splitmix64| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;

    let far_enough = |grid: &[Option<(f64, f64)>], px: f64, pz: f64| -> bool {
        let gx = ((px - f64::from(bounds.min_x)) / cell) as i32;
        let gz = ((pz - f64::from(bounds.min_z)) / cell) as i32;
        for nx in (gx - 2).max(0)..=(gx + 2).min(gw - 1) {
            for nz in (gz - 2).max(0)..=(gz + 2).min(gd - 1) {
                if let Some((qx, qz)) = grid[(nx * gd + nz) as usize] {
                    let (ex, ez) = (px - qx, pz - qz);
                    if ex * ex + ez * ez < r * r {
                        return false;
                    }
                }
            }
        }
        true
    };

    // Seeding: EVERY in-domain lattice column, not the first one (task #176).
    //
    // Bridson grows from its active list through jumps in `[r, 2r)`, so a run
    // seeded from ONE point only ever samples the part of the domain reachable
    // by such jumps. Both tree domains here break that assumption in two ways
    // at once: the annulus is a RING (not simply connected — a frontier can
    // never wrap it), and eligibility is SPECKLE (grass noise ∧ a keep hash),
    // so the frontier dies within a few steps. Measured on the fixture valley:
    // the crest pass had 147 eligible columns and returned **0** — its single
    // lattice scan found no in-domain column at all and the sampler exited
    // before its first jump — and the inner-slope pass had 481 eligible and
    // returned **7**. Seven trees in a whole valley is what the owner saw and
    // called a quarry (#176); the density constant was never the cause.
    //
    // Scanning the whole lattice in the same fixed order costs one extra
    // domain evaluation per lattice cell and is exactly as deterministic: the
    // order is positional, `far_enough` still enforces the spacing, and the
    // RNG stream is untouched (seeds consume none of it). Blue-noise character
    // is preserved — these are just more starting points for the same growth.
    let step = r.ceil().max(1.0) as i32;
    for x in (bounds.min_x..=bounds.max_x).step_by(step as usize) {
        for z in (bounds.min_z..=bounds.max_z).step_by(step as usize) {
            if !domain(x, z) {
                continue;
            }
            let p = (f64::from(x), f64::from(z));
            if !far_enough(&grid, p.0, p.1) {
                continue;
            }
            grid[gidx(p.0, p.1)] = Some(p);
            active.push(p);
            out.push((x, z));
        }
    }

    while let Some(&(ax, az)) = active.last() {
        let mut placed = false;
        for _ in 0..POISSON_K {
            // Candidate in the [r, 2r) ring, via square rejection (no trig).
            let (mut ox, mut oz);
            loop {
                ox = (2.0 * unit(&mut rng) - 1.0) * 2.0 * r;
                oz = (2.0 * unit(&mut rng) - 1.0) * 2.0 * r;
                let d2 = ox * ox + oz * oz;
                if d2 >= r * r && d2 < 4.0 * r * r {
                    break;
                }
            }
            let (px, pz) = (ax + ox, az + oz);
            let (cx, cz) = (px.round() as i32, pz.round() as i32);
            if cx < bounds.min_x
                || cx > bounds.max_x
                || cz < bounds.min_z
                || cz > bounds.max_z
                || !domain(cx, cz)
                || !far_enough(&grid, px, pz)
            {
                continue;
            }
            grid[gidx(px, pz)] = Some((px, pz));
            active.push((px, pz));
            out.push((cx, cz));
            placed = true;
            break;
        }
        if !placed {
            active.pop();
        }
    }
    out.sort_unstable();
    out
}

// ---------------------------------------------------------------------------
// Tree template (parameterized species; geometry identical across floras)
// ---------------------------------------------------------------------------

/// Stamp one tree into `cells` (world coords). The shape is the repo's
/// hand-shaped small tree (trunk + compact leaf ball + crown — the greenfield
/// oak, grown one storey taller for mountainsides); only the log/leaf ids come
/// from the flora table, so oak and cherry builds stamp byte-parallel
/// geometry.
/// `(cx, cz)` is the canopy centre — the trunk itself for a straight tree, or
/// the greenfield lean idiom for a slope tree (task #157 round 2: inner-slope
/// trees lean their canopy two columns toward the VALLEY, so no leaf platform
/// ever sits within hop-on/hop-off range of two different upslope terraces —
/// the empirical flood proof remains the authority).
#[allow(clippy::too_many_arguments)]
fn stamp_tree(
    cells: &mut BTreeMap<[i32; 3], &'static str>,
    x: i32,
    base_y: i32,
    z: i32,
    trunk_h: i32,
    cx: i32,
    cz: i32,
    table: &FloraTable,
) {
    let ball_base = base_y + trunk_h - 2;
    let top = base_y + trunk_h - 1;
    for y in base_y..=top {
        cells.insert([x, y, z], table.log);
    }
    for dy in 0..=2 {
        let layer = ball_base + dy;
        let rad = if dy == 2 { 1 } else { 2 };
        for dx in -rad..=rad {
            for dz in -rad..=rad {
                let r2 = dx * dx + dz * dz + (layer - top) * (layer - top);
                if r2 <= 5 {
                    cells
                        .entry([cx + dx, layer, cz + dz])
                        .or_insert(table.leaves);
                }
            }
        }
    }
    cells.entry([cx, ball_base + 3, cz]).or_insert(table.leaves);
}

/// The one-block-per-axis step from an annulus column TOWARD the scene (the
/// downslope/valley direction) — the inner-slope canopy lean.
fn inward_step(scene: &SceneRect, x: i32, z: i32) -> (i32, i32) {
    let ex = i32::from(x > scene.max_x) - i32::from(x < scene.min_x);
    let ez = i32::from(z > scene.max_z) - i32::from(z < scene.min_z);
    (-ex, -ez)
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Generate the valley surround. `floor_top_y` is the world y of the gap
/// floor's top solid block (= the horizon's `walk_ref_y − 1`); `seed` is the
/// campaign-derived stream seed (`stream_seed(campaign_seed,
/// "horizon/valley")`).
///
/// Errors are returned as strings for the caller to wrap in its diagnostic
/// (param-range problems belong to DW0853 at validation; this build-time check
/// is the belt-and-braces re-statement).
pub fn generate_valley(
    seed: u64,
    scene: SceneRect,
    floor_top_y: i32,
    params: &ValleyParams,
) -> Result<ValleySurround, String> {
    if !(2.0..=3.0).contains(&params.ratio) {
        return Err(format!(
            "valley `ratio` {} out of range 2..=3 (validation should have raised DW0853)",
            params.ratio
        ));
    }
    if params.rim_height < 8 {
        return Err(format!(
            "valley `rim_height` {} below the 8-block minimum",
            params.rim_height
        ));
    }
    let tree_top_allowance = 8; // tallest tree: trunk 5 + ball 2 + crown 1
    if floor_top_y - SKIRT < WORLD_MIN_Y
        || floor_top_y + params.rim_height + tree_top_allowance > WORLD_MAX_Y
    {
        return Err(format!(
            "valley surround (floor top {}, rim {}) leaves the −64..320 build range",
            floor_top_y, params.rim_height
        ));
    }
    let flora = flora_table(params.flora);
    let decor = decor_table(params.palette);
    let outer = outer_rect(&scene, params.ratio);

    // --- 1. Heightfield over every annulus column -------------------------
    let mut columns: BTreeMap<(i32, i32), (Zone, i32)> = BTreeMap::new();
    for x in outer.min_x..=outer.max_x {
        for z in outer.min_z..=outer.max_z {
            if !in_annulus(&scene, params.ratio, x, z) {
                continue;
            }
            columns.insert(
                (x, z),
                column_profile(seed, &scene, params.ratio, params.rim_height, x, z),
            );
        }
    }

    // --- 2. Tree layer: Poisson columns on the crest band / outer face ----
    let crest_domain = |x: i32, z: i32| -> bool {
        if !in_annulus(&scene, params.ratio, x, z) {
            return false;
        }
        // The WHOLE canopy footprint (radius 2) must clear the crest line —
        // exact, per-cell, so no fixed margin has to out-guess the domain
        // warp's gradient and no canopy is ever sheared by a clip.
        for dx in -2..=2 {
            for dz in -2..=2 {
                let dw = warped_distance(seed, &scene, x + dx, z + dz);
                if dw <= GAP_WIDTH + SLOPE_RUN + TREE_CREST_MARGIN {
                    return false;
                }
                if !in_annulus(&scene, params.ratio, x + dx, z + dz) {
                    return false;
                }
            }
        }
        // Grass shoulders only (surface material is flora-independent, so the
        // domain — and thus tree POSITIONS — is identical across floras).
        surface_is_grass(seed, &scene, params.ratio, x, z)
    };
    let mut tree_cells: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for (tx, tz) in poisson_columns(seed, &outer, TREE_SPACING, &crest_domain) {
        let (_, h) = columns[&(tx, tz)];
        let base_y = floor_top_y + h + 1;
        let trunk_h = 4 + (hash01(seed, tx, 0, tz, SALT_TREE_HEIGHT) > 0.6) as i32;
        stamp_tree(&mut tree_cells, tx, base_y, tz, trunk_h, tx, tz, &flora);
    }
    // Inner-slope blossom (task #157 round 2: the vista must show the flora
    // between the walk plane and the crest, not just past it). A second
    // Poisson pass over the inner slopes (every band carries the full rim
    // under the band floor), thinned
    // sparse-near-the-floor → denser-upslope, every tree leaning its canopy
    // two columns toward the valley (see `stamp_tree`). Positions derive from
    // geometry + hashes only, so they are identical across floras.
    let inner_seed = crate::edit::mix64(seed ^ 0xB105_50F7);
    let inner_domain = |x: i32, z: i32| -> bool {
        if !in_annulus(&scene, params.ratio, x, z) {
            return false;
        }
        let dw = warped_distance(seed, &scene, x, z);
        if !(GAP_WIDTH + 6.0..=GAP_WIDTH + SLOPE_RUN - 1.0).contains(&dw) {
            return false;
        }
        // Density ramp: sparse near the floor, denser upslope.
        let sl = ((dw - GAP_WIDTH) / SLOPE_RUN).clamp(0.0, 1.0);
        if hash01(seed, x, 9, z, SALT_TREE_KEEP) > 0.30 + 0.60 * sl {
            return false;
        }
        // The leaned canopy footprint must stay on the annulus and off the
        // gap floor (no shear, ever).
        let (ix, iz) = inward_step(&scene, x, z);
        for dx in -2..=2 {
            for dz in -2..=2 {
                let (nx, nz) = (x + 2 * ix + dx, z + 2 * iz + dz);
                if !in_annulus(&scene, params.ratio, nx, nz) {
                    return false;
                }
                if warped_distance(seed, &scene, nx, nz) <= GAP_WIDTH + 1.0 {
                    return false;
                }
            }
        }
        surface_is_grass(seed, &scene, params.ratio, x, z)
    };
    for (tx, tz) in poisson_columns(inner_seed, &outer, TREE_SPACING, &inner_domain) {
        if tree_cells.contains_key(&[tx, floor_top_y + columns[&(tx, tz)].1 + 1, tz]) {
            continue; // a crest tree's trunk already owns this column
        }
        let (_, h) = columns[&(tx, tz)];
        let base_y = floor_top_y + h + 1;
        let trunk_h = 4 + (hash01(seed, tx, 0, tz, SALT_TREE_HEIGHT) > 0.6) as i32;
        let (ix, iz) = inward_step(&scene, tx, tz);
        stamp_tree(
            &mut tree_cells,
            tx,
            base_y,
            tz,
            trunk_h,
            tx + 2 * ix,
            tz + 2 * iz,
            &flora,
        );
    }
    // Clip stray canopy: never over the gap floor or the scene.
    tree_cells.retain(|c, _| {
        in_annulus(&scene, params.ratio, c[0], c[2])
            && columns
                .get(&(c[0], c[2]))
                .is_some_and(|(zone, _)| *zone != Zone::Gap)
    });

    // --- 3. Understory decor — in the GENERATION phase, not the tile
    // slicer, so the un-climbability proof below runs over the finished tile
    // contents and can never diverge from what ships (task #157: the decor
    // used to be stamped after the proof had run, which is exactly how the
    // generator's own proof and the compiler's DW0854 came to disagree).
    let mut decor_cells: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for (&(x, z), &(zone, h)) in &columns {
        if zone == Zone::Gap {
            continue; // gap floor stays bare — the region-margin walk is clean
        }
        let surf = floor_top_y + h;
        if !surface_is_grass(seed, &scene, params.ratio, x, z)
            || tree_cells.contains_key(&[x, surf + 1, z])
        {
            continue;
        }
        if hash01(seed, x, surf + 1, z, SALT_DECOR_GATE) < DECOR_DENSITY {
            let d = pick(
                &decor[..],
                hash01(seed, x, surf + 1, z, SALT_DECOR_PICK).min(0.999_999),
            );
            decor_cells.insert([x, surf + 1, z], d);
        }
    }

    // --- 4. The by-construction un-climbability proof, over EVERYTHING the
    // tiles will contain (terrain + trees + decor, collision-classified) -----
    assert_inner_slopes_unclimbable(
        seed,
        &scene,
        params.ratio,
        floor_top_y,
        &columns,
        &tree_cells,
        &decor_cells,
    );

    // --- 5. Slice into ≤48×48×48 tiles and serialize ----------------------
    let tiles = build_tiles(
        seed,
        &scene,
        params,
        floor_top_y,
        &columns,
        &tree_cells,
        &decor_cells,
    );

    // --- 5. Biome paint rects (one per band, full build-height columns) ---
    let mut biome = Vec::new();
    for band in annulus_bands(&scene, &outer) {
        biome.push(BiomeRect {
            min: [band.min_x, WORLD_MIN_Y, band.min_z],
            max: [band.max_x, WORLD_MAX_Y, band.max_z],
            biome: flora.biome,
        });
    }

    // --- 6. Gap-floor start cells for the compiler-side nav flood ---------
    let gap_floor_starts: Vec<[i32; 3]> = columns
        .iter()
        .filter(|(_, (zone, _))| *zone == Zone::Gap)
        .map(|(&(x, z), _)| [x, floor_top_y + 1, z])
        .collect();

    Ok(ValleySurround {
        tiles,
        biome,
        gap_floor_starts,
        floor_top_y,
        seed,
        scene,
        ratio: params.ratio,
        rim_height: params.rim_height,
    })
}

/// Whether the surface material at a column is grass (vs bare rock). Shared by
/// the surface painter and the tree domain; deliberately flora-independent
/// (identical across floras, so tree positions and surface ids never fork).
fn surface_is_grass(seed: u64, scene: &SceneRect, ratio: f64, x: i32, z: i32) -> bool {
    let dw = warped_distance(seed, scene, x, z);
    if dw < GAP_WIDTH {
        return true;
    }
    if dw <= GAP_WIDTH + SLOPE_RUN {
        // Rockier as the wall climbs: grassy foot, crag top. Softened for the
        // blossom round (#157: the inner slope carries trees now).
        let s = ((dw - GAP_WIDTH) / SLOPE_RUN).clamp(0.0, 1.0);
        value_noise(seed, x, 0, z, 0.11, SALT_ROCK_SPECKLE) > 0.30 + 0.35 * s
    } else {
        // Crest band already tree country (the silhouette the scene sees —
        // for cherry-valley the blossoms must crown the rim), outer face
        // increasingly grassy.
        let p = annulus_progress(scene, ratio, x, z);
        let o = ((p - DECAY_START) / (1.0 - DECAY_START)).clamp(0.0, 1.0);
        value_noise(seed, x, 0, z, 0.11, SALT_ROCK_SPECKLE) > 0.40 - 0.22 * o
    }
}

// ---------------------------------------------------------------------------
// Tiling + NBT
// ---------------------------------------------------------------------------

/// The four rectangular bands (W, E, N, S) whose union is exactly the annulus.
fn annulus_bands(scene: &SceneRect, outer: &SceneRect) -> Vec<SceneRect> {
    let mut bands = Vec::new();
    if outer.min_x < scene.min_x {
        bands.push(SceneRect {
            min_x: outer.min_x,
            max_x: scene.min_x - 1,
            min_z: outer.min_z,
            max_z: outer.max_z,
        });
    }
    if outer.max_x > scene.max_x {
        bands.push(SceneRect {
            min_x: scene.max_x + 1,
            max_x: outer.max_x,
            min_z: outer.min_z,
            max_z: outer.max_z,
        });
    }
    if outer.min_z < scene.min_z {
        bands.push(SceneRect {
            min_x: scene.min_x,
            max_x: scene.max_x,
            min_z: outer.min_z,
            max_z: scene.min_z - 1,
        });
    }
    if outer.max_z > scene.max_z {
        bands.push(SceneRect {
            min_x: scene.min_x,
            max_x: scene.max_x,
            min_z: scene.max_z + 1,
            max_z: outer.max_z,
        });
    }
    bands
}

#[allow(clippy::too_many_arguments)]
fn build_tiles(
    seed: u64,
    scene: &SceneRect,
    params: &ValleyParams,
    floor_top_y: i32,
    columns: &BTreeMap<(i32, i32), (Zone, i32)>,
    tree_cells: &BTreeMap<[i32; 3], &'static str>,
    decor_cells: &BTreeMap<[i32; 3], &'static str>,
) -> Vec<SurroundTile> {
    let outer = outer_rect(scene, params.ratio);
    let mut tiles = Vec::new();
    let mut n = 0usize;
    for band in annulus_bands(scene, &outer) {
        let mut x0 = band.min_x;
        while x0 <= band.max_x {
            let x1 = (x0 + TILE_XZ - 1).min(band.max_x);
            let mut z0 = band.min_z;
            while z0 <= band.max_z {
                let z1 = (z0 + TILE_XZ - 1).min(band.max_z);
                for tile in tile_stack(
                    seed,
                    scene,
                    params,
                    floor_top_y,
                    columns,
                    tree_cells,
                    decor_cells,
                    x0,
                    x1,
                    z0,
                    z1,
                    &mut n,
                ) {
                    tiles.push(tile);
                }
                z0 = z1 + 1;
            }
            x0 = x1 + 1;
        }
    }
    tiles
}

/// Build the (vertically sliced) tile stack for one ≤48×48 footprint.
#[allow(clippy::too_many_arguments)]
fn tile_stack(
    seed: u64,
    scene: &SceneRect,
    params: &ValleyParams,
    floor_top_y: i32,
    columns: &BTreeMap<(i32, i32), (Zone, i32)>,
    tree_cells: &BTreeMap<[i32; 3], &'static str>,
    decor_cells: &BTreeMap<[i32; 3], &'static str>,
    x0: i32,
    x1: i32,
    z0: i32,
    z1: i32,
    n: &mut usize,
) -> Vec<SurroundTile> {
    // Vertical span: solid skirt under the lowest surface, headroom over the
    // tallest surface or tree cell in the footprint.
    let mut min_surf = i32::MAX;
    let mut max_top = i32::MIN;
    for x in x0..=x1 {
        for z in z0..=z1 {
            if let Some(&(_, h)) = columns.get(&(x, z)) {
                min_surf = min_surf.min(floor_top_y + h);
                max_top = max_top.max(floor_top_y + h + 1); // decor headroom
            }
        }
    }
    for (c, _) in tree_cells.range([x0, i32::MIN, z0]..=[x1, i32::MAX, z1]) {
        if c[0] >= x0 && c[0] <= x1 && c[2] >= z0 && c[2] <= z1 {
            max_top = max_top.max(c[1]);
        }
    }
    if min_surf == i32::MAX {
        return Vec::new(); // footprint holds no annulus column
    }
    let y_min = min_surf - SKIRT;
    let y_max = max_top;

    // World-cell content for the whole footprint, then sliced.
    let mut cells: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for x in x0..=x1 {
        for z in z0..=z1 {
            let Some(&(zone, h)) = columns.get(&(x, z)) else {
                continue;
            };
            let surf = floor_top_y + h;
            let grass = surface_is_grass(seed, scene, params.ratio, x, z);
            for y in y_min..=surf {
                let name = if y == surf {
                    match (zone, grass) {
                        (Zone::Gap, _) => {
                            if value_noise(seed, x, y, z, 0.16, SALT_GAP_DAPPLE) > 0.85 {
                                "minecraft:coarse_dirt"
                            } else {
                                "minecraft:grass_block"
                            }
                        }
                        (_, true) => "minecraft:grass_block",
                        (_, false) => pick(&ROCK, value_noise(seed, x, y, z, 0.13, SALT_ROCK_BAND)),
                    }
                } else if grass && surf - y <= 3 {
                    "minecraft:dirt"
                } else {
                    pick(&ROCK, value_noise(seed, x, y, z, 0.13, SALT_ROCK_BAND))
                };
                cells.insert([x, y, z], name);
            }
        }
    }
    // Trees + understory decor were produced in the GENERATION phase (so the
    // un-climbability proof saw them); the slicer only copies its window.
    for source in [tree_cells, decor_cells] {
        for (c, name) in source.range([x0, i32::MIN, z0]..=[x1, i32::MAX, z1]) {
            if c[0] >= x0 && c[0] <= x1 && c[2] >= z0 && c[2] <= z1 {
                cells.insert(*c, name);
            }
        }
    }

    // Slice into ≤48-high tiles.
    let mut out = Vec::new();
    let mut sy = y_min;
    while sy <= y_max {
        let sy1 = (sy + TILE_Y - 1).min(y_max);
        let size = [x1 - x0 + 1, sy1 - sy + 1, z1 - z0 + 1];
        let bytes = serialize_tile(&cells, [x0, sy, z0], size);
        out.push(SurroundTile {
            structure_id: format!("horizon/valley/t{n}"),
            bytes,
            pos: [x0, sy, z0],
            size,
        });
        *n += 1;
        sy = sy1 + 1;
    }
    out
}

// Vanilla structure NBT (the generators' serialization, minus jigsaws).
#[derive(Serialize, Clone, PartialEq)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}
#[derive(Serialize)]
struct BlockEntry {
    pos: [i32; 3],
    state: i32,
}
#[derive(Serialize)]
struct Entity {}
#[derive(Serialize)]
struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<Entity>,
}

/// Split `minecraft:id[k=v,…]` into a palette entry.
fn palette_entry(name: &str) -> PaletteEntry {
    match name.split_once('[') {
        None => PaletteEntry {
            name: name.to_string(),
            properties: None,
        },
        Some((id, props)) => {
            let props = props.trim_end_matches(']');
            let map: BTreeMap<String, String> = props
                .split(',')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            PaletteEntry {
                name: id.to_string(),
                properties: Some(map),
            }
        }
    }
}

/// Serialize one tile volume (air included, generator precedent) to
/// gzip-framed NBT with mtime pinned 0 (ADR-0006).
fn serialize_tile(
    cells: &BTreeMap<[i32; 3], &'static str>,
    pos: [i32; 3],
    size: [i32; 3],
) -> Vec<u8> {
    let mut palette: Vec<PaletteEntry> = vec![PaletteEntry {
        name: "minecraft:air".to_string(),
        properties: None,
    }];
    let mut index: BTreeMap<&'static str, i32> = BTreeMap::new();
    let mut blocks = Vec::new();
    for lx in 0..size[0] {
        for ly in 0..size[1] {
            for lz in 0..size[2] {
                let world = [pos[0] + lx, pos[1] + ly, pos[2] + lz];
                let state = match cells.get(&world) {
                    None => 0,
                    Some(name) => *index.entry(name).or_insert_with(|| {
                        palette.push(palette_entry(name));
                        (palette.len() - 1) as i32
                    }),
                };
                blocks.push(BlockEntry {
                    pos: [lx, ly, lz],
                    state,
                });
            }
        }
    }
    let structure = Structure {
        data_version: crate::DATA_VERSION,
        size,
        palette,
        blocks,
        entities: vec![],
    };
    let nbt = fastnbt::to_bytes(&structure).expect("surround tile NBT");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("surround tile gzip");
    gz.finish().expect("surround tile gzip finish")
}

// ---------------------------------------------------------------------------
// Generator-side un-climbability proof
// ---------------------------------------------------------------------------

/// Generator invariant (debug doctrine — fail generation, never ship): a
/// conservative reachability flood from every gap-floor standable cell, with
/// 1-block step-ups, unlimited drops, and tree solids included, must never
/// reach a column outward of the crest line. The compiler's nav gate over the
/// assembled model is the authoritative proof; this catches the class at its
/// source with a precise message.
fn assert_inner_slopes_unclimbable(
    seed: u64,
    scene: &SceneRect,
    ratio: f64,
    floor_top_y: i32,
    columns: &BTreeMap<(i32, i32), (Zone, i32)>,
    tree_cells: &BTreeMap<[i32; 3], &'static str>,
    decor_cells: &BTreeMap<[i32; 3], &'static str>,
) {
    // Per-column extra solids from everything stamped over the terrain —
    // trees AND decor — classified by the one collision model the assembled
    // world uses (task #157: a no-collision tuft must contribute nothing
    // here, and a future decor palette with real collision must be seen).
    let mut extra: BTreeMap<(i32, i32), BTreeSet<i32>> = BTreeMap::new();
    for (c, name) in tree_cells.iter().chain(decor_cells.iter()) {
        if crate::assembled::is_thin_decoration(name) {
            continue; // empty/sub-step collision: walked through, never stood on
        }
        extra.entry((c[0], c[2])).or_default().insert(c[1]);
    }
    let is_solid = |x: i32, y: i32, z: i32| -> bool {
        if let Some(&(_, h)) = columns.get(&(x, z))
            && y <= floor_top_y + h
        {
            return true;
        }
        extra.get(&(x, z)).is_some_and(|ys| ys.contains(&y))
    };
    // Standable feet positions per column: solid below, two clear above.
    let standables = |x: i32, z: i32| -> Vec<i32> {
        let mut ys = Vec::new();
        let Some(&(_, h)) = columns.get(&(x, z)) else {
            return ys;
        };
        let surf = floor_top_y + h;
        let mut cand: BTreeSet<i32> = BTreeSet::new();
        cand.insert(surf + 1);
        if let Some(tys) = extra.get(&(x, z)) {
            for &ty in tys {
                cand.insert(ty + 1);
            }
        }
        for y in cand {
            if is_solid(x, y - 1, z) && !is_solid(x, y, z) && !is_solid(x, y + 1, z) {
                ys.push(y);
            }
        }
        ys
    };

    let mut frontier: Vec<[i32; 3]> = Vec::new();
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (&(x, z), &(zone, _)) in columns {
        if zone == Zone::Gap {
            for y in standables(x, z) {
                if seen.insert([x, y, z]) {
                    frontier.push([x, y, z]);
                }
            }
        }
    }
    while let Some([x, y, z]) = frontier.pop() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, nz) = (x + dx, z + dz);
            for ny in standables(nx, nz) {
                // step up ≤1, drop unlimited (conservative superset of the
                // nav physics — over-approximation is sound for UNreachability).
                if ny > y + 1 || !seen.insert([nx, ny, nz]) {
                    continue;
                }
                let dw = warped_distance(seed, scene, nx, nz);
                assert!(
                    dw <= GAP_WIDTH + SLOPE_RUN,
                    "valley surround invariant: gap floor reaches beyond the crest line at \
                     [{nx}, {ny}, {nz}] (warped d {dw:.1}) — the inner slope grew a standable \
                     staircase; the even-step quantization or the tree clip regressed \
                     (seed {seed}, scene {scene:?}, ratio {ratio})"
                );
                frontier.push([nx, ny, nz]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> SceneRect {
        SceneRect {
            min_x: 0,
            min_z: 0,
            max_x: 95,
            max_z: 95,
        }
    }

    fn params(flora: Flora, palette: SurroundPalette) -> ValleyParams {
        ValleyParams {
            ratio: 2.5,
            rim_height: 48,
            flora,
            palette,
        }
    }

    #[test]
    fn valley_generation_is_deterministic() {
        let a = generate_valley(7, scene(), 62, &ValleyParams::default()).unwrap();
        let b = generate_valley(7, scene(), 62, &ValleyParams::default()).unwrap();
        assert_eq!(a.tiles.len(), b.tiles.len());
        for (ta, tb) in a.tiles.iter().zip(&b.tiles) {
            assert_eq!(ta.structure_id, tb.structure_id);
            assert_eq!(ta.pos, tb.pos);
            assert_eq!(ta.size, tb.size);
            assert_eq!(ta.bytes, tb.bytes, "tile {} bytes differ", ta.structure_id);
        }
    }

    #[test]
    fn tiles_obey_the_safe_template_envelope_and_cover_the_annulus() {
        let v = generate_valley(11, scene(), 62, &ValleyParams::default()).unwrap();
        assert!(!v.tiles.is_empty());
        let mut footprints: BTreeSet<(i32, i32)> = BTreeSet::new();
        for t in &v.tiles {
            assert!(t.size[0] <= TILE_XZ && t.size[2] <= TILE_XZ, "{:?}", t.size);
            assert!(t.size[1] <= TILE_Y, "{:?}", t.size);
            for x in t.pos[0]..t.pos[0] + t.size[0] {
                for z in t.pos[2]..t.pos[2] + t.size[2] {
                    footprints.insert((x, z));
                }
            }
        }
        // Every annulus column is inside some tile footprint; no tile reaches
        // into the scene.
        let o = outer_rect(&scene(), 2.5);
        for x in o.min_x..=o.max_x {
            for z in o.min_z..=o.max_z {
                if in_annulus(&scene(), 2.5, x, z) {
                    assert!(footprints.contains(&(x, z)), "({x},{z}) uncovered");
                }
            }
        }
        let s = scene();
        for x in s.min_x..=s.max_x {
            for z in s.min_z..=s.max_z {
                assert!(!footprints.contains(&(x, z)), "tile reaches into scene");
            }
        }
    }

    #[test]
    fn no_one_block_riser_exists_and_ridge_never_opens() {
        // The by-construction invariant behind DW0854: no two adjacent
        // annulus columns may ever differ by exactly 1 (the only riser a
        // vanilla step/jump climbs) — every surface height is quantized to
        // even steps.
        let s = scene();
        let heights = |x: i32, z: i32| -> Option<i32> {
            in_annulus(&s, 2.5, x, z).then(|| column_profile(9, &s, 2.5, 48, x, z).1)
        };
        for x in -80..=175 {
            for z in -80..=175 {
                let Some(h) = heights(x, z) else { continue };
                for (nx, nz) in [(x + 1, z), (x, z + 1)] {
                    if let Some(nh) = heights(nx, nz) {
                        assert_ne!(
                            (h - nh).abs(),
                            1,
                            "1-block riser between ({x},{z})={h} and ({nx},{nz})={nh}"
                        );
                    }
                }
            }
        }
        // Along the crest line the rim never degenerates to a walkable gap:
        // sample the ring at the slope end and require a hard minimum.
        let mut min_crest = i32::MAX;
        let o = outer_rect(&s, 2.5);
        for x in o.min_x..=o.max_x {
            for z in o.min_z..=o.max_z {
                if !in_annulus(&s, 2.5, x, z) {
                    continue;
                }
                let dw = warped_distance(9, &s, x, z);
                if (dw - (GAP_WIDTH + SLOPE_RUN)).abs() <= 1.0 {
                    let (_, h) = column_profile(9, &s, 2.5, 48, x, z);
                    min_crest = min_crest.min(h);
                }
            }
        }
        assert!(
            min_crest >= 10,
            "crest degenerates to {min_crest} blocks — the ring opens"
        );
    }

    /// The annulus band floor (owner ruling, 2026-08-04), on hollow-vigil's
    /// proportions (94×27): the short axis's proportional band (20) is under
    /// gap + slope, so it takes the floor (30) and carries the SAME full rim
    /// as every other axis — walkable gap floor, rising slope, crest — and
    /// the whole surround holds the nav-flood proof over its serialized
    /// tiles. `ratio` controls spaciousness above the floor, never below.
    #[test]
    fn short_axis_band_floors_at_the_full_rim() {
        let s = SceneRect {
            min_x: 0,
            min_z: 0,
            max_x: 93,
            max_z: 26,
        };
        let (sx, sz) = annulus_sides(&s, 2.5);
        assert!(
            (sx - 0.75 * 94.0).abs() < 1.0,
            "the long axis keeps its proportional band (got {sx})"
        );
        assert!(
            (sz - ANNULUS_BAND_FLOOR).abs() < f64::EPSILON,
            "the short axis takes the band floor (got {sz})"
        );
        let v = generate_valley(41, s, 62, &ValleyParams::default()).unwrap();
        // Mid-scene column, marching outward across the short (+z) band: the
        // full profile appears — gap floor, then a crest at real height.
        let x = 47;
        let o = outer_rect(&s, 2.5);
        let mut saw_gap_floor = false;
        let mut max_h = 0;
        for z in (s.max_z + 1)..=o.max_z {
            let (zone, h) = column_profile(41, &s, 2.5, 48, x, z);
            if zone == Zone::Gap {
                saw_gap_floor = true;
            }
            max_h = max_h.max(h);
        }
        assert!(saw_gap_floor, "the walkable gap floor must survive");
        assert!(
            max_h >= 10,
            "the short axis must carry a real rim (crest {max_h})"
        );
        // The empirical proof over the finished, serialized surround.
        let world = decoded_world(&v, &[]);
        if let Err(cell) = v.verify_unclimbable(&world) {
            panic!("gap floor escapes the floored-band valley at {cell:?}");
        }
    }

    /// Task #157 round 2: blossom must be IN the player's view — trees stand
    /// on the inner slopes (between walk plane and crest), not only past the
    /// crest.
    #[test]
    fn blossom_reaches_the_inner_slope() {
        let s = scene();
        let v = generate_valley(
            17,
            s,
            62,
            &params(Flora::Cherry, SurroundPalette::StonePetal),
        )
        .unwrap();
        let mut inner_tree_cells = 0usize;
        for t in &v.tiles {
            let d: fastnbt::Value = fastnbt::from_bytes(&gunzip(&t.bytes)).unwrap();
            let (palette, blocks) = palette_and_blocks(&d);
            for (pos, state) in blocks {
                let name = &palette[state as usize];
                if name.contains("cherry_log") || name.contains("cherry_leaves") {
                    let (wx, wz) = (t.pos[0] + pos[0], t.pos[2] + pos[2]);
                    if !v.beyond_crest(wx, wz) {
                        inner_tree_cells += 1;
                    }
                }
            }
        }
        assert!(
            inner_tree_cells >= 60,
            "the inner slopes must carry visible blossom (got {inner_tree_cells} tree cells)"
        );
    }

    /// Task #176 (owner playtest): the cherry valley "reads as a quarry".
    ///
    /// The test above was green throughout and could not have caught it — it
    /// asserts that inner blossom EXISTS, and seven trees in an entire valley
    /// do exist. This binds the QUANTITY instead: canopy coverage over the
    /// columns the surround actually occupies. That is the measurement the
    /// finding asked for, and the only thing that separates a grove from a
    /// quarry with a shrub on it.
    ///
    /// **The scene here is deliberately SMALL, and that is the whole point.**
    /// The defect degrades with scale: Bridson seeded from one point grows by
    /// jumps in `[r, 2r)`, so it covers a wide annulus tolerably and collapses
    /// on a narrow, speckled one. Measured both ways when this was written —
    /// on the 96×96 `scene()` used by the tests above, the single-seed sampler
    /// still produced 1050 trees / 42.9% (the fix lifts that to 1698 / 56.1%),
    /// so a gate written against THAT fixture passes with the bug in place and
    /// proves nothing. On THIS 24×24 fixture the same code gives 25 trees /
    /// 7.5%, against 63 / 16.6% with the fix — and a small scene is what the
    /// owner actually played. The floor sits between the two measurements.
    ///
    /// The ratio is deliberately reported against ALL surround columns, which
    /// includes the outer face no tree may stand on, so it is a comparable
    /// number for one fixture rather than a claim about how leafy a valley
    /// looks. Absolute density is `TREE_SPACING`, and that is a design knob.
    ///
    /// A gate must bind where the failure lives, not where the numbers are
    /// comfortable. This one WAS first written against the large scene, was
    /// run with the defect deliberately restored, passed, and had to be moved.
    #[test]
    fn canopy_covers_the_valley_rather_than_speckling_it() {
        // Small scene: the narrow annulus is where single-seed sampling dies.
        let small = SceneRect {
            min_x: 0,
            min_z: 0,
            max_x: 23,
            max_z: 23,
        };
        let v = generate_valley(
            17,
            small,
            62,
            &params(Flora::Cherry, SurroundPalette::StonePetal),
        )
        .unwrap();

        let mut leaf_columns: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut solid_columns: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut trunk_columns: BTreeSet<(i32, i32)> = BTreeSet::new();
        for t in &v.tiles {
            let d: fastnbt::Value = fastnbt::from_bytes(&gunzip(&t.bytes)).unwrap();
            let (palette, blocks) = palette_and_blocks(&d);
            for (pos, state) in blocks {
                let name = &palette[state as usize];
                let col = (t.pos[0] + pos[0], t.pos[2] + pos[2]);
                if name != "minecraft:air" {
                    solid_columns.insert(col);
                }
                if name.contains("leaves") {
                    leaf_columns.insert(col);
                }
                if name.contains("_log") {
                    trunk_columns.insert(col);
                }
            }
        }

        // State the binding count. A coverage ratio over zero columns is not a
        // pass — it is a surround that failed to generate at all.
        assert!(
            solid_columns.len() > 500,
            "the surround itself is missing — only {} solid columns",
            solid_columns.len()
        );

        let pct = 100.0 * leaf_columns.len() as f64 / solid_columns.len() as f64;
        assert!(
            pct >= 12.0,
            "canopy covers only {pct:.1}% of {} surround columns, from {} trees — \
             the valley reads as a quarry (task #176). A collapse of this size is \
             the tree SAMPLER failing to reach its own domain, not a density knob: \
             check that `poisson_columns` still seeds from EVERY in-domain lattice \
             column, because one seed point cannot cover a ring",
            solid_columns.len(),
            trunk_columns.len()
        );
    }

    #[test]
    fn cherry_differs_from_oak_only_in_flora_block_and_biome_ids() {
        let oak = generate_valley(
            13,
            scene(),
            62,
            &params(Flora::Oak, SurroundPalette::StoneGrass),
        )
        .unwrap();
        let cherry = generate_valley(
            13,
            scene(),
            62,
            &params(Flora::Cherry, SurroundPalette::StonePetal),
        )
        .unwrap();
        assert_eq!(oak.tiles.len(), cherry.tiles.len());
        // The flora id map (spec-0026 criterion 6): applying it to the cherry
        // emission must reproduce the oak emission byte-for-byte.
        let map = |name: &str| -> String {
            name.replace("minecraft:cherry_log", "minecraft:oak_log")
                .replace("minecraft:cherry_leaves", "minecraft:oak_leaves")
                .replace("minecraft:pink_petals", "minecraft:short_grass")
                // stone-petal decor slot 2 is short_grass where stone-grass
                // has fern — positional palette parity, asserted structurally
                // below instead of by id map.
                .to_string()
        };
        for (a, b) in oak.tiles.iter().zip(&cherry.tiles) {
            assert_eq!(a.structure_id, b.structure_id);
            assert_eq!(a.pos, b.pos);
            assert_eq!(a.size, b.size);
            // Decode both palettes; positions/states must be identical, and
            // palette names must agree after the flora map (modulo the decor
            // slot pair).
            let da: fastnbt::Value = fastnbt::from_bytes(&gunzip(&a.bytes)).unwrap();
            let db: fastnbt::Value = fastnbt::from_bytes(&gunzip(&b.bytes)).unwrap();
            let (pa, ba) = palette_and_blocks(&da);
            let (pb, bb) = palette_and_blocks(&db);
            assert_eq!(
                ba, bb,
                "block positions/states differ in {}",
                a.structure_id
            );
            assert_eq!(pa.len(), pb.len(), "palette lengths differ");
            for (na, nb) in pa.iter().zip(&pb) {
                let na_m = map(na);
                let nb_m = map(nb);
                assert!(
                    na_m == nb_m
                        || (na == "minecraft:fern" && nb_m == "minecraft:short_grass")
                        || (na_m == "minecraft:short_grass" && nb == "minecraft:fern"),
                    "palette mismatch beyond the flora map: `{na}` vs `{nb}`"
                );
            }
        }
        // Biome rects: identical geometry, only the biome id differs.
        assert_eq!(oak.biome.len(), cherry.biome.len());
        for (a, b) in oak.biome.iter().zip(&cherry.biome) {
            assert_eq!(a.min, b.min);
            assert_eq!(a.max, b.max);
            assert_eq!(a.biome, "minecraft:windswept_forest");
            assert_eq!(b.biome, "minecraft:cherry_grove");
        }
    }

    #[test]
    fn trees_stay_on_the_mountains() {
        // 树在山上: slopes and crest — NEVER the gap floor, never the scene
        // (inner-slope trees are legal since the blossom round, task #157).
        let v = generate_valley(17, scene(), 62, &ValleyParams::default()).unwrap();
        for t in &v.tiles {
            let d: fastnbt::Value = fastnbt::from_bytes(&gunzip(&t.bytes)).unwrap();
            let (palette, blocks) = palette_and_blocks(&d);
            for (pos, state) in blocks {
                let name = &palette[state as usize];
                if name.contains("_log") || name.contains("_leaves") {
                    let wx = t.pos[0] + pos[0];
                    let wz = t.pos[2] + pos[2];
                    assert!(v.in_annulus(wx, wz), "tree cell outside annulus");
                    let (zone, _) = column_profile(17, &scene(), 2.5, 48, wx, wz);
                    assert!(
                        zone != Zone::Gap,
                        "tree cell at ({wx},{wz}) hangs over the gap floor"
                    );
                }
            }
        }
    }

    #[test]
    fn nav_flood_from_gap_floor_never_crosses_the_crest() {
        // The compiler-side statement of the spec §5 proof, over the
        // SERIALIZED tiles (decode → solid set → nav world → flood).
        let s = SceneRect {
            min_x: 0,
            min_z: 0,
            max_x: 47,
            max_z: 47,
        };
        let v = generate_valley(31, s, 62, &ValleyParams::default()).unwrap();
        // EVERY decoded cell enters the model — decor included — and the one
        // real collision classifier (`occupancy_of`) decides what is ground
        // (task #157 regression: a manual "skip the tufts" list here once
        // masked the phantom-standable-plant defect; never pre-filter again).
        let world = decoded_world(&v, &[]);
        if let Err(cell) = v.verify_unclimbable(&world) {
            panic!("gap floor escapes the valley at {cell:?}");
        }
    }

    /// Decode every serialized tile into a blocks map (plus optional doctored
    /// extra cells), classify it with the REAL occupancy model, and wrap it as
    /// a nav world — the exact path the compiler's DW0854 gate takes.
    fn decoded_world(v: &ValleySurround, extra: &[([i32; 3], &str)]) -> crate::nav::World {
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for t in &v.tiles {
            let d: fastnbt::Value = fastnbt::from_bytes(&gunzip(&t.bytes)).unwrap();
            let (palette, cells) = palette_and_blocks(&d);
            for (pos, state) in cells {
                blocks.insert(
                    [t.pos[0] + pos[0], t.pos[1] + pos[1], t.pos[2] + pos[2]],
                    palette[state as usize].clone(),
                );
            }
        }
        for (c, name) in extra {
            if *name == "minecraft:air" {
                blocks.remove(c); // a doctored carve — the edit-batch shape
            } else {
                blocks.insert(*c, name.to_string());
            }
        }
        let occ = crate::assembled::occupancy_of(blocks, &BTreeSet::new());
        crate::nav::World::from_occupancy(occ)
    }

    /// `DW0854` (spec-0026 §5): carving a 1-block staircase up the inner slope
    /// is caught by the empirical nav flood — the check reads assembled
    /// geometry, so a post-generation change (an edit batch, a settle) cannot
    /// sneak a climbable wall past the even-step construction.
    #[test]
    fn a_carved_staircase_up_the_inner_slope_is_dw0369() {
        assert_eq!(DW_VALLEY_CLIMB.id(), "DW0854");
        let s = SceneRect {
            min_x: 0,
            min_z: 0,
            max_x: 47,
            max_z: 47,
        };
        let v = generate_valley(31, s, 62, &ValleyParams::default()).unwrap();
        // The saboteur: a staircase CARVED through the rim — stone steps with
        // air headroom, climbing +1 per column from the gap floor out past
        // the crest (exactly what a careless stage-7 edit batch could do).
        let cz = 24;
        let mut stair: Vec<([i32; 3], &str)> = Vec::new();
        for i in 0..40 {
            stair.push(([48 + i, 62 + i, cz], "minecraft:stone"));
            for dy in 1..=3 {
                stair.push(([48 + i, 62 + i + dy, cz], "minecraft:air"));
            }
        }
        let world = decoded_world(&v, &stair);
        let err = v.verify_unclimbable(&world);
        assert!(
            err.is_err(),
            "the carved staircase must be caught (would ship as {DW_VALLEY_CLIMB})"
        );
        let cell = err.unwrap_err();
        assert!(
            v.beyond_crest(cell[0], cell[2]),
            "the reported cell {cell:?} must lie outward of the crest line"
        );
    }

    /// Task #157 round 3: the vista's derived vertical FOV frames floor AND
    /// crest-top on the hollow-vigil proportions — asserted arithmetically
    /// (the planner's measured failure: a spawn eye 40 blocks from a 48-high
    /// rim subtends ~52°; no fixed FOV frames floor + crest + sky).
    #[test]
    fn vista_fov_frames_floor_and_crest_on_hollow_proportions() {
        let s = SceneRect {
            min_x: 0,
            min_z: 0,
            max_x: 93,
            max_z: 26,
        };
        let v = generate_valley(41, s, 62, &ValleyParams::default()).unwrap();
        let (eye, look, fov) = v.vista_camera(Some([5, 64, 5]));
        assert!(fov <= VISTA_MAX_FOV_DEG, "fov {fov} over the clamp");
        // Aim is MID-rim on the nearest crest line.
        assert_eq!(look[1], 62.0 + 24.0, "aim must be mid-rim");
        let (dx, dz) = (look[0] - eye[0], look[2] - eye[2]);
        let dist = (dx * dx + dz * dz).sqrt();
        let elev = |y: f64| ((y - eye[1]).atan2(dist)).to_degrees();
        let center = elev(look[1]);
        let crest_top = elev(62.0 + 48.0);
        assert!(
            center - fov / 2.0 <= VISTA_FLOOR_ELEV_DEG,
            "frame bottom {:.1}° misses the gap floor",
            center - fov / 2.0
        );
        assert!(
            center + fov / 2.0 >= crest_top + VISTA_SKY_MARGIN_DEG,
            "frame top {:.1}° misses crest-top {crest_top:.1}° + sky margin",
            center + fov / 2.0
        );
    }

    /// Task #157 round 3 (planner ruling): the scene-rect void moat — columns
    /// with zero piece-authored blocks receive gap-floor ground; authored
    /// columns are untouched; the surface dapple continues the gap floor's
    /// own pattern; byte-deterministic.
    #[test]
    fn the_moat_fills_only_unauthored_scene_columns() {
        let s = SceneRect {
            min_x: 0,
            min_z: 0,
            max_x: 47,
            max_z: 47,
        };
        let v = generate_valley(31, s, 62, &ValleyParams::default()).unwrap();
        // An L-shaped authored blob inside the rect.
        let mut authored: BTreeSet<(i32, i32)> = BTreeSet::new();
        for x in 0..=47 {
            for z in 0..=47 {
                if x <= 30 || z <= 20 {
                    authored.insert((x, z));
                }
            }
        }
        let (tiles, starts) = v.moat(&authored);
        let expected_cols = (47 - 30) * (47 - 20); // 17 × 27 unauthored columns
        assert_eq!(starts.len(), expected_cols as usize);
        let mut filled: BTreeSet<(i32, i32)> = BTreeSet::new();
        for t in &tiles {
            let d: fastnbt::Value = fastnbt::from_bytes(&gunzip(&t.bytes)).unwrap();
            let (palette, blocks) = palette_and_blocks(&d);
            for (pos, state) in blocks {
                let (wx, wy, wz) = (t.pos[0] + pos[0], t.pos[1] + pos[1], t.pos[2] + pos[2]);
                assert!(
                    !authored.contains(&(wx, wz)),
                    "moat writes into an authored column ({wx},{wz})"
                );
                assert!((0..=47).contains(&wx) && (0..=47).contains(&wz));
                assert!((58..=62).contains(&wy), "moat cell at y {wy}");
                if wy == 62 {
                    let name = &palette[state as usize];
                    assert!(
                        name == "minecraft:grass_block" || name == "minecraft:coarse_dirt",
                        "moat surface must be the gap-floor treatment, got {name}"
                    );
                    filled.insert((wx, wz));
                }
            }
        }
        assert_eq!(
            filled.len(),
            expected_cols as usize,
            "every column surfaced"
        );
        // Deterministic: a second derivation is byte-identical.
        let (tiles2, _) = v.moat(&authored);
        for (a, b) in tiles.iter().zip(&tiles2) {
            assert_eq!(a.bytes, b.bytes);
        }
    }

    #[test]
    fn poisson_spacing_is_respected() {
        let s = scene();
        let all = |_: i32, _: i32| true;
        let pts = poisson_columns(23, &s, TREE_SPACING, &all);
        assert!(pts.len() > 50, "sampler under-fills ({} pts)", pts.len());
        for (i, &(ax, az)) in pts.iter().enumerate() {
            for &(bx, bz) in &pts[i + 1..] {
                let d2 = (ax - bx).pow(2) + (az - bz).pow(2);
                // Rounding to columns can shave a fraction off the true float
                // distance; allow 1 block of slack under r².
                assert!(
                    d2 as f64 >= (TREE_SPACING - 1.0) * (TREE_SPACING - 1.0),
                    "columns ({ax},{az}) and ({bx},{bz}) too close (d² {d2})"
                );
            }
        }
    }

    #[test]
    fn out_of_range_params_are_refused() {
        assert!(
            generate_valley(
                1,
                scene(),
                62,
                &ValleyParams {
                    ratio: 3.5,
                    ..ValleyParams::default()
                }
            )
            .is_err()
        );
        // Rim through the build ceiling.
        assert!(
            generate_valley(
                1,
                scene(),
                290,
                &ValleyParams {
                    rim_height: 48,
                    ..ValleyParams::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn all_blocks_inside_build_range_and_no_gravity_blocks() {
        let v = generate_valley(29, scene(), 62, &ValleyParams::default()).unwrap();
        for t in &v.tiles {
            assert!(t.pos[1] >= WORLD_MIN_Y);
            assert!(t.pos[1] + t.size[1] - 1 <= WORLD_MAX_Y);
            let d: fastnbt::Value = fastnbt::from_bytes(&gunzip(&t.bytes)).unwrap();
            let (palette, _) = palette_and_blocks(&d);
            for name in &palette {
                assert!(
                    !matches!(name.as_str(), n if n.contains("gravel") || n.contains("sand")),
                    "gravity block `{name}` in a surround palette (would need a settle proof)"
                );
            }
        }
    }

    // -- test helpers -------------------------------------------------------

    fn gunzip(bytes: &[u8]) -> Vec<u8> {
        use std::io::Read as _;
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .expect("gunzip");
        out
    }

    /// Extract (palette names, non-air blocks as (pos, state)) from a decoded
    /// structure value.
    fn palette_and_blocks(v: &fastnbt::Value) -> (Vec<String>, Vec<([i32; 3], i32)>) {
        let fastnbt::Value::Compound(root) = v else {
            panic!("structure root")
        };
        let fastnbt::Value::List(pal) = &root["palette"] else {
            panic!("palette")
        };
        let names: Vec<String> = pal
            .iter()
            .map(|e| {
                let fastnbt::Value::Compound(c) = e else {
                    panic!("palette entry")
                };
                let fastnbt::Value::String(s) = &c["Name"] else {
                    panic!("palette name")
                };
                s.clone()
            })
            .collect();
        let fastnbt::Value::List(blocks) = &root["blocks"] else {
            panic!("blocks")
        };
        let mut out = Vec::new();
        for b in blocks {
            let fastnbt::Value::Compound(c) = b else {
                panic!("block entry")
            };
            let fastnbt::Value::Int(state) = c["state"] else {
                panic!("state")
            };
            if state == 0 {
                continue; // air
            }
            let fastnbt::Value::List(p) = &c["pos"] else {
                panic!("pos")
            };
            let pos: Vec<i32> = p
                .iter()
                .map(|x| {
                    let fastnbt::Value::Int(i) = x else {
                        panic!("pos int")
                    };
                    *i
                })
                .collect();
            out.push(([pos[0], pos[1], pos[2]], state));
        }
        (names, out)
    }
}
