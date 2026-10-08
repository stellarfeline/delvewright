// ---------------------------------------------------------------------------
// Seams: allocated on a face both boxes already have
// ---------------------------------------------------------------------------

/// The rectangle two boxes have in common on one face, in the face's own two
/// in-plane world axes, plus the plane the wall between them stands in.
#[derive(Debug, Clone, Copy)]
struct SharedFace {
    plane: i64,
    u: (i64, i64),
    v: (i64, i64),
    u_axis: &'static str,
    v_axis: &'static str,
}

/// Why two boxes do not share the declared face.
#[derive(Debug, Clone)]
enum NotShared {
    /// They are not neighbours across it: the gap is `gap` cells where the one
    /// wall they would have in common needs exactly 1.
    NotAdjacent { gap: i64 },
    /// They are neighbours, but the face they would share is empty because their
    /// spans miss each other on one of the two in-plane axes.
    NoCommonArea { axis: &'static str },
    /// One of them is sky-open with no stated headroom, so it has no ceiling or
    /// floor plane for a horizontal seam to sit in.
    NoPlane { which: &'static str },
}

/// Do these two boxes share `face` **of `a`**, and where?
///
/// A shared face is a **one-cell gap** — the wall the two places have in common,
/// which the derivation writes once. See [`Placed`] for why the box is the play
/// space rather than the play space plus its shell.
/// The geometry a shared-face question needs of one box: its footprint and its
/// vertical span, when it has one.
///
/// A tiny value rather than `&Placed` so that the **one** implementation of "do
/// these two boxes share this face" serves both readers of the resolved plan:
/// the stage-4 checks, which hold a partially-resolved box, and the derivation
/// and battery in the compiler, which hold a [`PlacedBox`]. A second copy of
/// this arithmetic is how a plan-time green and a byte-time green come to be
/// about different walls.
#[derive(Clone, Copy)]
struct FaceSide {
    foot: [i64; 4],
    y: Option<(i64, i64)>,
}

impl Placed<'_> {
    fn side(&self) -> FaceSide {
        FaceSide {
            foot: self.foot,
            y: self.y_span(),
        }
    }
}

impl PlacedBox {
    fn side(&self) -> FaceSide {
        let (lo, hi) = self.space();
        FaceSide {
            foot: self.foot,
            y: Some((lo[1], hi[1])),
        }
    }
}

/// [`shared_face`] over two fully resolved boxes.
fn shared_face_of(a: &PlacedBox, b: &PlacedBox, face: Face) -> Result<SharedFace, NotShared> {
    shared_face(a.side(), b.side(), face)
}

fn shared_face(a: FaceSide, b: FaceSide, face: Face) -> Result<SharedFace, NotShared> {
    let horizontal_pair = |plane: i64, u: (i64, i64), v: (i64, i64)| -> SharedFace {
        SharedFace {
            plane,
            u,
            v,
            u_axis: "x",
            v_axis: "z",
        }
    };
    match face {
        Face::East | Face::West | Face::South | Face::North => {
            // The axis the face's normal runs along, and the horizontal axis
            // that stays in the plane.
            let (normal, along) = match face {
                Face::East | Face::West => (0usize, 2usize),
                _ => (2usize, 0usize),
            };
            let a_span = span(a.foot, normal);
            let b_span = span(b.foot, normal);
            let positive = matches!(face, Face::East | Face::South);
            let (plane, gap) = if positive {
                (a_span.1 + 1, b_span.0 - a_span.1 - 1)
            } else {
                (a_span.0 - 1, a_span.0 - b_span.1 - 1)
            };
            if gap != SHARED_FACE_GAP_CELLS {
                return Err(NotShared::NotAdjacent { gap });
            }
            let a_along = span(a.foot, along);
            let b_along = span(b.foot, along);
            let u = overlap(a_along, b_along).ok_or(NotShared::NoCommonArea {
                axis: if along == 0 { "x" } else { "z" },
            })?;
            let (ya, yb) = match (a.y, b.y) {
                (Some(ya), Some(yb)) => (ya, yb),
                (None, _) => return Err(NotShared::NoPlane { which: "a" }),
                (_, None) => return Err(NotShared::NoPlane { which: "b" }),
            };
            let v = overlap(ya, yb).ok_or(NotShared::NoCommonArea { axis: "y" })?;
            Ok(SharedFace {
                plane,
                u,
                v,
                u_axis: if along == 0 { "x" } else { "z" },
                v_axis: "y",
            })
        }
        Face::Up | Face::Down => {
            let (Some(ya), Some(yb)) = (a.y, b.y) else {
                return Err(NotShared::NoPlane {
                    which: if a.y.is_none() { "a" } else { "b" },
                });
            };
            let (plane, gap) = if face == Face::Up {
                (ya.1 + 1, yb.0 - ya.1 - 1)
            } else {
                (ya.0 - 1, ya.0 - yb.1 - 1)
            };
            if gap != SHARED_FACE_GAP_CELLS {
                return Err(NotShared::NotAdjacent { gap });
            }
            let u = overlap(span(a.foot, 0), span(b.foot, 0))
                .ok_or(NotShared::NoCommonArea { axis: "x" })?;
            let v = overlap(span(a.foot, 2), span(b.foot, 2))
                .ok_or(NotShared::NoCommonArea { axis: "z" })?;
            Ok(horizontal_pair(plane, u, v))
        }
    }
}

/// One seam and everything already resolved about it: the two places it joins,
/// the connection it allocates, and the face they share. Carried as one value so
/// each rule below takes the seam and its context rather than eight positional
/// arguments — the shape a `clippy::too_many_arguments` allow would otherwise
/// have papered over.
struct SeamCtx<'a> {
    index: usize,
    seam: &'a Seam,
    edge: &'a Edge,
    a: &'a Placed<'a>,
    b: &'a Placed<'a>,
    face: SharedFace,
    /// The crossing's low corner on the face's own two world axes, from the
    /// packing — `[along, sill]` on a wall, `[x, z]` through a floor.
    at: [i64; 2],
}

/// `DW0828`–`DW0831`: every seam sits on a face its two boxes share, at cells
/// that face has, through a standard opening a body can use, and — where the two
/// places are on different planes — by a climb or a fall the standards allow.
fn seams(
    plan: &SitePlanContent,
    graph: &LayoutGraphContent,
    placed: &[Placed<'_>],
    packed: &Packed,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) {
    let by_node: BTreeMap<&str, &Placed<'_>> =
        placed.iter().map(|p| (p.plan.node.0.as_str(), p)).collect();
    let edges: BTreeMap<&str, &Edge> = graph.edges.iter().map(|e| (e.id().0.as_str(), e)).collect();

    for (i, s) in plan.seams.iter().enumerate() {
        if packed.refused.contains(&i) {
            continue; // the packing refused this seam by name already.
        }
        let Some(edge) = edges.get(s.edge.0.as_str()) else {
            continue; // `DW0824` refused the reference.
        };
        if !edge.has_seam() {
            continue; // `DW0824` said this carries a sightline.
        }
        let (Some(a), Some(b)) = (
            by_node.get(edge.a().0.as_str()).copied(),
            by_node.get(edge.b().0.as_str()).copied(),
        ) else {
            continue; // `DW0824` reported the missing box, or nothing placed it.
        };
        let Some(at) = packed.seam_at[i] else {
            continue; // an end has no plane; `DW0112` said so.
        };

        let face = match shared_face(a.side(), b.side(), s.face) {
            Ok(f) => f,
            Err(why) => {
                d.push(not_shared(i, s, edge, a, b, &why));
                continue;
            }
        };

        let ctx = SeamCtx {
            index: i,
            seam: s,
            edge,
            a,
            b,
            face,
            at,
        };

        // `DW0876`, first: a seam that does not state exactly one kind of
        // connection has no crossing for any rule below to judge, and telling
        // an author both that and what the crossing they did not state would
        // have meant prescribes two repairs for one mistake.
        if !contact_declaration(&ctx, table, reads, d) {
            continue;
        }

        let opening = if s.contact.is_some() {
            // A contact has no opening name to resolve and no single sill, so
            // `DW0829` does not run over it. That is stated rather than
            // shoehorned: calling a 55-cell front a door would make every
            // downstream door check wrong (spec-0053 §4).
            None
        } else {
            match table.resolve(
                MetricKind::Opening,
                s.opening.as_deref().unwrap_or_default(),
            ) {
                Ok(e) => match e.value(reads) {
                    MetricValue::Opening(o) => Some(*o),
                    _ => continue,
                },
                Err(unknown) => {
                    d.push(unknown.diagnostic("site-plan", &format!("/content/seams/{i}/opening")));
                    continue;
                }
            }
        };

        if let Some(opening) = opening {
            opening_fits(&ctx, opening, d);
        }
        match edge {
            Edge::Stair { .. } => stair(&ctx, table, reads, d),
            Edge::Drop { falls, .. } => drop_seam(&ctx, *falls, table, reads, d),
            Edge::Walk { .. } | Edge::Barred { .. } => {
                if let Some(opening) = opening {
                    sill(&ctx, opening, d);
                }
            }
            Edge::Carry { .. } | Edge::Vision { .. } => {}
        }
        if matches!(edge, Edge::Stair { .. }) && s.stair_in.is_none() {
            d.push(Diagnostic::error(
                DW_STAIR_PITCH,
                "site-plan",
                format!("/content/seams/{i}"),
                format!(
                    "the seam for stair `{id}` does not say which place hosts its treads. A \
                     stair is massing, and massing stands somewhere: name `{a}` or `{b}` in \
                     `stair_in`, so that the run it costs comes out of a footprint the plan has \
                     already allocated rather than out of whatever space happens to be left.",
                    id = s.edge,
                    a = edge.a(),
                    b = edge.b(),
                ),
            ));
        }
    }
}

/// `DW0828`, with the arithmetic that produced it.
fn not_shared(
    i: usize,
    s: &Seam,
    edge: &Edge,
    a: &Placed<'_>,
    b: &Placed<'_>,
    why: &NotShared,
) -> Diagnostic {
    let detail = match why {
        NotShared::NotAdjacent { gap } if *gap < 0 => format!(
            "they overlap by {} cell(s) across it rather than standing one apart",
            -gap
        ),
        NotShared::NotAdjacent { gap } => format!(
            "there are {gap} cells between them across that face where a shared wall is exactly \
             {SHARED_FACE_GAP_CELLS}"
        ),
        NotShared::NoCommonArea { axis } => format!(
            "they are neighbours across it, but their spans on {axis} miss each other entirely, \
             so the face they share has no area to cut an opening in"
        ),
        NotShared::NoPlane { which } => format!(
            "the `{which}` end is sky-open with no stated headroom, so it has no ceiling or floor \
             plane for a horizontal seam to sit in"
        ),
    };
    Diagnostic::error(
        DW_SEAM_NOT_SHARED,
        "site-plan",
        format!("/content/seams/{i}/face"),
        format!(
            "the seam for `{id}` is declared on the {face} face of `{an}`, and `{an}` and `{bn}` \
             do not share it: {detail}. **A seam is allocated on a face both boxes already \
             have** — that is the whole of why the plan places it while both are still free to \
             move, instead of two finished places discovering later that they cannot mate. \
             `{an}` is x {ax0}..{ax1}, z {az0}..{az1} at floor {af}; `{bn}` is x {bx0}..{bx1}, \
             z {bz0}..{bz1} at floor {bf}. Move one box against the other, or put the seam on \
             the face they really share.",
            id = s.edge,
            face = s.face.as_str(),
            an = edge.a(),
            bn = edge.b(),
            ax0 = a.x0(),
            ax1 = a.x1(),
            az0 = a.z0(),
            az1 = a.z1(),
            af = a.floor,
            bx0 = b.x0(),
            bx1 = b.x1(),
            bz0 = b.z0(),
            bz1 = b.z1(),
            bf = b.floor,
        ),
    )
}

/// **`DW0876`**: this seam states exactly one kind of
/// connection, and if it is a contact, one this engine builds (spec-0053 §4).
///
/// Returns `false` when the seam has no usable crossing, in which case the
/// caller stops: everything below reads the crossing rectangle.
fn contact_declaration(
    ctx: &SeamCtx<'_>,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) -> bool {
    let (i, s) = (ctx.index, ctx.seam);
    let mut refuse = |what: String, remedy: String| {
        d.push(Diagnostic::error(
            DW_CONTACT,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "the seam for `{edge}` {what}. To fix it, {remedy}.",
                edge = s.edge,
            ),
        ));
    };

    // ---- Shape 1: exactly one kind.
    match (s.opening.as_ref(), s.contact.as_ref()) {
        (Some(o), Some(_)) => {
            refuse(
                format!(
                    "declares BOTH an `opening` (`{o}`) and a `contact` — a hand-off is one \
                     kind or the other"
                ),
                "delete whichever this is not. A portal allocates the cells a body crosses \
                 at and every one of them must be passable; a contact is a front along which \
                 two places simply meet and needs only one crossable column. The derivation \
                 builds them differently and the byte observer measures them differently, so \
                 there is no world in which a seam is both"
                    .to_string(),
            );
            return false;
        }
        (None, None) => {
            refuse(
                "declares neither an `opening` nor a `contact`, so it states no way across"
                    .to_string(),
                format!(
                    "give it one. A doorway is `\"opening\": \"<name>\"` — defined \
                     standards: {names}. A front where the two places simply meet is \
                     `\"contact\": {{}}`, which spans from `at` to the far edge of the \
                     shared face",
                    names = table.names_of(MetricKind::Opening).join(", "),
                ),
            );
            return false;
        }
        (Some(_), None) => return true,
        (None, Some(_)) => {}
    }

    // ---- Shape 4: the classes a contact may carry.
    //
    // `walk` and `drop` only. A rim falling to a lower court is a genuine broad
    // hand-off, so `drop` is in; `stair`, `barred` and `vision` are excluded
    // until a campaign brief demands one (spec-0053 §4, the falsifier re-armed).
    if !matches!(ctx.edge, Edge::Walk { .. } | Edge::Drop { .. }) {
        refuse(
            format!(
                "is a contact on a `{class}` connection, and a contact carries `walk` or \
                 `drop` only",
                class = ctx.edge.class(),
            ),
            "give the seam a standard `opening`, or declare the connection `walk` or \
             `drop` in the layout graph. A stair needs a run and a pitch, a barred door \
             needs a gate region that seals and clears, and a sightline is not a crossing \
             at all — none of the three is a thing a front can be, and this engine does \
             not have them as contacts until a campaign brief demands one"
                .to_string(),
        );
        return false;
    }

    // ---- Shape 3: wider than the broadest standard opening.
    //
    // The floor is derived from the standard set, so anything at or under it
    // could have been a portal. That is what makes it a demand the defect
    // cannot supply: a door declared a contact is refused by its own width.
    let Some(floor) = table.broadest_opening_width(reads) else {
        return false; // `Metrics::self_check` reports a table with no openings.
    };
    let (u_span, v_span) = (ctx.face.u, ctx.face.v);
    let (u_hi, v_hi) = crossing_hi(ctx);
    let width = u_hi - ctx.at[0] + 1;
    if width <= i64::from(floor) {
        refuse(
            format!(
                "is a contact {width} cell(s) wide, which is not wider than the broadest \
                 standard opening ({floor} cells)"
            ),
            format!(
                "widen the span, or declare it a portal — anything this narrow could have \
                 been one, and a doorway called a contact would dodge the standard set \
                 while every downstream door check went on being wrong about it. Defined \
                 openings: {names}",
                names = table.names_of(MetricKind::Opening).join(", "),
            ),
        );
        return false;
    }

    // ---- Shape 2: the span lies on the shared face.
    let mut off: Vec<String> = Vec::new();
    if ctx.at[0] < u_span.0 || u_hi > u_span.1 {
        off.push(format!(
            "{}..{} on {}, against the face's {}..{}",
            ctx.at[0], u_hi, ctx.face.u_axis, u_span.0, u_span.1
        ));
    }
    if ctx.at[1] < v_span.0 || v_hi > v_span.1 {
        off.push(format!(
            "{}..{} on {}, against the face's {}..{}",
            ctx.at[1], v_hi, ctx.face.v_axis, v_span.0, v_span.1
        ));
    }
    if !off.is_empty() {
        refuse(
            format!(
                "is a contact whose span leaves the face the two boxes share: {}",
                off.join("; ")
            ),
            "move `at` onto the shared face, or shorten `contact.extent` — the span is \
             where the derivation writes no wall, and a span running off the face would \
             ask it to open a wall that is not there. Omitting `contact.extent` runs the \
             span from `at` to the far edge of the face, which never leaves it"
                .to_string(),
        );
        return false;
    }
    true
}

/// The far corner of a seam's crossing rectangle, on the face's own two in-plane
/// axes — [`contact_extent`] resolved against the seam's own anchor, so this
/// rule and the derivation describe one rectangle.
fn crossing_hi(ctx: &SeamCtx<'_>) -> (i64, i64) {
    let e = contact_extent(ctx.seam, ctx.at, &ctx.face);
    (ctx.at[0] + e[0] - 1, ctx.at[1] + e[1] - 1)
}

/// `DW0828`'s anchor half and `DW0829`'s geometric half: the opening's cells are
/// cells the shared face has.
fn opening_fits(ctx: &SeamCtx<'_>, opening: crate::metrics::Opening, d: &mut Vec<Diagnostic>) {
    let (i, s, edge, face, at) = (ctx.index, ctx.seam, ctx.edge, &ctx.face, ctx.at);
    let anchor_in =
        at[0] >= face.u.0 && at[0] <= face.u.1 && at[1] >= face.v.0 && at[1] <= face.v.1;
    if !anchor_in {
        d.push(Diagnostic::error(
            DW_SEAM_NOT_SHARED,
            "site-plan",
            format!("/content/seams/{i}/at"),
            format!(
                "the seam for `{id}` is anchored at {ua} {u}, {va} {v}, which is not on the face \
                 `{an}` and `{bn}` share — that face runs {ua} {u0}..{u1} by {va} {v0}..{v1} in \
                 the plane at {plane}. `at` names the opening's low corner in the face's own two \
                 axes, so a corner off the face allocates the seam nowhere.",
                id = s.edge,
                an = edge.a(),
                bn = edge.b(),
                ua = face.u_axis,
                va = face.v_axis,
                u = at[0],
                v = at[1],
                u0 = face.u.0,
                u1 = face.u.1,
                v0 = face.v.0,
                v1 = face.v.1,
                plane = face.plane,
            ),
        ));
        return;
    }
    let u_hi = at[0] + i64::from(opening.width) - 1;
    let v_hi = at[1] + i64::from(opening.height) - 1;
    if u_hi <= face.u.1 && v_hi <= face.v.1 {
        return;
    }
    d.push(Diagnostic::error(
        DW_SEAM_OPENING,
        "site-plan",
        format!("/content/seams/{i}/opening"),
        format!(
            "the `{name}` opening ({w}x{h}) does not fit on the face `{an}` and `{bn}` share. \
             Anchored at {ua} {u}, {va} {v} it would run to {ua} {u_hi}, {va} {v_hi}, and the \
             shared face ends at {ua} {u1}, {va} {v1}. Move the anchor, choose a narrower \
             standard opening, or grow the overlap between the two boxes — the standard set is \
             the vocabulary, so the opening is never quietly cropped to fit.",
            name = s.opening.as_deref().unwrap_or_default(),
            w = opening.width,
            h = opening.height,
            an = edge.a(),
            bn = edge.b(),
            ua = face.u_axis,
            va = face.v_axis,
            u = at[0],
            v = at[1],
            u1 = face.u.1,
            v1 = face.v.1,
        ),
    ));
}

/// `DW0829`'s step-rule half: a body standing on the floor of a side it enters
/// from can get onto the sill.
fn sill(ctx: &SeamCtx<'_>, opening: crate::metrics::Opening, d: &mut Vec<Diagnostic>) {
    let (i, s, edge, a, b, face) = (ctx.index, ctx.seam, ctx.edge, ctx.a, ctx.b, &ctx.face);
    if face.v_axis != "y" {
        return; // a horizontal seam has no sill; the fall or the treads own it.
    }
    let sources: Vec<(&NodeId, &Placed<'_>)> = match edge.direction() {
        Some(crate::layout::Direction::AToB) => vec![(edge.a(), a)],
        Some(crate::layout::Direction::BToA) => vec![(edge.b(), b)],
        None => vec![(edge.a(), a), (edge.b(), b)],
    };
    let max_rise = MAX_JUMP_RISE_16 / crate::metrics::FULL_16;
    for (name, p) in sources {
        let rise = ctx.at[1] - p.floor;
        if rise <= max_rise {
            continue;
        }
        d.push(Diagnostic::error(
            DW_SEAM_OPENING,
            "site-plan",
            format!("/content/seams/{i}/at"),
            format!(
                "the seam for `{id}` has its sill at y {sill}, {rise} blocks over the floor of \
                 `{name}` at y {floor}, and a body reaches at most {max_rise} block(s) by \
                 jumping ({j}/16 of vanilla's apex). A body entering from `{name}` cannot get \
                 into the opening at all, so the connection the graph declares is not one. The \
                 sill is the higher of the two floors: bring the floors within a step of each \
                 other, or declare the connection a `stair` and let the treads carry the climb. \
                 (The opening is {w}x{h}.)",
                id = s.edge,
                sill = ctx.at[1],
                j = MAX_JUMP_RISE_16,
                floor = p.floor,
                w = opening.width,
                h = opening.height,
            ),
        ));
    }
}

/// `DW0830`: the stair the plan allocated can be built at a standard pitch,
/// inside the box the plan said hosts it.
fn stair(ctx: &SeamCtx<'_>, table: &Metrics, reads: &mut Reads, d: &mut Vec<Diagnostic>) {
    let (i, s, edge, a, b, face) = (ctx.index, ctx.seam, ctx.edge, ctx.a, ctx.b, &ctx.face);
    let rise = b.floor - a.floor;
    if rise == 0 {
        d.push(Diagnostic::error(
            DW_STAIR_PITCH,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "`{id}` is a stair, and `{an}` and `{bn}` are both on plane y {f} — so it climbs \
                 nothing. A stair's rise is not authored here: it is the difference between the \
                 two floors the plan has already chosen, which means a stair between two places \
                 at one level is a walk that has been called a stair. Move one floor, or declare \
                 the connection a `walk`.",
                id = s.edge,
                an = edge.a(),
                bn = edge.b(),
                f = a.floor,
            ),
        ));
        return;
    }
    let Some(host_id) = &s.stair_in else {
        return; // the missing declaration is reported by `seams`.
    };
    // **The treads stand in the LOWER place**, and that is geometry rather than
    // taste: a stair is a stack of courses rising off a walk plane, and the only
    // walk plane it can rise off is the lower of the two. Hosting it in the
    // upper place asks for a stack that starts at that place's floor and has to
    // reach a level *below* it, which is not a stair — it is a hole with treads
    // drawn in the air under it.
    //
    // Found by building. This code checked only that the host affords the RUN,
    // so a plan naming the upper place reached green at stage 4 and the
    // derivation then laid a mound on the wrong side of the opening; the
    // stage-5 observer caught it as a seam whose hole was still solid, which is
    // the right refusal for the wrong defect. `stair_in` stays authored rather
    // than derived because it says WHICH of the two footprints pays for the run
    // when both are candidates — but when only one can be, saying the other is
    // a refusal.
    let (low, high) = if b.floor > a.floor {
        (edge.a(), edge.b())
    } else {
        (edge.b(), edge.a())
    };
    if host_id == high {
        d.push(Diagnostic::error(
            DW_STAIR_PITCH,
            "site-plan",
            format!("/content/seams/{i}/stair_in"),
            format!(
                "the stair for `{id}` hosts its treads in `{high}`, which is the HIGHER of the two \
                 places (`{an}` stands at y {af}, `{bn}` at y {bf}). Treads rise off a walk plane, \
                 and the only plane this stair can rise off is the lower one — massing in the \
                 upper place would have to start at that place's floor and reach a level beneath \
                 it, which is not a stair. Host it in `{low}`, and check that `{low}` affords the \
                 run: a stair costs its footprint, and moving the host moves who pays.",
                id = s.edge,
                an = edge.a(),
                bn = edge.b(),
                af = a.floor,
                bf = b.floor,
            ),
        ));
        return;
    }
    let host = if host_id == edge.a() { a } else { b };
    // **The run is measured the way the derivation lays it**, by the function
    // that lays it — [`stair_run`]. What this check used to measure instead was
    // the seam's rise against the host's whole extent, and neither is what a
    // tread run costs: the courses carry a body to the OPENING, and through a
    // pierced floor they leave along one side of the hole. See [`stair_run`] for
    // the plan that reached green at `needs 8, affords 8` and built no stair.
    let Some((_, extent)) = crossing_rect(s, ctx.at, face, table, reads) else {
        return; // `DW0812` refused the opening; there is no rectangle to measure.
    };
    let Some(run) = stair_run(
        host.floor,
        host.foot,
        normal_axis_of(s.face),
        face.plane,
        crossing_aabb(s, ctx.at, face, extent),
    ) else {
        return; // no run to lay: the higher host is refused above, the stray hole by `DW0828`.
    };
    let run_axis = if run.run_axis == 0 { "x" } else { "z" };
    if gentlest_pitch(table, reads, run.climb, run.available).is_some() {
        return; // some standard pitch fits.
    }
    let Some((name, needed)) = tightest_pitch(table, reads, run.climb) else {
        return; // the table defines no pitch; `Metrics::self_check` owns that.
    };
    let carries = if run.climb == rise.abs() {
        String::new()
    } else {
        format!(
            " The treads carry {climb}, not {rise}: they rise off `{host_id}`'s floor at \
             {hf} and stop at {target}, which is where the opening puts a body.",
            climb = run.climb,
            rise = rise.abs(),
            hf = host.floor,
            target = host.floor + run.climb,
        )
    };
    d.push(Diagnostic::error(
        DW_STAIR_PITCH,
        "site-plan",
        format!("/content/seams/{i}"),
        format!(
            "the stair for `{id}` climbs {rise} block(s) between `{an}` (floor {af}) and `{bn}` \
             (floor {bf}), and no standard pitch fits inside `{host_id}`. The tightest standard \
             is `{name}`, which needs {needed} block(s) of run for a climb of {climb}, and \
             `{host_id}` affords {available} of run on {run_axis}.{carries} Give the host a \
             longer footprint on that axis, move the opening so the run has more room beside it, \
             host the stair in the other place, or bring the two floors closer together — the \
             pitches are standards, so a steeper one is not on offer.",
            id = s.edge,
            an = edge.a(),
            bn = edge.b(),
            af = a.floor,
            bf = b.floor,
            rise = rise.abs(),
            climb = run.climb,
            available = run.available,
        ),
    ));
}

/// `DW0831`: a designed drop falls the way it says it falls, and no further than
/// the policy allows.
fn drop_seam(
    ctx: &SeamCtx<'_>,
    falls: crate::layout::Direction,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) {
    let (i, s, edge, a, b) = (ctx.index, ctx.seam, ctx.edge, ctx.a, ctx.b);
    let (from, from_p, to, to_p) = match falls {
        crate::layout::Direction::AToB => (edge.a(), a, edge.b(), b),
        crate::layout::Direction::BToA => (edge.b(), b, edge.a(), a),
    };
    let depth = from_p.floor - to_p.floor;
    if depth <= 0 {
        d.push(Diagnostic::error(
            DW_DROP_POLICY,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "`{id}` falls from `{from}` (floor {ff}) into `{to}` (floor {tf}), which is \
                 {what}. A drop is one-way because a body cannot climb back up the way it came, \
                 and that is only true going down — this one is a mislabelled stair. Swap the \
                 declared direction, move the floors, or declare the connection a `stair`.",
                id = s.edge,
                ff = from_p.floor,
                tf = to_p.floor,
                what = if depth == 0 {
                    "the same plane".to_string()
                } else {
                    format!("{} block(s) HIGHER", -depth)
                },
            ),
        ));
        return;
    }
    let Some(cap) = table.max_designed_drop_blocks(reads) else {
        return;
    };
    if depth <= i64::from(cap) {
        return;
    }
    d.push(Diagnostic::error(
        DW_DROP_POLICY,
        "site-plan",
        format!("/content/seams/{i}"),
        format!(
            "`{id}` drops {depth} blocks from `{from}` into `{to}`, and the designed-drop policy \
             caps a declared fall at {cap}. This is a **policy** cap and it is deliberately far \
             tighter than what a body survives: a drop is a decision about the shape of the map, \
             and it should not also be a decision about the party's health. Bring the two floors \
             closer, or break the fall with a place between them.",
            id = s.edge,
        ),
    ));
}

/// The floor a designed opening may never be chosen below: the cells a standing
/// body needs to pass at all.
///
/// Not a check of its own — [`Metrics::self_check`] already holds every standard
/// opening over it, and a second refusal here would be this module re-asking a
/// question the table has already answered about itself. It is re-exported so a
/// reader of `DW0829` can see what the standard set is bounded by.
#[must_use]
pub fn passable_opening_cells() -> (u32, u32) {
    (passable_width_cells(), passable_clearance_cells())
}
