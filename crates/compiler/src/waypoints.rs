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

/// Target spacing (blocks of accumulated path length) between exported waypoints on
/// a straight, flat run. A waypoint is ALSO forced at every floor-height (`y`)
/// change and at each leg's two endpoints, so stairs/steps stay densely sampled
/// (each hop trivially short for the runtime pathfinder) regardless of this value.
const WAYPOINT_SPACING: f64 = 10.0;

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

/// Thin an A* cell polyline to sparse waypoints: always keep the first and last
/// cell; keep a cell whenever the accumulated path length since the last kept
/// waypoint reaches [`WAYPOINT_SPACING`]; and keep a cell at every floor-height
/// (`y`) change (so a staircase is sampled step-by-step, each hop trivially short).
/// Deterministic and order-preserving.
fn thin(cells: &[[i32; 3]]) -> Vec<[i32; 3]> {
    if cells.len() <= 2 {
        return cells.to_vec();
    }
    let mut out = vec![cells[0]];
    let mut acc = 0.0f64;
    for i in 1..cells.len() - 1 {
        acc += dist(cells[i - 1], cells[i]);
        let y_change = cells[i][1] != out.last().unwrap()[1];
        if y_change || acc >= WAYPOINT_SPACING {
            out.push(cells[i]);
            acc = 0.0;
        }
    }
    out.push(cells[cells.len() - 1]);
    out
}

/// Euclidean distance between two integer cells.
fn dist(a: [i32; 3], b: [i32; 3]) -> f64 {
    let dx = (a[0] - b[0]) as f64;
    let dy = (a[1] - b[1]) as f64;
    let dz = (a[2] - b[2]) as f64;
    (dx * dx + dy * dy + dz * dz).sqrt()
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
    fn straight_flat_run_thins_to_roughly_every_spacing_blocks() {
        // A 24-block straight flat run: endpoints plus ~every 10 blocks.
        let cells: Vec<[i32; 3]> = (0..=24).map(|x| [x, 65, 0]).collect();
        let wps = thin(&cells);
        assert_eq!(wps.first(), Some(&[0, 65, 0]));
        assert_eq!(wps.last(), Some(&[24, 65, 0]));
        // Endpoints + interior at x≈10 and x≈20 → 4 waypoints, far fewer than 25.
        assert!(
            wps.len() >= 3 && wps.len() <= 6,
            "expected a sparse polyline, got {} waypoints: {wps:?}",
            wps.len()
        );
        // No gap between consecutive kept waypoints exceeds the spacing + one step.
        for w in wps.windows(2) {
            assert!(
                dist(w[0], w[1]) <= WAYPOINT_SPACING + 1.0,
                "gap too large between {:?} and {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn every_floor_height_change_is_a_waypoint() {
        // A staircase: each cell steps up in y. Every step must be kept so the
        // runtime pathfinder solves one short stair hop at a time.
        let cells: Vec<[i32; 3]> = (0..6).map(|i| [i, 65 + i, 0]).collect();
        let wps = thin(&cells);
        assert_eq!(wps, cells, "every stair step is a waypoint");
    }

    #[test]
    fn thinning_is_deterministic() {
        let cells: Vec<[i32; 3]> = (0..30).map(|x| [x, 65, 0]).collect();
        assert_eq!(thin(&cells), thin(&cells));
    }
}
