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

use delvewright_dsl::{AreaLighting, AreaMitigation, Campaign, Fixture, WorldTime, WorldWeather};

use crate::nav::World;
use crate::plan::{Plan, ResolvedAnchor};
use delvewright_dsl::DwCode;

/// `DW0210`: a reachable walkable cell measured below light 3 in an area with no
/// `lighting` declaration and no night-vision class-kit mitigation (spec-0010).
pub const DW_DARK_UNMITIGATED: DwCode = DwCode::every_version("DW0210");
/// `DW0211`: a declared fixture cannot raise every reachable walkable cell to
/// `min_light` — no valid placement site remains (spec-0010).
pub const DW_RELIGHT_UNSATISFIABLE: DwCode = DwCode::every_version("DW0211");

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
    pub code: DwCode,
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

/// A block's blockstate property, defaulting when the name carries no state —
/// the assembled model stores full blockstates, and a vanilla
/// structure palette always carries the complete property set, so the default
/// only applies to the compiler's own bare-id fixture strings.
fn prop<'a>(name: &'a str, key: &str, default: &'a str) -> &'a str {
    crate::assembled::state_value(name, key).unwrap_or(default)
}

/// Block-light emission of a block (0 if not a source), **Minecraft Java
/// 1.21.11**, evaluated over the block's actual blockstate.
///
/// # The never-overestimate contract
///
/// This module's whole purpose is to prove no reachable walkable cell ships
/// darker than its area's `min_light` (`DW0210`/`DW0211`). That proof is only
/// sound if the model's light is a **lower bound** on the game's: modelling a
/// block brighter than vanilla lets a genuinely dark area pass the gate and ship
/// unmitigated — the exact failure the gate exists to prevent.
///
/// Seven blocks break that contract the moment a table collapses a
/// state-dependent block onto its *brightest* state: `sea_pickle`,
/// `redstone_ore`, `respawn_anchor`, `amethyst_cluster`, `brewing_stand`,
/// `brown_mushroom`, and `glow_item_frame` (which is not even a block — it is an
/// entity, and emits no block light in Java at all). Every entry below is the
/// verified 1.21.11 value for the state actually present.
///
/// Blocks absent from the table emit 0 — an underestimate, which is the safe
/// direction.
///
/// # What the values are measured against
///
/// Every value here is checked against the pinned game's own answer by
/// `emission_matches_the_pinned_game` (`crates/compiler/tests/emission_table.rs`),
/// over `crates/compiler/tests/fixtures/light/emission-1.21.11.tsv` — a dump of
/// `BlockState.getLightEmission()` for all 29,671 blockstates of the pinned
/// 1.21.11 server jar, regenerated by `tools/dump-block-light.py`. The test
/// asserts the contract (`emission ≤ game`) over every state, so an entry that
/// is too bright is a red, not a design opinion.
///
/// # What the model evaluates, and the one exception
///
/// The model evaluates the blockstate the world **ships** with. It does not
/// simulate redstone, block entities, weathering or player action. Where the
/// world itself re-derives a light-bearing property at load time, the shipped
/// blockstate is not evidence of the shipped *light*, so the entry takes the
/// **minimum** over the states the world can drive the block to:
///
/// - `redstone_lamp` has no `onPlace`, and its `neighborChanged` schedules an
///   unlight the first time any neighbour updates while no signal is present —
///   which structure assembly does by writing the neighbouring blocks. A lamp
///   shipped `lit=true` is therefore not a stable configuration, and its entry
///   is the minimum over `lit`, i.e. **0**.
/// - `trial_spawner`'s `trial_spawner_state` is owned by its block entity
///   (`getTicker`); the minimum over its six states is **0**.
/// - `vault`'s `vault_state` is owned by its block entity too, but every one of
///   its states emits 6 or 12, so the minimum is **6** — a value that holds
///   whatever the block entity does.
///
/// `copper_bulb` is deliberately *not* in that list: its `onPlace` runs
/// `checkAndFlip`, which returns without touching `lit` whenever the neighbour
/// signal already agrees with `powered` (verified in the pinned jar's bytecode).
/// A bulb shipped `lit=true` in a room with no redstone stays lit, exactly as a
/// shipped `campfire[lit=true]` stays alight.
pub fn emission(name: &str) -> u8 {
    match base_id(name) {
        // --- unconditional 15 ---
        // beacon <https://minecraft.wiki/w/Beacon>;
        // conduit (active or not) <https://minecraft.wiki/w/Conduit>;
        // end_gateway <https://minecraft.wiki/w/End_Gateway_(block)>;
        // end_portal <https://minecraft.wiki/w/End_Portal_(block)>;
        // fire <https://minecraft.wiki/w/Fire>;
        // glowstone <https://minecraft.wiki/w/Glowstone>;
        // jack_o_lantern <https://minecraft.wiki/w/Jack_o%27Lantern>;
        // lantern <https://minecraft.wiki/w/Lantern>;
        // lava, incl. flowing <https://minecraft.wiki/w/Lava>;
        // lava_cauldron <https://minecraft.wiki/w/Cauldron>;
        // sea_lantern <https://minecraft.wiki/w/Sea_Lantern>;
        // shroomlight <https://minecraft.wiki/w/Shroomlight>;
        // the three froglights <https://minecraft.wiki/w/Froglight> — note there
        // is NO plain `minecraft:froglight` block, only these prefixed ids.
        //
        // The copper lantern family is 15 at EVERY oxidation stage, waxed or
        // not — unlike the copper bulb below, whose light does step down. All
        // eight ids measure 15 in every one of their four states.
        "beacon"
        | "conduit"
        | "end_gateway"
        | "end_portal"
        | "fire"
        | "glowstone"
        | "jack_o_lantern"
        | "lantern"
        | "lava"
        | "lava_cauldron"
        | "sea_lantern"
        | "shroomlight"
        | "ochre_froglight"
        | "verdant_froglight"
        | "pearlescent_froglight"
        | "copper_lantern"
        | "exposed_copper_lantern"
        | "weathered_copper_lantern"
        | "oxidized_copper_lantern"
        | "waxed_copper_lantern"
        | "waxed_exposed_copper_lantern"
        | "waxed_weathered_copper_lantern"
        | "waxed_oxidized_copper_lantern" => 15,

        // --- unconditional 14 ---
        // torch / wall_torch <https://minecraft.wiki/w/Torch>;
        // end_rod <https://minecraft.wiki/w/End_Rod>;
        // copper_torch / copper_wall_torch — one state each (plus `facing` on
        // the wall form), no lit/unlit axis, 14 throughout.
        "torch" | "wall_torch" | "end_rod" | "copper_torch" | "copper_wall_torch" => 14,

        // --- unconditional 10 ---
        // soul_lantern <https://minecraft.wiki/w/Soul_Lantern>;
        // soul_torch / soul_wall_torch <https://minecraft.wiki/w/Soul_Torch>;
        // soul_fire <https://minecraft.wiki/w/Soul_Fire>;
        // crying_obsidian <https://minecraft.wiki/w/Crying_Obsidian>.
        "soul_lantern" | "soul_torch" | "soul_wall_torch" | "soul_fire" | "crying_obsidian" => 10,

        // --- unconditional 7 ---
        // enchanting_table <https://minecraft.wiki/w/Enchanting_Table>;
        // ender_chest <https://minecraft.wiki/w/Ender_Chest>.
        "enchanting_table" | "ender_chest" => 7,

        // --- sculk_catalyst 6, both `bloom` states ---
        "sculk_catalyst" => 6,

        // --- vault: the block entity owns `vault_state`, so this is the
        // MINIMUM over its states (6 inactive, 12 active/unlocking/ejecting) ---
        "vault" => 6,

        // --- state-dependent, and the only OVERESTIMATE the table ever had ---
        // glow_lichen is a multiface decal: it emits 7 only where it is attached
        // to at least one face. Its default state — every face `false`, which is
        // exactly what a bare `minecraft:glow_lichen` places — emits **0** in
        // vanilla, and the flat 7 this arm used to return was a light the game
        // does not give. Two of its 128 states are affected; the other 126 are
        // unchanged at 7.
        "glow_lichen" => {
            let attached = ["down", "up", "north", "south", "east", "west"]
                .iter()
                .any(|f| prop(name, f, "false") == "true");
            if attached { 7 } else { 0 }
        }

        // --- small unconditional sources ---
        // The amethyst family is 5/4/2/1 by growth stage, NOT a flat 7
        // <https://minecraft.wiki/w/Amethyst_Cluster>,
        // <https://minecraft.wiki/w/Amethyst_Bud>.
        "amethyst_cluster" => 5,
        "large_amethyst_bud" => 4,
        "medium_amethyst_bud" => 2,
        "small_amethyst_bud" => 1,
        // magma_block 3 <https://minecraft.wiki/w/Magma_Block>.
        "magma_block" => 3,
        // brewing_stand 1 <https://minecraft.wiki/w/Brewing_Stand> and
        // brown_mushroom 1 <https://minecraft.wiki/w/Brown_Mushroom> — both were
        // modelled at 3.
        "brewing_stand" | "brown_mushroom" => 1,
        // nether_portal 11, both `axis` values.
        "nether_portal" => 11,
        // firefly_bush 2 — a single state, no properties at all.
        "firefly_bush" => 2,
        // dragon_egg 1; end_portal_frame 1 whether or not it carries an `eye`;
        // sculk_sensor / calibrated_sculk_sensor 1 in every phase, power level
        // and waterlogging.
        "dragon_egg" | "end_portal_frame" | "sculk_sensor" | "calibrated_sculk_sensor" => 1,

        // --- state-dependent ---
        // campfire / soul_campfire: `lit` defaults to TRUE, so a bare
        // `/setblock minecraft:campfire` really is a 15-light block (this is what
        // makes the relight fixture work); an authored `lit=false` campfire is
        // cold and dark <https://minecraft.wiki/w/Campfire>,
        // <https://minecraft.wiki/w/Soul_Campfire>.
        "campfire" => lit(name, "true", 15),
        "soul_campfire" => lit(name, "true", 10),
        // redstone_torch / redstone_wall_torch: `lit` defaults to TRUE
        // <https://minecraft.wiki/w/Redstone_Torch>.
        "redstone_torch" | "redstone_wall_torch" => lit(name, "true", 7),
        // redstone_ore / deepslate_redstone_ore: `lit` defaults to FALSE and the
        // lit value is 9, not 7. Idle ore is DARK — the old flat 7 is the entry
        // most likely to have hidden a real dark cavern
        // <https://minecraft.wiki/w/Redstone_Ore>.
        "redstone_ore" | "deepslate_redstone_ore" => lit(name, "false", 9),
        // furnace family: `lit` defaults to FALSE; lit value is 13
        // <https://minecraft.wiki/w/Furnace>, <https://minecraft.wiki/w/Smoker>,
        // <https://minecraft.wiki/w/Blast_Furnace>.
        "furnace" | "smoker" | "blast_furnace" => lit(name, "false", 13),
        // respawn_anchor: 0 / 3 / 7 / 11 / 15 by `charges`, which defaults to 0 —
        // a placed anchor is dark until charged
        // <https://minecraft.wiki/w/Respawn_Anchor>.
        "respawn_anchor" => match prop(name, "charges", "0") {
            "1" => 3,
            "2" => 7,
            "3" => 11,
            "4" => 15,
            _ => 0,
        },
        // sea_pickle: light ONLY underwater, `3 + 3 * pickles`; `waterlogged`
        // defaults to true and `pickles` to 1, so a default pickle is 6 — not the
        // 15 the old table claimed, and 0 the moment it is dry
        // <https://minecraft.wiki/w/Sea_Pickle>.
        "sea_pickle" => {
            if prop(name, "waterlogged", "true") == "true" {
                let n: u8 = prop(name, "pickles", "1").parse().unwrap_or(1);
                3 + 3 * n.clamp(1, 4)
            } else {
                0
            }
        }
        // cave vines light only while they carry glow berries (`berries` defaults
        // to false) <https://minecraft.wiki/w/Glow_Berries>.
        "cave_vines" | "cave_vines_plant" => {
            if prop(name, "berries", "false") == "true" {
                14
            } else {
                0
            }
        }
        // The technical light block: its `level`, default 15
        // <https://minecraft.wiki/w/Light_(block)>.
        "light" => prop(name, "level", "15").parse().unwrap_or(15).min(15),
        // copper bulb: 15 / 12 / 8 / 4 by oxidation stage, and ONLY while `lit`,
        // which defaults to FALSE — a bulb ships dark unless it was authored
        // lit. `lit` is a latch, not a live redstone read: `CopperBulbBlock`'s
        // `onPlace` → `checkAndFlip` returns without touching it whenever the
        // neighbour signal already agrees with `powered`, so the shipped state
        // stands. Waxed and unwaxed share the value at each stage.
        "copper_bulb" | "waxed_copper_bulb" => lit(name, "false", 15),
        "exposed_copper_bulb" | "waxed_exposed_copper_bulb" => lit(name, "false", 12),
        "weathered_copper_bulb" | "waxed_weathered_copper_bulb" => lit(name, "false", 8),
        "oxidized_copper_bulb" | "waxed_oxidized_copper_bulb" => lit(name, "false", 4),

        // --- the candle families, matched by suffix ---
        // Seventeen candle ids (plain + sixteen dyed) and seventeen candle-cake
        // ids, and the pinned registry has no OTHER block whose id ends that
        // way, so the suffix is exact rather than convenient.
        //
        // A candle is **3 per candle, and only while `lit`** — `candles` runs
        // 1..=4 and defaults to 1, `lit` defaults to FALSE. So a candle placed
        // the way vanilla places one is DARK, and a room lit by four lit candles
        // measures 12. This is the entry that made a candlelit room unbuildable:
        // it was absent, so every candle in the game measured 0.
        //
        // A candle cake carries exactly one candle: 3 when `lit`, else 0.
        id if id == "candle_cake" || id.ends_with("_candle_cake") => lit(name, "false", 3),
        // An UNLIT candle is not matched here at all and falls through to the
        // `_ => 0` below, which is the whole point: a candle placed the way
        // vanilla places one is dark at any count.
        id if (id == "candle" || id.ends_with("_candle"))
            && prop(name, "lit", "false") == "true" =>
        {
            let n: u8 = prop(name, "candles", "1").parse().unwrap_or(1);
            3 * n.clamp(1, 4)
        }

        // NOT a light source, and not a block at all: `glow_item_frame` is an
        // ENTITY in Java and emits no block light (the glow is an emissive
        // texture). The old table's 7 came from the Bedrock-only `Luminous` row
        // <https://minecraft.wiki/w/Glow_Item_Frame>.
        //
        // Also deliberately 0, and NOT an omission — see the module note on what
        // the model evaluates: `redstone_lamp` (any neighbour update unlights a
        // shipped `lit=true` lamp) and `trial_spawner` (its block entity owns
        // the state, and the minimum over that state is 0).
        _ => 0,
    }
}

/// The emission of a `lit`-gated block: `bright` when the block's `lit` state is
/// `true`, else 0. `default_lit` spells out the block's own default `lit` value
/// (`"true"` for campfires and redstone torches, `"false"` for redstone ore and
/// the furnace family), so a bare id evaluates to the state vanilla would place.
fn lit(name: &str, default_lit: &str, bright: u8) -> u8 {
    if prop(name, "lit", default_lit) == "true" {
        bright
    } else {
        0
    }
}

/// Whether a block id lets light pass (for the flood estimate). Full opaque rock
/// and masonry block light; air, water, glass, small non-full blocks and plants
/// pass it. An id absent from the world's block map is air → passes. Unknown
/// blocks are treated as opaque (conservative — never overestimates light),
/// matching the cave-generator's estimator.
///
/// ## The occupancy-coupling invariant (task: trap-trigger false dark)
///
/// The conservative-opaque default is safe for a block **nav also treats as
/// impassable**: over-blocking light can only make `DW0210`/`DW0211` stricter.
/// It is *not* safe for a block [`crate::assembled::occupancy_of`] deliberately
/// leaves **passable**, because then a cell a player really stands in is measured
/// at light 0 while the game lights it normally — a manufactured `DW0210` that no
/// amount of relighting can clear. Hence the invariant, asserted by
/// `every_nav_passable_block_passes_light`:
///
/// > **every block class whose cell `occupancy_of` leaves player-occupiable must
/// > be light-passing here.**
///
/// The classes that are player-occupiable by construction are pressure plates /
/// tripwire / tripwire hooks ([`crate::assembled::is_passable_trap_trigger`] —
/// load-bearing for the `DW0342` trap-avoidability proof, which needs nav to route
/// a player *onto* the trigger), thin decoration ([`crate::assembled::is_thin_decoration`]
/// — carpets and 1–4-layer snow), and fence gates (open = a passable threshold,
/// closed = passable-with-use). All of them are listed below.
///
/// ## Vanilla evidence (Minecraft Java 1.21.11)
///
/// A block's light opacity is its `lightBlock` / "filter light" value: 15 for a
/// full solid-render cube, 0 for anything that is neither solid-render nor a full
/// collision cube. Verified against the pinned `minecraft-data` block table
/// (`harness/node_modules/minecraft-data/.../pc/1.21.9/blocks.json`, the newest
/// vendored dump; block light opacity is unchanged across 1.21.x):
/// `filterLight = 0` for all 16 `*_pressure_plate`, `tripwire`, `tripwire_hook`,
/// all 20 carpets, `snow` (the layer block), and all 12 `*_fence_gate` — against
/// `filterLight = 15` for the control set `stone` / `dirt` / `oak_planks` /
/// `cobblestone` / `deepslate` / `sand` / `gravel` / `obsidian` / `snow_block`.
///
/// ## Deliberately still opaque
///
/// Vanilla also reports `filterLight = 0` for fences, walls, buttons, levers,
/// rails, slabs, stairs, doors, trapdoors, chests, signs and banners — but
/// `occupancy_of` classifies every one of them **solid or tall**, i.e. impassable
/// and never a player-occupiable cell. Their opacity can therefore only make the
/// light gate stricter, never manufacture a false *pass*, so correcting them is a
/// gate-loosening accuracy change that belongs in its own reviewed PR rather than
/// riding along with this false-failure fix. (`oak_fence` predates this rule and
/// is left as-is for the same reason: removing it would tighten the gate, adding
/// its siblings would loosen it — neither is this PR's concern.)
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
            | "iron_chain"
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
            // --- nav-passable classes (the occupancy-coupling invariant) ---
            // Trap triggers: thin, non-collidable, `filterLight = 0`.
            | "tripwire"
            | "tripwire_hook"
            // Thin decoration: the snow *layer* block (`snow_block` is a full
            // opaque cube and is deliberately NOT here). Every carpet — dyed,
            // `moss_carpet`, `pale_moss_carpet` — is caught by the `_carpet`
            // suffix below.
            | "snow"
    ) || id.ends_with("_stained_glass")
        || id.ends_with("_stained_glass_pane")
        || id == "glass_pane"
        || id.ends_with("_pressure_plate")
        || id.ends_with("_carpet")
        || id.ends_with("_fence_gate")
        // The occupancy-coupling invariant, at the class level: every
        // no-collision plant is nav-passable — a walker's FEET can occupy a
        // tuft/flower/crop cell — so the light model must not call it opaque
        // (vanilla: filterLight = 0 for the whole class). An opaque flower
        // measures light 0 in a cell a body may legally stand in, which is a
        // DW0210 darkness that is not there.
        || crate::assembled::is_no_collision_plant(id)
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
    /// gate clears, and unsupported falling blocks settled. Relight
    /// therefore evaluates opacity/emission over the same world the game assembles,
    /// so a `sand` floor that fell into the void is air here, not phantom rock.
    pub fn from_plan(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Self {
        Self::from_blocks(crate::assembled::assembled_blocks(plan, structures))
    }

    /// Build directly from a cell→block map (test entry point; no plan needed),
    /// over the box the given cells occupy.
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
        LightModel::from_blocks_within(blocks, min, max)
    }

    /// Build from a cell→block map over an **explicitly stated** box.
    ///
    /// [`Self::from_blocks`] infers the box from the cells it was handed, which
    /// is right when those cells ARE the world. A caller measuring one piece in
    /// isolation knows something the cell map cannot say: the piece stands in
    /// open air, so the sky reaches the cells *beside* it as well as the cells
    /// above it. Stating a box larger than the piece is how that is expressed —
    /// the extra cells are absent, therefore air, therefore sky-open, and sky
    /// light floods inward through every opening exactly as it does in the game.
    ///
    /// Inferring the box instead is what makes a roofed-but-open building read
    /// pitch dark: every one of its cells has a roof above it, none is sky-open,
    /// and the daylight that reaches it in the game arrives from the side.
    pub fn from_blocks_within(
        blocks: BTreeMap<[i32; 3], String>,
        min: [i32; 3],
        max: [i32; 3],
    ) -> Self {
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
    ///
    /// Public because the daylight-burn proof ([`crate::daylight`], `DW0496`)
    /// asks the same question of the same assembled world: "the sky is above this
    /// cell" must have exactly one definition in the compiler, or the light model
    /// and the burn model can disagree about the same roof.
    pub fn sky_open(&self, c: [i32; 3]) -> bool {
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
    ///
    /// Public because it is the compiler's ONE light flood, and `delve-admit`'s
    /// per-piece probe asks the same question of a single prefab. A second copy
    /// of it is what shipped a probe with no sky term at all.
    ///
    /// The work is done by [`LightField`], which is the same field in a dense,
    /// index-addressed form; this entry point exists for callers that want one
    /// measurement and a map to read it out of.
    pub fn flood(&self, effective_sky: u8) -> BTreeMap<[i32; 3], u8> {
        LightField::new(self, effective_sky).clone_map()
    }
}

// ---------------------------------------------------------------------------
// The flooded field, densely
// ---------------------------------------------------------------------------

/// Bit: this cell lets light pass ([`passes_light`]).
const P_PASSES: u8 = 0b1000_0000;
/// Bit: this cell has open sky above it ([`LightModel::sky_open`]).
const P_SKY: u8 = 0b0100_0000;
/// Mask: the cell's block-light emission ([`emission`], 0..=15).
const P_EMISSION: u8 = 0b0000_1111;

/// The flooded light field of a [`LightModel`], **densely indexed** over the
/// model's AABB.
///
/// # Why this exists rather than a map
///
/// A [`LightModel`] answers every question about a cell by looking a
/// `BTreeMap<[i32; 3], String>` up and reading the block id out of it, and the
/// relight pass asks those questions a great many times: once per cell of the
/// whole AABB to seed the flood, once per column step to decide whether the sky
/// reaches a cell, and once more per cell visited by the frontier — and then the
/// whole flood again for every fixture it places. The comparisons are string
/// comparisons and the lookups are tree descents, so a map of a million cells
/// costs a hundred million of them to light once, and a build of the metrics gym
/// spent better than 99% of its wall clock inside exactly that.
///
/// Nothing about the *answers* changes here. Two things about the questions do:
///
/// 1. Everything the flood needs to know about a cell is **three facts and a
///    number** — does light pass through it, is the sky above it, what does it
///    emit — so each cell is one byte, resolved from its block id once when the
///    field is built and never looked up again. A cell's byte is found by
///    arithmetic on its coordinates rather than by descending a tree.
/// 2. Light only ever **increases** when a fixture is placed into a cell whose
///    passability the fixture does not change, so the field after such a
///    placement is the old field with one brighter source flooded into it. That
///    is the [`Self::set`] fast path; a placement that changes passability (a
///    `shroomlight` written over a pane of glass, say) cannot use it and floods
///    the field again from nothing.
///
/// # The border
///
/// The array is padded by one cell on every face, and the padding is
/// permanently non-passing. That is what the AABB test used to do — light does
/// not flow out of the assembled bounds — expressed as data, so the frontier
/// walk needs no bounds test at all.
struct LightField {
    /// The model AABB's lower corner (the cell at padded index `[1, 1, 1]`).
    min: [i32; 3],
    /// Padded dimensions, `dim[a] = max[a] - min[a] + 3`.
    dim: [usize; 3],
    /// One step along z, `dim[0]`. (One step along x is 1.)
    stride_z: usize,
    /// One step along y, `dim[0] * dim[2]`.
    stride_y: usize,
    /// Effective sky light at a sky-open cell, as [`LightModel::flood`] takes it.
    sky: u8,
    /// Per padded cell: [`P_PASSES`] | [`P_SKY`] | [`P_EMISSION`].
    prop: Vec<u8>,
    /// Per padded cell: the flooded light value.
    light: Vec<u8>,
}

impl LightField {
    /// Build and flood the field of `model` under `effective_sky`.
    fn new(model: &LightModel, effective_sky: u8) -> Self {
        let dim = [
            (model.max[0] - model.min[0]) as usize + 3,
            (model.max[1] - model.min[1]) as usize + 3,
            (model.max[2] - model.min[2]) as usize + 3,
        ];
        let cells = dim[0] * dim[1] * dim[2];
        let mut f = LightField {
            min: model.min,
            dim,
            stride_z: dim[0],
            stride_y: dim[0] * dim[2],
            sky: effective_sky,
            // Every interior cell starts as air — passing, emitting nothing. The
            // padding is overwritten to 0 (non-passing) right after.
            prop: vec![P_PASSES; cells],
            light: vec![0; cells],
        };
        f.seal_border();

        // Resolve each DISTINCT block id once. The assembled world is overwhelmingly
        // repeated ids, and `emission`/`passes_light` are a pair of string matches.
        let mut resolved: BTreeMap<&str, u8> = BTreeMap::new();
        for (c, name) in &model.blocks {
            let Some(i) = f.index(*c) else { continue };
            let packed = *resolved
                .entry(name.as_str())
                .or_insert_with(|| Self::pack(name));
            f.prop[i] = packed;
        }

        f.compute_sky();
        f.flood();
        f
    }

    /// The padding is non-passing, which is the AABB test as data.
    fn seal_border(&mut self) {
        let (nx, ny, nz) = (self.dim[0], self.dim[1], self.dim[2]);
        for y in 0..ny {
            for z in 0..nz {
                let row = y * self.stride_y + z * self.stride_z;
                if y == 0 || y == ny - 1 || z == 0 || z == nz - 1 {
                    self.prop[row..row + nx].fill(0);
                } else {
                    self.prop[row] = 0;
                    self.prop[row + nx - 1] = 0;
                }
            }
        }
    }

    /// The padded index of a world cell, or `None` when it is outside the AABB.
    fn index(&self, c: [i32; 3]) -> Option<usize> {
        let mut off = [0usize; 3];
        for a in 0..3 {
            // `+ 1` for the border; widened so a cell far outside the AABB is a
            // `None` rather than an overflow.
            let d = i64::from(c[a]) - i64::from(self.min[a]) + 1;
            if d < 1 || d >= self.dim[a] as i64 - 1 {
                return None;
            }
            off[a] = d as usize;
        }
        Some(off[1] * self.stride_y + off[2] * self.stride_z + off[0])
    }

    /// A block id as one cell's byte: what it passes, what it emits.
    fn pack(block: &str) -> u8 {
        let e = emission(block);
        debug_assert!(e <= 15, "block light is 0..=15; `{block}` measured {e}");
        let mut p = e & P_EMISSION;
        if passes_light(block) {
            p |= P_PASSES;
        }
        p
    }

    /// [`LightModel::sky_open`] for every cell at once: walking a column
    /// downwards, the sky reaches a cell exactly while nothing opaque has been
    /// passed on the way down.
    fn compute_sky(&mut self) {
        let (nx, ny, nz) = (self.dim[0], self.dim[1], self.dim[2]);
        for z in 1..nz - 1 {
            for x in 1..nx - 1 {
                let mut open = true;
                for y in (1..ny - 1).rev() {
                    let i = y * self.stride_y + z * self.stride_z + x;
                    if open {
                        self.prop[i] |= P_SKY;
                    } else {
                        self.prop[i] &= !P_SKY;
                    }
                    if self.prop[i] & P_PASSES == 0 {
                        open = false;
                    }
                }
            }
        }
    }

    /// A cell's seed brightness: what it emits, or the sky above it, whichever is
    /// more.
    fn seed_at(&self, i: usize) -> u8 {
        let p = self.prop[i];
        let mut seed = p & P_EMISSION;
        if self.sky > 0 && p & P_PASSES != 0 && p & P_SKY != 0 {
            seed = seed.max(self.sky);
        }
        seed
    }

    /// Flood the whole field from every seed.
    fn flood(&mut self) {
        self.light.fill(0);
        let mut buckets: [Vec<u32>; 16] = Default::default();
        let (nx, ny, nz) = (self.dim[0], self.dim[1], self.dim[2]);
        for y in 1..ny - 1 {
            for z in 1..nz - 1 {
                let row = y * self.stride_y + z * self.stride_z;
                for i in row + 1..row + nx - 1 {
                    let seed = self.seed_at(i);
                    if seed > 0 {
                        self.light[i] = seed;
                        buckets[seed as usize].push(i as u32);
                    }
                }
            }
        }
        self.drain(buckets);
    }

    /// Drain a frontier held in per-brightness buckets, brightest first.
    ///
    /// Bucket order is not a tie-break: the flooded field is the pointwise
    /// maximum over every seed of `seed − distance`, which is one value per cell
    /// however the frontier is walked. Draining brightest-first only means each
    /// cell is relaxed once — a cell reached at its final value can never be
    /// improved later, because every later step is dimmer.
    fn drain(&mut self, mut buckets: [Vec<u32>; 16]) {
        let steps = [
            1isize,
            -1,
            self.stride_y as isize,
            -(self.stride_y as isize),
            self.stride_z as isize,
            -(self.stride_z as isize),
        ];
        // A value of 1 lights nothing (its neighbours would be 0, which they
        // already are), so the walk stops at 2.
        for l in (2..=15usize).rev() {
            let cur = std::mem::take(&mut buckets[l]);
            let nl = (l - 1) as u8;
            for &i in &cur {
                let i = i as usize;
                if self.light[i] as usize != l {
                    continue;
                }
                for s in steps {
                    let j = i.wrapping_add_signed(s);
                    if self.prop[j] & P_PASSES == 0 || self.light[j] >= nl {
                        continue;
                    }
                    self.light[j] = nl;
                    buckets[nl as usize].push(j as u32);
                }
            }
        }
    }

    /// The flooded light at a world cell (0 outside the AABB, as the map form's
    /// `unwrap_or(0)` readers already treat it).
    fn light_at(&self, c: [i32; 3]) -> u8 {
        self.index(c).map(|i| self.light[i]).unwrap_or(0)
    }

    /// Record that `block` was written at `c`, and re-establish the field.
    ///
    /// Placing a block that passes light exactly as the one it replaced did, and
    /// emits at least as much, can only make the field brighter: no distance
    /// changes, no column's sky changes, one seed rises. The new field is then
    /// the old one with that seed flooded into it, and every cell it does not
    /// reach keeps the value it had.
    ///
    /// A placement that fails either half of that — a `shroomlight` written over
    /// a pane of glass darkens the room behind it — is not an increase anywhere,
    /// so the field is built again from nothing.
    fn set(&mut self, c: [i32; 3], block: &str) {
        let Some(i) = self.index(c) else { return };
        let old = self.prop[i];
        let packed = Self::pack(block);
        self.prop[i] = packed | (old & P_SKY);
        if old & P_PASSES == packed & P_PASSES && packed & P_EMISSION >= old & P_EMISSION {
            let seed = self.seed_at(i);
            if seed > self.light[i] {
                self.light[i] = seed;
                let mut buckets: [Vec<u32>; 16] = Default::default();
                buckets[seed as usize].push(i as u32);
                self.drain(buckets);
            }
        } else {
            self.compute_sky();
            self.flood();
        }
    }

    /// The field as the map [`LightModel::flood`] hands out: one entry per lit
    /// cell, none for a cell the flood never reached.
    fn clone_map(&self) -> BTreeMap<[i32; 3], u8> {
        let mut out = BTreeMap::new();
        let (nx, ny, nz) = (self.dim[0], self.dim[1], self.dim[2]);
        for y in 1..ny - 1 {
            for z in 1..nz - 1 {
                let row = y * self.stride_y + z * self.stride_z;
                for x in 1..nx - 1 {
                    let l = self.light[row + x];
                    if l > 0 {
                        out.insert(
                            [
                                self.min[0] + x as i32 - 1,
                                self.min[1] + y as i32 - 1,
                                self.min[2] + z as i32 - 1,
                            ],
                            l,
                        );
                    }
                }
            }
        }
        out
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
        // Dusk (12000) is the sun going down and dawn (23000) is still before
        // sunrise, so both are held at the vanilla night floor. The sky at those two
        // instants is in fact brighter than midnight, so this is the CONSERVATIVE
        // reading — it can only make the `dark`-needs-mitigation proof stricter,
        // never weaker.
        WorldTime::Dusk | WorldTime::Night | WorldTime::Midnight | WorldTime::Dawn => 4,
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
    let (times, weathers) = reachable_time_weather(c);
    let mut darkest = 15u8;
    for &t in &times {
        for &w in &weathers {
            darkest = darkest.min(effective_sky(t, w));
        }
    }
    darkest
}

/// Every `(time, weather)` state the campaign can be in: the declared initial
/// state plus every reachable `set-time` / `set-weather` target, as two
/// independent sets (they switch independently).
///
/// The single scan behind both [`darkest_effective_sky`] (spec-0010's darkness
/// gate, which takes the worst of them) and [`crate::daylight::daylight_is_pinned`]
/// (`DW0496`, which asks whether they ALL burn). One reader of the campaign's
/// clock, so the two proofs can never disagree about what hours a delve reaches.
///
/// Deterministic (ADR-0006): collected through `BTreeSet`s keyed on a stable
/// discriminant, so the returned order is the declaration order of the enums and
/// never hash order.
pub fn reachable_time_weather(c: &Campaign) -> (Vec<WorldTime>, Vec<WorldWeather>) {
    let mut times: BTreeSet<u8> = BTreeSet::new(); // discriminant via token order
    let mut weathers: BTreeSet<u8> = BTreeSet::new();
    // Exhaustive both ways (no wildcard arm): adding a `WorldTime` variant fails to
    // compile until it is given a discriminant HERE and a case in `time_of` below,
    // so the reachable-state scan can never silently skip a new time state.
    let add_t = |t: WorldTime, set: &mut BTreeSet<u8>| {
        set.insert(match t {
            WorldTime::Day => 0,
            WorldTime::Noon => 1,
            WorldTime::Dusk => 2,
            WorldTime::Night => 3,
            WorldTime::Midnight => 4,
            WorldTime::Dawn => 5,
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
    // Quest effects — every root, every depth. This scan hand-listed three of the
    // five roots AND was shallow, so a `set-time` inside a `sequence` step or an
    // `on_respawn` bundle was invisible to it. Under-reporting the reachable state
    // set is the direction that PASSES a delve that goes dark (spec-0010,
    // `DW0496`), which is why the shallow half mattered as much as the root half.
    delvewright_dsl::for_each_campaign_effect(c, &mut |_path, _site, e| {
        if let Some(t) = e.set_time() {
            add_t(t, &mut times);
        }
        if let Some(w) = e.set_weather() {
            add_w(w, &mut weathers);
        }
    });
    // Dialogue effects. NOT a root: a `DialogueEffect::SetTime` / `SetWeather` is a
    // flat outcome of a conversation, in the dialogue vocabulary rather than the
    // quest one, so the root walk above does not and should not reach it. (What it
    // DOES reach in this stage is the `set-checkpoint` `on_respawn` bundle nested
    // inside one of these effects, which is quest-effect vocabulary.)
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
        2 => WorldTime::Dusk,
        3 => WorldTime::Night,
        4 => WorldTime::Midnight,
        _ => WorldTime::Dawn,
    };
    let weather_of = |d: u8| match d {
        0 => WorldWeather::Clear,
        1 => WorldWeather::Rain,
        _ => WorldWeather::Thunder,
    };
    (
        times.into_iter().map(time_of).collect(),
        weathers.into_iter().map(weather_of).collect(),
    )
}

// ---------------------------------------------------------------------------
// Night-vision mitigation (DSL v0.6 declaration)
// ---------------------------------------------------------------------------

/// Whether an area declares the `night-vision` darkness mitigation (DSL v0.6).
///
/// This is the **whole** night-vision signal. The pre-0.6 heuristic read a class
/// kit item's id or display *name* for `night_vision` / `night vision`, which
/// accepted a bare `minecraft:potion` renamed "Potion of Night Vision" — a water
/// bottle. `DW0210` passed while nothing in the shipped world granted night vision
/// (owner, island QA). The declaration is now both the gate's input and the thing
/// the compiler emits (`emit::night_vision_fns`), so the check cannot pass without
/// the feature existing.
///
/// It is also language-independent by construction: a declaration is not a
/// player-facing string, so `--lang` can never move the `DW0210` verdict (ADR-0006)
/// and no verdict has to be threaded around the localization pass any more.
pub fn area_night_vision(area: &delvewright_dsl::Area) -> bool {
    matches!(area.mitigation, Some(AreaMitigation::NightVision))
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
///
/// The night-vision mitigation is read **per area** from its `mitigation`
/// declaration ([`area_night_vision`]) — a schema field, never a player-facing
/// string — so the `DW0210` verdict is identical in every build language by
/// construction, with nothing to thread past the localization pass (ADR-0006).
pub fn relight(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Relight {
    relight_over(plan, &crate::assembled::assemble(plan, structures))
}

/// [`relight`] over an already-assembled (possibly **edited**) world model —
/// the spec-0017 re-entry point: after every edit batch, and for the final
/// build of an edited campaign, the relight pass runs over the edited geometry
/// instead of re-deriving the pristine assembly. Behavior-identical to
/// [`relight`] for an unedited world (both derive from the same
/// [`crate::assembled::Assembled`]).
pub fn relight_over(plan: &Plan, assembled: &crate::assembled::Assembled) -> Relight {
    let c = plan.campaign;
    let sky = darkest_effective_sky(c);
    // Which surfaces this campaign HAS to declare lighting on, asked once. A
    // refusal below prescribes only those; see `delvewright_dsl::placement`.
    let placement = delvewright_dsl::Placement::of(c);

    // The base assembled geometry (nav) and required-path cells fixtures must avoid.
    let nav = World::from_occupancy(crate::assembled::occupancy_of(
        assembled.blocks.clone(),
        &assembled.open_gates,
    ));
    // move-npc waypoint cells are part of the required paths; plan them on the base
    // world (an unroutable move is a separate DW0307 handled by emit — here we
    // just collect paths, ignoring routing errors).
    let moves = crate::nav::plan_moves(plan, &nav).unwrap_or_default();
    let required = nav.required_path_cells(plan, &moves);

    let mut model = LightModel::from_blocks(assembled.blocks.clone());
    let mut out = Relight::default();
    // The dark set of the whole build, kept per area and reported once at the end
    // (`dark_diagnostic`). Accumulated rather than raised per area because the
    // build fails on its first diagnostic: raising one per area meant every area
    // but the alphabetically-first was measured and thrown away.
    let mut dark: BTreeMap<String, DarkSurvey> = BTreeMap::new();

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
        // A site-plan campaign has no `areas[]` to carry a lighting declaration
        // — `DW0839` refuses one that does — and states its one setting on the
        // plan instead, so a blockout interior is walkable at night without
        // per-box surface. Read here rather than copied into a synthetic `Area`,
        // because `AreaLighting` is already the engine's "which fixture, to what
        // level" object and this pass is already the one that consumes it.
        let lighting = match dsl_area {
            Some(a) => a.lighting,
            None if area.area_id == delvewright_dsl::SITE_AREA => {
                c.site_plan.as_ref().and_then(|p| p.content.lighting)
            }
            None => None,
        };

        // **The fixture pass applies to DERIVED interiors only** (spec-0050 §3).
        //
        // A detail piece's frame is the piece's, and lighting is part of what a
        // place looks like: the whole hanging its own torches inside a building
        // somebody designed would be the whole writing inside a bound frame,
        // which is the one thing the fabric split says it does not do.
        //
        // The cells are not simply dropped, or a dark detailed place would be a
        // silence rather than a finding. They go to `survey_undeclared`, which
        // is the gate an area with no declaration already gets — so the piece
        // lights itself and is judged on whether it did.
        //
        // Empty for every campaign with no detail plan, so nothing moves for
        // anybody who has not opted in.
        let frames: Vec<([i32; 3], [i32; 3])> = if area.area_id == delvewright_dsl::SITE_AREA {
            let mut reads = delvewright_dsl::metrics::Reads::new();
            delvewright_dsl::placed_boxes(c, &mut reads)
                .iter()
                .filter(|b| delvewright_dsl::is_bound(c, &b.node))
                .map(|b| {
                    let f = delvewright_dsl::Frame::of(b);
                    (
                        [f.lo[0] as i32, f.lo[1] as i32, f.lo[2] as i32],
                        [f.hi[0] as i32, f.hi[1] as i32, f.hi[2] as i32],
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let detailed: BTreeSet<[i32; 3]> = reachable
            .iter()
            .copied()
            .filter(|c| frames.iter().any(|(lo, hi)| in_bounds(*c, *lo, *hi)))
            .collect();
        let reachable: BTreeSet<[i32; 3]> = reachable.difference(&detailed).copied().collect();

        if !reachable.is_empty() {
            match lighting {
                Some(spec) => {
                    relight_area(
                        &mut model,
                        &nav,
                        &reachable,
                        &required,
                        &area.area_id,
                        placement,
                        spec,
                        sky,
                        amin,
                        amax,
                        &mut out,
                    );
                }
                None => {
                    // Measured-darkness gate over the assembled reachable walkable cells.
                    let night_vision = dsl_area.is_some_and(area_night_vision);
                    let s = survey_undeclared(&model, &reachable, sky, night_vision);
                    dark.entry(area.area_id.clone()).or_default().absorb(&s);
                }
            }
        }

        // **A detailed place is judged LAST**, over the model the pass leaves
        // behind. Light does not stop at a frame boundary: a piece standing off a
        // corridor the whole has just hung torches in is lit by them, and
        // measuring before that would report it dark for want of a fixture this
        // same pass was about to place three cells away. Ordering is the whole of
        // the difference — the set and the gate are the same either way.
        if !detailed.is_empty() {
            let s = survey_undeclared(&model, &detailed, sky, false);
            dark.entry(area.area_id.clone()).or_default().absorb(&s);
        }
    }

    if let Some(diag) = dark_diagnostic(&dark, &nav, sky, placement) {
        out.diagnostics.push(diag);
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
pub(crate) fn relight_area(
    model: &mut LightModel,
    nav: &World,
    reachable: &BTreeSet<[i32; 3]>,
    required: &BTreeSet<[i32; 3]>,
    area_id: &str,
    placement: delvewright_dsl::Placement,
    spec: AreaLighting,
    sky: u8,
    amin: [i32; 3],
    amax: [i32; 3],
    out: &mut Relight,
) {
    let min_light = spec.min_light;
    // The field is flooded ONCE and carried through the loop: every fixture this
    // pass writes is a brighter source in a world whose geometry it does not
    // change, so [`LightField::set`] extends the measurement instead of taking it
    // again. The verdict is the same field either way — see its own doc comment
    // for the one case that cannot be extended and floods again.
    let mut field = LightField::new(model, sky);
    // The reachable cells in the order the darkest-cell rule breaks its ties:
    // ascending (y, z, x). Sorted once, so the scan below can keep the first cell
    // that attains the minimum rather than comparing coordinates every time.
    let mut order: Vec<[i32; 3]> = reachable.iter().copied().collect();
    order.sort_unstable_by_key(|c| [c[1], c[2], c[0]]);
    // A bounded loop: every iteration writes one fixture cell that was previously
    // unoccupied, so it terminates (cells are finite) — but cap for safety.
    let cap = (reachable.len() + 8) * 4;
    for _ in 0..cap {
        // Darkest deficient reachable cell, ties by ascending (y, z, x).
        let mut worst: Option<([i32; 3], u8)> = None;
        for &cell in &order {
            let l = field.light_at(cell);
            if l >= min_light {
                continue;
            }
            match worst {
                Some((_, wl)) if wl <= l => {}
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
                field.set(site.pos, &site.block);
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
                         has no valid placement site left. Fix in {}: \
                         choose a fixture that fits the geometry (`lantern`/`shroomlight` need \
                         less clearance than `torch`/`campfire`), lower the declared `min_light` \
                         (still within 1..=14), or open the room so a fixture site exists. Do NOT \
                         relax this by widening the reachable set — the cell is genuinely lit \
                         below target (spec-0010 DW0211)",
                        spec.fixture.token(),
                        placement.lighting_field()
                    ),
                });
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A brightness measurement over a set of cells
// ---------------------------------------------------------------------------

/// One **contiguous run of dark cells**: the cells a body can walk between
/// without ever leaving the dark, with the box they span and the darkest of
/// them.
///
/// A region is what "a room" means to the person who has to go and fix it, and
/// it is the unit the mitigation remedy is spent on — a fixture, a brighter
/// scene or a night-vision declaration is chosen per place, not per cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DarkRegion {
    /// How many dark cells the region holds.
    pub cells: usize,
    /// Lower corner of the region's axis-aligned extent.
    pub lo: [i32; 3],
    /// Upper corner of the region's axis-aligned extent.
    pub hi: [i32; 3],
    /// The darkest cell of the region and its measured light.
    pub darkest: ([i32; 3], u8),
}

/// A **brightness measurement over a set of world cells**: how many cells were
/// examined, and which of them measured below a threshold, each with the light
/// it actually has.
///
/// The object this belongs to is the *set of cells*, not any one diagnostic. A
/// gate that measures a set already holds every dark cell in it; folding that
/// set down to a single coordinate at the moment of reporting is where the
/// information is lost, and a designer told "one cell" and a designer told
/// "six places, and this one is 98% dark" do entirely different things next.
/// Whatever asks the question keeps the answer here and decides for itself how
/// much of it to say.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DarkSurvey {
    /// How many cells were measured (the denominator every count here is of).
    pub examined: usize,
    /// The cells that measured below the threshold, with their light.
    pub dark: BTreeMap<[i32; 3], u8>,
}

impl DarkSurvey {
    /// Measure `cells` against `model` under `sky`, keeping every cell below
    /// `below`. A cell the flood never reached measures 0 — the never-overestimate
    /// direction (see [`emission`]).
    pub fn measure(
        model: &LightModel,
        cells: &BTreeSet<[i32; 3]>,
        sky: u8,
        below: u8,
    ) -> DarkSurvey {
        let light = model.flood(sky);
        let mut out = DarkSurvey {
            examined: cells.len(),
            dark: BTreeMap::new(),
        };
        for &cell in cells {
            let l = light.get(&cell).copied().unwrap_or(0);
            if l < below {
                out.dark.insert(cell, l);
            }
        }
        out
    }

    /// Fold another measurement of a **disjoint** set of the same place into this
    /// one: the denominators add and the dark sets union. This is what lets an
    /// area measured in two passes (its own cells, then the cells inside a bound
    /// detail frame, which are judged after the pass has hung its fixtures) be
    /// reported as the one place a designer has to walk back into.
    pub fn absorb(&mut self, other: &DarkSurvey) {
        self.examined += other.examined;
        for (&cell, &l) in &other.dark {
            self.dark.insert(cell, l);
        }
    }

    /// The darkest cell measured, ties broken by ascending `[x, y, z]`. `None`
    /// when nothing measured dark.
    pub fn darkest(&self) -> Option<([i32; 3], u8)> {
        self.dark
            .iter()
            .map(|(&cell, &l)| (cell, l))
            .min_by_key(|&(cell, l)| (l, cell))
    }

    /// The dark cells grouped into contiguous regions, largest first (ties by
    /// ascending `lo`).
    ///
    /// Adjacency is [`World::neighbors`] — the engine's ONE answer to "can a body
    /// get from here to there", which is also the relation that produced the
    /// reachable set being measured. A second, private adjacency rule here (six
    /// face neighbours, say) would split a staircase into one region per step and
    /// would be a step rule this compiler proves nothing else under.
    ///
    /// The relation is **symmetrised** before the walk: a rise can be walkable
    /// downhill and refused uphill, so the directed edges alone would make the
    /// grouping depend on which cell the walk started from. Undirected components
    /// are order-independent, which is what determinism (ADR-0006) needs.
    pub fn regions(&self, nav: &World) -> Vec<DarkRegion> {
        let mut adj: BTreeMap<[i32; 3], BTreeSet<[i32; 3]>> = BTreeMap::new();
        for &cell in self.dark.keys() {
            adj.entry(cell).or_default();
            for n in nav.neighbors(cell) {
                if self.dark.contains_key(&n) {
                    adj.entry(cell).or_default().insert(n);
                    adj.entry(n).or_default().insert(cell);
                }
            }
        }

        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut out: Vec<DarkRegion> = Vec::new();
        for &start in self.dark.keys() {
            if !seen.insert(start) {
                continue;
            }
            let mut queue: VecDeque<[i32; 3]> = VecDeque::from([start]);
            let mut lo = start;
            let mut hi = start;
            let mut cells = 0usize;
            let mut darkest = (start, self.dark[&start]);
            while let Some(cur) = queue.pop_front() {
                cells += 1;
                for a in 0..3 {
                    lo[a] = lo[a].min(cur[a]);
                    hi[a] = hi[a].max(cur[a]);
                }
                let l = self.dark[&cur];
                if (l, cur) < (darkest.1, darkest.0) {
                    darkest = (cur, l);
                }
                for &n in adj.get(&cur).into_iter().flatten() {
                    if seen.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
            out.push(DarkRegion {
                cells,
                lo,
                hi,
                darkest,
            });
        }
        out.sort_by_key(|r| (std::cmp::Reverse(r.cells), r.lo, r.hi));
        out
    }
}

/// The measured-darkness survey for an **undeclared** area (spec-0010 mitigation
/// hierarchy step 4): every reachable walkable cell below [`DARK_THRESHOLD`] under
/// the darkest reachable sky. Empty when the area declares the night-vision
/// mitigation. Sealed cavities are not in `reachable`, so they are never counted.
///
/// The verdict is unchanged from the fold this replaced: "the darkest cell is
/// below the threshold" and "some cell is below the threshold" are the same
/// predicate. What changed is that the cells that satisfied it are kept.
fn survey_undeclared(
    model: &LightModel,
    reachable: &BTreeSet<[i32; 3]>,
    sky: u8,
    night_vision: bool,
) -> DarkSurvey {
    if night_vision {
        return DarkSurvey::default();
    }
    DarkSurvey::measure(model, reachable, sky, DARK_THRESHOLD)
}

/// How many dark areas the `DW0210` report lists in full before it summarises the
/// rest, and how many regions it lists per area. A truncated list that does not
/// say it was truncated reads as "here is everything", so both caps are stated in
/// the message whenever they bite.
const DARK_AREAS_LISTED: usize = 8;
/// See [`DARK_AREAS_LISTED`].
const DARK_REGIONS_LISTED: usize = 4;

/// Build the one `DW0210` diagnostic for a whole build from the per-area surveys,
/// or `None` when nothing measured dark.
///
/// **One diagnostic, not one per area.** A build fails on its *first* diagnostic
/// (`emit::BuildFailure` carries one code and one message, as every other check in
/// this compiler does), so a per-area diagnostic per dark area meant the sort
/// picked the alphabetically-first area's message and the rest were never printed
/// at all. The object this gate measures is the campaign's dark set; the report
/// now says so, and nothing about the fail-fast build channel had to move.
///
/// **The remedy it prescribes is the one this author can reach.** `placement` is
/// the campaign's single placement authority, asked once in [`relight_over`] —
/// one campaign, one authority, so it belongs to the whole-build report exactly
/// as the report itself does, rather than being asked again per area.
fn dark_diagnostic(
    areas: &BTreeMap<String, DarkSurvey>,
    nav: &World,
    sky: u8,
    placement: delvewright_dsl::Placement,
) -> Option<LightDiag> {
    let mut dark: Vec<(&String, &DarkSurvey)> =
        areas.iter().filter(|(_, s)| !s.dark.is_empty()).collect();
    if dark.is_empty() {
        return None;
    }
    // Worst first: the area with the most dark cells is the one whose remedy is a
    // different act. Ties by area id, which is unique, so the order is total.
    dark.sort_by_key(|(id, s)| (std::cmp::Reverse(s.dark.len()), (*id).clone()));

    let total: usize = dark.iter().map(|(_, s)| s.dark.len()).sum();
    let examined: usize = dark.iter().map(|(_, s)| s.examined).sum();
    let (wcell, wl) = dark
        .iter()
        .filter_map(|(_, s)| s.darkest())
        .min_by_key(|&(cell, l)| (l, cell))
        .expect("a non-empty dark set has a darkest cell");

    // **A refusal offers only the surfaces this campaign HAS.** `mitigation`
    // lives on an `areas[]` entry, which a site-plan campaign is required to
    // leave empty (`DW0839`), so on a derived map the third way does not exist
    // and naming it sends the author at a document another gate refuses. The
    // count moves with the list rather than being written down beside it, so a
    // surface added or removed later cannot leave the sentence claiming a way
    // that is not offered.
    let (declared, ways, mitigation) = if placement.has_area_mitigation() {
        (
            " and no `mitigation`",
            "three ways",
            ", or declare `world.areas[].mitigation: \"night-vision\"` on it (the compiler then \
             emits a clocked `effect give … night_vision` over its bounds)",
        )
    } else {
        ("", "two ways", "")
    };

    let n = dark.len();
    let mut m = format!(
        "{n} area(s) measure dark: {total} of the {examined} reachable walkable cell(s) measured \
         in them are below light {DARK_THRESHOLD} under the darkest reachable (time, weather) sky \
         (effective {sky}), with no `lighting`{declared} declaration. The darkest \
         reachable walkable cell is at {wcell:?}, measured at light {wl} \
         (< {DARK_THRESHOLD}).\n"
    );
    for (id, s) in dark.iter().take(DARK_AREAS_LISTED) {
        let regions = s.regions(nav);
        m.push_str(&format!(
            "  area `{id}`: {d} of {e} measured cell(s) dark, in {r} contiguous region(s):\n",
            d = s.dark.len(),
            e = s.examined,
            r = regions.len(),
        ));
        for reg in regions.iter().take(DARK_REGIONS_LISTED) {
            m.push_str(&format!(
                "    - {c} cell(s) spanning {lo:?}..{hi:?}, darkest {dc:?} at light {dl}\n",
                c = reg.cells,
                lo = reg.lo,
                hi = reg.hi,
                dc = reg.darkest.0,
                dl = reg.darkest.1,
            ));
        }
        if regions.len() > DARK_REGIONS_LISTED {
            let rest: usize = regions
                .iter()
                .skip(DARK_REGIONS_LISTED)
                .map(|r| r.cells)
                .sum();
            m.push_str(&format!(
                "    - … and {k} further region(s) covering {rest} cell(s), not listed\n",
                k = regions.len() - DARK_REGIONS_LISTED,
            ));
        }
    }
    if n > DARK_AREAS_LISTED {
        let rest: usize = dark
            .iter()
            .skip(DARK_AREAS_LISTED)
            .map(|(_, s)| s.dark.len())
            .sum();
        m.push_str(&format!(
            "  … and {k} further dark area(s) covering {rest} cell(s), not listed\n",
            k = n - DARK_AREAS_LISTED,
        ));
    }
    m.push_str(&format!(
        "Mitigate each area one of {ways}: declare {} (a relight \
         `fixture` + `min_light`) for it, brighten the scene (`world.time`/`weather`)\
         {mitigation} — or re-arrange the room, or raise the \
         density of what is already lit in it, and measure again. A renamed potion in a class kit \
         is NOT a mitigation — it grants nothing. Do NOT lower `DARK_THRESHOLD` or trim the \
         reachable set — the darkness is real (spec-0010 DW0210)",
        placement.lighting_field()
    ));
    Some(LightDiag {
        code: DW_DARK_UNMITIGATED,
        message: m,
    })
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

pub(crate) fn in_bounds(c: [i32; 3], min: [i32; 3], max: [i32; 3]) -> bool {
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

    /// A nav world built through the **real** collision classifier
    /// ([`crate::assembled::occupancy_of`]), so nav-passable blocks (pressure
    /// plates, tripwire, carpets, fence gates) keep their cells walkable exactly
    /// as the shipped model does — unlike [`nav_of`], which force-solids every
    /// non-air cell.
    fn nav_occ_of(map: &BTreeMap<[i32; 3], String>) -> World {
        World::from_occupancy(crate::assembled::occupancy_of(
            map.clone(),
            &BTreeSet::new(),
        ))
    }

    /// [`reachable_of`] over the real collision classifier, seeded at `start`.
    fn reachable_occ_of(map: &BTreeMap<[i32; 3], String>, start: [i32; 3]) -> BTreeSet<[i32; 3]> {
        nav_occ_of(map).reachable_walkable(&[start])
    }

    /// The `DW0210` diagnostic one undeclared area's survey produces, or `None`.
    /// The whole-build report ([`dark_diagnostic`]) applied to a single area, which
    /// is the unit the spec-0010 criterion tests below are about.
    fn undeclared_diag(
        model: &LightModel,
        nav: &World,
        reachable: &BTreeSet<[i32; 3]>,
        sky: u8,
        night_vision: bool,
        area_id: &str,
        placement: delvewright_dsl::Placement,
    ) -> Option<LightDiag> {
        let s = survey_undeclared(model, reachable, sky, night_vision);
        dark_diagnostic(
            &BTreeMap::from([(area_id.to_string(), s)]),
            nav,
            sky,
            placement,
        )
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
        // A bare `glow_lichen` is the FACELESS default state, which vanilla
        // lights at 0 — this used to assert 7, which was the table's only
        // overestimate. Attached to a face it is 7, as below.
        assert_eq!(emission("minecraft:glow_lichen"), 0);
        assert_eq!(emission("minecraft:glow_lichen[up=true]"), 7);
        assert_eq!(emission("minecraft:magma_block"), 3);
        assert_eq!(emission("minecraft:stone"), 0);
        assert_eq!(emission("minecraft:air"), 0);
    }

    #[test]
    fn emitter_table_never_overestimates_a_state_dependent_source() {
        // The seven entries that can break the never-overestimate contract,
        // each evaluated over the block's ACTUAL state. Modelling
        // any of these brighter than vanilla lets a genuinely dark area slip past
        // the DW0210/DW0211 gate and ship unmitigated.

        // Sea pickle: light ONLY underwater, 3 + 3*pickles. Was a flat 15.
        assert_eq!(
            emission("minecraft:sea_pickle[pickles=1,waterlogged=true]"),
            6
        );
        assert_eq!(
            emission("minecraft:sea_pickle[pickles=4,waterlogged=true]"),
            15
        );
        assert_eq!(
            emission("minecraft:sea_pickle[pickles=4,waterlogged=false]"),
            0,
            "a dry sea pickle is dark at any count"
        );

        // Redstone ore: dark until activated, and lit is 9 (not 7). Default unlit.
        assert_eq!(emission("minecraft:redstone_ore"), 0);
        assert_eq!(emission("minecraft:redstone_ore[lit=false]"), 0);
        assert_eq!(emission("minecraft:redstone_ore[lit=true]"), 9);
        assert_eq!(emission("minecraft:deepslate_redstone_ore"), 0);

        // Respawn anchor: 0 until charged. Was a flat 7.
        assert_eq!(emission("minecraft:respawn_anchor"), 0);
        assert_eq!(emission("minecraft:respawn_anchor[charges=0]"), 0);
        assert_eq!(emission("minecraft:respawn_anchor[charges=2]"), 7);
        assert_eq!(emission("minecraft:respawn_anchor[charges=4]"), 15);

        // Amethyst is 5/4/2/1 by growth stage, not a flat 7.
        assert_eq!(emission("minecraft:amethyst_cluster"), 5);
        assert_eq!(emission("minecraft:large_amethyst_bud"), 4);
        assert_eq!(emission("minecraft:small_amethyst_bud"), 1);

        // Brewing stand and brown mushroom are 1, not 3.
        assert_eq!(emission("minecraft:brewing_stand"), 1);
        assert_eq!(emission("minecraft:brown_mushroom"), 1);

        // Glow item frames emit NO block light in Java (they are entities; the
        // glow is an emissive texture). The old table's 7 was a Bedrock value.
        assert_eq!(emission("minecraft:glow_item_frame"), 0);

        // `minecraft:froglight` is not a block id — only the three variants are.
        assert_eq!(emission("minecraft:froglight"), 0);
        assert_eq!(emission("minecraft:ochre_froglight"), 15);
    }

    #[test]
    fn lit_state_blocks_default_to_the_state_vanilla_places() {
        // Campfires and redstone torches default to `lit=true`; furnaces and
        // redstone ore default to `lit=false`. A bare id must evaluate to the
        // state a `/setblock` with no properties actually produces — this is what
        // keeps the compiler's own campfire relight fixture worth 15.
        assert_eq!(emission("minecraft:campfire"), 15);
        assert_eq!(emission("minecraft:campfire[lit=false]"), 0);
        assert_eq!(emission("minecraft:soul_campfire"), 10);
        assert_eq!(emission("minecraft:soul_campfire[lit=false]"), 0);
        assert_eq!(emission("minecraft:redstone_torch"), 7);
        assert_eq!(emission("minecraft:redstone_torch[lit=false]"), 0);
        assert_eq!(emission("minecraft:furnace"), 0);
        assert_eq!(emission("minecraft:furnace[lit=true]"), 13);
        assert_eq!(emission("minecraft:blast_furnace[lit=true]"), 13);
        // Cave vines light only when they carry berries.
        assert_eq!(emission("minecraft:cave_vines"), 0);
        assert_eq!(emission("minecraft:cave_vines[age=3,berries=true]"), 14);
        // The technical light block reports its own level.
        assert_eq!(emission("minecraft:light"), 15);
        assert_eq!(emission("minecraft:light[level=4]"), 4);
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

    /// The light flood written the way it reads in the spec: one map, one queue,
    /// every seed pushed and every neighbour relaxed until nothing moves.
    ///
    /// It exists so the dense field has something to be WRONG against. The field
    /// packs a cell's opacity, sky exposure and emission into a byte, indexes by
    /// arithmetic and drains its frontier brightest-first; none of that is
    /// visible here, so the two share no premise beyond the rule the spec states.
    fn reference_flood(model: &LightModel, effective_sky: u8) -> BTreeMap<[i32; 3], u8> {
        let in_aabb = |c: [i32; 3]| in_bounds(c, model.min, model.max);
        let mut light: BTreeMap<[i32; 3], u8> = BTreeMap::new();
        let mut queue: VecDeque<([i32; 3], u8)> = VecDeque::new();
        for y in model.min[1]..=model.max[1] {
            for z in model.min[2]..=model.max[2] {
                for x in model.min[0]..=model.max[0] {
                    let c = [x, y, z];
                    let mut seed = emission(model.block_at(c));
                    if effective_sky > 0 && model.passes(c) && model.sky_open(c) {
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
            if light.get(&c).copied().unwrap_or(0) > l || l <= 1 {
                continue;
            }
            for d in DIRS {
                let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                if !in_aabb(n) || !model.passes(n) {
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

    /// A room with a glass roof, a glowstone in one corner, a lantern hung in
    /// another and a chimney of glass reaching the sky — enough shapes that
    /// opacity, emission and sky exposure all have something to say.
    fn mixed_room() -> BTreeMap<[i32; 3], String> {
        let mut m = room(9, 6, 9, false);
        for x in 3..=5 {
            for z in 3..=5 {
                m.insert([x, 5, z], "minecraft:glass".to_string());
            }
        }
        m.insert([1, 1, 1], "minecraft:glowstone".to_string());
        m.insert([7, 4, 7], "minecraft:lantern[hanging=true]".to_string());
        m.insert([2, 2, 6], "minecraft:oak_fence".to_string());
        m.insert([6, 3, 2], "minecraft:iron_bars".to_string());
        m
    }

    /// A sealed corridor twenty cells long with one emitter at the end of it, so
    /// the answer holds the WHOLE falloff: every value from the source down to 1,
    /// and then the dark beyond where it runs out.
    ///
    /// `mixed_room` alone cannot see the last step. Everything in it is within
    /// reach of a source, so a flood that stopped one level early — never letting
    /// a cell at 2 light its neighbour to 1 — measured the room exactly right and
    /// the comparison below stayed green. A test that cannot fail on the dimmest
    /// step is not testing the falloff.
    fn falloff_corridor() -> BTreeMap<[i32; 3], String> {
        let mut m = room(22, 4, 3, false);
        m.insert([1, 1, 1], "minecraft:glowstone".to_string());
        m
    }

    #[test]
    fn the_dense_field_measures_what_the_spec_flood_measures() {
        for (what, blocks) in [
            ("mixed room", mixed_room()),
            ("falloff", falloff_corridor()),
        ] {
            let model = LightModel::from_blocks(blocks);
            for sky in [0u8, 4, 7, 12, 15] {
                assert_eq!(
                    model.flood(sky),
                    reference_flood(&model, sky),
                    "the dense field and the spec flood disagree over the {what} at sky {sky}"
                );
            }
        }
        // The falloff really does reach the dimmest step, or the comparison above
        // is green about a shape it never met.
        let dim = LightModel::from_blocks(falloff_corridor()).flood(0);
        let mut seen = [false; 16];
        for &l in dim.values() {
            seen[l as usize] = true;
        }
        for (l, reached) in seen.iter().enumerate().skip(1) {
            assert!(reached, "no cell of the falloff corridor measures {l}");
        }
    }

    #[test]
    fn a_placed_fixture_extends_the_field_to_what_a_reflood_would_say() {
        // Every block the fixture registry writes, at cells the pass really uses:
        // air cells whose passability the fixture does not change.
        let writes: [([i32; 3], &str); 4] = [
            ([2, 1, 2], "minecraft:torch"),
            ([4, 2, 6], "minecraft:wall_torch[facing=east]"),
            ([6, 4, 4], "minecraft:lantern[hanging=true]"),
            ([3, 1, 7], "minecraft:campfire[lit=true]"),
        ];
        for sky in [0u8, 4, 15] {
            let mut model = LightModel::from_blocks(mixed_room());
            let mut field = LightField::new(&model, sky);
            for (c, block) in writes {
                model.set(c, block);
                field.set(c, block);
                let fresh = LightField::new(&model, sky);
                assert_eq!(
                    field.light, fresh.light,
                    "extending the field after writing {block} at {c:?} (sky {sky}) \
                     is not what a reflood measures"
                );
            }
            // …and the whole run agrees with the independent flood as well.
            assert_eq!(field.clone_map(), reference_flood(&model, sky));
        }
    }

    #[test]
    fn a_write_that_darkens_the_room_floods_the_field_again() {
        // The extension is only sound while the write cannot make anything
        // dimmer. Blocking a glass roof with stone can: the registry's own
        // fixtures cannot (each emits 14 or 15, which is at least what any cell
        // could have been conducting), so this case is constructed rather than
        // reached from `relight_area` — what it proves is that the field does not
        // simply keep the brighter answer it already had.
        let sky = 15;
        let mut model = LightModel::from_blocks(mixed_room());
        let mut field = LightField::new(&model, sky);
        let under_roof = [4, 4, 4];
        let before = field.light_at(under_roof);
        assert!(before > 0, "the glass roof lights the cell under it");
        for x in 3..=5 {
            for z in 3..=5 {
                let c = [x, 5, z];
                model.set(c, "minecraft:stone");
                field.set(c, "minecraft:stone");
            }
        }
        assert!(
            field.light_at(under_roof) < before,
            "roofing the room over left the cell under it as bright as before"
        );
        assert_eq!(field.clone_map(), reference_flood(&model, sky));
    }

    #[test]
    fn a_shroomlight_written_over_glass_agrees_with_a_reflood() {
        // The one production write that changes a cell's opacity: `shroomlight`
        // embeds by replacing a nav-solid block, and a nav-solid block is not
        // necessarily an opaque one.
        for sky in [0u8, 4, 15] {
            let mut model = LightModel::from_blocks(mixed_room());
            let mut field = LightField::new(&model, sky);
            for c in [[4, 5, 4], [2, 2, 6], [6, 3, 2]] {
                model.set(c, "minecraft:shroomlight");
                field.set(c, "minecraft:shroomlight");
            }
            assert_eq!(
                field.clone_map(),
                reference_flood(&model, sky),
                "a shroomlight over a light-passing solid (sky {sky})"
            );
        }
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
            delvewright_dsl::Placement::Prefabs,
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
            delvewright_dsl::Placement::Prefabs,
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
        let diag = undeclared_diag(
            &model,
            &nav_of(&map),
            &reachable,
            0,
            false,
            "area/crypt",
            delvewright_dsl::Placement::Prefabs,
        );
        assert!(diag.is_some());
        assert_eq!(diag.unwrap().code, DW_DARK_UNMITIGATED);
    }

    /// **A prefab campaign is still offered all three ways**, byte-for-byte.
    ///
    /// The refusal's remedy is a function of the campaign's placement authority;
    /// the arm a prefab campaign takes offers the same three surfaces, names the
    /// same field, and prints the same "no `lighting` and no `mitigation`"
    /// subject it always did.
    ///
    /// The prescription is asserted whole rather than by clause, because the
    /// defect this guards against is a surface silently dropped from the list
    /// while the count beside it stays at three.
    #[test]
    fn dw0210_offers_a_prefab_campaign_every_way_it_always_had() {
        let map = room(7, 5, 7, false);
        let model = LightModel::from_blocks(map.clone());
        let reachable = reachable_of(&map);
        let msg = undeclared_diag(
            &model,
            &nav_of(&map),
            &reachable,
            0,
            false,
            "area/crypt",
            delvewright_dsl::Placement::Prefabs,
        )
        .expect("an enclosed unlit room is dark")
        .message;
        assert!(
            msg.contains("with no `lighting` and no `mitigation` declaration."),
            "a prefab campaign has both surfaces, so the subject names both: {msg}"
        );
        let expected = "Mitigate each area one of three ways: declare `world.areas[].lighting` \
                        (a relight `fixture` + `min_light`) for it, brighten the scene \
                        (`world.time`/`weather`), or declare `world.areas[].mitigation: \
                        \"night-vision\"` on it (the compiler then emits a clocked `effect give \
                        … night_vision` over its bounds) — or re-arrange the room, or raise the \
                        density of what is already lit in it, and measure again. A renamed \
                        potion in a class kit is NOT a mitigation — it grants nothing. Do NOT \
                        lower `DARK_THRESHOLD` or trim the reachable set — the darkness is real \
                        (spec-0010 DW0210)";
        let tail = &msg[msg.find("Mitigate each area").expect("the tail is there")..];
        assert_eq!(tail, expected);
    }

    /// **A derived map is offered only the surfaces it has.**
    ///
    /// Reproduced before this was written: a site-plan campaign with no
    /// `lighting` on its plan and `world.time: night` refused with *"declare
    /// `world.areas[].lighting` … or declare `world.areas[].mitigation`"* — two
    /// prescriptions `DW0839` and `DW0160` refuse — and never named the one
    /// document that would have worked. `mitigation` lives on an `areas[]`
    /// entry, which such a campaign is required to leave empty, so it is a
    /// capability a derived map does not have rather than a wording question.
    #[test]
    fn dw0210_on_a_derived_map_names_the_plan_and_drops_the_mitigation_it_cannot_declare() {
        let map = room(7, 5, 7, false);
        let model = LightModel::from_blocks(map.clone());
        let reachable = reachable_of(&map);
        let msg = undeclared_diag(
            &model,
            &nav_of(&map),
            &reachable,
            0,
            false,
            delvewright_dsl::SITE_AREA,
            delvewright_dsl::Placement::SitePlan,
        )
        .expect("an enclosed unlit room is dark")
        .message;
        assert!(
            msg.contains("declare the site plan's `lighting`"),
            "the one reachable declaration must be named: {msg}"
        );
        assert!(
            !msg.contains("world.areas"),
            "`DW0839` refuses `areas[]` here, so nothing may send the author to it: {msg}"
        );
        assert!(
            !msg.contains("mitigation: "),
            "a derived map has no per-area `mitigation` surface to declare: {msg}"
        );
        assert!(
            msg.contains("one of two ways"),
            "the count moves with the list of ways actually offered: {msg}"
        );
    }

    /// The `DW0211` half of the same pair: only the FIELD moves.
    #[test]
    fn dw0211_names_the_document_this_campaign_declares_lighting_in() {
        assert_eq!(
            delvewright_dsl::Placement::Prefabs.lighting_field(),
            "`world.areas[].lighting`"
        );
        assert_eq!(
            delvewright_dsl::Placement::SitePlan.lighting_field(),
            "the site plan's `lighting`"
        );
    }

    // --- Criterion 5: night-vision kit suppresses DW0210 ---

    #[test]
    fn crit5_night_vision_suppresses_dw0210() {
        let map = room(7, 5, 7, false);
        let model = LightModel::from_blocks(map.clone());
        let reachable = reachable_of(&map);
        assert!(
            undeclared_diag(
                &model,
                &nav_of(&map),
                &reachable,
                0,
                true,
                "area/crypt",
                delvewright_dsl::Placement::Prefabs
            )
            .is_none(),
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
            undeclared_diag(
                &model,
                &nav_of(&map),
                &reachable,
                0,
                false,
                "area/room",
                delvewright_dsl::Placement::Prefabs
            )
            .is_none(),
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
            delvewright_dsl::Placement::Prefabs,
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
            delvewright_dsl::Placement::Prefabs,
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
            delvewright_dsl::Placement::Prefabs,
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
                delvewright_dsl::Placement::Prefabs,
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

    // -----------------------------------------------------------------------
    // Occupancy-coupling invariant: nav-passable ⇒ light-passing
    // -----------------------------------------------------------------------

    /// The opacity table agrees with vanilla 1.21.11 for every block class the nav
    /// model leaves passable, and still calls a full solid cube opaque.
    ///
    /// Evidence: `filterLight` in the pinned `minecraft-data` 1.21.9 block dump
    /// (`harness/node_modules/minecraft-data/.../pc/1.21.9/blocks.json`) — 0 for
    /// all 16 pressure plates, `tripwire`, `tripwire_hook`, all 20 carpets, `snow`
    /// (the layer block) and all 12 fence gates; 15 for the control cubes below.
    #[test]
    fn passes_light_matches_vanilla_filter_light() {
        // Trap triggers — the reported false-dark class (DW0342 needs these cells
        // walkable, so the light model must not call them opaque).
        for id in [
            "minecraft:oak_pressure_plate",
            "minecraft:oak_pressure_plate[powered=false]",
            "minecraft:stone_pressure_plate[powered=true]",
            "minecraft:polished_blackstone_pressure_plate",
            "minecraft:heavy_weighted_pressure_plate",
            "minecraft:light_weighted_pressure_plate",
            "minecraft:tripwire",
            "minecraft:tripwire[attached=true,powered=false]",
            "minecraft:tripwire_hook[facing=north]",
        ] {
            assert!(passes_light(id), "{id} is filterLight=0 in vanilla");
        }
        // Thin decoration (`is_thin_decoration`): carpets + shallow snow layers.
        for id in [
            "minecraft:white_carpet",
            "minecraft:black_carpet",
            "minecraft:moss_carpet",
            "minecraft:pale_moss_carpet[bottom=true]",
            "minecraft:snow",
            "minecraft:snow[layers=1]",
            "minecraft:snow[layers=8]",
        ] {
            assert!(passes_light(id), "{id} is filterLight=0 in vanilla");
        }
        // No-collision vegetation: nav-passable feet cells, all
        // filterLight=0 in vanilla — the class, not a per-generator id list.
        for id in [
            "minecraft:pink_petals",
            "minecraft:tall_grass",
            "minecraft:large_fern",
            "minecraft:poppy",
            "minecraft:oxeye_daisy",
            "minecraft:cornflower",
            "minecraft:oak_sapling",
            "minecraft:wheat[age=3]",
            "minecraft:sweet_berry_bush",
        ] {
            assert!(passes_light(id), "{id} is filterLight=0 in vanilla");
        }
        // Fence gates — open = a passable threshold, closed = passable-with-use.
        // Previously only `oak_fence_gate` passed light; every wood does now.
        for id in [
            "minecraft:oak_fence_gate",
            "minecraft:spruce_fence_gate[open=true]",
            "minecraft:dark_oak_fence_gate[facing=east,open=false]",
            "minecraft:warped_fence_gate",
        ] {
            assert!(passes_light(id), "{id} is filterLight=0 in vanilla");
        }
        // Control: full solid-render cubes stay opaque (filterLight=15). Note
        // `snow_block` is a full cube and must NOT be caught by the `snow` entry.
        for id in [
            "minecraft:stone",
            "minecraft:dirt",
            "minecraft:oak_planks",
            "minecraft:cobblestone",
            "minecraft:deepslate",
            "minecraft:sand",
            "minecraft:gravel",
            "minecraft:obsidian",
            "minecraft:snow_block",
        ] {
            assert!(!passes_light(id), "{id} is filterLight=15 in vanilla");
        }
    }

    /// **The invariant** (see [`passes_light`]): if
    /// [`crate::assembled::occupancy_of`] leaves a block's cell player-occupiable
    /// — absent from `solid`/`tall`/`flooded`, i.e. free air or a passable-with-use
    /// fence gate — then that block MUST pass light, or the gate measures a cell
    /// the player really stands in at light 0 and manufactures a `DW0210`.
    ///
    /// Driven through the real classifier over a one-cell world, so a future
    /// passability change (a new thin-decoration class, say) that forgets the light
    /// table fails here instead of in a campaign.
    #[test]
    fn every_nav_passable_block_passes_light() {
        let candidates = [
            // trap triggers
            "minecraft:oak_pressure_plate[powered=false]",
            "minecraft:stone_pressure_plate",
            "minecraft:polished_blackstone_pressure_plate",
            "minecraft:heavy_weighted_pressure_plate",
            "minecraft:tripwire",
            "minecraft:tripwire_hook",
            // thin decoration
            "minecraft:white_carpet",
            "minecraft:moss_carpet",
            "minecraft:pale_moss_carpet",
            "minecraft:snow[layers=1]",
            "minecraft:snow[layers=4]",
            // fence gates (closed = use-gate, open = free threshold)
            "minecraft:oak_fence_gate",
            "minecraft:spruce_fence_gate",
            "minecraft:crimson_fence_gate",
            // controls that must stay impassable AND may stay opaque
            "minecraft:stone",
            "minecraft:oak_slab",
            "minecraft:snow_block",
            "minecraft:cobblestone_wall",
            "minecraft:oak_fence",
        ];
        let cell = [0, 0, 0];
        for id in candidates {
            let mut map = BTreeMap::new();
            map.insert(cell, id.to_string());
            let occ = crate::assembled::occupancy_of(map, &BTreeSet::new());
            // Player-occupiable: nothing a walker is blocked by lives here. A
            // closed fence gate is in `use_gates`, which the player walks through.
            let blocked = occ.solid.contains(&cell)
                || occ.tall.contains(&cell)
                || occ.flooded.contains(&cell);
            if !blocked {
                assert!(
                    passes_light(id),
                    "occupancy leaves `{id}` player-occupiable, so the light model \
                     must not call it opaque — a player standing there would be \
                     measured at light 0 and trip a false DW0210"
                );
            }
        }
    }

    /// Red-before / green-after: a **roofed, lit** room whose floor carries a
    /// pressure-plate trap trigger. The trigger cell is a reachable walkable cell
    /// (nav keeps it passable on purpose, for the `DW0342` avoidability proof), so
    /// with the pre-fix table it measured light 0 and the whole area failed
    /// `DW0210` — a lighting failure invented by the model. After the fix the cell
    /// measures exactly what the same cell measures with plain air in it.
    #[test]
    fn trap_trigger_cell_measures_real_light_not_zero() {
        let plate = [2, 1, 2];
        let mut map = room(9, 5, 9, false); // roofed → no sky light
        map.insert([4, 3, 4], "minecraft:glowstone".to_string()); // the room's lamp
        let mut with_plate = map.clone();
        with_plate.insert(
            plate,
            "minecraft:oak_pressure_plate[powered=false]".to_string(),
        );

        // The trigger cell really is a reachable walkable cell under the shipped
        // collision model (this is what makes the false DW0210 reachable at all).
        let reachable = reachable_occ_of(&with_plate, [4, 1, 4]);
        let nav = nav_occ_of(&with_plate);
        assert!(
            reachable.contains(&plate),
            "the plate cell must stay walkable (DW0342 premise)"
        );

        // It measures the same light as the identical cell with air in it.
        let bare = LightModel::from_blocks(map).flood(0);
        let lit = LightModel::from_blocks(with_plate).flood(0);
        let expect = bare.get(&plate).copied().unwrap_or(0);
        assert!(expect >= DARK_THRESHOLD, "fixture must be genuinely lit");
        assert_eq!(
            lit.get(&plate).copied().unwrap_or(0),
            expect,
            "a pressure plate is transparent in vanilla (filterLight=0); its cell \
             must measure the room's real light, not 0"
        );

        // …and the area is clean.
        let model = LightModel::from_blocks({
            let mut m = room(9, 5, 9, false);
            m.insert([4, 3, 4], "minecraft:glowstone".to_string());
            m.insert(
                plate,
                "minecraft:oak_pressure_plate[powered=false]".to_string(),
            );
            m
        });
        assert!(
            undeclared_diag(
                &model,
                &nav,
                &reachable,
                0,
                false,
                "area/keep",
                delvewright_dsl::Placement::Prefabs
            )
            .is_none(),
            "a lit roofed room with a trap trigger must not trip DW0210"
        );
    }

    /// No over-correction: the same roofed room **without** a lamp is genuinely
    /// dark, and the transparent trap trigger does not launder that away —
    /// `DW0210` still fires, by code.
    #[test]
    fn dark_roofed_room_with_a_trap_trigger_still_fails_dw0210() {
        let plate = [2, 1, 2];
        let mut map = room(9, 5, 9, false); // roofed, unlit
        map.insert(
            plate,
            "minecraft:oak_pressure_plate[powered=false]".to_string(),
        );
        let reachable = reachable_occ_of(&map, [4, 1, 4]);
        assert!(reachable.contains(&plate));
        let nav = nav_occ_of(&map);
        let model = LightModel::from_blocks(map);
        let diag = undeclared_diag(
            &model,
            &nav,
            &reachable,
            0,
            false,
            "area/crypt",
            delvewright_dsl::Placement::Prefabs,
        )
        .expect("a genuinely dark roofed room must still fail");
        assert_eq!(diag.code, DW_DARK_UNMITIGATED);
    }

    /// The same fix for the other two nav-passable classes: a carpet and a closed
    /// fence gate on the critical path measure the room's real light too.
    #[test]
    fn carpet_and_fence_gate_cells_measure_real_light() {
        for block in [
            "minecraft:white_carpet",
            "minecraft:oak_fence_gate[open=false]",
            "minecraft:spruce_fence_gate[open=false]",
        ] {
            let cell = [2, 1, 2];
            let mut map = room(9, 5, 9, false);
            map.insert([4, 3, 4], "minecraft:glowstone".to_string());
            let bare = LightModel::from_blocks(map.clone()).flood(0);
            map.insert(cell, block.to_string());
            let got = LightModel::from_blocks(map).flood(0);
            assert_eq!(
                got.get(&cell).copied().unwrap_or(0),
                bare.get(&cell).copied().unwrap_or(0),
                "{block} is filterLight=0 in vanilla; its cell must measure real light"
            );
        }
    }

    /// The corrected opacity table does not disturb determinism: the same input
    /// still yields byte-identical light everywhere, including the new classes.
    #[test]
    fn corrected_opacity_is_deterministic() {
        let build = || {
            let mut map = room(11, 5, 11, false);
            map.insert([5, 3, 5], "minecraft:glowstone".to_string());
            map.insert(
                [2, 1, 2],
                "minecraft:oak_pressure_plate[powered=false]".to_string(),
            );
            map.insert([8, 1, 8], "minecraft:white_carpet".to_string());
            map.insert(
                [2, 1, 8],
                "minecraft:spruce_fence_gate[open=true]".to_string(),
            );
            LightModel::from_blocks(map).flood(0)
        };
        assert_eq!(build(), build());
    }

    // -----------------------------------------------------------------------
    // The report a designer is handed: how much is dark, and where
    //
    // `DW0210` prescribes re-arranging the room. Every assertion below is
    // computed from the fixture it is about — a count written down beside the
    // fixture is green on a gate that binds nothing.
    // -----------------------------------------------------------------------

    /// [`room`], translated. Two of these far apart are two places a body can
    /// never walk between, which is what makes them separate areas.
    fn room_at(w: i32, h: i32, d: i32, off: [i32; 3]) -> BTreeMap<[i32; 3], String> {
        room(w, h, d, false)
            .into_iter()
            .map(|(c, b)| ([c[0] + off[0], c[1] + off[1], c[2] + off[2]], b))
            .collect()
    }

    /// A corridor `w` long, one cell wide, roofed (no sky light), with a
    /// glowstone at each of `lamps`. Floor at `y=0`, walkable cells at
    /// `[x, 1, 1]` for `x in 1..=w-2`, head room at `y=1..=2`, ceiling at `y=3`
    /// — two cells of interior height, because one leaves no head clearance and
    /// nothing in it is standable at all.
    fn corridor(w: i32, lamps: &[[i32; 3]]) -> BTreeMap<[i32; 3], String> {
        let mut m = room(w, 4, 3, false);
        for &l in lamps {
            m.insert(l, "minecraft:glowstone".to_string());
        }
        m
    }

    /// **How much** is dark, per area — the fact that separates "one stubborn
    /// corner" from "the whole tower is unlit", which call for opposite
    /// responses. Two unlit rooms of different sizes: both are named, each with
    /// its own count, and the bigger one is reported first.
    #[test]
    fn dw0210_reports_every_dark_area_with_its_own_size() {
        let mut map = room_at(11, 5, 11, [0, 0, 0]);
        map.extend(room_at(7, 5, 7, [40, 0, 0]));
        let nav = nav_of(&map);
        let model = LightModel::from_blocks(map.clone());

        let big_cells = nav.reachable_walkable(&[[5, 1, 5]]);
        let small_cells = nav.reachable_walkable(&[[43, 1, 3]]);
        assert!(
            big_cells.len() > small_cells.len() && !small_cells.is_empty(),
            "fixture: two enclosed rooms of different sizes, got {} and {}",
            big_cells.len(),
            small_cells.len()
        );
        assert!(
            big_cells.is_disjoint(&small_cells),
            "fixture: the two rooms must not be connected"
        );

        let big = survey_undeclared(&model, &big_cells, 0, false);
        let small = survey_undeclared(&model, &small_cells, 0, false);
        assert_eq!(big.dark.len(), big_cells.len(), "the big room is all dark");
        assert_eq!(
            small.dark.len(),
            small_cells.len(),
            "and so is the small one"
        );

        let areas = BTreeMap::from([
            ("area/annex".to_string(), small),
            ("area/hall".to_string(), big),
        ]);
        let diag = dark_diagnostic(&areas, &nav, 0, delvewright_dsl::Placement::Prefabs)
            .expect("two dark rooms must fail the gate");
        assert_eq!(diag.code, DW_DARK_UNMITIGATED);
        let m = &diag.message;

        // Both places are named — the whole point. Before this, the build failed
        // on one diagnostic and the alphabetically-first area's message was the
        // only one anyone saw.
        let hall = m
            .find(&format!(
                "area `area/hall`: {n} of {n} measured cell(s) dark",
                n = big_cells.len()
            ))
            .unwrap_or_else(|| panic!("the big room and its count are missing:\n{m}"));
        let annex = m
            .find(&format!(
                "area `area/annex`: {n} of {n} measured cell(s) dark",
                n = small_cells.len()
            ))
            .unwrap_or_else(|| panic!("the small room and its count are missing:\n{m}"));
        assert!(
            hall < annex,
            "worst first: the bigger dark area is the one whose remedy is a different act\n{m}"
        );
        assert!(
            m.contains(&format!(
                "2 area(s) measure dark: {t} of the {t} reachable walkable cell(s)",
                t = big_cells.len() + small_cells.len()
            )),
            "the campaign-wide total is missing:\n{m}"
        );
    }

    /// **Where**: the dark cells grouped into places, not listed as coordinates.
    /// One corridor, one lamp in the middle, dark at both ends — one area, two
    /// rooms, and the report says two.
    #[test]
    fn dw0210_groups_dark_cells_into_the_places_they_are_in() {
        let map = corridor(31, &[[15, 3, 1]]);
        let nav = nav_of(&map);
        let model = LightModel::from_blocks(map.clone());
        let reachable = nav.reachable_walkable(&[[15, 1, 1]]);
        let s = survey_undeclared(&model, &reachable, 0, false);

        let regions = s.regions(&nav);
        assert_eq!(
            regions.len(),
            2,
            "fixture: a lamp mid-corridor leaves a dark run at each end, got {regions:?}"
        );
        assert_eq!(
            regions.iter().map(|r| r.cells).sum::<usize>(),
            s.dark.len(),
            "the regions must PARTITION the dark set — no cell counted twice, none dropped"
        );
        let (a, b) = (&regions[0], &regions[1]);
        assert!(
            a.hi[0] + 1 < b.lo[0] || b.hi[0] + 1 < a.lo[0],
            "fixture: the two dark runs are separated by lit corridor, got {a:?} and {b:?}"
        );
        for r in &regions {
            assert!(
                (0..3).all(|i| r.lo[i] <= r.darkest.0[i] && r.darkest.0[i] <= r.hi[i]),
                "a region's extent must contain its own darkest cell: {r:?}"
            );
        }

        let diag = dark_diagnostic(
            &BTreeMap::from([("area/gallery".to_string(), s.clone())]),
            &nav,
            0,
            delvewright_dsl::Placement::Prefabs,
        )
        .expect("a dark-ended corridor must fail the gate");
        let m = &diag.message;
        assert!(
            m.contains(&format!("in {} contiguous region(s)", regions.len())),
            "the region count is missing:\n{m}"
        );
        for r in &regions {
            assert!(
                m.contains(&format!(
                    "- {c} cell(s) spanning {lo:?}..{hi:?}, darkest {dc:?} at light {dl}",
                    c = r.cells,
                    lo = r.lo,
                    hi = r.hi,
                    dc = r.darkest.0,
                    dl = r.darkest.1,
                )),
                "region {r:?} is missing from the report:\n{m}"
            );
        }
    }

    /// **The single-cell fact, kept.** One dark cell must still be named, and the
    /// report must not have become noise on the way: one region, one line.
    #[test]
    fn dw0210_still_names_the_one_cell_when_only_one_is_dark() {
        let map = corridor(15, &[[0, 1, 1]]);
        let nav = nav_of(&map);
        let model = LightModel::from_blocks(map.clone());
        let reachable = nav.reachable_walkable(&[[7, 1, 1]]);
        let s = survey_undeclared(&model, &reachable, 0, false);
        assert_eq!(
            s.dark.len(),
            1,
            "fixture: a lamp at one end leaves exactly the far cell dark, got {:?}",
            s.dark
        );
        let (cell, level) = s.darkest().expect("one dark cell has a darkest");

        let diag = dark_diagnostic(
            &BTreeMap::from([("area/hall".to_string(), s)]),
            &nav,
            0,
            delvewright_dsl::Placement::Prefabs,
        )
        .expect("one dark cell is still a failed build");
        let m = &diag.message;
        assert!(
            m.contains(&format!(
                "The darkest reachable walkable cell is at {cell:?}, measured at light {level}"
            )),
            "the exemplar cell is missing:\n{m}"
        );
        assert!(
            m.contains(&format!(
                "area `area/hall`: 1 of {} measured cell(s) dark, in 1 contiguous region(s)",
                reachable.len()
            )),
            "the one-cell count is missing:\n{m}"
        );
        assert_eq!(
            m.lines()
                .filter(|l| l.trim_start().starts_with("- "))
                .count(),
            1,
            "one dark cell must produce one region line, not a wall of them:\n{m}"
        );
    }

    /// A lit area reports nothing at all — the gate stays silent where it always
    /// was, and the report is not a thing that fires on its own.
    #[test]
    fn a_lit_area_produces_no_report() {
        let mut map = room(9, 5, 9, false);
        map.insert([4, 3, 4], "minecraft:glowstone".to_string());
        let nav = nav_of(&map);
        let model = LightModel::from_blocks(map.clone());
        let reachable = nav.reachable_walkable(&[[4, 1, 4]]);
        let s = survey_undeclared(&model, &reachable, 0, false);
        assert!(!reachable.is_empty(), "fixture: the room is walkable");
        assert!(s.dark.is_empty(), "fixture: the room is lit");
        assert!(
            dark_diagnostic(
                &BTreeMap::from([("area/hall".to_string(), s)]),
                &nav,
                0,
                delvewright_dsl::Placement::Prefabs
            )
            .is_none(),
            "a lit area must produce no diagnostic"
        );
    }

    /// **A truncated list says what it dropped.** More dark regions than the
    /// report lists: the ones it did not print are counted, in cells, in the
    /// message. A list silently cut at N reads as "here is everything".
    #[test]
    fn a_capped_region_list_states_what_it_dropped() {
        // Lamps every 26 cells leave a dark run between each pair and at both
        // ends — more runs than `DARK_REGIONS_LISTED`.
        let lamps: Vec<[i32; 3]> = (0..4).map(|i| [15 + 26 * i, 3, 1]).collect();
        let map = corridor(108, &lamps);
        let nav = nav_of(&map);
        let model = LightModel::from_blocks(map.clone());
        let reachable = nav.reachable_walkable(&[[15, 1, 1]]);
        let s = survey_undeclared(&model, &reachable, 0, false);
        let regions = s.regions(&nav);
        assert!(
            regions.len() > DARK_REGIONS_LISTED,
            "fixture: more dark runs than the report lists, got {}",
            regions.len()
        );

        let diag = dark_diagnostic(
            &BTreeMap::from([("area/undercroft".to_string(), s)]),
            &nav,
            0,
            delvewright_dsl::Placement::Prefabs,
        )
        .expect("a corridor with dark runs must fail the gate");
        let m = &diag.message;
        let dropped: usize = regions
            .iter()
            .skip(DARK_REGIONS_LISTED)
            .map(|r| r.cells)
            .sum();
        assert!(
            m.contains(&format!(
                "… and {k} further region(s) covering {dropped} cell(s), not listed",
                k = regions.len() - DARK_REGIONS_LISTED,
            )),
            "the cut list does not say what it cut:\n{m}"
        );
        assert!(
            m.contains(&format!("in {} contiguous region(s)", regions.len())),
            "the full region count must be stated even where the list is cut:\n{m}"
        );
    }

    /// The same obligation for the other cap: more dark areas than the report
    /// lists, and it says how many and how many cells they hold.
    #[test]
    fn a_capped_area_list_states_what_it_dropped() {
        let n = DARK_AREAS_LISTED + 1;
        let mut map = BTreeMap::new();
        for i in 0..n {
            map.extend(room_at(5, 4, 5, [20 * i as i32, 0, 0]));
        }
        let nav = nav_of(&map);
        let model = LightModel::from_blocks(map.clone());
        let mut areas = BTreeMap::new();
        for i in 0..n {
            let cells = nav.reachable_walkable(&[[20 * i as i32 + 2, 1, 2]]);
            assert!(!cells.is_empty(), "fixture: room {i} must be walkable");
            areas.insert(
                format!("area/cell-{i:02}"),
                survey_undeclared(&model, &cells, 0, false),
            );
        }
        let dark_total: usize = areas.values().map(|s| s.dark.len()).sum();
        let diag = dark_diagnostic(&areas, &nav, 0, delvewright_dsl::Placement::Prefabs)
            .expect("nine unlit rooms must fail the gate");
        let m = &diag.message;
        assert!(
            m.contains(&format!("{n} area(s) measure dark: {dark_total} of the")),
            "the campaign-wide area and cell counts are missing:\n{m}"
        );
        let listed: usize = m
            .lines()
            .filter(|l| l.trim_start().starts_with("area `"))
            .count();
        assert_eq!(listed, DARK_AREAS_LISTED, "the list must be cut:\n{m}");
        // The report's own ordering, re-derived here rather than read off it.
        let mut order: Vec<(usize, String)> = areas
            .iter()
            .map(|(id, s)| (s.dark.len(), id.clone()))
            .collect();
        order.sort_by_key(|(c, id)| (std::cmp::Reverse(*c), id.clone()));
        let dropped: usize = order.iter().skip(DARK_AREAS_LISTED).map(|(c, _)| c).sum();
        assert!(
            m.contains(&format!(
                "… and {k} further dark area(s) covering {dropped} cell(s), not listed",
                k = n - DARK_AREAS_LISTED,
            )),
            "the cut area list does not say what it cut:\n{m}"
        );
    }

    /// Determinism (ADR-0006): the report is a pure function of the measurement.
    /// The grouping walk in particular must not depend on iteration order.
    #[test]
    fn the_dark_report_is_deterministic() {
        let map = corridor(31, &[[15, 3, 1]]);
        let nav = nav_of(&map);
        let model = LightModel::from_blocks(map.clone());
        let reachable = nav.reachable_walkable(&[[15, 1, 1]]);
        let build = || {
            let s = survey_undeclared(&model, &reachable, 0, false);
            (
                s.regions(&nav),
                dark_diagnostic(
                    &BTreeMap::from([("area/gallery".to_string(), s)]),
                    &nav,
                    0,
                    delvewright_dsl::Placement::Prefabs,
                )
                .map(|d| d.message),
            )
        };
        assert_eq!(build(), build());
    }
}
