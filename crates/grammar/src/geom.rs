//! Integer voxel boxes and local-axis orientation.
//!
//! Ported from `GrammarBox.py` (`BoundingBox`) and the orientation half of
//! `SplitGrammar.py`'s `Scope` in `yawgmoth/GDMC25` (BSD-3-Clause; see
//! `LICENSE-GDMC25`).
//!
//! A scope is a world-space box plus an [`Orientation`]: the **frame** a rule
//! reads its box through. A frame says two things about each local axis (the
//! `X`/`Y`/`Z` a rule writes) — which world axis it names, and which way along
//! that axis local coordinates increase. Rules are therefore written once and
//! reused turned (`reorient`) or reflected (`mirror`).

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
#[serde(deny_unknown_fields)]
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

/// Which local axes run **backwards** along the world axis they name.
///
/// Bilateral symmetry is the commonest symmetry a building has, and a
/// permutation cannot express it: turning a scope 180° about the vertical also
/// turns the other horizontal axis, so the two halves of a transept, a facade or
/// a stair pair are not related by any permutation. A reflection is the missing
/// sign, and it belongs to the frame rather than to any one verb, because
/// *every* question a rule asks of its box — where a split lays its first piece,
/// which corner `corner_min` is, which way an anchor looks — is asked in the
/// frame's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mirror {
    /// The local `X` runs against the world axis it names.
    #[serde(default, skip_serializing_if = "is_false")]
    pub x: bool,
    /// The local `Y` runs against the world axis it names.
    #[serde(default, skip_serializing_if = "is_false")]
    pub y: bool,
    /// The local `Z` runs against the world axis it names.
    #[serde(default, skip_serializing_if = "is_false")]
    pub z: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl Mirror {
    /// Every local axis runs forward — the frame every rule starts in.
    pub const NONE: Mirror = Mirror {
        x: false,
        y: false,
        z: false,
    };

    /// Reflect one local axis.
    pub const fn of(axis: Axis) -> Mirror {
        match axis {
            Axis::X => Mirror {
                x: true,
                y: false,
                z: false,
            },
            Axis::Y => Mirror {
                x: false,
                y: true,
                z: false,
            },
            Axis::Z => Mirror {
                x: false,
                y: false,
                z: true,
            },
        }
    }

    /// True when nothing is reflected.
    pub fn is_none(&self) -> bool {
        *self == Mirror::NONE
    }

    /// Whether one local axis is reflected.
    pub fn get(&self, local: Axis) -> bool {
        match local {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// Also reflect `axis` (builder form).
    pub fn and(mut self, axis: Axis) -> Mirror {
        match axis {
            Axis::X => self.x = true,
            Axis::Y => self.y = true,
            Axis::Z => self.z = true,
        }
        self
    }

    /// As a `[bool; 3]` triple indexed by local axis.
    pub fn axes(&self) -> [bool; 3] {
        [self.x, self.y, self.z]
    }

    /// Rebuild from a `[bool; 3]` triple indexed by local axis.
    pub fn from_axes(axes: [bool; 3]) -> Mirror {
        Mirror {
            x: axes[0],
            y: axes[1],
            z: axes[2],
        }
    }
}

/// The frame a rule reads its box through: a permutation mapping local axes onto
/// world axes, plus the direction each local axis runs in.
///
/// `orientation.axis(Axis::X)` is the world axis that the rule's local `X`
/// names; `orientation.reversed(Axis::X)` says whether local `X` counts *down*
/// that world axis. The identity frame maps every local axis onto its namesake,
/// running forward.
///
/// Deliberately **not** serialisable. It is the frame a scope is *in* at
/// expansion time, not a thing a document says: what a document writes is
/// [`ir::Reorient`](crate::ir::Reorient), a frame *request*. It carried a
/// `Serialize`/`Deserialize` derive that no format ever reached, which put a
/// second `mirror` field on the document surface's ledger with no document
/// behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Orientation {
    /// World axis of the local `X`.
    pub x: Axis,
    /// World axis of the local `Y`.
    pub y: Axis,
    /// World axis of the local `Z`.
    pub z: Axis,
    /// Which local axes run backwards along the world axis they name.
    pub mirror: Mirror,
}

impl Default for Orientation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Orientation {
    /// Local axes map onto their world namesakes, running forward.
    pub const IDENTITY: Orientation = Orientation {
        x: Axis::X,
        y: Axis::Y,
        z: Axis::Z,
        mirror: Mirror::NONE,
    };

    /// The world axis a local axis names.
    ///
    /// Deliberately *not* called `get`: it answers half the question, and the
    /// other half — which way along that axis local coordinates increase — is
    /// [`Orientation::reversed`]. A caller that only needs an extent wants this
    /// one; a caller that places a cell needs both, and has to say so.
    pub fn axis(&self, local: Axis) -> Axis {
        match local {
            Axis::X => self.x,
            Axis::Y => self.y,
            Axis::Z => self.z,
        }
    }

    /// Whether a local axis counts *down* the world axis it names.
    pub fn reversed(&self, local: Axis) -> bool {
        self.mirror.get(local)
    }

    /// The world cell offset, from the box's minimum **world** corner, of local
    /// coordinate `coord` along local axis `local` in a box of `size`.
    ///
    /// This is the one place the frame's sign turns into a number, so that "a
    /// reflected axis counts from the other end" is stated once rather than at
    /// every site that places a cell.
    pub fn offset(&self, local: Axis, coord: i64, size: [u32; 3]) -> i64 {
        let world = self.axis(local);
        if self.reversed(local) {
            size[world.index()] as i64 - 1 - coord
        } else {
            coord
        }
    }

    /// As a `[world_axis; 3]` triple indexed by local axis.
    pub fn axes(&self) -> [Axis; 3] {
        [self.x, self.y, self.z]
    }

    /// Rebuild from a `[world_axis; 3]` triple indexed by local axis, running
    /// forward.
    pub fn from_axes(axes: [Axis; 3]) -> Orientation {
        Orientation {
            x: axes[0],
            y: axes[1],
            z: axes[2],
            mirror: Mirror::NONE,
        }
    }

    /// The same permutation, with `mirror` applied (builder form).
    pub fn mirrored(mut self, mirror: Mirror) -> Orientation {
        self.mirror = mirror;
        self
    }

    /// The local axis naming a given world axis, if any.
    pub fn local_of(&self, world: Axis) -> Option<Axis> {
        Axis::ALL.into_iter().find(|&l| self.axis(l) == world)
    }

    /// True when every world axis is named exactly once — the invariant every
    /// orientation must satisfy.
    pub fn is_permutation(&self) -> bool {
        let a = self.axes();
        a[0] != a[1] && a[1] != a[2] && a[0] != a[2]
    }

    /// True when the frame is a **rotation** — proper, chirality-preserving —
    /// and false when it is a reflection.
    ///
    /// The determinant of the signed permutation: the parity of the axis
    /// permutation times the parity of the reflections. A half-turn about the
    /// vertical (local `X` and local `Z` both reversed) is proper, which is why
    /// a route doubling back is genuinely the same piece turned round and not a
    /// mirror image of it — a distinction a caller placing a chiral piece has
    /// to be able to ask about, and which [`Orientation::reversed`] alone
    /// cannot answer.
    pub fn is_rotation(&self) -> bool {
        debug_assert!(self.is_permutation());
        let a = self.axes();
        // A 3-permutation is odd exactly when it is a transposition, i.e. when
        // exactly one local axis keeps its namesake.
        let fixed = (0..3).filter(|&i| a[i] == Axis::from_index(i)).count();
        let perm_even = fixed != 1;
        let flips_even = self.mirror.axes().iter().filter(|r| **r).count() % 2 == 0;
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
            !Orientation::from_axes([Axis::X, Axis::X, Axis::Z]).is_permutation(),
            "naming one world axis twice is not a frame"
        );
        // A reflection does not disturb the permutation: the two halves of the
        // frame are independent, which is why one can be checked without the
        // other.
        assert!(
            Orientation::IDENTITY
                .mirrored(Mirror::of(Axis::X))
                .is_permutation()
        );
    }

    /// The frame's sign turns into a number in exactly one place, and it counts
    /// from the far end of the box.
    #[test]
    fn a_reflected_axis_counts_from_the_other_end() {
        let size = [7u32, 4, 5];
        let plain = Orientation::IDENTITY;
        assert_eq!(plain.offset(Axis::X, 0, size), 0);
        assert_eq!(plain.offset(Axis::X, 6, size), 6);

        let flipped = Orientation::IDENTITY.mirrored(Mirror::of(Axis::X));
        assert_eq!(flipped.offset(Axis::X, 0, size), 6, "local 0 is world max");
        assert_eq!(flipped.offset(Axis::X, 6, size), 0);
        assert_eq!(
            flipped.offset(Axis::Z, 0, size),
            0,
            "an unreflected axis is untouched"
        );

        // Reflection is measured on the WORLD axis the local one names, so a
        // rotated frame reflects the rotated extent.
        let rotated =
            Orientation::from_axes([Axis::Z, Axis::Y, Axis::X]).mirrored(Mirror::of(Axis::X));
        assert_eq!(
            rotated.offset(Axis::X, 0, size),
            4,
            "local X names world Z, which is 5 across"
        );
    }

    #[test]
    fn a_mirror_is_a_set_of_local_axes() {
        assert!(Mirror::NONE.is_none());
        assert_eq!(Mirror::of(Axis::Y).axes(), [false, true, false]);
        assert_eq!(
            Mirror::of(Axis::X).and(Axis::Z),
            Mirror::from_axes([true, false, true])
        );
        assert!(Mirror::of(Axis::Z).get(Axis::Z));
        assert!(!Mirror::of(Axis::Z).get(Axis::X));
    }
}
