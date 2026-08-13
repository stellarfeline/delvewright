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
use crate::geom::{Axis, Box3, Orientation};
use crate::ir::{
    Alternative, Bar, Cond, Envelope, Facing, Mark, MarkAt, Material, Node, Paint, Program,
    ProgramError, Side, Size, Split,
};
use crate::model::{PaletteFull, VoxelModel};
use crate::orient::{OrientError, reorient};
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
    },
    /// A size or guard expression could not be evaluated.
    Eval {
        /// The rule being expanded.
        symbol: String,
        /// The failure.
        error: EvalError,
    },
    /// A size pattern could not be laid out.
    Split {
        /// The rule being expanded.
        symbol: String,
        /// The failure.
        error: SplitError,
    },
    /// A reorientation request was contradictory.
    Orient {
        /// The rule being expanded.
        symbol: String,
        /// The failure.
        error: OrientError,
    },
    /// An absolute size evaluated negative, or a relative weight non-positive.
    BadSize {
        /// The rule being expanded.
        symbol: String,
        /// The offending value.
        value: i64,
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
            ExpandError::NoApplicableRule { symbol } => write!(
                f,
                "no alternative of rule {symbol:?} applies to this scope, and none is `otherwise`"
            ),
            ExpandError::Eval { symbol, error } => write!(f, "rule {symbol:?}: {error}"),
            ExpandError::Split { symbol, error } => match error {
                SplitError::Overflow { absolute, extent } => write!(
                    f,
                    "rule {symbol:?}: split needs {absolute} blocks but the scope is {extent} \
                     across — the region is too small for this rule"
                ),
                SplitError::ZeroStride => write!(
                    f,
                    "rule {symbol:?}: a repeating split whose pattern consumes nothing"
                ),
            },
            ExpandError::Orient { symbol, error } => write!(f, "rule {symbol:?}: {error}"),
            ExpandError::BadSize { symbol, value } => {
                write!(f, "rule {symbol:?}: {value} is not a usable split size")
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
            ExpandError::PaletteFull { symbol, error } => write!(f, "rule {symbol:?}: {error}"),
        }
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
        block: match program.palette.get(&b.block) {
            Some(Paint::Block(state)) => state.clone(),
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
            let ok = state
                .scope()
                .test(&alt.when)
                .map_err(|error| ExpandError::Eval {
                    symbol: symbol.to_string(),
                    error,
                })?;
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
            return Err(ExpandError::NoApplicableRule {
                symbol: symbol.to_string(),
            });
        }

        let weights: Vec<u32> = candidates.iter().map(|&i| alts[i].weight).collect();
        let picked = candidates[self
            .rng
            .weighted(&weights)
            .expect("validate() proved every weight is positive")];
        self.stats.rules_applied += 1;
        self.run_node(symbol, &alts[picked].body, state, depth + 1)
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
                let full = |error| ExpandError::PaletteFull {
                    symbol: symbol.to_string(),
                    error,
                };
                match paint {
                    Paint::Block(block) => {
                        let block = block.clone();
                        for pos in state.region.positions() {
                            self.model.set(pos, &block).map_err(full)?;
                        }
                    }
                    Paint::Mix(mix) => {
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
            Node::Reorient { orient, body } => {
                let orient =
                    reorient(state.orient, state.region.size, orient, None).map_err(|error| {
                        ExpandError::Orient {
                            symbol: symbol.to_string(),
                            error,
                        }
                    })?;
                let child = ScopeState {
                    region: state.region,
                    orient,
                    env: state.env,
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
                    let value = state
                        .scope()
                        .eval(value)
                        .map_err(|error| ExpandError::Eval {
                            symbol: symbol.to_string(),
                            error,
                        })?;
                    bound_params.insert(name.clone(), value);
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
            // A permutation cannot mirror, so a derived facing is always the
            // negative direction of the world axis the scope calls local Z.
            None => match state.orient.get(Axis::Z) {
                Axis::Z => Facing::North,
                Axis::X => Facing::West,
                Axis::Y => {
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
        // Centre of an extent, rounding down; 0 for a degenerate axis, which the
        // bounds check below then reports.
        let mid = |axis: Axis| (size[axis.index()].saturating_sub(1) / 2) as i64;

        // Offsets from the scope's minimum corner, per WORLD axis.
        let mut delta = [0i64; 3];
        match &mark.at {
            MarkAt::CornerMin => {}
            MarkAt::FloorCenter => {
                delta[Axis::X.index()] = mid(Axis::X);
                delta[Axis::Y.index()] = 0;
                delta[Axis::Z.index()] = mid(Axis::Z);
            }
            MarkAt::FaceCenter { axis, side } => {
                let pinned = state.orient.get(*axis);
                for world in Axis::ALL {
                    delta[world.index()] = if world == pinned {
                        match side {
                            Side::Min => 0,
                            Side::Max => (size[world.index()].saturating_sub(1)) as i64,
                        }
                    } else {
                        mid(world)
                    };
                }
            }
            MarkAt::Offset { x, y, z } => {
                let scope = state.scope();
                for (local, expr) in [(Axis::X, x), (Axis::Y, y), (Axis::Z, z)] {
                    let value = scope.eval(expr).map_err(|error| ExpandError::Eval {
                        symbol: symbol.to_string(),
                        error,
                    })?;
                    delta[state.orient.get(local).index()] = value;
                }
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
        let world_axis = state.orient.get(split.axis);
        let extent = state.region.extent(world_axis);

        let scope = state.scope();
        let mut sizes = Vec::with_capacity(split.sizes.len());
        for size in &split.sizes {
            let (expr, absolute) = match size {
                Size::Absolute { blocks } => (blocks, true),
                Size::Relative { weight } => (weight, false),
            };
            let value = scope.eval(expr).map_err(|error| ExpandError::Eval {
                symbol: symbol.to_string(),
                error,
            })?;
            let bad = if absolute { value < 0 } else { value <= 0 };
            if bad || value > u32::MAX as i64 {
                return Err(ExpandError::BadSize {
                    symbol: symbol.to_string(),
                    value,
                });
            }
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
        })?;

        let axis = world_axis.index();
        let mut cursor = state.region.origin[axis];
        for (i, piece) in pieces.iter().enumerate() {
            let mut origin = state.region.origin;
            let mut size = state.region.size;
            origin[axis] = cursor;
            size[axis] = *piece;
            cursor += *piece as i32;
            let child_state = ScopeState {
                region: Box3::new(origin, size),
                orient: child_orient,
                env: state.env,
            };
            let child = &split.children[i % split.children.len()];
            self.run_node(symbol, child, &child_state, depth + 1)?;
        }
        Ok(())
    }
}
