//! Composing one program out of others — the seam a zone program is built on.
//!
//! A zone of a campaign is **one** grammar program (REMAKE §2), but the shapes
//! it is made of are the staging vocabulary of [`crate::library`], and a `call`
//! reaches only rules of the program it is written in. [`include`] closes that
//! gap: it copies a source program's rules, parameters and palette into a
//! destination under a prefix, rewriting every internal reference as it goes, so
//! the destination can `call` the source's start rule as if the vocabulary had
//! been written inline.
//!
//! ```text
//!   rafter_hall           bell_hall_keep
//!   ├── hall              ├── keep                (the zone's own plan)
//!   ├── hall_shell   ──▶  ├── hall/hall           call("hall/hall")
//!   ├── nave              ├── hall/hall_shell
//!   └── …                 ├── hall/nave
//!                         ├── door/threshold      call("door/threshold")
//!                         └── …
//! ```
//!
//! # What is renamed, and what deliberately is not
//!
//! Renamed, because they are the program's private vocabulary: rule names and
//! the `call`s that reach them (including a rule's calls to *itself* — the
//! storeroom's tell is placed by a recursion, and a rewrite that missed it would
//! be a loud `UnknownRule` from [`crate::ir::Program::validate`], never a quiet
//! wrong model), parameter names and the [`Expr::Param`] reads of them, palette
//! roles and the `fill`s that name them.
//!
//! **Not** renamed: anchors. An anchor name is the contract with the campaign
//! DSL — `anchor/watch` is the id a `timed-gate` binds — so an include that
//! renamed it would make the vocabulary's whole reason for declaring anchors
//! evaporate at the moment of composition. The consequence is a real limit and
//! it is loud rather than silent: including the *same* piece twice in one zone
//! makes both copies declare `anchor/watch`, and expansion refuses with
//! `AnchorCollision` naming both rules. A zone that needs two watch bays needs
//! an anchor-namespace primitive on `mark`, which this module does not invent
//! (`docs/reference/grammar.md` §7).
//!
//! # Determinism
//!
//! Nothing here draws from the RNG or reads the environment; the maps stay
//! `BTreeMap`s, so a composed program serialises in the same authoring-order-
//! independent form as a hand-written one (ADR-0006).

use std::collections::BTreeMap;
use std::fmt;

use crate::ir::{Alternative, Cond, Expr, Mark, MarkAt, Material, Node, Program, Size, Split};

/// A composition that would have silently merged two names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeError {
    /// The prefix itself is unusable.
    BadPrefix {
        /// The prefix as given.
        prefix: String,
    },
    /// A prefixed name already exists in the destination.
    Clash {
        /// What kind of name clashed: `"rule"`, `"parameter"` or `"palette role"`.
        kind: &'static str,
        /// The prefixed name.
        name: String,
    },
}

impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComposeError::BadPrefix { prefix } => write!(
                f,
                "include prefix {prefix:?} is not usable: a prefix must be a non-empty name \
                 without a {SEPARATOR:?}, since that is what separates it from the included name"
            ),
            ComposeError::Clash { kind, name } => write!(
                f,
                "including under this prefix would redefine the {kind} {name:?}; two programs \
                 composed into one must not share a name, and merging them silently would make \
                 the second win"
            ),
        }
    }
}

impl std::error::Error for ComposeError {}

/// What separates an include prefix from the name it qualifies.
pub const SEPARATOR: char = '/';

/// The rule name a `call` uses to enter `source` once it has been included
/// under `prefix`.
///
/// The zone program that composes a piece needs exactly one name from it — its
/// start rule — and this is that name, derived rather than restated, so a
/// vocabulary rule that renames its own entry point cannot leave a zone calling
/// a symbol that no longer exists.
pub fn entry(prefix: &str, source: &Program) -> String {
    qualify(prefix, &source.start)
}

/// Copy `source` into `destination` under `prefix`.
///
/// Every rule, parameter and palette role of `source` gains the prefix, and
/// every reference `source` makes to one of them is rewritten to match. The
/// destination's own names are untouched, and a collision is refused rather than
/// merged.
///
/// The source's `start` rule is *not* made the destination's start — a zone has
/// its own plan and calls the piece from inside it; use [`entry`] for the name.
pub fn include(
    mut destination: Program,
    source: &Program,
    prefix: &str,
) -> Result<Program, ComposeError> {
    if prefix.is_empty() || prefix.contains(SEPARATOR) {
        return Err(ComposeError::BadPrefix {
            prefix: prefix.to_string(),
        });
    }
    for (name, value) in &source.params {
        insert(
            &mut destination.params,
            qualify(prefix, name),
            *value,
            "parameter",
        )?;
    }
    for (role, paint) in &source.palette {
        insert(
            &mut destination.palette,
            qualify(prefix, role),
            paint.clone(),
            "palette role",
        )?;
    }
    for (symbol, alts) in &source.rules {
        let rewritten = alts.iter().map(|alt| Alternative {
            weight: alt.weight,
            when: cond(prefix, &alt.when),
            body: node(prefix, &alt.body),
        });
        insert(
            &mut destination.rules,
            qualify(prefix, symbol),
            rewritten.collect(),
            "rule",
        )?;
    }
    Ok(destination)
}

fn insert<V>(
    map: &mut BTreeMap<String, V>,
    name: String,
    value: V,
    kind: &'static str,
) -> Result<(), ComposeError> {
    if map.contains_key(&name) {
        return Err(ComposeError::Clash { kind, name });
    }
    map.insert(name, value);
    Ok(())
}

fn qualify(prefix: &str, name: &str) -> String {
    format!("{prefix}{SEPARATOR}{name}")
}

fn node(prefix: &str, node: &Node) -> Node {
    match node {
        Node::Void => Node::Void,
        Node::Skip => Node::Skip,
        Node::Fill { material } => Node::Fill {
            material: match material {
                Material::Role { role } => Material::Role {
                    role: qualify(prefix, role),
                },
                inline @ Material::Inline(_) => inline.clone(),
            },
        },
        Node::Call { symbol } => Node::Call {
            symbol: qualify(prefix, symbol),
        },
        Node::Split(split) => Node::Split(Split {
            axis: split.axis,
            sizes: split.sizes.iter().map(|s| size(prefix, s)).collect(),
            rounding: split.rounding,
            repeat: split.repeat,
            orient: split.orient,
            children: split
                .children
                .iter()
                .map(|c| self::node(prefix, c))
                .collect(),
        }),
        Node::Reorient { orient, body } => Node::Reorient {
            orient: *orient,
            body: Box::new(self::node(prefix, body)),
        },
        Node::Mark { mark, body } => Node::Mark {
            // The anchor stem is untouched on purpose: it is the campaign's
            // name for the place, not the program's name for a rule.
            mark: Mark {
                anchor: mark.anchor.clone(),
                at: at(prefix, &mark.at),
                facing: mark.facing,
                index: mark.index,
            },
            body: Box::new(self::node(prefix, body)),
        },
    }
}

/// Every variant is spelled out, and there is no catch-all arm anywhere in this
/// walk on purpose.
///
/// This function's whole job is "rewrite every reference the source makes", and
/// a `_ =>` arm is the one way a future variant that carries an [`Expr`] — a
/// param-driven mark position is the obvious one — would compile green and skip
/// the rewrite. That failure would not be loud the way an unrewritten `call` is
/// (`UnknownRule`, before any expansion): the composed program would read the
/// *destination's* parameter of the unqualified name if it happened to have one,
/// and build a silently different model. So the compiler is left as the thing
/// that notices.
fn at(prefix: &str, at: &MarkAt) -> MarkAt {
    match at {
        MarkAt::Offset { x, y, z } => MarkAt::Offset {
            x: expr(prefix, x),
            y: expr(prefix, y),
            z: expr(prefix, z),
        },
        MarkAt::FloorCenter => MarkAt::FloorCenter,
        MarkAt::CornerMin => MarkAt::CornerMin,
        MarkAt::FaceCenter { axis, side } => MarkAt::FaceCenter {
            axis: *axis,
            side: *side,
        },
    }
}

fn size(prefix: &str, size: &Size) -> Size {
    match size {
        Size::Absolute { blocks } => Size::Absolute {
            blocks: expr(prefix, blocks),
        },
        Size::Relative { weight } => Size::Relative {
            weight: expr(prefix, weight),
        },
    }
}

fn cond(prefix: &str, cond: &Cond) -> Cond {
    match cond {
        Cond::Always => Cond::Always,
        Cond::Otherwise => Cond::Otherwise,
        Cond::Cmp { lhs, op, rhs } => Cond::Cmp {
            lhs: expr(prefix, lhs),
            op: *op,
            rhs: expr(prefix, rhs),
        },
        Cond::All { of } => Cond::All {
            of: of.iter().map(|c| self::cond(prefix, c)).collect(),
        },
        Cond::Any { of } => Cond::Any {
            of: of.iter().map(|c| self::cond(prefix, c)).collect(),
        },
        Cond::NoneOf { of } => Cond::NoneOf {
            of: of.iter().map(|c| self::cond(prefix, c)).collect(),
        },
        orientation @ Cond::Orientation { .. } => orientation.clone(),
    }
}

fn expr(prefix: &str, expr: &Expr) -> Expr {
    match expr {
        Expr::Int { value } => Expr::Int { value: *value },
        Expr::Param { name } => Expr::Param {
            name: qualify(prefix, name),
        },
        Expr::Dim { dim } => Expr::Dim { dim: *dim },
        Expr::Arith { lhs, op, rhs } => Expr::Arith {
            lhs: Box::new(self::expr(prefix, lhs)),
            op: *op,
            rhs: Box::new(self::expr(prefix, rhs)),
        },
    }
}
