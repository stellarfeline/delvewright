//! **The shut-gate model**: the one answer to *which quest configurations are
//! judged, where a shut gate's two faces are, where the party can get to, and
//! which faces of which shut gates it meets*. Two proofs ask it and neither
//! holds a copy: `DW1011` (a shut gate separates its two sides,
//! [`super::separate`]) and `DW1053`/`DW1054` (a shut way says how the player
//! learns it is shut, [`super::learned`]). An agreement test drives both over
//! generated worlds and asserts they meet the same (gate, configuration, face)
//! triples (spec-0118 §5).

use super::*;
use crate::compiler::plan::Plan;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One gate a proof judges: how a report names it, its region, and which of
/// its two faces opens it when a declaration says so.
#[derive(Debug, Clone)]
pub(super) struct GateUnderTest {
    /// The gate in words: the gated connection it bars, or its anchor and area.
    pub label: String,
    /// The region's inclusive corners.
    pub region: ([i32; 3], [i32; 3]),
    /// The axis the gate is thin on, when something states it: a gated
    /// connection's seam hands its own normal (spec-0107 §6), so a bar one cell
    /// on two axes — the grating over a one-wide hole — is judged on that axis's
    /// two faces rather than guessed at. `None` for every other gate, whose axis
    /// is read off its shape.
    pub axis: Option<usize>,
    /// `Some(true)` when only the low face (of the thin axis) opens it,
    /// `Some(false)` when only the high face does, `None` when either does.
    pub opens_low: Option<bool>,
    /// Where the openable side's anchor stands, when the gate is one-sided —
    /// resolved to [`GateUnderTest::opens_low`] once the thin axis is known.
    pub unlock: Option<[i32; 3]>,
}

/// A gate by name and region, for an escape path to name the gates it passes.
pub(super) type NamedGate = (String, ([i32; 3], [i32; 3]));

/// One quest configuration a proof judges: its world, the configuration in
/// words, its route cells, and the gates shut in it (indices into the gates).
pub(super) type SeparationConfig<'w> = (&'w World, String, Vec<[i32; 3]>, Vec<usize>);

/// The two faces of a gate: the cells a body may stand or float in either side
/// of its thin axis, low first. Candidates — each configuration keeps those a
/// body can be in there.
#[derive(Debug, Clone)]
pub(super) struct Faces {
    pub(super) axis: usize,
    pub(super) low: Vec<[i32; 3]>,
    pub(super) high: Vec<[i32; 3]>,
}

/// Whether a body can be in `c`: standing, holding a climbable (spec-0099), or
/// afloat at the top of water.
pub(super) fn can_be(w: &World, c: [i32; 3]) -> bool {
    w.holds_body(c) || w.is_water_surface(c)
}

/// The candidate cells on one face of a gate region, for thin axis `axis`.
/// A horizontal axis takes the column of cells just outside the slab over its
/// span, from a course under its bottom to its top (a body standing at a door
/// stands with its feet in the doorway's bottom course or a step below it); the
/// vertical axis takes the course on top and the four courses under it.
fn face_cells(lo: [i32; 3], hi: [i32; 3], axis: usize, high: bool) -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    if axis == 1 {
        let ys: Vec<i32> = if high {
            vec![hi[1] + 1]
        } else {
            (lo[1] - 4..lo[1]).collect()
        };
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                for &y in &ys {
                    out.push([x, y, z]);
                }
            }
        }
        return out;
    }
    let other = 2 - axis;
    let at = if high { hi[axis] + 1 } else { lo[axis] - 1 };
    for o in lo[other]..=hi[other] {
        for y in lo[1] - 1..=hi[1] {
            let mut c = [0; 3];
            c[axis] = at;
            c[other] = o;
            c[1] = y;
            out.push(c);
        }
    }
    out
}

/// The faces of a gate region over `base` (the world with every gate open), or
/// `None` when its shape names no two: the handed axis when one is stated;
/// else the thin axis is the unique smallest extent, or — among axes tied for
/// smallest — the one axis whose two faces both hold a cell a body can be in.
pub(super) fn faces_of(
    base: &World,
    region: ([i32; 3], [i32; 3]),
    axis: Option<usize>,
) -> Option<Faces> {
    let (a, b) = region;
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let extent = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let faces = |axis: usize| Faces {
        axis,
        low: face_cells(lo, hi, axis, false),
        high: face_cells(lo, hi, axis, true),
    };
    if let Some(axis) = axis {
        return Some(faces(axis));
    }
    let least = *extent.iter().min()?;
    let tied: Vec<usize> = (0..3).filter(|&i| extent[i] == least).collect();
    if tied.len() == 1 {
        return Some(faces(tied[0]));
    }
    let open: Vec<Faces> = tied
        .into_iter()
        .map(faces)
        .filter(|f| {
            f.low.iter().any(|c| can_be(base, *c)) && f.high.iter().any(|c| can_be(base, *c))
        })
        .collect();
    match open.len() {
        1 => open.into_iter().next(),
        _ => None,
    }
}

/// Which face of a one-sided gate opens it: the declaration's own answer, else
/// the side of the thin axis its unlock anchor stands on; `None` for either.
pub(super) fn opens_low(g: &GateUnderTest, f: Option<&Faces>) -> Option<bool> {
    g.opens_low.or_else(|| {
        let (f, u) = (f?, g.unlock?);
        let (lo, hi) = g.region;
        let a = f.axis;
        if u[a] < lo[a].min(hi[a]) {
            Some(true)
        } else if u[a] > lo[a].max(hi[a]) {
            Some(false)
        } else {
            None
        }
    })
}

/// The body-movement closure of `roots` over `w`.
pub(super) fn closure(w: &World, roots: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
    let mut seen: BTreeSet<[i32; 3]> = roots.iter().copied().collect();
    let mut queue: VecDeque<[i32; 3]> = seen.iter().copied().collect();
    while let Some(cur) = queue.pop_front() {
        for n in w.body_moves(cur) {
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

/// The party's reach in one configuration: the route cells a body can be in,
/// and the body-movement closure of them.
pub(super) fn party_reach(w: &World, route: &[[i32; 3]]) -> (Vec<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let seeds: Vec<[i32; 3]> = route.iter().copied().filter(|c| can_be(w, *c)).collect();
    let reach = closure(w, &seeds);
    (seeds, reach)
}

/// How the party meets one shut gate in one configuration: each face's cells a
/// body can be in there, and whether the party's reach holds a cell of each.
#[derive(Debug, Clone)]
pub(super) struct Meeting {
    pub(super) low: Vec<[i32; 3]>,
    pub(super) high: Vec<[i32; 3]>,
    pub(super) low_in: bool,
    pub(super) high_in: bool,
}

impl Meeting {
    /// A face has no cell a body can be in: the gate cannot be judged here.
    pub(super) fn unstood(&self) -> bool {
        self.low.is_empty() || self.high.is_empty()
    }

    /// The faces the party is at — `false` for the low one, `true` for the
    /// high one — none where the meeting is unstood.
    pub(super) fn met(&self) -> Vec<bool> {
        if self.unstood() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.low_in {
            out.push(false);
        }
        if self.high_in {
            out.push(true);
        }
        out
    }

    /// One face's cells a body can be in.
    pub(super) fn face(&self, high: bool) -> &Vec<[i32; 3]> {
        if high { &self.high } else { &self.low }
    }
}

/// How the party, with `reach`, meets a gate whose faces are `f`, over `w`.
pub(super) fn meet(w: &World, f: &Faces, reach: &BTreeSet<[i32; 3]>) -> Meeting {
    let low: Vec<[i32; 3]> = f.low.iter().copied().filter(|c| can_be(w, *c)).collect();
    let high: Vec<[i32; 3]> = f.high.iter().copied().filter(|c| can_be(w, *c)).collect();
    let low_in = low.iter().any(|c| reach.contains(c));
    let high_in = high.iter().any(|c| reach.contains(c));
    Meeting {
        low,
        high,
        low_in,
        high_in,
    }
}

/// One gated connection (spec-0107): the edge, the axis its seam hands, and
/// whether its `a` end lies on the low side of that axis.
#[derive(Debug, Clone, Copy)]
pub(super) struct GatedConnection<'p> {
    pub(super) edge: &'p delvewright_dsl::layout::Edge,
    pub(super) axis: Option<usize>,
    pub(super) a_low: Option<bool>,
}

/// Every gated connection the layout graph declares — every connection the
/// world builds shut ([`delvewright_dsl::layout::Edge::shuts`]) — keyed by the
/// gate anchor the derivation names for it, with the axis its seam hands.
pub(super) fn gated_connections(plan: &Plan) -> BTreeMap<String, GatedConnection<'_>> {
    let graph = plan.campaign.layout_graph.as_ref().map(|g| &g.content);
    let seams = delvewright_dsl::siteplan::SitePlan::of(plan.campaign).seams;
    let mut gated: BTreeMap<String, GatedConnection<'_>> = BTreeMap::new();
    for e in graph.map(|g| g.edges.iter()).into_iter().flatten() {
        if e.shuts() {
            let seam = seams.iter().find(|s| &s.edge == e.id());
            let axis = seam.map(delvewright_dsl::siteplan::PlacedSeam::bar_axis);
            let a_low = seam.map(|s| s.face.vector()[s.normal_axis] > 0);
            gated.insert(
                delvewright_dsl::siteplan::seam_anchor(e.id()),
                GatedConnection {
                    edge: e,
                    axis,
                    a_low,
                },
            );
        }
    }
    gated
}

/// Where a named `Point` anchor resolves, or `None`.
pub(super) fn point_anchor(plan: &Plan, name: &str) -> Option<[i32; 3]> {
    plan.anchors.iter().find_map(|((_, n), r)| match r {
        crate::compiler::plan::ResolvedAnchor::Point { pos, .. } if n == name => Some(*pos),
        _ => None,
    })
}

/// The gate under test for a gated connection whose seam gate is `seal`.
pub(super) fn gated_gate(
    plan: &Plan,
    g: &GatedConnection<'_>,
    seal: &crate::compiler::assembled::GateSeal,
) -> GateUnderTest {
    let e = g.edge;
    let terms = e.gating().map(|g| g.words()).unwrap_or_default();
    let unlock = e.opening_side().and_then(|_| {
        point_anchor(plan, &delvewright_dsl::siteplan::seam_unlock_anchor(e.id()))
    });
    GateUnderTest {
        label: format!(
            "gated {} `{}` (gate `{}`, opened on {terms})",
            e.class(),
            e.id(),
            seal.anchor,
        ),
        region: seal.region,
        axis: g.axis,
        opens_low: None,
        unlock,
    }
}

/// The quest configurations a proof judges: one per distinct region state the
/// critical path passes through, carrying the route cells of every leg that
/// arrives under it — the configurations `DW0921` judges.
pub(super) struct Configurations {
    states: Vec<(RegionState, usize, BTreeSet<[i32; 3]>)>,
    worlds: Vec<Option<World>>,
}

impl Configurations {
    /// The configurations of `plan` over `world`.
    pub(super) fn of(plan: &Plan, world: &World) -> Self {
        let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
        let mut states: Vec<(RegionState, usize, BTreeSet<[i32; 3]>)> = Vec::new();
        for (step, cells) in critical_route_cells(plan, world) {
            let st = world.region_state_at(&plan.region_events, step, &ancestor);
            match states.iter_mut().find(|(s, _, _)| *s == st) {
                Some((_, first, seeds)) => {
                    *first = (*first).min(step);
                    seeds.extend(cells);
                }
                None => states.push((st, step, cells.into_iter().collect())),
            }
        }
        let worlds = states
            .iter()
            .map(|(st, _, _)| (!st.is_empty()).then(|| world.with_region_state(st)))
            .collect();
        Configurations { states, worlds }
    }

    /// Each configuration as a proof judges it: its world, the configuration
    /// in words (`while \`obj/…\` is next`), its route cells, and which of
    /// `regions` are shut in it (every region cell made solid).
    pub(super) fn judged<'w>(
        &'w self,
        plan: &Plan,
        world: &'w World,
        regions: &[([i32; 3], [i32; 3])],
    ) -> Vec<SeparationConfig<'w>> {
        self.states
            .iter()
            .zip(&self.worlds)
            .map(|((st, first, seeds), w)| {
                let w = w.as_ref().unwrap_or(world);
                let when = plan
                    .critical_path
                    .get(*first)
                    .and_then(|s| s.objective())
                    .map(|o| format!("while `{o}` is next"))
                    .unwrap_or_else(|| format!("from critical step {first}"));
                let shut: Vec<usize> = regions
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| {
                        crate::compiler::assembled::region_cells(r.0, r.1)
                            .all(|c| st.solid.contains(&c))
                    })
                    .map(|(i, _)| i)
                    .collect();
                (w, when, seeds.iter().copied().collect(), shut)
            })
            .collect()
    }
}
