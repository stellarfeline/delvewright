//! Stair-orientation proof over the assembled world (`DW0430`).
//!
//! # The vanilla rule this enforces
//!
//! A bottom-half stair block is a full-height *quarter* sitting on one half of
//! the cell plus a half-slab across the whole cell. The `facing` property says
//! **which half carries the full-height part**. Verified against the vendored
//! 1.21.11 block shapes rather than folklore — the collision boxes of
//! `oak_stairs[half=bottom,shape=straight]` are:
//!
//! | `facing` | lower box | upper box |
//! |---|---|---|
//! | `north` | whole cell, 0→0.5 high | `z ∈ [0.0, 0.5]` — the **north** half |
//! | `south` | " | `z ∈ [0.5, 1.0]` — the **south** half |
//! | `west`  | " | `x ∈ [0.0, 0.5]` — the **west** half |
//! | `east`  | " | `x ∈ [0.5, 1.0]` — the **east** half |
//!
//! So the raised half is always on the `facing` side. For a staircase to be
//! climbable, each tread's raised half must be on the side of the *next tread
//! up* — otherwise the climber meets a full block face and must jump, and the
//! low half faces away from them where nobody can use it.
//!
//! **Therefore: `facing` = the direction you ascend.** Walking along a stair's
//! `facing` takes you up; walking against it takes you down.
//!
//! # What this proves, and what it deliberately does not
//!
//! Nav models a stair as a full cube ([`crate::assembled::collision_top_16`]
//! returns 16 for it), which is the conservative choice and is why no existing
//! proof can ever fire on a reversed stair: the climb is classified as a *jump*,
//! a jump of one block is legal, and the delve ships green with a staircase the
//! player has to hop up one tread at a time. That is exactly the defect this
//! module closes, and it closes it with a direction-only rule rather than by
//! teaching the occupancy model a two-height shape (which `Occupancy` cannot
//! express, and which would move nav in the over-proving direction its own
//! contract forbids).
//!
//! Scope is **proven routes only**, and within those, only stairs that actually
//! carry a climb:
//!
//! - For each consecutive pair of cells on a proven route whose elevation
//!   differs by one, the block under the **higher** cell is the tread being
//!   climbed onto. If it is a bottom-half stair, its `facing` must equal the
//!   direction from the lower cell to the higher one.
//! - Keying on the *higher* cell's floor is what makes turns safe. At a landing
//!   where a staircase turns, the lower cell's floor is the previous riser's
//!   tread and still legitimately points the old way; demanding it point the new
//!   way would be a false positive.
//! - From each riser found, the check widens **laterally** — perpendicular to
//!   the climb, while the neighbouring cells are also bottom-half stairs — so a
//!   staircase five lanes wide is proven across its full width even though the
//!   route only ever walks one lane.
//!
//! Stairs nowhere near a proven route are decoration and are not inspected: a
//! stepped gable, a corbel or a chair is a legitimate use of the block with no
//! climb semantics at all, and flagging those would be noise. A stair that is
//! the floor of a *flat* stretch of route is likewise fine (a half-step bump,
//! not a defect).

use std::collections::{BTreeMap, BTreeSet};

use crate::assembled::{base_id, state_value};
use crate::nav::NavError;
use crate::plan::Plan;
use crate::solver::Facing;
use delvewright_dsl::DwCode;

/// A stair whose `facing` contradicts the climb a proven route makes across it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReversedStair {
    /// The world cell of the stair block itself (the tread, one below the walk).
    pub cell: [i32; 3],
    /// The full blockstate found there.
    pub block: String,
    /// The `facing` the prefab authored.
    pub found: String,
    /// The `facing` the climb requires.
    pub expected: String,
    /// The prefab piece whose placement covers `cell`, when one does.
    pub piece: Option<String>,
}

const DW_STAIR_REVERSED: DwCode = DwCode::every_version("DW0430");

/// Whether `name` is a stair block laid the normal way up. Upside-down stairs
/// (`half=top`) present a flat full-height top face, so they carry no climb and
/// are out of scope.
fn is_walk_stair(name: &str) -> bool {
    base_id(name).ends_with("_stairs") && state_value(name, "half").unwrap_or("bottom") == "bottom"
}

/// The prefab piece covering `cell`, for the diagnostic message.
fn piece_at(plan: Option<&Plan>, cell: [i32; 3]) -> Option<String> {
    for area in &plan?.areas {
        for piece in &area.pieces {
            let (min, max) = piece.bbox();
            if (0..3).all(|i| cell[i] >= min[i] && cell[i] <= max[i]) {
                return Some(piece.prefab_id.clone());
            }
        }
    }
    None
}

/// Check one riser and every stair laterally beside it, recording defects.
fn check_riser(
    blocks: &BTreeMap<[i32; 3], String>,
    plan: Option<&Plan>,
    floor: [i32; 3],
    up: Facing,
    seen: &mut BTreeSet<[i32; 3]>,
    out: &mut Vec<ReversedStair>,
) {
    let want = up.token();
    let u = up.unit();
    // Does the cell `c` (a tread at the riser's height) itself carry a climb in
    // direction `up`? The riser signature is on the APPROACH side: you can stand
    // on this tread, and the cell you stepped from — one back along `up` — is a
    // walk surface exactly one block lower. Tested backwards, not forwards: a
    // shallow run gains height every 2-3 cells, so the floor ahead of a tread is
    // usually level with it and a forward test would discard most of the run.
    //
    // Checking this per lateral cell — rather than assuming the whole
    // perpendicular row belongs to the same run — is what stops a spiral
    // staircase's turn from widening into the flight at right angles to it and
    // reporting that flight's treads with this flight's expected facing.
    let solid = |c: [i32; 3]| blocks.get(&c).is_some_and(|n| !crate::assembled::is_air(n));
    let carries_climb = |c: [i32; 3]| -> bool {
        let above = [c[0], c[1] + 1, c[2]];
        let behind = [c[0] - u[0], c[1], c[2] - u[2]];
        let behind_floor = [c[0] - u[0], c[1] - 1, c[2] - u[2]];
        !solid(above) && !solid(behind) && solid(behind_floor)
    };
    // The riser itself (already confirmed by the route that walked it), then
    // outward along both perpendiculars while the neighbours are stairs that
    // independently carry the same climb — the width of the staircase.
    let mut lane = vec![floor];
    for side in up.perpendicular() {
        let d = side.unit();
        let mut c = floor;
        loop {
            c = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            match blocks.get(&c) {
                Some(n) if is_walk_stair(n) && carries_climb(c) => lane.push(c),
                _ => break,
            }
        }
    }
    for cell in lane {
        if !seen.insert(cell) {
            continue;
        }
        let Some(block) = blocks.get(&cell) else {
            continue;
        };
        if !is_walk_stair(block) {
            continue;
        }
        let found = state_value(block, "facing").unwrap_or("north");
        if found != want {
            out.push(ReversedStair {
                cell,
                block: block.clone(),
                found: found.to_string(),
                expected: want.to_string(),
                piece: piece_at(plan, cell),
            });
        }
    }
}

/// Every reversed stair under the given proven routes, in deterministic world
/// order. `routes` are integer cell polylines already proven walkable.
pub fn reversed_stairs(
    blocks: &BTreeMap<[i32; 3], String>,
    plan: Option<&Plan>,
    routes: &[Vec<[i32; 3]>],
) -> Vec<ReversedStair> {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut out: Vec<ReversedStair> = Vec::new();
    for cells in routes {
        for pair in cells.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if (b[1] - a[1]).abs() != 1 {
                continue;
            }
            let (lo, hi) = if a[1] < b[1] { (a, b) } else { (b, a) };
            // The climb direction, horizontally. A pure vertical hop (ladders,
            // resampled polylines) has no direction and carries no stair rule.
            let Some(up) = Facing::between(lo, hi) else {
                continue;
            };
            // The tread the climber lands on is the block under the higher cell.
            let floor = [hi[0], hi[1] - 1, hi[2]];
            match blocks.get(&floor) {
                Some(n) if is_walk_stair(n) => {}
                _ => continue,
            }
            check_riser(blocks, plan, floor, up, &mut seen, &mut out);
        }
    }
    out.sort_by_key(|r| r.cell);
    out
}

/// Build-tier proof: no stair on a proven route may face away from its climb.
pub fn check_stair_orientation(
    blocks: &BTreeMap<[i32; 3], String>,
    plan: Option<&Plan>,
    routes: &[Vec<[i32; 3]>],
) -> Result<(), NavError> {
    let bad = reversed_stairs(blocks, plan, routes);
    if bad.is_empty() {
        return Ok(());
    }
    // Per-piece rollup first — the fix list, since the defect is authored in the
    // prefab and one wrong literal in a generator produces a whole run of these.
    let mut by_piece: BTreeMap<(&str, &str, &str), (usize, [i32; 3])> = BTreeMap::new();
    for r in &bad {
        let key = (
            r.piece.as_deref().unwrap_or("<unplaced>"),
            r.found.as_str(),
            r.expected.as_str(),
        );
        let e = by_piece.entry(key).or_insert((0, r.cell));
        e.0 += 1;
    }
    let mut lines = Vec::new();
    for ((piece, found, expected), (n, first)) in &by_piece {
        lines.push(format!(
            "  {piece}: {n} stair(s) facing={found} on a run that climbs {expected} \
             (first at [{}, {}, {}])",
            first[0], first[1], first[2]
        ));
    }
    lines.push("  offending cells:".to_string());
    for r in bad.iter().take(24) {
        lines.push(format!(
            "    {} [{}, {}, {}] facing={} → {}",
            base_id(&r.block),
            r.cell[0],
            r.cell[1],
            r.cell[2],
            r.found,
            r.expected
        ));
    }
    if bad.len() > 24 {
        lines.push(format!("    … and {} more", bad.len() - 24));
    }
    Err(NavError {
        code: DW_STAIR_REVERSED,
        message: format!(
            "{} stair block(s) on a proven route face away from the climb they carry.\n{}\n\
             A vanilla stair's full-height half sits on its `facing` side, so `facing` is the \
             direction you ascend: a run climbing north must be `facing=north`. Reversed, the \
             climber meets a flat block face and has to jump every tread, and the half-step \
             points backwards into open air. Fix the PREFAB that authors these blocks — the \
             piece named above — so each tread's `facing` matches the direction its run gains \
             height, then re-export the `.nbt` and rebuild. Do NOT silence this by rerouting the \
             critical path around the staircase, and do NOT widen the nav step rule: the route \
             is correct, the geometry is not.",
            bad.len(),
            lines.join("\n")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STONE: &str = "minecraft:stone";

    /// A staircase climbing **north** (−z), `width` lanes wide, rising one block
    /// per cell. Lane `x`, tread `i` sits at `y = i`, `z = start_z - i`; the walk
    /// cell above it is `y = i + 1`. `facing` is what the treads are authored as.
    fn staircase(width: i32, steps: i32, facing: &str) -> BTreeMap<[i32; 3], String> {
        let mut b = BTreeMap::new();
        for x in 0..width {
            // The approach floor the climber starts from — one BELOW the first
            // tread, so stepping onto tread 0 is itself a rise.
            b.insert([x, -1, 4], STONE.to_string());
            for i in 0..steps {
                b.insert(
                    [x, i, 3 - i],
                    format!("minecraft:stone_stairs[facing={facing},half=bottom,shape=straight]"),
                );
            }
        }
        b
    }

    /// The route a climber walks up [`staircase`]: feet one above each tread.
    fn climb_route(steps: i32) -> Vec<Vec<[i32; 3]>> {
        let mut cells = vec![[0, 0, 4]];
        for i in 0..steps {
            cells.push([0, i + 1, 3 - i]);
        }
        vec![cells]
    }

    #[test]
    fn correct_staircase_is_silent() {
        let b = staircase(1, 3, "north");
        assert_eq!(reversed_stairs(&b, None, &climb_route(3)), vec![]);
        assert!(check_stair_orientation(&b, None, &climb_route(3)).is_ok());
    }

    #[test]
    fn reversed_staircase_is_an_error_naming_cell_and_expected_facing() {
        let b = staircase(1, 3, "south");
        let bad = reversed_stairs(&b, None, &climb_route(3));
        assert_eq!(bad.len(), 3, "every tread of the run is reported");
        assert!(
            bad.iter()
                .all(|r| r.found == "south" && r.expected == "north")
        );
        let err = check_stair_orientation(&b, None, &climb_route(3)).unwrap_err();
        assert_eq!(err.code, "DW0430");
        // The message must name a world cell so the content layer can find it.
        assert!(err.message.contains("[0, 0, 3]"), "{}", err.message);
        assert!(err.message.contains("climbs north"), "{}", err.message);
    }

    #[test]
    fn widening_covers_lanes_the_route_never_walks() {
        // The route walks lane x=0 only; a 3-wide run must still be proven whole.
        let b = staircase(3, 3, "south");
        let bad = reversed_stairs(&b, None, &climb_route(3));
        assert_eq!(bad.len(), 9, "3 treads x 3 lanes");
        assert!(bad.iter().any(|r| r.cell[0] == 2));
    }

    #[test]
    fn ornamental_stairs_off_route_are_not_touched() {
        let mut b = staircase(1, 3, "north");
        // A stepped gable far from anything walkable, deliberately "backwards".
        for i in 0..3 {
            b.insert(
                [40, 10 + i, 40],
                "minecraft:stone_stairs[facing=south,half=bottom,shape=straight]".to_string(),
            );
        }
        assert_eq!(reversed_stairs(&b, None, &climb_route(3)), vec![]);
    }

    #[test]
    fn upside_down_stairs_carry_no_climb() {
        let b = staircase(1, 3, "south")
            .into_iter()
            .map(|(k, v)| (k, v.replace("half=bottom", "half=top")))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(reversed_stairs(&b, None, &climb_route(3)), vec![]);
    }

    /// The false positive the "higher cell's floor" rule exists to avoid: where a
    /// staircase turns, the tread you arrived on still points the OLD way, and
    /// that is correct. Keying the rule on the lower cell's floor would flag it.
    #[test]
    fn a_turn_does_not_flag_the_flight_below_it() {
        let mut b = BTreeMap::new();
        b.insert([0, 0, 4], STONE.to_string());
        // North-climbing tread, then the run turns east and climbs again.
        b.insert(
            [0, 0, 3],
            "minecraft:stone_stairs[facing=north,half=bottom,shape=straight]".to_string(),
        );
        b.insert(
            [1, 1, 3],
            "minecraft:stone_stairs[facing=east,half=bottom,shape=straight]".to_string(),
        );
        let route = vec![vec![[0, 1, 4], [0, 1, 3], [1, 2, 3]]];
        assert_eq!(reversed_stairs(&b, None, &route), vec![]);
    }

    #[test]
    fn flat_walk_over_a_stair_is_not_a_defect() {
        let mut b = BTreeMap::new();
        for z in 0..4 {
            b.insert([0, 0, z], STONE.to_string());
        }
        b.insert(
            [0, 0, 2],
            "minecraft:stone_stairs[facing=west,half=bottom,shape=straight]".to_string(),
        );
        let route = vec![vec![[0, 1, 0], [0, 1, 1], [0, 1, 2], [0, 1, 3]]];
        assert_eq!(reversed_stairs(&b, None, &route), vec![]);
    }

    #[test]
    fn descending_the_same_run_agrees_with_ascending_it() {
        let b = staircase(1, 3, "south");
        let mut down = climb_route(3);
        down[0].reverse();
        let up = reversed_stairs(&b, None, &climb_route(3));
        let dn = reversed_stairs(&b, None, &down);
        assert_eq!(up, dn, "the rule is symmetric in travel direction");
    }
}
