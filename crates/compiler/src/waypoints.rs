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
use crate::plan::{Plan, TimedGatePlan};

/// The player's standing occupancy in blocks: the feet cell and the cell above it.
/// A `timed-gate` fill that intersects either one intersects the player's body.
const PLAYER_OCCUPANCY: i32 = 2;

/// Build the waypoints validation artifact for the campaign's proven walked legs.
/// Keys/coordinates mirror `critical-path.json` so the harness can match a leg by
/// its destination anchor.
pub fn waypoints_json(plan: &Plan, routes: &[LegRoute]) -> Value {
    let gates: Vec<([i32; 3], [i32; 3])> = plan
        .timed_gates
        .iter()
        .map(|g| normalized_region(g.gate_region))
        .collect();
    let legs: Vec<Value> = routes
        .iter()
        .map(|leg| {
            // Force-keep the gate mouth cells alongside the use-gate cells, so the
            // hop that actually crosses a clocked span is SHORT (see
            // [`gate_mouth_cells`]).
            let mut force_keep = leg.use_gates.clone();
            for region in &gates {
                force_keep.extend(gate_mouth_cells(&leg.cells, *region));
            }
            let wps: Vec<Value> = thin(&leg.cells, &force_keep)
                .into_iter()
                .map(|c| json!(c))
                .collect();
            let mut leg_json = json!({
                "from": leg.from,
                "to": leg.to,
                "waypoints": wps,
            });
            // Use-gate edges (task #59): the closed fence-gate cells this leg walks
            // through with an adventure-legal right-click. Emitted only when present,
            // so gate-free campaigns stay byte-identical. The harness pathfinder's
            // `canOpenDoors` performs the click (harness PR #110); this names the
            // cells first-class instead of leaving them workaround folklore.
            if !leg.use_gates.is_empty() {
                leg_json["use_gates"] = json!(leg.use_gates);
            }
            // spec-0016 §4 timed gates (task #81): the gates whose clock can
            // physically interrupt THIS leg, in declared order. See
            // [`leg_crosses_gate`] for the crossing definition.
            let crossed: Vec<&str> = plan
                .timed_gates
                .iter()
                .zip(&gates)
                .filter(|(_, region)| leg_crosses_gate(&leg.cells, **region))
                .map(|(g, _)| g.id.as_str())
                .collect();
            if !crossed.is_empty() {
                leg_json["timed_gates"] = json!(crossed);
            }
            leg_json
        })
        .collect();
    let mut root = json!({
        // Campaign-derived, matching `critical-path.json` (a v0.2 campaign emits a
        // v0.2 artifact, etc.).
        "version": plan.campaign.world.dsl_version,
        "campaign_id": plan.namespace,
        "legs": legs,
    });
    // The gate table the per-leg `timed_gates` ids index into: everything the runtime
    // harness needs to WAIT for a window instead of failing the leg — where the fill
    // lands and how long each half of the clock runs. Omitted entirely when the
    // campaign declares no timed gate, so such campaigns stay byte-identical.
    if !plan.timed_gates.is_empty() {
        root["timed_gates"] = json!(
            plan.timed_gates
                .iter()
                .zip(&gates)
                .map(|(g, region)| timed_gate_json(g, *region))
                .collect::<Vec<Value>>()
        );
    }
    root
}

/// One exported `timed-gate` (spec-0016 §4): the region its clock fills/clears in
/// absolute world coordinates, plus the clock itself. `phase` is the ticks after
/// world init before the first open, so a harness can reason about the schedule
/// without re-deriving it from the emitted functions. `crush` (spec-0016 §4
/// addendum) is whether the closing edge kills players caught inside the region —
/// the runtime bot must stage such a crossing at the gate mouth and enter only on
/// an observed fresh window, never blind (task #140); withholding the fact would
/// force the harness to guess at a lethal mechanic the compiler emitted.
fn timed_gate_json(g: &TimedGatePlan, (min, max): ([i32; 3], [i32; 3])) -> Value {
    json!({
        "id": g.id,
        "region": { "min": min, "max": max },
        "block": g.gate_block,
        "open_ticks": g.open_ticks,
        "closed_ticks": g.closed_ticks,
        "phase": g.phase,
        "crush": g.crush,
    })
}

/// Normalize a gate region's inclusive corners to `(min, max)` componentwise, so the
/// exported contract is a canonical bounding box regardless of which corner the
/// anchor declared first.
fn normalized_region((a, b): ([i32; 3], [i32; 3])) -> ([i32; 3], [i32; 3]) {
    let mut min = [0; 3];
    let mut max = [0; 3];
    for i in 0..3 {
        min[i] = a[i].min(b[i]);
        max[i] = a[i].max(b[i]);
    }
    (min, max)
}

/// Whether a leg's proven A* route **crosses** a timed gate.
///
/// The definition is exact, not proximity-based: the leg crosses the gate iff, at
/// some cell of the proven route, closing the gate would intersect the PLAYER'S OWN
/// body — the feet cell or the cell above it ([`PLAYER_OCCUPANCY`]) lies inside the
/// gate region. That is precisely the physical event the harness must survive (the
/// fill landing on top of the walk), and it is stated over the full A* cell route
/// rather than the thinned waypoint polyline, so a straight run that thins to its
/// endpoints still reports the gate it passes through.
///
/// A leg that merely walks *past* a gate is deliberately NOT marked. The mark is
/// what licenses the harness to retry a failed leg, and a blanket retry on legs the
/// gate cannot block would mask real navigation regressions — the mark must mean
/// "this gate can stop this walk", nothing looser.
fn leg_crosses_gate(cells: &[[i32; 3]], region: ([i32; 3], [i32; 3])) -> bool {
    cells.iter().any(|c| occupies_gate(*c, region))
}

/// Whether standing at feet cell `c` puts the player's body inside `region`.
fn occupies_gate(c: [i32; 3], (min, max): ([i32; 3], [i32; 3])) -> bool {
    c[0] >= min[0]
        && c[0] <= max[0]
        && c[2] >= min[2]
        && c[2] <= max[2]
        && c[1] <= max[1]
        && c[1] + (PLAYER_OCCUPANCY - 1) >= min[1]
}

/// The route cells at a timed gate's **mouth**: for each maximal run of cells whose
/// occupancy is inside the region, the route cell immediately BEFORE the run and the
/// one immediately AFTER it — the two cells flanking the crossing, on either side of
/// the gate. Cells *inside* the region are deliberately NOT returned.
///
/// These are force-kept as waypoints for the same reason a use-gate cell is (task
/// #59) — an interaction point must never be thinned away — but the payoff here is
/// timing, not interaction. Corner-thinning collapses a straight corridor through a
/// gate to its two endpoints, which asks the runtime bot to walk the WHOLE straight
/// run inside one open window; on the-drowned-bell that is an 18-block hop through a
/// 5-second window, and it loses the race. Pinning the mouth splits it into a long
/// approach hop that no clock can interrupt plus a short crossing that any readable
/// window admits — matching what `DW0378` actually proves, which is that the window
/// admits crossing the SPAN, not an arbitrary run-up to it.
///
/// **Why the in-region cells must not be pinned** (task #204): the harness treats
/// every waypoint as an *arrive-at* goal, so a waypoint inside the region parks the
/// bot under the gate — and a `crush: true` gate then fills that cell with the bot in
/// it. The waypoint contract is "stand here", and standing inside a timed gate is
/// never a thing to ask for. The flanking pair says the same thing about the route
/// (approach, then cross) without ever naming a lethal cell as a destination; the
/// crossing span itself is what `DW0378` charges, not a rest stop.
///
/// Purely additive and deterministic: every kept cell is a proven route cell, and
/// consecutive kept cells stay a straight constant-delta run, so the polyline can
/// never leave the proven path.
fn gate_mouth_cells(cells: &[[i32; 3]], region: ([i32; 3], [i32; 3])) -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < cells.len() {
        if !occupies_gate(cells[i], region) {
            i += 1;
            continue;
        }
        // `i..end` is a maximal run of cells inside the region.
        let mut end = i;
        while end + 1 < cells.len() && occupies_gate(cells[end + 1], region) {
            end += 1;
        }
        if i > 0 {
            out.push(cells[i - 1]);
        }
        if let Some(after) = cells.get(end + 1) {
            out.push(*after);
        }
        i = end + 1;
    }
    out
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
///
/// `force_keep` cells (a leg's use-gate cells, task #59) are always kept even
/// mid-straight-run: a gate the player must right-click open is an interaction
/// point the harness needs as an explicit waypoint, never thinned away. Purely
/// additive — every force-kept cell is a proven route cell.
pub(crate) fn thin(cells: &[[i32; 3]], force_keep: &[[i32; 3]]) -> Vec<[i32; 3]> {
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
        if force_keep.contains(&cells[i]) {
            keep[i] = true; // a use-gate cell is always an explicit waypoint
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
        assert_eq!(thin(&[[0, 65, 0]], &[]), vec![[0, 65, 0]]);
        assert_eq!(
            thin(&[[0, 65, 0], [1, 65, 0]], &[]),
            vec![[0, 65, 0], [1, 65, 0]]
        );
    }

    #[test]
    fn a_straight_run_collapses_to_its_endpoints() {
        // A 24-block straight flat run has no turns → just the two endpoints (the
        // bot walks the straight corridor between them directly).
        let cells: Vec<[i32; 3]> = (0..=24).map(|x| [x, 65, 0]).collect();
        assert_eq!(thin(&cells, &[]), vec![[0, 65, 0], [24, 65, 0]]);
        // A constant-delta staircase is also one straight run → endpoints only.
        let stair: Vec<[i32; 3]> = (0..6).map(|i| [i, 65 + i, 0]).collect();
        assert_eq!(thin(&stair, &[]), vec![[0, 65, 0], [5, 70, 0]]);
    }

    #[test]
    fn corners_are_kept_so_hops_never_cut_across_the_bend() {
        // An L: east along z=0, then a turn to north at [5,65,0]. The corner is kept
        // (otherwise the straight line from start to end cuts across the bend), plus
        // the corridor commit cell one step into the turn ([5,65,1]) — task #45.
        let mut cells: Vec<[i32; 3]> = (0..=5).map(|x| [x, 65, 0]).collect();
        cells.extend((1..=4).map(|z| [5, 65, z]));
        let wps = thin(&cells, &[]);
        assert_eq!(wps, vec![[0, 65, 0], [5, 65, 0], [5, 65, 1], [5, 65, 4]]);
        // A floor-height step is a direction change too, so it is kept.
        let ramp = [[0, 65, 0], [1, 65, 0], [2, 66, 0], [3, 66, 0]];
        assert_eq!(
            thin(&ramp, &[]),
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
        let wps = thin(&cells, &[]);
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
    fn a_use_gate_cell_is_never_thinned_away() {
        // A straight run through a closed fence gate at [3,65,0] (task #59): plain
        // corner-thinning would collapse it to the endpoints; the force-keep set
        // pins the gate cell so the harness gets the interaction point explicitly.
        let cells: Vec<[i32; 3]> = (0..=8).map(|x| [x, 65, 0]).collect();
        assert_eq!(thin(&cells, &[]), vec![[0, 65, 0], [8, 65, 0]]);
        assert_eq!(
            thin(&cells, &[[3, 65, 0]]),
            vec![[0, 65, 0], [3, 65, 0], [8, 65, 0]]
        );
    }

    #[test]
    fn a_region_is_normalized_to_a_canonical_min_max_box() {
        assert_eq!(
            normalized_region(([4, 66, -2], [1, 63, 5])),
            ([1, 63, -2], [4, 66, 5])
        );
        // Already canonical → unchanged.
        assert_eq!(
            normalized_region(([1, 63, -2], [4, 66, 5])),
            ([1, 63, -2], [4, 66, 5])
        );
    }

    #[test]
    fn a_leg_crosses_a_gate_its_route_walks_through() {
        // A straight west-to-east run at feet y=63 through a 1-wide portcullis
        // column filling y=63..64 at x=5.
        let region = ([5, 63, 0], [5, 64, 0]);
        let cells: Vec<[i32; 3]> = (0..=10).map(|x| [x, 63, 0]).collect();
        assert!(leg_crosses_gate(&cells, region));
        // The same run one block to the side never enters the column.
        let beside: Vec<[i32; 3]> = (0..=10).map(|x| [x, 63, 1]).collect();
        assert!(
            !leg_crosses_gate(&beside, region),
            "walking PAST a gate is not crossing it — the mark must mean the gate \
             can stop this walk"
        );
    }

    #[test]
    fn the_gate_mouth_is_pinned_so_the_crossing_hop_is_short() {
        // The-drowned-bell shape: a long straight corridor through a portcullis. Plain
        // corner-thinning collapses it to its endpoints, asking the bot to walk the
        // whole run inside one 5-second window. Pinning the mouth splits it.
        let region = ([22, 63, -10], [26, 65, -10]);
        let cells: Vec<[i32; 3]> = (-14..=4).rev().map(|z| [24, 63, z]).collect();
        assert_eq!(cells.first(), Some(&[24, 63, 4]));
        assert_eq!(thin(&cells, &[]), vec![[24, 63, 4], [24, 63, -14]]);
        let keep = gate_mouth_cells(&cells, region);
        // The two cells FLANKING the gate — never the cell under it (task #204).
        assert_eq!(keep, vec![[24, 63, -9], [24, 63, -11]]);
        assert_eq!(
            thin(&cells, &keep),
            vec![[24, 63, 4], [24, 63, -9], [24, 63, -11], [24, 63, -14]],
            "a long approach, then a two-block crossing any readable window admits"
        );
    }

    #[test]
    fn no_waypoint_ever_stands_inside_a_timed_gate_region() {
        // The harness treats every waypoint as an ARRIVE-AT goal, so a waypoint
        // inside the region parks the bot under the portcullis — and a `crush: true`
        // gate then fills that cell with the bot in it (the-drowned-bell round-2
        // death, task #204). The mouth pins must flank the crossing, never name it.
        //
        // A 3-deep region, so the crossing run is several cells long: the whole run
        // must be absent from the pins.
        let region = ([22, 63, -11], [26, 65, -9]);
        let cells: Vec<[i32; 3]> = (-14..=4).rev().map(|z| [24, 63, z]).collect();
        let keep = gate_mouth_cells(&cells, region);
        assert_eq!(keep, vec![[24, 63, -8], [24, 63, -12]]);
        for c in thin(&cells, &keep) {
            assert!(
                !occupies_gate(c, region),
                "waypoint {c:?} stands inside the gate region"
            );
        }
    }

    #[test]
    fn a_route_that_re_enters_a_gate_pins_both_crossings() {
        // Two separate crossings of the same region (a there-and-back leg): each
        // maximal in-region run contributes its own flanking pair, and no more.
        let region = ([5, 63, 0], [5, 64, 0]);
        let mut cells: Vec<[i32; 3]> = (0..=8).map(|x| [x, 63, 0]).collect();
        cells.extend((0..=7).rev().map(|x| [x, 63, 0]));
        assert_eq!(
            gate_mouth_cells(&cells, region),
            vec![[4, 63, 0], [6, 63, 0], [6, 63, 0], [4, 63, 0]]
        );
    }

    #[test]
    fn a_gate_at_the_very_start_of_a_leg_pins_only_the_far_mouth() {
        // No cell precedes the run, so there is only one flank to pin — and the
        // in-region start cell still never becomes a waypoint of its own accord.
        let region = ([0, 63, 0], [0, 64, 0]);
        let cells: Vec<[i32; 3]> = (0..=6).map(|x| [x, 63, 0]).collect();
        assert_eq!(gate_mouth_cells(&cells, region), vec![[1, 63, 0]]);
    }

    #[test]
    fn a_gate_the_leg_never_enters_pins_nothing() {
        let region = ([5, 63, 0], [5, 64, 0]);
        let beside: Vec<[i32; 3]> = (0..=10).map(|x| [x, 63, 1]).collect();
        assert!(gate_mouth_cells(&beside, region).is_empty());
        assert_eq!(thin(&beside, &[]), vec![[0, 63, 1], [10, 63, 1]]);
    }

    #[test]
    fn a_gate_that_fills_only_head_height_still_crosses() {
        // A portcullis whose fill covers y=64..65 only: the walker's FEET at y=63 are
        // never inside the region, but the block at head height is — the fill lands on
        // the player. Feet-only containment would miss it.
        let region = ([5, 64, 0], [5, 65, 0]);
        let cells: Vec<[i32; 3]> = (0..=10).map(|x| [x, 63, 0]).collect();
        assert!(leg_crosses_gate(&cells, region));
        // Two blocks of clearance under the fill → the player walks under it untouched.
        let low: Vec<[i32; 3]> = (0..=10).map(|x| [x, 62, 0]).collect();
        assert!(!leg_crosses_gate(&low, region));
    }

    #[test]
    fn a_gate_the_route_never_reaches_is_not_crossed() {
        let region = ([5, 63, 0], [5, 64, 0]);
        let cells: Vec<[i32; 3]> = (0..=3).map(|x| [x, 63, 0]).collect();
        assert!(!leg_crosses_gate(&cells, region));
    }

    #[test]
    fn thinning_is_deterministic() {
        let mut cells: Vec<[i32; 3]> = (0..30).map(|x| [x, 65, 0]).collect();
        cells.extend((1..10).map(|z| [29, 65, z]));
        assert_eq!(thin(&cells, &[]), thin(&cells, &[]));
    }
}
