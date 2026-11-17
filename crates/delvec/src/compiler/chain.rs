//! A tick's command chain stays under the game's limit (`DW0984`).
//!
//! The pinned game runs a function, and everything it calls in the same tick,
//! in one execution context with a command quota of the
//! `max_command_sequence_length` gamerule, default [`MAX_COMMAND_SEQUENCE_LENGTH`]
//! [cited — `GameRules.MAX_COMMAND_SEQUENCE_LENGTH`, read from the pinned server
//! jar's bytes]. When the quota runs out the server logs `Command execution
//! stopped due to limit` and drops the rest of the queue: the function stops
//! part-way, with no error any proof reads. Every root has its own context —
//! each `#minecraft:tick` / `#minecraft:load` entry, each scheduled function,
//! each advancement reward — and a `function` call runs its callee inside the
//! caller's.
//!
//! What costs a unit of quota [cited — `ExecutionContext.incrementCost`'s three
//! callers in the pinned bytes]: each `execute` sub-command stage that carries a
//! redirect modifier (`BuildContexts.execute`, once per stage), each command the
//! chain ends in (`ExecuteCommand.execute`, once per executing source), and each
//! function call (`CallFunction.execute`, once per executing source) — whose
//! body's commands then cost their own.
//!
//! [`measure`] reads the shipped tree and states, for every function, its
//! **chain**: one for the call, plus every line's cost, where a line costs its
//! `execute` stages once (every `execute` sub-command keyword before `run`, as
//! the pinned command tree names them — an upper bound, since a keyword can
//! also be an argument's literal), and its final command, or a called
//! function's chain, once per source its forks leave. The chain is measured at
//! a full party ([`PARTY`]): a fork over the players (`execute as @a …`) runs
//! its tail once per player, a fork over other entities once per entity its
//! selector's `limit` admits, and a fork whose selector states no bound (`@e`
//! with no `limit`, `on passengers`) once per source the game's fork limit
//! lets a stage leave ([`MOST_SOURCES`]). A function whose chain exceeds the
//! limit — or that reaches a call cycle no count bounds — refuses the build
//! ([`check`]); since a caller's chain contains its callee's, refusing every
//! function refuses every root.
//!
//! One cycle has a bound the bytes state: a **counted loop**, a function that
//! calls itself only under `if score <h> <o> matches ..<n>`, adds one to that
//! score unconditionally before the call and writes it nowhere else, and is
//! entered only from lines whose body last wrote the score with an
//! unconditional `scoreboard players set <h> <o> <k>`. It runs at most
//! `n - k + 1` times per entry ([`counted_loops`]), and its chain is that many
//! of its passes.
//!
//! The one-shot work that grows with the campaign — the world's block writes —
//! is the case this exists for. [`steps`] packs such a sequence into functions
//! each at most [`SETUP_STEP_BUDGET`], chained across ticks with `schedule
//! function`, so no tick runs more than one of them.
//!
//! The work every tick repeats cannot move to a later tick, and it grows with
//! the campaign too (its waves, actors, triggers, objectives). The quota is
//! read from the gamerule when a context is created
//! (`Commands.executeCommandInContext`), so the build states what its own
//! sources need: [`declare`] measures the delve's datapack and, when a source
//! outside `load` needs more than the default, writes `gamerule
//! max_command_sequence_length <its longest chain>` as the first line of the
//! delve's `load`. Every context the game creates after that line — each tick
//! entry, each scheduled function, each reward, each command — then has that
//! quota; the `load` context that runs the line, and everything it calls, does
//! not, and is held to the default. [`check`] reads the declaration back off
//! the shipped `load` and judges every function against the limit of the
//! context it runs in.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use delvewright_dsl::wave::MAX_PARTY;
use delvewright_dsl::{DwCode, ExitTier};

use crate::compiler::affordance::selector_terms;
use crate::compiler::commands::{CommandTree, tokenize};
use crate::compiler::emit::BuildOutput;

delvewright_dsl::dw_code! {
    /// `DW0984`: a shipped function's command chain — its own commands and
    /// every function it calls in the same tick — could exceed the
    /// `max_command_sequence_length` of the context it runs in (the default
    /// inside a `load`, the quota the build declares elsewhere), so the server
    /// would stop it part-way. An **engine self-check**: the emitter that wrote
    /// the function owes the bound.
    pub const DW_CHAIN_OVER_LIMIT: DwCode = DwCode::new("DW0984", ExitTier::Build);
}

/// The pinned game's default `max_command_sequence_length`: the command quota
/// of one execution context [cited — `GameRules.MAX_COMMAND_SEQUENCE_LENGTH`,
/// `ldc 65536`, in the pinned 1.21.11 server jar].
pub const MAX_COMMAND_SEQUENCE_LENGTH: u64 = 65_536;

/// The most quota one step of a split one-shot sequence may cost: a quarter of
/// the limit. The first step runs inside the tick that starts it, beside the
/// tick function's own commands; the rest is theirs, and [`check`] proves the
/// sum on every build.
pub const SETUP_STEP_BUDGET: u64 = MAX_COMMAND_SEQUENCE_LENGTH / 4;

/// The gamerule that sets one execution context's command quota.
pub const SEQUENCE_RULE: &str = "max_command_sequence_length";

/// The most a build may declare: the gamerule is an `int`.
const RULE_MAX: u64 = i32::MAX as u64;

/// The delve's own datapack, whose `load` carries the declaration.
const DELVE_PACK: &str = "datapack/";

/// The first world-build step's function name; step `k` (from 2) is
/// `world_build_<k>` ([`build_step_fn`]).
pub const BUILD_STEP_FN: &str = "world_build";

/// The function name of world-build step `k`, counted from 0.
pub fn build_step_fn(k: usize) -> String {
    if k == 0 {
        BUILD_STEP_FN.to_string()
    } else {
        format!("{BUILD_STEP_FN}_{}", k + 1)
    }
}

/// The first ground step's function name: the areas' own mass, written before
/// any template is placed; step `k` (from 2) is `world_ground_<k>`
/// ([`ground_step_fn`]).
pub const GROUND_STEP_FN: &str = "world_ground";

/// The ground's chunk gate: counts the chunks the areas' mass reaches, and
/// starts the first ground step once every one is loaded.
pub const GROUND_VERIFY_FN: &str = "ground_verify";

/// The function name of ground step `k`, counted from 0.
pub fn ground_step_fn(k: usize) -> String {
    if k == 0 {
        GROUND_STEP_FN.to_string()
    } else {
        format!("{GROUND_STEP_FN}_{}", k + 1)
    }
}

/// Is `name` (a function path) a step of the world's one-shot build — a
/// ground step or a world-build step?
fn is_build_step(name: &str) -> bool {
    [BUILD_STEP_FN, GROUND_STEP_FN].iter().any(|stem| {
        name == *stem
            || name
                .strip_prefix(stem)
                .and_then(|r| r.strip_prefix('_'))
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The `execute` sub-commands of the pinned command tree, `run` excluded: each
/// one before `run` is a stage that costs a unit.
static EXECUTE_STAGES: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    CommandTree::v1_21_11()
        .literals_under(&["execute"])
        .expect("the pinned command tree has `execute`")
        .into_iter()
        .filter(|l| *l != "run")
        .map(str::to_string)
        .collect()
});

/// The party every chain is measured at: the most players a delve is made
/// for ([`MAX_PARTY`]), so a quota measured here holds for every party.
pub const PARTY: u64 = MAX_PARTY as u64;

/// The pinned game's default `max_command_forks`: a stage that would leave
/// this many sources or more stops the line with `command.forkLimit` instead
/// [cited — `BuildContexts.execute` compares the sources gathered so far plus
/// the next source's against `ExecutionContext.forkLimit` before it adds them;
/// `GameRules.MAX_COMMAND_FORKS`, `ldc 65536`].
pub const MAX_COMMAND_FORKS: u64 = 65_536;

/// The most sources any `execute` stage leaves: one fewer than the fork
/// limit, whatever the selectors answer with.
pub const MOST_SOURCES: u64 = MAX_COMMAND_FORKS - 1;

/// How many executing sources a forking `execute` stage leaves for the rest
/// of the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fork {
    /// Over the players: never more than the party, and never more than the
    /// selector's `limit` when it states one.
    Players {
        /// The selector's `limit` (`@p` and `@r` default to one).
        limit: Option<u64>,
    },
    /// Over at most this many entities, whatever the party: a selector's
    /// stated `limit`.
    AtMost(u64),
    /// Over a count no reading of the bytes bounds, with why: the game's
    /// fork limit bounds it ([`MOST_SOURCES`]).
    AtTheForkLimit(String),
}

impl Fork {
    /// The sources the fork makes of one source at a party of `party`.
    pub fn at(&self, party: u64) -> u64 {
        match self {
            Fork::Players { limit } => limit.map_or(party, |l| l.min(party)),
            Fork::AtMost(n) => *n,
            Fork::AtTheForkLimit(_) => MOST_SOURCES,
        }
    }
}

/// The fork an entity argument makes: a name or a UUID is one entity, `@s`
/// the source itself, a player selector at most the party, any other
/// selector its stated `limit`, and a selector that states none the game's
/// fork limit.
pub fn selector_fork(token: &str) -> Option<Fork> {
    if token.contains("$(") {
        return Some(Fork::AtTheForkLimit(format!(
            "`{token}` is a macro argument, which names its entities at run time"
        )));
    }
    let Some(kind) = token.get(..2).filter(|k| k.starts_with('@')) else {
        // A player name or a UUID: one entity.
        return None;
    };
    let args = token[2..]
        .strip_prefix('[')
        .and_then(|a| a.strip_suffix(']'))
        .unwrap_or("");
    let limit = selector_terms(args)
        .find_map(|t| t.strip_prefix("limit="))
        .and_then(|n| n.parse::<u64>().ok());
    let player = selector_terms(args).any(|t| t == "type=player" || t == "type=minecraft:player");
    let fork = match kind {
        "@s" => return None,
        "@p" | "@r" => Fork::Players {
            limit: Some(limit.unwrap_or(1)),
        },
        "@a" => Fork::Players { limit },
        "@n" => Fork::AtMost(limit.unwrap_or(1)),
        "@e" if player => Fork::Players { limit },
        "@e" => match limit {
            Some(n) => Fork::AtMost(n),
            None => Fork::AtTheForkLimit(format!(
                "`{token}` states no `limit` and no player `type`, so any number of entities may \
                 answer it"
            )),
        },
        _ => Fork::AtTheForkLimit(format!("`{token}` is no selector the pinned game knows")),
    };
    (fork != Fork::AtMost(1)).then_some(fork)
}

/// The forks the `execute` sub-commands `subs` make, in order: `as`, `at`,
/// `positioned as`, `rotated as` and `facing entity` fork over their entity
/// argument, `on passengers` over every passenger; every other stage keeps
/// one source per source.
fn forks_of(subs: &[String]) -> Vec<Fork> {
    let mut forks = Vec::new();
    let mut j = 0usize;
    let at = |k: usize| subs.get(k).map(String::as_str);
    while j < subs.len() {
        let (arg, step) = match (at(j), at(j + 1)) {
            (Some("positioned" | "rotated"), Some("as")) | (Some("facing"), Some("entity")) => {
                (at(j + 2), 3)
            }
            (Some("as" | "at"), _) => (at(j + 1), 2),
            (Some("on"), Some("passengers")) => {
                forks.push(Fork::AtTheForkLimit(
                    "`on passengers` forks over every passenger, which no reading bounds"
                        .to_string(),
                ));
                j += 2;
                continue;
            }
            _ => (None, 1),
        };
        if let Some(fork) = arg.and_then(selector_fork) {
            forks.push(fork);
        }
        j += step;
    }
    forks
}

/// One command line's cost: the quota its `execute` stages take, the forks
/// they make, and the function the line calls, if it ends in a call.
///
/// The stages are charged once whatever they fork into
/// (`BuildContexts.execute`); the command the line ends in, or the call, is
/// charged once per source the forks leave (`ExecuteCommand.execute`,
/// `CallFunction.execute`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineCost {
    /// Quota the line's `execute` stages take, once.
    pub stages: u64,
    /// The forks its stages make, in order.
    pub forks: Vec<Fork>,
    /// The function the line calls — `ns:path`, or `#ns:path` for a tag.
    pub calls: Option<String>,
}

impl LineCost {
    /// The sources the line's tail runs for at a party of `party`: the
    /// forks multiplied, and never more than the game's fork limit leaves
    /// after any stage.
    pub fn sources(&self, party: u64) -> u64 {
        self.forks
            .iter()
            .fold(1u64, |n, f| n.saturating_mul(f.at(party)).min(MOST_SOURCES))
    }

    /// The quota the line spends itself at a party of `party` — its stages,
    /// and its final command once per source — a call's callee excepted.
    pub fn own(&self, party: u64) -> u64 {
        self.stages.saturating_add(self.sources(party))
    }

    /// Why the line's count is bounded by the game's fork limit alone, when
    /// it is.
    pub fn at_the_fork_limit(&self) -> Option<&str> {
        self.forks.iter().find_map(|f| match f {
            Fork::AtTheForkLimit(why) => Some(why.as_str()),
            _ => None,
        })
    }
}

/// The cost of one line; `None` for a blank line or a comment.
pub fn line_cost(line: &str) -> Option<LineCost> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    let tokens = tokens_of(t);
    let mut stages = 0u64;
    let mut forks = Vec::new();
    let mut i = 0usize;
    loop {
        match tokens.get(i).map(String::as_str) {
            Some("execute") => {
                let run = tokens[i + 1..].iter().position(|t| t == "run");
                let end = run.map_or(tokens.len(), |r| i + 1 + r);
                let subs = tokens[i + 1..end]
                    .iter()
                    .filter(|t| EXECUTE_STAGES.contains(t.as_str()))
                    .count() as u64;
                forks.extend(forks_of(&tokens[i + 1..end]));
                match run {
                    Some(_) => {
                        stages += subs;
                        i = end + 1;
                    }
                    // A terminal `execute if …`: its last stage is the
                    // command itself, run once per source.
                    None => {
                        return Some(LineCost {
                            stages: stages + subs.saturating_sub(1),
                            forks,
                            calls: None,
                        });
                    }
                }
            }
            Some("return") if tokens.get(i + 1).map(String::as_str) == Some("run") => {
                stages += 1;
                i += 2;
            }
            _ => break,
        }
    }
    let calls = match tokens.get(i).map(String::as_str) {
        Some("function") => tokens.get(i + 1).cloned(),
        _ => None,
    };
    Some(LineCost {
        stages,
        forks,
        calls,
    })
}

/// Pack a one-shot sequence of lines that call no function into steps, each
/// costing at most `budget`, in order. An empty sequence is no steps.
pub fn steps(lines: &[String], budget: u64) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut spent = 0u64;
    for line in lines {
        let cost = line_cost(line).map_or(0, |c| {
            debug_assert!(c.calls.is_none(), "a packed line calls nothing: {line}");
            c.own(PARTY)
        });
        if !cur.is_empty() && spent + cost > budget {
            out.push(std::mem::take(&mut cur));
            spent = 0;
        }
        cur.push(line.clone());
        spent += cost;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A function's measured chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chain {
    /// The quota one call spends, from one executing source, at the party
    /// measured.
    Bounded(u64),
    /// The function reaches itself through calls in the same tick.
    Recursive,
}

impl Chain {
    fn bounded(&self) -> Option<u64> {
        match self {
            Chain::Bounded(n) => Some(*n),
            _ => None,
        }
    }
}

/// What [`measure`] read and found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChainLedger {
    /// Shipped functions measured.
    pub functions: usize,
    /// Of those, the counted loops: self-calls the bytes bound.
    pub counted_loops: usize,
    /// Of those, the world-build steps the delve's own datapack ships.
    pub build_steps: usize,
    /// The party every chain is measured at ([`PARTY`]).
    pub party: u64,
    /// Every function's chain at that party, by `ns:path`.
    pub chains: BTreeMap<String, Chain>,
    /// The longest bounded chain at one player, for the binding to set
    /// beside the party's.
    pub solo_longest: Option<(String, u64)>,
    /// The lines whose fork only the game's fork limit bounds, as
    /// `(function, why)`, in the functions' order.
    pub fork_limited: Vec<(String, String)>,
    /// The quota the delve's `load` declares for every later context
    /// (`gamerule max_command_sequence_length <n>`), if it declares one.
    pub declared: Option<u64>,
    /// The functions a `#minecraft:load` entry runs in its own context, with
    /// everything they call in it: held to the limit that stands before the
    /// declaration runs.
    pub load_rooted: BTreeSet<String>,
}

impl ChainLedger {
    /// The longest bounded chain: `(function, chain)`.
    pub fn longest(&self) -> Option<(&str, u64)> {
        longest(&self.chains)
    }

    /// The quota of every context created after the delve's `load` ran.
    pub fn limit(&self) -> u64 {
        self.declared.unwrap_or(MAX_COMMAND_SEQUENCE_LENGTH)
    }

    /// The quota of the context `f` runs in: a `load` context's stands before
    /// the declaration runs, so it is the default (or a lower declaration);
    /// every other context is created after it.
    pub fn limit_of(&self, f: &str) -> u64 {
        if self.load_rooted.contains(f) {
            self.limit().min(MAX_COMMAND_SEQUENCE_LENGTH)
        } else {
            self.limit()
        }
    }

    /// The quota the tree's sources need: the longest bounded chain of a
    /// function any of its datapacks ships that no `load` runs — the
    /// creator overlay and the PackTest pack run beside the delve's, under
    /// the quota the delve's `load` declares — or the default when that is
    /// more.
    pub fn need(&self) -> u64 {
        self.chains
            .iter()
            .filter(|(f, _)| !self.load_rooted.contains(*f))
            .filter_map(|(_, c)| c.bounded())
            .fold(MAX_COMMAND_SEQUENCE_LENGTH, u64::max)
    }

    /// The functions that refuse: over the limit of the context they run in,
    /// recursive, or forking over a count nothing bounds.
    pub fn refused(&self) -> Vec<(&str, &Chain)> {
        self.chains
            .iter()
            .filter(|(f, c)| match c {
                Chain::Bounded(n) => *n > self.limit_of(f),
                Chain::Recursive => true,
            })
            .map(|(f, c)| (f.as_str(), c))
            .collect()
    }

    /// The ledger written to `validation/chain-length.json`.
    pub fn to_json(&self) -> serde_json::Value {
        let longest = self.longest();
        serde_json::json!({
            "limit": MAX_COMMAND_SEQUENCE_LENGTH,
            "declared_limit": self.declared,
            "load_rooted": self.load_rooted.len(),
            "step_budget": SETUP_STEP_BUDGET,
            "examined": self.functions,
            "counted_loops": self.counted_loops,
            "steps": self.build_steps,
            "party": self.party,
            "longest": longest.map(|(f, n)| serde_json::json!({ "function": f, "chain": n })),
            "longest_solo": self.solo_longest.as_ref().map(|(f, n)| serde_json::json!({ "function": f, "chain": n })),
            "refused": self.refused().iter().map(|(f, c)| serde_json::json!({
                "function": f,
                "chain": match c {
                    Chain::Bounded(n) => serde_json::json!(n),
                    Chain::Recursive => serde_json::json!("recursive"),
                },
            })).collect::<Vec<_>>(),
            "at_the_fork_limit": self.fork_limited.len(),
            "fork_limit": MAX_COMMAND_FORKS,
            "quantifier": "per root, at a full party: a fork over the players multiplies its tail by the party, a fork over other entities by its selector's limit, or by the game's fork limit when it states none",
        })
    }

    /// The one-line binding statement a build prints.
    pub fn binding(&self) -> String {
        let (f, n) = self.longest().unwrap_or(("none", 0));
        let (sf, sn) = self
            .solo_longest
            .as_ref()
            .map_or(("none", 0), |(f, n)| (f.as_str(), *n));
        let limit = match self.declared {
            Some(d) => format!(
                "a limit of {d} the delve's `load` declares (`gamerule {SEQUENCE_RULE}`; \
                 {MAX_COMMAND_SEQUENCE_LENGTH} by default)"
            ),
            None => format!("the game's default limit of {MAX_COMMAND_SEQUENCE_LENGTH}"),
        };
        format!(
            "chain binding: {} shipped function(s) measured, {} counted loop(s), {} world-build \
             step(s); longest chain {n} (`{f}`) at a party of {} ({sn} (`{sf}`) at one player), \
             against {limit}, and {} function(s) a `load` runs against {}; {} line(s) fork over \
             entities only the game's fork limit of {MAX_COMMAND_FORKS} bounds; {} refused \
             (DW0984).",
            self.functions,
            self.counted_loops,
            self.build_steps,
            self.party,
            self.load_rooted.len(),
            self.limit().min(MAX_COMMAND_SEQUENCE_LENGTH),
            self.fork_limited.len(),
            self.refused().len()
        )
    }
}

/// The longest bounded chain of `chains`: `(function, chain)`.
fn longest(chains: &BTreeMap<String, Chain>) -> Option<(&str, u64)> {
    chains
        .iter()
        .filter_map(|(f, c)| c.bounded().map(|n| (f.as_str(), n)))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
}

/// `<root>/data/<ns>/function/<path>.mcfunction` → `ns:path`.
fn function_id(path: &str) -> Option<String> {
    let (_, rest) = path.split_once("/data/")?;
    let (ns, rest) = rest.split_once("/function/")?;
    let name = rest.strip_suffix(".mcfunction")?;
    (!ns.contains('/')).then(|| format!("{ns}:{name}"))
}

/// `<root>/data/<ns>/tags/function/<path>.json` → `#ns:path`.
fn tag_id(path: &str) -> Option<String> {
    let (_, rest) = path.split_once("/data/")?;
    let (ns, rest) = rest.split_once("/tags/function/")?;
    let name = rest.strip_suffix(".json")?;
    (!ns.contains('/')).then(|| format!("#{ns}:{name}"))
}

/// A line's tokens, as the command validator splits them.
fn tokens_of(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('$').unwrap_or(t);
    let t = t.strip_prefix('/').unwrap_or(t);
    match tokenize(t) {
        Ok((tokens, _)) => tokens,
        Err(_) => t.split_whitespace().map(str::to_string).collect(),
    }
}

/// One measured line: its cost and its tokens.
struct Line {
    cost: LineCost,
    tokens: Vec<String>,
}

/// Does the line name the score `h o` anywhere?
fn touches(tokens: &[String], h: &str, o: &str) -> bool {
    tokens.windows(2).any(|w| w[0] == h && w[1] == o)
}

/// The counted loops of a tree, each with the most passes one entry runs: see
/// the module docs. A self-calling function that does not have every part of
/// the shape is not one, and stays a cycle no count bounds.
fn counted_loops(bodies: &BTreeMap<String, Vec<Line>>) -> BTreeMap<String, u64> {
    let mut loops = BTreeMap::new();
    for (id, body) in bodies {
        let self_calls: Vec<usize> = (0..body.len())
            .filter(|&i| body[i].cost.calls.as_deref() == Some(id.as_str()))
            .collect();
        let Some(&first_call) = self_calls.first() else {
            continue;
        };
        // The guard: `if score <h> <o> matches ..<n>`, the same on every self call.
        let guard = |t: &[String]| -> Option<(String, String, i64)> {
            let run = t.iter().position(|x| x == "run")?;
            t[..run].windows(6).find_map(|w| {
                (w[0] == "if" && w[1] == "score" && w[4] == "matches")
                    .then(|| w[5].strip_prefix(".."))
                    .flatten()
                    .and_then(|n| n.parse::<i64>().ok())
                    .map(|n| (w[2].clone(), w[3].clone(), n))
            })
        };
        let Some((h, o, n)) = guard(&body[first_call].tokens) else {
            continue;
        };
        if self_calls
            .iter()
            .any(|&i| guard(&body[i].tokens) != Some((h.clone(), o.clone(), n)))
        {
            continue;
        }
        // The count: one unconditional `add <h> <o> 1` before the first call,
        // and no other line of the body writes the score.
        let is_add = |t: &[String]| {
            t.len() == 6
                && t[..3] == ["scoreboard", "players", "add"]
                && t[3] == h
                && t[4] == o
                && t[5] == "1"
        };
        let adds: Vec<usize> = (0..body.len())
            .filter(|&i| is_add(&body[i].tokens))
            .collect();
        if adds.len() != 1 || adds[0] > first_call {
            continue;
        }
        if (0..body.len()).any(|i| {
            touches(&body[i].tokens, &h, &o) && !is_add(&body[i].tokens) && !self_calls.contains(&i)
        }) {
            continue;
        }
        // Every entry from outside sets the score first, to a number.
        let mut start: Option<i64> = None;
        let mut entered_bounded = true;
        for (caller, cbody) in bodies {
            if caller == id {
                continue;
            }
            for (i, line) in cbody.iter().enumerate() {
                if line.cost.calls.as_deref() != Some(id.as_str()) {
                    continue;
                }
                let last = cbody[..i]
                    .iter()
                    .rev()
                    .find(|l| touches(&l.tokens, &h, &o))
                    .map(|l| &l.tokens);
                let k = last.and_then(|t| {
                    (t.len() == 6
                        && t[..3] == ["scoreboard", "players", "set"]
                        && t[3] == h
                        && t[4] == o)
                        .then(|| t[5].parse::<i64>().ok())
                        .flatten()
                });
                match k {
                    Some(k) => start = Some(start.map_or(k, |s| s.min(k))),
                    None => entered_bounded = false,
                }
            }
        }
        if let (true, Some(k)) = (entered_bounded, start) {
            loops.insert(id.clone(), u64::try_from((n - k).max(0)).unwrap_or(0) + 1);
        }
    }
    loops
}

/// Measure every function every datapack of the tree ships — the delve's
/// own, the creator overlay, the PackTest pack — against one name space, as
/// the server loads them side by side, at a full party ([`PARTY`]).
pub fn measure(out: &BuildOutput) -> ChainLedger {
    let mut l = measure_at(out, PARTY);
    l.solo_longest = longest(&measure_at(out, 1).chains).map(|(f, n)| (f.to_string(), n));
    l
}

/// [`measure`] at a party of `party` players.
pub fn measure_at(out: &BuildOutput, party: u64) -> ChainLedger {
    let mut bodies: BTreeMap<String, Vec<Line>> = BTreeMap::new();
    let mut tags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, bytes) in out {
        if let Some(id) = function_id(path)
            && let Ok(body) = std::str::from_utf8(bytes)
        {
            bodies
                .entry(id)
                .or_default()
                .extend(body.lines().filter_map(|l| {
                    line_cost(l).map(|cost| Line {
                        cost,
                        tokens: tokens_of(l),
                    })
                }));
        } else if let Some(id) = tag_id(path)
            && let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes)
        {
            let values = v["values"].as_array().cloned().unwrap_or_default();
            tags.entry(id).or_default().extend(
                values
                    .iter()
                    .filter_map(|e| e.as_str().or_else(|| e["id"].as_str()))
                    .map(str::to_string),
            );
        }
    }
    let build_steps = out
        .keys()
        .filter(|p| p.starts_with("datapack/"))
        .filter_map(|p| function_id(p))
        .filter(|id| id.split_once(':').is_some_and(|(_, n)| is_build_step(n)))
        .count();
    let loops = counted_loops(&bodies);
    let mut m = Measurer {
        bodies: &bodies,
        tags: &tags,
        loops: &loops,
        party,
        chains: BTreeMap::new(),
        on_stack: BTreeSet::new(),
    };
    for id in bodies.keys() {
        m.chain_of(id);
    }
    ChainLedger {
        functions: bodies.len(),
        counted_loops: loops.len(),
        build_steps,
        party,
        chains: m.chains,
        solo_longest: None,
        fork_limited: bodies
            .iter()
            .flat_map(|(f, body)| {
                body.iter().filter_map(move |l| {
                    l.cost
                        .at_the_fork_limit()
                        .map(|why| (f.clone(), why.to_string()))
                })
            })
            .collect(),
        declared: declared(out, &bodies),
        load_rooted: load_rooted(&bodies, &tags),
    }
}

/// The path of the function that carries the declaration: the first the
/// delve's `#minecraft:load` tag names.
pub fn load_path(out: &BuildOutput) -> Option<String> {
    let (ns, name) = delve_load(out)
        .first()?
        .split_once(':')
        .map(|(a, b)| (a.to_string(), b.to_string()))?;
    Some(format!("{DELVE_PACK}data/{ns}/function/{name}.mcfunction"))
}

/// The quota `line` states, when it is the unconditional `gamerule
/// max_command_sequence_length <n>` [`declare`] writes.
fn stated_quota(line: &str) -> Option<u64> {
    let mut words = line.split(' ');
    match (words.next(), words.next(), words.next(), words.next()) {
        (Some("gamerule"), Some(rule), Some(n), None) if rule == SEQUENCE_RULE => n.parse().ok(),
        _ => None,
    }
}

/// The functions the delve's own `#minecraft:load` tag runs, in order.
fn delve_load(out: &BuildOutput) -> Vec<String> {
    out.get(&format!(
        "{DELVE_PACK}data/minecraft/tags/function/load.json"
    ))
    .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
    .and_then(|v| v["values"].as_array().cloned())
    .unwrap_or_default()
    .iter()
    .filter_map(|e| e.as_str().or_else(|| e["id"].as_str()))
    .map(str::to_string)
    .collect()
}

/// The quota the delve's `load` declares: the last unconditional `gamerule
/// max_command_sequence_length <n>` line of a function its `#minecraft:load`
/// tag names directly, as the game runs them in order.
fn declared(out: &BuildOutput, bodies: &BTreeMap<String, Vec<Line>>) -> Option<u64> {
    delve_load(out)
        .iter()
        .filter_map(|f| bodies.get(f))
        .flatten()
        .filter_map(|l| match l.tokens.as_slice() {
            [g, r, n] if g == "gamerule" && r == SEQUENCE_RULE => n.parse::<u64>().ok(),
            _ => None,
        })
        .rfind(|n| *n <= RULE_MAX)
}

/// Every function a `#minecraft:load` entry (of any datapack) runs in its own
/// context: the entries, and everything they call in the same tick.
fn load_rooted(
    bodies: &BTreeMap<String, Vec<Line>>,
    tags: &BTreeMap<String, Vec<String>>,
) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = tags.get("#minecraft:load").cloned().unwrap_or_default();
    while let Some(f) = stack.pop() {
        if let Some(members) = tags.get(&f) {
            stack.extend(members.iter().cloned());
            continue;
        }
        if !seen.insert(f.clone()) {
            continue;
        }
        if let Some(body) = bodies.get(&f) {
            stack.extend(body.iter().filter_map(|l| l.cost.calls.clone()));
        }
    }
    seen
}

/// State the quota the tree's sources need: when a function any of its
/// datapacks ships, outside `load`, measures past the default, write `gamerule
/// max_command_sequence_length <its chain>` as the first line of the first
/// function the delve's `#minecraft:load` tag names. Returns the declared
/// quota; `None` when the default holds — the tree is left byte-identical — or
/// when no declaration can state the need (past the gamerule's `int`, or no
/// `load` to carry it), which [`check`] then refuses.
///
/// Stated twice by a build that runs its checks, and the second statement
/// replaces the first: over the player tree, by emission; then over the tree
/// with the PackTest pack in it, whose templates run under the same quota
/// (ADR-0040, *not separated*). A build that runs no check states the first
/// alone.
pub fn declare(out: &mut BuildOutput) -> Option<u64> {
    let mut need = measure(out).need();
    if need <= MAX_COMMAND_SEQUENCE_LENGTH || need > RULE_MAX {
        return None;
    }
    let path = load_path(out)?;
    let stated = String::from_utf8(out.get(&path)?.clone()).ok()?;
    // An earlier statement is the function's first line: no other writer puts
    // the rule there.
    let original = match stated.split_once('\n') {
        Some((first, rest)) if stated_quota(first).is_some() => rest.to_string(),
        _ => stated,
    };
    // The line costs one unit wherever it stands, whatever it states, so a
    // second measure settles a `load` that some other source also calls.
    loop {
        out.insert(
            path.clone(),
            format!("gamerule {SEQUENCE_RULE} {need}\n{original}").into_bytes(),
        );
        let settled = measure(out).need();
        if settled <= need {
            return Some(need);
        }
        need = settled;
    }
}

struct Measurer<'a> {
    bodies: &'a BTreeMap<String, Vec<Line>>,
    tags: &'a BTreeMap<String, Vec<String>>,
    loops: &'a BTreeMap<String, u64>,
    party: u64,
    chains: BTreeMap<String, Chain>,
    on_stack: BTreeSet<String>,
}

impl Measurer<'_> {
    /// The quota a call to `id` spends (one for the call itself, then its
    /// body). A call to a name the tree does not ship costs its call alone.
    fn chain_of(&mut self, id: &str) -> Chain {
        if let Some(c) = self.chains.get(id) {
            return c.clone();
        }
        if self.on_stack.contains(id) {
            return Chain::Recursive;
        }
        // A tag runs each of its functions in turn, at no cost of its own.
        if let Some(members) = self.tags.get(id) {
            self.on_stack.insert(id.to_string());
            let mut sum = Chain::Bounded(0);
            for m in members {
                sum = match (sum, self.chain_of(m)) {
                    (Chain::Bounded(s), Chain::Bounded(n)) => Chain::Bounded(s.saturating_add(n)),
                    (Chain::Bounded(_), other) => other,
                    (first, _) => first,
                };
            }
            self.on_stack.remove(id);
            return sum;
        }
        let bodies = self.bodies;
        let Some(body) = bodies.get(id) else {
            return Chain::Bounded(1);
        };
        let passes = self.loops.get(id).copied();
        self.on_stack.insert(id.to_string());
        // One pass: the call, and every line's cost — a counted loop's own
        // re-entry excepted, which the pass count stands for.
        let mut pass = Chain::Bounded(1);
        for line in body {
            let Chain::Bounded(p) = pass else {
                break;
            };
            let sources = line.cost.sources(self.party);
            // The stages once; the tail once per source: the command the
            // line ends in, or the call, whose own unit the callee's chain
            // holds.
            let tail = match &line.cost.calls {
                Some(callee) if passes.is_some() && callee == id => Chain::Bounded(1),
                Some(callee) => self.chain_of(callee),
                None => Chain::Bounded(1),
            };
            pass = match tail {
                Chain::Bounded(n) => Chain::Bounded(
                    p.saturating_add(line.cost.stages)
                        .saturating_add(sources.saturating_mul(n)),
                ),
                other => other,
            };
        }
        self.on_stack.remove(id);
        let c = match pass {
            Chain::Bounded(p) => Chain::Bounded(p.saturating_mul(passes.unwrap_or(1))),
            other => other,
        };
        self.chains.insert(id.to_string(), c.clone());
        c
    }
}

/// A coded refusal of [`check`].
#[derive(Debug, Clone)]
pub struct ChainRefusal {
    /// Always [`DW_CHAIN_OVER_LIMIT`].
    pub code: DwCode,
    /// What was found, and that it is the engine's to fix.
    pub message: String,
}

/// [`measure`], refusing a tree with a function over the limit (`DW0984`).
pub fn check(out: &BuildOutput) -> Result<ChainLedger, ChainRefusal> {
    let ledger = measure(out);
    let refused = ledger.refused();
    if let Some((f, c)) = refused.first() {
        let limit = ledger.limit_of(f);
        let what = match c {
            Chain::Bounded(n) if ledger.load_rooted.contains(*f) => format!(
                "costs {n} units of command quota inside a `load` context, which runs before \
                 any declared limit and holds {limit}"
            ),
            Chain::Bounded(n) => format!(
                "costs {n} units of command quota at a party of {} against the {limit} its \
                 context holds \
                 (the delve's `load` declares {})",
                ledger.party,
                ledger
                    .declared
                    .map_or("no limit of its own".to_string(), |d| format!(
                        "`gamerule {SEQUENCE_RULE} {d}`"
                    ))
            ),
            Chain::Recursive => {
                "reaches a call cycle in the same tick, so no bound on its chain exists".to_string()
            }
        };
        return Err(ChainRefusal {
            code: DW_CHAIN_OVER_LIMIT,
            message: format!(
                "ENGINE SELF-CHECK FAILED — this is a defect in delvec, not in the campaign; \
                 report it. {} shipped function(s) could run past the command chain limit of \
                 the context they run in (`{SEQUENCE_RULE}`, {MAX_COMMAND_SEQUENCE_LENGTH} by \
                 default on the pinned server), where the server stops the function part-way \
                 and logs `Command execution stopped due to limit`. The first is `{f}`: one \
                 call, with every function it calls in the same tick, {what}. The emitter that \
                 wrote it owes the bound: one-shot work splits across ticks (`chain::steps`, \
                 `schedule function`), and a source outside `load` is covered by the limit the \
                 build declares (`chain::declare`).",
                refused.len()
            ),
        });
    }
    Ok(ledger)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(fns: &[(&str, String)]) -> BuildOutput {
        fns.iter()
            .map(|(n, b)| {
                (
                    format!("datapack/data/dw/function/{n}.mcfunction"),
                    b.clone().into_bytes(),
                )
            })
            .collect()
    }

    #[test]
    fn a_line_costs_its_stages_its_command_and_its_callee() {
        let plain = line_cost("setblock 1 2 3 minecraft:stone").unwrap();
        assert_eq!(
            plain,
            LineCost {
                stages: 0,
                forks: vec![],
                calls: None
            }
        );
        assert_eq!(plain.own(PARTY), 1);
        let forked =
            line_cost("execute if score #a dw.sys matches 1 as @a at @s run function dw:f")
                .unwrap();
        assert_eq!(
            forked,
            LineCost {
                stages: 3,
                forks: vec![Fork::Players { limit: None }],
                calls: Some("dw:f".to_string())
            }
        );
        assert_eq!(forked.own(1), 4, "one player: the stages and one call");
        assert_eq!(
            forked.own(PARTY),
            3 + PARTY,
            "the stages once, the call per player"
        );
        assert_eq!(
            line_cost("execute if block 1 2 3 minecraft:stone")
                .unwrap()
                .own(PARTY),
            1,
            "a terminal condition is the command itself"
        );
        assert_eq!(
            line_cost("execute as @a if entity @s[tag=x]")
                .unwrap()
                .own(PARTY),
            1 + PARTY,
            "a terminal condition runs once per source"
        );
        assert_eq!(
            line_cost("schedule function dw:f 1t").unwrap(),
            LineCost {
                stages: 0,
                forks: vec![],
                calls: None
            },
            "a schedule runs nothing this tick"
        );
        assert!(line_cost("# a comment").is_none());
        assert!(line_cost("").is_none());
    }

    /// What each entity argument forks into: the party bounds a player
    /// selector, a stated `limit` any other, and nothing bounds the rest.
    #[test]
    fn a_fork_is_bounded_by_the_party_or_the_selectors_limit() {
        let sources = |l: &str, party: u64| line_cost(l).unwrap().sources(party);
        for (line, solo, full) in [
            ("execute as @a run say hi", 1, PARTY),
            ("execute as @a[tag=x,limit=2] run say hi", 1, 2),
            ("execute at @p run say hi", 1, 1),
            ("execute as @r[limit=3] run say hi", 1, 3),
            (
                "execute as @e[type=minecraft:player,tag=x] run say hi",
                1,
                PARTY,
            ),
            ("execute as @e[type=player] run say hi", 1, PARTY),
            ("execute as @e[tag=x,limit=3] run say hi", 3, 3),
            ("execute as @n[type=minecraft:zombie] run say hi", 1, 1),
            ("execute as @s at @s run say hi", 1, 1),
            ("execute as Alex run say hi", 1, 1),
            ("execute as @a at @a run say hi", 1, PARTY * PARTY),
            (
                "execute positioned as @a rotated as @a run say hi",
                1,
                PARTY * PARTY,
            ),
            (
                "execute as @e[tag=x,limit=1] facing entity @a feet run say hi",
                1,
                PARTY,
            ),
            ("execute if entity @e[tag=x] run say hi", 1, 1),
            ("execute on vehicle run say hi", 1, 1),
            ("execute summon minecraft:marker run say hi", 1, 1),
            ("return run execute as @a run say hi", 1, PARTY),
            (
                "execute as @e[tag=x] as @e[tag=y] run say hi",
                MOST_SOURCES,
                MOST_SOURCES,
            ),
        ] {
            assert_eq!(sources(line, 1), solo, "{line} at one player");
            assert_eq!(sources(line, PARTY), full, "{line} at a full party");
        }
        for line in [
            "execute as @e[tag=x] run say hi",
            "execute at @e[type=!minecraft:player] run say hi",
            "execute on passengers run say hi",
            "$execute as $(who) run say hi",
        ] {
            let l = line_cost(line).unwrap();
            assert_eq!(
                l.sources(1),
                MOST_SOURCES,
                "{line}: the fork limit bounds it"
            );
            assert!(l.at_the_fork_limit().is_some(), "{line} names why");
        }
    }

    /// A tick that forks over the players: its chain at a full party is the
    /// stages once and the callee per player, and the build declares that.
    #[test]
    fn a_fork_over_the_players_is_measured_at_a_full_party() {
        let callee = "setblock 0 0 0 minecraft:stone\n".repeat(20_000);
        let mut out = delve(0, "function dw:setup");
        out.insert(
            "datapack/data/dw/function/tick.mcfunction".to_string(),
            b"execute as @a at @s run function dw:each".to_vec(),
        );
        out.insert(
            "datapack/data/dw/function/each.mcfunction".to_string(),
            callee.into_bytes(),
        );
        let solo = measure_at(&out, 1);
        let each = 20_001u64;
        assert_eq!(solo.chains["dw:tick"], Chain::Bounded(1 + 2 + each));
        let l = measure(&out);
        assert_eq!(l.party, PARTY);
        assert_eq!(l.chains["dw:tick"], Chain::Bounded(1 + 2 + PARTY * each));
        assert_eq!(l.solo_longest, Some(("dw:tick".to_string(), 1 + 2 + each)));
        // Within the default for one player, past it for a full party: the
        // build declares the party's chain and the check holds it there.
        assert!(1 + 2 + each <= MAX_COMMAND_SEQUENCE_LENGTH);
        let bare = out.clone();
        assert_eq!(check(&bare).unwrap_err().code.id(), "DW0984");
        let need = declare(&mut out).expect("a declaration");
        assert_eq!(need, 1 + 2 + PARTY * each);
        assert!(check(&out).is_ok());
    }

    /// A fork whose selector states no bound is held to the game's fork
    /// limit: the build declares that worst case, the ledger names the line,
    /// and a need past the gamerule's `int` refuses.
    #[test]
    fn a_fork_no_selector_bounds_is_held_to_the_fork_limit() {
        let mut out = delve(0, "function dw:setup");
        out.insert(
            "datapack/data/dw/function/tick.mcfunction".to_string(),
            b"execute as @e[tag=dw_thing] run say hi".to_vec(),
        );
        let l = measure(&out);
        assert_eq!(l.chains["dw:tick"], Chain::Bounded(1 + 1 + MOST_SOURCES));
        assert_eq!(l.fork_limited.len(), 1);
        assert_eq!(l.fork_limited[0].0, "dw:tick");
        assert!(
            l.fork_limited[0].1.contains("states no `limit`"),
            "{:?}",
            l.fork_limited
        );
        assert_eq!(check(&out).unwrap_err().code.id(), "DW0984");
        assert_eq!(declare(&mut out), Some(1 + 1 + MOST_SOURCES));
        assert!(check(&out).is_ok());

        let limited = tree(&[(
            "each",
            "execute as @e[tag=dw_thing,limit=8] run say hi".to_string(),
        )]);
        assert_eq!(
            measure(&limited).chains["dw:each"],
            Chain::Bounded(1 + 1 + 8)
        );
        assert!(measure(&limited).fork_limited.is_empty());

        // Every source a fork leaves runs a callee of 40 000: past the `int`
        // the gamerule holds, so no declaration states it, and it refuses.
        let mut past = delve(0, "function dw:setup");
        past.insert(
            "datapack/data/dw/function/tick.mcfunction".to_string(),
            b"execute as @e[tag=dw_thing] run function dw:each".to_vec(),
        );
        past.insert(
            "datapack/data/dw/function/each.mcfunction".to_string(),
            "say hi\n".repeat(40_000).into_bytes(),
        );
        const { assert!(MOST_SOURCES * 40_000 > RULE_MAX) };
        assert_eq!(declare(&mut past), None);
        assert_eq!(check(&past).unwrap_err().code.id(), "DW0984");
    }

    #[test]
    fn a_chain_counts_what_it_calls() {
        let out = tree(&[
            ("a", "say a\nfunction dw:b\nfunction dw:b".to_string()),
            ("b", "say b\nsay c".to_string()),
        ]);
        let l = measure(&out);
        assert_eq!(l.chains["dw:b"], Chain::Bounded(3));
        // 1 (call) + 1 (say) + 2 × chain(b).
        assert_eq!(l.chains["dw:a"], Chain::Bounded(8));
        assert_eq!(l.functions, 2);
    }

    /// `DW0984` on a crafted chain: no single function is near the limit, but
    /// one tick's chain through them is past it.
    #[test]
    fn dw0984_refuses_a_chain_over_the_limit() {
        let half = (MAX_COMMAND_SEQUENCE_LENGTH / 2) as usize;
        let body = "setblock 0 0 0 minecraft:stone\n".repeat(half);
        let out = tree(&[
            ("root", "function dw:left\nfunction dw:right".to_string()),
            ("left", body.clone()),
            ("right", body),
        ]);
        let err = check(&out).unwrap_err();
        assert_eq!(err.code, DW_CHAIN_OVER_LIMIT);
        assert_eq!(err.code.id(), "DW0984");
        assert!(err.message.contains("`dw:root`"), "{}", err.message);
        // Each half alone is under the limit.
        let l = measure(&out);
        assert_eq!(l.refused().len(), 1);
    }

    #[test]
    fn dw0984_refuses_a_function_that_calls_itself() {
        let out = tree(&[(
            "loop",
            "execute if entity @s run function dw:loop".to_string(),
        )]);
        let err = check(&out).unwrap_err();
        assert_eq!(err.code.id(), "DW0984");
        assert!(err.message.contains("call cycle"), "{}", err.message);
    }

    /// The creator overlay's ray, in the shape it ships: a counted loop of at
    /// most 256 passes per entry.
    #[test]
    fn a_counted_loop_is_bounded_by_its_count() {
        let ray = "scoreboard players add #ray dw.rh 1\n\
                   execute unless block ~ ~ ~ minecraft:air run summon minecraft:marker ~ ~ ~\n\
                   execute unless entity @e[tag=p] if score #ray dw.rh matches ..255 positioned ^ ^ ^0.25 run function dw:ray"
            .to_string();
        let cast = "scoreboard players set #ray dw.rh 0\nexecute anchored eyes positioned ^ ^ ^ run function dw:ray".to_string();
        let out = tree(&[("ray", ray.clone()), ("cast", cast.clone())]);
        let l = measure(&out);
        assert_eq!(l.counted_loops, 1);
        // A pass: the call (1), the add (1), the summon (`unless` + the
        // command, 2), the guard line (`unless`, `if`, `positioned` + the
        // call's line, 4) — 8; 256 passes.
        assert_eq!(l.chains["dw:ray"], Chain::Bounded(8 * 256));
        assert!(check(&out).is_ok());

        // Entered without setting the count first: no bound.
        let unset = tree(&[
            ("ray", ray.clone()),
            ("cast", "function dw:ray".to_string()),
        ]);
        assert_eq!(measure(&unset).counted_loops, 0);
        assert_eq!(check(&unset).unwrap_err().code.id(), "DW0984");

        // A body that writes the count a second time is not one.
        let rewound = tree(&[
            ("ray", format!("{ray}\nscoreboard players set #ray dw.rh 0")),
            ("cast", cast),
        ]);
        assert_eq!(measure(&rewound).counted_loops, 0);
        assert_eq!(check(&rewound).unwrap_err().code.id(), "DW0984");
    }

    /// A delve whose tick runs `n` lines, each a separate source of `setblock`,
    /// with a `load` the tag names.
    fn delve(tick_lines: usize, load: &str) -> BuildOutput {
        let mut out = tree(&[
            ("load", load.to_string()),
            ("setup", "say setup".to_string()),
            (
                "tick",
                "setblock 0 0 0 minecraft:stone\n".repeat(tick_lines),
            ),
        ]);
        out.insert(
            "datapack/data/minecraft/tags/function/load.json".to_string(),
            br#"{"values": ["dw:load"]}"#.to_vec(),
        );
        out.insert(
            "datapack/data/minecraft/tags/function/tick.json".to_string(),
            br#"{"values": ["dw:tick"]}"#.to_vec(),
        );
        out
    }

    /// A tick past the default: the build declares what it measures, the
    /// declaration is the first line of `load`, and the check holds every
    /// source to it — and refuses the same tree without it.
    #[test]
    fn a_tick_past_the_default_is_held_to_the_quota_the_build_declares() {
        let over = (MAX_COMMAND_SEQUENCE_LENGTH as usize) + 1979;
        let mut out = delve(over, "function dw:setup");
        let bare = out.clone();
        assert_eq!(check(&bare).unwrap_err().code.id(), "DW0984");

        let need = declare(&mut out).expect("a declaration");
        assert_eq!(need, over as u64 + 1, "the call and every line");
        let load =
            String::from_utf8(out["datapack/data/dw/function/load.mcfunction"].clone()).unwrap();
        assert_eq!(
            load.lines().next(),
            Some(format!("gamerule max_command_sequence_length {need}").as_str())
        );
        let l = check(&out).expect("every source within its context's limit");
        assert_eq!(l.declared, Some(need));
        assert_eq!(l.longest(), Some(("dw:tick", need)));
        assert!(l.load_rooted.contains("dw:load") && l.load_rooted.contains("dw:setup"));
        assert!(!l.load_rooted.contains("dw:tick"));

        // One more line than declared reds.
        let mut grown = out.clone();
        let tick = "datapack/data/dw/function/tick.mcfunction".to_string();
        let mut body = grown[&tick].clone();
        body.extend_from_slice(b"say one more\n");
        grown.insert(tick, body);
        assert_eq!(check(&grown).unwrap_err().code.id(), "DW0984");
    }

    /// **A second statement replaces the first** (ADR-0040, *not
    /// separated*). The delve's tick is past the default, so the statement
    /// over the player tree declares it; a function of the PackTest pack then calls the
    /// tick and runs lines of its own, so the statement over both trees
    /// declares more — in the same first line, with one `gamerule` line in the
    /// function and the rest of it as written — and the check holds that
    /// function to it. Stating again over the same tree changes no byte.
    /// Without the second statement it is over the limit the first
    /// declared, which is what the check then refuses.
    #[test]
    fn a_second_statement_over_the_packtest_pack_replaces_the_first() {
        let over = (MAX_COMMAND_SEQUENCE_LENGTH as usize) + 11;
        let mut out = delve(over, "function dw:setup");
        let first = declare(&mut out).expect("the player tree declares its tick");
        out.insert(
            "packtest-datapack/data/dw/function/drives.mcfunction".to_string(),
            b"function dw:tick\nsay one\nsay two\n".to_vec(),
        );
        assert_eq!(
            check(&out).unwrap_err().code.id(),
            "DW0984",
            "the pack's function is over what the player tree declared"
        );
        let second = declare(&mut out).expect("the pack's function needs more");
        assert!(second > first, "{second} against {first}");
        let load =
            String::from_utf8(out["datapack/data/dw/function/load.mcfunction"].clone()).unwrap();
        assert_eq!(
            load,
            format!("gamerule max_command_sequence_length {second}\nfunction dw:setup")
        );
        assert_eq!(
            check(&out).expect("held to the second").declared,
            Some(second)
        );
        let stated = out.clone();
        assert_eq!(declare(&mut out), Some(second));
        assert_eq!(
            out, stated,
            "a statement over the same tree changes no byte"
        );
    }

    /// The default holds: nothing is declared and the tree is untouched.
    #[test]
    fn a_tree_within_the_default_declares_nothing() {
        let mut out = delve(1000, "function dw:setup");
        let before = out.clone();
        assert_eq!(declare(&mut out), None);
        assert_eq!(out, before);
        assert_eq!(check(&out).unwrap().declared, None);
    }

    /// The declaration does not reach the context that runs it: a `load` past
    /// the default refuses whatever the build declares.
    #[test]
    fn a_load_past_the_default_refuses_whatever_is_declared() {
        let over = (MAX_COMMAND_SEQUENCE_LENGTH as usize) + 10;
        let mut out = delve(over, "function dw:setup");
        declare(&mut out).unwrap();
        let setup = "datapack/data/dw/function/setup.mcfunction".to_string();
        out.insert(
            setup,
            "setblock 0 0 0 minecraft:stone\n".repeat(over).into_bytes(),
        );
        let err = check(&out).unwrap_err();
        assert!(err.message.contains("`load` context"), "{}", err.message);
        // Nor does a declaration outside a `load` entry count.
        let mut moved = delve(over, "function dw:setup");
        let tick = "datapack/data/dw/function/tick.mcfunction".to_string();
        let mut body = b"gamerule max_command_sequence_length 2000000\n".to_vec();
        body.extend_from_slice(&moved[&tick]);
        moved.insert(tick, body);
        assert_eq!(measure(&moved).declared, None);
        assert_eq!(check(&moved).unwrap_err().code.id(), "DW0984");
    }

    /// A tick that also calls `load` settles on the measure with the
    /// declaration standing.
    #[test]
    fn a_declaration_settles_when_a_source_also_calls_load() {
        let over = (MAX_COMMAND_SEQUENCE_LENGTH as usize) + 5;
        let mut out = delve(over, "say load");
        let tick = "datapack/data/dw/function/tick.mcfunction".to_string();
        let mut body = b"function dw:load\n".to_vec();
        body.extend_from_slice(&out[&tick]);
        out.insert(tick, body);
        let need = declare(&mut out).unwrap();
        assert!(check(&out).is_ok());
        assert_eq!(measure(&out).longest().unwrap().1, need);
    }

    #[test]
    fn steps_stay_under_their_budget_in_order() {
        let lines: Vec<String> = (0..10).map(|i| format!("setblock {i} 0 0 stone")).collect();
        let s = steps(&lines, 4);
        assert_eq!(s.iter().map(Vec::len).collect::<Vec<_>>(), vec![4, 4, 2]);
        assert_eq!(s.concat(), lines);
        assert!(steps(&[], 4).is_empty());
    }
}
