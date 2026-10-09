//! The boxes against the region and each other: `DW0826`, `DW0827` and
//! `DW0835`.

use super::*;

/// `DW0826`: nothing the plan places leaves the region.
///
/// **The region is the brief's number flowing down.** A box is never grounds to
/// grow it: the prescription in the message is to move or shrink the box, or to
/// change the brief's fact and re-derive *visibly* — never to let the extent be
/// whatever the parts added up to, which is the failure this whole stage was
/// bought to end.
///
/// Whole-owned volumes answer to it too. A `massif` outside the region is the
/// whole owning mass beyond its own declared extent, which is the same
/// extent-flows-up defect arriving through the back door.
///
/// **One region number is one finding, however many boxes stand outside it**
/// (`crate::diagnostic`'s "one cause, one line"). The region is a single fact
/// handed down by the brief, so a region a course too short puts *every* box
/// over the same edge and each per-box refusal prints the same two numbers with
/// a different name in front: measured on a 24-place campaign, shortening the
/// region by five courses printed 24 identical paragraphs. When more than one
/// thing leaves it, this states the count, names every offender with its own
/// overrun, and prescribes once. A single offender still gets its own line
/// exactly as before — the fold is reachable only where the copies would have
/// been.
pub(super) fn region(plan: &SitePlanContent, placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    let r = &plan.region;
    let spans = [region_span(r, 0), region_span(r, 1), region_span(r, 2)];
    // How the region reads once, for the folded arms: the per-item clause
    // repeats it, and repeating it N times is most of what made N copies
    // unreadable.
    let region_text = format!(
        "x {}..{}, y {}..{}, z {}..{}",
        spans[0].0, spans[0].1, spans[1].0, spans[1].1, spans[2].0, spans[2].1
    );

    // ---- boxes ----
    let mut boxes_out: Vec<Overrun<'_>> = Vec::new();
    for p in placed {
        let mut bad: Vec<(&'static str, i64, i64)> = Vec::new();
        if !within((p.x0(), p.x1()), spans[0]) {
            bad.push(("x", p.x0(), p.x1()));
        }
        if let Some(y) = p.y_span()
            && !within(y, spans[1])
        {
            bad.push(("y", y.0, y.1));
        }
        if !within((p.z0(), p.z1()), spans[2]) {
            bad.push(("z", p.z0(), p.z1()));
        }
        if !bad.is_empty() {
            boxes_out.push(Overrun {
                index: p.index,
                name: p.plan.node.0.as_str(),
                axes: bad,
                by: p.by.describe(),
            });
        }
    }
    if boxes_out.len() == 1 {
        let o = &boxes_out[0];
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            format!("/content/boxes/{}", o.index),
            format!(
                "box for `{node}` leaves the region: {bad}. It stands where it stands because it \
                 is {by}. The region is the whole map's extent, and it comes from the brief — a \
                 box is never grounds to grow it. Move the box (its pin, or the offsets of the \
                 seam that placed it), shrink it, or change the brief's fact and re-derive the \
                 region so the change is visible in the document that owns it.",
                by = o.by,
                node = o.name,
                bad = against_region(&o.axes, &spans),
            ),
        ));
    } else if boxes_out.len() > 1 {
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            "/content/boxes",
            format!(
                "{n} of the {total} box(es) this plan places leave the region, which is \
                 {region_text}: {list}. The region is the whole map's extent, and it comes from \
                 the brief — a box is never grounds to grow it. One region is the cause of all \
                 {n} of these, which is why they are one line and not {n}: move or shrink the \
                 boxes, or change the brief's fact and re-derive the region so the change is \
                 visible in the document that owns it.",
                n = boxes_out.len(),
                total = placed.len(),
                list = named_overruns(&boxes_out),
            ),
        ));
    }

    // ---- whole-owned volumes ----
    let mut volumes_out: Vec<Overrun<'_>> = Vec::new();
    for (i, v) in plan.volumes.iter().enumerate() {
        let vmax = v.region.max();
        let mut bad: Vec<(&'static str, i64, i64)> = Vec::new();
        for (axis, name) in [(0usize, "x"), (1, "y"), (2, "z")] {
            if !within((v.region.min[axis], vmax[axis]), spans[axis]) {
                bad.push((name, v.region.min[axis], vmax[axis]));
            }
        }
        if !bad.is_empty() {
            volumes_out.push(Overrun {
                by: String::new(),
                index: i,
                name: v.id.0.as_str(),
                axes: bad,
            });
        }
    }
    if volumes_out.len() == 1 {
        let o = &volumes_out[0];
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            format!("/content/volumes/{}", o.index),
            format!(
                "whole-owned volume `{id}` leaves the region: {bad}. The region is the whole's \
                 own extent; mass outside it is the whole growing to fit what was put in it, \
                 which is the direction this stage exists to forbid.",
                id = o.name,
                bad = against_region(&o.axes, &spans),
            ),
        ));
    } else if volumes_out.len() > 1 {
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            "/content/volumes",
            format!(
                "{n} of the {total} whole-owned volume(s) leave the region, which is \
                 {region_text}: {list}. The region is the whole's own extent; mass outside it is \
                 the whole growing to fit what was put in it, which is the direction this stage \
                 exists to forbid. One region is the cause of all {n} of these, which is why \
                 they are one line and not {n}.",
                n = volumes_out.len(),
                total = plan.volumes.len(),
                list = named_overruns(&volumes_out),
            ),
        ));
    }
}

/// One offender's overrun, spelled against the region on each axis it leaves —
/// the wording a single finding carries, where the region's own numbers are
/// stated beside the box's because there is only one line to read them in.
fn against_region(bad: &[(&'static str, i64, i64)], spans: &[(i64, i64); 3]) -> String {
    bad.iter()
        .map(|(name, lo, hi)| {
            let axis = match name.rsplit(' ').next().unwrap_or(name) {
                "x" => 0,
                "y" => 1,
                _ => 2,
            };
            format!(
                "{name} {lo}..{hi} against the region's {}..{}",
                spans[axis].0, spans[axis].1
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// One thing the plan places that stands outside the region, and where.
///
/// A named struct rather than a tuple because both arms read it: the single
/// finding addresses its own `index`, the folded one prints `name` and `axes`,
/// and boxes and volumes differ in nothing else.
struct Overrun<'a> {
    /// Position in its own array — the path a single finding is addressed at.
    index: usize,
    /// The id an author reads.
    name: &'a str,
    /// Each axis it leaves, with its own inclusive span on that axis.
    axes: Vec<(&'static str, i64, i64)>,
    /// How its corner was obtained, in the words a refusal uses.
    by: String,
}

/// The offender list a folded finding carries: every name with its own overrun,
/// and no repeat of the region — the folded message states that once.
fn named_overruns(items: &[Overrun<'_>]) -> String {
    items
        .iter()
        .map(|o| {
            format!(
                "`{}` ({})",
                o.name,
                o.axes
                    .iter()
                    .map(|(ax, lo, hi)| format!("{ax} {lo}..{hi}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `DW0988`: a roof the plan has no room for (spec-0098 §3, §7).
///
/// Two shapes of one claim, both read off the plan before any geometry:
/// `roof` on a box whose ceiling is `open` — an open place has no lid to put a
/// roof on; and a course of the roof proper (over the shell footprint, above
/// the lid) lying in another place's play space or floor course — the stacked
/// case. Eaves are not this refusal: an eave stops at a neighbour's wall.
pub(super) fn roofs(placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    for p in placed {
        let Some(roof) = p.plan.roof else { continue };
        if matches!(p.plan.ceiling, Ceiling::Open) {
            d.push(Diagnostic::error(
                DW_ROOF_NO_ROOM,
                "site-plan",
                format!("/content/boxes/{}/roof", p.index),
                format!(
                    "the box for `{node}` declares a roof ({c} course(s), eaves {e}) and its \
                     ceiling is `open`. A sky-open place has no lid to put a roof on: it claims \
                     its ground and its headroom and nothing above that. Remove the roof, or \
                     give the place a `clearance` ceiling so the whole can reserve a roof over \
                     it.",
                    node = p.plan.node,
                    c = roof.courses,
                    e = roof.eaves,
                ),
            ));
            continue;
        }
        let Some((_, top)) = p.y_span() else { continue };
        if roof.courses == 0 {
            continue;
        }
        // The roof proper: the shell footprint, from one above the lid to the
        // top course. The lid itself is the shell's and a stacked upper box's
        // floor course by rule 3a; the courses above it are what need room.
        let (rlo, rhi) = ((top + 2), (top + 1 + i64::from(roof.courses)));
        let (sx0, sx1, sz0, sz1) = (p.x0() - 1, p.x1() + 1, p.z0() - 1, p.z1() + 1);
        for q in placed {
            if q.index == p.index {
                continue;
            }
            // The neighbour's play space (its footprint, floor to top) and its
            // floor course (its shell footprint, one course under its floor).
            let q_top = q.y_span().map_or(q.floor, |(_, t)| t);
            let in_space = overlap((sx0, sx1), (q.x0(), q.x1()))
                .zip(overlap((sz0, sz1), (q.z0(), q.z1())))
                .zip(overlap((rlo, rhi), (q.floor, q_top)));
            let in_floor = overlap((sx0, sx1), (q.x0() - 1, q.x1() + 1))
                .zip(overlap((sz0, sz1), (q.z0() - 1, q.z1() + 1)))
                .zip(overlap((rlo, rhi), (q.floor - 1, q.floor - 1)));
            let Some(((x, z), y)) = in_space.or(in_floor) else {
                continue;
            };
            d.push(Diagnostic::error(
                DW_ROOF_NO_ROOM,
                "site-plan",
                format!("/content/boxes/{}/roof", p.index),
                format!(
                    "the roof over `{a}` rises into `{b}`: its courses y {ry0}..{ry1} meet \
                     `{b}`'s floor and play space at x {x0}..{x1}, y {y0}..{y1}, z {z0}..{z1}. \
                     A roof is the whole reserving the volume a building will take, and that \
                     volume cannot be a place somebody stands in. Declare fewer courses, raise \
                     `{b}`, or make the two one place whose piece carries both.",
                    a = p.plan.node,
                    b = q.plan.node,
                    ry0 = rlo,
                    ry1 = rhi,
                    x0 = x.0,
                    x1 = x.1,
                    y0 = y.0,
                    y1 = y.1,
                    z0 = z.0,
                    z1 = z.1,
                ),
            ));
        }
    }
}

/// `DW0827`: the boxes are disjoint.
///
/// Shared **faces** are the only permitted contact, because a seam needs one —
/// and a shared face is a one-cell gap, not a touch (see [`Placed`]). Two boxes
/// whose play spaces meet are two authorities over one cell, which the
/// derivation would have to arbitrate and must never be asked to.
pub(super) fn disjoint(placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    for (a_i, a) in placed.iter().enumerate() {
        for b in &placed[a_i + 1..] {
            let (Some(x), Some(z)) = (
                overlap((a.x0(), a.x1()), (b.x0(), b.x1())),
                overlap((a.z0(), a.z1()), (b.z0(), b.z1())),
            ) else {
                continue;
            };
            let y = match (a.y_span(), b.y_span()) {
                (Some(ya), Some(yb)) => match overlap(ya, yb) {
                    Some(y) => y,
                    None => continue,
                },
                // One of them is sky-open with an unresolved class; `DW0812`
                // owns that name, and the footprints alone are enough to say
                // the two places stand in each other.
                _ => (a.floor.min(b.floor), a.floor.max(b.floor)),
            };
            d.push(Diagnostic::error(
                DW_BOXES_OVERLAP,
                "site-plan",
                format!("/content/boxes/{}", b.index),
                format!(
                    "the boxes for `{a_n}` and `{b_n}` overlap, sharing x {x0}..{x1}, y \
                     {y0}..{y1}, z {z0}..{z1}. Two places may share a FACE — that is what a seam \
                     is cut through — but never a cell: overlapping boxes are two owners for one \
                     block, and the derivation would have to pick between them with no rule to \
                     pick by. Boxes that connect sit one cell apart, and the cell between them \
                     is the wall they have in common.",
                    a_n = a.plan.node,
                    b_n = b.plan.node,
                    x0 = x.0,
                    x1 = x.1,
                    y0 = y.0,
                    y1 = y.1,
                    z0 = z.0,
                    z1 = z.1,
                ),
            ));
        }
    }
}

/// `DW0835`: the whole's mass stands beside, under and over places — never
/// inside one.
pub(super) fn volumes_outside_boxes(
    plan: &SitePlanContent,
    placed: &[Placed<'_>],
    d: &mut Vec<Diagnostic>,
) {
    for (i, v) in plan.volumes.iter().enumerate() {
        let vmax = v.region.max();
        for p in placed {
            let (Some(x), Some(z)) = (
                overlap((v.region.min[0], vmax[0]), (p.x0(), p.x1())),
                overlap((v.region.min[2], vmax[2]), (p.z0(), p.z1())),
            ) else {
                continue;
            };
            let Some(py) = p.y_span() else { continue };
            let Some(y) = overlap((v.region.min[1], vmax[1]), py) else {
                continue;
            };
            d.push(Diagnostic::error(
                DW_VOLUME_IN_BOX,
                "site-plan",
                format!("/content/volumes/{i}"),
                format!(
                    "whole-owned volume `{id}` ({role}) enters the box for `{node}`, sharing x \
                     {x0}..{x1}, y {y0}..{y1}, z {z0}..{z1}. The whole's mass may stand beside a \
                     place, under it and over it; inside it, the volume and the place are two \
                     authorities writing one cell, and the derivation must never be asked to \
                     arbitrate that. Pull the volume back to the place's face, or move the \
                     place.",
                    id = v.id,
                    role = v.role.as_str(),
                    node = p.plan.node,
                    x0 = x.0,
                    x1 = x.1,
                    y0 = y.0,
                    y1 = y.1,
                    z0 = z.0,
                    z1 = z.1,
                ),
            ));
        }
    }
}
