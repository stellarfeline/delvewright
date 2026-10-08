//! The leave relation: a place a body can get into and not out of (`DW0921`),
//! judged over a campaign's route and over a sculpted piece.

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use std::collections::{BTreeMap, BTreeSet};

/// How many pockets a `DW0921` report names before summarising the rest.
const POCKET_LIST_LIMIT: usize = 6;

/// What [`check_bodies_can_leave`] measured, whether or not it refused: the
/// population it judged, stated so a pass that judged nothing reads as that.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeaveBinding {
    /// Distinct quest configurations judged.
    pub configurations: usize,
    /// Route cells the judgement was rooted at, summed over configurations.
    pub route_cells: usize,
    /// Cells a body can reach from them, summed over configurations.
    pub reached: usize,
    /// Of those, the cells where the body is afloat at the top of water — the
    /// population the water half of the proof judges.
    pub afloat: usize,
    /// Of those, cells a body cannot leave.
    pub trapped: usize,
    /// Reached cells that are stand cells of a link live in their
    /// configuration — the third way out of a pocket (spec-0083 §3.9), counted
    /// over configurations.
    pub link_exits: usize,
}

impl LeaveBinding {
    /// The one line a build prints about this proof.
    pub fn line(&self) -> String {
        format!(
            "DW0921 binding: {} quest configuration(s), {} route cell(s), {} cell(s) a body can reach \
             by walking, falling, jumping or swimming ({} of them afloat), {} it cannot leave; {} \
             link stand cell(s) served as a way out",
            self.configurations,
            self.route_cells,
            self.reached,
            self.afloat,
            self.trapped,
            self.link_exits
        )
    }

    /// The ledger `validation/leave-proof.json` carries: the same counts, so a
    /// reader (and the staging gate) can see what the proof judged.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_BODY_CANNOT_LEAVE.id(),
            "configurations": self.configurations,
            "route_cells": self.route_cells,
            "reached": self.reached,
            "afloat": self.afloat,
            "trapped": self.trapped,
        })
    }
}

/// How a body got from `from` to `to` in one [`World::body_moves`] movement, in
/// words for a report.
fn movement_words(world: &World, from: [i32; 3], to: [i32; 3]) -> String {
    let run = (to[0] - from[0]).abs() + (to[2] - from[2]).abs();
    let rise = to[1] - from[1];
    if world.is_water_surface(from) {
        return if world.is_water_surface(to) {
            "swimming".to_string()
        } else {
            "climbing out of the water".to_string()
        };
    }
    if world.is_water_surface(to) {
        return if run == 1 && rise >= 0 {
            "wading into water".to_string()
        } else if run == 1 {
            format!("a fall of {} block(s) into water", -rise)
        } else {
            format!("a jump across {} column(s) into water", run - 1)
        };
    }
    if world.neighbors(from).contains(&to) {
        "a walk".to_string()
    } else if run == 1 {
        format!("a fall of {} block(s)", -rise)
    } else {
        let height = match rise.cmp(&0) {
            std::cmp::Ordering::Greater => format!(" up {rise}"),
            std::cmp::Ordering::Less => format!(" down {}", -rise),
            std::cmp::Ordering::Equal => String::new(),
        };
        format!("a jump across {} column(s){height}", run - 1)
    }
}

/// [`DW_BODY_CANNOT_LEAVE`] over a campaign's critical path. Returns the binding
/// beside the verdict so the caller can print it whichever way the verdict went.
///
/// `returned` is the campaign's playable region (`boundary`, spec-0013) as its
/// inclusive `(min, max)` corners, or `None` when it declares none. A body
/// standing outside it is carried back to the last checkpoint by the boundary
/// clock, so a cell outside it is a way out like a route cell is.
pub fn check_bodies_can_leave(
    plan: &Plan,
    world: &World,
    returned: Option<([i32; 3], [i32; 3])>,
) -> (LeaveBinding, Result<(), Failure>) {
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    // One configuration per distinct region state, carrying the route cells of
    // every leg that arrives under it and the first step that does.
    // Each configuration also carries the links live at any step that arrives
    // under it: a link's stand cell is a way out of a pocket there (spec-0083
    // §3.9), because a body standing in it performs the trigger and is put down
    // on a route cell — and the link is repeatable by construction.
    let mut configs: Vec<LeaveConfigSeed> = Vec::new();
    for (step, cells) in critical_route_cells(plan, world) {
        let st = world.region_state_at(&plan.region_events, step, &ancestor);
        let live: BTreeSet<usize> = plan
            .critical_path_live_links
            .get(step)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        match configs.iter_mut().find(|(s, _, _, _)| *s == st) {
            Some((_, first, seeds, links)) => {
                *first = (*first).min(step);
                seeds.extend(cells);
                links.extend(live);
            }
            None => configs.push((st, step, cells.into_iter().collect(), live)),
        }
    }
    let owned: Vec<Option<World>> = configs
        .iter()
        .map(|(st, _, _, _)| (!st.is_empty()).then(|| world.with_region_state(st)))
        .collect();
    let worlds: Vec<LeaveConfig<'_>> = configs
        .into_iter()
        .zip(&owned)
        .map(|((st, first, seeds, live), w)| {
            let w = w.as_ref().unwrap_or(world);
            let exits: BTreeSet<[i32; 3]> = live
                .iter()
                .filter_map(|li| plan.links.get(*li))
                .flat_map(|l| stand_cells(w, l))
                .collect();
            let mut when = plan
                .critical_path
                .get(first)
                .and_then(|s| s.objective())
                .map(|o| format!("while `{o}` is next"))
                .unwrap_or_else(|| format!("from critical step {first}"));
            // spec-0086 §5.3: the configuration names every loop's slab as it
            // has it — a holding slab is a wall a body is returned from.
            for l in &plan.loops {
                let held = st.held_regions.iter().any(|(r, _)| *r == l.slab);
                when.push_str(&format!(
                    ", with the slab of loop `{}` {}",
                    l.id,
                    if held { "holding" } else { "clear" }
                ));
            }
            // spec-0088: a pocket that exists only while a staged volume is dead
            // (or only while it is live) is named with the volume's state.
            let staged: Vec<String> = world
                .staged_volumes()
                .iter()
                .zip(world.staged_liveness(&plan.region_events, first, &ancestor))
                .map(|(v, l)| {
                    let state = if l.may { "may be live" } else { "dead" };
                    format!("lethal volume `{}` {state}", v.id)
                })
                .collect();
            let when = if staged.is_empty() {
                when
            } else {
                format!("{when}, {}", staged.join(", "))
            };
            (w, when, seeds.into_iter().collect(), exits)
        })
        .collect();
    verify_bodies_can_leave(&worlds, returned, &plan.shortcuts)
}

/// One configuration as it is gathered: its region state, the first step that
/// arrives under it, its route cells, and the links live at those steps.
type LeaveConfigSeed = (RegionState, usize, BTreeSet<[i32; 3]>, BTreeSet<usize>);

/// One quest configuration `DW0921` judges: its world, the configuration in
/// words, its route cells, and the stand cells of the links live in it.
type LeaveConfig<'w> = (&'w World, String, Vec<[i32; 3]>, BTreeSet<[i32; 3]>);

/// The pure core of [`check_bodies_can_leave`]: one `(world, when, route cells)`
/// per quest configuration, where `when` names the configuration in words. Split
/// out so it is unit-testable over a synthetic [`World`] without a [`Plan`].
fn verify_bodies_can_leave(
    worlds: &[LeaveConfig<'_>],
    returned: Option<([i32; 3], [i32; 3])>,
    shortcuts: &[crate::compiler::plan::ShortcutPlan],
) -> (LeaveBinding, Result<(), Failure>) {
    let mut binding = LeaveBinding {
        configurations: worlds.len(),
        ..LeaveBinding::default()
    };
    // Each configuration is judged over its own world alone, so they are
    // judged in parallel and folded below in configuration order.
    let judged = crate::par::map(worlds, |(w, when, seeds, exits)| {
        let seeds: Vec<[i32; 3]> = seeds
            .iter()
            .copied()
            .filter(|c| w.is_standable(*c))
            .collect();
        let seed_set: BTreeSet<[i32; 3]> = seeds.iter().copied().collect();
        // The ways out: the route itself and every reached stand cell of a live
        // link (spec-0083 §3.9), through the one leave relation (spec-0087).
        let ways_out: BTreeSet<[i32; 3]> = seed_set.union(exits).copied().collect();
        let judge = |ways: &BTreeSet<[i32; 3]>| w.trapped_places(&seeds, ways, returned);
        let judged = judge(&ways_out);
        // A stand cell SERVES as a way out where, without the links, a body
        // standing in it could not get back — counted against the same closure
        // judged with no link at all, and only when this configuration has one.
        let link_exits = if exits.is_empty() {
            0
        } else {
            let without = judge(&seed_set);
            without
                .pockets
                .iter()
                .flatten()
                .filter(|c| exits.contains(*c))
                .count()
        };
        let afloat = judged
            .reached
            .iter()
            .filter(|c| w.is_water_surface(**c))
            .count();
        // A shortcut is opened from its far side by whoever stands at its lever,
        // and the completability model holds it shut. A pocket whose own reach
        // takes a body to a lever, and through the door that lever opens back to
        // the route, is not a pocket.
        let kept: Vec<Vec<[i32; 3]>> = judged
            .pockets
            .iter()
            .filter(|p| !w.leaves_by_a_shortcut(p, &seed_set, returned, shortcuts))
            .cloned()
            .collect();
        let trapped: BTreeSet<[i32; 3]> = kept.iter().flatten().copied().collect();
        let reached = &judged.reached;
        let described: Vec<String> = kept
            .iter()
            .map(|pocket| {
                let how = w.way_in_words(pocket, &trapped, &judged.preds);
                format!(
                    "{} cell(s) around {:?} ({when}): {how}, and no walk, fall, jump or swim \
                     leads from any of them back to the route",
                    pocket.len(),
                    pocket[0]
                )
            })
            .collect();
        (
            seeds.len(),
            reached.len(),
            afloat,
            trapped.len(),
            described,
            link_exits,
        )
    });
    let mut pockets: Vec<String> = Vec::new();
    let mut pocket_count = 0usize;
    for (route_cells, reached, afloat, trapped, described, link_exits) in judged {
        binding.route_cells += route_cells;
        binding.link_exits += link_exits;
        binding.reached += reached;
        binding.afloat += afloat;
        binding.trapped += trapped;
        for pocket in described {
            pocket_count += 1;
            if pockets.len() < POCKET_LIST_LIMIT {
                pockets.push(pocket);
            }
        }
    }
    if pockets.is_empty() {
        return (binding, Ok(()));
    }
    let more = pocket_count.saturating_sub(pockets.len());
    let tail = if more > 0 {
        format!("; and {more} more")
    } else {
        String::new()
    };
    let verdict = Err(Failure {
        code: DW_BODY_CANNOT_LEAVE,
        message: format!(
            "{pocket_count} place(s) a body can get into and not out of — the player is \
             soft-locked there: {}{tail}. Reshape the place so whoever gets in can walk, fall, \
             jump or climb out of the water (lower the wall they are ringed by, give them a step, \
             or take away what they jumped in from); do not fence walkable-looking ground with an \
             invisible barrier. A body afloat climbs out only onto ground one cell above the \
             water's top, and does not dive. A room the story is meant to shut the party into \
             holds the objective the story waits on, and is not this. Moves are cardinal: a \
             diagonal jump or a climb (ladder, vine) is not counted, so a place left only that way \
             reads as a trap.",
            pockets.join("; ")
        ),
    });
    (binding, verdict)
}

/// Whether the boundary clock carries a body standing in `c` back to the last
/// checkpoint: the body can stand clear of the region's box when the cell lies
/// wholly outside it horizontally, or far enough under its floor that the head
/// is below it.
pub(crate) fn returned_from(returned: Option<([i32; 3], [i32; 3])>, c: [i32; 3]) -> bool {
    returned.is_some_and(|(lo, hi)| {
        c[0] < lo[0] || c[0] > hi[0] || c[2] < lo[2] || c[2] > hi[2] || c[1] + 2 < lo[1]
    })
}

/// The cells of `trapped`, split into pockets that touch (26-neighbourhood),
/// each sorted, in the order of their least cell.
fn pockets_of(trapped: &BTreeSet<[i32; 3]>) -> Vec<Vec<[i32; 3]>> {
    let mut left = trapped.clone();
    let mut out = Vec::new();
    while let Some(start) = left.pop_first() {
        let mut pocket = vec![start];
        let mut queue = vec![start];
        while let Some(c) = queue.pop() {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                        if left.remove(&n) {
                            pocket.push(n);
                            queue.push(n);
                        }
                    }
                }
            }
        }
        pocket.sort_unstable();
        out.push(pocket);
    }
    out
}

/// **What a body can get into and not out of** over one world — the closure a
/// body reaches from `roots` by [`World::body_moves`], and within it the places
/// from which no movement sequence reaches a way out. The one leave relation:
/// `DW0921` judges a campaign's route with it ([`check_bodies_can_leave`]) and
/// `delvec sculpt` judges a sculpted piece with it (spec-0087 §3.4), so the two
/// cannot disagree about what a pocket is.
#[derive(Debug, Clone, Default)]
pub struct TrappedPlaces {
    /// Every cell a body can stand or float in from the roots, roots included.
    pub reached: BTreeSet<[i32; 3]>,
    /// The cells of `reached` it cannot leave, split into touching places
    /// (26-neighbourhood), each sorted, in the order of their least cell.
    pub pockets: Vec<Vec<[i32; 3]>>,
    /// Every reached cell's predecessors, so a report can say how a body got in.
    pub preds: BTreeMap<[i32; 3], Vec<[i32; 3]>>,
}

impl World {
    /// [`TrappedPlaces`] from `roots`, where reaching any cell of `ways_out` —
    /// or a cell the boundary clock carries a body back from (`returned`, the
    /// inclusive corners of the playable box) — counts as having left.
    pub fn trapped_places(
        &self,
        roots: &[[i32; 3]],
        ways_out: &BTreeSet<[i32; 3]>,
        returned: Option<([i32; 3], [i32; 3])>,
    ) -> TrappedPlaces {
        let (reached, trapped, preds) = self.cells_a_body_cannot_leave(roots, ways_out, returned);
        TrappedPlaces {
            reached,
            pockets: pockets_of(&trapped),
            preds,
        }
    }

    /// How a body first gets into `pocket` from outside `trapped`, in words for
    /// a report: `a body gets in from [..] to [..] by a fall of 3 block(s)`.
    pub fn way_in_words(
        &self,
        pocket: &[[i32; 3]],
        trapped: &BTreeSet<[i32; 3]>,
        preds: &BTreeMap<[i32; 3], Vec<[i32; 3]>>,
    ) -> String {
        let entry = pocket.iter().find_map(|c| {
            preds
                .get(c)
                .and_then(|ps| ps.iter().find(|p| !trapped.contains(*p)))
                .map(|p| (*p, *c))
        });
        match entry {
            Some((from, to)) => format!(
                "a body gets in from {from:?} to {to:?} by {}",
                movement_words(self, from, to)
            ),
            None => "a body gets in".to_string(),
        }
    }
}

impl World {
    /// Whether a body in `pocket` gets back to `seeds` once it opens every shortcut
    /// whose lever its own reach stands it at — opened in rounds, since a door
    /// one lever opens can lead to the next lever.
    fn leaves_by_a_shortcut(
        &self,
        pocket: &[[i32; 3]],
        seeds: &BTreeSet<[i32; 3]>,
        returned: Option<([i32; 3], [i32; 3])>,
        shortcuts: &[crate::compiler::plan::ShortcutPlan],
    ) -> bool {
        let levers: Vec<Option<[i32; 3]>> = shortcuts
            .iter()
            .map(|s| self.snap_standable(s.unlock, SNAP_RADIUS))
            .collect();
        let mut opened: BTreeSet<usize> = BTreeSet::new();
        loop {
            let owned;
            let w: &World = if opened.is_empty() {
                self
            } else {
                let cells: BTreeSet<[i32; 3]> = opened
                    .iter()
                    .flat_map(|&i| {
                        let (lo, hi) = shortcuts[i].gate_region;
                        crate::compiler::assembled::region_cells(lo, hi)
                    })
                    .collect();
                owned = self.with_cleared(&cells);
                &owned
            };
            let mut seen: BTreeSet<[i32; 3]> = pocket.iter().copied().collect();
            let mut queue: std::collections::VecDeque<[i32; 3]> = seen.iter().copied().collect();
            while let Some(cur) = queue.pop_front() {
                if seeds.contains(&cur) || returned_from(returned, cur) {
                    return true;
                }
                for n in w.body_moves(cur) {
                    if seen.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
            let before = opened.len();
            for (i, lever) in levers.iter().enumerate() {
                if lever.is_some_and(|l| seen.contains(&l)) {
                    opened.insert(i);
                }
            }
            if opened.len() == before {
                return false;
            }
        }
    }

    /// The body-movement closure of `seeds` ([`World::body_moves`]) and, within it,
    /// the cells from which no movement sequence returns to a seed. Returns the
    /// closure, the trapped cells, and every closure cell's predecessors (so a
    /// report can say how a body got in).
    ///
    /// Exact over the closure: whatever a closure cell reaches is itself in the
    /// closure, so walking the predecessor edges backwards from the seeds inside
    /// it answers "can this cell get back" for every cell without leaving it.
    #[allow(clippy::type_complexity)]
    fn cells_a_body_cannot_leave(
        &self,
        roots: &[[i32; 3]],
        ways_out: &BTreeSet<[i32; 3]>,
        returned: Option<([i32; 3], [i32; 3])>,
    ) -> (
        BTreeSet<[i32; 3]>,
        BTreeSet<[i32; 3]>,
        BTreeMap<[i32; 3], Vec<[i32; 3]>>,
    ) {
        let mut preds: BTreeMap<[i32; 3], Vec<[i32; 3]>> = BTreeMap::new();
        let mut seen: BTreeSet<[i32; 3]> = roots.iter().copied().collect();
        let mut queue: std::collections::VecDeque<[i32; 3]> = seen.iter().copied().collect();
        while let Some(cur) = queue.pop_front() {
            for n in self.body_moves(cur) {
                preds.entry(n).or_default().push(cur);
                if seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        // The ways out: the cells that count as having left (the route itself,
        // for `DW0921`) that the closure reaches, and every reached cell the
        // boundary clock carries a body back from.
        let mut back: BTreeSet<[i32; 3]> = seen
            .iter()
            .copied()
            .filter(|c| ways_out.contains(c) || returned_from(returned, *c))
            .collect();
        let mut queue: std::collections::VecDeque<[i32; 3]> = back.iter().copied().collect();
        while let Some(cur) = queue.pop_front() {
            if let Some(ps) = preds.get(&cur) {
                for p in ps {
                    if back.insert(*p) {
                        queue.push_back(*p);
                    }
                }
            }
        }
        let trapped = seen.difference(&back).copied().collect();
        (seen, trapped, preds)
    }
}

/// `DW0921` and the body-movement relation it floods ([`World::body_moves`]).
#[cfg(test)]
mod leave_tests {
    use super::*;
    use delvewright_dsl::metrics::{jump_max_gap, unarmoured_survivable_fall_blocks};

    /// A flat yard, feet at y=1 over a stone floor at y=0, `w` × `d` cells.
    fn yard(w: i32, d: i32) -> BTreeSet<[i32; 3]> {
        let mut solid = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                solid.insert([x, 0, z]);
            }
        }
        solid
    }

    /// A bed ringed by a hedge two high — the vesperhold round-2 garden — with
    /// a boulder one high two cells north of it. `open_north` leaves the ring's
    /// north row out, the repair the campaign took.
    fn hedged_bed(open_north: bool) -> World {
        let mut solid = yard(14, 14);
        for x in 4..=8 {
            for z in 4..=8 {
                let ring = x == 4 || x == 8 || z == 4 || z == 8;
                if ring && !(open_north && z == 4) {
                    solid.insert([x, 1, z]);
                    solid.insert([x, 2, z]);
                }
            }
        }
        solid.insert([6, 1, 2]); // the boulder
        World::from_solid_cells(solid)
    }

    fn judge(w: &World, seeds: &[[i32; 3]]) -> (LeaveBinding, Result<(), Failure>) {
        verify_bodies_can_leave(
            &[(
                w,
                "from critical step 0".to_string(),
                seeds.to_vec(),
                BTreeSet::new(),
            )],
            None,
            &[],
        )
    }

    #[test]
    fn a_body_jumps_the_boulder_onto_the_hedge_and_drops_into_a_bed_it_cannot_leave_dw0921() {
        let w = hedged_bed(false);
        // Only a jump reaches the hedge's top: a gap of one column (z=3) at a
        // rise of one, off the boulder. The walk step never does.
        assert!(w.body_moves([6, 2, 2]).contains(&[6, 3, 4]));
        assert!(!w.neighbors([6, 2, 2]).contains(&[6, 3, 4]));
        let (binding, verdict) = judge(&w, &[[1, 1, 1]]);
        let err = verdict.expect_err("the bed is a trap");
        assert_eq!(err.code.id(), "DW0921");
        assert!(err.message.contains("[5, 1, 5]"), "{}", err.message);
        assert!(
            err.message.contains("a fall of 2 block(s)"),
            "{}",
            err.message
        );
        assert_eq!(binding.trapped, 9, "the bed's 3x3 floor, and nothing else");
    }

    #[test]
    fn a_bed_open_on_one_side_is_walked_out_of() {
        let (binding, verdict) = judge(&hedged_bed(true), &[[1, 1, 1]]);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert!(
            binding.reached > 14 * 14 - 30,
            "the proof judged the yard: {binding:?}"
        );
        assert_eq!(binding.trapped, 0);
    }

    /// A pool of water two deep inside a curb: the water's top cell is y=1 (the
    /// yard's feet level), the curb's standing cell y=3. `curb` is its height in
    /// blocks over the yard; `gap` leaves one curb cell out.
    fn curbed_pool(curb: i32, gap: bool) -> World {
        let mut solid = yard(14, 14);
        let mut water = BTreeSet::new();
        for x in 4..=8 {
            for z in 4..=8 {
                let ring = x == 4 || x == 8 || z == 4 || z == 8;
                if ring {
                    if gap && x == 4 && z == 6 {
                        continue;
                    }
                    for y in 1..=curb {
                        solid.insert([x, y, z]);
                    }
                } else {
                    solid.remove(&[x, 0, z]);
                    solid.insert([x, -1, z]);
                    water.insert([x, 0, z]);
                    water.insert([x, 1, z]);
                }
            }
        }
        World::from_solid_and_flooded(solid, water)
    }

    #[test]
    fn a_body_that_goes_over_the_curb_into_the_water_cannot_climb_back_out_dw0921() {
        // The curb stands one over the yard, so a body steps onto it and drops
        // into the water; afloat at y=1, the curb's top (standing cell y=2) is
        // one cell up — climbable. Two over: the standing cell is y=3, and the
        // body is held.
        let (_, low) = judge(&curbed_pool(1, false), &[[1, 1, 1]]);
        assert!(low.is_ok(), "{low:?}");
        let w = curbed_pool(2, false);
        // Nothing reaches the water here: the curb is two over the yard.
        let (b, walled) = judge(&w, &[[1, 1, 1]]);
        assert!(walled.is_ok());
        assert_eq!(b.afloat, 0, "no body gets over a two-high curb");
        // Give the body a way up — a step one over the yard beside the curb —
        // and it goes in and cannot come out.
        let mut solid: BTreeSet<[i32; 3]> = (0..14)
            .flat_map(|x| (0..14).map(move |z| [x, 0, z]))
            .filter(|c| w.solid_at(*c))
            .collect();
        solid.extend(
            (0..14)
                .flat_map(|x| (0..14).flat_map(move |z| [[x, 1, z], [x, 2, z]]))
                .filter(|c| w.solid_at(*c)),
        );
        solid.insert([6, 1, 3]);
        let water: BTreeSet<[i32; 3]> = (5..=7)
            .flat_map(|x| (5..=7).flat_map(move |z| [[x, 0, z], [x, 1, z]]))
            .collect();
        for c in &water {
            solid.remove(c);
        }
        for x in 5..=7 {
            for z in 5..=7 {
                solid.insert([x, -1, z]);
            }
        }
        let stepped = World::from_solid_and_flooded(solid, water);
        let (b, verdict) = judge(&stepped, &[[1, 1, 1]]);
        let err = verdict.expect_err("the pool holds a swimmer");
        assert_eq!(err.code.id(), "DW0921");
        assert!(err.message.contains("into water"), "{}", err.message);
        assert_eq!(b.afloat, 9, "the pool's nine surface cells, all afloat");
        assert_eq!(b.trapped, 9);
    }

    #[test]
    fn a_gap_in_the_curb_is_a_way_out_of_the_water() {
        // The curb two high with one cell left out: a body wades in through the
        // gap and climbs back out through it (standing cell y=1, level with the
        // water's top, which is inside the climb-out reach).
        let (b, verdict) = judge(&curbed_pool(2, true), &[[1, 1, 1]]);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(b.afloat, 9);
    }

    #[test]
    fn lava_is_never_a_place_a_body_floats() {
        let mut solid = yard(6, 6);
        solid.remove(&[3, 0, 3]);
        solid.insert([3, -1, 3]);
        let occ = crate::compiler::assembled::Occupancy {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: [[3, 0, 3]].into_iter().collect(),
            partial: BTreeMap::new(),
            waterloggable: BTreeSet::new(),
            lava: [[3, 0, 3]].into_iter().collect(),
        };
        let w = World::from_occupancy(occ, Premises::geometry_only());
        assert!(!w.body_moves([2, 1, 3]).contains(&[3, 0, 3]));
        assert!(!w.is_water_surface([3, 0, 3]));
    }

    /// `fatal_step_off` (spec-0085 §6.2): a lava pool let into the floor beside a
    /// standing cell is a step into lava; level floor is not a step at all.
    #[test]
    fn a_step_into_a_lava_pool_is_fatal_and_level_floor_is_not() {
        let mut solid = yard(6, 6);
        solid.remove(&[3, 0, 3]);
        solid.insert([3, -1, 3]);
        let occ = crate::compiler::assembled::Occupancy {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: [[3, 0, 3]].into_iter().collect(),
            partial: BTreeMap::new(),
            waterloggable: BTreeSet::new(),
            lava: [[3, 0, 3]].into_iter().collect(),
        };
        let w = World::from_occupancy(occ, Premises::geometry_only());
        assert_eq!(w.fatal_step_off([2, 1, 3]), Some(([3, 1, 3], true)));
        assert_eq!(w.fatal_step_off([1, 1, 1]), None, "level floor all round");
    }

    /// `fatal_step_off`: a drop deeper than an unarmoured body survives is
    /// fatal; one inside it is a landing.
    #[test]
    fn a_drop_past_the_survivable_fall_is_fatal_and_one_inside_it_is_not() {
        let deepest = unarmoured_survivable_fall_blocks() as i32;
        let ledge_over = |depth: i32| {
            // A one-cell ledge walled on three sides, open to the east.
            let mut solid = BTreeSet::new();
            solid.insert([0, 0, 0]);
            for wall in [[-1, 0], [0, -1], [0, 1]] {
                for y in 1..=2 {
                    solid.insert([wall[0], y, wall[1]]);
                }
            }
            solid.insert([1, -depth, 0]);
            World::from_solid_cells(solid)
        };
        // Feet at y=1; a landing whose top is at y=1-depth puts the feet at
        // 1-depth+1, a fall of `depth` blocks.
        assert_eq!(ledge_over(deepest).fatal_step_off([0, 1, 0]), None);
        assert_eq!(
            ledge_over(deepest + 1).fatal_step_off([0, 1, 0]),
            Some(([1, 1, 0], false))
        );
    }

    #[test]
    fn a_cell_the_boundary_carries_a_body_out_of_is_a_way_out() {
        let w = hedged_bed(false);
        // A region whose box ends at x=5: the bed's floor at x=5..7 has cells
        // outside it, and the clock carries a body there back.
        let (_, verdict) = verify_bodies_can_leave(
            &[(
                &w,
                "from critical step 0".to_string(),
                vec![[1, 1, 1]],
                BTreeSet::new(),
            )],
            Some(([0, -8, 0], [5, 64, 13])),
            &[],
        );
        assert!(verdict.is_ok(), "{verdict:?}");
    }

    #[test]
    fn the_jump_reaches_as_far_as_the_measured_envelope_and_no_further() {
        // Two platforms at feet y=1, a gap of `g` air columns between them.
        let platforms = |g: i32| {
            let mut solid = BTreeSet::new();
            for x in 0..3 {
                solid.insert([x, 0, 0]);
            }
            for x in (3 + g)..(6 + g) {
                solid.insert([x, 0, 0]);
            }
            World::from_solid_cells(solid)
        };
        let flat = jump_max_gap(0).expect("a flat jump") as i32;
        assert!(
            platforms(flat)
                .body_moves([2, 1, 0])
                .contains(&[3 + flat, 1, 0])
        );
        assert!(
            !platforms(flat + 1)
                .body_moves([2, 1, 0])
                .contains(&[4 + flat, 1, 0])
        );
    }
}
