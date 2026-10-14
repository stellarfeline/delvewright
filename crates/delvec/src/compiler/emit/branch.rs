//! The per-branch critical paths.

use super::*;

/// One executable critical path per REACHABLE enumerated branch (spec-0025 §3),
/// as `(slug, json)` in branch-enumeration order.
///
/// Empty — so byte-identical — for a campaign that declares no `branch_points`.
/// An unreachable branch contributes nothing: there is no world that plays it,
/// which `DW0482` has already failed the build for; `branch-plan.json` still names
/// it, so the harness reports it skipped rather than silently absent.
pub(super) fn branch_paths(
    plan: &Plan,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    takes: &BTreeMap<String, plan::LinkTakes>,
    legs: &[(String, Vec<plan::Step>, Vec<crate::compiler::nav::LegRoute>)],
) -> Result<Vec<(String, Value)>, BuildFailure> {
    let branches = crate::compiler::branch::realize(plan.campaign);
    if branches.is_empty() {
        return Ok(Vec::new());
    }
    let flow = crate::compiler::flow::Flow::new(plan.campaign);
    let mut out = Vec::new();
    for r in &branches {
        let Some(w) = r.world else { continue };
        let none = plan::LinkTakes::default();
        let cp = plan
            .branch_critical_path_linked(
                &flow,
                &flow.playthrough_in(w),
                takes.get(&r.branch.slug).unwrap_or(&none),
            )
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.failure.code,
                message: format!("branch `{}`: {}", r.branch.id, e.failure.message),
            })?;
        let routes = legs
            .iter()
            .find(|(slug, _, _)| *slug == r.branch.slug)
            .map(|(_, _, routes)| routes.as_slice())
            .unwrap_or(&[]);
        let en_route = crate::compiler::hold::en_route_holds(plan, &cp.steps, routes);
        out.push((
            r.branch.slug.clone(),
            critical_path_json(
                plan,
                &cp.steps,
                &cp.transport_by_step,
                &cp.sneak_by_step,
                CutsceneHolds {
                    after: &cp.cutscene_by_step,
                    en_route: &en_route,
                },
                moves,
                actor_moves,
            ),
        ));
    }
    Ok(out)
}
