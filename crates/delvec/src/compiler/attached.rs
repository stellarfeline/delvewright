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
struct Surround {
    ambient: Ambient,
    built: Vec<BuiltPiece>,
}

impl Surround {
    fn of(plan: &Plan) -> Surround {
        Surround {
            ambient: Ambient::of_plan(plan),
            built: crate::compiler::nav::built_volume(plan),
        }
    }

    fn inside(&self, c: [i32; 3]) -> Option<&str> {
        self.built
            .iter()
            .find(|(_, (lo, hi))| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]))
            .map(|(id, _)| id.as_str())
    }

    /// The block at an unwritten cell: air inside a piece's box (`/place
    /// template` writes the whole box) and under the void horizon; the ocean
    /// superflat's layers outside every box.
    fn unwritten(&self, c: [i32; 3]) -> &'static str {
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
    let surround = Surround::of(plan);
    let load = crate::compiler::view::beat::load_blocks(plan, world, base);
    judge_load(&load, &surround)
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
                match judge_cell(*cell, name, &at_load) {
                    Some(Ok(())) => b.held += 1,
                    Some(Err(wants)) => b.dropped.push(Dropped {
                        cell: *cell,
                        block: name.to_string(),
                        wants,
                        piece: surround.inside(*cell).map(str::to_string),
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
    cells: Vec<([i32; 3], crate::compiler::blockstate::BlockState)>,
    filter: Lays,
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
    let mut cells = Vec::new();
    let mut filter = Lays::Everywhere;
    let kind = match head {
        "place" if t.get(1).is_some_and(|w| w == "template") => {
            let pos = t.get(3..6).and_then(coords);
            let found = pos.and_then(|p| templates.get(&(t[2].clone(), p)));
            let Some((p, bytes, rot)) =
                found.and_then(|(file, r)| Some((pos?, structures.get(file)?, *r)))
            else {
                return Read::Unread;
            };
            cells.extend(
                crate::compiler::assembled::template_writes(rot, p, bytes)
                    .into_iter()
                    .map(|(c, s, _)| (c, s)),
            );
            "template"
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
            cells.extend(crate::compiler::assembled::region_cells(lo, hi).map(|c| (c, state)));
            "fill"
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
            cells.push((c, BlockState::new(&state_of(block))));
            "setblock"
        }
        "clone" => return Read::Unread,
        _ => return Read::Other,
    };
    Read::Write {
        conditional,
        laid: Laid { cells, filter },
        kind,
    }
}

/// A function's lines with the unconditional `function` calls in it inlined,
/// each tagged `<function>:<line>`; every function inlined is added to `seen`.
fn expand(
    ns: &str,
    name: &str,
    bodies: &BTreeMap<&str, &str>,
    stack: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<(String, String)>,
) {
    if stack.iter().any(|s| s == name) {
        return;
    }
    let Some(body) = bodies.get(name) else {
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
        out.push((format!("{name}:{}", i + 1), l.to_string()));
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

fn judge_writes_over(
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
        let mut queue: Vec<(String, String)> = Vec::new();
        expand(ns, &f, &bodies, &mut Vec::new(), &mut at_load, &mut queue);
        let mut falls: BTreeMap<[i32; 3], SequenceWrite> = BTreeMap::new();
        for (at, line) in queue {
            let write = SequenceWrite {
                at,
                command: line.clone(),
            };
            let read = read_line(&line, templates, structures);
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
            for (c, st) in laid.cells {
                if laid.filter.lays_over(sim.at(c)) && sim.put(c, st) {
                    changed.insert(c);
                }
            }
            if changed.is_empty() {
                continue;
            }
            b.changed += changed.len();
            for c in &changed {
                falls.entry(*c).or_insert_with(|| write.clone());
                falls
                    .entry(step(*c, Face::Up))
                    .or_insert_with(|| write.clone());
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
        for (cell, write) in falls {
            let name = sim.at(cell);
            if !crate::compiler::assembled::is_falling_block(name) {
                continue;
            }
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
    for (name, body) in bodies {
        if at_load.contains(*name) {
            continue;
        }
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
                    for (c, s) in laid.cells {
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
    // command also wrote this cell, from what it wrote there — never what
    // stood before it. `ends` are the writes that end that block's standing
    // (below): what they leave is never beside it.
    let beside_states = |c: [i32; 3], origin: Option<usize>, ends: &BTreeSet<usize>| {
        let mut skip = ends.clone();
        skip.extend(origin);
        let own = origin.and_then(|k| {
            may.get(&c)?
                .iter()
                .find(|(_, o, _)| *o == k)
                .map(|(st, _, _)| (st.as_str(), Some(k)))
        });
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
