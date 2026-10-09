//! How a body stands in the [`World`] and moves through it: footprints, the
//! standability rule, the step rule's measurement of a rise, the moves a
//! player, an NPC or a mob may take from a cell, and the step costs the
//! router prices them at.

use crate::compiler::nav::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// An entity's collision footprint over the voxel grid: the set of column offsets
/// it occupies horizontally and the number of vertical cells it needs clear
/// (`ceil(height)`). Standing feet-centred on a cell, an entity of `width <= 1`
/// occupies a single column; a taller entity needs more headroom (the warden, 2.9
/// tall, needs 3 cells vs a player's 2 — so it cannot walk a 2-high gap a player
/// fits). Drives footprint-aware standability + A* so a `move-actor` path is
/// walkable for the ACTUAL puppet, not a generic 1×2 humanoid (spec-0014).
#[derive(Debug, Clone)]
pub struct Footprint {
    /// Horizontal column offsets `[dx, dz]` the body occupies (feet cell = `[0, 0]`).
    pub(in crate::compiler::nav) cols: Vec<[i32; 2]>,
    /// Vertical cells of clearance the body needs (`ceil(height)`, min 1).
    pub(in crate::compiler::nav) height: i32,
    /// The un-quantized hitbox the columns were derived from.
    ///
    /// Carried rather than re-derived because the cell quantization is lossy in
    /// the direction that matters here: a 0.6-wide player and a 1.0-wide body
    /// both occupy one column, and only the second's box reaches a block face
    /// from the middle of its own cell. Anything asking a **sub-block** question
    /// of the routed body — whether its hitbox meets a lethal volume — asks this
    /// (`delvewright_dsl::metrics::Body`), never the columns.
    body: delvewright_dsl::metrics::Body,
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
        Footprint {
            cols,
            height: h,
            body: delvewright_dsl::metrics::Body::new(width, height),
        }
    }

    /// The un-quantized hitbox this footprint routes.
    pub fn body(&self) -> delvewright_dsl::metrics::Body {
        self.body
    }

    /// The default humanoid footprint (player / villager / mannequin: 0.6 × 1.8 →
    /// single column, 2 cells tall). Byte-identical to the pre-spec-0014 walkability
    /// model, so `move-npc` and critical-path routing are unchanged.
    pub fn player() -> Footprint {
        Footprint::for_dims(0.6, 1.8)
    }
}

/// The standing hitbox `(width, height)` in blocks for a vanilla entity id
/// (spec-0014 per-entity dims table) — the ONE table in the compiler that knows
/// how big a mob's body is. Covers the 1.21.11 mobs an actor or a re-dressed NPC
/// mannequin is likely to wear; anything unlisted falls back to the humanoid
/// default (0.6 × 1.95).
///
/// Two consumers, deliberately sharing one source of truth: [`entity_footprint`]
/// quantizes it to walkable cells for actor routing, and [`crate::compiler::eclipse`] uses
/// the raw floats for the sub-block body-vs-affordance overlap test (`DW0359`) —
/// a rule the cell-quantized view could not state honestly (a 1.4-wide iron
/// golem occupies three columns of *clearance* but its body is only 1.4 blocks
/// of *hitbox*).
pub fn entity_dims(entity: &str) -> (f64, f64) {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        "warden" => (0.9, 2.9),
        "iron_golem" => (1.4, 2.7),
        "ravager" => (1.95, 2.2),
        "hoglin" | "zoglin" => (1.4, 1.4),
        "sheep" | "goat" | "pig" | "cow" | "mooshroom" | "wolf" | "fox" | "panda" => (0.9, 1.4),
        "villager" | "zombie" | "husk" | "zombie_villager" => (0.6, 1.95),
        "skeleton" | "stray" | "wither_skeleton" => (0.6, 1.99),
        "creeper" | "enderman" => (0.6, 1.9),
        "allay" | "vex" => (0.35, 0.6),
        // The player's own row reads the metrics table, so the body every proof
        // in this engine routes is the body `delvec metrics` publishes. The mobs
        // around it stay literals: they are not the player, and a metrics table
        // that enumerated the 1.21.11 mob roster would be a registry dump.
        "armor_stand" | "player" | "mannequin" => (
            delvewright_dsl::metrics::PLAYER_WIDTH,
            delvewright_dsl::metrics::PLAYER_HEIGHT,
        ),
        _ => (0.6, 1.95),
    }
}

/// The collision height of a vanilla fence / wall / closed fence gate, in blocks
/// 1.5, half a block above the cell it sits in.
pub const BARRIER_HEIGHT: f64 = 1.5;

/// The entity id whose body a stage-2 NPC actually wears in the shipped delve.
/// A skinned NPC is summoned as `minecraft:mannequin` — the player model, not the
/// declared `base_entity` (see `emit::npc_summon_commands`) — so every geometric
/// proof about NPC bodies ([`crate::compiler::eclipse`], [`crate::compiler::clearance`]) must model
/// what ships, not what is declared. One helper, so the two cannot drift.
pub fn npc_body_entity(n: &delvewright_dsl::Npc) -> String {
    delvewright_dsl::BodyRef::Npc(n).worn_entity().to_string()
}

/// The entity id whose body a stage-5 actor wears — the actor's counterpart of
/// [`npc_body_entity`], same mannequin rule.
pub fn actor_body_entity(a: &delvewright_dsl::Actor) -> String {
    delvewright_dsl::BodyRef::Actor(a).worn_entity().to_string()
}

/// The hitbox footprint for a vanilla entity id (spec-0014 per-entity dims table).
/// Standing hitboxes for the 1.21.11 mobs an actor is likely to puppet; anything
/// unlisted falls back to the humanoid default (0.6 × 1.95). Width only matters
/// past 1.0 (sub-block mobs are single-column); height gates vertical clearance.
pub fn entity_footprint(entity: &str) -> Footprint {
    let (w, h) = entity_dims(entity);
    Footprint::for_dims(w, h)
}

/// How far to search for a standable floor cell when a `move-npc` endpoint anchor
/// is a solid affordance (altar / gate bars / wall marker) the NPC must stop in
/// front of rather than stand inside.
pub const SNAP_RADIUS: i32 = 3;

/// A reachability root: a declared cell, plus the AABB the snap that seats it may
/// not leave — the assembled piece that DECLARES the anchor.
///
/// The confinement exists because [`World::snap_in_bounds`] (and so
/// [`World::snap`]) chooses the nearest standable cell by squared distance and
/// **nothing else**: it does not care that solid geometry stands between the
/// anchor and the cell it lands on. An anchor a campaign must declare in a room's
/// ceiling — every spec-0022 `collapse` payload has one — is therefore closer to
/// the cell on top of the ROOF than to the floor below it, and snaps up through
/// the ceiling onto a component no player can ever walk to. Every proof rooted
/// there then reasons about the roof: boundary safety
/// ([`verify_boundary_safety`]) demanded a safe edge on a bare platform in a void
/// world, which is unsatisfiable by construction.
///
/// This is the same leak [`World::confined_standable_cells`] closed for wave
/// seating, one layer up: there a flood from a wave anchor crossed a
/// socket seam into the neighbouring piece, here a *snap* crosses a ceiling into
/// nothing. The piece AABB is the boundary in both cases because it is the only
/// shape that says "the room this anchor was authored inside".
///
/// Only the seating is confined. The walk that follows is deliberately not: a
/// player who reaches a room reaches whatever it connects to, and confining the
/// flood would shrink the region a boundary proof examines — a weaker check, not
/// a more correct one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorRoot {
    /// The declared cell (a `Point` anchor's position, a `Gate` anchor's `from`).
    pub at: [i32; 3],
    /// World AABB `(min, max)` of the piece that declares it.
    pub within: ([i32; 3], [i32; 3]),
}

/// The step rule's three constants, taken from the metrics table (spec-0049 §2)
/// rather than declared here.
///
/// The direction of the import is what the single-authority obligation means
/// concretely: the exported player metrics ARE these constants at compile time,
/// not a second table that agrees with them, so `delvec metrics` cannot describe
/// a walker this model does not route. Their derivations are on the definitions
/// (`dsl::metrics::MAX_AUTO_STEP_16` is vanilla's 0.6-block `maxUpStep` rounded
/// down to a sixteenth; `MAX_JUMP_RISE_16` is the ≈1.2522-block apex).
///
/// The **rule** they express comes from the same place, and no longer from here:
/// [`step_allowed`] is what [`World::neighbors_fp`] asks. It moved there because
/// this engine had a second answer to the same question — `delvec::schem`'s
/// box-of-cells walk, which a prefab's admission gate proves over — and a rule
/// with two implementations is a rule two gates can disagree about. What stays
/// this module's is the **measurement** of a rise: real collision tops, water,
/// tall barriers and a footprint's highest supporting face are facts about an
/// assembled world, and a box of cells has none of them.
use delvewright_dsl::metrics::{
    FULL_16, JUMP_REACH, WATER_CLIMB_OUT_RISE, jump_max_gap, step_allowed,
    unarmoured_survivable_fall_blocks,
};

impl World {
    /// Whether a cell is a valid standing position (feet + head passable, solid
    /// ground below). Public wrapper over the internal walkability rule so the
    /// relight pass (spec-0010) can collect reachable walkable cells.
    pub fn is_standable(&self, c: [i32; 3]) -> bool {
        self.standable(c)
    }

    /// **Where a standing body's feet actually are**, in blocks, for a body
    /// standing in cell `c` — the cell's own floor unless the support under it is
    /// partial, in which case it is lower (a bottom slab puts them half a block
    /// down).
    ///
    /// Public wrapper over [`World::feet_16_fp`] at the player footprint, for the
    /// same reason [`World::is_standable`] is one: a caller that needs the height a
    /// body's hitbox starts at must read this model's own measurement rather than
    /// assume the cell floor. [`crate::compiler::reach`] is that caller — vanilla adjudicates
    /// a completion volume against the body's AABB, and the AABB starts at the feet.
    pub fn feet_y(&self, c: [i32; 3]) -> f64 {
        self.feet_16_fp(c, &Footprint::player()) as f64 / FULL_16 as f64
    }

    /// Whether the model floods `c` — water (or lava) a walker cannot stand in
    /// and a body can be put in. Public so the engagement proof
    /// ([`crate::compiler::engage`], `DW0920`) can ask whether the party can put its
    /// feet in water within a fight's reach; the one flood this model carries,
    /// never a second reading of the block map.
    pub fn is_flooded(&self, c: [i32; 3]) -> bool {
        self.flooded.contains(&c)
    }

    /// Whether a cell is unoccupied — neither a solid block nor water-flooded, so a
    /// camera eye placed in it sees open air rather than the inside of a block.
    /// Public wrapper for the visual-tier clear-eye self-check
    /// ([`verify_camera_eyes`]).
    pub fn is_clear(&self, c: [i32; 3]) -> bool {
        !self.is_occupied(c)
    }

    /// The top face of the **full-cube-class solid** occupying `c`, in sixteenths
    /// of a block, or `None` when no such block is there. A bottom slab answers
    /// `8`, a `dirt_path` `15`, a plain stone `16` — i.e. exactly the collision
    /// volume `c.y ..= c.y + top/16`, honouring the partial-floor table.
    /// Public so the body-clearance proof ([`crate::compiler::clearance`]) can
    /// intersect a real entity AABB against real block volumes rather than
    /// against whole cells.
    pub fn solid_top_16(&self, c: [i32; 3]) -> Option<u8> {
        if !self.solid.contains(&c) {
            return None;
        }
        Some(
            self.partial
                .get(&c)
                .copied()
                .unwrap_or(crate::compiler::assembled::FULL_HEIGHT_16),
        )
    }

    /// Whether `c` holds a **1.5-block-tall barrier** — a fence, a wall, or a
    /// closed fence gate ([`crate::compiler::assembled::is_tall_barrier`] /
    /// [`crate::compiler::assembled::is_fence_gate`]). Its collision volume rises
    /// [`BARRIER_HEIGHT`] from the cell floor but is a narrow post/panel
    /// horizontally, which is why the clearance proof treats it as advisory
    /// rather than as a wall.
    pub fn is_barrier(&self, c: [i32; 3]) -> bool {
        self.tall.contains(&c) || self.use_gates.contains(&c)
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
        self.flood_walkable(starts.iter().filter_map(|&s| self.snap(s, SNAP_RADIUS)))
    }

    /// [`World::reachable_walkable`] with each root's **seating** confined to the
    /// AABB that declares it ([`AnchorRoot`]). Only the snap is confined; the walk
    /// itself is unbounded, because a player who reaches a room does reach what it
    /// connects to.
    pub fn reachable_walkable_rooted(&self, roots: &[AnchorRoot]) -> BTreeSet<[i32; 3]> {
        self.flood_walkable(roots.iter().filter_map(|r| self.seat(r)))
    }

    /// Where a root actually puts a walker: the nearest standable cell to
    /// `root.at` **inside `root.within`**. `None` when the declaring piece offers
    /// no footing within [`SNAP_RADIUS`] — a root that seats nowhere contributes
    /// nothing, exactly as an unsnappable start does.
    fn seat(&self, root: &AnchorRoot) -> Option<[i32; 3]> {
        let (lo, hi) = root.within;
        self.snap_in_bounds(root.at, SNAP_RADIUS, &|c| {
            (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i])
        })
    }

    /// The walk closure of already-seated cells: deterministic BFS over a
    /// `BTreeSet` frontier with the fixed neighbour order (ADR-0006). The one flood
    /// both [`World::reachable_walkable`] and [`World::reachable_walkable_rooted`]
    /// run, so confining a root changes *where the walk starts* and nothing else.
    fn flood_walkable(&self, seats: impl IntoIterator<Item = [i32; 3]>) -> BTreeSet<[i32; 3]> {
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut queue: std::collections::VecDeque<[i32; 3]> = std::collections::VecDeque::new();
        for cell in seats {
            if seen.insert(cell) {
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

    /// The standable cells confined to the AABB `bounds`, reachable by a walk from
    /// `anchor` (snapped to the nearest standable cell inside `bounds`), returned in
    /// ascending BFS step-distance order from that start with a fixed `(y, z, x)`
    /// tie-break. Seats spawn-wave mobs on validated footing near their anchor
    /// `bounds` is the anchor's own assembled piece, so the flood-fill
    /// never leaves that room even where a mated socket is open air — a wave
    /// cannot string its mobs across a socket seam into the neighbouring piece,
    /// which is how a flock ends up spread toward void. Empty
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

    /// The **aggro ring**: standable cells inside `bounds`, walk-reachable from
    /// `anchor`, at a straight-line distance in `[radius - tolerance, radius]`
    /// from the anchor's snapped cell — and able to see it.
    ///
    /// This is the placement model for `summon: aggro-edge` (spec-0016 §6): a
    /// non-raider wave materializes at the boundary of its own perception, so it
    /// acquires a target the instant it exists and closes under pure native AI.
    ///
    /// The band is deliberately one-sided — **at or just inside `radius`, never
    /// beyond it**. A cell one block outside the mob's own `follow_range` looks
    /// identical on a map and is a different mechanic entirely: the mob spawns,
    /// perceives nobody, and stands there until a player walks closer. The
    /// spec's "±tolerance" reading admits that cell; this does not, and stricter
    /// is the only safe direction for a rule whose failure is silent.
    ///
    /// Reachability is inherited from [`World::confined_standable_cells`] — a mob
    /// summoned into a sealed pocket at the right distance would never arrive —
    /// and line-of-sight is what makes the ring an *aggro* ring rather than a
    /// circle of coordinates: vanilla's nearest-attackable-target goal is
    /// sight-gated, so a cell that cannot see the defended point summons a mob
    /// that stands there.
    ///
    /// Deterministic (ADR-0006): integer squared distances throughout, ordered
    /// **outermost first** — the edge of perception is where the fiction puts
    /// them — with a fixed `(-d², y, z, x)` tie-break. `Vec` order is the summon
    /// order.
    pub fn annulus_standable_cells(
        &self,
        anchor: [i32; 3],
        bounds: ([i32; 3], [i32; 3]),
        radius: f64,
        tolerance: f64,
    ) -> Vec<[i32; 3]> {
        let Some(centre) = self.ring_centre(anchor, bounds) else {
            return Vec::new();
        };
        let lo_d = (radius - tolerance).max(0.0);
        let (lo2, hi2) = (lo_d * lo_d, radius * radius);
        let mut ring: Vec<(i64, [i32; 3])> = self
            .confined_standable_cells(anchor, bounds)
            .into_iter()
            .filter_map(|c| {
                let d2: i64 = (0..3)
                    .map(|i| i64::from(c[i] - centre[i]).pow(2))
                    .sum::<i64>();
                let d2f = d2 as f64;
                (d2f >= lo2 && d2f <= hi2 && self.has_line_of_sight(c, centre)).then_some((d2, c))
            })
            .collect();
        ring.sort_by_key(|(d2, c)| (-*d2, c[1], c[2], c[0]));
        ring.into_iter().map(|(_, c)| c).collect()
    }

    /// The cell an aggro ring is measured from: the defended anchor snapped to
    /// standable footing inside `bounds`. A defended point is usually an
    /// affordance the party stands *around* (a fire, a totem, a heart), so the
    /// raw anchor cell is routinely solid. Public because the generated PackTest
    /// asserts ring distance against exactly this centre — the runtime assertion
    /// and the compile-time placement must measure from the same origin or the
    /// test proves nothing about the mechanic.
    pub fn ring_centre(&self, anchor: [i32; 3], bounds: ([i32; 3], [i32; 3])) -> Option<[i32; 3]> {
        let (lo, hi) = bounds;
        let in_bounds = |c: [i32; 3]| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]);
        self.snap_in_bounds(anchor, SNAP_RADIUS, &in_bounds)
    }

    /// Whether a standing entity at cell `a` can see one at cell `b`: the segment
    /// between their eye points (1.5 blocks above each cell's floor, the vanilla
    /// mob eye height) crosses no camera-blocking geometry. Reuses the cutscene
    /// clip traversal, so "can this be seen through" has exactly one definition in
    /// the compiler.
    pub(crate) fn has_line_of_sight(&self, a: [i32; 3], b: [i32; 3]) -> bool {
        let eye = |c: [i32; 3]| {
            let p = cell_center(c);
            [p[0], p[1] + 1.5, p[2]]
        };
        walk_cells(eye(a), eye(b), |c| self.blocks_camera(c)).is_none()
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
    /// take the nearest OTHER standable cell. Flooded cells are already
    /// excluded (they are not standable), so a shore NPC never resolves onto a
    /// water-tongue cell.
    pub(in crate::compiler::nav) fn snap_endpoint(
        &self,
        c: [i32; 3],
        off_cell: bool,
    ) -> Option<[i32; 3]> {
        if off_cell {
            self.snap_in_bounds(c, SNAP_RADIUS, &|n| n != c)
        } else {
            self.snap_standable(c, SNAP_RADIUS)
        }
    }

    pub(in crate::compiler::nav) fn is_solid(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c)
    }

    /// Whether a cell contains block geometry a cutscene camera must not fly
    /// through: a full-cube solid, a 1.5-tall fence/wall, or a fence gate.
    /// Water does not clip a camera.
    ///
    /// **Public**: a declared sightline (`DW0821`) asks exactly this question of
    /// exactly these cells — whether a line of sight is stopped by geometry —
    /// and a second predicate for it would be a second opinion about what a
    /// fence does to a view.
    pub fn blocks_camera(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c) || self.tall.contains(&c) || self.use_gates.contains(&c)
    }

    /// Whether a cell is occupied — a solid block, a 1.5-tall barrier (fence /
    /// wall), **or** flooded by water. An occupied cell
    /// cannot hold a walker's feet or head, and cannot be jumped through. Water
    /// blocks passage but, unlike a solid, is never a floor; a tall barrier
    /// likewise blocks passage but is never a floor (not standable on top). A
    /// use-gate cell is deliberately NOT occupied here: the player passes it with
    /// a right-click (walkers that cannot are routed on
    /// [`World::without_gate_use`]).
    pub(in crate::compiler::nav) fn is_occupied(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c)
            || self.tall.contains(&c)
            || self.flooded.contains(&c)
            || self.lethal.contains(&c)
    }

    /// Whether a cell is a valid standing position: the feet-cell and the
    /// head-cell above it are both passable (neither solid nor flooded), with
    /// **solid** ground directly below (an entity is 2 blocks tall and needs a
    /// floor — a water surface is not standable, so the floor must be solid, not
    /// merely occupied).
    pub(in crate::compiler::nav) fn standable(&self, c: [i32; 3]) -> bool {
        self.standable_fp(c, &Footprint::player())
    }

    /// Footprint-aware standability (spec-0014): every occupied column has its
    /// `height` feet+body cells passable with solid floor directly below. For the
    /// player footprint (single column, 2 tall) this is exactly the pre-0.6 rule.
    ///
    /// …plus the one question the columns cannot state, because they are whole
    /// cells and a hitbox is not: a body may not stand where a declared lethal
    /// volume's selector would reach it ([`World::meets_lethal_fp`]). It is asked
    /// here, at the one predicate every route proof, snap, flood and export in
    /// this model funnels through, rather than at any of them.
    pub(in crate::compiler::nav) fn standable_fp(&self, c: [i32; 3], fp: &Footprint) -> bool {
        if self.meets_lethal_fp(c, fp) {
            return false;
        }
        // spec-0065: a body may not be PROVEN to stand on furniture. Asked beside
        // the lethal clause, in the same predicate, so every route, snap, flood,
        // seat and export inherits it and none of them restates it.
        if self.on_furniture_fp(c, fp) {
            return false;
        }
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
    pub(in crate::compiler::nav) fn snap_standable(
        &self,
        c: [i32; 3],
        radius: i32,
    ) -> Option<[i32; 3]> {
        self.snap_standable_fp(c, radius, &Footprint::player())
    }

    /// Footprint-aware nearest-standable snap (spec-0014), used by `move-actor`
    /// endpoint resolution so a wide/tall puppet snaps to a cell IT can stand on.
    pub fn snap_standable_fp(&self, c: [i32; 3], radius: i32, fp: &Footprint) -> Option<[i32; 3]> {
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

    /// The walkable top face of the block directly below cell `c`, in sixteenths
    /// of a block above that block's own cell floor (16 = a full cube).
    fn floor_top_16(&self, support: [i32; 3]) -> i64 {
        self.partial
            .get(&support)
            .copied()
            .unwrap_or(crate::compiler::assembled::FULL_HEIGHT_16) as i64
    }

    /// The **true feet height** of a walker standing in cell `c`, in sixteenths of
    /// a block (absolute, so two standing cells can be differenced directly).
    ///
    /// The standing-cell convention is unchanged — the feet cell is the cell above
    /// the support — but the height it denotes is no longer assumed to be the cell
    /// floor: standing on a bottom slab puts the feet at `y - 0.5`, not `y`
    /// — a bottom slab puts them half a block down. For a multi-column footprint
    /// the walker rests on the **highest**
    /// supporting face, as vanilla's AABB does.
    pub(in crate::compiler::nav) fn feet_16_fp(&self, c: [i32; 3], fp: &Footprint) -> i64 {
        let base = (c[1] as i64 - 1) * FULL_16;
        fp.cols
            .iter()
            .map(|&[dx, dz]| base + self.floor_top_16([c[0] + dx, c[1] - 1, c[2] + dz]))
            .max()
            .unwrap_or(base + FULL_16)
    }

    /// Standable cardinal neighbours of `c`, allowing a one-cell step up or down.
    /// Fixed order for determinism.
    ///
    /// A step **up** past the auto-step budget is a jump: the entity's head sweeps
    /// through the cell `height` above its feet at the source, so that cell must be
    /// clear or it head-bonks and the move is physically impossible (a mineflayer
    /// bot refuses it with "No path to the goal!"). Modelling that jump-clearance
    /// here — not just the destination's standability — keeps a routed/exported
    /// path actually walkable: an assembled seam that ramps up under a low ceiling
    /// becomes a `DW0311` build error instead of a runtime strand on geometry the
    /// compiler wrongly "proved" connected.
    ///
    /// **Public**, and the widening is the point rather than a convenience: the
    /// step rule is the engine's ONE answer to "can a body get from here to
    /// there", and the stage-5 blockout battery (`crate::compiler::blockout`) asks that
    /// question of a whole map — is any two places' geometry joined anywhere the
    /// site plan did not allocate a seam (`DW0838`). A private rule leaves that
    /// battery with nothing to reuse and a hand-rolled step rule to write, which
    /// is `CLAUDE.md`'s second review shape: a general mechanism privately
    /// re-implemented, working perfectly, and silently not the rule every other
    /// proof in this compiler is taken under.
    pub fn neighbors(&self, c: [i32; 3]) -> Vec<[i32; 3]> {
        self.neighbors_fp(c, &Footprint::player())
    }

    /// Footprint-aware standable neighbours (spec-0014), gated by the **physical
    /// rise** between the two standing surfaces rather than by cell adjacency:
    ///
    /// - rise ≤ [`delvewright_dsl::metrics::MAX_AUTO_STEP_16`] — a walk-up. No jump, so no headroom is
    ///   required above the source cell. This is what admits the step onto a bottom
    ///   slab under a low ceiling that the old full-cube rule wrongly refused.
    /// - rise ≤ [`delvewright_dsl::metrics::MAX_JUMP_RISE_16`] — a jump; the swept head cell must be clear.
    /// - anything higher is **impossible** and is refused. The load-bearing case:
    ///   standing on a bottom slab and "stepping" onto a full block one cell up is
    ///   a 1.5-block rise the old model proved as an ordinary `+1` step.
    ///
    /// Those three arms are [`step_allowed`], in `delvewright-dsl`, and are asked
    /// rather than restated: `delvec::schem`'s walk asks the same function of
    /// the same rise, so the admission gate and this router cannot answer the
    /// question differently.
    ///
    /// Vertical candidates stay `{0, -1, +1}` cells. A `+2`-cell move can be
    /// physically legal between two very thin floors, but leaving it out only ever
    /// *refuses* a route, never proves one — the safe direction.
    ///
    /// **And the climb** (spec-0099): the moves a body on a climbable makes on
    /// cue — up, down, off the side, over the top, in from beside
    /// ([`World::climb_moves_fp`]). A body holding on a climb in mid-air walks
    /// nowhere: from a climb cell that is not standable only the climb's own
    /// moves are taken. A world with no climbable answers exactly the walk.
    pub(in crate::compiler::nav) fn neighbors_fp(
        &self,
        c: [i32; 3],
        fp: &Footprint,
    ) -> Vec<[i32; 3]> {
        if !self.has_climbs() {
            return self.neighbors_walk_fp(c, fp);
        }
        let hanging = self.climb_cell_fp(c, fp) && !self.standable_fp(c, fp);
        let mut out = if hanging {
            Vec::new()
        } else {
            self.neighbors_walk_fp(c, fp)
        };
        for n in self.climb_moves_fp(c, fp) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }

    /// The **walk** half of [`World::neighbors_fp`]: standable cardinal
    /// neighbours under the step rule, in the fixed order. What a body standing
    /// on a floor does with its feet, and nothing a climb adds.
    pub(in crate::compiler::nav) fn neighbors_walk_fp(
        &self,
        c: [i32; 3],
        fp: &Footprint,
    ) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let head_clear_to_jump = fp
            .cols
            .iter()
            .all(|&[dx, dz]| !self.is_occupied([c[0] + dx, c[1] + fp.height, c[2] + dz]));
        let here = self.feet_16_fp(c, fp);
        let mut out = Vec::new();
        for (dx, dz) in HORIZ {
            for dy in [0i32, -1, 1] {
                let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                if !self.standable_fp(n, fp) {
                    continue;
                }
                let rise = self.feet_16_fp(n, fp) - here;
                // The rule itself is `dsl::metrics` — see `step_allowed`. What
                // stays here is the MEASUREMENT of the rise, which is this
                // model's own: real collision tops, and the footprint's highest
                // supporting face.
                if !step_allowed(rise, || head_clear_to_jump) {
                    continue;
                }
                out.push(n);
            }
        }
        out
    }

    /// **Everywhere a body in `c` can put itself in one movement** — the player's
    /// own movement over the assembled model, cardinal, player footprint,
    /// ordered and deduplicated (ADR-0006).
    ///
    /// A body is in one of two states, and `c` says which: **standing** in a
    /// standable cell, or **afloat** in a surface cell of water (a water cell with
    /// open air over it, [`World::is_water_surface`]).
    ///
    /// Standing, it can take the walk step ([`World::neighbors`]), and the two
    /// movements a route proof never takes and a player always can:
    ///
    /// - **Fall.** The neighbouring column is clear at the body's feet and head;
    ///   the body drops to the first thing that stops it. A floor counts when it
    ///   is standable (a lethal volume, a fence top are not) and no deeper than
    ///   the fall an unarmoured body survives; water catches it at any depth, and
    ///   the body floats at the surface it fell into.
    /// - **Jump.** The launch column is clear for the whole arc (feet to two cells
    ///   up, the measured headroom); each gap column is clear from the lower of the
    ///   launch and landing feet to that same top; the body lands on the first
    ///   floor below the arc in the landing column, and the gap is inside
    ///   [`delvewright_dsl::metrics::jump_max_gap`] for the rise. Requiring the whole band clear
    ///   refuses some arcs a body clears, never admits one it cannot.
    ///
    /// Walking into water whose surface is at or above the body's feet puts it
    /// afloat at that surface.
    ///
    /// Afloat, it can swim to a neighbouring surface cell at the same height, and
    /// **climb out** onto a neighbouring standable cell no higher than
    /// [`delvewright_dsl::metrics::WATER_CLIMB_OUT_RISE`] above the water it floats in — or step
    /// over a lower rim and fall. It cannot jump, and it does not dive: water it
    /// could leave only by swimming under something is water it cannot leave.
    /// Lava is never a place a body floats.
    ///
    /// A route proof may not use any of this: it must never prove a way the bot
    /// cannot walk on cue. A proof that asks **where a body can end up** must, or
    /// it cannot see the places a player jumps, falls or wades into — which is
    /// what [`DW_BODY_CANNOT_LEAVE`] asks, in both directions of the same relation.
    ///
    /// **Holding** on a climb is the fourth state (spec-0099): a body in a climb
    /// cell ([`World::climb_cell_fp`]) takes the climb's moves
    /// ([`World::climb_moves_fp`]), steps off sideways into a fall, and lets go
    /// at the bottom of a run that stands on nothing; and a body falling down a
    /// column is caught by the first climb cell it reaches within
    /// [`delvewright_dsl::metrics::climb_catch_fall_blocks`] — it holds there,
    /// because a player who sneaks holds still.
    ///
    /// Diagonal jumps are not moves here: a place a body reaches only by one is
    /// not seen, and a place it leaves only by one is seen as unleavable.
    pub fn body_moves(&self, c: [i32; 3]) -> Vec<[i32; 3]> {
        self.moves_of(c, &Footprint::player(), true)
    }

    /// **Where a body standing in `c` can step off into a fall it does not
    /// survive** (spec-0085 §6.2): the first neighbouring column, in the fixed
    /// cardinal order, whose feet and head cells beside `c` are clear, whose cell
    /// under them is no floor, and under which nothing arrests the fall within
    /// [`unarmoured_survivable_fall_blocks`] — or the first thing that does is
    /// lava. Water at any depth in range arrests it; so does any solid, tall
    /// barrier or use-gate block, a floor or not, because the question is whether
    /// the body survives the drop, never whether it can stand where it lands.
    /// A neighbouring cell that is lava, or whose floor is lava, is a step into
    /// it, and is answered the same way.
    ///
    /// Returns the neighbouring cell and whether what kills is lava (`true`) or
    /// the fall itself (`false`).
    ///
    /// `None` when every side of `c` is wall, floor, or a survivable drop. Asked
    /// of a world with its exclusions lifted, so a declared killing volume is not
    /// what this finds — the keep-out answers for those.
    pub fn fatal_step_off(&self, c: [i32; 3]) -> Option<([i32; 3], bool)> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let deepest = unarmoured_survivable_fall_blocks() as i32;
        for (dx, dz) in HORIZ {
            let side = [c[0] + dx, c[1], c[2] + dz];
            let head = [side[0], side[1] + 1, side[2]];
            let under = [side[0], side[1] - 1, side[2]];
            // A pool of lava let into the floor, or standing at the feet: one
            // step and the body is in it.
            if self.lava.contains(&side)
                || (!self.is_occupied(side)
                    && !self.is_occupied(head)
                    && self.lava.contains(&under))
            {
                return Some((side, true));
            }
            if self.is_occupied(side) || self.is_occupied(head) || self.use_gates.contains(&side) {
                continue;
            }
            let arrests = |y: i32| {
                let cell = [side[0], y, side[2]];
                self.is_occupied(cell) || self.use_gates.contains(&cell)
            };
            if arrests(c[1] - 1) {
                continue; // level ground beside: a walk, not a drop
            }
            match self.fall_from(side[0], side[2], c[1], deepest) {
                Some(lava) => return Some((side, lava)),
                None => continue,
            }
        }
        None
    }

    /// Whether a body that steps into column `(x, z)` with its feet at `feet`
    /// dies of it: `Some(true)` for lava, `Some(false)` for a fall no floor
    /// arrests within `deepest`, `None` when it lives.
    ///
    /// A climbable on the way down catches the body ([`World::catch_fp`]) — the
    /// game forgets the fall above it — and a body that does not hold on slides
    /// to the bottom of the run and falls on from there with a fresh count
    /// (spec-0099 §3.5). The blinded body this answers for does not sneak.
    fn fall_from(&self, x: i32, z: i32, feet: i32, deepest: i32) -> Option<bool> {
        let fp = Footprint::player();
        let arrests = |y: i32| {
            let cell = [x, y, z];
            self.is_occupied(cell) || self.use_gates.contains(&cell)
        };
        let mut feet = feet;
        loop {
            // The body's feet are at `feet`; a landing at cell y puts them at y + 1.
            let landing = ((feet - 1 - deepest)..=(feet - 1))
                .rev()
                .find(|&y| arrests(y));
            let caught = self.catch_fp(x, z, feet - 1, i64::from(feet) * FULL_16, &fp);
            match (landing, caught) {
                (_, Some(hold)) if landing.is_none_or(|y| y < hold[1]) => {
                    let bottom = self.climb_bottom(hold);
                    if arrests(bottom[1] - 1) {
                        return None; // the run stands on a floor
                    }
                    feet = bottom[1];
                }
                (Some(y), _) if self.lava.contains(&[x, y, z]) => return Some(true),
                (Some(_), _) => return None,
                (None, _) => return Some(false),
            }
        }
    }

    /// **Everywhere a mob in `c` can put itself in one movement** — the
    /// relation `DW0922` and `DW0923` flood from a wave's seats. It differs from
    /// [`World::body_moves`] in one respect only: a mob makes no gap jumps.
    /// Vanilla's ground pathfinder steps and jumps onto a neighbouring block up
    /// to one block higher and drops off edges, and never plans a leap across
    /// open air; so this is `body_moves` with the jump arc taken away, asked of
    /// the mob's own footprint instead of the player's. The walk, the fall and
    /// the afloat arms are the same arms.
    ///
    /// And one thing a mob does that the player relation leaves out: it stands
    /// on a **barrier top** ([`World::perch_feet_16`]) when it can step or jump
    /// up to one from where it is, walks along it, steps down off it and drops
    /// off it into the next column. Observed on the pinned server: a drowned
    /// stepped onto a floor lantern, jumped from its top onto a well's curb
    /// wall, walked along the curb over a shut fence gate and dropped into the
    /// water.
    pub fn mob_moves(&self, c: [i32; 3], fp: &Footprint) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let perched = self.perch_feet_16(c, fp);
        let mut out = match perched {
            Some(_) => Vec::new(),
            None => self.moves_of(c, fp, false),
        };
        if self.is_water_surface(c) || (perched.is_none() && !self.standable_fp(c, fp)) {
            return out;
        }
        let here = perched.unwrap_or_else(|| self.feet_16_fp(c, fp));
        // The head sweeps the cells over the body's top at the source for a jump.
        let top = c[1] + fp.height + i32::from(perched.is_some());
        let head_clear = || {
            fp.cols
                .iter()
                .all(|&[dx, dz]| !self.is_occupied([c[0] + dx, top, c[2] + dz]))
        };
        for (dx, dz) in HORIZ {
            for dy in [0i32, -1, 1] {
                let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                if let Some(feet) = self.perch_feet_16(n, fp)
                    && step_allowed(feet - here, head_clear)
                {
                    out.push(n);
                }
                if perched.is_some()
                    && self.standable_fp(n, fp)
                    && step_allowed(self.feet_16_fp(n, fp) - here, head_clear)
                {
                    out.push(n);
                }
            }
            if perched.is_some() {
                let (x1, z1) = (c[0] + dx, c[2] + dz);
                if (c[1]..=c[1] + 1).all(|y| !self.is_occupied([x1, y, z1]))
                    && let Some(n) = self.settle_fp(x1, z1, c[1] - 1, here, fp)
                {
                    out.push(n);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Where a body dropping down column `(x, z)` from `top` ends up: afloat at
    /// the first water it meets, or standing on the first floor, if that floor is
    /// standable and no deeper under `from` (its feet, in sixteenths) than a body
    /// survives. `None` when it lands in lava, a lethal volume, on something no
    /// body stands on, or nowhere at all. Nothing deeper than the survivable fall
    /// is a landing, and water that deep is not looked for: a fall that far into
    /// water is not counted as a way in.
    fn settle_fp(&self, x: i32, z: i32, top: i32, from: i64, fp: &Footprint) -> Option<[i32; 3]> {
        let deepest = unarmoured_survivable_fall_blocks() as i32;
        let bottom = from.div_euclid(FULL_16) as i32 - deepest - 1;
        let mut y = top;
        let catch = self.catch_fp(x, z, top, from, fp);
        while y >= bottom {
            let cell = [x, y, z];
            if catch == Some(cell) {
                return catch;
            }
            if self.is_occupied(cell) {
                if self.is_water_surface(cell) {
                    return Some(cell);
                }
                let n = [x, y + 1, z];
                return (self.standable_fp(n, fp)
                    && jump_max_gap(self.feet_16_fp(n, fp) - from).is_some())
                .then_some(n);
            }
            y -= 1;
        }
        None
    }

    /// The feet of a body **perched on a barrier top** in `p` — standing on the
    /// fence, wall or shut gate in the cell under it, half a block into `p`, with
    /// room for its whole height over that — in sixteenths; `None` when `p` is
    /// not such a place. No body walks or jumps onto a barrier top from the floor
    /// beside it (a 1.5-block rise is beyond the jump), and no route proof stands
    /// one there; but a body that is already higher — on a lantern, a step, a
    /// crate — jumps onto one and walks along it, which is how a wave climbs a
    /// well's curb (`DW0922`). Only the feet column's support is asked: a wider
    /// body overhangs a one-cell wall as it does in game.
    fn perch_feet_16(&self, p: [i32; 3], fp: &Footprint) -> Option<i64> {
        if !self.tall.contains(&[p[0], p[1] - 1, p[2]]) {
            return None;
        }
        let room = fp.cols.iter().all(|&[dx, dz]| {
            (0..=fp.height).all(|dy| !self.is_occupied([p[0] + dx, p[1] + dy, p[2] + dz]))
        });
        room.then(|| (i64::from(p[1]) - 1) * FULL_16 + (BARRIER_HEIGHT * FULL_16 as f64) as i64)
    }

    /// The one movement relation behind [`World::body_moves`] and
    /// [`World::mob_moves`]; `gap_jumps` is the only thing that differs.
    fn moves_of(&self, c: [i32; 3], fp: &Footprint, gap_jumps: bool) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let clear =
            |x: i32, y0: i32, y1: i32, z: i32| (y0..=y1).all(|y| !self.is_occupied([x, y, z]));
        let settle = |x: i32, z: i32, top: i32, from: i64| self.settle_fp(x, z, top, from, fp);
        let mut out = Vec::new();
        if self.is_water_surface(c) {
            for (dx, dz) in HORIZ {
                let (x1, z1) = (c[0] + dx, c[2] + dz);
                let n = [x1, c[1], z1];
                if self.is_water_surface(n) {
                    out.push(n);
                    continue;
                }
                // Climb out onto a ledge in reach of the water's top.
                for rise in (1..=WATER_CLIMB_OUT_RISE).rev() {
                    let up = [x1, c[1] + rise, z1];
                    if self.standable_fp(up, fp) && clear(c[0], c[1] + 1, c[1] + rise + 1, c[2]) {
                        out.push(up);
                        break;
                    }
                }
                // Or over a rim no higher than the water, and down whatever is there.
                if clear(x1, c[1], c[1] + 1, z1) {
                    let from = (i64::from(c[1]) + 1) * FULL_16;
                    if let Some(n) = settle(x1, z1, c[1] - 1, from) {
                        out.push(n);
                    }
                }
            }
            out.sort_unstable();
            out.dedup();
            return out;
        }
        out.extend(self.neighbors_fp(c, fp));
        let here = self.feet_16_fp(c, fp);
        let hanging = self.has_climbs() && self.climb_cell_fp(c, fp) && !self.standable_fp(c, fp);
        // Let go at the bottom of a run that stands on nothing, and drop.
        if hanging
            && !self.climb_cell_fp([c[0], c[1] - 1, c[2]], fp)
            && let Some(n) = settle(c[0], c[2], c[1] - 1, here)
        {
            out.push(n);
        }
        for (dx, dz) in HORIZ {
            let (x1, z1) = (c[0] + dx, c[2] + dz);
            // Wade in: water at the body's feet in the next column, rising to
            // a surface with air over it.
            if self.is_water([x1, c[1], z1])
                && !self.solid_at([x1, c[1] + 1, z1])
                && let Some(s) = self.surface_over([x1, c[1], z1])
            {
                out.push(s);
            }
            // Fall: step sideways into the next column and drop.
            if clear(x1, c[1], c[1] + 1, z1)
                && let Some(n) = settle(x1, z1, c[1] - 1, here)
            {
                out.push(n);
            }
            // Jump: launch headroom, then gaps of 1.. columns. A mob makes none,
            // and nor does a body holding on a climb — jump held there climbs.
            if !gap_jumps || hanging || !clear(c[0], c[1], c[1] + 2, c[2]) {
                continue;
            }
            let widest = JUMP_REACH.iter().map(|(_, g)| *g).max().unwrap_or(0) as i32;
            for gap in 1..=widest {
                let (lx, lz) = (c[0] + dx * (gap + 1), c[2] + dz * (gap + 1));
                // The arc crosses every gap column at launch height; one that is
                // blocked there stops this jump and every longer one.
                let (gx, gz) = (c[0] + dx * gap, c[2] + dz * gap);
                if !clear(gx, c[1], c[1] + 2, gz) {
                    break;
                }
                let Some(n) = settle(lx, lz, c[1] + 2, here) else {
                    continue;
                };
                // A landing afloat is judged as landing on the water's top.
                let landing_16 = if self.is_water_surface(n) {
                    (i64::from(n[1]) + 1) * FULL_16
                } else {
                    self.feet_16_fp(n, fp)
                };
                let Some(max_gap) = jump_max_gap(landing_16 - here) else {
                    continue;
                };
                if gap as u32 > max_gap {
                    continue;
                }
                let lo = n[1].min(c[1]);
                if (1..=gap).all(|i| clear(c[0] + dx * i, lo, c[1] + 2, c[2] + dz * i)) {
                    out.push(n);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Water, not lava: a flooded cell a body could float in.
    pub(in crate::compiler::nav) fn is_water(&self, c: [i32; 3]) -> bool {
        self.flooded.contains(&c) && !self.lava.contains(&c) && !self.lethal.contains(&c)
    }

    /// A **surface cell** of water: water with open air over it, where a body
    /// afloat keeps its head out.
    pub(in crate::compiler::nav) fn is_water_surface(&self, c: [i32; 3]) -> bool {
        self.is_water(c) && !self.is_occupied([c[0], c[1] + 1, c[2]])
    }

    /// The surface a body wading into the water at `c` rises to: up the column
    /// through water to the first cell with air over it, or `None` when the
    /// column is capped (water under a roof is water a body cannot breathe in).
    fn surface_over(&self, c: [i32; 3]) -> Option<[i32; 3]> {
        let mut y = c[1];
        while self.is_water([c[0], y, c[2]]) {
            if self.is_water_surface([c[0], y, c[2]]) {
                return Some([c[0], y, c[2]]);
            }
            y += 1;
        }
        None
    }

    /// Whether a body may walk the **straight horizontal segment** between the
    /// centres of two standing cells — the question [`smooth_walk`] asks to decide
    /// that the cells between them are not worth walking around.
    ///
    /// A* is four-connected, so every route it returns is a staircase of
    /// axis-aligned segments. Over a long open courtyard that reads as a machine
    /// tracing a grid rather than a person crossing a yard, and the repair is to
    /// drop the intermediate cells wherever the straight line between two of them
    /// is walkable in its own right. This is that test, and it is deliberately the
    /// SAME rule the route was proven under rather than a second, looser one:
    /// [`World::standable_fp`] per column, so the lethal-volume question
    /// ([`World::meets_lethal_fp`]) and the floor/headroom question are asked here
    /// exactly as A* asked them.
    ///
    /// What it demands, and each clause is a class of geometry the diagonal would
    /// otherwise cut through:
    ///
    /// * **Level, and only level.** Both endpoints stand at the same cell `y` AND
    ///   the same true feet height ([`World::feet_16_fp`]), and so does every
    ///   column swept between them. A step, a slab lip, a stair's edge or a drop
    ///   therefore ENDS a smoothed run and is rendered by the cardinal step shape
    ///   [`step_vertices`] was written for. Nothing about a rise is smoothed,
    ///   because nothing about a rise is a straight horizontal line.
    /// * **Every swept column, not every column the line passes through.** The
    ///   body is an AABB `width` across, not a point: off a cell centre it
    ///   straddles up to four columns, and a diagonal is off the centre almost
    ///   everywhere. The swept set is the Minkowski sum of the segment with that
    ///   box, computed exactly ([`segment_meets_cell_16`]) rather than sampled — a
    ///   sample grid can step over the sliver of a doorway jamb, and the corner of
    ///   a doorway is the exact place this must not be wrong.
    /// * **No use-gate column.** A closed fence gate is deliberately not
    ///   *occupied* ([`World::is_occupied`]) — the player opens it with a click —
    ///   so it is standable, and without this clause a diagonal could be routed
    ///   through a shut gate, or past the one the traversal proof recorded this
    ///   leg as using.
    ///
    /// **The width is the body that SHIPS, never the footprint that routed**, for
    /// the reason [`step_fold`] states: `move-npc` plans on the player footprint
    /// whatever the NPC wears, and a 0.9-wide body given the player's 0.6 would
    /// sweep a corridor narrower than itself.
    ///
    /// Determinism (ADR-0006): integer arithmetic throughout, in the same
    /// sixteenths [`World::feet_16_fp`] measures in. No float comparison decides
    /// whether a body may take a path.
    pub(in crate::compiler::nav) fn segment_walkable_fp(
        &self,
        a: [i32; 3],
        b: [i32; 3],
        fp: &Footprint,
        width: f64,
    ) -> bool {
        if a[1] != b[1] {
            return false;
        }
        let floor = self.feet_16_fp(a, fp);
        if self.feet_16_fp(b, fp) != floor {
            return false;
        }
        // Half the rendered hitbox, in sixteenths, rounded OUTWARD: a body swept
        // as slightly wider than it is refuses a diagonal it could have taken,
        // which is the safe direction; one swept as narrower clips.
        let half_16 = (width * (FULL_16 as f64) / 2.0).ceil() as i64;
        let centre = |c: [i32; 3]| {
            [
                c[0] as i64 * FULL_16 + FULL_16 / 2,
                c[2] as i64 * FULL_16 + FULL_16 / 2,
            ]
        };
        let (p0, p1) = (centre(a), centre(b));
        // Candidate columns: the segment's own span grown by the half-width. The
        // exact test below then culls the corners of that box.
        let pad = (half_16 / FULL_16) as i32 + 1;
        for cx in (a[0].min(b[0]) - pad)..=(a[0].max(b[0]) + pad) {
            for cz in (a[2].min(b[2]) - pad)..=(a[2].max(b[2]) + pad) {
                if !segment_meets_cell_16(p0, p1, [cx as i64, cz as i64], half_16) {
                    continue;
                }
                let col = [cx, a[1], cz];
                if self.is_use_gate(col)
                    || !self.standable_fp(col, fp)
                    || self.feet_16_fp(col, fp) != floor
                {
                    return false;
                }
            }
        }
        true
    }
}

/// The cost of one perfectly **flat** cardinal step, in sixteenths of a block of
/// level walking. Every A* cost is denominated in this unit so the elevation
/// penalty below can be expressed in the same currency as horizontal distance
/// (and so the whole cost function stays integer — ADR-0006 forbids float
/// comparisons deciding a path).
pub(in crate::compiler::nav) const STEP_COST_16: u32 = FULL_16 as u32;

/// What one block of **elevation change** costs, expressed as a multiple of the
/// same distance walked on the flat (round-8 owner playtest).
///
/// The defect this fixes: with a distance-only cost, every route of equal length
/// is equally good, so the planner walked the herd and the giant along the
/// straight line over the greenfield's bumpy 1-step terrain — bobbing up and down
/// a block a dozen times — while the flat cleared road two columns to the side
/// cost the same 2-step detour it always did and never won. Staged walks are
/// *photographed*: a body that pogos over lumps reads as broken even though every
/// step is legal, and the built road exists precisely to be walked.
///
/// **Why 2.** A rise past [`delvewright_dsl::metrics::MAX_AUTO_STEP_16`] is a jump, and vanilla's jump arc
/// is ≈12 ticks airborne against ≈4.6 ticks to walk one block on the flat — so
/// clearing a 1-block rise really does cost about 2.5 blocks of walking time.
/// Two is the integer under that: enough that the planner pays a genuine detour
/// to stay level (a 1-block bump must be worth ~2 blocks of going around), but
/// not so much that it invents long absurd circuits to dodge a single step. It is
/// deliberately *under* the physical figure — the safe direction, since
/// overpaying for flatness is what would distort routes on legitimately sloped
/// terrain (the mountain ramp, the beach grade).
///
/// Measured on the island (round 8): the beach→pen walk crossed the greenfield at
/// `x=7` with 24 blocks of cumulative elevation change; at weight 2 it moves onto
/// the built path spine (`x=9..11`, flat at `y=63`) and the bobbing is gone. The
/// weight is applied per sixteenth, so a slab or a `dirt_path` lip (a 1/16-15/16
/// partial floor) costs proportionally less than a full block — the planner is
/// not driven off intentional slab stairs by the same rule that keeps it off
/// lumpy ground.
pub(in crate::compiler::nav) const ELEV_WEIGHT: u32 = 2;

/// The cost of stepping between two standing surfaces whose **true feet heights**
/// (sixteenths, absolute — [`World::feet_16_fp`]) are `from` and `to`: one flat
/// step plus [`ELEV_WEIGHT`] per sixteenth of height change, up or down.
///
/// Up and down are charged alike: the owner's complaint is *bobbing*, and a path
/// that drops a block only to climb it back is exactly as ugly as the reverse.
/// Never zero, so the heuristic stays admissible and A* terminates.
pub(in crate::compiler::nav) fn step_cost_16(from: i64, to: i64) -> u32 {
    STEP_COST_16 + (to - from).unsigned_abs() as u32 * ELEV_WEIGHT
}

/// The world position an entity standing in cell `c` occupies: the **horizontal
/// centre** of the cell, on its floor.
///
/// A Minecraft block cell `(x, y, z)` spans `[x, x+1)` on each horizontal axis, and
/// an entity's position is the centre of its AABB. Emitting the bare integer cell
/// coordinate therefore parks the body on the *corner* where four columns meet: a
/// 0.6-wide villager at `x = 7.0` spans `[6.7, 7.3]`, i.e. 70 % of it sits inside
/// column 6 — inside the wall, whenever the proven-walkable cell is 7. That is the
/// owner's "the NPC visibly passes through blocks" defect (island QA: 234 of 385
/// waypoints on the beach→cave walk had the body AABB inside a solid). The `+0.5`
/// is the whole fix: on a cardinal path through cell centres the AABB stays inside
/// the proven-walkable columns.
///
/// This is the single conversion for **every entity the compiler places or moves**;
/// block-targeting commands (`setblock`/`fill`/`place`/`spawnpoint`) keep integer
/// cell coordinates, which is what they take.
pub fn cell_center(c: [i32; 3]) -> [f64; 3] {
    [c[0] as f64 + 0.5, c[1] as f64, c[2] as f64 + 0.5]
}

/// Whether the segment `p0`→`p1` comes within `half_16` of the unit column
/// `[cx, cz]` — i.e. whether a box `2 * half_16` across, carried along that
/// segment, ever overlaps that column. All coordinates are in sixteenths of a
/// block.
///
/// This is the separating-axis test for a segment against an axis-aligned box,
/// which for a 2-D segment is complete on exactly three axes: the box's two, and
/// the segment's own normal (its direction axis cannot separate, since the box's
/// two axes already bound the segment's extent along it). The box tested is the
/// column grown by `half_16` on every side, which is the standard Minkowski
/// restatement of "a box of that half-width, swept, touches this column".
///
/// The intervals are **closed**, so a body whose hitbox exactly grazes a column
/// counts as entering it. That is the conservative direction: it can only refuse
/// a diagonal, never admit one.
///
/// Integer throughout (ADR-0006). The products are bounded by segment length
/// times world coordinate, both in sixteenths — nowhere near `i64`.
fn segment_meets_cell_16(p0: [i64; 2], p1: [i64; 2], cell: [i64; 2], half_16: i64) -> bool {
    let min = [cell[0] * FULL_16 - half_16, cell[1] * FULL_16 - half_16];
    let max = [
        (cell[0] + 1) * FULL_16 + half_16,
        (cell[1] + 1) * FULL_16 + half_16,
    ];
    // The box's own two axes.
    for k in 0..2 {
        if p0[k].max(p1[k]) < min[k] || p0[k].min(p1[k]) > max[k] {
            return false;
        }
    }
    // The segment's normal. Both endpoints project onto it identically (the
    // normal is perpendicular to the segment), so the segment is a point here and
    // the box is an interval.
    let n = [-(p1[1] - p0[1]), p1[0] - p0[0]];
    let s = n[0] * p0[0] + n[1] * p0[1];
    let corners = [
        n[0] * min[0] + n[1] * min[1],
        n[0] * min[0] + n[1] * max[1],
        n[0] * max[0] + n[1] * min[1],
        n[0] * max[0] + n[1] * max[1],
    ];
    let lo = corners.iter().copied().min().unwrap();
    let hi = corners.iter().copied().max().unwrap();
    s >= lo && s <= hi
}

/// Walk every unit cell the segment `a → b` passes through, in order, returning
/// the first for which `hit` holds. Amanatides–Woo voxel traversal: from the
/// starting cell, repeatedly advance along whichever axis reaches its next cell
/// boundary soonest. An axis with zero delta never steps (its `t_max` is
/// infinite). Both endpoint cells are included.
///
/// **Public**, and `FnMut` rather than `Fn`: the camera clip wants the FIRST
/// blocking cell and stops, while a blocked sightline (`DW0821`) owes its reader
/// EVERY blocking cell, because a walk sheet that names one cell of a wall has
/// not told anybody where the wall is. One traversal, two questions, and the
/// difference lives entirely in the closure.
pub fn walk_cells(
    a: [f64; 3],
    b: [f64; 3],
    mut hit: impl FnMut([i32; 3]) -> bool,
) -> Option<[i32; 3]> {
    let mut cell = [
        a[0].floor() as i32,
        a[1].floor() as i32,
        a[2].floor() as i32,
    ];
    let end = [
        b[0].floor() as i32,
        b[1].floor() as i32,
        b[2].floor() as i32,
    ];
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let mut step = [0i32; 3];
    // `t` is the fraction of the segment consumed; `t_max[i]` is the fraction at
    // which the next boundary on axis `i` is crossed, `t_delta[i]` the fraction one
    // whole cell costs on that axis.
    let mut t_max = [f64::INFINITY; 3];
    let mut t_delta = [f64::INFINITY; 3];
    for i in 0..3 {
        if d[i] > 0.0 {
            step[i] = 1;
            t_max[i] = ((cell[i] + 1) as f64 - a[i]) / d[i];
            t_delta[i] = 1.0 / d[i];
        } else if d[i] < 0.0 {
            step[i] = -1;
            t_max[i] = (cell[i] as f64 - a[i]) / d[i];
            t_delta[i] = -1.0 / d[i];
        }
    }
    if hit(cell) {
        return Some(cell);
    }
    // A segment crosses at most |Δcell| boundaries per axis; the bound makes the
    // loop provably terminating even against a degenerate (NaN-free) input.
    let budget: i64 = (0..3)
        .map(|i| (end[i] - cell[i]).unsigned_abs() as i64)
        .sum();
    for _ in 0..budget {
        // Advance on the axis whose next boundary comes soonest (fixed x, y, z
        // tie-break keeps a corner crossing deterministic).
        let axis = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
            0
        } else if t_max[1] <= t_max[2] {
            1
        } else {
            2
        };
        if t_max[axis] > 1.0 {
            break; // the next boundary lies past the segment's end
        }
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        if hit(cell) {
            return Some(cell);
        }
    }
    // The end cell is always tested, even if float error stopped the walk short.
    if cell != end && hit(end) {
        return Some(end);
    }
    None
}

// --- DW0355: stealth onset survivability ------------------------------------

/// Ticks a sprinting player needs to cross one block. Vanilla sprint is
/// 5.612 blocks/s = 0.2806 blocks/tick → 3.56 t/block; rounded **up** to 4 so the
/// model never credits the player with speed they do not have. (Sprint-jumping is
/// faster; the proof deliberately does not assume the player chains jumps.)
pub(in crate::compiler::nav) const SPRINT_TICKS_PER_BLOCK: u32 = 4;
