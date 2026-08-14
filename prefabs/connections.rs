//! **Connection states are derived from the piece's own neighbours, at the
//! emitter — one authority, every generator.**
//!
//! A `multipart` blockstate definition *assembles* its model out of the
//! properties its selectors name, so a palette entry that omits one does not
//! mean "the author had no opinion": it means vanilla supplies the default, and
//! the default of every connection property is *disconnected*. A bare
//! `minecraft:iron_bars` is two isolated posts where a barred gate was meant; a
//! bare `minecraft:oak_fence` is a lone post where a run of fencing was meant.
//! That is what `DW0735` reports, and it is what six shipped pieces were doing.
//!
//! Filling those properties with the block's **defaults** would be worse than
//! leaving them out — it turns a state that says nothing into a state that
//! explicitly asserts the disconnection nobody intended, and it silences the
//! gate at the same time. So this module does the only other thing there is to
//! do: it computes each connection from the blocks actually next to the cell,
//! by the rule vanilla itself applies (`FenceBlock.connectsTo`,
//! `IronBarsBlock.attachsTo`, `WallBlock.connectsTo` / `shouldRaisePost`,
//! `MultifaceBlock.canAttachTo`).
//!
//! **A written value is never overwritten.** [`resolve`] fills the
//! shape-carrying properties a state *omits* and touches nothing else, so a
//! fully-specified state emits byte-identically and an author who means an
//! isolated post can still say so — by saying it.
//!
//! ## The one thing vanilla does not publish
//!
//! Every rule above is ultimately "does the neighbour present a full, sturdy
//! face towards me". `BlockState.isFaceSturdy` is *code*: it appears in no
//! Mojang data export, so unlike the block registry and the shape-property
//! table there is nothing to vendor. The honest response is neither to guess
//! nor to default: [`face_support`] answers for the classes it can decide from
//! the pinned data plus a **declared** table of full cubes, and **refuses**
//! — a panic naming the block and the piece — for anything else. A generator
//! that puts a new material next to a fence is red until somebody says what
//! that material's face is. There is no direction in which a wrong guess here
//! is conservative: connecting where vanilla would not and failing to connect
//! where it would are equally visible, so a default would be a hack rather
//! than a safe approximation.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::invariants::Cells;

/// The pinned block registry — legal property names and values per block.
/// Source-included the same way this file is (see `invariants.rs`: two readers
/// of one file is not two authorities).
const BLOCK_REGISTRY_JSON: &str = include_str!("../crates/compiler/data/blocks-1.21.11.json");

/// Per block, the properties its own blockstate definition's `multipart`
/// selectors name — Mojang's answer to "which properties carry this block's
/// shape", never a hand-kept id list. `DW0735` fires on exactly this class.
const SHAPE_PROPS_JSON: &str =
    include_str!("../crates/compiler/data/blockstate-shape-props-1.21.11.json");

/// Per block, its **form** (shape class), derived from vanilla's own block
/// tags. `fence`, `wall` and `pane` are the three connection classes whose
/// members must be recognised by class rather than by name.
const CLASSIFICATION_JSON: &str =
    include_str!("../crates/compiler/data/block-classification-1.21.11.json");

fn registry() -> &'static BTreeMap<String, BTreeMap<String, Vec<String>>> {
    static R: OnceLock<BTreeMap<String, BTreeMap<String, Vec<String>>>> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(BLOCK_REGISTRY_JSON).expect("the block registry parses"))
}

fn shape_props() -> &'static BTreeMap<String, Vec<String>> {
    static S: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();
    S.get_or_init(|| {
        serde_json::from_str(SHAPE_PROPS_JSON).expect("the shape-property table parses")
    })
}

/// `block id -> form`, read out of the classification table's `blocks` map.
fn forms() -> &'static BTreeMap<String, String> {
    static F: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    F.get_or_init(|| {
        let v: serde_json::Value =
            serde_json::from_str(CLASSIFICATION_JSON).expect("the classification table parses");
        v.get("blocks")
            .and_then(|b| b.as_object())
            .expect("the classification table has a `blocks` map")
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.get("form")?.as_str()?.to_string())))
            .collect()
    })
}

// ---------------------------------------------------------------------------
// Directions
// ---------------------------------------------------------------------------

/// The six faces, in the fixed order every iteration here uses (ADR-0006).
const DIRS: [(&str, [i32; 3]); 6] = [
    ("down", [0, -1, 0]),
    ("east", [1, 0, 0]),
    ("north", [0, 0, -1]),
    ("south", [0, 0, 1]),
    ("up", [0, 1, 0]),
    ("west", [-1, 0, 0]),
];

/// The four horizontal faces a fence / pane / wall can reach along.
const HORIZONTALS: [&str; 4] = ["east", "north", "south", "west"];

fn offset(dir: &str) -> [i32; 3] {
    DIRS.iter()
        .find(|(d, _)| *d == dir)
        .map(|(_, o)| *o)
        .unwrap_or_else(|| panic!("{dir} is not one of the six faces"))
}

fn opposite(dir: &str) -> &'static str {
    match dir {
        "down" => "up",
        "up" => "down",
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        other => panic!("{other} is not one of the six faces"),
    }
}

fn axis(dir: &str) -> &'static str {
    match dir {
        "east" | "west" => "x",
        "up" | "down" => "y",
        "north" | "south" => "z",
        other => panic!("{other} is not one of the six faces"),
    }
}

/// The direction 90° clockwise (viewed from above) — vanilla's
/// `Direction.getClockWise()`, which is how a fence gate decides which way it
/// can be joined into.
fn clockwise(dir: &str) -> &'static str {
    match dir {
        "north" => "east",
        "east" => "south",
        "south" => "west",
        "west" => "north",
        other => panic!("{other} is not a horizontal face"),
    }
}

// ---------------------------------------------------------------------------
// Block classes
// ---------------------------------------------------------------------------

/// Air, in every spelling, plus the sentinel for a cell outside the template.
///
/// A cell beyond the piece's own bounds is deliberately treated as empty: a
/// prefab is authored as a standalone piece, and vanilla itself re-derives the
/// states along the placed region's border when the template lands
/// (`StructureTemplate.updateShapeAtEdge`), so an edge guess here would be
/// overwritten anyway. What matters is the interior, which nothing re-derives.
const OUTSIDE: &str = "<outside>";

fn is_empty(name: &str) -> bool {
    matches!(
        name,
        OUTSIDE | "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// The connection class of a block, when it has one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// `*_fence` — connects to same-kind fences, to perpendicular fence gates,
    /// and to a sturdy face. Never to a wall or to bars.
    Fence,
    /// Glass panes, iron bars, copper bars — connect to each other, to walls,
    /// and to a sturdy face. Never to a fence.
    Pane,
    /// `*_wall` — connects to walls, to panes/bars, to perpendicular fence
    /// gates, and to a sturdy face, with a three-valued `none|low|tall` side
    /// and a computed `up` post.
    Wall,
    /// `*_fence_gate` — not itself derived here (its `facing`/`open` are an
    /// authored decision), but it is what a fence and a wall join into.
    FenceGate,
    /// `vine`, `glow_lichen`, `sculk_vein`, … — independent per-face decals.
    /// Each face is a placement decision, not a connection: what is derived is
    /// that an unwritten face is **absent**, and what is checked is that a
    /// written face has something to hold on to.
    Multiface,
}

/// The one fence in 1.21.11 that is not a wooden fence. Vanilla's
/// `FenceBlock.isSameFence` joins two fences only when they agree about
/// `#minecraft:wooden_fences`, and this is the whole of the disagreement —
/// pinned by a test against the registry's fence set so a new species cannot
/// silently change the arithmetic.
const NON_WOODEN_FENCES: &[&str] = &["minecraft:nether_brick_fence"];

/// Blocks vanilla explicitly refuses to let a fence or a pane join, however
/// solid they are (`Block.isExceptionForConnection`). Full cubes every one,
/// which is exactly why the exception exists.
const EXCEPTIONS_FOR_CONNECTION: &[&str] = &[
    "minecraft:barrier",
    "minecraft:carved_pumpkin",
    "minecraft:jack_o_lantern",
    "minecraft:melon",
    "minecraft:pumpkin",
];

pub fn class(name: &str) -> Option<Class> {
    if MULTIFACE.contains(&name) {
        return Some(Class::Multiface);
    }
    match forms().get(name).map(String::as_str) {
        Some("fence") => Some(Class::Fence),
        Some("pane") => Some(Class::Pane),
        Some("wall") => Some(Class::Wall),
        _ if is_fence_gate(name) => Some(Class::FenceGate),
        _ => None,
    }
}

/// The multiface blocks of the pin. Vanilla has no tag for them and the
/// classification table calls them plain blocks, so the class is read off the
/// one thing that does distinguish them: a shape-property set that is exactly
/// the six faces (`glow_lichen`, `sculk_vein`, `resin_clump`) or the five a
/// vine can climb. Pinned by a test against the shape table.
const MULTIFACE: &[&str] = &[
    "minecraft:glow_lichen",
    "minecraft:resin_clump",
    "minecraft:sculk_vein",
    "minecraft:vine",
];

/// A fence gate, by its property signature rather than its name — the same
/// technique the classification table uses for `pane`.
fn is_fence_gate(name: &str) -> bool {
    let Some(props) = registry().get(name) else {
        return false;
    };
    let keys: BTreeSet<&str> = props.keys().map(String::as_str).collect();
    keys == BTreeSet::from(["facing", "in_wall", "open", "powered"])
}

// ---------------------------------------------------------------------------
// Face support — the part vanilla does not publish
// ---------------------------------------------------------------------------

/// Blocks that present a full, sturdy face on **every** side: the plain full
/// cubes. Declared, because `isFaceSturdy` lives in the game's code and in no
/// data export (see the module header). Every name is checked against the
/// pinned registry by a test, so an entry that stopped being a block is a red
/// rather than dead weight.
const FULL_CUBES: &[&str] = &[
    "minecraft:andesite",
    "minecraft:black_wool",
    "minecraft:chiseled_stone_bricks",
    "minecraft:coarse_dirt",
    "minecraft:cobblestone",
    "minecraft:cracked_stone_bricks",
    "minecraft:dark_oak_log",
    "minecraft:dark_prismarine",
    "minecraft:dirt",
    "minecraft:glowstone",
    "minecraft:grass_block",
    "minecraft:gravel",
    "minecraft:hay_block",
    "minecraft:mossy_cobblestone",
    "minecraft:mossy_stone_bricks",
    "minecraft:oak_log",
    "minecraft:oak_planks",
    "minecraft:prismarine",
    "minecraft:sand",
    "minecraft:spruce_planks",
    "minecraft:stone",
    "minecraft:stone_bricks",
    "minecraft:stripped_spruce_log",
    "minecraft:suspicious_gravel",
    "minecraft:tuff",
];

/// Blocks that present a full face on **no** side: nothing joins them and
/// nothing hangs off them. Same declaration rule as [`FULL_CUBES`].
///
/// `minecraft:jigsaw` is here for a reason that is not about its shape. The
/// block is a full cube in the template and is **replaced by its `final_state`**
/// when the piece is placed, so a bar that joined it would be joining a block
/// that is not going to be there.
const NO_FULL_FACE: &[&str] = &[
    "minecraft:campfire",
    "minecraft:chest",
    "minecraft:dead_bush",
    "minecraft:jigsaw",
    "minecraft:ladder",
    "minecraft:lantern",
    "minecraft:lava",
    "minecraft:pointed_dripstone",
    "minecraft:short_grass",
    "minecraft:soul_campfire",
    "minecraft:soul_lantern",
    "minecraft:soul_torch",
    "minecraft:torch",
    "minecraft:water",
];

/// Vanilla's `#minecraft:wall_post_override` — the blocks that make a wall
/// raise its post even though they rest on nothing solid. Recognised by form,
/// because the tag is "signs, banners, pressure plates and torches" and those
/// are exactly the families the classification table already names.
fn raises_wall_post(name: &str) -> bool {
    matches!(
        forms().get(name).map(String::as_str),
        Some("sign") | Some("pressure_plate")
    ) || name.ends_with("_banner")
        || matches!(
            name,
            "minecraft:torch"
                | "minecraft:soul_torch"
                | "minecraft:redstone_torch"
                | "minecraft:wall_torch"
                | "minecraft:soul_wall_torch"
                | "minecraft:redstone_wall_torch"
                | "minecraft:tripwire"
        )
}

/// Does the block at a neighbouring cell present a full, sturdy face on the
/// side `face` — the side that touches us?
///
/// `None` means *this module has no verdict*, which is a refusal and never a
/// default. See the module header for why there is no safe direction to guess
/// in.
pub fn face_support(name: &str, props: &BTreeMap<String, String>, face: &str) -> Option<bool> {
    if is_empty(name) {
        return Some(false);
    }
    if FULL_CUBES.contains(&name) {
        return Some(true);
    }
    if NO_FULL_FACE.contains(&name) {
        return Some(false);
    }
    // A fence, a wall, a pane and a fence gate are all thinner than their cell
    // on every side; they join each other by class, never by face.
    if class(name).is_some() {
        return Some(false);
    }
    if forms().get(name).map(String::as_str) == Some("stair") {
        return stair_face_support(props, face);
    }
    None
}

/// A stair's sturdy faces, from its own state.
///
/// A straight stair is a slab plus the upper step against the side it faces, so
/// it is full on `facing`, and full on `down` (or `up`) according to `half`.
/// A corner stair (`inner_*` / `outer_*`) is deliberately **not** decided here:
/// its second quarter changes which faces are full, and no piece in this repo
/// has ever put one next to a connecting block. Refusing is the point — the
/// first piece that does gets a red naming the state, not a silent guess.
fn stair_face_support(props: &BTreeMap<String, String>, face: &str) -> Option<bool> {
    let shape = props.get("shape")?.as_str();
    if shape != "straight" {
        return None;
    }
    let facing = props.get("facing")?.as_str();
    let half = props.get("half")?.as_str();
    Some(match face {
        "up" => half == "top",
        "down" => half == "bottom",
        h => h == facing,
    })
}

/// **Can a decal — a vine, a patch of lichen — sit on this cell's `face`?**
///
/// `name`/`props` are the block in that direction, and the question is whether
/// it presents a full face back (`MultifaceBlock.canAttachTo`). A generator's
/// decoration scan asks this *before* placing, so an unclassified neighbour
/// answers **no**: declining to decorate is the safe direction for a placement
/// decision, and the piece is visibly poorer rather than silently wrong. The
/// refusal stays where it belongs — on an emitted state that names such a face
/// ([`assert_attachments_are_supported`]).
///
/// Private on purpose: see [`attachable_faces`], which is the question a placer
/// actually has.
fn can_attach(name: &str, props: &BTreeMap<String, String>, face: &str) -> bool {
    face_support(name, props, opposite(face)).unwrap_or(false)
}

/// The order [`attachable_faces`] offers a decal its supports in.
///
/// The four walls before the ceiling before the floor, because that is the
/// order in which a face reads: growth on a wall is what a wall decal is, a
/// ceiling is the next thing a player looks at, and a floor decal is under
/// their feet. Within the walls the sequence carries no meaning beyond being
/// **fixed** — a cell with rock on two sides must resolve the same way on every
/// run (ADR-0006) — and it is the sequence the decoration scans already walked,
/// so introducing this table moves no decal that already had a wall to hold.
///
/// Deliberately not [`DIRS`]: that order is alphabetical and puts `down` first,
/// which would hang every decal on the floor.
const ATTACH_PREFERENCE: [&str; 6] = ["west", "east", "north", "south", "up", "down"];

/// **Where may this attachable block hold on, here?**
///
/// The whole of "a multiface block needs something to attach to, and these are
/// the places it may attach" — returned as every face of *this* block that has
/// a supporting neighbour, in [`ATTACH_PREFERENCE`] order. A decoration scan
/// asks this and takes the first answer; an empty answer means the cell can
/// hold no decal at all and none should be placed.
///
/// The capability belongs here, on the block class that *has* faces, and not in
/// whatever pass happens to be placing plants, for two reasons that both cost a
/// shipped piece before this existed:
///
///  1. **The faces a block has are this module's fact.** They come from the
///     pinned shape table, so a vine is asked about its five and a lichen about
///     its six. Every caller that enumerated faces by hand enumerated *four* —
///     the horizontals — and so a decal whose only rock was overhead had
///     nowhere to hang and was silently dropped rather than hung from the
///     ceiling. That is a defect of expressibility, not of care, and it cannot
///     be fixed once per caller.
///  2. **A face and the direction it looks in are one fact, not two.** A caller
///     pairing its own offsets with its own face names can pair them backwards,
///     which is exactly what two scans did: they found the rock and then named
///     the face pointing away from it, and vanilla deleted the decal at the
///     first block update. Here the pairing is [`offset`]'s, once.
///
/// `at` answers what block occupies a cell — `None` for air or for outside the
/// piece. An unclassified neighbour answers *no* (see [`can_attach`]): a placer
/// declining to decorate leaves the piece poorer, never silently wrong.
///
/// This does not *repair* an emitted state, and must not: a multiface face is a
/// placement decision rather than a connection (see [`multiface_state`]), so
/// re-hanging a decal that already shipped facing nothing would convert
/// [`assert_attachments_are_supported`]'s red into a silent rewrite. The query
/// serves the placer; the assertion still judges the bytes.
pub fn attachable_faces<F>(name: &str, pos: [i32; 3], at: F) -> Vec<&'static str>
where
    F: Fn([i32; 3]) -> Option<(String, BTreeMap<String, String>)>,
{
    assert_eq!(
        class(name),
        Some(Class::Multiface),
        "{name} is not a multiface block, so it does not hold on by a face. Ask \
         `attachable_faces` only about blocks in `MULTIFACE`."
    );
    let faces = shape_props().get(name).map(Vec::as_slice).unwrap_or(&[]);
    ATTACH_PREFERENCE
        .iter()
        .copied()
        .filter(|face| faces.iter().any(|f| f == face))
        .filter(|face| {
            let o = offset(face);
            let q = [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]];
            match at(q) {
                Some((nname, nprops)) => can_attach(&nname, &nprops, face),
                None => false,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The piece, as every generator's `Structure` can be viewed
// ---------------------------------------------------------------------------

/// A piece's palette and block list, in the one shape every generator shares.
/// Each generator converts its own `Structure` into this and back — the types
/// are per-workspace (they are separate Cargo workspaces on purpose), the rule
/// is not.
pub struct Piece {
    /// `(block id, properties)`, in palette order.
    pub palette: Vec<(String, BTreeMap<String, String>)>,
    /// One entry per block, in the emitted order.
    pub positions: Vec<[i32; 3]>,
    /// The palette index of each block, parallel to `positions`.
    pub states: Vec<usize>,
}

impl Piece {
    fn cells(&self) -> Cells {
        self.positions
            .iter()
            .zip(&self.states)
            .map(|(pos, &s)| {
                let (name, props) = &self.palette[s];
                (*pos, (name.clone(), props.clone()))
            })
            .collect()
    }
}

/// **Fill every shape-carrying property the piece leaves unwritten, from the
/// blocks actually next to the cell.**
///
/// Written values are kept verbatim. Palette entries that no cell uses any more
/// are pruned and the indices remapped, so the emitted template carries no
/// entry that describes nothing.
pub fn resolve(id: &str, piece: &mut Piece) {
    let cells = piece.cells();
    let mut derived: BTreeMap<[i32; 3], BTreeMap<String, String>> = BTreeMap::new();

    // Walls first and from the top down: a wall's side height and its post both
    // read the wall above, which must therefore already be resolved. Everything
    // else reads only classes and faces, so its order is free.
    let mut wall_cells: Vec<[i32; 3]> = cells
        .iter()
        .filter(|(_, (n, _))| class(n) == Some(Class::Wall))
        .map(|(p, _)| *p)
        .collect();
    wall_cells.sort_by_key(|p| (-p[1], p[0], p[2]));
    for pos in wall_cells {
        let (name, props) = &cells[&pos];
        let _ = name;
        let filled = wall_state(id, &cells, &derived, pos, props);
        derived.insert(pos, filled);
    }

    for (pos, (name, props)) in &cells {
        let filled = match class(name) {
            Some(Class::Wall) => continue,
            Some(Class::Fence) => cross_state(id, &cells, *pos, name, props, Class::Fence),
            Some(Class::Pane) => cross_state(id, &cells, *pos, name, props, Class::Pane),
            Some(Class::Multiface) => multiface_state(name, props),
            _ => continue,
        };
        derived.insert(*pos, filled);
    }

    // Re-intern: an entry that changed becomes (or joins) another entry.
    let mut palette: Vec<(String, BTreeMap<String, String>)> = piece.palette.clone();
    for (i, pos) in piece.positions.iter().enumerate() {
        let Some(props) = derived.get(pos) else {
            continue;
        };
        let name = palette[piece.states[i]].0.clone();
        let want = (name, props.clone());
        piece.states[i] = match palette.iter().position(|e| *e == want) {
            Some(k) => k,
            None => {
                palette.push(want);
                palette.len() - 1
            }
        };
    }

    // Prune entries nothing uses and remap. A palette entry that describes no
    // cell is exactly the bare state this pass replaced, and leaving it behind
    // would keep the piece asserting a state it no longer places.
    let used: BTreeSet<usize> = piece.states.iter().copied().collect();
    let mut remap: Vec<Option<usize>> = vec![None; palette.len()];
    let mut kept: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for (i, entry) in palette.into_iter().enumerate() {
        if used.contains(&i) {
            remap[i] = Some(kept.len());
            kept.push(entry);
        }
    }
    for s in piece.states.iter_mut() {
        *s = remap[*s].expect("a used palette entry is kept");
    }
    piece.palette = kept;
}

/// The state of a fence or a pane: four booleans, each the answer to "is there
/// something in that direction to join".
fn cross_state(
    id: &str,
    cells: &Cells,
    pos: [i32; 3],
    name: &str,
    props: &BTreeMap<String, String>,
    kind: Class,
) -> BTreeMap<String, String> {
    let mut out = props.clone();
    for dir in HORIZONTALS {
        if out.contains_key(dir) {
            continue;
        }
        let (nname, nprops) = neighbour(cells, pos, dir);
        let joined = match kind {
            Class::Fence => {
                same_fence(name, &nname)
                    || gate_joins(&nname, &nprops, dir)
                    || sturdy_for_connection(id, pos, &nname, &nprops, dir)
            }
            Class::Pane => {
                matches!(class(&nname), Some(Class::Pane) | Some(Class::Wall))
                    || sturdy_for_connection(id, pos, &nname, &nprops, dir)
            }
            other => panic!("{other:?} is not a cross-collision block"),
        };
        out.insert(dir.to_string(), joined.to_string());
    }
    out
}

/// The state of a wall: three-valued sides plus the `up` post, both of which
/// read the block above.
fn wall_state(
    id: &str,
    cells: &Cells,
    derived: &BTreeMap<[i32; 3], BTreeMap<String, String>>,
    pos: [i32; 3],
    props: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let above_pos = [pos[0], pos[1] + 1, pos[2]];
    let (above, above_props) = neighbour(cells, pos, "up");
    // The wall above, already resolved (top-down order), if there is one.
    let above_wall: Option<&BTreeMap<String, String>> = (class(&above) == Some(Class::Wall))
        .then(|| derived.get(&above_pos))
        .flatten();
    let above_is_full = face_support(&above, &above_props, "down").unwrap_or_else(|| {
        panic!(
            "{id}: a wall at {pos:?} has {above} above it, and this module has no verdict on \
             whether that block's underside is a full face — so it cannot say whether the wall \
             runs up to meet it (`tall`) or stops short (`low`). Classify {above} in \
             `prefabs/connections.rs` (FULL_CUBES / NO_FULL_FACE) rather than letting the wall \
             guess."
        )
    });

    let mut out = props.clone();
    for dir in HORIZONTALS {
        if out.contains_key(dir) {
            continue;
        }
        let (nname, nprops) = neighbour(cells, pos, dir);
        let joined = matches!(class(&nname), Some(Class::Wall) | Some(Class::Pane))
            || gate_joins(&nname, &nprops, dir)
            || sturdy_for_connection(id, pos, &nname, &nprops, dir);
        // A side rises to full height when the block above covers the arm: a
        // full cube does, and so does a wall above that has its own arm the
        // same way. Anything else leaves the arm short.
        let tall = above_is_full
            || above_wall
                .and_then(|w| w.get(dir))
                .is_some_and(|v| v != "none");
        out.insert(
            dir.to_string(),
            match (joined, tall) {
                (false, _) => "none",
                (true, false) => "low",
                (true, true) => "tall",
            }
            .to_string(),
        );
    }

    if !out.contains_key("up") {
        let side = |d: &str| out.get(d).map(String::as_str).unwrap_or("none");
        let (n, s, e, w) = (
            side("north") == "none",
            side("south") == "none",
            side("east") == "none",
            side("west") == "none",
        );
        // Vanilla's `shouldRaisePost`: a wall keeps its post unless it is a
        // straight run through — a dead end, a lone post and a corner all keep
        // it, and so does anything that covers the post's own footprint.
        let straight_through = !((n && s && e && w) || (n != s) || (w != e));
        let above_covers_post = above_is_full
            || above_wall.is_some_and(|w| {
                w.get("up").map(String::as_str) == Some("true")
                    || HORIZONTALS
                        .iter()
                        .any(|d| w.get(*d).map(String::as_str).unwrap_or("none") != "none")
            })
            || raises_wall_post(&above);
        out.insert(
            "up".to_string(),
            (!straight_through || above_covers_post).to_string(),
        );
    }
    out
}

/// A multiface block's faces: the ones the author placed, and `false` for the
/// rest.
///
/// Each face of a vine or a lichen is an independent placement, not a
/// connection — vanilla puts the decal on the face you clicked and on no other
/// — so there is nothing here to derive from the neighbours except the fact
/// that an unwritten face is an absent one. What the neighbours *do* decide is
/// whether a written face can exist at all, and that is
/// [`assert_attachments_are_supported`]'s question.
fn multiface_state(name: &str, props: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut out = props.clone();
    for face in shape_props().get(name).map(Vec::as_slice).unwrap_or(&[]) {
        out.entry(face.clone())
            .or_insert_with(|| "false".to_string());
    }
    out
}

fn neighbour(cells: &Cells, pos: [i32; 3], dir: &str) -> (String, BTreeMap<String, String>) {
    let o = offset(dir);
    let q = [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]];
    cells
        .get(&q)
        .cloned()
        .unwrap_or_else(|| (OUTSIDE.to_string(), BTreeMap::new()))
}

/// Two fences join when they are both wooden or both not — vanilla's
/// `isSameFence`.
fn same_fence(a: &str, b: &str) -> bool {
    if class(a) != Some(Class::Fence) || class(b) != Some(Class::Fence) {
        return false;
    }
    NON_WOODEN_FENCES.contains(&a) == NON_WOODEN_FENCES.contains(&b)
}

/// A fence gate can be joined from the two sides its panel spans — vanilla's
/// `FenceGateBlock.connectsToDirection`.
fn gate_joins(name: &str, props: &BTreeMap<String, String>, dir: &str) -> bool {
    if class(name) != Some(Class::FenceGate) {
        return false;
    }
    props
        .get("facing")
        .is_some_and(|f| axis(clockwise(f)) == axis(dir))
}

/// The "…or it is simply solid there" half of every connection rule, with the
/// refusal that keeps it honest.
fn sturdy_for_connection(
    id: &str,
    pos: [i32; 3],
    nname: &str,
    nprops: &BTreeMap<String, String>,
    dir: &str,
) -> bool {
    if EXCEPTIONS_FOR_CONNECTION.contains(&nname) {
        return false;
    }
    face_support(nname, nprops, opposite(dir)).unwrap_or_else(|| {
        panic!(
            "{id}: the cell at {pos:?} has {nname} to its {dir}, and this module has no verdict \
             on whether that block presents a full face there — so it cannot say whether the \
             fence/wall/bars join it or stop at it. Classify {nname} in \
             `prefabs/connections.rs` (FULL_CUBES / NO_FULL_FACE, or a per-state rule if its \
             faces differ). Guessing is not available: connecting where vanilla would not and \
             failing to connect where it would are equally visible."
        )
    })
}

// ---------------------------------------------------------------------------
// Emitter post-conditions
// ---------------------------------------------------------------------------

/// **Every state the piece places writes the properties that carry its shape**
/// — `DW0735`, at the emitter rather than at admission.
///
/// The rule already exists at two later gates, and both of them bind to one
/// moment: `delve-admit audit` judges a piece as it *enters* the library, and
/// the library sweep re-judges what is already in it. Neither reaches a piece
/// between the two, which is a generator's entire output. This is the same
/// verdict where the bytes are produced, so a new placement path cannot ship a
/// disconnected wall and wait for someone to notice.
pub fn assert_shape_is_stated(id: &str, cells: &Cells) {
    let table = shape_props();
    let mut bad: BTreeMap<String, usize> = BTreeMap::new();
    let mut examined = 0usize;
    for (name, props) in cells.values() {
        let Some(need) = table.get(name) else {
            continue;
        };
        examined += 1;
        let missing: Vec<&str> = need
            .iter()
            .filter(|p| !props.contains_key(*p))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            *bad.entry(format!("{name} omits {}", missing.join(", ")))
                .or_insert(0) += 1;
        }
    }
    assert!(
        bad.is_empty(),
        "{id}: {} cell(s) of {examined} shape-carrying cell(s) place a block state that leaves a \
         shape-carrying property unwritten, so vanilla supplies its default — which for every \
         connection property is DISCONNECTED. The piece would ship isolated posts where a run of \
         fencing or a barred gate is meant. Offenders: {}",
        bad.values().sum::<usize>(),
        bad.iter()
            .map(|(reason, n)| format!("{reason} ({n} cell(s))"))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

/// **Every face a vine or a lichen declares has something to hold on to.**
///
/// The general form of a defect two generators shipped: the placement scan
/// found the rock and then named the face pointing *away* from it, so the decal
/// hung in the air on the wrong side of the cell. Vanilla deletes such a face on
/// the first block update, which makes it the worst kind of wrong — it looks
/// right in the template and disappears in play.
///
/// It is a separate assertion from [`assert_shape_is_stated`] on purpose: a
/// fully-written state satisfies that one no matter which faces it names, so
/// the completeness rule can never catch this.
pub fn assert_attachments_are_supported(id: &str, cells: &Cells) {
    let mut bad: Vec<String> = Vec::new();
    let mut examined = 0usize;
    for (pos, (name, props)) in cells {
        if class(name) != Some(Class::Multiface) {
            continue;
        }
        examined += 1;
        for (face, value) in props {
            if value != "true" || !DIRS.iter().any(|(d, _)| d == face) {
                continue;
            }
            let (nname, nprops) = neighbour(cells, *pos, face);
            let supported = face_support(&nname, &nprops, opposite(face)).unwrap_or_else(|| {
                panic!(
                    "{id}: {name} at {pos:?} declares a face to its {face}, where {nname} is, and \
                     this module has no verdict on whether that block can hold it. Classify \
                     {nname} in `prefabs/connections.rs`."
                )
            });
            if !supported {
                bad.push(format!(
                    "{name} at {pos:?} declares {face}=true, but {nname} is there"
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{id}: {} of {examined} multiface cell(s) declare a face with nothing behind it. Vanilla \
         removes such a face on the first block update, so the decal is in the template and not \
         in the game. Name the face that touches the rock, not the one opposite it. Offenders: {}",
        bad.len(),
        bad.iter().take(12).cloned().collect::<Vec<_>>().join("; ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn cell(cells: &mut Cells, pos: [i32; 3], name: &str, p: &[(&str, &str)]) {
        cells.insert(pos, (name.to_string(), props(p)));
    }

    fn piece_of(cells: &Cells) -> Piece {
        let mut palette: Vec<(String, BTreeMap<String, String>)> = Vec::new();
        let mut positions = Vec::new();
        let mut states = Vec::new();
        for (pos, entry) in cells {
            let i = palette.iter().position(|e| e == entry).unwrap_or_else(|| {
                palette.push(entry.clone());
                palette.len() - 1
            });
            positions.push(*pos);
            states.push(i);
        }
        Piece {
            palette,
            positions,
            states,
        }
    }

    fn resolved(cells: &Cells) -> Cells {
        let mut p = piece_of(cells);
        resolve("fixture", &mut p);
        p.cells()
    }

    /// The classes are read off the pinned data, not off a list somebody kept.
    #[test]
    fn the_connection_classes_come_from_the_pinned_tables() {
        assert_eq!(class("minecraft:oak_fence"), Some(Class::Fence));
        assert_eq!(class("minecraft:nether_brick_fence"), Some(Class::Fence));
        assert_eq!(class("minecraft:iron_bars"), Some(Class::Pane));
        assert_eq!(class("minecraft:glass_pane"), Some(Class::Pane));
        assert_eq!(class("minecraft:cobblestone_wall"), Some(Class::Wall));
        assert_eq!(class("minecraft:oak_fence_gate"), Some(Class::FenceGate));
        assert_eq!(class("minecraft:glow_lichen"), Some(Class::Multiface));
        assert_eq!(class("minecraft:stone"), None);
        // Binding counts: a table that stopped naming these would make every
        // rule here inert while every test above still passed on a hand list.
        let fences = forms().values().filter(|f| *f == "fence").count();
        let panes = forms().values().filter(|f| *f == "pane").count();
        let walls = forms().values().filter(|f| *f == "wall").count();
        assert_eq!((fences, panes, walls), (13, 26, 26));
    }

    /// Every declared block name is a block the pinned version has, and every
    /// multiface entry really is one — the same binding `invariants.rs` puts on
    /// its own curated lists.
    #[test]
    fn every_declared_block_name_is_real() {
        let reg = registry();
        for name in FULL_CUBES
            .iter()
            .chain(NO_FULL_FACE)
            .chain(MULTIFACE)
            .chain(NON_WOODEN_FENCES)
            .chain(EXCEPTIONS_FOR_CONNECTION)
        {
            assert!(
                reg.contains_key(*name),
                "{name} is declared here but Minecraft 1.21.11 has no such block"
            );
        }
        for name in MULTIFACE {
            let faces = shape_props()
                .get(*name)
                .unwrap_or_else(|| panic!("{name} carries no multipart shape properties"));
            assert!(
                faces.iter().all(|f| DIRS.iter().any(|(d, _)| d == f)),
                "{name}'s shape properties are not faces: {faces:?}"
            );
        }
        for name in NON_WOODEN_FENCES {
            assert_eq!(class(name), Some(Class::Fence));
        }
        assert_eq!(FULL_CUBES.len(), 25);
        assert_eq!(NO_FULL_FACE.len(), 14);
        assert!(FULL_CUBES.iter().all(|n| !NO_FULL_FACE.contains(n)));
    }

    /// The portcullis: a run of bars between two stone jambs is a grid, not a
    /// row of posts.
    #[test]
    fn a_run_of_bars_between_jambs_is_a_grid() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:stone_bricks", &[]);
        cell(&mut cells, [1, 0, 0], "minecraft:iron_bars", &[]);
        cell(&mut cells, [2, 0, 0], "minecraft:iron_bars", &[]);
        cell(&mut cells, [3, 0, 0], "minecraft:stone_bricks", &[]);
        let out = resolved(&cells);
        for x in [1, 2] {
            let (_, p) = &out[&[x, 0, 0]];
            assert_eq!(p["east"], "true", "bars at x={x}");
            assert_eq!(p["west"], "true", "bars at x={x}");
            assert_eq!(p["north"], "false");
            assert_eq!(p["south"], "false");
        }
    }

    /// A fence joins its own kind and a perpendicular gate, and stops at a
    /// lantern.
    #[test]
    fn a_fence_joins_its_kind_and_a_gate_but_not_a_lantern() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:oak_fence", &[]);
        cell(&mut cells, [1, 0, 0], "minecraft:oak_fence", &[]);
        cell(
            &mut cells,
            [2, 0, 0],
            "minecraft:oak_fence_gate",
            &[
                ("facing", "north"),
                ("in_wall", "false"),
                ("open", "false"),
                ("powered", "false"),
            ],
        );
        cell(&mut cells, [0, 0, 1], "minecraft:lantern", &[]);
        let out = resolved(&cells);
        let (_, a) = &out[&[0, 0, 0]];
        assert_eq!(a["east"], "true", "joins the fence to its east");
        assert_eq!(a["south"], "false", "a lantern is nothing to join");
        let (_, b) = &out[&[1, 0, 0]];
        assert_eq!(b["east"], "true", "a north-facing gate is joined along x");
    }

    /// A fence does not join a wall and a wall does not join a fence, but both
    /// join bars — vanilla's asymmetry, kept.
    #[test]
    fn a_fence_and_a_wall_do_not_join_each_other() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:oak_fence", &[]);
        cell(&mut cells, [1, 0, 0], "minecraft:cobblestone_wall", &[]);
        cell(&mut cells, [2, 0, 0], "minecraft:iron_bars", &[]);
        let out = resolved(&cells);
        assert_eq!(out[&[0, 0, 0]].1["east"], "false");
        assert_eq!(out[&[1, 0, 0]].1["west"], "none");
        assert_eq!(out[&[1, 0, 0]].1["east"], "low", "a wall joins bars");
        assert_eq!(out[&[2, 0, 0]].1["west"], "true", "bars join a wall");
    }

    /// A wall's post and height: a lone wall keeps its post, a straight run
    /// through drops it, and a full cube overhead makes the sides tall.
    #[test]
    fn a_wall_post_follows_the_run_and_the_block_above() {
        let mut lone = Cells::new();
        cell(&mut lone, [0, 0, 0], "minecraft:cobblestone_wall", &[]);
        assert_eq!(resolved(&lone)[&[0, 0, 0]].1["up"], "true");

        let mut run = Cells::new();
        for z in 0..3 {
            cell(&mut run, [0, 0, z], "minecraft:cobblestone_wall", &[]);
        }
        let out = resolved(&run);
        assert_eq!(out[&[0, 0, 1]].1["up"], "false", "straight through");
        assert_eq!(out[&[0, 0, 0]].1["up"], "true", "dead end keeps its post");
        assert_eq!(out[&[0, 0, 1]].1["north"], "low");

        let mut roofed = Cells::new();
        for z in 0..3 {
            cell(&mut roofed, [0, 0, z], "minecraft:cobblestone_wall", &[]);
        }
        cell(&mut roofed, [0, 1, 1], "minecraft:stone", &[]);
        let out = resolved(&roofed);
        assert_eq!(out[&[0, 0, 1]].1["north"], "tall", "runs up to the ceiling");
        assert_eq!(out[&[0, 0, 1]].1["up"], "true");
    }

    /// Stacked walls: the lower one runs up to meet the arm above it.
    #[test]
    fn a_stacked_wall_meets_the_arm_above_it() {
        let mut cells = Cells::new();
        for y in 0..2 {
            for z in 0..3 {
                cell(&mut cells, [0, y, z], "minecraft:stone_brick_wall", &[]);
            }
        }
        let out = resolved(&cells);
        assert_eq!(out[&[0, 1, 1]].1["north"], "low", "nothing above the top");
        assert_eq!(out[&[0, 0, 1]].1["north"], "tall", "arm above it");
        assert_eq!(out[&[0, 0, 1]].1["up"], "true", "the wall above covers it");
    }

    /// A written value is authority: the pass fills what is missing and changes
    /// nothing that was said.
    #[test]
    fn a_written_connection_is_never_overwritten() {
        let mut cells = Cells::new();
        cell(
            &mut cells,
            [0, 0, 0],
            "minecraft:oak_fence",
            &[("east", "false")],
        );
        cell(&mut cells, [1, 0, 0], "minecraft:oak_fence", &[]);
        let out = resolved(&cells);
        assert_eq!(out[&[0, 0, 0]].1["east"], "false", "kept as authored");
        assert_eq!(
            out[&[1, 0, 0]].1["west"],
            "true",
            "derived on the other side"
        );
    }

    /// A fully-specified piece is left exactly as it was — palette and all.
    #[test]
    fn a_complete_piece_is_untouched() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:stone", &[]);
        cell(&mut cells, [0, 1, 0], "minecraft:air", &[]);
        let before = piece_of(&cells);
        let mut after = piece_of(&cells);
        resolve("fixture", &mut after);
        assert_eq!(after.palette, before.palette);
        assert_eq!(after.states, before.states);
    }

    /// The bare state the whole pass exists for is refused at the emitter.
    #[test]
    #[should_panic(expected = "leaves a shape-carrying property unwritten")]
    fn a_bare_connection_state_fails_the_post_condition() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:iron_bars", &[]);
        assert_shape_is_stated("fixture", &cells);
    }

    #[test]
    fn the_resolved_state_passes_the_post_condition() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:stone_bricks", &[]);
        cell(&mut cells, [1, 0, 0], "minecraft:iron_bars", &[]);
        assert_shape_is_stated("fixture", &resolved(&cells));
    }

    /// The inversion itself: the face named away from the rock.
    #[test]
    #[should_panic(expected = "declare a face with nothing behind it")]
    fn a_lichen_facing_away_from_the_rock_fails() {
        // The rock is SOUTH of the lichen; the state names north, which is air.
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 2], "minecraft:tuff", &[]);
        cell(
            &mut cells,
            [0, 0, 1],
            "minecraft:glow_lichen",
            &[("north", "true")],
        );
        assert_attachments_are_supported("fixture", &resolved(&cells));
    }

    #[test]
    fn a_lichen_on_the_rock_passes_and_its_other_faces_are_absent() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:tuff", &[]);
        cell(
            &mut cells,
            [0, 0, 1],
            "minecraft:glow_lichen",
            &[("north", "true")],
        );
        // The lichen is at z=1 and the rock at z=0, which is to its NORTH.
        let out = resolved(&cells);
        assert_attachments_are_supported("fixture", &out);
        let (_, p) = &out[&[0, 0, 1]];
        assert_eq!(p["north"], "true");
        assert_eq!(p["south"], "false");
        assert_eq!(p["up"], "false");
        assert_eq!(p["down"], "false");
    }

    /// `attachable_faces` over a fixture built as a `Cells` map.
    fn faces_at(cells: &Cells, name: &str, pos: [i32; 3]) -> Vec<&'static str> {
        attachable_faces(name, pos, |q| cells.get(&q).cloned())
    }

    /// **The whole point of the capability living here.** A decal whose only
    /// rock is overhead hangs from it. Every caller that enumerated faces by
    /// hand enumerated the four horizontals, so this cell offered nothing and
    /// the decal was dropped instead of hung.
    #[test]
    fn a_decal_whose_only_rock_is_overhead_hangs_from_it() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 1, 0], "minecraft:tuff", &[]);
        // Everything around it that is not rock: air, and a dripstone tip,
        // which is the neighbour that looks like support and is not.
        cell(&mut cells, [1, 0, 0], "minecraft:pointed_dripstone", &[]);
        cell(&mut cells, [0, 0, 0], "minecraft:glow_lichen", &[]);
        assert_eq!(
            faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]),
            vec!["up"]
        );
        assert_eq!(faces_at(&cells, "minecraft:vine", [0, 0, 0]), vec!["up"]);
    }

    /// A wall is offered before the ceiling, so adopting the general query
    /// moves no decal that already had a wall to hold.
    #[test]
    fn a_wall_is_offered_before_the_ceiling_and_the_ceiling_before_the_floor() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:glow_lichen", &[]);
        cell(&mut cells, [0, -1, 0], "minecraft:tuff", &[]);
        assert_eq!(
            faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]),
            vec!["down"]
        );
        cell(&mut cells, [0, 1, 0], "minecraft:tuff", &[]);
        assert_eq!(
            faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]),
            vec!["up", "down"]
        );
        cell(&mut cells, [0, 0, 1], "minecraft:tuff", &[]);
        assert_eq!(
            faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]),
            vec!["south", "up", "down"]
        );
    }

    /// The faces a block has come from the pinned shape table, so a vine is
    /// asked about its five and never offered the `down` it does not have.
    #[test]
    fn a_vine_is_never_offered_the_face_it_does_not_have() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, -1, 0], "minecraft:tuff", &[]);
        cell(&mut cells, [0, 1, 0], "minecraft:tuff", &[]);
        assert_eq!(faces_at(&cells, "minecraft:vine", [0, 0, 0]), vec!["up"]);
        assert_eq!(
            faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]),
            vec!["up", "down"]
        );
        // Binding count, so the assertion above cannot pass by the table
        // having gone empty.
        assert_eq!(shape_props()["minecraft:vine"].len(), 5);
        assert_eq!(shape_props()["minecraft:glow_lichen"].len(), 6);
    }

    /// A face and the direction it looks in are one fact. The offsets are the
    /// module's, so a caller cannot pair them backwards the way two decoration
    /// scans did.
    #[test]
    fn the_face_offered_is_the_one_that_touches_the_rock() {
        let mut cells = Cells::new();
        // Rock to the WEST of the cell.
        cell(&mut cells, [-1, 0, 0], "minecraft:andesite", &[]);
        cell(&mut cells, [0, 0, 0], "minecraft:glow_lichen", &[]);
        assert_eq!(
            faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]),
            vec!["west"]
        );
        // And what it offers is exactly what the post-condition accepts.
        let mut out = Cells::new();
        cell(&mut out, [-1, 0, 0], "minecraft:andesite", &[]);
        cell(
            &mut out,
            [0, 0, 0],
            "minecraft:glow_lichen",
            &[("west", "true")],
        );
        assert_attachments_are_supported("fixture", &resolved(&out));
    }

    /// Nothing to hold on to is an empty answer, which is a placer's cue to
    /// place nothing — never a face picked anyway.
    #[test]
    fn a_cell_with_no_support_offers_no_face() {
        let mut cells = Cells::new();
        cell(&mut cells, [1, 0, 0], "minecraft:pointed_dripstone", &[]);
        cell(
            &mut cells,
            [0, 0, 1],
            "minecraft:lantern",
            &[("hanging", "true")],
        );
        cell(&mut cells, [0, 0, 0], "minecraft:glow_lichen", &[]);
        assert!(faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]).is_empty());
    }

    /// An unclassified neighbour declines, because this is a PLACEMENT
    /// decision: a poorer piece is safe, a silently wrong one is not. The
    /// refusal stays on the emitted state, which
    /// `an_unclassified_neighbour_refuses` covers.
    #[test]
    fn an_unclassified_neighbour_offers_no_face_rather_than_panicking() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 1, 0], "minecraft:cake", &[]);
        cell(&mut cells, [0, 0, 0], "minecraft:glow_lichen", &[]);
        assert!(faces_at(&cells, "minecraft:glow_lichen", [0, 0, 0]).is_empty());
    }

    /// The question is only meaningful for a block that holds on by a face.
    #[test]
    #[should_panic(expected = "does not hold on by a face")]
    fn asking_a_fence_where_it_may_attach_is_a_bug() {
        let cells = Cells::new();
        faces_at(&cells, "minecraft:oak_fence", [0, 0, 0]);
    }

    /// A material with no declared verdict is a refusal, never a default.
    #[test]
    #[should_panic(expected = "no verdict on whether that block presents a full face")]
    fn an_unclassified_neighbour_refuses() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:oak_fence", &[]);
        cell(&mut cells, [1, 0, 0], "minecraft:cake", &[]);
        resolved(&cells);
    }

    /// A straight stair is full on the side it faces and nowhere else
    /// horizontally; a corner stair is refused rather than guessed.
    #[test]
    fn a_straight_stair_is_full_on_the_side_it_faces() {
        let p = props(&[
            ("facing", "north"),
            ("half", "bottom"),
            ("shape", "straight"),
        ]);
        assert_eq!(
            face_support("minecraft:stone_brick_stairs", &p, "north"),
            Some(true)
        );
        assert_eq!(
            face_support("minecraft:stone_brick_stairs", &p, "east"),
            Some(false)
        );
        assert_eq!(
            face_support("minecraft:stone_brick_stairs", &p, "down"),
            Some(true)
        );
        assert_eq!(
            face_support("minecraft:stone_brick_stairs", &p, "up"),
            Some(false)
        );
        let corner = props(&[
            ("facing", "north"),
            ("half", "bottom"),
            ("shape", "inner_left"),
        ]);
        assert_eq!(
            face_support("minecraft:stone_brick_stairs", &corner, "north"),
            None
        );
    }

    /// **The interaction that belongs to neither half of this integration.**
    ///
    /// The `stair-shape` gate (`DW0801`) judges a *claim*: a stair that writes
    /// no `shape` makes none, so nothing can disagree with it, and
    /// `prefab-procedure.md` says so. This module needs a *fact* — which of the
    /// stair's faces are full — and a stair with no `shape` has none to give.
    ///
    /// So the two are not in conflict but they are not independent either, and
    /// the boundary is here: a shape-less stair standing alone is fine, and a
    /// shape-less stair with a fence beside it is a red naming the cell. It is
    /// latent in the shipped library (every stair there writes `shape`, `facing`
    /// and `half`), which is exactly why it is pinned rather than left to be
    /// discovered by the first piece that stops.
    #[test]
    #[should_panic(expected = "no verdict on whether that block presents a full face")]
    fn a_stair_that_writes_no_shape_refuses_rather_than_guessing_its_faces() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:oak_fence", &[]);
        cell(
            &mut cells,
            [1, 0, 0],
            "minecraft:stone_brick_stairs",
            // `facing` and `half` are written; `shape` is not. Vanilla derives
            // it on the first block update, which is precisely why the file can
            // omit it and precisely why this module cannot read it back.
            &[("facing", "north"), ("half", "bottom")],
        );
        resolved(&cells);
    }

    /// The same stair with nothing beside it that joins is never asked, so it
    /// passes — the refusal above is about the pair, not about the stair.
    #[test]
    fn a_stair_that_writes_no_shape_is_fine_when_nothing_joins_it() {
        let mut cells = Cells::new();
        cell(
            &mut cells,
            [0, 0, 0],
            "minecraft:stone_brick_stairs",
            &[("facing", "north"), ("half", "bottom")],
        );
        cell(&mut cells, [1, 0, 0], "minecraft:stone", &[]);
        let out = resolved(&cells);
        assert_eq!(out.len(), 2);
    }

    /// A jigsaw is a full cube in the file and air in the world, so nothing
    /// joins it.
    #[test]
    fn nothing_joins_a_jigsaw() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:iron_bars", &[]);
        cell(
            &mut cells,
            [1, 0, 0],
            "minecraft:jigsaw",
            &[("orientation", "east_up")],
        );
        assert_eq!(resolved(&cells)[&[0, 0, 0]].1["east"], "false");
    }

    /// Determinism (ADR-0006): the pass is a pure function of the cells, so two
    /// runs over independently-built inputs agree exactly, palette order
    /// included.
    #[test]
    fn resolving_twice_gives_the_same_palette_and_states() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:stone_bricks", &[]);
        for x in 1..6 {
            cell(&mut cells, [x, 0, 0], "minecraft:iron_bars", &[]);
        }
        cell(&mut cells, [6, 0, 0], "minecraft:stone_bricks", &[]);
        let mut a = piece_of(&cells);
        let mut b = piece_of(&cells);
        resolve("fixture", &mut a);
        resolve("fixture", &mut b);
        assert_eq!(a.palette, b.palette);
        assert_eq!(a.states, b.states);
    }

    /// The prune: the bare entry this pass replaced does not survive in the
    /// palette describing nothing.
    #[test]
    fn a_palette_entry_nothing_places_is_dropped() {
        let mut cells = Cells::new();
        cell(&mut cells, [0, 0, 0], "minecraft:oak_fence", &[]);
        let mut p = piece_of(&cells);
        assert_eq!(p.palette.len(), 1);
        resolve("fixture", &mut p);
        assert_eq!(p.palette.len(), 1, "replaced, not accumulated");
        assert!(p.palette[0].1.contains_key("north"));
    }
}
