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
//!   at least the interaction reach
//!   ([`crate::compiler::crosshair::INTERACTION_REACH`]) horizontally away from
//!   the body's feet: the party's last walked cell before it comes into reach to
//!   talk. The bearing from the body's feet to that cell is the direction the
//!   party arrives from. The cells inside reach are not read: the route ends on
//!   the snapped station, often the body's own cell, and the one step into it is
//!   the search's tie-break, not a direction anybody walks. A leg that never
//!   leaves reach is read at its cell farthest from the body, one block or more
//!   away;
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
    /// to the party's last walked cell before it comes into reach to talk
    /// differs from the yaw the
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
    /// Talk steps whose body declares `watch: "none"`: judged from every cell
    /// within interaction reach ([`check_held`]), not from the walked approach.
    pub held: usize,
}

impl ApproachBinding {
    /// The line every build that walks prints, zeroes included.
    pub fn line(&self) -> String {
        format!(
            "approach binding (DW1025): {} talk beat(s) over {} configuration(s): {} judged by \
             facing ({} yaw(s), {} refused), {} drawn by a watch, {} held by declaration \
             (judged from every cell), {} reached by no walked leg, {} with no resolved body",
            self.talks,
            self.configurations,
            self.judged,
            self.yaws,
            self.refused,
            self.watched,
            self.held,
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
/// from the end, to the first cell at least the interaction reach horizontally
/// from `feet`; a leg that never leaves reach, at its farthest cell one block or
/// more away.
fn approach_cell(step: &Step, i: usize, routes: &[LegRoute], feet: [f64; 3]) -> Option<[i32; 3]> {
    let leg = routes
        .iter()
        .filter(|l| l.to_step <= i)
        .max_by_key(|l| l.to_step)?;
    let last = *leg.cells.last()?;
    if !step.completes_from(last) {
        return None;
    }
    let away = |c: [i32; 3]| {
        let p = feet_point(c);
        ((p[0] - feet[0]).powi(2) + (p[2] - feet[2]).powi(2)).sqrt()
    };
    let reach = crate::compiler::crosshair::INTERACTION_REACH;
    if let Some(c) = leg.cells.iter().rev().copied().find(|&c| away(c) >= reach) {
        return Some(c);
    }
    leg.cells
        .iter()
        .copied()
        .filter(|&c| away(c) >= 1.0)
        .max_by(|&a, &b| away(a).total_cmp(&away(b)))
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
    // Every body's watch, declared or default (`dsl::body_watch_sites`): the one
    // answer to whether a body turns, which the emitter reads too.
    let watches: std::collections::BTreeMap<String, delvewright_dsl::BodyWatch> =
        delvewright_dsl::body_watch_sites(plan.campaign)
            .into_iter()
            .map(|s| (s.body.id().to_string(), s.watch))
            .collect();
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
            if holds_its_facing(plan, npc_id) {
                bind.held += 1;
                continue;
            }
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
            let watch = watches
                .get(npc_id.as_str())
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
                         it comes into reach to talk is [{}, {}, {}], at bearing {toward:.0} from the body's feet, \
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

/// Whether the NPC `npc` declares `watch: "none"`.
fn holds_its_facing(plan: &Plan, npc: &str) -> bool {
    plan.campaign
        .npcs
        .content
        .npcs
        .iter()
        .any(|n| n.id.as_str() == npc && delvewright_dsl::BodyRef::Npc(n).holds_its_facing())
}

/// What the held-facing proof examined: every body that declares
/// `watch: "none"`, at every place it can stand, against every cell a player
/// can stand on within interaction reach of it.
#[derive(Clone, Debug, Default)]
pub struct HeldBinding {
    /// Bodies declaring `watch: "none"`.
    pub bodies: usize,
    /// Places those bodies stand with a fixed yaw: each home summon, and each
    /// walk's arrival.
    pub stances: usize,
    /// (stance, cell) pairs judged: a cell of the walked population within
    /// interaction reach of the stance.
    pub cells: usize,
    /// Of those, the pairs whose bearing is more than [`FRONT_DEGREES`] off
    /// the stance's yaw.
    pub behind: usize,
    /// Stances refused (one or more cells behind them).
    pub refused: usize,
    /// Stances no cell of the walked population is within reach of.
    pub unreached: usize,
}

impl HeldBinding {
    /// The line every build that walks prints, zeroes included.
    pub fn line(&self) -> String {
        format!(
            "held-facing binding (DW1025): {} cell(s) judged over {} body(ies) declaring \
             `watch: \"none\"` at {} stance(s): {} cell(s) behind a body, {} stance(s) \
             refused, {} stance(s) no walked cell reaches",
            self.cells, self.bodies, self.stances, self.behind, self.refused, self.unreached
        )
    }
}

/// Whether a player standing on `cell` can right-click a body whose feet are
/// at `feet`: the distance from the player's eye to the nearest point of the
/// body's interaction hitbox ([`crate::compiler::emit::NPC_HITBOX_WIDTH`] wide,
/// [`crate::compiler::emit::NPC_HITBOX_HEIGHT`] tall, on the body's feet) is
/// within [`crate::compiler::crosshair::INTERACTION_REACH`] — the length of the
/// pick ray the client traces from the eye.
pub fn within_interaction_reach(feet: [f64; 3], cell: [i32; 3]) -> bool {
    let stand = feet_point(cell);
    let eye = [
        stand[0],
        stand[1] + delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
        stand[2],
    ];
    let half = crate::compiler::emit::NPC_HITBOX_WIDTH / 2.0;
    let dx = ((eye[0] - feet[0]).abs() - half).max(0.0);
    let dz = ((eye[2] - feet[2]).abs() - half).max(0.0);
    let top = feet[1] + crate::compiler::emit::NPC_HITBOX_HEIGHT;
    let dy = if eye[1] < feet[1] {
        feet[1] - eye[1]
    } else if eye[1] > top {
        eye[1] - top
    } else {
        0.0
    };
    (dx * dx + dy * dy + dz * dz).sqrt() <= crate::compiler::crosshair::INTERACTION_REACH
}

/// Every place `npc` stands with a yaw the compiler fixed: its home summon,
/// then each walk's arrival, in plan order, each once.
fn every_stance(plan: &Plan, moves: &[MovePlan], decl: &delvewright_dsl::Npc) -> Vec<Stance> {
    let (home, home_yaw) = crate::compiler::watching::npc_home(plan, decl);
    let mut out = vec![Stance {
        feet: feet_point(home),
        cell: home,
        yaw: wrap_degrees(f64::from(home_yaw)),
        source: format!("its home anchor `{}`'s facing", decl.anchor.as_str()),
    }];
    for m in moves.iter().filter(|m| m.npc == decl.id.as_str()) {
        let Some(&y) = m.yaws.last() else { continue };
        let yaw = wrap_degrees(f64::from(y));
        if out.iter().any(|s| s.cell == m.target && s.yaw == yaw) {
            continue;
        }
        out.push(Stance {
            feet: feet_point(m.target),
            cell: m.target,
            yaw,
            source: format!(
                "the arrival turn of its `move-npc` to `{}`, which is that anchor's declared \
                 facing or else the reverse of the walk's last leg",
                m.to.display()
            ),
        });
    }
    out
}

/// The cardinal facings an anchor declares, with the yaw each writes.
const CARDINALS: [(&str, f64); 4] = [
    ("south", 0.0),
    ("west", 90.0),
    ("north", 180.0),
    ("east", -90.0),
];

/// `DW1025` for a body that declares `watch: "none"`: at every place it stands
/// with a fixed yaw, every cell of the walked population `P`
/// ([`crate::compiler::lethal::walked_population`], the population the watchers
/// are judged over) from which a player can talk to it
/// ([`within_interaction_reach`]) lies in its front half-plane. Cells in the
/// body's own column (less than a block away horizontally) have no bearing
/// and are not judged; a cell inside a killing volume holds no live player
/// and is not judged.
pub fn check_held(
    plan: &Plan,
    population: &std::collections::BTreeSet<[i32; 3]>,
    moves: &[MovePlan],
) -> (HeldBinding, Vec<Failure>) {
    let mut bind = HeldBinding::default();
    let mut findings = Vec::new();
    for decl in &plan.campaign.npcs.content.npcs {
        if !delvewright_dsl::BodyRef::Npc(decl).holds_its_facing() {
            continue;
        }
        if plan
            .body_point(delvewright_dsl::BodyRef::Npc(decl))
            .is_none()
        {
            continue;
        }
        bind.bodies += 1;
        let id = decl.id.as_str();
        for s in every_stance(plan, moves, decl) {
            bind.stances += 1;
            let judged: Vec<([i32; 3], f64)> = population
                .iter()
                .copied()
                .filter(|&c| {
                    let p = feet_point(c);
                    let horizontal =
                        ((p[0] - s.feet[0]).powi(2) + (p[2] - s.feet[2]).powi(2)).sqrt();
                    horizontal >= 1.0
                        && within_interaction_reach(s.feet, c)
                        && !crate::compiler::watching::in_lethal_volume(plan, c)
                })
                .map(|c| (c, bearing(s.feet, feet_point(c))))
                .collect();
            if judged.is_empty() {
                bind.unreached += 1;
                continue;
            }
            bind.cells += judged.len();
            // The game's own degree constant leaves a side-on bearing a few
            // millionths past 90; the half-plane is closed.
            let off = |yaw: f64, b: f64| yaw_difference(b, yaw) > FRONT_DEGREES + 1e-3;
            let behind: Vec<&([i32; 3], f64)> =
                judged.iter().filter(|(_, b)| off(s.yaw, *b)).collect();
            if behind.is_empty() {
                continue;
            }
            bind.behind += behind.len();
            bind.refused += 1;
            let (worst, wb) = behind
                .iter()
                .map(|&&(c, b)| (c, b))
                .max_by(|a, b| {
                    yaw_difference(a.1, s.yaw)
                        .total_cmp(&yaw_difference(b.1, s.yaw))
                        .then_with(|| b.0.cmp(&a.0))
                })
                .expect("non-empty");
            let anchor = if s.source.starts_with("its home anchor") {
                decl.anchor.as_str().to_string()
            } else {
                moves
                    .iter()
                    .find(|m| m.npc == id && m.target == s.cell)
                    .map(|m| m.to.anchor.as_str().to_string())
                    .unwrap_or_else(|| decl.anchor.as_str().to_string())
            };
            let turn = CARDINALS
                .iter()
                .find(|(_, y)| judged.iter().all(|(_, b)| !off(*y, *b)));
            let arrange = match turn {
                Some((name, _)) => format!(
                    "Declare `{anchor}`'s facing `{name}` in its piece's metadata (`delvec prefab \
                     anchor <piece>.nbt --name {anchor} --pos <x,y,z> --facing {name}`), which \
                     faces every one of those cells"
                ),
                None => format!(
                    "No one facing faces every one of those cells from there: set its `offset` \
                     from `{anchor}` (and the cast's mark with it) so a wall or a counter is at \
                     its back and the cells a player reaches lie in front of it"
                ),
            };
            findings.push(Failure::new(
                DW_BACK_TO_THE_PARTY,
                format!(
                    "`{id}` declares `watch: \"none\"`, so it holds yaw {:.0} ({}) standing at \
                     [{}, {}, {}], and {} of the {} cell(s) a player can talk to it from lie \
                     behind it — more than {FRONT_DEGREES:.0} degrees off the way it faces. The \
                     one most behind it is [{}, {}, {}], at bearing {:.0} from the body's feet, \
                     {:.0} degrees off. A body that does not turn is judged from every cell a \
                     player can talk to it from, not only the route's approach. {arrange}; or \
                     remove `\"watch\": \"none\"` from `{id}` in npcs.json, so it watches the \
                     nearest player within {} blocks, the default",
                    s.yaw,
                    s.source,
                    s.cell[0],
                    s.cell[1],
                    s.cell[2],
                    behind.len(),
                    judged.len(),
                    worst[0],
                    worst[1],
                    worst[2],
                    wb.round() + 0.0,
                    yaw_difference(wb, s.yaw),
                    delvewright_dsl::DEFAULT_WATCH_WITHIN,
                ),
            ));
        }
    }
    (bind, findings)
}
