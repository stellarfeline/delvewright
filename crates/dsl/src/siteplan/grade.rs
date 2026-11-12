//! **An open side meets the ground in steps** (spec-0138): where a ground
//! place's floor stands higher or lower than the terrain beside a side it walls
//! off with nothing, the whole's terrain beyond the ring is stepped up or down
//! to it, one block every two columns, and a side declared `sheer` keeps its
//! drop.
//!
//! The grade is a bound on [`Ground::top`]: [`Ground::of`] hands this module
//! the placed boxes and the terrain's natural surface, and takes back the
//! graded surface, which every reader of the terrain then reads. The natural
//! surface has no reader but this module.
//!
//! - **Edge columns** of a place: its ring columns on graded sides only (a
//!   corner on both of its sides) that no other ground place's shell
//!   footprint covers.
//! - **Whole columns**: region columns in no ground place's shell footprint;
//!   the only columns the grade changes.
//! - **The step count** `k` of a column for a place: the least `k ≥ 0` with
//!   `q ≤ (1 + 2k)²`, `q` the least squared distance to the place's edge
//!   columns.
//! - **The bound**: `lo = max(P − k)`, `hi = min(P + k)` over every place with
//!   an edge column; a whole column's surface is its natural surface clamped
//!   into `[lo, hi]`.
//!
//! `DW1108` is refused from what this module finds: two settings that meet,
//! and a cut beside a declared fluid ([`check`]).

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;

use super::*;

/// How an unwalled side of a ground place meets the ground beyond it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Meets {
    /// Stepped to it (a side absent or `open`, on an open fill whose terrain
    /// resolved).
    Graded,
    /// Left as the terrain has it (`sheer`, or a site with no terrain).
    Sheer,
}

impl Meets {
    /// The spelling the handout and the ledger use.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Meets::Graded => "graded",
            Meets::Sheer => "sheer",
        }
    }
}

/// One unwalled side of a ground place, as the grade met it: a row of the
/// ledger (`validation/grade.json`) and of the handout's `sides`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GradeSide {
    /// The place.
    pub node: NodeId,
    /// The side.
    pub face: Face,
    /// Graded or sheer.
    pub meets: Meets,
    /// The place's surface `P`: the `y` of its floor course.
    pub surface: i64,
    /// Edge columns on this side (a corner counted on the first graded side
    /// it stands on, in [`Sides::FACES`] order).
    pub edge_columns: usize,
    /// Regraded columns attributed to this side, raised.
    pub raised: usize,
    /// Regraded columns attributed to this side, lowered.
    pub lowered: usize,
    /// The least and greatest natural surface minus `P` over the columns one
    /// beyond the ring along this side, inside the region; `None` when no
    /// such column is in the region or the site has no terrain.
    pub drop: Option<[i64; 2]>,
    /// The furthest regraded column's step count; `0` when none.
    pub reach: i64,
}

/// One column the grade moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regraded {
    /// World `x`.
    pub x: i64,
    /// World `z`.
    pub z: i64,
    /// The terrain's own surface there.
    pub natural: i64,
    /// The surface the grade gave it.
    pub graded: i64,
    /// The place whose bound set it (the first in plan order where two agree),
    /// as an index into the boxes.
    pub place: usize,
    /// The side of that place its nearest edge column stands on.
    pub face: Face,
    /// That place's step count at this column: how many runs of its bank
    /// the column stands out.
    pub k: i64,
    /// The bound the column was held to, `[lo, hi]`.
    pub bound: [i64; 2],
}

/// Which claim of `DW1108`'s first shape a column breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Meeting {
    /// A whole column where one place's bank must stand higher than another's
    /// may reach (`lo > hi`).
    Banks,
    /// An edge column of one place outside another's bound.
    Edge,
    /// A column at a place's wall or sheer foot another's bank would change.
    Foot,
}

/// One column where two settings meet (`DW1108`, first shape).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Clash {
    /// The place whose bank reaches the column (a box index).
    pub bank: usize,
    /// The side of it the bank comes from.
    pub bank_face: Face,
    /// The other place (a box index).
    pub other: usize,
    /// The other's side there.
    pub other_face: Face,
    /// What is broken.
    pub meeting: Meeting,
    /// The column `[x, z]`.
    pub column: [i64; 2],
    /// The height the bank asks for there.
    pub bank_height: i64,
    /// The height the other place holds there.
    pub other_height: i64,
}

/// The grade of one site: what [`Ground::of`] resolved, and everything the
/// ledger, the line and `DW1108` read of it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Grade {
    /// True on an `open` fill whose terrain resolved: the only site that
    /// grades.
    pub terrain: bool,
    /// Region min `[x, z]` and the grid's width (`x`) and depth (`z`).
    min: [i64; 2],
    width: i64,
    depth: i64,
    /// The graded surface over the region, row-major over `z` then `x`.
    graded: Vec<i64>,
    /// The ground places, as box indices, in plan order.
    pub ground_places: Vec<usize>,
    /// Every unwalled side of a ground place, in plan order and face order.
    pub sides: Vec<GradeSide>,
    /// Every column the grade moved, in `x`-then-`z` order.
    pub regraded: Vec<Regraded>,
    /// Edge columns, over every place.
    pub edge_columns: usize,
    /// Whole columns in the region.
    pub whole_columns: usize,
    /// Where two settings meet, in column order.
    pub clashes: Vec<Clash>,
}

/// The faces a ring column stands on, in [`Sides::FACES`] order.
fn ring_faces(foot: [i64; 4], x: i64, z: i64) -> Vec<Face> {
    let [x0, x1, z0, z1] = foot;
    let mut out = Vec::new();
    if x == x1 + 1 {
        out.push(Face::East);
    }
    if x == x0 - 1 {
        out.push(Face::West);
    }
    if z == z1 + 1 {
        out.push(Face::South);
    }
    if z == z0 - 1 {
        out.push(Face::North);
    }
    out
}

/// The ring columns of `foot`, each once, in face order then along the face.
fn ring_columns(foot: [i64; 4]) -> Vec<[i64; 2]> {
    let [x0, x1, z0, z1] = foot;
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut put = |c: [i64; 2], out: &mut Vec<[i64; 2]>| {
        if seen.insert(c) {
            out.push(c);
        }
    };
    for z in z0 - 1..=z1 + 1 {
        put([x1 + 1, z], &mut out);
    }
    for z in z0 - 1..=z1 + 1 {
        put([x0 - 1, z], &mut out);
    }
    for x in x0 - 1..=x1 + 1 {
        put([x, z1 + 1], &mut out);
    }
    for x in x0 - 1..=x1 + 1 {
        put([x, z0 - 1], &mut out);
    }
    out
}

/// The least `k ≥ 0` with `q ≤ (1 + run·k)²` (spec-0138 §4), in integers:
/// how many runs of the bank a column stands from an edge column at squared
/// distance `q`.
#[must_use]
pub fn steps(q: i64, run: i64) -> i64 {
    if q <= 0 {
        return 0;
    }
    let q = q as u64;
    let mut s = q.isqrt();
    if s * s < q {
        s += 1;
    }
    // `(1 + run·k)² ≥ q` ⇔ `1 + run·k ≥ ⌈√q⌉`.
    let s = s as i64;
    (s - 1 + run - 1) / run.max(1)
}

/// How far from `P` a column `k` runs out may stand: `rise · k`.
#[must_use]
pub fn allowance(slope: Slope, k: i64) -> i64 {
    slope.rise() * k
}

/// **Where a whole column settles** (spec-0138 §4): the natural surface `t`
/// held inside the bound `[lo, hi]`, and, where the bound moved it, carried
/// `n` blocks back toward the places that bound it — never past the bound's
/// other side. `n` is the bank's noise ([`Ground::bank_noise`]). Where two
/// banks cannot share the column (`lo > hi`) it stands at `lo`.
#[must_use]
pub fn settle(t: i64, lo: i64, hi: i64, n: i64) -> i64 {
    if lo > hi {
        lo
    } else if t < lo {
        (lo + n).min(hi)
    } else if t > hi {
        (hi - n).max(lo)
    } else {
        t
    }
}

/// A ground place's edge columns, each with the face it is counted on.
fn edges_of(b: &PlacedBox, covered_by_other: impl Fn(i64, i64) -> bool) -> Vec<([i64; 2], Face)> {
    ring_columns(b.foot)
        .into_iter()
        .filter_map(|[x, z]| {
            let faces = ring_faces(b.foot, x, z);
            if faces.is_empty() || !faces.iter().all(|f| b.sides.is_graded(*f)) {
                return None;
            }
            if covered_by_other(x, z) {
                return None;
            }
            Some(([x, z], faces[0]))
        })
        .collect()
}

/// The least squared distance from `c` to `edges`, with the first edge (in
/// order) that attains it.
fn nearest(edges: &[([i64; 2], Face)], c: [i64; 2]) -> Option<(i64, Face)> {
    let mut best: Option<(i64, Face)> = None;
    for (e, f) in edges {
        let q = (e[0] - c[0]).pow(2) + (e[1] - c[1]).pow(2);
        if best.is_none_or(|(bq, _)| q < bq) {
            best = Some((q, *f));
        }
    }
    best
}

/// One place's bank: each column it moves, with its allowance there, its
/// step count and the side of its nearest edge.
type Bank = BTreeMap<[i64; 2], (i64, i64, Face)>;

fn shell(b: &PlacedBox) -> [i64; 4] {
    [b.foot[0] - 1, b.foot[1] + 1, b.foot[2] - 1, b.foot[3] + 1]
}

/// Whether two places **meet**: their shells overlap or touch, so no whole
/// column stands between them. Where two places meet, the join is theirs to
/// draw (spec-0138 §10): no shape of `DW1108` is raised between them.
fn places_meet(a: &PlacedBox, b: &PlacedBox) -> bool {
    let (p, q) = (shell(a), shell(b));
    p[0] - 1 <= q[1] && q[0] <= p[1] + 1 && p[2] - 1 <= q[3] && q[2] <= p[3] + 1
}

impl Grade {
    /// Resolve the grade of `boxes` over a site whose natural surface over
    /// the region is `natural` (row-major `z` then `x`, `width × depth`, from
    /// `min`), or over a site with no terrain (`natural` `None`).
    #[must_use]
    pub fn of(
        boxes: &[PlacedBox],
        min: [i64; 2],
        width: i64,
        depth: i64,
        natural: Option<&[i64]>,
        seed: u64,
    ) -> Grade {
        let ground_places: Vec<usize> = (0..boxes.len())
            .filter(|i| boxes[*i].aloft().is_none())
            .collect();
        let Some(natural) = natural else {
            return Grade {
                terrain: false,
                ground_places: ground_places.clone(),
                sides: Self::sheer_sides(boxes, &ground_places),
                ..Grade::default()
            };
        };
        let (w, d) = (width.max(0) as usize, depth.max(0) as usize);
        let idx = |x: i64, z: i64| -> Option<usize> {
            let (px, pz) = (x - min[0], z - min[1]);
            (px >= 0 && pz >= 0 && px < width && pz < depth).then(|| (pz * width + px) as usize)
        };
        // How many ground places' shell footprints cover each column.
        let mut cover = vec![0u8; w * d];
        for &i in &ground_places {
            let s = shell(&boxes[i]);
            for x in s[0]..=s[1] {
                for z in s[2]..=s[3] {
                    if let Some(k) = idx(x, z) {
                        cover[k] = cover[k].saturating_add(1);
                    }
                }
            }
        }
        // Every place's edge columns, in the region.
        let edges: BTreeMap<usize, Vec<([i64; 2], Face)>> = ground_places
            .iter()
            .map(|&i| {
                let e: Vec<([i64; 2], Face)> =
                    edges_of(&boxes[i], |x, z| idx(x, z).is_none_or(|k| cover[k] > 1));
                (i, e)
            })
            .filter(|(_, e)| !e.is_empty())
            .collect();
        let surface = |i: usize| boxes[i].floor_course_y();

        // A place's allowance at a column, and the side of its nearest edge.
        let reach_of = |i: usize, c: [i64; 2]| -> Option<(i64, i64, Face)> {
            let b = &boxes[i];
            nearest(&edges[&i], c).map(|(q, f)| {
                let k = steps(q, b.slope.run());
                (allowance(b.slope, k), k, f)
            })
        };
        let whole = |c: [i64; 2]| idx(c[0], c[1]).is_some_and(|k| cover[k] == 0);
        let nat = |c: [i64; 2]| natural[idx(c[0], c[1]).unwrap_or(0)];

        // **Each place's bank** (§4): the whole columns joined to its edge,
        // 4-adjacent, through columns its own bound moves. It ends at the
        // first column the terrain already meets, and never reaches past it.
        let mut banks: BTreeMap<usize, Bank> = BTreeMap::new();
        for (&i, e) in &edges {
            let p = surface(i);
            let mut bank: Bank = BTreeMap::new();
            let mut todo: std::collections::VecDeque<[i64; 2]> = std::collections::VecDeque::new();
            let mut seen: BTreeSet<[i64; 2]> = BTreeSet::new();
            for (c, _) in e {
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = [c[0] + dx, c[1] + dz];
                    if whole(n) && seen.insert(n) {
                        todo.push_back(n);
                    }
                }
            }
            while let Some(c) = todo.pop_front() {
                let Some((a, k, f)) = reach_of(i, c) else {
                    continue;
                };
                let t = nat(c);
                if t >= p - a && t <= p + a {
                    continue;
                }
                bank.insert(c, (a, k, f));
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = [c[0] + dx, c[1] + dz];
                    if whole(n) && seen.insert(n) {
                        todo.push_back(n);
                    }
                }
            }
            banks.insert(i, bank);
        }
        // The banks that reach each column, in plan order.
        let mut reaching: BTreeMap<[i64; 2], Vec<usize>> = BTreeMap::new();
        for (&i, bank) in &banks {
            for c in bank.keys() {
                reaching.entry(*c).or_default().push(i);
            }
        }
        // The bound over a set of places at a column, with the gentlest rise
        // on each side: `(lo, hi, lo_rise, hi_rise)`.
        let bound = |c: [i64; 2], places: &mut dyn Iterator<Item = usize>| {
            let (mut lo, mut hi, mut lr, mut hr) = (i64::MIN, i64::MAX, 0, 0);
            for i in places {
                let (a, _, _) = banks[&i][&c];
                let (p, r) = (surface(i), boxes[i].slope.rise());
                let (l, h) = (p - a, p + a);
                if l > lo || (l == lo && r < lr) {
                    lr = r;
                }
                if h < hi || (h == hi && r < hr) {
                    hr = r;
                }
                lo = lo.max(l);
                hi = hi.min(h);
            }
            (lo, hi, lr, hr)
        };

        // The graded surface.
        let mut graded = natural.to_vec();
        let whole_columns = cover.iter().filter(|c| **c == 0).count();
        let mut regraded = Vec::new();
        let mut clashes = Vec::new();
        // The first place in `among` whose own bound is `want` on the side
        // that moved the column.
        let setter = |c: [i64; 2], among: &[usize], want: i64, up: bool| {
            among.iter().copied().find(|&i| {
                let (a, _, _) = banks[&i][&c];
                if up {
                    surface(i) - a == want
                } else {
                    surface(i) + a == want
                }
            })
        };
        for (&c, among) in &reaching {
            let k = idx(c[0], c[1]).unwrap_or(0);
            let t = natural[k];
            let (lo, hi, lr, hr) = bound(c, &mut among.iter().copied());
            if lo > hi {
                // (a) two banks that cannot share the column.
                if let (Some(a), Some(b)) =
                    (setter(c, among, lo, true), setter(c, among, hi, false))
                    && !places_meet(&boxes[a], &boxes[b])
                {
                    clashes.push(Clash {
                        bank: a,
                        bank_face: banks[&a][&c].2,
                        other: b,
                        other_face: banks[&b][&c].2,
                        meeting: Meeting::Banks,
                        column: c,
                        bank_height: lo,
                        other_height: hi,
                    });
                }
            }
            let amp = if t < lo { lr } else { hr };
            let g = settle(t, lo, hi, Ground::bank_noise(seed, c[0], c[1], amp));
            graded[k] = g;
            if g == t {
                continue;
            }
            let raised = g > t;
            let place = setter(c, among, if raised { lo } else { hi }, raised).unwrap_or(among[0]);
            let (_, kk, face) = banks[&place][&c];
            regraded.push(Regraded {
                x: c[0],
                z: c[1],
                natural: t,
                graded: g,
                place,
                face,
                k: kk,
                bound: [lo, hi],
            });
        }

        // (b) the foot of a place: a whole column beside a ring column of `j`
        // that no other ground place's shell covers is held, before the
        // noise, where `j`'s own bank alone would hold it — by the banks of
        // the places that do not meet `j`.
        for &j in &ground_places {
            let b = &boxes[j];
            let mut foot: BTreeMap<[i64; 2], Face> = BTreeMap::new();
            for [x, z] in ring_columns(b.foot) {
                let Some(k) = idx(x, z) else { continue };
                if cover[k] > 1 {
                    continue;
                }
                let face = ring_faces(b.foot, x, z)[0];
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = [x + dx, z + dz];
                    if whole(n) {
                        foot.entry(n).or_insert(face);
                    }
                }
            }
            for (c, face) in foot {
                let Some(among) = reaching.get(&c) else {
                    continue;
                };
                let others: Vec<usize> = among
                    .iter()
                    .copied()
                    .filter(|i| *i != j && !places_meet(&boxes[*i], b))
                    .collect();
                if others.is_empty() {
                    continue;
                }
                let t = nat(c);
                let own: Vec<usize> = among.iter().copied().filter(|i| *i == j).collect();
                let alone = if own.is_empty() {
                    t
                } else {
                    let (lo, hi, _, _) = bound(c, &mut own.iter().copied());
                    settle(t, lo, hi, 0)
                };
                let mut with: Vec<usize> = own.clone();
                with.extend(others.iter().copied());
                let (lo, hi, _, _) = bound(c, &mut with.iter().copied());
                let held = settle(t, lo, hi, 0);
                if held == alone {
                    continue;
                }
                let up = held > alone;
                let Some(i) = setter(c, &others, if up { lo } else { hi }, up) else {
                    continue;
                };
                clashes.push(Clash {
                    bank: i,
                    bank_face: banks[&i][&c].2,
                    other: j,
                    other_face: face,
                    meeting: Meeting::Foot,
                    column: c,
                    bank_height: held,
                    other_height: alone,
                });
            }
        }
        clashes.sort_by_key(|c| (c.column, c.meeting, c.bank, c.other));

        // The ledger's rows.
        let mut sides = Vec::new();
        for &i in &ground_places {
            let b = &boxes[i];
            let p = surface(i);
            for face in Sides::FACES {
                if !b.sides.is_open(face) {
                    continue;
                }
                let meets = if b.sides.is_graded(face) {
                    Meets::Graded
                } else {
                    Meets::Sheer
                };
                let [x0, x1, z0, z1] = b.foot;
                let beyond: Vec<[i64; 2]> = match face {
                    Face::East => (z0..=z1).map(|z| [x1 + 2, z]).collect(),
                    Face::West => (z0..=z1).map(|z| [x0 - 2, z]).collect(),
                    Face::South => (x0..=x1).map(|x| [x, z1 + 2]).collect(),
                    _ => (x0..=x1).map(|x| [x, z0 - 2]).collect(),
                };
                let drops: Vec<i64> = beyond
                    .iter()
                    .filter_map(|c| idx(c[0], c[1]).map(|k| natural[k] - p))
                    .collect();
                let drop = drops
                    .iter()
                    .min()
                    .zip(drops.iter().max())
                    .map(|(a, b)| [*a, *b]);
                let mine = regraded.iter().filter(|r| r.place == i && r.face == face);
                let (mut raised, mut lowered, mut reach) = (0, 0, 0);
                for r in mine {
                    if r.graded > r.natural {
                        raised += 1;
                    } else {
                        lowered += 1;
                    }
                    reach = reach.max(r.k);
                }
                sides.push(GradeSide {
                    node: b.node.clone(),
                    face,
                    meets,
                    surface: p,
                    edge_columns: edges
                        .get(&i)
                        .map_or(0, |e| e.iter().filter(|(_, f)| *f == face).count()),
                    raised,
                    lowered,
                    drop,
                    reach,
                });
            }
        }

        Grade {
            terrain: true,
            min,
            width,
            depth,
            graded,
            ground_places,
            sides,
            regraded,
            edge_columns: edges.values().map(Vec::len).sum(),
            whole_columns,
            clashes,
        }
    }

    /// Every unwalled side of a ground place on a site with no terrain: none
    /// graded.
    fn sheer_sides(boxes: &[PlacedBox], ground: &[usize]) -> Vec<GradeSide> {
        let mut out = Vec::new();
        for &i in ground {
            let b = &boxes[i];
            for face in Sides::FACES {
                if !b.sides.is_open(face) {
                    continue;
                }
                out.push(GradeSide {
                    node: b.node.clone(),
                    face,
                    meets: if b.sides.is_graded(face) {
                        Meets::Graded
                    } else {
                        Meets::Sheer
                    },
                    surface: b.floor_course_y(),
                    edge_columns: 0,
                    raised: 0,
                    lowered: 0,
                    drop: None,
                    reach: 0,
                });
            }
        }
        out
    }

    /// The graded surface over the region (`width × depth`, row-major `z`
    /// then `x`), when the grade moved any column.
    pub(super) fn surface(&self) -> Option<(i64, i64, &[i64])> {
        (!self.regraded.is_empty()).then_some((self.width, self.depth, self.graded.as_slice()))
    }

    /// The graded surface at `(x, z)` inside the region; `None` outside it or
    /// on a site with no terrain.
    #[must_use]
    pub fn top(&self, x: i64, z: i64) -> Option<i64> {
        let (px, pz) = (x - self.min[0], z - self.min[1]);
        if !self.terrain || px < 0 || pz < 0 || px >= self.width || pz >= self.depth {
            return None;
        }
        self.graded.get((pz * self.width + px) as usize).copied()
    }

    /// The moved column at `(x, z)`, if the grade moved it.
    #[must_use]
    pub fn regraded_at(&self, x: i64, z: i64) -> Option<&Regraded> {
        self.regraded
            .binary_search_by(|r| (r.x, r.z).cmp(&(x, z)))
            .ok()
            .map(|k| &self.regraded[k])
    }

    /// Unwalled sides counted: `(U, S, H)`.
    #[must_use]
    pub fn side_counts(&self) -> (usize, usize, usize) {
        let s = self
            .sides
            .iter()
            .filter(|r| self.terrain && r.meets == Meets::Graded)
            .count();
        let h = self
            .sides
            .iter()
            .filter(|r| r.meets == Meets::Sheer)
            .count();
        (self.sides.len(), s, h)
    }

    /// Raised and lowered columns: `(R, L)`.
    #[must_use]
    pub fn raised_lowered(&self) -> (usize, usize) {
        let r = self
            .regraded
            .iter()
            .filter(|c| c.graded > c.natural)
            .count();
        (r, self.regraded.len() - r)
    }

    /// The greatest drop a graded side meets: the greatest `|natural − P|`
    /// over the columns one beyond the ring of every graded side.
    #[must_use]
    pub fn greatest_drop(&self) -> i64 {
        self.sides
            .iter()
            .filter(|s| s.meets == Meets::Graded)
            .filter_map(|s| s.drop)
            .map(|[a, b]| a.abs().max(b.abs()))
            .max()
            .unwrap_or(0)
    }

    /// The text the ledger's `terrain_sha256` is taken over: one line
    /// `x z natural graded\n` per regraded column, in `x`-then-`z` order.
    #[must_use]
    pub fn terrain_rows(&self) -> String {
        terrain_rows(&self.regraded)
    }
}

/// One line `x z natural graded\n` per row, in the order given: what the
/// ledger's `terrain_sha256` hashes, as one function a test can recompute.
#[must_use]
pub fn terrain_rows(rows: &[Regraded]) -> String {
    let mut text = String::new();
    for r in rows {
        text.push_str(&format!("{} {} {} {}\n", r.x, r.z, r.natural, r.graded));
    }
    text
}

/// **What the grade examined, and what it did** (spec-0138 §8): the `grade:`
/// line every `validate` and `build` of a campaign with a site plan prints,
/// and the ledger `validation/grade.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GradeBinding {
    /// The grade, resolved.
    pub grade: Grade,
    /// True on a `solid` fill.
    pub solid: bool,
    /// Regraded columns a declared volume covers.
    pub under_volumes: usize,
}

impl GradeBinding {
    /// The binding of `c`'s site plan; `None` when it has none.
    #[must_use]
    pub fn of(c: &Campaign) -> Option<GradeBinding> {
        let plan = c.site_plan.as_ref().map(|p| &p.content)?;
        let ground = Ground::of(c);
        let grade = ground.grade().cloned().unwrap_or_default();
        let under_volumes = grade
            .regraded
            .iter()
            .filter(|r| {
                let (lo, hi) = (r.natural.min(r.graded), r.natural.max(r.graded));
                plan.volumes.iter().any(|v| {
                    let (a, b) = (v.region.min, v.region.max());
                    r.x >= a[0]
                        && r.x <= b[0]
                        && r.z >= a[2]
                        && r.z <= b[2]
                        && a[1] <= hi
                        && b[1] >= lo
                })
            })
            .count();
        Some(GradeBinding {
            grade,
            solid: matches!(plan.fill, Fill::Solid { .. }),
            under_volumes,
        })
    }

    /// The `grade:` line.
    #[must_use]
    pub fn line(&self) -> String {
        let g = &self.grade;
        let (u, s, h) = g.side_counts();
        if self.solid {
            return format!("grade: a solid fill has no terrain; 0 of {u} unwalled side(s) graded");
        }
        if !g.terrain {
            return format!("grade: the terrain did not resolve; 0 of {u} unwalled side(s) graded");
        }
        let (r, l) = g.raised_lowered();
        format!(
            "grade: {u} unwalled side(s) of {gp} ground place(s) — {s} graded, {h} sheer; \
             {e} edge column(s); {c} column(s) regraded of {w} whole column(s) ({r} raised, \
             {l} lowered); greatest drop met {dm}; {v} regraded column(s) under a declared volume",
            gp = g.ground_places.len(),
            e = g.edge_columns,
            c = g.regraded.len(),
            w = g.whole_columns,
            dm = g.greatest_drop(),
            v = self.under_volumes,
        )
    }

    /// The ledger `validation/grade.json`, its `terrain_sha256` the caller's
    /// hash of [`Grade::terrain_rows`].
    #[must_use]
    pub fn to_json(&self, terrain_sha256: &str) -> serde_json::Value {
        let g = &self.grade;
        let (u, s, h) = g.side_counts();
        let (r, l) = g.raised_lowered();
        serde_json::json!({
            "line": self.line(),
            "solid": self.solid,
            "terrain": g.terrain,
            "unwalled_sides": u,
            "ground_places": g.ground_places.len(),
            "graded_sides": s,
            "sheer_sides": h,
            "edge_columns": g.edge_columns,
            "regraded_columns": g.regraded.len(),
            "whole_columns": g.whole_columns,
            "raised": r,
            "lowered": l,
            "greatest_drop": g.greatest_drop(),
            "under_volumes": self.under_volumes,
            "terrain_sha256": terrain_sha256,
            "sides": g.sides.iter().map(|s| serde_json::json!({
                "place": s.node.0,
                "face": s.face.as_str(),
                "meets": s.meets.as_str(),
                "surface": s.surface,
                "edge_columns": s.edge_columns,
                "raised": s.raised,
                "lowered": s.lowered,
                "drop": s.drop,
                "reach": s.reach,
            })).collect::<Vec<_>>(),
        })
    }
}

/// An inclusive pair of cells: a volume's corners, or a removed cell and
/// the fluid cell beside it.
type Cuboid = ([i64; 3], [i64; 3]);

fn face_word(f: Face) -> &'static str {
    f.as_str()
}

/// `DW1108`, both shapes (spec-0138 §7).
pub(super) fn check(c: &Campaign, plan: &SitePlanContent, d: &mut Vec<Diagnostic>) {
    let ground = Ground::of(c);
    let Some(grade) = ground.grade() else { return };
    if !grade.terrain {
        return;
    }
    let boxes = placed_boxes(c, &mut crate::metrics::Reads::new());
    let name = |i: usize| boxes.get(i).map_or_else(String::new, |b| b.node.0.clone());

    // Shape 1: one refusal per pair of places.
    let mut pairs: BTreeMap<(usize, usize), Vec<&Clash>> = BTreeMap::new();
    for cl in &grade.clashes {
        let key = (cl.bank.min(cl.other), cl.bank.max(cl.other));
        pairs.entry(key).or_default().push(cl);
    }
    for (_, list) in pairs {
        let first = list[0];
        let (a, b) = (&boxes[first.bank], &boxes[first.other]);
        let between = gap(a, b);
        // The difference a bank must climb: to the other place's surface, or,
        // at a foot, to the ground the other place keeps there — the
        // greatest over the pair's columns.
        let climb = |c: &Clash| {
            let (p, q) = (&boxes[c.bank], &boxes[c.other]);
            let diff = match c.meeting {
                Meeting::Banks | Meeting::Edge => (p.floor_course_y() - q.floor_course_y()).abs(),
                Meeting::Foot => (p.floor_course_y() - c.other_height).abs(),
            };
            (
                diff,
                p.slope.run() * ((diff + p.slope.rise() - 1) / p.slope.rise()),
            )
        };
        let (diff, need) = list
            .iter()
            .map(|c| climb(c))
            .max_by_key(|(_, n)| *n)
            .unwrap_or((0, 0));
        let need = need.max(between + 1);
        let distinct: BTreeSet<[i64; 2]> = list.iter().map(|c| c.column).collect();
        let shown: Vec<String> = distinct
            .iter()
            .take(6)
            .map(|c| format!("[{}, {}]", c[0], c[1]))
            .collect();
        let count = distinct.len();
        let what = match first.meeting {
            Meeting::Banks => format!(
                "`{bn}`'s bank from its {bf} side must stand at least at y {bh} in column \
                 [{x}, {z}] and `{on}`'s bank from its {of} side at most at y {oh}",
                bn = name(first.bank),
                bf = face_word(first.bank_face),
                bh = first.bank_height,
                on = name(first.other),
                of = face_word(first.other_face),
                oh = first.other_height,
                x = first.column[0],
                z = first.column[1],
            ),
            Meeting::Edge => format!(
                "`{on}`'s {of} side stands its edge at y {oh} in column [{x}, {z}], where \
                 `{bn}`'s bank from its {bf} side can reach no further than y {bh}",
                bn = name(first.bank),
                bf = face_word(first.bank_face),
                bh = first.bank_height,
                on = name(first.other),
                of = face_word(first.other_face),
                oh = first.other_height,
                x = first.column[0],
                z = first.column[1],
            ),
            Meeting::Foot => format!(
                "`{bn}`'s bank from its {bf} side would put the ground at y {bh} in column \
                 [{x}, {z}], at the foot of `{on}`'s {of} side, where `{on}` alone stands it \
                 at y {oh}",
                bn = name(first.bank),
                bf = face_word(first.bank_face),
                bh = first.bank_height,
                on = name(first.other),
                of = face_word(first.other_face),
                oh = first.other_height,
                x = first.column[0],
                z = first.column[1],
            ),
        };
        d.push(Diagnostic::error(
            DW_GRADE_NO_ROOM,
            "site-plan",
            format!("/content/boxes/{}", first.bank),
            format!(
                "the ground between `{a}` and `{b}` has no room to step from one to the other: \
                 {what}. {n} column(s) break the same way (first {shown}). There are {between} \
                 whole column(s) between the two places and a difference of {diff} block(s) \
                 needs {need}. Move the places apart by {more} column(s); or declare `{bn}`'s \
                 {bf} side `sheer`, so its ground is left as the terrain has it; or join them \
                 with a place that carries the rise (a stair, a ramp).",
                a = a.node,
                b = b.node,
                n = count,
                shown = shown.join(", "),
                more = (need - between).max(1),
                bn = name(first.bank),
                bf = face_word(first.bank_face),
            ),
        ));
    }

    // Shape 2: a cut beside a declared fluid, one refusal per side and volume.
    let blocks: Vec<(usize, Cuboid, bool)> = plan
        .volumes
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let fluid = v.block.as_deref().is_some_and(crate::blockshape::is_fluid);
            (k, (v.region.min, v.region.max()), fluid)
        })
        .collect();
    let inside =
        |c: [i64; 3], r: ([i64; 3], [i64; 3])| (0..3).all(|a| c[a] >= r.0[a] && c[a] <= r.1[a]);
    // A removed cell a solid volume fills is not removed.
    let filled = |c: [i64; 3]| {
        plan.volumes.iter().any(|v| {
            v.role != VolumeRole::Clearance
                && !v.block.as_deref().is_some_and(crate::blockshape::is_fluid)
                && inside(c, (v.region.min, v.region.max()))
        })
    };
    let mut cuts: BTreeMap<(usize, Face, usize), Vec<Cuboid>> = BTreeMap::new();
    for r in grade.regraded.iter().filter(|r| r.graded < r.natural) {
        for y in r.graded + 1..=r.natural {
            let cell = [r.x, y, r.z];
            if filled(cell) || blocks.iter().any(|(_, reg, fl)| *fl && inside(cell, *reg)) {
                continue;
            }
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0)] {
                let n = [cell[0] + dx, cell[1] + dy, cell[2] + dz];
                if let Some((k, _, _)) = blocks.iter().find(|(_, reg, fl)| *fl && inside(n, *reg)) {
                    cuts.entry((r.place, r.face, *k))
                        .or_default()
                        .push((cell, n));
                    break;
                }
            }
        }
    }
    for ((place, face, k), cells) in cuts {
        let v = &plan.volumes[k];
        let (cell, n) = cells[0];
        let shown: Vec<String> = cells
            .iter()
            .take(6)
            .map(|(c, _)| format!("[{}, {}, {}]", c[0], c[1], c[2]))
            .collect();
        d.push(Diagnostic::error(
            DW_GRADE_NO_ROOM,
            "site-plan",
            format!("/content/volumes/{k}"),
            format!(
                "the ground beyond `{p}`'s {f} side is cut down to it, and the cut removes \
                 cell [{x}, {y}, {z}], {rel} cell [{nx}, {ny}, {nz}] of volume `{vid}`, which \
                 is a fluid: the fluid would run into the cut. {n} cell(s) are removed the same \
                 way (first {shown}). Declare `{p}`'s {f} side `sheer`, so its ground is left as \
                 the terrain has it; or move the volume or the place.",
                p = name(place),
                f = face_word(face),
                x = cell[0],
                y = cell[1],
                z = cell[2],
                rel = if n[1] > cell[1] { "under" } else { "beside" },
                nx = n[0],
                ny = n[1],
                nz = n[2],
                vid = v.id,
                n = cells.len(),
                shown = shown.join(", "),
            ),
        ));
    }
}

/// The whole columns between two places' shells, along the axis they are
/// furthest apart on.
fn gap(a: &PlacedBox, b: &PlacedBox) -> i64 {
    let (p, q) = (shell(a), shell(b));
    let gx = (q[0] - p[1] - 1).max(p[0] - q[1] - 1).max(0);
    let gz = (q[2] - p[3] - 1).max(p[2] - q[3] - 1).max(0);
    gx.max(gz)
}
