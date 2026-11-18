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
    /// cell a player stands in and no water a boat floats on, over the
    /// assembled world — or two checkpoints
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

delvewright_dsl::dw_code! {
    /// `DW1144`: a `reach-anchor` that runs a course, whose anchor is not
    /// inside the course's finish — the last checkpoint's box (spec-0139 §4).
    /// Such a reach is completed by the race being over, wherever the party
    /// stands, so the race ends at its finish; a route proven to the anchor
    /// past it walks to a place the game never asks for. The message names
    /// the objective, the anchor's cell, the race and the finish's box.
    pub const DW_COURSE_FINISH_OFF_ANCHOR: DwCode = DwCode::new("DW1144", ExitTier::Build);
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

    /// **Whether a racer can be in box `b`**: it holds a cell a player
    /// stands in, or a cell of the water's surface, where a boat floats and
    /// a swimmer's head is.
    pub fn box_holds_a_racer(&self, b: &CellBox) -> bool {
        if self.course_cell(b, &Footprint::player()).is_some() {
            return true;
        }
        let (lo, hi) = b;
        (lo[0]..=hi[0]).any(|x| {
            ((lo[1] - 1)..=hi[1]).any(|y| (lo[2]..=hi[2]).any(|z| self.is_water_surface([x, y, z])))
        })
    }
}

/// **A checkpoint's box**: its mark's cell, give or take its extent. `None`
/// when no placed piece provides the mark's anchor; validation names that
/// (`DW0142`). The one resolution every reader of a checkpoint takes — the
/// emitted selector, both routes and the proof.
pub fn checkpoint_box(plan: &Plan, cp: &delvewright_dsl::Checkpoint) -> Option<CellBox> {
    checkpoint_box_at(plan.point_any(cp.at.anchor.as_str())?, cp)
}

/// [`checkpoint_box`] from its anchor's cell, for a reader that holds the
/// resolved anchors before the plan exists ([`check_course_finishes`]).
fn checkpoint_box_at(anchor_cell: [i32; 3], cp: &delvewright_dsl::Checkpoint) -> Option<CellBox> {
    let c = cp.at.cell(anchor_cell);
    let e = cp.extent.map(|v| v as i32);
    Some((
        [c[0] - e[0], c[1] - e[1], c[2] - e[2]],
        [c[0] + e[0], c[1] + e[1], c[2] + e[2]],
    ))
}

/// **A reach that runs a course is reached at its finish** (`DW1144`): the
/// objective's anchor stands inside the last checkpoint's box, by the one
/// rule a racer is inside a box by ([`cell_in_box`]). The race being over
/// completes such a reach wherever the party stands, so the race's finish is
/// where the reach is done; an anchor elsewhere is a place the route is proven
/// to and the game never asks for. An anchor or a checkpoint that resolves
/// nowhere is left to the rule that names it (`DW0142`).
pub(crate) fn check_course_finishes(
    campaign: &delvewright_dsl::Campaign,
    point: &dyn Fn(&str) -> Option<[i32; 3]>,
) -> Result<(), crate::compiler::plan::PlanError> {
    let q = &campaign.quests.content;
    for o in q.quests.iter().flat_map(|x| &x.objectives) {
        let (delvewright_dsl::Objective::ReachAnchor { anchor, .. }, Some(race)) = (o, o.course())
        else {
            continue;
        };
        let Some(r) = q.races.iter().find(|r| &r.id == race) else {
            continue;
        };
        let Some(last) = r.checkpoints.last() else {
            continue;
        };
        let (Some(at), Some(cp_anchor)) = (point(anchor.as_str()), point(last.at.anchor.as_str()))
        else {
            continue;
        };
        let Some(finish) = checkpoint_box_at(cp_anchor, last) else {
            continue;
        };
        if cell_in_box(at, &finish) {
            continue;
        }
        return Err(crate::compiler::plan::PlanError::new(
            DW_COURSE_FINISH_OFF_ANCHOR,
            format!(
                "objective `{}` is reached at `{}` (the cell {:?}) and runs the course of `{}`, \
                 whose finish — checkpoint {} at `{}`, the box {:?}–{:?} — does not hold it. A \
                 reach that runs a course is completed by the race being over, wherever the \
                 party stands, so the party is done at the finish and never walks on to the \
                 anchor. Put the finish where the reach is done: move the last checkpoint of \
                 `{}` onto `{}`, or widen its `extent` until its box holds {:?}",
                o.id(),
                anchor,
                at,
                r.id,
                r.checkpoints.len(),
                last.at.display(),
                finish.0,
                finish.1,
                r.id,
                anchor,
                at
            ),
        ));
    }
    Ok(())
}

/// A race's checkpoints as boxes, once round the course.
pub fn lap_boxes(plan: &Plan, r: &delvewright_dsl::Race) -> Vec<CellBox> {
    r.checkpoints
        .iter()
        .filter_map(|cp| checkpoint_box(plan, cp))
        .collect()
}

/// A race's checkpoints as boxes, once per lap, in the order a racer passes
/// them.
pub fn course_boxes(plan: &Plan, r: &delvewright_dsl::Race) -> Vec<CellBox> {
    let lap = lap_boxes(plan, r);
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
             body stands in or floats on and for sharing none with the next, {} refused; {} actor racer(s); \
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
    for r in &q.races {
        gate.racers += r.racers.len();
        let boxes = lap_boxes(plan, r);
        if boxes.len() != r.checkpoints.len() {
            continue;
        }
        for (k, b) in boxes.iter().enumerate() {
            gate.checkpoints += 1;
            if !world.box_holds_a_racer(b) {
                gate.refused += 1;
                findings.push(Failure {
                    code: DW_CHECKPOINT_UNPASSABLE,
                    message: format!(
                        "race `{}`: checkpoint {} (at `{}`, the box {:?}–{:?}) holds no cell \
                         a body stands in or floats on over the assembled world — it is inside \
                         a wall, over a drop or above the ground a racer runs on, and nobody \
                         could be credited it. Centre it on the course's floor, or raise its `extent` \
                         until it reaches the ground.",
                        r.id,
                        k + 1,
                        r.checkpoints[k].at.display(),
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
