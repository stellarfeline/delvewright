//! Races (spec-0139): a course of checkpoints a racer is credited in order.
//!
//! A racer is a body with two scores of the race's own: how many checkpoints
//! it has been credited (`dw.rc.<race>`) and which checkpoint of the lap is
//! its next (`dw.rn.<race>`). While the race runs (`#ron_<race>` in
//! `dw.sys`), the root `tick` runs `race_<race>_body` as every player and as
//! every actor the race lists. The body function asks one question per
//! checkpoint — is this racer's next this one, and does its hitbox meet the
//! box (`@s[x=…,dx=…]`, the pinned selector's volume) — and credits at most
//! one a tick. A player whose hitbox meets a checkpoint further on in the lap
//! than its next is credited nothing and reads the race's `missed` line.
//!
//! A racer credited the last checkpoint of the last lap has finished: it
//! takes the next place on the race's board by the board's own placing
//! function. The race is over (`race_<race>_end`) when no racer in the world
//! is short of the finish, when its `grace_ticks` have passed since its
//! first finisher (a `schedule` the first finish makes and a start clears),
//! or when an `end-race` says so; being over latches `#rfin_<race>`, the
//! term an objective that declares the race its `course` waits on.

use super::*;
use delvewright_dsl::{BodyKey, Race, Verb};

/// The objective a racer's credited checkpoints are counted in.
pub(crate) fn race_progress(id: &str) -> String {
    format!("dw.rc.{}", plan::safe_local(id))
}

/// The objective a racer's next checkpoint of the lap is kept in (0-based).
pub(crate) fn race_next(id: &str) -> String {
    format!("dw.rn.{}", plan::safe_local(id))
}

/// The `dw.sys` holder that is 1 while race `id` runs.
pub(crate) fn race_running(id: &str) -> String {
    format!("#ron_{}", plan::safe_local(id))
}

/// The `dw.sys` holder that is 1 once race `id` is over, until it is started
/// again.
pub(crate) fn race_over(id: &str) -> String {
    format!("#rfin_{}", plan::safe_local(id))
}

/// The `dw.sys` holder that is 1 once a racer of race `id` has finished.
pub(crate) fn race_first(id: &str) -> String {
    format!("#r1st_{}", plan::safe_local(id))
}

/// The function that starts race `id`.
pub(crate) fn race_start_fn(id: &str) -> String {
    format!("race_{}_start", plan::safe_local(id))
}

/// The function that ends race `id`.
pub(crate) fn race_end_fn(id: &str) -> String {
    format!("race_{}_end", plan::safe_local(id))
}

/// The function the tick runs as each racer of race `id`.
pub(crate) fn race_body_fn(id: &str) -> String {
    format!("race_{}_body", plan::safe_local(id))
}

/// The function that credits the racer it runs as its next checkpoint.
fn race_pass_fn(id: &str) -> String {
    format!("race_{}_pass", plan::safe_local(id))
}

/// The function that finishes the racer it runs as.
pub(crate) fn race_finish_fn(id: &str) -> String {
    format!("race_{}_finish", plan::safe_local(id))
}

/// The selector terms that pick an actor racer's body: the tag the engine
/// puts on the puppet and its twin, never the answer box that wears it too.
pub(crate) fn racer_terms(actor: &str) -> String {
    format!(
        "tag=dw_actor_{},type=!minecraft:interaction",
        plan::safe_local(actor)
    )
}

/// A race's checkpoints as resolved boxes, in course order. A checkpoint
/// whose anchor no placed piece provides is skipped; validation names it
/// (`DW0142`).
pub(crate) fn race_boxes(plan: &Plan, r: &Race) -> Vec<([i32; 3], [i32; 3])> {
    r.checkpoints
        .iter()
        .filter_map(|cp| plan.zone_box(cp))
        .collect()
}

/// `setup` lines: each race's two objectives and its two latches, at rest.
/// Empty for a campaign that declares no race.
pub(super) fn race_setup(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for r in &plan.campaign.quests.content.races {
        let id = r.id.as_str();
        out.push(format!(
            "scoreboard objectives add {} dummy",
            race_progress(id)
        ));
        out.push(format!("scoreboard objectives add {} dummy", race_next(id)));
        out.push(format!(
            "scoreboard players set {} dw.sys 0",
            race_running(id)
        ));
        out.push(format!("scoreboard players set {} dw.sys 0", race_over(id)));
        out.push(format!(
            "scoreboard players set {} dw.sys 0",
            race_first(id)
        ));
    }
    out
}

/// The root `tick`'s lines: while a race runs, its body function as every
/// player and as every actor it lists. Empty for a campaign that declares no
/// race.
pub(super) fn race_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for r in &plan.campaign.quests.content.races {
        let id = r.id.as_str();
        let on = format!("execute if score {} dw.sys matches 1", race_running(id));
        let body = race_body_fn(id);
        out.push(format!("{on} as @a run function {ns}:{body}"));
        for a in &r.racers {
            out.push(format!(
                "{on} as @e[{}] run function {ns}:{body}",
                racer_terms(a.as_str())
            ));
        }
    }
    out
}

/// The race functions: per race its start, its end, the body the tick runs
/// as each racer, the credit and the finish.
pub(super) fn race_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for r in &plan.campaign.quests.content.races {
        let id = r.id.as_str();
        let (p, nx) = (race_progress(id), race_next(id));
        let boxes = race_boxes(plan, r);
        let n = boxes.len();
        let total = r.total();
        let (on, over, first) = (race_running(id), race_over(id), race_first(id));
        let end = format!("{ns}:{}", race_end_fn(id));
        out.push((
            race_start_fn(id),
            lines(&[
                format!("schedule clear {end}"),
                format!("scoreboard players reset * {p}"),
                format!("scoreboard players reset * {nx}"),
                format!("scoreboard players set {first} dw.sys 0"),
                format!("scoreboard players set {over} dw.sys 0"),
                format!("scoreboard players set {on} dw.sys 1"),
            ]),
        ));
        out.push((
            race_end_fn(id),
            lines(&[
                format!("execute unless score {on} dw.sys matches 1 run return 0"),
                format!("schedule clear {end}"),
                format!("scoreboard players set {on} dw.sys 0"),
                format!("scoreboard players set {over} dw.sys 1"),
            ]),
        ));
        // The body: seed a racer the start found absent, leave a finisher
        // alone, credit the next checkpoint, else tell a player what it
        // missed.
        let mut body = vec![
            format!("execute unless score @s {p} matches 0.. run scoreboard players set @s {nx} 0"),
            format!("execute unless score @s {p} matches 0.. run scoreboard players set @s {p} 0"),
            format!("execute if score @s {p} matches {total}.. run return 0"),
        ];
        for (j, (lo, hi)) in boxes.iter().enumerate() {
            body.push(format!(
                "execute if score @s {nx} matches {j} if entity @s[{}] run return run function \
                 {ns}:{}",
                box_selector_args(*lo, *hi),
                race_pass_fn(id)
            ));
        }
        let missed = tr(&r.missed);
        for (j, (lo, hi)) in boxes.iter().enumerate().skip(1) {
            body.push(format!(
                "execute if score @s {nx} matches ..{} if entity @s[type=minecraft:player,{}] \
                 run return run title @s actionbar {missed}",
                j - 1,
                box_selector_args(*lo, *hi),
            ));
        }
        out.push((race_body_fn(id), lines(&body)));
        let mut pass = vec![
            format!("scoreboard players add @s {p} 1"),
            format!("scoreboard players add @s {nx} 1"),
            format!("execute if score @s {nx} matches {n}.. run scoreboard players set @s {nx} 0"),
            format!(
                "execute if score @s {p} matches {total}.. run return run function {ns}:{}",
                race_finish_fn(id)
            ),
        ];
        if let Some(text) = &r.passed {
            pass.push(format!(
                "execute if entity @s[type=minecraft:player] run title @s actionbar {}",
                json!([
                    tr(text),
                    { "text": " " },
                    { "score": { "name": "@s", "objective": p } },
                    { "text": format!("/{total}") }
                ])
            ));
        }
        out.push((race_pass_fn(id), lines(&pass)));
        let mut finish: Vec<String> = Vec::new();
        if let Some(b) = &r.board {
            finish.push(format!(
                "execute if entity @s[type=minecraft:player] run function {ns}:{}",
                place_player_fn(b.as_str())
            ));
            for a in &r.racers {
                finish.push(format!(
                    "execute if entity @s[{}] run function {ns}:{}",
                    racer_terms(a.as_str()),
                    place_body_fn(b.as_str(), &BodyKey::actor(a.clone()))
                ));
            }
        }
        // The first finish starts the grace; every finish asks whether a
        // racer in the world is still short of the finish.
        if let Some(g) = r.grace_ticks {
            finish.push(format!(
                "execute unless score {first} dw.sys matches 1 run schedule function {end} {g}t \
                 replace"
            ));
        }
        finish.push(format!("scoreboard players set {first} dw.sys 1"));
        let short = format!("scores={{{p}=..{}}}", total.saturating_sub(1));
        let mut all = format!("execute unless entity @a[{short}]");
        for a in &r.racers {
            all.push_str(&format!(
                " unless entity @e[{},{short}]",
                racer_terms(a.as_str())
            ));
        }
        finish.push(format!("{all} run function {end}"));
        out.push((race_finish_fn(id), lines(&finish)));
    }
    out
}

/// What a race verb runs.
pub(super) fn emit_race_verb(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    match &eff.verb {
        Verb::StartRace { race } => {
            body.push(format!("function {ns}:{}", race_start_fn(race.as_str())));
        }
        Verb::EndRace { race } => {
            body.push(format!("function {ns}:{}", race_end_fn(race.as_str())));
        }
        _ => {}
    }
}

/// The term an objective that declares a `course` waits on beside its gate:
/// that race is over. Empty for every other objective.
pub(super) fn course_term(o: &Objective) -> String {
    o.course().map_or_else(String::new, |r| {
        format!(" if score {} dw.sys matches 1", race_over(r.as_str()))
    })
}
