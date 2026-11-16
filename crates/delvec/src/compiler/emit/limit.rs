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
pub(super) fn seconds_holder(oid: &str) -> String {
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
    // A quorum member settles done or lapsed (spec-0134 §7), as a limited
    // objective settles done or expired.
    if owns_settled(c, oid) {
        settled_score(oid)
    } else {
        obj_score(oid)
    }
}

/// Whether `oid` owns a settled score: it can settle some way other than
/// done — expired (a `limit`), lapsed (a quorum member) or failed (named by a
/// `fail-objective`).
pub(super) fn owns_settled(c: &delvewright_dsl::Campaign, oid: &str) -> bool {
    limited(c, oid).is_some() || super::quorum::is_member(c, oid) || failable(c, oid)
}

/// Whether some `fail-objective` anywhere in the campaign names `oid`: it can
/// settle **failed**.
pub(super) fn failable(c: &delvewright_dsl::Campaign, oid: &str) -> bool {
    delvewright_dsl::failed_objectives(c).contains(oid)
}

/// The scoreboard objective marking an objective **failed** (party holder):
/// set by `settle_failed_<obj>`, read by its guards.
pub(super) fn failed_score(oid: &str) -> String {
    plan::failed_score(oid)
}

/// The function that settles `oid` failed.
pub(super) fn fail_fn(oid: &str) -> String {
    format!("settle_failed_{}", safe_obj_fn(oid))
}

/// The line a `fail-objective` emits: the objective settled failed, unless it
/// has already settled some way.
pub(super) fn fail_lines(ns: &str, oid: &str) -> Vec<String> {
    vec![format!(
        "execute unless score {} {} matches 1 run function {ns}:{}",
        plan::PARTY,
        settled_score(oid),
        fail_fn(oid)
    )]
}

/// The line `complete_<obj>` gains for a failable objective that owns its
/// settled score through nothing else: settled.
pub(super) fn failure_completion_lines(c: &delvewright_dsl::Campaign, oid: &str) -> Vec<String> {
    if failable(c, oid) && limited(c, oid).is_none() && !super::quorum::is_member(c, oid) {
        vec![format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            settled_score(oid)
        )]
    } else {
        Vec::new()
    }
}

/// The lines that put a failable objective's failure back to a fresh
/// delve's: its failed score clear, and its settled score where nothing else
/// owns it — what every PackTest template that drives the objective owns, so a
/// sibling's leftover failure never decides its assertion (`DW0807`).
pub(super) fn failure_clear_lines(c: &delvewright_dsl::Campaign, oid: &str) -> Vec<String> {
    if !failable(c, oid) {
        return Vec::new();
    }
    let mut out = vec![format!(
        "scoreboard players set {} {} 0",
        plan::PARTY,
        failed_score(oid)
    )];
    if limited(c, oid).is_none() && !super::quorum::is_member(c, oid) {
        out.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            settled_score(oid)
        ));
    }
    out
}

/// `setup` lines: the failed score of every failable objective, and its
/// settled score where nothing else owns it.
pub(super) fn failure_setup_lines(plan: &Plan) -> Vec<String> {
    let c = plan.campaign;
    let mut out = Vec::new();
    for o in c.quests.content.quests.iter().flat_map(|q| &q.objectives) {
        let oid = o.id().as_str();
        if !failable(c, oid) {
            continue;
        }
        out.push(format!(
            "scoreboard objectives add {} dummy",
            failed_score(oid)
        ));
        if limited(c, oid).is_none() && !super::quorum::is_member(c, oid) {
            out.push(format!(
                "scoreboard objectives add {} dummy",
                settled_score(oid)
            ));
        }
    }
    out
}

/// Every failable objective's `settle_failed_<obj>`: failed and settled, its
/// clock stopped, its closing told to a party that was told of it, its
/// affordance gone as completion's goes, and its quest asked whether it is
/// complete. Nothing of its completion bundle or its `on_expire` fires: the
/// bundle that failed it is the story's account of how it was lost.
pub(super) fn failure_fns(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    holders: &[crate::compiler::holders::OwnedHolder],
    guide: &guidance::Guide,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns = Vec::new();
    for q in &c.quests.content.quests {
        let q_area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            let oid = o.id().as_str();
            if !failable(c, oid) {
                continue;
            }
            let mut body = vec![
                format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    failed_score(oid)
                ),
                format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    settled_score(oid)
                ),
            ];
            if o.limit().is_some() {
                body.extend(stop_settlement_lines(ns, oid));
            }
            // Told only to a party that was told of it: an objective failed
            // before it opened was never announced, and its closing names
            // nothing the party has heard of.
            if o.announced(&c.quests.content.guidance)
                && let Some(title) = o.title()
            {
                let told = format!(
                    "execute if score {} {} matches 1 run ",
                    plan::PARTY,
                    announce_score(oid)
                );
                body.push(format!(
                    "{told}tellraw @a {}",
                    tr_with(
                        &chrome.get(delvewright_dsl::chrome::LIMIT_FAILED),
                        &[
                            ("color", json!("red")),
                            (
                                "with",
                                json!([tr_with(title, &[("color", json!("white"))])])
                            ),
                        ],
                    )
                ));
                body.push(format!("{told}{}", cue_line(CUE_EXPIRED)));
            }
            if !activation_commands(plan, holders, q_area, o).is_empty() {
                body.extend(completion_cleanup(plan, o));
            }
            body.extend(guidance::completion_lines(guide, ns, oid));
            body.push(format!(
                "function {ns}:check_q_{}",
                plan::safe_local(q.id.as_str())
            ));
            fns.push((fail_fn(oid), lines(&body)));
        }
    }
    fns
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
/// tick — each in replace mode, so the clock cannot double up. The bar's
/// players are set by the draw ([`show_fn`]), not here: it runs every second,
/// so a member who joins after the start is shown the bar within one.
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
        format!("function {ns}:{}", show_fn(oid)),
        format!("schedule function {ns}:{} 20t replace", tick_fn(oid)),
        format!(
            "schedule function {ns}:{} {}t replace",
            expire_fn(oid),
            l.ticks
        ),
    ]
}

/// The lines a settlement that is not the clock's own runs on a limited
/// objective (a lapse, spec-0134 §7): its expiry unscheduled, its countdown
/// stopped.
pub(super) fn stop_settlement_lines(ns: &str, oid: &str) -> Vec<String> {
    let mut out = vec![format!("schedule clear {ns}:{}", expire_fn(oid))];
    out.extend(stop_lines(ns, oid));
    out
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
        out.push(format!(
            "scoreboard players set {} dw.sys 0",
            ran_out_latch(oid)
        ));
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
            // A quest active at world load starts its clocks at the first
            // player's arrival, never at tick 1 of an empty world: the same
            // first-join hook `join_place` keys on (`dw_joined`), or — in a
            // campaign with no spawn anchor, which places nobody — the first
            // player online.
            let arrived = if matches!(q.trigger, Trigger::CampaignStart) {
                if super::world::campaign_spawn(plan).is_some() {
                    " if entity @a[tag=dw_joined]"
                } else {
                    " if entity @a"
                }
            } else {
                ""
            };
            // spec-0140: a held quest starts no clock until it is held no
            // longer.
            let held = super::mode::held_term(plan.campaign, q.id.as_str());
            format!(
                "execute if score {} {} matches 1{held}{arrived} unless score {} dw.sys matches 1 run function {ns}:{}",
                plan::PARTY,
                quest_active_score(q.id.as_str()),
                quest_start_latch(q.id.as_str()),
                quest_start_fn(q.id.as_str())
            )
        })
        .collect()
}

/// The `dw.sys` latch a limited objective's clock sets when it runs out with
/// the objective not done (spec-0133 §3): read by its born-expired driver.
pub(super) fn ran_out_latch(oid: &str) -> String {
    format!("#lx_{}", plan::safe_local(oid))
}

/// The tick drivers that settle **born expired** (spec-0133 §3): a limited
/// objective whose clock starts before it opens — at its quest's start or at
/// an ancestor's activation — and has already run out when it opens settles
/// expired that tick, its `on_expire` firing and the story going on. An
/// objective clocked from its own activation cannot open after its clock
/// ran out, so it has none.
pub(super) fn born_expired_drivers(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for q in &plan.campaign.quests.content.quests {
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            let Some(l) = o.limit() else { continue };
            if l.from.is_activation() {
                continue;
            }
            let oid = o.id().as_str();
            // The objective's open guard, never its gate: a clock settles a
            // task whatever its completion's gate says.
            out.push(format!(
                "execute if score {} dw.sys matches 1{} run function {ns}:{}",
                ran_out_latch(oid),
                serving_guard(plan.campaign, o, &qa),
                settle_fn(oid)
            ));
        }
    }
    out
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
            // first is left alone. One not yet open when a clock started
            // before it runs out is marked run out (`#lx_<obj>`): it is born
            // expired the tick it opens ([`born_expired_drivers`]), never left
            // with no window at all.
            fns.push((
                expire_fn(oid),
                lines(&[
                    format!(
                        "execute unless score {} {} matches 1 run scoreboard players set {} dw.sys 1",
                        plan::PARTY,
                        obj_score(oid),
                        ran_out_latch(oid)
                    ),
                    // The open guard, never the gate (spec-0133 §3 as
                    // amended): a body the task needed may be lost, and a
                    // clock that waited on its gate would never settle.
                    format!(
                        "execute{} run function {ns}:{}",
                        serving_guard(c, o, &qa),
                        settle_fn(oid)
                    ),
                ]),
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
                    // Restated every second (spec-0141 §6): a member who joins
                    // the delve after the clock started is shown its bar.
                    format!("bossbar set {bar} players @a"),
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
