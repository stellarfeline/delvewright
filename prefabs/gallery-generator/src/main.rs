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

/// **The ferry cabin** (spec-0083): a sealed room in the far hall's west corner
/// that no walk, gap or door reaches — the place only a link carries the party
/// to. `(x0, x1, z0, z1)` inclusive, the cabin's footprint against the hall's
/// west and back walls: its own walls stand on `x = x1` and `z = z0`, its roof
/// is the course over the footprint, and its inside is `x0..x1`, `z0+1..=z1`,
/// three courses tall. The west corner because the east one is where the
/// stage-7 script plants its oak, and the far floor's scatter is told to avoid
/// the cabin and the deck (`gallery/world-edits.json`).
const CABIN: (i32, i32, i32, i32) = (1, 4, 25, 29);

/// The cabin's roof course: walls stand `y ∈ 1..CABIN_ROOF_Y` and the roof is
/// solid at `y = CABIN_ROOF_Y`.
const CABIN_ROOF_Y: i32 = 4;

/// The cabin's own light, hung from its roof: the hall's lantern grid is above
/// the roof and lights nothing inside a sealed room.
const CABIN_LANTERN: [i32; 3] = [2, 3, 27];

/// The top of the ferryman's chest against the cabin's far wall (the chest
/// stands in the cell under it). What the arrival view in the cabin frames.
const CABIN_LAMP: [i32; 3] = [2, 2, 29];

/// MC 1.21.11 data version (ADR-0009).
const DATA_VERSION: i32 = 4671;

/// The piece's id — the `.nbt`/`.json` stem and what the invariants report against.
const ID: &str = "gallery-hall";

/// Structure extent: 31 (x) × 12 (y) × 31 (z): the hall, roofed at
/// [`HALL_ROOF_Y`], and the long gallery standing on its roof.
///
/// One room, deliberately — and one corridor on top of it, reached by a stair
/// from the far hall, because a place the party is carried to across the void
/// would put the loop behind a crossing and its proofs behind a checkpoint the
/// crossing outruns. The gallery's job is to be exhaustive over the DSL
/// and **legible**, and a maze of chambers would make the second impossible: a
/// reader looking up where `anchor/hearth` is should find it on one floor plan.
/// Everything vertical the DSL can express is expressed against the same floor.
const SIZE: [i32; 3] = [31, 12, 31];

/// The hall's roof course: the room is `y ∈ 1..HALL_ROOF_Y`, and the long
/// gallery's floor IS this course.
const HALL_ROOF_Y: i32 = 7;

/// The z of the dividing wall that gives the hall a far side worth opening a
/// gate onto. Everything with `z < DIVIDER_Z` is the near hall (spawn, the two
/// speaking parts, the pedestal); everything beyond it is the far hall.
const DIVIDER_Z: i32 = 15;

/// The openings in the divider, as `(x_from, x_to)` inclusive. Each is
/// three tall (`y ∈ 1..=3`) and filled with iron bars, which is what a
/// prefab-declared gate anchor opens.
const GATES: [(i32, i32); 5] = [(14, 15), (24, 25), (4, 5), (9, 10), (19, 20)];

/// **The west terrace, and the well cut through it** — the gallery's pit whose
/// keep-out lies under the rim (spec-0062 §10).
///
/// A killing volume that catches floor the party walks is refused (`DW0891`),
/// and the repair is to MOVE THE HAZARD, never to mark walkable-looking ground
/// unwalkable. So the west pit is a real pit: a terrace three courses over the
/// near hall's west corner, with a one-cell well cut down through it to the
/// hall's own floor. The volume sits at the well's bottom, its keep-out's top
/// course lies a full body under the rim, and every cell of that keep-out at the
/// walk plane is terrace stone.
///
/// `(x0, x1, z0, z1)` inclusive, in room space.
const TERRACE: (i32, i32, i32, i32) = (0, 5, 1, 6);

/// The terrace's top solid course: solid `y ∈ 1..=TERRACE_TOP_Y`, so its rim is
/// walked at `y = TERRACE_TOP_Y + 1`.
///
/// Three courses is not a round number, it is the rule: a body steps one course
/// ([`prefab_invariants`]'s own walk), so a rim three over the well's floor is a
/// place nothing can walk down into. The well's bottom is therefore outside the
/// population `DW0891` measures against, which is what makes a hole in the floor
/// *checked and clear* rather than *caught*.
const TERRACE_TOP_Y: i32 = 3;

/// The well's column, `(x, z)` — the cell `anchor/west-pit` names, cut open from
/// the hall's floor to the terrace's rim.
const WELL: (i32, i32) = (2, 3);

/// **The lidded pit** — a second pit sunk in the terrace, roofed by the terrace's
/// own top course until a story beat clears it (spec-0088).
///
/// Two courses of air under the lid (`y ∈ 1..TERRACE_TOP_Y`) and the lid itself,
/// terrace stone, at `y = TERRACE_TOP_Y`. It is sunk in [`TERRACE_ANNEX`], not in
/// the terrace proper: the west well's reach (`obj/look-into-the-well`,
/// radius 3) covers the whole terrace, and a sealed chamber a body could stand
/// in inside that volume is floor it cannot walk to the well from (`DW0881`). The volume `lethal/lid-pit`
/// sits at the pit's bottom on `anchor/lid-pit`, three courses under the rim, so
/// no rim cell is in its keep-out; the beat that arms it clears `anchor/lid`, so
/// the floor that was a lid becomes a hole in the same beat the bottom starts to
/// kill. Before the beat nobody can reach the pit; after it a fall does. The
/// column is inside the terrace on every side, so its keep-out lies wholly under
/// stone.
const LID_PIT: (i32, i32) = (8, 5);

/// **The terrace annex** — the terrace carried east beside its treads, at the
/// same height, so the lidded pit has a terrace floor on every side and stands
/// clear of the west well's reach. `(x0, x1, z0, z1)` inclusive, in room space;
/// walked onto from the terrace at `x = 5`.
const TERRACE_ANNEX: (i32, i32, i32, i32) = (6, 9, 4, 6);

/// The treads that climb the terrace from the near hall, `(x, top_y)` on the
/// well's own `z` — one course of rise each, so the rim is somewhere the party
/// can actually stand and look in. Without them the terrace is scenery and every
/// reach anchored on the well is `DW0850`.
///
/// They stand OUTSIDE the terrace, and the terrace is drawn wide enough that no
/// hall-floor cell lies nearer the well than its own rim does. That is not
/// decoration: `DW0881` measures the anchor's footing as the nearest standable
/// cells to it, so a strip of hall floor two cells south of the well would be
/// "where a body arrives at the well" and the rim three courses up would be
/// floor nothing can walk to it from.
const TERRACE_STEPS: [(i32, i32); 2] = [(7, 1), (6, 2)];

/// **The east strip: a flush hazard that shows itself** (spec-0062 §3).
///
/// The other half of the ruling. This volume is level with the floor on purpose
/// — the block IS the signal — so the floor course under every cell it catches
/// is molten stone, and `lethal/east-pit` declares `shown_by`. The span is the
/// volume's keep-out at the walk plane, one cell wider than the volume on every
/// side the walls leave open, because that is exactly how far a body's hitbox
/// reaches into it.
///
/// It lies in the far corner of the barrier pocket, and the corner is the point:
/// a killing volume kills every body that is not a player, so no wave may be
/// seated within its pursuit of one (`DW0922`). The pocket's corner is the floor
/// of the near hall furthest from every place the hall seats a fight — more than
/// a sixteen-block pursuit from the muster — and the party still walks onto it.
///
/// `(x0, x1, z0, z1)` inclusive, in room space.
const BURNING_STRIP: (i32, i32, i32, i32) = (28, 29, 1, 2);

/// What the burning strip is made of — one of the blocks vanilla hurts a body
/// with ([`delvewright_dsl::blockshape::HURTING_BLOCKS_1_21_11`], reached here
/// through the campaign's own `shown_by`), and a full cube, so the floor it
/// makes is floor.
const BURNING_BLOCK: &str = "minecraft:magma_block";

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
        name: "anchor/usher",
        pos: [13, 1, 5],
        facing: Some("south"),
        trigger_block: None,
        note: "a standing place for two bodies a spawn puts down and NOTHING \
               takes away: the usher on the anchor and the page at an offset \
               from it, four cells east along the speaking row. `DW0896` refuses \
               two co-existing bodies on one cell and judges an entry only \
               against a body whose lifetime it can bound — every other actor \
               in this hall is `vulnerable` or is despawned, so without this \
               place the live half of that rule binds to the world-init npcs \
               alone. One anchor and an offset apiece is how a rank of bodies \
               is placed (spec-0066): the campaign spends one anchor on it, and \
               `DW0897` holds each offset inside this piece",
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
        name: "anchor/strongbox",
        pos: [29, 1, 17],
        facing: Some("west"),
        trigger_block: Some("minecraft:trapped_chest[facing=west,type=single]"),
        note: "the false chest: a `trapped-chest` trap's trigger is the chest the \
               piece places at its cell, and a trap whose cell holds anything else \
               is DW0917. Against the east wall, clear of the east bay's fill box, \
               so no region verb overwrites it",
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
        name: "anchor/lid-pit",
        pos: [LID_PIT.0, 1, LID_PIT.1],
        facing: None,
        trigger_block: None,
        note: "the bottom of the lidded pit in the terrace: a killing volume live \
               from the beat that clears the lid over it (spec-0088), three \
               courses under the rim, in the near hall with the west pit and for \
               the same reason (MUSTER_PIT_CLEARANCE)",
        role: None,
    },
    Anchor {
        name: "anchor/lid-rim",
        pos: [LID_PIT.0, TERRACE_TOP_Y + 1, LID_PIT.1],
        facing: None,
        trigger_block: None,
        note: "the terrace floor standing on the lid — the cell a body crosses \
               before the beat and falls through after it",
        role: None,
    },
    Anchor {
        name: "anchor/east-pit",
        pos: [29, 1, 1],
        facing: None,
        trigger_block: None,
        note: "the second killing volume, so the two never share a box: one \
               column in the barrier pocket's north-east corner, off every route \
               the piece's own bodies walk or are teleported along, and beyond the \
               pursuit of every fight the hall seats (DW0922). Its `extent` is 0 on \
               x and z, so the walls behind it are not part of it",
        role: None,
    },
    Anchor {
        name: "anchor/plinth",
        pos: [23, 1, 12],
        facing: Some("west"),
        trigger_block: None,
        note: "where the sentinel stands — the gallery's assembly (spec-0082): a \
               statue of display entities that sways, can be struck, and stamps \
               the 3 x 3 of floor round its own feet. Its arming region and its \
               landing box are this one cell, so a body is caught only from the \
               ring of cells beside it. Three cells east of the walk through the \
               near hall and two north of the counter, so the critical path never \
               stands where it stamps",
        role: None,
    },
    Anchor {
        name: "anchor/reach",
        pos: [7, 1, 12],
        facing: Some("east"),
        trigger_block: None,
        note: "the middle of the floor the reaching arm locks onto (spec-0094): the arm stands \
               five cells west of here at the wall, its lock region is this cell's 5 x 5 and \
               its arming region the 9 x 7 round it. South of the hearth and west of the walk \
               through the near hall, so the critical path crosses the arming region only on \
               its way to the hearth, never the lock region",
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
    // The three valves on the near hall's north wall: a numeric gate that
    // only presses move. `obj/reach-the-counter` waits on `state/valve-round`
    // reaching 3, which nothing but these three `use` triggers write, and a
    // valve pressed out of turn puts the round back to 0. The plan replays
    // every press the way the datapack runs it and schedules the order that
    // opens the gate (`DW0985` when none does), so the bot presses first,
    // second, third — although the document declares them the other way
    // round. One press, one anchor (DW0878), two cells apart so each lever is
    // read as its own.
    Anchor {
        name: "anchor/valve-first",
        pos: [19, 1, 1],
        facing: Some("south"),
        trigger_block: None,
        note: "the first valve: starts the round",
        role: None,
    },
    Anchor {
        name: "anchor/valve-second",
        pos: [21, 1, 1],
        facing: Some("south"),
        trigger_block: None,
        note: "the second valve: answers only after the first, and resets the round \
               if it is pressed again",
        role: None,
    },
    Anchor {
        name: "anchor/valve-third",
        pos: [23, 1, 1],
        facing: Some("south"),
        trigger_block: None,
        note: "the third valve: finishes the round after the second, and resets it if \
               pressed straight after the first",
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
    Anchor {
        name: "anchor/ferry-deck",
        pos: [9, 1, 21],
        facing: Some("east"),
        trigger_block: None,
        note: "the centre of the ferry's deck: the link's volume is this cell \
               \u{b1}[1, 1, 1], and the party stands in its east column to pull \
               the tiller. Off the patrol lane and clear of both bays' runtime \
               regions, so nothing the campaign writes moves a deck cell",
        role: None,
    },
    Anchor {
        name: "anchor/ferry-tiller",
        pos: [13, 1, 21],
        facing: Some("west"),
        trigger_block: None,
        note: "the ferry's tiller: outside the deck's volume and within a strike \
               of its east column only, so the volume shrunk to its middle column \
               holds no cell a body pulls it from",
        role: None,
    },
    Anchor {
        name: "anchor/ferry-landing",
        pos: [2, 1, 27],
        facing: Some("south"),
        trigger_block: None,
        note: "inside the sealed cabin: where the link puts the party down, \
               eight blocks west of the deck's east column so the bot can see \
               the carry happen",
        role: None,
    },
    Anchor {
        name: "anchor/cabin",
        pos: [2, 1, 28],
        facing: Some("north"),
        trigger_block: None,
        note: "in front of the ferryman's chest against the cabin's far wall: the beat only \
               the link reaches",
        role: None,
    },
    Anchor {
        name: "anchor/cabin-chest",
        pos: CABIN_LAMP,
        facing: None,
        trigger_block: None,
        note: "the top of the ferryman's chest, the thing a body arriving in the cabin \
               looks at",
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
    Anchor {
        name: "anchor/long-gallery-end",
        pos: long_gallery_cell(2, 1, CORRIDOR_SIZE[2] - 3),
        facing: Some("north"),
        trigger_block: None,
        note: "the long gallery's end room, past its three bays: the beat the loop stands in \
               front of until the party has crossed it enough",
        role: None,
    },
    Anchor {
        name: "anchor/long-gallery-chest",
        pos: long_gallery_cell(2, 2, CORRIDOR_SIZE[2] - 2),
        facing: None,
        trigger_block: None,
        note: "the top of the chest at the long gallery's far end, two bays past anything a \
               body in the loop's span can see: what the end room holds",
        role: None,
    },
    Anchor {
        name: "anchor/long-gallery-hall",
        pos: long_gallery_cell(3, 1, corridor_bay(1) + 3),
        facing: Some("north"),
        trigger_block: None,
        note: "a cell of the long gallery's bay 1, inside the loop's span: where a probe posts a \
               figure the move cannot repeat",
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
const SOLID_ANCHORS: &[Anchor] = &[
    Anchor {
        name: "anchor/east-vault",
        pos: [25, 5, 22],
        facing: None,
        trigger_block: None,
        note: "the stone canopy a `collapse` brings down onto the east bay floor",
        role: None,
    },
    Anchor {
        name: "anchor/lid",
        pos: [LID_PIT.0, TERRACE_TOP_Y, LID_PIT.1],
        facing: None,
        trigger_block: None,
        note: "the lid over the lidded pit: terrace stone a `clear-region` takes \
               away in the beat that arms `lethal/lid-pit` (spec-0088)",
        role: None,
    },
];

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

/// **The high table** — a laid dining table across the standard-bearer's walk
/// from `anchor/march` to `anchor/vantage` (spec-0065 §7).
///
/// Built the way the released castle's hall table is built, because that table
/// is the finding: a row of `oak_fence` legs under an `oak_slab[type=bottom]`
/// top, with a `spruce_slab[type=bottom]` bench on its near side. Every cell of
/// the top is standable under the walk model's own rule, and bench-then-top is a
/// half-block step and a one-block jump, so a body routed from the march to the
/// vantage takes three cells over the table where the way round is eleven — long
/// enough that the router's elevation cost (a block of rise or fall is two of
/// walking) still prefers the climb. The piece declares it furniture, and the
/// walk goes round.
///
/// `(x0, x1, z)` inclusive: the legs stand at `y = 1`, the top at `y = 2`.
const TABLE: (i32, i32, i32) = (12, 18, 26);

/// The bench on the table's near (south) side: `(x0, x1, z)`, at `y = 1`. The
/// far side has none — the back wall's levers and the reliquary stand there.
const BENCH: (i32, i32, i32) = (12, 18, 25);

/// The table's leg, top and bench blocks, each written once.
const TABLE_LEG: &str = "minecraft:oak_fence";
const TABLE_TOP: &str = "minecraft:oak_slab";
const BENCH_BLOCK: &str = "minecraft:spruce_slab";

/// A named place declared as furniture: the region is the furniture's own
/// blocks (spec-0065 §3), and the anchor carries `role: furniture`.
struct FurnitureAnchor {
    name: &'static str,
    from: [i32; 3],
    to: [i32; 3],
    note: &'static str,
}

/// The furniture inventory. One table, so the element answers one question: a
/// body walked past a table goes round it.
const FURNITURE_ANCHORS: &[FurnitureAnchor] = &[FurnitureAnchor {
    name: "anchor/high-table",
    from: [TABLE.0, 1, TABLE.2],
    to: [TABLE.1, 2, TABLE.2],
    note: "the laid table across the standard-bearer's walk to the vantage: legs and \
           top are furniture, so the bearer walks round it rather than over it",
}];

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
                out.push([x, HALL_ROOF_Y - 1, z]);
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
    out.push([19, HALL_ROOF_Y - 1, 18]);
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
    if y == 0 {
        let (bx0, bx1, bz0, bz1) = BURNING_STRIP;
        if (bx0..=bx1).contains(&x) && (bz0..=bz1).contains(&z) {
            return (BURNING_BLOCK, None);
        }
        return ("minecraft:stone", None);
    }
    if y >= HALL_ROOF_Y {
        // The long gallery on the roof and the shaft its stair climbs through;
        // everything else above the roof is solid, so no air of the piece's own
        // reaches its box boundary and nothing has to answer for an outside a
        // body could stand in (`DW0886`).
        if let Some(b) = long_gallery_at(x, y, z) {
            return b;
        }
        return ("minecraft:stone", None);
    }
    if x == 0 || x == SIZE[0] - 1 || z == 0 || z == SIZE[2] - 1 {
        return ("minecraft:stone", None);
    }
    // The long gallery's stair: one course of rise per cell, climbing east
    // along the far wall from the far hall's floor to the gallery's porch, in
    // the corner away from the lane the boss marches.
    if z == LONG_GALLERY_STAIR_Z && (21..=27).contains(&x) && y <= x - 20 {
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
    // The west terrace and its well. The well column is cut before the terrace
    // fills, so the hole is a hole rather than a cell the terrace happens to
    // miss — one order, one authority.
    let (tx0, tx1, tz0, tz1) = TERRACE;
    if (tx0..=tx1).contains(&x)
        && (tz0..=tz1).contains(&z)
        && (1..=TERRACE_TOP_Y).contains(&y)
        && (x, z) != WELL
    {
        return ("minecraft:stone", None);
    }
    let (ax0, ax1, az0, az1) = TERRACE_ANNEX;
    if (ax0..=ax1).contains(&x)
        && (az0..=az1).contains(&z)
        && (1..=TERRACE_TOP_Y).contains(&y)
        && ((x, z) != LID_PIT || y == TERRACE_TOP_Y)
    {
        return ("minecraft:stone", None);
    }
    if z == WELL.1
        && TERRACE_STEPS
            .iter()
            .any(|&(sx, top)| sx == x && (1..=top).contains(&y))
    {
        return ("minecraft:stone", None);
    }
    // The high table and its bench (spec-0065 §7).
    if (TABLE.0..=TABLE.1).contains(&x) && z == TABLE.2 {
        match y {
            1 => return (TABLE_LEG, None),
            2 => return (TABLE_TOP, Some(&[("type", "bottom")])),
            _ => {}
        }
    }
    if (BENCH.0..=BENCH.1).contains(&x) && z == BENCH.2 && y == 1 {
        return (BENCH_BLOCK, Some(&[("type", "bottom")]));
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
    // The ferry cabin: walls on the footprint's open edges, a roof over them,
    // and its own lantern. Nothing cuts a way in.
    let (fx0, fx1, fz0, fz1) = CABIN;
    if (fx0..=fx1).contains(&x) && (fz0..=fz1).contains(&z) && (1..=CABIN_ROOF_Y).contains(&y) {
        if [x, y, z] == CABIN_LANTERN {
            return ("minecraft:lantern", Some(&[("hanging", "true")]));
        }
        if [x, y, z] == [CABIN_LAMP[0], CABIN_LAMP[1] - 1, CABIN_LAMP[2]] {
            return ("minecraft:barrel", None);
        }
        if y == CABIN_ROOF_Y || x == fx1 || z == fz0 {
            return ("minecraft:stone", None);
        }
        return ("minecraft:air", None);
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
        (BURNING_BLOCK, None),
        (TABLE_LEG, None),
        (TABLE_TOP, Some(&[("type", "bottom")][..])),
        (BENCH_BLOCK, Some(&[("type", "bottom")][..])),
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
    // A volume's centre hangs in the air it centres.
    for a in VOLUME_ANCHORS {
        assert_eq!(
            at(a.pos),
            "minecraft:air",
            "{ID}: volume anchor `{}` is not in air",
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
    // Furniture is declared over the blocks it names, never over air: every cell
    // of the region is a leg or the top, and at least one of them is a top a
    // body could otherwise stand on (`DW0888`'s furniture key, asked here first).
    for f in FURNITURE_ANCHORS {
        let mut tops = 0usize;
        for x in f.from[0]..=f.to[0] {
            for y in f.from[1]..=f.to[1] {
                for z in f.from[2]..=f.to[2] {
                    let found = at([x, y, z]);
                    assert!(
                        found == TABLE_LEG || found == TABLE_TOP,
                        "{ID}: furniture `{}` claims {:?}, which holds `{found}` — not the \
                         table's own blocks",
                        f.name,
                        [x, y, z]
                    );
                    if found == TABLE_TOP && at([x, y + 1, z]) == "minecraft:air" {
                        tops += 1;
                    }
                }
            }
        }
        assert!(
            tops > 0,
            "{ID}: furniture `{}` has no top a body could stand on, so declaring it \
             withholds nothing",
            f.name
        );
    }
    // A metadata that declares nothing is the vacuous case: the assertions above
    // are all universally quantified and pass over an empty inventory.
    assert!(
        !ANCHORS.is_empty()
            && !GATE_ANCHORS.is_empty()
            && !CONTAINERS.is_empty()
            && !SOLID_ANCHORS.is_empty()
            && !FURNITURE_ANCHORS.is_empty(),
        "{ID}: the anchor inventory is empty, so nothing above examined anything"
    );
    println!(
        "{ID}: anchor inventory bound — {} point anchor(s), {} container(s), \
         {} solid anchor(s), {} gate anchor(s), {} furniture anchor(s) checked against the blocks",
        ANCHORS.len(),
        CONTAINERS.len(),
        SOLID_ANCHORS.len(),
        GATE_ANCHORS.len(),
        FURNITURE_ANCHORS.len()
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
    for pit in ["anchor/west-pit", "anchor/lid-pit", "anchor/east-pit"] {
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
        3,
        "{ID}: the pit clearance examined {} volume(s), not 3",
        pits.len()
    );
    println!(
        "{ID}: muster clearance bound — 3 killing volume(s), all across the wall at \
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
    let (top, floor) = (HALL_ROOF_Y - 1, 1);

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
    for a in ANCHORS
        .iter()
        .chain(CONTAINERS)
        .chain(SOLID_ANCHORS)
        .chain(VOLUME_ANCHORS)
    {
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
    for f in FURNITURE_ANCHORS {
        let mut m = Map::new();
        m.insert("region".into(), json!({ "from": f.from, "to": f.to }));
        m.insert("role".into(), json!("furniture"));
        m.insert("note".into(), json!(f.note));
        anchors.insert(f.name.into(), Value::Object(m));
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
/// `(texture_id, base_rgb, belt_rgb, model)`. Two, because `NpcSkin.model` has
/// two members and a `slim` skin and a `wide` skin must both exist for the pair
/// to be written; the model is the one `npcs.json` / `quests.json` wears it on,
/// because a skin is drawn to its model's boxes (spec-0097).
const SKINS: [(&str, [u8; 3], [u8; 3], &str); 2] = [
    (
        "curator",
        [0x3A, 0x3F, 0x55],
        [0xC9, 0xA2, 0x27],
        "player_slim",
    ),
    ("bearer", [0x5A, 0x3A, 0x2E], [0xB8, 0xC4, 0xCF], "player"),
];

/// The model-part table the engine judges every skin and entity texture by
/// (spec-0097 §3), read rather than restated: the gallery's sheets paint where
/// the pinned client's boxes are because they are cut from the same rows.
const MODEL_PARTS: &str = include_str!("../../../crates/delvec/data/model-parts-1.21.11.json");

/// The six face rectangles `(x, y, w, h)` of every box of `model` that `keep`
/// admits, at scale 1, with the box's face name.
fn model_faces(
    model: &str,
    keep: impl Fn(&serde_json::Value) -> bool,
) -> Vec<(&'static str, u32, u32, u32, u32)> {
    let table: serde_json::Value =
        serde_json::from_str(MODEL_PARTS).expect("the model-part table parses");
    let cubes = table["models"][model]["cubes"]
        .as_array()
        .unwrap_or_else(|| panic!("the model-part table has no model `{model}`"));
    let mut out = Vec::new();
    for c in cubes.iter().filter(|c| keep(c)) {
        let n = |k: &str| c[k].as_u64().expect("a whole-texel box") as u32;
        let (u, v, w, h, d) = (n("u"), n("v"), n("w"), n("h"), n("d"));
        out.extend([
            ("up", u + d, v, w, d),
            ("down", u + d + w, v, w, d),
            ("left", u, v + d, d, h),
            ("front", u + d, v + d, w, h),
            ("right", u + d + w, v + d, d, h),
            ("back", u + 2 * d + w, v + d, w, h),
        ]);
    }
    out
}

/// The base boxes of a model: grow 0.
fn is_base(c: &serde_json::Value) -> bool {
    c["grow"].as_f64() == Some(0.0)
}

/// A `w`×`h` 8-bit RGBA PNG whose pixel at `(x, y)` is `px(x, y)`.
fn rgba_png(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((h * (1 + w * 4)) as usize);
    for y in 0..h {
        raw.push(0); // filter type 0 (None) on every scanline
        for x in 0..w {
            raw.extend_from_slice(&px(x, y));
        }
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), Compression::new(6));
    z.write_all(&raw).expect("zlib write");
    let idat = z.finish().expect("zlib finish");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA, deflate, no filter, no interlace
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&png_chunk(b"IDAT", &idat));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

/// Whether `(x, y)` lies on one of `faces`.
fn on_faces(faces: &[(&'static str, u32, u32, u32, u32)], x: u32, y: u32) -> bool {
    faces
        .iter()
        .any(|&(_, fx, fy, fw, fh)| (fx..fx + fw).contains(&x) && (fy..fy + fh).contains(&y))
}

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

/// A 64×64 RGBA mannequin skin, flat-coloured with a belt band, painted on
/// exactly the faces of `model`'s base boxes and transparent everywhere else —
/// so the overlay is present and empty, and no pixel lands where no box reads.
///
/// Deliberately not art. A skin here exists so `NpcSkin.texture_id` names
/// something that resolves and `DW0309` has a file to find; the *look* of a
/// mannequin is a campaign's business, and the gallery having an opinion about
/// it would be authored content wearing a test surface's clothes.
fn skin_png(base: [u8; 3], belt: [u8; 3], model: &str) -> Vec<u8> {
    let faces = model_faces(model, is_base);
    rgba_png(64, 64, |x, y| {
        let c = if (20..24).contains(&y) { belt } else { base };
        let a = if on_faces(&faces, x, y) { 0xFF } else { 0x00 };
        [c[0], c[1], c[2], a]
    })
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

/// The seven scenes — see [`DesignScene`] for the tuple. The seventh is the
/// east bay after the muster is cleared (spec-0089): the hall's own noon, the
/// picture of a room a beat has changed, which a showcase camera answers from
/// the configuration after that beat.
const DESIGN_SCENES: [DesignScene; 7] = [
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
    (
        "bay-after-the-muster",
        [170, 200, 235],
        [150, 150, 145],
        [235, 235, 225],
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
/// picture from its six neighbours.
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

/// The images the gallery's `world.textures[]` row and its probes name
/// (spec-0084 §7), written into `--textures <dir>`.
///
/// - `hall-stone` replaces `minecraft:block/stone_bricks` — the annex tiles'
///   walls, the hall's tread courses and the seal an unmated socket gets: one
///   flat colour, deliberately not art, so a render of the annex shows at a
///   glance which block the pack replaced.
/// - `wrong-shape` is 24×24, which no 16×16 texture can be replaced by —
///   `a-texture-of-another-shape` points the row at it.
/// - `blank` is byte-for-byte the pinned client's own
///   `block/redstone_dust_overlay.png`, a fully transparent 16×16 grey-alpha
///   image — `a-texture-that-changes-nothing` points a row at it.
fn write_textures(out: &Path) {
    std::fs::create_dir_all(out).unwrap_or_else(|e| panic!("mkdir {}: {e}", out.display()));
    for (id, bytes) in [
        ("hall-stone", flat_png(16, 16, [0xB0, 0x30, 0x6A])),
        ("wrong-shape", flat_png(24, 24, [0xB0, 0x30, 0x6A])),
        ("blank", blank_png()),
        ("drowned-wrap", drowned_sheet(DrownedSheet::BasePositions)),
        (
            "drowned-player-layout",
            drowned_sheet(DrownedSheet::PlayerOverlay),
        ),
        ("drowned-crown", drowned_sheet(DrownedSheet::CrownOnly)),
    ] {
        let path = out.join(format!("{id}.png"));
        std::fs::write(&path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }
}

/// Which `minecraft:entity/zombie/drowned_outer_layer` sheet to write (spec-0097 §7).
enum DrownedSheet {
    /// `drowned-wrap`, the row the gallery ships: one flat colour on the torso
    /// and limb faces of the outer model's own boxes, which sit at the BASE
    /// positions. The head and its hat are left clear, so the drowned's face
    /// reads through.
    BasePositions,
    /// `drowned-player-layout`, for `a-texture-drawn-to-another-model`: the
    /// same colour on the player's jacket, sleeve and pants positions — the
    /// layout The Stranding's sheet was drawn to, which the outer model never
    /// samples.
    PlayerOverlay,
    /// `drowned-crown`, for `a-texture-painted-on-the-crown`: the colour on the
    /// outer hat's top face only — sampled, and seen by nobody standing level
    /// with the mob.
    CrownOnly,
}

fn drowned_sheet(which: DrownedSheet) -> Vec<u8> {
    let part = |c: &serde_json::Value| c["part"].as_str().unwrap_or("").to_string();
    let faces = match which {
        DrownedSheet::BasePositions => model_faces("drowned_outer_layer", |c| {
            let p = part(c);
            p != "head" && p != "head/hat"
        }),
        DrownedSheet::PlayerOverlay => {
            model_faces("player", |c| !is_base(c) && part(c) != "head/hat")
        }
        DrownedSheet::CrownOnly => model_faces("drowned_outer_layer", |c| part(c) == "head/hat")
            .into_iter()
            .filter(|f| f.0 == "up")
            .collect(),
    };
    rgba_png(64, 64, |x, y| {
        let a = if on_faces(&faces, x, y) { 0xFF } else { 0x00 };
        [0x3E, 0x6B, 0x5A, a]
    })
}

/// A `w`×`h` opaque RGB image of one colour.
fn flat_png(w: u32, h: u32, c: [u8; 3]) -> Vec<u8> {
    // Each scanline is filter type 0 (None) followed by its pixels.
    let row: Vec<u8> = std::iter::once(0).chain((0..w).flat_map(|_| c)).collect();
    let raw: Vec<u8> = (0..h).flat_map(|_| row.iter().copied()).collect();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), Compression::new(6));
    z.write_all(&raw).expect("zlib write");
    let idat = z.finish().expect("zlib finish");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, RGB, deflate, no filter, no interlace
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&png_chunk(b"IDAT", &idat));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

/// A fully transparent 16×16 8-bit grey-alpha image, encoded exactly as the
/// pinned client encodes its own `block/redstone_dust_overlay.png` (census
/// sha256 `d9ec0ce5…7759`): the IDAT is that file's 14-byte zlib stream, which
/// encodes 528 zero bytes — sixteen filter bytes and 256 transparent pixels —
/// and nothing else. A stream of zeros carries no expression (ADR-0013); it is
/// spelled out because no compressor this repository runs emits that exact
/// stream (zlib levels 0–9 all differ), and the probe needs vanilla's bytes.
fn blank_png() -> Vec<u8> {
    const IDAT: [u8; 14] = [
        0x78, 0xDA, 0x63, 0x18, 0x05, 0xA3, 0x00, 0x09, 0x00, 0x00, 0x02, 0x10, 0x00, 0x01,
    ];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&16u32.to_be_bytes());
    ihdr.extend_from_slice(&16u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 4, 0, 0, 0]); // 8-bit, grey-alpha, deflate, no filter, no interlace
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&png_chunk(b"IDAT", &IDAT));
    png.extend_from_slice(&png_chunk(b"IEND", &[]));
    png
}

fn write_skins(out: &Path) {
    std::fs::create_dir_all(out).unwrap_or_else(|e| panic!("mkdir {}: {e}", out.display()));
    for (id, base, belt, model) in SKINS {
        let path = out.join(format!("{id}.png"));
        std::fs::write(&path, skin_png(base, belt, model))
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }
    // `a-skin-painted-off-its-boxes` (spec-0097 §7): the curator's skin with one
    // more opaque pixel in the head unwrap's top-left corner, which no box of
    // either player model samples.
    let (_, base, belt, model) = SKINS[0];
    let faces = model_faces(model, is_base);
    let stray = rgba_png(64, 64, |x, y| {
        let c = if (20..24).contains(&y) { belt } else { base };
        let a = if on_faces(&faces, x, y) || (x, y) == (0, 0) {
            0xFF
        } else {
            0x00
        };
        [c[0], c[1], c[2], a]
    });
    let path = out.join("curator-stray.png");
    std::fs::write(&path, stray).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("wrote {}", path.display());
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
/// consequence of it rather than a choice (spec-0098 §4).
///
/// The site-plan overlay gives that place an 8×8 footprint at the `alcove`
/// rung with no ceiling, on an `open` site whose terrain stands one to four
/// courses under its floor. A place owns its outside, so its frame is its
/// claim: the ring its edge may stand in on the three sides it does not share,
/// and the ground under its plot down to the lowest terrain around it. The far
/// hall owns the party plane on the north (the arch's connection names it
/// first, and it is roofed where the yard is open), so the frame stops at the
/// play space there. `delvec allocation node/exit` hands exactly 10×8×9 with
/// the walk plane at local y=5, and `DW0843` refuses a cell either way.
const YARD_SIZE: [i32; 3] = [10, 8, 9];

/// The walk plane's local `y` — `datum_y` in the handout.
const YARD_FLOOR: i32 = 5;

/// **The ring's fixed ground inside the frame** — the handout's voids, every
/// one owned by the whole: the terrain continued to the plot's edge. The piece
/// holds `structure_void` there and writes no block (`DW0990`).
const YARD_VOIDS: [([i32; 3], [i32; 3]); 6] = [
    ([0, 0, 0], [0, 1, 8]),
    ([0, 2, 0], [0, 2, 0]),
    ([1, 0, 8], [9, 0, 8]),
    ([9, 0, 0], [9, 1, 7]),
    ([9, 1, 8], [9, 1, 8]),
    ([9, 2, 0], [9, 2, 0]),
];

/// The way in, in piece-local cells: the seam the plan cut in the far hall's
/// south wall, answered on the yard's own first row beside it, as
/// `delvec allocation` states it. The piece must leave exactly these cells
/// passable and must not open a second way out — the first is `DW0844` from one
/// direction and the second is `DW0844` from the other.
const YARD_WAY: ([i32; 3], [i32; 3]) = ([3, 5, 0], [4, 7, 0]);

/// Where a body stands when a quest seats it here — `anchor/node-exit` after the
/// re-binding. Open paving, clear of the plinth.
const YARD_SEAT: [i32; 3] = [2, 5, 4];

fn yard_void(p: [i32; 3]) -> bool {
    YARD_VOIDS
        .iter()
        .any(|(lo, hi)| (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i]))
}

/// A courtyard on a plinth: paving, a low stone to walk around, four corner
/// posts — and its own outside. The plinth is faced in stone down to the
/// terrain on the three sides it does not share, the ring above the ground is
/// left open so a body on the yard sees sky and the far hall's wall, and the
/// ring's ground is the whole's.
fn build_yard() -> Structure {
    let mut palette = Palette::new();
    let mut blocks = Vec::new();
    let [sx, sy, sz] = YARD_SIZE;
    let foot = |x: i32, z: i32| (1..=sx - 2).contains(&x) && (0..=sz - 2).contains(&z);
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let name = if yard_void([x, y, z]) {
                    "minecraft:structure_void"
                } else if !foot(x, z) {
                    // The ring above the ground, on the yard's free sides: open.
                    "minecraft:air"
                } else if y < YARD_FLOOR - 1 {
                    // The plinth, faced down to the terrain.
                    "minecraft:stone_bricks"
                } else if y == YARD_FLOOR - 1 {
                    "minecraft:polished_andesite"
                } else if y == YARD_FLOOR && (4..=5).contains(&x) && (3..=4).contains(&z) {
                    "minecraft:chiseled_stone_bricks"
                } else if (YARD_FLOOR..=YARD_FLOOR + 1).contains(&y)
                    && (x == 1 || x == sx - 2)
                    && (z == 0 || z == sz - 2)
                {
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
        // The yard owns its outside (spec-0098): it stands as a plinth over
        // terrain that falls away to the east, south and west, and the faces
        // of that plinth are what a body on the ground around it looks at. The
        // north side is the far hall's wall, and the underside sits on the
        // ground.
        //
        // The list is exact, never a blanket: a side declared here that the
        // world has in fact buried is refused by the same code, so padding it
        // out to six reds rather than passing. That is what makes this a bound
        // declaration and not a hatch — perturb it either way and the gallery
        // goes red, which is what `gallery/probes/a-face-nothing-stands-in-front-of`
        // pins.
        "shown_faces": ["east", "south", "west"],
        "spatial_contract": {
            "entry": "yard",
            "spaces": {
                "yard": {
                    "envelope": "open_top",
                    "boxes": [region([1, YARD_FLOOR, 0], [YARD_SIZE[0] - 2, YARD_SIZE[1] - 1, YARD_SIZE[2] - 2])]
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
        .filter(|(_, (name, _))| {
            name.as_str() != "minecraft:air" && name.as_str() != "minecraft:structure_void"
        })
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
// The BANK: a SITE — one box holding a building and the ground it stands on
// ---------------------------------------------------------------------------

/// The site piece's id.
///
/// # What a site is, and why the gallery owes one
///
/// Every other gallery piece is a *building*: it is the inside of something,
/// and everything outside its box belongs to the horizon. A **site** is the
/// other shape — one box holding the building together with its own ground, its
/// bank running out to the box's own edge, and mass under that bank. It is what
/// a whole-map zone exported by `delvec grammar expand` is, and until a
/// one-area campaign could state its extent it had nowhere to stand: no base
/// that builds terrain would take it.
///
/// Two things are only true of a site, and this piece is where the engine is
/// asked both:
///
/// * **It is seated by its walk plane.** Its bank's top course has to be the
///   ground outside it, or the party walks up to a cliff it cannot climb. A
///   piece whose walk plane is its own floor course could not tell whether that
///   was being done; this one carries three courses of mass under its bank, so
///   the number moves it.
/// * **Its outward faces are partly buried and partly seen, in one piece.**
///   The courses under the bank stand in the valley's own ground; the parapet
///   stands above it and the document answers for it. Both halves of `DW0885`
///   bind here, exactly as they do on the quay — where the burying is done by
///   water instead of by earth.
const BANK_ID: &str = "gallery-bank";

/// Extent: 24 × 12 × 24.
///
/// Wide enough that a body walks a real distance across the bank before it
/// reaches the parapet, small enough that the valley around it builds in
/// seconds.
const BANK_SIZE: [i32; 3] = [24, 12, 24];

/// The local y of the bank's top solid course — the ground a body stands on.
///
/// Three courses of mass sit under it (`0..=2`), and that mass is the half of
/// the site a horizon has to bury. Seated on a `valley` the origin is
/// `VALLEY_WALK_REF_Y - walk_y`, so this course lands exactly on the gap
/// floor's own top course and the two grounds are one ground.
const BANK_GRADE_Y: i32 = 3;

/// How many courses of parapet stand above the bank, on the box's outer ring.
const BANK_PARAPET: i32 = 3;

/// The x range of the way in, cut through the parapet on the north face.
const BANK_GATE_X: std::ops::RangeInclusive<i32> = 10..=13;

/// A site: three courses of island mass, a bank across the whole footprint, and
/// a parapet on the box's own edge with one way through it.
///
/// The parapet is what makes the piece answerable, for the quay's reason: a
/// bank with nothing above its grade course would put every solid boundary cell
/// at or below the ground outside, the valley would bury all of them, and the
/// binding would prove nothing about `shown_faces`. What is wanted is both.
fn build_bank() -> Structure {
    let mut palette = Palette::new();
    let mut blocks = Vec::new();
    let [sx, sy, sz] = BANK_SIZE;
    let parapet_top = BANK_GRADE_Y + BANK_PARAPET;
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let ring = x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                let gate = z == 0 && BANK_GATE_X.contains(&x);
                let above_grade = y > BANK_GRADE_Y && y <= parapet_top;
                // A lamp in the parapet, so the court is lit by something the
                // piece carries rather than by the sky alone: `DW0210` measures
                // under the DARKEST reachable sky, and a campaign is free to
                // declare one this court would not survive on daylight.
                let lamp = above_grade
                    && y == BANK_GRADE_Y + 2
                    && matches!(
                        (x, z),
                        (0, 0) | (0, 23) | (23, 0) | (23, 23) | (0, 11) | (23, 11) | (11, 23)
                    );
                let name = if y < BANK_GRADE_Y {
                    "minecraft:stone"
                } else if y == BANK_GRADE_Y {
                    "minecraft:grass_block"
                } else if lamp {
                    "minecraft:sea_lantern"
                } else if above_grade && ring && !gate {
                    "minecraft:cobblestone"
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
        size: BANK_SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

/// The site's document.
///
/// `shown_faces` names the four sides and nothing else, and the exactness is
/// the demonstration. `down` is the island's underside, which stands in the
/// valley's own ground; `up` is open sky over a court with no solid cell on the
/// box's top plane, so there is no side there to show. `DW0885` refuses a
/// declared side the world buried and a declared side of pure air alike, so
/// padding this list out to six reds — which is what makes these four a bound
/// declaration rather than a hatch.
fn bank_metadata() -> serde_json::Value {
    serde_json::json!({
        "prefab_id": format!("prefab/{BANK_ID}"),
        "structure": {
            "file": format!("{BANK_ID}.nbt"),
            "id": BANK_ID,
            "size": BANK_SIZE,
            "data_version": DATA_VERSION,
            "generator": "prefabs/gallery-generator (gallery-prefab-gen)"
        },
        "anchors": {
            "anchor/bank-arrival": {
                "pos": [11, BANK_GRADE_Y + 1, 2],
                "facing": "south",
                "role": "entry",
                "note": "just inside the way through the parapet — the cell a body arrives at"
            },
            "anchor/bank-court": {
                "pos": [11, BANK_GRADE_Y + 1, 11],
                "facing": "north",
                "note": "the middle of the court, where the warden of the bank stands"
            },
            "anchor/bank-corner": {
                "pos": [20, BANK_GRADE_Y + 1, 20],
                "facing": "north",
                "note": "the far corner of the bank, inside the parapet"
            }
        },
        "shown_faces": ["east", "north", "south", "west"],
        "lighting": {
            "profile": "lit",
            "measured_min_light": 15,
            "measured": "2026-09-10",
            "method": "derived: seven sea lanterns set in the parapet, over a court open to the sky"
        },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Original Delvewright project asset (pipeline-code license per prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            "provenance": "Generated deterministically by prefabs/gallery-generator (ADR-0006)."
        }
    })
}

fn write_bank(out: &Path) {
    let s = build_bank();
    let cells = invariant_cells(&s);
    invariants::assert_blocks_are_real(BANK_ID, &cells);
    connections::assert_shape_is_stated(BANK_ID, &cells);

    let nbt = fastnbt::to_bytes(&s).expect("structure serializes to NBT");
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::new(6));
    gz.write_all(&nbt).expect("gzip write");
    let framed = gz.finish().expect("gzip finish");
    std::fs::write(out.join(format!("{BANK_ID}.nbt")), &framed).expect("write bank nbt");
    let mut meta = bank_metadata();
    declare_walk_y(BANK_ID, &s, &mut meta);
    // The measured plane and the designed grade are one number or this piece is
    // not the site it says it is: a body stands one course above the bank, and
    // everything the seating rule derives is that number under the horizon's.
    assert_eq!(
        meta["walk_y"],
        serde_json::json!(BANK_GRADE_Y + 1),
        "{BANK_ID}: the measured walk plane must be the course above the bank"
    );
    document::write_preserving(&out.join(format!("{BANK_ID}.json")), &meta);
    println!(
        "{BANK_ID}: site piece written — {}x{}x{}, walk plane at local y={}, \
         {BANK_GRADE_Y} course(s) of mass under the bank, {BANK_PARAPET} of parapet above it",
        BANK_SIZE[0], BANK_SIZE[1], BANK_SIZE[2], meta["walk_y"],
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

// ---------------------------------------------------------------------------
// The long gallery (spec-0086): a corridor of identical bays that never ends
// ---------------------------------------------------------------------------

/// The long gallery's extent in its own frame, `(width, height, length)`: 7
/// across — the west wall, the passage 1..=3, the east wall with a glass window
/// in every bay, a sealed cavity strip behind the windows, the outer wall — 5
/// high (its floor is the hall's roof, the passage three courses, its own roof),
/// and 27 long. Its frame is laid on the hall's roof by [`long_gallery_frame`].
const CORRIDOR_SIZE: [i32; 3] = [7, 5, 27];

/// How many courses one bay repeats over, along the gallery's length.
const CORRIDOR_PERIOD: i32 = 6;

/// The first of the three bays starts four courses in; the porch is 1..=3.
const CORRIDOR_FIRST_BAY: i32 = 4;

/// How many identical bays the gallery holds.
const CORRIDOR_BAYS: i32 = 3;

/// Bay `k`'s mouth, in the gallery's own frame.
const fn corridor_bay(k: i32) -> i32 {
    CORRIDOR_FIRST_BAY + CORRIDOR_PERIOD * k
}

/// The hall's `z` the stair climbs along, against the far wall.
const LONG_GALLERY_STAIR_Z: i32 = SIZE[2] - 3;

/// A hall cell `(x, y, z)` in the gallery's own frame `(u, v, w)`: across the
/// hall's east strip from its outer wall inward, up from the roof, and along
/// the hall's length from the far wall toward the near one — so the porch
/// stands over the stair and the gallery runs back toward the stall.
fn long_gallery_frame(x: i32, y: i32, z: i32) -> Option<[i32; 3]> {
    let (u, v, w) = ((SIZE[0] - 1) - x, y - HALL_ROOF_Y, (SIZE[2] - 1) - z);
    let inside = (0..CORRIDOR_SIZE[0]).contains(&u)
        && (0..CORRIDOR_SIZE[1]).contains(&v)
        && (0..CORRIDOR_SIZE[2]).contains(&w);
    inside.then_some([u, v, w])
}

/// The hall cell of a gallery-frame cell — the inverse of [`long_gallery_frame`].
const fn long_gallery_cell(u: i32, v: i32, w: i32) -> [i32; 3] {
    [(SIZE[0] - 1) - u, v + HALL_ROOF_Y, (SIZE[2] - 1) - w]
}

/// The cells cut through the hall's roof and the gallery's east wall so the
/// stair's top courses have headroom and step into the porch: everything over
/// the stair's three highest treads up to the passage's head height.
fn in_stair_shaft(x: i32, y: i32, z: i32) -> bool {
    z == LONG_GALLERY_STAIR_Z
        && (24..=26).contains(&x)
        && (HALL_ROOF_Y..=HALL_ROOF_Y + 2).contains(&y)
        && !(x == 24 && y > HALL_ROOF_Y)
}

/// What stands at a hall cell the long gallery owns, or `None` off it.
///
/// Each bay is the same six courses: its mouth open (where the loop's slab
/// stands), a lantern hung from the roof, a baffle across the passage's two
/// western cells, two open courses with a window east, and a baffle across the
/// two eastern cells. The two baffles stagger, so a line of sight down the
/// passage closes inside one bay — the jog the loop's seamlessness proof asks
/// for — and a body walks it as a zigzag. The end room past bay 2 opens exactly
/// as a bay 3 would, with its lantern where bay 3's would hang, so the light a
/// body sees from the slab is the light it sees from the landing.
/// A block id and its optional block-state properties.
type BlockWithState = (
    &'static str,
    Option<&'static [(&'static str, &'static str)]>,
);

fn long_gallery_at(x: i32, y: i32, z: i32) -> Option<BlockWithState> {
    if in_stair_shaft(x, y, z) {
        return Some(("minecraft:air", None));
    }
    let [u, v, w] = long_gallery_frame(x, y, z)?;
    let [su, sv, sw] = CORRIDOR_SIZE;
    const LANTERN: Option<&[(&str, &str)]> = Some(&[("hanging", "true")]);
    let shell = v == 0 || v == sv - 1 || w == 0 || w == sw - 1 || u == 0 || u == su - 1;
    if shell {
        return Some(("minecraft:stone", None));
    }
    let in_bays = (corridor_bay(0)..corridor_bay(CORRIDOR_BAYS)).contains(&w);
    let o = (w - CORRIDOR_FIRST_BAY).rem_euclid(CORRIDOR_PERIOD);
    if u == 4 {
        if in_bays && o == 3 && v == 2 {
            return Some(("minecraft:glass", None));
        }
        return Some(("minecraft:stone", None));
    }
    if u == 5 {
        return Some(if in_bays {
            ("minecraft:air", None)
        } else {
            ("minecraft:stone", None)
        });
    }
    if in_bays {
        let baffle = (o == 2 && (u == 1 || u == 2)) || (o == 5 && (u == 2 || u == 3));
        if baffle {
            return Some(("minecraft:stone", None));
        }
        if o == 1 && u == 2 && v == sv - 2 {
            return Some(("minecraft:lantern", LANTERN));
        }
    }
    if w == corridor_bay(CORRIDOR_BAYS) + 1 && u == 2 && v == sv - 2 {
        return Some(("minecraft:lantern", LANTERN));
    }
    // The chest at the far end: what the end room holds, and what a body
    // arriving there looks at.
    if w == sw - 2 && u == 2 && v == 1 {
        return Some(("minecraft:barrel", None));
    }
    Some(("minecraft:air", None))
}

/// **The anchors that centre a volume rather than stand a body** — the loop's
/// slab and its landing, each at the passage's mid-height so `± [1, 1, 0]` is
/// exactly the open cross-section. They hang in air by design, so they are held
/// to air and to an open cross-section, not to a floor.
const VOLUME_ANCHORS: &[Anchor] = &[
    Anchor {
        name: "anchor/long-gallery-slab",
        pos: long_gallery_cell(2, 2, corridor_bay(2)),
        facing: None,
        trigger_block: None,
        note: "the mouth of the long gallery's bay 2, mid-height: the loop's slab is this cell \
               ± [1, 1, 0], exactly the passage's open cross-section",
        role: None,
    },
    Anchor {
        name: "anchor/long-gallery-landing",
        pos: long_gallery_cell(2, 2, corridor_bay(1)),
        facing: None,
        trigger_block: None,
        note: "the mouth of the long gallery's bay 1, one bay back toward the porch: where a body \
               crossing the slab is put down",
        role: None,
    },
    Anchor {
        name: "anchor/long-gallery-lamp-0",
        pos: long_gallery_cell(1, 3, corridor_bay(0) + 3),
        facing: None,
        trigger_block: None,
        note:
            "under the long gallery's roof in bay 0: where the first crossing hangs a second lamp",
        role: None,
    },
    Anchor {
        name: "anchor/long-gallery-lamp-1",
        pos: long_gallery_cell(1, 3, corridor_bay(1) + 3),
        facing: None,
        trigger_block: None,
        note: "the same cell of bay 1",
        role: None,
    },
    Anchor {
        name: "anchor/long-gallery-lamp-2",
        pos: long_gallery_cell(1, 3, corridor_bay(2) + 3),
        facing: None,
        trigger_block: None,
        note: "the same cell of bay 2",
        role: None,
    },
    Anchor {
        name: "anchor/long-gallery-lamp-3",
        pos: long_gallery_cell(1, 3, corridor_bay(3) + 3),
        facing: None,
        trigger_block: None,
        note: "the same cell of the end room, where a bay 3 would hang it",
        role: None,
    },
];

// ---------------------------------------------------------------------------
// The LONG HALL: a loop whose far end stays in view (spec-0090)
// ---------------------------------------------------------------------------

/// The long hall's id.
///
/// # What it is for
///
/// The gallery's long gallery on the hall roof closes its view inside one bay,
/// so its loop sees no far field, and the far-field rule binds nothing there.
/// This piece is the other shape: one straight hall of identical 6-block bays,
/// no fog, its exit lit and in plain view down the whole hall. The loop
/// `loop/the-hall-that-runs-on` stands across one bay's mouth and returns a
/// crossing body one bay back, so the exit stays where it was. What the
/// gallery shows with it is the far field admitted — cells that differ from
/// their images, judged by how far the jump moves them on screen — and its
/// binding line states the count and the largest shift against the threshold.
///
/// The hall is the spike's station 4 at a 6-block jump. Its numbers are what
/// spec-0090 §5.3 tabulates for that jump: an exit 54 blocks past the slab,
/// lit, shifts 0.7574°, under the 1.2852° threshold, where a 6-block jump's
/// near range is 10.6 blocks. The approach behind the landing is as deep as
/// the hall ahead of the slab, because the rule judges every direction.
const LONG_HALL_ID: &str = "gallery-long-hall";

/// The slab's course, along z, piece-local: 66 courses past the hall's rear
/// mouth, so the mouth stands 60 behind the landing.
const LONG_HALL_SLAB_Z: i32 = 74;

/// The jump: one bay.
const LONG_HALL_JUMP: i32 = 6;

/// The exit's course: the end room's front wall, 54 past the slab.
const LONG_HALL_END_Z: i32 = LONG_HALL_SLAB_Z + 54;

/// The hall's rear mouth, where the porch opens onto it.
const LONG_HALL_REAR_Z: i32 = 8;

/// The end room's depth past its front wall.
const LONG_HALL_ROOM_DEPTH: i32 = 10;

/// Extent: the end room's width, its height, and porch to end room.
const LONG_HALL_SIZE: [i32; 3] = [11, 7, LONG_HALL_END_Z + LONG_HALL_ROOM_DEPTH + 1];

/// The long hall's anchors. Every one says what the gallery does with it.
const LONG_HALL_ANCHORS: &[Anchor] = &[
    Anchor {
        name: "anchor/long-hall-porch",
        pos: [5, 1, 4],
        facing: Some("south"),
        trigger_block: None,
        note: "where the party arrives in the long hall: the porch, the hall's mouth ahead",
        role: Some("entry"),
    },
    Anchor {
        name: "anchor/long-hall-slab",
        pos: [5, 2, LONG_HALL_SLAB_Z],
        facing: None,
        trigger_block: None,
        note: "the centre of the loop's slab: a bay's mouth, the passage's whole cross-section \
               at ± [1, 1, 0]",
        role: None,
    },
    Anchor {
        name: "anchor/long-hall-landing",
        pos: [5, 2, LONG_HALL_SLAB_Z - LONG_HALL_JUMP],
        facing: None,
        trigger_block: None,
        note: "where the slab's centre lands: the mouth of the bay before it",
        role: None,
    },
    Anchor {
        name: "anchor/long-hall-end",
        pos: [5, 1, LONG_HALL_END_Z + 5],
        facing: None,
        trigger_block: None,
        note: "the middle of the lit end room: the objective the party reaches once the loop \
               lets it go",
        role: None,
    },
    Anchor {
        name: "anchor/long-hall-chest",
        pos: [5, 2, LONG_HALL_END_Z + LONG_HALL_ROOM_DEPTH - 1],
        facing: None,
        trigger_block: None,
        note: "the top of the chest against the end room's far wall: what the end room holds, \
               in front of the eye that arrives at the end",
        role: None,
    },
    Anchor {
        name: "anchor/long-hall-down-the-hall",
        pos: [6, 1, LONG_HALL_SLAB_Z + 16],
        facing: Some("north"),
        trigger_block: None,
        note: "a cell 16 courses past the slab, past the near field: where a probe posts a \
               figure the jump moves too far",
        role: None,
    },
];

fn build_long_hall() -> Structure {
    let mut cells: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    let mut fill = |a: [i32; 3], b: [i32; 3], block: &'static str| {
        for x in a[0]..=b[0] {
            for y in a[1]..=b[1] {
                for z in a[2]..=b[2] {
                    if block == "minecraft:air" {
                        cells.remove(&[x, y, z]);
                    } else {
                        cells.insert([x, y, z], block);
                    }
                }
            }
        }
    };
    let (rear, end) = (LONG_HALL_REAR_Z, LONG_HALL_END_Z);
    let room_end = end + LONG_HALL_ROOM_DEPTH;
    // The porch: a lit stone-brick room the hall's rear mouth opens from.
    fill([1, 0, 0], [9, 0, rear], "minecraft:polished_deepslate");
    fill([1, 1, 0], [9, 5, rear], "minecraft:stone_bricks");
    fill([2, 1, 1], [8, 4, rear - 1], "minecraft:air");
    fill([3, 4, 3], [3, 4, 3], "minecraft:lantern[hanging=true]");
    fill([7, 4, 3], [7, 4, 3], "minecraft:lantern[hanging=true]");
    // The hall: station 4's cross-section.
    fill([3, 0, rear], [7, 0, end], "minecraft:polished_deepslate");
    fill([3, 1, rear], [7, 4, end], "minecraft:stone_bricks");
    fill([4, 1, rear], [6, 3, end], "minecraft:air");
    fill([4, 0, rear], [6, 0, end], "minecraft:dark_oak_planks");
    fill([5, 1, rear], [5, 1, end], "minecraft:red_carpet");
    // A pillar pair and a hanging soul lantern every bay, the lamp two courses
    // past each bay's mouth, up to a full bay short of the exit.
    for z in (rear + 1)..=(end - LONG_HALL_JUMP) {
        if (z - (LONG_HALL_SLAB_Z + 2)).rem_euclid(LONG_HALL_JUMP) == 0 {
            fill([3, 1, z], [3, 3, z], "minecraft:polished_deepslate");
            fill([7, 1, z], [7, 3, z], "minecraft:polished_deepslate");
            fill([5, 3, z], [5, 3, z], "minecraft:soul_lantern[hanging=true]");
        }
    }
    // The end room, and the lit exit into it: a glowstone lintel over the
    // doorway, seen down the whole hall.
    fill([0, 0, end], [10, 6, room_end], "minecraft:deepslate_bricks");
    fill([1, 1, end + 1], [9, 5, room_end - 1], "minecraft:air");
    fill([4, 1, end], [6, 3, end], "minecraft:air");
    fill([4, 4, end], [6, 4, end], "minecraft:glowstone");
    for (x, z) in [
        (2, end + 3),
        (8, end + 3),
        (2, room_end - 3),
        (8, room_end - 3),
    ] {
        fill([x, 5, z], [x, 5, z], "minecraft:lantern[hanging=true]");
    }
    // What the end room holds: a chest against its far wall, on the hall's
    // axis, in front of the eye that arrives at the end.
    fill(
        [5, 1, room_end - 1],
        [5, 1, room_end - 1],
        "minecraft:chest[facing=north,type=single]",
    );
    let mut palette = Palette::new();
    let blocks = cells
        .into_iter()
        .map(|(pos, declared)| {
            let (name, props) = split_state(declared);
            BlockEntry {
                pos,
                state: palette.idx(name, (!props.is_empty()).then_some(&props[..])),
            }
        })
        .collect();
    Structure {
        data_version: DATA_VERSION,
        size: LONG_HALL_SIZE,
        palette: palette.entries,
        blocks,
        entities: Vec::new(),
    }
}

/// The long hall, cut into tiles along z at vanilla's 48-per-axis template
/// cap and declared as one piece (`structure_set`).
fn write_long_hall(out: &Path) {
    let s = build_long_hall();
    let cells = invariant_cells(&s);
    invariants::assert_blocks_are_real(LONG_HALL_ID, &cells);
    connections::assert_shape_is_stated(LONG_HALL_ID, &cells);
    invariants::assert_fluid_is_contained(LONG_HALL_ID, s.size, &cells);
    let solid: std::collections::BTreeSet<[i32; 3]> = cells
        .iter()
        .filter(|(_, (name, _))| {
            !matches!(
                name.as_str(),
                "minecraft:air"
                    | "minecraft:red_carpet"
                    | "minecraft:lantern"
                    | "minecraft:soul_lantern"
            )
        })
        .map(|(p, _)| *p)
        .collect();
    for a in LONG_HALL_ANCHORS {
        let [x, y, z] = a.pos;
        assert!(
            !solid.contains(&[x, y, z]) && !solid.contains(&[x, y + 1, z]),
            "{LONG_HALL_ID}: `{}` at {:?} is not a clear cell",
            a.name,
            a.pos
        );
    }
    let cut = 48;
    let mut parts = Vec::new();
    let mut z0 = 0;
    let mut i = 0;
    while z0 < s.size[2] {
        let depth = cut.min(s.size[2] - z0);
        let mut palette = Palette::new();
        let blocks = s
            .blocks
            .iter()
            .filter(|b| b.pos[2] >= z0 && b.pos[2] < z0 + depth)
            .map(|b| {
                let e = &s.palette[b.state as usize];
                let props: Vec<(&str, &str)> = e
                    .properties
                    .iter()
                    .flatten()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                BlockEntry {
                    pos: [b.pos[0], b.pos[1], b.pos[2] - z0],
                    state: palette.idx(&e.name, (!props.is_empty()).then_some(&props[..])),
                }
            })
            .collect();
        let tile = Structure {
            data_version: DATA_VERSION,
            size: [s.size[0], s.size[1], depth],
            palette: palette.entries,
            blocks,
            entities: Vec::new(),
        };
        let file = format!("{LONG_HALL_ID}.x0y0z{i}.nbt");
        let nbt = fastnbt::to_bytes(&tile).expect("structure serializes to NBT");
        let mut gz = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::new(6));
        gz.write_all(&nbt).expect("gzip write");
        std::fs::write(out.join(&file), gz.finish().expect("gzip finish"))
            .expect("write long hall tile");
        parts.push(serde_json::json!({
            "file": file,
            "id": format!("{LONG_HALL_ID}.x0y0z{i}"),
            "grid_index": [0, 0, i],
            "offset": [0, 0, z0],
            "size": [s.size[0], s.size[1], depth],
        }));
        z0 += depth;
        i += 1;
    }
    let mut anchors = serde_json::Map::new();
    for a in LONG_HALL_ANCHORS {
        let mut m = serde_json::Map::new();
        m.insert("pos".into(), serde_json::json!(a.pos));
        if let Some(f) = a.facing {
            m.insert("facing".into(), serde_json::json!(f));
        }
        m.insert("note".into(), serde_json::json!(a.note));
        if let Some(role) = a.role {
            m.insert("role".into(), serde_json::json!(role));
        }
        anchors.insert(a.name.into(), serde_json::Value::Object(m));
    }
    let mut meta = serde_json::json!({
        "prefab_id": format!("prefab/{LONG_HALL_ID}"),
        "structure_set": {
            "base": LONG_HALL_ID,
            "size": LONG_HALL_SIZE,
            "part_max": cut,
            "grid": [1, 1, i],
            "data_version": DATA_VERSION,
            "generator": "prefabs/gallery-generator (gallery-prefab-gen)",
            "parts": parts,
        },
        "anchors": serde_json::Value::Object(anchors),
        // Free-standing in a `void` world: every side of the box is something
        // a body that got outside would see.
        "shown_faces": ["down", "east", "north", "south", "up", "west"],
        "lighting": {
            "profile": "lit",
            "measured_min_light": 3,
            "measured": "2026-10-06",
            "method": "derived: a hanging soul lantern every 6-block bay, lanterns in the porch \
                       and the end room, a glowstone lintel over the exit"
        },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Original Delvewright project asset (pipeline-code license per \
                     prefabs/LICENSE-ASSETS.md). No third-party material ingested.",
            "provenance": "Generated deterministically by prefabs/gallery-generator (ADR-0006), \
                           after the eldritch spike's station 4."
        }
    });
    declare_walk_y(LONG_HALL_ID, &s, &mut meta);
    document::write_preserving(&out.join(format!("{LONG_HALL_ID}.json")), &meta);
    println!(
        "{LONG_HALL_ID}: long hall written — {}x{}x{} in {i} tile(s), its exit {} past the slab",
        LONG_HALL_SIZE[0],
        LONG_HALL_SIZE[1],
        LONG_HALL_SIZE[2],
        LONG_HALL_END_Z - LONG_HALL_SLAB_Z
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next() else {
        eprintln!(
            "usage: gallery-prefab-gen <out_dir> [--skins <skins_dir>] \
              [--design <design_dir>] [--textures <textures_dir>]   \
             (a BUILD directory — spec-0039 §6 commits no generated bytes)"
        );
        std::process::exit(2);
    };
    let mut skins: Option<String> = None;
    let mut design: Option<String> = None;
    let mut textures: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--skins" => skins = args.next(),
            "--design" => design = args.next(),
            "--textures" => textures = args.next(),
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
    write_bank(out);
    write_rig(out);
    write_long_hall(out);
    write_arm_rig(out);
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
    // The texture images, for the same reason again (spec-0084 §7).
    if let Some(t) = textures {
        write_textures(Path::new(&t));
    }
}

// ---------------------------------------------------------------------------
// The reaching arm's rig (spec-0094 §7)
// ---------------------------------------------------------------------------

/// The gallery's second rig: `rig/gallery-arm`, the arm that locks onto
/// whoever stands on the floor in front of it (`anchor/reach`).
///
/// Two parts — a post and a club — and two strikes at two reaches, because a
/// display entity cannot bend to a point: a lock turns the whole arm about its
/// mark and chooses the pose that comes down on the cell it locked. `near`
/// lays the club along the floor one to five cells in front of the post, `far`
/// three to eight. `windup` lifts the club back over the post, `idle` holds it
/// upright, and `rest` lays it down behind the post — the clip the story plays
/// to stand the arm down (spec-0094 §3.3).
const ARM_RIG_ID: &str = "gallery-arm";

fn write_arm_rig(out: &Path) {
    use delvewright_dsl::rig::{self, Clip, PartKind, Rig, RigPart, RigProvenance, Transform};
    let t = |translation: [f64; 3], scale: [f64; 3]| Transform {
        translation,
        left_rotation: [0.0, 0.0, 0.0, 1.0],
        scale,
        right_rotation: [0.0, 0.0, 0.0, 1.0],
    };
    let post = || t([-0.4, 0.0, -0.4], [0.8, 1.6, 0.8]);
    let pose = |club: Transform| vec![post(), club];
    let upright = |sway: f64| t([-0.25, 1.6, -0.25 + sway], [0.5, 2.5, 0.5]);
    let back = t([-0.25, 1.8, -0.9], [0.5, 2.8, 0.5]);
    // The club laid along the floor from `from` to `to` cells in front.
    let laid = |from: f64, to: f64| t([-0.5, 0.0, from], [1.0, 0.3, to - from]);
    let clip = |ticks_per_frame: u32, looping: bool, frames: Vec<Vec<Transform>>| Clip {
        ticks_per_frame,
        looping,
        frames,
    };
    let mut clips = std::collections::BTreeMap::new();
    clips.insert(
        "idle".to_string(),
        clip(10, true, vec![pose(upright(0.0)), pose(upright(0.1))]),
    );
    clips.insert(
        "windup".to_string(),
        clip(4, false, vec![pose(upright(0.0)), pose(back.clone())]),
    );
    clips.insert(
        "near".to_string(),
        clip(2, false, vec![pose(back.clone()), pose(laid(1.0, 5.0))]),
    );
    clips.insert(
        "far".to_string(),
        clip(2, false, vec![pose(back.clone()), pose(laid(3.0, 8.0))]),
    );
    clips.insert(
        "rest".to_string(),
        clip(5, false, vec![pose(t([-0.25, 0.0, -2.6], [0.5, 0.3, 2.2]))]),
    );
    let part = |id: &str, block: &str| RigPart {
        id: id.to_string(),
        kind: PartKind::Block,
        block: block.to_string(),
        rest: None,
    };
    let r = Rig {
        rig_version: rig::RIG_VERSION,
        parts: vec![
            part("post", "minecraft:polished_deepslate"),
            part("club", "minecraft:polished_blackstone"),
        ],
        clips,
        provenance: RigProvenance {
            generator: "prefabs/gallery-generator".to_string(),
            source: "original".to_string(),
            spdx: "GPL-3.0-or-later".to_string(),
        },
    };
    let issues = rig::check(&r);
    assert!(
        issues.is_empty(),
        "{ARM_RIG_ID}: the rig breaks a rig rule the engine refuses with DW0935: {issues:?}"
    );
    let dir = out.join(rig::RIGS_DIR).join(ARM_RIG_ID);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    let path = dir.join(rig::RIG_FILE);
    let mut text = serde_json::to_string_pretty(&r).expect("a rig serializes");
    text.push('\n');
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!(
        "wrote {} ({} part(s), {} clip(s))",
        path.display(),
        r.parts.len(),
        r.clips.len()
    );
}

// ---------------------------------------------------------------------------
// The sentinel's rig (spec-0082 §10)
// ---------------------------------------------------------------------------

/// The gallery's one rig: `rig/gallery-sentinel`, the assembly that stands on
/// `anchor/plinth`.
///
/// Four parts, so every clip role an assembly has is written: a base, a body,
/// a head and a stone hammer. `idle` loops a sway, `windup` lifts the hammer
/// overhead, `strike` brings it down flat on the floor round the plinth — a
/// 3 × 3 slab, the footprint the hall's landing box is declared under — and
/// `retract` sinks the whole statue below the floor. `sweep` and `hover` are
/// strikes only the probes play.
///
/// Written through `delvewright_dsl::rig::Rig`, the type the engine parses, so
/// a field this generator could misspell is a field the compiler would refuse
/// it for.
const RIG_ID: &str = "gallery-sentinel";

fn write_rig(out: &Path) {
    use delvewright_dsl::rig::{self, Clip, PartKind, Rig, RigPart, RigProvenance, Transform};
    let t = |translation: [f64; 3], scale: [f64; 3]| Transform {
        translation,
        left_rotation: [0.0, 0.0, 0.0, 1.0],
        scale,
        right_rotation: [0.0, 0.0, 0.0, 1.0],
    };
    // The three parts that do not swing, raised by `dy`.
    let figure = |dy: f64| {
        vec![
            t([-0.5, dy, -0.5], [1.0, 1.0, 1.0]),
            t([-0.45, dy + 1.0, -0.45], [0.9, 1.0, 0.9]),
            t([-0.3, dy + 2.0, -0.3], [0.6, 0.6, 0.6]),
        ]
    };
    let with_hammer = |dy: f64, hammer: Transform| {
        let mut f = figure(dy);
        f.push(hammer);
        f
    };
    // The hammer hanging at the statue's side, held overhead, and laid flat.
    let hanging = |sway: f64| t([0.55, 0.2 + sway, -0.15], [0.3, 1.4, 0.3]);
    let raised = |lift: f64| t([0.55, 1.6 + lift, -0.15], [0.3, 1.4, 0.3]);
    let mut clips = std::collections::BTreeMap::new();
    clips.insert(
        "idle".to_string(),
        Clip {
            ticks_per_frame: 10,
            looping: true,
            frames: vec![
                with_hammer(0.0, hanging(0.0)),
                with_hammer(0.0, hanging(0.1)),
            ],
        },
    );
    clips.insert(
        "windup".to_string(),
        Clip {
            ticks_per_frame: 4,
            looping: false,
            frames: vec![with_hammer(0.0, raised(0.0)), with_hammer(0.0, raised(0.4))],
        },
    );
    clips.insert(
        "strike".to_string(),
        Clip {
            ticks_per_frame: 2,
            looping: false,
            frames: vec![
                with_hammer(0.0, t([0.55, 1.0, 0.2], [0.3, 0.3, 1.4])),
                with_hammer(0.0, t([-1.5, 0.0, -1.5], [3.0, 0.3, 3.0])),
            ],
        },
    );
    clips.insert(
        "retract".to_string(),
        Clip {
            ticks_per_frame: 5,
            looping: false,
            frames: vec![with_hammer(-3.0, t([0.55, -2.8, -0.15], [0.3, 1.4, 0.3]))],
        },
    );
    // Two strikes no step of the primary plays, for the probes that refuse a
    // blow and its limb disagreeing (spec-0082 §5.4 shape 2): `sweep` lays the
    // hammer out long in front of the statue, five cells of floor under it;
    // `hover` brings the 3 x 3 slab down only to a block over the floor.
    clips.insert(
        "sweep".to_string(),
        Clip {
            ticks_per_frame: 2,
            looping: false,
            frames: vec![
                with_hammer(0.0, t([0.55, 1.0, 0.2], [0.3, 0.3, 1.4])),
                with_hammer(0.0, t([-0.5, 0.0, 0.5], [1.0, 0.3, 5.0])),
            ],
        },
    );
    clips.insert(
        "hover".to_string(),
        Clip {
            ticks_per_frame: 2,
            looping: false,
            frames: vec![
                with_hammer(0.0, t([0.55, 1.0, 0.2], [0.3, 0.3, 1.4])),
                with_hammer(0.0, t([-1.5, 1.2, -1.5], [3.0, 0.3, 3.0])),
            ],
        },
    );
    let part = |id: &str, block: &str| RigPart {
        id: id.to_string(),
        kind: PartKind::Block,
        block: block.to_string(),
        rest: None,
    };
    let r = Rig {
        rig_version: rig::RIG_VERSION,
        parts: vec![
            part("base", "minecraft:polished_deepslate"),
            part("body", "minecraft:deepslate_tiles"),
            part("head", "minecraft:chiseled_deepslate"),
            part("hammer", "minecraft:polished_blackstone"),
        ],
        clips,
        provenance: RigProvenance {
            generator: "prefabs/gallery-generator".to_string(),
            source: "original".to_string(),
            spdx: "GPL-3.0-or-later".to_string(),
        },
    };
    let issues = rig::check(&r);
    assert!(
        issues.is_empty(),
        "{RIG_ID}: the rig breaks a rig rule the engine refuses with DW0935: {issues:?}"
    );
    let dir = out.join(rig::RIGS_DIR).join(RIG_ID);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("create {}: {e}", dir.display()));
    let path = dir.join(rig::RIG_FILE);
    let mut text = serde_json::to_string_pretty(&r).expect("a rig serializes");
    text.push('\n');
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!(
        "wrote {} ({} part(s), {} clip(s))",
        path.display(),
        r.parts.len(),
        r.clips.len()
    );
}
