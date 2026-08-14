//! Driving a **sweep**: many expansions of one program, laid out for the
//! contact sheet the owner curates massing from (spec-0027 §3).
//!
//! `spec-0027 §6` records the gap this module closes. Every other step of the
//! §3 authoring loop was built — the expander, the `.nbt` export, the
//! contact-sheet compositor — and the step between them was "assembled by hand".
//! A hand-assembled sweep is not reproducible, so the page it produced was a
//! picture rather than a decision record: nothing on it said which inputs made
//! which cell, and nothing could rebuild it later to act on the owner's pick.
//!
//! # A candidate is a *variation*, not a seed
//!
//! The obvious shape for this module is "expand at N seeds", and it is the wrong
//! one. An expansion has four inputs — program, region, parameters, seed — and
//! the seed is the *least* load-bearing of them for the programs we actually
//! have: a box-split grammar picks alternatives by **guards on the scope's own
//! dimensions**, and only reaches for the RNG when two alternatives are
//! applicable at once. Five of the eight bell zones say so in their own fixture
//! notes ("nothing in the gatehouse draws from the seed; it is stated, not
//! chosen"), and measurement agrees: they are byte-identical across 32 seeds.
//!
//! So a seed-only sweep would have been a surface bound to the one axis that
//! happens to be inert, leaving the two that work — region and parameters —
//! with no way in, exactly the "keyed to the verb, not the object class" shape
//! CLAUDE.md names. [`Candidate`] therefore varies **any** of the three, and the
//! seed is one field of it rather than the whole idea.
//!
//! # What a sweep reports
//!
//! [`SweepReport`] states, per sweep, how many candidates were asked for, how
//! many expanded, and — the number that decides whether the owner has a choice
//! at all — how many **distinct models** and **distinct massings** came out. A
//! sweep of six candidates that produced one distinct massing is a finding, not
//! a page: it means every cell of the contact sheet is the same building, and
//! the "pick one" gate in front of it is empty. That count is computed from the
//! models, never from the pictures, because two renders can differ by a shadow
//! and two identical renders can hide a moved wall behind a roof.
//!
//! # What "the same building" means, and why the gate turns on it
//!
//! Both counts are taken **up to placement**: two candidates are the same when
//! one can be carried onto the other by a whole-body move a placement could
//! undo — a translation, one of the four yaw rotations, a horizontal mirror.
//! Nothing else is quotiented. Paint still separates models, shape still
//! separates massings, and a building turned *upside down* is a different
//! building, because gravity is not a symmetry and nothing downstream can set
//! it down that way. [`crate::model::VoxelModel::placement_canonical_bytes`]
//! carries the construction.
//!
//! Blind to pose is not a nicety here, it is the difference between a gate and
//! a number. Every bell zone opens `Reorient::KEEP.y(WorldY).z(Largest)`, which
//! normalises the scope onto its longer horizontal axis — so a region and its
//! transpose expand to *the same building written on swapped world axes*. Under
//! a pose-sensitive digest, `bell:barrow-shore` at `[19,6,24]` and `[24,6,19]`
//! reported two distinct massings, identical in filled cells, and the page held
//! one building. The gate that decides whether a sheet is worth the owner's
//! hour could be passed by candidates that offer her no choice — the vacuous
//! green CLAUDE.md names, in the place it costs most. A real curation pass hit
//! it: one zone reported four and held three.
//!
//! The other direction is the same defect mirrored, so the equivalence stops
//! where it does deliberately. A coarser one — ignoring the vertical, or scale,
//! or interior arrangement — would merge buildings that really are different
//! and hide choices instead of inventing them. `tests` below measure both
//! directions on the same zone.
//!
//! # What a candidate carries besides its blocks
//!
//! A grey solid is not the medium a massing decision is made in. The reviewer's
//! questions are *where does the party come in, where does it leave, which cells
//! can be walked on, and where is every declared anchor* — and the program
//! already knows all four. It computes anchors (rules declare them), and the
//! floor is derivable from the model with the same rule the generator's gates
//! assert with ([`crate::floor`]).
//!
//! So every candidate that builds gets a **semantics sidecar**, `<id>.json`
//! beside `<id>.nbt`, in the prefab-metadata shape `delve-render` already reads
//! — which is what puts an anchor on the picture rather than in a field nobody
//! draws. [`Built`] carries the counts, so `sweep.json` alone answers "did this
//! zone declare anything at all", and [`SweepReport`] states the binding: a
//! sweep whose candidates declared **zero** anchors is a finding about the
//! programs, not a page.
//!
//! What is emitted is exactly what is known. Anchors are **declared**;
//! standable cells and the boundary openings are **derived**; which opening is
//! the entrance is **neither**, today, and nothing here invents one — see
//! [`crate::floor`].
//!
//! Nothing here ships: sweeps, snapshots and sheets are generation-time working
//! material (ADR-0013), and none of it can move a delve's bytes (ADR-0006).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::eval::GuardRefusal;
use crate::expand::{ExpandError, ExpandOptions, Expansion, expand};
use crate::export::{ExportError, program_hash, snapshot_nbt};
use crate::floor::{FloorPlan, Openings};
use crate::geom::Box3;
use crate::ir::Program;
use crate::library::bell;

/// The manifest schema id, written into every report.
pub const SCHEMA: &str = "delvewright.grammar-sweep/1";

/// The schema id of the per-candidate semantics sidecar.
pub const SEMANTICS_SCHEMA: &str = "delvewright.snapshot-semantics/1";

/// Anchor names the engine treats as the party's way in and out.
///
/// These are the **engine's** reserved names (`crates/dsl/data/anchors.json`),
/// not one campaign's vocabulary: any creator's piece declares its entrance by
/// marking one of these, whatever the fiction calls the door. Matched on the
/// last path segment, so `entry` and `anchor/entry` are the same declaration.
pub const WAY_IN_NAMES: [&str; 2] = ["spawn", "entry"];

/// Anchor names the engine treats as the party's way onward.
pub const WAY_OUT_NAMES: [&str; 1] = ["exit"];

/// The last path segment of an anchor name (`anchor/gate-north` → `gate-north`).
fn stem_of(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// A program this crate can sweep by name, with the box it is designed for.
pub struct Entry {
    /// The program's stable id (`bell:gate-ward`).
    pub id: String,
    /// Builds the program.
    pub program: fn() -> Program,
    /// The design box.
    pub region: [u32; 3],
}

/// Every program that declares a design box, by id.
///
/// A registry rather than a `match`: a tool enumerates this to discover what it
/// can sweep, so a new zone reaches `delve-grammar list`, the sweep and any
/// future driver without one of them being edited. Programs with no design box
/// (the ported temple/castle/church fixtures) are deliberately absent — a sweep
/// needs a box, and inventing one for them here would be a fixture masquerading
/// as a design.
pub fn registry() -> Vec<Entry> {
    bell::ZONES
        .iter()
        .map(|z| Entry {
            id: format!("bell:{}", z.id),
            program: z.program,
            region: z.region,
        })
        .collect()
}

/// Look one program up by id.
pub fn lookup(id: &str) -> Option<Entry> {
    registry().into_iter().find(|e| e.id == id)
}

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

/// One candidate: how this expansion differs from the sweep's baseline.
///
/// Every field but `id` is an override — absent means "the sweep's baseline",
/// so a manifest states only what varies and a reader sees the axis at a glance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Stable id. Becomes the snapshot filename, the render directory and the
    /// cell label on the contact sheet — what the owner says out loud when she
    /// picks one. Lowercase letters, digits and hyphens.
    pub id: String,
    /// Expansion seed. Absent → the sweep's baseline seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Region override. Absent → the program's design box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<[u32; 3]>,
    /// Parameter overrides, applied over the program's own defaults.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, i64>,
}

impl Candidate {
    /// A candidate that varies nothing but the seed.
    pub fn at_seed(id: impl Into<String>, seed: u64) -> Candidate {
        Candidate {
            id: id.into(),
            seed: Some(seed),
            region: None,
            params: BTreeMap::new(),
        }
    }
}

/// A sweep: one program, a baseline, and the candidates that vary from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always [`SCHEMA`].
    pub schema: String,
    /// The program id, as [`registry`] names it.
    pub program: String,
    /// The baseline seed every candidate inherits unless it says otherwise.
    pub seed: u64,
    /// The baseline region. Absent → the program's design box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<[u32; 3]>,
    /// The candidates, in the order they were asked for.
    pub candidates: Vec<Candidate>,
}

impl Manifest {
    /// A seed-only sweep of one program — the simple case, spelled out.
    pub fn over_seeds(program: &str, seeds: &[u64]) -> Manifest {
        Manifest {
            schema: SCHEMA.to_string(),
            program: program.to_string(),
            seed: *seeds.first().unwrap_or(&0),
            region: None,
            candidates: seeds
                .iter()
                .map(|s| Candidate::at_seed(format!("seed-{s:03}"), *s))
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a sweep could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SweepError {
    /// The manifest names a program the registry does not have.
    UnknownProgram {
        /// The id as given.
        id: String,
        /// What the registry does have.
        known: Vec<String>,
    },
    /// The manifest's schema field is not [`SCHEMA`].
    BadSchema {
        /// The schema as given.
        got: String,
    },
    /// The sweep has no candidates. A zero-candidate sweep would write an empty
    /// directory and a contact sheet with nothing on it, which reads as "nothing
    /// varied" rather than "nothing was asked for".
    NoCandidates,
    /// Two candidates share an id, so one would overwrite the other's snapshot.
    DuplicateId {
        /// The repeated id.
        id: String,
    },
    /// A candidate id is not usable as a filename / sheet label.
    BadId {
        /// The id as given.
        id: String,
    },
    /// A candidate names a parameter the program does not declare. Refused
    /// rather than ignored: a silently-dropped override is a candidate that
    /// looks varied on the manifest and is a duplicate on the page.
    UnknownParam {
        /// The candidate.
        candidate: String,
        /// The parameter as given.
        param: String,
    },
}

impl fmt::Display for SweepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SweepError::UnknownProgram { id, known } => write!(
                f,
                "no program {id:?}; this crate can sweep: {}",
                known.join(", ")
            ),
            SweepError::BadSchema { got } => {
                write!(f, "expected schema {SCHEMA:?}, found {got:?}")
            }
            SweepError::NoCandidates => write!(
                f,
                "the sweep declares no candidates — an empty page cannot be told apart from a \
                 page whose candidates all came out the same, and those two mean opposite things"
            ),
            SweepError::DuplicateId { id } => write!(
                f,
                "two candidates are both called {id:?}; ids name files and sheet cells, so one \
                 would silently replace the other"
            ),
            SweepError::BadId { id } => write!(
                f,
                "candidate id {id:?} is not usable: use lowercase letters, digits and hyphens \
                 (it becomes a filename and a label on the owner's page)"
            ),
            SweepError::UnknownParam { candidate, param } => write!(
                f,
                "candidate {candidate:?} sets {param:?}, which this program does not declare — \
                 an override that binds to nothing makes a candidate that is varied on paper and \
                 identical on the page"
            ),
        }
    }
}

impl std::error::Error for SweepError {}

// ---------------------------------------------------------------------------
// Running
// ---------------------------------------------------------------------------

/// One anchor as the sidecar and the report carry it.
///
/// `pos` and `facing` are the shape a prefab's metadata already uses, so the
/// render layer reads this file with the code path it has. `declared_by` is the
/// third thing the expansion knows and the metadata shape has no room for — the
/// rule that put the anchor there — and it is what turns "there is a marker at
/// 12,1,40" into "the ambush alcove is at 12,1,40" on a reviewer's page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorRow {
    /// Local cell `[x, y, z]`.
    pub pos: [i32; 3],
    /// Cardinal facing keyword.
    pub facing: String,
    /// The rule that declared it.
    pub declared_by: String,
}

/// The per-candidate semantics sidecar: everything the program knows about this
/// building that a picture of its blocks cannot show.
///
/// Written as `<id>.json` beside `<id>.nbt`, which is exactly where
/// `delve-render` looks for a piece's metadata — so the anchors reach the shot
/// planner and the plan key with no adapter, and a consumer that only wants the
/// blocks is unaffected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Semantics {
    /// Always [`SEMANTICS_SCHEMA`].
    pub schema: String,
    /// The candidate id.
    pub id: String,
    /// The program this candidate came from.
    pub program: String,
    /// `sha256:` over the program's canonical JSON.
    pub program_hash: String,
    /// The seed this candidate expanded at.
    pub seed: u64,
    /// The region it expanded over.
    pub region: [u32; 3],
    /// The parameter overrides applied.
    pub params: BTreeMap<String, i64>,
    /// Declared anchors, by exported name.
    pub anchors: BTreeMap<String, AnchorRow>,
    /// The derived walkable floor.
    pub floor: FloorPlan,
    /// The derived boundary openings.
    pub openings: Openings,
    /// Anchors naming the party's way in ([`WAY_IN_NAMES`]) — empty when the
    /// program declares none, which is a statement and not an omission.
    pub declared_entries: Vec<String>,
    /// Anchors naming the party's way onward ([`WAY_OUT_NAMES`]).
    pub declared_exits: Vec<String>,
}

impl Semantics {
    /// Read one expansion.
    fn of(
        id: &str,
        program: &Program,
        program_hash: &str,
        seed: u64,
        region: [u32; 3],
        params: &BTreeMap<String, i64>,
        expansion: &Expansion,
    ) -> Semantics {
        let anchors: BTreeMap<String, AnchorRow> = expansion
            .anchors
            .iter()
            .map(|(name, a)| {
                (
                    name.clone(),
                    AnchorRow {
                        pos: a.pos,
                        facing: a.facing.to_string(),
                        declared_by: a.declared_by.clone(),
                    },
                )
            })
            .collect();
        let named = |set: &[&str]| -> Vec<String> {
            anchors
                .keys()
                .filter(|n| set.contains(&stem_of(n)))
                .cloned()
                .collect()
        };
        Semantics {
            schema: SEMANTICS_SCHEMA.to_string(),
            id: id.to_string(),
            program: program.name.clone(),
            program_hash: program_hash.to_string(),
            seed,
            region,
            params: params.clone(),
            declared_entries: named(&WAY_IN_NAMES),
            declared_exits: named(&WAY_OUT_NAMES),
            anchors,
            floor: FloorPlan::of(&expansion.model),
            openings: Openings::of(&expansion.model),
        }
    }
}

/// What one candidate produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Built {
    /// The candidate, with every override resolved to a concrete value — so the
    /// report alone regenerates these bytes (ADR-0006).
    pub id: String,
    /// The seed actually used.
    pub seed: u64,
    /// The region actually used.
    pub region: [u32; 3],
    /// The parameter overrides actually applied.
    pub params: BTreeMap<String, i64>,
    /// The snapshot filename written for this candidate.
    pub snapshot: String,
    /// The semantics sidecar written beside it. Empty for a candidate that did
    /// not build — there is nothing to say about a building that does not exist.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub semantics: String,
    /// Cells that are not air.
    pub filled_cells: usize,
    /// Anchors this candidate declared.
    pub anchors: usize,
    /// Cells of it a player can stand in.
    pub standable_cells: usize,
    /// Standable cells lying on the boundary — every place a body could cross
    /// into or out of this building. Which one is the entrance is authored, and
    /// is `declared_entries` on the sidecar, not this number.
    pub boundary_openings: usize,
    /// `sha256:` over the model's placement-canonical bytes — block-for-block
    /// identity, **up to placement**.
    pub model_digest: String,
    /// `sha256:` over the solid/air bitmap alone, likewise up to placement —
    /// **massing** identity, which is what the owner is choosing between. Two
    /// candidates that share this are the same building in different paint.
    pub massing_digest: String,
    /// Why the expansion failed, when it did. A refused candidate stays in the
    /// report — a guard refusing is a fact about the program, and dropping the
    /// row would make the sweep look smaller rather than the zone look stricter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The refusal, structured, when a **guard** was what refused.
    ///
    /// `error` carries the same reading as prose. This carries it as data,
    /// because the sweep report is what `tools/zone-sheets.py` and any other
    /// driver read: a refusal reason a human can act on and a tool cannot is
    /// half a diagnostic. Absent when the candidate built, and when it failed
    /// for a reason that is not a guard declining (an export refusal, a write
    /// error) — those say so in `error` alone rather than being flattened into
    /// a shape they are not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<GuardRefusal>,
}

/// The whole sweep, as it goes on disk beside the snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SweepReport {
    /// Always [`SCHEMA`].
    pub schema: String,
    /// The program id.
    pub program: String,
    /// The program's name as the IR declares it.
    pub program_name: String,
    /// `sha256:` over the program's canonical JSON — pins the *rules* the
    /// candidates were expanded from, so a later rule edit cannot be mistaken
    /// for a curation the owner made.
    pub program_hash: String,
    /// Candidates asked for.
    pub candidates: usize,
    /// Candidates that expanded.
    pub built: usize,
    /// Candidates a guard refused.
    pub refused: usize,
    /// Distinct models among those that built — block-for-block, up to
    /// placement.
    pub distinct_models: usize,
    /// Distinct **massings** among those that built. **The number that says
    /// whether there is a choice on the page at all**: `1` means every cell is
    /// the same building, however many candidates were rendered.
    ///
    /// Counted up to placement, so a building and the same building turned
    /// ninety degrees are one. See the module note.
    pub distinct_massings: usize,
    /// Anchors declared across every candidate that built. **Zero is a
    /// finding**: the sheet drawn from this sweep can annotate nothing, and a
    /// page that annotated nothing must not read like a page that had nothing
    /// to annotate (CLAUDE.md: a green gate that binds to nothing is vacuous).
    pub anchors_declared: usize,
    /// Candidates that declared at least one anchor.
    pub rows_with_anchors: usize,
    /// Candidates whose program declared the party's way in ([`WAY_IN_NAMES`]).
    pub rows_with_entry: usize,
    /// Candidates whose program declared the party's way onward
    /// ([`WAY_OUT_NAMES`]).
    pub rows_with_exit: usize,
    /// Per candidate, in manifest order.
    pub rows: Vec<Built>,
}

impl SweepReport {
    /// The one-line verdict a driver prints and a sheet title carries.
    pub fn summary(&self) -> String {
        format!(
            "{}: {} candidate(s), {} built, {} refused, {} distinct model(s), {} distinct \
             massing(s), {} anchor(s) on {}/{} built candidate(s)",
            self.program,
            self.candidates,
            self.built,
            self.refused,
            self.distinct_models,
            self.distinct_massings,
            self.anchors_declared,
            self.rows_with_anchors,
            self.built
        )
    }

    /// True when nothing that built declared an anchor, so every artifact drawn
    /// from this sweep annotates zero objects.
    ///
    /// Named for the same reason [`SweepReport::massing_is_uniform`] is: it is a
    /// condition every consumer has to say out loud rather than a comparison
    /// each of them re-invents, and a page that silently annotated nothing is
    /// precisely how this gap survived — nothing ever said "0 anchors drawn".
    pub fn anchors_bind_to_nothing(&self) -> bool {
        self.built > 0 && self.anchors_declared == 0
    }

    /// True when no candidate's program declares where the party enters or
    /// leaves. A statement about the programs, never repaired by guessing at
    /// the geometry.
    pub fn ways_are_undeclared(&self) -> bool {
        self.built > 0 && self.rows_with_entry == 0 && self.rows_with_exit == 0
    }

    /// True when every candidate that built is the same building.
    ///
    /// Named rather than left to `== 1` at each call site, because this is the
    /// condition every consumer has to say out loud: a sweep that is vacuous in
    /// this sense is a finding, and a page built from it is evidence of the
    /// finding rather than a choice (CLAUDE.md: a green gate that binds to
    /// nothing is vacuous, not a pass).
    pub fn massing_is_uniform(&self) -> bool {
        self.built > 1 && self.distinct_massings <= 1
    }
}

fn digest(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    let mut out = String::with_capacity(71);
    out.push_str("sha256:");
    for b in d {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// Check a manifest without expanding anything.
pub fn validate(manifest: &Manifest) -> Result<Entry, SweepError> {
    if manifest.schema != SCHEMA {
        return Err(SweepError::BadSchema {
            got: manifest.schema.clone(),
        });
    }
    let entry = lookup(&manifest.program).ok_or_else(|| SweepError::UnknownProgram {
        id: manifest.program.clone(),
        known: registry().into_iter().map(|e| e.id).collect(),
    })?;
    if manifest.candidates.is_empty() {
        return Err(SweepError::NoCandidates);
    }
    let program = (entry.program)();
    let mut seen = BTreeSet::new();
    for c in &manifest.candidates {
        if !is_valid_id(&c.id) {
            return Err(SweepError::BadId { id: c.id.clone() });
        }
        if !seen.insert(c.id.clone()) {
            return Err(SweepError::DuplicateId { id: c.id.clone() });
        }
        for p in c.params.keys() {
            if !program.params.contains_key(p) {
                return Err(SweepError::UnknownParam {
                    candidate: c.id.clone(),
                    param: p.clone(),
                });
            }
        }
    }
    Ok(entry)
}

/// Expand every candidate and write its snapshot — and its semantics — into
/// `dir`.
///
/// `dir` must exist. Per candidate that built: `<id>.nbt` (the blocks) and
/// `<id>.json` (the semantics sidecar), flat — which is exactly what
/// `delve-render batch` consumes, sidecar included, so the sweep hands the
/// render layer both halves without an adapter in between.
pub fn run(manifest: &Manifest, dir: &Path) -> Result<SweepReport, SweepError> {
    let entry = validate(manifest)?;
    let program = (entry.program)();
    let hash = program_hash(&program);
    let base_region = manifest.region.unwrap_or(entry.region);

    let mut rows = Vec::with_capacity(manifest.candidates.len());
    let mut models = BTreeSet::new();
    let mut massings = BTreeSet::new();
    let mut built = 0usize;
    let mut refused = 0usize;
    let mut anchors_declared = 0usize;
    let mut rows_with_anchors = 0usize;
    let mut rows_with_entry = 0usize;
    let mut rows_with_exit = 0usize;

    for c in &manifest.candidates {
        let seed = c.seed.unwrap_or(manifest.seed);
        let region_size = c.region.unwrap_or(base_region);
        let region = Box3::at_origin(region_size);
        let mut prog = program.clone();
        for (k, v) in &c.params {
            prog.params.insert(k.clone(), *v);
        }
        let snapshot = format!("{}.nbt", c.id);
        let sidecar = format!("{}.json", c.id);

        let mut row = Built {
            id: c.id.clone(),
            seed,
            region: region_size,
            params: c.params.clone(),
            snapshot: snapshot.clone(),
            semantics: String::new(),
            filled_cells: 0,
            anchors: 0,
            standable_cells: 0,
            boundary_openings: 0,
            model_digest: String::new(),
            massing_digest: String::new(),
            error: None,
            refusal: None,
        };

        match expand(&prog, region, &ExpandOptions::seeded(seed)) {
            Ok(e) => {
                // Both digests are taken **up to placement** (see
                // [`VoxelModel::placement_canonical_bytes`]): a building and the
                // same building turned or mirrored are one row on the owner's
                // page, so they must be one identity here. The massing digest
                // is additionally blind to which block is where — that is the
                // difference between "a different building" and "the same
                // building in a different stone".
                row.filled_cells = e.model.filled_cells();
                row.model_digest = digest(&e.model.placement_canonical_bytes());
                row.massing_digest = digest(&e.model.placement_canonical_massing());
                models.insert(row.model_digest.clone());
                massings.insert(row.massing_digest.clone());

                // What the program knows about this building, read once and
                // written where the render layer looks for it. The counts ride
                // on the row so `sweep.json` alone answers "was there anything
                // to annotate", and the sidecar carries the positions.
                let semantics =
                    Semantics::of(&c.id, &program, &hash, seed, region_size, &c.params, &e);
                row.anchors = semantics.anchors.len();
                row.standable_cells = semantics.floor.standable_cells;
                row.boundary_openings = semantics.openings.total();

                match snapshot_nbt(&e.model) {
                    Ok(nbt) => {
                        if let Err(err) = std::fs::write(dir.join(&snapshot), &nbt) {
                            row.error = Some(format!("cannot write {snapshot}: {err}"));
                            refused += 1;
                        } else {
                            match serde_json::to_string_pretty(&semantics) {
                                Ok(mut text) => {
                                    text.push('\n');
                                    if let Err(err) = std::fs::write(dir.join(&sidecar), &text) {
                                        // The blocks without their meaning is
                                        // the defect this sidecar exists to
                                        // remove, so a snapshot whose semantics
                                        // could not be written is a refusal and
                                        // not a quieter success.
                                        row.error = Some(format!("cannot write {sidecar}: {err}"));
                                        refused += 1;
                                    } else {
                                        row.semantics = sidecar.clone();
                                        anchors_declared += row.anchors;
                                        rows_with_anchors += usize::from(row.anchors > 0);
                                        rows_with_entry +=
                                            usize::from(!semantics.declared_entries.is_empty());
                                        rows_with_exit +=
                                            usize::from(!semantics.declared_exits.is_empty());
                                        built += 1;
                                    }
                                }
                                Err(err) => {
                                    row.error = Some(format!("cannot serialise {sidecar}: {err}"));
                                    refused += 1;
                                }
                            }
                        }
                    }
                    Err(ExportError::ForbiddenBlocks { reasons }) => {
                        row.error = Some(format!("forbidden blocks: {}", reasons.join("; ")));
                        refused += 1;
                    }
                    Err(err) => {
                        row.error = Some(err.to_string());
                        refused += 1;
                    }
                }
            }
            Err(err) => {
                row.error = Some(err.to_string());
                if let ExpandError::NoApplicableRule { refusal } = err {
                    row.refusal = Some(refusal);
                }
                refused += 1;
            }
        }
        rows.push(row);
    }

    Ok(SweepReport {
        schema: SCHEMA.to_string(),
        program: manifest.program.clone(),
        program_name: program.name.clone(),
        program_hash: hash,
        candidates: manifest.candidates.len(),
        built,
        refused,
        distinct_models: models.len(),
        distinct_massings: massings.len(),
        anchors_declared,
        rows_with_anchors,
        rows_with_entry,
        rows_with_exit,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(program: &str, candidates: Vec<Candidate>) -> Manifest {
        Manifest {
            schema: SCHEMA.to_string(),
            program: program.to_string(),
            seed: 1,
            region: None,
            candidates,
        }
    }

    #[test]
    fn every_zone_of_the_registry_resolves_to_its_own_design_box() {
        let reg = registry();
        assert_eq!(reg.len(), bell::ZONES.len());
        assert!(!reg.is_empty(), "the registry bound to no programs");
        for e in &reg {
            assert!(e.region.iter().all(|&d| d > 0), "{} has an empty box", e.id);
            assert_eq!(lookup(&e.id).map(|f| f.region), Some(e.region));
        }
    }

    #[test]
    fn an_override_that_binds_to_nothing_is_refused() {
        let m = manifest(
            "bell:barrow-shore",
            vec![Candidate {
                id: "typo".into(),
                seed: None,
                region: None,
                params: BTreeMap::from([("arena/raidus".to_string(), 5i64)]),
            }],
        );
        assert_eq!(
            validate(&m).err(),
            Some(SweepError::UnknownParam {
                candidate: "typo".into(),
                param: "arena/raidus".into(),
            })
        );
    }

    #[test]
    fn a_sweep_with_no_candidates_is_refused() {
        assert_eq!(
            validate(&manifest("bell:barrow-shore", vec![])).err(),
            Some(SweepError::NoCandidates)
        );
    }

    #[test]
    fn two_candidates_may_not_share_a_name() {
        let m = manifest(
            "bell:barrow-shore",
            vec![Candidate::at_seed("a", 1), Candidate::at_seed("a", 2)],
        );
        assert_eq!(
            validate(&m).err(),
            Some(SweepError::DuplicateId { id: "a".into() })
        );
    }

    #[test]
    fn an_unknown_program_names_the_ones_that_exist() {
        let err = validate(&manifest(
            "bell:no-such-zone",
            vec![Candidate::at_seed("a", 1)],
        ))
        .err();
        let Some(SweepError::UnknownProgram { known, .. }) = err else {
            panic!("expected UnknownProgram, got {err:?}");
        };
        assert!(known.iter().any(|k| k == "bell:gate-ward"), "{known:?}");
    }

    #[test]
    fn the_manifest_round_trips_through_json() {
        let m = Manifest::over_seeds("bell:cliff-road", &[1, 2, 3]);
        let text = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&text).unwrap(), m);
    }

    #[test]
    fn a_seed_sweep_of_a_zone_that_ignores_the_seed_reports_one_massing() {
        // The gatehouse's own fixture note says nothing in it draws from the
        // seed. This is that claim, measured rather than believed — and it is
        // the shape a driver must be able to see, because a page built from
        // this sweep would be six pictures of one building.
        let dir = std::env::temp_dir().join("dw-sweep-uniform");
        std::fs::create_dir_all(&dir).unwrap();
        let report = run(
            &Manifest::over_seeds("bell:gate-ward", &[1, 2, 3, 4, 5, 6]),
            &dir,
        )
        .unwrap();
        assert_eq!(report.built, 6);
        assert_eq!(report.distinct_massings, 1, "{}", report.summary());
        assert!(report.massing_is_uniform());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_parameter_and_region_sweep_of_the_same_zone_moves_the_massing() {
        // The counterpart of the test above, and the whole reason a candidate is
        // a *variation* rather than a seed: on the very same program whose seed
        // moved nothing, the other two axes each move the building.
        //
        // Binding: 4 candidates, all four expanded, 4 distinct massings.
        let dir = std::env::temp_dir().join("dw-sweep-params");
        std::fs::create_dir_all(&dir).unwrap();
        let candidates = vec![
            Candidate {
                id: "base".into(),
                seed: None,
                region: None,
                params: BTreeMap::new(),
            },
            Candidate {
                id: "bay-taller".into(),
                seed: None,
                region: None,
                params: BTreeMap::from([("gate/bay_height".to_string(), 3i64)]),
            },
            Candidate {
                id: "pockets-often".into(),
                seed: None,
                region: None,
                params: BTreeMap::from([("stair/pocket_period".to_string(), 6i64)]),
            },
            Candidate {
                id: "box-longer".into(),
                seed: None,
                region: Some([20, 10, 92]),
                params: BTreeMap::new(),
            },
        ];
        let report = run(&manifest("bell:gate-ward", candidates), &dir).unwrap();
        assert_eq!(report.built, 4, "{}", report.summary());
        assert_eq!(report.distinct_massings, 4, "{}", report.summary());
        assert!(!report.massing_is_uniform());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The motivating case, both directions on one zone.
    ///
    /// `bell:barrow-shore` at `[19,6,24]` and at its transpose `[24,6,19]` is
    /// ONE building: the zone's frame (`z(Largest)`) normalises the box onto its
    /// longer horizontal axis, so the transposed region expands to the same
    /// arena written on swapped world axes. Before this metric counted up to
    /// placement it reported 2, the sheet held 1, and `--require-choice` passed.
    ///
    /// The second half is the guard against a fix that is merely coarser: a
    /// third candidate that really is a different building — a wider arena in a
    /// deeper box — must still count.
    ///
    /// Binding: 3 candidates, 3 built, 2 distinct massings.
    #[test]
    fn a_transposed_region_is_one_building_and_a_reshaped_one_is_two() {
        let dir = std::env::temp_dir().join("dw-sweep-transpose");
        std::fs::create_dir_all(&dir).unwrap();
        let m = manifest(
            "bell:barrow-shore",
            vec![
                Candidate {
                    id: "as-designed".into(),
                    seed: None,
                    region: Some([19, 6, 24]),
                    params: BTreeMap::new(),
                },
                Candidate {
                    id: "transposed".into(),
                    seed: None,
                    region: Some([24, 6, 19]),
                    params: BTreeMap::new(),
                },
                Candidate {
                    id: "taller-head".into(),
                    seed: None,
                    region: Some([19, 6, 24]),
                    params: BTreeMap::from([("arena/head".to_string(), 4i64)]),
                },
            ],
        );
        let report = run(&m, &dir).unwrap();
        assert_eq!(report.built, 3, "{}", report.summary());
        assert_eq!(
            report.rows[0].massing_digest,
            report.rows[1].massing_digest,
            "a box and its transpose are one building: {}",
            report.summary()
        );
        assert_ne!(
            report.rows[0].massing_digest, report.rows[2].massing_digest,
            "a wider arena in a deeper box is a different building, and a metric \
             that merged it would hide a choice instead of inventing one"
        );
        assert_eq!(report.distinct_massings, 2, "{}", report.summary());
        // ...and the sibling count moves with it. A page whose massings collapse
        // and whose model count does not is the same inflation one layer over.
        assert_eq!(report.distinct_models, 2, "{}", report.summary());
        assert!(!report.massing_is_uniform());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The symmetry the frame introduces is a **reflection**, so the metric's
    /// group has to contain reflections — measured over every zone rather than
    /// argued, because "which symmetries" is the whole design decision and the
    /// motivating zone is the one that would not have settled it.
    ///
    /// `z(Largest)` swaps the two horizontal axes and reverses neither, which is
    /// an odd permutation. On `bell:barrow-shore` that is invisible: an open
    /// circular arena is mirror-symmetric, so its transpose is *also* reachable
    /// by a plain 90° turn and a rotations-only metric would have merged it by
    /// luck. On the other seven zones it is not: their transpose is reachable by
    /// no rotation at all, so a metric blind only to the four yaw rotations
    /// would have gone on reporting two buildings where the page holds one.
    ///
    /// Binding: 8 zones, every one of them non-square and expanding in both its
    /// design box and the transpose; at least one reflection-only.
    #[test]
    fn every_zones_design_box_and_its_transpose_are_one_building() {
        use crate::geom::Box3;
        let opts = ExpandOptions::seeded(1);
        let mut checked = 0usize;
        let mut reflection_only = Vec::new();
        for e in registry() {
            let d = e.region;
            assert_ne!(
                d[0], d[2],
                "{}: a square box has no transpose to test",
                e.id
            );
            let program = (e.program)();
            let one = expand(&program, Box3::at_origin(d), &opts)
                .unwrap_or_else(|err| panic!("{}: design box refused: {err}", e.id));
            let two = expand(&program, Box3::at_origin([d[2], d[1], d[0]]), &opts)
                .unwrap_or_else(|err| panic!("{}: transposed box refused: {err}", e.id));
            let onto = one.model.placements_onto(&two.model);
            assert!(
                !onto.is_empty(),
                "{}: transposing the design box built a different building — either the \
                 frame stopped normalising onto the long axis, or this zone reads the \
                 world axes directly",
                e.id
            );
            assert_eq!(
                digest(&one.model.placement_canonical_massing()),
                digest(&two.model.placement_canonical_massing()),
                "{}: the placements agree and the digests do not",
                e.id
            );
            if !onto.iter().any(|(rotation, _)| *rotation) {
                reflection_only.push(e.id.clone());
            }
            checked += 1;
        }
        assert_eq!(checked, bell::ZONES.len(), "some zone went unmeasured");
        // Measured 2026-08-11: 7 of the 8. Narrowing the group to the four
        // rotations turns each of those into a digest mismatch above, which is
        // what makes this an argument rather than an assertion.
        assert!(
            !reflection_only.is_empty(),
            "no zone's transpose needs a reflection any more, so the mirror half of the \
             group is bound to nothing measured — re-derive it before trusting it"
        );
    }

    #[test]
    fn a_refused_candidate_keeps_its_row() {
        let dir = std::env::temp_dir().join("dw-sweep-refused");
        std::fs::create_dir_all(&dir).unwrap();
        let m = Manifest {
            schema: SCHEMA.to_string(),
            program: "bell:barrow-shore".into(),
            seed: 1,
            region: None,
            // A square box is the one shape the shore's frame guard refuses.
            candidates: vec![
                Candidate {
                    id: "square".into(),
                    seed: None,
                    region: Some([19, 6, 19]),
                    params: BTreeMap::new(),
                },
                Candidate::at_seed("ok", 1),
            ],
        };
        let report = run(&m, &dir).unwrap();
        assert_eq!(report.rows.len(), 2, "a refused candidate was dropped");
        assert_eq!(report.refused, 1);
        assert_eq!(report.built, 1);
        assert!(report.rows[0].error.is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A refusal reason a human can act on and a tool cannot is half a fix.
    /// `tools/zone-sheets.py` reads this file, so the reading a refused row
    /// carries has to be data — the whole guard, both sides of the clause that
    /// decided it, and the distance — not only the prose in `error`.
    ///
    /// Binding: 2 candidates, 1 refused by a guard, 1 built; the refused row
    /// carries a structured refusal and the built row carries none.
    #[test]
    fn a_refused_row_carries_its_reading_as_data_not_only_as_prose() {
        let dir = std::env::temp_dir().join("dw-sweep-refusal-json");
        std::fs::create_dir_all(&dir).unwrap();
        let m = manifest(
            "bell:chapel-ward",
            vec![
                // The real case: a rest ward long enough to starve the chute.
                Candidate {
                    id: "great-hearth".into(),
                    seed: None,
                    region: None,
                    params: BTreeMap::from([("hearth_run".to_string(), 14i64)]),
                },
                Candidate {
                    id: "as-designed".into(),
                    seed: None,
                    region: None,
                    params: BTreeMap::new(),
                },
            ],
        );
        let report = run(&m, &dir).unwrap();
        assert_eq!(
            (report.refused, report.built),
            (1, 1),
            "{}",
            report.summary()
        );
        assert!(
            report.rows[1].refusal.is_none(),
            "a candidate that built has nothing to explain"
        );

        let refusal = report.rows[0]
            .refusal
            .as_ref()
            .expect("a guard refusal reaches the report as data");
        assert_eq!(refusal.symbol, "ward_plan");
        assert_eq!(refusal.scope.size, [16, 9, 26]);

        // ...and survives the round trip a driver actually makes.
        let text = serde_json::to_string(&report).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let clause = &json["rows"][0]["refusal"]["alternatives"][0]["guard"]["of"][3];
        assert_eq!(clause["holds"], false);
        assert_eq!(
            clause["lhs"]["source"],
            "Dimension.Z - junction_run - hearth_run"
        );
        assert_eq!(clause["lhs"]["value"], 4);
        assert_eq!(clause["rhs"]["value"], 7);
        assert_eq!(clause["shortfall"]["blocks"], 4);
        assert_eq!(clause["shortfall"]["lhs_must_reach"], 8);
        assert!(
            json["rows"][0]["error"]
                .as_str()
                .unwrap()
                .contains("4 short"),
            "the prose reading stays on `error` as well"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_same_manifest_twice_writes_the_same_bytes() {
        let a = std::env::temp_dir().join("dw-sweep-det-a");
        let b = std::env::temp_dir().join("dw-sweep-det-b");
        for d in [&a, &b] {
            std::fs::create_dir_all(d).unwrap();
        }
        let m = Manifest::over_seeds("bell:cliff-road", &[1, 2, 3]);
        let ra = run(&m, &a).unwrap();
        let rb = run(&m, &b).unwrap();
        assert_eq!(ra, rb);
        for row in &ra.rows {
            // Both halves: the blocks and their meaning. A sidecar that drifted
            // between two runs of one manifest would be a reviewer's page
            // changing under a decision that had already been made.
            for file in [&row.snapshot, &row.semantics] {
                assert_eq!(
                    std::fs::read(a.join(file)).unwrap(),
                    std::fs::read(b.join(file)).unwrap(),
                    "{file} differs between two runs of one manifest"
                );
            }
        }
        for d in [&a, &b] {
            std::fs::remove_dir_all(d).ok();
        }
    }

    /// The whole point, end to end: the anchors the rules declared and the floor
    /// derived from the model reach a file beside the snapshot, in the shape
    /// `delve-render` reads prefab metadata from — because a field the pictures
    /// do not consume is the unemitted vacuity, not a fix.
    ///
    /// Binding: 1 candidate, ≥1 anchor, a floor with cells in it, at least one
    /// boundary opening, and the report's counts equal to the sidecar's.
    #[test]
    fn a_built_candidate_carries_its_semantics_beside_its_blocks() {
        let dir = std::env::temp_dir().join("dw-sweep-semantics");
        std::fs::create_dir_all(&dir).unwrap();
        let report = run(&Manifest::over_seeds("bell:gate-ward", &[1]), &dir).unwrap();
        assert_eq!(report.built, 1, "{}", report.summary());
        let row = &report.rows[0];
        assert_eq!(row.semantics, "seed-001.json");

        let text = std::fs::read_to_string(dir.join(&row.semantics)).unwrap();
        let s: Semantics = serde_json::from_str(&text).unwrap();
        assert_eq!(s.schema, SEMANTICS_SCHEMA);
        assert_eq!(s.id, row.id);
        assert_eq!(s.seed, row.seed);
        assert_eq!(s.program_hash, report.program_hash);

        // Declared: the marks, with the rule that placed each one.
        assert!(!s.anchors.is_empty(), "the gatehouse declares marks");
        assert_eq!(s.anchors.len(), row.anchors);
        assert_eq!(report.anchors_declared, row.anchors);
        assert_eq!(report.rows_with_anchors, 1);
        for (name, a) in &s.anchors {
            assert!(!a.declared_by.is_empty(), "{name} has no declaring rule");
        }

        // Derived: the floor, and where it meets the boundary.
        assert_eq!(s.floor.standable_cells, row.standable_cells);
        assert!(s.floor.standable_cells > 0);
        assert_eq!(
            s.floor.columns.len(),
            (s.region[0] * s.region[2]) as usize,
            "the plan is not one entry per column"
        );
        assert_eq!(s.openings.total(), row.boundary_openings);
        assert!(s.openings.total() > 0, "no body could enter this zone");

        // Authored, and absent: nothing here nominates a doorway.
        assert!(s.declared_entries.is_empty() && s.declared_exits.is_empty());
        assert!(report.ways_are_undeclared());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// ...and the same reading over the whole registry, so "the sweep annotates
    /// something" is a measured fact about every zone rather than about the one
    /// a test happened to name.
    ///
    /// Binding: 8 zones, every one declaring at least one anchor and reaching
    /// its own boundary; **0 of 8 declaring an entry or an exit**, which is the
    /// finding this test pins rather than papers over. When a rule finally
    /// declares one, this assertion goes red and is the place to record it.
    #[test]
    fn every_zone_declares_anchors_and_none_declares_a_way_in() {
        let dir = std::env::temp_dir().join("dw-sweep-registry-semantics");
        std::fs::create_dir_all(&dir).unwrap();
        let mut checked = 0usize;
        for e in registry() {
            let report = run(&Manifest::over_seeds(&e.id, &[1]), &dir).unwrap();
            assert_eq!(report.built, 1, "{}", report.summary());
            assert!(
                report.anchors_declared > 0,
                "{}: declares no anchor, so every picture of it annotates nothing",
                e.id
            );
            assert!(!report.anchors_bind_to_nothing(), "{}", e.id);
            assert!(
                report.rows[0].boundary_openings > 0,
                "{}: its floor never reaches the edge of its box",
                e.id
            );
            assert_eq!(
                (report.rows_with_entry, report.rows_with_exit),
                (0, 0),
                "{}: a zone now declares a way in or out — record it, and give the plan key \
                 something better to draw than every boundary cell",
                e.id
            );
            checked += 1;
        }
        assert_eq!(checked, bell::ZONES.len(), "a zone went unswept");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A program that marks nothing says so, loudly, instead of producing a page
    /// that looks annotated. The vacuous-green case, kept exercisable.
    #[test]
    fn a_program_with_no_marks_reports_a_binding_of_zero() {
        use crate::block::BlockState;
        use crate::ir::{Material, Node};
        let program = Program::new("bare", "all").rule(
            "all",
            Node::Fill {
                material: Material::block(BlockState::simple("stone")),
            },
        );
        let expansion = expand(
            &program,
            Box3::at_origin([4, 4, 4]),
            &ExpandOptions::seeded(1),
        )
        .unwrap();
        let s = Semantics::of(
            "bare",
            &program,
            "sha256:0",
            1,
            [4, 4, 4],
            &BTreeMap::new(),
            &expansion,
        );
        assert!(s.anchors.is_empty());
        assert_eq!(s.floor.standable_cells, 0, "a solid block has no floor");
        assert!(s.declared_entries.is_empty());
    }

    #[test]
    fn a_reserved_way_name_is_recognised_however_it_is_pathed() {
        assert_eq!(stem_of("anchor/entry"), "entry");
        assert_eq!(stem_of("entry"), "entry");
        assert_eq!(stem_of("anchor/gate-north"), "gate-north");
        assert!(WAY_IN_NAMES.contains(&stem_of("anchor/spawn")));
        assert!(WAY_OUT_NAMES.contains(&stem_of("exit")));
        assert!(!WAY_IN_NAMES.contains(&stem_of("anchor/entry-hall")));
    }

    #[test]
    fn massing_ignores_paint_but_model_does_not() {
        // hall-keep's storeroom tell IS a seeded draw, and it moves blocks
        // without moving a wall — the exact case the two digests separate.
        let dir = std::env::temp_dir().join("dw-sweep-paint");
        std::fs::create_dir_all(&dir).unwrap();
        let report = run(
            &Manifest::over_seeds("bell:hall-keep", &[1, 2, 3, 4, 5, 6, 7, 8]),
            &dir,
        )
        .unwrap();
        assert!(
            report.distinct_models > 1,
            "the storeroom tell stopped drawing from the seed: {}",
            report.summary()
        );
        assert_eq!(
            report.distinct_massings,
            1,
            "the tell moved a wall, not a block: {}",
            report.summary()
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
