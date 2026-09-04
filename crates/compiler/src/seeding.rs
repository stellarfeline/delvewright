//! No emitted comparison may read a score entry the emitted pack never creates
//! (`DW0495`).
//!
//! ## The runtime fact this rests on
//!
//! Measured on the pinned server (Minecraft Java **1.21.11**, `versions.toml`
//! `[minecraft].version`), over rcon and with a real client joined:
//!
//! * A scoreboard entry does not exist until something **writes** it. Joining
//!   creates nothing: a fresh player has no entry in a `dummy` objective, none in
//!   a `deathCount` objective until they die, none in a statistic objective until
//!   the statistic moves, and none in a `trigger` objective until it is enabled.
//! * **Every** comparison against a holder with no entry is FALSE. `if score X O
//!   matches 0` does not fire; neither does `matches 0..`; neither does `if score
//!   A oA > B oB` when either side is missing. `unless` is therefore always TRUE.
//! * `scores={O=…}` in a selector matches no entity that has no entry in `O`.
//! * `scoreboard players set|add|remove|enable <h> <o>` creates the entry;
//!   `operation` creates BOTH its target's and its **source's**; `execute store
//!   … score <h> <o>` creates it; `scoreboard players reset` **destroys** it.
//!
//! The consequence, and the reason this is a whole defect class rather than one
//! bug: an unwritten score is not "zero". It reads as *false to every question*,
//! including the questions whose honest answer at zero is yes.
//!
//! ## The defect this closes
//!
//! `dw.death_ack` and `dw.death_seen` are `dummy` objectives, so a player who has
//! never died has no entry in either, and `execute if score @s dw.deaths > @s
//! dw.death_ack` did not fire. `on_death` and the checkpoint respawn dispatch
//! therefore **never ran on a player's first death** — no forfeit, no recovery
//! stake, no `on_respawn`, no engine re-seat. Latent since spec-0012, past every
//! shape proof and every manual test, because anyone testing a death loop dies
//! more than once. For a souls-like campaign the first death is the one that
//! matters most.
//!
//! The project had already learned the lesson at one site and not generalised it,
//! which is the shape CLAUDE.md says to look for: `docs/reference/compiler.md`
//! §`set-flag` states outright that *"`unless … matches 1` is the deliberate
//! unset-safe spelling"*, and `DW0501` carries the same insight one layer up — a
//! `requires_state` gate reading a datum no verb writes is *"a constant wearing a
//! condition's clothes"*. `DW0501` binds to **campaign-declared** state and reads
//! the campaign JSON; nothing looked at **engine-emitted** comparisons over
//! **engine-internal** objectives, which is where the death edge lived and where
//! no campaign could have declared anything. This module is that sibling: same
//! insight, the emitted layer, feature-blind, read off the bytes that ship.
//!
//! ## What counts as evidence
//!
//! A comparison is admitted on any ONE of four kinds of evidence. They are four
//! forms of the same demand — *the entry exists by the time this runs* — not four
//! rules:
//!
//! 1. **A write.** An unconditional write of that `(holder, objective)` earlier in
//!    the same body, or in a function the body calls unconditionally before it.
//! 2. **A spelling.** The comparison's answer on a missing entry equals its answer
//!    on an entry holding the baseline 0, so the absence cannot change behaviour:
//!    any `matches <range>` whose range excludes 0, in either sense. This is the
//!    flag idiom the reference already documents, stated as a property rather than
//!    left as folklore. A range covering the whole of `i32` is admitted too, and
//!    separately: it asks *"is there an entry at all"*, which is a deliberate
//!    existence probe, never an accidental value comparison.
//! 3. **A guard.** A conjunctive `if score <h> <O> matches <R>` with `0 ∉ R` —
//!    earlier in the same `execute` chain, or a sibling clause in the same
//!    `scores={…}` block — proves the entity has an entry in `O`, and therefore in
//!    every objective the pack always writes *alongside* `O`. That co-write group
//!    is computed from the tree, never declared: the bodies that write `O` to a
//!    value `R` admits, intersected. It is what makes a stake ledger's `kx/ky/kz`
//!    provable behind its own `kl matches 1`, and a shop's `shop_at` provable
//!    behind its `shop=1`.
//! 4. **A driver.** The objective is written unconditionally, for an entity, by a
//!    function the `minecraft:tick` / `minecraft:load` chain reaches without
//!    passing a single condition — the once-per-player seeding hooks
//!    (`state_seed`, `class_arm`). Such a write happens on a player's first tick,
//!    before any player-driven interaction can ask.
//!
//! **Named limits.** (a) Ordering *within* one tick is not modelled: a read placed
//! earlier in `tick` than its driver seed would pass here and be wrong for one
//! tick. (b) A `#`-prefixed holder is not an entity — vanilla's own convention for
//! a compiler-owned singleton — and its lifecycle is one emitter's arm/read pair,
//! so for those the demand is only that *something in the pack writes it*; the
//! ordering is the emitter's to guarantee, and the check would only guess. Both
//! limits are directional: they admit, they never accuse.

use delvewright_dsl::DwCode;
use std::collections::{BTreeMap, BTreeSet};

use crate::failure::Failure;
use crate::integrity::Tier;

/// `DW0495`: an emitted score comparison reads an entry the emitted pack never
/// creates, so its answer is decided by the absence rather than by play.
///
/// Build-tier (exit 3). The command compiles, the datapack loads, and the branch
/// simply never takes — the failure shape that cost every campaign with a
/// checkpoint its players' FIRST death.
///
/// **`every_version`**, and the reason is the same one `DW0497` has: this rule
/// judges the COMPILER's own output, never the campaign's documents. A campaign
/// can neither cause nor fix an unbacked comparison — it cannot name the
/// objective, cannot write it, and cannot see the command — so there is no
/// obligation here for a `dsl_version` to grandfather. Fencing it would mean
/// deciding that campaigns below some version keep shipping a first death that
/// does nothing, which is the opposite of what a fence is for.
pub const DW_UNSEEDED_SCORE_READ: DwCode = DwCode::every_version("DW0495");

// ---------------------------------------------------------------------------
// ranges
// ---------------------------------------------------------------------------

/// An inclusive integer range, as an mcfunction `matches` argument denotes one.
///
/// `i64` bounds so the whole `i32` domain — the existence-probe spelling — is
/// representable without saturating at its own edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Range {
    lo: i64,
    hi: i64,
}

impl Range {
    /// `5`, `5..`, `..5`, `1..9`. `None` for anything else (a macro placeholder,
    /// a malformed literal) — an unreadable range is never evidence.
    fn parse(spec: &str) -> Option<Range> {
        const MIN: i64 = i32::MIN as i64;
        const MAX: i64 = i32::MAX as i64;
        if let Some(rest) = spec.strip_prefix("..") {
            return rest.parse::<i64>().ok().map(|hi| Range { lo: MIN, hi });
        }
        if let Some(rest) = spec.strip_suffix("..") {
            return rest.parse::<i64>().ok().map(|lo| Range { lo, hi: MAX });
        }
        if let Some((a, b)) = spec.split_once("..") {
            let (lo, hi) = (a.parse::<i64>().ok()?, b.parse::<i64>().ok()?);
            return Some(Range { lo, hi });
        }
        spec.parse::<i64>().ok().map(|v| Range { lo: v, hi: v })
    }

    fn contains(&self, v: i64) -> bool {
        self.lo <= v && v <= self.hi
    }

    /// Whether the range spans the whole `i32` domain — the existence probe.
    fn is_whole_domain(&self) -> bool {
        self.lo <= i32::MIN as i64 && self.hi >= i32::MAX as i64
    }
}

/// Whether a `matches` argument is **unset-safe**: its answer on a missing entry
/// is the same as its answer on an entry holding 0, so nothing can turn on the
/// difference. True for a range that excludes 0 and for the whole-domain
/// existence probe; false for an unreadable spec.
fn spelling_is_unset_safe(spec: &str) -> bool {
    match Range::parse(spec) {
        Some(r) => !r.contains(0) || r.is_whole_domain(),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// one parsed emitted line
// ---------------------------------------------------------------------------

/// A `<holder> <objective>` pair, with `@s` already resolved against the
/// enclosing `execute as`.
type Key = (String, String);

/// One score comparison the emitted tree performs.
#[derive(Debug, Clone)]
enum Read {
    /// `if|unless score <holder> <objective> matches <spec>`.
    Matches {
        key: Key,
        spec: String,
        /// `if`, not `unless` — only an `if` can serve as a guard (an `unless`
        /// that passes may have passed *because* the entry is missing).
        positive: bool,
    },
    /// One side of `if|unless score <a> <aobj> <op> <b> <bobj>`. Never unset-safe:
    /// the other side's runtime value is unknown, so "missing" and "zero" cannot
    /// be shown to agree.
    Operand { key: Key },
    /// One `<objective>=<spec>` clause of a selector's `scores={…}` block. Every
    /// clause of one block must hold, so the clauses guard each other.
    Clause {
        key: Key,
        spec: String,
        /// Index of the block within the line, so sibling clauses find each other.
        block: usize,
    },
}

impl Read {
    fn key(&self) -> &Key {
        match self {
            Read::Matches { key, .. } | Read::Operand { key } | Read::Clause { key, .. } => key,
        }
    }
}

/// One write the emitted tree performs.
#[derive(Debug, Clone)]
struct Write {
    key: Key,
    /// The literal value the entry ends up holding, when the command names one
    /// (`set`). `None` for `add`/`remove`/`operation`/`enable`/`store`, whose
    /// result depends on what was there — which makes them *possible* members of
    /// any co-write group, the conservative direction.
    value: Option<i64>,
    /// No `if`/`unless` anywhere in this command's chain.
    unconditional: bool,
    /// `scoreboard players reset` — this destroys the entry rather than creating
    /// it, so it retires establishment instead of granting it.
    removes: bool,
}

/// Everything one emitted line does that this check cares about.
#[derive(Debug, Default, Clone)]
struct Line {
    reads: Vec<Read>,
    writes: Vec<Write>,
    /// An internal `function <ns>:<name>` call reached with no condition on the
    /// chain — the only kind whose callee summary transfers to the caller.
    unconditional_call: Option<String>,
    /// The `as` binding in force at the call, so a callee's `@s` facts land on the
    /// right holder here.
    call_binding: Option<String>,
}

/// Whether a holder names a real entity, rather than a compiler-owned singleton.
///
/// Vanilla's own convention: a `#`-prefixed name can never be an entity, so the
/// emitter uses it exactly for the scores it creates and consumes itself. `*` is
/// "every tracked entry", not a holder.
fn is_entity_holder(h: &str) -> bool {
    !h.starts_with('#') && h != "*"
}

/// Resolve `@s` against the `execute as` binding in force.
fn resolve(holder: &str, binding: Option<&str>) -> String {
    match (holder, binding) {
        ("@s", Some(b)) => b.to_string(),
        _ => holder.to_string(),
    }
}

/// Every `<objective>=<spec>` clause of every `scores={…}` block in one token.
///
/// Only tokens that are selectors (`@…`) are read, so the word `scores` inside a
/// `tellraw` payload is not a clause.
fn score_clauses(token: &str) -> Vec<Vec<(String, String)>> {
    let mut out = Vec::new();
    if !token.starts_with('@') {
        return out;
    }
    let mut rest = token;
    while let Some(i) = rest.find("scores={") {
        let after = &rest[i + "scores={".len()..];
        let Some(end) = after.find('}') else { break };
        let mut block = Vec::new();
        for pair in after[..end].split(',') {
            if let Some((o, spec)) = pair.split_once('=') {
                block.push((o.trim().to_string(), spec.trim().to_string()));
            }
        }
        if !block.is_empty() {
            out.push(block);
        }
        rest = &after[end..];
    }
    out
}

/// Record every `scores={…}` clause carried by one token, as one guarded block.
fn push_clauses(token: &str, blocks: &mut usize, out: &mut Line) {
    for block in score_clauses(token) {
        let idx = *blocks;
        *blocks += 1;
        for (objective, spec) in block {
            out.reads.push(Read::Clause {
                key: (token.to_string(), objective),
                spec,
                block: idx,
            });
        }
    }
}

/// Parse one emitted command.
///
/// The `execute` grammar is walked left to right so that `as` rebinding, the
/// order of conditions, and the boundary at `run` are all honoured; the command
/// after `run` is parsed as a fresh command, which is how a nested `execute` and
/// `return run function …` come out right. Nothing is read out of position, so a
/// chat payload naming `score` or `function` is not a command.
fn parse_line(ns: &str, raw: &str) -> Line {
    let mut line = Line::default();
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return line;
    }
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    parse_command(ns, &tokens, None, false, &mut 0, &mut line);
    line
}

/// One command (possibly an `execute` chain), from `tokens[0]`.
///
/// `binding` is the `as` executor in force on entry; `conditioned` says whether
/// any `if`/`unless` has already been crossed. `blocks` numbers the `scores={…}`
/// blocks across the whole line so siblings can be recognised.
fn parse_command(
    ns: &str,
    tokens: &[&str],
    binding: Option<&str>,
    conditioned: bool,
    blocks: &mut usize,
    out: &mut Line,
) {
    // `return run <command>` is a tail call, still one command.
    let tokens = match tokens {
        ["return", "run", rest @ ..] if !rest.is_empty() => rest,
        _ => tokens,
    };
    let Some(head) = tokens.first() else { return };

    if *head != "execute" {
        for t in tokens {
            push_clauses(t, blocks, out);
        }
        parse_terminal(ns, tokens, binding, conditioned, out);
        return;
    }

    // The chain ends at its first bare `run`; everything after it is a fresh
    // command, parsed below with the binding and the conditioning this chain
    // established.
    let chain_end = tokens
        .iter()
        .position(|t| *t == "run")
        .unwrap_or(tokens.len());
    // Selector `scores={…}` reads on every modifier the chain crosses — including
    // the ones consumed as operands (`as @a[scores={…}]`), which a walk that only
    // looked at the tokens it stopped on would step straight over.
    for t in &tokens[1..chain_end] {
        push_clauses(t, blocks, out);
    }

    let mut bind = binding.map(str::to_string);
    let mut cond = conditioned;
    let mut i = 1;
    while i < chain_end {
        match tokens[i] {
            "as" if i + 1 < tokens.len() => {
                bind = Some(tokens[i + 1].to_string());
                i += 2;
            }
            "if" | "unless" if tokens.get(i + 1) == Some(&"score") && i + 5 < tokens.len() => {
                let positive = tokens[i] == "if";
                let holder = resolve(tokens[i + 2], bind.as_deref());
                let objective = tokens[i + 3].to_string();
                if tokens[i + 4] == "matches" {
                    out.reads.push(Read::Matches {
                        key: (holder, objective),
                        spec: tokens[i + 5].to_string(),
                        positive,
                    });
                    i += 6;
                } else if i + 6 < tokens.len() {
                    let src = resolve(tokens[i + 5], bind.as_deref());
                    out.reads.push(Read::Operand {
                        key: (holder, objective),
                    });
                    out.reads.push(Read::Operand {
                        key: (src, tokens[i + 6].to_string()),
                    });
                    i += 7;
                } else {
                    i += 5;
                }
                cond = true;
            }
            "if" | "unless" => {
                cond = true;
                i += 1;
            }
            // `execute … store result|success score <holder> <objective> …`
            "store" if tokens.get(i + 2) == Some(&"score") && i + 4 < tokens.len() => {
                out.writes.push(Write {
                    key: (
                        resolve(tokens[i + 3], bind.as_deref()),
                        tokens[i + 4].to_string(),
                    ),
                    value: None,
                    unconditional: !cond,
                    removes: false,
                });
                i += 5;
            }
            _ => i += 1,
        }
    }
    if chain_end + 1 < tokens.len() {
        parse_command(
            ns,
            &tokens[chain_end + 1..],
            bind.as_deref(),
            cond,
            blocks,
            out,
        );
    }
}

/// A command that is not an `execute` chain: a `scoreboard players …` write, an
/// internal `function` call, or anything else (which this check ignores).
fn parse_terminal(
    ns: &str,
    tokens: &[&str],
    binding: Option<&str>,
    conditioned: bool,
    out: &mut Line,
) {
    match tokens {
        ["scoreboard", "players", verb, holder, objective, tail @ ..] => {
            let key = (resolve(holder, binding), objective.to_string());
            match *verb {
                "set" => out.writes.push(Write {
                    key,
                    value: tail.first().and_then(|v| v.parse::<i64>().ok()),
                    unconditional: !conditioned,
                    removes: false,
                }),
                "add" | "remove" | "operation" | "enable" => {
                    out.writes.push(Write {
                        key,
                        value: None,
                        unconditional: !conditioned,
                        removes: false,
                    });
                    // An `operation` reads its SOURCE through the same
                    // create-if-absent path as its target, so it establishes both
                    // (measured 1.21.11: `operation Dst = SrcGhost` leaves
                    // `SrcGhost` holding 0 where it had no entry at all).
                    if *verb == "operation"
                        && let [_op, src, sobj, ..] = tail
                    {
                        out.writes.push(Write {
                            key: (resolve(src, binding), sobj.to_string()),
                            value: None,
                            unconditional: !conditioned,
                            removes: false,
                        });
                    }
                }
                "reset" => out.writes.push(Write {
                    key,
                    value: None,
                    unconditional: !conditioned,
                    removes: true,
                }),
                _ => {}
            }
        }
        // `scoreboard players reset <holder>` — every objective at once.
        ["scoreboard", "players", "reset", holder] => out.writes.push(Write {
            key: (resolve(holder, binding), String::new()),
            value: None,
            unconditional: !conditioned,
            removes: true,
        }),
        ["function", target, ..] if !conditioned => {
            if let Some(name) = target.strip_prefix(ns).and_then(|r| r.strip_prefix(':'))
                && out.unconditional_call.is_none()
            {
                out.unconditional_call = Some(name.to_string());
                out.call_binding = binding.map(str::to_string);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// the whole-tree analysis
// ---------------------------------------------------------------------------

/// What the check looked at, so a green result can be told apart from a green
/// result that examined nothing (CLAUDE.md: *a green gate that binds to nothing is
/// vacuous, not a pass*).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SeedingCensus {
    /// Every score read the emitted tree performs, of every kind.
    pub comparisons: usize,
    /// Reads whose holder is a real entity (a selector or a player name).
    pub entity_reads: usize,
    /// Reads whose holder is a compiler-owned `#` singleton.
    pub fake_reads: usize,
    /// Reads admitted because the absence of an entry cannot change their answer.
    pub admitted_by_spelling: usize,
    /// Reads admitted by a write that reaches them.
    pub admitted_by_write: usize,
    /// Reads admitted by a conjunctive guard (its own objective, or one always
    /// written alongside it).
    pub admitted_by_guard: usize,
    /// Reads admitted because a tick/load driver seeds the objective for every
    /// entity before play.
    pub admitted_by_driver: usize,
    /// One line per read admitted by nothing, `path:line` first.
    pub findings: Vec<String>,
}

/// Every emitted `.mcfunction` body, keyed by artifact path.
fn bodies(out: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, String> {
    let mut fns = BTreeMap::new();
    for (path, bytes) in out {
        if !path.ends_with(".mcfunction") || Tier::of(path).is_none() {
            continue;
        }
        if let Ok(body) = std::str::from_utf8(bytes) {
            fns.insert(path.clone(), body.to_string());
        }
    }
    fns
}

/// The name a `function <ns>:<name>` call reaches this artifact by.
fn callable_name(path: &str) -> Option<&str> {
    path.rsplit_once("/function/")?
        .1
        .strip_suffix(".mcfunction")
}

/// The functions the `minecraft:load` and `minecraft:tick` tags name.
fn tag_roots(out: &BTreeMap<String, Vec<u8>>, ns: &str) -> BTreeSet<String> {
    let mut roots = BTreeSet::new();
    for (path, bytes) in out {
        let is_tag = path.ends_with("/minecraft/tags/function/load.json")
            || path.ends_with("/minecraft/tags/function/tick.json");
        if !is_tag || Tier::of(path) != Some(Tier::Shipped) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        // The emitted tag is `{"values": ["<ns>:<name>", …]}`; read the names out
        // without pulling a JSON parser into a byte-level check.
        for chunk in text.split('"') {
            if let Some(name) = chunk.strip_prefix(ns).and_then(|r| r.strip_prefix(':')) {
                roots.insert(name.to_string());
            }
        }
    }
    roots
}

/// Analyse a finished build output and report every unadmitted comparison.
///
/// `ns` is the campaign namespace. Iteration is over `BTreeMap`/`BTreeSet`
/// throughout, so the fix list is byte-stable across runs (ADR-0006).
pub fn census(ns: &str, out: &BTreeMap<String, Vec<u8>>) -> SeedingCensus {
    let fns = bodies(out);
    let parsed: BTreeMap<String, Vec<Line>> = fns
        .iter()
        .map(|(p, b)| {
            (
                p.clone(),
                b.lines().map(|l| parse_line(ns, l)).collect::<Vec<_>>(),
            )
        })
        .collect();
    let name_to_path: BTreeMap<String, String> = fns
        .keys()
        .filter_map(|p| callable_name(p).map(|n| (n.to_string(), p.clone())))
        .collect();

    // --- every write the pack performs, at all ------------------------------
    // The only demand a `#` singleton has to meet: the emitter that reads it also
    // writes it somewhere. Ordering between an emitter's own arm and its own read
    // is that emitter's to guarantee and is not decidable from the bytes.
    let mut global_writes: BTreeSet<Key> = BTreeSet::new();
    for lines in parsed.values() {
        for l in lines {
            for w in &l.writes {
                if !w.removes {
                    global_writes.insert(w.key.clone());
                }
            }
        }
    }

    // --- co-write groups ----------------------------------------------------
    // For a body, the objectives it writes unconditionally for one entity holder.
    // Intersecting those over every body that can leave `O` holding a value the
    // guard admits gives what the guard proves alongside `O`.
    let mut body_groups: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for (path, lines) in &parsed {
        let mut per_holder: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for l in lines {
            for w in &l.writes {
                if w.unconditional && !w.removes && is_entity_holder(&w.key.0) {
                    per_holder
                        .entry(w.key.0.clone())
                        .or_default()
                        .insert(w.key.1.clone());
                }
            }
        }
        body_groups.insert(path.clone(), per_holder);
    }
    // Which (path, holder) bodies can leave `objective` holding a value in `range`.
    let cowritten = |objective: &str, range: Range| -> BTreeSet<String> {
        let mut acc: Option<BTreeSet<String>> = None;
        for (path, lines) in &parsed {
            for l in lines {
                for w in &l.writes {
                    if w.removes
                        || !w.unconditional
                        || !is_entity_holder(&w.key.0)
                        || w.key.1 != objective
                    {
                        continue;
                    }
                    // A write with no literal value could land anywhere, so it is
                    // always a candidate — which only ever shrinks the group.
                    if w.value.is_some_and(|v| !range.contains(v)) {
                        continue;
                    }
                    let group = body_groups
                        .get(path)
                        .and_then(|m| m.get(&w.key.0))
                        .cloned()
                        .unwrap_or_default();
                    acc = Some(match acc {
                        None => group,
                        Some(prev) => prev.intersection(&group).cloned().collect(),
                    });
                }
            }
        }
        acc.unwrap_or_default()
    };

    // --- callee summaries ---------------------------------------------------
    // What a function establishes unconditionally, in terms of its own entry
    // context. Grown from the empty set to a fixpoint, which is the conservative
    // direction on a call graph that may contain a cycle.
    let mut summary: BTreeMap<String, BTreeSet<Key>> = BTreeMap::new();
    for _ in 0..16 {
        let mut changed = false;
        for (path, lines) in &parsed {
            let Some(name) = callable_name(path) else {
                continue;
            };
            let mut est: BTreeSet<Key> = BTreeSet::new();
            for l in lines {
                apply_line(l, &summary, &mut est);
            }
            if summary.get(name) != Some(&est) {
                summary.insert(name.to_string(), est);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // --- driver seeding -----------------------------------------------------
    // The closure from the load/tick tags along calls that cross NO condition, and
    // the objectives those functions write unconditionally for an entity. Such a
    // write lands on a player's first tick, before any player-driven site can ask.
    let mut frontier: Vec<String> = tag_roots(out, ns).into_iter().collect();
    let mut driver_fns: BTreeSet<String> = frontier.iter().cloned().collect();
    while let Some(name) = frontier.pop() {
        let Some(path) = name_to_path.get(&name) else {
            continue;
        };
        let Some(lines) = parsed.get(path) else {
            continue;
        };
        for l in lines {
            if let Some(callee) = &l.unconditional_call
                && driver_fns.insert(callee.clone())
            {
                frontier.push(callee.clone());
            }
        }
    }
    let mut driver_seeded: BTreeSet<String> = BTreeSet::new();
    for name in &driver_fns {
        let Some(path) = name_to_path.get(name) else {
            continue;
        };
        let Some(lines) = parsed.get(path) else {
            continue;
        };
        for l in lines {
            for w in &l.writes {
                if w.unconditional && !w.removes && is_entity_holder(&w.key.0) {
                    driver_seeded.insert(w.key.1.clone());
                }
            }
        }
    }

    // --- the walk -----------------------------------------------------------
    let mut census = SeedingCensus::default();
    for (path, lines) in &parsed {
        let mut est: BTreeSet<Key> = BTreeSet::new();
        for (n, l) in lines.iter().enumerate() {
            // Guards this line has already crossed, and the objectives they prove.
            let mut guarded: BTreeSet<Key> = BTreeSet::new();
            // A `scores={…}` block's clauses all hold together, so an exempt
            // clause guards its siblings regardless of order.
            let mut block_guards: BTreeMap<usize, BTreeSet<Key>> = BTreeMap::new();
            for r in &l.reads {
                if let Read::Clause { key, spec, block } = r
                    && let Some(range) = Range::parse(spec)
                    && !range.contains(0)
                {
                    let mut proved: BTreeSet<Key> = BTreeSet::new();
                    proved.insert(key.clone());
                    for o in cowritten(&key.1, range) {
                        proved.insert((key.0.clone(), o));
                    }
                    block_guards.entry(*block).or_default().extend(proved);
                }
            }

            for r in &l.reads {
                census.comparisons += 1;
                let key = r.key();
                let entity = is_entity_holder(&key.0);
                if entity {
                    census.entity_reads += 1;
                } else {
                    census.fake_reads += 1;
                }

                let spelling = match r {
                    Read::Matches { spec, .. } | Read::Clause { spec, .. } => {
                        spelling_is_unset_safe(spec)
                    }
                    Read::Operand { .. } => false,
                };
                if spelling {
                    census.admitted_by_spelling += 1;
                } else if est.contains(key) {
                    census.admitted_by_write += 1;
                } else if guarded.contains(key)
                    || matches!(r, Read::Clause { block, .. }
                        if block_guards.get(block).is_some_and(|g| g.contains(key)))
                {
                    census.admitted_by_guard += 1;
                } else if entity && driver_seeded.contains(&key.1) {
                    census.admitted_by_driver += 1;
                } else if !entity && global_writes.contains(key) {
                    // A `#` singleton the pack writes somewhere.
                    census.admitted_by_write += 1;
                } else {
                    census.findings.push(format!(
                        "`{path}` line {}: `{}` reads `{} {}`, and nothing in the emitted \
                         pack creates that entry before it",
                        n + 1,
                        lines_of(&fns[path], n),
                        key.0,
                        key.1
                    ));
                }

                // An `if … matches <R>` with `0 ∉ R` that has been crossed proves
                // the entry exists for everything after it on this chain.
                if let Read::Matches {
                    key,
                    spec,
                    positive: true,
                } = r
                    && let Some(range) = Range::parse(spec)
                    && !range.contains(0)
                {
                    guarded.insert(key.clone());
                    for o in cowritten(&key.1, range) {
                        guarded.insert((key.0.clone(), o));
                    }
                }
            }
            apply_line(l, &summary, &mut est);
        }
    }
    census
}

/// The `n`-th line of a body, trimmed — so a finding shows what actually ships.
fn lines_of(body: &str, n: usize) -> &str {
    body.lines().nth(n).unwrap_or("").trim()
}

/// Fold one line's unconditional effects into the establishment set.
fn apply_line(l: &Line, summary: &BTreeMap<String, BTreeSet<Key>>, est: &mut BTreeSet<Key>) {
    for w in &l.writes {
        if w.removes {
            if w.key.1.is_empty() {
                est.retain(|k| k.0 != w.key.0);
            } else {
                est.remove(&w.key);
            }
        } else if w.unconditional {
            est.insert(w.key.clone());
        }
    }
    if let Some(callee) = &l.unconditional_call {
        for (h, o) in summary.get(callee).into_iter().flatten() {
            est.insert((resolve(h, l.call_binding.as_deref()), o.clone()));
        }
    }
}

/// Prove the emitted tree never compares against an entry it does not create
/// (`DW0495`).
pub fn check_tree(ns: &str, out: &BTreeMap<String, Vec<u8>>) -> Result<(), Failure> {
    let c = census(ns, out);
    if c.findings.is_empty() {
        return Ok(());
    }
    Err(Failure {
        code: DW_UNSEEDED_SCORE_READ,
        message: format!(
            "the emitted datapack makes {n} comparison(s) against a scoreboard entry it \
             never creates. On the pinned 1.21.11 server an entry that does not exist is \
             not zero — EVERY comparison against it is false, so an `if` never fires and \
             an `unless` always does, whatever the honest answer at zero would have been \
             (measured; `scoreboard players add <entity> <obj> 0` is what creates the \
             entry). This is the defect that cost every campaign with a checkpoint its \
             players' FIRST death: `dw.death_ack` had no entry, `if score @s dw.deaths > \
             @s dw.death_ack` did not fire, and both edges worked only from the second \
             death onward. Fix the EMITTER — seed the entry on a path that reaches the \
             comparison, or write the comparison so a missing entry cannot change its \
             answer (a `matches` range that excludes 0). Never silence this by deleting \
             the comparison.\n{list}",
            n = c.findings.len(),
            list = c.findings.join("\n"),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(p, b)| (p.to_string(), b.as_bytes().to_vec()))
            .collect()
    }

    /// The instance the whole rule generalises: the first-death edge, exactly as
    /// the compiler emitted it before the acknowledgements were seeded.
    #[test]
    fn the_first_death_edge_is_refused() {
        let m = tree(&[(
            "datapack/data/isle/function/cp_respawn_check.mcfunction",
            "execute unless data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_ack \
             run function isle:cp_respawn_fire\n",
        )]);
        let e = check_tree("isle", &m).expect_err("an unseeded acknowledgement must be refused");
        assert_eq!(e.code, "DW0495");
        assert!(e.message.contains("dw.death_ack"), "{}", e.message);
    }

    /// …and seeding both sides ahead of it is what makes it legal.
    #[test]
    fn seeding_both_sides_admits_the_death_edge() {
        let m = tree(&[(
            "datapack/data/isle/function/cp_respawn_check.mcfunction",
            "scoreboard players add @s dw.deaths 0\n\
             scoreboard players add @s dw.death_ack 0\n\
             execute unless data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_ack \
             run function isle:cp_respawn_fire\n",
        )]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// A write that only happens when the comparison already passed is no
    /// evidence — the shape the death edge actually had.
    #[test]
    fn a_conditional_write_is_not_evidence() {
        let m = tree(&[(
            "datapack/data/isle/function/cp.mcfunction",
            "execute if score @s dw.deaths > @s dw.death_ack run function isle:fire\n\
             execute if data entity @s {Health:0.0f} run scoreboard players operation @s \
             dw.death_ack = @s dw.deaths\n",
        )]);
        assert_eq!(
            check_tree("isle", &m)
                .expect_err("written only behind a condition")
                .code,
            "DW0495"
        );
    }

    /// The flag idiom the reference documents: `matches 1` cannot tell a missing
    /// entry from a zero one, in either sense.
    #[test]
    fn a_range_excluding_zero_is_unset_safe() {
        let m = tree(&[(
            "datapack/data/isle/function/tick.mcfunction",
            "execute if score #party dw.f_flee matches 1 run say a\n\
             execute unless score #party dw.f_flee matches 1 run say b\n\
             execute if score @s dw.i_grind matches 1.. run say c\n\
             execute as @a[scores={dw.dlg_x=9}] run say d\n",
        )]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// …and a range that includes zero is not.
    #[test]
    fn a_range_including_zero_needs_a_writer() {
        let m = tree(&[(
            "datapack/data/isle/function/show.mcfunction",
            "execute if score @s dw.dmask matches 0 run say a\n",
        )]);
        assert_eq!(
            check_tree("isle", &m)
                .expect_err("`matches 0` is not unset-safe")
                .code,
            DW_UNSEEDED_SCORE_READ
        );
    }

    /// A whole-`i32` range asks "is there an entry at all" — the deliberate
    /// existence probe the generated PackTest suite is built on.
    #[test]
    fn a_whole_domain_range_is_an_existence_probe() {
        let m = tree(&[(
            "packtest-datapack/data/isle/test/class_once.mcfunction",
            "execute store success score #cls dw.sys if score @s dw.class matches \
             -2147483648..2147483647\n",
        )]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// The dialogue-mask shape: a callee seeds the score unconditionally, and the
    /// caller reads it after calling.
    #[test]
    fn a_callee_that_seeds_admits_its_callers_reads() {
        let m = tree(&[
            (
                "datapack/data/isle/function/show_wine.mcfunction",
                "function isle:dmask_wine\n\
                 execute if score @s dw.dmask matches 0 run say a\n",
            ),
            (
                "datapack/data/isle/function/dmask_wine.mcfunction",
                "scoreboard players set @s dw.dmask 0\n",
            ),
        ]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// …but only when the call itself is unconditional.
    #[test]
    fn a_conditional_call_transfers_nothing() {
        let m = tree(&[
            (
                "datapack/data/isle/function/show_wine.mcfunction",
                "execute if score #party dw.f_x matches 1 run function isle:dmask_wine\n\
                 execute if score @s dw.dmask matches 0 run say a\n",
            ),
            (
                "datapack/data/isle/function/dmask_wine.mcfunction",
                "scoreboard players set @s dw.dmask 0\n",
            ),
        ]);
        assert_eq!(
            check_tree("isle", &m)
                .expect_err("the callee may not have run")
                .code,
            "DW0495",
        );
    }

    /// A guard that excludes zero proves its own entry, and every entry the pack
    /// always writes beside it — the stake ledger's coordinates behind its slot.
    #[test]
    fn a_guard_proves_what_is_always_written_with_it() {
        let m = tree(&[
            (
                "datapack/data/isle/function/stk_slot.mcfunction",
                "scoreboard players set @s dw.kl0 1\n\
                 execute store result score @s dw.kx0 run data get entity @e[limit=1] Pos[0]\n",
            ),
            (
                "datapack/data/isle/function/stk_evict.mcfunction",
                "scoreboard players set @s dw.kl0 0\n",
            ),
            (
                "datapack/data/isle/function/stk_collect.mcfunction",
                "execute store result score #stk_x dw.sys run data get entity @e[limit=1] Pos[0]\n\
                 execute if score @s dw.kl0 matches 1 if score @s dw.kx0 = #stk_x dw.sys run say a\n",
            ),
        ]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// …and the guard proves nothing about an objective the pack writes on its own.
    #[test]
    fn a_guard_does_not_prove_an_unrelated_objective() {
        let m = tree(&[
            (
                "datapack/data/isle/function/stk_slot.mcfunction",
                "scoreboard players set @s dw.kl0 1\n",
            ),
            (
                "datapack/data/isle/function/stk_collect.mcfunction",
                "execute store result score #stk_x dw.sys run data get entity @e[limit=1] Pos[0]\n\
                 execute if score @s dw.kl0 matches 1 if score @s dw.kx0 = #stk_x dw.sys run say a\n",
            ),
        ]);
        assert_eq!(
            check_tree("isle", &m)
                .expect_err("kx0 is never written")
                .code,
            "DW0495",
        );
    }

    /// A sibling clause guards the rest of its own `scores={…}` block: every clause
    /// has to hold, so the one that excludes zero has already proved the entity is
    /// one the pack wrote.
    #[test]
    fn a_sibling_clause_guards_its_block() {
        let m = tree(&[
            (
                "datapack/data/isle/function/shop_open.mcfunction",
                "scoreboard players set @s dw.shop_at 0\n\
                 scoreboard players enable @s dw.shop\n",
            ),
            (
                "datapack/data/isle/function/tick.mcfunction",
                "execute as @a[scores={dw.shop=1,dw.shop_at=0}] run function isle:shop_pick\n",
            ),
            (
                "datapack/data/isle/function/shop_pick.mcfunction",
                "say a\n",
            ),
        ]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// A once-per-player seeding hook the tick driver reaches with no condition on
    /// the way establishes its objectives everywhere.
    #[test]
    fn a_driver_seed_admits_reads_elsewhere() {
        let m = tree(&[
            (
                "datapack/data/minecraft/tags/function/tick.json",
                "{\n  \"values\": [\n    \"isle:tick\"\n  ]\n}\n",
            ),
            (
                "datapack/data/isle/function/tick.mcfunction",
                "execute as @a[tag=!dw_state] run function isle:state_seed\n",
            ),
            (
                "datapack/data/isle/function/state_seed.mcfunction",
                "scoreboard players set @s dw.s_embers 0\ntag @s add dw_state\n",
            ),
            (
                "datapack/data/isle/function/shop_pick.mcfunction",
                "execute if score @s dw.s_embers matches ..0 run say broke\n",
            ),
        ]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// …and a hook the driver only reaches THROUGH a score condition does not: that
    /// is precisely the death edge's own shape one level up.
    #[test]
    fn a_seed_behind_a_condition_is_not_a_driver_seed() {
        let m = tree(&[
            (
                "datapack/data/minecraft/tags/function/tick.json",
                "{\"values\": [\"isle:tick\"]}\n",
            ),
            (
                "datapack/data/isle/function/tick.mcfunction",
                "execute if score #party dw.f_x matches 1 run function isle:state_seed\n",
            ),
            (
                "datapack/data/isle/function/state_seed.mcfunction",
                "scoreboard players set @s dw.s_embers 0\n",
            ),
            (
                "datapack/data/isle/function/shop_pick.mcfunction",
                "execute if score @s dw.s_embers matches ..0 run say broke\n",
            ),
        ]);
        assert_eq!(
            check_tree("isle", &m)
                .expect_err("the seed may never have run")
                .code,
            "DW0495",
        );
    }

    /// `scoreboard players reset` destroys the entry, so a write before it is no
    /// longer evidence for a read after it.
    #[test]
    fn a_reset_retires_the_evidence() {
        let m = tree(&[(
            "datapack/data/isle/function/f.mcfunction",
            "scoreboard players set @s dw.x 3\n\
             scoreboard players reset @s dw.x\n\
             execute if score @s dw.x matches 0 run say a\n",
        )]);
        assert_eq!(
            check_tree("isle", &m)
                .expect_err("the entry was destroyed")
                .code,
            "DW0495",
        );
    }

    /// A `#` holder is not an entity: the emitter creates and consumes it, so the
    /// demand is that the pack writes it somewhere at all.
    #[test]
    fn a_compiler_singleton_needs_only_a_writer_somewhere() {
        let m = tree(&[
            (
                "datapack/data/isle/function/arm.mcfunction",
                "scoreboard players set #t_scene dw.sys 0\n",
            ),
            (
                "datapack/data/isle/function/cs_tick.mcfunction",
                "execute if score #t_scene dw.sys matches 0 run say a\n",
            ),
        ]);
        assert!(check_tree("isle", &m).is_ok());
        let bad = tree(&[(
            "datapack/data/isle/function/cs_tick.mcfunction",
            "execute if score #t_never dw.sys matches 0 run say a\n",
        )]);
        assert_eq!(
            check_tree("isle", &bad)
                .expect_err("nothing writes it at all")
                .code,
            "DW0495",
        );
    }

    /// An `operation` creates its SOURCE's entry too — measured on 1.21.11.
    #[test]
    fn an_operation_seeds_its_source() {
        let m = tree(&[(
            "datapack/data/isle/function/f.mcfunction",
            "scoreboard players operation #dst dw.sys = @s dw.src\n\
             execute if score @s dw.src matches 0 run say a\n",
        )]);
        assert!(check_tree("isle", &m).is_ok());
    }

    /// The census is the binding count: a check that examined nothing is not a
    /// pass, so the number it looked at is part of its result.
    #[test]
    fn the_census_counts_what_it_examined() {
        let m = tree(&[(
            "datapack/data/isle/function/f.mcfunction",
            "execute if score #party dw.f_a matches 1 run say a\n\
             execute if score @s dw.b matches 0 run say b\n",
        )]);
        let c = census("isle", &m);
        assert_eq!(c.comparisons, 2);
        assert_eq!(c.entity_reads, 1);
        assert_eq!(c.fake_reads, 1);
        assert_eq!(c.admitted_by_spelling, 1);
        assert_eq!(c.findings.len(), 1);
    }

    /// A word in a chat payload is not a command.
    #[test]
    fn prose_naming_a_score_is_not_a_comparison() {
        let m = tree(&[(
            "datapack/data/isle/function/f.mcfunction",
            "say if score @s dw.nothing matches 0\n",
        )]);
        assert_eq!(census("isle", &m).comparisons, 0);
    }
}
