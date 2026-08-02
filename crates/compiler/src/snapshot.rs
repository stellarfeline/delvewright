//! `delvec snapshot` — the visual authoring loop's viewport (spec-0015 pillars 1+2).
//!
//! A voxel raycaster over the shared assembled-world model
//! ([`crate::assembled`]), so the designing agent can **look at its own build**
//! from any camera it chooses, mid-authoring, in well under a second — the
//! screenshot half of the dual-channel loop. The other half is the scene
//! manifest ([`manifest`]): the same frame described structurally, so review
//! feedback and edits reference ids and screen boxes instead of prose.
//!
//! ## What it is (and is not)
//!
//! This is a **draft** renderer, deliberately: flat block-palette colours, face
//! shading, block-edge relief, distance fade. It answers "is the thing where I
//! think it is, is it visible, is it framed" — layout questions — in one process
//! with no GPU, no resource pack and no server. It does **not** answer "does it
//! look good": Chunky (`delve-render`, spec-0007) remains the beauty pass, and
//! nothing here competes with it.
//!
//! Three consequences worth knowing before reading a frame:
//!
//! 1. **There is no lighting model.** The raycaster sees geometry regardless of
//!    block light, so a pitch-black cavern renders exactly as legibly as a lit
//!    meadow. That is the point for dark-area review — the owner's round-3/4
//!    "an entire invisible cavern" finding is a *rendering* artifact of the
//!    beauty tier, not a geometry defect, and this tier separates the two.
//!    Emissive blocks (glowstone, lantern, campfire, torch) still render at full
//!    brightness so a fire pit reads as a fire pit.
//! 2. **Only the assembled model exists.** Placed prefabs, solver seals, gate
//!    clears, gravity settling — everything [`crate::assembled`] models, and
//!    nothing else. Entities (NPC mannequins, actors, item displays) are not
//!    blocks and so are not drawn; their *posts* are in the manifest and, with
//!    `--labels`, stamped on the frame. The one world-generation element drawn
//!    is the `ocean` horizon's sea plane (see [`SEA_PLANE_NOTE`]).
//! 3. **Screen space is the vocabulary.** Every manifest entry carries a
//!    screen-space bbox in pixels, so "the shelf is behind the pillar" becomes
//!    `{"id": "anchor/shelf", "occluded": true}` and "raise it" becomes a
//!    coordinate edit.
//!
//! ## Camera convention
//!
//! `yaw`/`pitch` are **Minecraft** degrees, identical to the cutscene aim
//! ([`crate::emit`]'s `mc_aim`): `yaw = 0` faces +Z (south), `90` faces −X
//! (west), `180` north, `270` east; `pitch` is positive looking **down**, `0`
//! level. `fov` is the **vertical** field of view in degrees.
//!
//! Note this is NOT the `render-plan.json` convention (`crate::render_plan` uses
//! `yaw = atan2(-dz, dx)`, `0` = +X, for Chunky). `--shot` bridges them by
//! reading the shot's `pos`/`look_at` — plain world coordinates both conventions
//! agree on — and re-deriving Minecraft yaw/pitch from them.
//!
//! ## Determinism (ADR-0006)
//!
//! No RNG, no clock, no parallelism, no hash-order iteration: the voxel palette
//! is built from a `BTreeMap` walk, targets are sorted by `(kind, area, id)`,
//! floats in the manifest are rounded to 3 decimals, and the PNG encoder
//! ([`crate::png`]) pins its DEFLATE level. Two runs on one input are
//! byte-identical in both the PNG and the manifest.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::plan::{Plan, ResolvedAnchor};
use crate::raster::{Canvas, GLYPH_ROWS, LabelPlacer, ScreenBox, kind_color, text_width};

/// Default frame size — 16:9, big enough to read a label, small enough to render
/// in a fraction of a second.
pub const DEFAULT_WIDTH: u32 = 960;
/// See [`DEFAULT_WIDTH`].
pub const DEFAULT_HEIGHT: u32 = 540;
/// Default vertical field of view in degrees (vanilla's default is ~70).
pub const DEFAULT_FOV: f64 = 70.0;
/// How far a primary ray travels before it is treated as a miss (blocks). The
/// largest campaign layouts span a few hundred blocks, so this reaches the far
/// side of any of them while bounding the worst-case DDA step count.
pub const MAX_RAY_DISTANCE: f64 = 512.0;
/// Default `--dist` for `--at`/`--orbit` framing (blocks from the subject).
pub const DEFAULT_ORBIT_DIST: f64 = 14.0;

/// Why a sea plane appears in an `ocean`-horizon frame even though water is not
/// in the assembled block model: the shipped delve's superflat generator pins
/// bedrock/stone/water so the sea tops out at [`crate::plan::SEA_LEVEL`]
/// (spec-0013). Drawing it makes the beach read like the delve a player joins
/// instead of an island floating over void — without it, every shoreline frame
/// lies about the one thing shoreline framing is for. It is an analytic plane in
/// the background pass only: it never enters the voxel model, never occludes a
/// manifest target, and is skipped entirely for `void`-horizon campaigns.
pub const SEA_PLANE_NOTE: &str = "ocean-horizon sea plane drawn at world-gen sea level";

// ---------------------------------------------------------------------------
// Block → colour
// ---------------------------------------------------------------------------

/// The colour an unrecognised block renders as: full magenta, the same
/// missing-texture key `delve-render`'s fidelity gate scans for
/// (`crate::…`/`delvewright_render::detect`). Deliberately loud — a block the
/// palette has never seen must be *obvious* in the frame rather than quietly
/// shaded as generic stone, so extending the palette is prompted by looking at a
/// render instead of by reading source.
pub const FALLBACK_COLOR: [u8; 3] = [255, 0, 255];

/// Flat draft colour for a block id, plus whether it renders emissive (drawn at
/// full brightness, ignoring face shading and distance fade).
///
/// Resolution order, first match wins:
/// 1. exact vanilla id (the curated table — every block the shipped prefab
///    library uses is covered, see `every_library_block_has_a_colour`);
/// 2. material *family* substrings (`_planks`, `_wool`, `_concrete`, …), so an
///    unseen variant of a known material still shades plausibly;
/// 3. [`FALLBACK_COLOR`].
pub fn block_color(name: &str) -> ([u8; 3], bool) {
    let id = name.strip_prefix("minecraft:").unwrap_or(name);
    // 1. Exact ids.
    if let Some(c) = exact_color(id) {
        return (c, is_emissive(id));
    }
    // 2. Family fallbacks (ordered: the most specific suffix first).
    let fam: &[(&str, [u8; 3])] = &[
        ("_concrete_powder", [190, 165, 120]),
        ("_concrete", [125, 125, 135]),
        ("_terracotta", [160, 110, 90]),
        ("_glazed", [200, 200, 205]),
        ("_stained_glass", [160, 200, 215]),
        ("_glass", [180, 215, 225]),
        ("_wool", [200, 200, 200]),
        ("_carpet", [190, 190, 190]),
        ("_planks", [162, 130, 78]),
        ("_log", [104, 83, 50]),
        ("_wood", [104, 83, 50]),
        ("_leaves", [64, 110, 46]),
        ("_sapling", [72, 122, 52]),
        ("_stairs", [128, 128, 128]),
        ("_slab", [128, 128, 128]),
        ("_wall", [122, 122, 122]),
        ("_fence_gate", [150, 120, 72]),
        ("_fence", [150, 120, 72]),
        ("_door", [150, 120, 72]),
        ("_trapdoor", [150, 120, 72]),
        ("_button", [140, 112, 68]),
        ("_pressure_plate", [140, 112, 68]),
        ("_sign", [150, 120, 72]),
        ("_banner", [200, 200, 200]),
        ("_bed", [170, 60, 60]),
        ("_shulker_box", [140, 106, 140]),
        ("_ore", [120, 120, 122]),
        ("_bricks", [130, 130, 130]),
        ("_brick", [130, 130, 130]),
        ("stone", [126, 126, 126]),
        ("dirt", [122, 88, 60]),
        ("sand", [214, 203, 152]),
        ("grass", [104, 152, 66]),
        ("water", [58, 96, 178]),
        ("lava", [214, 106, 30]),
        ("ice", [160, 200, 230]),
        ("snow", [238, 244, 248]),
        ("copper", [176, 110, 80]),
        ("iron", [190, 190, 190]),
        ("gold", [220, 186, 84]),
        ("amethyst", [150, 110, 200]),
        ("coral", [200, 90, 140]),
        ("mushroom", [180, 140, 120]),
        ("candle", [220, 210, 180]),
    ];
    for (suffix, c) in fam {
        if id.contains(suffix) {
            return (*c, is_emissive(id));
        }
    }
    // 3. Unknown.
    (FALLBACK_COLOR, false)
}

/// Whether a block renders emissive. One rule for both resolution branches: an
/// id is emissive if it *contains* an [`EMISSIVE`] stem, which catches the
/// placement variants vanilla spells separately (`wall_torch`,
/// `soul_wall_torch`, `redstone_wall_torch`) without listing each.
///
/// `torchflower` (a plant) is the one id the substring rule would over-match, so
/// it is excluded by name.
fn is_emissive(id: &str) -> bool {
    !id.starts_with("torchflower") && EMISSIVE.iter().any(|e| id.contains(e))
}

/// Stems of blocks drawn at full brightness (see [`is_emissive`]). Kept small
/// and literal: only light *sources* a designer uses to read a dark room belong
/// here.
const EMISSIVE: &[&str] = &[
    "glowstone",
    "lantern",
    "soul_lantern",
    "torch",
    "soul_torch",
    "campfire",
    "soul_campfire",
    "sea_lantern",
    "shroomlight",
    "glow_lichen",
    "magma_block",
    "jack_o_lantern",
    "redstone_lamp",
    "ochre_froglight",
    "verdant_froglight",
    "pearlescent_froglight",
    "lava",
    "fire",
    "beacon",
    "end_rod",
    "light",
];

/// The curated exact-id colour table. Every block id the shipped prefab library
/// places is here (guarded by a test); ids beyond it fall through to the family
/// rules in [`block_color`].
#[rustfmt::skip]
fn exact_color(id: &str) -> Option<[u8; 3]> {
    Some(match id {
        // --- terrain / rock -------------------------------------------------
        "stone" => [126, 126, 126],
        "andesite" => [136, 136, 136],
        "polished_andesite" => [150, 150, 150],
        "diorite" => [188, 188, 188],
        "polished_diorite" => [199, 199, 199],
        "granite" => [154, 106, 88],
        "polished_granite" => [166, 116, 96],
        "tuff" => [108, 109, 102],
        "deepslate" => [80, 80, 84],
        "calcite" => [224, 224, 218],
        "dripstone_block" => [140, 108, 92],
        "pointed_dripstone" => [150, 118, 100],
        "cobblestone" => [122, 122, 122],
        "mossy_cobblestone" => [104, 122, 92],
        "stone_bricks" => [122, 122, 122],
        "mossy_stone_bricks" => [104, 120, 96],
        "cracked_stone_bricks" => [116, 116, 114],
        "chiseled_stone_bricks" => [130, 130, 128],
        "gravel" => [136, 130, 127],
        "suspicious_gravel" => [142, 136, 132],
        "sand" => [214, 203, 152],
        "red_sand" => [190, 102, 33],
        "clay" => [160, 166, 179],
        "obsidian" => [20, 16, 30],
        "basalt" => [80, 78, 82],
        "blackstone" => [42, 36, 42],
        // --- soils / plants -------------------------------------------------
        "dirt" => [122, 88, 60],
        "coarse_dirt" => [118, 86, 58],
        "rooted_dirt" => [144, 108, 82],
        "podzol" => [92, 66, 32],
        "grass_block" => [104, 152, 66],
        "moss_block" => [92, 128, 56],
        "short_grass" => [110, 160, 70],
        "tall_grass" => [110, 160, 70],
        "fern" => [104, 150, 68],
        "large_fern" => [104, 150, 68],
        "seagrass" => [70, 140, 80],
        "vine" => [76, 122, 50],
        "glow_lichen" => [130, 168, 130],
        "dead_bush" => [136, 106, 56],
        "hay_block" => [200, 172, 46],
        "poppy" => [200, 60, 56],
        "dandelion" => [230, 208, 60],
        "cornflower" => [86, 110, 210],
        "oxeye_daisy" => [232, 236, 226],
        "oak_leaves" => [64, 110, 46],
        "spruce_leaves" => [50, 88, 50],
        // --- wood -----------------------------------------------------------
        "oak_log" => [104, 83, 50],
        "spruce_log" => [82, 60, 34],
        "stripped_oak_log" => [172, 138, 84],
        "stripped_spruce_log" => [140, 106, 66],
        "oak_planks" => [162, 130, 78],
        "spruce_planks" => [114, 84, 48],
        "dark_oak_planks" => [66, 43, 20],
        // --- fixtures / furniture -------------------------------------------
        "barrel" => [136, 104, 60],
        "chest" => [162, 121, 55],
        "cartography_table" => [124, 100, 72],
        "decorated_pot" => [178, 110, 84],
        "ladder" => [150, 120, 72],
        "chain" => [70, 74, 84],
        "iron_bars" => [150, 152, 156],
        "glass_pane" => [180, 215, 225],
        "white_glazed_terracotta" => [222, 226, 226],
        "emerald_block" => [42, 202, 108],
        "white_wool" => [232, 232, 232],
        "light_gray_wool" => [156, 158, 158],
        "black_wool" => [26, 26, 30],
        "white_banner" => [230, 230, 230],
        // --- light sources ---------------------------------------------------
        "glowstone" => [252, 226, 154],
        "lantern" => [250, 214, 130],
        "soul_lantern" => [140, 220, 226],
        "torch" => [252, 214, 122],
        "wall_torch" => [252, 214, 122],
        "campfire" => [242, 156, 60],
        "soul_campfire" => [110, 214, 226],
        // --- fluids / structure ------------------------------------------------
        "water" => [58, 96, 178],
        "lava" => [214, 106, 30],
        // `jigsaw` / `structure_void` are authoring markers: vanilla deletes the
        // former at placement and the latter is not a block at all. They should
        // never reach the model (the solver strips them) — colour them the
        // fallback magenta so it is visible immediately if one ever does.
        "jigsaw" | "structure_block" | "structure_void" => FALLBACK_COLOR,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Voxel grid
// ---------------------------------------------------------------------------

/// Edge length of one storage chunk (matching vanilla's section size).
const CHUNK: i32 = 16;
/// Cells per storage chunk.
const CHUNK_CELLS: usize = (CHUNK * CHUNK * CHUNK) as usize;

/// A dense-chunked voxel view of the assembled world, built once per snapshot.
///
/// The assembled model is a `BTreeMap<[i32;3], String>`; a raycaster does tens of
/// millions of cell probes, and a `BTreeMap` lookup per DDA step is roughly two
/// orders of magnitude too slow for the sub-second budget. This flattens it into
/// a palette plus 16³ chunk arrays: probes are O(1) array indexing, memory stays
/// proportional to *occupied* chunks (a layout spanning `AREA_SPACING`-separated
/// areas allocates nothing for the void between them), and the palette walk is a
/// `BTreeMap` iteration so palette indices are deterministic.
pub struct VoxelGrid {
    /// Palette entry 0 is always "air"; block ids follow in `BTreeMap` order.
    palette: Vec<String>,
    /// Per-palette-entry `(colour, emissive)`, precomputed.
    shading: Vec<([u8; 3], bool)>,
    /// Chunk-space origin and dimensions.
    cmin: [i32; 3],
    cdim: [usize; 3],
    /// `cdim`-indexed chunk storage; `None` = wholly air.
    chunks: Vec<Option<Box<[u16; CHUNK_CELLS]>>>,
    /// Inclusive world-cell bounds of the occupied region.
    bounds: Option<([i32; 3], [i32; 3])>,
}

/// Floor-divide `v` by [`CHUNK`] (correct for negatives, unlike `/`).
fn chunk_of(v: i32) -> i32 {
    v.div_euclid(CHUNK)
}

impl VoxelGrid {
    /// Flatten an assembled cell→block map into the raycasting grid.
    pub fn build(blocks: &BTreeMap<[i32; 3], String>) -> VoxelGrid {
        let mut palette: Vec<String> = vec!["minecraft:air".to_string()];
        let mut index: BTreeMap<&str, u16> = BTreeMap::new();
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for (cell, name) in blocks {
            if !index.contains_key(name.as_str()) {
                index.insert(name.as_str(), palette.len() as u16);
                palette.push(name.clone());
            }
            for a in 0..3 {
                lo[a] = lo[a].min(cell[a]);
                hi[a] = hi[a].max(cell[a]);
            }
        }
        let shading = palette.iter().map(|n| block_color(n)).collect();
        if blocks.is_empty() {
            return VoxelGrid {
                palette,
                shading,
                cmin: [0; 3],
                cdim: [0; 3],
                chunks: Vec::new(),
                bounds: None,
            };
        }
        let cmin = [chunk_of(lo[0]), chunk_of(lo[1]), chunk_of(lo[2])];
        let cmax = [chunk_of(hi[0]), chunk_of(hi[1]), chunk_of(hi[2])];
        let cdim = [
            (cmax[0] - cmin[0] + 1) as usize,
            (cmax[1] - cmin[1] + 1) as usize,
            (cmax[2] - cmin[2] + 1) as usize,
        ];
        let chunks: Vec<Option<Box<[u16; CHUNK_CELLS]>>> =
            (0..cdim[0] * cdim[1] * cdim[2]).map(|_| None).collect();
        let mut grid = VoxelGrid {
            palette,
            shading,
            cmin,
            cdim,
            chunks,
            bounds: Some((lo, hi)),
        };
        for (cell, name) in blocks {
            let id = *index.get(name.as_str()).expect("palette holds every id");
            let Some((ci, oi)) = grid.slot(*cell) else {
                continue;
            };
            grid.chunks[ci].get_or_insert_with(|| Box::new([0u16; CHUNK_CELLS]))[oi] = id;
        }
        grid
    }

    /// `(chunk slot index, offset in chunk)` for a world cell, or `None` when the
    /// cell lies outside the grid's chunk box.
    fn slot(&self, cell: [i32; 3]) -> Option<(usize, usize)> {
        let c = [chunk_of(cell[0]), chunk_of(cell[1]), chunk_of(cell[2])];
        let mut idx = 0usize;
        for (a, &ca) in c.iter().enumerate() {
            let k = ca - self.cmin[a];
            if k < 0 || k as usize >= self.cdim[a] {
                return None;
            }
            idx = idx * self.cdim[a] + k as usize;
        }
        let o = [
            cell[0].rem_euclid(CHUNK) as usize,
            cell[1].rem_euclid(CHUNK) as usize,
            cell[2].rem_euclid(CHUNK) as usize,
        ];
        Some((idx, (o[1] * CHUNK as usize + o[2]) * CHUNK as usize + o[0]))
    }

    /// The palette index at a cell (`0` = air / outside the grid).
    pub fn at(&self, cell: [i32; 3]) -> u16 {
        match self.slot(cell) {
            Some((ci, oi)) => self.chunks[ci].as_ref().map_or(0, |c| c[oi]),
            None => 0,
        }
    }

    /// Whether a cell holds a block.
    pub fn solid(&self, cell: [i32; 3]) -> bool {
        self.at(cell) != 0
    }

    /// The block id behind a palette index.
    pub fn name(&self, id: u16) -> &str {
        &self.palette[id as usize]
    }

    /// Inclusive world-cell bounds of the occupied region, or `None` if empty.
    pub fn bounds(&self) -> Option<([i32; 3], [i32; 3])> {
        self.bounds
    }

    /// Number of distinct block ids (excluding the air sentinel).
    pub fn block_kinds(&self) -> usize {
        self.palette.len() - 1
    }
}

/// One primary-ray intersection.
#[derive(Debug, Clone, Copy)]
pub struct Hit {
    /// The cell the ray entered.
    pub cell: [i32; 3],
    /// Palette index of the block there.
    pub block: u16,
    /// Which axis the ray crossed to enter (`0`=X, `1`=Y, `2`=Z).
    pub axis: usize,
    /// Sign of the entered face's normal along `axis` (+1 or −1).
    pub sign: i32,
    /// Distance from the ray origin, in blocks.
    pub t: f64,
    /// Where on the entered face the ray landed, both coordinates in `0.0..1.0`
    /// (used for the block-edge relief).
    pub uv: [f64; 2],
}

/// The in-face coordinates (both in `0.0..1.0`) of a hit point on a face whose
/// normal runs along `axis` — the block-edge relief's input.
fn face_uv(axis: usize, hp: [f64; 3]) -> [f64; 2] {
    let (u_ax, v_ax) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    [hp[u_ax] - hp[u_ax].floor(), hp[v_ax] - hp[v_ax].floor()]
}

impl VoxelGrid {
    /// Walk a ray through the grid (Amanatides–Woo DDA) and return the first
    /// block it enters within `max_t`, or `None`.
    ///
    /// `dir` must be normalised. The traversal is exact integer-cell stepping —
    /// no sampling, so a one-block-thick wall can never be missed.
    pub fn cast(&self, origin: [f64; 3], dir: [f64; 3], max_t: f64) -> Option<Hit> {
        let (lo, hi) = self.bounds?;
        // Skip the empty space before the model's AABB: for a camera far outside
        // a small build this is the difference between stepping 500 cells of void
        // and starting at the geometry.
        let mut t = 0.0f64;
        // Which slab the ray entered last — the face of the AABB it crossed, and
        // therefore the face of the *first cell* if that cell turns out to be
        // solid (a silhouette-edge hit the stepping loop below never revisits).
        let mut entry_axis = 1usize;
        let mut entry_sign = 1i32;
        for a in 0..3 {
            let (slab_lo, slab_hi) = (lo[a] as f64, hi[a] as f64 + 1.0);
            if dir[a].abs() < 1e-12 {
                if origin[a] < slab_lo || origin[a] > slab_hi {
                    return None;
                }
                continue;
            }
            let t0 = (slab_lo - origin[a]) / dir[a];
            let t1 = (slab_hi - origin[a]) / dir[a];
            let enter = t0.min(t1);
            if enter > t {
                t = enter;
                entry_axis = a;
                entry_sign = if dir[a] > 0.0 { -1 } else { 1 };
            }
        }
        // Re-verify: the per-axis max of entry times can still exit another slab.
        let mut t_exit = max_t;
        for a in 0..3 {
            if dir[a].abs() < 1e-12 {
                continue;
            }
            let t0 = (lo[a] as f64 - origin[a]) / dir[a];
            let t1 = (hi[a] as f64 + 1.0 - origin[a]) / dir[a];
            t_exit = t_exit.min(t0.max(t1));
        }
        if t > t_exit {
            return None;
        }
        // Nudge inside so the starting cell is unambiguous on a slab boundary.
        let start_t = if t > 0.0 { t + 1e-6 } else { 0.0 };
        let p = [
            origin[0] + dir[0] * start_t,
            origin[1] + dir[1] * start_t,
            origin[2] + dir[2] * start_t,
        ];
        let mut cell = [
            p[0].floor() as i32,
            p[1].floor() as i32,
            p[2].floor() as i32,
        ];
        // The starting cell must be tested before any stepping: the DDA loop below
        // only ever examines cells it *steps into*, so a ray that enters the AABB
        // directly onto a solid cell would otherwise pass straight through it.
        // Two cases: the eye sits inside a block (t == 0 — report its interior),
        // or the ray met the model's hull here (report the crossed AABB face).
        {
            let b = self.at(cell);
            if b != 0 {
                let (axis, sign, uv) = if t > 0.0 {
                    let hp = [
                        origin[0] + dir[0] * t,
                        origin[1] + dir[1] * t,
                        origin[2] + dir[2] * t,
                    ];
                    (entry_axis, entry_sign, face_uv(entry_axis, hp))
                } else {
                    (1, 1, [0.5, 0.5])
                };
                return Some(Hit {
                    cell,
                    block: b,
                    axis,
                    sign,
                    t,
                    uv,
                });
            }
        }
        let mut step = [0i32; 3];
        let mut t_max = [f64::INFINITY; 3];
        let mut t_delta = [f64::INFINITY; 3];
        for a in 0..3 {
            if dir[a] > 1e-12 {
                step[a] = 1;
                t_delta[a] = 1.0 / dir[a];
                t_max[a] = start_t + ((cell[a] + 1) as f64 - p[a]) / dir[a];
            } else if dir[a] < -1e-12 {
                step[a] = -1;
                t_delta[a] = -1.0 / dir[a];
                t_max[a] = start_t + (cell[a] as f64 - p[a]) / dir[a];
            }
        }
        loop {
            // Advance along whichever axis crosses next (ties break X→Y→Z, which
            // only decides which face a corner-exact ray is credited to).
            let axis = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
                0
            } else if t_max[1] <= t_max[2] {
                1
            } else {
                2
            };
            let t_cur = t_max[axis];
            if t_cur > t_exit || t_cur > max_t {
                return None;
            }
            cell[axis] += step[axis];
            t_max[axis] += t_delta[axis];
            let b = self.at(cell);
            if b != 0 {
                let hp = [
                    origin[0] + dir[0] * t_cur,
                    origin[1] + dir[1] * t_cur,
                    origin[2] + dir[2] * t_cur,
                ];
                return Some(Hit {
                    cell,
                    block: b,
                    axis,
                    sign: -step[axis],
                    t: t_cur,
                    uv: face_uv(axis, hp),
                });
            }
        }
    }

    /// Whether the straight segment from `from` to `to` meets any block before
    /// reaching `to`.
    pub fn blocked(&self, from: [f64; 3], to: [f64; 3]) -> bool {
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-9 {
            return false;
        }
        let dir = [d[0] / len, d[1] / len, d[2] / len];
        self.cast(from, dir, len).is_some()
    }

    /// The manifest's `occluded` test: does anything **other than the target
    /// itself** stand between the camera and it?
    ///
    /// Two subtleties, both learned from the island build:
    ///
    /// - **Exclude the target's own cells.** A marker routinely *is* a block —
    ///   `anchor/fire-pit` names the campfire, an interact anchor names the lever
    ///   — so a probe that stops at "the first block before the centre point"
    ///   reports the most visible object in the frame as occluded. A probe that
    ///   ends inside the target's own cell box counts as having reached it.
    /// - **Probe more than the centre.** One centre ray grazing the rim of the
    ///   platform a fire pit stands on calls the whole fire pit hidden. Nine
    ///   samples ([`Target::probe_points`]) make the answer "can *any part* of it
    ///   be seen", which is what a reviewer means by visible — and nine rays per
    ///   target is nothing beside the half-million the frame already casts.
    pub fn occluded_target(&self, from: [f64; 3], t: &Target) -> bool {
        t.probe_points()
            .into_iter()
            .all(|to| self.sight_line_blocked(from, to, t))
    }

    /// Whether the segment `from → to` meets a block that is not part of `t`.
    fn sight_line_blocked(&self, from: [f64; 3], to: [f64; 3], t: &Target) -> bool {
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-9 {
            return false;
        }
        let dir = [d[0] / len, d[1] / len, d[2] / len];
        match self.cast(from, dir, len) {
            Some(h) => !(0..3).all(|a| h.cell[a] >= t.min[a] && h.cell[a] <= t.max[a]),
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

/// A pinhole camera in Minecraft yaw/pitch degrees (see the module note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Eye position in world coordinates.
    pub pos: [f64; 3],
    /// Minecraft yaw in degrees (`0` = south/+Z, `90` = west/−X).
    pub yaw: f64,
    /// Minecraft pitch in degrees (positive looks down).
    pub pitch: f64,
    /// Vertical field of view in degrees.
    pub fov: f64,
}

impl Camera {
    /// The forward/right/up orthonormal basis. `right` is derived from yaw alone,
    /// so it stays well-defined when the camera looks straight up or down.
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let (y, p) = (self.yaw.to_radians(), self.pitch.to_radians());
        let (sy, cy) = (y.sin(), y.cos());
        let (sp, cp) = (p.sin(), p.cos());
        let forward = [-sy * cp, -sp, cy * cp];
        let right = [-cy, 0.0, -sy];
        let up = cross(right, forward);
        (forward, right, up)
    }

    /// Aim a camera at `look_at` from `pos`, in Minecraft degrees — the exact
    /// formula [`crate::emit`]'s cutscene `mc_aim` uses, so a camera derived from
    /// a `render-plan.json` shot points where the shipped cutscene points.
    pub fn looking_at(pos: [f64; 3], look_at: [f64; 3], fov: f64) -> Camera {
        let d = [
            look_at[0] - pos[0],
            look_at[1] - pos[1],
            look_at[2] - pos[2],
        ];
        let horiz = (d[0] * d[0] + d[2] * d[2]).sqrt();
        let (yaw, pitch) = if horiz < 1e-9 && d[1].abs() < 1e-9 {
            (0.0, 0.0)
        } else {
            (
                (-d[0]).atan2(d[2]).to_degrees(),
                (-d[1]).atan2(horiz).to_degrees(),
            )
        };
        Camera {
            pos,
            yaw,
            pitch,
            fov,
        }
    }

    /// The normalised ray direction through pixel centre `(px, py)`.
    pub fn ray(&self, width: u32, height: u32, px: u32, py: u32) -> [f64; 3] {
        let (f, r, u) = self.basis();
        let aspect = width as f64 / height as f64;
        let ty = (self.fov.to_radians() * 0.5).tan();
        let ndc_x = ((px as f64 + 0.5) / width as f64) * 2.0 - 1.0;
        let ndc_y = 1.0 - ((py as f64 + 0.5) / height as f64) * 2.0;
        let d = [
            f[0] + r[0] * ndc_x * ty * aspect + u[0] * ndc_y * ty,
            f[1] + r[1] * ndc_x * ty * aspect + u[1] * ndc_y * ty,
            f[2] + r[2] * ndc_x * ty * aspect + u[2] * ndc_y * ty,
        ];
        normalize(d)
    }

    /// Project a world point to pixel coordinates plus camera-space depth.
    /// `None` when the point is at or behind the eye plane.
    pub fn project(&self, width: u32, height: u32, p: [f64; 3]) -> Option<([f64; 2], f64)> {
        let (f, r, u) = self.basis();
        let c = [p[0] - self.pos[0], p[1] - self.pos[1], p[2] - self.pos[2]];
        let z = dot(c, f);
        if z <= 1e-6 {
            return None;
        }
        let aspect = width as f64 / height as f64;
        let ty = (self.fov.to_radians() * 0.5).tan();
        let ndc_x = dot(c, r) / (z * ty * aspect);
        let ndc_y = dot(c, u) / (z * ty);
        Some((
            [
                (ndc_x * 0.5 + 0.5) * width as f64,
                (0.5 - ndc_y * 0.5) * height as f64,
            ],
            z,
        ))
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-12 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

/// Round to 3 decimals — the same stabilisation `render_plan` uses so float
/// formatting cannot drift between platforms under the byte-identity gate.
pub fn round3(v: f64) -> f64 {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 { 0.0 } else { r }
}

// ---------------------------------------------------------------------------
// Targets (the manifest's subjects)
// ---------------------------------------------------------------------------

/// One addressable thing in the world the manifest can describe: an anchor, a
/// gate region, an NPC or actor post, an interact marker, a stealth zone, or a
/// trigger region. Every target is an inclusive cell box (`min == max` for a
/// point), so screen bboxes and occlusion probes have one code path.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// Stable id — the DSL id of the thing (anchor / npc / actor / objective /
    /// trigger id), or `<beat>/<zone-anchor>` for a stealth zone.
    pub id: String,
    /// Kind tag: `anchor` · `gate` · `npc-post` · `actor-post` · `interact` ·
    /// `stealth-zone` · `trigger`.
    pub kind: &'static str,
    /// Owning area id (empty when the thing is not area-scoped).
    pub area: String,
    /// Inclusive minimum cell of the target's box.
    pub min: [i32; 3],
    /// Inclusive maximum cell of the target's box.
    pub max: [i32; 3],
}

/// How far inside its own box a visibility probe point sits (see
/// [`Target::probe_points`]) — enough to clear the surface of the block the
/// marker may coincide with, small enough to stay within a 1-cell target.
const PROBE_INSET: f64 = 0.3;

impl Target {
    fn point(id: String, kind: &'static str, area: String, pos: [i32; 3]) -> Target {
        Target {
            id,
            kind,
            area,
            min: pos,
            max: pos,
        }
    }

    /// Whether this target is a single cell (rendered as a point, not a box).
    pub fn is_point(&self) -> bool {
        self.min == self.max
    }

    /// The box centre in world coordinates (cell centres, so a point target's
    /// centre is the middle of its block).
    pub fn centre(&self) -> [f64; 3] {
        [
            (self.min[0] + self.max[0] + 1) as f64 / 2.0,
            (self.min[1] + self.max[1] + 1) as f64 / 2.0,
            (self.min[2] + self.max[2] + 1) as f64 / 2.0,
        ]
    }

    /// The eight world-space corners of the box.
    pub fn corners(&self) -> [[f64; 3]; 8] {
        let lo = [self.min[0] as f64, self.min[1] as f64, self.min[2] as f64];
        let hi = [
            self.max[0] as f64 + 1.0,
            self.max[1] as f64 + 1.0,
            self.max[2] as f64 + 1.0,
        ];
        let mut out = [[0.0; 3]; 8];
        for (i, c) in out.iter_mut().enumerate() {
            *c = [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ];
        }
        out
    }

    /// Nine visibility probe points: the box centre plus the corners of the box
    /// inset by [`PROBE_INSET`] on every side (clamped so a 1-cell target still
    /// yields eight distinct interior points). See [`VoxelGrid::occluded_target`]
    /// for why one centre ray is not enough.
    pub fn probe_points(&self) -> [[f64; 3]; 9] {
        let c = self.centre();
        let half = [
            ((self.max[0] - self.min[0] + 1) as f64 / 2.0 - PROBE_INSET).max(0.2),
            ((self.max[1] - self.min[1] + 1) as f64 / 2.0 - PROBE_INSET).max(0.2),
            ((self.max[2] - self.min[2] + 1) as f64 / 2.0 - PROBE_INSET).max(0.2),
        ];
        let mut out = [c; 9];
        for (i, p) in out.iter_mut().skip(1).enumerate() {
            *p = [
                c[0] + if i & 1 == 0 { -half[0] } else { half[0] },
                c[1] + if i & 2 == 0 { -half[1] } else { half[1] },
                c[2] + if i & 4 == 0 { -half[2] } else { half[2] },
            ];
        }
        out
    }

    /// The short display label burned into a `--labels` frame: the id's local
    /// part, upper-cased (the bitmap font is caps-only).
    pub fn label(&self) -> String {
        self.id
            .rsplit('/')
            .next()
            .unwrap_or(&self.id)
            .to_ascii_uppercase()
    }
}

/// One placed structure piece, as the manifest reports it — the **layout** half
/// of the scene description, beside the point/region [`Target`]s.
///
/// A `piece-local` edit frame (`{"kind":"piece-local","piece":N,"prefab":…}`) is
/// resolved by the replay as `piece.origin + rotation.transform(local)` against
/// `area.pieces[N]` (`edit::resolve_frame_point`). Before this listing existed
/// the manifest carried area bounds and anchors only, so an editor authoring a
/// piece-local frame had to *back-solve* N, the prefab guard and the transform
/// by hand from the rendered geometry. Every input to that resolution is now
/// stated in-band.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceEntry {
    /// Owning area id.
    pub area: String,
    /// Index **within that area's** placed pieces — exactly the `piece` field of
    /// a `piece-local` edit frame (entry piece is 0).
    pub index: usize,
    /// Bound prefab id (`prefab/…`) — the frame's `prefab` guard value.
    pub prefab: String,
    /// The `/place template` position: where prefab-local `(0,0,0)` lands.
    pub origin: [i32; 3],
    /// Unrotated prefab size `[sx, sy, sz]`.
    pub size: [i32; 3],
    /// Placement rotation, in the `/place template` vocabulary (`none`,
    /// `clockwise_90`, `180`, `counterclockwise_90`).
    pub rotation: &'static str,
    /// Inclusive minimum cell of the placed AABB.
    pub min: [i32; 3],
    /// Inclusive maximum cell of the placed AABB.
    pub max: [i32; 3],
}

/// Collect every placed piece of a compiled plan, in **plan order** — areas as
/// the plan holds them, pieces entry-first within each area — so `index` is the
/// piece-local frame's index and the ordering is deterministic (ADR-0006).
pub fn collect_pieces(plan: &Plan) -> Vec<PieceEntry> {
    let mut out = Vec::new();
    for area in &plan.areas {
        for (index, piece) in area.pieces.iter().enumerate() {
            let (min, max) = piece.bbox();
            out.push(PieceEntry {
                area: area.area_id.clone(),
                index,
                prefab: piece.prefab_id.clone(),
                origin: piece.pos,
                size: piece.size,
                rotation: piece.rotation.token().unwrap_or("none"),
                min,
                max,
            });
        }
    }
    out
}

/// Collect every manifest-addressable target from a compiled plan, sorted by
/// `(kind, area, id)` so the manifest's ordering is deterministic (ADR-0006).
///
/// A single world position may legitimately appear under two kinds — an
/// `interact` objective's marker and the `anchor` it binds to are the same cell
/// but different *things*, addressed by different ids, and a review that says
/// "the interact is occluded" means something different from "the anchor is". No
/// deduplication is applied.
pub fn collect_targets(plan: &Plan) -> Vec<Target> {
    let c = plan.campaign;
    let mut out: Vec<Target> = Vec::new();

    // Anchors + gate regions (BTreeMap order).
    for ((area, name), resolved) in &plan.anchors {
        match resolved {
            ResolvedAnchor::Point { pos, .. } => {
                out.push(Target::point(name.clone(), "anchor", area.clone(), *pos));
            }
            ResolvedAnchor::Gate { from, to, .. } => out.push(Target {
                id: name.clone(),
                kind: "gate",
                area: area.clone(),
                min: [from[0].min(to[0]), from[1].min(to[1]), from[2].min(to[2])],
                max: [from[0].max(to[0]), from[1].max(to[1]), from[2].max(to[2])],
            }),
        }
    }

    // NPC posts: where a mannequin stands.
    for npc in &c.npcs.content.npcs {
        let area = plan.npc_area(npc.id.as_str()).unwrap_or("").to_string();
        if let Some(pos) = plan.point(&area, npc.anchor.as_str()) {
            out.push(Target::point(
                npc.id.as_str().to_string(),
                "npc-post",
                area,
                pos,
            ));
        }
    }

    // Actor posts: where a scripted puppet is summoned (v0.6, spec-0014).
    for actor in &c.quests.content.actors {
        if let Some((area, pos)) = resolve_any_area(plan, actor.anchor.as_str()) {
            out.push(Target::point(
                actor.id.as_str().to_string(),
                "actor-post",
                area,
                pos,
            ));
        }
    }

    // Interact markers: the objective's own id at its anchor cell.
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if let Some((obj_id, anchor)) = interact_of(o)
                && let Some((area, pos)) = resolve_any_area(plan, anchor)
            {
                out.push(Target::point(obj_id.to_string(), "interact", area, pos));
            }
        }
    }

    // Stealth zones: `begin-stealth` boxes (centre + half-extents).
    for beat in &plan.stealth_beats {
        for (anchor, centre, half) in &beat.zones {
            let h = [half[0] as i32, half[1] as i32, half[2] as i32];
            out.push(Target {
                id: format!("stealth-{}/{}", beat.index, local_of(anchor)),
                kind: "stealth-zone",
                area: String::new(),
                min: [centre[0] - h[0], centre[1] - h[1], centre[2] - h[2]],
                max: [centre[0] + h[0], centre[1] + h[1], centre[2] + h[2]],
            });
        }
    }

    // Trigger regions: an `approach` trigger is a box of its range; `strike`/
    // `use` triggers are the single interaction-entity cell.
    for t in &c.quests.content.triggers {
        let Some((area, pos)) = resolve_any_area(plan, t.at.as_str()) else {
            continue;
        };
        let r = match t.on {
            delvewright_dsl::TriggerOn::Approach { range } => range as i32,
            _ => 0,
        };
        out.push(Target {
            id: t.id.as_str().to_string(),
            kind: "trigger",
            area,
            min: [pos[0] - r, pos[1] - r, pos[2] - r],
            max: [pos[0] + r, pos[1] + r, pos[2] + r],
        });
    }

    out.sort_by(|a, b| (a.kind, &a.area, &a.id).cmp(&(b.kind, &b.area, &b.id)));
    out
}

/// `(objective id, anchor id)` for an `interact` objective, else `None`.
fn interact_of(o: &delvewright_dsl::Objective) -> Option<(&str, &str)> {
    match o {
        delvewright_dsl::Objective::Interact { id, anchor, .. } => {
            Some((id.as_str(), anchor.as_str()))
        }
        _ => None,
    }
}

/// Resolve an anchor name in whichever area declares it (first in `BTreeMap`
/// order), returning `(area id, cell)`. Point anchors only.
fn resolve_any_area(plan: &Plan, anchor: &str) -> Option<(String, [i32; 3])> {
    plan.anchors.iter().find_map(|((area, name), r)| match r {
        ResolvedAnchor::Point { pos, .. } if name == anchor => Some((area.clone(), *pos)),
        _ => None,
    })
}

/// The local (post-`/`) part of an id.
fn local_of(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

// ---------------------------------------------------------------------------
// Frame rendering
// ---------------------------------------------------------------------------

/// Per-face brightness multipliers, indexed by `axis * 2 + (sign < 0) as usize`.
/// The cheap "ambient occlusion by face orientation" spec-0015 asks for: a
/// top face reads brightest, a bottom face darkest, and the two horizontal axes
/// differ so a corner between two walls is legible without a light model.
const FACE_SHADE: [f64; 6] = [
    0.66, // +X
    0.58, // −X
    1.00, // +Y (top)
    0.42, // −Y (bottom)
    0.86, // +Z
    0.76, // −Z
];

/// Distance (blocks) over which a surface fades toward the horizon colour. Pure
/// depth cueing — flat colours alone make a long cavern read as a single wall.
const FOG_DISTANCE: f64 = 190.0;

/// Rendering knobs for one frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameOpts {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Draw the ocean-horizon sea plane in the background pass
    /// (see [`SEA_PLANE_NOTE`]).
    pub sea_level: Option<i32>,
    /// Burn in labels + the ground coordinate grid.
    pub labels: bool,
}

impl Default for FrameOpts {
    fn default() -> Self {
        FrameOpts {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            sea_level: None,
            labels: false,
        }
    }
}

/// A rendered frame: the pixel canvas plus the per-pixel ray distance the label
/// pass uses to hide overlays the camera cannot actually see.
pub struct Frame {
    /// The pixel buffer (drawing primitives live on [`crate::raster::Canvas`]).
    pub canvas: Canvas,
    /// Row-major ray distance in blocks (`f64::INFINITY` = background).
    pub depth: Vec<f64>,
}

impl Frame {
    /// Frame width in pixels.
    pub fn width(&self) -> u32 {
        self.canvas.width
    }

    /// Frame height in pixels.
    pub fn height(&self) -> u32 {
        self.canvas.height
    }

    /// Ray distance at a pixel (`INFINITY` for background / off-frame).
    fn depth_at(&self, x: i64, y: i64) -> f64 {
        if x < 0 || y < 0 || x >= self.canvas.width as i64 || y >= self.canvas.height as i64 {
            return f64::INFINITY;
        }
        self.depth[(y as usize) * self.canvas.width as usize + x as usize]
    }
}

/// Raycast one frame of the assembled world.
pub fn render_frame(grid: &VoxelGrid, cam: &Camera, opts: &FrameOpts) -> Frame {
    let (w, h) = (opts.width, opts.height);
    let mut canvas = Canvas::filled(w, h, [0, 0, 0]);
    let mut depth = vec![f64::INFINITY; (w as usize) * (h as usize)];
    for py in 0..h {
        for px in 0..w {
            let dir = cam.ray(w, h, px, py);
            let (color, d) = match grid.cast(cam.pos, dir, MAX_RAY_DISTANCE) {
                Some(hit) => {
                    let mut c = shade(grid, &hit);
                    // The ground coordinate grid rides the primary ray: tinting
                    // here costs nothing, where a second full-frame cast to find
                    // the same top faces would double the render.
                    if opts.labels
                        && let Some(a) = grid_tint(&hit)
                    {
                        c = mix(
                            [c[0] as f64, c[1] as f64, c[2] as f64],
                            [120.0, 235.0, 235.0],
                            a,
                        );
                    }
                    (c, hit.t)
                }
                None => (background(cam, dir, opts), f64::INFINITY),
            };
            canvas.set(px as i64, py as i64, color);
            depth[(py as usize) * w as usize + px as usize] = d;
        }
    }
    Frame { canvas, depth }
}

/// Sky colour at the zenith / at the horizon / below the horizon (void).
const SKY_ZENITH: [f64; 3] = [96.0, 146.0, 214.0];
const SKY_HORIZON: [f64; 3] = [178.0, 204.0, 232.0];
const VOID_COLOR: [f64; 3] = [20.0, 20.0, 26.0];
/// Sea colour used only for the `ocean` horizon backdrop plane.
const SEA_COLOR: [f64; 3] = [46.0, 82.0, 152.0];

/// Background colour for a ray that hits no block: a sky gradient above the
/// horizon, the void below — plus, for an `ocean`-horizon campaign, the
/// world-generation sea plane (see [`SEA_PLANE_NOTE`]).
fn background(cam: &Camera, dir: [f64; 3], opts: &FrameOpts) -> [u8; 3] {
    if let Some(sea) = opts.sea_level {
        // Analytic intersection with the plane y = sea + 1 (the water surface is
        // the top face of the topmost water block).
        let surface = sea as f64 + 1.0;
        if dir[1] < -1e-9 && cam.pos[1] > surface {
            let t = (surface - cam.pos[1]) / dir[1];
            if t > 0.0 && t < MAX_RAY_DISTANCE {
                let f = fog_factor(t);
                return mix(SEA_COLOR, SKY_HORIZON, f);
            }
        }
    }
    if dir[1] >= 0.0 {
        mix(SKY_HORIZON, SKY_ZENITH, dir[1].powf(0.6))
    } else {
        mix(SKY_HORIZON, VOID_COLOR, (-dir[1]).powf(0.35))
    }
}

/// How much a surface at distance `t` has faded into the horizon.
fn fog_factor(t: f64) -> f64 {
    1.0 - (-t / FOG_DISTANCE).exp()
}

/// Flat-shade one hit: palette colour × face brightness, block-edge relief, then
/// distance fade. Emissive blocks skip all three.
fn shade(grid: &VoxelGrid, hit: &Hit) -> [u8; 3] {
    let (base, emissive) = grid.shading[hit.block as usize];
    let b = [base[0] as f64, base[1] as f64, base[2] as f64];
    if emissive {
        return [base[0], base[1], base[2]];
    }
    let face = FACE_SHADE[hit.axis * 2 + usize::from(hit.sign < 0)];
    // Block-edge relief: darken the outermost 1/16 of the face. This is what
    // makes a flat-coloured wall read as *blocks* rather than as a paint swatch —
    // the cheapest possible substitute for texture detail.
    let edge = hit.uv[0]
        .min(1.0 - hit.uv[0])
        .min(hit.uv[1].min(1.0 - hit.uv[1]));
    let relief = if edge < 0.0625 { 0.86 } else { 1.0 };
    let lit = [
        b[0] * face * relief,
        b[1] * face * relief,
        b[2] * face * relief,
    ];
    let f = fog_factor(hit.t);
    let c = mix(lit, SKY_HORIZON, f * 0.85);
    [c[0], c[1], c[2]]
}

/// Linearly blend two float colours and clamp to bytes.
fn mix(a: [f64; 3], b: [f64; 3], t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let mut out = [0u8; 3];
    for k in 0..3 {
        out[k] = (a[k] + (b[k] - a[k]) * t).round().clamp(0.0, 255.0) as u8;
    }
    out
}

// ---------------------------------------------------------------------------
// Screen-space geometry + the manifest
// ---------------------------------------------------------------------------

/// Project a target's box to a clipped screen bbox, or `None` when it is wholly
/// behind the camera or wholly off-frame (i.e. outside the frustum).
pub fn screen_box(cam: &Camera, w: u32, h: u32, t: &Target) -> Option<ScreenBox> {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    let mut any_front = false;
    for c in t.corners() {
        if let Some((s, _)) = cam.project(w, h, c) {
            any_front = true;
            lo[0] = lo[0].min(s[0]);
            lo[1] = lo[1].min(s[1]);
            hi[0] = hi[0].max(s[0]);
            hi[1] = hi[1].max(s[1]);
        }
    }
    if !any_front {
        return None;
    }
    let x0 = lo[0].floor().max(0.0) as i64;
    let y0 = lo[1].floor().max(0.0) as i64;
    let x1 = hi[0].ceil().min(w as f64) as i64;
    let y1 = hi[1].ceil().min(h as f64) as i64;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(ScreenBox {
        x: x0,
        y: y0,
        w: (x1 - x0).max(1),
        h: (y1 - y0).max(1),
    })
}

/// A target resolved against one camera: its screen footprint and whether the
/// straight line from the eye to its centre is blocked.
#[derive(Debug, Clone)]
pub struct ResolvedTarget<'a> {
    /// The target itself.
    pub target: &'a Target,
    /// Screen-space bbox in pixels.
    pub bbox: ScreenBox,
    /// Whether geometry stands between the camera and the target centre.
    pub occluded: bool,
    /// Eye→centre distance in blocks.
    pub distance: f64,
}

/// Resolve every target against a camera, returning `(in frustum, out of frame
/// ids)`. Input order is preserved, so the manifest inherits
/// [`collect_targets`]'s deterministic sort.
pub fn resolve_targets<'a>(
    grid: &VoxelGrid,
    cam: &Camera,
    opts: &FrameOpts,
    targets: &'a [Target],
) -> (Vec<ResolvedTarget<'a>>, Vec<&'a Target>) {
    let mut inside = Vec::new();
    let mut outside = Vec::new();
    for t in targets {
        match screen_box(cam, opts.width, opts.height, t) {
            Some(bbox) => {
                let centre = t.centre();
                let d = [
                    centre[0] - cam.pos[0],
                    centre[1] - cam.pos[1],
                    centre[2] - cam.pos[2],
                ];
                inside.push(ResolvedTarget {
                    target: t,
                    bbox,
                    occluded: grid.occluded_target(cam.pos, t),
                    distance: (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(),
                });
            }
            None => outside.push(t),
        }
    }
    (inside, outside)
}

/// What the manifest describes about the world, as opposed to the camera and the
/// image: the solved layout (`pieces`) plus the targets in and out of frame.
/// Grouped so [`manifest`] takes one scene rather than three parallel slices.
pub struct Scene<'a> {
    /// Every placed piece of the plan ([`collect_pieces`]).
    pub pieces: &'a [PieceEntry],
    /// Targets inside the frustum, with their screen boxes.
    pub inside: &'a [ResolvedTarget<'a>],
    /// Targets outside it.
    pub outside: &'a [&'a Target],
}

/// Build the scene manifest — the structured half of the dual-channel loop.
///
/// Schema (v2):
/// ```text
/// { manifest_version, campaign_id, delvec, image{path,width,height},
///   camera{pos,yaw,pitch,fov,convention}, world{blocks,block_kinds,bounds},
///   pieces[ {area,index,prefab,origin,size,rotation,box{min,max}} ],
///   targets[ {id,kind,area,pos|box,screen_bbox{x,y,w,h},occluded,distance} ],
///   out_of_frame[ {id,kind,area,pos|box} ] }
/// ```
/// `pos` is present for point targets, `box` (`{min,max}`, inclusive cells) for
/// region targets — never both. `out_of_frame` is the complement of `targets`:
/// it is what makes "the subject is absent entirely" (spec-0015 pillar 4)
/// machine-visible rather than something a reviewer must notice. `pieces`
/// ([`PieceEntry`]) is the layout half: the placed structure pieces of the whole
/// plan (not only the ones in frame), which is what a `piece-local` edit frame
/// addresses.
pub fn manifest(
    campaign_id: &str,
    image_path: &str,
    cam: &Camera,
    opts: &FrameOpts,
    grid: &VoxelGrid,
    scene: Scene<'_>,
) -> Value {
    let Scene {
        pieces,
        inside,
        outside,
    } = scene;
    let bounds = grid
        .bounds()
        .map(|(lo, hi)| json!({ "min": lo, "max": hi }));
    json!({
        "manifest_version": 2,
        "campaign_id": campaign_id,
        "delvec": crate::DELVEC_VERSION,
        "image": {
            "path": image_path,
            "width": opts.width,
            "height": opts.height,
        },
        "camera": {
            "pos": [round3(cam.pos[0]), round3(cam.pos[1]), round3(cam.pos[2])],
            "yaw": round3(cam.yaw),
            "pitch": round3(cam.pitch),
            "fov": round3(cam.fov),
            "convention": "minecraft degrees: yaw 0 = south (+Z), 90 = west (-X); \
                           pitch positive looks down; fov is vertical",
        },
        "world": {
            "block_kinds": grid.block_kinds(),
            "bounds": bounds,
            "sea_plane": opts.sea_level,
        },
        "pieces": pieces.iter().map(|p| json!({
            "area": p.area,
            "index": p.index,
            "prefab": p.prefab,
            "origin": p.origin,
            "size": p.size,
            "rotation": p.rotation,
            "box": { "min": p.min, "max": p.max },
        })).collect::<Vec<_>>(),
        "targets": inside.iter().map(|r| {
            let mut v = target_json(r.target);
            v["screen_bbox"] = json!({
                "x": r.bbox.x, "y": r.bbox.y, "w": r.bbox.w, "h": r.bbox.h,
            });
            v["occluded"] = json!(r.occluded);
            v["distance"] = json!(round3(r.distance));
            v
        }).collect::<Vec<_>>(),
        "out_of_frame": outside.iter().map(|t| target_json(t)).collect::<Vec<_>>(),
    })
}

/// The world-space half of a manifest entry (shared by `targets` and
/// `out_of_frame`).
fn target_json(t: &Target) -> Value {
    let mut v = json!({
        "id": t.id,
        "kind": t.kind,
    });
    if !t.area.is_empty() {
        v["area"] = json!(t.area);
    }
    if t.is_point() {
        v["pos"] = json!(t.min);
    } else {
        v["box"] = json!({ "min": t.min, "max": t.max });
    }
    v
}

// ---------------------------------------------------------------------------
// Label / grid overlay
// ---------------------------------------------------------------------------

/// Burn the `--labels` overlay into a frame: a coordinate grid on the ground, a
/// marker box per in-frustum target, and its id.
///
/// Labels are placed greedily in manifest order (which is deterministic), each
/// nudged straight down until it clears every label already placed — so the
/// overlay is a pure function of the frame, and a crowded corner degrades into a
/// legible stack instead of illegible overprint.
pub fn draw_labels(f: &mut Frame, grid: &VoxelGrid, cam: &Camera, inside: &[ResolvedTarget<'_>]) {
    draw_ground_grid(f, grid, cam);
    let scale = 1i64;
    // Every in-frustum target gets its outline (dim when occluded). Names are
    // stamped VISIBLE-FIRST: an interior camera has the whole rest of the layout
    // inside its frustum but behind rock, and letting those names compete for
    // space with what the camera can actually see would bury the shot's subject.
    // An occluded name is stamped only where it lands clear on the first try —
    // the manifest always carries the full list either way.
    for r in inside {
        f.canvas.stroke_rect(
            r.bbox,
            kind_color(r.target.kind),
            if r.occluded { 0.45 } else { 0.95 },
        );
    }
    let width = f.width() as i64;
    let mut placer = LabelPlacer::default();
    for pass_occluded in [false, true] {
        for r in inside.iter().filter(|r| r.occluded == pass_occluded) {
            let color = kind_color(r.target.kind);
            let text = r.target.label();
            let tw = text_width(&text, scale);
            let th = GLYPH_ROWS * scale;
            let lx = (r.bbox.x).min(width - tw - 2).max(2);
            let ly = if r.bbox.y - th - 3 < 2 {
                r.bbox.y + r.bbox.h + 3
            } else {
                r.bbox.y - th - 3
            };
            let want = ScreenBox {
                x: lx,
                y: ly,
                w: tw + 2,
                h: th + 2,
            };
            // Occluded names get no nudges: they are stamped only where they land
            // clear on the first try, and are otherwise crowded out (the manifest
            // still names them).
            let nudges = if pass_occluded { 0 } else { 24 };
            let Some(box_) = placer.place(want, th + 4, nudges) else {
                continue;
            };
            f.canvas.stamp_text(lx, box_.y, &text, scale, color);
        }
    }
}

/// Grid spacing in blocks for the ground coordinate grid.
const GRID_SPACING: i32 = 16;

/// Blend strength of the coordinate lattice on a hit's surface, or `None` when
/// the hit is not a top face on a [`GRID_SPACING`] line. Intersections read
/// brighter than the lines, so a corner is countable at a glance.
fn grid_tint(hit: &Hit) -> Option<f64> {
    if hit.axis != 1 || hit.sign != 1 {
        return None; // top faces only — the "ground plane", whatever its height
    }
    let on_x = hit.cell[0].rem_euclid(GRID_SPACING) == 0;
    let on_z = hit.cell[2].rem_euclid(GRID_SPACING) == 0;
    match (on_x, on_z) {
        (true, true) => Some(0.55),
        (true, _) | (_, true) => Some(0.3),
        _ => None,
    }
}

/// Tint every visible **top** face whose cell sits on a 16-block X or Z line, and
/// label the visible grid intersections with their `x,z`.
///
/// Drawing the grid on the surfaces the frame already shows (rather than on an
/// invented flat plane) means it follows the terrain: a beach, a cavern floor and
/// a ramp all get the same lattice at the same world coordinates, so a reviewer
/// can read a position off any of them.
fn draw_ground_grid(f: &mut Frame, grid: &VoxelGrid, cam: &Camera) {
    let (w, h) = (f.width(), f.height());
    // The lattice itself is tinted during the primary raycast (see
    // [`render_frame`]); this pass only adds the coordinate readouts.
    // Coordinate readouts at the grid intersections nearest the camera, so the
    // frame carries an absolute reference without becoming a wall of numbers.
    let Some((lo, hi)) = grid.bounds() else {
        return;
    };
    let mut marks: Vec<([i32; 3], f64)> = Vec::new();
    let x0 = lo[0].div_euclid(GRID_SPACING) * GRID_SPACING;
    let z0 = lo[2].div_euclid(GRID_SPACING) * GRID_SPACING;
    let mut x = x0;
    while x <= hi[0] {
        let mut z = z0;
        while z <= hi[2] {
            // The topmost solid cell in this column is where the label belongs.
            let mut y = hi[1];
            while y >= lo[1] && !grid.solid([x, y, z]) {
                y -= 1;
            }
            if y >= lo[1] {
                let c = [x as f64 + 0.5, y as f64 + 1.0, z as f64 + 0.5];
                let d = ((c[0] - cam.pos[0]).powi(2)
                    + (c[1] - cam.pos[1]).powi(2)
                    + (c[2] - cam.pos[2]).powi(2))
                .sqrt();
                marks.push(([x, y, z], d));
            }
            z += GRID_SPACING;
        }
        x += GRID_SPACING;
    }
    marks.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut drawn = 0;
    for (cell, dist) in marks {
        if drawn >= 10 {
            break;
        }
        let c = [
            cell[0] as f64 + 0.5,
            cell[1] as f64 + 1.0,
            cell[2] as f64 + 0.5,
        ];
        let Some((s, _)) = cam.project(w, h, c) else {
            continue;
        };
        let (sx, sy) = (s[0].round() as i64, s[1].round() as i64);
        if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 {
            continue;
        }
        // Depth test against the rendered frame: a readout on a surface the
        // camera cannot see would be a lie.
        if f.depth_at(sx, sy) + 1.5 < dist {
            continue;
        }
        f.canvas.stamp_text(
            sx + 3,
            sy + 3,
            &format!("{},{}", cell[0], cell[2]),
            1,
            [150, 245, 245],
        );
        drawn += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1-block world at the origin cell.
    fn one_block(name: &str) -> VoxelGrid {
        let mut m = BTreeMap::new();
        m.insert([0, 0, 0], name.to_string());
        VoxelGrid::build(&m)
    }

    #[test]
    fn grid_round_trips_cells_including_negative_coordinates() {
        let mut m = BTreeMap::new();
        m.insert([0, 64, 0], "minecraft:stone".to_string());
        m.insert([-33, -5, -17], "minecraft:sand".to_string());
        m.insert([200, 70, 120], "minecraft:water".to_string());
        let g = VoxelGrid::build(&m);
        assert_eq!(g.name(g.at([0, 64, 0])), "minecraft:stone");
        assert_eq!(g.name(g.at([-33, -5, -17])), "minecraft:sand");
        assert_eq!(g.name(g.at([200, 70, 120])), "minecraft:water");
        assert_eq!(g.at([1, 64, 0]), 0, "an empty cell is air");
        assert_eq!(g.at([9999, 9999, 9999]), 0, "outside the grid is air");
        assert_eq!(g.bounds(), Some(([-33, -5, -17], [200, 70, 120])));
        assert_eq!(g.block_kinds(), 3);
    }

    #[test]
    fn empty_world_casts_nothing() {
        let g = VoxelGrid::build(&BTreeMap::new());
        assert!(g.bounds().is_none());
        assert!(g.cast([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 100.0).is_none());
        assert!(!g.blocked([0.0, 0.0, 0.0], [10.0, 0.0, 0.0]));
    }

    #[test]
    fn dda_hits_the_expected_face_from_each_direction() {
        let g = one_block("minecraft:stone");
        // From −Z looking +Z: enters the −Z face (axis 2, sign −1).
        let h = g
            .cast([0.5, 0.5, -5.0], [0.0, 0.0, 1.0], 100.0)
            .expect("hits");
        assert_eq!((h.cell, h.axis, h.sign), ([0, 0, 0], 2, -1));
        assert!((h.t - 5.0).abs() < 1e-6, "t = {}", h.t);
        // From above looking down: enters the +Y (top) face.
        let h = g
            .cast([0.5, 9.0, 0.5], [0.0, -1.0, 0.0], 100.0)
            .expect("hits");
        assert_eq!((h.axis, h.sign), (1, 1));
        // From +X looking −X: enters the +X face.
        let h = g
            .cast([7.0, 0.5, 0.5], [-1.0, 0.0, 0.0], 100.0)
            .expect("hits");
        assert_eq!((h.axis, h.sign), (0, 1));
    }

    #[test]
    fn dda_never_tunnels_through_a_one_block_wall() {
        // A diagonal ray across a full 1-thick plane must always hit. Sampling
        // renderers miss this; exact cell stepping cannot.
        let mut m = BTreeMap::new();
        for x in -20..20 {
            for y in -20..20 {
                m.insert([x, y, 0], "minecraft:stone".to_string());
            }
        }
        let g = VoxelGrid::build(&m);
        for k in 0..64 {
            let a = k as f64 * 0.09;
            let dir = normalize([a.sin() * 0.4, a.cos() * 0.4, 1.0]);
            assert!(
                g.cast([0.5, 0.5, -12.0], dir, 100.0).is_some(),
                "ray {k} tunnelled through the wall"
            );
        }
    }

    #[test]
    fn cast_misses_a_ray_pointed_away_from_the_model() {
        let g = one_block("minecraft:stone");
        assert!(g.cast([0.5, 0.5, -5.0], [0.0, 0.0, -1.0], 100.0).is_none());
    }

    #[test]
    fn cast_respects_max_distance() {
        let g = one_block("minecraft:stone");
        assert!(g.cast([0.5, 0.5, -50.0], [0.0, 0.0, 1.0], 10.0).is_none());
        assert!(g.cast([0.5, 0.5, -50.0], [0.0, 0.0, 1.0], 100.0).is_some());
    }

    #[test]
    fn camera_basis_matches_minecraft_yaw() {
        // yaw 0 = south (+Z); 90 = west (−X); 180 = north (−Z); 270 = east (+X).
        for (yaw, want) in [
            (0.0, [0.0, 0.0, 1.0]),
            (90.0, [-1.0, 0.0, 0.0]),
            (180.0, [0.0, 0.0, -1.0]),
            (270.0, [1.0, 0.0, 0.0]),
        ] {
            let c = Camera {
                pos: [0.0; 3],
                yaw,
                pitch: 0.0,
                fov: 70.0,
            };
            let (f, _, _) = c.basis();
            for a in 0..3 {
                assert!(
                    (f[a] - want[a]).abs() < 1e-9,
                    "yaw {yaw} forward {f:?} != {want:?}"
                );
            }
        }
        // Positive pitch looks DOWN.
        let c = Camera {
            pos: [0.0; 3],
            yaw: 0.0,
            pitch: 90.0,
            fov: 70.0,
        };
        let (f, r, u) = c.basis();
        assert!(f[1] < -0.999, "pitch +90 must look down: {f:?}");
        // The basis stays orthonormal at the pole (right is derived from yaw).
        assert!(dot(r, f).abs() < 1e-9 && dot(u, f).abs() < 1e-9 && dot(r, u).abs() < 1e-9);
    }

    #[test]
    fn looking_at_matches_the_cutscene_aim_convention() {
        // Straight south → yaw 0; straight west → yaw 90; straight down → +90 pitch.
        let c = Camera::looking_at([0.0, 0.0, 0.0], [0.0, 0.0, 5.0], 70.0);
        assert!(c.yaw.abs() < 1e-9 && c.pitch.abs() < 1e-9, "{c:?}");
        let c = Camera::looking_at([0.0, 0.0, 0.0], [-5.0, 0.0, 0.0], 70.0);
        assert!((c.yaw - 90.0).abs() < 1e-9, "{c:?}");
        let c = Camera::looking_at([0.0, 10.0, 0.0], [0.0, 0.0, 0.0], 70.0);
        assert!((c.pitch - 90.0).abs() < 1e-9, "{c:?}");
    }

    #[test]
    fn projection_and_ray_are_mutual_inverses() {
        // Project a point, cast the ray through the pixel it lands on, and the ray
        // must point back at the point. This is the property the manifest's screen
        // bboxes depend on being true.
        let cam = Camera {
            pos: [3.0, 70.0, -12.0],
            yaw: 25.0,
            pitch: 12.0,
            fov: 70.0,
        };
        let p = [8.0, 66.0, 20.0];
        let (s, _) = cam.project(960, 540, p).expect("in front");
        let dir = cam.ray(960, 540, s[0] as u32, s[1] as u32);
        let to = normalize([p[0] - cam.pos[0], p[1] - cam.pos[1], p[2] - cam.pos[2]]);
        assert!(dot(dir, to) > 0.9999, "dir {dir:?} vs {to:?}");
    }

    #[test]
    fn projection_rejects_points_behind_the_camera() {
        let cam = Camera {
            pos: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 70.0,
        };
        assert!(cam.project(960, 540, [0.0, 0.0, -5.0]).is_none());
        assert!(cam.project(960, 540, [0.0, 0.0, 5.0]).is_some());
    }

    #[test]
    fn occlusion_sees_a_wall_but_not_the_floor_a_marker_rests_on() {
        let mut m = BTreeMap::new();
        // A wall plane at x = 5 and a floor at y = 0.
        for y in -2..8 {
            for z in -8..8 {
                m.insert([5, y, z], "minecraft:stone".to_string());
            }
        }
        for x in -8..5 {
            for z in -8..8 {
                m.insert([x, 0, z], "minecraft:stone".to_string());
            }
        }
        let g = VoxelGrid::build(&m);
        let eye = [0.5, 3.0, 0.5];
        let at = |c: [i32; 3]| Target::point("t".into(), "anchor", String::new(), c);
        // Behind the wall → occluded.
        assert!(g.occluded_target(eye, &at([9, 3, 0])));
        // In the open on this side → not occluded.
        assert!(!g.occluded_target(eye, &at([3, 3, 0])));
        // A marker standing ON the floor is NOT occluded by the floor itself.
        assert!(!g.occluded_target(eye, &at([3, 1, 0])));
        // The plain segment test still reports the wall.
        assert!(g.blocked(eye, [9.5, 3.0, 0.5]));
        assert!(!g.blocked(eye, [3.5, 3.0, 0.5]));
    }

    #[test]
    fn probe_points_stay_inside_a_one_cell_target() {
        let t = Target::point("a".into(), "anchor", String::new(), [4, 70, -3]);
        let ps = t.probe_points();
        assert_eq!(ps[0], t.centre(), "the first probe is the centre");
        for p in ps {
            for a in 0..3 {
                let (lo, hi) = (t.min[a] as f64, t.max[a] as f64 + 1.0);
                assert!(p[a] > lo && p[a] < hi, "probe {p:?} escaped the cell");
            }
        }
        // Distinct points, so nine rays really sample nine sight lines.
        for i in 1..9 {
            assert_ne!(ps[i], ps[0]);
        }
    }

    #[test]
    fn a_marker_that_is_itself_a_block_is_not_reported_occluded_by_itself() {
        // The island field case: `anchor/fire-pit` names the campfire cell, so a
        // naive "did the ray hit anything before the centre" probe called the
        // most visible object in the frame occluded. The target's own box is
        // excluded, so it reads visible — and a block in front of it still hides it.
        let mut m = BTreeMap::new();
        m.insert([0, 0, 10], "minecraft:campfire".to_string());
        let g = VoxelGrid::build(&m);
        let fire = Target::point(
            "anchor/fire-pit".into(),
            "anchor",
            String::new(),
            [0, 0, 10],
        );
        let eye = [0.5, 0.5, 0.0];
        assert!(
            !g.occluded_target(eye, &fire),
            "the marker's own block must not occlude it"
        );
        m.insert([0, 0, 5], "minecraft:stone".to_string());
        let g2 = VoxelGrid::build(&m);
        assert!(
            g2.occluded_target(eye, &fire),
            "a block in front of it must occlude it"
        );
    }

    #[test]
    fn unknown_blocks_render_as_the_loud_fallback() {
        let (c, e) = block_color("minecraft:totally_made_up_block");
        assert_eq!(c, FALLBACK_COLOR);
        assert!(!e);
    }

    #[test]
    fn family_rules_shade_unseen_variants_of_known_materials() {
        // A wood the exact table has never listed still reads as planks, not magenta.
        let (c, _) = block_color("minecraft:cherry_planks");
        assert_ne!(c, FALLBACK_COLOR);
        let (c, _) = block_color("minecraft:crimson_stairs");
        assert_ne!(c, FALLBACK_COLOR);
    }

    #[test]
    fn light_sources_are_emissive() {
        for id in [
            "minecraft:glowstone",
            "minecraft:lantern",
            "minecraft:campfire",
            "minecraft:torch",
            "minecraft:wall_torch",
        ] {
            assert!(block_color(id).1, "{id} must be emissive");
        }
        assert!(!block_color("minecraft:stone").1);
    }

    #[test]
    fn every_library_block_has_a_colour() {
        // Every block id the shipped prefab library places (enumerated from
        // `campaigns/prefabs/*.nbt`) must resolve to a real colour — the magenta
        // fallback is for blocks a future prefab introduces, never for today's.
        // `jigsaw`/`structure_void` are deliberately excluded: they are authoring
        // markers the solver strips, and colouring them magenta is the alarm.
        const LIBRARY: &[&str] = &[
            "andesite",
            "barrel",
            "black_wool",
            "campfire",
            "cartography_table",
            "chain",
            "chest",
            "chiseled_stone_bricks",
            "coarse_dirt",
            "cobblestone",
            "cobblestone_stairs",
            "cobblestone_wall",
            "cornflower",
            "cracked_stone_bricks",
            "dandelion",
            "dark_oak_planks",
            "dead_bush",
            "decorated_pot",
            "dirt",
            "dripstone_block",
            "emerald_block",
            "glass_pane",
            "glow_lichen",
            "glowstone",
            "grass_block",
            "gravel",
            "hay_block",
            "iron_bars",
            "ladder",
            "lantern",
            "light_gray_wool",
            "moss_block",
            "mossy_cobblestone",
            "mossy_stone_bricks",
            "oak_door",
            "oak_fence",
            "oak_fence_gate",
            "oak_leaves",
            "oak_log",
            "oak_planks",
            "oak_slab",
            "oak_stairs",
            "oak_trapdoor",
            "oxeye_daisy",
            "podzol",
            "pointed_dripstone",
            "polished_andesite",
            "poppy",
            "rooted_dirt",
            "sand",
            "seagrass",
            "short_grass",
            "spruce_button",
            "spruce_log",
            "spruce_planks",
            "spruce_stairs",
            "spruce_trapdoor",
            "stone",
            "stone_brick_stairs",
            "stone_bricks",
            "stripped_oak_log",
            "stripped_spruce_log",
            "suspicious_gravel",
            "torch",
            "tuff",
            "vine",
            "wall_torch",
            "water",
            "white_banner",
            "white_glazed_terracotta",
            "white_wool",
        ];
        for id in LIBRARY {
            let full = format!("minecraft:{id}");
            assert_ne!(
                block_color(&full).0,
                FALLBACK_COLOR,
                "prefab-library block `{full}` has no colour — extend the palette"
            );
        }
    }

    #[test]
    fn target_boxes_and_centres_cover_whole_cells() {
        let p = Target::point("anchor/x".into(), "anchor", "area/a".into(), [4, 70, -3]);
        assert!(p.is_point());
        assert_eq!(p.centre(), [4.5, 70.5, -2.5]);
        assert_eq!(p.label(), "X");
        let r = Target {
            id: "trigger/t".into(),
            kind: "trigger",
            area: String::new(),
            min: [0, 0, 0],
            max: [1, 1, 1],
        };
        assert!(!r.is_point());
        assert_eq!(r.centre(), [1.0, 1.0, 1.0]);
        // Corners span the full outer surface of the inclusive cell box.
        let cs = r.corners();
        assert!(cs.contains(&[0.0, 0.0, 0.0]) && cs.contains(&[2.0, 2.0, 2.0]));
    }

    #[test]
    fn screen_box_clips_to_the_frame_and_rejects_offscreen_targets() {
        // Eye at the centre of cell [0,0,0], so a cell straight down +Z is
        // symmetric about the optical axis.
        let cam = Camera {
            pos: [0.5, 0.5, 0.5],
            yaw: 0.0,
            pitch: 0.0,
            fov: 70.0,
        };
        // Straight ahead → a box around screen centre.
        let ahead = Target::point("a".into(), "anchor", String::new(), [0, 0, 20]);
        let b = screen_box(&cam, 960, 540, &ahead).expect("in frustum");
        assert!(b.x < 480 && b.x + b.w > 480, "centred: {b:?}");
        // Directly behind → out of frustum entirely.
        let behind = Target::point("b".into(), "anchor", String::new(), [0, 0, -20]);
        assert!(screen_box(&cam, 960, 540, &behind).is_none());
        // Far to the side → in front but off-frame.
        let aside = Target::point("c".into(), "anchor", String::new(), [400, 0, 5]);
        assert!(screen_box(&cam, 960, 540, &aside).is_none());
    }

    #[test]
    fn frame_render_is_byte_stable_and_fills_every_pixel() {
        let mut m = BTreeMap::new();
        for x in -4..4 {
            for z in -4..4 {
                m.insert([x, 0, z], "minecraft:grass_block".to_string());
            }
        }
        m.insert([0, 1, 0], "minecraft:glowstone".to_string());
        let g = VoxelGrid::build(&m);
        // yaw 315 aims +X/+Z (north-west corner looking south-east), so the
        // camera at (−8, −8) actually faces the patch of ground it renders.
        let cam = Camera {
            pos: [-8.0, 5.0, -8.0],
            yaw: 315.0,
            pitch: 25.0,
            fov: 70.0,
        };
        let opts = FrameOpts {
            width: 64,
            height: 36,
            ..FrameOpts::default()
        };
        let a = render_frame(&g, &cam, &opts);
        let b = render_frame(&g, &cam, &opts);
        assert_eq!(
            a.canvas.rgba, b.canvas.rgba,
            "ADR-0006: rendering is deterministic"
        );
        assert!(a.canvas.rgba.chunks(4).all(|p| p[3] == 255), "opaque frame");
        // The lit block renders at its full emissive colour somewhere in frame.
        let want = block_color("minecraft:glowstone").0;
        assert!(
            a.canvas.rgba.chunks(4).any(|p| p[..3] == want),
            "emissive block must render unshaded"
        );
    }

    #[test]
    fn a_camera_inside_a_block_sees_that_block() {
        let g = one_block("minecraft:stone");
        let h = g
            .cast([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], 100.0)
            .expect("inside a block");
        assert_eq!((h.cell, h.t), ([0, 0, 0], 0.0));
    }

    #[test]
    fn ocean_backdrop_only_appears_below_the_horizon_for_ocean_campaigns() {
        let cam = Camera {
            pos: [0.0, 80.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 70.0,
        };
        let down = normalize([0.0, -1.0, 1.0]);
        let sea = FrameOpts {
            sea_level: Some(62),
            ..FrameOpts::default()
        };
        let void = FrameOpts::default();
        assert_ne!(
            background(&cam, down, &sea),
            background(&cam, down, &void),
            "the sea plane must change a downward background ray"
        );
        let up = normalize([0.0, 1.0, 1.0]);
        assert_eq!(
            background(&cam, up, &sea),
            background(&cam, up, &void),
            "the sea plane must not touch the sky"
        );
    }

    #[test]
    fn text_stamping_stays_inside_the_frame() {
        // A label placed off the edges must clip rather than panic or wrap.
        let g = one_block("minecraft:stone");
        let cam = Camera {
            pos: [0.5, 0.5, -5.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 70.0,
        };
        let mut f = render_frame(
            &g,
            &cam,
            &FrameOpts {
                width: 32,
                height: 24,
                ..FrameOpts::default()
            },
        );
        f.canvas.stamp_text(-20, -20, "EDGE", 1, [255, 255, 255]);
        f.canvas.stamp_text(100, 100, "EDGE", 2, [255, 255, 255]);
        assert_eq!(f.canvas.rgba.len(), 32 * 24 * 4);
    }
}
