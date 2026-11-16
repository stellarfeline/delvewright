//! Modes (spec-0140): a state of the map in which every quest but the ones
//! the mode keeps is held.
//!
//! The map's mode is one party score, `#party dw.mode`: `0` in no mode, else
//! the mode's place in `modes[]`, from one. A quest some mode holds reads it
//! in every guard its objectives are told, served and completed under
//! ([`held_term`]) — one more term beside the quest's active score — and in
//! the driver that starts its `quest-start` clocks. So a held objective is
//! announced by nothing, followed by no compass, on no journal page,
//! completed by no act and no line, and no clock of it starts.
//!
//! A clock already running when its quest becomes held stands still: the
//! mode's entry clears its two schedules and hides its bar, remembering in
//! `#mh_<obj>` that it ran, and [`MODE_OFF_FN`] schedules them again from the
//! seconds the bar showed.
//!
//! A mode is left by `leave-mode`, by another mode's entry, and by itself:
//! `mode_<m>_settle` — run at the entry and at the completion of every quest
//! the mode keeps — leaves it when none of its kept quests is in progress.

use super::*;
use delvewright_dsl::{Mode, Verb};

/// The scoreboard objective the map's mode is held in (party holder).
pub(crate) const MODE_SCORE: &str = "dw.mode";

/// The function that puts the map in no mode and starts every held clock
/// again.
pub(crate) const MODE_OFF_FN: &str = "mode_off";

/// The storage the resumed clock's delay is handed to its macro in.
const MODE_STORAGE: &str = "dw:mode";

/// The function that puts the map in mode `id`.
pub(crate) fn mode_enter_fn(id: &str) -> String {
    format!("mode_{}_enter", plan::safe_local(id))
}

/// The function that leaves mode `id` when none of its kept quests is in
/// progress.
pub(crate) fn mode_settle_fn(id: &str) -> String {
    format!("mode_{}_settle", plan::safe_local(id))
}

/// The `dw.sys` holder that is 1 while the clock of limited objective `oid`
/// stands still under a mode.
pub(crate) fn clock_held(oid: &str) -> String {
    format!("#mh_{}", plan::safe_local(oid))
}

/// The function that starts the held clock of `oid` again.
fn resume_fn(oid: &str) -> String {
    format!("mode_resume_{}", safe_obj_fn(oid))
}

/// The macro function that schedules the expiry of `oid` `$(t)` ticks on.
fn resched_fn(oid: &str) -> String {
    format!("mode_resched_{}", safe_obj_fn(oid))
}

/// The `matches` ranges covering exactly `numbers` (ascending): each maximal
/// run as `a..b`, a single number as `a`.
fn runs(numbers: &[usize]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < numbers.len() {
        let mut j = i;
        while j + 1 < numbers.len() && numbers[j + 1] == numbers[j] + 1 {
            j += 1;
        }
        out.push(if i == j {
            numbers[i].to_string()
        } else {
            format!("{}..{}", numbers[i], numbers[j])
        });
        i = j + 1;
    }
    out
}

/// **The guard term a quest's objectives carry for the modes that hold it**:
/// one `unless score #party dw.mode matches <run>` per maximal run of the
/// numbers of the modes that do not keep `quest`. Empty for a campaign with
/// no mode and for a quest every mode keeps, so such a guard is
/// byte-identical.
pub(crate) fn held_term(c: &delvewright_dsl::Campaign, quest: &str) -> String {
    held_ranges(c, quest)
        .iter()
        .map(|r| format!(" unless score {} {MODE_SCORE} matches {r}", plan::PARTY))
        .collect()
}

/// **The `#party dw.mode` values under which `quest` is held**, as
/// `matches` ranges: one per maximal run of the numbers of the modes that do
/// not keep it. Empty for a quest no mode holds. [`held_term`] reads them, and
/// the journal ledger hands them to the bot.
pub(crate) fn held_ranges(c: &delvewright_dsl::Campaign, quest: &str) -> Vec<String> {
    runs(&c.quests.content.modes_holding(quest))
}

/// [`held_term`] for the quest that holds objective `oid`.
pub(crate) fn held_term_of_objective(c: &delvewright_dsl::Campaign, oid: &str) -> String {
    plan::objective_quest(c, oid).map_or_else(String::new, |(q, _)| held_term(c, q))
}

/// Whether some mode holds `quest`: its guards read the map's mode.
pub(crate) fn holds_some(c: &delvewright_dsl::Campaign, quest: &str) -> bool {
    !c.quests.content.modes_holding(quest).is_empty()
}

/// The line a PackTest that opens a guard of `quest` writes: the map in no
/// mode. Empty for a quest no mode holds.
pub(crate) fn mode_drive(c: &delvewright_dsl::Campaign, quest: &str) -> Vec<String> {
    if holds_some(c, quest) {
        vec![format!(
            "scoreboard players set {} {MODE_SCORE} 0",
            plan::PARTY
        )]
    } else {
        Vec::new()
    }
}

/// Every limited objective a mode can hold — its quest is not kept by some
/// mode — with its quest, in declaration order.
pub(crate) fn holdable_clocks(
    c: &delvewright_dsl::Campaign,
) -> Vec<(&delvewright_dsl::Quest, &Objective)> {
    c.quests
        .content
        .quests
        .iter()
        .filter(|q| holds_some(c, q.id.as_str()))
        .flat_map(|q| q.objectives.iter().map(move |o| (q, o)))
        .filter(|(_, o)| o.limit().is_some())
        .collect()
}

/// The limited objectives mode `m` holds, in declaration order.
pub(crate) fn clocks_held_by<'a>(
    c: &'a delvewright_dsl::Campaign,
    m: &Mode,
) -> Vec<(&'a delvewright_dsl::Quest, &'a Objective)> {
    holdable_clocks(c)
        .into_iter()
        .filter(|(q, _)| !m.keeps(q.id.as_str()))
        .collect()
}

/// `setup` lines: the mode objective, and the party's entry in it created
/// without being moved — a reload leaves the map in the mode it was in.
/// Empty for a campaign that declares no mode.
pub(super) fn mode_setup(plan: &Plan) -> Vec<String> {
    if plan.campaign.quests.content.modes.is_empty() {
        return Vec::new();
    }
    vec![
        format!("scoreboard objectives add {MODE_SCORE} dummy"),
        format!("scoreboard players add {} {MODE_SCORE} 0", plan::PARTY),
    ]
}

/// The lines `complete_q_<quest>` gains: every mode that keeps the quest is
/// asked whether anything it keeps is still in progress.
pub(super) fn quest_completion_lines(plan: &Plan, quest: &str) -> Vec<String> {
    let ns = &plan.namespace;
    plan.campaign
        .quests
        .content
        .modes
        .iter()
        .filter(|m| m.keeps(quest))
        .map(|m| format!("function {ns}:{}", mode_settle_fn(m.id.as_str())))
        .collect()
}

/// Every function a mode owns. Empty for a campaign that declares no mode.
pub(super) fn mode_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let q = &c.quests.content;
    let p = plan::PARTY;
    let mut fns = Vec::new();
    if q.modes.is_empty() {
        return fns;
    }
    for (i, m) in q.modes.iter().enumerate() {
        let n = i + 1;
        let id = m.id.as_str();
        // The entry: nothing for a mode already on; whichever mode was on is
        // left first, its held clocks running again; then this mode's held
        // clocks stop, and the mode is left at once when nothing it keeps
        // is in progress.
        let mut enter = vec![
            format!("execute if score {p} {MODE_SCORE} matches {n} run return 0"),
            format!("function {ns}:{MODE_OFF_FN}"),
            format!("scoreboard players set {p} {MODE_SCORE} {n}"),
        ];
        for (_, o) in clocks_held_by(c, m) {
            let oid = o.id().as_str();
            let held = clock_held(oid);
            // `schedule clear` answers how many it cleared: one for a clock
            // that runs, a failure — stored as 0 — for one that does not.
            enter.push(format!("scoreboard players set {held} dw.sys 0"));
            enter.push(format!(
                "execute store result score {held} dw.sys run schedule clear {ns}:{}",
                expire_fn(oid)
            ));
            enter.push(format!(
                "execute if score {held} dw.sys matches 1.. run schedule clear {ns}:{}",
                super::limit::tick_fn(oid)
            ));
            enter.push(format!(
                "execute if score {held} dw.sys matches 1.. run bossbar set {} visible false",
                super::limit::bar_id(ns, oid)
            ));
        }
        enter.push(format!("function {ns}:{}", mode_settle_fn(id)));
        fns.push((mode_enter_fn(id), lines(&enter)));

        // The settlement: a mode with a kept quest in progress — started and
        // not complete — stays on; any other is left.
        let mut settle = vec![format!(
            "execute unless score {p} {MODE_SCORE} matches {n} run return 0"
        )];
        for k in &m.keeps {
            settle.push(format!(
                "execute if score {p} {} matches 1 unless score {p} {} matches 1 run return 0",
                quest_active_score(k.as_str()),
                quest_score(k.as_str())
            ));
        }
        settle.push(format!("function {ns}:{MODE_OFF_FN}"));
        fns.push((mode_settle_fn(id), lines(&settle)));
    }

    // No mode: the score at rest, and every clock a mode stopped started
    // again.
    let mut off = vec![format!("scoreboard players set {p} {MODE_SCORE} 0")];
    for (_, o) in holdable_clocks(c) {
        let oid = o.id().as_str();
        off.push(format!(
            "execute if score {} dw.sys matches 1.. run function {ns}:{}",
            clock_held(oid),
            resume_fn(oid)
        ));
        let secs = super::limit::seconds_holder(oid);
        fns.push((
            resume_fn(oid),
            lines(&[
                format!("scoreboard players set {} dw.sys 0", clock_held(oid)),
                // Settled while it was held (a `fail-objective`): its clock
                // was stopped for good.
                format!(
                    "execute if score {p} {} matches 1 run return 0",
                    settled_score(oid)
                ),
                format!(
                    "execute if score {secs} dw.sys matches ..0 run scoreboard players set {secs} dw.sys 1"
                ),
                format!(
                    "bossbar set {} visible true",
                    super::limit::bar_id(ns, oid)
                ),
                format!(
                    "schedule function {ns}:{} 20t replace",
                    super::limit::tick_fn(oid)
                ),
                format!(
                    "execute store result storage {MODE_STORAGE} t int 20 run scoreboard players get {secs} dw.sys"
                ),
                format!(
                    "function {ns}:{} with storage {MODE_STORAGE}",
                    resched_fn(oid)
                ),
            ]),
        ));
        fns.push((
            resched_fn(oid),
            lines(&[format!(
                "$schedule function {ns}:{} $(t)t replace",
                expire_fn(oid)
            )]),
        ));
    }
    fns.push((MODE_OFF_FN.to_string(), lines(&off)));
    fns
}

/// The lines a mode verb emits (spec-0140 §3).
pub(super) fn emit_mode_verb(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    let q = &plan.campaign.quests.content;
    match &eff.verb {
        Verb::EnterMode { mode } => {
            body.push(format!("function {ns}:{}", mode_enter_fn(mode.as_str())));
        }
        Verb::LeaveMode { mode } => {
            if let Some(n) = q.mode_number(mode.as_str()) {
                body.push(format!(
                    "execute if score {} {MODE_SCORE} matches {n} run function {ns}:{MODE_OFF_FN}",
                    plan::PARTY
                ));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::runs;

    #[test]
    fn a_run_is_one_range_and_a_gap_starts_another() {
        assert!(runs(&[]).is_empty());
        assert_eq!(runs(&[2]), ["2"]);
        assert_eq!(runs(&[1, 2, 3]), ["1..3"]);
        assert_eq!(runs(&[1, 3, 4, 6]), ["1", "3..4", "6"]);
    }
}
