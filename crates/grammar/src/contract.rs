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

use crate::block::BlockState;
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

/// What one out-of-walk **cell** turned out to be (spec-0047 §2).
///
/// **Computed, never declared** (spec-0036 §0's corollary): an author who could
/// pick the kind would be picking which demand has to be met, and a choice among
/// demands is only ever as strong as the weakest on offer.
///
/// The region stays the unit of declaration — its name, its `reason`, its
/// coverage and its own reporting all key to it — and the kind is a fact about
/// blocks, so it is decided where the blocks are: at the cell. Keyed to the
/// region, the verdict moved with where an author drew boxes. A free-standing
/// pier whose deck stands in exterior air and whose masonry encloses one void
/// qualified for NOTHING as one region and passed as two, on the identical
/// bytes; the split that buys the pass is unauthorable where it matters,
/// because no rule owns a box around the cells a weighted mix seals per seed.
///
/// The variants are ordered strongest-first, and `Ord` is what picks between
/// them: the strongest applicable kind is the one a cell earns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NoBodyKind {
    /// Walled off: the whole passable component this cell's air belongs to lies
    /// inside the declared out-of-walk cells and touches no cell of the model's
    /// outer layer.
    Sealed,
    /// Anchored: within Chebyshev 2 of an anchor declared inside the cell's own
    /// region.
    Posted,
    /// Exterior dressing: the air outside the piece reaches the cell, and the
    /// cell lies inside no declared space.
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

/// An edge whose `via` is a **transit volume** (its own cells, disjoint from
/// every space) rather than an opening on a shared boundary.
///
/// A declared `way` makes any traversal edge one, because the cells the way
/// lays or clears have to BELONG to the edge (spec-0042 §2.1) — a level walk
/// whose deck is missing is a transit volume in exactly the way a stair's
/// treads are, and letting it declare an opening on a shared boundary instead
/// would put the laid cells inside a room.
fn is_transit(edge: &delvewright_schem::prefab::ContractEdge) -> bool {
    matches!(edge.class.as_str(), "stair" | "drop") || edge.way.is_some()
}

// ---------------------------------------------------------------------------
// The contingency, with `barred` normalised into it
// ---------------------------------------------------------------------------

/// Which direction opening a contingent region moves in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sign {
    /// The region is empty as built; opening fills it with the way's block.
    Laid,
    /// The region stands as built; opening voids it.
    Cleared,
}

/// One edge's contingency, **after `barred` has been normalised into it**
/// (spec-0042 §2.2).
///
/// `barred { bar }` means exactly `walk` + `way { cleared, bar.region,
/// bar.block }`, so this is what both spellings become before anything proves
/// anything. There is deliberately no second connectivity path for cleared
/// ways: a private copy of a general mechanism is the defect this corpus keeps
/// finding, and it is worst when the special case works, because then nothing
/// ever looks at it again.
#[derive(Debug, Clone)]
struct Contingency {
    /// The region name — what a verdict and, later, an effect both address.
    name: String,
    /// Which direction opening it moves in.
    sign: Sign,
    /// What a `laid` region is filled with. A `cleared` region is voided, so
    /// this is what it is expected to stand in and nothing reads it.
    block: BlockState,
    /// Its cells.
    cells: BTreeSet<[i32; 3]>,
    /// True when the author wrote `barred`.
    ///
    /// The prover is one; the **wording** is not. A refusal names the surface
    /// the author actually wrote, so a `barred` edge is still told about its
    /// bar and never about a "way" it never spelled.
    sugar: bool,
}

impl Contingency {
    /// The verb a verdict uses for opening it.
    fn verb(&self) -> &'static str {
        match (self.sugar, self.sign) {
            (true, _) => "opened",
            (false, Sign::Laid) => "laid",
            (false, Sign::Cleared) => "cleared",
        }
    }

    /// A copy of `model` with this contingency opened: filled for `laid`,
    /// voided for `cleared`.
    fn opened(&self, model: &VoxelModel) -> VoxelModel {
        let mut copy = model.clone();
        self.apply(&mut copy);
        copy
    }

    /// Apply the opening delta in place.
    fn apply(&self, model: &mut VoxelModel) {
        let block = match self.sign {
            Sign::Laid => self.block.clone(),
            Sign::Cleared => BlockState::air(),
        };
        for &cell in &self.cells {
            if model.get(cell).is_some() {
                let _ = model.set(cell, &block);
            }
        }
    }
}

/// The contingency of one metadata edge, with `barred` normalised in.
///
/// A `laid` way whose block does not parse resolves to air, which cannot open
/// anything; `well_formed` refuses the string before any of that matters, so
/// this never has to decide what an unparseable block means.
fn contingency_of(edge: &delvewright_schem::prefab::ContractEdge) -> Option<Contingency> {
    if let Some(way) = &edge.way {
        return Some(Contingency {
            name: way.region.clone(),
            sign: if way.opens == "laid" {
                Sign::Laid
            } else {
                Sign::Cleared
            },
            block: way.block.parse().unwrap_or_else(|_| BlockState::air()),
            cells: cells(&way.boxes),
            sugar: false,
        });
    }
    let bar = edge.bar.as_ref().filter(|_| edge.class == "barred")?;
    Some(Contingency {
        name: bar.region.clone(),
        sign: Sign::Cleared,
        block: bar.block.parse().unwrap_or_else(|_| BlockState::air()),
        cells: cells(&bar.boxes),
        sugar: true,
    })
}

/// The class the prover uses: `barred` is a `walk` carrying a cleared way.
fn proving_class(class: &str) -> &str {
    if class == "barred" { "walk" } else { class }
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
    /// Edge index → its contingency, `barred` normalised in (`None` when the
    /// edge is what it claims to be as shipped).
    contingency: Vec<Option<Contingency>>,
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
        let contingency: Vec<Option<Contingency>> =
            contract.edges.iter().map(contingency_of).collect();
        let all_space_cells = space_cells.values().flatten().copied().collect();
        let all_no_body_cells = no_body_cells.values().flatten().copied().collect();
        Index {
            contract,
            space_cells,
            no_body_cells,
            via_cells,
            contingency,
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
    gates.push(edge_proof(&ix, model, &mut enumeration));
    let kinds = no_body_kinds(&ix, model, anchors, &mut enumeration);
    gates.push(no_body_gate(&ix, &kinds));
    gates.push(reachability(&ix, model, &mut enumeration));
    gates.push(anchors_gate(&ix, anchors, &kinds, &mut enumeration));
    gates.push(exterior_faces_gate(&ix, model, &mut enumeration));
    gates.push(no_body_majority(&ix, &kinds));

    // §2.9's vacuity reds, judged by the one rule that judges every other
    // gate's. Sealed HERE and not only in `gates::judge`, because this report
    // has consumers that never reach `judge`: `delve-admit`'s spatial audit
    // reads it straight, and `export::refuse_broken_contract` refuses an
    // artifact on it. A zero binding raised here as a finding and nowhere as a
    // verdict is how a contract gate over nothing shipped a green.
    crate::gates::seal_zero_bindings(&mut gates, &mut findings, &mut enumeration);
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
        way_well_formed(ix, model, i, edge, &site, exterior, &mut bad);

        let via = &ix.via_cells[i];
        if via.is_empty() {
            continue;
        }
        if is_transit(edge) {
            // A transit volume is its own place: disjoint from every space, and
            // touching both ends.
            for (name, space) in &ix.space_cells {
                let overlap: BTreeSet<[i32; 3]> = via.intersection(space).copied().collect();
                if !overlap.is_empty() {
                    bad.push(format!(
                        "{site}: its transit volume overlaps space {name:?} on {} cell(s) ({}). A \
                         stair's treads, a drop's column and a way's laid cells belong to the \
                         edge, not to either end",
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
        empty_ok: None,
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

/// **A declared `way` is confined to its own edge's opening** (spec-0042 §2.1,
/// AC5).
///
/// The way region is the one place in this surface where an author names cells
/// that the shipped bytes do *not* have to agree with — a `laid` region is
/// empty as built and full later. Unconstrained, that is a build-anything
/// hatch: "content will put something here" over any cells at all. So every
/// demand here is §0-shaped — the region must be somewhere the defect cannot
/// put it — and the strongest is the last: **it lies inside the transit volume
/// of the edge it belongs to**, which no cell of a room, of another edge, or of
/// the world outside the piece can satisfy.
fn way_well_formed(
    ix: &Index,
    model: &VoxelModel,
    i: usize,
    edge: &delvewright_schem::prefab::ContractEdge,
    site: &str,
    exterior: bool,
    bad: &mut Vec<String>,
) {
    let Some(way) = &edge.way else {
        return;
    };
    if !matches!(way.opens.as_str(), "laid" | "cleared") {
        bad.push(format!(
            "{site}: {:?} is not a way sign — a way is `laid` (empty as built, filled to open) or \
             `cleared` (standing as built, voided to open)",
            way.opens
        ));
    }
    if edge.class == "vision" {
        bad.push(format!(
            "{site}: a sightline makes no traversal claim, so it has nothing to be contingent \
             about — a way says a body cannot cross yet, and no body was ever going to cross this"
        ));
    }
    if edge.class == "barred" {
        bad.push(format!(
            "{site}: this edge declares its contingency twice. `barred` IS a walk carrying a \
             cleared way over its bar's region; write one or the other, never both"
        ));
    }
    if exterior {
        bad.push(format!(
            "{site}: an edge with an `exterior` endpoint cannot carry a way — `exterior` has no \
             cells, so there is no far end for an opening to reach. A way across an assembly seam \
             is the face contract's business"
        ));
    }
    match way.block.parse::<BlockState>() {
        Ok(block) if block.is_air() => bad.push(format!(
            "{site}: the way {:?} is made of air, which opens nothing in either direction",
            way.region
        )),
        Ok(_) => {}
        Err(e) => bad.push(format!(
            "{site}: the way {:?} names the block {:?}, which is not a block state ({e})",
            way.region, way.block
        )),
    }

    let Some(cont) = &ix.contingency[i] else {
        return;
    };
    if cont.cells.is_empty() {
        bad.push(format!(
            "{site}: the way {:?} resolved to no cells at all, so opening it would change nothing \
             and the edge it claims to gate is gated by nothing",
            way.region
        ));
        return;
    }

    // Its own edge declares a transit volume, and the way lies inside it. This
    // is the demand stranding cannot supply: the cells have to belong to the
    // edge, and a transit volume is disjoint from every space and abuts both
    // ends (checked above, for every transit edge).
    let via = &ix.via_cells[i];
    if via.is_empty() {
        bad.push(format!(
            "{site}: an edge carrying a way declares a `via` — the cells a way lays or clears \
             belong to the edge, and without a transit volume there is nothing for them to lie \
             inside"
        ));
    } else {
        let outside: BTreeSet<[i32; 3]> = cont.cells.difference(via).copied().collect();
        if !outside.is_empty() {
            bad.push(format!(
                "{site}: {} of the way {:?}'s cell(s) lie outside this edge's own transit volume \
                 ({}). A way opens the edge that declares it and nothing else; a region reaching \
                 past it is a licence to build anywhere",
                outside.len(),
                way.region,
                describe_cells(&outside)
            ));
        }
    }

    // Disjoint from every space, and from every OTHER edge's own volumes. Both
    // follow from the containment above wherever that holds, and are stated
    // anyway so the red names the thing that is actually wrong.
    for (name, space) in &ix.space_cells {
        let overlap: BTreeSet<[i32; 3]> = cont.cells.intersection(space).copied().collect();
        if !overlap.is_empty() {
            bad.push(format!(
                "{site}: the way {:?} claims {} cell(s) of space {name:?} ({}). Opening a way \
                 rewrites its cells, and a room is not a thing an edge may rewrite",
                way.region,
                overlap.len(),
                describe_cells(&overlap)
            ));
        }
    }
    for (j, other) in ix.contract.edges.iter().enumerate() {
        if j == i {
            continue;
        }
        let mut theirs: BTreeSet<[i32; 3]> = ix.via_cells[j].clone();
        if let Some(c) = &ix.contingency[j] {
            theirs.extend(c.cells.iter().copied());
        }
        let overlap: BTreeSet<[i32; 3]> = cont.cells.intersection(&theirs).copied().collect();
        if !overlap.is_empty() {
            bad.push(format!(
                "{site}: the way {:?} shares {} cell(s) with edge {}--{}--{} ({}). Way regions are \
                 disjoint from every other edge's opening, bar and way — that disjointness is what \
                 makes opening MONOTONE, so opening one can never disconnect another",
                way.region,
                overlap.len(),
                other.a,
                other.class,
                other.b,
                describe_cells(&overlap)
            ));
        }
    }

    // A `laid` region is empty as built. Otherwise the break is not a break:
    // the treads are already there, the beat is decoration, and the closed
    // proof would be asked to fail against blocks that already connect.
    if cont.sign == Sign::Laid {
        let solid: BTreeSet<[i32; 3]> = cont
            .cells
            .iter()
            .filter(|c| nav::solid(model, **c))
            .copied()
            .collect();
        if !solid.is_empty() {
            bad.push(format!(
                "{site}: the way {:?} is declared `laid`, so its cells are empty as built — but {} \
                 of them hold this piece's own blocks ({}). A laid way is what is NOT there yet",
                way.region,
                solid.len(),
                describe_cells(&solid)
            ));
        }
    }
}

// --- §2.2 coverage ---------------------------------------------------------

/// Standable floor the contract accounts for nowhere — §2.2's population, and
/// also what makes an EMPTY `no_body` an honest claim rather than an unasked
/// question. Shared so the two cannot drift apart: if this said one thing to
/// the coverage gate and another to the out-of-walk gate, the second would be
/// excusing itself against a fact the first never established.
fn uncovered_standable(ix: &Index) -> BTreeSet<[i32; 3]> {
    let mut covered: BTreeSet<[i32; 3]> = ix.all_space_cells.clone();
    covered.extend(ix.all_no_body_cells.iter().copied());
    for (i, edge) in ix.contract.edges.iter().enumerate() {
        if is_traversal(&edge.class) {
            covered.extend(ix.via_cells[i].iter().copied());
        }
    }
    ix.standable.difference(&covered).copied().collect()
}

fn coverage(ix: &Index) -> Gate {
    let uncovered = uncovered_standable(ix);
    Gate {
        id: "contract-coverage",
        state: verdict(uncovered.is_empty()),
        undecided: 0,
        empty_ok: None,
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
        empty_ok: None,
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

/// **The class's own connectivity proof**, over one model and one graph.
///
/// One function, called three times per contingent edge — as built, on the
/// single-delta copy, and (through the reachability walk) with ways opened
/// cumulatively — so a cleared way and a laid way and a bar are all decided by
/// the same code. `Ok(())` means the class holds; `Err` carries the class's own
/// red, in its own words.
fn prove_class(
    class: &str,
    model: &VoxelModel,
    graph: &BTreeSet<[i32; 3]>,
    via: &BTreeSet<[i32; 3]>,
    a: &BTreeSet<[i32; 3]>,
    b: &BTreeSet<[i32; 3]>,
    ends: (&str, &str),
) -> Result<(), String> {
    match class {
        "walk" => {
            if nav::connected(model, graph, a, b) && nav::connected(model, graph, b, a) {
                Ok(())
            } else {
                Err("a walk connects both ways, and this one does not".to_string())
            }
        }
        "stair" => {
            if via.is_empty() {
                Err(
                    "its transit volume holds no standable cell — a stair's treads are what a \
                     body climbs"
                        .to_string(),
                )
            } else if nav::connected(model, graph, a, b) && nav::connected(model, graph, b, a) {
                Ok(())
            } else {
                Err("the climb does not connect its two ends through its own treads".to_string())
            }
        }
        "drop" => {
            if !nav::reachable_with_fall(model, graph, a, b) {
                Err(format!("nothing falls from {} to {}", ends.0, ends.1))
            } else if nav::connected(model, graph, b, a) {
                Err(format!(
                    "a drop is one-way, and a body can walk back up from {} to {}",
                    ends.1, ends.0
                ))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

fn edge_proof(ix: &Index, model: &VoxelModel, enumeration: &mut Vec<String>) -> Gate {
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
        let cont = ix.contingency[i].as_ref();
        // Every cell the edge could ever be walked through, whether or not it
        // is standable today: the two ends, the transit volume, and the
        // contingent region itself. A contingency's own cells belong to the
        // walk it decides — leave them out and the two ends are severed by the
        // graph rather than by the iron, and "the bar bars" passes over a
        // doorway with nothing in it at all.
        let mut span: BTreeSet<[i32; 3]> = ix.via_cells[i].clone();
        if let Some(c) = cont {
            span.extend(c.cells.iter().copied());
        }
        // The graph as built, over the cells a body can stand in today.
        let via: BTreeSet<[i32; 3]> = ix.via_cells[i]
            .iter()
            .filter(|c| ix.standable.contains(*c))
            .copied()
            .collect();
        let mut graph: BTreeSet<[i32; 3]> = a.union(&b).copied().collect();
        graph.extend(span.iter().filter(|c| ix.standable.contains(*c)).copied());

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

        let class = proving_class(&edge.class);
        let ends = (edge.a.as_str(), edge.b.as_str());
        let held = prove_class(class, model, &graph, &via, &a, &b, ends);

        let Some(cont) = cont else {
            // No contingency: the edge is what it claims to be as shipped, and
            // the class's proof is the whole of it.
            if let Err(red) = held {
                bad.push(format!("{site}: {red}"));
            }
            continue;
        };

        // **Part 1 — closed, on the bytes as shipped** (spec-0042 §2.1). The
        // class's proof must FAIL here: a way that opens something a body can
        // already cross is a beat that is not real, and the same demand as "the
        // bar does not bar anything" in the other sign. Run on `model`, never
        // on the copy below — a closed proof over the opened world passes
        // everything and proves nothing.
        if held.is_ok() {
            bad.push(if cont.sugar {
                format!(
                    "{site}: the bar does not bar anything — the two ends connect while it stands"
                )
            } else {
                format!(
                    "{site}: the way {:?} does not open anything: the two ends already connect \
                     without it",
                    cont.name
                )
            });
        }

        // **Part 2 — open, on a copy with the single delta applied.** Laid: the
        // region set to the way's block. Cleared: the region voided. The
        // class's own proof then has to HOLD — `walk` both ways, `stair`
        // through its treads, `drop` forward-only — over the cells a body can
        // stand in *there*, which for a laid way are mostly cells that did not
        // exist as built.
        let opened = cont.opened(model);
        let free = nav::standable_cells(&opened);
        let open_via: BTreeSet<[i32; 3]> = ix.via_cells[i].intersection(&free).copied().collect();
        let mut open_graph: BTreeSet<[i32; 3]> = a.union(&b).copied().collect();
        open_graph.extend(span.iter().copied());
        let open_graph: BTreeSet<[i32; 3]> = open_graph.intersection(&free).copied().collect();
        let oa: BTreeSet<[i32; 3]> = a.intersection(&free).copied().collect();
        let ob: BTreeSet<[i32; 3]> = b.intersection(&free).copied().collect();
        // **The opt-out, enumerated per instance** (spec-0036 §2.9, spec-0042
        // §4). A way is an opt-out from "reachable as built", so it is named —
        // by region, sign and cell count — rather than folded into a count.
        // This is also where the two block-level proof parts report what they
        // bound to: a way with nothing under it says so here, in a line a
        // reviewer disagrees with rather than a number a script satisfies.
        if !cont.sugar {
            enumeration.push(format!(
                "way {:?}: {} over {} cell(s) on {site} — closed on the bytes as shipped, open on \
                 the single-delta copy",
                cont.name,
                cont.verb(),
                cont.cells.len()
            ));
        }

        if let Err(red) = prove_class(class, &opened, &open_graph, &open_via, &oa, &ob, ends) {
            bad.push(if cont.sugar {
                format!(
                    "{site}: with the bar region voided the two ends still do not connect through \
                     it, so the bar is not what stands between them"
                )
            } else {
                format!(
                    "{site}: with the way {:?} {} the two ends still do not connect through it — \
                     {red}",
                    cont.name,
                    cont.verb()
                )
            });
        }
    }

    // A zero binding is red **where an edge could have existed**. One space and
    // no edges is a room with a door, and there is no interior traversal to
    // prove; two or more spaces with nothing proved between them is a graph
    // that is decoration, which is the thing the vacuity rule is for.
    //
    // This gate has decided its own zero honestly since it was written — and
    // the corpus audit reddened it anyway, because that door folded every
    // `bound == 0` into its red set without asking the gate. The judgement was
    // never missing here; it was being overruled. `empty_ok` is where it is now
    // stated so that the one rule reads it instead of second-guessing it.
    //
    // The defect cannot produce it: the population is empty only when the
    // document declares a single space, and a document cannot merge two spaces
    // to dodge an edge proof without §2.1 refusing the merged space for
    // spanning more than one floor, or §2.5 having to walk a body across the
    // whole of it from the entry.
    let could_have = ix.contract.spaces.len() > 1;
    let empty_ok = (proved == 0 && !could_have && bad.is_empty()).then(|| {
        "the contract declares one space, so there is no interior edge that could have been \
         proved: a room with a door makes no claim about moving BETWEEN spaces"
            .to_string()
    });
    Gate {
        id: "contract-edge-proof",
        state: verdict(bad.is_empty() && (proved > 0 || !could_have)),
        undecided: 0,
        empty_ok,
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

// --- §2.6 the out-of-walk obligation, per cell -----------------------------

/// Is this cell on the model's **outer layer** — the plane the piece stops at?
///
/// The clause that keeps `sealed` a statement about the piece's own masonry.
/// The model's region is the artifact's bounding box, so a cell there is closed
/// on that side by nothing but the end of the world, and a blanket declared out
/// to it would otherwise buy `sealed` for the whole sky (spec-0047 §1.4).
fn on_outer_layer(model: &VoxelModel, cell: [i32; 3]) -> bool {
    let region = model.region();
    let min = region.origin;
    let max = region.maximum();
    (0..3).any(|a| cell[a] == min[a] || cell[a] == max[a] - 1)
}

/// The **maximal passable component** containing `seed`: every passable cell
/// the air at `seed` is continuous with, however far it runs.
///
/// This is the object `sealed` quantifies over, and the reason stranding cannot
/// buy the kind. A stranded cell is stranded *with respect to something that
/// reaches its air* — the sky, the exterior, or play air through the breach —
/// and each of those is a cell in this set that no `no_body` declaration covers.
fn passable_component(model: &VoxelModel, seed: [i32; 3]) -> BTreeSet<[i32; 3]> {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    if !nav::passable(model, seed) {
        return seen;
    }
    seen.insert(seed);
    let mut queue: VecDeque<[i32; 3]> = VecDeque::from([seed]);
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

/// Why one cell earned no kind: the clause each demand refused it with, so a
/// red says which demand refused which cells rather than reciting all three.
///
/// A key rather than a sentence, because cells that were refused the same way
/// are one line in the verdict and cells refused differently are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Refused {
    sealed: &'static str,
    posted: &'static str,
    facade: &'static str,
}

/// The computed out-of-walk partition: **a kind per standable cell**
/// (spec-0047 §2), grouped by the region that declared it.
///
/// Per region as well as per cell because `posted` is the one demand that reads
/// the declaration — an anchor posts the cells of the region it stands in — so a
/// cell two regions declare can be posted in one and not the other, and each
/// region answers for its own cells. The other two demands are facts about
/// blocks and give the same answer wherever they are asked from.
struct CellKinds {
    /// Region name → its standable cells, each with the kind it earned there or
    /// the clauses that refused it.
    by_region: BTreeMap<String, BTreeMap<[i32; 3], Result<NoBodyKind, Refused>>>,
}

impl CellKinds {
    /// Every standable out-of-walk cell with the **strongest** kind it earned in
    /// any region declaring it — the piece-level view the majority gate counts
    /// and the summary reports.
    fn strongest(&self) -> BTreeMap<[i32; 3], Option<NoBodyKind>> {
        let mut out: BTreeMap<[i32; 3], Option<NoBodyKind>> = BTreeMap::new();
        for cells in self.by_region.values() {
            for (cell, kind) in cells {
                let slot = out.entry(*cell).or_insert(None);
                if let Ok(k) = kind
                    && slot.is_none_or(|had| k < &had)
                {
                    *slot = Some(*k);
                }
            }
        }
        out
    }

    /// Does this anchor stand among `posted` cells of the region it sits in?
    ///
    /// §2.7's expectation, re-keyed to the cell: the question is no longer "is
    /// this whole region `posted`" — a region can be part exterior dressing and
    /// part perch — but "did placing something here actually post anything".
    fn posts_anything(&self, region: &str, pos: [i32; 3]) -> bool {
        self.by_region.get(region).is_some_and(|cells| {
            cells.iter().any(|(c, kind)| {
                kind == &Ok(NoBodyKind::Posted)
                    && (0..3).all(|axis| (c[axis] - pos[axis]).abs() <= POSTED_RADIUS)
            })
        })
    }
}

/// Classify every standable out-of-walk cell: strongest applicable, computed
/// here and never picked by the author.
fn no_body_kinds(
    ix: &Index,
    model: &VoxelModel,
    anchors: &BTreeMap<String, [i32; 3]>,
    enumeration: &mut Vec<String>,
) -> CellKinds {
    let outside = exterior_air(model);
    // `sealed`'s two facts, computed once per component and shared by every cell
    // of it: the component escapes the declared out-of-walk cells, and the
    // component reaches the edge of the world. Memoised on the cell rather than
    // on a component id because the map is what every later lookup wants.
    let mut sealed_ok: BTreeMap<[i32; 3], bool> = BTreeMap::new();

    let mut by_region: BTreeMap<String, BTreeMap<[i32; 3], Result<NoBodyKind, Refused>>> =
        BTreeMap::new();
    for (name, cells) in &ix.no_body_cells {
        // The anchors this region declares. `posted` reads the declaration, so
        // it reads THIS region's — an anchor in the region next door posts
        // nothing here, which is what stops one decoy covering a piece.
        let inside: Vec<[i32; 3]> = anchors
            .values()
            .copied()
            .filter(|p| cells.contains(p))
            .collect();

        let mut per_cell: BTreeMap<[i32; 3], Result<NoBodyKind, Refused>> = BTreeMap::new();
        for &cell in cells.iter().filter(|c| ix.standable.contains(*c)) {
            // **`sealed`** — the cell's whole passable component lies inside the
            // declared out-of-walk cells AND touches no outer-layer cell. Both
            // halves are demands the defect cannot supply: a stranding always
            // leaves the component holding air nobody declared, and a blanket
            // out to the model's edge is closed by the world rather than by this
            // piece's blocks.
            let sealed = match sealed_ok.get(&cell) {
                Some(known) => *known,
                None => {
                    let component = passable_component(model, cell);
                    let escapes = component.iter().any(|c| !ix.all_no_body_cells.contains(c));
                    let open_to_edge = component.iter().any(|c| on_outer_layer(model, *c));
                    let sealed = !escapes && !open_to_edge;
                    // The answer is a property of the COMPONENT, so it is
                    // recorded for every cell of it. Recording it only for the
                    // cell that seeded the flood makes a deck of 615 cells
                    // standing in one body of exterior air flood the whole
                    // exterior 615 times — measured at 33.6s where the same zone
                    // took 0.33s, on a fact that was already known after the
                    // first cell.
                    for c in component {
                        sealed_ok.insert(c, sealed);
                    }
                    sealed
                }
            };
            if sealed {
                per_cell.insert(cell, Ok(NoBodyKind::Sealed));
                continue;
            }

            // **`posted`** — verbatim, and already per cell: within Chebyshev 2
            // of an anchor declared inside the cell's own region.
            let posted = inside
                .iter()
                .any(|a| (0..3).all(|axis| (cell[axis] - a[axis]).abs() <= POSTED_RADIUS));
            if posted {
                per_cell.insert(cell, Ok(NoBodyKind::Posted));
                continue;
            }

            // **`facade`** — verbatim: the exterior flood reaches the cell, and
            // the cell lies inside no declared space.
            let reached = outside.contains(&cell);
            let in_space = ix.all_space_cells.contains(&cell);
            if reached && !in_space {
                per_cell.insert(cell, Ok(NoBodyKind::Facade));
                continue;
            }

            per_cell.insert(
                cell,
                Err(Refused {
                    // Recomputed rather than carried out of the memo above: the
                    // first cell of a component fills the memo and the rest read
                    // it, so the clause has to be derivable from the cell alone.
                    sealed: if reached || on_outer_layer(model, cell) {
                        "its own boundary is not closed around it — the air it stands in reaches \
                         the edge of the world, which is not one of this piece's blocks"
                    } else {
                        "its own boundary is not closed around it — the air it stands in runs on \
                         into cells the contract does not declare out of walk"
                    },
                    posted: if inside.is_empty() {
                        "the region declares no anchor at all"
                    } else {
                        "no anchor the region declares stands within 2 cells of it"
                    },
                    facade: if in_space {
                        "it lies inside a declared space"
                    } else {
                        "the air outside the piece does not reach it"
                    },
                }),
            );
        }
        by_region.insert((*name).to_string(), per_cell);
    }

    let kinds = CellKinds { by_region };
    for (name, per_cell) in &kinds.by_region {
        let mut counts: BTreeMap<NoBodyKind, usize> = BTreeMap::new();
        for kind in per_cell.values().flatten() {
            *counts.entry(*kind).or_insert(0) += 1;
        }
        // The per-region breakdown by cell count (spec-0047 §2). A region that
        // came out one kind still says so first, because that is what a reviewer
        // greps for; a mixed one says what the mixture is, and the `facade`
        // share is complete — cells the flood reaches can no longer be counted
        // under `sealed`.
        let breakdown: Vec<String> = counts
            .iter()
            .map(|(k, n)| format!("{n} standable cell(s) {}", k.as_str()))
            .collect();
        match counts.iter().collect::<Vec<_>>().as_slice() {
            [(NoBodyKind::Sealed, n)] => enumeration.push(format!(
                "no_body {name:?}: sealed — {n} standable cell(s), the air each stands in is \
                 closed by this piece's own blocks and lies wholly inside the declared \
                 out-of-walk cells"
            )),
            [(NoBodyKind::Posted, n)] => {
                let named: Vec<&str> = anchors
                    .iter()
                    .filter(|(_, p)| ix.no_body_cells[name.as_str()].contains(*p))
                    .map(|(n, _)| n.as_str())
                    .collect();
                enumeration.push(format!(
                    "no_body {name:?}: posted — {n} standable cell(s), anchors {}",
                    named.join(", ")
                ));
            }
            [(NoBodyKind::Facade, n)] => enumeration.push(format!(
                "no_body {name:?}: facade — {n} standable cell(s), every one reached by the air \
                 outside the piece"
            )),
            [] => {}
            _ => enumeration.push(format!(
                "no_body {name:?}: mixed — {}, the kind computed per cell",
                breakdown.join(", ")
            )),
        }
    }
    kinds
}

fn no_body_gate(ix: &Index, kinds: &CellKinds) -> Gate {
    let mut bad: Vec<String> = Vec::new();
    for (name, per_cell) in &kinds.by_region {
        let cells = ix.no_body_cells[name.as_str()].len();
        // Its kind would be decided over an empty set. `no_body` means
        // "standable cells deliberately outside the walk", so a region with
        // none of them proved nothing about anything — the same vacuity a
        // zero-bound gate has, one object down.
        if per_cell.is_empty() {
            bad.push(format!(
                "out-of-walk region {name:?} holds no standable cell at all, so its kind was \
                 decided over an empty set. A `no_body` region names floor a body could stand on \
                 and does not; this one names {cells} cell(s) nobody could stand on either way"
            ));
            continue;
        }
        // A red names the kindless CELLS and which demand refused each, grouped
        // by the refusal so one line is one reason and not a recital.
        let mut refused: BTreeMap<Refused, BTreeSet<[i32; 3]>> = BTreeMap::new();
        for (cell, kind) in per_cell {
            if let Err(why) = kind {
                refused.entry(*why).or_default().insert(*cell);
            }
        }
        for (why, cells) in &refused {
            bad.push(format!(
                "out-of-walk region {name:?} qualifies for NOTHING on {} of its {} standable \
                 cell(s): {} (not `sealed`), {} (not `posted`), and {} (not `facade`). The \
                 author's reason was {:?} — {}",
                cells.len(),
                per_cell.len(),
                why.sealed,
                why.posted,
                why.facade,
                ix.contract.no_body[name].reason,
                describe_cells(cells)
            ));
        }
    }
    let strongest = kinds.strongest();
    let mut per_kind: BTreeMap<&str, usize> = BTreeMap::new();
    for kind in strongest.values().flatten() {
        *per_kind.entry(kind.as_str()).or_insert(0) += 1;
    }
    let summary: Vec<String> = per_kind
        .iter()
        .map(|(k, n)| format!("{n} cell(s) {k}"))
        .collect();
    // **Why declaring no out-of-walk region at all is honest, and when it is
    // not.** A piece where every standable cell is play space has no
    // out-of-walk floor to classify, and the only way to hand this gate an
    // object would be to declare a region that is not there — the exact vacuity
    // it exists to catch, one rung out.
    //
    // What makes the emptiness a CLAIM rather than a gap is that the floor did
    // not go anywhere: every standable cell must still land in a declared space
    // or a traversal edge's transit volume. That is §2.2's population, and an
    // author cannot empty it without the piece having no floor at all.
    //
    // So the defect cannot produce this. Deleting a region that qualifies for
    // nothing does not delete its cells: they must then sit in a space, where
    // §2.5 makes a body walk to every one of them through declared edges and
    // §2.3 closes the boundary around them. That is strictly more proof than
    // any `no_body` kind asks for, which is the test — an escape hatch that
    // costs more than the thing it escapes is not an escape hatch.
    let unaccounted = uncovered_standable(ix);
    let empty_ok = (kinds.by_region.is_empty()
        && !ix.standable.is_empty()
        && unaccounted.is_empty())
    .then(|| {
        format!(
            "the contract declares no out-of-walk region, and it does not need one: all {} \
             standable cell(s) lie in a declared space or a traversal edge's transit volume, so \
             every piece of floor here is play space and §2.5 must walk a body to it",
            ix.standable.len()
        )
    });
    Gate {
        id: "contract-no-body",
        state: verdict(bad.is_empty()),
        undecided: 0,
        empty_ok,
        bound: kinds.by_region.len(),
        detail: if bad.is_empty() {
            format!(
                "{} out-of-walk region(s), every standable cell of every one earning a computed \
                 kind: {}",
                kinds.by_region.len(),
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
    /// **Contingent edges, both signs**: region name → what it gates and what
    /// opening it does to the blocks.
    ///
    /// One map, not one per sign: a bar and a laid way are the same object
    /// here, a named region whose state decides whether a relation exists, and
    /// the only difference is which block the delta writes.
    ways: BTreeMap<String, WayGate>,
    /// Cells no walk ever counts: the out-of-walk regions.
    excluded: BTreeSet<[i32; 3]>,
}

/// One contingent region, as the reachability walk sees it.
struct WayGate {
    /// The step relations that exist only while it is open.
    relations: Vec<(usize, usize)>,
    /// The fall relations that exist only while it is open — a contingent
    /// `drop`, and only forward. Empty for every other class, which is why a
    /// bar reads exactly as it always did.
    fall: Vec<(usize, usize)>,
    /// The cells the delta rewrites.
    cells: BTreeSet<[i32; 3]>,
    /// Which direction opening moves in.
    sign: Sign,
    /// What a `laid` opening fills the cells with.
    block: BlockState,
    /// The verb a verdict uses — `opened` for a bar, `laid` / `cleared` for a
    /// declared way. The prover is one; the wording names what was written.
    verb: &'static str,
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
    let mut ways: BTreeMap<String, WayGate> = BTreeMap::new();

    for (i, edge) in ix.contract.edges.iter().enumerate() {
        if !is_traversal(&edge.class) {
            continue;
        }
        let cont = ix.contingency[i].as_ref();
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
        // would call a room reachable that no route enters. A declared way
        // needs no element of its own — §2.1 confines it inside this edge's
        // transit volume, which is already one.
        let bar = cont.filter(|c| c.sugar && !c.cells.is_empty()).map(|c| {
            let id = elements.len();
            elements.push(Element {
                label: format!("bar region of edge {}--{}--{}", edge.a, edge.class, edge.b),
                cells: c.cells.clone(),
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
        // **What a contingency contributes here is its BLOCKS**: the delta the
        // walk is re-run over, registered by name so a verdict can say which
        // openings a space needed.
        //
        // A `barred` edge additionally gates its *relations*, and keeps doing
        // so — that is the surface as it landed, and its bar region is an
        // element the graph would otherwise hop straight over. A declared way
        // does not, and must not: its cells lie inside a transit volume that is
        // part of the edge's own walk, so gating the relation would strand the
        // treads *below* a break as well as the ones a body cannot reach. The
        // blocks are the whole mechanism there, which is what makes the walk a
        // measurement of the building rather than of the declaration.
        if let Some(c) = cont {
            let gate = ways.entry(c.name.clone()).or_insert_with(|| WayGate {
                relations: Vec::new(),
                fall: Vec::new(),
                cells: BTreeSet::new(),
                sign: c.sign,
                block: c.block.clone(),
                verb: c.verb(),
            });
            gate.cells.extend(c.cells.iter().copied());
            if c.sugar {
                if directed {
                    gate.fall.extend(relations.iter().copied());
                }
                gate.relations.extend(relations);
                continue;
            }
        }
        if directed {
            fall.extend(relations.iter().copied());
        }
        walk.extend(relations);
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
        ways,
        excluded: ix.all_no_body_cells.clone(),
    }
}

impl Confined {
    fn allowed(&self, open: &BTreeSet<&str>, from: usize, to: usize, falling: bool) -> bool {
        if from == to {
            return true;
        }
        let base = if falling { &self.fall } else { &self.walk };
        if base.contains(&(from, to)) {
            return true;
        }
        self.ways.iter().any(|(name, gate)| {
            open.contains(name.as_str())
                && if falling { &gate.fall } else { &gate.relations }.contains(&(from, to))
        })
    }

    fn hop(&self, open: &BTreeSet<&str>, c: [i32; 3], d: [i32; 3], falling: bool) -> bool {
        let (Some(from), Some(to)) = (self.of_cell.get(&c), self.of_cell.get(&d)) else {
            return false;
        };
        from.iter()
            .any(|&f| to.iter().any(|&t| self.allowed(open, f, t, falling)))
    }

    /// **The walk in one opening state**: the cells it counts, and the cells it
    /// reaches, with the named ways open.
    ///
    /// Opening a way rewrites its **blocks** as well as its relation — voided
    /// for a `cleared` way, filled for a `laid` one — because a graph that
    /// hopped the two rooms while the iron still stood, or across a gap with
    /// nothing under it, would call a room reachable that no route enters.
    ///
    /// The target set is therefore recomputed *in this state* rather than fixed
    /// once as built: a laid way's whole point is that the cells a body stands
    /// on did not exist before it was laid, so a fixed as-built target set
    /// would leave every one of them out and make a laid way's reachability
    /// claim inert.
    fn reach(
        &self,
        model: &VoxelModel,
        start: &BTreeSet<[i32; 3]>,
        open: &BTreeSet<&str>,
    ) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
        let mut delta = model.clone();
        for name in open {
            if let Some(gate) = self.ways.get(*name) {
                let block = match gate.sign {
                    Sign::Laid => gate.block.clone(),
                    Sign::Cleared => BlockState::air(),
                };
                for &cell in &gate.cells {
                    if delta.get(cell).is_some() {
                        let _ = delta.set(cell, &block);
                    }
                }
            }
        }
        let model = &delta;
        // Every standable cell of every element, minus the out-of-walk regions
        // nested in them — judged over the blocks as they are in THIS state.
        let free = nav::standable_cells(model);
        let mut targets: BTreeSet<[i32; 3]> = BTreeSet::new();
        for element in &self.elements {
            targets.extend(
                element
                    .cells
                    .iter()
                    .filter(|c| free.contains(*c) && !self.excluded.contains(*c))
                    .copied(),
            );
        }
        let targets = &targets;
        let open_bars = open;
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
        (targets.clone(), seen)
    }
}

fn reachability(ix: &Index, model: &VoxelModel, enumeration: &mut Vec<String>) -> Gate {
    let confined = build_confined(ix);

    let start = ix.standable_in(&ix.contract.entry);

    // **Ways shut**: the walk over the bytes as shipped. Its target set is
    // every standable cell of every element (spaces, minus nested out-of-walk
    // cells, plus transit volumes — or an unreached space could be deleted and
    // its cells re-hung on a stair edge as 1x1x1 vias).
    let none: BTreeSet<&str> = BTreeSet::new();
    let (mut targets, mut reached) = confined.reach(model, &start, &none);

    // **Then opened cumulatively, by name** (spec-0042 §2.1/§3). A space behind
    // a way is not unreachable; it is reachable once the way is opened, and the
    // verdict says which openings that took.
    //
    // The union is what is proved, over the states along the chain: a cell is
    // red only when NO opening state reaches it. Taking the all-open state
    // alone would be wrong in the laid direction, where a cell standable as
    // built can stop being standable once the floor beside it is laid, and
    // where the interesting cells do not exist until something is.
    //
    // Combinations are not enumerated, and do not need to be: §2.1 makes way
    // regions disjoint from every other opening, which is exactly what makes
    // opening MONOTONE — opening more can never disconnect a proved edge.
    let mut required: BTreeMap<String, BTreeMap<&'static str, BTreeSet<String>>> = BTreeMap::new();
    if targets.difference(&reached).next().is_some() && !confined.ways.is_empty() {
        let all: BTreeSet<&str> = confined.ways.keys().map(String::as_str).collect();
        let mut opened: BTreeSet<&str> = BTreeSet::new();
        for name in &all {
            let mut trial = opened.clone();
            trial.insert(name);
            let (got_targets, got) = confined.reach(model, &start, &trial);
            let newly: BTreeSet<[i32; 3]> = got.difference(&reached).copied().collect();
            targets.extend(got_targets.iter().copied());
            reached.extend(got.iter().copied());
            if newly.is_empty() {
                continue;
            }
            let verb = confined.ways[*name].verb;
            for element in &confined.elements {
                if element.cells.iter().any(|c| newly.contains(c)) {
                    required
                        .entry(element.label.clone())
                        .or_default()
                        .entry(verb)
                        .or_default()
                        .insert((*name).to_string());
                }
            }
            opened = trial;
        }
        let (all_targets, all_reached) = confined.reach(model, &start, &all);
        targets.extend(all_targets);
        reached.extend(all_reached);
        for (element, by_verb) in &required {
            for (verb, names) in by_verb {
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                enumeration.push(format!(
                    "opened {}: {element} is reached only once {} is {verb}",
                    if *verb == "opened" { "bars" } else { "ways" },
                    names.join(" + ")
                ));
            }
        }
    }
    let unreached: BTreeSet<[i32; 3]> = targets.difference(&reached).copied().collect();

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
        empty_ok: None,
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
                        ", {} of them only once a {} is opened ({})",
                        required.len(),
                        // Named for what the author wrote: a piece whose only
                        // contingencies are bars is told about bars.
                        if required
                            .values()
                            .all(|by_verb| by_verb.keys().all(|v| *v == "opened"))
                        {
                            "bar"
                        } else {
                            "way"
                        },
                        required
                            .iter()
                            .map(|(element, by_verb)| format!(
                                "{element}: {}",
                                by_verb
                                    .iter()
                                    .map(|(verb, names)| format!(
                                        "{} {verb}",
                                        names.iter().cloned().collect::<Vec<_>>().join(" + ")
                                    ))
                                    .collect::<Vec<_>>()
                                    .join(", ")
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
        // Before `via`, because a way lies inside its edge's transit volume:
        // the narrower element is the one that names the place.
        if let Some(way) = &edge.way
            && hit(&way.boxes)
        {
            return Some(format!("way:{}", way.region));
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
    kinds: &CellKinds,
    enumeration: &mut Vec<String>,
) -> Gate {
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    for (name, &pos) in anchors {
        match resolves_to(ix.contract, pos) {
            Some(element) => {
                let head = element.split(':').next().unwrap_or("").to_string();
                *by_kind.entry(head).or_insert(0) += 1;
                // **The expectation, re-keyed to the cell** (spec-0047 §2): a
                // region can be part exterior dressing and part perch, so the
                // question is not "is this whole region `posted`" but "did
                // placing something here post anything at all".
                if let Some(region) = element.strip_prefix("no_body:")
                    && !kinds.posts_anything(region, pos)
                {
                    enumeration.push(format!(
                        "anchor {name:?} sits in out-of-walk region {region:?} and posts none of \
                         its cells — `posted` is the expected kind for the floor a thing is placed \
                         on"
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
    // **A piece that names no place inside itself.** A corridor or a wall
    // segment declares no anchor, and there is then no anchor that could sit
    // outside a declared element — the obligation is per anchor, and the set is
    // empty. The repo has always held this: every fixture in
    // `tests/contract_check.rs` passes `no_anchors()` and asserts the report
    // PASSES. Only the corpus audit disagreed, by folding every `bound == 0`
    // into its red set without asking the gate.
    //
    // The defect cannot produce it, and this one is worth stating exactly,
    // because the reasoning is not the same as the other two. The defect is an
    // anchor that lands outside every declared element. A piece holding one
    // *plus any other anchor* still binds non-zero and still reds — so emptying
    // the population is not a dodge available to a piece that has anchors at
    // all. Reaching zero means deleting every anchor including the good ones,
    // which deletes the only way a campaign can name a location inside the
    // piece: the escape route destroys the thing the anchors were for.
    let empty_ok = anchors.is_empty().then(|| {
        "the piece declares no anchor, so no anchor can sit outside a declared element. Nothing \
         in a campaign can name a place inside this piece either, which is a fact about the piece \
         rather than a gate's verdict"
            .to_string()
    });
    Gate {
        id: "contract-anchors",
        state: verdict(unresolved.is_empty()),
        undecided: 0,
        empty_ok,
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
        empty_ok: None,
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

fn no_body_majority(ix: &Index, kinds: &CellKinds) -> Gate {
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
    //
    // Counted over CELLS as classified (spec-0047 §2), which is what the kinds
    // are now facts about: a region that is half perch and half exterior
    // dressing contributes exactly its perches, where the region-keyed count
    // contributed all of it or none of it depending on which kind the region as
    // a whole came out.
    let posted_cells = kinds
        .strongest()
        .values()
        .filter(|k| **k == Some(NoBodyKind::Posted))
        .count();
    let posted_majority = posted_cells * 2 > out_of_walk.len().max(1);
    let acknowledged = ix.contract.no_body_majority_ack.is_some();
    let excused = acknowledged && !posted_majority;

    Gate {
        id: "contract-no-body-majority",
        state: verdict(!majority || excused),
        undecided: 0,
        empty_ok: None,
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
