/// A resolved stealth zone `(anchor name, centre cell, half-extents)`.
type ZoneCell = (String, [i32; 3], [u32; 3]);
/// A stealth beat probe for [`verify_stealth`]: `(zones, firing step)`.
type StealthProbe = (Vec<ZoneCell>, usize);

/// Prove every `begin-stealth` zone is standable and reachable from the beat that
/// activates it (DSL v0.6, spec-0014) — [`DW_STEALTH_ZONE`] (`DW0327`). A zone the
/// player can never legally occupy (walled/void) or can never walk to from the
/// activating position is a guaranteed unwinnable stealth beat.
pub fn check_stealth_zones(plan: &Plan, world: &World) -> Result<(), Failure> {
    let beats: Vec<StealthProbe> = plan
        .stealth_beats
        .iter()
        .map(|b| (b.zones.clone(), b.fire_step))
        .collect();
    verify_stealth(world, &beats, &critical_positions(plan))
}

/// The pure core of [`check_stealth_zones`] (unit-testable against a synthetic
/// [`World`]). Each beat is `(zones, fire_step)`; each zone `(name, centre,
/// half-extents)`.
fn verify_stealth(
    world: &World,
    beats: &[StealthProbe],
    positions: &[VisitedPos],
) -> Result<(), Failure> {
    for (zones, fire_step) in beats {
        // The player's position at the activating beat: the visited position at the
        // firing step, else the nearest earlier one, else the first position.
        let player_pos = positions
            .iter()
            .filter(|p| p.src_step <= *fire_step)
            .max_by_key(|p| p.src_step)
            .or_else(|| positions.first())
            .map(|p| p.pos);
        for (name, pos, extent) in zones {
            let lo = [
                pos[0] - extent[0] as i32,
                pos[1] - extent[1] as i32,
                pos[2] - extent[2] as i32,
            ];
            let hi = [
                pos[0] + extent[0] as i32,
                pos[1] + extent[1] as i32,
                pos[2] + extent[2] as i32,
            ];
            // EVERY standable cell of the zone box, not just the one nearest its
            // centre. A zone whose centre snaps to a standable cell in a
            // walled-off pocket while the rest of the box is perfectly reachable
            // used to raise a spurious `DW0327`; the obligation is "the player can
            // reach *somewhere* in this zone", so the proof is reachable-any.
            let stands: Vec<[i32; 3]> = crate::compiler::assembled::region_cells(lo, hi)
                .filter(|c| world.is_standable(*c))
                .collect();
            if stands.is_empty() {
                return Err(Failure {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (box {lo:?}..{hi:?}) has no standable cell — a \
                         player can never legally hide there, so the beat is unwinnable. Place the \
                         zone over reachable floor, or widen its `extent` to include a standable \
                         cell."
                    ),
                });
            }
            // One reachability flood from the player's position (not one A* per zone
            // cell): the question is set membership, not a route.
            let start = player_pos.and_then(|p| world.snap_standable(p, SNAP_RADIUS));
            if let Some(start) = start
                && {
                    let reachable = world.reachable_walkable(&[start]);
                    !stands.iter().any(|s| reachable.contains(s))
                }
            {
                return Err(Failure {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (box {lo:?}..{hi:?}, {n} standable cell(s)) is not \
                         reachable from the player's position {:?} when the stealth beat begins — \
                         NO cell of the zone is walkable from there, so the player would be caught \
                         before ever reaching cover. Route the zone within walkable reach of the \
                         activating beat, or move where the beat starts.",
                        player_pos.unwrap(),
                        n = stands.len(),
                    ),
                });
            }
        }
    }
    Ok(())
}
/// Extra ticks charged for each one-block step **up** on the flee route — the jump
/// arc a player must complete to gain the block. Conservative: a vanilla jump apex
/// is ~6 ticks.
const CLIMB_TICKS: u32 = 6;
/// Ticks charged before the player is under way at all: the beat arms while they
/// are standing still, mid-interaction, reading the narration that tells them to
/// run. 10 ticks = 0.5 s of orientation. This is the "fair warning" allowance —
/// without it the proof would assume a player already sprinting toward cover at
/// the instant the session arms.
const ONSET_REACTION_TICKS: u32 = 10;

/// A start position the onset proof must clear, with the label its diagnostic uses.
type OnsetStart = (String, [i32; 3]);

/// Prove every **punishing** `begin-stealth` beat is escapable at onset (DSL v0.6,
/// spec-0014 + spec-0016) — [`DW_STEALTH_ONSET`] (`DW0355`).
///
/// DW0327 already proves each zone is standable and connected to the beat. That is
/// not enough: `begin-stealth` arms *instantly*, the judge starts counting on the
/// very next tick, and `on_caught` fires `grace_ticks` later wherever the player
/// happens to be. So the real obligation is a **timing** one — from every position
/// a player can legally occupy when the session arms, some zone must be reachable
/// within the grace window at sprint speed:
///
/// - the **activating position** — the anchor of the objective whose completion
///   fires the beat (where the player provably is, since completing it is what
///   armed the session), and
/// - every **respawn position** — each `set-checkpoint` reigning at some step in
///   the beat's active window `[fire_step, end_step]`. A caught player respawns
///   there with the session still running and the grace clock restarted; if that
///   cell cannot beat the window either, the beat is an infinite death loop rather
///   than a souls retry.
///
/// Routes are measured over the same per-leg geometry DW0311/DW0315 use (gates
/// causally sealed by the beat's firing step forced solid), costed at
/// [`SPRINT_TICKS_PER_BLOCK`] per block plus [`CLIMB_TICKS`] per block climbed, and
/// charged [`ONSET_REACTION_TICKS`] of standing-start reaction.
///
/// Scope: beats whose `on_caught` actually punishes ([`StealthBeat::is_punishing`]
/// — `damage-players` or `spawn-wave`, at any nesting depth). A beat that only
/// narrates when spotted has nothing to escape, so no timing obligation exists.
pub fn check_stealth_onset(plan: &Plan, world: &World) -> Result<(), Failure> {
    let positions = critical_positions(plan);
    for beat in &plan.stealth_beats {
        if !beat.is_punishing() {
            continue;
        }
        let mut starts: Vec<OnsetStart> = Vec::new();
        // 1. Where the player stands when the beat arms: the visited position at
        //    the firing step, else the nearest earlier one, else the first.
        if let Some(p) = positions
            .iter()
            .filter(|p| p.src_step <= beat.fire_step)
            .max_by_key(|p| p.src_step)
            .or_else(|| positions.first())
        {
            starts.push((
                format!("the activating objective's anchor {:?}", p.pos),
                p.pos,
            ));
        }
        // 2. Every checkpoint that can drop a player into the running session: the
        //    one reigning when the beat arms (latest fire_step ≤ fire_step, ties
        //    broken by content index — a `set-checkpoint` listed beside the
        //    `begin-stealth` in the same objective's effects is the reigning one),
        //    plus every checkpoint set later while the beat is still active.
        let end = beat.end_step.unwrap_or(usize::MAX);
        if let Some(reigning) = plan
            .checkpoints
            .iter()
            .filter(|c| c.fire_step <= beat.fire_step)
            .max_by_key(|c| (c.fire_step, c.index))
        {
            starts.push((
                format!(
                    "checkpoint `{}` respawn {:?}",
                    reigning.anchor, reigning.pos
                ),
                reigning.pos,
            ));
        }
        for c in plan
            .checkpoints
            .iter()
            .filter(|c| c.fire_step > beat.fire_step && c.fire_step <= end)
        {
            starts.push((
                format!("checkpoint `{}` respawn {:?}", c.anchor, c.pos),
                c.pos,
            ));
        }
        let st = world.region_state_at(&plan.region_events, beat.fire_step, &|g, s| {
            plan.gate_fired_before(g, s)
        });
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        verify_stealth_onset(
            leg_world,
            &beat.zones,
            beat.grace_ticks,
            &starts,
            beat.index,
        )?;
    }
    Ok(())
}

/// The pure core of [`check_stealth_onset`] (unit-testable against a synthetic
/// [`World`]): every start must reach some zone cell within `grace_ticks`.
fn verify_stealth_onset(
    world: &World,
    zones: &[ZoneCell],
    grace_ticks: u32,
    starts: &[OnsetStart],
    beat_index: usize,
) -> Result<(), Failure> {
    // The budget the flee route itself gets, after the standing-start allowance.
    let budget = grace_ticks.saturating_sub(ONSET_REACTION_TICKS);
    for (label, raw) in starts {
        let Some(start) = world.snap_standable(*raw, SNAP_RADIUS) else {
            continue; // unsnappable start — DW0311/DW0315/DW0316's concern, not ours
        };
        let Some((cost, cell, zone)) = nearest_zone_by_flee_time(world, zones, start, grace_ticks)
        else {
            continue; // no zone reachable at all within the search cap — DW0327's concern
        };
        if cost <= budget {
            continue;
        }
        let need = cost + ONSET_REACTION_TICKS;
        let deficit = need - grace_ticks;
        return Err(Failure {
            code: DW_STEALTH_ONSET,
            message: format!(
                "stealth beat #{beat_index}: a player cannot beat the grace window from {label}. \
                 The nearest zone cell is `{zone}` {cell:?} — {cost} ticks of sprinting away \
                 (model: {SPRINT_TICKS_PER_BLOCK} t/block, +{CLIMB_TICKS} t per block climbed) \
                 plus {ONSET_REACTION_TICKS} ticks of standing-start reaction = {need} ticks, \
                 against `grace_ticks` {grace_ticks} — short by {deficit} ticks. The beat's \
                 `on_caught` punishes, so EVERY player dies here a fixed moment after it arms, \
                 and if this start is a checkpoint the retry loop never terminates. Fix the \
                 BEAT, not the proof: raise `grace_ticks` to at least {need} (the measured \
                 sprint time plus reaction) and add a tension margin, put a zone within reach \
                 of where the beat actually starts, move the checkpoint into/beside a zone, or \
                 arm the beat from a less exposed objective. Note that merely DELAYING the arm \
                 (a `sequence` step) does not discharge this: the clock still starts with the \
                 player free to be standing right here, so the grace window itself must cover \
                 the flee. Do NOT delete the `on_caught` consequence to silence this."
            ),
        });
    }
    Ok(())
}

/// The cheapest zone cell by **flee time** from `start`: a deterministic
/// tick-weighted Dijkstra over standable cells (cardinal steps cost
/// [`SPRINT_TICKS_PER_BLOCK`], a step up additionally costs [`CLIMB_TICKS`]),
/// stopping at the first settled cell inside any zone box. Returns
/// `(ticks, cell, zone name)`.
///
/// The search is capped well past the grace window so a failing beat can still
/// report a real number; beyond the cap the beat is failing by so much that the
/// exact figure carries no extra information. Determinism (ADR-0006): the frontier
/// is ordered by `(cost, cell)` and neighbours expand in `neighbors`' fixed order.
fn nearest_zone_by_flee_time(
    world: &World,
    zones: &[ZoneCell],
    start: [i32; 3],
    grace_ticks: u32,
) -> Option<(u32, [i32; 3], String)> {
    let boxes: Vec<(&str, [i32; 3], [i32; 3])> = zones
        .iter()
        .map(|(name, pos, extent)| {
            let lo = [
                pos[0] - extent[0] as i32,
                pos[1] - extent[1] as i32,
                pos[2] - extent[2] as i32,
            ];
            let hi = [
                pos[0] + extent[0] as i32,
                pos[1] + extent[1] as i32,
                pos[2] + extent[2] as i32,
            ];
            (name.as_str(), lo, hi)
        })
        .collect();
    let zone_of = |c: [i32; 3]| {
        boxes
            .iter()
            .find(|(_, lo, hi)| (0..3).all(|k| lo[k] <= c[k] && c[k] <= hi[k]))
            .map(|(n, _, _)| *n)
    };
    let cap = grace_ticks.saturating_mul(4).saturating_add(400);
    let mut best: BTreeMap<[i32; 3], u32> = BTreeMap::new();
    let mut open: BinaryHeap<Reverse<(u32, [i32; 3])>> = BinaryHeap::new();
    best.insert(start, 0);
    open.push(Reverse((0, start)));
    while let Some(Reverse((cost, cur))) = open.pop() {
        if cost > *best.get(&cur).unwrap_or(&u32::MAX) {
            continue; // stale heap entry
        }
        if let Some(name) = zone_of(cur) {
            return Some((cost, cur, name.to_string()));
        }
        if cost >= cap {
            continue;
        }
        for n in world.neighbors(cur) {
            let climb = if n[1] > cur[1] { CLIMB_TICKS } else { 0 };
            let next = cost + SPRINT_TICKS_PER_BLOCK + climb;
            if next < *best.get(&n).unwrap_or(&u32::MAX) {
                best.insert(n, next);
                open.push(Reverse((next, n)));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {

    #[test]
    fn stealth_zone_over_void_is_dw0327() {
        // A zero-extent zone centred on a void cell → no standable cell → DW0327.
        let world = floored(5, 3, 65, &[]);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [20, 65, 20], [0, 0, 0])],
            0usize,
        )];
        let err = verify_stealth(&world, &beats, &[at_step([2, 65, 1], 0)]).unwrap_err();
        assert_eq!(err.code, DW_STEALTH_ZONE); // DW0327
    }

    #[test]
    fn stealth_zone_unreachable_from_beat_is_dw0327() {
        // The zone is standable on the far patch, but the activating beat sits on
        // the near patch across a void gap → unreachable → DW0327.
        let world = split_world(65);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 1, 1])],
            0usize,
        )];
        let err = verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).unwrap_err();
        assert_eq!(err.code, DW_STEALTH_ZONE); // DW0327
    }

    #[test]
    fn stealth_zone_standable_and_reachable_passes() {
        let world = floored(6, 3, 65, &[]);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 1, 1])],
            0usize,
        )];
        assert!(verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).is_ok());
    }

    // --- DW0355: stealth onset survivability --------------------------------

    /// The island defect in miniature: cover EXISTS and is reachable (DW0327 is
    /// happy) but is too far to reach inside the grace window, so the beat kills
    /// every player a fixed moment after it arms.
    #[test]
    fn stealth_zone_out_of_sprint_range_at_onset_is_dw0352() {
        // A 40-long corridor; the beat arms at x=0, the only zone sits at x=39.
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [39, 65, 1], [1, 1, 1])];
        // Reachability alone passes — this is exactly the gap DW0355 closes.
        assert!(
            verify_stealth(&world, &[(zones.clone(), 0)], &[at_step([0, 65, 1], 0)]).is_ok(),
            "DW0327 must be satisfied, so the failure below is purely a timing one"
        );
        let starts = vec![("the activating objective's anchor".to_string(), [0, 65, 1])];
        let err = verify_stealth_onset(&world, &zones, 50, &starts, 1)
            .expect_err("cover 38 blocks away cannot be reached in 50 ticks");
        assert_eq!(err.code, DW_STEALTH_ONSET); // DW0355
        assert!(
            err.message.contains("zone/alcove") && err.message.contains("short by"),
            "the diagnostic names the nearest zone and the tick deficit: {}",
            err.message
        );
        // The deficit is measured, not guessed: 38 blocks × 4 t + 10 t reaction.
        assert!(
            err.message.contains("152 ticks of sprinting"),
            "the sprint cost is the nav-model measurement: {}",
            err.message
        );
    }

    /// A checkpoint that respawns the party into a running punishing beat is a
    /// start position too — if IT cannot beat the window, the retry loop never
    /// terminates (a broken beat, not a souls retry).
    #[test]
    fn checkpoint_respawning_into_a_running_beat_must_beat_the_window_dw0352() {
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [2, 65, 1], [1, 1, 1])];
        // The activating anchor is next to cover; the respawn point is not.
        let starts = vec![
            ("the activating objective's anchor".to_string(), [0, 65, 1]),
            ("checkpoint `cp/below` respawn".to_string(), [39, 65, 1]),
        ];
        let err = verify_stealth_onset(&world, &zones, 50, &starts, 1)
            .expect_err("a respawn point outside sprint range of cover is a death loop");
        assert_eq!(err.code, DW_STEALTH_ONSET);
        assert!(
            err.message.contains("cp/below"),
            "the diagnostic names the offending checkpoint: {}",
            err.message
        );
    }

    /// A climb on the flee route is charged its jump arc: the same horizontal
    /// distance costs more when cover is up a step, which is what made the
    /// island's ramp-top zone unreachable in time.
    #[test]
    fn stealth_onset_charges_the_climb() {
        // Floor at y=65 with a step up to y=66 at x=5..7 (a 2-block rise).
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..3 {
                solid.insert([x, 64, z]);
            }
        }
        for x in 5..8 {
            for z in 0..3 {
                solid.insert([x, 65, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let zones = vec![("zone/ledge".to_string(), [7, 66, 1], [0, 0, 0])];
        let starts = vec![("the activating objective's anchor".to_string(), [0, 65, 1])];
        // 7 horizontal steps (28 t) + one +1 climb (6 t) = 34 t, +10 reaction = 44.
        let err = verify_stealth_onset(&world, &zones, 40, &starts, 1)
            .expect_err("34 t of flee + 10 t reaction exceeds a 40-tick window");
        assert_eq!(err.code, DW_STEALTH_ONSET);
        assert!(
            err.message.contains("34 ticks of sprinting"),
            "the climb is charged its jump arc: {}",
            err.message
        );
        // Widening the window to the measured need discharges the obligation.
        assert!(
            verify_stealth_onset(&world, &zones, 44, &starts, 1).is_ok(),
            "grace sized to the measured need must pass"
        );
    }

    /// Cover inside the window passes — the green half of the story.
    #[test]
    fn stealth_onset_within_the_grace_window_passes() {
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [5, 65, 1], [1, 1, 1])];
        let starts = vec![
            ("the activating objective's anchor".to_string(), [0, 65, 1]),
            ("checkpoint `cp/below` respawn".to_string(), [8, 65, 1]),
        ];
        // 4 blocks to the zone edge = 16 t (+10) from the anchor; 4 t (+10) from cp.
        assert!(verify_stealth_onset(&world, &zones, 30, &starts, 1).is_ok());
    }

    /// The obligation is scoped to beats that actually punish: a `begin-stealth`
    /// whose `on_caught` only narrates has nothing to escape, so unreachable-in-time
    /// cover is atmosphere, not a broken beat.
    #[test]
    fn a_stealth_beat_that_only_narrates_is_not_punishing() {
        use delvewright_dsl::QuestEffect;
        let beat = |on_caught: Vec<QuestEffect>| crate::compiler::plan::StealthBeat {
            index: 1,
            zones: vec![("zone/alcove".to_string(), [0, 65, 0], [1, 1, 1])],
            on_caught,
            grace_ticks: 20,
            fire_step: 0,
            end_step: None,
        };
        assert!(
            !beat(vec![
                Verb::Narrate {
                    text: "Spotted!".to_string(),
                    style: None,
                    sound: None,
                }
                .into(),
            ])
            .is_punishing(),
            "a narrate-only on_caught carries no timing obligation"
        );
        assert!(
            beat(vec![
                Verb::DamagePlayers {
                    amount: 40,
                    damage_type: None,
                }
                .into(),
            ])
            .is_punishing(),
            "damage-players makes the beat punishing"
        );
    }

    // --- stealth zones: reachable-ANY, not reachable-centre --------

    #[test]
    fn stealth_zone_passes_when_any_zone_cell_is_reachable() {
        // The zone box straddles a wall: its CENTRE snaps to a standable cell in a
        // walled-off pocket, while the rest of the box is plainly reachable. The
        // obligation is "the player can reach cover somewhere in this zone", so
        // this must pass — testing only the snapped centre raised a spurious
        // DW0327.
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..7 {
            for z in 0..3 {
                cells.push(([x, 64, z], "minecraft:stone")); // floor
            }
        }
        // A full-height wall at x=4 sealing off the x=5 pocket from the walkway,
        // with a 2-high stack so nothing can be jumped over.
        for z in 0..3 {
            for y in 65..=66 {
                cells.push(([4, y, z], "minecraft:stone"));
                cells.push(([5, y, z], "minecraft:stone"));
            }
        }
        // Reopen the pocket floor at [5,65,1] by removing the wall cell there.
        cells.retain(|(c, _)| *c != [5, 65, 1] && *c != [5, 66, 1]);
        let world = blocks_world(&cells);
        // Zone box x 3..=5 at y=65: [5,65,1] (the pocket) and [3,65,z] (the open
        // walkway) are both standable, but only the walkway is reachable.
        assert!(world.is_standable([5, 65, 1]), "the pocket is standable");
        assert!(world.is_standable([3, 65, 1]), "the walkway is standable");
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 0, 1])],
            0usize,
        )];
        assert!(
            verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).is_ok(),
            "a zone with ANY reachable standable cell must pass"
        );
    }
}
