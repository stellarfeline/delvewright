//! Assembled-world lighting model + deterministic relight pass (spec-0010).
//!
//! The compiler already owns the assembled voxel geometry (nav occupancy, spec-0008
//! addendum), so real light can be measured over the *assembled* world at compile
//! time rather than trusting the per-piece admission profile. This module:
//!
//! 1. Builds a **light-voxel field** over the assembled world (per-cell opacity +
//!    a block-light emitter table; 1.21.11 values), reusing the same static
//!    flood-estimate family as `prefabs/cave-generator` (internal code — no
//!    attribution ledger entry). Block light floods from every emitter, −1 per
//!    step through light-passing cells; sky light is seeded geometrically at
//!    sky-open cells under the **darkest reachable (time, weather)** attenuation
//!    and floods the same way. A cell's light is the max of both.
//! 2. Collects **reachable walkable cells** (nav reachability from an area's entry
//!    anchors) below the area's target — sealed cavities are unreachable by
//!    construction and never counted, resolving the hollow-statue false-dark class.
//! 3. For an area declaring `lighting`, runs a **deterministic greedy relight**:
//!    place the declared fixture at the best valid site near the darkest deficient
//!    cell, re-flood, repeat until satisfied ([`Relight::placements`]) or no site
//!    remains (`DW0211`).
//! 4. Emits the mitigation gate: `DW0210` (measured-dark area, no declaration, no
//!    night-vision) / `DW0211` (declared fixture cannot reach `min_light`).
//!
//! Determinism (ADR-0006): every collection is a `BTreeMap`/`BTreeSet`, the flood
//! frontier drains in a fixed order, and site search breaks ties on
//! `(distance², y, z, x)` — same DSL + seed → byte-identical placements.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{AreaLighting, Campaign, Fixture, WorldTime, WorldWeather};

use crate::nav::World;
use crate::plan::{Plan, ResolvedAnchor};

/// `DW0210`: a reachable walkable cell measured below light 3 in an area with no
/// `lighting` declaration and no night-vision class-kit mitigation (spec-0010).
pub const DW_DARK_UNMITIGATED: &str = "DW0210";
/// `DW0211`: a declared fixture cannot raise every reachable walkable cell to
/// `min_light` — no valid placement site remains (spec-0010).
pub const DW_RELIGHT_UNSATISFIABLE: &str = "DW0211";

/// The measured-darkness threshold: a reachable walkable cell below this, with no
/// declaration and no night-vision, is `DW0210` (spec-0010 mitigation hierarchy).
const DARK_THRESHOLD: u8 = 3;

/// How far from a deficient cell the relight pass searches for a valid fixture
/// site. Generous enough to reach a wall/ceiling/floor in any prefab room while
/// keeping the search bounded and deterministic.
const SITE_RADIUS: i32 = 8;

/// A single relight fixture placement: a block written at a world cell in the
/// init path (spec-0002 sealing/init ordering, after structure placement).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The world cell the block is written at.
    pub pos: [i32; 3],
    /// The block id (+ optional `[state]`) to `setblock`.
    pub block: String,
}

/// A lighting diagnostic (`DW0210`/`DW0211`), mapped to exit 2 (spec-0010).
#[derive(Clone, Debug)]
pub struct LightDiag {
    /// The stable code.
    pub code: &'static str,
    /// Human-readable explanation naming the area / cell.
    pub message: String,
}

/// The result of the assembled-light + relight pass over a whole campaign.
#[derive(Clone, Debug, Default)]
pub struct Relight {
    /// Fixture placements, in deterministic emission order (area order, then
    /// greedy-placement order within each area).
    pub placements: Vec<Placement>,
    /// The colliding fixtures' cells (campfire / floor lantern) that post-relight
    /// nav verification must treat as solid (spec-0010).
    pub extra_solid: BTreeSet<[i32; 3]>,
    /// Gate diagnostics (`DW0210`/`DW0211`); non-empty means the build fails
    /// (exit 2). Sorted by `(code, message)`.
    pub diagnostics: Vec<LightDiag>,
}

// ---------------------------------------------------------------------------
// 1.21.11 block-light emitter table + opacity (ported from cave-generator)
// ---------------------------------------------------------------------------

/// The bare block id: strip a `minecraft:` namespace and any `[state]` /
/// `{nbt}` suffix, so `minecraft:lantern[hanging=true]` matches `lantern`.
fn base_id(name: &str) -> &str {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    let end = n.find(['[', '{']).unwrap_or(n.len());
    &n[..end]
}

/// Block-light emission of a block id (0 if not a source). 1.21.11 values,
/// matching the cave-generator's live-derived table plus the fixture-registry
/// blocks and the common vanilla sources a prefab might carry.
pub fn emission(name: &str) -> u8 {
    match base_id(name) {
        "beacon"
        | "campfire"
        | "conduit"
        | "end_gateway"
        | "end_portal"
        | "fire"
        | "froglight"
        | "ochre_froglight"
        | "verdant_froglight"
        | "pearlescent_froglight"
        | "glowstone"
        | "jack_o_lantern"
        | "lantern"
        | "lava"
        | "lava_cauldron"
        | "sea_lantern"
        | "sea_pickle"
        | "shroomlight" => 15,
        "torch" | "wall_torch" | "end_rod" => 14,
        "soul_campfire" | "soul_lantern" | "soul_torch" | "soul_wall_torch" | "soul_fire"
        | "crying_obsidian" => 10,
        "glow_lichen"
        | "redstone_torch"
        | "redstone_wall_torch"
        | "amethyst_cluster"
        | "respawn_anchor" => 7,
        "enchanting_table" | "ender_chest" | "glow_item_frame" | "redstone_ore" => 7,
        "magma_block" | "brewing_stand" | "brown_mushroom" => 3,
        "furnace" | "smoker" | "blast_furnace" => 0, // lit-state driven; treat unlit
        _ => 0,
    }
}

/// Whether a block id lets light pass (for the flood estimate). Full opaque rock
/// and masonry block light; air, water, glass, small non-full blocks and plants
/// pass it. An id absent from the world's block map is air → passes. Unknown
/// blocks are treated as opaque (conservative — never overestimates light),
/// matching the cave-generator's estimator.
pub fn passes_light(name: &str) -> bool {
    let id = base_id(name);
    matches!(
        id,
        "air"
            | "cave_air"
            | "void_air"
            | "water"
            | "lava"
            | "glass"
            | "tinted_glass"
            | "iron_bars"
            | "chain"
            | "campfire"
            | "soul_campfire"
            | "lantern"
            | "soul_lantern"
            | "torch"
            | "wall_torch"
            | "soul_torch"
            | "soul_wall_torch"
            | "redstone_torch"
            | "end_rod"
            | "oak_fence"
            | "oak_fence_gate"
            | "glow_lichen"
            | "vine"
            | "ladder"
            | "scaffolding"
            | "pointed_dripstone"
            | "seagrass"
            | "tall_seagrass"
            | "kelp"
            | "kelp_plant"
            | "dead_bush"
            | "short_grass"
            | "fern"
            | "sea_pickle"
            | "cobweb"
            | "sugar_cane"
            | "lily_pad"
    ) || id.ends_with("_stained_glass")
        || id.ends_with("_stained_glass_pane")
        || id == "glass_pane"
}

// ---------------------------------------------------------------------------
// Assembled light model
// ---------------------------------------------------------------------------

/// A per-cell block-name model of the assembled world (spec-0010), built exactly
/// like the nav occupancy model — placed structures + solver seals + gate clears —
/// but keeping block *identity* so opacity and emission can be evaluated. Cells
/// absent from `blocks` are air.
pub struct LightModel {
    /// Non-air cells → block id.
    blocks: BTreeMap<[i32; 3], String>,
    /// Inclusive world AABB of all cells (for the sky-column scan).
    min: [i32; 3],
    max: [i32; 3],
}

impl LightModel {
    /// Build the assembled light model from the shared gravity-settled
    /// assembled-world model ([`crate::assembled`]): placed pieces, solver seals,
    /// gate clears, and unsupported falling blocks settled (task #42). Relight
    /// therefore evaluates opacity/emission over the same world the game assembles,
    /// so a `sand` floor that fell into the void is air here, not phantom rock.
    pub fn from_plan(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Self {
        Self::from_blocks(crate::assembled::assembled_blocks(plan, structures))
    }

    /// Build directly from a cell→block map (test entry point; no plan needed).
    pub fn from_blocks(blocks: BTreeMap<[i32; 3], String>) -> Self {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for c in blocks.keys() {
            for a in 0..3 {
                min[a] = min[a].min(c[a]);
                max[a] = max[a].max(c[a]);
            }
        }
        if blocks.is_empty() {
            min = [0, 0, 0];
            max = [0, 0, 0];
        }
        LightModel { blocks, min, max }
    }

    /// The block id at a cell (`"minecraft:air"` if absent).
    fn block_at(&self, c: [i32; 3]) -> &str {
        self.blocks
            .get(&c)
            .map(String::as_str)
            .unwrap_or("minecraft:air")
    }

    /// Whether a cell lets light pass (air or a light-passing block).
    fn passes(&self, c: [i32; 3]) -> bool {
        match self.blocks.get(&c) {
            None => true,
            Some(name) => passes_light(name),
        }
    }

    /// Whether a cell is opaque (blocks both block light and sky light).
    fn opaque(&self, c: [i32; 3]) -> bool {
        !self.passes(c)
    }

    /// Whether a cell has open sky above it: no opaque block anywhere in its
    /// column from just above it to the top of the world AABB (cells above the
    /// AABB are open sky). Geometric — the sky-exposure test (spec-0010).
    fn sky_open(&self, c: [i32; 3]) -> bool {
        let mut y = c[1] + 1;
        while y <= self.max[1] {
            if self.opaque([c[0], y, c[2]]) {
                return false;
            }
            y += 1;
        }
        true
    }

    /// Place / replace a block at a cell (relight fixture emission).
    fn set(&mut self, c: [i32; 3], block: &str) {
        self.blocks.insert(c, block.to_string());
    }

    /// Flood the assembled light field and return per-cell light within the AABB.
    /// Block light is seeded at every emitter; sky light is seeded at every
    /// sky-open light-passing cell at `effective_sky`. Both propagate −1 per step
    /// through light-passing cells; a cell's value is the max reached. A seed cell
    /// may itself be opaque (a glowstone/shroomlight block) — it still lights its
    /// passing neighbours.
    fn flood(&self, effective_sky: u8) -> BTreeMap<[i32; 3], u8> {
        let mut light: BTreeMap<[i32; 3], u8> = BTreeMap::new();
        let mut queue: VecDeque<([i32; 3], u8)> = VecDeque::new();

        // Seeds in a deterministic order: iterate the whole AABB in (y, z, x).
        for y in self.min[1]..=self.max[1] {
            for z in self.min[2]..=self.max[2] {
                for x in self.min[0]..=self.max[0] {
                    let c = [x, y, z];
                    let mut seed = emission(self.block_at(c));
                    if effective_sky > 0 && self.passes(c) && self.sky_open(c) {
                        seed = seed.max(effective_sky);
                    }
                    if seed > 0 {
                        let e = light.entry(c).or_insert(0);
                        if seed > *e {
                            *e = seed;
                            queue.push_back((c, seed));
                        }
                    }
                }
            }
        }

        const DIRS: [[i32; 3]; 6] = [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ];
        while let Some((c, l)) = queue.pop_front() {
            // Skip stale entries (a brighter value was recorded since).
            if light.get(&c).copied().unwrap_or(0) > l || l <= 1 {
                continue;
            }
            for d in DIRS {
                let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                if !self.in_aabb(n) || !self.passes(n) {
                    continue;
                }
                let nl = l - 1;
                let e = light.entry(n).or_insert(0);
                if nl > *e {
                    *e = nl;
                    queue.push_back((n, nl));
                }
            }
        }
        light
    }

    /// The AABB membership test (light only flows within the assembled bounds).
    fn in_aabb(&self, c: [i32; 3]) -> bool {
        (self.min[0]..=self.max[0]).contains(&c[0])
            && (self.min[1]..=self.max[1]).contains(&c[1])
            && (self.min[2]..=self.max[2]).contains(&c[2])
    }
}

// ---------------------------------------------------------------------------
// Sky attenuation constants (per declared time × weather)
// ---------------------------------------------------------------------------

/// Effective sky light at a fully sky-open cell for a `(time, weather)` state.
///
/// **1.21.11 baseline verified live** (delvewright itzg VANILLA, 2026-07-31): at a
/// fully sky-open cell the *stored* sky light is 15 in every time/weather state
/// (`advance_time`/`advance_weather false` keeps the geometric value constant),
/// and all four `time set` keywords + all three `weather` states apply cleanly.
/// The *effective* (time-attenuated) brightness a player and mob-spawn logic see —
/// `skyLight − skyDarken` in `getMaxLocalRawBrightness` — is not directly
/// command-readable (the `location_check` light predicate exposes only the stored
/// value), so the per-state attenuation below follows the documented vanilla
/// `getSkyDarken` surface model, applied **conservatively** (it never overestimates
/// brightness):
///
/// | time \ weather | clear | rain | thunder |
/// |----------------|-------|------|---------|
/// | noon / day     | 15    | 12   | 7       |
/// | night / midnight | 4   | 4    | 4       |
///
/// Rationale: full daylight (noon/day, clear) is 15; a clear-night surface sits at
/// the vanilla floor of 4 (the value that lets hostile mobs spawn under open sky);
/// rain darkens daytime (skyDarken rises), thunder darkens it enough for daytime
/// hostile spawns (≤7). Weather darkening scales with the daylight factor (≈0 at
/// night), so night/midnight stay at their 4 floor regardless of weather.
pub fn effective_sky(time: WorldTime, weather: WorldWeather) -> u8 {
    let base: u8 = match time {
        WorldTime::Noon | WorldTime::Day => 15,
        WorldTime::Night | WorldTime::Midnight => 4,
    };
    if base <= 4 {
        // Night floor: weather darkening is negligible at night.
        return base;
    }
    let atten: u8 = match weather {
        WorldWeather::Clear => 0,
        WorldWeather::Rain => 3,
        WorldWeather::Thunder => 8,
    };
    base.saturating_sub(atten)
}

/// The darkest effective sky light reachable in the campaign: the minimum of
/// [`effective_sky`] over the initial `(time, weather)` **and** every reachable
/// `set-time` / `set-weather` target (conservative — any declared switch counts).
/// Time and weather switch independently, so this is `effective_sky(darkest
/// reachable time, darkest reachable weather)`.
pub fn darkest_effective_sky(c: &Campaign) -> u8 {
    let mut times: BTreeSet<u8> = BTreeSet::new(); // discriminant via token order
    let mut weathers: BTreeSet<u8> = BTreeSet::new();
    let add_t = |t: WorldTime, set: &mut BTreeSet<u8>| {
        set.insert(match t {
            WorldTime::Day => 0,
            WorldTime::Noon => 1,
            WorldTime::Night => 2,
            WorldTime::Midnight => 3,
        });
    };
    let add_w = |w: WorldWeather, set: &mut BTreeSet<u8>| {
        set.insert(match w {
            WorldWeather::Clear => 0,
            WorldWeather::Rain => 1,
            WorldWeather::Thunder => 2,
        });
    };
    add_t(c.world.content.time.unwrap_or_default(), &mut times);
    add_w(c.world.content.weather.unwrap_or_default(), &mut weathers);
    // Quest effects.
    for q in &c.quests.content.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            if let Some(t) = e.set_time() {
                add_t(t, &mut times);
            }
            if let Some(w) = e.set_weather() {
                add_w(w, &mut weathers);
            }
        }
    }
    for t in &c.quests.content.triggers {
        for e in &t.effects {
            if let Some(tt) = e.set_time() {
                add_t(tt, &mut times);
            }
            if let Some(w) = e.set_weather() {
                add_w(w, &mut weathers);
            }
        }
    }
    // Dialogue effects.
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for e in &opt.effects {
                    if let Some(t) = e.set_time() {
                        add_t(t, &mut times);
                    }
                    if let Some(w) = e.set_weather() {
                        add_w(w, &mut weathers);
                    }
                }
            }
        }
    }
    let time_of = |d: u8| match d {
        0 => WorldTime::Day,
        1 => WorldTime::Noon,
        2 => WorldTime::Night,
        _ => WorldTime::Midnight,
    };
    let weather_of = |d: u8| match d {
        0 => WorldWeather::Clear,
        1 => WorldWeather::Rain,
        _ => WorldWeather::Thunder,
    };
    let mut darkest = 15u8;
    for &t in &times {
        for &w in &weathers {
            darkest = darkest.min(effective_sky(time_of(t), weather_of(w)));
        }
    }
    darkest
}

// ---------------------------------------------------------------------------
// Night-vision mitigation (retained heuristic, owner decision 2026-07-31)
// ---------------------------------------------------------------------------

/// Whether some class kit grants a night-vision mitigation (spec-0001 v0.2
/// heuristic, retained by spec-0010): a kit item whose id or display name contains
/// `night_vision` / `night vision` (case-insensitive). This is a static policy
/// gate ("you declared darkness, so declare a light source or hand out night
/// vision"), not a runtime guarantee.
pub fn has_night_vision(c: &Campaign) -> bool {
    c.classes.content.classes.iter().any(|class| {
        class.kit.iter().any(|item| {
            let id = item.item.to_ascii_lowercase();
            let name = item.name.as_deref().unwrap_or("").to_ascii_lowercase();
            let is_nv = |s: &str| s.contains("night_vision") || s.contains("night vision");
            is_nv(&id) || is_nv(&name)
        })
    })
}

// ---------------------------------------------------------------------------
// Relight pass
// ---------------------------------------------------------------------------

/// Run the assembled-light + relight pass over the whole campaign (spec-0010).
///
/// For each area: measure reachable walkable cells under the darkest reachable sky
/// attenuation; if `lighting` is declared, greedily place fixtures until every
/// reachable walkable cell reaches `min_light` (or `DW0211`); otherwise gate on
/// measured darkness (`DW0210` unless a reachable cell is ≥ 3 or night-vision
/// mitigates). Never mutates the caller's inputs; returns placements + the
/// colliding-fixture cells for post-relight nav verification.
pub fn relight(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Relight {
    let c = plan.campaign;
    let sky = darkest_effective_sky(c);
    let night_vision = has_night_vision(c);

    // The base assembled geometry (nav) and required-path cells fixtures must avoid.
    let nav = World::from_plan(plan, structures);
    // move-npc waypoint cells are part of the required paths; plan them on the base
    // world (an unroutable move is a separate DW0307 handled by emit — here we
    // just collect paths, ignoring routing errors).
    let moves = crate::nav::plan_moves(plan, &nav).unwrap_or_default();
    let required = nav.required_path_cells(plan, &moves);

    let mut model = LightModel::from_plan(plan, structures);
    let mut out = Relight::default();

    for area in &plan.areas {
        let (amin, amax) = area.bounds();
        // Entry anchors: every resolved Point anchor in this area, snapped to a
        // standable floor cell. Reachable walkable cells flood out from these; a
        // sealed cavity has no reachable start and is never counted.
        let starts: Vec<[i32; 3]> = plan
            .anchors
            .iter()
            .filter_map(|((aid, _), resolved)| {
                if aid != &area.area_id {
                    return None;
                }
                match resolved {
                    ResolvedAnchor::Point { pos, .. } => Some(*pos),
                    ResolvedAnchor::Gate { from, .. } => Some(*from),
                }
            })
            .collect();
        let reachable: BTreeSet<[i32; 3]> = nav
            .reachable_walkable(&starts)
            .into_iter()
            .filter(|cell| in_bounds(*cell, amin, amax))
            .collect();
        if reachable.is_empty() {
            continue; // nothing a player can stand on / reach in this area
        }

        let dsl_area = c
            .world
            .content
            .areas
            .iter()
            .find(|a| a.id.as_str() == area.area_id);
        let lighting = dsl_area.and_then(|a| a.lighting);

        match lighting {
            Some(spec) => {
                relight_area(
                    &mut model,
                    &nav,
                    &reachable,
                    &required,
                    &area.area_id,
                    spec,
                    sky,
                    amin,
                    amax,
                    &mut out,
                );
            }
            None => {
                // Measured-darkness gate over the assembled reachable walkable cells.
                if let Some(diag) =
                    measure_undeclared(&model, &reachable, sky, night_vision, &area.area_id)
                {
                    out.diagnostics.push(diag);
                }
            }
        }
    }

    out.diagnostics
        .sort_by(|a, b| (a.code, &a.message).cmp(&(b.code, &b.message)));
    out
}

/// Greedy relight of one declared area (spec-0010 §pass step 4). Repeatedly pick
/// the darkest deficient reachable walkable cell (ties by ascending `(y, z, x)`),
/// place the declared fixture at the best valid site near it, re-flood, and repeat
/// until no deficient cell remains or no site is available (`DW0211`).
#[allow(clippy::too_many_arguments)]
fn relight_area(
    model: &mut LightModel,
    nav: &World,
    reachable: &BTreeSet<[i32; 3]>,
    required: &BTreeSet<[i32; 3]>,
    area_id: &str,
    spec: AreaLighting,
    sky: u8,
    amin: [i32; 3],
    amax: [i32; 3],
    out: &mut Relight,
) {
    let min_light = spec.min_light;
    // A bounded loop: every iteration writes one fixture cell that was previously
    // unoccupied, so it terminates (cells are finite) — but cap for safety.
    let cap = (reachable.len() + 8) * 4;
    for _ in 0..cap {
        let light = model.flood(sky);
        // Darkest deficient reachable cell, ties by ascending (y, z, x).
        let mut worst: Option<([i32; 3], u8)> = None;
        for &cell in reachable {
            let l = light.get(&cell).copied().unwrap_or(0);
            if l >= min_light {
                continue;
            }
            match worst {
                Some((wc, wl))
                    if (wl, [wc[1], wc[2], wc[0]]) <= (l, [cell[1], cell[2], cell[0]]) => {}
                _ => worst = Some((cell, l)),
            }
        }
        let Some((dark, _)) = worst else {
            return; // satisfied
        };
        match pick_site(
            model,
            nav,
            required,
            reachable,
            spec.fixture,
            dark,
            amin,
            amax,
        ) {
            Some(site) => {
                model.set(site.pos, &site.block);
                if site.colliding {
                    out.extra_solid.insert(site.pos);
                }
                out.placements.push(Placement {
                    pos: site.pos,
                    block: site.block,
                });
            }
            None => {
                out.diagnostics.push(LightDiag {
                    code: DW_RELIGHT_UNSATISFIABLE,
                    message: format!(
                        "area `{area_id}`: declared relight fixture `{}` cannot reach \
                         `min_light` {min_light} — the darkest reachable walkable cell at {dark:?} \
                         has no valid placement site left. Fix in stage-1 `world.areas[].lighting`: \
                         choose a fixture that fits the geometry (`lantern`/`shroomlight` need \
                         less clearance than `torch`/`campfire`), lower the declared `min_light` \
                         (still within 1..=14), or open the room so a fixture site exists. Do NOT \
                         relax this by widening the reachable set — the cell is genuinely lit \
                         below target (spec-0010 DW0211)",
                        spec.fixture.token()
                    ),
                });
                return;
            }
        }
    }
}

/// The measured-darkness gate for an **undeclared** area (spec-0010 mitigation
/// hierarchy step 4). Returns a `DW0210` diagnostic when the darkest reachable
/// walkable cell measures below [`DARK_THRESHOLD`] under the darkest reachable sky
/// and no night-vision kit mitigates; `None` otherwise. Sealed cavities are not in
/// `reachable`, so they are never counted.
fn measure_undeclared(
    model: &LightModel,
    reachable: &BTreeSet<[i32; 3]>,
    sky: u8,
    night_vision: bool,
    area_id: &str,
) -> Option<LightDiag> {
    let light = model.flood(sky);
    let mut darkest: Option<([i32; 3], u8)> = None;
    for &cell in reachable {
        let l = light.get(&cell).copied().unwrap_or(0);
        match darkest {
            Some((_, bl)) if bl <= l => {}
            _ => darkest = Some((cell, l)),
        }
    }
    let (cell, l) = darkest?;
    if l < DARK_THRESHOLD && !night_vision {
        Some(LightDiag {
            code: DW_DARK_UNMITIGATED,
            message: format!(
                "area `{area_id}` has a reachable walkable cell at {cell:?} measured at light {l} \
                 (< {DARK_THRESHOLD}) under the darkest reachable (time, weather) sky (effective \
                 {sky}), with no `lighting` declaration and no night-vision class kit. Mitigate \
                 one of three ways: declare `world.areas[].lighting` (a relight `fixture` + \
                 `min_light`) for this area, brighten the scene (`world.time`/`weather`), or give \
                 a class a night-vision kit item. Do NOT lower `DARK_THRESHOLD` or trim the \
                 reachable set — the darkness is real (spec-0010 DW0210)"
            ),
        })
    } else {
        None
    }
}

/// A valid fixture placement site: the world cell, the block to write, and
/// whether the block adds collision (so post-relight nav verification sees it).
struct Site {
    pos: [i32; 3],
    block: String,
    colliding: bool,
}

/// Pick the best valid placement site for `fixture` near the dark cell `dark`,
/// per the fixture registry rule (spec-0010). `None` if no valid site exists
/// within [`SITE_RADIUS`].
///
/// The site is the valid candidate nearest `dark` (ties by ascending
/// `(distance², y, z, x)`), which maximises the light delivered to `dark`.
#[allow(clippy::too_many_arguments)]
fn pick_site(
    model: &LightModel,
    nav: &World,
    required: &BTreeSet<[i32; 3]>,
    reachable: &BTreeSet<[i32; 3]>,
    fixture: Fixture,
    dark: [i32; 3],
    amin: [i32; 3],
    amax: [i32; 3],
) -> Option<Site> {
    let mut best: Option<(i32, [i32; 3], Site)> = None;
    // Candidate order: scan a bounded box around `dark` in (y, z, x); rank by
    // (distance², y, z, x) — deterministic and nearest-first.
    for y in (dark[1] - SITE_RADIUS)..=(dark[1] + SITE_RADIUS) {
        for z in (dark[2] - SITE_RADIUS)..=(dark[2] + SITE_RADIUS) {
            for x in (dark[0] - SITE_RADIUS)..=(dark[0] + SITE_RADIUS) {
                let c = [x, y, z];
                if !in_bounds(c, amin, amax) {
                    continue;
                }
                let Some(site) = candidate(model, nav, required, reachable, fixture, c) else {
                    continue;
                };
                let d2 = (x - dark[0]).pow(2) + (y - dark[1]).pow(2) + (z - dark[2]).pow(2);
                let order = [site.pos[1], site.pos[2], site.pos[0]];
                let key = (d2, order);
                match &best {
                    Some((bd, bord, _)) if (*bd, *bord) <= key => {}
                    _ => best = Some((d2, order, site)),
                }
            }
        }
    }
    best.map(|(_, _, site)| site)
}

/// Evaluate cell `c` as a placement site for `fixture` (spec-0010 fixture
/// registry v1). Returns the [`Site`] if the fixture's rule is satisfied at `c`,
/// else `None`.
fn candidate(
    model: &LightModel,
    nav: &World,
    required: &BTreeSet<[i32; 3]>,
    reachable: &BTreeSet<[i32; 3]>,
    fixture: Fixture,
    c: [i32; 3],
) -> Option<Site> {
    let below = [c[0], c[1] - 1, c[2]];
    let above = [c[0], c[1] + 1, c[2]];
    let air = |cell: [i32; 3]| model.block_at(cell) == "minecraft:air";
    let solid = |cell: [i32; 3]| nav.solid_at(cell);
    let free = |cell: [i32; 3]| air(cell) && !required.contains(&cell);
    let site = |block: String, colliding: bool| {
        Some(Site {
            pos: c,
            block,
            colliding,
        })
    };

    match fixture {
        // Floor torch on solid ground, off required paths (no collision); wall
        // torch on a wall face as fallback.
        Fixture::Torch => {
            if free(c) && solid(below) {
                return site("minecraft:torch".to_string(), false);
            }
            // wall_torch: an air cell (off path) with a solid horizontal neighbour
            // to mount against; face points away from the wall.
            if free(c) {
                for (d, facing) in [
                    ([1, 0, 0], "east"),
                    ([-1, 0, 0], "west"),
                    ([0, 0, 1], "south"),
                    ([0, 0, -1], "north"),
                ] {
                    let wall = [c[0] - d[0], c[1], c[2] - d[2]];
                    if solid(wall) {
                        return site(format!("minecraft:wall_torch[facing={facing}]"), false);
                    }
                }
            }
            None
        }
        // Lantern hung under a ceiling block; floor-sitting as fallback (colliding).
        Fixture::Lantern => {
            if free(c) && solid(above) {
                return site("minecraft:lantern[hanging=true]".to_string(), false);
            }
            if free(c) && solid(below) && !reachable.contains(&c) {
                // floor lantern occupies the cell → colliding; keep it off walkable
                // cells so it can never wall a walker in.
                return site("minecraft:lantern[hanging=false]".to_string(), true);
            }
            None
        }
        // Campfire on solid floor with headroom, never on or adjacent to a
        // required path cell (it is a damage source). Colliding.
        Fixture::Campfire => {
            let adj_required = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                .iter()
                .any(|d| required.contains(&[c[0] + d[0], c[1], c[2] + d[2]]));
            if air(c)
                && !required.contains(&c)
                && !adj_required
                && !reachable.contains(&c)
                && solid(below)
                && air(above)
            {
                return site("minecraft:campfire[lit=true]".to_string(), true);
            }
            None
        }
        // Shroomlight embedded: replace a solid wall/ceiling block that borders an
        // air cell (so its light reaches the room). No walkability change (the cell
        // was already solid).
        Fixture::Shroomlight => {
            if solid(c) && !required.contains(&c) {
                let borders_air = [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ]
                .iter()
                .any(|d| air([c[0] + d[0], c[1] + d[1], c[2] + d[2]]));
                if borders_air {
                    return site("minecraft:shroomlight".to_string(), false);
                }
            }
            None
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn in_bounds(c: [i32; 3], min: [i32; 3], max: [i32; 3]) -> bool {
    (min[0]..=max[0]).contains(&c[0])
        && (min[1]..=max[1]).contains(&c[1])
        && (min[2]..=max[2]).contains(&c[2])
}

// ---------------------------------------------------------------------------
// Tests (spec-0010 acceptance criteria, synthetic in-code fixtures — ADR-0006)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::{Fixture, WorldTime, WorldWeather};

    /// A stone room shell of size `[w, h, d]` with an air interior. Floor at y=0,
    /// ceiling at y=h-1 (omitted when `open_top`), walls on the x/z perimeter.
    fn room(w: i32, h: i32, d: i32, open_top: bool) -> BTreeMap<[i32; 3], String> {
        let mut m = BTreeMap::new();
        for x in 0..w {
            for y in 0..h {
                for z in 0..d {
                    let ceil = y == h - 1;
                    let shell = y == 0
                        || (ceil && !open_top)
                        || x == 0
                        || x == w - 1
                        || z == 0
                        || z == d - 1;
                    if shell {
                        m.insert([x, y, z], "minecraft:stone".to_string());
                    }
                }
            }
        }
        m
    }

    /// A nav world whose solid set is every non-air cell of `map`.
    fn nav_of(map: &BTreeMap<[i32; 3], String>) -> World {
        World::from_solid_cells(map.keys().copied().collect())
    }

    /// Interior standable feet cells (the reachable set) of a room, seeded from the
    /// geometric centre floor cell (an interior entry anchor, like the real
    /// `spawn`) so the flood stays inside the shell rather than escaping onto the
    /// roof (roof cells are also standable).
    fn reachable_of(map: &BTreeMap<[i32; 3], String>) -> BTreeSet<[i32; 3]> {
        let nav = nav_of(map);
        let (min, max) = bounds(map);
        let center = [(min[0] + max[0]) / 2, min[1] + 1, (min[2] + max[2]) / 2];
        nav.reachable_walkable(&[center])
    }

    fn bounds(map: &BTreeMap<[i32; 3], String>) -> ([i32; 3], [i32; 3]) {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for c in map.keys() {
            for a in 0..3 {
                min[a] = min[a].min(c[a]);
                max[a] = max[a].max(c[a]);
            }
        }
        (min, max)
    }

    fn min_reachable_light(model: &LightModel, reachable: &BTreeSet<[i32; 3]>, sky: u8) -> u8 {
        let light = model.flood(sky);
        reachable
            .iter()
            .map(|c| light.get(c).copied().unwrap_or(0))
            .min()
            .unwrap_or(0)
    }

    // --- emitter + attenuation constants ---

    #[test]
    fn emitter_table_1_21_11() {
        assert_eq!(emission("minecraft:torch"), 14);
        assert_eq!(emission("minecraft:wall_torch"), 14);
        assert_eq!(emission("minecraft:lantern"), 15);
        assert_eq!(emission("minecraft:campfire"), 15);
        assert_eq!(emission("minecraft:shroomlight"), 15);
        assert_eq!(emission("minecraft:glowstone"), 15);
        assert_eq!(emission("minecraft:sea_lantern"), 15);
        assert_eq!(emission("minecraft:soul_lantern"), 10);
        assert_eq!(emission("minecraft:glow_lichen"), 7);
        assert_eq!(emission("minecraft:magma_block"), 3);
        assert_eq!(emission("minecraft:stone"), 0);
        assert_eq!(emission("minecraft:air"), 0);
    }

    #[test]
    fn effective_sky_attenuation_table() {
        // Full daylight.
        assert_eq!(effective_sky(WorldTime::Noon, WorldWeather::Clear), 15);
        assert_eq!(effective_sky(WorldTime::Day, WorldWeather::Clear), 15);
        // Night floor (weather-independent).
        assert_eq!(effective_sky(WorldTime::Night, WorldWeather::Clear), 4);
        assert_eq!(effective_sky(WorldTime::Midnight, WorldWeather::Clear), 4);
        assert_eq!(effective_sky(WorldTime::Midnight, WorldWeather::Thunder), 4);
        // Weather darkens daytime.
        assert_eq!(effective_sky(WorldTime::Noon, WorldWeather::Rain), 12);
        assert_eq!(effective_sky(WorldTime::Noon, WorldWeather::Thunder), 7);
        // Monotone: brighter ≥ darker.
        assert!(
            effective_sky(WorldTime::Noon, WorldWeather::Clear)
                >= effective_sky(WorldTime::Midnight, WorldWeather::Thunder)
        );
    }

    // --- flood + sky geometry ---

    #[test]
    fn flood_block_light_falls_off_by_one() {
        let mut map = room(5, 5, 5, false);
        map.insert([2, 1, 2], "minecraft:glowstone".to_string());
        let model = LightModel::from_blocks(map);
        let light = model.flood(0);
        assert_eq!(light.get(&[2, 1, 2]).copied().unwrap_or(0), 15);
        // One step away through air = 14.
        assert_eq!(light.get(&[2, 2, 2]).copied().unwrap_or(0), 14);
    }

    #[test]
    fn sky_open_only_under_open_column() {
        let open = LightModel::from_blocks(room(5, 5, 5, true));
        // Interior floor cell has open sky above (no ceiling).
        assert!(open.sky_open([2, 1, 2]));
        let closed = LightModel::from_blocks(room(5, 5, 5, false));
        // A ceiling blocks the sky.
        assert!(!closed.sky_open([2, 1, 2]));
    }

    // --- Criterion 2: declared lantern reaches min_light, fixtures only ---

    #[test]
    fn crit2_declared_lantern_reaches_min_light() {
        let map = room(9, 5, 9, false); // enclosed, unlit
        let model_map = map.clone();
        let nav = nav_of(&map);
        let reachable = reachable_of(&map);
        let (amin, amax) = bounds(&map);
        let mut model = LightModel::from_blocks(model_map);
        let mut out = Relight::default();
        let spec = AreaLighting {
            fixture: Fixture::Lantern,
            min_light: 7,
        };
        relight_area(
            &mut model,
            &nav,
            &reachable,
            &BTreeSet::new(),
            "area/hall",
            spec,
            0,
            amin,
            amax,
            &mut out,
        );
        assert!(
            out.diagnostics.is_empty(),
            "must satisfy: {:?}",
            out.diagnostics
        );
        assert!(!out.placements.is_empty(), "expected fixtures placed");
        for p in &out.placements {
            assert!(
                p.block.starts_with("minecraft:lantern"),
                "only registry lantern fixtures, got {}",
                p.block
            );
        }
        assert!(
            min_reachable_light(&model, &reachable, 0) >= 7,
            "every reachable cell must reach min_light 7"
        );
    }

    // --- Criterion 4: dark seam between two lit ends gets a fixture ---

    #[test]
    fn crit4_dark_seam_corridor_gets_a_fixture() {
        let mut map = room(21, 5, 3, false);
        map.insert([1, 3, 1], "minecraft:glowstone".to_string());
        map.insert([19, 3, 1], "minecraft:glowstone".to_string());
        let nav = nav_of(&map);
        let reachable = reachable_of(&map);
        let (amin, amax) = bounds(&map);
        // Seam (mid-corridor) is dark before relight.
        let pre = LightModel::from_blocks(map.clone());
        assert!(pre.flood(0).get(&[10, 1, 1]).copied().unwrap_or(0) < 7);
        let mut model = LightModel::from_blocks(map);
        let mut out = Relight::default();
        relight_area(
            &mut model,
            &nav,
            &reachable,
            &BTreeSet::new(),
            "area/corridor",
            AreaLighting {
                fixture: Fixture::Torch,
                min_light: 7,
            },
            0,
            amin,
            amax,
            &mut out,
        );
        assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
        assert!(
            out.placements.iter().any(|p| (7..=13).contains(&p.pos[0])),
            "expected a fixture in the dark seam region, got {:?}",
            out.placements
        );
        assert!(min_reachable_light(&model, &reachable, 0) >= 7);
    }

    // --- Criterion 6: dark undeclared area → DW0210 ---

    #[test]
    fn crit6_dark_undeclared_is_dw0210() {
        let map = room(7, 5, 7, false); // enclosed, unlit
        let model = LightModel::from_blocks(map.clone());
        let reachable = reachable_of(&map);
        let diag = measure_undeclared(&model, &reachable, 0, false, "area/crypt");
        assert!(diag.is_some());
        assert_eq!(diag.unwrap().code, DW_DARK_UNMITIGATED);
    }

    // --- Criterion 5: night-vision kit suppresses DW0210 ---

    #[test]
    fn crit5_night_vision_suppresses_dw0210() {
        let map = room(7, 5, 7, false);
        let model = LightModel::from_blocks(map.clone());
        let reachable = reachable_of(&map);
        assert!(
            measure_undeclared(&model, &reachable, 0, true, "area/crypt").is_none(),
            "night vision must mitigate an undeclared dark area"
        );
    }

    // --- Criterion 3: a sealed dark cavity is never counted ---

    #[test]
    fn crit3_sealed_cavity_not_counted() {
        // A lit main room plus a fully sealed (unreachable) dark air pocket: a
        // detached 3×3×3 stone cube with a hollow air centre (the hollow-statue
        // false-dark class). The cavity is enclosed on all six sides by stone.
        let mut map = room(9, 5, 9, false);
        map.insert([4, 3, 4], "minecraft:glowstone".to_string()); // light the room
        for dx in 0..3 {
            for dy in 0..3 {
                for dz in 0..3 {
                    map.insert([20 + dx, dy, 20 + dz], "minecraft:stone".to_string());
                }
            }
        }
        map.remove(&[21, 1, 21]); // hollow the cube's centre → a sealed dark cell
        let model = LightModel::from_blocks(map.clone());
        let reachable = reachable_of(&map);
        // The sealed cell is dark but not a reachable walkable cell.
        assert!(model.flood(0).get(&[21, 1, 21]).copied().unwrap_or(0) < 3);
        assert!(!reachable.contains(&[21, 1, 21]));
        // The lit room measures clean despite the dark sealed pocket.
        assert!(
            measure_undeclared(&model, &reachable, 0, false, "area/room").is_none(),
            "a sealed dark cavity must not trip DW0210"
        );
    }

    // --- Criterion 7: declared fixture with no valid site → DW0211 ---

    #[test]
    fn crit7_unsatisfiable_is_dw0211() {
        // A tiny dark floating platform: every air cell is a required path cell, so
        // no off-path torch site exists and there is no wall to mount a wall torch.
        let mut map = BTreeMap::new();
        for x in 0..3 {
            for z in 0..3 {
                map.insert([x, 0, z], "minecraft:stone".to_string());
            }
        }
        let nav = nav_of(&map);
        let reachable = reachable_of(&map);
        let (amin, amax) = bounds(&map);
        // Mark every reachable + head cell required (and the air above), leaving no
        // free site.
        let mut required: BTreeSet<[i32; 3]> = BTreeSet::new();
        for &c in &reachable {
            required.insert(c);
            required.insert([c[0], c[1] + 1, c[2]]);
        }
        let mut model = LightModel::from_blocks(map);
        let mut out = Relight::default();
        relight_area(
            &mut model,
            &nav,
            &reachable,
            &required,
            "area/ledge",
            AreaLighting {
                fixture: Fixture::Torch,
                min_light: 7,
            },
            0,
            amin,
            amax,
            &mut out,
        );
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == DW_RELIGHT_UNSATISFIABLE),
            "expected DW0211, got {:?} / placements {:?}",
            out.diagnostics,
            out.placements
        );
    }

    // --- Criterion 9: sky-open shore at (noon, clear) needs no fixtures ---

    #[test]
    fn crit9_sky_shore_noon_clear_no_fixtures() {
        let map = room(9, 5, 9, true); // open top → sky-lit
        let nav = nav_of(&map);
        let reachable = reachable_of(&map);
        let (amin, amax) = bounds(&map);
        let sky = effective_sky(WorldTime::Noon, WorldWeather::Clear); // 15
        let mut model = LightModel::from_blocks(map);
        let mut out = Relight::default();
        relight_area(
            &mut model,
            &nav,
            &reachable,
            &BTreeSet::new(),
            "area/shore",
            AreaLighting {
                fixture: Fixture::Torch,
                min_light: 7,
            },
            sky,
            amin,
            amax,
            &mut out,
        );
        assert!(
            out.placements.is_empty(),
            "sky-lit noon shore needs no fixtures: {:?}",
            out.placements
        );
        assert!(out.diagnostics.is_empty());
    }

    // --- Criterion 10: same shore under midnight demands mitigation ---

    #[test]
    fn crit10_sky_shore_midnight_demands_mitigation() {
        let map = room(9, 5, 9, true);
        let nav = nav_of(&map);
        let reachable = reachable_of(&map);
        let (amin, amax) = bounds(&map);
        let sky_night = effective_sky(WorldTime::Midnight, WorldWeather::Clear); // 4
        // Under a min_light-7 declaration, the sky-lit shore is deficient at night.
        let pre = LightModel::from_blocks(map.clone());
        assert!(min_reachable_light(&pre, &reachable, sky_night) < 7);
        let mut model = LightModel::from_blocks(map);
        let mut out = Relight::default();
        relight_area(
            &mut model,
            &nav,
            &reachable,
            &BTreeSet::new(),
            "area/shore",
            AreaLighting {
                fixture: Fixture::Torch,
                min_light: 7,
            },
            sky_night,
            amin,
            amax,
            &mut out,
        );
        assert!(
            !out.placements.is_empty(),
            "midnight sky shore must demand fixtures"
        );
        assert!(min_reachable_light(&model, &reachable, sky_night) >= 7);
    }

    // --- Criterion 1: relight is deterministic (byte-identical placements) ---

    #[test]
    fn crit1_relight_is_deterministic() {
        let build = || {
            let map = room(11, 5, 11, false);
            let nav = nav_of(&map);
            let reachable = reachable_of(&map);
            let (amin, amax) = bounds(&map);
            let mut model = LightModel::from_blocks(map);
            let mut out = Relight::default();
            relight_area(
                &mut model,
                &nav,
                &reachable,
                &BTreeSet::new(),
                "area/hall",
                AreaLighting {
                    fixture: Fixture::Lantern,
                    min_light: 9,
                },
                0,
                amin,
                amax,
                &mut out,
            );
            out.placements
        };
        assert_eq!(
            build(),
            build(),
            "relight placements must be byte-identical"
        );
    }
}
