//! A beat that settles two ways (spec-0133 §3): an objective's `limit`, the
//! clock that closes it, and the countdown the party watches.
//!
//! The clock is the server's own `schedule` in replace mode — no per-tick
//! cost, never doubled up, kept across a restart — started once, by the
//! function that runs on the limit's start event: the objective's own
//! `activate_<obj>`, the quest's `activate_q_<quest>`, or a named ancestor's
//! `activate_<obj>`. It ends in one of two settlements: `complete_<obj>`
//! clears it, or `expire_<obj>` finds the objective still pending and runs
//! `settle_expired_<obj>`. Every function here is emitted only for an
//! objective that declares a `limit`, so a campaign with none is
//! byte-identical.

use super::*;

/// The `dw.sys` holder of a limited objective's seconds left.
fn seconds_holder(oid: &str) -> String {
    format!("#lt_{}", plan::safe_local(oid))
}

/// The scoreboard objective marking a limited objective **expired** (party
/// holder): set by `settle_expired_<obj>`, read by its pending guard.
pub(super) fn expired_score(oid: &str) -> String {
    format!("dw.x_{}", plan::safe_local(oid))
}

/// The scoreboard objective marking a limited objective **settled** — done or
/// expired (party holder). `dw.os_`, because `dw.s_` is a datum's. Every reading of "`after` this objective" and the
/// quest's completion check read it in place of the objective's done score,
/// so both settlements let the line go on.
pub(super) fn settled_score(oid: &str) -> String {
    format!("dw.os_{}", plan::safe_local(oid))
}

/// The score an `after` term or a quest's completion check reads for `oid`:
/// its settled score when it carries a limit, else its done score.
pub(super) fn after_score(c: &delvewright_dsl::Campaign, oid: &str) -> String {
    if limited(c, oid).is_some() {
        settled_score(oid)
    } else {
        obj_score(oid)
    }
}

/// The objective `oid`'s limit, when it declares one.
pub(super) fn limited<'a>(
    c: &'a delvewright_dsl::Campaign,
    oid: &str,
) -> Option<&'a delvewright_dsl::Limit> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .find(|o| o.id().as_str() == oid)
        .and_then(Objective::limit)
}

/// The bossbar a limited objective's countdown is drawn on.
pub(super) fn bar_id(ns: &str, oid: &str) -> String {
    format!("{ns}:limit_{}", plan::safe_local(oid))
}

/// The function `oid`'s clock runs when it runs out.
pub(super) fn expire_fn(oid: &str) -> String {
    format!("expire_{}", safe_obj_fn(oid))
}

/// The function that settles `oid` expired.
pub(super) fn settle_fn(oid: &str) -> String {
    format!("settle_expired_{}", safe_obj_fn(oid))
}

/// The countdown's once-a-second step.
pub(super) fn tick_fn(oid: &str) -> String {
    format!("limit_tick_{}", safe_obj_fn(oid))
}

/// The countdown's redraw.
fn show_fn(oid: &str) -> String {
    format!("limit_show_{}", safe_obj_fn(oid))
}

/// The function a quest's `quest-start` clocks start in.
pub(super) fn quest_start_fn(qid: &str) -> String {
    format!("activate_q_{}", plan::safe_local(qid))
}

/// The `dw.sys` once-latch of [`quest_start_fn`].
pub(super) fn quest_start_latch(qid: &str) -> String {
    format!("#actq_{}", plan::safe_local(qid))
}

/// The seconds a window of `ticks` shows at its start: rounded up, so the bar
/// never reads `0:00` while the clock still runs.
fn seconds_of(ticks: u32) -> u32 {
    ticks.div_ceil(20)
}

/// Every limited objective whose clock starts when objective `start`
/// activates: `start` itself under `from: activation`, and every objective
/// naming `start` under `from: {objective}`. In declaration order.
pub(super) fn clocks_started_by_objective<'a>(
    c: &'a delvewright_dsl::Campaign,
    start: &str,
) -> Vec<&'a Objective> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter(|o| match o.limit().map(|l| &l.from) {
            Some(delvewright_dsl::LimitFrom::Activation) => o.id().as_str() == start,
            Some(delvewright_dsl::LimitFrom::Objective(s)) => s.as_str() == start,
            _ => false,
        })
        .collect()
}

/// Every limited objective of quest `q` whose clock starts when the quest
/// activates (`from: quest-start`).
pub(super) fn clocks_started_by_quest(q: &delvewright_dsl::Quest) -> Vec<&Objective> {
    q.objectives
        .iter()
        .filter(|o| {
            matches!(
                o.limit().map(|l| &l.from),
                Some(delvewright_dsl::LimitFrom::QuestStart)
            )
        })
        .collect()
}

/// The lines that start `o`'s clock: the countdown drawn for the party, its
/// once-a-second step scheduled, and the expiry scheduled at the declared
/// tick — each in replace mode, so the clock cannot double up.
pub(super) fn start_lines(ns: &str, o: &Objective) -> Vec<String> {
    let Some(l) = o.limit() else {
        return Vec::new();
    };
    let oid = o.id().as_str();
    let bar = bar_id(ns, oid);
    let secs = seconds_of(l.ticks);
    vec![
        format!(
            "scoreboard players set {} dw.sys {secs}",
            seconds_holder(oid)
        ),
        format!("bossbar add {bar} \"\""),
        format!("bossbar set {bar} max {secs}"),
        format!("bossbar set {bar} color yellow"),
        format!("bossbar set {bar} players @a"),
        format!("function {ns}:{}", show_fn(oid)),
        format!("schedule function {ns}:{} 20t replace", tick_fn(oid)),
        format!(
            "schedule function {ns}:{} {}t replace",
            expire_fn(oid),
            l.ticks
        ),
    ]
}

/// The lines either settlement runs to stop `oid`'s countdown: the bar
/// removed, its step unscheduled.
fn stop_lines(ns: &str, oid: &str) -> Vec<String> {
    vec![
        format!("schedule clear {ns}:{}", tick_fn(oid)),
        format!("bossbar remove {}", bar_id(ns, oid)),
    ]
}

/// The lines `complete_<obj>` gains for a limited objective: settled, and
/// its clock cleared, so a done objective is never expired.
pub(super) fn limit_completion_lines(ns: &str, o: &Objective) -> Vec<String> {
    if o.limit().is_none() {
        return Vec::new();
    }
    let oid = o.id().as_str();
    let mut out = vec![
        format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            settled_score(oid)
        ),
        format!("schedule clear {ns}:{}", expire_fn(oid)),
    ];
    out.extend(stop_lines(ns, oid));
    out
}

/// `setup` lines: the two party scores every limited objective owns, and a
/// bar left by an earlier run of the same world removed.
pub(super) fn limit_setup_lines(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for o in plan
        .campaign
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter(|o| o.limit().is_some())
    {
        let oid = o.id().as_str();
        out.push(format!(
            "scoreboard objectives add {} dummy",
            expired_score(oid)
        ));
        out.push(format!(
            "scoreboard objectives add {} dummy",
            settled_score(oid)
        ));
        out.push(format!("bossbar remove {}", bar_id(ns, oid)));
    }
    out
}

/// The tick drivers of the quest-start clocks: a quest that starts a clock
/// runs [`quest_start_fn`] once, the tick it is active.
pub(super) fn quest_start_drivers(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    plan.campaign
        .quests
        .content
        .quests
        .iter()
        .filter(|q| !clocks_started_by_quest(q).is_empty())
        .map(|q| {
            format!(
                "execute if score {} {} matches 1 unless score {} dw.sys matches 1 run function {ns}:{}",
                plan::PARTY,
                quest_active_score(q.id.as_str()),
                quest_start_latch(q.id.as_str()),
                quest_start_fn(q.id.as_str())
            )
        })
        .collect()
}

/// Every function a limit owns: the quest-start activations, and per limited
/// objective its expiry, its settlement, its countdown step and redraw.
pub(super) fn limit_fns(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    holders: &[crate::compiler::holders::OwnedHolder],
    guide: &guidance::Guide,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    for q in &c.quests.content.quests {
        let started = clocks_started_by_quest(q);
        if !started.is_empty() {
            let mut body = vec![format!(
                "scoreboard players set {} dw.sys 1",
                quest_start_latch(q.id.as_str())
            )];
            for o in started {
                body.extend(start_lines(ns, o));
            }
            fns.push((quest_start_fn(q.id.as_str()), lines(&body)));
        }
        let qa = quest_active_score(q.id.as_str());
        let q_area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            let Some(limit) = o.limit() else { continue };
            let oid = o.id().as_str();
            // The clock settles only an objective still pending: one done
            // first, or not yet open, is left alone.
            fns.push((
                expire_fn(oid),
                lines(&[format!(
                    "execute{} run function {ns}:{}",
                    pending_guard(plan, o, &qa),
                    settle_fn(oid)
                )]),
            ));
            let mut settle: Vec<String> = vec![
                format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    expired_score(oid)
                ),
                format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    settled_score(oid)
                ),
                // The machine marker, first, as a completion's is: the harness
                // passes an `expire` step on it alone.
                format!(
                    "tellraw @a {}",
                    json!({
                        "text": plan::expired_marker_line(ns, oid),
                        "color": "dark_gray"
                    })
                ),
            ];
            if o.announced(&c.quests.content.guidance)
                && let Some(title) = o.title()
            {
                settle.push(format!(
                    "tellraw @a {}",
                    tr_with(
                        &chrome.get(delvewright_dsl::chrome::LIMIT_EXPIRED),
                        &[
                            ("color", json!("red")),
                            (
                                "with",
                                json!([tr_with(title, &[("color", json!("white"))])])
                            ),
                        ],
                    )
                ));
                settle.push(cue_line(CUE_EXPIRED));
            }
            settle.extend(stop_lines(ns, oid));
            // The objective's affordance goes as completion's does.
            if !activation_commands(plan, holders, q_area, o).is_empty() {
                settle.extend(completion_cleanup(plan, o));
            }
            settle.extend(guidance::completion_lines(guide, ns, oid));
            settle.extend(emit_effect_bundle(
                plan,
                limit.on_expire(),
                root_audience(delvewright_dsl::EffectRootKind::ObjectiveExpire),
            ));
            settle.push(format!(
                "function {ns}:check_q_{}",
                plan::safe_local(q.id.as_str())
            ));
            fns.push((settle_fn(oid), lines(&settle)));
            // The countdown: one second down, redrawn, and the next step
            // scheduled while any time is left.
            let secs = seconds_holder(oid);
            fns.push((
                tick_fn(oid),
                lines(&[
                    format!("scoreboard players remove {secs} dw.sys 1"),
                    format!("function {ns}:{}", show_fn(oid)),
                    format!(
                        "execute if score {secs} dw.sys matches 1.. run schedule function {ns}:{} 20t replace",
                        tick_fn(oid)
                    ),
                ]),
            ));
            let title = o.title().unwrap_or(oid);
            let bar = bar_id(ns, oid);
            let m = format!("#ltm_{}", plan::safe_local(oid));
            let t = format!("#ltt_{}", plan::safe_local(oid));
            let u = format!("#ltu_{}", plan::safe_local(oid));
            let score = |h: &str| json!({ "score": { "name": h, "objective": "dw.sys" } });
            let name = tr_with(
                &chrome.get(delvewright_dsl::chrome::LIMIT_COUNTDOWN),
                &[
                    ("color", json!("yellow")),
                    (
                        "with",
                        json!([
                            tr(title),
                            [score(&m), { "text": ":" }, score(&t), score(&u)]
                        ]),
                    ),
                ],
            );
            fns.push((
                show_fn(oid),
                lines(&[
                    "scoreboard players set #c60 dw.sys 60".to_string(),
                    "scoreboard players set #c10 dw.sys 10".to_string(),
                    format!("scoreboard players operation {m} dw.sys = {secs} dw.sys"),
                    format!("scoreboard players operation {m} dw.sys /= #c60 dw.sys"),
                    format!("scoreboard players operation {t} dw.sys = {secs} dw.sys"),
                    format!("scoreboard players operation {t} dw.sys %= #c60 dw.sys"),
                    format!("scoreboard players operation {u} dw.sys = {t} dw.sys"),
                    format!("scoreboard players operation {u} dw.sys %= #c10 dw.sys"),
                    format!("scoreboard players operation {t} dw.sys /= #c10 dw.sys"),
                    format!(
                        "execute store result bossbar {bar} value run scoreboard players get {secs} dw.sys"
                    ),
                    format!("bossbar set {bar} name {name}"),
                ]),
            ));
        }
    }
    fns
}

/// The cue an announced objective plays when its clock runs out.
pub(super) const CUE_EXPIRED: &str = "minecraft:block.note_block.bass";
