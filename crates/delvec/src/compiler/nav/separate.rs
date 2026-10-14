//! The separation relation: while a gate is shut, its far side cannot be
//! reached from its near side by any move a body can make (`DW1011`).
//!
//! The route proofs walk; this floods with every body move — walking, falling,
//! jumping and swimming ([`World::body_moves`]) — because the question is not
//! where the bot goes on cue but where a player can get to. It is the dual of
//! `DW0921`'s question over the same relation: that one asks whether a body can
//! get back, this one whether a body can get past.

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
             a way round (shortcut), {} clocked, {} with no two faces; over {} quest \
             configuration(s), {} (gate, configuration) pair(s) shut, {} with a face no body can \
             stand at, {} with the party at a face, {} got round by walking, falling, jumping or \
             swimming",
            self.gates,
            self.barred_bound,
            self.barred_edges,
            self.shortcuts,
            self.clocked,
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

/// The body-movement closure of `roots` over `w`.
fn closure(w: &World, roots: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
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

/// The fewest-moves path over `w` from any cell of `from` to any cell of `to`,
/// never stepping inside `region` (the shut gate itself), or `None`.
fn escape(
    w: &World,
    from: &[[i32; 3]],
    to: &BTreeSet<[i32; 3]>,
    region: ([i32; 3], [i32; 3]),
) -> Option<Vec<[i32; 3]>> {
    let (lo, hi) = region;
    let inside =
        |c: &[i32; 3]| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i]));
    let mut pred: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
    let mut seen: BTreeSet<[i32; 3]> = from.iter().copied().collect();
    let mut queue: VecDeque<[i32; 3]> = from.iter().copied().collect();
    while let Some(cur) = queue.pop_front() {
        if to.contains(&cur) {
            let mut path = vec![cur];
            let mut at = cur;
            while let Some(p) = pred.get(&at) {
                path.push(*p);
                at = *p;
            }
            path.reverse();
            return Some(path);
        }
        for n in w.body_moves(cur) {
            if !inside(&n) && seen.insert(n) {
                pred.insert(n, cur);
                queue.push_back(n);
            }
        }
    }
    None
}

/// The kind of one movement, in the four words the obligation is stated in.
fn move_kind(words: &str) -> &'static str {
    if words.contains("water") || words == "swimming" {
        "swimming"
    } else if words.starts_with("a fall") {
        "falling"
    } else if words.starts_with("a jump") {
        "jumping"
    } else {
        "walking"
    }
}

/// An escape path in words: its length, the move kinds it uses, where it leaves
/// walking and where it regains it, and a few cells along the way.
fn path_words(w: &World, path: &[[i32; 3]]) -> String {
    let moves: Vec<([i32; 3], [i32; 3], String)> = path
        .windows(2)
        .map(|p| (p[0], p[1], movement_words(w, p[0], p[1])))
        .collect();
    let mut kinds: Vec<(&'static str, usize)> = Vec::new();
    for (_, _, words) in &moves {
        let k = move_kind(words);
        match kinds.iter_mut().find(|(n, _)| *n == k) {
            Some((_, count)) => *count += 1,
            None => kinds.push((k, 1)),
        }
    }
    let kinds: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} x{n}")).collect();
    let off = moves.iter().find(|(_, _, m)| move_kind(m) != "walking");
    let on = moves
        .iter()
        .rev()
        .find(|(_, _, m)| move_kind(m) != "walking");
    let mut s = format!("{} move(s) ({})", moves.len(), kinds.join(", "));
    if let (Some((from, _, how_off)), Some((_, to, how_on))) = (off, on) {
        s.push_str(&format!(
            "; it leaves the walked ground at {from:?} by {how_off} and is back on it at {to:?} \
             by {how_on}"
        ));
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
    let (core, verdict) = verify_gates_separate(&gates, world, &judged);
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
    type Judged = (usize, usize, usize, Vec<(usize, String)>);
    let judged: Vec<Judged> = crate::par::map(configs, |(w, when, seeds, shut)| {
        let seeds: Vec<[i32; 3]> = seeds.iter().copied().filter(|c| can_be(w, *c)).collect();
        let reach = closure(w, &seeds);
        let (mut unstood, mut reached) = (0usize, 0usize);
        let mut found: Vec<(usize, String)> = Vec::new();
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
            let sides: Vec<(&Vec<[i32; 3]>, &Vec<[i32; 3]>, bool)> = match opens_low[gi] {
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
                if let Some(path) = escape(w, near, &far_set, gates[gi].region) {
                    let start = path[0];
                    let end = *path.last().unwrap_or(&start);
                    found.push((
                        gi,
                        format!(
                            "{} is shut {when}, and a body at its near face {start:?} gets to its \
                             far face {end:?} without passing it: {}",
                            gates[gi].label,
                            path_words(w, &path)
                        ),
                    ));
                    break;
                }
            }
        }
        (shut.len(), unstood, reached, found)
    });
    // Each bypassed gate is named once, at the first configuration that finds
    // it, with the number of configurations it is got round in.
    let mut first: BTreeMap<usize, (String, usize)> = BTreeMap::new();
    let mut order: Vec<usize> = Vec::new();
    for (shut, unstood, reached, found) in judged {
        binding.shut += shut;
        binding.unstood += unstood;
        binding.reached += reached;
        binding.bypassed += found.len();
        for (gi, words) in found {
            match first.get_mut(&gi) {
                Some((_, n)) => *n += 1,
                None => {
                    first.insert(gi, (words, 1));
                    order.push(gi);
                }
            }
        }
    }
    if order.is_empty() {
        return (binding, Ok(()));
    }
    let named: Vec<String> = order
        .iter()
        .take(BYPASS_LIST_LIMIT)
        .map(|gi| {
            let (words, n) = &first[gi];
            if *n > 1 {
                format!("{words} (got round in {n} configuration(s))")
            } else {
                words.clone()
            }
        })
        .collect();
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
        assert!(
            err.message.contains("jumping") || err.message.contains("walking"),
            "{}",
            err.message
        );
        assert!(err.message.contains("falling"), "{}", err.message);
    }

    #[test]
    fn raising_the_wall_closes_the_way_over() {
        let (base, shut) = over_the_top(true);
        let (_, verdict) = judge(&base, &shut);
        assert!(verdict.is_ok(), "{verdict:?}");
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
            &base,
            &[(&shut, "open".to_string(), vec![[2, 1, 2]], vec![])],
        );
        assert!(verdict.is_ok());
        assert_eq!((b.shut, b.reached), (0, 0));
        let (b, verdict) = verify_gates_separate(
            &[door()],
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
