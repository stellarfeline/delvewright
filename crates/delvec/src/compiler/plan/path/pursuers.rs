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
//! step, so its actor is a pursuer of every step from the path's first — a
//! dated despawn does not end that, since the arrival may come again after it
//! — and the row says `undated: true`. A `despawn-actor` the path cannot date may
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
        // What the dated lines leave loose as each step begins. An undated
        // unleash may fire at any step — after a dated despawn too — so no
        // despawn takes its body out: it is loose on every step.
        let mut dated_loose = false;
        let mut k = 0;
        for (i, slot) in out.iter_mut().enumerate() {
            // Every event at an earlier step has happened as step `i` begins.
            while k < events.len() && events[k].0 < i {
                dated_loose = events[k].1;
                k += 1;
            }
            let loose = undated || dated_loose;
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

/// **A course whose race is not running when the path reaches it** (spec-0139
/// §4, `DW1119`): the objective's `course` names a race, and of the
/// `start-race` and `end-race` lines of that race the path fires before the
/// objective's step, none is a start or the last is an end. Dated as a
/// vehicle's lines are ([`check_ridden_vehicles_stand`]): a `start-race` the
/// path cannot date — a trigger's, a dialogue option's — may fire at any
/// step, so it starts the race for every step; an `end-race` the path cannot
/// date, or does not certainly fire, ends nothing.
pub(crate) fn check_courses_started(c: &Campaign, firing: &PathFiring) -> Result<(), PlanError> {
    fn walk<'a>(
        list: &'a [QuestEffect],
        base: &str,
        reaction: bool,
        owner: &EffectRootOwner<'a>,
        out: &mut Vec<(String, EffectRootOwner<'a>, bool, &'a QuestEffect)>,
    ) {
        for (i, e) in list.iter().enumerate() {
            let path = format!("{base}/{i}");
            if matches!(e.verb, Verb::StartRace { .. } | Verb::EndRace { .. }) {
                out.push((path.clone(), *owner, reaction, e));
            }
            for (seg, _, nested) in e.nested_effect_lists_labeled() {
                let r = reaction || matches!(seg.as_str(), "on_respawn" | "on_rest" | "on_caught");
                walk(nested, &format!("{path}/{seg}"), r, owner, out);
            }
        }
    }
    let courses: Vec<(&str, &str)> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter_map(|o| Some((o.id().as_str(), o.course()?.as_str())))
        .collect();
    if courses.is_empty() {
        return Ok(());
    }
    let mut lines = Vec::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        walk(list, &site.path, false, &site.owner, &mut lines);
    });
    for &(objective, race) in &courses {
        let Some(&at) = firing.obj_step.get(objective) else {
            continue;
        };
        let mut events: Vec<(usize, bool)> = Vec::new();
        let mut started_undated = false;
        for (pointer, owner, reaction, e) in &lines {
            let (starts, r) = match &e.verb {
                Verb::StartRace { race } => (true, race),
                Verb::EndRace { race } => (false, race),
                _ => continue,
            };
            if r.as_str() != race {
                continue;
            }
            match dated(owner, pointer, *reaction, firing) {
                Some(When::At(s)) if s < at && (starts || firing.fires(pointer)) => {
                    events.push((s, starts));
                }
                Some(When::Undated) if starts => started_undated = true,
                _ => {}
            }
        }
        if started_undated {
            continue;
        }
        events.sort_by_key(|(s, _)| *s);
        let why = match events.last().copied() {
            Some((_, true)) => continue,
            Some((s, false)) => format!(
                "an `end-race` fires at step {s} and no `start-race` starts it again before \
                 step {at}"
            ),
            None => format!("no `start-race` of it fires before step {at}"),
        };
        return Err(PlanError::new(
            crate::compiler::nav::race::DW_COURSE_NOT_STARTED,
            format!(
                "objective `{objective}` runs the course of `{race}`, and that race is not \
                 running when the path reaches it: {why}. The party would ride the course \
                 credited nothing, and the objective would never complete. Fire a \
                 `start-race` of `{race}` at a beat before the objective — in the bundle of \
                 the step before it, or from a trigger the party trips on the way — or drop \
                 the objective's `course`"
            ),
        ));
    }
    Ok(())
}

/// What one path does with the map's mode (spec-0140 §5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ModeWalk {
    /// The spans a mode is on over the path: `(mode, entered at step, left
    /// at step)`, the last `None` for a mode still on when the path ends.
    pub spans: Vec<(String, usize, Option<usize>)>,
    /// Objective steps the path performs while a mode is on, each judged for
    /// its quest being one the mode keeps.
    pub judged: usize,
}

/// **A path that performs an objective a mode holds** (spec-0140 §5,
/// `DW1124`): the map's mode is replayed along the path as the emitted
/// functions move it, and every objective step performed while a mode is on
/// belongs to a quest that mode keeps.
///
/// Dated as a race's lines are ([`check_courses_started`]). An `enter-mode`
/// the path certainly fires at a step puts the map in the mode from that
/// step's completion; one the path cannot date — a trigger it does not
/// perform, a reaction bundle — enters nothing on this path. The mode is
/// left where the emitted settlement leaves it — at the entry itself, or at
/// the completion of a quest it keeps, when no kept quest is started and not
/// complete — at another mode's entry, and at a `leave-mode` the path
/// certainly fires; a `leave-mode` it cannot date leaves nothing.
pub(crate) fn walk_modes(c: &Campaign, firing: &PathFiring) -> Result<ModeWalk, PlanError> {
    fn walk<'a>(
        list: &'a [QuestEffect],
        base: &str,
        reaction: bool,
        owner: &EffectRootOwner<'a>,
        out: &mut Vec<(String, EffectRootOwner<'a>, bool, &'a QuestEffect)>,
    ) {
        for (i, e) in list.iter().enumerate() {
            let path = format!("{base}/{i}");
            if matches!(e.verb, Verb::EnterMode { .. } | Verb::LeaveMode { .. }) {
                out.push((path.clone(), *owner, reaction, e));
            }
            for (seg, _, nested) in e.nested_effect_lists_labeled() {
                let r = reaction || matches!(seg.as_str(), "on_respawn" | "on_rest" | "on_caught");
                walk(nested, &format!("{path}/{seg}"), r, owner, out);
            }
        }
    }
    let q = &c.quests.content;
    let mut out = ModeWalk::default();
    if q.modes.is_empty() {
        return Ok(out);
    }
    let mut lines = Vec::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        walk(list, &site.path, false, &site.owner, &mut lines);
    });
    // Step → the mode lines the path certainly fires there, in bundle order.
    let mut events: BTreeMap<usize, Vec<(bool, &str)>> = BTreeMap::new();
    for (pointer, owner, reaction, e) in &lines {
        let (enters, mode) = match &e.verb {
            Verb::EnterMode { mode } => (true, mode.as_str()),
            Verb::LeaveMode { mode } => (false, mode.as_str()),
            _ => continue,
        };
        if let Some(When::At(s)) = dated(owner, pointer, *reaction, firing)
            && firing.fires(pointer)
        {
            events.entry(s).or_default().push((enters, mode));
        }
    }
    if events.is_empty() {
        return Ok(out);
    }
    // The step each quest the path completes is complete at, and the
    // objectives each step performs.
    let done_at: BTreeMap<&str, usize> = q
        .quests
        .iter()
        .filter(|x| firing.quests.contains(x.id.as_str()))
        .filter_map(|x| {
            let s = x
                .objectives
                .iter()
                .filter_map(|o| firing.obj_step.get(o.id().as_str()).copied())
                .max()?;
            Some((x.id.as_str(), s))
        })
        .collect();
    let mut performed: BTreeMap<usize, Vec<(&str, &str)>> = BTreeMap::new();
    for x in &q.quests {
        for o in &x.objectives {
            if let Some(&s) = firing.obj_step.get(o.id().as_str()) {
                performed
                    .entry(s)
                    .or_default()
                    .push((x.id.as_str(), o.id().as_str()));
            }
        }
    }
    // Whether quest `k` is started and not complete once step `t` is done.
    let in_progress = |k: &str, t: usize| -> bool {
        let Some(x) = q.quests.iter().find(|x| x.id.as_str() == k) else {
            return false;
        };
        let started = match &x.trigger {
            delvewright_dsl::Trigger::CampaignStart => true,
            delvewright_dsl::Trigger::QuestComplete { quest } => {
                done_at.get(quest.as_str()).is_some_and(|&s| s <= t)
            }
        };
        started && done_at.get(k).is_none_or(|&s| s > t)
    };
    let last = performed
        .keys()
        .chain(events.keys())
        .copied()
        .max()
        .unwrap_or(0);
    // The mode the map is in, and the step it was entered at.
    let mut on: Option<(&delvewright_dsl::Mode, usize)> = None;
    for t in 0..=last {
        if let Some((m, from)) = on {
            for &(quest, objective) in performed.get(&t).map(Vec::as_slice).unwrap_or(&[]) {
                out.judged += 1;
                if !m.keeps(quest) {
                    return Err(PlanError::new(
                        crate::compiler::mode::DW_PATH_WALKS_HELD,
                        format!(
                            "the path performs objective `{objective}` of quest `{quest}` at \
                             step {t} while the map is in mode `{}`, which does not keep that \
                             quest: the mode is entered at step {from} and nothing has left it \
                             by step {t}, so the objective is held — not told, not completable \
                             — and the path stops there. A mode that interrupts a line is \
                             ordered by the quest graph: give the quest the mode keeps a \
                             `quest-complete` trigger on what comes before it and let what \
                             follows wait on its completion, enter the mode from a bundle of a \
                             quest it keeps, add `{quest}` to the mode's `keeps`, or fire a \
                             `leave-mode` of `{}` at a beat before the objective",
                            m.id, m.id
                        ),
                    ));
                }
            }
        }
        let settle = |on: &mut Option<(&delvewright_dsl::Mode, usize)>,
                      spans: &mut Vec<(String, usize, Option<usize>)>| {
            if let Some((m, from)) = *on
                && !m.keeps.iter().any(|k| in_progress(k.as_str(), t))
            {
                spans.push((m.id.as_str().to_string(), from, Some(t)));
                *on = None;
            }
        };
        for &(enters, mode) in events.get(&t).map(Vec::as_slice).unwrap_or(&[]) {
            let Some(m) = q.mode(mode) else { continue };
            match (enters, on) {
                (true, Some((cur, _))) if cur.id == m.id => {}
                (true, prev) => {
                    if let Some((cur, from)) = prev {
                        out.spans.push((cur.id.as_str().to_string(), from, Some(t)));
                    }
                    on = Some((m, t));
                    settle(&mut on, &mut out.spans);
                }
                (false, Some((cur, from))) if cur.id == m.id => {
                    out.spans.push((cur.id.as_str().to_string(), from, Some(t)));
                    on = None;
                }
                (false, _) => {}
            }
        }
        // A kept quest completed at this step asks the mode whether anything
        // it keeps is still in progress.
        if let Some((m, _)) = on
            && m.keeps.iter().any(|k| done_at.get(k.as_str()) == Some(&t))
        {
            settle(&mut on, &mut out.spans);
        }
    }
    if let Some((m, from)) = on {
        out.spans.push((m.id.as_str().to_string(), from, None));
    }
    Ok(out)
}

/// A body the campaign looses into the world with a mind of its own: a wave
/// (every stack of it), or an actor's free twin. A puppet stands where it is
/// put and makes no footstep, so an actor is a free body only once an
/// `unleash-actor` of it fires.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FreeBodyKey {
    /// A wave, by id.
    Wave(String),
    /// An actor's free twin, by id.
    Actor(String),
}

impl FreeBodyKey {
    /// The id as written.
    pub fn id(&self) -> &str {
        match self {
            FreeBodyKey::Wave(w) | FreeBodyKey::Actor(w) => w,
        }
    }
}

/// When one arrival line of a free body may first put it in the world on this
/// path: from step `from` on (at that step's walk or any later one), and
/// whether that is exact — the line fires at a performed step's completion and
/// at no other moment. Only an exact arrival is undone by a later despawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Arrival {
    pub from: usize,
    pub exact: bool,
}

/// The steps of a path `n` long a body with these arrivals is loose on:
/// step `i` when some arrival may have come by `i` and, for an exact one, no
/// despawn certainly fired after it and by `i` (`removals` are the first step
/// each despawn has taken effect for). An arrival that is not exact may come
/// after any despawn, so nothing takes its body out again.
pub(crate) fn loose_steps(arrivals: &[Arrival], removals: &[usize], n: usize) -> Vec<bool> {
    (0..n)
        .map(|i| {
            arrivals.iter().any(|a| {
                a.from <= i && !(a.exact && removals.iter().any(|&r| a.from < r && r <= i))
            })
        })
        .collect()
}

/// **Per free body, the steps of `steps` it may be loose while walked** — the
/// bodies a vibration from somewhere other than the party can come from
/// (spec-0100 §2.4: a mob's step sets a sensor off and never reaches a
/// shrieker). Aligned 1:1 with `steps`, bodies in key order; a body the path
/// never looses is absent.
///
/// A wave arrives with each `spawn-wave` of it, an actor's free twin with each
/// `unleash-actor` (an `ambush` desugars to one). A line is dated as a
/// pursuer's ([`dated`]): one a performed step's bundle fires arrives after
/// that step, exactly; one in the bundle of a trigger the path does not
/// perform arrives no earlier than `sprung(trigger)` — the first step whose
/// walk can spring it — and at any step after; every other undatable line
/// from the path's first step. A wave is never taken out again: a kill-count
/// objective completes on one player's count while a party's extra bodies
/// stand (spec-0134 §4), and a re-seat puts a cleared wave back. An actor is
/// taken out by a `despawn-actor` the path certainly fires after an exact
/// arrival ([`loose_steps`]).
pub(crate) fn free_bodies_by_step(
    c: &Campaign,
    steps: &[Step],
    firing: &PathFiring,
    sprung: &dyn Fn(&str) -> Option<usize>,
) -> Vec<(FreeBodyKey, Vec<bool>)> {
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
                Verb::SpawnWave { .. } | Verb::UnleashActor { .. } | Verb::DespawnActor { .. }
            ) {
                out.push((path.clone(), *owner, reaction, e));
            }
            for (seg, _, nested) in e.nested_effect_lists_labeled() {
                let r = reaction || matches!(seg.as_str(), "on_respawn" | "on_rest" | "on_caught");
                walk(nested, &format!("{path}/{seg}"), r, owner, out);
            }
        }
    }
    let mut lines = Vec::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        walk(list, &site.path, false, &site.owner, &mut lines);
    });
    let mut arrivals: BTreeMap<FreeBodyKey, Vec<Arrival>> = BTreeMap::new();
    let mut removals: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (pointer, owner, reaction, e) in &lines {
        let key = match &e.verb {
            Verb::SpawnWave { wave, .. } => FreeBodyKey::Wave(wave.as_str().to_string()),
            Verb::UnleashActor { actor } => FreeBodyKey::Actor(actor.as_str().to_string()),
            Verb::DespawnActor { actor, .. } => {
                if let Some(When::At(s)) = dated(owner, pointer, *reaction, firing)
                    && firing.fires(pointer)
                {
                    removals
                        .entry(actor.as_str().to_string())
                        .or_default()
                        .push(s + 1);
                }
                continue;
            }
            _ => continue,
        };
        let arrival = match (owner, dated(owner, pointer, *reaction, firing)) {
            (_, Some(When::At(s))) => Arrival {
                from: s + 1,
                exact: true,
            },
            (EffectRootOwner::Trigger(t), Some(When::Undated)) if !*reaction => Arrival {
                from: sprung(t.id.as_str()).unwrap_or(0),
                exact: false,
            },
            (_, Some(When::Undated)) => Arrival {
                from: 0,
                exact: false,
            },
            (_, None) => continue,
        };
        arrivals.entry(key).or_default().push(arrival);
    }
    let mut out = Vec::new();
    for (key, list) in arrivals {
        let gone: &[usize] = match &key {
            FreeBodyKey::Actor(a) => removals.get(a).map_or(&[], Vec::as_slice),
            FreeBodyKey::Wave(_) => &[],
        };
        let loose = loose_steps(&list, gone, steps.len());
        if loose.iter().any(|l| *l) {
            out.push((key, loose));
        }
    }
    out
}

delvewright_dsl::dw_code! {
    /// `DW1140`: a `despawn-actor` a proven path certainly fires before any
    /// line that places its actor can have fired on that path — the despawn
    /// removes nothing, and a body that arrives after it stays. Names the
    /// actor, the line, the step and the first step an arrival can come.
    pub const DW_DESPAWN_BEFORE_ARRIVAL: DwCode = DwCode::new("DW1140", ExitTier::Build);
}

/// Whether some body of an actor with these arrivals can stand by the
/// completion of step `d`: an exact arrival at a step's completion (`from` is
/// the step after it) at that step or an earlier one — within one step a
/// despawn is taken to follow an arrival, as the pursuers are walked — and an
/// arrival that is not exact (it comes during a walk, from `from` on) by the
/// walk of step `d`.
pub(crate) fn arrived_by(arrivals: &[Arrival], d: usize) -> bool {
    arrivals
        .iter()
        .any(|a| if a.exact { a.from <= d + 1 } else { a.from <= d })
}

/// What [`check_despawns_follow_arrivals`] judged on one path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DespawnBinding {
    /// `despawn-actor` lines the path certainly fires at a dated step.
    pub judged: usize,
    /// Of those, the ones an arrival of their actor can precede.
    pub after_arrival: usize,
}

impl DespawnBinding {
    /// The line a build prints for the exported path.
    pub fn line(&self) -> String {
        format!(
            "despawn binding (DW1140): {} despawn-actor line(s) the exported path certainly \
             fires judged, {} with an arrival of their actor able to come before them, {} refused",
            self.judged,
            self.after_arrival,
            self.judged - self.after_arrival
        )
    }
}

/// **`DW1140`: a despawn the path fires before its actor can be there.** For
/// every `despawn-actor` the path certainly fires at a dated step `d`, some
/// line that places its actor — a `spawn-actor` (the puppet) or an
/// `unleash-actor` (the twin; an `ambush` desugars to both) — must be able to
/// fire by `d` on this path ([`arrived_by`]): one dated at a step up to `d`;
/// one in a trigger the path does not perform, from `sprung(trigger)` — the
/// first step whose walk can spring it — when that is up to `d`, or from the
/// first step when no walk is known to spring it; and any other line the path
/// cannot date (a trap, a reaction bundle, an `on_kill`, a shop), which may
/// fire at any step. A despawn that tidies a body an optional act may have
/// brought in is therefore legal; one that every placing line on the path
/// follows is refused, because on that play order its beat — a death or a
/// departure the story states — never happens, and the body it was written
/// to remove arrives after it and stays.
pub(crate) fn check_despawns_follow_arrivals(
    c: &Campaign,
    steps: &[Step],
    firing: &PathFiring,
    sprung: &dyn Fn(&str) -> Option<usize>,
) -> (DespawnBinding, Result<(), Failure>) {
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
                Verb::SpawnActor { .. } | Verb::UnleashActor { .. } | Verb::DespawnActor { .. }
            ) {
                out.push((path.clone(), *owner, reaction, e));
            }
            for (seg, _, nested) in e.nested_effect_lists_labeled() {
                let r = reaction || matches!(seg.as_str(), "on_respawn" | "on_rest" | "on_caught");
                walk(nested, &format!("{path}/{seg}"), r, owner, out);
            }
        }
    }
    let mut lines = Vec::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        walk(list, &site.path, false, &site.owner, &mut lines);
    });
    let mut arrivals: BTreeMap<&str, Vec<Arrival>> = BTreeMap::new();
    let mut despawns: Vec<(&str, &str, usize)> = Vec::new();
    for (pointer, owner, reaction, e) in &lines {
        match &e.verb {
            Verb::SpawnActor { actor } | Verb::UnleashActor { actor } => {
                let arrival = match (owner, dated(owner, pointer, *reaction, firing)) {
                    (_, Some(When::At(s))) => Arrival {
                        from: s + 1,
                        exact: true,
                    },
                    (EffectRootOwner::Trigger(t), Some(When::Undated)) if !*reaction => Arrival {
                        from: sprung(t.id.as_str()).unwrap_or(0),
                        exact: false,
                    },
                    (_, Some(When::Undated)) => Arrival {
                        from: 0,
                        exact: false,
                    },
                    (_, None) => continue,
                };
                arrivals.entry(actor.as_str()).or_default().push(arrival);
            }
            Verb::DespawnActor { actor, .. } => {
                if let Some(When::At(d)) = dated(owner, pointer, *reaction, firing)
                    && firing.fires(pointer)
                {
                    despawns.push((actor.as_str(), pointer.as_str(), d));
                }
            }
            _ => {}
        }
    }
    let mut b = DespawnBinding {
        judged: despawns.len(),
        ..DespawnBinding::default()
    };
    let mut verdict = Ok(());
    for (actor, pointer, d) in despawns {
        let list = arrivals.get(actor).map_or(&[][..], Vec::as_slice);
        if arrived_by(list, d) {
            b.after_arrival += 1;
            continue;
        }
        if verdict.is_err() {
            continue;
        }
        let at = |i: usize| match steps.get(i).and_then(Step::objective) {
            Some(o) => format!("step {i} (`{o}`)"),
            None => format!("step {i}"),
        };
        let first = list
            .iter()
            .map(|a| if a.exact { a.from - 1 } else { a.from })
            .min();
        let when = match first {
            Some(s) => format!(
                "the first line that places it can fire at {}, after the despawn",
                at(s)
            ),
            None => "no line on this path places it at all".to_string(),
        };
        verdict = Err(Failure::new(
            DW_DESPAWN_BEFORE_ARRIVAL,
            format!(
                "`despawn-actor` of `{actor}` at `{pointer}` fires at {} of the path, and \
                 the actor cannot be standing then: {when} (a `spawn-actor` or `unleash-actor` \
                 of it, an `ambush` that names it included). On this play order the despawn \
                 removes nothing and its beat never happens, and a body that arrives later \
                 stays. Move the despawn to a beat after the actor's arrival, or bring the \
                 actor in before it",
                at(d)
            ),
        ));
    }
    (b, verdict)
}

#[cfg(test)]
mod free_body_tests {
    use super::*;

    /// `DW1140`'s arithmetic: a despawn at step 3's completion follows an
    /// exact arrival at step 3 (its `from` is 4) and one sprung by the walk
    /// of step 3, and precedes one sprung by step 4's walk or at step 4's
    /// completion — the gallery's rafter spider, despawned at the muster's
    /// clearing and loosed by an ambush a later walk springs.
    #[test]
    fn a_despawn_follows_only_an_arrival_that_can_come_by_its_step() {
        let exact = |from| Arrival { from, exact: true };
        let walk = |from| Arrival { from, exact: false };
        assert!(arrived_by(&[exact(4)], 3));
        assert!(arrived_by(&[walk(3)], 3));
        assert!(arrived_by(&[walk(0)], 0));
        assert!(!arrived_by(&[exact(5)], 3));
        assert!(!arrived_by(&[walk(4)], 3));
        assert!(!arrived_by(&[], 3));
        assert!(arrived_by(&[walk(4), exact(2)], 3));
    }

    #[test]
    fn an_exact_arrival_is_loose_from_the_step_after_its_beat_until_a_despawn() {
        let a = [Arrival {
            from: 3,
            exact: true,
        }];
        assert_eq!(
            loose_steps(&a, &[5], 7),
            vec![false, false, false, true, true, false, false]
        );
        // A despawn before the arrival takes nothing out.
        assert_eq!(
            loose_steps(&a, &[2], 5),
            vec![false, false, false, true, true]
        );
    }

    /// The gallery's rafter spider: its `despawn-actor` fires at the muster's
    /// clearing, and the ambush that looses it is sprung by a later walk — so
    /// the despawn is spent before the body arrives, and the body stays.
    #[test]
    fn an_arrival_sprung_by_a_walk_is_never_taken_out() {
        let a = [Arrival {
            from: 4,
            exact: false,
        }];
        assert_eq!(
            loose_steps(&a, &[3, 5], 7),
            vec![false, false, false, false, true, true, true]
        );
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;
    use serde_json::json;

    const MAIN: &str = "quest/open-the-door";

    /// hello-world (`obj/talk`, then `obj/exit`) beside a second quest,
    /// `quest/side` (`obj/side`), started as `side_trigger` says; one mode
    /// keeping `keeps`; and `lines` in the bundle of `obj/talk`.
    fn campaign(
        keeps: &[&str],
        side_trigger: serde_json::Value,
        lines: serde_json::Value,
    ) -> Campaign {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dsl/fixtures/valid/hello-world");
        let loaded = crate::compiler::load::load_campaign_dir(&dir).expect("the fixture loads");
        let mut c = crate::compiler::load::parse_loaded(&loaded).expect("the fixture parses");
        let q = &mut c.quests.content;
        q.quests.push(
            serde_json::from_value(json!({
                "id": "quest/side",
                "trigger": side_trigger,
                "objectives": [{"type": "reach-anchor", "id": "obj/side",
                                "anchor": "anchor/exit", "radius": 2}],
                "on_complete": []
            }))
            .unwrap(),
        );
        q.modes = serde_json::from_value(json!([{"id": "mode/aside", "keeps": keeps}])).unwrap();
        let bundle: Vec<QuestEffect> = serde_json::from_value(lines).unwrap();
        q.quests[0]
            .on_objective_complete
            .get_mut(&serde_json::from_value(json!("obj/talk")).unwrap())
            .expect("the fixture's bundle")
            .extend(bundle);
        c
    }

    /// A path performing `order` from step 1, firing every mode line unless
    /// `certain` is false.
    fn firing(c: &Campaign, order: [&str; 3], certain: bool) -> PathFiring {
        let mut f = PathFiring::default();
        for (i, o) in order.iter().enumerate() {
            f.obj_step.insert(o.to_string(), i + 1);
        }
        f.quests = c
            .quests
            .content
            .quests
            .iter()
            .map(|q| q.id.as_str().to_string())
            .collect();
        if certain {
            delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
                for (i, e) in list.iter().enumerate() {
                    if matches!(e.verb, Verb::EnterMode { .. } | Verb::LeaveMode { .. }) {
                        f.fired.insert(format!("{}/{i}", site.path));
                    }
                }
            });
        }
        f
    }

    fn enter() -> serde_json::Value {
        json!({"type": "enter-mode", "mode": "mode/aside"})
    }

    fn start() -> serde_json::Value {
        json!({"type": "campaign-start"})
    }

    const SIDE_FIRST: [&str; 3] = ["obj/talk", "obj/side", "obj/exit"];
    const SIDE_LAST: [&str; 3] = ["obj/talk", "obj/exit", "obj/side"];

    #[test]
    fn a_step_of_a_held_quest_under_the_mode_is_dw1124() {
        let c = campaign(&[MAIN], start(), json!([enter()]));
        let e = walk_modes(&c, &firing(&c, SIDE_FIRST, true)).unwrap_err();
        assert_eq!(e.failure.code.to_string(), "DW1124");
        for needle in [
            "`obj/side`",
            "`quest/side`",
            "step 2",
            "`mode/aside`",
            "step 1",
        ] {
            assert!(e.failure.message.contains(needle), "{}", e.failure.message);
        }
        // The same steps the other way round: the kept quest completes, the
        // mode is left, and the held quest's step comes after it.
        let w = walk_modes(&c, &firing(&c, SIDE_LAST, true)).unwrap();
        assert_eq!(w.spans, [("mode/aside".to_string(), 1, Some(2))]);
        assert_eq!(w.judged, 1);
    }

    #[test]
    fn a_mode_keeping_both_quests_is_left_when_the_last_completes() {
        let c = campaign(&[MAIN, "quest/side"], start(), json!([enter()]));
        let w = walk_modes(&c, &firing(&c, SIDE_FIRST, true)).unwrap();
        assert_eq!(w.spans, [("mode/aside".to_string(), 1, Some(3))]);
        assert_eq!(w.judged, 2);
    }

    #[test]
    fn a_leave_the_path_fires_leaves_and_an_entry_it_does_not_fire_enters_nothing() {
        let c = campaign(
            &[MAIN],
            start(),
            json!([enter(), {"type": "leave-mode", "mode": "mode/aside"}]),
        );
        let w = walk_modes(&c, &firing(&c, SIDE_FIRST, true)).unwrap();
        assert_eq!(w.spans, [("mode/aside".to_string(), 1, Some(1))]);
        let c = campaign(&[MAIN], start(), json!([enter()]));
        let w = walk_modes(&c, &firing(&c, SIDE_FIRST, false)).unwrap();
        assert_eq!(w, ModeWalk::default());
    }

    #[test]
    fn a_mode_whose_kept_quest_has_not_started_is_left_at_its_entry() {
        let c = campaign(
            &["quest/side"],
            json!({"type": "quest-complete", "quest": MAIN}),
            json!([enter()]),
        );
        let w = walk_modes(&c, &firing(&c, SIDE_LAST, true)).unwrap();
        assert_eq!(w.spans, [("mode/aside".to_string(), 1, Some(1))]);
        assert_eq!(w.judged, 0);
    }
}
