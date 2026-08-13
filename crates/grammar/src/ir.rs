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
use crate::geom::{Axis, Mirror, Orientation};
use crate::version::{
    BIND_SINCE, CONTRACT_SINCE, LATEST_PROGRAM_VERSION, MIRROR_SINCE, has_bind, has_contract,
    has_mirror, is_supported_version,
};

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
#[serde(tag = "expr", rename_all = "snake_case", deny_unknown_fields)]
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
#[serde(tag = "cond", rename_all = "snake_case", deny_unknown_fields)]
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
    /// The scope's frame is exactly this one — upstream's
    /// `orientation() == (0, 1, 2)` checks, which pick the correctly facing
    /// stair/door variant.
    ///
    /// The frame is the axis mapping **and** the reflection, and both are
    /// matched exactly: `mirror` defaults to no reflection, so a guard that does
    /// not mention it holds only in an unreflected scope. That is the strict
    /// reading on purpose. A `fill` writes the block state it was given
    /// verbatim, so a stair chosen for one frame is wrong in that frame's mirror
    /// image; a guard that matched both would place it silently, and this
    /// language has no silent wrong answers to spare.
    Orientation {
        /// World axis of the local `X`.
        x: Axis,
        /// World axis of the local `Y`.
        y: Axis,
        /// World axis of the local `Z`.
        z: Axis,
        /// Which local axes run backwards.
        #[serde(default, skip_serializing_if = "Mirror::is_none")]
        mirror: Mirror,
    },
}

impl Cond {
    /// `lhs <op> rhs`.
    pub fn cmp(lhs: Expr, op: CmpOp, rhs: Expr) -> Cond {
        Cond::Cmp { lhs, op, rhs }
    }

    /// The scope's frame is exactly this mapping, unreflected.
    pub fn orientation(x: Axis, y: Axis, z: Axis) -> Cond {
        Cond::Orientation {
            x,
            y,
            z,
            mirror: Mirror::NONE,
        }
    }

    /// The scope's frame is exactly this mapping, reflected as given.
    pub fn frame(x: Axis, y: Axis, z: Axis, mirror: Mirror) -> Cond {
        Cond::Orientation { x, y, z, mirror }
    }
}

// ---------------------------------------------------------------------------
// Splits and reorientation
// ---------------------------------------------------------------------------

/// One piece of a split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "size", rename_all = "snake_case", deny_unknown_fields)]
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

/// A (possibly partial) request for the child's frame: which parent axis each
/// local axis names, and which of them run backwards.
///
/// Unset axes are filled in by [`crate::orient::reorient`].
///
/// **`mirror` is relative to the source axis**, not to the world: it reverses
/// whichever direction the parent's chosen axis already ran in. So a rule need
/// not know its own handedness to reflect a child, and reflecting a reflected
/// frame gives the original back — which is what lets one rule be used at both
/// sites of a mirror pair, at any depth, without a copy.
///
/// The reflection lives here, on the frame request, rather than on `split` or on
/// a node of its own, because this struct **is** the language's statement about
/// a child's frame and it appears in two places — [`Node::Reorient`] and
/// [`Split::orient`]. A mirror keyed to either one of those would leave the
/// other with no surface, and the second site would then grow a bespoke field of
/// its own (CLAUDE.md: "a second bespoke field is the defect, not the fix").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Which of the child's local axes run backwards along the axis they name.
    #[serde(default, skip_serializing_if = "Mirror::is_none")]
    pub mirror: Mirror,
}

impl Reorient {
    /// No request at all: children keep the parent's frame entire.
    pub const KEEP: Reorient = Reorient {
        x: None,
        y: None,
        z: None,
        mirror: Mirror::NONE,
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

    /// Reverse one of the child's local axes.
    pub fn flip(mut self, axis: Axis) -> Reorient {
        self.mirror = self.mirror.and(axis);
        self
    }

    /// Set the whole reflection at once.
    pub fn mirror(mut self, mirror: Mirror) -> Reorient {
        self.mirror = mirror;
        self
    }
}

/// A subdivision of the current scope along one local axis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(tag = "at", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarkAt {
    /// The centre of the scope's **world** floor: lowest world `Y`, centred on
    /// world `X` and `Z`. Gravity is a world fact, so this one position ignores
    /// the scope's local axis names — an NPC stands on the floor however the
    /// rule chose to call its axes.
    FloorCenter,
    /// The scope's **local** minimum corner: the world minimum corner on
    /// unreflected axes, and the far end of every axis the frame reflects.
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
///
/// **The one document type in this module that is not a closed schema**, and it
/// cannot be one: `at` is `#[serde(flatten)]`, which serde cannot combine with
/// `deny_unknown_fields` — every flattened key reads as unknown, so the attribute
/// compiles and then refuses every well-formed mark. An unknown field on a mark
/// is therefore dropped rather than refused, and a future optional field here
/// would be silently droppable by an engine that predates it. What holds the line
/// instead is the version ledger of `grammar.md` §2e, which `tools/check-grammar-ir-compat.py`
/// enforces in both directions.
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
    /// The facing to declare. Omitted, it is derived from the scope's frame as
    /// the direction of *decreasing local `Z`*: `north`/`south` when local `Z`
    /// names world `Z`, `west`/`east` when it names world `X`, the second of
    /// each pair when the frame reflects local `Z`. A scope whose local `Z` is
    /// vertical has no cardinal facing to derive, and says so rather than
    /// guessing.
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
/// True for a contract region name: one or more kebab segments joined by `/`.
///
/// A region name is the *program's* private vocabulary, exactly like a rule
/// name, a parameter or a palette role — and like those three it takes an
/// include prefix, because a piece composed twice must not union its two rooms
/// into one region. So the grammar is a rule name's, not an anchor stem's: an
/// anchor is the campaign's id for a place and is therefore never qualified.
pub(crate) fn is_region_name(s: &str) -> bool {
    s != EXTERIOR && !s.is_empty() && s.split(SEGMENT).all(is_kebab)
}

/// What separates an include prefix from the name it qualifies.
const SEGMENT: char = '/';

pub(crate) fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

// ---------------------------------------------------------------------------
// The spatial contract
// ---------------------------------------------------------------------------

/// The endpoint name that means "outside the piece".
///
/// Reserved: a space may not be called this, because an edge endpoint is one
/// string and a space that took the name would make an exterior edge
/// unwritable.
pub const EXTERIOR: &str = "exterior";

/// How much of a space's boundary the author claims is solid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Envelope {
    /// Boundary solid on every face except a declared opening.
    Enclosed,
    /// Side faces solid; the top is deliberately open.
    OpenTop,
    /// No boundary claim.
    Open,
}

impl Envelope {
    /// The metadata keyword.
    pub fn as_str(self) -> &'static str {
        match self {
            Envelope::Enclosed => "enclosed",
            Envelope::OpenTop => "open_top",
            Envelope::Open => "open",
        }
    }
}

/// A space: one named region of the contract, with its envelope claim.
///
/// *Which cells* it covers is not here — that is the rules' business, stated at
/// the scope with [`Node::Claim`] and resolved per expansion. This says what the
/// named region **is**, once, however many rules claim boxes for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceDecl {
    /// The envelope claim.
    pub envelope: Envelope,
}

/// A `no_body` region: standable cells deliberately outside the walk.
///
/// **There is no kind field.** Which exemption a region qualifies for is a fact
/// about the blocks — walled off, anchored, exterior dressing — so it is
/// determined from them rather than chosen here. An author who could pick would
/// be picking which demand has to be met, and a choice between demands is only
/// ever as strong as the weakest one on offer. What the author does supply is
/// the reason, because no measurement recovers that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoBodyDecl {
    /// Why these cells are out of play, in the author's words.
    pub reason: String,
}

/// The bar of a `barred` edge: the region that stands in the way, and what
/// fills it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bar {
    /// A region name some rule claims.
    pub region: String,
    /// The palette role the bar is built from. A role bound to a mix is refused
    /// — a bar is one material, and "mostly a bar" is not a state a gate can be
    /// in.
    pub block: String,
}

/// How a body moves across an edge.
///
/// Each class carries exactly the fields it means, so a `bar` on a walk or a
/// `rise` on a sightline is not a thing an author can write and a check has to
/// catch afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case", deny_unknown_fields)]
pub enum EdgeClass {
    /// Level passage.
    Walk {
        /// Declared level change; 0 unless said otherwise.
        #[serde(default, skip_serializing_if = "is_default")]
        rise: i64,
        /// The opening, as a region name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via: Option<String>,
    },
    /// A climb, over a transit volume of its own.
    Stair {
        /// Declared level change.
        rise: i64,
        /// The transit volume — the treads belong to the edge, not to either
        /// end, which is why it is not optional here.
        via: String,
    },
    /// A one-way fall, `a` to `b`.
    Drop {
        /// Declared level change.
        rise: i64,
        /// The fall column, when the author names one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via: Option<String>,
    },
    /// A passage something stands in.
    Barred {
        /// Declared level change; 0 unless said otherwise.
        #[serde(default, skip_serializing_if = "is_default")]
        rise: i64,
        /// What stands in it.
        bar: Bar,
        /// The opening, when it is not the bar's own region.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via: Option<String>,
    },
    /// A hole a body cannot use — a window. No traversal claim, so no rise; the
    /// opening is the whole point of the declaration and is required.
    Vision {
        /// The opening.
        via: String,
    },
}

impl EdgeClass {
    /// The class keyword.
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeClass::Walk { .. } => "walk",
            EdgeClass::Stair { .. } => "stair",
            EdgeClass::Drop { .. } => "drop",
            EdgeClass::Barred { .. } => "barred",
            EdgeClass::Vision { .. } => "vision",
        }
    }

    /// The declared level change, where the class has one.
    pub fn rise(&self) -> Option<i64> {
        match self {
            EdgeClass::Walk { rise, .. }
            | EdgeClass::Stair { rise, .. }
            | EdgeClass::Drop { rise, .. }
            | EdgeClass::Barred { rise, .. } => Some(*rise),
            EdgeClass::Vision { .. } => None,
        }
    }

    /// The opening's region name, where one is declared.
    pub fn via(&self) -> Option<&str> {
        match self {
            EdgeClass::Walk { via, .. }
            | EdgeClass::Drop { via, .. }
            | EdgeClass::Barred { via, .. } => via.as_deref(),
            EdgeClass::Stair { via, .. } | EdgeClass::Vision { via } => Some(via),
        }
    }

    /// The bar, on the one class that has one.
    pub fn bar(&self) -> Option<&Bar> {
        match self {
            EdgeClass::Barred { bar, .. } => Some(bar),
            _ => None,
        }
    }
}

/// One declared way between two spaces, or between a space and the exterior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    /// A declared space name, or [`EXTERIOR`].
    pub a: String,
    /// A declared space name, or [`EXTERIOR`].
    pub b: String,
    /// The class, flattened so an edge reads as one object.
    #[serde(flatten)]
    pub class: EdgeClass,
}

/// The program's **spatial contract**: what its spaces are and how a body moves
/// between them.
///
/// The contract says *what*; [`Node::Claim`] says *where*. They are separated
/// because a name's meaning is one statement — an author writing a space out of
/// three rules states its envelope once, not three times that must agree — and
/// because a parametric program's boxes are not knowable until it is expanded,
/// while its intent is knowable from the document alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    /// The space a body enters at.
    pub entry: String,
    /// Named spaces.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub spaces: BTreeMap<String, SpaceDecl>,
    /// Named out-of-walk regions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub no_body: BTreeMap<String, NoBodyDecl>,
    /// The graph, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    /// The author's acknowledgement that this piece is mostly out-of-walk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_body_majority_ack: Option<String>,
}

impl Contract {
    /// A contract entered at `entry`, with nothing else declared yet.
    pub fn new(entry: &str) -> Contract {
        Contract {
            entry: entry.to_string(),
            spaces: BTreeMap::new(),
            no_body: BTreeMap::new(),
            edges: Vec::new(),
            no_body_majority_ack: None,
        }
    }

    /// Declare a space (builder form).
    pub fn space(mut self, name: &str, envelope: Envelope) -> Contract {
        self.spaces.insert(name.to_string(), SpaceDecl { envelope });
        self
    }

    /// Declare an out-of-walk region (builder form).
    pub fn no_body(mut self, name: &str, reason: &str) -> Contract {
        self.no_body.insert(
            name.to_string(),
            NoBodyDecl {
                reason: reason.to_string(),
            },
        );
        self
    }

    /// Declare an edge (builder form).
    pub fn edge(mut self, a: &str, b: &str, class: EdgeClass) -> Contract {
        self.edges.push(Edge {
            a: a.to_string(),
            b: b.to_string(),
            class,
        });
        self
    }

    /// Every region name the contract refers to, and what refers to it.
    fn referenced_regions(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for name in self.spaces.keys() {
            out.entry(name.clone())
                .or_insert_with(|| "space".to_string());
        }
        for name in self.no_body.keys() {
            out.entry(name.clone())
                .or_insert_with(|| "no_body".to_string());
        }
        for edge in &self.edges {
            let site = format!("edge {:?}->{:?}", edge.a, edge.b);
            if let Some(via) = edge.class.via() {
                out.entry(via.to_string()).or_insert_with(|| site.clone());
            }
            if let Some(bar) = edge.class.bar() {
                out.entry(bar.region.clone()).or_insert(site);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Rule bodies
// ---------------------------------------------------------------------------

/// One step of a rule body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
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
    /// Rebind names — parameters, palette roles, or both — over `body`.
    ///
    /// A scope is a box, a set of axis names, and a set of **value** names.
    /// [`Node::Split`] narrows the box, [`Node::Reorient`] renames the axes, and
    /// this renames the values. It is a wrapper rather than a field on
    /// [`Node::Call`] for the same reason `reorient` is: the capability belongs
    /// to the scope, not to the verb that first wanted it, so it reaches a split
    /// child, a `fill` or a `mark` and not only a call.
    ///
    /// [`Program::params`] and [`Program::palette`] are the outermost frame — a
    /// declaration *and* a default. A `bind` pushes a frame over it for the
    /// extent of `body`, and a name it does not mention keeps whatever the
    /// enclosing frame gave it. **The frame is inherited through calls**, which
    /// is what lets an argument survive a recursion that knows nothing about it.
    ///
    /// Both maps are evaluated in the **enclosing** scope, before the frame is
    /// pushed, and all of them at once: `{"a": param b, "b": param a}` swaps the
    /// two rather than chaining them.
    ///
    /// A `bind` may only name a parameter or role the program itself declares
    /// ([`ProgramError::UnknownBinding`]), so a misspelt name is refused where it
    /// was written instead of quietly expanding the default.
    Bind {
        /// Parameter overrides, each an integer expression over the enclosing
        /// scope. Read back by `{"expr": "param"}` inside `body`.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        params: BTreeMap<String, Expr>,
        /// Palette overrides, each resolved against the enclosing frame. Read
        /// back by a `fill` naming the role inside `body`.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        palette: BTreeMap<String, Material>,
        /// What to expand under them.
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
    /// Claim this scope's box for a named contract region, then expand `body`.
    ///
    /// A wrapper for the reason [`Node::Mark`] is one: a rule body is one node,
    /// so this way a claim sits on any piece of any split and names exactly the
    /// box that piece owns. `body` is [`Node::Skip`] when the claim is all that
    /// is wanted.
    ///
    /// It writes no blocks and draws nothing from the seeded stream. Several
    /// claims of one name **union**: a room whose cross-section is not a box is
    /// described by the boxes it is actually built from, which is the only
    /// description a rule can give without recomputing the shape by hand.
    ///
    /// What the region *is* — a space and its envelope, an out-of-walk region
    /// and its kind, an edge's opening — is [`Contract`]'s statement, made once
    /// per name. This node carries no meaning of its own on purpose: the same
    /// mechanism has to serve a space, a transit volume and a bar region, and a
    /// second declaration node per use is how the third one ends up with no
    /// surface at all.
    Claim {
        /// The region name, kebab-case; [`Contract`] must classify it.
        region: String,
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

    /// Rebind palette roles over this node.
    pub fn with_roles<'r>(self, palette: impl IntoIterator<Item = (&'r str, Material)>) -> Node {
        Node::Bind {
            params: BTreeMap::new(),
            palette: palette
                .into_iter()
                .map(|(role, m)| (role.to_string(), m))
                .collect(),
            body: Box::new(self),
        }
    }

    /// Rebind parameters over this node.
    pub fn with_params<'p>(self, params: impl IntoIterator<Item = (&'p str, Expr)>) -> Node {
        Node::Bind {
            params: params
                .into_iter()
                .map(|(name, e)| (name.to_string(), e))
                .collect(),
            palette: BTreeMap::new(),
            body: Box::new(self),
        }
    }
}

/// One alternative of a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct Program {
    /// **The document's own version** (ADR-0018 §7), not the crate's.
    ///
    /// Required, and an unrecognised one is refused rather than parsed
    /// best-effort: an engine that quietly ignored the parts of a newer document
    /// it did not understand would emit a world that is wrong in silence. A
    /// construct introduced later than this version is refused at
    /// [`Program::validate`], which is what lets an older document keep
    /// compiling to the same bytes forever. See [`crate::version`].
    pub version: String,
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
    /// The spatial contract, when the program declares one. Absent means the
    /// program makes no spatial claim at all, which is a different statement
    /// from an empty contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
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
    /// A `bind` binds nothing, so it is a wrapper with no effect.
    EmptyBind {
        /// The rule.
        symbol: String,
    },
    /// A `bind` names a parameter or role the program does not declare.
    UnknownBinding {
        /// The rule.
        symbol: String,
        /// `"parameter"` or `"palette role"`.
        kind: &'static str,
        /// The name as written.
        name: String,
    },
    /// A [`Cond::Orientation`] guard names an axis mapping that is not a
    /// permutation, so no scope can ever satisfy it.
    OrientationCondNotAPermutation {
        /// The rule.
        symbol: String,
        /// The mapping as written, local `X`/`Y`/`Z` to world axis.
        axes: [Axis; 3],
    },
    /// The document declares a version this engine does not accept — one the
    /// ledger does not name at all, or one it reserves for a surface this engine
    /// does not implement (`crates/grammar/src/version.rs`).
    UnsupportedVersion {
        /// The version as written.
        version: String,
    },
    /// The document writes a construct newer than the version it declares.
    FencedConstruct {
        /// What was written.
        construct: &'static str,
        /// The version that introduced it.
        since: &'static str,
        /// The version the document declares.
        declared: String,
        /// Where it was written — a rule name, or `"contract"`.
        written_by: String,
    },
    /// A region name is not kebab-case, or is the reserved exterior name.
    BadRegionName {
        /// Where it was written — a rule name, or `"contract"`.
        written_by: String,
        /// The name as written.
        region: String,
    },
    /// The contract names a region no rule ever claims, so nothing can resolve
    /// it to a box.
    UnclaimedRegion {
        /// The region.
        region: String,
        /// What refers to it.
        referenced_by: String,
    },
    /// A rule claims a region the contract never classifies, so the boxes it
    /// resolves would belong to nothing.
    UnclassifiedRegion {
        /// The region.
        region: String,
        /// The rule that claims it.
        declared_by: String,
    },
    /// One region name is classified twice — a space cannot also be an
    /// out-of-walk region or an edge's own volume.
    RegionClassifiedTwice {
        /// The region.
        region: String,
        /// The first classification.
        first: String,
        /// The second.
        second: String,
    },
    /// `entry`, or an edge endpoint, names something that is not a declared
    /// space.
    UnknownSpace {
        /// The name as written.
        space: String,
        /// What named it.
        referenced_by: String,
    },
    /// A bar names a palette role bound to a weighted mix. A bar is one
    /// material: "mostly a bar" is not a state a gate can be in.
    BarBlockIsAMix {
        /// The role.
        role: String,
        /// The bar's region.
        region: String,
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
            ProgramError::EmptyBind { symbol } => write!(
                f,
                "rule {symbol:?} has a `bind` that binds neither a parameter nor a palette role, \
                 so it renames nothing and its body would expand exactly as it does without it"
            ),
            ProgramError::UnknownBinding { symbol, kind, name } => write!(
                f,
                "rule {symbol:?} binds the {kind} {name:?}, which this program does not declare — \
                 a `bind` overrides a name for the extent of its body, so it can only name \
                 something the program already declares; otherwise a misspelt binding would \
                 silently expand the default"
            ),
            ProgramError::OrientationCondNotAPermutation { symbol, axes } => write!(
                f,
                "rule {symbol:?} guards on the orientation {axes:?}, which is not a permutation \
                 of the three world axes — no scope can ever match it, so the alternative is \
                 dead code"
            ),
            ProgramError::UnsupportedVersion { version } => {
                let accepted: Vec<&str> = crate::version::accepted_versions().collect();
                match crate::version::reserved_for(version) {
                    Some(anchor) => write!(
                        f,
                        "this program declares version {version:?}, a version the document \
                         format has but this engine does not implement — the ledger reserves \
                         it for the surface `{anchor}` introduces. It is refused rather than \
                         built with that surface silently dropped. This engine accepts \
                         {accepted:?}"
                    ),
                    None => write!(
                        f,
                        "this program declares version {version:?}, which this engine does not \
                         know; it accepts {accepted:?}. An unknown version is refused rather \
                         than parsed for the parts that look familiar, because a document whose \
                         newer half was skipped compiles green and builds the wrong world"
                    ),
                }
            }
            ProgramError::FencedConstruct {
                construct,
                since,
                declared,
                written_by,
            } => write!(
                f,
                "{written_by} writes {construct}, which version {since} introduced, but this \
                 program declares version {declared}. Raise the program's `version` to write it; \
                 leaving it where it is keeps this document compiling exactly as it always has"
            ),
            ProgramError::BadRegionName { written_by, region } => write!(
                f,
                "{written_by} names the contract region {region:?}, which is not a usable name: a \
                 region name is one or more kebab-case segments joined by {SEGMENT:?} (a \
                 composed piece's regions take their include prefix, as its rules do), and \
                 {EXTERIOR:?} is reserved for the endpoint that means outside the piece"
            ),
            ProgramError::UnclaimedRegion {
                region,
                referenced_by,
            } => write!(
                f,
                "the contract's {referenced_by} names the region {region:?}, which no rule claims; \
                 a region with no `claim` can never resolve to a box, so the declaration would \
                 describe nothing"
            ),
            ProgramError::UnclassifiedRegion {
                region,
                declared_by,
            } => write!(
                f,
                "rule {declared_by:?} claims the region {region:?}, which the contract never \
                 classifies as a space, an out-of-walk region or an edge's own volume. A claim \
                 the contract does not name resolves boxes that belong to nothing"
            ),
            ProgramError::RegionClassifiedTwice {
                region,
                first,
                second,
            } => write!(
                f,
                "the region {region:?} is classified both as {first} and as {second}; one name is \
                 one thing, and a region that is two would satisfy each obligation under the \
                 other's rules"
            ),
            ProgramError::UnknownSpace {
                space,
                referenced_by,
            } => write!(
                f,
                "the contract's {referenced_by} names {space:?}, which is not a declared space \
                 (an endpoint is a declared space or {EXTERIOR:?})"
            ),
            ProgramError::BarBlockIsAMix { role, region } => write!(
                f,
                "the bar over region {region:?} is built from the palette role {role:?}, which is \
                 bound to a weighted mix; a bar is one material, and a gate that is mostly a bar \
                 is not a state anything can be in"
            ),
        }
    }
}

impl std::error::Error for ProgramError {}

impl Program {
    /// An empty program with the given name and start rule, at the latest
    /// document version.
    pub fn new(name: &str, start: &str) -> Program {
        Program {
            version: LATEST_PROGRAM_VERSION.to_string(),
            name: name.to_string(),
            start: start.to_string(),
            params: BTreeMap::new(),
            palette: BTreeMap::new(),
            rules: BTreeMap::new(),
            contract: None,
        }
    }

    /// Declare the document version (builder form) — what a program written
    /// against an older surface says, and the only way to write one here.
    pub fn at_version(mut self, version: &str) -> Program {
        self.version = version.to_string();
        self
    }

    /// Declare the spatial contract (builder form).
    pub fn contract(mut self, contract: Contract) -> Program {
        self.contract = Some(contract);
        self
    }

    /// Every contract region name the rules claim, and the first rule that
    /// claims each.
    ///
    /// Deliberately syntactic, like [`crate::compose::declared_anchors`]: a
    /// claim under a guard that never holds in some box is still a claim the
    /// document makes, and the over-approximation only ever makes the
    /// reference checks below more permissive.
    pub fn claimed_regions(&self) -> BTreeMap<String, String> {
        fn walk(symbol: &str, node: &Node, into: &mut BTreeMap<String, String>) {
            match node {
                Node::Claim { region, body } => {
                    into.entry(region.clone())
                        .or_insert_with(|| symbol.to_string());
                    walk(symbol, body, into);
                }
                Node::Mark { body, .. } | Node::Reorient { body, .. } | Node::Bind { body, .. } => {
                    walk(symbol, body, into)
                }
                Node::Split(split) => split.children.iter().for_each(|c| walk(symbol, c, into)),
                Node::Void | Node::Skip | Node::Fill { .. } | Node::Call { .. } => {}
            }
        }
        let mut found = BTreeMap::new();
        for (symbol, alts) in &self.rules {
            for alt in alts {
                walk(symbol, &alt.body, &mut found);
            }
        }
        found
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
        if !is_supported_version(&self.version) {
            return Err(ProgramError::UnsupportedVersion {
                version: self.version.clone(),
            });
        }
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
        self.check_contract()?;
        Ok(())
    }

    /// The contract's own references: every name resolves, in both directions,
    /// and one name is one thing.
    ///
    /// Reference integrity only. Nothing here looks at a box, a cell or a
    /// block — whether the declared geometry is *true* is a question about the
    /// expanded model, and it belongs to the checker that reads one.
    fn check_contract(&self) -> Result<(), ProgramError> {
        let claimed = self.claimed_regions();
        if !has_contract(&self.version) {
            if let Some((region, symbol)) = claimed.iter().next() {
                return Err(ProgramError::FencedConstruct {
                    construct: "a `claim` node",
                    since: CONTRACT_SINCE,
                    declared: self.version.clone(),
                    written_by: format!("rule {symbol:?} (region {region:?})"),
                });
            }
            if self.contract.is_some() {
                return Err(ProgramError::FencedConstruct {
                    construct: "a `contract` block",
                    since: CONTRACT_SINCE,
                    declared: self.version.clone(),
                    written_by: "the program".to_string(),
                });
            }
            return Ok(());
        }
        for (region, symbol) in &claimed {
            if !is_region_name(region) {
                return Err(ProgramError::BadRegionName {
                    written_by: format!("rule {symbol:?}"),
                    region: region.clone(),
                });
            }
        }
        let Some(contract) = &self.contract else {
            if let Some((region, symbol)) = claimed.iter().next() {
                return Err(ProgramError::UnclassifiedRegion {
                    region: region.clone(),
                    declared_by: symbol.clone(),
                });
            }
            return Ok(());
        };

        // One name, one thing — checked before anything else reads the maps, so
        // a doubly-classified region cannot resolve under whichever reading
        // happens to be consulted first.
        for name in contract.no_body.keys() {
            if contract.spaces.contains_key(name) {
                return Err(ProgramError::RegionClassifiedTwice {
                    region: name.clone(),
                    first: "a space".to_string(),
                    second: "an out-of-walk region".to_string(),
                });
            }
        }

        for name in contract.spaces.keys().chain(contract.no_body.keys()) {
            if !is_region_name(name) {
                return Err(ProgramError::BadRegionName {
                    written_by: "the contract".to_string(),
                    region: name.clone(),
                });
            }
        }

        // An edge's own volumes are regions in their own right: a name that is
        // also a space would let a transit volume answer a space's obligations.
        for edge in &contract.edges {
            let site = format!("edge {:?}->{:?}", edge.a, edge.b);
            for endpoint in [&edge.a, &edge.b] {
                if endpoint != EXTERIOR && !contract.spaces.contains_key(endpoint) {
                    return Err(ProgramError::UnknownSpace {
                        space: endpoint.clone(),
                        referenced_by: site.clone(),
                    });
                }
            }
            let mut own = Vec::new();
            if let Some(via) = edge.class.via() {
                own.push(via.to_string());
            }
            if let Some(bar) = edge.class.bar() {
                own.push(bar.region.clone());
                match self.palette.get(&bar.block) {
                    None => {
                        return Err(ProgramError::UnknownRole {
                            role: bar.block.clone(),
                            referenced_by: site.clone(),
                        });
                    }
                    Some(Paint::Mix(_)) => {
                        return Err(ProgramError::BarBlockIsAMix {
                            role: bar.block.clone(),
                            region: bar.region.clone(),
                        });
                    }
                    Some(Paint::Block(_)) => {}
                }
            }
            for name in own {
                if !is_region_name(&name) {
                    return Err(ProgramError::BadRegionName {
                        written_by: format!("the contract's {site}"),
                        region: name,
                    });
                }
                if contract.spaces.contains_key(&name) {
                    return Err(ProgramError::RegionClassifiedTwice {
                        region: name,
                        first: "a space".to_string(),
                        second: format!("{site}'s own volume"),
                    });
                }
                if contract.no_body.contains_key(&name) {
                    return Err(ProgramError::RegionClassifiedTwice {
                        region: name,
                        first: "an out-of-walk region".to_string(),
                        second: format!("{site}'s own volume"),
                    });
                }
            }
        }

        if !contract.spaces.contains_key(&contract.entry) {
            return Err(ProgramError::UnknownSpace {
                space: contract.entry.clone(),
                referenced_by: "`entry`".to_string(),
            });
        }

        // Both directions: a name the contract uses that no rule claims cannot
        // resolve to a box, and a name a rule claims that the contract never
        // classifies resolves to boxes that belong to nothing.
        let referenced = contract.referenced_regions();
        for (region, referenced_by) in &referenced {
            if !claimed.contains_key(region) {
                return Err(ProgramError::UnclaimedRegion {
                    region: region.clone(),
                    referenced_by: referenced_by.clone(),
                });
            }
        }
        for (region, declared_by) in &claimed {
            if !referenced.contains_key(region) {
                return Err(ProgramError::UnclassifiedRegion {
                    region: region.clone(),
                    declared_by: declared_by.clone(),
                });
            }
        }
        Ok(())
    }

    fn check_node(&self, symbol: &str, node: &Node, in_split: bool) -> Result<(), ProgramError> {
        match node {
            Node::Void | Node::Skip => Ok(()),
            Node::Fill { material } => self.check_material(symbol, material),
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
            Node::Bind {
                params,
                palette,
                body,
            } => {
                // The fence before the shape checks, as `claim`'s is: a document
                // that may not write the construct at all is answered with the
                // version it declares, never with a complaint about how the
                // construct it may not write is spelled.
                //
                // The guard reads the two halves rather than the variant alone,
                // because the two halves are what the §2e ledger names and what
                // an older engine would have to honour — a `bind` IS its `params`
                // and its `palette`. One that binds neither carries no surface at
                // all and is `EmptyBind` at every version.
                if !has_bind(&self.version) && !(params.is_empty() && palette.is_empty()) {
                    return Err(ProgramError::FencedConstruct {
                        construct: "a `bind` node",
                        since: BIND_SINCE,
                        declared: self.version.clone(),
                        written_by: format!("rule {symbol:?}"),
                    });
                }
                if params.is_empty() && palette.is_empty() {
                    return Err(ProgramError::EmptyBind {
                        symbol: symbol.to_string(),
                    });
                }
                // A binding may only override a name the program declares. The
                // reason is `set_param`'s: a typo that quietly leaves the
                // default in place is the failure that stays green.
                for (name, value) in params {
                    if !self.params.contains_key(name) {
                        return Err(ProgramError::UnknownBinding {
                            symbol: symbol.to_string(),
                            kind: "parameter",
                            name: name.clone(),
                        });
                    }
                    self.check_expr(symbol, value)?;
                }
                for (role, material) in palette {
                    if !self.palette.contains_key(role) {
                        return Err(ProgramError::UnknownBinding {
                            symbol: symbol.to_string(),
                            kind: "palette role",
                            name: role.clone(),
                        });
                    }
                    self.check_material(symbol, material)?;
                }
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
            // The claim's own name, and its place in the contract, are checked
            // once over the whole program in `check_contract`: a claim is a
            // reference to a contract this walk has not read yet.
            Node::Claim { body, .. } => self.check_node(symbol, body, in_split),
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

    /// A material names a bound role, or is an inline paint with usable weights.
    ///
    /// Shared by `fill` and by a `bind`'s palette frame, so the two cannot drift
    /// into checking different things about the same value.
    fn check_material(&self, symbol: &str, material: &Material) -> Result<(), ProgramError> {
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

    fn check_orient(
        &self,
        symbol: &str,
        orient: &Reorient,
        in_split: bool,
    ) -> Result<(), ProgramError> {
        // Destructured with no `..` so a new field of a frame request has to be
        // considered here rather than skipped. `mirror` needs no *semantic*
        // check — every one of the eight reflections is a frame a scope can
        // really be in, so a reflection request is always satisfiable and never
        // dead code — but it does need the version fence: it is a
        // `#[serde(default)]` field, so an engine that predates it drops it
        // silently and builds the unreflected shape.
        let Reorient { x, y, z, mirror } = orient;
        if !mirror.is_none() && !has_mirror(&self.version) {
            return Err(ProgramError::FencedConstruct {
                construct: "a reflected frame (`reorient.mirror`)",
                since: MIRROR_SINCE,
                declared: self.version.clone(),
                written_by: format!("rule {symbol:?}"),
            });
        }
        for spec in [*x, *y, *z].into_iter().flatten() {
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
            // A reflection is legal on any mapping — every one of the eight
            // reflections of a permutation is a frame a scope can really be in —
            // so only the permutation half is *semantically* checkable here. The
            // other half is the version fence, for the reason `check_orient`
            // gives: a guard that names a reflection means something different
            // on an engine that drops the field, and it is exactly the guard an
            // author writes to keep a stair from being placed backwards.
            Cond::Orientation { x, y, z, mirror } => {
                if !mirror.is_none() && !has_mirror(&self.version) {
                    return Err(ProgramError::FencedConstruct {
                        construct: "a reflected frame guard (`orientation.mirror`)",
                        since: MIRROR_SINCE,
                        declared: self.version.clone(),
                        written_by: format!("rule {symbol:?}"),
                    });
                }
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
