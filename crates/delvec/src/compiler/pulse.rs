//! A sound beats over a place (spec-0102): a pulse's resolution, its derived
//! range, its listening stations on the forced route, and the binding it
//! prints.
//!
//! # The derived range
//!
//! The pinned client attenuates a positioned sound linearly: a listener at
//! distance `d ≤ R` hears gain `1 − d / R`, and `R = 16 · max(V, 1)` for an
//! event whose `sounds.json` row sets no `attenuation_distance` (spec-0102
//! §2.3). A pulse states `floor`, the gain at the farthest standing ear in its
//! place; the compiler measures that distance `far` over the standable cells
//! of the place (the cells every walk proof stands a body on, each judged at
//! its standing eye) and writes `R = far / (1 − floor)`, `V = max(R / 16, 1)`.
//!
//! The vendored sound registry lists ids only, so an event whose row sets its
//! own `attenuation_distance` (44 of the pinned game's events) is derived as
//! if it did not: its reach is the event's distance times `V`, not `R`.
//! Recorded in `compiler.md`'s pulse row.

use std::collections::BTreeSet;

use delvewright_dsl::{Diagnostic, DwCode, ExitTier};
use serde_json::{Value, json};

use crate::compiler::failure::Failure;
use crate::compiler::nav::{LegRoute, World};
use crate::compiler::plan::{Plan, StagedGate, safe_local};

delvewright_dsl::dw_code! {
    /// `DW0994`: **a pulse nobody can stand in hearing of** (spec-0102 §6.2).
    /// The box it is heard in holds no standable cell, so its farthest ear does
    /// not exist and its reach cannot be derived — the binding would be a zero.
    /// The remedy is the box.
    pub const DW_PULSE_UNHEARD: DwCode = DwCode::new("DW0994", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0995`: **a pulse the forced route never hears** (spec-0102 §6.3).
    /// Advisory: the forced route passes no configuration in which the pulse is
    /// live while a route cell lies in its box, so the ladder cannot exercise it
    /// and the bot reports it unheard. A beat the party need never hear is a
    /// design.
    pub const DW_PULSE_NOT_ON_ROUTE: DwCode = DwCode::new("DW0995", ExitTier::Build);
}

/// One gain step in vanilla's range rule: an event's range is 16 blocks times
/// its volume above 1, and 16 below (`SoundEvent.getRange`).
pub const BASE_RANGE: f64 = 16.0;

/// A resolved pulse (spec-0102): the declaration with its source and its box
/// placed in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct PulsePlan {
    /// The authored id (`pulse/<kebab>`).
    pub id: String,
    /// `safe_local(id)` — names the function and the latch holder.
    pub safe: String,
    /// The sound event, namespaced (`minecraft:entity.warden.heartbeat`).
    pub sound: String,
    /// The source cell: the mark's anchor cell plus its offset.
    pub cell: [i32; 3],
    /// Inclusive corners of the box it is heard in.
    pub heard: ([i32; 3], [i32; 3]),
    /// The interval in server ticks.
    pub every: u32,
    /// The declared loudness at the farthest standing ear.
    pub floor: f64,
    /// The pitch the line is written with.
    pub pitch: f64,
    /// The story stage it beats while, or `None` for a pulse that beats from
    /// world load.
    pub staged: Option<StagedGate>,
}

impl PulsePlan {
    /// The point the sound stands at: the source cell's centre.
    pub fn point(&self) -> [f64; 3] {
        [
            f64::from(self.cell[0]) + 0.5,
            f64::from(self.cell[1]) + 0.5,
            f64::from(self.cell[2]) + 0.5,
        ]
    }

    /// Whether `cell` lies in the box it is heard in.
    pub fn hears(&self, cell: [i32; 3]) -> bool {
        let (lo, hi) = self.heard;
        (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i])
    }
}

/// Every declared pulse resolved against the solved layout, in declaration
/// order. A pulse whose source or box does not resolve — an unprovided anchor
/// (`DW0142`), an unknown place (`DW0112`), neither or both of `region` /
/// `place` (`DW0929`) — is absent: validation has already refused it.
pub fn resolve(plan: &Plan) -> Vec<PulsePlan> {
    let c = plan.campaign;
    c.quests
        .content
        .pulses
        .iter()
        .filter_map(|p| {
            let anchor = plan.point_any(p.at.anchor.as_str())?;
            let heard = match (&p.heard.region, &p.heard.place) {
                (Some(zone), None) => plan.zone_box(zone)?,
                (None, Some(place)) => crate::compiler::horizon::place_bounds(plan, place)?,
                _ => return None,
            };
            let sound = if p.sound.contains(':') {
                p.sound.clone()
            } else {
                format!("minecraft:{}", p.sound)
            };
            Some(PulsePlan {
                id: p.id.as_str().to_string(),
                safe: safe_local(p.id.as_str()),
                sound,
                cell: p.at.cell(anchor),
                heard,
                every: p.every,
                floor: p.floor,
                pitch: p.pitch(),
                staged: p.when.as_ref().map(|g| StagedGate::of(c, g)),
            })
        })
        .collect()
}

/// `R` and `V` for a farthest ear `far` blocks from the source and a declared
/// `floor` (spec-0102 §4.1): `(R, V)`.
pub fn derived_range(far: f64, floor: f64) -> (f64, f64) {
    let range = far / (1.0 - floor);
    (range, (range / BASE_RANGE).max(1.0))
}

/// A cell of the forced route where the bot stands to listen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Station {
    /// The cell the body stands in.
    pub cell: [i32; 3],
    /// The critical-path step whose leg holds the cell.
    pub step: usize,
    /// The token of that step (an objective or trigger id) — what the harness
    /// keys "before this step" on.
    pub before: String,
}

/// One pulse, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct PulseRow {
    /// The resolved pulse.
    pub plan: PulsePlan,
    /// Standable cells in its box.
    pub cells: usize,
    /// The farthest standing eye from the source, in blocks.
    pub far: f64,
    /// The derived range `R`.
    pub range: f64,
    /// The emitted volume `V`.
    pub volume: f64,
    /// Whether some arrival on the forced route has it live.
    pub live_on_route: bool,
    /// Whether some live arrival's leg has a cell in its box.
    pub heard_on_route: bool,
    /// The first route cell inside the box under a live arrival, if any.
    pub listening: Option<Station>,
    /// The first route cell outside the box after it, while it stays live.
    pub silent: Option<Station>,
    /// When the route never hears it: why, in words.
    pub unheard: Option<String>,
}

/// What the pulse proofs examined (spec-0102 §5.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PulseGate {
    /// Pulses declared.
    pub declared: usize,
    /// The measured rows, in declaration order.
    pub rows: Vec<PulseRow>,
}

fn span(xs: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    xs.fold(None, |acc, x| match acc {
        None => Some((x, x)),
        Some((a, b)) => Some((a.min(x), b.max(x))),
    })
}

fn fmt_span(s: Option<(f64, f64)>, digits: usize) -> String {
    match s {
        None => "-".to_string(),
        Some((a, b)) => format!("{a:.digits$}..{b:.digits$}"),
    }
}

impl PulseGate {
    /// The binding line every build prints (spec-0102 §5.1).
    pub fn line(&self) -> String {
        let n = self.declared;
        let staged = self.rows.iter().filter(|r| r.plan.staged.is_some()).count();
        let cells = span(self.rows.iter().map(|r| r.cells as f64));
        let live = self.rows.iter().filter(|r| r.live_on_route).count();
        let heard = self.rows.iter().filter(|r| r.listening.is_some()).count();
        format!(
            "pulse binding: {n} pulse(s), {staged} staged; far {} block(s) over {} standable \
             cell(s), range {} (volume {}); {live} of {n} live on the forced route in some \
             configuration; {heard} of {n} with a listening station",
            fmt_span(span(self.rows.iter().map(|r| r.far)), 2),
            fmt_span(cells, 0),
            fmt_span(span(self.rows.iter().map(|r| r.range)), 2),
            fmt_span(span(self.rows.iter().map(|r| r.volume)), 3),
        )
    }

    /// The advisories: `DW0995` per pulse the forced route never hears.
    pub fn findings(&self) -> Vec<Diagnostic> {
        self.rows
            .iter()
            .filter_map(|r| {
                let why = r.unheard.as_ref()?;
                Some(Diagnostic::warning(
                    DW_PULSE_NOT_ON_ROUTE,
                    "build",
                    "pulse binding",
                    format!(
                        "pulse `{}` is never heard on the forced route: {why}. So the ladder \
                         cannot exercise it and the bot's report will say it was not heard \
                         (`not_heard: no station`). No change is required: a beat the party \
                         need never hear is a design.",
                        r.plan.id
                    ),
                ))
            })
            .collect()
    }

    /// `validation/pulses.json` (spec-0102 §5.1).
    pub fn to_json(&self) -> Value {
        let station = |s: &Option<Station>| match s {
            Some(s) => json!({ "cell": s.cell, "step": s.step, "before": s.before }),
            None => Value::Null,
        };
        json!({
            "declared": self.declared,
            "pulses": self.rows.iter().map(|r| {
                let p = &r.plan;
                json!({
                    "id": p.id,
                    "sound": p.sound,
                    "source": p.point(),
                    "box": { "min": p.heard.0, "max": p.heard.1 },
                    "every": p.every,
                    "floor": p.floor,
                    "pitch": p.pitch,
                    "standable_cells": r.cells,
                    "far": r.far,
                    "range": r.range,
                    "volume": r.volume,
                    "gate_terms": p.staged.as_ref().map_or_else(Vec::new, |g| {
                        g.terms.iter().map(|t| json!({
                            "objective": t.objective,
                            "party": t.party,
                            "min": t.min,
                            "max": t.max,
                            "negate": t.negate,
                        })).collect()
                    }),
                    "stations": {
                        "listening": station(&r.listening),
                        "silent": station(&r.silent),
                    },
                    "not_heard": if r.listening.is_some() {
                        Value::Null
                    } else {
                        json!("no station")
                    },
                })
            }).collect::<Vec<_>>(),
        })
    }
}

/// Every standable cell of `heard`, in x, y, z order (ADR-0006).
fn standable_in(world: &World, heard: ([i32; 3], [i32; 3])) -> Vec<[i32; 3]> {
    let (lo, hi) = heard;
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                if world.is_standable([x, y, z]) {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// **Measure every pulse** (spec-0102 §4.1, §5.3, §6.2–6.3) over the assembled
/// world and the forced route's proven legs: the derived range, the listening
/// and silent stations, and the refusal of a box nobody can stand in. The
/// binding is returned beside the verdict so the caller prints it on every
/// run, the refused run included.
pub fn measure(
    plan: &Plan,
    world: &World,
    routes: &[LegRoute],
) -> (PulseGate, Result<(), Failure>) {
    let plans = resolve(plan);
    let mut gate = PulseGate {
        declared: plan.campaign.quests.content.pulses.len(),
        rows: Vec::new(),
    };
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut refusal: Option<Failure> = None;
    for p in plans {
        let cells = standable_in(world, p.heard);
        if cells.is_empty() {
            if refusal.is_none() {
                let (lo, hi) = p.heard;
                let centre = [
                    (lo[0] + hi[0]).div_euclid(2),
                    (lo[1] + hi[1]).div_euclid(2),
                    (lo[2] + hi[2]).div_euclid(2),
                ];
                let reach = (0..3).map(|i| (hi[i] - lo[i]) / 2).max().unwrap_or(0) + 8;
                let nearest = world.snap_standable_fp(
                    centre,
                    reach.min(24),
                    &crate::compiler::nav::Footprint::player(),
                );
                let near = match nearest {
                    Some(c) => format!("the nearest standable cell to its centre is {c:?}"),
                    None => format!(
                        "no standable cell lies within {} blocks of its centre {centre:?}",
                        reach.min(24)
                    ),
                };
                refusal = Some(Failure {
                    code: DW_PULSE_UNHEARD,
                    message: format!(
                        "pulse `{}` is heard in the box {lo:?}..{hi:?}, and no cell of that box \
                         holds footing a body can stand in — so the beat has no farthest ear, \
                         its reach cannot be derived from `floor`, and nobody can ever stand in \
                         hearing of it; {near}. Draw the box over floor the party stands on: \
                         move or resize the `region` (its anchor and extent), or name the \
                         `place` the party walks.",
                        p.id
                    ),
                });
            }
            continue;
        }
        let point = p.point();
        let far = cells
            .iter()
            .map(|&c| dist(point, crate::compiler::view::sight::eye_of(c)))
            .fold(0.0f64, f64::max);
        let (range, volume) = derived_range(far, p.floor);
        let is_live = |arrival: usize| match &p.staged {
            None => true,
            Some(g) => {
                crate::compiler::nav::liveness_of(g, &plan.region_events, arrival, &ancestor).is
            }
        };
        let token = |step: usize| {
            plan.critical_path
                .get(step)
                .and_then(|s| s.objective().or(s.trigger()))
                .map(str::to_string)
        };
        let live_on_route = (0..=plan.critical_path.len()).any(is_live);
        let mut heard_on_route = false;
        let mut box_on_route = false;
        let mut listening: Option<(usize, usize, Station)> = None;
        let mut silent: Option<Station> = None;
        'legs: for (li, leg) in routes.iter().enumerate() {
            let live = is_live(leg.to_step);
            for (ci, &cell) in leg.cells.iter().enumerate() {
                let inside = p.hears(cell);
                box_on_route |= inside;
                if !live {
                    continue;
                }
                heard_on_route |= inside;
                match &listening {
                    None => {
                        if inside && let Some(before) = token(leg.to_step) {
                            listening = Some((
                                li,
                                ci,
                                Station {
                                    cell,
                                    step: leg.to_step,
                                    before,
                                },
                            ));
                        }
                    }
                    Some((lli, lci, _)) => {
                        if (li, ci) > (*lli, *lci)
                            && !inside
                            && let Some(before) = token(leg.to_step)
                        {
                            silent = Some(Station {
                                cell,
                                step: leg.to_step,
                                before,
                            });
                            break 'legs;
                        }
                    }
                }
            }
            // The silent station keeps the listening station's configuration:
            // a leg on which the pulse is no longer live ends the search.
            if listening.is_some() && !live {
                break;
            }
        }
        let unheard = (!heard_on_route).then(|| {
            if !box_on_route {
                format!(
                    "no cell the forced route walks lies in its box {:?}..{:?}",
                    p.heard.0, p.heard.1
                )
            } else {
                match &p.staged {
                    Some(g) => format!(
                        "the route crosses its box only where its gate does not hold — {}",
                        crate::compiler::lethal::never_held_term(
                            g,
                            &plan.region_events,
                            plan.critical_path.len().saturating_sub(1),
                            &ancestor,
                        )
                    ),
                    None => "the route crosses its box on no walked leg".to_string(),
                }
            }
        });
        gate.rows.push(PulseRow {
            cells: cells.len(),
            far,
            range,
            volume,
            live_on_route,
            heard_on_route,
            listening: listening.map(|(_, _, s)| s),
            silent,
            unheard,
            plan: p,
        });
    }
    (
        gate,
        match refusal {
            Some(f) => Err(f),
            None => Ok(()),
        },
    )
}

/// The ids of the pulses `rows` measured, for a claim (spec-0102 §5.2).
pub fn declared_safe(plan: &Plan) -> BTreeSet<String> {
    resolve(plan).into_iter().map(|p| p.safe).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `V = max(far / (16 · (1 − floor)), 1)` for three floors, and a place
    /// smaller than sixteen blocks is written at volume 1.
    #[test]
    fn the_range_is_derived_from_the_floor() {
        for (far, floor) in [(24.0, 0.4), (24.0, 0.0), (24.0, 0.75)] {
            let (r, v) = derived_range(far, floor);
            assert!((r - far / (1.0 - floor)).abs() < 1e-12);
            assert!((v - (far / (16.0 * (1.0 - floor))).max(1.0)).abs() < 1e-12);
        }
        assert_eq!(derived_range(6.0, 0.4).1, 1.0);
    }
}
