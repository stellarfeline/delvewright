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
//! `execute` stages (every `execute` sub-command keyword before `run`, as the
//! pinned command tree names them — an upper bound, since a keyword can also be
//! an argument's literal), one for its final command, and a called function's
//! chain. It is a count **per executing source**: a line that forks over
//! several entities (`execute as @a run …`) runs its tail once per entity, a
//! factor no reading of the bytes can bound. A function whose chain exceeds the
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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use delvewright_dsl::{DwCode, ExitTier};

use crate::compiler::commands::{CommandTree, tokenize};
use crate::compiler::emit::BuildOutput;

delvewright_dsl::dw_code! {
    /// `DW0984`: a shipped function's command chain — its own commands and
    /// every function it calls in the same tick — could exceed the game's
    /// `max_command_sequence_length`, so the server would stop it part-way. An
    /// **engine self-check**: the emitter that wrote the function owes the
    /// split across ticks.
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

/// Is `name` (a function path) a world-build step?
fn is_build_step(name: &str) -> bool {
    name == BUILD_STEP_FN
        || name
            .strip_prefix(BUILD_STEP_FN)
            .and_then(|r| r.strip_prefix('_'))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
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

/// One command line's cost: the quota its own stages and final command take,
/// and the function it calls, if it ends in a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineCost {
    /// Quota the line spends itself (stages + its final command).
    pub own: u64,
    /// The function the line calls — `ns:path`, or `#ns:path` for a tag.
    pub calls: Option<String>,
}

/// The cost of one line; `None` for a blank line or a comment.
pub fn line_cost(line: &str) -> Option<LineCost> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    let tokens = tokens_of(t);
    let mut stages = 0u64;
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
                match run {
                    Some(_) => {
                        stages += subs;
                        i = end + 1;
                    }
                    // A terminal `execute if …`: its last stage is the
                    // command itself.
                    None => {
                        return Some(LineCost {
                            own: stages + subs.max(1),
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
        own: stages + 1,
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
            c.own
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
    /// The quota one call spends, per executing source.
    Bounded(u64),
    /// The function reaches itself through calls in the same tick.
    Recursive,
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
    /// Every function's chain, by `ns:path`.
    pub chains: BTreeMap<String, Chain>,
}

impl ChainLedger {
    /// The longest bounded chain: `(function, chain)`.
    pub fn longest(&self) -> Option<(&str, u64)> {
        self.chains
            .iter()
            .filter_map(|(f, c)| match c {
                Chain::Bounded(n) => Some((f.as_str(), *n)),
                Chain::Recursive => None,
            })
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
    }

    /// The functions that refuse: over the limit, or recursive.
    pub fn refused(&self) -> Vec<(&str, &Chain)> {
        self.chains
            .iter()
            .filter(|(_, c)| match c {
                Chain::Bounded(n) => *n > MAX_COMMAND_SEQUENCE_LENGTH,
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
            "step_budget": SETUP_STEP_BUDGET,
            "examined": self.functions,
            "counted_loops": self.counted_loops,
            "steps": self.build_steps,
            "longest": longest.map(|(f, n)| serde_json::json!({ "function": f, "chain": n })),
            "refused": self.refused().iter().map(|(f, c)| serde_json::json!({
                "function": f,
                "chain": match c { Chain::Bounded(n) => serde_json::json!(n), Chain::Recursive => serde_json::json!("recursive") },
            })).collect::<Vec<_>>(),
            "quantifier": "per executing source: a fork over several entities multiplies its tail",
        })
    }

    /// The one-line binding statement a build prints.
    pub fn binding(&self) -> String {
        let (f, n) = self.longest().unwrap_or(("none", 0));
        format!(
            "chain binding: {} shipped function(s) measured, {} counted loop(s), {} world-build \
             step(s); longest chain {n} (`{f}`) of a {MAX_COMMAND_SEQUENCE_LENGTH} limit, per \
             executing source; {} refused (DW0984).",
            self.functions,
            self.counted_loops,
            self.build_steps,
            self.refused().len()
        )
    }
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
/// the server loads them side by side.
pub fn measure(out: &BuildOutput) -> ChainLedger {
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
        chains: m.chains,
    }
}

struct Measurer<'a> {
    bodies: &'a BTreeMap<String, Vec<Line>>,
    tags: &'a BTreeMap<String, Vec<String>>,
    loops: &'a BTreeMap<String, u64>,
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
            let mut sum = Some(0u64);
            for m in members {
                sum = match (sum, self.chain_of(m)) {
                    (Some(s), Chain::Bounded(n)) => Some(s.saturating_add(n)),
                    _ => None,
                };
            }
            self.on_stack.remove(id);
            return sum.map_or(Chain::Recursive, Chain::Bounded);
        }
        let bodies = self.bodies;
        let Some(body) = bodies.get(id) else {
            return Chain::Bounded(1);
        };
        let passes = self.loops.get(id).copied();
        self.on_stack.insert(id.to_string());
        // One pass: the call, and every line's cost — a counted loop's own
        // re-entry excepted, which the pass count stands for.
        let mut pass = Some(1u64);
        for line in body {
            pass = pass.map(|p| p.saturating_add(line.cost.own));
            let Some(callee) = &line.cost.calls else {
                continue;
            };
            if passes.is_some() && callee == id {
                continue;
            }
            pass = match (pass, self.chain_of(callee)) {
                // The call's own unit is counted in the callee's chain.
                (Some(p), Chain::Bounded(n)) => Some(p.saturating_add(n.saturating_sub(1))),
                _ => None,
            };
        }
        self.on_stack.remove(id);
        let c = match pass {
            Some(p) => Chain::Bounded(p.saturating_mul(passes.unwrap_or(1))),
            None => Chain::Recursive,
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
        let what = match c {
            Chain::Bounded(n) => format!("costs {n} units of command quota"),
            Chain::Recursive => {
                "reaches a call cycle in the same tick, so no bound on its chain exists".to_string()
            }
        };
        return Err(ChainRefusal {
            code: DW_CHAIN_OVER_LIMIT,
            message: format!(
                "ENGINE SELF-CHECK FAILED — this is a defect in delvec, not in the campaign; \
                 report it. {} shipped function(s) could run past the game's command chain \
                 limit (`max_command_sequence_length`, {MAX_COMMAND_SEQUENCE_LENGTH} by default \
                 on the pinned server), where the server stops the function part-way and logs \
                 `Command execution stopped due to limit`. The first is `{f}`: one call, with \
                 every function it calls in the same tick, {what}. The emitter that wrote it \
                 must split the work across ticks (`chain::steps`, `schedule function`).",
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
        assert_eq!(
            line_cost("setblock 1 2 3 minecraft:stone").unwrap(),
            LineCost {
                own: 1,
                calls: None
            }
        );
        assert_eq!(
            line_cost("execute if score #a dw.sys matches 1 as @a at @s run function dw:f")
                .unwrap(),
            LineCost {
                own: 4,
                calls: Some("dw:f".to_string())
            }
        );
        assert_eq!(
            line_cost("execute if block 1 2 3 minecraft:stone")
                .unwrap()
                .own,
            1,
            "a terminal condition is the command itself"
        );
        assert_eq!(
            line_cost("schedule function dw:f 1t").unwrap(),
            LineCost {
                own: 1,
                calls: None
            },
            "a schedule runs nothing this tick"
        );
        assert!(line_cost("# a comment").is_none());
        assert!(line_cost("").is_none());
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

    #[test]
    fn steps_stay_under_their_budget_in_order() {
        let lines: Vec<String> = (0..10).map(|i| format!("setblock {i} 0 0 stone")).collect();
        let s = steps(&lines, 4);
        assert_eq!(s.iter().map(Vec::len).collect::<Vec<_>>(), vec![4, 4, 2]);
        assert_eq!(s.concat(), lines);
        assert!(steps(&[], 4).is_empty());
    }
}
