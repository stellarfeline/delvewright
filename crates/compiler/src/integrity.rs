//! Build integrity of the emitted call graph — no `function <ns>:<name>` the
//! compiler writes may point at a function the compiler never wrote (`DW0497`).
//!
//! ## The defect class this closes
//!
//! Emission has two halves for almost every verb: the **call site** (compiled
//! from the effect tree, wherever the author put the verb) and the **machinery**
//! (compiled from a per-feature registration walk). When those two walks
//! disagree about what exists, the call site still emits — vanilla resolves an
//! unknown function to nothing at all, with no log line a player or a bot would
//! see — and the feature silently does not happen.
//!
//! The island's round-21 build is the canonical instance. `wave/storm-surf` was
//! fired from a top-level effect chain and got its full support machinery;
//! `wave/storm-shore` and `wave/storm-fire` were fired from step 7 of a
//! `sequence`, and the wave-machinery emitter — which registered only waves
//! reachable from top-level chains — produced nothing for them. `seq_under_ram`
//! still shipped `function nobodys-cave-island:spawn_storm_shore`. Two of three
//! storm waves never spawned. Every build-tier proof was green; the only thing
//! that noticed was the compiler's own generated census PackTest, because it
//! walks `waves[]` rather than the effect tree, and it failed on a *server*,
//! four minutes into a ladder run, with `Expected #wcen_d to have a score`.
//!
//! That gap is not about waves. It is about any emitter whose call walk and
//! machinery walk are two different pieces of code, and it is decidable from the
//! finished tree with no knowledge of the feature at all: **a call must have a
//! callee**. So this check is deliberately feature-blind. It runs last, over the
//! bytes that ship, in the same family as the affordance-hardware self-check
//! ([`crate::affordance`]) and the exported-waypoint self-check — a proof that
//! judges commands, not intent.
//!
//! ## Scope
//!
//! * **The campaign's own namespace only.** `function minecraft:…` (or any other
//!   pack's namespace) targets a tree this compiler does not emit and cannot
//!   reason about; it is never this diagnostic's business.
//! * **Functions, not function tags.** `function #<ns>:<tag>` names a tag, whose
//!   membership is a separate artifact; the `#` form is skipped.
//! * **Tiered resolution.** The shipped `datapack/` is what a delve image
//!   contains (ADR-0010), so a shipped function may only call shipped functions
//!   — a shipped call into `packtest-datapack/` or `creator-datapack/` resolves
//!   in CI and dangles in the player's world, which is the same bug wearing a
//!   different hat. The two overlays load *beside* the shipped pack, so their
//!   own functions may call either their own tier or the shipped one.

use delvewright_dsl::DwCode;
use std::collections::{BTreeMap, BTreeSet};

/// `DW0497`: an emitted `function <ns>:<name>` call whose target function is not
/// in the emitted tree.
///
/// Build-tier (exit 3). The call compiles, the datapack loads, and the verb
/// simply never happens — the failure shape that cost the island round 21 two of
/// its three storm waves.
pub const DW_DANGLING_FUNCTION_CALL: DwCode = DwCode::every_version("DW0497");

/// A build-integrity failure: a stable DW code plus the message naming the
/// caller, the line and the missing target.
#[derive(Debug, Clone)]
pub struct IntegrityError {
    /// The stable DW code.
    pub code: DwCode,
    /// Human-readable explanation, with the whole fix list.
    pub message: String,
}

/// Which emitted datapack a function belongs to. Determines what it may call:
/// the shipped pack ships alone, the overlays ship beside it.
///
/// Shared with [`crate::seeding`], the other feature-blind proof read off the
/// finished tree: the tier a body ships in is a fact about the build output, not
/// about either check, so both read it from here rather than each keeping a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Tier {
    /// `datapack/` — the delve itself (ADR-0010).
    Shipped,
    /// `packtest-datapack/` — the generated PackTest overlay (validation only).
    PackTest,
    /// `creator-datapack/` — the playtest-only creator overlay (spec-0006).
    Creator,
}

impl Tier {
    /// The tier a build-output path belongs to, or `None` for a non-datapack
    /// artifact (`server/`, `critical-path.json`, the resource pack, …).
    pub(crate) fn of(path: &str) -> Option<Tier> {
        match path.split_once('/')?.0 {
            "datapack" => Some(Tier::Shipped),
            "packtest-datapack" => Some(Tier::PackTest),
            "creator-datapack" => Some(Tier::Creator),
            _ => None,
        }
    }

    /// Whether a caller in `self` may resolve a call against a callee in `other`.
    fn may_call(self, other: Tier) -> bool {
        self == other || other == Tier::Shipped
    }
}

/// One `function <ns>:<name>` reference found in an emitted body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    /// The unqualified name the call targets (`spawn_storm_shore`).
    pub target: String,
    /// 1-based line number within the calling function's body.
    pub line: usize,
    /// The whole command, trimmed — so the message shows what actually ships.
    pub command: String,
}

/// Every internal (`<ns>:`) function call in one function body, in line order.
///
/// Recognises exactly the call forms the emitter produces: a bare `function
/// ns:x`, `execute … run function ns:x`, `schedule function ns:x 30t`, `execute
/// … run schedule function ns:x 30t` and `return run function ns:x`. The
/// `function` keyword is only read as a command when it opens the line or
/// follows `run`/`schedule`, so the word appearing inside a `tellraw` payload is
/// not a call site.
pub fn call_sites(ns: &str, body: &str) -> Vec<CallSite> {
    let mut out = Vec::new();
    for (i, raw) in body.lines().enumerate() {
        let line = raw.trim();
        // `#` opens an mcfunction comment.
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for (t, tok) in tokens.iter().enumerate() {
            if *tok != "function" {
                continue;
            }
            let in_command_position = t == 0 || matches!(tokens[t - 1], "run" | "schedule");
            if !in_command_position {
                continue;
            }
            let Some(arg) = tokens.get(t + 1) else {
                continue;
            };
            // `function #ns:tag` names a function TAG, not a function.
            if arg.starts_with('#') {
                continue;
            }
            let Some((arg_ns, name)) = arg.split_once(':') else {
                continue;
            };
            if arg_ns != ns {
                continue;
            }
            out.push(CallSite {
                target: name.to_string(),
                line: i + 1,
                command: line.to_string(),
            });
        }
    }
    out
}

/// The name a `function <ns>:<name>` call would use to reach this artifact, or
/// `None` if the artifact is not a callable function.
///
/// Callable bodies live under `data/<ns>/function/**`; a PackTest `test/` body is
/// a *caller* the test runner drives, never something another function can call.
fn callable_name(path: &str) -> Option<&str> {
    path.rsplit_once("/function/")?
        .1
        .strip_suffix(".mcfunction")
}

/// Prove the emitted call graph is closed (`DW0497`).
///
/// `functions` maps an emitted `.mcfunction` **artifact path** to its body — the
/// build output's own key, so nothing can collide and be silently dropped (which
/// would be this very defect class, inside its own check). This is the
/// unit-testable core: [`check_tree`] is it, applied to a finished build.
///
/// Iteration is over `BTreeMap`/`BTreeSet` throughout, so the reported fix list
/// is byte-stable across runs (ADR-0006).
pub fn check_functions(
    ns: &str,
    functions: &BTreeMap<String, String>,
) -> Result<(), IntegrityError> {
    // The callable set per tier, keyed by the name a `function ns:<name>` uses.
    let mut emitted: BTreeMap<Tier, BTreeSet<&str>> = BTreeMap::new();
    for path in functions.keys() {
        if let Some(tier) = Tier::of(path)
            && let Some(name) = callable_name(path)
        {
            emitted.entry(tier).or_default().insert(name);
        }
    }
    let resolves = |from: Tier, target: &str| {
        emitted
            .iter()
            .any(|(t, names)| from.may_call(*t) && names.contains(target))
    };

    let mut dangling: Vec<String> = Vec::new();
    for (path, body) in functions {
        let Some(tier) = Tier::of(path) else {
            continue;
        };
        for site in call_sites(ns, body) {
            if resolves(tier, &site.target) {
                continue;
            }
            // The artifact path, not the bare name: it identifies the calling
            // function AND the tier it ships in, which is half the answer to
            // "which emitter forgot to register what".
            dangling.push(format!(
                "`{path}` line {}: `{}` — no function `{ns}:{}` is emitted",
                site.line, site.command, site.target
            ));
        }
    }
    if dangling.is_empty() {
        return Ok(());
    }
    Err(IntegrityError {
        code: DW_DANGLING_FUNCTION_CALL,
        message: format!(
            "the emitted datapack calls {n} function(s) it never emits — each call \
             loads without error and does nothing at all, so the verb behind it \
             simply never happens (the island round-21 defect: two of three storm \
             waves were fired from a `sequence` step, the wave-machinery emitter \
             registered only top-level sites, and `function <ns>:spawn_…` shipped \
             pointing at nothing). This is a compiler defect, not content: an \
             emitter's call walk and its machinery walk have gone out of \
             agreement. Fix the emitter so both derive from one traversal — never \
             silence this by deleting the call site.\n{list}",
            n = dangling.len(),
            list = dangling.join("\n"),
        ),
    })
}

/// [`check_functions`] over a finished build output.
///
/// Reads every emitted `.mcfunction` — including the PackTest overlay's `test/`
/// bodies, which are call sites too (the island's generated census test called
/// the very machinery that was missing) — and resolves against every emitted
/// `<tier>/data/<ns>/function/**` name.
pub fn check_tree(ns: &str, out: &BTreeMap<String, Vec<u8>>) -> Result<(), IntegrityError> {
    let mut functions: BTreeMap<String, String> = BTreeMap::new();
    for (path, bytes) in out {
        if !path.ends_with(".mcfunction") || Tier::of(path).is_none() {
            continue;
        }
        let Ok(body) = std::str::from_utf8(bytes) else {
            continue;
        };
        functions.insert(path.clone(), body.to_string());
    }
    check_functions(ns, &functions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(p, b)| (p.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn a_closed_graph_passes() {
        let m = tree(&[
            (
                "datapack/data/isle/function/tick.mcfunction",
                "execute as @a run function isle:cp_respawn_check\n",
            ),
            (
                "datapack/data/isle/function/cp_respawn_check.mcfunction",
                "say ok\n",
            ),
        ]);
        assert!(check_functions("isle", &m).is_ok());
    }

    #[test]
    fn a_shipped_function_may_not_call_the_packtest_overlay() {
        let m = tree(&[
            (
                "datapack/data/isle/function/tick.mcfunction",
                "function isle:pt_camp_drive\n",
            ),
            (
                "packtest-datapack/data/isle/function/pt_camp_drive.mcfunction",
                "say fixture\n",
            ),
        ]);
        let e = check_functions("isle", &m).expect_err("the shipped pack ships alone");
        assert_eq!(e.code, "DW0497");
    }

    #[test]
    fn a_packtest_function_may_call_the_shipped_pack() {
        let m = tree(&[
            (
                "datapack/data/isle/function/spawn_x.mcfunction",
                "say mobs\n",
            ),
            (
                "packtest-datapack/data/isle/test/wave_census.mcfunction",
                "function isle:spawn_x\n",
            ),
        ]);
        assert!(check_functions("isle", &m).is_ok());
    }

    /// A PackTest `test/` body is a caller, never a callee: nothing may reach it
    /// with `function <ns>:<name>`, because it is not registered as a function.
    #[test]
    fn a_packtest_test_body_is_not_callable() {
        let m = tree(&[
            (
                "packtest-datapack/data/isle/function/pt_drive.mcfunction",
                "function isle:wave_census\n",
            ),
            (
                "packtest-datapack/data/isle/test/wave_census.mcfunction",
                "say fixture\n",
            ),
        ]);
        let e = check_functions("isle", &m).expect_err("a test body is not a function");
        assert_eq!(e.code, "DW0497");
    }

    #[test]
    fn a_comment_naming_a_function_is_not_a_call() {
        let m = tree(&[(
            "datapack/data/isle/function/tick.mcfunction",
            "# function isle:not_a_call\n",
        )]);
        assert!(check_functions("isle", &m).is_ok());
    }
}
