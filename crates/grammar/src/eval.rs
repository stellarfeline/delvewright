//! Evaluating split sizes and rule guards against a scope.
//!
//! Ported from `Comparer` / `CompositeComparer` / `Constraint` and
//! `Scope.get_value` in `SplitGrammar.py` (`yawgmoth/GDMC25`, BSD-3-Clause —
//! see `LICENSE-GDMC25`). Upstream builds the same expression tree out of
//! Python operator overloads; here it is plain data, so a guard can be
//! serialised, diffed and checked before anything is expanded.

use std::collections::BTreeMap;

use crate::geom::{Axis, Box3, Orientation};
use crate::ir::{ArithOp, CmpOp, Cond, DimRef, Expr, Paint};

/// **The names a scope resolves against** — a chain of frames, innermost first.
///
/// A scope is a box, a set of axis names and a set of value names. The box is
/// narrowed by a split, the axis names are renamed by a `reorient`, and the
/// value names are rebound by a [`Node::Bind`](crate::ir::Node::Bind). All three
/// are inherited by every child scope, including one reached through a `call`,
/// which is what lets an argument survive a recursion whose rules know nothing
/// about it.
///
/// The **root frame** is the program's own [`params`](crate::ir::Program::params)
/// and [`palette`](crate::ir::Program::palette): a declaration and a default at
/// once. A frame is a borrowed pair of maps rather than an owned one, so pushing
/// one costs nothing and the chain lives on the expansion's own stack —
/// [`crate::ir::Program::validate`] has already proved that a binding names
/// something the root declares, so a lookup that walks off the end is
/// impossible for a validated program.
///
/// Iteration order never matters here — a lookup is by name — but the maps are
/// `BTreeMap`s anyway, because everything the derivation reads is (ADR-0006).
#[derive(Debug, Clone, Copy)]
pub struct Env<'e> {
    parent: Option<&'e Env<'e>>,
    params: &'e BTreeMap<String, i64>,
    palette: &'e BTreeMap<String, Paint>,
}

impl<'e> Env<'e> {
    /// The root frame: a program's own declarations, which are also its
    /// defaults.
    pub fn root(
        params: &'e BTreeMap<String, i64>,
        palette: &'e BTreeMap<String, Paint>,
    ) -> Env<'e> {
        Env {
            parent: None,
            params,
            palette,
        }
    }

    /// A frame over `parent`. Names it does not carry fall through.
    pub fn child(
        parent: &'e Env<'e>,
        params: &'e BTreeMap<String, i64>,
        palette: &'e BTreeMap<String, Paint>,
    ) -> Env<'e> {
        Env {
            parent: Some(parent),
            params,
            palette,
        }
    }

    /// The value of a parameter in this environment.
    pub fn param(&self, name: &str) -> Option<i64> {
        let mut env = *self;
        loop {
            if let Some(value) = env.params.get(name) {
                return Some(*value);
            }
            env = *env.parent?;
        }
    }

    /// What a palette role resolves to in this environment.
    pub fn paint(&self, role: &str) -> Option<&'e Paint> {
        let mut env = *self;
        loop {
            if let Some(paint) = env.palette.get(role) {
                return Some(paint);
            }
            env = *env.parent?;
        }
    }
}

/// What a guard or size expression is measured against.
#[derive(Debug, Clone, Copy)]
pub struct Scope<'a> {
    /// The scope's world-space box.
    pub region: &'a Box3,
    /// Which world axis each local axis names.
    pub orient: Orientation,
    /// The names in force here.
    pub env: Env<'a>,
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

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
                .env
                .param(name)
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
        // The palette plays no part in an integer expression; an empty root
        // frame is the honest stand-in for "this test is about `params`".
        static NO_PALETTE: std::sync::LazyLock<BTreeMap<String, Paint>> =
            std::sync::LazyLock::new(BTreeMap::new);
        Scope {
            region,
            orient,
            env: Env::root(params, &NO_PALETTE),
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

    /// A frame shadows the frame under it, name by name, and a name it does not
    /// carry falls through — which is the whole of the scoping rule.
    #[test]
    fn a_frame_shadows_by_name_and_everything_else_falls_through() {
        let root_params = BTreeMap::from([("step".to_string(), 1), ("run".to_string(), 6)]);
        let root_palette = BTreeMap::from([
            (
                "mass".to_string(),
                Paint::Block(crate::block::BlockState::simple("stone")),
            ),
            (
                "cut".to_string(),
                Paint::Block(crate::block::BlockState::air()),
            ),
        ]);
        let root = Env::root(&root_params, &root_palette);

        let inner_params = BTreeMap::from([("step".to_string(), 3)]);
        let inner_palette = BTreeMap::from([(
            "cut".to_string(),
            Paint::Block(crate::block::BlockState::simple("glass")),
        )]);
        let inner = Env::child(&root, &inner_params, &inner_palette);

        assert_eq!(inner.param("step"), Some(3), "shadowed");
        assert_eq!(inner.param("run"), Some(6), "fell through");
        assert_eq!(inner.param("nope"), None);
        assert_eq!(
            inner.paint("cut"),
            Some(&Paint::Block(crate::block::BlockState::simple("glass")))
        );
        assert_eq!(
            inner.paint("mass"),
            Some(&Paint::Block(crate::block::BlockState::simple("stone")))
        );
        // The outer frame is untouched: a binding has the extent of its body and
        // nothing outlives it.
        assert_eq!(root.param("step"), Some(1));
        assert_eq!(
            root.paint("cut"),
            Some(&Paint::Block(crate::block::BlockState::air()))
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
}
