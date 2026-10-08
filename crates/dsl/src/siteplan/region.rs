//! The boxes against the kit grid, the region and each other: `DW0825`,
//! `DW0826`, `DW0827`, `DW0835` and the size and way classes (`DW0832`).

use super::*;

/// `DW0825`: every box's footprint is a multiple of the kit grid's quantum.
pub(super) fn grid(
    placed: &[Placed<'_>],
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) {
    let Some(grid) = table.grid(reads) else {
        return; // `Metrics::self_check` owns a table that defines no grid.
    };
    let q = grid.quantum;
    if q == 0 {
        return;
    }
    for p in placed {
        for (axis, name) in [(0usize, "x"), (1usize, "z")] {
            let e = p.plan.extent[axis].get();
            if !off_grid(e, q) {
                continue;
            }
            d.push(Diagnostic::error(
                DW_BOX_OFF_GRID,
                "site-plan",
                format!("/content/boxes/{}/extent/{axis}", p.index),
                format!(
                    "box for `{node}` is {e} blocks on {name}, and the kit grid's quantum is \
                     {q} — so it is not a multiple of it. Every box's footprint is a whole \
                     number of quanta on both horizontal axes, which is what lets a kit piece \
                     land in one without being cut. The nearest multiples are {lo} and {hi}.",
                    node = p.plan.node,
                    lo = e - e % q,
                    hi = e - e % q + q,
                ),
            ));
        }
    }
}

/// **`DW0825`'s own test, for one footprint on one axis.** One function so that
/// a verdict computed from a box can ask the SAME question the refusal asked,
/// rather than re-deriving the kit-grid rule beside it.
pub(super) fn off_grid(extent: u32, quantum: u32) -> bool {
    quantum != 0 && !extent.is_multiple_of(quantum)
}

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
            let axis = match *name {
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

/// `DW0832`: a box is built to its place's class — **either kind** (spec-0053
/// §3).
///
/// # The way branch, and why its third demand is structural
///
/// A size class bounds both horizontal extents and this is the one place that
/// becomes geometry. A way class bounds only the **cross-section**, which is the
/// box's *shorter* horizontal extent — the axis a body feels — and then demands
/// that the **run**, the longer extent, strictly EXCEED the class's
/// `max_width`.
///
/// That third demand is the elongation, and it is deliberately derived from the
/// class's own widest cross-section rather than seeded as a constant, because it
/// is exactly what a room cannot supply. A square box can never satisfy it: its
/// run equals its width, and one number cannot both be `<= max_width` and exceed
/// it. So "declare a room a way to escape the size ladder" is refused **by the
/// object's own shape** rather than by a rule the author could satisfy by
/// choosing differently — the property `CLAUDE.md` demands of an opt-out, since
/// the defect this branch exists to catch is structurally incapable of
/// producing its proof.
///
/// There is no maximum run and there is not going to be one: a route's length is
/// per-campaign geometry, never a standard (spec-0053 §7).
pub(super) fn size_classes(placed: &[Placed<'_>], d: &mut Vec<Diagnostic>) {
    for p in placed {
        let Some(class) = p.class else {
            continue; // `DW0812` refused the name.
        };
        let (kind, mut bad) = match class {
            PlaceClass::Size(sc) => {
                let mut bad: Vec<String> = Vec::new();
                for (axis, name) in [(0usize, "x"), (1, "z")] {
                    let e = p.plan.extent[axis].get();
                    if e < sc.min_footprint[axis] || e > sc.max_footprint[axis] {
                        bad.push(format!(
                            "{e} blocks on {name}, outside the class's {}..{}",
                            sc.min_footprint[axis], sc.max_footprint[axis]
                        ));
                    }
                }
                ("size", bad)
            }
            PlaceClass::Way(w) => {
                let mut bad: Vec<String> = Vec::new();
                let (dx, dz) = (p.plan.extent[0].get(), p.plan.extent[1].get());
                let (width, run) = (dx.min(dz), dx.max(dz));
                let axis = if dx <= dz { "x" } else { "z" };
                if width < w.min_width || width > w.max_width {
                    bad.push(format!(
                        "a cross-section of {width} blocks (its shorter extent, on {axis}), \
                         outside the class's {}..{}",
                        w.min_width, w.max_width
                    ));
                }
                if run <= w.max_width {
                    bad.push(format!(
                        "a run of {run} blocks, which does not exceed the class's widest \
                         cross-section of {}. A way is a place that is longer than it is wide \
                         by kind and not by margin, so this box is a room — give it a \
                         `size_class` instead, or make it longer",
                        w.max_width
                    ));
                }
                ("way", bad)
            }
        };
        if let Ceiling::Clearance(c) = p.plan.ceiling
            && c.get() < class.min_clearance()
        {
            bad.push(format!(
                "{c} cells of headroom, under the class's minimum of {}",
                class.min_clearance()
            ));
        }
        if bad.is_empty() {
            continue;
        }
        d.push(Diagnostic::error(
            DW_SIZE_CLASS,
            "site-plan",
            format!("/content/boxes/{}", p.index),
            format!(
                "the box for `{node}` is not built to its declared {kind} class: {bad}. The \
                 class is the vocabulary the graph chose this place's scale in, and this is the \
                 one place it becomes geometry — either build the box to it, or declare the \
                 place a different class in the layout graph and say so there.",
                node = p.plan.node,
                bad = bad.join("; "),
            ),
        ));
    }
}
