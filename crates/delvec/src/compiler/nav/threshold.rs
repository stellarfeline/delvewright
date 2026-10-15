//! **A trigger the path relies on cannot be walked past** (`DW1044`).
//!
//! The critical path performs an environment trigger when a later step needs
//! the flag it pays ([`crate::compiler::flow::Flow::trigger_debts`]). The bot
//! performs it by walking to the trigger's anchor cell, so the route proof
//! sees the volume crossed. A player walks from where the last step left them
//! to where the needing step completes, and only fires the trigger if that
//! walk enters its volume. When some walk goes round the volume, the flag is
//! never paid and the needing step never completes.
//!
//! So for every walk-fired trigger (`approach`, `step`) that pays a flag debt,
//! the proof asks the router ([`World::find_path_avoiding`]) for a walk over
//! each leg from the position before the trigger step to the needing step's
//! position that never enters a cell where the trigger **surely** fires. If
//! every leg has one, the trigger can be walked past and the build is refused.
//! The trigger's own region writes are withheld while the walk is sought,
//! because a body that walked past it never caused them.
//!
//! A cell surely fires when every place a body can stand in it is caught by the
//! emitted selector, sampled once a tick:
//!
//! * `approach` emits `execute positioned X Y Z if entity @a[distance=..R]`.
//!   Integer `X`/`Z` are centre-corrected by the game, so the point is
//!   `(X + 0.5, Y, Z + 0.5)`, and `distance` is measured to the player's feet.
//!   A cell counts when the farthest point its body can take — the column's
//!   square at the cell's own feet height, drawn in by half the player's width
//!   on a side where the next column is solid at foot or head height — plus
//!   half the fastest one-tick move
//!   ([`delvewright_dsl::metrics::POLL_HORIZONTAL_BLOCKS_PER_TICK`]) is within `R`.
//! * `step` emits a one-block `dx=0` box at the plate's cell, which a body's
//!   hitbox meets from anywhere in the plate's own column: that cell.
//!
//! A body in the air over a cell is not modelled: the feet are read where they
//! stand on the cell's floor.

use crate::compiler::failure::Failure;
use crate::compiler::nav::route::{VisitedPos, critical_positions};
use crate::compiler::nav::world::World;
use crate::compiler::nav::world::body::Footprint;
use crate::compiler::plan::{Plan, RegionEvents, Step};
use delvewright_dsl::TriggerOn;
use std::collections::BTreeSet;

/// The cells a walk-fired trigger surely fires in, by the rule its emitted
/// selector reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::compiler::nav) enum FiringVolume {
    /// An `approach`: the anchor cell and its `range`.
    Sphere {
        /// The anchor cell, as `positioned` is written.
        anchor: [i32; 3],
        /// The `distance=..range` bound.
        range: u32,
    },
    /// A `step`: the plate's cell.
    Cell([i32; 3]),
}

impl FiringVolume {
    /// Whether every body standing in `c` on `w` is caught on some tick.
    pub(in crate::compiler::nav) fn surely_fires(&self, w: &World, c: [i32; 3]) -> bool {
        match *self {
            FiringVolume::Cell(p) => c == p,
            FiringVolume::Sphere { anchor, range } => {
                use delvewright_dsl::metrics::{PLAYER_WIDTH, POLL_HORIZONTAL_BLOCKS_PER_TICK};
                let half = PLAYER_WIDTH / 2.0;
                let walled = |d: [i32; 3]| w.is_solid(d) || w.is_solid([d[0], d[1] + 1, d[2]]);
                let span = |lo: i32, minus: [i32; 3], plus: [i32; 3]| {
                    let a = f64::from(lo) + if walled(minus) { half } else { 0.0 };
                    let b = f64::from(lo + 1) - if walled(plus) { half } else { 0.0 };
                    (a, b)
                };
                let (x0, x1) = span(c[0], [c[0] - 1, c[1], c[2]], [c[0] + 1, c[1], c[2]]);
                let (z0, z1) = span(c[2], [c[0], c[1], c[2] - 1], [c[0], c[1], c[2] + 1]);
                let px = f64::from(anchor[0]) + 0.5;
                let pz = f64::from(anchor[2]) + 0.5;
                let fx = (x0 - px).abs().max((x1 - px).abs());
                let fz = (z0 - pz).abs().max((z1 - pz).abs());
                let dy = w.feet_y(c) - f64::from(anchor[1]);
                (fx * fx + dy * dy + fz * fz).sqrt() + POLL_HORIZONTAL_BLOCKS_PER_TICK / 2.0
                    <= f64::from(range)
            }
        }
    }

    /// The point a body is measured against, for a witness's nearest approach.
    fn point(&self) -> [f64; 3] {
        let a = match *self {
            FiringVolume::Sphere { anchor, .. } | FiringVolume::Cell(anchor) => anchor,
        };
        [
            f64::from(a[0]) + 0.5,
            f64::from(a[1]),
            f64::from(a[2]) + 0.5,
        ]
    }

    fn words(&self) -> String {
        match *self {
            FiringVolume::Sphere { anchor, range } => {
                format!("`approach` within {range} of {anchor:?}")
            }
            FiringVolume::Cell(p) => format!("`step` on the plate at {p:?}"),
        }
    }
}

/// One walk round a trigger's volume: the leg it walks and its cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::compiler::nav) struct WalkRound {
    pub(in crate::compiler::nav) from: [i32; 3],
    pub(in crate::compiler::nav) to: [i32; 3],
    pub(in crate::compiler::nav) cells: Vec<[i32; 3]>,
}

/// What the walks over a chain of legs do with a volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::compiler::nav) enum Passage {
    /// Leg `n` of the chain has no walk that keeps out of the volume.
    Crossed(usize),
    /// Every leg has one: the first leg's is the witness.
    Round(WalkRound),
}

/// **Whether every walk over `legs` enters `volume`** — the pure core of
/// [`check_threshold_triggers`]. Each leg is routed over its own
/// causally-sealed world ([`World::walked_leg_region_state`]) under `ancestor`,
/// which the caller has already stripped of the trigger's own steps. A leg
/// whose ends do not snap, or that has no walk at all, is one no body makes
/// without the trigger: it counts as crossed.
pub(in crate::compiler::nav) fn passage(
    world: &World,
    legs: &[(VisitedPos, VisitedPos)],
    volume: &FiringVolume,
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Passage {
    let mut first: Option<WalkRound> = None;
    for (n, (a, b)) in legs.iter().enumerate() {
        let st = world.walked_leg_region_state(region_events, ancestor, a.src_step, b.src_step);
        let owned;
        let w: &World = if st.is_empty() {
            world
        } else {
            owned = world.with_region_state(&st);
            &owned
        };
        let (Some(start), Some(goal)) = (
            w.snap_endpoint(a.pos, false),
            w.snap_endpoint(b.pos, b.talk_to),
        ) else {
            return Passage::Crossed(n);
        };
        let Some(cells) = w.find_path_avoiding(start, goal, &Footprint::player(), &|c| {
            volume.surely_fires(w, c)
        }) else {
            return Passage::Crossed(n);
        };
        if first.is_none() {
            first = Some(WalkRound {
                from: start,
                to: goal,
                cells,
            });
        }
    }
    match first {
        Some(r) => Passage::Round(r),
        // No walked leg at all: nothing a body walks enters the volume.
        None => Passage::Round(WalkRound {
            from: [0; 3],
            to: [0; 3],
            cells: Vec::new(),
        }),
    }
}

/// What `DW1044` examined, printed on every build whichever way it goes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThresholdBinding {
    /// Distinct triggers the critical path performs.
    pub performed: usize,
    /// Of those, walk-fired triggers that pay a flag a later step reads.
    pub judged: usize,
    /// Of the judged, triggers every walk to the needing step enters.
    pub crossed: usize,
    /// Performed triggers that pay no flag a later step reads (they open a way,
    /// or move a datum): not judged.
    pub no_debt: usize,
    /// Performed debt triggers fired by a hand (`use`, `strike`, `strike-npc`,
    /// `strike-assembly`): an act, not a volume walked through; not judged.
    pub by_hand: usize,
    /// Performed debt triggers that are a link the party stands in to be
    /// carried: the route proof walks into their volume; not judged.
    pub carried: usize,
}

impl ThresholdBinding {
    /// The one line a build prints about this proof.
    pub fn line(&self) -> String {
        format!(
            "DW1044 binding: {} trigger(s) performed by the critical path; {} judged (walk-fired, \
             paying a flag a later step reads), {} entered by every walk to the step that needs \
             it; not judged: {} pay no flag a later step reads, {} fired by hand, {} a link the \
             party is carried by",
            self.performed, self.judged, self.crossed, self.no_debt, self.by_hand, self.carried
        )
    }
}

/// **`DW1044`**: every walk-fired trigger the critical path performs to pay a
/// flag a later step reads is entered by every walk from the position before
/// it to the position where that step completes.
pub fn check_threshold_triggers(
    plan: &Plan,
    world: &World,
) -> (ThresholdBinding, Result<(), Failure>) {
    let mut b = ThresholdBinding::default();
    let positions = critical_positions(plan);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut refusal: Option<Failure> = None;
    for (k, step) in plan.critical_path.iter().enumerate() {
        let Step::Trigger {
            trigger_id, stand, ..
        } = step
        else {
            continue;
        };
        if !seen.insert(trigger_id.as_str()) {
            continue;
        }
        b.performed += 1;
        let Some((reader, flag)) = plan.path_firing.trigger_debts.get(trigger_id) else {
            b.no_debt += 1;
            continue;
        };
        let Some(&r) = plan.objective_steps.get(reader) else {
            b.no_debt += 1;
            continue;
        };
        let Some(t) = plan
            .campaign
            .quests
            .content
            .triggers
            .iter()
            .find(|t| t.id.as_str() == trigger_id)
        else {
            continue;
        };
        if stand.is_some() {
            b.carried += 1;
            continue;
        }
        let Some(anchor) = t.at_anchor().and_then(|a| plan.point_any(a)) else {
            b.by_hand += 1;
            continue;
        };
        let volume = match t.on {
            TriggerOn::Approach { range } => FiringVolume::Sphere { anchor, range },
            TriggerOn::Step => FiringVolume::Cell(anchor),
            _ => {
                b.by_hand += 1;
                continue;
            }
        };
        b.judged += 1;
        let own: BTreeSet<usize> = plan
            .critical_path
            .iter()
            .enumerate()
            .filter(|(_, s)| s.trigger() == Some(trigger_id.as_str()))
            .map(|(i, _)| i)
            .collect();
        let legs = chain(plan, &positions, k, r, &own);
        let ancestor = |g: usize, s: usize| !own.contains(&g) && plan.gate_fired_before(g, s);
        match passage(world, &legs, &volume, &plan.region_events, &ancestor) {
            Passage::Crossed(_) => b.crossed += 1,
            Passage::Round(witness) => {
                if refusal.is_none() {
                    let wider = match volume {
                        FiringVolume::Sphere { range, .. } => range + 1..=range + 8,
                        FiringVolume::Cell(_) => 1..=8,
                    }
                    .find(|&n| {
                        matches!(
                            passage(
                                world,
                                &legs,
                                &FiringVolume::Sphere { anchor, range: n },
                                &plan.region_events,
                                &ancestor,
                            ),
                            Passage::Crossed(_)
                        )
                    });
                    refusal = Some(walked_past(
                        trigger_id,
                        reader,
                        flag,
                        t.on.kind(),
                        &volume,
                        &witness,
                        wider,
                    ));
                }
            }
        }
    }
    (b, refusal.map_or(Ok(()), Err))
}

/// The legs a body walks from the position before trigger step `k` to the
/// position of step `r` that needs it, the trigger's own positions dropped and
/// rides left out. A trigger reached by a ride starts its walk where the ride
/// puts the party.
fn chain(
    plan: &Plan,
    positions: &[VisitedPos],
    k: usize,
    r: usize,
    own: &BTreeSet<usize>,
) -> Vec<(VisitedPos, VisitedPos)> {
    let Some(ti) = positions.iter().position(|p| p.src_step == k) else {
        return Vec::new();
    };
    let Some(ri) = positions.iter().rposition(|p| p.src_step == r) else {
        return Vec::new();
    };
    if ti == 0 || ri <= ti {
        return Vec::new();
    }
    let mut start = positions[ti - 1];
    if positions[ti].transport_before {
        let dest = (0..k)
            .rev()
            .find_map(|i| plan.critical_path_transport.get(i).copied().flatten());
        let Some(dest) = dest else {
            return Vec::new();
        };
        start = VisitedPos {
            pos: dest,
            transport_before: false,
            ..start
        };
    }
    let mut legs = Vec::new();
    let mut cur = start;
    for p in &positions[ti + 1..=ri] {
        if own.contains(&p.src_step) {
            continue;
        }
        if !p.transport_before {
            legs.push((cur, *p));
        }
        cur = *p;
    }
    legs
}

fn walked_past(
    trigger: &str,
    reader: &str,
    flag: &str,
    kind: &str,
    volume: &FiringVolume,
    w: &WalkRound,
    wider: Option<u32>,
) -> Failure {
    let p = volume.point();
    let near = w
        .cells
        .iter()
        .map(|c| {
            let d = [
                f64::from(c[0]) + 0.5 - p[0],
                f64::from(c[1]) - p[1],
                f64::from(c[2]) + 0.5 - p[2],
            ];
            ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(), *c)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let witness = match near {
        Some((d, c)) => format!(
            "a walk of {} cell(s) from {:?} to {:?} passes {c:?}, {d:.2} block(s) from the \
             trigger's point, and never stands where the selector is sure to catch it",
            w.cells.len(),
            w.from,
            w.to
        ),
        None => "no step between them is walked, so no walk enters it".to_string(),
    };
    let widen = match (volume, wider) {
        (FiringVolume::Sphere { range, .. }, Some(n)) => {
            format!("widen `range` from {range} to {n}, the least range every walk enters")
        }
        (FiringVolume::Sphere { range, .. }, None) => format!(
            "widen `range` past {range} (no range up to {} closes the way round)",
            range + 8
        ),
        (FiringVolume::Cell(_), Some(n)) => format!(
            "fire it on `approach` with `range` {n} instead of `step`, the least range every \
             walk enters"
        ),
        (FiringVolume::Cell(_), None) => {
            "fire it on `approach` with a range every walk enters instead of `step`".to_string()
        }
    };
    Failure {
        code: super::DW_THRESHOLD_WALKED_PAST,
        message: format!(
            "trigger `{trigger}` ({kind}) pays `{flag}`, which `{reader}` reads, and the \
             critical path performs it by walking onto its anchor; a player need not. Its volume is {}, and {witness}. \
             A player who takes that walk never fires it, and `{reader}` never completes. \
             Remedy: {widen}; or move the anchor into the only way through; or close the way \
             round it. Do not drop `{flag}` from `{reader}` to silence the proof.",
            volume.words()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::{floored, linear};

    fn at(pos: [i32; 3], src_step: usize) -> VisitedPos {
        VisitedPos {
            pos,
            transport_before: false,
            talk_to: false,
            src_step,
            by_link: false,
            by_loop: false,
        }
    }

    /// The chapel-door shape: a wall along z = 5 across a 12-wide floor with a
    /// doorway three cells wide (x 4..=6), the yard south of it and the nave
    /// north. `extra` opens more of the wall.
    fn chapel(extra: &[i32]) -> World {
        let mut walls = Vec::new();
        for x in 0..12 {
            if (4..=6).contains(&x) || extra.contains(&x) {
                continue;
            }
            walls.push([x, 64, 5]);
            walls.push([x, 65, 5]);
        }
        floored(12, 12, 64, &walls)
    }

    fn walk(world: &World, volume: FiringVolume) -> Passage {
        let legs = [(at([5, 64, 1], 1), at([5, 64, 10], 3))];
        passage(world, &legs, &volume, &RegionEvents::default(), &linear)
    }

    #[test]
    fn a_range_one_trigger_in_a_three_wide_door_is_walked_past() {
        let v = FiringVolume::Sphere {
            anchor: [5, 64, 5],
            range: 1,
        };
        let Passage::Round(w) = walk(&chapel(&[]), v) else {
            panic!("a three-wide door with a range-1 trigger in its middle is walked round")
        };
        let world = chapel(&[]);
        assert!(w.cells.iter().all(|&c| !v.surely_fires(&world, c)));
        // Not one cell of the doorway is sure to catch a body: its middle column
        // reaches 0.71 from the point, and a one-tick move carries that past 1.
        assert!((4..=6).all(|x| !v.surely_fires(&world, [x, 64, 5])));
    }

    #[test]
    fn the_same_door_with_the_range_widened_is_entered_by_every_walk() {
        let v = FiringVolume::Sphere {
            anchor: [5, 64, 5],
            range: 2,
        };
        assert_eq!(walk(&chapel(&[]), v), Passage::Crossed(0));
    }

    #[test]
    fn a_second_door_round_the_trigger_is_walked_past_at_the_widened_range() {
        let v = FiringVolume::Sphere {
            anchor: [5, 64, 5],
            range: 2,
        };
        assert!(matches!(walk(&chapel(&[10]), v), Passage::Round(_)));
    }

    #[test]
    fn a_range_one_trigger_in_a_one_wide_door_is_the_only_way_through() {
        // Narrow the door to x = 5: the wall's flanks hold the body to the
        // middle of the cell, and the column there is caught.
        let mut walls = Vec::new();
        for x in 0..12 {
            if x == 5 {
                continue;
            }
            walls.push([x, 64, 5]);
            walls.push([x, 65, 5]);
        }
        let w = floored(12, 12, 64, &walls);
        let v = FiringVolume::Sphere {
            anchor: [5, 64, 5],
            range: 1,
        };
        assert!(v.surely_fires(&w, [5, 64, 5]));
        assert_eq!(walk(&w, v), Passage::Crossed(0));
    }

    #[test]
    fn a_plate_in_a_wide_door_is_stepped_round_and_one_in_a_narrow_door_is_not() {
        assert!(matches!(
            walk(&chapel(&[]), FiringVolume::Cell([5, 64, 5])),
            Passage::Round(_)
        ));
        let mut walls = Vec::new();
        for x in 0..12 {
            if x != 5 {
                walls.push([x, 64, 5]);
                walls.push([x, 65, 5]);
            }
        }
        let w = floored(12, 12, 64, &walls);
        assert_eq!(
            walk(&w, FiringVolume::Cell([5, 64, 5])),
            Passage::Crossed(0)
        );
    }

    #[test]
    fn the_selector_point_is_centre_corrected_and_read_at_the_feet() {
        let w = floored(12, 12, 64, &[]);
        // Open floor: the anchor's own column reaches 0.71 from the point, and
        // a one-tick move takes it past 1.
        let v1 = FiringVolume::Sphere {
            anchor: [5, 64, 5],
            range: 1,
        };
        assert!(!v1.surely_fires(&w, [5, 64, 5]));
        let v2 = FiringVolume::Sphere {
            anchor: [5, 64, 5],
            range: 2,
        };
        assert!(v2.surely_fires(&w, [5, 64, 5]));
        // East neighbour: its far corner is 1.58 from the point, 1.87 with the
        // tick slack — inside 2. The next one out, 2.55, is not.
        assert!(v2.surely_fires(&w, [6, 64, 5]));
        assert!(!v2.surely_fires(&w, [7, 64, 5]));
        // The feet are read where they stand: the same column one block up is
        // a block farther, and range 2 no longer holds it.
        let mut raised = std::collections::BTreeSet::new();
        for x in 0..12 {
            for z in 0..12 {
                raised.insert([x, 64, z]);
                raised.insert([x, 67, z]);
            }
        }
        let up = World::from_solid_cells(raised);
        assert!(!v2.surely_fires(&up, [6, 65, 5]));
    }

    #[test]
    fn a_chain_with_no_walked_leg_enters_nothing() {
        let v = FiringVolume::Sphere {
            anchor: [5, 64, 5],
            range: 1,
        };
        // No legs: nothing walked enters the volume.
        assert!(matches!(
            passage(&chapel(&[]), &[], &v, &RegionEvents::default(), &linear),
            Passage::Round(_)
        ));
    }
}
