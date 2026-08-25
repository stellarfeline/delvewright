//! **What a block state does to a body that walks into it** (spec-0056) — the
//! one authority, for every crate in this workspace.
//!
//! One question: *given a vanilla block state, may a body occupy its cell, may a
//! body stand on it, and if so at what height?* Every walk in this repository
//! asks it — `delvec`'s route proofs, the grammar back end's contract gates, the
//! admission pipeline's light probe — and until spec-0056 each of them answered
//! it privately. Three copies, measured disagreeing in both directions: the
//! grammar walk read *air or a skull* and called a torch a wall; the light probe
//! kept a nine-id list that called open water walkable; `delvec` alone had a real
//! collision table and could not lend it to either.
//!
//! # Why this module is in `delvewright-dsl`
//!
//! The same argument that placed [`crate::metrics::step_allowed`], and it has two
//! halves.
//!
//! *Reachability.* `delvec` is published to crates.io and may depend only on
//! published crates, so the table cannot live in `delvewright-schem` or
//! `delvewright-grammar`; `delvewright-dsl` is the one crate every consumer
//! already resolves.
//!
//! *Object class.* A collision box is a fact about **a vanilla block state under
//! the pinned game version** (ADR-0009, Minecraft Java 1.21.11) — the same kind of
//! pinned-physics fact, in the same sixteenths, as the auto-step and jump-apex
//! budgets already in [`crate::metrics`]. [`collision_top_16`] is what a correct
//! rise measurement is made *of*: `metrics` says how far a body may climb, this
//! module says from what height to what height.
//!
//! # The rule, in one table
//!
//! [`Collision`] is the classification and [`collision_class`] computes it. Its
//! three answers are what a walk needs and all a walk gets:
//!
//! | class | [`Collision::passes_body`] | [`Collision::supports_body`] | [`Collision::floor_top_16`] |
//! |---|---|---|---|
//! | [`Collision::Air`] | yes | no | — |
//! | [`Collision::Thin`] (top < 8/16) | yes | no | — (the body rests on the block below) |
//! | [`Collision::PartialFloor`] (8..16) | no | yes | the measured top |
//! | [`Collision::FullCube`] | no | yes | 16 |
//! | [`Collision::TallBarrier`] (fence, wall) | no | **no** | — |
//! | [`Collision::FenceGate`] | **yes** | no | — |
//! | [`Collision::Fluid`] | no | no | — |
//!
//! Two rows are not the naive complement of each other, and that is the whole
//! reason there are two columns. A **tall barrier** is 1.5 blocks on a 1-block
//! cell: a body neither passes through it nor reaches its top face by jumping. A
//! **fence gate** is the mirror: adventure mode permits the right-click that
//! opens it, so a body gets through — and for a *closure* claim a gate was never
//! a seal anyway, so reading it as a hole is the sound direction. A **fluid** is
//! neither, from the other side: spec-0038 forbids a route crediting water, and
//! nothing stands on a surface.
//!
//! Sub-8/16 is [`THIN_HEIGHT_16`], and it is not a rounding: vanilla's auto-step
//! is 0.6 blocks ([`crate::metrics::MAX_AUTO_STEP_16`] = 9/16), so a carpet, a
//! shallow snow drift, a candle or a pressure plate is stepped **over** rather
//! than onto and never constitutes a floor level of its own.
//!
//! # What one boolean cannot carry, said out loud
//!
//! A walk's `passable` answers for a body *and* for a sightline, and for the
//! class this module changed most — empty collision — the two agree: glow lichen
//! neither collides nor occludes. They part for partial-height blocks, where a
//! slab stops a body and not an eye. This module answers the **body** question;
//! a caller asking about a sightline gets the full-cube reading, which refuses
//! claims it should sometimes grant. That is the refusing direction, and it is
//! recorded here rather than hidden (spec-0056 §3.3). Splitting the two needs a
//! second per-cell answer with its own conservative direction per claim sign, and
//! that surface is not proposed.
//!
//! # The direction of every error here is stated per entry
//!
//! Anything this module does not recognise is a [`Collision::FullCube`], which
//! **over-blocks**: it can only make a walk refuse a step vanilla admits, never
//! admit one vanilla refuses. So a block whose shape has not been read out of the
//! pinned game is left out on purpose rather than guessed at, and the places that
//! happens say so ([`is_no_collision_fixture`]).

/// A full block's collision height in sixteenths — the unit [`collision_top_16`]
/// reports in. Vanilla builds every partial collision box out of sixteenths, so
/// integer sixteenths represent every case exactly (no float ordering, ADR-0006).
pub const FULL_HEIGHT_16: u8 = 16;

/// Below this collision height a block is **stepped over, not onto**: the walker's
/// feet stay on whatever supports it. 8/16 = half a block, the slab step. Anything
/// thinner (carpet 1/16, a candle 6/16, a 1–4-layer snow drift ≤ 6/16) is under the
/// vanilla 0.6-block auto-step, so modelling it as a floor *level* of its own would
/// be noise; modelling it as a full cube is a lie.
pub const THIN_HEIGHT_16: u8 = 8;

// One number, one definition: a full block is 16/16 here and in the step rule, and
// this refuses to compile the day the two drift.
const _: () = assert!(FULL_HEIGHT_16 as i64 == crate::metrics::FULL_16);

/// The bare block id of a (possibly blockstate-carrying) block name: strips a
/// trailing `[state]` / `{nbt}` suffix, keeping the namespace
/// (`minecraft:oak_slab[type=top]` → `minecraft:oak_slab`).
///
/// Every classifier below matches on this, and the state-sensitive ones read the
/// property they need with [`state_value`]. Waterlogging, slab halves and
/// snow-layer counts are block *state*, and a model that throws the state away
/// cannot tell a half-step from a full cube.
pub fn base_id(name: &str) -> &str {
    match name.find(['[', '{']) {
        Some(i) => &name[..i],
        None => name,
    }
}

/// The value of blockstate property `key` in an `id[k=v,…]` block name, or
/// `None` when the name carries no state or lacks that property.
pub fn state_value<'a>(name: &'a str, key: &str) -> Option<&'a str> {
    let open = name.find('[')?;
    let close = name[open..].find(']')? + open;
    name[open + 1..close].split(',').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k.trim() == key).then_some(v.trim())
    })
}

/// The namespace-free bare id (`minecraft:oak_slab[type=top]` → `oak_slab`).
///
/// Public because the namespace is optional everywhere a human writes a block:
/// a prefab palette always carries `minecraft:`, an author's `fill-region` block
/// is a hand-written string, and both are legal. Every classifier here goes
/// through this so the two spellings cannot get different answers.
pub fn bare_id(name: &str) -> &str {
    let id = base_id(name);
    id.strip_prefix("minecraft:").unwrap_or(id)
}

/// The air block variants that count as "no block" — passable, transparent, and
/// never a floor.
///
/// Namespace-insensitive, unlike the predicate this replaced: a bare `air` in a
/// hand-written `fill-region` is the same block as `minecraft:air`, and a
/// namespace-sensitive reading classified it by the full-cube default, i.e. as
/// floor a route proof would walk on.
pub fn is_air(name: &str) -> bool {
    matches!(bare_id(name), "air" | "cave_air" | "void_air")
}

/// Whether a cell's block is a **free fluid** — water or lava occupying the whole
/// cell with no host block. **The one answer to "is this block id a fluid"** in
/// this workspace.
///
/// What it covers, and why each case is the way it is:
/// - **Block state is irrelevant.** A flowing `minecraft:water[level=3]` answers
///   the same as a source. Both leave a body swimming rather than standing, which
///   is the only question a collision model asks; the *reach* of a flow is a
///   world question, not this predicate's.
/// - **So is the namespace**, and that is not cosmetic: a bare `water` passes DSL
///   block validation and is emitted verbatim, and a namespace-sensitive
///   comparison would read it as an ordinary solid and prove a floor made of it.
/// - **`minecraft:lava` counts.** Nothing stands on lava either, and a model that
///   answered only for water would prove a lava surface walkable.
/// - **A waterlogged block does NOT count.** `oak_stairs[waterlogged=true]` is a
///   cell occupied by its *host* block — solid, standable, and simultaneously a
///   flood source for its neighbours. Folding it in here would delete a floor the
///   game plainly has.
pub fn is_fluid(name: &str) -> bool {
    matches!(bare_id(name), "water" | "lava")
}

/// Whether a block is a **1.5-block-tall barrier**: fences (`*_fence`, incl.
/// `nether_brick_fence`) and walls (`*_wall`). Vanilla gives these a collision box
/// 1.5 blocks tall on a 1-block cell, which breaks the full-cube assumption in
/// BOTH directions:
///
/// - **Not standable on top by a walking player**: a normal jump rises ~1.25
///   blocks ([`crate::metrics::MAX_JUMP_RISE_16`]), so a 1.5-tall top face is
///   unreachable by walking or jumping — a "legal +1 step onto a fence top" is a
///   proof of a route no player and no bot can walk.
/// - **Not passable through**: the barrier fills its cell for a walker, and its
///   top half also blocks the cell above.
///
/// Fence **gates** are excluded — they are the openable case, see
/// [`is_fence_gate`].
pub fn is_tall_barrier(name: &str) -> bool {
    let id = bare_id(name);
    id.ends_with("_fence") || id.ends_with("_wall")
}

/// Whether a block is a fence gate (`*_fence_gate` — every vanilla fence gate is
/// a wooden, right-click-openable one).
///
/// Closed, it is a 1.5-tall barrier like a fence but **passable-with-use**:
/// opening it is a right-click USE interaction vanilla permits in adventure mode,
/// the same action a human player performs. So a body gets through, and a caller
/// that needs to know whether a *non-player* walker does asks this predicate by
/// name rather than reading it out of the passability answer.
pub fn is_fence_gate(name: &str) -> bool {
    bare_id(name).ends_with("_fence_gate")
}

/// Thin, walkable trap-trigger blocks (spec-0011) a player steps *onto* rather
/// than being blocked by: pressure plates and tripwire.
///
/// Named separately from the rest of the empty-collision class because a caller
/// needs the *trap* fact as well as the collision fact — nav must route a player
/// ONTO a critical-path trap trigger (the hazard `DW0342` reasons about) instead
/// of routing around a "solid" plate and calling every trap avoidable. Their
/// collision shape is empty, so [`collision_top_16`] answers 0 for them too and
/// the two facts cannot disagree.
pub fn is_passable_trap_trigger(name: &str) -> bool {
    let id = bare_id(name);
    id.ends_with("_pressure_plate") || matches!(id, "tripwire" | "tripwire_hook")
}

/// Vanilla's **no-collision vegetation class**: blocks whose collision shape is
/// EMPTY — a walker passes straight through and stands on whatever is below
/// (they are visual/light-model content only).
///
/// Modelling one as a full cube is wrong in both directions. A `short_grass` tuft
/// on a valley terrace splits a deliberate 2-block riser into two climbable
/// 1-block steps, so `DW0854` refuses a landform vanilla cannot climb
/// (rejects-valid); and, worse, any walkability proof that stands a body ON a
/// tuft or flower cell is unsound (accepts-invalid).
///
/// The list is the **class**, not the three ids one generator happens to scatter.
/// Sources: Minecraft Java 1.21.11 block shapes — every id here has an empty
/// collision shape. Deliberately excluded because they DO collide (or attach in
/// ways this model does not represent): `azalea`/`flowering_azalea`,
/// `big_dripleaf`, `bamboo`, `cactus`, `chorus_*`, `pointed_dripstone`,
/// `scaffolding`, `sea_pickle`, `cocoa`, lily `pad` (a platform), all leaves, and
/// anything not certainly collision-free — the conservative full-cube default
/// keeps those sound.
///
/// Takes a **bare** id ([`bare_id`]), not a full block name.
pub fn is_no_collision_plant(id: &str) -> bool {
    id.ends_with("_sapling")
        || matches!(
            id,
            // grasses + ground cover
            "short_grass"
                | "tall_grass"
                | "fern"
                | "large_fern"
                | "dead_bush"
                | "bush"
                | "firefly_bush"
                | "short_dry_grass"
                | "tall_dry_grass"
                | "seagrass"
                | "tall_seagrass"
                | "pink_petals"
                | "wildflowers"
                | "leaf_litter"
                | "hanging_roots"
                | "mangrove_propagule"
                // small + tall flowers
                | "dandelion"
                | "poppy"
                | "blue_orchid"
                | "allium"
                | "azure_bluet"
                | "red_tulip"
                | "orange_tulip"
                | "white_tulip"
                | "pink_tulip"
                | "oxeye_daisy"
                | "cornflower"
                | "lily_of_the_valley"
                | "wither_rose"
                | "torchflower"
                | "sunflower"
                | "lilac"
                | "rose_bush"
                | "peony"
                | "pitcher_plant"
                // mushrooms + nether flora
                | "brown_mushroom"
                | "red_mushroom"
                | "crimson_fungus"
                | "warped_fungus"
                | "crimson_roots"
                | "warped_roots"
                | "nether_sprouts"
                | "nether_wart"
                // crops
                | "wheat"
                | "carrots"
                | "potatoes"
                | "beetroots"
                | "melon_stem"
                | "pumpkin_stem"
                | "attached_melon_stem"
                | "attached_pumpkin_stem"
                | "torchflower_crop"
                | "sweet_berry_bush"
                | "sugar_cane"
                | "bamboo_sapling"
                // climbing / hanging plants
                | "vine"
                | "glow_lichen"
                | "spore_blossom"
                | "small_dripleaf"
                | "kelp"
                | "kelp_plant"
                | "cave_vines"
                | "cave_vines_plant"
                | "twisting_vines"
                | "twisting_vines_plant"
                | "weeping_vines"
                | "weeping_vines_plant"
        )
}

/// Vanilla's **no-collision fixture class**: the things a builder hangs on a wall
/// or lays on a floor, whose collision shape is EMPTY.
///
/// This is the class the owner's report named — *a torch counting as a solid
/// block is obviously an engine bug*. A wall torch occupies the air cell beside
/// the wall it is fixed to; a body walks through that cell in the game, and a
/// model that calls it a full cube severs a corridor for every proof downstream.
/// The same is true of a sign, a banner, a lever, a button, a rail and a
/// pressure plate.
///
/// Sources: Minecraft Java 1.21.11 block shapes — every id here is declared
/// `noCollission()`.
///
/// **Deliberately excluded, and the reason is not that they collide.** `fire`,
/// `soul_fire`, `cobweb`, `nether_portal` and `end_portal` have empty collision
/// boxes too, and are left in the full-cube default on purpose: a body passing
/// through one is not a body that may be *routed* through one, and this module's
/// answer is consumed by proofs that would credit the step. Reading them as walls
/// over-blocks, which is the direction this module's errors are allowed to run.
///
/// Also excluded because their shapes were not read out of the pin: lanterns,
/// chains, end rods, ladders. The full-cube default keeps them sound.
///
/// Takes a **bare** id ([`bare_id`]), not a full block name.
pub fn is_no_collision_fixture(id: &str) -> bool {
    id.ends_with("_torch")
        || id.ends_with("_sign")
        || id.ends_with("_banner")
        || id.ends_with("_button")
        || id.ends_with("_rail")
        || matches!(
            id,
            "torch"
                | "lever"
                | "rail"
                | "redstone_wire"
                // A cell a data pack lights or leaves undecided: neither is a
                // block a body meets.
                | "light"
                | "structure_void"
        )
}

/// The height of a block's **collision box top face**, in sixteenths of a block
/// (0 = no collision at all, 16 = a full cube). Anything not listed is a full
/// cube — the conservative default.
///
/// Modelling a slab or a snow layer as a full 1×1×1 cube misplaces the surface a
/// walker stands on by up to a whole block, which makes the step rule prove
/// step-ups vanilla refuses (from a bottom slab up onto a full block is a
/// **1.5-block** rise, above the ~1.25-block jump apex) and refuse step-ups
/// vanilla allows (onto a bottom slab is a 0.5-block auto-step needing no jump
/// headroom at all).
///
/// Sources (Minecraft Java 1.21.11 block shapes):
/// - **Slabs**: `type=bottom` occupies the lower half → 8; `type=top` and
///   `type=double` reach the cell top → 16. `type` **defaults to `bottom`**, so a
///   bare `minecraft:oak_slab` is a half-step.
/// - **Snow layers** (`minecraft:snow`): collision height is `(layers - 1) * 2`
///   sixteenths (`SnowLayerBlock` indexes its shape table at `layers - 1`), so
///   `layers=1` has **no** collision box (you walk through it) and `layers=8` is
///   14/16. The *outline* shape is `layers * 2`, which is what the block looks
///   like — the collision box is what a walker stands on. `layers` defaults to 1.
/// - **Carpets**: 1/16 (`pale_moss_carpet` only when `bottom=true`; its default
///   wall-vine form has no collision box at all).
/// - **Candles**: 6/16 for every count (`CandleBlock`'s four shapes are all
///   `…,0,…→…,6,…`), so a candle on a floor is stepped over, never onto.
/// - **Flower pots** (`flower_pot`, `potted_*`): 6/16.
/// - **`dirt_path` / `farmland`**: 15/16 (the one-pixel dip you step down into).
/// - **Empty**: [`is_no_collision_plant`] and [`is_no_collision_fixture`] → 0.
pub fn collision_top_16(name: &str) -> u8 {
    let id = bare_id(name);
    if id.ends_with("_slab") {
        return match state_value(name, "type") {
            Some("top") | Some("double") => FULL_HEIGHT_16,
            // `bottom` — and the default state, which omits the property.
            _ => 8,
        };
    }
    if id == "snow" {
        let layers: u8 = state_value(name, "layers")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        return layers.clamp(1, 8).saturating_sub(1) * 2;
    }
    if id == "pale_moss_carpet" {
        // The odd one out: a floor carpet only when `bottom=true` (1/16); its
        // wall-vine form (`bottom=false`, the default) has NO collision box.
        return u8::from(state_value(name, "bottom") == Some("true"));
    }
    if id.ends_with("_carpet") || id == "moss_carpet" {
        return 1;
    }
    if id == "candle" || id.ends_with("_candle") {
        return 6;
    }
    if id == "flower_pot" || id.starts_with("potted_") {
        return 6;
    }
    if matches!(id, "dirt_path" | "farmland") {
        return 15;
    }
    if is_air(name)
        || is_passable_trap_trigger(name)
        || is_no_collision_plant(id)
        || is_no_collision_fixture(id)
    {
        return 0;
    }
    FULL_HEIGHT_16
}

/// Whether a block is thin enough to be **walked over rather than onto**
/// ([`THIN_HEIGHT_16`]): its cell is passable and never a floor level of its own,
/// so a walker standing there rests on the block below it. Vanilla agrees — none
/// of these blocks obstructs a 1.8-block-tall walker, and every one of them is
/// under the 0.6-block auto-step.
pub fn is_thin_decoration(name: &str) -> bool {
    collision_top_16(name) < THIN_HEIGHT_16
}

/// Whether a block is a **partial-height floor**: it fills its cell for passage
/// purposes but its walkable top face sits below the cell top (a bottom slab, a
/// deep snow drift). Its height is [`collision_top_16`].
pub fn is_partial_floor(name: &str) -> bool {
    let h = collision_top_16(name);
    (THIN_HEIGHT_16..FULL_HEIGHT_16).contains(&h)
}

/// **What a block state does to a body.** The classification every walk in this
/// workspace reads; see the module header for the table it stands for.
///
/// Deliberately an enum rather than a pair of booleans: the six classes are not
/// the four corners of two independent questions, and naming them is what lets a
/// caller that needs more than passability — a gate is openable, a fluid drowns —
/// ask for the distinction instead of re-deriving it from a block name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collision {
    /// One of the three air blocks: nothing here at all.
    Air,
    /// An empty or sub-auto-step collision box — a torch, a carpet, a candle, a
    /// pressure plate, a tuft of grass. Carries the measured top face, which is
    /// under [`THIN_HEIGHT_16`] by construction and is **not** a floor level.
    Thin(u8),
    /// A floor whose walkable top face is below the cell top: a bottom slab, a
    /// deep snow drift, a `dirt_path`. Carries that height in sixteenths.
    PartialFloor(u8),
    /// A full 1×1×1 cube — the conservative default for everything unrecognised.
    FullCube,
    /// A fence or a wall: 1.5 blocks tall on a 1-block cell, so a body neither
    /// passes through it nor reaches its top.
    TallBarrier,
    /// A fence gate: a body opens it with the adventure-legal right-click and
    /// walks through. Never a floor.
    FenceGate,
    /// Water or lava filling the cell: a body swims, and stands on nothing.
    Fluid,
}

impl Collision {
    /// **Can a body's own volume occupy this cell?**
    pub fn passes_body(self) -> bool {
        matches!(
            self,
            Collision::Air | Collision::Thin(_) | Collision::FenceGate
        )
    }

    /// **Can a body stand on this cell?** Not the complement of
    /// [`Collision::passes_body`], and that is the whole reason it is a separate
    /// question — a torch is neither, and lava is neither from the other side.
    pub fn supports_body(self) -> bool {
        matches!(self, Collision::PartialFloor(_) | Collision::FullCube)
    }

    /// The walkable top face of this block, in sixteenths above its own cell
    /// floor, for a class that supports a body. `None` for every class that does
    /// not, so a caller cannot silently read a height off a torch.
    pub fn floor_top_16(self) -> Option<u8> {
        match self {
            Collision::PartialFloor(h) => Some(h),
            Collision::FullCube => Some(FULL_HEIGHT_16),
            _ => None,
        }
    }
}

/// Classify a vanilla block state — **the one rule**.
///
/// The order of the arms is the rule: a fluid is a fluid whatever else it looks
/// like; a thin decoration is stepped over before anything asks whether it is a
/// gate; and only then do the two 1.5-tall classes separate from the ordinary
/// floor.
pub fn collision_class(name: &str) -> Collision {
    if is_air(name) {
        return Collision::Air;
    }
    if is_fluid(name) {
        return Collision::Fluid;
    }
    let top = collision_top_16(name);
    if top < THIN_HEIGHT_16 {
        return Collision::Thin(top);
    }
    if is_fence_gate(name) {
        return Collision::FenceGate;
    }
    if is_tall_barrier(name) {
        return Collision::TallBarrier;
    }
    if top < FULL_HEIGHT_16 {
        return Collision::PartialFloor(top);
    }
    Collision::FullCube
}

/// **Can a body's own volume occupy a cell holding this block?** —
/// [`collision_class`] then [`Collision::passes_body`].
pub fn passes_body(name: &str) -> bool {
    collision_class(name).passes_body()
}

/// **Can a body stand on a cell holding this block?** —
/// [`collision_class`] then [`Collision::supports_body`].
pub fn supports_body(name: &str) -> bool {
    collision_class(name).supports_body()
}

/// The walkable top face of this block in sixteenths, or `None` when a body
/// cannot stand on it at all — [`collision_class`] then
/// [`Collision::floor_top_16`].
pub fn floor_top_16(name: &str) -> Option<u8> {
    collision_class(name).floor_top_16()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table in the module header, asserted row by row. Every other test here
    /// is a measurement of one block; this one is the RULE, and it is what reds if
    /// somebody decides a fence gate should seal or a fluid should hold a body up.
    #[test]
    fn the_rule_is_the_table() {
        let rows: &[(Collision, bool, bool, Option<u8>)] = &[
            (Collision::Air, true, false, None),
            (Collision::Thin(0), true, false, None),
            (Collision::Thin(6), true, false, None),
            (Collision::PartialFloor(8), false, true, Some(8)),
            (Collision::PartialFloor(15), false, true, Some(15)),
            (Collision::FullCube, false, true, Some(16)),
            (Collision::TallBarrier, false, false, None),
            (Collision::FenceGate, true, false, None),
            (Collision::Fluid, false, false, None),
        ];
        assert_eq!(rows.len(), 9, "the table lost a row");
        for &(class, passes, supports, top) in rows {
            assert_eq!(class.passes_body(), passes, "{class:?}: passes_body");
            assert_eq!(class.supports_body(), supports, "{class:?}: supports_body");
            assert_eq!(class.floor_top_16(), top, "{class:?}: floor_top_16");
        }
    }

    /// **The owner's case.** A torch, a candle, a carpet and a pressure plate are
    /// things a builder puts in a room she expects to walk across. Every one of
    /// them answered "full cube" before spec-0056 — the wall a body could not get
    /// past.
    #[test]
    fn a_torch_is_not_a_wall() {
        for id in [
            "minecraft:torch",
            "minecraft:wall_torch",
            "minecraft:soul_torch",
            "minecraft:soul_wall_torch",
            "minecraft:redstone_torch",
            "minecraft:redstone_wall_torch",
            "minecraft:white_candle",
            "minecraft:candle[candles=4,lit=true]",
            "minecraft:red_carpet",
            "minecraft:stone_pressure_plate",
            "minecraft:oak_pressure_plate",
            "minecraft:lever",
            "minecraft:stone_button",
            "minecraft:oak_sign",
            "minecraft:oak_wall_sign",
            "minecraft:white_wall_banner",
            "minecraft:rail",
            "minecraft:powered_rail",
            "minecraft:redstone_wire",
            "minecraft:flower_pot",
            "minecraft:potted_cactus",
            "minecraft:glow_lichen",
            "minecraft:snow[layers=1]",
            // Namespaceless spellings reach the same answer.
            "torch",
            "red_carpet",
        ] {
            assert!(passes_body(id), "a body must get past {id}");
            assert!(!supports_body(id), "and must not stand on {id}");
            assert!(collision_top_16(id) < THIN_HEIGHT_16, "{id} is not thin");
        }
    }

    /// The other direction: the conservative default is still a wall, and the
    /// no-collision blocks this module deliberately declines to admit are still
    /// walls. A green here that came from admitting everything would be no gate
    /// at all.
    #[test]
    fn the_conservative_default_still_refuses() {
        for id in [
            "minecraft:stone",
            "minecraft:oak_stairs[facing=north]",
            "minecraft:oak_door[half=lower]",
            "minecraft:ladder",
            "minecraft:lantern",
            "minecraft:chain",
            "minecraft:end_rod",
            "minecraft:iron_bars",
            // Empty collision in vanilla, refused here on purpose: a body that
            // passes through is not a body that may be routed through.
            "minecraft:fire",
            "minecraft:cobweb",
            "minecraft:nether_portal",
        ] {
            assert!(!passes_body(id), "{id} must still stop a body");
            assert_eq!(collision_class(id), Collision::FullCube, "{id}");
        }
    }

    /// Two classes that are neither passable nor floor, and one that is passable
    /// and not floor — the three rows the naive complement gets wrong.
    #[test]
    fn tall_barriers_gates_and_fluids_are_not_the_complement_of_each_other() {
        for fence in ["minecraft:oak_fence", "minecraft:cobblestone_wall"] {
            assert_eq!(collision_class(fence), Collision::TallBarrier, "{fence}");
            assert!(!passes_body(fence) && !supports_body(fence), "{fence}");
        }
        for gate in [
            "minecraft:oak_fence_gate",
            "minecraft:oak_fence_gate[open=false]",
        ] {
            assert_eq!(collision_class(gate), Collision::FenceGate, "{gate}");
            assert!(passes_body(gate) && !supports_body(gate), "{gate}");
        }
        for fluid in [
            "minecraft:water",
            "water[level=3]",
            "minecraft:lava",
            "lava",
        ] {
            assert_eq!(collision_class(fluid), Collision::Fluid, "{fluid}");
            assert!(!passes_body(fluid) && !supports_body(fluid), "{fluid}");
        }
        // A waterlogged block is its host block, and a body stands on it.
        let stair = "minecraft:oak_stairs[facing=north,waterlogged=true]";
        assert_eq!(collision_class(stair), Collision::FullCube);
        assert!(supports_body(stair));
    }

    /// State sensitivity: the same block id, two collision boxes. A model that
    /// dropped the state would answer one of each pair wrong.
    #[test]
    fn the_state_decides_the_height() {
        let cases: &[(&str, u8)] = &[
            ("minecraft:oak_slab", 8),
            ("minecraft:oak_slab[type=bottom]", 8),
            ("minecraft:oak_slab[type=top]", 16),
            ("minecraft:oak_slab[type=double]", 16),
            ("minecraft:snow", 0),
            ("minecraft:snow[layers=1]", 0),
            ("minecraft:snow[layers=5]", 8),
            ("minecraft:snow[layers=8]", 14),
            ("minecraft:red_carpet", 1),
            ("minecraft:moss_carpet", 1),
            ("minecraft:pale_moss_carpet", 0),
            ("minecraft:pale_moss_carpet[bottom=true]", 1),
            ("minecraft:dirt_path", 15),
            ("minecraft:farmland", 15),
            ("minecraft:stone", 16),
        ];
        for &(name, want) in cases {
            assert_eq!(collision_top_16(name), want, "{name}");
        }
        assert_eq!(
            floor_top_16("minecraft:oak_slab[type=bottom]"),
            Some(8),
            "a bottom slab is a floor at half height, not a full cube"
        );
        assert_eq!(
            floor_top_16("minecraft:snow[layers=5]"),
            Some(8),
            "a five-layer drift is a floor at half height"
        );
        assert_eq!(
            floor_top_16("minecraft:torch"),
            None,
            "nothing stands on a torch, so it has no floor height to read"
        );
    }

    /// A block whose name merely contains a class's name is not in that class.
    #[test]
    fn a_name_that_contains_a_class_is_not_in_it() {
        for id in [
            "minecraft:water_cauldron",
            "minecraft:lava_cauldron",
            "minecraft:torchflower",
            "minecraft:white_candle_cake",
        ] {
            assert!(!is_fluid(id), "{id}");
        }
        // `torchflower` is a plant with an empty collision box; it reaches that
        // answer through the plant class, not through the `_torch` suffix.
        assert!(is_no_collision_plant("torchflower"));
        assert!(!is_no_collision_fixture("torchflower"));
        // A candle CAKE is a cake: a full-cube default, not a 6/16 candle.
        assert_eq!(
            collision_class("minecraft:white_candle_cake"),
            Collision::FullCube
        );
    }
}
