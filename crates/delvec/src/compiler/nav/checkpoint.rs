//! The checkpoint proofs: a checkpoint never strands the party and stands on
//! floor (`DW0315`, `DW0316`), and the walk back from a rest point is costed
//! (`DW0379`).

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, RegionEvents};
use delvewright_dsl::Diagnostic;
use std::collections::BTreeSet;

/// Prove no `set-checkpoint` strands the party (DSL v0.6, spec-0012). Two
/// obligations, per checkpoint:
///
/// 1. **Placement** ([`DW_CHECKPOINT_UNSTANDABLE`], `DW0316`): the checkpoint
///    anchor must have a standable floor cell within [`SNAP_RADIUS`] on the final
///    assembled model, or the party respawns into void / a wall. (Because the
///    relight pass — which runs before nav — proves every reachable walkable cell
///    meets the area's `min_light`, a standable, reachable checkpoint cell
///    provably meets `min_light` too; no separate light probe is needed.)
/// 2. **No stranding** ([`DW_CHECKPOINT_STRANDED`], `DW0315`, the core proof):
///    the DW0311 reachability, re-rooted at the checkpoint cell, must still reach
///    the remaining critical path. Since consecutive walked legs are already
///    proven forward-walkable, it suffices to reach the FIRST walked critical
///    position that fires after the checkpoint — reconnecting the whole forward
///    path. The message names the checkpoint and that first unreachable anchor,
///    and prescribes moving the checkpoint or adding a return route (never
///    deleting the checkpoint to silence the proof).
pub fn check_checkpoints(plan: &Plan, world: &World) -> Result<(), Failure> {
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    // A checkpoint a trigger sets is rooted where the party can first reach
    // the trigger (spec-0093 §6.2), not at the entry; the configurations are
    // gathered once and only when some checkpoint needs them.
    let mut configs: Option<Vec<TriggerRootConfig>> = None;
    let mut cps: Vec<(String, [i32; 3], usize)> = Vec::new();
    for c in &plan.checkpoints {
        let mut fire_step = c.fire_step;
        if let Some(trigger) = &c.trigger {
            let configs = configs.get_or_insert_with(|| trigger_root_configs(plan, world));
            if let Some(root) = trigger_root_step(plan, world, configs, trigger) {
                fire_step = fire_step.max(root.step);
                eprintln!(
                    "DW0315: checkpoint `{}` is set by `{trigger}`, which the party can first \
                     reach {} (root step {}, over {} configuration(s))",
                    c.anchor,
                    root.when,
                    root.step,
                    configs.len()
                );
            }
        }
        cps.push((c.anchor.clone(), c.pos, fire_step));
    }
    verify_checkpoints(
        world,
        &cps,
        &fixed_checkpoint_fires(plan),
        &critical_positions(plan),
        &plan.region_events,
        &ancestor,
    )
}

/// The critical-path steps at which a checkpoint is set **once, by a beat** — a
/// plain `set-checkpoint` no trigger sets. A bonfire moves only when the party
/// rests and a trigger's checkpoint whenever its trigger is pressed, so neither
/// is known to have replaced an earlier seat at any step; only these are.
/// One answer for the no-stranding proof (`DW0315`) and the stake proof
/// (`DW0525`), which both ask when a seat stops being in force.
pub(crate) fn fixed_checkpoint_fires(plan: &Plan) -> Vec<usize> {
    plan.checkpoints
        .iter()
        .filter(|c| c.trigger.is_none() && !c.rest)
        .map(|c| c.fire_step)
        .collect()
}

/// Every critical-path position as `(src_step, cell, carried_in)`, where
/// `carried_in` marks a position the party is put down at by an inter-area
/// crossing (not a link, not a loop). For the stake proof, which asks which
/// areas the party can stand in while a seat is in force.
pub(crate) fn route_positions(plan: &Plan) -> Vec<(usize, [i32; 3], bool)> {
    critical_positions(plan)
        .into_iter()
        .map(|p| {
            (
                p.src_step,
                p.pos,
                p.transport_before && !p.by_link && !p.by_loop,
            )
        })
        .collect()
}

/// One quest configuration as the trigger-root derivation reads it: the world
/// with that configuration's region writes applied, the first critical step
/// arriving under it, and the route cells of every leg that does.
type TriggerRootConfig = (Option<World>, usize, Vec<[i32; 3]>);

/// The configurations `DW0921` judges, gathered for the trigger-root derivation:
/// the critical route's cells grouped by the region state of their arrival step,
/// each with the first step arriving under it, in order of that first step.
fn trigger_root_configs(plan: &Plan, world: &World) -> Vec<TriggerRootConfig> {
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut seeds: Vec<(RegionState, usize, BTreeSet<[i32; 3]>)> = Vec::new();
    for (step, cells) in critical_route_cells(plan, world) {
        let st = world.region_state_at(&plan.region_events, step, &ancestor);
        match seeds.iter_mut().find(|(s, _, _)| *s == st) {
            Some((_, first, set)) => {
                *first = (*first).min(step);
                set.extend(cells);
            }
            None => seeds.push((st, step, cells.into_iter().collect())),
        }
    }
    seeds.sort_by_key(|(_, first, _)| *first);
    seeds
        .into_iter()
        .map(|(st, first, cells)| {
            let w = (!st.is_empty()).then(|| world.with_region_state(&st));
            (w, first, cells.into_iter().collect())
        })
        .collect()
}

/// How far from a `step` trigger's cell a standing body can fire it: a body's
/// hitbox reaches 0.3 past its own cell, so a body standing in any of the eight
/// horizontal neighbours can lean into the plate's cell, and the farthest of
/// them (a corner) is sqrt(2) away. Rooting from more cells roots no later.
const STEP_REACH: f64 = 1.5;

/// Where a trigger-set checkpoint is rooted (spec-0093 §6.2).
struct TriggerRoot {
    /// The critical step the no-stranding proof roots at.
    step: usize,
    /// The derivation in words, for the binding line and the message.
    when: String,
}

/// **The earliest critical step at which the party can fire `trigger`**: the
/// step the path performs it at when the path performs it; else `first − 1` for
/// the earliest configuration whose walkable flood from its route cells meets a
/// cell the trigger fires from; else `None` — no configuration reaches it, and
/// the caller keeps the entry.
///
/// A trigger fires from every standable cell within its `range` of its anchor
/// (`approach`), or within [`crate::compiler::crosshair::INTERACTION_REACH`] of
/// the anchor, the struck NPC's or the struck assembly's cell (`use`, `strike`,
/// `strike-npc`, `strike-assembly`), or within [`STEP_REACH`] of a plate's cell
/// (`step`). Flooding from the union of a
/// configuration's route cells can only root EARLIER than flooding from each
/// step's own cells, which is the conservative direction: a root too early asks
/// the checkpoint to re-reach more of the path, never less.
fn trigger_root_step(
    plan: &Plan,
    world: &World,
    configs: &[TriggerRootConfig],
    trigger: &str,
) -> Option<TriggerRoot> {
    // Performed by the path: rooted at its step, as the region model roots the
    // trigger's own writes.
    if let Some(step) = plan
        .critical_path
        .iter()
        .position(|s| s.trigger() == Some(trigger))
    {
        return Some(TriggerRoot {
            step,
            when: format!("at the path's own `trigger` step #{}", step + 1),
        });
    }
    let c = plan.campaign;
    let t = c
        .quests
        .content
        .triggers
        .iter()
        .find(|t| t.id.as_str() == trigger)?;
    let (centre, radius) = match &t.on {
        delvewright_dsl::TriggerOn::Approach { range } => {
            (plan.point_any(t.at_anchor()?)?, f64::from(*range))
        }
        delvewright_dsl::TriggerOn::Strike | delvewright_dsl::TriggerOn::Use => (
            plan.point_any(t.at_anchor()?)?,
            crate::compiler::crosshair::INTERACTION_REACH,
        ),
        // A step fires on a body whose hitbox is in the plate's cell (the
        // `dx=0` box of `emit::step_cell_box`): from the cell itself, or from a
        // horizontal neighbour, edge or corner, whose body leans into it.
        delvewright_dsl::TriggerOn::Step => (plan.point_any(t.at_anchor()?)?, STEP_REACH),
        delvewright_dsl::TriggerOn::StrikeNpc { npc } => {
            let n = c
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.id.as_str() == npc.as_str())?;
            (
                plan.body_point(delvewright_dsl::BodyRef::Npc(n))?,
                crate::compiler::crosshair::INTERACTION_REACH,
            )
        }
        delvewright_dsl::TriggerOn::StrikeAssembly { assembly } => {
            let decl = c.quests.content.assembly_decl(assembly.as_str())?;
            let a = plan.point_any(decl.at.anchor.as_str())?;
            (
                decl.at.cell(a),
                crate::compiler::crosshair::INTERACTION_REACH,
            )
        }
    };
    let r = radius.ceil() as i32;
    let r2 = radius * radius;
    let mut fire_cells: BTreeSet<[i32; 3]> = BTreeSet::new();
    for dx in -r..=r {
        for dy in -r..=r {
            for dz in -r..=r {
                let cell = [centre[0] + dx, centre[1] + dy, centre[2] + dz];
                let d2 = f64::from(dx * dx + dy * dy + dz * dz);
                if d2 <= r2 && world.is_standable(cell) {
                    fire_cells.insert(cell);
                }
            }
        }
    }
    if fire_cells.is_empty() {
        return None;
    }
    let first = earliest_reaching_config(configs, world, &fire_cells)?;
    let step = first.saturating_sub(1);
    let next = plan
        .critical_path
        .get(first)
        .and_then(|s| s.objective())
        .map(|o| format!("while `{o}` is next"))
        .unwrap_or_else(|| format!("from critical step {first}"));
    Some(TriggerRoot {
        step,
        when: format!(
            "{next} ({} firing cell(s) within {radius} of {centre:?})",
            fire_cells.len()
        ),
    })
}

/// The pure core of [`trigger_root_step`]: the `first` step of the earliest
/// configuration (in `first` order) whose walkable flood from its route cells,
/// over its own world, meets one of `fire_cells`; `None` when no configuration
/// reaches them. Split out so it is unit-testable over synthetic [`World`]s.
fn earliest_reaching_config(
    configs: &[TriggerRootConfig],
    world: &World,
    fire_cells: &BTreeSet<[i32; 3]>,
) -> Option<usize> {
    configs.iter().find_map(|(w, first, cells)| {
        let reached = w.as_ref().unwrap_or(world).reachable_walkable(cells);
        reached
            .iter()
            .any(|c| fire_cells.contains(c))
            .then_some(*first)
    })
}

/// The pure core of [`check_checkpoints`] (split out so it is unit-testable
/// against a synthetic [`World`] without a full [`Plan`]). Each checkpoint is
/// `(anchor, cell, fire_step)`.
fn verify_checkpoints(
    world: &World,
    checkpoints: &[(String, [i32; 3], usize)],
    fixed_fires: &[usize],
    positions: &[VisitedPos],
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), Failure> {
    for (anchor, pos, fire_step) in checkpoints {
        let Some(cell) = world.snap_standable(*pos, SNAP_RADIUS) else {
            return Err(Failure {
                code: DW_CHECKPOINT_UNSTANDABLE,
                message: format!(
                    "checkpoint anchor `{anchor}` at {pos:?} has no standable floor within \
                     {SNAP_RADIUS} blocks on the assembled model — the party would respawn into \
                     void or a wall. Move the checkpoint onto reachable floor (not a trap-trigger, \
                     hazard, or mid-air cell); if the prefab looks correct, this is an \
                     assembly/toolchain defect — escalate rather than hide it."
                ),
            });
        };
        // The first walked critical position strictly after the checkpoint fires.
        let Some(target) = positions
            .iter()
            .filter(|p| p.src_step > *fire_step && !p.transport_before)
            .min_by_key(|p| p.src_step)
        else {
            continue; // nothing left to walk to (checkpoint at/near the finale)
        };
        // Replaced before that leg is walked: a checkpoint a beat sets
        // ([`fixed_checkpoint_fires`]) fires after this one and before the
        // target, so a death on the leg respawns there, not here. This is how a
        // route that crosses into another area (a one-way carry) and sets a
        // checkpoint on arrival owes nothing to the seat it left behind.
        if fixed_fires
            .iter()
            .any(|f| *f > *fire_step && *f < target.src_step)
        {
            continue;
        }
        // Seal any gate closed by the time the party reaches the target (the same
        // per-leg gate state DW0311 routes under), so a checkpoint whose forward
        // path is walled off by a `close-gate` strands the party (DSL v0.6).
        let st = world.region_state_at(region_events, target.src_step, ancestor);
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        let Some(goal) = leg_world.snap_endpoint(target.pos, target.talk_to) else {
            continue; // the target itself is unsnappable → a DW0311 concern, not ours
        };
        if leg_world.find_path(cell, goal).is_none() {
            return Err(Failure {
                code: DW_CHECKPOINT_STRANDED,
                message: format!(
                    "checkpoint `{anchor}` (cell {cell:?}) strands the party: the next required \
                     anchor {:?} (critical step {}) is not walkable from it over the assembled \
                     geometry (a checkpoint behind a one-way drop the forward path can't re-cross \
                     after respawn). The proof is rooted at step {fire_step}: a beat's checkpoint \
                     at the beat, a trigger's at the earliest step the party can reach the trigger \
                     (the `DW0315:` line above says which), and that is the first required anchor \
                     after it. Move the checkpoint to a cell that keeps the remaining path \
                     reachable, or add a return route back up — do NOT delete the checkpoint to \
                     silence this proof.",
                    target.pos, target.src_step
                ),
            });
        }
    }
    Ok(())
}

/// The retry-cost budget (spec-0016 §7): 60 s of traversal from a rest point to
/// the point of failure it respawns the party into, in ticks.
const RETRY_BUDGET_TICKS: u32 = 60 * 20;

/// `DW0379`: the walk back from each rest point to the DEEPEST beat it governs.
pub(in crate::compiler::nav) fn retry_cost_lint(plan: &Plan, world: &World) -> Vec<Diagnostic> {
    let cps: Vec<(String, [i32; 3], usize, bool)> = plan
        .checkpoints
        .iter()
        .map(|c| (c.anchor.clone(), c.pos, c.fire_step, c.rest))
        .collect();
    verify_retry_cost(world, &cps, &critical_positions(plan))
}

/// A rest point as the retry-cost lint sees it.
struct RestRef<'a> {
    anchor: &'a str,
    pos: [i32; 3],
    fire_step: usize,
    rest: bool,
}

/// The pure core of [`retry_cost_lint`] (unit-testable against a synthetic
/// [`World`]). Each rest point is `(anchor, cell, fire_step, is_bonfire)`.
///
/// **The quantity is the walk to the DEEPEST beat the rest point governs**, not
/// to the first one after it. A party does not die at the nearest thing a fire
/// is the checkpoint for — it dies anywhere in the stretch that fire governs,
/// and the walk it complains about is the longest of them. Measured to the
/// first beat, the lint reports the best case and calls it the retry cost: on
/// vesperhold the Watch Fire's first beat is 12 blocks and the deepest beat it
/// governs is 146, the Tower Fire's are 53 and 211, and no value of
/// [`RETRY_BUDGET_TICKS`] separates the walk a human reported from rest points
/// nobody has complained about (`docs/reference/retry-cost-measurement.md`).
///
/// A rest point **governs** every beat from its own firing step up to and
/// including the firing step of the next rest point — which is the span over
/// which vanilla returns a dead party to this spawn point, since the next rest
/// point is not armed until its own step completes. The last rest point governs
/// everything after it.
///
/// A beat inside that span that does not snap or does not route is skipped, not
/// fatal: `DW0315`/`DW0316` own an unreachable beat, and dropping it only
/// lowers this maximum.
fn verify_retry_cost(
    world: &World,
    rests: &[(String, [i32; 3], usize, bool)],
    positions: &[VisitedPos],
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (anchor, pos, fire_step, is_rest) in rests {
        let cp = RestRef {
            anchor,
            pos: *pos,
            fire_step: *fire_step,
            rest: *is_rest,
        };
        let Some(from) = world.snap_standable(cp.pos, SNAP_RADIUS) else {
            continue; // DW0316 owns an unstandable rest point
        };
        // The step the next rest point arms at. Beats at or before it are still
        // this rest point's to answer for; anything past it belongs to that one.
        let governs_through = rests
            .iter()
            .map(|(_, _, f, _)| *f)
            .filter(|f| *f > cp.fire_step)
            .min()
            .unwrap_or(usize::MAX);
        let mut deepest: Option<(usize, u32)> = None;
        let mut governed = 0usize;
        let mut walked = 0usize;
        for target in positions
            .iter()
            .filter(|p| p.src_step > cp.fire_step && p.src_step <= governs_through)
            .filter(|p| !p.transport_before)
        {
            governed += 1;
            let Some(goal) = world.snap_endpoint(target.pos, target.talk_to) else {
                continue;
            };
            let Some(path) = world.find_path(from, goal) else {
                continue; // DW0315 owns an unreachable one
            };
            walked += 1;
            let blocks = path.len().saturating_sub(1) as u32;
            if deepest.is_none_or(|(_, b)| blocks > b) {
                deepest = Some((target.src_step, blocks));
            }
        }
        let Some((deepest_step, blocks)) = deepest else {
            continue;
        };
        let ticks = blocks * SPRINT_TICKS_PER_BLOCK;
        if ticks > RETRY_BUDGET_TICKS {
            out.push(Diagnostic::warning(
                DW_RETRY_COST,
                "quests",
                format!("/content/quests/checkpoint/{}", cp.anchor),
                format!(
                    "retry cost: {} `{}` is {blocks} blocks ({} s at {SPRINT_TICKS_PER_BLOCK} \
                     t/block) from the deepest beat it respawns the party into — critical-path \
                     step {deepest_step}, the furthest of {walked} walkable beat(s) of the \
                     {governed} it governs — over the {} s budget (spec-0016 §7). Dying must be \
                     an investment, not a commute: past this the loop stops teaching and starts \
                     taxing. Move the rest point forward, or add one closer to the far end of \
                     the stretch it governs.",
                    if cp.rest { "bonfire" } else { "checkpoint" },
                    cp.anchor,
                    ticks / 20,
                    RETRY_BUDGET_TICKS / 20
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;
    use std::collections::BTreeSet;

    use crate::compiler::plan::RegionEvents;
    use crate::compiler::plan::{RegionEvent, RegionWrite};

    /// A rest point 4 blocks from the next beat is a real retry loop: 16 ticks
    /// back, well inside the 60 s budget. No warning.
    #[test]
    fn a_close_rest_point_is_within_the_retry_budget() {
        let world = corridor(400, 65);
        let rests = vec![("anchor/fire".to_string(), [0, 65, 1], 0usize, true)];
        let diags = verify_retry_cost(&world, &rests, &[vp_at([4, 65, 1], 1)]);
        assert!(diags.is_empty(), "4 blocks is not a commute: {diags:#?}");
    }

    /// A rest point 350 blocks from the beat it respawns into is 70 s of walking
    /// back on every death — `DW0379`, at warning tier.
    #[test]
    fn a_distant_rest_point_is_dw0379() {
        let world = corridor(400, 65);
        let rests = vec![("anchor/fire".to_string(), [0, 65, 1], 0usize, true)];
        let diags = verify_retry_cost(&world, &rests, &[vp_at([350, 65, 1], 1)]);
        assert_eq!(diags.len(), 1, "one finding expected: {diags:#?}");
        assert_eq!(diags[0].code, DW_RETRY_COST); // DW0379
        assert_eq!(
            diags[0].severity,
            delvewright_dsl::Severity::Warning,
            "retry cost is a judgement the compiler measures but must not overrule"
        );
        assert!(
            diags[0].message.contains("bonfire `anchor/fire`"),
            "the message names the rest point: {}",
            diags[0].message
        );
    }

    /// The budget is measured to the DEEPEST beat the rest point governs, not to
    /// the first one after it. A party dies at the far end of the stretch its
    /// checkpoint answers for and walks back from there, so a rest point with its
    /// next beat two blocks away and its last beat across the delve is expensive,
    /// however cheap the first step looks. This is the defect's own shape: on the
    /// first-beat quantity the 2-block beat is the whole reading and nothing is
    /// emitted (`docs/reference/retry-cost-measurement.md`).
    #[test]
    fn retry_cost_measures_the_deepest_beat_the_rest_point_governs() {
        let world = corridor(400, 65);
        let rests = vec![("anchor/fire".to_string(), [0, 65, 1], 0usize, false)];
        let diags = verify_retry_cost(
            &world,
            &rests,
            &[vp_at([2, 65, 1], 1), vp_at([390, 65, 1], 2)],
        );
        assert_eq!(
            diags.len(),
            1,
            "the 390-block beat is this rest point's to answer for: {diags:#?}"
        );
        assert_eq!(diags[0].code, DW_RETRY_COST); // DW0379
        assert!(
            diags[0].message.contains("390 blocks")
                && diags[0]
                    .message
                    .contains("furthest of 2 walkable beat(s) of the 2 it governs")
                && diags[0].message.contains("step 2"),
            "the message reports the deepest walk, over how many beats, and which: {}",
            diags[0].message
        );
    }

    /// A beat inside the stretch that the walking model cannot route to is
    /// skipped rather than fatal — `DW0315`/`DW0316` own an unreachable beat, and
    /// dropping it only lowers this maximum. What it may not do is go unsaid: the
    /// message states how many of the governed beats were actually walked, so a
    /// maximum taken over a subset never reads as one taken over the whole.
    #[test]
    fn an_unroutable_beat_is_skipped_and_the_message_says_how_many_were_walked() {
        let world = corridor(400, 65);
        let rests = vec![("anchor/fire".to_string(), [0, 65, 1], 0usize, true)];
        let diags = verify_retry_cost(
            &world,
            &rests,
            &[vp_at([200, 65, 40], 1), vp_at([350, 65, 1], 2)],
        );
        assert_eq!(diags.len(), 1, "one finding expected: {diags:#?}");
        assert!(
            diags[0]
                .message
                .contains("furthest of 1 walkable beat(s) of the 2 it governs"),
            "the skipped beat is counted and named as skipped: {}",
            diags[0].message
        );
    }

    /// A beat past the NEXT rest point's firing step belongs to that rest point,
    /// not to this one: the party that dies there comes back at the later fire.
    /// Without this bound every rest point would be charged the whole delve.
    #[test]
    fn a_beat_past_the_next_rest_point_is_not_this_ones_to_answer_for() {
        let world = corridor(400, 65);
        let rests = vec![
            ("anchor/near".to_string(), [0, 65, 1], 0usize, true),
            ("anchor/far".to_string(), [388, 65, 1], 1usize, true),
        ];
        let diags = verify_retry_cost(
            &world,
            &rests,
            &[vp_at([2, 65, 1], 1), vp_at([390, 65, 1], 2)],
        );
        assert!(
            diags.is_empty(),
            "each fire walks only its own stretch — 2 blocks and 2 blocks: {diags:#?}"
        );
    }

    /// The beat that ARMS the next rest point is still governed by this one: the
    /// later checkpoint does not exist until its own step completes, so a party
    /// that dies reaching it respawns at the earlier fire. The span is
    /// `(fire_step, next_fire_step]`, inclusive at the far end.
    #[test]
    fn the_beat_that_arms_the_next_rest_point_is_still_this_ones() {
        let world = corridor(400, 65);
        let rests = vec![
            ("anchor/near".to_string(), [0, 65, 1], 0usize, true),
            ("anchor/far".to_string(), [350, 65, 1], 2usize, true),
        ];
        let diags = verify_retry_cost(
            &world,
            &rests,
            &[vp_at([2, 65, 1], 1), vp_at([350, 65, 1], 2)],
        );
        assert_eq!(diags.len(), 1, "one finding expected: {diags:#?}");
        assert!(
            diags[0].message.contains("bonfire `anchor/near`")
                && diags[0].message.contains("350 blocks"),
            "the walk to the arming beat is charged to the fire already lit: {}",
            diags[0].message
        );
    }

    /// A `close-gate` that walls off the forward path from a checkpoint strands the
    /// party (DW0315) — the checkpoint gate proof routes under the same per-leg seal.
    #[test]
    fn close_gate_walls_off_checkpoint_forward_path_is_dw0315() {
        let world = floored(5, 1, 65, &[]);
        // Checkpoint at the near end (fire_step 0); the next required anchor is past
        // the gate cell [2,65,0].
        let cps = vec![("cp/rest".to_string(), [0, 65, 0], 0usize)];
        let positions = vec![at_step([4, 65, 0], 1)];
        // Open gate → reachable.
        assert!(
            verify_checkpoints(
                &world,
                &cps,
                &[],
                &positions,
                &RegionEvents::default(),
                &linear
            )
            .is_ok()
        );
        // Sealed before the party reaches the target (fire_step 0 < 1) → stranded.
        let close = RegionEvent::forced(([2, 65, 0], [2, 65, 0]), RegionWrite::Fill, 0);
        let err = verify_checkpoints(
            &world,
            &cps,
            &[],
            &positions,
            &RegionEvents::from(vec![close.clone()]),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    #[test]
    fn checkpoint_behind_a_one_way_drop_is_dw0315() {
        // Checkpoint on the near patch; the next required anchor is on the far,
        // disconnected patch → not walkable from the checkpoint → DW0315.
        let world = split_world(65);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        let err = verify_checkpoints(
            &world,
            &cps,
            &[],
            &positions,
            &RegionEvents::default(),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    /// A checkpoint a beat sets after this one, and before the next leg is
    /// walked, replaces it: a party carried across to the far patch and given
    /// a checkpoint there does not owe the far patch's anchor from the near
    /// checkpoint it left. A replacement that fires only AT the target step is
    /// not before the leg, and the near checkpoint still owes it.
    #[test]
    fn a_checkpoint_replaced_before_the_next_leg_owes_it_nothing() {
        let world = split_world(65);
        let cps = vec![
            ("cp/near".to_string(), [0, 65, 1], 0usize),
            ("cp/far".to_string(), [3, 65, 1], 1usize),
        ];
        let positions = vec![at_step([4, 65, 1], 2)];
        verify_checkpoints(
            &world,
            &cps,
            &[1],
            &positions,
            &RegionEvents::default(),
            &linear,
        )
        .expect("the near checkpoint is replaced at step 1, before the leg to step 2");
        let err = verify_checkpoints(
            &world,
            &cps,
            &[2],
            &positions,
            &RegionEvents::default(),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED);
        assert!(err.message.contains("cp/near"), "{}", err.message);
    }

    /// A checkpoint a trigger sets is rooted where the party can first reach the
    /// trigger (spec-0093 §6.2). On the split floor the near patch is walked at
    /// steps 0–1 and the far patch from step 3; a trigger that fires only from the
    /// far patch is first reachable in the far configuration, so the checkpoint
    /// it sets roots at step 2 and owes only the far patch — which it reaches.
    #[test]
    fn a_trigger_checkpoint_is_rooted_where_the_trigger_is_first_reachable() {
        let world = split_world(65);
        let configs: Vec<TriggerRootConfig> = vec![
            (None, 0, vec![[0, 65, 1], [1, 65, 1]]),
            (None, 3, vec![[3, 65, 1], [4, 65, 1]]),
        ];
        let far: BTreeSet<[i32; 3]> = [[4, 65, 0]].into_iter().collect();
        let near: BTreeSet<[i32; 3]> = [[0, 65, 0]].into_iter().collect();
        assert_eq!(earliest_reaching_config(&configs, &world, &far), Some(3));
        assert_eq!(earliest_reaching_config(&configs, &world, &near), Some(0));
        let nowhere: BTreeSet<[i32; 3]> = [[9, 65, 9]].into_iter().collect();
        assert_eq!(earliest_reaching_config(&configs, &world, &nowhere), None);

        // The proof under each root: rooted at the entry (the old rule) the far
        // checkpoint strands the party — the near patch is not walkable from it;
        // rooted at step 2 it owes the far patch's own anchor and passes.
        let positions = vec![at_step([1, 65, 1], 1), at_step([4, 65, 1], 3)];
        let early = vec![("cp/far".to_string(), [3, 65, 1], 0usize)];
        let err = verify_checkpoints(
            &world,
            &early,
            &[],
            &positions,
            &RegionEvents::default(),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED);
        let rooted = vec![("cp/far".to_string(), [3, 65, 1], 2usize)];
        assert!(
            verify_checkpoints(
                &world,
                &rooted,
                &[],
                &positions,
                &RegionEvents::default(),
                &linear
            )
            .is_ok()
        );
        // And a trigger the party can reach from the entry keeps the entry root:
        // the same far checkpoint set from the near patch still strands.
        let near_first = earliest_reaching_config(&configs, &world, &near).unwrap();
        let cps = vec![(
            "cp/far".to_string(),
            [3, 65, 1],
            near_first.saturating_sub(1),
        )];
        assert!(
            verify_checkpoints(
                &world,
                &cps,
                &[],
                &positions,
                &RegionEvents::default(),
                &linear
            )
            .is_err()
        );
    }

    #[test]
    fn checkpoint_with_reachable_remaining_path_passes() {
        // Both the checkpoint and the next anchor sit on the same connected floor.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        assert!(
            verify_checkpoints(
                &world,
                &cps,
                &[],
                &positions,
                &RegionEvents::default(),
                &linear
            )
            .is_ok()
        );
    }

    #[test]
    fn checkpoint_over_void_is_dw0316() {
        // The checkpoint cell has no standable floor within snap radius.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [20, 65, 20], 0usize)];
        let err = verify_checkpoints(&world, &cps, &[], &[], &RegionEvents::default(), &linear)
            .unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_UNSTANDABLE); // DW0316
    }
}
