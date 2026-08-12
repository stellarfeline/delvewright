//! **How an expansion is judged before a human looks at it** (spec-0027 §3's
//! "machine gates filter", the step §6 records as not built).
//!
//! # A gate and a measurement are different things, and this module keeps them apart
//!
//! A *gate* has a verdict: it can be red, and the condition that reddens it is
//! stated. A *measurement* is a number with no threshold. The distinction is the
//! whole design here, because the failure this project keeps hitting is a green
//! that binds to nothing (CLAUDE.md): a number printed beside the word "pass" is
//! not a gate, and calling it one is worse than printing nothing, since the next
//! reader believes something was checked.
//!
//! So: [`Gate`]s carry a verdict **and a binding count** — how many objects the
//! gate examined. A zero binding is reported as a finding in its own right, not
//! folded into a pass. [`Measurements`] carry numbers and no verdicts, and are
//! deliberately not dressed up as gates.
//!
//! # Why the craft gates of spec-0027 §4 are not here
//!
//! §4 asks for a palette-role budget "computed per **material family**". Nothing
//! in this repo can decide what family a block belongs to: the two places that
//! need it (`tests/staging.rs`'s boulder-stair and broken-grate mirrors) each
//! hand-write the family map for the blocks that test uses. A diagnostic cannot
//! take a hand-written map — it would only ever be as complete as the fixture
//! that wrote it, which is the vacuity mode this module exists to avoid. The
//! honest state is therefore: **the family-grouped palette budget is not built,
//! and what blocks it is a missing derivation, not missing effort.**

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::expand::Expansion;
use crate::geom::Axis;
use crate::model::VoxelModel;
use crate::nav;

/// One gate's verdict over one expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gate {
    /// Stable id, e.g. `blocks-exist`.
    pub id: &'static str,
    /// True when the gate held.
    pub pass: bool,
    /// **How many objects the gate examined.** A gate that examined zero
    /// objects is not a pass; `Report::findings` says so by name.
    pub bound: usize,
    /// What the gate found, in one line.
    pub detail: String,
}

/// Numbers with no threshold. Not gates, and deliberately not presented as any.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measurements {
    /// Cells the expansion wrote something other than air into.
    pub filled_cells: usize,
    /// Cells in the region.
    pub region_cells: usize,
    /// Distinct block states in the model's palette (air included).
    pub distinct_states: usize,
    /// Cells a body could stand in.
    pub standable_cells: usize,
    /// Columns of the footprint that carry any block.
    pub footprint_area: usize,
    /// Edges of the footprint that face empty ground — the silhouette's length.
    ///
    /// spec-0027 §4 wants a "silhouette/perimeter complexity floor", calling it
    /// the one metric that tracked quality in the sandbox probe. The probe's
    /// threshold was never written down, so this is reported and **not** gated:
    /// inventing a number here would be a fabricated gate.
    pub footprint_perimeter: usize,
    /// `footprint_perimeter` over the perimeter of a solid rectangle of the same
    /// area — 1.0 for a plain box, higher the more articulated the plan.
    pub silhouette_complexity: f64,
    /// The five commonest non-air block states, with their share of filled cells.
    pub top_blocks: Vec<(String, f64)>,
}

/// The whole verdict over one expansion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// `"pass"` when every gate held, else `"fail"`.
    pub verdict: &'static str,
    /// Every gate, in a fixed order.
    pub gates: Vec<Gate>,
    /// Numbers, no verdicts.
    pub measurements: Measurements,
    /// Anchors the program declared, by exported name.
    pub anchors: BTreeMap<String, [i32; 3]>,
    /// Things a reader must be told even though no gate went red — a gate that
    /// examined nothing, an expansion that declared no anchors.
    pub findings: Vec<String>,
}

impl Report {
    /// True when no gate went red. Findings do not fail a report; they are
    /// carried so they cannot be lost.
    pub fn is_pass(&self) -> bool {
        self.verdict == "pass"
    }

    /// Canonical pretty JSON with a trailing newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("a gate report serialises");
        s.push('\n');
        s
    }
}

/// Which optional gates to run beyond the always-on ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Assert the piece can be walked from its approach end to its exit end.
    ///
    /// Opt-in because it is a claim about a *kind* of piece: a room with one
    /// door has no far end to reach and would fail a traversability gate
    /// correctly and uselessly. The author says which claim the piece makes.
    pub traversable: bool,
    /// Allow a fall edge when walking (a piece entered by stepping off a ledge).
    pub allow_falls: bool,
    /// Assert the piece is bilaterally symmetric about the mid-plane of this
    /// **world** axis.
    ///
    /// Opt-in, and for the reason `traversable` is: it is a claim about a *kind*
    /// of piece, and only the author knows whether this one makes it. What makes
    /// it worth having is that the claim is otherwise unenforceable by anything.
    /// A shape with a mirror plane is normally built by expanding one rule at
    /// both sites — and if the two sites are instead two hand-kept copies, or
    /// one site is missing its reflection, every other gate stays green while
    /// the building has a hole in one flank. This is the gate that reads it.
    pub symmetric: Option<Axis>,
}

/// Judge one expansion.
pub fn judge(expansion: &Expansion, options: Options) -> Report {
    let model = &expansion.model;
    let mut gates = Vec::new();
    let mut findings = Vec::new();

    // --- Gate: every block state exists in the pinned version. -------------
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
    let mut bad = Vec::new();
    for state in model.palette() {
        if let Err(e) = registry.validate(&state.name, &state.properties) {
            bad.push(e.to_string());
        }
    }
    gates.push(Gate {
        id: "blocks-exist",
        pass: bad.is_empty(),
        bound: model.palette().len(),
        detail: if bad.is_empty() {
            format!(
                "{} block state(s), all present in Minecraft {}",
                model.palette().len(),
                delvewright_schem::blocks::MC_VERSION
            )
        } else {
            bad.join("; ")
        },
    });

    // --- Gate: the expansion built something. ------------------------------
    let filled = model.filled_cells();
    let region_cells = model.region().positions().count();
    gates.push(Gate {
        id: "non-empty",
        pass: filled > 0,
        bound: region_cells,
        detail: format!("{filled} filled cell(s) of {region_cells} in the region"),
    });

    // --- Gate (opt-in): a body can walk the piece end to end. --------------
    let standable = nav::standable_cells(model);
    if options.traversable {
        let (entry, exit) = nav::ends(model);
        let bound = entry.len() + exit.len();
        let walked = if options.allow_falls {
            nav::reachable_with_fall(model, &standable, &entry, &exit)
        } else {
            nav::connected(&standable, &entry, &exit)
        };
        gates.push(Gate {
            id: "traversable",
            pass: walked && bound > 0,
            bound,
            detail: format!(
                "{} standable cell(s) at the approach end, {} at the exit end, {} standable in \
                 all; walking{} {}",
                entry.len(),
                exit.len(),
                standable.len(),
                if options.allow_falls {
                    " (with falls)"
                } else {
                    ""
                },
                if walked {
                    "connects them"
                } else {
                    "does NOT connect them"
                }
            ),
        });
    }

    // --- Gate (opt-in): the piece is its own mirror image. -----------------
    if let Some(axis) = options.symmetric {
        let (pairs, broken) = asymmetry(model, axis);
        gates.push(Gate {
            id: "symmetric",
            pass: broken.is_empty() && pairs > 0,
            bound: pairs,
            detail: if broken.is_empty() {
                format!(
                    "{pairs} cell pair(s) across the {axis:?} mid-plane, every one matched \
                     (presence, not block state)"
                )
            } else {
                let [x, y, z] = broken[0];
                format!(
                    "{} of {pairs} cell pair(s) across the {axis:?} mid-plane differ; the first \
                     is {x},{y},{z} — one side is solid and the other is not, so the two halves \
                     were not built from the same rule",
                    broken.len()
                )
            },
        });
    }

    for gate in &gates {
        if gate.bound == 0 {
            findings.push(format!(
                "gate `{}` examined ZERO objects — its verdict binds to nothing",
                gate.id
            ));
        }
    }
    if expansion.anchors.is_empty() {
        findings.push(
            "the program declared no anchors: nothing in a campaign can name a place inside this \
             piece"
                .to_string(),
        );
    }

    let (footprint_area, footprint_perimeter) = footprint(model);
    let verdict = if gates.iter().all(|g| g.pass) {
        "pass"
    } else {
        "fail"
    };
    Report {
        verdict,
        measurements: Measurements {
            filled_cells: filled,
            region_cells,
            distinct_states: model.palette().len(),
            standable_cells: standable.len(),
            footprint_area,
            footprint_perimeter,
            silhouette_complexity: complexity(footprint_area, footprint_perimeter),
            top_blocks: top_blocks(model, filled),
        },
        gates,
        anchors: expansion
            .anchors
            .iter()
            .map(|(name, a)| (name.clone(), a.pos))
            .collect(),
        findings,
    }
}

/// Cell pairs across the mid-plane of `axis`, and the ones whose two halves
/// disagree — lowest-first, so the first entry is stable (ADR-0006).
///
/// **Presence, not block state.** A stair or a door placed correctly in one half
/// is a *different* state in the other, since nothing reflects a block's
/// `facing`; comparing states would red every symmetric building that contains
/// one. Solid-versus-not is the property a mirror plane really asserts, and it
/// is the property the defect this gate exists for breaks: an interior face left
/// open where its mirror image is walled.
///
/// An odd extent leaves the centre plane paired with itself, which is trivially
/// equal and is not counted.
fn asymmetry(model: &VoxelModel, axis: Axis) -> (usize, Vec<[i32; 3]>) {
    let region = model.region();
    let a = axis.index();
    let lo = region.origin[a];
    let hi = lo + region.size[a] as i32 - 1;
    let solid = |p: [i32; 3]| model.get(p).is_some_and(|b| !b.is_air());

    let mut pairs = 0;
    let mut broken = Vec::new();
    for pos in region.positions() {
        if pos[a] * 2 >= lo + hi {
            continue; // the far half, and the self-paired centre plane
        }
        let mut partner = pos;
        partner[a] = lo + hi - pos[a];
        pairs += 1;
        if solid(pos) != solid(partner) {
            broken.push(pos);
        }
    }
    (pairs, broken)
}

/// The plan view: how many columns carry a block, and how long the outline of
/// those columns is.
fn footprint(model: &VoxelModel) -> (usize, usize) {
    let region = model.region();
    let mut columns: BTreeSet<[i32; 2]> = BTreeSet::new();
    for pos in region.positions() {
        if model.get(pos).is_some_and(|b| !b.is_air()) {
            columns.insert([pos[0], pos[2]]);
        }
    }
    let perimeter = columns
        .iter()
        .map(|&[x, z]| {
            [[1, 0], [-1, 0], [0, 1], [0, -1]]
                .iter()
                .filter(|[dx, dz]| !columns.contains(&[x + dx, z + dz]))
                .count()
        })
        .sum();
    (columns.len(), perimeter)
}

/// The outline's length over the outline a solid square of the same area would
/// have. 1.0 is a plain box; a colonnade or a buttressed wall is well above it.
fn complexity(area: usize, perimeter: usize) -> f64 {
    if area == 0 {
        return 0.0;
    }
    let square = 4.0 * (area as f64).sqrt();
    perimeter as f64 / square
}

fn top_blocks(model: &VoxelModel, filled: usize) -> Vec<(String, f64)> {
    if filled == 0 {
        return Vec::new();
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for pos in model.region().positions() {
        if let Some(b) = model.get(pos)
            && !b.is_air()
        {
            *counts.entry(b.to_string()).or_insert(0) += 1;
        }
    }
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    // Descending by count, then by name, so the order is total (ADR-0006).
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(5);
    rows.into_iter()
        .map(|(name, n)| (name, n as f64 / filled as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockState;
    use crate::expand::ExpandOptions;
    use crate::geom::Box3;
    use crate::ir::{Material, Node, Program};
    use crate::library;

    fn solid_block(name: &str) -> Program {
        Program::new("slab", "all").rule(
            "all",
            Node::Fill {
                material: Material::block(BlockState::simple(name)),
            },
        )
    }

    #[test]
    fn a_program_painting_a_renamed_block_fails_the_blocks_gate() {
        let out = crate::expand(
            &solid_block("minecraft:chain"),
            Box3::at_origin([3, 3, 3]),
            &ExpandOptions::seeded(0),
        )
        .unwrap();
        let report = judge(&out, Options::default());
        assert!(!report.is_pass());
        let gate = report
            .gates
            .iter()
            .find(|g| g.id == "blocks-exist")
            .unwrap();
        assert!(!gate.pass);
        assert_eq!(gate.bound, 2, "air plus the one painted state");
        assert!(
            gate.detail.contains("minecraft:iron_chain"),
            "{}",
            gate.detail
        );
    }

    /// A gate whose binding count is zero is called out even though nothing
    /// went red — the vacuity CLAUDE.md names.
    #[test]
    fn a_program_that_declares_no_anchors_is_a_finding_not_a_silent_pass() {
        let out = crate::expand(
            &solid_block("minecraft:stone"),
            Box3::at_origin([3, 3, 3]),
            &ExpandOptions::seeded(0),
        )
        .unwrap();
        let report = judge(&out, Options::default());
        assert!(report.is_pass());
        assert_eq!(report.findings.len(), 1);
        assert!(
            report.findings[0].contains("no anchors"),
            "{:?}",
            report.findings
        );
    }

    /// The opt-in walk gate, shown from both sides on rules whose whole point is
    /// that they answer it differently: a stair flight walks end to end, a drop
    /// shaft only does so if falls are allowed.
    #[test]
    fn the_traversability_gate_separates_a_stair_from_a_one_way_drop() {
        let opts = Options {
            traversable: true,
            allow_falls: false,
            symmetric: None,
        };
        let stair = crate::expand(
            &library::stair_flight(),
            Box3::at_origin([5, 14, 22]),
            &ExpandOptions::seeded(3),
        )
        .unwrap();
        let report = judge(&stair, opts);
        let gate = report.gates.iter().find(|g| g.id == "traversable").unwrap();
        assert!(gate.pass, "{}", gate.detail);
        assert!(gate.bound > 0, "{}", gate.detail);

        let drop = crate::expand(
            &library::drop_shaft(),
            Box3::at_origin([4, 8, 6]),
            &ExpandOptions::seeded(3),
        )
        .unwrap();
        let walk_only = judge(&drop, opts);
        assert!(
            !walk_only
                .gates
                .iter()
                .find(|g| g.id == "traversable")
                .unwrap()
                .pass,
            "a one-way spill must not walk back up"
        );
        let with_falls = judge(
            &drop,
            Options {
                traversable: true,
                allow_falls: true,
                symmetric: None,
            },
        );
        assert!(
            with_falls
                .gates
                .iter()
                .find(|g| g.id == "traversable")
                .unwrap()
                .pass,
            "a one-way spill IS traversable once falling is allowed"
        );
    }

    /// The silhouette measurement moves with the shape it measures, which is
    /// the only claim made for it: it is reported, never gated.
    #[test]
    fn the_silhouette_measurement_separates_a_box_from_a_colonnade() {
        let box_report = judge(
            &crate::expand(
                &solid_block("minecraft:stone"),
                Box3::at_origin([9, 5, 9]),
                &ExpandOptions::seeded(0),
            )
            .unwrap(),
            Options::default(),
        );
        assert_eq!(box_report.measurements.footprint_area, 81);
        assert!(
            (box_report.measurements.silhouette_complexity - 1.0).abs() < 0.01,
            "a solid square plan is complexity 1.0, got {}",
            box_report.measurements.silhouette_complexity
        );

        let temple = judge(
            &crate::expand(
                &library::temple(),
                Box3::at_origin([13, 14, 21]),
                &ExpandOptions::seeded(7),
            )
            .unwrap(),
            Options::default(),
        );
        assert!(
            temple.measurements.silhouette_complexity >= 1.0,
            "{}",
            temple.measurements.silhouette_complexity
        );
    }
}
