//! **A body the party talks to faces the party as they walk up** (`DW1025`).
//!
//! A still body keeps the yaw it was given: its summon writes its home
//! anchor's declared facing ([`crate::compiler::watching::npc_home`]), and a walk
//! leaves it on the walk's arrival turn (`nav::apply_arrival_yaw`). Neither of
//! those knows which way the party comes from. Unless the body watches
//! (spec-0101), the yaw it holds when a `talk-to` is owed is the yaw the party
//! sees, and a body facing away reads as a character with its back to the person
//! talking to it.
//!
//! # What is compared
//!
//! Per configuration — the exported critical path and every realised branch's
//! own path, each with the legs its route proof walked
//! (`emit::routes`'s `path_legs`) — and per `talk-to` step on it:
//!
//! * **the approach**: the leg that delivers the party to the step (the last leg
//!   whose `to_step` is at or before it and whose last cell completes the step,
//!   [`Step::completes_from`]), read back from its last cell to the first cell
//!   one block or more horizontally away from the body's feet. That cell is the
//!   party's last walked place before the talk, and the bearing from the body's
//!   feet to it is the direction the party arrives from;
//! * **the body's yaw at that moment**: the home summon's, when the step's
//!   station is the body's home cell, or the arrival yaw of every `move-npc`
//!   whose destination mark is the step's station
//!   ([`crate::compiler::nav::MovePlan::mark_cell`]); every such yaw is judged.
//!
//! A body faces the party when its yaw is within [`FRONT_DEGREES`] of the
//! approach bearing: the party arrives in the body's front half-plane. That
//! threshold is authored, not cited: it is the least reading of "not with its
//! back to them".
//!
//! A body that watches `nearest` (spec-0101) and whose reach covers the approach
//! cell turns to the party by itself, and is counted as drawn rather than judged.
//! A class watch turns only to players of that class, and the party member who
//! talks may be another, so it is judged by its yaw like a body that does not
//! watch.
//!
//! # The binding
//!
//! Every build that walks prints [`ApproachBinding::line`], zeroes included: the
//! talk beats over every configuration, how many were judged by facing, drawn by
//! a watch, reached by no walked leg (a ride, or a talk with no leg before it),
//! or whose body no station resolution places.

use delvewright_dsl::{DwCode, ExitTier};

use crate::compiler::failure::Failure;
use crate::compiler::nav::{LegRoute, MovePlan};
use crate::compiler::plan::{Plan, Step};
use crate::compiler::watching::{bearing, feet_distance, feet_point, wrap_degrees, yaw_difference};

delvewright_dsl::dw_code! {
    /// `DW1025`: **a body the party talks to has its back to them.** At a
    /// `talk-to` step of some configuration, the bearing from the body's feet
    /// to the party's last walked cell before the talk differs from the yaw the
    /// body holds at that beat — its home anchor's facing, or the arrival turn
    /// of the walk that left it there — by more than [`FRONT_DEGREES`], and the
    /// body does not watch the party from there. Names the body, the
    /// objective, the configuration, both bearings, and the `watch` that turns
    /// it.
    pub const DW_BACK_TO_THE_PARTY: DwCode = DwCode::new("DW1025", ExitTier::Build);
}

/// The widest angle, in degrees, between a body's yaw and the bearing to the
/// party's approach at which the body still faces them: the front half-plane.
pub const FRONT_DEGREES: f64 = 90.0;

/// What the approach proof examined.
#[derive(Clone, Debug, Default)]
pub struct ApproachBinding {
    /// Configurations walked (the critical path plus each realised branch).
    pub configurations: usize,
    /// `talk-to` steps over every configuration.
    pub talks: usize,
    /// Talk steps judged by the body's yaw.
    pub judged: usize,
    /// Talk steps whose body watches `nearest` within reach of the approach.
    pub watched: usize,
    /// Body yaws judged (a talk whose station several walks end on judges each).
    pub yaws: usize,
    /// Body yaws refused.
    pub refused: usize,
    /// Talk steps no walked leg delivers the party to.
    pub unwalked: usize,
    /// Talk steps whose body no station resolution places.
    pub unplaced: usize,
}

impl ApproachBinding {
    /// The line every build that walks prints, zeroes included.
    pub fn line(&self) -> String {
        format!(
            "approach binding (DW1025): {} talk beat(s) over {} configuration(s): {} judged by \
             facing ({} yaw(s), {} refused), {} drawn by a watch, {} reached by no walked leg, \
             {} with no resolved body",
            self.talks,
            self.configurations,
            self.judged,
            self.yaws,
            self.refused,
            self.watched,
            self.unwalked,
            self.unplaced
        )
    }
}

/// One yaw a body can hold at a talk beat, and where it came from.
struct Stance {
    feet: [f64; 3],
    cell: [i32; 3],
    yaw: f64,
    source: String,
}

/// The cell the party approaches from: the delivering leg's cells, read back
/// from the end, to the first cell at least one block horizontally from `feet`.
fn approach_cell(step: &Step, i: usize, routes: &[LegRoute], feet: [f64; 3]) -> Option<[i32; 3]> {
    let leg = routes
        .iter()
        .filter(|l| l.to_step <= i)
        .max_by_key(|l| l.to_step)?;
    let last = *leg.cells.last()?;
    if !step.completes_from(last) {
        return None;
    }
    leg.cells.iter().rev().copied().find(|&c| {
        let p = feet_point(c);
        ((p[0] - feet[0]).powi(2) + (p[2] - feet[2]).powi(2)).sqrt() >= 1.0
    })
}

/// Every yaw `npc` can hold standing at `station`.
fn stances(plan: &Plan, moves: &[MovePlan], npc: &str, station: [i32; 3]) -> Vec<Stance> {
    let mut out = Vec::new();
    let Some(decl) = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc)
    else {
        return out;
    };
    let (home, home_yaw) = crate::compiler::watching::npc_home(plan, decl);
    if home == station {
        out.push(Stance {
            feet: feet_point(home),
            cell: home,
            yaw: wrap_degrees(f64::from(home_yaw)),
            source: format!("its home anchor `{}`'s facing", decl.anchor.as_str()),
        });
    }
    for m in moves
        .iter()
        .filter(|m| m.npc == npc && m.mark_cell == station)
    {
        let Some(&y) = m.yaws.last() else { continue };
        let yaw = wrap_degrees(f64::from(y));
        if out.iter().any(|s| s.cell == m.target && s.yaw == yaw) {
            continue;
        }
        out.push(Stance {
            feet: feet_point(m.target),
            cell: m.target,
            yaw,
            source: format!("the arrival turn of its `move-npc` to `{}`", m.to.display()),
        });
    }
    out
}

/// `DW1025` over every `talk-to` step of every configuration. Returns the
/// binding beside the refusals, so the line a run prints counts every beat.
pub fn check(
    plan: &Plan,
    path_legs: &[(String, Vec<Step>, Vec<LegRoute>)],
    moves: &[MovePlan],
) -> (ApproachBinding, Vec<Failure>) {
    let mut bind = ApproachBinding {
        configurations: path_legs.len(),
        ..ApproachBinding::default()
    };
    let mut findings: Vec<Failure> = Vec::new();
    let mut reported: std::collections::BTreeSet<(String, String, [i32; 3], i64)> =
        std::collections::BTreeSet::new();
    for (label, steps, routes) in path_legs {
        let config = if label == "critical-path" {
            "the critical path".to_string()
        } else {
            format!("branch `{label}`")
        };
        for (i, step) in steps.iter().enumerate() {
            let Step::TalkTo {
                objective_id,
                npc_id,
                pos,
                ..
            } = step
            else {
                continue;
            };
            bind.talks += 1;
            let here = stances(plan, moves, npc_id, *pos);
            if here.is_empty() {
                bind.unplaced += 1;
                continue;
            }
            let Some(from) = approach_cell(step, i, routes, here[0].feet) else {
                bind.unwalked += 1;
                continue;
            };
            let party = feet_point(from);
            let watch = plan
                .campaign
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.id.as_str() == npc_id.as_str())
                .and_then(|n| n.watch.as_ref())
                .filter(|w| w.who.class().is_none());
            if let Some(w) = watch
                && here
                    .iter()
                    .all(|s| feet_distance(s.feet, party) <= f64::from(w.within.get()))
            {
                bind.watched += 1;
                continue;
            }
            bind.judged += 1;
            for s in &here {
                bind.yaws += 1;
                let toward = bearing(s.feet, party);
                let off = yaw_difference(toward, s.yaw);
                // The game's own degree constant leaves a side-on bearing a few
                // millionths past 90; the half-plane is closed.
                if off <= FRONT_DEGREES + 1e-3 {
                    continue;
                }
                bind.refused += 1;
                let key = (npc_id.clone(), objective_id.clone(), s.cell, s.yaw as i64);
                if !reported.insert(key) {
                    continue;
                }
                // The talk is made from anywhere within interaction reach, so the
                // watch that answers it covers that reach as well as the approach.
                let reach = feet_distance(s.feet, party)
                    .ceil()
                    .max(crate::compiler::crosshair::INTERACTION_REACH)
                    as i64;
                findings.push(Failure::new(
                    DW_BACK_TO_THE_PARTY,
                    format!(
                        "`{npc_id}` is talked to at `{objective_id}` on {config}, standing at \
                         [{}, {}, {}] with yaw {:.0} ({}). The party's last walked cell before \
                         the talk is [{}, {}, {}], at bearing {toward:.0} from the body's feet, \
                         {off:.0} degrees off the way it faces — more than {FRONT_DEGREES:.0}, so \
                         the party walks up to its back. Declare `watch: {{ \"who\": \"nearest\", \
                         \"within\": {reach} }}` on `{npc_id}` in npcs.json, so the body turns to \
                         the party as they arrive",
                        s.cell[0], s.cell[1], s.cell[2], s.yaw, s.source, from[0], from[1], from[2],
                    ),
                ));
            }
        }
    }
    (bind, findings)
}
