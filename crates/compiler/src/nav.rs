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

use delvewright_dsl::{CameraWaypoint, Lethality, QuestEffect, TrapReset};

use crate::plan::{GateEvent, Plan, ResolvedAnchor, Step, TrapPlan};

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
/// `DW0315`: a `set-checkpoint` (spec-0012) that would strand the party — from the
/// checkpoint cell, a remaining required critical-path anchor is no longer
/// walkable (a checkpoint behind a one-way drop). Re-roots the DW0311 reachability
/// at the checkpoint.
pub const DW_CHECKPOINT_STRANDED: &str = "DW0315";
/// `DW0316`: a `set-checkpoint` anchor with no standable footing on the final
/// assembled model (a trap-trigger / hazard / mid-air cell), so the party would
/// respawn into the void or a wall.
pub const DW_CHECKPOINT_UNSTANDABLE: &str = "DW0316";
/// `DW0327`: a `begin-stealth` (spec-0014) zone that is unstandable, or unreachable
/// from the player's position at the beat that activates the stealth check.
pub const DW_STEALTH_ZONE: &str = "DW0327";
/// `DW0342`: a **lethal** trap (spec-0011) whose trigger cell lies on the forced
/// critical path with no discharge — not avoidable (the trigger cell is a required
/// path cell), not survivable (`rearm`, so a respawn walk-back re-triggers it →
/// soft-loop), and not disarmable (no disarm affordance reachable before it). The
/// player is provably killed or soft-looped. Analysis-tier (exit 2) like `DW0312`:
/// a content-design mistake, not a geometry defect. (Renumbered from the spec's
/// stale `DW0314`.)
pub const DW_TRAP_LETHAL_UNAVOIDABLE: &str = "DW0342";

/// A resolved stealth zone `(anchor name, centre cell, half-extents)`.
type ZoneCell = (String, [i32; 3], [u32; 3]);
/// A stealth beat probe for [`verify_stealth`]: `(zones, firing step)`.
type StealthProbe = (Vec<ZoneCell>, usize);

/// `DW0325`: a `move-actor` destination unreachable by the actor's footprint over
/// the assembled geometry, or an actor spawn/destination anchor that does not
/// resolve to a placeable cell (spec-0014). Names the actor, the leg, and the
/// first blocked cell.
pub const DW_ACTOR_UNROUTABLE: &str = "DW0325";

/// Default NPC walking speed in blocks/tick (spec-0008 §5; owner spike). Used when
/// a `move-npc` effect omits `speed`.
pub const DEFAULT_SPEED: f64 = 0.15;

/// An entity's collision footprint over the voxel grid: the set of column offsets
/// it occupies horizontally and the number of vertical cells it needs clear
/// (`ceil(height)`). Standing feet-centred on a cell, an entity of `width <= 1`
/// occupies a single column; a taller entity needs more headroom (the warden, 2.9
/// tall, needs 3 cells vs a player's 2 — so it cannot walk a 2-high gap a player
/// fits). Drives footprint-aware standability + A* so a `move-actor` path is
/// walkable for the ACTUAL puppet, not a generic 1×2 humanoid (spec-0014, task #46).
#[derive(Debug, Clone)]
pub struct Footprint {
    /// Horizontal column offsets `[dx, dz]` the body occupies (feet cell = `[0, 0]`).
    cols: Vec<[i32; 2]>,
    /// Vertical cells of clearance the body needs (`ceil(height)`, min 1).
    height: i32,
}

impl Footprint {
    /// The footprint for the given hitbox `width` × `height` in blocks. Feet-centred
    /// on a cell: columns are the unit cells the width-wide AABB overlaps; height is
    /// `ceil(height)` (min 1).
    pub fn for_dims(width: f64, height: f64) -> Footprint {
        let half = width / 2.0;
        let lo = (0.5 - half).floor() as i32;
        let hi = (0.5 + half - 1e-9).floor() as i32;
        let mut cols = Vec::new();
        for dx in lo..=hi {
            for dz in lo..=hi {
                cols.push([dx, dz]);
            }
        }
        if cols.is_empty() {
            cols.push([0, 0]);
        }
        let h = (height.ceil() as i32).max(1);
        Footprint { cols, height: h }
    }

    /// The default humanoid footprint (player / villager / mannequin: 0.6 × 1.8 →
    /// single column, 2 cells tall). Byte-identical to the pre-spec-0014 walkability
    /// model, so `move-npc` and critical-path routing are unchanged.
    pub fn player() -> Footprint {
        Footprint::for_dims(0.6, 1.8)
    }
}

/// The hitbox footprint for a vanilla entity id (spec-0014 per-entity dims table).
/// Standing hitboxes for the 1.21.11 mobs an actor is likely to puppet; anything
/// unlisted falls back to the humanoid default (0.6 × 1.95). Width only matters
/// past 1.0 (sub-block mobs are single-column); height gates vertical clearance.
pub fn entity_footprint(entity: &str) -> Footprint {
    let (w, h) = match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        "warden" => (0.9, 2.9),
        "iron_golem" => (1.4, 2.7),
        "ravager" => (1.95, 2.2),
        "hoglin" | "zoglin" => (1.4, 1.4),
        "sheep" | "goat" | "pig" | "cow" | "mooshroom" | "wolf" | "fox" | "panda" => (0.9, 1.4),
        "villager" | "zombie" | "husk" | "zombie_villager" => (0.6, 1.95),
        "skeleton" | "stray" | "wither_skeleton" => (0.6, 1.99),
        "creeper" | "enderman" => (0.6, 1.9),
        "allay" | "vex" => (0.35, 0.6),
        "armor_stand" | "player" | "mannequin" => (0.6, 1.8),
        _ => (0.6, 1.95),
    };
    Footprint::for_dims(w, h)
}

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

/// A collision/standability model of the assembled world (spec-0008 addendum),
/// derived from the shared gravity-settled assembled-world model
/// ([`crate::assembled`]): every placed prefab block, plus the solver's socket
/// seals, with gate thresholds cleared and unsupported falling blocks settled
/// (task #42). Cells absent from both sets are passable (interior air, opened
/// sockets, gate thresholds, and any cell a gravity block fell out of).
///
/// Water is modelled separately from solids (task #45): `flooded` holds every cell
/// a conservative superset of vanilla water flow reaches (see
/// [`crate::assembled::assembled_occupancy`]). A flooded cell is **impassable** (a
/// walker cannot stand or pass through it) yet is **not solid floor** (you cannot
/// stand *on* a water surface) — the two sets are disjoint and both gate
/// standability, so nav / wave seating / relight / waypoint export never treat a
/// flooded cell as walkable ground.
pub struct World {
    solid: BTreeSet<[i32; 3]>,
    flooded: BTreeSet<[i32; 3]>,
}

impl World {
    /// Build the occupancy model from the plan's placed pieces and the structure
    /// `.nbt` bytes, via the shared assembled-world model. Every non-air cell of
    /// that settled map is a solid cell here — so a `sand`/`gravel` floor that
    /// falls out of the void world is passable (a hole), exactly as in game
    /// (task #42), not a phantom floor the model wrongly seats mobs on.
    pub fn from_plan(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Self {
        let (solid, flooded) = crate::assembled::assembled_occupancy(plan, structures);
        World { solid, flooded }
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

    /// A copy of this world with `extra` cells forced solid — a `close-gate`'s
    /// sealed region for the completability proof (DSL v0.6). The base occupancy
    /// model treats every gate cell as passable; sealing a gate for the legs that
    /// occur after it closes makes a path that must re-cross it fail routing.
    fn with_sealed(&self, extra: &BTreeSet<[i32; 3]>) -> World {
        let mut solid = self.solid.clone();
        solid.extend(extra.iter().copied());
        World {
            solid,
            flooded: self.flooded.clone(),
        }
    }

    /// Build a [`World`] directly from a set of solid cells, with no water (test /
    /// synthetic entry point; the relight unit tests build a world without a full
    /// [`Plan`]).
    pub fn from_solid_cells(solid: BTreeSet<[i32; 3]>) -> Self {
        World {
            solid,
            flooded: BTreeSet::new(),
        }
    }

    /// Build a [`World`] directly from disjoint solid + flooded cell sets (test /
    /// synthetic entry point for the flood-aware standability rules).
    pub fn from_solid_and_flooded(solid: BTreeSet<[i32; 3]>, flooded: BTreeSet<[i32; 3]>) -> Self {
        World { solid, flooded }
    }

    /// Whether a cell is a valid standing position (feet + head passable, solid
    /// ground below). Public wrapper over the internal walkability rule so the
    /// relight pass (spec-0010) can collect reachable walkable cells.
    pub fn is_standable(&self, c: [i32; 3]) -> bool {
        self.standable(c)
    }

    /// Whether a cell is unoccupied — neither a solid block nor water-flooded, so a
    /// camera eye placed in it sees open air rather than the inside of a block.
    /// Public wrapper for the visual-tier POV camera self-check ([`verify_pov_cameras`]).
    pub fn is_clear(&self, c: [i32; 3]) -> bool {
        !self.is_occupied(c)
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
            let from = pair[0].pos;
            let to = pair[1].pos;
            if pair[1].transport_before {
                continue;
            }
            let (Some(start), Some(goal)) = (
                self.snap_endpoint(from, false),
                self.snap_endpoint(to, pair[1].talk_to),
            ) else {
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

    /// Snap a walked-leg endpoint (`from`/`to`) to the cell the player stands on.
    ///
    /// Normally the nearest standable cell to the visited anchor. For a **talk-to**
    /// target (`off_cell`), the anchor is the NPC's own occupied cell (the mannequin
    /// stands there and its interaction hitbox fills it): the player stands within
    /// interaction range *beside* the NPC, so exclude the anchor cell itself and
    /// take the nearest OTHER standable cell (task #45). Flooded cells are already
    /// excluded (they are not standable), so a shore NPC never resolves onto a
    /// water-tongue cell.
    fn snap_endpoint(&self, c: [i32; 3], off_cell: bool) -> Option<[i32; 3]> {
        if off_cell {
            self.snap_in_bounds(c, SNAP_RADIUS, &|n| n != c)
        } else {
            self.snap_standable(c, SNAP_RADIUS)
        }
    }

    fn is_solid(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c)
    }

    /// Whether a cell is occupied — a solid block **or** flooded by water (task
    /// #45). An occupied cell cannot hold a walker's feet or head, and cannot be
    /// jumped through. Water blocks passage but, unlike a solid, is never a floor.
    fn is_occupied(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c) || self.flooded.contains(&c)
    }

    /// Whether a cell is a valid standing position: the feet-cell and the
    /// head-cell above it are both passable (neither solid nor flooded), with
    /// **solid** ground directly below (an entity is 2 blocks tall and needs a
    /// floor — a water surface is not standable, so the floor must be solid, not
    /// merely occupied; task #45).
    fn standable(&self, c: [i32; 3]) -> bool {
        self.standable_fp(c, &Footprint::player())
    }

    /// Footprint-aware standability (spec-0014): every occupied column has its
    /// `height` feet+body cells passable with solid floor directly below. For the
    /// player footprint (single column, 2 tall) this is exactly the pre-0.6 rule.
    fn standable_fp(&self, c: [i32; 3], fp: &Footprint) -> bool {
        fp.cols.iter().all(|&[dx, dz]| {
            let base = [c[0] + dx, c[1], c[2] + dz];
            self.is_solid([base[0], base[1] - 1, base[2]])
                && (0..fp.height).all(|dy| !self.is_occupied([base[0], base[1] + dy, base[2]]))
        })
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
        self.snap_standable_fp(c, radius, &Footprint::player())
    }

    /// Footprint-aware nearest-standable snap (spec-0014), used by `move-actor`
    /// endpoint resolution so a wide/tall puppet snaps to a cell IT can stand on.
    fn snap_standable_fp(&self, c: [i32; 3], radius: i32, fp: &Footprint) -> Option<[i32; 3]> {
        if self.standable_fp(c, fp) {
            return Some(c);
        }
        let mut best: Option<(i32, [i32; 3])> = None;
        for dy in -radius..=radius {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                    if !self.standable_fp(n, fp) {
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
    ///
    /// A one-block step **up** is a jump: the entity's head sweeps through the cell
    /// two above its feet at the source, so that cell must be clear or it head-bonks
    /// and the move is physically impossible (a mineflayer bot refuses it with
    /// "No path to the goal!"). Modelling that jump-clearance here — not just the
    /// destination's standability — keeps a routed/exported path actually walkable:
    /// an assembled seam that ramps up under a low ceiling becomes a `DW0311` build
    /// error instead of a runtime strand on geometry the compiler wrongly "proved"
    /// connected (task #38). Steps level or down need no such clearance.
    fn neighbors(&self, c: [i32; 3]) -> Vec<[i32; 3]> {
        self.neighbors_fp(c, &Footprint::player())
    }

    /// Footprint-aware standable neighbours (spec-0014). A step **up** is a jump:
    /// every occupied column's cell `height` above the feet (the swept head cell)
    /// must be clear, generalising the player's single `c+2` head-bonk check.
    fn neighbors_fp(&self, c: [i32; 3], fp: &Footprint) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let head_clear_to_jump = fp
            .cols
            .iter()
            .all(|&[dx, dz]| !self.is_occupied([c[0] + dx, c[1] + fp.height, c[2] + dz]));
        let mut out = Vec::new();
        for (dx, dz) in HORIZ {
            for dy in [0i32, -1, 1] {
                if dy == 1 && !head_clear_to_jump {
                    continue; // no room to jump up from here
                }
                let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                if self.standable_fp(n, fp) {
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
        self.find_path_fp(start, goal, &Footprint::player())
    }

    /// Footprint-aware A* (spec-0014). Identical to the pre-0.6 A* for the player
    /// footprint (so `move-npc` and critical-path routing stay byte-identical); a
    /// wider/taller footprint prunes cells the puppet cannot occupy. Deterministic:
    /// frontier ordered by `(f, g, cell)`, fixed neighbour order.
    fn find_path_fp(
        &self,
        start: [i32; 3],
        goal: [i32; 3],
        fp: &Footprint,
    ) -> Option<Vec<[i32; 3]>> {
        if start == goal {
            return self.standable_fp(start, fp).then(|| vec![start]);
        }
        if !self.standable_fp(start, fp) || !self.standable_fp(goal, fp) {
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
            for n in self.neighbors_fp(cur, fp) {
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
            ..
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

/// A planned `move-actor` (spec-0014): resolved endpoints, the per-tick waypoint
/// polyline the emitter teleports the puppet along, and a yaw per waypoint tangent
/// to the path (a wrong yaw moonwalks — task #46). `ticks() + 1` entries.
#[derive(Debug, Clone)]
pub struct ActorMovePlan {
    /// The moving actor id (`actor/…`).
    pub actor: String,
    /// The destination anchor id (`anchor/…`).
    pub to_anchor: String,
    /// The integer target cell (feet), for the arrival assertion.
    pub target: [i32; 3],
    /// Per-tick world positions along the walked path.
    pub waypoints: Vec<[f64; 3]>,
    /// Per-waypoint yaw (degrees), tangent to the path (facing the next step).
    pub yaws: Vec<i32>,
}

impl ActorMovePlan {
    /// The final tick index (`waypoints.len() - 1`).
    pub fn ticks(&self) -> usize {
        self.waypoints.len().saturating_sub(1)
    }
}

/// The stage-5 actor with this id, if declared.
fn actor_of<'a>(plan: &'a Plan, actor_id: &str) -> Option<&'a delvewright_dsl::Actor> {
    plan.campaign
        .quests
        .content
        .actors
        .iter()
        .find(|a| a.id.as_str() == actor_id)
}

/// Resolve an anchor name to a world point by scanning every area (first match) —
/// actors carry no area, so their anchors resolve globally like `open-gate`.
fn actor_anchor_pos(plan: &Plan, anchor: &str) -> Option<[i32; 3]> {
    for ((_, name), resolved) in &plan.anchors {
        if name == anchor {
            return Some(match resolved {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            });
        }
    }
    None
}

/// The MC yaw (degrees, 0 = +z/south) for a horizontal movement delta, or `None`
/// for no horizontal motion. `yaw = atan2(-dx, dz)`.
fn yaw_of(dx: f64, dz: f64) -> Option<i32> {
    if dx.abs() < 1e-6 && dz.abs() < 1e-6 {
        return None;
    }
    let deg = (-dx).atan2(dz).to_degrees();
    let mut y = deg.round() as i32 % 360;
    if y < 0 {
        y += 360;
    }
    Some(y)
}

/// A yaw per waypoint, each tangent to the path (facing the next distinct
/// waypoint); the last reuses the previous. A puppet tp'd without a matching yaw
/// moonwalks (task #46 packet evidence).
fn yaws_along(waypoints: &[[f64; 3]]) -> Vec<i32> {
    let n = waypoints.len();
    let mut yaws = vec![0i32; n];
    // Forward pass: each waypoint faces its NEXT step; the final waypoint reuses the
    // last motion direction (so arrival keeps the walk facing, not a snap to south).
    let mut last = 0i32;
    for i in 0..n {
        if i + 1 < n {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            if let Some(y) = yaw_of(b[0] - a[0], b[2] - a[2]) {
                last = y;
            }
        }
        yaws[i] = last;
    }
    yaws
}

/// The first cell along the straight start→target line the actor's footprint cannot
/// stand on — a best-effort "first blocked cell" for the `DW0325` message.
fn first_blocked_fp(world: &World, start: [i32; 3], target: [i32; 3], fp: &Footprint) -> [i32; 3] {
    let d = [
        target[0] - start[0],
        target[1] - start[1],
        target[2] - start[2],
    ];
    let steps = d[0].abs().max(d[1].abs()).max(d[2].abs()).max(1);
    for s in 0..=steps {
        let cell = [
            start[0] + d[0] * s / steps,
            start[1] + d[1] * s / steps,
            start[2] + d[2] * s / steps,
        ];
        if !world.standable_fp(cell, fp) {
            return cell;
        }
    }
    target
}

/// Plan every `move-actor` into a walked-path [`ActorMovePlan`] over the actor's
/// footprint, deduped by `(actor, to_anchor)` in first-seen order. `DW0325` when a
/// move is unroutable (names actor, leg, first blocked cell).
pub fn plan_actor_moves(plan: &Plan, world: &World) -> Result<Vec<ActorMovePlan>, NavError> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    for eff in all_effects(plan) {
        let QuestEffect::MoveActor {
            actor,
            to_anchor,
            speed,
            ..
        } = eff
        else {
            continue;
        };
        let key = (actor.as_str().to_string(), to_anchor.as_str().to_string());
        if !seen.insert(key) {
            continue;
        }
        let a = actor_of(plan, actor.as_str()).ok_or_else(|| NavError {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: unknown actor `{}` — declare it in the stage-5 `actors` list",
                actor.as_str()
            ),
        })?;
        let fp = entity_footprint(&a.entity);
        let start_anchor = actor_anchor_pos(plan, a.anchor.as_str()).ok_or_else(|| NavError {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: actor `{}` spawn anchor `{}` did not resolve to a world position — \
                 use a spawn `anchor` some area's prefab provides",
                actor.as_str(),
                a.anchor.as_str()
            ),
        })?;
        let dest = actor_anchor_pos(plan, to_anchor.as_str()).ok_or_else(|| NavError {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: destination anchor `{}` for actor `{}` did not resolve to a world \
                 position — use a `to_anchor` some area's prefab provides",
                to_anchor.as_str(),
                actor.as_str()
            ),
        })?;
        let start = world
            .snap_standable_fp(start_anchor, SNAP_RADIUS, &fp)
            .unwrap_or(start_anchor);
        let target = world
            .snap_standable_fp(dest, SNAP_RADIUS, &fp)
            .ok_or_else(|| NavError {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "move-actor: no cell the `{}` footprint can stand on near destination anchor \
                     `{}` {dest:?} for actor `{}` — the anchor is walled in, too low a ceiling for \
                     this mob, or over void",
                    a.entity,
                    to_anchor.as_str(),
                    actor.as_str()
                ),
            })?;
        let cells = world.find_path_fp(start, target, &fp).ok_or_else(|| {
            let blocked = first_blocked_fp(world, start, target, &fp);
            NavError {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "move-actor: actor `{}` ({}) cannot walk the leg `{}` {start:?} → `{}` {target:?} \
                     — no collision-free path for its footprint over the assembled geometry (first \
                     blocked cell ~{blocked:?}). Route the move within one connected area, widen the \
                     corridor/ceiling for this mob, or split it into shorter reachable hops",
                    actor.as_str(),
                    a.entity,
                    a.anchor.as_str(),
                    to_anchor.as_str(),
                ),
            }
        })?;
        let waypoints = resample(&cells, speed.unwrap_or(DEFAULT_SPEED));
        let yaws = yaws_along(&waypoints);
        out.push(ActorMovePlan {
            actor: actor.as_str().to_string(),
            to_anchor: to_anchor.as_str().to_string(),
            target,
            waypoints,
            yaws,
        });
    }
    Ok(out)
}

/// Verify every declared actor's spawn anchor resolves to a world position (the
/// puppet has somewhere to spawn). `DW0325` when it does not. Needs no `World` (a
/// spawn is a summon, not a walk), so it runs even for spawn-only campaigns.
pub fn check_actor_placement(plan: &Plan) -> Result<(), NavError> {
    for a in &plan.campaign.quests.content.actors {
        if actor_anchor_pos(plan, a.anchor.as_str()).is_none() {
            return Err(NavError {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "actor `{}` spawn anchor `{}` did not resolve to a world position — use an \
                     `anchor` some area's prefab provides",
                    a.id.as_str(),
                    a.anchor.as_str()
                ),
            });
        }
    }
    Ok(())
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
            push_deep(e, &mut out);
        }
    }
    for t in &plan.campaign.quests.content.triggers {
        for e in &t.effects {
            push_deep(e, &mut out);
        }
    }
    out
}

/// Push `e` and, recursively, every effect nested in a `sequence` step or a
/// `move-actor` `on_arrive` (spec-0014), so nav planning sees moves/cutscenes
/// wherever they appear. Pre-0.6 campaigns have no nesting, so the flattened list
/// equals the shallow one — output stays byte-identical.
fn push_deep<'a>(e: &'a QuestEffect, out: &mut Vec<&'a QuestEffect>) {
    out.push(e);
    match e {
        QuestEffect::Sequence { steps } => {
            for s in steps {
                for inner in &s.effects {
                    push_deep(inner, out);
                }
            }
        }
        QuestEffect::MoveActor { on_arrive, .. } => {
            for inner in on_arrive {
                push_deep(inner, out);
            }
        }
        _ => {}
    }
}

/// Whether the campaign uses any verb that needs the voxel `World` (`move-npc` or
/// `cutscene`). When false, the emitter skips building the occupancy model, so
/// v0.2/v0.3 output is untouched.
pub fn needs_world(plan: &Plan) -> bool {
    all_effects(plan).iter().any(|e| {
        matches!(
            e,
            QuestEffect::MoveNpc { .. } | QuestEffect::Cutscene { .. } | QuestEffect::MoveActor { .. }
        )
    })
    // The critical-path walkability check (DW0311) also needs the occupancy model.
        || has_walkable_critical_leg(plan)
    // The checkpoint (DW0315/DW0316) and stealth-zone (DW0327) proofs, v0.6, need
    // the assembled occupancy model too, as does the trap proof (DW0342, spec-0011).
        || !plan.checkpoints.is_empty()
        || !plan.stealth_beats.is_empty()
        || !plan.traps.is_empty()
}

/// The player-visited critical-path positions in order, each tagged with whether
/// the player was teleported here by an inter-area transport on the preceding
/// step (a ride, not a walk). `select-class` / `assert-complete` steps carry no
/// position and are skipped.
/// A player-visited critical-path position, with the metadata walked-leg routing
/// needs. Replaces the bare `([i32;3], bool)` tuple so a talk-to target — whose
/// anchor cell is the NPC's own occupied cell — can be endpoint-snapped correctly.
#[derive(Debug, Clone, Copy)]
struct VisitedPos {
    /// The raw visited anchor cell (an NPC stand, altar, chest, wave marker, …).
    pos: [i32; 3],
    /// The player rides an inter-area transport INTO this position, so the move
    /// here is a teleport, not a walk to validate/export.
    transport_before: bool,
    /// This position is a talk-to NPC anchor: the player stands *within interaction
    /// range beside* the NPC, never on the mannequin-occupied anchor cell, so the
    /// goal snap must exclude that cell (task #45).
    talk_to: bool,
    /// The originating `critical_path` step index (v0.6): lets the checkpoint /
    /// stealth proofs select the positions at or after a firing step.
    src_step: usize,
}

fn critical_positions(plan: &Plan) -> Vec<VisitedPos> {
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
            out.push(VisitedPos {
                pos,
                transport_before: transport_pending,
                talk_to: matches!(step, Step::TalkTo { .. }),
                src_step: i,
            });
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
    critical_positions(plan)
        .windows(2)
        .any(|w| !w[1].transport_before)
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
    route_visited(world, &critical_positions(plan), &plan.gate_events)
}

/// The gate-region cells sealed on a walked leg arriving at a position with
/// `src_step == before` (DSL v0.6 close-gate completability). A gate firing at
/// `fire_step` s affects a leg reaching a position with `src_step > s` (the same
/// `> fire_step` convention the checkpoint / stealth proofs use), so the gate state
/// during this leg is the **latest** firing on that region with `fire_step < before`
/// — sealed iff that firing is a `close-gate` (a later `open-gate` reopens it).
/// Regions with no qualifying close contribute nothing, so an open-gate-only
/// campaign yields an empty set and routes byte-identically to the base world.
fn sealed_gate_cells(gate_events: &[GateEvent], before: usize) -> BTreeSet<[i32; 3]> {
    // Per region, the final closed-state among firings strictly before `before`
    // (content order → last qualifying firing wins; deterministic).
    let mut state: BTreeMap<([i32; 3], [i32; 3]), bool> = BTreeMap::new();
    for ev in gate_events {
        if ev.fire_step < before {
            state.insert(ev.region, ev.closes);
        }
    }
    let mut sealed = BTreeSet::new();
    for (region, closed) in state {
        if closed {
            for cell in crate::assembled::region_cells(region.0, region.1) {
                sealed.insert(cell);
            }
        }
    }
    sealed
}

/// Route every walked leg between consecutive visited positions (the pure core of
/// [`check_critical_path`], split out so it is unit-testable without a full
/// [`Plan`]). A `transport_before` leg is a teleport ride and is skipped. Each leg
/// is routed over the world with any gate sealed by an earlier `close-gate`
/// ([`sealed_gate_cells`]) forced solid, so a forced path that must re-cross a
/// sealed gate fails [`DW_CRITICAL_UNROUTABLE`].
fn route_visited(
    world: &World,
    positions: &[VisitedPos],
    gate_events: &[GateEvent],
) -> Result<(), NavError> {
    for pair in positions.windows(2) {
        let from = pair[0].pos;
        let to = pair[1].pos;
        if pair[1].transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let sealed = sealed_gate_cells(gate_events, pair[1].src_step);
        let leg_world_owned;
        let leg_world: &World = if sealed.is_empty() {
            world
        } else {
            leg_world_owned = world.with_sealed(&sealed);
            &leg_world_owned
        };
        let start = leg_world.snap_endpoint(from, false).ok_or_else(|| NavError {
            code: DW_CRITICAL_UNROUTABLE,
            message: format!(
                "critical path: no standable floor within {SNAP_RADIUS} blocks of visited anchor \
                 {from:?} — a player-visited anchor sits walled in or over void. Fix the prefab \
                 so this anchor sits on/next to reachable floor; if the prefab looks correct, this \
                 is an assembly/toolchain defect — escalate rather than move the anchor into a \
                 wall"
            ),
        })?;
        let goal = leg_world
            .snap_endpoint(to, pair[1].talk_to)
            .ok_or_else(|| NavError {
                code: DW_CRITICAL_UNROUTABLE,
                message: format!(
                    "critical path: no standable floor within {SNAP_RADIUS} blocks of visited \
                     anchor {to:?} — a player-visited anchor sits walled in or over void (a \
                     talk-to NPC needs a dry standable cell beside it, within interaction range \
                     and clear of water). Fix the prefab so this anchor sits on/next to reachable \
                     floor; if the prefab looks correct, this is an assembly/toolchain defect — \
                     escalate rather than move it into a wall"
                ),
            })?;
        if leg_world.find_path(start, goal).is_none() {
            let gate_hint = if sealed.is_empty() {
                "this is a wedged doorway seam or a void gap in the assembled layout (or, if the \
                 jump is intended, a missing inter-area transport)."
            } else {
                "a `close-gate` has sealed a gate region on/before this leg (a point of no \
                 return), and the forced path must re-cross it. Reopen it with `open-gate` \
                 before this leg, route the forced path so it does not re-cross the sealed gate, \
                 or fire the `close-gate` later — do NOT delete the proof."
            };
            return Err(NavError {
                code: DW_CRITICAL_UNROUTABLE,
                message: format!(
                    "critical path: the player cannot walk from {from:?} (floor {start:?}) to \
                     {to:?} (floor {goal:?}) over the assembled geometry — no collision-free \
                     path. A same-area leg must be walkable end to end; {gate_hint}"
                ),
            });
        }
    }
    Ok(())
}

/// Prove no `set-checkpoint` strands the party (DSL v0.6, spec-0012). Two
/// obligations, per checkpoint:
///
/// 1. **Placement** ([`DW_CHECKPOINT_UNSTANDABLE`], `DW0316`): the checkpoint
///    anchor must have a standable floor cell within [`SNAP_RADIUS`] on the final
///    assembled model, or the party respawns into void / a wall. (Because the
///    relight pass — which runs before nav — proves every reachable walkable cell
///    meets the area's `min_light`, a standable, reachable checkpoint cell
///    provably meets `min_light` too; no separate light probe is needed.)
/// 2. **No stranding** ([`DW_CHECKPOINT_STRANDED`], `DW0315`, the core proof):
///    the DW0311 reachability, re-rooted at the checkpoint cell, must still reach
///    the remaining critical path. Since consecutive walked legs are already
///    proven forward-walkable, it suffices to reach the FIRST walked critical
///    position that fires after the checkpoint — reconnecting the whole forward
///    path. The message names the checkpoint and that first unreachable anchor,
///    and prescribes moving the checkpoint or adding a return route (never
///    deleting the checkpoint to silence the proof; #73 rubric).
pub fn check_checkpoints(plan: &Plan, world: &World) -> Result<(), NavError> {
    let cps: Vec<(String, [i32; 3], usize)> = plan
        .checkpoints
        .iter()
        .map(|c| (c.anchor.clone(), c.pos, c.fire_step))
        .collect();
    verify_checkpoints(world, &cps, &critical_positions(plan), &plan.gate_events)
}

/// The pure core of [`check_checkpoints`] (split out so it is unit-testable
/// against a synthetic [`World`] without a full [`Plan`]). Each checkpoint is
/// `(anchor, cell, fire_step)`.
fn verify_checkpoints(
    world: &World,
    checkpoints: &[(String, [i32; 3], usize)],
    positions: &[VisitedPos],
    gate_events: &[GateEvent],
) -> Result<(), NavError> {
    for (anchor, pos, fire_step) in checkpoints {
        let Some(cell) = world.snap_standable(*pos, SNAP_RADIUS) else {
            return Err(NavError {
                code: DW_CHECKPOINT_UNSTANDABLE,
                message: format!(
                    "checkpoint anchor `{anchor}` at {pos:?} has no standable floor within \
                     {SNAP_RADIUS} blocks on the assembled model — the party would respawn into \
                     void or a wall. Move the checkpoint onto reachable floor (not a trap-trigger, \
                     hazard, or mid-air cell); if the prefab looks correct, this is an \
                     assembly/toolchain defect — escalate rather than hide it."
                ),
            });
        };
        // The first walked critical position strictly after the checkpoint fires.
        let Some(target) = positions
            .iter()
            .filter(|p| p.src_step > *fire_step && !p.transport_before)
            .min_by_key(|p| p.src_step)
        else {
            continue; // nothing left to walk to (checkpoint at/near the finale)
        };
        // Seal any gate closed by the time the party reaches the target (the same
        // per-leg gate state DW0311 routes under), so a checkpoint whose forward
        // path is walled off by a `close-gate` strands the party (DSL v0.6).
        let sealed = sealed_gate_cells(gate_events, target.src_step);
        let leg_world_owned;
        let leg_world: &World = if sealed.is_empty() {
            world
        } else {
            leg_world_owned = world.with_sealed(&sealed);
            &leg_world_owned
        };
        let Some(goal) = leg_world.snap_endpoint(target.pos, target.talk_to) else {
            continue; // the target itself is unsnappable → a DW0311 concern, not ours
        };
        if leg_world.find_path(cell, goal).is_none() {
            return Err(NavError {
                code: DW_CHECKPOINT_STRANDED,
                message: format!(
                    "checkpoint `{anchor}` (cell {cell:?}) strands the party: the next required \
                     anchor {:?} is not walkable from it over the assembled geometry (a checkpoint \
                     behind a one-way drop the forward path can't re-cross after respawn). Move the \
                     checkpoint to a cell that keeps the remaining path reachable, or add a return \
                     route back up — do NOT delete the checkpoint to silence this proof.",
                    target.pos
                ),
            });
        }
    }
    Ok(())
}

/// Prove every `begin-stealth` zone is standable and reachable from the beat that
/// activates it (DSL v0.6, spec-0014) — [`DW_STEALTH_ZONE`] (`DW0327`). A zone the
/// player can never legally occupy (walled/void) or can never walk to from the
/// activating position is a guaranteed unwinnable stealth beat.
pub fn check_stealth_zones(plan: &Plan, world: &World) -> Result<(), NavError> {
    let beats: Vec<StealthProbe> = plan
        .stealth_beats
        .iter()
        .map(|b| (b.zones.clone(), b.fire_step))
        .collect();
    verify_stealth(world, &beats, &critical_positions(plan))
}

/// The pure core of [`check_stealth_zones`] (unit-testable against a synthetic
/// [`World`]). Each beat is `(zones, fire_step)`; each zone `(name, centre,
/// half-extents)`.
fn verify_stealth(
    world: &World,
    beats: &[StealthProbe],
    positions: &[VisitedPos],
) -> Result<(), NavError> {
    for (zones, fire_step) in beats {
        // The player's position at the activating beat: the visited position at the
        // firing step, else the nearest earlier one, else the first position.
        let player_pos = positions
            .iter()
            .filter(|p| p.src_step <= *fire_step)
            .max_by_key(|p| p.src_step)
            .or_else(|| positions.first())
            .map(|p| p.pos);
        for (name, pos, extent) in zones {
            let lo = [
                pos[0] - extent[0] as i32,
                pos[1] - extent[1] as i32,
                pos[2] - extent[2] as i32,
            ];
            let hi = [
                pos[0] + extent[0] as i32,
                pos[1] + extent[1] as i32,
                pos[2] + extent[2] as i32,
            ];
            let in_box = |c: [i32; 3]| (0..3).all(|k| lo[k] <= c[k] && c[k] <= hi[k]);
            let radius = extent.iter().copied().max().unwrap_or(0) as i32;
            let Some(stand) = world.snap_in_bounds(*pos, radius, &in_box) else {
                return Err(NavError {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (box {lo:?}..{hi:?}) has no standable cell — a \
                         player can never legally hide there, so the beat is unwinnable. Place the \
                         zone over reachable floor, or widen its `extent` to include a standable \
                         cell."
                    ),
                });
            };
            let start = player_pos.and_then(|p| world.snap_standable(p, SNAP_RADIUS));
            if let Some(start) = start
                && world.find_path(start, stand).is_none()
            {
                return Err(NavError {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (cell {stand:?}) is not reachable from the player's \
                         position {:?} when the stealth beat begins — the player would be caught \
                         before ever reaching cover. Route the zone within walkable reach of the \
                         activating beat, or move where the beat starts.",
                        player_pos.unwrap()
                    ),
                });
            }
        }
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
pub fn check_traps(plan: &Plan, world: &World, moves: &[MovePlan]) -> Result<(), NavError> {
    if plan.traps.is_empty() {
        return Ok(());
    }
    let required = world.required_path_cells(plan, moves);
    let spawn_starts: Vec<[i32; 3]> = plan
        .anchors
        .iter()
        .filter(|((_, name), _)| name == "spawn")
        .filter_map(|(_, a)| match a {
            ResolvedAnchor::Point { pos, .. } => Some(*pos),
            ResolvedAnchor::Gate { .. } => None,
        })
        .collect();
    verify_traps(world, &plan.traps, &required, &spawn_starts)
}

/// The pure core of [`check_traps`] (unit-testable against a synthetic [`World`]).
/// `required` is the forced critical-path cell set; `spawn_starts` are the spawn
/// cells the disarm-reachability search roots at.
fn verify_traps(
    world: &World,
    traps: &[TrapPlan],
    required: &BTreeSet<[i32; 3]>,
    spawn_starts: &[[i32; 3]],
) -> Result<(), NavError> {
    for t in traps {
        if !matches!(t.lethality, Lethality::Lethal) {
            continue; // only lethal traps carry the obligation
        }
        let tc = t.trigger_cell;
        // (a) Avoidable: the trigger cell is never a forced critical-path cell.
        if !required.contains(&tc) {
            continue;
        }
        // (b) Survivable: a single-shot trap fires once and is spent; the respawn
        // walk-back (keep_inventory) never re-triggers it → no soft-loop.
        if matches!(t.reset, TrapReset::Once) {
            continue;
        }
        // (c) Disarmable: a disarm affordance reachable before the trap is forced.
        if let Some(dis) = &t.disarm
            && disarm_reachable_before(world, spawn_starts, dis.via_cell, tc)
        {
            continue;
        }
        return Err(NavError {
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

/// A walked critical-path leg with the full A* cell route the compiler proved
/// connects it — the export counterpart of [`check_critical_path`] (task #38).
/// `from`/`to` are the raw visited anchor cells (identical to the harness
/// `critical-path.json` step positions, so the harness can key a leg by its
/// destination); `cells` is the standable-cell polyline between their snapped floor
/// endpoints, inclusive of both.
#[derive(Debug, Clone)]
pub struct LegRoute {
    /// The raw visited anchor the leg walks FROM (the previous critical position).
    pub from: [i32; 3],
    /// The raw visited anchor the leg walks TO (this critical position).
    pub to: [i32; 3],
    /// The `critical_path` step index of the destination position — the objective
    /// this leg walks toward. Lets the visual-tier POV planner
    /// (`crate::render_plan`) name the served objective without re-deriving the
    /// leg selection.
    pub to_step: usize,
    /// The standable-cell A* path from the snapped `from` floor to the snapped `to`
    /// floor, inclusive of both endpoints.
    pub cells: Vec<[i32; 3]>,
}

/// Compute the proven A* cell route for every WALKED critical-path leg (transport
/// hops skipped), for export as validation metadata (see the `waypoints` module).
/// Mirrors [`check_critical_path`]'s leg selection and endpoint snapping exactly, so
/// an exported leg is the same route the DW0311 guard proved routable. Intended to
/// be called only after [`check_critical_path`] has succeeded; a leg that fails to
/// snap or route is silently omitted (cannot occur once the check has passed).
pub fn critical_path_routes(plan: &Plan, world: &World) -> Vec<LegRoute> {
    let positions = critical_positions(plan);
    let mut out = Vec::new();
    for pair in positions.windows(2) {
        let from = pair[0].pos;
        let to = pair[1].pos;
        if pair[1].transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let (Some(start), Some(goal)) = (
            world.snap_endpoint(from, false),
            world.snap_endpoint(to, pair[1].talk_to),
        ) else {
            continue;
        };
        if let Some(cells) = world.find_path(start, goal) {
            out.push(LegRoute {
                from,
                to,
                to_step: pair[1].src_step,
                cells,
            });
        }
    }
    out
}

/// `DW0314`: an exported critical-path waypoint is not standable in the FINAL
/// assembled world (settled + water-flooded + relight fixtures). A build-time
/// self-check over the very cells the harness will replay: it makes it structurally
/// impossible to ship a waypoint the game floods or walls (the water-flow /
/// post-nav-mutation divergence class — task #45). Every cell a leg exports comes
/// from `find_path` over this same world, so this can only fire if a later pass
/// mutates a cell nav relied on or an endpoint resolves off the walkable set — in
/// which case it is a compiler/assembly defect to escalate, never a cell to nudge.
pub const DW_WAYPOINT_NOT_STANDABLE: &str = "DW0314";

/// Assert every exported waypoint cell is standable in `world` — the final model the
/// routes were computed over (settled + flooded + fixtures). Returns
/// [`DW_WAYPOINT_NOT_STANDABLE`] (`DW0314`) naming the first offending cell/leg on
/// violation. This is the structural guard the water-flood model exists to make
/// enforceable: a waypoint in a flooded (or newly-walled) cell fails the build
/// loudly instead of stranding the bot at runtime (task #45).
pub fn verify_exported_routes(world: &World, routes: &[LegRoute]) -> Result<(), NavError> {
    for leg in routes {
        for &cell in &leg.cells {
            if !world.is_standable(cell) {
                return Err(NavError {
                    code: DW_WAYPOINT_NOT_STANDABLE,
                    message: format!(
                        "critical-path waypoint export: cell {cell:?} on the leg to {to:?} is not \
                         standable in the final assembled world (it is solid, water-flooded, or \
                         has no floor). A proven route must not cross a cell a later pass mutated \
                         — this is the water-flow / post-nav-mutation divergence class: fix the \
                         prefab/water or the assembly, do not move the waypoint. (leg from {from:?})",
                        to = leg.to,
                        from = leg.from,
                    ),
                });
            }
        }
    }
    Ok(())
}

/// `DW0724`: a visual-tier player-POV camera eye cell is occupied (a solid block
/// or water) in the FINAL assembled world — the frame would render the inside of a
/// block instead of the scene the player sees. A self-check on the POV camera
/// derivation (`crate::render_plan`), mirroring the DW0314 waypoint self-check:
/// every POV camera stands at the eye-height of a DW0314-proven-standable waypoint,
/// so this cannot fire unless the eye-height derivation changes to place the eye in
/// a ceiling/wall (or a later pass mutates the cell). It makes "the camera clips a
/// wall" — the owner's exact visual-review failure mode — a build error to fix at
/// its source (the camera derivation), never a shot to nudge or a data value to
/// change.
pub const DW_POV_CAMERA_OCCLUDED: &str = "DW0724";

/// Assert every player-POV camera eye cell is clear (unoccupied) in `world` — the
/// final assembled model. Each entry is `(shot_id, eye_cell)` where `eye_cell` is
/// the integer block the camera eye sits in (`floor` of the eye position). Returns
/// [`DW_POV_CAMERA_OCCLUDED`] (`DW0724`) naming the first offending shot on
/// violation. The structural guard behind the visual tier: it is impossible to ship
/// a render plan whose first-person camera looks out from inside geometry.
pub fn verify_pov_cameras(world: &World, cameras: &[(String, [i32; 3])]) -> Result<(), NavError> {
    for (id, eye) in cameras {
        if !world.is_clear(*eye) {
            return Err(NavError {
                code: DW_POV_CAMERA_OCCLUDED,
                message: format!(
                    "player-POV shot `{id}`: the camera eye cell {eye:?} is occupied (a solid \
                     block or water) in the assembled world — the frame would render the inside \
                     of a block, not the player's view. The eye sits at 1.62 above a proven \
                     standable waypoint, so fix the POV camera derivation (eye height / standing \
                     cell) — do NOT move the waypoint or the geometry."
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
        World::from_solid_cells(solid)
    }

    /// A non-talk-to visited position (test convenience for `route_visited`).
    fn vp(pos: [i32; 3], transport_before: bool) -> VisitedPos {
        VisitedPos {
            pos,
            transport_before,
            talk_to: false,
            src_step: 0,
        }
    }

    #[test]
    fn pov_camera_in_open_air_passes_but_inside_a_block_is_dw0724() {
        // A flat floor at y=64 with headroom; the eye of a standing player is at
        // y=65..66 (clear). A camera eye in a clear cell passes; one placed inside
        // the floor block is DW0724.
        let world = floored(5, 5, 65, &[]);
        assert!(world.is_clear([2, 65, 2]), "standing eye cell is clear");
        // Clear eye → ok.
        verify_pov_cameras(&world, &[("pov/leg0/wp0".into(), [2, 65, 2])]).expect("clear eye ok");
        // Eye buried in the solid floor → DW0724.
        let err = verify_pov_cameras(&world, &[("pov/leg0/wp1".into(), [2, 64, 2])])
            .expect_err("occupied eye must fail");
        assert_eq!(err.code, DW_POV_CAMERA_OCCLUDED);
        assert!(err.message.contains("pov/leg0/wp1"), "names the shot");
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
        let world = World::from_solid_cells(solid);
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
        let world = World::from_solid_cells(solid);
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
        let world = World::from_solid_cells(solid);
        let a = [0, 65, 1];
        let b = [4, 65, 1];
        assert!(world.standable(a) && world.standable(b));
        // Walked leg → unroutable → DW0311.
        let err = route_visited(&world, &[vp(a, false), vp(b, false)], &[]).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        // Same leg ridden by an inter-area transport → skipped, ok.
        assert!(route_visited(&world, &[vp(a, false), vp(b, true)], &[]).is_ok());
    }

    #[test]
    fn talk_to_endpoint_excludes_the_npc_cell_and_flooded_cells() {
        // A flat floor; the NPC anchor cell is standable (a mannequin stands on the
        // floor), and one adjacent cell is flooded. The talk-to goal snap must NOT
        // return the NPC's own cell, and must skip the flooded neighbour — it lands
        // on a dry standable cell beside the NPC, within interaction range.
        let mut solid = BTreeSet::new();
        for x in 0..5 {
            for z in 0..5 {
                solid.insert([x, 64, z]); // floor at y=64, standable at y=65
            }
        }
        let npc = [2, 65, 2];
        let flooded: BTreeSet<[i32; 3]> = [[1, 65, 2]].into_iter().collect(); // west neighbour is water
        let world = World::from_solid_and_flooded(solid, flooded);
        assert!(
            world.standable(npc),
            "the NPC cell itself is standable in the model"
        );
        let goal = world
            .snap_endpoint(npc, true)
            .expect("a dry standable cell beside the NPC exists");
        assert_ne!(goal, npc, "must not stand on the NPC's own (occupied) cell");
        assert!(
            !world.flooded.contains(&goal),
            "must not stand in water: {goal:?}"
        );
        assert!(world.standable(goal));
        // …and it is within interaction range (adjacent) of the NPC.
        let d2 = (0..3).map(|i| (goal[i] - npc[i]).pow(2)).sum::<i32>();
        assert!(
            d2 <= SNAP_RADIUS * SNAP_RADIUS,
            "goal {goal:?} within range of NPC"
        );
    }

    #[test]
    fn verify_exported_routes_rejects_a_flooded_waypoint_dw0314() {
        // Synthetic negative for the DW0314 self-check (task #45): a hand-built leg
        // whose polyline crosses a flooded cell must fail the standability guard.
        let mut solid = BTreeSet::new();
        for x in 0..4 {
            solid.insert([x, 64, 0]); // floor
        }
        let flooded: BTreeSet<[i32; 3]> = [[2, 65, 0]].into_iter().collect(); // a water tongue on the route
        let world = World::from_solid_and_flooded(solid, flooded);
        let routes = vec![LegRoute {
            from: [0, 65, 0],
            to: [3, 65, 0],
            to_step: 1,
            cells: vec![[0, 65, 0], [1, 65, 0], [2, 65, 0], [3, 65, 0]],
        }];
        let err = verify_exported_routes(&world, &routes).unwrap_err();
        assert_eq!(err.code, DW_WAYPOINT_NOT_STANDABLE);
        assert!(
            err.message.contains("[2, 65, 0]"),
            "names the offending cell: {}",
            err.message
        );
        // A route entirely on dry standable floor passes.
        let dry = vec![LegRoute {
            from: [0, 65, 0],
            to: [1, 65, 0],
            to_step: 1,
            cells: vec![[0, 65, 0], [1, 65, 0]],
        }];
        assert!(verify_exported_routes(&world, &dry).is_ok());
    }

    #[test]
    fn critical_path_routable_leg_passes() {
        // A flat connected floor: consecutive visited cells are walkable → ok.
        let world = floored(6, 3, 65, &[]);
        assert!(
            route_visited(&world, &[vp([0, 65, 1], false), vp([5, 65, 1], false)], &[]).is_ok()
        );
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
        let world = World::from_solid_cells(solid);
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
    fn step_up_needs_head_clearance_to_jump() {
        // Lower stand at x=0 (floor y64 → stand y65); raised stand at x=1,2 (floor
        // y65 → stand y66). Reaching the raised floor means jumping up one block at
        // x=0, whose head sweeps the cell two above the feet ([0,67,0]).
        let mk = |low_ceiling: bool| {
            let mut solid = BTreeSet::new();
            solid.insert([0, 64, 0]); // lower floor
            solid.insert([1, 65, 0]); // raised floor
            solid.insert([2, 65, 0]);
            if low_ceiling {
                solid.insert([0, 67, 0]); // ceiling two above the jumper's feet
            }
            World::from_solid_cells(solid)
        };
        // Open headroom: the jump-up is walkable.
        let open = mk(false);
        assert!(open.standable([0, 65, 0]) && open.standable([2, 66, 0]));
        assert!(open.find_path([0, 65, 0], [2, 66, 0]).is_some());
        // A ceiling two above the feet blocks the jump (the entity would head-bonk),
        // so no walkable path exists — the DW0311 case a runtime bot rejects with
        // "No path to the goal!".
        let low = mk(true);
        assert!(low.standable([0, 65, 0]) && low.standable([2, 66, 0]));
        assert!(low.find_path([0, 65, 0], [2, 66, 0]).is_none());
    }

    #[test]
    fn critical_path_route_returns_the_proven_cell_polyline() {
        // A flat connected floor: the walked leg's exported route is the A* cell
        // path, inclusive of both snapped endpoints (task #38 export).
        let world = floored(8, 3, 65, &[]);
        let a = [0, 65, 1];
        let b = [6, 65, 1];
        let cells = world.find_path(a, b).expect("routable");
        assert_eq!(cells.first(), Some(&a));
        assert_eq!(cells.last(), Some(&b));
        // Every cell on an exported route is standable (a real floor cell).
        for c in &cells {
            assert!(world.standable(*c), "route cell {c:?} not standable");
        }
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

    // --- v0.6 checkpoint / stealth proofs (spec-0012 / spec-0014) ---

    /// Two floor patches (x∈{0,1} and x∈{3,4}) with a void gap at x=2.
    fn split_world(y: i32) -> World {
        let mut solid = BTreeSet::new();
        for x in [0, 1, 3, 4] {
            for z in 0..3 {
                solid.insert([x, y - 1, z]); // floor
                solid.insert([x, y + 2, z]); // ceiling
            }
        }
        World::from_solid_cells(solid)
    }

    fn at_step(pos: [i32; 3], src_step: usize) -> VisitedPos {
        VisitedPos {
            pos,
            transport_before: false,
            talk_to: false,
            src_step,
        }
    }

    // --- close-gate completability (DSL v0.6) --------------------------------

    /// A `close-gate` firing before a forced walked leg seals the gate region, so a
    /// critical path that must re-cross it fails DW0311; a later `open-gate` before
    /// the same leg reopens it and the route passes again.
    #[test]
    fn close_gate_seals_a_forced_leg_is_dw0311() {
        // A 1-wide corridor along x, y=65; the pass-through cell [2,65,0] is the sole
        // connection between the two ends. Base world (gate open) routes end to end.
        let world = floored(5, 1, 65, &[]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        assert!(
            route_visited(&world, &[a, b], &[]).is_ok(),
            "the open corridor must route with no gate events"
        );
        // A close-gate seals the pass-through before the leg to `b` (fire_step 0 < 2).
        let close = GateEvent {
            region: ([2, 65, 0], [2, 65, 0]),
            closes: true,
            fire_step: 0,
        };
        let err = route_visited(&world, &[a, b], std::slice::from_ref(&close)).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        assert!(
            err.message.contains("close-gate"),
            "the message must name the sealed gate: {}",
            err.message
        );
        // Reopening the gate before the leg (open-gate at a later fire_step) restores it.
        let open = GateEvent {
            region: ([2, 65, 0], [2, 65, 0]),
            closes: false,
            fire_step: 1,
        };
        assert!(
            route_visited(&world, &[a, b], &[close, open]).is_ok(),
            "a gate reopened by open-gate before the leg must route again"
        );
    }

    /// A `close-gate` that walls off the forward path from a checkpoint strands the
    /// party (DW0315) — the checkpoint gate proof routes under the same per-leg seal.
    #[test]
    fn close_gate_walls_off_checkpoint_forward_path_is_dw0315() {
        let world = floored(5, 1, 65, &[]);
        // Checkpoint at the near end (fire_step 0); the next required anchor is past
        // the gate cell [2,65,0].
        let cps = vec![("cp/rest".to_string(), [0, 65, 0], 0usize)];
        let positions = vec![at_step([4, 65, 0], 1)];
        // Open gate → reachable.
        assert!(verify_checkpoints(&world, &cps, &positions, &[]).is_ok());
        // Sealed before the party reaches the target (fire_step 0 < 1) → stranded.
        let close = GateEvent {
            region: ([2, 65, 0], [2, 65, 0]),
            closes: true,
            fire_step: 0,
        };
        let err =
            verify_checkpoints(&world, &cps, &positions, std::slice::from_ref(&close)).unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    #[test]
    fn checkpoint_behind_a_one_way_drop_is_dw0315() {
        // Checkpoint on the near patch; the next required anchor is on the far,
        // disconnected patch → not walkable from the checkpoint → DW0315.
        let world = split_world(65);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        let err = verify_checkpoints(&world, &cps, &positions, &[]).unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    #[test]
    fn checkpoint_with_reachable_remaining_path_passes() {
        // Both the checkpoint and the next anchor sit on the same connected floor.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        assert!(verify_checkpoints(&world, &cps, &positions, &[]).is_ok());
    }

    #[test]
    fn checkpoint_over_void_is_dw0316() {
        // The checkpoint cell has no standable floor within snap radius.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [20, 65, 20], 0usize)];
        let err = verify_checkpoints(&world, &cps, &[], &[]).unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_UNSTANDABLE); // DW0316
    }

    #[test]
    fn stealth_zone_over_void_is_dw0327() {
        // A zero-extent zone centred on a void cell → no standable cell → DW0327.
        let world = floored(5, 3, 65, &[]);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [20, 65, 20], [0, 0, 0])],
            0usize,
        )];
        let err = verify_stealth(&world, &beats, &[at_step([2, 65, 1], 0)]).unwrap_err();
        assert_eq!(err.code, DW_STEALTH_ZONE); // DW0327
    }

    #[test]
    fn stealth_zone_unreachable_from_beat_is_dw0327() {
        // The zone is standable on the far patch, but the activating beat sits on
        // the near patch across a void gap → unreachable → DW0327.
        let world = split_world(65);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 1, 1])],
            0usize,
        )];
        let err = verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).unwrap_err();
        assert_eq!(err.code, DW_STEALTH_ZONE); // DW0327
    }

    #[test]
    fn stealth_zone_standable_and_reachable_passes() {
        let world = floored(6, 3, 65, &[]);
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 1, 1])],
            0usize,
        )];
        assert!(verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).is_ok());
    }

    #[test]
    fn player_footprint_matches_pre_0_6_walkability() {
        // find_path (the delegating wrapper) must equal find_path_fp(player) — the
        // byte-identity guarantee for move-npc / critical-path.
        let world = floored(6, 3, 65, &[[3, 65, 1]]);
        let fp = Footprint::player();
        let a = [0, 65, 1];
        let b = [5, 65, 1];
        assert_eq!(world.find_path(a, b), world.find_path_fp(a, b, &fp));
        // Player footprint is one column, two cells tall.
        assert_eq!(fp.cols, vec![[0, 0]]);
        assert_eq!(fp.height, 2);
    }

    #[test]
    fn tall_footprint_cannot_walk_a_two_high_gap_a_player_fits() {
        // `floored` gives a floor at y-1 and a ceiling at y+2 → two clear cells (y,
        // y+1): a player (2 tall) fits; a warden (2.9 → 3 tall) head-bonks the
        // ceiling, so its footprint has no walkable path (the DW0325 condition).
        let world = floored(6, 3, 65, &[]);
        let a = [0, 65, 1];
        let b = [5, 65, 1];
        let player = Footprint::player();
        let warden = entity_footprint("minecraft:warden");
        assert_eq!(warden.height, 3, "warden is 2.9 tall → 3 cells");
        assert!(
            world.find_path_fp(a, b, &player).is_some(),
            "a player fits the 2-high corridor"
        );
        assert!(
            !world.standable_fp(a, &warden),
            "a warden cannot stand under a 2-high ceiling"
        );
        assert!(
            world.find_path_fp(a, b, &warden).is_none(),
            "a warden cannot walk the 2-high corridor → unroutable"
        );
        // The best-effort blocked-cell reporter names a non-standable cell on the leg.
        let blocked = first_blocked_fp(&world, a, b, &warden);
        assert!(!world.standable_fp(blocked, &warden));
    }

    #[test]
    fn dims_table_and_default_fallback() {
        // Sub-block-wide mobs are single-column; the default fallback is humanoid.
        assert_eq!(entity_footprint("minecraft:sheep").cols, vec![[0, 0]]);
        assert_eq!(entity_footprint("minecraft:sheep").height, 2); // 1.3 → 2
        assert_eq!(entity_footprint("minecraft:iron_golem").height, 3); // 2.7 → 3
        let unknown = entity_footprint("minecraft:some_new_mob");
        assert_eq!(unknown.cols, vec![[0, 0]]);
        assert_eq!(unknown.height, 2);
    }

    #[test]
    fn yaw_follows_the_movement_tangent() {
        // MC yaw: 0 = +z (south), 90 = -x (west), 180 = -z (north), 270 = +x (east).
        assert_eq!(yaw_of(0.0, 1.0), Some(0));
        assert_eq!(yaw_of(-1.0, 0.0), Some(90));
        assert_eq!(yaw_of(0.0, -1.0), Some(180));
        assert_eq!(yaw_of(1.0, 0.0), Some(270));
        assert_eq!(yaw_of(0.0, 0.0), None);
        // A straight +x path yaws every waypoint east (270), including the last.
        let wps = vec![[0.0, 65.0, 0.0], [1.0, 65.0, 0.0], [2.0, 65.0, 0.0]];
        assert_eq!(yaws_along(&wps), vec![270, 270, 270]);
    }

    // --- v0.6 trap completability proof (spec-0011, DW0342) ---

    use crate::plan::TrapDisarmPlan;
    use delvewright_dsl::{Lethality, TrapReset, TrapTrigger};

    /// A 1-wide walkable corridor along z=1: a floor strip at `[0..len, y-1, 1]`.
    /// `[x, y, 1]` are the only standable cells, so the corridor has no bypass — a
    /// cell on it is a genuine chokepoint the player cannot walk around.
    fn corridor(len: i32, y: i32) -> World {
        let mut solid = BTreeSet::new();
        for x in 0..len {
            solid.insert([x, y - 1, 1]);
        }
        World::from_solid_cells(solid)
    }

    /// A minimal lethal trap for the proof tests.
    fn lethal_trap(cell: [i32; 3], reset: TrapReset, disarm: Option<TrapDisarmPlan>) -> TrapPlan {
        TrapPlan {
            id: "trap/darts".to_string(),
            safe: "darts".to_string(),
            trigger: TrapTrigger::PressurePlate,
            trigger_cell: cell,
            dispenser: None,
            payload: None,
            lethality: Lethality::Lethal,
            reset,
            disarm,
            requires_flags: Vec::new(),
        }
    }

    #[test]
    fn forced_lethal_rearm_trap_with_no_discharge_is_dw0342() {
        // A rearming lethal trap on a required chokepoint, no disarm → soft-loop.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        let err = verify_traps(&world, &traps, &required, &[[0, 65, 1]]).unwrap_err();
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE);
    }

    #[test]
    fn forced_lethal_once_trap_is_survivable() {
        // The same forced trap set to `once` fires and is spent — no soft-loop.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        let traps = [lethal_trap(tc, TrapReset::Once, None)];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]]).is_ok());
    }

    #[test]
    fn off_path_lethal_trap_is_avoidable() {
        // A rearming lethal trap whose trigger cell is NOT a required path cell.
        let world = corridor(6, 65);
        let tc = [3, 65, 1];
        let required: BTreeSet<[i32; 3]> = BTreeSet::new(); // path avoids the trap
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]]).is_ok());
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
        assert!(verify_traps(&world, &traps, &required, &[[0, 65, 1]]).is_ok());
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
        let err = verify_traps(&world, &traps, &required, &[[0, 65, 1]]).unwrap_err();
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE);
    }

    #[test]
    fn non_lethal_forced_trap_carries_no_obligation() {
        // A harmful (non-lethal) trap on the forced path is fine — no DW0342.
        let world = corridor(6, 65);
        let mut t = lethal_trap([3, 65, 1], TrapReset::Rearm, None);
        t.lethality = Lethality::Harmful;
        let required: BTreeSet<[i32; 3]> = (0..6).map(|x| [x, 65, 1]).collect();
        assert!(verify_traps(&world, &[t], &required, &[[0, 65, 1]]).is_ok());
    }
}
