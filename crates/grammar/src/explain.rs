//! Rendering guards and expressions back into the author's terms, with the
//! values they evaluated to at a refusing scope.
//!
//! A guard exhaustion used to say only *which rule* refused. The author was
//! told a door is locked and not which key is wrong — and the scope reaching a
//! deep rule has been through reorientations and splits, so the dimensions at
//! the failure site are not the region on the command line. Reconstructing
//! five nested comparisons by hand cost a real campaign zone a brute-force
//! region sweep. This module is the forensics: every leaf comparison that
//! decided a rejection, both operands as evaluated at that scope, and the
//! `dim`/`param` bindings inside a composite operand.
//!
//! [`explain`] is **total**: a leaf whose operands cannot be evaluated (a
//! division by zero the short-circuiting [`Scope::test`] never reached) is
//! reported as [`GuardLeaf::Unevaluable`] instead of aborting the report.

use std::fmt;

use crate::eval::Scope;
use crate::geom::{Axis, Orientation};
use crate::ir::{ArithOp, CmpOp, Cond, DimRef, Expr};

/// The lowercase name of an axis, as a rule writes it.
pub fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::X => "x",
        Axis::Y => "y",
        Axis::Z => "z",
    }
}

/// A frame as an author reads it: which world axis each local axis names, and
/// which of them run backwards along it.
///
/// The sign is **not** decoration. A frame is a permutation *and* a reflection,
/// so two frames can share a mapping and still be different frames; printing
/// only the mapping makes a refusal read `required x→x, y→y, z→z; this scope
/// has x→x, y→y, z→z`, which tells the author their guard failed against
/// itself. The reflected axes are named only when there are any, so an
/// unreflected frame reads exactly as it always has.
pub fn render_orientation(orient: &Orientation) -> String {
    let mapping = format!(
        "x\u{2192}{}, y\u{2192}{}, z\u{2192}{}",
        axis_name(orient.x),
        axis_name(orient.y),
        axis_name(orient.z)
    );
    match render_reversed_axes(orient) {
        Some(reflected) => format!("{mapping}, {reflected}"),
        None => mapping,
    }
}

/// The reflected-axis clause of a frame, or `None` when every local axis runs
/// forward.
pub fn render_reversed_axes(orient: &Orientation) -> Option<String> {
    let named: Vec<&'static str> = [Axis::X, Axis::Y, Axis::Z]
        .into_iter()
        .filter(|a| orient.mirror.get(*a))
        .map(axis_name)
        .collect();
    if named.is_empty() {
        None
    } else {
        Some(format!("local {} reversed", named.join("/")))
    }
}

fn dim_name(dim: DimRef) -> &'static str {
    match dim {
        DimRef::X => "dim:x",
        DimRef::Y => "dim:y",
        DimRef::Z => "dim:z",
        DimRef::WorldX => "dim:world_x",
        DimRef::WorldY => "dim:world_y",
        DimRef::WorldZ => "dim:world_z",
        DimRef::Smallest => "dim:smallest",
        DimRef::Largest => "dim:largest",
    }
}

fn cmp_name(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Lt => "<",
        CmpOp::Le => "<=",
        CmpOp::Gt => ">",
        CmpOp::Ge => ">=",
        CmpOp::Eq => "==",
        CmpOp::Ne => "!=",
    }
}

/// Render an expression the way an author reads it: `param:shaft_run`,
/// `dim:x`, `(dim:x - param:strip_depth)`, `max(1, (dim:x / 4))`.
pub fn render_expr(expr: &Expr) -> String {
    match expr {
        Expr::Int { value } => value.to_string(),
        Expr::Param { name } => format!("param:{name}"),
        Expr::Dim { dim } => dim_name(*dim).to_string(),
        Expr::Arith { lhs, op, rhs } => match op {
            ArithOp::Max => format!("max({}, {})", render_expr(lhs), render_expr(rhs)),
            ArithOp::Min => format!("min({}, {})", render_expr(lhs), render_expr(rhs)),
            ArithOp::Add => format!("({} + {})", render_expr(lhs), render_expr(rhs)),
            ArithOp::Sub => format!("({} - {})", render_expr(lhs), render_expr(rhs)),
            ArithOp::Mul => format!("({} * {})", render_expr(lhs), render_expr(rhs)),
            ArithOp::Div => format!("({} / {})", render_expr(lhs), render_expr(rhs)),
            ArithOp::Rem => format!("({} % {})", render_expr(lhs), render_expr(rhs)),
        },
    }
}

/// Render a whole guard, structure and all — the fallback identification when a
/// single leaf cannot be named.
pub fn render_cond(cond: &Cond) -> String {
    match cond {
        Cond::Always => "always".to_string(),
        Cond::Otherwise => "otherwise".to_string(),
        Cond::Cmp { lhs, op, rhs } => {
            format!(
                "{} {} {}",
                render_expr(lhs),
                cmp_name(*op),
                render_expr(rhs)
            )
        }
        Cond::All { of } => composite("all of", of),
        Cond::Any { of } => composite("any of", of),
        Cond::NoneOf { of } => composite("none of", of),
        Cond::Orientation { x, y, z, mirror } => format!(
            "orientation {}",
            render_orientation(&Orientation {
                x: *x,
                y: *y,
                z: *z,
                mirror: *mirror,
            })
        ),
    }
}

fn composite(label: &str, of: &[Cond]) -> String {
    let parts: Vec<String> = of.iter().map(render_cond).collect();
    format!("{label} [{}]", parts.join("; "))
}

/// One leaf of a guard that decided a rejection, with the values it took at
/// the refusing scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardLeaf {
    /// A comparison, with both operands evaluated.
    Cmp {
        /// The comparison as authored, e.g.
        /// `param:shaft_run > (dim:x - param:strip_depth)`.
        rendered: String,
        /// The evaluated left operand.
        lhs: i64,
        /// The evaluated right operand.
        rhs: i64,
        /// The value of every `dim`/`param` read inside a *composite* operand,
        /// in first-use order — a bare `dim:x` operand is already shown by
        /// `lhs`/`rhs` and is not repeated here.
        bindings: Vec<(String, i64)>,
        /// `false`: the comparison had to hold and did not. `true`: it sat
        /// under a `none_of`, held, and thereby failed the guard.
        held: bool,
    },
    /// An `orientation` guard that decided the rejection.
    Orientation {
        /// The frame the guard demands — axis mapping and reflection both.
        required: Orientation,
        /// The frame the scope actually has.
        actual: Orientation,
        /// As on [`GuardLeaf::Cmp`]: `true` means it matched under a `none_of`.
        held: bool,
    },
    /// A condition with no operands to show: `always` under a `none_of`, a
    /// nested `otherwise` (false everywhere but as a whole-alternative guard),
    /// or an empty `any` (never holds).
    Trivial {
        /// The condition as authored.
        rendered: String,
        /// As on [`GuardLeaf::Cmp`].
        held: bool,
    },
    /// A leaf whose operands could not be evaluated at this scope — e.g. a
    /// division by zero in a conjunct the short-circuiting test never reached.
    Unevaluable {
        /// The comparison as authored.
        rendered: String,
        /// The evaluation failure.
        error: String,
    },
}

impl fmt::Display for GuardLeaf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bind = |bindings: &[(String, i64)]| -> String {
            if bindings.is_empty() {
                String::new()
            } else {
                let parts: Vec<String> =
                    bindings.iter().map(|(n, v)| format!("{n} = {v}")).collect();
                format!("  [{}]", parts.join(", "))
            }
        };
        match self {
            GuardLeaf::Cmp {
                rendered,
                lhs,
                rhs,
                bindings,
                held: false,
            } => write!(
                f,
                "required {rendered}; at this scope left = {lhs}, right = {rhs}{}",
                bind(bindings)
            ),
            GuardLeaf::Cmp {
                rendered,
                lhs,
                rhs,
                bindings,
                held: true,
            } => write!(
                f,
                "forbidden (under none_of) {rendered}; at this scope it held, left = {lhs}, \
                 right = {rhs}{}",
                bind(bindings)
            ),
            GuardLeaf::Orientation {
                required,
                actual,
                held,
            } => {
                let map = render_orientation;
                if *held {
                    write!(
                        f,
                        "forbidden (under none_of) orientation {}; this scope has exactly that \
                         mapping",
                        map(required)
                    )
                } else {
                    write!(
                        f,
                        "required orientation {}; this scope has {}",
                        map(required),
                        map(actual)
                    )
                }
            }
            GuardLeaf::Trivial { rendered, held } => {
                if *held {
                    write!(
                        f,
                        "forbidden (under none_of) {rendered}, which always holds"
                    )
                } else {
                    write!(f, "required {rendered}, which never holds here")
                }
            }
            GuardLeaf::Unevaluable { rendered, error } => {
                write!(f, "could not evaluate {rendered}: {error}")
            }
        }
    }
}

/// Collect the value of every `dim`/`param` leaf inside `expr`, first-use
/// order, deduplicated — but only when `expr` is composite: a bare leaf's value
/// is already the operand value itself.
fn bindings_of(scope: &Scope<'_>, expr: &Expr, out: &mut Vec<(String, i64)>) {
    fn walk(scope: &Scope<'_>, expr: &Expr, out: &mut Vec<(String, i64)>) {
        match expr {
            Expr::Int { .. } => {}
            Expr::Param { .. } | Expr::Dim { .. } => {
                let name = render_expr(expr);
                if out.iter().any(|(n, _)| *n == name) {
                    return;
                }
                if let Ok(v) = scope.eval(expr) {
                    out.push((name, v));
                }
            }
            Expr::Arith { lhs, rhs, .. } => {
                walk(scope, lhs, out);
                walk(scope, rhs, out);
            }
        }
    }
    if matches!(expr, Expr::Arith { .. }) {
        walk(scope, expr, out);
    }
}

/// Explain why `cond` evaluated the wrong way at `scope`, collecting into
/// `out` every leaf that decided it.
///
/// `want` is the truth value the guard needed: `true` at the top of an
/// exhausted alternative, flipped by each `none_of` on the way down. A leaf
/// that already agrees with `want` contributes nothing, so recursing blindly
/// into composites is safe — only the deciding leaves are reported. An `all`
/// is reported in full, every failed conjunct at once, because the author who
/// fixes only the first re-runs into the second; the whole point is to hand
/// them the complete system of constraints in one refusal.
pub fn explain(scope: &Scope<'_>, cond: &Cond, want: bool, out: &mut Vec<GuardLeaf>) {
    match cond {
        Cond::Always => {
            if !want {
                out.push(GuardLeaf::Trivial {
                    rendered: "always".to_string(),
                    held: true,
                });
            }
        }
        Cond::Otherwise => {
            // False under `test` wherever it is nested; only its position as a
            // whole-alternative guard gives it meaning.
            if want {
                out.push(GuardLeaf::Trivial {
                    rendered: "otherwise (holds only as a whole alternative's guard, never \
                               nested)"
                        .to_string(),
                    held: false,
                });
            }
        }
        Cond::Cmp { lhs, op, rhs } => {
            let (a, b) = match (scope.eval(lhs), scope.eval(rhs)) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => {
                    out.push(GuardLeaf::Unevaluable {
                        rendered: render_cond(cond),
                        error: e.to_string(),
                    });
                    return;
                }
            };
            let holds = match op {
                CmpOp::Lt => a < b,
                CmpOp::Le => a <= b,
                CmpOp::Gt => a > b,
                CmpOp::Ge => a >= b,
                CmpOp::Eq => a == b,
                CmpOp::Ne => a != b,
            };
            if holds != want {
                let mut bindings = Vec::new();
                bindings_of(scope, lhs, &mut bindings);
                bindings_of(scope, rhs, &mut bindings);
                out.push(GuardLeaf::Cmp {
                    rendered: render_cond(cond),
                    lhs: a,
                    rhs: b,
                    bindings,
                    held: !want,
                });
            }
        }
        Cond::All { of } | Cond::Any { of } => {
            // `any` needed but empty can never hold; `all` forbidden but empty
            // always holds. Both would otherwise blame nobody.
            if of.is_empty() {
                match (cond, want) {
                    (Cond::Any { .. }, true) => out.push(GuardLeaf::Trivial {
                        rendered: "any of [] (no sub-conditions, never holds)".to_string(),
                        held: false,
                    }),
                    (Cond::All { .. }, false) => out.push(GuardLeaf::Trivial {
                        rendered: "all of [] (no sub-conditions, always holds)".to_string(),
                        held: true,
                    }),
                    _ => {}
                }
                return;
            }
            for c in of {
                explain(scope, c, want, out);
            }
        }
        Cond::NoneOf { of } => {
            for c in of {
                explain(scope, c, !want, out);
            }
        }
        Cond::Orientation { x, y, z, mirror } => {
            let required = Orientation {
                x: *x,
                y: *y,
                z: *z,
                mirror: *mirror,
            };
            let matches = scope.orient == required;
            if matches != want {
                out.push(GuardLeaf::Orientation {
                    required,
                    actual: scope.orient,
                    held: !want,
                });
            }
        }
    }
}
