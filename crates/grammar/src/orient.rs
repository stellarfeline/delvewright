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
//! The **reflection** half has no upstream: a request's `mirror` reverses the
//! direction of whichever old axis a new slot ends up taking, so it composes by
//! exclusive-or with the direction that axis already ran in. Reflecting twice is
//! therefore the identity, which is what makes a mirrored rule safe to nest.

use crate::geom::{Axis, Mirror, Orientation};
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

/// Complete `request` into a full frame.
///
/// `current` is the parent's frame, `size` its world-space extents (which
/// [`AxisSpec::Smallest`] / [`AxisSpec::Largest`] measure), and `split_axis` the
/// local axis being split, when the request appears on a split.
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
            let world = current.axis(Axis::from_index(old)).index();
            let better = match best {
                None => true,
                Some(b) => {
                    let bw = current.axis(Axis::from_index(b)).index();
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

    // One pass over the slots settles both halves of the frame: which world axis
    // the slot names, and which way it runs. The direction is the direction the
    // source axis already ran in, reversed when the request asks — never read
    // off the world, so nesting reflections cancels.
    let wanted = request.mirror.axes();
    let mut axes = [Axis::X; 3];
    let mut mirror = [false; 3];
    for slot in 0..3 {
        let old = Axis::from_index(from_old[slot].expect("every slot is assigned"));
        axes[slot] = current.axis(old);
        mirror[slot] = current.reversed(old) ^ wanted[slot];
    }
    let result = Orientation::from_axes(axes).mirrored(Mirror::from_axes(mirror));
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

// ---------------------------------------------------------------------------
// Which frames a scope could have had — the same computation, asked of every
// region at once
// ---------------------------------------------------------------------------

/// The 48 frames of the cube: six axis permutations, each with eight
/// reflection patterns.
///
/// Small enough to be a bitmask, which is what makes [`FrameSet`] a `Copy`
/// field a scope can carry without allocating once per split piece.
pub const FRAME_COUNT: usize = 48;

/// The six axis permutations, in a fixed order (ADR-0006: the index is part of
/// a report's determinism).
const PERMS: [[Axis; 3]; 6] = [
    [Axis::X, Axis::Y, Axis::Z],
    [Axis::X, Axis::Z, Axis::Y],
    [Axis::Y, Axis::X, Axis::Z],
    [Axis::Y, Axis::Z, Axis::X],
    [Axis::Z, Axis::X, Axis::Y],
    [Axis::Z, Axis::Y, Axis::X],
];

/// Every frame, by index.
pub fn all_frames() -> [Orientation; FRAME_COUNT] {
    let mut out = [Orientation::IDENTITY; FRAME_COUNT];
    for (p, axes) in PERMS.iter().enumerate() {
        for bits in 0..8usize {
            out[p * 8 + bits] = Orientation::from_axes(*axes).mirrored(Mirror::from_axes([
                bits & 1 != 0,
                bits & 2 != 0,
                bits & 4 != 0,
            ]));
        }
    }
    out
}

/// A frame's index in [`all_frames`].
pub fn frame_index(frame: Orientation) -> usize {
    let perm = PERMS
        .iter()
        .position(|p| *p == frame.axes())
        .expect("a frame is a permutation");
    let m = frame.mirror.axes();
    perm * 8 + usize::from(m[0]) + 2 * usize::from(m[1]) + 4 * usize::from(m[2])
}

/// **The frames one scope could stand in, over every region the program could
/// be expanded at.**
///
/// A frame is a fact about the region as much as about the program: `z: largest`
/// hands a scope the identity when the box is already longest along Z and a
/// quarter-turn when it is not. An expansion sees one of those and has no way to
/// tell which kind of fact it just observed — which is what let a fill of a
/// world-frame `facing` be called sound by a test that never ran. This set is
/// the missing half: a singleton `{identity}` means the frame is a constant of
/// the PROGRAM, and anything larger means this expansion saw one of several.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSet(u64);

impl FrameSet {
    /// The set holding exactly one frame.
    pub fn just(frame: Orientation) -> FrameSet {
        FrameSet(1 << frame_index(frame))
    }

    /// Whether this set holds only `frame` — the scope's frame is a constant.
    pub fn is_only(&self, frame: Orientation) -> bool {
        self.0 == 1 << frame_index(frame)
    }

    /// Whether the scope's frame is a constant of the program, whichever frame
    /// that is.
    pub fn is_singleton(&self) -> bool {
        self.0.count_ones() == 1
    }

    /// The frames in the set, ascending by index.
    pub fn iter(&self) -> impl Iterator<Item = Orientation> + '_ {
        let all = all_frames();
        (0..FRAME_COUNT).filter_map(move |i| (self.0 >> i & 1 == 1).then_some(all[i]))
    }
}

/// Box proportions covering every weak ordering of the three world extents —
/// which is all [`AxisSpec::Smallest`] and [`AxisSpec::Largest`] can read.
///
/// 27 triples over `{1, 2, 3}`, not a clever 13: the extremal passes compare
/// extents pairwise and break ties by world-axis order, and enumerating every
/// triple is the version whose correctness needs no argument.
fn representative_sizes() -> impl Iterator<Item = [u32; 3]> {
    (1..=3u32).flat_map(|x| (1..=3u32).flat_map(move |y| (1..=3u32).map(move |z| [x, y, z])))
}

/// The frames `request` can hand a child, given every frame the parent could
/// have had and every proportion the box could have.
///
/// A refused reorientation (the request names one parent axis twice) contributes
/// nothing: it cannot be a frame a fill stands in, because the expansion would
/// have stopped there.
pub fn reachable_frames(
    parents: FrameSet,
    request: &Reorient,
    split_axis: Option<Axis>,
) -> FrameSet {
    if request.is_keep() {
        return parents;
    }
    let mut out = 0u64;
    for parent in parents.iter() {
        for size in representative_sizes() {
            if let Ok(child) = reorient(parent, size, request, split_axis) {
                out |= 1 << frame_index(child);
            }
        }
    }
    FrameSet(out)
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
        assert_eq!(got, Orientation::from_axes([Axis::Z, Axis::Y, Axis::X]));
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
        assert_eq!(got, Orientation::from_axes([Axis::Z, Axis::Y, Axis::X]));
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
        let rotated = Orientation::from_axes([Axis::Z, Axis::Y, Axis::X]);
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
        let rotated = Orientation::from_axes([Axis::Y, Axis::Z, Axis::X]);
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
                        mirror: Mirror::NONE,
                    };
                    if let Ok(o) = reorient(Orientation::IDENTITY, [3, 7, 11], &req, None) {
                        assert!(o.is_permutation(), "{req:?} produced {o:?}");
                    }
                }
            }
        }
    }

    /// **Reflecting twice is the identity**, at any depth and through any
    /// rotation. This is the property that lets one rule stand at both sites of
    /// a mirror pair without knowing which side it is on.
    #[test]
    fn a_reflection_composes_by_cancelling_itself() {
        let flip_x = Reorient::KEEP.flip(Axis::X);
        let once = reorient(Orientation::IDENTITY, CUBE, &flip_x, None).unwrap();
        assert_eq!(once, Orientation::IDENTITY.mirrored(Mirror::of(Axis::X)));
        let twice = reorient(once, CUBE, &flip_x, None).unwrap();
        assert_eq!(twice, Orientation::IDENTITY, "two reflections cancel");
    }

    /// A reflection follows the axis it was asked of **through the rename in the
    /// same request**: it is relative to the source axis, not to the world.
    #[test]
    fn a_reflection_is_relative_to_the_axis_the_slot_ends_up_naming() {
        // New Z is the old X, and new Z runs backwards.
        let got = reorient(
            Orientation::IDENTITY,
            CUBE,
            &Reorient::KEEP.z(AxisSpec::LocalX).flip(Axis::Z),
            None,
        )
        .unwrap();
        assert_eq!(got.axis(Axis::Z), Axis::X);
        assert!(got.reversed(Axis::Z));
        assert!(!got.reversed(Axis::X), "the swapped-in axis keeps its sign");

        // A parent already reversed on the axis a slot inherits hands that sign
        // on untouched when the request says nothing.
        let parent = Orientation::IDENTITY.mirrored(Mirror::of(Axis::X));
        let got = reorient(parent, CUBE, &Reorient::KEEP.z(AxisSpec::LocalX), None).unwrap();
        assert!(
            got.reversed(Axis::Z),
            "the old X was reversed, so whatever names it is too"
        );
        assert!(!got.reversed(Axis::X));
    }

    /// Every request still yields a permutation, with the reflection carried
    /// alongside rather than folded into it.
    #[test]
    fn every_reflected_request_yields_a_permutation() {
        let specs = [None, Some(AxisSpec::LocalZ), Some(AxisSpec::Largest)];
        for x in specs {
            for y in specs {
                for z in specs {
                    for m in [
                        Mirror::NONE,
                        Mirror::of(Axis::X),
                        Mirror::of(Axis::Y).and(Axis::Z),
                        Mirror::from_axes([true, true, true]),
                    ] {
                        let req = Reorient { x, y, z, mirror: m };
                        if let Ok(o) = reorient(Orientation::IDENTITY, [3, 7, 11], &req, None) {
                            assert!(o.is_permutation(), "{req:?} produced {o:?}");
                        }
                    }
                }
            }
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

#[cfg(test)]
mod frame_set_tests {
    use super::*;

    /// The index is a bijection over the 48 frames — which is what licenses the
    /// bitmask, and the one thing a set of them could silently get wrong.
    #[test]
    fn every_frame_has_its_own_index() {
        let all = all_frames();
        let mut seen = [false; FRAME_COUNT];
        for (i, frame) in all.iter().enumerate() {
            assert!(frame.is_permutation(), "{frame:?}");
            assert_eq!(frame_index(*frame), i, "{frame:?}");
            assert!(!seen[i], "index {i} twice");
            seen[i] = true;
        }
        assert!(seen.iter().all(|s| *s));
    }

    /// **A request that reads a proportion is the one that widens the set**, and
    /// nothing else here does. This is the whole distinction `DW0742` rests on.
    #[test]
    fn only_an_extremal_spec_makes_the_frame_a_fact_about_the_region() {
        let start = FrameSet::just(Orientation::IDENTITY);

        // `keep` and an outright naming both leave the frame a constant.
        assert!(reachable_frames(start, &Reorient::KEEP, None).is_only(Orientation::IDENTITY));
        let named = Reorient::KEEP
            .x(AxisSpec::WorldX)
            .y(AxisSpec::WorldY)
            .z(AxisSpec::WorldZ);
        assert!(reachable_frames(start, &named, None).is_only(Orientation::IDENTITY));

        // A fixed turn is still a constant — a different one.
        let turned = Reorient::KEEP.z(AxisSpec::WorldX).x(AxisSpec::WorldZ);
        let after = reachable_frames(start, &turned, None);
        assert!(!after.is_only(Orientation::IDENTITY));
        assert_eq!(after.iter().count(), 1, "a fixed turn reads no proportion");

        // `largest` does not: the box decides, so both answers are reachable and
        // the identity is only one of them.
        let extremal = Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest);
        let after = reachable_frames(start, &extremal, None);
        assert!(after.iter().count() > 1, "{:?}", after);
        assert!(after.iter().any(|f| f == Orientation::IDENTITY));
        // ...and the request still pins what it names: every reachable frame
        // keeps the local Y on the world Y, running forward.
        for frame in after.iter() {
            assert_eq!(frame.axis(Axis::Y), Axis::Y, "{frame:?}");
            assert!(!frame.reversed(Axis::Y), "{frame:?}");
        }
    }

    /// The set is a superset of what the expander actually produces, which is
    /// the direction that keeps it honest: it may say "could not decide" where a
    /// region never arises, and never "decided" where one does.
    #[test]
    fn the_set_contains_every_frame_a_real_expansion_reaches() {
        let request = Reorient::KEEP.y(AxisSpec::WorldY).z(AxisSpec::Largest);
        let reachable = reachable_frames(FrameSet::just(Orientation::IDENTITY), &request, None);
        for size in [[5, 4, 9], [9, 4, 5], [4, 9, 5], [7, 7, 7], [1, 1, 1]] {
            let got = reorient(Orientation::IDENTITY, size, &request, None).unwrap();
            assert!(
                reachable.iter().any(|f| f == got),
                "{size:?} reaches {got:?}, which the set omits"
            );
        }
    }
}
