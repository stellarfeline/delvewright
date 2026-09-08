//! Deterministic generator for the gallery campaign's one piece (spec-0039 §6).
//!
//! Emits **both halves** of the piece into `<out_dir>`:
//!
//! - `gallery-hall.nbt` — the structure template (gzip-framed Java NBT);
//! - `gallery-hall.json` — its prefab metadata: the anchor inventory, the
//!   lighting profile, the licence.
//!
//! Emitting the metadata here rather than committing it beside the generator is
//! the point, and it fixes a wart the older generators carry in a doc comment:
//! `hello-room-gen` says "the anchors this structure provides are declared
//! beside it in `hello-room.json` and **must be kept in sync** with the geometry
//! below". A rule enforced by a sentence is the shape CLAUDE.md calls UNRUN. Here
//! the anchor table below is the single authority: the geometry is carved around
//! it and the metadata is printed from it, so an anchor cannot come to name a
//! cell that is solid stone, and [`assert_anchors_are_standable`] proves that on
//! every run rather than trusting the carving.
//!
//! ```text
//! cargo run --release --manifest-path prefabs/gallery-generator/Cargo.toml -- <out_dir>
//! ```
//!
//! Unlike the tileset generators, `<out_dir>` here is a **build directory**, not
//! the content library: spec-0039 §6 keeps the gallery buildable from this
//! repository alone, so its piece is generated into the build tree on every run
//! and no `.nbt` is ever committed.
//!
//! ADR-0006: no wall-clock, fixed iteration order, gzip mtime pinned to 0.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use flate2::{Compression, GzBuilder};
use serde::Serialize;

/// The cross-tileset invariants and the connection derivation, shared as a
/// crate so the rule is compiled once and its own tests run with the
/// generators' (`prefabs/invariants`).
use prefab_invariants::{connections, document, invariants, walkplane, waterline};

/// MC 1.21.11 data version (ADR-0009).
const DATA_VERSION: i32 = 4671;

/// The piece's id — the `.nbt`/`.json` stem and what the invariants report against.
const ID: &str = "gallery-hall";

/// Structure extent: 31 (x) × 8 (y) × 31 (z).
///
/// One room, deliberately. The gallery's job is to be exhaustive over the DSL
/// and **legible**, and a maze of chambers would make the second impossible: a
/// reader looking up where `anchor/hearth` is should find it on one floor plan.
/// Everything vertical the DSL can express is expressed against the same floor.
const SIZE: [i32; 3] = [31, 8, 31];

/// The z of the dividing wall that gives the hall a far side worth opening a
/// gate onto. Everything with `z < DIVIDER_Z` is the near hall (spawn, the two
/// speaking parts, the pedestal); everything beyond it is the far hall.
const DIVIDER_Z: i32 = 15;

/// The openings in the divider, as `(x_from, x_to)` inclusive. Each is
/// three tall (`y ∈ 1..=3`) and filled with iron bars, which is what a
/// prefab-declared gate anchor opens.
const GATES: [(i32, i32); 5] = [(14, 15), (24, 25), (4, 5), (9, 10), (19, 20)];

/// One named place in the hall.
///
/// Two of its keys read as the same question and are not. **`note` is prose for
/// a person** — one line saying what the gallery does with this place, printed
/// so a creator reading the piece can tell the anchors apart without the
/// campaign in hand; the prefab document models the key
/// (`delvewright_dsl::prefab::Anchor::note`) and no reader acts on what it
/// says. **`role` is a term in the engine's own closed vocabulary**
/// (`AnchorRole`), which the compiler resolves when it has to find a place
/// without being told its name.
///
/// They are separate keys because they answer to separate readers, and the one
/// time they shared a name it cost this: the prose sat under `role` while the
/// engine modelled no such key, so it landed in the anchor's unknown-key
/// catch-all and every build reported it as `DW0543`. The moment `role` became
/// a modelled enum, the sentence stopped being an unknown key and became a
/// malformed value — which skips the WHOLE file (`DW0346`), taking the three
/// gate anchors with it and surfacing as three `DW0343`s that named the gates
/// and never mentioned the cause.
struct Anchor {
    name: &'static str,
    /// A point anchor: the cell a body stands in.
    pos: [i32; 3],
    /// The compass facing a body placed here takes, where the place has one.
    facing: Option<&'static str>,
    /// The block a `trap` at this anchor triggers on. Declared only where a trap
    /// really sits, and it is what makes a FLAG-GATED trap sound (`DW0363`): a
    /// gate removes the trigger from the world while it is shut, which is safe
    /// for a plate or a wire and destroys a trapped chest's inventory.
    trigger_block: Option<&'static str>,
    /// Prose, for a person reading the piece. See the type's own note.
    note: &'static str,
    /// A term of the engine's vocabulary, for the compiler. `None` on every
    /// place the compiler is always told the name of, which is all but one.
    role: Option<&'static str>,
}

/// A region anchor — a gate. `from`/`to` are inclusive local corners and the
/// whole span is filled with `block`.
struct GateAnchor {
    name: &'static str,
    from: [i32; 3],
    to: [i32; 3],
    note: &'static str,
}

/// **The anchor inventory, written once.**
///
/// Named by PURPOSE rather than by coordinate. A grid of `anchor/plot-7`s would
/// bind exactly the same units and tell a reader nothing, and legibility is a
/// property the gallery is required to have, not a nicety: the point of the
/// artifact is that a creator can go from an element to the surface it
/// exercises. So every anchor here says what the gallery does with it.
const ANCHORS: &[Anchor] = &[
    // The arrival, and the DECOY that stands behind it. Read as a pair: they are
    // the gallery's one instance of *how the compiler finds a place it was never
    // told the name of*, and neither of them means anything without the other.
    //
    // `anchor/arrival` is named like every other place in the hall — for what the
    // gallery does with it — and it is the entry only because it SAYS it is. That
    // is the whole of spec-0046: the name is the piece author's business, the role
    // is the engine's, and a producer whose anchor keys are always `anchor/<stem>`
    // (the grammar back end) can declare an entry without being able to spell one.
    Anchor {
        name: "anchor/arrival",
        pos: [15, 1, 2],
        facing: Some("south"),
        trigger_block: None,
        note: "where the party arrives — the entry, declared by ROLE and not by name",
        role: Some("entry"),
    },
    // `spawn` is the decoy: the spelling the compiler used to fall back to, now
    // an anchor like any other. It stands ten cells west of the real arrival, on
    // the same strip of floor, and NOTHING in the campaign binds it.
    //
    // It is here so that "a name supplies no entry point" is a fact this campaign
    // proves rather than a sentence in a doc comment. Delete the `role` above and
    // the hall stops being a place a body can arrive in at all: measured, the
    // build is REFUSED at exit 3 with `DW0873` — the campaign then starts in
    // `area/annex`, whose piece still declares a role, and its first leg becomes
    // a crossing into a hall nothing can put the party down in. It does not
    // quietly move the start ten blocks west onto this cell, which is exactly
    // what it did while a name could supply an entry. That is the perturbation
    // this element answers: a gallery element that cannot fail when the surface
    // it covers is removed is coverage in name only.
    //
    // Where it stands is chosen so that the campaign BUILDS with it present, and
    // both halves of that were measured rather than guessed. Ten cells down the
    // near hall's centre line keeps it dead ahead of `anchor/shortcut-door`, which
    // is what keeps `DW0374`'s proof true (opening the shortcut must shorten the
    // walk from the campaign entry to its own unlock). At the mouth of one of the
    // other four gates — `[5, 1, 2]` was tried — it reds that proof instead.
    Anchor {
        name: "spawn",
        pos: [15, 1, 12],
        facing: Some("south"),
        trigger_block: None,
        note: "the old compatibility spelling, standing where the party must NOT arrive: \
               a name the compiler no longer reads, so this cell is never the entry",
        role: None,
    },
    Anchor {
        name: "anchor/lectern",
        pos: [10, 1, 5],
        facing: Some("north"),
        trigger_block: None,
        note: "the speaking part: dialogue, barks, the cast ledger",
        role: None,
    },
    Anchor {
        name: "anchor/warden",
        pos: [20, 1, 5],
        facing: Some("north"),
        trigger_block: None,
        note: "the second speaking part, so a root SWAP has somewhere to go",
        role: None,
    },
    Anchor {
        name: "anchor/pedestal",
        pos: [15, 1, 9],
        facing: Some("north"),
        trigger_block: None,
        note: "the thing a player presses: an `interact` objective's affordance",
        role: None,
    },
    Anchor {
        name: "anchor/label",
        pos: [14, 1, 9],
        facing: Some("north"),
        trigger_block: None,
        note: "the second thing a player presses, one cell west of the pedestal \
               so the click trigger has a hitbox of its own. An objective's \
               affordance and a trigger's are two `minecraft:interaction` boxes; \
               on one cell they are coincident, the entity-pick ray ties, and \
               neither can be clicked (DW0878). One press, one anchor",
        role: None,
    },
    Anchor {
        name: "anchor/hearth",
        pos: [5, 1, 9],
        facing: Some("east"),
        trigger_block: None,
        note: "the respawn point — a bonfire and a plain checkpoint alike",
        role: None,
    },
    Anchor {
        name: "anchor/hearth-stone",
        pos: [6, 1, 9],
        facing: Some("west"),
        trigger_block: None,
        note: "the stone beside the hearth, where the `strike` trigger hangs. \
               A bonfire arms an interaction box of its own on `anchor/hearth`, \
               and the entity-pick ray chooses a box before the client reads \
               which button was pressed — so a left-click trigger on that same \
               cell would lose its swings to the rest point (DW0878). One press, \
               one anchor",
        role: None,
    },
    Anchor {
        name: "anchor/stall",
        pos: [23, 1, 9],
        facing: Some("west"),
        trigger_block: None,
        note: "the shop counter: offers, stakes, forfeits",
        role: None,
    },
    Anchor {
        name: "anchor/counter",
        pos: [26, 1, 9],
        facing: Some("west"),
        trigger_block: None,
        note: "the shop affordance. Three blocks EAST of the stall: clear of \
               the vendor body (DW0359), and standing where the critical path \
               already looks, so the POV shot that ends at the counter frames \
               something the campaign declares instead of empty floor",
        role: None,
    },
    Anchor {
        name: "anchor/muster",
        pos: [15, 1, 19],
        facing: Some("south"),
        trigger_block: None,
        note: "where a wave is seated, in the open middle of the far hall. A \
               wave takes the standable cells NEAREST its anchor, and standable \
               is judged for a body one cell wide — so an anchor with a wall two \
               cells off seats the 1.4-wide member of the stack against it and \
               the census reads a mob below full health before the fight starts. \
               Two cells of air on every side, and clear of the mezzanine and of \
               the loft's own completion box. Held off both killing volumes by \
               MUSTER_PIT_CLEARANCE and off the patrol by LANE_MUSTER_CLEARANCE",
        role: None,
    },
    Anchor {
        name: "anchor/march",
        pos: [15, 1, 24],
        facing: Some("south"),
        trigger_block: Some("minecraft:stone_pressure_plate"),
        note: "the far end of a lane's march",
        role: None,
    },
    Anchor {
        name: "anchor/west-bay",
        pos: [5, 1, 22],
        facing: Some("east"),
        trigger_block: Some("minecraft:tripwire[attached=true]"),
        note: "a room-sized volume: lethal boxes, teleport boxes, region edits",
        role: None,
    },
    Anchor {
        name: "anchor/east-bay",
        pos: [25, 1, 22],
        facing: Some("west"),
        trigger_block: None,
        note: "the second volume, so a pair of region verbs never share a box",
        role: None,
    },
    Anchor {
        name: "anchor/pocket",
        pos: [26, 1, 3],
        facing: Some("west"),
        trigger_block: None,
        note: "inside the barrier pocket. Its only way in is the full-cube course \
               in the wall line, so a body that walks here has crossed a line its \
               species is not allowed through — which is the whole of what a \
               `traversal` declaration answers. Off the critical path on purpose: \
               blocking geometry on the route makes the build's render plan and \
               the one `snapshot` derives disagree",
        role: None,
    },
    Anchor {
        name: "anchor/rafters",
        pos: [21, 1, 27],
        facing: Some("west"),
        trigger_block: None,
        note: "where the ambush stages, 24 blocks from the hearth because a body \
               inside a respawn point's aggro radius is DW0478",
        role: None,
    },
    Anchor {
        name: "anchor/lane-west",
        pos: [5, 1, 29],
        facing: Some("east"),
        trigger_block: None,
        note: "one end of the patrol lane — 20 blocks from its partner, because a \
               leg under 12 is one vanilla re-rolls off the lane (DW0386). On the \
               hall's back wall, not across its middle: the squad's perception is \
               its lane's `aggro_radius`, so a lane drawn through the muster room \
               puts a second, unmeasured fight inside the one the floor gate is \
               measuring. Held off the muster by LANE_MUSTER_CLEARANCE",
        role: None,
    },
    Anchor {
        name: "anchor/lane-east",
        pos: [25, 1, 29],
        facing: Some("west"),
        trigger_block: None,
        note: "the other end of the patrol lane",
        role: None,
    },
    Anchor {
        name: "anchor/west-pit",
        pos: [2, 1, 3],
        facing: None,
        trigger_block: None,
        note: "a killing volume with nothing posted in it (DW0511), in the near \
               hall's west corner — the deadest floor in the piece, four blocks \
               clear of anything that stands or walks there. It is in the near \
               hall and not in a bay because a killing volume may not share a \
               room with a fight: see MUSTER_PIT_CLEARANCE",
        role: None,
    },
    Anchor {
        name: "anchor/east-pit",
        pos: [20, 1, 2],
        facing: None,
        trigger_block: None,
        note: "the second killing volume, so the two never share a box: a strip \
               against the near hall's north wall, east of the arrival, off every \
               route the piece's own bodies are teleported along. Its `extent` is \
               0 on z, so it is one cell deep and the wall behind it is not part \
               of it",
        role: None,
    },
    Anchor {
        name: "anchor/vantage",
        pos: [15, 1, 27],
        facing: Some("north"),
        trigger_block: None,
        note: "where a camera stands to look back down the hall",
        role: None,
    },
    // The three levers on the back wall. Every one of them is a
    // compiler-summoned interaction box, so every one of them needs a cell
    // nothing else claims (DW0878) — a row of levers is what a room with three
    // things to unlatch actually looks like, and it is the shape the engine's
    // one-press-one-anchor rule produces.
    Anchor {
        name: "anchor/side-lever",
        pos: [14, 1, 27],
        facing: Some("north"),
        trigger_block: None,
        note: "the far-side bar of the souls shortcut: beyond the gate on the \
               axis the door is thin on, which is what lets the compiler name \
               the sealed side (DW0425)",
        role: None,
    },
    Anchor {
        name: "anchor/plate-lever",
        pos: [13, 1, 27],
        facing: Some("north"),
        trigger_block: None,
        note: "the trap's disarm: reachable from the hall while the plate at \
               the march is still live",
        role: None,
    },
    Anchor {
        name: "anchor/clock-lever",
        pos: [12, 1, 27],
        facing: Some("north"),
        trigger_block: None,
        note: "the timed gate's disarm, which has to be reachable while that \
               gate is still SHUT",
        role: None,
    },
    Anchor {
        name: "anchor/loft",
        pos: [19, LOFT_TOP_Y + 1, 18],
        facing: Some("south"),
        trigger_block: None,
        note: "on top of the mezzanine, which the broken flight is the ONLY way \
               onto. Three courses above the floor and a body steps one, so \
               anything staged here is content the campaign has to open a way to \
               — which is what makes the way load-bearing instead of scenery",
        role: None,
    },
    // Kept clear of the outer wall on purpose: a POV camera stands on an anchor
    // and looks along the leg it is walking, so an anchor one or two blocks from
    // a wall renders the wall — a flat frame, or one framing nothing declared.
    Anchor {
        name: "anchor/exit",
        pos: [11, 1, 25],
        facing: Some("south"),
        trigger_block: None,
        note: "the finale: the last thing a player reaches",
        role: None,
    },
];

/// **Container anchors** — the cells that hold a chest.
///
/// A `loot[]` fill and a `collect` objective's `container` both need a cell that
/// really is a fillable container, and a point anchor cannot be one: a body
/// stands in air and a chest is a block. So they are their own table, checked by
/// [`assert_anchors_are_standable`] against `minecraft:chest` rather than
/// against air — the standable rule would refuse every entry here for being
/// solid, correctly and uselessly.
const CONTAINERS: &[Anchor] = &[
    Anchor {
        name: "anchor/case",
        pos: [16, 1, 9],
        facing: Some("north"),
        trigger_block: None,
        note: "the case a `collect` objective is filled from",
        role: None,
    },
    Anchor {
        name: "anchor/reliquary",
        pos: [16, 1, 27],
        facing: Some("north"),
        trigger_block: None,
        note: "the chest a `loot` declaration fills",
        role: None,
    },
];

/// **Solid anchors** — cells that are deliberately stone.
///
/// `collapse` needs a slab of blocks to bring down and refuses both an empty
/// region (`DW0444`: "nothing would fall") and one hanging over open air
/// ("nothing beneath stops the debris"). So the hall carries a stone canopy over
/// the east bay, and this anchor points at it. A point anchor cannot serve: the
/// standable rule demands air, and this rule demands the opposite.
const SOLID_ANCHORS: &[Anchor] = &[Anchor {
    name: "anchor/east-vault",
    pos: [25, 5, 22],
    facing: None,
    trigger_block: None,
    note: "the stone canopy a `collapse` brings down onto the east bay floor",
    role: None,
}];

/// The canopy the collapse anchor points at: `(x0, x1, y, z0, z1)`, inclusive.
const CANOPY: (i32, i32, i32, i32, i32) = (22, 28, 5, 19, 25);

/// **The mezzanine** — a solid dais in the far hall's west corner, three courses
/// tall, whose top is the only floor in the piece a body cannot walk onto.
///
/// It is solid rather than a slab on legs on purpose: a mezzanine with a room
/// under it would roof floor the ceiling lanterns light, and the piece's
/// lighting profile is a claim about every walkable cell. A dais shades nothing
/// because there is nothing under it.
///
/// `(x0, x1, z0, z1)`, inclusive. Solid for `y ∈ 1..=LOFT_TOP_Y`.
const LOFT: (i32, i32, i32, i32) = (18, 21, 17, 20);

/// The dais's top course. A body on the mezzanine stands at `LOFT_TOP_Y + 1`.
const LOFT_TOP_Y: i32 = 3;

/// **The broken flight** — the stair that climbs the dais, with its treads
/// missing (spec-0042).
///
/// `(x0, x1)` is the flight's width and `(z0, z1)` its run; the two tread
/// courses are `TREADS`. As shipped those cells are AIR, so the mezzanine is
/// three courses above a body that can step one — severed, provably, on the
/// bytes that ship. A campaign's `open-way` lays them, and the same climb is
/// then four one-block steps.
const FLIGHT: (i32, i32, i32, i32) = (19, 20, 21, 22);

/// The tread courses the flight is missing, as `(y, z)` — the cells a laid way
/// fills and a body steps onto. Ordered bottom-first, which is the order the
/// exported metadata lists them in.
const TREADS: [(i32, i32); 2] = [(1, 22), (2, 21)];

/// What the treads are laid in. Deliberately not the hall's own stone: a player
/// who repairs the flight should be able to see which courses they put back.
const TREAD_BLOCK: &str = "minecraft:stone_bricks";

/// The way's name — what an `open-way` effect addresses, and what the piece's
/// contract exports it as.
const WAY_NAME: &str = "broken-flight";

/// The stair edge's transit volume: the flight's own airspace, treads included.
/// A way region must lie inside its edge's opening (spec-0042 §2.1), so this is
/// the box the tread courses are carved out of.
const FLIGHT_VIA: (i32, i32, i32, i32, i32, i32) = (FLIGHT.0, FLIGHT.1, 1, 3, FLIGHT.2, FLIGHT.3);

/// The gate inventory. Every opening is a real hole in the divider, so an
/// unopened gate really does stop a body and `DW0311` has something to prove.
const GATE_ANCHORS: &[GateAnchor] = &[
    GateAnchor {
        name: "anchor/gate-main",
        from: [4, 1, DIVIDER_Z],
        to: [5, 3, DIVIDER_Z],
        note: "the long way through, off in the west corner: opened by quest progress",
    },
    GateAnchor {
        name: "anchor/shortcut-door",
        from: [14, 1, DIVIDER_Z],
        to: [15, 3, DIVIDER_Z],
        note: "the souls shortcut: dead ahead of spawn, barred until it is unlocked \
               from the far side — which is what makes opening it SHORTEN the walk \
               (DW0374), where a second door beside the long one would not",
    },
    GateAnchor {
        name: "anchor/timed-door",
        from: [24, 1, DIVIDER_Z],
        to: [25, 3, DIVIDER_Z],
        note: "the gate on a clock: opens and re-seals on its own cycle",
    },
    // Three clocked gates, not one, and the LETHAL one is last. A per-object
    // mechanic's defects only exist at cardinality ≥2 — the runtime suite bound
    // `timed_gates.first()` and watched exactly one gate per campaign, so a
    // level whose crushing gate was its third shipped with no runtime proof of
    // the only thing in it that could kill a player. The gallery bound
    // `timed_gates` once, which is why it could not catch that; the coverage
    // gate counts UNITS, and a cardinality is not a unit.
    GateAnchor {
        name: "anchor/timed-door-mid",
        from: [9, 1, DIVIDER_Z],
        to: [10, 3, DIVIDER_Z],
        note: "the second gate on a clock, carrying nothing special — it is here so \
               the suite has a middle member to skip, which is how one-of-N coverage \
               shows up at all",
    },
    GateAnchor {
        name: "anchor/timed-door-inner",
        from: [19, 1, DIVIDER_Z],
        to: [20, 3, DIVIDER_Z],
        note: "the LAST gate on a clock, and the only one that crushes: the \
               consequential member, declared last, where a walk that stops at the \
               first member never reaches it",
    },
];

/// Ceiling-hung lanterns on a 6-block grid, so every walkable floor cell clears
/// the lighting contract's `lit` bar. Derived rather than listed: a hand-written
/// list at this size is a list that goes stale the first time `SIZE` moves.
fn lanterns() -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    let mut z = 3;
    while z < SIZE[2] - 1 {
        let mut x = 3;
        while x < SIZE[0] - 1 {
            if z != DIVIDER_Z {
                out.push([x, SIZE[1] - 2, z]);
            }
            x += 6;
        }
        z += 6;
    }
    // The mezzanine's own fixture. The grid is derived from `SIZE` and knows
    // nothing about a floor three courses up, so its nearest lantern leaves the
    // dais's far corner at light 7 — one under the `lit` bar the metadata
    // claims. A floor the grid does not reach carries its own light rather than
    // the profile carrying a claim it cannot meet.
    out.push([19, SIZE[1] - 2, 18]);
    out
}

#[derive(Serialize)]
struct Structure {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    size: [i32; 3],
    palette: Vec<PaletteEntry>,
    blocks: Vec<BlockEntry>,
    entities: Vec<Entity>,
}

#[derive(Serialize, PartialEq, Eq, Clone)]
struct PaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties", skip_serializing_if = "Option::is_none")]
    properties: Option<BTreeMap<String, String>>,
}

#[derive(Serialize)]
struct BlockEntry {
    pos: [i32; 3],
    state: i32,
}

/// The structure carries no entities — every body is summoned by the datapack.
#[derive(Serialize)]
struct Entity {}

struct Palette {
    entries: Vec<PaletteEntry>,
}

impl Palette {
    fn new() -> Self {
        Palette { entries: vec![] }
    }

    fn idx(&mut self, name: &str, props: Option<&[(&str, &str)]>) -> i32 {
        let written: BTreeMap<String, String> = props
            .unwrap_or(&[])
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        let e = PaletteEntry {
            name: name.to_string(),
            properties: if written.is_empty() {
                None
            } else {
                Some(written)
            },
        };
        if let Some(i) = self.entries.iter().position(|x| *x == e) {
            return i as i32;
        }
        self.entries.push(e);
        (self.entries.len() - 1) as i32
    }
}

/// `minecraft:tripwire[attached=true]` -> `("minecraft:tripwire", [("attached",
/// "true")])`.
///
/// One spelling of a block state, read where it is declared. The alternative was
/// a second constant beside [`Anchor::trigger_block`] holding the same block in
/// a different shape, which is two authorities on one fact.
fn split_state(declared: &'static str) -> (&'static str, Vec<(&'static str, &'static str)>) {
    match declared.split_once('[') {
        None => (declared, Vec::new()),
        Some((name, rest)) => (
            name,
            rest.trim_end_matches(']')
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|term| {
                    term.split_once('=')
                        .unwrap_or_else(|| panic!("{ID}: `{declared}` is not a block state"))
                })
                .collect(),
        ),
    }
}

fn block_at(
    x: i32,
    y: i32,
    z: i32,
    lantern_cells: &[[i32; 3]],
) -> (
    &'static str,
    Option<&'static [(&'static str, &'static str)]>,
) {
    if y == 0 || y == SIZE[1] - 1 {
        return ("minecraft:stone", None);
    }
    if x == 0 || x == SIZE[0] - 1 || z == 0 || z == SIZE[2] - 1 {
        return ("minecraft:stone", None);
    }
    if z == DIVIDER_Z {
        for (from_x, to_x) in GATES {
            if (from_x..=to_x).contains(&x) && (1..=3).contains(&y) {
                return ("minecraft:iron_bars", None);
            }
        }
        return ("minecraft:stone", None);
    }
    let (cx0, cx1, cy, cz0, cz1) = CANOPY;
    if y == cy && (cx0..=cx1).contains(&x) && (cz0..=cz1).contains(&z) {
        return ("minecraft:stone", None);
    }
    // The mezzanine: solid to its top course, and nothing carves a way into it.
    // The flight that climbs it is outside this footprint entirely, which is
    // what makes the break a break — remove the treads and the dais's south
    // face is a plain three-course wall.
    let (lx0, lx1, lz0, lz1) = LOFT;
    if (lx0..=lx1).contains(&x) && (lz0..=lz1).contains(&z) && (1..=LOFT_TOP_Y).contains(&y) {
        return ("minecraft:stone", None);
    }
    if CONTAINERS.iter().any(|a| a.pos == [x, y, z]) {
        // Facing north, so the lid opens toward the walker coming down the hall.
        return (
            "minecraft:chest",
            Some(&[("facing", "north"), ("type", "single")]),
        );
    }
    if lantern_cells.iter().any(|p| p == &[x, y, z]) {
        return ("minecraft:lantern", Some(&[("hanging", "true")]));
    }
    ("minecraft:air", None)
}

fn build() -> Structure {
    let mut palette = Palette::new();
    for (name, props) in [
        ("minecraft:air", None),
        ("minecraft:stone", None),
        ("minecraft:iron_bars", None),
        ("minecraft:lantern", Some(&[("hanging", "true")][..])),
        (
            "minecraft:chest",
            Some(&[("facing", "north"), ("type", "single")][..]),
        ),
    ] {
        palette.idx(name, props);
    }
    let lantern_cells = lanterns();
    let mut blocks = Vec::with_capacity((SIZE[0] * SIZE[1] * SIZE[2]) as usize);
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                let (name, props) = block_at(x, y, z, &lantern_cells);
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: palette.idx(name, props),
                });
            }
        }
    }
    // **A trap's trigger is HARDWARE the piece wires**, not a sentence the
    // document tells the compiler. A flag gate physically removes that block
    // while it is shut and puts it back verbatim when it opens, so a
    // `trigger_block` over an air cell makes opening the gate create a block the
    // piece never had — which is what `DW0888` refuses. Every anchor that
    // declares one gets it here, from the same constant the metadata is written
    // from, so the two halves cannot say different things.
    let mut triggers = 0usize;
    for a in ANCHORS {
        let Some(declared) = a.trigger_block else {
            continue;
        };
        let (name, props) = split_state(declared);
        let refs: Vec<(&str, &str)> = props.iter().map(|(k, v)| (*k, *v)).collect();
        let state = palette.idx(name, (!refs.is_empty()).then_some(&refs[..]));
        let cell = blocks
            .iter_mut()
            .find(|b| b.pos == a.pos)
            .unwrap_or_else(|| panic!("{ID}: anchor `{}` is outside the piece", a.name));
        cell.state = state;
        triggers += 1;
    }
    assert!(
        triggers > 0,
        "{ID}: no anchor declares a `trigger_block`, so the trap-hardware surface is unbound"
    );
    println!("{ID}: wired {triggers} declared trap trigger(s) into the blocks");
    for (i, e) in palette.entries.iter().enumerate() {
        assert!(
            blocks.iter().any(|b| b.state == i as i32),
            "{ID}: palette entry {i} ({}) is referenced by no cell",
            e.name
        );
    }
    Structure {
        data_version: DATA_VERSION,
        size: SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

/// **The gate the metadata's own doc comment could not be.**
///
/// Every point anchor must name a cell a body can occupy — air, with air above
/// it and something solid under it — and every gate anchor must be filled with
/// the block its metadata claims. Without this the two halves this program
/// emits are still two documents that can disagree; they simply disagree inside
/// one file instead of across two, which is not an improvement.
///
/// A failure here is a panic, so the generator writes nothing: an anchor
/// inventory that does not describe the blocks beside it is not a piece with a
/// documentation problem, it is a piece the compiler will place bodies inside
/// stone from.
fn assert_anchors_are_standable(s: &Structure) {
    let at = |p: [i32; 3]| -> &str {
        let cell = s
            .blocks
            .iter()
            .find(|b| b.pos == p)
            .unwrap_or_else(|| panic!("{ID}: anchor cell {p:?} is outside the piece"));
        s.palette[cell.state as usize].name.as_str()
    };
    for a in ANCHORS {
        let [x, y, z] = a.pos;
        // Air, or the trap trigger this very anchor declares. A pressure plate
        // and a tripwire are the two blocks a body stands ON rather than in, and
        // an anchor that declares one is an anchor whose own cell holds it —
        // demanding air there would forbid the hardware the document promises.
        let want = a.trigger_block.map(|b| split_state(b).0);
        assert_eq!(
            at([x, y, z]),
            want.unwrap_or("minecraft:air"),
            "{ID}: anchor `{}` stands in a solid cell",
            a.name
        );
        assert_eq!(
            at([x, y + 1, z]),
            "minecraft:air",
            "{ID}: anchor `{}` has no headroom",
            a.name
        );
        assert_ne!(
            at([x, y - 1, z]),
            "minecraft:air",
            "{ID}: anchor `{}` has no floor under it",
            a.name
        );
    }
    for a in CONTAINERS {
        assert_eq!(
            at(a.pos),
            "minecraft:chest",
            "{ID}: container anchor `{}` is not a chest",
            a.name
        );
    }
    for a in SOLID_ANCHORS {
        assert_eq!(
            at(a.pos),
            "minecraft:stone",
            "{ID}: solid anchor `{}` is not stone, so a collapse there would drop nothing",
            a.name
        );
    }
    for g in GATE_ANCHORS {
        for x in g.from[0]..=g.to[0] {
            for y in g.from[1]..=g.to[1] {
                for z in g.from[2]..=g.to[2] {
                    assert_eq!(
                        at([x, y, z]),
                        "minecraft:iron_bars",
                        "{ID}: gate `{}` claims a cell that is not its own block",
                        g.name
                    );
                }
            }
        }
    }
    // A metadata that declares nothing is the vacuous case: the assertions above
    // are all universally quantified and pass over an empty inventory.
    assert!(
        !ANCHORS.is_empty()
            && !GATE_ANCHORS.is_empty()
            && !CONTAINERS.is_empty()
            && !SOLID_ANCHORS.is_empty(),
        "{ID}: the anchor inventory is empty, so nothing above examined anything"
    );
    println!(
        "{ID}: anchor inventory bound — {} point anchor(s), {} container(s), \
         {} solid anchor(s), {} gate anchor(s) checked against the blocks",
        ANCHORS.len(),
        CONTAINERS.len(),
        SOLID_ANCHORS.len(),
        GATE_ANCHORS.len()
    );
}

/// The floor, in blocks, on how far a wave anchor stands from a killing
/// volume's own anchor — the weaker half of the rule below.
///
/// **Distance is not the rule, and this number is not where the safety comes
/// from.** A wave staged 4.72 blocks clear of the nearer box still lost a body
/// to it, and the bot fighting that wave died in it twice, because a fight does
/// not stay where it is staged: the census found a re-seated cohort spread
/// 14.4 blocks from its own anchor, and the unassisted attempt ended with the
/// bot dead at `[12, 65, 21]` — the cell OUTSIDE the volume whose 0.6-wide
/// occupant reaches the boundary. Any distance short of the room's own diameter
/// is a number a roaming fight walks through.
///
/// So the rule is the one [`assert_the_muster_clears_the_pits`] states first: a
/// killing volume does not share a ROOM with the fight a gate measures. The
/// hall is two rooms and the generator knows where the wall is, which is why
/// that half can be checked rather than assumed. This number stays as a second,
/// weaker guard for a volume that clears the wall by a cell and nothing else.
const MUSTER_PIT_CLEARANCE: f64 = 8.0;

/// How far the patrol lane must keep from the wave anchor the floor gate
/// measures, in blocks — measured to the lane's own SEGMENT, not to its ends.
///
/// A lane squad's `follow_range` is emitted verbatim from its lane's
/// `aggro_radius` (the compiler refuses a contradicting override, `DW0381`), so
/// the radius at which the patrol acquires a player is the campaign's own
/// number: eight. Ten leaves two blocks for a fight that moves. The ends can be
/// twenty blocks away and the LINE between them still cut through the muster,
/// which is exactly the shape this measures and an endpoint check would miss.
///
/// **Geometry is the belt, not the braces.** A perception radius of eight in a
/// hall fourteen deep covers the hall, so no line the patrol can walk keeps
/// eight blocks from the party's whole route — measured: with the lane on the
/// back wall the bot cleared the muster and then died to the same two crossbows
/// on its way to the finale, twice. What separates the two fights is that the
/// campaign arms the lane on the hall's CLOSING beat, which this program cannot
/// see. This constant still holds, because a staging that seats the two within
/// perception of each other is wrong however the beats are ordered, and an
/// ordering is the half a later edit can undo without touching a coordinate.
const LANE_MUSTER_CLEARANCE: f64 = 10.0;

/// Find one anchor by name, or panic: a clearance proof that silently examined
/// nothing is the vacuity this file exists to refuse.
fn anchor_at(name: &str) -> [i32; 3] {
    ANCHORS
        .iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| panic!("{ID}: no anchor named `{name}` to measure"))
        .pos
}

/// Squared distance in the floor plane, where every one of these clearances is
/// judged: a hall one storey tall gives `y` nothing to say.
fn plan_dist(a: [i32; 3], b: [i32; 3]) -> f64 {
    let (dx, dz) = ((a[0] - b[0]) as f64, (a[2] - b[2]) as f64);
    (dx * dx + dz * dz).sqrt()
}

/// **A killing volume does not share a room with a fight a gate measures.**
///
/// Measured on the bot ladder, twice, in that order. A muster staged on the bay
/// line lost its spider to `lethal/west-pit` mid-fight — a lethal volume
/// selects on HITBOX intersection, so a 1.4-wide body two cells outside the box
/// is inside it — and the floor gate then graded a wave the world had helped
/// kill. Moving the muster to the far end of the same room bought 4.72 blocks
/// of clearance and did not help: the bot chased the cohort across the hall and
/// died in the same volume, at the cell whose occupant merely reaches its edge.
///
/// A fight roams; a room is what bounds it. The hall is two rooms with one wall
/// between them, so this is checkable rather than assumed: the fight is beyond
/// the wall, and both volumes are in front of it.
fn assert_the_muster_clears_the_pits() {
    let muster = anchor_at("anchor/muster");
    let mut pits = Vec::new();
    for pit in ["anchor/west-pit", "anchor/east-pit"] {
        let at = anchor_at(pit);
        assert!(
            (muster[2] > DIVIDER_Z) != (at[2] > DIVIDER_Z),
            "{ID}: `{pit}` stands at z={} and `anchor/muster` at z={}, both on the \
             same side of the dividing wall (z={DIVIDER_Z}) — a fight roams the room \
             it is staged in, so a killing volume sharing that room takes bodies out \
             of the fight the floor gate is grading, and takes the party's own body \
             out of it too",
            at[2],
            muster[2]
        );
        let d = plan_dist(muster, at);
        assert!(
            d >= MUSTER_PIT_CLEARANCE,
            "{ID}: `anchor/muster` stands {d:.2} blocks from `{pit}`, under the \
             {MUSTER_PIT_CLEARANCE:.1} that keeps a volume off the wall the two \
             rooms share"
        );
        pits.push(format!("{pit} {d:.2}"));
    }
    assert_eq!(
        pits.len(),
        2,
        "{ID}: the pit clearance examined {} volume(s), not 2",
        pits.len()
    );
    println!(
        "{ID}: muster clearance bound — 2 killing volume(s), both across the wall at \
         z={DIVIDER_Z} from the muster: {} (floor {MUSTER_PIT_CLEARANCE:.1} block(s))",
        pits.join(", ")
    );
}

/// **A measured fight does not share its floor with a second fight.**
///
/// A patrol lane drawn across the hall's middle put two crossbows inside the
/// muster, unbilled and unmeasured — `DW0477` says in writing that nothing
/// measures `wave/lane`, so every bolt it fires into the muster is damage the
/// floor gate cannot account for.
///
/// Measured to the lane's SEGMENT: its two ends can be twenty blocks from the
/// muster while the line between them runs straight through it, which is what
/// an endpoint check would miss and what the staging that produced that run
/// actually did.
fn assert_the_lane_keeps_off_the_muster() {
    let muster = anchor_at("anchor/muster");
    let (w, e) = (anchor_at("anchor/lane-west"), anchor_at("anchor/lane-east"));
    let (ax, az) = ((w[0] - e[0]) as f64, (w[2] - e[2]) as f64);
    let len2 = ax * ax + az * az;
    assert!(len2 > 0.0, "{ID}: the patrol lane's two ends are one cell");
    let t = ((((muster[0] - e[0]) as f64) * ax + ((muster[2] - e[2]) as f64) * az) / len2)
        .clamp(0.0, 1.0);
    let near = [
        (e[0] as f64 + t * ax).round() as i32,
        muster[1],
        (e[2] as f64 + t * az).round() as i32,
    ];
    let d = plan_dist(muster, near);
    assert!(
        d >= LANE_MUSTER_CLEARANCE,
        "{ID}: the patrol lane passes {d:.2} blocks from `anchor/muster` at {near:?}, \
         under the {LANE_MUSTER_CLEARANCE:.1} that keeps the squad's own perception \
         radius out of the fight the floor gate measures"
    );
    println!(
        "{ID}: lane clearance bound — the patrol line passes `anchor/muster` at \
         {near:?}, {d:.2} block(s) away (floor {LANE_MUSTER_CLEARANCE:.1})"
    );
}

/// Every cell the broken flight is missing, as local coordinates — the way
/// region, in the order the metadata exports it.
fn tread_cells() -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    for (y, z) in TREADS {
        for x in FLIGHT.0..=FLIGHT.1 {
            out.push([x, y, z]);
        }
    }
    out
}

/// **The contract's own proof, run on the bytes this program is about to
/// write** (spec-0042 §2.1, the closed and open halves).
///
/// A hand-built piece carries a resolved `spatial_contract` the way an expanded
/// program does, and nothing downstream re-derives it: the compiler reads the
/// declaration and believes it. So the claim "the mezzanine is severed as
/// shipped, and laying `broken-flight` joins it" is proved HERE or it is proved
/// nowhere — which would be the same wart the anchor table exists to avoid, one
/// layer up.
///
/// Both halves are one walk over the piece's own blocks, run twice: once on the
/// bytes as they ship, once on a copy with the tread courses filled. The walk is
/// deliberately generous — a body steps up one course, falls any distance, and
/// needs two cells of headroom — because a generous walk failing to reach the
/// mezzanine is a stronger statement than a strict one failing to.
///
/// Rooted in the FAR hall rather than at `spawn`: the divider's iron bars are a
/// campaign-controlled gate, not part of this edge's claim, and threading them
/// here would make an assertion about the flight depend on how a gate is
/// written. What this proves is exactly what the stair edge declares.
fn assert_the_flight_is_broken(s: &Structure) {
    let mut cells: BTreeMap<[i32; 3], &str> = BTreeMap::new();
    for b in &s.blocks {
        cells.insert(b.pos, s.palette[b.state as usize].name.as_str());
    }

    // The closed direction, on the bytes as shipped: every tread cell really is
    // absent. A way declared over cells that are already solid is the
    // `barred`-shaped defect spec-0042 §7.3 names — the delta opens nothing.
    let treads = tread_cells();
    assert!(!treads.is_empty(), "{ID}: the way region is empty");
    for p in &treads {
        assert_eq!(
            cells.get(p).copied(),
            Some("minecraft:air"),
            "{ID}: way `{WAY_NAME}` claims cell {p:?}, which is not air as built — a laid way \
             fills what is not there, and this cell is already something"
        );
    }
    // Confinement (§2.1): the way lies inside its own edge's transit volume.
    let (vx0, vx1, vy0, vy1, vz0, vz1) = FLIGHT_VIA;
    for p in &treads {
        assert!(
            (vx0..=vx1).contains(&p[0])
                && (vy0..=vy1).contains(&p[1])
                && (vz0..=vz1).contains(&p[2]),
            "{ID}: way cell {p:?} lies outside the stair's transit volume"
        );
    }

    let laid: std::collections::BTreeSet<[i32; 3]> = treads.iter().copied().collect();
    let loft_anchor = ANCHORS
        .iter()
        .find(|a| a.name == "anchor/loft")
        .expect("the mezzanine declares its anchor")
        .pos;
    let foot = [FLIGHT.0, 1, FLIGHT.3 + 1];

    let shut = walk(&cells, &laid, false, foot);
    let open = walk(&cells, &laid, true, foot);
    assert!(
        shut.contains(&foot) && open.contains(&foot),
        "{ID}: the flight's foot at {foot:?} is not somewhere a body can stand, so neither half \
         of this proof examined anything"
    );
    assert!(
        !shut.contains(&loft_anchor),
        "{ID}: with `{WAY_NAME}` shut a body still reaches the mezzanine at {loft_anchor:?} — the \
         way does not open anything, and the contract's severance claim is false on the bytes \
         that ship"
    );
    assert!(
        open.contains(&loft_anchor),
        "{ID}: with `{WAY_NAME}` laid a body still cannot reach the mezzanine at \
         {loft_anchor:?} — the declared delta does not join the two ends"
    );
    println!(
        "{ID}: way `{WAY_NAME}` bound — {} tread cell(s) absent as built; {} stance(s) reachable \
         shut, {} laid, and the mezzanine is in the difference",
        treads.len(),
        shut.len(),
        open.len()
    );
}

/// The walk both halves of [`assert_the_flight_is_broken`] use.
///
/// A stance is a cell a body occupies: air, air above it, a full cube under it.
/// From one stance a body reaches a horizontally adjacent stance whose floor is
/// at most one course higher, or any distance lower with a clear column to fall
/// down. `laid` is the way region, treated as full cube when `open`.
fn walk(
    cells: &BTreeMap<[i32; 3], &str>,
    laid: &std::collections::BTreeSet<[i32; 3]>,
    open: bool,
    from: [i32; 3],
) -> std::collections::BTreeSet<[i32; 3]> {
    // A full cube a body stands on and cannot walk through. Iron bars and
    // lanterns are neither: the divider is not a floor and a lantern is not a
    // step.
    let full = |p: [i32; 3]| -> bool {
        if open && laid.contains(&p) {
            return true;
        }
        matches!(
            cells.get(&p).copied(),
            Some("minecraft:stone") | Some("minecraft:chest")
        )
    };
    let air = |p: [i32; 3]| -> bool {
        if open && laid.contains(&p) {
            return false;
        }
        cells.get(&p).copied() == Some("minecraft:air")
    };
    let stance = |p: [i32; 3]| -> bool {
        air(p) && air([p[0], p[1] + 1, p[2]]) && full([p[0], p[1] - 1, p[2]])
    };

    let mut seen = std::collections::BTreeSet::new();
    if !stance(from) {
        return seen;
    }
    let mut queue = std::collections::VecDeque::new();
    seen.insert(from);
    queue.push_back(from);
    while let Some(p) = queue.pop_front() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, nz) = (p[0] + dx, p[2] + dz);
            for ny in (1..=p[1] + 1).rev() {
                let q = [nx, ny, nz];
                if !stance(q) {
                    continue;
                }
                let reachable = if ny == p[1] + 1 {
                    // A step up needs the course above the body's head clear.
                    air([p[0], p[1] + 2, p[2]])
                } else {
                    // Level, or a fall down a clear column.
                    (ny + 2..=p[1] + 1).all(|y| air([nx, y, nz]))
                };
                if reachable && seen.insert(q) {
                    queue.push_back(q);
                }
                break;
            }
        }
    }
    seen
}

fn invariant_cells(s: &Structure) -> invariants::Cells {
    s.blocks
        .iter()
        .map(|b| {
            let p = &s.palette[b.state as usize];
            (
                b.pos,
                (p.name.clone(), p.properties.clone().unwrap_or_default()),
            )
        })
        .collect()
}

fn resolve_connections(id: &str, s: &mut Structure) {
    let mut piece = connections::Piece {
        palette: s
            .palette
            .iter()
            .map(|p| (p.name.clone(), p.properties.clone().unwrap_or_default()))
            .collect(),
        positions: s.blocks.iter().map(|b| b.pos).collect(),
        states: s.blocks.iter().map(|b| b.state as usize).collect(),
    };
    connections::resolve(id, &mut piece);
    s.palette = piece
        .palette
        .into_iter()
        .map(|(name, properties)| PaletteEntry {
            name,
            properties: (!properties.is_empty()).then_some(properties),
        })
        .collect();
    for (b, state) in s.blocks.iter_mut().zip(piece.states) {
        b.state = state as i32;
    }
}

/// One inclusive local box, as the contract writes it.
fn region(from: [i32; 3], to: [i32; 3]) -> serde_json::Value {
    serde_json::json!({ "from": from, "to": to })
}

/// **The piece's spatial contract** — what its own shape claims, resolved
/// against these exact bytes (spec-0036, spec-0042 §2.3).
///
/// Three spaces and two contingencies, one of each kind the surface has:
///
/// * `near-hall` — where the party arrives; the contract's entry.
/// * `far-hall` — beyond the divider, reached through a `barred` edge whose bar
///   is the iron in the three gate openings. Content opens that by voiding it,
///   which is the `cleared` direction.
/// * `loft` — the mezzanine's airspace, reached through a `stair` edge whose
///   treads are not there. Content opens that by filling it, which is the
///   `laid` direction, and it is the one a campaign addresses by name.
///
/// `faces` is deliberately absent: every edge here joins two of this piece's own
/// spaces, so the piece makes no claim about its outside and there is nothing
/// for a neighbour to mate with.
///
/// The space boxes are computed rather than listed because they are the
/// interior MINUS the two solid masses the mezzanine and its flight occupy, and
/// a hand-written decomposition of that is a list that goes stale the first
/// time a constant moves.
fn spatial_contract() -> serde_json::Value {
    use serde_json::{json, Map, Value};
    let (lx0, lx1, lz0, lz1) = LOFT;
    let (fx0, fx1, fz0, fz1) = FLIGHT;
    let (in0, in1) = (1, SIZE[0] - 2); // the interior, wall to wall
    let (top, floor) = (SIZE[1] - 2, 1);

    // A stair lands ON the dais, so the flight's run starts where the dais's
    // south face is. The decomposition below relies on that — with a gap
    // between them it would leave a strip of floor in no space at all, silently.
    assert_eq!(
        fz0,
        lz1 + 1,
        "{ID}: the flight does not abut the mezzanine it climbs"
    );
    // …and it lands within the dais's width rather than flush with an edge. A
    // flush flight collapses one of the strips below into an INVERTED box, which
    // `space_holding` reads through `min`/`max` — so it would silently claim the
    // dais's own solid cells for the far hall instead of failing.
    assert!(
        lx0 < fx0 && fx1 < lx1,
        "{ID}: the flight is flush with a side of the mezzanine, which the space \
         decomposition below cannot express"
    );
    // The far hall's walkable airspace, carved around the dais (solid, so in no
    // space at all) and around the flight's transit volume (the stair edge's,
    // not the room's).
    let far: Vec<Value> = vec![
        region([in0, floor, DIVIDER_Z + 1], [lx0 - 1, top, in1]),
        region([lx1 + 1, floor, DIVIDER_Z + 1], [in1, top, in1]),
        region([lx0, floor, DIVIDER_Z + 1], [lx1, top, lz0 - 1]),
        region([lx0, floor, fz0], [fx0 - 1, top, fz1]),
        region([fx1 + 1, floor, fz0], [lx1, top, fz1]),
        region([fx0, LOFT_TOP_Y + 1, fz0], [fx1, top, fz1]),
        region([lx0, floor, fz1 + 1], [lx1, top, in1]),
    ];

    let mut spaces = Map::new();
    spaces.insert(
        "near-hall".into(),
        json!({
            "envelope": "enclosed",
            "boxes": [region([in0, floor, in0], [in1, top, DIVIDER_Z - 1])],
        }),
    );
    spaces.insert(
        "far-hall".into(),
        json!({ "envelope": "enclosed", "boxes": far }),
    );
    spaces.insert(
        "loft".into(),
        json!({
            "envelope": "enclosed",
            "boxes": [region([lx0, LOFT_TOP_Y + 1, lz0], [lx1, top, lz1])],
        }),
    );

    let bar: Vec<Value> = GATE_ANCHORS.iter().map(|g| region(g.from, g.to)).collect();
    let (vx0, vx1, vy0, vy1, vz0, vz1) = FLIGHT_VIA;
    let treads: Vec<Value> = TREADS
        .iter()
        .map(|(y, z)| region([fx0, *y, *z], [fx1, *y, *z]))
        .collect();

    json!({
        "entry": "near-hall",
        "spaces": Value::Object(spaces),
        "edges": [
            {
                "a": "near-hall",
                "b": "far-hall",
                "class": "barred",
                "rise": 0,
                "via": { "region": "divider-openings", "boxes": bar.clone() },
                "bar": {
                    "region": "divider-iron",
                    "boxes": bar,
                    "block": "minecraft:iron_bars"
                }
            },
            {
                "a": "far-hall",
                "b": "loft",
                "class": "stair",
                "rise": LOFT_TOP_Y,
                "via": {
                    "region": "loft-flight",
                    "boxes": [region([vx0, vy0, vz0], [vx1, vy1, vz1])]
                },
                "way": {
                    "opens": "laid",
                    "region": WAY_NAME,
                    "boxes": treads,
                    "role": "tread",
                    "block": TREAD_BLOCK
                }
            }
        ]
    })
}

/// The prefab metadata, printed from the same tables the geometry was carved
/// around. `serde_json::Value` is built through a `BTreeMap` so key order is
/// fixed (ADR-0006) without depending on any preserve-order feature.
fn metadata() -> serde_json::Value {
    use serde_json::{json, Map, Value};
    let mut anchors = Map::new();
    for a in ANCHORS.iter().chain(CONTAINERS).chain(SOLID_ANCHORS) {
        let mut m = Map::new();
        m.insert("pos".into(), json!(a.pos));
        if let Some(f) = a.facing {
            m.insert("facing".into(), json!(f));
        }
        if let Some(t) = a.trigger_block {
            m.insert("trigger_block".into(), json!(t));
        }
        m.insert("note".into(), json!(a.note));
        if let Some(role) = a.role {
            m.insert("role".into(), json!(role));
        }
        anchors.insert(a.name.into(), Value::Object(m));
    }
    for g in GATE_ANCHORS {
        let mut m = Map::new();
        m.insert("region".into(), json!({ "from": g.from, "to": g.to }));
        m.insert("block".into(), json!("minecraft:iron_bars"));
        m.insert("note".into(), json!(g.note));
        anchors.insert(g.name.into(), Value::Object(m));
    }
    json!({
        "prefab_id": format!("prefab/{ID}"),
        "structure": {
            "file": format!("{ID}.nbt"),
            "id": ID,
            "size": SIZE,
            "data_version": DATA_VERSION,
            "generator": "prefabs/gallery-generator (gallery-prefab-gen)"
        },
        "anchors": Value::Object(anchors),
        "spatial_contract": spatial_contract(),
        "lighting": {
            "profile": "lit",
            "measured_min_light": 8,
            "measured": "2026-08-19",
            "method": "derived: ceiling-hung lanterns on a 6-block grid, roofed piece, \
                       same spacing and mounting as the measured `hello-room` grid"
        },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Original Delvewright project asset (pipeline-code license per \
                     prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            "provenance": "Generated deterministically by prefabs/gallery-generator \
                           (ADR-0006); regenerating yields byte-identical NBT and metadata."
        }
    })
}

fn write_piece(out: &Path) {
    let mut room = build();
    resolve_connections(ID, &mut room);
    assert_anchors_are_standable(&room);
    assert_the_muster_clears_the_pits();
    assert_the_lane_keeps_off_the_muster();
    assert_the_flight_is_broken(&room);
    // The room is designed against its own floor and then stands on the plinth
    // the sea needs: every proof above is about the room, every proof below is
    // about the piece that ships.
    let (s, meta) = to_shore(ID, &room, &metadata(), &HALL_POOL);
    let cells = invariant_cells(&s);
    invariants::assert_distress_never_stacks(ID, &cells);
    invariants::assert_blocks_are_real(ID, &cells);
    connections::assert_shape_is_stated(ID, &cells);
    connections::assert_attachments_are_supported(ID, &cells);
    invariants::assert_fluid_is_contained(ID, s.size, &cells);

    let nbt = fastnbt::to_bytes(&s).expect("structure serializes to NBT");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip write");
    let framed = gz.finish().expect("gzip finish");

    let nbt_path = out.join(format!("{ID}.nbt"));
    std::fs::write(&nbt_path, &framed)
        .unwrap_or_else(|e| panic!("write {}: {e}", nbt_path.display()));

    let meta_path = out.join(format!("{ID}.json"));
    // Merged onto whatever is already there: a generator deletes nothing it did
    // not write (`prefab_invariants::document`).
    document::write_preserving(&meta_path, &meta);

    println!(
        "wrote {} ({} blocks, {} palette entries, {} gz bytes) and {}",
        nbt_path.display(),
        s.blocks.len(),
        s.palette.len(),
        framed.len(),
        meta_path.display()
    );
}

/// The mannequin skins the gallery's `skin.texture_id` declarations name.
///
/// `(texture_id, base_rgb, belt_rgb)`. Two, because `NpcSkin.model` has two
/// members and a `slim` skin and a `wide` skin must both exist for the pair to
/// be written.
const SKINS: [(&str, [u8; 3], [u8; 3]); 2] = [
    ("curator", [0x3A, 0x3F, 0x55], [0xC9, 0xA2, 0x27]),
    ("bearer", [0x5A, 0x3A, 0x2E], [0xB8, 0xC4, 0xCF]),
];

/// Standard CRC-32 (PNG's, and gzip's). Written out rather than pulled in: the
/// generators deliberately carry a small third-party dependency set, and one
/// polynomial is cheaper than another crate in it.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for b in bytes {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 12);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = kind.to_vec();
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
    out
}

/// A 64×64 RGBA mannequin skin, flat-coloured with a belt band.
///
/// Deliberately not art. A skin here exists so `NpcSkin.texture_id` names
/// something that resolves and `DW0309` has a file to find; the *look* of a
/// mannequin is a campaign's business, and the gallery having an opinion about
/// it would be authored content wearing a test surface's clothes.
fn skin_png(base: [u8; 3], belt: [u8; 3]) -> Vec<u8> {
    const W: usize = 64;
    const H: usize = 64;
    let mut raw = Vec::with_capacity(H * (1 + W * 4));
    for y in 0..H {
        raw.push(0); // filter type 0 (None) on every scanline
        for x in 0..W {
            // Transparent outside the 64×32-style body block, so the skin reads
            // as a mannequin rather than a full sheet of colour.
            let opaque = y < 32 || (8..56).contains(&x);
            let c = if (20..24).contains(&y) { belt } else { base };
            raw.extend_from_slice(&[c[0], c[1], c[2], if opaque { 0xFF } else { 0x00 }]);
        }
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), Compression::new(6));
    z.write_all(&raw).expect("zlib write");
    let idat = z.finish().expect("zlib finish");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(W as u32).to_be_bytes());
    ihdr.extend_from_slice(&(H as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA, deflate, no filter, no interlace

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&png_chunk(b"IDAT", &idat));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

/// **The shore variants**: the same pieces, built to stand on an ocean.
///
/// `horizon: ocean` puts every area origin at `SEA_LEVEL - 2` (the compiler's
/// `OCEAN_BASE_Y`, 60, under a sea at 62), because that is the datum the island
/// convention needs: a shore piece authors its water up to local y=2 and stands
/// its land plane at local y=3, one block clear of the sea. A piece built with
/// its floor at local y=0 — which is every piece in this file — puts its walk
/// plane at world y=61, a block UNDER the surface, and vanilla then floods it:
/// `/place template` hands each waterloggable block the water already in the
/// cell, so the hall's own gate bars and chests come out `waterlogged=true` and
/// spread. Measured on the pinned server, booting this gallery's own
/// `ocean-horizon` point: 10 waterlogged blocks and 367 water cells across the
/// walk plane, under a `sea-seepage.json` that said `pass`.
///
/// So the ocean point does not build from the same bytes. It builds from these:
/// each piece lifted onto [`SHORE_PLINTH`] courses of solid plinth, with a tide
/// pool cut into its floor course so the piece's own top water block lands
/// exactly on the sea plane, and `waterline_y` declared to say so. The primary
/// (void) point still builds from the unlifted pieces, byte for byte.
const SHORE_PLINTH: i32 = 2;

/// The hall's tide pool, in ROOM-space `(x, z)` — cut out of the floor course,
/// which the lift puts at local y=[`SHORE_PLINTH`], so its water surface is the
/// sea's own plane.
///
/// Against the west wall and away from every anchor: the nearest is
/// `anchor/west-pit` at `[2, 1, 3]`, nine cells north of it, and
/// [`assert_the_pool_is_clear_of_every_anchor`] is what keeps that true rather
/// than this sentence.
const HALL_POOL: [(i32, i32); 4] = [(1, 12), (1, 13), (2, 12), (2, 13)];

/// The same, one cell, in each annex tile: the interior corner opposite the
/// tile's own anchor at `[3, 1, 3]`.
const ANNEX_POOL: [(i32, i32); 1] = [(1, 1)];

/// Lift a built structure onto its plinth and cut the tide pool into its floor
/// course.
///
/// Everything the piece already is moves up by [`SHORE_PLINTH`]; the courses
/// underneath are solid stone; and `pool` names the floor cells that become
/// water. Because the lift happens after [`resolve_connections`], the connector
/// shapes the tile set resolved are carried through untouched.
fn lift_to_shore(s: &Structure, pool: &[(i32, i32)]) -> Structure {
    let mut palette = s.palette.clone();
    let mut idx = |name: &str| -> i32 {
        let e = PaletteEntry {
            name: name.to_string(),
            properties: None,
        };
        match palette.iter().position(|x| *x == e) {
            Some(i) => i as i32,
            None => {
                palette.push(e);
                (palette.len() - 1) as i32
            }
        }
    };
    let stone = idx("minecraft:stone");
    let water = idx("minecraft:water");
    let mut blocks: Vec<BlockEntry> =
        Vec::with_capacity(s.blocks.len() + (s.size[0] * SHORE_PLINTH * s.size[2]) as usize);
    for y in 0..SHORE_PLINTH {
        for x in 0..s.size[0] {
            for z in 0..s.size[2] {
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: stone,
                });
            }
        }
    }
    for b in &s.blocks {
        let pos = [b.pos[0], b.pos[1] + SHORE_PLINTH, b.pos[2]];
        let state = if pos[1] == SHORE_PLINTH && pool.contains(&(pos[0], pos[2])) {
            water
        } else {
            b.state
        };
        blocks.push(BlockEntry { pos, state });
    }
    let cut = blocks.iter().filter(|b| b.state == water).count();
    assert_eq!(
        cut,
        pool.len(),
        "the tide pool cut {cut} cell(s) where {} were named — a waterline declared over water \
         the piece does not author is the fiction DW0344 exists to refuse",
        pool.len()
    );
    Structure {
        data_version: s.data_version,
        size: [s.size[0], s.size[1] + SHORE_PLINTH, s.size[2]],
        palette,
        blocks,
        entities: Vec::new(), // the gallery's pieces carry none, and a lift invents none
    }
}

/// The same lift applied to the piece's metadata: every declared position rises
/// with the blocks and the extent grows. The waterline is NOT stated here —
/// [`declare_waterline_y`] reads it back off the lifted bytes, beside
/// [`declare_walk_y`].
///
/// The keys are named rather than inferred. A blind walk over "every array of
/// three integers" would also lift `structure.size`, which is an extent and not
/// a place, and the piece would claim a box it does not fill.
fn lift_metadata(meta: &serde_json::Value) -> serde_json::Value {
    fn walk(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(map) => {
                for (k, child) in map.iter_mut() {
                    if matches!(k.as_str(), "pos" | "local_pos" | "from" | "to") {
                        if let Some(a) = child.as_array_mut() {
                            if a.len() == 3 {
                                let y = a[1].as_i64().expect("a position's y is an integer");
                                a[1] = serde_json::json!(y + SHORE_PLINTH as i64);
                                continue;
                            }
                        }
                    }
                    walk(child);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items.iter_mut() {
                    walk(child);
                }
            }
            _ => {}
        }
    }
    let mut m = meta.clone();
    walk(&mut m);
    let sy = m["structure"]["size"][1]
        .as_i64()
        .expect("the declared extent has a y");
    m["structure"]["size"][1] = serde_json::json!(sy + SHORE_PLINTH as i64);
    m
}

/// **The piece's own waterline, measured off the bytes about to be written**
/// (spec-0060 §4) — [`declare_walk_y`]'s pair.
///
/// The declaration `DW0344` binds to is the local y of the top authored water
/// block, and `DW0887` holds the document to it. The tide pool this generator
/// cuts puts that block on `SHORE_PLINTH` by construction, and writing the
/// constant was therefore *correct* — which is exactly why it is the shape to
/// remove: it is correct until the cut moves, and nothing here would say. Every
/// generator in this workspace reads the number back out of its own blocks
/// through one rule (`prefab_invariants::waterline`), and a piece that authors
/// no water writes no key at all.
fn declare_waterline_y(s: &Structure, meta: &mut serde_json::Value) {
    let cells = invariant_cells(s);
    match waterline::measure_waterline_y(&cells) {
        Some(y) => {
            meta["waterline_y"] = serde_json::json!(y);
        }
        None => {
            meta.as_object_mut()
                .expect("prefab metadata is an object")
                .remove("waterline_y");
        }
    }
}

/// **Every anchor stands clear of the tide pool** — the standability proof the
/// lift could break, because a pool cut under an anchor puts a body in the
/// water.
///
/// It used to carry a second half, that the declared `waterline_y` is the top
/// authored water block, and that half is **gone on purpose**. It is now
/// `DW0887` in the engine, reached by every library rather than by this one
/// generator; a generator-private copy of an engine rule is a second authority
/// that agrees until it does not, and this one could only ever have proven the
/// gallery. What proves the gallery's shores now is what proves a creator's:
/// the build opens the bytes (spec-0060 §11).
fn assert_the_shore_is_standable(id: &str, s: &Structure, meta: &serde_json::Value) {
    let water: Vec<[i32; 3]> = s
        .blocks
        .iter()
        .filter(|b| s.palette[b.state as usize].name == "minecraft:water")
        .map(|b| b.pos)
        .collect();
    let anchors = meta["anchors"].as_object().expect("an anchor inventory");
    let mut examined = 0usize;
    for (name, a) in anchors {
        let Some(pos) = a.get("pos").and_then(|p| p.as_array()) else {
            continue; // a region anchor (a gate) stands nowhere
        };
        let cell = [
            pos[0].as_i64().unwrap() as i32,
            pos[1].as_i64().unwrap() as i32,
            pos[2].as_i64().unwrap() as i32,
        ];
        examined += 1;
        for probe in [cell, [cell[0], cell[1] - 1, cell[2]]] {
            assert!(
                !water.contains(&probe),
                "{id}: anchor `{name}` stands in or on the tide pool at {probe:?}"
            );
        }
    }
    assert!(
        examined > 0,
        "{id}: the shore proof examined ZERO anchors — a universally quantified assertion over \
         an empty set is vacuous, not a pass"
    );
}

/// **The piece's own walk plane, measured off the bytes about to be written**
/// (spec-0060 §4).
///
/// Every generator writes this, and every one of them measures it rather than
/// typing it: `walk_y` has no default, so a number nobody read off the blocks
/// is one tileset's convention wearing the name of a measurement. The rule
/// itself lives once, in `prefab_invariants::walkplane`, so the number this
/// generator writes and the number the engine's seating derivation expects are
/// the same rule rather than two that agree.
///
/// It refuses a piece with no standable cell instead of writing some number for
/// it: a piece a body cannot stand in has no walk plane, and every piece that
/// reaches here is one a party walks.
fn declare_walk_y(id: &str, s: &Structure, meta: &mut serde_json::Value) {
    let cells = invariant_cells(s);
    meta["walk_y"] = serde_json::json!(walkplane::measure_walk_y(id, s.size, &cells));
}

/// Lift one piece onto its plinth, prove the result, and hand back both halves.
fn to_shore(
    id: &str,
    s: &Structure,
    meta: &serde_json::Value,
    pool: &[(i32, i32)],
) -> (Structure, serde_json::Value) {
    let lifted = lift_to_shore(s, pool);
    let mut meta = lift_metadata(meta);
    declare_waterline_y(&lifted, &mut meta);
    assert_the_shore_is_standable(id, &lifted, &meta);
    declare_walk_y(id, &lifted, &mut meta);
    (lifted, meta)
}

/// **One of the gallery's stand-in reference images**: `(stem, sky_top_rgb,
/// sky_bottom_rgb, lamp_rgb)`.
///
/// There is one per [`WorldTime`](delvewright_dsl::WorldTime) the gallery
/// reaches, and `gallery/design.json`'s rows spread the three weathers across
/// them, because `DW0890` compares the skies the rows state with the skies the
/// world reaches for EQUALITY: a picture missing for an hour the party can be
/// in is exactly what that code refuses.
type DesignScene = (&'static str, [u8; 3], [u8; 3], [u8; 3]);

/// The six scenes — see [`DesignScene`] for the tuple.
const DESIGN_SCENES: [DesignScene; 6] = [
    (
        "morning-quay",
        [150, 190, 230],
        [200, 215, 200],
        [255, 230, 160],
    ),
    (
        "noon-hall",
        [175, 205, 240],
        [215, 220, 205],
        [255, 235, 175],
    ),
    (
        "dusk-rampart",
        [205, 130, 85],
        [120, 85, 75],
        [255, 205, 130],
    ),
    ("night-quay", [20, 28, 55], [35, 40, 60], [255, 210, 140]),
    ("midnight-crypt", [8, 10, 22], [16, 18, 30], [230, 190, 130]),
    (
        "dawn-approach",
        [120, 120, 175],
        [195, 165, 145],
        [255, 225, 165],
    ),
];

/// A 48x48 RGB stand-in for an approved concept image: a vertical sky gradient
/// with one warm lamp in it.
///
/// **Deliberately not art**, for the reason [`skin_png`] gives about a
/// mannequin. Nothing in this toolchain opens an approved image — `DW0890`
/// holds the world to the TOKEN a creator wrote beside the picture, never to
/// its pixels (spec-0061 §12) — so what the gallery needs here is a file that
/// exists, resolves from a row's stem, and reads at a glance as a different
/// hour from its five neighbours.
fn concept_png(top: [u8; 3], bottom: [u8; 3], lamp: [u8; 3]) -> Vec<u8> {
    const W: usize = 48;
    const H: usize = 48;
    let mut raw = Vec::with_capacity(H * (1 + W * 3));
    for y in 0..H {
        raw.push(0); // filter type 0 (None) on every scanline
        let t = y as f32 / (H - 1) as f32;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        let sky = [
            mix(top[0], bottom[0]),
            mix(top[1], bottom[1]),
            mix(top[2], bottom[2]),
        ];
        for x in 0..W {
            let c = if (20..=27).contains(&x) && (28..=35).contains(&y) {
                lamp
            } else {
                sky
            };
            raw.extend_from_slice(&c);
        }
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), Compression::new(9));
    z.write_all(&raw).expect("zlib write");
    let idat = z.finish().expect("zlib finish");

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(W as u32).to_be_bytes());
    ihdr.extend_from_slice(&(H as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, RGB, deflate, no filter, no interlace

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&png_chunk(b"IDAT", &idat));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

/// Write the stand-in concept images into `<out>/concept/`, the directory
/// `gallery/design.json`'s row names resolve against.
fn write_design(out: &Path) {
    let concept = out.join("concept");
    std::fs::create_dir_all(&concept)
        .unwrap_or_else(|e| panic!("mkdir {}: {e}", concept.display()));
    for (stem, top, bottom, lamp) in DESIGN_SCENES {
        let path = concept.join(format!("{stem}.png"));
        std::fs::write(&path, concept_png(top, bottom, lamp))
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }
}

fn write_skins(out: &Path) {
    std::fs::create_dir_all(out).unwrap_or_else(|e| panic!("mkdir {}: {e}", out.display()));
    for (id, base, belt) in SKINS {
        let path = out.join(format!("{id}.png"));
        std::fs::write(&path, skin_png(base, belt))
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }
}

// ---------------------------------------------------------------- the annex --
//
// A second, TILED piece set, so the gallery can bind the surfaces that only
// exist once a world is assembled from a pool: `prefab_pool`, the piece verbs
// (`insert-piece`, `swap-piece`, `remove-piece`, `reseed-piece`), the socket
// verbs (`rewire-socket`) and `fragment`. Nothing in this repository — no
// campaign and no fixture — had ever written any of them, which is exactly the
// class spec-0039 exists to reach.
//
// Deliberately plain: three 7 x 6 x 7 stone boxes with 3 x 3 openings on the
// faces that carry a socket. The point is the ASSEMBLY, not the architecture,
// and a tileset with interesting rooms would make the placement harder to read
// without binding one more unit.

/// The annex tile extent.
const ANNEX_SIZE: [i32; 3] = [7, 6, 7];

/// Which faces of an annex tile carry a `gallery:socket`.
struct AnnexTile {
    id: &'static str,
    /// `true` for a socket on the north (z = 0) face.
    north: bool,
    /// `true` for a socket on the south (z = size - 1) face.
    south: bool,
    role: &'static str,
    weight: u32,
    /// The one anchor this tile declares: the standing cell at the middle of
    /// its floor. A tile with no anchor names no place, so every view of it
    /// binds zero targets and the camera checks nothing by having been aimed —
    /// see [`ANNEX_ANCHOR_POS`].
    anchor: &'static str,
    /// Prose about that anchor, printed into the metadata beside it (the same
    /// `note` key the hall's anchors carry) so a piece explains itself without
    /// the campaign in hand. Read by nobody but a person.
    anchor_note: &'static str,
    /// The term of the ENGINE's anchor vocabulary that anchor declares, if any.
    ///
    /// This is the tileset the compiler has to FIND its way into: an annex
    /// tile's anchor key is `anchor/<stem>`, which can spell neither `spawn`
    /// nor `entry`, so before the role existed `area/annex` resolved no entry
    /// point at all — every consumer that wanted one asked an honest question
    /// about the wrong key and got an honest `None`. Not to be confused with
    /// [`AnnexTile::role`] one field up, which is this tile's place in the
    /// template POOL and means nothing outside it.
    anchor_role: Option<&'static str>,
}

/// The cell every annex tile's anchor stands in: the middle of the floor, one
/// above it. Proven standable on every run by [`assert_annex_anchor_stands`]
/// rather than kept in step with the carving by hand.
const ANNEX_ANCHOR_POS: [i32; 3] = [3, 1, 3];

const ANNEX_TILES: &[AnnexTile] = &[
    AnnexTile {
        id: "gallery-annex-entry",
        north: false,
        south: true,
        role: "entry",
        weight: 1,
        anchor: "anchor/annex-threshold",
        anchor_note: "where the annex is entered — the entry tile's floor centre",
        anchor_role: Some("entry"),
    },
    AnnexTile {
        id: "gallery-annex-cell",
        north: true,
        south: true,
        role: "connector",
        weight: 2,
        anchor: "anchor/annex-first-bay",
        anchor_note: "the floor centre of the first bay the chain threads through",
        anchor_role: None,
    },
    // A SECOND two-socket variant, and the reason is `reseed-piece`: it re-rolls
    // a placed piece against the pool and refuses when no OTHER member can
    // re-mate that piece\u0027s sockets. With one connector shape in the pool the
    // verb is unwritable — a pool of one variant is a pool that cannot be
    // reseeded.
    AnnexTile {
        id: "gallery-annex-cell-b",
        north: true,
        south: true,
        role: "connector",
        weight: 1,
        anchor: "anchor/annex-second-bay",
        anchor_note: "the floor centre of the second bay the chain threads through",
        anchor_role: None,
    },
    AnnexTile {
        id: "gallery-annex-end",
        north: true,
        south: false,
        role: "terminal",
        weight: 1,
        anchor: "anchor/annex-cap",
        anchor_note: "the floor centre of the tile that caps the chain",
        anchor_role: None,
    },
];

/// The opening a socket sits in: 3 wide, 3 tall, centred on the face.
fn annex_opening(x: i32, y: i32) -> bool {
    (2..=4).contains(&x) && (1..=3).contains(&y)
}

/// The brick panel a socket face wears around its opening: the full width of the
/// face between the corners, floor to lintel.
///
/// **It is not decoration.** A tile carved from one material renders as one
/// material: `seam/annex/2` looked down a three-tile corridor of nothing but
/// `minecraft:stone` and came back a rectangle of ONE distinct colour, which the
/// render arm reports — correctly — as a frame that shows no scene at all. The
/// panel is the second material, and it is put HERE, framing the doorway,
/// because that is what a seam camera is aimed at. It also earns its keep twice:
/// the seal an unmated socket gets is `minecraft:stone_bricks`, so a socket with
/// nothing on the other side reads in the picture as a bricked-up doorway in a
/// brick surround rather than as a patch of the wrong wall.
fn annex_panel(x: i32, y: i32) -> bool {
    (1..=ANNEX_SIZE[0] - 2).contains(&x) && (1..=ANNEX_SIZE[1] - 2).contains(&y)
}

fn annex_block_at(t: &AnnexTile, x: i32, y: i32, z: i32) -> &'static str {
    let (w, h, d) = (ANNEX_SIZE[0], ANNEX_SIZE[1], ANNEX_SIZE[2]);
    if y == 0 {
        return "minecraft:stone_bricks";
    }
    if y == h - 1 {
        return "minecraft:stone";
    }
    let socket_face = (z == 0 && t.north) || (z == d - 1 && t.south);
    if socket_face && annex_opening(x, y) {
        return "minecraft:air";
    }
    if socket_face && annex_panel(x, y) {
        return "minecraft:stone_bricks";
    }
    if x == 0 || x == w - 1 || z == 0 || z == d - 1 {
        return "minecraft:stone";
    }
    if ANNEX_LANTERNS.contains(&[x, y, z]) {
        return "minecraft:lantern";
    }
    "minecraft:air"
}

/// Where a tile hangs its light. TWO lanterns on the ceiling diagonal rather
/// than one in the middle, which keeps the middle of the ceiling — where a seam
/// camera is framed from — clear of fixtures.
///
/// The diagonal keeps the measured floor light at 8: the darkest interior cells
/// are the two corners not on the diagonal, seven blocks of open air from the
/// nearer lantern.
const ANNEX_LANTERNS: [[i32; 3]; 2] = [[2, ANNEX_SIZE[1] - 2, 2], [4, ANNEX_SIZE[1] - 2, 4]];

fn build_annex(t: &AnnexTile) -> Structure {
    let mut palette = Palette::new();
    for (name, props) in [
        ("minecraft:air", None),
        ("minecraft:stone", None),
        ("minecraft:stone_bricks", None),
        ("minecraft:lantern", Some(&[("hanging", "true")][..])),
    ] {
        palette.idx(name, props);
    }
    let mut blocks = Vec::new();
    for x in 0..ANNEX_SIZE[0] {
        for y in 0..ANNEX_SIZE[1] {
            for z in 0..ANNEX_SIZE[2] {
                let name = annex_block_at(t, x, y, z);
                let props: Option<&[(&str, &str)]> = if name == "minecraft:lantern" {
                    Some(&[("hanging", "true")])
                } else {
                    None
                };
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: palette.idx(name, props),
                });
            }
        }
    }
    for (i, e) in palette.entries.iter().enumerate() {
        assert!(
            blocks.iter().any(|b| b.state == i as i32),
            "{}: palette entry {i} ({}) is referenced by no cell",
            t.id,
            e.name
        );
    }
    Structure {
        data_version: DATA_VERSION,
        size: ANNEX_SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

/// The tile's anchor inventory — one entry, printed from the same table the
/// carving is checked against.
fn annex_anchors(t: &AnnexTile) -> serde_json::Value {
    use serde_json::{json, Map, Value};
    let mut m = Map::new();
    m.insert("pos".into(), json!(ANNEX_ANCHOR_POS));
    m.insert("note".into(), json!(t.anchor_note));
    if let Some(role) = t.anchor_role {
        m.insert("role".into(), json!(role));
    }
    let mut anchors = Map::new();
    anchors.insert(t.anchor.into(), Value::Object(m));
    Value::Object(anchors)
}

/// The annex counterpart of [`assert_anchors_are_standable`]: the cell the
/// metadata calls a standing place really is one in the blocks beside it.
/// Proven on every run rather than kept in step by hand — an anchor that names
/// solid stone resolves to a place no body can occupy, and nothing downstream
/// re-checks it.
fn assert_annex_anchor_stands(t: &AnnexTile, s: &Structure) {
    let at = |p: [i32; 3]| -> &str {
        let cell = s
            .blocks
            .iter()
            .find(|b| b.pos == p)
            .unwrap_or_else(|| panic!("{}: anchor cell {p:?} is outside the tile", t.id));
        s.palette[cell.state as usize].name.as_str()
    };
    let [x, y, z] = ANNEX_ANCHOR_POS;
    assert_eq!(
        at([x, y, z]),
        "minecraft:air",
        "{}: anchor `{}` stands in a solid cell",
        t.id,
        t.anchor
    );
    assert_eq!(
        at([x, y + 1, z]),
        "minecraft:air",
        "{}: anchor `{}` has no headroom",
        t.id,
        t.anchor
    );
    assert_ne!(
        at([x, y - 1, z]),
        "minecraft:air",
        "{}: anchor `{}` has no floor under it",
        t.id,
        t.anchor
    );
}

/// The metadata for one annex tile, including its connectors.
fn annex_metadata(t: &AnnexTile) -> serde_json::Value {
    use serde_json::{json, Value};
    let mut connectors: Vec<Value> = Vec::new();
    if t.north {
        connectors.push(json!({
            "name": "gallery:socket",
            "target": "gallery:socket",
            "local_pos": [3, 1, 0],
            "facing": "north",
            "opening": [3, 3],
            "joint": "aligned"
        }));
    }
    if t.south {
        connectors.push(json!({
            "name": "gallery:socket",
            "target": "gallery:socket",
            "local_pos": [3, 1, ANNEX_SIZE[2] - 1],
            "facing": "south",
            "opening": [3, 3],
            "joint": "aligned"
        }));
    }
    assert!(
        !connectors.is_empty(),
        "{}: an annex tile with no socket can never be placed by a pool",
        t.id
    );
    json!({
        "prefab_id": format!("prefab/{}", t.id),
        "structure": {
            "file": format!("{}.nbt", t.id),
            "id": t.id,
            "size": ANNEX_SIZE,
            "data_version": DATA_VERSION,
            "generator": "prefabs/gallery-generator (gallery-prefab-gen)"
        },
        // One anchor per tile: the standing cell at the middle of its floor.
        //
        // A tile that declares no anchor names no place, so nothing the campaign
        // can address is ever inside it and every view of it binds zero targets
        // — a camera aimed at nothing, which the render arm reports.
        //
        // An UNMATED socket is not a hole. `solver::seal_layout` fills every
        // unmated connector's opening with `minecraft:stone_bricks` and clears
        // it to air only when the socket mates, so the opening carved below
        // exists in the world exactly when something is on the other side of it.
        // That is why a tile can carry both an anchor and a spare socket:
        // reachable floor beside a sealed socket borders a wall, not the void.
        "anchors": annex_anchors(t),
        "connectors": connectors,
        "lighting": {
            "profile": "lit",
            "measured_min_light": 8,
            "measured": "2026-08-20",
            "method": "derived: two ceiling-hung lanterns on the diagonal of a 5 x 4 x 5 \
                       interior, the same mounting as the hall's grid — the darkest floor \
                       cell is seven blocks of open air from the nearer of them"
        },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Original Delvewright project asset (pipeline-code license per \
                     prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            "provenance": "Generated deterministically by prefabs/gallery-generator \
                           (ADR-0006); regenerating yields byte-identical NBT and metadata."
        }
    })
}

/// The pools this library declares: the one the annex area draws from, and the
/// one that exists to be refused.
///
/// Written here rather than printed for a human to paste, unlike the tileset
/// generators: the gallery's prefab directory is a BUILD directory this program
/// owns end to end, so there is no shared library for a stray file to be
/// mis-parsed in (`DW0346`).
///
/// # `pool/gallery-two-planes` is a refusal, and it is made of real pieces
///
/// `DW0886`'s set shape — *the members of this pool do not agree about their own
/// walk plane, and one origin cannot be derived from two* — is the rule this
/// whole library exists to demonstrate firing, and it is the one thing a
/// campaign-level probe cannot reach on its own: a probe patches campaign
/// documents, and a walk plane is prefab metadata. So the pool is declared here,
/// out of two pieces this generator already writes whose planes genuinely
/// differ: `gallery-quay` stands on a shore plinth at local y=3 and
/// `gallery-yard` is a detail piece whose floor is local y=1. Nothing is faked —
/// what makes the pool unseatable on an ocean is a true fact about two true
/// pieces, which is exactly what a creator's own mixed pool would be.
///
/// The gallery's primary never seats it; `gallery/probes/a-pool-of-two-walk-planes`
/// does, and the engine refuses it.
fn pools() -> serde_json::Value {
    use serde_json::{json, Value};
    let members: Vec<Value> = ANNEX_TILES
        .iter()
        .map(
            |t| json!({ "prefab": format!("prefab/{}", t.id), "weight": t.weight, "role": t.role }),
        )
        .collect();
    json!({
        "pools": {
            "pool/gallery-annex": { "members": members },
            "pool/gallery-two-planes": { "members": [
                { "prefab": format!("prefab/{QUAY_ID}"), "weight": 1, "role": "entry" },
                { "prefab": format!("prefab/{YARD_ID}"), "weight": 1, "role": "terminal" },
            ]},
        }
    })
}

fn write_annex(out: &Path) {
    let mut anchors_proven = 0usize;
    for t in ANNEX_TILES {
        let mut room = build_annex(t);
        resolve_connections(t.id, &mut room);
        assert_annex_anchor_stands(t, &room);
        anchors_proven += 1;
        let (s, tile_meta) = to_shore(t.id, &room, &annex_metadata(t), &ANNEX_POOL);
        let cells = invariant_cells(&s);
        invariants::assert_distress_never_stacks(t.id, &cells);
        invariants::assert_blocks_are_real(t.id, &cells);
        connections::assert_shape_is_stated(t.id, &cells);
        connections::assert_attachments_are_supported(t.id, &cells);
        invariants::assert_fluid_is_contained(t.id, s.size, &cells);

        let nbt = fastnbt::to_bytes(&s).expect("structure serializes to NBT");
        let mut gz = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::new(6));
        gz.write_all(&nbt).expect("gzip write");
        let framed = gz.finish().expect("gzip finish");
        std::fs::write(out.join(format!("{}.nbt", t.id)), &framed)
            .unwrap_or_else(|e| panic!("write {}.nbt: {e}", t.id));

        document::write_preserving(&out.join(format!("{}.json", t.id)), &tile_meta);
    }
    document::write_preserving(&out.join("pools.json"), &pools());
    assert_eq!(
        anchors_proven,
        ANNEX_TILES.len(),
        "{ID}: the annex proofs examined {anchors_proven} anchor(s) over {} tile(s) — a \
         universally quantified assertion over an empty set is vacuous, not a pass",
        ANNEX_TILES.len()
    );
    println!(
        "{ID}: annex tileset written — {} tile(s) and one pool; {anchors_proven} anchor(s) proven \
         standable",
        ANNEX_TILES.len()
    );
}

/// A 3 x 3 x 3 marker block, the gallery\u0027s fragment source.
///
/// `fragment` writes a prefab\u0027s blocks into an already-placed piece at a point,
/// and `FragmentRotation` has four members — so binding them means four
/// placements, and the hall has no four 7-cube holes left in it. A piece this
/// size fits where the tiles cannot. It carries NO connector on purpose: it is
/// never a pool member, only something to stamp.
const SHARD_ID: &str = "gallery-shard";
const SHARD_SIZE: [i32; 3] = [3, 3, 3];

fn build_shard() -> Structure {
    let mut palette = Palette::new();
    for (name, props) in [
        ("minecraft:polished_blackstone", None),
        ("minecraft:lantern", Some(&[("hanging", "true")][..])),
    ] {
        palette.idx(name, props);
    }
    let mut blocks = Vec::new();
    for x in 0..SHARD_SIZE[0] {
        for y in 0..SHARD_SIZE[1] {
            for z in 0..SHARD_SIZE[2] {
                // A hanging lantern at the top centre, so the stamp is visible
                // and asymmetric — a rotation nobody can see is a rotation
                // nobody can check.
                let lantern = [x, y, z] == [0, SHARD_SIZE[1] - 1, 0];
                let (name, props): (&str, Option<&[(&str, &str)]>) = if lantern {
                    ("minecraft:lantern", Some(&[("hanging", "true")]))
                } else {
                    ("minecraft:polished_blackstone", None)
                };
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: palette.idx(name, props),
                });
            }
        }
    }
    Structure {
        data_version: DATA_VERSION,
        size: SHARD_SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

fn write_shard(out: &Path) {
    let mut s = build_shard();
    resolve_connections(SHARD_ID, &mut s);
    let cells = invariant_cells(&s);
    invariants::assert_blocks_are_real(SHARD_ID, &cells);
    connections::assert_shape_is_stated(SHARD_ID, &cells);
    invariants::assert_fluid_is_contained(SHARD_ID, s.size, &cells);
    let nbt = fastnbt::to_bytes(&s).expect("structure serializes to NBT");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip write");
    let framed = gz.finish().expect("gzip finish");
    std::fs::write(out.join(format!("{SHARD_ID}.nbt")), &framed).expect("write shard nbt");
    let mut meta = serde_json::json!({
        "prefab_id": format!("prefab/{SHARD_ID}"),
        "structure": { "file": format!("{SHARD_ID}.nbt"), "id": SHARD_ID, "size": SHARD_SIZE, "data_version": DATA_VERSION, "generator": "prefabs/gallery-generator (gallery-prefab-gen)" },
        "anchors": {},
        "lighting": { "profile": "lit", "measured_min_light": 8, "measured": "2026-08-20", "method": "derived: a lantern on a solid 3-cube; never entered, only stamped" },
        "license": { "source": "original", "spdx": "GPL-3.0-or-later", "note": "Original Delvewright project asset (pipeline-code license per prefabs/LICENSE-ASSETS.md). No third-party material ingested.", "provenance": "Generated deterministically by prefabs/gallery-generator (ADR-0006)." }
    });
    // A fragment source is a block of material something later cuts from, not a
    // place a party stands, so it may genuinely have no walk plane — and a
    // piece with none writes no `walk_y`. That is the honest document: it
    // cannot be seated on a horizon that derives an origin from one, and
    // `DW0886` is where a campaign that tries to learns it.
    if let Some(w) = walkplane::walk_y(s.size, &cells) {
        meta["walk_y"] = serde_json::json!(w);
    }
    document::write_preserving(&out.join(format!("{SHARD_ID}.json")), &meta);
    println!("{SHARD_ID}: fragment source written");
}

// ---------------------------------------------------------------------------
// The DETAIL piece (spec-0050): the yard the site-plan overlay's exit box holds
// ---------------------------------------------------------------------------

/// The detail piece's id.
const YARD_ID: &str = "gallery-yard";

/// **The frame the whole gives `node/exit`**, and every number here is a
/// consequence of it rather than a choice.
///
/// The site-plan overlay gives that place an 8×8 footprint at the `alcove` rung
/// with no ceiling, so its play space is 8×3×8 and its FRAME — the play space
/// plus the one floor course a piece owns — is 8×4×8. A detail piece must be
/// exactly the shape of its allocation, and `DW0843` refuses a cell either way,
/// so this constant is not a size the generator picked: it is what
/// `delvec allocation node/exit` hands out, and the campaign document that binds
/// this piece is what makes the two answerable to each other.
const YARD_SIZE: [i32; 3] = [8, 4, 8];

/// The way in, in piece-local cells: the seam the plan cut on the exit's north
/// face, as `delvec allocation` states it. The piece must leave exactly these
/// cells passable and must not open a second way out — the first is `DW0844`
/// from one direction and the second is `DW0844` from the other.
const YARD_WAY: ([i32; 3], [i32; 3]) = ([2, 1, 0], [3, 3, 0]);

/// Where a body stands when a quest seats it here — `anchor/node-exit` after the
/// re-binding. Open paving, clear of the plinth.
const YARD_SEAT: [i32; 3] = [1, 1, 4];

/// A courtyard: paving, a low plinth to walk around, and four corner posts.
///
/// It is a **building** rather than the box's massing repeated, and that is the
/// whole demonstration: the interior standable set and the route through a place
/// are deliberately free to change under detail (spec-0050 §7), while the seam,
/// its cells and its rise are not. What a reader should be able to see here is
/// that the plan's one way in is still exactly where the plan cut it, and that
/// everything else is the piece's own.
fn build_yard() -> Structure {
    let mut palette = Palette::new();
    let mut blocks = Vec::new();
    let [sx, sy, sz] = YARD_SIZE;
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let plinth = y == 1 && (3..=4).contains(&x) && (3..=4).contains(&z);
                let post =
                    (1..=2).contains(&y) && (x == 0 || x == sx - 1) && (z == 0 || z == sz - 1);
                let name = if y == 0 {
                    "minecraft:polished_andesite"
                } else if plinth {
                    "minecraft:chiseled_stone_bricks"
                } else if post {
                    "minecraft:polished_blackstone_bricks"
                } else {
                    // Air is AUTHORED rather than omitted: a detail piece
                    // replaces the massing inside its frame, and a cell it does
                    // not write is a cell whatever stood there keeps.
                    "minecraft:air"
                };
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: palette.idx(name, None),
                });
            }
        }
    }
    Structure {
        data_version: DATA_VERSION,
        size: YARD_SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

/// **The piece answers the plan's seam, and says so.** `DW0844` reads this face
/// contract against the site plan's own seams, in both directions, before any
/// byte assembles — and `DW0836`/`DW0838` read the bytes afterwards. The two
/// observers are deliberately redundant: a piece that lies here passes the first
/// and reds on the second.
fn yard_metadata() -> serde_json::Value {
    let (wl, wh) = YARD_WAY;
    serde_json::json!({
        "prefab_id": format!("prefab/{YARD_ID}"),
        "structure": {
            "file": format!("{YARD_ID}.nbt"),
            "id": YARD_ID,
            "size": YARD_SIZE,
            "data_version": DATA_VERSION,
            "generator": "prefabs/gallery-generator (gallery-prefab-gen)"
        },
        // `note`, not `role`. `role` is the engine's own closed vocabulary for
        // what the compiler must FIND without being told a name (spec-0046);
        // a sentence written there is a malformed value, and `DW0346` skips the
        // whole file — so this piece would silently not exist. The prose an
        // anchor carries and the term the engine reads are two keys.
        "anchors": {
            "yard-stone": {
                "pos": YARD_SEAT,
                "facing": "north",
                "resolves_to": "space:yard",
                "note": "where a body stands when the campaign seats it in this place"
            }
        },
        // The claim about what SIZE of box this piece is for, judged against its
        // own bytes at admission and again wherever a detail plan consumes it
        // (`DW0848`). 8×8 on the kit grid, three of clearance: an `alcove`.
        "footprint_class": "alcove",
        // **The sides of this piece the player is meant to see** (`DW0885`).
        // The yard is the one piece in this gallery whose outside the party's
        // own air reaches: it is an `open_top` court, so the air a body stands
        // in leaves through the sky, runs around the outside of the box and
        // comes back under it. Its four walls and its top are inside the site
        // plan's own volumes and buried by them; its FLOOR is the one face with
        // nothing in front of it, hanging over a `void` world's nothing.
        //
        // So `down` is the whole list, and it is the piece saying that its
        // underside is a deliberate finished face of a free-standing court
        // rather than the cut edge of something that expected ground there.
        //
        // The list is exact, never a blanket: a side declared here that the
        // world has in fact buried is refused by the same code, so padding it
        // out to six reds rather than passing. That is what makes this a bound
        // declaration and not a hatch — perturb it either way and the gallery
        // goes red, which is what `gallery/probes/a-face-nothing-stands-in-front-of`
        // pins.
        "shown_faces": ["down"],
        "spatial_contract": {
            "entry": "yard",
            "spaces": {
                "yard": {
                    "envelope": "open_top",
                    "boxes": [region([0, 1, 0], [YARD_SIZE[0] - 1, YARD_SIZE[1] - 1, YARD_SIZE[2] - 1])]
                }
            },
            "no_body": {},
            "edges": [
                {
                    "a": "yard",
                    "b": "exterior",
                    "class": "walk",
                    "via": { "region": "yard-arch", "boxes": [region(wl, wh)] }
                }
            ],
            "faces": [
                { "space": "yard", "class": "walk", "dir": "north", "opening": region(wl, wh) }
            ]
        },
        "lighting": {
            "profile": "lit",
            "measured_min_light": 15,
            "measured": "2026-08-21",
            "method": "derived: an open-top courtyard under the site plan's own sky volume, \
                       so its floor takes full daylight and needs no fixture"
        },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Original Delvewright project asset (pipeline-code license per \
                     prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            "provenance": "Generated deterministically by prefabs/gallery-generator (ADR-0006)."
        }
    })
}

fn write_yard(out: &Path) {
    let s = build_yard();
    let cells = invariant_cells(&s);
    invariants::assert_blocks_are_real(YARD_ID, &cells);
    connections::assert_shape_is_stated(YARD_ID, &cells);
    invariants::assert_fluid_is_contained(YARD_ID, s.size, &cells);
    // The piece's own claim about where a body stands, proved against its own
    // blocks rather than trusted — the same rule `assert_anchors_are_standable`
    // holds the hall to, and the reason a detail piece may carry an owed anchor
    // at all.
    let solid: std::collections::BTreeSet<[i32; 3]> = cells
        .iter()
        .filter(|(_, (name, _))| name.as_str() != "minecraft:air")
        .map(|(p, _)| *p)
        .collect();
    let [ax, ay, az] = YARD_SEAT;
    assert!(
        !solid.contains(&[ax, ay, az])
            && !solid.contains(&[ax, ay + 1, az])
            && solid.contains(&[ax, ay - 1, az]),
        "{YARD_ID}: `yard-stone` at {YARD_SEAT:?} is not a cell a body can stand in"
    );
    // And the way in is really open, in the bytes, at exactly the cells the
    // metadata claims. A face contract nothing checks is a claim, not a way.
    let (wl, wh) = YARD_WAY;
    for x in wl[0]..=wh[0] {
        for y in wl[1]..=wh[1] {
            for z in wl[2]..=wh[2] {
                assert!(
                    !solid.contains(&[x, y, z]),
                    "{YARD_ID}: the seam cell [{x}, {y}, {z}] is solid, so the piece seals \
                     the plan's only way into this place"
                );
            }
        }
    }

    let nbt = fastnbt::to_bytes(&s).expect("structure serializes to NBT");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip write");
    let framed = gz.finish().expect("gzip finish");
    std::fs::write(out.join(format!("{YARD_ID}.nbt")), &framed).expect("write yard nbt");
    let mut yard = yard_metadata();
    declare_walk_y(YARD_ID, &s, &mut yard);
    document::write_preserving(&out.join(format!("{YARD_ID}.json")), &yard);
    println!(
        "{YARD_ID}: detail piece written — {}x{}x{} to fill the exit box's frame exactly",
        YARD_SIZE[0], YARD_SIZE[1], YARD_SIZE[2]
    );
}

// ---------------------------------------------------------------------------
// The QUAY: the one gallery piece the party can walk OUTSIDE of, on a sea
// ---------------------------------------------------------------------------

/// The shore piece's id.
///
/// # What it is here to make askable
///
/// The gallery's ocean point was four sealed boxes. Every one of them is solid
/// on all six sides and every one of those sides has nothing in front of it —
/// and `DW0885` correctly judged none of them, because there is nowhere out
/// there for a body to be. So the ocean's own power to bury an outward face —
/// the thing that makes `ocean` seat a keep at all — was asserted nowhere, and
/// the point's exposure binding read `0 judged` and `0 shown_faces
/// declaration(s) of which 0 are bound`. A zero binding is a finding
/// (spec-0060 §11.3).
///
/// A quay closes it. It is an open court on two courses of plinth, so the air a
/// body stands in leaves through the sky and runs around the outside, and every
/// one of its four walls is then judged. Its plinth stands below y=62 and the
/// SEA buries it; its parapet stands above and the piece answers for it in
/// `shown_faces`. One piece, both halves of the rule, on the base where the
/// answer is the water itself.
const QUAY_ID: &str = "gallery-quay";

/// Extent before the shore lift; `to_shore` adds [`SHORE_PLINTH`] courses under
/// it and grows the y.
const QUAY_SIZE: [i32; 3] = [8, 4, 8];

/// The tide pool cut into the quay's own paving, in `(x, z)`. Clear of the
/// mooring anchor, and interior on every side so the water is the piece's own
/// rather than a run off its face.
const QUAY_POOL: [(i32, i32); 2] = [(1, 1), (1, 2)];

/// Where a body stands on the quay, before the lift.
const QUAY_SEAT: [i32; 3] = [4, 1, 4];

/// A quay: paving, a parapet around three sides, and a mooring post.
///
/// The parapet is what makes the piece answerable. A court with nothing above
/// its floor course would put every solid boundary cell under the sea, the sea
/// would bury all of them, and the binding would be non-zero and prove nothing
/// about `shown_faces`. What is wanted is both: cells the water buries and
/// cells the document answers for.
fn build_quay() -> Structure {
    let mut palette = Palette::new();
    let mut blocks = Vec::new();
    let [sx, sy, sz] = QUAY_SIZE;
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let edge = x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                // The way in: the north face is left open at the paving's own
                // level, so a body can walk off the quay into the water and
                // back up onto it — the beach relationship the ocean datum is.
                let opening = z == 0 && (3..=4).contains(&x);
                let lamp =
                    (1..=2).contains(&y) && matches!((x, z), (0, 0) | (0, 7) | (7, 0) | (7, 7));
                let name = if y == 0 {
                    "minecraft:polished_andesite"
                } else if lamp {
                    "minecraft:sea_lantern"
                } else if (1..=2).contains(&y) && edge && !opening {
                    "minecraft:polished_blackstone_bricks"
                } else {
                    "minecraft:air"
                };
                blocks.push(BlockEntry {
                    pos: [x, y, z],
                    state: palette.idx(name, None),
                });
            }
        }
    }
    Structure {
        data_version: DATA_VERSION,
        size: QUAY_SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

/// The quay's document, before the shore lift.
///
/// `shown_faces` names the four walls and NOTHING else, and the exactness is
/// the whole demonstration. `down` is the plinth's underside, standing under
/// the sea, which the water buries; `up` is open sky over a court with no solid
/// cell on its top plane, so there is no side there to show. `DW0885` refuses a
/// declared side the world has in fact buried and a declared side of pure air
/// alike, so padding this list out to six reds — which is what makes these four
/// a bound declaration rather than a hatch.
fn quay_metadata() -> serde_json::Value {
    serde_json::json!({
        "prefab_id": format!("prefab/{QUAY_ID}"),
        "structure": {
            "file": format!("{QUAY_ID}.nbt"),
            "id": QUAY_ID,
            "size": QUAY_SIZE,
            "data_version": DATA_VERSION,
            "generator": "prefabs/gallery-generator (gallery-prefab-gen)"
        },
        "anchors": {
            "quay-mooring": {
                "pos": QUAY_SEAT,
                "facing": "north",
                "note": "where a body stands on the quay, clear of the tide pool"
            }
        },
        "shown_faces": ["east", "north", "south", "west"],
        "lighting": {
            "profile": "lit",
            "measured_min_light": 15,
            "measured": "2026-09-07",
            "method": "derived: four sea lanterns in the parapet corners, over a court open to the sky"
        },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Original Delvewright project asset (pipeline-code license per prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            "provenance": "Generated deterministically by prefabs/gallery-generator (ADR-0006)."
        }
    })
}

fn write_quay(out: &Path) {
    let court = build_quay();
    let (s, meta) = to_shore(QUAY_ID, &court, &quay_metadata(), &QUAY_POOL);
    let cells = invariant_cells(&s);
    invariants::assert_blocks_are_real(QUAY_ID, &cells);
    connections::assert_shape_is_stated(QUAY_ID, &cells);
    let fluid = invariants::assert_fluid_is_contained(QUAY_ID, s.size, &cells);

    let nbt = fastnbt::to_bytes(&s).expect("structure serializes to NBT");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip write");
    let framed = gz.finish().expect("gzip finish");
    std::fs::write(out.join(format!("{QUAY_ID}.nbt")), &framed).expect("write quay nbt");
    document::write_preserving(&out.join(format!("{QUAY_ID}.json")), &meta);
    println!(
        "{QUAY_ID}: shore piece written — walk plane at local y={}, waterline {}, \
         {} fluid source(s) examined, {} at the piece's own face",
        meta["walk_y"], meta["waterline_y"], fluid.examined, fluid.at_edge
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next() else {
        eprintln!(
            "usage: gallery-prefab-gen <out_dir> [--skins <skins_dir>] \
              [--design <design_dir>]   \
             (a BUILD directory — spec-0039 §6 commits no generated bytes)"
        );
        std::process::exit(2);
    };
    let mut skins: Option<String> = None;
    let mut design: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--skins" => skins = args.next(),
            "--design" => design = args.next(),
            other => {
                eprintln!("gallery-prefab-gen: unknown argument `{other}`");
                std::process::exit(2);
            }
        }
    }
    let out = Path::new(&out);
    // Same rule as every other generator: a path this program created is a path
    // nothing reads, so a typo must fail rather than succeed into a void.
    if !out.is_dir() {
        eprintln!(
            "gallery-prefab-gen: {} is not an existing directory. Create the build \
             directory you mean to generate into and point this at it; this generator \
             will not create it, because a path it created is a path nothing reads.",
            out.display()
        );
        std::process::exit(2);
    }
    write_piece(out);
    write_annex(out);
    write_shard(out);
    write_yard(out);
    write_quay(out);
    // The skins destination IS created: unlike the prefab directory it is not an
    // existing library the operator might mistype, it is a fixed subdirectory of
    // the campaign the caller just named, and it is gitignored build output.
    if let Some(s) = skins {
        write_skins(Path::new(&s));
    }
    // The design destination is created for the same reason the skins one is:
    // it is a fixed subdirectory of the campaign the caller just named, and it
    // is gitignored build output (spec-0039 §6 commits no generated bytes).
    if let Some(d) = design {
        write_design(Path::new(&d));
    }
}
