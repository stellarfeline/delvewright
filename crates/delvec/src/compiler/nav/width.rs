//! **A way is whole** (spec-0120): the clear width a door, a stair or a road
//! declares is clear along the whole way, in every state its cells can take
//! (`DW1057`), and a narrowing the author means is declared on the way and
//! confirmed cell for cell (`DW1058`).
//!
//! Every other proof asks whether a body can get through, and one column of a
//! body's width answers yes. This one asks how much of the width the author
//! declared is free. The width is never measured: a seam's `opening` is its
//! clear width, a contact's span is the author's, a stair's treads are laid
//! across the opening's width, a way place states its `clear`, and a piece way
//! is its contract region. What is measured is which of those columns a body
//! can be in.
//!
//! # The model, cited and authored
//!
//! - **One predicate** (cited, spec-0120 §5): a column is clear when a body can
//!   be in its body cell — [`super::can_be`], the nav model's one answer to
//!   standing, holding a climbable or floating — or in the cell over it when
//!   what stands in the body cell is a floor a body steps onto
//!   (`MAX_AUTO_STEP_16`: a slab, a path). A beam over a body's head blocks
//!   nothing; a post, a wall, a raised full block takes the column. A
//!   vertical column (a hole through a floor) is clear when every cell of it
//!   is free (`World::is_clear`), which is what a falling or climbing body
//!   passes through.
//! - **Where the body cell is** (authored): a seam's approach stands at its
//!   place's walk plane, a contact's span at the higher of the two, a laid
//!   stair's course over its tread, a way place at its floor. A piece's way
//!   says no walk plane, so a station's plane is the lowest a body stands in
//!   any of its columns as built, and a column a body stands in only higher
//!   than a step above it is taken.
//! - **States** (cited, spec-0120 §3): a column is clear only when it is clear
//!   in every state its cells can take — the world as shipped
//!   ([`crate::compiler::assembled::shipped_blocks`]: every gate as authored,
//!   the relight fixtures, the trigger props) and that world after each
//!   runtime write alone: a `fill-region`, a `set-block`, an `open-gate`, a
//!   `close-gate`, a laid or cleared way, a shortcut seal, a timed gate's two
//!   phases. Each state is a world of its own, classified by the one
//!   occupancy model ([`crate::compiler::assembled::occupancy_over`]) over a
//!   window of the blocks round the way, geometry only: the width is a fact of
//!   the blocks, and no premise of the campaign changes it.
//! - **The way's own regions** (cited, spec-0120 §3a) stand open in every
//!   state: a shut seam's bar, a gate station the same beat opens, a piece
//!   way's `bar` and `way`. They are the way's shut state, judged by the
//!   proofs that own it (`DW0836`, `DW1011`).
//! - **A portal's plane** (cited, §3b) is `DW0836`'s, which refuses any
//!   allocated cell not passable; its columns are counted *left to `DW0836`*
//!   and never judged twice. A contact's plane is judged.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::metrics::MAX_AUTO_STEP_16;
use delvewright_dsl::siteplan::{Crossing, NarrowSide, PlacedBox, PlacedSeam, seam_anchor, way_at};
use delvewright_dsl::{DwCode, ExitTier};
use serde_json::{Value, json};

use super::{Premises, World};
use crate::compiler::blockstate::{BlockMap, BlockState};
use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, ResolvedAnchor};
use crate::compiler::registry::PrefabRegistry;

delvewright_dsl::dw_code! {
    /// `DW1057`: **a way narrowed by something the author did not declare**
    /// (spec-0120 §6.1). A column of a door's, a stair's or a road's declared
    /// clear width is taken — by prefab bytes, the stand-in's own blocks, an
    /// engine-seated object or a runtime write — in some state its cells can
    /// take, while other columns of the same station stay clear, and the way
    /// declares no `narrowed[]` entry for it. Build tier.
    pub const DW_WAY_NARROWED: DwCode = DwCode::new("DW1057", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1058`: **a declared narrowing the bytes do not show** (spec-0120
    /// §6.2). A `narrowed[]` entry names a column that is clear in every
    /// state, a block other than the one standing there, or a side, a course
    /// or a cell the way does not have — the excuse outlived, or never met,
    /// the thing it excuses. Build tier.
    pub const DW_NARROWING_NOT_SHOWN: DwCode = DwCode::new("DW1058", ExitTier::Build);
}

/// An inclusive region of world cells.
pub type Region3 = ([i32; 3], [i32; 3]);

/// A gate region by its anchor.
pub type NamedRegion = (String, Region3);

/// How many taking cells a refusal names before summarising the rest.
const CELL_LIST_LIMIT: usize = 8;

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// Which of the five declared objects a way is (spec-0120 §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WayKind {
    /// A `walk`, `stair`, `climb` or `drop` seam with an `opening`.
    Portal,
    /// A seam with a `contact`.
    Contact,
    /// A site-plan box that declares `way`.
    Place,
    /// A contract edge of a placed piece that answers no plan seam.
    Piece,
}

impl WayKind {
    fn word(self) -> &'static str {
        match self {
            WayKind::Portal => "portal",
            WayKind::Contact => "contact",
            WayKind::Place => "way-place",
            WayKind::Piece => "piece-way",
        }
    }
}

/// Where along a way a station stands — the frame a declaration names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StationKey {
    /// An approach, by the side it is on.
    Side(NarrowSide),
    /// A portal's or a contact's plane.
    Plane,
    /// A stair run's course, top first.
    Course(u32),
    /// A way place's station, in cells from the box's low corner on its axis.
    Along(u32),
    /// A piece way's station, by its coordinate on the travel axis.
    Piece(i32),
}

/// One column of a station: the world column it stands in, the cells a body
/// may be in there and the body cell the way's walk plane puts it at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Its place across the way, from the way's low corner (`across`).
    pub index: usize,
    /// The world `x`.
    pub x: i32,
    /// The world `z`.
    pub z: i32,
    /// The lowest cell of the column a body is judged in.
    pub ylo: i32,
    /// The highest.
    pub yhi: i32,
    /// The body cell the way's walk plane puts a body in, when the way says
    /// one; `None` on a piece way, whose station plane is measured.
    pub y0: Option<i32>,
    /// A hole through a floor: clear when every cell of it is free.
    pub vertical: bool,
}

/// One station of a way: a cross-section a body passes, judged column by
/// column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Station {
    /// Its place in the way's frame.
    pub key: StationKey,
    /// The station in words, for a message.
    pub label: String,
    /// A portal's plane, counted and left to `DW0836`.
    pub deferred: bool,
    /// Its columns.
    pub columns: Vec<Column>,
}

/// A narrowing the author declared on a way, in the way's frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The station it names; `None` when it names one the way does not have.
    pub station: Option<StationKey>,
    /// The column indices it names.
    pub columns: BTreeSet<usize>,
    /// What it named instead, when it names no station or a column the
    /// station does not have — the `DW1058` shape that is not a block.
    pub unplaced: Option<String>,
    /// The block it says stands there.
    pub block: String,
    /// What it says it is.
    pub what: String,
    /// The document pointer to the entry.
    pub path: String,
}

/// The frame a declaration is written in, for the remedy a refusal hands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// `seams[]` by edge: `{side, across, course?}`.
    Seam(String),
    /// `boxes[]` by node: `{along, across}`.
    Place(String),
    /// A piece's contract edge: `{cells}` in the piece frame, with the inverse
    /// of the placement to write them in.
    Piece {
        /// The prefab.
        prefab: String,
        /// The edge in words.
        edge: String,
        /// The placement's world position.
        pos: [i32; 3],
        /// The placement's rotation.
        rotation: crate::compiler::solver::Rotation,
    },
}

/// Where a stair seam's run came from (spec-0120 §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// Not a stair.
    None,
    /// Laid by the stand-in.
    Laid,
    /// The host piece's contract flight.
    Piece,
    /// A bound host whose contract declares no flight joining the face.
    Undeclared,
}

/// One way, built from the plan, ready to judge.
#[derive(Debug, Clone)]
pub struct Way {
    /// Which kind.
    pub kind: WayKind,
    /// Its ledger name: the edge, the node, or the piece and its edge.
    pub name: String,
    /// The way in words.
    pub label: String,
    /// The declared clear width, in columns.
    pub width: usize,
    /// Its stations, in travel order.
    pub stations: Vec<Station>,
    /// The way's own regions, each cell with its open state (`None` = air).
    pub own: BTreeMap<[i32; 3], Option<String>>,
    /// The way's declared narrowings.
    pub declared: Vec<Declared>,
    /// The frame a declaration of this way is written in.
    pub frame: Frame,
    /// Where a stair seam's run came from.
    pub run: Run,
}

/// One runtime write as a state: the region, what it leaves there (`None` =
/// air) and the state in words.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateWrite {
    /// The region's inclusive corners.
    pub region: ([i32; 3], [i32; 3]),
    /// What it leaves in the region.
    pub block: Option<String>,
    /// The state in words (`while \`anchor/x\` is shut`).
    pub label: String,
}

impl StateWrite {
    fn meets(&self, lo: [i32; 3], hi: [i32; 3]) -> bool {
        let (a, b) = self.region;
        (0..3).all(|i| a[i].min(b[i]) <= hi[i] && lo[i] <= a[i].max(b[i]))
    }
}

/// What the world holds for every state: the blocks as shipped, the fence
/// gates authored open, every runtime write, and every gate region sealed at
/// world load with its name.
pub struct Ground<'a> {
    /// The blocks as shipped ([`crate::compiler::assembled::shipped_blocks`]).
    pub shipped: &'a BlockMap,
    /// Fence-gate cells authored open.
    pub open_gates: &'a BTreeSet<[i32; 3]>,
    /// Every runtime write.
    pub writes: &'a [StateWrite],
    /// Every gate region the world authors shut at load, by anchor.
    pub sealed: &'a [NamedRegion],
    /// What the engine seated in a cell of the shipped world, in words — a
    /// relight fixture, a trigger's prop — so a refusal names the object.
    pub seated: &'a BTreeMap<[i32; 3], String>,
}

// ---------------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------------

/// One taking cell: where, which block, in which state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taking {
    /// The cell.
    pub cell: [i32; 3],
    /// The block standing there (`minecraft:air` for a missing floor).
    pub block: String,
    /// The state in which it takes the column.
    pub state: String,
}

/// One station's verdict.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationVerdict {
    /// Columns clear, per state judged (as built first).
    pub clear: Vec<usize>,
    /// The states in words, aligned with `clear`.
    pub states: Vec<String>,
    /// Columns taken in a state the station is not shut in, with what takes
    /// each (the first state that does).
    pub taken: BTreeMap<usize, Vec<Taking>>,
}

impl StationVerdict {
    fn narrowed(&self, width: usize) -> bool {
        self.clear.iter().any(|c| *c > 0 && *c < width)
    }
}

/// What one way's judgement found.
#[derive(Debug, Clone)]
pub struct WayRecord {
    /// The way.
    pub way: Way,
    /// Per station, aligned with `way.stations`.
    pub stations: Vec<StationVerdict>,
    /// States judged for this way, as built included.
    pub states: usize,
    /// The first undeclared narrowing, as a refusal.
    pub undeclared: Option<Failure>,
    /// Every declaration the bytes do not show, as refusals.
    pub not_shown: Vec<Failure>,
    /// Declarations confirmed.
    pub confirmed: usize,
    /// Columns those declarations excuse.
    pub excused: usize,
}

impl WayRecord {
    /// `whole`, `narrowed` or `narrowed-as-declared`, over the stations judged.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        let narrowed = self
            .way
            .stations
            .iter()
            .zip(&self.stations)
            .any(|(s, v)| !s.deferred && v.narrowed(s.columns.len()));
        if !narrowed {
            "whole"
        } else if self.undeclared.is_none() && self.not_shown.is_empty() {
            "narrowed-as-declared"
        } else {
            "narrowed"
        }
    }

    fn to_json(&self) -> Value {
        let stations: Vec<Value> = self
            .way
            .stations
            .iter()
            .zip(&self.stations)
            .map(|(s, v)| {
                let taken: Vec<Value> = v
                    .taken
                    .iter()
                    .map(|(col, cells)| {
                        json!({
                            "column": col,
                            "cells": cells.iter().map(|t| json!({
                                "cell": t.cell,
                                "block": t.block,
                                "state": t.state,
                            })).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                let verdict = if s.deferred {
                    "left to DW0836"
                } else if v.clear.iter().all(|c| *c == 0) {
                    "shut"
                } else if v.narrowed(s.columns.len()) {
                    "narrowed"
                } else {
                    "whole"
                };
                json!({
                    "station": s.label,
                    "columns": s.columns.len(),
                    "clear": v.clear,
                    "states": v.states,
                    "taken": taken,
                    "verdict": verdict,
                })
            })
            .collect();
        let declared: Vec<Value> = self
            .way
            .declared
            .iter()
            .map(|d| {
                let refused = self
                    .not_shown
                    .iter()
                    .any(|f| f.message.contains(&format!("`{}`", d.path)));
                json!({
                    "declared_at": d.path,
                    "what": d.what,
                    "block": d.block,
                    "columns": d.columns.iter().collect::<Vec<_>>(),
                    "confirmed": !refused && d.unplaced.is_none(),
                })
            })
            .collect();
        json!({
            "kind": self.way.kind.word(),
            "way": self.way.name,
            "width": self.way.width,
            "states": self.states,
            "verdict": self.verdict(),
            "stations": stations,
            "narrowed": declared,
        })
    }
}

// ---------------------------------------------------------------------------
// Judging
// ---------------------------------------------------------------------------

/// The cells a column's judgement reads: its own, the floor under it and the
/// headroom over it.
fn column_reach(c: &Column) -> ([i32; 3], [i32; 3]) {
    ([c.x, c.ylo - 1, c.z], [c.x, c.yhi + 2, c.z])
}

/// How far round a way's columns the judgement reads the blocks: as far as
/// water flows from a source on level ground (vanilla's seven cells, one more
/// for the cell it reaches) across and above, two below.
const WINDOW_FLOW: i32 = 8;

/// The window of blocks a judgement over `lo..=hi` reads.
fn window(lo: [i32; 3], hi: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    (
        [lo[0] - WINDOW_FLOW, lo[1] - 2, lo[2] - WINDOW_FLOW],
        [
            hi[0] + WINDOW_FLOW,
            hi[1] + WINDOW_FLOW,
            hi[2] + WINDOW_FLOW,
        ],
    )
}

/// **Whether a body can be in this column** over `w`, its body cell at `y0`
/// (spec-0120 §3, *Columns*): `can_be` at the body cell, or at the cell over it
/// when what stands in the body cell is a floor a body steps onto.
fn column_clear(w: &World, c: &Column, y0: i32) -> bool {
    if c.vertical {
        return (c.ylo..=c.yhi).all(|y| w.is_clear([c.x, y, c.z]));
    }
    let at = [c.x, y0, c.z];
    if super::can_be(w, at) {
        return true;
    }
    let over = [c.x, y0 + 1, c.z];
    super::can_be(w, over)
        && w.solid_top_16(at)
            .is_some_and(|top| i64::from(top) <= MAX_AUTO_STEP_16)
}

/// The lowest cell of a column a body can be in, over `w` — the measurement a
/// piece way's station plane is taken from.
fn lowest_body(w: &World, c: &Column) -> Option<i32> {
    (c.ylo..=c.yhi + 1).find(|y| super::can_be(w, [c.x, *y, c.z]))
}

/// What takes a column a body cannot be in: every non-air block of the body
/// cell and the head cell, or — when both are empty — the missing floor.
fn taking_cells(map: &BlockMap, c: &Column, y0: i32) -> Vec<([i32; 3], String)> {
    let ys: Vec<i32> = if c.vertical {
        (c.ylo..=c.yhi).collect()
    } else {
        vec![y0, y0 + 1]
    };
    let mut out: Vec<([i32; 3], String)> = ys
        .iter()
        .map(|y| [c.x, *y, c.z])
        .filter_map(|cell| map.get(&cell).map(|b| (cell, b.as_str().to_string())))
        .filter(|(_, b)| b != "minecraft:air")
        .collect();
    if out.is_empty() && !c.vertical {
        let under = [c.x, y0 - 1, c.z];
        out.push((
            under,
            map.get(&under)
                .map_or_else(|| "minecraft:air".to_string(), |b| b.as_str().to_string()),
        ));
    }
    out
}

fn set(map: &mut BlockMap, c: [i32; 3], block: Option<&str>) {
    match block {
        Some(b) if b != "minecraft:air" => {
            map.insert(c, BlockState::new(b));
        }
        _ => {
            map.remove(&c);
        }
    }
}

/// **Judge one way** over every state its corridor can take.
#[must_use]
pub fn judge(way: Way, ground: &Ground<'_>) -> WayRecord {
    let judged: Vec<&Column> = way
        .stations
        .iter()
        .filter(|s| !s.deferred)
        .flat_map(|s| s.columns.iter())
        .collect();
    let empty = WayRecord {
        stations: way
            .stations
            .iter()
            .map(|_| StationVerdict::default())
            .collect(),
        states: 0,
        undeclared: None,
        not_shown: Vec::new(),
        confirmed: 0,
        excused: 0,
        way: way.clone(),
    };
    if judged.is_empty() {
        return declarations(empty);
    }
    // The window the judgement reads: every column's reach, grown so every
    // cell is classified as the whole world classifies it — a sieve or a
    // climbable with its neighbours, and a cell water reaches with the source
    // it flows from (`WINDOW_FLOW` across and above).
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for c in &judged {
        let (a, b) = column_reach(c);
        for i in 0..3 {
            lo[i] = lo[i].min(a[i]);
            hi[i] = hi[i].max(b[i]);
        }
    }
    let (wlo, whi) = window(lo, hi);
    let inside = |c: &[i32; 3]| (0..3).all(|i| wlo[i] <= c[i] && c[i] <= whi[i]);
    let own_open = |m: &mut BlockMap| {
        for (c, b) in &way.own {
            if inside(c) {
                set(m, *c, b.as_deref());
            }
        }
    };
    let mut base: BlockMap = ground
        .shipped
        .range(wlo..=whi)
        .filter(|(c, _)| inside(c))
        .map(|(c, b)| (*c, *b))
        .collect();
    own_open(&mut base);
    let open_gates: BTreeSet<[i32; 3]> = ground
        .open_gates
        .iter()
        .filter(|c| inside(c))
        .copied()
        .collect();

    // The states: as shipped, then each write that reaches a column alone.
    let mut maps: Vec<(String, BlockMap)> = vec![("as built".to_string(), base.clone())];
    for w in ground.writes {
        if !judged.iter().any(|c| {
            let (a, b) = column_reach(c);
            w.meets(a, b)
        }) {
            continue;
        }
        let mut m = base.clone();
        for c in crate::compiler::assembled::region_cells(w.region.0, w.region.1) {
            if inside(&c) {
                set(&mut m, c, w.block.as_deref());
            }
        }
        own_open(&mut m);
        maps.push((w.label.clone(), m));
    }
    // Geometry only, and the decline is this check's subject: whether a body
    // can be in a column is a fact of the blocks there in one state. No
    // horizon, killing volume or gate premise changes the width a structure
    // takes, and each state already says what is shut.
    let worlds: Vec<World> = maps
        .iter()
        .map(|(_, m)| {
            World::from_occupancy(
                crate::compiler::assembled::occupancy_over(m, &open_gates),
                Premises::geometry_only(),
            )
        })
        .collect();

    // Who took a cell in the shipped world: an object the engine seated, a
    // gate shut at load, or the build.
    let shipped_label = |cell: [i32; 3]| -> String {
        if let Some(what) = ground.seated.get(&cell) {
            return format!("as built: {what}");
        }
        ground
            .sealed
            .iter()
            .find(|(_, (a, b))| {
                (0..3).all(|i| a[i].min(b[i]) <= cell[i] && cell[i] <= a[i].max(b[i]))
            })
            .map_or_else(
                || "as built".to_string(),
                |(name, _)| format!("while `{name}` is shut"),
            )
    };

    let mut verdicts = Vec::new();
    for s in &way.stations {
        if s.deferred || s.columns.is_empty() {
            verdicts.push(StationVerdict::default());
            continue;
        }
        // A station whose way says no walk plane: the lowest a body stands in
        // any of its columns as built.
        let plane: Option<i32> = s
            .columns
            .iter()
            .filter(|c| c.y0.is_none() && !c.vertical)
            .filter_map(|c| lowest_body(&worlds[0], c))
            .min();
        let y0 = |c: &Column| c.y0.or(plane).unwrap_or(c.ylo);
        let mut v = StationVerdict::default();
        let mut taken_in: BTreeMap<usize, Vec<Taking>> = BTreeMap::new();
        for (k, w) in worlds.iter().enumerate() {
            let clear: Vec<bool> = s
                .columns
                .iter()
                .map(|c| column_clear(w, c, y0(c)))
                .collect();
            let n = clear.iter().filter(|x| **x).count();
            v.clear.push(n);
            v.states.push(maps[k].0.clone());
            if n == 0 {
                continue; // shut in this state: the gate's own finding.
            }
            for (c, ok) in s.columns.iter().zip(&clear) {
                if *ok || taken_in.contains_key(&c.index) {
                    continue;
                }
                let cells = taking_cells(&maps[k].1, c, y0(c))
                    .into_iter()
                    .map(|(cell, block)| {
                        let shipped = maps[0].1.get(&cell).map(|b| b.as_str().to_string());
                        let state = if k == 0 || shipped.as_deref() == Some(block.as_str()) {
                            shipped_label(cell)
                        } else {
                            maps[k].0.clone()
                        };
                        Taking { cell, block, state }
                    })
                    .collect();
                taken_in.insert(c.index, cells);
            }
        }
        v.taken = taken_in;
        verdicts.push(v);
    }
    let rec = WayRecord {
        stations: verdicts,
        states: maps.len(),
        ..empty
    };
    declarations(rec)
}

/// The block a declaration names matches the block that stands: the same id,
/// and every property the declaration states with the same value.
fn block_matches(declared: &str, standing: &str) -> bool {
    let (did, dprops) = split_state(declared);
    let (sid, sprops) = split_state(standing);
    did == sid && dprops.iter().all(|(k, v)| sprops.get(k) == Some(v))
}

fn split_state(s: &str) -> (&str, BTreeMap<&str, &str>) {
    let (id, rest) = s.split_once('[').unwrap_or((s, ""));
    let props = rest
        .trim_end_matches(']')
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .collect();
    (id, props)
}

/// Compare the way's declarations with what its stations found, and write the
/// refusals.
fn declarations(mut rec: WayRecord) -> WayRecord {
    let way = rec.way.clone();
    let mut first_undeclared: Option<Failure> = None;
    let mut not_shown: Vec<Failure> = Vec::new();
    let mut confirmed = 0usize;
    let mut excused = 0usize;
    for d in way.declared.iter().filter(|d| d.unplaced.is_some()) {
        not_shown.push(Failure {
            code: DW_NARROWING_NOT_SHOWN,
            message: format!(
                "the narrowing `{what}` declared at `{path}` on {label} names {why}. A \
                 declaration is confirmed against the way it is written on, and this one names \
                 nothing that way has. Correct the entry or remove it.",
                what = d.what,
                path = d.path,
                label = way.label,
                why = d.unplaced.as_deref().unwrap_or(""),
            ),
        });
    }
    for (s, v) in way.stations.iter().zip(&rec.stations) {
        if s.deferred {
            continue;
        }
        let width = s.columns.len();
        let here: Vec<&Declared> = way
            .declared
            .iter()
            .filter(|d| d.unplaced.is_none() && d.station.as_ref() == Some(&s.key))
            .collect();
        let mut declared_cols: BTreeMap<usize, &Declared> = BTreeMap::new();
        for d in &here {
            for c in &d.columns {
                declared_cols.insert(*c, d);
            }
        }
        let narrowed = v.narrowed(width);
        // Undeclared: a taken column no entry names.
        let undeclared: Vec<usize> = if narrowed {
            v.taken
                .keys()
                .filter(|c| !declared_cols.contains_key(c))
                .copied()
                .collect()
        } else {
            Vec::new()
        };
        if !undeclared.is_empty() && first_undeclared.is_none() {
            first_undeclared = Some(undeclared_refusal(&way, s, v, &undeclared));
        }
        // Each entry: every column it names is taken, by the block it names.
        for d in &here {
            let mut faults: Vec<String> = Vec::new();
            for c in &d.columns {
                match v.taken.get(c) {
                    None => faults.push(format!(
                        "column {c} of {station} is clear in every state judged",
                        station = s.label
                    )),
                    Some(cells) => {
                        let standing = cells.first().map(|t| t.block.as_str()).unwrap_or("");
                        if !block_matches(&d.block, standing) {
                            let at = cells.first().map_or([0; 3], |t| t.cell);
                            faults.push(format!(
                                "it declares `{}`, but `{standing}` stands at [{}, {}, {}] in \
                                 column {c} of {}",
                                d.block, at[0], at[1], at[2], s.label
                            ));
                        }
                    }
                }
            }
            if faults.is_empty() {
                confirmed += 1;
                excused += d.columns.len();
            } else {
                not_shown.push(Failure {
                    code: DW_NARROWING_NOT_SHOWN,
                    message: format!(
                        "the narrowing `{what}` declared at `{path}` on {label} is not what the \
                         bytes show: {faults}. An excuse never outlives the thing it excused: \
                         correct the entry or remove it, or put the structure back.",
                        what = d.what,
                        path = d.path,
                        label = way.label,
                        faults = faults.join("; "),
                    ),
                });
            }
        }
    }
    rec.undeclared = first_undeclared;
    rec.not_shown = not_shown;
    rec.confirmed = confirmed;
    rec.excused = excused;
    rec
}

/// The `DW1057` refusal for one way at its first narrowed station.
fn undeclared_refusal(way: &Way, s: &Station, v: &StationVerdict, cols: &[usize]) -> Failure {
    let width = s.columns.len();
    let worst = v
        .clear
        .iter()
        .zip(&v.states)
        .filter(|(c, _)| **c > 0 && **c < width)
        .min_by_key(|(c, _)| **c)
        .map(|(c, st)| (*c, st.clone()))
        .unwrap_or((width, String::new()));
    let per_state: Vec<String> = v
        .clear
        .iter()
        .zip(&v.states)
        .map(|(c, st)| format!("{st}: {c} of {width}"))
        .collect();
    let mut cells: Vec<String> = Vec::new();
    for c in cols {
        for t in v.taken.get(c).into_iter().flatten() {
            cells.push(format!(
                "[{}, {}, {}] `{}` ({})",
                t.cell[0], t.cell[1], t.cell[2], t.block, t.state
            ));
        }
    }
    let more = cells.len().saturating_sub(CELL_LIST_LIMIT);
    cells.truncate(CELL_LIST_LIMIT);
    let block = v
        .taken
        .get(&cols[0])
        .and_then(|t| t.first())
        .map_or_else(|| "minecraft:air".to_string(), |t| t.block.clone());
    let (lo, hi) = (
        cols[0],
        *cols
            .iter()
            .take_while(|c| **c <= cols[0] + 64)
            .last()
            .unwrap_or(&cols[0]),
    );
    let span_contiguous: Vec<usize> = (lo..=hi).collect();
    let across = if span_contiguous.iter().all(|c| cols.contains(c)) {
        format!("[{lo}, {hi}]")
    } else {
        format!("[{lo}, {lo}]")
    };
    let paste = match &way.frame {
        Frame::Seam(edge) => {
            let (side, course) = match &s.key {
                StationKey::Side(x) => (x.as_str().to_string(), String::new()),
                StationKey::Plane => ("plane".to_string(), String::new()),
                StationKey::Course(k) => ("run".to_string(), format!(", \"course\": {k}")),
                _ => (String::new(), String::new()),
            };
            format!(
                "on the seam for `{edge}`: `\"narrowed\": [{{\"side\": \"{side}\", \"across\": \
                 {across}{course}, \"block\": \"{block}\", \"what\": \"…\"}}]`"
            )
        }
        Frame::Place(node) => {
            let along = match &s.key {
                StationKey::Along(a) => *a,
                _ => 0,
            };
            format!(
                "on the box for `{node}`: `\"narrowed\": [{{\"along\": {along}, \"across\": \
                 {across}, \"block\": \"{block}\", \"what\": \"…\"}}]`"
            )
        }
        Frame::Piece {
            prefab,
            edge,
            pos,
            rotation,
        } => {
            let local: Vec<String> = cols
                .iter()
                .flat_map(|c| v.taken.get(c).into_iter().flatten())
                .filter(|t| t.block != "minecraft:air")
                .map(|t| {
                    let l = local_cell(*rotation, *pos, t.cell);
                    format!("[{}, {}, {}]", l[0], l[1], l[2])
                })
                .collect();
            format!(
                "on the contract edge {edge} of `{prefab}`: `\"narrowed\": [{{\"cells\": [{}], \
                 \"block\": \"{block}\", \"what\": \"…\"}}]`",
                local.join(", ")
            )
        }
    };
    Failure {
        code: DW_WAY_NARROWED,
        message: format!(
            "{label} is narrowed at {station}: its declared clear width is {width} column(s), and \
             {free} of {width} are clear {state} ({per_state}). Taken: {cells}{more}. A way's \
             width is not diminished along the way — a structure in part of a door, a stair or a \
             road is in the way unless it is meant. Move the structure out of the way's {width} \
             column(s); or, where it is meant, declare it {paste}, saying in `what` what it is.",
            label = way.label,
            station = s.label,
            free = worst.0,
            state = worst.1,
            per_state = per_state.join("; "),
            cells = cells.join(", "),
            more = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            },
        ),
    }
}

/// A world cell back into a piece's own frame.
fn local_cell(
    rotation: crate::compiler::solver::Rotation,
    pos: [i32; 3],
    world: [i32; 3],
) -> [i32; 3] {
    use crate::compiler::solver::Rotation;
    let d = [world[0] - pos[0], world[1] - pos[1], world[2] - pos[2]];
    let back = match rotation {
        Rotation::None => Rotation::None,
        Rotation::Cw90 => Rotation::Ccw90,
        Rotation::Cw180 => Rotation::Cw180,
        Rotation::Ccw90 => Rotation::Cw90,
    };
    back.transform(d)
}

fn world_cell(
    rotation: crate::compiler::solver::Rotation,
    pos: [i32; 3],
    local: [i32; 3],
) -> [i32; 3] {
    let t = rotation.transform(local);
    [pos[0] + t[0], pos[1] + t[1], pos[2] + t[2]]
}

// ---------------------------------------------------------------------------
// The population
// ---------------------------------------------------------------------------

/// The denominators and the counts the population carries before judgement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Census {
    /// Traversal seams the plan resolves.
    pub seams: usize,
    /// Boxes the plan resolves.
    pub boxes: usize,
    /// Traversal and barred contract edges of the placed pieces.
    pub edges: usize,
    /// Stair seams whose bound host declares no flight joining the face.
    pub runs_undeclared: usize,
    /// Plane columns left to `DW0836`.
    pub plane_columns: usize,
}

fn narrow(c: [i64; 3]) -> [i32; 3] {
    c.map(|v| i32::try_from(v).expect("a site cell fits a world coordinate"))
}

fn n32(v: i64) -> i32 {
    i32::try_from(v).expect("a site cell fits a world coordinate")
}

/// The other horizontal axis.
fn across_of(axis: usize) -> usize {
    if axis == 0 { 2 } else { 0 }
}

/// A horizontal station's column at world `(t on axis, u across)`.
fn column_at(axis: usize, t: i64, u: i64, index: usize, y0: i64) -> Column {
    let (x, z) = if axis == 0 { (t, u) } else { (u, t) };
    Column {
        index,
        x: n32(x),
        z: n32(z),
        ylo: n32(y0),
        yhi: n32(y0),
        y0: Some(n32(y0)),
        vertical: false,
    }
}

/// Which anchors the beats that open `anchor` also open — the gate stations a
/// shut way's own story clears with it (spec-0120 §9).
fn opened_with(plan: &Plan, anchor: &str) -> BTreeSet<String> {
    let mut lists: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
        if let Some((a, false)) = e.gate_region_write() {
            let list = site
                .path
                .rsplit_once('/')
                .map_or(site.path.as_str(), |(l, _)| l)
                .to_string();
            lists
                .entry(list)
                .or_default()
                .insert(a.as_str().to_string());
        }
    });
    let mut out = BTreeSet::new();
    for opened in lists.values() {
        if opened.contains(anchor) {
            out.extend(opened.iter().cloned());
        }
    }
    out.insert(anchor.to_string());
    out
}

fn gate_cells(plan: &Plan, anchor: &str) -> Vec<[i32; 3]> {
    crate::compiler::plan::gate_region_block_any(&plan.anchors, anchor)
        .map(|(a, b, _)| crate::compiler::assembled::region_cells(a, b).collect())
        .unwrap_or_default()
}

/// The approach station on one side of a wall seam: the place's walk plane,
/// one cell off the plane, across the opening.
fn wall_side(s: &PlacedSeam, side: NarrowSide, at: i64, floor: i64, label: String) -> Station {
    let u = across_of(s.normal_axis);
    let (lo, hi) = s.opening;
    // A wall's normal is horizontal: columns along `u`, the station at `at`.
    let columns = (lo[u]..=hi[u])
        .enumerate()
        .map(|(i, uu)| column_at(s.normal_axis, at, uu, i, floor))
        .collect();
    Station {
        key: StationKey::Side(side),
        label,
        deferred: false,
        columns,
    }
}

fn side_of(s: &PlacedSeam, node: &delvewright_dsl::NodeId) -> NarrowSide {
    if *node == s.a {
        NarrowSide::A
    } else {
        NarrowSide::B
    }
}

/// Every seam's way: its portal or contact, and a stair's run.
#[allow(clippy::too_many_lines)]
fn seam_ways(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    census: &mut Census,
    flights: &mut BTreeSet<(String, usize)>,
    out: &mut Vec<Way>,
) {
    let Some(b) = plan.blockout.as_ref() else {
        return;
    };
    let site = plan.campaign.site_plan.as_ref().map(|p| &p.content);
    let by_node: BTreeMap<&str, &PlacedBox> =
        b.boxes.iter().map(|x| (x.node.0.as_str(), x)).collect();
    let pieces = bound_pieces(plan, prefabs);
    for s in &b.seams {
        census.seams += 1;
        let decl: Vec<delvewright_dsl::siteplan::SeamNarrowing> = site
            .and_then(|p| p.seams.iter().find(|x| x.edge == s.edge))
            .map(|x| x.narrowed.clone())
            .unwrap_or_default();
        let seam_index = site
            .and_then(|p| p.seams.iter().position(|x| x.edge == s.edge))
            .unwrap_or(0);
        let own_gates: Vec<[i32; 3]> = if s.shut {
            opened_with(plan, &seam_anchor(&s.edge))
                .iter()
                .flat_map(|a| gate_cells(plan, a))
                .collect()
        } else {
            Vec::new()
        };
        let input = SeamInput {
            s,
            boxes: &by_node,
            laid: b.runs.iter().find(|r| r.edge == s.edge),
            pieces: &pieces,
            decl: &decl,
            seam_index,
            own_gates: &own_gates,
        };
        if let Some(w) = seam_way(&input, census, flights) {
            out.push(w);
        }
    }
}

/// What one seam's way is built from: the seam, the places, the run the
/// stand-in laid, the bound pieces, the seam's declarations and the gate
/// regions its own beat opens.
pub struct SeamInput<'a> {
    /// The seam.
    pub s: &'a PlacedSeam,
    /// Every place, by node.
    pub boxes: &'a BTreeMap<&'a str, &'a PlacedBox>,
    /// The run the stand-in laid for it, if it did.
    pub laid: Option<&'a crate::compiler::blockout::LaidStair>,
    /// Every bound place's piece, by node.
    pub pieces: &'a BTreeMap<&'a str, BoundPiece<'a>>,
    /// The seam's `narrowed[]`.
    pub decl: &'a [delvewright_dsl::siteplan::SeamNarrowing],
    /// The seam's index in `seams[]`.
    pub seam_index: usize,
    /// The cells of the gate regions the beat that opens this seam opens.
    pub own_gates: &'a [[i32; 3]],
}

/// **One seam's way** (spec-0120 §1.3): its portal or contact, and a stair's
/// run — the stand-in's, or the host piece's flight joined to the face.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn seam_way(
    input: &SeamInput<'_>,
    census: &mut Census,
    flights: &mut BTreeSet<(String, usize)>,
) -> Option<Way> {
    let SeamInput {
        s,
        boxes: by_node,
        laid,
        pieces,
        decl: decl_src,
        seam_index,
        own_gates,
    } = *input;
    let host_piece = |n: &delvewright_dsl::NodeId| pieces.get(n.0.as_str());
    let pa = by_node.get(s.a.0.as_str()).copied()?;
    let pb = by_node.get(s.b.0.as_str()).copied()?;
    let class_words = if s.shut {
        format!("shut {}", s.class)
    } else {
        s.class.to_string()
    };
    let label = format!("the {class_words} `{}` (\"{}\")", s.edge, s.form);
    let mut own: BTreeMap<[i32; 3], Option<String>> = BTreeMap::new();
    if s.shut {
        if let Some((lo, hi)) = s.bar() {
            for c in crate::compiler::assembled::region_cells(narrow(lo), narrow(hi)) {
                own.insert(c, None);
            }
        }
        for c in own_gates {
            own.insert(*c, None);
        }
    }
    let floor_seam = s.normal_axis == 1;
    let (olo, ohi) = s.opening;
    let mut stations: Vec<Station> = Vec::new();
    let mut run = Run::None;
    let mut flight_words: Option<String> = None;
    let width;
    let kind;
    // The two sides' places, and which side of the plane each is on.
    let n_dir = s.face.vector()[s.normal_axis];
    let side_at = |node: &PlacedBox| -> i64 {
        if *node == *pa {
            s.plane - n_dir
        } else {
            s.plane + n_dir
        }
    };
    let plane_cells = || -> usize { crate::compiler::blockout::cells_of(olo, ohi).count() };
    if s.crossing == Crossing::Contact {
        kind = WayKind::Contact;
        if floor_seam {
            width = plane_cells();
        } else {
            let u = across_of(s.normal_axis);
            width = usize::try_from(ohi[u] - olo[u] + 1).unwrap_or(0);
            let drop = s.class == "drop";
            let high = if pa.floor >= pb.floor { pa } else { pb };
            for node in [pa, pb] {
                if drop && node.node != high.node {
                    continue; // the low side of a fall is the fall.
                }
                let side = side_of(s, &node.node);
                stations.push(wall_side(
                    s,
                    side,
                    side_at(node),
                    node.floor,
                    format!("the approach in `{}`", node.node),
                ));
            }
            let y0 = pa.floor.max(pb.floor);
            let columns = (olo[u]..=ohi[u])
                .enumerate()
                .map(|(i, uu)| column_at(s.normal_axis, s.plane, uu, i, y0))
                .collect();
            stations.insert(
                1.min(stations.len()),
                Station {
                    key: StationKey::Plane,
                    label: "the span".to_string(),
                    deferred: false,
                    columns,
                },
            );
        }
    } else if let Some(host_id) = s.stair_in.as_ref() {
        kind = WayKind::Portal;
        let host = by_node.get(host_id.0.as_str()).copied()?;
        let other = if host.node == pa.node { pb } else { pa };
        let host_side = side_of(s, &host.node);
        let other_side = side_of(s, &other.node);
        // The run: the courses the stand-in laid, or — where a piece hosts
        // the stair — the plan's own run, which the piece's flight must join.

        let mut flight: Option<Flight> = None;
        let geom: Option<RunGeom> = match laid {
            Some(r) => {
                run = Run::Laid;
                Some(RunGeom {
                    run_axis: r.run_axis,
                    cross: r.cross,
                    start: r.start,
                    step: r.step,
                    clo: r.clo,
                    chi: r.chi,
                    body_y: r.body_y.iter().map(|y| Some(*y)).collect(),
                })
            }
            None => {
                if let Some(bp) = host_piece(&host.node)
                    && let Some(f) = piece_flight(bp, s)
                {
                    run = Run::Piece;
                    flights.insert((bp.placement.prefab_id.clone(), f.edge));
                    own.extend(f.own.clone());
                    flight = Some(f);
                } else {
                    run = Run::Undeclared;
                    census.runs_undeclared += 1;
                }
                None
            }
        };
        let cross = match (&geom, &flight) {
            (Some(g), _) => g.cross,
            (None, Some(f)) => across_of(f.travel),
            (None, None) if floor_seam => 2,
            (None, None) => across_of(s.normal_axis),
        };
        width = usize::try_from(ohi[cross] - olo[cross] + 1).unwrap_or(0);
        let index = |uu: i64| usize::try_from(uu - olo[cross]).unwrap_or(0);
        // The head: the other place's approach — beside the hole's top on
        // the upper floor, or across the wall.
        if floor_seam {
            if let Some(g) = &geom {
                let t = g.start - g.step;
                stations.push(Station {
                    key: StationKey::Side(other_side),
                    label: format!("the head's approach in `{}`", other.node),
                    deferred: false,
                    columns: (g.clo..=g.chi)
                        .map(|uu| column_at(g.run_axis, t, uu, index(uu), other.floor))
                        .collect(),
                });
            }
        } else {
            stations.push(wall_side(
                s,
                other_side,
                side_at(other),
                other.floor,
                format!("the head's approach in `{}`", other.node),
            ));
        }
        census.plane_columns += plane_cells();
        stations.push(Station {
            key: StationKey::Plane,
            label: "the opening".to_string(),
            deferred: true,
            columns: Vec::new(),
        });
        match &geom {
            Some(g) => {
                // A course's body cell is the tread's, where the run was
                // laid; over a piece's treads it is measured, between the
                // host's floor and the cell a body steps off at the top.
                let top = if floor_seam { s.plane } else { olo[1] };
                for (k, y) in g.body_y.iter().enumerate() {
                    let t = g.start + g.step * i64::try_from(k).unwrap_or(0);
                    let columns = (g.clo..=g.chi)
                        .map(|uu| {
                            let mut c = column_at(g.run_axis, t, uu, index(uu), host.floor);
                            match y {
                                Some(y) => {
                                    c.ylo = n32(*y);
                                    c.yhi = n32(*y);
                                    c.y0 = Some(n32(*y));
                                }
                                None => {
                                    c.ylo = n32(host.floor);
                                    c.yhi = n32(top);
                                    c.y0 = None;
                                }
                            }
                            c
                        })
                        .collect();
                    stations.push(Station {
                        key: StationKey::Course(u32::try_from(k).unwrap_or(0)),
                        label: format!("course {k} of the run"),
                        deferred: false,
                        columns,
                    });
                }
                let t = g.start + g.step * i64::try_from(g.body_y.len()).unwrap_or(0);
                let (flo, fhi) = if g.run_axis == 0 {
                    (host.foot[0], host.foot[1])
                } else {
                    (host.foot[2], host.foot[3])
                };
                if t >= flo && t <= fhi {
                    stations.push(Station {
                        key: StationKey::Side(host_side),
                        label: format!("the foot's approach in `{}`", host.node),
                        deferred: false,
                        columns: (g.clo..=g.chi)
                            .map(|uu| column_at(g.run_axis, t, uu, index(uu), host.floor))
                            .collect(),
                    });
                }
            }
            None if flight.is_some() => {
                if let Some(f) = &flight {
                    stations.extend(flight_stations(f, s, host_side));
                    flight_words = Some(f.words.clone());
                }
            }
            None if !floor_seam => {
                // No run the piece declares: the host's approach is still
                // judged, at its floor beside the wall.
                stations.push(wall_side(
                    s,
                    host_side,
                    side_at(host),
                    host.floor,
                    format!("the approach in `{}`", host.node),
                ));
            }
            None => {
                // Through a floor with no run declared: the head still
                // owes the hole's columns free over the upper floor.
                stations.insert(
                    0,
                    Station {
                        key: StationKey::Side(other_side),
                        label: format!("the way out of the hole into `{}`", other.node),
                        deferred: false,
                        columns: hole_columns(olo, ohi, s.plane + 1, s.plane + 2),
                    },
                );
            }
        }
    } else {
        kind = WayKind::Portal;
        census.plane_columns += plane_cells();
        if floor_seam {
            width = plane_cells();
            // `a` is the upper place when the seam is on `a`'s floor.
            let (upper, lower) = if n_dir < 0 { (pa, pb) } else { (pb, pa) };
            let cols = |ylo: i64, yhi: i64| hole_columns(olo, ohi, ylo, yhi);
            stations.push(Station {
                key: StationKey::Side(side_of(s, &upper.node)),
                label: format!("the way into the hole from `{}`", upper.node),
                deferred: false,
                columns: cols(s.plane + 1, s.plane + 2),
            });
            stations.push(Station {
                key: StationKey::Plane,
                label: "the hole".to_string(),
                deferred: true,
                columns: Vec::new(),
            });
            stations.push(Station {
                key: StationKey::Side(side_of(s, &lower.node)),
                label: format!("the way down the hole into `{}`", lower.node),
                deferred: false,
                columns: cols(lower.floor, s.plane - 1),
            });
        } else {
            let u = across_of(s.normal_axis);
            width = usize::try_from(ohi[u] - olo[u] + 1).unwrap_or(0);
            // A fall and a climb across a wall are walked on one side only:
            // the high side. Their low side is the fall, or the ladder.
            let one_sided = s.class == "drop" || s.class == "climb";
            let high = if pa.floor >= pb.floor { pa } else { pb };
            let mut sides: Vec<Station> = Vec::new();
            for node in [pa, pb] {
                if one_sided && node.node != high.node {
                    continue;
                }
                sides.push(wall_side(
                    s,
                    side_of(s, &node.node),
                    side_at(node),
                    node.floor,
                    format!("the approach in `{}`", node.node),
                ));
            }
            let plane = Station {
                key: StationKey::Plane,
                label: "the opening".to_string(),
                deferred: true,
                columns: Vec::new(),
            };
            let mut it = sides.into_iter();
            if let Some(first) = it.next() {
                stations.push(first);
            }
            stations.push(plane);
            stations.extend(it);
        }
    }
    let declared = decl_src
        .iter()
        .enumerate()
        .map(|(k, e)| {
            let path = format!("/content/seams/{seam_index}/narrowed/{k}");
            let key = match e.side {
                NarrowSide::Run => StationKey::Course(e.course.unwrap_or(0)),
                NarrowSide::Plane => StationKey::Plane,
                side => StationKey::Side(side),
            };
            let station = stations.iter().find(|st| st.key == key && !st.deferred);
            let columns: BTreeSet<usize> = (e.across[0]..=e.across[1])
                .map(|c| usize::try_from(c).unwrap_or(usize::MAX))
                .collect();
            let unplaced = match station {
                None => Some(match &key {
                    StationKey::Course(c) => {
                        format!("course {c} of a run this way does not have")
                    }
                    _ => format!("side `{}`, which this way does not have", e.side.as_str()),
                }),
                Some(st) => {
                    let have: BTreeSet<usize> = st.columns.iter().map(|c| c.index).collect();
                    columns.iter().find(|c| !have.contains(c)).map(|c| {
                        format!(
                            "column {c} of {}, which that station does not have",
                            st.label
                        )
                    })
                }
            };
            Declared {
                station: station.map(|st| st.key.clone()),
                columns,
                unplaced,
                block: e.block.clone(),
                what: e.what.clone(),
                path,
            }
        })
        .collect();
    let label = match &flight_words {
        Some(w) => format!("{label}, its run the host piece's flight {w}"),
        None => label,
    };
    Some(Way {
        kind,
        name: s.edge.0.clone(),
        label,
        width,
        stations,
        own,
        declared,
        frame: Frame::Seam(s.edge.0.clone()),
        run,
    })
}

/// A floor hole's columns between two heights, numbered along `x`, then `z`
/// — a vertical station, every cell of which a body passes through.
fn hole_columns(olo: [i64; 3], ohi: [i64; 3], ylo: i64, yhi: i64) -> Vec<Column> {
    (olo[2]..=ohi[2])
        .flat_map(|z| (olo[0]..=ohi[0]).map(move |x| (x, z)))
        .enumerate()
        .map(|(i, (x, z))| Column {
            index: i,
            x: n32(x),
            z: n32(z),
            ylo: n32(ylo),
            yhi: n32(yhi),
            y0: None,
            vertical: true,
        })
        .collect()
}

/// A bound place's piece: its placement and its contract.
pub struct BoundPiece<'a> {
    /// The piece's placement.
    pub placement: &'a crate::compiler::plan::PiecePlacement,
    /// Its spatial contract.
    pub contract: &'a delvewright_dsl::prefab::SpatialContract,
}

/// Every place a detail plan binds, with the piece placed for it.
fn bound_pieces<'a>(
    plan: &'a Plan,
    prefabs: &'a PrefabRegistry,
) -> BTreeMap<&'a str, BoundPiece<'a>> {
    let mut out = BTreeMap::new();
    let Some(dp) = plan.campaign.detail_plan.as_ref() else {
        return out;
    };
    for d in &dp.content.details {
        let Some(contract) = prefabs
            .get(&d.piece.0)
            .and_then(|m| m.spatial_contract.as_ref())
        else {
            continue;
        };
        let Some(placement) = plan
            .areas
            .iter()
            .flat_map(|a| a.pieces.iter())
            .find(|p| p.prefab_id == d.piece.0)
        else {
            continue;
        };
        out.insert(
            d.place.0.as_str(),
            BoundPiece {
                placement,
                contract,
            },
        );
    }
    out
}

/// A contract region's cells in world coordinates, through the placement.
fn region_world(
    p: &crate::compiler::plan::PiecePlacement,
    boxes: &[delvewright_dsl::prefab::Region],
) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for r in boxes {
        let a = world_cell(p.rotation, p.pos, r.from);
        let b = world_cell(p.rotation, p.pos, r.to);
        out.extend(crate::compiler::assembled::region_cells(a, b));
    }
    out
}

/// A contract edge's own regions — its `bar` and its `way` — each cell with
/// the state opening leaves it in (`None` = air; a laid way's block).
fn own_regions(
    p: &crate::compiler::plan::PiecePlacement,
    e: &delvewright_dsl::prefab::ContractEdge,
) -> BTreeMap<[i32; 3], Option<String>> {
    let mut own = BTreeMap::new();
    if let Some(bar) = &e.bar {
        for c in region_world(p, &bar.boxes) {
            own.insert(c, None);
        }
    }
    if let Some(w) = &e.way {
        let open = (w.opens == "laid").then(|| w.block.clone());
        for c in region_world(p, &w.boxes) {
            own.insert(c, open.clone());
        }
    }
    own
}

/// A stair run's geometry: the axis it walks, the one across it, where
/// course 0 stands and which way the run walks back, the columns its treads
/// take, and per course the body cell over the tread when the run was laid.
struct RunGeom {
    run_axis: usize,
    cross: usize,
    start: i64,
    step: i64,
    clo: i64,
    chi: i64,
    body_y: Vec<Option<i64>>,
}

/// A host piece's flight for a stair seam (spec-0120 §1.3): the contract's
/// `stair` edge whose `via` the contract's own graph joins to the face
/// answering the seam.
struct Flight {
    /// The edge's index in the contract.
    edge: usize,
    /// The edge in words.
    words: String,
    /// Its `via`, in world cells.
    cells: BTreeSet<[i32; 3]>,
    /// Its own regions, opened.
    own: BTreeMap<[i32; 3], Option<String>>,
    /// The axis its `via` separates its two spaces on.
    travel: usize,
    /// The space at the face, and the other endpoint, with their cells.
    head: (String, BTreeSet<[i32; 3]>),
    foot: (String, BTreeSet<[i32; 3]>),
}

/// **The host piece's flight joins the face**: the exterior edge whose region
/// meets the seam's opening — a cell of it at most one cell off the plane, at
/// in-plane coordinates the opening holds — names the space at the face, and
/// a `stair` edge from that space to another of the piece's spaces is the
/// flight. `None` when the contract declares none: an exterior `stair` edge's
/// own `via` is the face, not a run.
fn piece_flight(p: &BoundPiece<'_>, s: &PlacedSeam) -> Option<Flight> {
    let (olo, ohi) = s.opening;
    let n = s.normal_axis;
    let meets = |c: &[i32; 3]| {
        (0..3).all(|i| {
            let v = i64::from(c[i]);
            if i == n {
                (v - s.plane).abs() <= 1
            } else {
                olo[i] <= v && v <= ohi[i]
            }
        })
    };
    fn region_of(
        e: &delvewright_dsl::prefab::ContractEdge,
    ) -> Option<&[delvewright_dsl::prefab::Region]> {
        e.via
            .as_ref()
            .map(|v| v.boxes.as_slice())
            .or_else(|| e.bar.as_ref().map(|b| b.boxes.as_slice()))
    }
    let face_space = p.contract.edges.iter().find_map(|e| {
        let space = if e.a == "exterior" {
            &e.b
        } else if e.b == "exterior" {
            &e.a
        } else {
            return None;
        };
        let cells = region_world(p.placement, region_of(e)?);
        cells.iter().any(meets).then(|| space.clone())
    })?;
    let spaces: BTreeMap<&str, BTreeSet<[i32; 3]>> = p
        .contract
        .spaces
        .iter()
        .map(|(k, v)| (k.as_str(), region_world(p.placement, &v.boxes)))
        .collect();
    for (i, e) in p.contract.edges.iter().enumerate() {
        if e.class != "stair" || e.a == "exterior" || e.b == "exterior" {
            continue;
        }
        let other = if e.a == face_space {
            &e.b
        } else if e.b == face_space {
            &e.a
        } else {
            continue;
        };
        let Some(via) = &e.via else { continue };
        let cells = region_world(p.placement, &via.boxes);
        if cells.is_empty() {
            continue;
        }
        let travel = travel_axis(p.placement, p.contract, e, &cells, &spaces);
        return Some(Flight {
            edge: i,
            words: format!("{}--{}--{}", e.a, e.class, e.b),
            own: own_regions(p.placement, e),
            travel,
            head: (
                face_space.clone(),
                spaces.get(face_space.as_str()).cloned().unwrap_or_default(),
            ),
            foot: (
                other.clone(),
                spaces.get(other.as_str()).cloned().unwrap_or_default(),
            ),
            cells,
        });
    }
    None
}

/// A piece flight's stations: its region's layers as courses, top first (the
/// end its head space stands at), and the approach in its foot space.
fn flight_stations(f: &Flight, s: &PlacedSeam, host_side: NarrowSide) -> Vec<Station> {
    let cross = across_of(f.travel);
    let u0 = n32(s.opening.0[cross]);
    let ends = [
        (f.head.0.as_str(), &f.head.1),
        (f.foot.0.as_str(), &f.foot.1),
    ];
    let (lo_end, region, hi_end) = region_stations(&f.cells, f.travel, u0, &ends);
    let head_name = format!("the approach in `{}`", f.head.0);
    // Which end is the head: the approach that lies in the head space, else
    // the end nearer the plane.
    let head_low = match (&lo_end, &hi_end) {
        (Some(l), _) if l.label == head_name => true,
        (_, Some(h)) if h.label == head_name => false,
        _ => {
            let t0 = region.first().map_or(0, |st| {
                st.columns
                    .first()
                    .map_or(0, |c| if f.travel == 0 { c.x } else { c.z })
            });
            let t1 = region.last().map_or(0, |st| {
                st.columns
                    .first()
                    .map_or(0, |c| if f.travel == 0 { c.x } else { c.z })
            });
            let plane = n32(s.plane);
            (t0 - plane).abs() <= (t1 - plane).abs()
        }
    };
    let (mut courses, foot) = if head_low {
        (region, hi_end)
    } else {
        (region.into_iter().rev().collect(), lo_end)
    };
    for (k, st) in courses.iter_mut().enumerate() {
        st.key = StationKey::Course(u32::try_from(k).unwrap_or(0));
        st.label = format!("course {k} of the run");
    }
    let mut out = courses;
    if let Some(mut st) = foot.filter(|st| st.label != head_name) {
        st.key = StationKey::Side(host_side);
        st.label = format!("the foot's approach in `{}`", f.foot.0);
        out.push(st);
    }
    out
}

/// Every way place: a box that declares `way`.
fn place_ways(plan: &Plan, census: &mut Census, cover: &BTreeSet<[i32; 3]>, out: &mut Vec<Way>) {
    let Some(b) = plan.blockout.as_ref() else {
        return;
    };
    let Some(site) = plan.campaign.site_plan.as_ref().map(|p| &p.content) else {
        return;
    };
    census.boxes += b.boxes.len();
    for (i, pb) in site.boxes.iter().enumerate() {
        let Some(way) = pb.way.as_ref() else { continue };
        let Some(placed) = b.boxes.iter().find(|x| x.node == pb.node) else {
            continue;
        };
        let axis = match way.axis {
            delvewright_dsl::siteplan::PlanAxis::X => 0usize,
            delvewright_dsl::siteplan::PlanAxis::Z => 2usize,
        };
        let cross = across_of(axis);
        let span = |a: usize| {
            if a == 0 {
                (placed.foot[0], placed.foot[1])
            } else {
                (placed.foot[2], placed.foot[3])
            }
        };
        let (t0, t1) = span(axis);
        let (c0, c1) = span(cross);
        let at = way_at(way, c1 - c0 + 1);
        let clear = i64::from(way.clear.get());
        let mut stations = Vec::new();
        for t in t0..=t1 {
            let columns: Vec<Column> = (0..clear)
                .filter_map(|k| {
                    let col = column_at(
                        axis,
                        t,
                        c0 + at + k,
                        usize::try_from(k).unwrap_or(0),
                        placed.floor,
                    );
                    (!cover.contains(&[col.x, n32(placed.floor), col.z])).then_some(col)
                })
                .collect();
            let along = u32::try_from(t - t0).unwrap_or(0);
            stations.push(Station {
                key: StationKey::Along(along),
                label: format!("{along} cell(s) along `{}`", pb.node),
                deferred: false,
                columns,
            });
        }
        let declared = pb
            .narrowed
            .iter()
            .enumerate()
            .map(|(k, e)| {
                let key = StationKey::Along(e.along);
                let station = stations.iter().find(|s| s.key == key);
                let columns: BTreeSet<usize> = (e.across[0]..=e.across[1])
                    .map(|c| usize::try_from(c).unwrap_or(usize::MAX))
                    .collect();
                let unplaced = match station {
                    None => Some(format!(
                        "station {} along a way {} cell(s) long",
                        e.along,
                        t1 - t0 + 1
                    )),
                    Some(st) => {
                        let have: BTreeSet<usize> = st.columns.iter().map(|c| c.index).collect();
                        columns.iter().find(|c| !have.contains(c)).map(|c| {
                            format!(
                                "column {c} of {}, which that station does not judge",
                                st.label
                            )
                        })
                    }
                };
                Declared {
                    station: station.map(|s| s.key.clone()),
                    columns,
                    unplaced,
                    block: e.block.clone(),
                    what: e.what.clone(),
                    path: format!("/content/boxes/{i}/narrowed/{k}"),
                }
            })
            .collect();
        out.push(Way {
            kind: WayKind::Place,
            name: pb.node.0.clone(),
            label: format!("the way `{}`", pb.node),
            width: usize::try_from(clear).unwrap_or(0),
            stations,
            own: BTreeMap::new(),
            declared,
            frame: Frame::Place(pb.node.0.clone()),
            run: Run::None,
        });
    }
}

/// The traversal and barred classes a piece way may be.
const PIECE_CLASSES: [&str; 5] = ["walk", "stair", "climb", "drop", "barred"];

/// Every piece way: a placed piece's contract edge that answers no plan seam,
/// and the refusal for a `narrowed` on one that does.
fn piece_ways(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    flights: &BTreeSet<(String, usize)>,
    census: &mut Census,
    out: &mut Vec<Way>,
    refusals: &mut Vec<Failure>,
) {
    let placed: Vec<(
        &crate::compiler::plan::PiecePlacement,
        &delvewright_dsl::prefab::SpatialContract,
    )> = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .filter_map(|p| {
            prefabs
                .get(&p.prefab_id)
                .and_then(|m| m.spatial_contract.as_ref())
                .map(|c| (p, c))
        })
        .collect();
    piece_ways_over(
        &placed,
        plan.campaign.site_plan.is_some(),
        flights,
        census,
        out,
        refusals,
    );
}

/// [`piece_ways`] over placed pieces and their contracts: every edge of the
/// five classes counted, every one that answers no plan seam judged, and a
/// `narrowed` on one that does refused (`DW0888`). `site_plan` says whether
/// every exterior edge answers a plan seam; `flights` names the edges a stair
/// seam took as its run.
pub fn piece_ways_over(
    placed: &[(
        &crate::compiler::plan::PiecePlacement,
        &delvewright_dsl::prefab::SpatialContract,
    )],
    site_plan: bool,
    flights: &BTreeSet<(String, usize)>,
    census: &mut Census,
    out: &mut Vec<Way>,
    refusals: &mut Vec<Failure>,
) {
    {
        for (p, contract) in placed.iter().copied() {
            let space_cells: BTreeMap<&str, BTreeSet<[i32; 3]>> = contract
                .spaces
                .iter()
                .map(|(n, sp)| (n.as_str(), region_world(p, &sp.boxes)))
                .collect();
            for (ei, e) in contract.edges.iter().enumerate() {
                if !PIECE_CLASSES.contains(&e.class.as_str()) {
                    continue;
                }
                census.edges += 1;
                let edge_words = format!("{}--{}--{}", e.a, e.class, e.b);
                let exterior = e.a == "exterior" || e.b == "exterior";
                let realizes = flights.contains(&(p.prefab_id.clone(), ei));
                if (site_plan && exterior) || realizes {
                    if !e.narrowed.is_empty() {
                        refusals.push(Failure {
                            code: crate::compiler::claims::DW_CLAIM_DENIED,
                            message: format!(
                                "the contract edge {edge_words} of `{}` declares {} narrowing(s), \
                                 and that edge realizes a plan seam: its way is the seam's, and \
                                 the seam's `narrowed[]` is the one authority for what narrows \
                                 it. Move the declaration to the seam in `site-plan.json` and \
                                 remove it from the piece.",
                                p.prefab_id,
                                e.narrowed.len()
                            ),
                        });
                    }
                    continue;
                }
                let region = if e.class == "barred" {
                    e.bar.as_ref().map(|b| &b.boxes)
                } else {
                    e.via.as_ref().map(|v| &v.boxes)
                };
                let Some(region) = region else { continue };
                let cells = region_world(p, region);
                if cells.is_empty() {
                    continue;
                }
                let own = own_regions(p, e);
                let travel = travel_axis(p, contract, e, &cells, &space_cells);
                let ends: Vec<(&str, &BTreeSet<[i32; 3]>)> = [e.a.as_str(), e.b.as_str()]
                    .into_iter()
                    .filter_map(|n| space_cells.get(n).map(|c| (n, c)))
                    .collect();
                let u0 = if travel == 1 {
                    0
                } else {
                    cells
                        .iter()
                        .map(|c| c[across_of(travel)])
                        .min()
                        .unwrap_or(0)
                };
                let (lo_end, region, hi_end) = region_stations(&cells, travel, u0, &ends);
                let mut stations: Vec<Station> = Vec::new();
                stations.extend(lo_end);
                stations.extend(region);
                stations.extend(hi_end);
                let width = stations.iter().map(|s| s.columns.len()).max().unwrap_or(0);
                let declared = e
                    .narrowed
                    .iter()
                    .enumerate()
                    .map(|(k, d)| piece_declared(p, &stations, travel, &edge_words, k, d))
                    .collect();
                out.push(Way {
                    kind: WayKind::Piece,
                    name: format!("{} {edge_words}", p.prefab_id),
                    label: format!("the piece way {edge_words} of `{}`", p.prefab_id),
                    width,
                    stations,
                    own,
                    declared,
                    frame: Frame::Piece {
                        prefab: p.prefab_id.clone(),
                        edge: edge_words.clone(),
                        pos: p.pos,
                        rotation: p.rotation,
                    },
                    run: Run::None,
                });
            }
        }
    }
}

/// A region's stations along its travel axis: one per layer of the region,
/// and one approach cell beyond each end that lies in one of `ends` (the
/// edge's endpoint spaces this piece owns). A vertical region (`travel` 1) is
/// one station of whole columns, every cell of which a body passes through.
/// Column indices count from `u0` across the travel.
fn region_stations(
    cells: &BTreeSet<[i32; 3]>,
    travel: usize,
    u0: i32,
    ends: &[(&str, &BTreeSet<[i32; 3]>)],
) -> (Option<Station>, Vec<Station>, Option<Station>) {
    if travel == 1 {
        let mut cols: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
        for c in cells {
            let k = cols.entry((c[0], c[2])).or_insert((c[1], c[1]));
            k.0 = k.0.min(c[1]);
            k.1 = k.1.max(c[1]);
        }
        let columns = cols
            .iter()
            .enumerate()
            .map(|(i, ((x, z), (lo, hi)))| Column {
                index: i,
                x: *x,
                z: *z,
                ylo: *lo,
                yhi: *hi,
                y0: None,
                vertical: true,
            })
            .collect();
        let st = Station {
            key: StationKey::Piece(0),
            label: "the way's region".to_string(),
            deferred: false,
            columns,
        };
        return (None, vec![st], None);
    }
    let cross = across_of(travel);
    let mut cols: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
    for c in cells {
        let k = cols.entry((c[travel], c[cross])).or_insert((c[1], c[1]));
        k.0 = k.0.min(c[1]);
        k.1 = k.1.max(c[1]);
    }
    let t0 = cells.iter().map(|c| c[travel]).min().unwrap_or(0);
    let t1 = cells.iter().map(|c| c[travel]).max().unwrap_or(0);
    let make = |t: i32, u: i32, lo: i32, hi: i32| {
        let (x, z) = if travel == 0 { (t, u) } else { (u, t) };
        Column {
            index: usize::try_from(u - u0).unwrap_or(0),
            x,
            z,
            ylo: lo,
            yhi: hi,
            y0: None,
            vertical: false,
        }
    };
    let approach = |t: i32, from: i32| -> Option<Station> {
        let columns: Vec<Column> = cols
            .range((from, i32::MIN)..=(from, i32::MAX))
            .map(|((_, u), (lo, _))| make(t, *u, *lo, *lo + 1))
            .collect();
        let (space, _) = ends.iter().find(|(_, sc)| {
            columns
                .iter()
                .any(|c| (c.ylo..=c.yhi).any(|y| sc.contains(&[c.x, y, c.z])))
        })?;
        Some(Station {
            key: StationKey::Piece(t),
            label: format!("the approach in `{space}`"),
            deferred: false,
            columns,
        })
    };
    let region = (t0..=t1)
        .map(|t| Station {
            key: StationKey::Piece(t),
            label: format!("the way's region at {} {t}", ["x", "y", "z"][travel]),
            deferred: false,
            columns: cols
                .range((t, i32::MIN)..=(t, i32::MAX))
                .map(|((_, u), (lo, hi))| make(t, *u, *lo, *hi))
                .collect(),
        })
        .collect();
    (approach(t0 - 1, t0), region, approach(t1 + 1, t1))
}

fn piece_declared(
    p: &crate::compiler::plan::PiecePlacement,
    stations: &[Station],
    travel: usize,
    edge_words: &str,
    k: usize,
    d: &delvewright_dsl::prefab::EdgeNarrowing,
) -> Declared {
    let path = format!("{} edge {edge_words} narrowed[{k}]", p.prefab_id);
    let mut station: Option<StationKey> = None;
    let mut columns = BTreeSet::new();
    let mut unplaced = None;
    for l in &d.cells {
        let w = world_cell(p.rotation, p.pos, *l);
        let hit = stations.iter().find_map(|s| {
            s.columns
                .iter()
                .find(|c| {
                    c.x == w[0]
                        && c.z == w[2]
                        && (travel == 1 || (c.ylo - 1..=c.yhi + 1).contains(&w[1]))
                })
                .map(|c| (s.key.clone(), c.index))
        });
        match hit {
            Some((key, idx)) if station.as_ref().is_none_or(|s| *s == key) => {
                station = Some(key);
                columns.insert(idx);
            }
            Some(_) => {
                unplaced = Some(format!(
                    "cells at more than one station of the way ([{}, {}, {}] among them)",
                    l[0], l[1], l[2]
                ));
            }
            None => {
                unplaced = Some(format!(
                    "the piece cell [{}, {}, {}], which is in no column of the way",
                    l[0], l[1], l[2]
                ));
            }
        }
    }
    Declared {
        station,
        columns,
        unplaced,
        block: d.block.clone(),
        what: d.what.clone(),
        path,
    }
}

/// The axis a piece way travels: the face its exterior edge reaches, or the
/// horizontal axis on which its region separates its two spaces; `y` where no
/// horizontal axis does.
fn travel_axis(
    p: &crate::compiler::plan::PiecePlacement,
    contract: &delvewright_dsl::prefab::SpatialContract,
    e: &delvewright_dsl::prefab::ContractEdge,
    cells: &BTreeSet<[i32; 3]>,
    spaces: &BTreeMap<&str, BTreeSet<[i32; 3]>>,
) -> usize {
    let exterior = e.a == "exterior" || e.b == "exterior";
    if exterior {
        let space = if e.a == "exterior" { &e.b } else { &e.a };
        if let Some(f) = contract
            .faces
            .iter()
            .find(|f| f.space == *space && f.class == e.class)
        {
            let local = match f.dir.as_str() {
                "east" | "west" => [1, 0, 0],
                "south" | "north" => [0, 0, 1],
                _ => [0, 1, 0],
            };
            let t = p.rotation.transform(local);
            return (0..3).find(|i| t[*i] != 0).unwrap_or(1);
        }
    }
    let (Some(a), Some(b)) = (spaces.get(e.a.as_str()), spaces.get(e.b.as_str())) else {
        return 1;
    };
    let footprint: BTreeSet<(i32, i32)> = cells.iter().map(|c| (c[0], c[2])).collect();
    let mut best: Option<(usize, usize)> = None;
    for axis in [0usize, 2] {
        // A space abuts the region one cell along the axis, beyond the
        // region's own footprint, at the region's course or a course above or
        // below it: a stair's top tread meets its landing a course up.
        let touches = |s: &BTreeSet<[i32; 3]>, d: i32| {
            cells
                .iter()
                .filter(|c| {
                    (-1..=1).any(|dy| {
                        let mut n = **c;
                        n[axis] += d;
                        n[1] += dy;
                        !footprint.contains(&(n[0], n[2])) && s.contains(&n)
                    })
                })
                .count()
        };
        let (la, hb, lb, ha) = (touches(a, -1), touches(b, 1), touches(b, -1), touches(a, 1));
        let score = if la > 0 && hb > 0 {
            la + hb
        } else if lb > 0 && ha > 0 {
            lb + ha
        } else {
            0
        };
        if score > 0 && best.is_none_or(|(_, s)| score > s) {
            best = Some((axis, score));
        }
    }
    match best {
        Some((a, _)) => a,
        // A stair travels across the ground whatever its spaces say: the
        // longer of its region's two horizontal extents.
        None if e.class == "stair" => {
            let ext = |a: usize| {
                let lo = cells.iter().map(|c| c[a]).min().unwrap_or(0);
                let hi = cells.iter().map(|c| c[a]).max().unwrap_or(0);
                hi - lo
            };
            if ext(0) > ext(2) { 0 } else { 2 }
        }
        None => 1,
    }
}

/// Every runtime write a cell of a way can take, as a state: each effect's
/// region write, the region model's laid writes and clears, and both phases
/// of every timed gate. Sorted and deduplicated (ADR-0006).
#[must_use]
pub fn state_writes(plan: &Plan) -> Vec<StateWrite> {
    let gate_of = |region: ([i32; 3], [i32; 3])| -> Option<String> {
        plan.anchors.iter().find_map(|((_, n), r)| match r {
            ResolvedAnchor::Gate { from, to, .. } if (*from, *to) == region => Some(n.clone()),
            _ => None,
        })
    };
    let mut out: BTreeSet<StateWrite> = BTreeSet::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
        for (region, _, block) in crate::compiler::plan::effect_writes(e, &plan.anchors, &plan.ways)
        {
            let label = match (gate_of(region), &block) {
                (Some(g), Some(_)) => format!("while `{g}` is shut"),
                (Some(g), None) => format!("once `{g}` opens"),
                (None, _) => format!("after `{}` at `{}`", e.verb.tag(), site.path),
            };
            out.insert(StateWrite {
                region,
                block,
                label,
            });
        }
    });
    for ev in plan.region_events.iter() {
        let block = ev.block().map(str::to_string);
        if block.is_none() && ev.write.fills() {
            continue; // a world-load seal: the shipped bytes already hold it.
        }
        let label = match (gate_of(ev.region), &block) {
            (Some(g), Some(_)) => format!("while `{g}` is shut"),
            (Some(g), None) => format!("once `{g}` opens"),
            (None, Some(b)) => format!(
                "once a runtime write lays `{b}` at critical-path step {}",
                ev.fire_step
            ),
            (None, None) => format!(
                "once a runtime write clears it at critical-path step {}",
                ev.fire_step
            ),
        };
        out.insert(StateWrite {
            region: ev.region,
            block,
            label,
        });
    }
    for g in &plan.timed_gates {
        out.insert(StateWrite {
            region: g.gate_region,
            block: Some(g.gate_block.clone()),
            label: format!("while timed gate `{}` is closed", g.id),
        });
        out.insert(StateWrite {
            region: g.gate_region,
            block: None,
            label: format!("while timed gate `{}` is open", g.id),
        });
    }
    // One state per (region, block): the first label in order names it.
    let mut seen: BTreeSet<(Region3, Option<String>)> = BTreeSet::new();
    out.into_iter()
        .filter(|w| seen.insert((w.region, w.block.clone())))
        .collect()
}

// ---------------------------------------------------------------------------
// The binding and the entry point
// ---------------------------------------------------------------------------

/// What the clear-width check judged, whether or not it refused.
#[derive(Debug, Clone, Default)]
pub struct WidthBinding {
    /// The census.
    pub census: Census,
    /// Every way, judged.
    pub records: Vec<WayRecord>,
    /// Distinct states judged across every way, as built included.
    pub states: usize,
}

impl WidthBinding {
    fn count(&self, k: WayKind) -> usize {
        self.records.iter().filter(|r| r.way.kind == k).count()
    }

    /// The one line every build prints.
    #[must_use]
    pub fn line(&self) -> String {
        let c = &self.census;
        let runs = self
            .records
            .iter()
            .filter(|r| matches!(r.way.run, Run::Laid | Run::Piece))
            .count();
        let columns: usize = self
            .records
            .iter()
            .flat_map(|r| r.way.stations.iter())
            .filter(|s| !s.deferred)
            .map(|s| s.columns.len())
            .sum();
        let stations = self
            .records
            .iter()
            .flat_map(|r| r.way.stations.iter())
            .filter(|s| !s.deferred && !s.columns.is_empty())
            .count();
        let whole = self
            .records
            .iter()
            .filter(|r| r.verdict() == "whole")
            .count();
        let undeclared = self
            .records
            .iter()
            .filter(|r| r.undeclared.is_some())
            .count();
        let as_declared = self
            .records
            .iter()
            .filter(|r| r.verdict() == "narrowed-as-declared")
            .count();
        let excused: usize = self.records.iter().map(|r| r.excused).sum();
        let confirmed: usize = self.records.iter().map(|r| r.confirmed).sum();
        let not_shown: usize = self.records.iter().map(|r| r.not_shown.len()).sum();
        format!(
            "clear width (DW1057/DW1058): {w} way(s) of {t} traversal seam(s), {b} box(es) and {e} \
             contract edge(s) — {p} portal(s), {s} stair run(s), {cc} contact(s), {y} way \
             place(s), {k} piece way(s); {m} column(s) at {n} station(s) judged over {v} \
             state(s); {h} way(s) whole, {u} narrowed undeclared, {d} narrowed as declared ({g} \
             column(s) excused by {f} declaration(s)), {x} declaration(s) not shown; {l} plane \
             column(s) left to DW0836; {r} stair run(s) undeclared by their piece",
            w = self.records.len(),
            t = c.seams,
            b = c.boxes,
            e = c.edges,
            p = self.count(WayKind::Portal),
            s = runs,
            cc = self.count(WayKind::Contact),
            y = self.count(WayKind::Place),
            k = self.count(WayKind::Piece),
            m = columns,
            n = stations,
            v = self.states,
            h = whole,
            u = undeclared,
            d = as_declared,
            g = excused,
            f = confirmed,
            x = not_shown,
            l = c.plane_columns,
            r = c.runs_undeclared,
        )
    }

    /// The ledger `validation/clear-width.json`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let c = &self.census;
        json!({
            "codes": [DW_WAY_NARROWED.id(), DW_NARROWING_NOT_SHOWN.id()],
            "line": self.line(),
            "ways": self.records.len(),
            "traversal_seams": c.seams,
            "boxes": c.boxes,
            "contract_edges": c.edges,
            "portals": self.count(WayKind::Portal),
            "contacts": self.count(WayKind::Contact),
            "way_places": self.count(WayKind::Place),
            "piece_ways": self.count(WayKind::Piece),
            "states": self.states,
            "plane_columns_left_to_dw0836": c.plane_columns,
            "stair_runs_undeclared": c.runs_undeclared,
            "records": self.records.iter().map(WayRecord::to_json).collect::<Vec<_>>(),
        })
    }

    /// Every refusal, `DW1057` first, in way order.
    #[must_use]
    pub fn refusals(&self) -> Vec<&Failure> {
        let mut out: Vec<&Failure> = self
            .records
            .iter()
            .filter_map(|r| r.undeclared.as_ref())
            .collect();
        out.extend(self.records.iter().flat_map(|r| r.not_shown.iter()));
        out
    }
}

/// Every way of the plan, built and not yet judged, with the census and the
/// refusals the population itself raises.
#[must_use]
pub fn population(plan: &Plan, prefabs: &PrefabRegistry) -> (Vec<Way>, Census, Vec<Failure>) {
    let mut census = Census::default();
    let mut ways = Vec::new();
    let mut refusals = Vec::new();
    let mut flights: BTreeSet<(String, usize)> = BTreeSet::new();
    seam_ways(plan, prefabs, &mut census, &mut flights, &mut ways);
    // A station a seam's approach covers is judged once, under the seam.
    let cover: BTreeSet<[i32; 3]> = ways
        .iter()
        .flat_map(|w| w.stations.iter())
        .filter(|s| !s.deferred)
        .flat_map(|s| s.columns.iter())
        .filter_map(|c| c.y0.map(|y| [c.x, y, c.z]))
        .collect();
    place_ways(plan, &mut census, &cover, &mut ways);
    piece_ways(
        plan,
        prefabs,
        &flights,
        &mut census,
        &mut ways,
        &mut refusals,
    );
    (ways, census, refusals)
}

/// **The clear-width check** (spec-0120 §5) over the built world: every way of
/// the plan judged in every state its cells can take. The binding is returned
/// beside the first refusal; the caller prints the line before the verdict.
#[must_use]
pub fn check(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    assembled: &crate::compiler::assembled::Assembled,
    fixtures: &[crate::compiler::light::Placement],
) -> (WidthBinding, Vec<Failure>) {
    let (ways, census, mut refusals) = population(plan, prefabs);
    let shipped = crate::compiler::assembled::shipped_blocks(
        plan,
        &assembled.blocks,
        fixtures,
        &assembled.gate_seals,
        None,
    );
    let writes = state_writes(plan);
    let sealed: Vec<NamedRegion> = assembled
        .gate_seals
        .iter()
        .filter(|s| s.sealed())
        .map(|s| (s.anchor.clone(), s.region))
        .collect();
    let mut seated: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for f in fixtures {
        seated.insert(
            f.pos,
            "the relight fixture the build sets at setup".to_string(),
        );
    }
    for (id, prop) in crate::compiler::pressable::trigger_props(plan) {
        seated.insert(prop.pos, format!("the prop of `{id}`"));
    }
    let ground = Ground {
        shipped: &shipped,
        open_gates: &assembled.open_gates,
        writes: &writes,
        sealed: &sealed,
        seated: &seated,
    };
    let records: Vec<WayRecord> = ways.into_iter().map(|w| judge(w, &ground)).collect();
    let used: BTreeSet<&StateWrite> = writes
        .iter()
        .filter(|w| {
            records.iter().any(|r| {
                r.way
                    .stations
                    .iter()
                    .filter(|s| !s.deferred)
                    .flat_map(|s| s.columns.iter())
                    .any(|c| {
                        let (a, b) = column_reach(c);
                        w.meets(a, b)
                    })
            })
        })
        .collect();
    let binding = WidthBinding {
        census,
        states: used.len() + 1,
        records,
    };
    let mut out: Vec<Failure> = binding.refusals().into_iter().cloned().collect();
    out.append(&mut refusals);
    (binding, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STONE: &str = "minecraft:stone";
    const MASONRY: &str = "minecraft:stone_bricks";
    const LOG: &str = "minecraft:stripped_spruce_log[axis=y]";

    /// A stone floor at y 0 over x 0..=20, z 0..=20.
    fn floor() -> BlockMap {
        let mut m = BlockMap::new();
        for x in 0..=20 {
            for z in 0..=20 {
                m.insert([x, 0, z], BlockState::new(STONE));
            }
        }
        m
    }

    fn put(m: &mut BlockMap, cells: impl IntoIterator<Item = [i32; 3]>, b: &str) {
        for c in cells {
            m.insert(c, BlockState::new(b));
        }
    }

    /// The yard's foot (spec-0120 §2): a three-wide approach row at x 10,
    /// z 12..=14, the body cell at y 1.
    fn foot() -> Station {
        Station {
            key: StationKey::Side(NarrowSide::B),
            label: "the foot's approach in `node/yard`".to_string(),
            deferred: false,
            columns: (12..=14)
                .enumerate()
                .map(|(i, z)| Column {
                    index: i,
                    x: 10,
                    z,
                    ylo: 1,
                    yhi: 1,
                    y0: Some(1),
                    vertical: false,
                })
                .collect(),
        }
    }

    fn way(stations: Vec<Station>, declared: Vec<Declared>) -> Way {
        Way {
            kind: WayKind::Portal,
            name: "edge/keeper-stair".to_string(),
            label: "the stair `edge/keeper-stair`".to_string(),
            width: 3,
            stations,
            own: BTreeMap::new(),
            declared,
            frame: Frame::Seam("edge/keeper-stair".to_string()),
            run: Run::Laid,
        }
    }

    fn decl(cols: &[usize], block: &str) -> Declared {
        Declared {
            station: Some(StationKey::Side(NarrowSide::B)),
            columns: cols.iter().copied().collect(),
            unplaced: None,
            block: block.to_string(),
            what: "the jamb of the cart road's mouth".to_string(),
            path: "/content/seams/0/narrowed/0".to_string(),
        }
    }

    const BARRICADE: Region3 = ([10, 1, 13], [12, 3, 13]);

    fn judge_over(
        w: Way,
        m: &BlockMap,
        writes: &[StateWrite],
        sealed: &[NamedRegion],
    ) -> WayRecord {
        let open = BTreeSet::new();
        let seated = BTreeMap::new();
        judge(
            w,
            &Ground {
                shipped: m,
                open_gates: &open,
                writes,
                sealed,
                seated: &seated,
            },
        )
    }

    fn opens_barricade() -> Vec<StateWrite> {
        vec![StateWrite {
            region: BARRICADE,
            block: None,
            label: "once `anchor/barricade` opens".to_string(),
        }]
    }

    fn sealed_barricade() -> Vec<NamedRegion> {
        vec![("anchor/barricade".to_string(), BARRICADE)]
    }

    /// The finding's shape (criterion 5): one approach column under fixed
    /// masonry, one under a foreign gate region — `1 of 3` shut, `2 of 3`
    /// open, both cells named with their blocks and states.
    fn yard() -> BlockMap {
        let mut m = floor();
        put(&mut m, (1..=3).map(|y| [10, y, 14]), MASONRY);
        put(
            &mut m,
            (10..=12).flat_map(|x| (1..=3).map(move |y| [x, y, 13])),
            LOG,
        );
        m
    }

    #[test]
    fn the_yards_foot_is_one_of_three_shut_and_two_of_three_open() {
        let rec = judge_over(
            way(vec![foot()], Vec::new()),
            &yard(),
            &opens_barricade(),
            &sealed_barricade(),
        );
        let f = rec
            .undeclared
            .as_ref()
            .expect("the foot is narrowed undeclared");
        assert_eq!(f.code.id(), "DW1057");
        let m = &f.message;
        assert!(m.contains("1 of 3"), "{m}");
        assert!(m.contains("2 of 3"), "{m}");
        assert!(
            m.contains("[10, 1, 14] `minecraft:stone_bricks` (as built)"),
            "{m}"
        );
        assert!(
            m.contains("[10, 1, 13] `minecraft:stripped_spruce_log[axis=y]` (while `anchor/barricade` is shut)"),
            "{m}"
        );
        assert!(m.contains("\"side\": \"b\""), "{m}");
        assert!(m.contains("\"what\": \"…\""), "{m}");
        assert_eq!(rec.stations[0].clear, vec![1, 2]);
        assert_eq!(rec.verdict(), "narrowed");
    }

    #[test]
    fn the_yards_foot_with_both_moved_one_column_outward_is_whole() {
        let mut m = floor();
        put(&mut m, (1..=3).map(|y| [10, y, 15]), MASONRY);
        put(
            &mut m,
            (10..=12).flat_map(|x| (1..=3).map(move |y| [x, y, 11])),
            LOG,
        );
        let moved = ([10, 1, 11], [12, 3, 11]);
        let rec = judge_over(
            way(vec![foot()], Vec::new()),
            &m,
            &[StateWrite {
                region: moved,
                block: None,
                label: "once `anchor/barricade` opens".to_string(),
            }],
            &[("anchor/barricade".to_string(), moved)],
        );
        assert!(rec.undeclared.is_none(), "{:?}", rec.undeclared);
        assert_eq!(rec.verdict(), "whole");
    }

    /// Criterion 6: the masonry declared, the gate region moved off the way.
    fn declared_yard() -> BlockMap {
        let mut m = floor();
        put(&mut m, (1..=3).map(|y| [10, y, 14]), MASONRY);
        m
    }

    #[test]
    fn a_declared_jamb_is_confirmed_and_excuses_its_column() {
        let rec = judge_over(
            way(vec![foot()], vec![decl(&[2], MASONRY)]),
            &declared_yard(),
            &[],
            &[],
        );
        assert!(rec.undeclared.is_none(), "{:?}", rec.undeclared);
        assert!(rec.not_shown.is_empty(), "{:?}", rec.not_shown);
        assert_eq!((rec.confirmed, rec.excused), (1, 1));
        assert_eq!(rec.verdict(), "narrowed-as-declared");
    }

    #[test]
    fn the_declaration_removed_reds_dw1057() {
        let rec = judge_over(way(vec![foot()], Vec::new()), &declared_yard(), &[], &[]);
        assert_eq!(rec.undeclared.as_ref().map(|f| f.code.id()), Some("DW1057"));
    }

    #[test]
    fn the_wrong_block_reds_dw1058() {
        let rec = judge_over(
            way(vec![foot()], vec![decl(&[2], "minecraft:oak_planks")]),
            &declared_yard(),
            &[],
            &[],
        );
        let f = rec.not_shown.first().expect("refused");
        assert_eq!(f.code.id(), "DW1058");
        assert!(
            f.message.contains("declares `minecraft:oak_planks`, but `minecraft:stone_bricks` stands at [10, 1, 14]"),
            "{}",
            f.message
        );
        assert!(rec.undeclared.is_none());
    }

    #[test]
    fn the_columns_moved_by_one_red_dw1058() {
        let rec = judge_over(
            way(vec![foot()], vec![decl(&[1], MASONRY)]),
            &declared_yard(),
            &[],
            &[],
        );
        let f = rec.not_shown.first().expect("refused");
        assert_eq!(f.code.id(), "DW1058");
        assert!(f.message.contains("column 1"), "{}", f.message);
        assert!(f.message.contains("clear in every state"), "{}", f.message);
        // …and the column the post really stands in is undeclared.
        assert_eq!(rec.undeclared.as_ref().map(|f| f.code.id()), Some("DW1057"));
    }

    #[test]
    fn a_stale_excuse_reds_dw1058() {
        let rec = judge_over(
            way(vec![foot()], vec![decl(&[2], MASONRY)]),
            &floor(),
            &[],
            &[],
        );
        let f = rec
            .not_shown
            .first()
            .expect("the post is gone and the excuse stands");
        assert_eq!(f.code.id(), "DW1058");
        assert!(
            f.message.contains("put the structure back"),
            "{}",
            f.message
        );
        assert_eq!(rec.verdict(), "whole");
    }

    #[test]
    fn a_state_given_only_for_the_block_compares_the_id() {
        assert!(block_matches(
            "minecraft:stone_brick_wall",
            "minecraft:stone_brick_wall[east=none,north=low,up=true]"
        ));
        assert!(block_matches(
            "minecraft:stone_brick_wall[up=true]",
            "minecraft:stone_brick_wall[east=none,north=low,up=true]"
        ));
        assert!(!block_matches(
            "minecraft:stone_brick_wall[up=false]",
            "minecraft:stone_brick_wall[east=none,north=low,up=true]"
        ));
        assert!(!block_matches("minecraft:stone_bricks", "minecraft:stone"));
    }

    /// Criterion 4: a gate region of ANOTHER way standing in the approach is
    /// refused while it is shut; the same region as the way's own is not.
    #[test]
    fn a_foreign_gate_in_the_approach_is_refused_and_the_ways_own_is_not() {
        let mut m = floor();
        put(&mut m, (1..=3).map(|y| [10, y, 13]), LOG);
        let region = ([10, 1, 13], [10, 3, 13]);
        let writes = vec![StateWrite {
            region,
            block: None,
            label: "once `anchor/cart-road` opens".to_string(),
        }];
        let sealed = vec![("anchor/cart-road".to_string(), region)];
        let rec = judge_over(way(vec![foot()], Vec::new()), &m, &writes, &sealed);
        let f = rec.undeclared.as_ref().expect("refused");
        assert!(
            f.message.contains("while `anchor/cart-road` is shut"),
            "{}",
            f.message
        );

        let mut own = way(vec![foot()], Vec::new());
        for y in 1..=3 {
            own.own.insert([10, y, 13], None);
        }
        let rec = judge_over(own, &m, &writes, &sealed);
        assert!(rec.undeclared.is_none(), "{:?}", rec.undeclared);
        assert_eq!(rec.verdict(), "whole");
    }

    #[test]
    fn a_set_block_into_an_approach_column_is_refused() {
        let writes = vec![StateWrite {
            region: ([10, 1, 12], [10, 1, 12]),
            block: Some(STONE.to_string()),
            label: "after `set-block` at `quests[0]/objectives[1]`".to_string(),
        }];
        let rec = judge_over(way(vec![foot()], Vec::new()), &floor(), &writes, &[]);
        let f = rec.undeclared.as_ref().expect("refused");
        assert!(
            f.message.contains(
                "[10, 1, 12] `minecraft:stone` (after `set-block` at `quests[0]/objectives[1]`)"
            ),
            "{}",
            f.message
        );
        assert_eq!(rec.stations[0].clear, vec![3, 2]);
    }

    #[test]
    fn a_thin_block_or_a_slab_a_body_steps_onto_narrows_nothing() {
        for block in [
            "minecraft:torch",
            "minecraft:white_carpet",
            "minecraft:stone_slab[type=bottom,waterlogged=false]",
        ] {
            let mut m = floor();
            put(&mut m, [[10, 1, 12]], block);
            let rec = judge_over(way(vec![foot()], Vec::new()), &m, &[], &[]);
            assert!(rec.undeclared.is_none(), "{block}: {:?}", rec.undeclared);
            assert_eq!(rec.verdict(), "whole", "{block}");
        }
    }

    #[test]
    fn a_post_a_wall_or_a_beam_at_head_height_takes_its_column() {
        for (cells, block) in [
            (vec![[10, 1, 12]], "minecraft:stone_brick_wall"),
            (vec![[10, 1, 12]], "minecraft:oak_fence"),
            (vec![[10, 2, 12]], "minecraft:oak_planks"),
            (
                vec![[10, 1, 12]],
                "minecraft:oak_trapdoor[facing=north,half=top,open=false]",
            ),
        ] {
            let mut m = floor();
            put(&mut m, cells, block);
            let rec = judge_over(way(vec![foot()], Vec::new()), &m, &[], &[]);
            assert_eq!(
                rec.undeclared.as_ref().map(|f| f.code.id()),
                Some("DW1057"),
                "{block}"
            );
        }
        // A beam over a body's head blocks nothing.
        let mut m = floor();
        put(&mut m, [[10, 3, 12]], "minecraft:oak_planks");
        let rec = judge_over(way(vec![foot()], Vec::new()), &m, &[], &[]);
        assert!(rec.undeclared.is_none(), "{:?}", rec.undeclared);
    }

    /// Criterion 7, the unit half: a portal's plane is left to `DW0836` and
    /// judged by nothing here; a contact's span is judged.
    #[test]
    fn a_portal_plane_is_deferred_and_a_contacts_span_is_judged() {
        let mut m = floor();
        put(&mut m, (1..=3).map(|y| [10, y, 12]), STONE);
        let mut plane = foot();
        plane.key = StationKey::Plane;
        plane.deferred = true;
        let rec = judge_over(way(vec![plane.clone()], Vec::new()), &m, &[], &[]);
        assert!(rec.undeclared.is_none());
        assert_eq!(rec.states, 0, "a deferred plane reads no state at all");

        plane.deferred = false;
        plane.label = "the span".to_string();
        let mut contact = way(vec![plane], Vec::new());
        contact.kind = WayKind::Contact;
        let rec = judge_over(contact, &m, &[], &[]);
        let f = rec
            .undeclared
            .as_ref()
            .expect("a half-walled span is narrowed");
        assert!(f.message.contains("at the span"), "{}", f.message);
    }

    /// Criterion 10: an engine-seated object in an approach column is named.
    #[test]
    fn an_engine_seated_object_is_named_and_moved_outward_is_whole() {
        let mut m = floor();
        put(&mut m, [[10, 1, 12]], "minecraft:sea_lantern");
        let open = BTreeSet::new();
        let mut seated = BTreeMap::new();
        seated.insert(
            [10, 1, 12],
            "the relight fixture the build sets at setup".to_string(),
        );
        let ground = Ground {
            shipped: &m,
            open_gates: &open,
            writes: &[],
            sealed: &[],
            seated: &seated,
        };
        let rec = judge(way(vec![foot()], Vec::new()), &ground);
        let f = rec.undeclared.as_ref().expect("refused");
        assert!(
            f.message
                .contains("(as built: the relight fixture the build sets at setup)"),
            "{}",
            f.message
        );
        let mut m = floor();
        put(&mut m, [[10, 1, 11]], "minecraft:sea_lantern");
        let rec = judge_over(way(vec![foot()], Vec::new()), &m, &[], &[]);
        assert!(rec.undeclared.is_none());
    }

    /// A tiny deterministic generator, so the agreement test below is a
    /// fixed population, never a reroll (ADR-0006).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }
    }

    const PALETTE: [&str; 10] = [
        "minecraft:stone",
        "minecraft:stone_slab[type=bottom,waterlogged=false]",
        "minecraft:oak_fence",
        "minecraft:torch",
        "minecraft:ladder[facing=east,waterlogged=false]",
        "minecraft:iron_bars",
        "minecraft:water",
        "minecraft:lantern[hanging=false,waterlogged=false]",
        "minecraft:oak_planks",
        "minecraft:dirt_path",
    ];

    /// **Criterion 3: one predicate.** Over generated worlds, the column test
    /// at a body cell agrees with `nav::can_be` there — clear wherever a body
    /// can be, and clear only where a body can be in the cell or on the floor
    /// it steps onto — and the windowed world the judgement builds answers
    /// `can_be` exactly as the whole world does at every body cell it judges.
    #[test]
    fn the_column_test_and_can_be_never_disagree_on_a_body_cell() {
        let mut rng = Lcg(0x0120);
        let mut judged = 0usize;
        for _ in 0..12 {
            let mut m = BlockMap::new();
            for x in 0..12 {
                for z in 0..12 {
                    m.insert([x, 0, z], BlockState::new(STONE));
                    for y in 1..5 {
                        if rng.next().is_multiple_of(4) {
                            let b = PALETTE[usize::try_from(rng.next() % 10).unwrap()];
                            m.insert([x, y, z], BlockState::new(b));
                        }
                    }
                }
            }
            let open = BTreeSet::new();
            let whole = World::from_occupancy(
                crate::compiler::assembled::occupancy_over(&m, &open),
                Premises::geometry_only(),
            );
            for x in 3..9 {
                for z in 3..9 {
                    let column = |y: i32| Column {
                        index: 0,
                        x,
                        z,
                        ylo: y,
                        yhi: y,
                        y0: Some(y),
                        vertical: false,
                    };
                    // The window `judge` builds round this column's two body
                    // cells, against the whole world.
                    let (lo, _) = column_reach(&column(1));
                    let (_, hi) = column_reach(&column(2));
                    let (wlo, whi) = window(lo, hi);
                    let local_map: BlockMap = m
                        .range(wlo..=whi)
                        .filter(|(k, _)| (0..3).all(|i| wlo[i] <= k[i] && k[i] <= whi[i]))
                        .map(|(k, v)| (*k, *v))
                        .collect();
                    let local = World::from_occupancy(
                        crate::compiler::assembled::occupancy_over(&local_map, &open),
                        Premises::geometry_only(),
                    );
                    for y in 1..3 {
                        let c = column(y);
                        let clear = column_clear(&whole, &c, y);
                        let here = super::super::can_be(&whole, [x, y, z]);
                        let over = super::super::can_be(&whole, [x, y + 1, z]);
                        assert!(
                            !here || clear,
                            "can_be at {:?} and the column is taken",
                            [x, y, z]
                        );
                        assert!(
                            !clear || here || over,
                            "a clear column at {:?} no body can be in",
                            [x, y, z]
                        );
                        assert_eq!(
                            column_clear(&local, &c, y),
                            clear,
                            "the window and the world judge the column at {:?} apart",
                            [x, y, z]
                        );
                        for yy in [y, y + 1] {
                            assert_eq!(
                                super::super::can_be(&local, [x, yy, z]),
                                super::super::can_be(&whole, [x, yy, z]),
                                "the window and the world disagree at {:?}",
                                [x, yy, z]
                            );
                        }
                        judged += 1;
                    }
                }
            }
        }
        assert_eq!(
            judged,
            12 * 6 * 6 * 2,
            "every generated body cell was judged"
        );
    }

    /// Criterion 9: a piece way whose `stair` via is three wide, with one
    /// solid cell in its foot approach, is refused in the piece's frame; the
    /// cell declared `narrowed` is confirmed.
    #[test]
    fn a_piece_flight_narrowed_at_its_foot_is_refused_in_the_piece_frame() {
        use delvewright_dsl::prefab::{
            ContractEdge, ContractSpace, ContractVolume, EdgeNarrowing, Region, SpatialContract,
        };
        let region = |a: [i32; 3], b: [i32; 3]| Region { from: a, to: b };
        // A piece at world [100, 64, 100]: a lower floor `room` (z 6..=9) and
        // a `landing` four up (z 0..=1); the flight climbs z 5..=2, three wide.
        let mut spaces = BTreeMap::new();
        spaces.insert(
            "room".to_string(),
            ContractSpace {
                envelope: "enclosed".to_string(),
                boxes: vec![region([1, 1, 6], [5, 4, 9])],
            },
        );
        spaces.insert(
            "landing".to_string(),
            ContractSpace {
                envelope: "enclosed".to_string(),
                boxes: vec![region([1, 5, 0], [5, 7, 1])],
            },
        );
        let edge = |narrowed: Vec<EdgeNarrowing>| ContractEdge {
            a: "room".to_string(),
            b: "landing".to_string(),
            class: "stair".to_string(),
            rise: Some(4),
            via: Some(ContractVolume {
                region: "flight".to_string(),
                boxes: vec![region([2, 1, 2], [4, 5, 5])],
            }),
            bar: None,
            way: None,
            narrowed,
        };
        let contract = |narrowed| SpatialContract {
            entry: "room".to_string(),
            spaces: spaces.clone(),
            no_body: BTreeMap::new(),
            edges: vec![edge(narrowed)],
            faces: Vec::new(),
            no_body_majority_ack: None,
        };
        let placement = crate::compiler::plan::PiecePlacement {
            prefab_id: "prefab/test-flight".to_string(),
            templates: Vec::new(),
            pos: [100, 64, 100],
            size: [7, 8, 10],
            rotation: crate::compiler::solver::Rotation::None,
            mated: Vec::new(),
        };
        // The bytes: a floor under everything, treads rising toward z 2, and
        // one stone in the foot's approach row (z 6) at x 3.
        let mut m = BlockMap::new();
        for x in 100..=106 {
            for z in 100..=109 {
                m.insert([x, 64, z], BlockState::new(STONE));
            }
        }
        for (k, z) in (102..=105).rev().enumerate() {
            let h = i32::try_from(k).unwrap() + 1;
            for x in 102..=104 {
                for y in 65..65 + h {
                    m.insert([x, y, z], BlockState::new(STONE));
                }
            }
        }
        for x in 102..=104 {
            for z in 100..=101 {
                for y in 65..=68 {
                    m.insert([x, y, z], BlockState::new(STONE));
                }
            }
        }
        m.insert([103, 65, 106], BlockState::new(MASONRY));
        let run = |narrowed: Vec<EdgeNarrowing>| {
            let c = contract(narrowed);
            let mut census = Census::default();
            let mut out = Vec::new();
            let mut refusals = Vec::new();
            piece_ways_over(
                &[(&placement, &c)],
                false,
                &BTreeSet::new(),
                &mut census,
                &mut out,
                &mut refusals,
            );
            assert_eq!(census.edges, 1);
            assert_eq!(out.len(), 1, "the flight is one piece way");
            judge_over(out.remove(0), &m, &[], &[])
        };
        let rec = run(Vec::new());
        let f = rec.undeclared.as_ref().expect("refused");
        assert_eq!(f.code.id(), "DW1057");
        assert!(
            f.message.contains("the approach in `room`"),
            "{}",
            f.message
        );
        assert!(
            f.message
                .contains("[103, 65, 106] `minecraft:stone_bricks`"),
            "{}",
            f.message
        );
        assert!(
            f.message.contains("\"cells\": [[3, 1, 6]]"),
            "{}",
            f.message
        );

        let rec = run(vec![EdgeNarrowing {
            cells: vec![[3, 1, 6]],
            block: MASONRY.to_string(),
            what: "a newel at the flight's foot".to_string(),
        }]);
        assert!(rec.undeclared.is_none(), "{:?}", rec.undeclared);
        assert!(rec.not_shown.is_empty(), "{:?}", rec.not_shown);
        assert_eq!(rec.confirmed, 1);

        // A flight a stair seam took as its run is judged under the seam, once.
        let c = contract(Vec::new());
        let mut census = Census::default();
        let mut out = Vec::new();
        let mut refusals = Vec::new();
        let flights: BTreeSet<(String, usize)> = [("prefab/test-flight".to_string(), 0)]
            .into_iter()
            .collect();
        piece_ways_over(
            &[(&placement, &c)],
            false,
            &flights,
            &mut census,
            &mut out,
            &mut refusals,
        );
        assert!(
            out.is_empty(),
            "the flight is the seam's, not a piece way of its own"
        );
        assert_eq!(
            census.edges, 1,
            "and it is still counted in the denominator"
        );
    }

    /// Criteria 2 and 9: a stair seam whose host piece declares no flight
    /// joining its face is counted undeclared and its host approach is still
    /// judged; one whose piece's flight joins the face takes that flight as
    /// its run, and names it so the piece way is judged once, under the seam.
    #[test]
    fn a_host_piece_with_no_flight_is_counted_and_one_with_a_flight_lends_it() {
        use delvewright_dsl::prefab::{
            ContractEdge, ContractSpace, ContractVolume, Region, SpatialContract,
        };
        use delvewright_dsl::siteplan::{Base, Face};
        let node = |n: &str| delvewright_dsl::NodeId(n.to_string());
        let hall = PlacedBox {
            node: node("node/hall"),
            foot: [0, 9, 0, 9],
            floor: 64,
            clearance: 8,
            open: false,
            base: Base::Ground,
            roof: None,
        };
        let loft = PlacedBox {
            node: node("node/loft"),
            foot: [11, 18, 0, 9],
            floor: 69,
            clearance: 3,
            open: false,
            base: Base::Ground,
            roof: None,
        };
        let seam = PlacedSeam {
            edge: delvewright_dsl::ids::EdgeId("edge/hall-loft".to_string()),
            class: "stair",
            a: node("node/hall"),
            b: node("node/loft"),
            face: Face::East,
            normal_axis: 0,
            plane: 10,
            opening: ([10, 69, 3], [10, 71, 5]),
            shared: ([10, 64, 0], [10, 71, 9]),
            crossing: Crossing::Portal,
            rise: 5,
            stair_in: Some(node("node/hall")),
            form: "a stone stair up to the loft".to_string(),
            shut: false,
        };
        let region = |a: [i32; 3], b: [i32; 3]| Region { from: a, to: b };
        let space = |a: [i32; 3], b: [i32; 3]| ContractSpace {
            envelope: "enclosed".to_string(),
            boxes: vec![region(a, b)],
        };
        let edge = |a: &str, b: &str, class: &str, via: Region| ContractEdge {
            a: a.to_string(),
            b: b.to_string(),
            class: class.to_string(),
            rise: Some(0),
            via: Some(ContractVolume {
                region: format!("{a}-{b}"),
                boxes: vec![via],
            }),
            bar: None,
            way: None,
            narrowed: Vec::new(),
        };
        let placement = crate::compiler::plan::PiecePlacement {
            prefab_id: "prefab/test-hall".to_string(),
            templates: Vec::new(),
            pos: [0, 63, 0],
            size: [10, 9, 10],
            rotation: crate::compiler::solver::Rotation::None,
            mated: Vec::new(),
        };
        let by_node: BTreeMap<&str, &PlacedBox> = [("node/hall", &hall), ("node/loft", &loft)]
            .into_iter()
            .collect();
        let run = |contract: &SpatialContract| {
            let pieces: BTreeMap<&str, BoundPiece<'_>> = [(
                "node/hall",
                BoundPiece {
                    placement: &placement,
                    contract,
                },
            )]
            .into_iter()
            .collect();
            let input = SeamInput {
                s: &seam,
                boxes: &by_node,
                laid: None,
                pieces: &pieces,
                decl: &[],
                seam_index: 0,
                own_gates: &[],
            };
            let mut census = Census::default();
            let mut flights = BTreeSet::new();
            let way = seam_way(&input, &mut census, &mut flights).expect("a way");
            (way, census, flights)
        };

        // An exterior stair edge whose own `via` is the face: no run.
        let mut spaces = BTreeMap::new();
        spaces.insert("room".to_string(), space([0, 1, 0], [3, 8, 9]));
        let faced = SpatialContract {
            entry: "room".to_string(),
            spaces: spaces.clone(),
            no_body: BTreeMap::new(),
            edges: vec![edge(
                "room",
                "exterior",
                "stair",
                region([9, 6, 3], [9, 8, 5]),
            )],
            faces: Vec::new(),
            no_body_majority_ack: None,
        };
        let (way, census, flights) = run(&faced);
        assert_eq!(way.run, Run::Undeclared);
        assert_eq!(census.runs_undeclared, 1);
        assert!(flights.is_empty());
        assert!(
            way.stations
                .iter()
                .any(|s| s.label == "the approach in `node/hall`" && s.columns.len() == 3),
            "{:?}",
            way.stations.iter().map(|s| &s.label).collect::<Vec<_>>()
        );

        // A landing at the face, and a flight from the room to it.
        spaces.insert("landing".to_string(), space([9, 5, 3], [9, 8, 5]));
        let flight = SpatialContract {
            entry: "room".to_string(),
            spaces,
            no_body: BTreeMap::new(),
            edges: vec![
                edge("landing", "exterior", "walk", region([9, 6, 3], [9, 8, 5])),
                edge("room", "landing", "stair", region([4, 1, 3], [8, 5, 5])),
            ],
            faces: Vec::new(),
            no_body_majority_ack: None,
        };
        let (way, census, flights) = run(&flight);
        assert_eq!(way.run, Run::Piece);
        assert_eq!(census.runs_undeclared, 0);
        assert_eq!(
            flights,
            [("prefab/test-hall".to_string(), 1)].into_iter().collect()
        );
        let labels: Vec<&str> = way.stations.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels.first(), Some(&"the head's approach in `node/loft`"));
        assert!(labels.contains(&"course 0 of the run"), "{labels:?}");
        assert_eq!(
            labels.last(),
            Some(&"the foot's approach in `room`"),
            "{labels:?}"
        );
        assert!(way.label.contains("room--stair--landing"), "{}", way.label);
    }

    /// A `narrowed` on a contract edge that realizes a plan seam is refused by
    /// the contract's own gate (`DW0888`): the seam's declaration is the one
    /// authority, and the edge is no piece way of its own.
    #[test]
    fn a_narrowing_on_an_edge_that_realizes_a_seam_is_dw0888() {
        use delvewright_dsl::prefab::{
            ContractEdge, ContractSpace, ContractVolume, EdgeNarrowing, Region, SpatialContract,
        };
        let region = |a: [i32; 3], b: [i32; 3]| Region { from: a, to: b };
        let mut spaces = BTreeMap::new();
        spaces.insert(
            "room".to_string(),
            ContractSpace {
                envelope: "enclosed".to_string(),
                boxes: vec![region([1, 1, 1], [5, 3, 5])],
            },
        );
        let contract = SpatialContract {
            entry: "room".to_string(),
            spaces,
            no_body: BTreeMap::new(),
            edges: vec![ContractEdge {
                a: "room".to_string(),
                b: "exterior".to_string(),
                class: "walk".to_string(),
                rise: None,
                via: Some(ContractVolume {
                    region: "door".to_string(),
                    boxes: vec![region([0, 1, 3], [0, 2, 3])],
                }),
                bar: None,
                way: None,
                narrowed: vec![EdgeNarrowing {
                    cells: vec![[0, 1, 3]],
                    block: MASONRY.to_string(),
                    what: "a jamb".to_string(),
                }],
            }],
            faces: Vec::new(),
            no_body_majority_ack: None,
        };
        let placement = crate::compiler::plan::PiecePlacement {
            prefab_id: "prefab/test-room".to_string(),
            templates: Vec::new(),
            pos: [0, 64, 0],
            size: [7, 5, 7],
            rotation: crate::compiler::solver::Rotation::None,
            mated: Vec::new(),
        };
        let mut census = Census::default();
        let mut out = Vec::new();
        let mut refusals = Vec::new();
        piece_ways_over(
            &[(&placement, &contract)],
            true,
            &BTreeSet::new(),
            &mut census,
            &mut out,
            &mut refusals,
        );
        assert!(
            out.is_empty(),
            "an exterior edge of a site-plan piece answers a seam"
        );
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0].code.id(), "DW0888");
        assert!(
            refusals[0].message.contains("realizes a plan seam"),
            "{}",
            refusals[0].message
        );
        // The same piece placed by `areas[]`: its door is a piece way of its own.
        let mut refusals = Vec::new();
        piece_ways_over(
            &[(&placement, &contract)],
            false,
            &BTreeSet::new(),
            &mut census,
            &mut out,
            &mut refusals,
        );
        assert!(refusals.is_empty());
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn a_binding_with_nothing_states_its_zeros() {
        let b = WidthBinding {
            states: 1,
            ..WidthBinding::default()
        };
        assert_eq!(
            b.line(),
            "clear width (DW1057/DW1058): 0 way(s) of 0 traversal seam(s), 0 box(es) and 0 \
             contract edge(s) — 0 portal(s), 0 stair run(s), 0 contact(s), 0 way place(s), 0 \
             piece way(s); 0 column(s) at 0 station(s) judged over 1 state(s); 0 way(s) whole, 0 \
             narrowed undeclared, 0 narrowed as declared (0 column(s) excused by 0 \
             declaration(s)), 0 declaration(s) not shown; 0 plane column(s) left to DW0836; 0 \
             stair run(s) undeclared by their piece"
        );
        let j = b.to_json();
        assert_eq!(j["ways"], 0);
        assert_eq!(j["codes"], json!(["DW1057", "DW1058"]));
    }

    #[test]
    fn the_codes_are_their_numbers() {
        assert_eq!(DW_WAY_NARROWED.id(), "DW1057");
        assert_eq!(DW_NARROWING_NOT_SHOWN.id(), "DW1058");
    }
}
