//! **A way is whole** (spec-0120): the clear width a door, a stair or a road
//! declares is clear along the whole way, in every state its cells can take
//! (`DW1057`). Nothing excuses a column that is not: a thing meant to stand
//! beside a way stands outside its declared width.
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
//!   be in its body cell — [`super::shutmodel::can_be`], the nav model's one answer to
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
use delvewright_dsl::siteplan::{Crossing, PlacedBox, PlacedSeam, seam_anchor, way_at};
use delvewright_dsl::{DwCode, ExitTier};
use serde_json::{Value, json};

use super::{Premises, World};
use crate::compiler::blockstate::{BlockMap, BlockState};
use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, ResolvedAnchor};
use crate::compiler::registry::PrefabRegistry;

delvewright_dsl::dw_code! {
    /// `DW1057`: **a way narrowed** (spec-0120 §6.1). A column of a door's,
    /// a stair's or a road's declared clear width is taken — by prefab bytes,
    /// the stand-in's own blocks, an engine-seated object or a runtime write —
    /// in some state its cells can take, while other columns of the same
    /// station stay clear. Nothing excuses it. Build tier.
    pub const DW_WAY_NARROWED: DwCode = DwCode::new("DW1057", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1065`: **an open side built shut** (spec-0123 §5.4). A side the
    /// plan declares open has every column of a station taken as built — a
    /// side open in the plan and walled in the piece, a hedge along the whole
    /// ring, or an open side at the region's edge with nothing beyond it.
    /// Build tier.
    pub const DW_OPEN_SIDE_SHUT: DwCode = DwCode::new("DW1065", ExitTier::Build);
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
    /// A side a site-plan box declares open (spec-0123 §5.4).
    OpenSide,
}

impl WayKind {
    fn word(self) -> &'static str {
        match self {
            WayKind::Portal => "portal",
            WayKind::Contact => "contact",
            WayKind::Place => "way-place",
            WayKind::Piece => "piece-way",
            WayKind::OpenSide => "open side",
        }
    }
}

/// Which side of a seam an approach stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    /// The approach in the connection's `a` place (on a stair, the host's
    /// approach is the run's foot).
    A,
    /// The approach in the connection's `b` place.
    B,
}

/// Where along a way a station stands.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StationKey {
    /// An approach, by the side it is on.
    Side(Side),
    /// A portal's or a contact's plane.
    Plane,
    /// A stair run's course, top first.
    Course(u32),
    /// A way place's station, in cells from the box's low corner on its axis.
    Along(u32),
    /// A piece way's station, by its coordinate on the travel axis.
    Piece(i32),
    /// An open side's ring row (`0`) or its approach one cell outward (`1`).
    OpenSide(u32),
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

/// Where a piece way's cells are in its own piece's frame, for the refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceFrame {
    /// The prefab.
    pub prefab: String,
    /// The placement's world position.
    pub pos: [i32; 3],
    /// The placement's rotation.
    pub rotation: crate::compiler::solver::Rotation,
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
    /// A piece way's piece, to name its taking cells in the piece's frame.
    pub piece: Option<PieceFrame>,
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
    /// The way's refusal at its first narrowed station, if it is narrowed.
    pub refusal: Option<Failure>,
}

impl WayRecord {
    /// `whole` or `narrowed`, over the stations judged. There is no third
    /// verdict.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        if self.refusal.is_some() {
            "narrowed"
        } else {
            "whole"
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
                } else if s.columns.is_empty() {
                    // Every column is a seam's, and judged once, under it.
                    "judged under its seam"
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
        json!({
            "kind": self.way.kind.word(),
            "way": self.way.name,
            "width": self.way.width,
            "states": self.states,
            "verdict": self.verdict(),
            "stations": stations,
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
    if super::shutmodel::can_be(w, at) {
        return true;
    }
    let over = [c.x, y0 + 1, c.z];
    super::shutmodel::can_be(w, over)
        && w.solid_top_16(at)
            .is_some_and(|top| i64::from(top) <= MAX_AUTO_STEP_16)
}

/// **The cells a body passing this column can be in** — the cells
/// [`column_clear`] and [`lowest_body`] read a body in: from `ylo` to `yhi`
/// and, off a vertical column, the cell over `yhi` (the body on the floor a
/// step puts it on).
fn body_cells(c: &Column) -> impl Iterator<Item = [i32; 3]> + '_ {
    let top = if c.vertical { c.yhi } else { c.yhi + 1 };
    (c.ylo..=top).map(move |y| [c.x, y, c.z])
}

/// One station of a declared way as a **cross-section**: the way's full
/// declared width there, as the cells a body passing any of its columns can be
/// in. The shut-way check cuts a branch at the narrowest one its path walks
/// (spec-0118 §5.2); the width is the station's column count, the denominator
/// this check judges the station's clear width against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossSection {
    /// The way in words.
    pub way: String,
    /// The station in words.
    pub station: String,
    /// The declared width there, in columns.
    pub width: usize,
    /// Every cell a body passing the station can be in.
    pub cells: BTreeSet<[i32; 3]>,
}

/// Every station of every way as a [`CrossSection`], a portal's plane
/// included: what a body passing the way passes, whoever judges its columns.
#[must_use]
pub fn cross_sections(ways: &[Way]) -> Vec<CrossSection> {
    ways.iter()
        .flat_map(|w| {
            w.stations.iter().map(move |s| CrossSection {
                way: w.label.clone(),
                station: s.label.clone(),
                width: s.columns.len(),
                cells: s.columns.iter().flat_map(body_cells).collect(),
            })
        })
        .filter(|x| x.width > 0)
        .collect()
}

/// The cross-sections of every declared way of the plan — the one entry
/// point a reader outside this check takes them by.
#[must_use]
pub fn plan_cross_sections(plan: &Plan, prefabs: &PrefabRegistry) -> Vec<CrossSection> {
    cross_sections(&population(plan, prefabs).0)
}

/// The lowest cell of a column a body can be in, over `w` — the measurement a
/// piece way's station plane is taken from.
fn lowest_body(w: &World, c: &Column) -> Option<i32> {
    (c.ylo..=c.yhi + 1).find(|y| super::shutmodel::can_be(w, [c.x, *y, c.z]))
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
        refusal: None,
        way: way.clone(),
    };
    if judged.is_empty() {
        return empty;
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
            // Shut in this state: the gate's own finding — except on an open
            // side, which no gate shuts, where a station built shut is the
            // finding (`DW1065`) and names every cell that takes it.
            if n == 0 && !(way.kind == WayKind::OpenSide && k == 0) {
                continue;
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
    let shut = (way.kind == WayKind::OpenSide)
        .then(|| {
            way.stations
                .iter()
                .zip(&verdicts)
                .find(|(s, v)| !s.deferred && !s.columns.is_empty() && v.clear.first() == Some(&0))
                .map(|(s, v)| shut_refusal(&way, s, v))
        })
        .flatten();
    let refusal = shut.or_else(|| {
        way.stations
            .iter()
            .zip(&verdicts)
            .find(|(s, v)| !s.deferred && v.narrowed(s.columns.len()))
            .map(|(s, v)| narrowed_refusal(&way, s, v))
    });
    WayRecord {
        stations: verdicts,
        states: maps.len(),
        refusal,
        ..empty
    }
}

/// The `DW1065` refusal for an open side built shut at one station: every
/// column taken as built, each taking cell named with its block and state.
fn shut_refusal(way: &Way, s: &Station, v: &StationVerdict) -> Failure {
    let width = s.columns.len();
    let cells: Vec<String> = v
        .taken
        .values()
        .flatten()
        .map(|t| {
            format!(
                "[{}, {}, {}] `{}` ({})",
                t.cell[0], t.cell[1], t.cell[2], t.block, t.state
            )
        })
        .collect();
    let first = v
        .taken
        .values()
        .flatten()
        .next()
        .map(|t| {
            format!(
                "[{}, {}, {}] by `{}`",
                t.cell[0], t.cell[1], t.cell[2], t.block
            )
        })
        .unwrap_or_else(|| "a cell no body can be in".to_string());
    Failure {
        code: DW_OPEN_SIDE_SHUT,
        message: format!(
            "{label} is declared open and built shut: {width} of {width} columns taken, first \
             at {first} ({station}). Taken: {cells}. An open side stands no wall; a body \
             meets the place's ground across it. Clear the run — move the hedge, the wall or the \
             fence inside the play space or into the verge — or declare the side `wall`; at the \
             region's edge, give the side ground beyond it.",
            label = way.label,
            station = s.label,
            cells = cells.join(", "),
        ),
    }
}

/// The `DW1057` refusal for one way at its first narrowed station.
fn narrowed_refusal(way: &Way, s: &Station, v: &StationVerdict) -> Failure {
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
    for t in v.taken.values().flatten() {
        let local = way
            .piece
            .as_ref()
            .filter(|_| t.block != "minecraft:air")
            .map(|p| {
                let l = crate::compiler::ways::local_cell(p.rotation, p.pos, t.cell);
                format!(
                    ", [{}, {}, {}] in `{}`'s own frame",
                    l[0], l[1], l[2], p.prefab
                )
            })
            .unwrap_or_default();
        cells.push(format!(
            "[{}, {}, {}] `{}` ({}){local}",
            t.cell[0], t.cell[1], t.cell[2], t.block, t.state
        ));
    }
    let more = cells.len().saturating_sub(CELL_LIST_LIMIT);
    cells.truncate(CELL_LIST_LIMIT);
    Failure {
        code: DW_WAY_NARROWED,
        message: format!(
            "{label} is narrowed at {station}: its declared clear width is {width} column(s), and \
             {free} of {width} are clear {state} ({per_state}). Taken: {cells}{more}. A way's \
             width is not diminished along the way, and nothing excuses a structure standing in \
             it. Move the structure out of the way's {width} column(s) — into the frame, the \
             wall, the verge, the next column over; or declare the way as wide as it is built, \
             and build it so.",
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
    /// Sides the plan's boxes declare open (spec-0123 §5.4).
    pub open_sides: usize,
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
fn wall_side(s: &PlacedSeam, side: Side, at: i64, floor: i64, label: String) -> Station {
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

fn side_of(s: &PlacedSeam, node: &delvewright_dsl::NodeId) -> Side {
    if *node == s.a { Side::A } else { Side::B }
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
    let by_node: BTreeMap<&str, &PlacedBox> =
        b.boxes.iter().map(|x| (x.node.0.as_str(), x)).collect();
    let pieces = bound_pieces(plan, prefabs);
    for s in &b.seams {
        census.seams += 1;
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
            own_gates: &own_gates,
        };
        if let Some(w) = seam_way(&input, census, flights) {
            out.push(w);
        }
    }
}

/// What one seam's way is built from: the seam, the places, the run the
/// stand-in laid, the bound pieces and the gate regions its own beat opens.
pub struct SeamInput<'a> {
    /// The seam.
    pub s: &'a PlacedSeam,
    /// Every place, by node.
    pub boxes: &'a BTreeMap<&'a str, &'a PlacedBox>,
    /// The run the stand-in laid for it, if it did.
    pub laid: Option<&'a crate::compiler::blockout::LaidStair>,
    /// Every bound place's piece, by node.
    pub pieces: &'a BTreeMap<&'a str, BoundPiece<'a>>,
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
        // The declared width is the opening's extent across the way; a run
        // that travels along the wall (a flight under the opening) is counted
        // across an axis the opening spans one cell of, so the width is read on
        // the opening's own in-plane axis, never on the run's.
        let declared_across = if floor_seam {
            cross
        } else {
            across_of(s.normal_axis)
        };
        width = usize::try_from(ohi[declared_across] - olo[declared_across] + 1).unwrap_or(0);
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
        piece: None,
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
        let a = crate::compiler::ways::world_cell(p.rotation, p.pos, r.from);
        let b = crate::compiler::ways::world_cell(p.rotation, p.pos, r.to);
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
///
/// A flight need not climb toward the plane: one that runs along the wall
/// under its opening (a stair up the wall to a landing at the face) travels
/// on an axis the plane does not cut, and its columns are counted across the
/// flight from its own first column, since the opening spans nothing across
/// it. Which end is the head is read off the climb — the end whose course
/// stands higher; only a flight whose two end courses stand at one height
/// falls back to the end nearer the plane.
fn flight_stations(f: &Flight, s: &PlacedSeam, host_side: Side) -> Vec<Station> {
    let cross = across_of(f.travel);
    let u0 = if cross == s.normal_axis {
        f.cells.iter().map(|c| c[cross]).min().unwrap_or(0)
    } else {
        n32(s.opening.0[cross])
    };
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
            let low_of =
                |st: Option<&Station>| st.and_then(|st| st.columns.iter().map(|c| c.ylo).min());
            match (low_of(region.first()), low_of(region.last())) {
                (Some(first), Some(last)) if first != last => first > last,
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
            }
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
    for pb in &site.boxes {
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
        out.push(Way {
            kind: WayKind::Place,
            name: pb.node.0.clone(),
            label: format!("the way `{}`", pb.node),
            width: usize::try_from(clear).unwrap_or(0),
            stations,
            own: BTreeMap::new(),
            piece: None,
            run: Run::None,
        });
    }
}

/// **Every open side as a way** (spec-0123 §5.4): its corridor is the ring
/// run at the walk plane — corners excluded — and one approach cell outward
/// where that cell is no seam's. Cells a `contact` on the side covers, and
/// any station a seam's approach covers, are judged under the seam and
/// counted once.
fn open_side_ways(
    plan: &Plan,
    census: &mut Census,
    cover: &BTreeSet<[i32; 3]>,
    out: &mut Vec<Way>,
) {
    let Some(b) = plan.blockout.as_ref() else {
        return;
    };
    let seam_cells: BTreeSet<[i32; 3]> = crate::compiler::blockout::seam_cells(&b.seams);
    for placed in &b.boxes {
        for side in placed.open_sides() {
            census.open_sides += 1;
            let contacted: BTreeSet<[i32; 3]> =
                crate::compiler::detail::covering_contacts(&side, &b.seams)
                    .into_iter()
                    .flat_map(|(_, (lo, hi))| crate::compiler::blockout::cells_of(lo, hi))
                    .map(narrow)
                    .collect();
            let (lo, hi) = side.cells;
            let (n, a) = (side.normal_axis(), side.along_axis());
            let y0 = side.walk_y;
            let mut stations = Vec::new();
            for (k, words) in [(0i64, "ring"), (1, "approach")] {
                let t = lo[n] + side.outward() * k;
                let columns: Vec<Column> = (lo[a]..=hi[a])
                    .enumerate()
                    .filter_map(|(index, u)| {
                        let col = column_at(n, t, u, index, y0);
                        let at = [col.x, n32(y0), col.z];
                        let skip = cover.contains(&at)
                            || contacted.contains(&at)
                            || (k == 1 && seam_cells.contains(&at));
                        (!skip).then_some(col)
                    })
                    .collect();
                stations.push(Station {
                    key: StationKey::OpenSide(u32::try_from(k).unwrap_or(0)),
                    label: format!("the {words} of the {} side", side.face.as_str()),
                    deferred: false,
                    columns,
                });
            }
            out.push(Way {
                kind: WayKind::OpenSide,
                name: format!("{} {}", placed.node.0, side.face.as_str()),
                label: format!("the {} side of `{}`", side.face.as_str(), placed.node),
                width: usize::try_from(hi[a] - lo[a] + 1).unwrap_or(0),
                stations,
                own: BTreeMap::new(),
                piece: None,
                run: Run::None,
            });
        }
    }
}

/// The traversal and barred classes a piece way may be.
const PIECE_CLASSES: [&str; 5] = ["walk", "stair", "climb", "drop", "barred"];

/// Every piece way: a placed piece's contract edge that answers no plan seam.
fn piece_ways(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    flights: &BTreeSet<(String, usize)>,
    census: &mut Census,
    out: &mut Vec<Way>,
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
    );
}

/// [`piece_ways`] over placed pieces and their contracts: every edge of the
/// five classes counted and every one that answers no plan seam judged.
/// `site_plan` says whether
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
                out.push(Way {
                    kind: WayKind::Piece,
                    name: format!("{} {edge_words}", p.prefab_id),
                    label: format!("the piece way {edge_words} of `{}`", p.prefab_id),
                    width,
                    stations,
                    own,
                    piece: Some(PieceFrame {
                        prefab: p.prefab_id.clone(),
                        pos: p.pos,
                        rotation: p.rotation,
                    }),
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
        let narrowed = self.records.len() - whole;
        format!(
            "clear width (DW1057): {w} way(s) of {t} traversal seam(s), {b} box(es) and {e} \
             contract edge(s) — {p} portal(s), {s} stair run(s), {cc} contact(s), {y} way \
             place(s), {o} open side(s), {k} piece way(s); {m} column(s) at {n} station(s) judged over {v} \
             state(s); {h} way(s) whole, {u} narrowed; {l} plane column(s) left to DW0836; {r} \
             stair run(s) undeclared by their piece",
            w = self.records.len(),
            t = c.seams,
            b = c.boxes,
            e = c.edges,
            p = self.count(WayKind::Portal),
            s = runs,
            cc = self.count(WayKind::Contact),
            y = self.count(WayKind::Place),
            o = self.count(WayKind::OpenSide),
            k = self.count(WayKind::Piece),
            m = columns,
            n = stations,
            v = self.states,
            h = whole,
            u = narrowed,
            l = c.plane_columns,
            r = c.runs_undeclared,
        )
    }

    /// The ledger `validation/clear-width.json`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let c = &self.census;
        json!({
            "codes": [DW_WAY_NARROWED.id(), DW_OPEN_SIDE_SHUT.id()],
            "line": self.line(),
            "ways": self.records.len(),
            "examined": self.records.len(),
            "traversal_seams": c.seams,
            "boxes": c.boxes,
            "contract_edges": c.edges,
            "portals": self.count(WayKind::Portal),
            "contacts": self.count(WayKind::Contact),
            "way_places": self.count(WayKind::Place),
            "piece_ways": self.count(WayKind::Piece),
            "open_sides": self.count(WayKind::OpenSide),
            "states": self.states,
            "plane_columns_left_to_dw0836": c.plane_columns,
            "stair_runs_undeclared": c.runs_undeclared,
            "records": self.records.iter().map(WayRecord::to_json).collect::<Vec<_>>(),
        })
    }

    /// Every refusal, in way order.
    #[must_use]
    pub fn refusals(&self) -> Vec<&Failure> {
        self.records
            .iter()
            .filter_map(|r| r.refusal.as_ref())
            .collect()
    }
}

/// Every way of the plan, built and not yet judged, with the census.
#[must_use]
pub fn population(plan: &Plan, prefabs: &PrefabRegistry) -> (Vec<Way>, Census) {
    let mut census = Census::default();
    let mut ways = Vec::new();
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
    open_side_ways(plan, &mut census, &cover, &mut ways);
    piece_ways(plan, prefabs, &flights, &mut census, &mut ways);
    (ways, census)
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
    let (ways, census) = population(plan, prefabs);
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
    let out: Vec<Failure> = binding.refusals().into_iter().cloned().collect();
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
            key: StationKey::Side(Side::B),
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

    fn way(stations: Vec<Station>) -> Way {
        Way {
            kind: WayKind::Portal,
            name: "edge/keeper-stair".to_string(),
            label: "the stair `edge/keeper-stair`".to_string(),
            width: 3,
            stations,
            own: BTreeMap::new(),
            piece: None,
            run: Run::Laid,
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
            way(vec![foot()]),
            &yard(),
            &opens_barricade(),
            &sealed_barricade(),
        );
        let f = rec.refusal.as_ref().expect("the foot is narrowed");
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
        assert!(
            m.contains("Move the structure out of the way's 3 column(s)"),
            "{m}"
        );
        assert!(m.contains("declare the way as wide as it is built"), "{m}");
        assert!(
            !m.contains("\"narrowed\""),
            "the message writes no entry to paste: {m}"
        );
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
            way(vec![foot()]),
            &m,
            &[StateWrite {
                region: moved,
                block: None,
                label: "once `anchor/barricade` opens".to_string(),
            }],
            &[("anchor/barricade".to_string(), moved)],
        );
        assert!(rec.refusal.is_none(), "{:?}", rec.refusal);
        assert_eq!(rec.verdict(), "whole");
    }

    /// Criterion 6: the masonry left standing, the gate region moved off the
    /// way — a jamb in the foot is refused whatever the documents say; there
    /// is no surface left to say it on.
    fn jamb_yard() -> BlockMap {
        let mut m = floor();
        put(&mut m, (1..=3).map(|y| [10, y, 14]), MASONRY);
        m
    }

    #[test]
    fn a_jamb_in_the_foot_is_refused_with_nothing_to_excuse_it() {
        let rec = judge_over(way(vec![foot()]), &jamb_yard(), &[], &[]);
        let f = rec.refusal.as_ref().expect("the jamb narrows the foot");
        assert_eq!(f.code.id(), "DW1057");
        assert!(f.message.contains("2 of 3"), "{}", f.message);
        assert_eq!(rec.verdict(), "narrowed");
    }

    /// Criterion 6, the second demo level's shape: a full-cube post in
    /// column 2 of a three-wide stair's foot approach is refused `2 of 3`
    /// naming the post; moved one column outward, onto the floor beside the
    /// flight, the foot is whole.
    #[test]
    fn a_newel_in_the_middle_of_the_foot_is_refused_and_beside_it_is_whole() {
        const NEWEL: &str = "minecraft:stripped_dark_oak_log[axis=y]";
        let mut m = floor();
        put(&mut m, [[10, 1, 13]], NEWEL);
        let rec = judge_over(way(vec![foot()]), &m, &[], &[]);
        let f = rec.refusal.as_ref().expect("the newel narrows the foot");
        assert_eq!(f.code.id(), "DW1057");
        assert!(f.message.contains("2 of 3"), "{}", f.message);
        assert!(
            f.message
                .contains("[10, 1, 13] `minecraft:stripped_dark_oak_log[axis=y]` (as built)"),
            "{}",
            f.message
        );
        let mut beside = floor();
        put(&mut beside, [[10, 1, 15]], NEWEL);
        let rec = judge_over(way(vec![foot()]), &beside, &[], &[]);
        assert!(rec.refusal.is_none(), "{:?}", rec.refusal);
        assert_eq!(rec.verdict(), "whole");
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
        let rec = judge_over(way(vec![foot()]), &m, &writes, &sealed);
        let f = rec.refusal.as_ref().expect("refused");
        assert!(
            f.message.contains("while `anchor/cart-road` is shut"),
            "{}",
            f.message
        );

        let mut own = way(vec![foot()]);
        for y in 1..=3 {
            own.own.insert([10, y, 13], None);
        }
        let rec = judge_over(own, &m, &writes, &sealed);
        assert!(rec.refusal.is_none(), "{:?}", rec.refusal);
        assert_eq!(rec.verdict(), "whole");
    }

    #[test]
    fn a_set_block_into_an_approach_column_is_refused() {
        let writes = vec![StateWrite {
            region: ([10, 1, 12], [10, 1, 12]),
            block: Some(STONE.to_string()),
            label: "after `set-block` at `quests[0]/objectives[1]`".to_string(),
        }];
        let rec = judge_over(way(vec![foot()]), &floor(), &writes, &[]);
        let f = rec.refusal.as_ref().expect("refused");
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
            let rec = judge_over(way(vec![foot()]), &m, &[], &[]);
            assert!(rec.refusal.is_none(), "{block}: {:?}", rec.refusal);
            assert_eq!(rec.verdict(), "whole", "{block}");
        }
    }

    #[test]
    fn a_post_a_wall_or_a_beam_at_head_height_takes_its_column() {
        for (cells, block) in [
            (vec![[10, 1, 12]], "minecraft:stone_brick_wall"),
            (vec![[10, 1, 12]], "minecraft:oak_fence"),
            (vec![[10, 2, 12]], "minecraft:oak_planks"),
            // A hatch a hand does not open (spec-0123 §3.1); an oak one is a
            // door the body opens, and its column is clear.
            (
                vec![[10, 1, 12]],
                "minecraft:iron_trapdoor[facing=north,half=top,open=false]",
            ),
        ] {
            let mut m = floor();
            put(&mut m, cells, block);
            let rec = judge_over(way(vec![foot()]), &m, &[], &[]);
            assert_eq!(
                rec.refusal.as_ref().map(|f| f.code.id()),
                Some("DW1057"),
                "{block}"
            );
        }
        // A beam over a body's head blocks nothing.
        let mut m = floor();
        put(&mut m, [[10, 3, 12]], "minecraft:oak_planks");
        let rec = judge_over(way(vec![foot()]), &m, &[], &[]);
        assert!(rec.refusal.is_none(), "{:?}", rec.refusal);
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
        let rec = judge_over(way(vec![plane.clone()]), &m, &[], &[]);
        assert!(rec.refusal.is_none());
        assert_eq!(rec.states, 0, "a deferred plane reads no state at all");

        plane.deferred = false;
        plane.label = "the span".to_string();
        let mut contact = way(vec![plane]);
        contact.kind = WayKind::Contact;
        let rec = judge_over(contact, &m, &[], &[]);
        let f = rec
            .refusal
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
        let rec = judge(way(vec![foot()]), &ground);
        let f = rec.refusal.as_ref().expect("refused");
        assert!(
            f.message
                .contains("(as built: the relight fixture the build sets at setup)"),
            "{}",
            f.message
        );
        let mut m = floor();
        put(&mut m, [[10, 1, 11]], "minecraft:sea_lantern");
        let rec = judge_over(way(vec![foot()]), &m, &[], &[]);
        assert!(rec.refusal.is_none());
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

    /// **The cross-section agrees with the width model** (spec-0118 §5.2 reads
    /// spec-0120's ways): over generated worlds and stations of every column
    /// shape — a walk plane's, a piece way's measured plane, a hole's — a
    /// station's cross-section spans exactly the columns this check judges,
    /// its width is the station's column count and bounds every clear count
    /// the judgement reports, and every cell a body can be in where the
    /// column test finds the column clear (or where a piece way's plane is
    /// measured, or down a hole) is a cell of the cross-section.
    #[test]
    fn a_cross_section_spans_the_columns_the_width_model_judges() {
        let mut rng = Lcg(0x0118);
        let mut bodies = 0usize;
        let mut stations = 0usize;
        for _ in 0..24 {
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
            let along_x = rng.next().is_multiple_of(2);
            let width = 1 + usize::try_from(rng.next() % 4).unwrap();
            let t = 3 + i32::try_from(rng.next() % 5).unwrap();
            let u0 = 2 + i32::try_from(rng.next() % 4).unwrap();
            let shape = rng.next() % 3;
            let columns: Vec<Column> = (0..width)
                .map(|i| {
                    let u = u0 + i32::try_from(i).unwrap();
                    let (x, z) = if along_x { (u, t) } else { (t, u) };
                    match shape {
                        0 => Column {
                            index: i,
                            x,
                            z,
                            ylo: 1,
                            yhi: 1,
                            y0: Some(1),
                            vertical: false,
                        },
                        1 => Column {
                            index: i,
                            x,
                            z,
                            ylo: 1,
                            yhi: 3,
                            y0: None,
                            vertical: false,
                        },
                        _ => Column {
                            index: i,
                            x,
                            z,
                            ylo: 1,
                            yhi: 4,
                            y0: None,
                            vertical: true,
                        },
                    }
                })
                .collect();
            let station = Station {
                key: StationKey::Side(Side::A),
                label: "the generated station".to_string(),
                deferred: false,
                columns: columns.clone(),
            };
            let w = Way {
                width,
                ..way(vec![station.clone()])
            };
            let sections = cross_sections(std::slice::from_ref(&w));
            assert_eq!(sections.len(), 1);
            let cs = &sections[0];
            assert_eq!(cs.width, station.columns.len());
            let rec = judge_over(w, &m, &[], &[]);
            assert!(
                rec.stations[0].clear.iter().all(|c| *c <= cs.width),
                "{:?} against {}",
                rec.stations[0].clear,
                cs.width
            );
            let spanned: BTreeSet<[i32; 2]> = cs.cells.iter().map(|c| [c[0], c[2]]).collect();
            let judged: BTreeSet<[i32; 2]> = columns.iter().map(|c| [c.x, c.z]).collect();
            assert_eq!(spanned, judged, "the cut spans the judged columns");
            for c in &columns {
                let body: Vec<[i32; 3]> = match c.y0 {
                    Some(y0) if column_clear(&whole, c, y0) => [y0, y0 + 1]
                        .into_iter()
                        .map(|y| [c.x, y, c.z])
                        .filter(|cell| super::super::shutmodel::can_be(&whole, *cell))
                        .collect(),
                    Some(_) => Vec::new(),
                    None if c.vertical => (c.ylo..=c.yhi).map(|y| [c.x, y, c.z]).collect(),
                    None => lowest_body(&whole, c)
                        .map(|y| vec![[c.x, y, c.z]])
                        .unwrap_or_default(),
                };
                for cell in body {
                    assert!(
                        cs.cells.contains(&cell),
                        "a body at {cell:?} passes the column and not the cut"
                    );
                    bodies += 1;
                }
            }
            stations += 1;
        }
        assert_eq!(stations, 24, "every generated station was judged");
        assert!(bodies > 0, "the generator puts a body in some column");
    }

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
                        let here = super::super::shutmodel::can_be(&whole, [x, y, z]);
                        let over = super::super::shutmodel::can_be(&whole, [x, y + 1, z]);
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
                                super::super::shutmodel::can_be(&local, [x, yy, z]),
                                super::super::shutmodel::can_be(&whole, [x, yy, z]),
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
    /// cell moved out of the approach, beside the flight, is green.
    #[test]
    fn a_piece_flight_narrowed_at_its_foot_is_refused_in_the_piece_frame() {
        use delvewright_dsl::prefab::{
            ContractEdge, ContractSpace, ContractVolume, Region, SpatialContract,
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
        let edge = ContractEdge {
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
            extra: BTreeMap::new(),
        };
        let c = SpatialContract {
            entry: "room".to_string(),
            spaces: spaces.clone(),
            no_body: BTreeMap::new(),
            edges: vec![edge],
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
        let floor_only = m.clone();
        m.insert([103, 65, 106], BlockState::new(MASONRY));
        let run = |m: &BlockMap| {
            let mut census = Census::default();
            let mut out = Vec::new();
            piece_ways_over(
                &[(&placement, &c)],
                false,
                &BTreeSet::new(),
                &mut census,
                &mut out,
            );
            assert_eq!(census.edges, 1);
            assert_eq!(out.len(), 1, "the flight is one piece way");
            judge_over(out.remove(0), m, &[], &[])
        };
        let rec = run(&m);
        let f = rec.refusal.as_ref().expect("refused");
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
            f.message
                .contains("[3, 1, 6] in `prefab/test-flight`'s own frame"),
            "{}",
            f.message
        );

        // The same stone one column outward, beside the flight's three.
        let mut beside = floor_only;
        beside.insert([105, 65, 106], BlockState::new(MASONRY));
        let rec = run(&beside);
        assert!(rec.refusal.is_none(), "{:?}", rec.refusal);
        assert_eq!(rec.verdict(), "whole");

        // A flight a stair seam took as its run is judged under the seam, once.
        let mut census = Census::default();
        let mut out = Vec::new();
        let flights: BTreeSet<(String, usize)> = [("prefab/test-flight".to_string(), 0)]
            .into_iter()
            .collect();
        piece_ways_over(&[(&placement, &c)], false, &flights, &mut census, &mut out);
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
            sides: Default::default(),
        };
        let loft = PlacedBox {
            node: node("node/loft"),
            foot: [11, 18, 0, 9],
            floor: 69,
            clearance: 3,
            open: false,
            base: Base::Ground,
            roof: None,
            sides: Default::default(),
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
            extra: Default::default(),
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

    /// A host piece's flight that runs along the wall under the opening — a
    /// stair up the yard's south wall, rising west to a landing at the face —
    /// travels on an axis the plane does not cut. Its head is the end its
    /// courses climb to, so its foot's approach is judged in the room it
    /// starts from, and its columns are counted across the flight from its own
    /// first column. Both were read off the plane before: the foot was taken
    /// for the head and went unjudged, and every column was numbered 0.
    #[test]
    fn a_host_flight_along_the_wall_is_judged_at_its_foot() {
        use delvewright_dsl::prefab::{
            ContractEdge, ContractSpace, ContractVolume, Region, SpatialContract,
        };
        use delvewright_dsl::siteplan::{Base, Face};
        let node = |n: &str| delvewright_dsl::NodeId(n.to_string());
        let yard = PlacedBox {
            node: node("node/yard"),
            foot: [0, 17, 0, 9],
            floor: 64,
            clearance: 9,
            open: false,
            base: Base::Ground,
            roof: None,
            sides: Default::default(),
        };
        let ledge = PlacedBox {
            node: node("node/ledge"),
            foot: [0, 9, 11, 16],
            floor: 69,
            clearance: 3,
            open: false,
            base: Base::Ground,
            roof: None,
            sides: Default::default(),
        };
        let seam = PlacedSeam {
            edge: delvewright_dsl::ids::EdgeId("edge/yard-ledge".to_string()),
            class: "stair",
            a: node("node/yard"),
            b: node("node/ledge"),
            face: Face::South,
            normal_axis: 2,
            plane: 10,
            opening: ([2, 69, 10], [4, 71, 10]),
            shared: ([0, 64, 10], [9, 71, 10]),
            crossing: Crossing::Portal,
            rise: 5,
            stair_in: Some(node("node/yard")),
            form: "a stone stair up the yard's south wall".to_string(),
            shut: false,
        };
        let region = |a: [i32; 3], b: [i32; 3]| Region { from: a, to: b };
        let space = |a: [i32; 3], b: [i32; 3]| ContractSpace {
            envelope: "enclosed".to_string(),
            boxes: vec![region(a, b)],
        };
        let mut spaces = BTreeMap::new();
        spaces.insert("room".to_string(), space([6, 1, 1], [16, 8, 9]));
        spaces.insert("landing".to_string(), space([1, 6, 7], [5, 8, 9]));
        let contract = SpatialContract {
            entry: "room".to_string(),
            spaces,
            no_body: BTreeMap::new(),
            edges: vec![
                ContractEdge {
                    a: "landing".to_string(),
                    b: "exterior".to_string(),
                    class: "walk".to_string(),
                    rise: Some(0),
                    via: Some(ContractVolume {
                        region: "stair-head".to_string(),
                        boxes: vec![region([2, 6, 10], [4, 8, 10])],
                    }),
                    bar: None,
                    way: None,
                    extra: Default::default(),
                },
                ContractEdge {
                    a: "room".to_string(),
                    b: "landing".to_string(),
                    class: "stair".to_string(),
                    rise: Some(5),
                    via: Some(ContractVolume {
                        region: "flight".to_string(),
                        boxes: (6..=9).map(|x| region([x, 10 - x, 7], [x, 8, 9])).collect(),
                    }),
                    bar: None,
                    way: None,
                    extra: Default::default(),
                },
            ],
            faces: Vec::new(),
            no_body_majority_ack: None,
        };
        let placement = crate::compiler::plan::PiecePlacement {
            prefab_id: "prefab/test-yard".to_string(),
            templates: Vec::new(),
            pos: [0, 63, 0],
            size: [18, 10, 11],
            rotation: crate::compiler::solver::Rotation::None,
            mated: Vec::new(),
        };
        let by_node: BTreeMap<&str, &PlacedBox> = [("node/yard", &yard), ("node/ledge", &ledge)]
            .into_iter()
            .collect();
        let pieces: BTreeMap<&str, BoundPiece<'_>> = [(
            "node/yard",
            BoundPiece {
                placement: &placement,
                contract: &contract,
            },
        )]
        .into_iter()
        .collect();
        let input = SeamInput {
            s: &seam,
            boxes: &by_node,
            laid: None,
            pieces: &pieces,
            own_gates: &[],
        };
        let mut census = Census::default();
        let mut flights = BTreeSet::new();
        let way = seam_way(&input, &mut census, &mut flights).expect("a way");
        assert_eq!(way.run, Run::Piece);
        assert_eq!(
            way.width, 3,
            "the ledger's width is the opening's declared 3 columns, not the one cell the opening spans across a run along the wall"
        );
        let labels: Vec<&str> = way.stations.iter().map(|s| s.label.as_str()).collect();
        let foot = way
            .stations
            .iter()
            .find(|s| s.label == "the foot's approach in `room`")
            .unwrap_or_else(|| panic!("the foot is judged: {labels:?}"));
        let cells: Vec<(usize, i32, i32, i32)> = foot
            .columns
            .iter()
            .map(|c| (c.index, c.x, c.ylo, c.z))
            .collect();
        assert_eq!(
            cells,
            vec![(0, 10, 64, 7), (1, 10, 64, 8), (2, 10, 64, 9)],
            "the foot's approach is the row east of the lowest tread, counted from the flight's first column"
        );
        let course0 = way
            .stations
            .iter()
            .find(|s| s.label == "course 0 of the run")
            .expect("course 0");
        assert!(
            course0.columns.iter().all(|c| c.x == 6),
            "course 0 is the top tread: {:?}",
            course0.columns
        );
    }

    /// A contract edge that realizes a plan seam is the seam's way and no
    /// piece way of its own; the same piece placed by `areas[]` is one.
    #[test]
    fn an_edge_that_realizes_a_seam_is_judged_under_the_seam() {
        use delvewright_dsl::prefab::{
            ContractEdge, ContractSpace, ContractVolume, Region, SpatialContract,
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
                extra: BTreeMap::new(),
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
        piece_ways_over(
            &[(&placement, &contract)],
            true,
            &BTreeSet::new(),
            &mut census,
            &mut out,
        );
        assert!(
            out.is_empty(),
            "an exterior edge of a site-plan piece answers a seam"
        );
        // The same piece placed by `areas[]`: its door is a piece way of its own.
        piece_ways_over(
            &[(&placement, &contract)],
            false,
            &BTreeSet::new(),
            &mut census,
            &mut out,
        );
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
            "clear width (DW1057): 0 way(s) of 0 traversal seam(s), 0 box(es) and 0 contract \
             edge(s) — 0 portal(s), 0 stair run(s), 0 contact(s), 0 way place(s), 0 open side(s), \
             0 piece way(s); 0 column(s) at 0 station(s) judged over 1 state(s); 0 way(s) whole, 0 \
             narrowed; 0 plane column(s) left to DW0836; 0 stair run(s) undeclared by their piece"
        );
        let j = b.to_json();
        assert_eq!(j["ways"], 0);
        assert_eq!(j["codes"], json!(["DW1057", "DW1065"]));
        assert_eq!(j["open_sides"], 0);
    }

    /// An open side's ring as a way (spec-0123 §5.4): the same three columns
    /// as the yard's foot, judged as an open side.
    fn open_side() -> Way {
        Way {
            kind: WayKind::OpenSide,
            name: "node/yard east".to_string(),
            label: "the east side of `node/yard`".to_string(),
            width: 3,
            stations: vec![Station {
                key: StationKey::OpenSide(0),
                label: "the ring of the east side".to_string(),
                ..foot()
            }],
            own: BTreeMap::new(),
            piece: None,
            run: Run::None,
        }
    }

    /// A post in one column of an open side narrows it: `DW1057`, its ledger
    /// row of kind `open side`; the clear side is whole.
    #[test]
    fn a_post_in_an_open_side_is_dw1057_of_kind_open_side() {
        let clear = judge_over(open_side(), &floor(), &[], &[]);
        assert_eq!(clear.verdict(), "whole");
        let mut m = floor();
        put(&mut m, [[10, 1, 13]], "minecraft:oak_fence");
        let rec = judge_over(open_side(), &m, &[], &[]);
        assert_eq!(rec.refusal.as_ref().map(|f| f.code.id()), Some("DW1057"));
        assert_eq!(rec.to_json()["kind"], "open side");
        // A station every column of which a seam covers is judged under the
        // seam, and the ledger says so rather than calling it shut.
        let mut covered = open_side();
        covered.stations[0].columns.clear();
        let rec = judge_over(covered, &floor(), &[], &[]);
        assert!(rec.refusal.is_none());
        assert_eq!(
            rec.to_json()["stations"][0]["verdict"],
            "judged under its seam"
        );
    }

    /// A hedge along the whole run leaves no column: `DW1065`, naming every
    /// taking cell with its block; the same hedge across a portal's foot is no
    /// finding of this check (a way shut in every column is its gate's).
    #[test]
    fn an_open_side_built_shut_is_dw1065_naming_every_cell() {
        let mut m = floor();
        let hedge = "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]";
        put(&mut m, (12..=14).map(|z| [10, 1, z]), hedge);
        let rec = judge_over(open_side(), &m, &[], &[]);
        let f = rec
            .refusal
            .as_ref()
            .expect("an open side built shut is refused");
        assert_eq!(f.code.id(), "DW1065");
        assert!(f.message.contains("3 of 3 columns taken"), "{}", f.message);
        for z in 12..=14 {
            assert!(
                f.message.contains(&format!("[10, 1, {z}]")),
                "{}",
                f.message
            );
        }
        assert!(f.message.contains("oak_leaves"), "{}", f.message);
        let portal = judge_over(way(vec![foot()]), &m, &[], &[]);
        assert!(portal.refusal.is_none(), "{:?}", portal.refusal);
    }

    #[test]
    fn the_codes_are_their_numbers() {
        assert_eq!(DW_WAY_NARROWED.id(), "DW1057");
        assert_eq!(DW_OPEN_SIDE_SHUT.id(), "DW1065");
    }
}
