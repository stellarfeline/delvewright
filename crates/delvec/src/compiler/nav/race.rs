//! **A race's course over the assembled world** (spec-0139 §6) — `DW1118`,
//! the stops a route through the course is planned by, and the binding line.
//!
//! A checkpoint is a box, and a racer is inside it when its hitbox meets the
//! box. The build's routes — a ridden leg of the party (`nav::vehicle`) and a
//! `move-actor`'s walk `along` the race (`nav::actor`) — each ask one
//! question of a cell, [`cell_in_box`]: does a body whose feet are in this
//! cell meet the box. One rule, so the two routes and the emitted selector's
//! box agree on what "inside" means.

use delvewright_dsl::{DwCode, ExitTier, Verb};

use super::World;
use super::world::Footprint;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1118`: a race's checkpoint no body can be in — its box holds no
    /// cell a player stands in over the assembled world — or two checkpoints
    /// that follow one another on the course share a cell, so one body is in
    /// both at once and the second is credited where the first was (spec-0139
    /// §6). The message names the race, the checkpoint and the box.
    pub const DW_CHECKPOINT_UNPASSABLE: DwCode = DwCode::new("DW1118", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1119`: an objective declares a race its `course`, and that race is
    /// not running when the path reaches the objective's step — no
    /// `start-race` of it fires before the step, or the last line of the
    /// race the path fires before it is an `end-race` (spec-0139 §4). The
    /// party would ride the course credited nothing, and the objective
    /// would never complete.
    pub const DW_COURSE_NOT_STARTED: DwCode = DwCode::new("DW1119", ExitTier::Build);
}

/// An inclusive box of cells.
pub type CellBox = ([i32; 3], [i32; 3]);

/// **Whether a body whose feet are in cell `c` meets `b`**: the cell's column
/// is inside the box, and the box reaches the feet cell or the one above it —
/// a standing body fills both, and a rider's body, seated above its vehicle's
/// feet, meets both.
pub fn cell_in_box(c: [i32; 3], b: &CellBox) -> bool {
    let (lo, hi) = b;
    (lo[0]..=hi[0]).contains(&c[0])
        && (lo[2]..=hi[2]).contains(&c[2])
        && lo[1] <= c[1] + 1
        && hi[1] >= c[1]
}

/// Whether two boxes share a cell.
fn boxes_meet(a: &CellBox, b: &CellBox) -> bool {
    (0..3).all(|i| a.0[i] <= b.1[i] && b.0[i] <= a.1[i])
}

impl World {
    /// **The cell of box `b` a body of footprint `fp` is routed through**: the
    /// standable cell that meets the box ([`cell_in_box`]) nearest its centre,
    /// then the least cell. `None` when the box holds none.
    pub fn course_cell(&self, b: &CellBox, fp: &Footprint) -> Option<[i32; 3]> {
        let (lo, hi) = b;
        let mid = [
            (lo[0] + hi[0]) / 2,
            (lo[1] + hi[1]) / 2,
            (lo[2] + hi[2]) / 2,
        ];
        let mut best: Option<(i64, [i32; 3])> = None;
        for x in lo[0]..=hi[0] {
            for y in (lo[1] - 1)..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let c = [x, y, z];
                    if !self.standable_fp(c, fp) {
                        continue;
                    }
                    let d: i64 = (0..3).map(|i| i64::from(c[i] - mid[i]).pow(2)).sum();
                    if best.is_none_or(|(bd, bc)| (d, c) < (bd, bc)) {
                        best = Some((d, c));
                    }
                }
            }
        }
        best.map(|(_, c)| c)
    }
}

/// A race's checkpoints as resolved boxes, once per lap, in the order a
/// racer passes them. A checkpoint whose anchor does not resolve is skipped;
/// validation names it (`DW0142`).
pub fn course_boxes(plan: &Plan, r: &delvewright_dsl::Race) -> Vec<CellBox> {
    let lap: Vec<CellBox> = r
        .checkpoints
        .iter()
        .filter_map(|cp| plan.zone_box(cp))
        .collect();
    (0..r.laps).flat_map(|_| lap.iter().copied()).collect()
}

/// What the race proof read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RaceGate {
    /// Races declared.
    pub races: usize,
    /// Checkpoints judged, over every race.
    pub checkpoints: usize,
    /// Actors listed as racers, over every race.
    pub racers: usize,
    /// Objectives that declare a course.
    pub courses: usize,
    /// `move-actor`s walked along a course.
    pub walks: usize,
    /// Checkpoints refused.
    pub refused: usize,
}

impl RaceGate {
    /// The one line this proof owes its reader, on every build that
    /// assembles a world and declares a race.
    pub fn line(&self) -> String {
        format!(
            "race binding (DW1117–DW1119): {} race(s), {} checkpoint(s) judged for a cell a \
             body stands in and for sharing none with the next, {} refused; {} actor racer(s); \
             {} objective(s) run a course; {} walk(s) follow one",
            self.races, self.checkpoints, self.refused, self.racers, self.courses, self.walks
        )
    }
}

/// Whether the campaign declares a race.
pub fn declares_one(plan: &Plan) -> bool {
    !plan.campaign.quests.content.races.is_empty()
}

/// **Every checkpoint of every race can be passed, one at a time**
/// (`DW1118`), with the binding stated whichever way it goes.
pub fn check(plan: &Plan, world: &World) -> (RaceGate, Vec<Failure>) {
    let q = &plan.campaign.quests.content;
    let mut gate = RaceGate {
        races: q.races.len(),
        courses: q
            .quests
            .iter()
            .flat_map(|x| &x.objectives)
            .filter(|o| o.course().is_some())
            .count(),
        ..RaceGate::default()
    };
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |_, e| {
        if matches!(&e.verb, Verb::MoveActor { along: Some(_), .. }) {
            gate.walks += 1;
        }
    });
    let mut findings = Vec::new();
    let body = Footprint::player();
    for r in &q.races {
        gate.racers += r.racers.len();
        let boxes: Vec<CellBox> = r
            .checkpoints
            .iter()
            .filter_map(|cp| plan.zone_box(cp))
            .collect();
        for (k, b) in boxes.iter().enumerate() {
            gate.checkpoints += 1;
            if world.course_cell(b, &body).is_none() {
                gate.refused += 1;
                findings.push(Failure {
                    code: DW_CHECKPOINT_UNPASSABLE,
                    message: format!(
                        "race `{}`: checkpoint {} (anchor `{}`, the box {:?}–{:?}) holds no cell \
                         a body stands in over the assembled world — it is inside a wall, over \
                         a drop or above the ground a racer runs on, and nobody could be \
                         credited it. Centre it on the course's floor, or raise its `extent` \
                         until it reaches the ground.",
                        r.id,
                        k + 1,
                        r.checkpoints[k].anchor,
                        b.0,
                        b.1
                    ),
                });
            }
            // The checkpoint after this one: the next written, and after the
            // last the first when the course is run again.
            let next = if k + 1 < boxes.len() {
                Some(k + 1)
            } else if r.laps > 1 && boxes.len() > 1 {
                Some(0)
            } else {
                None
            };
            if let Some(n) = next
                && boxes_meet(b, &boxes[n])
            {
                gate.refused += 1;
                findings.push(Failure {
                    code: DW_CHECKPOINT_UNPASSABLE,
                    message: format!(
                        "race `{}`: checkpoint {} (the box {:?}–{:?}) and checkpoint {}, which \
                         follows it (the box {:?}–{:?}), share a cell: a body standing there is \
                         in both, and is credited the second where it was credited the first — \
                         the course has one checkpoint fewer than it declares. Move one of \
                         them along the course, or shrink an `extent`, until the boxes are \
                         apart.",
                        r.id,
                        k + 1,
                        b.0,
                        b.1,
                        n + 1,
                        boxes[n].0,
                        boxes[n].1
                    ),
                });
            }
        }
    }
    (gate, findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_in_a_box_its_feet_cell_or_the_one_above_meets() {
        let b = ([0, 5, 0], [2, 6, 2]);
        assert!(cell_in_box([1, 5, 1], &b));
        // Feet one below the box: the cell above the feet is in it.
        assert!(cell_in_box([1, 4, 1], &b));
        // Feet in the box's top layer.
        assert!(cell_in_box([1, 6, 1], &b));
        assert!(!cell_in_box([1, 3, 1], &b));
        assert!(!cell_in_box([1, 7, 1], &b));
        assert!(!cell_in_box([3, 5, 1], &b));
    }

    #[test]
    fn boxes_that_touch_at_a_cell_meet_and_boxes_a_cell_apart_do_not() {
        let a = ([0, 0, 0], [2, 2, 2]);
        assert!(boxes_meet(&a, &([2, 0, 0], [4, 2, 2])));
        assert!(!boxes_meet(&a, &([3, 0, 0], [5, 2, 2])));
    }
}
