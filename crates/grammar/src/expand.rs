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
use crate::eval::{EvalError, Scope};
use crate::geom::{Axis, Box3, Orientation};
use crate::ir::{
    Alternative, Cond, Facing, Mark, MarkAt, Material, Node, Paint, Program, ProgramError, Side,
    Size, Split,
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
        rng: Rng::new(options.seed),
        limits: options.limits,
        stats: Stats::default(),
    };
    expander.run_rule(
        &program.start,
        &ScopeState {
            region,
            orient: options.orientation,
        },
        0,
    )?;
    Ok(Expansion {
        model: expander.model,
        anchors: expander.anchors,
        stats: expander.stats,
    })
}

/// The box a node is expanding into, and what it calls its axes.
#[derive(Debug, Clone, Copy)]
struct ScopeState {
    region: Box3,
    orient: Orientation,
}

struct Expander<'a> {
    program: &'a Program,
    model: VoxelModel,
    anchors: BTreeMap<String, Anchor>,
    /// Per-stem occurrence counter, so [`crate::ir::MarkIndex::Auto`] numbers in
    /// expansion order.
    marks_seen: BTreeMap<String, u32>,
    rng: Rng,
    limits: Limits,
    stats: Stats,
}

impl<'a> Expander<'a> {
    fn scope<'s>(&self, state: &'s ScopeState) -> Scope<'s>
    where
        'a: 's,
    {
        Scope {
            region: &state.region,
            orient: state.orient,
            params: &self.program.params,
        }
    }

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
            let ok = self
                .scope(state)
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
        state: &ScopeState,
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
                    Material::Role { role } => self
                        .program
                        .palette
                        .get(role)
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
                };
                self.run_node(symbol, body, &child, depth + 1)
            }
            Node::Mark { mark, body } => {
                // The mark first, so a nested mark of the same stem numbers
                // after it: expansion order is reading order.
                self.declare(symbol, mark, state)?;
                self.run_node(symbol, body, state, depth + 1)
            }
            Node::Split(split) => self.run_split(symbol, split, state, depth),
        }
    }

    /// Resolve one [`Node::Mark`] against the scope it sits on and record it.
    fn declare(
        &mut self,
        symbol: &str,
        mark: &Mark,
        state: &ScopeState,
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
        state: &ScopeState,
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
                let scope = self.scope(state);
                for (l, expr) in [(Axis::X, x), (Axis::Y, y), (Axis::Z, z)] {
                    let value = scope.eval(expr).map_err(|error| ExpandError::Eval {
                        symbol: symbol.to_string(),
                        error,
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
        state: &ScopeState,
        depth: u32,
    ) -> Result<(), ExpandError> {
        let world_axis = state.orient.axis(split.axis);
        let extent = state.region.extent(world_axis);

        let scope = self.scope(state);
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

        // The pattern is laid out along the LOCAL axis, from its low end. In an
        // unreflected frame that is the world low end; in a reflected one the
        // first piece is the world-highest, so the same rule puts its end wall
        // on the outside of both arms of a transept. The pieces are also
        // *visited* in pattern order either way, so expansion order stays
        // reading order — which is what `mark`'s auto-numbering counts in.
        let axis = world_axis.index();
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
            };
            let child = &split.children[i % split.children.len()];
            self.run_node(symbol, child, &child_state, depth + 1)?;
        }
        Ok(())
    }
}
