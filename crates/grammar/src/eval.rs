//! Evaluating split sizes and rule guards against a scope.
//!
//! Ported from `Comparer` / `CompositeComparer` / `Constraint` and
//! `Scope.get_value` in `SplitGrammar.py` (`yawgmoth/GDMC25`, BSD-3-Clause —
//! see `LICENSE-GDMC25`). Upstream builds the same expression tree out of
//! Python operator overloads; here it is plain data, so a guard can be
//! serialised, diffed and checked before anything is expanded.

use std::collections::BTreeMap;

use crate::geom::{Axis, Box3, Orientation};
use crate::ir::{ArithOp, CmpOp, Cond, DimRef, Expr};

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
}
