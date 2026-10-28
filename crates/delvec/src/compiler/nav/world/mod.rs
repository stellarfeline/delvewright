//! The voxel world the navigation proofs are taken over: the assembled
//! occupancy model ([`World`]), the premises it is built under, the derived
//! worlds a runtime write leaves, and the liveness of a staged volume.
//!
//! How a body stands in it and moves through it is [`body`].

pub(in crate::compiler::nav) mod body;
pub use body::*;
pub(in crate::compiler::nav) mod climb;
pub use climb::*;
pub(in crate::compiler::nav) mod confine;
pub use confine::*;
pub(in crate::compiler::nav) mod reach;
pub use reach::*;

use crate::compiler::cellset::{CellMap, CellSet};
use crate::compiler::nav::route::region::RegionState;
use crate::compiler::plan::{Plan, RegionEvents, StagedGate};
use std::collections::{BTreeMap, BTreeSet};

/// The **world-generator ambient** the placed geometry sits in — what a column
/// the compiler modelled nothing into actually contains in the delivered world
/// (spec-0013 `horizon`).
///
/// The assembled model ([`crate::compiler::assembled`]) knows only cells a prefab, a socket
/// seal or an edit wrote. Everything else is "absent", and what *absent* means is
/// a property of the level generator, not of the content: on a `void` base
/// an absent column is bottomless; on an `ocean` base it is the pinned
/// bedrock/stone/water superflat, so there is no void anywhere in the world and
/// stepping off the land is swimming. Boundary safety ([`verify_boundary_safety`])
/// is the one proof whose premise is exactly this, so the ambient rides on the
/// [`World`] rather than being re-derived (or, as before, silently assumed to be
/// `Void`) at the call site.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Ambient {
    /// A `void` horizon base — an empty superflat layer list. Every
    /// column the content did not build is bottomless.
    #[default]
    Void,
    /// An `ocean` horizon base — the pinned bedrock/stone/water superflat ([`Sea`]).
    Ocean(Sea),
}

/// The ocean horizon's ambient sea: a global water plane topping out at
/// [`Sea::level`], solid ground from [`Sea::floor_top`] down, and air above —
/// present in **every** column except those a placed piece overwrote (the
/// world's [`World::built`] volume).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sea {
    /// Y of the topmost ambient water block (`crate::compiler::plan::SEA_LEVEL`, 62).
    pub level: i32,
    /// Y of the topmost ambient solid block — the sea floor
    /// (`crate::compiler::plan::SEA_FLOOR_TOP_Y`, 54). Ambient water occupies
    /// `floor_top+1 ..= level`.
    pub floor_top: i32,
}

impl Ambient {
    /// The ambient a campaign's `horizon` declares (spec-0013). Purely the
    /// generator's own facts: **where the content ends is not one of them** —
    /// that is [`built_volume`], a property of the assembled world under every
    /// horizon (see [`World::built`]).
    pub fn of_plan(plan: &Plan) -> Ambient {
        Ambient::of_base(delvewright_dsl::horizon_base(
            &plan.campaign.world.content.horizon,
        ))
    }

    /// The ambient a horizon base declares — the same answer [`Ambient::of_plan`]
    /// gives, asked of the base alone, so a check that runs before anything is
    /// placed (`compiler::seating`) reads the one generator fact the build reads.
    pub fn of_base(base: delvewright_dsl::HorizonBase) -> Ambient {
        match base {
            delvewright_dsl::HorizonBase::Ocean => Ambient::Ocean(Sea {
                level: crate::compiler::plan::SEA_LEVEL,
                floor_top: crate::compiler::plan::SEA_FLOOR_TOP_Y,
            }),
            // A `valley` ambient is void: its ground is not a generator fact
            // but real placed blocks, so the surround enters this model through
            // [`built_volume`] like any other piece rather than as an analytic
            // per-column answer here. That is what lets every existing proof —
            // gravity, occupancy, relight, boundary safety — see the landform
            // without one of them being taught a new horizon.
            delvewright_dsl::HorizonBase::Void | delvewright_dsl::HorizonBase::Valley => {
                Ambient::Void
            }
        }
    }

    /// The horizon's own name, for a message or a ledger.
    pub fn name(&self) -> &'static str {
        match self {
            Ambient::Void => "void",
            Ambient::Ocean(_) => "ocean",
        }
    }
}

/// **Where the content decided what is there**: every placed piece's prefab id
/// paired with its inclusive world AABB, in plan order (area order, entry piece
/// first — deterministic, ADR-0006).
///
/// `/place template` writes the whole box, air included, so inside a box the
/// piece's bytes decide and the world generator does not apply; outside every
/// box the generator's [`Ambient`] does. Two proofs ask that one question — the
/// ocean's `ambient_water`, and [`measure_fluid_escape`] — and it is a fact
/// about the ASSEMBLED WORLD, not about the sea, which is why it lives on
/// [`World`] rather than on [`Sea`]. It was a field of `Sea` first; a world
/// on a `void` base therefore had no idea where its own content ended, and
/// water that ran off the last piece met nothing that could judge it.
pub fn built_volume(plan: &Plan) -> Vec<BuiltPiece> {
    plan.areas
        .iter()
        .flat_map(|a| a.pieces.iter().map(|p| (p.prefab_id.clone(), p.bbox())))
        .collect()
}

/// A collision/standability model of the assembled world (spec-0008 addendum),
/// derived from the shared gravity-settled assembled-world model
/// ([`crate::compiler::assembled`]): every placed prefab block, plus the solver's socket
/// seals, with gate thresholds cleared and unsupported falling blocks settled
/// Cells absent from both sets are passable (interior air, opened
/// sockets, gate thresholds, and any cell a gravity block fell out of).
///
/// Water is modelled separately from solids: `flooded` holds every cell
/// a conservative superset of vanilla water flow reaches (see
/// [`crate::compiler::assembled::assembled_occupancy`]). A flooded cell is **impassable** (a
/// walker cannot stand or pass through it) yet is **not solid floor** (you cannot
/// stand *on* a water surface) — the two sets are disjoint and both gate
/// standability, so nav / wave seating / relight / waypoint export never treat a
/// flooded cell as walkable ground.
///
/// Collision classes ([`crate::compiler::assembled::Occupancy`]): `solid` holds
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
/// Partial floor heights: `partial` records, for a `solid` cell whose
/// walkable top face sits **below** the cell top (a bottom slab at 8/16, a snow
/// drift, a `dirt_path` at 15/16), that true height. It is what makes
/// [`World::neighbors_fp`] a physical step rule rather than a cell-adjacency rule
/// — see [`delvewright_dsl::metrics::MAX_AUTO_STEP_16`] / [`delvewright_dsl::metrics::MAX_JUMP_RISE_16`].
/// One declared lethal volume as the navigation model carries it: `(id, box)`,
/// the box being inclusive world-space corners.
pub(in crate::compiler::nav) type LethalRegion = (String, ([i32; 3], [i32; 3]));

/// One lethal volume **live from a story stage** (spec-0088), as the region
/// model holds it: the id, the box, and the resolved gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StagedVolume {
    /// The authored id.
    pub id: String,
    /// Inclusive world-space corners.
    pub region: ([i32; 3], [i32; 3]),
    /// The gate the volume is live while.
    pub gate: StagedGate,
}

/// A staged volume's two readings at one quest configuration (spec-0088 §4.1).
///
/// `may` is the route proofs' reading — live unless the campaign can prove it is
/// not, so every proof that walks past it survives it; `is` is the ladder's —
/// live only where the forced route guarantees it. `is` implies `may`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Liveness {
    /// The volume may be live here.
    pub may: bool,
    /// The volume is live here on the forced route.
    pub is: bool,
}

impl Liveness {
    /// The state in words, for a diagnostic.
    pub fn words(&self) -> &'static str {
        match (self.may, self.is) {
            (_, true) => "is live",
            (true, false) => "may be live",
            (false, false) => "is dead",
        }
    }
}

/// **The two readings of one gate at one arrival** (spec-0088 §4.1), over a
/// path's flag writes and datum replay ([`RegionEvents`]) and the region model's
/// own ancestry predicate.
///
/// - `may`: every required flag has a setter this configuration credits — a
///   forced firing the arrival's ancestry contains, or ANY unforced one (credited
///   at step 0, as an unforced fill is); no forbidden flag has a forced setter the
///   ancestry contains (an unforced setter would make the volume passable, and an
///   unguaranteed firing may never open); every numeric term may hold — the
///   replay's value satisfies it, the replay cannot date it, or some unforced root
///   writes the datum at all.
/// - `is`: every required flag set by a forced firing the ancestry contains; no
///   forbidden flag with any setter credited here; every numeric term decided
///   true by the replay.
///
/// Flags are never cleared, so both readings are monotone in the flags along a
/// path.
pub(crate) fn liveness_of(
    gate: &StagedGate,
    events: &RegionEvents,
    arrival: usize,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Liveness {
    let forced_here = |f: &str| {
        events
            .flags
            .iter()
            .any(|e| e.flag == f && e.forced && ancestor(e.fire_step, arrival))
    };
    let any_here = |f: &str| {
        events
            .flags
            .iter()
            .any(|e| e.flag == f && (!e.forced || ancestor(e.fire_step, arrival)))
    };
    let values = events.data.at(arrival);
    // `v` satisfies the term iff pinning the term's set to `v` leaves it
    // non-empty — the gate's own arithmetic ([`DatumSet`]), never a second
    // reading of what a comparison means.
    //
    // [`DatumSet`]: delvewright_dsl::gate::DatumSet
    let satisfies = |c: &delvewright_dsl::StateCompare, v: i64| {
        i32::try_from(v).is_ok_and(|v| {
            let mut set = delvewright_dsl::gate::DatumSet::all();
            set.require(c.op, c.value);
            set.require(delvewright_dsl::CompareOp::Equals, v);
            set.pick().is_some()
        })
    };
    let may = gate.requires_flags.iter().all(|f| any_here(f))
        && !gate.forbids_flags.iter().any(|f| forced_here(f))
        && gate.requires_state.iter().all(|c| {
            events.data.unforced_writers.contains(c.state.as_str())
                || match values.get(c.state.as_str()) {
                    Some(Some(v)) => satisfies(c, *v),
                    _ => true,
                }
        });
    let is = gate.requires_flags.iter().all(|f| forced_here(f))
        && !gate.forbids_flags.iter().any(|f| any_here(f))
        && gate
            .requires_state
            .iter()
            .all(|c| matches!(values.get(c.state.as_str()), Some(Some(v)) if satisfies(c, *v)));
    Liveness { may, is: is && may }
}

/// A loop gate's terms in the words a route failure names (spec-0086 §5.1):
/// `forbids_flags: flag/x`, `requires_state: state/n at-most 1`,
/// `requires_flags: flag/y`, each in backticks, comma-separated.
pub(in crate::compiler::nav) fn loop_gate_words(g: &StagedGate) -> String {
    let mut out: Vec<String> = Vec::new();
    out.extend(
        g.forbids_flags
            .iter()
            .map(|f| format!("`forbids_flags: {f}`")),
    );
    out.extend(g.requires_state.iter().map(|c| {
        format!(
            "`requires_state: {} {} {}`",
            c.state.as_str(),
            c.op.token(),
            c.value
        )
    }));
    out.extend(
        g.requires_flags
            .iter()
            .map(|f| format!("`requires_flags: {f}`")),
    );
    if out.is_empty() {
        "{}".to_string()
    } else {
        out.join(", ")
    }
}

/// One placed furniture region as the navigation model carries it — the plan's
/// own type, `(anchor name, inclusive world box)` (spec-0065). Same shape as
/// [`LethalRegion`] for the same reason: a proof that refuses over it has to be
/// able to name the anchor.
use crate::compiler::plan::FurnitureRegion;

/// One placed piece as the built volume carries it: `(prefab id, box)`, the box
/// being inclusive world-space corners. Same shape as [`LethalRegion`] and for
/// the same reason — a proof that refuses over a region has to be able to NAME
/// the region, or the author is sent to look at geometry that was never wrong.
pub type BuiltPiece = (String, ([i32; 3], [i32; 3]));

pub struct World {
    /// **The box of the one piece this world is**, `[lo, hi)`, when it was
    /// built by [`World::of_piece`]; `None` for every other world. A body
    /// that leaves the box has left the thing being measured, so no move of
    /// [`World::body_flood`] — the one confined walk every per-piece reader
    /// takes — leads out of it, while the blocks, the water and its spill
    /// beyond the box stay what they are.
    pub(in crate::compiler::nav) piece: Option<([i32; 3], [i32; 3])>,
    pub(in crate::compiler::nav) solid: CellSet,
    /// Cells whose block hides what stands behind it from the eye — the one
    /// sight predicate, [`World::hides`]. Measured from the blocks
    /// ([`crate::compiler::assembled::Occupancy::hiding`]), and moved by a
    /// region write exactly as the block it writes does.
    pub(in crate::compiler::nav) hiding: CellSet,
    pub(in crate::compiler::nav) tall: CellSet,
    pub(in crate::compiler::nav) use_gates: CellSet,
    pub(in crate::compiler::nav) flooded: CellSet,
    /// For each `solid` cell whose walkable top face sits **below** the cell
    /// top, that height in sixteenths. Absent = a full cube. Feeds
    /// the physical step rule in [`World::neighbors_fp`].
    pub(in crate::compiler::nav) partial: CellMap<u8>,
    /// Cells whose block **can** hold water
    /// ([`crate::compiler::assembled::is_waterloggable`]). Read by exactly one
    /// proof, [`measure_sea_seepage`]: under an ocean ambient every one of these
    /// standing in the sea's own band comes out of `/place template` waterlogged,
    /// which makes it a water source the block map does not contain.
    pub(in crate::compiler::nav) waterloggable: CellSet,
    /// The subset of `flooded` that is **lava** rather than water
    /// ([`crate::compiler::assembled::Occupancy::lava`]). Read by the questions
    /// of where a body can be — the reach graph and a mob's moves: a body that
    /// enters water floats at its surface, and one that enters lava does not
    /// come out of it at all.
    lava: CellSet,
    /// The blocks that change how a body moves ([`crate::compiler::assembled::Occupancy::material`]).
    /// Read by the reach graph's simulator alone ([`reach`]).
    pub(in crate::compiler::nav) material: CellMap<delvewright_dsl::metrics::body::Material>,
    /// This world's content digest, computed once on first ask
    /// ([`World::fingerprint`]): the key the reach graph is memoised on.
    pub(in crate::compiler::nav) fingerprint: std::sync::OnceLock<[u8; 32]>,
    /// The climbables the assembled world **keeps**, with what keeps each
    /// (spec-0099, [`crate::compiler::assembled::climb_holds`]). Shared, never
    /// narrowed: a derived world narrows `climb`, not this.
    pub(in crate::compiler::nav) climb_holds: ClimbHolds,
    /// The subset of `climb_holds` this view still keeps: a runtime write that
    /// takes away what a climbable hangs on takes the climbable with it
    /// ([`World::drop_unheld_climbs`]). A body whose feet are in one of these
    /// cells climbs ([`World::climb_cell_fp`]).
    pub(in crate::compiler::nav) climb: CellSet,
    /// The climbables the block map holds and the world does not keep — read as
    /// air. Carried only so a route that needed one can name it (`DW0991`).
    pub(in crate::compiler::nav) unheld_climb: UnheldClimbs,
    /// Cells inside a declared **lethal volume** (DSL v0.10, spec-0031).
    ///
    /// A volume that kills whatever enters it is, for a route, a volume no route
    /// may enter — so these cells are **impassable**, exactly like a flooded one,
    /// and for the same reason: a walker that goes there does not come out the
    /// other side. They are deliberately *not* added to `solid`: a lethal cell is
    /// not floor, so nothing may stand on top of one either. Empty for every
    /// campaign that declares no volume, which is what keeps routing, standability
    /// and every downstream proof byte-identical.
    pub(in crate::compiler::nav) lethal: CellSet,
    /// The declared volumes behind `lethal`, as `(id, box)`, in declaration
    /// order. Carried so a route failure can NAME the volume that caused it
    /// rather than report an unroutable leg over geometry that looks open — the
    /// difference between `DW0510` and a `DW0311` that sends the author to fix a
    /// prefab that was never wrong.
    lethal_regions: Vec<LethalRegion>,
    /// The **completion volumes this leg keeps out of** (`DW0311`): each `reach`
    /// objective armed while the party walks the leg and not yet due, as
    /// `(objective, box)`. A body may not stand where the objective's selector
    /// would reach it — the hitbox rule a lethal volume is read by — because a
    /// body there completes the objective out of the order the path walks it in.
    /// Not solid and not occupied: a view and a click pass through it. Empty for
    /// every world but a leg's ([`World::with_region_state`]).
    pub(in crate::compiler::nav) keep_out: Vec<LethalRegion>,
    /// The declared volumes **live from a story stage** (spec-0088), in
    /// declaration order — absent from `lethal` and `lethal_regions`, because
    /// whether one is there is a fact about a point in the quest DAG and not
    /// about the world. [`World::region_state_at`] turns each into cells for the
    /// configurations it may be live in ([`World::staged_liveness`]), so a staged
    /// volume reaches a proof's world through that one door and no other. Empty
    /// for every campaign that stages none, which keeps every world and every
    /// region state byte-identical.
    pub(in crate::compiler::nav) staged_lethal: Vec<StagedVolume>,
    /// Every loop's slab with its gate (spec-0086 §5.1): the slab is held —
    /// impassable and never floor — in every configuration its gate may be open
    /// in, read through [`liveness_of`] as a staged volume's is (spec-0088
    /// §4.1). Empty for every campaign that declares no loop.
    pub(in crate::compiler::nav) loop_slabs: Vec<StagedVolume>,
    /// Cells inside a declared **furniture** region (spec-0065): the blocks of a
    /// laid table, an altar, a counter, as the piece that built them declared.
    ///
    /// Not impassable and not removed from `solid` — a table is geometry, and a
    /// body beside it stands on the floor. What these cells withhold is the cell
    /// ABOVE each solid one: [`World::standable_fp`] refuses a body whose support
    /// is a solid furniture cell, so no route, snap, flood, seat or export stands
    /// a body on a table. Empty for every campaign whose pieces declare none,
    /// which keeps every proof byte-identical.
    furniture: CellSet,
    /// The regions behind `furniture`, as `(anchor, box)`, in the plan's order
    /// ([`Plan::furniture`]). Carried so a route that exists only over a table
    /// names the table (`DW0510`).
    furniture_regions: Vec<FurnitureRegion>,
    /// Cells another proof has FORCED solid as its own premise — a `collapse`'s
    /// settled debris, an ambush's occupied cells, a timed gate's shut span, an
    /// aggro sphere ([`World::with_sealed`]).
    ///
    /// A runtime `clear-region` may not remove one. Clearing a region says "the
    /// blocks the campaign put here are gone"; it does not say "the hazard another
    /// proof is reasoning about never happened". Without this, a `clear-region`
    /// laid over a collapse's rubble would delete the rubble from that proof's
    /// world and `DW0445` would go quietly green — a new verb weakening an existing
    /// check, which is the one thing a new verb may never do.
    pinned: CellSet,
    /// **What the world authors shut at world-load**: one entry per gate anchor
    /// whose region the placed prefabs fill with a block
    /// ([`crate::compiler::assembled::GateSeal`]), as a world-load `Fill` at step 0.
    ///
    /// This is a property of the WORLD, not of any verb: the bars in
    /// `hello-room`'s doorway are there because the `.nbt` puts them there, and
    /// they stay there until something clears them. It lives beside the region
    /// model rather than on the [`Plan`] because the plan is built before the
    /// `.nbt` bytes are read, and because the one question it answers — *what is
    /// solid at this point of the quest DAG* ([`World::region_state_at`]) — is
    /// already this type's.
    ///
    /// Every gate is listed, sealed or not, because the ledger this backs
    /// (`validation/gate-seal.json`) has to state what was EXAMINED and not only
    /// what was found; [`World::modelled_seals`] is the subset the model treats as
    /// shut. Empty for a synthetic test world and for every campaign with no gate
    /// anchor at all — which is what keeps those routings byte-identical.
    world_load_seals: Vec<crate::compiler::assembled::GateSeal>,
    /// Gate regions a `timed-gate`'s clock owns (spec-0016 §4) — measured like any
    /// other gate and never modelled as shut, because the clock clears them twice
    /// a cycle from world-load. See [`World::with_world_load_seals`].
    clocked_gates: BTreeSet<([i32; 3], [i32; 3])>,
    /// Cells a **runtime fluid fill** has flooded on this view
    /// ([`crate::compiler::plan::RegionWrite::Flood`], [`World::with_flooded`]) — a subset of
    /// `flooded`, kept apart from the prefab-authored water only so the
    /// counterfactual [`World::without_runtime_flood`] can put exactly these cells
    /// back and no others. Empty on the base world and on every campaign that never
    /// fills a region with a fluid, which is what keeps every other campaign's
    /// proofs byte-identical.
    flood_written: CellSet,
    /// The boxes behind `flood_written`, in region order — who to blame for a route
    /// that only exists when a fluid is mistaken for floor. Same job as
    /// `lethal_regions`, for the same reason: without it the author gets "no
    /// collision-free path" over geometry that looks open.
    flood_regions: Vec<([i32; 3], [i32; 3])>,
    /// What the *unmodelled* columns contain (spec-0013 `horizon`). Defaults to
    /// [`Ambient::Void`] — the pre-0.6 world and every synthetic test world —
    /// and is set from the plan by [`World::from_plan`] /
    /// [`World::with_ambient`]. Read only by [`verify_boundary_safety`] and
    /// [`measure_fluid_escape`]; it deliberately does **not** feed the
    /// walkability sets, so routing, standability and every other proof stay
    /// byte-identical.
    pub(in crate::compiler::nav) ambient: Ambient,
    /// The base the campaign declared, beside the ambient it resolves to — see
    /// [`Premises::base`].
    pub(in crate::compiler::nav) base: &'static str,
    /// **Where the content ends** ([`built_volume`]): every placed piece's
    /// prefab id and inclusive world AABB. Empty on a synthetic test world,
    /// which is the fail-CLOSED direction for [`measure_fluid_escape`] — a
    /// world with no known built volume can prove nothing contained, rather than
    /// proving everything contained. Set beside `ambient` by
    /// [`World::with_ambient`], whose signature takes both so no call site can
    /// set the premise and forget the extent.
    pub(in crate::compiler::nav) built: Vec<BuiltPiece>,
    /// **Where the party is required to stand**, objective id paired with the
    /// cell the critical path names for it, in path order. Empty on a synthetic
    /// world. Read only by [`measure_sea_seepage`], so a wet walk region can be
    /// reported as the objectives it drowns rather than as a list of
    /// coordinates.
    pub(in crate::compiler::nav) objective_cells: Vec<(String, [i32; 3])>,
    /// Whether no fall kills a body in this world: a kit the campaign hands
    /// reduces fall damage (spec-0113 §4.2, [`Premises::unbounded_fall`]).
    pub(in crate::compiler::nav) unbounded_fall: bool,
    /// The declared jumps (spec-0113 §6) as `(from, to)`: the one move the walk
    /// relation gains ([`World::neighbors_fp`]). A build proves every one before
    /// any route is taken (`DW1041`), so a jump here is one a body on cue makes.
    pub(in crate::compiler::nav) declared_jumps: Vec<([i32; 3], [i32; 3])>,
    /// **Where the party stands to take from a holder entity** (spec-0111
    /// §5.2): each `collect`-claimed holder's cell, paired with the stance cell
    /// the stance pass chose for it ([`crate::compiler::holders::collect_stances`]).
    /// A leg whose endpoint is that holder's cell ends at the stance
    /// ([`World::snap_endpoint`]), never in the holder's body, which the block
    /// model reads as air. A fact of the template bytes, set after the world is
    /// built ([`World::with_holder_stances`]), for the reason `world_load_seals`
    /// lives here and not on the plan. Empty on every campaign with no holder
    /// collect, which keeps every route byte-identical.
    pub(in crate::compiler::nav) holder_stances: BTreeMap<[i32; 3], [i32; 3]>,
    /// **Which floors hold a body** ([`Floors`]): the solid cells whose block
    /// does not cover its cell, and the subset of them a body over falls
    /// through.
    pub(in crate::compiler::nav) floors: Floors,
    /// **Where a body is put down by declaration** ([`Seating`]): the cells
    /// whose block hurts a body, and each checkpoint's respawn seat once
    /// [`World::with_rest_seats`] has derived them over this world. Carried by
    /// every view, as `holder_stances` is.
    pub(in crate::compiler::nav) seating: crate::compiler::nav::Seating,
}

/// **Whether a solid cell holds a body standing over it**, read from the
/// collision footprints of its block and of the blocks beside it
/// ([`delvewright_dsl::blockshape::body_falls_through`]) rather than from
/// whether the cell holds a block.
///
/// `footprints` holds every solid cell whose block does not cover its cell —
/// iron bars, a pane, a chain, a lantern — with its server footprint; a solid
/// cell absent from it covers its cell. `sieve` is the subset a body over
/// falls through: the cell under it does not stop a fall (or is itself a
/// sieve), and a body-wide square of the course, centred over the cell, meets
/// no box. A sieve cell still stops a body walking into it, as a post does;
/// it is never a floor, and a falling body passes it.
///
/// Every view derives `sieve` from its own cells ([`World::resieve`]), so a
/// region write that lays or clears a block beside a hatch moves the answer.
/// Empty for a world with no such block, which keeps every proof over it
/// byte-identical.
#[derive(Clone, Debug, Default)]
pub struct Floors {
    pub(in crate::compiler::nav) footprints:
        BTreeMap<[i32; 3], delvewright_dsl::blockshape::Footprint16>,
    pub(in crate::compiler::nav) sieve: BTreeSet<[i32; 3]>,
    /// Every door and trapdoor cell with its leaf per state
    /// ([`crate::compiler::assembled::Occupancy::leaves`]): which faces of its
    /// cell a body crosses, and at what height ([`World::leaf_stopping`]).
    pub(in crate::compiler::nav) leaves: BTreeMap<[i32; 3], delvewright_dsl::blockshape::Leaf>,
    /// Whether a body in this view may set a door or trapdoor a hand opens
    /// before it moves — false for walkers that cannot use gates
    /// ([`World::without_gate_use`]), which meet each leaf as it stands.
    pub(in crate::compiler::nav) leaf_use: bool,
}

/// The block-derived cell sets of a [`World`], as [`World::from_occupancy`]
/// takes them from an occupancy.
struct Cells {
    solid: CellSet,
    hiding: CellSet,
    tall: CellSet,
    use_gates: CellSet,
    flooded: CellSet,
    partial: CellMap<u8>,
    waterloggable: CellSet,
    lava: CellSet,
    material: CellMap<delvewright_dsl::metrics::body::Material>,
    climb_holds: ClimbHolds,
    unheld_climb: UnheldClimbs,
    footprints: BTreeMap<[i32; 3], delvewright_dsl::blockshape::Footprint16>,
    leaves: BTreeMap<[i32; 3], delvewright_dsl::blockshape::Leaf>,
    hurting: CellSet,
}

/// **Everything a [`World`] carries that is not a block** — the premises the
/// campaign states about the world its geometry sits in: what the generator put
/// in the columns the content never built, where the content ends, which volumes
/// kill, which gates the prefabs shut at world-load, which of those a clock owns,
/// and where the party can be carried instead of walking.
///
/// It exists because those premises were previously applied by a *chain of
/// optional methods* on `World`, and an arm that forgot a link got a world that
/// was wrong in exactly the direction nothing reports. That is not hypothetical:
/// `emit::build`'s `edit_replay` arm — the arm every campaign declaring
/// `world-edits.json` takes — applied the ambient and the gate seals and not the
/// **lethal volumes**, so the completability proof of every edit-carrying
/// campaign routed straight through its own kill boxes and reported
/// `"cells": 0` in `validation/lethal-gate.json` while doing it. The gallery's
/// exported critical path walked six waypoints through two declared lethal
/// volumes and the bot died at the seventh step of the run.
///
/// The repair is this type rather than a fourth remembered method call: the
/// premises travel as **one value**, [`Premises::of_plan`] is the only way to
/// derive one from a campaign, and it fills every field. A caller cannot state a
/// subset, so it cannot forget one — and a premise added here reaches every arm
/// at once instead of reaching the arms whoever added it happened to grep for.
///
/// A world with no campaign behind it says so by name: [`Premises::geometry_only`].
#[derive(Clone, Debug)]
pub struct Premises {
    pub(in crate::compiler::nav) ambient: Ambient,
    /// **The base the campaign declared**, beside the ambient it resolves to.
    ///
    /// The two are not the same word and a reader cannot tell an unbound check
    /// from a mislabelled one: `valley` resolves to [`Ambient::Void`], because
    /// its ground is placed blocks rather than a generator fact, so every
    /// binding line keyed to the ambient alone said ``horizon `void``` about a
    /// world whose author wrote `valley`. Every line that prints a horizon
    /// prints this, and says "ambient" where it reports the other
    /// (spec-0060 §10.7).
    pub(in crate::compiler::nav) base: &'static str,
    pub(in crate::compiler::nav) built: Vec<BuiltPiece>,
    /// The volumes live from world-load to the end — every volume that declares
    /// no `when`. A staged volume is not a premise of the world: it is
    /// `staged_lethal`, held per quest configuration.
    pub(in crate::compiler::nav) lethal_regions: Vec<LethalRegion>,
    /// The volumes live from a story stage (spec-0088).
    pub(in crate::compiler::nav) staged_lethal: Vec<StagedVolume>,
    /// Every loop's slab and gate (spec-0086).
    pub(in crate::compiler::nav) loop_slabs: Vec<StagedVolume>,
    pub(in crate::compiler::nav) furniture_regions: Vec<FurnitureRegion>,
    pub(in crate::compiler::nav) world_load_seals: Vec<crate::compiler::assembled::GateSeal>,
    pub(in crate::compiler::nav) clocked_gates: BTreeSet<([i32; 3], [i32; 3])>,
    pub(in crate::compiler::nav) objective_cells: Vec<(String, [i32; 3])>,
    /// **Whether every fall is survivable** (spec-0113 §4.2): a kit the
    /// campaign hands a player reduces fall damage — a damage-reducing armour
    /// enchantment, a max-health change or a fall-damage multiplier — so the
    /// reach graph credits a landing however far the body fell.
    pub(in crate::compiler::nav) unbounded_fall: bool,
    /// The declared jumps, `(from, to)` (spec-0113 §6).
    pub(in crate::compiler::nav) declared_jumps: Vec<([i32; 3], [i32; 3])>,
}

impl Premises {
    /// Every premise `plan` states, with the world-load gate seals `seals`
    /// measured off the assembled bytes the world is being built from.
    ///
    /// `seals` is an argument rather than a plan field because it is a
    /// **measurement of the assembled world**, not a declaration: the plan is
    /// built before any `.nbt` byte is read, and an edited world's seals are the
    /// edited bytes'. Every call site that builds a world from a campaign has an
    /// [`crate::compiler::assembled::Assembled`] in hand and passes that world's own
    /// `gate_seals`.
    ///
    /// Two classes of measured seal are deliberately dropped by the model that
    /// reads these, and both drops are stated on [`World::modelled_seals`]: a
    /// gate authored open is not a seal, and a timed gate's region is its clock's.
    pub fn of_plan(plan: &Plan, seals: Vec<crate::compiler::assembled::GateSeal>) -> Self {
        Premises {
            ambient: Ambient::of_plan(plan),
            base: delvewright_dsl::horizon_base(&plan.campaign.world.content.horizon).token(),
            built: built_volume(plan),
            lethal_regions: plan
                .lethal_volumes
                .iter()
                .filter(|v| v.staged.is_none())
                .map(|v| (v.id.clone(), v.region))
                .collect(),
            staged_lethal: plan
                .lethal_volumes
                .iter()
                .filter_map(|v| {
                    Some(StagedVolume {
                        id: v.id.clone(),
                        region: v.region,
                        gate: v.staged.clone()?,
                    })
                })
                .collect(),
            loop_slabs: plan
                .loops
                .iter()
                .map(|l| StagedVolume {
                    id: l.id.clone(),
                    region: l.slab,
                    gate: l.staged_gate(plan.campaign),
                })
                .collect(),
            furniture_regions: plan.furniture.clone(),
            world_load_seals: seals,
            clocked_gates: plan.timed_gates.iter().map(|g| g.gate_region).collect(),
            // Where the party is required to stand, by name. A proof that finds
            // the walk region wet can then say WHICH objective is in the water
            // rather than only which cell is — the difference between a
            // coordinate and a thing to fix.
            objective_cells: plan
                .critical_path
                .iter()
                .filter_map(|s| Some((s.objective()?.to_string(), s.pos()?)))
                .collect(),
            unbounded_fall: false,
            declared_jumps: Vec::new(),
        }
        .with_body_of(plan)
    }

    /// These premises with **the body's own** taken from `plan`: its declared
    /// jumps (spec-0113 §6) and whether its kit makes every fall survivable
    /// (§4.2). They are facts about the player, not about the world it is
    /// in, so a world that declines the campaign's premises
    /// ([`Premises::geometry_only`]) still carries them when the question
    /// asked of it is where a body can be.
    #[must_use]
    pub fn with_body_of(mut self, plan: &Plan) -> Self {
        self.unbounded_fall = plan.movement.unbounded_fall();
        self.declared_jumps = plan.jumps.iter().map(|j| (j.from, j.to)).collect();
        self
    }

    /// A world that is **geometry alone**: no ambient beyond [`Ambient::Void`],
    /// no built extent, no lethal volume, no gate seal, no teleport source.
    ///
    /// This is not a default and it is not an omission — it is a claim, made by
    /// name at the site that makes it, that the question being asked of this
    /// world is a question about blocks. Every production call site of it is
    /// enumerated by `premise_declines_are_enumerated`, which reds when a new one
    /// appears: the whole point of this type is that declining a premise is
    /// visible, and a decline nobody has to write down is the defect wearing the
    /// repair's clothes.
    ///
    /// Synthetic worlds ([`World::from_solid_cells`],
    /// [`World::from_solid_and_flooded`], the unit tests' hand-built
    /// [`crate::compiler::assembled::Occupancy`]) have no campaign at all, so there is
    /// nothing for them to state.
    pub fn geometry_only() -> Self {
        Premises {
            ambient: Ambient::Void,
            // A synthetic world declares nothing; `void` is what a campaign
            // that writes no horizon gets, so it is what such a world is.
            base: "void",
            built: Vec::new(),
            lethal_regions: Vec::new(),
            staged_lethal: Vec::new(),
            loop_slabs: Vec::new(),
            furniture_regions: Vec::new(),
            world_load_seals: Vec::new(),
            clocked_gates: BTreeSet::new(),
            objective_cells: Vec::new(),
            unbounded_fall: false,
            declared_jumps: Vec::new(),
        }
    }
}

impl World {
    /// Build the occupancy model from the plan's placed pieces and the structure
    /// `.nbt` bytes, via the shared assembled-world model. Every non-air cell of
    /// that settled map is a solid cell here — so a `sand`/`gravel` floor that
    /// falls out of the void world is passable (a hole), exactly as in game
    /// — not a phantom floor the model wrongly seats mobs on.
    pub fn from_plan(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Self {
        let assembled = crate::compiler::assembled::assemble(plan, structures);
        let seals = assembled.gate_seals.clone();
        Self::from_occupancy(
            crate::compiler::assembled::occupancy_over(&assembled.blocks, &assembled.open_gates),
            Premises::of_plan(plan, seals),
        )
    }

    /// The measured gates the model actually treats as **shut at world-load**.
    ///
    /// Two classes of measured seal are deliberately **dropped** here, and both
    /// drops are optimism the model states rather than hides:
    ///
    /// * A gate authored **open** (`blocked == 0`) is not a seal at all. The
    ///   island boulder is twenty-seven cells of air until a `close-gate` fills
    ///   it, and that firing is already in [`Plan::region_events`].
    /// * A **timed gate**'s region (spec-0016 §4) is filled and cleared by its own
    ///   clock, twice a cycle, from world-load. Whatever the prefab authors there,
    ///   the region is open for part of every cycle, so modelling it as
    ///   permanently shut would refuse a campaign that plays. The clock's own
    ///   proofs own that region; this one declines it.
    ///
    /// A **shortcut**'s gate is kept: `Plan::build` already registers it as a
    /// world-load `Fill` for exactly this reason, and a duplicate write of the
    /// same region at the same step is the same verdict either way — keeping it
    /// is what lets the diagnostic NAME a shortcut door that walls off the only
    /// route.
    pub(in crate::compiler::nav) fn modelled_seals(
        &self,
    ) -> impl Iterator<Item = &crate::compiler::assembled::GateSeal> {
        self.world_load_seals
            .iter()
            .filter(|s| s.sealed() && !self.clocked_gates.contains(&s.region))
    }

    /// **Every gate the world resolves**, open or shut at world-load, each with
    /// whether a clock owns its region (spec-0016 §4) — the population `DW1011`
    /// enumerates, in `(area, anchor)` order.
    pub(in crate::compiler::nav) fn every_gate(
        &self,
    ) -> impl Iterator<Item = (&crate::compiler::assembled::GateSeal, bool)> {
        self.world_load_seals
            .iter()
            .map(|s| (s, self.clocked_gates.contains(&s.region)))
    }

    /// Whether any gate is modelled shut — the cheap guard that keeps a campaign
    /// with no sealed gate on exactly its old routing.
    pub(in crate::compiler::nav) fn has_world_load_seals(&self) -> bool {
        self.modelled_seals().next().is_some()
    }

    /// Every gate anchor whose world-load seal a route's `cells` pass through, in
    /// `(area, anchor)` order — the blame list [`DW_GATE_NEVER_OPENED`] names.
    pub(in crate::compiler::nav) fn gate_seals_over(
        &self,
        cells: &[[i32; 3]],
    ) -> Vec<&crate::compiler::assembled::GateSeal> {
        self.modelled_seals()
            .filter(|s| {
                let (lo, hi) = s.region;
                cells
                    .iter()
                    .any(|c| (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i])))
            })
            .collect()
    }

    /// The world-load gate ledger, for the binding count a validation artifact must
    /// state (CLAUDE.md): every gate examined, and how many of them the model
    /// treats as shut.
    pub fn gate_seal_ledger(&self) -> serde_json::Value {
        let mut v = crate::compiler::assembled::gate_seal_ledger(
            &self.world_load_seals,
            self.modelled_seals().count(),
        );
        // Each gate says whether a clock owns its region (spec-0016 §4): its
        // blocks at any instant are the clock's phase, so a reader comparing a
        // server's save against the model (`tools/ci/check-written-world.py`)
        // counts those cells as the clock's, never as the model's.
        if let Some(gates) = v.get_mut("gates").and_then(|g| g.as_array_mut()) {
            for (g, s) in gates.iter_mut().zip(&self.world_load_seals) {
                g["clocked"] = serde_json::json!(self.clocked_gates.contains(&s.region));
            }
        }
        v
    }

    /// Whether the layout resolved any gate anchor at all — a campaign with none
    /// emits no ledger, so a ledger that exists and reports zero is a finding.
    pub fn has_gate_anchors(&self) -> bool {
        !self.world_load_seals.is_empty()
    }

    /// A copy of this world with **every semantic exclusion lifted** — no lethal
    /// volume and no furniture region: the counterfactual `DW0510` is derived
    /// from, and the population `DW0891` is measured over (spec-0062 §2,
    /// spec-0065 §4.2).
    ///
    /// A route that fails on the real world and succeeds on this one failed
    /// *because of* an exclusion, and the cells it would have walked name which
    /// volume or which table. Without the counterfactual the author gets "no
    /// collision-free path" over geometry that looks perfectly open — the
    /// reachability report that sends someone to fix the prefab.
    ///
    /// One function for both kinds, on purpose: a check that must not be
    /// improved by an exclusion reads this, and a second counterfactual lifting
    /// only one kind would be the hatch a furniture declaration over caught floor
    /// walks through.
    pub fn without_exclusions(&self) -> World {
        let mut w = self.clone_world();
        w.lethal = CellSet::new();
        w.lethal_regions = Vec::new();
        w.furniture = CellSet::new();
        w.furniture_regions = Vec::new();
        w
    }

    /// A copy of this world with its **furniture** lifted and every other
    /// premise kept — the counterfactual a walked `move-npc` / `move-actor` leg
    /// is asked over when it cannot route (`DW0510`'s furniture shape). Those
    /// legs carry no lethal counterfactual, so lifting lethality there too would
    /// report a route through a kill box as a table's fault.
    pub fn without_furniture(&self) -> World {
        let mut w = self.clone_world();
        w.furniture = CellSet::new();
        w.furniture_regions = Vec::new();
        w
    }

    /// A field-for-field copy, for the counterfactuals that change one premise.
    pub(in crate::compiler::nav) fn clone_world(&self) -> World {
        World {
            piece: self.piece,
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            waterloggable: self.waterloggable.clone(),
            lava: self.lava.clone(),
            material: self.material.clone(),
            fingerprint: std::sync::OnceLock::new(),
            climb_holds: self.climb_holds.clone(),
            climb: self.climb.clone(),
            unheld_climb: self.unheld_climb.clone(),
            objective_cells: self.objective_cells.clone(),
            unbounded_fall: self.unbounded_fall,
            declared_jumps: self.declared_jumps.clone(),
            holder_stances: self.holder_stances.clone(),
            floors: self.floors.clone(),
            seating: self.seating.clone(),
            hiding: self.hiding.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            keep_out: self.keep_out.clone(),
            staged_lethal: self.staged_lethal.clone(),
            loop_slabs: self.loop_slabs.clone(),
            furniture: self.furniture.clone(),
            furniture_regions: self.furniture_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            base: self.base,
            built: self.built.clone(),
        }
    }

    /// This world without its declared jumps — the walk relation they are
    /// judged against (`DW1041`: a jump across a gap a walk already crosses is
    /// no jump).
    pub fn without_declared_jumps(&self) -> World {
        let mut w = self.clone_world();
        w.declared_jumps = Vec::new();
        w
    }

    /// This world with a straight run-up of `k` floor cells laid behind
    /// `from` along the jump toward `to`, each the launch cell's own floor with
    /// air over it — the course a declared jump would need, flown to say how
    /// long a run-up it needs (`DW1041`).
    pub fn with_flat_run_up(&self, from: [i32; 3], to: [i32; 3], k: u32) -> World {
        let mut w = self.clone_world();
        let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
        let axis = if dx.abs() >= dz.abs() {
            [dx.signum(), 0]
        } else {
            [0, dz.signum()]
        };
        let support = [from[0], from[1] - 1, from[2]];
        let top = self.partial.get(&support).copied();
        for i in 1..=k as i32 {
            let c = [from[0] - axis[0] * i, from[1], from[2] - axis[1] * i];
            let under = [c[0], c[1] - 1, c[2]];
            w.solid.insert(under);
            match top {
                Some(t) => {
                    w.partial.insert(under, t);
                }
                None => {
                    w.partial.remove(&under);
                }
            }
            for y in 0..3 {
                let a = [c[0], c[1] + y, c[2]];
                w.solid.remove(&a);
                w.tall.remove(&a);
                w.use_gates.remove(&a);
                w.flooded.remove(&a);
                w.partial.remove(&a);
            }
        }
        w
    }

    /// Whether this world carries any furniture region at all. Call sites skip
    /// the furniture counterfactual entirely when it does not.
    pub fn has_furniture(&self) -> bool {
        !self.furniture_regions.is_empty()
    }

    /// Whether a body standing in `c` would rest on a **solid furniture cell** —
    /// the one term [`World::standable_fp`] adds for spec-0065, asked per
    /// column of the footprint. Membership of the support cell, never a reach:
    /// a body beside a table, feet on the floor, is untouched.
    fn on_furniture_fp(&self, c: [i32; 3], fp: &Footprint) -> bool {
        if self.furniture.is_empty() {
            return false;
        }
        fp.cols.iter().any(|&[dx, dz]| {
            let support = [c[0] + dx, c[1] - 1, c[2] + dz];
            self.furniture.contains(&support) && self.is_solid(support)
        })
    }

    /// The furniture anchors a body standing in any of `cells` would rest on, in
    /// the plan's order, deduplicated — who to blame for a route that exists
    /// only when the furniture is lifted. Asked for the player's single column,
    /// which is what every blamed route here was routed for.
    pub(in crate::compiler::nav) fn furniture_over(&self, cells: &[[i32; 3]]) -> Vec<&str> {
        self.furniture_over_fp(cells, &Footprint::player())
    }

    /// [`World::furniture_over`] for a body of footprint `fp`.
    pub(in crate::compiler::nav) fn furniture_over_fp(
        &self,
        cells: &[[i32; 3]],
        fp: &Footprint,
    ) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for (id, (lo, hi)) in &self.furniture_regions {
            let hit = cells.iter().any(|c| {
                fp.cols.iter().any(|&[dx, dz]| {
                    let s = [c[0] + dx, c[1] - 1, c[2] + dz];
                    (0..3).all(|i| lo[i] <= s[i] && s[i] <= hi[i]) && self.is_solid(s)
                })
            });
            if hit && !out.contains(&id.as_str()) {
                out.push(id.as_str());
            }
        }
        out
    }

    /// **The furniture binding** (spec-0065 §4.3): regions, their solid cells,
    /// and how many cells a player could stand in on the bare geometry that the
    /// exclusion withholds. `(F, S, W)`.
    pub fn furniture_census(&self) -> (usize, usize, usize) {
        let solid: Vec<[i32; 3]> = self
            .furniture
            .iter()
            .filter(|c| self.is_solid(*c))
            .collect();
        let open = self.without_furniture();
        let fp = Footprint::player();
        let withheld = solid
            .iter()
            .map(|s| [s[0], s[1] + 1, s[2]])
            .filter(|c| open.standable_fp(*c, &fp) && !self.standable_fp(*c, &fp))
            .count();
        (self.furniture_regions.len(), solid.len(), withheld)
    }

    /// How many of `cells` a body of footprint `fp` would stand on furniture in —
    /// the `0 standing on furniture` the binding line states, computed rather
    /// than typed.
    pub fn cells_on_furniture(&self, cells: &[[i32; 3]], fp: &Footprint) -> usize {
        cells
            .iter()
            .filter(|c| self.on_furniture_fp(**c, fp))
            .count()
    }

    /// Whether this world carries any lethal-volume cell at all. Call sites skip
    /// the counterfactual clone entirely when it does not.
    pub fn has_lethal(&self) -> bool {
        !self.lethal.is_empty()
    }

    /// Whether `c` lies inside a declared lethal volume.
    pub fn is_lethal(&self, c: [i32; 3]) -> bool {
        self.lethal.contains(&c)
    }

    /// How many cells this world's lethal volumes occupy — the binding count the
    /// `validation/lethal-gate.json` ledger states out loud.
    pub fn lethal_cells(&self) -> usize {
        self.lethal.len()
    }

    /// The ids of the lethal volumes covering any of `cells`, in declaration
    /// order — who to blame for a route that only exists when lethality is
    /// ignored. Deterministic: declaration order, no hashing (ADR-0006).
    ///
    /// Asked of the **player's body**, not of the cell, for the same reason
    /// [`World::meets_lethal_fp`] is: the cells this names are the cells the
    /// counterfactual walk crossed, and a walker is refused one of them the
    /// moment its hitbox can reach a volume. A cell-only reading here would
    /// leave the refusal with no volume to blame and send the author to look at
    /// geometry that was never wrong — the exact failure `lethal_regions` was
    /// carried to prevent.
    pub(in crate::compiler::nav) fn lethal_volumes_over(&self, cells: &[[i32; 3]]) -> Vec<&str> {
        let fp = Footprint::player();
        self.lethal_regions
            .iter()
            .filter(|(_, (lo, hi))| {
                cells
                    .iter()
                    .any(|c| self.body_can_meet_volume(*c, &fp, *lo, *hi))
            })
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// **Can a body of this footprint, standing in `c`, be caught by a declared
    /// lethal volume?**
    ///
    /// The routing model's whole share of the footprint rule. A volume kills by
    /// a box selector, vanilla selects on hitbox intersection, and a body
    /// standing in the cell beside a volume's face reaches into it — so the
    /// impassable set is the volume widened by the walker's half-width, and it
    /// is the WALKER's, which is why this takes a footprint instead of adding
    /// cells to `lethal`. `lethal` stays the declared cells, because that is
    /// what `lethal_cells()` reports as the proof's binding count and what
    /// `is_lethal` answers about.
    /// Whether a body of this footprint in `c` meets a volume this leg keeps out
    /// of ([`World::keep_out`]), by the rule a lethal volume is met by.
    fn meets_keep_out_fp(&self, c: [i32; 3], fp: &Footprint) -> bool {
        self.keep_out
            .iter()
            .any(|(_, (lo, hi))| self.body_can_meet_volume(c, fp, *lo, *hi))
    }

    /// The armed objectives whose completion volumes a body standing on any of
    /// `cells` would meet — who to name for a leg that routes only when the
    /// keep-out is lifted. In the order the leg holds them.
    pub(in crate::compiler::nav) fn keep_out_over(&self, cells: &[[i32; 3]]) -> Vec<String> {
        let fp = Footprint::player();
        self.keep_out
            .iter()
            .filter(|(_, (lo, hi))| {
                cells
                    .iter()
                    .any(|c| self.body_can_meet_volume(*c, &fp, *lo, *hi))
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Whether this leg keeps out of any completion volume at all.
    pub(in crate::compiler::nav) fn has_keep_out(&self) -> bool {
        !self.keep_out.is_empty()
    }

    /// This world with every keep-out volume lifted — the counterfactual that
    /// names an armed objective as what closed a leg.
    pub(in crate::compiler::nav) fn without_keep_out(&self) -> World {
        let mut w = self.clone_world();
        w.keep_out = Vec::new();
        w
    }

    fn meets_lethal_fp(&self, c: [i32; 3], fp: &Footprint) -> bool {
        if self.lethal_regions.is_empty() {
            return false;
        }
        self.lethal_regions
            .iter()
            .any(|(_, (lo, hi))| self.body_can_meet_volume(c, fp, *lo, *hi))
    }

    /// **Can a body of this footprint, in cell `c`, meet the volume `lo..=hi`?**
    /// — [`delvewright_dsl::metrics::feet_can_meet_volume`] with the feet where
    /// this model puts them ([`World::feet_16_fp`]): on the collision top of the
    /// block under the cell, which for a partial block is below the cell floor.
    ///
    /// The one answer every proof that asks whether a body in a cell is caught
    /// by a volume takes — the router's keep-out, the reach flood, the
    /// danger-visibility population and the blind reach — so the height a body
    /// stands at is read off the block it stands on in all of them, and a body
    /// on an upward dripstone tip (feet 11/16 into the tip's cell) meets a
    /// volume drawn in the tip course, as it does in the game.
    pub fn body_can_meet_volume(
        &self,
        c: [i32; 3],
        fp: &Footprint,
        lo: [i32; 3],
        hi: [i32; 3],
    ) -> bool {
        delvewright_dsl::metrics::feet_can_meet_volume(c, self.feet_16_fp(c, fp), fp.body(), lo, hi)
    }

    /// Build the walkability model from a collision-classified [`Occupancy`] and
    /// the [`Premises`] the campaign states about the world it sits in — the
    /// **one door** every world goes through, so that adding a premise is one
    /// edit in [`Premises::of_plan`] rather than a grep across the call sites
    /// somebody remembers.
    ///
    /// The occupancy sets and the premises are separate arguments because they
    /// are separate kinds of fact — the first is measured off the assembled
    /// bytes, the second is declared by the campaign — but neither is optional.
    /// A call site with a campaign in hand writes [`Premises::of_plan`]; one
    /// without says [`Premises::geometry_only`] and means it.
    pub fn from_occupancy(occ: crate::compiler::assembled::Occupancy, premises: Premises) -> Self {
        Self::from_cells(
            Cells {
                solid: occ.solid.into(),
                hiding: occ.hiding.into(),
                tall: occ.tall.into(),
                use_gates: occ.use_gates.into(),
                flooded: occ.flooded.into(),
                partial: occ.partial.into(),
                waterloggable: occ.waterloggable.into(),
                lava: occ.lava.into(),
                material: occ.material.into(),
                climb_holds: std::sync::Arc::new(occ.climb),
                unheld_climb: std::sync::Arc::new(occ.unheld_climb),
                footprints: occ.footprints,
                leaves: occ.leaves,
                hurting: occ.hurting.into(),
            },
            premises,
        )
    }

    /// This world's block-derived cells under `premises` instead of its own —
    /// what [`World::from_occupancy`] would build from the same occupancy.
    /// The cells are shared, not copied; anything a proof has derived on top
    /// of this world (pinned cells, runtime floods) is not carried.
    pub fn with_premises(&self, premises: Premises) -> World {
        let mut w = Self::from_cells(
            Cells {
                solid: self.solid.clone(),
                hiding: self.hiding.clone(),
                tall: self.tall.clone(),
                use_gates: self.use_gates.clone(),
                flooded: self.flooded.clone(),
                partial: self.partial.clone(),
                waterloggable: self.waterloggable.clone(),
                lava: self.lava.clone(),
                material: self.material.clone(),
                climb_holds: self.climb_holds.clone(),
                unheld_climb: self.unheld_climb.clone(),
                footprints: self.floors.footprints.clone(),
                leaves: self.floors.leaves.clone(),
                hurting: self.seating.hurting.clone(),
            },
            premises,
        );
        w.piece = self.piece;
        w
    }

    /// This world with `extra` cells added to its solid set, as full cubes — a
    /// test's way to wall a cell off. A block the build writes is classified
    /// through the collision table instead ([`crate::compiler::light::lit_world`]).
    pub fn with_extra_solid(mut self, extra: &BTreeSet<[i32; 3]>) -> World {
        self.solid.extend(extra.iter().copied());
        self.solid.compact();
        // A full cube of stone hides what stands behind it.
        self.hiding.extend(extra.iter().copied());
        self.hiding.compact();
        for c in extra {
            self.floors.footprints.remove(c);
        }
        self.resieve()
    }

    /// This world with [`Floors::sieve`] derived from its own cells — every
    /// view ends here, so no view answers with another's sieve.
    ///
    /// A cell is a sieve when its block does not cover it and a body-wide
    /// square centred over it meets no box in its own course or the course
    /// under it ([`delvewright_dsl::blockshape::body_falls_through`]). A cell
    /// that is not solid is read as empty unless a body would land on it (a
    /// barrier, a gate leaf), which can only find a gap the world closes, never
    /// close one it leaves open.
    pub(in crate::compiler::nav) fn resieve(mut self) -> World {
        use delvewright_dsl::blockshape::{EMPTY_FOOTPRINT, FULL_FOOTPRINT, body_falls_through};
        // The reach graph's digest reads the sieve ([`World::fingerprint`]).
        self.fingerprint = std::sync::OnceLock::new();
        self.floors.sieve.clear();
        if self.floors.footprints.is_empty() {
            return self;
        }
        let cells: Vec<[i32; 3]> = self.floors.footprints.keys().copied().collect();
        let mut sieve = BTreeSet::new();
        // What a falling body meets in cell `n`: a solid block's footprint, a
        // whole cell for a barrier or a gate leaf it would land on, nothing
        // for anything a body passes or sinks into.
        let course = |y: i32, c: [i32; 3]| {
            let mut layer = [[EMPTY_FOOTPRINT; 3]; 3];
            for (dz, row) in layer.iter_mut().enumerate() {
                for (dx, fp) in row.iter_mut().enumerate() {
                    let n = [c[0] + dx as i32 - 1, y, c[2] + dz as i32 - 1];
                    if self.solid.contains(&n) {
                        *fp = self
                            .floors
                            .footprints
                            .get(&n)
                            .copied()
                            .unwrap_or(FULL_FOOTPRINT);
                    } else if self.tall.contains(&n) || self.use_gates.contains(&n) {
                        *fp = FULL_FOOTPRINT;
                    }
                }
            }
            layer
        };
        for c in cells {
            if !self.solid.contains(&c) {
                continue;
            }
            if body_falls_through(&course(c[1], c), &course(c[1] - 1, c)) {
                sieve.insert(c);
            }
        }
        self.floors.sieve = sieve;
        self
    }

    /// Whether a body over `c` falls through it ([`Floors::sieve`]).
    pub(in crate::compiler::nav) fn is_sieve(&self, c: [i32; 3]) -> bool {
        self.floors.sieve.contains(&c)
    }

    fn from_cells(cells: Cells, premises: Premises) -> Self {
        let floors = Floors {
            footprints: cells.footprints,
            sieve: BTreeSet::new(),
            leaves: cells.leaves,
            leaf_use: true,
        };
        World {
            piece: None,
            solid: cells.solid,
            tall: cells.tall,
            use_gates: cells.use_gates,
            flooded: cells.flooded,
            partial: cells.partial,
            waterloggable: cells.waterloggable,
            lava: cells.lava,
            material: cells.material,
            fingerprint: std::sync::OnceLock::new(),
            climb: cells.climb_holds.keys().copied().collect(),
            climb_holds: cells.climb_holds,
            unheld_climb: cells.unheld_climb,
            lethal: premises
                .lethal_regions
                .iter()
                .flat_map(|(_, (lo, hi))| crate::compiler::assembled::region_cells(*lo, *hi))
                .collect(),
            lethal_regions: premises.lethal_regions,
            keep_out: Vec::new(),
            staged_lethal: premises.staged_lethal,
            loop_slabs: premises.loop_slabs,
            furniture: premises
                .furniture_regions
                .iter()
                .flat_map(|(_, (lo, hi))| crate::compiler::assembled::region_cells(*lo, *hi))
                .collect(),
            furniture_regions: premises.furniture_regions,
            pinned: CellSet::new(),
            world_load_seals: premises.world_load_seals,
            clocked_gates: premises.clocked_gates,
            flood_written: CellSet::new(),
            flood_regions: Vec::new(),
            ambient: premises.ambient,
            base: premises.base,
            built: premises.built,
            objective_cells: premises.objective_cells,
            unbounded_fall: premises.unbounded_fall,
            declared_jumps: premises.declared_jumps,
            holder_stances: BTreeMap::new(),
            floors,
            hiding: cells.hiding,
            seating: crate::compiler::nav::Seating::of_hurting(cells.hurting),
        }
        .resieve()
    }

    /// This world with its world-generator [`Ambient`] declared (spec-0013) and
    /// its [`built_volume`] recorded. The occupancy sets are untouched — both are
    /// *premises* ([`verify_boundary_safety`], [`measure_fluid_escape`]), not
    /// geometry.
    ///
    /// **Synthetic worlds only.** A world built from a campaign gets both through
    /// [`Premises::of_plan`], along with every other premise, and that is the
    /// point of [`Premises`]: this method can set two of six fields, and a
    /// campaign world assembled out of it would be missing four. It survives
    /// because the unit tests build worlds out of bare cell sets and need to say
    /// what is in the columns around them.
    ///
    /// The two travel in one argument list on purpose. They answer the same
    /// question from opposite sides — *what is in a column the content did not
    /// build* and *which columns are those* — and a call site that declared the
    /// first without the second is how a void world came to have no idea where
    /// its own content ended.
    /// **The base the campaign declared** — the word its author wrote, not the
    /// ambient it resolves to. Every binding line that prints a horizon prints
    /// this (spec-0060 §10.7).
    pub fn base(&self) -> &'static str {
        self.base
    }

    /// This world with the stance each holder `collect` is taken from
    /// ([`World::holder_stances`], spec-0111 §5.2): holder cell → stance cell.
    #[must_use]
    pub fn with_holder_stances(mut self, stances: BTreeMap<[i32; 3], [i32; 3]>) -> Self {
        self.holder_stances = stances;
        self
    }

    pub fn with_ambient(mut self, ambient: Ambient, built: Vec<BuiltPiece>) -> Self {
        self.ambient = ambient;
        self.built = built;
        self.fingerprint = std::sync::OnceLock::new();
        self
    }

    /// This world with `cells` declared waterloggable — a fact a real world
    /// carries off its own block map ([`crate::compiler::assembled::Occupancy`])
    /// and a synthetic one has no blocks to carry.
    ///
    /// `#[cfg(test)]` on purpose: production code that set this by hand would be
    /// stating a block fact it did not read from the blocks, which is the shape
    /// the premise type exists to prevent.
    #[cfg(test)]
    pub fn with_waterloggable(mut self, cells: BTreeSet<[i32; 3]>) -> Self {
        self.waterloggable = cells.into();
        self
    }

    /// Whether `c` falls inside a placed piece's AABB — i.e. whether the
    /// content, rather than the world generator, decided what is in that cell.
    pub fn is_built(&self, c: [i32; 3]) -> bool {
        self.built
            .iter()
            .any(|(_, (lo, hi))| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]))
    }

    /// Whether `c` is ambient sea water: inside the ocean generator's water
    /// layers and not overwritten by a placed piece. Always `false` under
    /// [`Ambient::Void`], which has no water anywhere.
    pub(in crate::compiler::nav) fn ambient_water(&self, c: [i32; 3]) -> bool {
        match &self.ambient {
            Ambient::Void => false,
            Ambient::Ocean(sea) => c[1] > sea.floor_top && c[1] <= sea.level && !self.is_built(c),
        }
    }

    /// Whether a cell is occupied by a solid block in the assembled world.
    pub fn solid_at(&self, c: [i32; 3]) -> bool {
        self.is_solid(c)
    }

    /// A copy of this world with `extra` cells forced solid — a `close-gate`'s
    /// sealed region for the completability proof (DSL v0.6). The base occupancy
    /// model treats every gate cell as passable; sealing a gate for the legs that
    /// occur after it closes makes a path that must re-cross it fail routing.
    pub(in crate::compiler::nav) fn with_sealed(&self, extra: &BTreeSet<[i32; 3]>) -> World {
        self.with_sealed_by(extra, &BTreeMap::new(), extra)
    }

    /// [`World::with_sealed`] where the sealing block is known per cell: a cell
    /// in `footprints` holds a block that does not cover it (iron bars laid
    /// through a floor), and whether a body over it is held is then judged
    /// from that footprint ([`World::resieve`]) rather than assumed. Every
    /// other cell of `extra` is a full cube.
    pub(in crate::compiler::nav) fn with_sealed_by(
        &self,
        extra: &BTreeSet<[i32; 3]>,
        footprints: &BTreeMap<[i32; 3], delvewright_dsl::blockshape::Footprint16>,
        hides: &BTreeSet<[i32; 3]>,
    ) -> World {
        let mut solid = self.solid.clone();
        solid.extend(extra.iter().copied());
        // What the eye meets in a sealed cell is the block that seals it.
        let mut hiding = self.hiding.clone();
        for c in extra {
            if hides.contains(c) {
                hiding.insert(*c);
            } else {
                hiding.remove(c);
            }
        }
        // A sealed gate cell is a full-cube wall, never a partial floor.
        let mut partial = self.partial.clone();
        for c in extra {
            partial.remove(c);
        }
        // These cells are this proof's premise from here on: a later runtime clear
        // may not undo them (see `World::pinned`).
        let mut pinned = self.pinned.clone();
        pinned.extend(extra.iter().copied());
        let mut w = World {
            piece: self.piece,
            solid,
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial,
            waterloggable: self.waterloggable.clone(),
            lava: self.lava.clone(),
            material: self.material.clone(),
            fingerprint: std::sync::OnceLock::new(),
            climb_holds: self.climb_holds.clone(),
            climb: self.climb.clone(),
            unheld_climb: self.unheld_climb.clone(),
            objective_cells: self.objective_cells.clone(),
            unbounded_fall: self.unbounded_fall,
            declared_jumps: self.declared_jumps.clone(),
            holder_stances: self.holder_stances.clone(),
            floors: self.floors.clone(),
            seating: self.seating.clone(),
            hiding,
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            keep_out: self.keep_out.clone(),
            staged_lethal: self.staged_lethal.clone(),
            loop_slabs: self.loop_slabs.clone(),
            furniture: self.furniture.clone(),
            furniture_regions: self.furniture_regions.clone(),
            pinned,
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            base: self.base,
            built: self.built.clone(),
        };
        w.drop_unheld_climbs(extra);
        for c in extra {
            // A sealed cell holds the sealing block, not the trapdoor.
            w.floors.leaves.remove(c);
            match footprints.get(c) {
                Some(fp) => w.floors.footprints.insert(*c, *fp),
                None => w.floors.footprints.remove(c),
            };
        }
        w.resieve()
    }

    /// A copy of this world with `extra` cells **emptied of blocks** — the dual of
    /// [`World::with_sealed`], and what a runtime `clear-region` (or an
    /// `open-gate`, whose gate cells the assembled model already holds empty) does
    /// to the geometry from its point in the quest DAG.
    ///
    /// "Emptied of blocks" is exactly the four block-derived classes: a cleared
    /// cell is no longer a full cube, a 1.5-tall barrier, a closed fence gate, or a
    /// partial floor. `flooded` is deliberately **left alone**: a `fill … air`
    /// against a cell the model floods does not remove the water, it lets the water
    /// back in, so a cleared cell the model already knows is wet stays impassable.
    ///
    /// The one case this does not model is a clear that *opens* a dry region into
    /// adjacent water — the model would call the new cells dry and the server would
    /// flood them. Re-deriving the flood needs the block map, which this collision
    /// view does not carry; until it does, that campaign's route proof is optimistic
    /// and the limitation is stated here and in `docs/reference/compiler.md` rather
    /// than left to be discovered.
    pub(in crate::compiler::nav) fn with_cleared(&self, extra: &BTreeSet<[i32; 3]>) -> World {
        let mut w = World {
            piece: self.piece,
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            waterloggable: self.waterloggable.clone(),
            lava: self.lava.clone(),
            material: self.material.clone(),
            fingerprint: std::sync::OnceLock::new(),
            climb_holds: self.climb_holds.clone(),
            climb: self.climb.clone(),
            unheld_climb: self.unheld_climb.clone(),
            objective_cells: self.objective_cells.clone(),
            unbounded_fall: self.unbounded_fall,
            declared_jumps: self.declared_jumps.clone(),
            holder_stances: self.holder_stances.clone(),
            floors: self.floors.clone(),
            seating: self.seating.clone(),
            hiding: self.hiding.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            keep_out: self.keep_out.clone(),
            staged_lethal: self.staged_lethal.clone(),
            loop_slabs: self.loop_slabs.clone(),
            furniture: self.furniture.clone(),
            furniture_regions: self.furniture_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            base: self.base,
            built: self.built.clone(),
        };
        for c in extra {
            if w.pinned.contains(c) {
                continue; // another proof's premise, not a block this write owns
            }
            w.solid.remove(c);
            w.hiding.remove(c);
            w.tall.remove(c);
            w.use_gates.remove(c);
            w.floors.leaves.remove(c);
            w.partial.remove(c);
            w.floors.footprints.remove(c);
        }
        let owned: BTreeSet<[i32; 3]> = extra
            .iter()
            .filter(|c| !self.pinned.contains(c))
            .copied()
            .collect();
        w.drop_unheld_climbs(&owned);
        w.resieve()
    }

    /// A copy of this world with `extra` cells holding **free fluid** — the third
    /// thing a runtime region write can leave behind
    /// ([`crate::compiler::plan::RegionWrite::Flood`]), beside [`World::with_sealed`]'s wall
    /// and [`World::with_cleared`]'s empty box.
    ///
    /// A flooded cell is impassable and **never floor**, so this is not a weaker
    /// seal: it blocks passage exactly as a wall does, and additionally denies the
    /// footing a wall would have provided. Which is the whole point — `fill-region
    /// … minecraft:water` was previously routed as a wall *and* walked on top of.
    ///
    /// The written cells stop being any block class (a fill carries no `replace`
    /// filter, so it overwrites whatever was there), are recorded in
    /// `flood_written`/`flood_regions` for [`World::without_runtime_flood`], and are
    /// **pinned** for the same reason a seal is: they are this proof's premise, and
    /// a later `clear-region` laid over them may not quietly delete the water.
    fn with_flooded(&self, extra: &BTreeSet<[i32; 3]>, regions: &[([i32; 3], [i32; 3])]) -> World {
        let mut w = World {
            piece: self.piece,
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            waterloggable: self.waterloggable.clone(),
            lava: self.lava.clone(),
            material: self.material.clone(),
            fingerprint: std::sync::OnceLock::new(),
            climb_holds: self.climb_holds.clone(),
            climb: self.climb.clone(),
            unheld_climb: self.unheld_climb.clone(),
            objective_cells: self.objective_cells.clone(),
            unbounded_fall: self.unbounded_fall,
            declared_jumps: self.declared_jumps.clone(),
            holder_stances: self.holder_stances.clone(),
            floors: self.floors.clone(),
            seating: self.seating.clone(),
            hiding: self.hiding.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            keep_out: self.keep_out.clone(),
            staged_lethal: self.staged_lethal.clone(),
            loop_slabs: self.loop_slabs.clone(),
            furniture: self.furniture.clone(),
            furniture_regions: self.furniture_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            base: self.base,
            built: self.built.clone(),
        };
        for c in extra {
            w.solid.remove(c);
            w.tall.remove(c);
            w.use_gates.remove(c);
            w.floors.leaves.remove(c);
            w.partial.remove(c);
            w.flooded.insert(*c);
            w.pinned.insert(*c);
            w.flood_written.insert(*c);
            w.floors.footprints.remove(c);
        }
        w.flood_regions.extend_from_slice(regions);
        w.drop_unheld_climbs(extra);
        w.resieve()
    }

    /// A copy of this world with `extra` cells holding a block that **may or may not
    /// be there** — a solid laid by a beat the party is not forced to play
    /// ([`RegionState::unforced`]).
    ///
    /// The cell is made impassable and **not floor**: impassable because the party
    /// may arrive to find the box walled, not floor because they may equally arrive
    /// to find it as the world built it. `tall` is exactly that class already — the
    /// model's word for "blocks passage, and nothing stands on top" — so this reuses
    /// it rather than inventing a fifth occupancy set for a property that has one.
    ///
    /// **A cell the base world already holds solid is left alone, and that is what
    /// keeps this from refusing correct campaigns.** If the box was floor before the
    /// write, then it is floor whether or not the write happens: both futures agree,
    /// there is nothing uncertain about standing there, and only a fill over a cell
    /// the world does NOT already floor can lend the path footing it might not have.
    /// So the rule binds precisely to laying NEW floor, which is the defect, and not
    /// to re-surfacing existing floor, which is decoration.
    fn with_unforced(&self, extra: &BTreeSet<[i32; 3]>) -> World {
        let mut w = World {
            piece: self.piece,
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            waterloggable: self.waterloggable.clone(),
            lava: self.lava.clone(),
            material: self.material.clone(),
            fingerprint: std::sync::OnceLock::new(),
            climb_holds: self.climb_holds.clone(),
            climb: self.climb.clone(),
            unheld_climb: self.unheld_climb.clone(),
            objective_cells: self.objective_cells.clone(),
            unbounded_fall: self.unbounded_fall,
            declared_jumps: self.declared_jumps.clone(),
            holder_stances: self.holder_stances.clone(),
            floors: self.floors.clone(),
            seating: self.seating.clone(),
            hiding: self.hiding.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            keep_out: self.keep_out.clone(),
            staged_lethal: self.staged_lethal.clone(),
            loop_slabs: self.loop_slabs.clone(),
            furniture: self.furniture.clone(),
            furniture_regions: self.furniture_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            base: self.base,
            built: self.built.clone(),
        };
        for c in extra {
            if w.solid.contains(c) {
                continue; // floor either way — no uncertainty to model
            }
            w.use_gates.remove(c);
            w.floors.leaves.remove(c);
            w.partial.remove(c);
            w.tall.insert(*c);
            // This proof's premise from here on, exactly as a seal or a flood is.
            w.pinned.insert(*c);
        }
        w.drop_unheld_climbs(extra);
        w.resieve()
    }

    /// This world as of one point in the quest DAG: every region a runtime write
    /// has filled forced solid, every region a runtime write has cleared emptied,
    /// every region a runtime write has filled with a fluid flooded
    /// ([`RegionState`]).
    ///
    /// **The one place** that knows what a runtime region write does to the
    /// geometry a proof reasons over. `close-gate`, `open-gate`, `fill-region`,
    /// `clear-region` and a shortcut's world-load seal all arrive here as
    /// [`crate::compiler::plan::RegionEvent`]s and none of them carries its own copy of the
    /// rule.
    pub(in crate::compiler::nav) fn with_region_state(&self, st: &RegionState) -> World {
        // Writes are applied last-to-strictest, so where two regions overlap the
        // more restrictive answer wins and the result needs no tie-break on
        // declaration order (ADR-0006). A fill beats a clear — a proof that
        // survives the seal is the conservative answer — an UNFORCED fill beats a
        // forced one, being a wall that additionally may not be stood on, and a
        // flood beats them all, because a flooded cell is everything a walled cell
        // is (impassable) and one thing more (not floor).
        // A holding slab walls like an unforced fill and is never floor
        // (spec-0086 §5.1), so it joins that set here and nowhere else.
        let walled: BTreeSet<[i32; 3]> = if st.held.is_empty() {
            st.unforced.clone()
        } else {
            st.unforced.union(&st.held).copied().collect()
        };
        self.with_cleared(&st.cleared)
            .with_sealed_by(&st.solid, &st.fill_footprints, &st.fill_hides)
            .with_unforced(&walled)
            .with_flooded(&st.flooded, &st.flood_regions)
            .with_staged_lethal(&st.lethal, &st.lethal_regions)
            .with_keep_out(&st.keep_out)
            .with_laid_hurting(&st.laid)
    }

    /// This world with what each laid write leaves in its cells joined to the
    /// blocks that hurt a body ([`crate::compiler::nav::Seating`]): a cell whose
    /// last write lays a block that hurts ([`delvewright_dsl::blockshape::hurts_body`]
    /// — a campfire a pressable's `prop` lays at load) hurts, and a cell whose
    /// last write lays anything else, air included, does not.
    fn with_laid_hurting(
        mut self,
        laid: &[crate::compiler::nav::route::region::LaidWrite],
    ) -> World {
        if laid.is_empty() {
            return self;
        }
        let mut last: BTreeMap<[i32; 3], bool> = BTreeMap::new();
        for ((lo, hi), block) in laid {
            let hurts = block
                .as_deref()
                .is_some_and(delvewright_dsl::blockshape::hurts_body);
            for c in crate::compiler::assembled::region_cells(*lo, *hi) {
                last.insert(c, hurts);
            }
        }
        let mut hurting = self.seating.hurting.clone();
        for (c, hurts) in last {
            if hurts {
                hurting.insert(c);
            } else {
                hurting.remove(&c);
            }
        }
        hurting.compact();
        self.seating.hurting = hurting;
        self
    }

    /// This world keeping out of `regions` ([`World::keep_out`]).
    fn with_keep_out(mut self, regions: &[LethalRegion]) -> World {
        self.keep_out.extend(regions.iter().cloned());
        // The reach graph's digest reads the keep-out ([`World::fingerprint`]).
        self.fingerprint = std::sync::OnceLock::new();
        self
    }

    /// This world with the staged lethal volumes a configuration may hold live
    /// (spec-0088) — the rule [`World::from_occupancy`] applies to a premise
    /// volume: the cells join `lethal` (impassable, never floor) and the boxes
    /// join `lethal_regions` (the walker's keep-out, and the name a refusal
    /// gives). A configuration that may hold none returns the world unchanged.
    fn with_staged_lethal(mut self, cells: &BTreeSet<[i32; 3]>, regions: &[LethalRegion]) -> World {
        if regions.is_empty() {
            return self;
        }
        self.lethal.extend(cells.iter().copied());
        self.lethal.compact();
        self.lethal_regions.extend(regions.iter().cloned());
        self.fingerprint = std::sync::OnceLock::new();
        self
    }

    /// Whether any cell of this world was flooded by a **runtime** fluid fill.
    /// Call sites skip the counterfactual clone entirely when it is false, which is
    /// every campaign that does not fill a region with water or lava.
    pub fn has_runtime_flood(&self) -> bool {
        !self.flood_written.is_empty()
    }

    /// A copy of this world in which every runtime fluid fill is treated as a
    /// **solid** fill instead — the counterfactual the fluid diagnostic is derived
    /// from, and precisely the model's behaviour before a written block's identity
    /// was consulted at all.
    ///
    /// A route that fails on the real world and succeeds on this one failed
    /// *because* the campaign filled a region with a fluid and something needed to
    /// stand there. Without the counterfactual the author is handed "no
    /// collision-free path" over a box they can see is full — the reachability
    /// report that sends someone to fix a prefab that was never wrong.
    pub(in crate::compiler::nav) fn without_runtime_flood(&self) -> World {
        let mut w = World {
            piece: self.piece,
            solid: self.solid.clone(),
            tall: self.tall.clone(),
            use_gates: self.use_gates.clone(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            waterloggable: self.waterloggable.clone(),
            lava: self.lava.clone(),
            material: self.material.clone(),
            fingerprint: std::sync::OnceLock::new(),
            climb_holds: self.climb_holds.clone(),
            climb: self.climb.clone(),
            unheld_climb: self.unheld_climb.clone(),
            objective_cells: self.objective_cells.clone(),
            unbounded_fall: self.unbounded_fall,
            declared_jumps: self.declared_jumps.clone(),
            holder_stances: self.holder_stances.clone(),
            floors: self.floors.clone(),
            seating: self.seating.clone(),
            hiding: self.hiding.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            keep_out: self.keep_out.clone(),
            staged_lethal: self.staged_lethal.clone(),
            loop_slabs: self.loop_slabs.clone(),
            furniture: self.furniture.clone(),
            furniture_regions: self.furniture_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            flood_written: CellSet::new(),
            flood_regions: Vec::new(),
            ambient: self.ambient.clone(),
            base: self.base,
            built: self.built.clone(),
        };
        for c in &self.flood_written {
            w.flooded.remove(&c);
            w.solid.insert(c);
        }
        w
    }

    /// The runtime fluid-fill boxes a walk over `cells` **depends on**, in region
    /// order — who to blame for a route that only exists when a fluid is mistaken
    /// for floor.
    ///
    /// A route cell is where the body IS; the cell below it is what holds the body
    /// up, and that is the one a fluid fill usually takes away — `fill … water` at
    /// y=64 leaves the walk at y=65 standing on nothing while the route polyline
    /// never enters the box at all. Blaming only the occupied cells reports "(none)"
    /// for the commonest case there is, which is how a correct diagnostic still
    /// tells the author nothing. Both are checked; a box that covers either is
    /// named.
    ///
    /// Deterministic: region order, no hashing (ADR-0006).
    pub(in crate::compiler::nav) fn flood_regions_over(
        &self,
        cells: &[[i32; 3]],
    ) -> Vec<([i32; 3], [i32; 3])> {
        let touched: Vec<[i32; 3]> = cells
            .iter()
            .flat_map(|c| [*c, [c[0], c[1] - 1, c[2]]])
            .collect();
        self.flood_regions
            .iter()
            .filter(|(lo, hi)| {
                touched
                    .iter()
                    .any(|c| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]))
            })
            .copied()
            .collect()
    }

    /// A copy of this world for **autonomous** walkers that cannot use gates —
    /// wave mobs seated at spawn. Opening a fence gate is a right-click
    /// USE, so for a mob acting on its own a closed gate is exactly a 1.5-tall
    /// fence: the use-gate cells are folded into the tall-barrier set, and the
    /// seating flood neither seats a mob in a gate threshold nor spills through
    /// one. Scripted walks (`move-npc` / `move-actor`) deliberately do NOT use
    /// this view — see [`plan_moves`], and [`crate::compiler::traversal`]'s `DW0452` for
    /// the proof that keeps that choice honest. A world with no use-gates is returned
    /// unchanged in content (call sites skip the clone via
    /// [`World::has_use_gates`]).
    pub fn without_gate_use(&self) -> World {
        let mut tall = self.tall.clone();
        tall.extend(self.use_gates.iter());
        // A walker that cannot use a gate meets each door and trapdoor as it
        // stands.
        let mut floors = self.floors.clone();
        floors.leaf_use = false;
        World {
            piece: self.piece,
            solid: self.solid.clone(),
            tall,
            use_gates: CellSet::new(),
            flooded: self.flooded.clone(),
            partial: self.partial.clone(),
            waterloggable: self.waterloggable.clone(),
            lava: self.lava.clone(),
            material: self.material.clone(),
            fingerprint: std::sync::OnceLock::new(),
            climb_holds: self.climb_holds.clone(),
            climb: self.climb.clone(),
            unheld_climb: self.unheld_climb.clone(),
            objective_cells: self.objective_cells.clone(),
            unbounded_fall: self.unbounded_fall,
            declared_jumps: self.declared_jumps.clone(),
            holder_stances: self.holder_stances.clone(),
            floors,
            seating: self.seating.clone(),
            hiding: self.hiding.clone(),
            lethal: self.lethal.clone(),
            lethal_regions: self.lethal_regions.clone(),
            keep_out: self.keep_out.clone(),
            staged_lethal: self.staged_lethal.clone(),
            loop_slabs: self.loop_slabs.clone(),
            furniture: self.furniture.clone(),
            furniture_regions: self.furniture_regions.clone(),
            pinned: self.pinned.clone(),
            world_load_seals: self.world_load_seals.clone(),
            clocked_gates: self.clocked_gates.clone(),
            flood_written: self.flood_written.clone(),
            flood_regions: self.flood_regions.clone(),
            ambient: self.ambient.clone(),
            base: self.base,
            built: self.built.clone(),
        }
    }

    /// Whether any closed fence-gate (use-gate) cell exists in this world.
    pub fn has_use_gates(&self) -> bool {
        !self.use_gates.is_empty()
    }

    /// Whether `c` is a closed fence-gate cell — a "use-gate": the player walks
    /// through it after an adventure-legal right-click. Exported per
    /// leg in the critical-path waypoint metadata so the harness knows the edge.
    pub fn is_use_gate(&self, c: [i32; 3]) -> bool {
        self.use_gates.contains(&c)
    }

    /// **One piece alone, as a body meets it**: the world of `blocks` (a cell
    /// to block-state map; air spellings are dropped) inside the box
    /// `[lo, hi)`, with nothing outside it — outside is air, with nothing to
    /// stand on. Water may spill into that air; no move of
    /// [`World::body_flood`] follows it out of the box, because a body that
    /// has left the box has left the piece. What every reader that asks
    /// "can a body reach this cell" of one piece before any campaign places it
    /// builds its reach graph over: the grammar's gates and edge proofs, the
    /// sculpt, the light probe and the enclosure survey.
    ///
    /// A piece is judged before a campaign states anything about the world it
    /// sits in, so there is no horizon, volume or gate to declare:
    /// [`Premises::geometry_only`], said by name.
    pub fn of_piece(blocks: BTreeMap<[i32; 3], String>, lo: [i32; 3], hi: [i32; 3]) -> Self {
        let blocks: BTreeMap<[i32; 3], String> = blocks
            .into_iter()
            .filter(|(_, name)| !delvewright_dsl::blockshape::is_air(name))
            .collect();
        let mut w = Self::from_occupancy(
            crate::compiler::assembled::occupancy_of(blocks, &BTreeSet::new()),
            Premises::geometry_only(),
        );
        w.piece = Some((lo, hi));
        w
    }

    /// **The cells that wall up `floor`**: each cell of it and the two over it
    /// that `open` says a body could be in. The one rule every confined walk
    /// over the reach graph uses to take floor out of a body's reach — the
    /// fixtures' `reaches_walled`, the edge proof's confinement to an edge's
    /// cells, `DW0838`'s flood without the commons. Leaving cells out of the
    /// set a walk may enter is not the same claim for a body that jumps: an arc
    /// that lands on kept floor flies through the left-out cells without
    /// standing in them. A wall stops it; the caller lays one block in every
    /// cell this returns and builds the world from those blocks.
    ///
    /// The wall never takes a kept standable cell or its headroom: a cell two
    /// over removed floor is open only when the cell under it is, so no kept
    /// floor stands on it.
    pub fn walls_over(
        floor: &BTreeSet<[i32; 3]>,
        open: &dyn Fn([i32; 3]) -> bool,
    ) -> BTreeSet<[i32; 3]> {
        let mut out = BTreeSet::new();
        for c in floor {
            for dy in 0..3 {
                let p = [c[0], c[1] + dy, c[2]];
                if open(p) {
                    out.insert(p);
                }
            }
        }
        out
    }

    /// Whether `c` lies inside this world's piece box — every cell of a world
    /// that is no one piece.
    pub fn in_piece(&self, c: [i32; 3]) -> bool {
        self.piece
            .is_none_or(|(lo, hi)| (0..3).all(|a| c[a] >= lo[a] && c[a] < hi[a]))
    }

    /// Build a [`World`] directly from a set of solid cells, with no water (test /
    /// synthetic entry point; the relight unit tests build a world without a full
    /// [`Plan`]).
    pub fn from_solid_cells(solid: BTreeSet<[i32; 3]>) -> Self {
        Self::from_solid_and_flooded(solid, BTreeSet::new())
    }

    /// Build a [`World`] directly from disjoint solid + flooded cell sets (test /
    /// synthetic entry point for the flood-aware standability rules).
    ///
    /// Synthetic: there is no campaign behind these cells, so there is nothing
    /// for them to state — [`Premises::geometry_only`], said by name.
    pub fn from_solid_and_flooded(solid: BTreeSet<[i32; 3]>, flooded: BTreeSet<[i32; 3]>) -> Self {
        let solid_hides = solid.clone();
        Self::from_occupancy(
            crate::compiler::assembled::Occupancy {
                solid,
                tall: BTreeSet::new(),
                use_gates: BTreeSet::new(),
                flooded,
                partial: BTreeMap::new(),
                waterloggable: BTreeSet::new(),
                lava: BTreeSet::new(),
                climb: BTreeMap::new(),
                unheld_climb: BTreeMap::new(),
                material: Default::default(),
                footprints: BTreeMap::new(),
                // A synthetic solid cell is a full cube of stone: it hides.
                hiding: solid_hides,
                leaves: BTreeMap::new(),
                hurting: Default::default(),
            },
            Premises::geometry_only(),
        )
    }
}

#[cfg(test)]
mod tests;
