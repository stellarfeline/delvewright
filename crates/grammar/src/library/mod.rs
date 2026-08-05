//! The ported rule libraries.
//!
//! These are direct ports of the example grammars shipped with
//! `yawgmoth/GDMC25` (BSD-3-Clause — see `LICENSE-GDMC25`): the Greek temple
//! (`MakeTemple.py` / `Tetrastyle.py`, Markus Eger), the castle
//! (`MakeCastle.py`, Markus Eger) and the church (`MakeChurch.py`, Janista
//! Gitbumrungsin). spec-0027 §2 keeps them for two jobs: they are the regression
//! fixtures the interpreter is judged against, and they are a few-shot corpus of
//! grammar programs whose licence lets us use it.
//!
//! Each port is faithful to its source's rule structure. Where a rule's shape
//! changed, the module says so at the rule.
//!
//! Every program here is parameterised: integer knobs in
//! [`Program::params`](crate::ir::Program::params) are the size/kind controls
//! and role bindings in [`Program::palette`](crate::ir::Program::palette) are
//! the style controls, so one program yields a family of models rather than one
//! building.

pub mod castle;
pub mod church;
pub mod temple;

pub use castle::castle;
pub use church::church;
pub use temple::temple;

use crate::geom::Axis;
use crate::ir::{
    Alternative, ArithOp, CmpOp, Cond, DimRef, Expr, Mark, MarkAt, Node, Reorient, Rounding, Size,
    Split,
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

/// A guarded alternative.
fn alt_when(when: Cond, body: Node) -> Alternative {
    Alternative::new(body).when(when)
}

/// The fallback alternative.
fn alt_else(body: Node) -> Alternative {
    Alternative::new(body).when(Cond::Otherwise)
}
