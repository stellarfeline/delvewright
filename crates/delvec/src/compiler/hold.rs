//! **How long a cutscene holds the party, for every step the route walks.**
//!
//! A `cutscene` puts every player in spectator and returns them when its
//! `cs_end` runs. A bot that is still pathing when that happens is stranded at
//! the camera, so the critical path states, per step, every cutscene the step
//! can fire, as two numbers:
//!
//! * **`cutscene_seconds`** ([`after_holds`]): from the step's completion to the
//!   end of the last cutscene its completion schedules. The bundle the step
//!   fires (an objective's `on_objective_complete`, its quest's `on_complete`
//!   when it is the last objective, a performed trigger's `effects`, a bonfire's
//!   `on_rest` at a rest step) is walked along its own timeline: a `sequence`
//!   step's effects play `at_ticks` after the sequence, at any depth, and every
//!   cutscene on the timeline counts, the latest end winning.
//! * **`en_route_cutscene_seconds`** ([`en_route_holds`]): the longest a
//!   cutscene can hold the party while the step is being walked, fired by
//!   something the route passes rather than by the step itself: an `approach`
//!   trigger (an ambush is one) whose range a cell of the step's proven leg
//!   enters (every such trigger, for a walk the route proof did not route: a
//!   step with no proven leg, or the step after a rest), a pressure-plate or
//!   tripwire trap the leg steps on, a loop's
//!   `on_cross` on its exercise step, and an `on_arrive` bundle of a body a
//!   previous step set moving, which lands at a time no tick count states.
//!
//! The nested lists of the effect grammar are classified by [`Nesting`]; the
//! classification is exhaustive over the containers the schema declares
//! (`hold_containers.rs` enumerates them from `delvec schema`). A list that
//! fires on a death or a capture (`on_respawn`, `on_caught`) is not on the
//! route and holds no step.
//!
//! Every hold is an upper bound, gates (`when`, `requires_flags`) not read: the
//! bot sleeps a `cutscene_seconds` it is handed, and waits out an en-route hold
//! only when control is actually taken, so the direction of an over-statement
//! is a slower run, never a stranded one.

use delvewright_dsl::stages::{TrapTrigger, TriggerOn};
use delvewright_dsl::{Campaign, QuestEffect, Verb};

use crate::compiler::nav::LegRoute;
use crate::compiler::plan::{Plan, Step};

/// Server ticks per second.
const TICKS_PER_SECOND: u32 = 20;

/// When a nested effect list fires, relative to the effect that carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nesting {
    /// On the carrier's own timeline, this many ticks after it (`sequence`
    /// steps).
    Timeline(u32),
    /// When the body a `move-npc` / `move-actor` moves arrives (`on_arrive`):
    /// later, while the route is being walked.
    Arrival,
    /// When the party rests at the bonfire (`on_rest`): at the rest step.
    Rest,
    /// When a player respawns at the checkpoint (`on_respawn`): a death.
    Respawn,
    /// When a stealth beat catches the party (`on_caught`): a capture.
    Caught,
}

impl Nesting {
    /// The classification of a nested list by the field that holds it, as
    /// [`QuestEffect::nested_effect_lists_labeled`] names it; `None` for a
    /// field this module does not know, which `hold_containers.rs` reds.
    pub fn of_field(field: &str) -> Option<Nesting> {
        match field {
            "on_arrive" => Some(Nesting::Arrival),
            "on_rest" => Some(Nesting::Rest),
            "on_respawn" => Some(Nesting::Respawn),
            "on_caught" => Some(Nesting::Caught),
            _ => None,
        }
    }

    /// Every field name a nested list is held under, with `steps` for the
    /// `sequence` container — the set the schema is compared against.
    pub const FIELDS: [&'static str; 5] =
        ["steps", "on_arrive", "on_rest", "on_respawn", "on_caught"];
}

/// Every effect list nested directly in `eff`, classified.
pub fn nested(eff: &QuestEffect) -> Vec<(Nesting, &[QuestEffect])> {
    if let Verb::Sequence { steps } = &eff.verb {
        return steps
            .iter()
            .map(|s| (Nesting::Timeline(s.at_ticks), s.effects.as_slice()))
            .collect();
    }
    eff.nested_effect_lists_labeled()
        .into_iter()
        .map(|(field, _, list)| {
            let n = Nesting::of_field(&field).unwrap_or_else(|| {
                panic!(
                    "compiler::hold: the nested effect list `{field}` has no cutscene \
                     classification — add it to `Nesting::of_field`"
                )
            });
            (n, list)
        })
        .collect()
}

/// Ticks from `list` firing to the end of the last cutscene on its own
/// timeline; `None` when the timeline plays none.
pub fn timeline_end(list: &[QuestEffect]) -> Option<u32> {
    fn walk(list: &[QuestEffect], t0: u32, best: &mut Option<u32>) {
        for e in list {
            if let Some(off) = crate::compiler::link::cutscene_end_offset(e) {
                let end = t0 + off;
                *best = Some(best.map_or(end, |b| b.max(end)));
            }
            for (n, inner) in nested(e) {
                if let Nesting::Timeline(at) = n {
                    walk(inner, t0 + at, best);
                }
            }
        }
    }
    let mut best = None;
    walk(list, 0, &mut best);
    best
}

/// Every list of `kind` reachable from `list` along its timeline, and from
/// inside those, at any depth.
fn deferred<'a>(list: &'a [QuestEffect], kind: Nesting, out: &mut Vec<&'a [QuestEffect]>) {
    for e in list {
        for (n, inner) in nested(e) {
            match n {
                Nesting::Timeline(_) => deferred(inner, kind, out),
                k if k == kind => {
                    out.push(inner);
                    deferred(inner, kind, out);
                }
                _ => {}
            }
        }
    }
}

/// The longest hold of an `on_arrive` bundle `list` sets in motion, in ticks.
fn arrival_end(list: &[QuestEffect]) -> Option<u32> {
    let mut lists = Vec::new();
    deferred(list, Nesting::Arrival, &mut lists);
    lists.into_iter().filter_map(timeline_end).max()
}

/// Whole seconds covering `ticks`.
fn seconds(ticks: u32) -> u32 {
    ticks.div_ceil(TICKS_PER_SECOND)
}

fn max_opt(a: Option<u32>, b: Option<u32>) -> Option<u32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// The bundles a step's completion fires: an objective's own bundle and, when
/// it is its quest's last objective, the quest's `on_complete`; a performed
/// trigger's `effects`. Empty for every other step.
fn completion_bundles<'a>(campaign: &'a Campaign, step: &Step) -> Vec<&'a [QuestEffect]> {
    let mut out: Vec<&'a [QuestEffect]> = Vec::new();
    let quests = &campaign.quests.content;
    if let Some(obj) = step.objective() {
        for q in &quests.quests {
            if let Some(b) = q
                .on_objective_complete
                .get(&delvewright_dsl::ObjectiveId(obj.to_string()))
            {
                out.push(b.as_slice());
            }
            let last = crate::compiler::flow::objectives_in_order(&q.objectives)
                .last()
                .is_some_and(|o| o.id().as_str() == obj);
            if last {
                out.push(q.on_complete.as_slice());
            }
        }
    }
    if let Step::Trigger { trigger_id, .. } = step {
        for t in &quests.triggers {
            if t.id.as_str() == trigger_id {
                out.push(t.effects.as_slice());
            }
        }
        for a in &quests.ambushes {
            let t = a.to_trigger();
            if t.id.as_str() == trigger_id {
                // An ambush's bundle is its telegraph plus generated spawns.
                out.push(a.telegraph.as_slice());
            }
        }
    }
    out
}

/// Per step of `steps`, the `cutscene_seconds` its completion owes: seconds
/// from completion to the end of the last cutscene the bundles it fires
/// schedule. Aligned 1:1 with `steps`.
pub fn after_holds(campaign: &Campaign, steps: &[Step]) -> Vec<Option<u32>> {
    steps
        .iter()
        .map(|s| {
            completion_bundles(campaign, s)
                .into_iter()
                .filter_map(timeline_end)
                .max()
                .map(seconds)
        })
        .collect()
}

/// The `cutscene_seconds` a bonfire rest step owes: its `on_rest` bundle's
/// timeline.
pub fn rest_hold(on_rest: &[QuestEffect]) -> Option<u32> {
    timeline_end(on_rest).map(seconds)
}

/// Whether a body standing in `cell` is inside an `approach` trigger's range of
/// the anchor cell `at`, read as the emitted selector reads it: the player's
/// position (the cell's floor centre) within `range` of the anchor's corner.
fn within(cell: [i32; 3], at: [i32; 3], range: f64) -> bool {
    let d = [
        f64::from(cell[0]) + 0.5 - f64::from(at[0]),
        f64::from(cell[1]) - f64::from(at[1]),
        f64::from(cell[2]) + 0.5 - f64::from(at[2]),
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() <= range
}

/// Per step of `steps`, the `en_route_cutscene_seconds` its walk owes: the
/// longest a cutscene fired by something the route passes can hold the party
/// while the step is under way. `routes` are the step's proven legs
/// (`LegRoute::to_step` indexes `steps`). Aligned 1:1 with `steps`.
pub fn en_route_holds(plan: &Plan, steps: &[Step], routes: &[LegRoute]) -> Vec<Option<u32>> {
    let campaign = plan.campaign;
    let quests = &campaign.quests.content;
    let mut ticks: Vec<Option<u32>> = vec![None; steps.len()];
    let mut put = |i: usize, t: Option<u32>| {
        if let Some(slot) = ticks.get_mut(i) {
            *slot = max_opt(*slot, t);
        }
    };
    // Bodies a step's completion sets moving land later, during a step after
    // it: their `on_arrive` cutscenes may hold any later walk.
    let mut arriving: Option<u32> = None;
    for (i, s) in steps.iter().enumerate() {
        put(i, arriving);
        for b in completion_bundles(campaign, s) {
            arriving = max_opt(arriving, arrival_end(b));
        }
    }
    // A walk the route proof did not route is assumed to pass everything: a
    // step with no proven leg, and the step after a rest, which the bot walks
    // from the bonfire rather than from where its leg was proven.
    let routed: std::collections::BTreeSet<usize> = routes.iter().map(|r| r.to_step).collect();
    let mut unrouted: std::collections::BTreeSet<usize> = steps
        .iter()
        .enumerate()
        .filter(|(i, s)| s.pos().is_some() && !routed.contains(i))
        .map(|(i, _)| i)
        .collect();
    for bf in plan.bonfires() {
        let armed = plan
            .critical_path
            .get(bf.fire_step)
            .and_then(Step::objective);
        let k = match armed {
            Some(o) => steps.iter().position(|s| s.objective() == Some(o)),
            None => (bf.fire_step < steps.len()).then_some(bf.fire_step),
        };
        if let Some(k) = k
            && k + 1 < steps.len()
        {
            unrouted.insert(k + 1);
        }
    }
    // What a leg passes.
    let mut passed: Vec<(usize, Option<u32>)> = Vec::new();
    for t in quests.all_triggers() {
        let TriggerOn::Approach { range } = t.on else {
            continue;
        };
        let hold = max_opt(timeline_end(&t.effects), arrival_end(&t.effects));
        let (Some(hold), Some(at)) = (hold, t.at_anchor().and_then(|a| plan.point_any(a))) else {
            continue;
        };
        let first = routes
            .iter()
            .filter(|r| r.cells.iter().any(|&c| within(c, at, f64::from(range))))
            .map(|r| r.to_step)
            .min();
        for r in routes {
            if r.cells.iter().any(|&c| within(c, at, f64::from(range))) {
                passed.push((r.to_step, Some(hold)));
            }
        }
        passed.extend(unrouted.iter().map(|&i| (i, Some(hold))));
        if let (Some(f), Some(a)) = (first, arrival_end(&t.effects)) {
            for j in f + 1..steps.len() {
                passed.push((j, Some(a)));
            }
        }
    }
    for trap in &quests.traps {
        if !matches!(
            trap.trigger,
            TrapTrigger::PressurePlate | TrapTrigger::Tripwire
        ) {
            continue;
        }
        let hold = max_opt(timeline_end(&trap.payload), arrival_end(&trap.payload));
        let (Some(hold), Some(at)) = (hold, plan.point_any(trap.at.as_str())) else {
            continue;
        };
        for r in routes {
            if r.cells.iter().any(|&c| within(c, at, 1.0)) {
                passed.push((r.to_step, Some(hold)));
            }
        }
        passed.extend(unrouted.iter().map(|&i| (i, Some(hold))));
    }
    for (i, s) in steps.iter().enumerate() {
        if let Step::Loop { loop_id, .. } = s
            && let Some(l) = quests.loops.iter().find(|l| l.id.as_str() == loop_id)
        {
            passed.push((
                i,
                max_opt(timeline_end(&l.on_cross), arrival_end(&l.on_cross)),
            ));
        }
    }
    for (i, t) in passed {
        put(i, t);
    }
    ticks.into_iter().map(|t| t.map(seconds)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effects(v: serde_json::Value) -> Vec<QuestEffect> {
        serde_json::from_value(v).expect("effects parse")
    }

    fn cutscene(seconds: u32) -> serde_json::Value {
        serde_json::json!({
            "type": "cutscene",
            "seconds": seconds,
            "path": [{ "anchor": "anchor/a" }, { "anchor": "anchor/b" }]
        })
    }

    #[test]
    fn a_cutscene_inside_a_sequence_holds_from_its_step_offset() {
        let top = effects(serde_json::json!([cutscene(2)]));
        let top_end = timeline_end(&top).unwrap();
        let nested = effects(serde_json::json!([{
            "type": "sequence",
            "steps": [
                { "at_ticks": 0, "effects": [{ "type": "narrate", "text": "x" }] },
                { "at_ticks": 60, "effects": [cutscene(2)] }
            ]
        }]));
        assert_eq!(timeline_end(&nested), Some(60 + top_end));
    }

    #[test]
    fn the_latest_cutscene_on_a_timeline_wins_not_the_first() {
        let two = effects(serde_json::json!([cutscene(1), cutscene(5)]));
        let five = effects(serde_json::json!([cutscene(5)]));
        assert_eq!(timeline_end(&two), timeline_end(&five));
    }

    #[test]
    fn a_bundle_with_no_cutscene_holds_nothing() {
        let none = effects(serde_json::json!([{ "type": "narrate", "text": "x" }]));
        assert_eq!(timeline_end(&none), None);
    }

    #[test]
    fn seconds_cover_every_tick() {
        assert_eq!(seconds(1), 1);
        assert_eq!(seconds(20), 1);
        assert_eq!(seconds(21), 2);
    }

    #[test]
    fn a_body_in_range_is_within_and_one_beyond_is_not() {
        assert!(within([10, 64, 10], [10, 64, 10], 1.0));
        assert!(within([13, 64, 10], [10, 64, 10], 4.0));
        assert!(!within([15, 64, 10], [10, 64, 10], 4.0));
    }
}
