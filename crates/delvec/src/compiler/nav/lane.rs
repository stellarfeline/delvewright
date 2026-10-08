/// The proven lane polyline of every wave that declares one (spec-0016 §6): wave
/// id → the snapped, walk-connected waypoint cells in march order. These cells —
/// not the raw anchor positions — are what the compiler writes into
/// `patrol_target`, so the squad is always sent somewhere it can actually stand.
pub type LaneRoutes = BTreeMap<String, Vec<[i32; 3]>>;

/// The minimum legal distance between consecutive lane waypoints, in blocks
/// (`DW0386`). Vanilla re-rolls a patrol target to a random point once the
/// patroller gets within 10 blocks of it, so a leg of 10 or less is a leg the
/// engine stops following; the spike's measured working default is 12.
const LANE_MIN_LEG: f64 = 10.0;

/// Resolve and prove every TD lane (spec-0016 §6), raising `DW0386` on the first
/// failure.
///
/// Four obligations per lane, in the order an author hits them:
/// 1. every waypoint anchor resolves in the wave's area;
/// 2. every waypoint has standable footing within [`SNAP_RADIUS`];
/// 3. every leg — including the one from the spawn anchor to the first waypoint —
///    is genuinely walkable by the squad;
/// 4. every leg is longer than [`LANE_MIN_LEG`].
///
/// Routed over the **no-gate-use** view, exactly like wave seating: lane mobs
/// cannot right-click a fence gate open, so a lane that "works" only by walking
/// through one does not work.
pub fn plan_lanes(plan: &Plan, world: &World) -> Result<LaneRoutes, Failure> {
    let entity_world_owned;
    let world: &World = if world.has_use_gates() {
        entity_world_owned = world.without_gate_use();
        &entity_world_owned
    } else {
        world
    };
    let c = plan.campaign;
    let mut out: LaneRoutes = BTreeMap::new();
    for w in &c.quests.content.waves {
        let Some(lane) = &w.lane else { continue };
        let area = crate::compiler::plan::wave_area(c, w.id.as_str());
        let anchor = area.and_then(|a| plan.point(a, w.anchor.as_str()));
        let (Some(area), Some(anchor)) = (area, anchor) else {
            // An unresolvable spawn anchor is DW0310's concern (the dangling
            // `spawn-wave`); do not double-report it here.
            continue;
        };
        let fail = |message: String| Failure {
            code: DW_LANE_GEOMETRY,
            message,
        };
        let mut cells: Vec<[i32; 3]> = Vec::new();
        let mut prev = world.snap_standable(anchor, SNAP_RADIUS).ok_or_else(|| {
            fail(format!(
                "lane wave `{}`: its spawn anchor `{}` ({anchor:?}) has no standable footing \
                 within {SNAP_RADIUS} blocks, so the squad has nowhere to form up before the \
                 march (spec-0016 §6)",
                w.id,
                w.anchor.as_str()
            ))
        })?;
        let mut prev_name = w.anchor.as_str().to_string();
        for wp in &lane.waypoints {
            let Some(pos) = plan.point(area, wp.as_str()) else {
                return Err(fail(format!(
                    "lane wave `{}`: waypoint anchor `{}` resolves to no position in area `{area}` \
                     (spec-0016 §6). A lane is a polyline of REAL places; use an anchor the area's \
                     assembled prefabs actually expose.",
                    w.id,
                    wp.as_str()
                )));
            };
            let cell = world.snap_standable(pos, SNAP_RADIUS).ok_or_else(|| {
                fail(format!(
                    "lane wave `{}`: waypoint `{}` ({pos:?}) has no standable footing within \
                     {SNAP_RADIUS} blocks (spec-0016 §6) — a patrol target the squad cannot stand \
                     on is a target it never arrives at, so the lane stalls there forever",
                    w.id,
                    wp.as_str()
                ))
            })?;
            if world.find_path(prev, cell).is_none() {
                return Err(fail(format!(
                    "lane wave `{}`: the leg `{prev_name}` ({prev:?}) → `{}` ({cell:?}) is not \
                     walkable (spec-0016 §6). The squad marches this polyline on foot with native \
                     pathfinding; a leg the compiler cannot walk is a leg the mobs cannot walk. \
                     Note lane mobs cannot open fence gates — the proof runs on the same \
                     no-gate-use view wave seating uses.",
                    w.id,
                    wp.as_str()
                )));
            }
            let leg = (0..3)
                .map(|i| f64::from(cell[i] - prev[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            if leg <= LANE_MIN_LEG {
                return Err(fail(format!(
                    "lane wave `{}`: the leg `{prev_name}` ({prev:?}) → `{}` ({cell:?}) is {leg:.1} \
                     blocks, which is not more than {LANE_MIN_LEG:.0} (spec-0016 §6). Vanilla \
                     re-rolls a patrol target to a RANDOM point once the patroller is within 10 \
                     blocks of it, so a leg this short is one the engine quietly stops following \
                     and the squad wanders off-lane — it reads as working-but-drunk, not as a bug. \
                     Space lane waypoints at least 12 blocks apart (the spike's measured default), \
                     or drop this waypoint.",
                    w.id,
                    wp.as_str()
                )));
            }
            cells.push(cell);
            prev = cell;
            prev_name = wp.as_str().to_string();
        }
        out.insert(w.id.as_str().to_string(), cells);
    }
    Ok(out)
}

/// The measured off-lane drift of a marching TD squad, in blocks — the constraint
/// source is the td-routing-spike dossier (`docs/notes/td-routing-spike.md`,
/// "Lane fidelity": followers deviate mean ≤3.2, **max 7.9** blocks off the lane
/// polyline, 116 samples). A marching squad is a CORRIDOR around its polyline,
/// not a line: a placement can clear the centre-line by 2 blocks and still stand
/// inside the marching mobs' real aggro reach — run nine's live death at 17.7
/// blocks from a 16-`follow_range` lane. `DW0478`'s lane term is therefore
/// `follow_range + LANE_MARCH_DRIFT`; stationary
/// spawn/staging cells keep the plain `follow_range` term.
pub const LANE_MARCH_DRIFT: f64 = 7.9;
