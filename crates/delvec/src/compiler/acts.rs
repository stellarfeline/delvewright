//! **The acts on every walked path** (spec-0104 §1): the one reading of "what
//! the party must do, where, and from where it stands", shared by the rules
//! that judge an act — that it can be found (`DW0863`).
//!
//! An **act** is a thing the party must do at a place to advance a walked
//! path: every objective step of every kind, and every `use`, `strike`,
//! `strike-npc` or `strike-assembly` trigger the path performs
//! ([`Step::Trigger`]). An `approach` or `step` trigger is a place on the
//! route, not a thing to press, and is not an act.
//!
//! The walks are the exported critical path and each realised branch's own
//! path — the walks [`crate::compiler::flow::Flow::journal`] takes for
//! `DW0982` — each read off the plan's own steps, so an act's cell is the cell
//! the bot is sent to. The cell the party **stands on** before an act is the
//! cell of the preceding positioned step, or where that step's completion
//! carries the party when it carries it (an inter-area crossing, a link); for
//! the first act, the campaign's entry cell.

use std::collections::BTreeMap;

use delvewright_dsl::{EnvTrigger, Objective, TriggerOn};

use crate::compiler::plan::{Plan, Step};

/// What an act is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActWhat {
    /// An objective: its quest index and objective index in stage 5.
    Objective {
        /// Index into `quests.content.quests`.
        quest: usize,
        /// Index into that quest's `objectives`.
        objective: usize,
    },
    /// A trigger the path performs, by id.
    Trigger(String),
}

/// One act on one walk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Act {
    /// Its index among the walk's steps.
    pub step: usize,
    /// The objective's or trigger's id.
    pub id: String,
    /// What it is.
    pub what: ActWhat,
    /// Its kind tag: the objective's `type` or the trigger's event.
    pub kind: &'static str,
    /// The cell the party stands on before it; `None` when the walk names no
    /// entry and nothing precedes it.
    pub from: Option<[i32; 3]>,
    /// What [`Self::from`] is, for a message: `the entry cell`, or the
    /// preceding step.
    pub from_what: String,
    /// The act's target cell: the step's own cell.
    pub target: [i32; 3],
    /// Whether the target is a body (a person, a wave, a place stood in) —
    /// sighted at chest height — rather than a block, sighted at its centre.
    pub body: bool,
}

/// A branch path's own region writes, in its own step space, with its step
/// ancestry ([`Plan::branch_gate_model`]).
pub type WalkGates = std::sync::Arc<(
    crate::compiler::plan::RegionEvents,
    BTreeMap<usize, std::collections::BTreeSet<usize>>,
)>;

/// One walked path's acts.
#[derive(Clone, Debug, Default)]
pub struct ActWalk {
    /// The walk's name for a message: `the critical path`, `branch <id>`.
    pub label: String,
    /// Its acts in path order.
    pub acts: Vec<Act>,
    /// The region writes the walk's steps make, which decide what blocks a
    /// view at each act; `None` = the plan's own (the exported path's).
    pub gates: Option<WalkGates>,
}

/// Every walk's acts, derived once per plan.
#[derive(Clone, Debug, Default)]
pub struct ActWalks {
    /// The exported critical path first, then each realised branch in
    /// enumeration order.
    pub walks: Vec<ActWalk>,
}

/// Whether a trigger is an act (spec-0104 §1): a `use`, `strike`,
/// `strike-npc` or `strike-assembly` is a thing the party presses; an
/// `approach` or `step` is a place on the route.
pub fn trigger_is_act(t: &EnvTrigger) -> bool {
    !matches!(t.on, TriggerOn::Approach { .. } | TriggerOn::Step)
}

/// The acts of one path's steps, `transport` aligned 1:1 with them, departing
/// from the campaign's entry cell.
pub fn acts_of(
    plan: &Plan,
    label: &str,
    steps: &[Step],
    transport: &[Option<[i32; 3]>],
) -> ActWalk {
    acts_of_from(
        plan,
        label,
        steps,
        transport,
        plan.campaign_start().map(|(_, p)| p),
    )
}

/// [`acts_of`] departing from `entry`, the cell the first act is judged from.
pub fn acts_of_from(
    plan: &Plan,
    label: &str,
    steps: &[Step],
    transport: &[Option<[i32; 3]>],
    entry: Option<[i32; 3]>,
) -> ActWalk {
    let c = plan.campaign;
    let mut objectives: BTreeMap<&str, (usize, usize, &Objective)> = BTreeMap::new();
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            objectives.insert(o.id().as_str(), (qi, oi, o));
        }
    }
    let triggers = c.quests.content.all_triggers();
    let mut from: Option<([i32; 3], String)> = entry.map(|p| (p, "the entry cell".to_string()));
    let mut acts = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        let Some(pos) = step.pos() else { continue };
        let act = if let Some(oid) = step.objective() {
            objectives.get(oid).map(|(qi, oi, o)| Act {
                step: i,
                id: oid.to_string(),
                what: ActWhat::Objective {
                    quest: *qi,
                    objective: *oi,
                },
                kind: o.kind(),
                from: from.as_ref().map(|f| f.0),
                from_what: from.as_ref().map(|f| f.1.clone()).unwrap_or_default(),
                target: pos,
                body: match o {
                    Objective::Interact { .. } => false,
                    Objective::Collect { dropped_by, .. } => dropped_by.is_some(),
                    _ => true,
                },
            })
        } else if let Some(tid) = step.trigger() {
            triggers
                .iter()
                .find(|t| t.id.as_str() == tid)
                .filter(|t| trigger_is_act(t))
                .map(|t| Act {
                    step: i,
                    id: tid.to_string(),
                    what: ActWhat::Trigger(tid.to_string()),
                    kind: t.on.kind(),
                    from: from.as_ref().map(|f| f.0),
                    from_what: from.as_ref().map(|f| f.1.clone()).unwrap_or_default(),
                    target: pos,
                    body: matches!(
                        t.on,
                        TriggerOn::StrikeNpc { .. } | TriggerOn::StrikeAssembly { .. }
                    ),
                })
        } else {
            None
        };
        if let Some(a) = act {
            acts.push(a);
        }
        let name = step
            .objective()
            .map(|o| format!("`{o}`"))
            .or_else(|| step.trigger().map(|t| format!("`{t}`")))
            .unwrap_or_else(|| format!("step {i}"));
        from = Some(match transport.get(i).copied().flatten() {
            Some(dest) => (dest, format!("where {name} carries the party")),
            None => (pos, format!("{name}'s cell")),
        });
    }
    ActWalk {
        label: label.to_string(),
        acts,
        gates: None,
    }
}

/// Every walk's acts: the exported critical path, then each realised branch
/// (a branch whose path cannot be built is skipped — the branch proofs refuse
/// it by name). Derived once per plan; the build fixes it first, with the
/// links its route proof took on each branch ([`fix_walks`]), so every reader
/// after it reads the walks the bot is handed.
pub fn walks<'p>(plan: &'p Plan<'_>) -> &'p ActWalks {
    plan.acts
        .get_or_init(|| derive_walks(plan, &BTreeMap::new()))
}

/// Fix the plan's walks with each branch's taken links (`slug` → takes), as
/// `emit::build` knows them once the world exists. A no-op when the walks
/// were already derived.
pub fn fix_walks(plan: &Plan, takes: &BTreeMap<String, crate::compiler::plan::LinkTakes>) {
    let _ = plan.acts.set(derive_walks(plan, takes));
}

fn derive_walks(
    plan: &Plan,
    takes: &BTreeMap<String, crate::compiler::plan::LinkTakes>,
) -> ActWalks {
    let mut out = ActWalks::default();
    out.walks.push(acts_of(
        plan,
        "the critical path",
        &plan.critical_path,
        &plan.critical_path_transport,
    ));
    let realized = crate::compiler::branch::realize(plan.campaign);
    if !realized.is_empty() {
        let flow = crate::compiler::flow::Flow::new(plan.campaign);
        let none = crate::compiler::plan::LinkTakes::default();
        for r in &realized {
            let Some(w) = r.world else { continue };
            let Ok(cp) = plan.branch_critical_path_linked(
                &flow,
                &flow.playthrough_in(w),
                takes.get(&r.branch.slug).unwrap_or(&none),
            ) else {
                continue;
            };
            let mut walk = acts_of(
                plan,
                &format!("branch `{}`", r.branch.id),
                &cp.steps,
                &cp.transport_by_step,
            );
            walk.gates = Some(std::sync::Arc::new(plan.branch_gate_model(&cp)));
            out.walks.push(walk);
        }
    }
    out
}
