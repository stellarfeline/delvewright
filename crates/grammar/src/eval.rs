//! Evaluating split sizes and rule guards against a scope — **and saying what a
//! guard measured when it declines.**
//!
//! Ported from `Comparer` / `CompositeComparer` / `Constraint` and
//! `Scope.get_value` in `SplitGrammar.py` (`yawgmoth/GDMC25`, BSD-3-Clause —
//! see `LICENSE-GDMC25`). Upstream builds the same expression tree out of
//! Python operator overloads; here it is plain data, so a guard can be
//! serialised, diffed and checked before anything is expanded.
//!
//! # Why the reporting lives here
//!
//! A guard refusing is the most informative event an author can get out of the
//! grammar: it is the program stating, in its own terms, what the scope would
//! have to be. [`Scope::test`] throws that away — it answers `bool` — so every
//! consumer that surfaced a refusal (`delve-grammar sweep`, the zone gates, any
//! future driver) could only say *that* nothing applied. `bell:chapel-ward`'s
//! frame guard is a four-clause conjunction; candidates breaking different
//! clauses of it all printed the same sentence, so the next candidate was a
//! guess in every case.
//!
//! So the explanation belongs to **guard evaluation**, not to the command that
//! happened to surface it (CLAUDE.md: a capability belongs to the object class
//! it acts on). [`Scope::explain`] measures a guard into a [`CondTrace`] — every
//! clause, what each side of it came to, what it is built from, and how far the
//! scope is from satisfying it — and [`GuardRefusal`] is that for a whole rule.
//! An expansion refused anywhere therefore says the same thing.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

use crate::geom::{Axis, Box3, Orientation};
use crate::ir::{Alternative, ArithOp, CmpOp, Cond, DimRef, Expr};

/// What a guard or size expression is measured against.
#[derive(Debug, Clone, Copy)]
pub struct Scope<'a> {
    /// The scope's world-space box.
    pub region: &'a Box3,
    /// Which world axis each local axis names.
    pub orient: Orientation,
    /// The program's parameters.
    pub params: &'a BTreeMap<String, i64>,
}

/// Why an expression could not be evaluated. Missing names are impossible here
/// — [`crate::ir::Program::validate`] rejects them before expansion — but they
/// are still reported rather than assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// Division or remainder by zero.
    DivideByZero,
    /// An undeclared parameter survived validation.
    UnknownParam {
        /// Parameter name.
        name: String,
    },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::DivideByZero => write!(f, "division or remainder by zero"),
            EvalError::UnknownParam { name } => {
                write!(f, "parameter {name:?} is not declared by this program")
            }
        }
    }
}

impl std::error::Error for EvalError {}

impl<'a> Scope<'a> {
    /// Measure one dimension, in blocks.
    pub fn dim(&self, dim: DimRef) -> i64 {
        let world = |axis: Axis| self.region.extent(axis) as i64;
        match dim {
            DimRef::X => world(self.orient.x),
            DimRef::Y => world(self.orient.y),
            DimRef::Z => world(self.orient.z),
            DimRef::WorldX => world(Axis::X),
            DimRef::WorldY => world(Axis::Y),
            DimRef::WorldZ => world(Axis::Z),
            DimRef::Smallest => self.region.size.iter().min().copied().unwrap_or(0) as i64,
            DimRef::Largest => self.region.size.iter().max().copied().unwrap_or(0) as i64,
        }
    }

    /// Evaluate an integer expression.
    pub fn eval(&self, expr: &Expr) -> Result<i64, EvalError> {
        match expr {
            Expr::Int { value } => Ok(*value),
            Expr::Dim { dim } => Ok(self.dim(*dim)),
            Expr::Param { name } => self
                .params
                .get(name)
                .copied()
                .ok_or_else(|| EvalError::UnknownParam { name: name.clone() }),
            Expr::Arith { lhs, op, rhs } => {
                let a = self.eval(lhs)?;
                let b = self.eval(rhs)?;
                Ok(match op {
                    ArithOp::Add => a.saturating_add(b),
                    ArithOp::Sub => a.saturating_sub(b),
                    ArithOp::Mul => a.saturating_mul(b),
                    ArithOp::Div => {
                        if b == 0 {
                            return Err(EvalError::DivideByZero);
                        }
                        a.div_euclid(b)
                    }
                    ArithOp::Rem => {
                        if b == 0 {
                            return Err(EvalError::DivideByZero);
                        }
                        // Euclidean: `Dimension.Z % 2 == 0` must mean "even" for
                        // every value, which `%` on negatives would not give.
                        a.rem_euclid(b)
                    }
                    ArithOp::Max => a.max(b),
                    ArithOp::Min => a.min(b),
                })
            }
        }
    }

    /// Evaluate a guard. [`Cond::Otherwise`] is always false here: rule
    /// selection gives it its meaning (see [`crate::expand`]).
    pub fn test(&self, cond: &Cond) -> Result<bool, EvalError> {
        Ok(match cond {
            Cond::Always => true,
            Cond::Otherwise => false,
            Cond::Cmp { lhs, op, rhs } => {
                let a = self.eval(lhs)?;
                let b = self.eval(rhs)?;
                match op {
                    CmpOp::Lt => a < b,
                    CmpOp::Le => a <= b,
                    CmpOp::Gt => a > b,
                    CmpOp::Ge => a >= b,
                    CmpOp::Eq => a == b,
                    CmpOp::Ne => a != b,
                }
            }
            Cond::All { of } => {
                for c in of {
                    if !self.test(c)? {
                        return Ok(false);
                    }
                }
                true
            }
            Cond::Any { of } => {
                for c in of {
                    if self.test(c)? {
                        return Ok(true);
                    }
                }
                false
            }
            Cond::NoneOf { of } => {
                for c in of {
                    if self.test(c)? {
                        return Ok(false);
                    }
                }
                true
            }
            Cond::Orientation { x, y, z } => {
                self.orient.x == *x && self.orient.y == *y && self.orient.z == *z
            }
        })
    }

    /// The box this scope is, as a guard reads it.
    pub fn facts(&self) -> ScopeFacts {
        ScopeFacts {
            origin: self.region.origin,
            size: self.region.size,
            dims: [
                self.local_dim(Axis::X, DimRef::X),
                self.local_dim(Axis::Y, DimRef::Y),
                self.local_dim(Axis::Z, DimRef::Z),
            ],
        }
    }

    fn local_dim(&self, local: Axis, dim: DimRef) -> LocalDim {
        LocalDim {
            local,
            world: self.orient.get(local),
            blocks: self.dim(dim),
        }
    }

    /// Measure one side of a comparison: what it reads as, what it came to, and
    /// the named quantities it was built from.
    pub fn measure(&self, expr: &Expr) -> Result<Operand, EvalError> {
        let value = self.eval(expr)?;
        let inputs = if expr.is_leaf() {
            // A bare name or a literal: the rendering already says everything,
            // and `strip_depth = 9 from strip_depth = 9` is noise.
            Vec::new()
        } else {
            let mut out = Vec::new();
            for name in expr.inputs() {
                let value = match name.as_str() {
                    n if n.starts_with("Dimension.") => self.dim(dim_named(n)),
                    n => self.eval(&Expr::param(n))?,
                };
                out.push(Input { name, value });
            }
            out
        };
        Ok(Operand {
            source: expr.to_string(),
            value,
            inputs,
        })
    }

    /// Measure a guard: the same verdict [`Scope::test`] reaches, with every
    /// clause's own reading kept.
    ///
    /// **Reporting never fails.** `test` short-circuits — an `all` stops at its
    /// first false clause — so a later clause may divide by zero without that
    /// ever having decided anything. Propagating such an error out of a *report*
    /// would replace the diagnostic an author needs with one about a clause that
    /// was never consulted, so an unreadable clause becomes a
    /// [`CondTrace::Unreadable`] node that says so and counts as not holding —
    /// which is exactly the weight `test` gave it. `explain(c).holds()` and
    /// `test(c)` therefore agree wherever `test` returns at all
    /// (`explain_agrees_with_test`).
    pub fn explain(&self, cond: &Cond) -> CondTrace {
        match cond {
            Cond::Always => CondTrace::Always,
            Cond::Otherwise => CondTrace::Otherwise,
            Cond::Cmp { lhs, op, rhs } => match (self.measure(lhs), self.measure(rhs)) {
                (Ok(l), Ok(r)) => CondTrace::Cmp(CmpFact::new(l, *op, r)),
                (Err(e), _) | (_, Err(e)) => CondTrace::Unreadable {
                    source: format!("{lhs} {op} {rhs}"),
                    reason: e.to_string(),
                },
            },
            Cond::All { of } => CondTrace::All {
                of: of.iter().map(|c| self.explain(c)).collect(),
            },
            Cond::Any { of } => CondTrace::Any {
                of: of.iter().map(|c| self.explain(c)).collect(),
            },
            Cond::NoneOf { of } => CondTrace::NoneOf {
                of: of.iter().map(|c| self.explain(c)).collect(),
            },
            Cond::Orientation { x, y, z } => CondTrace::Orientation {
                want: [*x, *y, *z],
                got: self.orient.axes(),
            },
        }
    }
}

/// The [`DimRef`] a rendered `Dimension.…` names. Total over
/// [`DimRef::as_str`]'s own output, which is the only thing that reaches it.
fn dim_named(rendered: &str) -> DimRef {
    for dim in [
        DimRef::X,
        DimRef::Y,
        DimRef::Z,
        DimRef::WorldX,
        DimRef::WorldY,
        DimRef::WorldZ,
        DimRef::Smallest,
        DimRef::Largest,
    ] {
        if dim.as_str() == rendered {
            return dim;
        }
    }
    unreachable!("every rendered dimension comes from DimRef::as_str")
}

// ---------------------------------------------------------------------------
// What a guard measured
// ---------------------------------------------------------------------------

/// One of a scope's three local dimensions, as its own rule reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LocalDim {
    /// The local axis: what the rule calls it.
    pub local: Axis,
    /// The world axis it names in this scope.
    pub world: Axis,
    /// Its extent, in blocks.
    pub blocks: i64,
}

/// The box a guard was measured against.
///
/// Both halves matter to an author: the world box says how big the piece is,
/// and the local mapping says which of its numbers the rule's own
/// `Dimension.X` is — a rule reasons in the frame it was handed, so the same
/// box satisfies a guard under one orientation and refuses it under another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ScopeFacts {
    /// Minimum corner, world space.
    pub origin: [i32; 3],
    /// Extents along world `X`, `Y`, `Z`.
    pub size: [u32; 3],
    /// What `Dimension.X`, `.Y` and `.Z` measured here.
    pub dims: [LocalDim; 3],
}

impl fmt::Display for ScopeFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{}x{} at {},{},{}; the rule reads ",
            self.size[0],
            self.size[1],
            self.size[2],
            self.origin[0],
            self.origin[1],
            self.origin[2]
        )?;
        for (i, d) in self.dims.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(
                f,
                "Dimension.{} = {} (world {})",
                d.local, d.blocks, d.world
            )?;
        }
        Ok(())
    }
}

/// One named quantity a guard operand was measured from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Input {
    /// The name as the expression spells it (`strip_depth`, `Dimension.X`).
    pub name: String,
    /// What it was in this scope.
    pub value: i64,
}

/// One side of a comparison, as written and as measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Operand {
    /// The expression as a grammar author reads it.
    pub source: String,
    /// What it came to in this scope.
    pub value: i64,
    /// The named quantities `source` is built from, in reading order, each
    /// once.
    ///
    /// **Empty when the operand is one name or one literal**, where the number
    /// alone is already actionable. A *derived* operand is what this field
    /// exists for: told only that `10 > 7` is false, an author knows the
    /// distance but not which of the four knobs feeding those two numbers to
    /// turn, and the guard is the only thing that knows.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Input>,
}

/// What it would take for a comparison that came out false to hold.
///
/// Stated from **both** sides because either can move: a scope's dimensions are
/// the region a caller passes and the parameters are the program's own knobs,
/// and which of the two an author may touch is not something the guard knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Shortfall {
    /// The smallest whole-block move, on either side, that flips the verdict.
    pub blocks: i64,
    /// What the left side must reach with the right held where it is.
    pub lhs_must_reach: i64,
    /// What the right side must reach with the left held where it is.
    pub rhs_must_reach: i64,
}

/// One comparison of a guard, measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CmpFact {
    /// The left side.
    pub lhs: Operand,
    /// The operator.
    pub op: CmpOp,
    /// The right side.
    pub rhs: Operand,
    /// Whether it held in this scope.
    pub holds: bool,
    /// How far off it was; `None` when it held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortfall: Option<Shortfall>,
}

impl CmpFact {
    /// Compare two measured operands.
    pub fn new(lhs: Operand, op: CmpOp, rhs: Operand) -> CmpFact {
        let (a, b) = (lhs.value, rhs.value);
        let holds = match op {
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
        };
        // The nearest satisfying value for each side with the other held fixed.
        // `!=` is the one case with two equally near answers (a false `!=` means
        // the sides are equal, so either may move by one); it takes the upward
        // one, deliberately and always the same way.
        let shortfall = (!holds).then(|| {
            let (lhs_must_reach, rhs_must_reach) = match op {
                CmpOp::Lt => (b.saturating_sub(1), a.saturating_add(1)),
                CmpOp::Le | CmpOp::Ge | CmpOp::Eq => (b, a),
                CmpOp::Gt => (b.saturating_add(1), a.saturating_sub(1)),
                CmpOp::Ne => (a.saturating_add(1), b.saturating_add(1)),
            };
            Shortfall {
                blocks: lhs_must_reach.saturating_sub(a).abs(),
                lhs_must_reach,
                rhs_must_reach,
            }
        });
        CmpFact {
            lhs,
            op,
            rhs,
            holds,
            shortfall,
        }
    }

    /// The comparison as it is written: `hearth_run > Dimension.X - strip_depth`.
    pub fn source(&self) -> String {
        format!("{} {} {}", self.lhs.source, self.op, self.rhs.source)
    }
}

/// A guard, measured against one scope: the shape of [`Cond`] with every clause
/// carrying what it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "cond", rename_all = "snake_case")]
pub enum CondTrace {
    /// [`Cond::Always`].
    Always,
    /// [`Cond::Otherwise`] — never holds of its own accord; rule selection gives
    /// it its meaning.
    Otherwise,
    /// A comparison.
    Cmp(CmpFact),
    /// Every clause must hold.
    All {
        /// The clauses, in declaration order.
        of: Vec<CondTrace>,
    },
    /// At least one clause must hold.
    Any {
        /// The clauses, in declaration order.
        of: Vec<CondTrace>,
    },
    /// No clause may hold.
    NoneOf {
        /// The clauses, in declaration order.
        of: Vec<CondTrace>,
    },
    /// The scope's axis naming, wanted against measured.
    Orientation {
        /// The mapping the guard asks for, local `X`/`Y`/`Z` to world axis.
        want: [Axis; 3],
        /// The mapping the scope has.
        got: [Axis; 3],
    },
    /// A clause that could not be measured — see [`Scope::explain`].
    Unreadable {
        /// The clause as written.
        source: String,
        /// Why it could not be read.
        reason: String,
    },
}

impl CondTrace {
    /// The verdict, identical to [`Scope::test`]'s wherever that returns.
    pub fn holds(&self) -> bool {
        match self {
            CondTrace::Always => true,
            CondTrace::Otherwise | CondTrace::Unreadable { .. } => false,
            CondTrace::Cmp(fact) => fact.holds,
            CondTrace::All { of } => of.iter().all(|c| c.holds()),
            CondTrace::Any { of } => of.iter().any(|c| c.holds()),
            CondTrace::NoneOf { of } => !of.iter().any(|c| c.holds()),
            CondTrace::Orientation { want, got } => want == got,
        }
    }

    /// Write the guard as a checklist an author can act on.
    ///
    /// `want` is what this node had to do — hold, or (under a `none_of`) not
    /// hold. A clause that did what was wanted is one line with its numbers, so
    /// the author can see the headroom on the clauses they must not break while
    /// fixing the one they must; a clause that did not is opened up.
    fn write(&self, out: &mut String, indent: &str, want: bool) {
        let culprit = self.holds() != want;
        match self {
            CondTrace::All { of } => {
                let bad = of.iter().filter(|c| !c.holds()).count();
                out.push_str(&format!(
                    "every clause must hold; {bad} of {} {} not:\n",
                    of.len(),
                    does(bad)
                ));
                write_clauses(out, indent, of, true);
            }
            CondTrace::Any { of } => {
                out.push_str(&format!(
                    "at least one clause must hold; none of the {} does:\n",
                    of.len()
                ));
                write_clauses(out, indent, of, true);
            }
            CondTrace::NoneOf { of } => {
                let bad = of.iter().filter(|c| c.holds()).count();
                out.push_str(&format!(
                    "no clause may hold; {bad} of {} {}:\n",
                    of.len(),
                    does(bad)
                ));
                write_clauses(out, indent, of, false);
            }
            // A guard that is one clause reads as one clause, with the same
            // marker and the same reading as it would inside a composite —
            // an author should not have to learn two layouts.
            _ => {
                out.push_str(if culprit {
                    "its one clause does not hold:\n"
                } else {
                    "its one clause holds:\n"
                });
                write_clauses(out, indent, std::slice::from_ref(self), want);
            }
        }
    }

    /// The clause as it is written in the program.
    fn key(&self) -> String {
        match self {
            CondTrace::Always => "always".to_string(),
            CondTrace::Otherwise => {
                "otherwise (stands in only when nothing else applies)".to_string()
            }
            CondTrace::Cmp(fact) => fact.source(),
            CondTrace::Orientation { want, .. } => {
                format!("orientation is {},{},{}", want[0], want[1], want[2])
            }
            CondTrace::Unreadable { source, .. } => source.clone(),
            CondTrace::All { of } => format!("all of {} clauses", of.len()),
            CondTrace::Any { of } => format!("any of {} clauses", of.len()),
            CondTrace::NoneOf { of } => format!("none of {} clauses", of.len()),
        }
    }

    /// What the clause came to here, when that is a thing to state.
    fn measured(&self) -> Option<String> {
        match self {
            CondTrace::Cmp(fact) => {
                Some(format!("{} {} {}", fact.lhs.value, fact.op, fact.rhs.value))
            }
            CondTrace::Orientation { got, .. } => {
                Some(format!("the scope's is {},{},{}", got[0], got[1], got[2]))
            }
            CondTrace::Unreadable { reason, .. } => {
                Some(format!("cannot be measured here: {reason}"))
            }
            _ => None,
        }
    }

    /// The one-line form: the clause as written, then what it came to, in a
    /// column `pad` wide so a reader compares the numbers down the page.
    fn headline(&self, pad: usize) -> String {
        match self.measured() {
            Some(m) => format!("{:pad$}  {m}", self.key()),
            None => self.key(),
        }
    }

    /// The reading of a clause that did not do what the guard needed: each
    /// side's value, what it is built from, and the distance to satisfaction.
    fn write_detail(&self, out: &mut String, indent: &str) {
        let CondTrace::Cmp(fact) = self else {
            return;
        };
        for (side, operand) in [("left ", &fact.lhs), ("right", &fact.rhs)] {
            out.push_str(indent);
            out.push_str(&format!("    {side} = {}", operand.value));
            if !operand.inputs.is_empty() {
                let from: Vec<String> = operand
                    .inputs
                    .iter()
                    .map(|i| format!("{} = {}", i.name, i.value))
                    .collect();
                out.push_str(&format!("   from {}", from.join(", ")));
            }
            out.push('\n');
        }
        if let Some(s) = fact.shortfall {
            let toward = |from: i64, to: i64| if to > from { "rise to" } else { "fall to" };
            out.push_str(indent);
            out.push_str(&format!(
                "    {} short: the left must {} {}, or the right {} {}\n",
                s.blocks,
                toward(fact.lhs.value, s.lhs_must_reach),
                s.lhs_must_reach,
                toward(fact.rhs.value, s.rhs_must_reach),
                s.rhs_must_reach
            ));
        }
    }
}

impl fmt::Display for CondTrace {
    /// The checklist on its own, for a caller that has a guard and a scope but
    /// no rule around them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        self.write(&mut out, "", true);
        f.write_str(out.trim_end_matches('\n'))
    }
}

/// `does` / `do`, for a count.
fn does(n: usize) -> &'static str {
    if n == 1 { "does" } else { "do" }
}

/// Write a composite's clauses, marking the ones that decided the verdict.
fn write_clauses(out: &mut String, indent: &str, of: &[CondTrace], want: bool) {
    // Every clause's numbers in one column, so the reader compares them by
    // eye. Capped so one enormous clause cannot push the rest off the screen.
    let pad = of.iter().map(|c| c.key().len()).max().unwrap_or(0).min(72);
    for clause in of {
        let culprit = clause.holds() != want;
        out.push_str(indent);
        out.push_str("  ");
        out.push_str(if culprit {
            if want { "FALSE  " } else { "HELD   " }
        } else {
            "ok     "
        });
        match clause {
            CondTrace::All { .. } | CondTrace::Any { .. } | CondTrace::NoneOf { .. } => {
                let nested = format!("{indent}    ");
                clause.write(out, &nested, want);
            }
            _ => {
                out.push_str(clause.headline(pad).trim_end());
                out.push('\n');
                if culprit {
                    clause.write_detail(out, indent);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A whole rule declining a scope
// ---------------------------------------------------------------------------

/// One alternative's guard, measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardedAlternative {
    /// Position in the rule's declaration order, counting from 1 — the number
    /// an author counts down the rule to find.
    pub index: usize,
    /// The guard, as it read this scope.
    pub guard: CondTrace,
}

/// Why every alternative of one rule declined one scope.
///
/// The record a refusal is worth: the box, and per alternative every clause of
/// its guard with both sides measured and the distance to satisfaction. It is
/// [`Serialize`] as well as [`Display`] because a refusal a human can read and a
/// tool cannot is half a diagnostic — `sweep.json` carries this per refused row,
/// so a driver can rank, group or auto-widen candidates without parsing prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuardRefusal {
    /// The rule that declined.
    pub symbol: String,
    /// The box it was handed.
    pub scope: ScopeFacts,
    /// Every alternative, in declaration order.
    pub alternatives: Vec<GuardedAlternative>,
}

impl GuardRefusal {
    /// Measure every alternative of a rule against the scope that refused it.
    ///
    /// Runs only on the refusal path — the happy path never pays for it — and
    /// cannot itself fail, so a diagnostic is never lost to a second defect.
    pub fn of(symbol: &str, scope: &Scope<'_>, alts: &[Alternative]) -> GuardRefusal {
        GuardRefusal {
            symbol: symbol.to_string(),
            scope: scope.facts(),
            alternatives: alts
                .iter()
                .enumerate()
                .map(|(i, alt)| GuardedAlternative {
                    index: i + 1,
                    guard: scope.explain(&alt.when),
                })
                .collect(),
        }
    }
}

impl fmt::Display for GuardRefusal {
    /// The author-facing report, every line indented four spaces so it reads
    /// under whatever sentence introduced it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&format!("    scope: {}\n", self.scope));
        let total = self.alternatives.len();
        for alt in &self.alternatives {
            out.push_str(&format!("    alternative {} of {total} — ", alt.index,));
            alt.guard.write(&mut out, "    ", true);
        }
        f.write_str(out.trim_end_matches('\n'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> BTreeMap<String, i64> {
        BTreeMap::from([("column_height".to_string(), 8)])
    }

    fn scope<'a>(
        region: &'a Box3,
        params: &'a BTreeMap<String, i64>,
        orient: Orientation,
    ) -> Scope<'a> {
        Scope {
            region,
            orient,
            params,
        }
    }

    #[test]
    fn local_dimensions_read_through_the_orientation() {
        let region = Box3::at_origin([3, 7, 11]);
        let p = params();
        let rotated = Orientation {
            x: Axis::Z,
            y: Axis::Y,
            z: Axis::X,
            reversed: [false; 3],
        };
        let s = scope(&region, &p, rotated);
        assert_eq!(s.dim(DimRef::X), 11);
        assert_eq!(s.dim(DimRef::Z), 3);
        assert_eq!(
            s.dim(DimRef::WorldX),
            3,
            "world dimensions ignore the rotation"
        );
        assert_eq!(s.dim(DimRef::Smallest), 3);
        assert_eq!(
            s.dim(DimRef::Largest),
            11,
            "upstream's get_value returned min() here — a copy/paste bug"
        );
    }

    #[test]
    fn arithmetic_and_comparisons() {
        let region = Box3::at_origin([4, 9, 10]);
        let p = params();
        let s = scope(&region, &p, Orientation::IDENTITY);
        // The castle's `Dimension.Z % 2 == 0` parity guard.
        assert!(
            s.test(&Cond::cmp(
                Expr::dim(DimRef::Z).arith(ArithOp::Rem, Expr::int(2)),
                CmpOp::Eq,
                Expr::int(0)
            ))
            .unwrap()
        );
        // A parameter in a guard: the size controls are real inputs.
        assert!(
            s.test(&Cond::cmp(
                Expr::dim(DimRef::Y),
                CmpOp::Ge,
                Expr::param("column_height")
            ))
            .unwrap()
        );
        assert_eq!(
            s.eval(&Expr::int(7).arith(ArithOp::Div, Expr::int(0))),
            Err(EvalError::DivideByZero)
        );
        assert_eq!(
            s.eval(&Expr::param("nope")),
            Err(EvalError::UnknownParam {
                name: "nope".into()
            })
        );
    }

    #[test]
    fn composite_guards() {
        let region = Box3::at_origin([4, 9, 10]);
        let p = params();
        let s = scope(&region, &p, Orientation::IDENTITY);
        let even_x = Cond::cmp(
            Expr::dim(DimRef::X).arith(ArithOp::Rem, Expr::int(2)),
            CmpOp::Eq,
            Expr::int(0),
        );
        let tall = Cond::cmp(Expr::dim(DimRef::Y), CmpOp::Gt, Expr::int(100));
        assert!(
            s.test(&Cond::Any {
                of: vec![even_x.clone(), tall.clone()]
            })
            .unwrap()
        );
        assert!(
            !s.test(&Cond::All {
                of: vec![even_x.clone(), tall.clone()]
            })
            .unwrap()
        );
        assert!(s.test(&Cond::NoneOf { of: vec![tall] }).unwrap());
        assert!(
            !s.test(&Cond::Otherwise).unwrap(),
            "`otherwise` is decided by rule selection"
        );
        assert!(
            s.test(&Cond::Orientation {
                x: Axis::X,
                y: Axis::Y,
                z: Axis::Z
            })
            .unwrap()
        );
    }

    /// The chapel ward's own frame guard, rebuilt here so the unit test owns
    /// its inputs: `chute_run > mainline_width`, both sides derived.
    fn derived_guard() -> Cond {
        let mainline_width =
            || Expr::dim(DimRef::X).arith(ArithOp::Sub, Expr::param("strip_depth"));
        let chute_run = || {
            Expr::dim(DimRef::Z)
                .arith(ArithOp::Sub, Expr::param("junction_run"))
                .arith(ArithOp::Sub, Expr::param("hearth_run"))
        };
        Cond::All {
            of: vec![
                Cond::cmp(
                    Expr::param("strip_depth"),
                    CmpOp::Gt,
                    Expr::param("junction_run"),
                ),
                Cond::cmp(chute_run(), CmpOp::Gt, mainline_width()),
            ],
        }
    }

    fn zone_params(hearth_run: i64) -> BTreeMap<String, i64> {
        BTreeMap::from([
            ("strip_depth".to_string(), 9),
            ("junction_run".to_string(), 8),
            ("hearth_run".to_string(), hearth_run),
        ])
    }

    /// A derived operand's *number* says how far off the scope is; it does not
    /// say which of the knobs feeding it to turn, and the guard is the only
    /// thing that knows. So a measured operand carries its inputs — and a plain
    /// name carries none, because `strip_depth = 9 from strip_depth = 9` is
    /// noise.
    #[test]
    fn a_derived_operand_names_the_quantities_that_move_it() {
        let region = Box3::at_origin([16, 9, 26]);
        let p = zone_params(14);
        let s = scope(&region, &p, Orientation::IDENTITY);

        let derived = s
            .measure(
                &Expr::dim(DimRef::Z)
                    .arith(ArithOp::Sub, Expr::param("junction_run"))
                    .arith(ArithOp::Sub, Expr::param("hearth_run")),
            )
            .unwrap();
        assert_eq!(derived.source, "Dimension.Z - junction_run - hearth_run");
        assert_eq!(derived.value, 4);
        assert_eq!(
            derived
                .inputs
                .iter()
                .map(|i| (i.name.as_str(), i.value))
                .collect::<Vec<_>>(),
            vec![("Dimension.Z", 26), ("junction_run", 8), ("hearth_run", 14)],
            "the reader must be able to see which term made it 4"
        );

        let plain = s.measure(&Expr::param("strip_depth")).unwrap();
        assert_eq!((plain.source.as_str(), plain.value), ("strip_depth", 9));
        assert!(plain.inputs.is_empty(), "{:?}", plain.inputs);

        let literal = s.measure(&Expr::int(3)).unwrap();
        assert_eq!((literal.source.as_str(), literal.value), ("3", 3));
        assert!(literal.inputs.is_empty());
    }

    /// A refusal must name the conjunct that was false — not the guard — and
    /// state the distance from it, so the next candidate is a deduction.
    #[test]
    fn a_measured_guard_names_the_false_conjunct_and_the_distance_to_it() {
        let region = Box3::at_origin([16, 9, 26]);
        let p = zone_params(14);
        let s = scope(&region, &p, Orientation::IDENTITY);
        let guard = derived_guard();
        assert!(!s.test(&guard).unwrap());

        let CondTrace::All { of } = s.explain(&guard) else {
            panic!("an `all` guard explains as an `all`");
        };
        let [first, second] = &of[..] else {
            panic!("two clauses");
        };
        assert!(first.holds(), "9 > 8 holds and must be shown holding");

        let CondTrace::Cmp(fact) = second else {
            panic!("a comparison");
        };
        assert!(!fact.holds);
        assert_eq!((fact.lhs.value, fact.rhs.value), (4, 7));
        let short = fact.shortfall.expect("a false comparison states its gap");
        // 26 - 8 - hearth_run must exceed 7, so hearth_run may be at most 10 —
        // which is exactly what "the left must reach 8" lets an author derive.
        assert_eq!(short.blocks, 4);
        assert_eq!(short.lhs_must_reach, 8);
        assert_eq!(short.rhs_must_reach, 3);

        // ...and the same guard on a scope it accepts states no gap at all.
        let ok = zone_params(8);
        let s = scope(&region, &ok, Orientation::IDENTITY);
        assert!(s.test(&guard).unwrap());
        assert!(s.explain(&guard).holds());
    }

    /// Every operator says how far off it is, from both sides — a scope's
    /// dimensions and a program's parameters are different things to move, and
    /// the guard does not know which of them the author may touch.
    #[test]
    fn the_distance_to_satisfaction_is_stated_for_every_operator() {
        let region = Box3::at_origin([4, 9, 10]);
        let p = params();
        let s = scope(&region, &p, Orientation::IDENTITY);
        let gap = |a: i64, op: CmpOp, b: i64| {
            let fact = CmpFact::new(
                s.measure(&Expr::int(a)).unwrap(),
                op,
                s.measure(&Expr::int(b)).unwrap(),
            );
            (fact.holds, fact.shortfall)
        };
        for (a, op, b, blocks, lhs, rhs) in [
            (4i64, CmpOp::Gt, 7i64, 4i64, 8i64, 3i64),
            (4, CmpOp::Ge, 7, 3, 7, 4),
            (9, CmpOp::Lt, 7, 3, 6, 10),
            (9, CmpOp::Le, 7, 2, 7, 9),
            (9, CmpOp::Eq, 7, 2, 7, 9),
            (7, CmpOp::Ne, 7, 1, 8, 8),
        ] {
            let (holds, short) = gap(a, op, b);
            assert!(!holds, "{a} {op} {b}");
            let short = short.expect("a false comparison states its gap");
            assert_eq!(
                (short.blocks, short.lhs_must_reach, short.rhs_must_reach),
                (blocks, lhs, rhs),
                "{a} {op} {b}"
            );
            // The stated targets are exactly satisfying, not merely nearer.
            assert!(
                CmpFact::new(
                    s.measure(&Expr::int(short.lhs_must_reach)).unwrap(),
                    op,
                    s.measure(&Expr::int(b)).unwrap()
                )
                .holds
            );
            assert!(
                CmpFact::new(
                    s.measure(&Expr::int(a)).unwrap(),
                    op,
                    s.measure(&Expr::int(short.rhs_must_reach)).unwrap()
                )
                .holds
            );
        }
        assert_eq!(gap(8, CmpOp::Gt, 7).1, None, "a true comparison has no gap");
    }

    /// The report and the decision are two readings of one guard, and they may
    /// never disagree — a diagnostic that describes a verdict the interpreter
    /// did not reach is worse than none. The interesting half is where `test`
    /// short-circuits: a clause it never consulted must not be able to change
    /// the verdict the report prints, nor make the report refuse to print.
    #[test]
    fn explain_agrees_with_test_including_where_test_short_circuits() {
        let region = Box3::at_origin([4, 9, 10]);
        let p = params();
        let s = scope(&region, &p, Orientation::IDENTITY);
        let yes = Cond::cmp(Expr::dim(DimRef::Y), CmpOp::Eq, Expr::int(9));
        let no = Cond::cmp(Expr::dim(DimRef::Y), CmpOp::Eq, Expr::int(1));
        let unreadable = Cond::cmp(
            Expr::dim(DimRef::Y).arith(ArithOp::Div, Expr::int(0)),
            CmpOp::Eq,
            Expr::int(0),
        );

        for cond in [
            Cond::Always,
            Cond::Otherwise,
            yes.clone(),
            no.clone(),
            Cond::All {
                of: vec![yes.clone(), no.clone()],
            },
            Cond::Any {
                of: vec![no.clone(), yes.clone()],
            },
            Cond::NoneOf {
                of: vec![no.clone()],
            },
            Cond::NoneOf {
                of: vec![yes.clone()],
            },
            Cond::All { of: vec![] },
            Cond::Any { of: vec![] },
            Cond::Orientation {
                x: Axis::X,
                y: Axis::Y,
                z: Axis::Z,
            },
            Cond::Orientation {
                x: Axis::Z,
                y: Axis::Y,
                z: Axis::X,
            },
            // The short-circuit cases: an unreadable clause sits behind a
            // verdict already decided.
            Cond::All {
                of: vec![no.clone(), unreadable.clone()],
            },
            Cond::Any {
                of: vec![yes.clone(), unreadable.clone()],
            },
        ] {
            assert_eq!(
                s.explain(&cond).holds(),
                s.test(&cond).unwrap(),
                "the report and the decision disagree on {cond:?}"
            );
        }

        // Reporting never fails, even where evaluating would: the clause says
        // so and the verdict the interpreter reached is still what is printed.
        let trace = s.explain(&Cond::All {
            of: vec![no, unreadable.clone()],
        });
        let CondTrace::All { of } = &trace else {
            panic!("an `all`")
        };
        assert!(matches!(of[1], CondTrace::Unreadable { .. }), "{:?}", of[1]);
        assert!(trace.to_string().contains("division or remainder by zero"));
        // Where `test` itself refuses, it still refuses — nothing is masked.
        assert_eq!(s.test(&unreadable), Err(EvalError::DivideByZero));
    }

    /// The whole-rule report: the box it was handed, every alternative, and the
    /// clause that decided each one.
    #[test]
    fn a_rule_refusal_states_the_scope_and_every_alternative() {
        use crate::ir::{Alternative, Node};

        let region = Box3::at_origin([16, 9, 26]);
        let p = zone_params(14);
        let s = scope(&region, &p, Orientation::IDENTITY);
        let alts = vec![
            Alternative::new(Node::fill("margin")).when(derived_guard()),
            Alternative::new(Node::Void).when(Cond::cmp(
                Expr::dim(DimRef::Y),
                CmpOp::Ge,
                Expr::int(40),
            )),
        ];
        let refusal = GuardRefusal::of("ward_plan", &s, &alts);
        assert_eq!(refusal.symbol, "ward_plan");
        assert_eq!(refusal.scope.size, [16, 9, 26]);
        assert_eq!(refusal.alternatives.len(), 2);
        assert_eq!(refusal.alternatives[0].index, 1);
        assert!(refusal.alternatives.iter().all(|a| !a.guard.holds()));

        let text = refusal.to_string();
        // The box, in the rule's own frame.
        assert!(text.contains("16x9x26 at 0,0,0"), "{text}");
        assert!(text.contains("Dimension.Z = 26 (world Z)"), "{text}");
        // Both alternatives, each with the clause that decided it.
        assert!(text.contains("alternative 1 of 2"), "{text}");
        assert!(text.contains("alternative 2 of 2"), "{text}");
        assert!(
            text.contains("ok     strip_depth > junction_run"),
            "a satisfied clause is shown too, so an author can see the headroom \
             they must not spend while fixing the other one: {text}"
        );
        assert!(
            text.contains(
                "FALSE  Dimension.Z - junction_run - hearth_run > Dimension.X - strip_depth"
            ),
            "{text}"
        );
        assert!(
            text.contains("from Dimension.Z = 26, junction_run = 8, hearth_run = 14"),
            "{text}"
        );
        assert!(
            text.contains("4 short: the left must rise to 8, or the right fall to 3"),
            "{text}"
        );
        // A one-clause guard reads with the same marker and the same layout.
        assert!(text.contains("its one clause does not hold"), "{text}");
        assert!(text.contains("FALSE  Dimension.Y >= 40"), "{text}");
    }

    /// The report is data as well as prose: a driver reads `sweep.json` and a
    /// refusal reason it cannot parse is half a diagnostic.
    #[test]
    fn a_refusal_serialises_with_every_number_a_reader_needs() {
        use crate::ir::{Alternative, Node};

        let region = Box3::at_origin([16, 9, 26]);
        let p = zone_params(14);
        let s = scope(&region, &p, Orientation::IDENTITY);
        let alts = vec![Alternative::new(Node::Void).when(derived_guard())];
        let json = serde_json::to_value(GuardRefusal::of("ward_plan", &s, &alts)).unwrap();

        assert_eq!(json["symbol"], "ward_plan");
        assert_eq!(json["scope"]["size"][2], 26);
        assert_eq!(json["scope"]["dims"][2]["world"], "z");
        let clause = &json["alternatives"][0]["guard"]["of"][1];
        assert_eq!(clause["cond"], "cmp");
        assert_eq!(clause["op"], "gt");
        assert_eq!(clause["holds"], false);
        assert_eq!(
            clause["lhs"]["source"],
            "Dimension.Z - junction_run - hearth_run"
        );
        assert_eq!(clause["lhs"]["value"], 4);
        assert_eq!(clause["lhs"]["inputs"][2]["name"], "hearth_run");
        assert_eq!(clause["lhs"]["inputs"][2]["value"], 14);
        assert_eq!(clause["shortfall"]["blocks"], 4);
        assert_eq!(clause["shortfall"]["lhs_must_reach"], 8);
        // A clause that held carries no gap and no noise.
        assert!(json["alternatives"][0]["guard"]["of"][0]["shortfall"].is_null());
        assert!(json["alternatives"][0]["guard"]["of"][0]["lhs"]["inputs"].is_null());
    }
}
