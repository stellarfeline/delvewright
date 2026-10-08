/// The minimum share of a timed hazard's cycle that must admit passage
/// (spec-0016 §4). Below this the hazard stops being a timing read and becomes a
/// coin flip. Expressed as a percentage so the arithmetic below stays in
/// integers — no float rounding in a proof (ADR-0006). One floor for every timed
/// hazard: a `timed-gate` (`DW0378`) and a `volley` (`DW0918`) are judged by
/// [`timing_read`] against this same number.
const TIMING_READ_MIN_ADMIT_PERCENT: u32 = 20;

/// One timing read: a route of `moves` blocks that must be completed inside a
/// window of `open_ticks` which recurs every `open_ticks + closed_ticks`.
///
/// **The one body model every timed-hazard proof is taken under.** The route is
/// charged at [`SPRINT_TICKS_PER_BLOCK`]; a body that sets off `p` ticks into the
/// window arrives in time iff `p + cross <= open_ticks`, so the admitting phases
/// are `max(0, open_ticks - cross + 1)` of the cycle, and the share is an integer
/// percentage rounded DOWN — the proof never credits a hazard with a share it
/// does not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TimingRead {
    /// Blocks on the route the body must complete inside the window.
    moves: u32,
    /// `moves` charged at [`SPRINT_TICKS_PER_BLOCK`].
    cross_ticks: u32,
    /// Entry phases, of `cycle`, from which the route is completed in time.
    admits: u32,
    /// Ticks in one full cycle.
    cycle: u32,
    /// `admits` as a percentage of `cycle`, rounded down.
    percent: u32,
}

impl TimingRead {
    /// Below [`TIMING_READ_MIN_ADMIT_PERCENT`]: a coin flip, not a timing read.
    fn is_coin_flip(&self) -> bool {
        self.percent < TIMING_READ_MIN_ADMIT_PERCENT
    }
}

/// Judge a route of `moves` blocks against a window of `open_ticks` recurring
/// every `open_ticks + closed_ticks` — see [`TimingRead`].
fn timing_read(moves: u32, open_ticks: u32, closed_ticks: u32) -> TimingRead {
    let cross_ticks = moves * SPRINT_TICKS_PER_BLOCK;
    let cycle = open_ticks + closed_ticks;
    let admits = open_ticks.saturating_sub(cross_ticks) + u32::from(cross_ticks <= open_ticks);
    let percent = admits.saturating_mul(100) / cycle.max(1);
    TimingRead {
        moves,
        cross_ticks,
        admits,
        cycle,
        percent,
    }
}

/// Prove every `timed-gate` is readable — [`DW_TIMED_GATE_COIN_FLIP`] (`DW0378`).
///
/// The requirement is deliberately **not** all-phase passability: spec-0016 §4 is
/// explicit that a gate which punishes bad timing is the entire point. What must
/// hold is that the gate can be *read*: over one full cycle, the entry phases from
/// which a walking player clears the span before it shuts must cover at least
/// [`TIMING_READ_MIN_ADMIT_PERCENT`] of the cycle.
///
/// The crossing cost comes from the same nav model every other proof uses: the A*
/// step count from the footing on one side of the gate region to the footing on
/// the other with the gate open, charged at [`SPRINT_TICKS_PER_BLOCK`]. A player
/// who starts the crossing `p` ticks into the open window arrives in time iff
/// `p + cross <= open_ticks`, so the admitting window is
/// `max(0, open_ticks - cross + 1)` ticks out of `open_ticks + closed_ticks`.
///
/// A gate whose two sides have no walkable footing, or that no route connects even
/// while open, is left to the geometry proofs that own it (`DW0311`) rather than
/// double-reported here.
pub fn check_timed_gates(plan: &Plan, world: &World) -> Result<(), Failure> {
    verify_timed_gates(world, &plan.timed_gates)
}

/// Prove every `timed-gate` `disarm` affordance can be reached before the gate is
/// crossed — [`DW_TIMED_GATE_DISARM_UNREACHABLE`] (`DW0393`).
///
/// One clause, the same one `DW0373` puts on a shortcut's unlock: the `via` cell
/// must be walkable from the campaign entry over the world with the gate span
/// **SEALED**. Searching the open world would "prove" a lever whose only approach
/// is through the portcullis — precisely the fake third rung this exists to
/// refuse.
///
/// Vacuous where another proof owns the ground: no entry (`DW0345`), an
/// unstandable entry or `via` cell (the anchor checks), a gate with no `disarm`.
pub fn check_timed_gate_disarms(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> Result<(), Failure> {
    verify_timed_gate_disarms(world, &plan.timed_gates, entry)
}

/// The pure core of [`check_timed_gate_disarms`] (unit-testable against a
/// synthetic [`World`]).
fn verify_timed_gate_disarms(
    world: &World,
    gates: &[crate::compiler::plan::TimedGatePlan],
    entry: Option<[i32; 3]>,
) -> Result<(), Failure> {
    let Some(entry) = entry else {
        return Ok(());
    };
    for g in gates {
        let Some(dis) = &g.disarm else {
            continue;
        };
        let cells: BTreeSet<[i32; 3]> =
            crate::compiler::assembled::region_cells(g.gate_region.0, g.gate_region.1).collect();
        let sealed = world.with_sealed(&cells);
        let start = sealed.snap_standable(entry, SNAP_RADIUS);
        let goal = sealed.snap_standable(dis.via_cell, SNAP_RADIUS);
        let (Some(start), Some(goal)) = (start, goal) else {
            continue; // an unstandable entry or lever cell is another proof's concern
        };
        if sealed.find_path(start, goal).is_some() {
            continue;
        }
        return Err(Failure {
            code: DW_TIMED_GATE_DISARM_UNREACHABLE,
            message: format!(
                "timed gate `{}`: its disarm affordance at `{}` ({:?}) is not walkable from the \
                 campaign entry while gate `{}` is closed, so the only way to the jam lever is \
                 THROUGH the portcullis. A disarm the party can reach only by first surviving the \
                 hazard disables nothing — it is a trophy for having beaten it, not the third rung \
                 of the ladder (souls dossier §5.2: readable, avoidable, disable-able). \
                 Put the lever on ground the approach already touches — the stair head above the \
                 run, the alcove beside the doorway — or drop the `disarm` and let the clock \
                 stand. Do NOT leave the gate open at world-load to silence this.",
                g.id, dis.via_anchor, dis.via_cell, g.gate_anchor
            ),
        });
    }
    Ok(())
}

/// The pure core of [`check_timed_gates`] (unit-testable against a synthetic
/// [`World`]).
fn verify_timed_gates(
    world: &World,
    gates: &[crate::compiler::plan::TimedGatePlan],
) -> Result<(), Failure> {
    for g in gates {
        let cells: BTreeSet<[i32; 3]> =
            crate::compiler::assembled::region_cells(g.gate_region.0, g.gate_region.1).collect();
        let Some((near, far)) = gate_crossing_footings(world, g.gate_region, &cells) else {
            continue; // no footing on both sides — a geometry concern, not a timing one
        };
        let Some(path) = world.find_path(near, far) else {
            continue; // the open gate connects nothing — DW0311's business
        };
        // `path` includes both endpoints; the crossing is the moves between them.
        let read = timing_read(
            path.len().saturating_sub(1) as u32,
            g.open_ticks,
            g.closed_ticks,
        );
        if read.is_coin_flip() {
            let TimingRead {
                moves,
                cross_ticks,
                admits,
                cycle,
                percent,
            } = read;
            return Err(Failure {
                code: DW_TIMED_GATE_COIN_FLIP,
                message: format!(
                    "timed gate `{}` is a coin flip, not a timing read: crossing its span takes \
                     {cross_ticks} ticks ({moves} blocks at {SPRINT_TICKS_PER_BLOCK} t/block), so \
                     only {admits} of its {cycle}-tick cycle ({percent}%) admit a player who \
                     starts walking then — under the {TIMING_READ_MIN_ADMIT_PERCENT}% floor \
                     (spec-0016 §4). Punishing bad timing is the point; punishing EVERY \
                     timing is a slot machine. Lengthen `open_ticks`, shorten `closed_ticks`, or \
                     narrow the span — never lower the floor.",
                    g.id
                ),
            });
        }
    }
    Ok(())
}

/// The standable footings immediately on either side of a gate region, over the
/// world with the region SEALED so neither endpoint can land inside it or snap
/// through. The crossing axis is whichever horizontal axis actually has footing on
/// both sides — trying x then z rather than guessing from the region's extents is
/// both deterministic and correct for a square 1×1 gate column, where the extents
/// tie and a guess would pick the wall's own plane.
fn gate_crossing_footings(
    world: &World,
    region: ([i32; 3], [i32; 3]),
    cells: &BTreeSet<[i32; 3]>,
) -> Option<([i32; 3], [i32; 3])> {
    let sealed = world.with_sealed(cells);
    let (from, to) = region;
    for axis in [0usize, 2] {
        let mut near = None;
        let mut far = None;
        for cell in crate::compiler::assembled::region_cells(from, to) {
            for (slot, delta) in [(&mut near, -1), (&mut far, 1)] {
                if slot.is_some() {
                    continue;
                }
                let mut c = cell;
                c[axis] += delta;
                if !cells.contains(&c) && sealed.standable(c) {
                    *slot = Some(c);
                }
            }
            if near.is_some() && far.is_some() {
                break;
            }
        }
        if let (Some(n), Some(f)) = (near, far) {
            return Some((n, f));
        }
    }
    None
}

#[cfg(test)]
mod tests {

    fn timed_gate(
        region: ([i32; 3], [i32; 3]),
        open_ticks: u32,
        closed_ticks: u32,
    ) -> crate::compiler::plan::TimedGatePlan {
        crate::compiler::plan::TimedGatePlan {
            id: "timed-gate/piston-hall".to_string(),
            safe: "piston_hall".to_string(),
            gate_anchor: "anchor/gate".to_string(),
            gate_region: region,
            gate_block: "minecraft:iron_bars".to_string(),
            open_ticks,
            closed_ticks,
            phase: 0,
            // The DW0378 window proof is about geometry and timing, not the
            // penalty for mistiming it — crush changes neither.
            crush: false,
            // …nor does a disarm: `DW0393` is its own proof below.
            disarm: None,
        }
    }

    /// The same gate with a jam lever at `via`.
    fn timed_gate_with_disarm(
        region: ([i32; 3], [i32; 3]),
        via: [i32; 3],
    ) -> crate::compiler::plan::TimedGatePlan {
        let mut g = timed_gate(region, 60, 40);
        g.disarm = Some(crate::compiler::plan::TimedGateDisarmPlan {
            via_anchor: "anchor/jam-lever".to_string(),
            via_cell: via,
            sets_flag: "flag/gate-jammed".to_string(),
        });
        g
    }

    // --- timed-gate disarm reachability (DW0393) ---

    /// The jam lever on the ENTRY side of the barred doorway: the party walks up
    /// to the portcullis, sees the clock, and can pull the lever without ever
    /// stepping into the span. The third rung, working.
    #[test]
    fn timed_gate_disarm_on_the_approach_side_passes() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate_with_disarm(([1, 65, 4], [1, 66, 4]), [3, 65, 2]);
        verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect("a lever on the near side of the gate is reachable before the crossing");
    }

    /// The same lever moved past the doorway, with the gate the only hole in the
    /// wall: the only route to it is through the portcullis, so the "disarm"
    /// rewards a crossing the party already survived. `DW0393`.
    #[test]
    fn timed_gate_disarm_behind_its_own_gate_is_dw0393() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate_with_disarm(([1, 65, 4], [1, 66, 4]), [3, 65, 7]);
        let err = verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect_err("a lever only reachable through the gate must fail");
        assert_eq!(err.code, DW_TIMED_GATE_DISARM_UNREACHABLE); // DW0393
        assert!(
            err.message.contains("timed-gate/piston-hall"),
            "{}",
            err.message
        );
        assert!(err.message.contains("anchor/jam-lever"), "{}", err.message);
    }

    /// …and with a bypass hole in the same wall, that far-side lever is reachable
    /// the long way round while the gate is shut, so the same geometry passes. The
    /// proof is about pre-commitment, not about which side of a wall a cell is on.
    #[test]
    fn timed_gate_disarm_behind_the_gate_but_reachable_the_long_way_passes() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        let g = timed_gate_with_disarm(([1, 65, 4], [1, 66, 4]), [3, 65, 7]);
        verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect("a detour around the gate makes the lever pre-commitment ground");
    }

    /// A gate with no `disarm` is not judged at all — the whole proof is vacuous
    /// for every campaign authored before the field existed.
    #[test]
    fn timed_gate_without_a_disarm_is_not_judged() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 60, 40);
        verify_timed_gate_disarms(&world, &[g], Some([1, 65, 1]))
            .expect("no disarm, nothing to prove");
    }

    /// A generous window: crossing a 1-cell doorway costs a handful of ticks and
    /// the gate stands open for 60 of every 100, so most of the cycle is a legal
    /// entry. A readable gate.
    #[test]
    fn timed_gate_with_a_generous_window_is_readable() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 60, 40);
        verify_timed_gates(&world, &[g]).expect("60 open of a 100-tick cycle is a timing read");
    }

    /// The same span with an open window barely longer than the crossing itself:
    /// almost every entry phase is a death, so the gate is a coin flip. `DW0378`.
    #[test]
    fn timed_gate_whose_window_barely_admits_a_crossing_is_dw0378() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        // Crossing the doorway is 2 moves = 8 ticks; a 10-tick open window inside
        // a 200-tick cycle admits 3 of 200 phases = 1%.
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 10, 190);
        let err = verify_timed_gates(&world, &[g])
            .expect_err("a window that admits ~1% of the cycle is a slot machine");
        assert_eq!(err.code, DW_TIMED_GATE_COIN_FLIP); // DW0378
        assert!(
            err.message.contains("coin flip"),
            "the message must name the failure: {}",
            err.message
        );
    }

    /// An open window SHORTER than the crossing admits nothing at all — the
    /// degenerate end of the same rule, and the one a player can never learn.
    #[test]
    fn timed_gate_no_one_can_ever_cross_is_dw0378() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 2, 20);
        let err = verify_timed_gates(&world, &[g])
            .expect_err("a window shorter than the crossing admits no phase at all");
        assert_eq!(err.code, DW_TIMED_GATE_COIN_FLIP); // DW0378
    }

    /// The window arithmetic is the one `DW0378` uses: a 2-move doorway inside a
    /// 10-open / 190-shut gate admits 3 of 200 phases, the figure its own test
    /// states.
    #[test]
    fn timing_read_is_the_gate_window_arithmetic() {
        let r = timing_read(2, 10, 190);
        assert_eq!(
            (r.cross_ticks, r.admits, r.cycle, r.percent),
            (8, 3, 200, 1)
        );
        assert!(r.is_coin_flip());
        let v = timing_read(3, 15, 0);
        assert_eq!(
            (v.cross_ticks, v.admits, v.cycle, v.percent),
            (12, 4, 15, 26)
        );
        assert!(!v.is_coin_flip());
    }
}
