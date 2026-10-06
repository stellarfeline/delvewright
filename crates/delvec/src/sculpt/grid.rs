//! **The solid sub-voxel grid** — step 1 of spec-0087 §3.3: `solids[]` stamped
//! in order, `add` and `cut`, with seeded weathering where a solid is `noisy`.
//!
//! Every sample is the centre of a sub-voxel, in continuous block units, and
//! every quantity is computed with `+ − × ÷`, `sqrt`, `abs`, `min`, `max` and
//! `floor` only — operations IEEE 754 defines exactly — so the grid is the same
//! bytes on every platform (ADR-0006). The one source of randomness is the
//! crate's seeded generator ([`crate::grammar::rng::Rng`]).

use crate::grammar::rng::Rng;
use crate::sculpt::form::{Form, Op, Solid};

/// A box of sub-voxels, `sub` per block per axis, each solid or not.
pub struct SubGrid {
    /// Sub-voxels per block per axis.
    pub sub: usize,
    /// Extent in sub-voxels, `[x, y, z]`.
    pub dims: [usize; 3],
    bits: Vec<u8>,
}

impl SubGrid {
    /// An empty grid over a box of `blocks`.
    pub fn new(blocks: [u32; 3], sub: usize) -> SubGrid {
        let dims = [
            blocks[0] as usize * sub,
            blocks[1] as usize * sub,
            blocks[2] as usize * sub,
        ];
        SubGrid {
            sub,
            dims,
            bits: vec![0; dims[0] * dims[1] * dims[2]],
        }
    }

    #[inline]
    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        (i * self.dims[1] + j) * self.dims[2] + k
    }

    /// Is this sub-voxel solid? `false` outside the grid.
    #[inline]
    pub fn get(&self, i: i64, j: i64, k: i64) -> bool {
        if i < 0 || j < 0 || k < 0 {
            return false;
        }
        let (i, j, k) = (i as usize, j as usize, k as usize);
        if i >= self.dims[0] || j >= self.dims[1] || k >= self.dims[2] {
            return false;
        }
        self.bits[self.index(i, j, k)] != 0
    }

    #[inline]
    fn set(&mut self, i: usize, j: usize, k: usize, v: bool) {
        let at = self.index(i, j, k);
        self.bits[at] = v as u8;
    }

    /// Solid sub-voxels in the grid.
    pub fn count(&self) -> usize {
        self.bits.iter().filter(|b| **b != 0).count()
    }

    /// This grid grown by one sub-voxel across each face (the 6-neighbourhood)
    /// — what the thin-plate refit reads (spec-0087 §3.3 step 3).
    pub fn thickened(&self) -> SubGrid {
        let mut out = SubGrid {
            sub: self.sub,
            dims: self.dims,
            bits: self.bits.clone(),
        };
        let [nx, ny, nz] = self.dims;
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    if self.bits[self.index(i, j, k)] != 0 {
                        continue;
                    }
                    let (a, b, c) = (i as i64, j as i64, k as i64);
                    if self.get(a - 1, b, c)
                        || self.get(a + 1, b, c)
                        || self.get(a, b - 1, c)
                        || self.get(a, b + 1, c)
                        || self.get(a, b, c - 1)
                        || self.get(a, b, c + 1)
                    {
                        out.set(i, j, k, true);
                    }
                }
            }
        }
        out
    }
}

/// The weathering field: a coarse lattice of seeded values, smoothed once and
/// scaled to `amplitude` at one standard deviation, read by trilinear
/// interpolation.
pub struct Noise {
    cell: f64,
    dims: [usize; 3],
    values: Vec<f64>,
}

/// An approximately standard-normal draw: the sum of four uniforms, centred and
/// scaled to unit variance (Irwin–Hall), so the stream needs no transcendental
/// function and is the same on every platform.
pub(crate) fn normal_draw(rng: &mut Rng) -> f64 {
    let mut s = 0.0;
    for _ in 0..4 {
        s += (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    }
    // Four uniforms have mean 2 and variance 4/12; sqrt(3) rescales to 1.
    (s - 2.0) * 3f64.sqrt()
}

impl Noise {
    /// The field over a box of `blocks` at lattice spacing `cell`.
    pub fn new(blocks: [u32; 3], cell: f64, amplitude: f64, seed: u64) -> Noise {
        let dims = [
            (blocks[0] as f64 / cell).floor() as usize + 4,
            (blocks[1] as f64 / cell).floor() as usize + 4,
            (blocks[2] as f64 / cell).floor() as usize + 4,
        ];
        let mut rng = Rng::new(seed ^ 0x5EED_0FF0_12A1_u64);
        let n = dims[0] * dims[1] * dims[2];
        let raw: Vec<f64> = (0..n).map(|_| normal_draw(&mut rng)).collect();
        // One [1 2 1]/4 pass per axis: low-frequency patches, not salt and pepper.
        let mut v = raw;
        for axis in 0..3 {
            v = smooth121(&v, dims, axis);
        }
        let mean = v.iter().sum::<f64>() / n as f64;
        let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64;
        let scale = if var > 0.0 {
            amplitude / var.sqrt()
        } else {
            0.0
        };
        let values = v.iter().map(|x| (x - mean) * scale).collect();
        Noise { cell, dims, values }
    }

    /// The weathering offset at a point of the piece frame. The lattice is
    /// offset one cell so the box's low faces interpolate.
    pub fn at(&self, p: [f64; 3]) -> f64 {
        let u = [
            p[0] / self.cell + 1.0,
            p[1] / self.cell + 1.0,
            p[2] / self.cell + 1.0,
        ];
        let mut base = [0usize; 3];
        let mut frac = [0f64; 3];
        for a in 0..3 {
            let hi = (self.dims[a] - 2) as f64;
            let c = u[a].clamp(0.0, hi);
            let f = c.floor();
            base[a] = f as usize;
            frac[a] = c - f;
        }
        let at = |i: usize, j: usize, k: usize| {
            self.values[((base[0] + i) * self.dims[1] + base[1] + j) * self.dims[2] + base[2] + k]
        };
        let mut out = 0.0;
        for (i, wi) in [(0, 1.0 - frac[0]), (1, frac[0])] {
            for (j, wj) in [(0, 1.0 - frac[1]), (1, frac[1])] {
                for (k, wk) in [(0, 1.0 - frac[2]), (1, frac[2])] {
                    out += wi * wj * wk * at(i, j, k);
                }
            }
        }
        out
    }
}

/// One `[1 2 1] / 4` smoothing pass along `axis` over a dense `dims` field,
/// edges clamped.
fn smooth121(v: &[f64], dims: [usize; 3], axis: usize) -> Vec<f64> {
    let mut out = vec![0.0; v.len()];
    let stride = match axis {
        0 => dims[1] * dims[2],
        1 => dims[2],
        _ => 1,
    };
    let n = dims[axis];
    for (idx, o) in out.iter_mut().enumerate() {
        let pos = (idx / stride) % n;
        let lo = if pos == 0 { idx } else { idx - stride };
        let hi = if pos + 1 == n { idx } else { idx + stride };
        *o = (v[lo] + 2.0 * v[idx] + v[hi]) / 4.0;
    }
    out
}

/// What each block belongs to, x-major (`(x*Y + y)*Z + z`): the solid whose
/// material it takes, and the walkway climb it carries.
pub struct Owners {
    /// The ordinal (1-based index into `solids`) of the solid whose material
    /// the block takes — a shelf's tread, or a block whose centre a solid with
    /// its own material contains — or `0` for the palette.
    pub owner: Vec<u16>,
    /// The walkway's climb at the block: `0` not a walkway, [`FLAT`] a level
    /// walkway, or [`climb_code`] of the cardinal direction it ascends. A stair
    /// in a walkway may face only the way the walkway climbs — its `facing` is the
    /// direction a body ascends (`DW0430`) — so the fit is restricted by this.
    pub climb: Vec<u8>,
}

/// [`Owners::climb`] for a level stretch of walkway: no stair belongs there.
pub const FLAT: u8 = 1;

/// [`Owners::climb`] for a walkway ascending toward `facing`.
pub fn climb_code(facing: crate::schem::stairs::Facing) -> u8 {
    use crate::schem::stairs::Facing;
    match facing {
        Facing::North => 2,
        Facing::East => 3,
        Facing::South => 4,
        Facing::West => 5,
    }
}

/// Stamp every solid of `form` into a fresh grid, and record what each block
/// belongs to — the solid whose material it takes, and a shelf tread's climb.
pub fn stamp(form: &Form, seed: u64) -> (SubGrid, Owners) {
    let sub = form.sub as usize;
    let mut grid = SubGrid::new(form.extent, sub);
    let noise = Noise::new(form.extent, form.noise.cell, form.noise.amplitude, seed);
    let floor = form.body_floor() as f64;
    let blocks = form.extent.map(|v| v as usize);
    let mut owners = Owners {
        owner: vec![0; blocks[0] * blocks[1] * blocks[2]],
        climb: vec![0; blocks[0] * blocks[1] * blocks[2]],
    };
    for (si, solid) in form.solids.iter().enumerate() {
        // The ordinal a block owned by this solid's material records.
        let ordinal = u16::try_from(si + 1).unwrap_or(u16::MAX);
        match solid {
            Solid::Shelf {
                path,
                width,
                clearance,
                depth,
                material,
            } => {
                let shelf = Shelf {
                    path,
                    half: width / 2.0,
                };
                let (lo, hi) = shelf.bounds(*depth, *clearance);
                // The tread first, then the headroom over it: the walkway is
                // solid under the feet surface and clear above it, whatever the
                // solids stamped before it put there.
                for_each_sample(&mut grid, floor, lo, hi, |p| {
                    let (lateral, feet, _) = shelf.lateral_and_feet(p);
                    (lateral < 0.0 && p[1] < feet && p[1] > feet - depth).then_some(true)
                });
                for_each_sample(&mut grid, floor, lo, hi, |p| {
                    let (lateral, feet, _) = shelf.lateral_and_feet(p);
                    (lateral < 0.5 && p[1] >= feet && p[1] < feet + clearance).then_some(false)
                });
                for x in 0..blocks[0] {
                    for y in 0..blocks[1] {
                        for z in 0..blocks[2] {
                            let p = [x as f64 + 0.5, y as f64 + 0.5 - floor, z as f64 + 0.5];
                            if p[1] < lo[1] || p[1] > hi[1] {
                                continue;
                            }
                            let (lateral, feet, climb) = shelf.lateral_and_feet(p);
                            if lateral < 0.5 && p[1] > feet - depth && p[1] < feet + 0.5 {
                                let i = (x * blocks[1] + y) * blocks[2] + z;
                                owners.climb[i] = climb;
                                if material.is_some() {
                                    owners.owner[i] = ordinal;
                                }
                            }
                        }
                    }
                }
            }
            other => {
                let (op, noisy, lo, hi) = bounds(other);
                let value = op == Op::Add;
                for_each_sample(&mut grid, floor, lo, hi, |p| {
                    let mut d = distance(other, p);
                    if noisy {
                        d += noise.at([p[0], p[1] + floor, p[2]]);
                    }
                    (d < 0.0).then_some(value)
                });
                // A solid with its own material owns every block whose centre
                // it contains; a later one with a material takes it over.
                if value && other.material().is_some() {
                    let lo_i = [0, 1, 2].map(|a| {
                        let v = if a == 1 { lo[a] + floor } else { lo[a] };
                        (v.floor().max(0.0) as usize).min(blocks[a])
                    });
                    let hi_i = [0, 1, 2].map(|a| {
                        let v = if a == 1 { hi[a] + floor } else { hi[a] };
                        ((v.ceil() + 1.0).max(0.0) as usize).min(blocks[a])
                    });
                    for x in lo_i[0]..hi_i[0] {
                        for y in lo_i[1]..hi_i[1] {
                            for z in lo_i[2]..hi_i[2] {
                                let p = [x as f64 + 0.5, y as f64 + 0.5 - floor, z as f64 + 0.5];
                                let mut d = distance(other, p);
                                if noisy {
                                    d += noise.at([p[0], p[1] + floor, p[2]]);
                                }
                                if d < 0.0 {
                                    owners.owner[(x * blocks[1] + y) * blocks[2] + z] = ordinal;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (grid, owners)
}

/// The op, noisiness and body-frame bounding box of a solid that is not a shelf,
/// padded by two blocks so the weathering cannot be clipped.
fn bounds(s: &Solid) -> (Op, bool, [f64; 3], [f64; 3]) {
    let pad = 2.0;
    match s {
        Solid::Capsule {
            op,
            from,
            to,
            radius_from,
            radius_to,
            stretch_y,
            noisy,
            ..
        } => {
            let r = radius_from.max(*radius_to) * stretch_y.unwrap_or(1.0).max(1.0) + pad;
            let lo = [0, 1, 2].map(|a| from[a].min(to[a]) - r);
            let hi = [0, 1, 2].map(|a| from[a].max(to[a]) + r);
            (*op, *noisy, lo, hi)
        }
        Solid::Ellipsoid {
            op,
            centre,
            radii,
            noisy,
            ..
        } => (
            *op,
            *noisy,
            [0, 1, 2].map(|a| centre[a] - radii[a] - pad),
            [0, 1, 2].map(|a| centre[a] + radii[a] + pad),
        ),
        Solid::Disc {
            op,
            centre,
            radius,
            height,
            noisy,
            ..
        } => {
            let r = radius + height / 2.0 + pad;
            (
                *op,
                *noisy,
                [0, 1, 2].map(|a| centre[a] - r),
                [0, 1, 2].map(|a| centre[a] + r),
            )
        }
        Solid::Box {
            op,
            from,
            to,
            noisy,
            ..
        } => (
            *op,
            *noisy,
            [0, 1, 2].map(|a| from[a].min(to[a]) - pad),
            [0, 1, 2].map(|a| from[a].max(to[a]) + pad),
        ),
        Solid::Shelf { .. } => unreachable!("a shelf is stamped by its own arm"),
    }
}

/// Visit every sub-voxel whose centre lies in the body-frame box `lo..hi`, and
/// write what `f` answers (`None` leaves the sub-voxel as it is).
fn for_each_sample(
    grid: &mut SubGrid,
    floor: f64,
    lo: [f64; 3],
    hi: [f64; 3],
    f: impl Fn([f64; 3]) -> Option<bool>,
) {
    let s = grid.sub as f64;
    // Body frame to piece frame on y only.
    let plo = [lo[0], lo[1] + floor, lo[2]];
    let phi = [hi[0], hi[1] + floor, hi[2]];
    let mut range = [(0usize, 0usize); 3];
    for a in 0..3 {
        let a0 = ((plo[a] * s).floor() - 1.0).max(0.0) as usize;
        let a1 = (((phi[a] * s).floor() + 2.0).max(0.0) as usize).min(grid.dims[a]);
        if a0 >= a1 {
            return;
        }
        range[a] = (a0, a1);
    }
    for i in range[0].0..range[0].1 {
        let x = (i as f64 + 0.5) / s;
        for j in range[1].0..range[1].1 {
            let y = (j as f64 + 0.5) / s - floor;
            for k in range[2].0..range[2].1 {
                let z = (k as f64 + 0.5) / s;
                if let Some(v) = f([x, y, z]) {
                    grid.set(i, j, k, v);
                }
            }
        }
    }
}

/// The signed distance (negative inside) of a non-shelf solid at `p`.
fn distance(s: &Solid, p: [f64; 3]) -> f64 {
    match s {
        Solid::Capsule {
            from,
            to,
            radius_from,
            radius_to,
            stretch_y,
            ..
        } => {
            let ys = stretch_y.unwrap_or(1.0);
            let ab = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
            let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2] + 1e-9;
            let q = [p[0] - from[0], p[1] - from[1], p[2] - from[2]];
            let t = ((q[0] * ab[0] + q[1] * ab[1] + q[2] * ab[2]) / l2).clamp(0.0, 1.0);
            let dx = q[0] - t * ab[0];
            let dy = (q[1] - t * ab[1]) / ys;
            let dz = q[2] - t * ab[2];
            (dx * dx + dy * dy + dz * dz).sqrt() - (radius_from + (radius_to - radius_from) * t)
        }
        Solid::Ellipsoid { centre, radii, .. } => {
            let mut q = 0.0;
            for a in 0..3 {
                let v = (p[a] - centre[a]) / radii[a];
                q += v * v;
            }
            let rmin = radii[0].min(radii[1]).min(radii[2]);
            (q.sqrt() - 1.0) * rmin
        }
        Solid::Disc {
            centre,
            axis,
            radius,
            height,
            ..
        } => {
            let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
            let u = [axis[0] / len, axis[1] / len, axis[2] / len];
            let q = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
            let along = q[0] * u[0] + q[1] * u[1] + q[2] * u[2];
            let r = [
                q[0] - along * u[0],
                q[1] - along * u[1],
                q[2] - along * u[2],
            ];
            let radial = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
            let dx = radial - radius;
            let dy = along.abs() - height / 2.0;
            let ox = dx.max(0.0);
            let oy = dy.max(0.0);
            dx.max(dy).min(0.0) + (ox * ox + oy * oy).sqrt()
        }
        Solid::Box { from, to, .. } => {
            let mut d = f64::NEG_INFINITY;
            for a in 0..3 {
                let c = (from[a] + to[a]) / 2.0;
                let h = (to[a] - from[a]).abs() / 2.0;
                d = d.max((p[a] - c).abs() - h);
            }
            d
        }
        Solid::Shelf { .. } => unreachable!("a shelf is stamped by its own arm"),
    }
}

/// A shelf's path, read as a walkway.
struct Shelf<'a> {
    path: &'a [[f64; 3]],
    half: f64,
}

impl Shelf<'_> {
    /// The body-frame box a shelf's tread and headroom lie in.
    fn bounds(&self, depth: f64, clearance: f64) -> ([f64; 3], [f64; 3]) {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for k in self.path {
            for a in 0..3 {
                lo[a] = lo[a].min(k[a]);
                hi[a] = hi[a].max(k[a]);
            }
        }
        let r = self.half + 1.0;
        (
            [lo[0] - r, lo[1] - depth - 1.0, lo[2] - r],
            [hi[0] + r, hi[1] + clearance + 1.0, hi[2] + r],
        )
    }

    /// How far `p` lies outside the walkway's width (negative inside it), the
    /// feet height at the nearest point of the path, both in blocks, and the
    /// walkway's climb there ([`Owners::climb`]).
    fn lateral_and_feet(&self, p: [f64; 3]) -> (f64, f64, u8) {
        use crate::schem::stairs::Facing;
        let mut best = (f64::INFINITY, self.path[0][1], FLAT);
        for w in self.path.windows(2) {
            let (a, b) = (w[0], w[1]);
            let ab = [b[0] - a[0], b[2] - a[2]];
            let l2 = ab[0] * ab[0] + ab[1] * ab[1];
            let ap = [p[0] - a[0], p[2] - a[2]];
            let t = if l2 > 0.0 {
                ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let dx = ap[0] - t * ab[0];
            let dz = ap[1] - t * ab[1];
            let d2 = dx * dx + dz * dz;
            if d2 < best.0 {
                let rise = b[1] - a[1];
                let climb = if rise == 0.0 || l2 == 0.0 {
                    FLAT
                } else {
                    // The uphill direction in plan, snapped to a cardinal.
                    let (ux, uz) = if rise > 0.0 {
                        (ab[0], ab[1])
                    } else {
                        (-ab[0], -ab[1])
                    };
                    climb_code(if ux.abs() >= uz.abs() {
                        if ux > 0.0 { Facing::East } else { Facing::West }
                    } else if uz > 0.0 {
                        Facing::South
                    } else {
                        Facing::North
                    })
                };
                best = (d2, a[1] + rise * t, climb);
            }
        }
        (best.0.sqrt() - self.half, best.1, best.2)
    }
}
