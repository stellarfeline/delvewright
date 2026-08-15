//! **The spatial contract's checker** (spec-0036 §1c/§2): does the building agree
//! with what its author said the building is?
//!
//! # The direction is one-way, and that is the whole design
//!
//! A space's kind, an edge's class, an envelope's claim and a region's
//! out-of-walk status are **declared**. Nothing here reads them out of the
//! voxels. Inference is what ADR-0020 forbids by name, and the reason is that an
//! inferred claim cannot be wrong: if "this is a room" means "these cells look
//! like a room", then every building trivially agrees with itself and the gate
//! measures nothing. The declaration comes from the author; the blocks are the
//! evidence; a gate is the disagreement between them.
//!
//! # One checker, two doors
//!
//! The input is a block grid plus a **resolved** contract in exactly the shape
//! the prefab metadata carries ([`SpatialContract`]). A grammar expansion
//! resolves its scope-bound declarations into that shape and hands it over; a
//! hand-built or ingested piece reads the same block out of its metadata file.
//! Both doors therefore compare the same two things, and "same bytes + same
//! resolved contract → same verdict" is true because there is only one
//! implementation to be true about.
//!
//! # Every gate says what it examined
//!
//! Each obligation is a [`Gate`] with a binding count, and a binding of zero is
//! a red on the three obligations that carry the weight (closure, edge proof,
//! reachability) rather than a quiet pass — a green over nothing is the failure
//! mode this project ships most often (CLAUDE.md).
//!
//! # Where this deliberately departs from spec-0036, and why
//!
//! **A via-less edge to `exterior` licenses no closure exception.** §2.3 lists
//! "the discovered opening of an edge with no `via`" among the excuses. Between
//! two declared spaces that costs nothing: an abutting space is already an
//! excuse in its own right, and crossing into it without a declared edge still
//! fails the graph-confined walk. Toward `exterior` it is an unsecured opt-out
//! of exactly the kind §0 forbids — the demand would be "declare an edge", and
//! an eleven-course missing wall can supply that as easily as a door can. So an
//! opening in an enclosed envelope is excused only by a **claimed** region: the
//! author names the cells, and the via constraints then bind to them. What a
//! piece leaves open at its own outer face is not a closure question at all; it
//! is the face contract, which assembly consumes ([`exterior_faces`]).
//!
//! **`no_body_majority_ack` cannot buy a `posted` majority.** The
//! acknowledgement as specified demands a string, which is a property the defect
//! it excuses supplies for free. It is narrowed here by a computed fact the
//! author cannot write: it silences the majority red only when the out-of-walk
//! majority is made of `sealed` and `facade` cells, whose demands are facts
//! about the blocks. A majority of `posted` cells — the one kind an author
//! secures by placing something — reds with the acknowledgement present.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_schem::prefab::{Region as MetaRegion, SpatialContract};

use crate::gates::{Gate, verdict};
use crate::model::VoxelModel;
use crate::nav;

/// The endpoint name that means "outside the piece".
pub use crate::ir::EXTERIOR;

/// How many cells a red lists before it stops counting out loud. The totals are
/// exact however many there are; the list exists to send someone to a place.
const CELLS_LISTED: usize = 8;

/// How far from an anchor a `posted` cell may sit: the Chebyshev radius spec-0036
/// §2.6 fixes, per cell rather than per region.
const POSTED_RADIUS: i32 = 2;

/// The six face directions.
const DIRS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

/// What one out-of-walk region turned out to be.
///
/// **Computed, never declared** (spec-0036 §0's corollary): an author who could
/// pick the kind would be picking which demand has to be met, and a choice among
/// demands is only ever as strong as the weakest on offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NoBodyKind {
    /// Walled off: the union of every sealed region is itself closed.
    Sealed,
    /// Anchored: every standable cell is within Chebyshev 2 of an anchor the
    /// region contains.
    Posted,
    /// Exterior dressing: every standable cell is touched by the air outside the
    /// piece, and the region is not nested inside any space.
    Facade,
}

impl NoBodyKind {
    /// The keyword a verdict prints.
    pub fn as_str(self) -> &'static str {
        match self {
            NoBodyKind::Sealed => "sealed",
            NoBodyKind::Posted => "posted",
            NoBodyKind::Facade => "facade",
        }
    }
}

/// The verdict over one (grid, resolved contract) pair.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractReport {
    /// Every obligation, in a fixed order.
    pub gates: Vec<Gate>,
    /// Things a reader must be told even where nothing went red.
    pub findings: Vec<String>,
    /// **Every opt-out instance, by name** — the per-instance form a blind
    /// script cannot satisfy and a reviewer actually reads (spec-0036 §2.9).
    pub enumeration: Vec<String>,
}

impl ContractReport {
    /// True when no obligation went red.
    pub fn is_pass(&self) -> bool {
        self.gates.iter().all(Gate::passed)
    }
}

// ---------------------------------------------------------------------------
// Cell sets
// ---------------------------------------------------------------------------

/// Every cell an inclusive metadata range covers.
fn range_cells(r: &MetaRegion, out: &mut BTreeSet<[i32; 3]>) {
    for x in r.from[0]..=r.to[0] {
        for y in r.from[1]..=r.to[1] {
            for z in r.from[2]..=r.to[2] {
                out.insert([x, y, z]);
            }
        }
    }
}

/// Every cell a list of ranges covers.
fn cells(boxes: &[MetaRegion]) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for r in boxes {
        range_cells(r, &mut out);
    }
    out
}

/// The lowest `y` a list of ranges reaches, or `None` for an empty list. The
/// level a `rise` is measured from (spec-0036 §1a: `min_y(b) − min_y(a)` over
/// resolved boxes, not over cells a body happens to be able to stand in).
fn min_y(boxes: &[MetaRegion]) -> Option<i32> {
    boxes.iter().map(|r| r.from[1]).min()
}

/// `x 2..8 y 0..3 z 5..5`, so a red sends someone to a place rather than to a
/// number.
fn describe_cells(cells: &BTreeSet<[i32; 3]>) -> String {
    let listed: Vec<String> = cells
        .iter()
        .take(CELLS_LISTED)
        .map(|c| format!("[{},{},{}]", c[0], c[1], c[2]))
        .collect();
    if cells.len() > CELLS_LISTED {
        format!("{} … ({} in all)", listed.join(" "), cells.len())
    } else {
        listed.join(" ")
    }
}

// ---------------------------------------------------------------------------
// The resolved contract, indexed
// ---------------------------------------------------------------------------

/// A traversal class: something a body crosses. `vision` is the one class that
/// is not one.
fn is_traversal(class: &str) -> bool {
    matches!(class, "walk" | "stair" | "drop" | "barred")
}

/// A class whose `via` is a **transit volume** (its own cells, disjoint from
/// every space) rather than an opening on a shared boundary.
fn is_transit(class: &str) -> bool {
    matches!(class, "stair" | "drop")
}

/// One element of the reachability graph: a space, or a traversal edge's own
/// volume.
#[derive(Debug, Clone)]
struct Element {
    /// How a verdict names it.
    label: String,
    /// Its cells.
    cells: BTreeSet<[i32; 3]>,
}

/// The contract, resolved into cell sets once so every obligation reads the same
/// ones.
struct Index<'a> {
    contract: &'a SpatialContract,
    /// Space name → cells.
    space_cells: BTreeMap<&'a str, BTreeSet<[i32; 3]>>,
    /// Out-of-walk region name → cells.
    no_body_cells: BTreeMap<&'a str, BTreeSet<[i32; 3]>>,
    /// Edge index → its via's cells (empty when it declares none).
    via_cells: Vec<BTreeSet<[i32; 3]>>,
    /// Edge index → its bar's cells (empty when it has none).
    bar_cells: Vec<BTreeSet<[i32; 3]>>,
    /// Every cell of every space, unioned.
    all_space_cells: BTreeSet<[i32; 3]>,
    /// Every cell of every out-of-walk region, unioned.
    all_no_body_cells: BTreeSet<[i32; 3]>,
    /// Standable cells of the model.
    standable: BTreeSet<[i32; 3]>,
}

impl<'a> Index<'a> {
    fn new(model: &VoxelModel, contract: &'a SpatialContract) -> Index<'a> {
        let space_cells: BTreeMap<&str, BTreeSet<[i32; 3]>> = contract
            .spaces
            .iter()
            .map(|(name, s)| (name.as_str(), cells(&s.boxes)))
            .collect();
        let no_body_cells: BTreeMap<&str, BTreeSet<[i32; 3]>> = contract
            .no_body
            .iter()
            .map(|(name, r)| (name.as_str(), cells(&r.boxes)))
            .collect();
        let via_cells: Vec<BTreeSet<[i32; 3]>> = contract
            .edges
            .iter()
            .map(|e| e.via.as_ref().map(|v| cells(&v.boxes)).unwrap_or_default())
            .collect();
        let bar_cells: Vec<BTreeSet<[i32; 3]>> = contract
            .edges
            .iter()
            .map(|e| e.bar.as_ref().map(|b| cells(&b.boxes)).unwrap_or_default())
            .collect();
        let all_space_cells = space_cells.values().flatten().copied().collect();
        let all_no_body_cells = no_body_cells.values().flatten().copied().collect();
        Index {
            contract,
            space_cells,
            no_body_cells,
            via_cells,
            bar_cells,
            all_space_cells,
            all_no_body_cells,
            standable: nav::standable_cells(model),
        }
    }

    /// A space's cells, or an empty set for `exterior` / an undeclared name.
    fn space(&self, name: &str) -> &BTreeSet<[i32; 3]> {
        static EMPTY: std::sync::OnceLock<BTreeSet<[i32; 3]>> = std::sync::OnceLock::new();
        self.space_cells
            .get(name)
            .unwrap_or_else(|| EMPTY.get_or_init(BTreeSet::new))
    }

    /// A space's standable cells.
    fn standable_in(&self, name: &str) -> BTreeSet<[i32; 3]> {
        self.space(name)
            .iter()
            .filter(|c| self.standable.contains(*c))
            .copied()
            .collect()
    }

    /// Every edge incident to a space, by index.
    fn edges_at(&self, space: &str) -> Vec<usize> {
        self.contract
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.a == space || e.b == space)
            .map(|(i, _)| i)
            .collect()
    }
}

/// The cells that separate two spaces: not in either, and touching both. What a
/// `walk`/`barred`/`vision` opening has to be cut out of (spec-0036 §1a).
fn shared_boundary(a: &BTreeSet<[i32; 3]>, b: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for cell in a {
        for d in DIRS {
            let n = [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]];
            if a.contains(&n) || b.contains(&n) {
                continue;
            }
            if DIRS
                .iter()
                .any(|e| b.contains(&[n[0] + e[0], n[1] + e[1], n[2] + e[2]]))
            {
                out.insert(n);
            }
        }
    }
    out
}

/// Cells 6-adjacent to a set but outside it.
fn shell(set: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for cell in set {
        for d in DIRS {
            let n = [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]];
            if !set.contains(&n) {
                out.insert(n);
            }
        }
    }
    out
}

/// Every passable cell the air outside the piece reaches.
///
/// The positive fact `facade` demands (spec-0036 §2.6). The model's region *is*
/// the artifact's bounding box, so the flood is seeded from every passable cell
/// on its outer layer and runs inward through passable cells only. An enclosed
/// interior can never be reached by it — its own closure proof guarantees as
/// much — which is exactly why an interior stranding cannot buy this kind.
fn exterior_air(model: &VoxelModel) -> BTreeSet<[i32; 3]> {
    let region = model.region();
    let min = region.origin;
    let max = region.maximum();
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    for pos in region.positions() {
        let on_face = (0..3).any(|a| pos[a] == min[a] || pos[a] == max[a] - 1);
        if on_face && nav::passable(model, pos) && seen.insert(pos) {
            queue.push_back(pos);
        }
    }
    while let Some(pos) = queue.pop_front() {
        for d in DIRS {
            let n = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
            if nav::passable(model, n) && seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

// ---------------------------------------------------------------------------
// The checker
// ---------------------------------------------------------------------------

/// Check a resolved contract against the blocks it claims to describe.
///
/// `anchors` are the piece's declared anchors by exported name — the campaign's
/// namespace, and the thing a `posted` region has to contain.
pub fn check(
    model: &VoxelModel,
    contract: &SpatialContract,
    anchors: &BTreeMap<String, [i32; 3]>,
) -> ContractReport {
    let ix = Index::new(model, contract);
    let mut gates = Vec::new();
    let mut findings = Vec::new();
    let mut enumeration = Vec::new();

    gates.push(well_formed(&ix, model));
    gates.push(coverage(&ix));
    gates.push(closure(&ix, model, &mut enumeration));
    gates.push(edge_proof(&ix, model));
    let kinds = no_body_kinds(&ix, model, anchors, &mut enumeration);
    gates.push(no_body_gate(&ix, &kinds));
    gates.push(reachability(&ix, model, &mut enumeration));
    gates.push(anchors_gate(&ix, anchors, &kinds, &mut enumeration));
    gates.push(exterior_faces_gate(&ix, model, &mut enumeration));
    gates.push(no_body_majority(&ix, &kinds));

    // §2.9's vacuity reds are stated where the reader is, not only in the gate
    // that carries them: a binding of zero is a finding by name.
    for gate in &gates {
        if gate.bound == 0 {
            findings.push(format!(
                "contract gate `{}` examined ZERO objects — its verdict binds to nothing",
                gate.id
            ));
        }
    }
    if ix.contract.spaces.len() == 1 && ix.contract.edges.is_empty() {
        findings.push(
            "the contract declares one space and no edges: nothing about how a body moves through \
             this piece is being claimed, so the edge and reachability obligations have almost \
             nothing to prove"
                .to_string(),
        );
    }

    ContractReport {
        gates,
        findings,
        enumeration,
    }
}

// --- §2.1 well-formed ------------------------------------------------------

fn well_formed(ix: &Index, model: &VoxelModel) -> Gate {
    let contract = ix.contract;
    let mut bad: Vec<String> = Vec::new();
    let bound = contract.spaces.len() + contract.no_body.len() + contract.edges.len();

    // The entry is a declared space, and the piece is enterable where it says.
    if !contract.spaces.contains_key(&contract.entry) {
        bad.push(format!(
            "`entry` names {:?}, which is not a declared space",
            contract.entry
        ));
    } else if !contract.edges.iter().any(|e| {
        is_traversal(&e.class)
            && ((e.a == contract.entry && e.b == EXTERIOR)
                || (e.b == contract.entry && e.a == EXTERIOR))
    }) {
        bad.push(format!(
            "the entry space {:?} carries no `exterior` edge of a traversal class — the piece \
             claims no way in",
            contract.entry
        ));
    }

    // Two different spaces may abut; they may never overlap.
    let names: Vec<&str> = ix.space_cells.keys().copied().collect();
    for (i, a) in names.iter().enumerate() {
        for b in &names[i + 1..] {
            let overlap: BTreeSet<[i32; 3]> =
                ix.space(a).intersection(ix.space(b)).copied().collect();
            if !overlap.is_empty() {
                bad.push(format!(
                    "spaces {a:?} and {b:?} overlap on {} cell(s): {}",
                    overlap.len(),
                    describe_cells(&overlap)
                ));
            }
        }
    }

    // A space is one floor: its standable cells span at most two consecutive
    // levels. Greater relief is two places and a transition, and a transition is
    // an edge that owes a `rise`.
    //
    // Measured over the space's *walk*, so nested out-of-walk cells are left
    // out: a corbel shelf eight courses up is not a second storey of the room,
    // and §2.5 already excludes it from the walk for the same reason.
    for name in &names {
        let standable: BTreeSet<[i32; 3]> = ix
            .standable_in(name)
            .difference(&ix.all_no_body_cells)
            .copied()
            .collect();
        let (Some(lo), Some(hi)) = (
            standable.iter().map(|c| c[1]).min(),
            standable.iter().map(|c| c[1]).max(),
        ) else {
            continue;
        };
        if hi - lo > 1 {
            bad.push(format!(
                "space {name:?} has standable floor at y {lo}..{hi}, which is {} levels — a space \
                 is ONE floor (at most two consecutive levels, for a dais). Two levels are two \
                 places and a transition, and a transition is an edge that owes a `rise`",
                hi - lo + 1
            ));
        }
    }

    // An out-of-walk region nests wholly inside one space, or touches none.
    for (name, region) in &ix.no_body_cells {
        let hosts: Vec<&str> = ix
            .space_cells
            .iter()
            .filter(|(_, s)| !s.is_disjoint(region))
            .map(|(n, _)| *n)
            .collect();
        match hosts.as_slice() {
            [] => {}
            [host] => {
                let outside: BTreeSet<[i32; 3]> =
                    region.difference(ix.space(host)).copied().collect();
                if !outside.is_empty() {
                    bad.push(format!(
                        "out-of-walk region {name:?} straddles the boundary of space {host:?}: {} \
                         of its cells lie outside it ({}). A region either nests wholly inside one \
                         space or touches none",
                        outside.len(),
                        describe_cells(&outside)
                    ));
                }
            }
            many => bad.push(format!(
                "out-of-walk region {name:?} spans {} spaces ({}); a region spanning hosts splits \
                 by host",
                many.len(),
                many.join(", ")
            )),
        }
    }

    for (i, edge) in contract.edges.iter().enumerate() {
        let site = format!("edge {}--{}--{}", edge.a, edge.class, edge.b);
        let exterior = edge.a == EXTERIOR || edge.b == EXTERIOR;
        for endpoint in [&edge.a, &edge.b] {
            if endpoint != EXTERIOR && !contract.spaces.contains_key(endpoint) {
                bad.push(format!("{site}: {endpoint:?} is not a declared space"));
            }
        }
        if !matches!(
            edge.class.as_str(),
            "walk" | "stair" | "drop" | "barred" | "vision"
        ) {
            bad.push(format!("{site}: {:?} is not an edge class", edge.class));
            continue;
        }

        // `rise` presence and sign, per class. The IR's enum already makes most
        // of this unwritable; the metadata form does not, and a hand-built piece
        // enters through the metadata form.
        //
        // A level edge's `rise` field is written by its absence — the classes
        // that default it serialise `0` as nothing — so an exterior edge is
        // refused a *declared* rise, which is a non-zero one. There is no way in
        // the IR to spell "this walk has no rise at all", and inventing one
        // would be a second spelling of the default.
        match (edge.class.as_str(), edge.rise, exterior) {
            ("vision", Some(_), _) => bad.push(format!("{site}: a sightline declares no `rise`")),
            (_, Some(r), true) if r != 0 => bad.push(format!(
                "{site}: declares rise {r}, but an edge with an `exterior` endpoint has no \
                 resolved box on the far side to measure a level against"
            )),
            ("stair", Some(r), false) if r < 1 => {
                bad.push(format!("{site}: a stair rises, so `rise` is >= 1, not {r}"))
            }
            ("drop", Some(r), false) if r > -1 => {
                bad.push(format!("{site}: a drop falls, so `rise` is <= -1, not {r}"))
            }
            ("stair" | "drop", None, false) => {
                bad.push(format!("{site}: this class requires a declared `rise`"))
            }
            _ => {}
        }

        if edge.class == "barred" && edge.bar.is_none() {
            bad.push(format!("{site}: a barred edge declares what stands in it"));
        }
        if edge.class != "barred" && edge.bar.is_some() {
            bad.push(format!("{site}: only a barred edge carries a bar"));
        }
        if matches!(edge.class.as_str(), "stair" | "vision") && edge.via.is_none() {
            bad.push(format!(
                "{site}: this class requires a `via` — a stair's treads belong to the edge, and a \
                 sightline IS its opening"
            ));
        }

        let via = &ix.via_cells[i];
        if via.is_empty() {
            continue;
        }
        if is_transit(&edge.class) {
            // A transit volume is its own place: disjoint from every space, and
            // touching both ends.
            for (name, space) in &ix.space_cells {
                let overlap: BTreeSet<[i32; 3]> = via.intersection(space).copied().collect();
                if !overlap.is_empty() {
                    bad.push(format!(
                        "{site}: its transit volume overlaps space {name:?} on {} cell(s) ({}). A \
                         stair's treads and a drop's column belong to the edge, not to either end",
                        overlap.len(),
                        describe_cells(&overlap)
                    ));
                }
            }
            let shell_of_via = shell(via);
            for endpoint in [&edge.a, &edge.b] {
                if endpoint == EXTERIOR {
                    continue;
                }
                if shell_of_via.is_disjoint(ix.space(endpoint)) {
                    bad.push(format!(
                        "{site}: its transit volume does not touch {endpoint:?} — a transit volume \
                         abuts both endpoints"
                    ));
                }
            }
        } else if let Some((name, _)) = ix
            .space_cells
            .iter()
            .find(|(_, space)| !space.is_disjoint(via))
        {
            // An opening is a hole through a boundary, so it is never made of
            // cells that are already inside a room. Without this, a `via`
            // claimed anywhere in the interior would excuse any breach on that
            // space's shell — the unconstrained-`via` hatch, one layer in.
            bad.push(format!(
                "{site}: its opening claims cells that are inside space {name:?}. An opening is a \
                 hole through a boundary, not a piece of the room it opens"
            ));
        } else if exterior {
            // An opening to the outside has to actually be one: every cell of it
            // touches the space, and the air outside the piece reaches it. A via
            // claimed on interior cells cannot supply that.
            let space = if edge.a == EXTERIOR { &edge.b } else { &edge.a };
            let outside = exterior_air(model);
            let detached: BTreeSet<[i32; 3]> = via
                .iter()
                .filter(|c| {
                    !DIRS.iter().any(|d| {
                        ix.space(space)
                            .contains(&[c[0] + d[0], c[1] + d[1], c[2] + d[2]])
                    })
                })
                .copied()
                .collect();
            if !detached.is_empty() {
                bad.push(format!(
                    "{site}: {} of its opening's cells do not touch {space:?} ({})",
                    detached.len(),
                    describe_cells(&detached)
                ));
            }
            let sealed_in: BTreeSet<[i32; 3]> = via
                .iter()
                .filter(|c| !outside.contains(*c))
                .copied()
                .collect();
            if !sealed_in.is_empty() {
                bad.push(format!(
                    "{site}: {} of its opening's cells are not reached by the air outside the piece \
                     ({}) — an opening to the exterior is a hole to the outside, and claiming \
                     interior cells for one does not make it one",
                    sealed_in.len(),
                    describe_cells(&sealed_in)
                ));
            }
        } else {
            // An interior opening lies on the boundary the two ends share. An
            // unconstrained `via` was a closure exemption anywhere on the model
            // (spec-0036 §1a) — five 1x1x1 boxes over five breaches bought a
            // pass, and this is what refuses them.
            let boundary = shared_boundary(ix.space(&edge.a), ix.space(&edge.b));
            let off: BTreeSet<[i32; 3]> = via.difference(&boundary).copied().collect();
            if !off.is_empty() {
                bad.push(format!(
                    "{site}: {} of its opening's cells are not on the boundary {:?} and {:?} share \
                     ({})",
                    off.len(),
                    edge.a,
                    edge.b,
                    describe_cells(&off)
                ));
            }
        }
    }

    Gate {
        id: "contract-well-formed",
        state: verdict(bad.is_empty()),
        undecided: 0,
        bound,
        detail: if bad.is_empty() {
            format!(
                "{} space(s), {} out-of-walk region(s) and {} edge(s) hold together: no overlap, \
                 one floor each, every opening on the boundary it claims",
                contract.spaces.len(),
                contract.no_body.len(),
                contract.edges.len()
            )
        } else {
            bad.join(" · ")
        },
    }
}

// --- §2.2 coverage ---------------------------------------------------------

fn coverage(ix: &Index) -> Gate {
    let mut covered: BTreeSet<[i32; 3]> = ix.all_space_cells.clone();
    covered.extend(ix.all_no_body_cells.iter().copied());
    for (i, edge) in ix.contract.edges.iter().enumerate() {
        if is_traversal(&edge.class) {
            covered.extend(ix.via_cells[i].iter().copied());
        }
    }
    let uncovered: BTreeSet<[i32; 3]> = ix.standable.difference(&covered).copied().collect();
    Gate {
        id: "contract-coverage",
        state: verdict(uncovered.is_empty()),
        undecided: 0,
        bound: ix.standable.len(),
        detail: if uncovered.is_empty() {
            format!(
                "every one of {} standable cell(s) lies in a declared space, an out-of-walk region \
                 or a traversal edge's transit volume",
                ix.standable.len()
            )
        } else {
            format!(
                "{} of {} standable cell(s) are in NOTHING the contract declares — floor the piece \
                 does not account for: {}",
                uncovered.len(),
                ix.standable.len(),
                describe_cells(&uncovered)
            )
        },
    }
}

// --- §2.3 closure ----------------------------------------------------------

fn closure(ix: &Index, model: &VoxelModel, enumeration: &mut Vec<String>) -> Gate {
    let mut examined = 0usize;
    let mut breaches: Vec<String> = Vec::new();

    for (name, decl) in &ix.contract.spaces {
        let space = ix.space(name);
        match decl.envelope.as_str() {
            "enclosed" | "open_top" => {}
            "open" => {
                enumeration.push(format!(
                    "envelope: space {name:?} is declared `open` — no boundary claim, {} cell(s)",
                    space.len()
                ));
            }
            other => {
                breaches.push(format!("space {name:?}: {other:?} is not an envelope"));
                continue;
            }
        }

        // An envelope that claims openness demands sky (spec-0036 §0/§2.3): a
        // roofed room cannot be downgraded out of closure.
        if decl.envelope != "enclosed" {
            if decl.envelope == "open_top" {
                enumeration.push(format!(
                    "envelope: space {name:?} is declared `open_top` — side faces still closed"
                ));
            }
            let roofed: BTreeSet<[i32; 3]> = space
                .iter()
                .filter(|c| ix.standable.contains(*c) && nav::sheltered(model, **c))
                .copied()
                .collect();
            if !roofed.is_empty() {
                breaches.push(format!(
                    "space {name:?} is declared `{}` but {} of its standable cell(s) have this \
                     piece's own blocks overhead ({}) — a roofed room cannot be downgraded out of \
                     closure",
                    decl.envelope,
                    roofed.len(),
                    describe_cells(&roofed)
                ));
            }
        }
        if decl.envelope == "open" {
            continue;
        }

        // What a passable boundary cell is excused by, and nothing else: a
        // declared opening, the neighbouring room, or an out-of-walk region that
        // abuts. There is no "discovered" opening — see the module header.
        let mut excused: BTreeSet<[i32; 3]> = BTreeSet::new();
        for i in ix.edges_at(name) {
            excused.extend(ix.via_cells[i].iter().copied());
        }
        for (other, set) in &ix.space_cells {
            if other != name {
                excused.extend(set.iter().copied());
            }
        }
        excused.extend(ix.all_no_body_cells.iter().copied());

        let mut open: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        for cell in space {
            for d in DIRS {
                // `open_top` gives up the top face and keeps every other; a cell
                // that is also a side neighbour is still examined, through that
                // side.
                if decl.envelope == "open_top" && d == [0, 1, 0] {
                    continue;
                }
                let n = [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]];
                if space.contains(&n) || !seen.insert(n) {
                    continue;
                }
                examined += 1;
                if nav::passable(model, n) && !excused.contains(&n) {
                    open.insert(n);
                }
            }
        }
        if !open.is_empty() {
            breaches.push(format!(
                "space {name:?} is declared `{}` but {} of its boundary cell(s) are open air that \
                 no declared opening, neighbouring space or out-of-walk region accounts for: {}",
                decl.envelope,
                open.len(),
                describe_cells(&open)
            ));
        }
    }

    Gate {
        id: "contract-closure",
        state: verdict(breaches.is_empty() && examined > 0),
        undecided: 0,
        bound: examined,
        detail: if !breaches.is_empty() {
            breaches.join(" · ")
        } else if examined == 0 {
            "no space declares an envelope this gate can examine — nothing is `enclosed` or \
             `open_top`, so closure proved nothing"
                .to_string()
        } else {
            format!("{examined} boundary cell(s) examined; every one is accounted for")
        },
    }
}

// --- §2.4 edge proof -------------------------------------------------------

/// A copy of the model with a region replaced by air — what a `barred` edge's
/// second half is proved on.
fn with_voided(model: &VoxelModel, region: &BTreeSet<[i32; 3]>) -> VoxelModel {
    let mut copy = model.clone();
    let air = crate::block::BlockState::air();
    for &cell in region {
        if copy.get(cell).is_some() {
            let _ = copy.set(cell, &air);
        }
    }
    copy
}

fn edge_proof(ix: &Index, model: &VoxelModel) -> Gate {
    let mut proved = 0usize;
    let mut bad: Vec<String> = Vec::new();

    for (i, edge) in ix.contract.edges.iter().enumerate() {
        let site = format!("edge {}--{}--{}", edge.a, edge.class, edge.b);
        if edge.class == "vision" {
            continue; // no traversal claim; its via is a closure exemption only
        }
        if edge.a == EXTERIOR || edge.b == EXTERIOR {
            // Exterior is a face, not a node: it has no cells, so there is
            // nothing here to walk to. The claim it does make is checked by the
            // exterior-face gate.
            continue;
        }
        proved += 1;

        let a = ix.standable_in(&edge.a);
        let b = ix.standable_in(&edge.b);
        let via: BTreeSet<[i32; 3]> = ix.via_cells[i]
            .iter()
            .filter(|c| ix.standable.contains(*c))
            .copied()
            .collect();
        let mut graph: BTreeSet<[i32; 3]> = a.union(&b).copied().collect();
        graph.extend(via.iter().copied());
        // A bar's own cells belong to the walk it is standing in the way of.
        // Leave them out and the two ends are severed by the graph rather than
        // by the iron, and "the bar bars" passes over a doorway with nothing in
        // it at all.
        graph.extend(
            ix.bar_cells[i]
                .iter()
                .filter(|c| ix.standable.contains(*c))
                .copied(),
        );

        if a.is_empty() || b.is_empty() {
            bad.push(format!(
                "{site}: {} has no standable cell, so there is nothing to walk between",
                if a.is_empty() { &edge.a } else { &edge.b }
            ));
            continue;
        }

        // The declared level relation, measured over the resolved boxes. Three
        // of the four recorded bell-zone drifts reach geometry and are green on
        // every topology obligation; `rise` is what reds them.
        let measured = match (
            min_y(&ix.contract.spaces[&edge.a].boxes),
            min_y(&ix.contract.spaces[&edge.b].boxes),
        ) {
            (Some(ya), Some(yb)) => Some(i64::from(yb) - i64::from(ya)),
            _ => None,
        };
        if let (Some(declared), Some(measured)) = (edge.rise, measured)
            && declared != measured
        {
            bad.push(format!(
                "{site}: declares rise {declared} but the resolved boxes measure {measured} \
                 (min_y({}) - min_y({}))",
                edge.b, edge.a
            ));
        }

        match edge.class.as_str() {
            "walk" => {
                if !nav::connected(&graph, &a, &b) || !nav::connected(&graph, &b, &a) {
                    bad.push(format!(
                        "{site}: a walk connects both ways, and this one does not"
                    ));
                }
            }
            "stair" => {
                if via.is_empty() {
                    bad.push(format!(
                        "{site}: its transit volume holds no standable cell — a stair's treads are \
                         what a body climbs"
                    ));
                } else if !nav::connected(&graph, &a, &b) || !nav::connected(&graph, &b, &a) {
                    bad.push(format!(
                        "{site}: the climb does not connect its two ends through its own treads"
                    ));
                }
            }
            "drop" => {
                if !nav::reachable_with_fall(model, &graph, &a, &b) {
                    bad.push(format!(
                        "{site}: nothing falls from {} to {}",
                        edge.a, edge.b
                    ));
                }
                if nav::connected(&graph, &b, &a) {
                    bad.push(format!(
                        "{site}: a drop is one-way, and a body can walk back up from {} to {}",
                        edge.b, edge.a
                    ));
                }
            }
            "barred" => {
                if nav::connected(&graph, &a, &b) {
                    bad.push(format!(
                        "{site}: the bar does not bar anything — the two ends connect while it \
                         stands"
                    ));
                }
                let opened = with_voided(model, &ix.bar_cells[i]);
                let free = nav::standable_cells(&opened);
                let mut open_graph: BTreeSet<[i32; 3]> = graph.clone();
                open_graph.extend(
                    ix.bar_cells[i]
                        .iter()
                        .filter(|c| free.contains(*c))
                        .copied(),
                );
                let open_graph: BTreeSet<[i32; 3]> =
                    open_graph.intersection(&free).copied().collect();
                let oa: BTreeSet<[i32; 3]> = a.intersection(&free).copied().collect();
                let ob: BTreeSet<[i32; 3]> = b.intersection(&free).copied().collect();
                if !nav::connected(&open_graph, &oa, &ob) || !nav::connected(&open_graph, &ob, &oa)
                {
                    bad.push(format!(
                        "{site}: with the bar region voided the two ends still do not connect \
                         through it, so the bar is not what stands between them"
                    ));
                }
            }
            _ => {}
        }
    }

    // A zero binding is red **where an edge could have existed**. One space and
    // no edges is a room with a door, and spec-0036 §2.9 keeps that as a printed
    // finding rather than a red; two or more spaces with nothing proved between
    // them is a graph that is decoration, which is the thing the vacuity rule is
    // for.
    let could_have = ix.contract.spaces.len() > 1;
    Gate {
        id: "contract-edge-proof",
        state: verdict(bad.is_empty() && (proved > 0 || !could_have)),
        undecided: 0,
        bound: proved,
        detail: if !bad.is_empty() {
            bad.join(" · ")
        } else if proved == 0 && could_have {
            format!(
                "the contract declares {} spaces and NO edge between any two of them — this gate \
                 examined nothing, so no claim about how a body moves through the piece was \
                 proved, and the graph is decoration",
                ix.contract.spaces.len()
            )
        } else if proved == 0 {
            "one space and no interior edge: a room with a door has no traversal claim to prove, \
             so this gate examined nothing and says so rather than reporting a pass"
                .to_string()
        } else {
            format!(
                "{proved} interior edge(s) proved, each against its class and its declared rise"
            )
        },
    }
}

// --- §2.6 the out-of-walk obligation ---------------------------------------

/// Classify every out-of-walk region: strongest applicable, computed here and
/// never picked by the author.
fn no_body_kinds(
    ix: &Index,
    model: &VoxelModel,
    anchors: &BTreeMap<String, [i32; 3]>,
    enumeration: &mut Vec<String>,
) -> BTreeMap<String, Option<NoBodyKind>> {
    // `sealed` is a property of the union, so it is computed as one: start with
    // every region a candidate, and drop whichever region owns a cell whose
    // boundary is open, until the survivors' union is closed. A stranded gallery
    // opens onto the nave air and is dropped; a walled recess is not.
    let mut candidates: BTreeSet<&str> = ix.no_body_cells.keys().copied().collect();
    loop {
        let union: BTreeSet<[i32; 3]> = candidates
            .iter()
            .flat_map(|n| ix.no_body_cells[n].iter().copied())
            .collect();
        let mut guilty: BTreeSet<&str> = BTreeSet::new();
        for cell in &union {
            for d in DIRS {
                let n = [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]];
                if union.contains(&n) || !nav::passable(model, n) {
                    continue;
                }
                for (name, set) in &ix.no_body_cells {
                    if candidates.contains(name) && set.contains(cell) {
                        guilty.insert(name);
                    }
                }
            }
        }
        if guilty.is_empty() {
            break;
        }
        for name in guilty {
            candidates.remove(name);
        }
        if candidates.is_empty() {
            break;
        }
    }

    let outside = exterior_air(model);
    let mut out: BTreeMap<String, Option<NoBodyKind>> = BTreeMap::new();
    for (name, cells) in &ix.no_body_cells {
        let standable: BTreeSet<[i32; 3]> = cells
            .iter()
            .filter(|c| ix.standable.contains(*c))
            .copied()
            .collect();
        let nested = ix.space_cells.values().any(|s| !s.is_disjoint(cells));

        let kind = if candidates.contains(name) {
            Some(NoBodyKind::Sealed)
        } else {
            // `posted`: an anchor inside the region, and every standable cell
            // within Chebyshev 2 of one. Per cell deliberately — one decoy
            // anchor over a thousand stranded cells is the blanket this refuses.
            let inside: Vec<[i32; 3]> = anchors
                .values()
                .copied()
                .filter(|p| cells.contains(p))
                .collect();
            let posted = !inside.is_empty()
                && standable.iter().all(|c| {
                    inside
                        .iter()
                        .any(|a| (0..3).all(|axis| (c[axis] - a[axis]).abs() <= POSTED_RADIUS))
                });
            if posted {
                Some(NoBodyKind::Posted)
            } else if !nested
                && standable.iter().all(|c| outside.contains(c))
                && !standable.is_empty()
            {
                Some(NoBodyKind::Facade)
            } else {
                None
            }
        };

        match kind {
            Some(NoBodyKind::Sealed) => enumeration.push(format!(
                "no_body {name:?}: sealed — {} cell(s), the sealed union's own boundary is closed",
                cells.len()
            )),
            Some(NoBodyKind::Posted) => {
                let named: Vec<&str> = anchors
                    .iter()
                    .filter(|(_, p)| cells.contains(*p))
                    .map(|(n, _)| n.as_str())
                    .collect();
                enumeration.push(format!(
                    "no_body {name:?}: posted — {} standable cell(s), anchors {}",
                    standable.len(),
                    named.join(", ")
                ));
            }
            Some(NoBodyKind::Facade) => enumeration.push(format!(
                "no_body {name:?}: facade — {} standable cell(s), every one reached by the air \
                 outside the piece",
                standable.len()
            )),
            None => {}
        }
        out.insert((*name).to_string(), kind);
    }
    out
}

fn no_body_gate(ix: &Index, kinds: &BTreeMap<String, Option<NoBodyKind>>) -> Gate {
    let mut bad: Vec<String> = Vec::new();
    let mut per_kind: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, kind) in kinds {
        let cells = ix.no_body_cells[name.as_str()].len();
        match kind {
            // Its kind would be decided over an empty set. `no_body` means
            // "standable cells deliberately outside the walk", so a region with
            // none of them proved nothing about anything — the same vacuity a
            // zero-bound gate has, one object down.
            _ if ix.no_body_cells[name.as_str()]
                .iter()
                .all(|c| !ix.standable.contains(c)) =>
            {
                bad.push(format!(
                    "out-of-walk region {name:?} holds no standable cell at all, so its kind was \
                     decided over an empty set. A `no_body` region names floor a body could stand \
                     on and does not; this one names {cells} cell(s) nobody could stand on either \
                     way"
                ));
            }
            Some(k) => *per_kind.entry(k.as_str()).or_insert(0) += cells,
            None => bad.push(format!(
                "out-of-walk region {name:?} ({}) qualifies for NOTHING: its own boundary is not \
                 closed (not `sealed`), it holds no anchor covering its cells (not `posted`), and \
                 the air outside the piece does not reach it or it nests inside a space (not \
                 `facade`). The author's reason was {:?}",
                cells, ix.contract.no_body[name].reason
            )),
        }
    }
    let summary: Vec<String> = per_kind
        .iter()
        .map(|(k, n)| format!("{n} cell(s) {k}"))
        .collect();
    Gate {
        id: "contract-no-body",
        state: verdict(bad.is_empty()),
        undecided: 0,
        bound: kinds.len(),
        detail: if bad.is_empty() {
            format!(
                "{} out-of-walk region(s), every one earning a computed kind: {}",
                kinds.len(),
                if summary.is_empty() {
                    "none declared".to_string()
                } else {
                    summary.join(", ")
                }
            )
        } else {
            bad.join(" · ")
        },
    }
}

// --- §2.5 reachability, per cell and graph-confined -------------------------

/// The walk that makes an edge a checked claim: it may leave a space only
/// through a declared edge.
///
/// The physical reading — walk the blocks and see where you get — was rejected
/// in the prototype and the choice is load-bearing: under it, deleting an edge
/// changes nothing and edges decay into decoration.
struct Confined {
    /// Every element, in a fixed order.
    elements: Vec<Element>,
    /// Cell → the elements holding it.
    of_cell: BTreeMap<[i32; 3], Vec<usize>>,
    /// Directed step relations between elements, `(from, to)`.
    walk: BTreeSet<(usize, usize)>,
    /// Directed fall relations — a `drop` edge, and only forward.
    fall: BTreeSet<(usize, usize)>,
    /// Barred edges: `(bar region name, the relations it gates)`.
    bars: BTreeMap<String, Vec<(usize, usize)>>,
    /// Bar region name → its cells, so opening one can void the blocks as well
    /// as the relation.
    bar_regions: BTreeMap<String, BTreeSet<[i32; 3]>>,
}

fn build_confined(ix: &Index) -> Confined {
    let mut elements: Vec<Element> = Vec::new();
    let mut by_space: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, cells) in &ix.space_cells {
        by_space.insert(name, elements.len());
        elements.push(Element {
            label: format!("space {name}"),
            cells: cells.clone(),
        });
    }
    let mut walk: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut fall: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut bars: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    let mut bar_regions: BTreeMap<String, BTreeSet<[i32; 3]>> = BTreeMap::new();

    for (i, edge) in ix.contract.edges.iter().enumerate() {
        if !is_traversal(&edge.class) {
            continue;
        }
        let a = (edge.a != EXTERIOR).then(|| by_space[edge.a.as_str()]);
        let b = (edge.b != EXTERIOR).then(|| by_space[edge.b.as_str()]);
        let via = (!ix.via_cells[i].is_empty()).then(|| {
            let id = elements.len();
            elements.push(Element {
                label: format!(
                    "{} volume of edge {}--{}--{}",
                    edge.class, edge.a, edge.class, edge.b
                ),
                cells: ix.via_cells[i].clone(),
            });
            id
        });
        // A bar's own cells are a place too, once the bar is gone: a body walks
        // *through* the gateway, and a graph that hops the two rooms without it
        // would call a room reachable that no route enters.
        let bar = (!ix.bar_cells[i].is_empty()).then(|| {
            let id = elements.len();
            elements.push(Element {
                label: format!("bar region of edge {}--{}--{}", edge.a, edge.class, edge.b),
                cells: ix.bar_cells[i].clone(),
            });
            id
        });

        // The hops this edge licenses, in order: a -> via/bar -> b, plus a -> b
        // for an opening thin enough that no cell of it is stood in.
        let mut hops: Vec<(usize, usize)> = Vec::new();
        let chain: Vec<usize> = [a, via, bar, b].into_iter().flatten().collect();
        for pair in chain.windows(2) {
            hops.push((pair[0], pair[1]));
        }
        if let (Some(a), Some(b)) = (a, b) {
            hops.push((a, b));
        }

        let directed = edge.class == "drop";
        let mut relations: Vec<(usize, usize)> = Vec::new();
        for (from, to) in hops {
            relations.push((from, to));
            if !directed {
                relations.push((to, from));
            }
        }
        match edge.class.as_str() {
            "barred" => {
                let region = edge
                    .bar
                    .as_ref()
                    .map(|b| b.region.clone())
                    .unwrap_or_else(|| format!("edge#{i}"));
                bars.entry(region.clone()).or_default().extend(relations);
                bar_regions
                    .entry(region)
                    .or_default()
                    .extend(ix.bar_cells[i].iter().copied());
            }
            "drop" => {
                fall.extend(relations.iter().copied());
                walk.extend(relations);
            }
            _ => walk.extend(relations),
        }
    }

    let mut of_cell: BTreeMap<[i32; 3], Vec<usize>> = BTreeMap::new();
    for (id, element) in elements.iter().enumerate() {
        for &cell in &element.cells {
            of_cell.entry(cell).or_default().push(id);
        }
    }
    Confined {
        elements,
        of_cell,
        walk,
        fall,
        bars,
        bar_regions,
    }
}

impl Confined {
    fn allowed(&self, open_bars: &BTreeSet<&str>, from: usize, to: usize, falling: bool) -> bool {
        if from == to {
            return true;
        }
        let base = if falling { &self.fall } else { &self.walk };
        if base.contains(&(from, to)) {
            return true;
        }
        if falling {
            return false;
        }
        self.bars
            .iter()
            .any(|(name, rel)| open_bars.contains(name.as_str()) && rel.contains(&(from, to)))
    }

    fn hop(&self, open_bars: &BTreeSet<&str>, c: [i32; 3], d: [i32; 3], falling: bool) -> bool {
        let (Some(from), Some(to)) = (self.of_cell.get(&c), self.of_cell.get(&d)) else {
            return false;
        };
        from.iter()
            .any(|&f| to.iter().any(|&t| self.allowed(open_bars, f, t, falling)))
    }

    /// The cells the walk reaches, starting from `start`, with `open_bars` open.
    ///
    /// Opening a bar voids its **blocks** as well as its relation: a graph that
    /// hopped the two rooms while the iron still stood would call a room
    /// reachable that no route enters. So the walk runs over a model with the
    /// opened regions turned to air, and the gateway's own cells join the
    /// walkable set for as long as they are open.
    fn reach(
        &self,
        model: &VoxelModel,
        targets: &BTreeSet<[i32; 3]>,
        start: &BTreeSet<[i32; 3]>,
        open_bars: &BTreeSet<&str>,
    ) -> BTreeSet<[i32; 3]> {
        let mut opened: BTreeSet<[i32; 3]> = BTreeSet::new();
        for name in open_bars {
            if let Some(cells) = self.bar_regions.get(*name) {
                opened.extend(cells.iter().copied());
            }
        }
        let model = &if opened.is_empty() {
            model.clone()
        } else {
            with_voided(model, &opened)
        };
        let mut targets = targets.clone();
        if !opened.is_empty() {
            let free = nav::standable_cells(model);
            targets.extend(opened.iter().filter(|c| free.contains(*c)).copied());
        }
        let targets = &targets;
        let mut seen: BTreeSet<[i32; 3]> = start
            .iter()
            .filter(|c| targets.contains(*c))
            .copied()
            .collect();
        let mut queue: VecDeque<[i32; 3]> = seen.iter().copied().collect();
        while let Some([x, y, z]) = queue.pop_front() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                for dy in [0, 1, -1] {
                    let next = [x + dx, y + dy, z + dz];
                    if targets.contains(&next)
                        && !seen.contains(&next)
                        && self.hop(open_bars, [x, y, z], next, false)
                    {
                        seen.insert(next);
                        queue.push_back(next);
                    }
                }
                // A drop edge is walked off, not stepped down: land on the first
                // floor below, however far that is.
                let mut fy = y;
                loop {
                    fy -= 1;
                    if y - fy > 64 {
                        break;
                    }
                    let below = [x + dx, fy, z + dz];
                    match model.get(below) {
                        None => break,
                        Some(_) if nav::solid(model, below) => {
                            let landing = [x + dx, fy + 1, z + dz];
                            if targets.contains(&landing)
                                && !seen.contains(&landing)
                                && self.hop(open_bars, [x, y, z], landing, true)
                            {
                                seen.insert(landing);
                                queue.push_back(landing);
                            }
                            break;
                        }
                        _ => continue,
                    }
                }
            }
        }
        seen
    }
}

fn reachability(ix: &Index, model: &VoxelModel, enumeration: &mut Vec<String>) -> Gate {
    let confined = build_confined(ix);

    // Every standable cell of every space, minus the out-of-walk regions nested
    // in it — plus every standable cell of a transit volume, or an unreached
    // space could be deleted and its cells re-hung on a stair edge as 1x1x1
    // vias.
    let mut targets: BTreeSet<[i32; 3]> = BTreeSet::new();
    for element in &confined.elements {
        targets.extend(
            element
                .cells
                .iter()
                .filter(|c| ix.standable.contains(*c) && !ix.all_no_body_cells.contains(*c))
                .copied(),
        );
    }
    let start = ix.standable_in(&ix.contract.entry);

    let none: BTreeSet<&str> = BTreeSet::new();
    let reached = confined.reach(model, &targets, &start, &none);
    let mut unreached: BTreeSet<[i32; 3]> = targets.difference(&reached).copied().collect();

    // A space behind a bar is not unreachable; it is reachable once the bar is
    // opened, and the verdict says which bars that took.
    let mut required: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if !unreached.is_empty() && !confined.bars.is_empty() {
        let all: BTreeSet<&str> = confined.bars.keys().map(String::as_str).collect();
        let with_all = confined.reach(model, &targets, &start, &all);
        // Which bars each space actually needed: open them one at a time, in
        // name order, keeping the ones that let the walk somewhere new. Named
        // per space, because "unreachable behind a shortcut you have not opened
        // yet" and "unreachable" are different findings and only one of them is
        // a defect.
        let mut opened: BTreeSet<&str> = BTreeSet::new();
        let mut running: BTreeSet<[i32; 3]> = reached.intersection(&targets).copied().collect();
        for name in &all {
            let mut trial = opened.clone();
            trial.insert(name);
            let got: BTreeSet<[i32; 3]> = confined
                .reach(model, &targets, &start, &trial)
                .intersection(&targets)
                .copied()
                .collect();
            let newly: BTreeSet<[i32; 3]> = got.difference(&running).copied().collect();
            if newly.is_empty() {
                continue;
            }
            for element in &confined.elements {
                if element.cells.iter().any(|c| newly.contains(c)) {
                    required
                        .entry(element.label.clone())
                        .or_default()
                        .insert((*name).to_string());
                }
            }
            opened = trial;
            running = got;
        }
        unreached = targets.difference(&with_all).copied().collect();
        for (element, bars) in &required {
            let names: Vec<&str> = bars.iter().map(String::as_str).collect();
            enumeration.push(format!(
                "opened bars: {element} is reached only once {} is opened",
                names.join(" + ")
            ));
        }
    }

    let mut per_element: Vec<String> = Vec::new();
    for element in &confined.elements {
        let missing = element
            .cells
            .iter()
            .filter(|c| unreached.contains(*c))
            .count();
        if missing > 0 {
            per_element.push(format!("{}: {missing} cell(s)", element.label));
        }
    }

    Gate {
        id: "contract-reachability",
        state: verdict(unreached.is_empty() && !targets.is_empty()),
        undecided: 0,
        bound: targets.len(),
        detail: if !unreached.is_empty() {
            format!(
                "{} of {} standable cell(s) in declared space cannot be reached from the entry \
                 space {:?} by a walk confined to declared spaces and crossing only through \
                 declared edges — {} · {}",
                unreached.len(),
                targets.len(),
                ix.contract.entry,
                per_element.join(", "),
                describe_cells(&unreached)
            )
        } else if targets.is_empty() {
            "no declared space or transit volume holds a standable cell, so the walk had nowhere \
             to go and this gate proved nothing"
                .to_string()
        } else {
            format!(
                "every one of {} standable cell(s) in declared space is reached from {:?} through \
                 declared edges only{}",
                targets.len(),
                ix.contract.entry,
                if required.is_empty() {
                    String::new()
                } else {
                    format!(
                        ", {} of them only once a bar is opened ({})",
                        required.len(),
                        required
                            .iter()
                            .map(|(e, b)| format!(
                                "{e}: {}",
                                b.iter().cloned().collect::<Vec<_>>().join(" + ")
                            ))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                }
            )
        },
    }
}

// --- §2.7 anchors ----------------------------------------------------------

/// Which contract element an anchor lands in, in the form the metadata records.
pub fn resolves_to(contract: &SpatialContract, pos: [i32; 3]) -> Option<String> {
    let hit = |boxes: &[MetaRegion]| {
        boxes
            .iter()
            .any(|r| (0..3).all(|a| pos[a] >= r.from[a] && pos[a] <= r.to[a]))
    };
    for (name, region) in &contract.no_body {
        if hit(&region.boxes) {
            return Some(format!("no_body:{name}"));
        }
    }
    for edge in &contract.edges {
        if let Some(bar) = &edge.bar
            && hit(&bar.boxes)
        {
            return Some(format!("bar:{}", bar.region));
        }
        if let Some(via) = &edge.via
            && hit(&via.boxes)
        {
            return Some(format!("via:{}", via.region));
        }
    }
    for (name, space) in &contract.spaces {
        if hit(&space.boxes) {
            return Some(format!("space:{name}"));
        }
    }
    None
}

fn anchors_gate(
    ix: &Index,
    anchors: &BTreeMap<String, [i32; 3]>,
    kinds: &BTreeMap<String, Option<NoBodyKind>>,
    enumeration: &mut Vec<String>,
) -> Gate {
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    for (name, &pos) in anchors {
        match resolves_to(ix.contract, pos) {
            Some(element) => {
                let head = element.split(':').next().unwrap_or("").to_string();
                *by_kind.entry(head).or_insert(0) += 1;
                if let Some(region) = element.strip_prefix("no_body:")
                    && kinds.get(region).copied().flatten() != Some(NoBodyKind::Posted)
                {
                    enumeration.push(format!(
                        "anchor {name:?} sits in out-of-walk region {region:?}, which is not \
                         `posted` — the expected kind for a region something is placed in"
                    ));
                }
            }
            None => unresolved.push(format!("{name:?} at [{},{},{}]", pos[0], pos[1], pos[2])),
        }
    }
    let summary: Vec<String> = by_kind
        .iter()
        .map(|(k, n)| format!("{n} in a {k}"))
        .collect();
    Gate {
        id: "contract-anchors",
        state: verdict(unresolved.is_empty()),
        undecided: 0,
        bound: anchors.len(),
        detail: if unresolved.is_empty() {
            format!(
                "{} anchor(s), every one landing in a contract element: {}",
                anchors.len(),
                if summary.is_empty() {
                    "none declared".to_string()
                } else {
                    summary.join(", ")
                }
            )
        } else {
            format!(
                "{} anchor(s) land in nothing the contract declares — a campaign would bind \
                 content to a place the piece does not account for: {}",
                unresolved.len(),
                unresolved.join(", ")
            )
        },
    }
}

// --- §2.8 exterior faces ---------------------------------------------------

/// Which side of the piece a face is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FaceDir(pub [i32; 3]);

impl FaceDir {
    /// The keyword a metadata document and a refusal both use.
    pub fn as_str(self) -> &'static str {
        match self.0 {
            [1, 0, 0] => "east",
            [-1, 0, 0] => "west",
            [0, 1, 0] => "up",
            [0, -1, 0] => "down",
            [0, 0, 1] => "south",
            _ => "north",
        }
    }
}

/// One face of the piece's face contract: a way in or out, on a named side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExteriorFace {
    /// The space the face belongs to.
    pub space: String,
    /// The edge's class.
    pub class: String,
    /// Which side of the piece.
    pub dir: FaceDir,
    /// The opening's cells, in the plane of the face.
    pub cells: BTreeSet<[i32; 3]>,
}

/// The piece's face contract: every `exterior` edge, as the side it is on and
/// the cells it opens.
///
/// This is what `--traversable` re-derives its claim from, and what assembly
/// consumes. The old approach heuristic counted any standable cell on a face and
/// so reported 47 approaches where 3 were doors; a face contract counts doors,
/// because a door is a thing the author declared.
pub fn exterior_faces(model: &VoxelModel, contract: &SpatialContract) -> Vec<ExteriorFace> {
    let region = model.region();
    let min = region.origin;
    let max = region.maximum();
    let mut out = Vec::new();
    for edge in &contract.edges {
        if edge.a != EXTERIOR && edge.b != EXTERIOR {
            continue;
        }
        let space_name = if edge.a == EXTERIOR { &edge.b } else { &edge.a };
        let Some(space) = contract.spaces.get(space_name) else {
            continue;
        };
        // The opening's cells: the declared via when there is one, otherwise the
        // space's own cells that sit on the region's outer layer — the piece
        // stops there, and what it leaves open is its face.
        let opening = match &edge.via {
            Some(via) => cells(&via.boxes),
            None => cells(&space.boxes),
        };
        for (axis, dir) in [
            (0, [1, 0, 0]),
            (0, [-1, 0, 0]),
            (1, [0, 1, 0]),
            (1, [0, -1, 0]),
            (2, [0, 0, 1]),
            (2, [0, 0, -1]),
        ] {
            let plane = if dir[axis] > 0 {
                max[axis] - 1
            } else {
                min[axis]
            };
            let on_face: BTreeSet<[i32; 3]> = opening
                .iter()
                .filter(|c| c[axis] == plane && nav::passable(model, **c))
                .copied()
                .collect();
            if on_face.is_empty() {
                continue;
            }
            out.push(ExteriorFace {
                space: space_name.clone(),
                class: edge.class.clone(),
                dir: FaceDir(dir),
                cells: on_face,
            });
        }
    }
    out.sort_by(|a, b| {
        a.dir
            .cmp(&b.dir)
            .then_with(|| a.space.cmp(&b.space))
            .then_with(|| a.class.cmp(&b.class))
    });
    out
}

fn exterior_faces_gate(ix: &Index, model: &VoxelModel, enumeration: &mut Vec<String>) -> Gate {
    let declared = ix
        .contract
        .edges
        .iter()
        .filter(|e| e.a == EXTERIOR || e.b == EXTERIOR)
        .count();
    let faces = exterior_faces(model, ix.contract);
    let mut silent: Vec<String> = Vec::new();
    for edge in &ix.contract.edges {
        if edge.a != EXTERIOR && edge.b != EXTERIOR {
            continue;
        }
        let space = if edge.a == EXTERIOR { &edge.b } else { &edge.a };
        if !faces
            .iter()
            .any(|f| &f.space == space && f.class == edge.class)
        {
            silent.push(format!(
                "edge {}--{}--{} claims a way {} the piece, but no cell of {space:?} reaches the \
                 piece's outer face and it declares no opening that does — the face contract it \
                 exports is empty, so nothing downstream can mate with it",
                edge.a,
                edge.class,
                edge.b,
                if edge.class == "vision" {
                    "through"
                } else {
                    "into"
                }
            ));
        }
    }
    for face in &faces {
        enumeration.push(format!(
            "exterior face: {} on the {} side, via space {:?}, {} cell(s)",
            face.class,
            face.dir.as_str(),
            face.space,
            face.cells.len()
        ));
    }
    Gate {
        id: "contract-exterior-faces",
        state: verdict(silent.is_empty()),
        undecided: 0,
        bound: declared,
        detail: if !silent.is_empty() {
            silent.join(" · ")
        } else {
            format!(
                "{declared} exterior edge(s) export {} face(s): {}",
                faces.len(),
                faces
                    .iter()
                    .map(|f| format!("{} {}", f.dir.as_str(), f.class))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
    }
}

// --- §2.9 the out-of-walk majority -----------------------------------------

fn no_body_majority(ix: &Index, kinds: &BTreeMap<String, Option<NoBodyKind>>) -> Gate {
    let out_of_walk: BTreeSet<[i32; 3]> = ix
        .all_no_body_cells
        .iter()
        .filter(|c| ix.standable.contains(*c))
        .copied()
        .collect();
    let total = ix.standable.len();
    let majority = total > 0 && out_of_walk.len() * 2 > total;

    // What the acknowledgement can and cannot buy. It silences a majority made
    // of `sealed` and `facade` cells, whose demands are facts about the blocks.
    // It cannot silence a `posted` majority: `posted` is the one kind an author
    // secures by placing something, so a string plus a scattering of anchors
    // would be an author writing their own exemption.
    let posted_cells: usize = kinds
        .iter()
        .filter(|(_, k)| **k == Some(NoBodyKind::Posted))
        .map(|(n, _)| {
            ix.no_body_cells[n.as_str()]
                .iter()
                .filter(|c| ix.standable.contains(*c))
                .count()
        })
        .sum();
    let posted_majority = posted_cells * 2 > out_of_walk.len().max(1);
    let acknowledged = ix.contract.no_body_majority_ack.is_some();
    let excused = acknowledged && !posted_majority;

    Gate {
        id: "contract-no-body-majority",
        state: verdict(!majority || excused),
        undecided: 0,
        bound: total,
        detail: if !majority {
            format!(
                "{} of {} standable cell(s) are out of walk — the piece is mostly play space",
                out_of_walk.len(),
                total
            )
        } else if excused {
            format!(
                "{} of {} standable cell(s) are out of walk, acknowledged: {:?}. {posted_cells} of \
                 them are `posted`, which is not the majority of the out-of-walk floor",
                out_of_walk.len(),
                total,
                ix.contract.no_body_majority_ack.as_deref().unwrap_or("")
            )
        } else if acknowledged {
            format!(
                "{} of {} standable cell(s) are out of walk and {posted_cells} of those are \
                 `posted` — a majority. An acknowledgement cannot buy a `posted` majority: \
                 `posted` is the one kind an author secures by placing something, so the \
                 acknowledgement and the anchors would both be the author's own word",
                out_of_walk.len(),
                total
            )
        } else {
            format!(
                "{} of {} standable cell(s) are out of walk — most of this piece is not play \
                 space. Say so in `no_body_majority_ack` if that is what it is, which does not \
                 weaken any region's own proof",
                out_of_walk.len(),
                total
            )
        },
    }
}
