//! **A census's bodies can reach its box** (spec-0134 §1, `DW1089`).
//!
//! A counted datum rests on a mob's own AI to bring its bodies into the box,
//! and the ruling judges it like combat: the process is skipped and the
//! result judged. What the compiler proves is the geometry the process needs —
//! for a census `of` a wave or an actor, the walk from each body's seat (the
//! wave's spawn anchor, the actor's mark) to some cell of the counted box is
//! connected **for that body's footprint** (`entity_footprint`, the walk
//! `DW0386` proves a lane on and `DW0325` an actor's move on), over the world
//! with no gate a mob opens. A census `of` a kind or an NPC names no seat a
//! mob walks from, and is stated unproven on the binding line.

use std::collections::BTreeSet;

use delvewright_dsl::CensusOf;

use super::{Footprint, SNAP_RADIUS, World, entity_footprint};
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

use super::DW_CENSUS_UNREACHED;

/// What the census proof examined, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CensusBinding {
    /// Counted data declared.
    pub datums: usize,
    /// Bodies' seats proven connected to their box.
    pub proven: usize,
    /// Counted data `of` a kind or an NPC, which name no seat.
    pub unproven: usize,
}

impl CensusBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "census binding (DW1089): {} counted datum(s); {} body seat(s) proven to reach \
             their box at their footprint; {} census(es) of a kind or an NPC name no seat and \
             are not proven",
            self.datums, self.proven, self.unproven
        )
    }
}

/// Whether a body of footprint `fp` seated at `seat` walks into `region`.
pub fn seat_reaches(
    world: &World,
    seat: [i32; 3],
    region: ([i32; 3], [i32; 3]),
    fp: &Footprint,
) -> bool {
    let Some(start) = world.snap_standable_fp(seat, SNAP_RADIUS, fp) else {
        return false;
    };
    let (lo, hi) = region;
    let goals: BTreeSet<[i32; 3]> = crate::compiler::assembled::region_cells(lo, hi)
        .filter(|c| world.standable_fp(*c, fp))
        .collect();
    goals
        .iter()
        .any(|g| *g == start || world.find_path_fp(start, *g, fp).is_some())
}

/// `DW1089` for one body `who` of kind `entity` seated at `seat`, or `None`
/// when it walks into `region`.
pub fn seat_unreached(
    world: &World,
    datum: &str,
    who: &str,
    seat: [i32; 3],
    region: ([i32; 3], [i32; 3]),
    entity: &str,
) -> Option<Failure> {
    let fp = entity_footprint(entity);
    (!seat_reaches(world, seat, region, &fp)).then(|| Failure {
        code: DW_CENSUS_UNREACHED,
        message: format!(
            "`{datum}` counts the bodies in {:?}..{:?}, but {who} seated at {seat:?} cannot walk \
             into that box at its footprint ({entity}) over the world with no gate a mob opens — \
             the count can never rise for it. Open a way from the seat to the box (a gate the mob \
             passes, a gap its body fits), or move the box or the seat",
            region.0, region.1
        ),
    })
}

/// The census proof over the assembled world: every wave's spawn anchor (for
/// each mob kind of the wave) and every actor's mark reaches its box.
pub fn check_censuses(plan: &Plan, world: &World) -> Result<CensusBinding, Failure> {
    let entity_world;
    let world: &World = if world.has_use_gates() {
        entity_world = world.without_gate_use();
        &entity_world
    } else {
        world
    };
    let c = plan.campaign;
    let mut b = CensusBinding::default();
    for s in &c.quests.content.state {
        let Some(k) = &s.counts else {
            continue;
        };
        b.datums += 1;
        let Some(region) = plan.zone_box(&k.region) else {
            continue;
        };
        let seats: Vec<(String, [i32; 3], String)> = match &k.of {
            CensusOf::Wave(w) => {
                let Some(wave) = c.quests.content.waves.iter().find(|x| x.id == *w) else {
                    continue;
                };
                let Some(area) = crate::compiler::plan::wave_area(c, w.as_str()) else {
                    continue;
                };
                let Some(seat) = plan.point(area, wave.anchor.as_str()) else {
                    continue;
                };
                wave.mobs
                    .iter()
                    .map(|m| {
                        (
                            format!("a `{}` of `{}`", m.entity, w.as_str()),
                            seat,
                            m.entity.clone(),
                        )
                    })
                    .collect()
            }
            CensusOf::Actor(a) => {
                let Some(actor) = c.quests.content.actors.iter().find(|x| x.id == *a) else {
                    continue;
                };
                let Some(seat) = plan.body_point(delvewright_dsl::BodyRef::Actor(actor)) else {
                    continue;
                };
                vec![(
                    format!("`{}`", a.as_str()),
                    seat,
                    super::actor_body_entity(actor),
                )]
            }
            CensusOf::Kind(_) | CensusOf::Npc(_) => {
                b.unproven += 1;
                continue;
            }
        };
        for (who, seat, entity) in seats {
            if let Some(f) = seat_unreached(world, s.id.as_str(), &who, seat, region, &entity) {
                return Err(f);
            }
            b.proven += 1;
        }
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::floored;

    /// A goat's pen across a fence (a two-high wall at x = 5) from its seat;
    /// with `gate` the fence has a one-cell gap.
    fn pen(gate: bool) -> World {
        let mut walls: Vec<[i32; 3]> = Vec::new();
        for z in 0..5 {
            if gate && z == 2 {
                continue;
            }
            walls.push([5, 65, z]);
            walls.push([5, 66, z]);
        }
        floored(10, 5, 65, &walls)
    }

    #[test]
    fn a_goat_across_a_fence_from_its_pen_does_not_reach_it_and_a_gate_lets_it() {
        let goat = entity_footprint("minecraft:goat");
        let pen_box = ([7, 65, 1], [9, 65, 3]);
        assert!(!seat_reaches(&pen(false), [1, 65, 2], pen_box, &goat));
        assert!(seat_reaches(&pen(true), [1, 65, 2], pen_box, &goat));
        let f = seat_unreached(
            &pen(false),
            "state/goats-home",
            "a goat",
            [1, 65, 2],
            pen_box,
            "minecraft:goat",
        )
        .expect("the fenced goat is refused");
        assert_eq!(f.code, "DW1089");
        assert!(
            seat_unreached(
                &pen(true),
                "state/goats-home",
                "a goat",
                [1, 65, 2],
                pen_box,
                "minecraft:goat"
            )
            .is_none()
        );
    }

    #[test]
    fn the_binding_line_states_every_count() {
        let b = CensusBinding {
            datums: 2,
            proven: 3,
            unproven: 1,
        };
        assert!(
            b.line()
                .starts_with("census binding (DW1089): 2 counted datum(s); 3 body")
        );
    }
}
