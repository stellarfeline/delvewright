//! **The sculk family works** (spec-0100): sensors, shriekers and catalysts
//! enter a map as the vanilla mechanisms they are, pinned at rest, reaching
//! nothing undeclared, and the walk proves them.
//!
//! Four proofs over the assembled block map, and one export:
//!
//! * **At rest** ([`check_rest`]): every acting sculk block of the assembled
//!   world — a generator's piece, `detail`'s, `sculpt`'s, which no admission
//!   audit ran over — and every block a runtime write lays is judged by the one
//!   rest rule, [`delvewright_dsl::blocks::sculk_rest`] (`DW0998`, `DW0999`).
//!   The entry points refuse where the state is typed; this call cannot be
//!   bypassed.
//! * **Containment** ([`prove_reach`], `DW1000`): a sensor's reach is its six
//!   neighbours and, when the cell above it conducts, that cell's five other
//!   neighbours; no reach cell may hold a block that reads a signal. A
//!   calibrated sensor's input cell holds neither a source nor a conductor, so
//!   its frequency filter is open.
//! * **The catalyst** ([`prove_catalysts`], `DW1001`): no cell any body
//!   relation can put a body in lies within integer `distSqr ≤ 64` of a
//!   catalyst.
//! * **The walk's vibrations** ([`Listening::leg_vibrations`]): which sensors
//!   the proven route sets off and which shriekers answer, exported on each
//!   leg of `critical-path-waypoints.json` for the bot to assert.
//!
//! The redstone facts are the pinned jar's (`blockshape::redstone_*`, read from
//! `crates/dsl/data/redstone-1.21.11.tsv`), the body relations the nav model's
//! (`World::body_moves`, `World::mob_moves`); nothing here models either again.
//!
//! # Runtime writes: every possible block, at once
//!
//! A cell a runtime write may lay a block in — a `fill-region`, a `set-block`,
//! a `close-gate`, a timed gate, a laid way, a shortcut seal — can hold its
//! as-placed block or any block any write lays there. The containment proof
//! judges every one of them at once: a reach cell refuses if ANY block it may
//! hold reads a signal, a cell above conducts if ANY block it may hold
//! conducts, and a sensor exists wherever ANY write may lay one. That is every
//! configuration the derived-world sequence passes through and every order of
//! its unforced writes besides, so it can only refuse more.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::blockshape::{
    bare_id, redstone_conductor, redstone_reader, redstone_source, state_value,
};
use delvewright_dsl::{DwCode, ExitTier, Verb};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::failure::Failure;
use crate::compiler::nav::{Footprint, World};
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1000`: **a sculk sensor's redstone neighbourhood is not inert**
    /// (spec-0100 §3.4, §3.5). First arm: a cell of the sensor's reach — its six
    /// neighbours, and the five other neighbours of the cell above it when that
    /// cell conducts — holds a block that reads a signal. Second arm: a
    /// calibrated sensor's input cell holds a signal source or a conductor.
    /// Build tier.
    pub const DW_SENSOR_REACH: DwCode = DwCode::new("DW1000", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1001`: **a sculk catalyst can hear a death** (spec-0100 §3.7): a cell a
    /// body relation can put a body in lies within integer `distSqr ≤ 64` of the
    /// catalyst's cell. Build tier.
    pub const DW_CATALYST_HEARS_DEATH: DwCode = DwCode::new("DW1001", ExitTier::Build);
}

/// A sensor's listener radius (`SculkSensorBlockEntity$VibrationUser`), and a
/// shrieker's and a catalyst's: 8, compared as integer `distSqr ≤ 64`
/// (`EuclideanGameEventListenerRegistry`, spec-0100 §2.6).
pub const LISTENER_RADIUS_SQ: i64 = 64;

/// The radius a sensor is **predicted** within: one block inside the listener
/// radius (`distSqr ≤ 49`), so the bot's deviation from the proven route cannot
/// falsify the prediction (spec-0100 §4.6).
pub const PREDICTED_RADIUS_SQ: i64 = 49;

/// `#minecraft:occludes_vibration_signals` in the pinned jar
/// (`data/minecraft/tags/block/occludes_vibration_signals.json` = `#wool`),
/// expanded through `wool.json`. **Cited.**
pub const OCCLUDES_VIBRATION_SIGNALS_1_21_11: [&str; 16] = [
    "minecraft:white_wool",
    "minecraft:orange_wool",
    "minecraft:magenta_wool",
    "minecraft:light_blue_wool",
    "minecraft:yellow_wool",
    "minecraft:lime_wool",
    "minecraft:pink_wool",
    "minecraft:gray_wool",
    "minecraft:light_gray_wool",
    "minecraft:cyan_wool",
    "minecraft:purple_wool",
    "minecraft:blue_wool",
    "minecraft:brown_wool",
    "minecraft:green_wool",
    "minecraft:red_wool",
    "minecraft:black_wool",
];

/// Whether a block is in `#minecraft:occludes_vibration_signals`.
pub fn occludes_vibrations(name: &str) -> bool {
    let id = bare_id(name);
    OCCLUDES_VIBRATION_SIGNALS_1_21_11
        .iter()
        .any(|w| bare_id(w) == id)
}

/// Whether a block is in `#minecraft:dampens_vibrations` — `#wool` and
/// `#wool_carpets` in the pinned jar, the sixteen colours of each (the carpet
/// tag spells `<colour>_carpet` for every `<colour>_wool`). **Cited**:
/// `VibrationSystem$User.isValidVibration` refuses an event whose affected
/// block state is in this tag, so a step on wool or a wool carpet sets off
/// nothing.
pub fn dampens_vibrations(name: &str) -> bool {
    let id = bare_id(name);
    OCCLUDES_VIBRATION_SIGNALS_1_21_11.iter().any(|w| {
        bare_id(w) == id
            || bare_id(w)
                .strip_suffix("_wool")
                .is_some_and(|colour| id.strip_suffix("_carpet") == Some(colour))
    })
}

/// Which acting sculk block a state is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sculk {
    /// `sculk_sensor`.
    Sensor,
    /// `calibrated_sculk_sensor`.
    Calibrated,
    /// `sculk_shrieker`.
    Shrieker,
    /// `sculk_catalyst`.
    Catalyst,
}

/// The acting sculk block this state is, if any.
pub fn sculk_kind(name: &str) -> Option<Sculk> {
    match bare_id(name) {
        "sculk_sensor" => Some(Sculk::Sensor),
        "calibrated_sculk_sensor" => Some(Sculk::Calibrated),
        "sculk_shrieker" => Some(Sculk::Shrieker),
        "sculk_catalyst" => Some(Sculk::Catalyst),
        _ => None,
    }
}

/// One runtime write a cell may receive: the inclusive box, the block it lays,
/// and what to call it in a message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Write {
    /// The inclusive world box (any corner order).
    pub region: ([i32; 3], [i32; 3]),
    /// The block the write lays.
    pub block: String,
    /// The write, in words: the verb and its document path.
    pub label: String,
}

impl Write {
    fn contains(&self, c: [i32; 3]) -> bool {
        let (a, b) = self.region;
        (0..3).all(|i| a[i].min(b[i]) <= c[i] && c[i] <= a[i].max(b[i]))
    }

    fn cells(&self) -> impl Iterator<Item = [i32; 3]> {
        let (a, b) = self.region;
        let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
        crate::compiler::assembled::region_cells(lo, hi)
    }
}

/// **Every block a runtime write may lay, and where** — the `fill-region`,
/// `set-block` and `close-gate` effects at every root and depth, every laid
/// write the region model carries (a way, a shortcut seal), and both phases of
/// every timed gate. Sorted and deduplicated (ADR-0006). Clears lay air, which
/// no rule here reads, so they are not carried.
pub fn runtime_writes(plan: &Plan) -> Vec<Write> {
    let mut out: BTreeSet<Write> = BTreeSet::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
        let label = format!("`{}` at `{}`", e.verb.tag(), site.path);
        match &e.verb {
            Verb::FillRegion { region, block, .. } => {
                if let Some(r) = plan.zone_box(region) {
                    out.insert(Write {
                        region: r,
                        block: block.clone(),
                        label,
                    });
                }
            }
            Verb::SetBlock { anchor, block, .. } => {
                if let Some(p) = plan.point_any(anchor.as_str()) {
                    out.insert(Write {
                        region: (p, p),
                        block: block.clone(),
                        label,
                    });
                }
            }
            Verb::CloseGate { anchor, .. } => {
                if let Some((from, to, block)) =
                    crate::compiler::plan::gate_region_block_any(&plan.anchors, anchor.as_str())
                {
                    out.insert(Write {
                        region: (from, to),
                        block,
                        label,
                    });
                }
            }
            _ => {}
        }
    });
    for ev in plan.region_events.iter() {
        if let Some(b) = ev.block() {
            out.insert(Write {
                region: ev.region,
                block: b.to_string(),
                label: format!(
                    "a runtime write laying `{b}` at critical-path step {}",
                    ev.fire_step
                ),
            });
        }
    }
    for g in &plan.timed_gates {
        out.insert(Write {
            region: g.gate_region,
            block: g.gate_block.clone(),
            label: format!("timed gate `{}` closed", g.id),
        });
    }
    out.into_iter().collect()
}

/// What [`check_rest`] examined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestBinding {
    /// Acting sculk cells of the assembled world judged.
    pub cells: usize,
    /// Runtime writes judged.
    pub writes: usize,
}

/// **The rest rule over the assembled world and every runtime write**
/// (spec-0100 §4.1): the first fault, naming the cell or the write.
pub fn check_rest(blocks: &BlockMap, writes: &[Write]) -> (RestBinding, Option<Failure>) {
    let mut b = RestBinding::default();
    let mut first: Option<Failure> = None;
    for (c, s) in blocks {
        if sculk_kind(s.as_str()).is_none() {
            continue;
        }
        b.cells += 1;
        if first.is_none()
            && let Err(fault) = delvewright_dsl::blocks::sculk_rest(s.as_str(), None)
        {
            first = Some(Failure {
                code: fault.code(),
                message: format!(
                    "the assembled world holds, at [{}, {}, {}], {fault}. This cell came from a \
                     piece no document typed (a generator's, `detail`'s or `sculpt`'s), so it is \
                     judged here, over the world the build ships",
                    c[0], c[1], c[2]
                ),
            });
        }
    }
    for w in writes {
        if sculk_kind(&w.block).is_none() {
            continue;
        }
        b.writes += 1;
        if first.is_none()
            && let Err(fault) = delvewright_dsl::blocks::sculk_rest(&w.block, None)
        {
            first = Some(Failure {
                code: fault.code(),
                message: format!("{} lays {fault}", w.label),
            });
        }
    }
    (b, first)
}

/// Every block a cell may hold: the as-placed one (air when the map holds
/// none), then every block a runtime write may lay there.
fn possible<'a>(blocks: &'a BlockMap, writes: &'a [Write], c: [i32; 3]) -> Vec<(&'a str, String)> {
    let mut out = vec![(
        blocks.get(&c).map_or("minecraft:air", |s| s.as_str()),
        "in the world as placed".to_string(),
    )];
    for w in writes.iter().filter(|w| w.contains(c)) {
        out.push((w.block.as_str(), format!("laid by {}", w.label)));
    }
    out
}

/// The six face neighbours of `c`, in the fixed order down, up, north, south,
/// west, east (ADR-0006).
fn neighbours(c: [i32; 3]) -> [[i32; 3]; 6] {
    [
        [c[0], c[1] - 1, c[2]],
        [c[0], c[1] + 1, c[2]],
        [c[0], c[1], c[2] - 1],
        [c[0], c[1], c[2] + 1],
        [c[0] - 1, c[1], c[2]],
        [c[0] + 1, c[1], c[2]],
    ]
}

/// A calibrated sensor's input cell: `pos.relative(facing.getOpposite())`
/// (spec-0100 §2.3). `None` for a facing the state does not spell.
pub fn calibrated_input(c: [i32; 3], state: &str) -> Option<[i32; 3]> {
    let facing = state_value(state, "facing").or_else(|| {
        delvewright_dsl::blocks::BlockRegistry::v1_21_11()
            .default_state("minecraft:calibrated_sculk_sensor")
            .and_then(|d| d.get("facing"))
            .map(String::as_str)
    })?;
    Some(match facing {
        "north" => [c[0], c[1], c[2] + 1],
        "south" => [c[0], c[1], c[2] - 1],
        "west" => [c[0] + 1, c[1], c[2]],
        "east" => [c[0] - 1, c[1], c[2]],
        _ => return None,
    })
}

/// What [`prove_reach`] examined.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReachBinding {
    /// Sensors judged (plain and calibrated).
    pub sensors: usize,
    /// Of which calibrated.
    pub calibrated: usize,
    /// Reach cells checked, summed over sensors.
    pub reach_cells: usize,
    /// Shriekers in the world as placed or laid by a write.
    pub shriekers: usize,
    /// Catalysts in the world as placed or laid by a write.
    pub catalysts: usize,
    /// Runtime writes whose blocks were judged beside the world as placed.
    pub writes: usize,
}

impl ReachBinding {
    /// The binding line (spec-0100 §4.3).
    pub fn line(&self) -> String {
        format!(
            "sculk: {} sensor(s) ({} calibrated), {} reach cell(s) checked, {} shrieker(s), {} \
             catalyst(s); {} runtime write(s) judged beside the world as placed",
            self.sensors,
            self.calibrated,
            self.reach_cells,
            self.shriekers,
            self.catalysts,
            self.writes
        )
    }
}

/// Every cell an acting sculk block of `kind` stands in, as placed or laid by
/// a write, with its state.
fn cells_of(blocks: &BlockMap, writes: &[Write], kind: Sculk) -> BTreeMap<[i32; 3], String> {
    let mut out: BTreeMap<[i32; 3], String> = blocks
        .iter()
        .filter(|(_, s)| sculk_kind(s.as_str()) == Some(kind))
        .map(|(c, s)| (*c, s.as_str().to_string()))
        .collect();
    for w in writes.iter().filter(|w| sculk_kind(&w.block) == Some(kind)) {
        for c in w.cells() {
            out.entry(c).or_insert_with(|| w.block.clone());
        }
    }
    out
}

/// **`DW1000`: a sensor's power reaches nothing** (spec-0100 §4.3), over the
/// world as placed and every block a runtime write may lay. The binding is
/// returned beside the first refusal.
pub fn prove_reach(blocks: &BlockMap, writes: &[Write]) -> (ReachBinding, Option<Failure>) {
    let plain = cells_of(blocks, writes, Sculk::Sensor);
    let calibrated = cells_of(blocks, writes, Sculk::Calibrated);
    let mut b = ReachBinding {
        sensors: plain.len() + calibrated.len(),
        calibrated: calibrated.len(),
        shriekers: cells_of(blocks, writes, Sculk::Shrieker).len(),
        catalysts: cells_of(blocks, writes, Sculk::Catalyst).len(),
        writes: writes.len(),
        ..ReachBinding::default()
    };
    let mut first: Option<Failure> = None;
    let sensors = plain
        .iter()
        .map(|(c, s)| (*c, s, false))
        .chain(calibrated.iter().map(|(c, s)| (*c, s, true)));
    let mut all: Vec<([i32; 3], &String, bool)> = sensors.collect();
    all.sort();
    for (c, state, is_calibrated) in all {
        let above = [c[0], c[1] + 1, c[2]];
        let conducting = possible(blocks, writes, above)
            .into_iter()
            .find(|(n, _)| redstone_conductor(n));
        let mut reach: Vec<([i32; 3], Option<String>)> =
            neighbours(c).into_iter().map(|n| (n, None)).collect();
        if let Some((n, how)) = &conducting {
            for m in neighbours(above) {
                if m != c {
                    reach.push((
                        m,
                        Some(format!(
                            "a neighbour of the cell above it, [{}, {}, {}], which holds the \
                             conductor `{n}` {how} and so hands the sensor's strong upward \
                             signal on",
                            above[0], above[1], above[2]
                        )),
                    ));
                }
            }
        }
        b.reach_cells += reach.len();
        if first.is_none() {
            for (r, via) in &reach {
                if let Some((n, how)) = possible(blocks, writes, *r)
                    .into_iter()
                    .find(|(n, _)| redstone_reader(n))
                {
                    let via = via
                        .clone()
                        .unwrap_or_else(|| "a face neighbour".to_string());
                    first = Some(Failure {
                        code: DW_SENSOR_REACH,
                        message: format!(
                            "the sculk sensor `{state}` at [{}, {}, {}] powers [{}, {}, {}] — {via} \
                             — which holds `{n}` {how}, a block that reads a redstone signal \
                             (the pinned jar's redstone table). A sensor fires on every footstep \
                             within 8 blocks, so that block would change whenever anyone walks \
                             past: a mechanism nothing declared. Nothing in the DSL consumes \
                             redstone, so a sensor's power must reach nothing: move the sensor or \
                             the block, or put a block that reads no signal in the cell — and \
                             above the sensor, a block that does not conduct (glass, leaves) \
                             keeps its strong signal from travelling",
                            c[0], c[1], c[2], r[0], r[1], r[2]
                        ),
                    });
                    break;
                }
            }
        }
        if first.is_none()
            && is_calibrated
            && let Some(input) = calibrated_input(c, state)
            && let Some((n, how)) = possible(blocks, writes, input)
                .into_iter()
                .find(|(n, _)| redstone_source(n) || redstone_conductor(n))
        {
            first = Some(Failure {
                code: DW_SENSOR_REACH,
                message: format!(
                    "the calibrated sculk sensor `{state}` at [{}, {}, {}] has `{n}` {how} on \
                     its input side, [{}, {}, {}], which is a {} — the signal there would set \
                     the sensor's frequency filter, so it would hear only one frequency instead \
                     of every vibration. A declared frequency needs a signal of exact strength, \
                     which this engine does not admit; keep the input cell free of sources and \
                     conductors (air, glass, a slab) so the filter stays open",
                    c[0],
                    c[1],
                    c[2],
                    input[0],
                    input[1],
                    input[2],
                    if redstone_source(n) {
                        "signal source"
                    } else {
                        "redstone conductor"
                    }
                ),
            });
        }
    }
    (b, first)
}

/// Integer-cell squared distance (`BlockPos.distSqr`).
pub fn dist_sq(a: [i32; 3], b: [i32; 3]) -> i64 {
    (0..3)
        .map(|i| {
            let d = i64::from(a[i]) - i64::from(b[i]);
            d * d
        })
        .sum()
}

/// What [`prove_catalysts`] examined.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalystBinding {
    /// Catalysts judged.
    pub catalysts: usize,
    /// Cells the body relation puts a player in.
    pub body_cells: usize,
    /// Cells the mob relation puts a wave member in.
    pub mob_cells: usize,
    /// The nearest body cell to any catalyst: `(catalyst, cell, distSqr)`.
    pub nearest: Option<([i32; 3], [i32; 3], i64)>,
}

impl CatalystBinding {
    /// The binding line (spec-0100 §4.4).
    pub fn line(&self) -> String {
        match self.nearest {
            Some((_, cell, d)) => format!(
                "sculk: {} catalyst(s), nearest body cell at distSqr {d} ({:?}); {} body cell(s), \
                 {} mob cell(s) examined",
                self.catalysts, cell, self.body_cells, self.mob_cells
            ),
            None => format!(
                "sculk: {} catalyst(s), no body cell to measure against; {} body cell(s), {} mob \
                 cell(s) examined",
                self.catalysts, self.body_cells, self.mob_cells
            ),
        }
    }
}

/// The closure of a movement relation from `roots`.
fn closure(
    roots: impl IntoIterator<Item = [i32; 3]>,
    moves: impl Fn([i32; 3]) -> Vec<[i32; 3]>,
) -> BTreeSet<[i32; 3]> {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    for r in roots {
        if seen.insert(r) {
            queue.push_back(r);
        }
    }
    while let Some(c) = queue.pop_front() {
        for n in moves(c) {
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

/// **`DW1001`: a catalyst hears no death** (spec-0100 §4.4). `world` is the
/// world bodies move in (the caller lifts the exclusions and opens what a hand
/// opens); `body_roots` are the standable cells the campaign puts the party
/// at; `mob_roots` each wave stack's seats with its footprint. The body cells
/// are the closure of [`World::body_moves`] from the first, united with the
/// closure of [`World::mob_moves`] from the second.
pub fn prove_catalysts(
    catalysts: &[[i32; 3]],
    world: &World,
    body_roots: &[[i32; 3]],
    mob_roots: &[(Vec<[i32; 3]>, Footprint)],
) -> (CatalystBinding, Option<Failure>) {
    let mut b = CatalystBinding {
        catalysts: catalysts.len(),
        ..CatalystBinding::default()
    };
    if catalysts.is_empty() {
        return (b, None);
    }
    let bodies = closure(body_roots.iter().copied(), |c| world.body_moves(c));
    b.body_cells = bodies.len();
    let mut mobs: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (seats, fp) in mob_roots {
        mobs.extend(closure(seats.iter().copied(), |c| world.mob_moves(c, fp)));
    }
    b.mob_cells = mobs.len();
    let mut first: Option<Failure> = None;
    for &cat in catalysts {
        let near = bodies
            .iter()
            .map(|c| (dist_sq(cat, *c), *c, "a player"))
            .chain(mobs.iter().map(|c| (dist_sq(cat, *c), *c, "a wave member")))
            .min();
        let Some((d, cell, who)) = near else { continue };
        if b.nearest.is_none_or(|(_, _, best)| d < best) {
            b.nearest = Some((cat, cell, d));
        }
        if first.is_none() && d <= LISTENER_RADIUS_SQ {
            first = Some(Failure {
                code: DW_CATALYST_HEARS_DEATH,
                message: format!(
                    "the sculk catalyst at [{}, {}, {}] can hear a death: {who} can stand at \
                     [{}, {}, {}], distSqr {d} from it, within its listener range (distSqr ≤ \
                     {LISTENER_RADIUS_SQ}). A catalyst blooms on every death in range and, on one \
                     that drops experience, rewrites the blocks around it into sculk by the \
                     game's random — the world the proofs judged would change mid-fight. Move it \
                     so every cell a body can reach is more than 8 blocks away (distSqr > \
                     {LISTENER_RADIUS_SQ}): sealed below a floor, behind a wall a body cannot \
                     pass",
                    cat[0], cat[1], cat[2], cell[0], cell[1], cell[2]
                ),
            });
        }
    }
    (b, first)
}

/// One vibration a leg is predicted to make: the sensor, and the shriekers
/// that answer it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vibration {
    /// The sensor's cell.
    pub sensor: [i32; 3],
    /// The shriekers that answer, in cell order.
    pub shriekers: Vec<[i32; 3]>,
}

/// The listening devices of the world as placed, for the waypoint export.
#[derive(Debug, Clone, Default)]
pub struct Listening {
    /// Sensors no runtime write touches (plain and calibrated), with their
    /// listener radius squared.
    sensors: Vec<([i32; 3], i64)>,
    /// Shriekers no runtime write touches.
    shriekers: Vec<[i32; 3]>,
    /// Sensors or shriekers a runtime write touches: never predicted.
    rewritten: usize,
    /// Cells holding a block in `#occludes_vibration_signals`.
    occluders: BTreeSet<[i32; 3]>,
    /// Cells holding a block in `#dampens_vibrations`.
    dampeners: BTreeSet<[i32; 3]>,
}

impl Listening {
    /// Read the devices off the world as placed. A device any runtime write
    /// covers is not predicted: what stands there on a given leg is the
    /// write's, not the placement's.
    pub fn of(blocks: &BlockMap, writes: &[Write]) -> Listening {
        let mut l = Listening::default();
        for (c, s) in blocks {
            let n = s.as_str();
            if occludes_vibrations(n) {
                l.occluders.insert(*c);
            }
            if dampens_vibrations(n) {
                l.dampeners.insert(*c);
            }
            let Some(kind) = sculk_kind(n) else { continue };
            if matches!(kind, Sculk::Catalyst) {
                continue;
            }
            if writes.iter().any(|w| w.contains(*c)) {
                l.rewritten += 1;
                continue;
            }
            match kind {
                // The calibrated sensor hears 16 blocks; it is predicted at the
                // plain one's margin, which is inside both.
                Sculk::Sensor | Sculk::Calibrated => l.sensors.push((*c, PREDICTED_RADIUS_SQ)),
                Sculk::Shrieker => l.shriekers.push(*c),
                Sculk::Catalyst => {}
            }
        }
        l
    }

    /// The sensors the world holds that a leg can predict.
    pub fn sensors(&self) -> usize {
        self.sensors.len()
    }

    /// The shriekers the world holds that a leg can predict.
    pub fn shriekers(&self) -> usize {
        self.shriekers.len()
    }

    /// Devices a runtime write touches, never predicted.
    pub fn rewritten(&self) -> usize {
        self.rewritten
    }

    /// Whether an occluding block lies in the box the two cells span — a
    /// superset of every line `isOccluded` clips between their centres.
    fn occluded(&self, a: [i32; 3], b: [i32; 3]) -> bool {
        let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
        self.occluders
            .range(lo..=hi)
            .any(|c| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]))
    }

    /// **The vibrations a leg makes** (spec-0100 §4.6): none on a `sneak` leg;
    /// otherwise every sensor within `distSqr ≤ 49` of a route cell whose step
    /// is not dampened (`#dampens_vibrations` in the feet cell or the cell under
    /// it) and with no occluder in the box the two cells span, and for each the
    /// shriekers within `distSqr ≤ 64` of it with no occluder between.
    pub fn leg_vibrations(&self, cells: &[[i32; 3]], sneak: bool) -> Vec<Vibration> {
        if sneak || self.sensors.is_empty() {
            return Vec::new();
        }
        let steps: Vec<[i32; 3]> = cells
            .iter()
            .copied()
            .filter(|c| {
                !self.dampeners.contains(c) && !self.dampeners.contains(&[c[0], c[1] - 1, c[2]])
            })
            .collect();
        let mut out = Vec::new();
        for &(s, r2) in &self.sensors {
            let heard = steps
                .iter()
                .any(|c| dist_sq(*c, s) <= r2 && !self.occluded(*c, s));
            if !heard {
                continue;
            }
            let shriekers: Vec<[i32; 3]> = self
                .shriekers
                .iter()
                .copied()
                .filter(|k| dist_sq(*k, s) <= LISTENER_RADIUS_SQ && !self.occluded(*k, s))
                .collect();
            out.push(Vibration {
                sensor: s,
                shriekers,
            });
        }
        out
    }
}

/// Everything the build's sculk block proved, for its lines.
#[derive(Debug, Clone, Default)]
pub struct SculkBinding {
    /// The rest rule's counts.
    pub rest: RestBinding,
    /// The containment proof's counts.
    pub reach: ReachBinding,
    /// The catalyst proof's counts.
    pub catalysts: CatalystBinding,
}

/// **The build's sculk block** (spec-0100 §4.1, §4.3, §4.4): the rest rule
/// over the assembled world and every runtime write, the containment proof,
/// and the catalyst proof. Prints each binding line before its verdict is
/// taken, zeroes included.
pub fn check(
    plan: &Plan,
    world: &World,
    blocks: &BlockMap,
    entry: Option<[i32; 3]>,
    wave_seats: &BTreeMap<String, Vec<[i32; 3]>>,
) -> Result<SculkBinding, Failure> {
    let writes = runtime_writes(plan);
    let (rest, refused) = check_rest(blocks, &writes);
    eprintln!(
        "sculk rest: {} acting sculk cell(s) of the assembled world and {} runtime write(s) \
         judged by the one rest rule",
        rest.cells, rest.writes
    );
    if let Some(f) = refused {
        return Err(f);
    }
    let (reach, refused) = prove_reach(blocks, &writes);
    eprintln!("{}", reach.line());
    if let Some(f) = refused {
        return Err(f);
    }
    let catalysts: Vec<[i32; 3]> = cells_of(blocks, &writes, Sculk::Catalyst)
        .into_keys()
        .collect();
    // The world a body can be in: no exclusion keeps a body out of a place it
    // can die (a killing volume is a place to die), and every barrier a hand
    // opens stands open.
    let open = world.without_exclusions();
    let openable: BTreeSet<[i32; 3]> = blocks
        .iter()
        .filter(|(_, n)| delvewright_dsl::blockshape::is_player_openable(n.as_str()))
        .map(|(c, _)| *c)
        .collect();
    let opened = if openable.is_empty() {
        open
    } else {
        open.with_openings_open(&openable)
    };
    let body_roots: Vec<[i32; 3]> = crate::compiler::lethal::put_at_roots(plan, entry)
        .into_iter()
        .filter_map(|r| opened.snap(r, crate::compiler::nav::SNAP_RADIUS))
        .collect();
    let mut mob_roots: Vec<(Vec<[i32; 3]>, Footprint)> = Vec::new();
    for w in &plan.campaign.quests.content.waves {
        let Some(cells) = wave_seats.get(w.id.as_str()) else {
            continue;
        };
        let mut seat = 0usize;
        for mob in &w.mobs {
            let roots: Vec<[i32; 3]> = (0..mob.count as usize)
                .filter_map(|i| cells.get(seat + i).copied())
                .collect();
            seat += mob.count as usize;
            if !roots.is_empty() {
                mob_roots.push((roots, crate::compiler::nav::entity_footprint(&mob.entity)));
            }
        }
    }
    let (cats, refused) = prove_catalysts(&catalysts, &opened, &body_roots, &mob_roots);
    eprintln!("{}", cats.line());
    if let Some(f) = refused {
        return Err(f);
    }
    Ok(SculkBinding {
        rest,
        reach,
        catalysts: cats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    fn map(cells: &[([i32; 3], &str)]) -> BlockMap {
        cells
            .iter()
            .map(|(c, s)| (*c, BlockState::new(s)))
            .collect()
    }

    fn world_of(blocks: &BlockMap) -> World {
        World::from_occupancy(
            crate::compiler::assembled::occupancy_over(blocks, &BTreeSet::new()),
            crate::compiler::nav::Premises::geometry_only(),
        )
    }

    fn reach_code(blocks: &BlockMap) -> Option<&'static str> {
        prove_reach(blocks, &[]).1.map(|f| f.code.id())
    }

    /// Acceptance criterion 5: the reach is the six neighbours, extended through
    /// a conducting cell above; glass above does not conduct; a calibrated
    /// sensor's input side holds no source.
    #[test]
    fn a_sensor_reaches_its_neighbours_and_the_cell_above_conducts() {
        let s = [0, 1, 0];
        let door = "minecraft:iron_door[facing=north,half=lower,hinge=left,open=false,powered=false]";
        // An iron door beside the sensor.
        let beside = map(&[(s, "minecraft:sculk_sensor"), ([1, 1, 0], door)]);
        assert_eq!(reach_code(&beside), Some("DW1000"));
        // Stone above the sensor and an iron door beside the stone.
        let through_stone = map(&[
            (s, "minecraft:sculk_sensor"),
            ([0, 2, 0], "minecraft:stone"),
            ([1, 2, 0], door),
        ]);
        let (b, f) = prove_reach(&through_stone, &[]);
        assert_eq!(f.map(|f| f.code.id()), Some("DW1000"));
        assert_eq!(b.reach_cells, 11, "6 neighbours + 5 of the conducting cell above");
        // Glass above does not conduct: the same door is out of reach.
        let through_glass = map(&[
            (s, "minecraft:sculk_sensor"),
            ([0, 2, 0], "minecraft:glass"),
            ([1, 2, 0], door),
        ]);
        let (b, f) = prove_reach(&through_glass, &[]);
        assert!(f.is_none(), "{f:?}");
        assert_eq!(b.reach_cells, 6);
        assert_eq!(b.sensors, 1);
        // A calibrated sensor facing north with a lever behind it (south, +z).
        let cal = "minecraft:calibrated_sculk_sensor[facing=north,power=0,sculk_sensor_phase=inactive,waterlogged=false]";
        let lever = "minecraft:lever[face=floor,facing=north,powered=false]";
        let behind = map(&[(s, cal), ([0, 1, 1], lever)]);
        let f = prove_reach(&behind, &[]).1.expect("second arm");
        assert_eq!(f.code.id(), "DW1000");
        assert!(f.message.contains("input side"), "{}", f.message);
        // Air behind it passes.
        let open = map(&[(s, cal)]);
        let (b, f) = prove_reach(&open, &[]);
        assert!(f.is_none(), "{f:?}");
        assert_eq!((b.sensors, b.calibrated), (1, 1));
    }

    /// A runtime write that lays a reader beside a sensor is the same defect
    /// later: the write is named.
    #[test]
    fn a_runtime_write_beside_a_sensor_is_judged() {
        let blocks = map(&[([0, 1, 0], "minecraft:sculk_sensor")]);
        let w = Write {
            region: ([1, 1, 0], [1, 1, 0]),
            block: "minecraft:iron_door[facing=north,half=lower,hinge=left,open=false,powered=false]"
                .to_string(),
            label: "`fill-region` at `/content/quests/0/on_complete/0`".to_string(),
        };
        let f = prove_reach(&blocks, std::slice::from_ref(&w))
            .1
            .expect("refused");
        assert_eq!(f.code.id(), "DW1000");
        assert!(f.message.contains("fill-region"), "{}", f.message);
        // And a write that lays a summoning shrieker is the rest rule's.
        let w = Write {
            region: ([5, 1, 5], [5, 1, 5]),
            block: "minecraft:sculk_shrieker[can_summon=true]".to_string(),
            label: "`set-block` at `/x`".to_string(),
        };
        let f = check_rest(&BlockMap::new(), &[w]).1.expect("refused");
        assert_eq!(f.code.id(), "DW0998");
    }

    /// The rest rule over the assembled world names the cell.
    #[test]
    fn the_assembled_world_is_judged_at_rest() {
        let blocks = map(&[(
            [3, 4, 5],
            "minecraft:sculk_sensor[power=0,sculk_sensor_phase=active,waterlogged=false]",
        )]);
        let (b, f) = check_rest(&blocks, &[]);
        assert_eq!(b.cells, 1);
        let f = f.expect("refused");
        assert_eq!(f.code.id(), "DW0999");
        assert!(f.message.contains("[3, 4, 5]"), "{}", f.message);
    }

    /// A stone floor 0..20 × 0..3 at y=0, a body standing at y=1.
    fn hall() -> Vec<([i32; 3], &'static str)> {
        let mut v = Vec::new();
        for x in 0..20 {
            for z in 0..3 {
                v.push(([x, 0, z], "minecraft:stone"));
            }
        }
        v
    }

    /// Acceptance criterion 6: a catalyst 8 cells from a route cell is refused
    /// naming that cell and 64; at 9 it passes; one out of the route's reach but
    /// 7 from a wave seat is refused.
    #[test]
    fn a_catalyst_is_refused_within_reach_of_a_body() {
        // The body walks only x in 0..=10 (a wall at x=11 shuts the hall).
        let mut cells = hall();
        for z in 0..3 {
            cells.push(([11, 1, z], "minecraft:stone"));
            cells.push(([11, 2, z], "minecraft:stone"));
        }
        let blocks = map(&cells);
        let world = world_of(&blocks);
        let root = [0, 1, 0];
        // Below the floor at x=10: 8 cells under the walked cell [10, 1, 0]
        // would be y=-7, distSqr 64.
        let (b, f) = prove_catalysts(&[[10, -7, 0]], &world, &[root], &[]);
        let f = f.expect("8 cells is within range");
        assert_eq!(f.code.id(), "DW1001");
        assert!(f.message.contains("[10, 1, 0]"), "{}", f.message);
        assert!(f.message.contains("distSqr 64"), "{}", f.message);
        assert_eq!(b.nearest.map(|n| n.2), Some(64));
        // At 9 it passes, and the nearest is stated.
        let (b, f) = prove_catalysts(&[[10, -8, 0]], &world, &[root], &[]);
        assert!(f.is_none(), "{f:?}");
        assert_eq!(b.nearest.map(|n| n.2), Some(81));
        // Beyond the wall, out of the route's reach: the nearest walked cell is
        // [10, 1, 1], distSqr 36 + 49 = 85 from [16, -6, 1] — but a wave seated
        // at [16, 1, 1] stands 7 above it.
        let cat = [16, -6, 1];
        let (_, f) = prove_catalysts(&[cat], &world, &[root], &[]);
        assert!(f.is_none(), "the route alone does not reach it: {f:?}");
        let seat = [16, 1, 1];
        let (b, f) = prove_catalysts(
            &[cat],
            &world,
            &[root],
            &[(vec![seat], Footprint::player())],
        );
        let f = f.expect("a wave member stands 7 from it");
        assert_eq!(f.code.id(), "DW1001");
        assert!(f.message.contains("wave member"), "{}", f.message);
        assert!(b.mob_cells > 0);
    }

    /// Acceptance criterion 7 (the nav half): a body steps from a floor onto a
    /// sensor and back, standing where it stands on a bottom slab.
    #[test]
    fn the_sculk_blocks_are_half_floors() {
        for device in [
            "minecraft:sculk_sensor",
            "minecraft:calibrated_sculk_sensor",
            "minecraft:sculk_shrieker",
        ] {
            let mut cells = hall();
            cells.push(([5, 1, 1], device));
            let world = world_of(&map(&cells));
            let mut slab = hall();
            slab.push(([5, 1, 1], "minecraft:stone_slab[type=bottom,waterlogged=false]"));
            let slab = world_of(&map(&slab));
            let on = [5, 2, 1];
            assert!(world.is_standable(on), "{device}: its top is standable");
            assert_eq!(world.feet_y(on), slab.feet_y(on), "{device}: a slab's height");
            assert!(world.find_path([4, 1, 1], on).is_some(), "{device}: onto it");
            assert!(world.find_path(on, [6, 1, 1]).is_some(), "{device}: off it");
            assert_eq!(
                world.find_path([4, 1, 1], [6, 1, 1]),
                slab.find_path([4, 1, 1], [6, 1, 1]),
                "{device}: the route over it is the route over a slab"
            );
        }
    }

    /// The prediction: a sensor within 7 of a route cell, its shrieker within 8
    /// of it, nothing on a sneak leg, nothing through wool.
    #[test]
    fn a_leg_predicts_the_sensors_its_route_sets_off() {
        let blocks = map(&[
            ([5, 0, 3], "minecraft:sculk_sensor"),
            ([5, 0, 10], "minecraft:sculk_shrieker"),
            ([30, 0, 0], "minecraft:sculk_sensor"),
        ]);
        let l = Listening::of(&blocks, &[]);
        assert_eq!((l.sensors(), l.shriekers()), (2, 1));
        let route: Vec<[i32; 3]> = (0..=10).map(|x| [x, 1, 0]).collect();
        let v = l.leg_vibrations(&route, false);
        assert_eq!(
            v,
            vec![Vibration {
                sensor: [5, 0, 3],
                shriekers: vec![[5, 0, 10]],
            }]
        );
        assert!(l.leg_vibrations(&route, true).is_empty(), "a sneak leg");
        // Wool between the shrieker and the sensor occludes.
        let walled = map(&[
            ([5, 0, 3], "minecraft:sculk_sensor"),
            ([5, 0, 7], "minecraft:white_wool"),
            ([5, 0, 10], "minecraft:sculk_shrieker"),
        ]);
        let v = Listening::of(&walled, &[]).leg_vibrations(&route, false);
        assert_eq!(v.len(), 1);
        assert!(v[0].shriekers.is_empty(), "{v:?}");
        // A carpet underfoot dampens every step.
        let mut carpeted = vec![([5, 0, 3], "minecraft:sculk_sensor")];
        for x in 0..=10 {
            carpeted.push(([x, 0, 0], "minecraft:red_carpet"));
        }
        let v = Listening::of(&map(&carpeted), &[]).leg_vibrations(&route, false);
        assert!(v.is_empty(), "{v:?}");
        // A sensor a runtime write covers is not predicted.
        let w = Write {
            region: ([5, 0, 3], [5, 0, 3]),
            block: "minecraft:stone".into(),
            label: "x".into(),
        };
        let l = Listening::of(&blocks, &[w]);
        assert_eq!(l.rewritten(), 1);
    }
}
