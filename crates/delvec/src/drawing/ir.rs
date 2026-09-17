//! **The drawing document**: a place's detail as an ordered list of solids
//! (spec-0072 §2–§4, ADR-0030 §§1–4).
//!
//! A drawing is one JSON document per place, executed in the place's box in the
//! box's own frame — `x` east, `y` up, `z` south, the origin at the box's
//! minimum corner, every coordinate a cell. **A later operation overwrites an
//! earlier one**, which is what a designer does and what a box-split `Program`
//! cannot do.
//!
//! # The types are shared, not copied
//!
//! [`Expr`], [`Cond`], [`States`], [`Mark`] and [`Contract`] are the **program
//! document's own** types (`crate::grammar::ir`). A drawing uses them by
//! reference, so the expression algebra, the guard algebra, the weighted paint,
//! the anchor and the spatial contract have one definition apiece and one
//! checker apiece. This module defines no type of any of those names, and
//! `drawing_defines_no_shared_type` fails if it ever does.
//!
//! The one shared type a drawing does **not** reach is
//! [`Paint`](crate::grammar::ir::Paint). `Paint` is exactly the world-frame /
//! local-frame distinction, and a drawing has neither spelling to choose
//! between: **every state in a drawing is written in the frame of the scope
//! that paints it** (§5.2), so the palette binds [`States`] — one block state or
//! a weighted list — and the executor resolves it through the frame. Offering
//! `Paint` would offer a `local` wrapper that means nothing and a world-frame
//! spelling that no operation could honour.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::grammar::ir::{Cond, Contract, Expr, Mark, States};

/// The role a solid paints to write nothing — the reserved name that clears.
///
/// `clear` is not an operation (ADR-0030 §2 lists one and spec-0072 §3.1
/// strikes it): a verb that clears only boxes leaves a round room with nothing
/// to clear it, so the capability belongs to the act of painting and reaches
/// every solid.
pub const AIR: &str = "air";

/// The role a cell answers to in [`Solid::where_roles`] when a `grammar`
/// operation wrote it.
///
/// Reserved as a *current* role only. Nothing paints it: a `grammar` operation
/// writes the program's own states, and a solid naming it as a role to paint is
/// refused, because the drawing's palette does not bind it.
pub const GRAMMAR: &str = "grammar";

/// An integer position or size: a JSON integer, or an [`Expr`] of the one
/// algebra.
///
/// A bare integer is one more spelling of `{"expr": "int", "value": n}` at a
/// drawing's own fields, and nowhere else: inside an `Expr` the operands are the
/// algebra's own tagged form. There is no second algebra, no string syntax and
/// no float (ADR-0006 forbids the last outright).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Int {
    /// A literal, written bare.
    Literal(i64),
    /// An expression over literals, parameters, the scope's extents and
    /// `+ − × ÷ % max min`.
    Expr(Expr),
}

impl Int {
    /// A literal.
    pub fn at(value: i64) -> Int {
        Int::Literal(value)
    }

    /// The expression this is, whichever spelling was written.
    pub fn expr(&self) -> Expr {
        match self {
            Int::Literal(value) => Expr::Int { value: *value },
            Int::Expr(e) => e.clone(),
        }
    }
}

/// A cell or an extent, as three [`Int`]s in the scope's own `x`, `y`, `z`.
pub type Vec3 = [Int; 3];

/// A literal cell, the spelling a Rust caller wants.
pub fn cell(x: i64, y: i64, z: i64) -> Vec3 {
    [Int::at(x), Int::at(y), Int::at(z)]
}

/// **A face of an operation's box** — the engine's one face vocabulary
/// ([`delvewright_dsl::siteplan::Face`]), the same six names a prefab's face
/// contract and a site plan's seams are written with.
///
/// Not a type of this module's own: a second enum of six words would be a
/// second vocabulary, and the two would disagree the first time one of them
/// gained a name. The low and high faces of `x`, `y`, `z` are `west`/`east`,
/// `down`/`up`, `north`/`south`.
pub use delvewright_dsl::siteplan::Face;

/// **A world axis** — the engine's own ([`delvewright_dsl::siteplan::Axis`]).
///
/// What a `cylinder` calls straight and what a fitted `repeat` runs along.
pub use delvewright_dsl::siteplan::Axis;

/// **A horizontal axis** — the engine's own
/// ([`delvewright_dsl::siteplan::PlanAxis`]), and the only kind a drawing
/// turns, mirrors or tapers along.
///
/// **The vertical never moves: a drawing has gravity.** A frame that named a
/// horizontal world axis as its local `y` would leave every `half`, every yaw
/// and every stair with nothing to mean, and a `mirror` about the vertical
/// would turn a building upside down. One type for `prism.taper`,
/// `use.mirror` and `mirror.axis`, so the restriction is stated once and
/// refused by the schema rather than by three checks — and it is the type a
/// site plan already uses to say *a box is a footprint, so its extent has no
/// `y` to ask about*.
pub use delvewright_dsl::siteplan::PlanAxis as Horizontal;

/// **Which axis a face lies on, and which end of it** — derived from the face's
/// own unit vector rather than tabulated beside it, so a face and its axis
/// cannot disagree.
pub fn face_axis(face: Face) -> usize {
    let v = face.vector();
    (0..3)
        .find(|&a| v[a] != 0)
        .expect("a face points somewhere")
}

/// True for the high face of its axis.
pub fn face_is_high(face: Face) -> bool {
    face.vector()[face_axis(face)] > 0
}

/// Every face, in the order the box rule reads them: the low and the high face
/// of `x`, then of `y`, then of `z`.
pub const FACES: [Face; 6] = [
    Face::West,
    Face::East,
    Face::Down,
    Face::Up,
    Face::North,
    Face::South,
];

/// Which faces of a [`Op::Prism`]'s tapering axis step in as the courses rise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sides {
    /// Both: the gable.
    #[default]
    Both,
    /// The low face only: the wedge, the buttress, the lean-to.
    Low,
    /// The high face only.
    High,
}

/// The course a [`Op::Pyramid`] is cut in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    /// Rectangular courses: the spire, the batter.
    #[default]
    Square,
    /// Each course the ellipse inscribed in its own rectangle: the cone.
    Round,
}

/// Where a fitted [`Op::Repeat`] puts the cells its items do not cover.
///
/// **Required**, and the words are `split`'s `rounding` words, because it is the
/// same question asked of the same kind of run. There is no default: a run whose
/// leftover the author has not placed is a run whose ends nobody decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Remainder {
    /// There must be none.
    Exact,
    /// Before the first item.
    Start,
    /// After the last item.
    End,
    /// Split across both ends, the lower half first.
    Middle,
}

/// A named, parameterised list of operations with its own local box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Define {
    /// Integer knobs, each a declaration **and** a default, read by
    /// `{"expr": "param"}` inside `body`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, i64>,
    /// The palette roles this body paints, in its own vocabulary. Every one is
    /// bound at every `use`: a role has no default the way a parameter does, so
    /// an unbound one names nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// The operations.
    pub body: Vec<Op>,
}

/// One operation. Thirteen, and the tag is the verb.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// A box, solid or restricted to a thickness at named faces.
    Box(BoxOp),
    /// An elliptic cylinder about one axis; one cell long it is the disc,
    /// halved it is the apse and the barrel vault.
    Cylinder(CylinderOp),
    /// An ellipsoid; halved it is the dome.
    Sphere(SphereOp),
    /// A box tapering along one horizontal axis to a ridge: the gable, and
    /// one-sided the wedge.
    Prism(PrismOp),
    /// A box tapering on all four horizontal faces to a point or a cap: the
    /// spire, the batter, and round the cone.
    Pyramid(PyramidOp),
    /// A line of cells from one end point to another, stamped with a brush.
    Line(LineOp),
    /// A box its body's coordinates are local to.
    Scope(ScopeOp),
    /// An instance of a [`Define`], at a position, a turn and a mirror.
    Use(UseOp),
    /// A body stamped along a step vector, or fitted along an axis.
    Repeat(RepeatOp),
    /// A body run, then run again reflected across the centre plane of the
    /// operation's box.
    Mirror(MirrorOp),
    /// A grammar program expanded into the operation's box at a literal seed.
    Grammar(GrammarOp),
    /// An anchor the piece offers the campaign.
    Mark(MarkOp),
    /// A named box of the spatial contract, and what stands in it.
    Claim(ClaimOp),
}

/// The name each operation answers to in a refusal and in the dead-operation
/// report.
impl Op {
    /// The verb, as the document spells it.
    pub fn verb(&self) -> &'static str {
        match self {
            Op::Box(_) => "box",
            Op::Cylinder(_) => "cylinder",
            Op::Sphere(_) => "sphere",
            Op::Prism(_) => "prism",
            Op::Pyramid(_) => "pyramid",
            Op::Line(_) => "line",
            Op::Scope(_) => "scope",
            Op::Use(_) => "use",
            Op::Repeat(_) => "repeat",
            Op::Mirror(_) => "mirror",
            Op::Grammar(_) => "grammar",
            Op::Mark(_) => "mark",
            Op::Claim(_) => "claim",
        }
    }

    /// Every verb, in the order the union declares them — what the schema's
    /// `oneOf` names and what the surface census counts.
    pub const VERBS: [&'static str; 13] = [
        "box", "cylinder", "sphere", "prism", "pyramid", "line", "scope", "use", "repeat",
        "mirror", "grammar", "mark", "claim",
    ];

    /// The guard, where the operation carries one.
    pub fn when(&self) -> Option<&Cond> {
        match self {
            Op::Box(o) => o.when.as_ref(),
            Op::Cylinder(o) => o.when.as_ref(),
            Op::Sphere(o) => o.when.as_ref(),
            Op::Prism(o) => o.when.as_ref(),
            Op::Pyramid(o) => o.when.as_ref(),
            Op::Line(o) => o.when.as_ref(),
            Op::Scope(o) => o.when.as_ref(),
            Op::Use(o) => o.when.as_ref(),
            Op::Repeat(o) => o.when.as_ref(),
            Op::Mirror(o) => o.when.as_ref(),
            Op::Grammar(o) => o.when.as_ref(),
            Op::Mark(o) => o.when.as_ref(),
            Op::Claim(o) => o.when.as_ref(),
        }
    }

    /// The operation's box in its scope, where it declares one. `from` omitted
    /// and `to` omitted is the whole scope; `to` omitted alone is one cell.
    pub fn extent(&self) -> (Option<&Vec3>, Option<&Vec3>) {
        match self {
            Op::Box(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Cylinder(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Sphere(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Prism(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Pyramid(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Line(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Scope(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Use(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Repeat(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Mirror(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Grammar(o) => (o.from.as_ref(), o.to.as_ref()),
            Op::Mark(_) => (None, None),
            Op::Claim(o) => (o.from.as_ref(), o.to.as_ref()),
        }
    }

    /// The role a solid paints, and the roles it reads.
    pub fn solid(&self) -> Option<Solid<'_>> {
        Some(match self {
            Op::Box(o) => Solid::new(&o.role, &o.where_roles),
            Op::Cylinder(o) => Solid::new(&o.role, &o.where_roles),
            Op::Sphere(o) => Solid::new(&o.role, &o.where_roles),
            Op::Prism(o) => Solid::new(&o.role, &o.where_roles),
            Op::Pyramid(o) => Solid::new(&o.role, &o.where_roles),
            Op::Line(o) => Solid::new(&o.role, &o.where_roles),
            _ => return None,
        })
    }

    /// The operations this one carries, where it carries any.
    pub fn body(&self) -> &[Op] {
        match self {
            Op::Scope(o) => &o.body,
            Op::Repeat(o) => &o.body,
            Op::Mirror(o) => &o.body,
            Op::Claim(o) => &o.body,
            _ => &[],
        }
    }
}

/// What a solid paints, and what it is allowed to paint over.
#[derive(Debug, Clone, Copy)]
pub struct Solid<'o> {
    /// The palette role, or the reserved role `air`.
    pub role: &'o str,
    /// The roles whose cells this operation may overwrite. Empty is every cell.
    pub where_roles: &'o [String],
}

impl<'o> Solid<'o> {
    fn new(role: &'o str, where_roles: &'o [String]) -> Solid<'o> {
        Solid { role, where_roles }
    }
}

// ---------------------------------------------------------------------------
// The six solids
// ---------------------------------------------------------------------------
//
// The five fields every solid carries are written out on each of the six rather
// than flattened in from a shared struct, because `serde`'s `flatten` and
// `deny_unknown_fields` are mutually exclusive: a flattened common part would
// buy one definition of five fields and pay for it with six operations that
// silently accept a misspelt key. `every_solid_carries_the_five_common_fields`
// holds them equal from the exported schema, which is where a reader would
// notice the difference.

/// `box` — every cell of the box, or a thickness at named faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoxOp {
    /// The palette role, or `air`.
    pub role: String,
    /// The low corner, inclusive. Omitted with `to`: the whole scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive. Omitted: one cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// Overwrite only cells whose **current** role is listed.
    #[serde(default, rename = "where", skip_serializing_if = "Vec::is_empty")]
    pub where_roles: Vec<String>,
    /// A guard over the scope. False: the operation does nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader. The engine never reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// A non-empty subset of the six faces. Absent: every cell of the box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<Vec<Face>>,
    /// Thickness at each listed face, default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<Int>,
}

/// `cylinder` — the ellipse of §3.3 swept along `axis`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CylinderOp {
    /// The palette role, or `air`.
    pub role: String,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// Overwrite only cells whose current role is listed.
    #[serde(default, rename = "where", skip_serializing_if = "Vec::is_empty")]
    pub where_roles: Vec<String>,
    /// A guard over the scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The straight axis; the other two are round.
    pub axis: Axis,
    /// A face on a round axis: the solid is the half whose cut plane lies on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flat: Option<Face>,
    /// Wall thickness. Absent: solid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<Int>,
}

/// `sphere` — the ellipsoid of §3.3, three round axes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SphereOp {
    /// The palette role, or `air`.
    pub role: String,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// Overwrite only cells whose current role is listed.
    #[serde(default, rename = "where", skip_serializing_if = "Vec::is_empty")]
    pub where_roles: Vec<String>,
    /// A guard over the scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// A face of the box: the solid is the half whose cut plane lies on it —
    /// the dome, on the box's floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flat: Option<Face>,
    /// Shell thickness. Absent: solid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<Int>,
}

/// `prism` — courses that step in along one horizontal axis as they rise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrismOp {
    /// The palette role, or `air`.
    pub role: String,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// Overwrite only cells whose current role is listed.
    #[serde(default, rename = "where", skip_serializing_if = "Vec::is_empty")]
    pub where_roles: Vec<String>,
    /// A guard over the scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The horizontal axis the span narrows along; the ridge runs along the
    /// other.
    pub taper: Horizontal,
    /// Which faces of `taper` step in.
    #[serde(default, skip_serializing_if = "is_default")]
    pub sides: Sides,
    /// Courses per step, default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rise: Option<Int>,
    /// Cells stepped in per `rise` courses, default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Int>,
    /// Keep only the surface: one ring per course.
    #[serde(default, skip_serializing_if = "is_false")]
    pub skin: bool,
}

/// `pyramid` — courses that step in on all four horizontal faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PyramidOp {
    /// The palette role, or `air`.
    pub role: String,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// Overwrite only cells whose current role is listed.
    #[serde(default, rename = "where", skip_serializing_if = "Vec::is_empty")]
    pub where_roles: Vec<String>,
    /// A guard over the scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The course's shape.
    #[serde(default, skip_serializing_if = "is_default")]
    pub section: Section,
    /// Courses per step, default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rise: Option<Int>,
    /// Cells stepped in per `rise` courses, default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Int>,
    /// Keep only the surface.
    #[serde(default, skip_serializing_if = "is_false")]
    pub skin: bool,
}

/// `line` — the integer line of §3.5, stamped with a brush.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LineOp {
    /// The palette role, or `air`.
    pub role: String,
    /// The first end point. **Not a corner**: a line runs between two cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The second end point. Omitted: the single cell `from` names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// Overwrite only cells whose current role is listed.
    #[serde(default, rename = "where", skip_serializing_if = "Vec::is_empty")]
    pub where_roles: Vec<String>,
    /// A guard over the scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The box painted at each point, its minimum corner on the point.
    /// Default `[1, 1, 1]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brush: Option<Vec3>,
}

// ---------------------------------------------------------------------------
// The five arrangers
// ---------------------------------------------------------------------------

/// `scope` — a box a block of operations is written in, so the block moves as
/// one when the plan moves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeOp {
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// A guard over the enclosing scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The operations, in the new box's own coordinates.
    pub body: Vec<Op>,
}

/// `use` — one instance of a [`Define`], turned and mirrored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UseOp {
    /// The define's name.
    pub define: String,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// A guard over the enclosing scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Quarter-turns clockwise seen from above, 0–3, applied after `mirror`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub turn: u8,
    /// Reflect the body across the centre plane of its own local box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror: Option<Horizontal>,
    /// Arguments, each an integer expression in the **enclosing** scope. Only
    /// parameters the define declares.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Int>,
    /// The role each of the define's own role names is bound to here. Every
    /// role the define lists, and no other.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<String, String>,
}

/// `repeat` — a body stamped along a step vector, or fitted along an axis.
///
/// Two spellings of *how many*, one construct. The counted spelling moves the
/// scope's origin and keeps its extents, which is what makes a flight, a stepped
/// arch and a corbel table one operation each; the fitted spelling divides the
/// scope's own axis into item-long slices and says where the leftover goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepeatOp {
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// A guard over the enclosing scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// **Counted**: the offset instance `k` moves the scope's origin by, times
    /// `k`. Written with `count` and with neither `along`, `stride`, `item` nor
    /// `remainder`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<Vec3>,
    /// **Counted**: how many instances.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<Int>,
    /// **Fitted**: the scope axis the items stand along.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub along: Option<Axis>,
    /// **Fitted**: cells from one item's start to the next's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stride: Option<Int>,
    /// **Fitted**: cells each item is long; each instance's scope is its own
    /// slice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Int>,
    /// **Fitted**, required: where the cells the items do not cover go.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remainder: Option<Remainder>,
    /// Bind the instance number as a parameter for the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    /// The operations.
    pub body: Vec<Op>,
}

/// `mirror` — the body, then the body reflected across the centre plane of the
/// operation's box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorOp {
    /// The axis the reflection is across.
    pub axis: Horizontal,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// A guard over the enclosing scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The operations, run once as written and once reflected.
    pub body: Vec<Op>,
}

/// `grammar` — a box-split program expanded into the operation's box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GrammarOp {
    /// The program document, as a path relative to the drawing.
    pub program: String,
    /// The rule to expand from. Absent: the program's `start`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// A guard over the enclosing scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// **The seed, literal.** Which variant is a judgement, so it is an
    /// argument; nothing derives it, and a drawing's own seed reaches weighted
    /// paints and nothing else.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub seed: u64,
    /// Parameters set on the program before it is expanded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, i64>,
    /// Palette roles of the program rebound to block states.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<String, String>,
}

/// `mark` — an anchor, at a cell of the scope it is written in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarkOp {
    /// The declaration, the program document's own [`Mark`] verbatim.
    pub mark: Mark,
    /// A guard over the scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `claim` — the operation's box, named for the spatial contract.
///
/// **This is ADR-0030's `region` and `way` both.** What a claimed name *is* — a
/// space, an out-of-walk region, an edge's opening, a bar a story lifts, a way
/// it lays — is the header's `contract` to say, once per name, and the operation
/// that claims a gate's box is the operation that paints it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimOp {
    /// The region name, kebab-case; the contract classifies it.
    pub region: String,
    /// The low corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec3>,
    /// The high corner, inclusive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec3>,
    /// A guard over the enclosing scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Cond>,
    /// One line for a reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The operations, in the claimed box.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<Op>,
}

// ---------------------------------------------------------------------------
// The document
// ---------------------------------------------------------------------------

/// **A drawing**: one place's detail, at `drawings/<place stem>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Drawing {
    /// The engine's one campaign-format number (ADR-0024). A drawing is a
    /// campaign document, so it carries it and is refused at any other.
    pub dsl_version: String,
    /// Provenance label, as a program's.
    pub name: String,
    /// Integer knobs, each a declaration and a default. The `handed/…` names of
    /// spec-0058 §2.3 are declared here and bound by `delvec detail`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, i64>,
    /// Role to states — one block state, or a weighted list. **Written in the
    /// frame of the scope that paints it**; there is no world-frame spelling.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub palette: BTreeMap<String, States>,
    /// Named, parameterised bodies.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub defines: BTreeMap<String, Define>,
    /// The spatial contract, the program document's own. Required wherever
    /// `delvec detail` binds the drawing (`DW0843`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
    /// The sides that are finished exterior surface (`DW0885`), written through
    /// to the piece.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shown_faces: Vec<String>,
    /// The ordered list. A later operation overwrites an earlier one.
    pub ops: Vec<Op>,
}

impl Drawing {
    /// An empty drawing at the engine's `dsl_version`.
    pub fn new(name: &str) -> Drawing {
        Drawing {
            dsl_version: crate::compiler::DSL_VERSION.to_string(),
            name: name.to_string(),
            params: BTreeMap::new(),
            palette: BTreeMap::new(),
            defines: BTreeMap::new(),
            contract: None,
            shown_faces: Vec::new(),
            ops: Vec::new(),
        }
    }

    /// Bind a palette role to one block state (builder form).
    pub fn role(mut self, role: &str, state: &str) -> Drawing {
        self.palette.insert(
            role.to_string(),
            States::One(state.parse().expect("a block state")),
        );
        self
    }

    /// Declare a parameter (builder form).
    pub fn param(mut self, name: &str, value: i64) -> Drawing {
        self.params.insert(name.to_string(), value);
        self
    }

    /// Append an operation (builder form).
    pub fn op(mut self, op: Op) -> Drawing {
        self.ops.push(op);
        self
    }

    /// Declare the spatial contract (builder form).
    pub fn contract(mut self, contract: Contract) -> Drawing {
        self.contract = Some(contract);
        self
    }

    /// The canonical bytes a drawing's hash is taken over: its serde JSON form.
    ///
    /// Every map in the document is a `BTreeMap` and every list is the author's
    /// own order, so these bytes depend on the document's content and on nothing
    /// else — not on authoring order, not on how it was built (ADR-0006).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a Drawing serialises to JSON")
    }
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}
