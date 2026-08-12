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
//! roles and the `fill`s that name them, and **both halves of a `bind`** — a
//! frame's keys are the source's own parameter and role names, so they take the
//! prefix exactly as the declarations they override do.
//!
//! **Not** renamed by the prefix: anchors. An anchor name is the contract with
//! the campaign DSL — `anchor/watch` is the id a `timed-gate` binds — so an
//! include that silently qualified it would make the vocabulary's whole reason
//! for declaring anchors evaporate at the moment of composition. A blanket
//! prefix is therefore not on offer here and never will be.
//!
//! # Renaming an anchor on purpose
//!
//! What *is* on offer is [`include_renaming`]: an explicit, per-anchor rename
//! given at the include site. Only the stems the caller names move; everything
//! else keeps the contract it was written with, so every program composed before
//! this existed is untouched.
//!
//! The reason it is a caller decision rather than an automatic one is that the
//! two anchors are genuinely two different places. A ward with a causeway keeper
//! *and* a dormant elite has two elites, and the campaign has to be able to say
//! which is which:
//!
//! ```text
//!   include_renaming(zone, &causeway(),     "ward", [("elite", "keeper-elite")])
//!   include_renaming(zone, &elite_ground(), "ring", [])
//!   ⇒ anchor/keeper-elite   (declared by ward/guard_support)
//!     anchor/elite          (declared by ring/circle)
//! ```
//!
//! Written out at the include site, the two names are visible to a reader of the
//! zone. Derived from a prefix, they would silently change every anchor a
//! `timed-gate` already binds.
//!
//! Refusals, because a rename that quietly does nothing is worse than no rename:
//! naming a stem the source never declares is [`ComposeError::UnknownAnchor`]
//! (the typo guard — without it a misspelled entry leaves the collision exactly
//! where it was, and the composition fails later for a reason that names neither
//! the rename nor the caller), a target that is not a kebab-case stem is
//! [`ComposeError::BadAnchorName`], and a target some other piece of the
//! composition already declares is [`ComposeError::AnchorTaken`].
//!
//! What is deliberately *not* refused here: two pieces that collide on a name
//! **nobody renamed**. That stays an expansion-time `AnchorCollision` naming
//! both rules, exactly as before — this module only checks the claims the caller
//! actually made.
//!
//! # Determinism
//!
//! Nothing here draws from the RNG or reads the environment; the maps stay
//! `BTreeMap`s, so a composed program serialises in the same authoring-order-
//! independent form as a hand-written one (ADR-0006).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::ir::{Alternative, Cond, Expr, Mark, MarkAt, Material, Node, Program, Size, Split};

/// An anchor rename given at an include site: source stem → destination stem.
///
/// A `BTreeMap` rather than a list of pairs so that one stem cannot be renamed
/// twice — the "which entry won?" question is removed by construction rather
/// than by a refusal — and so that iteration order is the map's, not the
/// caller's (ADR-0006).
pub type AnchorRenames<'a> = BTreeMap<&'a str, &'a str>;

/// A composition that would have silently merged two names, or a rename that
/// would have silently done nothing.
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
    /// A rename named an anchor stem the source never declares.
    UnknownAnchor {
        /// The stem the rename asked for.
        anchor: String,
        /// The program it was asked of.
        program: String,
        /// Every stem that program does declare.
        declares: Vec<String>,
    },
    /// A rename's target is not a kebab-case anchor stem.
    BadAnchorName {
        /// The stem the rename asked for.
        anchor: String,
        /// The target as given.
        target: String,
    },
    /// A rename's target is a stem the composition already carries.
    AnchorTaken {
        /// The stem the rename asked for.
        anchor: String,
        /// The target it aimed at.
        target: String,
        /// Where that target is already declared.
        holder: String,
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
            ComposeError::UnknownAnchor {
                anchor,
                program,
                declares,
            } => write!(
                f,
                "the rename names the anchor {anchor:?}, which {program:?} never declares; it \
                 declares {declares:?}. A rename that matches nothing would leave the name it \
                 was written to move exactly where it was, so it is refused rather than ignored"
            ),
            ComposeError::BadAnchorName { anchor, target } => write!(
                f,
                "the rename of {anchor:?} aims at {target:?}, which is not kebab-case; an \
                 exported anchor is named `anchor/<kebab>` because that is the id the DSL \
                 resolves, so a stem the DSL could never name is refused here"
            ),
            ComposeError::AnchorTaken {
                anchor,
                target,
                holder,
            } => write!(
                f,
                "the rename of {anchor:?} aims at {target:?}, which {holder} already declares; \
                 a rename replaces one contract name with another and never merges two, so \
                 pick a target no piece of this composition has taken"
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
///
/// Anchors keep the names the source gave them. [`include_renaming`] is the way
/// to move one, and this is exactly that call with an empty rename map.
pub fn include(
    destination: Program,
    source: &Program,
    prefix: &str,
) -> Result<Program, ComposeError> {
    include_renaming(destination, source, prefix, &AnchorRenames::new())
}

/// [`include`], plus an explicit per-anchor rename.
///
/// `renames` maps a stem the *source* declares to the stem the composition
/// should carry instead — `("elite", "keeper-elite")`, not `("anchor/elite",
/// "anchor/keeper-elite")`: the `anchor/` prefix is the export's, not the
/// program's. An indexed mark is renamed by its stem too, so `("niche",
/// "shore-niche")` turns `anchor/niche-1` into `anchor/shore-niche-1`.
///
/// Stems the map does not name are untouched, which is what makes this safe to
/// add to a vocabulary whose anchor names are already campaign contracts: an
/// empty map is the old behaviour, byte for byte.
///
/// # Refusals
///
/// * a rename of a stem `source` does not declare — [`ComposeError::UnknownAnchor`];
/// * a target that is not a kebab-case stem — [`ComposeError::BadAnchorName`];
/// * a target the destination, or the source's own surviving stems, already
///   carry — [`ComposeError::AnchorTaken`].
///
/// The last one is checked here rather than left to expansion because the caller
/// *made a claim about a name*; a collision between two names nobody renamed is
/// still the expansion-time `AnchorCollision`, which names both rules.
pub fn include_renaming(
    mut destination: Program,
    source: &Program,
    prefix: &str,
    renames: &AnchorRenames<'_>,
) -> Result<Program, ComposeError> {
    if prefix.is_empty() || prefix.contains(SEPARATOR) {
        return Err(ComposeError::BadPrefix {
            prefix: prefix.to_string(),
        });
    }
    check_renames(&destination, source, renames)?;
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
            body: node(prefix, renames, &alt.body),
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

/// Every anchor stem a program's rules declare, whether or not the alternative
/// carrying the mark can ever be selected.
///
/// Deliberately syntactic: a rename is a statement about the program's *text*,
/// so "this rule has a mark under a guard that never holds in your box" is still
/// a stem the caller may legitimately rename. The over-approximation only ever
/// makes the typo guard more permissive, never less.
pub fn declared_anchors(program: &Program) -> BTreeSet<String> {
    fn walk(node: &Node, into: &mut BTreeSet<String>) {
        match node {
            Node::Mark { mark, body } => {
                into.insert(mark.anchor.clone());
                walk(body, into);
            }
            Node::Reorient { body, .. } | Node::Bind { body, .. } => walk(body, into),
            Node::Split(split) => split.children.iter().for_each(|c| walk(c, into)),
            Node::Void | Node::Skip | Node::Fill { .. } | Node::Call { .. } => {}
        }
    }
    let mut found = BTreeSet::new();
    for alts in program.rules.values() {
        for alt in alts {
            walk(&alt.body, &mut found);
        }
    }
    found
}

/// Refuse a rename map that would do nothing, name something unnameable, or
/// land on a name the composition already carries.
fn check_renames(
    destination: &Program,
    source: &Program,
    renames: &AnchorRenames<'_>,
) -> Result<(), ComposeError> {
    if renames.is_empty() {
        return Ok(());
    }
    let declares = declared_anchors(source);
    let already = declared_anchors(destination);
    for (from, to) in renames {
        if !declares.contains(*from) {
            return Err(ComposeError::UnknownAnchor {
                anchor: from.to_string(),
                program: source.name.clone(),
                declares: declares.iter().cloned().collect(),
            });
        }
        if !crate::ir::is_kebab(to) {
            return Err(ComposeError::BadAnchorName {
                anchor: from.to_string(),
                target: to.to_string(),
            });
        }
        if already.contains(*to) {
            return Err(ComposeError::AnchorTaken {
                anchor: from.to_string(),
                target: to.to_string(),
                holder: format!("{:?}, already composed in,", destination.name),
            });
        }
        // ...and against what this same include is about to bring: another
        // rename aimed at the same target, or a stem of the source that is
        // staying put. Both would be an `AnchorCollision` two steps later,
        // blamed on a rule rather than on the map that caused it.
        let collides = declares
            .iter()
            .any(|stem| rename(renames, stem) == **to && stem != *from);
        if collides {
            return Err(ComposeError::AnchorTaken {
                anchor: from.to_string(),
                target: to.to_string(),
                holder: format!("{:?}, the piece being included,", source.name),
            });
        }
    }
    Ok(())
}

/// The stem a source anchor ends up with.
fn rename(renames: &AnchorRenames<'_>, stem: &str) -> String {
    renames
        .get(stem)
        .map_or_else(|| stem.to_string(), |to| to.to_string())
}

fn node(prefix: &str, renames: &AnchorRenames<'_>, node: &Node) -> Node {
    match node {
        Node::Void => Node::Void,
        Node::Skip => Node::Skip,
        Node::Fill { material } => Node::Fill {
            material: self::material(prefix, material),
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
                .map(|c| self::node(prefix, renames, c))
                .collect(),
        }),
        Node::Reorient { orient, body } => Node::Reorient {
            orient: *orient,
            body: Box::new(self::node(prefix, renames, body)),
        },
        // A binding's KEYS are the source's own parameter and role names, so
        // they take the prefix exactly as the declarations they override do. A
        // walk that rewrote only the values would produce a program binding a
        // name the composition does not have — `UnknownBinding`, loudly, which
        // is the reason the key rewrite is here and not left to be discovered.
        Node::Bind {
            params,
            palette,
            body,
        } => Node::Bind {
            params: params
                .iter()
                .map(|(name, value)| (qualify(prefix, name), expr(prefix, value)))
                .collect(),
            palette: palette
                .iter()
                .map(|(role, m)| (qualify(prefix, role), self::material(prefix, m)))
                .collect(),
            body: Box::new(self::node(prefix, renames, body)),
        },
        Node::Mark { mark, body } => Node::Mark {
            // The anchor stem never takes the prefix: it is the campaign's name
            // for the place, not the program's name for a rule. It moves only
            // when the caller named it in `renames`, and then to the name the
            // caller wrote — not to a derived one.
            mark: Mark {
                anchor: rename(renames, &mark.anchor),
                at: at(prefix, &mark.at),
                facing: mark.facing,
                index: mark.index,
            },
            body: Box::new(self::node(prefix, renames, body)),
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

fn material(prefix: &str, material: &Material) -> Material {
    match material {
        Material::Role { role } => Material::Role {
            role: qualify(prefix, role),
        },
        inline @ Material::Inline(_) => inline.clone(),
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
