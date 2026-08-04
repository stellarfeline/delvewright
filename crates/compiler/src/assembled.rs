//! The shared assembled-world block model (task #42).
//!
//! One authoritative cell→block map of the world the shipped delve actually
//! assembles, built the way vanilla does: place each prefab structure with
//! `/place template <pos> <rotation>`, apply the solver's socket seals (air fill
//! opens a mated socket; wall material seals an unused one), clear gate
//! thresholds — **and then settle gravity-affected blocks**, because the delve
//! ships into a `the_void` flat world (validation/compose.yaml) with no natural
//! floor, so an unsupported `sand`/`gravel`/… block placed by `/place template`
//! immediately falls out of the world and leaves air.
//!
//! Both downstream models derive from this one map (CLAUDE.md "no hacks at any
//! layer"; the fix belongs in the shared model, not at each consumer):
//!
//! - [`crate::nav::World`] — collision/standability for `move-npc`, cutscene,
//!   the DW0311 critical-path walk, DW0312 wave seating, and the waypoint export.
//! - [`crate::light::LightModel`] — opacity/emission for the spec-0010 relight.
//!
//! ## Why settling is required for fidelity (task #42)
//!
//! Field bug: the cave-den prefab floor is a single layer of blocks over void.
//! Where that layer is `minecraft:sand` (a [`FallingBlock`], gravity-affected),
//! the block is placed by `/place template` and then falls into the void on the
//! next block update — the assembled game world has a *hole* there. The old
//! occupancy model counted every non-air block as permanent solid floor (it had
//! no gravity model), so it "proved" a solid floor the game does not have and
//! seated wave mobs / routed the player over void. Settling here makes the model
//! match the game: an unsupported falling block is removed (it despawned into the
//! void); a falling block with solid support somewhere below rests on top of it,
//! exactly as the entity would land.
//!
//! Determinism (ADR-0006): placement/seal/gate order is fixed, and settling
//! iterates `BTreeMap`-ordered columns and stacks with a fixed tie-break — same
//! DSL + seed → identical map.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use crate::plan::{Plan, ResolvedAnchor};

/// The bare block id of a (possibly blockstate-carrying) block name: strips a
/// trailing `[state]` / `{nbt}` suffix, keeping the namespace
/// (`minecraft:oak_slab[type=top]` → `minecraft:oak_slab`).
///
/// The assembled map stores **full blockstates** (task #78): waterlogging, slab
/// halves and snow-layer counts are all block *state*, and a model that throws
/// the state away cannot tell a half-step from a full cube or a submerged fence
/// from a dry one. Every classifier below therefore matches on `base_id`, and
/// the state-sensitive ones read the property they need with [`state_value`].
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

/// Rewrite a blockstate's **orientation properties** for a piece placed with
/// rotation `r`, the way vanilla's `/place template … <rotation>` does.
///
/// Model fidelity (task #191). [`placed_blocks`] rotates a prefab cell's
/// *position* via [`crate::solver::Rotation::transform`] but used to insert the
/// palette name verbatim, so for any piece with `rotation != None` the assembled
/// map disagreed with the world the server actually builds: vanilla rotates
/// blockstates during structure placement, the model did not. Nothing consumed
/// the affected properties before, which is why it never surfaced — the
/// occupancy classifiers read only `type`/`layers`/`open`/`waterlogged`/`bottom`,
/// every one of them rotation-invariant, so this correction leaves nav, seating,
/// relight, snapshots and every emitted command byte-identical (ADR-0006). The
/// stair-orientation proof (`DW0430`) is the first consumer that reads `facing`,
/// and without this it would report a false defect on every rotated piece.
///
/// Rotated, per the vanilla `BlockState::rotate` implementations:
/// - `facing` — horizontal values only; `up`/`down` (hoppers, observers) are
///   yaw-invariant and pass through.
/// - `axis` — `x` ↔ `z` under a quarter turn, unchanged under a half turn; `y`
///   never moves.
/// - `rotation` — the 16-step sign/banner/skull dial, `+4` per clockwise quarter.
/// - `north`/`south`/`east`/`west` — the connection properties of fences, walls,
///   panes, redstone wire and vines. Permuted as a set, so a wall's
///   `none`/`low`/`tall` values travel with their side.
/// - `orientation` — the crafter/jigsaw `<front>_<top>` pair; both components are
///   rotated when horizontal.
/// - `shape` — **only rail shapes** (`north_east`, `ascending_west`, …). A
///   *stair's* `shape` (`straight`, `inner_left`, `outer_right`, …) is expressed
///   relative to its own `facing`, so it is already correct once `facing` moves;
///   rotating it too would corrupt the corner. The value, not the block id,
///   decides — which is what keeps this table-free.
pub fn rotate_state(name: &str, r: crate::solver::Rotation) -> String {
    use crate::solver::Rotation;
    if r == Rotation::None {
        return name.to_string();
    }
    let Some(open) = name.find('[') else {
        return name.to_string();
    };
    let Some(close) = name[open..].find(']').map(|i| i + open) else {
        return name.to_string();
    };
    let quarter_turns = match r {
        Rotation::None => 0,
        Rotation::Cw90 => 1,
        Rotation::Cw180 => 2,
        Rotation::Ccw90 => 3,
    };
    /// The rotated value of one `k=v` pair, or `None` to keep `v` unchanged.
    /// `name` is the ORIGINAL blockstate, so the connection-property permutation
    /// reads pre-rotation values and is therefore simultaneous.
    fn rotated_value(
        name: &str,
        k: &str,
        v: &str,
        r: crate::solver::Rotation,
        quarter_turns: u32,
    ) -> Option<String> {
        use crate::solver::{Facing, Rotation};
        let rot_dir = |s: &str| Facing::parse(s).map(|f| f.rotate(r).token());
        match k {
            "facing" => rot_dir(v).map(str::to_string),
            "axis" => match (v, quarter_turns % 2) {
                ("x", 1) => Some("z".to_string()),
                ("z", 1) => Some("x".to_string()),
                _ => None,
            },
            "rotation" => v
                .parse::<u32>()
                .ok()
                .map(|n| ((n + 4 * quarter_turns) % 16).to_string()),
            // The value now under `k` is the one that sat on the side `k` came
            // FROM — `k` turned backwards by the same amount.
            "north" | "south" | "east" | "west" => {
                let mut src = Facing::parse(k)?;
                for _ in 0..(4 - quarter_turns) % 4 {
                    src = src.rotate(Rotation::Cw90);
                }
                state_value(name, src.token()).map(str::to_string)
            }
            // `<front>_<top>` (crafter, jigsaw) — rotate each horizontal half.
            "orientation" => {
                let (a, b) = v.split_once('_')?;
                let ra = rot_dir(a).unwrap_or(a);
                let rb = rot_dir(b).unwrap_or(b);
                Some(format!("{ra}_{rb}"))
            }
            // Rail shapes only — a stair's shape is relative to its own facing.
            "shape" => match v.split_once('_')? {
                ("ascending", d) => rot_dir(d).map(|rd| format!("ascending_{rd}")),
                (a, b) => Some(format!("{}_{}", rot_dir(a)?, rot_dir(b)?)),
            },
            _ => None,
        }
    }
    let body: Vec<String> = name[open + 1..close]
        .split(',')
        .map(|kv| {
            let Some((k, v)) = kv.split_once('=') else {
                return kv.to_string();
            };
            let (k, v) = (k.trim(), v.trim());
            let out = rotated_value(name, k, v, r, quarter_turns);
            format!("{k}={}", out.unwrap_or_else(|| v.to_string()))
        })
        .collect();
    // Keys keep their (already BTreeMap-sorted) order; only values move, so the
    // rendered name stays canonical without re-sorting. Except the connection
    // set, whose keys are themselves sorted — still stable, values permuted.
    format!("{}[{}]", &name[..open], body.join(","))
}

/// The air block variants that count as "no block" (passable / transparent).
pub fn is_air(name: &str) -> bool {
    matches!(
        base_id(name),
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// Thin, walkable trap-trigger blocks (DSL v0.6, spec-0011) a player steps *onto*
/// rather than being blocked by: pressure plates and tripwire. Their cells are
/// modelled **passable** so nav routes a player ONTO a critical-path trap trigger
/// (the hazard the `DW0342` proof reasons about) instead of routing around a
/// "solid" plate and falsely calling every trap avoidable. They are non-collidable
/// in game, so this is the faithful model; a `_pressure_plate`/`tripwire` cell
/// always rests on a solid support block below, so standability is unaffected.
pub fn is_passable_trap_trigger(name: &str) -> bool {
    let id = strip_ns(base_id(name));
    id.ends_with("_pressure_plate") || matches!(id, "tripwire" | "tripwire_hook")
}

/// The namespace-free bare id (`minecraft:oak_slab[type=top]` → `oak_slab`).
fn strip_ns(id: &str) -> &str {
    let id = base_id(id);
    id.strip_prefix("minecraft:").unwrap_or(id)
}

/// Whether a block is a **1.5-block-tall barrier** (task #59): fences (`*_fence`,
/// incl. `nether_brick_fence`) and walls (`*_wall`). Vanilla gives these a
/// collision box 1.5 blocks tall on a 1-block cell, which breaks the full-cube
/// assumption in BOTH directions:
///
/// - **Not standable on top by a walking player**: a normal jump rises ~1.25
///   blocks, so a 1.5-tall top face is unreachable by walking/jumping — the
///   full-solid model's "legal +1 step onto a fence-top" was a proof of a route no
///   player (or mineflayer bot) can walk (owner-hit island bug; harness #110).
/// - **Not passable through**: the barrier fills its cell for a walker, and its
///   top half also blocks the cell above (feet at `y+1` intersect the `y..y+1.5`
///   box), which the model gets for free because a tall barrier is never valid
///   floor.
///
/// Fence **gates** are excluded — they are the openable case, see
/// [`is_fence_gate`].
pub fn is_tall_barrier(name: &str) -> bool {
    let id = strip_ns(name);
    id.ends_with("_fence") || id.ends_with("_wall")
}

/// Whether a block is a fence gate (`*_fence_gate` — every vanilla fence gate is
/// a wooden, right-click-openable one). Closed, it is a 1.5-tall barrier like a
/// fence but **passable-with-use**: opening it is a right-click USE interaction
/// vanilla permits in adventure mode (the same action a human player performs),
/// so the nav model treats a closed gate cell as walkable for the player and tags
/// it as a "use-gate" cell in the exported critical-path waypoints. Open (block
/// state `open=true` in the prefab), the threshold is simply passable.
pub fn is_fence_gate(name: &str) -> bool {
    strip_ns(name).ends_with("_fence_gate")
}

/// Whether a palette block id is **free water** (a source or flowing block that
/// occupies its whole cell as fluid, with no host block), as opposed to a
/// waterlogged solid block ([`is_waterlogged`]).
///
/// A stored flowing-water cell (`level>0`) also reads as `minecraft:water`; the
/// flood treats every water cell as a full-strength source, which over-marks its
/// reach — deliberately safe (see [`flood`]).
pub fn is_water(name: &str) -> bool {
    base_id(name) == "minecraft:water"
}

/// Whether a block carries `waterlogged=true` — **the cell contains a water
/// source** alongside the host block (MC 1.13+ waterlogging).
///
/// Fidelity fix (task #78). The model previously stored bare block ids and its
/// own doc claimed "vanilla waterlogging never spreads to a neighbour", which is
/// factually wrong: a waterlogged block's cell holds a genuine water *source*
/// that ticks and spreads into adjacent air exactly like a free source block
/// (this is why a single waterlogged stair placed on dry land produces flowing
/// water around it). A model that ignores it under-marks the flood — the one
/// direction [`assembled_occupancy`]'s never-under-mark contract forbids, because
/// an under-marked cell ships as proven-dry and strands the bot.
///
/// So a waterlogged cell is BOTH:
/// - a flood **source** (it wets its neighbours), and
/// - its host block's normal collision class (solid / tall / gate): the cell is
///   still occupied by the block, so nothing walks or flows *into* it.
pub fn is_waterlogged(name: &str) -> bool {
    state_value(name, "waterlogged") == Some("true")
}

/// Whether a cell's block is a **fluid** — water or lava. Vanilla's
/// `FallingBlock.isFree` treats a fluid (and air, fire, and replaceable blocks)
/// as "no support": a falling block entity passes straight through it and keeps
/// falling, displacing the fluid when it finally lands on something solid. Used
/// by [`settle`]; deliberately narrower than `isFree` (replaceable plants are not
/// modelled), which only ever makes settling *more* conservative.
fn is_liquid(name: &str) -> bool {
    matches!(base_id(name), "minecraft:water" | "minecraft:lava")
}

/// Whether a block falls under gravity when the cell below cannot support it
/// (vanilla `FallingBlock`). In the delve's `the_void` world such a block, placed
/// unsupported by `/place template`, drops out of the world and leaves air — so
/// the assembled-world model must not treat it as permanent floor.
///
/// Deliberately excludes `pointed_dripstone` and `scaffolding`: those attach
/// upward / by support-distance rather than resting on the block directly below,
/// so the "supported from below" settling rule here does not model them — a cave
/// generator hangs dripstone *stalactites* from the ceiling, and this rule must
/// never mistake a ceiling-hung block for an unsupported one and delete it.
/// A full block's collision height in sixteenths — the unit [`collision_top_16`]
/// reports in. Vanilla builds every partial collision box out of sixteenths, so
/// integer sixteenths represent every case exactly (no float ordering, ADR-0006).
pub const FULL_HEIGHT_16: u8 = 16;
/// Below this collision height a block is **stepped over, not onto**: the walker's
/// feet stay on whatever supports it. 8/16 = half a block, the slab step. Anything
/// thinner (carpet 1/16, a 1–4-layer snow drift ≤ 6/16) is under the vanilla
/// 0.6-block auto-step, so modelling it as a floor *level* of its own would be
/// noise; modelling it as a full cube (the old behaviour) is a lie.
pub const THIN_HEIGHT_16: u8 = 8;

/// The height of a block's **collision box top face**, in sixteenths of a block
/// (0 = no collision at all, 16 = a full cube). Anything not listed is a full
/// cube — the conservative default, unchanged from the pre-#78 model.
///
/// Fidelity fix (task #78): modelling a slab or a snow layer as a full 1×1×1 cube
/// misplaces the surface a walker stands on by up to a whole block, which makes
/// the nav step rule prove step-ups vanilla refuses (stepping from a bottom slab
/// up onto a full block is a **1.5-block** rise — above the ~1.25-block jump
/// apex) and refuse step-ups vanilla allows (stepping onto a bottom slab is a
/// 0.5-block auto-step needing no jump headroom at all).
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
/// - **`dirt_path` / `farmland`**: 15/16 (the one-pixel dip you step down into).
pub fn collision_top_16(name: &str) -> u8 {
    let id = strip_ns(name);
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
    if matches!(id, "dirt_path" | "farmland") {
        return 15;
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

pub fn is_falling_block(name: &str) -> bool {
    let id = strip_ns(name);
    // NB: `suspicious_sand`/`suspicious_gravel` are brushable archaeology blocks,
    // NOT `FallingBlock` — they stay put, so they are deliberately excluded.
    matches!(
        id,
        "sand" | "red_sand" | "gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg"
    ) || id.ends_with("_concrete_powder")
}

/// Inclusive cell iterator over a region given two (possibly unordered) corners.
pub fn region_cells(a: [i32; 3], b: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    (lo[1]..=hi[1]).flat_map(move |y| {
        (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z]))
    })
}

/// Parse a gzipped vanilla structure `.nbt`, returning its non-air block cells as
/// `(local [x, y, z], block id)`. Unparseable structures contribute nothing.
pub fn structure_named_cells(bytes: &[u8]) -> Vec<([i32; 3], String)> {
    structure_cells(bytes)
        .into_iter()
        .map(|(pos, name, _)| (pos, name))
        .collect()
}

/// Parse a gzipped vanilla structure `.nbt`, returning its non-air block cells as
/// `(local [x, y, z], block id, open)`, where `open` is the block-state `open`
/// property when the palette entry carries one (`Some(true)` for an authored open
/// fence gate / door / trapdoor; `None` when the state has no `open` property).
/// The decoder otherwise keeps only `Name`; the assembled model uses the
/// blockstate-preserving [`structure_cells_stateful`] instead (task #78).
/// Unparseable structures contribute nothing.
pub fn structure_cells(bytes: &[u8]) -> Vec<([i32; 3], String, Option<bool>)> {
    structure_cells_inner(bytes, false)
}

/// [`structure_cells`] keeping each block's **full blockstate** string —
/// `minecraft:lantern[hanging=true]`, `minecraft:oak_log[axis=x]` — with the
/// properties in sorted key order (deterministic, ADR-0006).
///
/// The occupancy model deliberately stores bare ids (its classifiers match exact
/// names), so [`structure_cells`] stays the default. This variant exists for the
/// one consumer that must reproduce a prefab's blocks *as blocks* rather than as
/// occupancy: the stage-7 `fragment` verb, whose emitted `setblock` lines are
/// the actual runtime writes. Stamping a bare `minecraft:lantern` where the
/// prefab authored `minecraft:lantern[hanging=true]` places a floor lantern into
/// mid-air — which vanilla drops on the next chunk tick (the defect `DW0354`
/// surfaced).
pub fn structure_cells_stateful(bytes: &[u8]) -> Vec<([i32; 3], String, Option<bool>)> {
    structure_cells_inner(bytes, true)
}

fn structure_cells_inner(bytes: &[u8], stateful: bool) -> Vec<([i32; 3], String, Option<bool>)> {
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
    let palette: Vec<Option<(String, Option<bool>)>> = match root.get("palette") {
        Some(fastnbt::Value::List(entries)) => entries
            .iter()
            .map(|e| match e {
                fastnbt::Value::Compound(c) => match c.get("Name") {
                    Some(fastnbt::Value::String(s)) => {
                        let props = match c.get("Properties") {
                            Some(fastnbt::Value::Compound(p)) => Some(p),
                            _ => None,
                        };
                        let open = match props.and_then(|p| p.get("open")) {
                            Some(fastnbt::Value::String(v)) => Some(v == "true"),
                            _ => None,
                        };
                        // `fastnbt`'s compound is a `HashMap`, so the property
                        // order it yields is hash order — collect through a
                        // `BTreeMap` before rendering (ADR-0006: no hash-order
                        // iteration in the compiler).
                        let name = match (stateful, props) {
                            (true, Some(p)) if !p.is_empty() => {
                                let sorted: std::collections::BTreeMap<&String, String> = p
                                    .iter()
                                    .filter_map(|(k, v)| match v {
                                        fastnbt::Value::String(sv) => Some((k, sv.clone())),
                                        _ => None,
                                    })
                                    .collect();
                                let body = sorted
                                    .iter()
                                    .map(|(k, v)| format!("{k}={v}"))
                                    .collect::<Vec<_>>()
                                    .join(",");
                                if body.is_empty() {
                                    s.clone()
                                } else {
                                    format!("{s}[{body}]")
                                }
                            }
                            _ => s.clone(),
                        };
                        Some((name, open))
                    }
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
            if let Some(Some((name, open))) = palette.get(state)
                && !is_air(name.split('[').next().unwrap_or(name))
            {
                out.push((pos, name.clone(), *open));
            }
        }
    }
    out
}

/// The un-settled cell→block map: placed structures + solver seals + gate clears,
/// exactly as the two legacy models built it — plus the set of fence-gate cells
/// whose authored block state is `open=true` (task #59: an open gate threshold is
/// passable; a closed one is passable-with-use). Kept separate from settling so
/// unit tests can exercise each half.
fn placed_blocks(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> (BTreeMap<[i32; 3], String>, BTreeSet<[i32; 3]>) {
    let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
    let mut open_gates: BTreeSet<[i32; 3]> = BTreeSet::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let Some(bytes) = structures.get(&piece.structure_file) else {
                continue;
            };
            // Blockstate-preserving read (task #78): waterlogging, slab halves and
            // snow-layer counts are block STATE, and the fluid/step models below
            // are wrong without them. Every classifier matches on [`base_id`].
            for (local, name, open) in structure_cells_stateful(bytes) {
                // Vanilla rotates blockstates as well as positions during
                // `/place template … <rotation>` (task #191) — see [`rotate_state`].
                let name = rotate_state(&name, piece.rotation);
                let t = piece.rotation.transform(local);
                let cell = [
                    piece.pos[0] + t[0],
                    piece.pos[1] + t[1],
                    piece.pos[2] + t[2],
                ];
                if is_fence_gate(&name) && open == Some(true) {
                    open_gates.insert(cell);
                } else {
                    open_gates.remove(&cell); // a later block overwrites the cell
                }
                blocks.insert(cell, name);
            }
        }
        // Seals land after placement: an air fill opens a mated socket; anything
        // else seals an unused one. Either way the sealed cell is no longer an
        // authored open gate.
        for s in &area.seals {
            if is_air(&s.block) {
                for cell in region_cells(s.from, s.to) {
                    blocks.remove(&cell);
                    open_gates.remove(&cell);
                }
            } else {
                for cell in region_cells(s.from, s.to) {
                    blocks.insert(cell, s.block.clone());
                    open_gates.remove(&cell);
                }
            }
        }
    }
    // Horizon surround tiles (spec-0026 §5): placed pieces like any other —
    // the voxel model, gravity settle, nav and every downstream proof see
    // them. Unrotated, sealless, and outside every scene AABB by construction.
    if let Some(surround) = &plan.surround {
        for piece in &surround.pieces {
            let Some(bytes) = structures.get(&piece.structure_file) else {
                continue;
            };
            for (local, name, open) in structure_cells_stateful(bytes) {
                let cell = [
                    piece.pos[0] + local[0],
                    piece.pos[1] + local[1],
                    piece.pos[2] + local[2],
                ];
                if is_fence_gate(&name) && open == Some(true) {
                    open_gates.insert(cell);
                } else {
                    open_gates.remove(&cell);
                }
                blocks.insert(cell, name);
            }
        }
    }
    // Gate thresholds are passable (an open-gate effect fills them with air).
    for resolved in plan.anchors.values() {
        if let ResolvedAnchor::Gate { from, to, .. } = resolved {
            for cell in region_cells(*from, *to) {
                blocks.remove(&cell);
                open_gates.remove(&cell);
            }
        }
    }
    (blocks, open_gates)
}

/// The outcome of gravity settling for one falling block: the world cell it ended
/// up at (or `None` if it despawned into the void), tagged with its original cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    /// The falling block's id (e.g. `minecraft:sand`).
    pub block: String,
    /// The block's authored (placed) world cell before settling.
    pub from: [i32; 3],
    /// Where it came to rest, or `None` if it fell out of the world (despawned).
    pub to: Option<[i32; 3]>,
}

/// Settle every gravity-affected block in `blocks` as vanilla physics would in the
/// void world: within each `(x, z)` column, non-falling **solid** blocks are
/// immovable supports; each falling block drops onto the highest support at or
/// below it (stacking in original order), and a falling block with no support
/// anywhere below it despawns (falls out of the world → air). Mutates `blocks` in
/// place and returns one [`Settled`] per falling block (its authored cell and
/// where it ended up), so callers can distinguish a benign land-on-support from a
/// despawn.
///
/// **Fluids are not supports** (task #78). Vanilla's `FallingBlock.isFree` counts
/// a liquid cell as free space: a falling block sinks straight through water/lava
/// and lands on the first genuinely solid block, *displacing* the fluid in the
/// cell it comes to rest in. The pre-#78 model treated a `minecraft:water` cell as
/// an immovable support, so a sand block authored over a pool "settled on the
/// water surface" — a floating floor the game does not have, which the flood then
/// dammed and nav then walked on. See [`is_liquid`].
///
/// Deterministic (ADR-0006): columns iterate in `BTreeMap` order and blocks stack
/// bottom-up.
fn settle(blocks: &mut BTreeMap<[i32; 3], String>) -> Vec<Settled> {
    // Group cell y's by column.
    let mut columns: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
    for c in blocks.keys() {
        columns.entry((c[0], c[2])).or_default().push(c[1]);
    }
    let mut outcomes: Vec<Settled> = Vec::new();
    for ((x, z), mut ys) in columns {
        ys.sort_unstable();
        // Split the column into immovable supports and falling blocks (ascending).
        // A liquid cell is NEITHER: it does not hold a falling block up, and a
        // block that lands in it replaces it.
        let mut fixed: Vec<i32> = Vec::new();
        let mut falling: Vec<(i32, String)> = Vec::new();
        for y in &ys {
            let name = &blocks[&[x, *y, z]];
            if is_falling_block(name) {
                falling.push((*y, name.clone()));
            } else if !is_liquid(name) {
                fixed.push(*y);
            }
        }
        if falling.is_empty() {
            continue; // nothing to settle in this column
        }
        // Lift every falling block out; re-drop onto its support.
        for (y, _) in &falling {
            blocks.remove(&[x, *y, z]);
        }
        // Group falling blocks by the nearest immovable support strictly below
        // them; a group has no support → those blocks despawned into the void.
        let mut by_base: BTreeMap<i32, Vec<(i32, String)>> = BTreeMap::new();
        for (y, name) in falling {
            if let Some(base) = fixed.iter().copied().filter(|&f| f < y).max() {
                by_base.entry(base).or_default().push((y, name));
            } else {
                // No support anywhere below → despawns into the void.
                outcomes.push(Settled {
                    block: name,
                    from: [x, y, z],
                    to: None,
                });
            }
        }
        for (base, group) in by_base {
            // Stack from base+1 upward; the group came from the open gap above
            // `base`, so it cannot overrun the next support — but skip any fixed
            // cell defensively. A liquid cell in the way is deliberately NOT
            // skipped: the landing block displaces the fluid (the insert below
            // overwrites it), exactly as sand dropped into a pond does.
            let mut yy = base + 1;
            for (from_y, name) in group {
                while fixed.binary_search(&yy).is_ok() {
                    yy += 1;
                }
                outcomes.push(Settled {
                    block: name.clone(),
                    from: [x, from_y, z],
                    to: Some([x, yy, z]),
                });
                blocks.insert([x, yy, z], name);
                yy += 1;
            }
        }
    }
    outcomes
}

/// Re-run gravity settling over an (edited) cell→block map — the map editor's
/// re-entry into the settle model (spec-0017): a `setblock`-placed falling
/// block falls exactly like a template-placed one, so after every edit batch
/// the model re-settles and the same despawn rule applies. Same algorithm,
/// same determinism as the assembly-time [`settle`].
pub(crate) fn resettle(blocks: &mut BTreeMap<[i32; 3], String>) -> Vec<Settled> {
    settle(blocks)
}

/// The authoritative assembled-world model: the settled cell→block map plus the
/// per-falling-block settle outcomes (for the gravity-despawn diagnostic).
pub struct Assembled {
    /// The gravity-settled cell→block map (cells absent from it are air).
    pub blocks: BTreeMap<[i32; 3], String>,
    /// One outcome per falling block: where it came to rest, or `None` if it
    /// despawned into the void.
    pub settled: Vec<Settled>,
    /// Fence-gate cells whose authored block state is `open=true` (task #59):
    /// passable thresholds, as opposed to closed gates (passable-with-use).
    pub open_gates: BTreeSet<[i32; 3]>,
}

/// Assemble the world: placed structures + solver seals + gate clears, then
/// gravity-settle, returning both the settled map and the per-falling-block
/// outcomes. Shared root for [`assembled_blocks`] and the gravity-despawn check.
pub fn assemble(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Assembled {
    let (mut blocks, open_gates) = placed_blocks(plan, structures);
    let settled = settle(&mut blocks);
    Assembled {
        blocks,
        settled,
        open_gates,
    }
}

/// The authoritative assembled-world cell→block map: placed structures + solver
/// seals + gate clears, **then gravity-settled**. Cells absent from the map are
/// air. Shared by the nav occupancy model and the relight light model so a single
/// gravity-faithful world feeds every consumer (task #42).
pub fn assembled_blocks(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<[i32; 3], String> {
    assemble(plan, structures).blocks
}

/// The standard vanilla horizontal flow decay: a water source spreads at most this
/// many cells horizontally before running dry (level 1..=7 → 7 steps).
const WATER_FLOW_RANGE: u8 = 7;

/// The collision-classified nav occupancy of the settled assembled world (tasks
/// #45, #59). The sets are pairwise disjoint; a cell in none of them is passable
/// air.
///
/// | Set | Blocks | Walk through? | Stand on top? |
/// |---|---|---|---|
/// | `solid` | every other non-air block (full cube unless listed in `partial`) | no | yes |
/// | `tall` | fences + walls ([`is_tall_barrier`], 1.5-tall) | no | **no** |
/// | `use_gates` | closed fence gates ([`is_fence_gate`], 1.5-tall, openable) | player: yes, via right-click USE; NPC/actor/wave walkers: no | **no** |
/// | `flooded` | water reach (conservative superset) | no | no |
/// | `partial` | a `solid` cell's true top-face height, in sixteenths, when < 16 | no | yes, **at that height** |
///
/// **Modelled precisely**: fences, walls, fence gates (open = passable, closed =
/// use-gate), pressure plates / tripwire and every other sub-auto-step decoration
/// (passable, [`is_thin_decoration`]), free water AND waterlogged blocks
/// ([`is_waterlogged`]), and partial floor heights for slabs / snow layers /
/// paths ([`collision_top_16`]). **Modelled conservatively** (treated as a full
/// solid cube — may over-block a route, never over-prove one): stairs, doors,
/// trapdoors, and every other partial-collision block.
pub struct Occupancy {
    /// Full-cube solid cells: block passage AND are valid floor.
    pub solid: BTreeSet<[i32; 3]>,
    /// 1.5-tall barrier cells (fences/walls): block passage, never valid floor.
    pub tall: BTreeSet<[i32; 3]>,
    /// Closed fence-gate cells: passable-with-use for the player (adventure-legal
    /// right-click), impassable for walkers that cannot use gates; never floor.
    pub use_gates: BTreeSet<[i32; 3]>,
    /// Water-flooded cells: impassable, never floor (task #45). Disjoint from
    /// every block set — a waterlogged cell is its host block's class, not this.
    pub flooded: BTreeSet<[i32; 3]>,
    /// For each `solid` cell whose walkable top face is **below** the cell top,
    /// that height in sixteenths of a block (task #78). Absent = a full cube
    /// (16/16). Drives the nav step rule's true rise between two standing cells.
    pub partial: BTreeMap<[i32; 3], u8>,
}

/// The nav occupancy of the settled assembled world (tasks #45, #59) — see
/// [`Occupancy`] for the collision classes.
///
/// ## Why water is modelled, and why as a *superset* of vanilla flow
///
/// A `minecraft:water` block placed by `/place template` does not stay put: vanilla
/// fluid physics spreads it at world-load into neighbouring air cells the prefab
/// left empty. The compile-time model must reflect that, or nav proves a route/seat
/// standable on a cell the game floods — the water analogue of the gravity-despawn
/// divergence (task #42). Field case: the `cave-shore` pool floods `[261,66,1]`, a
/// cell an unpatched model routed the perimedes talk-to leg's step-up through.
///
/// [`flood`] is a deliberate **conservative superset** of real flow, mirroring
/// spec-0010's never-overestimate-walkability stance: it may mark a cell wet that
/// vanilla would leave dry, but never the reverse. Over-marking can only turn a
/// proof red (caught, escalated); under-marking would let a wet cell ship as
/// proven-dry (a silent bot strand). So downstream nav/wave/relight/waypoint
/// consumers treat every flooded cell as impassable and never as standable floor.
///
/// It models **two** vanilla mechanics that both matter for standability:
/// - **Infinite-water source formation** (pooling): a supported flowing cell with
///   ≥2 horizontally-adjacent source cells becomes a new source, cascading — so a
///   walled basin seeded by prefab sources fills completely, not just 7 cells out.
///   Missing this silently under-marks a pool's edge (the perimedes field case: the
///   `cave-shore` prefab omits the middle of one source row, and vanilla's infinite
///   water fills the gap, pushing the flow one cell further onto `[261,66,1]`).
/// - **7-level flow decay** from the (now complete) source set, plus infinite
///   downward flow. Vanilla's drop-seeking *direction* preference is omitted (spread
///   goes every way with decay), which only ever over-marks — preserving the
///   superset guarantee.
///
/// Settle runs **before** flood: a settled sand column can dam or open a channel, so
/// the flood must see the post-gravity geometry (task #42 order preserved).
pub fn assembled_occupancy(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Occupancy {
    let assembled = assemble(plan, structures);
    occupancy_of(assembled.blocks, &assembled.open_gates)
}

/// Pure core of [`assembled_occupancy`]: classify a settled cell→block map into an
/// [`Occupancy`]. `open_gates` is the set of fence-gate cells authored `open=true`
/// (they are passable; closed gates become `use_gates`). Split out so the flood
/// and the collision classes are unit-testable without a [`Plan`].
///
/// Vanilla water flows only into **air** cells, so *every* non-air block dams the
/// flood — fences, walls, and gates (open or closed) included, exactly as the
/// pre-classification full-solid model had it. The flood barrier set is therefore
/// the union of every classified block cell, keeping the water model byte-stable.
pub fn occupancy_of(
    blocks: BTreeMap<[i32; 3], String>,
    open_gates: &BTreeSet<[i32; 3]>,
) -> Occupancy {
    let mut solid: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut tall: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut use_gates: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut barriers: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut sources: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut partial: BTreeMap<[i32; 3], u8> = BTreeMap::new();
    for (cell, name) in &blocks {
        // A waterlogged block's cell holds a real water source that spreads into
        // its air neighbours (task #78) — seed the flood from it, then classify
        // the host block normally below (the cell itself stays occupied).
        if is_waterlogged(name) {
            sources.insert(*cell);
        }
        if is_water(name) {
            sources.insert(*cell);
        } else if is_passable_trap_trigger(name) || is_thin_decoration(name) {
            // A pressure plate / tripwire / carpet / thin snow drift is walkable
            // floor decoration, not an obstacle (spec-0011) — leave its cell
            // passable so the trap-trigger cell stays on the walkable path, and
            // so a 2-high corridor with a carpet in it stays walkable.
        } else if is_fence_gate(name) {
            barriers.insert(*cell);
            if !open_gates.contains(cell) {
                use_gates.insert(*cell); // closed: passable-with-use (task #59)
            } // open: a passable threshold (still dams water)
        } else if is_tall_barrier(name) {
            barriers.insert(*cell);
            tall.insert(*cell);
        } else {
            barriers.insert(*cell);
            solid.insert(*cell);
            let h = collision_top_16(name);
            if h < FULL_HEIGHT_16 {
                partial.insert(*cell, h);
            }
        }
    }
    // The sets stay pairwise disjoint: a waterlogged (or fluid-adjacent) barrier
    // cell is already impassable as its host class, so it must not also appear as
    // free water — `flooded` means "a walker would be in open water here".
    let mut flooded = flood(&barriers, &sources);
    flooded.retain(|c| !barriers.contains(c));
    Occupancy {
        solid,
        tall,
        use_gates,
        flooded,
        partial,
    }
}

/// The four cardinal horizontal steps, in a fixed order (determinism, ADR-0006).
const HORIZ4: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// A deterministic, conservative **superset** of vanilla water flow from `sources`
/// through the air cells of a world whose solid barriers are `solid`. Returns every
/// flooded cell (the sources themselves plus every air cell water reaches).
///
/// Two phases (see the [`assembled_occupancy`] module note for the field case):
/// 1. [`form_sources`] grows the source set by vanilla **infinite-water** pooling —
///    a supported air cell flanked by ≥2 source cells becomes a source, cascading —
///    so a walled basin fills completely rather than only 7 cells from its seeds.
/// 2. [`spread`] then flows the completed source set outward (7-level decay +
///    infinite downward), omitting only vanilla's drop-seeking direction preference
///    (a safe over-mark).
fn flood(solid: &BTreeSet<[i32; 3]>, sources: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    if sources.is_empty() {
        return BTreeSet::new();
    }
    let pooled = form_sources(solid, sources);
    spread(solid, &pooled)
}

/// Whether cell `c` has support below (a solid floor or standing water) — the
/// condition under which vanilla water pools/forms a source instead of flowing down.
fn supported_below(solid: &BTreeSet<[i32; 3]>, sources: &BTreeSet<[i32; 3]>, c: [i32; 3]) -> bool {
    let below = [c[0], c[1] - 1, c[2]];
    solid.contains(&below) || sources.contains(&below)
}

/// Grow `init` by vanilla infinite-water source formation: a **supported** air cell
/// with ≥2 horizontally-adjacent source cells becomes a source, cascading to
/// fixpoint. This fills a walled basin (the field case: the `cave-shore` pool's
/// omitted source-row middle) that plain decay would leave dry one cell short.
///
/// Deterministic (ADR-0006): a queue cascade guarded by a `BTreeSet`; the fixpoint
/// source set is independent of visit order.
fn form_sources(solid: &BTreeSet<[i32; 3]>, init: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    use std::collections::VecDeque;
    let mut sources = init.clone();
    // Seed the cascade with the open (air) cells neighbouring a source — the only
    // cells that can newly qualify.
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    for &s in &sources {
        for &c in &open_neighbours(solid, &sources, s) {
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        if sources.contains(&c) || solid.contains(&c) {
            continue;
        }
        if !supported_below(solid, &sources, c) {
            continue;
        }
        let adjacent_sources = HORIZ4
            .iter()
            .filter(|(dx, dz)| sources.contains(&[c[0] + dx, c[1], c[2] + dz]))
            .count();
        if adjacent_sources >= 2 {
            sources.insert(c);
            // c is a new source: its air neighbours (and the cell above, for stacked
            // pools) may now qualify — re-examine them.
            for &n in &open_neighbours(solid, &sources, c) {
                queue.push_back(n);
            }
            let up = [c[0], c[1] + 1, c[2]];
            if !solid.contains(&up) && !sources.contains(&up) {
                queue.push_back(up);
            }
        }
    }
    sources
}

/// The horizontally-adjacent cells of `c` that are open (not solid, not already a
/// source) — the candidates a new source can propagate pooling into.
fn open_neighbours(
    solid: &BTreeSet<[i32; 3]>,
    sources: &BTreeSet<[i32; 3]>,
    c: [i32; 3],
) -> Vec<[i32; 3]> {
    HORIZ4
        .iter()
        .map(|(dx, dz)| [c[0] + dx, c[1], c[2] + dz])
        .filter(|n| !solid.contains(n) && !sources.contains(n))
        .collect()
}

/// Flow `sources` outward through air: infinite downward fall (level reset to 0 on
/// the way down) plus horizontal spread with 1-per-step decay up to
/// [`WATER_FLOW_RANGE`], blocked by solid cells. Returns every wet cell.
///
/// Deterministic (ADR-0006): 0-1 BFS (down = level-0 edge, horizontal = +1 edge)
/// seeded from `sources` in sorted order with a fixed neighbour order; the resulting
/// wet-cell set is independent of visit order. Downward is bounded at the lowest
/// solid/source cell — a column into the void reaches no standable ground.
fn spread(solid: &BTreeSet<[i32; 3]>, sources: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    use std::collections::VecDeque;
    let Some(min_y) = solid
        .iter()
        .chain(sources.iter())
        .map(|c| c[1])
        .min()
        .map(|y| y - 1)
    else {
        return BTreeSet::new();
    };
    let mut level: BTreeMap<[i32; 3], u8> = BTreeMap::new();
    let mut dq: VecDeque<[i32; 3]> = VecDeque::new();
    for &s in sources {
        level.insert(s, 0);
        dq.push_back(s);
    }
    while let Some(c) = dq.pop_front() {
        let l = level[&c];
        // Downward: infinite fall, resets to full strength (level 0). A 0-cost edge.
        let d = [c[0], c[1] - 1, c[2]];
        if d[1] >= min_y && !solid.contains(&d) && level.get(&d).is_none_or(|&e| e > 0) {
            level.insert(d, 0);
            dq.push_front(d);
        }
        // Horizontal: cardinal spread with 1-per-step decay, blocked by solid. A
        // +1-cost edge; stops at the flow range.
        if l < WATER_FLOW_RANGE {
            for (dx, dz) in HORIZ4 {
                let n = [c[0] + dx, c[1], c[2] + dz];
                if !solid.contains(&n) && level.get(&n).is_none_or(|&e| e > l + 1) {
                    level.insert(n, l + 1);
                    dq.push_back(n);
                }
            }
        }
    }
    level.into_keys().collect()
}

/// `DW0313`: one or more placed gravity blocks despawn into the void at placement.
/// A gravity floor (`sand`/`gravel`/…) laid unsupported over the delve's `the_void`
/// world falls out of the world on the first block update, silently deforming the
/// shipped map (holes, light leaks, visual damage) even where no critical path or
/// wave seat happens to cross it — so DW0311/DW0312 alone would let it ship green.
/// This is the authoritative, direct gate: no DSL verb can intend a despawn, so it
/// is always a prefab/generator defect (task #42, owner addendum).
pub const DW_GRAVITY_DESPAWN: &str = "DW0313";

/// A placed piece's prefab id paired with its world AABB `(min, max)`, for
/// attributing a despawned cell back to the piece that placed it.
type PieceBox<'a> = (&'a str, ([i32; 3], [i32; 3]));

/// Build the [`DW_GRAVITY_DESPAWN`] (`DW0313`) message if any placed gravity block
/// despawns into the void, attributing the despawned cells to the prefab piece
/// that placed them; `None` when the assembled world settles with zero losses.
///
/// A gravity block that merely *falls onto support* is NOT flagged here: the
/// settled model represents it faithfully, so every consumer (nav, light,
/// waypoints) already ships the true geometry — there is no correctness gap, and
/// the generator's own zero-unsupported invariant catches an *unintended* fall at
/// authoring time. Only a despawn — an irrecoverable hole — is a build error.
pub fn gravity_despawn_error(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Option<String> {
    let assembled = assemble(plan, structures);
    // (prefab_id, world AABB) for every placed piece, for despawn attribution.
    let pieces: Vec<PieceBox> = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .map(|p| (p.prefab_id.as_str(), p.bbox()))
        .collect();
    despawn_message(&assembled.settled, &pieces)
}

/// Pure core of [`gravity_despawn_error`]: the `DW0313` message for the despawned
/// members of `settled`, attributing each despawned cell to the prefab piece whose
/// AABB contains it; `None` when nothing despawns. Split out so the diagnostic's
/// attribution and #73-rubric wording are unit-testable without a full [`Plan`].
fn despawn_message(settled: &[Settled], pieces: &[PieceBox]) -> Option<String> {
    let despawned: Vec<&Settled> = settled.iter().filter(|s| s.to.is_none()).collect();
    if despawned.is_empty() {
        return None;
    }

    // Pieces never overlap (the solver rejects overlaps), so the first containing
    // AABB is the unique placer.
    let piece_of = |cell: [i32; 3]| -> &str {
        pieces
            .iter()
            .find(|(_, (lo, hi))| (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i]))
            .map(|(id, _)| *id)
            .unwrap_or("<unknown piece>")
    };

    // Group despawned cells + block kinds per piece (deterministic BTreeMap).
    type PerPiece<'a> = BTreeMap<&'a str, (Vec<[i32; 3]>, BTreeSet<String>)>;
    let mut per_piece: PerPiece = BTreeMap::new();
    for s in &despawned {
        let entry = per_piece.entry(piece_of(s.from)).or_default();
        entry.0.push(s.from);
        entry.1.insert(strip_ns(&s.block).to_string());
    }

    let total = despawned.len();
    let mut summaries = Vec::new();
    for (piece, (mut cells, kinds)) in per_piece {
        cells.sort_unstable();
        let sample: Vec<String> = cells.iter().take(4).map(|c| format!("{c:?}")).collect();
        let more = cells.len().saturating_sub(sample.len());
        let kinds: Vec<&str> = kinds.iter().map(String::as_str).collect();
        summaries.push(format!(
            "`{piece}` — {n} cell(s) of {kinds} at {sample}{extra}",
            n = cells.len(),
            kinds = kinds.join("/"),
            sample = sample.join(", "),
            extra = if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            },
        ));
    }

    Some(format!(
        "gravity settling: {total} placed gravity block(s) fall out of the world at placement and \
         despawn into the void, leaving holes in the assembled floor. The delve ships into a \
         `the_void` world, so a gravity block ({kinds_hint}) with no solid block directly beneath it \
         is unsupported and drops away on the first block update. Affected: {summary}. \
         WHERE to fix: the prefab / tileset generator that produced these piece(s), not the compiler. \
         HOW: give every gravity floor cell a non-falling SUPPORT block directly beneath it — a \
         substrate layer under the visible surface, exactly as `cave-shore`'s beach seabed is built \
         (every sand/gravel cell rests on solid rock). Do NOT swap the floor palette to non-falling \
         blocks (sandstone/tuff/stone) just to silence this: sand/gravel floors are a supported, \
         first-class content need — the fix is to add the substrate, never to remove the material.",
        kinds_hint = "sand/red_sand/gravel/concrete_powder/anvil/dragon_egg",
        summary = summaries.join("; "),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_falling_block_over_void_despawns() {
        // A single layer of sand at y=64 over void — the cave-den floor field bug:
        // every unsupported sand cell falls out of the world.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:sand".to_string());
        blocks.insert([1, 64, 0], "minecraft:sand".to_string());
        settle(&mut blocks);
        assert!(
            blocks.is_empty(),
            "unsupported sand must despawn: {blocks:?}"
        );
    }

    #[test]
    fn falling_block_rests_on_support_and_fills_a_gap() {
        // stone at y=64 (support), air at 65, sand at 66 → sand falls to 65.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:stone".to_string());
        blocks.insert([0, 66, 0], "minecraft:sand".to_string());
        settle(&mut blocks);
        assert_eq!(
            blocks.get(&[0, 64, 0]).map(String::as_str),
            Some("minecraft:stone")
        );
        assert_eq!(
            blocks.get(&[0, 65, 0]).map(String::as_str),
            Some("minecraft:sand")
        );
        assert!(
            !blocks.contains_key(&[0, 66, 0]),
            "sand should have fallen off 66"
        );
    }

    #[test]
    fn stacked_falling_blocks_settle_contiguously_on_support() {
        // stone(64), sand(66), sand(68) → both sand fall onto the stone: 65,66.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:stone".to_string());
        blocks.insert([0, 66, 0], "minecraft:sand".to_string());
        blocks.insert([0, 68, 0], "minecraft:gravel".to_string());
        settle(&mut blocks);
        assert!(blocks.contains_key(&[0, 64, 0]));
        assert!(blocks.contains_key(&[0, 65, 0]));
        assert!(blocks.contains_key(&[0, 66, 0]));
        assert!(!blocks.contains_key(&[0, 67, 0]));
        assert!(!blocks.contains_key(&[0, 68, 0]));
    }

    // --- water flood model (task #45) ---

    /// A flat solid floor at `y` over `[x0,x1] × [z0,z1]`.
    fn floor(y: i32, x0: i32, x1: i32, z0: i32, z1: i32) -> BTreeMap<[i32; 3], String> {
        let mut b = BTreeMap::new();
        for x in x0..=x1 {
            for z in z0..=z1 {
                b.insert([x, y, z], "minecraft:stone".to_string());
            }
        }
        b
    }

    #[test]
    fn flood_horizontal_decay_stops_at_seven() {
        // A source on a long flat floor spreads exactly 7 cells each way, no more —
        // the standard vanilla horizontal decay (level 1..=7).
        let mut b = floor(64, -10, 10, 0, 0);
        b.insert([0, 65, 0], "minecraft:water".to_string()); // source at x=0
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        for x in -7..=7 {
            assert!(flooded.contains(&[x, 65, 0]), "x={x} should flood");
        }
        assert!(!flooded.contains(&[8, 65, 0]), "x=8 is out of 7-range");
        assert!(!flooded.contains(&[-8, 65, 0]), "x=-8 is out of 7-range");
    }

    #[test]
    fn flood_falls_downward_infinitely_and_respreads() {
        // A source high over a deep floor falls the full drop through the open column
        // and, on landing, spreads again at full strength — downward is a level-0
        // (free) edge, so the respread reaches the full 7-range from the landing.
        let mut b = BTreeMap::new();
        for x in 0..=9 {
            b.insert([x, 60, 0], "minecraft:stone".to_string()); // deep floor
        }
        b.insert([0, 72, 0], "minecraft:water".to_string()); // source high above x=0
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        // Falls down the open column x=0 from 72 to just above the y=60 floor…
        for y in 61..=72 {
            assert!(flooded.contains(&[0, y, 0]), "column cell [0,{y},0] floods");
        }
        // …then respreads full-strength (7) along the deep floor.
        assert!(
            flooded.contains(&[7, 61, 0]),
            "respread reaches x=7 on landing"
        );
    }

    /// A 1-wide flat corridor at floor `y` walled on both z sides at `y+1`, so water
    /// is confined to the z=0 lane (the superset otherwise flows around a 1-cell
    /// obstacle over the open floorless sides).
    fn walled_corridor(y: i32, x0: i32, x1: i32) -> BTreeMap<[i32; 3], String> {
        let mut b = BTreeMap::new();
        for x in x0..=x1 {
            b.insert([x, y, 0], "minecraft:stone".to_string()); // floor
            b.insert([x, y + 1, 1], "minecraft:stone".to_string()); // +z wall
            b.insert([x, y + 1, -1], "minecraft:stone".to_string()); // -z wall
        }
        b
    }

    #[test]
    fn flood_is_dammed_by_solid_and_settled_sand() {
        // A full-height dam across a walled corridor stops the source: cells past the
        // dam stay dry. A settled sand block is a dam exactly like stone.
        let mut b = walled_corridor(64, 0, 10);
        b.insert([0, 65, 0], "minecraft:water".to_string()); // source in the lane
        b.insert([3, 65, 0], "minecraft:sand".to_string()); // supported → stays, dams the lane
        let occ = occupancy_of(b, &BTreeSet::new());
        let (solid, flooded) = (occ.solid, occ.flooded);
        assert!(
            solid.contains(&[3, 65, 0]),
            "the sand dam is solid, not flooded"
        );
        assert!(flooded.contains(&[2, 65, 0]), "water reaches up to the dam");
        assert!(
            !flooded.contains(&[3, 65, 0]),
            "the dam cell itself is solid, not water"
        );
        assert!(!flooded.contains(&[4, 65, 0]), "water is dammed past x=3");
    }

    #[test]
    fn settle_then_flood_order_a_fallen_sand_dam_still_dams() {
        // Prove the flood sees POST-settle geometry: a sand "dam" authored one block
        // too high settles DOWN into the lane and still dams. settle() runs first
        // (as `assemble` does), then occupancy_of floods the settled result.
        let mut b = walled_corridor(64, 0, 10);
        b.insert([0, 65, 0], "minecraft:water".to_string()); // source
        b.insert([3, 66, 0], "minecraft:sand".to_string()); // authored high; air at [3,65,0]
        settle(&mut b); // sand falls from 66 → 65 (onto the floor at 64)
        assert_eq!(
            b.get(&[3, 65, 0]).map(String::as_str),
            Some("minecraft:sand"),
            "sand settled down into the lane"
        );
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        assert!(
            flooded.contains(&[2, 65, 0]),
            "water reaches the settled dam"
        );
        assert!(!flooded.contains(&[4, 65, 0]), "settled sand dams the lane");
    }

    #[test]
    fn settle_then_flood_order_a_despawned_sand_opens_the_channel() {
        // The complementary case: a sand "dam" over a hole in the floor despawns into
        // the void (settle), so it is NOT a dam and the flood flows through. Under-
        // settling here would have wrongly kept a phantom dam and marked cells dry.
        let mut b = walled_corridor(64, 0, 10);
        b.remove(&[5, 64, 0]); // a hole in the floor at x=5
        b.insert([0, 65, 0], "minecraft:water".to_string()); // source
        b.insert([5, 66, 0], "minecraft:sand".to_string()); // over the hole → despawns
        settle(&mut b);
        assert!(!b.contains_key(&[5, 66, 0]), "sand over the hole despawned");
        assert!(
            !b.contains_key(&[5, 65, 0]),
            "…and did not settle onto a missing floor"
        );
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        // No dam remains, so the flow passes x=5 (and falls into the hole column).
        assert!(
            flooded.contains(&[6, 65, 0]),
            "flow passes the (despawned) non-dam"
        );
    }

    #[test]
    fn infinite_water_fills_a_pool_basin_past_the_seven_range() {
        // The perimedes field case in miniature: a long walled basin whose source
        // rows have a GAP in the middle. Vanilla infinite water fills the gap (a
        // flowing cell flanked by ≥2 sources becomes a source), so the pool fills its
        // whole basin — reaching a cell a plain 7-decay flood would leave dry.
        let mut b = BTreeMap::new();
        // A walled trough at y=65 (floor y=64), 1 wide (z=0), x 0..=20.
        for x in 0..=20 {
            b.insert([x, 64, 0], "minecraft:stone".to_string());
            b.insert([x, 65, 1], "minecraft:stone".to_string()); // +z wall
            b.insert([x, 65, -1], "minecraft:stone".to_string()); // -z wall
        }
        b.insert([0, 66, 0], "minecraft:stone".to_string()); // end caps at y=66 too
        b.insert([20, 66, 0], "minecraft:stone".to_string());
        for x in 0..=20 {
            b.insert([x, 66, 1], "minecraft:stone".to_string());
            b.insert([x, 66, -1], "minecraft:stone".to_string());
        }
        // Source rows at both ends, but a GAP in the middle (x 6..=14 has no seed).
        for x in [0, 1, 2, 3, 4, 5, 15, 16, 17, 18, 19, 20] {
            b.insert([x, 65, 0], "minecraft:water".to_string());
        }
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        // The gap fills completely — every basin cell is wet, well past 7 from any
        // original seed (x=10 is 5 past the x=5 seed's 7-range… reachable only via
        // infinite-water source formation cascading across the gap).
        for x in 0..=20 {
            assert!(
                flooded.contains(&[x, 65, 0]),
                "basin cell x={x} must be flooded"
            );
        }
    }

    #[test]
    fn single_puddle_does_not_infinitely_fill() {
        // A lone source on an open flat floor does NOT form new sources (no ≥2-source
        // cell), so it makes a bounded 7-cell puddle, not a whole-floor flood — the
        // infinite-water rule stays tight, not runaway.
        let mut b = floor(64, -20, 20, 0, 0);
        b.insert([0, 65, 0], "minecraft:water".to_string());
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        assert!(flooded.contains(&[7, 65, 0]), "reaches 7");
        assert!(
            !flooded.contains(&[8, 65, 0]),
            "a single source does not fill the floor"
        );
    }

    #[test]
    fn flood_superset_marks_unsupported_horizontal_spread() {
        // The superset omits vanilla's "only spread level on a supported floor": a
        // source spreads horizontally over an open ledge (no floor under the flow),
        // which vanilla would instead drop straight down. Over-marking is safe.
        let mut b = BTreeMap::new();
        b.insert([0, 64, 0], "minecraft:stone".to_string()); // only the source has a floor
        b.insert([0, 65, 0], "minecraft:water".to_string()); // source
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        // No floor under [1,65,0], yet the superset still marks it wet (vanilla would
        // let it fall). This is the intentional over-approximation.
        assert!(
            flooded.contains(&[1, 65, 0]),
            "superset marks unsupported horizontal spread wet"
        );
    }

    #[test]
    fn shore_pool_floods_the_approach_tongue_but_not_a_walled_dry_alcove() {
        // The `cave-shore`/perimedes field case (task #45), reduced: a sand shore at
        // y=65 with a water source pooled on it floods the open shore cells (the flow
        // TONGUE a talk-to leg must never step through), while a cell fully walled off
        // from the water on every horizontal side stays a DRY, standable alcove.
        let mut b = BTreeMap::new();
        // Sand shore floor at y=65 over x 0..=6, z=0 (standing level y=66).
        for x in 0..=6 {
            b.insert([x, 65, 0], "minecraft:sand".to_string());
        }
        // Water source pooled on the shore at x=0.
        b.insert([0, 66, 0], "minecraft:water".to_string());
        // A dry alcove at x=6: wall it off from the flow with stone at x=5 (full
        // height at the standing + head level), so water cannot reach it.
        b.insert([5, 66, 0], "minecraft:stone".to_string());
        b.insert([5, 67, 0], "minecraft:stone".to_string());
        let occ = occupancy_of(b, &BTreeSet::new());
        let (solid, flooded) = (occ.solid, occ.flooded);
        // The open shore between source and wall is the flooded tongue (7-range).
        assert!(
            flooded.contains(&[1, 66, 0]),
            "approach tongue must be marked flooded"
        );
        assert!(
            flooded.contains(&[4, 66, 0]),
            "tongue reaches up to the wall"
        );
        // The walled alcove is dry and standable (sand floor, clear head).
        assert!(
            !flooded.contains(&[6, 66, 0]),
            "the walled dry alcove must NOT be flooded"
        );
        assert!(
            solid.contains(&[6, 65, 0]),
            "the alcove has a solid floor below it"
        );
        assert!(
            !flooded.contains(&[6, 67, 0]),
            "the alcove head cell is clear"
        );
    }

    #[test]
    fn no_water_leaves_flooded_empty_and_solid_unchanged() {
        // Determinism / no-op guarantee: a waterless world has an empty flood set and
        // every block stays solid.
        let b = floor(64, 0, 3, 0, 3);
        let occ = occupancy_of(b.clone(), &BTreeSet::new());
        let (solid, flooded) = (occ.solid, occ.flooded);
        assert!(flooded.is_empty());
        assert_eq!(solid, b.into_keys().collect());
    }

    #[test]
    fn non_falling_blocks_float_and_are_untouched() {
        // Stone floats in vanilla; a floating andesite floor over void stays put —
        // this is why the cave-den andesite/coarse_dirt cells survived while the
        // sand cells fell.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:andesite".to_string());
        blocks.insert([1, 64, 0], "minecraft:coarse_dirt".to_string());
        let before = blocks.clone();
        settle(&mut blocks);
        assert_eq!(blocks, before);
    }

    #[test]
    fn cave_den_field_bug_mixed_floor_keeps_only_the_non_falling_cells() {
        // The exact task #42 field shape: a single-layer cave floor over void,
        // part `sand` (falling) and part `andesite` (fixed). In game every sand
        // cell falls out of the void and leaves a hole; every andesite cell
        // stays. The model must reproduce this — seating a wave or routing a path
        // over the sand region has no footing.
        let mut blocks = BTreeMap::new();
        for x in 0..5 {
            let name = if x < 3 {
                "minecraft:sand"
            } else {
                "minecraft:andesite"
            };
            blocks.insert([x, 64, 0], name.to_string());
        }
        settle(&mut blocks);
        for x in 0..3 {
            assert!(
                !blocks.contains_key(&[x, 64, 0]),
                "sand floor cell {x} must fall into the void"
            );
        }
        for x in 3..5 {
            assert_eq!(
                blocks.get(&[x, 64, 0]).map(String::as_str),
                Some("minecraft:andesite"),
                "andesite floor cell {x} must remain"
            );
        }
    }

    // --- collision classes (task #59) ---

    #[test]
    fn fences_walls_and_gates_classify_off_the_solid_set() {
        // A stone floor with a fence, a wall, a closed gate, and an open gate on it.
        let mut b = floor(64, 0, 4, 0, 0);
        b.insert([0, 65, 0], "minecraft:oak_fence".to_string());
        b.insert([1, 65, 0], "minecraft:cobblestone_wall".to_string());
        b.insert([2, 65, 0], "minecraft:oak_fence_gate".to_string()); // closed
        b.insert([3, 65, 0], "minecraft:oak_fence_gate".to_string()); // open (below)
        b.insert([4, 65, 0], "minecraft:nether_brick_fence".to_string());
        let open: BTreeSet<[i32; 3]> = [[3, 65, 0]].into_iter().collect();
        let occ = occupancy_of(b, &open);
        assert!(
            occ.tall.contains(&[0, 65, 0]),
            "oak_fence is a tall barrier"
        );
        assert!(
            occ.tall.contains(&[1, 65, 0]),
            "cobblestone_wall is a tall barrier"
        );
        assert!(
            occ.tall.contains(&[4, 65, 0]),
            "nether_brick_fence is a tall barrier"
        );
        assert!(
            occ.use_gates.contains(&[2, 65, 0]),
            "a closed fence gate is a use-gate cell"
        );
        assert!(
            !occ.use_gates.contains(&[3, 65, 0]) && !occ.tall.contains(&[3, 65, 0]),
            "an open fence gate is a passable threshold"
        );
        for c in [[0, 65, 0], [1, 65, 0], [2, 65, 0], [3, 65, 0], [4, 65, 0]] {
            assert!(!occ.solid.contains(&c), "{c:?} must not be full-cube solid");
        }
        // Conservative default: a slab / stairs / carpet cell stays full solid.
        let mut c = floor(64, 0, 0, 0, 0);
        c.insert([0, 65, 0], "minecraft:oak_slab".to_string());
        let occ = occupancy_of(c, &BTreeSet::new());
        assert!(
            occ.solid.contains(&[0, 65, 0]),
            "slab is conservative solid"
        );
    }

    #[test]
    fn fences_and_gates_still_dam_the_water_flood() {
        // Vanilla water flows only into air: a fence (and a gate, open or closed)
        // dams the flow exactly like the full-solid model did — the flood must not
        // sneak "through" a now-walker-passable gate cell.
        let mut b = walled_corridor(64, 0, 10);
        b.insert([0, 65, 0], "minecraft:water".to_string()); // source in the lane
        b.insert([3, 65, 0], "minecraft:oak_fence_gate".to_string()); // closed gate dam
        let occ = occupancy_of(b, &BTreeSet::new());
        assert!(occ.flooded.contains(&[2, 65, 0]), "water reaches the gate");
        assert!(!occ.flooded.contains(&[3, 65, 0]), "the gate cell dams");
        assert!(
            !occ.flooded.contains(&[4, 65, 0]),
            "water is dammed past x=3"
        );
        assert!(
            occ.use_gates.contains(&[3, 65, 0]),
            "the dam is still a use-gate for the walker model"
        );
    }

    #[test]
    fn decoder_reads_the_open_gate_block_state() {
        // A minimal 2-block structure NBT: a closed and an open oak_fence_gate.
        // The decoder must surface `open` so occupancy can split them.
        use fastnbt::Value;
        let palette = Value::List(vec![
            Value::Compound(
                [
                    (
                        "Name".to_string(),
                        Value::String("minecraft:oak_fence_gate".to_string()),
                    ),
                    (
                        "Properties".to_string(),
                        Value::Compound(
                            [
                                ("open".to_string(), Value::String("false".to_string())),
                                ("facing".to_string(), Value::String("north".to_string())),
                            ]
                            .into_iter()
                            .collect(),
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            Value::Compound(
                [
                    (
                        "Name".to_string(),
                        Value::String("minecraft:oak_fence_gate".to_string()),
                    ),
                    (
                        "Properties".to_string(),
                        Value::Compound(
                            [("open".to_string(), Value::String("true".to_string()))]
                                .into_iter()
                                .collect(),
                        ),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            Value::Compound(
                [(
                    "Name".to_string(),
                    Value::String("minecraft:stone".to_string()),
                )]
                .into_iter()
                .collect(),
            ),
        ]);
        let block = |pos: [i32; 3], state: i32| {
            Value::Compound(
                [
                    (
                        "pos".to_string(),
                        Value::List(pos.iter().map(|&v| Value::Int(v)).collect()),
                    ),
                    ("state".to_string(), Value::Int(state)),
                ]
                .into_iter()
                .collect(),
            )
        };
        let root = Value::Compound(
            [
                ("palette".to_string(), palette),
                (
                    "blocks".to_string(),
                    Value::List(vec![
                        block([0, 0, 0], 0),
                        block([1, 0, 0], 1),
                        block([2, 0, 0], 2),
                    ]),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let raw = fastnbt::to_bytes(&root).expect("serialize NBT");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &raw).expect("gzip");
        let bytes = gz.finish().expect("gzip finish");
        let cells = structure_cells(&bytes);
        assert_eq!(
            cells,
            vec![
                (
                    [0, 0, 0],
                    "minecraft:oak_fence_gate".to_string(),
                    Some(false)
                ),
                (
                    [1, 0, 0],
                    "minecraft:oak_fence_gate".to_string(),
                    Some(true)
                ),
                ([2, 0, 0], "minecraft:stone".to_string(), None),
            ]
        );
    }

    #[test]
    fn dripstone_is_not_settled_as_a_falling_floor() {
        // pointed_dripstone hangs from the ceiling (attaches upward); the settle
        // rule must not treat it as an unsupported floor block and delete it.
        assert!(!is_falling_block("minecraft:pointed_dripstone"));
    }

    #[test]
    fn settle_reports_despawns_and_landings_distinctly() {
        // Column A (x=0): sand over void -> despawns (to == None).
        // Column B (x=1): stone support, air gap, sand above -> lands on support.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:sand".to_string());
        blocks.insert([1, 64, 0], "minecraft:stone".to_string());
        blocks.insert([1, 66, 0], "minecraft:sand".to_string());
        let outcomes = settle(&mut blocks);
        let despawned: Vec<_> = outcomes.iter().filter(|s| s.to.is_none()).collect();
        let landed: Vec<_> = outcomes.iter().filter(|s| s.to.is_some()).collect();
        assert_eq!(despawned.len(), 1, "one sand cell despawns: {outcomes:?}");
        assert_eq!(despawned[0].from, [0, 64, 0]);
        assert_eq!(landed.len(), 1, "one sand cell lands on support");
        assert_eq!(landed[0].from, [1, 66, 0]);
        assert_eq!(landed[0].to, Some([1, 65, 0]));
    }

    #[test]
    fn despawn_message_fires_with_attribution_and_anti_dodge_wording() {
        // An unsupported sand floor in a synthetic piece "prefab/test-den" whose
        // AABB covers x0..4,y64,z0. Every sand cell despawns.
        let mut blocks = BTreeMap::new();
        for x in 0..5 {
            blocks.insert([x, 64, 0], "minecraft:sand".to_string());
        }
        let settled = settle(&mut blocks);
        let pieces = [("prefab/test-den", ([0, 64, 0], [4, 64, 0]))];
        let msg = despawn_message(&settled, &pieces).expect("despawn must be flagged");
        // WHAT: count + kind; WHERE: the piece; HOW + anti-dodge clause (#73 rubric).
        assert!(msg.contains("despawn"), "names the failure: {msg}");
        assert!(
            msg.contains("prefab/test-den"),
            "attributes the piece: {msg}"
        );
        assert!(msg.contains("sand"), "names the block kind: {msg}");
        assert!(msg.contains("substrate"), "prescribes the fix: {msg}");
        assert!(
            msg.contains("Do NOT swap the floor palette"),
            "carries the anti-dodge clause: {msg}"
        );
        // A supported floor produces no message.
        let mut ok = BTreeMap::new();
        for x in 0..5 {
            ok.insert([x, 63, 0], "minecraft:stone".to_string());
            ok.insert([x, 64, 0], "minecraft:sand".to_string());
        }
        let ok_settled = settle(&mut ok);
        assert!(
            despawn_message(&ok_settled, &pieces).is_none(),
            "supported floor is clean"
        );
    }

    // --- waterlogging is water (task #78) ---

    #[test]
    fn a_waterlogged_block_seeds_the_flood_like_a_free_source() {
        // MC 1.13+: a waterlogged block's cell holds a real water source that
        // spreads into adjacent air. The pre-#78 model stored bare ids and
        // asserted (wrongly) that waterlogging "never spreads to a neighbour", so
        // it under-marked the flood — the one direction the never-under-mark
        // contract forbids.
        let mut b = floor(64, 0, 6, 0, 0);
        b.insert(
            [0, 65, 0],
            "minecraft:oak_fence[waterlogged=true]".to_string(),
        );
        let occ = occupancy_of(b, &BTreeSet::new());
        assert!(
            occ.tall.contains(&[0, 65, 0]),
            "the host block keeps its own collision class"
        );
        assert!(
            !occ.flooded.contains(&[0, 65, 0]),
            "the sets stay disjoint: an occupied cell is not open water"
        );
        for x in 1..=6 {
            assert!(
                occ.flooded.contains(&[x, 65, 0]),
                "water must flow out of the waterlogged cell to x={x}"
            );
        }
    }

    #[test]
    fn a_dry_block_of_the_same_kind_floods_nothing() {
        // The control: `waterlogged=false` (what every current prefab authors) is
        // bone dry, so this fix cannot silently wet an existing build.
        let mut b = floor(64, 0, 6, 0, 0);
        b.insert(
            [0, 65, 0],
            "minecraft:oak_fence[waterlogged=false]".to_string(),
        );
        let occ = occupancy_of(b, &BTreeSet::new());
        assert!(occ.flooded.is_empty(), "a dry fence floods nothing");
    }

    // --- gravity blocks sink through fluids (task #78) ---

    #[test]
    fn a_falling_block_sinks_through_water_and_displaces_it() {
        // Vanilla `FallingBlock.isFree` counts a fluid as free space: sand dropped
        // over a pool falls THROUGH the water and lands on the rock beneath,
        // replacing the water cell it rests in. The pre-#78 model treated the
        // water cell as an immovable support and floated the sand on the surface —
        // a phantom floor that dammed the flood and that nav then walked on.
        let mut b = BTreeMap::new();
        b.insert([0, 63, 0], "minecraft:stone".to_string()); // pool floor
        b.insert([0, 64, 0], "minecraft:water".to_string()); // 2-deep water
        b.insert([0, 65, 0], "minecraft:water".to_string());
        b.insert([0, 67, 0], "minecraft:sand".to_string()); // dropped in
        let outcomes = settle(&mut b);
        assert_eq!(
            b.get(&[0, 64, 0]).map(String::as_str),
            Some("minecraft:sand"),
            "the sand rests on the rock, displacing the water it landed in"
        );
        assert_eq!(
            b.get(&[0, 65, 0]).map(String::as_str),
            Some("minecraft:water"),
            "the water above the landing is untouched"
        );
        assert!(
            !b.contains_key(&[0, 67, 0]),
            "the sand left its authored cell"
        );
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].to, Some([0, 64, 0]));
    }

    #[test]
    fn a_falling_block_over_water_with_no_floor_still_despawns() {
        // Water cannot rescue an unsupported gravity block: with nothing solid
        // anywhere below, it sinks through and falls out of the void world. The
        // old model "caught" it on the water and hid the hole (DW0313).
        let mut b = BTreeMap::new();
        b.insert([0, 64, 0], "minecraft:water".to_string());
        b.insert([0, 66, 0], "minecraft:sand".to_string());
        let outcomes = settle(&mut b);
        assert!(!b.contains_key(&[0, 66, 0]));
        assert!(!b.contains_key(&[0, 65, 0]));
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].to, None, "no fluid is a support: {outcomes:?}");
    }

    // --- partial collision heights (task #78) ---

    #[test]
    fn collision_heights_match_the_vanilla_shapes() {
        // Slabs: `type` defaults to `bottom`, so a bare id is a half-step.
        assert_eq!(collision_top_16("minecraft:oak_slab"), 8);
        assert_eq!(collision_top_16("minecraft:oak_slab[type=bottom]"), 8);
        assert_eq!(collision_top_16("minecraft:oak_slab[type=top]"), 16);
        assert_eq!(collision_top_16("minecraft:oak_slab[type=double]"), 16);
        // Snow: collision is (layers-1)*2/16 — one layer has no collision box.
        assert_eq!(collision_top_16("minecraft:snow[layers=1]"), 0);
        assert_eq!(collision_top_16("minecraft:snow"), 0); // layers defaults to 1
        assert_eq!(collision_top_16("minecraft:snow[layers=5]"), 8);
        assert_eq!(collision_top_16("minecraft:snow[layers=8]"), 14);
        // Carpets and paths.
        assert_eq!(collision_top_16("minecraft:red_carpet"), 1);
        assert_eq!(collision_top_16("minecraft:moss_carpet"), 1);
        assert_eq!(collision_top_16("minecraft:pale_moss_carpet"), 0);
        assert_eq!(
            collision_top_16("minecraft:pale_moss_carpet[bottom=true]"),
            1
        );
        assert_eq!(collision_top_16("minecraft:dirt_path"), 15);
        assert_eq!(collision_top_16("minecraft:farmland"), 15);
        // Everything else stays a conservative full cube.
        assert_eq!(collision_top_16("minecraft:stone"), 16);
        assert_eq!(collision_top_16("minecraft:oak_stairs[facing=north]"), 16);
    }

    #[test]
    fn partial_floors_are_solid_and_thin_decoration_is_passable() {
        let mut b = floor(63, 0, 3, 0, 0);
        b.insert([0, 64, 0], "minecraft:oak_slab[type=bottom]".to_string());
        b.insert([1, 64, 0], "minecraft:oak_slab[type=top]".to_string());
        b.insert([2, 64, 0], "minecraft:red_carpet".to_string());
        b.insert([3, 64, 0], "minecraft:snow[layers=1]".to_string());
        let occ = occupancy_of(b, &BTreeSet::new());
        assert_eq!(
            occ.partial.get(&[0, 64, 0]),
            Some(&8),
            "bottom slab is 8/16"
        );
        assert!(
            occ.solid.contains(&[1, 64, 0]) && !occ.partial.contains_key(&[1, 64, 0]),
            "a top slab is a full-height floor"
        );
        for c in [[2, 64, 0], [3, 64, 0]] {
            assert!(
                !occ.solid.contains(&c),
                "{c:?} is sub-auto-step decoration, walked over rather than onto"
            );
        }
    }

    #[test]
    fn substrate_under_gravity_floor_settles_with_zero_despawns() {
        // The generator's fix shape: a non-falling substrate (stone) beneath every
        // gravity surface cell -> nothing despawns, and the surface is preserved.
        let mut blocks = BTreeMap::new();
        for x in 0..5 {
            blocks.insert([x, 63, 0], "minecraft:stone".to_string()); // substrate
            blocks.insert([x, 64, 0], "minecraft:sand".to_string()); // surface
        }
        let outcomes = settle(&mut blocks);
        assert!(
            outcomes.iter().all(|s| s.to == Some(s.from)),
            "every supported gravity cell stays put: {outcomes:?}"
        );
        for x in 0..5 {
            assert_eq!(
                blocks.get(&[x, 64, 0]).map(String::as_str),
                Some("minecraft:sand")
            );
            assert_eq!(
                blocks.get(&[x, 63, 0]).map(String::as_str),
                Some("minecraft:stone")
            );
        }
    }
}

#[cfg(test)]
mod rotate_state_tests {
    use super::rotate_state;
    use crate::solver::Rotation;

    #[test]
    fn identity_rotation_is_untouched() {
        let s = "minecraft:stone_stairs[facing=north,half=bottom]";
        assert_eq!(rotate_state(s, Rotation::None), s);
    }

    #[test]
    fn stateless_names_pass_through() {
        assert_eq!(
            rotate_state("minecraft:stone", Rotation::Cw90),
            "minecraft:stone"
        );
    }

    #[test]
    fn facing_turns_with_the_piece() {
        let s = "minecraft:stone_stairs[facing=north,half=bottom,shape=straight]";
        assert_eq!(
            rotate_state(s, Rotation::Cw90),
            "minecraft:stone_stairs[facing=east,half=bottom,shape=straight]"
        );
        assert_eq!(
            rotate_state(s, Rotation::Cw180),
            "minecraft:stone_stairs[facing=south,half=bottom,shape=straight]"
        );
        assert_eq!(
            rotate_state(s, Rotation::Ccw90),
            "minecraft:stone_stairs[facing=west,half=bottom,shape=straight]"
        );
    }

    /// A stair's `shape` is expressed relative to its own `facing`, so it must
    /// NOT be rotated — only rail shapes are absolute.
    #[test]
    fn stair_shape_is_relative_and_never_rotated() {
        assert_eq!(
            rotate_state(
                "minecraft:stone_stairs[facing=north,shape=inner_left]",
                Rotation::Cw90
            ),
            "minecraft:stone_stairs[facing=east,shape=inner_left]"
        );
    }

    #[test]
    fn rail_shapes_are_absolute_and_do_rotate() {
        assert_eq!(
            rotate_state("minecraft:rail[shape=ascending_north]", Rotation::Cw90),
            "minecraft:rail[shape=ascending_east]"
        );
        assert_eq!(
            rotate_state("minecraft:rail[shape=north_east]", Rotation::Cw90),
            "minecraft:rail[shape=east_south]"
        );
    }

    #[test]
    fn axis_swaps_on_a_quarter_turn_only() {
        assert_eq!(
            rotate_state("minecraft:oak_log[axis=x]", Rotation::Cw90),
            "minecraft:oak_log[axis=z]"
        );
        assert_eq!(
            rotate_state("minecraft:oak_log[axis=x]", Rotation::Cw180),
            "minecraft:oak_log[axis=x]"
        );
        assert_eq!(
            rotate_state("minecraft:oak_log[axis=y]", Rotation::Cw90),
            "minecraft:oak_log[axis=y]"
        );
    }

    #[test]
    fn sign_rotation_dial_advances_four_per_quarter_turn() {
        assert_eq!(
            rotate_state("minecraft:oak_sign[rotation=0]", Rotation::Cw90),
            "minecraft:oak_sign[rotation=4]"
        );
        assert_eq!(
            rotate_state("minecraft:oak_sign[rotation=14]", Rotation::Cw90),
            "minecraft:oak_sign[rotation=2]"
        );
    }

    /// Connection properties permute as a SET, simultaneously — reading the
    /// original state, never a half-rewritten one.
    #[test]
    fn fence_connections_permute_simultaneously() {
        let s = "minecraft:oak_fence[east=false,north=true,south=false,west=false]";
        // Cw90 sends north -> east, so the `true` must land on `east`.
        assert_eq!(
            rotate_state(s, Rotation::Cw90),
            "minecraft:oak_fence[east=true,north=false,south=false,west=false]"
        );
    }

    #[test]
    fn wall_heights_travel_with_their_side() {
        let s = "minecraft:stone_brick_wall[east=none,north=tall,south=low,west=none]";
        assert_eq!(
            rotate_state(s, Rotation::Cw90),
            "minecraft:stone_brick_wall[east=tall,north=none,south=none,west=low]"
        );
    }

    #[test]
    fn vertical_facings_are_yaw_invariant() {
        assert_eq!(
            rotate_state("minecraft:hopper[facing=down]", Rotation::Cw90),
            "minecraft:hopper[facing=down]"
        );
    }

    /// Rotation-invariant properties the occupancy classifiers actually read are
    /// untouched — this is why the correction keeps every output byte-identical.
    #[test]
    fn occupancy_relevant_properties_are_untouched() {
        for s in [
            "minecraft:oak_slab[type=top,waterlogged=false]",
            "minecraft:snow[layers=5]",
            "minecraft:oak_fence_gate[facing=north,open=true]",
        ] {
            let r = rotate_state(s, Rotation::Cw90);
            for key in ["type", "waterlogged", "layers", "open"] {
                assert_eq!(
                    super::state_value(s, key),
                    super::state_value(&r, key),
                    "{key} changed in {r}"
                );
            }
        }
    }
}
