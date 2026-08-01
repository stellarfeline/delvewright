//! Compile-time navigation over the solved voxel grid (spec-0008 addendum).
//!
//! The compiler owns the assembled geometry, so two v0.4 verbs are made
//! collision-safe *by construction* here rather than trusting downstream runtime
//! behaviour (CLAUDE.md "no hacks at any layer"):
//!
//! - **`move-npc`** walks a real path planned by A* over the placed-world block
//!   data (pieces + the solver's socket seals). Waypoints step only through
//!   passable cells and stand on solid ground — no wall-clipping (owner playtest
//!   finding). An unroutable move is [`DW_MOVE_UNROUTABLE`] (`DW0307`), a compile
//!   error, not a runtime glitch.
//! - **`cutscene`** camera dolly paths are validated to pass only through
//!   non-solid blocks. Cameras fly (exempt from walkability) but must not clip a
//!   solid; a violation is [`DW_CUTSCENE_CLIP`] (`DW0308`).
//!
//! **Gate cells are passable.** A `ResolvedAnchor::Gate` region is a
//! compiler-managed openable threshold (an `open-gate` effect fills it with air),
//! never a wall, so its cells are treated as passable for both planning and the
//! camera check. Modelling a sealed gate as an obstacle would wrongly forbid an
//! NPC from walking through a doorway the campaign opens.
//!
//! Determinism (ADR-0006): the solid set is a `BTreeSet`, the A* frontier breaks
//! ties on `(f, g, cell)` in a fixed order, and neighbour expansion order is
//! fixed — same DSL + seed → identical waypoints.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::io::Read;

use delvewright_dsl::{CameraWaypoint, QuestEffect};

use crate::plan::{Plan, ResolvedAnchor, Step};

/// `DW0307`: a `move-npc` destination unreachable by any walkable path from the
/// NPC's position over the assembled geometry.
pub const DW_MOVE_UNROUTABLE: &str = "DW0307";
/// `DW0308`: a `cutscene` camera dolly path that passes through a solid block.
pub const DW_CUTSCENE_CLIP: &str = "DW0308";
/// `DW0311`: a consecutive pair of player-visited critical-path anchors that no
/// walkable path connects over the assembled geometry (with no inter-area
/// transport between them) — the player would be stranded. Turns the whole
/// "assembled seams aren't walkable" bug class (task #34: a prefab regen wedged a
/// doorway shut / opened a void gap and only a runtime bot caught it) into a
/// compile error.
pub const DW_CRITICAL_UNROUTABLE: &str = "DW0311";

/// Default NPC walking speed in blocks/tick (spec-0008 §5; owner spike). Used when
/// a `move-npc` effect omits `speed`.
pub const DEFAULT_SPEED: f64 = 0.15;

/// How far to search for a standable floor cell when a `move-npc` endpoint anchor
/// is a solid affordance (altar / gate bars / wall marker) the NPC must stop in
/// front of rather than stand inside.
const SNAP_RADIUS: i32 = 3;

/// A build diagnostic raised by navigation planning (mapped to exit 3, `DW03xx`).
#[derive(Debug)]
pub struct NavError {
    /// The stable diagnostic code (`DW0307` / `DW0308`).
    pub code: &'static str,
    /// Human-readable explanation, naming the offending NPC / endpoints / segment.
    pub message: String,
}

/// A planned `move-npc`: the resolved endpoints plus the per-tick waypoint
/// polyline the emitter teleports the NPC body + interaction hitbox along.
/// `waypoints[0]` is the origin and `waypoints.last()` is exactly the integer
/// target cell; there are `ticks() + 1` entries.
#[derive(Debug, Clone)]
pub struct MovePlan {
    /// The moving NPC id (`npc/…`).
    pub npc: String,
    /// The destination anchor id (`anchor/…`).
    pub to_anchor: String,
    /// The integer target cell (feet), for the arrival assertion.
    pub target: [i32; 3],
    /// Per-tick world positions along the walked path.
    pub waypoints: Vec<[f64; 3]>,
}

impl MovePlan {
    /// The final tick index (`waypoints.len() - 1`).
    pub fn ticks(&self) -> usize {
        self.waypoints.len().saturating_sub(1)
    }
}

fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// Inclusive cell iterator over a region given two (possibly unordered) corners.
fn region_cells(a: [i32; 3], b: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    (lo[1]..=hi[1]).flat_map(move |y| {
        (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z]))
    })
}

/// Parse a gzipped vanilla structure `.nbt`, returning its non-air block cells as
/// local `[x, y, z]` positions. Mirrors the palette/blocks decode the emitter's
/// sentinel picker uses. Unparseable structures contribute nothing.
fn structure_solid_cells(bytes: &[u8]) -> Vec<[i32; 3]> {
    let mut raw = Vec::new();
    if flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut raw)
        .is_err()
    {
        return Vec::new();
    }
    let Ok(fastnbt::Value::Compound(root)) = fastnbt::from_bytes::<fastnbt::Value>(&raw) else {
        return Vec::new();
    };
    let palette: Vec<Option<String>> = match root.get("palette") {
        Some(fastnbt::Value::List(entries)) => entries
            .iter()
            .map(|e| match e {
                fastnbt::Value::Compound(c) => match c.get("Name") {
                    Some(fastnbt::Value::String(s)) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect(),
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Some(fastnbt::Value::List(blocks)) = root.get("blocks") {
        for b in blocks {
            let fastnbt::Value::Compound(b) = b else {
                continue;
            };
            let pos = match b.get("pos") {
                Some(fastnbt::Value::List(p)) if p.len() == 3 => {
                    let mut o = [0i32; 3];
                    let mut ok = true;
                    for (i, v) in p.iter().enumerate() {
                        match v {
                            fastnbt::Value::Int(n) => o[i] = *n,
                            _ => ok = false,
                        }
                    }
                    if !ok {
                        continue;
                    }
                    o
                }
                _ => continue,
            };
            let state = match b.get("state") {
                Some(fastnbt::Value::Int(n)) => *n as usize,
                _ => continue,
            };
            match palette.get(state) {
                Some(Some(name)) if !is_air(name) => out.push(pos),
                _ => {}
            }
        }
    }
    out
}

/// A solid-block occupancy model of the assembled world (spec-0008 addendum),
/// built from the placed prefab structures plus the solver's socket seals. Cells
/// absent from `solid` are passable (interior air, opened sockets, gate
/// thresholds).
pub struct World {
    solid: BTreeSet<[i32; 3]>,
}

impl World {
    /// Build the occupancy model from the plan's placed pieces and the structure
    /// `.nbt` bytes. Applies the solver's seals (air clears an opened socket;
    /// anything else seals it) and clears gate anchor regions to passable.
    pub fn from_plan(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Self {
        let mut solid = BTreeSet::new();
        for area in &plan.areas {
            for piece in &area.pieces {
                let Some(bytes) = structures.get(&piece.structure_file) else {
                    continue;
                };
                for local in structure_solid_cells(bytes) {
                    let t = piece.rotation.transform(local);
                    solid.insert([
                        piece.pos[0] + t[0],
                        piece.pos[1] + t[1],
                        piece.pos[2] + t[2],
                    ]);
                }
            }
            // Seals land after placement: an air fill opens a mated socket (clears
            // the jigsaw block); a solid fill seals an unused socket.
            for s in &area.seals {
                let clear = is_air(&s.block);
                for cell in region_cells(s.from, s.to) {
                    if clear {
                        solid.remove(&cell);
                    } else {
                        solid.insert(cell);
                    }
                }
            }
        }
        // Gate thresholds are passable (see module docs).
        for resolved in plan.anchors.values() {
            if let ResolvedAnchor::Gate { from, to, .. } = resolved {
                for cell in region_cells(*from, *to) {
                    solid.remove(&cell);
                }
            }
        }
        World { solid }
    }

    /// Build the occupancy model exactly like [`World::from_plan`], then add
    /// `extra_solid` cells (the relight pass's colliding fixtures — campfire /
    /// floor lantern — so post-relight nav verification sees them; spec-0010). A
    /// fixture that adds no collision (torch, wall/hanging fixtures, embedded
    /// shroomlight) contributes nothing here.
    pub fn from_plan_with_extra(
        plan: &Plan,
        structures: &BTreeMap<String, Vec<u8>>,
        extra_solid: &BTreeSet<[i32; 3]>,
    ) -> Self {
        let mut world = Self::from_plan(plan, structures);
        world.solid.extend(extra_solid.iter().copied());
        world
    }

    /// Whether a cell is occupied by a solid block in the assembled world.
    pub fn solid_at(&self, c: [i32; 3]) -> bool {
        self.is_solid(c)
    }

    /// Build a [`World`] directly from a set of solid cells (test / synthetic
    /// entry point; the relight unit tests build a world without a full [`Plan`]).
    pub fn from_solid_cells(solid: BTreeSet<[i32; 3]>) -> Self {
        World { solid }
    }

    /// Whether a cell is a valid standing position (feet + head passable, solid
    /// ground below). Public wrapper over the internal walkability rule so the
    /// relight pass (spec-0010) can collect reachable walkable cells.
    pub fn is_standable(&self, c: [i32; 3]) -> bool {
        self.standable(c)
    }

    /// The nearest standable cell to `c` within `radius` (itself if already
    /// standable), broken deterministically by `(distance², cell)`; `None` if
    /// none. Public wrapper over `snap_standable` for the relight pass.
    pub fn snap(&self, c: [i32; 3], radius: i32) -> Option<[i32; 3]> {
        self.snap_standable(c, radius)
    }

    /// Every standable cell reachable by a walk (one-block step up/down, cardinal)
    /// from any of `starts`, over the assembled geometry. Deterministic BFS over a
    /// `BTreeSet` frontier with fixed neighbour order (ADR-0006). Starts that are
    /// not themselves standable are snapped within [`SNAP_RADIUS`] first; an
    /// unsnappable start contributes nothing.
    pub fn reachable_walkable(&self, starts: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut queue: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
        for &s in starts {
            if let Some(cell) = self.snap(s, SNAP_RADIUS)
                && seen.insert(cell)
            {
                queue.push_back(cell);
            }
        }
        while let Some(cur) = queue.pop_front() {
            for n in self.neighbors(cur) {
                if seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        seen
    }

    /// The union of every cell on a critical-path walked leg (the A* paths
    /// [`check_critical_path`] validates) plus every `move-npc` waypoint cell — the
    /// "required nav path cells" the relight pass must never occupy or obstruct
    /// (spec-0010). Computed over the base assembled geometry (fixtures avoid these
    /// cells, so they never appear here).
    pub fn required_path_cells(&self, plan: &Plan, moves: &[MovePlan]) -> BTreeSet<[i32; 3]> {
        let mut cells: BTreeSet<[i32; 3]> = BTreeSet::new();
        // Critical-path legs.
        let positions = critical_positions(plan);
        for pair in positions.windows(2) {
            let (from, _) = pair[0];
            let (to, transport_before) = pair[1];
            if transport_before {
                continue;
            }
            let (Some(start), Some(goal)) =
                (self.snap(from, SNAP_RADIUS), self.snap(to, SNAP_RADIUS))
            else {
                continue;
            };
            if let Some(path) = self.find_path(start, goal) {
                cells.extend(path);
            }
        }
        // move-npc waypoint cells (floored).
        for m in moves {
            for w in &m.waypoints {
                cells.insert([
                    w[0].floor() as i32,
                    w[1].floor() as i32,
                    w[2].floor() as i32,
                ]);
            }
        }
        cells
    }

    /// The standable cells confined to the AABB `bounds`, reachable by a walk from
    /// `anchor` (snapped to the nearest standable cell inside `bounds`), returned in
    /// ascending BFS step-distance order from that start with a fixed `(y, z, x)`
    /// tie-break. Seats spawn-wave mobs on validated footing near their anchor
    /// (task #41): `bounds` is the anchor's own assembled piece, so the flood-fill
    /// never leaves that room even where a mated socket is open air — a wave can no
    /// longer string its mobs across a socket seam into the neighbouring piece (the
    /// field bug: six sheep spread +x across the den↔mouth seam toward void). Empty
    /// when no standable cell exists inside `bounds` within reach of the anchor.
    ///
    /// Deterministic (ADR-0006): BFS over a `VecDeque` with the fixed neighbour
    /// order, then a total sort on `(distance, y, z, x)`.
    pub fn confined_standable_cells(
        &self,
        anchor: [i32; 3],
        bounds: ([i32; 3], [i32; 3]),
    ) -> Vec<[i32; 3]> {
        let (lo, hi) = bounds;
        let in_bounds = |c: [i32; 3]| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]);
        // A wave anchor often marks a solid affordance (a totem, a marker block) the
        // mobs stand *around*, not inside: snap the start to the nearest standable
        // floor cell within the room before flooding.
        let Some(start) = self.snap_in_bounds(anchor, SNAP_RADIUS, &in_bounds) else {
            return Vec::new();
        };
        let mut dist: BTreeMap<[i32; 3], u32> = BTreeMap::new();
        let mut queue: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
        dist.insert(start, 0);
        queue.push_back(start);
        while let Some(cur) = queue.pop_front() {
            let d = dist[&cur] + 1;
            for n in self.neighbors(cur) {
                if in_bounds(n) && !dist.contains_key(&n) {
                    dist.insert(n, d);
                    queue.push_back(n);
                }
            }
        }
        let mut cells: Vec<[i32; 3]> = dist.keys().copied().collect();
        cells.sort_by_key(|c| (dist[c], c[1], c[2], c[0]));
        cells
    }

    /// Nearest standable cell to `c` within `radius` that also satisfies `accept`,
    /// broken deterministically by `(distance², cell)`; `None` if none. The
    /// `accept` predicate confines the search (e.g. to one piece's AABB).
    fn snap_in_bounds(
        &self,
        c: [i32; 3],
        radius: i32,
        accept: &impl Fn([i32; 3]) -> bool,
    ) -> Option<[i32; 3]> {
        let mut best: Option<(i32, [i32; 3])> = None;
        for dy in -radius..=radius {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                    if !accept(n) || !self.standable(n) {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy + dz * dz;
                    match best {
                        Some((bd, bc)) if (bd, bc) <= (d2, n) => {}
                        _ => best = Some((d2, n)),
                    }
                }
            }
        }
        best.map(|(_, n)| n)
    }

    fn is_solid(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c)
    }

    /// Whether a cell is a valid standing position: the feet-cell and the
    /// head-cell above it are both passable, with solid ground directly below (an
    /// entity is 2 blocks tall and needs a floor).
    fn standable(&self, c: [i32; 3]) -> bool {
        !self.is_solid(c)
            && !self.is_solid([c[0], c[1] + 1, c[2]])
            && self.is_solid([c[0], c[1] - 1, c[2]])
    }

    /// The nearest standable cell to `c` (itself if already standable), searched
    /// outward in a bounded box and broken deterministically by
    /// `(distance², cell)`. `None` if nothing standable is within `radius`.
    ///
    /// A `move-npc` target anchor is often a solid affordance — an altar, a gate
    /// bar row, a wall marker — that the NPC should walk *up to*, not *into*
    /// (owner's "lands inside a wall" finding). Snapping resolves the walk to the
    /// floor cell in front of such an anchor.
    fn snap_standable(&self, c: [i32; 3], radius: i32) -> Option<[i32; 3]> {
        if self.standable(c) {
            return Some(c);
        }
        let mut best: Option<(i32, [i32; 3])> = None;
        for dy in -radius..=radius {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                    if !self.standable(n) {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy + dz * dz;
                    match best {
                        Some((bd, bc)) if (bd, bc) <= (d2, n) => {}
                        _ => best = Some((d2, n)),
                    }
                }
            }
        }
        best.map(|(_, n)| n)
    }

    /// Standable cardinal neighbours of `c`, allowing a one-block step up or down
    /// (stairs). Fixed order for determinism.
    fn neighbors(&self, c: [i32; 3]) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let mut out = Vec::new();
        for (dx, dz) in HORIZ {
            for dy in [0i32, -1, 1] {
                let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                if self.standable(n) {
                    out.push(n);
                }
            }
        }
        out
    }

    /// A* over standable cells from `start` to `goal`, returning the cell path
    /// (inclusive of both ends) or `None` if unreachable. Deterministic: the
    /// frontier is ordered by `(f, g, cell)` and neighbours expand in a fixed
    /// order.
    fn find_path(&self, start: [i32; 3], goal: [i32; 3]) -> Option<Vec<[i32; 3]>> {
        if start == goal {
            return self.standable(start).then(|| vec![start]);
        }
        if !self.standable(start) || !self.standable(goal) {
            return None;
        }
        let h = |c: [i32; 3]| ((c[0] - goal[0]).abs() + (c[2] - goal[2]).abs()) as u32;
        let mut g_score: BTreeMap<[i32; 3], u32> = BTreeMap::new();
        let mut came_from: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
        let mut open: BinaryHeap<Reverse<(u32, u32, [i32; 3])>> = BinaryHeap::new();
        g_score.insert(start, 0);
        open.push(Reverse((h(start), 0, start)));
        while let Some(Reverse((_f, g, cur))) = open.pop() {
            if cur == goal {
                let mut path = vec![cur];
                let mut node = cur;
                while let Some(&prev) = came_from.get(&node) {
                    path.push(prev);
                    node = prev;
                }
                path.reverse();
                return Some(path);
            }
            // Skip stale heap entries (a cheaper route was already recorded).
            if g > *g_score.get(&cur).unwrap_or(&u32::MAX) {
                continue;
            }
            for n in self.neighbors(cur) {
                let tentative = g + 1;
                if tentative < *g_score.get(&n).unwrap_or(&u32::MAX) {
                    came_from.insert(n, cur);
                    g_score.insert(n, tentative);
                    open.push(Reverse((tentative + h(n), tentative, n)));
                }
            }
        }
        None
    }
}

/// Resample a cell path into per-tick waypoints at `speed` blocks/tick along the
/// polyline through the integer cell centres. Guarantees the final waypoint is
/// exactly the goal cell and at least one step exists.
fn resample(cells: &[[i32; 3]], speed: f64) -> Vec<[f64; 3]> {
    let pts: Vec<[f64; 3]> = cells
        .iter()
        .map(|c| [c[0] as f64, c[1] as f64, c[2] as f64])
        .collect();
    if pts.len() == 1 {
        return vec![pts[0]];
    }
    // Cumulative arc length at each vertex.
    let mut cum = vec![0.0f64];
    for w in pts.windows(2) {
        let d = ((w[1][0] - w[0][0]).powi(2)
            + (w[1][1] - w[0][1]).powi(2)
            + (w[1][2] - w[0][2]).powi(2))
        .sqrt();
        cum.push(cum.last().unwrap() + d);
    }
    let total = *cum.last().unwrap();
    let speed = if speed > 0.0 { speed } else { DEFAULT_SPEED };
    let ticks = ((total / speed).ceil() as i64).max(1) as usize;
    let mut out = Vec::with_capacity(ticks + 1);
    for t in 0..=ticks {
        let d = total * (t as f64) / (ticks as f64);
        let p = point_at(&pts, &cum, d);
        // Round to 0.01 block — far finer than needed at 0.15 blk/tick, and keeps
        // the emitted per-tick `tp` coordinates short and stable.
        out.push([round2(p[0]), round2(p[1]), round2(p[2])]);
    }
    *out.last_mut().unwrap() = *pts.last().unwrap();
    out
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// The point at arc length `d` along the polyline `pts` with cumulative lengths
/// `cum`.
fn point_at(pts: &[[f64; 3]], cum: &[f64], d: f64) -> [f64; 3] {
    let total = *cum.last().unwrap();
    if d <= 0.0 || total == 0.0 {
        return pts[0];
    }
    if d >= total {
        return *pts.last().unwrap();
    }
    // Find the segment containing `d`.
    let mut i = 0;
    while i + 1 < cum.len() && cum[i + 1] < d {
        i += 1;
    }
    let seg = cum[i + 1] - cum[i];
    let f = if seg > 0.0 { (d - cum[i]) / seg } else { 0.0 };
    let a = pts[i];
    let b = pts[i + 1];
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

/// The absolute world position of an NPC's home anchor (its spawn cell), which is
/// where a `move-npc` walk begins.
fn npc_start(plan: &Plan, npc_id: &str) -> Option<[i32; 3]> {
    let npc = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)?;
    let area = plan.npc_area(npc_id)?;
    plan.point(area, npc.anchor.as_str())
}

/// Resolve a `move-npc` destination: the anchor in the NPC's own area, else any
/// area (first match). Mirrors the emitter's `movenpc_target`.
fn move_target(plan: &Plan, npc_id: &str, to_anchor: &str) -> Option<[i32; 3]> {
    if let Some(area) = plan.npc_area(npc_id)
        && let Some(pos) = plan.point(area, to_anchor)
    {
        return Some(pos);
    }
    for ((_, name), resolved) in &plan.anchors {
        if name == to_anchor {
            return match resolved {
                ResolvedAnchor::Point { pos, .. } => Some(*pos),
                ResolvedAnchor::Gate { from, .. } => Some(*from),
            };
        }
    }
    None
}

/// Plan every `move-npc` in the campaign into a walked-path [`MovePlan`], deduped
/// by `(npc, to_anchor)` in first-seen order. `DW0307` when a move is unroutable.
pub fn plan_moves(plan: &Plan, world: &World) -> Result<Vec<MovePlan>, NavError> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for eff in all_effects(plan) {
        let QuestEffect::MoveNpc {
            npc,
            to_anchor,
            speed,
        } = eff
        else {
            continue;
        };
        let key = (npc.as_str().to_string(), to_anchor.as_str().to_string());
        if !seen.insert(key) {
            continue;
        }
        let start = npc_start(plan, npc.as_str()).ok_or_else(|| NavError {
            code: DW_MOVE_UNROUTABLE,
            message: format!(
                "move-npc: NPC `{}` has no resolved home anchor to walk from — give the npc a \
                 stage-2 `anchor` that its area's prefab provides, so the walk has a start",
                npc.as_str()
            ),
        })?;
        let anchor_pos =
            move_target(plan, npc.as_str(), to_anchor.as_str()).ok_or_else(|| NavError {
                code: DW_MOVE_UNROUTABLE,
                message: format!(
                    "move-npc: destination anchor `{}` for NPC `{}` did not resolve to a world \
                     position — use a `to_anchor` that the NPC's area prefab provides",
                    to_anchor.as_str(),
                    npc.as_str()
                ),
            })?;
        // The NPC walks up to a solid affordance, not into it: snap both endpoints
        // to the floor cell nearest the anchor.
        let start = world.snap_standable(start, SNAP_RADIUS).unwrap_or(start);
        let target = world
            .snap_standable(anchor_pos, SNAP_RADIUS)
            .ok_or_else(|| NavError {
                code: DW_MOVE_UNROUTABLE,
                message: format!(
                    "move-npc: no standable floor cell near destination anchor `{}` {anchor_pos:?} \
                 for NPC `{}` — the anchor is walled in or over void; place `{}` beside walkable \
                 floor the npc can stand on",
                    to_anchor.as_str(),
                    npc.as_str(),
                    to_anchor.as_str(),
                ),
            })?;
        let cells = world.find_path(start, target).ok_or_else(|| NavError {
            code: DW_MOVE_UNROUTABLE,
            message: format!(
                "move-npc: NPC `{}` cannot walk from `{}` {start:?} to `{}` {anchor_pos:?} (floor \
                 {target:?}) — no collision-free path over the solved geometry. Route the move \
                 within one connected area (a wall/void/closed gate separates start and \
                 destination), or split it into shorter reachable hops",
                npc.as_str(),
                plan_npc_anchor(plan, npc.as_str()),
                to_anchor.as_str(),
            ),
        })?;
        out.push(MovePlan {
            npc: npc.as_str().to_string(),
            to_anchor: to_anchor.as_str().to_string(),
            target,
            waypoints: resample(&cells, speed.unwrap_or(DEFAULT_SPEED)),
        });
    }
    Ok(out)
}

/// The home-anchor id of an NPC, for diagnostics (or `?` if unknown).
fn plan_npc_anchor(plan: &Plan, npc_id: &str) -> String {
    plan.campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map(|n| n.anchor.as_str().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// The camera dolly world points of a cutscene (anchor + offset, block centres) —
/// the exact points the emitter lerps between. Shared with the emitter so the
/// air-corridor check validates what actually ships.
pub fn camera_points(plan: &Plan, path: &[CameraWaypoint]) -> Vec<[f64; 3]> {
    path.iter()
        .map(|w| {
            let base = plan
                .anchors
                .iter()
                .find(|((_, name), _)| name == w.anchor.as_str())
                .map(|(_, r)| match r {
                    ResolvedAnchor::Point { pos, .. } => *pos,
                    ResolvedAnchor::Gate { from, .. } => *from,
                })
                .unwrap_or([0, crate::plan::BASE_Y, 0]);
            [
                (base[0] + w.offset[0]) as f64 + 0.5,
                (base[1] + w.offset[1]) as f64 + 0.5,
                (base[2] + w.offset[2]) as f64 + 0.5,
            ]
        })
        .collect()
}

/// Validate every cutscene camera dolly path passes only through non-solid blocks
/// (cameras fly but must not clip a solid). `DW0308` names the offending segment
/// and the block coordinate that clips.
pub fn check_cutscenes(plan: &Plan, world: &World) -> Result<(), NavError> {
    for eff in all_effects(plan) {
        let QuestEffect::Cutscene { path, .. } = eff else {
            continue;
        };
        let pts = camera_points(plan, path.as_slice());
        if let Some((seg, cell)) = first_clip(world, &pts) {
            return Err(NavError {
                code: DW_CUTSCENE_CLIP,
                message: format!(
                    "cutscene: camera dolly segment {seg} (from {:?} to {:?}) clips a solid block \
                     at {cell:?} — cameras must fly through open air; move the segment's \
                     waypoint `anchor`/`offset` so the whole path clears solid blocks",
                    round3(pts[seg]),
                    round3(pts[seg + 1]),
                ),
            });
        }
    }
    Ok(())
}

/// The first `(segment index, block cell)` where a camera dolly polyline passes
/// through a solid block, or `None` if the whole path is air. Each segment is
/// sampled finely (≤ 0.25 blocks) so no unit cell is stepped over.
fn first_clip(world: &World, pts: &[[f64; 3]]) -> Option<(usize, [i32; 3])> {
    for (seg, w) in pts.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
        let steps = ((len / 0.25).ceil() as i64).max(1);
        for s in 0..=steps {
            let f = s as f64 / steps as f64;
            let cell = [
                (a[0] + (b[0] - a[0]) * f).floor() as i32,
                (a[1] + (b[1] - a[1]) * f).floor() as i32,
                (a[2] + (b[2] - a[2]) * f).floor() as i32,
            ];
            if world.is_solid(cell) {
                return Some((seg, cell));
            }
        }
    }
    None
}

fn round3(p: [f64; 3]) -> [f64; 3] {
    [
        (p[0] * 1000.0).round() / 1000.0,
        (p[1] * 1000.0).round() / 1000.0,
        (p[2] * 1000.0).round() / 1000.0,
    ]
}

/// Every quest effect in the campaign (objective-complete, quest-complete, and
/// trigger effects), matching the emitter's `all_campaign_effects` traversal.
fn all_effects<'a>(plan: &'a Plan) -> Vec<&'a QuestEffect> {
    let mut out = Vec::new();
    for q in &plan.campaign.quests.content.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            out.push(e);
        }
    }
    for t in &plan.campaign.quests.content.triggers {
        for e in &t.effects {
            out.push(e);
        }
    }
    out
}

/// Whether the campaign uses any verb that needs the voxel `World` (`move-npc` or
/// `cutscene`). When false, the emitter skips building the occupancy model, so
/// v0.2/v0.3 output is untouched.
pub fn needs_world(plan: &Plan) -> bool {
    all_effects(plan).iter().any(|e| {
        matches!(
            e,
            QuestEffect::MoveNpc { .. } | QuestEffect::Cutscene { .. }
        )
    })
    // The critical-path walkability check (DW0311) also needs the occupancy model.
        || has_walkable_critical_leg(plan)
}

/// The player-visited critical-path positions in order, each tagged with whether
/// the player was teleported here by an inter-area transport on the preceding
/// step (a ride, not a walk). `select-class` / `assert-complete` steps carry no
/// position and are skipped.
fn critical_positions(plan: &Plan) -> Vec<([i32; 3], bool)> {
    let mut out = Vec::new();
    let mut transport_pending = false;
    for (i, step) in plan.critical_path.iter().enumerate() {
        let pos = match step {
            Step::TalkTo { pos, .. }
            | Step::Reach { pos, .. }
            | Step::Kill { pos, .. }
            | Step::Collect { pos, .. }
            | Step::Interact { pos, .. } => Some(*pos),
            Step::SelectClass { .. } | Step::AssertComplete { .. } => None,
        };
        if let Some(pos) = pos {
            out.push((pos, transport_pending));
            transport_pending = false;
        }
        // A transport marker on step `i` teleports the player when that step's
        // objective completes — i.e. before the *next* visited position is reached,
        // so the move INTO that next position is a ride, not a walk to validate.
        if plan
            .critical_path_transport
            .get(i)
            .and_then(|t| *t)
            .is_some()
        {
            transport_pending = true;
        }
    }
    out
}

/// Whether the campaign has at least one consecutive pair of player-visited
/// critical-path positions with no inter-area transport between them — a leg the
/// player must walk, hence one DW0311 must validate.
fn has_walkable_critical_leg(plan: &Plan) -> bool {
    critical_positions(plan).windows(2).any(|w| !w[1].1)
}

/// Validate that every consecutive pair of player-visited critical-path anchors is
/// connected by a walkable A* path over the assembled geometry (unless the player
/// rides an inter-area transport between them). This is the compile-time counterpart
/// to the runtime critical-path bot: it makes an unwalkable assembled seam — a
/// prefab whose regenerated geometry wedged a doorway shut or opened a void gap — a
/// build failure ([`DW_CRITICAL_UNROUTABLE`], `DW0311`) instead of a bot surprise.
///
/// Endpoints are snapped to the nearest standable floor cell (an anchor often marks
/// a solid affordance — an altar, a wave marker, an NPC stand — the player walks up
/// to, not into), exactly as `move-npc` planning does.
pub fn check_critical_path(plan: &Plan, world: &World) -> Result<(), NavError> {
    route_visited(world, &critical_positions(plan))
}

/// Route every walked leg between consecutive visited positions (the pure core of
/// [`check_critical_path`], split out so it is unit-testable without a full
/// [`Plan`]). `positions[i].1` is the transport-before flag: `true` legs are
/// teleport rides and skipped.
fn route_visited(world: &World, positions: &[([i32; 3], bool)]) -> Result<(), NavError> {
    for pair in positions.windows(2) {
        let (from, _) = pair[0];
        let (to, transport_before) = pair[1];
        if transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let start = world.snap_standable(from, SNAP_RADIUS).ok_or_else(|| NavError {
            code: DW_CRITICAL_UNROUTABLE,
            message: format!(
                "critical path: no standable floor within {SNAP_RADIUS} blocks of visited anchor \
                 {from:?} — a player-visited anchor sits walled in or over void. Fix the prefab \
                 so this anchor sits on/next to reachable floor; if the prefab looks correct, this \
                 is an assembly/toolchain defect — escalate rather than move the anchor into a \
                 wall"
            ),
        })?;
        let goal = world.snap_standable(to, SNAP_RADIUS).ok_or_else(|| NavError {
            code: DW_CRITICAL_UNROUTABLE,
            message: format!(
                "critical path: no standable floor within {SNAP_RADIUS} blocks of visited anchor \
                 {to:?} — a player-visited anchor sits walled in or over void. Fix the prefab so \
                 this anchor sits on/next to reachable floor; if the prefab looks correct, this is \
                 an assembly/toolchain defect — escalate rather than move the anchor into a wall"
            ),
        })?;
        if world.find_path(start, goal).is_none() {
            return Err(NavError {
                code: DW_CRITICAL_UNROUTABLE,
                message: format!(
                    "critical path: the player cannot walk from {from:?} (floor {start:?}) to {to:?} \
                     (floor {goal:?}) over the assembled geometry — no collision-free path. A same-area \
                     leg must be walkable end to end; this is a wedged doorway seam or a void gap in the \
                     assembled layout (or, if the jump is intended, a missing inter-area transport)."
                ),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat solid floor at `y-1` over `[0,w) × [0,d)`, with the given interior
    /// cells at `y` set solid (obstacles). Cells at `y` not listed are open air.
    fn floored(w: i32, d: i32, y: i32, walls: &[[i32; 3]]) -> World {
        let mut solid = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                solid.insert([x, y - 1, z]); // floor
                solid.insert([x, y + 2, z]); // ceiling (headroom = y, y+1)
            }
        }
        for &c in walls {
            solid.insert(c);
        }
        World { solid }
    }

    #[test]
    fn path_routes_around_a_wall_corner() {
        // A wall spanning z=0..2 at x=2 forces a detour around its open end at z=2.
        let world = floored(5, 4, 65, &[[2, 65, 0], [2, 65, 1], [2, 65, 2]]);
        let path = world.find_path([0, 65, 0], [4, 65, 0]).expect("routable");
        assert_eq!(path.first(), Some(&[0, 65, 0]));
        assert_eq!(path.last(), Some(&[4, 65, 0]));
        // The detour must have turned a corner: it cannot be a straight x-line.
        assert!(
            path.iter().any(|c| c[2] >= 3),
            "path must round the wall's open end, got {path:?}"
        );
        // No waypoint sits inside the wall.
        for c in &path {
            assert!(!world.is_solid(*c), "path clips wall at {c:?}");
        }
    }

    #[test]
    fn disconnected_floors_are_unroutable() {
        // Two floor patches with a void gap (no floor at x=2) → DW0307 condition.
        let mut solid = BTreeSet::new();
        for x in [0, 1, 3, 4] {
            for z in 0..3 {
                solid.insert([x, 64, z]);
                solid.insert([x, 67, z]);
            }
        }
        let world = World { solid };
        assert!(world.standable([0, 65, 1]));
        assert!(world.standable([4, 65, 1]));
        assert!(world.find_path([0, 65, 1], [4, 65, 1]).is_none());
    }

    #[test]
    fn snap_finds_floor_in_front_of_a_solid_affordance() {
        // A solid altar block at the target; the nearest standable cell is beside
        // it (the NPC walks up to it, not into it).
        let world = floored(5, 5, 65, &[[2, 65, 2]]);
        assert!(world.snap_standable([2, 65, 2], 0).is_none());
        let snapped = world.snap_standable([2, 65, 2], 2).expect("floor nearby");
        assert!(world.standable(snapped));
        assert!((snapped[0] - 2).abs() + (snapped[2] - 2).abs() <= 1);
    }

    #[test]
    fn snap_none_when_fully_embedded() {
        // A solid cell walled in by solids within the radius → no floor to snap to.
        let mut solid = BTreeSet::new();
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    solid.insert([10 + dx, 65 + dy, 10 + dz]);
                }
            }
        }
        let world = World { solid };
        assert!(world.snap_standable([10, 65, 10], 2).is_none());
    }

    #[test]
    fn cutscene_clip_detects_a_solid_on_the_dolly_and_passes_clean_air() {
        // A solid pillar at [2,66,1]; a dolly through it clips, one beside it does
        // not.
        let world = floored(5, 4, 65, &[[2, 66, 1]]);
        let through = [[0.5, 66.5, 1.5], [4.5, 66.5, 1.5]];
        assert_eq!(first_clip(&world, &through), Some((0, [2, 66, 1])));
        let clear = [[0.5, 66.5, 3.5], [4.5, 66.5, 3.5]];
        assert_eq!(first_clip(&world, &clear), None);
    }

    #[test]
    fn critical_path_unroutable_leg_is_dw0311() {
        // Two standable floor patches separated by a void gap (no floor at x=2):
        // a walked leg across them is DW0311; the same leg guarded by a transport
        // hop (transport_before = true) is skipped.
        let mut solid = BTreeSet::new();
        for x in [0, 1, 3, 4] {
            for z in 0..3 {
                solid.insert([x, 64, z]);
                solid.insert([x, 67, z]);
            }
        }
        let world = World { solid };
        let a = [0, 65, 1];
        let b = [4, 65, 1];
        assert!(world.standable(a) && world.standable(b));
        // Walked leg → unroutable → DW0311.
        let err = route_visited(&world, &[(a, false), (b, false)]).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        // Same leg ridden by an inter-area transport → skipped, ok.
        assert!(route_visited(&world, &[(a, false), (b, true)]).is_ok());
    }

    #[test]
    fn critical_path_routable_leg_passes() {
        // A flat connected floor: consecutive visited cells are walkable → ok.
        let world = floored(6, 3, 65, &[]);
        assert!(route_visited(&world, &[([0, 65, 1], false), ([5, 65, 1], false)]).is_ok());
    }

    #[test]
    fn confined_cells_are_standable_distinct_and_ordered_by_distance() {
        // A 5×5 floored room. Placement floods standable cells from the anchor.
        let world = floored(5, 5, 65, &[]);
        let bounds = ([0, 64, 0], [4, 66, 4]);
        let cells = world.confined_standable_cells([2, 65, 2], bounds);
        // Every returned cell is standable, and all are distinct.
        for c in &cells {
            assert!(world.standable(*c), "non-standable cell {c:?}");
        }
        let uniq: BTreeSet<_> = cells.iter().copied().collect();
        assert_eq!(uniq.len(), cells.len(), "duplicate spawn cell");
        // The anchor's own snapped start comes first (distance 0), then its
        // cardinal neighbours (distance 1) before any distance-2 cell.
        assert_eq!(cells[0], [2, 65, 2]);
        // Non-increasing BFS distance is enforced by construction; spot-check that a
        // near cell precedes a far corner.
        let idx = |t: [i32; 3]| cells.iter().position(|c| *c == t).unwrap();
        assert!(idx([2, 65, 3]) < idx([0, 65, 0]));
    }

    #[test]
    fn confined_cells_never_cross_a_socket_seam() {
        // Two 3-wide rooms sharing an open (air) seam at x=3 — as a mated jigsaw
        // socket would be. Confining to the left room's bounds must keep every
        // placement cell at x<=2, never flooding through the open seam into the
        // right room (the den↔mouth spill this fix prevents).
        let mut solid = BTreeSet::new();
        for x in 0..=6 {
            for z in 0..3 {
                solid.insert([x, 64, z]); // continuous floor across both rooms
                solid.insert([x, 67, z]); // ceiling
            }
        }
        let world = World { solid };
        let left_bounds = ([0, 64, 0], [2, 66, 2]);
        let cells = world.confined_standable_cells([1, 65, 1], left_bounds);
        assert!(!cells.is_empty());
        for c in &cells {
            assert!(
                c[0] <= 2,
                "placement {c:?} crossed the seam into the right room"
            );
        }
        // Sanity: the floor genuinely connects across the seam (an unconfined flood
        // would reach the right room), so confinement — not a wall — is what holds.
        assert!(world.find_path([1, 65, 1], [5, 65, 1]).is_some());
    }

    #[test]
    fn confined_cells_deterministic_across_runs() {
        let world = floored(6, 4, 65, &[[3, 65, 1]]);
        let bounds = ([0, 64, 0], [5, 66, 3]);
        let a = world.confined_standable_cells([1, 65, 1], bounds);
        let b = world.confined_standable_cells([1, 65, 1], bounds);
        assert_eq!(a, b);
    }

    #[test]
    fn resample_honors_speed_and_lands_exactly_on_target() {
        let cells = [[0, 65, 0], [10, 65, 0]];
        let slow = resample(&cells, 0.15);
        let fast = resample(&cells, 1.0);
        // Slower speed → more per-tick waypoints for the same distance.
        assert!(slow.len() > fast.len());
        // The final waypoint is exactly the integer target cell.
        assert_eq!(*slow.last().unwrap(), [10.0, 65.0, 0.0]);
        assert_eq!(slow[0], [0.0, 65.0, 0.0]);
    }
}
