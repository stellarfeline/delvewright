//! Declared jumps (spec-0113 §5.2, §6): a `world.jumps[]` entry is proven
//! makeable by flying its window, or refused (`DW1041`); a proven one is a move
//! the route relation takes.

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use delvewright_dsl::metrics::body::Rules;

/// One declared jump the build proved makeable: what the route takes and the
/// waypoints carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenJump {
    /// The authored id.
    pub id: String,
    /// The launch cell.
    pub from: [i32; 3],
    /// The landing cell.
    pub to: [i32; 3],
    /// Whether it is sprinted.
    pub sprint: bool,
    /// The straight run-up behind `from` the course gives, measured.
    pub run_up: u32,
}

/// What `DW1041` judged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JumpBinding {
    /// Jumps declared.
    pub declared: usize,
    /// Proven makeable.
    pub proven: usize,
    /// Refused.
    pub refused: usize,
    /// Of the refused, those across a gap the walk relation already crosses.
    pub not_a_jump: usize,
    /// The run-ups measured, one per jump judged.
    pub run_ups: Vec<u32>,
}

impl JumpBinding {
    /// The line a build prints (spec-0113 §5.5).
    pub fn line(&self) -> String {
        let range = match (self.run_ups.iter().min(), self.run_ups.iter().max()) {
            (Some(a), Some(b)) => format!("{a}..{b}"),
            _ => "none".to_string(),
        };
        format!(
            "jumps binding: {} declared, {} proven, {} refused, {} not a jump; run-ups measured {range}",
            self.declared, self.proven, self.refused, self.not_a_jump
        )
    }
}

/// Every rule set a declared jump is proven under: the jar's, which a player
/// plays by, and the bot's, which drives it (spec-0113 §3.1).
const PROVEN_UNDER: [Rules; 2] = [Rules::Game, Rules::Bot];

fn rules_words(r: Rules) -> &'static str {
    match r {
        Rules::Game => "the game's rules",
        Rules::Bot => "the bot's physics",
    }
}

/// `DW1041` over a campaign's declared jumps, against `world`.
pub fn check_declared_jumps(
    plan: &Plan,
    world: &World,
) -> (JumpBinding, Vec<ProvenJump>, Result<(), Failure>) {
    let mut b = JumpBinding {
        declared: plan.jumps.len(),
        ..JumpBinding::default()
    };
    let mut proven: Vec<ProvenJump> = Vec::new();
    let mut first: Option<Failure> = None;
    let walk = world.without_declared_jumps();
    for j in &plan.jumps {
        let refuse = |why: String| Failure {
            code: DW_JUMP_NOT_MAKEABLE,
            message: format!(
                "declared jump `{}` from {:?} to {:?} is not makeable: {why}. A declared jump is \
                 proven by flying it as a body on cue makes it — facing within {}° of the line \
                 between the two cells, the jump pressed on either of the last {} ticks before \
                 the edge, from the straight run-up the course gives — and every one of those \
                 must land with the body over the far support",
                j.id,
                j.from,
                j.to,
                delvewright_dsl::metrics::DECLARED_JUMP_FACING_DEG,
                delvewright_dsl::metrics::DECLARED_JUMP_PRESS_TICKS,
            ),
        };
        let standing = |c: [i32; 3]| world.rest_kind(c) == Some(RestKind::Standing);
        if !standing(j.from) || !standing(j.to) {
            b.refused += 1;
            let which = if standing(j.from) { j.to } else { j.from };
            first.get_or_insert_with(|| {
                refuse(format!(
                    "{which:?} is no place a body stands — the anchor must name the cell the \
                     body's feet are in, on a floor"
                ))
            });
            continue;
        }
        if walk.walk_region(&[j.from]).contains(&j.to) {
            b.refused += 1;
            b.not_a_jump += 1;
            first.get_or_insert_with(|| {
                refuse(
                    "the walk relation already joins the two cells, so the declaration confirms \
                     nothing — move an anchor across the gap the jump is meant to clear, or \
                     delete the declaration"
                        .to_string(),
                )
            });
            continue;
        }
        let run_up = world.run_up(j.from, j.to);
        b.run_ups.push(run_up);
        let miss = PROVEN_UNDER.iter().find_map(|r| {
            world
                .declared_jump(j.from, j.to, j.sprint, run_up, *r)
                .err()
        });
        match miss {
            None => {
                b.proven += 1;
                proven.push(ProvenJump {
                    id: j.id.clone(),
                    from: j.from,
                    to: j.to,
                    sprint: j.sprint,
                    run_up,
                });
            }
            Some(m) => {
                b.refused += 1;
                // The run-up that would make it: the shortest the course would
                // have to give, flown the same way over a run-up laid flat.
                let needed = (run_up + 1..=delvewright_dsl::metrics::DECLARED_JUMP_RUN_UP_CAP)
                    .find(|k| {
                        PROVEN_UNDER.iter().all(|r| {
                            world
                                .with_flat_run_up(j.from, j.to, *k)
                                .declared_jump(j.from, j.to, j.sprint, *k, *r)
                                .is_ok()
                        })
                    });
                let needed = match needed {
                    Some(k) => format!(
                        "it needs a straight run-up of {k} cell(s) behind the launch cell, and the \
                         course gives {run_up} — lengthen the run-up"
                    ),
                    None => format!(
                        "no run-up up to {} cell(s) makes it from this launch (the course gives \
                         {run_up}) — shorten the gap, lower the landing or raise the launch",
                        delvewright_dsl::metrics::DECLARED_JUMP_RUN_UP_CAP
                    ),
                };
                first.get_or_insert_with(|| {
                    refuse(format!(
                        "under {}, facing {:.0}° with the jump pressed on tick {} of the run, the \
                         body ended at [{:.2}, {:.2}, {:.2}]; {needed}",
                        rules_words(m.rules),
                        m.facing,
                        m.press,
                        m.ended[0],
                        m.ended[1],
                        m.ended[2]
                    ))
                });
            }
        }
    }
    let verdict = match first {
        Some(f) => Err(f),
        None => Ok(()),
    };
    (b, proven, verdict)
}
