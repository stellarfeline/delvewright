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
    },
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
            } => GroundKind::Open {
                surface: surface.clone(),
                below: below.clone(),
                tops: Self::tops(plan, terrain, c.heightmap.as_ref()),
            },
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
            GroundKind::Open { surface, below, .. } => {
                let top = self.top(cell[0], cell[2])?;
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
                if cell[1] >= g {
                    surface
                } else {
                    below
                }
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
