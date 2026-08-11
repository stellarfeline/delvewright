//! Completing a partial reorientation into a full axis permutation.
//!
//! Ported from `Scope.calculate_reorientation` in `SplitGrammar.py`
//! (`yawgmoth/GDMC25`, BSD-3-Clause — see `LICENSE-GDMC25`). Upstream mixes two
//! index spaces in one array (it stores *slot* indices into a list of *world*
//! axes) and only accidentally agrees with itself because the cases it exercises
//! start from the identity orientation. We do the whole computation in one
//! space — old local axes — and convert once at the end, which reproduces
//! upstream's answers on the cases its own rule libraries use (both are
//! regression-tested below) and is well defined on the ones it is not.
//!
//! Reading the rule: naming a new axis consumes an old one. Whatever is left is
//! matched up so that the result stays as close to the parent as possible —
//! first keep an axis where you can, then complete the cycle you started (asking
//! for "my new Z is the old X" means the old Z becomes the new X, i.e. a swap),
//! and only then fall back to the lowest free axis.
//!
//! # Signs ride along, they do not participate
//!
//! Which parent axis a slot claims is decided entirely by the [`AxisSpec`]
//! passes above; the request's `reverse` flags never enter that decision. Once
//! a slot has its axis, the child's sign for that slot is the parent's sign for
//! the axis it took, **exclusive-or** the reversal the request asked for.
//!
//! That is what makes a reversal *relative to the parent* rather than to the
//! world, and it is the property a composed piece needs: a rule reasons in the
//! frame it was handed, so a piece already turned round and then turned round
//! again is the way it started, whichever world axis it happens to be lying on
//! by then. `Smallest`/`Largest` measure world extents, which signs cannot
//! change, so this is not merely convenient — nothing in the axis choice
//! *could* depend on a sign.

use crate::geom::{Axis, Orientation};
use crate::ir::{AxisSpec, Reorient};

/// Why a reorientation request could not be honoured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrientError {
    /// Two axes of the request name the same source axis.
    Conflict {
        /// The doubly-named source axis, as the parent names it.
        axis: Axis,
    },
}

impl std::fmt::Display for OrientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrientError::Conflict { axis } => write!(
                f,
                "the reorientation names the parent's local {axis:?} axis twice; \
                 an orientation is a permutation, so each axis may be claimed once"
            ),
        }
    }
}

impl std::error::Error for OrientError {}

/// Complete `request` into a full permutation.
///
/// `current` is the parent's local-to-world mapping, `size` its world-space
/// extents (which [`AxisSpec::Smallest`] / [`AxisSpec::Largest`] measure), and
/// `split_axis` the local axis being split, when the request appears on a split.
pub fn reorient(
    current: Orientation,
    size: [u32; 3],
    request: &Reorient,
    split_axis: Option<Axis>,
) -> Result<Orientation, OrientError> {
    if request.is_keep() {
        return Ok(current);
    }
    let specs = [request.x, request.y, request.z];

    // Slot -> old local axis index, filled in three passes.
    let mut from_old: [Option<usize>; 3] = [None; 3];
    let mut taken = [false; 3];

    // Pass 1: the specs that name an axis outright.
    for (slot, spec) in specs.iter().enumerate() {
        let Some(spec) = spec else { continue };
        let old = match spec {
            AxisSpec::LocalX => Some(Axis::X.index()),
            AxisSpec::LocalY => Some(Axis::Y.index()),
            AxisSpec::LocalZ => Some(Axis::Z.index()),
            AxisSpec::WorldX => current.local_of(Axis::X).map(Axis::index),
            AxisSpec::WorldY => current.local_of(Axis::Y).map(Axis::index),
            AxisSpec::WorldZ => current.local_of(Axis::Z).map(Axis::index),
            AxisSpec::SplitAxis => split_axis.map(Axis::index),
            AxisSpec::Smallest | AxisSpec::Largest => None,
        };
        if let Some(old) = old {
            claim(slot, old, &mut from_old, &mut taken)?;
        }
    }

    // Pass 2: the extremal specs, measured over what pass 1 left.
    // Ties go to the lowest world axis, as upstream's scan does.
    for (slot, spec) in specs.iter().enumerate() {
        let Some(spec) = spec else { continue };
        let want_largest = match spec {
            AxisSpec::Smallest => false,
            AxisSpec::Largest => true,
            _ => continue,
        };
        let mut best: Option<usize> = None;
        for old in free_axes(&taken) {
            let world = current.get(Axis::from_index(old)).index();
            let better = match best {
                None => true,
                Some(b) => {
                    let bw = current.get(Axis::from_index(b)).index();
                    if size[world] == size[bw] {
                        world < bw
                    } else if want_largest {
                        size[world] > size[bw]
                    } else {
                        size[world] < size[bw]
                    }
                }
            };
            if better {
                best = Some(old);
            }
        }
        if let Some(old) = best {
            claim(slot, old, &mut from_old, &mut taken)?;
        }
    }

    // Pass 3: the unnamed slots. Keep, then complete the cycle, then fall back.
    for slot in 0..3 {
        if from_old[slot].is_some() {
            continue;
        }
        let old = if !taken[slot] {
            slot
        } else if let Some(cycle) = from_old
            .iter()
            .position(|f| *f == Some(slot))
            .filter(|&j| !taken[j])
        {
            cycle
        } else {
            free_axes(&taken).next().expect("a free axis must remain")
        };
        claim(slot, old, &mut from_old, &mut taken)?;
    }

    let taken_by = [0, 1, 2].map(|slot| from_old[slot].expect("every slot is assigned"));
    let axes = taken_by.map(|old| current.get(Axis::from_index(old)));
    // The child's sign for a slot is the parent's sign for the axis that slot
    // claimed, flipped when the request asked. See the module note.
    let reversed = [0, 1, 2].map(|slot| current.reversed[taken_by[slot]] ^ request.reverse[slot]);
    let result = Orientation::from_signed_axes(axes, reversed);
    debug_assert!(result.is_permutation(), "reorientation must permute");
    Ok(result)
}

fn free_axes(taken: &[bool; 3]) -> impl Iterator<Item = usize> + '_ {
    (0..3).filter(move |&i| !taken[i])
}

/// Record that new slot `slot` takes old local axis `old`.
fn claim(
    slot: usize,
    old: usize,
    from_old: &mut [Option<usize>; 3],
    taken: &mut [bool; 3],
) -> Result<(), OrientError> {
    if taken[old] {
        return Err(OrientError::Conflict {
            axis: Axis::from_index(old),
        });
    }
    taken[old] = true;
    from_old[slot] = Some(old);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: [u32; 3] = [10, 10, 10];

    #[test]
    fn keeping_everything_is_the_identity() {
        let got = reorient(Orientation::IDENTITY, CUBE, &Reorient::KEEP, None).unwrap();
        assert_eq!(got, Orientation::IDENTITY);
    }

    #[test]
    fn naming_one_axis_swaps_it_in_upstream_castle_case() {
        // MakeCastle.mid_crennels: split(Dimension.Z, [...], z=Dimension.X).
        // Upstream answer for an identity parent: (2, 1, 0).
        let got = reorient(
            Orientation::IDENTITY,
            CUBE,
            &Reorient::KEEP.z(AxisSpec::LocalX),
            Some(Axis::Z),
        )
        .unwrap();
        assert_eq!(
            got,
            Orientation {
                x: Axis::Z,
                y: Axis::Y,
                z: Axis::X,
                reversed: [false; 3],
            }
        );
    }

    #[test]
    fn largest_picks_the_long_axis_upstream_castle_root_case() {
        // MakeCastle.castle: reorient(x=Dimension.LARGEST, y=Dimension.Y) over a
        // box longer in world Z. Upstream answer: (2, 1, 0).
        let got = reorient(
            Orientation::IDENTITY,
            [20, 12, 30],
            &Reorient::KEEP.x(AxisSpec::Largest).y(AxisSpec::LocalY),
            None,
        )
        .unwrap();
        assert_eq!(
            got,
            Orientation {
                x: Axis::Z,
                y: Axis::Y,
                z: Axis::X,
                reversed: [false; 3],
            }
        );
        // ...and leaves the box alone when world X is already the longest.
        let got = reorient(
            Orientation::IDENTITY,
            [30, 12, 20],
            &Reorient::KEEP.x(AxisSpec::Largest).y(AxisSpec::LocalY),
            None,
        )
        .unwrap();
        assert_eq!(got, Orientation::IDENTITY);
    }

    #[test]
    fn smallest_and_largest_measure_the_world_box_not_the_local_names() {
        let rotated = Orientation {
            x: Axis::Z,
            y: Axis::Y,
            z: Axis::X,
            reversed: [false; 3],
        };
        let got = reorient(
            rotated,
            [4, 9, 30],
            &Reorient::KEEP.x(AxisSpec::Smallest).y(AxisSpec::LocalY),
            None,
        )
        .unwrap();
        assert_eq!(got.x, Axis::X, "world X is the 4-wide one");
        assert_eq!(got.y, Axis::Y);
        assert!(got.is_permutation());
    }

    #[test]
    fn split_axis_resolves_to_the_axis_being_cut() {
        let got = reorient(
            Orientation::IDENTITY,
            CUBE,
            &Reorient::KEEP.x(AxisSpec::SplitAxis),
            Some(Axis::Z),
        )
        .unwrap();
        assert_eq!(got.x, Axis::Z);
        assert!(got.is_permutation());
    }

    #[test]
    fn world_specs_are_read_through_the_parents_orientation() {
        let rotated = Orientation {
            x: Axis::Y,
            y: Axis::Z,
            z: Axis::X,
            reversed: [false; 3],
        };
        let got = reorient(rotated, CUBE, &Reorient::KEEP.y(AxisSpec::WorldY), None).unwrap();
        assert_eq!(got.y, Axis::Y);
        assert!(got.is_permutation());
    }

    #[test]
    fn every_request_yields_a_permutation() {
        let specs = [
            None,
            Some(AxisSpec::LocalX),
            Some(AxisSpec::LocalY),
            Some(AxisSpec::LocalZ),
            Some(AxisSpec::WorldX),
            Some(AxisSpec::Smallest),
            Some(AxisSpec::Largest),
        ];
        for x in specs {
            for y in specs {
                for z in specs {
                    let req = Reorient {
                        x,
                        y,
                        z,
                        reverse: [false; 3],
                    };
                    if let Ok(o) = reorient(Orientation::IDENTITY, [3, 7, 11], &req, None) {
                        assert!(o.is_permutation(), "{req:?} produced {o:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_reversal_follows_the_axis_its_slot_claimed() {
        // The subtle half: a slot's sign is the parent's sign for the axis that
        // *slot* took, not for the axis of the same name. Under a swap of X and
        // Z, a parent reversal on X must come out on the child's Z.
        let parent = Orientation {
            reversed: [true, false, false],
            ..Orientation::IDENTITY
        };
        let got = reorient(
            parent,
            CUBE,
            &Reorient::KEEP.z(AxisSpec::LocalX),
            Some(Axis::Z),
        )
        .unwrap();
        assert_eq!(got.axes(), [Axis::Z, Axis::Y, Axis::X]);
        assert_eq!(
            got.reversed,
            [false, false, true],
            "the parent's reversed X was claimed by the child's Z and the sign \
             stayed with the axis, not with the name"
        );
    }

    #[test]
    fn a_reversal_is_relative_to_the_parent_and_composes() {
        // Turning a turned piece is not turning at all.
        let once = reorient(Orientation::IDENTITY, CUBE, &Reorient::KEEP.turned(), None).unwrap();
        assert_eq!(once.reversed, [true, false, true]);
        assert!(once.is_rotation());
        let twice = reorient(once, CUBE, &Reorient::KEEP.turned(), None).unwrap();
        assert_eq!(twice, Orientation::IDENTITY);
    }

    #[test]
    fn a_reversal_never_changes_which_axis_is_chosen() {
        // `Smallest`/`Largest` measure world extents, which a sign cannot move,
        // so a request's reversals must leave the axis assignment untouched. A
        // choice that consulted the sign would make `turned()` mean different
        // things on different boxes.
        let size = [4, 9, 30];
        for parent in [
            Orientation::IDENTITY,
            Orientation {
                reversed: [true, true, true],
                ..Orientation::IDENTITY
            },
        ] {
            let plain = reorient(
                parent,
                size,
                &Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest),
                None,
            )
            .unwrap();
            let turned = reorient(
                parent,
                size,
                &Reorient::KEEP
                    .y(AxisSpec::WorldY)
                    .z(AxisSpec::Largest)
                    .turned(),
                None,
            )
            .unwrap();
            assert_eq!(plain.axes(), turned.axes());
            assert_eq!(
                turned.reversed,
                [!plain.reversed[0], plain.reversed[1], !plain.reversed[2]],
                "the half-turn flipped something other than X and Z"
            );
        }
    }

    #[test]
    fn naming_the_same_axis_twice_is_refused() {
        let req = Reorient::KEEP.x(AxisSpec::LocalZ).y(AxisSpec::LocalZ);
        assert_eq!(
            reorient(Orientation::IDENTITY, CUBE, &req, None),
            Err(OrientError::Conflict { axis: Axis::Z })
        );
    }
}
