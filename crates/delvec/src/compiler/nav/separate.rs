//! The separation relation: while a gate is shut, its far side cannot be
//! reached from its near side by any move a body can make (`DW1011`).
//!
//! The route proofs walk; this floods the reach graph — walking, falling,
//! jumping, climbing and swimming ([`World::body_reach`], spec-0113) — because
//! the question is not where the bot goes on cue but where a player can get to.
//! It is the dual of `DW0921`'s question over the same relation: that one asks
//! whether a body can get back, this one whether a body can get past.

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use std::collections::{BTreeMap, BTreeSet};

/// How many bypassed gates a `DW1011` report names before summarising the rest.
const BYPASS_LIST_LIMIT: usize = 6;

/// How many waypoints a reported escape path names between its two ends.
const WAYPOINTS: usize = 4;

/// What [`check_gates_separate`] measured, whether or not it refused: the
/// population it judged, stated so a pass that judged nothing reads as that.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeparationBinding {
    /// Gates the world resolves (every gate anchor, open or shut at world-load).
    pub gates: usize,
    /// Barred edges the layout graph declares.
    pub barred_edges: usize,
    /// Of them, edges whose seam gate the world resolves.
    pub barred_bound: usize,
    /// Gates not judged because a declaration says a body gets round them: a
    /// `shortcut`'s door, or a barred edge declared `shortcut`.
    pub shortcuts: usize,
    /// Gates not judged because a clock owns the region (spec-0016 §4).
    pub clocked: usize,
    /// Gates not judged because the story never operates them: no `open-gate`
    /// or `close-gate` names the anchor and no barred edge bars it, so no beat
    /// of the story is kept behind it — a barred window, a door nothing opens.
    pub unoperated: usize,
    /// Gates whose shape names no two faces (no unique thin axis the faces
    /// decide), counted once each and not judged.
    pub faceless: usize,
    /// Distinct quest configurations judged.
    pub configurations: usize,
    /// `(gate, configuration)` pairs where the gate is shut.
    pub shut: usize,
    /// Of them, pairs where a face has no cell a body can stand or float in.
    pub unstood: usize,
    /// Of them, pairs where the party can get to a face of the shut gate.
    pub reached: usize,
    /// Of them, pairs where a body gets from the near face to the far one.
    pub bypassed: usize,
}

impl SeparationBinding {
    /// The one line a build prints about this proof.
    pub fn line(&self) -> String {
        format!(
            "DW1011 binding: {} gate(s) resolved ({} of {} barred edge(s) among them); {} declared \
             a way round (shortcut), {} clocked, {} operated by no beat, {} with no two faces; over \
             {} quest \
             configuration(s), {} (gate, configuration) pair(s) shut, {} with a face no body can \
             stand at, {} with the party at a face, {} got round by walking, falling, jumping or \
             swimming",
            self.gates,
            self.barred_bound,
            self.barred_edges,
            self.shortcuts,
            self.clocked,
            self.unoperated,
            self.faceless,
            self.configurations,
            self.shut,
            self.unstood,
            self.reached,
            self.bypassed,
        )
    }

    /// The ledger `validation/separation-proof.json` carries: the same counts.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_GATE_BYPASSED.id(),
            "gates": self.gates,
            "barred_edges": self.barred_edges,
            "barred_bound": self.barred_bound,
            "shortcuts": self.shortcuts,
            "clocked": self.clocked,
            "unoperated": self.unoperated,
            "faceless": self.faceless,
            "configurations": self.configurations,
            "shut": self.shut,
            "unstood": self.unstood,
            "reached": self.reached,
            "bypassed": self.bypassed,
        })
    }
}

/// One gate the proof judges: how a report names it, its region, and which of
/// its two faces opens it when a declaration says so.
#[derive(Debug, Clone)]
pub(super) struct GateUnderTest {
    /// The gate in words: the barred edge it bars, or its anchor and area.
    pub label: String,
    /// The region's inclusive corners.
    pub region: ([i32; 3], [i32; 3]),
    /// `Some(true)` when only the low face (of the thin axis) opens it,
    /// `Some(false)` when only the high face does, `None` when either does.
    pub opens_low: Option<bool>,
    /// Where the openable side's anchor stands, when the gate is one-sided —
    /// resolved to [`GateUnderTest::opens_low`] once the thin axis is known.
    pub unlock: Option<[i32; 3]>,
}

/// A gate by name and region, for an escape path to name the gates it passes.
pub(super) type NamedGate = (String, ([i32; 3], [i32; 3]));

/// One quest configuration the proof judges: its world, the configuration in
/// words, its route cells, and the gates shut in it (indices into the gates).
pub(super) type SeparationConfig<'w> = (&'w World, String, Vec<[i32; 3]>, Vec<usize>);

/// The two faces of a gate: the cells a body may stand or float in either side
/// of its thin axis, low first. Candidates — each configuration keeps those a
/// body can be in there.
#[derive(Debug, Clone)]
struct Faces {
    axis: usize,
    low: Vec<[i32; 3]>,
    high: Vec<[i32; 3]>,
}

/// Whether a body can be in `c`: standing, or afloat at the top of water.
fn can_be(w: &World, c: [i32; 3]) -> bool {
    w.is_standable(c) || w.is_water_surface(c)
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
/// `None` when its shape names no two: the thin axis is the unique smallest
/// extent, or — among axes tied for smallest — the one axis whose two faces
/// both hold a cell a body can be in.
fn faces_of(base: &World, region: ([i32; 3], [i32; 3])) -> Option<Faces> {
    let (a, b) = region;
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let extent = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let least = *extent.iter().min()?;
    let tied: Vec<usize> = (0..3).filter(|&i| extent[i] == least).collect();
    let faces = |axis: usize| Faces {
        axis,
        low: face_cells(lo, hi, axis, false),
        high: face_cells(lo, hi, axis, true),
    };
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

/// Whether a body in `c` is in water: afloat at its top or under it.
fn wet(w: &World, c: [i32; 3]) -> bool {
    matches!(
        w.rest_kind(c),
        Some(RestKind::Afloat | RestKind::Submerged)
    )
}

/// An escape path: the cells it passes, and the edge of the reach graph that
/// joins each to the next (`edges.len() + 1 == cells.len()`).
struct Path {
    cells: Vec<[i32; 3]>,
    edges: Vec<Edge>,
}

/// The path over `w` from any cell of `from` to any cell of `to` that takes the
/// fewest moves other than a walk, and of those the fewest moves — never
/// stepping inside `region` (the shut gate itself) — or `None`.
///
/// Fewest non-walk moves first, because the witness is read by an author who
/// must close the way round: a path that jumps where it could walk would name
/// jumps that are not the defect.
///
/// With `must_swim`, only a path that goes into the water somewhere counts as
/// arriving: the best way round by water, named in its own right beside a dry
/// one, because walling the dry way leaves it open.
fn escape(
    w: &World,
    reach: &Reach,
    from: &[[i32; 3]],
    to: &BTreeSet<[i32; 3]>,
    region: ([i32; 3], [i32; 3]),
    must_swim: bool,
) -> Option<Path> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    type State = ([i32; 3], bool);
    let (lo, hi) = region;
    let inside =
        |c: &[i32; 3]| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i]));
    let mut best: BTreeMap<State, (u32, u32)> = BTreeMap::new();
    let mut pred: BTreeMap<State, (State, Edge)> = BTreeMap::new();
    let mut heap: BinaryHeap<Reverse<(u32, u32, State)>> = BinaryHeap::new();
    for c in from {
        best.insert((*c, false), (0, 0));
        heap.push(Reverse((0, 0, (*c, false))));
    }
    while let Some(Reverse((spent, steps, state))) = heap.pop() {
        if best.get(&state).is_some_and(|b| *b < (spent, steps)) {
            continue;
        }
        let (cur, wet) = state;
        if to.contains(&cur) && (wet || !must_swim) {
            let mut cells = vec![cur];
            let mut edges = Vec::new();
            let mut at = state;
            while let Some((p, e)) = pred.get(&at) {
                cells.push(p.0);
                edges.push(e.clone());
                at = *p;
            }
            cells.reverse();
            edges.reverse();
            return Some(Path { cells, edges });
        }
        // Every edge of the graph out of `cur`, and — the open sea being one
        // place (spec-0113 §4.7) — a swim to every other cell of it.
        let mut next: Vec<Edge> = w.body_edges(cur);
        if reach.open_sea.contains(&cur) {
            next.extend(reach.open_sea.iter().map(|&c| Edge {
                to: c,
                kind: EdgeKind::Dive,
                yaw: None,
                sprint: false,
            }));
        }
        for e in next {
            let n = e.to;
            if e.kind == EdgeKind::Exit || inside(&n) {
                continue;
            }
            let walk = e.kind == EdgeKind::Walk && !self::wet(w, cur) && !self::wet(w, n);
            let touched = must_swim && (wet || self::wet(w, cur) || self::wet(w, n));
            let to_state = (n, touched);
            let cost = (spent + u32::from(!walk), steps + 1);
            if best.get(&to_state).is_none_or(|b| cost < *b) {
                best.insert(to_state, cost);
                pred.insert(to_state, (state, e));
                heap.push(Reverse((cost.0, cost.1, to_state)));
            }
        }
    }
    None
}

/// One direction a gate is judged in: the near face, the far face, and whether
/// the party reaches the near one.
type Side<'a> = (&'a Vec<[i32; 3]>, &'a Vec<[i32; 3]>, bool);

/// One gate got round in one configuration: its index, the way round in words,
/// and what a second search for a way round by water needs.
struct Bypass {
    gate: usize,
    words: String,
    near: Vec<[i32; 3]>,
    far: BTreeSet<[i32; 3]>,
    swims: bool,
}

/// The kind of one movement, in the words the obligation is stated in: any
/// edge into, out of or through water is swimming.
fn move_kind(w: &World, from: [i32; 3], edge: &Edge) -> &'static str {
    if wet(w, from) || wet(w, edge.to) {
        return "swimming";
    }
    match edge.kind {
        EdgeKind::Walk => "walking",
        EdgeKind::Fall => "falling",
        EdgeKind::Jump => "jumping",
        EdgeKind::Dive => "swimming",
        EdgeKind::Climb => "climbing",
        EdgeKind::Exit => "leaving the world",
    }
}

/// An escape path in words: its length, the move kinds it uses, where it leaves
/// walking and where it regains it, and a few cells along the way.
fn path_words(w: &World, route: &Path, others: &[NamedGate]) -> String {
    let path = &route.cells;
    let moves: Vec<([i32; 3], [i32; 3], &'static str, String)> = path
        .iter()
        .zip(&route.edges)
        .map(|(&from, e)| (from, e.to, move_kind(w, from, e), movement_words(w, from, e)))
        .collect();
    let mut kinds: Vec<(&'static str, usize)> = Vec::new();
    for (_, _, k, _) in &moves {
        let k = *k;
        match kinds.iter_mut().find(|(n, _)| *n == k) {
            Some((_, count)) => *count += 1,
            None => kinds.push((k, 1)),
        }
    }
    let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} x{n}")).collect();
    let off = moves.iter().find(|(_, _, k, _)| *k != "walking");
    let on = moves.iter().rev().find(|(_, _, k, _)| *k != "walking");
    let mut s = format!("{} move(s) ({})", moves.len(), kinds.join(", "));
    if let (Some((from, _, _, how_off)), Some((_, to, _, how_on))) = (off, on) {
        s.push_str(&format!(
            "; it leaves the walked ground at {from:?} by {how_off} and is back on it at {to:?} \
             by {how_on}"
        ));
    }
    let through: Vec<&str> = others
        .iter()
        .filter(|(_, (lo, hi))| {
            path.iter()
                .any(|c| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i])))
        })
        .map(|(name, _)| name.as_str())
        .collect();
    if !through.is_empty() {
        s.push_str(&format!("; it passes through {}", through.join(", ")));
    }
    if path.len() > 2 {
        let inner = &path[1..path.len() - 1];
        let step = (inner.len() / (WAYPOINTS + 1)).max(1);
        let via: Vec<String> = inner
            .iter()
            .step_by(step)
            .take(WAYPOINTS)
            .map(|c| format!("{c:?}"))
            .collect();
        s.push_str(&format!("; by way of {}", via.join(", ")));
    }
    s
}

/// [`DW_GATE_BYPASSED`] over a campaign's gates and barred edges, judged once
/// per quest configuration the critical path passes through. Returns the
/// binding beside the verdict so the caller can print it whichever way the
/// verdict went.
pub fn check_gates_separate(
    plan: &Plan,
    world: &World,
) -> (SeparationBinding, Result<(), Failure>) {
    let mut binding = SeparationBinding::default();
    let graph = plan.campaign.layout_graph.as_ref().map(|g| &g.content);
    // The barred edges, keyed by the gate anchor the derivation names for them.
    let mut barred: BTreeMap<String, &delvewright_dsl::layout::Edge> = BTreeMap::new();
    for e in graph.map(|g| g.edges.iter()).into_iter().flatten() {
        if matches!(e, delvewright_dsl::layout::Edge::Barred { .. }) {
            binding.barred_edges += 1;
            barred.insert(delvewright_dsl::siteplan::seam_anchor(e.id()), e);
        }
    }
    let shortcut_regions: BTreeSet<([i32; 3], [i32; 3])> =
        plan.shortcuts.iter().map(|s| s.gate_region).collect();
    let point = |name: &str| {
        plan.anchors.iter().find_map(|((_, n), r)| match r {
            crate::compiler::plan::ResolvedAnchor::Point { pos, .. } if n == name => Some(*pos),
            _ => None,
        })
    };
    let mut gates: Vec<GateUnderTest> = Vec::new();
    let mut bound_edges: BTreeSet<String> = BTreeSet::new();
    // Every gate by name, so an escape path through one — a clocked door, a
    // shortcut's — names it.
    let others: Vec<NamedGate> = world
        .every_gate()
        .map(|(seal, clocked)| {
            let kind = if clocked {
                "the clocked gate"
            } else if shortcut_regions.contains(&seal.region) {
                "the shortcut gate"
            } else {
                "the gate"
            };
            (format!("{kind} `{}`", seal.anchor), seal.region)
        })
        .collect();
    // The gates the story operates: an `open-gate` or `close-gate` anywhere in
    // the campaign names them (an unforced beat's included — the route model
    // drops an unforced opening, and a gate a side beat opens still keeps what
    // is behind it until then).
    let mut operated: BTreeSet<&str> = BTreeSet::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |_, eff| match &eff.verb {
        delvewright_dsl::Verb::OpenGate { anchor, .. }
        | delvewright_dsl::Verb::CloseGate { anchor, .. } => {
            operated.insert(anchor.as_str());
        }
        _ => {}
    });
    for (seal, clocked) in world.every_gate() {
        binding.gates += 1;
        let edge = barred.get(&seal.anchor).copied();
        if edge.is_some() {
            bound_edges.insert(seal.anchor.clone());
        }
        let declared_shortcut = matches!(
            edge,
            Some(delvewright_dsl::layout::Edge::Barred { shortcut: true, .. })
        );
        if shortcut_regions.contains(&seal.region) || declared_shortcut {
            binding.shortcuts += 1;
            continue;
        }
        if clocked {
            binding.clocked += 1;
            continue;
        }
        if edge.is_none() && !operated.contains(seal.anchor.as_str()) {
            binding.unoperated += 1;
            continue;
        }
        let (label, unlock) = match edge {
            Some(delvewright_dsl::layout::Edge::Barred {
                id,
                opens_from,
                gating,
                ..
            }) => {
                let mut terms: Vec<String> =
                    gating.flags.iter().map(|f| format!("`{f}`")).collect();
                if let Some(q) = &gating.quest {
                    terms.push(format!("`{q}` complete"));
                }
                let unlock = (!matches!(opens_from, delvewright_dsl::layout::OpensFrom::Either))
                    .then(|| point(&delvewright_dsl::siteplan::seam_unlock_anchor(id)))
                    .flatten();
                (
                    format!(
                        "barred edge `{id}` (gate `{}`, opened on {})",
                        seal.anchor,
                        terms.join(" and ")
                    ),
                    unlock,
                )
            }
            _ => (format!("gate `{}` of `{}`", seal.anchor, seal.area), None),
        };
        gates.push(GateUnderTest {
            label,
            region: seal.region,
            opens_low: None,
            unlock,
        });
    }
    binding.barred_bound = bound_edges.len();

    // One configuration per distinct region state the critical path passes
    // through, carrying the route cells of every leg that arrives under it —
    // the same configurations `DW0921` judges.
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut configs: Vec<(RegionState, usize, BTreeSet<[i32; 3]>)> = Vec::new();
    for (step, cells) in critical_route_cells(plan, world) {
        let st = world.region_state_at(&plan.region_events, step, &ancestor);
        match configs.iter_mut().find(|(s, _, _)| *s == st) {
            Some((_, first, seeds)) => {
                *first = (*first).min(step);
                seeds.extend(cells);
            }
            None => configs.push((st, step, cells.into_iter().collect())),
        }
    }
    let owned: Vec<Option<World>> = configs
        .iter()
        .map(|(st, _, _)| (!st.is_empty()).then(|| world.with_region_state(st)))
        .collect();
    let judged: Vec<SeparationConfig<'_>> = configs
        .iter()
        .zip(&owned)
        .map(|((st, first, seeds), w)| {
            let w = w.as_ref().unwrap_or(world);
            let when = plan
                .critical_path
                .get(*first)
                .and_then(|s| s.objective())
                .map(|o| format!("while `{o}` is next"))
                .unwrap_or_else(|| format!("from critical step {first}"));
            let shut: Vec<usize> = gates
                .iter()
                .enumerate()
                .filter(|(_, g)| {
                    crate::compiler::assembled::region_cells(g.region.0, g.region.1)
                        .all(|c| st.solid.contains(&c))
                })
                .map(|(i, _)| i)
                .collect();
            (w, when, seeds.iter().copied().collect(), shut)
        })
        .collect();
    let (core, verdict) = verify_gates_separate(&gates, &others, world, &judged);
    binding.faceless = core.faceless;
    binding.configurations = core.configurations;
    binding.shut = core.shut;
    binding.unstood = core.unstood;
    binding.reached = core.reached;
    binding.bypassed = core.bypassed;
    (binding, verdict)
}

/// The pure core of [`check_gates_separate`]: the gates, the world with every
/// gate open (`base`, where a gate's faces are found), and one
/// `(world, when, route cells, shut gates)` per quest configuration. Split out
/// so it is unit-testable over a synthetic [`World`] without a [`Plan`].
pub(super) fn verify_gates_separate(
    gates: &[GateUnderTest],
    others: &[NamedGate],
    base: &World,
    configs: &[SeparationConfig<'_>],
) -> (SeparationBinding, Result<(), Failure>) {
    let mut binding = SeparationBinding {
        configurations: configs.len(),
        ..SeparationBinding::default()
    };
    // Each gate's faces, once, over the world with every gate open; and which
    // face opens it, where a one-sided declaration says.
    let faces: Vec<Option<Faces>> = gates.iter().map(|g| faces_of(base, g.region)).collect();
    binding.faceless = faces.iter().filter(|f| f.is_none()).count();
    let opens_low: Vec<Option<bool>> = gates
        .iter()
        .zip(&faces)
        .map(|(g, f)| {
            g.opens_low.or_else(|| {
                let (f, u) = (f.as_ref()?, g.unlock?);
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
        })
        .collect();
    type Judged = (usize, usize, usize, Vec<Bypass>, Option<u64>, Reach);
    let judged: Vec<Judged> = crate::par::map(configs, |(w, when, seeds, shut)| {
        let seeds: Vec<[i32; 3]> = seeds.iter().copied().filter(|c| can_be(w, *c)).collect();
        let graph = w.body_reach(&seeds);
        if let Some(budget) = graph.over_budget {
            // A partial closure takes no verdict (spec-0113 §5.3).
            return (shut.len(), 0, 0, Vec::new(), Some(budget), graph);
        }
        let reach = graph.cells();
        let (mut unstood, mut reached) = (0usize, 0usize);
        let mut found: Vec<Bypass> = Vec::new();
        for &gi in shut {
            let Some(f) = faces[gi].as_ref() else {
                continue;
            };
            let low: Vec<[i32; 3]> = f.low.iter().copied().filter(|c| can_be(w, *c)).collect();
            let high: Vec<[i32; 3]> = f.high.iter().copied().filter(|c| can_be(w, *c)).collect();
            if low.is_empty() || high.is_empty() {
                unstood += 1;
                continue;
            }
            let low_in = low.iter().any(|c| reach.contains(c));
            let high_in = high.iter().any(|c| reach.contains(c));
            if !low_in && !high_in {
                continue;
            }
            reached += 1;
            // The near face: the one that opens it when a declaration says so,
            // else each face the party reaches. A body reaching the far face
            // only across the gate's own region is what the gate is for.
            let sides: Vec<Side<'_>> = match opens_low[gi] {
                Some(true) => vec![(&low, &high, low_in)],
                Some(false) => vec![(&high, &low, high_in)],
                None => vec![(&low, &high, low_in), (&high, &low, high_in)],
            };
            for (near, far, near_in) in sides {
                if !near_in || !far.iter().any(|c| reach.contains(c)) {
                    // The near face's closure lies inside the party's reach, so
                    // a far face outside it is one no body gets to from here.
                    continue;
                }
                let far_set: BTreeSet<[i32; 3]> = far.iter().copied().collect();
                if let Some(path) = escape(w, &graph, near, &far_set, gates[gi].region, false) {
                    let start = path.cells[0];
                    let end = *path.cells.last().unwrap_or(&start);
                    let words = format!(
                        "{} is shut {when}, and a body at its near face {start:?} gets to its far \
                         face {end:?} without passing it: {}",
                        gates[gi].label,
                        path_words(w, &path, others)
                    );
                    let swims = path.cells.iter().any(|c| wet(w, *c));
                    found.push(Bypass {
                        gate: gi,
                        words,
                        near: near.clone(),
                        far: far_set,
                        swims,
                    });
                    break;
                }
            }
        }
        (shut.len(), unstood, reached, found, None, graph)
    });
    // Each bypassed gate is named once, at the first configuration that finds
    // it, with the number of configurations it is got round in.
    let mut first: BTreeMap<usize, (usize, Bypass, usize)> = BTreeMap::new();
    let mut order: Vec<usize> = Vec::new();
    let mut over_budget: Option<u64> = None;
    let mut graphs: ReachBinding = ReachBinding::default();
    // A configuration's closure is kept only where a gate is got round in it:
    // the search for a way round by water reads its open sea.
    let mut closures: Vec<Option<Reach>> = Vec::with_capacity(judged.len());
    for (ci, (shut, unstood, reached, found, over, graph)) in judged.into_iter().enumerate() {
        over_budget = over_budget.or(over);
        graphs.add(configs[ci].0, &graph);
        closures.push((!found.is_empty()).then_some(graph));
        binding.shut += shut;
        binding.unstood += unstood;
        binding.reached += reached;
        binding.bypassed += found.len();
        for b in found {
            match first.get_mut(&b.gate) {
                Some((_, _, n)) => *n += 1,
                None => {
                    order.push(b.gate);
                    first.insert(b.gate, (ci, b, 1));
                }
            }
        }
    }
    if let Some(budget) = over_budget {
        return (binding, Err(reach_budget_failure(&graphs, budget)));
    }
    if order.is_empty() {
        return (binding, Ok(()));
    }
    let listed: Vec<&(usize, Bypass, usize)> = order
        .iter()
        .take(BYPASS_LIST_LIMIT)
        .map(|gi| &first[gi])
        .collect();
    // A way round by water is named in its own right beside a dry one, because
    // walling the dry way leaves it open. Searched once per named gate, in the
    // first configuration that gets round it.
    let named: Vec<String> = crate::par::map(&listed, |(ci, b, n)| {
        let w = configs[*ci].0;
        let mut words = b.words.clone();
        if !b.swims
            && let Some(wet) = escape(
                w,
                closures[*ci].as_ref().expect("a configuration that got round a gate keeps its closure"),
                &b.near,
                &b.far,
                gates[b.gate].region,
                true,
            )
        {
            words.push_str(&format!(
                "; and by water, from {:?} to {:?}: {}",
                wet.cells[0],
                wet.cells.last().unwrap_or(&wet.cells[0]),
                path_words(w, &wet, others)
            ));
        }
        if *n > 1 {
            format!("{words} (got round in {n} configuration(s))")
        } else {
            words
        }
    });
    let more = order.len().saturating_sub(named.len());
    let tail = if more > 0 {
        format!("; and {more} more gate(s)")
    } else {
        String::new()
    };
    let verdict = Err(Failure {
        code: DW_GATE_BYPASSED,
        message: format!(
            "{} shut gate(s) do not separate their two sides — a body gets round each by moves a \
             player can make, and reaches what the story keeps shut out of order: {}{tail}. Close \
             the way round in blocks: wall the water off where the body goes into it, or raise the \
             edge it climbs, jumps or drops over, until no walk, fall, jump or swim leads from the \
             near face to the far one while the gate is shut. Do not fence walkable-looking ground \
             with an invisible barrier or mark it unwalkable. A gate the party is meant to get \
             round is declared so: a `shortcut`, or a one-sided barred way whose openable side is \
             the far one.",
            order.len(),
            named.join("; ")
        ),
    });
    (binding, verdict)
}

/// `DW1011` over synthetic worlds: a swim, a jump and a fall round a shut door,
/// each red, and each walled off green.
#[cfg(test)]
mod separate_tests {
    use super::*;

    /// Two yards either side of a wall at z = 10, joined only by a door one wide
    /// and two high at x = 6 (the gate region `[6,1,10]..[6,2,10]`). Feet at y=1
    /// over a stone floor at y=0, x in 0..14, z in 0..21. `shut` fills the door.
    fn walled_yards(shut: bool) -> (BTreeSet<[i32; 3]>, World) {
        let mut solid = BTreeSet::new();
        for x in 0..14 {
            for z in 0..21 {
                solid.insert([x, 0, z]);
            }
            for y in 1..=4 {
                if x != 6 || y > 2 || shut {
                    solid.insert([x, y, 10]);
                }
            }
        }
        // The yards are ringed by a wall four high, so nothing leaves them.
        for x in -1..=14 {
            for y in 1..=4 {
                solid.insert([x, y, -1]);
                solid.insert([x, y, 21]);
            }
        }
        for z in -1..=21 {
            for y in 1..=4 {
                solid.insert([-1, y, z]);
                solid.insert([14, y, z]);
            }
        }
        let w = World::from_solid_cells(solid.clone());
        (solid, w)
    }

    fn door() -> GateUnderTest {
        GateUnderTest {
            label: "barred edge `edge/yard-door`".to_string(),
            region: ([6, 1, 10], [6, 2, 10]),
            opens_low: None,
            unlock: None,
        }
    }

    fn judge(base: &World, shut: &World) -> (SeparationBinding, Result<(), Failure>) {
        verify_gates_separate(
            &[door()],
            &[],
            base,
            &[(
                shut,
                "while `obj/test` is next".to_string(),
                vec![[2, 1, 2]],
                vec![0],
            )],
        )
    }

    #[test]
    fn a_shut_door_in_a_whole_wall_separates_its_yards() {
        let (_, base) = walled_yards(false);
        let (_, shut) = walled_yards(true);
        let (b, verdict) = judge(&base, &shut);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(b.shut, 1);
        assert_eq!(b.reached, 1, "the party stands at the near face");
        assert_eq!(b.bypassed, 0);
        assert_eq!(b.faceless, 0, "a door one wide in a wall has two faces");
    }

    /// The wall stops at the sea: a channel of water two deep runs through the
    /// wall's east end (x = 11..12), so a body wades in on one side, swims
    /// through the gap and climbs out on the other. Walled (`walled`), the wall
    /// stands in the channel from its bed to full height.
    fn sea_round(walled: bool) -> (World, World) {
        let build = |shut: bool| {
            let (mut solid, _) = walled_yards(shut);
            let mut water = BTreeSet::new();
            for z in 4..=16 {
                for x in 11..=12 {
                    solid.remove(&[x, 0, z]);
                    solid.insert([x, -2, z]);
                    if z == 10 && walled {
                        for y in -1..=4 {
                            solid.insert([x, y, z]);
                        }
                        continue;
                    }
                    if z == 10 {
                        for y in 1..=4 {
                            solid.remove(&[x, y, z]);
                        }
                    }
                    water.insert([x, -1, z]);
                    water.insert([x, 0, z]);
                }
            }
            World::from_solid_and_flooded(solid, water)
        };
        (build(false), build(true))
    }

    #[test]
    fn a_body_swims_round_a_shut_door_dw1011() {
        let (base, shut) = sea_round(false);
        let (b, verdict) = judge(&base, &shut);
        let err = verdict.expect_err("the channel under the wall is a way round");
        assert_eq!(err.code.id(), "DW1011");
        assert!(err.message.contains("edge/yard-door"), "{}", err.message);
        assert!(err.message.contains("swimming x"), "{}", err.message);
        assert!(
            err.message.contains("leaves the walked ground"),
            "{}",
            err.message
        );
        assert_eq!(b.bypassed, 1);
    }

    #[test]
    fn walling_the_water_closes_the_swim() {
        let (base, shut) = sea_round(true);
        let (b, verdict) = judge(&base, &shut);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!((b.reached, b.bypassed), (1, 0));
    }

    /// The wall is two courses too low at its west end (x = 0): a crate one high
    /// a cell back from it is a step up, and from the crate a body jumps the gap
    /// onto the wall's top and drops down the far side — one way, since nothing
    /// on the far side stands high enough to jump back. `raised` brings the
    /// wall's west end to full height.
    fn over_the_top(raised: bool) -> (World, World) {
        let build = |shut: bool| {
            let (mut solid, _) = walled_yards(shut);
            if !raised {
                for y in 3..=4 {
                    solid.remove(&[0, y, 10]);
                }
                solid.insert([0, 1, 8]);
            }
            World::from_solid_cells(solid)
        };
        (build(false), build(true))
    }

    #[test]
    fn a_body_jumps_onto_a_low_wall_and_drops_past_a_shut_door_dw1011() {
        let (base, shut) = over_the_top(false);
        let (_, verdict) = judge(&base, &shut);
        let err = verdict.expect_err("the low wall end is a way round");
        assert_eq!(err.code.id(), "DW1011");
        assert!(err.message.contains("jumping x1"), "{}", err.message);
        assert!(err.message.contains("falling"), "{}", err.message);
    }

    #[test]
    fn raising_the_wall_closes_the_way_over() {
        let (base, shut) = over_the_top(true);
        let (_, verdict) = judge(&base, &shut);
        assert!(verdict.is_ok(), "{verdict:?}");
    }

    /// A way over the low wall and a channel through the wall's east end, both:
    /// the witness is the dry way (fewer moves that are not walks), and the
    /// swim is named beside it, because it is a way round in its own right.
    #[test]
    fn a_swim_beside_a_dry_way_round_is_named_too_dw1011() {
        let build = |shut: bool| {
            let (mut solid, _) = walled_yards(shut);
            for y in 3..=4 {
                solid.remove(&[0, y, 10]);
            }
            solid.insert([0, 1, 8]);
            let mut water = BTreeSet::new();
            for z in 4..=16 {
                for x in 11..=12 {
                    solid.remove(&[x, 0, z]);
                    solid.insert([x, -2, z]);
                    if z == 10 {
                        for y in 1..=4 {
                            solid.remove(&[x, y, z]);
                        }
                    }
                    water.insert([x, -1, z]);
                    water.insert([x, 0, z]);
                }
            }
            World::from_solid_and_flooded(solid, water)
        };
        let (base, shut) = (build(false), build(true));
        let (_, verdict) = judge(&base, &shut);
        let err = verdict.expect_err("two ways round");
        assert_eq!(err.code.id(), "DW1011");
        assert!(err.message.contains("jumping x1"), "{}", err.message);
        assert!(
            err.message.contains("; and by water, from"),
            "{}",
            err.message
        );
    }

    /// A one-sided gate is judged from its openable face only. The way over the
    /// low wall runs one way, from the low yard to the high one: a gate opened
    /// from the low face is got round, and one opened from the high face is not
    /// (nothing leads from the high face back), while a gate either face opens
    /// is got round from the low one.
    #[test]
    fn a_one_sided_gate_is_judged_from_its_openable_face() {
        let (base, shut) = over_the_top(false);
        let judge_with = |unlock: Option<[i32; 3]>| {
            verify_gates_separate(
                &[GateUnderTest { unlock, ..door() }],
                &[],
                &base,
                &[(
                    &shut,
                    "while `obj/test` is next".to_string(),
                    vec![[2, 1, 2]],
                    vec![0],
                )],
            )
        };
        let (_, low_opens) = judge_with(Some([6, 1, 8]));
        assert_eq!(
            low_opens.expect_err("opened from the near yard").code.id(),
            "DW1011"
        );
        let (b, high_opens) = judge_with(Some([6, 1, 12]));
        assert!(high_opens.is_ok(), "{high_opens:?}");
        assert_eq!(
            b.reached, 1,
            "the party reaches the high face over the wall"
        );
        let (_, either) = judge_with(None);
        assert_eq!(
            either.expect_err("either face opens it").code.id(),
            "DW1011"
        );
    }

    /// A gate nobody reaches in a configuration is not judged there; a gate
    /// that is not shut there is not judged at all.
    #[test]
    fn an_open_or_unreached_gate_is_not_judged() {
        let (base, shut) = sea_round(false);
        let (b, verdict) = verify_gates_separate(
            &[door()],
            &[],
            &base,
            &[(&shut, "open".to_string(), vec![[2, 1, 2]], vec![])],
        );
        assert!(verdict.is_ok());
        assert_eq!((b.shut, b.reached), (0, 0));
        let (b, verdict) = verify_gates_separate(
            &[door()],
            &[],
            &base,
            &[(&shut, "nobody".to_string(), vec![], vec![0])],
        );
        assert!(verdict.is_ok());
        assert_eq!((b.shut, b.reached), (1, 0));
    }

    #[test]
    fn a_cube_gate_with_faces_on_two_axes_names_no_two_faces() {
        // A 1x1 column in the open yard: walkable round on both axes.
        let mut solid = BTreeSet::new();
        for x in 0..10 {
            for z in 0..10 {
                solid.insert([x, 0, z]);
            }
        }
        let base = World::from_solid_cells(solid);
        assert!(faces_of(&base, ([5, 1, 5], [5, 2, 5])).is_none());
    }
}
