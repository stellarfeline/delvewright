//! The derivation: a grammar program plus a box plus a seed gives a model.
//!
//! Ported from the interpreter half of `SplitGrammar.py` (`yawgmoth/GDMC25`,
//! BSD-3-Clause — see `LICENSE-GDMC25`): `Scope.split`, `rule`'s dispatch, and
//! the `CONTEXT` stack that `with split(...)` pushes children onto.
//!
//! What changed, and why:
//!
//! * **No global state.** Upstream keeps the derivation in module-level `RULES`
//!   / `CONTEXT` / `MATERIALS` dicts, so two expansions cannot coexist and rule
//!   registration leaks between files (hence its `clearrules`). Here the whole
//!   derivation is one owned [`Expander`].
//! * **No implicit context.** Upstream matches body statements to child scopes
//!   by *executing* them in order against a stack; a mismatch silently consumes
//!   a sibling's scope. Here [`crate::ir::Split::children`] states the match,
//!   and a mismatch is refused before expansion starts.
//! * **Seeded randomness only** (ADR-0006): rule choice and per-cell mixes draw
//!   from one [`Rng`] stream, in a fixed traversal order.
//! * **Failure is loud.** Upstream prints and calls `void()` when no rule
//!   applies; here it is [`ExpandError::NoApplicableRule`], because a silently
//!   missing wing is exactly the defect the machine gates exist to catch.

use std::collections::BTreeMap;
use std::fmt;

use crate::block::BlockState;
use crate::eval::{Env, EvalError, Scope};
use crate::explain::{self, GuardLeaf, axis_name, render_cond, render_expr};
use crate::geom::{Axis, Box3, Orientation};
use crate::ir::{
    Alternative, AxisSpec, Bar, Cond, Envelope, Facing, Mark, MarkAt, Material, Node, Paint,
    Program, ProgramError, Reorient, Side, Size, Split, States,
};
use crate::model::{PaletteFull, VoxelModel};
use crate::orient::{FrameSet, OrientError, reachable_frames, reorient};
use crate::rng::Rng;
use crate::split::{ResolvedSize, SplitError, make_split};

/// Recursion and work limits. A grammar can recurse without bound (upstream's
/// church roof does), so the interpreter always runs under a budget: hitting it
/// is a deterministic error, never a hang or a stack overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum nesting of rule calls, splits and reorientations.
    pub max_depth: u32,
    /// Maximum number of scopes visited.
    pub max_scopes: u64,
    /// Maximum cells in the region a program may be expanded over.
    ///
    /// The model is a dense grid allocated **before** the first rule runs, so
    /// an absurd region is not a slow expansion — it is an allocation the
    /// process may not survive, and a killed process reports nothing. The
    /// budget turns it into an [`ExpandError::VolumeLimit`] naming both
    /// numbers. It is never silently clamped: a caller that means to build
    /// something enormous raises the limit and says so.
    pub max_volume: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_depth: 256,
            max_scopes: 2_000_000,
            // 2^24 cells ≈ 32 MiB of cell indices. Comfortably above anything a
            // prefab needs (the vanilla structure cap is 48³ = 110 592) and far
            // below what a 64-bit `Box3` can ask for.
            max_volume: 16_777_216,
        }
    }
}

/// Everything an expansion needs beyond the program and the box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandOptions {
    /// The one seed every random choice derives from.
    pub seed: u64,
    /// Work budget.
    pub limits: Limits,
    /// The orientation the start rule is expanded under.
    pub orientation: Orientation,
}

impl ExpandOptions {
    /// Default limits and identity orientation, with the given seed.
    pub fn seeded(seed: u64) -> ExpandOptions {
        ExpandOptions {
            seed,
            limits: Limits::default(),
            orientation: Orientation::IDENTITY,
        }
    }
}

impl Default for ExpandOptions {
    fn default() -> Self {
        ExpandOptions::seeded(0)
    }
}

/// What an expansion did, beyond the blocks it wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// Scopes visited.
    pub scopes: u64,
    /// Deepest nesting reached.
    pub depth: u32,
    /// Rule alternatives applied.
    pub rules_applied: u64,
}

/// One anchor a rule declared with [`Node::Mark`].
///
/// The position is **local to the expansion region**, already rebased off its
/// origin, because that is what a structure template's metadata means by a
/// position — and because it keeps the ADR-0006 promise that moving the box
/// moves nothing in the output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// Local cell `[x, y, z]`.
    pub pos: [i32; 3],
    /// The facing, declared or derived.
    pub facing: Facing,
    /// The rule that declared it. Provenance for review, and the other half of
    /// a collision report.
    pub declared_by: String,
}

/// One region a rule claimed, resolved for this expansion.
///
/// The boxes are **local to the expansion region**, rebased off its origin, for
/// the reason [`Anchor::pos`] is: a structure template is local-coordinate, so
/// moving the box must move nothing in the output (ADR-0006).
///
/// They are sorted and de-duplicated rather than kept in derivation order.
/// Sorting is what makes the record a *set of cells* rather than a trace of how
/// the rules got there, so two programs that claim the same boxes in different
/// orders resolve to the same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedRegion {
    /// The boxes, canonically ordered. A claim on a scope with no cells
    /// contributes nothing, exactly as `fill` writes nothing there.
    pub boxes: Vec<Box3>,
    /// The rules that claimed it. Provenance for review.
    pub declared_by: Vec<String>,
}

impl ResolvedRegion {
    /// Cells covered, over every box. The binding count a later gate reports.
    pub fn cells(&self) -> u64 {
        self.boxes.iter().map(Box3::volume).sum()
    }
}

/// A declared space, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpace {
    /// The envelope claim, as declared.
    pub envelope: Envelope,
    /// Where it is.
    pub region: ResolvedRegion,
}

/// A declared out-of-walk region, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNoBody {
    /// Why, as declared.
    pub reason: String,
    /// Where it is.
    pub region: ResolvedRegion,
}

/// An edge's own volume — an opening, a stair's treads, a fall column — resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVolume {
    /// The region name, kept so the campaign-side binding has something to
    /// address after the boxes have been consumed.
    pub region: String,
    /// Where it is.
    pub boxes: Vec<Box3>,
}

/// A `barred` edge's bar, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBar {
    /// The region name.
    pub region: String,
    /// Where it is.
    pub boxes: Vec<Box3>,
    /// The palette role it is built from.
    pub role: String,
    /// The block state that role resolves to.
    pub block: BlockState,
}

/// A declared edge, resolved: the class as declared, with every region name it
/// used replaced by the boxes it resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEdge {
    /// A declared space name, or [`crate::ir::EXTERIOR`].
    pub a: String,
    /// A declared space name, or [`crate::ir::EXTERIOR`].
    pub b: String,
    /// The class keyword.
    pub class: &'static str,
    /// The declared level change, where the class has one.
    pub rise: Option<i64>,
    /// The opening or transit volume, where one is declared.
    pub via: Option<ResolvedVolume>,
    /// The bar, on a `barred` edge.
    pub bar: Option<ResolvedBar>,
}

/// The program's spatial contract, **as resolved for this expansion**.
///
/// This — not the program's declaration — is what an export writes, and it is
/// the whole reason the declarations are scope-bound. A program re-expanded at
/// other parameters produces other boxes; a contract that carried literal
/// coordinates would describe the expansion it was written against and quietly
/// mis-describe every other one, which is exactly the disagreement between
/// "what the program says" and "what the bytes are" the contract exists to
/// close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContract {
    /// The space a body enters at.
    pub entry: String,
    /// Spaces, by name.
    pub spaces: BTreeMap<String, ResolvedSpace>,
    /// Out-of-walk regions, by name.
    pub no_body: BTreeMap<String, ResolvedNoBody>,
    /// Edges, in declaration order.
    pub edges: Vec<ResolvedEdge>,
    /// The author's acknowledgement, carried through unchanged.
    pub no_body_majority_ack: Option<String>,
}

/// One orientation-unguarded fill: the `DW0736` finding
/// (`delvewright_schem::blocks::DW_ORIENTED_FILL_UNGUARDED`).
///
/// A frame permutes and reflects the *geometry* a rule describes and never
/// rewrites block-state properties ([`crate::orient`]); the intended mechanism
/// for an oriented block is [`Cond::Orientation`] — the author writes one
/// alternative per frame and the guard selects the matching variant. This
/// record is a fill that skipped that mechanism: it wrote a state whose
/// `facing`/`axis`/connection property lands wrong under the scope's actual
/// frame, with no passed `orientation` guard pinning that frame on the path to
/// the fill.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrientedFinding {
    /// The rule whose fill wrote the state.
    pub rule: String,
    /// The state, vanilla string form.
    pub state: String,
    /// The property that lands wrong, as `key=value`.
    pub property: String,
    /// The scope's frame, as `x->X,y->Y,z->-Z` (local -> world, a leading `-`
    /// on a reflected axis). The reflection is printed because a reflected
    /// frame is a *different* frame that lands a different facing, and a
    /// message that named only the permutation would point a mirrored author
    /// at a frame that reads as identity.
    pub orientation: String,
}

/// One fill the `DW0736` predicate could not judge here: the
/// `DW0742` record (`delvewright_schem::blocks::DW_ORIENTED_FILL_UNDECIDED`).
///
/// A world-frame state standing in the identity frame, in a scope whose frame
/// is **not a constant of the program** — its reachable set
/// ([`crate::orient::FrameSet`]) holds another frame, and that frame would land
/// one of the state's properties wrong.
///
/// The mismatch test short-circuits on the identity frame before it reads a
/// single property, so what it returned here was not a verdict; it was silence.
/// The same program at a region whose axes rank differently hands the same scope
/// a turned frame, and then the literal is judged — which is how the fact that
/// it was never judged here becomes visible only after the piece has shipped.
///
/// A scope whose frame set is a singleton is **not** this, however it got there:
/// no reorientation at all, or one that names its axes outright. Its frame is
/// the identity at every region there will ever be, so a world-frame literal is
/// unconditionally what the author wrote. Nor is a state the reachable frames
/// cannot disturb — an `axis=y` pillar under a request that pins the vertical
/// has no image to get wrong.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UndecidedFinding {
    /// The rule whose fill wrote the state.
    pub rule: String,
    /// The state, vanilla string form.
    pub state: String,
    /// A property some frame would land wrong, as `key=value`.
    pub property: String,
    /// The reorientation request the scope reached this fill through, as the
    /// author wrote it (`z: largest`) — the thing to go and read, because it is
    /// what makes the frame here a fact about the REGION rather than about the
    /// program.
    pub through: String,
    /// The rule the reorientation request was written in.
    pub through_rule: String,
}

/// What the expander saw of orientation-sensitive fills — the binding record
/// the `oriented-fills` gate reports (a green gate states what it examined;
/// zero examined is a fact the report must carry, CLAUDE.md vacuity rule).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrientedFillAudit {
    /// Fill applications the predicate examined — every fill, because "carries
    /// no properties" is itself the predicate's first question.
    pub fills: u64,
    /// Of those, the fills whose paint carries any block-state properties —
    /// the population the mismatch test can bite on.
    pub carrying: u64,
    /// Of THOSE, the fills whose states were read in the scope's own axis names
    /// and resolved into the world's ([`Paint::Local`]) — the binding count of
    /// the local frame. A literal cannot land wrong when there is no literal,
    /// so these leave the mismatch test with nothing to say; reporting how many
    /// went that way is what keeps the remaining population visible instead of
    /// letting the gate quietly stop binding.
    pub resolved: u64,
    /// The unguarded oriented fills, deduplicated and sorted.
    pub unguarded: Vec<OrientedFinding>,
    /// **The fills this region could not decide**, deduplicated and sorted:
    /// world-frame, in a scope whose frame is the identity here and could have
    /// been something else at another region, carrying a property one of those
    /// other frames would land wrong (`DW0742`).
    ///
    /// Separate from [`Self::unguarded`] because it is a different answer, not a
    /// weaker one. An unguarded fill is wrong at this region and will ship
    /// wrong; an undecided fill is one the predicate never got to read, and
    /// whether it is right is a question about a region nobody expanded.
    pub undecided: Vec<UndecidedFinding>,
}

/// The result of expanding a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    /// The blocks.
    pub model: VoxelModel,
    /// The anchors the rules declared, by exported name.
    ///
    /// They live here rather than on the [`VoxelModel`] because a mark writes no
    /// blocks: the model is the block grid whose `canonical_bytes` is the
    /// block-level determinism form, and folding metadata into it would both
    /// change what that hash means and make "a mark changed nothing about the
    /// building" untestable. [`Expansion`] is already the record of what an
    /// expansion did beyond the blocks it wrote.
    pub anchors: BTreeMap<String, Anchor>,
    /// The spatial contract the program declared, resolved against this
    /// expansion's boxes. `None` when the program declares none — which is a
    /// different statement from an empty one, exactly as an absent `lighting`
    /// block differs from `unmeasured`.
    ///
    /// It lives here rather than on the [`VoxelModel`] for the reason
    /// [`Expansion::anchors`] does: a claim writes no blocks, and folding it
    /// into the block grid would change what `canonical_bytes` means and make
    /// "declaring a space changed nothing about the building" untestable.
    pub contract: Option<ResolvedContract>,
    /// The derivation's shape.
    pub stats: Stats,
    /// The orientation-sensitive fills seen, and which were unguarded.
    pub oriented: OrientedFillAudit,
}

/// The scope at a refusal site, in the terms guards read it.
///
/// The dimensions here are **not** the region the author typed: the scope
/// reaching a deep rule has been through reorientations and splits, and losing
/// that fact is what once cost a campaign zone a brute-force region sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeAt {
    /// Extents as the rule's local `dim:x` / `dim:y` / `dim:z` read them,
    /// through the orientation.
    pub local: [u32; 3],
    /// The world box's minimum corner.
    pub origin: [i32; 3],
    /// The world box's extents, in world-axis order.
    pub size: [u32; 3],
    /// Which world axis each local axis names.
    pub orient: Orientation,
}

impl fmt::Display for ScopeAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The frame's SIGN is part of the record: two scopes can share an axis
        // mapping and still read their box from opposite ends, and a rule that
        // refuses in one and passes in the other is exactly the confusion this
        // record exists to end.
        let reversed = match explain::render_reversed_axes(&self.orient) {
            Some(clause) => format!(", {clause}"),
            None => String::new(),
        };
        write!(
            f,
            "local {}x{}x{} (x\u{2192}world {}, y\u{2192}world {}, z\u{2192}world {}{reversed}; \
             world box corner {},{},{} size {}x{}x{})",
            self.local[0],
            self.local[1],
            self.local[2],
            axis_name(self.orient.x),
            axis_name(self.orient.y),
            axis_name(self.orient.z),
            self.origin[0],
            self.origin[1],
            self.origin[2],
            self.size[0],
            self.size[1],
            self.size[2]
        )
    }
}

/// Why one alternative of an exhausted rule was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedAlternative {
    /// 1-based position in the rule's declaration order.
    pub index: usize,
    /// Every guard leaf that decided the rejection — all of them, not the
    /// first: an author handed one constraint at a time re-runs into the next,
    /// which is exactly the sweep this report exists to end.
    pub failed: Vec<GuardLeaf>,
}

/// Render a derivation path as one `at:` line, eliding the middle of a very
/// deep one (a recursion's path repeats its own name once per step).
fn write_path(f: &mut fmt::Formatter<'_>, path: &[String]) -> fmt::Result {
    if path.is_empty() {
        return Ok(());
    }
    write!(f, "\n  at: ")?;
    const HEAD: usize = 6;
    const TAIL: usize = 5;
    if path.len() <= HEAD + TAIL + 1 {
        write!(f, "{}", path.join(" \u{203a} "))
    } else {
        write!(
            f,
            "{} \u{203a} \u{2026} ({} more) \u{203a} {}",
            path[..HEAD].join(" \u{203a} "),
            path.len() - HEAD - TAIL,
            path[path.len() - TAIL..].join(" \u{203a} ")
        )
    }
}

/// Why an expansion stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandError {
    /// The program did not pass [`Program::validate`].
    Program(ProgramError),
    /// Every alternative's guard failed and none was `otherwise`.
    NoApplicableRule {
        /// The rule.
        symbol: String,
        /// The scope the rule was asked to expand into.
        scope: Box<ScopeAt>,
        /// Every non-`otherwise` alternative, with the guard leaves that
        /// rejected it.
        rejected: Vec<RejectedAlternative>,
        /// The derivation path that reached the scope: rule names and split
        /// pieces, outermost first.
        path: Vec<String>,
    },
    /// A size or guard expression could not be evaluated.
    Eval {
        /// The rule being expanded.
        symbol: String,
        /// The failure.
        error: EvalError,
        /// The expression (or guard leaf) that failed, as authored.
        expr: String,
        /// The scope it was evaluated against.
        scope: Box<ScopeAt>,
        /// The derivation path that reached the scope.
        path: Vec<String>,
    },
    /// A size pattern could not be laid out.
    Split {
        /// The rule being expanded.
        symbol: String,
        /// The failure.
        error: SplitError,
        /// The local axis being cut, e.g. `z`.
        axis: Axis,
        /// The world axis that local axis names here.
        world_axis: Axis,
        /// The size pattern with each expression evaluated, e.g.
        /// `abs param:approach = 8, abs 4, rel 1`.
        pattern: String,
        /// The scope being split.
        scope: Box<ScopeAt>,
        /// The derivation path that reached the scope.
        path: Vec<String>,
    },
    /// A reorientation request was contradictory.
    Orient {
        /// The rule being expanded.
        symbol: String,
        /// The failure.
        error: OrientError,
        /// The scope being reoriented.
        scope: Box<ScopeAt>,
        /// The derivation path that reached the scope.
        path: Vec<String>,
    },
    /// An absolute size evaluated negative, or a relative weight non-positive.
    BadSize {
        /// The rule being expanded.
        symbol: String,
        /// The offending value.
        value: i64,
        /// The size expression that produced it, as authored.
        expr: String,
        /// The scope it was evaluated against.
        scope: Box<ScopeAt>,
        /// The derivation path that reached the scope.
        path: Vec<String>,
    },
    /// The depth budget ran out — usually an unguarded recursive rule.
    DepthLimit {
        /// The budget.
        limit: u32,
    },
    /// The scope budget ran out.
    ScopeLimit {
        /// The budget.
        limit: u64,
    },
    /// The region is larger than the model the budget allows.
    VolumeLimit {
        /// Cells the region covers.
        volume: u64,
        /// The budget.
        limit: u64,
    },
    /// A `mark` aimed at a cell outside the scope the rule was given.
    MarkOutsideScope {
        /// The rule being expanded.
        symbol: String,
        /// The anchor stem.
        anchor: String,
        /// The cell it asked for, world space.
        cell: [i64; 3],
        /// The scope's minimum corner.
        origin: [i32; 3],
        /// The scope's extents.
        size: [u32; 3],
    },
    /// A `mark` with no explicit facing sat in a scope whose local `Z` is the
    /// vertical axis, so there is no cardinal direction to derive.
    MarkFacingNotCardinal {
        /// The rule being expanded.
        symbol: String,
        /// The anchor stem.
        anchor: String,
    },
    /// Two marks produced the same anchor name.
    AnchorCollision {
        /// The name both produced.
        anchor: String,
        /// The rule that declared it first.
        first: String,
        /// The rule that declared it again.
        second: String,
    },
    /// A local-frame paint carried a property the pinned vocabulary cannot map
    /// onto the world frame the scope was given
    /// (`delvewright_schem::blocks::DW_LOCAL_FRAME_UNRESOLVABLE`).
    LocalFrameUnresolvable {
        /// The rule being expanded.
        symbol: String,
        /// The state, vanilla string form, as authored in the local frame.
        state: String,
        /// The property with no image, as `key=value`.
        property: String,
        /// The scope's frame, as `x->X,y->Y,z->Z` with a `-` on a reflected
        /// axis.
        orientation: String,
    },
    /// A fill needed a 65537th distinct block state.
    PaletteFull {
        /// The rule being expanded.
        symbol: String,
        /// The failure.
        error: PaletteFull,
    },
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpandError::Program(e) => write!(f, "invalid program: {e}"),
            ExpandError::NoApplicableRule {
                symbol,
                scope,
                rejected,
                path,
            } => {
                write!(
                    f,
                    "no alternative of rule {symbol:?} applies to this scope, and none is \
                     `otherwise`"
                )?;
                write_path(f, path)?;
                write!(
                    f,
                    "\n  scope: {scope} — these are the dimensions at the failure site, not the \
                     region as given"
                )?;
                for alt in rejected {
                    write!(
                        f,
                        "\n  alternative {} of {} rejected — {} condition(s) decided it:",
                        alt.index,
                        rejected.len(),
                        alt.failed.len()
                    )?;
                    for leaf in &alt.failed {
                        write!(f, "\n    {leaf}")?;
                    }
                }
                Ok(())
            }
            ExpandError::Eval {
                symbol,
                error,
                expr,
                scope,
                path,
            } => {
                write!(f, "rule {symbol:?}: {error}, evaluating {expr}")?;
                write_path(f, path)?;
                write!(f, "\n  scope: {scope}")
            }
            ExpandError::Split {
                symbol,
                error,
                axis,
                world_axis,
                pattern,
                scope,
                path,
            } => {
                match error {
                    SplitError::Overflow { absolute, extent } => write!(
                        f,
                        "rule {symbol:?}: split needs {absolute} blocks along local {} (world {}) \
                         but the scope is {extent} across — the region is too small for this rule",
                        axis_name(*axis),
                        axis_name(*world_axis)
                    )?,
                    SplitError::ZeroStride => write!(
                        f,
                        "rule {symbol:?}: a repeating split along local {} (world {}) whose \
                         pattern consumes nothing",
                        axis_name(*axis),
                        axis_name(*world_axis)
                    )?,
                }
                write!(f, "\n  sizes: {pattern}")?;
                write_path(f, path)?;
                write!(f, "\n  scope: {scope}")
            }
            ExpandError::Orient {
                symbol,
                error,
                scope,
                path,
            } => {
                write!(f, "rule {symbol:?}: {error}")?;
                write_path(f, path)?;
                write!(f, "\n  scope: {scope}")
            }
            ExpandError::BadSize {
                symbol,
                value,
                expr,
                scope,
                path,
            } => {
                write!(
                    f,
                    "rule {symbol:?}: {value} is not a usable split size — from {expr}"
                )?;
                write_path(f, path)?;
                write!(f, "\n  scope: {scope}")
            }
            ExpandError::DepthLimit { limit } => {
                write!(f, "expansion exceeded the depth limit of {limit}")
            }
            ExpandError::ScopeLimit { limit } => {
                write!(f, "expansion exceeded the scope limit of {limit}")
            }
            ExpandError::VolumeLimit { volume, limit } => write!(
                f,
                "the region covers {volume} cells but the volume limit is {limit} — raise \
                 `Limits::max_volume` deliberately if a model that large is really wanted"
            ),
            ExpandError::MarkOutsideScope {
                symbol,
                anchor,
                cell,
                origin,
                size,
            } => write!(
                f,
                "rule {symbol:?} marks the anchor {anchor:?} at {},{},{}, which is outside its \
                 own scope (corner {},{},{}, {}x{}x{}) — a rule may only name cells of the box \
                 it was given",
                cell[0],
                cell[1],
                cell[2],
                origin[0],
                origin[1],
                origin[2],
                size[0],
                size[1],
                size[2]
            ),
            ExpandError::MarkFacingNotCardinal { symbol, anchor } => write!(
                f,
                "rule {symbol:?} marks the anchor {anchor:?} in a scope whose local Z is the \
                 vertical axis, so there is no cardinal facing to derive — declare `facing` on \
                 the mark"
            ),
            ExpandError::AnchorCollision {
                anchor,
                first,
                second,
            } => write!(
                f,
                "the anchor {anchor:?} is declared twice: first by rule {first:?}, then by rule \
                 {second:?}. One name is one place; use an indexed mark if the rule is meant to \
                 declare an anchor per expansion, or — if these are two composed pieces that \
                 happen to share a stem — give one of them a name of its own with \
                 `compose::include_renaming`"
            ),
            ExpandError::LocalFrameUnresolvable {
                symbol,
                state,
                property,
                orientation,
            } => write!(
                f,
                "{}: rule {symbol:?} fills {state}, written in the scope's own axis names, into a \
                 scope framed {orientation} — and {property} has no image there. A yaw and a \
                 handedness are stated against a fixed vertical AND a fixed handedness, so any \
                 frame but a pure turn about the vertical leaves them nothing to mean; a \
                 `top`/`bottom` half needs the vertical kept and running forward; and a \
                 direction whose image is not a legal value of the block has nowhere to land. \
                 Write the state in the world frame under an `orientation` guard for the frames \
                 it must cover, or keep the scope's vertical on the world's and unreflected",
                delvewright_schem::blocks::DW_LOCAL_FRAME_UNRESOLVABLE
            ),
            ExpandError::PaletteFull { symbol, error } => write!(f, "rule {symbol:?}: {error}"),
        }
    }
}

impl ExpandError {
    /// Prepend a derivation-path segment onto errors that carry a path.
    ///
    /// The path is built during unwinding — each frame prepends its own
    /// segment as the error passes through — so the happy path allocates
    /// nothing for it.
    fn pushed(mut self, segment: impl FnOnce() -> String) -> Self {
        match &mut self {
            ExpandError::NoApplicableRule { path, .. }
            | ExpandError::Eval { path, .. }
            | ExpandError::Split { path, .. }
            | ExpandError::Orient { path, .. }
            | ExpandError::BadSize { path, .. } => path.insert(0, segment()),
            _ => {}
        }
        self
    }
}

impl std::error::Error for ExpandError {}

impl From<ProgramError> for ExpandError {
    fn from(e: ProgramError) -> Self {
        ExpandError::Program(e)
    }
}

/// Expand `program` over `region`.
///
/// Same program + same region + same seed gives a byte-identical model
/// (ADR-0006); nothing here reads the clock, the environment or a hash order.
pub fn expand(
    program: &Program,
    region: Box3,
    options: &ExpandOptions,
) -> Result<Expansion, ExpandError> {
    program.validate()?;
    // Before the model is allocated, not after: the grid is dense.
    let volume = region.volume();
    if volume > options.limits.max_volume {
        return Err(ExpandError::VolumeLimit {
            volume,
            limit: options.limits.max_volume,
        });
    }
    let mut expander = Expander {
        program,
        model: VoxelModel::new(region),
        anchors: BTreeMap::new(),
        marks_seen: BTreeMap::new(),
        regions: BTreeMap::new(),
        rng: Rng::new(options.seed),
        limits: options.limits,
        stats: Stats::default(),
        oriented_fills: 0,
        oriented_carrying: 0,
        oriented_resolved: 0,
        oriented_unguarded: std::collections::BTreeSet::new(),
        oriented_undecided: std::collections::BTreeSet::new(),
    };
    // The root binding frame is the program's own declarations, which are its
    // defaults; every `bind` pushes a frame over this one.
    let root = Env::root(&program.params, &program.palette);
    expander.run_rule(
        &program.start,
        &ScopeState {
            region,
            orient: options.orientation,
            env: root,
            pinned: None,
            reachable: FrameSet::just(options.orientation),
            reframed: None,
        },
        0,
    )?;
    expander.canonicalise_regions();
    let contract = resolve_contract(program, &mut expander.regions);
    Ok(Expansion {
        model: expander.model,
        anchors: expander.anchors,
        contract,
        stats: expander.stats,
        oriented: OrientedFillAudit {
            fills: expander.oriented_fills,
            carrying: expander.oriented_carrying,
            resolved: expander.oriented_resolved,
            unguarded: expander.oriented_unguarded.into_iter().collect(),
            undecided: expander.oriented_undecided.into_iter().collect(),
        },
    })
}

/// Turn the program's declared contract into the resolved one, using the boxes
/// the claims produced.
///
/// Every name resolves — `Program::validate` proved that before a single rule
/// ran — but a name may resolve to *no* boxes, when every rule that claims it
/// sat under a guard this seed did not take. That is recorded as an empty
/// region rather than refused: a zero binding is a finding for whatever reads
/// the contract, and losing the declaration would hide it.
fn resolve_contract(
    program: &Program,
    regions: &mut BTreeMap<String, ResolvedRegion>,
) -> Option<ResolvedContract> {
    let declared = program.contract.as_ref()?;
    let take = |regions: &mut BTreeMap<String, ResolvedRegion>, name: &str| -> ResolvedRegion {
        regions.remove(name).unwrap_or_default()
    };
    let spaces = declared
        .spaces
        .iter()
        .map(|(name, decl)| {
            (
                name.clone(),
                ResolvedSpace {
                    envelope: decl.envelope,
                    region: take(regions, name),
                },
            )
        })
        .collect();
    let no_body = declared
        .no_body
        .iter()
        .map(|(name, decl)| {
            (
                name.clone(),
                ResolvedNoBody {
                    reason: decl.reason.clone(),
                    region: take(regions, name),
                },
            )
        })
        .collect();
    // An edge's volumes are read rather than taken: two edges may legitimately
    // name one opening, and the second must see the same boxes as the first.
    let left: &BTreeMap<String, ResolvedRegion> = regions;
    let boxes_of = |name: &str| left.get(name).map(|r| r.boxes.clone()).unwrap_or_default();
    let volume = |name: &str| ResolvedVolume {
        region: name.to_string(),
        boxes: boxes_of(name),
    };
    let bar = |b: &Bar| ResolvedBar {
        region: b.region.clone(),
        boxes: boxes_of(&b.region),
        role: b.block.clone(),
        block: match program.palette.get(&b.block).map(Paint::states) {
            Some(States::One(state)) => state.clone(),
            // `validate` refuses a mix and an unbound role, so neither reaches
            // here; air is the inert stand-in a panic would otherwise be.
            _ => BlockState::air(),
        },
    };
    let edges = declared
        .edges
        .iter()
        .map(|e| ResolvedEdge {
            a: e.a.clone(),
            b: e.b.clone(),
            class: e.class.as_str(),
            rise: e.class.rise(),
            via: e.class.via().map(volume),
            bar: e.class.bar().map(bar),
        })
        .collect();
    Some(ResolvedContract {
        entry: declared.entry.clone(),
        spaces,
        no_body,
        edges,
        no_body_majority_ack: declared.no_body_majority_ack.clone(),
    })
}

/// True when a passed `cond` **entails** a [`Cond::Orientation`]: the guard
/// itself, or an `all` that contains one (recursively). `any`/`none_of` do not
/// entail — the alternative may have been selected by a sibling condition, so
/// the frame was never proved.
fn cond_pins_orientation(cond: &Cond) -> bool {
    match cond {
        Cond::Orientation { .. } => true,
        Cond::All { of } => of.iter().any(cond_pins_orientation),
        _ => false,
    }
}

/// A frame written for a person to find: `x->X,y->Y,z->-Z`, local to world,
/// with a leading `-` on an axis that runs backwards.
///
/// A diagnostic that named only the permutation would print `x->X,y->Y,z->Z`
/// for a reflected identity frame — an author reading that would look for a
/// `reorient` there is none of, and the reflection that actually turned their
/// block would not appear anywhere in the message.
fn frame_label(orient: Orientation) -> String {
    let axis = |local: Axis| {
        format!(
            "{}{:?}",
            if orient.reversed(local) { "-" } else { "" },
            orient.axis(local)
        )
    };
    format!(
        "x->{},y->{},z->{}",
        axis(Axis::X),
        axis(Axis::Y),
        axis(Axis::Z)
    )
}

/// What a child scope inherits about its frame: the set of frames it could have
/// stood in, and the innermost request that made that set bigger than one.
///
/// A request that widens nothing is not a reframing, however it is spelled.
/// `keep` is the obvious case and is the default on every `split`, so counting
/// it would make every split piece a reframed scope; but so is `x: world_x,
/// y: world_y, z: world_z` from a scope whose frame was already a constant, and
/// so is any request from a scope where the answer was fixed anyway. Only a
/// request that leaves the child with more frames than one has made a fact about
/// the region out of something that was a fact about the program.
fn reframed_by<'e>(
    parent: &ScopeState<'e>,
    symbol: &'e str,
    request: &Reorient,
    split_axis: Option<Axis>,
) -> (FrameSet, Option<(&'e str, Reorient)>) {
    let reachable = reachable_frames(parent.reachable, request, split_axis);
    let reframed = if reachable.is_singleton() {
        // A request that closes the question back down — a reset to the world
        // axes under an outer `largest` — leaves nothing region-dependent to
        // name, and carrying the parent's request here would point an author at
        // a reorientation this scope is no longer under.
        None
    } else if reachable == parent.reachable {
        parent.reframed
    } else {
        Some((symbol, *request))
    };
    (reachable, reframed)
}

/// A reorientation request as the author wrote it — `z: largest, x: local_z`,
/// or `mirror: x` for one that only flips.
fn reorient_label(request: &Reorient) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (name, spec) in [("x", request.x), ("y", request.y), ("z", request.z)] {
        if let Some(spec) = spec {
            parts.push(format!("{name}: {}", axis_spec_name(spec)));
        }
    }
    let flipped = request.mirror.axes();
    let names: Vec<&str> = ["x", "y", "z"]
        .iter()
        .zip(flipped)
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect();
    if !names.is_empty() {
        parts.push(format!("mirror: {}", names.join("+")));
    }
    if parts.is_empty() {
        "keep".to_string()
    } else {
        parts.join(", ")
    }
}

/// The authoring name of one axis assignment, as the JSON IR spells it.
fn axis_spec_name(spec: AxisSpec) -> &'static str {
    match spec {
        AxisSpec::LocalX => "local_x",
        AxisSpec::LocalY => "local_y",
        AxisSpec::LocalZ => "local_z",
        AxisSpec::WorldX => "world_x",
        AxisSpec::WorldY => "world_y",
        AxisSpec::WorldZ => "world_z",
        AxisSpec::Smallest => "smallest",
        AxisSpec::Largest => "largest",
        AxisSpec::SplitAxis => "split_axis",
    }
}

/// The box a node is expanding into, what it calls its axes, and what its names
/// mean.
///
/// All three are inherited by every child scope, and each has one construct that
/// changes it: `split` the box, `reorient` the axes, `bind` the names.
#[derive(Debug, Clone, Copy)]
struct ScopeState<'e> {
    region: Box3,
    orient: Orientation,
    env: Env<'e>,
    /// The frame the innermost passed [`Cond::Orientation`] guard asserted, if
    /// any. A fill of an orientation-sensitive state is guarded exactly when
    /// this equals the scope's *current* frame: the guard proved the author
    /// wrote the state for this frame. A later `reorient` — including one that
    /// only reflects — leaves the pin recording the old value, so the equality
    /// fails, which is right, because the guard said nothing about the new
    /// frame. `bind` and `claim` change neither half of the frame and so hand
    /// the pin on unchanged.
    pinned: Option<Orientation>,
    /// **Every frame this scope could have stood in**, over every region the
    /// program could be expanded at ([`crate::orient::FrameSet`]).
    ///
    /// This is what separates "the frame here is the identity" from "the frame
    /// here is the identity *at this region*". A scope no rule reoriented, and
    /// one reoriented only by requests that name axes outright, has a frame that
    /// is a constant of the PROGRAM — the set is a singleton and every frame
    /// question about it is settled here for good. A scope under `z: largest`
    /// has the identity only while this box's longest axis happens to be the one
    /// the request already names, and its set holds the quarter-turns too.
    ///
    /// The frame itself cannot tell those apart, which is why `DW0736` returning
    /// `None` for the identity was read as a verdict when it was silence.
    reachable: FrameSet,
    /// The innermost reorientation request that widened [`Self::reachable`],
    /// with the rule it was written in — the thing to go and read, kept so a
    /// `DW0742` can point at it instead of describing it.
    reframed: Option<(&'e str, Reorient)>,
}

impl ScopeState<'_> {
    /// What a guard or size expression is measured against here.
    fn scope(&self) -> Scope<'_> {
        Scope {
            region: &self.region,
            orient: self.orient,
            env: self.env,
        }
    }

    /// The refusal-site record of this scope.
    fn at(&self) -> ScopeAt {
        ScopeAt {
            local: [
                self.region.extent(self.orient.x),
                self.region.extent(self.orient.y),
                self.region.extent(self.orient.z),
            ],
            origin: self.region.origin,
            size: self.region.size,
            orient: self.orient,
        }
    }
}

struct Expander<'a> {
    program: &'a Program,
    model: VoxelModel,
    anchors: BTreeMap<String, Anchor>,
    /// Per-stem occurrence counter, so [`crate::ir::MarkIndex::Auto`] numbers in
    /// expansion order.
    marks_seen: BTreeMap<String, u32>,
    /// Boxes claimed per contract region name, before they are canonicalised.
    regions: BTreeMap<String, ResolvedRegion>,
    rng: Rng,
    limits: Limits,
    stats: Stats,
    /// Fill applications examined.
    oriented_fills: u64,
    /// Fill applications whose paint carries any properties.
    oriented_carrying: u64,
    /// Of those, the ones resolved out of the local frame.
    oriented_resolved: u64,
    /// `DW0736` findings, deduplicated (a rule in a repeat split would
    /// otherwise report once per piece).
    oriented_unguarded: std::collections::BTreeSet<OrientedFinding>,
    /// `DW0742` records, deduplicated for the same reason.
    oriented_undecided: std::collections::BTreeSet<UndecidedFinding>,
}

impl<'a> Expander<'a> {
    fn enter(&mut self, depth: u32) -> Result<(), ExpandError> {
        if depth > self.limits.max_depth {
            return Err(ExpandError::DepthLimit {
                limit: self.limits.max_depth,
            });
        }
        self.stats.scopes += 1;
        self.stats.depth = self.stats.depth.max(depth);
        if self.stats.scopes > self.limits.max_scopes {
            return Err(ExpandError::ScopeLimit {
                limit: self.limits.max_scopes,
            });
        }
        Ok(())
    }

    fn run_rule(
        &mut self,
        symbol: &'a str,
        state: &ScopeState<'_>,
        depth: u32,
    ) -> Result<(), ExpandError> {
        self.run_rule_inner(symbol, state, depth)
            .map_err(|e| e.pushed(|| symbol.to_string()))
    }

    fn run_rule_inner(
        &mut self,
        symbol: &'a str,
        state: &ScopeState,
        depth: u32,
    ) -> Result<(), ExpandError> {
        self.enter(depth)?;
        let alts: &'a [Alternative] = self
            .program
            .rules
            .get(symbol)
            .expect("validate() proved every rule exists");

        // Guarded alternatives first; `otherwise` alternatives only stand in
        // when nothing else applies (upstream's ELSE).
        let mut candidates = Vec::new();
        for (i, alt) in alts.iter().enumerate() {
            if matches!(alt.when, Cond::Otherwise) {
                continue;
            }
            let ok = match state.scope().test(&alt.when) {
                Ok(ok) => ok,
                Err(error) => {
                    // Name the leaf that failed, not just the rule: `explain`
                    // walks the same guard and reports the unevaluable leaf.
                    let mut leaves = Vec::new();
                    explain::explain(&state.scope(), &alt.when, true, &mut leaves);
                    let expr = leaves
                        .iter()
                        .find_map(|l| match l {
                            GuardLeaf::Unevaluable { rendered, .. } => Some(rendered.clone()),
                            _ => None,
                        })
                        .unwrap_or_else(|| render_cond(&alt.when));
                    return Err(ExpandError::Eval {
                        symbol: symbol.to_string(),
                        error,
                        expr,
                        scope: Box::new(state.at()),
                        path: Vec::new(),
                    });
                }
            };
            if ok {
                candidates.push(i);
            }
        }
        if candidates.is_empty() {
            candidates = alts
                .iter()
                .enumerate()
                .filter(|(_, alt)| matches!(alt.when, Cond::Otherwise))
                .map(|(i, _)| i)
                .collect();
        }
        if candidates.is_empty() {
            // Guard exhaustion. Say, for every alternative, which comparisons
            // rejected it and what the operands were — the refusal an author
            // can act on without a brute-force region sweep.
            let scope = state.scope();
            let rejected = alts
                .iter()
                .enumerate()
                .filter(|(_, alt)| !matches!(alt.when, Cond::Otherwise))
                .map(|(i, alt)| {
                    let mut failed = Vec::new();
                    explain::explain(&scope, &alt.when, true, &mut failed);
                    RejectedAlternative {
                        index: i + 1,
                        failed,
                    }
                })
                .collect();
            return Err(ExpandError::NoApplicableRule {
                symbol: symbol.to_string(),
                scope: Box::new(state.at()),
                rejected,
                path: Vec::new(),
            });
        }

        let weights: Vec<u32> = candidates.iter().map(|&i| alts[i].weight).collect();
        let picked = candidates[self
            .rng
            .weighted(&weights)
            .expect("validate() proved every weight is positive")];
        self.stats.rules_applied += 1;
        // A passed guard that *entails* an orientation pins the frame: the
        // author wrote this alternative for the frame the guard names — the
        // permutation and the reflection both, since `Cond::Orientation`
        // matches both exactly — which is what licenses the oriented block
        // states inside it (`DW0736`).
        let state = ScopeState {
            pinned: if cond_pins_orientation(&alts[picked].when) {
                Some(state.orient)
            } else {
                state.pinned
            },
            ..*state
        };
        self.run_node(symbol, &alts[picked].body, &state, depth + 1)
    }

    fn run_node(
        &mut self,
        symbol: &'a str,
        node: &'a Node,
        state: &ScopeState<'_>,
        depth: u32,
    ) -> Result<(), ExpandError> {
        self.enter(depth)?;
        match node {
            Node::Skip => Ok(()),
            Node::Void => {
                let air = BlockState::air();
                for pos in state.region.positions() {
                    self.model
                        .set(pos, &air)
                        .map_err(|error| ExpandError::PaletteFull {
                            symbol: symbol.to_string(),
                            error,
                        })?;
                }
                Ok(())
            }
            Node::Fill { material } => {
                let paint = match material {
                    Material::Role { role } => state
                        .env
                        .paint(role)
                        .expect("validate() proved every role is bound"),
                    Material::Inline(paint) => paint,
                };
                // A local-frame paint is read in the scope's own axis names and
                // resolved here, where the frame exists; a world-frame one is
                // written as authored and audited for `DW0736`.
                let resolved: Option<States> = if paint.is_local() {
                    Some(self.resolve_local(symbol, paint.states(), state)?)
                } else {
                    None
                };
                self.audit_oriented_fill(symbol, paint, state);
                let states = resolved.as_ref().unwrap_or(paint.states());
                let full = |error| ExpandError::PaletteFull {
                    symbol: symbol.to_string(),
                    error,
                };
                match states {
                    States::One(block) => {
                        let block = block.clone();
                        for pos in state.region.positions() {
                            self.model.set(pos, &block).map_err(full)?;
                        }
                    }
                    States::Mix(mix) => {
                        let weights: Vec<u32> = mix.iter().map(|w| w.weight).collect();
                        let blocks: Vec<BlockState> = mix.iter().map(|w| w.block.clone()).collect();
                        let positions: Vec<[i32; 3]> = state.region.positions().collect();
                        for pos in positions {
                            let pick = self
                                .rng
                                .weighted(&weights)
                                .expect("validate() proved every weight is positive");
                            self.model.set(pos, &blocks[pick]).map_err(full)?;
                        }
                    }
                }
                Ok(())
            }
            Node::Call { symbol: target } => self.run_rule(target, state, depth + 1),
            Node::Reorient {
                orient: request,
                body,
            } => {
                let orient =
                    reorient(state.orient, state.region.size, request, None).map_err(|error| {
                        ExpandError::Orient {
                            symbol: symbol.to_string(),
                            error,
                            scope: Box::new(state.at()),
                            path: Vec::new(),
                        }
                    })?;
                let (reachable, reframed) = reframed_by(state, symbol, request, None);
                let child = ScopeState {
                    region: state.region,
                    orient,
                    env: state.env,
                    // The pin travels through the reorientation rather than
                    // being cleared, so it still *equals* the frame when the
                    // request was a no-op or a second reflection that cancels
                    // the first — those land the author back in the frame the
                    // guard proved, and the state is right again there.
                    pinned: state.pinned,
                    reachable,
                    reframed,
                };
                self.run_node(symbol, body, &child, depth + 1)
            }
            Node::Bind {
                params,
                palette,
                body,
            } => {
                // Every binding is evaluated in the ENCLOSING scope, before the
                // frame is pushed, and all of them at once: a frame is a
                // simultaneous rebinding, so `{a: param b, b: param a}` swaps
                // the two rather than chaining them. Nothing here draws from the
                // RNG, so a `bind` cannot move a cell of the model on its own.
                let mut bound_params = BTreeMap::new();
                for (name, value) in params {
                    let evaluated =
                        state
                            .scope()
                            .eval(value)
                            .map_err(|error| ExpandError::Eval {
                                symbol: symbol.to_string(),
                                error,
                                expr: render_expr(value),
                                scope: Box::new(state.at()),
                                path: Vec::new(),
                            })?;
                    bound_params.insert(name.clone(), evaluated);
                }
                let mut bound_palette = BTreeMap::new();
                for (name, material) in palette {
                    let paint = match material {
                        Material::Role { role } => state
                            .env
                            .paint(role)
                            .expect("validate() proved every role is bound")
                            .clone(),
                        Material::Inline(paint) => paint.clone(),
                    };
                    bound_palette.insert(name.clone(), paint);
                }
                let child = ScopeState {
                    region: state.region,
                    orient: state.orient,
                    env: Env::child(&state.env, &bound_params, &bound_palette),
                    // A `bind` renames values; it moves neither half of the
                    // frame, so what the guard proved is still true here — and
                    // for the same reason it asks for no frame of its own.
                    pinned: state.pinned,
                    reachable: state.reachable,
                    reframed: state.reframed,
                };
                self.run_node(symbol, body, &child, depth + 1)
            }
            Node::Mark { mark, body } => {
                // The mark first, so a nested mark of the same stem numbers
                // after it: expansion order is reading order.
                self.declare(symbol, mark, state)?;
                self.run_node(symbol, body, state, depth + 1)
            }
            Node::Claim { region, body } => {
                self.claim(symbol, region, state);
                self.run_node(symbol, body, state, depth + 1)
            }
            Node::Split(split) => self.run_split(symbol, split, state, depth),
        }
    }

    /// Record one [`Node::Claim`]: the scope's box, rebased local, under the
    /// region's name.
    ///
    /// Infallible on purpose. A claim needs a *box*, and every scope has one; a
    /// mark needs a *cell*, which is why a mark on a degenerate scope is an
    /// error and a claim on one simply contributes nothing — the same thing
    /// `fill` and `void` do there. Whether a region ends up covering the cells
    /// its author meant is a question about the model, and this is not the
    /// layer that answers it.
    fn claim(&mut self, symbol: &str, region: &str, state: &ScopeState) {
        let entry = self.regions.entry(region.to_string()).or_default();
        if !entry.declared_by.iter().any(|s| s == symbol) {
            entry.declared_by.push(symbol.to_string());
        }
        if state.region.is_empty() {
            return;
        }
        let origin = self.model.region().origin;
        let local = Box3::new(
            [
                state.region.origin[0] - origin[0],
                state.region.origin[1] - origin[1],
                state.region.origin[2] - origin[2],
            ],
            state.region.size,
        );
        entry.boxes.push(local);
    }

    /// Sort every claimed region's boxes into the canonical order and drop
    /// duplicates, so the record is the set of cells rather than a trace of the
    /// derivation.
    fn canonicalise_regions(&mut self) {
        for region in self.regions.values_mut() {
            region
                .boxes
                .sort_by_key(|b| (b.origin, [b.size[0], b.size[1], b.size[2]]));
            region.boxes.dedup();
            region.declared_by.sort();
            region.declared_by.dedup();
        }
    }

    /// Read a local-frame paint's states in the scope's own axis names and
    /// return them in the world's.
    ///
    /// The transform is the registry's
    /// (`BlockRegistry::permuted_properties`) — the same one the `DW0736`
    /// predicate runs to decide that an unframed literal landed wrong. It is
    /// handed **both halves of the frame**: the axis permutation and the
    /// reflection. Handing it the permutation alone would be the same short
    /// circuit the `DW0736` judge once had, except that here it does not miss a
    /// defect, it writes one — a pure reflection has the identity permutation,
    /// so every mirrored body would silently take the unmirrored state.
    ///
    /// A property whose image the pinned vocabulary does not determine is
    /// refused here rather than guessed: there is no correct block to write,
    /// and writing a plausible one is how a wrong facing gets frozen into a
    /// `.nbt`.
    fn resolve_local(
        &self,
        symbol: &str,
        states: &States,
        state: &ScopeState<'_>,
    ) -> Result<States, ExpandError> {
        let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
        let perm = [
            state.orient.axis(Axis::X).index(),
            state.orient.axis(Axis::Y).index(),
            state.orient.axis(Axis::Z).index(),
        ];
        let reflected = state.orient.mirror.axes();
        states.map(|block| {
            match registry.permuted_properties(&block.name, &block.properties, perm, reflected) {
                Ok(properties) => Ok(BlockState {
                    name: block.name.clone(),
                    properties,
                }),
                Err(property) => Err(ExpandError::LocalFrameUnresolvable {
                    symbol: symbol.to_string(),
                    state: block.to_string(),
                    property,
                    orientation: frame_label(state.orient),
                }),
            }
        })
    }

    /// Record what one fill means for the `DW0736` audit.
    ///
    /// The predicate itself —
    /// [`delvewright_schem::blocks::BlockRegistry::oriented_mismatch`] — lives
    /// with the block-state model, derived from the registry's own value
    /// vocabulary, so this method only supplies the two facts the expander
    /// alone knows: the scope's frame — **both** halves of it, the permutation
    /// and the reflection — and whether a passed [`Cond::Orientation`] guard
    /// pinned it. The paint is the one the scope's own bindings resolved, so a
    /// role a `bind` rebound is audited as what it now paints.
    fn audit_oriented_fill(&mut self, symbol: &str, paint: &Paint, state: &ScopeState) {
        let states: Vec<&BlockState> = paint.states().each();
        self.oriented_fills += 1;
        if states.iter().all(|b| b.properties.is_empty()) {
            return;
        }
        self.oriented_carrying += 1;
        if paint.is_local() {
            // Resolved through this very frame a moment ago, so there is no
            // literal left to land wrong. Counted, never skipped: the gate has
            // to be able to say how much of its population went this way.
            self.oriented_resolved += 1;
            return;
        }
        if state.pinned == Some(state.orient) {
            return; // the guard proved the author wrote these for this frame
        }
        let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
        let perm = [
            state.orient.axis(Axis::X).index(),
            state.orient.axis(Axis::Y).index(),
            state.orient.axis(Axis::Z).index(),
        ];
        let reflected = state.orient.mirror.axes();
        // **The frame is the identity.** `oriented_mismatch` returns `None`
        // there before it reads a property, so running the loop below would
        // record a pass over an examination that did not happen. Which of the
        // two facts that `None` is depends on whether the identity is a fact
        // about the PROGRAM — this scope's frame is the identity at every
        // region — or about the REGION, `z: largest` on a box whose longest
        // axis is already Z. The scope's own frame cannot say; its reachable
        // SET can, and the second case is `DW0742`.
        //
        // Judged against the frames this scope could actually have stood in,
        // never against all 48. A request that pins the vertical
        // (`y: world_y`) leaves every `axis=y` pillar and every `facing=up`
        // barrel decided, and reporting those would have put six of the live
        // campaign's eight zones into a state their authors could do nothing
        // about — the shape of a gate that gets routed around.
        if perm == [0, 1, 2] && reflected == [false; 3] {
            if let Some((through_rule, request)) = state.reframed
                && !state.reachable.is_only(state.orient)
            {
                let frames: Vec<([usize; 3], [bool; 3])> = state
                    .reachable
                    .iter()
                    .map(|f| {
                        (
                            [
                                f.axis(Axis::X).index(),
                                f.axis(Axis::Y).index(),
                                f.axis(Axis::Z).index(),
                            ],
                            f.mirror.axes(),
                        )
                    })
                    .collect();
                for block in states.iter() {
                    if let Some(property) = registry
                        .frame_sensitive(&block.name, &block.properties, frames.iter().copied())
                        .into_iter()
                        .next()
                    {
                        self.oriented_undecided.insert(UndecidedFinding {
                            rule: symbol.to_string(),
                            state: block.to_string(),
                            property,
                            through: reorient_label(&request),
                            through_rule: through_rule.to_string(),
                        });
                    }
                }
            }
            return;
        }
        for block in states {
            if let Some(property) =
                registry.oriented_mismatch(&block.name, &block.properties, perm, reflected)
            {
                self.oriented_unguarded.insert(OrientedFinding {
                    rule: symbol.to_string(),
                    state: block.to_string(),
                    property,
                    orientation: frame_label(state.orient),
                });
            }
        }
    }

    /// Resolve one [`Node::Mark`] against the scope it sits on and record it.
    fn declare(
        &mut self,
        symbol: &str,
        mark: &Mark,
        state: &ScopeState<'_>,
    ) -> Result<(), ExpandError> {
        let cell = self.mark_cell(symbol, mark, state)?;
        let facing = match mark.facing {
            Some(f) => f,
            // A derived facing is the direction of *decreasing local Z* — the
            // way the rule library's frame says travel runs. Which world
            // direction that is depends on both halves of the frame: the world
            // axis local Z names, and whether local Z runs down it.
            None => match (state.orient.axis(Axis::Z), state.orient.reversed(Axis::Z)) {
                (Axis::Z, false) => Facing::North,
                (Axis::Z, true) => Facing::South,
                (Axis::X, false) => Facing::West,
                (Axis::X, true) => Facing::East,
                (Axis::Y, _) => {
                    return Err(ExpandError::MarkFacingNotCardinal {
                        symbol: symbol.to_string(),
                        anchor: mark.anchor.clone(),
                    });
                }
            },
        };

        let seen = self.marks_seen.entry(mark.anchor.clone()).or_insert(0);
        *seen += 1;
        let name = mark.name(*seen);

        let origin = self.model.region().origin;
        let anchor = Anchor {
            pos: [
                cell[0] - origin[0],
                cell[1] - origin[1],
                cell[2] - origin[2],
            ],
            facing,
            declared_by: symbol.to_string(),
        };
        if let Some(first) = self.anchors.get(&name) {
            return Err(ExpandError::AnchorCollision {
                anchor: name,
                first: first.declared_by.clone(),
                second: symbol.to_string(),
            });
        }
        self.anchors.insert(name, anchor);
        Ok(())
    }

    /// The world cell a mark names, refused if it is not one of the scope's own.
    fn mark_cell(
        &self,
        symbol: &str,
        mark: &Mark,
        state: &ScopeState<'_>,
    ) -> Result<[i32; 3], ExpandError> {
        let size = state.region.size;
        // Extent along the world axis a local axis names.
        let extent = |local: Axis| size[state.orient.axis(local).index()] as i64;
        // Centre of an extent, rounding down; 0 for a degenerate axis, which the
        // bounds check below then reports.
        let mid = |n: i64| (n - 1).max(0) / 2;

        // Offsets from the scope's minimum **world** corner, per world axis.
        //
        // Every `at` but `floor_center` names a cell in LOCAL terms, so it is
        // computed in local coordinates and put through the frame once, at the
        // end: a reflected axis counts from the far end of the box, which is
        // exactly what makes the mirror image of a rule land on the mirror image
        // of its anchor.
        let mut delta = [0i64; 3];
        let mut local = [Option::<i64>::None; 3];
        match &mark.at {
            MarkAt::CornerMin => local = [Some(0), Some(0), Some(0)],
            MarkAt::FloorCenter => {
                // Gravity is a world fact, so this one position ignores the
                // frame entirely — both halves of it.
                delta[Axis::X.index()] = mid(size[Axis::X.index()] as i64);
                delta[Axis::Y.index()] = 0;
                delta[Axis::Z.index()] = mid(size[Axis::Z.index()] as i64);
            }
            MarkAt::FaceCenter { axis, side } => {
                for l in Axis::ALL {
                    local[l.index()] = Some(if l == *axis {
                        match side {
                            Side::Min => 0,
                            Side::Max => (extent(l) - 1).max(0),
                        }
                    } else {
                        mid(extent(l))
                    });
                }
            }
            MarkAt::Offset { x, y, z } => {
                let scope = state.scope();
                for (l, expr) in [(Axis::X, x), (Axis::Y, y), (Axis::Z, z)] {
                    let value = scope.eval(expr).map_err(|error| ExpandError::Eval {
                        symbol: symbol.to_string(),
                        error,
                        expr: render_expr(expr),
                        scope: Box::new(state.at()),
                        path: Vec::new(),
                    })?;
                    local[l.index()] = Some(value);
                }
            }
        }
        for l in Axis::ALL {
            if let Some(coord) = local[l.index()] {
                delta[state.orient.axis(l).index()] = state.orient.offset(l, coord, size);
            }
        }

        let cell = [
            state.region.origin[0] as i64 + delta[0],
            state.region.origin[1] as i64 + delta[1],
            state.region.origin[2] as i64 + delta[2],
        ];
        let inside = (0..3).all(|a| delta[a] >= 0 && delta[a] < size[a] as i64);
        if !inside {
            return Err(ExpandError::MarkOutsideScope {
                symbol: symbol.to_string(),
                anchor: mark.anchor.clone(),
                cell,
                origin: state.region.origin,
                size,
            });
        }
        Ok([cell[0] as i32, cell[1] as i32, cell[2] as i32])
    }

    fn run_split(
        &mut self,
        symbol: &'a str,
        split: &'a Split,
        state: &ScopeState<'_>,
        depth: u32,
    ) -> Result<(), ExpandError> {
        let world_axis = state.orient.axis(split.axis);
        let extent = state.region.extent(world_axis);

        let scope = state.scope();
        let mut sizes = Vec::with_capacity(split.sizes.len());
        // The evaluated pattern, kept beside the resolved sizes so a refusal
        // can show which size expression contributed what.
        let mut resolved: Vec<(String, bool, i64)> = Vec::with_capacity(split.sizes.len());
        for size in &split.sizes {
            let (expr, absolute) = match size {
                Size::Absolute { blocks } => (blocks, true),
                Size::Relative { weight } => (weight, false),
            };
            let value = scope.eval(expr).map_err(|error| ExpandError::Eval {
                symbol: symbol.to_string(),
                error,
                expr: render_expr(expr),
                scope: Box::new(state.at()),
                path: Vec::new(),
            })?;
            let bad = if absolute { value < 0 } else { value <= 0 };
            if bad || value > u32::MAX as i64 {
                return Err(ExpandError::BadSize {
                    symbol: symbol.to_string(),
                    value,
                    expr: render_expr(expr),
                    scope: Box::new(state.at()),
                    path: Vec::new(),
                });
            }
            resolved.push((render_expr(expr), absolute, value));
            sizes.push(if absolute {
                ResolvedSize::Absolute(value as u32)
            } else {
                ResolvedSize::Relative(value as u32)
            });
        }

        let pieces = make_split(extent, &sizes, split.rounding, split.repeat).map_err(|error| {
            ExpandError::Split {
                symbol: symbol.to_string(),
                error,
                axis: split.axis,
                world_axis,
                pattern: resolved
                    .iter()
                    .map(|(expr, absolute, value)| {
                        let kind = if *absolute { "abs" } else { "rel" };
                        // A literal already shows its own value; an expression
                        // shows both the authored form and what it came to.
                        if *expr == value.to_string() {
                            format!("{kind} {value}")
                        } else {
                            format!("{kind} {expr} = {value}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                scope: Box::new(state.at()),
                path: Vec::new(),
            }
        })?;

        // The child orientation is computed once, from the parent box, exactly
        // as upstream does (`assignment` is taken before the pieces are cut).
        let child_orient = reorient(
            state.orient,
            state.region.size,
            &split.orient,
            Some(split.axis),
        )
        .map_err(|error| ExpandError::Orient {
            symbol: symbol.to_string(),
            error,
            scope: Box::new(state.at()),
            path: Vec::new(),
        })?;

        // The frame set is a fact about the request and the parent, not about
        // any one piece, so it is computed once rather than per piece.
        let (reachable, reframed) = reframed_by(state, symbol, &split.orient, Some(split.axis));

        // The pattern is laid out along the LOCAL axis, from its low end. In an
        // unreflected frame that is the world low end; in a reflected one the
        // first piece is the world-highest, so the same rule puts its end wall
        // on the outside of both arms of a transept. The pieces are also
        // *visited* in pattern order either way, so expansion order stays
        // reading order — which is what `mark`'s auto-numbering counts in.
        let axis = world_axis.index();
        let piece_count = pieces.len();
        let reversed = state.orient.reversed(split.axis);
        let mut cursor = if reversed {
            state.region.origin[axis] + state.region.size[axis] as i32
        } else {
            state.region.origin[axis]
        };
        for (i, piece) in pieces.iter().enumerate() {
            let mut origin = state.region.origin;
            let mut size = state.region.size;
            if reversed {
                cursor -= *piece as i32;
                origin[axis] = cursor;
            } else {
                origin[axis] = cursor;
                cursor += *piece as i32;
            }
            size[axis] = *piece;
            let child_state = ScopeState {
                region: Box3::new(origin, size),
                orient: child_orient,
                env: state.env,
                pinned: state.pinned,
                reachable,
                reframed,
            };
            let child = &split.children[i % split.children.len()];
            self.run_node(symbol, child, &child_state, depth + 1)
                .map_err(|e| {
                    e.pushed(|| {
                        format!(
                            "split {}\u{2192}{} piece {}/{piece_count}",
                            axis_name(split.axis),
                            axis_name(world_axis),
                            i + 1
                        )
                    })
                })?;
        }
        Ok(())
    }
}
