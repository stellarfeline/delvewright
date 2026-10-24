//! The trap proofs: a lethal trap on the forced path can be discharged
//! (`DW0342`), a volley blankets its zone, and a collapse does not bury the
//! path (`DW0442`, `DW0444`, `DW0445`, `DW0446`).

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, TrapPlan};
use delvewright_dsl::{Lethality, TrapReset};
use std::collections::{BTreeMap, BTreeSet};

/// Prove every **lethal** trap on the forced critical path is discharged (DSL
/// v0.6, spec-0011) — [`DW_TRAP_LETHAL_UNAVOIDABLE`] (`DW0342`). Death is
/// recoverable but costly (`keep_inventory true`, respawn at the entrance or last
/// checkpoint), so an unavoidable lethal trap deep in the delve can soft-loop the
/// party. For every trap whose lethality is `lethal` and whose trigger cell is a
/// required critical-path cell, exactly one discharge must hold:
///
/// - **Avoidable** — the trigger cell is not a forced-path cell (the exported
///   waypoints already steer clear). No obligation; the preferred outcome.
/// - **Survivable** — the trap is `reset: once`: it fires, is spent, and the
///   respawn walk-back never re-triggers it, so there is no soft-loop.
/// - **Disarmable** — a disarm affordance is reachable from the spawn **without
///   crossing the trap cell**, so the party can turn the trap off before being
///   forced onto it.
///
/// A forced lethal `rearm` trap with no reachable disarm provably soft-loops the
/// party → `DW0342`. Non-critical-path (branch/optional) lethal traps carry no
/// obligation here (existing `DW0306` gate-reachability covers not sealing off a
/// mandatory anchor).
pub fn check_traps(plan: &Plan, world: &World, moves: &[MovePlan]) -> Result<(), Failure> {
    if plan.traps.is_empty() {
        return Ok(());
    }
    let required = world.required_path_cells(plan, moves);
    // Every area's entry point, through the one resolver (`Plan::entry_points`).
    // This used to sweep the anchor map for a literal name, which counted no
    // island-tileset area at all — an honest question about the wrong key.
    let spawn_starts: Vec<[i32; 3]> = plan.entry_points().collect();
    let legs = world.walked_legs_sealed(plan);
    verify_traps(world, &plan.traps, &required, &spawn_starts, &legs)
}

/// The pure core of [`check_traps`] (unit-testable against a synthetic [`World`]).
/// `required` is the forced critical-path cell set; `spawn_starts` are the spawn
/// cells the disarm-reachability search roots at; `legs` are the walked legs with
/// their gate seals, used to pick the gate state a disarm must be reachable under.
fn verify_traps(
    world: &World,
    traps: &[TrapPlan],
    required: &BTreeSet<[i32; 3]>,
    spawn_starts: &[[i32; 3]],
    legs: &[(LegRoute, BTreeSet<[i32; 3]>)],
) -> Result<(), Failure> {
    for t in traps {
        if !matches!(t.lethality, Lethality::Lethal) {
            continue; // only lethal traps carry the obligation
        }
        let tc = t.trigger_cell;
        // (a) Avoidable: the trigger cell is never a forced critical-path cell.
        // `required` is computed over the causally-sealed per-leg world,
        // so a detour the player only has to walk BECAUSE a `close-gate`
        // shut the direct route counts as forced, exactly as it is in play.
        if !required.contains(&tc) {
            continue;
        }
        // (b) Survivable: a single-shot trap fires once and is spent; the respawn
        // walk-back (keep_inventory) never re-triggers it → no soft-loop.
        if matches!(t.reset, TrapReset::Once) {
            continue;
        }
        // (c) Disarmable: a disarm affordance reachable before the trap is forced —
        // under the gate state in force on the earliest leg that crosses the trap
        // cell. Searching the fully-open world would "prove" a disarm the party
        // can no longer reach once a `close-gate` has fired.
        let seal = legs
            .iter()
            .find(|(leg, _)| leg.cells.contains(&tc))
            .map(|(_, s)| s)
            .filter(|s| !s.is_empty());
        let sealed_world;
        let disarm_world: &World = match seal {
            Some(s) => {
                sealed_world = world.with_sealed(s);
                &sealed_world
            }
            None => world,
        };
        if let Some(dis) = &t.disarm
            && disarm_reachable_before(disarm_world, spawn_starts, dis.via_cell, tc)
        {
            continue;
        }
        return Err(Failure {
            code: DW_TRAP_LETHAL_UNAVOIDABLE,
            message: format!(
                "lethal trap `{}` sits on the forced critical path at {tc:?} with no discharge — \
                 it is not avoidable (its trigger cell is a required path cell), not survivable (it \
                 `rearm`s, so a respawn walk-back re-triggers it → soft-loop), and not disarmable \
                 (no disarm affordance is reachable before it). Move the trap off the critical \
                 path, set `reset: once`, or add a `disarm` whose `via` anchor is reachable before \
                 the trap cell — do NOT weaken this check to get green.",
                t.id
            ),
        });
    }
    Ok(())
}

/// Whether the disarm affordance at `via` is reachable from any spawn start over
/// the walkable world **without ever stepping on the trap cell** — i.e. the party
/// can reach and use the disarm before being forced onto the trap. A BFS over
/// standable cells with `trap_cell` removed from the walkable set.
///
/// The walk relation, not the reach graph (spec-0113 §4.5): the critical path
/// performs the disarm (`trap-disarm`), so the question is whether the party
/// walks on cue to the affordance before the trap is forced.
fn disarm_reachable_before(
    world: &World,
    starts: &[[i32; 3]],
    via: [i32; 3],
    trap_cell: [i32; 3],
) -> bool {
    let Some(goal) = world.snap_standable(via, SNAP_RADIUS) else {
        return false;
    };
    if goal == trap_cell {
        return false;
    }
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
    for s in starts {
        if let Some(start) = world.snap_standable(*s, SNAP_RADIUS)
            && start != trap_cell
            && seen.insert(start)
        {
            queue.push_back(start);
        }
    }
    while let Some(cur) = queue.pop_front() {
        if cur == goal {
            return true;
        }
        for n in world.neighbors(cur) {
            if n != trap_cell && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen.contains(&goal)
}

// ---------------------------------------------------------------------------
// spec-0022 — command-driven trap payloads: volley coverage + collapse burial
// ---------------------------------------------------------------------------

/// Height above a kill-zone cell's floor a volley aims at: centre mass of a
/// standing player (a 1.8-tall hitbox with feet on the floor). Aiming at the
/// centre rather than the feet means the shot passes through the hitbox for the
/// whole cell rather than grazing it.
const VOLLEY_AIM_HEIGHT: f64 = 1.0;

/// Speed of a summoned volley projectile, in blocks/tick.
///
/// Arrow impact damage in 1.21.11 is `ceil(|velocity| * damage)` with `damage`
/// defaulting to 2.0, so 2.5 b/t lands 5 half-hearts per arrow — a real
/// consequence that three salvos of saturating fire can kill, without any one
/// arrow being an instant death.
pub const VOLLEY_SPEED: f64 = 2.5;

/// One computed shot of a volley: the kill-zone cell it covers and the exact
/// `Motion` vector that carries a projectile from the gallery slot into it.
#[derive(Debug, Clone, PartialEq)]
pub struct VolleyShot {
    /// The standable kill-zone cell this shot covers.
    pub cell: [i32; 3],
    /// The `Motion` NBT vector, in blocks/tick.
    pub motion: [f64; 3],
}

/// A proven volley: every standable cell of the kill zone, each with the
/// velocity vector that reaches it.
#[derive(Debug, Clone)]
pub struct VolleyGeometry {
    /// The gallery slot cell the projectiles are summoned in.
    pub from: [i32; 3],
    /// One shot per standable kill-zone cell, in ascending cell order
    /// (deterministic — ADR-0006; no RNG anywhere in the pattern).
    pub shots: Vec<VolleyShot>,
}

/// The exact world-space point a volley projectile is summoned at.
pub fn volley_source(from: [i32; 3]) -> [f64; 3] {
    let c = cell_center(from);
    [c[0], c[1], c[2]]
}

/// The exact world-space point a volley shot aims at for kill-zone cell `c`.
pub fn volley_target(c: [i32; 3]) -> [f64; 3] {
    let p = cell_center(c);
    [p[0], p[1] - 0.5 + VOLLEY_AIM_HEIGHT, p[2]]
}

impl World {
    /// Whether a cell stops a projectile. Collision geometry stops it outright;
    /// water is included because it destroys a flat trajectory rather than
    /// merely slowing it — a shot that has to swim is not a shot that arrives.
    ///
    /// Deliberately NOT `blocks_camera`: glass is transparent to a camera and
    /// solid to an arrow, so reusing the sight predicate would prove coverage
    /// through a window the projectile cannot pass.
    fn blocks_projectile(&self, c: [i32; 3]) -> bool {
        self.is_occupied(c)
    }

    /// The first cell that stops a projectile flying `from` → `to`, or `None`
    /// when the line of fire is clear. The origin cell is exempt: that is where
    /// the projectile is summoned.
    ///
    /// Uses the same [`walk_cells`] traversal as the cutscene clip and the mob
    /// line-of-sight, so "can this be traversed" has one definition in the
    /// compiler. Critically, the ray checked here is *exactly* the segment the
    /// emitted `Motion` vector flies (projectiles are summoned `NoGravity`, and
    /// drag scales speed without turning the path), so the proof and the runtime
    /// cannot drift apart.
    fn first_projectile_block(&self, from: [f64; 3], to: [f64; 3]) -> Option<[i32; 3]> {
        let origin = [
            from[0].floor() as i32,
            from[1].floor() as i32,
            from[2].floor() as i32,
        ];
        walk_cells(from, to, |c| c != origin && self.blocks_projectile(c))
    }

    /// Whether this cell is clear enough to summon a projectile in.
    pub fn is_volley_slot_clear(&self, c: [i32; 3]) -> bool {
        !self.blocks_projectile(c)
    }
}

/// Plan a volley, proving saturation by construction (spec-0022).
///
/// The returned geometry contains one shot per standable kill-zone cell — so
/// emitting every shot IS the coverage — and the function errors rather than
/// returning partial cover. `label` names the volley in diagnostics.
pub fn plan_volley(
    world: &World,
    from: [i32; 3],
    region: ([i32; 3], [i32; 3]),
    label: &str,
) -> Result<VolleyGeometry, Failure> {
    if !world.is_volley_slot_clear(from) {
        return Err(Failure {
            code: DW_VOLLEY_SLOT_OCCLUDED,
            message: format!(
                "{label}: the `from_anchor` cell [{}, {}, {}] is solid or flooded, so a \
                 summoned projectile would never leave it. Move the gallery slot into the \
                 open air of the firing niche (the anchor marks where the projectile \
                 spawns, not the wall it comes out of)",
                from[0], from[1], from[2]
            ),
        });
    }
    let src = volley_source(from);
    let mut shots = Vec::new();
    // BTreeSet-ordered cells: the pattern is a pure function of the geometry,
    // with no RNG and no hash-order iteration (ADR-0006).
    let cells: Vec<[i32; 3]> = crate::compiler::assembled::region_cells(region.0, region.1)
        .filter(|c| world.is_standable(*c))
        .collect();
    if cells.is_empty() {
        return Err(Failure {
            code: DW_TRAP_REGION_EMPTY,
            message: format!(
                "{label}: the `kill_zone` region [{}, {}, {}]..[{}, {}, {}] contains no \
                 standable cell, so there is nothing for the volley to saturate — it would \
                 fire into geometry no player can occupy. Point `kill_zone` at the floor \
                 players actually cross (the stair treads, the corridor run), not at the \
                 wall or the air above it",
                region.0[0], region.0[1], region.0[2], region.1[0], region.1[1], region.1[2]
            ),
        });
    }
    for cell in cells {
        let dst = volley_target(cell);
        if let Some(block) = world.first_projectile_block(src, dst) {
            return Err(Failure {
                code: DW_VOLLEY_ZONE_UNCOVERED,
                message: format!(
                    "{label}: the gallery slot [{}, {}, {}] has no line of fire to \
                     kill-zone cell [{}, {}, {}] — the shot is stopped at [{}, {}, {}]. A \
                     volley must BLANKET its kill zone: an uncovered cell is a pocket a \
                     player is safe in by accident, which turns dodging from a decision \
                     into luck. Either clear the obstruction, move `from_anchor` where it \
                     sees the whole zone, or shrink `kill_zone` to the part it does cover",
                    from[0],
                    from[1],
                    from[2],
                    cell[0],
                    cell[1],
                    cell[2],
                    block[0],
                    block[1],
                    block[2]
                ),
            });
        }
        let d = [dst[0] - src[0], dst[1] - src[1], dst[2] - src[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let motion = if len <= f64::EPSILON {
            [0.0, 0.0, 0.0]
        } else {
            [
                d[0] / len * VOLLEY_SPEED,
                d[1] / len * VOLLEY_SPEED,
                d[2] / len * VOLLEY_SPEED,
            ]
        };
        shots.push(VolleyShot { cell, motion });
    }
    Ok(VolleyGeometry { from, shots })
}

/// A proven collapse: what falls, and where it comes to rest.
#[derive(Debug, Clone)]
pub struct CollapseGeometry {
    /// The region whose blocks are deleted.
    pub region: ([i32; 3], [i32; 3]),
    /// Cells that currently hold a block — one `falling_block` summon each, in
    /// ascending cell order.
    pub drops: Vec<[i32; 3]>,
    /// Where the debris settles, in ascending cell order. This is the geometry
    /// the completability proof treats as solid.
    pub debris: Vec<[i32; 3]>,
    /// The tallest fall, in blocks — drives the `then_floor` paving delay.
    pub max_fall: i32,
}

impl CollapseGeometry {
    /// **The debris cells a body cannot pass once the collapse has landed**:
    /// each column's top cell holds `then_floor` when one is authored (the
    /// paving `setblock` replaces it), every other debris cell the falling
    /// block. A cell whose block a body passes through
    /// ([`delvewright_dsl::blockshape::passes_body`], the pinned collision
    /// table) walls nothing and floors nothing, so it is not sealed.
    pub fn walls(&self, falling_block: &str, then_floor: Option<&str>) -> BTreeSet<[i32; 3]> {
        let mut tops: BTreeMap<[i32; 2], i32> = BTreeMap::new();
        for d in &self.debris {
            let e = tops.entry([d[0], d[2]]).or_insert(d[1]);
            *e = (*e).max(d[1]);
        }
        self.debris
            .iter()
            .copied()
            .filter(|d| {
                let block = match then_floor {
                    Some(f) if tops.get(&[d[0], d[2]]) == Some(&d[1]) => f,
                    _ => falling_block,
                };
                !delvewright_dsl::blockshape::passes_body(block)
            })
            .collect()
    }
}

/// Plan a collapse and settle its debris deterministically (spec-0022).
///
/// Settling reuses the assembled model's rule — a falling block comes to rest on
/// the first solid cell beneath it, stacking within its own column — so the
/// post-collapse world the proof reasons over is the world the server will
/// actually have.
pub fn plan_collapse<V>(
    world: &World,
    blocks: &BTreeMap<[i32; 3], V>,
    region: ([i32; 3], [i32; 3]),
    label: &str,
) -> Result<CollapseGeometry, Failure> {
    let drops: Vec<[i32; 3]> = crate::compiler::assembled::region_cells(region.0, region.1)
        .filter(|c| blocks.contains_key(c))
        .collect();
    if drops.is_empty() {
        return Err(Failure {
            code: DW_TRAP_REGION_EMPTY,
            message: format!(
                "{label}: the `collapse` region [{}, {}, {}]..[{}, {}, {}] contains no \
                 blocks, so nothing would fall. Point `region_anchor` at the ceiling slab \
                 that caves in, not at the air below it",
                region.0[0], region.0[1], region.0[2], region.1[0], region.1[1], region.1[2]
            ),
        });
    }
    let lo_y = region.0[1].min(region.1[1]);
    // Group the drops by column so a stack settles as a stack.
    let mut by_col: BTreeMap<[i32; 2], usize> = BTreeMap::new();
    for c in &drops {
        *by_col.entry([c[0], c[2]]).or_insert(0) += 1;
    }
    let mut debris: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut max_fall = 0;
    let mut landed_any = false;
    for (col, n) in by_col {
        // Find the first solid cell below the region in this column: the debris
        // rests on top of it. Search stops at the world floor.
        let mut rest: Option<i32> = None;
        let mut y = lo_y - 1;
        while y > lo_y - MAX_COLLAPSE_FALL {
            if world.is_solid([col[0], y, col[1]]) {
                rest = Some(y + 1);
                break;
            }
            y -= 1;
        }
        let Some(base) = rest else { continue };
        landed_any = true;
        max_fall = max_fall.max(lo_y - base);
        for k in 0..n as i32 {
            debris.insert([col[0], base + k, col[1]]);
        }
    }
    if !landed_any {
        return Err(Failure {
            code: DW_TRAP_REGION_EMPTY,
            message: format!(
                "{label}: nothing beneath the `collapse` region [{}, {}, {}]..[{}, {}, {}] \
                 stops the debris within {MAX_COLLAPSE_FALL} blocks — the falling blocks \
                 would drop out of the box garden instead of burying anyone. Put the \
                 region over the floor the players walk on",
                region.0[0], region.0[1], region.0[2], region.1[0], region.1[1], region.1[2]
            ),
        });
    }
    Ok(CollapseGeometry {
        region,
        drops,
        debris: debris.into_iter().collect(),
        max_fall,
    })
}

/// How far debris is allowed to fall before the compiler calls the collapse
/// unmodellable. Well beyond any box-garden room height.
const MAX_COLLAPSE_FALL: i32 = 64;

/// Prove the critical path survives every collapse (spec-0022, `DW0445`).
///
/// A trap can always fire — the player WILL step on the plate — so the world the
/// completability proof must hold in is the world after the collapse, not
/// before. This is the same pessimism the `shortcut` seal applies (a shortcut is
/// proven never-taken; a trap is proven always-sprung), and it is deliberately
/// conservative in one direction: the debris is added as solid geometry
/// ([`CollapseGeometry::walls`]: every landed cell a body cannot pass) while
/// the deleted region is left in place, so the proof can only ever be stricter
/// than the real post-collapse world, never laxer.
pub fn check_collapses(
    plan: &Plan,
    world: &World,
    collapses: &[(String, BTreeSet<[i32; 3]>)],
) -> Result<(), Failure> {
    for (label, walls) in collapses {
        let collapsed = world.with_sealed(walls);
        if let Err(e) = check_critical_path(plan, &collapsed) {
            return Err(Failure {
                code: DW_COLLAPSE_BURIES_PATH,
                message: format!(
                    "{label}: the critical path is no longer completable once this collapse \
                     has fired — the debris buries the route ({}). A trap is proven in its \
                     SPRUNG state, because a player will step on the trigger: either leave a \
                     way through the rubble, drop fewer layers (a shallower `region_anchor`), \
                     or move the collapse off the forced path",
                    e.message
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;
    use std::collections::BTreeSet;

    use crate::compiler::plan::RegionEvents;
    use crate::compiler::plan::TrapDisarmPlan;
    use crate::compiler::plan::{RegionEvent, RegionWrite};
    use delvewright_dsl::{Lethality, TrapReset, TrapTrigger};

    // --- collapse debris, judged by its landed block (spec-0022, DW0445) ---

    /// A two-high pile in one column and a one-high pile in another: every cell
    /// of stone debris walls; a carpet paved over the tops walls neither top,
    /// and a light block that falls walls nothing.
    #[test]
    fn collapse_debris_walls_only_where_its_landed_block_collides() {
        let g = CollapseGeometry {
            region: ([0, 70, 0], [1, 71, 0]),
            drops: vec![[0, 70, 0], [0, 71, 0], [1, 70, 0]],
            debris: vec![[0, 65, 0], [0, 66, 0], [1, 65, 0]],
            max_fall: 5,
        };
        let all: BTreeSet<[i32; 3]> = g.debris.iter().copied().collect();
        assert_eq!(g.walls("minecraft:gravel", None), all);
        assert_eq!(
            g.walls("minecraft:gravel", Some("minecraft:white_carpet")),
            BTreeSet::from([[0, 65, 0]]),
            "each column's top holds the carpet the paving sets"
        );
        assert!(g.walls("minecraft:light", None).is_empty());
    }

    /// A minimal lethal trap for the proof tests.
    fn lethal_trap(cell: [i32; 3], reset: TrapReset, disarm: Option<TrapDisarmPlan>) -> TrapPlan {
        TrapPlan {
            id: "trap/darts".to_string(),
            safe: "darts".to_string(),
            trigger: TrapTrigger::PressurePlate,
            at_anchor: "anchor/trap".to_string(),
            trigger_cell: cell,
            dispenser: None,
            payload: None,
            payload_effects: Vec::new(),
            lethality: Lethality::Lethal,
            reset,
            disarm,
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
        }
    }

    #[test]
    fn forced_lethal_rearm_trap_with_no_discharge_is_dw0342() {
        // A rearming lethal trap on a required chokepoint, no disarm → soft-loop.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        let err = verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).unwrap_err();
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE);
    }

    #[test]
    fn forced_lethal_once_trap_is_survivable() {
        // The same forced trap set to `once` fires and is spent — no soft-loop.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let traps = [lethal_trap(tc, TrapReset::Once, None)];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).is_ok());
    }

    #[test]
    fn off_path_lethal_trap_is_avoidable() {
        // A rearming lethal trap whose trigger cell is NOT a required path cell.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = BTreeSet::new(); // path avoids the trap
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).is_ok());
    }

    #[test]
    fn forced_lethal_trap_with_reachable_disarm_is_discharged() {
        // Disarm affordance BEFORE the trap on the corridor (reachable from spawn
        // without crossing the trap cell) → disarmable.
        let world = corridor(6, 65);
        let tc = [4, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let disarm = TrapDisarmPlan {
            via_anchor: "anchor/lever".to_string(),
            via_cell: [1, 65, 1],
            sets_flag: "flag/darts-off".to_string(),
        };
        let traps = [lethal_trap(tc, TrapReset::Rearm, Some(disarm))];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).is_ok());
    }

    #[test]
    fn forced_lethal_trap_with_disarm_behind_the_trap_is_dw0342() {
        // The only route to the disarm crosses the trap chokepoint, so the disarm
        // cannot be reached first → still a soft-loop → DW0342.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let disarm = TrapDisarmPlan {
            via_anchor: "anchor/lever".to_string(),
            via_cell: [5, 65, 1],
            sets_flag: "flag/darts-off".to_string(),
        };
        let traps = [lethal_trap(tc, TrapReset::Rearm, Some(disarm))];
        let err = verify_traps(&world, &traps, &required, &[[0, 65, 1]], &[]).unwrap_err();
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE);
    }

    #[test]
    fn non_lethal_forced_trap_carries_no_obligation() {
        // A harmful (non-lethal) trap on the forced path is fine — no DW0342.
        let world = corridor(6, 65);
        let mut t = lethal_trap([3, 65, 1], TrapReset::Rearm, None);
        t.lethality = Lethality::Harmful;
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        assert!(verify_traps(&world, &[t], &required, &[[0, 65, 1]], &[]).is_ok());
    }

    #[test]
    fn a_lethal_plate_on_a_close_gate_detour_is_forced_and_dw0342() {
        // Same room. A rearming lethal plate sits on the DETOUR lane at [2,65,2].
        // With the gate open the player walks the short lane and the trap is
        // genuinely avoidable. Once the `close-gate` seals the short lane, the
        // detour is forced and the plate becomes a provable soft-loop — which the
        // old unsealed forced-cell set could not see.
        let mut world = two_lane_room(65);
        world.solid.remove(&[0, 65, 1]);
        world.solid.remove(&[0, 66, 1]);
        world.solid.remove(&[4, 65, 1]);
        world.solid.remove(&[4, 66, 1]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let close = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Fill, 0);
        let tc = [2, 65, 2];
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        let spawn = [[0, 65, 0]];

        let open_legs = route_walked_legs(&world, &[a, b], &RegionEvents::default(), &linear);
        let open_required: BTreeSet<[i32; 3]> = open_legs
            .iter()
            .flat_map(|(l, _)| l.cells.clone())
            .collect();
        assert!(
            verify_traps(&world, &traps, &open_required, &spawn, &open_legs).is_ok(),
            "with the gate open the plate is genuinely avoidable"
        );

        let sealed_legs = route_walked_legs(
            &world,
            &[a, b],
            &RegionEvents::from(vec![close.clone()]),
            &linear,
        );
        let sealed_required: BTreeSet<[i32; 3]> = sealed_legs
            .iter()
            .flat_map(|(l, _)| l.cells.clone())
            .collect();
        let err = verify_traps(&world, &traps, &sealed_required, &spawn, &sealed_legs)
            .expect_err("the sealed detour forces the party across the plate");
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE); // DW0342
    }
}
