/// Prove a `volley`'s cadence is a timing read — [`DW_VOLLEY_COIN_FLIP`]
/// (`DW0918`).
///
/// **What must be readable is the way out.** spec-0022 defines a volley as
/// saturation: every standable cell of the kill zone is fired on every salvo, so
/// "escaping means LEAVING the zone, a decision, not a lucky strafe". A volley is
/// finite and does not loop, so a body outside the zone is never forced through
/// it — the chain ends, and a `rearm` trap fires again only when someone steps on
/// its trigger. The one body a volley can trap is the one inside the zone when a
/// salvo lands (the party's own step on a pressure plate fires salvo 0 the same
/// tick), and the decision spec-0022 promises it is to walk out before the next
/// salvo. So the route judged is the **escape**: from the standable kill-zone
/// cell whose walk out is longest, the moves to the nearest standable cell
/// outside the zone, by the same router and step rule every route proof uses.
///
/// The window is the interval between two salvos. The body sets off `p` ticks
/// after a salvo and is clear iff it stands outside when the next one is
/// summoned, so this is [`timing_read`] with `open_ticks = interval` and nothing
/// shut: the admitting phases must cover [`TIMING_READ_MIN_ADMIT_PERCENT`] of the
/// interval — a reaction window the player can read, not a sprint that has to
/// start on the frame the plate clicks. Projectile flight is not credited as
/// extra time (the proof never grants a share it does not have).
///
/// A volley with `salvos: 1` has no cadence — no next salvo exists to escape —
/// and is not judged: its single salvo is the trap's consequence, the way a
/// `collapse` is, and whether it can be watched before it is triggered is
/// `DW0388`'s question. A zone with no standable cell is `DW0444`'s. A standable
/// zone cell from which no standable cell outside the zone can be reached at all
/// is the limit of this rule — no phase admits an escape — and is refused.
pub fn check_volley_cadence(
    world: &World,
    region: ([i32; 3], [i32; 3]),
    salvos: u32,
    interval: u32,
    label: &str,
) -> Result<(), Failure> {
    if salvos < 2 {
        return Ok(());
    }
    let zone: BTreeSet<[i32; 3]> =
        crate::compiler::assembled::region_cells(region.0, region.1).collect();
    let cells: Vec<[i32; 3]> = zone
        .iter()
        .copied()
        .filter(|c| world.is_standable(*c))
        .collect();
    // Every way out passes through a first cell outside the zone, and that cell
    // is a step-rule neighbour of a zone cell — so these are all the exits.
    let exits: BTreeSet<[i32; 3]> = cells
        .iter()
        .flat_map(|c| world.neighbors(*c))
        .filter(|n| !zone.contains(n))
        .collect();
    // The worst cell: the longest shortest walk out. Ties keep the first cell in
    // ascending order (ADR-0006).
    let mut worst: Option<([i32; 3], Option<u32>)> = None;
    for &cell in &cells {
        let out = exits
            .iter()
            .filter_map(|&e| world.find_path(cell, e))
            .map(|path| path.len().saturating_sub(1) as u32)
            .min();
        let worse = match (&worst, out) {
            (None, _) => true,
            (Some((_, Some(_))), None) => true,
            (Some((_, Some(w))), Some(m)) => m > *w,
            (Some((_, None)), _) => false,
        };
        if worse {
            worst = Some((cell, out));
        }
    }
    let Some((cell, moves)) = worst else {
        return Ok(()); // no standable cell — DW0444's business
    };
    let Some(moves) = moves else {
        return Err(Failure {
            code: DW_VOLLEY_COIN_FLIP,
            message: format!(
                "{label}: a player standing on kill-zone cell [{}, {}, {}] cannot walk out of \
                 the zone at all — no standable cell outside it is reachable — so every one of \
                 the {salvos} salvos lands on them and no timing of theirs changes that. A \
                 volley's counterplay is LEAVING the zone (spec-0022); give the zone an exit \
                 a body can step onto, or shrink `kill_zone` to the floor that has one — never \
                 lower the floor of the proof.",
                cell[0], cell[1], cell[2]
            ),
        });
    };
    let read = timing_read(moves, interval, 0);
    if read.is_coin_flip() {
        let TimingRead {
            moves,
            cross_ticks,
            admits,
            cycle,
            percent,
        } = read;
        return Err(Failure {
            code: DW_VOLLEY_COIN_FLIP,
            message: format!(
                "{label} is a coin flip, not a timing read: from kill-zone cell [{}, {}, {}] \
                 the walk out of the zone takes {cross_ticks} ticks ({moves} blocks at \
                 {SPRINT_TICKS_PER_BLOCK} t/block), so only {admits} of its {cycle}-tick salvo \
                 interval ({percent}%) admit a player who sets off then — under the \
                 {TIMING_READ_MIN_ADMIT_PERCENT}% floor every timed hazard is held to \
                 (spec-0016 §4). A volley's counterplay is LEAVING the zone (spec-0022); an \
                 interval shorter than the walk out takes that decision away. Lengthen \
                 `interval`, or shrink `kill_zone` so no cell is deep inside it — never lower \
                 the floor.",
                cell[0], cell[1], cell[2]
            ),
        });
    }
    Ok(())
}

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
/// conservative in one direction: the debris is added as solid geometry while
/// the deleted region is left in place, so the proof can only ever be stricter
/// than the real post-collapse world, never laxer.
pub fn check_collapses(
    plan: &Plan,
    world: &World,
    collapses: &[(String, CollapseGeometry)],
) -> Result<(), Failure> {
    for (label, g) in collapses {
        let debris: BTreeSet<[i32; 3]> = g.debris.iter().copied().collect();
        let collapsed = world.with_sealed(&debris);
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

    // --- volley cadence (spec-0022, DW0918) ---

    /// A kill zone five cells deep down a one-wide hall at x=0 (z=3..=7). The
    /// middle cell is 3 blocks from the floor outside either end, so the walk out
    /// costs 12 ticks at the sprint model.
    fn volley_hall() -> (World, ([i32; 3], [i32; 3])) {
        (carved(&run_z(0, 0, 12)), ([0, WY, 3], [0, WY, 7]))
    }

    /// 15 ticks between salvos against a 12-tick walk out: 4 of 15 phases (26%)
    /// admit an escape. A readable cadence.
    #[test]
    fn volley_whose_interval_admits_the_walk_out_is_readable() {
        let (world, zone) = volley_hall();
        check_volley_cadence(&world, zone, 3, 15, "volley")
            .expect("4 of 15 phases admit the walk out: a timing read");
    }

    /// The same zone with 13 ticks between salvos: 2 of 13 phases (15%) admit the
    /// walk out — under the floor. `DW0918`, naming the cell it judged.
    #[test]
    fn volley_whose_interval_barely_admits_the_walk_out_is_dw0918() {
        let (world, zone) = volley_hall();
        let err = check_volley_cadence(&world, zone, 3, 13, "volley")
            .expect_err("15% of the interval is a coin flip");
        assert_eq!(err.code, DW_VOLLEY_COIN_FLIP); // DW0918
        assert!(err.message.contains("coin flip"), "{}", err.message);
        assert!(err.message.contains("[0, 65, 5]"), "{}", err.message);
        assert!(err.message.contains("2 of its 13-tick"), "{}", err.message);
    }

    /// One salvo has no cadence: there is no next salvo to walk out ahead of, so
    /// even an interval no escape fits in is not judged.
    #[test]
    fn a_single_salvo_volley_has_no_cadence_to_judge() {
        let (world, zone) = volley_hall();
        check_volley_cadence(&world, zone, 1, 1, "volley").expect("salvos: 1 is not judged");
    }

    /// A zone that fills a sealed chamber has no way out at all: every salvo lands
    /// and no timing changes that — the limit of the same rule.
    #[test]
    fn a_volley_zone_with_no_way_out_is_dw0918() {
        let world = carved(&run_z(0, 0, 4));
        let err = check_volley_cadence(&world, ([0, WY, 0], [0, WY, 4]), 3, 200, "volley")
            .expect_err("no exit admits no phase");
        assert_eq!(err.code, DW_VOLLEY_COIN_FLIP); // DW0918
        assert!(err.message.contains("cannot walk out"), "{}", err.message);
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
