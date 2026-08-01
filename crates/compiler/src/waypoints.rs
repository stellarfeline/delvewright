//! Validation metadata: the compiler-proven critical-path waypoint polyline
//! (`<out>/validation/critical-path-waypoints.json`, task #38).
//!
//! DW0311 ([`crate::nav::check_critical_path`]) already proves an A* route connects
//! every walked critical-path leg over the assembled geometry. This module exports
//! those proven routes, thinned to sparse waypoints, so the runtime harness can
//! drive the bot leg-by-leg — feeding successive nearby pathfinder goals instead of
//! one distant goal that explodes mineflayer's A* search budget on a large open
//! winding cave (the "No path to the goal!" strand on geometry that is provably
//! connected).
//!
//! This is VALIDATION metadata, not shipped gameplay content: it lives under
//! `validation/` (excluded from the delve image by `Dockerfile.delve`, like
//! `packtest-datapack/`) and the datapack never references it.
//!
//! Determinism (ADR-0006): the thinning is a pure function of the (deterministic)
//! A* cell route, so same DSL + seed → byte-identical waypoints.

use serde_json::{Value, json};

use crate::nav::LegRoute;
use crate::plan::Plan;

/// Build the waypoints validation artifact for the campaign's proven walked legs.
/// Keys/coordinates mirror `critical-path.json` so the harness can match a leg by
/// its destination anchor.
pub fn waypoints_json(plan: &Plan, routes: &[LegRoute]) -> Value {
    let legs: Vec<Value> = routes
        .iter()
        .map(|leg| {
            let wps: Vec<Value> = thin(&leg.cells).into_iter().map(|c| json!(c)).collect();
            json!({
                "from": leg.from,
                "to": leg.to,
                "waypoints": wps,
            })
        })
        .collect();
    json!({
        // Campaign-derived, matching `critical-path.json` (a v0.2 campaign emits a
        // v0.2 artifact, etc.).
        "version": plan.campaign.world.dsl_version,
        "campaign_id": plan.namespace,
        "legs": legs,
    })
}

/// Thin an A* cell polyline to its **corners**: keep the two endpoints and every
/// cell where the direction of travel changes (a turn, or a floor-height step). The
/// cells are adjacent cardinal steps, so between two kept corners the path is a
/// single straight constant-delta run — a straight corridor or a straight staircase
/// the runtime pathfinder can always walk directly. Dropping only the interior of a
/// straight run keeps the polyline sparse WITHOUT ever leaving the proven route:
/// the straight line the bot walks between consecutive waypoints IS the proven path
/// there (distance-based thinning could cut a corner and send the bot into a wall).
/// Deterministic and order-preserving.
///
/// Shared with the visual-tier POV shot planner (`crate::render_plan`): the same
/// corner-thinned waypoint list the harness replays is where each first-person
/// camera stands, so a POV shot is taken at every turn/endpoint (not every cell).
pub(crate) fn thin(cells: &[[i32; 3]]) -> Vec<[i32; 3]> {
    if cells.len() <= 2 {
        return cells.to_vec();
    }
    // Keep the endpoints, every corner (direction change / floor-height step), and —
    // for the dead-end-pocket fix (task #45) — the **cell immediately after each
    // corner**: the first cell of the outgoing segment. A corner where a wide room
    // narrows into a corridor is range-1-satisfiable from an off-route "pocket" cell
    // beside it (e.g. wp9 `[261,65,-3]` from `[260,65,-3]`, walled to the north), and
    // the runtime bot can stall there. Keeping the post-corner cell gives the harness
    // a proven **corridor commit cell** one step into the new direction: a close,
    // corridor-axis target for its stall-recovery to re-centre and unstick toward,
    // instead of a distant diagonal to the next corner. Purely additive — every kept
    // cell is a proven route cell and consecutive kept cells stay a straight
    // constant-delta run — so it can never send the bot off the proven path.
    let mut keep = vec![false; cells.len()];
    keep[0] = true;
    keep[cells.len() - 1] = true;
    for i in 1..cells.len() - 1 {
        if delta(cells[i - 1], cells[i]) != delta(cells[i], cells[i + 1]) {
            keep[i] = true; // the corner
            keep[i + 1] = true; // the corridor commit cell just past the corner
        }
    }
    (0..cells.len())
        .filter(|&i| keep[i])
        .map(|i| cells[i])
        .collect()
}

/// The step vector from `a` to `b`.
fn delta(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_route_is_kept_verbatim() {
        assert_eq!(thin(&[[0, 65, 0]]), vec![[0, 65, 0]]);
        assert_eq!(
            thin(&[[0, 65, 0], [1, 65, 0]]),
            vec![[0, 65, 0], [1, 65, 0]]
        );
    }

    #[test]
    fn a_straight_run_collapses_to_its_endpoints() {
        // A 24-block straight flat run has no turns → just the two endpoints (the
        // bot walks the straight corridor between them directly).
        let cells: Vec<[i32; 3]> = (0..=24).map(|x| [x, 65, 0]).collect();
        assert_eq!(thin(&cells), vec![[0, 65, 0], [24, 65, 0]]);
        // A constant-delta staircase is also one straight run → endpoints only.
        let stair: Vec<[i32; 3]> = (0..6).map(|i| [i, 65 + i, 0]).collect();
        assert_eq!(thin(&stair), vec![[0, 65, 0], [5, 70, 0]]);
    }

    #[test]
    fn corners_are_kept_so_hops_never_cut_across_the_bend() {
        // An L: east along z=0, then a turn to north at [5,65,0]. The corner is kept
        // (otherwise the straight line from start to end cuts across the bend), plus
        // the corridor commit cell one step into the turn ([5,65,1]) — task #45.
        let mut cells: Vec<[i32; 3]> = (0..=5).map(|x| [x, 65, 0]).collect();
        cells.extend((1..=4).map(|z| [5, 65, z]));
        let wps = thin(&cells);
        assert_eq!(wps, vec![[0, 65, 0], [5, 65, 0], [5, 65, 1], [5, 65, 4]]);
        // A floor-height step is a direction change too, so it is kept.
        let ramp = [[0, 65, 0], [1, 65, 0], [2, 66, 0], [3, 66, 0]];
        assert_eq!(
            thin(&ramp),
            vec![[0, 65, 0], [1, 65, 0], [2, 66, 0], [3, 66, 0]]
        );
    }

    #[test]
    fn dead_end_pocket_corner_keeps_a_corridor_commit_cell() {
        // The nobodys-cave wp9 pocket geometry (task #45): the route runs east along
        // z=-3 through a WIDE room (so the cell just south-west of the corner is
        // standable and range-1-satisfiable), then turns north into a corridor at the
        // corner [261,65,-3]. The runtime bot can satisfy the corner's range-1 goal at
        // the off-route pocket [260,65,-3] (walled north) and stall. Thinning must keep
        // the corridor commit cell [261,65,-2] — one step into the turn — so the
        // harness recovery has a close corridor-axis target to unstick toward.
        let mut cells: Vec<[i32; 3]> = (247..=261).map(|x| [x, 65, -3]).collect(); // east run
        cells.extend((-2..=0).map(|z| [261, 65, z])); // turn north into the corridor
        let wps = thin(&cells);
        // The corner and the commit cell one step into the +z corridor are both kept.
        let corner_idx = wps
            .iter()
            .position(|&c| c == [261, 65, -3])
            .expect("corner kept");
        assert_eq!(
            wps[corner_idx + 1],
            [261, 65, -2],
            "the corridor commit cell just past the corner must follow it: {wps:?}"
        );
        // The pocket cell itself is never a waypoint (it is off-route).
        assert!(
            !wps.contains(&[260, 65, -3]),
            "off-route pocket is not a waypoint"
        );
        // Endpoints preserved.
        assert_eq!(wps.first(), Some(&[247, 65, -3]));
        assert_eq!(wps.last(), Some(&[261, 65, 0]));
    }

    #[test]
    fn thinning_is_deterministic() {
        let mut cells: Vec<[i32; 3]> = (0..30).map(|x| [x, 65, 0]).collect();
        cells.extend((1..10).map(|z| [29, 65, z]));
        assert_eq!(thin(&cells), thin(&cells));
    }
}
