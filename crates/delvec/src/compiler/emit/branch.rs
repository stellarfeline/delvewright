//! Branch transport and the per-branch critical paths.

use super::*;

/// One executable critical path per REACHABLE enumerated branch (spec-0025 §3),
/// as `(slug, json)` in branch-enumeration order.
///
/// Empty — so byte-identical — for a campaign that declares no `branch_points`.
/// An unreachable branch contributes nothing: there is no world that plays it,
/// which `DW0482` has already failed the build for; `branch-plan.json` still names
/// it, so the harness reports it skipped rather than silently absent.
/// One branch-only inter-area crossing: where to set the party down, and the
/// flag assignment that identifies the branch it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchTransport {
    /// Flags pinned SET on the branch this crossing belongs to.
    pub set: BTreeSet<String>,
    /// Flags pinned UNSET on that branch.
    pub unset: BTreeSet<String>,
    /// The branch's slug — the row's deterministic sort key, and the name of the
    /// `branch-path-<slug>.json` this crossing is the datapack half of.
    pub slug: String,
    /// The destination area's `spawn` anchor, in world coordinates.
    pub pos: [i32; 3],
}

/// Objective id → the branch-only crossings completing it must perform.
pub type BranchTransportOverlay = BTreeMap<String, Vec<BranchTransport>>;

/// The crossings that exist on a BRANCH but not on the exported path.
///
/// [`crate::compiler::plan::build_critical_path`] derives an inter-area transport map for
/// whatever playthrough it is handed, so every branch's map already exists via
/// [`Plan::branch_critical_path`] — and `branch-path-<slug>.json` publishes it to
/// the harness. Emission, however, reads only `plan.transport`, the **exported**
/// path's map. A campaign whose branch alone leaves the starting area therefore
/// promised the harness a crossing the datapack never performed, and the branch
/// run stranded where the teleport should have been (island round 21).
///
/// This is that difference, ready to be emitted as flag-gated teleports beside
/// the unconditional one. A crossing the exported path already performs is
/// omitted: it is emitted unconditionally, which is stronger. A crossing whose
/// destination *contradicts* the exported path's is [`DW_BRANCH_TRANSPORT_DIVERGES`]
/// — one objective cannot land the party in two areas.
///
/// Empty — so byte-identical to the pre-task emission — for a campaign with no
/// `branch_points`, and for one whose branches cross only where the exported path
/// already does. Deterministic: `BTreeMap` keys, rows sorted by branch slug
/// (ADR-0006).
pub fn branch_transport_overlay(plan: &Plan) -> Result<BranchTransportOverlay, BuildFailure> {
    let mut out: BranchTransportOverlay = BTreeMap::new();
    let branches = crate::compiler::branch::realize(plan.campaign);
    if branches.is_empty() {
        return Ok(out);
    }
    let flow = crate::compiler::flow::Flow::new(plan.campaign);
    for r in &branches {
        // An unreachable branch has no world to walk — `DW0482` has already
        // failed the build for it (same skip as `branch_paths`).
        let Some(w) = r.world else { continue };
        let cp = plan
            .branch_critical_path(&flow, &flow.playthrough_in(w))
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.failure.code,
                message: format!("branch `{}`: {}", r.branch.id, e.failure.message),
            })?;
        for (oid, pos) in &cp.transport {
            match plan.transport.get(oid) {
                // Already emitted unconditionally by the exported path.
                Some(d) if d == pos => continue,
                Some(d) => {
                    return Err(BuildFailure::Diagnostic {
                        code: DW_BRANCH_TRANSPORT_DIVERGES,
                        message: format!(
                            "objective `{oid}` crosses to {d:?} on the exported path but to \
                             {pos:?} on branch `{}`; completing it can only put the party in \
                             one place — split the crossing into one objective per branch, \
                             each gated by that branch's flags",
                            r.branch.id
                        ),
                    });
                }
                None => {}
            }
            out.entry(oid.clone()).or_default().push(BranchTransport {
                set: r.branch.set.clone(),
                unset: r.branch.unset.clone(),
                slug: r.branch.slug.clone(),
                pos: *pos,
            });
        }
    }
    for rows in out.values_mut() {
        rows.sort_by(|a, b| a.slug.cmp(&b.slug));
    }
    Ok(out)
}

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
