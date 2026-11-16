//! **Where a party carry puts each member** (`teleport` with `from: "party"`,
//! DSL v0.88).
//!
//! A party carry moves every member present, wherever they stand, onto its
//! mark. Members are not stacked on one cell: each is given a **seat** of their
//! own, and the seats are derived here and never written — the mark's cell
//! first, then the nearest other safe standing cells, nearest ring first
//! (Chebyshev distance 1, then 2, up to [`SNAP_RADIUS`]), within a ring the
//! nearest to the mark and then the least cell. A seat is a cell a body may be
//! put down on by declaration ([`World::is_safe_seat`], the respawn seat's
//! rule) that a body standing on the mark reaches ([`World::body_reach`]), so
//! no member is put behind a wall from the rest. Two seats are two cells, and
//! a body is narrower than a cell, so no member lands inside another.
//!
//! The party is one to four players ([`PARTY_SEATS`]); a mark with fewer seats
//! than that is refused (`DW1141`). The seats are derived over the world as
//! shipped, as a respawn seat is ([`World::seating_world`]).

use super::*;
use crate::compiler::failure::Failure;

delvewright_dsl::dw_code! {
    /// `DW1141`: **a party carry with no seat for every member.** A `teleport`
    /// whose subject is the party names a mark that is not a cell a body may be
    /// put down on, or around which fewer than [`PARTY_SEATS`] such cells stand
    /// within [`SNAP_RADIUS`] that a body on the mark walks to.
    pub const DW_PARTY_CARRY_UNSEATED: DwCode = DwCode::new("DW1141", ExitTier::Build);
}

/// The most members a party has, and so the seats a party carry owes.
pub const PARTY_SEATS: usize = 4;

impl World {
    /// **The seats of a party carry onto `to`**, in the order members take
    /// them: `to` first, then the nearest safe cells a body on `to` reaches, up
    /// to [`PARTY_SEATS`]. Empty when `to` itself is not a safe seat; shorter
    /// than [`PARTY_SEATS`] when the mark has fewer seats around it.
    pub fn party_seats(&self, to: [i32; 3]) -> Vec<[i32; 3]> {
        if !self.is_safe_seat(to) {
            return Vec::new();
        }
        let reached = self.body_reach(&[to]).standing();
        let mut out = vec![to];
        for ring in 1..=SNAP_RADIUS {
            let mut found: Vec<(i32, [i32; 3])> = Vec::new();
            for dy in -ring..=ring {
                for dz in -ring..=ring {
                    for dx in -ring..=ring {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != ring {
                            continue;
                        }
                        let c = [to[0] + dx, to[1] + dy, to[2] + dz];
                        if self.is_safe_seat(c) && reached.contains(&c) {
                            found.push((dx * dx + dy * dy + dz * dz, c));
                        }
                    }
                }
            }
            found.sort();
            for (_, c) in found {
                if out.len() == PARTY_SEATS {
                    return out;
                }
                out.push(c);
            }
        }
        out.truncate(PARTY_SEATS);
        out
    }
}

/// **Fix every party carry's seats for the emitter** ([`Plan::party_seats`])
/// from the build's final world, refusing a carry without a seat for every
/// member (`DW1141`). Returns the seats fixed, counted over the distinct marks.
pub fn fix_party_seats(plan: &Plan, world: &World) -> Result<usize, Failure> {
    if plan.party_carries.is_empty() {
        return Ok(0);
    }
    let shipped = world.seating_world(plan);
    let seating = shipped.as_ref().unwrap_or(world);
    let mut seats: std::collections::BTreeMap<[i32; 3], Vec<[i32; 3]>> =
        std::collections::BTreeMap::new();
    for c in &plan.party_carries {
        if seats.contains_key(&c.to) {
            continue;
        }
        let s = seating.party_seats(c.to);
        if s.len() < PARTY_SEATS {
            let found = if s.is_empty() {
                format!(
                    "{:?} is not a cell a body may be put down on: it stands on no floor, is \
                     inside a block, or meets a block that hurts a body",
                    c.to
                )
            } else {
                format!(
                    "only {} cell(s) a body may be put down on and walks to from the mark stand \
                     within {SNAP_RADIUS} blocks of it ({:?})",
                    s.len(),
                    s
                )
            };
            return Err(Failure {
                code: DW_PARTY_CARRY_UNSEATED,
                message: format!(
                    "the party `teleport` at `{}` (fired by {}) puts every member on mark `{}` \
                     at {:?}, each on a cell of their own — the mark's, then the nearest cells \
                     around it a body walks to — and a party is up to {PARTY_SEATS} players. \
                     {found}. Fault: no seat for every member. Remedy: move the mark (its \
                     anchor or `offset`) onto open floor with room for {PARTY_SEATS} players \
                     to stand side by side.",
                    c.path, c.root, c.to_anchor, c.to
                ),
            });
        }
        seats.insert(c.to, s);
    }
    let n = seats.values().map(Vec::len).sum();
    let _ = plan.party_seats.set(seats);
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;

    /// A floor of `w × d` stone at y 64 (stand at y 65).
    fn floor(w: i32, d: i32) -> Vec<([i32; 3], &'static str)> {
        let mut cells = Vec::new();
        for x in 0..w {
            for z in 0..d {
                cells.push(([x, 64, z], "minecraft:stone"));
            }
        }
        cells
    }

    #[test]
    fn the_mark_is_the_first_seat_and_the_rest_are_its_nearest_neighbours() {
        let w = blocks_world(&floor(5, 5));
        let seats = w.party_seats([2, 65, 2]);
        assert_eq!(seats.len(), PARTY_SEATS);
        assert_eq!(seats[0], [2, 65, 2]);
        // The ring-1 cells at distance 1, least first.
        assert_eq!(&seats[1..], &[[1, 65, 2], [2, 65, 1], [2, 65, 3]]);
        let distinct: std::collections::BTreeSet<_> = seats.iter().collect();
        assert_eq!(distinct.len(), seats.len(), "no two members share a cell");
    }

    #[test]
    fn a_mark_in_a_block_has_no_seat() {
        let mut cells = floor(5, 5);
        cells.push(([2, 65, 2], "minecraft:stone"));
        let w = blocks_world(&cells);
        assert!(w.party_seats([2, 65, 2]).is_empty());
    }

    #[test]
    fn a_seat_behind_a_wall_is_not_a_seat() {
        // A one-wide corridor along x with a sealed cell across a wall at z 2.
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..3 {
            cells.push(([x, 64, 0], "minecraft:stone"));
            for y in 65..68 {
                cells.push(([x, y, 1], "minecraft:stone"));
            }
            cells.push(([x, 64, 2], "minecraft:stone"));
        }
        let w = blocks_world(&cells);
        let seats = w.party_seats([1, 65, 0]);
        assert!(
            seats.iter().all(|c| c[2] == 0),
            "no seat across the wall: {seats:?}"
        );
        assert!(seats.len() < PARTY_SEATS, "{seats:?}");
    }
}
