//! A window measured against the walk it allows (spec-0133 §6, `DW1084`).
//!
//! For each limited objective the critical path completes, the walk the
//! window must at least hold: the proven route the path walks from the step
//! at which the limit's clock starts to the objective's own step, priced at
//! the sprint model every other walk price in this module reads
//! ([`SPRINT_TICKS_PER_BLOCK`]). A window shorter than that walk is a task no
//! party can do. The walk is a floor, never a judgement of the window: a
//! fight, a search, a shot are activity, and how much room the window leaves
//! for them is the author's call and the playtest's.
//!
//! A limited objective the critical path does not complete — one in an
//! optional quest, or on a branch the path does not take — is counted as
//! unwalked and never judged.

use super::*;
use crate::compiler::plan::Step;

use delvewright_dsl::{LimitFrom, Objective, Trigger};

delvewright_dsl::dw_code! {
    /// (spec-0133 §7) **A limit shorter than the walk it allows**: the window
    /// of a limited objective on the critical path is fewer ticks than the
    /// proven walk from the clock's start to the objective, at the sprint model.
    pub const DW_LIMIT_SHORTER_THAN_WALK: DwCode = DwCode::new("DW1084", ExitTier::Build);
}

/// One limited objective, measured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LimitRow {
    /// The objective.
    pub objective: String,
    /// The window, in ticks.
    pub ticks: u32,
    /// The start event, as the DSL spells it.
    pub from: String,
    /// The walk's start cell and its length in blocks, `None` when the path
    /// does not complete the objective.
    pub walk: Option<([i32; 3], u32)>,
}

impl LimitRow {
    /// The walk in ticks at the sprint model.
    pub fn walk_ticks(&self) -> Option<u32> {
        self.walk.map(|(_, b)| b * SPRINT_TICKS_PER_BLOCK)
    }
}

/// What `DW1084` measured on a build: every limit, its walk, and the expiry
/// worlds the build exports with what waiting them out costs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LimitBinding {
    /// One row per limited objective, in declaration order.
    pub rows: Vec<LimitRow>,
    /// The expiry witness worlds exported, and the summed `wait_ticks` of
    /// their `expire` steps.
    pub expiry_worlds: usize,
    /// Summed `wait_ticks` over every exported expiry witness.
    pub wait_ticks: u64,
    /// Limited objectives whose expiry witness the enumeration did not keep
    /// (past its cap): unproven, never counted as walked.
    pub unproven: Vec<String>,
}

impl LimitBinding {
    /// `validation/limit.json`: the line's numbers, for a reader that is not
    /// a person.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "limits": self.rows.iter().map(|r| serde_json::json!({
                "objective": r.objective,
                "ticks": r.ticks,
                "from": r.from,
                "walk_from": r.walk.map(|(c, _)| c),
                "walk_blocks": r.walk.map(|(_, b)| b),
                "walk_ticks": r.walk_ticks(),
            })).collect::<Vec<_>>(),
            "expiry_worlds": self.expiry_worlds,
            "wait_ticks": self.wait_ticks,
            "unproven": self.unproven,
        })
    }

    /// The build line (spec-0133 §6).
    pub fn line(&self) -> String {
        let walked: Vec<String> = self
            .rows
            .iter()
            .map(|r| match r.walk_ticks() {
                Some(w) => {
                    let pct = if r.ticks == 0 {
                        0
                    } else {
                        u64::from(w) * 100 / u64::from(r.ticks)
                    };
                    format!("walk {w} of {} tick(s) ({pct}%) — {}", r.ticks, r.objective)
                }
                None => format!("unwalked, {} tick(s) — {}", r.ticks, r.objective),
            })
            .collect();
        let mut s = format!(
            "limit binding (DW1084): {} objective(s) carry a limit",
            self.rows.len()
        );
        if !walked.is_empty() {
            s.push_str("; ");
            s.push_str(&walked.join("; "));
        }
        s.push_str(&format!(
            "; {} expiry world(s) exported, {} tick(s) of waiting",
            self.expiry_worlds, self.wait_ticks
        ));
        if !self.unproven.is_empty() {
            s.push_str(&format!(
                "; unproven (no world kept): {}",
                self.unproven.join(", ")
            ));
        }
        s
    }
}

/// The critical-path step index each objective completes at.
fn obj_steps<'a>(plan: &'a Plan<'_>) -> std::collections::BTreeMap<&'a str, usize> {
    plan.critical_path
        .iter()
        .enumerate()
        .filter(|(_, s)| !matches!(s, Step::Expire { .. }))
        .filter_map(|(i, s)| s.objective().map(|o| (o, i)))
        .collect()
}

/// The step at which `o` (in quest `q`) activates on the path: after its last
/// `after` prerequisite and after the step that opens its quest; `0` (the
/// spawn) when neither is on the path.
fn activation_step(
    plan: &Plan,
    steps: &std::collections::BTreeMap<&str, usize>,
    q: &delvewright_dsl::Quest,
    o: &Objective,
) -> usize {
    let mut at = quest_start_step(plan, steps, q);
    for a in o.after() {
        if let Some(&s) = steps.get(a.as_str()) {
            at = at.max(s);
        }
    }
    at
}

/// The step whose completion opens quest `q`: the last step of the quest its
/// trigger names, `0` for a campaign-start quest.
fn quest_start_step(
    plan: &Plan,
    steps: &std::collections::BTreeMap<&str, usize>,
    q: &delvewright_dsl::Quest,
) -> usize {
    match &q.trigger {
        Trigger::CampaignStart => 0,
        Trigger::QuestComplete { quest } => plan
            .campaign
            .quests
            .content
            .quests
            .iter()
            .find(|p| p.id == *quest)
            .map(|p| {
                p.objectives
                    .iter()
                    .filter_map(|x| steps.get(x.id().as_str()).copied())
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0),
    }
}

/// Measure every limit against the critical path's proven legs and refuse a
/// window shorter than its walk (`DW1084`).
pub fn judge_limits(
    plan: &Plan,
    routes: &[LegRoute],
) -> (LimitBinding, Result<(), crate::compiler::failure::Failure>) {
    let steps = obj_steps(plan);
    let start_cell = plan.campaign_start().map(|(_, p)| p);
    let mut binding = LimitBinding::default();
    let mut refused: Vec<String> = Vec::new();
    for q in &plan.campaign.quests.content.quests {
        for o in &q.objectives {
            let Some(l) = o.limit() else { continue };
            let oid = o.id().as_str();
            let mut row = LimitRow {
                objective: oid.to_string(),
                ticks: l.ticks,
                from: l.from.token(),
                walk: None,
            };
            if let Some(&t) = steps.get(oid) {
                let s = match &l.from {
                    LimitFrom::Activation => activation_step(plan, &steps, q, o),
                    LimitFrom::QuestStart => quest_start_step(plan, &steps, q),
                    LimitFrom::Objective(x) => q
                        .objectives
                        .iter()
                        .find(|y| y.id() == x)
                        .map_or(0, |y| activation_step(plan, &steps, q, y)),
                };
                let legs: Vec<&LegRoute> = routes
                    .iter()
                    .filter(|r| r.to_step > s && r.to_step <= t)
                    .collect();
                let blocks: u32 = legs
                    .iter()
                    .map(|r| u32::try_from(r.cells.len().saturating_sub(1)).unwrap_or(u32::MAX))
                    .sum();
                let from_cell = legs
                    .iter()
                    .min_by_key(|r| r.to_step)
                    .and_then(|r| r.cells.first().copied())
                    .or(start_cell)
                    .unwrap_or([0, 0, 0]);
                row.walk = Some((from_cell, blocks));
                let walk = blocks * SPRINT_TICKS_PER_BLOCK;
                if walk > l.ticks {
                    refused.push(format!(
                        "objective `{oid}` has a window of {} tick(s) from `{}`, but the proven \
                         walk from where the party stands when its clock starts — [{}, {}, {}], \
                         step {s} of the critical path — to its own step is {blocks} block(s), \
                         {walk} tick(s) at {SPRINT_TICKS_PER_BLOCK} t/block: no party can do the \
                         task in time. Lengthen the window to at least {walk} tick(s) and leave \
                         room for what the task asks beyond the walk, or shorten the route.",
                        l.ticks,
                        l.from.token(),
                        from_cell[0],
                        from_cell[1],
                        from_cell[2],
                    ));
                }
            }
            binding.rows.push(row);
        }
    }
    let verdict = if refused.is_empty() {
        Ok(())
    } else {
        Err(crate::compiler::failure::Failure::new(
            DW_LIMIT_SHORTER_THAN_WALK,
            refused.join("\n"),
        ))
    };
    (binding, verdict)
}
