//! The checks spec-0098 adds to the plan: the declared `fill` and its terrain
//! (`DW0112`, `DW0193`, `DW0826`), every claim inside the region (`DW0826`),
//! and every cell two claims share awarded by a rule (`DW0827`, widened).

use super::*;

/// `DW0193`/`DW0112`/`DW0826` over the plan's `fill` and its volumes' blocks.
pub(super) fn fill(c: &Campaign, plan: &SitePlanContent, d: &mut Vec<Diagnostic>) {
    let blocks = crate::blocks::BlockRegistry::v1_21_11();
    let block = |state: &str, path: String, what: &str, d: &mut Vec<Diagnostic>| {
        if let Err(e) = blocks.validate_state_string(state) {
            d.push(Diagnostic::error(
                crate::codes::BLOCK_UNKNOWN,
                "site-plan",
                path,
                format!(
                    "{what} is `{state}`, which is not a block state of Minecraft Java 1.21.11 \
                     ({e:?}). Every block the whole writes is declared, so it must be one the \
                     game has."
                ),
            ));
        }
    };
    match &plan.fill {
        Fill::Solid { block: b } => block(b, "/content/fill/block".into(), "the solid fill", d),
        Fill::Open {
            terrain,
            surface,
            below,
        } => {
            block(
                surface,
                "/content/fill/surface".into(),
                "the ground's surface",
                d,
            );
            block(
                below,
                "/content/fill/below".into(),
                "the ground below its surface",
                d,
            );
            terrain_check(c, plan, terrain, d);
        }
    }
    for (i, v) in plan.volumes.iter().enumerate() {
        let Some(b) = &v.block else { continue };
        if v.role == VolumeRole::Clearance {
            d.push(Diagnostic::error(
                crate::codes::BLOCK_UNKNOWN,
                "site-plan",
                format!("/content/volumes/{i}/block"),
                format!(
                    "volume `{id}` is a `clearance` — air the whole keeps empty — and names the \
                     block `{b}`. A clearance is made of nothing; remove `block`, or make the \
                     volume a `massif` or a `ground`.",
                    id = v.id,
                ),
            ));
            continue;
        }
        block(
            b,
            format!("/content/volumes/{i}/block"),
            &format!("volume `{}`'s block", v.id),
            d,
        );
    }
}

fn terrain_check(c: &Campaign, plan: &SitePlanContent, terrain: &Terrain, d: &mut Vec<Diagnostic>) {
    match terrain {
        Terrain::Flat { datum } => {
            if !plan.datums.iter().any(|x| &x.id == datum) {
                d.push(Diagnostic::error(
                    crate::codes::DANGLING_REF,
                    "site-plan",
                    "/content/fill/terrain/datum",
                    format!(
                        "the terrain stands on `{datum}`, which this plan declares no \
                         `datums[]` entry for. Declare the plane — the terrain's walk plane is \
                         that datum, and its surface block stands one under it."
                    ),
                ));
            }
        }
        Terrain::Heightmap {
            heightmap,
            base_y,
            range,
        } => {
            let path = "/content/fill/terrain/heightmap";
            let (w, dz) = (plan.region.extent[0].get(), plan.region.extent[2].get());
            match c.heightmap.as_ref() {
                None => d.push(Diagnostic::error(
                    DW_BOX_LEAVES_REGION,
                    "site-plan",
                    path,
                    format!(
                        "the terrain is the heightmap `{heightmap}`, and nothing read it — this \
                         campaign was handed to the checks without its image. The ground every \
                         plot is stitched to cannot be guessed; build or validate the campaign \
                         from its directory, where the loader reads the image."
                    ),
                )),
                Some(HeightmapRead::Unreadable { path: p, why }) => d.push(Diagnostic::error(
                    DW_BOX_LEAVES_REGION,
                    "site-plan",
                    path,
                    format!(
                        "the terrain's heightmap `{p}` cannot be read: {why}. It must be a \
                         greyscale PNG in the campaign directory, exactly the region's {w} × {dz} \
                         pixels."
                    ),
                )),
                Some(HeightmapRead::Read {
                    path: p,
                    width,
                    depth,
                    grey,
                }) => {
                    if *width != w || *depth != dz || p != heightmap {
                        d.push(Diagnostic::error(
                            DW_BOX_LEAVES_REGION,
                            "site-plan",
                            path,
                            format!(
                                "the terrain's heightmap `{p}` is {width} × {depth} pixels and \
                                 the region is {w} × {dz} columns. One pixel is one column of \
                                 the region, so the image is exactly the region's size — redraw \
                                 or regenerate it at {w} × {dz}."
                            ),
                        ));
                        return;
                    }
                    let top = grey
                        .iter()
                        .map(|v| pixel_top(*base_y, *range, *v))
                        .max()
                        .unwrap_or(*base_y);
                    let low = grey
                        .iter()
                        .map(|v| pixel_top(*base_y, *range, *v))
                        .min()
                        .unwrap_or(*base_y);
                    let (rlo, rhi) = (plan.region.min[1], plan.region.max()[1]);
                    if top > rhi || low < rlo {
                        d.push(Diagnostic::error(
                            DW_BOX_LEAVES_REGION,
                            "site-plan",
                            path,
                            format!(
                                "the terrain leaves the region: its surface runs from y {low} to \
                                 y {top} (`base_y` {base_y}, `range` {range}) and the region \
                                 spans y {rlo}..{rhi}. The terrain is placed by the plan like \
                                 any box, so it stands inside the region the brief fixed — lower \
                                 `base_y`, narrow `range`, or change the brief's fact and \
                                 re-derive the region."
                            ),
                        ));
                    }
                }
            }
        }
    }
}

/// `DW0826` over every claim, and `DW0827` over every cell two claims share.
///
/// Read off the resolved plan, the same [`Site`] every frame is cut from.
pub(super) fn claims(c: &Campaign, plan: &SitePlanContent, d: &mut Vec<Diagnostic>) -> usize {
    let resolved = SitePlan::of(c);
    let site = resolved.site();
    let (rlo, rhi) = (plan.region.min, plan.region.max());
    let index: BTreeMap<&str, usize> = plan
        .boxes
        .iter()
        .enumerate()
        .map(|(i, b)| (b.node.0.as_str(), i))
        .collect();
    for (i, b) in resolved.boxes.iter().enumerate() {
        let (space_lo, space_hi) = b.space();
        let space_in = (0..3).all(|a| space_lo[a] >= rlo[a] && space_hi[a] <= rhi[a]);
        if !space_in {
            continue; // the play space itself is `region`'s own finding
        }
        let (lo, hi) = site.claim_bounds(i);
        if (0..3).all(|a| lo[a] >= rlo[a] && hi[a] <= rhi[a]) {
            continue;
        }
        d.push(Diagnostic::error(
            DW_BOX_LEAVES_REGION,
            "site-plan",
            format!(
                "/content/boxes/{}",
                index.get(b.node.0.as_str()).copied().unwrap_or(0)
            ),
            format!(
                "the claim of `{node}` leaves the region: it spans x {x0}..{x1}, y {y0}..{y1}, z \
                 {z0}..{z1} — its play space, the one-cell ring its walls may stand in, the \
                 ground under it down to y {y0} and any roof the plan declares — and the region \
                 spans x {a0}..{a1}, y {b0}..{b1}, z {c0}..{c1}. A place owns its outside, so \
                 everything it owns is placed, and the region holds what the plan places. Move \
                 the box in from the edge, lower its roof, or change the brief's fact and \
                 re-derive the region.",
                node = b.node,
                x0 = lo[0],
                x1 = hi[0],
                y0 = lo[1],
                y1 = hi[1],
                z0 = lo[2],
                z1 = hi[2],
                a0 = rlo[0],
                a1 = rhi[0],
                b0 = rlo[1],
                b1 = rhi[1],
                c0 = rlo[2],
                c1 = rhi[2],
            ),
        ));
    }
    let (contests, awarded) = site.contests();
    for (names, cells) in contests {
        let shown: Vec<String> = cells
            .iter()
            .take(6)
            .map(|c| format!("[{}, {}, {}]", c[0], c[1], c[2]))
            .collect();
        let last = names
            .last()
            .cloned()
            .unwrap_or_else(|| NodeId(String::new()));
        let who = names
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(" and ");
        d.push(Diagnostic::error(
            DW_BOXES_OVERLAP,
            "site-plan",
            format!(
                "/content/boxes/{}",
                index.get(last.0.as_str()).copied().unwrap_or(0)
            ),
            format!(
                "{who} all claim {n} cell(s) and no rule awards them — {shown}{more}. Two owners \
                 for one block, and no rule to pick by: a place owns the one-cell ring its walls \
                 may stand in, so places exactly one cell apart (side by side, or corner to \
                 corner) ask to draw the same wall, and only a connection between them says \
                 which side draws it (its `a`), or which is roofed where the other is open. Give \
                 them a seam (its `a` draws the wall), stand them two or more cells apart (each \
                 then owns its own ring and the gap between is the site's fill — on an open \
                 site, the commons both may open onto), or make them \
                 one place. Where seams between them already exist, they disagree about which \
                 is `a` on this plane.",
                n = cells.len(),
                shown = shown.join(", "),
                more = if cells.len() > shown.len() {
                    format!(", and {} more", cells.len() - shown.len())
                } else {
                    String::new()
                },
            ),
        ));
    }
    awarded
}
