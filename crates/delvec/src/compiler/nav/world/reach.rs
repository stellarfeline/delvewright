//! **The reach graph** (spec-0113 §4): where a vanilla body can be in a
//! [`World`], as rest states joined by movements the body simulator
//! ([`delvewright_dsl::metrics::body`]) proves by flying them, tick by tick,
//! over the assembled world with every premise the world carries applied.
//!
//! Nodes are cells, each in the one rest state the world gives it
//! ([`RestKind`]). Edges from a standing cell are the walk relation's
//! ([`World::neighbors_fp`]) and every launch of the launch set (§4.3) that
//! comes to rest somewhere else; from a water cell, the six water cells beside
//! it and every simulated climb out; from a climb cell, the climb's own moves
//! and every simulated let-go. The graph over-approximates where a body can be:
//! a proof that reads it can only turn red where the walk relation was green.
//!
//! **Templates propose, simulation disposes** (§3.3). The launch set from a
//! cell is first flown once over a template world — an endless floor of the
//! launch cell's support class, the launch column's own cells over it, air
//! everywhere else — recording every cell each launch read and what the
//! template held there. A world cell whose cells match a launch's record
//! answers exactly as the template did, so it is credited without a tick; a
//! launch whose record meets anything else is flown over the real world.
//!
//! The work is bounded and the bound refused, never truncated
//! ([`DW_REACH_BUDGET`]).

use super::World;
use crate::compiler::nav::world::Ambient;
use crate::compiler::nav::world::body::Footprint;
use delvewright_dsl::metrics::body::{
    self as sim, Aabb, Attributes, CollisionWorld, Fluid, Input, Material, Rules, State,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

/// The rest state a cell holds a body in (spec-0113 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RestKind {
    /// On a collision top: a floor, a partial block, a fence, wall or gate top.
    Standing,
    /// At the top of water.
    Afloat,
    /// In water with water over it.
    Submerged,
    /// On a climbable the world keeps (spec-0099).
    Holding,
}

/// How one edge of the graph moves a body (spec-0113 §5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    /// Walking or sprinting on the ground, the walk relation's steps included.
    Walk,
    /// Off a ledge without a jump.
    Fall,
    /// With the jump pressed.
    Jump,
    /// Swimming or sinking from water cell to water cell.
    Dive,
    /// On a climbable, or out of water onto a ledge.
    Climb,
    /// Out of the world: into the void, or into the open sea (§4.7).
    Exit,
}

impl EdgeKind {
    /// Every kind, in the order the binding line counts them.
    pub const ALL: [EdgeKind; 6] = [
        EdgeKind::Walk,
        EdgeKind::Fall,
        EdgeKind::Jump,
        EdgeKind::Dive,
        EdgeKind::Climb,
        EdgeKind::Exit,
    ];
}

/// Where the body went at the end of an edge, in words a report can use: the
/// launch's shape and how far it fell, so a refusal reads "a sprint jump
/// north-east up one" rather than a pair of coordinates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    /// The rest cell the movement ends in.
    pub to: [i32; 3],
    /// The movement's kind.
    pub kind: EdgeKind,
    /// The facing the launch flew at, whole degrees (`None` for a cell-relation
    /// move).
    pub yaw: Option<i16>,
    /// Whether the launch was sprinting.
    pub sprint: bool,
}

// ---------------------------------------------------------------------------
// The world the simulator reads: a dense snapshot of the assembled world.
// ---------------------------------------------------------------------------

/// A cell's code: bits 0..=4 the collision shape (`0` none, `1..=16` a floor
/// that many sixteenths tall, [`BARRIER`] a fence-class 1.5-block box), bit 5
/// water, bit 6 lava, bit 7 a kept climbable, bits 8..=11 the [`Material`].
type Code = u16;
const SHAPE: Code = 0x1f;
/// A fence, wall or gate: a full-width box 1.5 tall. The model carries no
/// post-and-arm geometry, so a barrier is read as its whole cell — more top to
/// land on and less room to squeeze past (stated in `docs/reference/compiler.md`).
const BARRIER: Code = 24;
const WATER: Code = 1 << 5;
const LAVA: Code = 1 << 6;
const CLIMB: Code = 1 << 7;

fn material_code(m: Material) -> Code {
    (match m {
        Material::Plain => 0,
        Material::Ice => 1,
        Material::BlueIce => 2,
        Material::Slime => 3,
        Material::Honey => 4,
        Material::SoulSand => 5,
        Material::Hay => 6,
        Material::Bed => 7,
        Material::Cobweb => 8,
        Material::Climbable => 9,
    }) << 8
}

fn material_of_code(c: Code) -> Material {
    match (c >> 8) & 0xf {
        1 => Material::Ice,
        2 => Material::BlueIce,
        3 => Material::Slime,
        4 => Material::Honey,
        5 => Material::SoulSand,
        6 => Material::Hay,
        7 => Material::Bed,
        8 => Material::Cobweb,
        _ if c & CLIMB != 0 => Material::Climbable,
        _ => Material::Plain,
    }
}

/// The collision top of a code's shape above its cell floor, in blocks.
fn top_of(c: Code) -> Option<f64> {
    match c & SHAPE {
        0 => None,
        BARRIER => Some(1.5),
        h => Some(f64::from(h) / 16.0),
    }
}

/// What lies outside the snapshot's window.
#[derive(Debug, Clone, Copy)]
enum Outside {
    Void,
    Ocean { level: i32, floor_top: i32 },
}

/// A dense snapshot of a [`World`] over its window: the built volume grown by
/// [`WINDOW_MARGIN`] (spec-0113 §4.7).
pub(in crate::compiler::nav) struct Dense {
    lo: [i32; 3],
    dims: [i32; 3],
    cells: Vec<Code>,
    outside: Outside,
    /// Whether any cell holds water, lava or a cobweb, here or outside.
    wet: bool,
}

/// How far the window reaches past the built volume, horizontally: past the
/// farthest arc a launch flies (a sprint jump carries `3.63`, a fall drifts
/// under `4.5`), so a body that leaves the window has left the content.
const WINDOW_MARGIN: i32 = 8;
/// How far the window reaches over the highest modelled cell.
const WINDOW_OVER: i32 = 6;

impl Dense {
    fn of(world: &World) -> Dense {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        let mut note = |c: [i32; 3]| {
            for i in 0..3 {
                lo[i] = lo[i].min(c[i]);
                hi[i] = hi[i].max(c[i]);
            }
        };
        // The window is the modelled cells' extent, never the premises': two
        // views of one world (one with its campaign's premises, one with
        // geometry alone) share a window, and so a lineage. Under an ocean the
        // placed pieces' boxes join it, because they decide where the sea is.
        if matches!(world.ambient, Ambient::Ocean(_)) {
            for (_, (a, b)) in &world.built {
                note(*a);
                note(*b);
            }
        }
        for set in [&world.solid, &world.tall, &world.use_gates, &world.flooded] {
            for c in set.iter() {
                note(c);
            }
        }
        for c in world.climb.iter() {
            note(c);
        }
        let outside = match &world.ambient {
            Ambient::Void => Outside::Void,
            Ambient::Ocean(sea) => Outside::Ocean {
                level: sea.level,
                floor_top: sea.floor_top,
            },
        };
        if lo[0] == i32::MAX {
            return Dense {
                lo: [0; 3],
                dims: [0; 3],
                cells: Vec::new(),
                outside,
                wet: matches!(outside, Outside::Ocean { .. }),
            };
        }
        lo[0] -= WINDOW_MARGIN;
        lo[2] -= WINDOW_MARGIN;
        hi[0] += WINDOW_MARGIN;
        hi[2] += WINDOW_MARGIN;
        hi[1] += WINDOW_OVER;
        lo[1] -= 2;
        if let Outside::Ocean { floor_top, level } = outside {
            lo[1] = lo[1].min(floor_top);
            hi[1] = hi[1].max(level + WINDOW_OVER);
        }
        let dims = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let n = dims.iter().map(|d| *d as usize).product();
        let mut d = Dense {
            lo,
            dims,
            cells: vec![0; n],
            outside,
            wet: matches!(outside, Outside::Ocean { .. })
                || !world.flooded.is_empty()
                || world.material.iter().any(|(_, m)| *m == Material::Cobweb),
        };
        // The ambient first, outside every placed piece; then the model's own
        // cells, which win wherever they are.
        if let Outside::Ocean { level, floor_top } = outside {
            let mut built = vec![false; n];
            for (_, (a, b)) in &world.built {
                for x in a[0].min(b[0])..=a[0].max(b[0]) {
                    for y in a[1].min(b[1])..=a[1].max(b[1]) {
                        for z in a[2].min(b[2])..=a[2].max(b[2]) {
                            if let Some(i) = d.index([x, y, z]) {
                                built[i] = true;
                            }
                        }
                    }
                }
            }
            for x in lo[0]..=hi[0] {
                for z in lo[2]..=hi[2] {
                    for y in lo[1]..=hi[1] {
                        let i = d.index([x, y, z]).expect("inside");
                        if built[i] {
                            continue;
                        }
                        d.cells[i] = if y <= floor_top {
                            16
                        } else if y <= level {
                            WATER
                        } else {
                            0
                        };
                    }
                }
            }
        }
        let mut set = |c: [i32; 3], f: &dyn Fn(Code) -> Code| {
            if let Some(i) = d.index(c) {
                d.cells[i] = f(d.cells[i]);
            }
        };
        for c in world.solid.iter() {
            let top = world.partial.get(&c).copied().unwrap_or(16) as Code;
            set(c, &|_| top.max(1));
        }
        for c in world.tall.iter().chain(world.use_gates.iter()) {
            set(c, &|_| BARRIER);
        }
        for c in world.flooded.iter() {
            let lava = world.lava.contains(&c);
            set(c, &|_| if lava { LAVA } else { WATER });
        }
        for c in world.climb.iter() {
            set(c, &|v| (v & !SHAPE) | CLIMB);
        }
        for (c, m) in world.material.iter() {
            let m = material_code(*m);
            set(*c, &|v| (v & 0xff) | m);
        }
        d
    }

    #[inline]
    fn index(&self, c: [i32; 3]) -> Option<usize> {
        let x = c[0].wrapping_sub(self.lo[0]) as u32;
        let y = c[1].wrapping_sub(self.lo[1]) as u32;
        let z = c[2].wrapping_sub(self.lo[2]) as u32;
        if x < self.dims[0] as u32 && y < self.dims[1] as u32 && z < self.dims[2] as u32 {
            Some(
                ((x as usize * self.dims[2] as usize) + z as usize) * self.dims[1] as usize
                    + y as usize,
            )
        } else {
            None
        }
    }

    #[inline]
    fn code(&self, c: [i32; 3]) -> Code {
        match self.index(c) {
            Some(i) => self.cells[i],
            None => match self.outside {
                Outside::Void => 0,
                Outside::Ocean { level, floor_top } => {
                    if c[1] <= floor_top {
                        16
                    } else if c[1] <= level {
                        WATER
                    } else {
                        0
                    }
                }
            },
        }
    }

    /// Whether `c` lies on the window's horizontal rim.
    fn on_rim(&self, c: [i32; 3]) -> bool {
        self.dims[0] > 0
            && (c[0] <= self.lo[0]
                || c[2] <= self.lo[2]
                || c[0] >= self.lo[0] + self.dims[0] - 1
                || c[2] >= self.lo[2] + self.dims[2] - 1)
    }

    fn inside(&self, c: [i32; 3]) -> bool {
        self.index(c).is_some()
    }

    fn bottom(&self) -> i32 {
        self.lo[1]
    }
}

/// Anything the launch procedure reads cells through: a world's snapshot, the
/// template, or a recorder around either.
trait Codes {
    fn code(&self, c: [i32; 3]) -> Code;
    /// Whether any cell can hold water, lava or a cobweb.
    fn wet(&self) -> bool;
}

impl Codes for Dense {
    fn code(&self, c: [i32; 3]) -> Code {
        Dense::code(self, c)
    }
    fn wet(&self) -> bool {
        self.wet
    }
}

/// [`Codes`] as the simulator's [`CollisionWorld`].
struct Sim<'a, C: Codes + ?Sized>(&'a C);

impl<C: Codes + ?Sized> CollisionWorld for Sim<'_, C> {
    fn shapes(&self, c: [i32; 3], out: &mut Vec<Aabb>) {
        if let Some(top) = top_of(self.0.code(c)) {
            let (x, y, z) = (f64::from(c[0]), f64::from(c[1]), f64::from(c[2]));
            out.push(Aabb::new(x, y, z, x + 1.0, y + top, z + 1.0));
        }
    }
    fn material(&self, c: [i32; 3]) -> Material {
        material_of_code(self.0.code(c))
    }
    fn may_hold_fluid_or_web(&self) -> bool {
        self.0.wet()
    }
    fn fluid(&self, c: [i32; 3]) -> Option<Fluid> {
        let v = self.0.code(c);
        if v & WATER != 0 {
            Some(Fluid::SOURCE_WATER)
        } else if v & LAVA != 0 {
            Some(Fluid {
                lava: true,
                ..Fluid::SOURCE_WATER
            })
        } else {
            None
        }
    }
}

/// The template world of one launch class: an endless floor of `support` at
/// `y = -1`, the launch column `column` at `x = z = 0`, air everywhere else.
struct Template {
    support: Code,
    column: [Code; COLUMN],
}

/// How many cells of the launch column a launch class carries: the feet cell
/// and three over it, the room a jump's arc uses.
const COLUMN: usize = 4;

impl Codes for Template {
    fn wet(&self) -> bool {
        [self.support]
            .iter()
            .chain(&self.column)
            .any(|v| v & (WATER | LAVA) != 0 || (v >> 8) & 0xf == 8)
    }
    fn code(&self, c: [i32; 3]) -> Code {
        if c[1] == -1 {
            self.support
        } else if c[0] == 0 && c[2] == 0 && (0..COLUMN as i32).contains(&c[1]) {
            self.column[c[1] as usize]
        } else {
            0
        }
    }
}

/// A [`Codes`] that writes down every cell it is asked about.
struct Recorder<'a, C: Codes> {
    inner: &'a C,
    seen: std::cell::RefCell<Vec<[i32; 3]>>,
}

impl<C: Codes> Codes for Recorder<'_, C> {
    fn wet(&self) -> bool {
        self.inner.wet()
    }
    fn code(&self, c: [i32; 3]) -> Code {
        self.seen.borrow_mut().push(c);
        self.inner.code(c)
    }
}

/// A [`Codes`] relative to a world cell: the template's origin put at `at`.
struct Shifted<'a> {
    dense: &'a Dense,
    at: [i32; 3],
}

impl Codes for Shifted<'_> {
    fn wet(&self) -> bool {
        self.dense.wet
    }
    fn code(&self, c: [i32; 3]) -> Code {
        self.dense
            .code([c[0] + self.at[0], c[1] + self.at[1], c[2] + self.at[2]])
    }
}

// ---------------------------------------------------------------------------
// The launch set (spec-0113 §4.3).
// ---------------------------------------------------------------------------

/// The facing step, in degrees (spec-0113 §4.3: one degree, through the
/// game's own sine table).
pub const FACING_STEP_DEG: f64 = 1.0;

/// How a launch leaves its cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Mode {
    /// From the cell centre (a climb's: at rest), the jump pressed on tick
    /// `press` (`None`: never).
    Approach { press: Option<u8> },
    /// From the furthest supported position toward the exit edge at a
    /// saturated sprint: the jump pressed on tick `0` (the last supported tick)
    /// or `1` (the lagged `onGround` tick), or never.
    Edge { press: Option<u8> },
    /// From the furthest supported position at a walk, never jumping.
    WalkOff,
    /// From the furthest supported position at rest, tipping off it.
    Creep,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Launch {
    yaw: f64,
    strafe: f64,
    mode: Mode,
    /// The saturated velocity of the launch's input.
    vel: [f64; 3],
    /// The horizontal directions the launch can carry a body in: bit 0 `+x`,
    /// 1 `-x`, 2 `+z`, 3 `-z`. Every horizontal push a launch gets is along its
    /// facing (the sprint jump's boost) or its input (the run-up and the
    /// acceleration), and a collision only zeroes a component, so a component
    /// neither has positive never becomes positive.
    signs: u8,
}

/// The horizontal directions the vectors `vs` push a body in ([`Launch::signs`]).
fn signs_of(vs: &[[f64; 2]]) -> u8 {
    let mut s = 0u8;
    for v in vs {
        if v[0] > 1e-12 {
            s |= 1;
        }
        if v[0] < -1e-12 {
            s |= 2;
        }
        if v[1] > 1e-12 {
            s |= 4;
        }
        if v[1] < -1e-12 {
            s |= 8;
        }
    }
    s
}

/// Which launches a node's open envelope asks for: bit `(rx + 1) * 3 + (rz + 1)`
/// set when some open cell needs a carry of sign `rx` along `x` and `rz` along
/// `z` (`0`: either). `0` asks for none; [`ALL_QUADRANTS`] for every launch.
type Quadrants = u16;
const ALL_QUADRANTS: Quadrants = 0x1ff;

fn quadrant_bit(rx: i32, rz: i32) -> Quadrants {
    1 << ((rx + 1) * 3 + (rz + 1))
}

impl Launch {
    /// Whether this launch can carry a body toward a cell needing `rx`, `rz`.
    fn serves(&self, q: Quadrants) -> bool {
        if q == ALL_QUADRANTS {
            return true;
        }
        (0..9).any(|i| {
            if q & (1 << i) == 0 {
                return false;
            }
            let (rx, rz) = (i / 3 - 1, i % 3 - 1);
            let x = match rx {
                1 => self.signs & 1 != 0,
                -1 => self.signs & 2 != 0,
                _ => true,
            };
            let z = match rz {
                1 => self.signs & 4 != 0,
                -1 => self.signs & 8 != 0,
                _ => true,
            };
            x && z
        })
    }
}

/// The launch set (spec-0113 §4.3), at a facing step of `step` degrees and the
/// strafe values `strafes`. From a standing cell, at each facing: from the
/// furthest supported position toward the exit edge, a saturated sprint with
/// the jump pressed on the last supported tick or on the lagged one, or never
/// (sprinting off); a walk off; and a tip off from rest — walking off at its
/// fastest and its slowest. From a climb, a push off at each facing from rest.
fn launch_set(holding: bool, step: f64, strafes: &[f64]) -> Vec<Launch> {
    let mut out = Vec::new();
    let n = (360.0 / step).round() as i32;
    for k in 0..n {
        let yaw = f64::from(k) * step - 180.0;
        for &strafe in strafes {
            let vel = |sprint: bool| {
                saturated(&Input {
                    forward: 1,
                    strafe,
                    jump: false,
                    sprint,
                    sneak: false,
                    yaw,
                    magnitude: 1.0,
                })
            };
            let (run, walk) = (vel(true), vel(false));
            let r = (yaw as f32) * (std::f32::consts::PI / 180.0);
            let facing = [-f64::from(r.sin()), f64::from(r.cos())];
            let signs = signs_of(&[facing, [run[0], run[2]], [walk[0], walk[2]]]);
            let mut push = |mode: Mode, vel: [f64; 3]| {
                out.push(Launch {
                    yaw,
                    strafe,
                    mode,
                    vel,
                    signs,
                })
            };
            if holding {
                push(Mode::Approach { press: None }, run);
                continue;
            }
            for press in [Some(0), Some(1), None] {
                push(Mode::Edge { press }, run);
            }
            push(Mode::WalkOff, walk);
            push(Mode::Creep, walk);
        }
    }
    out
}

/// [`launch_set`] at `params`, built once per process.
fn shared_launch_set(holding: bool, params: &ReachParams) -> Arc<Vec<Launch>> {
    type Sets = Mutex<HashMap<(bool, String), Arc<Vec<Launch>>>>;
    static MEMO: OnceLock<Sets> = OnceLock::new();
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (
        holding,
        format!("{}:{:?}", params.facing_step, params.strafes),
    );
    if let Some(l) = memo.lock().expect("memo").get(&key) {
        return Arc::clone(l);
    }
    let l = Arc::new(launch_set(holding, params.facing_step, &params.strafes));
    memo.lock()
        .expect("memo")
        .entry(key)
        .or_insert_with(|| Arc::clone(&l))
        .clone()
}

/// The parameters a graph is built at: the facing step and strafe values
/// (§4.3; the convergence test builds a finer graph beside the shipped one),
/// and whether every fall is survivable (§4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ReachParams {
    /// Degrees between facings.
    pub facing_step: f64,
    /// The strafe inputs taken at each facing.
    pub strafes: Vec<f64>,
    /// Whether no fall kills (a kit reduces fall damage, §4.2).
    pub unbounded_fall: bool,
    /// The work past which a closure is refused rather than finished
    /// (`DW1042`): launches simulated plus template arcs credited.
    pub budget: u64,
}

impl ReachParams {
    /// The shipped launch set: every degree, strafe `−1`, `0`, `+1`.
    pub fn shipped(unbounded_fall: bool) -> ReachParams {
        ReachParams {
            facing_step: FACING_STEP_DEG,
            strafes: vec![-1.0, 0.0, 1.0],
            unbounded_fall,
            budget: delvewright_dsl::metrics::REACH_BUDGET_STATES,
        }
    }

    fn key(&self) -> String {
        format!(
            "{}:{:?}:{}",
            self.facing_step, self.strafes, self.unbounded_fall
        )
    }
}

/// What one launch did.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Outcome {
    /// Nothing new: it came back to rest in its own cell, or never settled.
    Nowhere,
    /// It died: a lethal fall, lava, or (over the real world) a killing volume.
    Dead(Death),
    /// It left the world: below the window's floor with nothing under it.
    LeftWorld,
    /// It came to rest in these cells (relative to the launch cell), in this
    /// state, by this kind of movement.
    Rest(Vec<([i32; 3], RestKind)>, EdgeKind),
}

/// What killed a body a launch flew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Death {
    /// A declared killing volume.
    Volume,
    /// Lava.
    Lava,
    /// A fall it does not survive.
    Fall,
    /// Out of the world, into the void.
    Void,
}

/// The ticks a launch is flown for before it is called unsettled: long enough
/// to fall the window's whole height.
const LAUNCH_TICKS: usize = 160;

fn cell_of(p: [f64; 3]) -> [i32; 3] {
    [
        p[0].floor() as i32,
        p[1].floor() as i32,
        p[2].floor() as i32,
    ]
}

/// The feet height a body standing in `c` rests at, read through `codes`:
/// on the collision top of the block under the cell.
fn feet_of<C: Codes + ?Sized>(codes: &C, c: [i32; 3]) -> Option<f64> {
    top_of(codes.code([c[0], c[1] - 1, c[2]])).map(|t| f64::from(c[1] - 1) + t)
}

/// The cells a body that touched down with its feet at `p` rests in: every
/// column the box widened by `0.1` overlaps whose support's collision top is
/// at the feet (spec-0113 §4.3).
fn landing_cells<C: Codes + ?Sized>(codes: &C, body: &Attributes, p: [f64; 3]) -> Vec<[i32; 3]> {
    let h = body.width / 2.0 + LANDING_WIDENING;
    let mut out = Vec::new();
    for x in (p[0] - h).floor() as i32..=(p[0] + h).floor() as i32 {
        for z in (p[2] - h).floor() as i32..=(p[2] + h).floor() as i32 {
            let y0 = (p[1] - 1e-7).floor() as i32;
            // A barrier's top is half a block into the cell over it: the cell
            // under the feet cell's support is asked only for feet at a half.
            let half = ((p[1] - p[1].floor()) - 0.5).abs() < 1e-6;
            for sy in [y0, y0 - 1].into_iter().take(if half { 2 } else { 1 }) {
                if let Some(t) = top_of(codes.code([x, sy, z]))
                    && (f64::from(sy) + t - p[1]).abs() < 1e-6
                {
                    out.push([x, sy + 1, z]);
                }
            }
        }
    }
    out
}

/// The widening of the body's box a landing is credited by (spec-0113 §4.3).
pub const LANDING_WIDENING: f64 = 0.1;

fn water_kind<C: Codes + ?Sized>(codes: &C, c: [i32; 3]) -> RestKind {
    let up = codes.code([c[0], c[1] + 1, c[2]]);
    if up & WATER != 0 || top_of(up).is_some() {
        RestKind::Submerged
    } else {
        RestKind::Afloat
    }
}

/// The furthest position along `dir` from the centre of `c` that the body's box
/// is still supported at — the box overhanging the cell's faces by up to its
/// half width, the support any collision top at the feet under the box — found
/// by halving.
fn edge_position<C: Codes + ?Sized>(
    codes: &C,
    body: &Attributes,
    c: [i32; 3],
    feet: f64,
    dir: [f64; 2],
) -> Option<[f64; 3]> {
    let centre = [f64::from(c[0]) + 0.5, feet, f64::from(c[2]) + 0.5];
    let at = |t: f64| [centre[0] + dir[0] * t, feet, centre[2] + dir[1] * t];
    let world = Sim(codes);
    let mut boxes = Vec::new();
    let ok = |p: [f64; 3], boxes: &mut Vec<Aabb>| {
        let bb = Aabb::new(
            p[0] - body.width / 2.0,
            p[1],
            p[2] - body.width / 2.0,
            p[0] + body.width / 2.0,
            p[1] + body.height,
            p[2] + body.width / 2.0,
        );
        boxes.clear();
        for x in bb.min[0].floor() as i32..=bb.max[0].floor() as i32 {
            for z in bb.min[2].floor() as i32..=bb.max[2].floor() as i32 {
                for y in (bb.min[1].floor() as i32 - 1)..=bb.max[1].floor() as i32 {
                    world.shapes([x, y, z], boxes);
                }
            }
        }
        // No overlap at all: a box that starts a hair inside a block is not
        // collided with it, and walks through it.
        if boxes.iter().any(|b| b.intersects(&bb)) {
            return false;
        }
        boxes.iter().any(|b| {
            (b.max[1] - feet).abs() < 1e-7
                && b.min[0] < bb.max[0]
                && b.max[0] > bb.min[0]
                && b.min[2] < bb.max[2]
                && b.max[2] > bb.min[2]
        })
    };
    if !ok(centre, &mut boxes) {
        return None;
    }
    // The furthest the centre may go inside the cell along `dir`.
    // The box may hang past the cell's face by its half width and still stand
    // on the cell's own support; the centre goes no further than that.
    let half = body.width / 2.0;
    let reach = |d: f64, c0: f64| {
        if d > 1e-9 {
            (c0 + 1.0 + half - 1e-6 - (c0 + 0.5)) / d
        } else if d < -1e-9 {
            (c0 - half + 1e-6 - (c0 + 0.5)) / d
        } else {
            f64::INFINITY
        }
    };
    let t_max = reach(dir[0], f64::from(c[0])).min(reach(dir[1], f64::from(c[2])));
    if ok(at(t_max), &mut boxes) {
        return Some(at(t_max));
    }
    let (mut lo, mut hi) = (0.0, t_max);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if ok(at(mid), &mut boxes) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(at(lo))
}

/// The saturated velocity a body holding `input` reaches on plain ground
/// (spec-0113 §4.3, "taken from the simulator's own approach rather than
/// assumed"), computed once per launch when the launch set is built.
fn saturated(input: &Input) -> [f64; 3] {
    sim::saturated_velocity(&Attributes::PLAYER, Rules::Game, Material::Plain, input)
}

/// The furthest supported positions [`edge_position`] found from one cell, by
/// direction: every launch along a direction starts there, whatever its gait.
type EdgeMemo = std::cell::RefCell<HashMap<(u64, u64), Option<[f64; 3]>>>;

/// Fly one launch from cell `c` over `codes`; `killed` says whether a body box
/// meets a killing volume (always `false` over a template).
#[allow(clippy::too_many_arguments)]
fn fly<C: Codes + ?Sized>(
    codes: &C,
    c: [i32; 3],
    holding: bool,
    launch: &Launch,
    unbounded_fall: bool,
    bottom: i32,
    killed: &dyn Fn(&State) -> bool,
    edges: Option<&EdgeMemo>,
) -> (Outcome, Option<Aabb>) {
    let body = Attributes::PLAYER;
    let world = Sim(codes);
    let sprint = !matches!(launch.mode, Mode::WalkOff | Mode::Creep);
    let input = Input {
        forward: 1,
        strafe: launch.strafe,
        jump: false,
        sprint,
        sneak: false,
        yaw: launch.yaw,
        magnitude: 1.0,
    };
    let vel = launch.vel;
    let speed = (vel[0] * vel[0] + vel[2] * vel[2]).sqrt();
    let dir = if speed > 0.0 {
        [vel[0] / speed, vel[2] / speed]
    } else {
        [0.0, 0.0]
    };
    let feet = if holding {
        Some(f64::from(c[1]))
    } else {
        feet_of(codes, c)
    };
    let Some(feet) = feet else {
        return (Outcome::Nowhere, None);
    };
    let (mut st, press): (State, Option<u8>) = match launch.mode {
        Mode::Approach { press } => {
            let mut st =
                State::standing([f64::from(c[0]) + 0.5, feet, f64::from(c[2]) + 0.5], &body);
            st.vel = if holding { [0.0; 3] } else { vel };
            st.on_ground = !holding;
            (st, press)
        }
        Mode::Edge { .. } | Mode::WalkOff | Mode::Creep if !holding => {
            let found = match edges {
                Some(m) => *m
                    .borrow_mut()
                    .entry((dir[0].to_bits(), dir[1].to_bits()))
                    .or_insert_with(|| edge_position(codes, &body, c, feet, dir)),
                None => edge_position(codes, &body, c, feet, dir),
            };
            let Some(p) = found else {
                return (Outcome::Nowhere, None);
            };
            let mut st = State::standing(p, &body);
            st.vel = match launch.mode {
                // At rest on the edge: only the tick of gravity a resting body
                // carries.
                Mode::Creep => [0.0, -body.gravity * f64::from(0.98f32), 0.0],
                _ => vel,
            };
            let press = match launch.mode {
                Mode::Edge { press } => press,
                _ => None,
            };
            (st, press)
        }
        _ => return (Outcome::Nowhere, None),
    };
    // An approach pressed late must still be supported when it presses.
    let mut jumped = false;
    let mut airborne = false;
    let mut bbox: Option<Aabb> = None;
    let mut still = 0;
    let origin = [c[0], c[2]];
    for t in 0..LAUNCH_TICKS {
        let mut i = input;
        if let Some(p) = press
            && t >= usize::from(p)
        {
            if t == usize::from(p) && !st.on_ground {
                // The press comes on a tick the body is no longer on the
                // ground: that launch is another one's.
                return (Outcome::Nowhere, bbox);
            }
            i.jump = true;
        }
        let was_ground = st.on_ground;
        sim::tick(&world, &body, Rules::Game, &mut st, &i);
        if i.jump && was_ground && st.vel[1] > 0.0 && !st.in_water {
            jumped = true;
        }
        let bb = st.aabb(&body);
        bbox = Some(match bbox {
            None => bb,
            Some(b) => Aabb::new(
                b.min[0].min(bb.min[0]),
                b.min[1].min(bb.min[1]),
                b.min[2].min(bb.min[2]),
                b.max[0].max(bb.max[0]),
                b.max[1].max(bb.max[1]),
                b.max[2].max(bb.max[2]),
            ),
        });
        if killed(&st) {
            return (Outcome::Dead(Death::Volume), bbox);
        }
        if st.in_lava {
            return (Outcome::Dead(Death::Lava), bbox);
        }
        if !unbounded_fall && st.landing_damage >= body.max_health {
            return (Outcome::Dead(Death::Fall), bbox);
        }
        let kind = if jumped {
            EdgeKind::Jump
        } else if airborne {
            EdgeKind::Fall
        } else {
            EdgeKind::Walk
        };
        let here = cell_of(st.pos);
        if st.in_water {
            let mut w = here;
            if codes.code(w) & WATER == 0 {
                w[1] += 1;
            }
            if codes.code(w) & WATER == 0 {
                return (Outcome::Nowhere, bbox);
            }
            let rel = [w[0] - c[0], w[1] - c[1], w[2] - c[2]];
            return (Outcome::Rest(vec![(rel, water_kind(codes, w))], kind), bbox);
        }
        if here != c && codes.code(here) & CLIMB != 0 {
            let rel = [here[0] - c[0], here[1] - c[1], here[2] - c[2]];
            let kind = if kind == EdgeKind::Walk {
                EdgeKind::Climb
            } else {
                kind
            };
            return (Outcome::Rest(vec![(rel, RestKind::Holding)], kind), bbox);
        }
        if st.on_ground {
            let moved = [here[0], here[2]] != origin || here[1] != c[1];
            if airborne || moved {
                let cells: Vec<([i32; 3], RestKind)> = landing_cells(codes, &body, st.pos)
                    .into_iter()
                    .filter(|r| *r != c)
                    .map(|r| ([r[0] - c[0], r[1] - c[1], r[2] - c[2]], RestKind::Standing))
                    .collect();
                if !cells.is_empty() || airborne {
                    return (
                        if cells.is_empty() {
                            Outcome::Nowhere
                        } else {
                            Outcome::Rest(cells, kind)
                        },
                        bbox,
                    );
                }
            }
            if st.vel[0].abs() < 1e-4 && st.vel[2].abs() < 1e-4 {
                still += 1;
                if still > 3 {
                    return (Outcome::Nowhere, bbox);
                }
            }
        } else {
            airborne = true;
        }
        if st.pos[1] < f64::from(bottom) {
            return (Outcome::LeftWorld, bbox);
        }
    }
    (Outcome::Nowhere, bbox)
}

// ---------------------------------------------------------------------------
// Templates.
// ---------------------------------------------------------------------------

/// One deduplicated template arc: the cells it read and what the template held
/// there, its outcome, the box it swept, and the launches that share both.
struct TemplateArc {
    read: Vec<([i32; 3], Code)>,
    outcome: Outcome,
    swept: Option<Aabb>,
    members: Vec<Launch>,
}

/// The cells a launch read, with what each held.
type ReadSet = Vec<([i32; 3], Code)>;

/// Every arc of one launch class, flown once over its template.
struct TemplateClass {
    arcs: Vec<TemplateArc>,
}

type ClassKey = (Code, [Code; COLUMN], bool, String);

fn template_class(key: &ClassKey, params: &ReachParams) -> Arc<TemplateClass> {
    static MEMO: OnceLock<Mutex<HashMap<ClassKey, Arc<TemplateClass>>>> = OnceLock::new();
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(t) = memo.lock().expect("memo").get(key) {
        return Arc::clone(t);
    }
    let template = Template {
        support: key.0,
        column: key.1,
    };
    let holding = key.2;
    let launches = shared_launch_set(holding, params);
    // The template's floor is endless, so a body never leaves the world over it.
    let flown: Vec<(ReadSet, Outcome, Option<Aabb>, Launch)> = crate::par::map(&launches, |l| {
        let rec = Recorder {
            inner: &template,
            seen: std::cell::RefCell::new(Vec::new()),
        };
        let (outcome, swept) = fly(
            &rec,
            [0, 0, 0],
            holding,
            l,
            params.unbounded_fall,
            i32::MIN,
            &|_| false,
            None,
        );
        let mut seen = rec.seen.into_inner();
        seen.sort_unstable();
        seen.dedup();
        let read = seen.into_iter().map(|c| (c, template.code(c))).collect();
        (read, outcome, swept, *l)
    });
    let mut index: HashMap<(ReadSet, Outcome), usize> = HashMap::new();
    let mut arcs: Vec<TemplateArc> = Vec::new();
    for (read, outcome, swept, l) in flown {
        match index.get(&(read.clone(), outcome.clone())) {
            Some(&i) => {
                let a = &mut arcs[i];
                a.members.push(l);
                a.swept = union(a.swept, swept);
            }
            None => {
                index.insert((read.clone(), outcome.clone()), arcs.len());
                arcs.push(TemplateArc {
                    read,
                    outcome,
                    swept,
                    members: vec![l],
                });
            }
        }
    }
    let t = Arc::new(TemplateClass { arcs });
    memo.lock()
        .expect("memo")
        .insert(key.clone(), Arc::clone(&t));
    t
}

fn union(a: Option<Aabb>, b: Option<Aabb>) -> Option<Aabb> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Aabb::new(
            a.min[0].min(b.min[0]),
            a.min[1].min(b.min[1]),
            a.min[2].min(b.min[2]),
            a.max[0].max(b.max[0]),
            a.max[1].max(b.max[1]),
            a.max[2].max(b.max[2]),
        )),
        (a, None) => a,
        (None, b) => b,
    }
}

// ---------------------------------------------------------------------------
// The graph.
// ---------------------------------------------------------------------------

/// The edges out of one node as they are gathered: the first launch that
/// reaches a cell by a kind of movement names it.
type Edges = BTreeMap<([i32; 3], EdgeKind), (Option<i16>, bool)>;

/// Everything one node of the graph leads to.
#[derive(Debug, Clone, Default)]
pub(in crate::compiler::nav) struct NodeEdges {
    /// The node's own rest state.
    pub kind: Option<RestKind>,
    /// The edges, ordered and deduplicated.
    pub edges: Vec<Edge>,
    /// Some launch from here leaves the world (spec-0113 §4.7).
    pub leaves_world: bool,
    /// The node is in the open sea (spec-0113 §4.7).
    pub open_sea: bool,
    /// Some launch from here dies of lava, of the fall or of the void — the
    /// first such death in launch order.
    pub dies: Option<Death>,
    /// Launches flown over the real world.
    pub simulated: u64,
    /// Template arcs credited without a tick.
    pub credited: u64,
    /// Every cell the node's launches read or reached, as an inclusive box: a
    /// world that differs from this one in no cell of it gives the node the
    /// same launches ([`GraphMemo::lineage`]).
    pub read: Option<([i32; 3], [i32; 3])>,
}

/// A closure taken over a graph, with the roots it was taken from.
type Closure = (Vec<[i32; 3]>, Arc<Reach>);

/// A node and the quadrants of its launch set that were flown.
type FlownKey = ([i32; 3], Quadrants);

/// The memo of one world's graph: its snapshot and every node expanded so far.
pub(in crate::compiler::nav) struct GraphMemo {
    dense: Dense,
    /// Each node's cell-relation edges (and, for water, its climbs out).
    nodes: Mutex<HashMap<[i32; 3], Arc<NodeEdges>>>,
    /// Each standing or holding node's launches, flown.
    flown: Mutex<HashMap<FlownKey, Arc<NodeEdges>>>,
    /// Per column of the window, the cells a body could rest in, and whether
    /// nothing in it arrests a fall (under a void).
    columns: Columns,
    /// The graph built last over a world like this one — the same window,
    /// ambient and launch set — and every cell the two worlds' snapshots,
    /// killing volumes or furniture differ in. A node of that graph whose
    /// launches read none of those cells is this graph's node too: a quest
    /// configuration that differs from another by a gate re-flies only the
    /// nodes beside the gate (spec-0113 §7).
    lineage: Option<(Arc<GraphMemo>, Vec<[i32; 3]>)>,
    /// The killing volumes' and furniture's cells, for a later graph's diff.
    premise_cells: BTreeSet<[i32; 3]>,
    /// Every closure taken over this graph, with the roots it was taken from.
    closures: Mutex<Vec<Closure>>,
}

impl GraphMemo {
    /// A node of the lineage's graph whose read box this world leaves alone.
    fn inherited(&self, key: &FlownKey) -> Option<Arc<NodeEdges>> {
        let (base, diff) = self.lineage.as_ref()?;
        let e = Arc::clone(base.flown.lock().expect("memo").get(key)?);
        let (lo, hi) = e.read?;
        let lo_i = diff.partition_point(|c| c[0] < lo[0]);
        let clear = diff[lo_i..]
            .iter()
            .take_while(|c| c[0] <= hi[0])
            .all(|c| !(lo[1] <= c[1] && c[1] <= hi[1] && lo[2] <= c[2] && c[2] <= hi[2]));
        clear.then_some(e)
    }
}

/// The envelope index: per column of the window, the heights of every cell a
/// body could possibly rest in (a superset of the rest states: no premise is
/// applied), and whether the column holds nothing at all.
struct Columns {
    rest: Vec<Vec<i32>>,
    empty: Vec<bool>,
    /// Some cell of the window holds slime, which throws a body further than
    /// any launch: the envelope proves nothing and every node is flown.
    slime: bool,
}

impl Columns {
    fn of(d: &Dense) -> Columns {
        let n = (d.dims[0].max(0) * d.dims[2].max(0)) as usize;
        let mut rest = vec![Vec::new(); n];
        let mut empty = vec![true; n];
        let mut slime = false;
        for x in 0..d.dims[0] {
            for z in 0..d.dims[2] {
                let col = (x * d.dims[2] + z) as usize;
                let mut below: Code = match d.outside {
                    Outside::Void => 0,
                    Outside::Ocean { .. } => 16,
                };
                for y in 0..d.dims[1] {
                    let c = [d.lo[0] + x, d.lo[1] + y, d.lo[2] + z];
                    let v = d.code(c);
                    if v & (SHAPE | WATER | LAVA) != 0 {
                        empty[col] = false;
                    }
                    if material_of_code(v) == Material::Slime {
                        slime = true;
                    }
                    let open = v & SHAPE == 0;
                    // A body stands with two cells of room; water and a climb
                    // hold it in one.
                    let head = d.code([c[0], c[1] + 1, c[2]]) & SHAPE == 0;
                    if open && (v & (WATER | CLIMB) != 0 || (head && top_of(below).is_some())) {
                        rest[col].push(c[1]);
                    }
                    // A barrier's top stands a body in the cell over the one
                    // above it as well; a cell under a barrier is a rest cell
                    // for nothing.
                    below = v;
                }
            }
        }
        Columns { rest, empty, slime }
    }
}

/// A world with nothing in it.
struct EmptyCodes;

impl Codes for EmptyCodes {
    fn code(&self, _: [i32; 3]) -> Code {
        0
    }
    fn wet(&self) -> bool {
        false
    }
}

/// How far a launch can carry a body horizontally before its feet fall more
/// than `k` blocks below where they left, for every `k` the window can hold:
/// the furthest any launch of the set flies over an empty world (a sprint jump,
/// a sprint off, a walk off), with half a block of margin. Simulated once.
fn carry_table() -> &'static [f64] {
    static TABLE: OnceLock<Vec<f64>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let world = Sim(&EmptyCodes);
        let body = Attributes::PLAYER;
        let mut table = vec![0.0f64; 1024];
        for strafe in [-1.0, 0.0, 1.0] {
            for sprint in [true, false] {
                for jump in [true, false] {
                    let input = Input {
                        forward: 1,
                        strafe,
                        jump,
                        sprint,
                        sneak: false,
                        yaw: 0.0,
                        magnitude: 1.0,
                    };
                    let mut st = State::standing([0.0; 3], &body);
                    st.vel = saturated(&input);
                    let mut far = 0.0f64;
                    for _ in 0..2000 {
                        sim::tick(&world, &body, Rules::Game, &mut st, &input);
                        far = far.max((st.pos[0] * st.pos[0] + st.pos[2] * st.pos[2]).sqrt());
                        // Until the feet are `k` under the start, the body is no
                        // further than `far`.
                        let k = (-st.pos[1]).floor().max(0.0) as usize;
                        if k >= table.len() {
                            break;
                        }
                        for t in table.iter_mut().skip(k) {
                            *t = t.max(far);
                        }
                    }
                }
            }
        }
        table.iter().map(|v| v + 0.5).collect()
    })
}

/// The widest Chebyshev distance, in columns, a launch from a cell can put a
/// body's widened box over after a fall of at most `k` blocks: the carry, plus
/// the launch position's offset in its cell and the box's half width widened.
fn reach_columns(k: i32) -> i32 {
    let t = carry_table();
    let k = (k.max(0) as usize).min(t.len() - 1);
    (t[k] + 1.0 + Attributes::PLAYER.width / 2.0 + LANDING_WIDENING).floor() as i32
}

impl World {
    /// **This world's content digest** — the sha256 of every cell set the reach
    /// graph reads and every premise it applies, in cell order. Two worlds with
    /// one digest are one graph (spec-0113 §7).
    pub fn fingerprint(&self) -> [u8; 32] {
        *self.fingerprint.get_or_init(|| {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            let mut cells = |tag: &[u8], it: &mut dyn Iterator<Item = [i32; 3]>| {
                h.update(tag);
                for c in it {
                    for v in c {
                        h.update(v.to_le_bytes());
                    }
                }
            };
            cells(b"solid", &mut self.solid.iter());
            cells(b"tall", &mut self.tall.iter());
            cells(b"gates", &mut self.use_gates.iter());
            cells(b"flooded", &mut self.flooded.iter());
            cells(b"lava", &mut self.lava.iter());
            cells(b"climb", &mut self.climb.iter());
            cells(b"furniture", &mut self.furniture.iter());
            h.update(b"partial");
            for (c, v) in self.partial.iter() {
                for x in c {
                    h.update(x.to_le_bytes());
                }
                h.update([*v]);
            }
            h.update(b"material");
            for (c, v) in self.material.iter() {
                for x in c {
                    h.update(x.to_le_bytes());
                }
                h.update(material_code(*v).to_le_bytes());
            }
            h.update(b"lethal");
            for (id, (lo, hi)) in &self.lethal_regions {
                h.update(id.as_bytes());
                for x in lo.iter().chain(hi) {
                    h.update(x.to_le_bytes());
                }
            }
            h.update(b"built");
            for (id, (lo, hi)) in &self.built {
                h.update(id.as_bytes());
                for x in lo.iter().chain(hi) {
                    h.update(x.to_le_bytes());
                }
            }
            h.update(self.ambient.name().as_bytes());
            h.update([u8::from(self.unbounded_fall)]);
            h.finalize().into()
        })
    }

    /// The memo of this world's graph at `params`, shared by every world with
    /// this world's digest.
    pub(in crate::compiler::nav) fn graph_memo(&self, params: &ReachParams) -> Arc<GraphMemo> {
        type Graphs = Mutex<HashMap<([u8; 32], String), Arc<GraphMemo>>>;
        static MEMO: OnceLock<Graphs> = OnceLock::new();
        let key = (self.fingerprint(), params.key());
        let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(g) = memo.lock().expect("memo").get(&key) {
            return Arc::clone(g);
        }
        let dense = Dense::of(self);
        let premise_cells: BTreeSet<[i32; 3]> =
            self.lethal.iter().chain(self.furniture.iter()).collect();
        // The lineage: the first graph over a world with this window, ambient
        // and launch set.
        static SEEDS: OnceLock<Mutex<HashMap<String, Arc<GraphMemo>>>> = OnceLock::new();
        let seed_key = format!(
            "{}:{}:{:?}:{:?}:{}",
            params.key(),
            self.ambient.name(),
            dense.lo,
            dense.dims,
            self.unbounded_fall
        );
        let seeds = SEEDS.get_or_init(|| Mutex::new(HashMap::new()));
        let seed = seeds.lock().expect("memo").get(&seed_key).cloned();
        let lineage = seed.map(|base| {
            let mut diff: Vec<[i32; 3]> = Vec::new();
            for (i, (a, b)) in dense.cells.iter().zip(&base.dense.cells).enumerate() {
                if a != b {
                    let (dy, dz) = (dense.dims[1] as usize, dense.dims[2] as usize);
                    let y = (i % dy) as i32;
                    let z = ((i / dy) % dz) as i32;
                    let x = (i / dy / dz) as i32;
                    diff.push([dense.lo[0] + x, dense.lo[1] + y, dense.lo[2] + z]);
                }
            }
            diff.extend(
                premise_cells
                    .symmetric_difference(&base.premise_cells)
                    .copied(),
            );
            diff.sort_unstable();
            diff.dedup();
            (base, diff)
        });
        let g = Arc::new(GraphMemo {
            columns: Columns::of(&dense),
            dense,
            nodes: Mutex::new(HashMap::new()),
            flown: Mutex::new(HashMap::new()),
            lineage,
            premise_cells,
            closures: Mutex::new(Vec::new()),
        });
        // The latest graph is the next one's lineage: configurations are asked
        // about in story order, and the one before differs least.
        seeds.lock().expect("memo").insert(seed_key, Arc::clone(&g));
        memo.lock()
            .expect("memo")
            .entry(key)
            .or_insert_with(|| Arc::clone(&g))
            .clone()
    }

    /// The rest state `c` holds a body in, if any (spec-0113 §4.1).
    pub fn rest_kind(&self, c: [i32; 3]) -> Option<RestKind> {
        let fp = Footprint::player();
        if self.is_water(c) || self.ambient_water(c) {
            let up = [c[0], c[1] + 1, c[2]];
            return Some(
                if self.is_water(up) || self.ambient_water(up) || self.is_occupied(up) {
                    RestKind::Submerged
                } else {
                    RestKind::Afloat
                },
            );
        }
        if self.standable_fp(c, &fp) || self.perch_feet_16_player(c) {
            return Some(RestKind::Standing);
        }
        if self.climb_cell_fp(c, &fp) {
            return Some(RestKind::Holding);
        }
        None
    }

    /// Whether a body in the water at `c` stands: the water is one cell deep
    /// over a floor, with air over it — the shallows a body wades.
    pub fn wades(&self, c: [i32; 3]) -> bool {
        let up = [c[0], c[1] + 1, c[2]];
        (self.is_water(c) || self.ambient_water(c))
            && self.is_solid([c[0], c[1] - 1, c[2]])
            && !(self.is_water(up) || self.ambient_water(up) || self.is_occupied(up))
    }

    /// Whether a body in `c` stands on a fence, wall or gate top — a standing
    /// rest state the walk relation never stands a body in (spec-0113 §4.1).
    pub fn is_perched(&self, c: [i32; 3]) -> bool {
        !self.standable_fp(c, &Footprint::player()) && self.perch_feet_16_player(c)
    }

    fn perch_feet_16_player(&self, c: [i32; 3]) -> bool {
        let fp = Footprint::player();
        let support = [c[0], c[1] - 1, c[2]];
        (self.tall.contains(&support) || self.use_gates.contains(&support))
            && (0..=2).all(|dy| !self.is_occupied([c[0], c[1] + dy, c[2]]))
            && !self.meets_lethal_fp(c, &fp)
    }

    /// Whether a body's box at `st` meets a killing volume of this world.
    fn kills(&self, st: &State) -> bool {
        if self.lethal_regions.is_empty() {
            return false;
        }
        let body =
            delvewright_dsl::metrics::Body::new(delvewright_dsl::metrics::PLAYER_WIDTH, st.height);
        self.lethal_regions.iter().any(|(_, (lo, hi))| {
            delvewright_dsl::metrics::body_meets_volume(st.pos, body, *lo, *hi)
        })
    }

    /// Whether a killing volume reaches into the box `b`.
    fn lethal_near(&self, b: &Aabb) -> bool {
        self.lethal_regions.iter().any(|(_, (lo, hi))| {
            (0..3).all(|i| {
                f64::from(lo[i]) <= b.max[i] + 1.0 && f64::from(hi[i]) + 1.0 >= b.min[i] - 1.0
            })
        })
    }

    /// Every edge out of `c` (spec-0113 §4.3), computed over `memo`'s snapshot.
    fn expand(
        &self,
        memo: &GraphMemo,
        params: &ReachParams,
        c: [i32; 3],
        wanted: Quadrants,
    ) -> NodeEdges {
        let fly_launches = wanted != 0;
        let fp = Footprint::player();
        let mut out = NodeEdges {
            kind: self.rest_kind(c),
            ..NodeEdges::default()
        };
        let Some(kind) = out.kind else {
            return out;
        };
        let dense = &memo.dense;
        let mut edges: Edges = BTreeMap::new();
        let accept = |to: [i32; 3], want: RestKind| -> bool {
            match self.rest_kind(to) {
                Some(k) => k == want || (want == RestKind::Standing && k == RestKind::Holding),
                None => false,
            }
        };
        match kind {
            RestKind::Afloat | RestKind::Submerged => {
                if matches!(self.ambient, Ambient::Ocean(_)) && dense.on_rim(c) {
                    out.open_sea = true;
                }
                for d in [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ] {
                    let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                    if dense.inside(n)
                        && matches!(
                            self.rest_kind(n),
                            Some(RestKind::Afloat | RestKind::Submerged)
                        )
                    {
                        edges.entry((n, EdgeKind::Dive)).or_insert((None, false));
                    }
                }
                // Out of the water: pressed into whatever is beside, the jump
                // held, from low in the cell and from its top.
                let beside =
                    (-1..=1)
                        .flat_map(|dx| (-1..=1).map(move |dz| [c[0] + dx, c[1], c[2] + dz]))
                        .chain((-1..=1).flat_map(|dx| {
                            (-1..=1).map(move |dz| [c[0] + dx, c[1] + 1, c[2] + dz])
                        }))
                        .any(|n| top_of(dense.code(n)).is_some());
                if beside {
                    self.climb_out(memo, params, c, &mut edges, &mut out);
                }
            }
            RestKind::Standing | RestKind::Holding if !fly_launches => {
                // The cell relation: the walk and the climb, a wade in and a
                // straight drop into the next column — moves the launches below
                // make too, taken first because they are cheap.
                let walks = self.neighbors_fp(c, &fp);
                for n in self.moves_of(c, &fp) {
                    if self.rest_kind(n).is_some() {
                        let kind = if self.climb_cell_fp(n, &fp) || self.climb_cell_fp(c, &fp) {
                            EdgeKind::Climb
                        } else if walks.contains(&n) {
                            EdgeKind::Walk
                        } else {
                            EdgeKind::Fall
                        };
                        edges.entry((n, kind)).or_insert((None, false));
                    }
                }
                if kind == RestKind::Holding {
                    for n in self.climb_moves_fp(c, &fp) {
                        edges.entry((n, EdgeKind::Climb)).or_insert((None, false));
                    }
                }
            }
            RestKind::Standing | RestKind::Holding => {
                for n in self.neighbors_fp(c, &fp) {
                    if self.rest_kind(n).is_some() {
                        let kind = if self.climb_cell_fp(n, &fp) || self.climb_cell_fp(c, &fp) {
                            EdgeKind::Climb
                        } else {
                            EdgeKind::Walk
                        };
                        edges.entry((n, kind)).or_insert((None, false));
                    }
                }
                if kind == RestKind::Holding {
                    for n in self.climb_moves_fp(c, &fp) {
                        edges.entry((n, EdgeKind::Climb)).or_insert((None, false));
                    }
                }
                let holding = kind == RestKind::Holding;
                let support = dense.code([c[0], c[1] - 1, c[2]]);
                let mut column = [0; COLUMN];
                for (i, v) in column.iter_mut().enumerate() {
                    *v = dense.code([c[0], c[1] + i as i32, c[2]]);
                }
                let class = template_class(&(support, column, holding, params.key()), params);
                let shifted = Shifted { dense, at: c };
                let killed = |st: &State| self.kills(st);
                let edge_memo: EdgeMemo = std::cell::RefCell::new(HashMap::new());
                // The box the launches read: the cell's own neighbourhood, and
                // every swept box grown by the cells a tick and a landing read
                // around it.
                let mut read = (
                    [c[0] - 2, c[1] - 2, c[2] - 2],
                    [c[0] + 2, c[1] + COLUMN as i32 + 1, c[2] + 2],
                );
                let mut grow = |b: &Aabb| {
                    let lo = [
                        b.min[0].floor() as i32 - 2,
                        b.min[1].floor() as i32 - 3,
                        b.min[2].floor() as i32 - 2,
                    ];
                    let hi = [
                        b.max[0].floor() as i32 + 2,
                        b.max[1].floor() as i32 + 3,
                        b.max[2].floor() as i32 + 2,
                    ];
                    for i in 0..3 {
                        read.0[i] = read.0[i].min(lo[i]);
                        read.1[i] = read.1[i].max(hi[i]);
                    }
                };
                for arc in &class.arcs {
                    if !arc.members.iter().any(|l| l.serves(wanted)) {
                        continue;
                    }
                    let matches = arc.read.iter().all(|(r, code)| shifted.code(*r) == *code)
                        && !arc.swept.is_some_and(|b| {
                            self.lethal_near(&b.offset([
                                f64::from(c[0]),
                                f64::from(c[1]),
                                f64::from(c[2]),
                            ]))
                        });
                    if let Some(b) = arc.swept {
                        grow(&b.offset([f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]));
                    }
                    if matches {
                        out.credited += 1;
                        let l = arc.members[0];
                        self.record(c, &arc.outcome, &l, &accept, &mut edges, &mut out);
                        continue;
                    }
                    for l in arc.members.iter().filter(|l| l.serves(wanted)) {
                        out.simulated += 1;
                        let (o, swept) = fly(
                            &shifted,
                            [0, 0, 0],
                            holding,
                            l,
                            params.unbounded_fall,
                            dense.bottom() - c[1],
                            &|st: &State| {
                                let mut s = *st;
                                s.pos = [
                                    st.pos[0] + f64::from(c[0]),
                                    st.pos[1] + f64::from(c[1]),
                                    st.pos[2] + f64::from(c[2]),
                                ];
                                killed(&s)
                            },
                            Some(&edge_memo),
                        );
                        if let Some(b) = swept {
                            grow(&b.offset([f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]));
                        }
                        self.record(c, &o, l, &accept, &mut edges, &mut out);
                    }
                }
                out.read = Some(read);
            }
        }
        out.edges = edges
            .into_iter()
            .map(|((to, kind), (yaw, sprint))| Edge {
                to,
                kind,
                yaw,
                sprint,
            })
            .collect();
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &self,
        c: [i32; 3],
        outcome: &Outcome,
        l: &Launch,
        accept: &dyn Fn([i32; 3], RestKind) -> bool,
        edges: &mut Edges,
        out: &mut NodeEdges,
    ) {
        match outcome {
            Outcome::LeftWorld => {
                out.leaves_world = true;
                out.dies.get_or_insert(Death::Void);
            }
            Outcome::Rest(cells, kind) => {
                for (rel, rest) in cells {
                    let to = [c[0] + rel[0], c[1] + rel[1], c[2] + rel[2]];
                    if to != c && accept(to, *rest) {
                        edges.entry((to, *kind)).or_insert((
                            Some(l.yaw.round() as i16),
                            !matches!(l.mode, Mode::WalkOff | Mode::Creep),
                        ));
                    }
                }
            }
            Outcome::Dead(d @ (Death::Lava | Death::Fall | Death::Void)) => {
                out.dies.get_or_insert(*d);
            }
            Outcome::Nowhere | Outcome::Dead(Death::Volume) => {}
        }
    }

    /// The simulated ways out of the water at `c`: facing every sixteenth of a
    /// turn, forward with the jump held, from the cell's foot and its top.
    fn climb_out(
        &self,
        memo: &GraphMemo,
        params: &ReachParams,
        c: [i32; 3],
        edges: &mut Edges,
        out: &mut NodeEdges,
    ) {
        let body = Attributes::PLAYER;
        let world = Sim(&memo.dense);
        for k in 0..16 {
            let yaw = f64::from(k) * 22.5 - 180.0;
            for lift in [0.05, 0.6] {
                out.simulated += 1;
                let mut st = State::standing(
                    [
                        f64::from(c[0]) + 0.5,
                        f64::from(c[1]) + lift,
                        f64::from(c[2]) + 0.5,
                    ],
                    &body,
                );
                st.on_ground = false;
                let mut input = Input::forward(yaw);
                input.jump = true;
                let mut left_water = false;
                for _ in 0..60 {
                    sim::tick(&world, &body, Rules::Game, &mut st, &input);
                    if self.kills(&st) || st.in_lava {
                        break;
                    }
                    if !params.unbounded_fall && st.landing_damage >= body.max_health {
                        break;
                    }
                    if !st.in_water {
                        left_water = true;
                    }
                    if left_water && st.on_ground && !st.in_water {
                        for to in landing_cells(&memo.dense, &body, st.pos) {
                            if self.rest_kind(to) == Some(RestKind::Standing) {
                                edges
                                    .entry((to, EdgeKind::Climb))
                                    .or_insert((Some(yaw.round() as i16), false));
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    /// **Where a body can be** from `roots`: the closure of the reach graph
    /// (spec-0113 §4), built at the shipped launch set.
    pub fn body_reach(&self, roots: &[[i32; 3]]) -> Reach {
        self.body_reach_at(roots, &ReachParams::shipped(self.unbounded_fall))
    }

    /// **Every edge of the reach graph out of `c`**: its cell relation and
    /// every launch of the launch set, flown — where one movement takes a body
    /// standing, swimming or holding in `c`. Empty when `c` is no rest state.
    pub fn body_edges(&self, c: [i32; 3]) -> Vec<Edge> {
        let params = ReachParams::shipped(self.unbounded_fall);
        let memo = self.graph_memo(&params);
        let mut out = self.expand(&memo, &params, c, 0).edges;
        if matches!(
            self.rest_kind(c),
            Some(RestKind::Standing | RestKind::Holding)
        ) {
            out.extend(self.expand(&memo, &params, c, ALL_QUADRANTS).edges);
        }
        out.sort();
        out.dedup_by(|a, b| a.to == b.to && a.kind == b.kind);
        out
    }

    /// **Fly a declared jump's window** (spec-0113 §4.4) from `from` toward
    /// `to` under `rules`: facing every whole degree within
    /// [`delvewright_dsl::metrics::DECLARED_JUMP_FACING_DEG`] of the line
    /// between the two cells, strafe `0`, the forward key (the move vector at
    /// [`delvewright_dsl::metrics::DECLARED_JUMP_INPUT`]), sprinting when
    /// `sprint`, from rest at the back of a straight run-up of `run_up` cells
    /// behind `from` along the jump's axis, the jump pressed on each of the
    /// last [`delvewright_dsl::metrics::DECLARED_JUMP_PRESS_TICKS`] supported
    /// ticks before the edge. Every launch must touch down on `to`'s support
    /// with the exact box over it by at least
    /// [`delvewright_dsl::metrics::DECLARED_JUMP_OVERLAP`] on both axes.
    /// `Ok(())` when all land; the first that does not, otherwise.
    pub fn declared_jump(
        &self,
        from: [i32; 3],
        to: [i32; 3],
        sprint: bool,
        run_up: u32,
        rules: Rules,
    ) -> Result<(), JumpMiss> {
        use delvewright_dsl::metrics as m;
        let params = ReachParams::shipped(self.unbounded_fall);
        let memo = self.graph_memo(&params);
        let world = Sim(&memo.dense);
        let body = Attributes::PLAYER;
        let axis = jump_axis(from, to);
        let back = [
            from[0] - axis[0] * run_up as i32,
            from[1],
            from[2] - axis[1] * run_up as i32,
        ];
        let Some(feet) = feet_of(&memo.dense, back) else {
            return Err(JumpMiss {
                facing: 0.0,
                press: 0,
                ended: [
                    f64::from(back[0]) + 0.5,
                    f64::from(back[1]),
                    f64::from(back[2]) + 0.5,
                ],
                rules,
            });
        };
        let Some(land_feet) = feet_of(&memo.dense, to) else {
            return Err(JumpMiss {
                facing: 0.0,
                press: 0,
                ended: [
                    f64::from(to[0]) + 0.5,
                    f64::from(to[1]),
                    f64::from(to[2]) + 0.5,
                ],
                rules,
            });
        };
        // The landing support: `to`'s own floor and the floor round it at the
        // same height the walk joins to it, out to
        // [`delvewright_dsl::metrics::DECLARED_JUMP_LANDING_REACH`] cells — the
        // far side a body that made the jump stands on, whatever cell of it a
        // facing off the line or a long run-up puts it in.
        let fp = Footprint::player();
        let reach = m::DECLARED_JUMP_LANDING_REACH as i32;
        let mut landing: Vec<[i32; 3]> = vec![to];
        let mut i = 0;
        while i < landing.len() {
            let c = landing[i];
            i += 1;
            for n in self.neighbors_walk_fp(c, &fp) {
                if !landing.contains(&n)
                    && (n[0] - to[0]).abs() <= reach
                    && (n[2] - to[2]).abs() <= reach
                    && feet_of(&memo.dense, n).is_some_and(|f| (f - land_feet).abs() < 1e-6)
                {
                    landing.push(n);
                }
            }
        }
        let (dx, dz) = (f64::from(to[0] - from[0]), f64::from(to[2] - from[2]));
        let yaw0 = (-dx).atan2(dz).to_degrees();
        let start = [f64::from(back[0]) + 0.5, feet, f64::from(back[2]) + 0.5];
        // A body at rest on a floor carries the one tick of gravity its last
        // collision left (`(0 − g)·0.98`), and stays on the ground for it.
        let rest = |p: [f64; 3]| {
            let mut st = State::standing(p, &body);
            st.vel[1] = -body.gravity * f64::from(0.98f32);
            st
        };
        let window = m::DECLARED_JUMP_FACING_DEG as i32;
        for k in -window..=window {
            let yaw = yaw0 + f64::from(k);
            let mut input = Input::forward(yaw);
            input.sprint = sprint;
            // The run without a jump: the last ticks that still start on the
            // ground with the box over the launch cell's column are the window
            // a press may come on.
            let mut st = rest(start);
            let mut supported: Vec<usize> = Vec::new();
            for t in 0..(run_up as usize + 4) * 12 {
                let over = st.on_ground && box_over_column(&st, &body, from);
                if over {
                    supported.push(t);
                }
                sim::tick(&world, &body, rules, &mut st, &input);
                if !st.on_ground && !supported.is_empty() {
                    break;
                }
            }
            let presses: Vec<usize> = supported
                .iter()
                .rev()
                .take(m::DECLARED_JUMP_PRESS_TICKS as usize)
                .copied()
                .collect();
            if presses.is_empty() {
                return Err(JumpMiss {
                    facing: yaw,
                    press: 0,
                    ended: st.pos,
                    rules,
                });
            }
            for p in presses {
                let mut st = rest(start);
                let mut airborne = false;
                let mut landed = None;
                for t in 0..400 {
                    let mut i = input;
                    i.jump = t >= p;
                    sim::tick(&world, &body, rules, &mut st, &i);
                    if self.kills(&st) || st.in_lava || st.in_water {
                        break;
                    }
                    if !self.unbounded_fall && st.landing_damage >= body.max_health {
                        break;
                    }
                    if !st.on_ground {
                        airborne = true;
                    } else if airborne {
                        landed = Some(st);
                        break;
                    }
                }
                let ok = landed.is_some_and(|l| {
                    let bb = l.aabb(&body);
                    let over = |lo: f64, hi: f64, c: i32| {
                        hi.min(f64::from(c) + 1.0) - lo.max(f64::from(c))
                    };
                    (l.pos[1] - land_feet).abs() < 1e-6
                        && landing.iter().any(|c| {
                            over(bb.min[0], bb.max[0], c[0]) >= m::DECLARED_JUMP_OVERLAP
                                && over(bb.min[2], bb.max[2], c[2]) >= m::DECLARED_JUMP_OVERLAP
                        })
                });
                if !ok {
                    return Err(JumpMiss {
                        facing: yaw,
                        press: p,
                        ended: landed.map_or(st.pos, |l| l.pos),
                        rules,
                    });
                }
            }
        }
        Ok(())
    }

    /// The straight run-up behind `from` along the jump toward `to`: how many
    /// cells, up to [`delvewright_dsl::metrics::DECLARED_JUMP_RUN_UP_CAP`], a body
    /// walks in a line into `from` — each a standing state the walk relation
    /// joins to the next.
    pub fn run_up(&self, from: [i32; 3], to: [i32; 3]) -> u32 {
        let fp = Footprint::player();
        let axis = jump_axis(from, to);
        let mut n = 0u32;
        let mut cur = from;
        while n < delvewright_dsl::metrics::DECLARED_JUMP_RUN_UP_CAP {
            let prev = [cur[0] - axis[0], cur[1], cur[2] - axis[1]];
            if !self.standable_fp(prev, &fp) || !self.neighbors_walk_fp(prev, &fp).contains(&cur) {
                break;
            }
            n += 1;
            cur = prev;
        }
        n
    }

    /// The cells [`World::body_edges`] leads to from `c`.
    pub fn body_edge_cells(&self, c: [i32; 3]) -> Vec<[i32; 3]> {
        let mut out: Vec<[i32; 3]> = self.body_edges(c).iter().map(|e| e.to).collect();
        out.dedup();
        out
    }

    /// How a body standing or holding in `c` dies by one of its launches, if
    /// any does — of lava, of a fall it does not survive or out of the world (a
    /// killing volume is the keep-out's to say).
    pub fn fatal_launch(&self, c: [i32; 3]) -> Option<Death> {
        let params = ReachParams::shipped(self.unbounded_fall);
        let memo = self.graph_memo(&params);
        self.expand(&memo, &params, c, ALL_QUADRANTS).dies
    }

    /// [`World::body_reach`] from `starts` each snapped to the nearest
    /// standable cell first, as [`World::walk_region`] snaps them; a start
    /// with none within [`crate::compiler::nav::SNAP_RADIUS`] contributes nothing.
    pub fn body_reach_snapped(&self, starts: &[[i32; 3]]) -> Reach {
        let seats: Vec<[i32; 3]> = starts
            .iter()
            .filter_map(|s| self.snap(*s, crate::compiler::nav::SNAP_RADIUS))
            .collect();
        self.body_reach(&seats)
    }

    /// **Every cell a body can stand in** from `starts` (snapped as
    /// [`World::body_reach_snapped`] snaps them): the standing states of the
    /// reach graph — the region every proof that asks "can a player be here"
    /// reads (spec-0113 §5.1).
    pub fn body_region(&self, starts: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
        self.body_reach_snapped(starts).standing()
    }

    /// [`World::body_region`] from roots seated in the AABB that declares each
    /// ([`crate::compiler::nav::AnchorRoot`]).
    pub fn body_region_rooted(
        &self,
        roots: &[crate::compiler::nav::AnchorRoot],
    ) -> BTreeSet<[i32; 3]> {
        self.body_reach_rooted(roots).standing()
    }

    /// [`World::body_reach`] at `params`.
    ///
    /// The closure is taken in rounds. Each round first closes over the cheap
    /// edges — the cell relation of every standing and holding node, and every
    /// water node's swims and climbs out — and then flies the launch set from
    /// every standing or holding node whose **envelope** still holds a cell the
    /// closure has not reached: a cell a body could rest in, inside the columns
    /// a launch can carry it over for the fall it would take, or (under a void)
    /// a column with nothing in it. A node whose envelope holds neither cannot
    /// add a state by any launch, so its launches are not flown; the closure is
    /// the one every node's launches would give. Rounds end when no node is
    /// left to fly.
    pub fn body_reach_at(&self, roots: &[[i32; 3]], params: &ReachParams) -> Reach {
        let memo = self.graph_memo(params);
        let start: Vec<[i32; 3]> = roots
            .iter()
            .copied()
            .filter(|r| self.rest_kind(*r).is_some())
            .collect();
        // A closure this graph already took is this one when every root lies
        // in it and, over its own edges, the roots reach every root it was
        // taken from: then each closure holds the other.
        let done: Vec<Closure> = memo.closures.lock().expect("memo").clone();
        for (from, c) in &done {
            if start.iter().all(|r| c.states.contains_key(r)) && c.reaches_all(&start, from) {
                let mut r = (**c).clone();
                if r.simulated + r.credited > params.budget {
                    r.over_budget = Some(params.budget);
                }
                return r;
            }
        }
        let mut reach = Reach::default();
        self.close(&memo, params, &mut reach, start.clone());
        loop {
            let todo: Vec<([i32; 3], Quadrants)> = reach
                .states
                .iter()
                .filter(|(_, k)| matches!(k, RestKind::Standing | RestKind::Holding))
                .filter_map(|(c, _)| {
                    let q = self.envelope_open(&memo, &reach, *c) & !reach.flown_mask(*c);
                    (q != 0).then_some((*c, q))
                })
                .collect();
            if todo.is_empty() {
                break;
            }
            for chunk in todo.chunks(2048) {
                if reach.simulated + reach.credited > params.budget {
                    break;
                }
                let chunk: Vec<([i32; 3], Quadrants)> = chunk
                    .iter()
                    .filter_map(|(c, _)| {
                        let q = self.envelope_open(&memo, &reach, *c) & !reach.flown_mask(*c);
                        (q != 0).then_some((*c, q))
                    })
                    .collect();
                self.fly_into(&memo, params, &mut reach, &chunk);
            }
            // The budget is refused, never truncated (`DW1042`): a closure that
            // passed it stops here and says so, and every proof that reads it
            // refuses the build.
            if reach.simulated + reach.credited > params.budget {
                reach.over_budget = Some(params.budget);
                return reach;
            }
        }
        memo.closures
            .lock()
            .expect("memo")
            .push((start, Arc::new(reach.clone())));
        reach
    }

    /// Fly every launch of `cells` — standing or holding states of `reach` —
    /// and close over what they reach. The caller that needs a node's every
    /// edge (a pocket's way out, `DW0921`) asks this of the nodes it judges.
    pub fn fly_cells(&self, reach: &mut Reach, cells: &[[i32; 3]]) {
        let params = ReachParams::shipped(self.unbounded_fall);
        let memo = self.graph_memo(&params);
        let todo: Vec<([i32; 3], Quadrants)> = cells
            .iter()
            .copied()
            .filter(|c| {
                reach.flown_mask(*c) != ALL_QUADRANTS
                    && matches!(
                        reach.states.get(c),
                        Some(RestKind::Standing | RestKind::Holding)
                    )
            })
            .map(|c| (c, ALL_QUADRANTS))
            .collect();
        self.fly_into(&memo, &params, reach, &todo);
    }

    fn fly_into(
        &self,
        memo: &GraphMemo,
        params: &ReachParams,
        reach: &mut Reach,
        cells: &[([i32; 3], Quadrants)],
    ) {
        let flown: Vec<Arc<NodeEdges>> = crate::par::map(cells, |(c, q)| {
            let key = (*c, *q);
            if let Some(e) = memo.flown.lock().expect("memo").get(&key) {
                return Arc::clone(e);
            }
            let e = memo
                .inherited(&key)
                .unwrap_or_else(|| Arc::new(self.expand(memo, params, *c, *q)));
            memo.flown
                .lock()
                .expect("memo")
                .entry(key)
                .or_insert_with(|| Arc::clone(&e))
                .clone()
        });
        let mut new: Vec<[i32; 3]> = Vec::new();
        for ((c, q), e) in cells.iter().zip(flown) {
            *reach.flown.entry(*c).or_insert(0) |= *q;
            reach.simulated += e.simulated;
            reach.credited += e.credited;
            if e.leaves_world {
                reach.leaves_world.insert(*c);
            }
            if let Some(d) = e.dies {
                reach.fatal.insert(*c, d);
            }
            let have: BTreeSet<([i32; 3], EdgeKind)> = reach
                .succ
                .get(c)
                .into_iter()
                .flatten()
                .map(|x| (x.to, x.kind))
                .collect();
            for edge in &e.edges {
                if have.contains(&(edge.to, edge.kind)) {
                    continue;
                }
                reach
                    .preds
                    .entry(edge.to)
                    .or_default()
                    .push((*c, edge.clone()));
                reach.succ.entry(*c).or_default().push(edge.clone());
                if !reach.states.contains_key(&edge.to) {
                    new.push(edge.to);
                }
            }
        }
        new.sort_unstable();
        new.dedup();
        self.close(memo, params, reach, new);
    }

    /// Close `reach` over the cheap edges from `frontier`.
    fn close(
        &self,
        memo: &GraphMemo,
        params: &ReachParams,
        reach: &mut Reach,
        frontier: Vec<[i32; 3]>,
    ) {
        let mut frontier: Vec<[i32; 3]> = frontier
            .into_iter()
            .filter(|c| !reach.states.contains_key(c))
            .collect();
        frontier.sort_unstable();
        frontier.dedup();
        let mut queued: BTreeSet<[i32; 3]> = frontier.iter().copied().collect();
        while !frontier.is_empty() {
            // The nodes the memo holds are read under one lock; the rest are
            // expanded across the cores only when there are enough of them to
            // pay for the threads.
            let mut expanded: Vec<Option<Arc<NodeEdges>>> = {
                let held = memo.nodes.lock().expect("memo");
                frontier.iter().map(|c| held.get(c).cloned()).collect()
            };
            let missing: Vec<[i32; 3]> = frontier
                .iter()
                .zip(&expanded)
                .filter(|(_, e)| e.is_none())
                .map(|(c, _)| *c)
                .collect();
            if !missing.is_empty() {
                let fresh: Vec<Arc<NodeEdges>> = if missing.len() >= 64 {
                    crate::par::map(&missing, |c| Arc::new(self.expand(memo, params, *c, 0)))
                } else {
                    missing
                        .iter()
                        .map(|c| Arc::new(self.expand(memo, params, *c, 0)))
                        .collect()
                };
                let mut held = memo.nodes.lock().expect("memo");
                let mut fresh = missing.iter().zip(fresh);
                for (c, slot) in frontier.iter().zip(expanded.iter_mut()) {
                    if slot.is_none() {
                        let (m, e) = fresh.next().expect("one per missing node");
                        debug_assert_eq!(m, c);
                        *slot = Some(Arc::clone(held.entry(*c).or_insert(e)));
                    }
                }
            }
            let expanded: Vec<Arc<NodeEdges>> =
                expanded.into_iter().map(|e| e.expect("expanded")).collect();
            let mut next: Vec<[i32; 3]> = Vec::new();
            for (c, e) in frontier.iter().zip(expanded) {
                reach.simulated += e.simulated;
                reach.credited += e.credited;
                let Some(k) = e.kind else {
                    continue;
                };
                reach.states.insert(*c, k);
                reach.mark(&memo.dense, *c);
                if e.leaves_world {
                    reach.leaves_world.insert(*c);
                }
                if e.open_sea {
                    reach.open_sea.insert(*c);
                }
                for edge in &e.edges {
                    reach
                        .preds
                        .entry(edge.to)
                        .or_default()
                        .push((*c, edge.clone()));
                    if !reach.states.contains_key(&edge.to) && queued.insert(edge.to) {
                        next.push(edge.to);
                    }
                }
                reach
                    .succ
                    .entry(*c)
                    .or_default()
                    .extend(e.edges.iter().cloned());
            }
            next.sort_unstable();
            frontier = next;
        }
    }

    /// Which launches from `c` could still add to `reach` ([`Quadrants`]):
    /// those that can carry a body toward a cell of its envelope a body could
    /// rest in and the closure has not reached, or (under a void) toward a
    /// column with nothing in it. `0` when there is none.
    fn envelope_open(&self, memo: &GraphMemo, reach: &Reach, c: [i32; 3]) -> Quadrants {
        let d = &memo.dense;
        if memo.columns.slime || d.dims[0] == 0 {
            return ALL_QUADRANTS;
        }
        let void = matches!(d.outside, Outside::Void);
        let deepest = c[1] - d.bottom() + 1;
        let r = reach_columns(deepest);
        let need = |dx: i32, dz: i32| -> Quadrants {
            let s = |v: i32| {
                if v >= 2 {
                    1
                } else if v <= -2 {
                    -1
                } else {
                    0
                }
            };
            quadrant_bit(s(dx), s(dz))
        };
        let mut q: Quadrants = 0;
        for dx in -r..=r {
            for dz in -r..=r {
                let bit = need(dx, dz);
                if q & bit != 0 {
                    continue;
                }
                let x = c[0] + dx - d.lo[0];
                let z = c[2] + dz - d.lo[2];
                let cheb = dx.abs().max(dz.abs());
                if x < 0 || z < 0 || x >= d.dims[0] || z >= d.dims[2] {
                    // Outside the window: the void, or the open sea a water
                    // node on the rim already stands for.
                    if void {
                        q |= bit;
                    }
                    continue;
                }
                let col = (x * d.dims[2] + z) as usize;
                if void && memo.columns.empty[col] {
                    q |= bit;
                    continue;
                }
                for &y in &memo.columns.rest[col] {
                    if y > c[1] + 2 {
                        break;
                    }
                    if cheb <= reach_columns(c[1] - y + 1)
                        && !reach.has(d, [c[0] + dx, y, c[2] + dz])
                    {
                        q |= bit;
                        break;
                    }
                }
            }
        }
        q
    }
}

/// The cardinal axis a jump from `from` to `to` runs along: the larger of its
/// two horizontal components, as a unit `[dx, dz]`.
fn jump_axis(from: [i32; 3], to: [i32; 3]) -> [i32; 2] {
    let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
    if dx.abs() >= dz.abs() {
        [dx.signum(), 0]
    } else {
        [0, dz.signum()]
    }
}

/// Whether the body's box at `st` lies over `cell`'s column at all.
fn box_over_column(st: &State, body: &Attributes, cell: [i32; 3]) -> bool {
    let bb = st.aabb(body);
    bb.max[0] > f64::from(cell[0])
        && bb.min[0] < f64::from(cell[0]) + 1.0
        && bb.max[2] > f64::from(cell[2])
        && bb.min[2] < f64::from(cell[2]) + 1.0
}

/// A declared jump's launch that misses (spec-0113 §5.2): its facing, the tick
/// its jump was pressed on, where the body ended, and the rules it flew under.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpMiss {
    /// The facing, the game's degrees.
    pub facing: f64,
    /// The tick of the run the jump was pressed on.
    pub press: usize,
    /// Where the body's feet ended.
    pub ended: [f64; 3],
    /// The rules the launch flew under.
    pub rules: Rules,
}

/// The closure of the reach graph from a set of roots.
#[derive(Debug, Clone, Default)]
pub struct Reach {
    /// Every rest state reached, roots included.
    pub states: BTreeMap<[i32; 3], RestKind>,
    /// Every reached state's edges.
    pub succ: BTreeMap<[i32; 3], Vec<Edge>>,
    /// Every reached state's predecessors, with the edge that joins them.
    pub preds: BTreeMap<[i32; 3], Vec<([i32; 3], Edge)>>,
    /// The states some launch from which leaves the world.
    pub leaves_world: BTreeSet<[i32; 3]>,
    /// The states in the open sea.
    pub open_sea: BTreeSet<[i32; 3]>,
    /// The flown states some launch from which dies of lava, of the fall or of
    /// the void.
    pub fatal: BTreeMap<[i32; 3], Death>,
    /// Launches flown over the real world, summed over the states expanded.
    pub simulated: u64,
    /// Template arcs credited, summed likewise.
    pub credited: u64,
    /// The standing and holding states whose launches have been flown, with
    /// the [`Quadrants`] flown.
    flown: BTreeMap<[i32; 3], Quadrants>,
    /// Set to the budget when the closure's work passed it (`DW1042`): the
    /// closure is then partial, and no proof may take a verdict over it.
    pub over_budget: Option<u64>,
    /// The reached states inside the window, as a bitmap over it.
    bits: Vec<u64>,
}

impl Reach {
    /// Whether, over this closure's own edges, `from` reaches every one of
    /// `targets`.
    fn reaches_all(&self, from: &[[i32; 3]], targets: &[[i32; 3]]) -> bool {
        let mut want: BTreeSet<[i32; 3]> = targets.iter().copied().collect();
        let mut seen: BTreeSet<[i32; 3]> = from.iter().copied().collect();
        let mut queue: Vec<[i32; 3]> = from.to_vec();
        for f in from {
            want.remove(f);
        }
        while let Some(c) = queue.pop() {
            if want.is_empty() {
                return true;
            }
            for n in self.next(c) {
                if seen.insert(n) {
                    want.remove(&n);
                    queue.push(n);
                }
            }
        }
        want.is_empty()
    }

    fn flown_mask(&self, c: [i32; 3]) -> Quadrants {
        self.flown.get(&c).copied().unwrap_or(0)
    }

    /// Whether `c` is a standing or holding state some launch of which has
    /// not been flown.
    pub fn unflown(&self, c: [i32; 3]) -> bool {
        matches!(
            self.states.get(&c),
            Some(RestKind::Standing | RestKind::Holding)
        ) && self.flown_mask(c) != ALL_QUADRANTS
    }

    fn mark(&mut self, d: &Dense, c: [i32; 3]) {
        if let Some(i) = d.index(c) {
            if self.bits.is_empty() {
                self.bits = vec![0; d.cells.len().div_ceil(64)];
            }
            self.bits[i / 64] |= 1 << (i % 64);
        }
    }

    fn has(&self, d: &Dense, c: [i32; 3]) -> bool {
        match d.index(c) {
            Some(i) => self
                .bits
                .get(i / 64)
                .is_some_and(|w| w & (1 << (i % 64)) != 0),
            None => self.states.contains_key(&c),
        }
    }

    /// The reached cells.
    pub fn cells(&self) -> BTreeSet<[i32; 3]> {
        self.states.keys().copied().collect()
    }

    /// The reached standing states.
    pub fn standing(&self) -> BTreeSet<[i32; 3]> {
        self.states
            .iter()
            .filter(|(_, k)| **k == RestKind::Standing)
            .map(|(c, _)| *c)
            .collect()
    }

    /// The successors of `c`, open-sea states joined to one another.
    pub fn next(&self, c: [i32; 3]) -> impl Iterator<Item = [i32; 3]> + '_ {
        let sea = self.open_sea.contains(&c);
        self.succ.get(&c).into_iter().flatten().map(|e| e.to).chain(
            sea.then(|| self.open_sea.iter().copied())
                .into_iter()
                .flatten(),
        )
    }

    /// How many states of each kind: standing, afloat, submerged, holding.
    pub fn census(&self) -> [usize; 4] {
        let mut n = [0; 4];
        for k in self.states.values() {
            n[*k as usize] += 1;
        }
        n
    }

    /// How many edges of each kind, in [`EdgeKind::ALL`]'s order; the exit
    /// edges are the states that leave the world or lie in the open sea.
    pub fn edge_census(&self) -> [usize; 6] {
        let mut n = [0; 6];
        for es in self.succ.values() {
            for e in es {
                n[e.kind as usize] += 1;
            }
        }
        n[EdgeKind::Exit as usize] += self.leaves_world.len() + self.open_sea.len();
        n
    }
}

/// **The reach binding** (spec-0113 §5.5): what the graph was built over and
/// how much work it took, summed over the configurations a build judged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReachBinding {
    /// Distinct configurations.
    pub configurations: usize,
    /// Rest states by kind: standing, afloat, submerged, holding.
    pub states: [usize; 4],
    /// Edges by kind, in [`EdgeKind::ALL`]'s order.
    pub edges: [usize; 6],
    /// Launches flown over the real world.
    pub simulated: u64,
    /// Template arcs credited without a tick.
    pub credited: u64,
    /// Whether every fall is survivable (§4.2).
    pub unbounded_fall: bool,
    /// The sha256 of each configuration's occupancy, in configuration order.
    pub digests: Vec<String>,
}

impl ReachBinding {
    /// Fold one configuration's closure in.
    pub fn add(&mut self, world: &World, reach: &Reach) {
        self.configurations += 1;
        for (a, b) in self.states.iter_mut().zip(reach.census()) {
            *a += b;
        }
        for (a, b) in self.edges.iter_mut().zip(reach.edge_census()) {
            *a += b;
        }
        self.simulated += reach.simulated;
        self.credited += reach.credited;
        self.unbounded_fall |= world.unbounded_fall;
        self.digests.push(
            world
                .fingerprint()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        );
    }

    /// The work the graph took: launches simulated plus template arcs
    /// credited — the count the budget bounds ([`REACH_BUDGET_STATES`]).
    pub fn work(&self) -> u64 {
        self.simulated + self.credited
    }

    /// The line a build prints.
    pub fn line(&self) -> String {
        let [s, a, u, h] = self.states;
        let [w, f, j, d, c, x] = self.edges;
        format!(
            "reach binding: {} configuration(s); {} rest state(s) ({s} standing, {a} afloat, {u} \
             submerged, {h} holding); {} edge(s) ({w} walk, {f} fall, {j} jump, {d} dive, {c} climb, \
             {x} exit); {} launch state(s) simulated, {} template arc(s) credited; {} of budget {}; \
             survivable fall {} (unbounded: {})",
            self.configurations,
            s + a + u + h,
            w + f + j + d + c + x,
            self.simulated,
            self.credited,
            self.work(),
            delvewright_dsl::metrics::REACH_BUDGET_STATES,
            if self.unbounded_fall {
                "any".to_string()
            } else {
                format!(
                    "{} blocks",
                    delvewright_dsl::metrics::unarmoured_survivable_fall_blocks()
                )
            },
            if self.unbounded_fall { "yes" } else { "no" },
        )
    }

    /// `validation/reach-graph.json`: the same counts, per kind, with each
    /// configuration's occupancy sha256.
    pub fn to_json(&self) -> serde_json::Value {
        let [s, a, u, h] = self.states;
        let [w, f, j, d, c, x] = self.edges;
        serde_json::json!({
            "budget": delvewright_dsl::metrics::REACH_BUDGET_STATES,
            "configurations": self.configurations,
            "credited": self.credited,
            "edges": {"climb": c, "dive": d, "exit": x, "fall": f, "jump": j, "walk": w},
            "occupancy_sha256": self.digests,
            "simulated": self.simulated,
            "states": {"afloat": a, "holding": h, "standing": s, "submerged": u, "total": s + a + u + h},
            "unbounded_fall": self.unbounded_fall,
        })
    }
}

#[cfg(test)]
mod tests;
