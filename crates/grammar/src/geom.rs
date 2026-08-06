//! Integer voxel boxes and local-axis orientation.
//!
//! Ported from `GrammarBox.py` (`BoundingBox`) and the orientation half of
//! `SplitGrammar.py`'s `Scope` in `yawgmoth/GDMC25` (BSD-3-Clause; see
//! `LICENSE-GDMC25`).
//!
//! A scope is a world-space box plus an [`Orientation`]: a permutation mapping
//! the grammar's *local* axes (the `X`/`Y`/`Z` a rule writes) onto world axes.
//! Rules are therefore written once and reused under `reorient`.

use serde::{Deserialize, Serialize};

/// A world axis. `Y` is up, matching Minecraft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// West-east.
    X,
    /// Down-up.
    Y,
    /// North-south.
    Z,
}

impl Axis {
    /// The axis as an index into a `[_; 3]` world-space triple.
    pub const fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    /// The axis for an index; panics outside `0..=2`.
    pub const fn from_index(i: usize) -> Axis {
        match i {
            0 => Axis::X,
            1 => Axis::Y,
            2 => Axis::Z,
            _ => panic!("axis index out of range"),
        }
    }

    /// All three axes in canonical order.
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];
}

/// A half-open integer box: `origin` inclusive, `origin + size` exclusive.
///
/// Sizes are unsigned; a zero-sized box is legal (it simply contains no cells)
/// because grammar splits legitimately produce degenerate leftovers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Box3 {
    /// Smallest corner, world space.
    pub origin: [i32; 3],
    /// Extent along each world axis.
    pub size: [u32; 3],
}

impl Box3 {
    /// A box at `origin` with `size`.
    pub const fn new(origin: [i32; 3], size: [u32; 3]) -> Self {
        Self { origin, size }
    }

    /// A box at the world origin with `size`.
    pub const fn at_origin(size: [u32; 3]) -> Self {
        Self::new([0, 0, 0], size)
    }

    /// Extent along a world axis.
    pub fn extent(&self, axis: Axis) -> u32 {
        self.size[axis.index()]
    }

    /// Exclusive maximum corner.
    pub fn maximum(&self) -> [i32; 3] {
        [
            self.origin[0] + self.size[0] as i32,
            self.origin[1] + self.size[1] as i32,
            self.origin[2] + self.size[2] as i32,
        ]
    }

    /// Number of cells the box covers.
    pub fn volume(&self) -> u64 {
        self.size[0] as u64 * self.size[1] as u64 * self.size[2] as u64
    }

    /// True when the box covers no cells.
    pub fn is_empty(&self) -> bool {
        self.volume() == 0
    }

    /// Every position inside the box, iterated `x`, then `y`, then `z` — the
    /// same order as upstream's `BoundingBox.positions`, and the order every
    /// randomised per-cell fill consumes its PRNG stream in (ADR-0006).
    pub fn positions(&self) -> impl Iterator<Item = [i32; 3]> + '_ {
        let max = self.maximum();
        (self.origin[0]..max[0]).flat_map(move |x| {
            (self.origin[1]..max[1])
                .flat_map(move |y| (self.origin[2]..max[2]).map(move |z| [x, y, z]))
        })
    }
}

/// A permutation mapping local axes onto world axes.
///
/// `orientation.get(Axis::X)` is the world axis that the rule's local `X` names.
/// The identity orientation maps every local axis onto its namesake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orientation {
    /// World axis of the local `X`.
    pub x: Axis,
    /// World axis of the local `Y`.
    pub y: Axis,
    /// World axis of the local `Z`.
    pub z: Axis,
}

impl Default for Orientation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Orientation {
    /// Local axes map onto their world namesakes.
    pub const IDENTITY: Orientation = Orientation {
        x: Axis::X,
        y: Axis::Y,
        z: Axis::Z,
    };

    /// The world axis a local axis names.
    pub fn get(&self, local: Axis) -> Axis {
        match local {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// As a `[world_axis; 3]` triple indexed by local axis.
    pub fn axes(&self) -> [Axis; 3] {
        [self.x, self.y, self.z]
    }

    /// Rebuild from a `[world_axis; 3]` triple indexed by local axis.
    pub fn from_axes(axes: [Axis; 3]) -> Orientation {
        Orientation {
            x: axes[0],
            y: axes[1],
            z: axes[2],
        }
    }

    /// The local axis naming a given world axis, if any.
    pub fn local_of(&self, world: Axis) -> Option<Axis> {
        Axis::ALL.into_iter().find(|&l| self.get(l) == world)
    }

    /// True when every world axis is named exactly once — the invariant every
    /// orientation must satisfy.
    pub fn is_permutation(&self) -> bool {
        let a = self.axes();
        a[0] != a[1] && a[1] != a[2] && a[0] != a[2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_iterate_x_then_y_then_z() {
        let b = Box3::new([1, 2, 3], [2, 1, 2]);
        let got: Vec<_> = b.positions().collect();
        assert_eq!(
            got,
            vec![[1, 2, 3], [1, 2, 4], [2, 2, 3], [2, 2, 4]],
            "cell order is the PRNG consumption order and must not drift"
        );
        assert_eq!(b.volume(), 4);
        assert_eq!(b.maximum(), [3, 3, 5]);
    }

    #[test]
    fn zero_sized_boxes_are_legal_and_empty() {
        let b = Box3::new([0, 0, 0], [4, 0, 4]);
        assert!(b.is_empty());
        assert_eq!(b.positions().count(), 0);
    }

    #[test]
    fn identity_orientation_is_a_permutation() {
        assert!(Orientation::IDENTITY.is_permutation());
        assert_eq!(Orientation::IDENTITY.local_of(Axis::Z), Some(Axis::Z));
        assert!(
            !Orientation {
                x: Axis::X,
                y: Axis::X,
                z: Axis::Z
            }
            .is_permutation()
        );
    }
}
