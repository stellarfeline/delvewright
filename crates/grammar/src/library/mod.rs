//! The rule library — ported buildings, and original staging vocabulary.
//!
//! **The ports.** [`temple`], [`castle`] and [`church`] are direct ports of the
//! example grammars shipped with `yawgmoth/GDMC25` (BSD-3-Clause — see
//! `LICENSE-GDMC25`): the Greek temple (`MakeTemple.py` / `Tetrastyle.py`,
//! Markus Eger), the castle (`MakeCastle.py`, Markus Eger) and the church
//! (`MakeChurch.py`, Janista Gitbumrungsin). spec-0027 §2 keeps them for two
//! jobs: they are the regression fixtures the interpreter is judged against, and
//! they are a few-shot corpus of grammar programs whose licence lets us use it.
//! Each port is faithful to its source's rule structure; where a rule's shape
//! changed, the module says so at the rule.
//!
//! **The staging vocabulary.** [`cliff_path`], [`watch_bay`], [`rafter_hall`],
//! [`ambush_door`] and [`store_room`] are *original* Delvewright rules — no
//! upstream, nothing ported, licence `original`. They are the W1 (path and
//! hazard geometry) and W2 (interior ambush) families of the drowned-bell
//! remake's grammar vocabulary: not buildings but *encounters*, box grammars
//! whose reason to exist is a machine gate about how the space plays.
//!
//! [`hearth_ward`], [`bait_stand`] and [`disarm_stand`] are that family's last
//! three members, and each is a **mechanism rather than a beat**: somewhere off
//! the road with one declared focus and one way in; a lure with its watcher in
//! the same frame; and a hazard's control, put where the hazard cannot reach.
//! Each module note names what a creator building an entirely different game
//! would bind to it, because that — and not the fiction the bell remake happens
//! to want — is what earns a rule a place in this library.
//!
//! They share
//! one local frame, and it is worth stating once because every derived anchor
//! facing depends on it:
//!
//! > **Local `Y` is up. Local `Z`-max is the approach end, and travel runs
//! > toward local `Z`-min.**
//!
//! That is not a coin flip. A [`Mark`]'s facing, when it is not spelled out as a
//! world direction, is *always* the direction of decreasing local `Z` — so a
//! rule can only hand an anchor a facing that points down-axis. Choosing travel
//! to run that way is what makes every anchor these rules declare look at the
//! thing it is about. The cost is that anchors number *against* travel (a split
//! visits its pieces from the low end of the axis); see [`cliff_path`]. A
//! reflection reverses both together, so the trade-off is the same in a mirrored
//! frame.
//!
//! **The idiom index.** [`idioms`] is a third kind again: one minimal program
//! per *technique* of the IR — repetition, priority, shape, erosion, graded
//! erosion, surface detail, symmetry, `skip`, light — plus
//! one composition demonstration. They build nothing anyone wants, and they are
//! in the library because `delve-grammar list` / `show` is the only way an
//! author reaches the corpus, and the corpus is where technique is learned.
//!
//! **The zone programs.** [`bell`] is the layer above: the drowned-bell
//! remake's zones, each one program that composes the vocabulary above with
//! [`crate::compose::include`] and writes no encounter geometry of its own. A
//! rule builds a shape; a zone builds a route through several of them, and its
//! gates are about what the composition did or failed to preserve.
//!
//! Every program here is parameterised: integer knobs in
//! [`Program::params`](crate::ir::Program::params) are the size/kind controls
//! and role bindings in [`Program::palette`](crate::ir::Program::palette) are
//! the style controls, so one program yields a family of models rather than one
//! building.

pub mod ambush_door;
pub mod bait_stand;
pub mod bell;
pub mod boulder_stair;
pub mod broken_grate;
pub mod castle;
pub mod causeway;
pub mod church;
pub mod cliff_path;
pub mod disarm_stand;
pub mod drop_shaft;
pub mod dumbwaiter;
pub mod elite_ground;
pub mod far_side_bar;
pub mod hearth_ward;
pub mod idioms;
pub mod lift_shaft;
pub mod negated_guard;
pub mod rafter_hall;
pub mod spatial_contract;
pub mod stair_flight;
pub mod store_room;
pub mod tee_passage;
pub mod temple;
pub mod threshold_motif;
pub mod watch_bay;

pub use ambush_door::ambush_door;
pub use bait_stand::bait_stand;
pub use bell::{
    barrow_shore, bell_tower, chapel_ward, cistern_deep, cliff_road, drowned_ward, gate_ward,
    hall_keep,
};
pub use boulder_stair::boulder_stair;
pub use broken_grate::broken_grate;
pub use castle::castle;
pub use causeway::causeway;
pub use church::church;
pub use cliff_path::cliff_path;
pub use disarm_stand::disarm_stand;
pub use drop_shaft::drop_shaft;
pub use dumbwaiter::dumbwaiter;
pub use elite_ground::elite_ground;
pub use far_side_bar::far_side_bar;
pub use hearth_ward::hearth_ward;
pub use lift_shaft::lift_shaft;
pub use rafter_hall::rafter_hall;
pub use spatial_contract::spatial_contract;
pub use stair_flight::stair_flight;
pub use store_room::store_room;
pub use tee_passage::tee_passage;
pub use temple::temple;
pub use threshold_motif::threshold_motif;
pub use watch_bay::watch_bay;

use crate::geom::Axis;
use crate::ir::Program;
use crate::ir::{
    Alternative, ArithOp, CmpOp, Cond, DimRef, Expr, Mark, MarkAt, Node, Reorient, Rounding, Side,
    Size, Split,
};

// ---------------------------------------------------------------------------
// Terse constructors, so a ported rule reads roughly like its Python original.
// ---------------------------------------------------------------------------

/// A fixed-size piece.
fn abs(n: i64) -> Size {
    Size::abs(n)
}

/// A fixed-size piece taken from a parameter.
fn absp(name: &str) -> Size {
    Size::Absolute {
        blocks: Expr::param(name),
    }
}

/// A fixed-size piece whose length is computed.
fn abse(blocks: Expr) -> Size {
    Size::Absolute { blocks }
}

/// A share of the leftover.
fn rel(weight: i64) -> Size {
    Size::rel(weight)
}

/// A split along a local axis.
fn split(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding: Rounding::Truncate,
        repeat: false,
        orient: Reorient::KEEP,
        children,
    })
}

/// A split that tiles its pattern across the axis.
fn split_repeat(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding: Rounding::Truncate,
        repeat: true,
        orient: Reorient::KEEP,
        children,
    })
}

/// A split whose relative pieces cover the axis **exactly**, the odd block
/// going to the earliest share.
///
/// [`split`] uses upstream's `Truncate`, which drops the remainder — fine for a
/// crenellation rhythm, wrong for anything load-bearing: an uncovered piece is
/// never written, and an unwritten cell is air. A floor with a one-block hole in
/// it at the far end is exactly the silent defect the machine gates exist to
/// stop, so a split that lays out ground says which it wants.
fn split_exact(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding: Rounding::Start,
        repeat: false,
        orient: Reorient::KEEP,
        children,
    })
}

/// A split that hands its children a new orientation.
fn split_oriented(axis: Axis, sizes: Vec<Size>, orient: Reorient, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding: Rounding::Truncate,
        repeat: false,
        orient,
        children,
    })
}

/// Rename the axes, then expand `body`.
fn reoriented(orient: Reorient, body: Node) -> Node {
    Node::Reorient {
        orient,
        body: Box::new(body),
    }
}

/// Expand `body` reflected across one local axis — the same rule, standing at
/// the other site of a mirror pair.
fn mirrored(axis: Axis, body: Node) -> Node {
    reoriented(Reorient::KEEP.flip(axis), body)
}

/// Expand another rule.
fn call(symbol: &str) -> Node {
    Node::call(symbol)
}

/// Fill with a palette role.
fn fill(role: &str) -> Node {
    Node::fill(role)
}

/// Fill with an inline block state — for states whose properties depend on the
/// scope's orientation, which a palette role (one state per name) cannot
/// carry. Pair it with [`oriented`] guards, one alternative per orientation,
/// each writing the facing/connections that match: that is the mechanism the
/// `oriented-fills` gate (`DW0736`) checks for.
fn fill_block(block: crate::block::BlockState) -> Node {
    Node::Fill {
        material: crate::ir::Material::block(block),
    }
}

/// The scope's frame is exactly this local-to-world mapping, **unreflected** —
/// the guard that picks the correctly oriented block-state variant.
///
/// Unreflected because a reflection lands a different facing, so a guard that
/// matched both arms of a mirror pair would license one arm's state on the
/// other. A rule that wants to stand at both sites writes the reflected
/// alternative too, with [`Cond::frame`].
fn oriented(x: Axis, y: Axis, z: Axis) -> Cond {
    Cond::orientation(x, y, z)
}

/// Write air.
fn void() -> Node {
    Node::Void
}

/// Claim this scope's box for a named contract region, then expand `body`.
fn claimed(region: &str, body: Node) -> Node {
    Node::Claim {
        region: region.to_string(),
        body: Box::new(body),
    }
}

/// Declare an anchor on this scope, then expand `body`.
fn marked(anchor: &str, at: MarkAt, body: Node) -> Node {
    Node::Mark {
        mark: Mark::new(anchor, at),
        body: Box::new(body),
    }
}

/// Declare an anchor on this scope, numbered per expansion, then expand `body`.
///
/// The rule that runs once per niche does not know how many niches there are;
/// [`crate::ir::MarkIndex::Auto`] is how it names them anyway.
fn marked_each(anchor: &str, at: MarkAt, body: Node) -> Node {
    Node::Mark {
        mark: Mark::new(anchor, at).indexed(),
        body: Box::new(body),
    }
}

/// A cell named by its offset, in **local** cells, from the scope's minimum
/// corner.
fn at_offset(x: Expr, y: Expr, z: Expr) -> MarkAt {
    MarkAt::Offset { x, y, z }
}

/// The centre of one face: the given **local** axis pinned to an end, the other
/// two centred. What a rule wants when the anchor belongs at one end of a run —
/// the inner tip of a corbel, the near end of a barrel row.
fn face(axis: Axis, side: Side) -> MarkAt {
    MarkAt::FaceCenter { axis, side }
}

/// A local dimension.
fn dim(dim: DimRef) -> Expr {
    Expr::dim(dim)
}

/// A literal.
fn int(value: i64) -> Expr {
    Expr::int(value)
}

/// A parameter.
fn par(name: &str) -> Expr {
    Expr::param(name)
}

/// `lhs % modulus == value` — the parity and divisibility guards the ported
/// grammars lean on to keep crenellations and window rhythms even.
fn modulo_is(lhs: Expr, modulus: i64, value: i64) -> Cond {
    Cond::cmp(lhs.arith(ArithOp::Rem, int(modulus)), CmpOp::Eq, int(value))
}

/// `lhs <op> rhs`.
fn cmp(lhs: Expr, op: CmpOp, rhs: Expr) -> Cond {
    Cond::cmp(lhs, op, rhs)
}

/// Every sub-guard has to hold.
fn all_of(of: Vec<Cond>) -> Cond {
    Cond::All { of }
}

/// At least one sub-guard has to hold. What a rule reaches for when it needs
/// the exact complement of an `all_of` guard, so two alternatives are a
/// decision rather than a weighted draw (`docs/reference/grammar.md` §2).
///
/// Its other shape is a clause *inside* an `all_of`, when a knob adds an
/// obligation rather than replacing one: "the knob is off, **or** the geometry
/// it needs is there" belongs in the rule's own guard, not in a second rule
/// (`causeway`'s `berm_gate`).
fn any_of(of: Vec<Cond>) -> Cond {
    Cond::Any { of }
}

/// A guarded alternative.
fn alt_when(when: Cond, body: Node) -> Alternative {
    Alternative::new(body).when(when)
}

/// An unguarded alternative with an explicit selection weight — a taste
/// distribution rather than a decision (see the note on selection in
/// `docs/reference/grammar.md` §2).
fn alt_weight(weight: u32, body: Node) -> Alternative {
    Alternative::new(body).weight(weight)
}

/// The fallback alternative.
fn alt_else(body: Node) -> Alternative {
    Alternative::new(body).when(Cond::Otherwise)
}

/// One library entry: its stable id, the function that builds it, **and the
/// expansion it is judged at**.
///
/// The last part is why this is a struct rather than the `(id, build)` pair it
/// was. A program is region-polymorphic — that is the point of a grammar — so
/// "which region" is not a property of the program. It is a property of the
/// *entry*: the corpus demonstrates this piece at this size, and that is the
/// expansion any sweep over the corpus must judge. While it lived in prose
/// (`grammar.md` §5) and in three hand-written tables in `tests/`, no sweep
/// could be driven from the registry, so every sweep enumerated its own subset
/// — 22 of 33 in `tests/library.rs`, 10 of 33 in `tests/idioms.rs`, and
/// `negated-guard` in neither. Carrying it here makes the omission
/// unexpressible: a new program cannot reach `PROGRAMS` without saying where it
/// is judged, and the sweeps iterate `PROGRAMS`.
pub struct LibraryProgram {
    /// The stable id `delve-grammar list` prints and `--program` takes.
    pub id: &'static str,
    /// Build the program.
    pub build: fn() -> Program,
    /// The region the corpus demonstrates it at, `[X, Y, Z]`.
    pub region: [u32; 3],
    /// The seed it is demonstrated at.
    pub seed: u64,
    /// Which optional gates the entry CLAIMS. `traversable` is a claim that the
    /// piece is a route, not a licence to skip a check: a piece that is not a
    /// route has no approach and exit face to join, and asserting one would
    /// bind the gate to a fiction. The claim is stated per entry rather than
    /// defaulted so that adding a program is a decision about what it is.
    pub gates: crate::gates::Options,
    /// Which corpus this belongs to.
    pub kind: Kind,
}

/// What a library entry IS — a thing to build with, or a thing to learn from.
///
/// Carried on the entry rather than inferred from the id. The `idiom-` prefix
/// looked like the discriminator and is not one: `negated-guard` is a language
/// example that carries no prefix, and a sweep keyed on the prefix asserted
/// "that is a cube, not a building" against a program whose whole job is to
/// fill its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A piece of the building vocabulary: a zone composes these.
    Piece,
    /// A minimal example of one IR construct or technique. It demonstrates;
    /// it does not have to look like anything.
    Example,
}

/// The entry claims no optional gate: not a route, roofs nothing, has no
/// mirror plane. Commonest.
const NO_CLAIM: crate::gates::Options = crate::gates::Options {
    traversable: false,
    allow_falls: false,
    symmetric: None,
    reachable_floor: false,
};

/// A piece a body walks end to end on the level.
const ROUTE: crate::gates::Options = crate::gates::Options {
    traversable: true,
    allow_falls: false,
    symmetric: None,
    reachable_floor: false,
};

/// A piece built as one rule standing at both sites of a mirror plane on the
/// world `Y`.
///
/// Claimed by the entry rather than left to a `--symmetric` a person remembers
/// to pass: the registry is what the corpus sweep and `audit` read, so a claim
/// that lives only on a command line is a gate the corpus never runs. The
/// bilateral-symmetry gate would otherwise bind ZERO over the whole library —
/// green because nothing asked it anything.
const MIRRORED_Y: crate::gates::Options = crate::gates::Options {
    traversable: false,
    allow_falls: false,
    symmetric: Some(Axis::Y),
    reachable_floor: false,
};

const fn entry(
    id: &'static str,
    build: fn() -> Program,
    region: [u32; 3],
    seed: u64,
    kind: Kind,
    gates: crate::gates::Options,
) -> LibraryProgram {
    LibraryProgram {
        id,
        build,
        region,
        seed,
        gates,
        kind,
    }
}

/// **Every program in this library, by id, with the expansion it is judged at.**
///
/// A registry rather than a `match`, for the reason a tool exists at all: a
/// creator has to be able to *discover* what the back end can build without
/// reading Rust. `delve-grammar list` enumerates this, so a rule added to the
/// library reaches the tool without the tool being edited.
///
/// The `bell::` zone programs are deliberately absent: a zone is one campaign's
/// composition of these, not a piece of the general vocabulary, and listing a
/// campaign's own material as if it were library surface is the "authored
/// content wearing a primitive's clothes" shape CLAUDE.md names. A campaign's
/// zones declare their own expansions, in the campaign, at
/// `design/programs/zones.toml`.
pub const PROGRAMS: &[LibraryProgram] = &[
    entry(
        "ambush-door",
        ambush_door,
        [11, 5, 13],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "bait-stand",
        bait_stand,
        [9, 8, 14],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "boulder-stair",
        boulder_stair,
        [9, 6, 27],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "broken-grate",
        broken_grate,
        [3, 5, 14],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry("castle", castle, [41, 14, 25], 1, Kind::Piece, NO_CLAIM),
    entry("causeway", causeway, [7, 10, 9], 1, Kind::Piece, NO_CLAIM),
    entry("church", church, [15, 16, 30], 1, Kind::Piece, NO_CLAIM),
    entry(
        "cliff-path",
        cliff_path,
        [3, 6, 30],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "disarm-stand",
        disarm_stand,
        [9, 7, 16],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "drop-shaft",
        drop_shaft,
        [4, 8, 6],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "dumbwaiter",
        dumbwaiter,
        [6, 8, 8],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "elite-ground",
        elite_ground,
        [19, 5, 25],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "far-side-bar",
        far_side_bar,
        [5, 5, 7],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "hearth-ward",
        hearth_ward,
        [8, 6, 14],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    // The idiom index (`idioms`): one minimal program per technique, plus one
    // composition demonstration. They are here because `delve-grammar list` and
    // `show` are the only way an author reaches the corpus at all. Their
    // regions, seeds and route claims are the ones `grammar.md` §2c documents.
    entry(
        "idiom-arguments",
        idioms::arguments,
        [15, 7, 15],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "idiom-composition-arcade",
        idioms::composition_arcade,
        [3, 14, 20],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "idiom-erosion",
        idioms::erosion,
        [9, 5, 3],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "idiom-erosion-graded",
        idioms::graded_erosion,
        [9, 13, 3],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "idiom-light",
        idioms::light,
        [5, 6, 13],
        1,
        Kind::Example,
        ROUTE,
    ),
    entry(
        "idiom-mirror",
        idioms::mirror,
        [15, 11, 2],
        1,
        Kind::Example,
        MIRRORED_Y,
    ),
    entry(
        "idiom-priority",
        idioms::priority,
        [13, 6, 2],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "idiom-repetition",
        idioms::repetition,
        [3, 5, 17],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "idiom-shape",
        idioms::shape,
        [15, 9, 3],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "idiom-skip",
        idioms::skip,
        [7, 5, 5],
        1,
        Kind::Example,
        ROUTE,
    ),
    entry(
        "idiom-surface-detail",
        idioms::surface_detail,
        [9, 12, 9],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "lift-shaft",
        lift_shaft,
        [5, 16, 7],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    // A corpus example rather than an idiom-index entry (spec-0033 §4.8):
    // every IR construct owes `delve-grammar list` one example, and `none_of`
    // is a language feature rather than a technique. Its region is the one
    // `tests/idioms.rs` demonstrates the guard holding at.
    entry(
        "negated-guard",
        negated_guard::negated_guard,
        [5, 4, 12],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "rafter-hall",
        rafter_hall,
        [13, 6, 25],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    // A corpus example rather than an idiom-index entry (spec-0033 §4.8): the
    // one program that writes `claim` and a `contract` block. Its region and
    // seed are the ones its own module documents it at.
    entry(
        "spatial-contract",
        spatial_contract::spatial_contract,
        [11, 6, 15],
        1,
        Kind::Example,
        NO_CLAIM,
    ),
    entry(
        "stair-flight",
        stair_flight,
        [5, 14, 22],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "store-room",
        store_room,
        [7, 5, 14],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry(
        "tee-passage",
        tee_passage,
        [5, 5, 12],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry("temple", temple, [13, 14, 21], 1, Kind::Piece, NO_CLAIM),
    entry(
        "threshold-motif",
        threshold_motif,
        [9, 6, 13],
        1,
        Kind::Piece,
        NO_CLAIM,
    ),
    entry("watch-bay", watch_bay, [7, 7, 24], 1, Kind::Piece, NO_CLAIM),
];

/// Look one library entry up by its `PROGRAMS` id.
pub fn entry_by_id(id: &str) -> Option<&'static LibraryProgram> {
    PROGRAMS.iter().find(|p| p.id == id)
}

/// Look one library program up by its `PROGRAMS` id.
pub fn by_id(id: &str) -> Option<Program> {
    entry_by_id(id).map(|p| (p.build)())
}
