//! Turning a size pattern into piece lengths.
//!
//! Ported from `make_split` in `SplitGrammar.py` (`yawgmoth/GDMC25`,
//! BSD-3-Clause — see `LICENSE-GDMC25`), with four corrections that upstream's
//! `print`-and-continue style left open. All four are stated as tests below:
//!
//! 1. **Overflow is an error, not a warning.** Upstream prints "Split exceeded
//!    size" and keeps going, emitting child boxes that stick out of their
//!    parent — i.e. writing blocks outside the region it was handed. Here the
//!    expansion stops with [`SplitError::Overflow`].
//! 2. **No zero-length tail under `repeat`.** Upstream clamps the piece that
//!    reaches the end but does not leave the pattern loop, so the remaining
//!    sizes of that pass emit degenerate boxes at the far edge. We stop at the
//!    clamp. Zero-volume boxes write nothing, so this cannot change any block.
//! 3. **A repeating pattern that consumes nothing is an error**
//!    ([`SplitError::ZeroStride`]) rather than an infinite loop.
//! 4. **Relative pieces cover the axis exactly.** Upstream computed
//!    `int(reltotal/relsizes)` and dropped what did not divide, so a pattern
//!    like `[rel, abs 2, rel]` over five blocks laid out 1, 2, 1 and left the
//!    fifth block written by nobody. No child owns that block, so no rule can
//!    fill, light or seal it, and every gate reads it as a cell outside the
//!    model rather than a hole in it. [`Rounding`] therefore only chooses where
//!    the indivisible remainder lands; it can no longer be discarded.

use crate::grammar::ir::Rounding;

/// A size pattern entry with its expressions already evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedSize {
    /// A fixed block count.
    Absolute(u32),
    /// A share weight over the leftover.
    Relative(u32),
}

/// Why a size pattern could not be laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitError {
    /// The absolute pieces alone are longer than the axis being split.
    Overflow {
        /// Total of the absolute pieces.
        absolute: u32,
        /// The available extent.
        extent: u32,
    },
    /// A repeating pattern whose pieces are all zero-length would never end.
    ZeroStride,
}

/// Lay a size pattern over `extent` blocks, returning the piece lengths in
/// order. With at least one relative piece — or with `repeat` — the lengths sum
/// to exactly `extent`. A pattern of absolutes alone is the author's own
/// arithmetic and sums to whatever they wrote, which may be less than `extent`.
pub fn make_split(
    extent: u32,
    sizes: &[ResolvedSize],
    rounding: Rounding,
    repeat: bool,
) -> Result<Vec<u32>, SplitError> {
    let pattern = resolve_pattern(extent, sizes, rounding)?;
    if !repeat {
        return Ok(pattern);
    }
    let stride: u32 = pattern.iter().sum();
    if stride == 0 {
        return Err(SplitError::ZeroStride);
    }
    let mut pieces = Vec::new();
    let mut used: u32 = 0;
    'tile: loop {
        for &p in &pattern {
            if used + p >= extent {
                pieces.push(extent - used);
                break 'tile;
            }
            pieces.push(p);
            used += p;
        }
    }
    Ok(pieces)
}

/// One pass of the pattern: absolute pieces keep their length, relative pieces
/// share what is left over.
fn resolve_pattern(
    extent: u32,
    sizes: &[ResolvedSize],
    rounding: Rounding,
) -> Result<Vec<u32>, SplitError> {
    let absolute: u32 = sizes
        .iter()
        .map(|s| match s {
            ResolvedSize::Absolute(n) => *n,
            ResolvedSize::Relative(_) => 0,
        })
        .sum();
    if absolute > extent {
        return Err(SplitError::Overflow { absolute, extent });
    }
    let weight_total: u32 = sizes
        .iter()
        .map(|s| match s {
            ResolvedSize::Absolute(_) => 0,
            ResolvedSize::Relative(w) => *w,
        })
        .sum();
    if weight_total == 0 {
        // Nothing to share the leftover: the pattern is exactly its absolutes,
        // and any remainder is simply not covered (upstream behaviour).
        return Ok(sizes
            .iter()
            .map(|s| match s {
                ResolvedSize::Absolute(n) => *n,
                ResolvedSize::Relative(_) => 0,
            })
            .collect());
    }

    let leftover = extent - absolute;
    let per_unit = leftover / weight_total;
    let remainder = leftover % weight_total;

    // The remainder is handed out one *weight unit* at a time over a contiguous
    // run of units; the rounding mode only chooses where that run starts. A
    // relative piece of weight w therefore grows by at most w.
    let run_start = match rounding {
        Rounding::Start => 0,
        Rounding::End => weight_total - remainder,
        Rounding::Middle => (weight_total - remainder) / 2,
    };
    let run_end = run_start.saturating_add(remainder);

    let mut unit = 0;
    let mut pieces = Vec::with_capacity(sizes.len());
    for size in sizes {
        match size {
            ResolvedSize::Absolute(n) => pieces.push(*n),
            ResolvedSize::Relative(w) => {
                let (lo, hi) = (unit, unit + w);
                let extra = hi.min(run_end).saturating_sub(lo.max(run_start));
                pieces.push(w * per_unit + extra);
                unit = hi;
            }
        }
    }
    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResolvedSize::{Absolute as A, Relative as R};

    fn split(extent: u32, sizes: &[ResolvedSize], rounding: Rounding) -> Vec<u32> {
        make_split(extent, sizes, rounding, false).unwrap()
    }

    #[test]
    fn absolute_pieces_keep_their_length() {
        assert_eq!(
            split(10, &[A(1), A(8), A(1)], Rounding::default()),
            [1, 8, 1]
        );
    }

    #[test]
    fn relative_pieces_share_the_leftover() {
        // The temple floorplan shape: two 1-block walls around an open middle.
        assert_eq!(
            split(11, &[A(1), R(1), A(1)], Rounding::default()),
            [1, 9, 1]
        );
        assert_eq!(
            split(12, &[R(1), R(2), R(1)], Rounding::default()),
            [3, 6, 3]
        );
    }

    /// The window bay that seamed the castle: a fixed feature between two
    /// shares, over an extent whose leftover is odd. Upstream laid out 1, 2, 1
    /// and left the fifth block written by nobody — a one-block slot from the
    /// floor to the string course, in a wall the courtyard looks straight at.
    #[test]
    fn a_feature_between_two_shares_covers_the_wall_it_stands_in() {
        for extent in [5u32, 7, 9, 11, 13] {
            let pieces = split(extent, &[R(1), A(2), R(1)], Rounding::default());
            assert_eq!(
                pieces.iter().sum::<u32>(),
                extent,
                "[rel, abs 2, rel] over {extent}"
            );
            assert_eq!(pieces[1], 2, "the fixed piece keeps its length");
        }
        assert_eq!(
            split(5, &[R(1), A(2), R(1)], Rounding::default()),
            [2, 2, 1]
        );
        assert_eq!(
            split(7, &[R(1), A(2), R(1)], Rounding::default()),
            [3, 2, 2]
        );
    }

    /// The invariant the seam was a counter-example to, over every mode and a
    /// range of extents: a pattern carrying a relative piece has no remainder
    /// left to lose.
    #[test]
    fn a_pattern_with_a_relative_piece_always_covers_its_extent() {
        let patterns: [&[ResolvedSize]; 6] = [
            &[R(1), A(2), R(1)],
            &[R(1), A(1), R(1)],
            &[A(1), R(1), A(1)],
            &[R(1), R(1), R(1)],
            &[R(3), R(1)],
            &[A(2), R(2), A(1), R(1)],
        ];
        let mut checked = 0usize;
        for pattern in patterns {
            let absolute: u32 = pattern
                .iter()
                .map(|s| match s {
                    A(n) => *n,
                    R(_) => 0,
                })
                .sum();
            for extent in absolute..=40 {
                for mode in [Rounding::Start, Rounding::End, Rounding::Middle] {
                    let pieces = split(extent, pattern, mode);
                    assert_eq!(
                        pieces.iter().sum::<u32>(),
                        extent,
                        "{pattern:?} over {extent} under {mode:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 714, "binding: patterns x extents x modes");
    }

    #[test]
    fn the_rounding_modes_put_the_spare_block_in_different_places() {
        for (mode, want) in [(Rounding::Start, [4, 1, 3]), (Rounding::End, [3, 1, 4])] {
            let pieces = split(8, &[R(1), A(1), R(1)], mode);
            assert_eq!(pieces, want, "{mode:?}");
            assert_eq!(pieces.iter().sum::<u32>(), 8, "{mode:?}");
        }
        // One spare block across three equal shares: each mode puts it
        // somewhere different, and every mode covers the extent exactly.
        assert_eq!(split(10, &[R(1), R(1), R(1)], Rounding::Start), [4, 3, 3]);
        assert_eq!(split(10, &[R(1), R(1), R(1)], Rounding::Middle), [3, 4, 3]);
        assert_eq!(split(10, &[R(1), R(1), R(1)], Rounding::End), [3, 3, 4]);
    }

    #[test]
    fn a_weight_grows_by_at_most_its_own_size() {
        // weight 3 + weight 1 over 9: per unit 2, remainder 1 -> Start gives the
        // spare block to the first piece only.
        assert_eq!(split(9, &[R(3), R(1)], Rounding::Start), [7, 2]);
        assert_eq!(split(9, &[R(3), R(1)], Rounding::End), [6, 3]);
    }

    #[test]
    fn repeat_tiles_the_pattern_and_clamps_the_tail() {
        // The castle crenellation: alternating block/gap across any wall run.
        assert_eq!(
            make_split(5, &[A(1), A(1)], Rounding::default(), true).unwrap(),
            [1, 1, 1, 1, 1],
            "no zero-length tail after the clamp"
        );
        assert_eq!(
            make_split(6, &[A(2), A(1)], Rounding::default(), true).unwrap(),
            [2, 1, 2, 1]
        );
        assert_eq!(
            make_split(7, &[A(2), A(1)], Rounding::default(), true).unwrap(),
            [2, 1, 2, 1, 1],
            "the clamped piece is short, and the pass stops there"
        );
    }

    #[test]
    fn overflow_is_an_error_not_a_box_outside_the_parent() {
        assert_eq!(
            make_split(3, &[A(2), A(2)], Rounding::default(), false),
            Err(SplitError::Overflow {
                absolute: 4,
                extent: 3
            })
        );
    }

    #[test]
    fn a_repeating_zero_length_pattern_is_refused() {
        assert_eq!(
            make_split(9, &[A(0), A(0)], Rounding::default(), true),
            Err(SplitError::ZeroStride)
        );
    }

    #[test]
    fn degenerate_extents_are_legal() {
        assert_eq!(split(0, &[R(1), R(1)], Rounding::default()), [0, 0]);
        assert_eq!(
            make_split(0, &[A(1)], Rounding::default(), false),
            Err(SplitError::Overflow {
                absolute: 1,
                extent: 0
            })
        );
    }
}
