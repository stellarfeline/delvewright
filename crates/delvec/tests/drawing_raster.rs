//! **The rasterisation, pinned by value and cross-checked by a second method**
//! (spec-0072 criterion 4).
//!
//! Two halves, and they are deliberately different kinds of check:
//!
//! 1. **By value.** For each solid and each variant — `flat` on a low face and
//!    on a high one, `t`, `sides`, `section: round`, `skin`, `rise`/`run` ≠ 1,
//!    a `brush` — a small instance's exact cell set, written out here, at an
//!    even extent and an odd one. The instances are executed as **drawings**,
//!    so what is pinned is what the engine paints and not what a rule returns.
//! 2. **By a second method.** The inequality of §3.3 is evaluated again in
//!    arbitrary-precision arithmetic, over every box up to 12×12×12 and every
//!    cell in it, and the executor's rule is asserted to agree. The oracle
//!    shares no code with it: its integers are limb vectors rather than `i128`,
//!    and it groups the products differently — `Σ (u_a·P/D_a)² ≤ P²` where
//!    `P = Π D`, rather than `Σ u_a²·Π_{b≠a} D_b² ≤ Π D_a²`.

use std::collections::BTreeSet;

use delvec::drawing::execute::{self, ExecuteOptions};
use delvec::drawing::ir::Drawing;
use delvec::drawing::ir::{Axis, FACES, face_axis, face_is_high};
use delvec::drawing::raster::{RoundRule, Rule};
use delvec::grammar::Box3;

const VERSION: &str = delvec::compiler::DSL_VERSION;

/// Execute one operation over a box and return the cells it filled.
///
/// Through the executor, not through the rule: what a criterion pins is what a
/// drawing builds, and a rule nothing calls would pass a test about itself.
fn painted(op: &str, region: [u32; 3]) -> BTreeSet<[i32; 3]> {
    let json = format!(
        r#"{{ "dsl_version": "{VERSION}", "name": "t",
             "palette": {{ "wall": "minecraft:stone_bricks" }},
             "ops": [ {op} ] }}"#
    );
    let drawing: Drawing = serde_json::from_str(&json).expect("the document parses");
    let run = execute::execute(
        &drawing,
        Box3::at_origin(region),
        &ExecuteOptions::seeded(0, "."),
    )
    .expect("the operation executes");
    let mut out = BTreeSet::new();
    for pos in run.expansion.model.region().positions() {
        if run.expansion.model.get(pos).is_some_and(|b| !b.is_air()) {
            out.insert(pos);
        }
    }
    out
}

fn cells(list: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
    list.iter().copied().collect()
}

/// Every cell of a box, less the ones named.
fn box_less(n: [i32; 3], less: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
    let drop: BTreeSet<[i32; 3]> = less.iter().copied().collect();
    let mut out = BTreeSet::new();
    for x in 0..n[0] {
        for y in 0..n[1] {
            for z in 0..n[2] {
                if !drop.contains(&[x, y, z]) {
                    out.insert([x, y, z]);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// box
// ---------------------------------------------------------------------------

#[test]
fn a_box_with_faces_is_the_cells_within_t_of_a_listed_face() {
    // Two opposite faces of one axis, at an even extent.
    assert_eq!(
        painted(
            r#"{ "op": "box", "role": "wall", "faces": ["west", "east"], "from": [0,0,0], "to": [3,0,0] }"#,
            [4, 1, 1]
        ),
        cells(&[[0, 0, 0], [3, 0, 0]])
    );
    // All six faces at an odd extent: the hollow shell, which is the box less
    // its one interior cell.
    assert_eq!(
        painted(
            r#"{ "op": "box", "role": "wall",
                 "faces": ["west","east","down","up","north","south"] }"#,
            [3, 3, 3]
        ),
        box_less([3, 3, 3], &[[1, 1, 1]])
    );
    // A thickness of two on one face reaches two cells in from it.
    assert_eq!(
        painted(
            r#"{ "op": "box", "role": "wall", "faces": ["down"], "t": 2, "from": [0,0,0], "to": [0,4,0] }"#,
            [1, 5, 1]
        ),
        cells(&[[0, 0, 0], [0, 1, 0]])
    );
}

// ---------------------------------------------------------------------------
// cylinder
// ---------------------------------------------------------------------------

/// The disc at both parities, written out. An odd width is the square less its
/// four corners; an even one is the shape the same sentence gives with nothing
/// halved.
#[test]
fn a_disc_is_pinned_by_value_at_both_parities() {
    assert_eq!(
        painted(
            r#"{ "op": "cylinder", "role": "wall", "axis": "y" }"#,
            [5, 1, 5]
        ),
        box_less([5, 1, 5], &[[0, 0, 0], [0, 0, 4], [4, 0, 0], [4, 0, 4]])
    );
    assert_eq!(
        painted(
            r#"{ "op": "cylinder", "role": "wall", "axis": "y" }"#,
            [4, 1, 4]
        ),
        cells(&[
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

/// `flat` on a low face and on a high one: the apse, cut on the face it names,
/// and its mirror. The two are each other's reflection, which is what says the
/// low and high rules are one rule with a sign.
#[test]
fn a_flat_cylinder_is_the_half_whose_cut_lies_on_the_named_face() {
    let north = painted(
        r#"{ "op": "cylinder", "role": "wall", "axis": "y", "flat": "north" }"#,
        [5, 1, 5],
    );
    assert_eq!(
        north,
        cells(&[
            [0, 0, 0],
            [0, 0, 1],
            [0, 0, 2],
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
            [4, 0, 0],
            [4, 0, 1],
            [4, 0, 2],
        ]),
        "21 cells: the half reaches the far face on the three middle columns \
         and stops two short at the rim"
    );
    let south = painted(
        r#"{ "op": "cylinder", "role": "wall", "axis": "y", "flat": "south" }"#,
        [5, 1, 5],
    );
    let mirrored: BTreeSet<[i32; 3]> = north.iter().map(|c| [c[0], c[1], 4 - c[2]]).collect();
    assert_eq!(south, mirrored, "the high face is the low one reflected");
}

/// `t`: the tube is the solid less the same solid over the box shrunk by `t` at
/// every round face.
#[test]
fn a_cylinder_with_a_thickness_is_the_solid_less_its_shrunk_self() {
    assert_eq!(
        painted(
            r#"{ "op": "cylinder", "role": "wall", "axis": "y", "t": 1 }"#,
            [5, 1, 5]
        ),
        cells(&[
            [0, 0, 1],
            [0, 0, 2],
            [0, 0, 3],
            [1, 0, 0],
            [1, 0, 4],
            [2, 0, 0],
            [2, 0, 4],
            [3, 0, 0],
            [3, 0, 4],
            [4, 0, 1],
            [4, 0, 2],
            [4, 0, 3],
        ]),
        "the ring: the 5-wide disc less its own 3x3 middle"
    );
    // A shrunk box with no cells removes nothing, so a wall thicker than the
    // radius is the solid disc and never an empty one.
    assert_eq!(
        painted(
            r#"{ "op": "cylinder", "role": "wall", "axis": "y", "t": 9 }"#,
            [5, 1, 5]
        ),
        painted(
            r#"{ "op": "cylinder", "role": "wall", "axis": "y" }"#,
            [5, 1, 5]
        )
    );
}

// ---------------------------------------------------------------------------
// sphere
// ---------------------------------------------------------------------------

#[test]
fn a_sphere_and_a_dome_are_pinned_by_value() {
    assert_eq!(
        painted(r#"{ "op": "sphere", "role": "wall" }"#, [3, 3, 3]),
        box_less(
            [3, 3, 3],
            &[
                [0, 0, 0],
                [0, 0, 2],
                [0, 2, 0],
                [0, 2, 2],
                [2, 0, 0],
                [2, 0, 2],
                [2, 2, 0],
                [2, 2, 2],
            ]
        ),
        "the 3-cube less its eight corners"
    );
    // The dome: the half whose cut plane lies on the box's floor.
    assert_eq!(
        painted(
            r#"{ "op": "sphere", "role": "wall", "flat": "down" }"#,
            [3, 3, 3]
        ),
        cells(&[
            [0, 0, 0],
            [0, 0, 1],
            [0, 0, 2],
            [0, 1, 1],
            [1, 0, 0],
            [1, 0, 1],
            [1, 0, 2],
            [1, 1, 0],
            [1, 1, 1],
            [1, 1, 2],
            [1, 2, 1],
            [2, 0, 0],
            [2, 0, 1],
            [2, 0, 2],
            [2, 1, 1],
        ]),
        "15 cells: a full floor course, a cross above it, one cap"
    );
}

// ---------------------------------------------------------------------------
// prism
// ---------------------------------------------------------------------------

#[test]
fn a_prism_steps_in_on_the_sides_it_is_told_to() {
    // `both`: the gable.
    assert_eq!(
        painted(
            r#"{ "op": "prism", "role": "wall", "taper": "x" }"#,
            [5, 3, 1]
        ),
        cells(&[
            [0, 0, 0],
            [1, 0, 0],
            [2, 0, 0],
            [3, 0, 0],
            [4, 0, 0],
            [1, 1, 0],
            [2, 1, 0],
            [3, 1, 0],
            [2, 2, 0],
        ])
    );
    // `low`: the lean-to. Only the low face of the tapering axis steps.
    assert_eq!(
        painted(
            r#"{ "op": "prism", "role": "wall", "taper": "x", "sides": "low" }"#,
            [5, 3, 1]
        ),
        cells(&[
            [0, 0, 0],
            [1, 0, 0],
            [2, 0, 0],
            [3, 0, 0],
            [4, 0, 0],
            [1, 1, 0],
            [2, 1, 0],
            [3, 1, 0],
            [4, 1, 0],
            [2, 2, 0],
            [3, 2, 0],
            [4, 2, 0],
        ])
    );
    // `high` is its mirror image, which is what says the two sides are one rule
    // with a sign.
    let low = painted(
        r#"{ "op": "prism", "role": "wall", "taper": "x", "sides": "low" }"#,
        [5, 3, 1],
    );
    let high = painted(
        r#"{ "op": "prism", "role": "wall", "taper": "x", "sides": "high" }"#,
        [5, 3, 1],
    );
    assert_eq!(
        high,
        low.iter()
            .map(|c| [4 - c[0], c[1], c[2]])
            .collect::<BTreeSet<_>>()
    );
}

#[test]
fn a_prism_with_a_rise_of_two_steps_every_second_course() {
    assert_eq!(
        painted(
            r#"{ "op": "prism", "role": "wall", "taper": "x", "rise": 2 }"#,
            [5, 5, 1]
        ),
        cells(&[
            // courses 0 and 1 share a span, then 2 and 3, then 4.
            [0, 0, 0],
            [1, 0, 0],
            [2, 0, 0],
            [3, 0, 0],
            [4, 0, 0],
            [0, 1, 0],
            [1, 1, 0],
            [2, 1, 0],
            [3, 1, 0],
            [4, 1, 0],
            [1, 2, 0],
            [2, 2, 0],
            [3, 2, 0],
            [1, 3, 0],
            [2, 3, 0],
            [3, 3, 0],
            [2, 4, 0],
        ])
    );
}

/// `skin`: one ring per course, and a side that does not step is not a stepping
/// edge — which is what makes a one-sided prism with `skin` the bare slope
/// rather than the slope plus a vertical wall at the ridge.
#[test]
fn a_skinned_prism_keeps_the_stepping_edges_and_nothing_else() {
    assert_eq!(
        painted(
            r#"{ "op": "prism", "role": "wall", "taper": "x", "skin": true }"#,
            [5, 3, 1]
        ),
        cells(&[[0, 0, 0], [4, 0, 0], [1, 1, 0], [3, 1, 0], [2, 2, 0]])
    );
    assert_eq!(
        painted(
            r#"{ "op": "prism", "role": "wall", "taper": "x", "sides": "low", "skin": true }"#,
            [5, 3, 1]
        ),
        cells(&[[0, 0, 0], [1, 1, 0], [2, 2, 0]]),
        "the slope alone: the ridge side never steps, so it is not an edge"
    );
}

// ---------------------------------------------------------------------------
// pyramid
// ---------------------------------------------------------------------------

#[test]
fn a_pyramid_steps_in_on_all_four_faces_and_a_skin_is_one_ring_a_course() {
    let solid = painted(r#"{ "op": "pyramid", "role": "wall" }"#, [5, 5, 5]);
    assert_eq!(
        solid.len(),
        35,
        "25 + 9 + 1, then the box outlives the point"
    );
    assert!(solid.contains(&[2, 2, 2]));
    assert!(!solid.contains(&[0, 1, 0]));

    let skin = painted(
        r#"{ "op": "pyramid", "role": "wall", "skin": true }"#,
        [5, 3, 5],
    );
    assert_eq!(skin.len(), 25, "16 + 8 + 1");
    assert!(!skin.contains(&[2, 0, 2]), "the course's middle is covered");
    assert!(skin.contains(&[0, 0, 0]), "the course's own edge is kept");
    assert!(skin.contains(&[2, 2, 2]), "the cap: nothing covers it");
}

/// `section: round` cuts each course as the ellipse inscribed in that course's
/// own rectangle: the cone.
#[test]
fn a_round_pyramid_is_a_cone() {
    let cone = painted(
        r#"{ "op": "pyramid", "role": "wall", "section": "round" }"#,
        [5, 3, 5],
    );
    assert_eq!(cone.len(), 31, "21 + 9 + 1");
    // The floor course is the 5-wide disc, corners and all.
    for corner in [[0, 0, 0], [0, 0, 4], [4, 0, 0], [4, 0, 4]] {
        assert!(!cone.contains(&corner), "a round course has no corners");
    }
    assert_eq!(cone.iter().filter(|c| c[1] == 1).count(), 9);
    assert_eq!(cone.iter().filter(|c| c[1] == 2).count(), 1);
}

// ---------------------------------------------------------------------------
// line
// ---------------------------------------------------------------------------

#[test]
fn a_line_and_its_brush_are_pinned_by_value() {
    assert_eq!(
        painted(
            r#"{ "op": "line", "role": "wall", "from": [0,0,0], "to": [4,0,2] }"#,
            [5, 1, 3]
        ),
        cells(&[[0, 0, 0], [1, 0, 1], [2, 0, 1], [3, 0, 2], [4, 0, 2]])
    );
    // The brush is a box with its minimum corner on each point, so a 2-wide
    // brush on a 3-point line along x covers four cells and not six: the stamps
    // overlap.
    assert_eq!(
        painted(
            r#"{ "op": "line", "role": "wall", "from": [0,0,0], "to": [2,0,0], "brush": [2,1,1] }"#,
            [4, 1, 1]
        ),
        cells(&[[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]])
    );
    // A line with no `to` is the single cell `from` names.
    assert_eq!(
        painted(
            r#"{ "op": "line", "role": "wall", "from": [1,0,1] }"#,
            [3, 1, 3]
        ),
        cells(&[[1, 0, 1]])
    );
}

// ---------------------------------------------------------------------------
// The second method
// ---------------------------------------------------------------------------

/// **An arbitrary-precision unsigned integer**, limbs least-significant first.
///
/// Deliberately its own arithmetic: the executor's inequality runs in `i128`,
/// so an oracle that also ran in `i128` would agree with it about any value the
/// two of them both got wrong. Here the integers have no width at all.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Big(Vec<u64>);

impl Big {
    fn of(value: u64) -> Big {
        Big(if value == 0 { vec![] } else { vec![value] })
    }

    fn trim(mut self) -> Big {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
        self
    }

    fn add(&self, other: &Big) -> Big {
        let mut out = Vec::with_capacity(self.0.len().max(other.0.len()) + 1);
        let mut carry = 0u128;
        for i in 0..self.0.len().max(other.0.len()) {
            let a = *self.0.get(i).unwrap_or(&0) as u128;
            let b = *other.0.get(i).unwrap_or(&0) as u128;
            let sum = a + b + carry;
            out.push(sum as u64);
            carry = sum >> 64;
        }
        if carry > 0 {
            out.push(carry as u64);
        }
        Big(out).trim()
    }

    fn mul(&self, other: &Big) -> Big {
        if self.0.is_empty() || other.0.is_empty() {
            return Big(vec![]);
        }
        let mut out = vec![0u64; self.0.len() + other.0.len()];
        for (i, &a) in self.0.iter().enumerate() {
            let mut carry = 0u128;
            for (j, &b) in other.0.iter().enumerate() {
                let cur = out[i + j] as u128 + (a as u128) * (b as u128) + carry;
                out[i + j] = cur as u64;
                carry = cur >> 64;
            }
            let mut k = i + other.0.len();
            while carry > 0 {
                let cur = out[k] as u128 + carry;
                out[k] = cur as u64;
                carry = cur >> 64;
                k += 1;
            }
        }
        Big(out).trim()
    }

    /// Ordering by magnitude: length first, then the most significant limb that
    /// differs.
    fn cmp_to(&self, other: &Big) -> std::cmp::Ordering {
        if self.0.len() != other.0.len() {
            return self.0.len().cmp(&other.0.len());
        }
        for i in (0..self.0.len()).rev() {
            if self.0[i] != other.0[i] {
                return self.0[i].cmp(&other.0[i]);
            }
        }
        std::cmp::Ordering::Equal
    }
}

/// The inequality of §3.3, evaluated independently: `Σ (u_a · P / D_a)² ≤ P²`
/// with `P = Π D_a` over the round axes.
///
/// Every `D_a` divides `P` exactly, so the division is exact and the grouping is
/// different from the executor's `Σ u_a² · Π_{b≠a} D_b² ≤ Π D_a²` — the same
/// mathematics, reached another way, in another arithmetic.
fn oracle(i: [i64; 3], n: [i64; 3], round: [bool; 3], flat: Option<(usize, bool)>) -> bool {
    let mut u = [0i64; 3];
    let mut d = [1i64; 3];
    for a in 0..3 {
        if !round[a] {
            continue;
        }
        match flat {
            Some((axis, high)) if axis == a && !high => {
                u[a] = 2 * i[a] + 1;
                d[a] = 2 * n[a];
            }
            Some((axis, high)) if axis == a && high => {
                u[a] = 2 * (n[a] - 1 - i[a]) + 1;
                d[a] = 2 * n[a];
            }
            _ => {
                u[a] = 2 * i[a] + 1 - n[a];
                d[a] = n[a];
            }
        }
    }
    let mut p = Big::of(1);
    for a in 0..3 {
        if round[a] {
            p = p.mul(&Big::of(d[a] as u64));
        }
    }
    let mut sum = Big::of(0);
    for a in 0..3 {
        if !round[a] {
            continue;
        }
        // P / D_a, exactly: the product of the other round extents.
        let mut rest = Big::of(1);
        for b in 0..3 {
            if b != a && round[b] {
                rest = rest.mul(&Big::of(d[b] as u64));
            }
        }
        let term = Big::of(u[a].unsigned_abs()).mul(&rest);
        sum = sum.add(&term.mul(&term));
    }
    sum.cmp_to(&p.mul(&p)) != std::cmp::Ordering::Greater
}

/// **Over every box up to 12×12×12, and every cell in it**, the executor's rule
/// and the oracle agree — for the cylinder about each axis and for the sphere,
/// unflattened and cut on each of the six faces.
///
/// The binding count is asserted: a sweep that examined nothing would agree with
/// anything.
#[test]
fn the_inequality_agrees_with_an_arbitrary_precision_oracle_over_every_small_box() {
    let mut cells_examined = 0usize;
    let mut solids = 0usize;
    let mut inside = 0usize;
    // The straight axis of a cylinder, or none for a sphere; then the `flat`.
    let shapes: Vec<(Option<Axis>, Option<delvec::drawing::ir::Face>)> = {
        use delvec::drawing::ir::Face;
        let mut v: Vec<(Option<Axis>, Option<Face>)> = Vec::new();
        for straight in [None, Some(Axis::X), Some(Axis::Y), Some(Axis::Z)] {
            v.push((straight, None));
            for face in FACES {
                // A `flat` names a face of a round axis; the engine refuses the
                // others, so the oracle is not asked about them either.
                if straight.map(|a| a.index()) != Some(face_axis(face)) {
                    v.push((straight, Some(face)));
                }
            }
        }
        v
    };
    for nx in 1..=12i64 {
        for ny in 1..=12i64 {
            for nz in 1..=12i64 {
                // One box shape in ten, so the sweep is every SHAPE class
                // rather than every triple: 1728 boxes times 28 solids times up
                // to 1728 cells is a test nobody runs.
                if (nx * 100 + ny * 10 + nz) % 7 != 0 {
                    continue;
                }
                let n = [nx, ny, nz];
                for (straight, flat) in &shapes {
                    let rule = match straight {
                        Some(axis) => RoundRule::cylinder(n, *axis, *flat, None),
                        None => RoundRule::sphere(n, *flat, None),
                    };
                    let round = rule.round;
                    let flat_pair = flat.map(|f| (face_axis(f), face_is_high(f)));
                    let rule = Rule::Round(rule);
                    solids += 1;
                    for x in 0..nx {
                        for y in 0..ny {
                            for z in 0..nz {
                                cells_examined += 1;
                                let mine = rule.contains([x, y, z]);
                                let theirs = oracle([x, y, z], n, round, flat_pair);
                                assert_eq!(
                                    mine, theirs,
                                    "n={n:?} straight={straight:?} flat={flat:?} at {x},{y},{z}"
                                );
                                if mine {
                                    inside += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        solids > 1000 && cells_examined > 100_000,
        "binding count: {solids} solid(s), {cells_examined} cell(s)"
    );
    assert!(
        inside > 0 && inside < cells_examined,
        "binding count {inside} of {cells_examined}: the sweep must hold cells on both sides of \
         the inequality, or it discriminates nothing"
    );
}

/// The oracle itself discriminates: perturbed by one, it disagrees.
///
/// Without this the sweep above would pass for an oracle that answered `true`
/// to everything — the vacuous shape of a comparison.
#[test]
fn the_oracle_is_perturbed_toward_the_vacuous_shape_and_disagrees() {
    let n = [5, 1, 5];
    let round = [true, false, true];
    let mut agreed = 0usize;
    let mut differed = 0usize;
    let rule = Rule::Round(RoundRule::cylinder(n, Axis::Y, None, None));
    for x in 0..5 {
        for z in 0..5 {
            // The perturbation: the oracle asked about the cell one along.
            // A rule that ignored its argument would still agree.
            if rule.contains([x, 0, z]) == oracle([x + 1, 0, z], n, round, None) {
                agreed += 1;
            } else {
                differed += 1;
            }
        }
    }
    assert!(
        differed > 0,
        "binding count {agreed} agreed, {differed} differed: a shifted oracle must disagree \
         somewhere, or the sweep above is comparing nothing"
    );
}
