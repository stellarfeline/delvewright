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
//! - **`cutscene`** camera dollies are validated to pass only through non-solid
//!   blocks — both the authored waypoint polyline and the client-rendered
//!   keyframe chords ([`crate::camera`]). Cameras fly (exempt from walkability)
//!   but must not clip a solid; a violation is [`DW_CUTSCENE_CLIP`] (`DW0308`).
//!   Shots are also held to the angular-rate budget ([`DW_CAMERA_SPIN`],
//!   `DW0347`).
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
/// `DW0347`: a `cutscene` shot whose aim sweeps faster than the angular budget
/// ([`crate::camera::MAX_AIM_DEG_PER_TICK`], 6 °/tick = 120 °/s) — a pan that
/// fast at 20 Hz is nausea-tier and provably bad *before* it ships. Typical
/// cause: a `look_at` subject too close to a fast dolly. See the camera dossier
/// (`docs/notes/camera-dossier.md` §1) for the budget's derivation.
pub const DW_CAMERA_SPIN: &str = "DW0347";
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
/// `DW0378`: a `timed-gate` (spec-0016 §4) that is a coin flip rather than a
/// timing read — the set of entry phases from which a walking player clears the
/// span before the gate shuts covers **less than 20% of the cycle** (owner ruling
/// 2026-08-02). All-phase passability is explicitly NOT the requirement: a gate
/// that punishes bad timing is the point. A gate that punishes *every* timing is
/// not a skill check, it is a slot machine, and no amount of learning the level
/// makes it fair.
pub const DW_TIMED_GATE_COIN_FLIP: &str = "DW0378";
/// `DW0376`: an `ambush` (spec-0016 §3) with no counterplay — with every
/// ambusher standing where it will stand, no rest point (a checkpoint, a bonfire,
/// or the campaign entry) is walkable from the trigger cell any more. The player
/// is sealed in a pocket with the ambush and can only trade blows blind.
///
/// This is deliberately NOT a telegraph requirement. The un-telegraphed ambush is
/// core souls vocabulary (owner ruling 2026-08-02): dying uninformed once is how
/// the level teaches, and determinism guarantees the second attempt meets the same
/// ambushers in the same cells. What the engine owes the informed player is a
/// *play* — a retreat, luring ground, a positioning line — and that is what this
/// proves exists.
pub const DW_AMBUSH_NO_COUNTERPLAY: &str = "DW0376";
/// `DW0373`: a `shortcut` (spec-0016 §2) whose far-side `unlock` affordance is
/// not reachable while the gate is still sealed — the LONG route does not exist,
/// so the mechanism that opens the shortcut can never be pulled and the gate is
/// dead scenery. The whole pattern is "earn the far side the hard way, then open
/// the door forever"; without a hard way there is nothing to earn.
pub const DW_SHORTCUT_NO_LONG_ROUTE: &str = "DW0373";
/// `DW0374`: a `shortcut` (spec-0016 §2) that **leaks** — opening its gate does not
/// shorten the walk from the campaign entry to its own `unlock` affordance, so the
/// unlock is not on the far side of anything. The pattern is "earn the far side
/// the hard way, then open the door forever"; if the door is irrelevant to
/// reaching the mechanism that opens it, the loop-back moment — which IS the
/// design — never happens. The classic form is an `unlock` placed on the NEAR
/// side of its own gate.
pub const DW_SHORTCUT_NO_GAIN: &str = "DW0374";
/// `DW0327`: a `begin-stealth` (spec-0014) zone that is unstandable, or unreachable
/// from the player's position at the beat that activates the stealth check.
pub const DW_STEALTH_ZONE: &str = "DW0327";
/// `DW0355`: a **punishing** `begin-stealth` whose grace window cannot be beaten —
/// from a position a player legally occupies the instant the beat arms (the
/// activating objective's anchor, or any checkpoint that can respawn them into the
/// running session), no zone is reachable within `grace_ticks` at sprint speed over
/// the assembled geometry. DW0327 proves cover *exists and is reachable*; this
/// proves it is reachable **in time**. Without it a beat that arms under the
/// player's feet at the most exposed cell in the room kills every player — machine
/// or human — a fixed couple of seconds later, and if the checkpoint it respawns
/// them at is also outside cover, the retry loop never terminates. A structurally
/// unavoidable death is not 初见杀 (spec-0016), it is a broken beat.
pub const DW_STEALTH_ONSET: &str = "DW0355";
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

/// The **world-generator ambient** the placed geometry sits in — what a column
/// the compiler modelled nothing into actually contains in the delivered world
/// (spec-0013 `horizon`).
///
/// The assembled model ([`crate::assembled`]) knows only cells a prefab, a socket
/// seal or an edit wrote. Everything else is "absent", and what *absent* means is
/// a property of the level generator, not of the content: under `horizon: void`
/// an absent column is bottomless; under `horizon: ocean` it is the pinned
/// bedrock/stone/water superflat, so there is no void anywhere in the world and
/// stepping off the land is swimming. Boundary safety ([`verify_boundary_safety`])
/// is the one proof whose premise is exactly this, so the ambient rides on the
/// [`World`] rather than being re-derived (or, as before, silently assumed to be
/// `Void`) at the call site.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Ambient {
    /// `horizon: void` (default/absent) — an empty superflat layer list. Every
    /// column the content did not build is bottomless.
    #[default]
    Void,
    /// `horizon: ocean` — the pinned bedrock/stone/water superflat ([`Sea`]).
    Ocean(Sea),
}

/// The ocean horizon's ambient sea: a global water plane topping out at
/// [`Sea::level`], solid ground from [`Sea::floor_top`] down, and air above —
/// present in **every** column except those a placed piece overwrote
/// ([`Sea::covered`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sea {
    /// Y of the topmost ambient water block (`crate::plan::SEA_LEVEL`, 62).
    pub level: i32,
    /// Y of the topmost ambient solid block — the sea floor
    /// (`crate::plan::SEA_FLOOR_TOP_Y`, 54). Ambient water occupies
    /// `floor_top+1 ..= level`.
    pub floor_top: i32,
    /// Inclusive world AABBs of every placed piece. `/place template` writes the
    /// whole box (air included), so inside a box the piece decides what is there
    /// and the ambient does not apply; outside every box (and below them —
    /// pieces sit *on* the sea, so the water under an island base is ambient) it
    /// does. Deterministic order: plan area order, entry piece first.
    pub covered: Vec<([i32; 3], [i32; 3])>,
}

impl Sea {
    /// Whether `c` falls inside a placed piece's AABB (piece-authored, so the
    /// ambient says nothing about it).
    fn covered(&self, c: [i32; 3]) -> bool {
        self.covered
            .iter()
            .any(|(lo, hi)| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]))
    }

    /// Whether `c` is ambient sea water: inside the generator's water layers and
    /// not overwritten by a placed piece.
    fn ambient_water(&self, c: [i32; 3]) -> bool {
        c[1] > self.floor_top && c[1] <= self.level && !self.covered(c)
    }
}

impl Ambient {
    /// The ambient a campaign's `horizon` declares (spec-0013), with the placed
    /// pieces of `plan` as the covered region.
    pub fn of_plan(plan: &Plan) -> Ambient {
        match plan.campaign.world.content.horizon {
            Some(delvewright_dsl::Horizon::Ocean) => Ambient::Ocean(Sea {
                level: crate::plan::SEA_LEVEL,
                floor_top: crate::plan::SEA_FLOOR_TOP_Y,
                covered: plan
                    .areas
                    .iter()
                    .flat_map(|a| a.pieces.iter().map(|p| p.bbox()))
                    .collect(),
            }),
            _ => Ambient::Void,
        }
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
///
/// Collision classes (task #59, [`crate::assembled::Occupancy`]): `solid` holds
/// only full-cube cells (passage-blocking AND valid floor); `tall` holds 1.5-tall
/// fence/wall cells (passage-blocking, **never** valid floor — a walking player
/// cannot jump 1.5, so a fence-top is not standable and the old full-solid model's
/// "stand on the fence" routes are gone); `use_gates` holds closed fence-gate
/// cells, passable for the **player** via an adventure-legal right-click (a
/// "use-gate" edge, exported to the harness) but impassable for NPC/actor/wave
/// walkers ([`World::without_gate_use`]) — and never valid floor either, so no
/// route stands on a gate-top. Because a tall/gate cell is never floor, the cell
/// above it has no footing, which also models the barrier's upper half blocking
/// walk-overs at `y+1` for free.
///
/// Partial floor heights (task #78): `partial` records, for a `solid` cell whose
/// walkable top face sits **below** the cell top (a bottom slab at 8/16, a snow
/// drift, a `dirt_path` at 15/16), that true height. It is what makes
/// [`World::neighbors_fp`] a physical step rule rather than a cell-adjacency rule
/// — see [`MAX_AUTO_STEP_16`] / [`MAX_JUMP_RISE_16`].
pub struct World {
    solid: BTreeSet<[i32; 3]>,
    tall: BTreeSet<[i32; 3]>,
    use_gates: BTreeSet<[i32; 3]>,
    flooded: BTreeSet<[i32; 3]>,
    /// For each `solid` cell whose walkable top face sits **below** the cell
    /// top, that height in sixteenths (task #78). Absent = a full cube. Feeds
    /// the physical step rule in [`World::neighbors_fp`].
    partial: BTreeMap<[i32; 3], u8>,
    /// What the *unmodelled* columns contain (spec-0013 `horizon`). Defaults to
    /// [`Ambient::Void`] — the pre-0.6 world and every synthetic test world —
    /// and is set from the plan by [`World::from_plan`] /
    /// [`World::with_ambient`]. Read only by [`verify_boundary_safety`]; it
    /// deliberately does **not** feed the walkability sets, so routing,
    /// standability and every other proof stay byte-identical.
    ambient: Ambient,
}

/// The largest rise, in sixteenths of a block, a walker crosses **without
/// jumping** — vanilla's player `maxUpStep` is 0.6 blocks, and 9/16 = 0.5625 is
/// the largest sixteenth under it (10/16 = 0.625 already needs a jump). A rise
/// within this budget needs no headroom above the *source* cell: the player walks
/// straight up onto a slab or a path edge.
const MAX_AUTO_STEP_16: i64 = 9;

/// The largest rise a walker can reach **by jumping**, in sixteenths. A vanilla
/// player's jump apex is ≈1.2522 blocks, so a surface 20/16 = 1.25 up is
/// reachable and 21/16 = 1.3125 is not. This is the bound that makes the
/// **1.5-block** slab-to-full-block step-up — which the old full-cube model
/// happily "proved" as an ordinary `+1` step — the impossible move it is.
const MAX_JUMP_RISE_16: i64 = 20;

/// A full block's height in sixteenths (mirrors
/// [`crate::assembled::FULL_HEIGHT_16`] in nav's `i64` step arithmetic).
const FULL_16: i64 = 16;

impl World {
    /// Build the occupancy model from the plan's placed pieces and the structure
    /// `.nbt` bytes, via the shared assembled-world model. Every non-air cell of
    /// that settled map is a solid cell here — so a `sand`/`gravel` floor that
    /// falls out of the void world is passable (a hole), exactly as in game
    /// (task #42), not a phantom floor the model wrongly seats mobs on.
    pub fn from_plan(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Self {
        Self::from_occupancy(crate::assembled::assembled_occupancy(plan, structures))
            .with_ambient(Ambient::of_plan(plan))
    }

    /// Build the walkability model from a collision-classified [`Occupancy`]
    /// (task #59) — the sets map across one-to-one.
    pub fn from_occupancy(occ: crate::assembled::Occupancy) -> Self {
        World {
            solid: occ.solid,
            tall: occ.tall,
            use_gates: occ.use_gates,
            flooded: occ.flooded,
            partial: occ.partial,
            ambient: Ambient::Void,
        }
    }

    /// This world with its world-generator [`Ambient`] declared (spec-0013). The
    /// occupancy sets are untouched — the ambient is a *premise*
    /// ([`verify_boundary_safety`]), not geometry.
    pub fn with_ambient(mut self, ambient: Ambient) -> Self {
        self.ambient = ambient;
        self
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
        // A sealed gate cell is a full-cube wall, never a partial floor.
        let mut partial = self.partial.clone();
        for c in extra {
            partial.remove(c);
        }
        World {
            solid,
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial,
            ambient: self.ambient.clone(),
        }
    }

    /// A copy of this world for **autonomous** walkers that cannot use gates —
    /// wave mobs seated at spawn (task #59). Opening a fence gate is a right-click
    /// USE, so for a mob acting on its own a closed gate is exactly a 1.5-tall
    /// fence: the use-gate cells are folded into the tall-barrier set, and the
    /// seating flood neither seats a mob in a gate threshold nor spills through
    /// one. Scripted walks (`move-npc` / `move-actor`) deliberately do NOT use
    /// this view — see [`plan_moves`]. A world with no use-gates is returned
    /// unchanged in content (call sites skip the clone via
    /// [`World::has_use_gates`]).
    pub fn without_gate_use(&self) -> World {
        let mut tall = self.tall.clone();
        tall.extend(self.use_gates.iter().copied());
        World {
            solid: self.solid.clone(),
            tall,
            use_gates: BTreeSet::new(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            ambient: self.ambient.clone(),
        }
    }

    /// Whether any closed fence-gate (use-gate) cell exists in this world.
    pub fn has_use_gates(&self) -> bool {
        !self.use_gates.is_empty()
    }

    /// Whether `c` is a closed fence-gate cell — a "use-gate": the player walks
    /// through it after an adventure-legal right-click (task #59). Exported per
    /// leg in the critical-path waypoint metadata so the harness knows the edge.
    pub fn is_use_gate(&self, c: [i32; 3]) -> bool {
        self.use_gates.contains(&c)
    }

    /// Build a [`World`] directly from a set of solid cells, with no water (test /
    /// synthetic entry point; the relight unit tests build a world without a full
    /// [`Plan`]).
    pub fn from_solid_cells(solid: BTreeSet<[i32; 3]>) -> Self {
        World {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
            ambient: Ambient::Void,
        }
    }

    /// Build a [`World`] directly from disjoint solid + flooded cell sets (test /
    /// synthetic entry point for the flood-aware standability rules).
    pub fn from_solid_and_flooded(solid: BTreeSet<[i32; 3]>, flooded: BTreeSet<[i32; 3]>) -> Self {
        World {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded,
            partial: BTreeMap::new(),
            ambient: Ambient::Void,
        }
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
    /// (spec-0010), and the cell set the lethal-trap proof calls "forced".
    ///
    /// Routed over the **causally-sealed** per-leg world, exactly like
    /// [`check_critical_path`] (task #78). Before that fix this walked the base
    /// open world while completability was proven under seals, so the two
    /// disagreed about which cells the player is actually forced across: a leg the
    /// player can only walk as a detour *because* a `close-gate` shut the direct
    /// route was routed through the (still open) gate here, and the trap proof
    /// then declared a lethal plate on the detour "avoidable" — a provable death
    /// the build shipped green.
    pub fn required_path_cells(&self, plan: &Plan, moves: &[MovePlan]) -> BTreeSet<[i32; 3]> {
        let mut cells: BTreeSet<[i32; 3]> = BTreeSet::new();
        // Critical-path legs.
        for leg in self.walked_legs(plan) {
            cells.extend(leg.cells);
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

    /// The proven A* cell route for every **walked** critical-path leg (transport
    /// hops skipped), each routed over that leg's causally-sealed world — the one
    /// leg model shared by [`check_critical_path`] (the completability proof),
    /// [`World::required_path_cells`] (relight + the lethal-trap forced-cell set)
    /// and [`critical_path_routes`] (the exported harness waypoints).
    ///
    /// Unifying these is task #78: the proof ran under `close-gate` seals while
    /// the trap analysis and the waypoint export ran over the open world, so the
    /// compiler could export a bot route through a gate the campaign had already
    /// sealed, and could call a trap on a forced detour "avoidable". A leg that
    /// fails to snap or route is omitted — it cannot occur once
    /// [`check_critical_path`] has passed, and before that the DW0311 error is the
    /// diagnostic that matters.
    fn walked_legs(&self, plan: &Plan) -> Vec<LegRoute> {
        self.walked_legs_sealed(plan)
            .into_iter()
            .map(|(leg, _)| leg)
            .collect()
    }

    /// [`World::walked_legs`], each leg paired with the gate cells sealed while the
    /// player walks it ([`leg_seal`]). The trap proof needs the seal itself, not
    /// just the route: a disarm affordance is only genuinely reachable "before the
    /// trap" if it is reachable under the gate state in force at that point.
    fn walked_legs_sealed(&self, plan: &Plan) -> Vec<(LegRoute, BTreeSet<[i32; 3]>)> {
        route_walked_legs(
            self,
            &critical_positions(plan),
            &plan.gate_events,
            &|g, s| plan.gate_fired_before(g, s),
        )
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

    /// Whether a cell contains block geometry a cutscene camera must not fly
    /// through: a full-cube solid, a 1.5-tall fence/wall, or a fence gate (task
    /// #59). Water does not clip a camera (pre-#45 behaviour preserved).
    fn blocks_camera(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c) || self.tall.contains(&c) || self.use_gates.contains(&c)
    }

    /// Whether a cell is occupied — a solid block, a 1.5-tall barrier (fence /
    /// wall; task #59), **or** flooded by water (task #45). An occupied cell
    /// cannot hold a walker's feet or head, and cannot be jumped through. Water
    /// blocks passage but, unlike a solid, is never a floor; a tall barrier
    /// likewise blocks passage but is never a floor (not standable on top). A
    /// use-gate cell is deliberately NOT occupied here: the player passes it with
    /// a right-click (walkers that cannot are routed on
    /// [`World::without_gate_use`]).
    fn is_occupied(&self, c: [i32; 3]) -> bool {
        self.solid.contains(&c) || self.tall.contains(&c) || self.flooded.contains(&c)
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

    /// The walkable top face of the block directly below cell `c`, in sixteenths
    /// of a block above that block's own cell floor (16 = a full cube).
    fn floor_top_16(&self, support: [i32; 3]) -> i64 {
        self.partial
            .get(&support)
            .copied()
            .unwrap_or(crate::assembled::FULL_HEIGHT_16) as i64
    }

    /// The **true feet height** of a walker standing in cell `c`, in sixteenths of
    /// a block (absolute, so two standing cells can be differenced directly).
    ///
    /// The standing-cell convention is unchanged — the feet cell is the cell above
    /// the support — but the height it denotes is no longer assumed to be the cell
    /// floor: standing on a bottom slab puts the feet at `y - 0.5`, not `y`
    /// (task #78). For a multi-column footprint the walker rests on the **highest**
    /// supporting face, as vanilla's AABB does.
    fn feet_16_fp(&self, c: [i32; 3], fp: &Footprint) -> i64 {
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
    /// compiler wrongly "proved" connected (task #38).
    fn neighbors(&self, c: [i32; 3]) -> Vec<[i32; 3]> {
        self.neighbors_fp(c, &Footprint::player())
    }

    /// Footprint-aware standable neighbours (spec-0014), gated by the **physical
    /// rise** between the two standing surfaces rather than by cell adjacency
    /// (task #78):
    ///
    /// - rise ≤ [`MAX_AUTO_STEP_16`] — a walk-up. No jump, so no headroom is
    ///   required above the source cell. This is what admits the step onto a bottom
    ///   slab under a low ceiling that the old full-cube rule wrongly refused.
    /// - rise ≤ [`MAX_JUMP_RISE_16`] — a jump; the swept head cell must be clear.
    /// - anything higher is **impossible** and is refused. The load-bearing case:
    ///   standing on a bottom slab and "stepping" onto a full block one cell up is
    ///   a 1.5-block rise the old model proved as an ordinary `+1` step.
    ///
    /// Vertical candidates stay `{0, -1, +1}` cells. A `+2`-cell move can be
    /// physically legal between two very thin floors, but leaving it out only ever
    /// *refuses* a route, never proves one — the safe direction.
    fn neighbors_fp(&self, c: [i32; 3], fp: &Footprint) -> Vec<[i32; 3]> {
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
                if rise > MAX_JUMP_RISE_16 {
                    continue; // above the jump apex: no player can make this step
                }
                if rise > MAX_AUTO_STEP_16 && !head_clear_to_jump {
                    continue; // needs a jump, and there is no room to jump here
                }
                out.push(n);
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

/// Resample a cell path into per-tick waypoints at `speed` blocks/tick along the
/// polyline through the cell centres ([`cell_center`]). Guarantees the final
/// waypoint is exactly the goal cell's centre and at least one step exists.
///
/// **Vertical steps are L-shaped, not diagonal.** A one-block step up inserts an
/// intermediate vertex directly above the source cell (rise in place, then cross at
/// the new height); a step down crosses at the source height, then drops. A straight
/// lerp between the two cell centres would sweep the body through the *corner* of
/// the step block — the same "inside the geometry" artifact at a stair that the
/// centring fixes along a wall. Both legs of the L stay inside cells the neighbour
/// rule already proved clear (`standable_fp` + the jump head-clearance check).
fn resample(cells: &[[i32; 3]], speed: f64) -> Vec<[f64; 3]> {
    let mut pts: Vec<[f64; 3]> = Vec::with_capacity(cells.len() * 2);
    for (i, c) in cells.iter().enumerate() {
        let p = cell_center(*c);
        if i > 0 {
            let prev = cells[i - 1];
            match c[1] - prev[1] {
                // step up: rise over the source column first, then cross.
                1 => pts.push([prev[0] as f64 + 0.5, c[1] as f64, prev[2] as f64 + 0.5]),
                // step down: cross at the source height first, then drop.
                -1 => pts.push([p[0], prev[1] as f64, p[2]]),
                _ => {}
            }
        }
        pts.push(p);
    }
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
/// Each NPC's successive moves **chain**: the first leg starts at the stage-2
/// anchor, every later leg at the previous leg's target (round-6; see
/// [`plan_actor_moves`]). Two moves sharing `(npc, to_anchor)` still share one
/// content-keyed driver, planned from the first occurrence's origin (documented
/// limitation of the content key).
///
/// **Use-gate cells are walkable edges here** (task #59): a scripted walk is a
/// compiler-emitted, supervised tp polyline fired by a campaign beat, and the
/// beat's fiction controls the gate (the island ram leaves its pen only after the
/// player has opened the pen gate to reach it). Routing through the openable
/// threshold is strictly more faithful than the old full-solid model, which
/// "proved" the same legs by hopping the body over a fence-top. Only autonomous
/// placement (wave seating) uses the no-gate-use view — a spawned mob really
/// cannot pass a closed gate on its own.
pub fn plan_moves(plan: &Plan, world: &World) -> Result<Vec<MovePlan>, NavError> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    // Chained origins (round-6): each NPC's next walk starts from its LAST staged
    // location — the previous move's (snapped) target — not its declared anchor.
    // Planning every leg from the declared anchor made a second consecutive
    // `move-npc` on the same NPC degenerate (worst case start == target → a
    // single-waypoint instant teleport instead of a walk). Keyed by npc id, in
    // campaign effect order (the same deterministic order the dedup uses).
    let mut chained_start: BTreeMap<String, [i32; 3]> = BTreeMap::new();
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
        let key = (npc.as_str().to_string(), to_anchor.as_str().to_string());
        if !seen.insert(key) {
            // Deduped (same content-keyed driver) — but the walk still ends here,
            // so the NPC's next leg chains from this target.
            chained_start.insert(npc.as_str().to_string(), target);
            continue;
        }
        let start = match chained_start.get(npc.as_str()) {
            Some(pos) => *pos,
            None => {
                let home = npc_start(plan, npc.as_str()).ok_or_else(|| NavError {
                    code: DW_MOVE_UNROUTABLE,
                    message: format!(
                        "move-npc: NPC `{}` has no resolved home anchor to walk from — give the \
                         npc a stage-2 `anchor` that its area's prefab provides, so the walk has \
                         a start",
                        npc.as_str()
                    ),
                })?;
                // The NPC walks up to a solid affordance, not into it: snap the
                // home endpoint to the floor cell nearest the anchor.
                world.snap_standable(home, SNAP_RADIUS).unwrap_or(home)
            }
        };
        let cells = world.find_path(start, target).ok_or_else(|| NavError {
            code: DW_MOVE_UNROUTABLE,
            message: format!(
                "move-npc: NPC `{}` cannot walk from its last staged location {start:?} (home \
                 anchor `{}`) to `{}` {anchor_pos:?} (floor {target:?}) — no collision-free path \
                 over the solved geometry. Route the move within one connected area (a \
                 wall/void/closed gate separates start and destination), or split it into shorter \
                 reachable hops",
                npc.as_str(),
                plan_npc_anchor(plan, npc.as_str()),
                to_anchor.as_str(),
            ),
        })?;
        chained_start.insert(npc.as_str().to_string(), target);
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
/// move is unroutable (names actor, leg, first blocked cell). Each actor's
/// successive moves **chain** — first leg from the declared spawn anchor, every
/// later leg from the previous leg's target (round-6 fix; see the loop comment).
/// Two moves sharing `(actor, to_anchor)` still share one content-keyed driver,
/// planned from the first occurrence's origin (documented limitation).
///
/// Use-gate cells are walkable edges for a scripted puppet walk, exactly as for
/// `move-npc` (see [`plan_moves`]): the island ram's pen→mouth leg crosses the pen
/// gate the player has just opened — through the threshold, no longer over the
/// fence-top the full-solid model wrongly proved (task #59).
pub fn plan_actor_moves(plan: &Plan, world: &World) -> Result<Vec<ActorMovePlan>, NavError> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    // Chained origins (round-6, live-server proven): a SECOND consecutive
    // `move-actor` on the same actor must start from the actor's CURRENT staged
    // location — the previous move's (snapped) target — not its declared spawn
    // anchor. Planning every leg from the declared anchor degenerated the
    // island's t=260 mouth→fire-pit walk into a single-waypoint instant teleport
    // (start == declared anchor == target), so the giant snapped instead of
    // walking on camera. Keyed by actor id, in campaign effect order (the same
    // deterministic order the dedup uses).
    let mut chained_start: BTreeMap<String, [i32; 3]> = BTreeMap::new();
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
        let a = actor_of(plan, actor.as_str()).ok_or_else(|| NavError {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: unknown actor `{}` — declare it in the stage-5 `actors` list",
                actor.as_str()
            ),
        })?;
        let fp = entity_footprint(&a.entity);
        let dest = actor_anchor_pos(plan, to_anchor.as_str()).ok_or_else(|| NavError {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: destination anchor `{}` for actor `{}` did not resolve to a world \
                 position — use a `to_anchor` some area's prefab provides",
                to_anchor.as_str(),
                actor.as_str()
            ),
        })?;
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
        let key = (actor.as_str().to_string(), to_anchor.as_str().to_string());
        if !seen.insert(key) {
            // Deduped (same content-keyed driver) — but the walk still ends here,
            // so the actor's next leg chains from this target.
            chained_start.insert(actor.as_str().to_string(), target);
            continue;
        }
        let start = match chained_start.get(actor.as_str()) {
            Some(pos) => *pos,
            None => {
                let start_anchor =
                    actor_anchor_pos(plan, a.anchor.as_str()).ok_or_else(|| NavError {
                        code: DW_ACTOR_UNROUTABLE,
                        message: format!(
                            "move-actor: actor `{}` spawn anchor `{}` did not resolve to a world \
                             position — use a spawn `anchor` some area's prefab provides",
                            actor.as_str(),
                            a.anchor.as_str()
                        ),
                    })?;
                world
                    .snap_standable_fp(start_anchor, SNAP_RADIUS, &fp)
                    .unwrap_or(start_anchor)
            }
        };
        let cells = world.find_path_fp(start, target, &fp).ok_or_else(|| {
            let blocked = first_blocked_fp(world, start, target, &fp);
            NavError {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "move-actor: actor `{}` ({}) cannot walk the leg {start:?} (last staged \
                     location; spawn anchor `{}`) → `{}` {target:?} — no collision-free path for \
                     its footprint over the assembled geometry (first blocked cell ~{blocked:?}). \
                     Route the move within one connected area, widen the corridor/ceiling for \
                     this mob, or split it into shorter reachable hops",
                    actor.as_str(),
                    a.entity,
                    a.anchor.as_str(),
                    to_anchor.as_str(),
                ),
            }
        })?;
        chained_start.insert(actor.as_str().to_string(), target);
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
        .map(|w| anchor_offset_point(plan, w.anchor.as_str(), w.offset))
        .collect()
}

/// The world point a cutscene's `look_at` subject resolves to (DSL v0.6) — the
/// same anchor + offset block-centre convention as [`camera_points`], so a
/// waypoint and a look target at the same anchor/offset name the same point.
pub fn camera_look_point(plan: &Plan, target: &delvewright_dsl::CameraTarget) -> [f64; 3] {
    anchor_offset_point(plan, target.anchor.as_str(), target.offset)
}

/// Resolve `anchor + offset` to a block-centre world point (the shared cutscene
/// camera convention). An unresolved anchor falls back to the layout origin —
/// referential validation reports it separately.
pub(crate) fn anchor_offset_point(plan: &Plan, anchor: &str, offset: [i32; 3]) -> [f64; 3] {
    let base = plan
        .anchors
        .iter()
        .find(|((_, name), _)| name == anchor)
        .map(|(_, r)| match r {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
        .unwrap_or([0, crate::plan::BASE_Y, 0]);
    [
        (base[0] + offset[0]) as f64 + 0.5,
        (base[1] + offset[1]) as f64 + 0.5,
        (base[2] + offset[2]) as f64 + 0.5,
    ]
}

/// Validate every cutscene camera dolly (per shot: a multi-shot cutscene
/// hard-cuts between shots, so only the within-shot dolly is a corridor the
/// camera actually flies):
///
/// - **`DW0308` (authored polyline)**: the waypoint polyline passes only
///   through non-solid blocks (cameras fly but must not clip a solid). Names
///   the offending shot, segment and clipping block.
/// - **`DW0308` (rendered chords)**: the client draws straight chords between
///   the emitted keyframes ([`crate::camera::plan_shot`] — the tween is
///   client-side and linear, spike-measured), which can cut a corner of the
///   authored polyline by up to [`crate::camera::CHORD_POS_TOLERANCE`]. The
///   chord polyline is what actually ships, so it is ray-checked too.
/// - **`DW0347` (angular budget)**: the shot's peak aim rate must stay within
///   [`crate::camera::MAX_AIM_DEG_PER_TICK`]. An over-budget pan is a
///   provably nauseating shot — an error, not a warning: the fix (more camera
///   distance, a longer shot, or a hard cut between two shots) is always
///   available, and a red check is information (CLAUDE.md debug doctrine).
pub fn check_cutscenes(
    plan: &Plan,
    world: &World,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> Result<(), NavError> {
    for (eff, ctx) in crate::camera::cutscene_units(plan.campaign) {
        let Some(shots) = eff.cutscene_shots() else {
            continue;
        };
        let mut offset: i32 = 0;
        for (si, shot) in shots.iter().enumerate() {
            let ex = crate::camera::expand_shot(plan, moves, actor_moves, shot, &ctx, offset);
            offset += ex.ticks + 1;
            let pts = ex.clip_polyline();
            if let Some((seg, cell)) = first_clip(world, pts) {
                return Err(NavError {
                    code: DW_CUTSCENE_CLIP,
                    message: format!(
                        "cutscene: shot {si} camera dolly segment {seg} (from {:?} to {:?}) clips \
                         a solid block at {cell:?} — cameras must fly through open air; move the \
                         segment's waypoint `anchor`/`offset` (or the shot's `bearing`/`dist` for \
                         a styled shot) so the whole path clears solid blocks",
                        round3(pts[seg]),
                        round3(pts[seg + 1]),
                    ),
                });
            }
            let frames = ex.frames();
            let chord: Vec<[f64; 3]> = frames.frames.iter().map(|f| f.pos).collect();
            if let Some((seg, cell)) = first_clip(world, &chord) {
                return Err(NavError {
                    code: DW_CUTSCENE_CLIP,
                    message: format!(
                        "cutscene: shot {si} client-rendered dolly chord {seg} (keyframe {:?} to \
                         {:?}) clips a solid block at {cell:?} — the client tweens straight \
                         between keyframes, cutting inside the authored waypoint corner; move the \
                         nearby waypoint `anchor`/`offset` a block outward so the smoothed path \
                         also clears",
                        round3(chord[seg]),
                        round3(chord[seg + 1]),
                    ),
                });
            }
            let rate = ex.max_aim_deg_per_tick();
            if rate > crate::camera::MAX_AIM_DEG_PER_TICK {
                return Err(NavError {
                    code: DW_CAMERA_SPIN,
                    message: format!(
                        "cutscene: shot {si} pans at {rate} deg/tick, over the {} deg/tick \
                         (120 deg/s) budget — at 20 Hz that reads as a spin, not a shot \
                         (comfortable is <= 2 deg/tick). Move the camera path farther from its \
                         `look_at` subject, lengthen `seconds`, or split the move into two shots \
                         (the hard cut between shots is the idiomatic fast reframe)",
                        crate::camera::MAX_AIM_DEG_PER_TICK,
                    ),
                });
            }
        }
    }
    Ok(())
}

/// The first `(segment index, block cell)` where a camera dolly polyline passes
/// through a solid block, or `None` if the whole path is air.
///
/// **Exact, not sampled** (task #78). This used to step each segment at ≤ 0.25
/// blocks and floor the sample point, which misses any cell the segment only
/// grazes: a shot can cut a block corner, enter and leave the cell entirely
/// between two samples, and ship as "provably clear". The clip test is now a
/// 3-D grid walk (Amanatides–Woo digital differential analyser) that visits
/// **every** cell the segment intersects, in order, with no error term at all —
/// so `DW0308` can no longer be dodged by geometry that happens to fall between
/// two sample points.
///
/// Deterministic (ADR-0006): integer cell stepping driven by exact ratios; ties
/// (a segment passing exactly through a cell corner) resolve on the fixed axis
/// order x, y, z.
fn first_clip(world: &World, pts: &[[f64; 3]]) -> Option<(usize, [i32; 3])> {
    for (seg, w) in pts.windows(2).enumerate() {
        if let Some(cell) = walk_cells(w[0], w[1], |c| world.blocks_camera(c)) {
            return Some((seg, cell));
        }
    }
    None
}

/// Walk every unit cell the segment `a → b` passes through, in order, returning
/// the first for which `hit` holds. Amanatides–Woo voxel traversal: from the
/// starting cell, repeatedly advance along whichever axis reaches its next cell
/// boundary soonest. An axis with zero delta never steps (its `t_max` is
/// infinite). Both endpoint cells are included.
fn walk_cells(a: [f64; 3], b: [f64; 3], hit: impl Fn([i32; 3]) -> bool) -> Option<[i32; 3]> {
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
/// `move-actor` / `move-npc` `on_arrive` (spec-0014), so nav planning sees
/// moves/cutscenes wherever they appear. Pre-0.6 campaigns have no nesting, so
/// the flattened list equals the shallow one — output stays byte-identical.
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
        QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
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
    route_visited(
        world,
        &critical_positions(plan),
        &plan.gate_events,
        &|g, s| plan.gate_fired_before(g, s),
    )
}

/// The gate-region cells sealed on a walked leg arriving at the objective at
/// critical-path step `arrival` (DSL v0.6 close-gate completability). A gate event
/// counts only if its firing objective is a **causal (DAG) ancestor** of the leg's
/// objective — `ancestor(ev.fire_step, arrival)` — i.e. it is guaranteed to have
/// fired before this leg in *every* valid play order. That excludes a gate on a
/// parallel quest branch that the lineariser merely happens to interleave ahead of
/// this leg (which would falsely seal it). Among the causally-preceding events on a
/// region, the **latest** (max `fire_step`, respecting the DAG linearisation) wins;
/// the region is sealed iff that latest firing is a `close-gate` (a later
/// `open-gate` reopens it). Regions with no qualifying close contribute nothing, so
/// an open-gate-only campaign yields an empty set and routes byte-identically to the
/// base world.
fn sealed_gate_cells(
    gate_events: &[GateEvent],
    arrival: usize,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> BTreeSet<[i32; 3]> {
    // Per region, the causally-latest closed-state among firings that precede this
    // leg (ancestor of the arrival objective); higher `fire_step` overrides.
    let mut state: BTreeMap<([i32; 3], [i32; 3]), (usize, bool)> = BTreeMap::new();
    for ev in gate_events {
        if ancestor(ev.fire_step, arrival) {
            let e = state.entry(ev.region).or_insert((ev.fire_step, ev.closes));
            if ev.fire_step >= e.0 {
                *e = (ev.fire_step, ev.closes);
            }
        }
    }
    let mut sealed = BTreeSet::new();
    for (region, (_, closed)) in state {
        if closed {
            for cell in crate::assembled::region_cells(region.0, region.1) {
                sealed.insert(cell);
            }
        }
    }
    sealed
}

/// The gate cells sealed for the walked leg `from_step → to_step` — the single
/// definition of "which gates are shut while the player walks this leg", shared by
/// the completability proof, the forced-cell set the trap proof reasons about, and
/// the exported harness waypoints (task #78).
///
/// Only a **causal** leg is sealed — one whose start objective is a DAG ancestor of
/// the arrival objective, i.e. a step the player is genuinely forced to walk to
/// reach the arrival. The lineariser concatenates parallel quest branches, producing
/// artifact "legs" between objectives with no causal order (e.g. a `take-the-cheese`
/// beat followed by a `nobody` beat on a sibling branch); the player never actually
/// walks that pairing under the arrival's gate state, so sealing it would falsely
/// fail. A genuinely-forced re-crossing (start IS a causal ancestor) is still
/// sealed, preserving the proof. Base DW0311 (open world) already checked every leg.
fn leg_seal(
    gate_events: &[GateEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
    from_step: usize,
    to_step: usize,
) -> BTreeSet<[i32; 3]> {
    if ancestor(from_step, to_step) {
        sealed_gate_cells(gate_events, to_step, ancestor)
    } else {
        BTreeSet::new()
    }
}

/// Route every walked leg between consecutive visited positions over its
/// causally-sealed world ([`leg_seal`]), returning the proven cell routes. The
/// shared core of [`World::walked_legs`] and [`critical_path_routes`]; a leg whose
/// endpoints do not snap or that does not route is omitted (that is exactly the
/// [`route_visited`] failure, reported there as `DW0311`).
fn route_walked_legs(
    world: &World,
    positions: &[VisitedPos],
    gate_events: &[GateEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Vec<(LegRoute, BTreeSet<[i32; 3]>)> {
    let mut out = Vec::new();
    for pair in positions.windows(2) {
        if pair[1].transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let sealed = leg_seal(gate_events, ancestor, pair[0].src_step, pair[1].src_step);
        let leg_world_owned;
        let leg_world: &World = if sealed.is_empty() {
            world
        } else {
            leg_world_owned = world.with_sealed(&sealed);
            &leg_world_owned
        };
        let (Some(start), Some(goal)) = (
            leg_world.snap_endpoint(pair[0].pos, false),
            leg_world.snap_endpoint(pair[1].pos, pair[1].talk_to),
        ) else {
            continue;
        };
        if let Some(cells) = leg_world.find_path(start, goal) {
            let use_gates = cells
                .iter()
                .copied()
                .filter(|&c| leg_world.is_use_gate(c))
                .collect();
            out.push((
                LegRoute {
                    from: pair[0].pos,
                    to: pair[1].pos,
                    to_step: pair[1].src_step,
                    cells,
                    use_gates,
                },
                sealed,
            ));
        }
    }
    out
}

/// Route every walked leg between consecutive visited positions (the pure core of
/// [`check_critical_path`], split out so it is unit-testable without a full
/// [`Plan`]). A `transport_before` leg is a teleport ride and is skipped. Each leg
/// is routed over the world with any gate sealed by an earlier `close-gate`
/// ([`leg_seal`]) forced solid, so a forced path that must re-cross a sealed gate
/// fails [`DW_CRITICAL_UNROUTABLE`].
fn route_visited(
    world: &World,
    positions: &[VisitedPos],
    gate_events: &[GateEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), NavError> {
    for pair in positions.windows(2) {
        let from = pair[0].pos;
        let to = pair[1].pos;
        if pair[1].transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let sealed = leg_seal(gate_events, ancestor, pair[0].src_step, pair[1].src_step);
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
                "this is a wedged doorway seam, a void gap in the assembled layout, or an \
                 unbroken 1.5-tall barrier (fence/wall) ring — a walking player can neither pass \
                 through nor stand on top of a fence, so a pen needs a fence-gate opening (or, if \
                 the jump is intended, a missing inter-area transport)."
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
    verify_checkpoints(
        world,
        &cps,
        &critical_positions(plan),
        &plan.gate_events,
        &|g, s| plan.gate_fired_before(g, s),
    )
}

/// The pure core of [`check_checkpoints`] (split out so it is unit-testable
/// against a synthetic [`World`] without a full [`Plan`]). Each checkpoint is
/// `(anchor, cell, fire_step)`.
fn verify_checkpoints(
    world: &World,
    checkpoints: &[(String, [i32; 3], usize)],
    positions: &[VisitedPos],
    gate_events: &[GateEvent],
    ancestor: &dyn Fn(usize, usize) -> bool,
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
        let sealed = sealed_gate_cells(gate_events, target.src_step, ancestor);
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

/// The minimum share of a `timed-gate` cycle that must admit a crossing
/// (spec-0016 §4, owner ruling 2026-08-02). Below this the gate stops being a
/// timing read and becomes a coin flip. Expressed as a percentage so the
/// arithmetic below stays in integers — no float rounding in a proof (ADR-0006).
const TIMED_GATE_MIN_ADMIT_PERCENT: u32 = 20;

/// Prove every `timed-gate` is readable — [`DW_TIMED_GATE_COIN_FLIP`] (`DW0378`).
///
/// The requirement is deliberately **not** all-phase passability: spec-0016 §4 is
/// explicit that a gate which punishes bad timing is the entire point. What must
/// hold is that the gate can be *read*: over one full cycle, the entry phases from
/// which a walking player clears the span before it shuts must cover at least
/// [`TIMED_GATE_MIN_ADMIT_PERCENT`] of the cycle.
///
/// The crossing cost comes from the same nav model every other proof uses: the A*
/// step count from the footing on one side of the gate region to the footing on
/// the other with the gate open, charged at [`SPRINT_TICKS_PER_BLOCK`]. A player
/// who starts the crossing `p` ticks into the open window arrives in time iff
/// `p + cross <= open_ticks`, so the admitting window is
/// `max(0, open_ticks - cross + 1)` ticks out of `open_ticks + closed_ticks`.
///
/// A gate whose two sides have no walkable footing, or that no route connects even
/// while open, is left to the geometry proofs that own it (`DW0311`) rather than
/// double-reported here.
pub fn check_timed_gates(plan: &Plan, world: &World) -> Result<(), NavError> {
    verify_timed_gates(world, &plan.timed_gates)
}

/// The pure core of [`check_timed_gates`] (unit-testable against a synthetic
/// [`World`]).
fn verify_timed_gates(world: &World, gates: &[crate::plan::TimedGatePlan]) -> Result<(), NavError> {
    for g in gates {
        let cells: BTreeSet<[i32; 3]> =
            crate::assembled::region_cells(g.gate_region.0, g.gate_region.1).collect();
        let Some((near, far)) = gate_crossing_footings(world, g.gate_region, &cells) else {
            continue; // no footing on both sides — a geometry concern, not a timing one
        };
        let Some(path) = world.find_path(near, far) else {
            continue; // the open gate connects nothing — DW0311's business
        };
        // `path` includes both endpoints; the crossing is the moves between them.
        let cross_ticks = (path.len().saturating_sub(1) as u32) * SPRINT_TICKS_PER_BLOCK;
        let cycle = g.open_ticks + g.closed_ticks;
        let admits =
            g.open_ticks.saturating_sub(cross_ticks) + u32::from(cross_ticks <= g.open_ticks);
        // Integer percentage, rounded DOWN — the proof never credits the gate with
        // a share it does not have.
        let percent = admits.saturating_mul(100) / cycle.max(1);
        if percent < TIMED_GATE_MIN_ADMIT_PERCENT {
            return Err(NavError {
                code: DW_TIMED_GATE_COIN_FLIP,
                message: format!(
                    "timed gate `{}` is a coin flip, not a timing read: crossing its span takes \
                     {cross_ticks} ticks ({} blocks at {SPRINT_TICKS_PER_BLOCK} t/block), so only \
                     {admits} of its {cycle}-tick cycle ({percent}%) admit a player who starts \
                     walking then — under the {TIMED_GATE_MIN_ADMIT_PERCENT}% floor (spec-0016 §4, \
                     owner ruling 2026-08-02). Punishing bad timing is the point; punishing EVERY \
                     timing is a slot machine. Lengthen `open_ticks`, shorten `closed_ticks`, or \
                     narrow the span — never lower the floor.",
                    g.id,
                    path.len().saturating_sub(1)
                ),
            });
        }
    }
    Ok(())
}

/// The standable footings immediately on either side of a gate region, over the
/// world with the region SEALED so neither endpoint can land inside it or snap
/// through. The crossing axis is whichever horizontal axis actually has footing on
/// both sides — trying x then z rather than guessing from the region's extents is
/// both deterministic and correct for a square 1×1 gate column, where the extents
/// tie and a guess would pick the wall's own plane.
fn gate_crossing_footings(
    world: &World,
    region: ([i32; 3], [i32; 3]),
    cells: &BTreeSet<[i32; 3]>,
) -> Option<([i32; 3], [i32; 3])> {
    let sealed = world.with_sealed(cells);
    let (from, to) = region;
    for axis in [0usize, 2] {
        let mut near = None;
        let mut far = None;
        for cell in crate::assembled::region_cells(from, to) {
            for (slot, delta) in [(&mut near, -1), (&mut far, 1)] {
                if slot.is_some() {
                    continue;
                }
                let mut c = cell;
                c[axis] += delta;
                if !cells.contains(&c) && sealed.standable(c) {
                    *slot = Some(c);
                }
            }
            if near.is_some() && far.is_some() {
                break;
            }
        }
        if let (Some(n), Some(f)) = (near, far) {
            return Some((n, f));
        }
    }
    None
}

/// Prove every `ambush` (spec-0016 §3) leaves the player a play —
/// [`DW_AMBUSH_NO_COUNTERPLAY`] (`DW0376`).
///
/// The obligation is *not* "warn the player". Spec-0016 is explicit that the
/// un-telegraphed ambush — 初见杀, the shove off the cliff you could not have
/// known about — is legitimate and essential: you die uninformed once, and the
/// SECOND attempt is where the design pays off. Determinism already guarantees
/// that second attempt meets the same ambushers in the same cells.
///
/// What the compiler adds is the half determinism cannot supply: that there is
/// something to *do* about them. Generalizing the trap-avoidability machinery
/// (`DW0342`'s "reachable with the hazard cell blocked"), this stands every
/// ambusher on the cell it will occupy and re-asks whether the trigger cell still
/// connects to any rest point — a checkpoint, a bonfire, or the campaign entry.
/// If it does, a retreat exists: luring ground, a positioning line, an exit. If it
/// does not, the player is sealed in a pocket with the ambush and the beat has no
/// second attempt to reward — that is a broken beat, not a hard one.
pub fn check_ambushes(plan: &Plan, world: &World, entry: Option<[i32; 3]>) -> Result<(), NavError> {
    let mut rests: Vec<[i32; 3]> = plan.checkpoints.iter().map(|c| c.pos).collect();
    rests.extend(entry);
    verify_ambushes(world, &plan.ambushes, &rests)
}

/// The pure core of [`check_ambushes`] (unit-testable against a synthetic
/// [`World`]). `rests` are the cells that count as safety — every checkpoint and
/// bonfire cell plus the campaign entry.
fn verify_ambushes(
    world: &World,
    ambushes: &[crate::plan::AmbushPlan],
    rests: &[[i32; 3]],
) -> Result<(), NavError> {
    if ambushes.is_empty() || rests.is_empty() {
        return Ok(());
    }
    for amb in ambushes {
        let blocked: BTreeSet<[i32; 3]> = amb
            .actor_cells
            .iter()
            .copied()
            .filter(|c| *c != amb.at)
            .collect();
        if blocked.is_empty() {
            continue; // nothing stands in the player's way
        }
        let occupied = world.with_sealed(&blocked);
        let Some(from) = occupied.snap_standable(amb.at, SNAP_RADIUS) else {
            continue; // an unstandable trigger cell is another proof's concern
        };
        let escapes = rests.iter().any(|r| {
            occupied
                .snap_standable(*r, SNAP_RADIUS)
                .is_some_and(|goal| occupied.find_path(from, goal).is_some())
        });
        if !escapes {
            return Err(NavError {
                code: DW_AMBUSH_NO_COUNTERPLAY,
                message: format!(
                    "ambush `{}` at {:?} leaves no counterplay: with its ambushers standing on \
                     {:?}, no checkpoint, bonfire or campaign entry is walkable from the trigger \
                     cell any more — the party is sealed in a pocket with the ambush and can only \
                     trade blows blind. An un-telegraphed ambush is fine (spec-0016 §3: dying \
                     uninformed once is how the level teaches); an ambush with no retreat, no \
                     luring ground and no exit is not, because the second attempt has nothing to \
                     reward. Widen the room, move an ambusher off the only way out, or add a rest \
                     point behind the player — do NOT delete the proof.",
                    amb.id, amb.at, amb.actor_cells
                ),
            });
        }
    }
    Ok(())
}

/// Prove every `shortcut` door (spec-0016 §2) is a real shortcut.
///
/// The base occupancy model treats every gate region as passable (the
/// "assume the gate the player needs is opened" stance), and `Plan::build`
/// registers each shortcut gate as sealed from step 0, so the critical path,
/// the checkpoints and the traps are all already proven **without** any shortcut
/// taken. What remains are the two obligations the pattern itself carries, both
/// measured against the same sealed world:
///
/// 1. [`DW_SHORTCUT_NO_LONG_ROUTE`] (`DW0373`) — with the gate SEALED, the
///    far-side `unlock` affordance must still be walkable from the campaign
///    entry. That walk IS the long route. Without it the mechanism that opens the
///    shortcut sits behind the shortcut, and the gate is dead scenery.
/// 2. [`DW_SHORTCUT_NO_GAIN`] (`DW0374`) — opening the gate must strictly shorten
///    that same walk. This is the anti-leak proof: it is what makes `unlock` a
///    FAR-side anchor rather than a label. An unlock on the near side of its own
///    gate measures identically sealed and open, and fails here.
///
/// Distances are A* step counts over the nav model — the same routing every other
/// completability proof uses, so the two numbers are directly comparable.
pub fn check_shortcuts(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> Result<(), NavError> {
    verify_shortcuts(world, &plan.shortcuts, entry)
}

/// The pure core of [`check_shortcuts`] (split out so it is unit-testable against
/// a synthetic [`World`] without a full [`Plan`]). With no resolvable entry cell
/// there is nothing to measure from and both proofs are vacuous — `DW0345`
/// already fails a campaign whose entry does not resolve.
fn verify_shortcuts(
    world: &World,
    shortcuts: &[crate::plan::ShortcutPlan],
    entry: Option<[i32; 3]>,
) -> Result<(), NavError> {
    let Some(entry) = entry else {
        return Ok(());
    };
    for sc in shortcuts {
        let cells: BTreeSet<[i32; 3]> =
            crate::assembled::region_cells(sc.gate_region.0, sc.gate_region.1).collect();
        let sealed = world.with_sealed(&cells);

        // Both walks are measured from the same footing and to the same goal; only
        // the gate differs. Snapping happens on the SEALED world so neither
        // endpoint can land inside the gate region itself.
        let start = sealed.snap_standable(entry, SNAP_RADIUS);
        let goal = sealed.snap_standable(sc.unlock, SNAP_RADIUS);
        let (Some(start), Some(goal)) = (start, goal) else {
            // An unstandable entry or unlock is another proof's concern
            // (`DW0345` / the anchor checks); do not double-report it here.
            continue;
        };

        // (1) the long route exists while the gate is sealed.
        let Some(long) = sealed.find_path(start, goal).map(|p| p.len()) else {
            return Err(NavError {
                code: DW_SHORTCUT_NO_LONG_ROUTE,
                message: format!(
                    "shortcut `{}`: no long route — its unlock affordance at `{}` ({:?}) is not \
                     walkable from the campaign entry while gate `{}` is sealed, so the mechanism \
                     that opens the shortcut sits behind the shortcut and can never be pulled. A \
                     shortcut is earned the hard way first (spec-0016 §2): connect the far side by \
                     a long route, or move the unlock onto one. Do NOT open the gate at world-load \
                     to silence this.",
                    sc.id, sc.unlock_anchor, sc.unlock, sc.gate_anchor
                ),
            });
        };

        // (2) opening the gate strictly shortens that same walk (anti-leak).
        let short = world
            .find_path(start, goal)
            .map(|p| p.len())
            .unwrap_or(long);
        if short >= long {
            return Err(NavError {
                code: DW_SHORTCUT_NO_GAIN,
                message: format!(
                    "shortcut `{}` leaks: opening gate `{}` does not shorten the walk from the \
                     campaign entry to its own unlock `{}` ({long} steps sealed, {short} open), so \
                     the unlock is not on the far side of anything and the loop-back the shortcut \
                     is FOR never happens. Put the unlock past the gate, on the end of the long \
                     route (spec-0016 §2) — never delete the proof.",
                    sc.id, sc.gate_anchor, sc.unlock_anchor
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
            // EVERY standable cell of the zone box, not just the one nearest its
            // centre (task #78). A zone whose centre snaps to a standable cell in a
            // walled-off pocket while the rest of the box is perfectly reachable
            // used to raise a spurious `DW0327`; the obligation is "the player can
            // reach *somewhere* in this zone", so the proof is reachable-any.
            let stands: Vec<[i32; 3]> = crate::assembled::region_cells(lo, hi)
                .filter(|c| world.is_standable(*c))
                .collect();
            if stands.is_empty() {
                return Err(NavError {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (box {lo:?}..{hi:?}) has no standable cell — a \
                         player can never legally hide there, so the beat is unwinnable. Place the \
                         zone over reachable floor, or widen its `extent` to include a standable \
                         cell."
                    ),
                });
            }
            // One reachability flood from the player's position (not one A* per zone
            // cell): the question is set membership, not a route.
            let start = player_pos.and_then(|p| world.snap_standable(p, SNAP_RADIUS));
            if let Some(start) = start
                && {
                    let reachable = world.reachable_walkable(&[start]);
                    !stands.iter().any(|s| reachable.contains(s))
                }
            {
                return Err(NavError {
                    code: DW_STEALTH_ZONE,
                    message: format!(
                        "stealth zone `{name}` (box {lo:?}..{hi:?}, {n} standable cell(s)) is not \
                         reachable from the player's position {:?} when the stealth beat begins — \
                         NO cell of the zone is walkable from there, so the player would be caught \
                         before ever reaching cover. Route the zone within walkable reach of the \
                         activating beat, or move where the beat starts.",
                        player_pos.unwrap(),
                        n = stands.len(),
                    ),
                });
            }
        }
    }
    Ok(())
}

// --- DW0355: stealth onset survivability ------------------------------------

/// Ticks a sprinting player needs to cross one block. Vanilla sprint is
/// 5.612 blocks/s = 0.2806 blocks/tick → 3.56 t/block; rounded **up** to 4 so the
/// model never credits the player with speed they do not have. (Sprint-jumping is
/// faster; the proof deliberately does not assume the player chains jumps.)
const SPRINT_TICKS_PER_BLOCK: u32 = 4;
/// Extra ticks charged for each one-block step **up** on the flee route — the jump
/// arc a player must complete to gain the block. Conservative: a vanilla jump apex
/// is ~6 ticks.
const CLIMB_TICKS: u32 = 6;
/// Ticks charged before the player is under way at all: the beat arms while they
/// are standing still, mid-interaction, reading the narration that tells them to
/// run. 10 ticks = 0.5 s of orientation. This is the "fair warning" allowance —
/// without it the proof would assume a player already sprinting toward cover at
/// the instant the session arms.
const ONSET_REACTION_TICKS: u32 = 10;

/// A start position the onset proof must clear, with the label its diagnostic uses.
type OnsetStart = (String, [i32; 3]);

/// Prove every **punishing** `begin-stealth` beat is escapable at onset (DSL v0.6,
/// spec-0014 + spec-0016) — [`DW_STEALTH_ONSET`] (`DW0355`).
///
/// DW0327 already proves each zone is standable and connected to the beat. That is
/// not enough: `begin-stealth` arms *instantly*, the judge starts counting on the
/// very next tick, and `on_caught` fires `grace_ticks` later wherever the player
/// happens to be. So the real obligation is a **timing** one — from every position
/// a player can legally occupy when the session arms, some zone must be reachable
/// within the grace window at sprint speed:
///
/// - the **activating position** — the anchor of the objective whose completion
///   fires the beat (where the player provably is, since completing it is what
///   armed the session), and
/// - every **respawn position** — each `set-checkpoint` reigning at some step in
///   the beat's active window `[fire_step, end_step]`. A caught player respawns
///   there with the session still running and the grace clock restarted; if that
///   cell cannot beat the window either, the beat is an infinite death loop rather
///   than a souls retry.
///
/// Routes are measured over the same per-leg geometry DW0311/DW0315 use (gates
/// causally sealed by the beat's firing step forced solid), costed at
/// [`SPRINT_TICKS_PER_BLOCK`] per block plus [`CLIMB_TICKS`] per block climbed, and
/// charged [`ONSET_REACTION_TICKS`] of standing-start reaction.
///
/// Scope: beats whose `on_caught` actually punishes ([`StealthBeat::is_punishing`]
/// — `damage-players` or `spawn-wave`, at any nesting depth). A beat that only
/// narrates when spotted has nothing to escape, so no timing obligation exists.
pub fn check_stealth_onset(plan: &Plan, world: &World) -> Result<(), NavError> {
    let positions = critical_positions(plan);
    for beat in &plan.stealth_beats {
        if !beat.is_punishing() {
            continue;
        }
        let mut starts: Vec<OnsetStart> = Vec::new();
        // 1. Where the player stands when the beat arms: the visited position at
        //    the firing step, else the nearest earlier one, else the first.
        if let Some(p) = positions
            .iter()
            .filter(|p| p.src_step <= beat.fire_step)
            .max_by_key(|p| p.src_step)
            .or_else(|| positions.first())
        {
            starts.push((
                format!("the activating objective's anchor {:?}", p.pos),
                p.pos,
            ));
        }
        // 2. Every checkpoint that can drop a player into the running session: the
        //    one reigning when the beat arms (latest fire_step ≤ fire_step, ties
        //    broken by content index — a `set-checkpoint` listed beside the
        //    `begin-stealth` in the same objective's effects is the reigning one),
        //    plus every checkpoint set later while the beat is still active.
        let end = beat.end_step.unwrap_or(usize::MAX);
        if let Some(reigning) = plan
            .checkpoints
            .iter()
            .filter(|c| c.fire_step <= beat.fire_step)
            .max_by_key(|c| (c.fire_step, c.index))
        {
            starts.push((
                format!(
                    "checkpoint `{}` respawn {:?}",
                    reigning.anchor, reigning.pos
                ),
                reigning.pos,
            ));
        }
        for c in plan
            .checkpoints
            .iter()
            .filter(|c| c.fire_step > beat.fire_step && c.fire_step <= end)
        {
            starts.push((
                format!("checkpoint `{}` respawn {:?}", c.anchor, c.pos),
                c.pos,
            ));
        }
        let sealed = sealed_gate_cells(&plan.gate_events, beat.fire_step, &|g, s| {
            plan.gate_fired_before(g, s)
        });
        let leg_world_owned;
        let leg_world: &World = if sealed.is_empty() {
            world
        } else {
            leg_world_owned = world.with_sealed(&sealed);
            &leg_world_owned
        };
        verify_stealth_onset(
            leg_world,
            &beat.zones,
            beat.grace_ticks,
            &starts,
            beat.index,
        )?;
    }
    Ok(())
}

/// The pure core of [`check_stealth_onset`] (unit-testable against a synthetic
/// [`World`]): every start must reach some zone cell within `grace_ticks`.
fn verify_stealth_onset(
    world: &World,
    zones: &[ZoneCell],
    grace_ticks: u32,
    starts: &[OnsetStart],
    beat_index: usize,
) -> Result<(), NavError> {
    // The budget the flee route itself gets, after the standing-start allowance.
    let budget = grace_ticks.saturating_sub(ONSET_REACTION_TICKS);
    for (label, raw) in starts {
        let Some(start) = world.snap_standable(*raw, SNAP_RADIUS) else {
            continue; // unsnappable start — DW0311/DW0315/DW0316's concern, not ours
        };
        let Some((cost, cell, zone)) = nearest_zone_by_flee_time(world, zones, start, grace_ticks)
        else {
            continue; // no zone reachable at all within the search cap — DW0327's concern
        };
        if cost <= budget {
            continue;
        }
        let need = cost + ONSET_REACTION_TICKS;
        let deficit = need - grace_ticks;
        return Err(NavError {
            code: DW_STEALTH_ONSET,
            message: format!(
                "stealth beat #{beat_index}: a player cannot beat the grace window from {label}. \
                 The nearest zone cell is `{zone}` {cell:?} — {cost} ticks of sprinting away \
                 (model: {SPRINT_TICKS_PER_BLOCK} t/block, +{CLIMB_TICKS} t per block climbed) \
                 plus {ONSET_REACTION_TICKS} ticks of standing-start reaction = {need} ticks, \
                 against `grace_ticks` {grace_ticks} — short by {deficit} ticks. The beat's \
                 `on_caught` punishes, so EVERY player dies here a fixed moment after it arms, \
                 and if this start is a checkpoint the retry loop never terminates. Fix the \
                 BEAT, not the proof: raise `grace_ticks` to at least {need} (the measured \
                 sprint time plus reaction) and add a tension margin, put a zone within reach \
                 of where the beat actually starts, move the checkpoint into/beside a zone, or \
                 arm the beat from a less exposed objective. Note that merely DELAYING the arm \
                 (a `sequence` step) does not discharge this: the clock still starts with the \
                 player free to be standing right here, so the grace window itself must cover \
                 the flee. Do NOT delete the `on_caught` consequence to silence this."
            ),
        });
    }
    Ok(())
}

/// The cheapest zone cell by **flee time** from `start`: a deterministic
/// tick-weighted Dijkstra over standable cells (cardinal steps cost
/// [`SPRINT_TICKS_PER_BLOCK`], a step up additionally costs [`CLIMB_TICKS`]),
/// stopping at the first settled cell inside any zone box. Returns
/// `(ticks, cell, zone name)`.
///
/// The search is capped well past the grace window so a failing beat can still
/// report a real number; beyond the cap the beat is failing by so much that the
/// exact figure carries no extra information. Determinism (ADR-0006): the frontier
/// is ordered by `(cost, cell)` and neighbours expand in `neighbors`' fixed order.
fn nearest_zone_by_flee_time(
    world: &World,
    zones: &[ZoneCell],
    start: [i32; 3],
    grace_ticks: u32,
) -> Option<(u32, [i32; 3], String)> {
    let boxes: Vec<(&str, [i32; 3], [i32; 3])> = zones
        .iter()
        .map(|(name, pos, extent)| {
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
            (name.as_str(), lo, hi)
        })
        .collect();
    let zone_of = |c: [i32; 3]| {
        boxes
            .iter()
            .find(|(_, lo, hi)| (0..3).all(|k| lo[k] <= c[k] && c[k] <= hi[k]))
            .map(|(n, _, _)| *n)
    };
    let cap = grace_ticks.saturating_mul(4).saturating_add(400);
    let mut best: BTreeMap<[i32; 3], u32> = BTreeMap::new();
    let mut open: BinaryHeap<Reverse<(u32, [i32; 3])>> = BinaryHeap::new();
    best.insert(start, 0);
    open.push(Reverse((0, start)));
    while let Some(Reverse((cost, cur))) = open.pop() {
        if cost > *best.get(&cur).unwrap_or(&u32::MAX) {
            continue; // stale heap entry
        }
        if let Some(name) = zone_of(cur) {
            return Some((cost, cur, name.to_string()));
        }
        if cost >= cap {
            continue;
        }
        for n in world.neighbors(cur) {
            let climb = if n[1] > cur[1] { CLIMB_TICKS } else { 0 };
            let next = cost + SPRINT_TICKS_PER_BLOCK + climb;
            if next < *best.get(&n).unwrap_or(&u32::MAX) {
                best.insert(n, next);
                open.push(Reverse((next, n)));
            }
        }
    }
    None
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
) -> Result<(), NavError> {
    for t in traps {
        if !matches!(t.lethality, Lethality::Lethal) {
            continue; // only lethal traps carry the obligation
        }
        let tc = t.trigger_cell;
        // (a) Avoidable: the trigger cell is never a forced critical-path cell.
        // `required` is now computed over the causally-sealed per-leg world (task
        // #78), so a detour the player only has to walk BECAUSE a `close-gate`
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
    /// The cells of `cells` that are closed fence gates the player must right-click
    /// open to pass ("use-gate" edges, task #59), in path order. Exported in the
    /// waypoint metadata so the harness bot knows the leg crosses a gate (its
    /// pathfinder's `canOpenDoors` performs the adventure-legal click — harness
    /// PR #110); always kept as thinned waypoints.
    pub use_gates: Vec<[i32; 3]>,
}

/// Compute the proven A* cell route for every WALKED critical-path leg (transport
/// hops skipped), for export as validation metadata (see the `waypoints` module).
/// Mirrors [`check_critical_path`]'s leg selection, endpoint snapping **and
/// per-leg gate seals** exactly (task #78), so an exported leg is the same route
/// the DW0311 guard proved routable — and an exported waypoint can never cross a
/// gate a `close-gate` has already shut by the time the bot walks that leg.
/// Intended to be called only after [`check_critical_path`] has succeeded; a leg
/// that fails to snap or route is silently omitted (cannot occur once the check
/// has passed).
pub fn critical_path_routes(plan: &Plan, world: &World) -> Vec<LegRoute> {
    world.walked_legs(plan)
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

/// `DW0322`: **boundary safety** (spec-0017 invariant 4) — after a world edit,
/// the reachable walk region fails the "one step off the proven ground is
/// survivable and recoverable" guarantee the greenfield generator's bounding
/// berm used to provide *physically*. What that means is a property of the
/// world-generator [`Ambient`], so the code names one rule stated per horizon:
///
/// * `horizon: void` — a reachable walkable cell borders a **void drop**: a
///   horizontally adjacent column the player can step (or open a gate) into
///   with no support of any kind below, so the step leaves the world.
/// * `horizon: ocean` — a reachable walkable cell borders **water the player
///   cannot get out of**: the pinned superflat puts bedrock under every column,
///   so nothing can fall out of an ocean world and the void premise is vacuous;
///   the real hazard the ocean horizon introduced (`plan::OCEAN_BASE_Y`) is
///   *stranding* — a player who ends up in the sea with no shoreline to climb
///   back onto is out of the delve just as permanently as one who fell out of a
///   void world. See [`verify_boundary_safety`] for the exact model.
pub const DW_EDIT_BORDERS_VOID: &str = "DW0322";

/// How many individual violations a `DW0322` report names before summarising the
/// remainder as a count. A boundary failure is systemic by nature — one stripped
/// berm is hundreds of exposed columns — and hundreds of identical lines are
/// noise, not information (the `DW0354` aggregation precedent, `edit::check_support`).
/// Aborting at the first one instead hid the *scale*, which is the single most
/// useful fact about the failure: "one cell" and "the whole coastline" call for
/// completely different fixes.
const BOUNDARY_LIST_LIMIT: usize = 6;

/// How far past the placed geometry the ocean-stranding search window extends
/// before the sea counts as **open sea** (see [`verify_boundary_safety`]). Any
/// margin ≥ 1 works: the ring beyond the window is untouched ambient water in
/// every direction, so every body that reaches it is one and the same sea.
const OPEN_SEA_MARGIN: i32 = 2;

/// Assert the reachable walk region's boundary is safe (spec-0017 boundary
/// safety; [`DW_EDIT_BORDERS_VOID`]). `starts` are the reachability roots (the
/// plan's resolved anchors — the same roots the relight pass floods from). Run
/// after every edit batch — never on the no-edit path, whose worlds provide this
/// physically.
///
/// The premise is the world's [`Ambient`] — what a column the compiler modelled
/// nothing into actually contains — and the rule follows from it:
///
/// **`Ambient::Void`** (unchanged, byte-identical semantics). A neighbour column
/// is a void drop when the player could enter it — its feet and head cells are
/// clear (a closed fence gate counts as enterable: opening it is an
/// adventure-legal right-click) — and **nothing anywhere below** would arrest
/// the fall: no solid, no 1.5-tall barrier top, no gate top, no water. A deep
/// drop onto real geometry is legal (that is falling, not leaving the world);
/// only a bottomless column is an error.
///
/// **`Ambient::Ocean`** — the *stranding* invariant. The superflat's bedrock
/// floor makes every column fall-arresting, so the question is never "can the
/// player fall out" but "can the player get back". The model:
///
/// 1. **Entering.** A reachable walkable cell `c` puts the player in the sea if
///    some horizontally adjacent column is enterable at `c`'s level (feet + head
///    clear of solids and 1.5-tall barriers — water does *not* block walking in)
///    and that column is open, between `c`'s level and the sea surface, all the
///    way to ambient water. Whether the player walks in, wades in or falls from a
///    cliff, they end up afloat: vanilla buoyancy puts a swimmer at the surface
///    plane, `sea.level`.
/// 2. **The sea.** A surface cell (`y == sea.level`) is swimmable when it is not
///    solid/tall and is either ambient water or authored water (a lagoon at sea
///    level is physically the same plane). Surface cells are 4-connected into
///    **bodies**; a body that reaches the edge of the search window is the open
///    sea, and all such bodies are one (the ring beyond the window is untouched
///    ambient water). Connectivity is taken on the surface plane only — a diver
///    might swim under a land bridge into another body, which this model
///    deliberately does not count on.
/// 3. **Climbing out.** A body is escapable when some surface cell of it is
///    horizontally adjacent to a **proven reachable walkable** cell whose feet
///    are at `sea.level` (wade out of the shallows onto a rim one block below the
///    waterline) or at `sea.level + 1` (the canonical beach: land flush with the
///    sea surface). A ledge higher than that is a wall to a swimmer, and a
///    boat/blocks are not available in adventure mode.
///
/// A body the player can enter and cannot climb out of is the violation.
///
/// ## Why the climb-out band stays **cell-level** under partial floor heights
///
/// The step rule reasons in sixteenths ([`World::feet_16_fp`], task #78), but this
/// band deliberately does not. A partial floor can only ever *lower* the standing
/// surface inside its own cell (`feet_16(c) ≤ c.y · 16`), so:
///
/// - inside the band, refining `level` / `level + 1` to a true feet height never
///   flips a verdict — a swimmer climbing onto a slab at `level + 0.5` has an
///   easier exit than onto full ground at `level + 1`, and the body is escapable
///   either way;
/// - the only refinement that *could* flip one is admitting a cell at
///   `level + 2` whose partial support drops its feet back into jump range. That
///   would mark **more** bodies escapable, i.e. weaken the stranding proof.
///
/// So the cell-level band is already the conservative reading of the sixteenth
/// model, and tightening it here could only ever lose a `DW0322` that should
/// fire. The two models compose without interacting: partial heights change
/// *which cells are reachable* (via [`World::neighbors_fp`], feeding `reachable`
/// above), never *what counts as a climb-out*.
pub fn verify_boundary_safety(world: &World, starts: &[[i32; 3]]) -> Result<(), NavError> {
    let reachable = world.reachable_walkable(starts);
    match &world.ambient {
        Ambient::Void => boundary_void(world, &reachable),
        Ambient::Ocean(sea) => boundary_ocean(world, &reachable, sea),
    }
}

/// Boundary safety under [`Ambient::Void`]: no reachable walkable cell may
/// border a bottomless column. Every violation is collected (see
/// [`BOUNDARY_LIST_LIMIT`]) so one report shows the scale of the breach.
fn boundary_void(world: &World, reachable: &BTreeSet<[i32; 3]>) -> Result<(), NavError> {
    // Per-column lowest fall-arresting cell: solid, tall barrier, use-gate, or
    // flooded — anything vanilla stops a falling player on (or in).
    let mut col_min: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    for set in [&world.solid, &world.tall, &world.use_gates, &world.flooded] {
        for c in set.iter() {
            col_min
                .entry((c[0], c[2]))
                .and_modify(|m| *m = (*m).min(c[1]))
                .or_insert(c[1]);
        }
    }
    // (edge cell, void column entered) pairs, in deterministic BTreeSet order.
    let mut hits: Vec<([i32; 3], [i32; 3])> = Vec::new();
    let mut columns: BTreeSet<(i32, i32)> = BTreeSet::new();
    for &cell in reachable {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [cell[0] + dx, cell[1], cell[2] + dz];
            let head = [n[0], n[1] + 1, n[2]];
            // Enterable: feet + head clear of solids/talls/water. A use-gate
            // cell is deliberately enterable (the player can open it and walk
            // through — a gate onto a bottomless drop is exactly the hazard).
            let blocked = |c: [i32; 3]| {
                world.solid.contains(&c) || world.tall.contains(&c) || world.flooded.contains(&c)
            };
            if blocked(n) || blocked(head) {
                continue;
            }
            let has_support = col_min
                .get(&(n[0], n[2]))
                .is_some_and(|&lowest| lowest < n[1]);
            if !has_support {
                hits.push((cell, n));
                columns.insert((n[0], n[2]));
            }
        }
    }
    if hits.is_empty() {
        return Ok(());
    }
    let mut listing = String::new();
    for (cell, n) in hits.iter().take(BOUNDARY_LIST_LIMIT) {
        listing.push_str(&format!("\n  - {cell:?} → void drop at {n:?}"));
    }
    if hits.len() > BOUNDARY_LIST_LIMIT {
        listing.push_str(&format!(
            "\n  - … and {} more",
            hits.len() - BOUNDARY_LIST_LIMIT
        ));
    }
    let first = hits[0].1;
    Err(NavError {
        code: DW_EDIT_BORDERS_VOID,
        message: format!(
            "boundary safety (spec-0017): {} reachable walkable cell(s) border a void drop over \
             {} distinct column(s) — one step off the proven ground falls out of the world:{}\n\
             The edit stripped the physical boundary here: extend the terrain under the exposed \
             edge (fill/morph a slope or outcrop below {first:?}) or reinstate a barrier shape; \
             do NOT weaken this check or reroute the path to sidestep it",
            hits.len(),
            columns.len(),
            listing,
        ),
    })
}

/// One 4-connected body of sea-surface cells, plus what the walk region does
/// with it (see [`verify_boundary_safety`]'s ocean model).
struct SeaBody {
    /// Reaches the search-window edge ⇒ it is the open sea, and every other
    /// open body is the same water.
    open: bool,
    /// Some surface cell of the body is adjacent to a reachable walkable cell at
    /// `sea.level` or `sea.level + 1`.
    escapable: bool,
    /// Reachable walkable cells from which the player enters this body, in
    /// deterministic order.
    entries: BTreeSet<[i32; 3]>,
    /// A representative surface cell (the smallest, for a stable message).
    sample: [i32; 3],
    /// Surface cells in the body.
    size: usize,
}

/// Boundary safety under [`Ambient::Ocean`]: the stranding invariant. See
/// [`verify_boundary_safety`] for the model this implements.
fn boundary_ocean(
    world: &World,
    reachable: &BTreeSet<[i32; 3]>,
    sea: &Sea,
) -> Result<(), NavError> {
    let level = sea.level;
    let Some(([min_x, min_z], [max_x, max_z])) = ocean_window(world, sea) else {
        return Ok(()); // nothing placed: open sea everywhere, nothing to strand
    };
    let w = (max_x - min_x + 1) as usize;
    let d = (max_z - min_z + 1) as usize;
    let idx = |x: i32, z: i32| (x - min_x) as usize * d + (z - min_z) as usize;
    let inside = |x: i32, z: i32| (min_x..=max_x).contains(&x) && (min_z..=max_z).contains(&z);
    let blocked = |c: [i32; 3]| world.solid.contains(&c) || world.tall.contains(&c);
    let swimmable = |x: i32, z: i32| {
        let c = [x, level, z];
        !blocked(c) && (world.flooded.contains(&c) || sea.ambient_water(c))
    };

    // --- label the sea-surface bodies (deterministic scan + BFS) -------------
    const NONE: u32 = u32::MAX;
    let mut label = vec![NONE; w * d];
    let mut bodies: Vec<SeaBody> = Vec::new();
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            if label[idx(x, z)] != NONE || !swimmable(x, z) {
                continue;
            }
            let id = bodies.len() as u32;
            let mut body = SeaBody {
                open: false,
                escapable: false,
                entries: BTreeSet::new(),
                sample: [x, level, z],
                size: 0,
            };
            let mut queue = std::collections::VecDeque::from([(x, z)]);
            label[idx(x, z)] = id;
            while let Some((cx, cz)) = queue.pop_front() {
                body.size += 1;
                if cx == min_x || cx == max_x || cz == min_z || cz == max_z {
                    body.open = true;
                }
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, nz) = (cx + dx, cz + dz);
                    if !inside(nx, nz) || label[idx(nx, nz)] != NONE || !swimmable(nx, nz) {
                        continue;
                    }
                    label[idx(nx, nz)] = id;
                    queue.push_back((nx, nz));
                }
            }
            bodies.push(body);
        }
    }
    if bodies.is_empty() {
        return Ok(());
    }

    // --- where the walk region touches the water ----------------------------
    for &cell in reachable {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [cell[0] + dx, cell[1], cell[2] + dz];
            // Climb-out: standing at (or one above) the waterline beside water.
            if (cell[1] == level || cell[1] == level + 1) && inside(n[0], n[2]) {
                let id = label[idx(n[0], n[2])];
                if id != NONE {
                    bodies[id as usize].escapable = true;
                }
            }
            // Entry: an enterable neighbour column that is open all the way to
            // the sea surface. Water does not block walking in.
            if blocked(n) || blocked([n[0], n[1] + 1, n[2]]) || !inside(n[0], n[2]) {
                continue;
            }
            let id = label[idx(n[0], n[2])];
            if id == NONE {
                continue;
            }
            let (lo, hi) = if n[1] > level {
                (level + 1, n[1])
            } else {
                (n[1], level)
            };
            if (lo..=hi).any(|y| blocked([n[0], y, n[2]])) {
                continue;
            }
            bodies[id as usize].entries.insert(cell);
        }
    }

    // Every body that reaches the window edge is the same open sea: one climb-out
    // anywhere on the coast serves all of them.
    let open_escapable = bodies.iter().any(|b| b.open && b.escapable);
    let stranding: Vec<&SeaBody> = bodies
        .iter()
        .filter(|b| !b.entries.is_empty() && !b.escapable && !(b.open && open_escapable))
        .collect();
    if stranding.is_empty() {
        return Ok(());
    }

    let shores: usize = stranding.iter().map(|b| b.entries.len()).sum();
    let mut listing = String::new();
    let mut listed = 0usize;
    for b in &stranding {
        for cell in b.entries.iter() {
            if listed == BOUNDARY_LIST_LIMIT {
                break;
            }
            listing.push_str(&format!(
                "\n  - {cell:?} → the sea at {:?} ({})",
                b.sample,
                if b.open { "open sea" } else { "enclosed water" }
            ));
            listed += 1;
        }
    }
    if shores > listed {
        listing.push_str(&format!("\n  - … and {} more", shores - listed));
    }
    let first = stranding[0]
        .entries
        .iter()
        .next()
        .copied()
        .unwrap_or_default();
    Err(NavError {
        code: DW_EDIT_BORDERS_VOID,
        message: format!(
            "boundary safety (spec-0017, `horizon: ocean`): {shores} reachable walkable cell(s) \
             let the player into {} body/bodies of water ({} surface cell(s)) with NO way back \
             ashore — nothing in an ocean world falls out of the world, but a swimmer who cannot \
             climb out is stranded there for the rest of the delve:{}\n\
             A climb-out is a proven-walkable cell at y={level} (a rim one block under the \
             waterline: wade out) or y={} (land flush with the sea surface) beside the water. \
             Give the shoreline near {first:?} such a step — a beach, a bank, a ladder-free \
             landing — or wall the edge off so the player cannot enter the water there; do NOT \
             weaken this check",
            stranding.len(),
            stranding.iter().map(|b| b.size).sum::<usize>(),
            listing,
            level + 1,
        ),
    })
}

/// The x/z window the ocean stranding search runs over: every placed piece and
/// every modelled cell, inflated by [`OPEN_SEA_MARGIN`]. `None` when the world is
/// completely empty. Beyond the window the ambient sea is uniform in every
/// direction, so a body that reaches the edge is the open sea.
fn ocean_window(world: &World, sea: &Sea) -> Option<([i32; 2], [i32; 2])> {
    let mut lo = [i32::MAX; 2];
    let mut hi = [i32::MIN; 2];
    let mut note = |x: i32, z: i32| {
        lo[0] = lo[0].min(x);
        lo[1] = lo[1].min(z);
        hi[0] = hi[0].max(x);
        hi[1] = hi[1].max(z);
    };
    for (bmin, bmax) in &sea.covered {
        note(bmin[0], bmin[2]);
        note(bmax[0], bmax[2]);
    }
    for set in [&world.solid, &world.tall, &world.use_gates, &world.flooded] {
        for c in set.iter() {
            note(c[0], c[2]);
        }
    }
    if lo[0] > hi[0] {
        return None;
    }
    Some((
        [lo[0] - OPEN_SEA_MARGIN, lo[1] - OPEN_SEA_MARGIN],
        [hi[0] + OPEN_SEA_MARGIN, hi[1] + OPEN_SEA_MARGIN],
    ))
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

    /// The linear "every earlier step is an ancestor" gate-ordering used by the
    /// synthetic gate tests (no parallel branches). Production routing uses the
    /// campaign's real DAG-causal predicate (`Plan::gate_fired_before`).
    fn linear(g: usize, s: usize) -> bool {
        g < s
    }

    /// Boundary safety (spec-0017 invariant 4): a walkable platform edge whose
    /// neighbour column has NOTHING below is a void drop → `DW0322`; ringing the
    /// platform with a 2-high (unjumpable) rim, or giving the neighbour column
    /// real geometry anywhere below (a deep drop onto land is falling, not
    /// leaving the world), passes.
    #[test]
    fn boundary_safety_flags_a_walkable_edge_over_void_dw0322() {
        // A bare 3×3 platform: every rim cell borders bottomless columns.
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let err = verify_boundary_safety(&world, &[[1, 64, 1]]).expect_err("edge borders void");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(err.message.contains("void drop"), "names the hazard");
    }

    #[test]
    fn boundary_safety_accepts_rimmed_platforms_and_deep_drops() {
        // (a) The same platform ringed by a 2-high rim (feet + head blocked, and
        // the rim top is +2 — unclimbable, so it never joins the walkable set).
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        for x in -1..4 {
            for z in -1..4 {
                if (0..3).contains(&x) && (0..3).contains(&z) {
                    continue;
                }
                solid.insert([x, 64, z]);
                solid.insert([x, 65, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        verify_boundary_safety(&world, &[[1, 64, 1]]).expect("a 2-high rim holds the line");

        // (b) A single floor cell whose four neighbour columns all have geometry
        // far below: a deep drop is legal (falling, not leaving the world).
        let mut solid = BTreeSet::new();
        solid.insert([0, 63, 0]);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            solid.insert([dx, 10, dz]);
        }
        let world = World::from_solid_cells(solid);
        verify_boundary_safety(&world, &[[0, 64, 0]]).expect("deep drops onto land are legal");
    }

    // -----------------------------------------------------------------------
    // DW0322 aggregation + the ocean horizon's stranding invariant
    // -----------------------------------------------------------------------

    /// `DW0322` reports **every** violation of a run, not the first: a stripped
    /// boundary is systemic, and the scale of the breach is the most useful fact
    /// about it. The bare 3×3 platform exposes 12 edge/void pairs over 12 columns
    /// (4 corners × 2 + 4 edges × 1); the message counts all of them and lists
    /// [`BOUNDARY_LIST_LIMIT`] before summarising the rest.
    #[test]
    fn boundary_safety_aggregates_every_void_drop_dw0322() {
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let err = verify_boundary_safety(&world, &[[1, 64, 1]]).expect_err("edge borders void");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("12 reachable walkable cell(s)"),
            "counts every violation, not just the first:\n{}",
            err.message
        );
        assert!(
            err.message.contains("12 distinct column(s)"),
            "counts the exposed columns:\n{}",
            err.message
        );
        assert_eq!(
            err.message.matches("void drop at").count(),
            BOUNDARY_LIST_LIMIT,
            "listing is bounded:\n{}",
            err.message
        );
        assert!(
            err.message.contains("and 6 more"),
            "summarises the tail:\n{}",
            err.message
        );
    }

    /// The `ocean` horizon's ambient: sea level 62, sea floor 54 (the pinned
    /// superflat `crate::plan::SEA_LEVEL` / `SEA_FLOOR_TOP_Y`), with `covered`
    /// standing in for the placed pieces' AABBs.
    fn ocean(solid: BTreeSet<[i32; 3]>, flooded: BTreeSet<[i32; 3]>, covered: Vec<Bbox>) -> World {
        World::from_solid_and_flooded(solid, flooded).with_ambient(Ambient::Ocean(Sea {
            level: 62,
            floor_top: 54,
            covered,
        }))
    }

    /// An inclusive world AABB, as `Sea::covered` carries it.
    type Bbox = ([i32; 3], [i32; 3]);

    /// A `size`×`size` island of one solid plate whose top block is at `top`,
    /// inside a piece AABB spanning y 60..=`top`.
    fn island(size: i32, top: i32) -> (BTreeSet<[i32; 3]>, Vec<Bbox>) {
        let mut solid = BTreeSet::new();
        for x in 0..size {
            for z in 0..size {
                for y in 60..=top {
                    solid.insert([x, y, z]);
                }
            }
        }
        (solid, vec![([0, 60, 0], [size - 1, top, size - 1])])
    }

    /// Ocean horizon (spec-0013), the false-premise fix: the pinned superflat
    /// puts bedrock under *every* column, so a coastline is not a void drop —
    /// the identical geometry is `DW0322` under `horizon: void` and clean under
    /// `horizon: ocean`, because its shore is a canonical beach (land top flush
    /// with the sea surface, walk plane at `sea_level + 1`).
    #[test]
    fn boundary_safety_ocean_beach_is_not_a_void_drop_dw0322() {
        let (solid, covered) = island(8, 62);
        let voidish = World::from_solid_cells(solid.clone());
        let err = verify_boundary_safety(&voidish, &[[3, 63, 3]])
            .expect_err("under `void` the same coast IS a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(err.message.contains("void drop"));

        let sea = ocean(solid, BTreeSet::new(), covered);
        verify_boundary_safety(&sea, &[[3, 63, 3]])
            .expect("an ocean beach is swimming, not falling out of the world");
    }

    /// The ocean horizon's replacement invariant: a sheer-cliff coast with no
    /// climb-out anywhere strands the player who steps off it, and that is
    /// `DW0322` — with every shore cell aggregated, not the first one.
    #[test]
    fn boundary_safety_ocean_sheer_cliff_strands_the_player_dw0322() {
        let (solid, covered) = island(8, 70);
        let world = ocean(solid, BTreeSet::new(), covered);
        let err = verify_boundary_safety(&world, &[[3, 71, 3]])
            .expect_err("a sheer coast cannot be re-climbed");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("NO way back ashore"),
            "names the stranding hazard:\n{}",
            err.message
        );
        assert!(
            err.message.contains("open sea"),
            "names the water body:\n{}",
            err.message
        );
        // 8×8 plateau: 28 distinct rim cells touch the water (a corner counts
        // once — the report is per shore *cell*, not per cell/direction pair).
        assert!(
            err.message.contains("28 reachable walkable cell(s)"),
            "aggregates every shore cell:\n{}",
            err.message
        );
        assert!(
            err.message.contains("and 22 more"),
            "bounded listing + tail count:\n{}",
            err.message
        );
    }

    /// The other admitted shoreline profile: a rim one block **under** the
    /// waterline (walk plane at `sea_level`, wade out of the shallows). Both it
    /// and the flush beach pass; a lip two blocks above the surface does not.
    #[test]
    fn boundary_safety_ocean_admits_a_rim_below_the_waterline() {
        let (solid, covered) = island(8, 61);
        let world = ocean(solid, BTreeSet::new(), covered);
        verify_boundary_safety(&world, &[[3, 62, 3]]).expect("wade out of the shallows");

        // One block higher than the flush beach: the swimmer faces a wall.
        let (solid, covered) = island(8, 64);
        let world = ocean(solid, BTreeSet::new(), covered);
        let err = verify_boundary_safety(&world, &[[3, 65, 3]])
            .expect_err("a lip 2 above the surface is not a climb-out");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
    }

    /// Stranding is proven **per body of water**, not globally: an island whose
    /// outer coast is a perfect beach still fails if it contains an inner pool
    /// the player can walk into and not climb out of. A global "is there a
    /// climb-out anywhere" test would pass this world.
    #[test]
    fn boundary_safety_ocean_enclosed_pool_is_checked_separately_dw0322() {
        // Outer plate: top 62 (flush beach, walk plane 63) over 0..=12.
        let mut solid = BTreeSet::new();
        for x in 0..=12 {
            for z in 0..=12 {
                for y in 60..=62 {
                    solid.insert([x, y, z]);
                }
            }
        }
        // Inner plateau one step up (top 63, walk plane 64) over 3..=9, with a
        // 3×3 shaft at 5..=7 down to a pool at sea level.
        let mut flooded = BTreeSet::new();
        for x in 3..=9 {
            for z in 3..=9 {
                if (5..=7).contains(&x) && (5..=7).contains(&z) {
                    solid.remove(&[x, 62, z]);
                    flooded.insert([x, 62, z]);
                } else {
                    solid.insert([x, 63, z]);
                }
            }
        }
        let world = ocean(solid, flooded, vec![([0, 60, 0], [12, 63, 12])]);
        let err = verify_boundary_safety(&world, &[[1, 63, 1]])
            .expect_err("the inner pool has 2-high walls all round");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("enclosed water"),
            "names the enclosed body, not the (escapable) open sea:\n{}",
            err.message
        );
        assert!(
            !err.message.contains("open sea"),
            "the outer beach is proven fine and must not be reported:\n{}",
            err.message
        );
        assert!(
            err.message.contains("1 body/bodies of water"),
            "exactly one failing body:\n{}",
            err.message
        );

        // Lower the inner plateau to the outer datum and the pool is flush with
        // its bank: the player steps straight back out.
        let mut solid = BTreeSet::new();
        for x in 0..=12 {
            for z in 0..=12 {
                for y in 60..=62 {
                    solid.insert([x, y, z]);
                }
            }
        }
        let mut flooded = BTreeSet::new();
        for x in 5..=7 {
            for z in 5..=7 {
                solid.remove(&[x, 62, z]);
                flooded.insert([x, 62, z]);
            }
        }
        let world = ocean(solid, flooded, vec![([0, 60, 0], [12, 62, 12])]);
        verify_boundary_safety(&world, &[[1, 63, 1]])
            .expect("a flush pool is a puddle, not a trap");
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

    /// Whether an entity of `width` standing (feet) at `p` has any part of its AABB
    /// inside a solid cell. Height 1.95 (the player/villager box).
    fn aabb_clips(world: &World, p: [f64; 3], width: f64) -> bool {
        let span = |c: f64| {
            (
                (c - width / 2.0).floor() as i32,
                (c + width / 2.0 - 1e-9).floor() as i32,
            )
        };
        let (x0, x1) = span(p[0]);
        let (z0, z1) = span(p[2]);
        let (y0, y1) = (p[1].floor() as i32, (p[1] + 1.95 - 1e-9).floor() as i32);
        (x0..=x1).any(|x| (z0..=z1).any(|z| (y0..=y1).any(|y| world.solid_at([x, y, z]))))
    }

    /// The full walked path for `cells`, as the emitter would teleport it.
    fn walked(cells: &[[i32; 3]]) -> Vec<[f64; 3]> {
        resample(cells, DEFAULT_SPEED)
    }

    /// **Regression (owner, island QA): "the NPC visibly passes through blocks".**
    ///
    /// A 1-wide corridor with solid walls on both sides. Every planned waypoint must
    /// keep the mover's whole AABB out of the walls. The bare integer cell
    /// coordinate — what the emitter used before `cell_center` — puts 70 % of a
    /// 0.6-wide body inside the neighbouring column, i.e. inside the wall, for the
    /// entire walk; the second half of this test asserts exactly that, so the defect
    /// cannot silently come back.
    #[test]
    fn walked_path_keeps_the_body_out_of_corridor_walls() {
        let y = 65;
        let mut walls = Vec::new();
        for z in 0..8 {
            for dy in 0..2 {
                walls.push([0, y + dy, z]); // west wall
                walls.push([2, y + dy, z]); // east wall
            }
        }
        let world = floored(3, 8, y, &walls);
        let cells: Vec<[i32; 3]> = (0..8).map(|z| [1, y, z]).collect();
        let path = world
            .find_path(cells[0], *cells.last().unwrap())
            .expect("the corridor is walkable");
        assert_eq!(path, cells, "a 1-wide corridor has exactly one route");

        for w in walked(&path) {
            assert!(
                !aabb_clips(&world, w, 0.6),
                "waypoint {w:?} puts the body inside a corridor wall"
            );
        }
        // The pre-fix emission (bare cell coordinates) DID clip — the defect this
        // test guards. Keep as the counter-example, never as the behaviour.
        assert!(
            aabb_clips(&world, [1.0, y as f64, 3.0], 0.6),
            "a body at the bare integer cell straddles the wall columns"
        );
    }

    /// An L-shaped corridor whose inside corner is solid. A* is strictly cardinal
    /// (`neighbors_fp` offers 4 horizontal moves, never a diagonal), so no path can
    /// cut the corner; this pins that property *and* proves the interpolated body
    /// never enters the corner block on the turn.
    #[test]
    fn corner_turn_routes_around_the_corner_block_not_through_it() {
        let y = 65;
        // Open cells: the column z=1..=4 at x=1, then x=1..=4 at z=4. Everything
        // else at head height is solid, including the inside corner [2, y, 1].
        let open: BTreeSet<[i32; 3]> = (1..=4)
            .map(|z| [1, y, z])
            .chain((1..=4).map(|x| [x, y, 4]))
            .collect();
        let mut walls = Vec::new();
        for x in 0..6 {
            for z in 0..6 {
                for dy in 0..2 {
                    if !open.contains(&[x, y, z]) {
                        walls.push([x, y + dy, z]);
                    }
                }
            }
        }
        let world = floored(6, 6, y, &walls);
        let path = world
            .find_path([1, y, 1], [4, y, 4])
            .expect("the L-corridor is walkable");
        assert!(
            path.iter().all(|c| open.contains(c)),
            "the route must stay in the open cells: {path:?}"
        );
        for w in walked(&path) {
            assert!(
                !aabb_clips(&world, w, 0.6),
                "waypoint {w:?} clips the corner block"
            );
        }
    }

    /// A one-block step up is interpolated as an **L** (rise over the source column,
    /// then cross), not a diagonal lerp: a straight line between the two cell centres
    /// drags the body through the corner of the step block. Both legs stay inside
    /// cells the neighbour rule already proved clear (`standable_fp` + the jump
    /// head-clearance check), so the AABB never enters the step.
    #[test]
    fn vertical_step_is_l_shaped_and_never_clips_the_step_block() {
        let y = 65;
        let mut solid = BTreeSet::new();
        for x in 0..4 {
            for z in 0..3 {
                solid.insert([x, y - 1, z]); // lower floor
                solid.insert([x, y + 4, z]); // ceiling, clear of both levels
            }
        }
        // A raised ledge at x∈{2,3}: its top face is the upper walking surface.
        for x in [2, 3] {
            for z in 0..3 {
                solid.insert([x, y, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let path = world
            .find_path([1, y, 1], [3, y + 1, 1])
            .expect("a one-block step up is walkable");
        assert_eq!(path, vec![[1, y, 1], [2, y + 1, 1], [3, y + 1, 1]]);

        let pts = walked(&path);
        // The rise happens over the SOURCE column: some waypoint sits at the source
        // cell's centre already at the upper height.
        let src = cell_center([1, y, 1]);
        assert!(
            pts.iter()
                .any(|w| w[0] == src[0] && w[2] == src[2] && w[1] > y as f64),
            "the step up must rise in place before crossing: {pts:?}"
        );
        for w in &pts {
            assert!(
                !aabb_clips(&world, *w, 0.6),
                "waypoint {w:?} clips the step block"
            );
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
        let err = route_visited(&world, &[vp(a, false), vp(b, false)], &[], &linear).unwrap_err();
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
        // Same leg ridden by an inter-area transport → skipped, ok.
        assert!(route_visited(&world, &[vp(a, false), vp(b, true)], &[], &linear).is_ok());
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
            use_gates: Vec::new(),
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
            use_gates: Vec::new(),
        }];
        assert!(verify_exported_routes(&world, &dry).is_ok());
    }

    #[test]
    fn critical_path_routable_leg_passes() {
        // A flat connected floor: consecutive visited cells are walkable → ok.
        let world = floored(6, 3, 65, &[]);
        assert!(
            route_visited(
                &world,
                &[vp([0, 65, 1], false), vp([5, 65, 1], false)],
                &[],
                &linear
            )
            .is_ok()
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

    // --- collision-accurate standability: fences / walls / fence gates (task #59) ---

    /// A world from explicit collision classes (task #59): a flat solid floor at
    /// `y-1` over `[0,w) × [0,d)` with the given `tall` (fence/wall) and
    /// `use_gates` (closed fence gate) cells at stand level.
    fn classified(w: i32, d: i32, y: i32, tall: &[[i32; 3]], use_gates: &[[i32; 3]]) -> World {
        let mut solid = BTreeSet::new();
        for x in 0..w {
            for z in 0..d {
                solid.insert([x, y - 1, z]);
            }
        }
        World::from_occupancy(crate::assembled::Occupancy {
            solid,
            tall: tall.iter().copied().collect(),
            use_gates: use_gates.iter().copied().collect(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
        })
    }

    /// The gateless ram-pen shape: a closed fence ring at stand level around an
    /// interior anchor. `gate`, when set, replaces one ring cell with a closed
    /// fence gate (a use-gate cell).
    fn fence_ring_world(gate: Option<[i32; 3]>) -> World {
        let y = 65;
        let mut ring: Vec<[i32; 3]> = Vec::new();
        for i in 1..=5 {
            ring.push([i, y, 1]);
            ring.push([i, y, 5]);
            ring.push([1, y, i]);
            ring.push([5, y, i]);
        }
        let gates: Vec<[i32; 3]> = gate.into_iter().collect();
        ring.retain(|c| !gates.contains(c));
        classified(7, 7, y, &ring, &gates)
    }

    #[test]
    fn fence_top_is_not_standable_and_fence_is_not_passable() {
        // The owner-hit island bug, modelled: a 1.5-tall oak_fence is neither a
        // floor (no walking player can jump 1.5 onto its top) nor a passable cell.
        let world = classified(3, 1, 65, &[[1, 65, 0]], &[]);
        assert!(
            !world.standable([1, 66, 0]),
            "a fence-top cell must not be standable (the old full-solid model's bug)"
        );
        assert!(
            !world.standable([1, 65, 0]),
            "the fence cell itself must not be passable"
        );
        // The two floor cells beside it are fine but no longer connected.
        assert!(world.standable([0, 65, 0]) && world.standable([2, 65, 0]));
        assert!(
            world.find_path([0, 65, 0], [2, 65, 0]).is_none(),
            "no route through or over a fence line"
        );
    }

    #[test]
    fn gateless_fence_ring_is_dw0311() {
        // The soundness hole the full-solid model had: a pen fenced on every side
        // with NO gate "passed" the completability proof by standing the player on
        // the fence-top. It must now be a DW0311 build failure.
        let world = fence_ring_world(None);
        let inside = [3, 65, 3];
        let outside = [0, 65, 0]; // corner, outside the ring
        assert!(world.standable(inside) && world.standable(outside));
        let err = route_visited(
            &world,
            &[vp(outside, false), vp(inside, false)],
            &[],
            &linear,
        )
        .expect_err("a humanly impassable gateless fence ring must fail the proof");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
        assert!(
            err.message.contains("fence"),
            "the message should name the barrier class: {}",
            err.message
        );
    }

    #[test]
    fn fence_ring_with_closed_gate_routes_through_it_as_a_use_gate_edge() {
        // The island ram pen: the ring's only opening is a closed oak_fence_gate.
        // The player passes it with an adventure-legal right-click, so the proof
        // routes THROUGH the gate cell — a first-class use-gate edge, not a
        // fence-top hop and not a harness workaround.
        let gate = [3, 65, 1];
        let world = fence_ring_world(Some(gate));
        let inside = [3, 65, 3];
        let outside = [3, 65, 0];
        let path = world
            .find_path(outside, inside)
            .expect("the pen is enterable through its gate");
        assert!(
            path.contains(&gate),
            "the proven route must pass through the gate cell: {path:?}"
        );
        assert!(world.is_use_gate(gate), "the gate cell is tagged use-gate");
        // Every route cell is standable in the final model (the DW0314 guard).
        for &c in &path {
            assert!(world.is_standable(c), "route cell {c:?} standable");
        }
        assert!(
            route_visited(
                &world,
                &[vp(outside, false), vp(inside, false)],
                &[],
                &linear
            )
            .is_ok()
        );
        // The gate is still never a floor: its top is not standable.
        assert!(!world.standable([3, 66, 1]));
    }

    #[test]
    fn autonomous_walkers_treat_a_closed_gate_as_a_fence() {
        // A wave mob acting on its own cannot right-click: on the no-gate-use view
        // (wave seating) the pen is sealed again.
        let gate = [3, 65, 1];
        let world = fence_ring_world(Some(gate));
        let entity_world = world.without_gate_use();
        assert!(world.has_use_gates() && !entity_world.has_use_gates());
        assert!(
            entity_world.find_path([3, 65, 0], [3, 65, 3]).is_none(),
            "a non-player walker must not route through a closed gate"
        );
        // Wave seating never picks the gate threshold or anything past it.
        let cells = entity_world.confined_standable_cells([3, 65, 3], ([2, 64, 2], [4, 66, 4]));
        assert!(!cells.is_empty());
        assert!(!cells.contains(&gate), "no mob seated in the gate cell");
    }

    #[test]
    fn open_fence_gate_is_a_passable_threshold_with_no_use_tag() {
        // An authored-open gate (block state open=true) is just a passable cell:
        // no use-gate tag, and even gate-incapable walkers pass it.
        let world = classified(3, 1, 65, &[], &[]); // flat; the "gate" cell is plain air
        assert!(world.find_path([0, 65, 0], [2, 65, 0]).is_some());
        assert!(!world.is_use_gate([1, 65, 0]));
    }

    #[test]
    fn camera_dolly_clips_fences_and_closed_gates() {
        // A fence contains visible geometry: a cutscene camera flying through its
        // cell is a DW0308 clip, exactly like a full solid — and so is a closed
        // fence gate (the camera would fly through the gate leaves).
        let world = classified(5, 3, 65, &[[2, 65, 1]], &[[2, 65, 2]]);
        let through_fence = [[0.5, 65.5, 1.5], [4.5, 65.5, 1.5]];
        assert_eq!(first_clip(&world, &through_fence), Some((0, [2, 65, 1])));
        let through_gate = [[0.5, 65.5, 2.5], [4.5, 65.5, 2.5]];
        assert_eq!(first_clip(&world, &through_gate), Some((0, [2, 65, 2])));
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
        // Endpoints are the CENTRES of the start/goal cells, not their corners:
        // a body positioned on the integer cell coordinate straddles four columns.
        assert_eq!(*slow.last().unwrap(), cell_center([10, 65, 0]));
        assert_eq!(slow[0], cell_center([0, 65, 0]));
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
            route_visited(&world, &[a, b], &[], &linear).is_ok(),
            "the open corridor must route with no gate events"
        );
        // A close-gate seals the pass-through before the leg to `b` (fire_step 0 < 2).
        let close = GateEvent {
            region: ([2, 65, 0], [2, 65, 0]),
            closes: true,
            fire_step: 0,
        };
        let err =
            route_visited(&world, &[a, b], std::slice::from_ref(&close), &linear).unwrap_err();
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
            route_visited(&world, &[a, b], &[close, open], &linear).is_ok(),
            "a gate reopened by open-gate before the leg must route again"
        );
    }

    /// The seal is **DAG-causal**, not linear: a `close-gate` fired on a parallel
    /// quest branch (not a causal ancestor of the leg) must NOT seal it, even though
    /// its `fire_step` is numerically earlier — the fix for the lineariser
    /// interleaving a sibling branch ahead of a sealed leg (island `take-the-cheese`
    /// vs `hide`). A genuinely-forced causal re-crossing is still sealed.
    #[test]
    fn close_gate_seal_is_dag_causal_not_linear() {
        let world = floored(5, 1, 65, &[]);
        let close = GateEvent {
            region: ([2, 65, 0], [2, 65, 0]),
            closes: true,
            fire_step: 8,
        };
        let a = at_step([0, 65, 0], 9);
        let b = at_step([4, 65, 0], 10);
        // Parallel: neither the close (step 8) nor the prior position (step 9) is a
        // causal ancestor of the arrival (step 10) — a cross-branch artifact leg.
        let parallel = |g: usize, s: usize| !((g == 8 || g == 9) && s == 10) && g < s;
        assert!(
            route_visited(&world, &[a, b], std::slice::from_ref(&close), &parallel).is_ok(),
            "a close on a parallel branch must not seal a non-causal leg"
        );
        // Causal: step 8 (close) and step 9 are ancestors of step 10 (a forced
        // re-crossing with no reopen) → sealed → DW0311 (proof preserved).
        let err = route_visited(&world, &[a, b], std::slice::from_ref(&close), &linear)
            .expect_err("a forced causal re-crossing of a sealed gate must fail");
        assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
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
        assert!(verify_checkpoints(&world, &cps, &positions, &[], &linear).is_ok());
        // Sealed before the party reaches the target (fire_step 0 < 1) → stranded.
        let close = GateEvent {
            region: ([2, 65, 0], [2, 65, 0]),
            closes: true,
            fire_step: 0,
        };
        let err = verify_checkpoints(
            &world,
            &cps,
            &positions,
            std::slice::from_ref(&close),
            &linear,
        )
        .unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    #[test]
    fn checkpoint_behind_a_one_way_drop_is_dw0315() {
        // Checkpoint on the near patch; the next required anchor is on the far,
        // disconnected patch → not walkable from the checkpoint → DW0315.
        let world = split_world(65);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        let err = verify_checkpoints(&world, &cps, &positions, &[], &linear).unwrap_err();
        assert_eq!(err.code, DW_CHECKPOINT_STRANDED); // DW0315
    }

    #[test]
    fn checkpoint_with_reachable_remaining_path_passes() {
        // Both the checkpoint and the next anchor sit on the same connected floor.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [0, 65, 1], 0usize)];
        let positions = vec![at_step([4, 65, 1], 1)];
        assert!(verify_checkpoints(&world, &cps, &positions, &[], &linear).is_ok());
    }

    #[test]
    fn checkpoint_over_void_is_dw0316() {
        // The checkpoint cell has no standable floor within snap radius.
        let world = floored(5, 3, 65, &[]);
        let cps = vec![("cp/rest".to_string(), [20, 65, 20], 0usize)];
        let err = verify_checkpoints(&world, &cps, &[], &[], &linear).unwrap_err();
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

    // --- DW0355: stealth onset survivability --------------------------------

    /// The island defect in miniature: cover EXISTS and is reachable (DW0327 is
    /// happy) but is too far to reach inside the grace window, so the beat kills
    /// every player a fixed moment after it arms.
    #[test]
    fn stealth_zone_out_of_sprint_range_at_onset_is_dw0352() {
        // A 40-long corridor; the beat arms at x=0, the only zone sits at x=39.
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [39, 65, 1], [1, 1, 1])];
        // Reachability alone passes — this is exactly the gap DW0355 closes.
        assert!(
            verify_stealth(&world, &[(zones.clone(), 0)], &[at_step([0, 65, 1], 0)]).is_ok(),
            "DW0327 must be satisfied, so the failure below is purely a timing one"
        );
        let starts = vec![("the activating objective's anchor".to_string(), [0, 65, 1])];
        let err = verify_stealth_onset(&world, &zones, 50, &starts, 1)
            .expect_err("cover 38 blocks away cannot be reached in 50 ticks");
        assert_eq!(err.code, DW_STEALTH_ONSET); // DW0355
        assert!(
            err.message.contains("zone/alcove") && err.message.contains("short by"),
            "the diagnostic names the nearest zone and the tick deficit: {}",
            err.message
        );
        // The deficit is measured, not guessed: 38 blocks × 4 t + 10 t reaction.
        assert!(
            err.message.contains("152 ticks of sprinting"),
            "the sprint cost is the nav-model measurement: {}",
            err.message
        );
    }

    /// A checkpoint that respawns the party into a running punishing beat is a
    /// start position too — if IT cannot beat the window, the retry loop never
    /// terminates (a broken beat, not a souls retry).
    #[test]
    fn checkpoint_respawning_into_a_running_beat_must_beat_the_window_dw0352() {
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [2, 65, 1], [1, 1, 1])];
        // The activating anchor is next to cover; the respawn point is not.
        let starts = vec![
            ("the activating objective's anchor".to_string(), [0, 65, 1]),
            ("checkpoint `cp/below` respawn".to_string(), [39, 65, 1]),
        ];
        let err = verify_stealth_onset(&world, &zones, 50, &starts, 1)
            .expect_err("a respawn point outside sprint range of cover is a death loop");
        assert_eq!(err.code, DW_STEALTH_ONSET);
        assert!(
            err.message.contains("cp/below"),
            "the diagnostic names the offending checkpoint: {}",
            err.message
        );
    }

    /// A climb on the flee route is charged its jump arc: the same horizontal
    /// distance costs more when cover is up a step, which is what made the
    /// island's ramp-top zone unreachable in time.
    #[test]
    fn stealth_onset_charges_the_climb() {
        // Floor at y=65 with a step up to y=66 at x=5..7 (a 2-block rise).
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..3 {
                solid.insert([x, 64, z]);
            }
        }
        for x in 5..8 {
            for z in 0..3 {
                solid.insert([x, 65, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let zones = vec![("zone/ledge".to_string(), [7, 66, 1], [0, 0, 0])];
        let starts = vec![("the activating objective's anchor".to_string(), [0, 65, 1])];
        // 7 horizontal steps (28 t) + one +1 climb (6 t) = 34 t, +10 reaction = 44.
        let err = verify_stealth_onset(&world, &zones, 40, &starts, 1)
            .expect_err("34 t of flee + 10 t reaction exceeds a 40-tick window");
        assert_eq!(err.code, DW_STEALTH_ONSET);
        assert!(
            err.message.contains("34 ticks of sprinting"),
            "the climb is charged its jump arc: {}",
            err.message
        );
        // Widening the window to the measured need discharges the obligation.
        assert!(
            verify_stealth_onset(&world, &zones, 44, &starts, 1).is_ok(),
            "grace sized to the measured need must pass"
        );
    }

    /// Cover inside the window passes — the green half of the story.
    #[test]
    fn stealth_onset_within_the_grace_window_passes() {
        let world = floored(40, 3, 65, &[]);
        let zones = vec![("zone/alcove".to_string(), [5, 65, 1], [1, 1, 1])];
        let starts = vec![
            ("the activating objective's anchor".to_string(), [0, 65, 1]),
            ("checkpoint `cp/below` respawn".to_string(), [8, 65, 1]),
        ];
        // 4 blocks to the zone edge = 16 t (+10) from the anchor; 4 t (+10) from cp.
        assert!(verify_stealth_onset(&world, &zones, 30, &starts, 1).is_ok());
    }

    /// The obligation is scoped to beats that actually punish: a `begin-stealth`
    /// whose `on_caught` only narrates has nothing to escape, so unreachable-in-time
    /// cover is atmosphere, not a broken beat.
    #[test]
    fn a_stealth_beat_that_only_narrates_is_not_punishing() {
        use delvewright_dsl::QuestEffect;
        let beat = |on_caught: Vec<QuestEffect>| crate::plan::StealthBeat {
            index: 1,
            zones: vec![("zone/alcove".to_string(), [0, 65, 0], [1, 1, 1])],
            on_caught,
            grace_ticks: 20,
            fire_step: 0,
            end_step: None,
        };
        assert!(
            !beat(vec![QuestEffect::Narrate {
                text: "Spotted!".to_string(),
                style: None,
                sound: None,
                requires_flags: Vec::new(),
                forbids_flags: Vec::new(),
            }])
            .is_punishing(),
            "a narrate-only on_caught carries no timing obligation"
        );
        assert!(
            beat(vec![QuestEffect::DamagePlayers {
                amount: 40,
                within: None,
                damage_type: None,
                requires_flags: Vec::new(),
                forbids_flags: Vec::new(),
            }])
            .is_punishing(),
            "damage-players makes the beat punishing"
        );
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

    /// A 1-wide, ceilinged corridor along x at z=1 (walls at z=0 and z=2, floor
    /// at y=64, ceiling at y=67). A body standing in it cannot be climbed over —
    /// the headroom above a blocked cell is the ceiling.
    fn walled_corridor() -> World {
        let mut walls = Vec::new();
        for x in 0..9 {
            for y in [65, 66] {
                walls.push([x, y, 0]);
                walls.push([x, y, 2]);
            }
        }
        floored(9, 3, 65, &walls)
    }

    fn timed_gate(
        region: ([i32; 3], [i32; 3]),
        open_ticks: u32,
        closed_ticks: u32,
    ) -> crate::plan::TimedGatePlan {
        crate::plan::TimedGatePlan {
            id: "timed-gate/piston-hall".to_string(),
            safe: "piston_hall".to_string(),
            gate_anchor: "anchor/gate".to_string(),
            gate_region: region,
            gate_block: "minecraft:iron_bars".to_string(),
            open_ticks,
            closed_ticks,
            phase: 0,
        }
    }

    /// A generous window: crossing a 1-cell doorway costs a handful of ticks and
    /// the gate stands open for 60 of every 100, so most of the cycle is a legal
    /// entry. A readable gate.
    #[test]
    fn timed_gate_with_a_generous_window_is_readable() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 60, 40);
        verify_timed_gates(&world, &[g]).expect("60 open of a 100-tick cycle is a timing read");
    }

    /// The same span with an open window barely longer than the crossing itself:
    /// almost every entry phase is a death, so the gate is a coin flip. `DW0378`.
    #[test]
    fn timed_gate_whose_window_barely_admits_a_crossing_is_dw0378() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        // Crossing the doorway is 2 moves = 8 ticks; a 10-tick open window inside
        // a 200-tick cycle admits 3 of 200 phases = 1%.
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 10, 190);
        let err = verify_timed_gates(&world, &[g])
            .expect_err("a window that admits ~1% of the cycle is a slot machine");
        assert_eq!(err.code, DW_TIMED_GATE_COIN_FLIP); // DW0378
        assert!(
            err.message.contains("coin flip"),
            "the message must name the failure: {}",
            err.message
        );
    }

    /// An open window SHORTER than the crossing admits nothing at all — the
    /// degenerate end of the same rule, and the one a player can never learn.
    #[test]
    fn timed_gate_no_one_can_ever_cross_is_dw0378() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let g = timed_gate(([1, 65, 4], [1, 66, 4]), 2, 20);
        let err = verify_timed_gates(&world, &[g])
            .expect_err("a window shorter than the crossing admits no phase at all");
        assert_eq!(err.code, DW_TIMED_GATE_COIN_FLIP); // DW0378
    }

    fn ambush(at: [i32; 3], actor_cells: Vec<[i32; 3]>) -> crate::plan::AmbushPlan {
        crate::plan::AmbushPlan {
            id: "ambush/stair-turn".to_string(),
            at,
            actor_cells,
        }
    }

    /// An ambush in an open room: the ambusher stands beside the player, so a
    /// retreat to the entry is still walkable. Un-telegraphed and lethal is fine
    /// — there is a play on the retry, which is all the engine owes.
    #[test]
    fn ambush_in_open_ground_has_counterplay() {
        let world = floored(9, 9, 65, &[]);
        let amb = ambush([4, 65, 4], vec![[5, 65, 4]]);
        verify_ambushes(&world, &[amb], &[[0, 65, 0]])
            .expect("an ambusher in open ground never seals the room");
    }

    /// A 1-wide corridor with the ambusher between the player and everything
    /// behind them: no retreat, no luring ground, no exit. `DW0376`.
    #[test]
    fn ambush_that_seals_the_only_way_out_is_dw0376() {
        let world = walled_corridor();
        // Player at the dead end (x=8); the ambusher spawns at x=7, behind them.
        let amb = ambush([8, 65, 1], vec![[7, 65, 1]]);
        let err = verify_ambushes(&world, &[amb], &[[0, 65, 1]])
            .expect_err("an ambush that corks the only corridor has no counterplay");
        assert_eq!(err.code, DW_AMBUSH_NO_COUNTERPLAY); // DW0376
        assert!(
            err.message.contains("no counterplay"),
            "the message must name the missing play: {}",
            err.message
        );
    }

    /// The same corridor, but a bonfire sits at the dead end WITH the player:
    /// dying is now cheap and the retry is a real second attempt, so the beat
    /// carries no obligation.
    #[test]
    fn ambush_with_a_rest_point_on_the_players_side_has_counterplay() {
        let world = walled_corridor();
        let amb = ambush([8, 65, 1], vec![[7, 65, 1]]);
        verify_ambushes(&world, &[amb], &[[0, 65, 1], [8, 65, 1]])
            .expect("a rest point on the player's own side is a play");
    }

    /// A synthetic shortcut-door world (spec-0016 §2): a room `w × d` split by a
    /// solid wall at `z = zw`, with a 1-cell **gate** doorway at `x = gx` and an
    /// optional **bypass** hole at `x = bx` (the long way round). The gate cells
    /// are open in the base world — the assembled model always clears a gate
    /// region — and the proof re-seals them itself.
    fn shortcut_world(w: i32, d: i32, y: i32, zw: i32, gx: i32, bypass: Option<i32>) -> World {
        let mut walls = Vec::new();
        for x in 0..w {
            if x == gx || Some(x) == bypass {
                continue;
            }
            walls.push([x, y, zw]);
            walls.push([x, y + 1, zw]);
        }
        floored(w, d, y, &walls)
    }

    /// A shortcut plan over a 1-cell gate column at `(gx, y..y+1, zw)`.
    fn shortcut(gx: i32, y: i32, zw: i32, unlock: [i32; 3]) -> crate::plan::ShortcutPlan {
        crate::plan::ShortcutPlan {
            id: "shortcut/lift".to_string(),
            safe: "lift".to_string(),
            gate_anchor: "anchor/gate".to_string(),
            gate_region: ([gx, y, zw], [gx, y + 1, zw]),
            gate_block: "minecraft:iron_bars".to_string(),
            unlock_anchor: "anchor/lift-lever".to_string(),
            unlock,
            on_unlock: Vec::new(),
        }
    }

    /// The happy path: a wall with a barred doorway AND a far bypass hole. The
    /// unlock is reachable the long way while the gate is sealed, and opening the
    /// gate genuinely shortens the crossing — a real shortcut.
    #[test]
    fn shortcut_with_a_long_way_round_passes_both_proofs() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        let sc = shortcut(1, 65, 4, [1, 65, 7]);
        verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect("a gate with a genuine detour around it is a real shortcut");
    }

    /// No bypass: sealing the gate cuts the room in two, so the unlock on the far
    /// side can never be reached the hard way — the mechanism that opens the
    /// shortcut is behind the shortcut. `DW0373`.
    #[test]
    fn shortcut_whose_unlock_is_only_behind_its_own_gate_is_dw0373() {
        let world = shortcut_world(12, 9, 65, 4, 1, None);
        let sc = shortcut(1, 65, 4, [1, 65, 7]);
        let err = verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect_err("a shortcut with no long route must fail");
        assert_eq!(err.code, DW_SHORTCUT_NO_LONG_ROUTE); // DW0373
        assert!(
            err.message.contains("no long route"),
            "the message must name the missing long route: {}",
            err.message
        );
    }

    /// The classic leak: the `unlock` sits on the NEAR side of its own gate, so
    /// the player can pull it without ever earning the far side — and opening the
    /// gate measurably changes nothing about reaching it. `DW0374`.
    #[test]
    fn shortcut_whose_unlock_is_on_the_near_side_is_dw0374() {
        let world = shortcut_world(12, 9, 65, 4, 1, Some(10));
        // Entry z=1, wall z=4: an unlock at z=2 is on the entry's own side.
        let sc = shortcut(1, 65, 4, [5, 65, 2]);
        let err = verify_shortcuts(&world, &[sc], Some([1, 65, 1]))
            .expect_err("an unlock the gate does not stand in front of is a leak");
        assert_eq!(err.code, DW_SHORTCUT_NO_GAIN); // DW0374
        assert!(
            err.message.contains("leaks"),
            "the message must name the leak: {}",
            err.message
        );
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
            lethality: Lethality::Lethal,
            reset,
            disarm,
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
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

    // --- partial floor heights: a physical step rule (task #78) ---------------

    /// A world from an explicit cell→block map, through the real classifier — the
    /// only way to exercise the partial-height model end to end.
    fn blocks_world(cells: &[([i32; 3], &str)]) -> World {
        let map: BTreeMap<[i32; 3], String> =
            cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect();
        World::from_occupancy(crate::assembled::occupancy_of(map, &BTreeSet::new()))
    }

    #[test]
    fn step_up_from_a_bottom_slab_onto_a_full_block_is_impossible() {
        // THE regression the full-cube model proved wrong. Standing on a bottom
        // slab puts the feet at y=65.5; the neighbouring ledge's top face is at
        // y=67.0 — a **1.5-block** rise, past the ~1.25-block jump apex. The old
        // model saw an ordinary "+1 cell" step (feet cell 66 → 67) and proved a
        // route no player and no mineflayer bot can walk.
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"), // support under the slab
            ([0, 65, 0], "minecraft:oak_slab[type=bottom]"), // stand at y=65.5
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
            ([1, 66, 0], "minecraft:stone"), // ledge top at y=67.0
        ]);
        // Both standing cells are standable in isolation…
        assert!(world.is_standable([0, 66, 0]), "the slab top is standable");
        assert!(world.is_standable([1, 67, 0]), "the ledge top is standable");
        // …but no step connects them: 1.5 blocks is not jumpable.
        assert!(
            !world.neighbors([0, 66, 0]).contains(&[1, 67, 0]),
            "a 1.5-block rise must not be a legal step: {:?}",
            world.neighbors([0, 66, 0])
        );
        assert!(
            world.find_path([0, 66, 0], [1, 67, 0]).is_none(),
            "no route may cross an unjumpable rise"
        );
        // The same ledge one cell lower IS reachable — a 0.5-block auto-step off
        // the slab. The rule rejects the impossible rise, not the block kind.
        let ok = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:oak_slab[type=bottom]"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
        ]);
        assert!(
            ok.neighbors([0, 66, 0]).contains(&[1, 66, 0]),
            "slab top → full block top is a 0.5 auto-step: {:?}",
            ok.neighbors([0, 66, 0])
        );
    }

    #[test]
    fn step_up_onto_a_bottom_slab_needs_no_jump_headroom() {
        // The other direction — a step vanilla ADMITS that the full-cube model
        // refused. From a full floor onto a bottom slab is a 0.5-block rise: an
        // auto-step (vanilla `maxUpStep` 0.6), not a jump, so a ceiling directly
        // over the walker's jump arc is irrelevant. The old rule treated it as a
        // "+1 cell" jump and demanded head clearance that a real player never
        // needs.
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"), // stand at y=65
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:oak_slab[type=bottom]"), // slab top y=65.5
            ([0, 67, 0], "minecraft:stone"),                 // ceiling over the source
        ]);
        assert!(world.is_standable([0, 65, 0]));
        assert!(world.is_standable([1, 66, 0]));
        assert!(
            world.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
            "a 0.5-block auto-step must be legal even under a low ceiling: {:?}",
            world.neighbors([0, 65, 0])
        );
    }

    #[test]
    fn top_slab_and_double_slab_are_full_height_steps() {
        // A `type=top` slab's walkable face IS the cell top, so stepping onto it
        // from a full floor one cell down is an ordinary 1.0-block jump — legal
        // with headroom. The half-step rule must key on the slab HALF, not on the
        // word "slab".
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:oak_slab[type=top]"),
        ]);
        assert!(
            world.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
            "a top slab is a full-height step up: {:?}",
            world.neighbors([0, 65, 0])
        );
        // And from a top slab, the next full block one cell up is a normal 1.0
        // rise — unlike the bottom-slab case above, this stays legal.
        let world = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:oak_slab[type=top]"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
        ]);
        assert!(
            world.neighbors([0, 66, 0]).contains(&[1, 66, 0]),
            "top slab → full block at the same standing cell is level"
        );
    }

    #[test]
    fn snow_layers_step_by_layer_count_and_thin_snow_is_walked_over() {
        // `snow` collision is `(layers-1)*2/16`: one layer has NO collision box at
        // all (walked straight over — the floor is what is under it), five layers
        // is a half-block auto-step, and a deep drift plus a full block above is
        // past the jump apex.
        let thin = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:snow[layers=1]"),
        ]);
        assert!(
            thin.is_standable([0, 65, 0]),
            "a single snow layer is walked through, not stood on top of"
        );
        let drift = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:snow[layers=5]"), // top at +0.5
        ]);
        assert!(
            drift.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
            "a 5-layer drift is a 0.5-block auto-step: {:?}",
            drift.neighbors([0, 65, 0])
        );
        let over = blocks_world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([0, 65, 0], "minecraft:snow[layers=5]"), // stand at y=65.5
            ([1, 64, 0], "minecraft:stone"),
            ([1, 65, 0], "minecraft:stone"),
            ([1, 66, 0], "minecraft:stone"), // ledge top at y=67.0
        ]);
        assert!(
            !over.neighbors([0, 66, 0]).contains(&[1, 67, 0]),
            "1.5 blocks up off a drift is not jumpable: {:?}",
            over.neighbors([0, 66, 0])
        );
    }

    // --- exact camera clip test (task #78) -----------------------------------

    #[test]
    fn first_clip_catches_a_corner_graze_the_old_sampler_missed() {
        // A single solid cell the dolly cuts diagonally through the corner of. The
        // old 0.25-sampler stepped over it (the segment is inside the cell for far
        // less than one sample); the DDA walk visits every cell the segment
        // touches, so the clip is caught.
        let world = World::from_solid_cells([[1, 0, 1]].into_iter().collect());
        let pts = [[0.9, 0.5, 0.9], [1.2, 0.5, 1.2]];
        assert_eq!(
            first_clip(&world, &pts).map(|(_, c)| c),
            Some([1, 0, 1]),
            "the grazed cell must be reported"
        );
        // A parallel path that never enters the cell stays clean.
        let clear = [[0.9, 0.5, 0.9], [0.9, 0.5, 2.5]];
        assert_eq!(first_clip(&world, &clear), None);
    }

    #[test]
    fn walk_cells_visits_every_cell_in_order() {
        // A long axis-aligned run visits each cell once, in order; the first
        // matching cell wins.
        let seen = std::cell::RefCell::new(Vec::new());
        let out = walk_cells([0.5, 0.5, 0.5], [4.5, 0.5, 0.5], |c| {
            seen.borrow_mut().push(c);
            c[0] == 3
        });
        assert_eq!(out, Some([3, 0, 0]));
        assert_eq!(
            *seen.borrow(),
            vec![[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]
        );
    }

    // --- stealth zones: reachable-ANY, not reachable-centre (task #78) --------

    #[test]
    fn stealth_zone_passes_when_any_zone_cell_is_reachable() {
        // The zone box straddles a wall: its CENTRE snaps to a standable cell in a
        // walled-off pocket, while the rest of the box is plainly reachable. The
        // obligation is "the player can reach cover somewhere in this zone", so
        // this must pass — testing only the snapped centre raised a spurious
        // DW0327.
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..7 {
            for z in 0..3 {
                cells.push(([x, 64, z], "minecraft:stone")); // floor
            }
        }
        // A full-height wall at x=4 sealing off the x=5 pocket from the walkway,
        // with a 2-high stack so nothing can be jumped over.
        for z in 0..3 {
            for y in 65..=66 {
                cells.push(([4, y, z], "minecraft:stone"));
                cells.push(([5, y, z], "minecraft:stone"));
            }
        }
        // Reopen the pocket floor at [5,65,1] by removing the wall cell there.
        cells.retain(|(c, _)| *c != [5, 65, 1] && *c != [5, 66, 1]);
        let world = blocks_world(&cells);
        // Zone box x 3..=5 at y=65: [5,65,1] (the pocket) and [3,65,z] (the open
        // walkway) are both standable, but only the walkway is reachable.
        assert!(world.is_standable([5, 65, 1]), "the pocket is standable");
        assert!(world.is_standable([3, 65, 1]), "the walkway is standable");
        let beats = vec![(
            vec![("zone/shadow".to_string(), [4, 65, 1], [1, 0, 1])],
            0usize,
        )];
        assert!(
            verify_stealth(&world, &beats, &[at_step([0, 65, 1], 0)]).is_ok(),
            "a zone with ANY reachable standable cell must pass"
        );
    }

    // --- causally-sealed waypoint export + trap forcing (task #78) ------------

    /// A 5-long, 3-wide room at y=65 whose only two lanes (z=0 and z=2) run from
    /// x=0 to x=4; the middle lane z=1 is walled. Sealing one lane's chokepoint
    /// forces the route onto the other.
    fn two_lane_room(y: i32) -> World {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..5 {
            for z in 0..3 {
                cells.push(([x, y - 1, z], "minecraft:stone"));
            }
            // The middle lane is a solid wall at stand + head height.
            cells.push(([x, y, 1], "minecraft:stone"));
            cells.push(([x, y + 1, 1], "minecraft:stone"));
        }
        blocks_world(&cells)
    }

    #[test]
    fn exported_waypoints_never_route_through_a_sealed_gate() {
        // The z=0 lane is the short way; a `close-gate` seals its chokepoint
        // [2,65,0] before the leg is walked. The completability proof already
        // routed the detour — the EXPORT must agree, or the harness bot is handed
        // a route through a boulder that has already dropped.
        let mut world = two_lane_room(65);
        // Join the two lanes at both ends so a detour exists.
        world.solid.remove(&[0, 65, 1]);
        world.solid.remove(&[0, 66, 1]);
        world.solid.remove(&[4, 65, 1]);
        world.solid.remove(&[4, 66, 1]);
        let a = at_step([0, 65, 0], 1);
        let b = at_step([4, 65, 0], 2);
        let close = GateEvent {
            region: ([2, 65, 0], [2, 66, 0]),
            closes: true,
            fire_step: 0,
        };
        let open_legs = route_walked_legs(&world, &[a, b], &[], &linear);
        assert!(
            open_legs[0].0.cells.contains(&[2, 65, 0]),
            "with the gate open the export takes the short lane"
        );
        let sealed_legs = route_walked_legs(&world, &[a, b], std::slice::from_ref(&close), &linear);
        assert_eq!(sealed_legs.len(), 1, "the leg is still routable via z=2");
        assert!(
            !sealed_legs[0].0.cells.contains(&[2, 65, 0]),
            "an exported waypoint must never cross a sealed gate cell: {:?}",
            sealed_legs[0].0.cells
        );
        assert!(
            sealed_legs[0].0.cells.contains(&[2, 65, 2]),
            "the export takes the detour lane the proof routed"
        );
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
        let close = GateEvent {
            region: ([2, 65, 0], [2, 66, 0]),
            closes: true,
            fire_step: 0,
        };
        let tc = [2, 65, 2];
        let traps = [lethal_trap(tc, TrapReset::Rearm, None)];
        let spawn = [[0, 65, 0]];

        let open_legs = route_walked_legs(&world, &[a, b], &[], &linear);
        let open_required: BTreeSet<[i32; 3]> = open_legs
            .iter()
            .flat_map(|(l, _)| l.cells.clone())
            .collect();
        assert!(
            verify_traps(&world, &traps, &open_required, &spawn, &open_legs).is_ok(),
            "with the gate open the plate is genuinely avoidable"
        );

        let sealed_legs = route_walked_legs(&world, &[a, b], std::slice::from_ref(&close), &linear);
        let sealed_required: BTreeSet<[i32; 3]> = sealed_legs
            .iter()
            .flat_map(|(l, _)| l.cells.clone())
            .collect();
        let err = verify_traps(&world, &traps, &sealed_required, &spawn, &sealed_legs)
            .expect_err("the sealed detour forces the party across the plate");
        assert_eq!(err.code, DW_TRAP_LETHAL_UNAVOIDABLE); // DW0342
    }

    // --- partial heights x ocean horizon compose (#78 x #159) ----------------

    /// The two world models are independent axes and must both stay live in one
    /// `World`: `partial` (task #78) decides *which cells are reachable*, and
    /// `ambient` (spec-0013, #159) decides *what the unmodelled columns contain*.
    ///
    /// Geometry: an ocean island whose top plate is a course of BOTTOM SLABS, so
    /// the walk plane sits at `sea_level + 0.5` rather than `sea_level + 1`. The
    /// slab cells are still `solid` (feet cell `sea_level + 1`), so the climb-out
    /// band sees the canonical beach and the coast is not a stranding — while the
    /// step rule is simultaneously reasoning in sixteenths over the same cells.
    #[test]
    fn partial_floor_heights_and_the_ocean_horizon_compose() {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..8 {
            for z in 0..8 {
                for y in 60..=61 {
                    cells.push(([x, y, z], "minecraft:stone"));
                }
                // The shore course is a bottom slab: top face at 62.5.
                cells.push(([x, 62, z], "minecraft:oak_slab[type=bottom]"));
            }
        }
        let occ = crate::assembled::occupancy_of(
            cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect(),
            &BTreeSet::new(),
        );
        // The partial map is populated…
        assert_eq!(
            occ.partial.get(&[3, 62, 3]),
            Some(&8),
            "the slab course must be modelled as a half-height floor"
        );
        let world = World::from_occupancy(occ).with_ambient(Ambient::Ocean(Sea {
            level: 62,
            floor_top: 54,
            covered: vec![([0, 60, 0], [7, 62, 7])],
        }));

        // …and BOTH axes are live on the same World: the step rule sees the true
        // feet height (62 + 0.5 → 62·16 + 8 sixteenths) …
        let fp = Footprint::player();
        assert_eq!(
            world.feet_16_fp([3, 63, 3], &fp),
            62 * 16 + 8,
            "feet rest on the slab face, not the cell floor"
        );
        // … while the ocean premise still governs the boundary verdict.
        verify_boundary_safety(&world, &[[3, 63, 3]])
            .expect("a slab-course beach is a climb-out, not a stranding");

        // The control: the identical geometry under the void premise is still the
        // void-drop error #159 kept byte-identical — partial heights did not
        // disturb that verdict either.
        let voidish = World::from_occupancy(crate::assembled::occupancy_of(
            cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect(),
            &BTreeSet::new(),
        ));
        let err = verify_boundary_safety(&voidish, &[[3, 63, 3]])
            .expect_err("under `void` the same slab coast is a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
    }
}
