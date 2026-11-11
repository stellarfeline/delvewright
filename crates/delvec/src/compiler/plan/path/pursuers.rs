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
    /// What its kind hears (`delvewright_dsl::senses`), or `None` for a kind
    /// that hears nothing the engine knows of.
    pub hearing: Option<&'static delvewright_dsl::senses::KindHearing>,
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
        // spec-0132 §5.2: the kind's hearing, which the bot waits out before
        // a move it cannot make silently. Present only for a kind that hears.
        if let (Some(h), Some(o)) = (self.hearing, v.as_object_mut()) {
            o.insert(
                "hearing".to_string(),
                serde_json::json!({"reach": h.reach, "forget_ticks": h.forget_ticks}),
            );
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
            hearing: delvewright_dsl::senses::kind_hearing(&a.entity),
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
