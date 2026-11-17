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

/// How far round a census's box the walk between two of its cells may stray
/// when [`seat_cells`] asks which other cells the first reached one joins: a
/// cell joined only by a longer way round is left out, never let in.
const CELL_JOIN_MARGIN: i32 = 2;

/// The first cell of `goals` (in order) a body of footprint `fp` standing at
/// `start` walks to, by the one router.
fn first_reached(
    world: &World,
    start: [i32; 3],
    goals: &[[i32; 3]],
    fp: &Footprint,
) -> Option<usize> {
    goals
        .iter()
        .position(|g| *g == start || world.find_path_fp(start, *g, fp).is_some())
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
    first_reached(
        world,
        start,
        &box_stands(world, region, std::slice::from_ref(fp)),
        fp,
    )
    .is_some()
}

/// **The cells of `region` a body seated at `seat` walks into**, in
/// [`box_stands`] order: the first standable cell its walk reaches, and every
/// other standable cell of the box that one joins by a walk kept within
/// [`CELL_JOIN_MARGIN`] of the box. Empty exactly when [`seat_reaches`] is
/// false. A cell that stands but which the body's walk never meets — a pocket
/// sealed off inside the box, a cake's top course — is no place to put it.
pub fn seat_cells(
    world: &World,
    seat: [i32; 3],
    region: ([i32; 3], [i32; 3]),
    fp: &Footprint,
) -> Vec<[i32; 3]> {
    let Some(start) = world.snap_standable_fp(seat, SNAP_RADIUS, fp) else {
        return Vec::new();
    };
    let goals = box_stands(world, region, std::slice::from_ref(fp));
    let Some(first) = first_reached(world, start, &goals, fp) else {
        return Vec::new();
    };
    let reached = goals[first];
    let (lo, hi) = region;
    let m = CELL_JOIN_MARGIN;
    let outside = move |c: [i32; 3]| (0..3).any(|i| c[i] < lo[i] - m || c[i] > hi[i] + m);
    // The cells before `first` were each asked of the seat and refused; one
    // the reached cell joined would contradict that, so they stay out.
    goals[first..]
        .iter()
        .filter(|g| {
            **g == reached
                || world
                    .find_path_avoiding(reached, **g, fp, &outside)
                    .is_some()
        })
        .copied()
        .collect()
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
    (!seat_reaches(world, seat, region, &fp)).then(|| unreached(datum, who, seat, region, entity))
}

/// The `DW1089` refusal itself.
fn unreached(
    datum: &str,
    who: &str,
    seat: [i32; 3],
    region: ([i32; 3], [i32; 3]),
    entity: &str,
) -> Failure {
    Failure {
        code: DW_CENSUS_UNREACHED,
        message: format!(
            "`{datum}` counts the bodies in {:?}..{:?}, but {who} seated at {seat:?} cannot walk \
             into that box at its footprint ({entity}) over the world with no gate a mob opens — \
             the count can never rise for it. Open a way from the seat to the box (a gate the mob \
             passes, a gap its body fits), or move the box or the seat",
            region.0, region.1
        ),
    }
}

/// What the census proof established: its binding, and per counted datum
/// the cells its bodies are placed on.
#[derive(Clone, Debug, Default)]
pub struct CensusProof {
    /// The binding line's counts.
    pub binding: CensusBinding,
    /// **Where a census places its bodies** (spec-0134 §1), per counted datum,
    /// in placement order: for a census of a wave or an actor, the cells of
    /// its box that body's own walk from its seat reaches ([`seat_cells`],
    /// for every mob kind of a wave) — so no body is put inside a block, onto
    /// a floor's course, or into a pocket of the box it could never walk to;
    /// for a census of a kind or an NPC, which names no seat, every cell of
    /// the box a body of the set stands in ([`box_stands`]). A vehicle's
    /// riders are placed by boarding and take none.
    pub cells: std::collections::BTreeMap<String, Vec<[i32; 3]>>,
}

/// The census proof over the assembled world: every wave's spawn anchor (for
/// each mob kind of the wave) and every actor's mark reaches its box
/// (`DW1089`), and the cells each census places its bodies on, from the same
/// walk over the same world with no gate a mob opens.
pub fn check_censuses(plan: &Plan, world: &World) -> Result<CensusProof, Failure> {
    let entity_world;
    let world: &World = if world.has_use_gates() {
        entity_world = world.without_gate_use();
        &entity_world
    } else {
        world
    };
    let c = plan.campaign;
    let mut proof = CensusProof::default();
    let b = &mut proof.binding;
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
            CensusOf::Npc(n) => {
                b.unproven += 1;
                let worn = c
                    .npcs
                    .content
                    .npcs
                    .iter()
                    .find(|x| x.id == *n)
                    .map(|x| delvewright_dsl::BodyRef::Npc(x).worn_entity().to_string());
                let fps: Vec<Footprint> = worn.iter().map(|e| entity_footprint(e)).collect();
                proof
                    .cells
                    .insert(s.id.as_str().to_string(), box_stands(world, region, &fps));
                continue;
            }
            CensusOf::Kind(kind) => {
                b.unproven += 1;
                proof.cells.insert(
                    s.id.as_str().to_string(),
                    box_stands(world, region, &[entity_footprint(kind)]),
                );
                continue;
            }
            CensusOf::Riders(_) => {
                b.unproven += 1;
                continue;
            }
        };
        // Every body of the set walks to the cells it is put on: the box's
        // cells each seat-and-kind's walk reaches, kept where all agree.
        let mut cells: Option<Vec<[i32; 3]>> = None;
        let mut asked: std::collections::BTreeMap<(String, [i32; 3]), Vec<[i32; 3]>> =
            std::collections::BTreeMap::new();
        for (who, seat, entity) in seats {
            let reached = asked
                .entry((entity.clone(), seat))
                .or_insert_with(|| seat_cells(world, seat, region, &entity_footprint(&entity)))
                .clone();
            if reached.is_empty() {
                return Err(unreached(s.id.as_str(), &who, seat, region, &entity));
            }
            b.proven += 1;
            cells = Some(match cells {
                None => reached,
                Some(prev) => prev.into_iter().filter(|c| reached.contains(c)).collect(),
            });
        }
        proof
            .cells
            .insert(s.id.as_str().to_string(), cells.unwrap_or_default());
    }
    Ok(proof)
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
    use std::collections::BTreeSet;

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

    /// A pen whose centre cell is sealed off by a two-high ring of wall: it
    /// stands (floor under it, headroom over it) and no walk meets it.
    fn sealed_centre() -> World {
        let mut walls: Vec<[i32; 3]> = Vec::new();
        for y in [65, 66] {
            for z in 2..=4 {
                walls.push([5, y, z]);
                walls.push([7, y, z]);
            }
            walls.push([6, y, 2]);
            walls.push([6, y, 4]);
        }
        floored(12, 7, 65, &walls)
    }

    /// **A census places a body only on a cell its walk reaches.** The sealed
    /// centre is the box's first standable cell, where the placement used to
    /// start; the body's own walk from its seat never meets it, so it is no
    /// cell of the census. Every other cell the walk reaches is kept, in the
    /// same order, and the answer agrees with the reach proof's.
    #[test]
    fn a_census_places_on_no_cell_its_walk_never_meets() {
        let world = sealed_centre();
        let goat = entity_footprint("minecraft:goat");
        let pen_box = ([4, 65, 1], [8, 65, 5]);
        let stands = box_stands(&world, pen_box, std::slice::from_ref(&goat));
        assert_eq!(
            stands[0],
            [6, 65, 3],
            "the sealed centre stands, and comes first"
        );
        let cells = seat_cells(&world, [1, 65, 3], pen_box, &goat);
        assert!(!cells.contains(&[6, 65, 3]), "{cells:?}");
        let expected: Vec<[i32; 3]> = stands
            .iter()
            .copied()
            .filter(|c| *c != [6, 65, 3])
            .collect();
        assert_eq!(cells, expected);
        assert!(seat_reaches(&world, [1, 65, 3], pen_box, &goat));
        // A seat inside the sealed cell reaches only it.
        assert_eq!(
            seat_cells(&world, [6, 65, 3], pen_box, &goat),
            vec![[6, 65, 3]]
        );
        // No walk in, no cell, and the proof refuses alike.
        let fenced = pen(false);
        let pen_box = ([7, 65, 1], [9, 65, 3]);
        assert!(seat_cells(&fenced, [1, 65, 2], pen_box, &goat).is_empty());
        assert!(!seat_reaches(&fenced, [1, 65, 2], pen_box, &goat));
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
