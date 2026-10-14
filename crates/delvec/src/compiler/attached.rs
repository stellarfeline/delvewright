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
    /// `None` at load; the critical step a runtime configuration arrives at.
    pub after_step: Option<usize>,
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
    /// Runtime configurations judged, and the cells re-judged across them.
    pub configurations: usize,
    pub rejudged: usize,
    /// Every dropped block, load first, then by configuration.
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
             {} held, {} dropped; {} configuration(s) the critical path passes re-judged {} cell(s) \
             around their writes, {} dropped there; unjudged by the measured rule: {} cell(s) ({}); \
             not in the table: {} cell(s) ({})",
            self.attached,
            self.cells,
            self.held,
            self.dropped
                .iter()
                .filter(|d| d.after_step.is_none())
                .count(),
            self.configurations,
            self.rejudged,
            self.dropped
                .iter()
                .filter(|d| d.after_step.is_some())
                .count(),
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
    let when = match d.after_step {
        None => "at load".to_string(),
        Some(s) => format!("once the critical path reaches step {s}"),
    };
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
        "`{}` at [{x}, {y}, {z}]{piece} {when} needs {wants}",
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

/// **Judge the world the build writes** (module header): the load world, then
/// every configuration the critical path passes around its laid writes.
pub fn judge(plan: &Plan, world: &World, base: &BlockMap) -> Binding {
    let surround = Surround::of(plan);
    let load = crate::compiler::view::beat::load_blocks(plan, world, base);
    let mut b = judge_load(&load, &surround);
    let load_dropped: BTreeSet<[i32; 3]> = b.dropped.iter().map(|d| d.cell).collect();

    let (configs, _) = crate::compiler::nav::path_configurations(plan, world);
    for config in &configs {
        let over = config.laid();
        if over.is_empty() {
            continue;
        }
        b.configurations += 1;
        let at = |c: [i32; 3]| -> &str {
            match over.get(&c) {
                Some(Some(s)) => s,
                Some(None) => "minecraft:air",
                None => base
                    .get(&c)
                    .map(|s| &**s)
                    .unwrap_or_else(|| surround.unwritten(c)),
            }
        };
        let mut around: BTreeSet<[i32; 3]> = BTreeSet::new();
        for c in over.keys() {
            around.insert(*c);
            around.extend(FACES.iter().map(|f| step(*c, *f)));
        }
        for cell in around {
            let name = at(cell);
            if delvewright_dsl::blockshape::is_air(name) || load_dropped.contains(&cell) {
                continue;
            }
            b.rejudged += 1;
            if let Some(Err(wants)) = judge_cell(cell, name, &at)
                && !b.dropped.iter().any(|d| d.cell == cell && d.block == name)
            {
                b.dropped.push(Dropped {
                    cell,
                    block: name.to_string(),
                    wants,
                    piece: surround.inside(cell).map(str::to_string),
                    after_step: Some(config.step),
                });
            }
        }
    }
    b
}

/// The load half of [`judge`]: every non-air cell of `load`, its unwritten
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
                        after_step: None,
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
            next = bodies[n.as_str()]
                .lines()
                .find_map(|l| call_target(l, "schedule function "));
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

/// **Judge the load one write at a time**, as the server runs it (module
/// header): every function of the load in order, every block-writing line in
/// each, and after each write every changed cell and its neighbours.
pub fn judge_sequence(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
    functions: &[(String, String)],
) -> SequenceBinding {
    use crate::compiler::blockstate::BlockState;
    let ns = plan.namespace.as_str();
    let surround = Surround::of(plan);
    let bodies: BTreeMap<&str, &str> = functions
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    // `place template <ns:id> x y z [rotation]` → the template's bytes and the
    // piece's rotation, keyed as the command names them.
    let mut templates: BTreeMap<(String, [i32; 3]), (&str, crate::compiler::solver::Rotation)> =
        BTreeMap::new();
    for piece in plan.placed_pieces() {
        for t in &piece.templates {
            templates.insert(
                (format!("{ns}:{}", t.structure_id), t.pos),
                (t.structure_file.as_str(), piece.rotation),
            );
        }
    }
    let mut b = SequenceBinding {
        functions: load_order(ns, &bodies),
        ..SequenceBinding::default()
    };
    let mut sim = Sim {
        cells: BlockMap::new(),
        surround: &surround,
    };
    let air = BlockState::new("minecraft:air");
    // Each function, with the unconditional `function` calls in it inlined.
    let mut queue: Vec<(String, usize, String)> = Vec::new();
    fn expand(
        ns: &str,
        name: &str,
        bodies: &BTreeMap<&str, &str>,
        stack: &mut Vec<String>,
        out: &mut Vec<(String, usize, String)>,
    ) {
        if stack.iter().any(|s| s == name) {
            return;
        }
        let Some(body) = bodies.get(name) else {
            return;
        };
        stack.push(name.to_string());
        for (i, line) in body.lines().enumerate() {
            let l = line.trim();
            if let Some(callee) = l
                .strip_prefix("function ")
                .and_then(|r| r.split_whitespace().next())
                .and_then(|id| id.strip_prefix(&format!("{ns}:")))
            {
                expand(ns, callee, bodies, stack, out);
                continue;
            }
            out.push((name.to_string(), i + 1, l.to_string()));
        }
        stack.pop();
    }
    for f in b.functions.clone() {
        expand(ns, &f, &bodies, &mut Vec::new(), &mut queue);
    }
    for (func, lineno, line) in queue {
        let write = SequenceWrite {
            at: format!("{func}:{lineno}"),
            command: line.clone(),
        };
        let Ok((t, _)) = crate::compiler::commands::tokenize(&line) else {
            continue;
        };
        let Some(head) = t.first().map(String::as_str) else {
            continue;
        };
        if head == "execute" {
            if t.iter()
                .skip_while(|w| *w != "run")
                .nth(1)
                .is_some_and(|c| matches!(c.as_str(), "fill" | "setblock" | "place" | "clone"))
            {
                b.conditional += 1;
            }
            continue;
        }
        let mut changed: Vec<[i32; 3]> = Vec::new();
        match head {
            "place" if t.get(1).is_some_and(|w| w == "template") => {
                let pos = t.get(3..6).and_then(coords);
                let found = pos.and_then(|p| templates.get(&(t[2].clone(), p)));
                match (
                    pos,
                    found.and_then(|(file, r)| Some((structures.get(*file)?, *r))),
                ) {
                    (Some(p), Some((bytes, rot))) => {
                        b.templates += 1;
                        for (c, state, _) in
                            crate::compiler::assembled::template_writes(rot, p, bytes)
                        {
                            if sim.put(c, state) {
                                changed.push(c);
                            }
                        }
                    }
                    _ => b.unread.push(write.clone()),
                }
            }
            "fill" => {
                let (Some(a), Some(z), Some(block)) = (
                    t.get(1..4).and_then(coords),
                    t.get(4..7).and_then(coords),
                    t.get(7),
                ) else {
                    b.unread.push(write.clone());
                    continue;
                };
                let state = BlockState::new(&state_of(block));
                // `replace <filter>` lays only over the filter's block; `keep`
                // only over air; no mode, or `replace` alone, over everything.
                let filter = match t.get(8).map(String::as_str) {
                    None => Lays::Everywhere,
                    Some("replace") if t.get(9).is_some_and(|f| f.starts_with('#')) => {
                        b.unread.push(write.clone());
                        continue;
                    }
                    Some("replace") => t
                        .get(9)
                        .map_or(Lays::Everywhere, |f| Lays::Over(state_of(f))),
                    Some("keep") => Lays::OverAir,
                    Some(_) => {
                        b.unread.push(write.clone());
                        continue;
                    }
                };
                b.fills += 1;
                let lo = [0, 1, 2].map(|i| a[i].min(z[i]));
                let hi = [0, 1, 2].map(|i| a[i].max(z[i]));
                for c in crate::compiler::assembled::region_cells(lo, hi) {
                    if !filter.lays_over(sim.at(c)) {
                        continue;
                    }
                    if sim.put(c, state) {
                        changed.push(c);
                    }
                }
            }
            "setblock" => {
                let (Some(c), Some(block)) = (t.get(1..4).and_then(coords), t.get(4)) else {
                    b.unread.push(write.clone());
                    continue;
                };
                match t.get(5).map(String::as_str) {
                    None | Some("replace") => {}
                    Some("keep") if !delvewright_dsl::blockshape::is_air(sim.at(c)) => {
                        b.setblocks += 1;
                        continue;
                    }
                    Some("keep") => {}
                    Some(_) => {
                        b.unread.push(write.clone());
                        continue;
                    }
                }
                b.setblocks += 1;
                if sim.put(c, BlockState::new(&state_of(block))) {
                    changed.push(c);
                }
            }
            "clone" => {
                b.unread.push(write.clone());
                continue;
            }
            _ => continue,
        }
        if changed.is_empty() {
            continue;
        }
        b.changed += changed.len();
        let mut around: BTreeSet<[i32; 3]> = BTreeSet::new();
        for c in &changed {
            around.insert(*c);
            around.extend(FACES.iter().map(|f| step(*c, *f)));
        }
        for cell in around {
            let name = sim.at(cell);
            if delvewright_dsl::blockshape::is_air(name) {
                continue;
            }
            let mut why: Option<String> = None;
            if matches!(needs(name), Some(Needs::Support(_))) {
                b.attached += 1;
                let at = |c: [i32; 3]| sim.at(c);
                if let Some(Err(wants)) = judge_cell(cell, name, &at) {
                    why = Some(
                        wants
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
                            .join("; or "),
                    );
                }
            }
            if crate::compiler::assembled::is_falling_block(name) {
                b.falling += 1;
                let below = step(cell, Face::Down);
                let under = sim.at(below);
                if delvewright_dsl::support::falls_into(under) != Some(false) {
                    let [x, y, z] = below;
                    why = Some(format!(
                        "it falls: the cell below it, [{x}, {y}, {z}], holds `{under}` when this \
                         write lands, which a falling block falls into"
                    ));
                }
            }
            if let Some(why) = why {
                b.dropped.push(SequenceDrop {
                    cell,
                    block: name.to_string(),
                    why,
                    after: write.clone(),
                });
                sim.cells.insert(cell, air);
            }
        }
    }
    b
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
}
