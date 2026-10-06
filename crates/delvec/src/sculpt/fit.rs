//! **From sub-voxels to blocks** — steps 2–7 of spec-0087 §3.3, a port of the
//! organic-voxel research's back half (`tools/spike-organic-voxel/voxelize.py`,
//! `docs/reference/organic-structures.md` §3).
//!
//! 1. **Octant fit.** Each block is 2 × 2 × 2 octants; an octant's fraction is
//!    the share of its sub-voxels that are solid. The block becomes air, a full
//!    block, a bottom or top slab, or a straight stair of any facing and half —
//!    whichever minimises the summed occupancy error, with a cost bias against
//!    the sub-block shapes so a curved surface does not pit.
//! 2. **Thin-plate refit.** Where the fit says air, the block is refitted from
//!    the solid thickened by one sub-voxel, so a plate thinner than half an
//!    octant is kept; thick material is never refitted, so it never bloats; and
//!    a shelf's headroom is never refitted, so a walkway stays clear over it.
//! 3. **The apron.** Every air block at `y <= ground.top` becomes ground.
//! 4. **Islands.** A disconnected group of fewer than [`MIN_COMPONENT`] blocks
//!    is dropped and counted.
//! 5. **Lights**, each replacing its cell.
//! 6. **Stair corners** by [`crate::schem::stairs::derive_shape`] over the final
//!    grid — the fitter never chooses a corner.
//! 7. **Tone and material.** A tone from the smoothed surface normal, the
//!    openness and a seeded low-frequency field; a block by tone and palette;
//!    every axis-bearing block by its longest local run.
//!
//! Every smoothing here is a cascade of three box filters (a Gaussian to within
//! a few per cent, and exact in IEEE arithmetic), so nothing calls a
//! transcendental function and the bytes are the same on every platform.

use std::collections::BTreeMap;

use crate::grammar::block::BlockState;
use crate::grammar::rng::Rng;
use crate::schem::stairs::{Facing, Half, Stair, derive_shape};
use crate::sculpt::form::{Form, Tone, parse_state};
use crate::sculpt::grid::{self, Owners, SubGrid, normal_draw};

/// What the fit put in a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Nothing.
    Air,
    /// A full block of the body's material.
    Full,
    /// A bottom slab.
    SlabBottom,
    /// A top slab.
    SlabTop,
    /// A straight stair (its corner is derived later).
    Stair(Facing, Half),
    /// The ground.
    Ground,
    /// A declared light, by its index in `lights[]`.
    Light(usize),
    /// The partial block a `recessed` hull light sits behind, by the light's
    /// index in `lights[]`, with the orientation the sculpt chose.
    Cover(usize, CoverShape),
}

/// How a cover is oriented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverShape {
    /// A stair of this facing and half (its corner is derived later).
    Stair(Facing, Half),
    /// A slab, `true` for a top slab.
    Slab(bool),
}

impl Kind {
    /// The stair this block is to vanilla's corner rule, fitted or cover.
    pub(crate) fn as_stair(self) -> Option<Stair> {
        match self {
            Kind::Stair(facing, half) | Kind::Cover(_, CoverShape::Stair(facing, half)) => {
                Some(Stair { facing, half })
            }
            _ => None,
        }
    }
}

impl Kind {
    fn solid(self) -> bool {
        self != Kind::Air
    }
}

/// The research's cost bias against a stair, in octant units: a stair must beat
/// a full block or air by this margin.
pub const STAIR_BIAS: f64 = 0.6;
/// The same bias for a slab.
pub const SLAB_BIAS: f64 = 0.4;
/// A disconnected group of blocks smaller than this is dropped.
pub const MIN_COMPONENT: usize = 4;

/// The facings in the research's order, with the octant side each fills.
const FACINGS: [Facing; 4] = [Facing::North, Facing::East, Facing::South, Facing::West];

/// The twelve candidates, each an 8-vector over octants `o = ox + 2·oy + 4·oz`
/// (`ox` 0 west / 1 east, `oy` 0 bottom / 1 top, `oz` 0 north / 1 south), in the
/// research's order: full, air, two slabs, then each facing's bottom and top
/// stair. The order breaks ties, so it is part of the rule.
fn candidates() -> Vec<(Kind, [f64; 8], f64)> {
    let mask = |pred: &dyn Fn(usize, usize, usize) -> bool| {
        let mut m = [0.0; 8];
        for (o, v) in m.iter_mut().enumerate() {
            if pred(o & 1, (o >> 1) & 1, (o >> 2) & 1) {
                *v = 1.0;
            }
        }
        m
    };
    let mut out = vec![
        (Kind::Full, mask(&|_, _, _| true), 0.0),
        (Kind::Air, mask(&|_, _, _| false), 0.0),
        (Kind::SlabBottom, mask(&|_, y, _| y == 0), SLAB_BIAS),
        (Kind::SlabTop, mask(&|_, y, _| y == 1), SLAB_BIAS),
    ];
    for facing in FACINGS {
        // The full-height half of a stair is on the side it faces.
        let side = move |x: usize, z: usize| match facing {
            Facing::North => z == 0,
            Facing::East => x == 1,
            Facing::South => z == 1,
            Facing::West => x == 0,
        };
        out.push((
            Kind::Stair(facing, Half::Bottom),
            mask(&|x, y, z| y == 0 || side(x, z)),
            STAIR_BIAS,
        ));
        out.push((
            Kind::Stair(facing, Half::Top),
            mask(&|x, y, z| y == 1 || side(x, z)),
            STAIR_BIAS,
        ));
    }
    out
}

/// The block grid the fit produces, x-major (`(x*Y + y)*Z + z`).
pub struct Blocks {
    /// Extent in blocks.
    pub dims: [usize; 3],
    /// What each block is.
    pub kind: Vec<Kind>,
    /// Mean octant fill per block, `0..=1` — what the tone reads.
    pub fill: Vec<f64>,
}

impl Blocks {
    #[inline]
    pub(crate) fn index(&self, x: usize, y: usize, z: usize) -> usize {
        (x * self.dims[1] + y) * self.dims[2] + z
    }

    pub(crate) fn kind_at(&self, x: i64, y: i64, z: i64) -> Kind {
        if x < 0 || y < 0 || z < 0 {
            return Kind::Air;
        }
        let (x, y, z) = (x as usize, y as usize, z as usize);
        if x >= self.dims[0] || y >= self.dims[1] || z >= self.dims[2] {
            return Kind::Air;
        }
        self.kind[self.index(x, y, z)]
    }
}

/// The octant fractions of one block.
fn fractions(grid: &SubGrid, x: usize, y: usize, z: usize) -> [f64; 8] {
    let h = grid.sub / 2;
    let per = (h * h * h) as f64;
    let mut out = [0.0; 8];
    for (o, slot) in out.iter_mut().enumerate() {
        let (ox, oy, oz) = (o & 1, (o >> 1) & 1, (o >> 2) & 1);
        let mut n = 0usize;
        for i in 0..h {
            for j in 0..h {
                for k in 0..h {
                    if grid.get(
                        (x * grid.sub + ox * h + i) as i64,
                        (y * grid.sub + oy * h + j) as i64,
                        (z * grid.sub + oz * h + k) as i64,
                    ) {
                        n += 1;
                    }
                }
            }
        }
        *slot = n as f64 / per;
    }
    out
}

/// Whether a candidate may stand in a block of walkway with this climb
/// ([`crate::sculpt::grid::Owners::climb`]): anything off a walkway; no stair
/// on a level stretch; on a climbing stretch, only a stair facing the climb.
fn allowed(kind: Kind, climb: u8) -> bool {
    match (kind, climb) {
        (_, 0) => true,
        (Kind::Stair(..), grid::FLAT) => false,
        (Kind::Stair(facing, _), c) => grid::climb_code(facing) == c,
        _ => true,
    }
}

/// The least-error candidate for one block's fractions, among those its
/// walkway climb allows.
fn best(cands: &[(Kind, [f64; 8], f64)], frac: &[f64; 8], climb: u8) -> Kind {
    let mut best_kind = Kind::Full;
    let mut best_cost = f64::INFINITY;
    for (kind, mask, bias) in cands {
        if !allowed(*kind, climb) {
            continue;
        }
        let mut cost = *bias;
        for o in 0..8 {
            cost += (frac[o] - mask[o]).abs();
        }
        if cost < best_cost - 1e-6 {
            best_cost = cost;
            best_kind = *kind;
        }
    }
    best_kind
}

/// What the fit counted, for the command's binding lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FitCounts {
    /// Blocks the thin-plate refit filled.
    pub thin_filled: usize,
    /// Air blocks the apron laid.
    pub apron_laid: usize,
    /// Blocks dropped as islands.
    pub islands_dropped: usize,
    /// Connected components kept.
    pub components: usize,
}

/// Steps 1–5: the octant fit, the refit, the apron, the islands and the lights.
/// A block of walkway is fitted only to the shapes its climb allows.
pub fn fit(form: &Form, grid: &SubGrid, owners: &Owners) -> (Blocks, FitCounts) {
    let dims = form.extent.map(|v| v as usize);
    let n = dims[0] * dims[1] * dims[2];
    let cands = candidates();
    let mut blocks = Blocks {
        dims,
        kind: vec![Kind::Air; n],
        fill: vec![0.0; n],
    };
    let mut counts = FitCounts::default();
    for x in 0..dims[0] {
        for y in 0..dims[1] {
            for z in 0..dims[2] {
                let frac = fractions(grid, x, y, z);
                let i = blocks.index(x, y, z);
                blocks.kind[i] = best(&cands, &frac, owners.climb[i]);
                blocks.fill[i] = frac.iter().sum::<f64>() / 8.0;
            }
        }
    }
    // The thin-plate refit, where the fit says air.
    let thick = grid.thickened();
    for x in 0..dims[0] {
        for y in 0..dims[1] {
            for z in 0..dims[2] {
                let i = blocks.index(x, y, z);
                if blocks.kind[i] != Kind::Air || owners.headroom[i] {
                    continue;
                }
                let frac = fractions(&thick, x, y, z);
                if frac.iter().all(|f| *f == 0.0) {
                    continue;
                }
                let k = best(&cands, &frac, owners.climb[i]);
                if k != Kind::Air {
                    blocks.kind[i] = k;
                    blocks.fill[i] = frac.iter().sum::<f64>() / 8.0;
                    counts.thin_filled += 1;
                }
            }
        }
    }
    drop(thick);
    // The apron.
    let top = form.ground.as_ref().map_or(-1, |g| g.top as i64);
    for x in 0..dims[0] {
        for y in 0..dims[1].min((top + 1).max(0) as usize) {
            for z in 0..dims[2] {
                let i = blocks.index(x, y, z);
                if blocks.kind[i] == Kind::Air {
                    blocks.kind[i] = Kind::Ground;
                    blocks.fill[i] = 1.0;
                    counts.apron_laid += 1;
                }
            }
        }
    }
    // Islands: 26-connected components of every non-air block.
    let (dropped, kept) = drop_islands(&mut blocks);
    counts.islands_dropped = dropped;
    counts.components = kept;
    // Lights placed by hand replace their cells; `hull` lights are placed
    // after, by [`crate::sculpt::hull::place`], over the finished surface.
    let floor = form.body_floor();
    for (li, l) in form.lights.iter().enumerate() {
        let Some(at) = l.at else { continue };
        let (x, y, z) = (
            at[0] as usize,
            (at[1] as i64 + floor) as usize,
            at[2] as usize,
        );
        let i = blocks.index(x, y, z);
        blocks.kind[i] = Kind::Light(li);
    }
    (blocks, counts)
}

/// Drop every 26-connected group of blocks smaller than [`MIN_COMPONENT`];
/// return `(blocks dropped, components kept)`.
fn drop_islands(blocks: &mut Blocks) -> (usize, usize) {
    let [dx, dy, dz] = blocks.dims;
    let n = dx * dy * dz;
    let mut label = vec![0u32; n];
    let mut next = 0u32;
    let mut dropped = 0usize;
    let mut kept = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut members: Vec<usize> = Vec::new();
    for start in 0..n {
        if label[start] != 0 || !blocks.kind[start].solid() {
            continue;
        }
        next += 1;
        label[start] = next;
        stack.push(start);
        members.clear();
        while let Some(i) = stack.pop() {
            members.push(i);
            let (x, y, z) = (i / (dy * dz), (i / dz) % dy, i % dz);
            for ox in -1i64..=1 {
                for oy in -1i64..=1 {
                    for oz in -1i64..=1 {
                        let (nx, ny, nz) = (x as i64 + ox, y as i64 + oy, z as i64 + oz);
                        if nx < 0
                            || ny < 0
                            || nz < 0
                            || nx >= dx as i64
                            || ny >= dy as i64
                            || nz >= dz as i64
                        {
                            continue;
                        }
                        let j = (nx as usize * dy + ny as usize) * dz + nz as usize;
                        if label[j] == 0 && blocks.kind[j].solid() {
                            label[j] = next;
                            stack.push(j);
                        }
                    }
                }
            }
        }
        if members.len() < MIN_COMPONENT {
            dropped += members.len();
            for &i in &members {
                blocks.kind[i] = Kind::Air;
            }
        } else {
            kept += 1;
        }
    }
    (dropped, kept)
}

/// Step 6: each stair's `shape`, derived by vanilla's rule over the final grid.
pub fn stair_shapes(blocks: &Blocks) -> BTreeMap<usize, &'static str> {
    let mut out = BTreeMap::new();
    let [dx, dy, dz] = blocks.dims;
    for x in 0..dx {
        for y in 0..dy {
            for z in 0..dz {
                let i = blocks.index(x, y, z);
                let Some(stair) = blocks.kind[i].as_stair() else {
                    continue;
                };
                let neighbour = |f: Facing| {
                    let s = f.step();
                    blocks
                        .kind_at(x as i64 + s[0] as i64, y as i64, z as i64 + s[2] as i64)
                        .as_stair()
                };
                out.insert(i, derive_shape(stair, neighbour).as_str());
            }
        }
    }
    out
}

/// A dense scalar field over the block grid.
struct Field {
    dims: [usize; 3],
    v: Vec<f64>,
}

impl Field {
    /// One box-filter pass of odd width `w` along `axis`, edges clamped.
    fn box_pass(&self, w: usize, axis: usize) -> Field {
        let [dx, dy, dz] = self.dims;
        let r = (w / 2) as i64;
        let stride = match axis {
            0 => dy * dz,
            1 => dz,
            _ => 1,
        };
        let len = self.dims[axis];
        let mut out = vec![0.0; self.v.len()];
        for (idx, o) in out.iter_mut().enumerate() {
            let pos = ((idx / stride) % len) as i64;
            let base = idx as i64 - pos * stride as i64;
            let mut s = 0.0;
            for d in -r..=r {
                let p = (pos + d).clamp(0, len as i64 - 1);
                s += self.v[(base + p * stride as i64) as usize];
            }
            *o = s / w as f64;
        }
        let _ = (dx, dy, dz);
        Field {
            dims: self.dims,
            v: out,
        }
    }

    /// Three box passes per axis: a smoothing of variance `(w² − 1) / 4`.
    fn smooth(&self, w: usize) -> Field {
        let mut f = Field {
            dims: self.dims,
            v: self.v.clone(),
        };
        for axis in 0..3 {
            for _ in 0..3 {
                f = f.box_pass(w, axis);
            }
        }
        f
    }

    /// Normalised to zero mean and unit standard deviation.
    fn standardised(mut self) -> Field {
        let n = self.v.len() as f64;
        let mean = self.v.iter().sum::<f64>() / n;
        let var = self.v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n;
        let sd = var.sqrt() + 1e-9;
        for x in &mut self.v {
            *x = (*x - mean) / sd;
        }
        self
    }
}

/// The tone thresholds on `−t`, bleached to weathered (the research's).
const TONE_CUTS: [f64; 3] = [-0.35, 0.15, 0.55];

/// Step 7a: a tone `0..=3` per block, and a member pick `0..1` per block.
pub fn shade(blocks: &Blocks, seed: u64) -> (Vec<u8>, Vec<f64>) {
    let dims = blocks.dims;
    let fill = Field {
        dims,
        v: blocks.fill.clone(),
    };
    let g = fill.smooth(3);
    let crevice = fill.smooth(3);
    let body = fill.smooth(9);
    let mut rng = Rng::new(seed ^ 0x0070_4E50_FA11_u64);
    let noise = Field {
        dims,
        v: (0..blocks.kind.len())
            .map(|_| normal_draw(&mut rng))
            .collect(),
    }
    .smooth(5)
    .standardised();
    let pick_field = Field {
        dims,
        v: (0..blocks.kind.len())
            .map(|_| normal_draw(&mut rng))
            .collect(),
    }
    .smooth(3)
    .standardised();
    let [dx, dy, dz] = dims;
    let at = |f: &Field, x: i64, y: i64, z: i64| {
        let x = x.clamp(0, dx as i64 - 1) as usize;
        let y = y.clamp(0, dy as i64 - 1) as usize;
        let z = z.clamp(0, dz as i64 - 1) as usize;
        f.v[(x * dy + y) * dz + z]
    };
    let mut tone = vec![0u8; blocks.kind.len()];
    let mut pick = vec![0.0; blocks.kind.len()];
    for x in 0..dx {
        for y in 0..dy {
            for z in 0..dz {
                let i = (x * dy + y) * dz + z;
                let (xi, yi, zi) = (x as i64, y as i64, z as i64);
                let gx = (at(&g, xi + 1, yi, zi) - at(&g, xi - 1, yi, zi)) / 2.0;
                let gy = (at(&g, xi, yi + 1, zi) - at(&g, xi, yi - 1, zi)) / 2.0;
                let gz = (at(&g, xi, yi, zi + 1) - at(&g, xi, yi, zi - 1)) / 2.0;
                let norm = (gx * gx + gy * gy + gz * gz).sqrt() + 1e-6;
                let up = -gy / norm;
                let t = 0.6 * up
                    + 1.2 * ((1.0 - crevice.v[i]) - 0.55)
                    + 0.6 * ((1.0 - body.v[i]) - 0.85)
                    + 0.30 * noise.v[i];
                tone[i] = TONE_CUTS.iter().filter(|c| **c <= -t).count() as u8;
                pick[i] = (0.5 + 0.2 * pick_field.v[i]).clamp(0.0, 0.999_999);
            }
        }
    }
    (tone, pick)
}

/// Every property of `state` the block has and the state does not write,
/// filled from the pinned registry's default — never a shape-carrying one
/// (`Form::check` refuses a full block that has any, and `states-complete`
/// reds on anything left unwritten).
fn complete(mut state: BlockState) -> BlockState {
    let registry = crate::schem::blocks::BlockRegistry::v1_21_11();
    let shape_carrying = registry.shape_carrying(&state.name).to_vec();
    if let Some(defaults) = registry.default_state(&state.name) {
        for (k, v) in defaults {
            if !shape_carrying.contains(k) {
                state
                    .properties
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }
        }
    }
    state
}

/// The member of a tone's full blocks a pick in `0..1` lands on.
fn member(tone: &Tone, pick: f64) -> &str {
    let total: u64 = tone.full.iter().map(|(_, w)| *w as u64).sum();
    let mut r = pick * total as f64;
    for (block, w) in &tone.full {
        if r < *w as f64 {
            return block;
        }
        r -= *w as f64;
    }
    &tone.full.last().expect("a tone lists a full block").0
}

/// The axis of the longest run of solid blocks through a block within ±3.
fn run_axis(blocks: &Blocks, x: usize, y: usize, z: usize) -> &'static str {
    let mut best = (0usize, "x");
    for (axis, name) in [(0usize, "x"), (1, "y"), (2, "z")] {
        let mut n = 0usize;
        for d in -3i64..=3 {
            let mut p = [x as i64, y as i64, z as i64];
            p[axis] += d;
            if blocks.kind_at(p[0], p[1], p[2]).solid() {
                n += 1;
            }
        }
        if n > best.0 {
            best = (n, name);
        }
    }
    best.1
}

/// Step 7b: the block state of every cell, in the model's x-major order.
pub fn states(
    form: &Form,
    blocks: &Blocks,
    owners: &Owners,
    tone: &[u8],
    pick: &[f64],
) -> Vec<Option<BlockState>> {
    let registry = crate::schem::blocks::BlockRegistry::v1_21_11();
    let shapes = stair_shapes(blocks);

    let parsed = |s: &str| parse_state(s).expect("Form::check parsed every block");
    let ground = form.ground.as_ref().map(|g| parsed(&g.block));
    let [dx, dy, dz] = blocks.dims;
    let mut out = vec![None; blocks.kind.len()];
    for x in 0..dx {
        for y in 0..dy {
            for z in 0..dz {
                let i = blocks.index(x, y, z);
                let material = match owners.owner[i] {
                    0 => &form.palette[tone[i] as usize],
                    s => form.solids[s as usize - 1]
                        .material()
                        .unwrap_or(&form.palette[tone[i] as usize]),
                };
                let full_with_axis = |mut state: BlockState| {
                    if registry
                        .properties(&state.name)
                        .is_some_and(|p| p.contains_key("axis"))
                    {
                        state
                            .properties
                            .insert("axis".to_string(), run_axis(blocks, x, y, z).to_string());
                    }
                    complete(state)
                };
                let state = match blocks.kind[i] {
                    Kind::Air => continue,
                    Kind::Full => full_with_axis(parsed(member(material, pick[i]))),
                    Kind::Ground => full_with_axis(ground.clone().expect("a form has ground")),
                    Kind::Light(li) => complete(parsed(&form.lights[li].block)),
                    Kind::Cover(li, shape) => {
                        let cover = form.lights[li]
                            .hull
                            .as_ref()
                            .and_then(|h| h.cover.as_deref())
                            .expect("Form::check: a recessed hull light names a cover");
                        let mut s = parsed(cover);
                        match shape {
                            CoverShape::Stair(facing, half) => {
                                s.properties
                                    .insert("facing".to_string(), facing.as_str().to_string());
                                s.properties
                                    .insert("half".to_string(), half.as_str().to_string());
                                s.properties
                                    .insert("shape".to_string(), shapes[&i].to_string());
                            }
                            CoverShape::Slab(top) => {
                                s.properties.insert(
                                    "type".to_string(),
                                    if top { "top" } else { "bottom" }.to_string(),
                                );
                            }
                        }
                        complete(s)
                    }
                    Kind::SlabBottom | Kind::SlabTop => {
                        let mut s = parsed(&format!("minecraft:{}_slab", material.family));
                        s.properties.insert(
                            "type".to_string(),
                            if blocks.kind[i] == Kind::SlabBottom {
                                "bottom"
                            } else {
                                "top"
                            }
                            .to_string(),
                        );
                        complete(s)
                    }
                    Kind::Stair(facing, half) => {
                        let mut s = parsed(&format!("minecraft:{}_stairs", material.family));
                        s.properties
                            .insert("facing".to_string(), facing.as_str().to_string());
                        s.properties
                            .insert("half".to_string(), half.as_str().to_string());
                        s.properties
                            .insert("shape".to_string(), shapes[&i].to_string());
                        complete(s)
                    }
                };
                out[i] = Some(state);
            }
        }
    }
    out
}
