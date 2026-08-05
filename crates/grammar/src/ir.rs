//! The grammar program: a typed, serialisable IR.
//!
//! Ported from the decorator/context-manager DSL of `SplitGrammar.py`
//! (`yawgmoth/GDMC25`, BSD-3-Clause — see `LICENSE-GDMC25`). Upstream a rule
//! *is* a Python function and the derivation runs by executing it against a
//! global context stack; a rule body's structure is therefore only observable by
//! running it. We keep the semantics and drop the host language: a program is
//! data (spec-0027 §3, "the grammar program is the artifact of record"), which
//! is what makes it schema-checkable, hashable for provenance, diffable in
//! review, and safe to accept from an LLM.
//!
//! Two deliberate representation changes, both removing folklore:
//!
//! * split sizes carry their kind ([`Size::Absolute`] / [`Size::Relative`])
//!   instead of encoding "relative" as a negative integer;
//! * fills name block states (or palette roles) instead of integer material ids
//!   registered in a global side table.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::block::BlockState;
use crate::geom::{Axis, Orientation};

// ---------------------------------------------------------------------------
// Expressions and constraints
// ---------------------------------------------------------------------------

/// A measurable quantity of the current scope.
///
/// Local dimensions (`X`/`Y`/`Z`) are read through the scope's orientation;
/// world dimensions are read straight off the box. Upstream's `LARGEST` in
/// *constraints* returned `min(size)` — a copy/paste bug in `Scope.get_value`
/// (`SplitGrammar.py`); here it returns the maximum, as its name and its own
/// use in reorientation both require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimRef {
    /// Extent along the local `X`.
    X,
    /// Extent along the local `Y`.
    Y,
    /// Extent along the local `Z`.
    Z,
    /// Extent along the world `X`.
    WorldX,
    /// Extent along the world `Y`.
    WorldY,
    /// Extent along the world `Z`.
    WorldZ,
    /// The smallest of the three extents.
    Smallest,
    /// The largest of the three extents.
    Largest,
}

/// Integer arithmetic available inside constraints and split sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArithOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Floor division. Division by zero is a program error, never a panic.
    Div,
    /// Euclidean remainder. Modulo zero is a program error, never a panic.
    Rem,
    /// Maximum of the two operands.
    Max,
    /// Minimum of the two operands.
    Min,
}

/// An integer expression over constants, program parameters and scope
/// dimensions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "expr", rename_all = "snake_case")]
pub enum Expr {
    /// A literal.
    Int {
        /// The value.
        value: i64,
    },
    /// A program parameter, resolved against [`Program::params`].
    Param {
        /// Parameter name.
        name: String,
    },
    /// A dimension of the current scope.
    Dim {
        /// Which dimension.
        dim: DimRef,
    },
    /// Binary arithmetic.
    Arith {
        /// Left operand.
        lhs: Box<Expr>,
        /// Operator.
        op: ArithOp,
        /// Right operand.
        rhs: Box<Expr>,
    },
}

impl Expr {
    /// A literal.
    pub fn int(value: i64) -> Expr {
        Expr::Int { value }
    }

    /// A program parameter.
    pub fn param(name: &str) -> Expr {
        Expr::Param {
            name: name.to_string(),
        }
    }

    /// A scope dimension.
    pub fn dim(dim: DimRef) -> Expr {
        Expr::Dim { dim }
    }

    /// `self <op> rhs`.
    pub fn arith(self, op: ArithOp, rhs: Expr) -> Expr {
        Expr::Arith {
            lhs: Box::new(self),
            op,
            rhs: Box::new(rhs),
        }
    }
}

/// A comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CmpOp {
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `==`
    Eq,
    /// `!=`
    Ne,
}

/// A rule guard, evaluated against the scope the rule is about to expand into.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cond", rename_all = "snake_case")]
pub enum Cond {
    /// Always applicable.
    #[default]
    Always,
    /// Applicable only when no other alternative of the same rule matched —
    /// upstream's `Constraint.ELSE`.
    Otherwise,
    /// An integer comparison.
    Cmp {
        /// Left operand.
        lhs: Expr,
        /// Operator.
        op: CmpOp,
        /// Right operand.
        rhs: Expr,
    },
    /// Every sub-condition holds (upstream `Constraint.ALL` / `AND`).
    All {
        /// Sub-conditions.
        of: Vec<Cond>,
    },
    /// At least one sub-condition holds (upstream `ANY` / `OR`).
    Any {
        /// Sub-conditions.
        of: Vec<Cond>,
    },
    /// No sub-condition holds (upstream `NONE` / `NOT` / `NOR`).
    NoneOf {
        /// Sub-conditions.
        of: Vec<Cond>,
    },
    /// The scope's orientation is exactly this local-to-world mapping —
    /// upstream's `orientation() == (0, 1, 2)` checks, which pick the correctly
    /// facing stair/door variant.
    Orientation {
        /// World axis of the local `X`.
        x: Axis,
        /// World axis of the local `Y`.
        y: Axis,
        /// World axis of the local `Z`.
        z: Axis,
    },
}

impl Cond {
    /// `lhs <op> rhs`.
    pub fn cmp(lhs: Expr, op: CmpOp, rhs: Expr) -> Cond {
        Cond::Cmp { lhs, op, rhs }
    }
}

// ---------------------------------------------------------------------------
// Splits and reorientation
// ---------------------------------------------------------------------------

/// One piece of a split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "size", rename_all = "snake_case")]
pub enum Size {
    /// A fixed number of blocks.
    Absolute {
        /// Block count; must be non-negative.
        blocks: Expr,
    },
    /// A share of whatever the absolute pieces leave over.
    Relative {
        /// Share weight; must be positive.
        weight: Expr,
    },
}

impl Size {
    /// A fixed number of blocks.
    pub fn abs(blocks: i64) -> Size {
        Size::Absolute {
            blocks: Expr::int(blocks),
        }
    }

    /// One share of the leftover.
    pub fn rel(weight: i64) -> Size {
        Size::Relative {
            weight: Expr::int(weight),
        }
    }
}

/// Where the leftover blocks of a relative split go.
///
/// Upstream declares these four modes but implements only the truncating one:
/// `make_split` computes `int(reltotal/relsizes)` and silently drops the
/// remainder, so the children of a split need not cover their parent. We keep
/// [`Rounding::Truncate`] bit-compatible with that and give the other three the
/// behaviour their names require, so a program can ask for an exact cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rounding {
    /// Drop the remainder; the split may leave a gap at the far end.
    #[default]
    Truncate,
    /// Give the remainder to the earliest relative pieces.
    Start,
    /// Give the remainder to the latest relative pieces.
    End,
    /// Give the remainder to the middle relative pieces.
    Middle,
}

/// A local axis of the child scope, expressed in terms of the parent's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisSpec {
    /// The parent's local `X`.
    LocalX,
    /// The parent's local `Y`.
    LocalY,
    /// The parent's local `Z`.
    LocalZ,
    /// The world `X`, whatever the parent calls it.
    WorldX,
    /// The world `Y`.
    WorldY,
    /// The world `Z`.
    WorldZ,
    /// Whichever remaining axis the box is shortest along.
    Smallest,
    /// Whichever remaining axis the box is longest along.
    Largest,
    /// The axis being split. Only meaningful on a [`Node::Split`]; upstream's
    /// `Dimension.SPLIT`.
    SplitAxis,
}

/// A (possibly partial) reorientation request: unset axes are filled in by
/// [`crate::orient::reorient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Reorient {
    /// What the child should call its local `X`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<AxisSpec>,
    /// What the child should call its local `Y`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<AxisSpec>,
    /// What the child should call its local `Z`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<AxisSpec>,
}

impl Reorient {
    /// No reorientation: children keep the parent's axes.
    pub const KEEP: Reorient = Reorient {
        x: None,
        y: None,
        z: None,
    };

    /// True when nothing is requested.
    pub fn is_keep(&self) -> bool {
        *self == Reorient::KEEP
    }

    /// Set the child's local `X`.
    pub fn x(mut self, spec: AxisSpec) -> Reorient {
        self.x = Some(spec);
        self
    }

    /// Set the child's local `Y`.
    pub fn y(mut self, spec: AxisSpec) -> Reorient {
        self.y = Some(spec);
        self
    }

    /// Set the child's local `Z`.
    pub fn z(mut self, spec: AxisSpec) -> Reorient {
        self.z = Some(spec);
        self
    }
}

/// A subdivision of the current scope along one local axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Split {
    /// The local axis to cut along.
    pub axis: Axis,
    /// The piece pattern, in order.
    pub sizes: Vec<Size>,
    /// Where leftover blocks go.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rounding: Rounding,
    /// Repeat the pattern until the axis is consumed, clamping the last piece.
    #[serde(default, skip_serializing_if = "is_false")]
    pub repeat: bool,
    /// Orientation handed to every child.
    #[serde(default, skip_serializing_if = "Reorient::is_keep")]
    pub orient: Reorient,
    /// What to expand in each piece. Matched to pieces in order and **cycled**
    /// when a `repeat` split produces more pieces than children — upstream's
    /// `while items: void(); fill()` idiom.
    pub children: Vec<Node>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn is_false(v: &bool) -> bool {
    !*v
}

// ---------------------------------------------------------------------------
// Materials
// ---------------------------------------------------------------------------

/// One block of a weighted material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightedBlock {
    /// Relative weight; must be positive.
    pub weight: u32,
    /// The block state.
    pub block: BlockState,
}

/// What a palette role resolves to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Paint {
    /// A single block state.
    Block(BlockState),
    /// A per-cell weighted draw from the seeded stream (upstream's `dict`
    /// materials, which drew with `random.choices`).
    Mix(Vec<WeightedBlock>),
}

/// What a `fill` writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Material {
    /// A palette role, resolved against [`Program::palette`]. Swapping the
    /// palette restyles a whole program without touching its rules.
    Role {
        /// Role name.
        role: String,
    },
    /// An inline paint.
    Inline(Paint),
}

impl Material {
    /// A palette role.
    pub fn role(role: &str) -> Material {
        Material::Role {
            role: role.to_string(),
        }
    }

    /// An inline single block.
    pub fn block(block: BlockState) -> Material {
        Material::Inline(Paint::Block(block))
    }
}

// ---------------------------------------------------------------------------
// Rule bodies
// ---------------------------------------------------------------------------

/// One step of a rule body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Node {
    /// Write a material into every cell of the scope.
    Fill {
        /// What to write.
        material: Material,
    },
    /// Write air into every cell of the scope (upstream `void()`).
    Void,
    /// Leave the scope exactly as it is (upstream `skip()`), so an earlier fill
    /// shows through.
    Skip,
    /// Expand another rule in this scope.
    Call {
        /// Rule name.
        symbol: String,
    },
    /// Subdivide.
    Split(Split),
    /// Rename the scope's axes, then expand `body`.
    Reorient {
        /// The requested axis mapping.
        orient: Reorient,
        /// What to expand under it.
        body: Box<Node>,
    },
}

impl Node {
    /// Expand another rule.
    pub fn call(symbol: &str) -> Node {
        Node::Call {
            symbol: symbol.to_string(),
        }
    }

    /// Fill with a palette role.
    pub fn fill(role: &str) -> Node {
        Node::Fill {
            material: Material::role(role),
        }
    }
}

/// One alternative of a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alternative {
    /// Selection weight among the applicable alternatives. Must be positive.
    #[serde(default = "one")]
    pub weight: u32,
    /// The guard.
    #[serde(default, skip_serializing_if = "is_default")]
    pub when: Cond,
    /// The body.
    pub body: Node,
}

fn one() -> u32 {
    1
}

impl Alternative {
    /// An unguarded, unit-weight alternative.
    pub fn new(body: Node) -> Alternative {
        Alternative {
            weight: 1,
            when: Cond::Always,
            body,
        }
    }

    /// Guard this alternative.
    pub fn when(mut self, when: Cond) -> Alternative {
        self.when = when;
        self
    }

    /// Set the selection weight.
    pub fn weight(mut self, weight: u32) -> Alternative {
        self.weight = weight;
        self
    }
}

// ---------------------------------------------------------------------------
// Programs
// ---------------------------------------------------------------------------

/// A complete grammar program.
///
/// `BTreeMap` throughout: iteration order is the authoring-independent, stable
/// order determinism requires (ADR-0006).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    /// Human-readable program name; part of the provenance record.
    pub name: String,
    /// The rule expanded into the whole region.
    pub start: String,
    /// Integer knobs referenced by [`Expr::Param`] — the size/shape controls.
    #[serde(default)]
    pub params: BTreeMap<String, i64>,
    /// Role-to-block bindings — the style controls.
    #[serde(default)]
    pub palette: BTreeMap<String, Paint>,
    /// Rules, each a non-empty list of alternatives in declaration order.
    pub rules: BTreeMap<String, Vec<Alternative>>,
}

/// A program that cannot be expanded, found before any work is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    /// The start rule, or a rule a `call` names, does not exist.
    UnknownRule {
        /// The missing rule.
        symbol: String,
        /// Where it was referenced from (`"start"` or the naming rule).
        referenced_by: String,
    },
    /// A rule was declared with no alternatives.
    EmptyRule {
        /// The rule.
        symbol: String,
    },
    /// A `fill` names a palette role the program does not bind.
    UnknownRole {
        /// The missing role.
        role: String,
        /// The rule that names it.
        referenced_by: String,
    },
    /// An expression names a parameter the program does not declare.
    UnknownParam {
        /// The missing parameter.
        name: String,
        /// The rule that names it.
        referenced_by: String,
    },
    /// A split declared no pieces or no children.
    EmptySplit {
        /// The rule.
        symbol: String,
    },
    /// A non-repeating split's children do not match its pieces one for one.
    ChildCountMismatch {
        /// The rule.
        symbol: String,
        /// Number of pieces the pattern declares.
        pieces: usize,
        /// Number of children given.
        children: usize,
    },
    /// A zero weight cannot be selected and is therefore always an authoring
    /// mistake.
    ZeroWeight {
        /// The rule (or `"palette:<role>"`).
        symbol: String,
    },
    /// `Rounding` other than `Truncate` on a split with no relative pieces has
    /// nowhere to put the remainder.
    RoundingWithoutRelative {
        /// The rule.
        symbol: String,
    },
    /// [`AxisSpec::SplitAxis`] used outside a split.
    SplitAxisOutsideSplit {
        /// The rule.
        symbol: String,
    },
    /// A [`Cond::Orientation`] guard names an axis mapping that is not a
    /// permutation, so no scope can ever satisfy it.
    OrientationCondNotAPermutation {
        /// The rule.
        symbol: String,
        /// The mapping as written, local `X`/`Y`/`Z` to world axis.
        axes: [Axis; 3],
    },
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramError::UnknownRule {
                symbol,
                referenced_by,
            } => write!(
                f,
                "rule {symbol:?} (referenced by {referenced_by:?}) is not defined"
            ),
            ProgramError::EmptyRule { symbol } => write!(f, "rule {symbol:?} has no alternatives"),
            ProgramError::UnknownRole {
                role,
                referenced_by,
            } => write!(
                f,
                "palette role {role:?} (used by rule {referenced_by:?}) is not bound"
            ),
            ProgramError::UnknownParam {
                name,
                referenced_by,
            } => write!(
                f,
                "parameter {name:?} (used by rule {referenced_by:?}) is not declared"
            ),
            ProgramError::EmptySplit { symbol } => {
                write!(
                    f,
                    "rule {symbol:?} has a split with no sizes or no children"
                )
            }
            ProgramError::ChildCountMismatch {
                symbol,
                pieces,
                children,
            } => write!(
                f,
                "rule {symbol:?} splits into {pieces} pieces but gives {children} children; \
                 only a `repeat` split may cycle its children"
            ),
            ProgramError::ZeroWeight { symbol } => {
                write!(
                    f,
                    "{symbol:?} has a zero weight, which can never be selected"
                )
            }
            ProgramError::RoundingWithoutRelative { symbol } => write!(
                f,
                "rule {symbol:?} asks for non-truncating rounding on a split with no relative pieces"
            ),
            ProgramError::SplitAxisOutsideSplit { symbol } => {
                write!(f, "rule {symbol:?} uses `split_axis` outside a split")
            }
            ProgramError::OrientationCondNotAPermutation { symbol, axes } => write!(
                f,
                "rule {symbol:?} guards on the orientation {axes:?}, which is not a permutation \
                 of the three world axes — no scope can ever match it, so the alternative is \
                 dead code"
            ),
        }
    }
}

impl std::error::Error for ProgramError {}

impl Program {
    /// An empty program with the given name and start rule.
    pub fn new(name: &str, start: &str) -> Program {
        Program {
            name: name.to_string(),
            start: start.to_string(),
            params: BTreeMap::new(),
            palette: BTreeMap::new(),
            rules: BTreeMap::new(),
        }
    }

    /// Declare a parameter (builder form).
    pub fn param(mut self, name: &str, value: i64) -> Program {
        self.params.insert(name.to_string(), value);
        self
    }

    /// Bind a palette role to a single block (builder form).
    pub fn role(mut self, role: &str, block: BlockState) -> Program {
        self.palette.insert(role.to_string(), Paint::Block(block));
        self
    }

    /// Bind a palette role to a weighted mix (builder form).
    pub fn role_mix(mut self, role: &str, mix: Vec<WeightedBlock>) -> Program {
        self.palette.insert(role.to_string(), Paint::Mix(mix));
        self
    }

    /// Declare a rule with a single unguarded alternative (builder form).
    pub fn rule(self, symbol: &str, body: Node) -> Program {
        self.rule_alts(symbol, vec![Alternative::new(body)])
    }

    /// Declare a rule with explicit alternatives (builder form).
    pub fn rule_alts(mut self, symbol: &str, alts: Vec<Alternative>) -> Program {
        self.rules.insert(symbol.to_string(), alts);
        self
    }

    /// Override a parameter, returning the previous value if it was declared.
    ///
    /// Overriding an *undeclared* parameter is refused: a typo in a sweep must
    /// not silently produce the default model.
    pub fn set_param(&mut self, name: &str, value: i64) -> Result<i64, ProgramError> {
        match self.params.get_mut(name) {
            Some(slot) => Ok(std::mem::replace(slot, value)),
            None => Err(ProgramError::UnknownParam {
                name: name.to_string(),
                referenced_by: self.name.clone(),
            }),
        }
    }

    /// Override a palette role, returning the previous binding. Refuses an
    /// unbound role, for the same reason [`Program::set_param`] does.
    pub fn set_role(&mut self, role: &str, paint: Paint) -> Result<Paint, ProgramError> {
        match self.palette.get_mut(role) {
            Some(slot) => Ok(std::mem::replace(slot, paint)),
            None => Err(ProgramError::UnknownRole {
                role: role.to_string(),
                referenced_by: self.name.clone(),
            }),
        }
    }

    /// Check every reference the program makes, and every structural rule that
    /// can be decided without expanding. [`crate::expand`] runs this first.
    pub fn validate(&self) -> Result<(), ProgramError> {
        if !self.rules.contains_key(&self.start) {
            return Err(ProgramError::UnknownRule {
                symbol: self.start.clone(),
                referenced_by: "start".to_string(),
            });
        }
        for (symbol, alts) in &self.rules {
            if alts.is_empty() {
                return Err(ProgramError::EmptyRule {
                    symbol: symbol.clone(),
                });
            }
            for alt in alts {
                if alt.weight == 0 {
                    return Err(ProgramError::ZeroWeight {
                        symbol: symbol.clone(),
                    });
                }
                self.check_cond(symbol, &alt.when)?;
                self.check_node(symbol, &alt.body, false)?;
            }
        }
        for (role, paint) in &self.palette {
            if let Paint::Mix(mix) = paint
                && (mix.is_empty() || mix.iter().any(|w| w.weight == 0))
            {
                return Err(ProgramError::ZeroWeight {
                    symbol: format!("palette:{role}"),
                });
            }
        }
        Ok(())
    }

    fn check_node(&self, symbol: &str, node: &Node, in_split: bool) -> Result<(), ProgramError> {
        match node {
            Node::Void | Node::Skip => Ok(()),
            Node::Fill { material } => {
                let paint = match material {
                    Material::Role { role } => {
                        self.palette
                            .get(role)
                            .ok_or_else(|| ProgramError::UnknownRole {
                                role: role.clone(),
                                referenced_by: symbol.to_string(),
                            })?
                    }
                    Material::Inline(paint) => paint,
                };
                if let Paint::Mix(mix) = paint
                    && (mix.is_empty() || mix.iter().any(|w| w.weight == 0))
                {
                    return Err(ProgramError::ZeroWeight {
                        symbol: symbol.to_string(),
                    });
                }
                Ok(())
            }
            Node::Call { symbol: target } => {
                if self.rules.contains_key(target) {
                    Ok(())
                } else {
                    Err(ProgramError::UnknownRule {
                        symbol: target.clone(),
                        referenced_by: symbol.to_string(),
                    })
                }
            }
            Node::Reorient { orient, body } => {
                self.check_orient(symbol, orient, in_split)?;
                self.check_node(symbol, body, in_split)
            }
            Node::Split(split) => {
                if split.sizes.is_empty() || split.children.is_empty() {
                    return Err(ProgramError::EmptySplit {
                        symbol: symbol.to_string(),
                    });
                }
                if !split.repeat && split.sizes.len() != split.children.len() {
                    return Err(ProgramError::ChildCountMismatch {
                        symbol: symbol.to_string(),
                        pieces: split.sizes.len(),
                        children: split.children.len(),
                    });
                }
                let has_relative = split
                    .sizes
                    .iter()
                    .any(|s| matches!(s, Size::Relative { .. }));
                if split.rounding != Rounding::Truncate && !has_relative {
                    return Err(ProgramError::RoundingWithoutRelative {
                        symbol: symbol.to_string(),
                    });
                }
                for size in &split.sizes {
                    let e = match size {
                        Size::Absolute { blocks } => blocks,
                        Size::Relative { weight } => weight,
                    };
                    self.check_expr(symbol, e)?;
                }
                self.check_orient(symbol, &split.orient, true)?;
                for child in &split.children {
                    self.check_node(symbol, child, false)?;
                }
                Ok(())
            }
        }
    }

    fn check_orient(
        &self,
        symbol: &str,
        orient: &Reorient,
        in_split: bool,
    ) -> Result<(), ProgramError> {
        for spec in [orient.x, orient.y, orient.z].into_iter().flatten() {
            if spec == AxisSpec::SplitAxis && !in_split {
                return Err(ProgramError::SplitAxisOutsideSplit {
                    symbol: symbol.to_string(),
                });
            }
        }
        Ok(())
    }

    fn check_cond(&self, symbol: &str, cond: &Cond) -> Result<(), ProgramError> {
        match cond {
            Cond::Always | Cond::Otherwise => Ok(()),
            // An orientation is a permutation by definition (`geom::Orientation`),
            // but the guard spells one out field by field, so `{x: z, y: z, z: z}`
            // is expressible. It matches nothing, ever — which at expansion time
            // surfaces as a baffling `NoApplicableRule` about a *different*
            // alternative. Refuse it where it was written (PR #266 review).
            Cond::Orientation { x, y, z } => {
                let axes = [*x, *y, *z];
                if Orientation::from_axes(axes).is_permutation() {
                    Ok(())
                } else {
                    Err(ProgramError::OrientationCondNotAPermutation {
                        symbol: symbol.to_string(),
                        axes,
                    })
                }
            }
            Cond::Cmp { lhs, rhs, .. } => {
                self.check_expr(symbol, lhs)?;
                self.check_expr(symbol, rhs)
            }
            Cond::All { of } | Cond::Any { of } | Cond::NoneOf { of } => {
                for c in of {
                    self.check_cond(symbol, c)?;
                }
                Ok(())
            }
        }
    }

    fn check_expr(&self, symbol: &str, expr: &Expr) -> Result<(), ProgramError> {
        match expr {
            Expr::Int { .. } | Expr::Dim { .. } => Ok(()),
            Expr::Param { name } => {
                if self.params.contains_key(name) {
                    Ok(())
                } else {
                    Err(ProgramError::UnknownParam {
                        name: name.clone(),
                        referenced_by: symbol.to_string(),
                    })
                }
            }
            Expr::Arith { lhs, rhs, .. } => {
                self.check_expr(symbol, lhs)?;
                self.check_expr(symbol, rhs)
            }
        }
    }
}
