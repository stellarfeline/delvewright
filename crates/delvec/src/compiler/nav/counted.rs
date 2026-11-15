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
            CensusOf::Kind(_) | CensusOf::Npc(_) | CensusOf::Riders(_) => {
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

/// **Where a census places its bodies** (spec-0134 §1): per counted datum,
/// every cell of its box a body of the set can stand in at its footprint, by
/// the nav model's one standability predicate (`World::standable_fp`), nearest
/// the box's centre first (then by cell, ADR-0006). The harness puts each body
/// it places on a cell of its own, so no body is put inside a block or onto
/// another. A wave's cells are taken at the footprint of its widest stack; a
/// vehicle's riders are placed by boarding and take none.
pub fn census_cells(
    plan: &Plan,
    world: &World,
) -> std::collections::BTreeMap<String, Vec<[i32; 3]>> {
    let c = plan.campaign;
    let mut out = std::collections::BTreeMap::new();
    for s in &c.quests.content.state {
        let Some(k) = &s.counts else {
            continue;
        };
        let Some((lo, hi)) = plan.zone_box(&k.region) else {
            continue;
        };
        let entities: Vec<String> = match &k.of {
            CensusOf::Wave(w) => c
                .quests
                .content
                .waves
                .iter()
                .find(|x| x.id == *w)
                .map(|x| x.mobs.iter().map(|m| m.entity.clone()).collect())
                .unwrap_or_default(),
            CensusOf::Actor(a) => c
                .quests
                .content
                .actors
                .iter()
                .find(|x| x.id == *a)
                .map(|x| vec![super::actor_body_entity(x)])
                .unwrap_or_default(),
            CensusOf::Npc(n) => c
                .npcs
                .content
                .npcs
                .iter()
                .find(|x| x.id == *n)
                .map(|x| vec![delvewright_dsl::BodyRef::Npc(x).worn_entity().to_string()])
                .unwrap_or_default(),
            CensusOf::Kind(kind) => vec![kind.clone()],
            CensusOf::Riders(_) => continue,
        };
        let fps: Vec<Footprint> = entities.iter().map(|e| entity_footprint(e)).collect();
        out.insert(s.id.as_str().to_string(), box_stands(world, (lo, hi), &fps));
    }
    out
}

/// The cells of `region` every footprint of `fps` stands in, nearest the
/// box's horizontal centre first, then lowest, then by cell.
pub(in crate::compiler::nav) fn box_stands(
    world: &World,
    region: ([i32; 3], [i32; 3]),
    fps: &[Footprint],
) -> Vec<[i32; 3]> {
    let (lo, hi) = region;
    let centre2 = [lo[0] + hi[0], lo[2] + hi[2]];
    let mut cells: Vec<[i32; 3]> = crate::compiler::assembled::region_cells(lo, hi)
        .filter(|cell| fps.iter().all(|fp| world.standable_fp(*cell, fp)))
        .collect();
    cells.sort_by_key(|cell| {
        let dx = 2 * cell[0] - centre2[0];
        let dz = 2 * cell[2] - centre2[1];
        (dx * dx + dz * dz, cell[1], *cell)
    });
    cells
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

    /// A box that takes in the floor course under its standing course (the
    /// stage's box, two courses tall from the floor) holds only the cells a
    /// body stands in: never the floor's own cells, where a body put there
    /// suffocates, and never a wall's.
    #[test]
    fn a_census_places_on_standable_cells_only_and_never_in_the_floor() {
        let world = pen(false);
        let goat = entity_footprint("minecraft:goat");
        let cells = box_stands(
            &world,
            ([3, 64, 1], [6, 65, 3]),
            std::slice::from_ref(&goat),
        );
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|c| c[1] == 65), "{cells:?}");
        assert!(cells.iter().all(|c| c[0] != 5), "a wall's cell: {cells:?}");
        let distinct: BTreeSet<_> = cells.iter().collect();
        assert_eq!(distinct.len(), cells.len());
        // The old placement, the box's centre on its floor course, is no
        // cell a body stands in.
        assert!(!cells.contains(&[5, 64, 2]));
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
