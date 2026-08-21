//! The metrics standard (spec-0049 §2) — pipeline stage 0.
//!
//! One machine-readable table, engine-owned data, in two halves whose epistemic
//! status differs and is recorded per entry:
//!
//! * **Player metrics** — facts of pinned Minecraft Java 1.21.11. Not chosen,
//!   so not calibratable: walking a level cannot make a player 0.7 blocks wide.
//! * **Building metrics** — standards this project fixes. Every one carries
//!   [`BuildingEntry::calibrated`], `false` until the metrics gym's walk
//!   (spec-0049 §2.3) rules on it.
//!
//! # Why this module is in `delvewright-dsl`
//!
//! It is the crate every other one already reaches: `schem`, `grammar`, `admit`,
//! `compiler` and `render` all resolve it. A metrics module that the navigation
//! model imports but the render layer cannot would be one authority for some
//! consumers and a copy for the rest, which is the shape this table exists to
//! end. It is also the crate that owns [`crate::diagnostic`], and the stage-3
//! and stage-4 documents whose names resolve into this table are validated here.
//!
//! # One authority, structurally
//!
//! The player half is not a second table that agrees with the navigation model —
//! it **is** the navigation model's constants. `compiler::nav` imports
//! [`MAX_AUTO_STEP_16`], [`MAX_JUMP_RISE_16`] and [`FULL_16`] from here;
//! `compiler::crosshair`, `compiler::render_plan`, `compiler::view::viewer` and
//! `compiler::combat` import the body constants they used to declare. There is
//! one definition of each number in the workspace and the export re-serializes
//! it, so a player metric cannot drift from the model that proves routes.
//!
//! What that replaced is worth recording, because it is the defect this table
//! was written against rather than a hypothetical: the player eye height was
//! declared four times (`compiler::render_plan`, `compiler::view::viewer`,
//! `compiler::creator` in milli-blocks, `render::occupancy` in `f32`) and the
//! body width twice, each a literal `0.6` or `1.62` with its own doc comment
//! saying it was vanilla's. Nothing related them, so nothing could have gone red
//! had one moved.
//!
//! # Provenance is recorded per entry
//!
//! [`Provenance`] says where a number came from, and the four values are
//! deliberately not interchangeable: an [`Provenance::EngineConstant`] cannot
//! drift because there is nothing to drift from, a [`Provenance::VanillaRule`] is
//! a claim about the game this repository has **not** measured on a running
//! server, a [`Provenance::Derived`] carries its arithmetic in its note, and a
//! [`Provenance::Provisional`] is a seed for the gym and is not a standard yet.
//! Dressing the last as one of the first three is the failure this field exists
//! to make impossible to commit silently.
//!
//! # A provisional value cannot be consumed quietly
//!
//! [`BuildingEntry::value`] is the only way to read a building metric's number,
//! and it takes `&mut `[`Reads`]. So a verdict that rests on an uncalibrated
//! standard has, by construction, recorded that it did, and [`Metrics::notice`]
//! turns that ledger into `DW0813`. The obligation lives in the signature, not
//! in a line of documentation somebody has to remember.
//!
//! The residual, named rather than implied: a caller that constructs its own
//! [`Reads`], reads through it and drops it has bypassed the notice. That is a
//! deliberate act and not the omission the rule exists to catch — nothing
//! *forgets* to thread a ledger it had to construct. It closes completely when
//! stage-3 and stage-4 validation thread one run-scoped ledger, which is the
//! round that first has checks reading this half.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::diagnostic::{Diagnostic, DwCode};

/// `DW0812`: a document names a metrics entry the table does not define.
///
/// `every_version`, and by the test [`crate::diagnostic::Binds`] states: the rule
/// judges what a document SAYS — a `size_class`, an `opening` or a `pitch` that
/// resolves to nothing — and its verdict is a function of the campaign alone. It
/// cannot reach a campaign written before the surface existed, because there is
/// no field below `dsl_version` 0.13.0 in which to write such a name and
/// `DW0141` refuses one that tries.
pub const DW_METRIC_UNKNOWN: DwCode = DwCode::every_version("DW0812");

/// `DW0813`: a verdict rests on a standard the gym has not walked.
///
/// `every_version` for a different reason from its neighbour, and the difference
/// is worth stating: this rule asks the campaign for nothing at all. It reports a
/// property of the ENGINE's own table — that some number a check just used is a
/// seed rather than a standard — so there is no obligation for a fence to
/// grandfather and no campaign that could adopt its way out of it. It is a
/// warning (exit 0) for the same reason: a provisional number is still a number,
/// the check still refuses, and what the line adds is that the green rests on
/// something nobody has walked.
pub const DW_METRIC_PROVISIONAL: DwCode = DwCode::every_version("DW0813");

// ---------------------------------------------------------------------------
// The player half — the one definition of each constant in this workspace.
// ---------------------------------------------------------------------------

/// The table's own revision. Bumped whenever the exported JSON changes by a
/// single byte, which `crates/dsl/tests/metrics.rs` enforces against a committed
/// digest: a consumer that pins a version is pinning values, and values that
/// move under a fixed version are the drift the pin was bought to prevent.
///
/// It is deliberately **not** a version ledger in the sense
/// `tools/check-version-ledger-uniqueness.py` guards. That gate compares the
/// *fence anchors* two branches attach to one number, and this ledger has none:
/// nothing grandfathers against a metrics version, no document declares one, and
/// no surface is gated by one. What it needs instead is that the number cannot
/// stand still while the table moves, and that is the digest test.
pub const METRICS_VERSION: u32 = 1;

/// Player collision-box width in blocks (`0.6 × 0.6 × 1.8` standing).
pub const PLAYER_WIDTH: f64 = 0.6;

/// Player collision-box height in blocks, standing.
pub const PLAYER_HEIGHT: f64 = 1.8;

/// Player collision-box height in blocks, crouched.
pub const PLAYER_CROUCHED_HEIGHT: f64 = 1.5;

/// Player eye height above the floor of the cell the body stands in.
pub const PLAYER_EYE_HEIGHT: f64 = 1.62;

/// Player maximum health, in half-heart damage points.
pub const PLAYER_MAX_HEALTH: f64 = 20.0;

/// The largest rise, in sixteenths of a block, a walker crosses **without
/// jumping** — vanilla's player `maxUpStep` is 0.6 blocks, and 9/16 = 0.5625 is
/// the largest sixteenth under it (10/16 = 0.625 already needs a jump). A rise
/// within this budget needs no headroom above the *source* cell: the player walks
/// straight up onto a slab or a path edge.
pub const MAX_AUTO_STEP_16: i64 = 9;

/// The largest rise a walker can reach **by jumping**, in sixteenths. A vanilla
/// player's jump apex is ≈1.2522 blocks, so a surface 20/16 = 1.25 up is
/// reachable and 21/16 = 1.3125 is not. This is the bound that makes the
/// **1.5-block** slab-to-full-block step-up the impossible move it is.
pub const MAX_JUMP_RISE_16: i64 = 20;

/// A full block's height in sixteenths.
pub const FULL_16: i64 = 16;

/// Ticks a jumping player spends off the ground, apex to landing included.
pub const JUMP_AIRBORNE_TICKS: f64 = 12.0;

/// Player walking speed on the flat, in blocks per second (not sprinting).
pub const WALK_SPEED_BLOCKS_PER_SECOND: f64 = 4.317;

/// Server ticks per second.
pub const TICKS_PER_SECOND: f64 = 20.0;

/// The fall distance in blocks below which vanilla deals no fall damage: damage
/// is `ceil(distance − 3)` points, so a 3-block fall is free and a 4-block fall
/// costs one.
pub const FALL_DAMAGE_ONSET_BLOCKS: f64 = 3.0;

/// Ticks a walking player spends crossing one block on the flat.
///
/// Derived rather than stored, because both operands are facts and nothing
/// downstream decides a route on this number — it is the pacing denominator.
#[must_use]
pub fn walk_ticks_per_block() -> f64 {
    TICKS_PER_SECOND / WALK_SPEED_BLOCKS_PER_SECOND
}

/// The largest fall an unarmoured player at full health survives, in blocks.
///
/// `ceil(d − 3) < 20` holds up to `d = 22`, which lands on one half-heart; 23
/// blocks deals 20 and kills. Derived from [`FALL_DAMAGE_ONSET_BLOCKS`] and
/// [`PLAYER_MAX_HEALTH`] so that moving either moves this, and it exists so the
/// designed-drop policy beside it has a physical ceiling to be **tighter than**
/// rather than a number chosen next to nothing.
#[must_use]
pub fn unarmoured_survivable_fall_blocks() -> f64 {
    (FALL_DAMAGE_ONSET_BLOCKS + PLAYER_MAX_HEALTH - 1.0).floor()
}

/// Horizontal cells a standing body needs to pass: `ceil(0.6)`.
#[must_use]
pub fn passable_width_cells() -> u32 {
    PLAYER_WIDTH.ceil() as u32
}

/// Vertical cells a standing body needs to pass: `ceil(1.8)`.
///
/// This is a **player** metric and the spec's building half listed it, which is
/// the correction worth naming: the width and clearance at which a body can pass
/// at all are functions of the collision box, so no walk can change them and
/// `calibrated` would mean nothing on them. What the gym calibrates is the
/// *designed* minimum (`corridor.min-width`, `corridor.min-clearance`), which is
/// a comfort judgement and can never be chosen below this floor —
/// [`Metrics::self_check`] is what holds it there.
#[must_use]
pub fn passable_clearance_cells() -> u32 {
    PLAYER_HEIGHT.ceil() as u32
}

// ---------------------------------------------------------------------------
// Entry shapes
// ---------------------------------------------------------------------------

/// Where a number came from. The four are not interchangeable — see the module
/// docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// The number **is** a constant this module defines and the rest of the
    /// workspace imports. One definition, so nothing to drift from.
    EngineConstant,
    /// A stated rule of pinned Minecraft Java 1.21.11 that no engine constant
    /// held before this table, and that this repository has **not** measured on
    /// a running server. The note names the rule so the claim is checkable.
    VanillaRule,
    /// Computed from other entries; the note carries the arithmetic.
    Derived,
    /// A seed for the metrics gym's calibration walk. Chosen, not established.
    Provisional,
}

/// A named opening in the standard seam set, in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Opening {
    /// Clear width, in cells.
    pub width: u32,
    /// Clear height, in cells.
    pub height: u32,
}

/// A named stair-pitch standard: a rise:run pattern together with the vanilla
/// blocks that realize it, and the per-step rise the realization actually
/// presents to a walking body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Pitch {
    /// Blocks of rise per `run` blocks of horizontal travel.
    pub rise: u32,
    /// Blocks of horizontal travel per `rise` blocks of rise.
    pub run: u32,
    /// The rise in sixteenths that one realized tread presents. A stair block
    /// offers its lower half first, so a 1:1 stair run steps 8/16 twice per
    /// block of rise rather than 16/16 once; a slab ramp steps 8/16 as well.
    /// This is the number [`Metrics::self_check`] holds under the walk-up
    /// budget, which is what makes "standard pitch" mean *walked*, not merely
    /// *legal*.
    pub step_16: i64,
    /// The vanilla realization, named so the derivation above is checkable
    /// against blocks rather than asserted.
    pub realization: &'static str,
}

/// A rung of the size-class ladder — the vocabulary a layout-graph node declares
/// and a site-plan box is judged against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SizeClass {
    /// Smallest interior footprint, `[x, z]` in cells, inclusive.
    pub min_footprint: [u32; 2],
    /// Largest interior footprint, `[x, z]` in cells, inclusive.
    pub max_footprint: [u32; 2],
    /// Least interior clearance, in cells.
    pub min_clearance: u32,
    /// Nominal blocks of route a body walks crossing a place of this class —
    /// the per-leg length the pacing projection sums over a critical path
    /// (spec-0049 §3.3).
    ///
    /// The code that reads it is not named here on purpose: a `DW` number in a
    /// source comment is a code as far as `tools/check-dw-codes.py` is
    /// concerned, and one whose check lands two rounds from now has no catalog
    /// row to match, so naming it early reds the docs job on a rule nothing has
    /// written yet.
    pub nominal_traverse_blocks: u32,
}

/// The kit grid: the quantum box extents are multiples of, and the datum
/// convention that fixes what a declared `y` means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Grid {
    /// The footprint quantum `q`, in blocks.
    pub quantum: u32,
    /// The axes extents are quantized on. Vertical extent is not quantized:
    /// storey heights are their own entries and a box's height follows them.
    pub axes: [&'static str; 2],
    /// What a box's declared datum `y` names. `floor-surface`: the walk plane is
    /// at `y`, and whatever stands in the box later puts its own floor there.
    pub datum: &'static str,
}

/// One entry's value. `untagged`, so the export reads as the number or object it
/// is rather than as a wrapper a consumer has to unpick.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MetricValue {
    /// A count of cells, blocks or ticks.
    Count(u32),
    /// A measurement that is not a whole number of anything.
    Number(f64),
    /// A yes/no fact.
    Flag(bool),
    /// A named seam opening.
    Opening(Opening),
    /// A named stair pitch.
    Pitch(Pitch),
    /// A rung of the size-class ladder.
    SizeClass(SizeClass),
    /// The kit grid.
    Grid(Grid),
}

/// One player metric: a fact of the pinned game.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayerEntry {
    /// The value.
    pub value: MetricValue,
    /// What the value counts (`blocks`, `sixteenths`, `ticks`, …), or `none`.
    pub unit: &'static str,
    /// Where the number came from.
    pub provenance: Provenance,
    /// One sentence: the constant this **is**, the vanilla rule it states, or
    /// the arithmetic it was derived by.
    pub note: &'static str,
}

/// One building metric: a standard this project fixes.
///
/// The value is deliberately not a public field. [`BuildingEntry::value`] is the
/// only way to read it and it takes `&mut `[`Reads`], so nothing can rest a
/// verdict on an uncalibrated standard without saying that it did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuildingEntry {
    /// The entry's own key, carried so a read records itself without the caller
    /// having to repeat the name it looked up under.
    key: &'static str,
    value: MetricValue,
    /// What the value counts, or `none`.
    pub unit: &'static str,
    /// Where the number came from. Orthogonal to `calibrated`: a pitch's
    /// geometry can be derived from vanilla blocks while the judgement that it
    /// is *the* standard is still unwalked.
    pub provenance: Provenance,
    /// Whether the metrics gym's walk has ruled on this value. `false` on every
    /// entry at this version.
    pub calibrated: bool,
    /// One sentence: what the number is for and what the gym is being asked to
    /// decide about it.
    pub note: &'static str,
}

impl BuildingEntry {
    /// Read the value, recording the read.
    ///
    /// The `&mut `[`Reads`] is the whole mechanism: [`Metrics::notice`] reports
    /// exactly the uncalibrated entries a run actually consumed, so `DW0813`
    /// cannot be forgotten by a check that reads one and can never fire over a
    /// standard nothing looked at.
    #[must_use]
    pub fn value(&self, reads: &mut Reads) -> &MetricValue {
        reads.record(self);
        &self.value
    }

    /// The value with no read recorded — for rendering the table, never for
    /// deciding anything.
    ///
    /// Reachable only inside this crate, and used at exactly one site: the
    /// export, which reports the table rather than resting a verdict on it. A
    /// serialization is not a verdict, and counting it as one would put every
    /// entry in every `DW0813` line and make the code mean nothing.
    pub(crate) fn value_for_display(&self) -> &MetricValue {
        &self.value
    }

    /// The entry's key.
    #[must_use]
    pub fn key(&self) -> &'static str {
        self.key
    }
}

/// The building metrics a run's verdicts have read.
///
/// Deterministic (ADR-0006): a `BTreeSet` of `&'static str`, so the order a
/// `DW0813` line names them in is the table's own order and not the order the
/// checks happened to run in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reads {
    read: BTreeSet<&'static str>,
    provisional: BTreeSet<&'static str>,
}

impl Reads {
    /// A ledger nothing has read through yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn record(&mut self, entry: &BuildingEntry) {
        self.read.insert(entry.key);
        if !entry.calibrated {
            self.provisional.insert(entry.key);
        }
    }

    /// How many building metrics this run's verdicts read, and how many of those
    /// the gym has not walked.
    #[must_use]
    pub fn binding(&self) -> ReadBinding {
        ReadBinding {
            read: self.read.len(),
            provisional: self.provisional.len(),
        }
    }

    /// The uncalibrated entries read, in table order.
    #[must_use]
    pub fn provisional(&self) -> Vec<&'static str> {
        self.provisional.iter().copied().collect()
    }
}

/// What a run's building-metric reads bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReadBinding {
    /// Building metrics read.
    pub read: usize,
    /// Of those, entries whose `calibrated` is false.
    pub provisional: usize,
}

/// A name a document wrote that this table does not define — `DW0812`'s payload.
///
/// It carries the defined set as well as the bad name, because the author's next
/// action is choosing a real one and a refusal that only says *no* sends them to
/// read the compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownMetric {
    /// The kind of entry the document was naming (`size class`, `opening`, …).
    pub kind: &'static str,
    /// The name it wrote.
    pub named: String,
    /// Every name this table does define of that kind, in table order.
    pub defined: Vec<&'static str>,
}

impl UnknownMetric {
    /// The `DW0812` refusal, located by the caller.
    #[must_use]
    pub fn diagnostic(&self, stage: &str, path: &str) -> Diagnostic {
        let defined = if self.defined.is_empty() {
            "nothing".to_string()
        } else {
            self.defined.join(", ")
        };
        Diagnostic::error(
            DW_METRIC_UNKNOWN,
            stage,
            path,
            format!(
                "the metrics table defines no {kind} called `{named}`. The table is \
                 the single authority for this vocabulary, so a name it does not \
                 define cannot compile and no check downstream has to cope with one. \
                 Defined {kind}s: {defined}. Run `delvec metrics` for the whole \
                 table, including what each entry is for.",
                kind = self.kind,
                named = self.named,
            ),
        )
    }
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

/// The metrics standard, as exported and as read.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Metrics {
    /// The table's revision.
    pub metrics_version: u32,
    /// The Minecraft version the player half states facts about.
    pub mc_version: &'static str,
    /// Facts of the pinned game, in key order.
    pub player: BTreeMap<&'static str, PlayerEntry>,
    /// Standards this project fixes, in key order.
    pub building: BTreeMap<&'static str, BuildingEntry>,
}

/// Kinds of building entry a document can name, for [`Metrics::resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    /// A seam opening (`opening.<name>`).
    Opening,
    /// A stair pitch (`pitch.<name>`).
    Pitch,
    /// A rung of the size-class ladder (`size-class.<name>`).
    SizeClass,
    /// A storey height (`storey.<name>`).
    Storey,
}

impl MetricKind {
    /// The key prefix entries of this kind carry.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            MetricKind::Opening => "opening.",
            MetricKind::Pitch => "pitch.",
            MetricKind::SizeClass => "size-class.",
            MetricKind::Storey => "storey.",
        }
    }

    /// What the kind is called in a refusal.
    #[must_use]
    pub fn noun(self) -> &'static str {
        match self {
            MetricKind::Opening => "seam opening",
            MetricKind::Pitch => "stair pitch",
            MetricKind::SizeClass => "size class",
            MetricKind::Storey => "storey height",
        }
    }
}

fn player(
    value: MetricValue,
    unit: &'static str,
    provenance: Provenance,
    note: &'static str,
) -> PlayerEntry {
    PlayerEntry {
        value,
        unit,
        provenance,
        note,
    }
}

fn building(
    key: &'static str,
    value: MetricValue,
    unit: &'static str,
    provenance: Provenance,
    note: &'static str,
) -> (&'static str, BuildingEntry) {
    (
        key,
        BuildingEntry {
            key,
            value,
            unit,
            provenance,
            // Every entry lands uncalibrated. The gym walk flips it, one entry
            // at a time, and that walk is the only thing that may.
            calibrated: false,
            note,
        },
    )
}

impl Metrics {
    /// The table.
    ///
    /// Rebuilt per call rather than held in a `static`, because the values are a
    /// handful of maps and a `static` would need interior mutability the moment
    /// the gym's walk starts flipping `calibrated`. Deterministic by
    /// construction: `BTreeMap`, no clock, no environment.
    #[must_use]
    pub fn table() -> Self {
        let player_entries: Vec<(&'static str, PlayerEntry)> = vec![
            (
                "body.width",
                player(
                    MetricValue::Number(PLAYER_WIDTH),
                    "blocks",
                    Provenance::EngineConstant,
                    "The standing collision box is 0.6 wide; `dsl::metrics::PLAYER_WIDTH` \
                     is the definition `compiler::nav`'s entity dims table and \
                     `compiler::crosshair` both import.",
                ),
            ),
            (
                "body.height",
                player(
                    MetricValue::Number(PLAYER_HEIGHT),
                    "blocks",
                    Provenance::EngineConstant,
                    "The standing collision box is 1.8 tall; the same definition the \
                     entity dims table reads for `minecraft:player`.",
                ),
            ),
            (
                "body.crouched-height",
                player(
                    MetricValue::Number(PLAYER_CROUCHED_HEIGHT),
                    "blocks",
                    Provenance::VanillaRule,
                    "A sneaking player's collision box is 1.5 tall, which is what lets a \
                     body pass under a cell a standing one cannot. No engine constant \
                     held this before the table, and nothing in this workspace has \
                     measured it on a running server.",
                ),
            ),
            (
                "body.eye-height",
                player(
                    MetricValue::Number(PLAYER_EYE_HEIGHT),
                    "blocks",
                    Provenance::EngineConstant,
                    "The eye sits 1.62 above the floor of the cell the body stands in. \
                     This is the definition the render plan, the viewer page and the \
                     creator overlay all import, having each declared their own before.",
                ),
            ),
            (
                "body.max-health",
                player(
                    MetricValue::Number(PLAYER_MAX_HEALTH),
                    "half-hearts",
                    Provenance::EngineConstant,
                    "Twenty points of health, the definition `compiler::combat` imports \
                     for its winnability arithmetic.",
                ),
            ),
            (
                "step.walk-up",
                player(
                    MetricValue::Count(MAX_AUTO_STEP_16 as u32),
                    "sixteenths",
                    Provenance::EngineConstant,
                    "The largest rise a walker crosses without jumping: vanilla's player \
                     `maxUpStep` is 0.6 blocks and 9/16 is the largest sixteenth under \
                     it. `compiler::nav` imports this as its step rule's walk-up budget.",
                ),
            ),
            (
                "step.jump-rise",
                player(
                    MetricValue::Count(MAX_JUMP_RISE_16 as u32),
                    "sixteenths",
                    Provenance::EngineConstant,
                    "The largest rise a walker reaches by jumping: the apex is ≈1.2522 \
                     blocks, so 20/16 is reachable and 21/16 is not. `compiler::nav` \
                     imports this, and it is why a slab-to-full-block step-up of 1.5 is \
                     the impossible move it is.",
                ),
            ),
            (
                "step.block",
                player(
                    MetricValue::Count(FULL_16 as u32),
                    "sixteenths",
                    Provenance::EngineConstant,
                    "A full block in the sixteenths the step rule is denominated in, so \
                     that no comparison in the navigation model is a float.",
                ),
            ),
            (
                "jump.airborne",
                player(
                    MetricValue::Number(JUMP_AIRBORNE_TICKS),
                    "ticks",
                    Provenance::VanillaRule,
                    "A jump is airborne about twelve ticks, apex to landing. This was \
                     prose in the navigation model's elevation-weight derivation and \
                     nothing could read it; the table is where it becomes data.",
                ),
            ),
            (
                "walk.speed",
                player(
                    MetricValue::Number(WALK_SPEED_BLOCKS_PER_SECOND),
                    "blocks/second",
                    Provenance::VanillaRule,
                    "A walking player covers 4.317 blocks a second on the flat; \
                     sprinting is faster and is not the pacing basis, because a route \
                     nobody has learnt is walked.",
                ),
            ),
            (
                "walk.ticks-per-block",
                player(
                    MetricValue::Number(walk_ticks_per_block()),
                    "ticks",
                    Provenance::Derived,
                    "Twenty ticks a second over 4.317 blocks a second. Against the \
                     twelve airborne ticks of a jump this is what makes a block of \
                     climb cost about two and a half blocks of walking, which is where \
                     the navigation model's elevation weight of two comes from.",
                ),
            ),
            (
                "fluid.passable",
                player(
                    MetricValue::Flag(false),
                    "none",
                    Provenance::EngineConstant,
                    "Water and lava are impassable to every proof in this engine and are \
                     never floor either: a body cannot stand on a fluid surface, so the \
                     two sets are disjoint and both gate standability.",
                ),
            ),
            (
                "fall.damage-onset",
                player(
                    MetricValue::Number(FALL_DAMAGE_ONSET_BLOCKS),
                    "blocks",
                    Provenance::VanillaRule,
                    "Fall damage is `ceil(distance − 3)` points, so a three-block fall is \
                     free and a four-block fall costs one. Stated from the vanilla \
                     damage rule; this repository has not measured it on a running \
                     server.",
                ),
            ),
            (
                "fall.unarmoured-survivable",
                player(
                    MetricValue::Number(unarmoured_survivable_fall_blocks()),
                    "blocks",
                    Provenance::Derived,
                    "`ceil(distance − 3) < 20` holds up to 22 blocks, which lands a \
                     full-health unarmoured body on one half-heart; 23 deals twenty and \
                     kills. The survivable ceiling is a function of health and armour, \
                     and this is its unarmoured, full-health case — the physical bound \
                     the designed-drop policy is deliberately tighter than.",
                ),
            ),
            (
                "passable.width",
                player(
                    MetricValue::Count(passable_width_cells()),
                    "cells",
                    Provenance::Derived,
                    "`ceil(0.6)`: one cell is the narrowest a standing body fits \
                     through. This is a fact, not a standard — no walk can change it, \
                     and it is the floor the designed corridor minimum may never be \
                     chosen below.",
                ),
            ),
            (
                "passable.clearance",
                player(
                    MetricValue::Count(passable_clearance_cells()),
                    "cells",
                    Provenance::Derived,
                    "`ceil(1.8)`: two cells is the lowest a standing body passes under. \
                     Like the width beside it this is a fact rather than a standard, \
                     which is why neither carries a calibration flag.",
                ),
            ),
        ];

        let building_entries: Vec<(&'static str, BuildingEntry)> = vec![
            building(
                "grid",
                MetricValue::Grid(Grid {
                    quantum: 4,
                    axes: ["x", "z"],
                    datum: "floor-surface",
                }),
                "blocks",
                Provenance::Provisional,
                "The footprint quantum every site-plan box's horizontal extents are \
                 multiples of, and the datum convention: a box's floor SURFACE is at \
                 its declared y, and whatever stands in the box later puts its walk \
                 plane there. Four is a seed and nothing in the existing piece library \
                 argues for it — the cave tileset is odd on every axis and the keep \
                 tileset is even but not quartered — so what the gym is being asked is \
                 whether a quantum this fine buys anything a coarser one would not.",
            ),
            building(
                "corridor.min-width",
                MetricValue::Count(2),
                "cells",
                Provenance::Provisional,
                "The narrowest a designed corridor may be. One cell is passable and \
                 reads as a crawlspace; two lets two bodies pass and is the seed. The \
                 gym walks one, two and three side by side and the walk decides.",
            ),
            building(
                "corridor.min-clearance",
                MetricValue::Count(3),
                "cells",
                Provenance::Provisional,
                "The lowest a designed corridor's ceiling may be. Two cells is passable \
                 and puts the ceiling on the walker's head; three is the seed. The gym \
                 walks two, three and four.",
            ),
            building(
                "opening.door",
                MetricValue::Opening(Opening {
                    width: 1,
                    height: 2,
                }),
                "cells",
                Provenance::Provisional,
                "The narrow seam: one body at a time, the size of a vanilla door. Seeded \
                 at the smallest opening a standing body passes, so the gym is deciding \
                 whether the tightest legal seam is one anybody wants to walk.",
            ),
            building(
                "opening.arch",
                MetricValue::Opening(Opening {
                    width: 2,
                    height: 3,
                }),
                "cells",
                Provenance::Provisional,
                "The ordinary seam between two interior places: two abreast, headroom \
                 over both.",
            ),
            building(
                "opening.passage",
                MetricValue::Opening(Opening {
                    width: 3,
                    height: 3,
                }),
                "cells",
                Provenance::Derived,
                "The three-by-three doorway the existing jigsaw socket conventions \
                 already standardize on — `cave:socket` and `tk:socket` are both this \
                 opening, so the prefab library has been built against it for as long \
                 as it has existed. Derived from that convention rather than chosen \
                 here, and still uncalibrated: what the gym decides is whether the \
                 convention is right, not what it is.",
            ),
            building(
                "opening.gateway",
                MetricValue::Opening(Opening {
                    width: 5,
                    height: 5,
                }),
                "cells",
                Provenance::Provisional,
                "The broad seam a thing of scenery scale passes: a cart, a barge, a \
                 processional. Seeded wide enough to read as an event from inside the \
                 place it opens onto, which is the judgement the walk is for.",
            ),
            building(
                "pitch.stair",
                MetricValue::Pitch(Pitch {
                    rise: 1,
                    run: 1,
                    step_16: 8,
                    realization: "minecraft:*_stairs",
                }),
                "none",
                Provenance::Derived,
                "One block of rise per block of run, realized in stair blocks. The \
                 geometry is vanilla's: a stair offers its lower half first, so the \
                 body walks two eight-sixteenth steps per block of rise and never \
                 jumps. What is uncalibrated is the comfort judgement — whether a climb \
                 this steep is one a player wants to make repeatedly.",
            ),
            building(
                "pitch.ramp",
                MetricValue::Pitch(Pitch {
                    rise: 1,
                    run: 2,
                    step_16: 8,
                    realization: "minecraft:*_slab + full block",
                }),
                "none",
                Provenance::Derived,
                "One block of rise per two of run, realized as a bottom slab then a full \
                 block. Same eight-sixteenth tread as the stair and half the pitch, so \
                 it is the gentle standard; the run it costs is what the gym weighs it \
                 on.",
            ),
            building(
                "storey.low",
                MetricValue::Count(5),
                "blocks",
                Provenance::Derived,
                "Floor course, three cells of interior clearance, ceiling course. Taken \
                 from the existing cave tileset, every passage and room of which is five \
                 blocks tall, so this is the storey the shipped library already has \
                 rather than a number invented here.",
            ),
            building(
                "storey.standard",
                MetricValue::Count(8),
                "blocks",
                Provenance::Provisional,
                "The storey an interior room of consequence gets: six cells of clearance \
                 between courses. A seed, and the walk is what says whether a room this \
                 tall reads as generous or merely as far away.",
            ),
            building(
                "storey.hall",
                MetricValue::Count(14),
                "blocks",
                Provenance::Provisional,
                "The storey a hall gets, where the height itself is the effect. The seed \
                 is deliberately at the point where volume starts costing walking time \
                 for nothing, because that is the trade the walk has to judge.",
            ),
            building(
                "size-class.alcove",
                MetricValue::SizeClass(SizeClass {
                    min_footprint: [4, 4],
                    max_footprint: [8, 8],
                    min_clearance: 3,
                    nominal_traverse_blocks: 6,
                }),
                "cells",
                Provenance::Provisional,
                "A place a body stands in rather than crosses: a shrine, a landing, a \
                 cell. The smallest rung of the ladder, and the whole ladder's bounds \
                 are seeds — what the walk fixes is where one class stops feeling like \
                 the next.",
            ),
            building(
                "size-class.room",
                MetricValue::SizeClass(SizeClass {
                    min_footprint: [8, 8],
                    max_footprint: [16, 16],
                    min_clearance: 4,
                    nominal_traverse_blocks: 12,
                }),
                "cells",
                Provenance::Provisional,
                "A place with a purpose and something in it: a guardroom, a chapel, a \
                 workshop.",
            ),
            building(
                "size-class.hall",
                MetricValue::SizeClass(SizeClass {
                    min_footprint: [16, 16],
                    max_footprint: [32, 32],
                    min_clearance: 8,
                    nominal_traverse_blocks: 24,
                }),
                "cells",
                Provenance::Provisional,
                "A place a fight or a crowd fits in, and the smallest rung whose height \
                 is doing work of its own.",
            ),
            building(
                "size-class.arena",
                MetricValue::SizeClass(SizeClass {
                    min_footprint: [32, 32],
                    max_footprint: [64, 64],
                    min_clearance: 12,
                    nominal_traverse_blocks: 48,
                }),
                "cells",
                Provenance::Provisional,
                "A place built around one encounter, with room to retreat and re-approach.",
            ),
            building(
                "size-class.expanse",
                MetricValue::SizeClass(SizeClass {
                    min_footprint: [64, 64],
                    max_footprint: [128, 128],
                    min_clearance: 16,
                    nominal_traverse_blocks: 96,
                }),
                "cells",
                Provenance::Provisional,
                "A shore, a valley floor, a cavern — a place whose job is that crossing \
                 it takes time. The rung most at risk of being a big empty room, which \
                 is what the walk is watching for.",
            ),
            building(
                "drop.max-designed-rise",
                MetricValue::Count(5),
                "blocks",
                Provenance::Provisional,
                "The deepest fall a designed one-way drop edge may declare. A policy \
                 cap, not a physical one: the unarmoured survivable fall beside it in \
                 the player half is 22 blocks, and this is far tighter on purpose, \
                 because a drop is a topology decision and should not also be a health \
                 decision. Five costs two of twenty at full health, which is the seed \
                 the walk argues with.",
            ),
            building(
                "pacing.route-blocks-per-minute",
                MetricValue::Count(60),
                "blocks/minute",
                Provenance::Provisional,
                "Blocks of route a party gets through per minute of play, once looking, \
                 fighting and backtracking are in it. Carried with NO THRESHOLD \
                 anywhere until the first walked blockout and the first full playtest \
                 calibrate it: a threshold on a number this uncertain would be defending \
                 nothing. Its upper bound is the pure-walk figure beside it, which no \
                 party achieves.",
            ),
            building(
                "pacing.walk-only-blocks-per-minute",
                MetricValue::Count((WALK_SPEED_BLOCKS_PER_SECOND * 60.0) as u32),
                "blocks/minute",
                Provenance::Derived,
                "Walking speed times sixty: what a body covers doing nothing but \
                 walking in a straight line. It exists so the route coefficient above \
                 has a ceiling that is a fact rather than another guess, and so the \
                 ratio between them is the thing the playtest actually measures.",
            ),
        ];

        Metrics {
            metrics_version: METRICS_VERSION,
            mc_version: crate::blocks::MC_VERSION,
            player: player_entries.into_iter().collect(),
            building: building_entries.into_iter().collect(),
        }
    }

    /// Resolve a name a document wrote to its building entry.
    ///
    /// This is the **only** path from a name to an entry, and it is what makes
    /// the table the single authority rather than a suggestion: a name it does
    /// not define cannot be resolved, so it cannot compile, so no check
    /// downstream ever meets one. A second lookup written beside this one would
    /// be a second authority, which is why the map itself is private.
    ///
    /// # Errors
    ///
    /// [`UnknownMetric`], which the caller turns into `DW0812` with its own
    /// stage and path.
    pub fn resolve(&self, kind: MetricKind, named: &str) -> Result<&BuildingEntry, UnknownMetric> {
        let key = format!("{}{}", kind.prefix(), named);
        self.building
            .get(key.as_str())
            .ok_or_else(|| UnknownMetric {
                kind: kind.noun(),
                named: named.to_string(),
                defined: self.names_of(kind),
            })
    }

    /// Every name defined for a kind, in table order.
    #[must_use]
    pub fn names_of(&self, kind: MetricKind) -> Vec<&'static str> {
        self.building
            .keys()
            .filter_map(|k| k.strip_prefix(kind.prefix()))
            .collect()
    }

    /// The `DW0813` notice for a run, or `None` when no verdict rested on an
    /// unwalked standard.
    ///
    /// `None` at a zero binding is the calibrated end state and not a vacuity:
    /// the line's job is to say that a green rests on a seed, and once the gym
    /// has walked every entry a run reads there is nothing left for it to say.
    /// The distinguishable failure — a run that read NOTHING — is reported by
    /// [`Reads::binding`], which every caller states whether or not this returns
    /// a line.
    #[must_use]
    pub fn notice(&self, reads: &Reads, stage: &str) -> Option<Diagnostic> {
        let provisional = reads.provisional();
        if provisional.is_empty() {
            return None;
        }
        let binding = reads.binding();
        Some(Diagnostic::warning(
            DW_METRIC_PROVISIONAL,
            stage,
            "",
            format!(
                "{n} of the {read} building metric(s) this run read are provisional — \
                 the metrics gym has not walked them, so every verdict above that used \
                 one is resting on a seed rather than on a standard: {names}. The \
                 checks still ran and still refuse; what is unproven is the number they \
                 refused against. Walking the gym is what retires this line, one entry \
                 at a time.",
                n = binding.provisional,
                read = binding.read,
                names = provisional.join(", "),
            ),
        ))
    }

    /// Check the table against itself and against the player half.
    ///
    /// These are verdicts, and they read building metrics through a [`Reads`], so
    /// they are also what gives `DW0813` a live binding at this version: `delvec
    /// metrics` runs them, and the notice names the seeds the consistency verdict
    /// rested on. They are the reason the mechanism is demonstrable now rather
    /// than at the round that adds the documents.
    ///
    /// A violation is an **internal error**, not a diagnostic: the table is
    /// engine data, so an inconsistent one is a defect in this file and not in
    /// anybody's campaign, and there is no author to address a refusal to.
    #[must_use]
    pub fn self_check(&self) -> SelfCheck {
        let mut reads = Reads::new();
        let mut failures: Vec<String> = Vec::new();
        let mut checked = 0usize;

        let floor_w = u64::from(passable_width_cells());
        let floor_h = u64::from(passable_clearance_cells());

        // A designed minimum may never be chosen below the physical floor: the
        // building half's job is comfort, and a standard under the passable
        // width would be a standard nothing can use.
        for (key, floor, what) in [
            ("corridor.min-width", floor_w, "passable.width"),
            ("corridor.min-clearance", floor_h, "passable.clearance"),
        ] {
            let Some(entry) = self.building.get(key) else {
                failures.push(format!("the table defines no `{key}`"));
                continue;
            };
            checked += 1;
            if let MetricValue::Count(n) = entry.value(&mut reads) {
                if u64::from(*n) < floor {
                    failures.push(format!(
                        "`{key}` is {n}, below the `{what}` floor of {floor}"
                    ));
                }
            } else {
                failures.push(format!("`{key}` is not a count"));
            }
        }

        let quantum = match self.building.get("grid").map(|e| e.value(&mut reads)) {
            Some(MetricValue::Grid(g)) => {
                checked += 1;
                if g.quantum == 0 {
                    failures.push("the kit grid's quantum is zero".to_string());
                }
                g.quantum
            }
            _ => {
                failures.push("the table defines no kit `grid`".to_string());
                1
            }
        };

        for (key, entry) in &self.building {
            match entry.value(&mut reads) {
                MetricValue::Opening(o) => {
                    checked += 1;
                    if u64::from(o.width) < floor_w || u64::from(o.height) < floor_h {
                        failures.push(format!(
                            "`{key}` is {}×{}, which no standing body passes ({floor_w}×{floor_h} \
                             is the floor)",
                            o.width, o.height
                        ));
                    }
                }
                MetricValue::Pitch(p) => {
                    checked += 1;
                    if p.step_16 > MAX_AUTO_STEP_16 {
                        failures.push(format!(
                            "`{key}` presents a tread of {}/16, over the {MAX_AUTO_STEP_16}/16 \
                             walk-up budget, so it is climbed by jumping and is not a standard \
                             pitch",
                            p.step_16
                        ));
                    }
                    if p.rise == 0 || p.run == 0 {
                        failures.push(format!("`{key}` has a zero rise or run"));
                    }
                }
                MetricValue::SizeClass(c) => {
                    checked += 1;
                    for (axis, lo, hi) in [
                        ("x", c.min_footprint[0], c.max_footprint[0]),
                        ("z", c.min_footprint[1], c.max_footprint[1]),
                    ] {
                        if lo > hi {
                            failures
                                .push(format!("`{key}` has a {axis} minimum above its maximum"));
                        }
                        if lo % quantum != 0 || hi % quantum != 0 {
                            failures.push(format!(
                                "`{key}` bounds its {axis} footprint at {lo}..{hi}, which is not \
                                 on the kit grid's quantum of {quantum}"
                            ));
                        }
                    }
                    if u64::from(c.min_clearance) < floor_h {
                        failures.push(format!(
                            "`{key}` allows a clearance of {}, under the passable floor of \
                             {floor_h}",
                            c.min_clearance
                        ));
                    }
                    if c.nominal_traverse_blocks == 0 {
                        failures.push(format!("`{key}` has a nominal traverse of zero"));
                    }
                }
                _ => {}
            }
        }

        for (key, floor) in [
            ("storey.low", floor_h + 2),
            ("storey.standard", floor_h + 2),
            ("storey.hall", floor_h + 2),
        ] {
            let Some(entry) = self.building.get(key) else {
                failures.push(format!("the table defines no `{key}`"));
                continue;
            };
            checked += 1;
            if let MetricValue::Count(n) = entry.value(&mut reads)
                && u64::from(*n) < floor
            {
                failures.push(format!(
                    "`{key}` is {n} blocks, which leaves no passable interior between a \
                     floor course and a ceiling course ({floor} is the floor)"
                ));
            }
        }

        // The policy cap is deliberately tighter than the physical one. A cap
        // that reached the survivability ceiling would not be a policy.
        if let Some(entry) = self.building.get("drop.max-designed-rise") {
            checked += 1;
            if let MetricValue::Count(n) = entry.value(&mut reads) {
                let physical = unarmoured_survivable_fall_blocks();
                if f64::from(*n) >= physical {
                    failures.push(format!(
                        "`drop.max-designed-rise` is {n} blocks, at or past the unarmoured \
                         survivable fall of {physical}, so it is not a policy cap at all"
                    ));
                }
            }
        }

        // A party cannot out-pace a body walking in a straight line.
        if let (Some(route), Some(walk)) = (
            self.building.get("pacing.route-blocks-per-minute"),
            self.building.get("pacing.walk-only-blocks-per-minute"),
        ) {
            checked += 1;
            if let (MetricValue::Count(r), MetricValue::Count(w)) =
                (route.value(&mut reads), walk.value(&mut reads))
                && r > w
            {
                failures.push(format!(
                    "`pacing.route-blocks-per-minute` is {r}, over the pure-walk ceiling of {w}"
                ));
            }
        }

        SelfCheck {
            binding: SelfCheckBinding {
                invariants: checked,
                entries: self.building.len(),
                reads: reads.binding(),
            },
            reads,
            failures,
        }
    }
}

/// What [`Metrics::self_check`] examined and what it found.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfCheck {
    /// The ledger the verdicts read through, for [`Metrics::notice`].
    pub reads: Reads,
    /// What the run bound to. Stated whether or not anything failed, because a
    /// check that examined nothing is a finding and not a pass.
    pub binding: SelfCheckBinding,
    /// Inconsistencies, each a whole sentence. Non-empty is an internal error.
    pub failures: Vec<String>,
}

/// [`Metrics::self_check`]'s binding count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SelfCheckBinding {
    /// Invariants evaluated.
    pub invariants: usize,
    /// Building entries in the table.
    pub entries: usize,
    /// What those invariants read.
    pub reads: ReadBinding,
}

/// The table rendered for export: every entry with its value, unit, provenance
/// and note, plus the halves' own counts.
///
/// Separate from [`Metrics`]'s own `Serialize` because the building half's value
/// is private — an export is the one consumer that reads a number without
/// resting a verdict on it.
#[must_use]
pub fn export(metrics: &Metrics) -> serde_json::Value {
    let mut player = serde_json::Map::new();
    for (k, e) in &metrics.player {
        player.insert((*k).to_string(), serde_json::json!(e));
    }
    let mut building = serde_json::Map::new();
    let mut uncalibrated = 0usize;
    for (k, e) in &metrics.building {
        if !e.calibrated {
            uncalibrated += 1;
        }
        building.insert(
            (*k).to_string(),
            serde_json::json!({
                "value": e.value_for_display(),
                "unit": e.unit,
                "provenance": e.provenance,
                "calibrated": e.calibrated,
                "note": e.note,
            }),
        );
    }
    serde_json::json!({
        "metrics_version": metrics.metrics_version,
        "mc_version": metrics.mc_version,
        "counts": {
            "player": metrics.player.len(),
            "building": metrics.building.len(),
            "uncalibrated": uncalibrated,
        },
        "player": player,
        "building": building,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_consistent_with_itself_and_with_the_player_half() {
        let m = Metrics::table();
        let check = m.self_check();
        assert!(
            check.failures.is_empty(),
            "the shipped metrics table is inconsistent: {:?}",
            check.failures
        );
        assert!(
            check.binding.invariants > 0,
            "a self-check that evaluated nothing is vacuous, not a pass"
        );
    }

    #[test]
    fn every_building_entry_lands_uncalibrated() {
        let m = Metrics::table();
        assert!(!m.building.is_empty(), "the building half is empty");
        for (k, e) in &m.building {
            assert!(
                !e.calibrated,
                "`{k}` claims to be calibrated, but the metrics gym has not been walked"
            );
        }
    }

    #[test]
    fn a_verdict_that_reads_a_seed_raises_dw0813_naming_it() {
        let m = Metrics::table();
        let check = m.self_check();
        assert!(
            check.binding.reads.provisional > 0,
            "the self-check read no provisional entry, so DW0813 binds to nothing"
        );
        let d = m
            .notice(&check.reads, "metrics")
            .expect("a run that read a seed owes the notice");
        assert_eq!(d.code, "DW0813");
        assert_eq!(d.severity, crate::diagnostic::Severity::Warning);
        for name in check.reads.provisional() {
            assert!(
                d.message.contains(name),
                "the notice must name `{name}`, the seed a verdict rested on"
            );
        }
    }

    #[test]
    fn a_run_that_read_nothing_provisional_gets_no_notice() {
        let m = Metrics::table();
        let reads = Reads::new();
        assert!(m.notice(&reads, "metrics").is_none());
        assert_eq!(reads.binding().read, 0);
    }

    #[test]
    fn an_undefined_name_is_dw0812_and_names_what_is_defined() {
        let m = Metrics::table();
        let err = m
            .resolve(MetricKind::SizeClass, "cathedral")
            .expect_err("`cathedral` is not a size class");
        let d = err.diagnostic("layout-graph", "/nodes/0/size_class");
        assert_eq!(d.code, "DW0812");
        assert!(d.message.contains("cathedral"));
        assert!(d.message.contains("room"), "the defined set is named");
        assert_eq!(d.stage, "layout-graph");
        assert_eq!(d.path, "/nodes/0/size_class");
    }

    #[test]
    fn every_kind_resolves_at_least_one_defined_name() {
        let m = Metrics::table();
        for kind in [
            MetricKind::Opening,
            MetricKind::Pitch,
            MetricKind::SizeClass,
            MetricKind::Storey,
        ] {
            let names = m.names_of(kind);
            assert!(
                !names.is_empty(),
                "{} resolves nothing, so DW0812 would refuse every name",
                kind.noun()
            );
            for n in names {
                assert!(m.resolve(kind, n).is_ok(), "`{n}` does not resolve");
            }
        }
    }

    #[test]
    fn the_derived_player_values_are_the_arithmetic_their_notes_claim() {
        assert!((walk_ticks_per_block() - 20.0 / 4.317).abs() < f64::EPSILON);
        assert!((walk_ticks_per_block() - 4.633).abs() < 0.001);
        assert_eq!(unarmoured_survivable_fall_blocks(), 22.0);
        assert_eq!(passable_width_cells(), 1);
        assert_eq!(passable_clearance_cells(), 2);
    }
}
