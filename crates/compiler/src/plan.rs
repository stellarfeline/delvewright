//! Resolve a validated [`Campaign`] into a placement + naming model that
//! emission and `critical-path.json` both consume.
//!
//! ## Coordinate scheme (deterministic)
//!
//! Each stage-1 area is placed at origin `[index * AREA_SPACING, base_y, 0]`
//! (M1 has one area → `[0, 64, 0]`). The origin Y is fixed per **horizon**
//! (spec-0013): `void` → [`BASE_Y`] (64), `ocean` → [`OCEAN_BASE_Y`] (60), which is
//! `sea_level - island waterline` so authored island water meets the world ocean.
//! A prefab's local anchor position resolves to `origin + local`. All coordinates
//! are integers; no randomness is used in v0.
//!
//! ## Naming scheme (scoreboard/function-safe)
//!
//! DSL ids are type-prefixed kebab (`obj/talk`); scoreboard objectives, function
//! names and tags need `[a-z0-9_.-]`. Each id's local part (after its `/`) is
//! lowered to `_` for `-`, giving stable, collision-free names (DSL ids are
//! unique within their namespace):
//! `dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` (quest active), `dw.dlg_<npc>`,
//! tag `dw_npc_<npc>`, function `class_apply_<class>`, dialog `<npc>_<node>`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{
    Campaign, Diagnostic, DialogueEffect, DialogueId, EnvTrigger, Lethality, Npc, NpcDialogue,
    Objective, Quest, QuestEffect, Trap, TrapReset, TrapTrigger, Trigger,
};

use crate::flow::objectives_in_order;
use crate::registry::{AnchorMeta, PrefabRegistry};
use crate::solver::{self, Facing, Rotation, SealFill, Splitmix64};
use delvewright_dsl::DwCode;

/// World-space distance between successive area origins.
pub const AREA_SPACING: i32 = 256;
/// The Y of every area origin under `horizon: void` (structures carry their own
/// floor at local y=0). Also the fallback Y for an unresolvable position.
pub const BASE_Y: i32 = 64;
/// Sea level of the `ocean` horizon superflat (spec-0013): the pinned
/// bedrock/stone/water layer stack (1 + 118 + 8 from the -64 build floor) tops the
/// water at y=62. Emission pins the same stack in `generator-settings`.
pub const SEA_LEVEL: i32 = 62;
/// Height of the `ocean` horizon superflat's water layer (spec-0013) — the `8`
/// in the pinned `generator-settings` stack emission writes
/// (`emit::emit_server`). Ambient water occupies `SEA_LEVEL - 7 ..= SEA_LEVEL`.
pub const OCEAN_WATER_LAYERS: i32 = 8;
/// Y of the topmost ambient **solid** block of the `ocean` horizon superflat: the
/// sea floor (stone) directly under the water layers, at 54. The ambient model
/// boundary safety reasons about (`nav::Sea`) starts here — below it the world is
/// stone all the way to bedrock, which is why an ocean world has no void column
/// anywhere.
pub const SEA_FLOOR_TOP_Y: i32 = SEA_LEVEL - OCEAN_WATER_LAYERS;
/// The island tileset's authored waterline (`prefabs/island-tileset.md`): every
/// island piece puts its top water block at **local y=2**, with the walkable land
/// plane one block above it at local y=3.
///
/// Assumption (documented in `docs/reference/compiler.md`): the tileset convention
/// is a *library* constant, not a per-piece one — prefab metadata may *declare* its
/// waterline (`waterline_y`), which [`check_ocean_waterline`] then verifies against
/// sea level, but placement itself uses this single convention height so that every
/// area of an ocean world sits on one deterministic datum.
pub const ISLAND_WATERLINE_Y: i32 = 2;
/// The Y of every area origin under `horizon: ocean`: the piece base sits at
/// `SEA_LEVEL - ISLAND_WATERLINE_Y` (= 60) so the authored waterline (local y=2)
/// meets the world ocean (y=62) and the walk plane (local y=3) is the vanilla-normal
/// one block above the sea. Placing ocean areas at [`BASE_Y`] instead floats the
/// island ~4 blocks above the sea: a player who falls into open water cannot climb
/// ashore.
pub const OCEAN_BASE_Y: i32 = SEA_LEVEL - ISLAND_WATERLINE_Y;

/// The area-origin Y for a campaign's horizon (spec-0013). `void` (default/absent)
/// keeps [`BASE_Y`], so every pre-0.6 / void campaign stays byte-identical; `ocean`
/// uses [`OCEAN_BASE_Y`] so the island waterline convention holds.
pub fn base_y(campaign: &Campaign) -> i32 {
    match campaign.world.content.horizon {
        Some(delvewright_dsl::Horizon::Ocean) => OCEAN_BASE_Y,
        _ => BASE_Y,
    }
}

/// A resolved `set-checkpoint` effect (DSL v0.6, spec-0012), collected in
/// deterministic content order so its `index` is a stable, byte-identical id used
/// both for the active-checkpoint marker (`#cp dw.sys`) and its `on_respawn`
/// dispatch function.
#[derive(Clone, Debug)]
pub struct CheckpointPlan {
    /// Stable content-ordered id (0-based).
    pub index: usize,
    /// The checkpoint anchor name.
    pub anchor: String,
    /// The resolved absolute anchor cell.
    pub pos: [i32; 3],
    /// Per-player `on_respawn` effects (may be empty). For a bonfire
    /// (`rest == true`) this is the `on_rest` bundle: the same effects run on a
    /// rest and on a respawn (spec-0016 §1).
    pub on_respawn: Vec<QuestEffect>,
    /// `critical_path` step index at which this checkpoint fires (roots DW0315).
    /// For a bonfire this is the step that **arms** the rest affordance — the
    /// earliest beat at which a rest (and therefore a respawn here) is possible,
    /// so the no-stranding proof stays conservative.
    pub fire_step: usize,
    /// `true` for a `bonfire` (spec-0016 §1): the checkpoint moves only when the
    /// party rests at the affordance, not when the effect fires. `false` for a
    /// plain `set-checkpoint` (spec-0012), which is immediate.
    pub rest: bool,
    /// The bonfire rest dialog's three strings, already resolved against the
    /// compiler's canonical English. Meaningless for a
    /// plain `set-checkpoint`, which shows no dialog.
    pub prompt: String,
    /// The **rest and save** button label.
    pub rest_label: String,
    /// The **save only** button label.
    pub save_label: String,
}

/// A resolved stage-5 `shortcut` (spec-0016 §2), collected in deterministic
/// content order. A shortcut whose `gate` is not a resolvable gate region, or
/// whose `unlock` anchor does not resolve to a point, carries no plan entry (and
/// so no emission and no proof) — `DW0371` rejects those at validation.
#[derive(Clone, Debug)]
pub struct ShortcutPlan {
    /// The full shortcut id (`shortcut/<kebab>`).
    pub id: String,
    /// The function/tag-safe local id.
    pub safe: String,
    /// The gate anchor name.
    pub gate_anchor: String,
    /// The gate region's inclusive corners (absolute world coords).
    pub gate_region: ([i32; 3], [i32; 3]),
    /// The block the gate region is filled with (cleared to air on unlock).
    pub gate_block: String,
    /// The unlock anchor name.
    pub unlock_anchor: String,
    /// The resolved far-side unlock cell.
    pub unlock: [i32; 3],
    /// Effects fired once, when the shortcut opens.
    pub on_unlock: Vec<QuestEffect>,
    /// The volume a presser must stand in for the press to count as coming from
    /// the wrong side, derived from the gate slab and the `unlock` cell. `None`
    /// when the geometry does not decide it — `DW0425`.
    pub sealed_side: Option<crate::wrongside::SealedSide>,
}

impl ShortcutPlan {
    /// The **shell** cells of the sealed gate: every region cell with at least
    /// one axis-neighbour outside the region, in ascending `(x, y, z)` order —
    /// exactly the clickable surface, and for the thin slab a doorway usually is,
    /// the whole region.
    ///
    /// The same rule [`SealHintPlan::shell_cells`] applies to a `close-gate`
    /// seal, for the same reason: a cell buried inside the door has six sealed
    /// neighbours, so no face of it can ever be in a crosshair, and arming it
    /// would ship an entity nothing can reach.
    pub fn shell_cells(&self) -> Vec<[i32; 3]> {
        shell_cells_of(self.gate_region)
    }
}

/// A resolved stage-5 **lethal volume** (DSL v0.10, spec-0031): the box, the
/// wording, and the damage type the kill is dealt with.
///
/// Resolution is the shared [`Plan::zone_box`] — the same anchor-centred box a
/// `begin-stealth` zone and a `damage-players` `in` filter resolve through — so a
/// volume cannot drift into its own geometry rule. A volume whose anchor no placed
/// piece provides is simply absent from this list (validation reports it as
/// `DW0142`), never a blank box at the world origin.
pub struct LethalVolumePlan {
    /// The authored id (`lethal/<kebab>`).
    pub id: String,
    /// `safe_local(id)` — the segment that names the emitted function.
    pub safe: String,
    /// Inclusive world-space corners of the box.
    pub region: ([i32; 3], [i32; 3]),
    /// The (l10n-tagged) line the volume says as it kills.
    pub message: String,
    /// The damage type the kill is dealt with.
    pub damage_type: delvewright_dsl::DamageKind,
}

impl LethalVolumePlan {
    /// Whether `cell` lies inside this volume.
    pub fn contains(&self, cell: [i32; 3]) -> bool {
        let (lo, hi) = self.region;
        (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i])
    }
}

/// The compiler's own answer a sealed gate gives a right-click when the
/// `close-gate` authors no `sealed_hint`.
///
/// There is no such thing as a seal with nothing to say: a sealed boulder that
/// answers a right-click with SILENCE is a defect. The answer is therefore the
/// compiler's obligation, and the authored line is only the wording.
///
/// It is **chrome** (`dsl::chrome::GATE_SEALED`): compiler-owned, translated with
/// the compiler, and not l10n-inventoried — a campaign that wants its own wording
/// authors `sealed_hint`, which is inventoried like any other line. The plan
/// carries the chrome default in its tagged form; `emit` rebinds it to the build's
/// language.
pub const SEAL_HINT_DEFAULT: &str = delvewright_dsl::chrome::GATE_SEALED.en;

/// A gate anchor that some `close-gate` seals, and the line the seal answers a
/// right-click with (DSL v0.8). One entry per **anchor**: the seal is
/// a place, not an event, so two `close-gate`s on one anchor share its hitboxes
/// and must agree on the wording (`DW0423`).
#[derive(Clone, Debug)]
pub struct SealHintPlan {
    /// The gate anchor name (`anchor/boulder`).
    pub anchor: String,
    /// The function/tag-safe local id, used for `dw_seal_<safe>`.
    pub safe: String,
    /// The gate region's inclusive corners (absolute world coords).
    pub region: ([i32; 3], [i32; 3]),
    /// The block the region is filled with while sealed (the generated PackTest
    /// stages and un-stages the seal with it).
    pub block: String,
    /// The line the seal answers with — authored, or [`SEAL_HINT_DEFAULT`].
    pub text: String,
    /// Whether [`Self::text`] came from a `sealed_hint` the campaign wrote.
    /// `false` means it is the compiler's chrome fallback, which above
    /// `dsl_version` 0.11.0 the compiler is no longer allowed to supply
    /// (`DW0429`).
    pub authored: bool,
}

impl SealHintPlan {
    /// The **shell** cells of the seal: every region cell with at least one
    /// axis-neighbour outside the region, in ascending `(x, y, z)` order.
    ///
    /// A cell buried inside the region has six sealed neighbours, so no face of
    /// it can ever be in a player's crosshair — giving it a hitbox would ship an
    /// entity nothing can reach. The shell is exactly the clickable surface, and
    /// for the thin slab a gate anchor usually is (a doorway one block deep) it
    /// is the whole region.
    pub fn shell_cells(&self) -> Vec<[i32; 3]> {
        shell_cells_of(self.region)
    }
}

/// The **shell** cells of an inclusive region: every cell with at least one
/// axis-neighbour outside it, in ascending `(x, y, z)` order.
///
/// Extracted verbatim from [`SealHintPlan::shell_cells`] when the shortcut door's
/// own answer needed the identical surface. One definition, because
/// two copies of "which cells of a sealed slab can be clicked" would be free to
/// drift apart, and the whole point of the geometry is that it is the same
/// question in both places.
fn shell_cells_of(region: ([i32; 3], [i32; 3])) -> Vec<[i32; 3]> {
    let (a, b) = region;
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let interior = (lo[0] < x && x < hi[0])
                    && (lo[1] < y && y < hi[1])
                    && (lo[2] < z && z < hi[2]);
                if !interior {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

/// A resolved stage-5 `timed-gate` (spec-0016 §4), in declared order.
#[derive(Clone, Debug)]
pub struct TimedGatePlan {
    /// The full timed-gate id.
    pub id: String,
    /// The function/tag-safe local id.
    pub safe: String,
    /// The gate anchor name.
    pub gate_anchor: String,
    /// The gate region's inclusive corners (absolute world coords).
    pub gate_region: ([i32; 3], [i32; 3]),
    /// The block the region is filled with while closed.
    pub gate_block: String,
    /// Ticks open per cycle.
    pub open_ticks: u32,
    /// Ticks closed per cycle.
    pub closed_ticks: u32,
    /// Ticks after world init before the first open window.
    pub phase: u32,
    /// Whether the closing edge kills players caught inside the region
    /// (spec-0016 §4 addendum).
    pub crush: bool,
    /// The resolved disarm affordance, if declared. A gate whose
    /// `disarm.via` anchor does not resolve carries `None` — the DSL tier's
    /// `DW0377` reports that, and no half-built affordance reaches emission.
    pub disarm: Option<TimedGateDisarmPlan>,
}

/// A resolved `timed-gate` disarm affordance — the same shape a
/// trap's [`TrapDisarmPlan`] takes.
#[derive(Clone, Debug)]
pub struct TimedGateDisarmPlan {
    /// The anchor name the player interacts with.
    pub via_anchor: String,
    /// Its resolved absolute cell.
    pub via_cell: [i32; 3],
    /// The flag jamming the gate sets, party-wide.
    pub sets_flag: String,
}

/// A resolved stage-5 `ambush` (spec-0016 §3), collected in declared order —
/// the trigger cell and the cell each ambusher will stand on. An ambush whose
/// anchors do not resolve carries no entry (and so no proof); the desugared
/// trigger's own anchor checks report that.
#[derive(Clone, Debug)]
pub struct AmbushPlan {
    /// The full ambush id (`ambush/<kebab>`).
    pub id: String,
    /// The resolved trigger cell — where the player is standing when it springs.
    pub at: [i32; 3],
    /// One resolved spawn cell per ambusher, in declared order.
    pub actor_cells: Vec<[i32; 3]>,
}

/// A resolved `begin-stealth` beat (DSL v0.6, spec-0014), collected in
/// deterministic content order; its `index` (1-based) is the active-session id
/// written to `#stealth dw.sys` (0 = inactive).
#[derive(Clone, Debug)]
pub struct StealthBeat {
    /// Stable content-ordered session id (1-based).
    pub index: usize,
    /// Zones: `(anchor name, resolved centre cell, half-extents)`.
    pub zones: Vec<(String, [i32; 3], [u32; 3])>,
    /// Per-player `on_caught` effects (may be empty).
    pub on_caught: Vec<QuestEffect>,
    /// Ticks of exposure tolerated before `on_caught` fires.
    pub grace_ticks: u32,
    /// `critical_path` step index that activates the beat (roots DW0327).
    pub fire_step: usize,
    /// `critical_path` step index at which the beat stops judging players — the
    /// firing step of the first `end-stealth` after `fire_step`, or of the next
    /// `begin-stealth` (a new session replaces the running one), whichever comes
    /// first. `None` = the beat is never closed and runs to the end of the
    /// campaign. Roots the DW0355 onset proof's respawn-position set: a
    /// checkpoint reigning anywhere in `[fire_step, end_step]` can drop a player
    /// into this beat.
    pub end_step: Option<usize>,
}

impl StealthBeat {
    /// Whether being caught in this beat actually **punishes** the player — the
    /// `on_caught` tree contains a `damage-players` (direct harm) or a
    /// `spawn-wave` (hostile mobs). A beat that only narrates has nothing to
    /// escape from, so the DW0355 onset-survivability obligation does not apply
    /// to it; a punishing beat must be escapable from every position a player can
    /// legally occupy when it starts.
    pub fn is_punishing(&self) -> bool {
        fn punishing(eff: &QuestEffect) -> bool {
            if matches!(
                eff,
                QuestEffect::DamagePlayers { .. } | QuestEffect::SpawnWave { .. }
            ) {
                return true;
            }
            eff.nested_effect_lists()
                .into_iter()
                .flatten()
                .any(punishing)
        }
        self.on_caught.iter().any(punishing)
    }
}

/// A resolved trap (DSL v0.6, spec-0011), collected in deterministic content
/// order. Carries everything the nav proof (`DW0342`), the payload/disarm
/// emission, and the PackTest need.
#[derive(Clone, Debug)]
pub struct TrapPlan {
    /// The raw trap id (`trap/<name>`).
    pub id: String,
    /// Sanitized local name (`dart_hall`) for emitted function/tag names.
    pub safe: String,
    /// The declared trigger kind (informs the hazard model + PackTest).
    pub trigger: TrapTrigger,
    /// The `anchor/trap` marker this trap sits on — the key into prefab metadata
    /// for its hardware declarations (`dispenser`, `trigger_block`).
    pub at_anchor: String,
    /// The resolved absolute trigger/hazard cell (the trap's `at` anchor cell).
    pub trigger_cell: [i32; 3],
    /// The resolved absolute dispenser socket cell (from the `at` anchor's
    /// `dispenser` metadata), or `None` if the prefab exposes none.
    pub dispenser: Option<[i32; 3]>,
    /// The dispense payload `(item, count)` this trap loads, if any.
    pub payload: Option<(String, u32)>,
    /// The spec-0022 **command payload**: the ordered effect bundle the trigger
    /// fires. Empty for a pure spec-0011 redstone trap, which is what keeps such
    /// a campaign's output byte-identical.
    pub payload_effects: Vec<QuestEffect>,
    /// How dangerous the trap is.
    pub lethality: Lethality,
    /// Whether the trap re-arms after firing.
    pub reset: TrapReset,
    /// The resolved disarm affordance, if declared.
    pub disarm: Option<TrapDisarmPlan>,
    /// Flags that gate the trap being active.
    pub requires_flags: Vec<String>,
    /// Flags whose being set deactivates the trap (DSL v0.6 negative gate).
    pub forbids_flags: Vec<String>,
    /// Numeric gate terms (DSL v0.10, spec-0031): the trap is armed only while
    /// every comparison holds.
    pub requires_state: Vec<delvewright_dsl::StateCompare>,
}

/// **One runtime region write**, collected in deterministic content order — the
/// single completability model of "a box the delve fills or clears while it is
/// running", whichever verb spelled it.
///
/// Five verbs produce these and none of them owns the rule: `close-gate` fills a
/// prefab gate anchor's region with the block that anchor declares and `open-gate`
/// clears it (DSL v0.6); `fill-region` and `clear-region` do the same to an
/// author-declared box (DSL v0.10, spec-0031); `open-way` does it to the cells a
/// placed piece's spatial contract exports, in the direction that contract
/// declares (DSL v0.12, spec-0042); a `shortcut`'s gate is registered filled from
/// world-load. The occupancy model (`crate::assembled`) treats every
/// *gate* cell as always passable — the conservative "assume the gate the player
/// needs is opened" stance `DW0306` checks — and a `fill` is the physical dual:
/// the critical-path / checkpoint reachability proofs treat the region as
/// **solid** on any walked leg reached *after* the latest write at or before it is
/// a fill, so a path that must cross it fails `DW0311`/`DW0315`. A `clear` is the
/// other direction, and is credited the same way: the region is **passable** from
/// the DAG point at which it fires (`nav::World::with_cleared`).
///
/// The type is named for the object it acts on — a region — and not for the verb
/// that first needed it (CLAUDE.md): the third consumer inherits this proof
/// instead of re-deriving it, which is exactly what `open-gate`/`close-gate`
/// having owned it privately prevented.
/// What a runtime region write leaves in the region — read straight off the
/// command the verb emits, because that is the only thing the model may conclude.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionWrite {
    /// Every cell becomes solid: `close-gate` (the gate anchor's declared block),
    /// `fill-region` (the author's block), a `shortcut`'s world-load seal.
    ///
    /// "Solid" is a claim about the **block**, not about the write. Only a write
    /// whose block is a full collision cube leaves floor behind, so the block is
    /// classified once, by [`RegionWrite::of_block`], and a fluid lands in
    /// [`RegionWrite::Flood`] instead.
    Fill,
    /// Every cell becomes **free fluid**: a `fill-region` / `close-gate` /
    /// `shortcut` seal whose block is water or lava
    /// ([`crate::assembled::is_fluid`]).
    ///
    /// A separate case from [`RegionWrite::Fill`] because the two conclusions are
    /// opposite where it matters. A fill of stone is impassable **and** floor; a
    /// fill of water is impassable and **never** floor. Collapsing them says a body
    /// stands on a water surface, and the nav model's `flooded` set — impassable,
    /// never standable — is precisely the set that already says otherwise, so this
    /// is a classification the model was missing, not a capability.
    ///
    /// **What it does not model**: the fluid's spread beyond the written region.
    /// Vanilla flows a source outward at world-tick; this marks the written cells
    /// and no more, so the model can under-mark the wet set exactly as
    /// [`crate::nav::World::with_cleared`] documents for a clear that opens a dry
    /// region into adjacent water. Both are the same missing input — a runtime
    /// block map to re-derive the flood from — and both are stated in
    /// `docs/reference/compiler.md` rather than left to be discovered.
    Flood,
    /// Every cell becomes empty: `clear-region`, whose emitted
    /// `fill … minecraft:air` carries no `replace` filter and so removes whatever
    /// is there.
    Clear,
    /// **Only the gate's own block** becomes empty: `open-gate`, whose emitted fill
    /// is `replace`-filtered to the block the gate anchor declares.
    ///
    /// A third case rather than a synonym for [`RegionWrite::Clear`], because the
    /// emitted commands differ and so does what may be concluded from them. The
    /// assembled world already holds every gate cell empty, so an unseal removes
    /// nothing the model believed was there — an unfiltered clear does. Collapsing
    /// the two says an `open-gate` deletes a `collapse`'s debris resting in the
    /// doorway, which in game it plainly does not (`DW0445`, measured:
    /// `v06_trap_payloads::collapse_that_buries_the_critical_path_is_dw0445` goes
    /// green — i.e. stops proving anything — the moment they are collapsed). An
    /// unseal still takes part in latest-write-wins, which is how a later
    /// `open-gate` cancels an earlier `close-gate`.
    Unseal,
}

impl RegionWrite {
    /// **The one place a block id becomes a region write's conclusion.** Every
    /// site that turns "this verb fills that box with that block" into a model
    /// update goes through here, so no two of them can disagree about what a
    /// fluid leaves behind.
    ///
    /// It reads [`crate::assembled::is_fluid`] — the same predicate the static
    /// occupancy model uses — because the question "what does this block do to a
    /// walker" belongs to the block, not to the verb that wrote it. A waterlogged
    /// block is deliberately a [`RegionWrite::Fill`]: its cell is occupied by the
    /// host block and is genuine floor (see `is_fluid`'s note).
    pub fn of_block(block: &str) -> RegionWrite {
        if crate::assembled::is_fluid(block) {
            RegionWrite::Flood
        } else {
            RegionWrite::Fill
        }
    }

    /// Whether this write **overwrites** the region with a block, rather than
    /// emptying it — true for both [`RegionWrite::Fill`] and
    /// [`RegionWrite::Flood`], because a `fill … minecraft:water` destroys
    /// whatever was in the box exactly as a `fill … minecraft:stone` does. It says
    /// nothing about whether the result is standable; that is the variant's job.
    pub fn fills(&self) -> bool {
        matches!(self, RegionWrite::Fill | RegionWrite::Flood)
    }
}

/// One resolved region write: the inclusive world box, and what the write leaves
/// in it. A verb resolves to a LIST of these, because a way is a region of as
/// many boxes as its contract gave it and each is written by its own `fill`.
type ResolvedWrite = (([i32; 3], [i32; 3]), RegionWrite);

#[derive(Clone, Debug)]
pub struct RegionEvent {
    /// The region's inclusive corners (absolute world coords).
    pub region: ([i32; 3], [i32; 3]),
    /// What this write leaves in the region.
    pub write: RegionWrite,
    /// The `critical_path` step index at which this firing happens.
    pub fire_step: usize,
    /// **Whether the party is guaranteed to cause this firing**, computed from the
    /// quest graph and the effect's root — never asserted by an author, because the
    /// DSL has no surface on which to assert it (see [`collect_region_events`]).
    ///
    /// Private, with [`RegionEvent::forced`] / [`RegionEvent::unforced`] as the only
    /// ways in, so a `RegionEvent` **cannot be built without answering this
    /// question**. It is the same move [`RegionWrite::of_block`] makes for the block:
    /// the model's premises are constructed, not defaulted.
    forced: bool,
    /// The beat this firing hangs off, in words, for a diagnostic to name. Empty for
    /// a forced write, which never needs blaming.
    blame: String,
}

impl RegionEvent {
    /// A write the party **cannot avoid causing**: a quest bundle they must complete,
    /// an environment trigger, or a wall the placed world is born holding.
    pub fn forced(region: ([i32; 3], [i32; 3]), write: RegionWrite, fire_step: usize) -> Self {
        RegionEvent {
            region,
            write,
            fire_step,
            forced: true,
            blame: String::new(),
        }
    }

    /// A write that **may never happen**: a sprung trap, a bought offer, a death, a
    /// shortcut taken from the far side. `blame` names the beat in words.
    pub fn unforced(
        region: ([i32; 3], [i32; 3]),
        write: RegionWrite,
        fire_step: usize,
        blame: impl Into<String>,
    ) -> Self {
        RegionEvent {
            region,
            write,
            fire_step,
            forced: false,
            blame: blame.into(),
        }
    }

    /// Whether this write overwrites the region with a block
    /// ([`RegionWrite::fills`]).
    pub fn fills(&self) -> bool {
        self.write.fills()
    }

    /// Whether the party is guaranteed to cause this firing.
    pub fn is_forced(&self) -> bool {
        self.forced
    }

    /// The beat this firing hangs off, in words; empty when it is forced.
    pub fn blame(&self) -> &str {
        &self.blame
    }
}

/// A resolved trap disarm affordance (DSL v0.6, spec-0011).
#[derive(Clone, Debug)]
pub struct TrapDisarmPlan {
    /// The disarm anchor name (`anchor/…`).
    pub via_anchor: String,
    /// The resolved absolute cell of the disarm affordance.
    pub via_cell: [i32; 3],
    /// The flag the disarm sets.
    pub sets_flag: String,
}

/// The compiled model.
pub struct Plan<'a> {
    /// The source campaign.
    pub campaign: &'a Campaign,
    /// Datapack namespace = campaign id.
    pub namespace: String,
    /// Stage-1 seed (level seed / future PRNG source).
    pub seed: u64,
    /// Area placements, in stage-1 order.
    pub areas: Vec<AreaPlacement>,
    /// Advisory findings the placement stage raised, in area order. Currently
    /// `DW0498` ([`crate::pool`]): a pool draw that seats the same anchor-bearing
    /// prefab twice, so every anchor that prefab declares has more than one
    /// carrier. Reported by [`crate::emit::build_with_warnings`], which prepends
    /// them to the build's own advisories; never fatal.
    pub warnings: Vec<Diagnostic>,
    /// Resolved absolute anchors, keyed by `(area_id, anchor_name)`.
    pub anchors: BTreeMap<(String, String), ResolvedAnchor>,
    /// Class selection plan (n starts at 1).
    pub classes: Vec<ClassPlan>,
    /// Per-NPC dialogue plan.
    pub npcs: Vec<NpcPlan>,
    /// The bot critical path.
    pub critical_path: Vec<Step>,
    /// Inter-area transport: objective id → absolute teleport target. When
    /// completing an objective moves the player into a different area on the
    /// critical path, the compiler teleports them to that area's entry spawn
    /// (areas sit `AREA_SPACING` apart across void; the pathfinder-free bot cannot
    /// walk between them). Emitted in that objective's completion function.
    pub transport: BTreeMap<String, [i32; 3]>,
    /// Per-step transport marker, aligned 1:1 with `critical_path`: `Some(dest)` if
    /// completing that step's objective teleports the player to `dest` (a different
    /// area), else `None`. Emitted into `critical-path.json` as the step's
    /// `transport` field so the harness can wait for the position discontinuity
    /// before starting the next step (gap 8). `None` for `select-class` /
    /// `assert-complete` and any step that does not change area.
    pub critical_path_transport: Vec<Option<[i32; 3]>>,
    /// Resolved lethal volumes (DSL v0.10, spec-0031), declaration-ordered. Empty
    /// for every campaign that declares none — which is what keeps the navigation
    /// world, the emitted tick and the build outputs byte-identical.
    pub lethal_volumes: Vec<LethalVolumePlan>,
    /// Per-step stealth hint (DSL v0.4), aligned 1:1 with `critical_path`: `true`
    /// when the step's objective is `stealth`-marked → emitted as `sneak: true`.
    pub critical_path_sneak: Vec<bool>,
    /// Per-step cutscene duration (DSL v0.4), aligned 1:1 with `critical_path`:
    /// `Some(seconds)` when completing that step's objective triggers a
    /// `QuestEffect::Cutscene` → emitted as `cutscene_seconds`.
    pub critical_path_cutscene: Vec<Option<u32>>,
    /// Resolved `set-checkpoint` effects (DSL v0.6, spec-0012), content-ordered.
    pub checkpoints: Vec<CheckpointPlan>,
    /// Resolved `begin-stealth` beats (DSL v0.6, spec-0014), content-ordered.
    pub stealth_beats: Vec<StealthBeat>,
    /// Objective id → its `critical_path` step index. The inverse of a step's
    /// serving objective — used by the visual-tier POV shot planner
    /// (`crate::render_plan`) to name the objective each player-POV leg walks
    /// toward, and by the v0.6 checkpoint / stealth proofs to root a beat.
    pub objective_steps: BTreeMap<String, usize>,
    /// Resolved traps (DSL v0.6, spec-0011), content-ordered.
    pub traps: Vec<TrapPlan>,
    /// Resolved shortcut doors (spec-0016 §2), content-ordered.
    pub shortcuts: Vec<ShortcutPlan>,
    /// Resolved container fills (spec-0021), declaration-ordered.
    pub loot: Vec<LootPlan>,
    /// Resolved `collect` container adoptions (DSL v0.8), campaign-
    /// ordered. Empty for a campaign whose collects keep the compiler's chest.
    pub collect_fills: Vec<CollectFillPlan>,
    /// Resolved ambushes (spec-0016 §3), declaration-ordered.
    pub ambushes: Vec<AmbushPlan>,
    /// Resolved timed gates (spec-0016 §4), declaration-ordered.
    pub timed_gates: Vec<TimedGatePlan>,
    /// One entry per gate anchor some `close-gate` seals (DSL v0.8),
    /// in first-firing order — the seal the party can press for an answer. Empty
    /// for a campaign that never seals a gate.
    pub seal_hints: Vec<SealHintPlan>,
    /// **What every compiler-owned sealed body answers a press with** (DSL v0.11).
    /// One entry per pressable body the campaign does not answer itself — a
    /// `close-gate` seal, a sealed `shortcut` door — in that order. Empty for a
    /// campaign with neither.
    pub press_answers: Vec<PressAnswer>,
    /// Resolved gate open/close firings (DSL v0.6), content-ordered — drives the
    /// `close-gate` completability model in `crate::nav`. Empty when the campaign
    /// uses no gate effects (byte-identical routing to pre-close-gate behavior).
    pub region_events: Vec<RegionEvent>,
    /// **Every contingent way the placed world stages** (spec-0042 §2.4), in
    /// area → placement → declaration order, with its world cells, its block and
    /// its direction read from the carrying piece's metadata. Empty for every
    /// world whose pieces declare none — which is every world built before this
    /// surface — so nothing about such a build moves.
    ///
    /// It is on the plan rather than recomputed per consumer because three
    /// readers need the same answer: emission (what an `open-way` fills),
    /// the completability model ([`collect_region_events`]) and the disposition
    /// gate (`crate::ways`). Two of the three deriving it independently is how a
    /// verb and its proof come to disagree about what a way is.
    pub ways: crate::ways::WayStaging,
    /// The way gate's binding ledger (`crate::ways`, spec-0042 AC11) — what the
    /// disposition enumeration examined and what it found. `None` for a world
    /// that stages no way, which emits no artifact at all: a file reading zero is
    /// a finding, and an absent file is the honest statement that there was
    /// nothing to enumerate.
    pub way_gate: Option<crate::ways::WayGate>,
    /// **Where the party can be CARRIED rather than walk**: every declared
    /// `teleport`'s resolved source volume (DSL v0.10, spec-0031), content-ordered.
    /// Empty for every campaign that declares no `teleport`, which is what keeps
    /// those campaigns' routing byte-identical.
    ///
    /// The completability model reads it for one purpose: a walked leg whose
    /// *start* lies inside one of these boxes is a leg the party may never walk, so
    /// a world-load gate seal is not applied to it and
    /// [`crate::nav::DW_GATE_NEVER_OPENED`] declines to judge it. It deliberately
    /// carries **no firing step**: the suppression must hold for a branch path too,
    /// whose step indices are its own, and here the conservative direction is *not
    /// to fire* — refusing a campaign over a door the party is teleported past is
    /// the false positive this model must not have. The class left unproven as a
    /// result is named in `docs/reference/compiler.md`.
    pub transit_teleports: Vec<([i32; 3], [i32; 3])>,
    /// Per-batch affected world AABBs from the stage-7 L2 massing verbs
    /// (spec-0017), keyed by batch id — the editor's per-batch snapshot
    /// framing for massing batches. Empty for a campaign without massing.
    pub massing_bounds: BTreeMap<String, ([i32; 3], [i32; 3])>,
    /// For each objective's `critical_path` step, the set of steps of its **strict
    /// DAG ancestors** — objectives guaranteed to complete before it in *every* valid
    /// play order (transitive `after` within its quest ∪ every objective of a
    /// transitive `depends_on`-ancestor quest). The `close-gate` seal model
    /// (`crate::nav`) uses this so a gate only seals a leg whose objective is a true
    /// causal descendant of the gate's firing objective — not a parallel branch the
    /// lineariser merely interleaved ahead of it.
    pub strict_ancestor_steps: BTreeMap<usize, BTreeSet<usize>>,
}

/// A placed area: one or more pieces plus their socket seals.
pub struct AreaPlacement {
    /// Area id (`area/…`).
    pub area_id: String,
    /// The placed pieces (single-prefab areas have exactly one; pool areas have
    /// the solver's assembly, entry first).
    pub pieces: Vec<PiecePlacement>,
    /// Socket seal/clear fills for this area — one per connector of every placed
    /// piece, whatever assembled them. Empty only when no placed piece declares a
    /// connector; a single-prefab area's lone piece has all of its unmated, so
    /// each one is walled.
    pub seals: Vec<SealFill>,
}

impl AreaPlacement {
    /// The union world AABB `(min, max)` covering every placed piece. For a
    /// single-prefab area this is exactly `origin .. origin+size-1`.
    pub fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for piece in &self.pieces {
            let (pmin, pmax) = piece.bbox();
            for a in 0..3 {
                min[a] = min[a].min(pmin[a]);
                max[a] = max[a].max(pmax[a]);
            }
        }
        (min, max)
    }
}

/// One placed structure piece.
pub struct PiecePlacement {
    /// Bound prefab id (`prefab/…`).
    pub prefab_id: String,
    /// The structure templates this piece's blocks arrive in, each already
    /// placed in world space — one for a single-template prefab, one per tile
    /// for a zone past the vanilla 48-per-axis cap.
    ///
    /// **A piece is one piece however many files it ships as.** Tiling is a
    /// packaging fact about a file format, so it is absorbed here, at the one
    /// place a `.nbt` filename is reachable from: everything above this — the
    /// area's anchors, its seals, the face-contract mating check, the pool
    /// draw, massing — sees the piece the author bound, at its size, with its
    /// rotation. Everything below it emits one `/place template` per entry and
    /// never asks how many there were.
    pub templates: Vec<PlacedTemplate>,
    /// World-space `/place template` position `[x, y, z]` of the PIECE (where
    /// piece-local `(0,0,0)` lands). Each template's own position is derived
    /// from it and is on [`PlacedTemplate::pos`].
    pub pos: [i32; 3],
    /// Unrotated prefab size `[sx, sy, sz]` — the WHOLE piece, from prefab
    /// metadata, never one tile's extent.
    pub size: [i32; 3],
    /// Placement rotation (identity for single-prefab areas).
    pub rotation: Rotation,
}

/// One structure template of a placed piece, in world space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedTemplate {
    /// Datapack structure id path segment (e.g. `hello-room`, or
    /// `z0-barrow-shore.x0y0z1` for a tile).
    pub structure_id: String,
    /// Structure `.nbt` filename (relative to `prefabs/`).
    pub structure_file: String,
    /// World-space `/place template` position `[x, y, z]` — where THIS
    /// template's local `(0,0,0)` lands, already carrying the piece rotation
    /// applied to the template's piece-local offset. Equal to the piece's own
    /// `pos` for a single-template prefab.
    pub pos: [i32; 3],
    /// The template's extent `[x, y, z]`, unrotated.
    pub size: [i32; 3],
}

impl PiecePlacement {
    /// The world AABB `(min, max)` of this placed piece, for chunk `forceload`.
    pub fn bbox(&self) -> ([i32; 3], [i32; 3]) {
        self.rotation.bbox(self.pos, self.size)
    }
}

/// The templates a piece's metadata declares, placed in world space at `pos`
/// under `rotation`.
///
/// Vanilla `/place template … <rotation>` rotates about the placement position,
/// so a tile at piece-local `offset` lands at `pos + rotation(offset)` and its
/// own cells then rotate about that — which composes to exactly
/// `pos + rotation(offset + local)`, the whole zone rotated about the piece
/// origin. That identity is what lets a tiled zone be rotated at all, and it is
/// the same arithmetic [`Rotation::bbox`] already uses.
fn placed_templates(
    meta: &delvewright_dsl::prefab::PrefabMeta,
    pos: [i32; 3],
    rotation: Rotation,
) -> Vec<PlacedTemplate> {
    meta.templates()
        .into_iter()
        .map(|t| {
            let o = rotation.transform(t.offset);
            PlacedTemplate {
                structure_id: t.id.to_string(),
                structure_file: t.file.to_string(),
                pos: [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]],
                size: t.size,
            }
        })
        .collect()
}

/// A resolved anchor (absolute world coords).
pub enum ResolvedAnchor {
    /// A point with optional facing.
    Point {
        /// Absolute position.
        pos: [i32; 3],
        /// Facing keyword, if any.
        facing: Option<String>,
    },
    /// A gate region of `block`.
    Gate {
        /// Absolute min/max corners.
        from: [i32; 3],
        /// The opposite corner.
        to: [i32; 3],
        /// Filling block id.
        block: String,
    },
}

/// One selectable class.
pub struct ClassPlan {
    /// Trigger value (`/trigger dw.class set <n>`), 1-based.
    pub n: i32,
    /// Class id.
    pub class_id: String,
    /// Sanitized name for the apply function.
    pub safe: String,
}

/// A dialogue plan for one NPC.
pub struct NpcPlan {
    /// NPC id.
    pub npc_id: String,
    /// Sanitized local name (`keeper`).
    pub safe: String,
    /// The trigger objective (`dw.dlg_<npc>`).
    pub trigger_objective: String,
    /// Entity tag on the interaction entity (`dw_npc_<npc>`).
    pub tag: String,
    /// Root dialogue node id.
    pub root: String,
    /// Options in a stable order, each with its trigger value.
    pub options: Vec<OptionPlan>,
}

/// One dialogue option and the `/trigger` value it fires.
pub struct OptionPlan {
    /// Trigger value (`/trigger dw.dlg_<npc> set <n>`), 1-based across the NPC.
    pub n: i32,
    /// The node this option belongs to.
    pub node_id: String,
    /// Button label.
    pub label: String,
    /// The button's hover tooltip (DSL v0.8) — the full line the label captions.
    /// `None` emits no `tooltip` key at all, so a campaign that authors none is
    /// byte-identical to a pre-0.8 build.
    pub tooltip: Option<String>,
    /// Navigation target node, if any.
    pub next: Option<String>,
    /// Objectives this option completes.
    pub completes: Vec<String>,
    /// Flags this option sets when chosen (DSL v0.4 dialogue `set-flag`).
    pub sets_flags: Vec<String>,
    /// Flags that must be set for this option to be shown (DSL v0.4).
    pub requires_flags: Vec<String>,
    /// Flags whose being set HIDES this option (DSL v0.6 negative gate).
    pub forbids_flags: Vec<String>,
    /// Numeric gate terms (DSL v0.10, spec-0031): the option is shown only while
    /// every comparison holds.
    pub requires_state: Vec<delvewright_dsl::StateCompare>,
    /// World-time cuts this option fires (DSL v0.5 dialogue `set-time`), in order.
    pub sets_time: Vec<delvewright_dsl::WorldTime>,
    /// Weather cuts this option fires (DSL v0.5 dialogue `set-weather`), in order.
    pub sets_weather: Vec<delvewright_dsl::WorldWeather>,
    /// Checkpoints this option sets (DSL v0.6 dialogue `set-checkpoint`), each
    /// `(anchor, on_respawn)`, in order.
    pub sets_checkpoints: Vec<(String, Vec<QuestEffect>)>,
    /// Deferred NPCs this option summons (DSL v0.6 dialogue `spawn-npc`), in order.
    pub spawns_npcs: Vec<String>,
}

/// A critical-path step (mirrors the amended `critical-path.json` shape).
///
/// Every step that stands for a DSL objective carries that objective's id
/// (`objective_id`, exported as the step's `objective` field). It is the step's
/// **proof obligation**: the harness passes the step only when the anchored
/// completion marker for exactly this objective arrives ([`marker_line`]). Without
/// it a step could only be checked positionally — arriving somewhere is not
/// completing anything — which is how a run once passed 22/22 on a path whose
/// campaign had in fact completed at step 12.
pub enum Step {
    /// Select a class by chatting `command`.
    SelectClass {
        /// Class id.
        class_id: String,
        /// The chat command the bot sends.
        command: String,
    },
    /// Talk to an NPC; `command` fires the objective-completing option.
    TalkTo {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// NPC id.
        npc_id: String,
        /// Absolute NPC position.
        pos: [i32; 3],
        /// The chat command the bot sends.
        command: String,
    },
    /// Walk to within `radius` of `pos`.
    Reach {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Anchor id.
        anchor_id: String,
        /// Absolute anchor position.
        pos: [i32; 3],
        /// Completion radius.
        radius: u32,
    },
    /// Slay a wave: goto `pos` (the wave anchor), attack entities tagged `tag`
    /// until the marker channel reports completion (v0.3).
    Kill {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Wave id (`wave/…`).
        wave_id: String,
        /// Absolute wave-anchor position.
        pos: [i32; 3],
        /// Entity tag on the wave's mobs (`dw_wave_<wave>`).
        tag: String,
        /// Total mob count.
        count: i32,
    },
    /// Collect `count` of `item` from a chest at `pos` (v0.3) — or, when
    /// `dropped` is set, off the ground where that wave died (DSL v0.9).
    Collect {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Vanilla item id.
        item: String,
        /// Required count.
        count: i32,
        /// Absolute chest-anchor position — or, for a dropped collect, the wave
        /// anchor whose floor the item lands on.
        pos: [i32; 3],
        /// The wave whose declared drop provides the item (DSL v0.9), when the
        /// objective is drop-gated. There is no container at `pos`: the harness
        /// walks the fight's ground and waits for the pickup instead of opening
        /// a block that is not there.
        dropped: Option<String>,
    },
    /// Interact at `pos`: goto, then chat `command` (the same `/trigger` the
    /// interaction advancement fires). `requires_item` gates completion (v0.3).
    Interact {
        /// The `obj/<id>` this step proves complete.
        objective_id: String,
        /// Interact anchor id.
        anchor_id: String,
        /// Absolute interact-anchor position.
        pos: [i32; 3],
        /// The chat command the bot sends.
        command: String,
        /// Item required in inventory, if any.
        requires_item: Option<String>,
    },
    /// Assert a scoreboard objective value.
    AssertComplete {
        /// The objective (`dw.campaign`).
        objective: String,
        /// Expected value.
        value: i32,
    },
}

impl Step {
    /// The `obj/<id>` this step proves, when it stands for a DSL objective.
    ///
    /// `None` for the two path-frame steps (`select-class`, `assert-complete`),
    /// which prove no objective of their own.
    pub fn objective(&self) -> Option<&str> {
        match self {
            Step::TalkTo { objective_id, .. }
            | Step::Reach { objective_id, .. }
            | Step::Kill { objective_id, .. }
            | Step::Collect { objective_id, .. }
            | Step::Interact { objective_id, .. } => Some(objective_id.as_str()),
            Step::SelectClass { .. } | Step::AssertComplete { .. } => None,
        }
    }
}

/// The **party holder** (spec-0018): the single fake player that carries every
/// shared progression score.
///
/// Progress is a fact about the party, not about a player. Objective completion,
/// quest activation/completion, story flags, the announce-once latches and
/// campaign completion all live on `#party` — so any player's completing action
/// advances everyone, and two players clearing two arms of an `after` AND-join in
/// two different rooms unlock the successor together.
///
/// A fake player needs no entity and survives every join/leave, which is exactly
/// the lifetime party state needs. Everything that is genuinely per-player —
/// class + kit, `dw.dlg_shown`, the interact/dialogue triggers, `dw.dmask`, the
/// `deathCount` respawn edge, the stealth grace clocks, `dw.hold` — stays on the
/// player and is deliberately NOT routed here.
pub const PARTY: &str = "#party";

/// The declared mandatory party size (spec-0018 `world.min_players`), defaulting
/// to 1 — a party of one is always legal, and every pre-0.6 campaign reads as 1.
pub fn min_players(campaign: &Campaign) -> u8 {
    campaign.world.content.min_players.unwrap_or(1)
}

/// Sanitize an id's local part (after its `/`) to `[a-z0-9_]`.
pub fn safe_local(id: &str) -> String {
    let local = id.split_once('/').map(|(_, r)| r).unwrap_or(id);
    local.replace(['-', '/', '.'], "_")
}

/// Version of the `critical-path.json` **contract** (its `format_version` field),
/// independent of the campaign's DSL version: the DSL describes the delve, this
/// describes what the harness is told about proving it.
///
/// * `1` — the pre-oracle shape (never written; a file with no `format_version`).
///   Steps carried no objective id, so the harness could only check position and a
///   single unanchored campaign-completion substring — a step could pass without
///   its objective completing.
/// * `2` — every objective-bearing step carries `objective`, and completion is
///   proved by the anchored per-objective marker channel ([`marker_line`]).
///
/// The harness **requires** the current version: an older `critical-path.json`
/// (which it cannot verify) is rejected rather than run hollow.
pub const CRITICAL_PATH_FORMAT_VERSION: u32 = 2;

/// The machine completion-marker token for campaign completion. An objective's
/// token is simply its own id (`obj/<kebab>`).
pub const MARKER_TOKEN_CAMPAIGN: &str = "campaign";

/// One line of the machine completion-marker channel:
/// `[dw:complete <campaign_id> <token>]`.
///
/// The format is **anchored and exact**: the harness matches the whole chat line
/// against this grammar (campaign id = the running campaign's, token = `campaign`
/// or an `obj/<kebab>` id), never a substring anywhere in chat. Three properties
/// make it a real oracle:
/// * player chat reaches the client as `<name> …`, so no player can utter a line
///   that starts with the sigil;
/// * the campaign id is part of the match, so a marker from other content cannot
///   satisfy this campaign's step;
/// * the sigil is reserved in every player-visible string by `DW0182`
///   ([`delvewright_dsl::validate_marker_channel`]), so authored — or
///   LLM-translated — text cannot forge one.
pub fn marker_line(campaign_id: &str, token: &str) -> String {
    format!("[dw:complete {campaign_id} {token}]")
}

/// Scoreboard objective for a DSL objective id.
pub fn obj_score(objective_id: &str) -> String {
    format!("dw.o_{}", safe_local(objective_id))
}
/// Scoreboard objective marking a quest complete.
pub fn quest_score(quest_id: &str) -> String {
    format!("dw.q_{}", safe_local(quest_id))
}
/// Scoreboard objective marking a quest active (its trigger fired).
pub fn quest_active_score(quest_id: &str) -> String {
    format!("dw.qa_{}", safe_local(quest_id))
}
/// Trigger objective for an NPC's dialogue.
pub fn dlg_trigger(npc_id: &str) -> String {
    format!("dw.dlg_{}", safe_local(npc_id))
}
/// Per-player scoreboard for a campaign flag (`set-flag` / `requires_flags`, v0.3).
pub fn flag_score(flag_id: &str) -> String {
    format!("dw.f_{}", safe_local(flag_id))
}
/// Scoreboard objective holding a declared runtime datum (`state/<kebab>`, DSL
/// v0.10, spec-0031).
///
/// One objective per datum, holding an ordinary integer. **Who** holds the value
/// is the datum's declared scope, not a property of the objective: a `party`
/// datum lives on the [`PARTY`] fake player (where every story flag already
/// lives, spec-0018) and a `player` datum on each real player.
pub fn state_score(state_id: &str) -> String {
    format!("dw.s_{}", safe_local(state_id))
}

/// The per-player tag marking "this player's `player`-scoped data are seeded to
/// their declared initials" (DSL v0.10). Player tags live in player data, so the
/// seed runs exactly once per player per world — on their first tick, never
/// again on a relog, which is what makes a datum survive a disconnect the way a
/// scoreboard score does.
pub const STATE_SEEDED_TAG: &str = "dw_state";

/// Trigger objective the bot chats / an interaction advancement sets to drive an
/// `interact` objective (v0.3).
pub fn interact_trigger(obj_id: &str) -> String {
    format!("dw.i_{}", safe_local(obj_id))
}
/// The shared scoreboard objective holding every wave's remaining-mob countdown
/// (fake players `#<wave>`, v0.3).
pub const WAVE_OBJECTIVE: &str = "dw.wave";
/// The fake-player key holding a wave's remaining-mob count.
pub fn wave_counter(wave_id: &str) -> String {
    format!("#{}", safe_local(wave_id))
}
/// The entity tag stamped on a wave's spawned mobs (v0.3).
pub fn wave_tag(wave_id: &str) -> String {
    format!("dw_wave_{}", safe_local(wave_id))
}

/// The entity tag a **census brand** stamps on a wave's currently-living mobs
/// The harness applies it before a scripted death and reads it back
/// after the re-seat: a mob still wearing it is, by identity and not by
/// silhouette, one the previous life already fought.
///
/// Per wave rather than one shared brand, so branding one encounter can never
/// colour a neighbouring wave's census.
pub fn wave_brand_tag(wave_id: &str) -> String {
    format!("dw_brand_{}", safe_local(wave_id))
}

/// Marker token for the per-wave census SUMMARY line.
pub const MARKER_TOKEN_CENSUS: &str = "census";
/// Marker token for one mob's line inside a census.
pub const MARKER_TOKEN_CENSUS_MOB: &str = "censusmob";

/// A stage-5 wave by id (v0.3).
pub fn wave_of<'a>(campaign: &'a Campaign, wave_id: &str) -> Option<&'a delvewright_dsl::Wave> {
    campaign
        .quests
        .content
        .waves
        .iter()
        .find(|w| w.id.as_str() == wave_id)
}
/// A wave's total mob count.
pub fn wave_total(wave: &delvewright_dsl::Wave) -> i32 {
    wave.mobs.iter().map(|m| m.count as i32).sum()
}

/// The area a stage-4 quest belongs to (free-function form of [`Plan::quest_area`],
/// usable before a [`Plan`] exists — e.g. from anchor collection).
fn quest_area_of<'a>(campaign: &'a Campaign, quest_id: &str) -> Option<&'a str> {
    campaign
        .quest_plan
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == quest_id)
        .map(|q| q.area.as_str())
}

/// Does any effect in `effs`, or anywhere in the trees nested under them, fire a
/// `spawn-wave` for `wave_id`?
///
/// Descends through [`QuestEffect::visit_deep`], so `sequence` steps,
/// `set-checkpoint` `on_respawn`, `bonfire` `on_rest`, `begin-stealth`
/// `on_caught` and `move-npc`/`move-actor` `on_arrive` are all spawn sites — as
/// they already are for emission. A verb the emitter compiles from a nesting site
/// is a verb every consumer scan must see from the same site.
fn fires_wave<'a>(effs: impl IntoIterator<Item = &'a QuestEffect>, wave_id: &str) -> bool {
    let mut found = false;
    for e in effs {
        e.visit_deep(&mut |x| {
            if matches!(x.spawn_wave(), Some(w) if w.as_str() == wave_id) {
                found = true;
            }
        });
    }
    found
}

/// The area a wave's mobs spawn in — resolved from the wave's **spawn site**, not
/// from any `kill` objective. A `spawn-wave` effect (on a quest step, on a quest's
/// completion, or on an environment trigger) is what makes a wave appear; its
/// mobs materialize at `Wave.anchor` resolved in that spawning quest's area. This
/// is deliberately independent of objective type so a kill-less "live threat" wave
/// (spec-0008 §4 — e.g. a weakened warden the player sneaks past, an ambient mob
/// flock) resolves a spawn position exactly like a wave that is later slain.
///
/// Resolution order: the quest that fires the `spawn-wave` (`on_objective_complete`
/// or `on_complete`); else, in a single-area campaign, an environment trigger or a
/// trap payload that fires it (both are global — their sole possible area is the
/// one area); else a quest whose `kill` objective references the wave (defensive
/// fallback for a wave declared with a kill but no explicit spawn). `None` if
/// nothing spawns it.
///
/// **Every root is walked DEEP** ([`fires_wave`]), through
/// [`QuestEffect::nested_effect_lists`] — the DSL's single authority on effect
/// nesting, and the same authority `emit::all_campaign_effects` walks to decide
/// what to compile. A wave the emitter writes a `function <ns>:spawn_<wave>` call
/// for is therefore always a wave this function resolves an area for, and so
/// always a wave whose support machinery is emitted: the agreement is structural,
/// not two walks that have to remember each other.
///
/// It used to be a shallow scan of the top-level chains only, and the island's
/// round-21 build is what that cost: `wave/storm-shore` and `wave/storm-fire` were
/// fired from step 7 of a `sequence`, resolved no area, got no `spawn_…`, no
/// census, no brand and no kill reward — while `seq_under_ram` still shipped the
/// call. Two of three storm waves never spawned (`DW0497` is now the standing
/// proof that this class cannot ship again).
pub fn wave_area<'a>(campaign: &'a Campaign, wave_id: &str) -> Option<&'a str> {
    // 1. A quest whose effect TREE fires `spawn-wave` for this wave — the true
    //    spawn site.
    for q in &campaign.quests.content.quests {
        if fires_wave(
            q.on_objective_complete
                .values()
                .flatten()
                .chain(&q.on_complete),
            wave_id,
        ) {
            return quest_area_of(campaign, q.id.as_str());
        }
    }
    // 2. An environment trigger or trap payload that fires it. Both are global
    //    effect roots carrying no area of their own; in a single-area campaign the
    //    sole area is unambiguous. (Multi-area trigger-only waves are not
    //    resolvable here and surface as a build diagnostic rather than a silent
    //    dangling spawn.)
    if campaign.world.content.areas.len() == 1
        && (campaign
            .quests
            .content
            .triggers
            .iter()
            .any(|t| fires_wave(&t.effects, wave_id))
            || campaign
                .quests
                .content
                .traps
                .iter()
                .any(|t| fires_wave(&t.payload, wave_id)))
    {
        return campaign.world.content.areas.first().map(|a| a.id.as_str());
    }
    // 3. Defensive fallback: a `kill` objective's quest.
    for q in &campaign.quests.content.quests {
        if q.objectives
            .iter()
            .any(|o| matches!(o, Objective::Kill { wave, .. } if wave.as_str() == wave_id))
        {
            return quest_area_of(campaign, q.id.as_str());
        }
    }
    None
}

/// Errors that stop planning (map to build failure, exit 3). Carries a stable
/// `DW03xx` build/solver diagnostic code (catalogued in
/// `docs/reference/compiler.md` §5).
#[derive(Debug)]
pub struct PlanError {
    /// The stable `DW03xx` code.
    pub code: DwCode,
    /// Human-readable explanation.
    pub message: String,
    /// Advisory findings that were raised before this error stopped planning,
    /// and that explain it. Printed alongside the failure — a `DW0305` ambiguous
    /// anchor is usually the use-site symptom of a pool `DW0498` already
    /// describes at the declaration, and dropping the explanation because the
    /// build failed is exactly the silence `DW0498` exists to remove.
    pub warnings: Vec<Diagnostic>,
}

impl PlanError {
    /// Build a plan error with an explicit code.
    pub fn new(code: DwCode, message: impl Into<String>) -> Self {
        PlanError {
            code,
            message: message.into(),
            warnings: Vec::new(),
        }
    }

    /// The same error, carrying the advisories that explain it.
    pub fn with_warnings(mut self, warnings: Vec<Diagnostic>) -> Self {
        self.warnings = warnings;
        self
    }
}

/// `DW0300`: generic build/resolution failure (missing prefab metadata, unknown
/// anchor, dependency cycle in the critical path).
pub const DW_BUILD: DwCode = DwCode::every_version("DW0300");

/// `DW0306`: gate-aware reachability deadlock (M2 fix 7). After the solver produces
/// a layout, sealed gates are modelled as cut edges in the piece-connectivity
/// graph; an objective whose anchor is only reachable through a gate that no
/// earlier objective (in the quest/objective DAG order) has opened is a deadlock —
/// the delve is unwinnable even though every anchor resolves. The canonical case:
/// a key chest sealed behind the very gate its key opens.
pub const DW_GATE_DEADLOCK: DwCode = DwCode::every_version("DW0306");

/// `DW0344`: an ocean-horizon world places a piece whose declared waterline does not
/// land at sea level — the piece floats above the sea or is drowned by it.
///
/// It is also the code this invariant's **zero binding** refuses under: a gate
/// that examined nothing has proved nothing, and the gate that examined nothing
/// is this one, so it answers under its own name rather than under a second
/// code. See [`WaterlineBinding::seal`].
pub const DW_OCEAN_WATERLINE: DwCode = DwCode::every_version("DW0344");

/// `DW0345`: the assembled world resolves **no entry anchor** — the compiler has
/// no cell to call the campaign's start, so it cannot `setworldspawn`, cannot place
/// a first-joining player, and cannot teleport a player who picks a class. The
/// world then falls back to the vanilla spawn search, which a dedicated server
/// resolves to the surface but the integrated (singleplayer) server resolves to
/// the build floor — inside solid stone. Silent before; a hard build error now.
pub const DW_NO_ENTRY_ANCHOR: DwCode = DwCode::every_version("DW0345");

/// The prefab-metadata anchor names that mark a campaign's **entry point**, in
/// resolution order. One concept, two spellings in the shipped tileset library:
/// the keep/cave/test tilesets name it `spawn`, the island tileset names it
/// `entry`. The compiler owns the resolution (CLAUDE.md: never leave a layer
/// boundary to downstream folklore) — every consumer goes through
/// [`Plan::entry_point`] / `emit::campaign_spawn`, and a campaign that resolves
/// none of these names fails the build with [`DW_NO_ENTRY_ANCHOR`].
pub const ENTRY_ANCHOR_NAMES: [&str; 2] = ["spawn", "entry"];

/// Ocean-horizon waterline invariant (DW0344). In a `horizon: ocean` world every
/// placed piece that **declares** a waterline (`waterline_y` in its prefab metadata,
/// local y of its top authored water block) must land with that waterline at world
/// [`SEA_LEVEL`] — `piece.pos.y + waterline_y == 62`.
///
/// Why this is a hard invariant rather than a style rule: the island convention
/// (`prefabs/island-tileset.md`) puts the walkable land plane one block above the
/// waterline, which is the vanilla-normal beach relationship — a player swimming in
/// open sea can climb ashore, and the authored water reads as one body with the
/// world ocean. Off by a few blocks and the whole island floats above the sea: the
/// shore becomes an unclimbable cliff and the authored water pocket hangs in the
/// air. Nothing downstream (nav, boundary, POV, PackTest) can see this, because
/// every one of them derives from the very placement that is wrong.
///
/// Pieces that declare no `waterline_y` (interior keep/cave pieces, `hello-room`)
/// are not island pieces and are not checked.
///
/// # The binding count, and why it is a diagnostic
///
/// "Not checked" is the whole exposure. This invariant is keyed off a single
/// optional metadata field, so a piece that loses that field does not fail the
/// check — it leaves it, silently, and the world it was supposed to prove ships
/// green. That is not hypothetical: the field lives on five island prefabs, and
/// the admission tool that reads and rewrites their metadata modelled fewer
/// fields than the document has, so every admission step deleted it. Had that
/// reached the library, an ocean world would have compiled with this invariant
/// examining zero pieces and nothing would have said so.
///
/// So the check returns how many placed pieces it examined, and an ocean world
/// where that count is zero is sealed by [`WaterlineBinding::seal`] rather than
/// passed.
///
/// A world with no sea-authoring pieces at all really is a legitimate reason for
/// the zero. That does not make the zero advisory: it makes the honesty of the
/// emptiness a thing to **compute**. Lowering the severity instead would be an
/// opt-out secured by exactly the property in question — a piece that lost its
/// `waterline_y` and a piece that never had one are indistinguishable by the
/// declaration, because the declaration is what went missing. The discharge is
/// therefore taken from geometry, which the defect cannot edit: see
/// [`WaterlineBinding::reaching_sea`].
fn check_ocean_waterline(
    campaign: &Campaign,
    areas: &[AreaPlacement],
    prefabs: &PrefabRegistry,
) -> Result<WaterlineBinding, PlanError> {
    if !matches!(
        campaign.world.content.horizon,
        Some(delvewright_dsl::Horizon::Ocean)
    ) {
        return Ok(WaterlineBinding::NOT_AN_OCEAN);
    }
    let mut binding = WaterlineBinding {
        ocean: true,
        placed: 0,
        checked: 0,
        reaching_sea: 0,
    };
    for area in areas {
        for piece in &area.pieces {
            binding.placed += 1;
            // The computed discharge, taken before any metadata is consulted:
            // does this piece's box reach the sea plane at all? Geometry, not
            // declaration — the defect deletes declarations and cannot move a
            // box.
            if piece.bbox().0[1] <= SEA_LEVEL {
                binding.reaching_sea += 1;
            }
            let Some(meta) = prefabs.get(&piece.prefab_id) else {
                continue; // missing metadata is already DW0300 upstream
            };
            let Some(w) = meta.waterline_y else {
                continue;
            };
            binding.checked += 1;
            let placed = piece.pos[1] + w;
            if placed != SEA_LEVEL {
                let delta = placed - SEA_LEVEL;
                let (dir, verb) = if delta > 0 {
                    (
                        "above",
                        "floats above the sea — its shore is an unclimbable cliff",
                    )
                } else {
                    ("below", "is drowned — the walk plane sits under the sea")
                };
                return Err(PlanError::new(
                    DW_OCEAN_WATERLINE,
                    format!(
                        "area `{}` places prefab `{}` at y={} with a declared waterline of local \
                         y={w}, putting its waterline at world y={placed} — {} blocks {dir} the \
                         ocean sea level (y={SEA_LEVEL}). The piece {verb}. Prefab metadata and \
                         placement disagree about the island datum: either declare the waterline \
                         the piece really authors (`waterline_y` in `{}.json`, the local y of its \
                         top water block — the island tileset convention is {ISLAND_WATERLINE_Y}), \
                         or rebuild the piece against that convention. Ocean areas are placed at \
                         y={OCEAN_BASE_Y} (= sea level - {ISLAND_WATERLINE_Y}); a piece with a \
                         different waterline cannot share that datum",
                        area.area_id,
                        piece.prefab_id,
                        piece.pos[1],
                        delta.abs(),
                        meta.base(),
                    ),
                ));
            }
        }
    }
    Ok(binding)
}

/// How much of the world the ocean-datum invariant actually examined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaterlineBinding {
    /// Whether this world declares `horizon: ocean` at all.
    pub ocean: bool,
    /// Placed pieces in the world.
    pub placed: usize,
    /// Placed pieces that declare a `waterline_y` — the binding count.
    pub checked: usize,
    /// Placed pieces whose box reaches the sea plane (`lo.y <= SEA_LEVEL`).
    ///
    /// Reported, never used as an excuse. It is the number that decides
    /// whether a zero binding could ever be honest, and under the single
    /// global ocean datum ([`OCEAN_BASE_Y`] = 60, sea at [`SEA_LEVEL`] = 62)
    /// it is equal to [`Self::placed`] for every ocean world that exists: an
    /// area origin is the same y for every piece, and that y is under the sea.
    /// So there is at present **no** ocean world whose pieces stand clear of
    /// the water, which is why [`Self::seal`] offers no discharge and refuses
    /// outright.
    ///
    /// It is computed rather than assumed because the fact is a property of
    /// the datum, not a law: spec-0026 replaces the global datum with a
    /// per-area one, at which point pieces genuinely can sit clear of the sea
    /// and this count stops tracking `placed`. Reading it off geometry now
    /// means the day that happens the number is already right.
    pub reaching_sea: usize,
}

impl WaterlineBinding {
    /// A world with no ocean horizon: the invariant does not apply, which is a
    /// different statement from applying and binding to nothing.
    pub const NOT_AN_OCEAN: WaterlineBinding = WaterlineBinding {
        ocean: false,
        placed: 0,
        checked: 0,
        reaching_sea: 0,
    };

    /// **Seal the binding**: what a binding of zero means for this invariant.
    ///
    /// A check that examined nothing has proved nothing, so the verdict a zero
    /// binding *earns* here is a refusal — the answer
    /// `grammar::gates::seal_zero_bindings` reaches at the other door, and the
    /// two doors are not allowed to disagree about what a zero means. No
    /// discharge is available to soften it, and both candidates fail the two
    /// rules that mechanism sets for an honest empty:
    ///
    /// - The only thing an author could offer is "this piece needs no
    ///   waterline" — the deleted declaration wearing a different name. The
    ///   defect produces it perfectly, so it is an opt-out secured by exactly
    ///   the property in question.
    /// - The only geometric candidate is "no piece reaches the sea"
    ///   ([`Self::reaching_sea`] = 0), and under the single global ocean datum
    ///   no such world can be built: every ocean area origin is
    ///   [`OCEAN_BASE_Y`] (60) and the sea is at [`SEA_LEVEL`] (62), so every
    ///   piece stands in the water. A discharge no world can satisfy is a dead
    ///   escape hatch that reads like a live one.
    ///
    /// # Why this reports rather than refuses, and what changes that
    ///
    /// A refusal is only landable when what it demands is authorable, and here
    /// it is not. The same global datum that makes the geometric discharge
    /// unsatisfiable also leaves an author no lever: a piece's walk plane lands
    /// where its own geometry puts it above y=60, and nothing in the DSL can
    /// raise it clear of the sea. So the only move that would green a refusal
    /// is declaring a `waterline_y` for water the piece does not author — the
    /// gate would be demanding a fiction, which is the same vacuity arriving
    /// from the other side.
    ///
    /// The capability that makes the demand satisfiable is spec-0026's
    /// per-area datum, and it arrives together with the gate that supplies the
    /// honest discharge: an empirical flood proof that reads assembled blocks
    /// rather than declarations, and therefore cannot be emptied by editing
    /// metadata. The refusal belongs in that change, not ahead of it.
    ///
    /// That deferral is **bound, not remembered**: the fixture in
    /// `tests/cli.rs` asserts that every placed piece of an ocean world stands
    /// at or below the sea plane. The day a per-area datum makes that false,
    /// the assertion reds and the severity question is reopened by the test
    /// rather than by anyone recalling this paragraph.
    pub fn seal(&self) -> Option<Diagnostic> {
        if !self.ocean || self.checked > 0 || self.placed == 0 {
            return None;
        }
        Some(Diagnostic::warning(
            DW_OCEAN_WATERLINE,
            "world",
            "/content/horizon",
            format!(
                "the ocean-datum check examined ZERO of {placed} placed piece(s): this world \
                 declares `horizon: ocean` and {reaching} of those piece(s) stand at or below the \
                 sea plane (y={SEA_LEVEL}), but not one declares a `waterline_y` in its prefab \
                 metadata. Nothing here proves that anything in this world meets the sea where \
                 the sea is, while every downstream proof — nav, boundary, POV, PackTest — \
                 derives from the placement none of them checked. A check that examined nothing \
                 has proved nothing, so this refuses rather than noting itself: the invariant is \
                 keyed off an optional field, which makes a declaration that was DELETED look \
                 exactly like one that was never needed, and those two need opposite answers. \
                 Declare `waterline_y` on the piece(s) that meet the sea — the local y of the \
                 top authored water block, {ISLAND_WATERLINE_Y} by the island convention \
                 (`prefabs/island-tileset.md`) — or, if this world really authors no shore, it \
                 is still standing in the water at y={OCEAN_BASE_Y} and wants a horizon that is \
                 not `ocean`",
                placed = self.placed,
                reaching = self.reaching_sea,
            ),
        ))
    }
}

#[cfg(test)]
mod waterline_binding_tests {
    use super::*;

    /// The three states, and the one that is not a pass.
    ///
    /// `NOT_AN_OCEAN` and "examined something" are silent for different
    /// reasons, and a world that placed nothing has no population at all —
    /// none of the three is the case this exists for.
    #[test]
    fn only_an_ocean_that_placed_pieces_and_examined_none_reports() {
        assert!(WaterlineBinding::NOT_AN_OCEAN.seal().is_none());
        let bound = WaterlineBinding {
            ocean: true,
            placed: 3,
            checked: 3,
            reaching_sea: 3,
        };
        assert!(bound.seal().is_none(), "a bound check says nothing");
        let empty = WaterlineBinding {
            ocean: true,
            placed: 0,
            checked: 0,
            reaching_sea: 0,
        };
        assert!(empty.seal().is_none(), "no pieces is not a zero binding");

        let unbound = WaterlineBinding {
            ocean: true,
            placed: 2,
            checked: 0,
            reaching_sea: 2,
        };
        let d = unbound.seal().expect("a zero binding is never silent");
        assert_eq!(d.code, "DW0344", "it answers under its own code");
        assert!(
            d.message.contains("examined ZERO of 2 placed piece(s)"),
            "the binding count is stated: {}",
            d.message
        );
    }
}

/// Inter-area transport map: objective id → absolute teleport target (see
/// [`Plan::transport`]).
pub type TransportMap = BTreeMap<String, [i32; 3]>;

impl<'a> Plan<'a> {
    /// Build the plan. Requires a validated campaign and loaded prefab metadata.
    pub fn build(campaign: &'a Campaign, prefabs: &PrefabRegistry) -> Result<Self, PlanError> {
        let namespace = campaign.world.campaign_id.as_str().to_string();
        let seed = campaign.world.content.seed;

        // ---- placements + anchors ----
        let mut areas = Vec::new();
        // Advisory placement findings, in area order (`DW0498`, `crate::pool`).
        let mut warnings: Vec<Diagnostic> = Vec::new();
        let mut anchors: BTreeMap<(String, String), ResolvedAnchor> = BTreeMap::new();
        // v0.6 (spec-0011): the absolute dispenser socket cell for each `anchor/trap`
        // marker that declares one, keyed like `anchors`. Empty for a campaign with no
        // trap hardware.
        let mut dispenser_cells: BTreeMap<(String, String), [i32; 3]> = BTreeMap::new();
        // Per-batch affected AABBs from L2 massing (spec-0017), for the
        // editor's per-batch snapshots. Empty without massing verbs.
        let mut massing_bounds: BTreeMap<String, ([i32; 3], [i32; 3])> = BTreeMap::new();
        // Socket doorways severed by `rewire-socket sealed`, per area — the
        // DW0306 connectivity graph must not count those edges.
        let mut severed: BTreeMap<String, BTreeSet<[i32; 3]>> = BTreeMap::new();
        // Origin Y is a per-horizon datum (spec-0013): void keeps 64, ocean drops to
        // sea_level-2 so the island waterline convention holds.
        let base_y = base_y(campaign);
        for (i, area) in campaign.world.content.areas.iter().enumerate() {
            let area_id = area.id.as_str().to_string();
            let origin = [i as i32 * AREA_SPACING, base_y, 0];

            let placement = if let Some(prefab) = &area.prefab {
                // Single-prefab area (the M1 degenerate assembly): one piece at
                // the origin, rotation none — and every connector it declares is
                // unmated by construction, because there is no second piece for
                // one to mate with.
                let prefab_id = prefab.as_str().to_string();
                let meta = prefabs.get(&prefab_id).ok_or_else(|| {
                    PlanError::new(
                        DW_BUILD,
                        format!(
                            "area `{area_id}` binds prefab `{prefab_id}` but no matching prefab \
                             metadata was found in the prefabs dir — bind a prefab that exists in \
                             the prefab library, or add `{prefab_id}` (`.nbt` + metadata) to it. \
                             This is a prefab-library/naming issue, not a quest-logic one"
                        ),
                    )
                })?;
                if crate::massing::targets_area(campaign, &area_id) {
                    return Err(PlanError::new(
                        crate::massing::DW_MASSING,
                        format!(
                            "world-edits massing verbs target area `{area_id}`, which binds a \
                             single `prefab` — there is no jigsaw layout to mass. Massing \
                             applies only to `prefab_pool` areas; use the L3 detailing verbs \
                             (carve/fill/fragment/…) on a single-prefab area instead"
                        ),
                    ));
                }
                for (name, am) in &meta.anchors {
                    anchors.insert((area_id.clone(), name.clone()), resolve_anchor(origin, am));
                    if let Some(dp) = am.dispenser {
                        dispenser_cells.insert(
                            (area_id.clone(), name.clone()),
                            [origin[0] + dp[0], origin[1] + dp[1], origin[2] + dp[2]],
                        );
                    }
                }
                // Seal this piece's sockets through the SAME mechanism the pool
                // path uses (`solver::seal_layout`), rather than skipping it.
                //
                // A connector is a hole the prefab deliberately leaves in its own
                // wall — a 3×3 doorway plus the `minecraft:jigsaw` marker at its
                // sill — and the solver's invariant is "every unmated socket is
                // sealed with wall material; every mated socket's jigsaw block is
                // cleared to air". That invariant is a property of a PLACED PIECE,
                // not of having run the layout solver, and binding it to the
                // solver left the one-piece case with no surface: a single-prefab
                // area shipped the doorway wide open onto whatever the horizon
                // says lies outside the piece, with the authoring marker still
                // standing in it as a real block a player can stand on.
                //
                // Nothing caught it, because both symptoms hide behind content.
                // `DW0322` reads the exposed doorway as a walkable cell one step
                // from a void drop — but only once the doorway is REACHABLE, and
                // a flooded cell is impassable, so any standing water between the
                // piece's anchors and its own sill deletes the finding while
                // leaving the hole. Correcting such a flood is what exposes it,
                // which is the worst possible time to first meet it. And the
                // stray jigsaw never surfaces in the datapack at all: `snapshot`
                // colours it the magenta fallback precisely as an alarm, on the
                // stated premise that "the solver strips them" — true of every
                // piece a solver had placed, and of no other.
                //
                // Byte impact is confined to what was broken: a prefab with no
                // connectors yields no seals, so every campaign and fixture that
                // binds one is byte-identical.
                let (bbox_min, bbox_max) = Rotation::None.bbox(origin, meta.size());
                let seals = solver::seal_layout(
                    prefabs,
                    &[solver::PlacedPiece {
                        prefab_id: prefab_id.clone(),
                        pos: origin,
                        rotation: Rotation::None,
                        bbox_min,
                        bbox_max,
                        mated: vec![false; meta.connectors.len()],
                    }],
                );
                AreaPlacement {
                    area_id: area_id.clone(),
                    pieces: vec![PiecePlacement {
                        prefab_id,
                        templates: placed_templates(meta, origin, Rotation::None),
                        pos: origin,
                        size: meta.size(),
                        rotation: Rotation::None,
                    }],
                    seals,
                }
            } else if let Some(pool) = &area.prefab_pool {
                // Pool area (ADR-0004 jigsaw assembly): the solver grows a layout
                // from the campaign seed and we transform each piece's anchors to
                // world space. `pieces` bounds are guaranteed present by validation
                // (a pool binds `pieces`); default defensively.
                let pool_id = pool.as_str().to_string();
                let (pmin, pmax) = area.pieces.map(|p| (p.min, p.max)).unwrap_or((1, 1));
                let required = required_anchors_for_area(campaign, &area_id);
                let mut stream = Splitmix64::new(solver::stream_seed(seed, &area_id));
                let mut layout = solver::solve_area(
                    prefabs,
                    &pool_id,
                    &required,
                    pmin,
                    pmax,
                    origin,
                    &mut stream,
                )
                .map_err(|e| {
                    // A solver failure raised after growth (`DW0305`) carries the
                    // draw that produced it: attach the pool-level `DW0498` so the
                    // author reads the cause at the declaration, not just the
                    // symptom at the use site.
                    let mut w = warnings.clone();
                    w.extend(crate::pool::check(
                        prefabs,
                        &crate::pool::PoolArea {
                            area_id: &area_id,
                            area_index: i,
                            pool_id: &pool_id,
                            pieces_min: pmin,
                            pieces_max: pmax,
                        },
                        e.placed.iter().map(String::as_str),
                    ));
                    PlanError::new(e.code, e.message).with_warnings(w)
                })?;
                // Stage-7 L2 massing (spec-0017): apply the edit script's
                // massing batches for this area over the solved layout, so
                // everything downstream — anchor resolution just below, the
                // gate/waterline checks, assembly, relight, nav, the L3
                // detailing replay — sees the massaged layout. No-op (layout
                // and seals byte-identical) for a campaign without massing
                // verbs targeting this area.
                let massing_out =
                    crate::massing::apply(campaign, &area_id, &mut layout, prefabs, seed)
                        .map_err(|e| PlanError::new(e.code, e.message))?;
                massing_bounds.extend(massing_out.bounds);
                if !massing_out.severed.is_empty() {
                    severed.insert(area_id.clone(), massing_out.severed);
                }

                // `DW0498`: the draw is settled — read it back and say
                // so ONCE, here at the declaration, if it seats the same
                // anchor-bearing prefab more than once. Every anchor that prefab
                // declares now has more than one carrier; the `or_insert_with`
                // resolution just below silently keeps the first, and the solver's
                // `DW0305` will fail the build at whichever campaign-referenced
                // anchor happens to be the first use site. Advisory: a repeat with
                // no such use is legal, and shipping campaigns rely on it. Read
                // AFTER massing so the reported draw is the one the player gets.
                warnings.extend(crate::pool::check(
                    prefabs,
                    &crate::pool::PoolArea {
                        area_id: &area_id,
                        area_index: i,
                        pool_id: &pool_id,
                        pieces_min: pmin,
                        pieces_max: pmax,
                    },
                    layout.pieces.iter().map(|p| p.prefab_id.as_str()),
                ));

                let mut pieces = Vec::new();
                for placed in &layout.pieces {
                    let meta = prefabs.get(&placed.prefab_id).ok_or_else(|| {
                        PlanError::new(
                            DW_BUILD,
                            format!(
                                "internal invariant violation: the solver placed prefab `{}`, \
                                 which has no metadata entry — the solver and metadata registry \
                                 disagree. This is a compiler bug, not a campaign error; stop and \
                                 escalate",
                                placed.prefab_id
                            ),
                        )
                    })?;
                    // Transform this piece's anchors to world space. Each required
                    // anchor is carried by exactly one placed piece (fillers are
                    // anchorless connectors), so names do not collide.
                    for (name, am) in &meta.anchors {
                        anchors
                            .entry((area_id.clone(), name.clone()))
                            .or_insert_with(|| resolve_piece_anchor(placed, am));
                        if let Some(dp) = am.dispenser {
                            dispenser_cells
                                .entry((area_id.clone(), name.clone()))
                                .or_insert_with(|| solver::transform_point(placed, dp));
                        }
                    }
                    pieces.push(PiecePlacement {
                        prefab_id: placed.prefab_id.clone(),
                        templates: placed_templates(meta, placed.pos, placed.rotation),
                        pos: placed.pos,
                        size: meta.size(),
                        rotation: placed.rotation,
                    });
                }
                AreaPlacement {
                    area_id: area_id.clone(),
                    pieces,
                    seals: layout.seals,
                }
            } else {
                // Validation (DW0160) guarantees exactly one binding.
                return Err(PlanError::new(
                    DW_BUILD,
                    format!(
                        "internal invariant violation: area `{area_id}` binds neither `prefab` \
                         nor `prefab_pool` at build time — `DW0160` should have rejected this \
                         during validation. This is a compiler bug; stop and escalate"
                    ),
                ));
            };
            areas.push(placement);
        }

        // ---- gate-aware reachability (M2 fix 7, DW0306) ----
        // With the layout solved, verify no objective's anchor is sealed behind a
        // gate that only a later objective opens (an unwinnable deadlock the anchor
        // resolver alone cannot see).
        for area in &areas {
            check_gate_reachability(
                campaign,
                &area.area_id,
                &area.pieces,
                prefabs,
                severed.get(&area.area_id),
            )?;
        }

        // ---- ocean waterline invariant (DW0344) ----
        //
        // Bound here and nowhere else, on the same reasoning as the mating check
        // below: every campaign build goes through `Plan::build`. The binding
        // count comes back with the verdict, because a check keyed off an
        // optional metadata field goes quiet rather than red when the field
        // disappears — and `seal` is what stops that quiet from reading as a
        // pass.
        if let Some(finding) = check_ocean_waterline(campaign, &areas, prefabs)?.seal() {
            warnings.push(finding);
        }

        // ---- the pieces fit together (DW0780/DW0781, ADR-0020) ----
        //
        // Bound here and nowhere else: every campaign build goes through
        // `Plan::build`, so a world whose pieces contradict each other at the
        // faces they share cannot be compiled, packaged or shipped. There is no
        // flag and no separate command to remember.
        let binding = crate::faces::check(&areas, prefabs).map_err(|e| {
            let mut w = warnings.clone();
            w.extend(e.warnings.clone());
            PlanError::new(e.code, e.message).with_warnings(w)
        })?;
        if let Some(finding) = binding.finding(crate::faces::placed_pieces(&areas)) {
            warnings.push(finding);
        }

        // ---- the pieces fit together (DW0780/DW0781, ADR-0020) ----
        //
        // Bound here and nowhere else: every campaign build goes through
        // `Plan::build`, so a world whose pieces contradict each other at the
        // faces they share cannot be compiled, packaged or shipped. There is no
        // flag and no separate command to remember.
        let binding = crate::faces::check(&areas, prefabs).map_err(|e| {
            let mut w = warnings.clone();
            w.extend(e.warnings.clone());
            PlanError::new(e.code, e.message).with_warnings(w)
        })?;
        if let Some(finding) = binding.finding(crate::faces::placed_pieces(&areas)) {
            warnings.push(finding);
        }

        // ---- classes ----
        let classes = campaign
            .classes
            .content
            .classes
            .iter()
            .enumerate()
            .map(|(i, c)| ClassPlan {
                n: i as i32 + 1,
                class_id: c.id.as_str().to_string(),
                safe: safe_local(c.id.as_str()),
            })
            .collect();

        // ---- npc dialogue numbering ----
        // Dialogue lives in stage 6 (1:1 with stage-2 NPCs, guaranteed by
        // validation, which `build` implies). An NPC without a tree is skipped
        // defensively.
        let npcs = campaign
            .npcs
            .content
            .npcs
            .iter()
            .filter_map(|npc| {
                campaign
                    .dialogue
                    .content
                    .tree_for(npc.id.as_str())
                    .map(|tree| plan_npc(npc, tree))
            })
            .collect::<Vec<_>>();

        // ---- critical path + inter-area transport ----
        let flow = crate::flow::Flow::new(campaign);
        let cp = build_critical_path(campaign, &anchors, &npcs, &flow, &flow.playthrough())?;

        // ---- v0.6 checkpoints + stealth beats (spec-0012 / spec-0014) ----
        let (checkpoints, stealth_beats) = collect_v06_effects(campaign, &anchors, &cp.obj_step);
        let objective_steps = cp.obj_step;

        // ---- v0.6 traps (spec-0011) ----
        let traps = collect_traps(campaign, &anchors, &dispenser_cells);

        // ---- shortcut doors (spec-0016 §2) ----
        let shortcuts = collect_shortcuts(campaign, &anchors);

        // ---- container fills (spec-0021) ----
        let loot = collect_loot(campaign, &anchors);

        // ---- lethal volumes (spec-0031) ----
        let lethal_volumes = collect_lethal_volumes(campaign, &anchors);

        // ---- `collect` container adoption (DSL v0.8) ----
        let collect_fills = collect_collect_fills(campaign, &anchors);

        // ---- ambushes (spec-0016 §3) ----
        let ambushes = collect_ambushes(campaign, &anchors);

        // ---- timed gates (spec-0016 §4) ----
        let timed_gates: Vec<TimedGatePlan> = campaign
            .quests
            .content
            .timed_gates
            .iter()
            .filter_map(|g| {
                let (from, to, block) = gate_region_block_any(&anchors, g.gate.as_str())?;
                Some(TimedGatePlan {
                    id: g.id.as_str().to_string(),
                    safe: safe_local(g.id.as_str()),
                    gate_anchor: g.gate.as_str().to_string(),
                    gate_region: (from, to),
                    gate_block: block,
                    open_ticks: g.open_ticks,
                    closed_ticks: g.closed_ticks,
                    phase: g.phase,
                    crush: g.crush,
                    disarm: g.disarm.as_ref().and_then(|dis| {
                        point_any(&anchors, dis.via.as_str()).map(|via_cell| TimedGateDisarmPlan {
                            via_anchor: dis.via.as_str().to_string(),
                            via_cell,
                            sets_flag: dis.sets_flag.as_str().to_string(),
                        })
                    }),
                })
            })
            .collect();

        // ---- v0.8 seal hints: what a sealed gate answers ----
        let seal_hints = collect_seal_hints(campaign, &anchors);

        // ---- the press answers (DSL v0.11): what every sealed body answers ----
        // Collected AFTER both `shortcuts` and `seal_hints`, because it is derived
        // from the union of the two — one rule over the pressable class, not one
        // rule per verb.
        let press_answers = collect_press_answers(campaign, &seal_hints, &shortcuts);

        // ---- the contingent ways the placed world stages (spec-0042 §2.4) ----
        //
        // Before the region-write model, because an `open-way`'s geometry IS a
        // staged way: the campaign names a piece and a way, and everything else
        // about the write comes from the piece. Sealed here and nowhere else —
        // every campaign build goes through `Plan::build`, so a way that reaches
        // the world with no cells to open cannot be compiled, packaged or
        // shipped, and there is no flag and no separate command to remember.
        let ways = crate::ways::stage(&areas, prefabs);
        ways.seal().map_err(|e| e.with_warnings(warnings.clone()))?;

        // ---- v0.6 gate open/close firings (drives the close-gate nav proof) ----
        let mut region_events = collect_region_events(campaign, &anchors, &objective_steps, &ways);
        // A shortcut gate is sealed from world-load and is opened only by an
        // OPTIONAL far-side interaction no proof can order (spec-0016 §2). Seal it
        // for the whole completability model — `fire_step: 0` precedes every leg —
        // so the critical path, the checkpoints and the traps are all proven over
        // a world where no shortcut has been taken. The delve must be finishable
        // the long way; the shortcut is a reward, never a requirement.
        // FORCED, and the word is exact: what is unforced about a shortcut is the
        // player OPENING it, and that firing is registered separately (and dropped,
        // being an unseal from an optional root). The door standing shut is a fact
        // about the world at load — nobody has to do anything for it to be true — so
        // its footing is footing the party really has.
        region_events.extend(shortcuts.iter().map(|sc| {
            RegionEvent::forced(sc.gate_region, RegionWrite::of_block(&sc.gate_block), 0)
        }));
        let strict_ancestor_steps = compute_strict_ancestor_steps(campaign, &objective_steps);
        // v0.10 (spec-0031): where the party can be CARRIED rather than walk.
        let transit_teleports = collect_transit_teleports(campaign, &anchors);

        let region_events = region_events;

        // ---- what became of every staged way (spec-0042 §2.5, DW0548) ----
        //
        // Last, because it needs everything above it: the staged ways, the
        // openings' DAG points, the resolved anchors and the strict-ancestor
        // relation the region-write model orders the world by. The ancestry
        // predicate is `Plan::gate_fired_before`'s body, handed over rather than
        // re-derived — a second reading of "has this fired yet" is a second
        // instrument, and the whole point of the verdict is that it agrees with
        // the route proof.
        let mut way_gate = None;
        if !ways.ways.is_empty() {
            let openings = collect_way_openings(campaign, &objective_steps);
            let elements = collect_required_elements(campaign, &anchors, &objective_steps);
            let precedes = |g: usize, s: usize| {
                g == 0
                    || strict_ancestor_steps
                        .get(&s)
                        .is_some_and(|anc| anc.contains(&g))
            };
            let gate = crate::ways::judge(&ways, &openings, &elements, &areas, prefabs, &precedes)
                .map_err(|e| e.with_warnings(warnings.clone()))?;
            if let Some(finding) = crate::ways::unbound_finding(&gate) {
                warnings.push(finding);
            }
            way_gate = Some(gate);
        }

        Ok(Self {
            campaign,
            namespace,
            seed,
            areas,
            warnings,
            anchors,
            classes,
            npcs,
            critical_path: cp.steps,
            transport: cp.transport,
            critical_path_transport: cp.transport_by_step,
            critical_path_sneak: cp.sneak_by_step,
            critical_path_cutscene: cp.cutscene_by_step,
            checkpoints,
            lethal_volumes,
            stealth_beats,
            objective_steps,
            traps,
            shortcuts,
            loot,
            collect_fills,
            ambushes,
            timed_gates,
            seal_hints,
            press_answers,
            region_events,
            ways,
            way_gate,
            transit_teleports,
            strict_ancestor_steps,
            massing_bounds,
        })
    }

    /// The EXECUTABLE critical path of one enumerated branch (spec-0025 §3).
    ///
    /// The same [`build_critical_path`] the exported `critical-path.json` is made
    /// of, driven by the playthrough of the world that realizes a branch instead
    /// of the default one. That identity is the point: a branch run must walk
    /// steps of exactly the shape the ladder already proves, or "branch coverage"
    /// would mean coverage of a second, less-tested contract. The branch's
    /// **scripted dialogue choices are inside the result** — each `talk-to` step
    /// carries the `/trigger` line of the option that belongs to THIS branch,
    /// which is the only player-legal way to actuate a server-driven dialog
    /// button (mineflayer cannot click one).
    ///
    /// Not called for an unreachable branch: there is no world to walk, and
    /// `DW0482` has already failed the build.
    ///
    /// `flow` is the model `path` came out of: the builder reads the flag state
    /// this branch holds at each step from its journal, which is how a `talk-to`
    /// step lands on the cast row THIS branch declares (`crate::cast::station`).
    pub fn branch_critical_path(
        &self,
        flow: &crate::flow::Flow<'_>,
        path: &crate::flow::Playthrough,
    ) -> Result<CriticalPath, PlanError> {
        build_critical_path(self.campaign, &self.anchors, &self.npcs, flow, path)
    }

    /// The gate/seal model of ONE branch's exported path (spec-0025):
    /// the campaign's `open-gate`/`close-gate` firings with `fire_step` indices in
    /// the **branch path's own step space**, plus the strict DAG-ancestor relation
    /// over that space — exactly the model [`Plan::build`] computes for the
    /// exported path (`region_events` / `strict_ancestor_steps`), driven by the
    /// branch's own objective→step map instead of the default playthrough's.
    ///
    /// A branch path is a *different sequence* of steps, so the default path's
    /// step indices cannot be carried across (the same trap `rest_step_index`
    /// documents for bonfires): a seal attributed through the default indices
    /// would inherit another branch's ordering. Shortcut gates are sealed from
    /// world-load (`fire_step: 0`) here for the same reason they are in
    /// [`Plan::build`] — the branch must be walkable the long way too.
    ///
    /// Deterministic: both halves are pure functions of the campaign and the
    /// branch's own `CriticalPath` (ADR-0006).
    pub fn branch_gate_model(
        &self,
        cp: &CriticalPath,
    ) -> (Vec<RegionEvent>, BTreeMap<usize, BTreeSet<usize>>) {
        let mut region_events =
            collect_region_events(self.campaign, &self.anchors, &cp.obj_step, &self.ways);
        region_events.extend(self.shortcuts.iter().map(|sc| {
            RegionEvent::forced(sc.gate_region, RegionWrite::of_block(&sc.gate_block), 0)
        }));
        let ancestors = compute_strict_ancestor_steps(self.campaign, &cp.obj_step);
        (region_events, ancestors)
    }

    /// Whether a gate firing at critical-path step `g` is guaranteed to have fired
    /// before a walked leg arriving at step `s` — i.e. `g`'s objective is a strict
    /// DAG ancestor of `s`'s objective (see [`Self::strict_ancestor_steps`]). Step
    /// `0` (class-select / an environment trigger's conservative fire step) is
    /// treated as always-preceding. Drives the `close-gate` seal model in
    /// `crate::nav`.
    pub fn gate_fired_before(&self, g: usize, s: usize) -> bool {
        g == 0
            || self
                .strict_ancestor_steps
                .get(&s)
                .is_some_and(|anc| anc.contains(&g))
    }

    /// The area an NPC or quest belongs to.
    pub fn npc_area(&self, npc_id: &str) -> Option<&str> {
        self.campaign
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc_id)
            .map(|n| n.area.as_str())
    }

    /// The area a stage-4 quest belongs to.
    pub fn quest_area(&self, quest_id: &str) -> Option<&str> {
        self.campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == quest_id)
            .map(|q| q.area.as_str())
    }

    /// Resolve `(area, anchor)` to a point position, if it is a point anchor.
    pub fn point(&self, area_id: &str, anchor: &str) -> Option<[i32; 3]> {
        match self.anchors.get(&(area_id.to_string(), anchor.to_string())) {
            Some(ResolvedAnchor::Point { pos, .. }) => Some(*pos),
            _ => None,
        }
    }

    /// The AABB of the assembled piece carrying `cell` in `area_id` — "the room
    /// this cell was authored inside". Falls back to the whole area's bounds when
    /// the cell sits in no single piece box (defensive; a single-prefab area has
    /// exactly one piece == the area), and to the degenerate `(cell, cell)` when
    /// the area is not placed at all.
    ///
    /// The confinement boundary for anything that must not silently leave the
    /// piece it was declared in: wave seating
    /// ([`crate::nav::World::confined_standable_cells`]) and anchor seating
    /// ([`crate::nav::AnchorRoot`]).
    pub fn piece_bounds(&self, area_id: &str, cell: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let Some(area) = self.areas.iter().find(|a| a.area_id == area_id) else {
            return (cell, cell);
        };
        for piece in &area.pieces {
            let (lo, hi) = piece.bbox();
            if (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i]) {
                return (lo, hi);
            }
        }
        area.bounds()
    }

    /// Resolve an anchor **by name alone**, across areas — the area-agnostic
    /// lookup `open-gate` / `move-npc` destinations / actor spawns already use.
    /// `Point` yields its cell, `Gate` its `from` corner; `None` when no placed
    /// piece provides the name. First match in `anchors` order (a `BTreeMap`, so
    /// deterministic).
    pub fn point_any(&self, anchor: &str) -> Option<[i32; 3]> {
        point_any_in(&self.anchors, anchor)
    }

    /// Resolve an anchor-centred box (spec-0022) to absolute inclusive corners:
    /// `anchor ± extent`, the same shape `begin-stealth` zones and
    /// `damage-players`'s `in` filter use. `None` when no placed piece provides
    /// the anchor.
    ///
    /// This — not a prefab `region` anchor — is how the trap-payload verbs
    /// describe a volume, because [`crate::assembled`] unconditionally CLEARS
    /// every `ResolvedAnchor::Gate` region from the assembled world. A `collapse`
    /// ceiling declared as a region anchor would be deleted at build time, and a
    /// `volley` kill zone would silently punch a hole in the geometry it names.
    pub fn zone_box(&self, zone: &delvewright_dsl::StealthZone) -> Option<([i32; 3], [i32; 3])> {
        zone_box_in(&self.anchors, zone)
    }

    /// Whether any collected checkpoint carries an `on_respawn` hook — gates the
    /// vanilla respawn-detection machinery so checkpoint-free / hook-free campaigns
    /// stay byte-identical (DSL v0.6, spec-0012).
    pub fn any_checkpoint_on_respawn(&self) -> bool {
        self.checkpoints.iter().any(|c| !c.on_respawn.is_empty())
            || !self.reseat_waves().is_empty()
            || !self.undefeated_reseat_waves().is_empty()
            || !self.reseat_actors().is_empty()
    }

    /// Whether the campaign declares **any** checkpoint at all (spec-0012 /
    /// spec-0016 §1). Gates the respawn **re-seat** machinery: the delve's own
    /// promise is "die and resume at the last checkpoint", and vanilla's
    /// `/spawnpoint` is only a hint — it silently falls back to the world spawn
    /// whenever the recorded cell is not a legal respawn position.
    /// A campaign with no checkpoint keeps the pre-0.6 emission byte-for-byte.
    pub fn any_checkpoint(&self) -> bool {
        !self.checkpoints.is_empty()
    }

    /// The campaign's `on_death` bundle (DSL v0.10, spec-0031) — effect root R7,
    /// the effects that run at the moment a player dies. Empty for every campaign
    /// below 0.10.0 and for any that declares no death beat, which is what keeps
    /// the whole corpse-side half of the death edge out of their emission.
    ///
    /// Read straight off the campaign rather than planned into a field: unlike a
    /// checkpoint or a shortcut this bundle resolves no geometry, so a planning
    /// step would only be a second place for it to go stale.
    pub fn on_death(&self) -> &[QuestEffect] {
        &self.campaign.quests.content.on_death
    }

    /// The waves a bonfire rest / bonfire respawn re-seats (spec-0016 §1), in
    /// content order. Empty unless the campaign declares BOTH a `bonfire` and at
    /// least one wave with `respawns_on_rest` — `DW0370` rejects the half that
    /// declares the field without a bonfire, so this is empty exactly for
    /// campaigns that use none of the surface (byte-identical emission).
    pub fn reseat_waves(&self) -> Vec<&delvewright_dsl::Wave> {
        if !self.checkpoints.iter().any(|c| c.rest) {
            return Vec::new();
        }
        self.campaign
            .quests
            .content
            .waves
            .iter()
            .filter(|w| w.respawns_on_rest)
            .collect()
    }

    /// The waves a bonfire refreshes **only while they are undefeated**
    /// (spec-0016 §1): every `elite`/`boss`-tier wave
    /// that does NOT declare `respawns_on_rest`, in content order.
    ///
    /// The distinction from [`Self::reseat_waves`] is the whole ruling. A
    /// `respawns_on_rest` wave comes back *whether or not* the party beat it —
    /// the fire is not a progress ratchet. A billed elite/boss does not: beat it
    /// and it stays beaten (spec-0016 §1, "stage bosses never respawn on rest").
    /// But while it is still standing, chipping it down one hit per life is never
    /// a valid path, so a rest wipes what is left of it and re-seats the authored
    /// wave at full count and full health. The two sets are disjoint by
    /// construction here, so no wave can be re-seated twice by one rest;
    /// `DW0499` forbids the `boss` + `respawns_on_rest` combination outright.
    ///
    /// Empty without a bonfire, and empty for every campaign that bills no
    /// encounter → byte-identical emission.
    pub fn undefeated_reseat_waves(&self) -> Vec<&delvewright_dsl::Wave> {
        if !self.checkpoints.iter().any(|c| c.rest) {
            return Vec::new();
        }
        self.campaign
            .quests
            .content
            .waves
            .iter()
            .filter(|w| !w.respawns_on_rest)
            .filter(|w| {
                w.tier
                    .is_some_and(delvewright_dsl::EncounterTier::has_floor_expectation)
            })
            .collect()
    }

    /// The actors a bonfire refreshes while they are undefeated (spec-0016 §1),
    /// in declaration order: every actor the campaign
    /// `unleash-actor`s — the compiler's one definition of an actor that is a
    /// *fight* ([`crate::combat::hostile_actors`]).
    ///
    /// Empty without a bonfire, and empty for every campaign whose actors are all
    /// scenery → byte-identical emission.
    pub fn reseat_actors(&self) -> Vec<&delvewright_dsl::Actor> {
        if !self.checkpoints.iter().any(|c| c.rest) {
            return Vec::new();
        }
        crate::combat::hostile_actors(self.campaign)
    }

    /// The collected checkpoint matching a `set-checkpoint` effect (by anchor +
    /// `on_respawn` list), giving the emitter its stable content-ordered index.
    pub fn checkpoint_for(
        &self,
        anchor: &str,
        on_respawn: &[QuestEffect],
    ) -> Option<&CheckpointPlan> {
        self.checkpoints
            .iter()
            .find(|c| !c.rest && c.anchor == anchor && c.on_respawn.as_slice() == on_respawn)
    }

    /// The collected **bonfire** matching a `bonfire` effect (by anchor +
    /// `on_rest` list), giving the emitter its stable content-ordered index
    /// (spec-0016 §1). Disjoint from [`Self::checkpoint_for`]: a bonfire and a
    /// plain `set-checkpoint` may share an anchor and a hook list and still be
    /// two distinct rest points.
    pub fn bonfire_for(&self, anchor: &str, on_rest: &[QuestEffect]) -> Option<&CheckpointPlan> {
        self.checkpoints
            .iter()
            .find(|c| c.rest && c.anchor == anchor && c.on_respawn.as_slice() == on_rest)
    }

    /// Every collected bonfire (spec-0016 §1), content-ordered.
    pub fn bonfires(&self) -> impl Iterator<Item = &CheckpointPlan> {
        self.checkpoints.iter().filter(|c| c.rest)
    }

    /// **Every trigger this build emits**: the campaign's own, in declaration
    /// order, then the compiler's press answers ([`PressAnswer`]).
    ///
    /// This is the emission-side counterpart of `QuestsContent::all_triggers` (the
    /// authority an `ambush` desugars into), and it exists for the same reason:
    /// there must be exactly one list, or the sugar acquires a second code path to
    /// drift down. Every place that gives a click a body, a tick clause, a
    /// function, an advancement or a rider tag reads this — so a press answer is
    /// emitted by the code that emits author triggers, and cannot be given a
    /// dialect of its own.
    ///
    /// **Why the press answers are added here rather than in `parse_campaign`**
    /// (where the `ambush` sugar expands). An ambush's strings are the author's
    /// and belong in the campaign's l10n inventory under the desugared trigger's
    /// keys; a press answer's are not. An authored `sealed_hint` is already
    /// inventoried at `fx.….sealed_hint`, and expanding at parse time would move
    /// that key and orphan every sidecar that has it; the compiler's own default
    /// is **chrome**, which must never enter a campaign's inventory at all. The
    /// key contract is a property of the authored document, so the desugar happens
    /// one layer below it — after `localize`, before emission.
    ///
    /// Deterministic: two fixed orders concatenated, no hashing (ADR-0006).
    pub fn emitted_triggers(&self, chrome: &delvewright_dsl::Chrome) -> Vec<EnvTrigger> {
        let mut out = self.campaign.quests.content.triggers.clone();
        out.extend(self.press_answers.iter().map(|p| p.trigger(chrome)));
        out
    }

    /// [`Self::emitted_triggers`] for a consumer that asks *which triggers exist,
    /// where, and of what kind* and never reads what they say — the hitbox proofs
    /// (`DW0422`/`DW0426`) and the affordance ledger.
    ///
    /// The build language only ever decides which rendition of a **chrome**
    /// string rides a component as its fallback, so a body question cannot depend
    /// on it. Spelling that out here is what keeps those consumers from having to
    /// thread a `Chrome` they would not use.
    pub fn emitted_triggers_unlocalized(&self) -> Vec<EnvTrigger> {
        self.emitted_triggers(&delvewright_dsl::Chrome::default())
    }

    /// For each [`Self::checkpoints`] entry, in the same order: the step at which
    /// it stops being where a dead player lands, or `None` for "never".
    ///
    /// A plain `set-checkpoint` is **monotonic** (spec-0012): the next one to fire
    /// replaces it outright, so its reign is `[fire_step, next_set_checkpoint)`.
    /// A later **bonfire** does not end it — the checkpoint moves only when the
    /// party actually rests, and nobody is forced to (the same "an unguaranteed
    /// firing may be assumed only where assuming so is conservative" rule
    /// [`collect_region_events`] states).
    ///
    /// A **bonfire**'s reign is `None`: the party can return to the last fire they
    /// rested at for the rest of the campaign, and a proof over it must not narrow
    /// on the strength of a rest that might never happen. That is also exactly
    /// today's behaviour, so every bonfire proof is unchanged by this existing.
    pub fn respawn_reign_ends(&self) -> Vec<Option<usize>> {
        let later_plain: Vec<usize> = self
            .checkpoints
            .iter()
            .filter(|c| !c.rest)
            .map(|c| c.fire_step)
            .collect();
        self.checkpoints
            .iter()
            .map(|c| {
                if c.rest {
                    return None;
                }
                later_plain
                    .iter()
                    .copied()
                    .filter(|s| *s > c.fire_step)
                    .min()
            })
            .collect()
    }

    /// The earliest critical-path step at which each hostile force can be in the
    /// world, keyed by wave id and actor id.
    ///
    /// Read off the campaign's own staging beats — `spawn-wave` and
    /// `spawn-actor` — through the one effect walk, so a new root or a new
    /// nesting site cannot leave a body invisible here. A force no beat stages,
    /// or one staged from a root with no step of its own (a trigger, a trap
    /// payload, a death bundle), reports **0**: the conservative answer is "it
    /// could be there from the start", and a proof that guessed later would be a
    /// proof that looked away.
    ///
    /// `unleash-actor` is deliberately NOT an onset. It does not put a body in
    /// the world — it replaces an already-summoned puppet with a real-AI twin, and
    /// an unleash of something never spawned is a no-op ([`crate::combat`]'s
    /// "unleash or nothing" rule states the same fact from the other side).
    /// Counting it moved `nobodys-cave-island`'s warden onto step 0 because a
    /// proximity trigger unleashes it, and reported a body five quests in the
    /// future as standing two blocks from the party's respawn.
    pub fn hostile_onsets(&self) -> BTreeMap<String, usize> {
        let mut out: BTreeMap<String, usize> = BTreeMap::new();
        let mut note = |id: &str, step: usize| {
            let slot = out.entry(id.to_string()).or_insert(step);
            *slot = (*slot).min(step);
        };
        delvewright_dsl::for_each_campaign_effect(self.campaign, &mut |_, site, eff| {
            let step = match site {
                delvewright_dsl::EffectSite::Objective { objective, .. } => self
                    .objective_steps
                    .get(objective.as_str())
                    .copied()
                    .unwrap_or(0),
                delvewright_dsl::EffectSite::QuestComplete { quest } => self
                    .campaign
                    .quests
                    .content
                    .quests
                    .iter()
                    .find(|q| q.id.as_str() == quest)
                    .map_or(0, |q| quest_complete_step(q, &self.objective_steps)),
                // Roots with no beat of their own: proximity, a sprung trap, a
                // death, a shortcut bar lifting, a shop purchase, a dialogue
                // respawn hook. Rooted at 0, conservatively.
                _ => 0,
            };
            match eff {
                QuestEffect::SpawnWave { wave, .. } => note(wave.as_str(), step),
                QuestEffect::SpawnActor { actor, .. } => note(actor.as_str(), step),
                _ => {}
            }
        });
        out
    }

    /// Translate a [`Self::critical_path`] index into the index the SAME step
    /// carries in the **exported** `critical-path.json`.
    ///
    /// Two coordinate systems came into existence the moment spec-0016 §1's rest
    /// splice landed: `critical_path` is the compiler's own list — what every
    /// `CheckpointPlan::fire_step`, every nav proof and every internal index
    /// means — while the exported path additionally carries one `rest` step
    /// after the beat that arms each bonfire. They drift by exactly one per
    /// bonfire armed strictly earlier, and a consumer that mixed them read the
    /// wrong step (the combat plan's `step` claimed to be a `critical-path.json`
    /// index while being a `critical_path` one).
    ///
    /// **Every artifact a harness reads states EXPORTED coordinates**, and this
    /// is where that translation lives for the MAIN path.
    ///
    /// **Scope — the main `critical-path.json` only.** spec-0025's per-branch
    /// paths are a different *sequence* of the same steps, so an index cannot be
    /// carried across at all; `emit::rest_step_index` is the general translation
    /// and goes through the **objective** the arming beat names, because a fire
    /// is armed by a beat rather than by a position. On the main path that
    /// translation is the identity (an objective appears at exactly one step),
    /// which is precisely what makes the count below correct here and nowhere
    /// else. A branch-path consumer must use `rest_step_index`, never this.
    ///
    /// The arithmetic mirrors `emit::with_bonfire_rest_steps` by construction —
    /// a rest for bonfire `b` is pushed after the step at `b.fire_step`, so a
    /// step at index `i` is preceded by one rest per bonfire with
    /// `fire_step < i`. That agreement is not left to inspection:
    /// `the_combat_plan_step_indexes_the_exported_path` pins the two together
    /// against the real emitted documents (the step the plan points at must BE
    /// the encounter's kill), so a future change to the splice fails the test
    /// rather than silently desynchronising this.
    ///
    /// Identity for a campaign with no bonfire.
    pub fn exported_step(&self, step: usize) -> usize {
        step + self.bonfires().filter(|b| b.fire_step < step).count()
    }

    /// Every class-kit **flask** (DSL v0.8, spec-0016 §1): `(class index, kit
    /// index)` pairs in declaration order — the recovery stacks a bonfire rest
    /// replenishes to their declared `count`. Empty for a campaign that declares
    /// none, which is exactly the campaigns whose emission stays byte-identical
    /// (`DW0476` guarantees a bonfire campaign is never in that set).
    pub fn flasks(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (i, class) in self.campaign.classes.content.classes.iter().enumerate() {
            for (k, item) in class.kit.iter().enumerate() {
                if item.flask {
                    out.push((i, k));
                }
            }
        }
        out
    }

    /// The collected stealth beat matching a `begin-stealth` effect (by zone
    /// anchors + `grace_ticks`), giving the emitter its 1-based session id.
    pub fn stealth_for(
        &self,
        zones: &[delvewright_dsl::StealthZone],
        grace: u32,
    ) -> Option<&StealthBeat> {
        self.stealth_beats.iter().find(|b| {
            b.grace_ticks == grace
                && b.zones.len() == zones.len()
                && b.zones
                    .iter()
                    .zip(zones)
                    .all(|((a, _, e), z)| a.as_str() == z.anchor.as_str() && *e == z.extent)
        })
    }
}

/// **Every place the campaign says the party has to reach**, resolved
/// (spec-0042 §2.5), for the way-disposition gate to judge against a piece's
/// contract.
///
/// The set is [`required_anchors_for_area`]'s — the anchors the layout solver is
/// already required to guarantee, which is this engine's existing definition of
/// "a campaign reference to a place": NPC stands, `reach-anchor` / `collect` /
/// `interact` targets, wave spawns and lane waypoints, every anchor-bearing
/// effect. Re-deriving a narrower list here would be a second enumeration that
/// drifts; a wider one would judge places nobody is required to visit.
///
/// An element carries the step it must be reachable BY when the campaign orders
/// it — an objective's own critical-path step. Everything else carries none and
/// is judged on the weaker half alone: it must be behind a way something forces
/// open, in no particular order. That asymmetry is the honest one — a body placed
/// at world load has no step, and inventing one would order a thing the campaign
/// never ordered.
fn collect_required_elements(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    objective_steps: &BTreeMap<String, usize>,
) -> Vec<crate::ways::RequiredElement> {
    // anchor name → the earliest objective that targets it, with its step. The
    // earliest, because an anchor two objectives share must be reachable by the
    // time the FIRST of them asks the party to stand there.
    let mut by_objective: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
    for quest in &campaign.quests.content.quests {
        for objective in &quest.objectives {
            let anchor = match objective {
                Objective::ReachAnchor { anchor, .. }
                | Objective::Collect { anchor, .. }
                | Objective::Interact { anchor, .. } => anchor.as_str(),
                Objective::Kill { .. } | Objective::TalkTo { .. } => continue,
            };
            let Some(step) = objective_steps.get(objective.id().as_str()).copied() else {
                continue;
            };
            let entry = by_objective
                .entry(anchor)
                .or_insert((objective.id().as_str(), step));
            if step < entry.1 {
                *entry = (objective.id().as_str(), step);
            }
        }
    }
    let mut out = Vec::new();
    for area in &campaign.world.content.areas {
        let area_id = area.id.as_str();
        for name in required_anchors_for_area(campaign, area_id) {
            let Some(resolved) = anchors.get(&(area_id.to_string(), name.clone())) else {
                continue;
            };
            let pos = match resolved {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            };
            let (what, by_step) = match by_objective.get(name.as_str()) {
                Some((oid, step)) => (
                    format!("objective `{oid}` (at anchor `{name}`)"),
                    Some(*step),
                ),
                None => (format!("the campaign reference to anchor `{name}`"), None),
            };
            out.push(crate::ways::RequiredElement {
                what,
                area_id: area_id.to_string(),
                pos,
                by_step,
            });
        }
    }
    out
}

/// The set of anchor names the campaign references inside `area_id`: NPC stands
/// (NPCs in this area), `reach-anchor` targets and `open-gate` anchors (quests
/// planned in this area). Sorted + deduped for deterministic solver input. These
/// are the anchors the solver must guarantee exist in the assembled layout.
fn required_anchors_for_area(campaign: &Campaign, area_id: &str) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for npc in &campaign.npcs.content.npcs {
        if npc.area.as_str() == area_id {
            set.insert(npc.anchor.as_str().to_string());
        }
    }
    // Which planned quests belong to this area.
    let quest_area: BTreeMap<&str, &str> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    for q in &campaign.quests.content.quests {
        if quest_area.get(q.id.as_str()).copied() != Some(area_id) {
            continue;
        }
        for o in &q.objectives {
            match o {
                Objective::ReachAnchor { anchor, .. } | Objective::Collect { anchor, .. } => {
                    set.insert(anchor.as_str().to_string());
                }
                Objective::Interact { anchor, .. } => {
                    set.insert(anchor.as_str().to_string());
                }
                // Wave spawn anchors are registered below via `wave_area`, driven
                // by the `spawn-wave` effect (the true spawn site) rather than the
                // `kill` objective — so a kill-less live-threat wave is placed too.
                Objective::Kill { .. } | Objective::TalkTo { .. } => {}
            }
            // v0.8: an adopted container is a piece of hardware the
            // objective cannot do without — a pool draw that omits its carrier
            // leaves the collect with nothing to fill, so it joins the required
            // set exactly as a lane waypoint does. Absent field adds nothing.
            if let Some(cont) = o.collect_container() {
                set.insert(cont.as_str().to_string());
            }
        }
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            collect_effect_anchors(e, &mut set);
        }
    }
    // Wave spawn anchors: a `spawn-wave` effect materializes its mobs at the wave's
    // `anchor` in the area of the quest (or single-area trigger) that fires it —
    // independent of any `kill` objective. Register the anchor for that area so the
    // solver guarantees a piece providing it; a kill-less live-threat wave would
    // otherwise resolve no spawn position and its `spawn_<wave>` call would dangle.
    for w in &campaign.quests.content.waves {
        if wave_area(campaign, w.id.as_str()) == Some(area_id) {
            set.insert(w.anchor.as_str().to_string());
            // spec-0016 §6: a TD lane's waypoints are places the squad has to
            // reach, so the solver must guarantee a piece providing each one —
            // exactly like the wave's own spawn anchor. Without this a pool area
            // simply may not draw the piece carrying a waypoint, and the lane
            // fails DW0386 ("resolves nowhere") for a reason the author cannot
            // act on: the anchor IS in the pool, the layout just did not use it.
            if let Some(lane) = &w.lane {
                set.extend(lane.waypoints.iter().map(|a| a.as_str().to_string()));
            }
        }
    }
    // Environment triggers (v0.4) are global. When the campaign has a single area,
    // their `at` and effect anchors must be provided by that area's assembly. For
    // a multi-area campaign, a trigger anchor is expected to coincide with an
    // objective anchor (already required above); over-provisioning every area is
    // avoided so the solver is not asked for an anchor an area's pool cannot fit.
    if campaign.world.content.areas.len() == 1 {
        for t in &campaign.quests.content.triggers {
            if let Some(at) = t.at_anchor() {
                set.insert(at.to_string());
            }
            for e in &t.effects {
                collect_effect_anchors(e, &mut set);
            }
        }
    }
    set.into_iter().collect()
}

/// Collect the anchors a v0.4 quest effect references (`open-gate`, `set-block`,
/// `move-npc` target, `cutscene` waypoints) into `set`, so the layout solver
/// guarantees they exist in the assembled area.
fn collect_effect_anchors(e: &QuestEffect, set: &mut BTreeSet<String>) {
    if let Some(a) = e.open_gate_anchor() {
        set.insert(a.as_str().to_string());
    }
    if let Some(a) = e.close_gate_anchor() {
        set.insert(a.as_str().to_string());
    }
    if let Some((a, _)) = e.set_block() {
        set.insert(a.as_str().to_string());
    }
    if let Some((_, a)) = e.move_npc() {
        set.insert(a.as_str().to_string());
    }
    // Every shot's waypoints, plus each shot's `look_at` subject — the camera is
    // aimed at that world point, so the area's assembly must provide its anchor.
    if let Some(shots) = e.cutscene_shots() {
        for shot in &shots {
            for w in &shot.path {
                set.insert(w.anchor.as_str().to_string());
            }
            if let Some(t) = &shot.look_at {
                set.insert(t.anchor.as_str().to_string());
            }
        }
    }
}

/// Resolve a placed-piece anchor to absolute world coords (transforming through
/// the piece's pos + rotation).
fn resolve_piece_anchor(placed: &solver::PlacedPiece, am: &AnchorMeta) -> ResolvedAnchor {
    if let Some(region) = &am.region {
        ResolvedAnchor::Gate {
            from: solver::transform_point(placed, region.from),
            to: solver::transform_point(placed, region.to),
            block: am
                .block
                .clone()
                .unwrap_or_else(|| "minecraft:air".to_string()),
        }
    } else {
        ResolvedAnchor::Point {
            pos: solver::transform_point(placed, am.pos.unwrap_or([0, 0, 0])),
            facing: solver::transform_facing(placed, am.facing.as_deref()),
        }
    }
}

fn resolve_anchor(origin: [i32; 3], am: &AnchorMeta) -> ResolvedAnchor {
    let add = |p: [i32; 3]| [origin[0] + p[0], origin[1] + p[1], origin[2] + p[2]];
    if let Some(region) = &am.region {
        ResolvedAnchor::Gate {
            from: add(region.from),
            to: add(region.to),
            block: am
                .block
                .clone()
                .unwrap_or_else(|| "minecraft:air".to_string()),
        }
    } else {
        ResolvedAnchor::Point {
            pos: add(am.pos.unwrap_or([0, 0, 0])),
            facing: am.facing.clone(),
        }
    }
}

fn plan_npc(npc: &Npc, tree: &NpcDialogue) -> NpcPlan {
    let safe = safe_local(npc.id.as_str());
    let mut options = Vec::new();
    let mut n = 0;
    for node in &tree.nodes {
        for opt in &node.options {
            n += 1;
            let mut completes = Vec::new();
            let mut sets_flags = Vec::new();
            let mut sets_time = Vec::new();
            let mut sets_weather = Vec::new();
            let mut sets_checkpoints = Vec::new();
            let mut spawns_npcs = Vec::new();
            for e in &opt.effects {
                match e {
                    DialogueEffect::CompleteObjective { objective } => {
                        completes.push(objective.as_str().to_string());
                    }
                    DialogueEffect::SetFlag { flag } => {
                        sets_flags.push(flag.as_str().to_string());
                    }
                    DialogueEffect::SetTime { time } => sets_time.push(*time),
                    DialogueEffect::SetWeather { weather } => sets_weather.push(*weather),
                    DialogueEffect::SetCheckpoint { anchor, on_respawn } => {
                        sets_checkpoints.push((anchor.as_str().to_string(), on_respawn.clone()));
                    }
                    DialogueEffect::SpawnNpc { npc } => {
                        spawns_npcs.push(npc.as_str().to_string());
                    }
                }
            }
            options.push(OptionPlan {
                n,
                node_id: node.id.as_str().to_string(),
                label: opt.label.clone(),
                tooltip: opt.tooltip.clone(),
                next: opt
                    .next
                    .as_ref()
                    .map(|d: &DialogueId| d.as_str().to_string()),
                completes,
                sets_flags,
                requires_flags: opt
                    .requires_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
                forbids_flags: opt
                    .forbids_flags
                    .iter()
                    .map(|f| f.as_str().to_string())
                    .collect(),
                requires_state: opt.requires_state.clone(),
                sets_time,
                sets_weather,
                sets_checkpoints,
                spawns_npcs,
            });
        }
    }
    NpcPlan {
        npc_id: npc.id.as_str().to_string(),
        trigger_objective: dlg_trigger(npc.id.as_str()),
        tag: format!("dw_npc_{safe}"),
        root: tree.root.as_str().to_string(),
        safe,
        options,
    }
}

/// The computed critical path and its per-step metadata.
pub struct CriticalPath {
    pub steps: Vec<Step>,
    pub(crate) transport: TransportMap,
    pub transport_by_step: Vec<Option<[i32; 3]>>,
    pub sneak_by_step: Vec<bool>,
    pub cutscene_by_step: Vec<Option<u32>>,
    /// Objective id → its `critical_path` step index (v0.6): roots the checkpoint
    /// no-stranding proof (DW0315) and the stealth-zone reachability proof
    /// (DW0327) at the beat that fires the effect.
    pub(crate) obj_step: BTreeMap<String, usize>,
}

/// Build the critical path: select first class, then each objective of the
/// **flow-proven single-branch playthrough** ([`crate::flow::Flow::playthrough`])
/// in topological order (quests by `depends_on`, objectives by `after`), then
/// assert campaign completion. Quests that belong to a mutually exclusive branch
/// the chosen playthrough does not take are excluded, and each `talk-to` takes
/// the completing dialogue option that belongs to that branch — so the exported
/// path is a sequence one player can actually walk (proven by
/// `crate::flow::Flow::replay`, `DW0204`, before the build reaches here).
///
/// Also returns the inter-area transport map (when consecutive objectives sit in
/// different areas) and, per step, the DSL v0.4 harness hints: `sneak` (a
/// `stealth` objective) and `cutscene_seconds` (a step whose completion triggers
/// a `QuestEffect::Cutscene`).
fn build_critical_path(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    npcs: &[NpcPlan],
    flow: &crate::flow::Flow<'_>,
    path: &crate::flow::Playthrough,
) -> Result<CriticalPath, PlanError> {
    let mut steps = Vec::new();
    // (objective id, physical area, step index) in critical-path order, for the
    // transport map and the per-step transport marker.
    let mut obj_areas: Vec<(String, String, usize)> = Vec::new();

    // select-class: first declared class.
    if let Some(first) = campaign.classes.content.classes.first() {
        steps.push(Step::SelectClass {
            class_id: first.id.as_str().to_string(),
            command: "/trigger dw.class set 1".to_string(),
        });
    }

    // The branch-coherent playthrough: one world's completing quests in
    // `depends_on` order, their objectives in `after` order, and the dialogue
    // option each `talk-to` takes on that branch. Supplied by the caller so the
    // same builder serves the exported path (the default playthrough) and the
    // spec-0025 per-branch paths (the playthrough of the world realizing a
    // branch) — one code path, so a branch run walks steps built exactly like
    // the ones the ladder has always walked.
    if path.cyclic {
        return Err(PlanError::new(
            DW_BUILD,
            "internal invariant violation: a quest dependency cycle survived into critical-path \
             ordering — `DW0130` should have rejected it in validation. This is a compiler bug; \
             stop and escalate",
        ));
    }
    let stage5: BTreeMap<&str, &_> = campaign
        .quests
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q))
        .collect();

    // The flag state the party holds as it walks up to each step, and the quests
    // this playthrough ever activates. Together they are what selects a `talk-to`
    // NPC's cast row — the same journal `crate::branch`'s `DW0483` reads, so the
    // placement the ladder walks to and the placement the proofs check are chosen
    // by ONE model (see [`crate::cast::station`]).
    let flags_at: Vec<BTreeSet<String>> = flow
        .journal(path)
        .into_iter()
        .map(|s| s.flags_before)
        .collect();
    let begun: BTreeSet<String> = path.quests.iter().cloned().collect();

    for (si, st) in path.steps.iter().enumerate() {
        let qid = st.quest.as_str();
        let Some(quest) = stage5.get(qid) else {
            continue;
        };
        let area = campaign
            .quest_plan
            .content
            .quests
            .iter()
            .find(|q| q.id.as_str() == qid)
            .map(|q| q.area.as_str())
            .unwrap_or("");
        let Some(obj) = quest
            .objectives
            .iter()
            .find(|o| o.id().as_str() == st.objective)
        else {
            continue;
        };
        {
            match obj {
                Objective::TalkTo { id, npc, .. } => {
                    let npc_plan =
                        npcs.iter()
                            .find(|n| n.npc_id == npc.as_str())
                            .ok_or_else(|| {
                                PlanError::new(
                                    DW_BUILD,
                                    format!(
                                        "internal invariant violation: `talk-to` references npc \
                                         `{npc}` with no build-time plan — `DW0112`/`DW0152` \
                                         should have caught this in validation. This is a compiler \
                                         bug; stop and escalate"
                                    ),
                                )
                            })?;
                    // The branch-consistent completing option (the flow model
                    // picked it); fall back to the first completing option only
                    // for a campaign with no branch at all.
                    let opt = st
                        .talk_option
                        .and_then(|n| npc_plan.options.iter().find(|o| o.n as usize == n))
                        .or_else(|| {
                            npc_plan
                                .options
                                .iter()
                                .find(|o| o.completes.iter().any(|c| c == id.as_str()))
                        })
                        .ok_or_else(|| {
                            PlanError::new(DW_BUILD, format!(
                                "internal invariant violation: objective `{id}` has no dialogue \
                                 option completing it at build time — `DW0123`/`DW0203` should \
                                 have caught this in validation/analysis. This is a compiler bug; \
                                 stop and escalate"
                            ))
                        })?;
                    // NPC position: where the CAST LEDGER stations the body for
                    // THIS beat, on THIS path — not the stage-2 anchor.
                    //
                    // The stage-2 anchor is only where the NPC is first summoned;
                    // a `move-npc` walks him away from it and the ledger records
                    // where he then stands (`DW0461` proves the record equals the
                    // effect history). Reading the anchor here made the bot
                    // contract a second, staler source of truth: on the island,
                    // `npc/perimedes` is declared at `anchor/mouth` and cast at
                    // `anchor/alcove-2` for his stone beat, and the eye-ray bot
                    // walked to the mouth — where the sealed boulder region's
                    // wall of interaction entities stands — and could not acquire
                    // him. The emitted cast was right the whole time.
                    let decl = campaign
                        .npcs
                        .content
                        .npcs
                        .iter()
                        .find(|nn| nn.id.as_str() == npc.as_str());
                    let home_area = decl.map(|nn| nn.area.as_str()).unwrap_or(area);
                    let (npc_area, pos) = match crate::cast::station(
                        campaign,
                        npc.as_str(),
                        qid,
                        &begun,
                        flags_at.get(si).unwrap_or(&BTreeSet::new()),
                    ) {
                        Some(crate::cast::Station::At(anchor)) => {
                            cast_point(anchors, home_area, anchor).ok_or_else(|| {
                                PlanError::new(
                                    DW_BUILD,
                                    format!(
                                        "internal invariant violation: quest `{qid}` casts npc \
                                     `{npc}` at `{anchor}`, which resolves to no world position \
                                     at build time — `DW0464` (dangling cast anchor) / `DW0142` \
                                     should have named it in validation. This is a compiler bug; \
                                     stop and escalate"
                                    ),
                                )
                            })?
                        }
                        Some(crate::cast::Station::Absent(kind)) => {
                            return Err(PlanError::new(
                                DW_BUILD,
                                format!(
                                    "internal invariant violation: `talk-to` objective `{id}` needs a \
                                 body to click, but quest `{qid}`'s cast ledger declares npc \
                                 `{npc}` `\"{}\"` for this beat — `DW0195` (talk-to on an NPC a \
                                 prerequisite despawned) / `DW0461` (a declared absence that \
                                 contradicts the effect history) should have refused this in \
                                 validation. This is a compiler bug; stop and escalate",
                                    kind.token()
                                ),
                            ));
                        }
                        // No ledger row anywhere up to this beat: a pre-0.7
                        // campaign. Keep the stage-2 anchor, byte for byte.
                        None => {
                            let anchor = decl.map(|nn| nn.anchor.as_str()).unwrap_or("");
                            (home_area.to_string(), point_of(anchors, home_area, anchor)?)
                        }
                    };
                    steps.push(Step::TalkTo {
                        objective_id: id.as_str().to_string(),
                        npc_id: npc.as_str().to_string(),
                        pos,
                        command: format!("/trigger {} set {}", npc_plan.trigger_objective, opt.n),
                    });
                    obj_areas.push((id.as_str().to_string(), npc_area, steps.len() - 1));
                }
                Objective::ReachAnchor {
                    id, anchor, radius, ..
                } => {
                    let pos = point_of(anchors, area, anchor.as_str())?;
                    steps.push(Step::Reach {
                        objective_id: id.as_str().to_string(),
                        anchor_id: anchor.as_str().to_string(),
                        pos,
                        radius: *radius,
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Kill { id, wave, .. } => {
                    let w = wave_of(campaign, wave.as_str()).ok_or_else(|| {
                        PlanError::new(
                            DW_BUILD,
                            format!(
                                "internal invariant violation: `kill` objective references wave \
                                 `{wave}` with no declaration at build time — `DW0170` should have \
                                 caught this in validation. This is a compiler bug; stop and \
                                 escalate"
                            ),
                        )
                    })?;
                    let pos = point_of(anchors, area, w.anchor.as_str())?;
                    steps.push(Step::Kill {
                        objective_id: id.as_str().to_string(),
                        wave_id: wave.as_str().to_string(),
                        pos,
                        tag: wave_tag(wave.as_str()),
                        count: wave_total(w),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Collect {
                    id,
                    item,
                    count,
                    anchor,
                    container,
                    dropped_by,
                    ..
                } => {
                    // The step position is the CONTAINER the bot opens: the
                    // adopted prefab chest/barrel when the objective declares one
                    // (DSL v0.8), else the chest the compiler places at `anchor`.
                    // The harness walks to this cell and opens the block standing
                    // there, so pointing it at the objective anchor while the items
                    // sit in a barrel three blocks away is a guaranteed bot stall.
                    // An unresolvable container anchor falls back to the objective
                    // anchor; the DSL tier reports it (`DW0142`).
                    // v0.9: a drop-gated collect has no container at
                    // all — the item is on the floor the wave died on, so the
                    // step points at that wave's own anchor.
                    let dropped_at = dropped_by.as_ref().and_then(|w| {
                        campaign
                            .quests
                            .content
                            .waves
                            .iter()
                            .find(|wv| wv.id.as_str() == w.as_str())
                            .and_then(|wv| point_any(anchors, wv.anchor.as_str()))
                    });
                    let pos = match dropped_at.or_else(|| {
                        container
                            .as_ref()
                            .and_then(|cont| point_any(anchors, cont.as_str()))
                    }) {
                        Some(cell) => cell,
                        None => point_of(anchors, area, anchor.as_str())?,
                    };
                    steps.push(Step::Collect {
                        objective_id: id.as_str().to_string(),
                        item: item.clone(),
                        count: *count as i32,
                        pos,
                        dropped: dropped_by.as_ref().map(|w| w.as_str().to_string()),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
                Objective::Interact {
                    id,
                    anchor,
                    requires_item,
                    ..
                } => {
                    let pos = point_of(anchors, area, anchor.as_str())?;
                    steps.push(Step::Interact {
                        objective_id: id.as_str().to_string(),
                        anchor_id: anchor.as_str().to_string(),
                        pos,
                        command: format!("/trigger {} set 1", interact_trigger(id.as_str())),
                        requires_item: requires_item.clone(),
                    });
                    obj_areas.push((id.as_str().to_string(), area.to_string(), steps.len() - 1));
                }
            }
        }
    }

    steps.push(Step::AssertComplete {
        objective: "dw.campaign".to_string(),
        value: 1,
    });

    // Transport: when consecutive critical objectives change area, completing the
    // earlier objective teleports the player to the later area's entry spawn.
    let mut transport: BTreeMap<String, [i32; 3]> = BTreeMap::new();
    // Per-step transport marker, aligned with `steps`. Filled from `transport` via
    // each objective's recorded step index (gap 8).
    let mut transport_by_step: Vec<Option<[i32; 3]>> = vec![None; steps.len()];
    for pair in obj_areas.windows(2) {
        let (prev_id, prev_area, prev_idx) = &pair[0];
        let (_, next_area, _) = &pair[1];
        if prev_area != next_area
            && let Some(ResolvedAnchor::Point { pos, .. }) =
                anchors.get(&(next_area.clone(), "spawn".to_string()))
        {
            transport.insert(prev_id.clone(), *pos);
            transport_by_step[*prev_idx] = Some(*pos);
        }
    }

    // DSL v0.4 per-step harness hints: `sneak` (a stealth objective) and
    // `cutscene_seconds` (a step whose completion fires a `Cutscene` effect).
    let mut sneak_by_step = vec![false; steps.len()];
    let mut cutscene_by_step: Vec<Option<u32>> = vec![None; steps.len()];
    for (obj_id, _, step_idx) in &obj_areas {
        if let Some((qid, obj)) = objective_quest(campaign, obj_id) {
            sneak_by_step[*step_idx] = obj.stealth();
            let mut secs = cutscene_seconds_in(objective_effects(campaign, obj_id).into_iter());
            if secs.is_none()
                && is_last_objective_of_quest(campaign, qid, obj_id)
                && let Some(q) = campaign
                    .quests
                    .content
                    .quests
                    .iter()
                    .find(|q| q.id.as_str() == qid)
            {
                secs = cutscene_seconds_in(q.on_complete.iter());
            }
            cutscene_by_step[*step_idx] = secs;
        }
    }

    let obj_step: BTreeMap<String, usize> = obj_areas
        .iter()
        .map(|(id, _, idx)| (id.clone(), *idx))
        .collect();

    Ok(CriticalPath {
        steps,
        transport,
        transport_by_step,
        sneak_by_step,
        cutscene_by_step,
        obj_step,
    })
}

/// Resolve a **cast-ledger** anchor to `(area, cell)`: the NPC's own area first,
/// then by name across areas.
///
/// The two-step lookup is [`crate::crosshair`]'s, over the same ledger and for
/// the same reason: a `move-npc` may station a body in an area the NPC was never
/// declared in, and the ledger is allowed to say so. Returning the area the
/// anchor actually resolved in — not the NPC's home area — is what keeps the
/// inter-area transport map coherent with the position the step now carries.
fn cast_point(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    home_area: &str,
    anchor: &str,
) -> Option<(String, [i32; 3])> {
    let cell = |r: &ResolvedAnchor| match r {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    };
    if let Some(r) = anchors.get(&(home_area.to_string(), anchor.to_string())) {
        return Some((home_area.to_string(), cell(r)));
    }
    anchors
        .iter()
        .find(|((_, n), _)| n == anchor)
        .map(|((a, _), r)| (a.clone(), cell(r)))
}

/// Resolve an anchor name to a point cell by scanning every area's resolved
/// anchors (first match), mirroring the emitter's `anchor_point_any`.
pub(crate) fn point_any(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    name: &str,
) -> Option<[i32; 3]> {
    for ((_, n), resolved) in anchors {
        if n == name {
            return match resolved {
                ResolvedAnchor::Point { pos, .. } => Some(*pos),
                ResolvedAnchor::Gate { from, .. } => Some(*from),
            };
        }
    }
    None
}

/// The `critical_path` step index at which a quest's `on_complete` fires: its
/// last objective's step (max over the quest's objectives). `0` if the quest has
/// no positioned objective (degenerate; conservative — proves the whole path).
fn quest_complete_step(quest: &Quest, obj_step: &BTreeMap<String, usize>) -> usize {
    quest
        .objectives
        .iter()
        .filter_map(|o| obj_step.get(o.id().as_str()).copied())
        .max()
        .unwrap_or(0)
}

/// The `critical_path` step index of the `talk-to` objective that a dialogue tree
/// belongs to (its NPC's completing beat), rooting a dialogue-hosted
/// `set-checkpoint`. `0` if none is found (degenerate).
fn dialogue_fire_step(
    campaign: &Campaign,
    npc_id: &str,
    obj_step: &BTreeMap<String, usize>,
) -> usize {
    campaign
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .filter_map(|o| match o {
            Objective::TalkTo { id, npc, .. } if npc.as_str() == npc_id => {
                obj_step.get(id.as_str()).copied()
            }
            _ => None,
        })
        .min()
        .unwrap_or(0)
}

/// Collect every `set-checkpoint` and `begin-stealth` effect (DSL v0.6) in a
/// deterministic content order, resolving each anchor to a cell and rooting it at
/// its firing step. An effect whose anchor does not resolve to a point is skipped
/// here (validation guarantees the anchor exists; a pool anchor that fails to
/// resolve at plan time simply carries no proof/emission).
fn collect_v06_effects(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    obj_step: &BTreeMap<String, usize>,
) -> (Vec<CheckpointPlan>, Vec<StealthBeat>) {
    let mut c = V06Collector {
        anchors,
        checkpoints: Vec::new(),
        stealth: Vec::new(),
        stealth_ends: Vec::new(),
    };

    // Stage 5 — quest effects (on_objective_complete, then on_complete).
    for q in &campaign.quests.content.quests {
        for (obj_id, effs) in &q.on_objective_complete {
            let step = obj_step.get(obj_id.as_str()).copied().unwrap_or(0);
            for eff in effs {
                c.handle(eff, step);
            }
        }
        let done_step = quest_complete_step(q, obj_step);
        for eff in &q.on_complete {
            c.handle(eff, done_step);
        }
    }

    // Stage 5 — environment triggers (conservative fire step 0: a trigger fires on
    // an environmental condition, not a critical beat, so require the checkpoint to
    // re-reach the whole remaining path).
    for t in &campaign.quests.content.triggers {
        for eff in &t.effects {
            c.handle(eff, 0);
        }
    }

    // Stage 6 — dialogue `set-checkpoint` (rooted at the NPC's talk-to beat).
    for tree in &campaign.dialogue.content.dialogues {
        let step = dialogue_fire_step(campaign, tree.npc.as_str(), obj_step);
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some((anchor, on_respawn)) = eff.set_checkpoint() {
                        c.push_checkpoint(anchor.as_str(), on_respawn, step, false, None);
                    }
                }
            }
        }
    }

    close_stealth_windows(&mut c.stealth, &c.stealth_ends);
    (c.checkpoints, c.stealth)
}

/// Close every beat's active window: a running session ends at the first
/// `end-stealth` fired after it, or when the next `begin-stealth` replaces it
/// (`#stealth dw.sys` holds ONE session id), whichever is earlier. A beat with
/// neither runs to the end of the campaign (`None`). Deterministic: driven by the
/// content-ordered collections only.
fn close_stealth_windows(beats: &mut [StealthBeat], ends: &[usize]) {
    let fires: Vec<usize> = beats.iter().map(|b| b.fire_step).collect();
    for (i, beat) in beats.iter_mut().enumerate() {
        let after = |s: &usize| *s > beat.fire_step;
        let first_end = ends.iter().filter(|s| after(s)).min().copied();
        let next_begin = fires.iter().skip(i + 1).filter(|s| after(s)).min().copied();
        beat.end_step = match (first_end, next_begin) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
}

/// Collect every stage-5 `shortcut` (spec-0016 §2) in declared order, resolving
/// its gate region and far-side unlock cell. A shortcut whose anchors do not
/// resolve is skipped here (validation owns that, `DW0371`).
fn collect_shortcuts(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<ShortcutPlan> {
    let mut out = Vec::new();
    for sc in &campaign.quests.content.shortcuts {
        let Some((from, to, block)) = gate_region_block_any(anchors, sc.gate.as_str()) else {
            continue;
        };
        let Some(unlock) = point_any(anchors, sc.unlock.as_str()) else {
            continue;
        };
        out.push(ShortcutPlan {
            id: sc.id.as_str().to_string(),
            safe: safe_local(sc.id.as_str()),
            gate_anchor: sc.gate.as_str().to_string(),
            gate_region: (from, to),
            gate_block: block,
            unlock_anchor: sc.unlock.as_str().to_string(),
            unlock,
            on_unlock: sc.on_unlock.clone(),
            // Which half of the doorway is the sealed one, from the
            // slab's thin axis and the side the unlock stands on. `None` is not
            // an error here — `emit` raises `DW0425` only if an answer was
            // actually authored for a side the geometry does not name.
            sealed_side: crate::wrongside::derive((from, to), unlock),
        });
    }
    out
}

/// Collect every stage-5 `ambush` (spec-0016 §3) in declared order, resolving the
/// trigger cell and each ambusher's spawn cell. An ambush whose trigger anchor or
/// whose every actor cell fails to resolve is skipped (the desugared trigger's own
/// anchor checks own that failure).
fn collect_ambushes(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<AmbushPlan> {
    let by_id: BTreeMap<&str, &delvewright_dsl::Actor> = campaign
        .quests
        .content
        .actors
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect();
    let mut out = Vec::new();
    for amb in &campaign.quests.content.ambushes {
        let Some(at) = point_any(anchors, amb.at.as_str()) else {
            continue;
        };
        let actor_cells: Vec<[i32; 3]> = amb
            .actors
            .iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .filter_map(|a| point_any(anchors, a.anchor.as_str()))
            .collect();
        out.push(AmbushPlan {
            id: amb.id.as_str().to_string(),
            at,
            actor_cells,
        });
    }
    out
}

/// Collect one [`SealHintPlan`] per gate anchor that any `close-gate` seals (DSL
/// v0.8), in first-firing order.
///
/// A repeat of an anchor already collected is dropped: the seal is a **place**,
/// so its hitboxes and its answer belong to the anchor, not to each firing. When
/// two firings disagree about the wording, `gates::check_seal_hints` (`DW0423`)
/// has already rejected the campaign — here the first-firing text wins.
///
/// A `close-gate` whose anchor is not a resolvable gate region carries no entry
/// (`DW0343` owns that).
fn collect_seal_hints(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<SealHintPlan> {
    let mut out: Vec<SealHintPlan> = Vec::new();
    for_each_gate_effect(campaign, &mut |_site, e| {
        let Some(anchor) = e.close_gate_anchor() else {
            return;
        };
        let name = anchor.as_str();
        if out.iter().any(|s| s.anchor == name) {
            return;
        }
        let Some((from, to, block)) = gate_region_block_any(anchors, name) else {
            return;
        };
        out.push(SealHintPlan {
            anchor: name.to_string(),
            safe: safe_local(name),
            region: (from, to),
            block,
            text: match e.close_gate_sealed_hint() {
                Some(h) => h.to_string(),
                None => delvewright_dsl::chrome::GATE_SEALED.tagged(),
            },
            authored: e.close_gate_sealed_hint().is_some(),
        });
    });
    out
}

/// The compiler-supplied answer one **pressable body** gives a right-click
/// (DSL v0.11).
///
/// ## Why this is not a field on a verb
///
/// `close-gate` owned `sealed_hint`: its own hitbox fleet, its own advancement,
/// its own actionbar reply, its own baked English. Every one of those is a
/// property of *being a thing a player can press*, and none of them has anything
/// to do with closing a gate — so the second object that needed them, a sealed
/// `shortcut` door, had no surface at all and answered a press with silence,
/// which is exactly the door a souls loop-back invites the party to push on.
/// CLAUDE.md's rule, on this precise case: *a second bespoke field is the
/// defect, not the fix*.
///
/// So a press answer is **not a mechanism**. It is an ordinary
/// [`EnvTrigger`]`{on: use, audience: presser}` carrying an ordinary
/// [`QuestEffect::Narrate`]`{style: actionbar}` — the general "click a thing, run
/// anything" verb, which since DSL v0.11 can reach both the channel and the
/// addressee that the private copy reached. This struct is the *sugar*: the wording
/// and the body it hangs on, lowered by [`PressAnswer::trigger`] into the one path
/// every author-written click already takes. There is no second emitter, no second
/// advancement shape, no second l10n rule and no second diagnostic family.
///
/// ## Lifetime is the body's lifetime
///
/// The synthesized trigger summons nothing: it **rides** the hitboxes the sealed
/// object already owns ([`crate::pressable::body_at`]). So a `close-gate` seal
/// answers exactly while it is sealed (`open-gate` kills `dw_seal_<safe>`), and a
/// shortcut door answers exactly until it is opened (`shortcut_open_<id>` kills
/// `dw_ws_<safe>`). A door that kept saying it cannot be opened after you opened
/// it would be worse than silence, and nothing has to remember not to do that:
/// there is no answer left to give once the thing you pressed is gone. That is
/// also why the shortcut needs no re-seal reasoning — `DW0372` forbids one.
#[derive(Clone, Debug)]
pub struct PressAnswer {
    /// The anchor of the body this answer hangs on.
    pub anchor: String,
    /// The full id of the trigger this lowers to (`trigger/dw-press-…`).
    pub trigger_id: String,
    /// What owns the body (`close-gate seal` / `shortcut door`), for diagnostics.
    pub owner: &'static str,
    /// The line, l10n-tagged: an authored `sealed_hint`'s campaign key, or the
    /// compiler's own `delvewright.ui.gate.sealed` chrome. Chrome is rebound to
    /// the build language at emission ([`delvewright_dsl::Chrome::rebind`]); an
    /// authored line passes through untouched and keeps its campaign key, so the
    /// l10n inventory is exactly what it was.
    pub text: String,
    /// Whether [`Self::text`] is the **campaign's** wording rather than the
    /// compiler's chrome fallback.
    ///
    /// This is the distinction the rule turns on. A `close-gate`
    /// with an authored `sealed_hint` has said what its seal says, and the
    /// compiler lowering that onto the general path is not the compiler putting
    /// words in a player's mouth. A `close-gate` with none has said nothing, and
    /// above the fence that is `DW0429` rather than `The way is sealed.`
    pub authored: bool,
}

/// The `trigger/<local>` id a press answer is synthesized under.
///
/// Two parts carry the collision argument. `dw-` is **reserved** from authored
/// trigger ids (`DW0428`), so a campaign can never write one of these; and
/// `<kind>` separates the two body classes, so a `close-gate` on `anchor/bell`
/// and a `shortcut/bell` — which share nothing but a local name — cannot land on
/// one id and silently become one answer.
fn press_answer_trigger_id(kind: &str, local: &str) -> String {
    format!("trigger/dw-press-{kind}-{local}")
}

/// The local part of an id (`anchor/bell` → `bell`), which is already kebab.
fn local_of(id: &str) -> &str {
    id.split_once('/').map(|(_, r)| r).unwrap_or(id)
}

impl PressAnswer {
    /// This answer lowered into the general verb: a repeatable right-click at the
    /// body's anchor that puts one line on the presser's actionbar.
    ///
    /// `chrome` resolves the compiler's own default into the build's language (a
    /// `--lang` bake ships no language files, so the component's fallback is what
    /// the player reads); an authored line is not chrome and is returned unchanged.
    pub fn trigger(&self, chrome: &delvewright_dsl::Chrome) -> EnvTrigger {
        EnvTrigger {
            id: delvewright_dsl::TriggerId(self.trigger_id.clone()),
            at: Some(delvewright_dsl::AnchorId(self.anchor.clone())),
            on: delvewright_dsl::TriggerOn::Use,
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
            // A wall is not consumed by being asked: it answers every press.
            once: false,
            audience: delvewright_dsl::TriggerAudience::Presser,
            effects: vec![QuestEffect::Narrate {
                text: chrome.rebind(&self.text),
                style: Some(delvewright_dsl::NarrateStyle::Actionbar),
                sound: None,
                requires_flags: Vec::new(),
                forbids_flags: Vec::new(),
                requires_state: Vec::new(),
            }],
        }
    }
}

/// **What happens when the campaign leaves a pressable body silent.**
///
/// The policy is a property of the **body class**, not of this function, so
/// extending an owner ruling from one class to another is a changed arm in
/// [`press_answer_sites`] rather than a re-architecture. The site that builds the
/// answers is shared; only this decides who supplies the wording.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SilencePolicy {
    /// **The campaign must author the wording** — every pressable body, from
    /// `dsl_version` 0.11.0.
    ///
    /// The compiler still *lowers* an authored wording onto the general path: a
    /// `close-gate`'s `sealed_hint` is an authored answer and is synthesized from
    /// as it always was. What it may not do is **invent** one. A body with neither
    /// an authored wording nor a `use` trigger is `DW0429`.
    ///
    /// The reasoning belongs in the code because it is the project's own rule
    /// arriving at a new site. A baked default is the compiler making a **design
    /// statement** — about tone, about what this specific thing is — on the
    /// author's behalf, and then never telling them it did. An error makes the
    /// author say it. Same rule as "no hacks at any layer": if content needs a
    /// thing, the DSL exposes it and the author declares it, rather than a lower
    /// layer inventing it.
    ///
    /// It is also the only end state where the docs, the code and the player
    /// agree. `wrongside.rs` and the reference both claimed for two versions that
    /// a shortcut door's wording "defaults", and no code defaulted anything: the
    /// door said nothing. The honest repair was never to make the claimed default
    /// real — it was to refuse to compile a body with no answer.
    Authored,
    /// **Grandfathered: a `close-gate` seal below 0.11.0.** The compiler falls
    /// back to its own `delvewright.ui.gate.sealed` chrome, exactly as it has
    /// since v0.8.
    Defaulted,
    /// **Grandfathered: a `shortcut` door below 0.11.0.** Nothing is emitted and
    /// nothing is demanded — the door is silent, byte for byte what it emitted
    /// before this version existed.
    Silent,
}

impl SilencePolicy {
    /// May the compiler word this body when the campaign has not?
    ///
    /// The two grandfathered arms differ from each other only because the two
    /// classes *historically* differed — a seal defaulted since v0.8, a door was
    /// silent — and preserving that is the whole point of a fence: **the same
    /// declared `dsl_version` yields the same verdicts and the same
    /// behaviour**. It is emphatically not a
    /// policy split by object class. Above the fence there is exactly one rule for
    /// every pressable body, which is what stops this from becoming the
    /// "capability keyed to the verb" defect CLAUDE.md's worked example is about —
    /// and this task IS that worked example.
    fn compiler_may_word_it(self) -> bool {
        matches!(self, SilencePolicy::Defaulted)
    }
}

/// The pressable bodies a press answer can hang on, each with its silence policy
/// — seals first, then shortcut doors, each in its own planner's order.
///
/// **This list is the class.** A third pressable object gets an answer by joining
/// it, not by growing a field on the verb that owns it.
fn press_answer_sites<'p>(
    seal_hints: &'p [SealHintPlan],
    shortcuts: &'p [ShortcutPlan],
    authored_required: bool,
) -> Vec<(PressAnswer, SilencePolicy)> {
    let mut out: Vec<(PressAnswer, SilencePolicy)> = seal_hints
        .iter()
        .map(|s| {
            (
                PressAnswer {
                    anchor: s.anchor.clone(),
                    trigger_id: press_answer_trigger_id("seal", local_of(&s.anchor)),
                    owner: "close-gate seal",
                    text: s.text.clone(),
                    authored: s.authored,
                },
                if authored_required {
                    SilencePolicy::Authored
                } else {
                    SilencePolicy::Defaulted
                },
            )
        })
        .collect();
    out.extend(shortcuts.iter().filter_map(|sc| {
        // A door whose sealed side the geometry does not name has no body to hang
        // an answer on; `emit::check_shortcut_sides` (`DW0425`) fails the build
        // before this could matter.
        sc.sealed_side.as_ref()?;
        Some((
            PressAnswer {
                anchor: sc.gate_anchor.clone(),
                trigger_id: press_answer_trigger_id("door", local_of(&sc.id)),
                owner: "shortcut door",
                // A shortcut carries no wording field, so there is never an
                // authored wording to lower: its answer is always a trigger.
                text: delvewright_dsl::chrome::GATE_SEALED.tagged(),
                authored: false,
            },
            if authored_required {
                SilencePolicy::Authored
            } else {
                SilencePolicy::Silent
            },
        ))
    }));
    out
}

/// **The silence-policy ledger**: every pressable body in the campaign, what owns
/// it, and who supplies its wording when the campaign says nothing.
///
/// CLAUDE.md: every validation artifact states its binding count. Here the count
/// that matters is not how many answers the compiler produced — that is zero
/// for a door, by design — but how many bodies
/// were **examined** and under which policy. A reader can see at a glance that
/// the door was considered and its wording withheld on purpose, rather than
/// missed.
pub fn press_answer_policies(plan: &Plan) -> Vec<(&'static str, String, SilencePolicy)> {
    press_answer_sites(
        &plan.seal_hints,
        &plan.shortcuts,
        delvewright_dsl::is_v11(plan.campaign.quests.dsl_version.as_str()),
    )
    .into_iter()
    .map(|(a, p)| (a.owner, a.anchor, p))
    .collect()
}

/// Collect the compiler's press answers: **one per pressable body whose class is
/// `Defaulted` and which the campaign does not answer itself**.
///
/// ## The rule, stated once
///
/// > A sealed body the campaign never answers is answered by the compiler —
/// > where, and only where, its class says the compiler may speak for it.
///
/// "The campaign answers it" is `QuestsContent::answers_press_at`, the one
/// predicate `DW0429` also reads, so the refusal and the synthesis can never
/// disagree about what counts as an answer.
fn collect_press_answers(
    campaign: &Campaign,
    seal_hints: &[SealHintPlan],
    shortcuts: &[ShortcutPlan],
) -> Vec<PressAnswer> {
    let quests = &campaign.quests.content;
    let authored_required = delvewright_dsl::is_v11(campaign.quests.dsl_version.as_str());
    press_answer_sites(seal_hints, shortcuts, authored_required)
        .into_iter()
        // The compiler lowers a wording it was GIVEN (an authored `sealed_hint`)
        // at every version; it invents one only where its policy still lets it.
        .filter(|(a, policy)| a.authored || policy.compiler_may_word_it())
        // …and stands down entirely where the campaign answers the press itself.
        .filter(|(a, _)| !quests.answers_press_at(&a.anchor))
        .map(|(a, _)| a)
        .collect()
}

/// Which of the five effect roots a visited effect hangs off — the part of a
/// site that decides **when** the firing happens, **whether the player is
/// forced to cause it**, and **what gates the firing as a whole**, which is all
/// the completability model needs on top of the effect itself.
///
/// The three roots that have an owner carry it: a consumer that must gate or
/// date a firing reads the owner off the site instead of re-deriving it from a
/// second, drift-prone walk (that is the whole point of the enumeration).
#[derive(Clone, Copy)]
pub(crate) enum EffectRoot<'a> {
    /// A quest's `on_objective_complete[<objective>]` — fires at that objective's
    /// `critical_path` step. Forced: completing the objective is the mainline.
    ObjectiveComplete(&'a str),
    /// A quest's `on_complete` — fires at the quest's completion step. Forced.
    QuestComplete(&'a Quest),
    /// An environment `triggers[].effects` — proximity/interaction-fired, so it has
    /// no step of its own; conservatively rooted at step 0. Carries the trigger,
    /// whose `requires_flags` gate the whole bundle.
    Trigger(&'a EnvTrigger),
    /// A `traps[].payload` (spec-0022) — proximity/interaction-fired exactly like a
    /// trigger, and **optional**: the party may never trip it. Carries the trap,
    /// whose `requires_flags` gate the whole payload.
    TrapPayload(&'a Trap),
    /// A dialogue option's `set-checkpoint` `on_respawn` bundle — re-run on death
    /// while that checkpoint is active, so it is optional too (nobody is forced to
    /// die).
    DialogueRespawn,
    /// A `shortcuts[].on_unlock` (spec-0016 §2) — fired by the far-side
    /// interaction, so it has no step of its own, and **optional**: `Plan::build`
    /// registers every shortcut gate as sealed at step 0 so the delve is proven
    /// completable with no shortcut ever taken, which is exactly the statement
    /// "the party may never fire this bundle".
    ///
    /// Carries nothing, unlike its trigger/trap siblings, because a shortcut
    /// declares no flag gate — there is no `requires_flags` for a consumer to read
    /// off it. The owning object is still available on the DSL side
    /// (`EffectRootOwner::ShortcutUnlock`) for a consumer that needs to name it,
    /// and the site's `path` already does.
    ShortcutUnlock,
    /// The campaign's `on_death` (spec-0031) — fired at the moment a player dies,
    /// so it has no step and is optional in the strongest sense the model has:
    /// nobody is forced to die.
    OnDeath,
    /// A `shops[].offers[].effects` (spec-0032) — fired by a player pressing a
    /// button, so it has no step and is optional: nobody is forced to buy
    /// anything. Carries nothing for the same reason `ShortcutUnlock` does — the
    /// gate that decides whether the button exists is the offer's own, and the
    /// site's `path` already names it.
    ShopOffer,
}

/// Where an effect was declared: which stage document, the JSON pointer inside it,
/// and which root it hangs off. Carried so a diagnostic can name the exact firing
/// site and so a consumer can reason about *when* the firing happens.
pub(crate) struct GateSite<'a> {
    /// The stage document the effect lives in (`quests` or `dialogue`).
    pub stage: &'static str,
    /// JSON pointer to the effect within that document.
    pub path: String,
    /// The effect root this firing hangs off.
    pub root: EffectRoot<'a>,
}

/// Where a top-level effect **list** was declared: which stage document, the JSON
/// pointer to the list itself, and which root it is.
pub(crate) struct EffectRootSite<'a> {
    /// The stage document the list lives in (`quests` or `dialogue`).
    pub stage: &'static str,
    /// JSON pointer to the **list** within that document (an element's pointer is
    /// this plus `/<index>`).
    pub path: String,
    /// Which root this list is.
    pub root: EffectRoot<'a>,
}

/// Visit **every top-level effect list the compiler can lower**, in one fixed
/// deterministic order.
///
/// A thin adapter over [`delvewright_dsl::for_each_effect_root`], which is the
/// single enumeration of effect roots in the workspace. It exists to re-present
/// the DSL's [`delvewright_dsl::EffectRootOwner`] as this crate's [`EffectRoot`],
/// which carries the same owners plus the completability model's reading of them
/// (see [`collect_region_events`]); it enumerates nothing itself.
///
/// A list is a root if `emit::emit_quest_effect` can reach it, not if the quests
/// stage happens to own it. Five lists are. Their order, and the reasoning, live
/// with the enumeration in `delvewright_dsl::effects`.
///
/// Consumers: [`for_each_gate_effect`] (→ the seal planner, `gates::check_seal_hints`
/// and the completability model), [`crate::timeline::walk_campaign`] (→ the
/// `DW0410` staged-walk model and, defined as it, `nav::all_effects`),
/// `emit::all_campaign_effects` (→ the generated functions themselves),
/// `emit::check_effect_anchors` (→ `DW0360`), `emit::declared_flags` (→ the
/// `dw.f_<flag>` scoreboard objectives), `rehearsal::bundles` (→ the
/// `dw:rehearsal` inventory) and both halves of [`crate::flow`].
pub(crate) fn for_each_effect_root<'a>(
    campaign: &'a Campaign,
    f: &mut dyn FnMut(&EffectRootSite<'a>, &'a [QuestEffect]),
) -> delvewright_dsl::RootBinding {
    delvewright_dsl::for_each_effect_root(campaign, &mut |site, list| {
        let root = match site.owner {
            delvewright_dsl::EffectRootOwner::ObjectiveComplete { objective, .. } => {
                EffectRoot::ObjectiveComplete(objective)
            }
            delvewright_dsl::EffectRootOwner::QuestComplete { quest } => {
                EffectRoot::QuestComplete(quest)
            }
            delvewright_dsl::EffectRootOwner::Trigger(t) => EffectRoot::Trigger(t),
            delvewright_dsl::EffectRootOwner::TrapPayload(t) => EffectRoot::TrapPayload(t),
            delvewright_dsl::EffectRootOwner::DialogueRespawn => EffectRoot::DialogueRespawn,
            delvewright_dsl::EffectRootOwner::ShortcutUnlock(_) => EffectRoot::ShortcutUnlock,
            delvewright_dsl::EffectRootOwner::OnDeath => EffectRoot::OnDeath,
            delvewright_dsl::EffectRootOwner::ShopOffer(_) => EffectRoot::ShopOffer,
        };
        f(
            &EffectRootSite {
                stage: site.stage,
                path: site.path.clone(),
                root,
            },
            list,
        );
    })
}

/// Visit **every effect the compiler can lower to a gate command**, at every
/// nesting depth: [`for_each_effect_root`] flattened, each root's list walked in
/// declaration order and each effect yielded ahead of its own nested lists.
///
/// Every consumer that reasons about emitted gate commands walks THIS: the seal
/// planner ([`collect_seal_hints`]), the wording check (`gates::check_seal_hints`,
/// `DW0423`) and the completability model ([`collect_region_events`], which feeds
/// `DW0311`/`DW0315`/`DW0342`/`DW0410`). Sharing the traversal is what makes the
/// checks and the emission unable to disagree about which firings exist.
pub(crate) fn for_each_gate_effect<'a>(
    campaign: &'a Campaign,
    f: &mut dyn FnMut(&GateSite<'a>, &'a QuestEffect),
) {
    fn deep<'a>(
        eff: &'a QuestEffect,
        stage: &'static str,
        path: &str,
        root: EffectRoot<'a>,
        f: &mut dyn FnMut(&GateSite<'a>, &'a QuestEffect),
    ) {
        f(
            &GateSite {
                stage,
                path: path.to_string(),
                root,
            },
            eff,
        );
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                deep(inner, stage, &format!("{path}/{pseg}/{j}"), root, f);
            }
        }
    }
    for_each_effect_root(campaign, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            deep(eff, site.stage, &format!("{}/{i}", site.path), site.root, f);
        }
    });
}

/// The absolute gate region **and fill block** a gate anchor resolves to. `None`
/// if the anchor is not a gate region.
///
/// The block is not an extra the callers happen to want: a `close-gate` is a
/// region write like any other, and what a write leaves behind is decided by what
/// it writes ([`RegionWrite::of_block`]) — a gate anchor declaring a fluid seals
/// nothing a body can stand on. Resolving the region without the block is what let
/// that conclusion be assumed instead of derived, so there is deliberately no
/// region-only variant of this lookup.
fn gate_region_block_any(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    name: &str,
) -> Option<([i32; 3], [i32; 3], String)> {
    for ((_, n), resolved) in anchors {
        if n == name
            && let ResolvedAnchor::Gate { from, to, block } = resolved
        {
            return Some((*from, *to, block.clone()));
        }
    }
    None
}

/// Resolve an anchor by name alone over a resolved-anchor map — the free-function
/// core of [`Plan::point_any`], so the planning stage can resolve a box *while*
/// building the `Plan` (which is where the region-write model is collected) rather
/// than needing a finished one.
fn point_any_in(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    anchor: &str,
) -> Option<[i32; 3]> {
    anchors.iter().find_map(|((_, name), resolved)| {
        (name == anchor).then_some(match resolved {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
    })
}

/// Resolve an anchor-centred box (`anchor ± extent`) over a resolved-anchor map —
/// the free-function core of [`Plan::zone_box`], for the same reason
/// [`point_any_in`] exists.
fn zone_box_in(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    zone: &delvewright_dsl::StealthZone,
) -> Option<([i32; 3], [i32; 3])> {
    let c = point_any_in(anchors, zone.anchor.as_str())?;
    let e = zone.extent;
    Some((
        [c[0] - e[0] as i32, c[1] - e[1] as i32, c[2] - e[2] as i32],
        [c[0] + e[0] as i32, c[1] + e[1] as i32, c[2] + e[2] as i32],
    ))
}

/// Collect every `open-gate` / `close-gate` firing (DSL v0.6) that emission can
/// lower, resolving each anchor to its gate region and rooting it at its firing
/// step. Feeds the `close-gate` completability model in `crate::nav`
/// (`DW0311`/`DW0315`/`DW0342`/`DW0410`).
///
/// Walks [`for_each_gate_effect`] — the **same** traversal the seal planner and
/// `gates::check_seal_hints` walk — so the model and the emission cannot disagree
/// about which firings exist. A model that saw only three of the five roots
/// emission reaches would leave a `close-gate` in a `traps[].payload` or a
/// dialogue option's `on_respawn` bundle filled in the datapack while every nav
/// proof believed the wall was open. Nesting is descended by that traversal, so a
/// gate effect inside a `sequence` step / lifecycle bundle is registered at its
/// root's firing step. An effect whose anchor is not a resolvable gate is skipped
/// (a point anchor / bad close-gate is a validation concern, `DW0142`/`DW0343`).
///
/// **When** a firing happens is read off the site's [`EffectRoot`]:
///
/// - a quest `on_objective_complete` fires at that objective's step, an
///   `on_complete` at the quest's completion step — the player is *forced* through
///   both, so both directions are modelled;
/// - an environment trigger, a trap payload and a dialogue-hosted `on_respawn`
///   bundle have no step of their own (proximity, a sprung trap, a death), so all
///   three are rooted conservatively at step 0, which precedes every leg.
///
/// The **optional** roots — a trap the party may never trip, a death nobody is
/// forced to suffer, an offer nobody is forced to buy — register their *filling*
/// writes only. An unguaranteed firing may be assumed to have happened exactly when
/// assuming so is conservative: it can seal a region (the proof must survive the
/// seal), it can never unseal one (the proof may not lean on a wall the player might
/// never open). That is the same rule a shortcut gate already obeys — sealed for the
/// whole model, because the delve must be finishable the long way. Environment
/// triggers keep their older both-directions treatment unchanged; narrowing that is a
/// different proof's verdict and is not this function's call to make.
///
/// **A fill from such a root is registered, and marked unforced, because "it sealed"
/// and "you can stand on it" are two different conclusions and only the first is
/// conservative.** The same solid block that walls a doorway floors the cell above
/// it, so a fill assumed-to-have-happened both blocks the party (harder — sound) and
/// carries them (easier — unsound). Dropping the event would lose the seal; keeping
/// it as an ordinary fill lends the forced path footing off a beat nobody has to
/// play. So the event is kept and the *uncertainty travels with it*
/// ([`RegionEvent::is_forced`]); `crate::nav` is where the two conclusions part.
///
/// A **flood** needs no such split and gets none: a flooded cell is impassable and
/// never floor, which is already the pointwise-worst of "the water is there" and "it
/// is not", so an unforced flood is exactly as conservative as a forced one.
fn collect_region_events(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    obj_step: &BTreeMap<String, usize>,
    ways: &crate::ways::WayStaging,
) -> Vec<RegionEvent> {
    let mut out = Vec::new();
    for_each_gate_effect(campaign, &mut |site, e| {
        // How a firing is BLAMED when it turns out to be unforced. Worded off the
        // root the site already carries, so the diagnostic names the beat an author
        // can go and look at rather than a JSON pointer alone.
        let blame = || match site.root {
            EffectRoot::TrapPayload(t) => {
                format!(
                    "the payload of trap `{}`, which the party may never spring",
                    t.id
                )
            }
            EffectRoot::DialogueRespawn => format!(
                "a `set-checkpoint` `on_respawn` bundle at `{}`, which runs only if somebody dies",
                site.path
            ),
            EffectRoot::ShortcutUnlock => format!(
                "a shortcut's `on_unlock` bundle at `{}`, which fires only if the party opens the \
                 shortcut from its far side",
                site.path
            ),
            EffectRoot::OnDeath => format!(
                "the campaign's `on_death` bundle at `{}`, which runs only if somebody dies",
                site.path
            ),
            EffectRoot::ShopOffer => format!(
                "a shop offer's effects at `{}`, which fire only if the party buys it",
                site.path
            ),
            // Not reachable: these three are forced, and a forced event carries no
            // blame. Worded rather than `unreachable!()` so a later root added to
            // the optional list cannot panic the compiler.
            EffectRoot::ObjectiveComplete(_)
            | EffectRoot::QuestComplete(_)
            | EffectRoot::Trigger(_) => format!("the effect bundle at `{}`", site.path),
        };
        let (fire_step, forced) = firing_of(&site.root, obj_step);
        // The three spellings of one write. A gate names a prefab gate anchor and
        // takes that anchor's box and its `replace`-filtered clear; a
        // `fill-region`/`clear-region` names its own anchor-centred box and clears
        // it outright; an `open-way` names a placed piece's exported way and takes
        // its cells, its block and its direction from the piece's metadata. None
        // of the three owns the model.
        //
        // A list rather than one box, because a way is a region with as many
        // boxes as the contract gave it, and each is written by its own `fill`.
        let resolved: Vec<ResolvedWrite> =
            match (e.gate_region_write(), e.region_write(), e.way_write()) {
                (Some((anchor, fills)), _, _) => gate_region_block_any(anchors, anchor.as_str())
                    .map(|(from, to, gate_block)| {
                        vec![(
                            (from, to),
                            if fills {
                                RegionWrite::of_block(&gate_block)
                            } else {
                                RegionWrite::Unseal
                            },
                        )]
                    })
                    .unwrap_or_default(),
                (_, Some((zone, block)), _) => zone_box_in(anchors, zone)
                    .map(|r| {
                        vec![(
                            r,
                            match block {
                                Some(b) => RegionWrite::of_block(b),
                                None => RegionWrite::Clear,
                            },
                        )]
                    })
                    .unwrap_or_default(),
                // An unresolvable way reference is `DW0547`'s finding, raised by
                // `crate::ways` before this model is consulted; here it simply
                // contributes nothing, exactly as a dangling anchor does.
                (_, _, Some((piece, name))) => ways
                    .resolve(piece.as_str(), name)
                    .map(|w| {
                        let write = match w.sign {
                            crate::ways::Sign::Laid => RegionWrite::of_block(&w.block),
                            crate::ways::Sign::Cleared => RegionWrite::Clear,
                        };
                        w.boxes.iter().map(|b| (*b, write)).collect()
                    })
                    .unwrap_or_default(),
                _ => return,
            };
        if resolved.is_empty() {
            return; // an unresolvable anchor is DW0142/DW0343/DW0355's finding
        }
        for (region, write) in resolved {
            if !write.fills() && !forced {
                // An optional firing may make a region impassable, never passable — a
                // flood is credited for the same reason a fill is: the proof must
                // survive it.
                continue;
            }
            out.push(if forced {
                RegionEvent::forced(region, write, fire_step)
            } else {
                RegionEvent::unforced(region, write, fire_step, blame())
            });
        }
    });
    out
}

/// **When a firing happens, and whether the party can avoid causing it** — read
/// off the site's [`EffectRoot`] and nowhere else.
///
/// One function because it is one reading. [`collect_region_events`] credits the
/// geometry from it and [`collect_way_openings`] states the disposition from it;
/// two copies of this match would be two instruments that agree until the day a
/// root is added to one of them.
///
/// - a quest `on_objective_complete` fires at that objective's step, an
///   `on_complete` at the quest's completion step — the player is *forced*
///   through both;
/// - an environment trigger, a trap payload and a dialogue-hosted `on_respawn`
///   bundle have no step of their own (proximity, a sprung trap, a death), so all
///   three are rooted conservatively at step 0, which precedes every leg.
///
/// The **optional** roots — a trap the party may never trip, a death nobody is
/// forced to suffer, an offer nobody is forced to buy, a shortcut opened from its
/// far side — are unforced: every shortcut gate is registered sealed at step 0 so
/// the delve is proven completable with no shortcut ever taken, which is exactly
/// "the party may never fire this bundle".
fn firing_of(root: &EffectRoot<'_>, obj_step: &BTreeMap<String, usize>) -> (usize, bool) {
    match root {
        EffectRoot::ObjectiveComplete(oid) => (obj_step.get(*oid).copied().unwrap_or(0), true),
        EffectRoot::QuestComplete(q) => (quest_complete_step(q, obj_step), true),
        EffectRoot::Trigger(_) => (0, true),
        EffectRoot::TrapPayload(_)
        | EffectRoot::DialogueRespawn
        | EffectRoot::ShortcutUnlock
        | EffectRoot::OnDeath
        | EffectRoot::ShopOffer => (0, false),
    }
}

/// Every `open-way` the campaign writes, with the quest-DAG point it fires at and
/// whether the party is forced to cause it (spec-0042 §2.5).
///
/// The same [`for_each_gate_effect`] walk and the same [`firing_of`] reading the
/// region-write model uses, so an `open-way` nested in a `sequence` step, a trap
/// payload or a shop offer is found by existing rather than by being remembered —
/// and is judged unforced there for the same reason its fill is.
pub(crate) fn collect_way_openings(
    campaign: &Campaign,
    obj_step: &BTreeMap<String, usize>,
) -> Vec<crate::ways::WayOpening> {
    let mut out = Vec::new();
    for_each_gate_effect(campaign, &mut |site, e| {
        let Some((piece, name)) = e.way_write() else {
            return;
        };
        let (fire_step, forced) = firing_of(&site.root, obj_step);
        out.push(crate::ways::WayOpening {
            prefab_id: piece.as_str().to_string(),
            way: name.to_string(),
            path: site.path.clone(),
            stage: site.stage,
            fire_step,
            forced,
        });
    });
    out
}

/// Collect every declared `teleport`'s resolved source volume with the step it
/// fires at ([`TeleportTransit`]), over the **same** general effect walk the
/// region-write model uses — so a `teleport` nested in a `sequence` step, in a trap
/// payload or in a shop offer is found by existing rather than by being
/// remembered.
///
/// Unlike [`collect_region_events`] it draws no forced/optional distinction: a
/// firing that may never happen must not be *leaned on* to prove a delve
/// completable, and must not be *ignored* when the question is whether the party is
/// even standing where the proof thinks they are. See [`Plan::transit_teleports`].
fn collect_transit_teleports(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<([i32; 3], [i32; 3])> {
    let mut out = Vec::new();
    for_each_gate_effect(campaign, &mut |_site, e| {
        let Some((from, _to)) = e.teleport() else {
            return;
        };
        // A dangling `from` anchor is `DW0360`'s finding, not this model's.
        if let Some(region) = zone_box_in(anchors, from) {
            out.push(region);
        }
    });
    out
}

/// Compute, for each objective's `critical_path` step, the set of steps of its
/// **strict DAG ancestors** (see [`Plan::strict_ancestor_steps`]): the transitive
/// `after`-closure within its own quest, plus every objective of every transitive
/// `depends_on`-ancestor quest (a quest completes — all its objectives — before any
/// dependent quest starts). Pure DAG structure, so it is deterministic and
/// independent of the lineariser's choice among valid orders.
/// Transitive-reachability closure over a `node → direct successors` adjacency,
/// seeded by `start` (exclusive of the seeds' own membership only insofar as they
/// re-enter via the graph). Shared by the quest-`depends_on` and objective-`after`
/// ancestor computations.
fn transitive_closure<'a>(
    start: &[&'a str],
    next: &BTreeMap<&'a str, Vec<&'a str>>,
) -> BTreeSet<&'a str> {
    let mut seen: BTreeSet<&'a str> = BTreeSet::new();
    let mut stack: Vec<&'a str> = start.to_vec();
    while let Some(x) = stack.pop() {
        if seen.insert(x)
            && let Some(nx) = next.get(x)
        {
            stack.extend(nx.iter().copied());
        }
    }
    seen
}

fn compute_strict_ancestor_steps(
    campaign: &Campaign,
    obj_step: &BTreeMap<String, usize>,
) -> BTreeMap<usize, BTreeSet<usize>> {
    // Quest direct `depends_on`, then its transitive-ancestor closure.
    let quest_deps: BTreeMap<&str, Vec<&str>> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| {
            (
                q.id.as_str(),
                q.depends_on.iter().map(|d| d.as_str()).collect(),
            )
        })
        .collect();
    let quest_anc: BTreeMap<&str, BTreeSet<&str>> = quest_deps
        .iter()
        .map(|(q, deps)| (*q, transitive_closure(deps, &quest_deps)))
        .collect();

    // Objective structure from stage 5: quest→objectives, objective→quest, `after`.
    let mut quest_objs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut obj_quest: BTreeMap<&str, &str> = BTreeMap::new();
    let mut obj_after: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for q in &campaign.quests.content.quests {
        let qid = q.id.as_str();
        for o in &q.objectives {
            let oid = o.id().as_str();
            quest_objs.entry(qid).or_default().push(oid);
            obj_quest.insert(oid, qid);
            obj_after.insert(oid, o.after().iter().map(|a| a.as_str()).collect());
        }
    }
    let after_closure: BTreeMap<&str, BTreeSet<&str>> = obj_after
        .iter()
        .map(|(o, a)| (*o, transitive_closure(a, &obj_after)))
        .collect();

    // Assemble the step-level ancestor sets.
    let mut out: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (&oid, &qid) in &obj_quest {
        let Some(&s) = obj_step.get(oid) else {
            continue;
        };
        let mut anc: BTreeSet<usize> = BTreeSet::new();
        let add = |name: &str, anc: &mut BTreeSet<usize>| {
            if let Some(&st) = obj_step.get(name) {
                anc.insert(st);
            }
        };
        if let Some(cl) = after_closure.get(oid) {
            for a in cl {
                add(a, &mut anc);
            }
        }
        if let Some(aq) = quest_anc.get(qid) {
            for q2 in aq {
                if let Some(objs) = quest_objs.get(*q2) {
                    for a in objs {
                        add(a, &mut anc);
                    }
                }
            }
        }
        out.insert(s, anc);
    }
    out
}

/// Resolve every stage-5 trap (DSL v0.6, spec-0011) in content order into a
/// [`TrapPlan`]: the trigger/hazard cell (the trap's `at` anchor), the dispenser
/// socket cell (from the `at` anchor's metadata), the dispense payload, and the
/// disarm affordance. A trap whose `at` anchor does not resolve to a point is
/// skipped (validation guarantees the anchor exists; an unresolved pool anchor
/// simply carries no proof/emission — the same policy as `collect_v06_effects`).
/// A resolved container fill (spec-0021).
#[derive(Clone, Debug, PartialEq)]
pub struct LootPlan {
    /// Loot id (`loot/<kebab>`).
    pub id: String,
    /// The anchor named by the declaration.
    pub anchor: String,
    /// The world cell of the container to fill.
    pub cell: [i32; 3],
    /// Contents in declaration order; index IS the container slot.
    pub items: Vec<LootItemPlan>,
}

/// One stack in a [`LootPlan`].
#[derive(Clone, Debug, PartialEq)]
pub struct LootItemPlan {
    /// Item id.
    pub item: String,
    /// Stack size.
    pub count: u32,
    /// Custom name, already localized by the build language.
    pub name: Option<String>,
    /// Enchantment id → level.
    pub enchantments: BTreeMap<String, u32>,
}

/// A `collect` objective that ADOPTS a prefab-placed container (DSL v0.8),
/// resolved to the container's world cell.
///
/// One resolution, one cell: the build-tier container proof (`DW0438`), the
/// activation-time fill and the critical-path step the bot opens all read THIS
/// value, so the cell the compiler proves is provably the cell it fills and the
/// cell the bot walks to. Resolving the anchor separately at each site is how a
/// proof and its emission drift apart.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectFillPlan {
    /// The `collect` objective's id.
    pub objective_id: String,
    /// The anchor named by `container`.
    pub anchor: String,
    /// The world cell of the container to fill.
    pub cell: [i32; 3],
    /// How many slots the fill occupies: the objective's own stack plus
    /// `fill_count` padding stacks.
    pub slots: usize,
}

/// Resolve every `collect` objective's adopted `container` (DSL v0.8) to a world
/// cell, in campaign order. An unresolvable anchor is skipped here and reported
/// by the DSL tier (`DW0142`) — the same policy [`collect_loot`] follows.
fn collect_collect_fills(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<CollectFillPlan> {
    let mut out = Vec::new();
    for q in &campaign.quests.content.quests {
        for o in &q.objectives {
            let Some(cont) = o.collect_container() else {
                continue;
            };
            let Some(cell) = point_any(anchors, cont.as_str()) else {
                continue;
            };
            out.push(CollectFillPlan {
                objective_id: o.id().as_str().to_string(),
                anchor: cont.as_str().to_string(),
                cell,
                slots: 1 + o.collect_fill_count() as usize,
            });
        }
    }
    out
}

/// Resolve every stage-5 `loot` declaration to a world cell. An unresolvable
/// anchor is skipped here and reported by the DSL tier (`DW0142`).
fn collect_loot(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<LootPlan> {
    campaign
        .quests
        .content
        .loot
        .iter()
        .filter_map(|l| {
            let cell = point_any(anchors, l.anchor.as_str())?;
            Some(LootPlan {
                id: l.id.as_str().to_string(),
                anchor: l.anchor.as_str().to_string(),
                cell,
                items: l
                    .items
                    .iter()
                    .map(|it| LootItemPlan {
                        item: it.item.clone(),
                        count: it.count,
                        name: it.name.clone(),
                        enchantments: it.enchantments.clone(),
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Resolve every declared lethal volume (DSL v0.10, spec-0031) against the solved
/// layout, in declaration order.
///
/// The box is `anchor ± extent`, resolved exactly as [`Plan::zone_box`] resolves a
/// stealth zone and a `damage-players` `in` filter — one geometry rule for every
/// anchor-centred box in the engine. A volume whose anchor no placed piece
/// provides is dropped (validation already reported `DW0142`) rather than
/// silently becoming a box at the origin.
fn collect_lethal_volumes(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<LethalVolumePlan> {
    campaign
        .quests
        .content
        .lethal_volumes
        .iter()
        .filter_map(|v| {
            let c = point_any(anchors, v.region.anchor.as_str())?;
            let e = v.region.extent;
            Some(LethalVolumePlan {
                id: v.id.as_str().to_string(),
                safe: safe_local(v.id.as_str()),
                region: (
                    [c[0] - e[0] as i32, c[1] - e[1] as i32, c[2] - e[2] as i32],
                    [c[0] + e[0] as i32, c[1] + e[1] as i32, c[2] + e[2] as i32],
                ),
                message: v.message.clone(),
                damage_type: v
                    .damage_type
                    .unwrap_or(delvewright_dsl::DamageKind::Generic),
            })
        })
        .collect()
}

fn collect_traps(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    dispenser_cells: &BTreeMap<(String, String), [i32; 3]>,
) -> Vec<TrapPlan> {
    let mut out = Vec::new();
    for t in &campaign.quests.content.traps {
        let Some(trigger_cell) = point_any(anchors, t.at.as_str()) else {
            continue;
        };
        let dispenser = dispenser_cells
            .iter()
            .find(|((_, name), _)| name == t.at.as_str())
            .map(|(_, cell)| *cell);
        let payload = t.dispense().map(|(item, count)| (item.to_string(), count));
        let payload_effects = t.payload.clone();
        let disarm = t.disarm.as_ref().and_then(|dis| {
            point_any(anchors, dis.via.as_str()).map(|via_cell| TrapDisarmPlan {
                via_anchor: dis.via.as_str().to_string(),
                via_cell,
                sets_flag: dis.sets_flag.as_str().to_string(),
            })
        });
        out.push(TrapPlan {
            id: t.id.as_str().to_string(),
            safe: safe_local(t.id.as_str()),
            trigger: t.trigger,
            at_anchor: t.at.as_str().to_string(),
            trigger_cell,
            dispenser,
            payload,
            payload_effects,
            lethality: t.lethality,
            reset: t.reset,
            disarm,
            requires_flags: t
                .requires_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
            requires_state: t.requires_state.clone(),
            forbids_flags: t
                .forbids_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect(),
        });
    }
    out
}

/// Accumulates v0.6 checkpoints / stealth beats in content order while resolving
/// their anchors (a struct so the collection borrows stay simple).
struct V06Collector<'a> {
    anchors: &'a BTreeMap<(String, String), ResolvedAnchor>,
    checkpoints: Vec<CheckpointPlan>,
    stealth: Vec<StealthBeat>,
    /// Firing steps of every `end-stealth`, in content order — closes each beat's
    /// active window ([`StealthBeat::end_step`]).
    stealth_ends: Vec<usize>,
}

impl V06Collector<'_> {
    fn push_checkpoint(
        &mut self,
        anchor: &str,
        on_respawn: &[QuestEffect],
        fire_step: usize,
        rest: bool,
        labels: Option<delvewright_dsl::BonfireLabels<'_>>,
    ) {
        if let Some(pos) = point_any(self.anchors, anchor) {
            let labels = labels.unwrap_or(delvewright_dsl::BonfireLabels {
                prompt: None,
                rest_label: None,
                save_label: None,
            });
            self.checkpoints.push(CheckpointPlan {
                index: self.checkpoints.len(),
                anchor: anchor.to_string(),
                pos,
                on_respawn: on_respawn.to_vec(),
                fire_step,
                rest,
                // Authored strings are ordinary inventoried campaign text; an
                // unauthored one takes the compiler's chrome default in its tagged
                // form, which `emit` rebinds to the build's language.
                prompt: labels
                    .prompt
                    .map(str::to_string)
                    .unwrap_or_else(|| delvewright_dsl::chrome::BONFIRE_TITLE.tagged()),
                rest_label: labels
                    .rest_label
                    .map(str::to_string)
                    .unwrap_or_else(|| delvewright_dsl::chrome::BONFIRE_REST.tagged()),
                save_label: labels
                    .save_label
                    .map(str::to_string)
                    .unwrap_or_else(|| delvewright_dsl::chrome::BONFIRE_SAVE.tagged()),
            });
        }
    }

    fn push_stealth(
        &mut self,
        zones: &[delvewright_dsl::StealthZone],
        on_caught: &[QuestEffect],
        grace_ticks: u32,
        fire_step: usize,
    ) {
        let resolved: Vec<(String, [i32; 3], [u32; 3])> = zones
            .iter()
            .filter_map(|z| {
                point_any(self.anchors, z.anchor.as_str())
                    .map(|p| (z.anchor.as_str().to_string(), p, z.extent))
            })
            .collect();
        if resolved.len() == zones.len() {
            self.stealth.push(StealthBeat {
                index: self.stealth.len() + 1,
                zones: resolved,
                on_caught: on_caught.to_vec(),
                grace_ticks,
                fire_step,
                end_step: None, // filled in by `close_stealth_windows`
            });
        }
    }

    fn handle(&mut self, eff: &QuestEffect, fire_step: usize) {
        if let Some((anchor, on_respawn)) = eff.set_checkpoint() {
            self.push_checkpoint(anchor.as_str(), on_respawn, fire_step, false, None);
        } else if let Some((anchor, on_rest)) = eff.bonfire() {
            // A bonfire IS a checkpoint (spec-0016 §1) — it inherits DW0315 /
            // DW0316 by being collected here. It is rooted at the arming step,
            // the earliest beat a rest can happen.
            self.push_checkpoint(
                anchor.as_str(),
                on_rest,
                fire_step,
                true,
                eff.bonfire_labels(),
            );
        } else if let Some((zones, on_caught, grace)) = eff.begin_stealth() {
            self.push_stealth(zones, on_caught, grace, fire_step);
        } else if matches!(eff, QuestEffect::EndStealth) {
            self.stealth_ends.push(fire_step);
        }
        // Descend into every nested effect list (`sequence` steps, `on_respawn`,
        // `on_caught`, `on_arrive`): a `set-checkpoint`/`begin-stealth` nested in a
        // `sequence` step is a real checkpoint/beat, fired at the same critical-path
        // step, and must be collected — else its content-ordered index is never
        // registered and `emit_set_checkpoint` silently mis-binds `#cp` to 0.
        for list in eff.nested_effect_lists() {
            for inner in list {
                self.handle(inner, fire_step);
            }
        }
    }
}

/// The total duration of the first `Cutscene` effect in `effects`, if any — the
/// sum over its shots, which is how long the harness must wait out the whole
/// cinematic (a multi-shot cutscene plays back-to-back in one bracket).
fn cutscene_seconds_in<'a>(effects: impl Iterator<Item = &'a QuestEffect>) -> Option<u32> {
    for e in effects {
        if let Some(shots) = e.cutscene_shots() {
            return Some(shots.iter().map(|s| s.resolved_seconds()).sum());
        }
    }
    None
}

/// Whether `obj_id` is the last objective (in `after`-DAG order) of `quest_id`,
/// i.e. its completion is what fires the quest's `on_complete` effects.
fn is_last_objective_of_quest(campaign: &Campaign, quest_id: &str, obj_id: &str) -> bool {
    campaign
        .quests
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == quest_id)
        .and_then(|q| objectives_in_order(&q.objectives).last().copied())
        .is_some_and(|last| last.id().as_str() == obj_id)
}

fn point_of(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    area: &str,
    anchor: &str,
) -> Result<[i32; 3], PlanError> {
    match anchors.get(&(area.to_string(), anchor.to_string())) {
        Some(ResolvedAnchor::Point { pos, .. }) => Ok(*pos),
        Some(ResolvedAnchor::Gate { from, .. }) => Ok(*from),
        None => Err(PlanError::new(
            DW_BUILD,
            format!(
                "anchor `{anchor}` in area `{area}` did not resolve to a world position at build \
                 time — if the campaign references an anchor no bound prefab/pool provides, \
                 `DW0142`/`DW0302` should have named it; reaching here means the resolver and \
                 validator disagree, a compiler bug — stop and escalate"
            ),
        )),
    }
}

/// The stage-5 quest containing an objective id, and that objective.
pub fn objective_quest<'a>(
    campaign: &'a Campaign,
    obj_id: &str,
) -> Option<(&'a str, &'a Objective)> {
    for q in &campaign.quests.content.quests {
        for o in &q.objectives {
            if o.id().as_str() == obj_id {
                return Some((q.id.as_str(), o));
            }
        }
    }
    None
}

/// The `on_objective_complete` effects for an objective, across the campaign.
pub fn objective_effects<'a>(campaign: &'a Campaign, obj_id: &str) -> Vec<&'a QuestEffect> {
    let mut out = Vec::new();
    for q in &campaign.quests.content.quests {
        if let Some(effects) = q
            .on_objective_complete
            .get(&delvewright_dsl::ObjectiveId(obj_id.to_string()))
        {
            out.extend(effects.iter());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Gate-aware reachability (M2 fix 7, DW0306)
// ---------------------------------------------------------------------------

/// A gate inside a placed piece: the carrying piece index and the local plane
/// (`axis`, `plane`) the barred row sits on. A sealed gate splits its piece into
/// the `local[axis] < plane` half (side 0) and the `>= plane` half (side 1); the
/// only path between them is the gate cut-edge, present once the gate is opened.
struct GateInfo {
    piece: usize,
    axis: usize,
    plane: i32,
}

/// A placed connector socket in world space, for reconstructing which pieces mate.
struct WorldSocket {
    piece: usize,
    connector: usize,
    world: [i32; 3],
    facing: Facing,
}

/// A piece-connectivity graph node: `(piece index, gate side)`. Non-gate pieces
/// are always side 0.
type Node = (usize, u8);

/// Verify every objective anchored in `area_id` is reachable from the area's
/// `spawn` using only gates already opened by earlier objectives in the DAG order
/// ([`DW_GATE_DEADLOCK`]). No-op for areas without gates.
fn check_gate_reachability(
    campaign: &Campaign,
    area_id: &str,
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    severed: Option<&BTreeSet<[i32; 3]>>,
) -> Result<(), PlanError> {
    // Gates carried by a piece in this area.
    let mut gates: BTreeMap<String, GateInfo> = BTreeMap::new();
    for name in collect_open_gate_anchors(campaign) {
        if let Some((pi, meta)) = anchor_piece(pieces, registry, &name)
            && let Some(region) = &meta.region
            && let Some((axis, plane)) = gate_plane(region)
        {
            gates.insert(
                name,
                GateInfo {
                    piece: pi,
                    axis,
                    plane,
                },
            );
        }
    }
    if gates.is_empty() {
        return Ok(());
    }

    // Global critical objective order (quests topo-sorted, objectives by `after`).
    let order = critical_objective_order(campaign);
    // When each gate opens: the earliest order index whose objective/quest opens it.
    let gate_open_at = gate_open_indices(campaign, &order, &gates);

    // Static (gate-independent) adjacency: mated sockets between pieces.
    let sockets = world_sockets(pieces, registry);
    let adj = build_adjacency(&sockets, pieces, registry, &gates, severed);

    // The entry piece, resolved through the same alias list every other consumer
    // uses (`spawn`, then `entry`) — the gate-deadlock proof must start where the
    // player actually starts, and the island tileset spells that anchor `entry`.
    let Some(spawn) = ENTRY_ANCHOR_NAMES
        .iter()
        .find_map(|name| anchor_node(pieces, registry, name, &gates))
    else {
        return Ok(()); // no entry anchor in this area → DW0345 reports it at build
    };

    for (i, step) in order.iter().enumerate() {
        let Some((tarea, tname)) = objective_target(campaign, step.obj, step.area) else {
            continue;
        };
        if tarea != area_id {
            continue;
        }
        let Some(target) = anchor_node(pieces, registry, &tname, &gates) else {
            continue;
        };
        // Gates already open when the player must stand at this objective's anchor:
        // those an earlier objective (index < i) has opened.
        let open: BTreeSet<usize> = gates
            .values()
            .zip(gates.keys())
            .filter(|(_, name)| gate_open_at.get(*name).is_some_and(|&j| j < i))
            .map(|(g, _)| g.piece)
            .collect();
        if !reachable(spawn, target, &adj, &gates, &open) {
            let culprit = gates
                .iter()
                .filter(|(name, _)| gate_open_at.get(*name).is_none_or(|&j| j >= i))
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            // A `rewire-socket sealed` (spec-0017) cuts doorway edges out
            // of this graph, so the blockage may be a massed-away passage, not
            // a quest-order mistake — say so.
            let severed_note = if severed.is_some_and(|s| !s.is_empty()) {
                " NOTE: this area's world-edits script seals doorway socket(s) via \
                 `rewire-socket` — those passages are cut from this proof; if the blockage \
                 is one of them, reopen it or leave another route"
            } else {
                ""
            };
            return Err(PlanError::new(
                DW_GATE_DEADLOCK,
                format!(
                    "objective `{}` (anchor `{tname}` in area `{area_id}`) is only reachable \
                     through a gate that no earlier objective opens (sealed gate(s): {culprit}), \
                     so the delve deadlocks. Fix the quest order: add an earlier objective whose \
                     `open-gate` effect opens {culprit} before this objective, or move `{tname}` \
                     to the near side of the gate. Do NOT delete the gate to dodge the check — \
                     that removes intended progression.{severed_note}",
                    step.obj.id()
                ),
            ));
        }
    }
    Ok(())
}

/// One objective in critical order, with its owning quest area.
struct OrderedObj<'a> {
    obj: &'a Objective,
    quest: &'a str,
    area: &'a str,
}

/// Every objective in critical-path order — the branch-coherent playthrough
/// ([`crate::flow::Flow::playthrough`]), i.e. exactly the sequence
/// [`build_critical_path`] exports, so the gate-aware reachability proof
/// (`DW0306`) judges the same walk the bot will.
fn critical_objective_order(campaign: &Campaign) -> Vec<OrderedObj<'_>> {
    let path = crate::flow::Flow::new(campaign).playthrough();
    let stage5: BTreeMap<&str, &Quest> = campaign
        .quests
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q))
        .collect();
    let quest_area: BTreeMap<&str, &str> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    let mut out = Vec::new();
    for st in &path.steps {
        let Some(q) = stage5.get(st.quest.as_str()) else {
            continue;
        };
        let Some(obj) = q
            .objectives
            .iter()
            .find(|o| o.id().as_str() == st.objective)
        else {
            continue;
        };
        out.push(OrderedObj {
            obj,
            quest: q.id.as_str(),
            area: quest_area.get(st.quest.as_str()).copied().unwrap_or(""),
        });
    }
    out
}

/// The `(area, anchor)` a player must stand at to complete `obj`.
fn objective_target(
    campaign: &Campaign,
    obj: &Objective,
    quest_area: &str,
) -> Option<(String, String)> {
    match obj {
        Objective::TalkTo { npc, .. } => {
            let n = campaign
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.id.as_str() == npc.as_str())?;
            Some((n.area.as_str().to_string(), n.anchor.as_str().to_string()))
        }
        Objective::ReachAnchor { anchor, .. }
        | Objective::Collect { anchor, .. }
        | Objective::Interact { anchor, .. } => {
            Some((quest_area.to_string(), anchor.as_str().to_string()))
        }
        Objective::Kill { wave, .. } => {
            let w = wave_of(campaign, wave.as_str())?;
            Some((quest_area.to_string(), w.anchor.as_str().to_string()))
        }
    }
}

/// Every anchor named by an `open-gate` effect anywhere in the campaign — quest
/// effects **and** environment triggers, descending every nested effect list
/// ([`QuestEffect::visit_deep`]). A gate opened from inside a `sequence` step or an
/// `on_arrive` bundle is a real gate: missing it here dropped the gate out of the
/// deadlock proof's model entirely, so `DW0306` silently proved reachability
/// against a world with one fewer door than the delve ships.
fn collect_open_gate_anchors(campaign: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut note = |e: &QuestEffect| {
        e.visit_deep(&mut |inner| {
            if let Some(a) = inner.open_gate_anchor() {
                out.insert(a.as_str().to_string());
            }
        });
    };
    for q in &campaign.quests.content.quests {
        for effs in q.on_objective_complete.values() {
            for e in effs {
                note(e);
            }
        }
        for e in &q.on_complete {
            note(e);
        }
    }
    for t in &campaign.quests.content.triggers {
        for e in &t.effects {
            note(e);
        }
    }
    out
}

/// The earliest critical-order index at which each gate opens: the index of the
/// objective whose `on_objective_complete` opens it, or the last objective of a
/// quest whose `on_complete` opens it (min over all openers).
fn gate_open_indices(
    campaign: &Campaign,
    order: &[OrderedObj<'_>],
    gates: &BTreeMap<String, GateInfo>,
) -> BTreeMap<String, usize> {
    let index_of: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, s)| (s.obj.id().as_str(), i))
        .collect();
    let last_obj_index: BTreeMap<&str, usize> = {
        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, s) in order.iter().enumerate() {
            m.entry(s.quest)
                .and_modify(|e| *e = (*e).max(i))
                .or_insert(i);
        }
        m
    };
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    let note = |gate: &str, idx: usize, out: &mut BTreeMap<String, usize>| {
        if gates.contains_key(gate) {
            out.entry(gate.to_string())
                .and_modify(|e| *e = (*e).min(idx))
                .or_insert(idx);
        }
    };
    // Deep + trigger-aware, mirroring `collect_open_gate_anchors`: a gate opened
    // from a `sequence` step opens at the step that fires the sequence, and a
    // trigger-fired gate is conservatively treated as open from step 0 (a trigger
    // has no place in the objective DAG — the same conservative rooting
    // `collect_region_events` uses). Treating either as "never opened" is what made
    // the deadlock proof reject a perfectly playable delve.
    let deep = |e: &QuestEffect, idx: usize, out: &mut BTreeMap<String, usize>| {
        e.visit_deep(&mut |inner| {
            if let Some(a) = inner.open_gate_anchor() {
                note(a.as_str(), idx, out);
            }
        });
    };
    for q in &campaign.quests.content.quests {
        for (oid, effs) in &q.on_objective_complete {
            let Some(&idx) = index_of.get(oid.as_str()) else {
                continue;
            };
            for e in effs {
                deep(e, idx, &mut out);
            }
        }
        if let Some(&idx) = last_obj_index.get(q.id.as_str()) {
            for e in &q.on_complete {
                deep(e, idx, &mut out);
            }
        }
    }
    for t in &campaign.quests.content.triggers {
        for e in &t.effects {
            deep(e, 0, &mut out);
        }
    }
    out
}

/// The gate's local dividing plane: the horizontal axis (x or z) the barred row is
/// thin along, plus the coordinate of that plane. `None` if neither horizontal
/// axis is a single cell thick (not a wall-like gate).
fn gate_plane(region: &crate::registry::Region) -> Option<(usize, i32)> {
    let span = |a: usize| (region.from[a] - region.to[a]).abs();
    // Prefer the thinner of the two horizontal axes (a vertical gate wall).
    let (zx, xx) = (span(2), span(0));
    if zx <= xx && zx == 0 {
        Some((2, region.from[2]))
    } else if xx == 0 {
        Some((0, region.from[0]))
    } else {
        None
    }
}

/// The placed piece carrying `anchor_name`, with that anchor's local metadata.
fn anchor_piece<'a>(
    pieces: &[PiecePlacement],
    registry: &'a PrefabRegistry,
    anchor_name: &str,
) -> Option<(usize, &'a AnchorMeta)> {
    pieces.iter().enumerate().find_map(|(i, p)| {
        registry
            .get(&p.prefab_id)
            .and_then(|m| m.anchors.get(anchor_name))
            .map(|am| (i, am))
    })
}

/// The gate side a local point falls on within piece `pi` (0 if the piece has no
/// gate).
fn side_of(pi: usize, local: [i32; 3], gates: &BTreeMap<String, GateInfo>) -> u8 {
    for g in gates.values() {
        if g.piece == pi {
            return u8::from(local[g.axis] >= g.plane);
        }
    }
    0
}

/// The graph node an anchor resolves to: its carrying piece + gate side.
fn anchor_node(
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    anchor_name: &str,
    gates: &BTreeMap<String, GateInfo>,
) -> Option<Node> {
    let (pi, am) = anchor_piece(pieces, registry, anchor_name)?;
    let local = am
        .pos
        .or_else(|| am.region.as_ref().map(|r| r.from))
        .unwrap_or([0, 0, 0]);
    Some((pi, side_of(pi, local, gates)))
}

/// Every connector socket of every placed piece, in world space.
fn world_sockets(pieces: &[PiecePlacement], registry: &PrefabRegistry) -> Vec<WorldSocket> {
    let mut out = Vec::new();
    for (pi, p) in pieces.iter().enumerate() {
        let Some(meta) = registry.get(&p.prefab_id) else {
            continue;
        };
        for (ci, conn) in meta.connectors.iter().enumerate() {
            let Some(f) = Facing::parse(&conn.facing) else {
                continue;
            };
            let t = p.rotation.transform(conn.local_pos);
            out.push(WorldSocket {
                piece: pi,
                connector: ci,
                world: [p.pos[0] + t[0], p.pos[1] + t[1], p.pos[2] + t[2]],
                facing: f.rotate(p.rotation),
            });
        }
    }
    out
}

/// Static adjacency over `(piece, side)` nodes: two mated sockets (child socket one
/// block beyond the parent, facing opposite) link their pieces' sub-nodes.
fn build_adjacency(
    sockets: &[WorldSocket],
    pieces: &[PiecePlacement],
    registry: &PrefabRegistry,
    gates: &BTreeMap<String, GateInfo>,
    severed: Option<&BTreeSet<[i32; 3]>>,
) -> BTreeMap<Node, BTreeSet<Node>> {
    // Local pos of a socket (for gate-side classification).
    let local_pos = |s: &WorldSocket| -> [i32; 3] {
        registry
            .get(&pieces[s.piece].prefab_id)
            .and_then(|m| m.connectors.get(s.connector))
            .map(|c| c.local_pos)
            .unwrap_or([0, 0, 0])
    };
    let mut adj: BTreeMap<Node, BTreeSet<Node>> = BTreeMap::new();
    for a in sockets {
        let a_next = [
            a.world[0] + a.facing.unit()[0],
            a.world[1] + a.facing.unit()[1],
            a.world[2] + a.facing.unit()[2],
        ];
        for b in sockets {
            if a.piece == b.piece {
                continue;
            }
            if b.world == a_next && b.facing == a.facing.opposite() {
                // A doorway severed by `rewire-socket sealed` (spec-0017)
                // is walled on both planes — no edge.
                if severed.is_some_and(|s| s.contains(&a.world) || s.contains(&b.world)) {
                    continue;
                }
                let na = (a.piece, side_of(a.piece, local_pos(a), gates));
                let nb = (b.piece, side_of(b.piece, local_pos(b), gates));
                adj.entry(na).or_default().insert(nb);
                adj.entry(nb).or_default().insert(na);
            }
        }
    }
    adj
}

/// BFS reachability from `spawn` to `target` over static edges plus the cut-edge of
/// every gate whose piece is in `open` (its two sides become connected).
fn reachable(
    spawn: Node,
    target: Node,
    adj: &BTreeMap<Node, BTreeSet<Node>>,
    gates: &BTreeMap<String, GateInfo>,
    open: &BTreeSet<usize>,
) -> bool {
    let mut seen: BTreeSet<Node> = BTreeSet::new();
    let mut queue: VecDeque<Node> = VecDeque::new();
    seen.insert(spawn);
    queue.push_back(spawn);
    while let Some(n) = queue.pop_front() {
        if n == target {
            return true;
        }
        if let Some(neis) = adj.get(&n) {
            for &m in neis {
                if seen.insert(m) {
                    queue.push_back(m);
                }
            }
        }
        // Open gate cut-edge on this piece connects its two sides.
        if open.contains(&n.0) && gates.values().any(|g| g.piece == n.0) {
            let other = (n.0, 1 - n.1);
            if seen.insert(other) {
                queue.push_back(other);
            }
        }
    }
    seen.contains(&target)
}

/// Which quests are triggered by `campaign-start`.
pub fn campaign_start_quests(campaign: &Campaign) -> Vec<&str> {
    campaign
        .quests
        .content
        .quests
        .iter()
        .filter(|q| matches!(q.trigger, Trigger::CampaignStart))
        .map(|q| q.id.as_str())
        .collect()
}
