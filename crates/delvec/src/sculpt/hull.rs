//! **Light set into the body's inside surface** — the `hull` placement of a
//! `lights[]` entry (spec-0087 §9).
//!
//! The form states the block, the mode, the surfaces, the region and the
//! spacing; this module derives where each source goes:
//!
//! 1. **Candidates.** Every fitted body block (full, stair or slab) inside `within` whose face
//!    meets air *with the body over that air* — the inside surface, never the
//!    outside of the body and never the ground. A block facing air below it is
//!    `vault`; else one facing air sideways is `wall`; else one facing air above
//!    it is `floor`. Only the surfaces `on` names are kept, and for `recessed`
//!    only those where the recess fits (below).
//! 2. **A seeded Poisson-disk draw** (Bridson's blue noise over a finite
//!    candidate set): the candidates in the order of a seeded shuffle, each
//!    accepted when no source of the entry lies within `spacing` blocks of it
//!    and no source of an earlier entry lies within the smaller of the two
//!    entries' spacings. Every pair of an entry's sources is at least
//!    `spacing` apart and every candidate left out has one within it, so the
//!    set is staggered, irregular and covers the surface evenly — never a
//!    grid — and a sparse entry interleaves with a dense one over the same
//!    surface rather than being crowded out by it.
//! 3. **Placement.** `embedded`: the source replaces the surface block, flush.
//!    `recessed`: with `d` the way the surface faces the room and `u` a way
//!    along the surface (for a wall the first sideways cardinal that fits,
//!    then up; for a vault the first cardinal that fits), the source goes one block behind the surface (`H − d`), the cover
//!    goes in the surface in front of it (`H`), and the two blocks beside them
//!    along `u` (`H + u`, `H − d + u`) are opened into a slot, so the source is
//!    hidden behind the cover and its light leaves through the slot. The recess
//!    fits only where every block round the source and the cell behind the slot
//!    is solid body, so the nook opens to the room and nowhere else.
//!
//! The engine's light model ([`crate::compiler::light`]) counts a stair or a
//! slab as opaque; the slot is the path it sees, three steps long, so a source
//! of emission `E` reaches the room at `E − 3`. [`crate::sculpt::sculpt`]
//! floods the finished piece with that model and refuses (`DW0952`) a source
//! whose room cell it measures dark.

use crate::grammar::rng::Rng;
use crate::schem::stairs::{Facing, Half};
use crate::sculpt::fit::{Blocks, CoverShape, Kind};
use crate::sculpt::form::{Form, Hull, LightMode, Surface};

/// The cardinal directions, in the order a choice among them is made.
const CARDINALS: [Facing; 4] = [Facing::North, Facing::East, Facing::South, Facing::West];

/// One source a `hull` light placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The light-emitting block's cell, piece frame.
    pub at: [i32; 3],
    /// The surface block the draw chose, which the spacing is measured
    /// between: the source itself (`embedded`) or its cover (`recessed`).
    pub host: [i32; 3],
    /// The air cell in the room its light is judged at: in front of the
    /// source (`embedded`) or in front of the slot (`recessed`).
    pub room: [i32; 3],
    /// The surface it sits in.
    pub surface: Surface,
}

/// What one `hull` light placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HullReading {
    /// The entry's index in `lights[]`.
    pub light: usize,
    /// Its mode.
    pub mode: LightMode,
    /// Its spacing, as declared.
    pub spacing: String,
    /// Candidate cells, per surface `[wall, vault, floor]`, before the draw.
    pub candidates: [usize; 3],
    /// The sources placed.
    pub sources: Vec<Source>,
    /// The least and greatest light the model measures at a source's room
    /// cell, filled in once the piece is flooded.
    pub room_light: Option<(u8, u8)>,
}

impl HullReading {
    /// Sources per surface `[wall, vault, floor]`.
    pub fn per_surface(&self) -> [usize; 3] {
        let mut n = [0; 3];
        for s in &self.sources {
            n[slot(s.surface)] += 1;
        }
        n
    }

    /// The binding line `delvec sculpt` prints.
    pub fn line(&self) -> String {
        let [w, v, f] = self.per_surface();
        let [cw, cv, cf] = self.candidates;
        format!(
            "lights[{}] hull {}: {} source(s) — {w} wall, {v} vault, {f} floor — of {} \
             candidate cell(s) ({cw} wall, {cv} vault, {cf} floor), spacing {}; room light {}",
            self.light,
            match self.mode {
                LightMode::Embedded => "embedded",
                LightMode::Recessed => "recessed",
            },
            self.sources.len(),
            cw + cv + cf,
            self.spacing,
            match self.room_light {
                Some((lo, hi)) => format!("{lo}..{hi}"),
                None => "unmeasured".to_string(),
            }
        )
    }
}

fn slot(s: Surface) -> usize {
    match s {
        Surface::Wall => 0,
        Surface::Vault => 1,
        Surface::Floor => 2,
    }
}

fn add(p: [i64; 3], d: [i32; 3]) -> [i64; 3] {
    [p[0] + d[0] as i64, p[1] + d[1] as i64, p[2] + d[2] as i64]
}

fn neg(d: [i32; 3]) -> [i32; 3] {
    [-d[0], -d[1], -d[2]]
}

const UP: [i32; 3] = [0, 1, 0];
const DOWN: [i32; 3] = [0, -1, 0];

/// A candidate: the surface block, the surface it is, the way it faces the
/// room, and for `recessed` the way along the surface the slot opens.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    at: [i64; 3],
    surface: Surface,
    d: [i32; 3],
    u: Option<[i32; 3]>,
}

fn kind(blocks: &Blocks, p: [i64; 3]) -> Kind {
    blocks.kind_at(p[0], p[1], p[2])
}

fn is_air(blocks: &Blocks, p: [i64; 3]) -> bool {
    in_box(blocks, p) && kind(blocks, p) == Kind::Air
}

fn in_box(blocks: &Blocks, p: [i64; 3]) -> bool {
    (0..3).all(|a| p[a] >= 0 && (p[a] as usize) < blocks.dims[a])
}

/// A block of the body's surface a source may take the place of: a fitted
/// full block, stair or slab — never the ground, a light, or a cover.
fn is_surface(blocks: &Blocks, p: [i64; 3]) -> bool {
    in_box(blocks, p)
        && matches!(
            kind(blocks, p),
            Kind::Full | Kind::Stair(..) | Kind::SlabBottom | Kind::SlabTop
        )
}

/// Solid body or ground: what may stand round a recess.
fn is_solid(blocks: &Blocks, p: [i64; 3]) -> bool {
    in_box(blocks, p)
        && !matches!(
            kind(blocks, p),
            Kind::Air | Kind::Light(_) | Kind::Cover(..)
        )
}

/// Has this air cell the body (anything not air) somewhere over it?
fn roofed(blocks: &Blocks, p: [i64; 3]) -> bool {
    let mut q = add(p, UP);
    while in_box(blocks, q) {
        if kind(blocks, q) != Kind::Air {
            return true;
        }
        q = add(q, UP);
    }
    false
}

/// The surface a body block is and the way it faces the room, if it is
/// one: vault before wall before floor.
fn classify(blocks: &Blocks, p: [i64; 3]) -> Option<(Surface, Vec<[i32; 3]>)> {
    if is_air(blocks, add(p, DOWN)) {
        return Some((Surface::Vault, vec![DOWN]));
    }
    let sides: Vec<[i32; 3]> = CARDINALS
        .iter()
        .map(|f| f.step())
        .filter(|d| {
            let a = add(p, *d);
            is_air(blocks, a) && roofed(blocks, a)
        })
        .collect();
    if !sides.is_empty() {
        return Some((Surface::Wall, sides));
    }
    let above = add(p, UP);
    if is_air(blocks, above) && roofed(blocks, above) {
        return Some((Surface::Floor, vec![UP]));
    }
    None
}

/// Does a recess fit at `h` facing the room along `d`, its slot opening along
/// `u`? The surface and the slot block are body surface; the room cells in front
/// of both are air; the source's cell, the cell behind the slot, and every
/// block round those two (but each other, the cover and the slot) are solid.
fn recess_fits(blocks: &Blocks, h: [i64; 3], d: [i32; 3], u: [i32; 3]) -> bool {
    let s = add(h, u);
    let l = add(h, neg(d));
    let o = add(l, u);
    if !is_surface(blocks, h) || !is_surface(blocks, s) {
        return false;
    }
    if !is_air(blocks, add(h, d)) || !is_air(blocks, add(s, d)) {
        return false;
    }
    if !is_solid(blocks, l) || !is_solid(blocks, o) || !is_solid(blocks, add(s, u)) {
        return false;
    }
    let inner = [h, s, l, o];
    for c in [l, o] {
        for step in [UP, DOWN]
            .into_iter()
            .chain(CARDINALS.iter().map(|f| f.step()))
        {
            let n = add(c, step);
            if !inner.contains(&n) && !is_solid(blocks, n) {
                return false;
            }
        }
    }
    true
}

/// The candidates of one `hull` light, in scan order (x, then y, then z).
fn candidates(form: &Form, blocks: &Blocks, hull: &Hull) -> Vec<Candidate> {
    let floor = form.body_floor();
    let lo = [
        hull.within.from[0] as i64,
        hull.within.from[1] as i64 + floor,
        hull.within.from[2] as i64,
    ];
    let hi = [
        hull.within.to[0] as i64,
        hull.within.to[1] as i64 + floor,
        hull.within.to[2] as i64,
    ];
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let p = [x, y, z];
                if !is_surface(blocks, p) {
                    continue;
                }
                let Some((surface, faces)) = classify(blocks, p) else {
                    continue;
                };
                if !hull.on.contains(&surface) {
                    continue;
                }
                let chosen = match hull.mode {
                    LightMode::Embedded => Some(Candidate {
                        at: p,
                        surface,
                        d: faces[0],
                        u: None,
                    }),
                    LightMode::Recessed => faces.iter().find_map(|d| {
                        // Along the surface: for a wall, sideways first (the
                        // slot's mouth at the source's own height) and then up;
                        // for a vault, the four cardinals.
                        let ways: Vec<[i32; 3]> = match surface {
                            Surface::Wall => CARDINALS
                                .iter()
                                .map(|f| f.step())
                                .filter(|w| w[0] * d[0] + w[2] * d[2] == 0)
                                .chain([UP])
                                .collect(),
                            _ => CARDINALS.iter().map(|f| f.step()).collect(),
                        };
                        ways.into_iter()
                            .find(|u| recess_fits(blocks, p, *d, *u))
                            .map(|u| Candidate {
                                at: p,
                                surface,
                                d: *d,
                                u: Some(u),
                            })
                    }),
                };
                out.extend(chosen);
            }
        }
    }
    out
}

fn facing_of(step: [i32; 3]) -> Facing {
    *CARDINALS
        .iter()
        .find(|f| f.step() == step)
        .expect("a horizontal unit step is a cardinal")
}

/// The cover's orientation: on a wall, a stair whose full-height half is on
/// the room side, half bottom, or a bottom slab. In a vault the source stands
/// ON the cover, so the cover is its top half — a top-half stair, its quarter
/// hanging away from the slot, or a top slab: a lantern asks the face under it
/// to be sturdy at its centre, a bottom half's top is not, and the server drops
/// a source stood on one (`DW1002`).
fn cover_shape(hull: &Hull, c: &Candidate) -> CoverShape {
    let cover = hull.cover.as_deref().unwrap_or_default();
    let vault = c.surface != Surface::Wall;
    if cover.ends_with("_slab") {
        return CoverShape::Slab(vault);
    }
    let u = c.u.expect("a recessed candidate has a slot");
    let facing = match c.surface {
        Surface::Wall => facing_of(c.d),
        _ => facing_of(neg(u)),
    };
    CoverShape::Stair(facing, if vault { Half::Top } else { Half::Bottom })
}

/// **Place every `hull` light** of `form` into `blocks`, after the fit, the
/// apron, the islands and the lights placed by hand, in `lights[]` order. Each
/// entry draws from its own seeded stream, so an entry added after another
/// never moves the earlier one's sources.
pub fn place(form: &Form, blocks: &mut Blocks, seed: u64) -> Vec<HullReading> {
    let mut out = Vec::new();
    // Every hull source placed so far, by the surface block it took, with
    // its entry's spacing.
    let mut accepted: Vec<([i64; 3], f64)> = Vec::new();
    for (li, light) in form.lights.iter().enumerate() {
        let Some(hull) = &light.hull else { continue };
        let mut cands = candidates(form, blocks, hull);
        let mut counts = [0usize; 3];
        for c in &cands {
            counts[slot(c.surface)] += 1;
        }
        // A seeded Fisher–Yates shuffle: the draw order.
        let mut rng = Rng::new(
            seed ^ 0x4C16_4875_11A5_u64 ^ (li as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        for i in (1..cands.len()).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            cands.swap(i, j);
        }
        let mut sources = Vec::new();
        for c in cands {
            let near = accepted.iter().any(|(a, spacing)| {
                let r = spacing.min(hull.spacing);
                let d: f64 = (0..3).map(|k| ((a[k] - c.at[k]) as f64).powi(2)).sum();
                d < r * r
            });
            if near {
                continue;
            }
            // An earlier source may have changed the blocks round this one.
            let at = c.at;
            let i = |p: [i64; 3]| blocks.index(p[0] as usize, p[1] as usize, p[2] as usize);
            match hull.mode {
                LightMode::Embedded => {
                    if !is_surface(blocks, at) {
                        continue;
                    }
                    let k = i(at);
                    blocks.kind[k] = Kind::Light(li);
                    sources.push(Source {
                        at: at.map(|v| v as i32),
                        host: at.map(|v| v as i32),
                        room: add(at, c.d).map(|v| v as i32),
                        surface: c.surface,
                    });
                }
                LightMode::Recessed => {
                    let u = c.u.expect("a recessed candidate has a slot");
                    if !recess_fits(blocks, at, c.d, u) {
                        continue;
                    }
                    let shape = cover_shape(hull, &c);
                    let l = add(at, neg(c.d));
                    let s = add(at, u);
                    let o = add(l, u);
                    let (kl, ko, ks, kh) = (i(l), i(o), i(s), i(at));
                    blocks.kind[kl] = Kind::Light(li);
                    blocks.kind[ko] = Kind::Air;
                    blocks.kind[ks] = Kind::Air;
                    blocks.kind[kh] = Kind::Cover(li, shape);
                    sources.push(Source {
                        at: l.map(|v| v as i32),
                        host: at.map(|v| v as i32),
                        room: add(s, c.d).map(|v| v as i32),
                        surface: c.surface,
                    });
                }
            }
            accepted.push((at, hull.spacing));
        }
        out.push(HullReading {
            light: li,
            mode: hull.mode,
            spacing: format!("{}", hull.spacing),
            candidates: counts,
            sources,
            room_light: None,
        });
    }
    out
}
