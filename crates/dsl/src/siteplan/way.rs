//! **A way is whole** (spec-0120): the site plan's half of the declared clear
//! width — a place that declares itself a way (`boxes[].way`), and the
//! narrowings an author means, declared on the way they stand in
//! (`boxes[].narrowed[]`, `seams[].narrowed[]`).
//!
//! The engine never measures the bytes to learn how wide a way is: a seam's
//! `opening` is its clear width, a contact's span is the author's, and a way
//! place states its `clear` here. What the compiler's clear-width check
//! (`DW1057`, `DW1058`) measures is how much of that declared width is free,
//! along the whole way and in every state its cells can take.
//!
//! This module holds the surface and its validation-tier refusals only. The
//! three narrowing entries share their `block` and `what` fields and the one
//! rule that holds them ([`narrowing_text`]); they are three structs rather
//! than one flattened type because each document refuses an unknown field
//! (`deny_unknown_fields`), which serde does not combine with a flattened
//! field.

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

/// **A narrowing a way place means** (spec-0120 §4.1): a structure the author
/// stands inside the place's declared clear width on purpose — a lamp post on
/// a causeway, a market cross in a street — named where it stands in the way's
/// own frame, by its block and by what it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceNarrowing {
    /// The station, in cells from the box's low corner along the way's `axis`.
    pub along: u32,
    /// The inclusive span of columns it takes, counted from the way's first
    /// clear column (`way.at`).
    pub across: [u32; 2],
    /// The block that stands there, with its state as a piece's palette writes
    /// it (`minecraft:stone_brick_wall`). A state the declaration names must
    /// match; one it omits is not compared.
    pub block: String,
    /// What it is, in a few words — the creative statement, never
    /// player-facing.
    pub what: String,
}

/// **Where along a seam's way a narrowing stands** (spec-0120 §4.1).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum NarrowSide {
    /// The approach in the connection's `a` place (on a stair, the host's
    /// approach is the run's foot).
    A,
    /// The approach in the connection's `b` place.
    B,
    /// A contact's span. Refused on a portal, whose plane is `DW0836`'s.
    Plane,
    /// A course of a stair's run, named by `course`.
    Run,
}

impl NarrowSide {
    /// The word the document writes.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            NarrowSide::A => "a",
            NarrowSide::B => "b",
            NarrowSide::Plane => "plane",
            NarrowSide::Run => "run",
        }
    }
}

/// **A narrowing a seam's way means** (spec-0120 §4.1): a newel post at the
/// foot of a broad stair, a fallen voussoir across half an arch — named where
/// it stands in the way's own frame, by its block and by what it is. The
/// declaration is confirmed cell for cell against the bytes (`DW1058`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeamNarrowing {
    /// Where along the travel: `a`, `b`, `plane` (a contact's span) or `run`
    /// (a stair's course, with `course`).
    pub side: NarrowSide,
    /// The inclusive span of columns it takes, counted from the opening's low
    /// corner across the way: on a wall, along the face's horizontal axis (the
    /// `at` axis); on a stair, across its treads; through a floor otherwise,
    /// the hole's cells numbered along `x`, then `z`.
    pub across: [u32; 2],
    /// The course of a stair's run, counted from the top as the run counts it
    /// (course 0 is the one a body steps off). Required with `side: run` and
    /// refused with any other side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub course: Option<u32>,
    /// The block that stands there, with its state as a piece's palette writes
    /// it. A state the declaration names must match; one it omits is not
    /// compared.
    pub block: String,
    /// What it is, in a few words — the creative statement, never
    /// player-facing.
    pub what: String,
}

/// **The one rule a narrowing's `block` and `what` answer to**, wherever the
/// entry is declared — a seam, a way place or a piece's contract edge: the
/// block is a block the pinned registry knows, and `what` says something.
/// `Err` carries the sentence a refusal prints.
///
/// # Errors
///
/// The reason the pair is refused.
pub fn narrowing_text(block: &str, what: &str) -> Result<(), String> {
    if what.trim().is_empty() {
        return Err(
            "its `what` is empty — a narrowing is excused only by the author saying what the \
             structure is, which is the one word the engine cannot write"
                .to_string(),
        );
    }
    let id = block.split('[').next().unwrap_or(block);
    if crate::blocks::BlockRegistry::v1_21_11()
        .default_state(id)
        .is_none()
    {
        return Err(format!(
            "its `block` `{block}` names no block of the pinned registry, so nothing in the \
             world can ever be confirmed as it"
        ));
    }
    Ok(())
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

/// `DW0826` and `DW0824`, widened by one shape each (spec-0120 §4.1): a way
/// place that does not fit its place, a narrowing a place or a seam declares
/// where its way has no such column, side or course, or one whose `block` or
/// `what` the rule refuses.
pub(super) fn ways(
    c: &Campaign,
    plan: &SitePlanContent,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) {
    let boxes = placed_boxes(c, reads);
    for (i, pb) in plan.boxes.iter().enumerate() {
        let path = |rest: &str| format!("/content/boxes/{i}{rest}");
        let Some(way) = pb.way.as_ref() else {
            if !pb.narrowed.is_empty() {
                d.push(Diagnostic::error(
                    DW_BOX_LEAVES_REGION,
                    "site-plan",
                    path("/narrowed"),
                    format!(
                        "the box for `{n}` declares {k} narrowing(s) and no `way`. A narrowing \
                         excuses a structure standing inside a way's declared clear width, and a \
                         box without `way` is a room: it has no width to narrow, and the \
                         clear-width check never enters it. Declare the place a way \
                         (`way: {{axis, clear}}`), or remove `narrowed`.",
                        n = pb.node,
                        k = pb.narrowed.len(),
                    ),
                ));
            }
            continue;
        };
        let axis = match way.axis {
            PlanAxis::X => 0usize,
            PlanAxis::Z => 1usize,
        };
        let along_extent = i64::from(pb.extent[axis].get());
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
            continue;
        }
        for (k, e) in pb.narrowed.iter().enumerate() {
            let at_path = path(&format!("/narrowed/{k}"));
            let mut fault = |why: String| {
                d.push(Diagnostic::error(
                    DW_BOX_LEAVES_REGION,
                    "site-plan",
                    at_path.clone(),
                    format!(
                        "the narrowing `{what}` declared on the way `{n}` is refused: {why}.",
                        what = e.what,
                        n = pb.node,
                    ),
                ));
            };
            if i64::from(e.along) >= along_extent {
                fault(format!(
                    "it stands {along} cell(s) along a run {along_extent} cell(s) long — outside \
                     the place",
                    along = e.along
                ));
            }
            if e.across[0] > e.across[1] || i64::from(e.across[1]) >= clear {
                fault(format!(
                    "it spans columns {}..{} of a way {clear} column(s) wide; `across` is an \
                     inclusive span inside `0..{}`",
                    e.across[0],
                    e.across[1],
                    clear - 1
                ));
            }
            if let Err(why) = narrowing_text(&e.block, &e.what) {
                fault(why);
            }
        }
    }

    let seams = placed_seams(c, &boxes, reads);
    let graph_edges: BTreeMap<&str, &str> = c
        .layout_graph
        .as_ref()
        .map(|g| {
            g.content
                .edges
                .iter()
                .map(|e| (e.id().0.as_str(), e.class()))
                .collect()
        })
        .unwrap_or_default();
    for (i, s) in plan.seams.iter().enumerate() {
        if s.narrowed.is_empty() {
            continue;
        }
        let class = graph_edges.get(s.edge.0.as_str()).copied().unwrap_or("");
        let placed = seams.iter().find(|p| p.edge == s.edge);
        for (k, e) in s.narrowed.iter().enumerate() {
            let at_path = format!("/content/seams/{i}/narrowed/{k}");
            let mut fault = |why: String| {
                d.push(Diagnostic::error(
                    DW_PLAN_AGREEMENT,
                    "site-plan",
                    at_path.clone(),
                    format!(
                        "the narrowing `{what}` declared on the seam for `{edge}` is refused: \
                         {why}.",
                        what = e.what,
                        edge = s.edge,
                    ),
                ));
            };
            if class == "vision" || class == "carry" {
                fault(format!(
                    "`{edge}` is a `{class}` connection, which is not a way a body walks",
                    edge = s.edge
                ));
                continue;
            }
            let contact = s.contact.is_some();
            match e.side {
                NarrowSide::Plane if !contact => fault(
                    "`side: plane` names a portal's opening, and a portal's plane is \
                     `DW0836`'s: every allocated cell of it is passable, so nothing stands \
                     there to excuse"
                        .to_string(),
                ),
                NarrowSide::Run if class != "stair" => fault(format!(
                    "`side: run` names a stair's course, and `{edge}` is a `{class}`",
                    edge = s.edge
                )),
                NarrowSide::Run if e.course.is_none() => fault(
                    "`side: run` names a course of the stair, and `course` is not written"
                        .to_string(),
                ),
                _ => {}
            }
            if e.side != NarrowSide::Run && e.course.is_some() {
                fault(format!(
                    "`course` names a course of a stair's run, and the entry stands on side `{}`",
                    e.side.as_str()
                ));
            }
            if let Some(p) = placed {
                let width = seam_width(p);
                if e.across[0] > e.across[1] || i64::from(e.across[1]) >= width {
                    fault(format!(
                        "it spans columns {}..{} of a way {width} column(s) wide; `across` is an \
                         inclusive span inside `0..{}`",
                        e.across[0],
                        e.across[1],
                        width - 1
                    ));
                }
            }
            if let Err(why) = narrowing_text(&e.block, &e.what) {
                fault(why);
            }
        }
    }
}

/// How many columns a seam's way has across its travel — the denominator a
/// narrowing's `across` is held inside. On a wall, the opening's (or the
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
