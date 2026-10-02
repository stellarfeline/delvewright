//! A fight the party must win stays where the party can strike it (`DW0924`).
//!
//! ## The shape it guards against
//!
//! A wave a `kill` objective adjudicates, seated on a ledge with an open edge.
//! A blow knocks a body off the edge exactly as it walks off one, and a body
//! that survives the fall where the party cannot follow leaves the objective
//! waiting forever. Every other rung stays green: the wave has footing, the
//! ledge is reachable, the fight is winnable.
//!
//! `vesperhold`'s Cliff Watchmen, before its shelf carried a parapet, are an
//! instance this rule does not refuse, and rightly: the two that survived a
//! knock-off landed on the valley floor sixteen blocks down, which the party
//! reaches by the postern, the causeway and its stair. That route was walked
//! live from the shelf to both landing cells, and both bodies were struck
//! there. The ladder that reported them unreachable had its bot stalled on the
//! stair's balustrade, on the way down.
//!
//! ## The rule
//!
//! For every stack of a wave some `kill` objective names, flood the movement a
//! mob has ([`World::mob_moves`] — walking, a step or jump up one block, a perch
//! on a barrier top, a drop off any edge to the first floor it survives, and
//! floating in water) from the seats the seating pass chose, inside the stack's
//! follow range — the radius `DW0478` and `DW0922` read. A drop off an edge is
//! where a body knocked off one lands, so the flood already holds every ledge a
//! blow can send it to. A reached cell is **in reach** when some cell the party
//! can be in holds an eye within a strike of the body's box there
//! ([`STRIKE_REACH`] from [`PLAYER_EYE_HEIGHT`]); the party's cells are where it
//! can **walk** ([`World::reachable_walkable`], the walk graph every route is
//! proven on) from everywhere the campaign puts it, less every cell outside the
//! playable region, from which the boundary carries a body back. A drop the
//! party could survive is not a way to the fight: sixteen blocks into a valley
//! costs a body most of its health, and no route takes it. A member is
//! **stranded** when it can reach a cell from which no mob movement inside the
//! range leads back to a cell in reach: it can get there, and the party can never
//! get at it.
//!
//! Only the hand is counted. A bow reaches further, and whether a class carries
//! one, and whether a body in a valley can be shot from a shelf, are questions
//! about the fight, which this rule does not ask; a body only an arrow can find
//! is refused.
//!
//! ## Prescription
//!
//! Close the edge a body goes off (a parapet is a rise, and a body knocked into
//! one stays on the shelf), give the party a way down to where it lands, or seat
//! the wave where nothing it can fall to is out of the party's reach. Never mark
//! the shelf unwalkable, and never shrink `follow_range` to hide the placement.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::metrics::PLAYER_EYE_HEIGHT;
use delvewright_dsl::{DwCode, ExitTier, Objective};

use crate::compiler::failure::Failure;
use crate::compiler::nav::{DEFAULT_FOLLOW_RANGE, Footprint, World, entity_footprint};
use crate::compiler::plan::Plan;

/// `DW0924`: a body a `kill` objective waits on can get to a place it survives
/// and the party cannot strike it from.
pub const DW_FIGHT_OUT_OF_REACH: DwCode = DwCode::new("DW0924", ExitTier::Build);

/// How far a player strikes, in blocks, from the eye to the target's box: the
/// default of vanilla's `minecraft:entity_interaction_range` attribute for a
/// player in survival or adventure, 3.0 (Minecraft Wiki, *Attribute*, pinned
/// 1.21.11). No delve changes it.
pub const STRIKE_REACH: f64 = 3.0;

/// How many stranded stacks a report names before counting the rest.
const NAME_LIMIT: usize = 6;

/// One stack that can be stranded.
#[derive(Clone, Debug)]
pub struct Stranding {
    /// The wave.
    pub wave: String,
    /// The `kill` objective that waits on it.
    pub objective: String,
    /// The member's entity id.
    pub entity: String,
    /// Its follow range.
    pub radius: f64,
    /// The seat it set off from, then every cell of its way to the first
    /// stranded cell.
    pub path: Vec<[i32; 3]>,
    /// How many cells of its reach are stranded.
    pub cells: usize,
}

/// What `DW0924` examined, whether or not it refused.
#[derive(Clone, Debug, Default)]
pub struct StrandBinding {
    /// Waves a `kill` objective names and the seating pass seated.
    pub waves: usize,
    /// Their stacks with a seat.
    pub stacks: usize,
    /// Seats flooded from.
    pub seats: usize,
    /// Cells a member can reach, summed over stacks.
    pub reached: usize,
    /// Of those, cells within a strike of a cell the party can be in.
    pub in_reach: usize,
    /// Cells the party can walk to while each fight is next, summed over the
    /// fights.
    pub party: usize,
    /// One row per stranded stack.
    pub stranded: Vec<Stranding>,
}

impl StrandBinding {
    /// The one line a build prints about this proof.
    pub fn line(&self) -> String {
        format!(
            "DW0924 binding: {} kill-objective wave(s), {} stack(s) over {} seat(s); {} cell(s) a \
             member can reach within its follow range, {} of them within a strike of a cell the \
             party can walk to while the fight is next ({} such cell(s) over the fights); {} stack(s) can be stranded out of reach",
            self.waves,
            self.stacks,
            self.seats,
            self.reached,
            self.in_reach,
            self.party,
            self.stranded.len(),
        )
    }

    /// The same counts as `validation/strand.json`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_FIGHT_OUT_OF_REACH.id(),
            "waves": self.waves,
            "stacks": self.stacks,
            "seats": self.seats,
            "reached": self.reached,
            "in_reach": self.in_reach,
            "party": self.party,
            "examined": self.stacks,
            "stranded": self.stranded.iter().map(|s| serde_json::json!({
                "wave": s.wave, "objective": s.objective, "entity": s.entity,
                "path": s.path, "cells": s.cells,
            })).collect::<Vec<_>>(),
        })
    }

    /// The verdict.
    pub fn verdict(&self) -> Result<(), Failure> {
        let Some(first) = self.stranded.first() else {
            return Ok(());
        };
        let seat = first.path.first().copied().unwrap_or([0, 0, 0]);
        let at = first.path.last().copied().unwrap_or(seat);
        let way = first
            .path
            .windows(2)
            .filter(|w| w[1][1] < w[0][1] - 1)
            .map(|w| format!("a fall of {} from {:?}", w[0][1] - w[1][1], w[0]))
            .collect::<Vec<_>>();
        let way = if way.is_empty() {
            String::new()
        } else {
            format!(", by {}", way.join(" and "))
        };
        let more: Vec<String> = self
            .stranded
            .iter()
            .skip(1)
            .take(NAME_LIMIT)
            .map(|s| format!("wave `{}`'s `{}`", s.wave, s.entity))
            .collect();
        let tail = match self.stranded.len().saturating_sub(1) {
            0 => String::new(),
            n if n <= NAME_LIMIT => format!("; the same holds for {}", more.join(", ")),
            n => format!(
                "; the same holds for {}, and {} more",
                more.join(", "),
                n - NAME_LIMIT
            ),
        };
        Err(Failure {
            code: DW_FIGHT_OUT_OF_REACH,
            message: format!(
                "wave `{}`'s `{}`, which `kill` objective `{}` waits on, can get from its seat at \
                 {seat:?} to {at:?}{way}, inside its follow range of {} block(s) — and from there \
                 ({} cell(s) like it) no movement a mob has leads back to anywhere a party member's \
                 eye is within a strike ({STRIKE_REACH} blocks) of it{tail}. A blow knocks a body \
                 off an open edge exactly as it walks off one, and a body that survives the fall \
                 where nobody can follow leaves the objective waiting forever. The party's reach \
                 counts walking, falling, jumping and swimming, and no cell outside the playable \
                 region; only the hand is counted, not a bow. Close the edge (a parapet keeps a \
                 body on the shelf), give the party a way to where it lands, or seat the wave \
                 where nothing it can fall to is out of reach. Never mark the shelf unwalkable, and \
                 never shrink `follow_range` to hide the placement.",
                first.wave, first.entity, first.objective, first.radius, first.cells,
            ),
        })
    }
}

/// Whether a party member standing in `p` strikes a body of `w` x `h` whose
/// feet cell is `m`. The body's box is taken from the cell floor to a block over
/// its height, so a perched or partially floored body is never out of reach by a
/// half block the model does not carry.
fn strikes(world: &World, p: [i32; 3], m: [i32; 3], w: f64, h: f64) -> bool {
    let half = w / 2.0;
    eye_reaches_box(
        world,
        p,
        [
            f64::from(m[0]) + 0.5 - half,
            f64::from(m[1]),
            f64::from(m[2]) + 0.5 - half,
        ],
        [
            f64::from(m[0]) + 0.5 + half,
            f64::from(m[1]) + 1.0 + h,
            f64::from(m[2]) + 0.5 + half,
        ],
    )
}

/// **The one strike-reach measure**: whether a player standing in cell `p`
/// holds an eye within [`STRIKE_REACH`] of the box `lo..hi` — the eye at the
/// cell's centre, [`PLAYER_EYE_HEIGHT`] over the floor the model gives the
/// cell, the distance to the nearest point of the box. Read by `DW0924` here
/// and by `DW0937` (`compiler::assembly`), so the two can never disagree about
/// what a blow reaches.
pub fn eye_reaches_box(world: &World, p: [i32; 3], lo: [f64; 3], hi: [f64; 3]) -> bool {
    let eye = [
        f64::from(p[0]) + 0.5,
        world.feet_y(p) + PLAYER_EYE_HEIGHT,
        f64::from(p[2]) + 0.5,
    ];
    let gap = |e: f64, lo: f64, hi: f64| (lo - e).max(e - hi).max(0.0);
    let dx = gap(eye[0], lo[0], hi[0]);
    let dy = gap(eye[1], lo[1], hi[1]);
    let dz = gap(eye[2], lo[2], hi[2]);
    (dx * dx + dy * dy + dz * dz).sqrt() <= STRIKE_REACH
}

/// `DW0924` over every seated wave a `kill` objective names.
///
/// `party_roots` are the cells the campaign puts the party (the population
/// `DW0891` is rooted at); `returned` is the playable region, outside which the
/// boundary carries a body back.
pub fn check(
    plan: &Plan,
    world: &World,
    wave_seats: &BTreeMap<String, Vec<[i32; 3]>>,
    party_roots: &[[i32; 3]],
    returned: Option<([i32; 3], [i32; 3])>,
) -> StrandBinding {
    let mut b = StrandBinding::default();
    let c = plan.campaign;
    let killed: Vec<(String, String)> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter_map(|o| match o {
            Objective::Kill { wave, .. } => {
                Some((wave.as_str().to_string(), o.id().as_str().to_string()))
            }
            _ => None,
        })
        .collect();
    if killed.is_empty() {
        return b;
    }
    for w in &c.quests.content.waves {
        let Some(objective) = killed
            .iter()
            .find(|(wave, _)| wave == w.id.as_str())
            .map(|(_, o)| o.clone())
        else {
            continue;
        };
        let Some(cells) = wave_seats.get(w.id.as_str()) else {
            continue;
        };
        b.waves += 1;
        // The world while this fight is the beat the story waits on: a gate a
        // later beat opens is shut until the fight is won.
        let step = plan.critical_path.iter().position(
            |s| matches!(s, crate::compiler::plan::Step::Kill { wave_id, .. } if wave_id == w.id.as_str()),
        );
        let config = step.and_then(|s| crate::compiler::nav::world_while_next(plan, world, s));
        let ground = config.as_ref().unwrap_or(world).without_furniture();
        // The party: where it can walk from where the campaign puts it — a
        // route, not a leap off a cliff it might survive.
        let mut party: BTreeSet<[i32; 3]> = ground.reachable_walkable(party_roots);
        party.retain(|p| !crate::compiler::nav::returned_from(returned, *p));
        b.party += party.len();
        let mob_owned;
        let mobs: &World = if ground.has_use_gates() {
            mob_owned = ground.without_gate_use();
            &mob_owned
        } else {
            &ground
        };
        let mut seat = 0usize;
        for mob in &w.mobs {
            let roots: Vec<[i32; 3]> = (0..mob.count as usize)
                .filter_map(|i| cells.get(seat + i).copied())
                .collect();
            seat += mob.count as usize;
            if roots.is_empty() {
                continue;
            }
            b.stacks += 1;
            b.seats += roots.len();
            let radius = match &w.lane {
                Some(l) => f64::from(l.aggro_radius),
                None => mob
                    .attributes
                    .and_then(|a| a.follow_range)
                    .unwrap_or(f64::from(DEFAULT_FOLLOW_RANGE)),
            };
            let fp = entity_footprint(&mob.entity);
            if let Some(s) = strand(mobs, &party, &roots, &fp, radius, &mut b) {
                b.stranded.push(Stranding {
                    wave: w.id.as_str().to_string(),
                    objective: objective.clone(),
                    entity: mob.entity.clone(),
                    radius,
                    path: s.0,
                    cells: s.1,
                });
            }
        }
    }
    b
}

/// One stack's flood: the way to the first stranded cell and how many there
/// are, or `None` when every reached cell leads back into reach.
fn strand(
    world: &World,
    party: &BTreeSet<[i32; 3]>,
    roots: &[[i32; 3]],
    fp: &Footprint,
    radius: f64,
    b: &mut StrandBinding,
) -> Option<(Vec<[i32; 3]>, usize)> {
    let body = fp.body();
    let within = |c: [i32; 3]| {
        roots
            .iter()
            .any(|s| (0..3).map(|i| f64::from(c[i] - s[i]).powi(2)).sum::<f64>() <= radius * radius)
    };
    let mut seen: BTreeSet<[i32; 3]> = roots.iter().copied().collect();
    let mut order: Vec<[i32; 3]> = roots.to_vec();
    let mut pred: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
    let mut back: BTreeMap<[i32; 3], Vec<[i32; 3]>> = BTreeMap::new();
    let mut queue: VecDeque<[i32; 3]> = roots.iter().copied().collect();
    while let Some(cur) = queue.pop_front() {
        // Pursuit bounds where a body walks; gravity bounds where it lands. A
        // drop from a cell inside the range lands wherever it lands, which is
        // where a blow sends a body off an edge.
        let walks = within(cur);
        for n in world.mob_moves(cur, fp) {
            if !(within(n) || (walks && n[1] < cur[1] - 1)) {
                continue;
            }
            back.entry(n).or_default().push(cur);
            if seen.insert(n) {
                pred.insert(n, cur);
                order.push(n);
                queue.push_back(n);
            }
        }
    }
    let in_reach: BTreeSet<[i32; 3]> = seen
        .iter()
        .copied()
        .filter(|m| {
            (-4..=4).any(|dx| {
                (-4..=4).any(|dz| {
                    (-5..=4).any(|dy| {
                        let p = [m[0] + dx, m[1] + dy, m[2] + dz];
                        party.contains(&p) && strikes(world, p, *m, body.width, body.height)
                    })
                })
            })
        })
        .collect();
    b.reached += seen.len();
    b.in_reach += in_reach.len();
    // Every cell that leads back into reach, walked backwards from it.
    let mut ok: BTreeSet<[i32; 3]> = in_reach.clone();
    let mut queue: VecDeque<[i32; 3]> = in_reach.iter().copied().collect();
    while let Some(cur) = queue.pop_front() {
        for p in back.get(&cur).into_iter().flatten() {
            if ok.insert(*p) {
                queue.push_back(*p);
            }
        }
    }
    let stranded: Vec<[i32; 3]> = order.iter().copied().filter(|c| !ok.contains(c)).collect();
    let first = *stranded.first()?;
    let mut path = vec![first];
    while let Some(p) = pred.get(path.last().unwrap_or(&first)) {
        path.push(*p);
    }
    path.reverse();
    Some((path, stranded.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shelf eight courses over a floor, its open edge a drop the body
    /// survives; the party stands on the shelf only.
    fn shelf() -> World {
        let mut solid = BTreeSet::new();
        for x in 0..12 {
            for z in 0..3 {
                solid.insert([x, 0, z]);
                if x < 4 {
                    for y in 1..=8 {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        World::from_solid_cells(solid)
    }

    fn party_on_shelf(w: &World) -> BTreeSet<[i32; 3]> {
        // Only the shelf: the boundary returns a body that leaves it.
        w.reachable_walkable(&[[1, 9, 1]])
            .into_iter()
            .filter(|c| c[0] < 4)
            .collect()
    }

    /// A body that walks off the shelf's edge lands eight courses under the
    /// party and, a few steps out, beyond any strike, with no way back up:
    /// stranded.
    #[test]
    fn a_body_that_falls_off_the_shelf_out_of_reach_is_stranded() {
        let w = shelf();
        let party = party_on_shelf(&w);
        let mut b = StrandBinding::default();
        let fp = entity_footprint("minecraft:zombie");
        let s = strand(&w, &party, &[[1, 9, 1]], &fp, 16.0, &mut b).expect("stranded");
        assert!(s.1 > 0);
        assert_eq!(s.0.first(), Some(&[1, 9, 1]));
        assert!(b.in_reach > 0 && b.reached > b.in_reach, "{b:?}");
        let v = StrandBinding {
            stranded: vec![Stranding {
                wave: "wave/watch".into(),
                objective: "obj/watch".into(),
                entity: "minecraft:zombie".into(),
                radius: 16.0,
                path: s.0,
                cells: s.1,
            }],
            ..b
        };
        assert_eq!(v.verdict().unwrap_err().code.id(), "DW0924");
    }

    /// The same shelf with the party free to follow it down: nothing is
    /// stranded.
    #[test]
    fn a_body_the_party_can_follow_is_not_stranded() {
        let w = shelf();
        let party = w.reachable_walkable(&[[1, 9, 1], [10, 1, 1]]);
        let mut b = StrandBinding::default();
        let fp = entity_footprint("minecraft:zombie");
        assert!(strand(&w, &party, &[[1, 9, 1]], &fp, 16.0, &mut b).is_none());
    }

    /// A strike is measured from the eye to the box: three blocks out on the
    /// level is in reach, four is not.
    #[test]
    fn a_strike_reaches_three_blocks() {
        let w = World::from_solid_cells((0..10).map(|x| [x, 0, 0]).collect());
        assert!(strikes(&w, [0, 1, 0], [3, 1, 0], 0.6, 1.95));
        assert!(!strikes(&w, [0, 1, 0], [4, 1, 0], 0.6, 1.95));
    }
}
