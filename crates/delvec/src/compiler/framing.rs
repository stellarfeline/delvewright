//! **What a frame frames** (spec-0112 §7.5): the camera, the occupancy grid a
//! sight line is asked of, the targets a review names, and the manifest that
//! says per target where it stands on the frame and whether it can be seen.
//!
//! One definition of each, read by every surface that describes a frame:
//! `delvec frames` (each review frame's `shows` and `out_of_frame`), the per-batch
//! frames of `delvec edit`, the blocking chart's labels and the prefab viewer's
//! eye-level measurements ([`crate::compiler::view::sight`]). The pixels are
//! Lumenwright's ([`crate::compiler::view::frames`]); nothing here draws a
//! block.
//!
//! ## Camera convention
//!
//! `yaw`/`pitch` are **Minecraft** degrees, identical to the cutscene aim and
//! to `render-plan.json`: `yaw = 0` faces +Z (south), `90` faces −X (west),
//! `180` north, `270` east; `pitch` is positive looking **down**, `0` level.
//! `fov` is the **vertical** field of view in degrees.
//!
//! ## One sight rule
//!
//! A target is occluded when all nine of its probe points
//! ([`Target::probe_points`]) are hidden by a block that is not the target's
//! own ([`VoxelGrid::occluded_target`]). [`resolve_targets`] is the one place a
//! target is put on a frame.
//!
//! ## Determinism (ADR-0006)
//!
//! No RNG, no clock, no parallelism, no hash-order iteration: the grid's
//! palette is built from a `BTreeMap` walk, targets are sorted by
//! `(kind, area, id)`, and floats in the manifest are rounded to 3 decimals.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::compiler::plan::{Plan, ResolvedAnchor};
use crate::compiler::raster::ScreenBox;

/// Default vertical field of view in degrees (vanilla's default is 70).
pub const DEFAULT_FOV: f64 = 70.0;
/// Default `--dist` for `--at`/`--orbit` framing (blocks from the subject).
pub const DEFAULT_ORBIT_DIST: f64 = 14.0;

// ---------------------------------------------------------------------------
// Voxel grid
// ---------------------------------------------------------------------------

/// Edge length of one storage chunk (matching vanilla's section size).
const CHUNK: i32 = 16;
/// Cells per storage chunk.
const CHUNK_CELLS: usize = (CHUNK * CHUNK * CHUNK) as usize;

/// A dense-chunked occupancy view of a block map, built once per world.
///
/// The block map is a `BTreeMap<[i32;3], BlockState>`; a sight line probes a
/// cell per DDA step, and a review set asks nine lines per target per frame, so
/// a `BTreeMap` lookup per step is too slow. This flattens it into
/// a palette plus 16³ chunk arrays: probes are O(1) array indexing, memory stays
/// proportional to *occupied* chunks (a layout spanning `AREA_SPACING`-separated
/// areas allocates nothing for the void between them), and the palette walk is a
/// `BTreeMap` iteration so palette indices are deterministic.
pub struct VoxelGrid {
    /// Palette entry 0 is always "air"; block ids follow in `BTreeMap` order.
    palette: Vec<String>,
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
    /// Flatten a cell→block map into the occupancy grid.
    pub fn build(blocks: &crate::compiler::blockstate::BlockMap) -> VoxelGrid {
        let mut palette: Vec<String> = vec!["minecraft:air".to_string()];
        let mut index: BTreeMap<&str, u16> = BTreeMap::new();
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for (cell, name) in blocks {
            if !index.contains_key(name.as_str()) {
                index.insert(name.as_str(), palette.len() as u16);
                palette.push(name.to_string());
            }
            for a in 0..3 {
                lo[a] = lo[a].min(cell[a]);
                hi[a] = hi[a].max(cell[a]);
            }
        }
        if blocks.is_empty() {
            return VoxelGrid {
                palette,
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

/// One ray intersection.
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
    /// Where on the entered face the ray landed, both coordinates in `0.0..1.0`.
    pub uv: [f64; 2],
}

/// The in-face coordinates (both in `0.0..1.0`) of a hit point on a face whose
/// normal runs along `axis`.
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
    ///   target is a few hundred cell probes.
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
    /// formula [`crate::compiler::emit`]'s cutscene `mc_aim` uses, so a camera derived from
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
    pub fn point(id: String, kind: &'static str, area: String, pos: [i32; 3]) -> Target {
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
        if let Some(pos) = plan
            .point(&area, npc.anchor.as_str())
            .map(|p| delvewright_dsl::offset_cell(p, npc.offset))
        {
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
                delvewright_dsl::offset_cell(pos, actor.offset),
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
        let Some((area, pos)) = t.at_anchor().and_then(|at| resolve_any_area(plan, at)) else {
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

/// **The body a target is drawn as**, by entity id without its namespace, when
/// the target is a body the datapack summons: an NPC's post is its mannequin
/// (a skinned NPC) or its declared entity; an actor's post its puppet; a
/// marked `interact` objective with no prop is its glowing lantern
/// `item_display`. `None` for every other target. A review frame reads the
/// renderer's own statistics for that kind ([`crate::compiler::view::frames::Stats`])
/// to say whether the body was drawn (`DW1039`).
pub fn body_of(plan: &Plan, t: &Target) -> Option<String> {
    let short = |e: &str| e.strip_prefix("minecraft:").unwrap_or(e).to_string();
    let c = plan.campaign;
    match t.kind {
        "npc-post" => c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == t.id)
            .map(|n| {
                if n.skin.is_some() {
                    "mannequin".to_string()
                } else {
                    short(&n.base_entity)
                }
            }),
        "actor-post" => c
            .quests
            .content
            .actors
            .iter()
            .find(|a| a.id.as_str() == t.id)
            .map(|a| {
                if a.skin.is_some() {
                    "mannequin".to_string()
                } else {
                    short(&a.entity)
                }
            }),
        "interact" => c
            .quests
            .content
            .quests
            .iter()
            .flat_map(|q| q.objectives.iter())
            .find(|o| interact_of(o).is_some_and(|(id, _)| id == t.id))
            .filter(|o| o.prop().is_none() && o.marker_shown(&c.quests.content.guidance))
            .map(|_| "item_display".to_string()),
        _ => None,
    }
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
    width: u32,
    height: u32,
    targets: &'a [Target],
) -> (Vec<ResolvedTarget<'a>>, Vec<&'a Target>) {
    let mut inside = Vec::new();
    let mut outside = Vec::new();
    for t in targets {
        match screen_box(cam, width, height, t) {
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

/// The picture a manifest describes: its size and its pixels, read for the
/// `featureless` verdict and never hashed.
pub struct Picture<'a> {
    pub width: u32,
    pub height: u32,
    /// RGBA8, row major.
    pub rgba: &'a [u8],
}

/// Build the frame manifest — the structured half of a frame.
///
/// Schema (v3):
/// ```text
/// { manifest_version, campaign_id, delvec, image{path,width,height},
///   camera{pos,yaw,pitch,fov,convention}, world{block_kinds,bounds},
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
    picture: &Picture<'_>,
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
        "manifest_version": 3,
        "campaign_id": campaign_id,
        "delvec": crate::compiler::DELVEC_VERSION,
        "image": {
            "path": image_path,
            "width": picture.width,
            "height": picture.height,
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
        // What this frame is worth to a reviewer, judged by the arm that drew
        // it. Two numbers, and both are the CLAUDE.md binding rule applied to a
        // picture: `targets` is what the shot was aimed at and `featureless` is
        // whether it shows anything at all. A render that succeeds, writes a
        // file, and is a rectangle of flat background looks exactly like one
        // more shot taken — to a directory listing, to a contact sheet, and to
        // a reviewer skimming. The producer answers it here because a verdict
        // computed by whoever consumes the file later is a second authority on
        // one question.
        "frame": {
            "targets_in_frame": inside.len(),
            "targets_out_of_frame": outside.len(),
            "featureless": crate::compiler::view::detect::is_featureless(
                picture.rgba,
                picture.width,
                picture.height,
            )
            .map(|f| json!({ "distinct_colors": f.distinct }))
            .unwrap_or(Value::Null),
        },
    })
}

/// The world-space half of a manifest entry (shared by `targets` and
/// `out_of_frame`).
pub fn target_json(t: &Target) -> Value {
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


#[cfg(test)]
mod tests {
    use super::*;

    /// A 1-block world at the origin cell.
    fn one_block(name: &str) -> VoxelGrid {
        let mut m = BTreeMap::new();
        m.insert([0, 0, 0], name.to_string());
        VoxelGrid::build(&crate::compiler::blockstate::interned(m.clone()))
    }

    #[test]
    fn grid_round_trips_cells_including_negative_coordinates() {
        let mut m = BTreeMap::new();
        m.insert([0, 64, 0], "minecraft:stone".to_string());
        m.insert([-33, -5, -17], "minecraft:sand".to_string());
        m.insert([200, 70, 120], "minecraft:water".to_string());
        let g = VoxelGrid::build(&crate::compiler::blockstate::interned(m.clone()));
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
        let g = VoxelGrid::build(&crate::compiler::blockstate::interned(m.clone()));
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
        let g = VoxelGrid::build(&crate::compiler::blockstate::interned(m.clone()));
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
        let g = VoxelGrid::build(&crate::compiler::blockstate::interned(m.clone()));
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
        let g2 = VoxelGrid::build(&crate::compiler::blockstate::interned(m.clone()));
        assert!(
            g2.occluded_target(eye, &fire),
            "a block in front of it must occlude it"
        );
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
}
