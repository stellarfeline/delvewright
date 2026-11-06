//! The shared assembled-world block model.
//!
//! One authoritative cell→block map of the world the shipped delve actually
//! assembles, built the way vanilla does: place each prefab structure with
//! `/place template <pos> <rotation>`, apply the solver's socket seals (air fill
//! opens a mated socket; wall material seals an unused one), clear gate
//! thresholds — **and then settle gravity-affected blocks**, because the delve
//! ships into a void flat world (validation/compose.yaml) with no natural
//! floor, so an unsupported `sand`/`gravel`/… block placed by `/place template`
//! immediately falls out of the world and leaves air.
//!
//! Both downstream models derive from this one map (CLAUDE.md "no hacks at any
//! layer"; the fix belongs in the shared model, not at each consumer):
//!
//! - [`crate::compiler::nav::World`] — collision/standability for `move-npc`, cutscene,
//!   the DW0311 critical-path walk, DW0312 wave seating, and the waypoint export.
//! - [`crate::compiler::light::LightModel`] — opacity/emission for the spec-0010 relight.
//!
//! ## Why settling is required for fidelity
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

use crate::compiler::blockstate::{BlockMap, BlockState};
use crate::compiler::cellset::{CellMap, CellSet};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use crate::compiler::plan::{Plan, ResolvedAnchor};
use delvewright_dsl::blockshape::{Collision, bare_id, collision_class};
use delvewright_dsl::{DwCode, ExitTier};

/// **What a block state does to a body is not decided here** (spec-0056).
///
/// It is decided once, in [`delvewright_dsl::blockshape`], because three crates
/// need the answer and only one of them could reach this module: the grammar back
/// end's contract gates read *air or a skull* and called a torch a wall, and the
/// admission pipeline's light probe kept a nine-id list that called open water
/// walkable. The table below is re-exported rather than re-implemented, so there
/// is no second definition for a private copy to drift from — a `pub use` is
/// checked by the compiler, which is a stronger tie than any test could be.
///
/// Read that module for the collision table, the class rule and the direction of
/// its errors.
pub use delvewright_dsl::blockshape::{
    FULL_HEIGHT_16, THIN_HEIGHT_16, base_id, collision_top_16, is_air, is_fence_gate, is_fluid,
    is_no_collision_plant, is_partial_floor, is_passable_trap_trigger, is_tall_barrier,
    is_thin_decoration, opens_by_hand, state_value,
};

/// Rewrite a blockstate's **orientation properties** for a piece placed with
/// rotation `r`, the way vanilla's `/place template … <rotation>` does.
///
/// Model fidelity. [`placed_blocks`] rotates a prefab cell's
/// *position* via [`crate::compiler::solver::Rotation::transform`] but used to insert the
/// palette name verbatim, so for any piece with `rotation != None` the assembled
/// map disagreed with the world the server actually builds: vanilla rotates
/// blockstates during structure placement, the model did not. Nothing consumed
/// the affected properties before, which is why it never surfaced — the
/// occupancy classifiers read only `type`/`layers`/`open`/`waterlogged`/`bottom`,
/// every one of them rotation-invariant, so this correction leaves nav, seating,
/// relight, frames and every emitted command byte-identical (ADR-0006). The
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
pub fn rotate_state(name: &str, r: crate::compiler::solver::Rotation) -> String {
    use crate::compiler::solver::Rotation;
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
        r: crate::compiler::solver::Rotation,
        quarter_turns: u32,
    ) -> Option<String> {
        use crate::compiler::solver::{Facing, Rotation};
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

/// Whether a block carries `waterlogged=true` — **the cell contains a water
/// source** alongside the host block (MC 1.13+ waterlogging).
///
/// Fidelity fix. The model previously stored bare block ids and its
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

/// Whether a block **can** hold water — its pinned registry entry carries the
/// `waterlogged` property — regardless of what the authored state says.
///
/// The distinction from [`is_waterlogged`] is the whole of it. That one asks what
/// the prefab's bytes SAY; this one asks what vanilla will DO to those bytes when
/// they land in water. `/place template` carries the fluid already in the cell
/// onto the block it writes there: a waterloggable block placed into an ocean
/// column comes out of the placement `waterlogged=true` whatever the template
/// declared, because `waterlogged=false` is a state the block can hold water in
/// and vanilla fills it from the world rather than from the template.
///
/// Measured, not reasoned: the gallery's own `ocean-horizon` build, booted on the
/// pinned 1.21.11 image, comes out with 10 waterlogged blocks — 8 iron bars and 2
/// chests, all of them authored with no `waterlogged` key at all — and 367 water
/// cells across its walk plane that the model called air.
///
/// The registry is the authority rather than a list of block ids here for the
/// reason every other block question is: a list is a copy of the registry that
/// stops being one.
pub fn is_waterloggable(name: &str) -> bool {
    delvewright_dsl::blocks::BlockRegistry::v1_21_11()
        .properties(bare_id(name))
        .is_some_and(|props| props.contains_key("waterlogged"))
}

/// Whether a block falls under gravity when the cell below cannot support it
/// (vanilla `FallingBlock`). In the delve's void world such a block, placed
/// unsupported by `/place template`, drops out of the world and leaves air — so
/// the assembled-world model must not treat it as permanent floor.
///
/// Deliberately excludes `pointed_dripstone` and `scaffolding`: those attach
/// upward / by support-distance rather than resting on the block directly below,
/// so the "supported from below" settling rule here does not model them — a cave
/// generator hangs dripstone *stalactites* from the ceiling, and this rule must
/// never mistake a ceiling-hung block for an unsupported one and delete it.
///
/// Gravity is a world fact, not a block-shape fact, so it stays here rather than
/// travelling to [`delvewright_dsl::blockshape`] with the collision table.
pub fn is_falling_block(name: &str) -> bool {
    let id = bare_id(name);
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

/// The extent `[x, y, z]` a structure template declares in its own `size` tag,
/// or `None` when the bytes do not decode as one.
///
/// The template's own claim about how big it is, as distinct from the metadata's
/// claim about the same thing. Nothing else can tell the two apart, which is
/// what makes a stale `.nbt` beside a fresh manifest undetectable without it.
pub fn structure_size(bytes: &[u8]) -> Option<[i32; 3]> {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut raw)
        .ok()?;
    if let Some(root) = crate::compiler::nbtread::root(&raw) {
        return crate::compiler::nbtread::size_of(root.size.as_ref());
    }
    let fastnbt::Value::Compound(root) = fastnbt::from_bytes::<fastnbt::Value>(&raw).ok()? else {
        return None;
    };
    crate::compiler::nbtread::size_of(root.get("size"))
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
/// blockstate-preserving [`structure_cells_stateful`] instead.
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

fn structure_cells_inner(bytes: &[u8], stateful: bool) -> Vec<TemplateCell> {
    let decoded = decode_template(bytes, stateful);
    decoded
        .cells
        .iter()
        .map(|&(pos, i)| {
            let (name, open) = decoded.palette[i].as_ref().expect("a kept entry");
            (pos, name.clone(), *open)
        })
        .collect()
}

/// [`structure_cells_stateful`] with each block state interned once per
/// palette entry rather than copied once per cell — the form the assembled
/// world stores.
pub(crate) fn structure_cells_interned(bytes: &[u8]) -> Vec<([i32; 3], BlockState, Option<bool>)> {
    interned(decode_template(bytes, true))
}

/// [`structure_cells_interned`] **with the template's air kept**: every cell
/// the game places when `/place template` writes this structure, which is every
/// cell but a `structure_void` and a cell the template omits (spec-0098 §7).
/// A template's air is placed, so it carves whatever stood there.
pub(crate) fn structure_cells_placed(bytes: &[u8]) -> Vec<([i32; 3], BlockState, Option<bool>)> {
    let mut decoded = decode_template(bytes, true);
    if let Some(raw) = gunzip(bytes) {
        decoded.cells = placed_cells(&raw).unwrap_or(decoded.cells);
    }
    interned(decoded)
}

fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut raw)
        .ok()?;
    Some(raw)
}

/// Every `(position, palette index)` a template's typed decoding names, air
/// included; `None` when the bytes do not have the typed shape.
fn placed_cells(raw: &[u8]) -> Option<Vec<([i32; 3], usize)>> {
    let root = crate::compiler::nbtread::root(raw)?;
    let mut out = Vec::new();
    for b in root.blocks.iter().flatten() {
        let (Some(pos), Some(state)) = (b.pos3(), &b.state) else {
            continue;
        };
        out.push((pos, state.0 as usize));
    }
    Some(out)
}

fn interned(decoded: Decoded) -> Vec<([i32; 3], BlockState, Option<bool>)> {
    let states: Vec<Option<(BlockState, Option<bool>)>> = decoded
        .palette
        .iter()
        .map(|e| {
            e.as_ref()
                .map(|(name, open)| (BlockState::new(name), *open))
        })
        .collect();
    decoded
        .cells
        .iter()
        .filter_map(|&(pos, i)| {
            let (state, open) = states.get(i).copied().flatten()?;
            Some((pos, state, open))
        })
        .collect()
}

/// One placed cell as the structure readers return it: local position, block
/// id, and the `open` property.
type TemplateCell = ([i32; 3], String, Option<bool>);

/// A decoded template: its palette as the readers render it, and every
/// non-air block as `(local position, palette index)` in file order.
struct Decoded {
    palette: Vec<Option<(String, Option<bool>)>>,
    cells: Vec<([i32; 3], usize)>,
}

impl Decoded {
    fn empty() -> Self {
        Decoded {
            palette: Vec::new(),
            cells: Vec::new(),
        }
    }

    /// Keep a block whose state indexes a named, non-air palette entry.
    fn keep(&mut self, pos: [i32; 3], state: usize) {
        if let Some(Some((name, _))) = self.palette.get(state)
            && !is_air(name.split('[').next().unwrap_or(name))
        {
            self.cells.push((pos, state));
        }
    }
}

/// Decode a gzipped template; unparseable bytes decode to nothing. The typed
/// decoding first; it reads exactly the shape the walk reads, and anything it
/// refuses is walked as before ([`crate::compiler::nbtread`]).
fn decode_template(bytes: &[u8], stateful: bool) -> Decoded {
    let mut raw = Vec::new();
    if flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut raw)
        .is_err()
    {
        return Decoded::empty();
    }
    decode_typed(&raw, stateful).unwrap_or_else(|| decode_walk(&raw, stateful))
}

/// [`decode_template`] over the typed decoding, or `None` when the bytes do
/// not have its strict shape.
fn decode_typed(raw: &[u8], stateful: bool) -> Option<Decoded> {
    let root = crate::compiler::nbtread::root(raw)?;
    let Some(entries) = root.palette else {
        return Some(Decoded::empty());
    };
    let mut out = Decoded {
        palette: entries
            .iter()
            .map(|e| {
                let props = match &e.properties {
                    Some(fastnbt::Value::Compound(p)) => Some(p),
                    _ => None,
                };
                e.name.as_ref().map(|s| palette_state(s, props, stateful))
            })
            .collect(),
        cells: Vec::new(),
    };
    for b in root.blocks.iter().flatten() {
        let Some(pos) = b.pos3() else {
            continue;
        };
        let Some(state) = &b.state else {
            continue;
        };
        out.keep(pos, state.0 as usize);
    }
    Some(out)
}

/// [`decode_template`] over the dynamic [`fastnbt::Value`] tree.
fn decode_walk(raw: &[u8], stateful: bool) -> Decoded {
    let Ok(fastnbt::Value::Compound(root)) = fastnbt::from_bytes::<fastnbt::Value>(raw) else {
        return Decoded::empty();
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
                        Some(palette_state(s, props, stateful))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect(),
        _ => return Decoded::empty(),
    };
    let mut out = Decoded {
        palette,
        cells: Vec::new(),
    };
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
            out.keep(pos, state);
        }
    }
    out
}

/// The cells a decoded template yields, as [`structure_cells_inner`] returns
/// them.
#[cfg(test)]
fn cells_of(d: &Decoded) -> Vec<TemplateCell> {
    d.cells
        .iter()
        .map(|&(pos, i)| {
            let (name, open) = d.palette[i].as_ref().expect("a kept entry");
            (pos, name.clone(), *open)
        })
        .collect()
}

/// One palette entry as the structure readers render it: the block id (with
/// its properties in sorted key order when `stateful`) and its `open` property.
fn palette_state(
    name: &String,
    props: Option<&std::collections::HashMap<String, fastnbt::Value>>,
    stateful: bool,
) -> (String, Option<bool>) {
    let open = match props.and_then(|p| p.get("open")) {
        Some(fastnbt::Value::String(v)) => Some(v == "true"),
        _ => None,
    };
    // `fastnbt`'s compound is a `HashMap`, so the property order it yields is
    // hash order — collect through a `BTreeMap` before rendering (ADR-0006: no
    // hash-order iteration in the compiler).
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
                name.clone()
            } else {
                format!("{name}[{body}]")
            }
        }
        _ => name.clone(),
    };
    (name, open)
}

/// **What the assembled world puts inside one gate anchor's region at world-load.**
///
/// A gate region is not empty because it is a gate; it holds whatever the prefab
/// `.nbt` authors there, and the two cases both ship in the shipped library:
/// `hello-room`'s `anchor/door` is six cells of `iron_bars` (a barred doorway the
/// campaign must `open-gate`), and `island-mountain`'s `anchor/boulder` is
/// twenty-seven cells of air (an open cave mouth a `close-gate` seals later).
/// Which one a gate is, is a **measurement**, never a default — see
/// [`Assembled::gate_seals`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateSeal {
    /// The area the carrying piece was placed in.
    pub area: String,
    /// The gate anchor name (`anchor/door`).
    pub anchor: String,
    /// The gate region's inclusive corners (absolute world coords) — the key
    /// `crate::compiler::plan::RegionEvent` uses, so a world-load seal and an `open-gate`
    /// on the same anchor meet in the one latest-write-wins model.
    pub region: ([i32; 3], [i32; 3]),
    /// How many cells the region has.
    pub cells: usize,
    /// How many of them hold a non-air block at world-load. `0` = the gate is
    /// authored **open**; anything else = authored **sealed**.
    pub blocked: usize,
    /// How many blocked cells hold a block other than the one the anchor declares
    /// — the residue an `open-gate` will **not** clear, because the emitted fill is
    /// `replace`-filtered to the declared block. `0` is the healthy case. See
    /// `docs/reference/compiler.md` for the stated limitation this number exposes.
    pub foreign: usize,
    /// Every blocked cell whose block does not cover its cell, with that block's
    /// server footprint ([`delvewright_dsl::blockshape::server_footprint`]) —
    /// iron bars, a pane. Measured from the bytes, as `blocked` is, so the seal
    /// the route model lays is the block the world holds: a floor of bars is
    /// read as the posts a body falls between, never as a full cube. Empty for
    /// a seal of blocks that cover their cells.
    pub open_footprints: Vec<([i32; 3], delvewright_dsl::blockshape::Footprint16)>,
    /// Every blocked cell whose block **hides** what stands behind it from the
    /// eye ([`delvewright_dsl::blockshape::hides`]). Measured from the bytes,
    /// so a shut way's sight is the block the world holds: a grille of iron
    /// bars is looked through, a hearth's stone back is not.
    pub hiding: Vec<[i32; 3]>,
    /// **Where a body stands on this gate while it is shut**, when it is a
    /// floor: a region one course thick whose every cell holds a block at
    /// world-load with nothing over it — a hatch, the cover over a ladder's
    /// hole. The feet height a body standing on it has (the course plus the
    /// tallest measured collision top among its blocks), and the points over
    /// it to stand at: every cell centre, every midpoint between two of its
    /// cells and every corner four of them share, because a body falls through
    /// a hatch where its boxes leave a gap, and a lattice's gap is between its
    /// cells. Measured from the bytes, never from the nav model's verdict, so
    /// the bot that stands there judges the model rather than repeating it.
    /// `None` for every other gate.
    pub stand: Option<GateStand>,
}

/// Where a body stands on a shut floor gate ([`GateSeal::stand`]).
#[derive(Clone, Debug, PartialEq)]
pub struct GateStand {
    /// The feet height of a body standing on the gate, in blocks.
    pub feet_y: f64,
    /// The `(x, z)` points over the gate to stand at, in blocks.
    pub points: Vec<[f64; 2]>,
}

impl Eq for GateStand {}

/// The stand of a one-course region `lo..=hi` whose blocks top out at
/// `top_16` sixteenths: every half-cell point strictly inside its footprint.
fn gate_stand(lo: [i32; 3], hi: [i32; 3], top_16: u8) -> GateStand {
    let mut points = Vec::new();
    for i in 1..(2 * (hi[0] - lo[0] + 1)) {
        for k in 1..(2 * (hi[2] - lo[2] + 1)) {
            points.push([
                f64::from(lo[0]) + f64::from(i) / 2.0,
                f64::from(lo[2]) + f64::from(k) / 2.0,
            ]);
        }
    }
    GateStand {
        feet_y: f64::from(lo[1]) + f64::from(top_16) / 16.0,
        points,
    }
}

impl GateSeal {
    /// Whether the assembled world authors this gate shut.
    pub fn sealed(&self) -> bool {
        self.blocked > 0
    }

    /// This gate's row in the `validation/gate-seal.json` ledger.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "area": self.area,
            "anchor": self.anchor,
            "from": self.region.0,
            "to": self.region.1,
            "cells": self.cells,
            "blocked_at_world_load": self.blocked,
            "foreign_blocks": self.foreign,
            "sealed": self.sealed(),
            "stand": self.stand.as_ref().map(|st| serde_json::json!({
                "feet_y": st.feet_y,
                "points": st.points,
            })),
        })
    }
}

/// **The world-load gate ledger** (`validation/gate-seal.json`,
/// `docs/reference/playtest-methodology.md` rule 1): what the completability model
/// measured about every gate the layout resolves.
///
/// It exists because the failure this whole measurement replaces was invisible in
/// exactly this way — the model's answer for a gate was a constant, so a green
/// carried no information about whether any gate had been looked at. A campaign
/// with no gate anchor emits no file at all, so a file that exists and reports
/// `"sealed": 0` is a finding (every gate is authored open) rather than an absence.
pub fn gate_seal_ledger(seals: &[GateSeal], modelled: usize) -> serde_json::Value {
    serde_json::json!({
        "gates_examined": seals.len(),
        "sealed_at_world_load": seals.iter().filter(|s| s.sealed()).count(),
        "modelled_as_sealed": modelled,
        "gates": seals.iter().map(GateSeal::to_json).collect::<Vec<_>>(),
        "unbound": modelled == 0,
    })
}

/// Measure what the placed world authors inside every resolved gate anchor's
/// region, **before** [`placed_blocks`] clears those cells out of the base model.
///
/// Deterministic (ADR-0006): `plan.anchors` is a `BTreeMap<(area, anchor), _>`, so
/// the ledger is emitted in `(area, anchor)` order.
pub(crate) fn measure_gate_seals(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    blocks: &BlockMap,
) -> Vec<GateSeal> {
    let mut out = Vec::new();
    for ((area, anchor), resolved) in anchors {
        let ResolvedAnchor::Gate { from, to, block } = resolved else {
            continue;
        };
        let declared = base_id(block);
        let (mut cells, mut blocked, mut foreign) = (0usize, 0usize, 0usize);
        let mut open_footprints = Vec::new();
        let mut hiding = Vec::new();
        let (lo, hi) = (
            [from[0].min(to[0]), from[1].min(to[1]), from[2].min(to[2])],
            [from[0].max(to[0]), from[1].max(to[1]), from[2].max(to[2])],
        );
        // A floor gate: one course, every cell blocked, nothing over any.
        let mut floor = lo[1] == hi[1];
        let mut top_16 = 0u8;
        for cell in region_cells(*from, *to) {
            let above = [cell[0], cell[1] + 1, cell[2]];
            match blocks.get(&cell) {
                Some(name) if !is_air(name) => {
                    let top = delvewright_dsl::blockshape::measured_collision(name)
                        .and_then(|m| m.span)
                        .and_then(|(_, hi)| hi.whole())
                        .and_then(|t| u8::try_from(t).ok())
                        .unwrap_or(delvewright_dsl::blockshape::FULL_HEIGHT_16);
                    top_16 = top_16.max(top);
                }
                _ => floor = false,
            }
            if blocks.get(&above).is_some_and(|b| !is_air(b)) {
                floor = false;
            }
            cells += 1;
            let Some(name) = blocks.get(&cell) else {
                continue; // absent from the map = air
            };
            if is_air(name) {
                continue;
            }
            blocked += 1;
            if base_id(name) != declared {
                foreign += 1;
            }
            let fp = delvewright_dsl::blockshape::server_footprint(name);
            if fp != delvewright_dsl::blockshape::FULL_FOOTPRINT {
                open_footprints.push((cell, fp));
            }
            if delvewright_dsl::blockshape::hides(name) {
                hiding.push(cell);
            }
        }
        out.push(GateSeal {
            area: area.clone(),
            anchor: anchor.clone(),
            region: (*from, *to),
            cells,
            blocked,
            foreign,
            open_footprints,
            hiding,
            stand: floor.then(|| gate_stand(lo, hi, top_16)),
        });
    }
    out
}

/// What [`placed_blocks`] produces: the un-settled world plus the two facts about
/// it that survive gravity settling unchanged.
struct Placed {
    /// The un-settled cell→block map.
    blocks: BlockMap,
    /// Fence-gate cells authored `open=true` ([`Assembled::open_gates`]).
    open_gates: BTreeSet<[i32; 3]>,
    /// The per-gate world-load measurement ([`Assembled::gate_seals`]).
    gate_seals: Vec<GateSeal>,
    /// The cells a build write reached ([`Assembled::reached`]).
    reached: Reached,
}

/// The un-settled cell→block map: placed structures + solver seals + gate clears,
/// exactly as the two legacy models built it — plus the set of fence-gate cells
/// whose authored block state is `open=true` (an open gate threshold is
/// passable; a closed one is passable-with-use), plus the per-gate world-load
/// measurement ([`GateSeal`]) taken immediately before the gate clear. Kept
/// separate from settling so unit tests can exercise each half.
/// The block a template holds where it places nothing.
const STRUCTURE_VOID: &str = "minecraft:structure_void";

/// **The cells one `/place template <pos> <rotation>` writes**, in the world:
/// each with its rotated state and, for a fence gate, whether it stands open.
/// A template's air is written (it carves what stood there); a template's
/// `structure_void` is a cell the game does not place (spec-0098 §7), so it is
/// not in the list — whatever stood there shows through, exactly as at a cell
/// the template omits. The one reading [`placed_blocks`] and `DW1002`'s write
/// sequence (`crate::compiler::attached`) both take.
pub(crate) fn template_writes(
    rotation: crate::compiler::solver::Rotation,
    pos: [i32; 3],
    bytes: &[u8],
) -> Vec<([i32; 3], BlockState, Option<bool>)> {
    // Blockstate-preserving read: waterlogging, slab halves and snow-layer
    // counts are block STATE, and the fluid/step models are wrong without
    // them. Every classifier matches on [`base_id`]. Each distinct state is
    // rotated once per template.
    let mut rotated: BTreeMap<BlockState, BlockState> = BTreeMap::new();
    structure_cells_placed(bytes)
        .into_iter()
        .filter(|(_, name, _)| base_id(name) != STRUCTURE_VOID)
        .map(|(local, name, open)| {
            // Vanilla rotates blockstates as well as positions during
            // `/place template … <rotation>` — see [`rotate_state`].
            let name = *rotated
                .entry(name)
                .or_insert_with(|| BlockState::new(&rotate_state(&name, rotation)));
            let t = rotation.transform(local);
            ([pos[0] + t[0], pos[1] + t[1], pos[2] + t[2]], name, open)
        })
        .collect()
}

/// **Every block entity the placed templates carry** (spec-0112 §5.4): per world
/// cell, the template block's rotated state and its `nbt` compound, in the
/// order [`placed_blocks`] writes templates, so a later template's cell — with
/// or without a compound — replaces an earlier one's. A block entity is a
/// picture only where the world still holds the template's block there, which
/// the caller asks of the configuration it writes.
pub fn template_block_entities(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<[i32; 3], (BlockState, crate::compiler::view::world::Nbt)> {
    let mut out: BTreeMap<[i32; 3], (BlockState, crate::compiler::view::world::Nbt)> =
        BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for template in &piece.templates {
                let Some(bytes) = structures.get(&template.structure_file) else {
                    continue;
                };
                let Some(raw) = gunzip(bytes) else { continue };
                let Some(root) = crate::compiler::nbtread::root(&raw) else {
                    continue;
                };
                let palette: Vec<Option<String>> = decode_template(bytes, true)
                    .palette
                    .iter()
                    .map(|e| e.as_ref().map(|(n, _)| n.clone()))
                    .collect();
                for b in root.blocks.iter().flatten() {
                    let (Some(local), Some(state)) = (b.pos3(), &b.state) else {
                        continue;
                    };
                    let Some(Some(name)) = palette.get(state.0 as usize) else {
                        continue;
                    };
                    if base_id(name) == STRUCTURE_VOID {
                        continue;
                    }
                    let t = piece.rotation.transform(local);
                    let cell = [
                        template.pos[0] + t[0],
                        template.pos[1] + t[1],
                        template.pos[2] + t[2],
                    ];
                    match b.nbt() {
                        Some(v) => {
                            let rotated = BlockState::new(&rotate_state(name, piece.rotation));
                            out.insert(
                                cell,
                                (rotated, crate::compiler::view::world::Nbt::from_value(v)),
                            );
                        }
                        None => {
                            out.remove(&cell);
                        }
                    }
                }
            }
        }
    }
    out
}

/// One write of the assembly's program, in the order the world lays them.
enum AssemblyWrite<'a> {
    /// A region fill — an area's mass or seal: `None` clears to air.
    Region {
        from: [i32; 3],
        to: [i32; 3],
        block: Option<BlockState>,
    },
    /// A piece's template, placed with the piece's rotation; its air is
    /// placed too, and carves what stood there.
    Template {
        rotation: crate::compiler::solver::Rotation,
        template: &'a crate::compiler::plan::PlacedTemplate,
    },
    /// A surround tile: unrotated, its air not placed.
    Tile(&'a crate::compiler::plan::PlacedTemplate),
}

impl AssemblyWrite<'_> {
    /// The inclusive box the write's cells lie in.
    fn footprint(&self) -> ([i32; 3], [i32; 3]) {
        match self {
            AssemblyWrite::Region { from, to, .. } => (
                [0, 1, 2].map(|i| from[i].min(to[i])),
                [0, 1, 2].map(|i| from[i].max(to[i])),
            ),
            AssemblyWrite::Template { rotation, template } => {
                let a = rotation.transform([0, 0, 0]);
                let b = rotation.transform(template.size.map(|n| n - 1));
                (
                    [0, 1, 2].map(|i| template.pos[i] + a[i].min(b[i])),
                    [0, 1, 2].map(|i| template.pos[i] + a[i].max(b[i])),
                )
            }
            AssemblyWrite::Tile(t) => (t.pos, [0, 1, 2].map(|i| t.pos[i] + t.size[i] - 1)),
        }
    }

    /// The write's cells, each with the block it leaves (`None`: air) and,
    /// for a fence gate, whether it stands open.
    fn cells(
        &self,
        structures: &BTreeMap<String, Vec<u8>>,
    ) -> Vec<([i32; 3], Option<BlockState>, Option<bool>)> {
        match self {
            AssemblyWrite::Region { from, to, block } => region_cells(*from, *to)
                .map(|c| (c, *block, None))
                .collect(),
            AssemblyWrite::Template { rotation, template } => {
                let Some(bytes) = structures.get(&template.structure_file) else {
                    return Vec::new();
                };
                template_writes(*rotation, template.pos, bytes)
                    .into_iter()
                    .map(|(c, name, open)| (c, (!is_air(&name)).then_some(name), open))
                    .collect()
            }
            AssemblyWrite::Tile(template) => {
                let Some(bytes) = structures.get(&template.structure_file) else {
                    return Vec::new();
                };
                // A template's `structure_void` is a cell the game does not
                // place (spec-0098 §7): whatever stood there shows through,
                // exactly as at a cell the template omits. Writing it as a
                // block would model a voided cell as passable while the
                // server keeps the wall under it.
                structure_cells_interned(bytes)
                    .into_iter()
                    .filter(|(_, name, _)| base_id(name) != STRUCTURE_VOID)
                    .map(|(local, name, open)| {
                        let cell = [0, 1, 2].map(|i| template.pos[i] + local[i]);
                        (cell, Some(name), open)
                    })
                    .collect()
            }
        }
    }
}

fn placed_blocks(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Placed {
    let mut blocks = BlockMap::new();
    let mut open_gates: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut reached = Reached::default();
    // The assembly's program: every write the world lays, in the order it
    // lays them, so a later write wins a cell. Per area: its own mass first
    // — a derived blockout's blocks arrive as region writes rather than in a
    // `.nbt` (`crate::compiler::blockout`), and they are the *ground* the
    // rest of the area stands in, so anything placed over them wins, exactly
    // as a template placed over an earlier template does (empty for every
    // prefab-placed area) — then its pieces' templates in order, then its
    // seals: an air fill opens a mated socket; anything else seals an unused
    // one, and either way the sealed cell is no longer an authored open
    // gate. The horizon's surround after every area (spec-0026): terrain
    // the compiler generated rather than a prefab an author bound, entering
    // the voxel model on exactly the terms a placed piece does — so gravity
    // settling, the occupancy model, relight, the fluid model, boundary
    // safety and the frame worlds all see the landform without one of them
    // being taught a new horizon. AFTER, not before: the surround stands
    // OUTSIDE the map's declared rectangle by construction, so the two
    // cannot contend for a cell — and if they ever do, the map wins the
    // argument, because the map is the thing the campaign is about. `None`
    // for a base with no surround.
    let region = |f: &crate::compiler::solver::SealFill| AssemblyWrite::Region {
        from: f.from,
        to: f.to,
        block: (!is_air(&f.block)).then(|| BlockState::new(&f.block)),
    };
    let mut program: Vec<AssemblyWrite> = Vec::new();
    for area in &plan.areas {
        program.extend(area.mass.iter().map(region));
        for piece in &area.pieces {
            program.extend(
                piece
                    .templates
                    .iter()
                    .map(|template| AssemblyWrite::Template {
                        rotation: piece.rotation,
                        template,
                    }),
            );
        }
        program.extend(area.seals.iter().map(region));
    }
    let mut tiles = 0;
    if let Some(surround) = &plan.surround {
        tiles = surround.piece.templates.len();
        program.extend(surround.piece.templates.iter().map(AssemblyWrite::Tile));
    }
    let templates = program
        .iter()
        .filter(|w| matches!(w, AssemblyWrite::Template { .. }))
        .count();
    // The program is laid section column by section column
    // (`write_by_section_column`), not write by write: a row of places or
    // of tiles shares its border sections with the next row, and write by
    // write that row is the store's working set, O(width) in sections.
    let footprints: Vec<_> = program.iter().map(AssemblyWrite::footprint).collect();
    let sweep = write_by_section_column(
        &footprints,
        |i| program[i].cells(structures),
        |cell, block, open| {
            reached.mark(cell);
            match block {
                None => {
                    blocks.remove(&cell);
                    open_gates.remove(&cell);
                }
                Some(name) => {
                    if opens_by_hand(&name) && open == Some(true) {
                        open_gates.insert(cell);
                    } else {
                        open_gates.remove(&cell); // a later block overwrites the cell
                    }
                    blocks.insert(cell, name);
                }
            }
        },
    );
    crate::note!(
        "{}",
        sweep.line(program.len() - templates - tiles, templates, tiles)
    );
    // **The base world holds every gate threshold open, and that is a choice about
    // the BASE world only.**
    //
    // It has to pick one state, and "open" is the one that keeps the fill/clear
    // model expressible: `crate::compiler::plan::RegionWrite::Unseal` (what `open-gate`
    // emits) is `replace`-filtered to the gate's own block, so a model that held
    // the bars here would need a block-aware clear to remove them and would then
    // wrongly clear a `collapse`'s debris resting in the same doorway (`DW0445`).
    //
    // What is NOT decided here is whether the gate is shut *at world-load*: that
    // is measured, one line above the clear, and re-enters the model as a
    // world-load `Fill` at step 0 — the identical shape a shortcut gate's seal
    // already uses. Before this measurement existed, the base world's choice was
    // silently also the answer to "is this gate ever shut", so a campaign that
    // never opened its door compiled green and shipped unplayable.
    let gate_seals = measure_gate_seals(&plan.anchors, &blocks);
    for resolved in plan.anchors.values() {
        if let ResolvedAnchor::Gate { from, to, .. } = resolved {
            for cell in region_cells(*from, *to) {
                blocks.remove(&cell);
                open_gates.remove(&cell);
            }
        }
    }
    Placed {
        blocks,
        open_gates,
        gate_seals,
        reached,
    }
}

/// The outcome of gravity settling for one falling block: the world cell it ended
/// up at (or `None` if it despawned into the void), tagged with its original cell.
/// **A template's cells in section order** (ADR-0037 §2, assembly): the
/// cells grouped by the store section each lies in, the sections in key
/// order, and the cells of one section in the order the template listed them
/// (a stable sort), so a template is written section by section and the
/// store's working set while it is written is one section, whatever order
/// the template's bytes list its cells in. A cell a template lists twice
/// keeps its two writes in order, so the map written is the one the listed
/// order writes.
fn by_section<T, U>(cells: &mut [([i32; 3], T, U)]) {
    cells.sort_by_key(|(c, _, _)| section_key(*c));
}

fn section_key(c: [i32; 3]) -> [i32; 3] {
    c.map(|v| v.div_euclid(crate::compiler::sections::SIDE))
}

/// The section column `[sz, sx]` a cell lies in: the sweep's key, `z`
/// before `x`, so a tile row along `x` is swept within one band of `z`.
fn column_key(c: [i32; 3]) -> [i32; 2] {
    let s = section_key(c);
    [s[2], s[0]]
}

/// The columns an inclusive box `lo..=hi` covers: inclusive `[sz, sx]`
/// corners, `None` for a box with no cells.
fn column_span(lo: [i32; 3], hi: [i32; 3]) -> Option<([i32; 2], [i32; 2])> {
    if lo[0] > hi[0] || lo[1] > hi[1] || lo[2] > hi[2] {
        return None;
    }
    Some((column_key(lo), column_key(hi)))
}

/// What a column sweep did: stated on every build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnSweep {
    /// Writes listed.
    pub writes: usize,
    /// Section columns some write reaches, each written in one step.
    pub columns: usize,
    /// Writes decoded: a write once per band of sections along `z` it
    /// crosses, never held across bands.
    pub decodes: usize,
    /// The most writes held decoded at once.
    pub held: usize,
}

impl ColumnSweep {
    /// The assembly's line: the writes by kind, the columns, the decodes.
    pub fn line(&self, regions: usize, templates: usize, tiles: usize) -> String {
        format!(
            "assembly written: {} write(s) — {} region write(s), {} template(s), {} surround \
             tile(s) — over {} section column(s), each column in one step; {} decode(s), at \
             most {} write(s) held decoded at once (ADR-0037 §2)",
            self.writes, regions, templates, tiles, self.columns, self.decodes, self.held
        )
    }
}

/// **Writes laid section column by section column** (ADR-0037 §2,
/// assembly): the `(sx, sz)` section columns the writes' footprints
/// (inclusive boxes) cover are swept once each, in `z`-band then `x`
/// order, and at each column every write reaching it is asked for the
/// cells it lays there, in write order, those cells written together,
/// grouped by section (a stable sort). Every cell's writes come in the
/// order listed, so the map written is the one write-by-write laying makes
/// (`a_sweep_by_column_writes_what_write_order_writes`), while no section
/// is written in two steps: the store's working set is one column,
/// whatever the row's width. A write is decoded when the sweep first
/// reaches it in a band and dropped when the band's sweep has passed it,
/// so the writes held decoded are those under the column in hand; a write
/// is decoded once per band it crosses. `decode(i)` gives write `i`'s
/// cells in world coordinates; a cell outside the footprint declared for
/// its write is refused, since the sweep could not have planned a step
/// for it.
pub(crate) fn write_by_section_column<V: Copy + Send>(
    footprints: &[([i32; 3], [i32; 3])],
    decode: impl Fn(usize) -> Vec<([i32; 3], V, Option<bool>)> + Sync,
    mut write: impl FnMut([i32; 3], V, Option<bool>),
) -> ColumnSweep {
    type Cells<V> = Vec<([i32; 3], V, Option<bool>)>;
    let spans: Vec<Option<([i32; 2], [i32; 2])>> = footprints
        .iter()
        .map(|&(lo, hi)| column_span(lo, hi))
        .collect();
    // Every column some write reaches, with the writes reaching it in
    // order.
    let mut columns: BTreeMap<[i32; 2], Vec<usize>> = BTreeMap::new();
    for (i, span) in spans.iter().enumerate() {
        let Some((lo, hi)) = span else { continue };
        for sz in lo[0]..=hi[0] {
            for sx in lo[1]..=hi[1] {
                columns.entry([sz, sx]).or_default().push(i);
            }
        }
    }
    let mut sweep = ColumnSweep {
        writes: footprints.len(),
        columns: columns.len(),
        decodes: 0,
        held: 0,
    };
    // The writes decoded, each split by the column its cells lie in.
    let mut held: BTreeMap<usize, BTreeMap<[i32; 2], Cells<V>>> = BTreeMap::new();
    for (&[sz, sx], tiles) in &columns {
        let fresh: Vec<usize> = tiles
            .iter()
            .copied()
            .filter(|i| !held.contains_key(i))
            .collect();
        let decoded = crate::par::map(&fresh, |&i| {
            let (lo, hi) = spans[i].expect("a write the sweep reaches has a footprint");
            let mut by: BTreeMap<[i32; 2], Cells<V>> = BTreeMap::new();
            for cell in decode(i) {
                let k = column_key(cell.0);
                assert!(
                    (lo[0]..=hi[0]).contains(&k[0]) && (lo[1]..=hi[1]).contains(&k[1]),
                    "write {i} lists the cell {:?} outside its declared footprint {:?}",
                    cell.0,
                    footprints[i]
                );
                by.entry(k).or_default().push(cell);
            }
            by
        });
        sweep.decodes += fresh.len();
        held.extend(fresh.into_iter().zip(decoded));
        sweep.held = sweep.held.max(held.len());
        // The column's cells over every write reaching it, in write order,
        // then grouped by section (a stable sort): a cell's writes keep
        // their order, and each section is written in one run.
        let mut cells: Cells<V> = Vec::new();
        for &i in tiles {
            if let Some(mine) = held.get_mut(&i).and_then(|by| by.remove(&[sz, sx])) {
                cells.extend(mine);
            }
        }
        by_section(&mut cells);
        for (c, v, o) in cells {
            write(c, v, o);
        }
        // A write stays decoded only while this band's sweep still reaches it.
        held.retain(|&i, _| {
            let (lo, hi) = spans[i].expect("a held write has a footprint");
            lo[0] <= sz && sz <= hi[0] && hi[1] > sx
        });
    }
    sweep
}

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
/// **Fluids are not supports**. Vanilla's `FallingBlock.isFree` counts
/// a liquid cell as free space: a falling block sinks straight through water/lava
/// and lands on the first genuinely solid block, *displacing* the fluid in the
/// cell it comes to rest in. Treating a `minecraft:water` cell as an immovable
/// support would settle a sand block authored over a pool "on the water surface"
/// — a floating floor the game does not have, which the flood would dam and nav
/// would walk on. See [`is_fluid`].
///
/// Deterministic (ADR-0006): the outcomes come in `(x, z)` column order and blocks
/// stack bottom-up.
fn settle(blocks: &mut BlockMap) -> Vec<Settled> {
    // Section column by section column (ADR-0037 §2, assembly): the cells of
    // one `(sx, sz)` column of sections are read, grouped by cell column and
    // settled, and the store's working set is that column's sections. A cell
    // column's outcomes are the same whatever column is taken first, and the
    // list is put back in `(x, z)` order at the end (a stable sort over a
    // column's own order), so what is returned is what the walk in cell
    // order returned.
    let side = crate::compiler::sections::SIDE;
    let mut outcomes: Vec<((i32, i32), Settled)> = Vec::new();
    for ([sx, sz], [sy0, sy1]) in blocks.section_columns() {
        let lo = [sx * side, sy0 * side, sz * side];
        let hi = [
            sx * side + side - 1,
            sy1 * side + side - 1,
            sz * side + side - 1,
        ];
        let mut columns: BTreeMap<(i32, i32), Vec<(i32, BlockState)>> = BTreeMap::new();
        for (c, b) in blocks.in_box(lo, hi) {
            columns.entry((c[0], c[2])).or_default().push((c[1], *b));
        }
        for ((x, z), cells) in columns {
            settle_column(blocks, x, z, cells, &mut outcomes);
        }
    }
    outcomes.sort_by_key(|(col, _)| *col);
    outcomes.into_iter().map(|(_, s)| s).collect()
}

/// Settle one cell column `(x, z)` whose cells are `cells` (any order), writing
/// the landed blocks into `blocks` and pushing each falling block's outcome,
/// tagged with the column, in the order the column gives them: the despawns
/// in ascending `y`, then each support's group from the lowest support up.
fn settle_column(
    blocks: &mut BlockMap,
    x: i32,
    z: i32,
    mut cells: Vec<(i32, BlockState)>,
    outcomes: &mut Vec<((i32, i32), Settled)>,
) {
    cells.sort_by_key(|(y, _)| *y);
    // Split the column into immovable supports and falling blocks (ascending).
    // A liquid cell is NEITHER: it does not hold a falling block up, and a
    // block that lands in it replaces it.
    let mut fixed: Vec<i32> = Vec::new();
    let mut falling: Vec<(i32, BlockState)> = Vec::new();
    for (y, name) in cells {
        if is_falling_block(&name) {
            falling.push((y, name));
        } else if !is_fluid(&name) {
            fixed.push(y);
        }
    }
    if falling.is_empty() {
        return; // nothing to settle in this column
    }
    // Lift every falling block out; re-drop onto its support.
    for (y, _) in &falling {
        blocks.remove(&[x, *y, z]);
    }
    // Group falling blocks by the nearest immovable support strictly below
    // them; a group has no support → those blocks despawned into the void.
    let mut by_base: BTreeMap<i32, Vec<(i32, BlockState)>> = BTreeMap::new();
    for (y, name) in falling {
        if let Some(base) = fixed.iter().copied().filter(|&f| f < y).max() {
            by_base.entry(base).or_default().push((y, name));
        } else {
            // No support anywhere below → despawns into the void.
            outcomes.push((
                (x, z),
                Settled {
                    block: name.to_string(),
                    from: [x, y, z],
                    to: None,
                },
            ));
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
            outcomes.push((
                (x, z),
                Settled {
                    block: name.to_string(),
                    from: [x, from_y, z],
                    to: Some([x, yy, z]),
                },
            ));
            blocks.insert([x, yy, z], name);
            yy += 1;
        }
    }
}

/// Re-run gravity settling over an (edited) cell→block map — the map editor's
/// re-entry into the settle model (spec-0017): a `setblock`-placed falling
/// block falls exactly like a template-placed one, so after every edit batch
/// the model re-settles and the same despawn rule applies. Same algorithm,
/// same determinism as the assembly-time [`settle`].
pub(crate) fn resettle(blocks: &mut BlockMap) -> Vec<Settled> {
    settle(blocks)
}

/// The authoritative assembled-world model: the settled cell→block map plus the
/// per-falling-block settle outcomes (for the gravity-despawn diagnostic).
pub struct Assembled {
    /// The gravity-settled cell→block map (cells absent from it are air).
    ///
    /// Shared rather than owned so a model that reads it whole (the light
    /// model) holds it without a copy; the edit replay, its one writer,
    /// mutates its own copy through `Arc::make_mut`.
    pub blocks: std::sync::Arc<BlockMap>,
    /// One outcome per falling block: where it came to rest, or `None` if it
    /// despawned into the void.
    pub settled: Vec<Settled>,
    /// Fence-gate cells whose authored block state is `open=true`:
    /// passable thresholds, as opposed to closed gates (passable-with-use).
    pub open_gates: BTreeSet<[i32; 3]>,
    /// One entry per **resolved gate anchor**, in `(area, anchor)` order, saying
    /// what the placed world puts in its region at world-load ([`GateSeal`]).
    ///
    /// Every gate is listed, sealed or not, because the binding count of every
    /// proof that reads this is "how many gates were examined", not "how many
    /// happened to be shut" (CLAUDE.md: *a green gate that binds to nothing is
    /// vacuous*).
    pub gate_seals: Vec<GateSeal>,
    /// **Every cell a build write reached** at or under [`Reached::CEILING`]:
    /// a template's placed cell (its air included, its `structure_void` not),
    /// an area's mass and seals, the surround, an edit's write. The block map
    /// cannot say this — a written air and a cell nothing wrote are both
    /// absent from it — and the server tells them apart: a cell nothing wrote
    /// holds what the level generator laid there
    /// ([`crate::compiler::plan::ocean_layer_at`] on an `ocean` horizon base).
    pub reached: Reached,
}

/// Which cells at or under [`Reached::CEILING`] a build write reached, as one
/// bit per height per column — the band a superflat stack can occupy, held at
/// a cost of one word per column rather than one entry per cell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reached {
    columns: BTreeMap<(i32, i32), u128>,
    /// **The sections a build write covers**, every height: one bit per
    /// section row (from the build floor's section) per section column. What
    /// bounds the stores a stage lays over the written world — the write
    /// sequence judge's — in the cost the engine states (`compiler::cost`,
    /// ADR-0037 §3): a derived fact of the write program, never of a run.
    sections: BTreeMap<(i32, i32), u64>,
    /// The section last marked, so a sweep that marks a section's cells in
    /// a row looks its row up once.
    last: Option<[i32; 3]>,
}

impl Reached {
    /// The lowest height a bit stands for: the build floor.
    pub const FLOOR: i32 = crate::compiler::plan::BUILD_FLOOR_Y;
    /// The highest height a bit stands for. Every generator layer the engine
    /// emits lies at or under it (asserted below), so a cell above it is never
    /// a generator's.
    pub const CEILING: i32 = Self::FLOOR + 127;

    /// Mark `c` reached (a no-op above [`Self::CEILING`] or under [`Self::FLOOR`]
    /// for the band; its section is marked at every height).
    pub fn mark(&mut self, c: [i32; 3]) {
        if (Self::FLOOR..=Self::CEILING).contains(&c[1]) {
            *self.columns.entry((c[0], c[2])).or_default() |= 1u128 << (c[1] - Self::FLOOR);
        }
        self.mark_section(c.map(|v| v.div_euclid(crate::compiler::sections::SIDE)));
    }

    /// Mark `section` (by section coordinate) as one a build write covers;
    /// the band of `Self::columns` is untouched.
    fn mark_section(&mut self, section: [i32; 3]) {
        if self.last == Some(section) {
            return;
        }
        let row =
            u32::try_from(section[1] - Self::FLOOR.div_euclid(crate::compiler::sections::SIDE))
                .expect("a build write at or over the build floor");
        assert!(
            row < 64,
            "a build write under the section rows a word holds"
        );
        *self.sections.entry((section[0], section[2])).or_default() |= 1u64 << row;
        self.last = Some(section);
    }

    /// How many sections a build write covers, at every height.
    pub fn sections_written(&self) -> usize {
        self.sections
            .values()
            .map(|m| m.count_ones() as usize)
            .sum()
    }

    /// Mark every cell of the inclusive box `a..=b` reached.
    pub fn mark_box(&mut self, a: [i32; 3], b: [i32; 3]) {
        let (lo, hi) = (
            a[1].min(b[1]).max(Self::FLOOR),
            a[1].max(b[1]).min(Self::CEILING),
        );
        if lo > hi {
            return;
        }
        let mut mask = 0u128;
        for y in lo..=hi {
            mask |= 1u128 << (y - Self::FLOOR);
        }
        for x in a[0].min(b[0])..=a[0].max(b[0]) {
            for z in a[2].min(b[2])..=a[2].max(b[2]) {
                *self.columns.entry((x, z)).or_default() |= mask;
            }
        }
        let (ylo, yhi) = (a[1].min(b[1]).max(Self::FLOOR), a[1].max(b[1]));
        for sx in a[0].min(b[0]).div_euclid(16)..=a[0].max(b[0]).div_euclid(16) {
            for sz in a[2].min(b[2]).div_euclid(16)..=a[2].max(b[2]).div_euclid(16) {
                for sy in ylo.div_euclid(16)..=yhi.div_euclid(16) {
                    self.mark_section([sx, sy, sz]);
                }
            }
        }
    }

    /// Whether a build write reached `c`. Above [`Self::CEILING`] the answer is
    /// `false`: nothing there is a generator's to show through.
    pub fn contains(&self, c: [i32; 3]) -> bool {
        (Self::FLOOR..=Self::CEILING).contains(&c[1])
            && self
                .columns
                .get(&(c[0], c[2]))
                .is_some_and(|m| m & (1u128 << (c[1] - Self::FLOOR)) != 0)
    }
}

const _: () = assert!(
    crate::compiler::plan::SEA_LEVEL <= Reached::CEILING,
    "every ocean layer lies in the band `Reached` records"
);

/// **The world as shipped, before any runtime write**: the assembled blocks,
/// every gate the placed world authors shut, the relight fixtures and the
/// trigger props the datapack sets at setup — the assembly clears a gate's region (the route model
/// reads the gate through its seal), and the datapack's setup writes the gate's
/// block back. Clipped to `clip` when one is given (inclusive), so a caller
/// asking about one box does not copy the map: [`shipped_within`] over
/// [`shipped_writes`].
///
/// The one derivation of those bytes: the loop's tiling (spec-0086), a
/// camera's world (spec-0089) and the clear-width check (spec-0120) all read it.
pub fn shipped_blocks(
    plan: &Plan,
    blocks: &BlockMap,
    placements: &[crate::compiler::light::Placement],
    seals: &[GateSeal],
    clip: Option<([i32; 3], [i32; 3])>,
) -> BlockMap {
    shipped_within(
        blocks,
        &shipped_writes(plan, placements, seals),
        &plan.loot,
        clip,
    )
}

/// A write of the shipped world: the cell and the block it holds there, or
/// `None` where the write clears it.
pub type ShippedWrite = ([i32; 3], Option<BlockState>);

/// **What setup lays over the assembled blocks**, in the order it lays them:
/// every sealed gate's block cell by cell (`region_fill`: a door stands its
/// lower half under its upper), then the relight fixtures (`setup_finish` sets
/// them after the world-load seals stand, so a fixture inside a gate's region
/// is what the server holds there, measured by `tools/ci/check-written-world.py`),
/// then every pressable's block (spec-0117 §5.1, from the same list
/// `setup_finish` reads, [`crate::compiler::pressable::props`], a door's two
/// halves included). A cell written twice is listed twice, in order.
pub fn shipped_writes(
    plan: &Plan,
    placements: &[crate::compiler::light::Placement],
    seals: &[GateSeal],
) -> Vec<ShippedWrite> {
    let mut writes: Vec<ShippedWrite> = Vec::new();
    for s in seals.iter().filter(|s| s.sealed()) {
        if let Some((from, to, block)) =
            crate::compiler::plan::gate_region_block_any(&plan.anchors, &s.anchor)
        {
            for (c, state) in crate::compiler::region_fill::cells((from, to), &block) {
                writes.push((c, (!is_air(&state)).then(|| BlockState::new(&state))));
            }
        }
    }
    for p in placements {
        writes.push((p.pos, Some(BlockState::new(&p.block))));
    }
    for (_, p) in crate::compiler::pressable::props(plan) {
        writes.push((p.pos, Some(BlockState::new(&p.block))));
    }
    writes
}

/// **The shipped world inside `clip`** (the whole when `None`): the assembled
/// blocks there, `writes` laid over them section by section
/// ([`BlockMap::write_by_section`]: the same map the listed order lays, with a
/// working set of one section), and last what a holder shows of its contents —
/// `setup_finish` fills every `loot` holder, and a brewing stand's first tick
/// then draws its bottle slots as `has_bottle_<n>` (`loot::show_contents`), a
/// rule of each cell alone. Every step is a rule of one cell, so the world
/// inside a clip is the whole world's inside that box
/// (`a_clipped_shipped_world_is_the_whole_worlds_inside_the_clip`).
pub fn shipped_within(
    blocks: &BlockMap,
    writes: &[ShippedWrite],
    loot: &[crate::compiler::plan::LootPlan],
    clip: Option<([i32; 3], [i32; 3])>,
) -> BlockMap {
    let inside = |c: &[i32; 3]| {
        clip.is_none_or(|(lo, hi)| {
            (0..3).all(|i| lo[i].min(hi[i]) <= c[i] && c[i] <= lo[i].max(hi[i]))
        })
    };
    let mut m: BlockMap = match clip {
        Some((lo, hi)) => blocks.in_box(lo, hi).map(|(c, b)| (c, *b)).collect(),
        None => blocks.clone(),
    };
    m.write_by_section(writes.iter().filter(|(c, _)| inside(c)).copied());
    crate::compiler::loot::show_contents(loot, &mut m);
    m
}

/// Assemble the world: placed structures + solver seals + gate clears, then
/// gravity-settle, returning both the settled map and the per-falling-block
/// outcomes. Shared root for [`assembled_blocks`] and the gravity-despawn check.
pub fn assemble(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Assembled {
    let mut placed = placed_blocks(plan, structures);
    let settled = settle(&mut placed.blocks);
    Assembled {
        blocks: std::sync::Arc::new(placed.blocks),
        settled,
        open_gates: placed.open_gates,
        gate_seals: placed.gate_seals,
        reached: placed.reached,
    }
}

/// The block that closes the judged world's faces ([`Assembled::judged_within`]):
/// a full collision box that passes light and hides nothing from the eye
/// (`light-block`, `occlusion` tables) — the boundary's return, as a block.
pub const JUDGED_WALL: &str = "minecraft:barrier";

/// What [`Assembled::judged_within`] kept and left out, for the binding line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Judged {
    /// The judged columns' inclusive `[x, z]` corners.
    pub lo: [i32; 2],
    pub hi: [i32; 2],
    /// Cells of the assembled world inside the judged columns.
    pub kept: usize,
    /// Cells of the assembled world outside them: placed and written, never judged.
    pub left_out: usize,
    /// Wall cells standing on the judged columns' faces.
    pub wall: usize,
}

impl Judged {
    /// The binding line every build that judges part of its world prints.
    pub fn line(&self) -> String {
        format!(
            "judged world: columns x {}..{}, z {}..{} (the boundary's playable region grown by {}); \
             {} assembled cell(s) judged, {} placed beyond them and judged by nothing, {} wall \
             cell(s) closing the faces",
            self.lo[0],
            self.hi[0],
            self.lo[1],
            self.hi[1],
            crate::compiler::emit::JUDGED_MARGIN,
            self.kept,
            self.left_out,
            self.wall
        )
    }
}

impl Assembled {
    /// **The store's size, stated** (ADR-0037 §1): how many sections hold the
    /// world's placed cells and the bytes they cost, with the judged copy's
    /// when one was taken. The resident cost of the assembled world, which
    /// the budget of the reach graph does not count.
    pub fn store_line(&self, judged: Option<&Assembled>) -> String {
        let mut s = format!(
            "world store: {} placed cell(s) in {} section(s) of 16 x 16 x 16, {} byte(s)",
            self.blocks.len(),
            self.blocks.sections(),
            self.blocks.bytes()
        );
        if let Some(j) = judged {
            s.push_str(&format!(
                "; the judged copy {} cell(s) in {} section(s), {} byte(s)",
                j.blocks.len(),
                j.blocks.sections(),
                j.blocks.bytes()
            ));
        }
        s.push_str(" (ADR-0037 §1)");
        s
    }

    /// **The part of the world the body-judging analyses read**: the cells of
    /// the inclusive columns `lo..=hi` (`[x, z]`), every height, with a
    /// [`JUDGED_WALL`] standing on each face one column out, from the lowest
    /// cell of the world to three over its highest. `None` when no cell lies
    /// outside the columns: the world is judged whole and nothing changes.
    ///
    /// Ground past the boundary's playable region is seen and never reached —
    /// the boundary returns a body that leaves — so it enters placement and
    /// the world write and no proof of where a body can be, what it stands in
    /// or how lit it is. The wall keeps a flood from walking off the judged
    /// columns onto nothing: a body that walks out meets the return, never a
    /// fall. It passes light, so a column's sky is what it was.
    pub fn judged_within(&self, lo: [i32; 2], hi: [i32; 2]) -> Option<(Assembled, Judged)> {
        let inside =
            |c: &[i32; 3]| c[0] >= lo[0] && c[0] <= hi[0] && c[2] >= lo[1] && c[2] <= hi[1];
        let mut ymin = i32::MAX;
        let mut ymax = i32::MIN;
        let mut left_out = 0usize;
        let mut blocks = BlockMap::new();
        // Section by section (ADR-0037 §2): the copy's section is written
        // whole while the whole's is read, so the two stores' working set is
        // one section each, not a slab; the cells land in the copy in the
        // same order a walk of the whole lists them within each section.
        let side = crate::compiler::sections::SIDE;
        if let Some((slo, shi)) = self.blocks.section_bounds() {
            let (slo, shi) = (section_key(slo), section_key(shi));
            for sx in slo[0]..=shi[0] {
                for sy in slo[1]..=shi[1] {
                    for sz in slo[2]..=shi[2] {
                        let lo = [sx * side, sy * side, sz * side];
                        let hi = [lo[0] + side - 1, lo[1] + side - 1, lo[2] + side - 1];
                        for (c, b) in self.blocks.in_box(lo, hi) {
                            ymin = ymin.min(c[1]);
                            ymax = ymax.max(c[1]);
                            if inside(&c) {
                                blocks.insert(c, *b);
                            } else {
                                left_out += 1;
                            }
                        }
                    }
                }
            }
        }
        if left_out == 0 {
            return None;
        }
        let kept = blocks.len();
        let wall = BlockState::new(JUDGED_WALL);
        let mut walls = 0usize;
        for y in ymin..=ymax + 3 {
            for x in lo[0] - 1..=hi[0] + 1 {
                for z in [lo[1] - 1, hi[1] + 1] {
                    blocks.insert([x, y, z], wall);
                    walls += 1;
                }
            }
            for z in lo[1]..=hi[1] {
                for x in [lo[0] - 1, hi[0] + 1] {
                    blocks.insert([x, y, z], wall);
                    walls += 1;
                }
            }
        }
        let judged = Assembled {
            blocks: std::sync::Arc::new(blocks),
            settled: self.settled.clone(),
            open_gates: self
                .open_gates
                .iter()
                .copied()
                .filter(|c| inside(c))
                .collect(),
            gate_seals: self.gate_seals.clone(),
            reached: self.reached.clone(),
        };
        Some((
            judged,
            Judged {
                lo,
                hi,
                kept,
                left_out,
                wall: walls,
            },
        ))
    }
}

/// The authoritative assembled-world cell→block map: placed structures + solver
/// seals + gate clears, **then gravity-settled**. Cells absent from the map are
/// air. Shared by the nav occupancy model and the relight light model so a single
/// gravity-faithful world feeds every consumer.
pub fn assembled_blocks(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> BlockMap {
    std::sync::Arc::unwrap_or_clone(assemble(plan, structures).blocks)
}

/// The standard vanilla horizontal flow decay: a water source spreads at most this
/// many cells horizontally before running dry (level 1..=7 → 7 steps).
///
/// [`flood`] applies it to **lava sources too**. Overworld lava flows 3 cells, so
/// this over-marks a lava pool's reach — the direction [`assembled_occupancy`]'s
/// never-under-mark contract requires, and the reason lava does not get a flow
/// range of its own: a second constant could only ever be the smaller one, and a
/// smaller one is the failure mode this model refuses.
const WATER_FLOW_RANGE: u8 = 7;

/// The collision-classified nav occupancy of the settled assembled world.
/// The sets are pairwise disjoint; a cell in none of them is passable
/// air.
///
/// | Set | Blocks | Walk through? | Stand on top? |
/// |---|---|---|---|
/// | `solid` | every other non-air block (full cube unless listed in `partial`) | no | yes |
/// | `tall` | fences + walls ([`is_tall_barrier`], 1.5-tall) | no | **no** |
/// | `use_gates` | closed doors, trapdoors and fence gates a hand opens ([`opens_by_hand`], spec-0123 §3.1) | player: yes, via right-click USE; NPC/actor/wave walkers: no | **no** |
/// | `flooded` | fluid reach — water and lava ([`is_fluid`]), conservative superset | no | no |
/// | `partial` | a `solid` cell's true top-face height, in sixteenths, when < 16 | no | yes, **at that height** |
///
/// **Modelled precisely**: fences, walls, every door, trapdoor and fence gate a
/// hand opens (open = passable, closed = use-gate), pressure plates / tripwire and every other sub-auto-step decoration
/// (passable, [`is_thin_decoration`]), free fluid AND waterlogged blocks
/// ([`is_waterlogged`]), and partial floor heights for slabs / snow layers /
/// paths ([`collision_top_16`]). **Modelled conservatively** (treated as a full
/// solid cube — may over-block a route, never over-prove one): stairs,
/// iron doors and iron trapdoors, and every other partial-collision block whose pinned collision box
/// is not a floor at a measured height ([`collision_top_16`] reads the rest from
/// the jar's own table).
pub struct Occupancy {
    /// Full-cube solid cells: block passage AND are valid floor.
    pub solid: CellSet,
    /// 1.5-tall barrier cells (fences/walls): block passage, never valid floor.
    pub tall: CellSet,
    /// Closed cells of a door, trapdoor or fence gate a hand opens: passable-with-use
    /// for the player (adventure-legal right-click), impassable for walkers that
    /// cannot use gates; never floor.
    pub use_gates: CellSet,
    /// Fluid-flooded cells: impassable, never floor. Every cell a free
    /// fluid ([`is_fluid`]) occupies, plus the reach [`flood`] gives it. Disjoint
    /// from every block set — a waterlogged cell is its host block's class, not
    /// this.
    pub flooded: CellSet,
    /// For each `solid` cell whose walkable top face is **below** the cell top,
    /// that height in sixteenths of a block. Absent = a full cube
    /// (16/16). Drives the nav step rule's true rise between two standing cells.
    pub partial: CellMap<u8>,
    /// Cells holding a block that **can** hold water ([`is_waterloggable`]),
    /// whatever its authored state. Not a collision class and not disjoint from
    /// the others: it is a second question about the same cells, asked because a
    /// world generator that puts water where these blocks land turns every one of
    /// them into a source (`crate::compiler::nav::measure_sea_seepage`, `DW0851`).
    /// Under a horizon with no ambient water nothing reads it.
    pub waterloggable: CellSet,
    /// The subset of `flooded` that lava reaches rather than water — the same
    /// flood run from the lava sources alone. A body stands on neither, which is
    /// why every other reader takes `flooded` whole; the one that asks where a
    /// body can FLOAT has to tell them apart.
    pub lava: CellSet,
    /// Every climbable the world **keeps** (spec-0099), with what keeps it — a
    /// ladder, a vine, a weeping, twisting or cave vine whose `canSurvive` rule
    /// holds over this block map, transitively ([`climb_holds`]). Not a
    /// collision class and disjoint from all of them: the cell is empty to a
    /// body, and what this adds is that a body in it climbs.
    pub climb: BTreeMap<[i32; 3], ClimbHold>,
    /// Every climbable the block map holds and the world does **not** keep: the
    /// game removes it at the first shape update it receives, so the model reads
    /// its cell as air. Carried, with the block, so a route that needed it can
    /// name it (`DW0991`) rather than report an unroutable leg over a ladder the
    /// author can see in the piece.
    pub unheld_climb: BTreeMap<[i32; 3], String>,
    /// The blocks that change how a body moves on them or through them
    /// ([`delvewright_dsl::metrics::body::Material`]): ice, slime, honey, soul
    /// sand, hay, beds and cobwebs. Not a collision class: the movement
    /// simulator the reach graph runs reads it (spec-0113). A block absent here
    /// is [`delvewright_dsl::metrics::body::Material::Plain`].
    pub material: CellMap<delvewright_dsl::metrics::body::Material>,
    /// For each `solid` cell whose block does **not** cover its cell, the
    /// block's server footprint ([`delvewright_dsl::blockshape::server_footprint`]):
    /// the posts of iron bars, a pane, a chain, a lantern. Not a collision class
    /// and a subset of `solid`: the cell still stops a body walking into it, and
    /// whether a body over it is held or falls between its boxes is the nav
    /// model's to judge from these and the cells around them.
    pub footprints: BTreeMap<[i32; 3], delvewright_dsl::blockshape::Footprint16>,
    /// Cells whose block **hides** what stands behind it from the eye
    /// ([`delvewright_dsl::blockshape::hides`], the pinned jar's occlusion
    /// table). Not a collision class and not disjoint from them: a second
    /// question about the same cells, asked by every sight walk
    /// ([`crate::compiler::nav::World::hides`]). Iron bars are `solid` and not
    /// here; a stone wall is both.
    pub hiding: CellSet,
    /// Every door and trapdoor cell, with the shape its leaf takes per state
    /// ([`delvewright_dsl::blockshape::leaf`]). Not a collision class and not
    /// disjoint from them: the cell's class says whether a body may be in it,
    /// and this says which of its faces a body may cross, and at what height
    /// ([`delvewright_dsl::blockshape::Leaf::stops_crossing`]).
    pub leaves: BTreeMap<[i32; 3], delvewright_dsl::blockshape::Leaf>,
    /// Cells whose block **hurts a body that meets it**
    /// ([`delvewright_dsl::blockshape::hurts_body`]: a campfire, magma, a
    /// cactus, fire, a berry bush). Not a collision class and not disjoint from
    /// them: a campfire is a partial floor and hurts. Read by one question, where
    /// a body may be put down by declaration
    /// ([`crate::compiler::nav::World::is_safe_seat`]); routing never reads it.
    pub hurting: CellSet,
}

/// **What keeps one climbable where it is** (spec-0099 §3.2), as the cells its
/// `canSurvive` rule reads. Any one alternative holding is enough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClimbHold {
    /// The block state, as the map holds it.
    pub block: String,
    /// What kind of climbable it is ([`delvewright_dsl::blockshape::climbable`]).
    pub kind: delvewright_dsl::blockshape::Climbable,
    /// The alternatives, any one of which keeps it: `(face, hold)`, the face
    /// being the vine face the hold serves (`None` for every other kind).
    pub holds: Vec<(Option<delvewright_dsl::blockshape::Face>, Hold)>,
}

/// One thing a climbable hangs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// A block whose face toward the climbable is full: it holds as long as the
    /// block is there.
    Block([i32; 3]),
    /// Another climbable of the same plant (or, for a vine face, the vine above
    /// carrying the same face): it holds as long as that one is kept.
    Climb([i32; 3]),
}

impl ClimbHold {
    /// The way the climbable faces, for a ladder — what a body climbing it
    /// pushes against is the block behind it, on the opposite side.
    pub fn ladder_facing(&self) -> Option<delvewright_dsl::blockshape::Face> {
        match self.kind {
            delvewright_dsl::blockshape::Climbable::Ladder { facing } => Some(facing),
            _ => None,
        }
    }
}

/// **Which climbables a block map keeps** (spec-0099 §3.2): every cell whose
/// block is [`Collision::Climbable`], split into those whose pinned
/// `canSurvive` rule holds — read transitively, to the fixed point the game's
/// own shape updates cascade to — and those whose rule does not.
///
/// The rules, each **cited** from the pinned jar (`docs/specs/spec-0099`):
///
/// * `ladder[facing=F]` — the block behind it answers `isFaceSturdy(F)`
///   ([`delvewright_dsl::blockshape::face_is_sturdy`]);
/// * `vine` — each face it sets is held by a neighbour whose facing face is full
///   to `MultifaceBlock.canAttachTo` (sturdy, or a full collision face), or by
///   the vine above carrying the same face; it is kept while any face is held.
///   The `up` face is **not** counted, which can only drop a vine the game keeps;
/// * a weeping, twisting or cave vine — the cell it grows from is the same
///   plant, or answers `isFaceSturdy(growth)`.
///
/// The chains are read to their fixed point rather than one step: the game's
/// own check reads only the neighbour, and a chain whose top has nothing to hang
/// on survives until a shape update reaches it and then falls a block at a time.
/// A model that credited it would credit a climb the first update removes.
pub fn climb_holds(
    blocks: &BlockMap,
) -> (BTreeMap<[i32; 3], ClimbHold>, BTreeMap<[i32; 3], String>) {
    climb_holds_with(blocks, &mut StateClasses::default())
}

/// [`climb_holds`], reading each state's collision class from `classes`.
fn climb_holds_with(
    blocks: &BlockMap,
    classes: &mut StateClasses,
) -> (BTreeMap<[i32; 3], ClimbHold>, BTreeMap<[i32; 3], String>) {
    use delvewright_dsl::blockshape::{Climbable, Face, climbable, face_is_full, face_is_sturdy};
    let add = |c: [i32; 3], d: [i32; 3]| [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
    let name_at = |c: [i32; 3]| blocks.get(&c).map(|b| &**b).unwrap_or("minecraft:air");
    let mut cand: BTreeMap<[i32; 3], ClimbHold> = BTreeMap::new();
    for (cell, name) in blocks {
        if classes.of(*name).collision != Collision::Climbable {
            continue;
        }
        let Some(kind) = climbable(name) else {
            continue;
        };
        let mut holds = Vec::new();
        match kind {
            Climbable::Ladder { facing } => {
                let behind = add(cell, facing.opposite().offset());
                if face_is_sturdy(name_at(behind), facing) {
                    holds.push((None, Hold::Block(behind)));
                }
            }
            Climbable::Vine { faces } => {
                for (f, set) in Face::HORIZONTAL.into_iter().zip(faces) {
                    if !set {
                        continue;
                    }
                    let n = add(cell, f.offset());
                    let nb = name_at(n);
                    if face_is_sturdy(nb, f.opposite()) || face_is_full(nb, f.opposite()) {
                        holds.push((Some(f), Hold::Block(n)));
                    }
                    let above = add(cell, Face::Up.offset());
                    if matches!(
                        climbable(name_at(above)),
                        Some(Climbable::Vine { faces: up }) if up[Face::HORIZONTAL.iter().position(|h| *h == f).expect("horizontal")]
                    ) {
                        holds.push((Some(f), Hold::Climb(above)));
                    }
                }
            }
            Climbable::Growing(plant) => {
                let from = add(cell, plant.growth().opposite().offset());
                let fb = name_at(from);
                if plant
                    .members()
                    .contains(&delvewright_dsl::blockshape::base_id(fb))
                {
                    holds.push((None, Hold::Climb(from)));
                } else if face_is_sturdy(fb, plant.growth()) {
                    holds.push((None, Hold::Block(from)));
                }
            }
            Climbable::Scaffolding => continue,
        }
        cand.insert(
            cell,
            ClimbHold {
                block: name.to_string(),
                kind,
                holds,
            },
        );
    }
    let kept = keep_climbs(cand.keys().copied().collect(), &cand, &BTreeSet::new());
    let mut held = BTreeMap::new();
    let mut unheld = BTreeMap::new();
    for (c, h) in cand {
        if kept.contains(&c) {
            held.insert(c, h);
        } else {
            unheld.insert(c, h.block);
        }
    }
    (held, unheld)
}

/// The climbables of `alive` that stay kept, to the fixed point, when the
/// blocks in `gone` are removed. A climbable is kept while one of its holds is
/// alive: a `Block` hold while its cell is not in `gone`, a `Climb` hold while
/// the climbable it names is kept — and, for a vine, kept ON THE SAME FACE, which
/// is why the fixed point runs over `(cell, face)` pairs rather than cells. The
/// one fixed point both the assembled world ([`climb_holds`]) and a runtime clear
/// (`nav::World::with_cleared`) take.
pub fn keep_climbs(
    alive: BTreeSet<[i32; 3]>,
    holds: &BTreeMap<[i32; 3], ClimbHold>,
    gone: &BTreeSet<[i32; 3]>,
) -> BTreeSet<[i32; 3]> {
    type Key = ([i32; 3], Option<delvewright_dsl::blockshape::Face>);
    let mut live: BTreeSet<Key> = alive
        .iter()
        .filter(|c| !gone.contains(*c))
        .filter_map(|c| holds.get(c).map(|h| (c, h)))
        .flat_map(|(c, h)| h.holds.iter().map(move |(f, _)| (*c, *f)))
        .collect();
    loop {
        let next: BTreeSet<Key> = live
            .iter()
            .copied()
            .filter(|(c, f)| {
                holds[c].holds.iter().any(|(hf, hold)| {
                    hf == f
                        && match hold {
                            Hold::Block(b) => !gone.contains(b),
                            Hold::Climb(u) => live.contains(&(*u, *f)),
                        }
                })
            })
            .collect();
        if next == live {
            return live.into_iter().map(|(c, _)| c).collect();
        }
        live = next;
    }
}

/// The nav occupancy of the settled assembled world — see
/// [`Occupancy`] for the collision classes.
///
/// ## Why fluid is modelled, and why as a *superset* of vanilla flow
///
/// A `minecraft:water` block placed by `/place template` does not stay put: vanilla
/// fluid physics spreads it at world-load into neighbouring air cells the prefab
/// left empty. The compile-time model must reflect that, or nav proves a route/seat
/// standable on a cell the game floods — the water analogue of the gravity-despawn
/// divergence. Field case: the `cave-shore` pool floods `[261,66,1]`, a
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
/// the flood must see the post-gravity geometry.
///
/// ## Lava is the same question, so it gets the same answer
///
/// A body no more stands on lava than on water, so [`occupancy_of`] classifies a
/// free-fluid cell by [`is_fluid`] and both fluids land in `flooded`. Vanilla's two
/// fluids differ only in *reach* — overworld lava decays over 3 cells rather than 7
/// and forms no new sources — and the delve ships into an ordinary overworld
/// (a void superflat, not an ultrawarm dimension),
/// so running lava through the water flow above over-marks its spread. That is the
/// permitted direction: the model may call a cell molten that the game leaves dry,
/// and may never call a molten cell floor.
pub fn assembled_occupancy(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Occupancy {
    let assembled = assemble(plan, structures);
    occupancy_over(&assembled.blocks, &assembled.open_gates)
}

/// Pure core of [`assembled_occupancy`]: classify a settled cell→block map into an
/// [`Occupancy`]. `open_gates` is the set of fence-gate cells authored `open=true`
/// (they are passable; closed gates become `use_gates`). Split out so the flood
/// and the collision classes are unit-testable without a [`Plan`].
///
/// Vanilla water flows only into **air** cells, so every block that occupies its
/// cell dams the flood — full cubes, partial floors, fences, walls, and gates
/// (open or closed). The flood barrier set is the union of those cells. Air, thin
/// walkable decoration (carpet, torch, plate) and free fluid are not barriers.
///
/// A free fluid is deliberately **not** in that barrier set: it is a flood source
/// instead, so water spreads through a lava cell rather than being dammed by one.
/// Vanilla turns that meeting into stone or obsidian — a solid the model declines
/// to invent, leaving the cell impassable-and-not-floor, which is the conservative
/// half of the answer.
pub fn occupancy_of(
    blocks: BTreeMap<[i32; 3], String>,
    open_gates: &BTreeSet<[i32; 3]>,
) -> Occupancy {
    occupancy_over(&crate::compiler::blockstate::interned(blocks), open_gates)
}

/// [`occupancy_of`] over a borrowed cell→block map.
///
/// Every cell set is built **in its dense form directly** (ADR-0037 §2,
/// [`CellSet::builder`]): one bit per cell of the box the store's sections
/// span, set as the blocks are walked, finished into the set a `BTreeSet` of
/// the same cells would give; the two maps are section stores from the first
/// cell. No whole-world list or tree of cells stands at this stage.
pub fn occupancy_over(blocks: &BlockMap, open_gates: &BTreeSet<[i32; 3]>) -> Occupancy {
    let (lo, hi) = blocks.section_bounds().unwrap_or(([0; 3], [-1; 3]));
    let mut solid = CellSet::builder(lo, hi);
    let mut tall = CellSet::builder(lo, hi);
    let mut use_gates = CellSet::builder(lo, hi);
    let mut barriers = CellSet::builder(lo, hi);
    let mut sources = CellSet::builder(lo, hi);
    let mut lava_sources = CellSet::builder(lo, hi);
    let mut partial: CellMap<u8> = CellMap::new();
    let mut waterloggable = CellSet::builder(lo, hi);
    let mut footprints: BTreeMap<[i32; 3], delvewright_dsl::blockshape::Footprint16> =
        BTreeMap::new();
    let mut hiding = CellSet::builder(lo, hi);
    let mut hurting = CellSet::builder(lo, hi);
    let mut leaves: BTreeMap<[i32; 3], delvewright_dsl::blockshape::Leaf> = BTreeMap::new();
    let mut material: CellMap<delvewright_dsl::metrics::body::Material> = CellMap::new();
    // Every reading below is a function of the block state alone, so it is
    // made once per distinct state ([`StateClass`]), not once per cell.
    let mut classes = StateClasses::default();
    for (cell, name) in blocks {
        let k = classes.of(*name);
        if k.hurts {
            hurting.insert(cell);
        }
        // A door's or a trapdoor's leaf per state (`blockshape::leaf`): what
        // its panels and its plate do to a body crossing its cell's faces.
        // Whether a hand one stands open is the gate model's own reading, as
        // for any door.
        if let Some(mut h) = k.leaf {
            if h.by_hand {
                h.open = open_gates.contains(&cell);
            }
            leaves.insert(cell, h);
        }
        if k.hides {
            hiding.insert(cell);
        }
        if k.waterloggable {
            waterloggable.insert(cell);
        }
        // A waterlogged block's cell holds a real water source that spreads into
        // its air neighbours — seed the flood from it, then classify
        // the host block normally below (the cell itself stays occupied).
        if k.waterlogged {
            sources.insert(cell);
        }
        if k.material != delvewright_dsl::metrics::body::Material::Plain {
            material.insert(cell, k.material);
        }
        // The classification is not made here: it is
        // [`delvewright_dsl::blockshape::collision_class`], the one rule three
        // crates read. What IS this function's own is the world layer on top of
        // it — which cells dam a flood, which gate was authored open, and the
        // disjointness of the five sets.
        match k.collision {
            Collision::Fluid => {
                // Water AND lava: a body stands on neither, so neither is `solid`.
                // A water-only test dropped lava through to the default and made
                // a lava surface into floor a route proof walks.
                sources.insert(cell);
                if k.lava {
                    lava_sources.insert(cell);
                }
            }
            // A climbable's cell is empty to a body (spec-0099); whether a body in
            // it climbs is `climb_holds`'s to say, below. A ladder dams water —
            // the pinned `FlowingFluid.canHoldAnyFluid` lists `LADDER` among the
            // blocks no fluid enters — and a vine does not: water flows into its
            // cell, and the flooded cell is then no climb.
            Collision::Climbable => {
                if k.ladder {
                    barriers.insert(cell);
                }
            }
            // A pressure plate / tripwire / carpet / candle / torch / thin snow
            // drift is walkable floor decoration, not an obstacle — leave its
            // cell passable so the trap-trigger cell stays on the walkable path
            // (spec-0011), and so a corridor with a torch in it stays a corridor.
            // Air reaches this arm too, and its cell is likewise not a floor.
            Collision::Air | Collision::Thin(_) => {}
            Collision::OpensByHand => {
                barriers.insert(cell);
                if !open_gates.contains(&cell) {
                    use_gates.insert(cell); // closed: passable-with-use
                } // open: a passable threshold (still dams water)
            }
            Collision::TallBarrier => {
                barriers.insert(cell);
                tall.insert(cell);
            }
            class @ (Collision::PartialFloor(_) | Collision::FullCube) => {
                barriers.insert(cell);
                solid.insert(cell);
                if let Collision::PartialFloor(h) = class {
                    partial.insert(cell, h);
                }
                if let Some(fp) = k.footprint {
                    footprints.insert(cell, fp);
                }
            }
        }
    }
    // A waterlogged fluid cell is pushed to `sources` twice; the set holds it
    // once, as the set it replaces did.
    let solid = solid.finish();
    let tall = tall.finish();
    let use_gates = use_gates.finish();
    let barriers = barriers.finish();
    let sources = sources.finish();
    let lava_sources = lava_sources.finish();
    let waterloggable = waterloggable.finish();
    let hiding = hiding.finish();
    let hurting = hurting.finish();
    // The sets stay pairwise disjoint: a waterlogged (or fluid-adjacent) barrier
    // cell is already impassable as its host class, so it must not also appear as
    // free water — `flooded` means "a walker would be in open water here".
    let flooded: CellSet = flood(&barriers, &sources)
        .into_iter()
        .filter(|c| !barriers.contains(c))
        .collect();
    let lava: CellSet = if lava_sources.is_empty() {
        CellSet::new()
    } else {
        flood(&barriers, &lava_sources)
            .into_iter()
            .filter(|c| flooded.contains(c))
            .collect()
    };
    let (climb, unheld_climb) = climb_holds_with(blocks, &mut classes);
    Occupancy {
        solid,
        tall,
        use_gates,
        flooded,
        partial,
        waterloggable,
        lava,
        climb,
        unheld_climb,
        material,
        footprints,
        hiding,
        leaves,
        hurting,
    }
}

/// What [`occupancy_over`] and [`climb_holds`] read of one block state: each
/// field the answer of the one `blockshape` (or registry) rule that names it,
/// asked once per distinct state.
#[derive(Clone, Copy)]
struct StateClass {
    hurts: bool,
    leaf: Option<delvewright_dsl::blockshape::Leaf>,
    hides: bool,
    waterloggable: bool,
    waterlogged: bool,
    material: delvewright_dsl::metrics::body::Material,
    collision: Collision,
    lava: bool,
    ladder: bool,
    /// The server footprint, where the state is a floor and its footprint is
    /// not the full cell.
    footprint: Option<delvewright_dsl::blockshape::Footprint16>,
}

impl StateClass {
    fn of(name: &str) -> StateClass {
        use delvewright_dsl::blockshape as bs;
        let collision = collision_class(name);
        StateClass {
            hurts: bs::hurts_body(name),
            leaf: bs::leaf(name),
            hides: bs::hides(name),
            waterloggable: is_waterloggable(name),
            waterlogged: is_waterlogged(name),
            material: delvewright_dsl::metrics::body::material_of_block(name),
            collision,
            lava: collision == Collision::Fluid && bs::bare_id(name) == "lava",
            ladder: collision == Collision::Climbable
                && matches!(bs::climbable(name), Some(bs::Climbable::Ladder { .. })),
            footprint: match collision {
                Collision::PartialFloor(_) | Collision::FullCube => {
                    let fp = bs::server_footprint(name);
                    (fp != bs::FULL_FOOTPRINT).then_some(fp)
                }
                _ => None,
            },
        }
    }
}

/// The [`StateClass`] of each state met so far, by the state's identity.
#[derive(Default)]
struct StateClasses(std::collections::HashMap<(usize, usize), StateClass>);

impl StateClasses {
    fn of(&mut self, state: BlockState) -> StateClass {
        *self
            .0
            .entry(state.key())
            .or_insert_with(|| StateClass::of(&state))
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
///
/// `pub(crate)` rather than private because the **ambient sea** is a second
/// source of water this world has and the block map does not
/// ([`crate::compiler::nav::measure_sea_seepage`], `DW0851`). That proof seeds this same
/// function from the sea's contact face instead of writing a second physics —
/// one flow model, two seed sets, so a change to decay or to source formation
/// cannot leave the two disagreeing about the same room.
pub(crate) fn flood(solid: &CellSet, sources: &CellSet) -> BTreeSet<[i32; 3]> {
    if sources.is_empty() {
        return BTreeSet::new();
    }
    let pooled = form_sources(solid, sources);
    spread(solid, &pooled)
}

/// Whether cell `c` has support below (a solid floor or standing water) — the
/// condition under which vanilla water pools/forms a source instead of flowing down.
fn supported_below(solid: &CellSet, sources: &CellSet, c: [i32; 3]) -> bool {
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
fn form_sources(solid: &CellSet, init: &CellSet) -> CellSet {
    use std::collections::VecDeque;
    let mut sources = init.clone();
    // Seed the cascade with the open (air) cells neighbouring a source — the only
    // cells that can newly qualify.
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    for s in init.iter() {
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
fn open_neighbours(solid: &CellSet, sources: &CellSet, c: [i32; 3]) -> Vec<[i32; 3]> {
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
fn spread(solid: &CellSet, sources: &CellSet) -> BTreeSet<[i32; 3]> {
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
    for s in sources.iter() {
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

delvewright_dsl::dw_code! {
    /// `DW0313`: one or more placed gravity blocks despawn into the void at placement.
    /// A gravity floor (`sand`/`gravel`/…) laid unsupported over the delve's void
    /// world falls out of the world on the first block update, silently deforming the
    /// shipped map (holes, light leaks, visual damage) even where no critical path or
    /// wave seat happens to cross it — so DW0311/DW0312 alone would let it ship green.
    /// This is the authoritative, direct gate: no DSL verb can intend a despawn, so it
    /// is always a prefab/generator defect.
    pub const DW_GRAVITY_DESPAWN: DwCode = DwCode::new("DW0313", ExitTier::Analysis);
}

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
    gravity_despawn_error_of(plan, &assemble(plan, structures))
}

/// [`gravity_despawn_error`] over a world the caller has already assembled.
pub fn gravity_despawn_error_of(plan: &Plan, assembled: &Assembled) -> Option<String> {
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
/// attribution and rubric wording are unit-testable without a full [`Plan`].
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
        entry.1.insert(bare_id(&s.block).to_string());
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
         void world, so a gravity block ({kinds_hint}) with no solid block directly beneath it \
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

    /// [`settle`] over a map of owned strings, as the fixtures here write them.
    fn settle_strings(blocks: &mut BTreeMap<[i32; 3], String>) -> Vec<Settled> {
        let mut map = crate::compiler::blockstate::interned(std::mem::take(blocks));
        let out = settle(&mut map);
        *blocks = map.into_iter().map(|(c, s)| (c, s.to_string())).collect();
        out
    }

    /// A one-cell-thick gate anchor at `z`, spanning x 0..=1, y 64..=65.
    fn gate_anchors(block: &str) -> BTreeMap<(String, String), ResolvedAnchor> {
        let mut a = BTreeMap::new();
        a.insert(
            ("area/keep".to_string(), "anchor/door".to_string()),
            ResolvedAnchor::Gate {
                from: [0, 64, 6],
                to: [1, 65, 6],
                block: block.to_string(),
            },
        );
        a
    }

    /// **Both shipped gates are real, and only a measurement tells them apart.**
    /// `hello-room`'s barred doorway is authored solid; `island-mountain`'s cave
    /// mouth is authored air and only a `close-gate` ever fills it. Assuming
    /// either one is how the model got the other wrong.
    #[test]
    fn a_gate_is_sealed_or_open_by_what_the_world_puts_in_it() {
        let anchors = gate_anchors("minecraft:iron_bars");
        let open = measure_gate_seals(&anchors, &BlockMap::new());
        assert_eq!(open.len(), 1, "the gate is examined either way");
        assert_eq!(open[0].cells, 4);
        assert_eq!(open[0].blocked, 0);
        assert!(!open[0].sealed(), "an empty gate region is authored OPEN");

        let mut blocks = BTreeMap::new();
        for cell in region_cells([0, 64, 6], [1, 65, 6]) {
            blocks.insert(cell, "minecraft:iron_bars".to_string());
        }
        let shut = measure_gate_seals(
            &anchors,
            &crate::compiler::blockstate::interned(blocks.clone()),
        );
        assert_eq!(shut[0].blocked, 4);
        assert!(shut[0].sealed(), "a filled gate region is authored SHUT");
        assert_eq!(shut[0].foreign, 0);
        assert_eq!(shut[0].anchor, "anchor/door");
        assert_eq!(shut[0].area, "area/keep");
    }

    /// Air by any of its three spellings is not a seal, and a blockstate is the
    /// same block: `iron_bars[east=true]` is what a placed doorway really holds.
    #[test]
    fn air_is_not_a_seal_and_a_blockstate_is_the_same_block() {
        let anchors = gate_anchors("minecraft:iron_bars");
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 6], "minecraft:cave_air".to_string());
        blocks.insert([1, 64, 6], "minecraft:void_air".to_string());
        blocks.insert(
            [0, 65, 6],
            "minecraft:iron_bars[east=true,west=true]".to_string(),
        );
        let m = measure_gate_seals(
            &anchors,
            &crate::compiler::blockstate::interned(blocks.clone()),
        );
        assert_eq!(m[0].blocked, 1, "only the bars count");
        assert_eq!(
            m[0].foreign, 0,
            "a blockstate is the declared block, not a foreign one"
        );
    }

    /// **The residue an `open-gate` will not clear.** The emitted fill is
    /// `replace`-filtered to the anchor's declared block, so a cell holding
    /// anything else survives the opening. `cave-mouth.nbt` really does author
    /// five `mossy_cobblestone` cells inside a gate declaring `cobblestone`; the
    /// count is reported rather than silently modelled away.
    #[test]
    fn a_block_the_gate_does_not_declare_is_counted_as_foreign() {
        let anchors = gate_anchors("minecraft:cobblestone");
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 6], "minecraft:cobblestone".to_string());
        blocks.insert([1, 64, 6], "minecraft:mossy_cobblestone".to_string());
        let m = measure_gate_seals(
            &anchors,
            &crate::compiler::blockstate::interned(blocks.clone()),
        );
        assert_eq!(m[0].blocked, 2);
        assert_eq!(m[0].foreign, 1);
    }

    /// The ledger says what it examined, and calls a zero a zero.
    #[test]
    fn the_ledger_reports_an_unbound_measurement_as_unbound() {
        let anchors = gate_anchors("minecraft:iron_bars");
        let seals = measure_gate_seals(&anchors, &BlockMap::new());
        let j = gate_seal_ledger(&seals, 0);
        assert_eq!(j["gates_examined"], 1);
        assert_eq!(j["sealed_at_world_load"], 0);
        assert_eq!(j["modelled_as_sealed"], 0);
        assert_eq!(j["unbound"], true);
    }

    #[test]
    fn unsupported_falling_block_over_void_despawns() {
        // A single layer of sand at y=64 over void — the cave-den floor field bug:
        // every unsupported sand cell falls out of the world.
        let mut blocks = BTreeMap::new();
        blocks.insert([0, 64, 0], "minecraft:sand".to_string());
        blocks.insert([1, 64, 0], "minecraft:sand".to_string());
        settle_strings(&mut blocks);
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
        settle_strings(&mut blocks);
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
        settle_strings(&mut blocks);
        assert!(blocks.contains_key(&[0, 64, 0]));
        assert!(blocks.contains_key(&[0, 65, 0]));
        assert!(blocks.contains_key(&[0, 66, 0]));
        assert!(!blocks.contains_key(&[0, 67, 0]));
        assert!(!blocks.contains_key(&[0, 68, 0]));
    }

    // --- water flood model ---

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
        settle_strings(&mut b); // sand falls from 66 → 65 (onto the floor at 64)
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
        settle_strings(&mut b);
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

    // --- lava is a fluid, so it is not floor ---

    /// The static half of the fluid-is-not-floor defect. A prefab cell of
    /// `minecraft:lava` reached `occupancy_of`'s final `else` — because the branch
    /// that catches a fluid asked `is_water` — and came out `solid`: full-cube
    /// floor a route proof stands the party on.
    ///
    /// The counter-case is the same map with the block swapped for `stone`, so the
    /// only variable between the two assertions is the block id.
    #[test]
    fn a_lava_cell_is_flooded_and_never_solid() {
        let occ_of = |block: &str| {
            let mut b = floor(64, 0, 0, 0, 0);
            b.insert([0, 65, 0], block.to_string());
            occupancy_of(b, &BTreeSet::new())
        };
        let lava = occ_of("minecraft:lava");
        assert!(
            !lava.solid.contains(&[0, 65, 0]),
            "a body does not stand on lava, so its cell is not floor"
        );
        assert!(
            lava.flooded.contains(&[0, 65, 0]),
            "and it is impassable, which is what `flooded` means"
        );
        // Counter-case: identical geometry, a solid block.
        let stone = occ_of("minecraft:stone");
        assert!(
            stone.solid.contains(&[0, 65, 0]) && !stone.flooded.contains(&[0, 65, 0]),
            "the same cell holding stone is still ordinary floor — the block is the \
             only variable"
        );
    }

    /// The namespace is not part of the question, at the static site as at the
    /// runtime one: the DSL's block registry accepts a bare `lava`, so a hand-written
    /// block id can reach the model without `minecraft:` on it.
    #[test]
    fn a_bare_namespace_lava_cell_is_flooded_too() {
        let mut b = floor(64, 0, 0, 0, 0);
        b.insert([0, 65, 0], "lava".to_string());
        let occ = occupancy_of(b, &BTreeSet::new());
        assert!(!occ.solid.contains(&[0, 65, 0]));
        assert!(occ.flooded.contains(&[0, 65, 0]));
    }

    /// Lava spreads, so a lava cell is a flood **source**, not merely a cell of
    /// its own — and the reach it gets is water's, which over-marks (overworld
    /// lava decays over 3). Over-marking is the only permitted direction.
    #[test]
    fn a_lava_source_floods_its_reach_like_water() {
        let mut b = floor(64, -10, 10, 0, 0);
        b.insert([0, 65, 0], "minecraft:lava".to_string());
        let flooded = occupancy_of(b, &BTreeSet::new()).flooded;
        for x in -7..=7 {
            assert!(
                flooded.contains(&[x, 65, 0]),
                "x={x} is within lava's reach"
            );
        }
        assert!(
            !flooded.contains(&[8, 65, 0]),
            "and the decay still ends at 7"
        );
    }

    /// The general form, bound to the layer that decides what an author may write.
    ///
    /// The DSL's block registry accepts every block, item form or not; these are
    /// the air and fluid ids a `fill`/`set-block` may name.
    /// Every one of them must reach a class `occupancy_of` handles **on purpose**:
    /// an air variant (no cell) or a fluid (flooded). An id that is neither gets
    /// the collision table's full-cube default and becomes floor, which is how
    /// `minecraft:lava` shipped as standable ground — so a sixth technical block
    /// added to that list reds here rather than being discovered in a delve.
    ///
    /// It examines the list's own spellings, which are namespaced. `is_fluid` is
    /// namespace-insensitive and is covered bare above; `is_air` is not, and what
    /// that costs is a separate question from this one.
    #[test]
    fn every_technical_block_the_dsl_accepts_is_air_or_fluid_to_this_model() {
        use delvewright_dsl::registry::{BlockRegistry, VendoredBlockRegistry};
        const TECHNICAL_BLOCK_IDS: &[&str] = &[
            "minecraft:air",
            "minecraft:cave_air",
            "minecraft:void_air",
            "minecraft:water",
            "minecraft:lava",
        ];
        let mut fluids = 0usize;
        for id in TECHNICAL_BLOCK_IDS {
            assert!(
                VendoredBlockRegistry.contains(id),
                "`{id}` is not in the DSL's block registry"
            );
            assert!(
                is_air(id) || is_fluid(id),
                "`{id}` is a block the DSL accepts but the collision table gives \
                 its full-cube default, i.e. floor"
            );
            if is_fluid(id) {
                fluids += 1;
                let mut b = floor(64, 0, 0, 0, 0);
                b.insert([0, 65, 0], (*id).to_string());
                let occ = occupancy_of(b, &BTreeSet::new());
                assert!(!occ.solid.contains(&[0, 65, 0]), "`{id}` became floor");
                assert!(occ.flooded.contains(&[0, 65, 0]), "`{id}` is not flooded");
            }
        }
        // Binding count (playtest-methodology rule 1): a zero here would be a green
        // that examined nothing.
        assert_eq!(
            (TECHNICAL_BLOCK_IDS.len(), fluids),
            (5, 2),
            "the DSL's technical-block list changed shape; re-derive this proof"
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
        // The `cave-shore`/perimedes field case, reduced: a sand shore at
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
        settle_strings(&mut blocks);
        assert_eq!(blocks, before);
    }

    #[test]
    fn cave_den_field_bug_mixed_floor_keeps_only_the_non_falling_cells() {
        // The exact field shape: a single-layer cave floor over void,
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
        settle_strings(&mut blocks);
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

    // --- collision classes ---

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
        let outcomes = settle_strings(&mut blocks);
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
        let settled = settle_strings(&mut blocks);
        let pieces = [("prefab/test-den", ([0, 64, 0], [4, 64, 0]))];
        let msg = despawn_message(&settled, &pieces).expect("despawn must be flagged");
        // WHAT: count + kind; WHERE: the piece; HOW + anti-dodge clause.
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
        let ok_settled = settle_strings(&mut ok);
        assert!(
            despawn_message(&ok_settled, &pieces).is_none(),
            "supported floor is clean"
        );
    }

    // --- waterlogging is water ---

    #[test]
    fn a_waterlogged_block_seeds_the_flood_like_a_free_source() {
        // MC 1.13+: a waterlogged block's cell holds a real water source that
        // spreads into adjacent air. A model that stores bare block ids cannot
        // see it, and under-marks the flood — the one direction the
        // never-under-mark contract forbids.
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

    // --- gravity blocks sink through fluids ---

    #[test]
    fn a_falling_block_sinks_through_water_and_displaces_it() {
        // Vanilla `FallingBlock.isFree` counts a fluid as free space: sand dropped
        // over a pool falls THROUGH the water and lands on the rock beneath,
        // replacing the water cell it rests in. Treating that water cell as an
        // immovable support floats the sand on the surface — a phantom floor
        // that dams the flood and that nav then walks on.
        let mut b = BTreeMap::new();
        b.insert([0, 63, 0], "minecraft:stone".to_string()); // pool floor
        b.insert([0, 64, 0], "minecraft:water".to_string()); // 2-deep water
        b.insert([0, 65, 0], "minecraft:water".to_string());
        b.insert([0, 67, 0], "minecraft:sand".to_string()); // dropped in
        let outcomes = settle_strings(&mut b);
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
        let outcomes = settle_strings(&mut b);
        assert!(!b.contains_key(&[0, 66, 0]));
        assert!(!b.contains_key(&[0, 65, 0]));
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].to, None, "no fluid is a support: {outcomes:?}");
    }

    // --- partial collision heights ---

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

    /// Vanilla no-collision vegetation has an EMPTY collision shape.
    /// Modelling it as a full cube creates phantom standable cells — a tuft
    /// splits a valley terrace's 2-block riser into two climbable 1-block
    /// steps, which is a false `DW0854` — and lets a walkability proof stand a
    /// body on a flower, which is unsound. The class is pinned here so no
    /// future palette id regresses to the full-cube fallback silently.
    #[test]
    fn no_collision_plants_have_an_empty_collision_shape() {
        for id in [
            "minecraft:short_grass",
            "minecraft:tall_grass",
            "minecraft:fern",
            "minecraft:large_fern",
            "minecraft:pink_petals",
            "minecraft:poppy",
            "minecraft:oxeye_daisy",
            "minecraft:cornflower",
            "minecraft:dandelion",
            "minecraft:dead_bush",
            "minecraft:oak_sapling",
            "minecraft:cherry_sapling",
            "minecraft:wheat[age=7]",
            "minecraft:sweet_berry_bush",
            "minecraft:vine",
            "minecraft:glow_lichen",
            "minecraft:seagrass",
            "minecraft:sugar_cane",
        ] {
            assert_eq!(collision_top_16(id), 0, "{id} must have no collision");
            assert!(is_thin_decoration(id), "{id} is walked through");
        }
        // The lookalikes that DO collide keep a collision box a body cannot
        // walk through: a full cube, or the jar's own floor height (a cactus
        // at 15, an upward dripstone tip at 11).
        for id in [
            "minecraft:azalea",
            "minecraft:big_dripleaf",
            "minecraft:bamboo",
            "minecraft:cactus",
            "minecraft:pointed_dripstone",
            "minecraft:oak_leaves[persistent=true]",
            "minecraft:sea_pickle",
        ] {
            assert!(
                collision_top_16(id) >= delvewright_dsl::blockshape::THIN_HEIGHT_16,
                "{id} must keep collision"
            );
            assert!(!is_thin_decoration(id), "{id} is not walked through");
        }
    }

    /// The occupancy consequence of the class: a tuft/flower/petal cell is
    /// passable air for the walker — the standable surface is the block BELOW
    /// it, never the plant itself.
    #[test]
    fn plants_are_not_floors_and_not_obstacles() {
        let mut b = floor(63, 0, 2, 0, 0);
        b.insert([0, 64, 0], "minecraft:short_grass".to_string());
        b.insert([1, 64, 0], "minecraft:pink_petals".to_string());
        b.insert([2, 64, 0], "minecraft:fern".to_string());
        let occ = occupancy_of(b, &BTreeSet::new());
        for c in [[0, 64, 0], [1, 64, 0], [2, 64, 0]] {
            assert!(
                !occ.solid.contains(&c) && !occ.tall.contains(&c),
                "{c:?} must be passable — a plant is not an obstacle"
            );
            assert!(
                !occ.partial.contains_key(&c),
                "{c:?} must not be a floor level of its own"
            );
        }
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

    /// **A lit corridor is a corridor** (spec-0056).
    ///
    /// Torches, signs, banners, levers, buttons, rails, redstone wire, candles
    /// and flower pots are all `noCollission()` or under the auto-step in
    /// vanilla, and every one of them classified as a full solid cube here until
    /// spec-0056 moved the table into `delvewright_dsl::blockshape`. A torch in a
    /// two-high corridor therefore walled it, and `DW0311` refused the route.
    ///
    /// This is the assertion that answers differently if a value in that table is
    /// wrong: the cells must be neither `solid` nor `partial`, which is a claim
    /// about the classification and not about whether the ids were listed.
    #[test]
    fn a_lit_corridor_is_walkable_and_a_lantern_is_still_conservative() {
        let lit = [
            "minecraft:torch",
            "minecraft:wall_torch[facing=north]",
            "minecraft:soul_torch",
            "minecraft:redstone_wall_torch[facing=east,lit=true]",
            "minecraft:white_candle[candles=2,lit=true,waterlogged=false]",
            "minecraft:oak_sign[rotation=0,waterlogged=false]",
            "minecraft:oak_wall_sign[facing=north,waterlogged=false]",
            "minecraft:white_wall_banner[facing=north]",
            "minecraft:lever[face=wall,facing=north,powered=false]",
            "minecraft:stone_button[face=wall,facing=north,powered=false]",
            "minecraft:rail[shape=north_south]",
            "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]",
            "minecraft:flower_pot",
            "minecraft:potted_cactus",
        ];
        let n = lit.len() as i32;
        let mut b = floor(63, 0, n - 1, 0, 0);
        for (i, name) in lit.iter().enumerate() {
            b.insert([i as i32, 64, 0], (*name).to_string());
        }
        let occ = occupancy_of(b, &BTreeSet::new());
        for (i, name) in lit.iter().enumerate() {
            let c = [i as i32, 64, 0];
            assert!(
                !occ.solid.contains(&c) && !occ.tall.contains(&c) && !occ.use_gates.contains(&c),
                "{name} walls the corridor it is lighting"
            );
            assert!(
                !occ.partial.contains_key(&c),
                "{name} is not a floor level of its own — a body rests on the block below"
            );
        }
        // Binding, computed from the objects rather than written beside them.
        assert_eq!(
            occ.solid.len(),
            lit.len(),
            "only the floor under them is solid"
        );

        // The other direction, and it is what keeps this honest: a lantern is a
        // floor at its measured top, never a passage. A ladder is a passage and
        // never a floor (spec-0099) — and this one, facing north with nothing
        // south of it to hang on, is not even a climb: the world does not keep it.
        let mut b = floor(63, 0, 1, 0, 0);
        b.insert([0, 64, 0], "minecraft:lantern[hanging=false]".to_string());
        b.insert([1, 64, 0], "minecraft:ladder[facing=north]".to_string());
        let occ = occupancy_of(b, &BTreeSet::new());
        assert!(occ.solid.contains(&[0, 64, 0]));
        assert!(!occ.solid.contains(&[1, 64, 0]) && !occ.climb.contains_key(&[1, 64, 0]));
        assert!(occ.unheld_climb.contains_key(&[1, 64, 0]));
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
        let outcomes = settle_strings(&mut blocks);
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
    use crate::compiler::solver::Rotation;

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

#[cfg(test)]
mod typed_read_tests {
    use super::{cells_of, decode_typed, decode_walk, structure_size};
    use fastnbt::Value;
    use std::collections::HashMap;

    fn compound(entries: Vec<(&str, Value)>) -> Value {
        Value::Compound(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn ints(v: &[i32]) -> Value {
        Value::List(v.iter().map(|&n| Value::Int(n)).collect())
    }

    fn block(pos: Value, state: Value) -> Value {
        compound(vec![("pos", pos), ("state", state)])
    }

    /// A template with every part a vanilla file carries.
    fn full(blocks: Vec<Value>, extra_root: Vec<(&str, Value)>) -> Value {
        let palette = Value::List(vec![
            compound(vec![("Name", Value::String("minecraft:air".into()))]),
            compound(vec![
                ("Name", Value::String("minecraft:oak_fence_gate".into())),
                (
                    "Properties",
                    compound(vec![
                        ("open", Value::String("true".into())),
                        ("facing", Value::String("north".into())),
                        ("powered", Value::String("false".into())),
                    ]),
                ),
            ]),
            compound(vec![("Name", Value::String("minecraft:stone".into()))]),
            compound(vec![]),
            compound(vec![
                ("Name", Value::String("minecraft:lantern".into())),
                ("Properties", Value::Int(3)),
            ]),
        ]);
        let mut root = vec![
            ("DataVersion", Value::Int(4671)),
            ("size", ints(&[4, 2, 3])),
            ("palette", palette),
            ("blocks", Value::List(blocks)),
            ("entities", Value::List(vec![])),
        ];
        root.extend(extra_root);
        compound(root)
    }

    fn ordinary_blocks() -> Vec<Value> {
        vec![
            block(ints(&[0, 0, 0]), Value::Int(0)),
            block(ints(&[1, 0, 0]), Value::Int(1)),
            block(ints(&[2, 1, 0]), Value::Int(2)),
            block(ints(&[3, 1, 2]), Value::Int(3)),
            block(ints(&[3, 0, 2]), Value::Int(4)),
            block(ints(&[3, 0, 1]), Value::Int(-1)),
            block(ints(&[3, 0, 0]), Value::Int(99)),
            block(ints(&[1, 1]), Value::Int(2)),
            compound(vec![
                ("pos", ints(&[0, 1, 2])),
                ("state", Value::Int(2)),
                ("nbt", compound(vec![("id", Value::String("x".into()))])),
            ]),
        ]
    }

    /// Whatever the typed decoding accepts, it reads exactly as the dynamic
    /// walk does; whatever it refuses falls through to the walk. The ordinary
    /// file must take the typed path, or the comparison binds to nothing.
    #[test]
    fn the_typed_read_agrees_with_the_walk_or_refuses() {
        let cases: Vec<(&str, Value, bool)> = vec![
            ("ordinary", full(ordinary_blocks(), vec![]), true),
            (
                "byte state",
                full(vec![block(ints(&[0, 0, 0]), Value::Byte(2))], vec![]),
                false,
            ),
            (
                "long position",
                full(
                    vec![block(
                        Value::List(vec![Value::Long(0), Value::Long(0), Value::Long(0)]),
                        Value::Int(2),
                    )],
                    vec![],
                ),
                false,
            ),
            (
                "int-array position",
                full(
                    vec![block(
                        Value::IntArray(fastnbt::IntArray::new(vec![0, 0, 0])),
                        Value::Int(2),
                    )],
                    vec![],
                ),
                false,
            ),
            (
                "unknown root key",
                full(
                    ordinary_blocks(),
                    vec![("author", Value::String("a".into()))],
                ),
                false,
            ),
            (
                "non-compound block",
                full(
                    vec![Value::Int(7), block(ints(&[0, 0, 0]), Value::Int(2))],
                    vec![],
                ),
                false,
            ),
            (
                "no palette",
                compound(vec![("blocks", Value::List(ordinary_blocks()))]),
                true,
            ),
        ];
        for (what, root, typed) in cases {
            let raw = fastnbt::to_bytes(&root).expect("serialize NBT");
            for stateful in [false, true] {
                let fast = decode_typed(&raw, stateful).map(|d| cells_of(&d));
                assert_eq!(fast.is_some(), typed, "{what}: typed path taken");
                let walked = cells_of(&decode_walk(&raw, stateful));
                assert_eq!(
                    fast.unwrap_or_else(|| walked.clone()),
                    walked,
                    "{what} (stateful {stateful})"
                );
            }
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            std::io::Write::write_all(&mut gz, &raw).expect("gzip");
            let bytes = gz.finish().expect("gzip finish");
            let walked_size = match fastnbt::from_bytes::<Value>(&raw) {
                Ok(Value::Compound(r)) => crate::compiler::nbtread::size_of(r.get("size")),
                _ => None,
            };
            assert_eq!(structure_size(&bytes), walked_size, "{what}: size");
        }
    }
}

#[cfg(test)]
mod judged_tests {
    use super::*;

    fn world(cells: &[([i32; 3], &str)]) -> Assembled {
        let mut blocks = BlockMap::new();
        for (c, b) in cells {
            blocks.insert(*c, BlockState::new(b));
        }
        Assembled {
            blocks: std::sync::Arc::new(blocks),
            settled: Vec::new(),
            open_gates: BTreeSet::new(),
            gate_seals: Vec::new(),
            reached: Reached::default(),
        }
    }

    /// The store's line states the cells, the sections and the bytes of the
    /// whole and of the judged copy: two cells in two sections, one of them
    /// judged, each section its index array and its palette.
    /// **A template is written section by section** (`by_section`): cells
    /// listed in a scrambled order over several sections come out grouped by
    /// section, the sections in key order, each section's cells in the order
    /// listed — a cell listed twice keeps its two writes in order, so the map
    /// the writes make is the map the listed order makes.
    #[test]
    fn a_template_is_written_section_by_section() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move |n: i32| -> i32 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as i32
        };
        let stone = BlockState::new("minecraft:stone");
        let dirt = BlockState::new("minecraft:dirt");
        let mut listed: Vec<([i32; 3], BlockState, Option<bool>)> = Vec::new();
        for k in 0..3000 {
            let c = [rnd(50) - 20, rnd(40), rnd(50) - 20];
            listed.push((c, if k % 3 == 0 { stone } else { dirt }, None));
        }
        // The same cell twice, with different blocks: the later write wins
        // in both orders only if the sort is stable.
        for k in 0..200 {
            let c = listed[k * 7].0;
            listed.push((c, if k % 2 == 0 { dirt } else { stone }, Some(true)));
        }
        let mut sorted = listed.clone();
        by_section(&mut sorted);
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut last: Option<[i32; 3]> = None;
        for (c, _, _) in &sorted {
            let s = section_key(*c);
            if last != Some(s) {
                assert!(seen.insert(s), "section {s:?} appears in two runs");
                last = Some(s);
            }
        }
        assert!(
            seen.len() > 20,
            "{} section(s): the case binds nothing",
            seen.len()
        );
        let replay = |cells: &[([i32; 3], BlockState, Option<bool>)]| {
            let mut m: BTreeMap<[i32; 3], (BlockState, Option<bool>)> = BTreeMap::new();
            for (c, b, o) in cells {
                m.insert(*c, (*b, *o));
            }
            m
        };
        assert_eq!(replay(&sorted), replay(&listed));
        // The listed order of one cell's writes is kept.
        let twice = listed[0].0;
        let order_in = |cells: &[([i32; 3], BlockState, Option<bool>)]| {
            cells
                .iter()
                .filter(|(c, _, _)| *c == twice)
                .map(|(_, b, o)| (*b, *o))
                .collect::<Vec<_>>()
        };
        assert_eq!(order_in(&sorted), order_in(&listed));
        assert!(
            order_in(&listed).len() >= 2,
            "the doubled cell was listed once"
        );
    }

    /// **A sweep by column writes what write order writes** (ADR-0037 §2,
    /// assembly): sixty generated writes of up to 48 cells a side at
    /// unaligned positions over a 400 × 300 field, overlapping one another,
    /// a cell listed twice in one write, a write listed twice and cells
    /// cleared to air among them, laid by `write_by_section_column` and
    /// write by write; the two maps and the open flags equal, cell for
    /// cell. The sweep writes no section in two steps; the writes it holds
    /// decoded at once are a handful; it decodes a write once per band of
    /// sections along `z` the write crosses and no more. The case refuses
    /// a zero count of overlapping cells and of writes crossing a column
    /// border.
    #[test]
    fn a_sweep_by_column_writes_what_write_order_writes() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut rnd = move |n: i32| -> i32 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as i32
        };
        let names = ["minecraft:stone", "minecraft:dirt", "minecraft:oak_door"];
        type Write = (
            ([i32; 3], [i32; 3]),
            Vec<([i32; 3], Option<BlockState>, Option<bool>)>,
        );
        let mut tiles: Vec<Write> = Vec::new();
        for _ in 0..60 {
            let pos = [rnd(400) - 200, rnd(20) + 40, rnd(300) - 150];
            let size = [rnd(48) + 1, rnd(48) + 1, rnd(48) + 1];
            let mut cells = Vec::new();
            for _ in 0..rnd(600) + 100 {
                let c = [
                    pos[0] + rnd(size[0]),
                    pos[1] + rnd(size[1]),
                    pos[2] + rnd(size[2]),
                ];
                let k = rnd(4) as usize;
                let open = if k == 2 { Some(rnd(2) == 0) } else { None };
                let block = (k < 3).then(|| BlockState::new(names[k]));
                cells.push((c, block, open));
            }
            // A cell listed twice in one write: the later write wins.
            let (c, _, _) = cells[0];
            cells.push((c, Some(BlockState::new(names[1])), None));
            let hi = [
                pos[0] + size[0] - 1,
                pos[1] + size[1] - 1,
                pos[2] + size[2] - 1,
            ];
            tiles.push(((pos, hi), cells));
        }
        // A write listed twice.
        let again = tiles[7].clone();
        tiles.push(again);
        let footprints: Vec<_> = tiles.iter().map(|(f, _)| *f).collect();

        type Written = BTreeMap<[i32; 3], (BlockState, Option<bool>)>;
        let lay = |m: &mut Written, c: [i32; 3], b: Option<BlockState>, o: Option<bool>| match b {
            Some(b) => {
                m.insert(c, (b, o));
            }
            None => {
                m.remove(&c);
            }
        };
        let by_tile: Written = {
            let mut m = Written::new();
            for (_, cells) in &tiles {
                let mut cells = cells.clone();
                by_section(&mut cells);
                for (c, b, o) in cells {
                    lay(&mut m, c, b, o);
                }
            }
            m
        };
        let mut by_column = Written::new();
        let mut steps: Vec<[i32; 3]> = Vec::new();
        let sweep = write_by_section_column(
            &footprints,
            |i| tiles[i].1.clone(),
            |c, b, o| {
                steps.push(section_key(c));
                lay(&mut by_column, c, b, o);
            },
        );
        assert_eq!(by_column, by_tile);

        // The binding: cells two writes lay, and writes crossing a column.
        let mut writers: BTreeMap<[i32; 3], usize> = BTreeMap::new();
        for (_, cells) in &tiles {
            for (c, _, _) in cells {
                *writers.entry(*c).or_insert(0) += 1;
            }
        }
        let overlapping = writers.values().filter(|&&n| n > 1).count();
        assert!(overlapping > 100, "{overlapping} overlapping cell(s)");
        let crossing = footprints
            .iter()
            .filter(|&&(lo, hi)| {
                let (lo, hi) = column_span(lo, hi).unwrap();
                lo != hi
            })
            .count();
        assert!(crossing > 40, "{crossing} write(s) cross a column border");

        // No section is written in two steps.
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut last: Option<[i32; 3]> = None;
        for s in &steps {
            if last != Some(*s) {
                assert!(seen.insert(*s), "section {s:?} written in two steps");
                last = Some(*s);
            }
        }
        assert!(seen.len() > 200, "{} section(s)", seen.len());

        // A write is decoded once per z band it crosses, and no more.
        let bands: usize = footprints
            .iter()
            .map(|&(lo, hi)| {
                let (lo, hi) = column_span(lo, hi).unwrap();
                (hi[0] - lo[0] + 1) as usize
            })
            .sum();
        assert_eq!(sweep.decodes, bands);
        assert!(
            sweep.decodes > sweep.writes,
            "{sweep:?}: no write crosses a band"
        );
        assert_eq!(sweep.writes, tiles.len());
        assert!(
            sweep.held <= 16,
            "{sweep:?}: the writes held grow with the field"
        );
        assert!(sweep.columns > 100, "{sweep:?}");
    }

    #[test]
    fn the_store_line_states_the_sections_and_their_bytes() {
        let a = world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([40, 64, 0], "minecraft:stone"),
        ]);
        assert_eq!(a.blocks.sections(), 2);
        assert!(a.blocks.bytes() >= 2 * 4096);
        let (j, _) = a
            .judged_within([0, 0], [2, 2])
            .expect("one cell lies outside");
        let line = a.store_line(Some(&j));
        assert!(
            line.starts_with(&format!(
                "world store: 2 placed cell(s) in 2 section(s) of 16 x 16 x 16, {} byte(s); the \
                 judged copy {} cell(s) in {} section(s), {} byte(s)",
                a.blocks.bytes(),
                j.blocks.len(),
                j.blocks.sections(),
                j.blocks.bytes()
            )),
            "{line}"
        );
        assert!(a.store_line(None).ends_with("byte(s) (ADR-0037 §1)"));
    }

    /// Nothing outside the columns: the world is judged whole, unchanged.
    #[test]
    fn a_world_inside_its_columns_is_judged_whole() {
        let a = world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([3, 65, 3], "minecraft:stone"),
        ]);
        assert!(a.judged_within([0, 0], [3, 3]).is_none());
    }

    /// Cells past the columns leave; the faces one column out are walled from
    /// the lowest cell to three over the highest; the cells inside stay as
    /// they were.
    #[test]
    fn ground_past_the_columns_leaves_and_a_wall_closes_the_faces() {
        let a = world(&[
            ([0, 64, 0], "minecraft:stone"),
            ([2, 66, 2], "minecraft:grass_block"),
            ([9, 70, 0], "minecraft:stone"),
            ([0, 60, -9], "minecraft:dirt"),
        ]);
        let (j, b) = a.judged_within([0, 0], [2, 2]).expect("cells lie outside");
        assert_eq!((b.kept, b.left_out), (2, 2));
        assert_eq!(
            j.blocks.get(&[0, 64, 0]).map(|s| s.as_str()),
            Some("minecraft:stone")
        );
        assert_eq!(
            j.blocks.get(&[2, 66, 2]).map(|s| s.as_str()),
            Some("minecraft:grass_block")
        );
        assert!(!j.blocks.contains_key(&[9, 70, 0]));
        assert!(!j.blocks.contains_key(&[0, 60, -9]));
        // A ring of 5 x 5 minus the 3 x 3 inside = 16 columns, y 60..=73.
        assert_eq!(b.wall, 16 * 14);
        for c in [
            [-1, 60, 0],
            [3, 73, 1],
            [1, 70, -1],
            [3, 65, 3],
            [-1, 66, -1],
        ] {
            assert_eq!(
                j.blocks.get(&c).map(|s| s.as_str()),
                Some(JUDGED_WALL),
                "{c:?}"
            );
        }
        assert!(!j.blocks.contains_key(&[-1, 74, 0]));
        assert!(!j.blocks.contains_key(&[-1, 59, 0]));
    }

    /// The wall stops a body and leaves the light and the eye alone.
    #[test]
    fn the_wall_is_solid_passes_light_and_hides_nothing() {
        assert!(delvewright_dsl::blockshape::supports_body(JUDGED_WALL));
        assert!(!delvewright_dsl::blockshape::passes_body(JUDGED_WALL));
        assert!(crate::compiler::light::passes_light(JUDGED_WALL));
        let occ = occupancy_of(
            BTreeMap::from([([0, 64, 0], JUDGED_WALL.to_string())]),
            &BTreeSet::new(),
        );
        assert!(occ.hiding.is_empty());
    }

    /// **The settle by section column is the settle in cell order**: generated
    /// worlds of supports, fluids and falling blocks across section borders and
    /// the origin, settled by section column and by the walk in cell order
    /// (`settle_in_cell_order`, the form before ADR-0037), the maps equal cell
    /// for cell and the outcomes equal in order.
    #[test]
    fn the_settle_by_section_column_is_the_settle_in_cell_order() {
        fn settle_in_cell_order(blocks: &mut BlockMap) -> Vec<Settled> {
            let mut columns: BTreeMap<(i32, i32), Vec<i32>> = BTreeMap::new();
            for c in blocks.keys() {
                columns.entry((c[0], c[2])).or_default().push(c[1]);
            }
            let mut outcomes: Vec<((i32, i32), Settled)> = Vec::new();
            for ((x, z), ys) in columns {
                let cells = ys
                    .iter()
                    .map(|y| (*y, *blocks.get(&[x, *y, z]).expect("listed")))
                    .collect();
                settle_column(blocks, x, z, cells, &mut outcomes);
            }
            outcomes.into_iter().map(|(_, s)| s).collect()
        }
        let mut seed = 0x1234_5678_9abc_def1u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let kinds = [
            "minecraft:stone",
            "minecraft:sand",
            "minecraft:gravel",
            "minecraft:water",
        ];
        let mut despawned = 0;
        let mut landed = 0;
        for round in 0..24 {
            let mut m = BlockMap::new();
            let n = 40 + round * 25;
            for _ in 0..n {
                let c = [
                    next(50) as i32 - 25,
                    next(40) as i32 - 8,
                    next(50) as i32 - 25,
                ];
                m.insert(c, BlockState::new(kinds[next(4) as usize]));
            }
            // Columns with several falling blocks over one support, and
            // falling blocks with nothing under them.
            for _ in 0..6 {
                let (x, z) = (next(50) as i32 - 25, next(50) as i32 - 25);
                let base = next(20) as i32 - 4;
                m.insert([x, base, z], BlockState::new("minecraft:stone"));
                for k in 1..=(1 + next(4) as i32) {
                    m.insert([x, base + 2 * k, z], BlockState::new("minecraft:sand"));
                }
            }
            let mut by_section = m.clone();
            let mut in_order = m;
            let a = settle(&mut by_section);
            let b = settle_in_cell_order(&mut in_order);
            assert_eq!(a, b, "round {round}: the outcomes");
            assert_eq!(
                by_section.iter().collect::<Vec<_>>(),
                in_order.iter().collect::<Vec<_>>(),
                "round {round}: the map"
            );
            despawned += a.iter().filter(|s| s.to.is_none()).count();
            landed += a.iter().filter(|s| s.to.is_some()).count();
        }
        assert!(
            despawned > 0 && landed > 0,
            "{despawned} despawned, {landed} landed"
        );
    }

    /// **A clipped shipped world is the whole world's inside the clip**: a
    /// generated store with writes laid over it (seals' cells, fixtures, props;
    /// cells written twice, clears among them), laid whole and laid inside
    /// several boxes, each box's read equal cell for cell to the whole's cells
    /// in it; and the whole equals the writes applied in order to a tree.
    #[test]
    fn a_clipped_shipped_world_is_the_whole_worlds_inside_the_clip() {
        let mut seed = 0x0fed_cba9_8765_4321u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let kinds = [
            "minecraft:stone",
            "minecraft:oak_door[half=lower]",
            "minecraft:oak_door[half=upper]",
            "minecraft:sea_lantern",
            "minecraft:lever",
        ];
        for round in 0..12 {
            let mut blocks = BlockMap::new();
            let mut tree: BTreeMap<[i32; 3], BlockState> = BTreeMap::new();
            for _ in 0..(200 + round * 100) {
                let c = [next(60) as i32 - 30, next(30) as i32, next(60) as i32 - 30];
                let b = BlockState::new(kinds[next(5) as usize]);
                blocks.insert(c, b);
                tree.insert(c, b);
            }
            let mut writes: Vec<ShippedWrite> = Vec::new();
            for _ in 0..(30 + round * 10) {
                let c = [next(60) as i32 - 30, next(30) as i32, next(60) as i32 - 30];
                let w = if next(5) == 0 {
                    None
                } else {
                    Some(BlockState::new(kinds[next(5) as usize]))
                };
                writes.push((c, w));
            }
            for (c, w) in &writes {
                match w {
                    Some(b) => {
                        tree.insert(*c, *b);
                    }
                    None => {
                        tree.remove(c);
                    }
                }
            }
            let whole = shipped_within(&blocks, &writes, &[], None);
            assert_eq!(
                whole.iter().map(|(c, b)| (c, *b)).collect::<Vec<_>>(),
                tree.iter().map(|(c, b)| (*c, *b)).collect::<Vec<_>>(),
                "round {round}: the whole"
            );
            for _ in 0..6 {
                let a = [
                    next(70) as i32 - 35,
                    next(36) as i32 - 3,
                    next(70) as i32 - 35,
                ];
                let b = [
                    a[0] + next(40) as i32,
                    a[1] + next(20) as i32,
                    a[2] + next(40) as i32,
                ];
                let clipped = shipped_within(&blocks, &writes, &[], Some((a, b)));
                assert_eq!(
                    clipped.iter().collect::<Vec<_>>(),
                    whole.in_box(a, b).collect::<Vec<_>>(),
                    "round {round}: the clip {a:?}..{b:?}"
                );
                // A clip given corner-last reads the same box.
                let swapped = shipped_within(&blocks, &writes, &[], Some((b, a)));
                assert_eq!(
                    swapped, clipped,
                    "round {round}: the clip's corners swapped"
                );
            }
        }
    }
}
