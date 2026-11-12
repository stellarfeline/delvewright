//! **Every block the build writes is one the server keeps** (`DW1002`).
//!
//! A wall torch hung on a glass pane, a lantern hung under air, a flower on
//! stone, a rail on a fence: the build writes each of them and the pinned
//! server drops it the first time a shape update reaches it, because its
//! `canSurvive` rule asks a neighbour for something the neighbour does not
//! give. Until this check the only thing that noticed was the staging-time
//! comparison of the written world with a server save (`DW0955`), an hour of
//! the ladder after the build said yes.
//!
//! The rule is the pinned jar's, measured ([`delvewright_dsl::support`]); the
//! world judged is the one `DW0955` compares:
//!
//! * **at load** — [`crate::compiler::view::beat::load_blocks`] over
//!   [`crate::compiler::view::beat::picture_base`]: the assembled pieces, the
//!   world edits, the world-load seals, the relight fixtures and the trigger
//!   props, in the critical path's configuration at its first step. Every
//!   block that needs support is judged;
//! * **after each runtime write the model lays** — every distinct
//!   configuration the critical path passes ([`crate::compiler::nav::path_configurations`]),
//!   its forced writes laid as the emitted commands write them. Each written
//!   cell and its six neighbours are judged again there. An unforced write is
//!   not laid by the model, and a branch's configurations are not walked:
//!   neither is judged.
//!
//! A cell no block was written to is air, except under the `ocean` horizon,
//! where the generator's stone and water stand outside every placed piece's
//! box. A block whose rule the table cannot read from single neighbours
//! (`unjudged`: a crop's light, a vine's far reads) is counted by block and
//! not judged.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::blockshape::Face;
use delvewright_dsl::support::{Needs, Rule, needs};
use delvewright_dsl::{DwCode, ExitTier};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::nav::{Ambient, BuiltPiece, World};
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1002`: a block the build writes is one the pinned server drops — its
    /// `canSurvive` rule, measured from the jar
    /// (`crates/dsl/data/support-1.21.11.tsv`), is met by none of its
    /// neighbours in the world at load, or in a configuration the critical path
    /// passes. The message names the block, the cell, each neighbour that could
    /// hold it, what that neighbour must give, and what stands there.
    pub const DW_BLOCK_DROPPED: DwCode = DwCode::new("DW1002", ExitTier::Build);
}

/// How many dropped cells a refusal names.
const LIST_LIMIT: usize = 8;

const FACES: [Face; 6] = [
    Face::Down,
    Face::Up,
    Face::North,
    Face::South,
    Face::West,
    Face::East,
];

/// What the generator puts where nothing was written.
pub(crate) struct Surround {
    ambient: Ambient,
    built: Vec<BuiltPiece>,
}

impl Surround {
    pub(crate) fn of(plan: &Plan) -> Surround {
        Surround {
            ambient: Ambient::of_plan(plan),
            built: crate::compiler::nav::built_volume(plan),
        }
    }

    pub(crate) fn inside(&self, c: [i32; 3]) -> Option<&str> {
        self.built
            .iter()
            .find(|(_, (lo, hi))| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]))
            .map(|(id, _)| id.as_str())
    }

    /// The block at an unwritten cell: air inside a piece's box (`/place
    /// template` writes the whole box) and under the void horizon; the ocean
    /// superflat's layers outside every box.
    pub(crate) fn unwritten(&self, c: [i32; 3]) -> &'static str {
        if self.inside(c).is_some() {
            "minecraft:air"
        } else {
            self.generated(c)
        }
    }

    /// What the level generator put at `c` before any write: air under the
    /// void horizon, the ocean superflat's layers under the ocean.
    fn generated(&self, c: [i32; 3]) -> &'static str {
        match &self.ambient {
            Ambient::Void => "minecraft:air",
            Ambient::Ocean(sea) => {
                if c[1] > sea.level {
                    "minecraft:air"
                } else if c[1] > sea.floor_top {
                    "minecraft:water"
                } else if c[1] == crate::compiler::view::world::MIN_SECTION * 16 {
                    "minecraft:bedrock"
                } else if c[1] < crate::compiler::view::world::MIN_SECTION * 16 {
                    "minecraft:air"
                } else {
                    "minecraft:stone"
                }
            }
        }
    }
}

/// What one rule asks and what stands there: `(rule, neighbour cell,
/// neighbour block)`.
pub type Want = (Rule, [i32; 3], String);

/// One dropped block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    /// Where.
    pub cell: [i32; 3],
    /// The block state written there.
    pub block: String,
    /// What each rule asks and what stands there: `(rule, neighbour cell,
    /// neighbour block)`.
    pub wants: Vec<Want>,
    /// The piece whose box holds the cell, if any.
    pub piece: Option<String>,
}

/// What the judgement bound to, and what it found.
#[derive(Debug, Default)]
pub struct Binding {
    /// Non-air cells of the load world.
    pub cells: usize,
    /// Of them, blocks that need support (a `support` row).
    pub attached: usize,
    /// Of those, held.
    pub held: usize,
    /// Cells whose rule is unjudged, by block id.
    pub unjudged: BTreeMap<String, usize>,
    /// Cells holding a block the table does not hold, by block id.
    pub unknown: BTreeMap<String, usize>,
    /// Every dropped block.
    pub dropped: Vec<Dropped>,
}

impl Binding {
    /// The binding line, printed on every build.
    pub fn line(&self) -> String {
        let list = |m: &BTreeMap<String, usize>| {
            if m.is_empty() {
                "none".to_string()
            } else {
                m.iter()
                    .map(|(b, n)| format!("{b} x{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        format!(
            "attached blocks (DW1002): {} of {} non-air cell(s) at load need a neighbour to stay — \
             {} held, {} dropped; unjudged by the measured rule: {} cell(s) ({}); not in the \
             table: {} cell(s) ({})",
            self.attached,
            self.cells,
            self.held,
            self.dropped.len(),
            self.unjudged.values().sum::<usize>(),
            list(&self.unjudged),
            self.unknown.values().sum::<usize>(),
            list(&self.unknown),
        )
    }

    /// The refusal, when anything was dropped.
    pub fn refusal(&self) -> Option<String> {
        self.dropped.first()?;
        let mut msg = format!(
            "{} block(s) the build writes are dropped by the pinned server: each one's \
             `canSurvive` rule (measured from the jar, `crates/dsl/data/support-1.21.11.tsv`) is \
             met by none of its neighbours, so the server removes it the first time a shape \
             update reaches it, and the world the party walks is not the world the build wrote \
             (the staging comparison would find it as `DW0955`).",
            self.dropped.len()
        );
        for d in self.dropped.iter().take(LIST_LIMIT) {
            msg.push_str(&format!("\n  - {}", describe(d)));
        }
        if self.dropped.len() > LIST_LIMIT {
            msg.push_str(&format!(
                "\n  … and {} more.",
                self.dropped.len() - LIST_LIMIT
            ));
        }
        msg.push_str(
            "\nRepair it where the block is authored (the piece, or the edit that writes it): \
             give it the neighbour it asks for, hang it on a face that holds it, or choose a \
             block that needs nothing there.",
        );
        Some(msg)
    }
}

fn describe(d: &Dropped) -> String {
    let [x, y, z] = d.cell;
    let piece = d
        .piece
        .as_ref()
        .map(|p| format!(" (in the box of piece `{p}`)"))
        .unwrap_or_default();
    let wants = d
        .wants
        .iter()
        .map(|(rule, [nx, ny, nz], there)| {
            format!(
                "the block {} at [{nx}, {ny}, {nz}] to {}, and there is `{there}`",
                beside(rule.cell),
                rule.describe()
            )
        })
        .collect::<Vec<_>>()
        .join("; or ");
    format!(
        "`{}` at [{x}, {y}, {z}]{piece} at load needs {wants}",
        d.block
    )
}

/// Where the neighbour across `f` stands, in words.
pub(crate) fn beside(f: Face) -> String {
    match f {
        Face::Down => "below it".to_string(),
        Face::Up => "above it".to_string(),
        other => format!("{} of it", other.name()),
    }
}

fn step(c: [i32; 3], f: Face) -> [i32; 3] {
    let o = f.offset();
    [c[0] + o[0], c[1] + o[1], c[2] + o[2]]
}

/// Judge one cell whose block is `name`, with `at` answering any cell.
fn judge_cell<'a>(
    cell: [i32; 3],
    name: &str,
    at: &dyn Fn([i32; 3]) -> &'a str,
) -> Option<Result<(), Vec<Want>>> {
    match needs(name)? {
        Needs::Free | Needs::Unjudged(_) => None,
        Needs::Support(rules) => {
            if rules.iter().any(|r| r.holds(at(step(cell, r.cell)))) {
                Some(Ok(()))
            } else {
                Some(Err(rules
                    .iter()
                    .map(|r| {
                        let n = step(cell, r.cell);
                        (r.clone(), n, at(n).to_string())
                    })
                    .collect()))
            }
        }
    }
}

/// **Judge the world the load ends in** (module header): every block of the
/// world `DW0955` compares.
pub fn judge(plan: &Plan, world: &World, base: &BlockMap) -> Binding {
    let load = crate::compiler::view::beat::load_blocks(plan, world, base);
    judge_over(plan, &load)
}

/// [`judge`] over the load world already laid
/// ([`crate::compiler::view::beat::load_blocks`]), so the build lays it once
/// for every judgement made of it.
pub fn judge_over(plan: &Plan, load: &BlockMap) -> Binding {
    judge_load(load, &Surround::of(plan))
}

/// [`judge`] over a load map: every non-air cell of `load`, its unwritten
/// neighbours answered by `surround`.
fn judge_load(load: &BlockMap, surround: &Surround) -> Binding {
    let mut b = Binding::default();
    let at_load = |c: [i32; 3]| -> &str {
        load.get(&c)
            .map(|s| &**s)
            .unwrap_or_else(|| surround.unwritten(c))
    };
    for (cell, state) in load {
        let name: &str = state;
        if delvewright_dsl::blockshape::is_air(name) {
            continue;
        }
        b.cells += 1;
        match needs(name) {
            None => *b.unknown.entry(base_name(name)).or_default() += 1,
            Some(Needs::Unjudged(_)) => *b.unjudged.entry(base_name(name)).or_default() += 1,
            Some(Needs::Free) => {}
            Some(Needs::Support(_)) => {
                b.attached += 1;
                match judge_cell(cell, name, &at_load) {
                    Some(Ok(())) => b.held += 1,
                    Some(Err(wants)) => b.dropped.push(Dropped {
                        cell,
                        block: name.to_string(),
                        wants,
                        piece: surround.inside(cell).map(str::to_string),
                    }),
                    None => {}
                }
            }
        }
    }
    b
}

fn base_name(name: &str) -> String {
    delvewright_dsl::blockshape::base_id(name).to_string()
}

// ---------------------------------------------------------------------------
// The write sequence: the load as the server runs it, one write at a time.
// ---------------------------------------------------------------------------

/// One block write of the load, as its command reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceWrite {
    /// `<function>:<line>`, 1-based, of the command in the shipped datapack.
    pub at: String,
    /// The command.
    pub command: String,
}

/// A block the load sequence writes and the server does not keep where it was
/// written: an attached block whose rule no neighbour meets when the write
/// lands, or a falling block written over a cell it falls into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceDrop {
    /// Where.
    pub cell: [i32; 3],
    /// The block state written there.
    pub block: String,
    /// What it needed and did not have, in words.
    pub why: String,
    /// The write after which it was judged.
    pub after: SequenceWrite,
}

/// What [`judge_sequence`] bound to, and what it found.
#[derive(Debug, Default)]
pub struct SequenceBinding {
    /// The functions the load runs, in order.
    pub functions: Vec<String>,
    /// Writes laid, by kind.
    pub templates: usize,
    pub fills: usize,
    pub setblocks: usize,
    /// Cells a write changed, judged with their six neighbours.
    pub changed: usize,
    /// Of the cells judged, attached blocks and falling blocks.
    pub attached: usize,
    pub falling: usize,
    /// Writes behind an `execute` condition the load does not decide, not laid.
    pub conditional: usize,
    /// Block-writing lines this reader does not read (a mode it does not
    /// model, a template no piece places), not laid — each named.
    pub unread: Vec<SequenceWrite>,
    /// Every block the sequence does not keep, in write order.
    pub dropped: Vec<SequenceDrop>,
}

impl SequenceBinding {
    /// The binding line, printed on every build.
    pub fn line(&self) -> String {
        format!(
            "write sequence (DW1002): {} write(s) the load runs in order over {} function(s) — \
             {} template(s), {} fill(s), {} setblock(s); {} changed cell(s) judged with their \
             neighbours as each write lands, {} attached and {} falling block(s) among them, {} \
             not kept; {} conditional write(s) not laid; {} line(s) not read",
            self.templates + self.fills + self.setblocks,
            self.functions.len(),
            self.templates,
            self.fills,
            self.setblocks,
            self.changed,
            self.attached,
            self.falling,
            self.dropped.len(),
            self.conditional,
            self.unread.len(),
        )
    }

    /// The refusal, when the sequence drops anything or holds a line it cannot
    /// read.
    pub fn refusal(&self) -> Option<String> {
        if self.dropped.is_empty() && self.unread.is_empty() {
            return None;
        }
        let mut msg = String::new();
        if !self.dropped.is_empty() {
            msg.push_str(&format!(
                "{} block(s) the load writes are not kept where they are written: the server \
                 runs the load one write at a time (`{}`), and as each write lands every block \
                 it changes and every neighbour is asked the pinned jar's rules — an attached \
                 block's `canSurvive` (`crates/dsl/data/support-1.21.11.tsv`) and a falling \
                 block's `FallingBlock.isFree` below it (`support-bases-1.21.11.tsv`). A block \
                 that fails is removed, or falls; a later write landing on a falling block \
                 turns it into an item on the ground.",
                self.dropped.len(),
                self.functions.join(" → "),
            ));
            msg.push_str(&format!("\n  By block: {}.", tally(&self.dropped)));
            for d in self.dropped.iter().take(LIST_LIMIT) {
                let [x, y, z] = d.cell;
                msg.push_str(&format!(
                    "\n  - `{}` at [{x}, {y}, {z}], after `{}` ({}): {}",
                    d.block, d.after.command, d.after.at, d.why
                ));
            }
            if self.dropped.len() > LIST_LIMIT {
                msg.push_str(&format!(
                    "\n  … and {} more.",
                    self.dropped.len() - LIST_LIMIT
                ));
            }
            msg.push_str(
                "\nRepair it where the block is authored: give it what it needs in the same \
                 piece (or the same edit) that writes it — a block under a falling block, the \
                 face an attached block hangs on — so it holds the moment it lands, whatever \
                 is written after it; or choose a block that needs nothing there.",
            );
        }
        if !self.unread.is_empty() {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str(&format!(
                "{} block-writing line(s) of the load are ones this check cannot read, so the \
                 world after them is not known and nothing after them is judged honestly. \
                 The emitter that wrote them owes a form the sequence reads:",
                self.unread.len()
            ));
            for u in self.unread.iter().take(LIST_LIMIT) {
                msg.push_str(&format!("\n  - `{}` ({})", u.command, u.at));
            }
        }
        Some(msg)
    }
}

/// How many of `dropped` each block id is, most first (ties by id).
fn tally(dropped: &[SequenceDrop]) -> String {
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for d in dropped {
        *by.entry(base_name(&d.block)).or_default() += 1;
    }
    let mut v: Vec<(String, usize)> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.iter()
        .map(|(b, n)| format!("{b} x{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The functions the load runs, in order, read off the shipped datapack: `setup`,
/// the ground steps (`ground_verify`'s target and every step it schedules), the
/// placement (`place_all`), then the world build `place_verify` starts and every
/// step it schedules, ending in `setup_finish`.
fn load_order(ns: &str, bodies: &BTreeMap<&str, &str>) -> Vec<String> {
    let call_target = |line: &str, verb: &str| -> Option<String> {
        let rest = line.split(verb).nth(1)?;
        let id = rest.split_whitespace().next()?;
        id.strip_prefix(&format!("{ns}:")).map(str::to_string)
    };
    // The function the last line of `name` calls, if it ends in a call.
    let last_call = |name: &str| -> Option<String> {
        let body = bodies.get(name)?;
        let last = body.lines().rev().find(|l| !l.trim().is_empty())?;
        call_target(last, "run function ")
    };
    let chain = |start: Option<String>, out: &mut Vec<String>| {
        let mut next = start;
        while let Some(n) = next.take() {
            if out.contains(&n) || !bodies.contains_key(n.as_str()) {
                break;
            }
            // `setup_finish` ends the load: what it schedules is play.
            if n != "setup_finish" {
                next = bodies[n.as_str()]
                    .lines()
                    .find_map(|l| call_target(l, "schedule function "));
            }
            out.push(n);
        }
    };
    let mut order = vec!["setup".to_string()];
    chain(
        last_call(crate::compiler::chain::GROUND_VERIFY_FN),
        &mut order,
    );
    order.push("place_all".to_string());
    chain(last_call("place_verify"), &mut order);
    order.retain(|n| bodies.contains_key(n.as_str()));
    order
}

/// The block a write lays, read off its token: the state, without block-entity
/// data.
fn state_of(token: &str) -> String {
    let mut depth = 0i32;
    for (i, ch) in token.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => depth -= 1,
            '{' if depth == 0 => return token[..i].to_string(),
            _ => {}
        }
    }
    token.to_string()
}

/// Where a `fill` lays its block: `replace <filter>` only over the filter's
/// block (its id alone, or the exact state when it names properties); `keep`
/// only over air; no mode, or `replace` alone, everywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Lays {
    Everywhere,
    Over(String),
    OverAir,
}

impl Lays {
    fn lays_over(&self, there: &str) -> bool {
        use delvewright_dsl::blockshape::{base_id, is_air};
        match self {
            Lays::Everywhere => true,
            Lays::OverAir => is_air(there),
            Lays::Over(f) if f.contains('[') => there == f,
            Lays::Over(f) => base_id(there) == base_id(f),
        }
    }
}

fn coords(t: &[String]) -> Option<[i32; 3]> {
    Some([t[0].parse().ok()?, t[1].parse().ok()?, t[2].parse().ok()?])
}

/// The sequence's world: what each write left, over what the generator put.
struct Sim<'a> {
    cells: BlockMap,
    surround: &'a Surround,
}

impl Sim<'_> {
    fn at(&self, c: [i32; 3]) -> &'static str {
        self.cells
            .get(&c)
            .map(|s| s.as_str())
            .unwrap_or_else(|| self.surround.generated(c))
    }

    /// Write `state` at `c`; whether the cell changed.
    fn put(&mut self, c: [i32; 3], state: crate::compiler::blockstate::BlockState) -> bool {
        if self.at(c) == state.as_str() {
            return false;
        }
        self.cells.insert(c, state);
        true
    }
}

/// Every `place template` the build can run, keyed as the command names it
/// (`<ns:id>` and the position): the structure file and the piece's rotation.
pub type TemplateIndex = BTreeMap<(String, [i32; 3]), (String, crate::compiler::solver::Rotation)>;

/// One block write a command lays: each cell it reaches with the state it
/// leaves there, over the cells its filter admits.
struct Laid {
    shape: Shape,
    filter: Lays,
}

/// The cells a block write reaches, before they are laid.
enum Shape {
    /// A `fill`: every cell of the inclusive box `lo..=hi`, one state.
    Box([i32; 3], [i32; 3], crate::compiler::blockstate::BlockState),
    /// A `setblock` or a template: these cells, in the order they are laid.
    Cells(Vec<([i32; 3], crate::compiler::blockstate::BlockState)>),
}

/// An inclusive rectangle of columns, `[x0, z0]..=[x1, z1]`, every height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Columns {
    lo: [i32; 2],
    hi: [i32; 2],
}

impl Columns {
    fn holds(&self, c: [i32; 3]) -> bool {
        self.lo[0] <= c[0] && c[0] <= self.hi[0] && self.lo[1] <= c[2] && c[2] <= self.hi[1]
    }

    fn meets(&self, lo: [i32; 3], hi: [i32; 3]) -> bool {
        lo[0] <= self.hi[0] && self.lo[0] <= hi[0] && lo[2] <= self.hi[1] && self.lo[1] <= hi[2]
    }

    /// Grown by `r` cells on every horizontal side.
    fn grown(&self, r: i32) -> Columns {
        Columns {
            lo: [self.lo[0] - r, self.lo[1] - r],
            hi: [self.hi[0] + r, self.hi[1] + r],
        }
    }

    fn covers(&self, o: &Columns) -> bool {
        self.lo[0] <= o.lo[0]
            && self.lo[1] <= o.lo[1]
            && o.hi[0] <= self.hi[0]
            && o.hi[1] <= self.hi[1]
    }
}

impl Shape {
    /// The inclusive box every cell it reaches lies in; `None` when it reaches
    /// none.
    fn bounds(&self) -> Option<([i32; 3], [i32; 3])> {
        match self {
            Shape::Box(lo, hi, _) => Some((*lo, *hi)),
            Shape::Cells(cells) => {
                let (first, _) = cells.first()?;
                Some(cells.iter().fold((*first, *first), |(lo, hi), (c, _)| {
                    (
                        [0, 1, 2].map(|i| lo[i].min(c[i])),
                        [0, 1, 2].map(|i| hi[i].max(c[i])),
                    )
                }))
            }
        }
    }

    /// The cells it reaches with their column in `within` (all of them when
    /// `None`), in the order it lays them.
    fn cells_within(
        self,
        within: Option<Columns>,
    ) -> Vec<([i32; 3], crate::compiler::blockstate::BlockState)> {
        match (self, within) {
            (Shape::Box(lo, hi, st), None) => crate::compiler::assembled::region_cells(lo, hi)
                .map(|c| (c, st))
                .collect(),
            (Shape::Box(lo, hi, st), Some(w)) => {
                if !w.meets(lo, hi) {
                    return Vec::new();
                }
                let a = [lo[0].max(w.lo[0]), lo[1], lo[2].max(w.lo[1])];
                let b = [hi[0].min(w.hi[0]), hi[1], hi[2].min(w.hi[1])];
                crate::compiler::assembled::region_cells(a, b)
                    .map(|c| (c, st))
                    .collect()
            }
            (Shape::Cells(cells), None) => cells,
            (Shape::Cells(mut cells), Some(w)) => {
                cells.retain(|(c, _)| w.holds(*c));
                cells
            }
        }
    }
}

/// What reading one command line gave.
enum Read {
    /// Not a block write.
    Other,
    /// A block write behind an `execute` (`conditional`), and what it lays.
    Write {
        conditional: bool,
        laid: Laid,
        kind: &'static str,
    },
    /// A block write this reader cannot lay.
    Unread,
}

/// Read one shipped command line: the cells it reaches and what it leaves in
/// each, and the filter that decides, over the world it runs in, which of them
/// it writes.
fn read_line(
    line: &str,
    templates: &TemplateIndex,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Read {
    use crate::compiler::blockstate::BlockState;
    let Ok((all, _)) = crate::compiler::commands::tokenize(line) else {
        return Read::Other;
    };
    // An `execute` chain lays what its last `run` runs.
    let (t, conditional) = match all.first().map(String::as_str) {
        Some("execute") => match all.iter().rposition(|w| w == "run") {
            Some(i) => (&all[i + 1..], true),
            None => return Read::Other,
        },
        _ => (&all[..], false),
    };
    let Some(head) = t.first().map(String::as_str) else {
        return Read::Other;
    };
    let mut filter = Lays::Everywhere;
    let (kind, shape) = match head {
        "place" if t.get(1).is_some_and(|w| w == "template") => {
            let pos = t.get(3..6).and_then(coords);
            let found = pos.and_then(|p| templates.get(&(t[2].clone(), p)));
            let Some((p, bytes, rot)) =
                found.and_then(|(file, r)| Some((pos?, structures.get(file)?, *r)))
            else {
                return Read::Unread;
            };
            let cells = crate::compiler::assembled::template_writes(rot, p, bytes)
                .into_iter()
                .map(|(c, s, _)| (c, s))
                .collect();
            ("template", Shape::Cells(cells))
        }
        "fill" => {
            let (Some(a), Some(z), Some(block)) = (
                t.get(1..4).and_then(coords),
                t.get(4..7).and_then(coords),
                t.get(7),
            ) else {
                return Read::Unread;
            };
            filter = match t.get(8).map(String::as_str) {
                None => Lays::Everywhere,
                Some("replace") if t.get(9).is_some_and(|f| f.starts_with('#')) => {
                    return Read::Unread;
                }
                Some("replace") => t
                    .get(9)
                    .map_or(Lays::Everywhere, |f| Lays::Over(state_of(f))),
                Some("keep") => Lays::OverAir,
                Some(_) => return Read::Unread,
            };
            let state = BlockState::new(&state_of(block));
            let lo = [0, 1, 2].map(|i| a[i].min(z[i]));
            let hi = [0, 1, 2].map(|i| a[i].max(z[i]));
            ("fill", Shape::Box(lo, hi, state))
        }
        "setblock" => {
            let (Some(c), Some(block)) = (t.get(1..4).and_then(coords), t.get(4)) else {
                return Read::Unread;
            };
            match t.get(5).map(String::as_str) {
                None | Some("replace") => {}
                Some("keep") => filter = Lays::OverAir,
                Some(_) => return Read::Unread,
            }
            (
                "setblock",
                Shape::Cells(vec![(c, BlockState::new(&state_of(block)))]),
            )
        }
        "clone" => return Read::Unread,
        _ => return Read::Other,
    };
    Read::Write {
        conditional,
        laid: Laid { shape, filter },
        kind,
    }
}

/// A function's lines with the unconditional `function` calls in it inlined,
/// each as `(<function>, <line number, 1-based>, <line>)`; every function
/// inlined is added to `seen`.
fn expand<'a>(
    ns: &str,
    name: &str,
    bodies: &BTreeMap<&'a str, &'a str>,
    stack: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(&'a str, usize, &'a str)>,
) {
    if stack.iter().any(|s| s == name) {
        return;
    }
    let Some((&name, &body)) = bodies.get_key_value(name) else {
        return;
    };
    seen.insert(name.to_string());
    stack.push(name.to_string());
    for (i, line) in body.lines().enumerate() {
        let l = line.trim();
        if let Some(callee) = l
            .strip_prefix("function ")
            .and_then(|r| r.split_whitespace().next())
            .and_then(|id| id.strip_prefix(&format!("{ns}:")))
        {
            expand(ns, callee, bodies, stack, seen, out);
            continue;
        }
        out.push((name, i + 1, l));
    }
    stack.pop();
}

/// What the runtime arm bound to, and what it found.
#[derive(Debug, Default)]
pub struct RuntimeBinding {
    /// Functions outside the load that write blocks.
    pub functions: usize,
    /// Block-writing lines read, and of them behind an `execute` condition.
    pub writes: usize,
    pub conditional: usize,
    /// Cells some runtime write may change.
    pub cells: usize,
    /// Cells judged (those and their neighbours), and the (cell, block) pairs
    /// judged there.
    pub judged: usize,
    pub pairs: usize,
    /// Block-writing lines this reader cannot lay (a relative position, a mode
    /// it does not model) — each named, never judged.
    pub unread: Vec<SequenceWrite>,
    /// Every block some configuration does not keep.
    pub dropped: Vec<SequenceDrop>,
}

impl RuntimeBinding {
    /// The binding line, printed on every build.
    pub fn line(&self) -> String {
        format!(
            "runtime writes (DW1002): {} block-writing line(s) outside the load, in {} \
             function(s), {} of them behind a condition, may change {} cell(s); {} cell(s) judged \
             with their neighbours against every block any write may leave there ({} block(s) \
             judged) — every configuration a story path, a branch or an unforced beat can reach, \
             in any order; {} not kept; {} line(s) not read",
            self.writes,
            self.functions,
            self.conditional,
            self.cells,
            self.judged,
            self.pairs,
            self.dropped.len(),
            self.unread.len(),
        )
    }

    /// The refusal, when some configuration drops a block.
    pub fn refusal(&self) -> Option<String> {
        self.dropped.first()?;
        let mut msg = format!(
            "{} block(s) are not kept in some configuration the story can reach: a cell a \
             runtime write reaches — a `set-block`, a `fill-region` or `clear-region`, a gate, a \
             way, a collapse, on any path, branch or beat nobody has to play — can hold its \
             block at load or any block any write leaves there, and a block beside it must be \
             held by every one of them. The server drops it, as an item, the first time a write \
             leaves it without its support.",
            self.dropped.len()
        );
        msg.push_str(&format!("\n  By block: {}.", tally(&self.dropped)));
        for d in self.dropped.iter().take(LIST_LIMIT) {
            let [x, y, z] = d.cell;
            msg.push_str(&format!(
                "\n  - `{}` at [{x}, {y}, {z}] ({}): {}",
                d.block, d.after.at, d.why
            ));
        }
        if self.dropped.len() > LIST_LIMIT {
            msg.push_str(&format!(
                "\n  … and {} more.",
                self.dropped.len() - LIST_LIMIT
            ));
        }
        msg.push_str(
            "\nRepair it where the block is authored: hang it on a block no write removes or \
             replaces, move the write off its support, or have the write itself lay what holds \
             it.",
        );
        Some(msg)
    }
}

/// Both write arms' findings.
#[derive(Debug, Default)]
pub struct Writes {
    pub sequence: SequenceBinding,
    pub runtime: RuntimeBinding,
    /// How the sequence arm's replay was cut.
    pub by_chunk: ByChunk,
}

/// **Judge every block write the shipped datapack makes** (module header): the
/// load one write at a time, then every write outside it against every block
/// any write may leave.
pub fn judge_writes(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
    functions: &[(String, String)],
) -> Writes {
    let ns = plan.namespace.as_str();
    let mut templates = TemplateIndex::new();
    for piece in plan.placed_pieces() {
        for t in &piece.templates {
            templates.insert(
                (format!("{ns}:{}", t.structure_id), t.pos),
                (t.structure_file.clone(), piece.rotation),
            );
        }
    }
    judge_writes_over(ns, functions, &templates, structures, &Surround::of(plan))
}

/// The side of a chunk the load is replayed by, in section columns.
const CHUNK_SIDE: i32 = 4;
/// The halo a chunk's replay is first tried with, in cells.
const CHUNK_HALO: i32 = 16;

/// The sequence arm's binding, judged by chunk of columns
/// ([`replay_by_chunk`]): the load replayed over each chunk and a halo.
fn judge_writes_over(
    ns: &str,
    functions: &[(String, String)],
    templates: &TemplateIndex,
    structures: &BTreeMap<String, Vec<u8>>,
    surround: &Surround,
) -> Writes {
    judge_writes_by(
        ns, functions, templates, structures, surround, CHUNK_SIDE, CHUNK_HALO,
    )
}

/// One block write of the load: where it is, what it says, and the box its
/// cells lie in.
struct LoadWrite<'a> {
    /// The function of the load it runs in, and its place in that function's
    /// lines with every call inlined.
    f: usize,
    q: usize,
    /// The function whose line it is, and the line's number (1-based).
    name: &'a str,
    no: usize,
    line: &'a str,
    lo: [i32; 3],
    hi: [i32; 3],
}

impl LoadWrite<'_> {
    fn write(&self) -> SequenceWrite {
        SequenceWrite {
            at: format!("{}:{}", self.name, self.no),
            command: self.line.to_string(),
        }
    }
}

/// How the load's replay was cut ([`replay_by_chunk`]), printed on every
/// build that assembles a world.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ByChunk {
    /// A chunk's side, in section columns, and the halo first tried, in cells.
    pub side: i32,
    pub halo: i32,
    /// Chunks some write reaches, and the replays run over them.
    pub chunks: usize,
    pub replays: usize,
    /// Chunks replayed again over a wider halo, and the widest halo a chunk
    /// was judged over.
    pub regrown: usize,
    pub widest: i32,
    /// The generator's own blocks read a neighbour, so the load is replayed
    /// whole.
    pub whole: bool,
    /// The most sections one replay's world held.
    pub sections: usize,
}

impl ByChunk {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "write sequence by chunk: {} chunk(s) of {} x {} section column(s), replayed {} \
             time(s), each over its own columns and a halo of {} cell(s), {} of them again over \
             a wider halo (the widest {} cell(s)){}; at most {} section(s) held by one replay \
             (ADR-0037 §2)",
            self.chunks,
            self.side,
            self.side,
            self.replays,
            self.halo,
            self.regrown,
            self.widest,
            if self.whole {
                ", the load replayed whole: the generator's blocks read a neighbour"
            } else {
                ""
            },
            self.sections,
        )
    }
}

/// The faces a block state's survival reads: an attached block's rule cells,
/// and the cell below a falling block. One bit per face of [`FACES`].
fn reads_of(name: &str) -> u8 {
    let mut m = 0u8;
    let bit = |f: Face| -> u8 { 1 << FACES.iter().position(|g| *g == f).expect("a face") };
    if let Some(Needs::Support(rules)) = needs(name) {
        for r in rules {
            m |= bit(r.cell);
        }
    }
    if crate::compiler::assembled::is_falling_block(name) {
        m |= bit(Face::Down);
    }
    m
}

/// [`judge_writes_over`] with the chunk's side and first halo given.
fn judge_writes_by(
    ns: &str,
    functions: &[(String, String)],
    templates: &TemplateIndex,
    structures: &BTreeMap<String, Vec<u8>>,
    surround: &Surround,
    side: i32,
    halo: i32,
) -> Writes {
    let bodies: BTreeMap<&str, &str> = functions
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    let mut b = SequenceBinding {
        functions: load_order(ns, &bodies),
        ..SequenceBinding::default()
    };
    let mut at_load: BTreeSet<String> = BTreeSet::new();
    // The load's writes, each read once for its kind and its box.
    let mut load: Vec<LoadWrite> = Vec::new();
    for (f, outer) in b.functions.clone().iter().enumerate() {
        let mut queue: Vec<(&str, usize, &str)> = Vec::new();
        expand(
            ns,
            outer,
            &bodies,
            &mut Vec::new(),
            &mut at_load,
            &mut queue,
        );
        for (q, (name, no, line)) in queue.into_iter().enumerate() {
            match read_line(line, templates, structures) {
                Read::Other => {}
                Read::Unread => b.unread.push(SequenceWrite {
                    at: format!("{name}:{no}"),
                    command: line.to_string(),
                }),
                Read::Write {
                    conditional: true, ..
                } => b.conditional += 1,
                Read::Write { laid, kind, .. } => {
                    match kind {
                        "template" => b.templates += 1,
                        "fill" => b.fills += 1,
                        _ => b.setblocks += 1,
                    }
                    if let Some((lo, hi)) = laid.shape.bounds() {
                        load.push(LoadWrite {
                            f,
                            q,
                            name,
                            no,
                            line,
                            lo,
                            hi,
                        });
                    }
                }
            }
        }
    }
    let order = b.functions.clone();
    let (cells, by_chunk) = replay_by_chunk(
        &load, &order, templates, structures, surround, side, halo, &mut b,
    );
    let sim = Sim { cells, surround };
    let runtime = judge_runtime(ns, &bodies, &at_load, &sim, templates, structures);
    Writes {
        sequence: b,
        runtime,
        by_chunk,
    }
}

/// **The load replayed by chunk of columns** (ADR-0037 §2, "the write
/// sequence"). Each chunk of `side` x `side` section columns some write
/// reaches is replayed over its own columns and a halo — every write of the
/// load, in order, laid over the columns of the chunk and the halo — and only
/// what lands in its own columns is kept: the cells it changes, the blocks
/// asked there, the drops there. What a replay answers for a cell is what the
/// whole replay answers whenever no chain of reads leaves the halo: a cell's
/// verdict reads only the cells its block's rules name (a falling block, the
/// cell below), so a cell can come to differ from the whole replay only by a
/// drop, and a drop only through such a read of a cell that differs. From
/// every cell of the chunk that ever held a block that reads, the reads are
/// followed through every cell that ever held one; if they reach a cell
/// outside the halo that some write reaches, the chunk is replayed again over
/// twice the halo, until none does or the halo holds every write. The drops
/// are put back in the load's order — function, line, cell — and the cells
/// the load leaves are kept chunk by chunk for the runtime arm.
#[allow(clippy::too_many_arguments)]
fn replay_by_chunk(
    load: &[LoadWrite],
    functions: &[String],
    templates: &TemplateIndex,
    structures: &BTreeMap<String, Vec<u8>>,
    surround: &Surround,
    side: i32,
    halo: i32,
    b: &mut SequenceBinding,
) -> (BlockMap, ByChunk) {
    assert!(side >= 1 && halo >= 1, "a chunk and a halo of one or more");
    let mut stats = ByChunk {
        side,
        halo,
        ..ByChunk::default()
    };
    let mut cells = BlockMap::new();
    let Some(all) = load.iter().map(|w| (w.lo, w.hi)).reduce(|(a, b), (c, d)| {
        (
            [0, 1, 2].map(|i| a[i].min(c[i])),
            [0, 1, 2].map(|i| b[i].max(d[i])),
        )
    }) else {
        return (cells, stats);
    };
    let span = Columns {
        lo: [all.0[0], all.0[2]],
        hi: [all.1[0], all.1[2]],
    };
    // A cell no write reaches holds what the generator put there, which
    // depends on its height alone; if any of that reads a neighbour, an
    // unwritten cell can drop, and the chunk's own reads are not all held.
    stats.whole =
        (all.0[1] - 1..=all.1[1] + 1).any(|y| reads_of(surround.generated([0, y, 0])) != 0);
    let width = side * crate::compiler::sections::SIDE;
    let chunks: Vec<Columns> = if stats.whole {
        vec![span]
    } else {
        let (cx0, cx1) = (span.lo[0].div_euclid(width), span.hi[0].div_euclid(width));
        let (cz0, cz1) = (span.lo[1].div_euclid(width), span.hi[1].div_euclid(width));
        (cx0..=cx1)
            .flat_map(|cx| {
                (cz0..=cz1).map(move |cz| Columns {
                    lo: [cx * width, cz * width],
                    hi: [cx * width + width - 1, cz * width + width - 1],
                })
            })
            .filter(|own| load.iter().any(|w| own.meets(w.lo, w.hi)))
            .collect()
    };
    // Each chunk is replayed from the load alone and reads nothing another
    // chunk's replay computes, so the chunks are replayed in parallel
    // (`par::try_for_each_ordered`) and folded here in chunk order: the counts
    // summed, the cells laid and the drops gathered as the sequential walk
    // gathered them. At most `threads` replays' worlds (each a chunk and its
    // halo) and `2 × threads` chunks' own cells and drops are held at once,
    // where the sequential walk held one replay (ADR-0037 §2).
    let whole = stats.whole;
    let judge_chunk = |own: &Columns| -> JudgedChunk {
        let mut r = if whole { 0 } else { halo };
        let mut replays = 0;
        let mut sections = 0;
        loop {
            let reach = own.grown(r);
            let held = whole || reach.covers(&span);
            replays += 1;
            let rep = replay(
                load, functions, *own, reach, templates, structures, surround,
            );
            sections = sections.max(rep.cells.sections());
            if held || !reaches_past(&rep.reads, *own, reach, all) {
                let cells = rep
                    .cells
                    .in_box(
                        [own.lo[0], i32::MIN, own.lo[1]],
                        [own.hi[0], i32::MAX, own.hi[1]],
                    )
                    .map(|(c, st)| (c, *st))
                    .collect();
                return JudgedChunk {
                    halo: r,
                    regrown: r > halo,
                    replays,
                    sections,
                    changed: rep.changed,
                    attached: rep.attached,
                    falling: rep.falling,
                    drops: rep.drops,
                    cells,
                };
            }
            r *= 2;
        }
    };
    let mut drops: Vec<KeyedDrop> = Vec::new();
    let folded: Result<(), std::convert::Infallible> =
        crate::par::try_for_each_ordered(&chunks, judge_chunk, |j| {
            stats.chunks += 1;
            stats.replays += j.replays;
            stats.sections = stats.sections.max(j.sections);
            stats.widest = stats.widest.max(j.halo);
            stats.regrown += usize::from(j.regrown);
            b.changed += j.changed;
            b.attached += j.attached;
            b.falling += j.falling;
            drops.extend(j.drops);
            for (c, st) in j.cells {
                cells.insert(c, st);
            }
            Ok(())
        });
    let Ok(()) = folded;
    drops.sort_by_key(|d| d.0);
    b.dropped = drops.into_iter().map(|(_, d)| d).collect();
    (cells, stats)
}

/// One chunk as [`replay_by_chunk`] folds it: the replay that held (its
/// halo, and whether it was regrown past the first), how many replays and the
/// most sections one held, its counts, its drops, and the cells it leaves in
/// its own columns, in cell order.
struct JudgedChunk {
    halo: i32,
    regrown: bool,
    replays: usize,
    sections: usize,
    changed: usize,
    attached: usize,
    falling: usize,
    drops: Vec<KeyedDrop>,
    cells: Vec<([i32; 3], crate::compiler::blockstate::BlockState)>,
}

/// A drop with its place in the load: function, line (`usize::MAX` for the
/// function's falls), cell.
type KeyedDrop = ((usize, usize, [i32; 3]), SequenceDrop);

/// What one chunk's replay found in its own columns, and the world it laid.
struct Replayed {
    changed: usize,
    attached: usize,
    falling: usize,
    /// Each drop in its own columns.
    drops: Vec<KeyedDrop>,
    cells: BlockMap,
    /// Every cell that ever held a block that reads: the faces read.
    reads: crate::compiler::sections::SectionMap<u8>,
}

/// The load laid over `reach`, the replay of [`judge_writes_whole`] with
/// every write clipped to those columns; what lands in `own` is counted.
fn replay(
    load: &[LoadWrite],
    functions: &[String],
    own: Columns,
    reach: Columns,
    templates: &TemplateIndex,
    structures: &BTreeMap<String, Vec<u8>>,
    surround: &Surround,
) -> Replayed {
    use crate::compiler::blockstate::BlockState;
    let air = BlockState::new("minecraft:air");
    let mut sim = Sim {
        cells: BlockMap::new(),
        surround,
    };
    let mut reads: crate::compiler::sections::SectionMap<u8> =
        crate::compiler::sections::SectionMap::new();
    let mark = |reads: &mut crate::compiler::sections::SectionMap<u8>, c: [i32; 3], m: u8| {
        let was = reads.get(&c).copied().unwrap_or(0);
        if was | m != was {
            reads.insert(c, was | m);
        }
    };
    let mut out = Replayed {
        changed: 0,
        attached: 0,
        falling: 0,
        drops: Vec::new(),
        cells: BlockMap::new(),
        reads: crate::compiler::sections::SectionMap::new(),
    };
    let writes: Vec<&LoadWrite> = load.iter().filter(|w| reach.meets(w.lo, w.hi)).collect();
    let mut k = 0;
    while k < writes.len() {
        let f = writes[k].f;
        // As the whole replay keeps them: each cell a falling block may stand
        // in once the function has run, with the first write that changed it
        // or the cell under it.
        let mut falls: crate::compiler::sections::SectionMap<u32> =
            crate::compiler::sections::SectionMap::new();
        while k < writes.len() && writes[k].f == f {
            let lw = writes[k];
            let wk = u32::try_from(k).expect("fewer than 2^32 writes");
            k += 1;
            let Read::Write {
                laid,
                kind,
                conditional: false,
            } = read_line(lw.line, templates, structures)
            else {
                unreachable!("the load's writes were read as unconditional writes");
            };
            let template = kind == "template";
            // The cells the write changed, and below the cells it asks, are
            // sets kept as ascending, deduplicated vectors: the same cells in
            // the same order as the whole replay's ordered sets, without a
            // tree node per cell of a fill that lays a hundred thousand.
            let mut changed: Vec<[i32; 3]> = Vec::new();
            for (c, st) in laid.shape.cells_within(Some(reach)) {
                let was = sim.at(c);
                if laid.filter.lays_over(was) && sim.put(c, st) {
                    mark(&mut reads, c, reads_of(was) | reads_of(st.as_str()));
                    changed.push(c);
                }
            }
            if changed.is_empty() {
                continue;
            }
            changed.sort_unstable();
            changed.dedup();
            let is_changed = |c: &[i32; 3]| changed.binary_search(c).is_ok();
            out.changed += changed.iter().filter(|c| own.holds(**c)).count();
            for c in &changed {
                for cell in [*c, step(*c, Face::Up)] {
                    if !falls.contains_key(&cell) {
                        falls.insert(cell, wk);
                    }
                }
            }
            let mut asked: Vec<[i32; 3]> = Vec::with_capacity(changed.len() * FACES.len());
            for c in &changed {
                asked.extend(FACES.iter().map(|f| step(*c, *f)));
            }
            asked.sort_unstable();
            asked.dedup();
            asked.retain(|n| {
                !is_changed(n)
                    && match needs(sim.at(*n)) {
                        Some(Needs::Support(rules)) => {
                            rules.iter().any(|r| is_changed(&step(*n, r.cell)))
                        }
                        _ => false,
                    }
            });
            if template {
                asked.extend(changed.iter().copied());
                asked.sort_unstable();
                asked.dedup();
            }
            for cell in asked {
                let name = sim.at(cell);
                if !matches!(needs(name), Some(Needs::Support(_))) {
                    continue;
                }
                let mine = own.holds(cell);
                out.attached += usize::from(mine);
                let look = |c: [i32; 3]| sim.at(c);
                if let Some(Err(wants)) = judge_cell(cell, name, &look) {
                    if mine {
                        let placed = template && is_changed(&cell);
                        let why = wants
                            .iter()
                            .map(|(rule, [nx, ny, nz], there)| {
                                format!(
                                    "it needs the block {} at [{nx}, {ny}, {nz}] to {}, and \
                                     there is `{there}` when this write lands",
                                    beside(rule.cell),
                                    rule.describe()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; or ");
                        out.drops.push((
                            (lw.f, lw.q, cell),
                            SequenceDrop {
                                cell,
                                block: name.to_string(),
                                why: if placed {
                                    format!("the template's own placement removes it: {why}")
                                } else {
                                    format!("the server drops it as an item: {why}")
                                },
                                after: lw.write(),
                            },
                        ));
                    }
                    mark(&mut reads, cell, reads_of(name));
                    sim.cells.insert(cell, air);
                }
            }
        }
        for (cell, &wk) in falls.iter() {
            let name = sim.at(cell);
            if !crate::compiler::assembled::is_falling_block(name) {
                continue;
            }
            let mine = own.holds(cell);
            out.falling += usize::from(mine);
            let below = step(cell, Face::Down);
            let under = sim.at(below);
            if delvewright_dsl::support::falls_into(under) != Some(false) {
                if mine {
                    let [x, y, z] = below;
                    out.drops.push((
                        (f, usize::MAX, cell),
                        SequenceDrop {
                            cell,
                            block: name.to_string(),
                            why: format!(
                                "it falls: the cell below it, [{x}, {y}, {z}], still holds \
                                 `{under}` when `{}` has run, which a falling block falls into \
                                 — a write that lands under it later turns it into an item on \
                                 the ground",
                                functions[f]
                            ),
                            after: writes[wk as usize].write(),
                        },
                    ));
                }
                mark(&mut reads, cell, reads_of(name));
                sim.cells.insert(cell, air);
            }
        }
    }
    out.cells = sim.cells;
    out.reads = reads;
    out
}

/// Whether the reads from the cells of `own` that ever held a block that
/// reads, followed through every cell that ever held one, reach a cell
/// outside `reach` inside the box every write lies in (`all`): the cells a
/// replay over `reach` may answer differently from the whole replay.
fn reaches_past(
    reads: &crate::compiler::sections::SectionMap<u8>,
    own: Columns,
    reach: Columns,
    all: ([i32; 3], [i32; 3]),
) -> bool {
    let mut todo: Vec<[i32; 3]> = reads
        .in_box(
            [own.lo[0], i32::MIN, own.lo[1]],
            [own.hi[0], i32::MAX, own.hi[1]],
        )
        .filter(|(_, m)| **m != 0)
        .map(|(c, _)| c)
        .collect();
    let mut seen: BTreeSet<[i32; 3]> = todo.iter().copied().collect();
    while let Some(c) = todo.pop() {
        let m = reads.get(&c).copied().unwrap_or(0);
        for (bit, face) in FACES.iter().enumerate() {
            if m & (1 << bit) == 0 {
                continue;
            }
            let n = step(c, *face);
            if !reach.holds(n) {
                if (0..3).all(|a| all.0[a] <= n[a] && n[a] <= all.1[a]) {
                    return true;
                }
                continue;
            }
            if reads.get(&n).is_some_and(|m| *m != 0) && seen.insert(n) {
                todo.push(n);
            }
        }
    }
    false
}

/// The sequence arm as one replay of the whole load: the reference
/// [`judge_writes_over`]'s replay by chunk is held to
/// (`a_sequence_judged_by_chunk_is_the_one_judged_whole`).
#[cfg(test)]
fn judge_writes_whole(
    ns: &str,
    functions: &[(String, String)],
    templates: &TemplateIndex,
    structures: &BTreeMap<String, Vec<u8>>,
    surround: &Surround,
) -> Writes {
    use crate::compiler::blockstate::BlockState;
    let bodies: BTreeMap<&str, &str> = functions
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    let mut b = SequenceBinding {
        functions: load_order(ns, &bodies),
        ..SequenceBinding::default()
    };
    let mut sim = Sim {
        cells: BlockMap::new(),
        surround,
    };
    let air = BlockState::new("minecraft:air");
    let mut at_load: BTreeSet<String> = BTreeSet::new();
    // One tick per function of the load: a falling block written in it, or
    // over a cell it changes, is asked of the cell below once the function
    // has run — its scheduled tick comes after the function's other writes.
    for f in b.functions.clone() {
        let mut queue: Vec<(&str, usize, &str)> = Vec::new();
        expand(ns, &f, &bodies, &mut Vec::new(), &mut at_load, &mut queue);
        // The cells a falling block may stand in once the function has run,
        // each with the first write that changed it or the cell under it:
        // kept as the section store (ADR-0037 §1) over the writes the
        // function made, a byte per cell, never a tree entry carrying the
        // write's text.
        let mut falls: crate::compiler::sections::SectionMap<u32> =
            crate::compiler::sections::SectionMap::new();
        let mut fall_writes: Vec<SequenceWrite> = Vec::new();
        for (name, no, line) in queue {
            let write = SequenceWrite {
                at: format!("{name}:{no}"),
                command: line.to_string(),
            };
            let read = read_line(line, templates, structures);
            let (laid, template) = match read {
                Read::Other => continue,
                Read::Unread => {
                    b.unread.push(write);
                    continue;
                }
                Read::Write {
                    conditional: true, ..
                } => {
                    b.conditional += 1;
                    continue;
                }
                Read::Write { laid, kind, .. } => {
                    match kind {
                        "template" => b.templates += 1,
                        "fill" => b.fills += 1,
                        _ => b.setblocks += 1,
                    }
                    (laid, kind == "template")
                }
            };
            let mut changed: BTreeSet<[i32; 3]> = BTreeSet::new();
            for (c, st) in laid.shape.cells_within(None) {
                if laid.filter.lays_over(sim.at(c)) && sim.put(c, st) {
                    changed.insert(c);
                }
            }
            if changed.is_empty() {
                continue;
            }
            b.changed += changed.len();
            let wi = u32::try_from(fall_writes.len()).expect("fewer than 2^32 writes");
            let mut first_for_some = false;
            for c in &changed {
                for cell in [*c, step(*c, Face::Up)] {
                    if !falls.contains_key(&cell) {
                        falls.insert(cell, wi);
                        first_for_some = true;
                    }
                }
            }
            if first_for_some {
                fall_writes.push(write.clone());
            }
            // Who is asked `canSurvive` as the write lands (measured from the
            // pinned jar's bytes): a template asks every block it placed
            // (`StructureTemplate.placeInWorld` → `updateFromNeighbourShapes`)
            // and every neighbour its writes reach; a `fill` or `setblock`
            // keeps the block it places even where that block cannot survive
            // (`BlockInput.place` falls back to the state as written), and
            // asks only the neighbours it reaches.
            // A neighbour is asked only when the write changed a cell its rule
            // reads: `canSurvive` reads nothing else.
            let mut asked: BTreeSet<[i32; 3]> = BTreeSet::new();
            for c in &changed {
                asked.extend(FACES.iter().map(|f| step(*c, *f)));
            }
            asked.retain(|c| !changed.contains(c));
            asked.retain(|n| match needs(sim.at(*n)) {
                Some(Needs::Support(rules)) => {
                    rules.iter().any(|r| changed.contains(&step(*n, r.cell)))
                }
                _ => false,
            });
            if template {
                asked.extend(changed.iter().copied());
            }
            for cell in asked {
                let name = sim.at(cell);
                if !matches!(needs(name), Some(Needs::Support(_))) {
                    continue;
                }
                b.attached += 1;
                let look = |c: [i32; 3]| sim.at(c);
                if let Some(Err(wants)) = judge_cell(cell, name, &look) {
                    let placed = template && changed.contains(&cell);
                    let why = wants
                        .iter()
                        .map(|(rule, [nx, ny, nz], there)| {
                            format!(
                                "it needs the block {} at [{nx}, {ny}, {nz}] to {}, and there is \
                                 `{there}` when this write lands",
                                beside(rule.cell),
                                rule.describe()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; or ");
                    b.dropped.push(SequenceDrop {
                        cell,
                        block: name.to_string(),
                        why: if placed {
                            format!("the template's own placement removes it: {why}")
                        } else {
                            format!("the server drops it as an item: {why}")
                        },
                        after: write.clone(),
                    });
                    sim.cells.insert(cell, air);
                }
            }
        }
        for (cell, &wi) in falls.iter() {
            let name = sim.at(cell);
            if !crate::compiler::assembled::is_falling_block(name) {
                continue;
            }
            let write = fall_writes[wi as usize].clone();
            b.falling += 1;
            let below = step(cell, Face::Down);
            let under = sim.at(below);
            if delvewright_dsl::support::falls_into(under) != Some(false) {
                let [x, y, z] = below;
                b.dropped.push(SequenceDrop {
                    cell,
                    block: name.to_string(),
                    why: format!(
                        "it falls: the cell below it, [{x}, {y}, {z}], still holds `{under}` when \
                         `{f}` has run, which a falling block falls into — a write that lands \
                         under it later turns it into an item on the ground"
                    ),
                    after: write,
                });
                sim.cells.insert(cell, air);
            }
        }
    }
    let runtime = judge_runtime(ns, &bodies, &at_load, &sim, templates, structures);
    Writes {
        sequence: b,
        runtime,
        by_chunk: ByChunk::default(),
    }
}

/// **The runtime arm**: every block-writing line of every function the load
/// does not run, conditional or not, laid at once over the loaded world — a
/// cell may hold its loaded block or any block any of them leaves — and every
/// cell they reach, and its neighbours, judged against all of it. Every
/// configuration a path, a branch or an unforced beat can reach, in any order,
/// is one of these, so it can only refuse more.
fn judge_runtime(
    ns: &str,
    bodies: &BTreeMap<&str, &str>,
    at_load: &BTreeSet<String>,
    loaded: &Sim,
    templates: &TemplateIndex,
    structures: &BTreeMap<String, Vec<u8>>,
) -> RuntimeBinding {
    use crate::compiler::blockstate::BlockState;
    let mut r = RuntimeBinding::default();
    // Cell → every state a runtime write may leave there, with the write.
    let mut may: BTreeMap<[i32; 3], Vec<(BlockState, usize, Lays)>> = BTreeMap::new();
    let mut writes: Vec<SequenceWrite> = Vec::new();
    // Per write: the function it is a line of, and whether that function
    // always runs it whole once it runs it at all — unconditional, laid
    // everywhere, in a function with no `return` (the co-writes rule below).
    let mut whole: Vec<(usize, bool)> = Vec::new();
    for (f, (name, body)) in bodies.iter().enumerate() {
        if at_load.contains(*name) {
            continue;
        }
        let returns = body
            .lines()
            .any(|l| l.split_whitespace().any(|w| w == "return"));
        let mut writes_here = false;
        for (i, line) in body.lines().enumerate() {
            let write = SequenceWrite {
                at: format!("{name}:{}", i + 1),
                command: line.trim().to_string(),
            };
            match read_line(line.trim(), templates, structures) {
                Read::Other => {}
                Read::Unread => r.unread.push(write),
                Read::Write {
                    conditional, laid, ..
                } => {
                    writes_here = true;
                    r.writes += 1;
                    r.conditional += usize::from(conditional);
                    let k = writes.len();
                    writes.push(write);
                    whole.push((
                        f,
                        !returns && !conditional && laid.filter == Lays::Everywhere,
                    ));
                    for (c, s) in laid.shape.cells_within(None) {
                        may.entry(c).or_default().push((s, k, laid.filter.clone()));
                    }
                }
            }
        }
        r.functions += usize::from(writes_here);
    }
    let _ = ns;
    r.cells = may.len();
    // Every state a cell may hold, from `start`: each a write leaves where its
    // filter admits a state the cell may already hold, until none is added.
    // `skip` is the writes not to take: the one whose block is being judged,
    // and every write that ends that block's standing.
    let admitted = |c: [i32; 3],
                    mut v: Vec<(&'static str, Option<usize>)>,
                    skip: &BTreeSet<usize>|
     -> Vec<(&'static str, Option<usize>)> {
        let Some(e) = may.get(&c) else {
            return v;
        };
        loop {
            let mut grew = false;
            for (st, k, filter) in e {
                if skip.contains(k) || v.iter().any(|(_, o)| *o == Some(*k)) {
                    continue;
                }
                if v.iter().any(|(x, _)| filter.lays_over(x)) {
                    v.push((st.as_str(), Some(*k)));
                    grew = true;
                }
            }
            if !grew {
                return v;
            }
        }
    };
    // From its loaded block, taking every write but `skip`.
    let states_but =
        |c: [i32; 3], skip: &BTreeSet<usize>| admitted(c, vec![(loaded.at(c), None)], skip);
    let states = |c: [i32; 3]| states_but(c, &BTreeSet::new());
    // While a block write `origin` left beside it stands: where that one
    // command also wrote this cell, or a line of its own function that runs
    // whenever it runs (a co-write: unconditional, laid everywhere, in a
    // function with no `return` — one batch's cloth and the lantern hung
    // under it), from what the last of them in line order left there — never
    // what stood before the function ran. `ends` are the writes that end that
    // block's standing (below): what they leave is never beside it.
    let beside_states = |c: [i32; 3], origin: Option<usize>, ends: &BTreeSet<usize>| {
        let mut skip = ends.clone();
        skip.extend(origin);
        let own = origin.and_then(|k| {
            let f = whole[k].0;
            may.get(&c)?
                .iter()
                .filter(|(_, o, _)| {
                    *o == k || (whole[*o].0 == f && whole[*o].1 && !ends.contains(o))
                })
                .max_by_key(|(_, o, _)| *o)
                .map(|(st, o, _)| (st.as_str(), Some(*o)))
        });
        if let Some((_, Some(o))) = own {
            skip.insert(o);
        }
        match own {
            Some(first) => admitted(c, vec![first], &skip),
            None => states_but(c, ends),
        }
    };
    let by = |k: Option<usize>| -> String {
        match k {
            None => "at load".to_string(),
            Some(k) => format!("left by `{}` ({})", writes[k].command, writes[k].at),
        }
    };
    let mut around: BTreeSet<[i32; 3]> = BTreeSet::new();
    for c in may.keys() {
        around.insert(*c);
        around.extend(FACES.iter().map(|f| step(*c, *f)));
    }
    r.judged = around.len();
    for cell in around {
        for (name, origin) in states(cell) {
            if delvewright_dsl::blockshape::is_air(name) {
                continue;
            }
            r.pairs += 1;
            // **The writes that take a two-high block whole**: one command
            // that writes an upper half's cell over it and its lower half's
            // cell too (an `open-gate`'s clear of a door) removes the block,
            // and what it leaves below is never under this half. Whichever
            // cell it reaches first, the upper half goes without an item: the
            // pinned jar's loot tables give an upper half's item only through
            // its lower half, or only while its lower half stands under it, for
            // every block with a `half` of `lower`/`upper`
            // (`docs/reference/compiler.md`, `region_fill`). Only that block:
            // a torch whose floor one `fill` clears with it is shape-updated as
            // its floor goes, and drops.
            let below = step(cell, Face::Down);
            let upper_half = crate::compiler::region_fill::two_high(name)
                && delvewright_dsl::blockshape::state_value(name, "half") == Some("upper");
            let ends: BTreeSet<usize> = may
                .get(&cell)
                .into_iter()
                .flatten()
                .filter(|(_, k, filter)| {
                    upper_half
                        && Some(*k) != origin
                        && filter.lays_over(name)
                        && may
                            .get(&below)
                            .is_some_and(|e| e.iter().any(|(_, o, _)| o == k))
                })
                .map(|(_, k, _)| *k)
                .collect();
            let mut why: Option<String> = None;
            // `canSurvive` is asked only when a write changes a cell the rule
            // reads (a `fill` or `setblock` keeps the block it places): a block
            // whose rule cells no other write touches is never asked again.
            let asked = |rules: &[delvewright_dsl::support::Rule]| {
                rules.iter().any(|r| {
                    may.get(&step(cell, r.cell)).is_some_and(|e| {
                        e.iter()
                            .any(|(_, k, _)| Some(*k) != origin && !ends.contains(k))
                    })
                })
            };
            if let Some(Needs::Support(rules)) = needs(name)
                && asked(rules)
            {
                let held = rules.iter().any(|rule| {
                    beside_states(step(cell, rule.cell), origin, &ends)
                        .iter()
                        .all(|(n, _)| rule.holds(n))
                });
                if !held {
                    why = Some(
                        rules
                            .iter()
                            .map(|rule| {
                                let n = step(cell, rule.cell);
                                let [nx, ny, nz] = n;
                                let failing = beside_states(n, origin, &ends)
                                    .into_iter()
                                    .filter(|(s, _)| !rule.holds(s))
                                    .map(|(s, k)| format!("`{s}` {}", by(k)))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!(
                                    "it needs the block {} at [{nx}, {ny}, {nz}] to {}, and that \
                                     cell may hold {failing}",
                                    beside(rule.cell),
                                    rule.describe()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; or "),
                    );
                }
            }
            // A falling block a runtime write places asks the cell below as it
            // lands; one that stood at load, only once a write changes it.
            let below_written = may.get(&step(cell, Face::Down)).is_some_and(|e| {
                e.iter()
                    .any(|(_, k, _)| Some(*k) != origin && !ends.contains(k))
            });
            if crate::compiler::assembled::is_falling_block(name)
                && (origin.is_some() || below_written)
            {
                let below = step(cell, Face::Down);
                let free: Vec<String> = beside_states(below, origin, &ends)
                    .into_iter()
                    .filter(|(s, _)| delvewright_dsl::support::falls_into(s) != Some(false))
                    .map(|(s, k)| format!("`{s}` {}", by(k)))
                    .collect();
                if !free.is_empty() {
                    let [x, y, z] = below;
                    why = Some(format!(
                        "it falls: the cell below it, [{x}, {y}, {z}], may hold {}, which a \
                         falling block falls into",
                        free.join(", ")
                    ));
                }
            }
            if let Some(why) = why {
                let after = match origin {
                    Some(k) => writes[k].clone(),
                    None => SequenceWrite {
                        at: "at load".to_string(),
                        command: String::new(),
                    },
                };
                r.dropped.push(SequenceDrop {
                    cell,
                    block: name.to_string(),
                    why,
                    after,
                });
            }
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    fn void() -> Surround {
        Surround {
            ambient: Ambient::Void,
            built: Vec::new(),
        }
    }

    fn map(cells: &[([i32; 3], &str)]) -> BlockMap {
        cells
            .iter()
            .map(|(c, b)| (*c, BlockState::new(b)))
            .collect()
    }

    /// The finding (demo The Still Ones): a wall torch hung on a glass pane is
    /// dropped, and the refusal names the block, its cell, the face it needs
    /// and what stands there. `wall_torch[facing=north]` hangs on the block
    /// south of it.
    #[test]
    fn a_wall_torch_on_a_glass_pane_is_dw1002_naming_the_cell() {
        let b = judge_load(
            &map(&[
                ([4, 65, 7], "minecraft:wall_torch[facing=north]"),
                ([4, 65, 8], "minecraft:glass_pane"),
            ]),
            &void(),
        );
        assert_eq!((b.attached, b.held, b.dropped.len()), (1, 0, 1));
        let why = b.refusal().expect("refused");
        assert!(
            why.contains("`minecraft:wall_torch[facing=north]` at [4, 65, 7]"),
            "{why}"
        );
        assert!(why.contains("the block south of it at [4, 65, 8]"), "{why}");
        assert!(why.contains("a full (sturdy) north face"), "{why}");
        assert!(why.contains("there is `minecraft:glass_pane`"), "{why}");
        assert!(b.line().contains("1 of 2 non-air cell(s)"), "{}", b.line());
    }

    /// The same torch on stone is held, and nothing is refused.
    #[test]
    fn a_wall_torch_on_stone_is_held() {
        let b = judge_load(
            &map(&[
                ([4, 65, 7], "minecraft:wall_torch[facing=north]"),
                ([4, 65, 8], "minecraft:stone"),
            ]),
            &void(),
        );
        assert_eq!((b.attached, b.held), (1, 1));
        assert!(b.refusal().is_none());
    }

    /// A hanging lantern under air is dropped; under planks it is held.
    #[test]
    fn a_hanging_lantern_under_air_is_dw1002() {
        let lantern = "minecraft:lantern[hanging=true]";
        let b = judge_load(&map(&[([0, 70, 0], lantern)]), &void());
        let why = b.refusal().expect("refused");
        assert!(why.contains("the block above it at [0, 71, 0]"), "{why}");
        assert!(why.contains("there is `minecraft:air`"), "{why}");
        let b = judge_load(
            &map(&[([0, 70, 0], lantern), ([0, 71, 0], "minecraft:oak_planks")]),
            &void(),
        );
        assert!(b.refusal().is_none());
    }

    /// A torch on a slab, as the jar says: kept on a top slab, dropped on a
    /// bottom one (its top face is half a block below the cell's).
    #[test]
    fn a_torch_on_a_slab_is_judged_as_the_jar_judges_it() {
        let on = |slab: &str| {
            judge_load(
                &map(&[([0, 65, 0], "minecraft:torch"), ([0, 64, 0], slab)]),
                &void(),
            )
            .dropped
            .len()
        };
        assert_eq!(on("minecraft:oak_slab[type=top]"), 0);
        assert_eq!(on("minecraft:oak_slab[type=double]"), 0);
        assert_eq!(on("minecraft:oak_slab[type=bottom]"), 1);
    }

    /// Under the ocean horizon the generator's water and stone stand outside
    /// every piece: a lily pad on the open sea is held, one inside a piece's
    /// box (where `/place template` wrote air) is not.
    #[test]
    fn the_ocean_surround_holds_what_the_generator_puts_there() {
        let ocean = Surround {
            ambient: Ambient::Ocean(crate::compiler::nav::Sea {
                level: 62,
                floor_top: 54,
            }),
            built: vec![("prefab/box".to_string(), ([10, 50, 10], [20, 70, 20]))],
        };
        let pad = "minecraft:lily_pad";
        assert!(
            judge_load(&map(&[([0, 63, 0], pad)]), &ocean)
                .refusal()
                .is_none()
        );
        assert!(
            judge_load(&map(&[([15, 63, 15], pad)]), &ocean)
                .refusal()
                .is_some()
        );
    }

    /// A block the rule cannot judge is counted by block, never refused.
    #[test]
    fn an_unjudged_block_is_counted_not_refused() {
        let b = judge_load(&map(&[([0, 64, 0], "minecraft:wheat")]), &void());
        assert!(b.refusal().is_none());
        assert_eq!(b.unjudged.get("minecraft:wheat"), Some(&1));
        assert!(b.line().contains("minecraft:wheat x1"), "{}", b.line());
    }

    /// The shipped functions of a load: `setup`, the placement (`place`), the
    /// world build `place_verify` starts (`build`), `setup_finish`; and any
    /// runtime function after.
    fn load_with(
        place: &[&str],
        build: &[&str],
        runtime: &[(&str, &[&str])],
    ) -> Vec<(String, String)> {
        let mut f = vec![
            (
                "setup".to_string(),
                "scoreboard players set #placed dw.sys 0\n".to_string(),
            ),
            ("place_all".to_string(), place.join("\n")),
            (
                "place_verify".to_string(),
                "execute if score #placeok dw.sys matches 0 run function t:world_build\n"
                    .to_string(),
            ),
            (
                "world_build".to_string(),
                format!(
                    "{}\nschedule function t:setup_finish 1t\n",
                    build.join("\n")
                ),
            ),
            (
                "setup_finish".to_string(),
                "scoreboard players set #placed dw.sys 1\n".to_string(),
            ),
        ];
        for (n, lines) in runtime {
            f.push((n.to_string(), lines.join("\n")));
        }
        f
    }

    fn load(build: &[&str], runtime: &[(&str, &[&str])]) -> Vec<(String, String)> {
        load_with(&[], build, runtime)
    }

    fn writes(f: &[(String, String)]) -> Writes {
        judge_writes_over("t", f, &TemplateIndex::new(), &BTreeMap::new(), &void())
    }

    /// The Stranding's finding, reduced: gravel written over air by the
    /// placement, the ground under it written by the world build a tick or
    /// more later. The load ends with gravel on packed mud, which the load arm
    /// holds; the server let it fall as the placement's tick ended.
    #[test]
    fn gravel_written_before_its_ground_is_dw1002() {
        let f = load_with(
            &["setblock 0 65 0 minecraft:gravel"],
            &["fill 0 30 0 0 64 0 minecraft:packed_mud"],
            &[],
        );
        let w = writes(&f);
        assert_eq!(w.sequence.dropped.len(), 1, "{}", w.sequence.line());
        let why = w.sequence.refusal().expect("refused");
        assert!(why.contains("`minecraft:gravel` at [0, 65, 0]"), "{why}");
        assert!(why.contains("(place_all:1)"), "{why}");
        assert!(
            why.contains("still holds `minecraft:air` when `place_all` has run"),
            "{why}"
        );
        assert!(why.contains("By block: minecraft:gravel x1."), "{why}");
        // The ground first, or in the same function: held.
        for f in [
            load_with(
                &["fill 0 30 0 0 64 0 minecraft:packed_mud"],
                &["setblock 0 65 0 minecraft:gravel"],
                &[],
            ),
            load(
                &[
                    "setblock 0 65 0 minecraft:gravel",
                    "fill 0 30 0 0 64 0 minecraft:packed_mud",
                ],
                &[],
            ),
        ] {
            let w = writes(&f);
            assert!(w.sequence.refusal().is_none(), "{}", w.sequence.line());
            assert_eq!((w.sequence.fills, w.sequence.setblocks), (1, 1));
            assert_eq!(w.sequence.falling, 1);
        }
    }

    /// A `setblock` keeps the block it places even where it cannot survive
    /// (`BlockInput.place` falls back to the state as written), so a torch set
    /// before its wall is held once the wall's write reaches it; a write that
    /// reaches it while it still has no wall drops it.
    #[test]
    fn a_torch_set_before_its_wall_is_judged_when_a_write_reaches_it() {
        let torch = "setblock 4 65 7 minecraft:wall_torch[facing=north]";
        let wall = "setblock 4 65 8 minecraft:stone";
        let floor = "setblock 4 64 7 minecraft:stone";
        let pane = "setblock 4 65 8 minecraft:glass_pane";
        let held = |lines: &[&str]| writes(&load(lines, &[])).sequence.refusal().is_none();
        assert!(held(&[torch, wall]));
        assert!(held(&[wall, torch]));
        // A write beside it that its rule does not read leaves it be.
        assert!(held(&[torch, floor, wall]));
        let w = writes(&load(&[torch, pane, wall], &[]));
        let why = w.sequence.refusal().expect("refused");
        assert!(why.contains("drops it as an item"), "{why}");
        assert!(why.contains("(world_build:2)"), "{why}");
    }

    /// A later write that clears a support drops what hangs on it.
    #[test]
    fn a_load_write_that_clears_a_support_is_dw1002() {
        let w = writes(&load(
            &[
                "setblock 0 71 0 minecraft:oak_planks",
                "setblock 0 70 0 minecraft:lantern[hanging=true]",
                "fill 0 71 0 0 71 0 minecraft:air",
            ],
            &[],
        ));
        let why = w.sequence.refusal().expect("refused");
        assert!(
            why.contains("`minecraft:lantern[hanging=true]` at [0, 70, 0]"),
            "{why}"
        );
        assert!(why.contains("world_build:3"), "{why}");
    }

    /// The perturbation for the runtime arm, unforced: a write in a function
    /// the load never runs (a trap's payload, a beat nobody has to play) that
    /// clears the block a lantern hangs from. No configuration the critical
    /// path passes lays it; the runtime arm judges it anyway.
    #[test]
    fn an_unforced_write_that_clears_a_support_is_dw1002() {
        let f = load(
            &[
                "setblock 0 71 0 minecraft:oak_planks",
                "setblock 0 70 0 minecraft:lantern[hanging=true]",
            ],
            &[("trap_payload_plate", &["fill 0 71 0 0 71 0 minecraft:air"])],
        );
        let w = writes(&f);
        assert!(w.sequence.refusal().is_none(), "{}", w.sequence.line());
        assert_eq!(
            (w.runtime.writes, w.runtime.cells),
            (1, 1),
            "{}",
            w.runtime.line()
        );
        let why = w.runtime.refusal().expect("refused");
        assert!(
            why.contains("`minecraft:lantern[hanging=true]` at [0, 70, 0]"),
            "{why}"
        );
        assert!(
            why.contains(
                "`minecraft:air` left by `fill 0 71 0 0 71 0 minecraft:air` (trap_payload_plate:1)"
            ),
            "{why}"
        );
    }

    /// The perturbation for the runtime arm, branch-only: a write behind the
    /// branch's own condition, in a bundle only that branch plays, that takes
    /// the wall a torch hangs on. A branch's configurations were never walked;
    /// the runtime arm lays every write, conditional or not.
    #[test]
    fn a_branch_only_write_that_takes_a_support_is_dw1002() {
        let wall = "setblock 3 66 4 minecraft:stone";
        let torch = "setblock 3 66 3 minecraft:wall_torch[facing=north]";
        let branch = |line: &'static str| -> Vec<(String, String)> {
            load(&[wall, torch], &[("complete_o_take_the_bone", &[line])])
        };
        let w = writes(&branch(
            "execute if score #party dw.f_read_the_label matches 1 run setblock 3 66 4 minecraft:air",
        ));
        assert_eq!((w.runtime.writes, w.runtime.conditional), (1, 1));
        let why = w.runtime.refusal().expect("refused");
        assert!(
            why.contains("`minecraft:wall_torch[facing=north]` at [3, 66, 3] (at load)"),
            "{why}"
        );
        assert!(why.contains("(complete_o_take_the_bone:1)"), "{why}");
        // The same write beside the wall, not on it: held.
        let w = writes(&branch(
            "execute if score #party dw.f_read_the_label matches 1 run setblock 4 66 4 minecraft:air",
        ));
        assert!(w.runtime.refusal().is_none(), "{}", w.runtime.line());
    }

    /// A runtime `setblock` keeps the block it places where it cannot survive,
    /// and nothing asks it again while no write touches the cell its rule
    /// reads: a block hung on air by an effect is not dropped (it floats); the
    /// same block once a write reaches its support is.
    #[test]
    fn a_runtime_block_is_asked_only_when_its_support_is_written() {
        let hang = "setblock 5 66 5 minecraft:wall_torch[facing=north]";
        let w = writes(&load(&[], &[("prop", &[hang])]));
        assert!(w.runtime.refusal().is_none(), "{}", w.runtime.line());
        let w = writes(&load(
            &[],
            &[
                ("prop", &[hang]),
                ("later", &["setblock 5 66 6 minecraft:glass_pane"]),
            ],
        ));
        assert!(w.runtime.refusal().is_some(), "{}", w.runtime.line());
    }

    /// The town's night batch, reduced: one by-effect batch writes a row of
    /// cloth and hangs a lantern under it. The two writes are lines of one
    /// function that runs whole, so the lantern never stands under the air the
    /// cell held before the batch ran; held, in either line order. The same
    /// cloth written by another batch, by no batch, or behind a condition of
    /// its own leaves the air beside the lantern, and is refused.
    #[test]
    fn a_lantern_hung_under_cloth_its_own_batch_writes_is_held() {
        let cloth = "fill -4 71 0 4 71 0 minecraft:red_wool";
        let lamp = "setblock 2 70 0 minecraft:lantern[hanging=true,waterlogged=false]";
        let w = writes(&load(&[], &[("edit_batch_festival_night", &[cloth, lamp])]));
        assert!(w.runtime.refusal().is_none(), "{}", w.runtime.line());
        let w = writes(&load(&[], &[("edit_batch_festival_night", &[lamp, cloth])]));
        assert!(w.runtime.refusal().is_none(), "{}", w.runtime.line());
        // Another batch writes the cloth: the lantern's batch may run alone.
        let w = writes(&load(
            &[],
            &[
                ("edit_batch_festival_night", &[lamp]),
                ("edit_batch_bunting", &[cloth]),
            ],
        ));
        let why = w.runtime.refusal().expect("a support another batch writes");
        assert!(why.contains("`minecraft:air` at load"), "{why}");
        // No batch writes the cloth, and another clears the cell above.
        let w = writes(&load(
            &[],
            &[
                ("edit_batch_festival_night", &[lamp]),
                ("later", &["setblock 2 71 0 minecraft:air"]),
            ],
        ));
        assert!(w.runtime.refusal().is_some(), "{}", w.runtime.line());
        // The cloth behind its own condition: not run whenever the lantern is.
        let gated = "execute if score #party dw.f_night matches 1 run fill -4 71 0 4 71 0 minecraft:red_wool";
        let w = writes(&load(&[], &[("edit_batch_festival_night", &[gated, lamp])]));
        assert!(w.runtime.refusal().is_some(), "{}", w.runtime.line());
        // A batch function that may return before its cloth.
        let w = writes(&load(
            &[],
            &[(
                "edit_batch_festival_night",
                &[
                    lamp,
                    "execute if score #party dw.f_night matches 1 run return 0",
                    cloth,
                ],
            )],
        ));
        assert!(w.runtime.refusal().is_some(), "{}", w.runtime.line());
        // A later write of the same batch clears the cloth again.
        let w = writes(&load(
            &[],
            &[(
                "edit_batch_festival_night",
                &[cloth, lamp, "setblock 2 71 0 minecraft:air"],
            )],
        ));
        assert!(w.runtime.refusal().is_some(), "{}", w.runtime.line());
    }

    /// A door gate the story opens and shuts: its clear takes both halves in
    /// one command, so the upper half never stands over the air it leaves
    /// below; the close lays the lower half before the upper. A torch whose
    /// floor one `fill` clears with it is still dropped: the torch is
    /// shape-updated as its floor goes, and leaves its item.
    #[test]
    fn a_door_gate_cleared_whole_is_held_and_a_torch_cleared_with_its_floor_is_not() {
        let lower =
            "minecraft:iron_door[facing=south,half=lower,hinge=left,open=false,powered=false]";
        let upper =
            "minecraft:iron_door[facing=south,half=upper,hinge=left,open=false,powered=false]";
        let f = load(
            &[
                "fill 0 63 0 1 63 0 minecraft:stone",
                &format!("fill 0 64 0 1 64 0 {lower}"),
                &format!("fill 0 65 0 1 65 0 {upper}"),
            ],
            &[
                (
                    "open",
                    &["fill 0 64 0 1 65 0 minecraft:air replace minecraft:iron_door"],
                ),
                (
                    "close",
                    &[
                        &format!("fill 0 64 0 1 64 0 {lower}"),
                        &format!("fill 0 65 0 1 65 0 {upper}"),
                    ],
                ),
            ],
        );
        let w = writes(&f);
        assert!(w.runtime.refusal().is_none(), "{}", w.runtime.line());
        // The same clear over the lower course alone leaves the upper half
        // over air: refused.
        let f = load(
            &[
                "fill 0 63 0 1 63 0 minecraft:stone",
                &format!("fill 0 64 0 1 64 0 {lower}"),
                &format!("fill 0 65 0 1 65 0 {upper}"),
            ],
            &[(
                "open",
                &["fill 0 64 0 1 64 0 minecraft:air replace minecraft:iron_door"],
            )],
        );
        assert!(writes(&f).runtime.refusal().is_some());
        // A torch on a stone one `fill` clears with it.
        let f = load(
            &[
                "setblock 3 64 3 minecraft:stone",
                "setblock 3 65 3 minecraft:torch",
            ],
            &[("clear", &["fill 3 64 3 3 65 3 minecraft:air"])],
        );
        let why = writes(&f).runtime.refusal().expect("refused");
        assert!(why.contains("minecraft:torch"), "{why}");
    }

    /// A gate the story opens and shuts: a block hung on the gate's own cell
    /// is dropped in the configuration where the gate stands open.
    #[test]
    fn a_block_hung_on_a_gate_is_dw1002() {
        let f = load(
            &[
                "fill 0 64 0 0 66 0 minecraft:iron_bars",
                "setblock 1 66 0 minecraft:stone",
            ],
            &[
                (
                    "open_gate",
                    &["fill 0 64 0 0 66 0 minecraft:air replace minecraft:iron_bars"],
                ),
                ("close_gate", &["fill 0 64 0 0 66 0 minecraft:iron_bars"]),
                (
                    "hang",
                    &["setblock 0 67 0 minecraft:lantern[hanging=false]"],
                ),
            ],
        );
        let w = writes(&f);
        let why = w.runtime.refusal().expect("refused");
        assert!(
            why.contains("`minecraft:lantern[hanging=false]` at [0, 67, 0]"),
            "{why}"
        );
    }

    /// A write this reader cannot lay at load is refused by name, never
    /// skipped; one at runtime is counted and named in the binding.
    #[test]
    fn an_unread_write_is_named() {
        let w = writes(&load(
            &["clone 0 0 0 1 1 1 5 5 5"],
            &[("r", &["setblock ~ ~ ~ minecraft:stone"])],
        ));
        assert_eq!(w.sequence.unread.len(), 1);
        assert!(
            w.sequence
                .refusal()
                .expect("refused")
                .contains("clone 0 0 0 1 1 1 5 5 5")
        );
        assert_eq!(w.runtime.unread.len(), 1);
        assert!(
            w.runtime.line().contains("1 line(s) not read"),
            "{}",
            w.runtime.line()
        );
    }

    /// The ground phase is read as the load runs it: `ground_verify`'s target
    /// and the steps it schedules, before `place_all`. Gravel the placement
    /// writes over the ground is held.
    /// A template's gzipped bytes: each block at its local position (plain
    /// ids).
    fn structure(blocks: &[([i32; 3], &str)]) -> Vec<u8> {
        use fastnbt::Value;
        let mut names: Vec<&str> = Vec::new();
        for (_, n) in blocks {
            if !names.contains(n) {
                names.push(n);
            }
        }
        let compound = |pairs: Vec<(&str, Value)>| {
            Value::Compound(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
        };
        let palette = names
            .iter()
            .map(|n| compound(vec![("Name", Value::String(n.to_string()))]))
            .collect();
        let list = blocks
            .iter()
            .map(|(p, n)| {
                let i = names.iter().position(|m| m == n).expect("named");
                compound(vec![
                    (
                        "pos",
                        Value::List(p.iter().map(|&v| Value::Int(v)).collect()),
                    ),
                    ("state", Value::Int(i32::try_from(i).expect("small"))),
                ])
            })
            .collect();
        let root = compound(vec![
            ("palette", Value::List(palette)),
            ("blocks", Value::List(list)),
            (
                "size",
                Value::List(vec![Value::Int(3), Value::Int(3), Value::Int(3)]),
            ),
        ]);
        let raw = fastnbt::to_bytes(&root).expect("serialize NBT");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &raw).expect("gzip");
        gz.finish().expect("gzip finish")
    }

    /// One generated load: its functions, templates and structure files.
    type Case = (
        Vec<(String, String)>,
        TemplateIndex,
        BTreeMap<String, Vec<u8>>,
    );

    /// A load of `n` generated writes per function over a field crossing
    /// several chunk borders at negative and positive coordinates — boxes of
    /// solid, glass and air under every filter, attached and falling blocks,
    /// templates — and, at each side of the chunk borders at 0 and 64 on
    /// both axes, a chain of reads four cells long that leaves the chunk: a
    /// wall torch hung on sand standing on a wall torch hung on stone, the
    /// stone cleared by the world build, so the drop reaches back into the
    /// chunk two functions later through cells only a halo of two or more
    /// holds.
    fn generated(seed: u64, n: usize) -> Case {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = move |lo: i32, hi: i32| -> i32 {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            lo + i32::try_from((s >> 33) % u64::try_from(hi - lo + 1).expect("a range"))
                .expect("small")
        };
        let mut templates = TemplateIndex::new();
        let mut structures = BTreeMap::new();
        structures.insert(
            "a.nbt".to_string(),
            structure(&[
                ([0, 0, 0], "minecraft:stone"),
                ([0, 1, 0], "minecraft:torch"),
                ([1, 1, 0], "minecraft:sand"),
                ([2, 0, 1], "minecraft:stone"),
                ([2, 1, 1], "minecraft:lantern"),
                ([1, 0, 2], "minecraft:air"),
            ]),
        );
        structures.insert(
            "b.nbt".to_string(),
            structure(&[
                ([0, 0, 0], "minecraft:glass_pane"),
                ([0, 1, 0], "minecraft:rail"),
                ([1, 0, 0], "minecraft:gravel"),
                ([2, 2, 2], "minecraft:torch"),
            ]),
        );
        let solids = [
            "minecraft:stone",
            "minecraft:packed_mud",
            "minecraft:glass_pane",
            "minecraft:sand",
        ];
        let modes = [
            "",
            " keep",
            " replace minecraft:air",
            " replace minecraft:stone",
        ];
        let attached = [
            "minecraft:torch",
            "minecraft:wall_torch[facing=north]",
            "minecraft:wall_torch[facing=south]",
            "minecraft:wall_torch[facing=east]",
            "minecraft:wall_torch[facing=west]",
            "minecraft:lantern[hanging=true]",
            "minecraft:rail",
            "minecraft:sand",
            "minecraft:gravel",
            "minecraft:red_carpet",
        ];
        let mut lines = |count: usize, templates: &mut TemplateIndex| -> Vec<String> {
            (0..count)
                .map(|_| {
                    let (x, y, z) = (next(-50, 100), next(60, 68), next(-40, 90));
                    match next(0, 9) {
                        0..=2 => format!(
                            "fill {x} {y} {z} {} {} {} {}{}",
                            x + next(0, 8),
                            y + next(0, 2),
                            z + next(0, 8),
                            solids[next(0, 3) as usize],
                            modes[next(0, 3) as usize]
                        ),
                        3 => format!(
                            "fill {x} {y} {z} {} {} {} minecraft:air{}",
                            x + next(0, 8),
                            y + next(0, 2),
                            z + next(0, 8),
                            ["", " replace minecraft:stone"][next(0, 1) as usize]
                        ),
                        4..=7 => format!(
                            "setblock {x} {y} {z} {}{}",
                            attached[next(0, 9) as usize],
                            ["", " keep"][next(0, 1) as usize]
                        ),
                        8 => format!(
                            "setblock {x} {y} {z} {}",
                            ["minecraft:cave_air keep", "minecraft:stone"][next(0, 1) as usize]
                        ),
                        _ => {
                            let (id, file, rot) = if next(0, 1) == 0 {
                                ("t:a", "a.nbt", crate::compiler::solver::Rotation::None)
                            } else {
                                ("t:b", "b.nbt", crate::compiler::solver::Rotation::Cw90)
                            };
                            templates.insert((id.to_string(), [x, y, z]), (file.to_string(), rot));
                            format!("place template {id} {x} {y} {z}")
                        }
                    }
                })
                .collect()
        };
        let place = lines(n, &mut templates);
        let mut build = lines(n, &mut templates);
        build.push("function t:extra".to_string());
        let extra = lines(n / 4, &mut templates);
        let door = lines(6, &mut templates);
        let mut finish: Vec<String> = Vec::new();
        let (mut place_c, mut build_c) = (Vec::new(), Vec::new());
        // (anchor in the chunk, the step out of it, the torch's facing)
        for (a, d, facing) in [
            ([-1, 80, 20], [1, 0], "west"),
            ([0, 80, 25], [-1, 0], "east"),
            ([63, 80, 30], [1, 0], "west"),
            ([64, 80, 35], [-1, 0], "east"),
            ([10, 80, -1], [0, 1], "north"),
            ([15, 80, 0], [0, -1], "south"),
            ([40, 80, 63], [0, 1], "north"),
            ([45, 80, 64], [0, -1], "south"),
        ] {
            let at =
                |k: i32, dy: i32| format!("{} {} {}", a[0] + k * d[0], a[1] + dy, a[2] + k * d[1]);
            let torch = format!("minecraft:wall_torch[facing={facing}]");
            place_c.push(format!("setblock {} minecraft:stone", at(2, -1)));
            place_c.push(format!("setblock {} {torch}", at(1, -1)));
            place_c.push(format!("setblock {} minecraft:sand", at(1, 0)));
            place_c.push(format!("setblock {} {torch}", at(0, 0)));
            build_c.push(format!("setblock {} minecraft:air", at(2, -1)));
            build_c.push(format!("setblock {} minecraft:cave_air keep", at(1, -1)));
            finish.push(format!("setblock {} minecraft:cave_air keep", at(1, 0)));
        }
        let place: Vec<String> = place.into_iter().chain(place_c).collect();
        let build: Vec<String> = build.into_iter().chain(build_c).collect();
        let refs = |v: &[String]| v.iter().map(String::as_str).collect::<Vec<_>>().join("\n");
        let mut f = load_with(&[], &[], &[]);
        for (name, body) in &mut f {
            match name.as_str() {
                "place_all" => *body = refs(&place),
                "world_build" => {
                    *body = format!("{}\nschedule function t:setup_finish 1t\n", refs(&build));
                }
                "setup_finish" => {
                    *body = format!("scoreboard players set #placed dw.sys 1\n{}", refs(&finish));
                }
                _ => {}
            }
        }
        f.push(("extra".to_string(), refs(&extra)));
        f.push(("open_door".to_string(), refs(&door)));
        (f, templates, structures)
    }

    /// Everything both arms answer, as one string: a difference anywhere is a
    /// difference here.
    fn answer(w: &Writes) -> String {
        format!("{:#?}\n{:#?}", w.sequence, w.runtime)
    }

    /// **The replay by chunk answers what the whole replay answers** (ADR-0037
    /// §4): over generated loads under the void and the ocean horizons, at
    /// chunk sides of one, two and four section columns and first halos of
    /// one cell to sixteen, every verdict, count, drop and its order, and the
    /// runtime arm judged over the world the chunks leave, equal the one
    /// replay of the whole load, at one thread and with the chunks replayed
    /// in parallel on four. Refuses a zero count of drops, of chunks, of
    /// regrown halos and of the chains' drops.
    #[test]
    fn a_sequence_judged_by_chunk_is_the_one_judged_whole() {
        let ocean = || Surround {
            ambient: Ambient::Ocean(crate::compiler::nav::Sea {
                level: 62,
                floor_top: 54,
            }),
            built: Vec::new(),
        };
        let (mut cases, mut drops, mut chained, mut regrown, mut chunks, mut runtime) =
            (0, 0, 0, 0, 0, 0);
        for seed in 0..6u64 {
            let (f, templates, structures) = generated(seed, 400);
            for surround in [void(), ocean()] {
                let whole = judge_writes_whole("t", &f, &templates, &structures, &surround);
                let want = answer(&whole);
                drops += whole.sequence.dropped.len();
                chained += whole
                    .sequence
                    .dropped
                    .iter()
                    .filter(|d| d.cell[1] == 80 && d.after.at.starts_with("setup_finish"))
                    .count();
                runtime += whole.runtime.writes;
                for (side, halo) in [(1, 1), (1, 2), (2, 4), (4, 16), (CHUNK_SIDE, CHUNK_HALO)] {
                    let by =
                        judge_writes_by("t", &f, &templates, &structures, &surround, side, halo);
                    assert_eq!(
                        answer(&by),
                        want,
                        "seed {seed}, side {side}, halo {halo}: {}",
                        by.by_chunk.line()
                    );
                    // The chunks replayed in parallel fold to the same answer
                    // and the same binding line as at one thread.
                    let par = crate::par::with_threads(4, || {
                        judge_writes_by("t", &f, &templates, &structures, &surround, side, halo)
                    });
                    assert_eq!(
                        answer(&par),
                        want,
                        "seed {seed}, side {side}, halo {halo}, 4 threads"
                    );
                    assert_eq!(par.by_chunk.line(), by.by_chunk.line(), "4 threads");
                    if (side, halo) == (1, 1) {
                        regrown += by.by_chunk.regrown;
                        chunks += by.by_chunk.chunks;
                    }
                    assert!(!by.by_chunk.whole);
                    cases += 1;
                }
            }
        }
        eprintln!(
            "{cases} case(s): {drops} drop(s) judged whole, {chained} of the chains; {chunks} \
             chunk(s) and {regrown} regrown at side 1, halo 1; {runtime} runtime write(s)"
        );
        assert_eq!(cases, 60);
        assert_eq!(chained, 6 * 2 * 8, "every chain drops its torch: {chained}");
        assert!(drops > 500, "drops: {drops}");
        assert!(regrown >= 6 * 2 * 8, "regrown: {regrown}");
        assert!(chunks > 6 * 2 * 40, "chunks: {chunks}");
        assert!(runtime > 0, "runtime writes: {runtime}");
    }

    #[test]
    fn the_ground_steps_run_before_the_placement() {
        let mut f = load_with(&["setblock 0 65 0 minecraft:gravel"], &[], &[]);
        f.push((
            "ground_verify".to_string(),
            "execute if score #placeok dw.sys matches 0 run function t:world_ground\n".to_string(),
        ));
        f.push((
            "world_ground".to_string(),
            "fill 0 30 0 0 64 0 minecraft:packed_mud\nschedule function t:world_ground_2 1t\n"
                .to_string(),
        ));
        f.push((
            "world_ground_2".to_string(),
            "fill 1 30 0 1 64 0 minecraft:packed_mud\nscoreboard players set #placed dw.sys 0\n"
                .to_string(),
        ));
        let w = writes(&f);
        assert_eq!(
            w.sequence.functions,
            [
                "setup",
                "world_ground",
                "world_ground_2",
                "place_all",
                "world_build",
                "setup_finish"
            ]
        );
        assert!(w.sequence.refusal().is_none(), "{}", w.sequence.line());
        assert_eq!(w.runtime.writes, 0, "the ground is the load's, not play's");
    }
}
