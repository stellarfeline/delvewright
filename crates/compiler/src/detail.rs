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
//! [`place`], called from [`crate::plan::Plan::build`], which is the only
//! constructor every world-reaching verb goes through. That is the same tooth
//! the blockout's is: inversion is not forbidden, it is **uncompilable**.
//!
//! **Frame equality is exact, and undersize refuses exactly as oversize does**
//! (`DW0843`). A part that under-fills its allocation renegotiates the whole as
//! much as one that overflows it: the box is the footprint, so a smaller
//! building means a smaller box, which is a site-plan edit and a re-walk, taken
//! visibly.
//!
//! # What invokes each check, and what happens without it
//!
//! | check | event it is bound to |
//! |---|---|
//! | `DW0841`–`DW0845` ([`check`]) | `validate_loaded` in `delvec`'s `main` — the one funnel every subcommand's validation goes through, `build` included |
//! | `DW0841` again ([`check_walk`]) | `delvec allocation`, before it prints a single number |
//! | `DW0848` | `delve-admit audit`, and [`check`] wherever a row consumes the piece |
//! | the frame, and the piece's bytes | [`place`], inside `Plan::build` |
//! | the hash line, and the blockout-drift advisory | `emit::build_with_warnings`, the one function that turns a `Plan` into a datapack |
//!
//! There is no flag, no subcommand and no checklist line. The two events that
//! begin detail work — obtaining an allocation, compiling a binding — are both
//! bound, and there is no third, because no other verb reads a `detail-plan`.
//!
//! # The hatch question, answered
//!
//! This module creates **no** opt-out. A place is bound or unbound, and the kind
//! is determined by whether a `details[]` row exists rather than chosen among
//! demands; there is no acknowledgement field, no exemption list and no severity
//! an author selects. The two soft edges are each secured by a property the
//! defect cannot supply:
//!
//! * the walk record's freshness hash — the defect `DW0841` catches is
//!   *detailing a plan the whole's walk never passed*, and that defect moves the
//!   PLAN hash, which is the one thing a fabricated-but-fresh record cannot
//!   survive a plan edit with;
//! * the blockout-drift advisory — reachable only by toolchain movement, because
//!   [`crate::blockout::walked_massing`] hashes the derivation as a pure function
//!   of plan, metrics and engine, so no campaign edit can move it without moving
//!   the plan hash first.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use delvewright_dsl::detailplan::Frame;
use delvewright_dsl::layout::{Direction, Edge};
use delvewright_dsl::metrics::Reads;
use delvewright_dsl::prefab::ContractFace;
use delvewright_dsl::siteplan::{PlacedBox, PlacedSeam};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, NodeId};

use crate::plan::PiecePlacement;
use crate::registry::PrefabRegistry;
use crate::solver::Rotation;

/// The stage name every diagnostic here carries — the document being judged.
const STAGE: &str = "detail-plan";

/// `DW0841`: detail without a passed, fresh walk of this plan.
pub const DW_UNWALKED: DwCode = DwCode::every_version("DW0841");

/// `DW0842`: the binding does not bind.
pub const DW_BINDING: DwCode = DwCode::every_version("DW0842");

/// `DW0843`: the piece is not the shape of its allocation.
pub const DW_NOT_THE_FRAME: DwCode = DwCode::every_version("DW0843");

/// `DW0844`: the piece's openings are not the plan's seams.
pub const DW_FACES: DwCode = DwCode::every_version("DW0844");

/// `DW0845`: an owed anchor has no standing.
pub const DW_ANCHOR_STANDING: DwCode = DwCode::every_version("DW0845");

// ---------------------------------------------------------------------------
// The instrument (spec-0050 §2)
// ---------------------------------------------------------------------------

/// **The engine revision, named literally.**
///
/// `CLAUDE.md`'s rule that a frozen measurement names its instrument by
/// revision, never by a version string, is the reason this is not
/// `DELVEC_VERSION`: two engines 136 commits apart report the same version.
///
/// Stamped at COMPILE time by whoever builds the binary, and honestly `unstamped`
/// when nobody did. This is a departure from a literal reading of spec-0050 §2,
/// recorded here because this is where it is made: naming the revision would
/// otherwise need a `build.rs` shelling out to `git`, on a crate published to
/// crates.io where there is no `.git` to read — a distribution change the spec
/// does not ask for, to answer a question the release recipe can answer for
/// free. What the engine must never do is *claim* a revision it does not have,
/// and `unstamped` is that claim withheld.
#[must_use]
pub fn engine_revision() -> &'static str {
    option_env!("DELVEC_ENGINE_REVISION").unwrap_or("unstamped")
}

/// How the engine names itself in a record or a refusal: the revision, and the
/// version beside it as context rather than as the name.
#[must_use]
pub fn engine_name() -> String {
    format!(
        "{rev} (delvec {ver})",
        rev = engine_revision(),
        ver = crate::DELVEC_VERSION
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let mut s = String::with_capacity(64);
    for b in h.finalize() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// **The site plan's hash**: sha256 over the document's CANONICAL bytes.
///
/// Canonical rather than as-written, so that a reformat is not a re-walk. The
/// writer is `delvewright_dsl::to_canonical_string`, which is the same one
/// `delvec fmt --check` accepts, so the hash a creator's own formatter produces
/// is the hash the gate compares.
#[must_use]
pub fn site_plan_sha256(c: &Campaign) -> Option<String> {
    let plan = c.site_plan.as_ref()?;
    let text = delvewright_dsl::to_canonical_string(plan).ok()?;
    Some(sha256_hex(text.as_bytes()))
}

/// **The blockout's hash**: sha256 over the derived massing the WALK judged.
///
/// Taken over [`crate::blockout::walked_massing`] — the derivation with nothing
/// bound — for the reason that function's own note gives.
#[must_use]
pub fn blockout_sha256(c: &Campaign) -> Option<String> {
    let mut reads = Reads::new();
    let fills = crate::blockout::walked_massing(c, &mut reads)?;
    let mut text = String::new();
    for f in &fills {
        text.push_str(&format!(
            "{} {} {} {} {} {} {}\n",
            f.from[0], f.from[1], f.from[2], f.to[0], f.to[1], f.to[2], f.block
        ));
    }
    Some(sha256_hex(text.as_bytes()))
}

/// The two hashes and the engine that produced them, printed on every build of a
/// site-plan campaign so a walk record can name its instrument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes {
    /// Over the plan's canonical bytes.
    pub site_plan: String,
    /// Over the massing a walker walked.
    pub blockout: String,
}

impl Hashes {
    /// The two hashes of `c`, or `None` for a campaign with no site plan.
    #[must_use]
    pub fn of(c: &Campaign) -> Option<Hashes> {
        Some(Hashes {
            site_plan: site_plan_sha256(c)?,
            blockout: blockout_sha256(c)?,
        })
    }

    /// One line, for stderr and for whoever is writing the walk record.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "site plan sha256: {sp}\nblockout sha256:  {bo}\nengine revision:  {rev}",
            sp = self.site_plan,
            bo = self.blockout,
            rev = engine_name(),
        )
    }
}

// ---------------------------------------------------------------------------
// The walk record (spec-0049 §5.4, gated here)
// ---------------------------------------------------------------------------

/// What a walk concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// The whole was walked and is fit to detail.
    Passed,
    /// The whole was walked and something must change first.
    Findings,
}

/// One thing a walk found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkFinding {
    /// What it is about — a place, a seam, a view.
    pub subject: String,
    /// What was wrong, in the walker's words.
    pub note: String,
}

/// **`walk-record.json`** — the record of a human walking one derived blockout.
///
/// The form is spec-0049 §5.4's, unchanged. It is not a stage document and
/// carries no `dsl_version`: it is not authored against a schema version, it is
/// the record of an event.
///
/// The machine half of the gate below is **freshness and an explicit verdict**,
/// stated plainly rather than implied. That a human actually walked is this
/// document's author's assertion, held by operating practice; no engine check
/// can prove a walk happened and nothing here pretends one can.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkRecord {
    /// The plan that was walked, by its canonical-bytes hash.
    pub site_plan_sha256: String,
    /// The massing that was walked, by its hash.
    pub blockout_sha256: String,
    /// The engine that built it — the revision, never a version string.
    pub engine_revision: String,
    /// The verdict.
    pub verdict: Verdict,
    /// What was found. Present and non-empty is compatible with `passed`: a
    /// walker may note something without it blocking detail.
    #[serde(default)]
    pub findings: Vec<WalkFinding>,
}

impl WalkRecord {
    /// Parse a record, or say why it is not one.
    pub fn parse(src: &str) -> Result<WalkRecord, String> {
        serde_json::from_str(src).map_err(|e| e.to_string())
    }
}

/// What the walk gate examined, with its denominator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalkBinding {
    /// Records read — 0 or 1, and 0 with a detail plan present is the refusal.
    pub records: usize,
    /// Hash comparisons made.
    pub compared: usize,
    /// `details[]` rows the gate stood in front of.
    pub rows: usize,
}

impl WalkBinding {
    /// One line, stated whether or not it is zero.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "detail walk gate binding: {r} walk record(s) read, {c} plan-hash comparison(s) made, \
             standing in front of {n} `details[]` row(s).",
            r = self.records,
            c = self.compared,
            n = self.rows,
        )
    }
}

/// **`DW0841`: detail without a passed walk of this plan** — at the *compiling
/// a binding* event.
///
/// Bound at the two events that begin detail work, and there is no third
/// because no other verb reads a `detail-plan`: this one is validation, and
/// therefore every build; [`allocation_walk_gate`] is `delvec allocation`.
///
/// Missing, `"findings"` and stale are each named, and a stale record's refusal
/// prints both hashes. A campaign with no detail plan binds zero of this and
/// says so.
#[must_use]
pub fn check_walk(c: &Campaign, record: Option<&str>) -> (Vec<Diagnostic>, WalkBinding) {
    let mut binding = WalkBinding::default();
    let Some(plan) = c.detail_plan.as_ref() else {
        return (Vec::new(), binding); // nothing to gate.
    };
    binding.rows = plan.content.details.len();
    let (d, b) = walk_gate(c, record, binding.rows);
    binding.records = b.records;
    binding.compared = b.compared;
    (d.into_iter().collect(), binding)
}

/// **`DW0841`** — at the *obtaining an allocation* event.
///
/// The same rule and the same code as [`check_walk`], asked of a campaign that
/// has no `detail-plan` yet — which is exactly the campaign asking for its first
/// allocation, and exactly the moment the ordering has to hold. A gate that only
/// fired once a detail plan existed would fire after the work it guards had
/// begun.
#[must_use]
pub fn allocation_walk_gate(c: &Campaign, record: Option<&str>) -> Option<Diagnostic> {
    walk_gate(
        c,
        record,
        c.detail_plan
            .as_ref()
            .map_or(0, |e| e.content.details.len()),
    )
    .0
}

/// The one implementation both doors ask.
fn walk_gate(c: &Campaign, record: Option<&str>, rows: usize) -> (Option<Diagnostic>, WalkBinding) {
    let mut d: Option<Diagnostic> = None;
    let mut binding = WalkBinding {
        rows,
        ..WalkBinding::default()
    };
    let Some(current) = site_plan_sha256(c) else {
        // A detail plan in a campaign with no site plan. `DW0842`'s limiting
        // case names that, and answering it twice would be two diagnostics for
        // one defect.
        return (d, binding);
    };
    let Some(src) = record else {
        d = Some(Diagnostic::error(
            DW_UNWALKED,
            STAGE,
            "/content/details",
            format!(
                "this campaign has no `walk-record.json`. The whole map \
                 is walked before any part of it is detailed: detailing is where a map's cost \
                 stops being cheap to change, so the whole is judged first, in game, and the \
                 record of that judgement is what unlocks the parts. Write \
                 `walk-record.json` beside the stage documents, with \
                 `site_plan_sha256` set to `{current}`, `blockout_sha256` set to the hash the \
                 build printed beside it, `engine_revision` set to the revision that built it \
                 (this one is `{rev}`), `verdict` set to `passed`, and a `findings` list of \
                 whatever the walk noted. Every build of this campaign prints both hashes. \
                 Binding: {n} `details[]` row(s) stood in front of, ZERO records read.",
                rev = engine_revision(),
                n = binding.rows,
            ),
        ));
        return (d, binding);
    };
    binding.records = 1;
    let rec = match WalkRecord::parse(src) {
        Ok(r) => r,
        Err(e) => {
            d = Some(Diagnostic::error(
                DW_UNWALKED,
                STAGE,
                "/content/details",
                format!(
                    "`walk-record.json` is not a walk record: {e}. Its form is fixed — \
                     `site_plan_sha256`, `blockout_sha256`, `engine_revision`, `verdict` \
                     (`passed` or `findings`), and `findings[]` of `{{subject, note}}`. A record \
                     that does not parse is a record nothing can be judged against, so it is a \
                     refusal rather than an absence. Binding: {n} `details[]` row(s) stood in \
                     front of, 1 record read.",
                    n = binding.rows,
                ),
            ));
            return (d, binding);
        }
    };
    binding.compared = 1;
    if rec.site_plan_sha256 != current {
        d = Some(Diagnostic::error(
            DW_UNWALKED,
            STAGE,
            "/content/details",
            format!(
                "`walk-record.json` records a walk of a DIFFERENT site plan, so this plan has \
                 not been walked. The record names `{recorded}`; this campaign's plan hashes to \
                 `{current}`. The hash is taken over the plan's canonical bytes, so a reformat is \
                 not a re-walk — these are different plans. That is the escalation path working, \
                 not a nuisance: a part that wants different traversal revises the SITE PLAN, \
                 which moves this hash, which re-opens this gate, which re-runs the whole's walk. \
                 Walk the current blockout and re-record. Binding: {n} `details[]` row(s) stood \
                 in front of, 1 plan-hash comparison made.",
                recorded = rec.site_plan_sha256,
                n = binding.rows,
            ),
        ));
        return (d, binding);
    }
    if rec.verdict != Verdict::Passed {
        let list = if rec.findings.is_empty() {
            "and names no findings, which is a record that says the walk did not pass and does \
             not say why"
                .to_string()
        } else {
            format!(
                "and names {} finding(s): {}",
                rec.findings.len(),
                rec.findings
                    .iter()
                    .map(|f| format!("`{}` — {}", f.subject, f.note))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };
        d = Some(Diagnostic::error(
            DW_UNWALKED,
            STAGE,
            "/content/details",
            format!(
                "`walk-record.json` records `verdict: \"findings\"` {list}. Detail work does not \
                 begin on a whole that has not passed its walk — that is the ordering this \
                 pipeline exists to make structural. Answer the findings in the graph or the \
                 plan, rebuild, walk again, and re-record with `verdict: \"passed\"`. Binding: \
                 {n} `details[]` row(s) stood in front of, 1 plan-hash comparison made.",
                n = binding.rows,
            ),
        ));
    }
    (d, binding)
}

/// **The blockout-drift advisory** (spec-0050 §2): same plan, different massing
/// bytes.
///
/// A warning naming both hashes and both engine revisions, and never a refusal.
/// The hatch question, answered: a campaign author has **no edit** that moves
/// the blockout hash without moving the plan hash, because the derivation is a
/// pure function of plan, metrics and engine. So this path is reachable only by
/// toolchain movement — an engine or metrics change — which is a re-walk
/// *decision* for the round summary, not a defect anyone could launder through
/// it.
#[must_use]
pub fn blockout_drift(c: &Campaign, record: Option<&str>) -> Option<Diagnostic> {
    c.detail_plan.as_ref()?;
    let rec = WalkRecord::parse(record?).ok()?;
    let current = blockout_sha256(c)?;
    if rec.blockout_sha256 == current {
        return None;
    }
    // A record of a different PLAN is `DW0841`'s refusal; saying so twice would
    // be two diagnostics for one defect.
    if site_plan_sha256(c).as_deref() != Some(rec.site_plan_sha256.as_str()) {
        return None;
    }
    Some(Diagnostic::warning(
        DW_UNWALKED,
        STAGE,
        "/content/details",
        format!(
            "the walked massing has MOVED under an unchanged site plan. The record was taken on \
             engine `{was_rev}` and names blockout `{was}`; engine `{now_rev}` derives \
             `{now}` from the same plan. This refuses nothing. No campaign edit can reach this \
             state — the derivation is a pure function of the plan, the metrics table and the \
             engine, so a plan edit would have moved the plan hash and refused above. What has \
             moved is the toolchain, and whether the whole is walked again is a decision for the \
             round summary rather than a defect to repair.",
            was = rec.blockout_sha256,
            was_rev = rec.engine_revision,
            now = current,
            now_rev = engine_name(),
        ),
    ))
}

// ---------------------------------------------------------------------------
// The frames, and what they hand out
// ---------------------------------------------------------------------------

/// Every place's frame, by node name, in plan document order.
#[must_use]
pub fn frames(c: &Campaign) -> Vec<(Frame, PlacedBox)> {
    let mut reads = Reads::new();
    delvewright_dsl::placed_boxes(c, &mut reads)
        .into_iter()
        .map(|b| (Frame::of(&b), b))
        .collect()
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

/// **Which cell layer of the frame answers this seam**, and it is one of exactly
/// two cases.
///
/// A vertical party plane sits one cell beyond the frame, so the answering layer
/// is the frame's own boundary cell and the piece's opening leaves *toward* it.
/// A horizontal party plane between stacked boxes **is** the upper box's floor
/// course, so for that box the answering layer is the plane itself and the
/// piece's opening is *at* those cells — which is spec-0050 §3's stacked-box
/// sentence, in arithmetic.
fn answering_layer(frame: &Frame, s: &PlacedSeam) -> i64 {
    let a = s.normal_axis;
    if s.plane >= frame.lo[a] && s.plane <= frame.hi[a] {
        s.plane // the seam lies in the piece's own floor course
    } else if s.plane > frame.hi[a] {
        frame.hi[a]
    } else {
        frame.lo[a]
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
            // The bar stands in the whole's plane beyond the piece unless the
            // plane IS the piece's own floor course, in which case the piece
            // ships the gate's shut state.
            if answering_layer(frame, s) == s.plane {
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
    /// Which way out of the piece it leaves by.
    pub face: String,
    /// The cells the piece's answering face must cover, **piece-local**.
    pub cells: [[i64; 3]; 2],
    /// `floor(other) − floor(this)`, in cells.
    pub rise: i64,
    /// The class of face the piece must answer with. More than one where both
    /// are correct — a stair the piece hosts may meet its opening at grade.
    pub answer_with: Vec<String>,
}

/// **What `delvec allocation` prints**: everything the whole gives a place, and
/// nothing a piece could give back.
///
/// Derived from the site plan on every invocation and **not an input to
/// anything**. No gate, no build step and no check ever reads an allocation
/// file: `DW0842`–`DW0845` and the stage-5 bytes battery recompute every
/// obligation from the plan itself at every validation, so a committed
/// allocation file is a copy with no consumer and its staleness has no vector
/// into the build. It exists for the authoring loop alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Allocation {
    /// The place.
    pub place: String,
    /// The frame's size in cells, `[x, y, z]` — what the piece must be, exactly.
    pub extent: [i64; 3],
    /// The walk plane's piece-local `y`. The floor course under it is the
    /// piece's; everything around the frame is the whole's.
    pub datum_y: i64,
    /// The frame's world position, for a reader who wants to find it in game.
    /// Not an input: nothing reads it back.
    pub world_min: [i64; 3],
    /// Every seam of this box, piece-local.
    pub seams: Vec<AllocatedSeam>,
    /// The synthesized names this place owes, which `anchors` must bind.
    pub owed_anchors: Vec<String>,
    /// The whole's material vocabulary, handed through ungated (spec-0050 §4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<BTreeMap<String, String>>,
}

/// The allocation for one place, or `None` when the plan has no such box.
#[must_use]
pub fn allocation(c: &Campaign, node: &NodeId) -> Option<Allocation> {
    let mut reads = Reads::new();
    let boxes = delvewright_dsl::placed_boxes(c, &mut reads);
    let seams = delvewright_dsl::placed_seams(c, &boxes, &mut reads);
    let b = boxes.iter().find(|b| &b.node == node)?;
    let frame = Frame::of(b);
    let mine = seams_of(&seams, node);
    Some(Allocation {
        place: node.0.clone(),
        extent: frame.extent(),
        datum_y: frame.datum_y(),
        world_min: frame.lo,
        seams: mine
            .iter()
            .map(|(s, out)| {
                let (lo, hi) = answering_cells(&frame, s);
                AllocatedSeam {
                    edge: s.edge.0.clone(),
                    class: s.class.to_string(),
                    face: dir_name(*out).to_string(),
                    cells: [frame.to_local(lo), frame.to_local(hi)],
                    rise: if &s.a == node { s.rise } else { -s.rise },
                    answer_with: required_face_class(edge_of(c, s), s, node, &frame)
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                }
            })
            .collect(),
        owed_anchors: delvewright_dsl::owed_anchors(c, node).into_iter().collect(),
        palette: c
            .detail_plan
            .as_ref()
            .and_then(|e| e.content.palette.clone()),
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
    /// `details[]` rows resolved — `DW0842`.
    pub details: usize,
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
}

impl DetailBinding {
    /// One line, stated whether or not it is zero.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "detail binding: {d} of {b} place(s) bound, {m} piece(s) measured against their \
             frame, {sr} seam(s) required answering over {fe} declared face(s) examined, {o} \
             owed anchor name(s) checked, {cl} piece(s) declaring a footprint class.",
            d = self.details,
            b = self.boxes,
            m = self.measured,
            sr = self.seams_required,
            fe = self.faces_examined,
            o = self.owed,
            cl = self.classed,
        )
    }
}

/// **`DW0841`–`DW0845` and `DW0848`'s consumer door, over a whole campaign.**
///
/// Validation tier: a diagnostic here is exit 1, before any byte is written.
/// Bound in `validate_loaded`, which every `delvec` subcommand's validation goes
/// through — `build` included, so a defect cannot reach a datapack by skipping
/// `delvec validate`.
///
/// A campaign with no detail plan runs every check zero times and says so.
pub fn check(
    c: &Campaign,
    prefabs: &PrefabRegistry,
    walk_record: Option<&str>,
) -> (Vec<Diagnostic>, DetailBinding) {
    let (mut d, _walk) = check_walk(c, walk_record);
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
    let boxes = delvewright_dsl::placed_boxes(c, &mut reads);
    let seams = delvewright_dsl::placed_seams(c, &boxes, &mut reads);
    let by_node: BTreeMap<&str, &PlacedBox> =
        boxes.iter().map(|b| (b.node.0.as_str(), b)).collect();
    binding.boxes = boxes.len();

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, row) in doc.details.iter().enumerate() {
        binding.details += 1;
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
                     plan first — which is a plan edit, and re-opens the walk gate.",
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
        let frame = Frame::of(b);

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
                     smaller building means a smaller box. That is a site-plan edit and a \
                     re-walk, taken visibly, and it is the only way a part changes what the whole \
                     gave it. Run `delvec allocation {place}` for the frame, the datum and every \
                     seam this box must answer.",
                    piece = row.piece,
                    place = row.place,
                    gx = got64[0],
                    gy = got64[1],
                    gz = got64[2],
                    wx = want[0],
                    wy = want[1],
                    wz = want[2],
                    over = over.join("; "),
                ),
            ));
            continue;
        }

        // ---- DW0843, second half: a detail piece owes a contract ----
        let Some(contract) = meta.spatial_contract.as_ref() else {
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
                     it through `delve-admit`, which resolves one.",
                    piece = row.piece,
                ),
            ));
            continue;
        };

        // ---- DW0848: the declared class, at the consumer door ----
        if meta.footprint_class.is_some() {
            binding.classed += 1;
            if let Some(f) = delvewright_dsl::prefab::check_footprint_class(
                meta,
                STAGE,
                &format!("{path}/piece"),
                &mut reads,
            ) {
                d.push(f);
            }
        }

        // ---- DW0844: faces against seams, both directions ----
        let mine = seams_of(&seams, &row.place);
        let mut answered: BTreeSet<usize> = BTreeSet::new();
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
                             SITE PLAN — which moves the plan hash and re-runs the whole's walk.",
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
                     graph and the site plan — which is a plan edit, and re-runs the whole's walk.",
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
    /// `(campaign anchor name, world cell, facing)` for every owed name a row
    /// binds — the re-binding of spec-0050 §6. The facing is the piece's,
    /// because which way a body faces when it arrives is a fact about the room
    /// it arrives in.
    pub anchors: Vec<(String, [i32; 3], Option<String>)>,
}

/// **The one path from a `details[]` row to placed bytes.**
///
/// Called once, from [`crate::plan::Plan::build`], and there is no second
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
            templates: crate::plan::placed_templates(meta, pos, Rotation::None),
            pos,
            size: meta.size(),
            rotation: Rotation::None,
        });
        for (name, bound_to) in &row.anchors {
            let Some(a) = meta.anchors.get(bound_to) else {
                continue; // `DW0842` refused it.
            };
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
        }
    }

    /// A seam of `class` on `face` of `a`, with its plane at `plane`.
    fn a_seam(class: &'static str, face: Face, plane: i64, normal_axis: usize) -> PlacedSeam {
        let flat = |v: i64| {
            let mut c = [1i64; 3];
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
            rise: 0,
            stair_in: None,
        }
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
        let f = Frame::of(&upper);
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

        // `barred` in a VERTICAL party plane → `walk`: the bar stands in the
        // whole's plane, beyond the piece.
        let s = a_seam("barred", Face::East, f.hi[0] + 1, 0);
        assert_ne!(
            answering_layer(&f, &s),
            s.plane,
            "the plane is outside the frame"
        );
        assert_eq!(required_face_class(None, &s, &a, &f), ["walk"]);

        // `barred` in THIS piece's own floor course → `barred`: the piece ships
        // the gate's shut state, because that plane is the piece's. **This row is
        // reached by no site plan in this repository**, which is why it is here.
        let s = a_seam("barred", Face::Down, f.lo[1], 1);
        assert_eq!(
            answering_layer(&f, &s),
            s.plane,
            "the plane IS the floor course"
        );
        assert_eq!(required_face_class(None, &s, &a, &f), ["barred"]);
    }

    /// The answering layer is one of exactly two cells, and which one is a fact
    /// about where the party plane lies relative to the frame — not a choice.
    #[test]
    fn the_answering_layer_is_the_plane_or_the_frame_face_nearest_it() {
        let f = Frame::of(&a_box("node/a", 64));
        for (axis, face) in [(0usize, Face::East), (1, Face::Up), (2, Face::South)] {
            let beyond_high = a_seam("walk", face, f.hi[axis] + 1, axis);
            assert_eq!(answering_layer(&f, &beyond_high), f.hi[axis]);
            let beyond_low = a_seam("walk", face, f.lo[axis] - 1, axis);
            assert_eq!(answering_layer(&f, &beyond_low), f.lo[axis]);
            let inside = a_seam("walk", face, f.lo[axis], axis);
            assert_eq!(
                answering_layer(&f, &inside),
                f.lo[axis],
                "a plane inside the frame answers AT itself"
            );
        }
    }
}
