//! **The bodies loose while each step is walked** (spec-0132 §5.3): the
//! `outrun` actors a path's steps are walked beside, derived by the compiler
//! and never by the bot.
//!
//! A step that stands nowhere (`select-class`, `mock`, `assert-complete`) is
//! walked beside nobody. Otherwise an actor is loose while a step is walked
//! when an `unleash-actor` of it has
//! fired at an earlier step of the path — the body arrived before the walk
//! began — and no `despawn-actor` of it has fired since. The step whose own
//! completion (or performance, for a trigger) fires the `despawn-actor` is not
//! walked beside it to the end: the reading after its completion would find
//! no body, so that step is not pursued. A step is dated by the root that
//! fires the line: an objective's bundle at the objective's step, a quest's
//! `on_complete` at the step that completes its last objective on the path, a
//! trigger's or a loop's bundle at the step that performs it — the same step
//! maps every other proof reads ([`PathFiring`]). A reaction bundle nested
//! below a line (`on_respawn`, `on_rest`, `on_caught`) runs when its event
//! does, not at the step.
//!
//! An `unleash-actor` the path cannot date — a trigger the path does not
//! perform, a trap, a reaction bundle, an `on_kill`, a shop — may fire at any
//! step, so its actor is a pursuer of every step from the path's first, and
//! the row says `undated: true`. A `despawn-actor` the path cannot date may
//! never fire, so it removes nothing.

use super::*;
use delvewright_dsl::EffectRootOwner;

/// One body loose while a step is walked: the exported `pursuers[]` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pursuer {
    /// The actor id (`actor/<kebab>`).
    pub actor: String,
    /// The free body's kind: the actor's `entity` as written.
    pub kind: String,
    /// The census function the harness calls to read it
    /// (`<ns>:actor_census_<id>`).
    pub census: String,
    /// The registry's entrance length for the kind, `0` for a body that
    /// declares no `entrance`.
    pub entrance_ticks: u32,
    /// The unleash that looses it could not be dated on this path.
    pub undated: bool,
}

impl Pursuer {
    /// The row as the harness contract spells it.
    pub fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::json!({
            "actor": self.actor,
            "kind": self.kind,
            "census": self.census,
            "entrance_ticks": self.entrance_ticks,
        });
        if self.undated
            && let Some(o) = v.as_object_mut()
        {
            o.insert("undated".to_string(), serde_json::json!(true));
        }
        v
    }
}

/// When a lifecycle line of one actor fires on the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum When {
    /// At the completion (or performance) of this step.
    At(usize),
    /// At a moment the path cannot date.
    Undated,
}

/// The step a line is fired at on this path, or `None` when the path does
/// not fire it.
fn dated(
    owner: &EffectRootOwner<'_>,
    pointer: &str,
    reaction: bool,
    firing: &PathFiring,
) -> Option<When> {
    if reaction {
        return Some(When::Undated);
    }
    let step = match owner {
        EffectRootOwner::ObjectiveComplete { objective, .. } => {
            firing.obj_step.get(*objective).copied()
        }
        EffectRootOwner::QuestComplete { quest } => quest
            .objectives
            .iter()
            .filter_map(|o| firing.obj_step.get(o.id().as_str()).copied())
            .max(),
        EffectRootOwner::Trigger(t) => match firing.trigger_step.get(t.id.as_str()) {
            Some(s) => Some(*s),
            None => return Some(When::Undated),
        },
        EffectRootOwner::LoopCross(l) => match firing.trigger_step.get(l.id.as_str()) {
            Some(s) => Some(*s),
            None => return Some(When::Undated),
        },
        _ => return Some(When::Undated),
    };
    match step {
        Some(s) if firing.fired.contains(pointer) => Some(When::At(s)),
        _ => None,
    }
}

/// Every lifecycle line (`unleash-actor`, `despawn-actor`) of an `outrun`
/// actor at any depth of any root, with the JSON pointer, its root and
/// whether a reaction bundle holds it.
fn lifecycle_lines<'a>(
    c: &'a Campaign,
    outrun: &BTreeSet<&str>,
) -> Vec<(String, EffectRootOwner<'a>, bool, &'a QuestEffect)> {
    fn walk<'a>(
        list: &'a [QuestEffect],
        base: &str,
        reaction: bool,
        owner: &EffectRootOwner<'a>,
        outrun: &BTreeSet<&str>,
        out: &mut Vec<(String, EffectRootOwner<'a>, bool, &'a QuestEffect)>,
    ) {
        for (i, e) in list.iter().enumerate() {
            let path = format!("{base}/{i}");
            if let Verb::UnleashActor { actor } | Verb::DespawnActor { actor, .. } = &e.verb
                && outrun.contains(actor.as_str())
            {
                out.push((path.clone(), *owner, reaction, e));
            }
            for (seg, _, nested) in e.nested_effect_lists_labeled() {
                let r = reaction || matches!(seg.as_str(), "on_respawn" | "on_rest" | "on_caught");
                walk(nested, &format!("{path}/{seg}"), r, owner, outrun, out);
            }
        }
    }
    let mut out = Vec::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        walk(list, &site.path, false, &site.owner, outrun, &mut out);
    });
    out
}

/// **Per step of `steps`, the `outrun` actors loose while it is walked**, in
/// actor declaration order. Aligned 1:1 with `steps`; every entry empty for a
/// campaign that declares no `outrun` actor.
pub(crate) fn pursuers_by_step(
    c: &Campaign,
    namespace: &str,
    steps: &[Step],
    firing: &PathFiring,
) -> Vec<Vec<Pursuer>> {
    let mut out: Vec<Vec<Pursuer>> = vec![Vec::new(); steps.len()];
    let actors: Vec<&delvewright_dsl::Actor> = c
        .quests
        .content
        .actors
        .iter()
        .filter(|a| a.outrun)
        .collect();
    if actors.is_empty() {
        return out;
    }
    let ids: BTreeSet<&str> = actors.iter().map(|a| a.id.as_str()).collect();
    let lines = lifecycle_lines(c, &ids);
    for a in actors {
        let id = a.id.as_str();
        // (step, is_unleash), dated lines only; undated unleash recorded apart.
        let mut events: Vec<(usize, bool)> = Vec::new();
        let mut undated = false;
        for (pointer, owner, reaction, e) in &lines {
            let (is_unleash, actor) = match &e.verb {
                Verb::UnleashActor { actor } => (true, actor),
                Verb::DespawnActor { actor, .. } => (false, actor),
                _ => continue,
            };
            if actor.as_str() != id {
                continue;
            }
            match dated(owner, pointer, *reaction, firing) {
                Some(When::At(s)) => {
                    // A despawn counts only where the path certainly fires it.
                    if is_unleash || firing.fires(pointer) {
                        events.push((s, is_unleash));
                    }
                }
                // An undated despawn may never fire, so it removes nothing.
                Some(When::Undated) if is_unleash => undated = true,
                Some(When::Undated) | None => {}
            }
        }
        // Within one step, a despawn is taken to follow an unleash: the body
        // the step's beat brought in and took out again is not loose after it.
        events.sort_by_key(|(s, unleash)| (*s, !*unleash));
        let row = Pursuer {
            actor: id.to_string(),
            kind: a.entity.clone(),
            census: format!(
                "{namespace}:actor_census_{}",
                crate::compiler::plan::safe_local(id)
            ),
            entrance_ticks: delvewright_dsl::entrance::entrance_ticks(a),
            undated,
        };
        let mut loose = undated;
        let mut k = 0;
        for (i, slot) in out.iter_mut().enumerate() {
            // Every event at an earlier step has happened as step `i` begins.
            while k < events.len() && events[k].0 < i {
                loose = events[k].1;
                k += 1;
            }
            let despawned_here = events.iter().any(|(s, u)| *s == i && !*u);
            // A path-frame step stands nowhere — the class is chosen, the
            // completion asserted, an act mocked — so nothing is walked on it.
            if loose && !despawned_here && steps[i].pos().is_some() {
                slot.push(row.clone());
            }
        }
    }
    out
}

/// `DW1082`'s second arm (spec-0132 §4): an `outrun` actor loose on no step
/// of the exported critical path. The ladder walks no step beside it, so its
/// declaration would bind to nothing.
pub(crate) fn check_outrun_loose(
    c: &Campaign,
    namespace: &str,
    steps: &[Step],
    firing: &PathFiring,
) -> Result<(), PlanError> {
    let by_step = pursuers_by_step(c, namespace, steps, firing);
    let unleashed = delvewright_dsl::unleashed_actors(c);
    for a in c.quests.content.actors.iter().filter(|a| a.outrun) {
        let id = a.id.as_str();
        // An `outrun` nothing unleashes is the first arm's (`dsl::validate`).
        if !unleashed.contains(id) {
            continue;
        }
        if by_step.iter().any(|row| row.iter().any(|p| p.actor == id)) {
            continue;
        }
        return Err(PlanError::new(
            delvewright_dsl::entrance::DW_FREE_BODY_NEVER_FREED,
            format!(
                "actor `{id}` declares `outrun`, and no step of the exported critical path \
                 ({} step(s)) is walked while its free body is loose: every `unleash-actor` \
                 that names it fires after the path's last step, or is despawned again before \
                 the next step begins (the path is loose beside it on no step). Unleash the \
                 actor on the path, or drop the field",
                steps.len()
            ),
        ));
    }
    Ok(())
}

/// **A leg declared ridden on a vehicle the path has taken away** (spec-0135
/// §4): the objective's `ride` names a vehicle whose last lifecycle line the
/// path certainly fires before the objective's step is a `despawn-vehicle` —
/// the party walks up to a mark nothing stands at. Dated as a pursuer's
/// lines are ([`dated`]): a `seat-vehicle` the path cannot date may fire at
/// any step, so it seats the vehicle for every step; a `despawn-vehicle` the
/// path cannot date, or does not certainly fire, removes nothing. A vehicle
/// is seated at load unless it is seated `by-story`, which stands nowhere
/// until a `seat-vehicle` fires. `DW0311`: the leg is one its vehicle does not
/// carry. A vehicle that stays where its rider left it (`left: where-left`)
/// ridden a second time with no `seat-vehicle` the path fires between the two
/// rides is `DW1107`: its ride is proven from its mark, and its body is where
/// the first ride left it.
pub(crate) fn check_ridden_vehicles_stand(
    c: &Campaign,
    firing: &PathFiring,
) -> Result<(), PlanError> {
    fn walk<'a>(
        list: &'a [QuestEffect],
        base: &str,
        reaction: bool,
        owner: &EffectRootOwner<'a>,
        out: &mut Vec<(String, EffectRootOwner<'a>, bool, &'a QuestEffect)>,
    ) {
        for (i, e) in list.iter().enumerate() {
            let path = format!("{base}/{i}");
            if matches!(
                e.verb,
                Verb::SeatVehicle { .. } | Verb::DespawnVehicle { .. }
            ) {
                out.push((path.clone(), *owner, reaction, e));
            }
            for (seg, _, nested) in e.nested_effect_lists_labeled() {
                let r = reaction || matches!(seg.as_str(), "on_respawn" | "on_rest" | "on_caught");
                walk(nested, &format!("{path}/{seg}"), r, owner, out);
            }
        }
    }
    let rides: Vec<(&str, &str)> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter_map(|o| Some((o.id().as_str(), o.ride()?.as_str())))
        .collect();
    if rides.is_empty() {
        return Ok(());
    }
    let mut lines = Vec::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        walk(list, &site.path, false, &site.owner, &mut lines);
    });
    let decl = |id: &str| {
        c.quests
            .content
            .vehicles
            .iter()
            .find(|v| v.id.as_str() == id)
    };
    for &(objective, vehicle) in &rides {
        let Some(&at) = firing.obj_step.get(objective) else {
            continue;
        };
        // (step, seats) for every line of this vehicle the path dates before
        // the objective's step.
        let mut events: Vec<(usize, bool)> = Vec::new();
        let mut seated_undated = false;
        for (pointer, owner, reaction, e) in &lines {
            let (seats, v) = match &e.verb {
                Verb::SeatVehicle { vehicle } => (true, vehicle),
                Verb::DespawnVehicle { vehicle } => (false, vehicle),
                _ => continue,
            };
            if v.as_str() != vehicle {
                continue;
            }
            match dated(owner, pointer, *reaction, firing) {
                Some(When::At(s)) if s < at && (seats || firing.fires(pointer)) => {
                    events.push((s, seats));
                }
                Some(When::Undated) if seats => seated_undated = true,
                _ => {}
            }
        }
        // Within one step the lines keep the order their bundle writes them
        // in (the sort is stable over the reading order).
        events.sort_by_key(|(s, _)| *s);
        let by_story =
            decl(vehicle).is_some_and(|v| v.seated == delvewright_dsl::VehicleSeated::ByStory);
        if by_story && !seated_undated && !events.iter().any(|(_, seats)| *seats) {
            return Err(PlanError::new(
                crate::compiler::nav::DW_CRITICAL_UNROUTABLE,
                format!(
                    "objective `{objective}` declares its leg ridden on `{vehicle}`, which is \
                     seated `by-story`, and no `seat-vehicle` the path fires before step {at} \
                     seats it — the party walks up to a mark nothing stands at. Seat it at a \
                     beat before the ride, seat it at load, or drop the objective's `ride`"
                ),
            ));
        }
        // A vehicle left where its rider leaves it: an earlier ride on it,
        // with no seat the path fires after that ride and before this one.
        if decl(vehicle).is_some_and(|v| v.left == delvewright_dsl::VehicleLeft::WhereLeft) {
            let earlier = rides
                .iter()
                .filter(|(o, v)| *v == vehicle && *o != objective)
                .filter_map(|(o, _)| Some((*o, *firing.obj_step.get(*o)?)))
                .filter(|(_, s)| *s < at)
                .max_by_key(|(_, s)| *s);
            if let Some((first, s0)) = earlier
                && !seated_undated
                && !events.iter().any(|(s, seats)| *seats && *s >= s0)
            {
                return Err(PlanError::new(
                    crate::compiler::nav::vehicle::DW_VEHICLE_LEFT_ELSEWHERE,
                    format!(
                        "objective `{objective}` declares its leg ridden on `{vehicle}`, which \
                         stays where its rider leaves it (`left: where-left`), and `{first}` \
                         rode it at step {s0} with no `seat-vehicle` the path fires between — \
                         its ride is proven from its mark, and its body is wherever the first \
                         ride left it. Seat it again between the two rides, or let it return \
                         to its mark (drop `left`)"
                    ),
                ));
            }
        }
        if let Some((s, false)) = events.last().copied()
            && !seated_undated
        {
            return Err(PlanError::new(
                crate::compiler::nav::DW_CRITICAL_UNROUTABLE,
                format!(
                    "objective `{objective}` declares its leg ridden on `{vehicle}`, and the \
                     path takes that vehicle away before it: a `despawn-vehicle` fires at step \
                     {s}, the objective is performed at step {at}, and no `seat-vehicle` puts \
                     it back in between (a despawn ends the re-seat) — the party walks up to a \
                     mark nothing stands at. Move the `despawn-vehicle` to a beat after the \
                     ride, seat the vehicle again before it, or drop the objective's `ride`"
                ),
            ));
        }
    }
    Ok(())
}
