// ---------------------------------------------------------------------------
// Packing — a box is placed by its seam, and the grid is derived (spec-0059 §3)
// ---------------------------------------------------------------------------

/// How a box came to stand where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// The author pinned its `min`.
    Pinned,
    /// A seam hung it off a box that already stood.
    Seam {
        /// Index into `seams[]`.
        seam: usize,
        /// The connection the seam allocates.
        edge: EdgeId,
        /// The box it was hung off.
        from: NodeId,
        /// The face of `from` it hangs off — or, when `from` is the seam's `b`
        /// end, the face of this box the seam names.
        face: Face,
    },
}

impl Provenance {
    /// The words a refusal or the placing line uses.
    fn describe(&self) -> String {
        match self {
            Provenance::Pinned => "pinned".to_string(),
            Provenance::Seam {
                edge, from, face, ..
            } => format!(
                "hung off `{from}` across the {face} face by the seam for `{edge}`",
                face = face.as_str()
            ),
        }
    }
}

/// One box's corner, with its provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedBox {
    /// The place.
    pub node: NodeId,
    /// Low corner `[x, z]`, in world coordinates.
    pub min: [i64; 2],
    /// Where the corner came from.
    pub by: Provenance,
}

/// The packing of one plan: every corner the pins and seams settle, and the
/// world anchor of every seam whose two ends stand.
#[derive(Debug, Default)]
struct Packed {
    /// Per `plan.boxes` index; `None` when nothing placed the box.
    boxes: Vec<Option<PackedBox>>,
    /// Per `plan.seams` index: the crossing's low corner on the face's own two
    /// world axes — `[along, sill]` on a wall, `[x, z]` through a floor or
    /// ceiling. `None` when either end is unplaced or a floor is unresolved.
    seam_at: Vec<Option<[i64; 2]>>,
    /// Seams the packing itself refused; [`seams`] does not judge them twice.
    refused: BTreeSet<usize>,
    /// Connected components of the seam graph, over boxes the graph declares.
    components: usize,
    pinned: usize,
    derived: usize,
}

/// A seam resolved far enough to place a box: its two box indices and its two
/// offsets, each `[u, v]` on a horizontal face and `[u, 0]` on a vertical one.
#[derive(Debug, Clone, Copy)]
struct Link {
    a: usize,
    b: usize,
    at: [i64; 2],
    meets: [i64; 2],
}

/// The two horizontal-axis indices a face's arithmetic uses: `(normal, along)`
/// into `[x, z]`.
fn face_axes(face: Face) -> (usize, usize) {
    match face {
        Face::East | Face::West => (0, 1),
        Face::North | Face::South => (1, 0),
        Face::Up | Face::Down => (0, 1), // unused: a horizontal face has no normal in [x, z]
    }
}

/// The width a crossing is centred by, on the face's own two axes, or `None`
/// for a contact with no extent — the whole face, so the offsets default to 0.
/// `Err` when the opening cannot be resolved (`DW0812`/`DW0876` name that).
fn centring_width(s: &Seam, table: &Metrics, reads: &mut Reads) -> Result<Option<[i64; 2]>, ()> {
    if let Some(c) = &s.contact {
        return Ok(c
            .extent
            .map(|e| [i64::from(e[0].get()), i64::from(e[1].get())]));
    }
    let Some(name) = s.opening.as_deref() else {
        return Err(());
    };
    let entry = table.resolve(MetricKind::Opening, name).map_err(|_| ())?;
    match entry.value(reads) {
        MetricValue::Opening(o) => Ok(Some([i64::from(o.width), i64::from(o.height)])),
        _ => Err(()),
    }
}

/// Why an offset is not a position on its face.
enum OffsetProblem {
    /// The shape does not match the face: an integer through a floor, a pair
    /// on a wall.
    Shape { which: &'static str },
    /// The crossing, anchored there, leaves the box's own face.
    OffFace {
        which: &'static str,
        axis: &'static str,
        value: i64,
        extent: i64,
        width: i64,
    },
}

/// Resolve `at` and `meets` for one seam against its two boxes' extents:
/// declared values taken as written, omitted ones centred (spec-0059 §3).
fn offsets(
    s: &Seam,
    ext_a: [i64; 2],
    ext_b: [i64; 2],
    width: Option<[i64; 2]>,
) -> Result<([i64; 2], [i64; 2]), OffsetProblem> {
    let horizontal = s.face.is_horizontal_plane();
    let one = |which: &'static str,
               declared: Option<Offset>,
               ext: [i64; 2]|
     -> Result<[i64; 2], OffsetProblem> {
        // Which of `[dx, dz]` each offset component runs along, and its name.
        let (axes, names): ([usize; 2], [&'static str; 2]) = if horizontal {
            ([0, 1], ["x", "z"])
        } else {
            let (_, along) = face_axes(s.face);
            ([along, 0], [if along == 0 { "x" } else { "z" }, ""])
        };
        // Centred; a crossing wider than the face centres at the corner and
        // whether it fits is `DW0829`'s or `DW0876`'s question.
        let default = |i: usize| match width {
            Some(w) => (ext[axes[i]] - w[i]).div_euclid(2).max(0),
            None => 0,
        };
        let off = match (declared, horizontal) {
            (None, true) => [default(0), default(1)],
            (None, false) => [default(0), 0],
            (Some(Offset::Plane(p)), true) => p,
            (Some(Offset::Along(u)), false) => [u, 0],
            _ => return Err(OffsetProblem::Shape { which }),
        };
        let n = if horizontal { 2 } else { 1 };
        for i in 0..n {
            // The CORNER is on the face; whether the crossing fits from there is
            // `DW0829`'s (a portal) or `DW0876`'s (a contact) question, as ever.
            let extent = ext[axes[i]];
            let w = width.map_or(1, |w| w[i]);
            if off[i] < 0 || off[i] > extent - 1 {
                return Err(OffsetProblem::OffFace {
                    which,
                    axis: names[i],
                    value: off[i],
                    extent,
                    width: w,
                });
            }
        }
        Ok(off)
    };
    Ok((one("at", s.at, ext_a)?, one("meets", s.meets, ext_b)?))
}

/// The corner of the box a seam places, from the corner of the one that
/// already stands. `from_a` places `b` off `a`; otherwise `a` off `b`.
fn derive_corner(
    face: Face,
    known: [i64; 2],
    ext_a: [i64; 2],
    ext_b: [i64; 2],
    at: [i64; 2],
    meets: [i64; 2],
    from_a: bool,
) -> [i64; 2] {
    if face.is_horizontal_plane() {
        return if from_a {
            [known[0] + at[0] - meets[0], known[1] + at[1] - meets[1]]
        } else {
            [known[0] - at[0] + meets[0], known[1] - at[1] + meets[1]]
        };
    }
    let (normal, along) = face_axes(face);
    let positive = matches!(face, Face::East | Face::South);
    let mut out = [0i64; 2];
    if from_a {
        out[along] = known[along] + at[0] - meets[0];
        out[normal] = if positive {
            known[normal] + ext_a[normal] + 1
        } else {
            known[normal] - ext_b[normal] - 1
        };
    } else {
        out[along] = known[along] - at[0] + meets[0];
        out[normal] = if positive {
            known[normal] - ext_a[normal] - 1
        } else {
            known[normal] + ext_b[normal] + 1
        };
    }
    out
}

/// The crossing's low corner in the face's own two world axes — what every
/// seam rule judges. `[along, sill]` on a wall, the sill being the higher of
/// the two floors; `[x, z]` through a floor or ceiling.
fn crossing_anchor(
    face: Face,
    a_min: [i64; 2],
    at: [i64; 2],
    floor_a: i64,
    floor_b: i64,
) -> [i64; 2] {
    if face.is_horizontal_plane() {
        [a_min[0] + at[0], a_min[1] + at[1]]
    } else {
        let (_, along) = face_axes(face);
        [a_min[along] + at[0], floor_a.max(floor_b)]
    }
}

fn ext_of(b: &PlanBox) -> [i64; 2] {
    [i64::from(b.extent[0].get()), i64::from(b.extent[1].get())]
}

/// **The packing** (spec-0059 §3): the pinned boxes seed it; then `seams[]` in
/// document order, repeatedly, each seam with exactly one end standing placing
/// the other, until a pass places nothing. A seam whose two ends both stand is
/// then checked — the corner it would derive against the corner the box has —
/// and a component of the seam graph with no pinned box is refused, because
/// nothing places it.
fn pack(
    plan: &SitePlanContent,
    graph: &LayoutGraphContent,
    table: &Metrics,
    floors: &[Option<i64>],
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) -> Packed {
    let mut out = Packed {
        boxes: vec![None; plan.boxes.len()],
        seam_at: vec![None; plan.seams.len()],
        ..Packed::default()
    };
    let nodes: BTreeSet<&str> = graph.nodes.iter().map(|n| n.id.0.as_str()).collect();
    let mut by_node: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, b) in plan.boxes.iter().enumerate() {
        if nodes.contains(b.node.0.as_str()) {
            by_node.entry(b.node.0.as_str()).or_insert(i);
        }
    }
    let edges: BTreeMap<&str, &Edge> = graph.edges.iter().map(|e| (e.id().0.as_str(), e)).collect();

    // ---- seeds
    for (i, b) in plan.boxes.iter().enumerate() {
        if let Some(min) = b.min {
            out.boxes[i] = Some(PackedBox {
                node: b.node.clone(),
                min,
                by: Provenance::Pinned,
            });
            out.pinned += 1;
        }
    }

    // ---- the links: every seam whose edge and boxes resolve, with its offsets
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut links: Vec<Option<Link>> = Vec::with_capacity(plan.seams.len());
    for (i, s) in plan.seams.iter().enumerate() {
        let Some(edge) = edges.get(s.edge.0.as_str()) else {
            links.push(None);
            continue; // `DW0824` refused the reference.
        };
        if !edge.has_seam() {
            links.push(None);
            continue; // `DW0824` said this carries a sightline.
        }
        let (Some(&a), Some(&b)) = (
            by_node.get(edge.a().0.as_str()),
            by_node.get(edge.b().0.as_str()),
        ) else {
            links.push(None);
            continue; // `DW0824` reported the missing box.
        };
        pairs.push((a, b));
        // An unresolvable opening (`DW0812`/`DW0876` say so) centres nothing:
        // the crossing is taken one cell wide at the corner, so the seam still
        // places and the rules that name the opening run over a plan that
        // stands.
        let width = centring_width(s, table, reads).unwrap_or_default();
        match offsets(s, ext_of(&plan.boxes[a]), ext_of(&plan.boxes[b]), width) {
            Ok((at, meets)) => links.push(Some(Link { a, b, at, meets })),
            Err(problem) => {
                d.push(offset_problem(i, s, edge, problem));
                out.refused.insert(i);
                links.push(None);
            }
        }
    }

    // ---- placement passes
    loop {
        let mut changed = false;
        for (i, link) in links.iter().enumerate() {
            let Some(l) = link else { continue };
            let s = &plan.seams[i];
            let (ext_a, ext_b) = (ext_of(&plan.boxes[l.a]), ext_of(&plan.boxes[l.b]));
            match (out.boxes[l.a].clone(), out.boxes[l.b].clone()) {
                (Some(a), None) => {
                    let min = derive_corner(s.face, a.min, ext_a, ext_b, l.at, l.meets, true);
                    out.boxes[l.b] = Some(PackedBox {
                        node: plan.boxes[l.b].node.clone(),
                        min,
                        by: Provenance::Seam {
                            seam: i,
                            edge: s.edge.clone(),
                            from: a.node,
                            face: s.face,
                        },
                    });
                    out.derived += 1;
                    changed = true;
                }
                (None, Some(b)) => {
                    let min = derive_corner(s.face, b.min, ext_a, ext_b, l.at, l.meets, false);
                    out.boxes[l.a] = Some(PackedBox {
                        node: plan.boxes[l.a].node.clone(),
                        min,
                        by: Provenance::Seam {
                            seam: i,
                            edge: s.edge.clone(),
                            from: b.node,
                            face: s.face,
                        },
                    });
                    out.derived += 1;
                    changed = true;
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }

    // ---- every seam whose two ends stand: one placement, and the anchor
    for (i, link) in links.iter().enumerate() {
        let Some(l) = link else { continue };
        let (Some(a), Some(b)) = (&out.boxes[l.a], &out.boxes[l.b]) else {
            continue;
        };
        let s = &plan.seams[i];
        let (ext_a, ext_b) = (ext_of(&plan.boxes[l.a]), ext_of(&plan.boxes[l.b]));
        let want = derive_corner(s.face, a.min, ext_a, ext_b, l.at, l.meets, true);
        if want != b.min {
            d.push(two_placements(i, s, a, b, want));
            out.refused.insert(i);
            continue;
        }
        if let (Some(fa), Some(fb)) = (floors[l.a], floors[l.b]) {
            out.seam_at[i] = Some(crossing_anchor(s.face, a.min, l.at, fa, fb));
        }
    }

    // ---- components with nothing to place them
    let mut parent: Vec<usize> = (0..plan.boxes.len()).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut c = i;
        while p[c] != r {
            let n = p[c];
            p[c] = r;
            c = n;
        }
        r
    }
    for &(a, b) in &pairs {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    }
    let mut members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, b) in plan.boxes.iter().enumerate() {
        if nodes.contains(b.node.0.as_str()) {
            let r = find(&mut parent, i);
            members.entry(r).or_default().push(i);
        }
    }
    out.components = members.len();
    let entry = graph.entry.0.as_str();
    for (_, boxes) in members {
        if boxes.iter().any(|&i| plan.boxes[i].min.is_some()) {
            continue;
        }
        let names: Vec<String> = boxes
            .iter()
            .map(|&i| format!("`{}`", plan.boxes[i].node))
            .collect();
        let suggested = boxes
            .iter()
            .find(|&&i| plan.boxes[i].node.0 == entry)
            .or(boxes.first())
            .map(|&i| plan.boxes[i].node.to_string())
            .unwrap_or_default();
        d.push(Diagnostic::error(
            DW_UNPLACED,
            "site-plan",
            format!("/content/boxes/{}", boxes[0]),
            format!(
                "nothing places {list}: no box among them pins its `min`, and a box stands \
                 only where a pin puts it or where a seam hangs it off a box that already \
                 stands. Pin one of them — `{suggested}` — with `\"min\": [x, z]`, and the \
                 seams place the rest. {count} box(es) in this component.",
                list = names.join(", "),
                count = boxes.len(),
            ),
        ));
    }
    out
}

/// `DW0828`: an offset that is not a position on its own box's face.
fn offset_problem(i: usize, s: &Seam, edge: &Edge, problem: OffsetProblem) -> Diagnostic {
    let (which, detail) = match problem {
        OffsetProblem::Shape { which } => (
            which,
            if s.face.is_horizontal_plane() {
                format!(
                    "the {face} face is a floor or ceiling with two in-plane axes, and `{which}` \
                     gives one number. Write `[dx, dz]` — cells along x and z from the box's \
                     low corner",
                    face = s.face.as_str()
                )
            } else {
                format!(
                    "the {face} face is a wall with one horizontal axis, and `{which}` gives \
                     two numbers. Write one — cells along the face from the box's low corner; \
                     the sill is not written, it is the higher of the two floors",
                    face = s.face.as_str()
                )
            },
        ),
        OffsetProblem::OffFace {
            which,
            axis,
            value,
            extent,
            width,
        } => (
            which,
            format!(
                "`{which}` puts the crossing's corner at {value} along {axis} on a face that \
                 runs 0..{last} — the crossing is {width} wide and the box is {extent} on that \
                 axis. Write an offset on the face, or omit it and the crossing is centred; an \
                 offset is never quietly clamped to fit",
                last = extent - 1,
            ),
        ),
    };
    Diagnostic::error(
        DW_SEAM_NOT_SHARED,
        "site-plan",
        format!("/content/seams/{i}/{which}"),
        format!(
            "the seam for `{id}` between `{an}` and `{bn}` names no position on the {side} \
             box's face: {detail}.",
            id = s.edge,
            an = edge.a(),
            bn = edge.b(),
            side = if which == "at" { "`a`" } else { "`b`" },
        ),
    )
}

/// A seam whose two boxes both stand, and which would put `b` somewhere else:
/// `DW0883` when a pin is one of the two authorities, `DW0828` when a loop does
/// not close.
fn two_placements(i: usize, s: &Seam, a: &PackedBox, b: &PackedBox, want: [i64; 2]) -> Diagnostic {
    // Two authorities on `b`: its pin and this seam. When `b` was placed by
    // another seam, the disagreement is between seams — a loop that does not
    // close — however `a` came to stand where it stands.
    let pinned = matches!(b.by, Provenance::Pinned);
    let where_ = format!(
        "`{an}` stands at [{ax}, {az}] ({a_by}); `{bn}` stands at [{bx}, {bz}] ({b_by}); hung \
         off `{an}`'s {face} face by this seam, `{bn}` would stand at [{wx}, {wz}]",
        an = a.node,
        ax = a.min[0],
        az = a.min[1],
        a_by = a.by.describe(),
        bn = b.node,
        bx = b.min[0],
        bz = b.min[1],
        b_by = b.by.describe(),
        face = s.face.as_str(),
        wx = want[0],
        wz = want[1],
    );
    if pinned {
        Diagnostic::error(
            DW_UNPLACED,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "two things place one box, and they disagree: {where_}. A pin is a claim the \
                 packing verifies, never a second authority — move the pin to the corner the \
                 seam derives, delete it and let the seam place the box, or change this seam's \
                 `at`/`meets` so the two agree.",
            ),
        )
    } else {
        Diagnostic::error(
            DW_SEAM_NOT_SHARED,
            "site-plan",
            format!("/content/seams/{i}"),
            format!(
                "the seam for `{id}` closes a loop, and the loop does not close: {where_}. Every \
                 box in the loop was placed by an earlier seam, so this one can only check; \
                 change its `at`/`meets` to where the two boxes really meet, or move the \
                 offsets of the seams that placed them.",
                id = s.edge,
            ),
        )
    }
}

/// Every box's corner and how it was obtained, one line each — the derivation
/// handed back, so a creator reads a corner from the build rather than typing
/// it into the document.
#[must_use]
pub fn placements(c: &Campaign) -> Vec<String> {
    let (Some(plan), Some(graph)) = (
        c.site_plan.as_ref().map(|p| &p.content),
        c.layout_graph.as_ref().map(|g| &g.content),
    ) else {
        return Vec::new();
    };
    let table = Metrics::table();
    let mut reads = Reads::new();
    let mut sink = Vec::new();
    let (_, packed) = resolve(plan, graph, &table, &mut reads, &mut sink);
    packed
        .boxes
        .iter()
        .flatten()
        .map(|b| {
            format!(
                "site-plan placing: `{node}` stands at [{x}, {z}] — {by}.",
                node = b.node,
                x = b.min[0],
                z = b.min[1],
                by = b.by.describe(),
            )
        })
        .collect()
}

/// Resolve every box once: its footprint, its walk plane, its headroom and its
/// size class. A floor naming a datum the plan does not declare is the ordinary
/// dangling reference (`DW0112`) and the box is dropped, because a place with no
/// plane has no geometry for any rule below to judge.
/// The corners come from the packing (spec-0059 §3), which runs here so that
/// every reader of the resolved plan — the checks, the derivation, the battery
/// — holds one grid.
fn resolve<'a>(
    plan: &'a SitePlanContent,
    graph: &LayoutGraphContent,
    table: &Metrics,
    reads: &mut Reads,
    d: &mut Vec<Diagnostic>,
) -> (Vec<Placed<'a>>, Packed) {
    let datums: BTreeMap<&str, i64> = plan.datums.iter().map(|x| (x.id.0.as_str(), x.y)).collect();
    // Whichever of the two classifications the node declared. `DW0875` is what
    // refuses a node that declared both or neither; this map takes the size
    // class first so that a node which slipped past with both is judged against
    // one of the two rather than against neither — a refused campaign builds
    // nothing either way, and a check that quietly examines zero boxes is the
    // shape worth avoiding.
    let classes: BTreeMap<&str, (MetricKind, &str)> = graph
        .nodes
        .iter()
        .filter_map(|n| {
            let named = n
                .size_class
                .as_deref()
                .map(|x| (MetricKind::SizeClass, x))
                .or_else(|| n.way_class.as_deref().map(|x| (MetricKind::WayClass, x)))?;
            Some((n.id.0.as_str(), named))
        })
        .collect();
    // Floors first: the packing needs them for every sill, and a box with no
    // plane has no cells for any reader to work in.
    let mut floors: Vec<Option<i64>> = Vec::with_capacity(plan.boxes.len());
    for (i, b) in plan.boxes.iter().enumerate() {
        floors.push(match &b.floor {
            Floor::Y(y) => Some(*y),
            Floor::Datum(id) => match datums.get(id.0.as_str()) {
                Some(y) => Some(*y),
                None => {
                    d.push(Diagnostic::error(
                        crate::codes::DANGLING_REF,
                        "site-plan",
                        format!("/content/boxes/{i}/floor"),
                        format!(
                            "box for `{node}` stands on `{id}`, which this plan declares no \
                             `datums[]` entry for. Declare the plane, or give the box its own \
                             `y` — a place with no plane has no walk surface, so nothing below \
                             can say where it is.",
                            node = b.node,
                        ),
                    ));
                    None
                }
            },
        });
    }
    let packed = pack(plan, graph, table, &floors, reads, d);
    let mut out = Vec::new();
    for (i, b) in plan.boxes.iter().enumerate() {
        let (Some(floor), Some(pb)) = (floors[i], &packed.boxes[i]) else {
            continue; // `DW0112` or `DW0883` said why this box has no cells.
        };
        let class = classes
            .get(b.node.0.as_str())
            .and_then(|(kind, name)| table.resolve(*kind, name).ok())
            .and_then(|entry| match entry.value(reads) {
                MetricValue::SizeClass(sc) => Some(PlaceClass::Size(*sc)),
                MetricValue::WayClass(w) => Some(PlaceClass::Way(*w)),
                _ => None,
            });
        let clearance = match b.ceiling {
            Ceiling::Clearance(c) => Some(c.get()),
            // A sky-open place claims its class's own minimum headroom and
            // nothing above it: an open place is precisely one that makes no
            // claim on the air over it. True of both kinds of class, which is
            // why the question is asked of the classification rather than of
            // one of its variants.
            Ceiling::Open => class.map(PlaceClass::min_clearance),
        };
        out.push(Placed {
            index: i,
            plan: b,
            foot: [
                pb.min[0],
                pb.min[0] + i64::from(b.extent[0].get()) - 1,
                pb.min[1],
                pb.min[1] + i64::from(b.extent[1].get()) - 1,
            ],
            floor,
            clearance,
            class,
            by: pb.by.clone(),
        });
    }
    (out, packed)
}
