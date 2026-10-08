//! The plan resolved once — every box's footprint, plane, headroom and class —
//! and the resolved plan in world cells that the checks, the derivation and the
//! battery all read: placed boxes, placed seams, and the stair run.

use super::*;

// ---------------------------------------------------------------------------
// Resolution — the plan read once, so no check re-derives a number
// ---------------------------------------------------------------------------

/// One box with everything the checks below need already worked out.
///
/// **The model every geometric check depends on** is stated where an author
/// reads it — on [`PlanBox`], whose schema description carries it — and the
/// number itself is [`SHARED_FACE_GAP_CELLS`]. In short: a box is the **play
/// space** of a place, the shell stands in the one-cell gap between two
/// neighbours, `extent` is therefore the interior footprint the size-class
/// ladder judges directly (`DW0832`), and two connected places sit exactly
/// [`SHARED_FACE_GAP_CELLS`] apart on the face they share (`DW0828`).
#[derive(Debug, Clone)]
pub(super) struct Placed<'a> {
    pub(super) index: usize,
    pub(super) plan: &'a PlanBox,
    /// Footprint, inclusive: `[x0, x1, z0, z1]`.
    pub(super) foot: [i64; 4],
    /// The walk plane.
    pub(super) floor: i64,
    /// Cells of headroom over the walk plane, or `None` when the place is
    /// sky-open and its classification did not resolve.
    pub(super) clearance: Option<u32>,
    /// How the place is classified, when the name resolved.
    pub(super) class: Option<PlaceClass>,
    /// How its corner was obtained (spec-0059 §3).
    pub(super) by: Provenance,
}

/// **How a place is classified** — the two kinds of standard a box is judged
/// against (spec-0053 §3).
///
/// The classification belongs to the PLACE, so it is one field of two kinds
/// rather than two fields. Written as a second `Option<WayClass>` beside the
/// first, every consumer would have had to remember to look at both, and the
/// one that forgot would silently judge a road against nothing — which is the
/// state this whole surface exists to end, reintroduced one layer down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlaceClass {
    /// A rung of the size ladder: both horizontal extents bounded.
    Size(SizeClass),
    /// A way: the cross-section bounded, the run free.
    Way(WayClass),
}

impl PlaceClass {
    /// The least interior clearance the class demands.
    ///
    /// The one question both kinds answer identically, which is why a sky-open
    /// box needs no arm: an open place claims exactly its class's minimum
    /// headroom and nothing above it, and that sentence is true of a road as it
    /// is of a hall.
    pub(super) fn min_clearance(self) -> u32 {
        match self {
            PlaceClass::Size(c) => c.min_clearance,
            PlaceClass::Way(w) => w.min_clearance,
        }
    }
}

impl Placed<'_> {
    pub(super) fn x0(&self) -> i64 {
        self.foot[0]
    }
    pub(super) fn x1(&self) -> i64 {
        self.foot[1]
    }
    pub(super) fn z0(&self) -> i64 {
        self.foot[2]
    }
    pub(super) fn z1(&self) -> i64 {
        self.foot[3]
    }

    /// The inclusive vertical span of the play space, when it is bounded.
    pub(super) fn y_span(&self) -> Option<(i64, i64)> {
        let c = i64::from(self.clearance?);
        Some((self.floor, self.floor + c - 1))
    }

    /// The centre of the footprint, in blocks.
    pub(super) fn centre_xz(&self) -> (f64, f64) {
        (
            (self.x0() as f64 + self.x1() as f64) / 2.0,
            (self.z0() as f64 + self.z1() as f64) / 2.0,
        )
    }
}

// ---------------------------------------------------------------------------
// The resolved plan, in world cells — ONE authority, three readers
// ---------------------------------------------------------------------------

/// One place, resolved into world cells: the play space the plan gives it.
///
/// Public because three readers need the same answer and two of them are in
/// another crate: the stage-4 checks here, the **blockout derivation** that
/// builds the mass, and the **stage-5 battery** that judges the built bytes
/// against the plan. Two of those computing "where is this box" independently is
/// how a builder and its observer come to agree about a world neither of them
/// describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedBox {
    /// The place this embeds.
    pub node: NodeId,
    /// Inclusive footprint `[x0, x1, z0, z1]`.
    pub foot: [i64; 4],
    /// The walk plane's world `y`.
    pub floor: i64,
    /// Cells of headroom over the walk plane.
    pub clearance: u32,
    /// True when the plan declared no ceiling — a courtyard, a shore, a summit.
    /// The place still claims its size class's own minimum headroom (which is
    /// what [`PlacedBox::clearance`] holds); what it makes no claim on is the
    /// air above that.
    pub open: bool,
}

impl PlacedBox {
    /// The play space's inclusive world AABB.
    #[must_use]
    pub fn space(&self) -> ([i64; 3], [i64; 3]) {
        (
            [self.foot[0], self.floor, self.foot[2]],
            [
                self.foot[1],
                self.floor + i64::from(self.clearance) - 1,
                self.foot[3],
            ],
        )
    }

    /// The floor centre — where a body seated in this place stands.
    #[must_use]
    pub fn centre(&self) -> [i64; 3] {
        [
            (self.foot[0] + self.foot[1]) / 2,
            self.floor,
            (self.foot[2] + self.foot[3]) / 2,
        ]
    }
}

/// One connection, resolved into world cells: the wall the two places share and
/// the hole the plan cut in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedSeam {
    /// The connection this allocates.
    pub edge: EdgeId,
    /// Its class, as the graph spells it.
    pub class: &'static str,
    /// The `a` end.
    pub a: NodeId,
    /// The `b` end.
    pub b: NodeId,
    /// Which face **of `a`** the seam sits on.
    pub face: Face,
    /// The axis the shared wall is flat in: 0 = x, 1 = y, 2 = z.
    pub normal_axis: usize,
    /// The wall's coordinate on that axis — one cell thick, so one number.
    pub plane: i64,
    /// The opening's inclusive world AABB (flat in [`Self::normal_axis`]).
    pub opening: ([i64; 3], [i64; 3]),
    /// The whole rectangle the two boxes share on that wall, inclusive.
    pub shared: ([i64; 3], [i64; 3]),
    /// Which kind of connection this seam allocates (spec-0053 §4). A portal's
    /// `opening` is a standard's rectangle; a contact's is its span.
    pub crossing: Crossing,
    /// `floor(b) − floor(a)`, derived — never authored (see [`Seam`]).
    pub rise: i64,
    /// Which place hosts the stair massing, on a `stair`.
    pub stair_in: Option<NodeId>,
}

/// The plan's boxes, resolved by the code the stage-4 checks judge with.
///
/// A box whose floor names an undeclared datum is **absent** — `DW0112` has
/// refused it, and a place with no plane has no cells for any reader to work in.
/// A sky-open box whose size class did not resolve is absent for the same reason
/// (`DW0812` refused the class, so the plan states no headroom for it at all).
#[must_use]
pub fn placed_boxes(c: &Campaign, reads: &mut Reads) -> Vec<PlacedBox> {
    let (Some(plan), Some(graph)) = (
        c.site_plan.as_ref().map(|p| &p.content),
        c.layout_graph.as_ref().map(|g| &g.content),
    ) else {
        return Vec::new();
    };
    let table = Metrics::table();
    let mut sink = Vec::new();
    resolve(plan, graph, &table, reads, &mut sink)
        .0
        .into_iter()
        .filter_map(|p| {
            Some(PlacedBox {
                node: p.plan.node.clone(),
                foot: p.foot,
                floor: p.floor,
                clearance: p.clearance?,
                open: matches!(p.plan.ceiling, Ceiling::Open),
            })
        })
        .collect()
}

/// **The one place a seam's crossing rectangle is computed**, for either kind
/// of connection (spec-0053 §4).
///
/// A portal's rectangle is the named standard's `width × height` anchored at
/// `at`. A contact's is its span: `at` plus the declared `extent`, or `at` to
/// the far edge of the shared face when no extent is declared.
///
/// One function rather than one per kind, and one call rather than a copy in
/// each reader, because this rectangle is simultaneously the derivation's carve,
/// `DW0836`'s allocation, `DW0838`'s allocation set and `DW0877`'s span. Two
/// implementations of it would be a plan-time green and a byte-time green about
/// two different rectangles, which is the defect `shared_face` already has one
/// implementation to prevent.
///
/// `None` when the seam names an opening the table does not define — `DW0812`
/// refused it and there is no rectangle to build or measure.
pub(super) fn crossing_rect(
    s: &Seam,
    at: [i64; 2],
    face: &SharedFace,
    table: &Metrics,
    reads: &mut Reads,
) -> Option<(Crossing, [i64; 2])> {
    if s.contact.is_some() {
        return Some((Crossing::Contact, contact_extent(s, at, face)));
    }
    let named = s.opening.as_ref()?;
    let entry = table.resolve(MetricKind::Opening, named).ok()?;
    match entry.value(reads) {
        MetricValue::Opening(o) => {
            Some((Crossing::Portal, [i64::from(o.width), i64::from(o.height)]))
        }
        _ => None,
    }
}

/// **How big a contact's span is** — the one authority, read by
/// [`crossing_rect`] and by the refusal that judges it (`DW0876`).
///
/// A declared `extent` is taken as written. With none declared the span runs
/// from `at` to the far edge of the shared face on both axes, which is how a
/// contact along the whole of a face is spelled. An `at` already past that edge
/// would give a negative extent, so it is clamped to one cell: the rectangle
/// stays well-formed and `DW0876` describes it, rather than the arithmetic
/// producing a rectangle nothing downstream could reason about.
pub(super) fn contact_extent(s: &Seam, at: [i64; 2], face: &SharedFace) -> [i64; 2] {
    match s.contact.as_ref().and_then(|c| c.extent) {
        Some(e) => [i64::from(e[0].get()), i64::from(e[1].get())],
        None => [(face.u.1 - at[0] + 1).max(1), (face.v.1 - at[1] + 1).max(1)],
    }
}

/// Which axis a face is flat in: 0 = x, 1 = y, 2 = z.
#[must_use]
pub fn normal_axis_of(face: Face) -> usize {
    match face {
        Face::East | Face::West => 0,
        Face::Up | Face::Down => 1,
        Face::South | Face::North => 2,
    }
}

/// A seam's crossing rectangle as an inclusive world AABB, flat in the face's
/// normal axis — [`crossing_rect`]'s two numbers put where the world is.
///
/// Extracted rather than written twice because [`stair_run`] needs the same
/// rectangle at validation tier, before any `PlacedSeam` exists.
pub(super) fn crossing_aabb(
    s: &Seam,
    at: [i64; 2],
    face: &SharedFace,
    extent: [i64; 2],
) -> ([i64; 3], [i64; 3]) {
    let normal_axis = normal_axis_of(s.face);
    let (u_axis, v_axis) = in_plane_axes(s.face);
    let mut lo = [0i64; 3];
    let mut hi = [0i64; 3];
    lo[normal_axis] = face.plane;
    hi[normal_axis] = face.plane;
    lo[u_axis] = at[0];
    hi[u_axis] = at[0] + extent[0] - 1;
    lo[v_axis] = at[1];
    hi[v_axis] = at[1] + extent[1] - 1;
    (lo, hi)
}

/// **What a stair costs the box that hosts it** — the one place the geometry of
/// a run is worked out, for both the check that refuses a plan (`DW0830`) and
/// the derivation that lays the treads.
///
/// # Why this is one function and not two
///
/// It was two, and the two disagreed. `DW0830` measured the run against the
/// seam's **rise** and against the host's whole **extent**; the derivation
/// measures it against the **climb** the opening really asks for and against
/// the run the host really has beside the hole. Both readings are defensible
/// in isolation and neither is the other, so a plan could pass the check by one
/// arithmetic and be refused by the other at build time — the treads then went
/// unlaid, and the place they were the only way into came back as `DW0837` with
/// nothing pointing at the stair. Found by building: a two-place plan whose
/// lower room is entered through a hole in the upper room's floor reached green
/// at stage 4 with `needs 8, affords 8` and built no stair at all, because the
/// climb to the pierced plane is 7 and the run beside a centred hole is 6.
///
/// So the rule lives here and both readers call it. A plan that reaches green
/// is a plan the derivation can build, by construction rather than by two
/// arithmetics agreeing.
///
/// # What the numbers mean
///
/// * `climb` — how high the courses must carry a body. Across a **vertical**
///   face that is the seam's own sill, because the body stands at the sill and
///   steps through; through a **floor or ceiling** it is the pierced plane,
///   because the body stands in the hole and steps out beside it. Both come out
///   as the floor difference on an ordinary plan and differ exactly where the
///   plan puts the sill somewhere other than the far floor.
/// * `run_axis` — the horizontal axis the run is spent on. Across a vertical
///   face it is that face's normal; through a floor or ceiling the run may go
///   either way, so it is the host's longer horizontal axis.
/// * `start`/`step` — the stair arrives AT its seam, so course 0 is the one the
///   body steps off and the run walks back into the room.
/// * `available` — how many cells of that walk the host really has. Across a
///   vertical face the run walks the whole footprint. Through a floor or a
///   ceiling it leaves along **one** side of the hole, so it is the room on the
///   roomier side plus the hole's own width — never the host's whole extent,
///   which is a run that only exists if the stair could run both ways at once.
///
/// `None` where there is no run to lay or judge: a host at or above what the
/// stair has to reach (the plan named the higher place, which `DW0830` refuses
/// by name), or a hole that is not over this host at all (`DW0828`'s finding).
#[must_use]
pub fn stair_run(
    host_floor: i64,
    host_foot: [i64; 4],
    normal_axis: usize,
    plane: i64,
    opening: ([i64; 3], [i64; 3]),
) -> Option<StairRun> {
    let (olo, ohi) = opening;
    let lo = [host_foot[0], host_floor, host_foot[2]];
    let hi = [host_foot[1], host_floor, host_foot[3]];
    let target = if normal_axis == 1 { plane } else { olo[1] };
    let climb = target - host_floor;
    if climb <= 0 {
        return None;
    }
    let run_axis = if normal_axis == 1 {
        let ex = host_foot[1] - host_foot[0] + 1;
        let ez = host_foot[3] - host_foot[2] + 1;
        if ex >= ez { 0usize } else { 2 }
    } else {
        normal_axis
    };
    let (start, step, available) = if normal_axis == 1 {
        let (olo_r, ohi_r) = (
            olo[run_axis].max(lo[run_axis]),
            ohi[run_axis].min(hi[run_axis]),
        );
        if olo_r > ohi_r {
            return None;
        }
        let width = ohi_r - olo_r + 1;
        let room_lo = olo_r - lo[run_axis];
        let room_hi = hi[run_axis] - ohi_r;
        if room_lo >= room_hi {
            (ohi_r, -1, room_lo + width)
        } else {
            (olo_r, 1, room_hi + width)
        }
    } else if plane > hi[run_axis] {
        (hi[run_axis], -1, hi[run_axis] - lo[run_axis] + 1)
    } else {
        (lo[run_axis], 1, hi[run_axis] - lo[run_axis] + 1)
    };
    Some(StairRun {
        climb,
        run_axis,
        start,
        step,
        available,
    })
}

/// What a stair's treads cost their host — see [`stair_run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StairRun {
    /// How high the courses must carry a body, off the host's own walk plane.
    pub climb: i64,
    /// The horizontal axis the run walks: 0 = x, 2 = z.
    pub run_axis: usize,
    /// Where course 0 — the one the body steps off — stands on `run_axis`.
    pub start: i64,
    /// Which way the run walks back into the host: `+1` or `-1`.
    pub step: i64,
    /// How many cells of that walk the host really affords.
    pub available: i64,
}

/// The run one standard pitch costs a climb, rounded up: a course is a whole
/// cell, and a run two-thirds of a cell short is a run the host does not have.
#[must_use]
pub fn run_of(pitch: &Pitch, climb: i64) -> i64 {
    let (span, per) = (climb.abs() * i64::from(pitch.run), i64::from(pitch.rise));
    if per == 0 {
        span
    } else {
        span / per + i64::from(span % per != 0)
    }
}

/// **The gentlest standard pitch a climb fits in `available` cells of run**, or
/// `None` when the table defines none that does.
///
/// The walk is over [`Metrics::names_of`] in table order — gentlest first — and
/// takes the first that fits. One authority, so the verdict `DW0830` reaches and
/// the geometry the derivation lays cannot be about different standards.
#[must_use]
pub fn gentlest_pitch(
    table: &Metrics,
    reads: &mut Reads,
    climb: i64,
    available: i64,
) -> Option<Pitch> {
    for name in table.names_of(MetricKind::Pitch) {
        let Ok(entry) = table.resolve(MetricKind::Pitch, name) else {
            continue;
        };
        let MetricValue::Pitch(p) = entry.value(reads) else {
            continue;
        };
        if p.rise == 0 {
            continue;
        }
        if run_of(p, climb) <= available {
            return Some(*p);
        }
    }
    None
}

/// The standard pitch that costs a climb the LEAST run, and what that run is —
/// the number `DW0830`'s refusal quotes, because it is the shortest run any
/// standard could do the climb in.
#[must_use]
pub fn tightest_pitch(
    table: &Metrics,
    reads: &mut Reads,
    climb: i64,
) -> Option<(&'static str, i64)> {
    let mut best: Option<(&'static str, i64)> = None;
    for name in table.names_of(MetricKind::Pitch) {
        let Ok(entry) = table.resolve(MetricKind::Pitch, name) else {
            continue;
        };
        let MetricValue::Pitch(p) = entry.value(reads) else {
            continue;
        };
        if p.rise == 0 {
            continue;
        }
        let needed = run_of(p, climb);
        if best.is_none_or(|(_, b)| needed < b) {
            best = Some((name, needed));
        }
    }
    best
}

/// The plan's seams, resolved by the code the stage-4 checks judge with.
///
/// A seam whose face the two boxes do not share, or whose opening the table does
/// not define, is **absent**: `DW0828`/`DW0812` refused it, and there is no hole
/// for a reader to build or measure.
#[must_use]
pub fn placed_seams(c: &Campaign, boxes: &[PlacedBox], reads: &mut Reads) -> Vec<PlacedSeam> {
    let (Some(plan), Some(graph)) = (
        c.site_plan.as_ref().map(|p| &p.content),
        c.layout_graph.as_ref().map(|g| &g.content),
    ) else {
        return Vec::new();
    };
    let table = Metrics::table();
    let by_node: BTreeMap<&str, &PlacedBox> =
        boxes.iter().map(|b| (b.node.0.as_str(), b)).collect();
    let edges: BTreeMap<&str, &Edge> = graph.edges.iter().map(|e| (e.id().0.as_str(), e)).collect();
    // The seam anchors come from the same packing that placed `boxes`: one
    // arithmetic, so the derivation and its observer cannot disagree about
    // where a hole is.
    let mut sink = Vec::new();
    let (_, packed) = resolve(plan, graph, &table, reads, &mut sink);
    let mut out = Vec::new();
    for (i, s) in plan.seams.iter().enumerate() {
        let Some(at) = packed.seam_at[i] else {
            continue; // the packing refused or could not place this seam
        };
        let Some(edge) = edges.get(s.edge.0.as_str()) else {
            continue;
        };
        if !edge.has_seam() {
            continue;
        }
        let (Some(a), Some(b)) = (
            by_node.get(edge.a().0.as_str()).copied(),
            by_node.get(edge.b().0.as_str()).copied(),
        ) else {
            continue;
        };
        let Ok(face) = shared_face_of(a, b, s.face) else {
            continue;
        };
        let Some((crossing, extent)) = crossing_rect(s, at, &face, &table, reads) else {
            continue;
        };
        let normal_axis = normal_axis_of(s.face);
        // The face's two in-plane axes, in the order `at` names them.
        let (u_axis, v_axis) = in_plane_axes(s.face);
        let (lo, hi) = crossing_aabb(s, at, &face, extent);
        let mut smin = [0i64; 3];
        let mut smax = [0i64; 3];
        smin[normal_axis] = face.plane;
        smax[normal_axis] = face.plane;
        smin[u_axis] = face.u.0;
        smax[u_axis] = face.u.1;
        smin[v_axis] = face.v.0;
        smax[v_axis] = face.v.1;
        out.push(PlacedSeam {
            edge: s.edge.clone(),
            class: edge.class(),
            a: edge.a().clone(),
            b: edge.b().clone(),
            face: s.face,
            normal_axis,
            plane: face.plane,
            opening: (lo, hi),
            shared: (smin, smax),
            crossing,
            rise: b.floor - a.floor,
            stair_in: s.stair_in.clone(),
        });
    }
    out
}

/// **A contact**: the span of a shared face along which two places simply meet
/// (spec-0053 §4).
///
/// # What a contact MEANS
///
/// The boundary is continuous ground. The derivation writes **no wall along the
/// span** — and wall as ever outside it — and crossing is legitimate anywhere
/// along it the step rule admits. It is not a wide door: `DW0829`'s standard-name
/// resolution and sill rule are portal checks and do not apply, because a
/// contact has no opening name to resolve and no single sill. Calling a 55-cell
/// front a door would make every downstream door check wrong.
///
/// # What the author allocates and what the engine measures
///
/// The author allocates **where** the places meet. The engine measures the
/// **crossing profile** from assembled bytes — which columns of the span a body
/// actually crosses under the step rule — and `DW0877` refuses a contact nothing
/// can cross. *"This face is fine"* is never a declaration this engine accepts.
///
/// Seams stay **allocated, never discovered**: the span is the edge's allocation
/// set for `DW0838`, so a crossing outside it is still a refusal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    /// The span, `[u, v]` in cells on the face's own two in-plane axes, anchored
    /// at the seam's `at`.
    ///
    /// **Omitted, the span runs from `at` to the far edge of the shared face on
    /// both axes** — which is how a contact along the whole of a face is
    /// written, by putting `at` at the face's low corner. `DW0828`'s refusal
    /// prints that corner, so the number an author needs is in the message they
    /// would already be reading.
    ///
    /// There is no `width` standard and no `length` here, and both absences are
    /// the design (spec-0053 §7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<[NonZeroU32; 2]>,
}

/// **Which kind of connection a seam allocates** (spec-0053 §4).
///
/// Carried on the resolved seam rather than re-derived from the authored one at
/// each reader, so that the derivation and the byte observer cannot disagree
/// about which kind a seam is — the same reason `shared_face` has one
/// implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Crossing {
    /// A standard opening. Every allocated cell must be passable (`DW0836`).
    Portal,
    /// A front where two places meet. No wall along the span, and **at least
    /// one** passable column of body width somewhere in it (`DW0877`) — not
    /// every cell, because a contact is ground rather than a hole and the
    /// massing standing on it is content.
    Contact,
}

/// The two world axes a face's `at` names, in that order.
fn in_plane_axes(face: Face) -> (usize, usize) {
    match face {
        // `[along, y]` — `z` for east/west, `x` for north/south.
        Face::East | Face::West => (2, 1),
        Face::South | Face::North => (0, 1),
        // `[x, z]`.
        Face::Up | Face::Down => (0, 2),
    }
}

/// A footprint's inclusive span on one WORLD axis (0 = x, 2 = z). Axis 1 has no
/// answer here — a footprint is horizontal — and no caller asks for it.
pub(super) fn span(foot: [i64; 4], axis: usize) -> (i64, i64) {
    if axis == 0 {
        (foot[0], foot[1])
    } else {
        (foot[2], foot[3])
    }
}

/// Inclusive overlap of two ranges, or `None`.
pub(super) fn overlap(a: (i64, i64), b: (i64, i64)) -> Option<(i64, i64)> {
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    (lo <= hi).then_some((lo, hi))
}

/// Is `[lo, hi]` inside `[within_lo, within_hi]`?
pub(super) fn within(r: (i64, i64), w: (i64, i64)) -> bool {
    r.0 >= w.0 && r.1 <= w.1
}

/// The region's inclusive span on one axis.
pub(super) fn region_span(region: &WorldBox, axis: usize) -> (i64, i64) {
    (region.min[axis], region.max()[axis])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The run beside a pierced floor is not the host's extent**, and this is
    /// the arithmetic the whole one-authority repair turns on.
    ///
    /// An eight-by-eight host with a three-wide hole cut through the ceiling
    /// above it: the treads leave along ONE side of the hole, so what they have
    /// is the room on the roomier side plus the hole's own width. Centre the
    /// hole and that is 3 + 3 = 6, not 8 — and the courses have to carry the
    /// body to the pierced plane, which is one below the floor of the place
    /// above it.
    ///
    /// The numbers are read off the geometry by hand rather than recomputed
    /// here, so a change to the rule shows up as a failure rather than as two
    /// arithmetics agreeing.
    #[test]
    fn a_run_beside_a_pierced_floor_is_the_room_on_one_side_plus_the_hole() {
        // Host x 4..11, z 4..11, walk plane y 56; the hole is cut at y 63 and
        // spans x 7..9 — three of the eight cells, three to the low side and
        // two to the high.
        let run = stair_run(56, [4, 11, 4, 11], 1, 63, ([7, 63, 6], [9, 63, 8]))
            .expect("the hole is over this host and the plane is above its floor");
        assert_eq!(run.climb, 7, "56 up to the pierced plane at 63");
        assert_eq!(run.run_axis, 0, "the host is square, so the run takes x");
        assert_eq!(
            run.available, 6,
            "three cells of room on the low side, plus the hole's three"
        );
        assert_eq!(
            run.start, 9,
            "course 0 stands under the far edge of the hole"
        );
        assert_eq!(run.step, -1, "and the run walks back into the room");
    }

    /// The same host, the same hole, and the check and the derivation asking the
    /// same question: seven courses do not fit six cells, so no standard pitch
    /// does either. Measured against the host's extent it would — which is the
    /// green a plan used to reach before building nothing.
    #[test]
    fn no_standard_pitch_fits_a_climb_of_seven_in_six_cells_of_run() {
        let table = Metrics::table();
        let mut reads = Reads::default();
        assert!(gentlest_pitch(&table, &mut reads, 7, 6).is_none());
        assert!(gentlest_pitch(&table, &mut reads, 7, 7).is_some());
        assert_eq!(tightest_pitch(&table, &mut reads, 7), Some(("stair", 7)));
    }

    /// Across a vertical face the run walks the whole footprint and the climb is
    /// the seam's own sill — the body stands at the sill and steps through.
    #[test]
    fn a_run_across_a_vertical_face_is_the_whole_footprint() {
        // Host x 4..11, z 4..11, walk plane y 64; the wall is at x 12 and the
        // opening's low corner is at y 72.
        let run = stair_run(64, [4, 11, 4, 11], 0, 12, ([12, 72, 6], [12, 74, 8]))
            .expect("the wall is beyond the host, so the run walks back from it");
        assert_eq!(run.climb, 8, "64 up to the sill at 72");
        assert_eq!(run.run_axis, 0);
        assert_eq!(run.available, 8, "the host's whole extent on x");
        assert_eq!(run.start, 11, "course 0 stands against the wall");
        assert_eq!(run.step, -1);
    }
}
