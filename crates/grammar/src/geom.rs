//! Integer voxel boxes and local-axis orientation.
//!
//! Ported from `GrammarBox.py` (`BoundingBox`) and the orientation half of
//! `SplitGrammar.py`'s `Scope` in `yawgmoth/GDMC25` (BSD-3-Clause; see
//! `LICENSE-GDMC25`).
//!
//! A scope is a world-space box plus an [`Orientation`]: a **signed**
//! permutation mapping the grammar's *local* axes (the `X`/`Y`/`Z` a rule
//! writes) onto world axes. Rules are therefore written once and reused under
//! `reorient`.
//!
//! # Why the map carries signs
//!
//! Upstream's orientation is a bare axis permutation — six maps, all of them
//! taking local minimum to world minimum. That is enough to turn a piece 90°
//! and not enough to turn it **round**, and the difference is not academic:
//!
//! * A hairpin's second leg is the first leg rotated 180° about the vertical.
//!   Every one-sided piece — a ledge with the drop on one hand, a stair whose
//!   treads peel off one end, a guard post at one end of a causeway — meets
//!   this the moment a route doubles back.
//! * The vocabulary recorded the gap as "a permutation cannot **reflect**",
//!   which is the wrong diagnosis and sent two rounds looking for the wrong
//!   primitive. A half-turn is a *rotation*: proper, orientation-preserving,
//!   chirality-preserving. What the map could not express was not reflection
//!   in particular but **sign at all** — of the 48 signed axis maps, a bare
//!   permutation reaches 6, and of the 24 rotations it reaches 3.
//!
//! So a local axis now records whether it runs *with* or *against* the world
//! axis it names. Signs default to positive everywhere and no rule sets one
//! unless it asks, so every program written against the unsigned map expands to
//! the same bytes it always did.
//!
//! Reflections (an odd number of reversals) are expressible too and are not
//! refused: a mirrored building is a legitimate thing to want. They do flip
//! chirality, so [`Orientation::is_rotation`] exists to let a caller — or a
//! test — say which kind it is holding.

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

    /// The axis's name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        }
    }
}

impl std::fmt::Display for Axis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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

/// A signed permutation mapping local axes onto world axes.
///
/// `orientation.get(Axis::X)` is the world axis that the rule's local `X` names,
/// and `orientation.is_reversed(Axis::X)` says whether local `X` counts *down*
/// that world axis. The identity orientation maps every local axis onto its
/// namesake, running forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Orientation {
    /// World axis of the local `X`.
    pub x: Axis,
    /// World axis of the local `Y`.
    pub y: Axis,
    /// World axis of the local `Z`.
    pub z: Axis,
    /// Which local axes run **against** the world axis they name, indexed by
    /// local axis ([`Axis::index`]).
    ///
    /// A reversed local axis puts the rule's local *minimum* at the box's world
    /// *maximum*: split pieces are laid from that end, and a local offset is
    /// measured from it. Defaults to all-forward, and is skipped in JSON when
    /// it is, so an unsigned program serialises exactly as it always did.
    #[serde(default, skip_serializing_if = "not_reversed")]
    pub reversed: [bool; 3],
}

/// Serde helper: an all-forward orientation writes no `reversed` key.
fn not_reversed(reversed: &[bool; 3]) -> bool {
    !reversed[0] && !reversed[1] && !reversed[2]
}

impl Default for Orientation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Orientation {
    /// Local axes map onto their world namesakes, all running forward.
    pub const IDENTITY: Orientation = Orientation {
        x: Axis::X,
        y: Axis::Y,
        z: Axis::Z,
        reversed: [false; 3],
    };

    /// The world axis a local axis names.
    pub fn get(&self, local: Axis) -> Axis {
        match local {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// Whether a local axis counts down the world axis it names.
    pub fn is_reversed(&self, local: Axis) -> bool {
        self.reversed[local.index()]
    }

    /// The world offset of a local offset along one local axis, given the
    /// scope's world extent along the axis that local one names.
    ///
    /// This is the one place the sign is applied to a coordinate, so a mark, a
    /// split cursor and a face all mirror the same way or none of them do.
    /// Order-reversing on `0..extent`, which is why an out-of-range local
    /// offset stays out of range once mapped and cannot smuggle itself back
    /// inside the box.
    pub fn place(&self, local: Axis, offset: i64, extent: u32) -> i64 {
        if self.is_reversed(local) {
            extent as i64 - 1 - offset
        } else {
            offset
        }
    }

    /// As a `[world_axis; 3]` triple indexed by local axis. Signs are not part
    /// of it — ask [`Orientation::is_reversed`] for those.
    pub fn axes(&self) -> [Axis; 3] {
        [self.x, self.y, self.z]
    }

    /// Rebuild from a `[world_axis; 3]` triple indexed by local axis, all
    /// running forward.
    pub fn from_axes(axes: [Axis; 3]) -> Orientation {
        Orientation::from_signed_axes(axes, [false; 3])
    }

    /// Rebuild from a `[world_axis; 3]` triple and its per-local-axis signs.
    pub fn from_signed_axes(axes: [Axis; 3], reversed: [bool; 3]) -> Orientation {
        Orientation {
            x: axes[0],
            y: axes[1],
            z: axes[2],
            reversed,
        }
    }

    /// The local axis naming a given world axis, if any.
    pub fn local_of(&self, world: Axis) -> Option<Axis> {
        Axis::ALL.into_iter().find(|&l| self.get(l) == world)
    }

    /// True when every world axis is named exactly once — the invariant every
    /// orientation must satisfy. Signs are free and never affect it.
    pub fn is_permutation(&self) -> bool {
        let a = self.axes();
        a[0] != a[1] && a[1] != a[2] && a[0] != a[2]
    }

    /// True when the map is a **rotation** — proper, chirality-preserving — and
    /// false when it is a reflection.
    ///
    /// The determinant of the signed permutation: the parity of the axis
    /// permutation times the parity of the reversals. A half-turn about the
    /// vertical (local `X` and local `Z` both reversed) is proper, which is why
    /// a hairpin's second leg is genuinely the same piece turned round and not
    /// a mirror image of it.
    pub fn is_rotation(&self) -> bool {
        debug_assert!(self.is_permutation());
        let a = self.axes();
        // A 3-permutation is odd exactly when it is a transposition, i.e. when
        // exactly one local axis keeps its namesake.
        let fixed = (0..3).filter(|&i| a[i] == Axis::from_index(i)).count();
        let perm_even = fixed != 1;
        let flips_even = self.reversed.iter().filter(|r| **r).count() % 2 == 0;
        perm_even == flips_even
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
                z: Axis::Z,
                reversed: [false; 3],
            }
            .is_permutation()
        );
    }

    #[test]
    fn a_reversed_axis_puts_the_local_minimum_at_the_world_maximum() {
        let turned = Orientation {
            reversed: [true, false, true],
            ..Orientation::IDENTITY
        };
        // Local Z-min is world Z-max, and the mapping is an involution.
        assert_eq!(turned.place(Axis::Z, 0, 10), 9);
        assert_eq!(turned.place(Axis::Z, 9, 10), 0);
        assert_eq!(turned.place(Axis::Y, 3, 10), 3, "Y was not reversed");
        // Order-reversing on the whole integer line, so out of range stays out.
        assert!(turned.place(Axis::X, -1, 4) >= 4);
        assert!(turned.place(Axis::X, 4, 4) < 0);
    }

    #[test]
    fn the_half_turn_about_the_vertical_is_a_rotation_and_one_flip_is_not() {
        // The switchback's second leg: X and Z both reversed, Y left alone.
        let half_turn = Orientation {
            reversed: [true, false, true],
            ..Orientation::IDENTITY
        };
        assert!(half_turn.is_permutation());
        assert!(
            half_turn.is_rotation(),
            "a hairpin wants a rotation, not a reflection — that was the whole \
             misdiagnosis"
        );
        let mirror = Orientation {
            reversed: [true, false, false],
            ..Orientation::IDENTITY
        };
        assert!(mirror.is_permutation());
        assert!(!mirror.is_rotation(), "one reversal mirrors");
        // ...and the parity of the axis permutation counts too: a bare
        // transposition is already improper before any sign is set.
        let swap = Orientation {
            x: Axis::Z,
            y: Axis::Y,
            z: Axis::X,
            reversed: [false; 3],
        };
        assert!(!swap.is_rotation());
        assert!(
            Orientation {
                reversed: [true, false, false],
                ..swap
            }
            .is_rotation()
        );
    }

    #[test]
    fn an_unsigned_orientation_serialises_without_the_sign_field() {
        let json = serde_json::to_string(&Orientation::IDENTITY).unwrap();
        assert!(
            !json.contains("reversed"),
            "an unsigned orientation must round trip as the bytes it always \
             was, got {json}"
        );
        assert_eq!(
            serde_json::from_str::<Orientation>(&json).unwrap(),
            Orientation::IDENTITY
        );
        let turned = Orientation {
            reversed: [true, false, true],
            ..Orientation::IDENTITY
        };
        let json = serde_json::to_string(&turned).unwrap();
        assert!(json.contains("reversed"));
        assert_eq!(serde_json::from_str::<Orientation>(&json).unwrap(), turned);
    }
}
