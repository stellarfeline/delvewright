//! **A numeric gate is judged against the writes the path performs**
//! (`DW0879`).
//!
//! ## The gap this closes
//!
//! The engine's own gallery shipped a delve that could not be finished, and
//! every static verb was green about it. `obj/reach-the-end` was gated on
//! `state/labels-read at-least 1`; three objectives earlier,
//! `obj/clear-the-muster`'s completion bundle performed a `clear-state` on that
//! same datum, returning it to `0`. `delvec analyze` exported the path as
//! reachable, the bot ladder walked as far as the mezzanine, and the finale's
//! gate could never open.
//!
//! Nothing was wrong with any of the checks that looked:
//!
//! | Check | What it asks | Why it is silent here |
//! |---|---|---|
//! | `DW0501` | is this datum written **anywhere** | it is — three times |
//! | `DW0502` | is it read anywhere | it is |
//! | `DW0503` | can the reading site reach its scope | it can |
//! | `DW0527` | does a gate read a datum an **earlier effect of the same bundle** wrote | the write and the read are four beats apart |
//! | `DW0203`/`DW0204` | is the path reachable and walkable | the reachability model walks objectives and flags; it never evaluated the arithmetic a `requires_state` compares |
//!
//! Each of the five is correct and each quantifies over something other than
//! *the value the datum holds at the moment a later gate reads it*. That
//! quantifier needs an ORDER, and the campaign model has exactly one thing that
//! has an order: [`crate::flow::Flow`]'s path replay. So this rule is not a new
//! mechanism beside the replay — it is the replay's binding widened from flags
//! to the whole gate, and this module is the diagnostic surface over it, exactly
//! as [`crate::analyze`] is the diagnostic surface over the reachability
//! fixpoint.
//!
//! ## What is walked
//!
//! Two orders, and both are orders the engine itself exports:
//!
//! * the **critical path** — [`Flow::playthrough`], the participation-minimal
//!   walk `DW0204` already proves is a playthrough. A gate that cannot open here
//!   makes the delve unfinishable.
//! * **every enumerated branch world's own whole path** —
//!   [`Flow::playthrough_in`], which is a `dag_order` over everything that
//!   completes in that world, so it reaches optional strands the finale-rooted
//!   critical path never visits, and it reaches a branch the critical path is
//!   not on. A finding already named on the critical path is not named again.
//!
//! ## What is refused, and what is deliberately withheld
//!
//! Only a gate whose failure **no play order avoids**. The emitter's
//! `pending_guard` lets a player complete any activatable objective at any
//! moment, so two beats with no `after` between them can be played either way
//! round; refusing a gate that fails under one of those orders and holds under
//! the other would be a false certainty. [`Flow::state_gates`] therefore refuses
//! only where the writes the walk applied are chained into one order by the
//! campaign's own `after` and `quest-complete` relations, and that chain is
//! forced to finish before the gate. Everything else is counted as withheld and
//! said out loud in the binding line.
//!
//! The same reasoning one layer down decides which data are judged at all: a
//! datum an ambient producer, a reaction bundle or a stake forfeit can move is
//! not a function of the path, so no comparison against it is ever refused. That
//! set is named and counted — see `Flow`'s `undatable_state`.
//!
//! ## Where this runs
//!
//! Beside [`crate::promise::check`] and [`crate::branch::check_branches`], in
//! the one validation funnel every subcommand goes through, so a delve that
//! cannot be finished cannot reach a datapack by skipping `delvec analyze`. It
//! is guarded on the path being coherent in the first place: a campaign whose
//! finale is unreachable (`DW0201`) or whose path does not replay (`DW0204`) has
//! a fault upstream of this one, and naming a consequence of it as well would
//! bury the cause.

use delvewright_dsl::{Campaign, Diagnostic};

use crate::flow::{DW_STATE_GATE_CLEARED, Flow, StateWalk};

/// What this run examined — the binding count, stated whether or not anything
/// was found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatePathBinding {
    /// The walk's own counts, summed over every path walked.
    pub walk: StateWalk,
    /// How many paths were walked (the critical path plus one per branch world).
    pub paths: usize,
    /// Set when nothing was walked because the campaign has a fault upstream of
    /// this rule — the reason, for the line.
    pub not_judged: Option<&'static str>,
}

impl StatePathBinding {
    /// The binding line this check states on every run.
    pub fn line(&self) -> String {
        if let Some(why) = self.not_judged {
            return format!("state path binding: 0 path(s) walked — {why}");
        }
        format!(
            "state path binding: {p} path(s) walked over {s} step(s); {g} numeric gate term(s) \
             read, of which {n} against an undatable datum and {h} withheld (the order is not \
             forced); {w} state write(s) replayed; {d} declared datum(s) of which {u} undatable",
            p = self.paths,
            s = self.walk.steps,
            g = self.walk.gates,
            n = self.walk.undated,
            h = self.walk.withheld,
            w = self.walk.writes,
            d = self.walk.data,
            u = self.walk.undatable,
        )
    }
}

/// Run the rule. `DW0879` per forced-path gate the path itself has cleared.
pub fn check(c: &Campaign) -> (Vec<Diagnostic>, StatePathBinding) {
    let flow = Flow::new(c);
    let mut b = StatePathBinding::default();
    let main = flow.playthrough();
    if main.degenerate {
        b.not_judged = Some(
            "no branch of this campaign completes its finale, so there is no path to read a \
             gate at (`DW0201` names the cause)",
        );
        return (Vec::new(), b);
    }
    if flow.replay(&main).is_err() {
        b.not_judged = Some(
            "the exported critical path is not a playthrough any player can walk, so the value \
             a gate would be read against is not established (`DW0204` names the cause)",
        );
        return (Vec::new(), b);
    }

    let mut d = Vec::new();
    // Keyed on the gate, not the objective: one objective may read two data, and
    // both are the author's to fix. The critical path is walked first, so its
    // position and its (absent) branch label are the ones a reader sees.
    let mut seen: std::collections::BTreeSet<(String, String, String, i32)> =
        std::collections::BTreeSet::new();
    let mut push = |flow: &Flow<'_>, path: &crate::flow::Playthrough, branch: Option<&str>| {
        let (found, walk) = flow.state_gates(path, branch);
        b.walk.merge(walk);
        b.paths += 1;
        for f in found {
            let key = (
                f.objective.clone(),
                f.state.clone(),
                f.op.token().to_string(),
                f.value,
            );
            if !seen.insert(key) {
                continue;
            }
            d.push(Diagnostic::error(
                DW_STATE_GATE_CLEARED,
                "quests",
                crate::analyze::objective_path(c, &f.objective),
                f.message(),
            ));
        }
    };
    push(&flow, &main, None);

    let main_world = flow.playthrough_world();
    for i in 0..flow.world_count() {
        let pt = flow.playthrough_in(i);
        let label = branch_label(&flow, i, main_world);
        push(&flow, &pt, label.as_deref());
    }

    d.sort_by(|a, x| (&a.code, &a.path, &a.message).cmp(&(&x.code, &x.path, &x.message)));
    (d, b)
}

/// How a message names branch world `i`: by the flags it holds that the world
/// the critical path came from does not.
///
/// A reader has no use for an enumeration index, and the flags are the thing
/// they actually chose — `flag/flee` is the branch, and the world is only how
/// this model spells it. A world with no distinguishing flag is the mainline's
/// own, and needs no label at all.
fn branch_label(flow: &Flow<'_>, i: usize, main_world: Option<usize>) -> Option<String> {
    if main_world == Some(i) {
        return None;
    }
    let mine = flow.world_flags(i);
    let theirs = main_world.map(|m| flow.world_flags(m)).unwrap_or_default();
    let extra: Vec<String> = mine.difference(&theirs).map(|f| format!("`{f}`")).collect();
    if extra.is_empty() {
        return Some("an alternative playthrough of this campaign".to_string());
    }
    Some(format!("the playthrough that sets {}", extra.join(", ")))
}
