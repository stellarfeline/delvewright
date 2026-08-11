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

impl DimRef {
    /// The dimension as a grammar author reads it (`Dimension.X`).
    pub fn as_str(self) -> &'static str {
        match self {
            DimRef::X => "Dimension.X",
            DimRef::Y => "Dimension.Y",
            DimRef::Z => "Dimension.Z",
            DimRef::WorldX => "Dimension.WorldX",
            DimRef::WorldY => "Dimension.WorldY",
            DimRef::WorldZ => "Dimension.WorldZ",
            DimRef::Smallest => "Dimension.Smallest",
            DimRef::Largest => "Dimension.Largest",
        }
    }
}

impl fmt::Display for DimRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
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

impl ArithOp {
    /// The operator as a grammar author reads it: an infix symbol, or the name
    /// of the two-argument form for [`ArithOp::Max`] / [`ArithOp::Min`].
    pub fn as_str(self) -> &'static str {
        match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
            ArithOp::Div => "/",
            ArithOp::Rem => "%",
            ArithOp::Max => "max",
            ArithOp::Min => "min",
        }
    }

    /// True when the operator reads as `op(a, b)` rather than `a op b`.
    pub fn is_call(self) -> bool {
        matches!(self, ArithOp::Max | ArithOp::Min)
    }

    /// How tightly the operator binds, for parenthesising a rendered
    /// expression. Higher binds tighter; a leaf and a call form are `3`.
    fn precedence(self) -> u8 {
        match self {
            ArithOp::Add | ArithOp::Sub => 1,
            ArithOp::Mul | ArithOp::Div | ArithOp::Rem => 2,
            ArithOp::Max | ArithOp::Min => 3,
        }
    }
}

impl fmt::Display for ArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
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

    /// How tightly this expression binds; leaves bind tightest.
    fn precedence(&self) -> u8 {
        match self {
            Expr::Int { .. } | Expr::Param { .. } | Expr::Dim { .. } => 3,
            Expr::Arith { op, .. } => op.precedence(),
        }
    }

    /// True when the expression is one name or one literal, so a rendering of
    /// it says everything there is to say and its value needs no breakdown.
    pub fn is_leaf(&self) -> bool {
        matches!(
            self,
            Expr::Int { .. } | Expr::Param { .. } | Expr::Dim { .. }
        )
    }

    /// The named quantities this expression reads, in reading order, each once.
    ///
    /// A guard operand's *number* says how far off a scope is; these say which
    /// knob moves it. Empty for a literal, and one entry for a bare name.
    pub fn inputs(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_inputs(&mut out);
        out
    }

    fn collect_inputs(&self, out: &mut Vec<String>) {
        match self {
            Expr::Int { .. } => {}
            Expr::Param { name } => {
                if !out.iter().any(|n| n == name) {
                    out.push(name.clone());
                }
            }
            Expr::Dim { dim } => {
                let name = dim.as_str();
                if !out.iter().any(|n| n == name) {
                    out.push(name.to_string());
                }
            }
            Expr::Arith { lhs, rhs, .. } => {
                lhs.collect_inputs(out);
                rhs.collect_inputs(out);
            }
        }
    }
}

impl fmt::Display for Expr {
    /// The expression as a grammar author reads it, bracketed conservatively:
    /// a nested operand keeps its brackets when it binds *less* tightly than
    /// its parent, and, on the **right** of an infix operator, when it binds no
    /// more tightly. Never fewer than the tree's meaning requires, sometimes one
    /// pair more (`a + (b + c)` is bracketed though it need not be).
    ///
    /// Integer division makes this load-bearing rather than cosmetic:
    /// `a * (b / c)` and `a * b / c` are genuinely different values, so an
    /// operand of equal binding strength on the **right** of an infix operator
    /// always keeps its brackets. Left-nested chains — the common shape, and
    /// the one the zone guards are written in — stay flat:
    /// `Dimension.Z - junction_run - hearth_run`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Int { value } => write!(f, "{value}"),
            Expr::Param { name } => f.write_str(name),
            Expr::Dim { dim } => write!(f, "{dim}"),
            Expr::Arith { lhs, op, rhs } if op.is_call() => {
                write!(f, "{op}({lhs}, {rhs})")
            }
            Expr::Arith { lhs, op, rhs } => {
                let here = op.precedence();
                if lhs.precedence() < here {
                    write!(f, "({lhs})")?;
                } else {
                    write!(f, "{lhs}")?;
                }
                write!(f, " {op} ")?;
                let bracket_rhs = matches!(rhs.as_ref(), Expr::Arith { op, .. } if op.precedence() <= here && !op.is_call());
                if bracket_rhs {
                    write!(f, "({rhs})")
                } else {
                    write!(f, "{rhs}")
                }
            }
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

impl CmpOp {
    /// The operator as a grammar author reads it (`>`).
    pub fn as_str(self) -> &'static str {
        match self {
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
        }
    }
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
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
    ///
    /// **Axis names only**: it does not read the orientation's signs, so a
    /// piece and the same piece turned round answer it identically. That is
    /// deliberate — every upstream use is "is my length running east-west",
    /// which a half-turn does not change — and a guard that wants the sign has
    /// no business being a `Cond` anyway: it would be selecting a variant for a
    /// mirroring the interpreter already performs on the body it guards.
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
///
/// Two independent halves. The [`AxisSpec`] fields say **which** parent axis
/// each of the child's local axes names; `reverse` says whether the child
/// counts that axis **backwards**. They are independent because the questions
/// are: a piece can be turned 90° without being turned round, and turned round
/// without being turned 90°.
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
    /// Which of the child's local axes run **backwards** along the parent axis
    /// they name, indexed by local axis ([`Axis::index`]).
    ///
    /// Relative to the parent, never to the world: a rule reasons in the frame
    /// it was handed, so reversals compose (reversing an already-reversed axis
    /// puts it back). Defaults to none, and is skipped in JSON when it is.
    #[serde(default, skip_serializing_if = "no_reversal")]
    pub reverse: [bool; 3],
}

/// Serde helper: a request with no reversal writes no `reverse` key.
fn no_reversal(reverse: &[bool; 3]) -> bool {
    !reverse[0] && !reverse[1] && !reverse[2]
}

impl Reorient {
    /// No reorientation: children keep the parent's axes, running the same way.
    pub const KEEP: Reorient = Reorient {
        x: None,
        y: None,
        z: None,
        reverse: [false; 3],
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

    /// Run one of the child's local axes backwards along the parent axis it
    /// names.
    ///
    /// The whole body mirrors along that axis: split pieces are laid from the
    /// far end, a local offset is measured from the far end, and a derived
    /// facing points the other way. One reversal is a reflection; two is a
    /// rotation — see [`Reorient::turned`] and
    /// [`crate::geom::Orientation::is_rotation`].
    pub fn reverse(mut self, axis: Axis) -> Reorient {
        self.reverse[axis.index()] = !self.reverse[axis.index()];
        self
    }

    /// A half-turn about the vertical: local `X` and local `Z` both reversed,
    /// `Y` left alone.
    ///
    /// The piece faces the way it came. Named because it is the case a route
    /// doubling back always wants — a hairpin leg, a spiral stair's return
    /// flight, a corridor entered from the other end — and because it is the
    /// one combination that is a *rotation*, so a chiral piece stays the hand
    /// it was built.
    pub fn turned(self) -> Reorient {
        self.reverse(Axis::X).reverse(Axis::Z)
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
// Anchors
// ---------------------------------------------------------------------------

/// Which end of an axis a [`MarkAt::FaceCenter`] means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// The low end of the axis.
    Min,
    /// The high end.
    Max,
}

/// A cardinal direction an anchor can face, as prefab metadata spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facing {
    /// `-Z`.
    North,
    /// `+Z`.
    South,
    /// `-X`.
    West,
    /// `+X`.
    East,
}

impl Facing {
    /// The metadata keyword.
    pub fn as_str(self) -> &'static str {
        match self {
            Facing::North => "north",
            Facing::South => "south",
            Facing::West => "west",
            Facing::East => "east",
        }
    }
}

impl fmt::Display for Facing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which cell of the current scope a [`Mark`] lands on.
///
/// The named positions are the ones a staging anchor actually wants, so that a
/// rule need not recompute `(size - 1) / 2` by hand — and so that the *intent*
/// survives into review. Centres round down on an even extent (the lower-middle
/// cell), which is a choice, not an accident: it has to be one of the two, and
/// it has to be the same one every time (ADR-0006).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum MarkAt {
    /// The centre of the scope's **world** floor: lowest world `Y`, centred on
    /// world `X` and `Z`. Gravity is a world fact, so this one position ignores
    /// the scope's local axis names — an NPC stands on the floor however the
    /// rule chose to call its axes.
    FloorCenter,
    /// The scope's minimum corner. A permutation cannot mirror, so the local
    /// minimum corner and the world one are the same cell.
    CornerMin,
    /// The centre of one face: the given **local** axis pinned to `side`, the
    /// other two centred.
    FaceCenter {
        /// The local axis to pin.
        axis: Axis,
        /// Which end of it.
        side: Side,
    },
    /// An explicit offset in **local** cells from the scope's minimum corner.
    Offset {
        /// Along the local `X`.
        x: Expr,
        /// Along the local `Y`.
        y: Expr,
        /// Along the local `Z`.
        z: Expr,
    },
}

impl MarkAt {
    /// A literal local offset from the scope's minimum corner.
    pub fn offset(x: i64, y: i64, z: i64) -> MarkAt {
        MarkAt::Offset {
            x: Expr::int(x),
            y: Expr::int(y),
            z: Expr::int(z),
        }
    }
}

/// How a [`Mark`]'s anchor name is completed when the rule runs more than once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkIndex {
    /// The name is exactly `anchor/<anchor>`. A second mark producing the same
    /// name is an error — one name, one place.
    #[default]
    Unique,
    /// The name is `anchor/<anchor>-<n>`, `n` counting from 1 per stem in
    /// expansion order. This is how a rule that expands once per tower gives
    /// every tower its own anchor without knowing how many towers there are.
    Auto,
}

/// An anchor declaration: a named position the prefab offers the campaign.
///
/// Anchors are **metadata**, not geometry: no composition of `fill` / `split`
/// can express "this cell is where the boss stands", and reading one back out of
/// the block pattern afterwards is a guess. So the rule that shapes the space
/// says so while it has the box in hand (spec-0027 phase 2b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    /// The anchor name stem, kebab-case. The exported key is `anchor/<stem>`
    /// (plus the index suffix when [`MarkIndex::Auto`]), which is the DSL's
    /// `anchor/<kebab>` id grammar — a mark cannot name an anchor the DSL could
    /// not reference.
    pub anchor: String,
    /// Which cell of the scope. Flattened, so the authoring form reads
    /// `{"anchor": "bay", "at": "floor_center"}` rather than nesting a second
    /// object under a key of the same name.
    #[serde(flatten)]
    pub at: MarkAt,
    /// The facing to declare. Omitted, it is derived from the scope's
    /// orientation: the negative direction of the world axis the scope calls
    /// local `Z` (`north` when that is world `Z`, `west` when it is world `X`).
    /// A scope whose local `Z` is vertical has no cardinal facing to derive, and
    /// says so rather than guessing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<Facing>,
    /// Name completion.
    #[serde(default, skip_serializing_if = "is_default")]
    pub index: MarkIndex,
}

impl Mark {
    /// A uniquely-named mark with a derived facing.
    pub fn new(anchor: &str, at: MarkAt) -> Mark {
        Mark {
            anchor: anchor.to_string(),
            at,
            facing: None,
            index: MarkIndex::Unique,
        }
    }

    /// Declare the facing explicitly instead of deriving it.
    pub fn facing(mut self, facing: Facing) -> Mark {
        self.facing = Some(facing);
        self
    }

    /// Number this mark per expansion instead of requiring a unique name.
    pub fn indexed(mut self) -> Mark {
        self.index = MarkIndex::Auto;
        self
    }

    /// The exported anchor name for occurrence `n` (1-based) of this stem.
    pub fn name(&self, n: u32) -> String {
        match self.index {
            MarkIndex::Unique => format!("anchor/{}", self.anchor),
            MarkIndex::Auto => format!("anchor/{}-{n}", self.anchor),
        }
    }
}

/// True for a kebab-case anchor stem: the `<kebab>` of the DSL's
/// `anchor/<kebab>` ids.
///
/// `pub(crate)` because [`crate::compose`] applies the same test to a rename's
/// *target* at the include site. `Program::validate` would catch a bad stem
/// either way, but it would name the rule that carries the mark — a rule inside
/// an included piece, which the caller never wrote — instead of the rename entry
/// the caller did write.
pub(crate) fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
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
    /// Declare an anchor in this scope, then expand `body`.
    ///
    /// A wrapper rather than a statement because a rule body is one node: this
    /// way a mark can sit on any piece of any split — the courtyard child, the
    /// tower child — and annotate exactly the box that piece owns. `body` is
    /// [`Node::Skip`] when the mark is all that is wanted.
    Mark {
        /// The declaration.
        mark: Mark,
        /// What to expand in the same scope.
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
    /// A `mark` names an anchor stem that is not kebab-case, so the exported
    /// key would not be a DSL `anchor/<kebab>` id.
    BadAnchorName {
        /// The rule.
        symbol: String,
        /// The stem as written.
        anchor: String,
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
            ProgramError::BadAnchorName { symbol, anchor } => write!(
                f,
                "rule {symbol:?} marks the anchor {anchor:?}, which is not kebab-case; an \
                 exported anchor is named `anchor/<kebab>` because that is the id the DSL \
                 resolves, so a stem the DSL could never name is refused here"
            ),
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
            Node::Mark { mark, body } => {
                if !is_kebab(&mark.anchor) {
                    return Err(ProgramError::BadAnchorName {
                        symbol: symbol.to_string(),
                        anchor: mark.anchor.clone(),
                    });
                }
                if let MarkAt::Offset { x, y, z } = &mark.at {
                    self.check_expr(symbol, x)?;
                    self.check_expr(symbol, y)?;
                    self.check_expr(symbol, z)?;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A guard operand is reported back to its author as text, so the text has
    /// to mean the same number the tree does. Integer division makes that a
    /// correctness question rather than a cosmetic one.
    #[test]
    fn an_expression_renders_as_the_number_it_means() {
        let x = || Expr::dim(DimRef::X);
        let p = |n: &str| Expr::param(n);

        assert_eq!(x().to_string(), "Dimension.X");
        assert_eq!(Expr::int(-3).to_string(), "-3");
        assert_eq!(p("strip_depth").to_string(), "strip_depth");

        // The zone guards' own shape: a left-nested chain stays flat.
        assert_eq!(
            Expr::dim(DimRef::Z)
                .arith(ArithOp::Sub, p("junction_run"))
                .arith(ArithOp::Sub, p("hearth_run"))
                .to_string(),
            "Dimension.Z - junction_run - hearth_run"
        );
        // A right operand of equal or lower binding keeps its brackets, because
        // `a - (b - c)` and `a - b - c` are different numbers...
        assert_eq!(
            x().arith(ArithOp::Sub, p("a").arith(ArithOp::Sub, p("b")))
                .to_string(),
            "Dimension.X - (a - b)"
        );
        // ...and so are `a * (b / c)` and `a * b / c` under floor division.
        assert_eq!(
            x().arith(ArithOp::Mul, p("a").arith(ArithOp::Div, p("b")))
                .to_string(),
            "Dimension.X * (a / b)"
        );
        // A weaker-binding operand keeps them on either side.
        assert_eq!(
            x().arith(ArithOp::Sub, Expr::int(1))
                .arith(ArithOp::Mul, Expr::int(2))
                .to_string(),
            "(Dimension.X - 1) * 2"
        );
        assert_eq!(
            x().arith(ArithOp::Mul, Expr::int(2))
                .arith(ArithOp::Sub, Expr::int(1))
                .to_string(),
            "Dimension.X * 2 - 1"
        );
        // `max`/`min` read as the two-argument forms they are, and need no
        // brackets of their own.
        assert_eq!(
            x().arith(ArithOp::Max, Expr::int(4))
                .arith(ArithOp::Add, Expr::int(1))
                .to_string(),
            "max(Dimension.X, 4) + 1"
        );
        assert_eq!(
            x().arith(ArithOp::Rem, Expr::int(2)).to_string(),
            "Dimension.X % 2"
        );
    }

    /// The named quantities an operand reads, which is what tells an author
    /// which knob moves a derived number.
    #[test]
    fn an_expression_lists_the_names_it_reads_once_each_in_reading_order() {
        let e = Expr::dim(DimRef::Z)
            .arith(ArithOp::Sub, Expr::param("run"))
            .arith(ArithOp::Sub, Expr::param("run"))
            .arith(ArithOp::Add, Expr::dim(DimRef::Smallest));
        assert_eq!(
            e.inputs(),
            vec![
                "Dimension.Z".to_string(),
                "run".to_string(),
                "Dimension.Smallest".to_string()
            ]
        );
        assert!(!e.is_leaf());

        for leaf in [Expr::int(2), Expr::param("run"), Expr::dim(DimRef::Y)] {
            assert!(leaf.is_leaf(), "{leaf}");
        }
        assert!(Expr::int(2).inputs().is_empty());
        assert_eq!(Expr::param("run").inputs(), vec!["run".to_string()]);
    }
}
