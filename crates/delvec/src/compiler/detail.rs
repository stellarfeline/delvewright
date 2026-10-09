//! **Stage 6: a place is detailed inside the box the whole gave it**
//! (spec-0050).
//!
//! The document is `delvewright_dsl::detailplan`, and its whole surface is
//! *which piece stands in which place*. This module is everything that surface
//! is judged by, and the one computation that turns a binding into bytes.
//!
//! # The frame is computed, never authored
//!
//! A `details[]` row says a node's name and a prefab's name. Where the piece
//! goes is [`delvewright_dsl::Frame::of`] over the site plan's own resolved box
//! — the play space grown one course downward — and there is no field anywhere
//! that could nudge it. The only path from a row to placed bytes is
//! [`place`], called from [`crate::compiler::plan::Plan::build`], which is the only
//! constructor every world-reaching verb goes through. That is the same tooth
//! the blockout's is: inversion is not forbidden, it is **uncompilable**.
//!
//! **Frame equality is exact, and undersize refuses exactly as oversize does**
//! (`DW0843`). A part that under-fills its allocation renegotiates the whole as
//! much as one that overflows it: the box is the footprint, so a smaller
//! building means a smaller box, which is a site-plan edit, taken visibly.
//!
//! # What invokes each check, and what happens without it
//!
//! | check | event it is bound to |
//! |---|---|
//! | `DW0842`–`DW0845` ([`check`]) | `validate_loaded` in `delvec`'s `main` — the one funnel every subcommand's validation goes through, `build` included |
//! | `DW0848` | `delvec prefab audit`, and [`check`] wherever a row consumes the piece |
//! | the frame, and the piece's bytes | [`place`], inside `Plan::build` |
//!
//! There is no flag, no subcommand and no checklist line.
//!
//! # The hatch question, answered
//!
//! This module creates **no** opt-out. A place is bound or unbound, and the kind
//! is determined by whether a `details[]` row exists rather than chosen among
//! demands; there is no acknowledgement field, no exemption list and no severity
//! an author selects.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use delvewright_dsl::detailplan::Frame;
use delvewright_dsl::layout::{Direction, Edge};
use delvewright_dsl::metrics::Reads;
use delvewright_dsl::prefab::ContractFace;
use delvewright_dsl::siteplan::{PlacedBox, PlacedSeam, SitePlan};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, NodeId};

use crate::compiler::plan::PiecePlacement;
use crate::compiler::registry::PrefabRegistry;
use crate::compiler::solver::Rotation;

/// The stage name every diagnostic here carries — the document being judged.
const STAGE: &str = "detail-plan";

delvewright_dsl::dw_code! {
    /// `DW0842`: the binding does not bind.
    pub const DW_BINDING: DwCode = DwCode::new("DW0842", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0843`: the piece is not the shape of its allocation.
    pub const DW_NOT_THE_FRAME: DwCode = DwCode::new("DW0843", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0844`: the piece's openings are not the plan's seams.
    pub const DW_FACES: DwCode = DwCode::new("DW0844", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0845`: an owed anchor has no standing.
    pub const DW_ANCHOR_STANDING: DwCode = DwCode::new("DW0845", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0987`: a piece paints a cell it does not own (spec-0098 §7).
    pub const DW_PAINTS_NEIGHBOUR: DwCode = DwCode::new("DW0987", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0989`: a roofed place bound to a piece that encloses nothing
    /// (spec-0098 §6b).
    pub const DW_ROOFED_ENCLOSES_NOTHING: DwCode = DwCode::new("DW0989", ExitTier::Build);
}

// ---------------------------------------------------------------------------
// The frames, and what they hand out
// ---------------------------------------------------------------------------

/// Every place's frame, by node name, in plan document order.
#[must_use]
pub fn frames(c: &Campaign) -> Vec<(Frame, PlacedBox)> {
    Frame::all(c)
}

/// The seams that touch `node`, with the direction they leave its frame by.
fn seams_of<'a>(seams: &'a [PlacedSeam], node: &NodeId) -> Vec<(&'a PlacedSeam, [i64; 3])> {
    seams
        .iter()
        .filter_map(|s| {
            let v = s.face.vector();
            if &s.a == node {
                Some((s, v))
            } else if &s.b == node {
                Some((s, [-v[0], -v[1], -v[2]]))
            } else {
                None
            }
        })
        .collect()
}

/// **Which cell layer of the frame answers this seam** (spec-0098 §2, §6b).
///
/// The place that owns the seam's plane answers **at the plane itself**: the
/// opening is its to cut. The other place answers at its own first layer on its
/// side of the plane — the play space's boundary across a wall, the top course
/// under a stacked floor. So two seams of one place meet only where their
/// planes do, and an owner's two openings at a corner are disjoint by
/// construction.
fn answering_layer(frame: &Frame, s: &PlacedSeam) -> i64 {
    let a = s.normal_axis;
    if frame.owns(s.opening.0) {
        return s.plane;
    }
    // This place's side of the plane: where its frame lies.
    let mid = (frame.lo[a] + frame.hi[a]) / 2;
    if mid < s.plane {
        s.plane - 1
    } else {
        s.plane + 1
    }
}

/// The graph edge a seam allocates, when the graph has it.
fn edge_of<'a>(c: &'a Campaign, s: &PlacedSeam) -> Option<&'a Edge> {
    c.layout_graph
        .as_ref()
        .map(|g| &g.content)?
        .edges
        .iter()
        .find(|e| e.id() == &s.edge)
}

/// **The class of face the piece must answer a seam with** — spec-0050 §3's
/// table, keyed to the geometry rather than chosen by anyone.
///
/// It takes the graph EDGE rather than the campaign, and that is not tidying: one
/// row of the table — a `barred` seam lying in the piece's own floor course — is
/// reached by no site plan this repository ships, so with a `&Campaign` argument
/// it could only be exercised by writing a whole campaign to reach one line. A
/// function of a seam, a frame and an edge is a function `table_covers_every_row`
/// can call directly, and a row nobody has checked is a row that is wrong the
/// first time somebody authors it.
fn required_face_class(
    edge: Option<&Edge>,
    s: &PlacedSeam,
    node: &NodeId,
    frame: &Frame,
) -> Vec<&'static str> {
    match s.class {
        "walk" => vec!["walk"],
        "stair" => {
            if s.stair_in.as_ref() == Some(node) {
                // The treads are the piece's; a piece that meets the opening at
                // grade answers with a `walk` instead, and both are correct.
                vec!["stair", "walk"]
            } else {
                vec!["walk"]
            }
        }
        "drop" => {
            let Some(Edge::Drop { falls, .. }) = edge else {
                return vec!["walk"];
            };
            let leaving = match falls {
                Direction::AToB => &s.a,
                Direction::BToA => &s.b,
            };
            if leaving == node {
                vec!["drop"]
            } else {
                vec!["walk"]
            }
        }
        "barred" => {
            // The piece that owns the plane ships the gate's shut state
            // (spec-0098 §2); the other side answers with the way it opens onto.
            if frame.owns(s.opening.0) {
                vec!["barred"]
            } else {
                vec!["walk"]
            }
        }
        _ => vec!["walk"],
    }
}

/// The seam's opening projected onto the frame layer that answers it — the cells
/// the piece's face must cover.
fn answering_cells(frame: &Frame, s: &PlacedSeam) -> ([i64; 3], [i64; 3]) {
    let a = s.normal_axis;
    let layer = answering_layer(frame, s);
    let (mut lo, mut hi) = s.opening;
    lo[a] = layer;
    hi[a] = layer;
    (lo, hi)
}

/// A declared face's world AABB, for a piece placed unrotated at `frame.lo`.
fn face_world(frame: &Frame, f: &ContractFace) -> ([i64; 3], [i64; 3]) {
    let lo = [
        frame.lo[0] + i64::from(f.opening.from[0].min(f.opening.to[0])),
        frame.lo[1] + i64::from(f.opening.from[1].min(f.opening.to[1])),
        frame.lo[2] + i64::from(f.opening.from[2].min(f.opening.to[2])),
    ];
    let hi = [
        frame.lo[0] + i64::from(f.opening.from[0].max(f.opening.to[0])),
        frame.lo[1] + i64::from(f.opening.from[1].max(f.opening.to[1])),
        frame.lo[2] + i64::from(f.opening.from[2].max(f.opening.to[2])),
    ];
    (lo, hi)
}

fn dir_of(name: &str) -> Option<[i64; 3]> {
    Some(match name {
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        "south" => [0, 0, 1],
        "north" => [0, 0, -1],
        _ => return None,
    })
}

fn dir_name(v: [i64; 3]) -> &'static str {
    match v {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        _ => "north",
    }
}

fn aabb(lo: [i64; 3], hi: [i64; 3]) -> String {
    format!(
        "x {}..{} y {}..{} z {}..{}",
        lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
    )
}

// ---------------------------------------------------------------------------
// The handing (spec-0050 §4)
// ---------------------------------------------------------------------------

/// One seam, as the allocation hands it to whoever writes the piece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AllocatedSeam {
    /// The connection this allocates.
    pub edge: String,
    /// Its class, as the graph spells it.
    pub class: String,
    /// What the crossing is, as the plan declares it — handed to both places
    /// the seam joins, so each designs its side knowing what meets it.
    pub form: String,
    /// The other place it joins.
    pub other: String,
    /// Which way out of the piece it leaves by.
    pub face: String,
    /// The two opposite corner cells of the opening the piece's answering face
    /// must cover, **piece-local**, inclusive — every cell between them is in it.
    pub cells: [[i64; 3]; 2],
    /// `floor(other) − floor(this)`, in cells.
    pub rise: i64,
    /// The class of face the piece must answer with. More than one where both
    /// are correct — a stair the piece hosts may meet its opening at grade.
    pub answer_with: Vec<String>,
    /// Whether this place owns the plane the opening stands in, and so cuts it
    /// (and, on a `barred` way, ships its shut state).
    pub owns_plane: bool,
    /// The ring's ground under a vertical seam, piece-local: the sill minus one,
    /// flat across the opening (spec-0098 §2c). `None` through a floor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ground_y: Option<i64>,
}

/// What is built here, as the layout graph says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedBrief {
    /// The node's `intent`.
    pub intent: String,
    /// The node's `note`, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// `size <class>` or `way <class>`.
    pub class: String,
    /// The named places inside this one, each `<anchor> (<kind>)`.
    pub stations: Vec<String>,
    /// `roofed` or `open`.
    pub kind: String,
}

/// One approved image of the design record, by path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedImage {
    /// The row's `name` — `design/<name>.<ext>` is the file.
    pub name: String,
    /// What the row says the picture shows.
    pub shows: String,
    /// The time of day it was drawn under.
    pub time: String,
    /// The weather it was drawn under.
    pub weather: String,
}

/// The place's own concept image: its row, or the named absence of one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedConcept {
    /// The row name the place's image is recorded under: `concept/<place stem>`.
    pub name: String,
    /// The row, once the image is approved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<HandedImage>,
    /// Why there is none yet, when there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absent: Option<String>,
}

/// One neighbour, by the side it stands on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Neighbour {
    /// The neighbouring place.
    pub place: String,
    /// The side of this place it stands on.
    pub face: String,
    /// `roofed` or `open`.
    pub kind: String,
    /// Its walk plane's world `y`.
    pub floor: i64,
    /// Its declared roof, `[courses, eaves]`, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roof: Option<[u32; 2]>,
    /// The seams that join the two, by edge.
    pub seams: Vec<String>,
}

/// A plan view that sees this place, and the command that frames it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedView {
    /// The view's id.
    pub id: String,
    /// The view's note, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The `delvec snapshot` invocation that renders it over the whole.
    pub snapshot: String,
}

/// The ground the whole hands the place (spec-0098 §2c), piece-local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedGround {
    /// The site's fill kind, `solid` or `open`.
    pub fill: String,
    /// The claim's bottom, piece-local — the lowest course of ground the piece
    /// owns under its plot.
    pub bottom_y: i64,
    /// The walk plane, piece-local.
    pub floor_y: i64,
    /// The terrain's surface height along the plot's perimeter (the ring's
    /// columns), world `y`: each `[x, z, top]` piece-local `x`/`z`.
    pub perimeter: Vec<[i64; 3]>,
    /// The lowest of those heights, piece-local `y`.
    pub min_y: i64,
    /// The highest, piece-local `y`.
    pub max_y: i64,
    /// Every fixed ring cell: piece-local cell and the block the whole writes
    /// there. The piece holds `structure_void` at each and may write no block.
    pub fixed: Vec<FixedCell>,
    /// The terrain-shaped ground inside the ring, one column per footprint
    /// cell, from the claim's bottom: the piece reshapes it freely.
    pub columns: Vec<GroundColumn>,
}

/// One fixed ring cell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixedCell {
    /// Piece-local cell.
    pub cell: [i64; 3],
    /// The block the whole writes there.
    pub block: String,
}

/// One column of handed ground.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GroundColumn {
    /// Piece-local `x`.
    pub x: i64,
    /// Piece-local `z`.
    pub z: i64,
    /// The terrain's surface height here, piece-local `y` (on a `solid` site,
    /// the floor course).
    pub top: i64,
    /// The blocks from the claim's bottom up to `top` or the floor course,
    /// whichever is lower, bottom first.
    pub blocks: Vec<String>,
}

/// The roof the plan declares over this place, piece-local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedRoof {
    /// Courses above the lid.
    pub courses: u32,
    /// Overhang on every side.
    pub eaves: u32,
    /// The lid's piece-local `y`.
    pub lid_y: i64,
    /// The roof zone's top course, piece-local.
    pub top_y: i64,
    /// Eaves the plan clipped at a neighbour's play space: piece-local corners
    /// and the neighbour.
    pub clipped: Vec<([[i64; 3]; 2], String)>,
}

/// A run of the frame the place does not own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedVoid {
    /// Piece-local corners, inclusive.
    pub cells: [[i64; 3]; 2],
    /// Who writes them.
    pub owner: String,
}

/// **What `delvec allocation` prints — the handout**: everything the whole gives
/// a place, and nothing a piece could give back (spec-0098 §4).
///
/// Derived from the site plan on every invocation and **not an input to
/// anything**. No gate, no build step and no check ever reads an allocation
/// file: `DW0842`–`DW0845`, `DW0987`, `DW0990` and the stage-5 bytes battery
/// recompute every obligation from the plan itself at every validation, so a
/// committed allocation file is a copy with no consumer and its staleness has
/// no vector into the build. It exists for the authoring loop alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Allocation {
    /// The place.
    pub place: String,
    /// What is built here, as the layout graph says it.
    pub brief: HandedBrief,
    /// The whole's material vocabulary, handed through ungated (spec-0050 §4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<BTreeMap<String, String>>,
    /// The place's own concept image.
    pub concept: HandedConcept,
    /// The whole's reference sheet: the design record's `reference/` rows.
    pub sheet: Vec<HandedImage>,
    /// The frame's size in cells, `[x, y, z]` — what the piece must be, exactly.
    pub extent: [i64; 3],
    /// The walk plane's piece-local `y`.
    pub datum_y: i64,
    /// The frame's world position.
    pub world_min: [i64; 3],
    /// The play space's two opposite corner cells, piece-local, inclusive —
    /// where a body is; everything else the place owns is its outside.
    pub space: [[i64; 3]; 2],
    /// Every place whose claim meets this one's, by side.
    pub neighbours: Vec<Neighbour>,
    /// The plan views that see this place.
    pub views: Vec<HandedView>,
    /// The ground the whole hands it.
    pub ground: HandedGround,
    /// The roof the plan declares over it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roof: Option<HandedRoof>,
    /// Every seam of this box, piece-local.
    pub seams: Vec<AllocatedSeam>,
    /// The synthesized names this place owes, which `anchors` must bind.
    pub owed_anchors: Vec<String>,
    /// Every cell of the frame the place does not own, with who does. The
    /// piece holds `structure_void` at each.
    pub voids: Vec<HandedVoid>,
}

fn face_between(b: &PlacedBox, q: &PlacedBox) -> &'static str {
    if q.foot[0] > b.foot[1] {
        "east"
    } else if q.foot[1] < b.foot[0] {
        "west"
    } else if q.foot[2] > b.foot[3] {
        "south"
    } else if q.foot[3] < b.foot[2] {
        "north"
    } else if q.floor > b.floor {
        "up"
    } else {
        "down"
    }
}

fn kind_of(b: &PlacedBox) -> String {
    if b.open { "open" } else { "roofed" }.to_string()
}

/// The `delvec snapshot --camera` value that stands at `eye` looking at `at`.
fn camera(eye: [i64; 3], at: [i64; 3]) -> String {
    let d = [
        (at[0] - eye[0]) as f64,
        (at[1] - eye[1]) as f64,
        (at[2] - eye[2]) as f64,
    ];
    let yaw = (-d[0]).atan2(d[2]).to_degrees();
    let horiz = (d[0] * d[0] + d[2] * d[2]).sqrt();
    let pitch = -(d[1].atan2(horiz)).to_degrees();
    format!("{},{},{},{:.1},{:.1}", eye[0], eye[1], eye[2], yaw, pitch)
}

/// True when a view's eye sees `p`: inside a cone of half-angle 35° about the
/// line from the eye to its `look_at`.
fn view_sees(eye: [i64; 3], at: [i64; 3], p: [i64; 3]) -> bool {
    let v = |a: [i64; 3]| {
        [
            (a[0] - eye[0]) as f64,
            (a[1] - eye[1]) as f64,
            (a[2] - eye[2]) as f64,
        ]
    };
    let (u, w) = (v(at), v(p));
    let dot = u[0] * w[0] + u[1] * w[1] + u[2] * w[2];
    let n = (u.iter().map(|x| x * x).sum::<f64>() * w.iter().map(|x| x * x).sum::<f64>()).sqrt();
    n > 0.0 && dot / n >= 35f64.to_radians().cos()
}

fn image(r: &delvewright_dsl::design::Reference) -> HandedImage {
    let as_str = |v: serde_json::Value| v.as_str().unwrap_or_default().to_string();
    HandedImage {
        name: r.name.clone(),
        shows: r.shows.clone(),
        time: as_str(serde_json::to_value(r.time).unwrap_or_default()),
        weather: as_str(serde_json::to_value(r.weather).unwrap_or_default()),
    }
}

/// The handout for one place, or `None` when the plan has no such box.
#[must_use]
pub fn allocation(c: &Campaign, node: &NodeId) -> Option<Allocation> {
    let plan = SitePlan::of(c);
    let site = plan.site();
    let i = site.index_of(node)?;
    let b = &plan.boxes[i];
    let frame = Frame::of(&site, i);
    let local = |w: [i64; 3]| frame.to_local(w);
    let graph_node = c
        .layout_graph
        .as_ref()
        .and_then(|g| g.content.nodes.iter().find(|n| &n.id == node));
    let brief = HandedBrief {
        intent: graph_node.map(|n| n.intent.clone()).unwrap_or_default(),
        note: graph_node.and_then(|n| n.note.clone()),
        class: graph_node
            .map(|n| match (&n.size_class, &n.way_class) {
                (Some(s), _) => format!("size {s}"),
                (_, Some(w)) => format!("way {w}"),
                _ => String::new(),
            })
            .unwrap_or_default(),
        stations: graph_node
            .map(|n| {
                n.stations
                    .iter()
                    .map(|s| {
                        format!(
                            "{} ({})",
                            s.anchor.as_str(),
                            match s.kind {
                                delvewright_dsl::StationKind::Point => "point",
                                delvewright_dsl::StationKind::Gate => "gate",
                            }
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        kind: kind_of(b),
    };
    let stem = node.0.strip_prefix("node/").unwrap_or(&node.0);
    let concept_name = format!("concept/{stem}");
    let refs: Vec<&delvewright_dsl::design::Reference> = c
        .design
        .as_ref()
        .map(|d| d.content.references.iter().collect())
        .unwrap_or_default();
    let row = refs
        .iter()
        .find(|r| r.name == concept_name)
        .map(|r| image(r));
    let concept = HandedConcept {
        name: concept_name,
        absent: row.is_none().then(|| {
            "no approved image yet: the place's concept is generated at step 9, after the walk, \
             anchored on the whole's sheet and on this handout"
                .to_string()
        }),
        row,
    };
    let sheet: Vec<HandedImage> = refs
        .iter()
        .filter(|r| r.name.starts_with("reference/"))
        .map(|r| image(r))
        .collect();

    let mine = seams_of(&plan.seams, node);
    let near = site.near(site.claim_bounds(i));
    let neighbours: Vec<Neighbour> = near
        .iter()
        .filter(|&&k| k != i)
        .map(|&k| {
            let q = &plan.boxes[k];
            Neighbour {
                place: q.node.0.clone(),
                face: face_between(b, q).to_string(),
                kind: kind_of(q),
                floor: q.floor,
                roof: q.roof.map(|r| [r.courses, r.eaves]),
                seams: plan
                    .seams
                    .iter()
                    .filter(|s| (&s.a == node && s.b == q.node) || (&s.b == node && s.a == q.node))
                    .map(|s| s.edge.0.clone())
                    .collect(),
            }
        })
        .collect();

    let centre = [
        (frame.lo[0] + frame.hi[0]) / 2,
        (frame.lo[1] + frame.hi[1]) / 2,
        (frame.lo[2] + frame.hi[2]) / 2,
    ];
    let views: Vec<HandedView> = c
        .site_plan
        .as_ref()
        .map(|p| {
            p.content
                .views
                .iter()
                .filter(|v| view_sees(v.eye, v.look_at, centre))
                .map(|v| HandedView {
                    id: v.id.0.clone(),
                    note: v.note.clone(),
                    snapshot: format!(
                        "delvec snapshot <campaign> --camera {}",
                        camera(v.eye, v.look_at)
                    ),
                })
                .collect()
        })
        .unwrap_or_default();

    // ---- the ground ----
    let mut perimeter: Vec<[i64; 3]> = Vec::new();
    for x in b.foot[0] - 1..=b.foot[1] + 1 {
        for z in b.foot[2] - 1..=b.foot[3] + 1 {
            if b.is_ring_column(x, z) {
                let g = site.ground_height(i, x, z);
                let l = local([x, g, z]);
                perimeter.push(l);
            }
        }
    }
    let min_y = perimeter
        .iter()
        .map(|p| p[1])
        .min()
        .unwrap_or(frame.datum_y() - 1);
    let max_y = perimeter
        .iter()
        .map(|p| p[1])
        .max()
        .unwrap_or(frame.datum_y() - 1);
    let mut columns = Vec::new();
    for x in b.foot[0]..=b.foot[1] {
        for z in b.foot[2]..=b.foot[3] {
            let top = plan.ground.top(x, z).unwrap_or(b.floor_course_y());
            let blocks: Vec<String> = (site.bottom(i)..=top.min(b.floor_course_y()))
                .map(|y| {
                    plan.ground
                        .fill_block([x, y, z])
                        .unwrap_or("minecraft:air")
                        .to_string()
                })
                .collect();
            let l = local([x, top, z]);
            columns.push(GroundColumn {
                x: l[0],
                z: l[2],
                top: l[1],
                blocks,
            });
        }
    }
    let ground = HandedGround {
        fill: if plan.ground.is_open() {
            "open"
        } else {
            "solid"
        }
        .to_string(),
        bottom_y: frame.bottom - frame.lo[1],
        floor_y: frame.datum_y(),
        perimeter,
        min_y,
        max_y,
        fixed: frame
            .fixed
            .iter()
            .map(|(cell, block)| FixedCell {
                cell: local(*cell),
                block: block.clone(),
            })
            .collect(),
        columns,
    };
    let roof = b.roof.filter(|_| !b.open).map(|r| HandedRoof {
        courses: r.courses,
        eaves: r.eaves,
        lid_y: b.top() + 1 - frame.lo[1],
        top_y: b.top() + 1 + i64::from(r.courses) - frame.lo[1],
        clipped: frame
            .clipped
            .iter()
            .map(|((lo, hi), n)| ([local(*lo), local(*hi)], n.0.clone()))
            .collect(),
    });

    Some(Allocation {
        place: node.0.clone(),
        brief,
        palette: c
            .detail_plan
            .as_ref()
            .and_then(|e| e.content.palette.clone()),
        concept,
        sheet,
        extent: frame.extent(),
        datum_y: frame.datum_y(),
        world_min: frame.lo,
        space: {
            let (lo, hi) = b.space();
            [local(lo), local(hi)]
        },
        neighbours,
        views,
        ground,
        roof,
        seams: mine
            .iter()
            .map(|(s, out)| {
                let (lo, hi) = answering_cells(&frame, s);
                AllocatedSeam {
                    edge: s.edge.0.clone(),
                    class: s.class.to_string(),
                    form: s.form.clone(),
                    other: if &s.a == node {
                        s.b.0.clone()
                    } else {
                        s.a.0.clone()
                    },
                    face: dir_name(*out).to_string(),
                    cells: [frame.to_local(lo), frame.to_local(hi)],
                    rise: if &s.a == node { s.rise } else { -s.rise },
                    answer_with: required_face_class(edge_of(c, s), s, node, &frame)
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                    owns_plane: frame.owns(s.opening.0),
                    ground_y: (s.normal_axis != 1).then(|| s.opening.0[1] - 1 - frame.lo[1]),
                }
            })
            .collect(),
        owed_anchors: delvewright_dsl::owed_anchors(c, node).into_iter().collect(),
        voids: frame
            .voids
            .iter()
            .map(|v| HandedVoid {
                cells: [local(v.lo), local(v.hi)],
                owner: v.owner.describe(),
            })
            .collect(),
    })
}

/// Every place's allocation, in plan document order — `delvec allocation --all`.
#[must_use]
pub fn allocations(c: &Campaign) -> Vec<Allocation> {
    frames(c)
        .iter()
        .filter_map(|(f, _)| allocation(c, &f.node))
        .collect()
}

// ---------------------------------------------------------------------------
// The bindings check (spec-0050 §3, §5, §6)
// ---------------------------------------------------------------------------

/// What the detail checks examined, each count with its denominator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DetailBinding {
    /// `details[]` rows READ — the denominator every row-level check has.
    ///
    /// Distinct from [`Self::bound`] on purpose: a duplicate row and a row
    /// naming a place the graph does not have are both rows this check read and
    /// neither is a place that got bound, so one number cannot honestly be both.
    /// Collapsed, it could exceed its own denominator.
    pub rows: usize,
    /// Of those, rows that resolved to a distinct box — places actually bound.
    pub bound: usize,
    /// The plan's boxes, which is what those rows are resolved against.
    pub boxes: usize,
    /// Pieces measured against their frame — `DW0843`.
    pub measured: usize,
    /// Seams a bound box must answer — `DW0844`.
    pub seams_required: usize,
    /// Faces of bound pieces examined — `DW0844`, the other direction.
    pub faces_examined: usize,
    /// Owed anchor names checked over every bound place — `DW0845`.
    pub owed: usize,
    /// Bound pieces declaring a `footprint_class` — `DW0848`.
    pub classed: usize,
    /// Roofed places bound to a piece with a contract — `DW0989`.
    pub roofed: usize,
    /// Of those, pieces carrying an enclosing space.
    pub enclosing: usize,
}

impl DetailBinding {
    /// One line, stated whether or not any of it is zero, because a check that
    /// reports no binding is a check nobody can tell apart from one that never
    /// ran.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "detail binding: {bd} of {b} place(s) bound over {r} `details[]` row(s), {m} \
             piece(s) measured against their frame, {sr} seam(s) required answering over {fe} \
             declared face(s) examined, {o} owed anchor name(s) checked, {cl} piece(s) \
             declaring a footprint class, {rf} roofed place(s) bound ({en} with an enclosing \
             space).",
            rf = self.roofed,
            en = self.enclosing,
            bd = self.bound,
            b = self.boxes,
            r = self.rows,
            m = self.measured,
            sr = self.seams_required,
            fe = self.faces_examined,
            o = self.owed,
            cl = self.classed,
        )
    }
}

/// **`DW0842`'s anchor half and `DW0845`, for one row.**
///
/// A helper rather than a block inside the loop because there are two ways
/// through that loop and both owe these verdicts. A piece that is not the shape
/// of its frame, or that declares no contract, suspends the FACE check — those
/// cells are computed from the frame — and suspends nothing else. It used to
/// suspend everything, so fixing a piece's size produced a fresh crop of
/// refusals nobody had been shown, and the owed names of that place were missing
/// from the binding count while it happened.
fn check_owed(
    c: &Campaign,
    d: &mut Vec<Diagnostic>,
    binding: &mut DetailBinding,
    row: &delvewright_dsl::Detail,
    meta: &delvewright_dsl::PrefabMeta,
    path: &str,
    gates: &BTreeMap<String, ([i64; 3], [i64; 3])>,
) {
    // ---- DW0842 / DW0845: the owed names ----
    let owed = delvewright_dsl::owed_anchors(c, &row.place);
    for key in row.anchors.keys() {
        if !owed.contains(key) {
            d.push(Diagnostic::error(
                DW_BINDING,
                STAGE,
                format!("{path}/anchors/{key}"),
                format!(
                    "`{key}` is not a name `{place}` owes. A row re-binds exactly the \
                     synthesized names whose bearer is this box — its own `anchor/node-…`, \
                     `spawn` when it is the entry, and each `anchor/unlock-…` whose \
                     opening side it is. A gate region (`anchor/seam-…`) is never owed: it \
                     stands in a party plane the whole owns, not in the piece. This place \
                     owes {n} name(s): {list}.",
                    place = row.place,
                    n = owed.len(),
                    list = if owed.is_empty() {
                        "none".to_string()
                    } else {
                        owed.iter().cloned().collect::<Vec<_>>().join(", ")
                    },
                ),
            ));
        }
    }
    for name in &owed {
        binding.owed += 1;
        let Some(bound_to) = row.anchors.get(name) else {
            d.push(Diagnostic::error(
                DW_ANCHOR_STANDING,
                STAGE,
                format!("{path}/anchors"),
                format!(
                    "`{place}` owes the anchor `{name}` and this row binds nothing to it. \
                     The campaign's quest layer bound that name to this place before any \
                     detail existed, so detailing must never force a quest edit — the row's \
                     `anchors` map is what keeps the campaign's vocabulary and the piece's \
                     own vocabulary both intact. Add `\"{name}\": \"<an anchor of \
                     {piece}>\"`. This place owes {n} name(s): {list}.",
                    place = row.place,
                    piece = row.piece,
                    n = owed.len(),
                    list = owed.iter().cloned().collect::<Vec<_>>().join(", "),
                ),
            ));
            continue;
        };
        let Some(anchor) = meta.anchors.get(bound_to) else {
            d.push(Diagnostic::error(
                DW_BINDING,
                STAGE,
                format!("{path}/anchors/{name}"),
                format!(
                    "`{name}` is bound to `{bound_to}`, which is not an anchor of \
                     `{piece}`. That piece declares {n} anchor(s): {list}.",
                    piece = row.piece,
                    n = meta.anchors.len(),
                    list = if meta.anchors.is_empty() {
                        "none".to_string()
                    } else {
                        meta.anchors.keys().cloned().collect::<Vec<_>>().join(", ")
                    },
                ),
            ));
            continue;
        };
        // spec-0052 §7.5: the SHAPE the station declared is demanded of the
        // piece anchor it binds to, so the kind validation read off the graph
        // and the kind the built world actually has cannot drift.
        //
        // **Only the gate direction is raised here**, and the reason is that
        // `DW0845` already owns the other one: an owed POINT bound to a region
        // is exactly "a region, not a place to stand", which the standing check
        // below says in its own words and has said since spec-0050. Raising a
        // second code for it would prescribe two repairs for one mistake and
        // move a diagnostic a campaign already meets. A gate station bound to a
        // point is the case nothing could report before, because before
        // spec-0052 no owed name was ever a gate.
        if delvewright_dsl::synthesized_anchor_kinds(c)
            .get(name)
            .copied()
            .is_some_and(|k| k == delvewright_dsl::StationKind::Gate)
        {
            // A seam's gate region, owed by the place that owns the plane
            // (spec-0098 §2), is exactly the allocated opening.
            if let (Some(want), Some(r)) = (gates.get(name), anchor.region.as_ref()) {
                let lo = [
                    i64::from(r.from[0].min(r.to[0])),
                    i64::from(r.from[1].min(r.to[1])),
                    i64::from(r.from[2].min(r.to[2])),
                ];
                let hi = [
                    i64::from(r.from[0].max(r.to[0])),
                    i64::from(r.from[1].max(r.to[1])),
                    i64::from(r.from[2].max(r.to[2])),
                ];
                if (lo, hi) != *want {
                    d.push(Diagnostic::error(
                        DW_BINDING,
                        STAGE,
                        format!("{path}/anchors/{name}"),
                        format!(
                            "`{name}` is the gate over a seam whose plane `{place}` owns, and it \
                             is bound to `{piece}`'s anchor `{bound_to}`, whose region is {got}. \
                             The gate is exactly the opening the plan allocated: {want_s} \
                             (piece-local). Bind it to a gate anchor whose region is those \
                             cells.",
                            place = row.place,
                            piece = row.piece,
                            got = aabb(lo, hi),
                            want_s = aabb(want.0, want.1),
                        ),
                    ));
                }
            }
            if anchor.region.is_none() {
                d.push(Diagnostic::error(
                    DW_BINDING,
                    STAGE,
                    format!("{path}/anchors/{name}"),
                    format!(
                        "`{name}` is declared as a gate station and is bound to `{piece}`'s \
                         anchor `{bound_to}`, which declares no region. A gate is a volume that \
                         seals and clears: the campaign's `open-gate`, `close-gate`, `shortcut` \
                         and `timed-gate` verbs were validated against the shape this name says \
                         it is, and a cell is not one. Bind it to an anchor of `{piece}` that \
                         declares a `region` and the block filling it, or change the station's \
                         `kind` to `point` in the layout graph.",
                        piece = row.piece,
                    ),
                ));
            }
            // A gate's cells are not a place to stand, so the standing demands
            // below are the POINT contract and are not asked of it — asking
            // them would refuse every correct gate binding.
            continue;
        }
        // DW0845's second half: bound to somewhere a body cannot be.
        if anchor.pos.is_none() {
            d.push(Diagnostic::error(
                DW_ANCHOR_STANDING,
                STAGE,
                format!("{path}/anchors/{name}"),
                format!(
                    "`{name}` is bound to `{piece}`'s anchor `{bound_to}`, which declares no \
                     cell — it is a region, not a place to stand. Every name a place owes is \
                     a point a body is put at: `anchor/node-…` is where a quest, an NPC or a \
                     wave is seated, `spawn` is where the delve opens. A region anchor \
                     answers a gate, and a gate region is never owed by a place. Bind this \
                     to a point anchor of the piece.",
                    piece = row.piece,
                ),
            ));
            continue;
        }
        if let Some(resolves) = anchor.resolves_to.as_deref()
            && !resolves.starts_with("space:")
        {
            d.push(Diagnostic::error(
                DW_ANCHOR_STANDING,
                STAGE,
                format!("{path}/anchors/{name}"),
                format!(
                    "`{name}` is bound to `{piece}`'s anchor `{bound_to}`, which the piece's \
                     own contract resolves into `{resolves}` — not play space. The campaign \
                     puts bodies, quests and waves at this name; a body cannot be in a \
                     `no_body` region, inside a bar, or in a transit volume. Bind it to an \
                     anchor standing in one of the piece's declared spaces.",
                    piece = row.piece,
                ),
            ));
        }
    }
}

/// **`DW0848`'s consumer door, for one row** — see [`check_owed`] for why it is
/// a helper.
fn check_class(
    d: &mut Vec<Diagnostic>,
    binding: &mut DetailBinding,
    meta: &delvewright_dsl::PrefabMeta,
    reads: &mut Reads,
    path: &str,
) {
    if meta.footprint_class.is_none() {
        return;
    }
    binding.classed += 1;
    if let Some(f) =
        delvewright_dsl::prefab::check_footprint_class(meta, STAGE, &format!("{path}/piece"), reads)
    {
        d.push(f);
    }
}

/// **`DW0842`–`DW0845` and `DW0848`'s consumer door, over a whole campaign.**
///
/// Validation tier: a diagnostic here is exit 1, before any byte is written.
/// Bound in `validate_loaded`, which every `delvec` subcommand's validation goes
/// through — `build` included, so a defect cannot reach a datapack by skipping
/// `delvec validate`.
///
/// A campaign with no detail plan runs every check zero times and says so.
pub fn check(c: &Campaign, prefabs: &PrefabRegistry) -> (Vec<Diagnostic>, DetailBinding) {
    let mut d: Vec<Diagnostic> = Vec::new();
    let mut binding = DetailBinding::default();
    let Some(doc) = c.detail_plan.as_ref().map(|e| &e.content) else {
        return (d, binding);
    };

    // The limiting case, named rather than inferred: a detail plan needs a plan.
    let Some(_) = c.site_plan.as_ref() else {
        d.push(Diagnostic::error(
            DW_BINDING,
            STAGE,
            "/content/details",
            format!(
                "this campaign carries a `detail-plan` and no `site-plan.json`. A `details[]` row \
                 names a place and a piece and says nothing about where anything goes — the frame \
                 is computed from the site plan's box, so without a plan there is no frame for a \
                 piece to be checked against and nothing to build. That is the ordering made \
                 uncompilable rather than advised: there is nothing to author early, because the \
                 document cannot state where anything is. Binding: {n} row(s) resolved against \
                 ZERO boxes.",
                n = doc.details.len(),
            ),
        ));
        return (d, binding);
    };

    let mut reads = Reads::new();
    let plan = SitePlan::of(c);
    let site = plan.site();
    let boxes = &plan.boxes;
    let seams = &plan.seams;
    let by_node: BTreeMap<&str, usize> = boxes
        .iter()
        .enumerate()
        .map(|(i, b)| (b.node.0.as_str(), i))
        .collect();
    binding.boxes = boxes.len();

    // **A plan that resolves NO box is one finding, not one per row**
    // (`Diagnostic`'s "one cause, one line"). With zero boxes every `details[]`
    // row misses `by_node` by construction, so the per-row refusal below says
    // the same sentence once for each row and none of those copies is the
    // finding: the finding is that the embedding this plan is detailing does
    // not exist. Measured on a 24-place campaign with `layout-graph.json`
    // deleted: 24 copies of "is not a place this map has", ahead of nothing and
    // behind a `DW0824` that had already said it.
    //
    // The code still refuses, and still refuses per row the moment there is a
    // map to be wrong about — this arm is reachable only at a zero box count.
    if boxes.is_empty() && !doc.details.is_empty() {
        binding.rows = doc.details.len();
        let places: Vec<String> = doc
            .details
            .iter()
            .map(|row| format!("`{}`", row.place))
            .collect();
        let cause = if c.layout_graph.is_none() {
            " This campaign carries no `layout-graph.json`, which `DW0824` has already refused: \
             the plan embeds a graph, so with no graph there are no places for it to place, and \
             that one line is the finding these rows are downstream of."
        } else {
            " The site plan places no box at all, so there is no place for any row to name; the \
             plan is what has to gain them."
        };
        d.push(Diagnostic::error(
            DW_BINDING,
            STAGE,
            "/content/details",
            format!(
                "the site plan resolves ZERO boxes, so none of the {n} `details[]` row(s) names a \
                 place this map has: {places}. A row fills the box the site plan gave a \
                 layout-graph node, and there is no such box to fill.{cause} This is stated once \
                 rather than once per row, because one map is the cause of all {n} and repairing \
                 the rows would repair nothing.",
                n = doc.details.len(),
                places = places.join(", "),
            ),
        ));
        return (d, binding);
    }

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, row) in doc.details.iter().enumerate() {
        binding.rows += 1;
        let path = format!("/content/details[{i}]");
        if !seen.insert(row.place.0.as_str()) {
            d.push(Diagnostic::error(
                DW_BINDING,
                STAGE,
                format!("{path}/place"),
                format!(
                    "`{place}` is bound by more than one `details[]` row. A place is filled by \
                     one piece: the row's whole meaning is *this building stands in this box*, \
                     and two buildings in one box have no arbitration and no frame either could \
                     be checked against. Several pieces per place is deliberately excluded until \
                     a campaign brief demands it (spec-0050 §17) — if this one does, that is the \
                     falsifier, and the answer is a first-class surface rather than two rows. \
                     Binding: {n} row(s) resolved against {b} box(es).",
                    place = row.place,
                    n = doc.details.len(),
                    b = boxes.len(),
                ),
            ));
            continue;
        }
        let Some(b) = by_node.get(row.place.0.as_str()).copied() else {
            let known: Vec<&str> = boxes.iter().map(|b| b.node.0.as_str()).collect();
            d.push(Diagnostic::error(
                DW_BINDING,
                STAGE,
                format!("{path}/place"),
                format!(
                    "`{place}` is not a place this map has. A `details[]` row fills the box the \
                     site plan gave a layout-graph node, and the plan resolves {b} box(es): {known}. \
                     Either name one of those, or add the place to the layout graph and the site \
                     plan first — which is a plan edit.",
                    place = row.place,
                    b = boxes.len(),
                    known = if known.is_empty() {
                        "none".to_string()
                    } else {
                        known.join(", ")
                    },
                ),
            ));
            continue;
        };
        binding.bound += 1;
        let frame = Frame::of(&site, b);
        let b = &boxes[b];

        let Some(meta) = prefabs.get(row.piece.as_str()) else {
            d.push(Diagnostic::error(
                DW_BINDING,
                STAGE,
                format!("{path}/piece"),
                format!(
                    "`{piece}` is not a piece the prefab library holds. A detail piece is an \
                     ordinary prefab — frozen bytes plus metadata — whether a grammar program \
                     exported it or it was admitted from other tooling; the engine consumes the \
                     object class, never the tool that made it. Export or admit `{piece}` into \
                     the prefabs directory (`.nbt` + metadata) and bind it again.",
                    piece = row.piece,
                ),
            ));
            continue;
        };

        // ---- DW0843: exactly the shape of the allocation ----
        binding.measured += 1;
        let want = frame.extent();
        let got = meta.size();
        let got64 = [i64::from(got[0]), i64::from(got[1]), i64::from(got[2])];
        if got64 != want {
            let over: Vec<String> = (0..3)
                .filter(|&a| got64[a] != want[a])
                .map(|a| {
                    format!(
                        "{ax} is {g}, and the frame is {w} ({diff})",
                        ax = ["x", "y", "z"][a],
                        g = got64[a],
                        w = want[a],
                        diff = if got64[a] > want[a] {
                            format!("{} too many", got64[a] - want[a])
                        } else {
                            format!("{} too few", want[a] - got64[a])
                        }
                    )
                })
                .collect();
            d.push(Diagnostic::error(
                DW_NOT_THE_FRAME,
                STAGE,
                format!("{path}/piece"),
                format!(
                    "`{piece}` is not the shape of the box `{place}` gives it. The piece is \
                     {gx}x{gy}x{gz}; the frame is {wx}x{wy}x{wz} — {over}. The frame is the play \
                     space plus the one floor course under it, and equality is EXACT: undersize \
                     refuses exactly as oversize does, because the box is the footprint and a \
                     smaller building means a smaller box. That is a site-plan edit, taken \
                     visibly, and it is the only way a part changes what the whole gave it. Run `delvec allocation {place}` for the frame, the datum and every \
                     seam this box must answer.{upstream}",
                    piece = row.piece,
                    place = row.place,
                    upstream = delvewright_dsl::refused_upstream(c, &row.place, seams, &mut reads),
                    gx = got64[0],
                    gy = got64[1],
                    gz = got64[2],
                    wx = want[0],
                    wy = want[1],
                    wz = want[2],
                    over = over.join("; "),
                ),
            ));
        }

        // ---- DW0843, second half: a detail piece owes a contract ----
        let contract = meta.spatial_contract.as_ref();
        if contract.is_none() {
            d.push(Diagnostic::error(
                DW_NOT_THE_FRAME,
                STAGE,
                format!("{path}/piece"),
                format!(
                    "`{piece}` declares no spatial contract, so it cannot be a detail piece. \
                     Traversal equivalence is proved by the piece's OWN contract gates \
                     (reachability inside the place, its faces against the plan's seams) running \
                     beside the map's; a piece with no contract gives the equivalence instrument \
                     nothing to read, and a place detailed with one would be a hole in the proof \
                     rather than a finding in it. Re-export the piece with its contract, or admit \
                     it through `delvec prefab`, which resolves one.",
                    piece = row.piece,
                ),
            ));
        }

        // ---- DW0989: a roofed place's piece encloses something ----
        //
        // The kind is the object's — the box's `ceiling` — never the piece's
        // own word: a place with a lid is bound to a piece carrying at least
        // one space the closure gate examines.
        if let Some(contract) = contract {
            binding.roofed += usize::from(!b.open);
            let enclosing = contract
                .spaces
                .values()
                .filter(|sp| sp.envelope == "enclosed" || sp.envelope == "open_top")
                .count();
            binding.enclosing += usize::from(!b.open && enclosing > 0);
            if !b.open && enclosing == 0 {
                let envelopes: BTreeSet<&str> = contract
                    .spaces
                    .values()
                    .map(|sp| sp.envelope.as_str())
                    .collect();
                d.push(Diagnostic::error(
                    DW_ROOFED_ENCLOSES_NOTHING,
                    STAGE,
                    format!("{path}/piece"),
                    format!(
                        "`{place}` is a roofed place — its box has a `clearance` ceiling of {h} \
                         — and `{piece}` declares no `enclosed` or `open_top` space: its \
                         envelope(s) are {envs}. A lid is the place's reason for a roof, so \
                         the room under it is in the piece, and the closure gate examines it. \
                         Declare the room under the lid `enclosed` (or `open_top`), or make the \
                         place open in the site plan (`\"ceiling\": \"open\"`).",
                        place = row.place,
                        piece = row.piece,
                        h = b.clearance,
                        envs = if envelopes.is_empty() {
                            "none".to_string()
                        } else {
                            envelopes
                                .iter()
                                .map(|e| format!("`{e}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        },
                    ),
                ));
            }
        }

        // ---- DW0844: faces against seams, both directions ----
        //
        // **The one half a wrong extent or a missing contract really does
        // suspend**, and it is suspended rather than skipped silently: the cells
        // this compares are computed from the FRAME, so against a piece that is
        // not the frame every verdict would be about a geometry nobody has. The
        // checks below it — the owed names, the declared class — depend on
        // neither fact, so they run either way. Suppressing them was how fixing a
        // piece's size produced a fresh crop of refusals nobody had been shown.
        let mine = seams_of(seams, &row.place);
        let gates: BTreeMap<String, ([i64; 3], [i64; 3])> = mine
            .iter()
            .filter(|(s, _)| s.class == "barred" && frame.owns(s.opening.0))
            .map(|(s, _)| {
                (
                    delvewright_dsl::siteplan::seam_anchor(&s.edge),
                    (frame.to_local(s.opening.0), frame.to_local(s.opening.1)),
                )
            })
            .collect();
        let mut answered: BTreeSet<usize> = BTreeSet::new();
        let Some(contract) = contract.filter(|_| got64 == want) else {
            check_owed(c, &mut d, &mut binding, row, meta, &path, &gates);
            check_class(&mut d, &mut binding, meta, &mut reads, &path);
            continue;
        };
        for (s, out) in &mine {
            binding.seams_required += 1;
            let (clo, chi) = answering_cells(&frame, s);
            let want_classes = required_face_class(edge_of(c, s), s, &row.place, &frame);
            let hit = contract.faces.iter().position(|f| {
                dir_of(&f.dir) == Some(*out) && {
                    let (flo, fhi) = face_world(&frame, f);
                    flo == clo && fhi == chi
                }
            });
            match hit {
                Some(idx) if want_classes.contains(&contract.faces[idx].class.as_str()) => {
                    answered.insert(idx);
                }
                Some(idx) => {
                    answered.insert(idx);
                    d.push(Diagnostic::error(
                        DW_FACES,
                        STAGE,
                        format!("{path}/piece"),
                        format!(
                            "`{piece}` answers the seam `{edge}` with a `{got}` face where the \
                             plan allocated a `{class}` way. The plan's seam is what a body does \
                             here, and the two are not the same crossing. This box must answer \
                             it with {want}. Cells (piece-local): {cells}.",
                            piece = row.piece,
                            edge = s.edge,
                            got = contract.faces[idx].class,
                            class = s.class,
                            want = want_classes.join(" or "),
                            cells = aabb(frame.to_local(clo), frame.to_local(chi)),
                        ),
                    ));
                }
                None => {
                    let near: Vec<String> = contract
                        .faces
                        .iter()
                        .filter(|f| dir_of(&f.dir) == Some(*out))
                        .map(|f| {
                            let (flo, fhi) = face_world(&frame, f);
                            format!(
                                "a `{}` at {}",
                                f.class,
                                aabb(frame.to_local(flo), frame.to_local(fhi))
                            )
                        })
                        .collect();
                    d.push(Diagnostic::error(
                        DW_FACES,
                        STAGE,
                        format!("{path}/piece"),
                        format!(
                            "`{piece}` leaves the seam `{edge}` unanswered. The plan cut a \
                             `{class}` way out of `{place}` on its {dir} side at {cells} \
                             (piece-local), and the piece declares {offered} there. A place is \
                             detailed inside the box the whole gave it, and the ways out of that \
                             box are the whole's: a piece that does not answer one seals a \
                             connection the map is built on. Answer it with {want}, or revise the \
                             SITE PLAN.",
                            piece = row.piece,
                            edge = s.edge,
                            class = s.class,
                            place = row.place,
                            dir = dir_name(*out),
                            cells = aabb(frame.to_local(clo), frame.to_local(chi)),
                            offered = if near.is_empty() {
                                "no face on that side at all".to_string()
                            } else {
                                near.join("; and ")
                            },
                            want = want_classes.join(" or "),
                        ),
                    ));
                }
            }
        }
        // The other direction: a face of the piece answering no seam is a way
        // out the plan never allocated — the discovered seam, at the earliest
        // tier there is.
        for (idx, f) in contract.faces.iter().enumerate() {
            binding.faces_examined += 1;
            if answered.contains(&idx) || f.class == "vision" {
                continue;
            }
            if dir_of(&f.dir).is_none() {
                continue; // a face naming no direction is the contract checker's finding.
            }
            let (flo, fhi) = face_world(&frame, f);
            d.push(Diagnostic::error(
                DW_FACES,
                STAGE,
                format!("{path}/piece"),
                format!(
                    "`{piece}` declares a `{class}` way out of `{place}` on its {dir} side at \
                     {cells} (piece-local), and the plan allocated no seam there. A way out that \
                     nothing allocated is a connection DISCOVERED rather than designed, which is \
                     the exact failure the allocation exists to end — and a body that takes it \
                     leaves the map's own graph. `{place}` is allocated {n} seam(s): {list}. \
                     Either seal this face in the piece, or allocate the connection in the layout \
                     graph and the site plan — which is a plan edit.{upstream}",
                    upstream = delvewright_dsl::refused_upstream(c, &row.place, seams, &mut reads),
                    piece = row.piece,
                    class = f.class,
                    place = row.place,
                    dir = f.dir,
                    cells = aabb(frame.to_local(flo), frame.to_local(fhi)),
                    n = mine.len(),
                    list = if mine.is_empty() {
                        "none".to_string()
                    } else {
                        mine.iter()
                            .map(|(s, out)| {
                                format!("`{}` ({} {})", s.edge, dir_name(*out), s.class)
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    },
                ),
            ));
        }

        check_owed(c, &mut d, &mut binding, row, meta, &path, &gates);
        check_class(&mut d, &mut binding, meta, &mut reads, &path);
    }
    (d, binding)
}

// ---------------------------------------------------------------------------
// The piece never writes a cell it does not own (spec-0098 §7)
// ---------------------------------------------------------------------------

/// What the void check examined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoidBinding {
    /// Bound pieces whose bytes were opened.
    pub pieces: usize,
    /// Void cells of their frames examined — `DW0987`.
    pub voids: usize,
    /// Of those, painted.
    pub painted: usize,
    /// Fixed ring ground cells of their frames examined — `DW0990`.
    pub fixed: usize,
    /// Of those, written.
    pub written: usize,
}

impl VoidBinding {
    /// One line, zeroes included.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "void binding: {p} bound piece(s) opened, {v} void cell(s) examined, {pa} painted \
             (DW0987); {f} fixed ring ground cell(s) examined, {w} written (DW0990).",
            p = self.pieces,
            v = self.voids,
            pa = self.painted,
            f = self.fixed,
            w = self.written,
        )
    }
}

/// Every cell a piece's own templates hold, piece-local, with the block —
/// air included, a cell no template places absent.
fn piece_entries(
    meta: &delvewright_dsl::PrefabMeta,
    dir: &std::path::Path,
) -> Result<BTreeMap<[i64; 3], String>, String> {
    let mut out = BTreeMap::new();
    for t in meta.templates() {
        let path = dir.join(t.file);
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let s = crate::admit::structure::Structure::read(&bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        for x in 0..s.size[0] {
            for y in 0..s.size[1] {
                for z in 0..s.size[2] {
                    if let Some(entry) = s.entry_at([x, y, z]) {
                        out.insert(
                            [
                                i64::from(x + t.offset[0]),
                                i64::from(y + t.offset[1]),
                                i64::from(z + t.offset[2]),
                            ],
                            entry.name.clone(),
                        );
                    }
                }
            }
        }
    }
    Ok(out)
}

/// **`DW0987` and `DW0990`'s first shape: a bound piece writes no cell it does
/// not own**, read off the piece's own `.nbt` at validation.
///
/// Every void cell of the frame must hold `minecraft:structure_void` or nothing
/// — air counts as painting, because the game places a template's air. A void
/// the ring's fixed ground fills is `DW0990` (the plot does not stitch), any
/// other is `DW0987` (a neighbour's cell, a gap, a clipped eave), each named
/// with its owner. A piece whose size is not its frame's is `DW0843`'s and is
/// not opened here; one whose bytes will not open is `DW0346`'s.
pub fn check_voids(
    c: &Campaign,
    prefabs: &PrefabRegistry,
    prefabs_dir: &std::path::Path,
) -> (Vec<Diagnostic>, VoidBinding) {
    let mut d = Vec::new();
    let mut binding = VoidBinding::default();
    let Some(doc) = c.detail_plan.as_ref().map(|e| &e.content) else {
        return (d, binding);
    };
    for (frame, _) in frames(c) {
        let Some((i, row)) = doc
            .details
            .iter()
            .enumerate()
            .find(|(_, r)| r.place == frame.node)
        else {
            continue;
        };
        let Some(meta) = prefabs.get(row.piece.as_str()) else {
            continue;
        };
        let size = meta.size();
        if [i64::from(size[0]), i64::from(size[1]), i64::from(size[2])] != frame.extent() {
            continue;
        }
        let Ok(cells) = piece_entries(meta, prefabs_dir) else {
            continue;
        };
        binding.pieces += 1;
        let mut painted: Vec<([i64; 3], String, String)> = Vec::new();
        let mut written: Vec<([i64; 3], String, String)> = Vec::new();
        let fixed: BTreeMap<[i64; 3], &String> = frame.fixed.iter().map(|(c, b)| (*c, b)).collect();
        for v in &frame.voids {
            for x in v.lo[0]..=v.hi[0] {
                for y in v.lo[1]..=v.hi[1] {
                    for z in v.lo[2]..=v.hi[2] {
                        let world = [x, y, z];
                        let ground = fixed.get(&world);
                        if ground.is_some() {
                            binding.fixed += 1;
                        } else {
                            binding.voids += 1;
                        }
                        let Some(block) = cells.get(&frame.to_local(world)) else {
                            continue;
                        };
                        if block == "minecraft:structure_void" {
                            continue;
                        }
                        match ground {
                            Some(g) => written.push((world, block.clone(), (*g).clone())),
                            None => painted.push((world, block.clone(), v.owner.describe())),
                        }
                    }
                }
            }
        }
        binding.painted += painted.len();
        binding.written += written.len();
        let path = format!("/content/details[{i}]/piece");
        let show = |list: &[([i64; 3], String, String)], what: &str| -> String {
            list.iter()
                .take(6)
                .map(|(w, b, o)| {
                    let l = frame.to_local(*w);
                    format!(
                        "`{b}` at piece-local [{}, {}, {}] (world [{}, {}, {}]), {what} {o}",
                        l[0], l[1], l[2], w[0], w[1], w[2]
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        if !painted.is_empty() {
            d.push(Diagnostic::error(
                DW_PAINTS_NEIGHBOUR,
                STAGE,
                path.clone(),
                format!(
                    "`{piece}` writes {n} cell(s) of its frame that `{place}` does not own: \
                     {list}{more}. A frame is a box and a place's claim is not, so the cells of \
                     the box a neighbour, the site's fill or nobody owns are the piece's voids: \
                     the piece holds `minecraft:structure_void` there (or no block) and the \
                     owner's block shows through. Air counts as painting — the game places a \
                     template's air, so air over a neighbour's wall carves it. `delvec \
                     allocation {place}` lists every void with its owner, and `delvec detail` \
                     writes them for a program-detailed place.",
                    piece = row.piece,
                    place = row.place,
                    n = painted.len(),
                    list = show(&painted, "owned by"),
                    more = if painted.len() > 6 {
                        format!(", and {} more", painted.len() - 6)
                    } else {
                        String::new()
                    },
                ),
            ));
        }
        if !written.is_empty() {
            d.push(Diagnostic::error(
                crate::compiler::blockout::DW_PLOT_UNSTITCHED,
                STAGE,
                path,
                format!(
                    "`{piece}` writes {n} cell(s) of the ring's fixed ground around `{place}`: \
                     {list}{more}. The ring of ground around every place is the whole's — the \
                     terrain continued to the plot's edge — so two neighbouring plots meet the \
                     same ground and the map's surface runs unbroken through every edge. The \
                     piece holds `minecraft:structure_void` at each fixed cell and shapes its \
                     own ground inside the ring; `delvec allocation {place}` lists every fixed \
                     cell with its block.",
                    piece = row.piece,
                    place = row.place,
                    n = written.len(),
                    list = show(&written, "displacing the terrain's"),
                    more = if written.len() > 6 {
                        format!(", and {} more", written.len() - 6)
                    } else {
                        String::new()
                    },
                ),
            ));
        }
    }
    (d, binding)
}

// ---------------------------------------------------------------------------
// The one path from a binding to placed bytes (spec-0050 §1)
// ---------------------------------------------------------------------------

/// What a detail plan puts in the world: the pieces, and the campaign names
/// their anchors now answer to.
#[derive(Default)]
pub struct Detailing {
    /// Every bound piece, at the frame the site plan computes for it.
    pub pieces: Vec<PiecePlacement>,
    /// `(campaign anchor name, world cell, facing)` for every owed point a row
    /// binds — the re-binding of spec-0050 §6. The facing is the piece's,
    /// because which way a body faces when it arrives is a fact about the room
    /// it arrives in.
    pub anchors: Vec<(String, [i32; 3], Option<String>)>,
    /// `(campaign anchor name, world low corner, world high corner, fill
    /// block)` for every owed gate a row binds — a gate station, or a barred
    /// seam's gate region the place owns the plane of (spec-0098 §2). The block
    /// is the piece's own bar, the one `close-gate` writes back.
    pub gates: Vec<(String, [i32; 3], [i32; 3], String)>,
}

/// **The one path from a `details[]` row to placed bytes.**
///
/// Called once, from [`crate::compiler::plan::Plan::build`], and there is no second
/// caller: a `Plan` is the only thing every world-reaching verb can reach a
/// world through, and there is no other constructor. Someone placing a detail
/// piece without this would have had to build a `Plan` some other way, and there
/// is none.
///
/// The position is [`Frame::of`] over the plan's own resolved box and nothing
/// else — no field on any document contributes a term to it. Rotation is
/// [`Rotation::None`] and there is no field that could make it anything else: a
/// frame is a specific box in the world, and a piece that had to be turned to
/// fit it is a piece that is not the shape of its allocation, which is
/// `DW0843`'s refusal rather than a placement decision.
///
/// The anchors half is what keeps the campaign's own vocabulary working: the
/// quest layer bound `anchor/node-…` to a place at stage 3, before any detail
/// existed, so detailing must never force a quest edit. A name a row does not
/// bind is absent here rather than guessed at — `DW0845` has refused it, and
/// inventing a position would answer a refusal with a silent default.
///
/// Returns nothing for a campaign with no detail plan, so such a campaign's
/// output does not move by a byte.
#[must_use]
pub fn place(c: &Campaign, prefabs: &PrefabRegistry) -> Detailing {
    let mut out = Detailing::default();
    let Some(doc) = c.detail_plan.as_ref().map(|e| &e.content) else {
        return out;
    };
    for (frame, _) in frames(c) {
        let Some(row) = doc.detail_of(&frame.node) else {
            continue;
        };
        let Some(meta) = prefabs.get(row.piece.as_str()) else {
            continue; // `DW0842` refused it; there is nothing to place.
        };
        let pos = [frame.lo[0] as i32, frame.lo[1] as i32, frame.lo[2] as i32];
        out.pieces.push(PiecePlacement {
            prefab_id: row.piece.0.clone(),
            templates: crate::compiler::plan::placed_templates(meta, pos, Rotation::None),
            pos,
            size: meta.size(),
            rotation: Rotation::None,
            // A detail piece is handed a frozen frame, not mated to a neighbour.
            mated: Vec::new(),
        });
        for (name, bound_to) in &row.anchors {
            let Some(a) = meta.anchors.get(bound_to) else {
                continue; // `DW0842` refused it.
            };
            if let Some(r) = a.region.as_ref() {
                let at = |c: [i32; 3]| [pos[0] + c[0], pos[1] + c[1], pos[2] + c[2]];
                out.gates.push((
                    name.clone(),
                    at(r.from),
                    at(r.to),
                    a.block
                        .clone()
                        .unwrap_or_else(|| delvewright_dsl::siteplan::SEAM_BAR.to_string()),
                ));
                continue;
            }
            let Some(p) = a.pos else {
                continue; // `DW0845` refused it: an owed name is a place to stand.
            };
            out.anchors.push((
                name.clone(),
                [pos[0] + p[0], pos[1] + p[1], pos[2] + p[2]],
                a.facing.clone(),
            ));
        }
    }
    out
}

/// True when the campaign's detail plan binds every layout-graph node — the
/// computed fact `DW0821`'s severity is keyed to (spec-0050 §7.6).
///
/// Keyed to the artifact rather than to a stage marker or an author flag, so
/// there is nothing to set and nothing to forget. A fully detailed map asserting
/// a vista owes the vista; a map with one box still massed does not, because
/// derived massing has no landform and the ridge the vista reads over has not
/// been carved yet.
#[must_use]
pub fn fully_detailed(c: &Campaign) -> bool {
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return false;
    };
    if graph.nodes.is_empty() {
        return false; // an empty graph details nothing; a zero is not a yes.
    }
    let bound = delvewright_dsl::bound_places(c);
    graph.nodes.iter().all(|n| bound.contains(n.id.0.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::EdgeId;
    use delvewright_dsl::siteplan::Face;

    fn a_box(node: &str, floor: i64) -> PlacedBox {
        PlacedBox {
            node: NodeId(node.into()),
            foot: [0, 7, 0, 7],
            floor,
            clearance: 3,
            open: false,
            roof: None,
        }
    }

    /// A seam of `class` on `face` of `a`, with its plane at `plane`.
    fn a_seam(class: &'static str, face: Face, plane: i64, normal_axis: usize) -> PlacedSeam {
        let flat = |v: i64| {
            let mut c = [3i64, 65, 3];
            c[normal_axis] = v;
            c
        };
        PlacedSeam {
            edge: EdgeId("edge/way".into()),
            class,
            a: NodeId("node/a".into()),
            b: NodeId("node/b".into()),
            face,
            normal_axis,
            plane,
            opening: (flat(plane), flat(plane)),
            shared: (flat(plane), flat(plane)),
            crossing: delvewright_dsl::siteplan::Crossing::Portal,
            rise: 0,
            stair_in: None,
            form: "a doorway".into(),
        }
    }

    fn lone_frame(b: &PlacedBox) -> Frame {
        let boxes = vec![b.clone()];
        let g = delvewright_dsl::siteplan::Ground::solid("minecraft:stone");
        let site = delvewright_dsl::siteplan::Site::new(&boxes, &[], &g);
        Frame::of(&site, 0)
    }

    /// **Every row of spec-0050 §3's table**, including the one no site plan in
    /// this repository reaches.
    ///
    /// The table is the contract between a plan's seams and a piece's faces, and
    /// a row nobody has exercised is a row that is wrong the first time somebody
    /// authors it. The fixtures cover five of the six between them; this covers
    /// all six from the geometry alone.
    #[test]
    fn the_class_table_answers_every_row() {
        let upper = a_box("node/a", 64);
        let f = lone_frame(&upper);
        let a = NodeId("node/a".into());
        let b = NodeId("node/b".into());

        // `walk` → `walk`, from either side.
        let s = a_seam("walk", Face::East, f.hi[0] + 1, 0);
        assert_eq!(required_face_class(None, &s, &a, &f), ["walk"]);

        // `stair`, this box hosts → `stair` or `walk`; the other box → `walk`.
        let mut s = a_seam("stair", Face::East, f.hi[0] + 1, 0);
        s.stair_in = Some(a.clone());
        assert_eq!(required_face_class(None, &s, &a, &f), ["stair", "walk"]);
        assert_eq!(required_face_class(None, &s, &b, &f), ["walk"]);

        // `drop` → `drop` leaving, `walk` landing.
        let s = a_seam("drop", Face::Down, f.lo[1], 1);
        let edge = Edge::Drop {
            id: EdgeId("edge/way".into()),
            a: a.clone(),
            b: b.clone(),
            falls: Direction::AToB,
            shortcut: false,
            gating: None,
        };
        assert_eq!(required_face_class(Some(&edge), &s, &a, &f), ["drop"]);
        assert_eq!(required_face_class(Some(&edge), &s, &b, &f), ["walk"]);

        // `barred` in a plane this piece does NOT own → `walk`: the bar is the
        // owner's to ship.
        let s = a_seam("barred", Face::East, f.hi[0] + 1, 0);
        assert!(!f.owns(s.opening.0), "the plane is beyond the frame");
        assert_eq!(
            answering_layer(&f, &s),
            f.hi[0] + 0,
            "the layer beside the plane"
        );
        assert_eq!(required_face_class(None, &s, &a, &f), ["walk"]);

        // `barred` in a plane this piece OWNS → `barred`: the piece ships the
        // gate's shut state (spec-0098 §2), and answers at the plane itself.
        let s = a_seam("barred", Face::East, f.hi[0], 0);
        assert!(f.owns(s.opening.0), "the ring is the piece's");
        assert_eq!(
            answering_layer(&f, &s),
            s.plane,
            "the owner answers at the plane"
        );
        assert_eq!(required_face_class(None, &s, &a, &f), ["barred"]);
    }

    /// The answering layer is the plane for its owner and the cell beside it,
    /// on the place's own side, for the other — a fact about who owns the plane.
    #[test]
    fn the_answering_layer_is_the_plane_or_the_cell_beside_it() {
        let f = lone_frame(&a_box("node/a", 64));
        for (axis, face) in [(0usize, Face::East), (2, Face::South)] {
            let beyond_high = a_seam("walk", face, f.hi[axis] + 1, axis);
            assert_eq!(answering_layer(&f, &beyond_high), f.hi[axis]);
            let beyond_low = a_seam("walk", face, f.lo[axis] - 1, axis);
            assert_eq!(answering_layer(&f, &beyond_low), f.lo[axis]);
            let owned = a_seam("walk", face, f.lo[axis], axis);
            assert_eq!(
                answering_layer(&f, &owned),
                f.lo[axis],
                "a plane the piece owns answers AT itself"
            );
        }
    }

    /// **Criterion 14: two seams at a corner bind** — the Narrows' shape. A
    /// way-classed strip with a contact along its whole west face and another
    /// across its north end, both naming the strip first, answers each at its
    /// own party plane: the two openings meet only at the ring's corner column,
    /// which neither shared face reaches, so they are disjoint; and a piece whose
    /// contract carries one via per plane passes `contract-well-formed` and
    /// exports exactly the faces `DW0844` compares against those cells. Vacuous
    /// if the shared faces did not both reach the strip's corner: asserted.
    #[test]
    fn two_contacts_at_a_corner_answer_at_their_own_planes() {
        use delvewright_dsl::siteplan::{Crossing, Ground, Site};
        let strip = PlacedBox {
            node: NodeId("node/narrows".into()),
            foot: [10, 13, 10, 29],
            floor: 64,
            clearance: 3,
            open: true,
            roof: None,
        };
        let west = PlacedBox {
            node: NodeId("node/flat".into()),
            foot: [0, 8, 10, 29],
            floor: 64,
            clearance: 3,
            open: true,
            roof: None,
        };
        let north = PlacedBox {
            node: NodeId("node/quay".into()),
            foot: [10, 13, 0, 8],
            floor: 64,
            clearance: 3,
            open: true,
            roof: None,
        };
        let contact =
            |edge: &str, b: &str, face: Face, axis: usize, plane: i64, span: (i64, i64)| {
                let mut lo = [0, 64, 0];
                let mut hi = [0, 66, 0];
                lo[axis] = plane;
                hi[axis] = plane;
                let other = if axis == 0 { 2 } else { 0 };
                lo[other] = span.0;
                hi[other] = span.1;
                PlacedSeam {
                    edge: EdgeId(edge.into()),
                    class: "walk",
                    a: strip.node.clone(),
                    b: NodeId(b.into()),
                    face,
                    normal_axis: axis,
                    plane,
                    opening: (lo, hi),
                    shared: (lo, hi),
                    crossing: Crossing::Contact,
                    rise: 0,
                    stair_in: None,
                    form: "open ground".into(),
                }
            };
        // The shared faces are the overlaps of the play spaces' spans: z 10..29
        // on the west plane x = 9, x 10..13 on the north plane z = 9.
        let s_west = contact("edge/narrows-flat", "node/flat", Face::West, 0, 9, (10, 29));
        let s_north = contact(
            "edge/narrows-quay",
            "node/quay",
            Face::North,
            2,
            9,
            (10, 13),
        );
        assert_eq!(
            s_west.shared.0[2], strip.foot[2],
            "the west face reaches the corner"
        );
        assert_eq!(
            s_north.shared.0[0], strip.foot[0],
            "the north face reaches the corner"
        );
        let boxes = vec![strip.clone(), west, north];
        let seams = vec![s_west.clone(), s_north.clone()];
        let g = Ground::solid("minecraft:stone");
        let site = Site::new(&boxes, &seams, &g);
        let f = Frame::of(&site, 0);
        let (wl, wh) = answering_cells(&f, &s_west);
        let (nl, nh) = answering_cells(&f, &s_north);
        assert_eq!((wl[0], wh[0]), (9, 9), "the west contact at its own plane");
        assert_eq!((nl[2], nh[2]), (9, 9), "the north contact at its own plane");
        let cells = |lo: [i64; 3], hi: [i64; 3]| -> BTreeSet<[i64; 3]> {
            (lo[0]..=hi[0])
                .flat_map(|x| {
                    (lo[1]..=hi[1]).flat_map(move |y| (lo[2]..=hi[2]).map(move |z| [x, y, z]))
                })
                .collect()
        };
        let (cw, cn) = (cells(wl, wh), cells(nl, nh));
        assert!(cw.is_disjoint(&cn), "the two openings share no cell");
        assert!(f.owns(wl) && f.owns(nl), "the strip owns both planes");

        // A piece: the frame, its floor course stone, the play space and both
        // openings air, every other cell of the ring wall.
        let ext = f.extent();
        let mut model =
            crate::grammar::model::VoxelModel::new(crate::grammar::geom::Box3::at_origin([
                ext[0] as u32,
                ext[1] as u32,
                ext[2] as u32,
            ]));
        let (slo, shi) = strip.space();
        let stone = crate::grammar::block::BlockState::simple("minecraft:stone_bricks");
        let air = crate::grammar::block::BlockState::air();
        for x in 0..ext[0] {
            for y in 0..ext[1] {
                for z in 0..ext[2] {
                    let w = [f.lo[0] + x, f.lo[1] + y, f.lo[2] + z];
                    let open = (w[1] >= slo[1] && (0..3).all(|i| w[i] >= slo[i] && w[i] <= shi[i]))
                        || cw.contains(&w)
                        || cn.contains(&w)
                        || w[1] > shi[1];
                    let b = if open { &air } else { &stone };
                    model.set([x as i32, y as i32, z as i32], b).unwrap();
                }
            }
        }
        let local = |c: [i64; 3]| {
            let l = f.to_local(c);
            [l[0] as i32, l[1] as i32, l[2] as i32]
        };
        let region = |lo: [i64; 3], hi: [i64; 3]| delvewright_dsl::prefab::Region {
            from: local(lo),
            to: local(hi),
        };
        let mut spaces = BTreeMap::new();
        spaces.insert(
            "strip".to_string(),
            delvewright_dsl::prefab::ContractSpace {
                envelope: "open".to_string(),
                boxes: vec![region(slo, [shi[0], shi[1], shi[2]])],
            },
        );
        let via = |name: &str, lo, hi| delvewright_dsl::prefab::ContractEdge {
            a: "strip".into(),
            b: "exterior".into(),
            class: "walk".into(),
            rise: Some(0),
            via: Some(delvewright_dsl::prefab::ContractVolume {
                region: name.into(),
                boxes: vec![region(lo, hi)],
            }),
            bar: None,
            way: None,
        };
        let contract = delvewright_dsl::prefab::SpatialContract {
            entry: "strip".into(),
            spaces,
            no_body: BTreeMap::new(),
            edges: vec![via("to-flat", wl, wh), via("to-quay", nl, nh)],
            faces: Vec::new(),
            no_body_majority_ack: None,
        };
        let report = crate::grammar::contract::check(&model, &contract, &BTreeMap::new());
        let wf = report
            .gates
            .iter()
            .find(|g| g.id == "contract-well-formed")
            .expect("the gate runs");
        assert!(wf.passed(), "{}", wf.detail);
        let faces = crate::grammar::contract::exterior_faces(&model, &contract);
        assert_eq!(faces.len(), 2, "{faces:?}");
        for (face, (lo, hi)) in faces.iter().zip([(wl, wh), (nl, nh)]) {
            let want: BTreeSet<[i32; 3]> = cells(lo, hi).into_iter().map(local).collect();
            assert_eq!(
                face.cells, want,
                "the face DW0844 compares is the answering cells"
            );
        }
    }
}
