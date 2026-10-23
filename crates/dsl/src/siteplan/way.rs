//! **A way is whole** (spec-0120): the site plan's half of the declared clear
//! width — a place that declares itself a way (`boxes[].way`).
//!
//! The engine never measures the bytes to learn how wide a way is: a seam's
//! `opening` is its clear width, a contact's span is the author's, and a way
//! place states its `clear` here. What the compiler's clear-width check
//! (`DW1057`) measures is how much of that declared width is free, along the
//! whole way and in every state its cells can take. Nothing excuses a column
//! that is not: a thing meant to stand beside a way stands outside its
//! declared width — on a way place, in the verge `clear` does not claim.
//!
//! This module holds the surface and its validation-tier refusal only.

use super::*;

/// **A place that is a way** (spec-0120 §4.1) — a road, a street, a bridge,
/// a causeway: its run travels along `axis` and is `clear` columns wide across
/// it, the columns starting `at` cells from the box's low corner on the cross
/// axis. The run is judged column by column at every station along its whole
/// length, as a stair's is. A box without `way` is a room, and the clear-width
/// check never enters it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoxWay {
    /// The horizontal axis the run travels along: `x` or `z`.
    pub axis: PlanAxis,
    /// How many columns across the run are its clear width, in cells — the
    /// author's number, never read from the box's extent.
    pub clear: NonZeroU32,
    /// Where the first clear column stands, in cells from the box's low corner
    /// on the cross axis. Omitted, the columns are centred:
    /// `max(0, (extent − clear) div 2)`, exactly as a seam's `at` defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<u32>,
}

/// The first clear column of a way place on its cross axis, from the box's low
/// corner: the declared `at`, or centred.
#[must_use]
pub fn way_at(way: &BoxWay, cross_extent: i64) -> i64 {
    match way.at {
        Some(a) => i64::from(a),
        None => ((cross_extent - i64::from(way.clear.get())) / 2).max(0),
    }
}

/// `DW0826`, widened by one shape (spec-0120 §4.1): a way place that does not
/// fit its place.
pub(super) fn ways(plan: &SitePlanContent, d: &mut Vec<Diagnostic>) {
    for (i, pb) in plan.boxes.iter().enumerate() {
        let path = |rest: &str| format!("/content/boxes/{i}{rest}");
        let Some(way) = pb.way.as_ref() else {
            continue;
        };
        let axis = match way.axis {
            PlanAxis::X => 0usize,
            PlanAxis::Z => 1usize,
        };
        let cross_extent = i64::from(pb.extent[1 - axis].get());
        let clear = i64::from(way.clear.get());
        let at = way_at(way, cross_extent);
        if at + clear > cross_extent {
            d.push(Diagnostic::error(
                DW_BOX_LEAVES_REGION,
                "site-plan",
                path("/way"),
                format!(
                    "the way `{n}` declares {clear} clear column(s) starting {at} cell(s) from the \
                     box's low corner across its run, and the box is {cross_extent} cell(s) wide \
                     on that axis: the way leaves its place. A way is no wider than the place it \
                     runs through — narrow `clear`, move `at`, or widen the box.",
                    n = pb.node,
                ),
            ));
        }
    }
}

/// How many columns a seam's way has across its travel — the denominator its
/// clear width is judged against. On a wall, the opening's (or the
/// span's) extent along the face's horizontal axis; through a floor, a stair's
/// widest horizontal extent, and any other crossing's hole cells.
#[must_use]
pub fn seam_width(s: &PlacedSeam) -> i64 {
    let (lo, hi) = s.opening;
    let ext = |a: usize| hi[a] - lo[a] + 1;
    if s.normal_axis == 1 {
        if s.stair_in.is_some() {
            ext(0).max(ext(2))
        } else {
            ext(0) * ext(2)
        }
    } else {
        ext(2 - s.normal_axis)
    }
}
