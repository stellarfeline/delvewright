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
//! whose reason to exist is a machine gate about how the space plays. They share
//! one local frame, and it is worth stating once because every derived anchor
//! facing depends on it:
//!
//! > **Local `Y` is up. Local `Z`-max is the approach end, and travel runs
//! > toward local `Z`-min.**
//!
//! That is not a coin flip. A [`Mark`]'s facing, when it is not spelled out as a
//! world direction, is *always* the negative direction of the world axis the
//! scope calls local `Z` — so a rule can only hand an anchor a facing that
//! points down-axis. Choosing travel to run that way is what makes every anchor
//! these rules declare look at the thing it is about. The cost is that anchors
//! number *against* travel (a split visits its pieces low to high); see
//! [`cliff_path`].
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
pub mod bell;
pub mod boulder_stair;
pub mod broken_grate;
pub mod castle;
pub mod causeway;
pub mod church;
pub mod cliff_path;
pub mod drop_shaft;
pub mod dumbwaiter;
pub mod elite_ground;
pub mod far_side_bar;
pub mod rafter_hall;
pub mod stair_flight;
pub mod store_room;
pub mod tee_passage;
pub mod temple;
pub mod threshold_motif;
pub mod watch_bay;

pub use ambush_door::ambush_door;
pub use bell::{barrow_shore, chapel_ward, cistern_deep, cliff_road, gate_ward, hall_keep};
pub use boulder_stair::boulder_stair;
pub use broken_grate::broken_grate;
pub use castle::castle;
pub use causeway::causeway;
pub use church::church;
pub use cliff_path::cliff_path;
pub use drop_shaft::drop_shaft;
pub use dumbwaiter::dumbwaiter;
pub use elite_ground::elite_ground;
pub use far_side_bar::far_side_bar;
pub use rafter_hall::rafter_hall;
pub use stair_flight::stair_flight;
pub use store_room::store_room;
pub use tee_passage::tee_passage;
pub use temple::temple;
pub use threshold_motif::threshold_motif;
pub use watch_bay::watch_bay;

use crate::geom::Axis;
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

/// Expand another rule.
fn call(symbol: &str) -> Node {
    Node::call(symbol)
}

/// Fill with a palette role.
fn fill(role: &str) -> Node {
    Node::fill(role)
}

/// Write air.
fn void() -> Node {
    Node::Void
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
