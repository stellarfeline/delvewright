//! **A task the story loses** (spec-0133 §1 as amended): where a
//! `fail-objective` can stand so that the story worlds date it, and `DW1112`
//! where it cannot.
//!
//! A failure settles its objective at the moment its bundle fires. The story
//! worlds date that moment in two kinds of root:
//!
//! * a quest's bundles — an objective's completion, its quest's completion, a
//!   limit's `on_expire` — which fire at their step of every walk, so the
//!   failure is dated there and the objective it names settles with nothing of
//!   its own firing;
//! * an **elective** ambient act's bundle — a kill credited to a player, a
//!   trigger no path is forced through, a purchase — which a world performs or
//!   skips. Every such act that fails an objective is a world of its own (the
//!   all world with it performed, origin `failure`), its deed performed in
//!   front of the first step of the objective it fails.
//!
//! A root the walks never fire in order — a forced ambient act, a death, a
//! respawn, a loop's crossing, a strike's landing — has no moment a proof can
//! name, so a failure in it would settle a task at runtime that every proof
//! believes open: `DW1112`.

use delvewright_dsl::{Campaign, Diagnostic, DwCode, EffectRootOwner, ExitTier};

use crate::compiler::flow::Flow;

delvewright_dsl::dw_code! {
    /// `DW1112`: a `fail-objective` in a root no story world dates — a forced
    /// ambient act, or a root fired by an event no walk performs — so the
    /// proofs would hold open a task the delve has lost. Build-tier.
    pub const DW_FAILURE_UNDATED: DwCode = DwCode::new("DW1112", ExitTier::Build);
}

/// What [`check`] examined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LossBinding {
    /// `fail-objective` sites, at any depth.
    pub sites: usize,
    /// Sites in a quest bundle, dated at their step.
    pub dated: usize,
    /// Sites in an elective act's bundle, each act a world.
    pub elective: usize,
    /// Elective acts that fail an objective — the failure worlds.
    pub acts: usize,
    /// Sites refused (`DW1112`).
    pub refused: usize,
}

impl LossBinding {
    /// The line printed on every run that reads a campaign with a
    /// `fail-objective`.
    pub fn line(&self) -> String {
        format!(
            "failure binding (DW1112): {} fail-objective site(s) — {} in a quest bundle, {} in \
             {} elective act(s), each a failure world — {} refused",
            self.sites, self.dated, self.elective, self.acts, self.refused
        )
    }
}

/// **`DW1112`**: every `fail-objective` stands in a root the story worlds
/// date.
pub fn check(c: &Campaign) -> (Vec<Diagnostic>, LossBinding) {
    let mut d = Vec::new();
    let mut b = LossBinding::default();
    if delvewright_dsl::failed_objectives(c).is_empty() {
        return (d, b);
    }
    let flow = Flow::new(c);
    let electives: std::collections::BTreeSet<String> =
        flow.electives().iter().map(|e| e.id.clone()).collect();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        let mut n = 0usize;
        for e in list {
            e.visit_deep(&mut |x| {
                if x.fail_objective().is_some() {
                    n += 1;
                }
            });
        }
        if n == 0 {
            return;
        }
        b.sites += n;
        match &site.owner {
            EffectRootOwner::ObjectiveComplete { .. }
            | EffectRootOwner::QuestComplete { .. }
            | EffectRootOwner::ObjectiveExpire { .. } => b.dated += n,
            other => {
                let act = crate::compiler::ending::act_of_owner(other, &site.path);
                if act.as_ref().is_some_and(|a| electives.contains(a)) {
                    b.elective += n;
                } else {
                    b.refused += n;
                    let why = if act.is_some() {
                        "an act every story world performs wherever its gate holds (a fight a \
                         mandatory `kill` slays, a trigger the path is forced through), whose \
                         bundle no walk fires at a step"
                    } else {
                        "a root no walk fires at a step (a death, a respawn, a loop's crossing, \
                         a strike's landing)"
                    };
                    d.push(Diagnostic::error(
                        DW_FAILURE_UNDATED,
                        site.stage,
                        site.path.clone(),
                        format!(
                            "this bundle fails an objective (`fail-objective`) from {why} ({}). \
                             No story world can say when the task is lost, so every proof would \
                             hold it open after the delve has settled it. Put the failure on an \
                             event a walk performs: an objective's completion, or an act a \
                             player may choose — a credited kill of a fight no `kill` requires, \
                             a trigger off the path.",
                            site.kind().label()
                        ),
                    ));
                }
            }
        }
    });
    b.acts = flow.failing_acts().len();
    (d, b)
}
