//! **The integer rasterisation** (spec-0072 §3.2–§3.5): which cells of its own
//! box each solid covers.
//!
//! Every rule here is stated in integers over the box's extents and a cell's
//! index in it. Nothing is halved, nothing is rounded from a float, and no rule
//! reads a seed: geometry has no seed (ADR-0030 §6), so two executions of one
//! drawing are byte-identical (ADR-0006).
//!
//! The rules are separated from the executor because they are the part a test
//! can pin **by value**: a small instance's exact cell set, written out, at an
//! even extent and an odd one (spec-0072 criterion 4). The executor's job is
//! where the box is; this is what is in it.

use crate::drawing::ir::{Face, Horizontal, Section, Sides};
use crate::grammar::geom::Axis;

/// The largest extent a round axis may have.
///
/// The inclusion test multiplies squared doubled coordinates by squared doubled
/// extents, three of them for a sphere. At this bound every term is under
/// `2^82`, so the `i128` arithmetic below cannot wrap — and the bound is
/// **refused** rather than the arithmetic allowed to wrap, because a wrapped
/// comparison is a solid that is quietly the wrong shape.
/// `the_round_rule_does_not_overflow_at_its_own_bound` is the proof at the
/// bound itself.
pub const MAX_ROUND_AXIS: i64 = 4096;

/// One solid's rule over the cells of its own box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// A box, whole or restricted to a thickness at named faces.
    Boxed(BoxRule),
    /// An ellipse or an ellipsoid inscribed in the box, possibly halved and
    /// possibly hollow.
    Round(RoundRule),
    /// Courses that step in as they rise.
    Stepped(StepRule),
}

impl Rule {
    /// The box's extents.
    pub fn extents(&self) -> [i64; 3] {
        match self {
            Rule::Boxed(r) => r.n,
            Rule::Round(r) => r.n,
            Rule::Stepped(r) => r.n,
        }
    }

    /// Whether the cell at index `i` of the box is in the solid.
    pub fn contains(&self, i: [i64; 3]) -> bool {
        let n = self.extents();
        if (0..3).any(|a| i[a] < 0 || i[a] >= n[a]) {
            return false;
        }
        match self {
            Rule::Boxed(r) => r.contains(i),
            Rule::Round(r) => r.contains(i),
            Rule::Stepped(r) => r.contains(i),
        }
    }

    /// Every cell the solid covers, in `x`→`y`→`z` order. For tests and for the
    /// small solids; the executor streams instead of collecting.
    pub fn cells(&self) -> Vec<[i64; 3]> {
        let n = self.extents();
        let mut out = Vec::new();
        for x in 0..n[0] {
            for y in 0..n[1] {
                for z in 0..n[2] {
                    if self.contains([x, y, z]) {
                        out.push([x, y, z]);
                    }
                }
            }
        }
        out
    }
}

/// `box`: every cell, or every cell within `t` of a listed face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxRule {
    /// The box's extents.
    pub n: [i64; 3],
    /// The faces the shell stands at. `None` is the solid box.
    pub faces: Option<Vec<Face>>,
    /// Thickness at each listed face.
    pub t: i64,
}

impl BoxRule {
    fn contains(&self, i: [i64; 3]) -> bool {
        let Some(faces) = &self.faces else {
            return true;
        };
        faces.iter().any(|f| {
            let a = f.axis().index();
            if f.is_high() {
                i[a] >= self.n[a] - self.t
            } else {
                i[a] < self.t
            }
        })
    }
}

/// `cylinder` and `sphere`: the ellipse or ellipsoid inscribed in the box,
/// tested at cell centres in doubled coordinates.
///
/// One rule for both, and one rule for an even extent and an odd one: writing
/// `2·i + 1 − n` rather than `i − (n − 1)/2` is what makes a 6-wide disc and a
/// 7-wide disc obey the same sentence with nothing halved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundRule {
    /// The box's extents.
    pub n: [i64; 3],
    /// Which axes are round. A cylinder's straight axis is always in.
    pub round: [bool; 3],
    /// The face the cut plane lies on, for a half solid.
    pub flat: Option<Face>,
    /// Wall thickness. `None` is solid.
    pub t: Option<i64>,
}

impl RoundRule {
    /// A cylinder about `axis`.
    pub fn cylinder(n: [i64; 3], axis: Axis, flat: Option<Face>, t: Option<i64>) -> RoundRule {
        let mut round = [true; 3];
        round[axis.index()] = false;
        RoundRule { n, round, flat, t }
    }

    /// A sphere.
    pub fn sphere(n: [i64; 3], flat: Option<Face>, t: Option<i64>) -> RoundRule {
        RoundRule {
            n,
            round: [true; 3],
            flat,
            t,
        }
    }

    fn contains(&self, i: [i64; 3]) -> bool {
        if !self.inscribed(i, self.n, [0; 3]) {
            return false;
        }
        let Some(t) = self.t else {
            return true;
        };
        // The shrunk box: `t` off every round face that is not the cut plane.
        // A shrunk box with no cells removes nothing, so a tube thicker than
        // its own radius is the solid cylinder and not an empty one.
        let mut inner = self.n;
        let mut lo = [0i64; 3];
        for a in 0..3 {
            if !self.round[a] {
                continue;
            }
            let cut_low = self
                .flat
                .is_some_and(|f| f.axis().index() == a && !f.is_high());
            let cut_high = self
                .flat
                .is_some_and(|f| f.axis().index() == a && f.is_high());
            if !cut_low {
                lo[a] = t;
                inner[a] -= t;
            }
            if !cut_high {
                inner[a] -= t;
            }
            if inner[a] <= 0 {
                return true;
            }
        }
        if (0..3).any(|a| self.round[a] && (i[a] < lo[a] || i[a] >= lo[a] + inner[a])) {
            return true;
        }
        !self.inscribed(i, inner, lo)
    }

    /// The inequality of §3.3 over a box of extents `n` whose minimum corner
    /// sits at `lo` inside this rule's own box.
    fn inscribed(&self, i: [i64; 3], n: [i64; 3], lo: [i64; 3]) -> bool {
        let mut u = [0i128; 3];
        let mut d = [0i128; 3];
        for a in 0..3 {
            if !self.round[a] {
                continue;
            }
            let c = i[a] - lo[a];
            let na = n[a];
            match self.flat {
                Some(f) if f.axis().index() == a && !f.is_high() => {
                    u[a] = (2 * c + 1) as i128;
                    d[a] = (2 * na) as i128;
                }
                Some(f) if f.axis().index() == a && f.is_high() => {
                    u[a] = (2 * (na - 1 - c) + 1) as i128;
                    d[a] = (2 * na) as i128;
                }
                _ => {
                    u[a] = (2 * c + 1 - na) as i128;
                    d[a] = na as i128;
                }
            }
        }
        // Σ_a ( u_a² · Π_{b≠a} D_b² ) ≤ Π_a D_a², over the round axes.
        let mut whole: i128 = 1;
        for (a, da) in d.iter().enumerate() {
            if self.round[a] {
                whole *= da * da;
            }
        }
        let mut sum: i128 = 0;
        for (a, ua) in u.iter().enumerate() {
            if !self.round[a] {
                continue;
            }
            let mut term = ua * ua;
            for (b, db) in d.iter().enumerate() {
                if b != a && self.round[b] {
                    term *= db * db;
                }
            }
            sum += term;
        }
        sum <= whole
    }
}

/// What a [`StepRule`]'s courses step in on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// One horizontal axis narrows; the ridge runs along the other.
    Prism {
        /// The narrowing axis.
        taper: Horizontal,
        /// Which of its faces step in.
        sides: Sides,
    },
    /// Both horizontal axes narrow, by the same inset.
    Pyramid {
        /// The course's shape.
        section: Section,
    },
}

/// `prism` and `pyramid`: courses that step in `run` cells every `rise`
/// courses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRule {
    /// The box's extents.
    pub n: [i64; 3],
    /// What steps.
    pub step: Step,
    /// Courses per step.
    pub rise: i64,
    /// Cells stepped in per `rise` courses.
    pub run: i64,
    /// Keep only the surface.
    pub skin: bool,
}

impl StepRule {
    /// `inset(k) = ⌊k · run / rise⌋` — how far course `k` has stepped in.
    pub fn inset(&self, k: i64) -> i64 {
        k * self.run / self.rise
    }

    /// The span course `k` covers on a horizontal axis, inclusive, or `None`
    /// when it has closed. **Computed whether or not the box reaches course
    /// `k`**, which is what lets the surface rule ask about the course above
    /// the last one.
    fn span(&self, k: i64, axis: usize) -> Option<(i64, i64)> {
        let inset = self.inset(k);
        let n = self.n[axis];
        let (lo, hi) = match self.step {
            Step::Prism { taper, sides } => {
                if taper.axis().index() != axis {
                    return Some((0, n - 1));
                }
                let low = matches!(sides, Sides::Both | Sides::Low);
                let high = matches!(sides, Sides::Both | Sides::High);
                (
                    if low { inset } else { 0 },
                    if high { n - 1 - inset } else { n - 1 },
                )
            }
            Step::Pyramid { .. } => (inset, n - 1 - inset),
        };
        (lo <= hi).then_some((lo, hi))
    }

    /// Whether the cell is inside course `k`'s own span — the rectangle, or the
    /// ellipse inscribed in it for a round section.
    fn in_course(&self, k: i64, i: [i64; 3]) -> bool {
        let Some((xlo, xhi)) = self.span(k, 0) else {
            return false;
        };
        let Some((zlo, zhi)) = self.span(k, 2) else {
            return false;
        };
        if i[0] < xlo || i[0] > xhi || i[2] < zlo || i[2] > zhi {
            return false;
        }
        match self.step {
            Step::Pyramid {
                section: Section::Round,
            } => {
                // The ellipse of §3.3 inscribed in this course's own rectangle,
                // which is the one rule the round solids already state.
                let course = RoundRule {
                    n: [xhi - xlo + 1, 1, zhi - zlo + 1],
                    round: [true, false, true],
                    flat: None,
                    t: None,
                };
                course.contains([i[0] - xlo, 0, i[2] - zlo])
            }
            _ => true,
        }
    }

    /// Whether the cell lies on an edge of course `k` that **steps** — the
    /// surface the skin keeps.
    ///
    /// A side that does not step is not a stepping edge, which is what makes a
    /// one-sided `prism` with `skin` the bare slope rather than the slope plus
    /// the vertical wall at the ridge. Every side of a round course steps, so
    /// there the stepping edge is the ellipse's own boundary.
    fn on_stepping_edge(&self, k: i64, i: [i64; 3]) -> bool {
        if !self.in_course(k, i) {
            return false;
        }
        match self.step {
            Step::Prism { taper, sides } => {
                let a = taper.axis().index();
                let Some((lo, hi)) = self.span(k, a) else {
                    return false;
                };
                (matches!(sides, Sides::Both | Sides::Low) && i[a] == lo)
                    || (matches!(sides, Sides::Both | Sides::High) && i[a] == hi)
            }
            Step::Pyramid {
                section: Section::Square,
            } => {
                let Some((xlo, xhi)) = self.span(k, 0) else {
                    return false;
                };
                let Some((zlo, zhi)) = self.span(k, 2) else {
                    return false;
                };
                i[0] == xlo || i[0] == xhi || i[2] == zlo || i[2] == zhi
            }
            Step::Pyramid {
                section: Section::Round,
            } => [[1i64, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                .into_iter()
                .any(|d| !self.in_course(k, [i[0] + d[0], i[1], i[2] + d[2]])),
        }
    }

    fn contains(&self, i: [i64; 3]) -> bool {
        let k = i[1];
        if !self.in_course(k, i) {
            return false;
        }
        if !self.skin {
            return true;
        }
        self.on_stepping_edge(k, i) || !self.in_course(k + 1, i)
    }
}

/// `line`: the cells of the integer line from `a` to `b`, in order.
///
/// `n = max(|dx|, |dy|, |dz|)` steps, and the point at step `i` is
/// `A_a + ⌊(2·i·d_a + n) / (2n)⌋` on each axis, the division flooring toward
/// −∞. A half is always rounded **up**, so a line from `b` to `a` may differ
/// from the line from `a` to `b` by a cell; the rule is stated rather than
/// symmetrised, because a symmetric rule would have to pick a tie-break that is
/// no more true and is harder to predict.
pub fn line_points(a: [i64; 3], b: [i64; 3]) -> Vec<[i64; 3]> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let n = d.iter().map(|v| v.abs()).max().unwrap_or(0);
    if n == 0 {
        return vec![a];
    }
    (0..=n)
        .map(|i| {
            let mut p = [0i64; 3];
            for axis in 0..3 {
                p[axis] = a[axis] + (2 * i * d[axis] + n).div_euclid(2 * n);
            }
            p
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(cells: &[[i64; 3]]) -> Vec<[i64; 3]> {
        let mut v = cells.to_vec();
        v.sort();
        v
    }

    /// The disc an odd width reduces to is the one a generator wrote for its
    /// rose window: `dx² + dz² ≤ r² + r`.
    #[test]
    fn an_odd_disc_is_the_generators_own_disc() {
        for r in 1..=12i64 {
            let n = 2 * r + 1;
            let rule = RoundRule::cylinder([n, 1, n], Axis::Y, None, None);
            for x in 0..n {
                for z in 0..n {
                    let dx = x - r;
                    let dz = z - r;
                    assert_eq!(
                        rule.contains([x, 0, z]),
                        dx * dx + dz * dz <= r * r + r,
                        "r={r} at {x},{z}"
                    );
                }
            }
        }
    }

    /// An even extent and an odd one obey one rule, and the cell sets are
    /// written out rather than described.
    #[test]
    fn a_small_disc_is_pinned_by_value_at_both_parities() {
        let odd = Rule::Round(RoundRule::cylinder([5, 1, 5], Axis::Y, None, None));
        assert_eq!(
            set(&odd.cells()),
            set(&[
                [0, 0, 1],
                [0, 0, 2],
                [0, 0, 3],
                [1, 0, 0],
                [1, 0, 1],
                [1, 0, 2],
                [1, 0, 3],
                [1, 0, 4],
                [2, 0, 0],
                [2, 0, 1],
                [2, 0, 2],
                [2, 0, 3],
                [2, 0, 4],
                [3, 0, 0],
                [3, 0, 1],
                [3, 0, 2],
                [3, 0, 3],
                [3, 0, 4],
                [4, 0, 1],
                [4, 0, 2],
                [4, 0, 3],
            ]),
            "a 5-wide disc is the 5x5 square less its four corners"
        );
        let even = Rule::Round(RoundRule::cylinder([4, 1, 4], Axis::Y, None, None));
        assert_eq!(
            set(&even.cells()),
            set(&[
                [0, 0, 1],
                [0, 0, 2],
                [1, 0, 0],
                [1, 0, 1],
                [1, 0, 2],
                [1, 0, 3],
                [2, 0, 0],
                [2, 0, 1],
                [2, 0, 2],
                [2, 0, 3],
                [3, 0, 1],
                [3, 0, 2],
            ])
        );
    }

    /// The bound exists so the `i128` arithmetic cannot wrap, and the proof is
    /// taken **at** the bound rather than argued from it.
    #[test]
    fn the_round_rule_does_not_overflow_at_its_own_bound() {
        let n = MAX_ROUND_AXIS;
        let rule = RoundRule::sphere([n, n, n], Some(Face::Down), None);
        // The extreme cell of the extreme box: every term at its largest.
        assert!(rule.contains([n / 2, 0, n / 2]));
        assert!(!rule.contains([0, n - 1, 0]));
    }

    /// A line is the rule, written out. The diagonal is exact; the shallow line
    /// rounds a half up, which is why `b`→`a` may differ from `a`→`b`.
    #[test]
    fn a_line_is_pinned_by_value() {
        assert_eq!(line_points([0, 0, 0], [0, 0, 0]), vec![[0, 0, 0]]);
        assert_eq!(
            line_points([0, 0, 0], [3, 0, 3]),
            vec![[0, 0, 0], [1, 0, 1], [2, 0, 2], [3, 0, 3]]
        );
        assert_eq!(
            line_points([0, 0, 0], [4, 0, 2]),
            vec![[0, 0, 0], [1, 0, 1], [2, 0, 1], [3, 0, 2], [4, 0, 2]]
        );
        assert_eq!(
            line_points([4, 0, 2], [0, 0, 0]),
            vec![[4, 0, 2], [3, 0, 2], [2, 0, 1], [1, 0, 1], [0, 0, 0]],
            "the same cells here, and the rule is asymmetric in general"
        );
        // A line that descends: the negative numerator floors toward −∞.
        assert_eq!(
            line_points([0, 4, 0], [2, 0, 0]),
            vec![[0, 4, 0], [1, 3, 0], [1, 2, 0], [2, 1, 0], [2, 0, 0]]
        );
    }
}
