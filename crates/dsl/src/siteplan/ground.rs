//! **What the whole writes where no place does** (spec-0098 §2b, §2c): the
//! site's `fill`, resolved once into the one answer every reader asks of it —
//! how high the terrain stands in a column, and which block a cell of the whole
//! holds.
//!
//! The claim (`claim.rs`), the blockout derivation and the handout all read
//! this, so the ground a piece is handed, the ground the derivation lays and the
//! ground the crack check measures are one computation.

use super::*;

/// The site plan's heightmap as a loader read it.
///
/// The DSL reads no file: the loader decodes the image the plan names and
/// attaches it to the [`Campaign`] (`Campaign::heightmap`), and everything here
/// reads that. Grey values only, row-major over `z` then `x`, so
/// `grey[pz * width + px]` is column `(region.min.x + px, region.min.z + pz)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeightmapRead {
    /// The decoded image.
    Read {
        /// The path the plan named, as read.
        path: String,
        /// Pixels along `x`.
        width: u32,
        /// Pixels along `z`.
        depth: u32,
        /// One grey value per pixel.
        grey: Vec<u8>,
    },
    /// The image could not be read or decoded; the loader's words.
    Unreadable {
        /// The path the plan named.
        path: String,
        /// Why it could not be read.
        why: String,
    },
}

/// The fill, resolved: what the whole holds wherever no place claims a cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ground {
    kind: GroundKind,
    region_min: [i64; 3],
    region_max: [i64; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum GroundKind {
    /// No plan, or a plan whose fill does not parse — nothing is written.
    None,
    Solid {
        block: String,
    },
    Open {
        surface: String,
        below: String,
        tops: Tops,
        /// The surface rules, in document order.
        strata: Vec<Stratum>,
        /// Per column (row-major over the region, `z` then `x`), the index of
        /// the first stratum the column meets plus one; `0` is the fill's own
        /// `surface` and `below`. Empty when the fill declares no strata.
        cols: Vec<u8>,
        /// The world's seed, which every noise and every tree draws from.
        seed: u64,
    },
}

/// A 32-bit integer hash of a seed and a lattice point (splitmix-style):
/// deterministic, platform-independent, and the only randomness here.
fn lattice(seed: u64, salt: u32, ix: i64, iz: i64) -> u32 {
    let mut h = seed
        ^ u64::from(salt).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (ix as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (iz as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    (h >> 32) as u32
}

/// Value noise over a `scale`-block lattice, read per mille (0..=1000), with
/// a smoothstep between lattice points computed in integers (per mille).
fn patch_reading(seed: u64, p: &Patch, x: i64, z: i64) -> u32 {
    let sc = i64::from(p.scale.get());
    let (ix, iz) = (x.div_euclid(sc), z.div_euclid(sc));
    let (fx, fz) = (x.rem_euclid(sc) * 1000 / sc, z.rem_euclid(sc) * 1000 / sc);
    let ease = |t: i64| t * t * (3000 - 2 * t) / 1_000_000;
    let (ex, ez) = (ease(fx), ease(fz));
    let v = |dx: i64, dz: i64| i64::from(lattice(seed, p.salt, ix + dx, iz + dz) % 1001);
    let top = v(0, 0) * (1000 - ex) + v(1, 0) * ex;
    let bot = v(0, 1) * (1000 - ex) + v(1, 1) * ex;
    u32::try_from((top * (1000 - ez) + bot * ez) / 1_000_000).unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tops {
    /// One surface `y` everywhere.
    Flat(i64),
    /// A decoded heightmap: `tops[pz * width + px]`.
    Map {
        width: i64,
        depth: i64,
        tops: Vec<i64>,
    },
    /// A terrain this campaign cannot resolve — an undeclared datum (`DW0112`)
    /// or a heightmap not read or not region-shaped (`DW0826`). Refused at
    /// validation; anything asked of it answers as though there were no
    /// terrain, which only a refused plan ever sees.
    Unresolved,
}

/// The surface `y` a heightmap pixel stands for: `base_y + value × range / 255`,
/// integer division.
#[must_use]
pub fn pixel_top(base_y: i64, range: u32, value: u8) -> i64 {
    base_y + (i64::from(value) * i64::from(range)) / 255
}

impl Ground {
    /// The ground a campaign's site plan declares.
    #[must_use]
    pub fn of(c: &Campaign) -> Ground {
        let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
            return Ground {
                kind: GroundKind::None,
                region_min: [0; 3],
                region_max: [-1; 3],
            };
        };
        let region_min = plan.region.min;
        let region_max = plan.region.max();
        let kind = match &plan.fill {
            Fill::Solid { block } => GroundKind::Solid {
                block: block.clone(),
            },
            Fill::Open {
                terrain,
                surface,
                below,
                strata,
            } => {
                let tops = Self::tops(plan, terrain, c.heightmap.as_ref());
                let seed = c.world.content.seed;
                let cols = Self::columns(&tops, strata, seed, region_min);
                GroundKind::Open {
                    surface: surface.clone(),
                    below: below.clone(),
                    tops,
                    strata: strata.clone(),
                    cols,
                    seed,
                }
            }
        };
        Ground {
            kind,
            region_min,
            region_max,
        }
    }

    fn tops(plan: &SitePlanContent, terrain: &Terrain, read: Option<&HeightmapRead>) -> Tops {
        match terrain {
            Terrain::Flat { datum } => plan
                .datums
                .iter()
                .find(|d| &d.id == datum)
                .map_or(Tops::Unresolved, |d| Tops::Flat(d.y - 1)),
            Terrain::Heightmap {
                heightmap,
                base_y,
                range,
            } => {
                let Some(HeightmapRead::Read {
                    path,
                    width,
                    depth,
                    grey,
                }) = read
                else {
                    return Tops::Unresolved;
                };
                let (w, d) = (
                    i64::from(plan.region.extent[0].get()),
                    i64::from(plan.region.extent[2].get()),
                );
                if path != heightmap || i64::from(*width) != w || i64::from(*depth) != d {
                    return Tops::Unresolved;
                }
                Tops::Map {
                    width: w,
                    depth: d,
                    tops: grey
                        .iter()
                        .map(|v| pixel_top(*base_y, *range, *v))
                        .collect(),
                }
            }
        }
    }

    /// Which stratum each column meets first (index + 1; 0 none), over a
    /// heightmap; empty when there are no strata or no map.
    fn columns(tops: &Tops, strata: &[Stratum], seed: u64, region_min: [i64; 3]) -> Vec<u8> {
        let Tops::Map { width, depth, tops } = tops else {
            return Vec::new();
        };
        if strata.is_empty() {
            return Vec::new();
        }
        let (w, d) = (*width, *depth);
        let at = |px: i64, pz: i64| tops[(pz.clamp(0, d - 1) * w + px.clamp(0, w - 1)) as usize];
        let mut out = Vec::with_capacity(tops.len());
        for pz in 0..d {
            for px in 0..w {
                let y = at(px, pz);
                let steep = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .map(|(dx, dz)| (at(px + dx, pz + dz) - y).abs())
                    .max()
                    .unwrap_or(0);
                let (x, z) = (region_min[0] + px, region_min[2] + pz);
                let k = strata.iter().position(|st| {
                    let w = &st.when;
                    w.y_min.is_none_or(|m| y >= m)
                        && w.y_max.is_none_or(|m| y <= m)
                        && w.steep.is_none_or(|m| steep >= i64::from(m))
                        && w.patch
                            .is_none_or(|p| patch_reading(seed, &p, x, z) >= p.min)
                        && w.area.is_none_or(|a| {
                            x >= a.min[0] && x <= a.max[0] && z >= a.min[1] && z <= a.max[1]
                        })
                });
                out.push(k.map_or(0, |k| u8::try_from(k + 1).unwrap_or(0)));
            }
        }
        out
    }

    /// The stratum a column takes (index + 1; 0 is the fill's own blocks).
    #[must_use]
    pub fn column_kind(&self, x: i64, z: i64) -> u8 {
        let GroundKind::Open { cols, tops, .. } = &self.kind else {
            return 0;
        };
        let Tops::Map { width, depth, .. } = tops else {
            return 0;
        };
        if cols.is_empty() {
            return 0;
        }
        let px = (x - self.region_min[0]).clamp(0, width - 1);
        let pz = (z - self.region_min[2]).clamp(0, depth - 1);
        cols[(pz * width + px) as usize]
    }

    /// The surface and below blocks of column `(x, z)` on an `open` site.
    fn column_blocks(&self, x: i64, z: i64) -> Option<(&str, &str)> {
        let GroundKind::Open {
            surface,
            below,
            strata,
            ..
        } = &self.kind
        else {
            return None;
        };
        match self.column_kind(x, z) {
            0 => Some((surface, below)),
            k => strata
                .get(usize::from(k) - 1)
                .map(|st| (st.surface.as_str(), st.below.as_str())),
        }
    }

    /// **The trees the fill grows** (`strata[].trees`): every trunk and crown
    /// cell, with its block, in a fixed order. A tree stands where its column
    /// draws under the stratum's `per_mille`, and only where its whole crown
    /// (two cells round the trunk) and two cells beyond keep clear of `clear`,
    /// the footprints (`[x0, x1, z0, z1]`, rings included) of every place, so
    /// no place's claim ever cuts a tree.
    #[must_use]
    pub fn trees(&self, clear: &[[i64; 4]]) -> Vec<([i64; 3], &str)> {
        let GroundKind::Open { strata, seed, .. } = &self.kind else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let (lo, hi) = (self.region_min, self.region_max);
        for x in lo[0] + 3..=hi[0] - 3 {
            for z in lo[2] + 3..=hi[2] - 3 {
                let k = self.column_kind(x, z);
                if k == 0 {
                    continue;
                }
                let Some(t) = strata[usize::from(k) - 1].trees.as_ref() else {
                    continue;
                };
                let draw = lattice(*seed, 0x7EE5, x, z);
                if draw % 1000 >= t.per_mille {
                    continue;
                }
                // One tree per 3×3: a neighbour that also drew keeps the
                // lower draw, so crowns never grow through each other.
                let rival = (-2..=2).any(|dx: i64| {
                    (-2..=2).any(|dz: i64| {
                        (dx, dz) != (0, 0) && self.column_kind(x + dx, z + dz) == k && {
                            let o = lattice(*seed, 0x7EE5, x + dx, z + dz);
                            o % 1000 < t.per_mille && o < draw
                        }
                    })
                });
                if rival {
                    continue;
                }
                if clear
                    .iter()
                    .any(|f| x >= f[0] - 4 && x <= f[1] + 4 && z >= f[2] - 4 && z <= f[3] + 4)
                {
                    continue;
                }
                let Some(top) = self.top(x, z) else { continue };
                let [h0, h1] = t.height;
                let span = h1.saturating_sub(h0) + 1;
                let h = i64::from(h0 + (draw >> 10) % span.max(1));
                if top + h + 2 > hi[1] {
                    continue;
                }
                // A body walks out under every crown: its lowest layer
                // stands at least three over the ground of every column it
                // covers (a body's two courses and a step), so no crown
                // closes a pocket on a slope.
                let low = top + h - 2;
                let clear_under = (-2..=2_i64).all(|dx| {
                    (-2..=2_i64).all(|dz| self.top(x + dx, z + dz).is_some_and(|g| low - g >= 3))
                });
                if !clear_under {
                    continue;
                }
                for y in top + 1..=top + h {
                    out.push(([x, y, z], t.log.as_str()));
                }
                // The crown: two wide layers under the top, one narrow layer
                // at it, and a cap.
                for (dy, r) in [(h - 2, 2i64), (h - 1, 2), (h, 1), (h + 1, 1)] {
                    for dx in -r..=r {
                        for dz in -r..=r {
                            if dx == 0 && dz == 0 && dy <= h {
                                continue;
                            }
                            if r == 2 && dx.abs() == 2 && dz.abs() == 2 {
                                continue;
                            }
                            if dy == h + 1 && dx != 0 && dz != 0 {
                                continue;
                            }
                            out.push(([x + dx, top + dy, z + dz], t.leaves.as_str()));
                        }
                    }
                }
            }
        }
        out
    }

    /// A `solid` site of `block`, standing alone — for a caller that has no
    /// campaign (a test, a probe).
    #[must_use]
    pub fn solid(block: &str) -> Ground {
        Ground {
            kind: GroundKind::Solid {
                block: block.to_string(),
            },
            region_min: [i64::MIN / 4; 3],
            region_max: [i64::MAX / 4; 3],
        }
    }

    /// An `open` site over one flat surface `y`, standing alone.
    #[must_use]
    pub fn open_flat(top: i64, surface: &str, below: &str) -> Ground {
        Ground {
            kind: GroundKind::Open {
                surface: surface.to_string(),
                below: below.to_string(),
                tops: Tops::Flat(top),
                strata: Vec::new(),
                cols: Vec::new(),
                seed: 0,
            },
            region_min: [i64::MIN / 4; 3],
            region_max: [i64::MAX / 4; 3],
        }
    }

    /// True for an `open` site.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(self.kind, GroundKind::Open { .. })
    }

    /// True when the plan declares a fill this ground answers for.
    #[must_use]
    pub fn is_declared(&self) -> bool {
        !matches!(self.kind, GroundKind::None)
    }

    /// The terrain's surface `y` in column `(x, z)`, on an `open` site whose
    /// terrain resolved. A column outside the region reads the nearest edge
    /// column, so a ring standing one cell past the region's rim still meets
    /// the ground the rim has (`DW0826` refuses a claim that leaves the region).
    #[must_use]
    pub fn top(&self, x: i64, z: i64) -> Option<i64> {
        let GroundKind::Open { tops, .. } = &self.kind else {
            return None;
        };
        match tops {
            Tops::Flat(y) => Some(*y),
            Tops::Map { width, depth, tops } => {
                let px = (x - self.region_min[0]).clamp(0, width - 1);
                let pz = (z - self.region_min[2]).clamp(0, depth - 1);
                tops.get(usize::try_from(pz * width + px).ok()?).copied()
            }
            Tops::Unresolved => None,
        }
    }

    /// The block the whole holds at `cell` where nothing — no place, no volume
    /// — claims it; `None` is air.
    #[must_use]
    pub fn fill_block(&self, cell: [i64; 3]) -> Option<&str> {
        match &self.kind {
            GroundKind::None => None,
            GroundKind::Solid { block } => Some(block),
            GroundKind::Open { .. } => {
                let top = self.top(cell[0], cell[2])?;
                let (surface, below) = self.column_blocks(cell[0], cell[2])?;
                match cell[1].cmp(&top) {
                    std::cmp::Ordering::Greater => None,
                    std::cmp::Ordering::Equal => Some(surface),
                    std::cmp::Ordering::Less => Some(below),
                }
            }
        }
    }

    /// The block a **fixed ground** cell holds (spec-0098 §2 rule 0) — a ring
    /// cell at or under its ground height `g`: the `surface` block at `g`,
    /// `below` under it on an `open` site; the `solid` fill's block on a solid
    /// one. Volumes never reach the ring's ground: it is the terrain continued
    /// to the plot's edge, block for block.
    #[must_use]
    pub fn ground_block(&self, cell: [i64; 3], g: i64) -> &str {
        match &self.kind {
            GroundKind::None => "minecraft:air",
            GroundKind::Solid { block } => block,
            GroundKind::Open { surface, below, .. } => {
                let (surface, below) = self
                    .column_blocks(cell[0], cell[2])
                    .unwrap_or((surface.as_str(), below.as_str()));
                if cell[1] >= g { surface } else { below }
            }
        }
    }

    /// The block a volume of `role` is made of when it names none: the fill's
    /// own block of that kind (spec-0098 §2b). `None` is air.
    #[must_use]
    pub fn volume_block(&self, role: VolumeRole, cell: [i64; 3], vol_top: i64) -> Option<&str> {
        match (role, &self.kind) {
            (VolumeRole::Clearance, _) | (_, GroundKind::None) => None,
            (_, GroundKind::Solid { block }) => Some(block),
            (VolumeRole::Massif, GroundKind::Open { below, .. }) => Some(below),
            (VolumeRole::Ground, GroundKind::Open { surface, below, .. }) => {
                Some(if cell[1] == vol_top { surface } else { below })
            }
        }
    }

    /// The region's inclusive corners.
    #[must_use]
    pub fn region(&self) -> ([i64; 3], [i64; 3]) {
        (self.region_min, self.region_max)
    }
}
