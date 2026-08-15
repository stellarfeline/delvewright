//! **A stair's `shape` is not stored — it is derived, and the game derives it
//! again.**
//!
//! Every other block-state property an author writes is kept: a
//! `facing=east` stair faces east forever. `shape` is different. The game
//! recomputes it from the stair's neighbours on every horizontal block update
//! at that cell, so an authored `shape` is not a decision — it is a *claim*
//! about the four cells around it, and a wrong claim is corrected by the world
//! the first time anything is placed, broken or flooded beside it.
//!
//! That makes it the one property that can be right in every tool this project
//! owns and wrong in the game. The render draws what the bytes say. The
//! reviewer approves the picture. The world draws something else.
//!
//! # The derivation
//!
//! Straight unless a stair of the same `half` sits in front of or behind it
//! *across* its facing axis:
//!
//! - a perpendicular stair in FRONT (the direction the stair faces) makes an
//!   OUTER corner — left if that stair faces this one's counter-clockwise
//!   direction, otherwise right;
//! - a perpendicular stair BEHIND makes an INNER corner, by the same hand;
//! - either is suppressed when the cell on the far side of the turn already
//!   holds a stair with the same `facing` and `half` — vanilla's
//!   `canTakeShape`, which is what keeps a continuous run from mitring itself
//!   at every step.
//!
//! Neighbour means *any* stair block, never the same one: an oak stair mitres
//! against a stone-brick stair.
//!
//! # Why this is measured rather than read
//!
//! [`derive_shape`] is replayed cell for cell against a field of stairs placed,
//! settled and read back on the pinned 1.21.11 server
//! (`tools/spike-block-settling/`, its `observations.json`, and the test that
//! replays it). A reading of vanilla's source that nothing re-checks is exactly
//! the kind of second-hand fact this project has been bitten by; the game's own
//! answers are in the repository instead.

use std::fmt;

/// A stair whose `shape` disagrees with the shape vanilla derives at its cell.
pub const DW_STAIR_SHAPE_DERIVED: &str = "DW0801";

/// The four horizontal directions, in vanilla's own vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Facing {
    North,
    South,
    East,
    West,
}

impl Facing {
    /// Every facing, in a fixed order.
    pub const ALL: [Facing; 4] = [Facing::North, Facing::South, Facing::East, Facing::West];

    /// Parse a `facing` property value.
    pub fn parse(s: &str) -> Option<Facing> {
        Some(match s {
            "north" => Facing::North,
            "south" => Facing::South,
            "east" => Facing::East,
            "west" => Facing::West,
            _ => return None,
        })
    }

    /// The property value.
    pub fn as_str(self) -> &'static str {
        match self {
            Facing::North => "north",
            Facing::South => "south",
            Facing::East => "east",
            Facing::West => "west",
        }
    }

    /// The opposite direction.
    pub fn opposite(self) -> Facing {
        match self {
            Facing::North => Facing::South,
            Facing::South => Facing::North,
            Facing::East => Facing::West,
            Facing::West => Facing::East,
        }
    }

    /// A quarter turn anti-clockwise about the world's vertical axis, which is
    /// the hand vanilla's corner naming uses.
    pub fn counter_clockwise(self) -> Facing {
        match self {
            Facing::North => Facing::West,
            Facing::West => Facing::South,
            Facing::South => Facing::East,
            Facing::East => Facing::North,
        }
    }

    /// True when the two directions run along the same axis.
    pub fn same_axis(self, other: Facing) -> bool {
        self == other || self == other.opposite()
    }

    /// The one-cell step in this direction, in Minecraft's axes (north is −z).
    pub fn step(self) -> [i32; 3] {
        match self {
            Facing::North => [0, 0, -1],
            Facing::South => [0, 0, 1],
            Facing::East => [1, 0, 0],
            Facing::West => [-1, 0, 0],
        }
    }
}

impl fmt::Display for Facing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which half of the cell the solid part of a stair occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Half {
    Bottom,
    Top,
}

impl Half {
    /// Parse a `half` property value.
    pub fn parse(s: &str) -> Option<Half> {
        Some(match s {
            "bottom" => Half::Bottom,
            "top" => Half::Top,
            _ => return None,
        })
    }

    /// The property value.
    pub fn as_str(self) -> &'static str {
        match self {
            Half::Bottom => "bottom",
            Half::Top => "top",
        }
    }
}

impl fmt::Display for Half {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The five values of a stair's `shape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shape {
    Straight,
    InnerLeft,
    InnerRight,
    OuterLeft,
    OuterRight,
}

impl Shape {
    /// Parse a `shape` property value.
    pub fn parse(s: &str) -> Option<Shape> {
        Some(match s {
            "straight" => Shape::Straight,
            "inner_left" => Shape::InnerLeft,
            "inner_right" => Shape::InnerRight,
            "outer_left" => Shape::OuterLeft,
            "outer_right" => Shape::OuterRight,
            _ => return None,
        })
    }

    /// The property value.
    pub fn as_str(self) -> &'static str {
        match self {
            Shape::Straight => "straight",
            Shape::InnerLeft => "inner_left",
            Shape::InnerRight => "inner_right",
            Shape::OuterLeft => "outer_left",
            Shape::OuterRight => "outer_right",
        }
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The two properties of a neighbouring stair the derivation reads. It reads
/// nothing else — not the block, not the neighbour's own `shape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stair {
    pub facing: Facing,
    pub half: Half,
}

/// **The `shape` vanilla derives for a stair at a cell.**
///
/// `neighbour` answers what stair, if any, stands one cell away in a given
/// direction — `None` for air, for a non-stair block, and for anything outside
/// the piece being judged.
pub fn derive_shape(stair: Stair, neighbour: impl Fn(Facing) -> Option<Stair>) -> Shape {
    let can_take_shape = |dir: Facing| match neighbour(dir) {
        Some(n) => n.facing != stair.facing || n.half != stair.half,
        None => true,
    };

    if let Some(front) = neighbour(stair.facing)
        && front.half == stair.half
        && !front.facing.same_axis(stair.facing)
        && can_take_shape(front.facing.opposite())
    {
        return if front.facing == stair.facing.counter_clockwise() {
            Shape::OuterLeft
        } else {
            Shape::OuterRight
        };
    }

    if let Some(back) = neighbour(stair.facing.opposite())
        && back.half == stair.half
        && !back.facing.same_axis(stair.facing)
        && can_take_shape(back.facing)
    {
        return if back.facing == stair.facing.counter_clockwise() {
            Shape::InnerLeft
        } else {
            Shape::InnerRight
        };
    }

    Shape::Straight
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stair(facing: Facing, half: Half) -> Stair {
        Stair { facing, half }
    }

    #[test]
    fn a_lone_stair_is_straight() {
        assert_eq!(
            derive_shape(stair(Facing::North, Half::Bottom), |_| None),
            Shape::Straight
        );
    }

    #[test]
    fn a_run_of_parallel_stairs_is_straight() {
        // Front and back both hold a stair facing the same way: the whole run
        // is straight, which is the case every flight of steps is made of.
        assert_eq!(
            derive_shape(stair(Facing::North, Half::Bottom), |_| Some(stair(
                Facing::North,
                Half::Bottom
            ))),
            Shape::Straight
        );
    }

    #[test]
    fn a_perpendicular_stair_in_front_is_an_outer_corner() {
        let north = stair(Facing::North, Half::Bottom);
        // North's counter-clockwise is west.
        assert_eq!(
            derive_shape(north, |d| (d == Facing::North)
                .then_some(stair(Facing::West, Half::Bottom))),
            Shape::OuterLeft
        );
        assert_eq!(
            derive_shape(north, |d| (d == Facing::North)
                .then_some(stair(Facing::East, Half::Bottom))),
            Shape::OuterRight
        );
    }

    #[test]
    fn a_perpendicular_stair_behind_is_an_inner_corner() {
        let north = stair(Facing::North, Half::Bottom);
        assert_eq!(
            derive_shape(north, |d| (d == Facing::South)
                .then_some(stair(Facing::West, Half::Bottom))),
            Shape::InnerLeft
        );
        assert_eq!(
            derive_shape(north, |d| (d == Facing::South)
                .then_some(stair(Facing::East, Half::Bottom))),
            Shape::InnerRight
        );
    }

    #[test]
    fn a_neighbour_in_the_other_half_makes_no_corner() {
        assert_eq!(
            derive_shape(stair(Facing::North, Half::Bottom), |d| (d == Facing::North)
                .then_some(stair(Facing::West, Half::Top))),
            Shape::Straight
        );
    }

    #[test]
    fn the_front_corner_wins_over_the_back_one() {
        assert_eq!(
            derive_shape(stair(Facing::North, Half::Bottom), |d| match d {
                Facing::North => Some(stair(Facing::West, Half::Bottom)),
                Facing::South => Some(stair(Facing::East, Half::Bottom)),
                _ => None,
            }),
            Shape::OuterLeft
        );
    }

    #[test]
    fn a_continuous_run_does_not_mitre_itself() {
        // `canTakeShape`: the turn is suppressed when the cell on the far side
        // of it already carries a stair of this facing and half.
        let north = stair(Facing::North, Half::Bottom);
        assert_eq!(
            derive_shape(north, |d| match d {
                Facing::North => Some(stair(Facing::West, Half::Bottom)),
                Facing::East => Some(stair(Facing::North, Half::Bottom)),
                _ => None,
            }),
            Shape::Straight
        );
        // ...and is NOT suppressed when that cell carries a stair of a
        // different half, which is a different building.
        assert_eq!(
            derive_shape(north, |d| match d {
                Facing::North => Some(stair(Facing::West, Half::Bottom)),
                Facing::East => Some(stair(Facing::North, Half::Top)),
                _ => None,
            }),
            Shape::OuterLeft
        );
    }
}
