//! Jumps the route relation takes (spec-0113 §4.4, §5.2, §6).
//!
//! A `world.jumps[]` entry is proven makeable by flying its window, or refused
//! (`DW1041`); a proven one is a move the route relation takes. And a **hop**:
//! where a leg's walk does not reach its goal, the route also takes the jumps
//! across a gap its own frontier meets that the same window proves (the
//! one-cell gap between two level landings a player clears without
//! thinking). One question — can a body on cue make this jump — with one
//! answer, [`makeable`], asked of a declared jump and of a hop alike; the
//! reach graph flies the same simulator over every launch, so the route never
//! credits a jump the reach graph does not.

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

/// **Whether a body on cue makes the jump from `from` to `to`**: the window
/// of spec-0113 §4.4 flown under every rule set of [`PROVEN_UNDER`] from the
/// straight run-up `run_up` the course gives. `None` when every launch lands;
/// the first miss otherwise. The one judgement a declared jump (`DW1041`) and
/// a route's hop ([`World::hops_fp`]) both answer to.
pub(in crate::compiler::nav) fn makeable(
    world: &World,
    from: [i32; 3],
    to: [i32; 3],
    sprint: bool,
    run_up: u32,
) -> Option<crate::compiler::nav::world::reach::JumpMiss> {
    PROVEN_UNDER
        .iter()
        .find_map(|r| world.declared_jump(from, to, sprint, run_up, *r).err())
}

/// The farthest along its axis a hop's landing is looked for, in cells from
/// the launch: four columns of air, the most a sprint jump clears on the flat
/// (`body-movement.md` §5). Whether a body makes it is [`makeable`]'s.
const HOP_REACH: i32 = 5;

impl World {
    /// **The hops out of `c`**: for each cardinal direction the walk does not
    /// leave `c` by, the nearest standable cell along that axis — up to
    /// [`HOP_REACH`] cells off, its feet within a cell of `c`'s, over columns
    /// clear from the lower feet to a head above the higher — that a body on
    /// cue reaches by a jump from the run-up the course gives ([`hop_sprint`]).
    /// In the fixed direction order.
    ///
    /// [`hop_sprint`]: World::hop_sprint
    pub(in crate::compiler::nav) fn hops_fp(&self, c: [i32; 3], fp: &Footprint) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        if !self.standable_fp(c, fp) {
            return Vec::new();
        }
        let walk = self.neighbors_walk_fp(c, fp);
        let mut out = Vec::new();
        for (dx, dz) in HORIZ {
            if walk.iter().any(|n| n[0] - c[0] == dx && n[2] - c[2] == dz) {
                continue;
            }
            'along: for k in 2..=HOP_REACH {
                for dy in [0, 1, -1] {
                    let to = [c[0] + dx * k, c[1] + dy, c[2] + dz * k];
                    if !self.standable_fp(to, fp) {
                        continue;
                    }
                    let (lo, hi) = (c[1].min(to[1]), c[1].max(to[1]) + fp.height);
                    let clear = (1..k).all(|j| {
                        (lo..=hi).all(|y| !self.is_occupied([c[0] + dx * j, y, c[2] + dz * j]))
                    });
                    if clear && self.hop_sprint(c, to).is_some() {
                        out.push(to);
                    }
                    break 'along;
                }
                // A column that is not clear ends the look along this axis.
                let col = [c[0] + dx * k, c[1], c[2] + dz * k];
                if self.is_occupied(col) || self.is_occupied([col[0], col[1] + 1, col[2]]) {
                    break;
                }
            }
        }
        out
    }

    /// **How a hop from `from` to `to` is made**, when it is: `Some(false)`
    /// when a walking jump makes it, else `Some(true)` when a sprinted one does
    /// ([`makeable`], from the run-up the course gives), `None` when neither.
    /// The walk is asked first: a sprint carries a body past a landing one
    /// tread long (the next step of a stair, measured), where a walking jump
    /// stops on it.
    pub(in crate::compiler::nav) fn hop_sprint(
        &self,
        from: [i32; 3],
        to: [i32; 3],
    ) -> Option<bool> {
        let run_up = self.run_up(from, to);
        [false, true]
            .into_iter()
            .find(|sprint| makeable(self, from, to, *sprint, run_up).is_none())
    }
}

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
        // The walk relation, not the reach graph: the question is whether the
        // route relation already has this edge (spec-0113 §5.2), which only a
        // declaration adds. A gap the launch set clears is still one the walk
        // does not, and declaring it is what lets a route take it.
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
        let miss = makeable(world, j.from, j.to, j.sprint, run_up);
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

    /// The festival town's lighthouse stair: a tread, a one-cell gap, the next
    /// tread level with it, and no run-up behind the launch (a wall). The walk
    /// does not cross; the route does, by a hop the window proves from a
    /// standing start, and the reach graph flies the same crossing.
    #[test]
    fn a_one_cell_gap_between_level_landings_is_a_hop_the_route_takes() {
        let mut solid = BTreeSet::new();
        for z in -2..=2 {
            solid.insert([-1, 1, z]);
            solid.insert([-1, 2, z]);
            solid.insert([-1, 3, z]);
            solid.insert([0, 0, z]);
            for x in 2..6 {
                solid.insert([x, 0, z]);
            }
        }
        let w = World::from_solid_cells(solid);
        let (from, to) = ([0, 1, 0], [5, 1, 0]);
        assert!(
            w.find_path(from, to).is_none(),
            "the walk alone does not cross the gap"
        );
        assert_eq!(w.run_up([0, 1, 0], [2, 1, 0]), 0);
        let route = w.find_route(from, to).expect("the route hops the gap");
        assert!(
            route
                .windows(2)
                .any(|p| p[0] == [0, 1, 0] && p[1] == [2, 1, 0]),
            "the hop is from the tread's edge to the next tread: {route:?}"
        );
        assert!(
            w.body_reach(&[from]).cells().contains(&[2, 1, 0]),
            "the reach graph crosses what the route hops"
        );
    }

    /// The direction that could let something ship: a gap no window makes is
    /// no hop. Five columns of air from a full run-up, which `DW1041` refuses
    /// as a declaration, is not taken by the route either.
    #[test]
    fn a_gap_the_window_does_not_make_is_no_hop() {
        let w = course(6, 5);
        assert!(w.hops_fp([0, 1, 0], &Footprint::player()).is_empty());
        assert!(w.find_route([-6, 1, 0], [8, 1, 0]).is_none());
        // And the one judgement: the declaration of the same jump is refused.
        let (_, _, verdict) = judge_jumps(&[jump([0, 1, 0], [6, 1, 0])], &w);
        assert!(verdict.is_err());
    }

    /// The festival lighthouse's stair as the town builds it: a lane one cell
    /// wide round a 2x2 newel, a tread a course, three treads missing, the
    /// tread after each gap level with the one before it — and the tread
    /// after that a course up, one cell past the landing. A sprinted jump from
    /// the corner overshoots the landing onto that next tread (measured: the
    /// feet end at x 15.7, a course up); the walking jump stops on it. The
    /// route climbs the whole stair, hopping each gap walked.
    #[test]
    fn the_lighthouse_stair_is_climbed_hopping_each_gap_walked() {
        let ring: [(i32, i32); 12] = [
            (12, 12),
            (13, 12),
            (14, 12),
            (15, 12),
            (15, 13),
            (15, 14),
            (15, 15),
            (14, 15),
            (13, 15),
            (12, 15),
            (12, 14),
            (12, 13),
        ];
        let broken = [10, 19, 28];
        let mut treads = Vec::new();
        let (mut y, mut after, mut k) = (0, false, 0usize);
        while y < 32 {
            if broken.contains(&k) {
                after = true;
                k += 1;
                continue;
            }
            if !after {
                y += 1;
            }
            after = false;
            let (x, z) = ring[(3 + k) % 12];
            treads.push([x, y, z]);
            k += 1;
        }
        let mut solid: BTreeSet<[i32; 3]> = treads.iter().copied().collect();
        for x in 10..=17 {
            for z in 10..=17 {
                for yy in 0..=40 {
                    if yy == 0 || !ring.contains(&(x, z)) {
                        solid.insert([x, yy, z]);
                    }
                }
            }
        }
        let w = World::from_solid_cells(solid);
        let (launch, landing) = ([12, 11, 12], [14, 11, 12]);
        assert!(
            w.find_path(launch, landing).is_none(),
            "the gap is not walked"
        );
        assert!(
            makeable(&w, launch, landing, true, 0).is_some(),
            "a sprint overshoots the landing"
        );
        assert_eq!(w.hop_sprint(launch, landing), Some(false));
        let feet = |t: [i32; 3]| [t[0], t[1] + 1, t[2]];
        let route = w
            .find_route(feet(treads[0]), feet(*treads.last().unwrap()))
            .expect("the route climbs the broken stair");
        let hops = route
            .windows(2)
            .filter(|p| (p[1][0] - p[0][0]).abs() + (p[1][2] - p[0][2]).abs() > 1)
            .count();
        assert_eq!(hops, 3, "one hop a gap: {route:?}");
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
