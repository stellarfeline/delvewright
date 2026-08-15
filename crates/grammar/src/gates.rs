//! **How an expansion is judged before a human looks at it** (spec-0027 §3's
//! "machine gates filter", the step §6 records as not built).
//!
//! # A gate and a measurement are different things, and this module keeps them apart
//!
//! A *gate* has a verdict: it can be red, and the condition that reddens it is
//! stated. A *measurement* is a number with no threshold. The distinction is the
//! whole design here, because the failure this project keeps hitting is a green
//! that binds to nothing (CLAUDE.md): a number printed beside the word "pass" is
//! not a gate, and calling it one is worse than printing nothing, since the next
//! reader believes something was checked.
//!
//! So: [`Gate`]s carry a verdict **and a binding count** — how many objects the
//! gate examined. A zero binding is reported as a finding in its own right, not
//! folded into a pass. [`Measurements`] carry numbers and no verdicts, and are
//! deliberately not dressed up as gates.
//!
//! # Why the craft gates of spec-0027 §4 are not here
//!
//! §4 asks for a palette-role budget "computed per **material family**". Nothing
//! in this repo can decide what family a block belongs to: the two places that
//! need it (`tests/staging.rs`'s boulder-stair and broken-grate mirrors) each
//! hand-write the family map for the blocks that test uses. A diagnostic cannot
//! take a hand-written map — it would only ever be as complete as the fixture
//! that wrote it, which is the vacuity mode this module exists to avoid. The
//! honest state is therefore: **the family-grouped palette budget is not built,
//! and what blocks it is a missing derivation, not missing effort.**

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::contract;
use crate::expand::Expansion;
use crate::export;
use crate::geom::Axis;
use crate::model::VoxelModel;
use crate::nav;
use crate::settle;

/// **What a gate can answer.** Three things, not two.
///
/// A binary gate has to fold "I examined this and it held" together with "this
/// expansion could not examine it", and it folds them into `pass` — which is the
/// vacuity CLAUDE.md names, one level below a zero binding: the gate bound to
/// plenty of objects and its *predicate* bound to none of them. `oriented-fills`
/// is the worked example. Its mismatch test short-circuits on the identity
/// frame, so a program whose reorientation happens to resolve to the identity at
/// its declared region gets a green from a test that read nothing, and the same
/// program at a region whose axes rank differently is refused outright.
///
/// [`Undecided`](GateState::Undecided) is the third answer. It is not a weaker
/// fail and not a stricter pass: it says the expansion examined the objects and
/// **this region could not decide them**, which is a fact about the region and
/// so cannot be repaired by editing the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GateState {
    /// The gate examined its objects and they held.
    Pass,
    /// The gate examined its objects and they did not.
    Fail,
    /// The gate could not decide at this expansion. Never a fail: the program
    /// may be entirely correct, and nothing it could do here would make this
    /// region decide it.
    Undecided,
}

/// One gate's verdict over one expansion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gate {
    /// Stable id, e.g. `blocks-exist`.
    pub id: &'static str,
    /// `pass`, `fail`, or `undecided`.
    pub state: GateState,
    /// **How many objects the gate examined.** A gate that examined zero
    /// objects is not a pass; `Report::findings` says so by name.
    pub bound: usize,
    /// Of `bound`, how many this expansion could not decide — the binding count
    /// of the third answer, reported the way every other binding count is.
    /// Zero on a gate with nothing undecided, which is every gate but
    /// `oriented-fills` today.
    pub undecided: usize,
    /// What the gate found, in one line.
    pub detail: String,
}

impl Gate {
    /// A green verdict — examined, and held. **Not** the complement of
    /// [`Self::failed`]: an undecided gate is neither.
    pub fn passed(&self) -> bool {
        self.state == GateState::Pass
    }

    /// A red verdict — examined, and did not hold. This is what refuses an
    /// artifact; an undecided gate never does.
    pub fn failed(&self) -> bool {
        self.state == GateState::Fail
    }
}

/// A two-valued gate's verdict. Most gates have only two answers, and saying so
/// at the construction site is what keeps [`GateState::Undecided`] a thing a
/// gate has to reach for deliberately rather than a third value every gate now
/// has to think about.
pub(crate) fn verdict(pass: bool) -> GateState {
    if pass {
        GateState::Pass
    } else {
        GateState::Fail
    }
}

/// Numbers with no threshold. Not gates, and deliberately not presented as any.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measurements {
    /// Cells the expansion wrote something other than air into.
    pub filled_cells: usize,
    /// Cells in the region.
    pub region_cells: usize,
    /// Distinct block states in the model's palette (air included).
    pub distinct_states: usize,
    /// Cells a body could stand in.
    pub standable_cells: usize,
    /// Columns of the footprint that carry any block.
    pub footprint_area: usize,
    /// Edges of the footprint that face empty ground — the silhouette's length.
    ///
    /// spec-0027 §4 wants a "silhouette/perimeter complexity floor", calling it
    /// the one metric that tracked quality in the sandbox probe. The probe's
    /// threshold was never written down, so this is reported and **not** gated:
    /// inventing a number here would be a fabricated gate.
    pub footprint_perimeter: usize,
    /// `footprint_perimeter` over the perimeter of a solid rectangle of the same
    /// area — 1.0 for a plain box, higher the more articulated the plan.
    pub silhouette_complexity: f64,
    /// The five commonest non-air block states, with their share of filled cells.
    pub top_blocks: Vec<(String, f64)>,
    /// **Stairs in the piece.** A number, not a verdict: a piece with no stair
    /// is not a piece that passed the stair rule, and the `stair-shape` gate
    /// is emitted only when there is a stair to judge (see [`judge`]).
    pub stairs: usize,
    /// **Fluid cells in the piece** — `water`/`lava` blocks, the fluid that
    /// runs. The `fluid-contained` gate is emitted only when this is non-zero.
    pub fluid_cells: usize,
    /// Cells written `waterlogged=true`: wet, measured not to spread, and so
    /// under no containment obligation of their own.
    pub fluid_held_cells: usize,
    /// Run directions in which a body of fluid leaves the piece's own outer
    /// face, where these bytes decide nothing. Counted, never judged.
    pub fluid_at_edge: usize,
    /// Fills whose block states were written in the scope's own axis names and
    /// resolved into the world's — **the binding count of the local frame**.
    ///
    /// A number, not a verdict: a piece that needs no oriented block writes
    /// zero of them and is not thereby worse. It is reported because the
    /// `oriented-fills` gate's population shrinks by exactly this much, and a
    /// gate whose binding falls has to be able to say where it went.
    pub local_frame_fills: usize,
    /// How much of the floor a body can actually get to, and where the rest is.
    pub reachability: Reachability,
}

/// One pocket of standable floor with no walking route to the rest of the piece.
///
/// It carries its bounding box because a count is not actionable: "42% of the
/// floor is reachable" tells an author nothing about which tower is stranded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pocket {
    /// Standable cells in the pocket.
    pub cells: usize,
    /// How many of them have something solid overhead — floor under a roof that
    /// nobody can walk to. The other cells are open to the sky.
    pub sheltered: usize,
    /// Minimum corner of the pocket's bounding box.
    pub min: [i32; 3],
    /// Maximum corner of the pocket's bounding box, **inclusive**.
    pub max: [i32; 3],
}

impl Pocket {
    /// `48 cells (48 sheltered) x 4..26 y 14..21 z 8..84` — where to go and look.
    pub fn describe(&self) -> String {
        format!(
            "{} cell(s), {} sheltered — x {}..{} y {}..{} z {}..{}",
            self.cells,
            self.sheltered,
            self.min[0],
            self.max[0],
            self.min[1],
            self.max[1],
            self.min[2],
            self.max[2],
        )
    }
}

/// **What fraction of the standable floor a body can actually reach on foot from
/// the entrance, and what it cannot.**
///
/// # Why this is a measurement and not a gate
///
/// [`Options::traversable`] proves one thing: a walk connects the approach face
/// to the exit face. Both faces are at ground level, so a piece passes it while
/// every storey above the floor is stranded decoration — measured on the two
/// Notre-Dame trial artifacts at 42% and 46% of standable floor reachable, with
/// **zero** reachable above the ground band in either. The gate is not wrong; it
/// answers a narrower question than a reader takes it for.
///
/// The obvious repair — gate on it — is a gate that reds on almost every piece
/// in the library, because a roof is standable and nobody walks it, a rafter is
/// standable and is meant to be looked at, and a sealed crypt is floor on
/// purpose. **The engine cannot tell "unreachable" from "not meant to be
/// reached"**, and a gate that cannot make that distinction is one an author
/// learns to pass rather than read.
///
/// So the split is: the numbers are always computed and always printed, the one
/// distinction the engine *can* draw is drawn ([`nav::sheltered`] — is there a
/// roof over it), unreachable floor **under a roof** is raised as a finding
/// because that is a room nobody can enter, unreachable floor **open to the
/// sky** is reported as a number and never nagged about, and an author who
/// wants the strong claim asks for it with [`Options::reachable_floor`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reachability {
    /// Standable cells examined — **the binding count of the measurement**.
    pub standable: usize,
    /// Of those, how many have something solid overhead: floor somebody was
    /// meant to stand on, as opposed to a roof or a parapet. The binding set of
    /// the opt-in [`Options::reachable_floor`] gate.
    pub sheltered: usize,
    /// Cells the walk started from: standable, on a side face, at grade
    /// ([`nav::ground_entry`]). Zero is a finding, not a reachability of zero.
    pub entry_cells: usize,
    /// Standable cells a body can walk to from `entry_cells`.
    pub reachable: usize,
    /// `reachable / standable`; `0.0` when nothing is standable.
    pub reachable_share: f64,
    /// Unreachable cells with something solid overhead: floor in a space a body
    /// cannot enter.
    pub unreachable_sheltered: usize,
    /// Unreachable cells open to the sky — a roof, a parapet, a terrace, a cliff
    /// top. The engine cannot tell which, and never gates on them.
    pub unreachable_open: usize,
    /// How many disconnected pockets the unreachable floor forms.
    pub pockets: usize,
    /// The pockets worth walking to, at most five: **most sheltered cells
    /// first**, then largest, then by position so the order is total (ADR-0006).
    ///
    /// Ranked that way and not by size alone because size alone answers the
    /// wrong question on a building. Notre-Dame run 1 strands 2715 cells; the
    /// five biggest are all aisle-roof and tower-deck plates, and the pockets an
    /// author can do something about — rooms with a roof and no door — are
    /// nowhere near the top. Where nothing is sheltered the ranking degenerates
    /// to size, which is the right answer for a piece stranded out of doors.
    pub largest_pockets: Vec<Pocket>,
}

/// How many pockets the report names before it stops. Five is enough to send an
/// author somewhere; the totals above it are exact however many there are.
const POCKETS_REPORTED: usize = 5;

/// Walk the model from its grade entrance and count what that reaches.
fn reachability(model: &VoxelModel, standable: &BTreeSet<[i32; 3]>) -> Reachability {
    let entry = nav::ground_entry(model);
    let sheltered_total = standable
        .iter()
        .filter(|&&c| nav::sheltered(model, c))
        .count();
    let components = nav::components(standable);
    let (reached, stranded): (Vec<_>, Vec<_>) = components
        .into_iter()
        .partition(|c| c.iter().any(|cell| entry.contains(cell)));
    let reachable: usize = reached.iter().map(|c| c.len()).sum();

    let mut unreachable_sheltered = 0usize;
    let mut unreachable_open = 0usize;
    let mut pockets: Vec<Pocket> = Vec::with_capacity(stranded.len());
    for component in &stranded {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        let mut sheltered = 0usize;
        for &cell in component {
            for axis in 0..3 {
                min[axis] = min[axis].min(cell[axis]);
                max[axis] = max[axis].max(cell[axis]);
            }
            if nav::sheltered(model, cell) {
                sheltered += 1;
            }
        }
        unreachable_sheltered += sheltered;
        unreachable_open += component.len() - sheltered;
        pockets.push(Pocket {
            cells: component.len(),
            sheltered,
            min,
            max,
        });
    }
    pockets.sort_by(|a, b| {
        b.sheltered
            .cmp(&a.sheltered)
            .then_with(|| b.cells.cmp(&a.cells))
            .then_with(|| a.min.cmp(&b.min))
    });
    pockets.truncate(POCKETS_REPORTED);

    Reachability {
        standable: standable.len(),
        sheltered: sheltered_total,
        entry_cells: entry.len(),
        reachable,
        reachable_share: if standable.is_empty() {
            0.0
        } else {
            reachable as f64 / standable.len() as f64
        },
        unreachable_sheltered,
        unreachable_open,
        pockets: stranded.len(),
        largest_pockets: pockets,
    }
}

/// The whole verdict over one expansion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// `"fail"` when any gate went red; else `"undecided"` when any gate could
    /// not decide; else `"pass"`.
    ///
    /// Three values, in that precedence, because they answer different
    /// questions and only the first refuses an artifact. `"undecided"` says the
    /// building is sound as far as anything here could establish, and that one
    /// gate's predicate never got to read what it was pointed at — see
    /// [`GateState`]. It is deliberately NOT a red: a program with a
    /// region-dependent reorientation and a world-frame literal may be entirely
    /// correct, and a gate that reddens on it is one an author routes around
    /// within a week.
    pub verdict: &'static str,
    /// Every gate, in a fixed order.
    pub gates: Vec<Gate>,
    /// Numbers, no verdicts.
    pub measurements: Measurements,
    /// Anchors the program declared, by exported name.
    pub anchors: BTreeMap<String, [i32; 3]>,
    /// Things a reader must be told even though no gate went red — a gate that
    /// examined nothing, an expansion that declared no anchors.
    pub findings: Vec<String>,
    /// **Every spatial-contract opt-out instance, by name** (spec-0036 §2.9):
    /// each open envelope, each sightline, each out-of-walk region with its
    /// computed kind, each bar the reachability walk had to open, each exterior
    /// face. Enumerated rather than counted, because a count is a thing a blind
    /// script can satisfy and a list is a thing a reviewer reads.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enumeration: Vec<String>,
}

impl Report {
    /// True when every gate held. Findings do not fail a report; they are
    /// carried so they cannot be lost.
    ///
    /// **Not** the complement of [`Self::is_fail`]. An undecided report is
    /// neither, and the two are asked for different things: `is_fail` is what
    /// refuses to write an artifact, `is_pass` is what a test asserts when it
    /// means "and nothing was left unexamined".
    pub fn is_pass(&self) -> bool {
        self.verdict == "pass"
    }

    /// True when some gate went red. The artifact-refusing question.
    pub fn is_fail(&self) -> bool {
        self.verdict == "fail"
    }

    /// True when no gate went red and at least one could not decide.
    pub fn is_undecided(&self) -> bool {
        self.verdict == "undecided"
    }

    /// Canonical pretty JSON with a trailing newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("a gate report serialises");
        s.push('\n');
        s
    }
}

/// Which optional gates to run beyond the always-on ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Assert the piece can be walked from its approach end to its exit end.
    ///
    /// Opt-in because it is a claim about a *kind* of piece: a room with one
    /// door has no far end to reach and would fail a traversability gate
    /// correctly and uselessly. The author says which claim the piece makes.
    pub traversable: bool,
    /// Allow a fall edge when walking (a piece entered by stepping off a ledge).
    pub allow_falls: bool,
    /// Assert the piece is bilaterally symmetric about the mid-plane of this
    /// **world** axis.
    ///
    /// Opt-in, and for the reason `traversable` is: it is a claim about a *kind*
    /// of piece, and only the author knows whether this one makes it. What makes
    /// it worth having is that the claim is otherwise unenforceable by anything.
    /// A shape with a mirror plane is normally built by expanding one rule at
    /// both sites — and if the two sites are instead two hand-kept copies, or
    /// one site is missing its reflection, every other gate stays green while
    /// the building has a hole in one flank. This is the gate that reads it.
    pub symmetric: Option<Axis>,
    /// Assert that **every sheltered standable cell** — every piece of floor
    /// with a roof over it — can be walked to from the grade entrance.
    ///
    /// Opt-in for the same reason `traversable` is, and the reason is sharper
    /// here: a piece whose upper floor is decoration seen from below is a
    /// legitimate piece, and so is a rafter, and so is a sealed crypt. The
    /// author says whether this one claims a body can get everywhere indoors.
    /// The *measurement* runs either way and is printed either way
    /// ([`Reachability`]) — this flag only decides whether it can go red.
    pub reachable_floor: bool,
}

/// Judge one expansion.
pub fn judge(expansion: &Expansion, options: Options) -> Report {
    let model = &expansion.model;
    let mut gates = Vec::new();
    let mut findings = Vec::new();

    // --- Gate: every block state exists in the pinned version. -------------
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
    let mut bad = Vec::new();
    for state in model.palette() {
        if let Err(e) = registry.validate(&state.name, &state.properties) {
            bad.push(e.to_string());
        }
    }
    gates.push(Gate {
        id: "blocks-exist",
        state: verdict(bad.is_empty()),
        undecided: 0,
        bound: model.palette().len(),
        detail: if bad.is_empty() {
            format!(
                "{} block state(s), all present in Minecraft {}",
                model.palette().len(),
                delvewright_schem::blocks::MC_VERSION
            )
        } else {
            bad.join("; ")
        },
    });

    // --- Gate: every placed state writes its shape-carrying properties. -----
    //
    // A `multipart` property the state omits removes assembled geometry — a
    // wall with none written places as an isolated post — and no downstream
    // reader can tell the omission from a choice (`DW0735`). Judged over the
    // states CELLS actually use: an entry an earlier fill created and a later
    // fill fully overwrote ships in no cell and is not this gate's business.
    let mut used: std::collections::BTreeSet<&crate::block::BlockState> = Default::default();
    for pos in model.region().positions() {
        if let Some(state) = model.get(pos) {
            used.insert(state);
        }
    }
    let omissions: Vec<String> = used
        .iter()
        .filter_map(|state| {
            let omitted = registry.omitted_shape_carrying(&state.name, &state.properties);
            if omitted.is_empty() {
                None
            } else {
                Some(format!("{state} omits {}", omitted.join(", ")))
            }
        })
        .collect();
    gates.push(Gate {
        id: "shape-complete",
        state: verdict(omissions.is_empty()),
        undecided: 0,
        bound: used.len(),
        detail: if omissions.is_empty() {
            format!(
                "{} placed block state(s), every shape-carrying (multipart) property written",
                used.len()
            )
        } else {
            format!(
                "{}: {} — these properties assemble the block's model, so the omitted \
                 default drops geometry (a wall reads as an isolated post). Write the \
                 connection state the design means",
                delvewright_schem::blocks::DW_SHAPE_OMITTED,
                omissions.join("; ")
            )
        },
    });

    // --- Gate: every placed state writes EVERY property it has. -------------
    //
    // The whole class `shape-complete` is the hard half of (`DW0737`). Vanilla
    // fills an omitted property from the block's default state, so a partial
    // state is legal and the SERVER resolves it correctly; nothing upstream of
    // the server can. The review image, the navigation walk, the diff a
    // reviewer reads and the machine gates themselves each have to guess, and
    // the guesses disagree — which is the whole reason this project renders a
    // build before believing it. An `oak_stairs[facing=east]` with no `half`
    // and no `shape` is not "the author meant the default"; it is a stair whose
    // geometry no document states.
    //
    // Same binding as `shape-complete` — the states cells actually use — so a
    // palette entry a later fill fully overwrote is not held against the piece.
    let under: Vec<String> = used
        .iter()
        .filter_map(|state| {
            let omitted = registry.omitted_properties(&state.name, &state.properties);
            if omitted.is_empty() {
                None
            } else {
                Some(format!("{state} omits {}", omitted.join(", ")))
            }
        })
        .collect();
    gates.push(Gate {
        id: "states-complete",
        state: verdict(under.is_empty()),
        undecided: 0,
        bound: used.len(),
        detail: if under.is_empty() {
            format!(
                "{} placed block state(s), every property of every block written",
                used.len()
            )
        } else {
            format!(
                "{}: {} — a state that omits a property means whatever a 1.21.11 server \
                 decides, and no reader upstream of the server can know which. Write the \
                 property the design means, including when it is the block's default",
                delvewright_schem::blocks::DW_STATE_UNDER_SPECIFIED,
                under.join("; ")
            )
        },
    });

    // --- Gate: oriented block states were guarded where the scope turns. ----
    //
    // A reorientation permutes geometry and never rewrites properties
    // (`crate::orient`), so a literal `facing`/`axis`/connection state inside
    // a reoriented scope lands however the scope was turned — silently —
    // unless a `Cond::Orientation` guard pinned the orientation the author
    // wrote it for (`DW0736`). The predicate ran during expansion, where the
    // scope orientations exist; this gate reports what it saw.
    //
    // **And what it could not see** (`DW0742`). The mismatch predicate returns
    // `None` for the identity frame before it reads a property, so a fill whose
    // scope was reoriented onto the identity AT THIS REGION got a green from a
    // test that examined nothing. That is not a pass and it is not a fail: the
    // program may be perfectly correct, and no edit to it would make this
    // region decide the question. It is the third answer, and it is
    // deliberately narrow — a fill under NO reorientation request stands in the
    // identity frame at every region there will ever be, so its world-frame
    // literal is unconditionally what the author wrote and is never counted
    // here. That is what keeps this off the ordinary building, which reorients
    // nothing and would otherwise carry the whole corpus into `undecided`.
    let audit = &expansion.oriented;
    gates.push(Gate {
        id: "oriented-fills",
        state: if !audit.unguarded.is_empty() {
            GateState::Fail
        } else if !audit.undecided.is_empty() {
            GateState::Undecided
        } else {
            GateState::Pass
        },
        undecided: audit.undecided.len(),
        bound: audit.fills as usize,
        detail: if !audit.unguarded.is_empty() {
            format!(
                "{}: {} — write one alternative per orientation, each guarded with the \
                 `orientation` cond and carrying the facing that matches it (the guard \
                 mechanism `Cond::Orientation` exists for exactly this)",
                delvewright_schem::blocks::DW_ORIENTED_FILL_UNGUARDED,
                audit
                    .unguarded
                    .iter()
                    .map(|f| format!(
                        "rule {:?} fills {} whose {} lands wrong under {}",
                        f.rule, f.state, f.property, f.orientation
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        } else if !audit.undecided.is_empty() {
            format!(
                "{}: {} fill(s) examined, {} carrying block-state properties, {} of those \
                 resolved out of the scope's own axis frame — and {} THIS REGION CANNOT DECIDE: \
                 {}. Each stands in the identity frame here only because its reorientation \
                 request resolved to a no-op at this region's proportions, so the mismatch test \
                 short-circuited before it read the state; at a region whose axes rank \
                 differently the same scope is turned and the same literal is judged. Wrap the \
                 state as `{{\"local\": …}}` so it is read in the scope's own axis names and \
                 resolved at fill time, or pin the frame with an `orientation` guard. Neither \
                 the verdict nor the artifact is refused on this — the program may be right, and \
                 nothing it could do would make THIS region say so",
                delvewright_schem::blocks::DW_ORIENTED_FILL_UNDECIDED,
                audit.fills,
                audit.carrying,
                audit.resolved,
                audit.undecided.len(),
                audit
                    .undecided
                    .iter()
                    .map(|f| format!(
                        "rule {:?} fills {} whose {} is frame-sensitive, under `{}` in rule {:?}",
                        f.rule, f.state, f.property, f.through, f.through_rule
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        } else {
            format!(
                "{} fill(s) examined, {} carrying block-state properties, {} of those resolved \
                 out of the scope's own axis frame; every remaining orientation-sensitive one \
                 was judged against a turned frame, was under an `orientation` guard, or stands \
                 in a scope no rule reorients",
                audit.fills, audit.carrying, audit.resolved
            )
        },
    });

    // --- Gates: what the world will settle these bytes into. ---------------
    //
    // Every gate above judges the model as written. These two judge it as the
    // game will hold it, and they are the only gates here that can go red on a
    // piece every other reader agrees with: a stair's `shape` is re-derived by
    // vanilla from its neighbours, and a body of fluid runs the moment a chunk
    // ticks. Always on, and for the reason the contract gates are: an author
    // who writes a stair or a pond has made a claim about the world, and there
    // is no flag that expands the piece without asking whether it is true.
    // Each is emitted only when the piece HOLDS what it judges. A gate over
    // zero objects is not a pass (CLAUDE.md), and most buildings hold no water
    // — so the honest report for them carries the count as a MEASUREMENT and
    // makes no claim at all, rather than a green line that reads like one.
    let shapes = settle::stair_shapes(model);
    if shapes.bound > 0 {
        gates.push(Gate {
            id: "stair-shape",
            state: verdict(shapes.mismatches.is_empty()),
            undecided: 0,
            bound: shapes.bound,
            detail: if shapes.mismatches.is_empty() {
                format!(
                    "{} stair(s), every written `shape` the one vanilla derives from that stair's \
                     own neighbours",
                    shapes.bound
                )
            } else {
                settle::shape_detail(&shapes)
            },
        });
    }
    let fluid = settle::fluid_bodies(model);
    if fluid.bound > 0 {
        gates.push(Gate {
            id: "fluid-contained",
            state: verdict(fluid.leaks.is_empty()),
            undecided: 0,
            bound: fluid.bound,
            detail: if fluid.leaks.is_empty() {
                settle::fluid_summary(&fluid)
            } else {
                settle::fluid_detail(&fluid)
            },
        });
    }

    // --- Gate: the expansion built something. ------------------------------
    let filled = model.filled_cells();
    let region_cells = model.region().positions().count();
    gates.push(Gate {
        id: "non-empty",
        state: verdict(filled > 0),
        undecided: 0,
        bound: region_cells,
        detail: format!("{filled} filled cell(s) of {region_cells} in the region"),
    });

    // --- Gate (opt-in): a body can walk the piece end to end. --------------
    let standable = nav::standable_cells(model);
    let reach = reachability(model, &standable);
    // The declared face contract, when the piece has one. It is what
    // `traversable` re-derives its claim from (spec-0036 §2.8): a door is a
    // thing the author declared, and counting standable cells on a face counts
    // louvres, parapets and window sills — 47 "approaches" where 3 were doors.
    let resolved = export::contract_metadata(expansion);
    let faces: Option<Vec<contract::ExteriorFace>> = resolved.as_ref().map(|c| {
        contract::exterior_faces(model, c)
            .into_iter()
            .filter(|f| f.class != "vision")
            .collect()
    });
    if options.traversable {
        match &faces {
            Some(faces) => {
                let mouths: Vec<BTreeSet<[i32; 3]>> =
                    faces.iter().map(|f| mouth(model, &standable, f)).collect();
                let bound = faces.len();
                let mut severed: Vec<String> = Vec::new();
                for (i, a) in mouths.iter().enumerate() {
                    for (j, b) in mouths.iter().enumerate().skip(i + 1) {
                        let walked = if options.allow_falls {
                            nav::reachable_with_fall(model, &standable, a, b)
                                || nav::reachable_with_fall(model, &standable, b, a)
                        } else {
                            nav::connected(&standable, a, b)
                        };
                        if !walked {
                            severed.push(format!(
                                "{} {} <-> {} {}",
                                faces[i].dir.as_str(),
                                faces[i].class,
                                faces[j].dir.as_str(),
                                faces[j].class
                            ));
                        }
                    }
                }
                gates.push(Gate {
                    id: "traversable",
                    state: verdict(bound >= 2 && severed.is_empty()),
                    undecided: 0,
                    bound,
                    detail: if bound < 2 {
                        format!(
                            "the contract declares {bound} exterior traversal edge(s). A \
                             traversability claim is a claim that a body walks THROUGH the piece, \
                             which needs two ways out"
                        )
                    } else if severed.is_empty() {
                        format!(
                            "{bound} declared way(s) in or out — {} — and a walk{} connects every \
                             pair of them",
                            faces
                                .iter()
                                .map(|f| format!("{} {}", f.dir.as_str(), f.class))
                                .collect::<Vec<_>>()
                                .join(", "),
                            if options.allow_falls {
                                " (with falls)"
                            } else {
                                ""
                            }
                        )
                    } else {
                        format!(
                            "{bound} declared way(s) in or out; no walk connects {}",
                            severed.join(", ")
                        )
                    },
                });
            }
            None => {
                // Legacy: a piece that declares no contract has no doors to
                // count, so the old face heuristic is all there is — and the
                // detail says so rather than letting the number read as doors.
                let (entry, exit) = nav::ends(model);
                let bound = entry.len() + exit.len();
                let walked = if options.allow_falls {
                    nav::reachable_with_fall(model, &standable, &entry, &exit)
                } else {
                    nav::connected(&standable, &entry, &exit)
                };
                gates.push(Gate {
                    id: "traversable",
                    state: verdict(walked && bound > 0),
                    undecided: 0,
                    bound,
                    detail: format!(
                        "{} standable cell(s) at the approach end, {} at the exit end; walking{} \
                         {}. This piece declares no spatial contract, so the binding count is \
                         standable CELLS on two faces of the region, not declared ways in — \
                         declare exterior edges and it counts doors",
                        entry.len(),
                        exit.len(),
                        if options.allow_falls {
                            " (with falls)"
                        } else {
                            ""
                        },
                        if walked {
                            "connects them"
                        } else {
                            "does NOT connect them"
                        }
                    ),
                });
            }
        }
    }
    // --- Gates: the spatial contract, whenever the piece declares one. ------
    //
    // Not opt-in, and that is the binding. An author who declares spaces has
    // made a claim about the building; the obligations are the claim's own
    // proof, and there is no flag that expands the piece without asking whether
    // the claim is true. A red here writes no `.nbt` (`main.rs` judges before it
    // freezes), so a piece whose blocks disagree with its contract cannot become
    // an artifact anyone picks up later.
    let mut enumeration = Vec::new();
    if let Some(resolved) = &resolved {
        let anchor_positions: BTreeMap<String, [i32; 3]> = expansion
            .anchors
            .iter()
            .map(|(name, a)| (name.clone(), a.pos))
            .collect();
        let checked = contract::check(model, resolved, &anchor_positions);
        gates.extend(checked.gates);
        findings.extend(checked.findings);
        enumeration = checked.enumeration;
    } else {
        findings.push(
            "this piece declares no spatial contract: no space, edge or envelope is claimed, so \
             every contract obligation examined nothing. What the building IS remains unstated, \
             and nothing downstream can check that a placed piece fits its neighbours"
                .to_string(),
        );
    }

    // --- Gate (opt-in): the piece is its own mirror image. -----------------
    if let Some(axis) = options.symmetric {
        let (pairs, broken) = asymmetry(model, axis);
        gates.push(Gate {
            id: "symmetric",
            state: verdict(broken.is_empty() && pairs > 0),
            undecided: 0,
            bound: pairs,
            detail: if broken.is_empty() {
                format!(
                    "{pairs} cell pair(s) across the {axis:?} mid-plane, every one matched \
                     (presence, not block state)"
                )
            } else {
                let [x, y, z] = broken[0];
                format!(
                    "{} of {pairs} cell pair(s) across the {axis:?} mid-plane differ; the first \
                     is {x},{y},{z} — one side is solid and the other is not, so the two halves \
                     were not built from the same rule",
                    broken.len()
                )
            },
        });
    }

    // --- Gate (opt-in): every roofed cell of floor can be walked to. --------
    if options.reachable_floor {
        let bound = reach.sheltered;
        gates.push(Gate {
            id: "reachable-floor",
            state: verdict(bound > 0 && reach.unreachable_sheltered == 0),
            undecided: 0,
            bound,
            detail: format!(
                "{} standable cell(s) under a roof; {} of them have no walking route from the {} \
                 grade entry cell(s){}",
                bound,
                reach.unreachable_sheltered,
                reach.entry_cells,
                if reach.unreachable_sheltered == 0 {
                    String::new()
                } else {
                    format!(
                        ". Largest pocket: {}",
                        reach
                            .largest_pockets
                            .first()
                            .map(Pocket::describe)
                            .unwrap_or_default()
                    )
                }
            ),
        });
    }

    for gate in &gates {
        // The contract's own gates raise their zero bindings by name inside
        // `contract::check`, which is where the second door reads them from too;
        // repeating them here would print each twice.
        if gate.bound == 0 && !gate.id.starts_with("contract-") {
            findings.push(format!(
                "gate `{}` examined ZERO objects — its verdict binds to nothing",
                gate.id
            ));
        }
        // The undecided binding, raised by name the way a zero binding is —
        // and for the same reason. A reader who takes `undecided` for a softer
        // pass is exactly the reader this whole module exists to stop, so the
        // count says out loud what the gate could not establish.
        if gate.undecided > 0 {
            findings.push(format!(
                "gate `{}` could not decide {} of the {} object(s) it examined at this region — \
                 UNDECIDED, which is neither a pass nor a fail. Its detail names each one; a \
                 region whose axes rank differently would decide them",
                gate.id, gate.undecided, gate.bound
            ));
        }
    }
    // The reachability measurement's own vacuity, then the one thing it finds
    // that an author must be told about even though no gate went red.
    //
    // Open-to-sky pockets are deliberately NOT a finding. Almost every building
    // has a roof, a roof is standable, and nobody walks it: raising it every
    // time is the nag an author learns to skip past, which would cost the two
    // findings below their audience. They are counted in the measurement line.
    if reach.standable == 0 {
        findings.push(
            "the reachability measurement examined ZERO standable cells — nothing in this piece \
             can be stood in, so it binds to nothing"
                .to_string(),
        );
    } else if reach.entry_cells == 0 {
        findings.push(format!(
            "the reachability measurement found ZERO entry cells — no standable cell touches a \
             side face of the region at grade, so there is nowhere for a body to walk in from and \
             the measurement binds to nothing. The piece's {} standable cell(s) form {} \
             component(s)",
            reach.standable, reach.pockets
        ));
    } else if reach.unreachable_sheltered > 0 {
        let named: Vec<String> = reach
            .largest_pockets
            .iter()
            .filter(|p| p.sheltered > 0)
            .map(Pocket::describe)
            .collect();
        findings.push(format!(
            "{} standable cell(s) UNDER A ROOF have no walking route from the entrance — floor in \
             a space a body cannot get to. {} pocket(s) of unreachable floor in all; the largest: \
             {}",
            reach.unreachable_sheltered,
            reach.pockets,
            named.join(" · ")
        ));
    }
    // The one thing the fluid rule deliberately does not judge, said out loud
    // rather than left as a number in the measurement line. A body that reaches
    // the piece's own outer face is a claim about the piece's NEIGHBOUR, and
    // these bytes cannot make it — a shoreline piece's water is the sea. It is
    // also the direction in which this gate could be answered rather than
    // fixed, by dragging a body out to the region face, so the count belongs
    // where a reviewer reads it.
    if !fluid.at_edge.is_empty() {
        let named: Vec<String> = fluid
            .at_edge
            .iter()
            .take(3)
            .map(|e| format!("{},{},{}", e.from[0], e.from[1], e.from[2]))
            .collect();
        findings.push(format!(
            "a body of fluid reaches this piece's own outer face in {} run direction(s) (from {}) \
             — what is beyond a face is not in these bytes, so `fluid-contained` counted them and \
             judged nothing. Whatever this piece is placed against decides where that water goes, \
             and the compiler holds it to that: `DW0318` refuses a build whose fluid ends up \
             outside every placed piece under a void horizon",
            fluid.at_edge.len(),
            named.join(", ")
        ));
    }
    if expansion.anchors.is_empty() {
        findings.push(
            "the program declared no anchors: nothing in a campaign can name a place inside this \
             piece"
                .to_string(),
        );
    }

    let (footprint_area, footprint_perimeter) = footprint(model);
    // Fail wins over undecided, and undecided over pass: a report says the worst
    // thing that is true of it. Undecided is a distinct headline rather than a
    // silent pass because "pass" is the word every caller reads, and the whole
    // defect being repaired here is a pass printed over an examination that did
    // not happen.
    let verdict = if gates.iter().any(Gate::failed) {
        "fail"
    } else if gates.iter().any(|g| g.state == GateState::Undecided) {
        "undecided"
    } else {
        "pass"
    };
    Report {
        verdict,
        measurements: Measurements {
            filled_cells: filled,
            region_cells,
            distinct_states: model.palette().len(),
            standable_cells: standable.len(),
            stairs: shapes.bound,
            fluid_cells: fluid.bound,
            fluid_held_cells: fluid.held,
            fluid_at_edge: fluid.at_edge.len(),
            footprint_area,
            footprint_perimeter,
            silhouette_complexity: complexity(footprint_area, footprint_perimeter),
            top_blocks: top_blocks(model, filled),
            local_frame_fills: expansion.oriented.resolved as usize,
            reachability: reach,
        },
        gates,
        anchors: expansion
            .anchors
            .iter()
            .map(|(name, a)| (name.clone(), a.pos))
            .collect(),
        findings,
        enumeration,
    }
}

/// Where a body actually stands at a declared way in or out.
///
/// The face's own cells when a body can stand in them (a doorway at grade), and
/// otherwise the standable cells one step inside it (a doorway whose floor
/// course belongs to the wall). Empty only when the declared opening has no
/// footing at all, which the traversability verdict then reports as a severed
/// pair rather than as a pass.
fn mouth(
    model: &VoxelModel,
    standable: &BTreeSet<[i32; 3]>,
    face: &contract::ExteriorFace,
) -> BTreeSet<[i32; 3]> {
    let direct: BTreeSet<[i32; 3]> = face
        .cells
        .iter()
        .filter(|c| standable.contains(*c))
        .copied()
        .collect();
    if !direct.is_empty() {
        return direct;
    }
    let mut near = BTreeSet::new();
    for cell in &face.cells {
        for d in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let n = [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]];
            if standable.contains(&n) {
                near.insert(n);
            }
        }
    }
    let _ = model;
    near
}

/// Cell pairs across the mid-plane of `axis`, and the ones whose two halves
/// disagree — lowest-first, so the first entry is stable (ADR-0006).
///
/// **Presence, not block state.** A stair or a door placed correctly in one half
/// is a *different* state in the other, since nothing reflects a block's
/// `facing`; comparing states would red every symmetric building that contains
/// one. Solid-versus-not is the property a mirror plane really asserts, and it
/// is the property the defect this gate exists for breaks: an interior face left
/// open where its mirror image is walled.
///
/// An odd extent leaves the centre plane paired with itself, which is trivially
/// equal and is not counted.
fn asymmetry(model: &VoxelModel, axis: Axis) -> (usize, Vec<[i32; 3]>) {
    let region = model.region();
    let a = axis.index();
    let lo = region.origin[a];
    let hi = lo + region.size[a] as i32 - 1;
    let solid = |p: [i32; 3]| model.get(p).is_some_and(|b| !b.is_air());

    let mut pairs = 0;
    let mut broken = Vec::new();
    for pos in region.positions() {
        if pos[a] * 2 >= lo + hi {
            continue; // the far half, and the self-paired centre plane
        }
        let mut partner = pos;
        partner[a] = lo + hi - pos[a];
        pairs += 1;
        if solid(pos) != solid(partner) {
            broken.push(pos);
        }
    }
    (pairs, broken)
}

/// The plan view: how many columns carry a block, and how long the outline of
/// those columns is.
fn footprint(model: &VoxelModel) -> (usize, usize) {
    let region = model.region();
    let mut columns: BTreeSet<[i32; 2]> = BTreeSet::new();
    for pos in region.positions() {
        if model.get(pos).is_some_and(|b| !b.is_air()) {
            columns.insert([pos[0], pos[2]]);
        }
    }
    let perimeter = columns
        .iter()
        .map(|&[x, z]| {
            [[1, 0], [-1, 0], [0, 1], [0, -1]]
                .iter()
                .filter(|[dx, dz]| !columns.contains(&[x + dx, z + dz]))
                .count()
        })
        .sum();
    (columns.len(), perimeter)
}

/// The outline's length over the outline a solid square of the same area would
/// have. 1.0 is a plain box; a colonnade or a buttressed wall is well above it.
fn complexity(area: usize, perimeter: usize) -> f64 {
    if area == 0 {
        return 0.0;
    }
    let square = 4.0 * (area as f64).sqrt();
    perimeter as f64 / square
}

fn top_blocks(model: &VoxelModel, filled: usize) -> Vec<(String, f64)> {
    if filled == 0 {
        return Vec::new();
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for pos in model.region().positions() {
        if let Some(b) = model.get(pos)
            && !b.is_air()
        {
            *counts.entry(b.to_string()).or_insert(0) += 1;
        }
    }
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    // Descending by count, then by name, so the order is total (ADR-0006).
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows.truncate(5);
    rows.into_iter()
        .map(|(name, n)| (name, n as f64 / filled as f64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockState;
    use crate::expand::ExpandOptions;
    use crate::geom::{Axis, Box3};
    use crate::ir::{
        Alternative, CmpOp, Cond, DimRef, Expr, Material, Node, Program, Reorient, Rounding, Size,
        Split,
    };
    use crate::library;

    fn solid_block(name: &str) -> Program {
        Program::new("slab", "all").rule(
            "all",
            Node::Fill {
                material: Material::block(BlockState::simple(name)),
            },
        )
    }

    /// **The red and the green of the reachability work, one rule apart.**
    ///
    /// A two-storey building under one roof, expanded into `9 x 13 x 9`: a
    /// ground floor at `y=1`, a first floor at `y=6` over a slab at `y=5`, a
    /// roof at `y=10`, and a three-block-wide well cut down the `x=0..2` side.
    ///
    /// `with_stair` is the only difference and it is one rule. `false` floors
    /// the well like the rest of the storey, so the first floor is 81 cells of
    /// standable floor under a roof with nothing leading to it — the shape the
    /// Notre-Dame zones ship at scale, and one every existing gate calls green.
    /// `true` fills the well with the taper recursion, whose treads rise one
    /// course per cell of depth and therefore walk.
    ///
    /// The roof is standable too, and is *meant* to be: it is the fixture's
    /// second job, because a gate that cannot tell an unreachable roof from an
    /// unreachable room is one an author turns off.
    fn two_storey(with_stair: bool) -> Program {
        let void_floor_void = Node::Split(Split {
            axis: Axis::Y,
            sizes: vec![Size::abs(4), Size::abs(1), Size::rel(1)],
            rounding: Rounding::Truncate,
            repeat: false,
            orient: Reorient::KEEP,
            children: vec![Node::Void, Node::fill("stone"), Node::Void],
        });
        let well = if with_stair {
            Node::call("stair")
        } else {
            void_floor_void.clone()
        };
        Program::new("two-storey", "all")
            .role("stone", BlockState::simple("minecraft:stone_bricks"))
            .rule(
                "all",
                Node::Split(Split {
                    axis: Axis::Y,
                    sizes: vec![Size::abs(1), Size::abs(9), Size::abs(1), Size::rel(1)],
                    rounding: Rounding::Truncate,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![
                        Node::fill("stone"),
                        Node::call("body"),
                        Node::fill("stone"),
                        Node::Void,
                    ],
                }),
            )
            .rule(
                "body",
                Node::Split(Split {
                    axis: Axis::X,
                    sizes: vec![Size::abs(3), Size::rel(1)],
                    rounding: Rounding::Truncate,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![well, Node::call("storeys")],
                }),
            )
            .rule("storeys", void_floor_void)
            // One tread per course, cut back one cell per course: the taper
            // recursion of `grammar.md` §2c, which is the only way the language
            // states a stair.
            .rule_alts(
                "stair",
                vec![
                    Alternative::new(Node::fill("stone")).when(Cond::Any {
                        of: vec![
                            Cond::cmp(Expr::dim(DimRef::Y), CmpOp::Le, Expr::int(1)),
                            Cond::cmp(Expr::dim(DimRef::Z), CmpOp::Le, Expr::int(1)),
                        ],
                    }),
                    Alternative::new(Node::Split(Split {
                        axis: Axis::Y,
                        sizes: vec![Size::abs(1), Size::rel(1)],
                        rounding: Rounding::Truncate,
                        repeat: false,
                        orient: Reorient::KEEP,
                        children: vec![
                            Node::fill("stone"),
                            Node::Split(Split {
                                axis: Axis::Z,
                                sizes: vec![Size::abs(1), Size::rel(1)],
                                rounding: Rounding::Truncate,
                                repeat: false,
                                orient: Reorient::KEEP,
                                children: vec![Node::Void, Node::call("stair")],
                            }),
                        ],
                    }))
                    .when(Cond::Otherwise),
                ],
            )
    }

    fn judge_two_storey(with_stair: bool, options: Options) -> Report {
        let out = crate::expand(
            &two_storey(with_stair),
            Box3::at_origin([9, 13, 9]),
            &ExpandOptions::seeded(0),
        )
        .unwrap();
        judge(&out, options)
    }

    #[test]
    fn a_program_painting_a_renamed_block_fails_the_blocks_gate() {
        let out = crate::expand(
            &solid_block("minecraft:chain"),
            Box3::at_origin([3, 3, 3]),
            &ExpandOptions::seeded(0),
        )
        .unwrap();
        let report = judge(&out, Options::default());
        assert!(!report.is_pass());
        let gate = report
            .gates
            .iter()
            .find(|g| g.id == "blocks-exist")
            .unwrap();
        assert!(!gate.passed());
        assert_eq!(gate.bound, 2, "air plus the one painted state");
        assert!(
            gate.detail.contains("minecraft:iron_chain"),
            "{}",
            gate.detail
        );
    }

    /// A gate whose binding count is zero is called out even though nothing
    /// went red — the vacuity CLAUDE.md names.
    #[test]
    fn a_program_that_declares_no_anchors_is_a_finding_not_a_silent_pass() {
        let out = crate::expand(
            &solid_block("minecraft:stone"),
            Box3::at_origin([3, 3, 3]),
            &ExpandOptions::seeded(0),
        )
        .unwrap();
        let report = judge(&out, Options::default());
        assert!(report.is_pass());
        // Three of them, and a solid cube earns all three: nothing names a place
        // in it, there is nowhere in it to stand so the reachability measurement
        // examined nothing either, and it makes no spatial claim at all.
        assert_eq!(report.findings.len(), 3, "{:?}", report.findings);
        // ...and the two settling rules are ABSENT rather than green: the cube
        // holds no stair and no fluid, so there is nothing for them to judge
        // and their counts are measurements instead.
        for id in ["stair-shape", "fluid-contained"] {
            assert!(
                !report.gates.iter().any(|g| g.id == id),
                "`{id}` claimed a verdict over nothing: {:?}",
                report.gates
            );
        }
        assert_eq!(report.measurements.stairs, 0);
        assert_eq!(report.measurements.fluid_cells, 0);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.contains("no spatial contract")),
            "{:?}",
            report.findings
        );
        assert!(
            report.findings.iter().any(|f| f.contains("no anchors")),
            "{:?}",
            report.findings
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.contains("ZERO standable cells")),
            "{:?}",
            report.findings
        );
    }

    /// The opt-in walk gate, shown from both sides on rules whose whole point is
    /// that they answer it differently: a stair flight walks end to end, a drop
    /// shaft only does so if falls are allowed.
    #[test]
    fn the_traversability_gate_separates_a_stair_from_a_one_way_drop() {
        let opts = Options {
            traversable: true,
            allow_falls: false,
            symmetric: None,
            reachable_floor: false,
        };
        let stair = crate::expand(
            &library::stair_flight(),
            Box3::at_origin([5, 14, 22]),
            &ExpandOptions::seeded(3),
        )
        .unwrap();
        let report = judge(&stair, opts);
        let gate = report.gates.iter().find(|g| g.id == "traversable").unwrap();
        assert!(gate.passed(), "{}", gate.detail);
        assert!(gate.bound > 0, "{}", gate.detail);

        let drop = crate::expand(
            &library::drop_shaft(),
            Box3::at_origin([4, 8, 6]),
            &ExpandOptions::seeded(3),
        )
        .unwrap();
        let walk_only = judge(&drop, opts);
        assert!(
            !walk_only
                .gates
                .iter()
                .find(|g| g.id == "traversable")
                .unwrap()
                .passed(),
            "a one-way spill must not walk back up"
        );
        let with_falls = judge(
            &drop,
            Options {
                traversable: true,
                allow_falls: true,
                symmetric: None,
                reachable_floor: false,
            },
        );
        assert!(
            with_falls
                .gates
                .iter()
                .find(|g| g.id == "traversable")
                .unwrap()
                .passed(),
            "a one-way spill IS traversable once falling is allowed"
        );
    }

    /// **The red.** A storey of floor under a roof with no stair to it, and
    /// every gate that existed before this one calls the building green —
    /// including `traversable`, whose walk runs along the ground floor and
    /// never looks up.
    #[test]
    fn an_upper_storey_with_no_stair_passes_traversable_and_is_still_unreachable() {
        let report = judge_two_storey(
            false,
            Options {
                traversable: true,
                allow_falls: false,
                symmetric: None,
                reachable_floor: false,
            },
        );
        assert!(report.is_pass(), "{:#?}", report.gates);
        let walk = report.gates.iter().find(|g| g.id == "traversable").unwrap();
        assert!(walk.passed(), "{}", walk.detail);

        let r = &report.measurements.reachability;
        assert_eq!(r.standable, 243, "two storeys and a roof, 81 cells each");
        assert_eq!(r.sheltered, 162, "the two storeys; the roof is not");
        assert_eq!(r.reachable, 81, "the ground floor, and nothing else");
        assert_eq!(r.unreachable_sheltered, 81, "the whole upper storey");
        assert_eq!(r.unreachable_open, 81, "the roof — reported, never gated");
        assert_eq!(r.pockets, 2);
        // Ranked so the room an author can fix comes before the roof they
        // cannot, and carrying the box that says which storey it is.
        assert_eq!(r.largest_pockets[0].sheltered, 81);
        assert_eq!(r.largest_pockets[0].min, [0, 6, 0]);
        assert_eq!(r.largest_pockets[0].max, [8, 6, 8]);
        assert_eq!(r.largest_pockets[1].sheltered, 0, "the roof");

        assert!(
            report
                .findings
                .iter()
                .any(|f| f.contains("81 standable cell(s) UNDER A ROOF")),
            "{:?}",
            report.findings
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.contains("open to the sky")),
            "an unreachable roof is never raised as a finding: {:?}",
            report.findings
        );
    }

    /// **The red as a verdict, and the green.** The same building, one rule
    /// apart, under the gate an author opts into when the piece claims a body
    /// can get everywhere indoors.
    #[test]
    fn the_reachable_floor_gate_reds_on_the_missing_stair_and_greens_on_the_stair() {
        let opts = Options {
            traversable: true,
            allow_falls: false,
            symmetric: None,
            reachable_floor: true,
        };

        let red = judge_two_storey(false, opts);
        let gate = red
            .gates
            .iter()
            .find(|g| g.id == "reachable-floor")
            .unwrap();
        assert!(!gate.passed(), "{}", gate.detail);
        assert_eq!(gate.bound, 162, "it examined the floor under the roof");
        assert!(!red.is_pass());

        let green = judge_two_storey(true, opts);
        let gate = green
            .gates
            .iter()
            .find(|g| g.id == "reachable-floor")
            .unwrap();
        assert!(gate.passed(), "{}", gate.detail);
        assert!(gate.bound > 0, "{}", gate.detail);
        assert!(green.is_pass(), "{:#?}", green.gates);

        let r = &green.measurements.reachability;
        assert_eq!(r.unreachable_sheltered, 0);
        assert!(
            r.unreachable_open > 0,
            "the roof is still unreachable and still fine"
        );
        assert!(
            !green.findings.iter().any(|f| f.contains("UNDER A ROOF")),
            "{:?}",
            green.findings
        );
    }

    /// A gate that can only go red is not a gate. This one binds to the floor
    /// under the roof, so a piece with no such floor reports a binding of zero
    /// rather than a pass — the vacuity rule applied to the new artifact.
    #[test]
    fn a_piece_with_no_sheltered_floor_binds_the_reachability_gate_to_zero() {
        let report = judge_two_storey(
            true,
            Options {
                traversable: false,
                allow_falls: false,
                symmetric: None,
                reachable_floor: true,
            },
        );
        assert!(report.is_pass());

        let solid = judge(
            &crate::expand(
                &solid_block("minecraft:stone"),
                Box3::at_origin([5, 5, 5]),
                &ExpandOptions::seeded(0),
            )
            .unwrap(),
            Options {
                traversable: false,
                allow_falls: false,
                symmetric: None,
                reachable_floor: true,
            },
        );
        let gate = solid
            .gates
            .iter()
            .find(|g| g.id == "reachable-floor")
            .unwrap();
        assert_eq!(gate.bound, 0);
        assert!(!gate.passed(), "a zero binding is never a pass");
        assert!(
            solid
                .findings
                .iter()
                .any(|f| f.contains("ZERO standable cells")),
            "{:?}",
            solid.findings
        );
    }

    /// A sealed piece has nowhere for a body to walk in from, and that is a
    /// binding of zero — never a reachability of zero, which would read as a
    /// building full of stranded rooms.
    #[test]
    fn a_piece_with_no_way_in_reports_a_zero_binding_not_a_zero_reachability() {
        // A roofed slab with air inside and no opening: standable floor, no
        // standable cell anywhere on a side face.
        let sealed = Program::new("sealed", "all")
            .role("stone", BlockState::simple("minecraft:stone_bricks"))
            .rule(
                "all",
                Node::Split(Split {
                    axis: Axis::X,
                    sizes: vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                    rounding: Rounding::Truncate,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![
                        Node::fill("stone"),
                        Node::call("slice"),
                        Node::fill("stone"),
                    ],
                }),
            )
            .rule(
                "slice",
                Node::Split(Split {
                    axis: Axis::Z,
                    sizes: vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                    rounding: Rounding::Truncate,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![
                        Node::fill("stone"),
                        Node::call("column"),
                        Node::fill("stone"),
                    ],
                }),
            )
            .rule(
                "column",
                Node::Split(Split {
                    axis: Axis::Y,
                    sizes: vec![Size::abs(1), Size::rel(1), Size::abs(1)],
                    rounding: Rounding::Truncate,
                    repeat: false,
                    orient: Reorient::KEEP,
                    children: vec![Node::fill("stone"), Node::Void, Node::fill("stone")],
                }),
            );
        let report = judge(
            &crate::expand(
                &sealed,
                Box3::at_origin([7, 6, 7]),
                &ExpandOptions::seeded(0),
            )
            .unwrap(),
            Options::default(),
        );
        let r = &report.measurements.reachability;
        assert!(r.standable > 0, "there is floor inside");
        assert_eq!(r.entry_cells, 0, "and no way in");
        assert_eq!(r.reachable, 0);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.contains("ZERO entry cells")),
            "{:?}",
            report.findings
        );
    }

    /// The measurement is a function of the model alone: expanding the same
    /// program at the same seed twice gives byte-identical report JSON, and the
    /// component walk is order-independent because every set it walks is a
    /// `BTreeSet` (ADR-0006).
    #[test]
    fn the_reachability_measurement_is_deterministic_and_order_independent() {
        let opts = Options {
            traversable: true,
            allow_falls: false,
            symmetric: None,
            reachable_floor: false,
        };
        let a = judge_two_storey(false, opts);
        let b = judge_two_storey(false, opts);
        assert_eq!(a.to_json(), b.to_json());

        // Order independence, stated where it can fail: the components of a set
        // do not depend on which cell the walk starts from.
        let model = &crate::expand(
            &two_storey(false),
            Box3::at_origin([9, 13, 9]),
            &ExpandOptions::seeded(0),
        )
        .unwrap()
        .model;
        let cells = nav::standable_cells(model);
        let forward = nav::components(&cells);
        let reversed: BTreeSet<[i32; 3]> = cells.iter().rev().copied().collect();
        assert_eq!(forward, nav::components(&reversed));
        assert_eq!(forward.iter().map(|c| c.len()).sum::<usize>(), cells.len());
    }

    /// The silhouette measurement moves with the shape it measures, which is
    /// the only claim made for it: it is reported, never gated.
    #[test]
    fn the_silhouette_measurement_separates_a_box_from_a_colonnade() {
        let box_report = judge(
            &crate::expand(
                &solid_block("minecraft:stone"),
                Box3::at_origin([9, 5, 9]),
                &ExpandOptions::seeded(0),
            )
            .unwrap(),
            Options::default(),
        );
        assert_eq!(box_report.measurements.footprint_area, 81);
        assert!(
            (box_report.measurements.silhouette_complexity - 1.0).abs() < 0.01,
            "a solid square plan is complexity 1.0, got {}",
            box_report.measurements.silhouette_complexity
        );

        let temple = judge(
            &crate::expand(
                &library::temple(),
                Box3::at_origin([13, 14, 21]),
                &ExpandOptions::seeded(7),
            )
            .unwrap(),
            Options::default(),
        );
        assert!(
            temple.measurements.silhouette_complexity >= 1.0,
            "{}",
            temple.measurements.silhouette_complexity
        );
    }
}
