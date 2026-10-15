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
    judge_jumps(&plan.jumps, world)
}

/// [`check_declared_jumps`] over the resolved jumps alone.
pub(in crate::compiler::nav) fn judge_jumps(
    jumps: &[crate::compiler::plan::JumpPlan],
    world: &World,
) -> (JumpBinding, Vec<ProvenJump>, Result<(), Failure>) {
    let mut b = JumpBinding {
        declared: jumps.len(),
        ..JumpBinding::default()
    };
    let mut proven: Vec<ProvenJump> = Vec::new();
    let mut first: Option<Failure> = None;
    let walk = world.without_declared_jumps();
    for j in jumps {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::plan::JumpPlan;
    use std::collections::BTreeSet;

    /// Two platforms five wide, feet at y=1: a run-up of `run` cells ending at
    /// x=0, `gap` air columns, a landing platform beyond.
    fn course(run: i32, gap: i32) -> World {
        let mut solid = BTreeSet::new();
        for z in -2..=2 {
            for x in -run..=0 {
                solid.insert([x, 0, z]);
            }
            for x in (1 + gap)..(5 + gap) {
                solid.insert([x, 0, z]);
            }
        }
        World::from_solid_cells(solid)
    }

    fn jump(from: [i32; 3], to: [i32; 3]) -> JumpPlan {
        JumpPlan {
            id: "jump/test".to_string(),
            from,
            to,
            sprint: true,
        }
    }

    #[test]
    fn a_two_column_sprint_jump_with_a_run_up_is_proven() {
        let w = course(6, 2);
        let (b, proven, verdict) = judge_jumps(&[jump([0, 1, 0], [3, 1, 0])], &w);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(
            (b.declared, b.proven, b.refused, b.not_a_jump),
            (1, 1, 0, 0)
        );
        assert_eq!(b.run_ups, vec![6]);
        assert_eq!(proven[0].run_up, 6);
        assert!(
            b.line()
                .contains("1 declared, 1 proven, 0 refused, 0 not a jump"),
            "{}",
            b.line()
        );
    }

    #[test]
    fn a_five_column_leap_no_body_makes_is_dw1041() {
        let w = course(6, 5);
        let (b, _, verdict) = judge_jumps(&[jump([0, 1, 0], [6, 1, 0])], &w);
        let err = verdict.expect_err("five columns of air is no jump");
        assert_eq!(err.code.id(), "DW1041");
        assert!(err.message.contains("jump/test"), "{}", err.message);
        assert!(err.message.contains("facing"), "{}", err.message);
        assert!(err.message.contains("no run-up up to 6"), "{}", err.message);
        assert_eq!((b.proven, b.refused), (0, 1));
    }

    #[test]
    fn a_short_run_up_names_the_run_up_it_needs() {
        // Four columns down one: from a standing start in the launch cell the
        // window misses, and one cell of run-up behind it is enough.
        let mut solid = BTreeSet::new();
        for z in -2..=2 {
            solid.insert([0, 0, z]);
            for x in 5..9 {
                solid.insert([x, -1, z]);
            }
        }
        let w = World::from_solid_cells(solid);
        let (b, _, verdict) = judge_jumps(&[jump([0, 1, 0], [5, 0, 0])], &w);
        let err = verdict.expect_err("no run-up");
        assert!(
            err.message.contains("the course gives 0"),
            "{}",
            err.message
        );
        assert!(
            err.message.contains("needs a straight run-up of 1 cell(s)"),
            "{}",
            err.message
        );
        assert_eq!(b.run_ups, vec![0]);
    }

    #[test]
    fn a_jump_across_a_walk_is_not_a_jump() {
        let w = course(6, 0);
        let (b, _, verdict) = judge_jumps(&[jump([0, 1, 0], [2, 1, 0])], &w);
        let err = verdict.expect_err("the walk crosses it");
        assert!(err.message.contains("already joins"), "{}", err.message);
        assert_eq!((b.proven, b.refused, b.not_a_jump), (0, 1, 1));
        assert!(b.line().contains("0 proven, 1 refused, 1 not a jump"));
    }

    #[test]
    fn a_proven_jump_is_a_move_of_the_walk_relation() {
        let mut p = Premises::geometry_only();
        p.declared_jumps = vec![([0, 1, 0], [3, 1, 0])];
        let w = course(6, 2).with_premises(p);
        assert!(w.neighbors([0, 1, 0]).contains(&[3, 1, 0]));
        assert!(
            !w.without_declared_jumps()
                .neighbors([0, 1, 0])
                .contains(&[3, 1, 0])
        );
        assert!(w.walk_region(&[[-6, 1, 0]]).contains(&[6, 1, 0]));
    }
}
