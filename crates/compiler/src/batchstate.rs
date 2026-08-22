//! Batch-state ownership of the generated PackTest suite: a template that DRIVES
//! the outcome it asserts on must OWN every `#party` term the gates on that
//! outcome's path read (`DW0807`).
//!
//! ## The defect class this closes
//!
//! The generated suite runs as ONE batch on ONE shared server. Every `# @dummy`
//! test spawns its own dummy, all the dummies coexist, and PackTest runs them in
//! a RANDOMISED order the compiler does not choose. Progression state is the
//! batch-global `#party` holder (spec-0018), so the contract is *own dummy, own
//! scores, own init*: a template must establish every term it depends on,
//! because a sibling may legitimately hold that term at any value.
//!
//! A template body is one atomic `mcfunction`, which makes that contract cheap
//! to keep — but only for terms the template actually writes. The terms it
//! merely *reads* are decided by whoever ran last, and one template in the suite
//! is not atomic at all: the campaign-playthrough template drives the real
//! campaign, `schedule`s its next phase twenty ticks out, and `await`s. For
//! those twenty ticks the whole party ledger is whatever the campaign's own
//! completion functions left there, and the last phase's leavings persist to the
//! end of the run.
//!
//! Two worked instances, and the second is why the scope below is drawn where it
//! is rather than around `tick`.
//!
//! **A gate in `tick`.** The gallery's `trigger/skip-the-label` forbids
//! `flag/hall-sealed`, so its dispatch line in `tick` carries `unless score
//! #party dw.f_hall_sealed matches 1`. The campaign template's phase-0 run
//! completes `q_far_hall`, whose `complete_q_far_hall` SETS that very flag, and
//! nothing clears it again. `v04_strike_npc` — which struck the NPC, ran the
//! real `tick`, and asserted the trigger fired — therefore passed or failed on
//! nothing but whether its body happened to run before or after that. The same
//! bytes produced both verdicts, three green runs on one branch and a red on
//! another, and the emission was byte-identical across all of them.
//!
//! **A gate the campaign template reaches through its own drive.** The
//! campaign-playthrough template calls `complete_<objective>` directly and never
//! `tick` at all; the gate that decides it is `check_q_<quest>`'s `unless score
//! #party dw.q_<quest> matches 1`, one dispatch further down, and the score it
//! asserts is written by `campaign_complete`, two dispatches down. A sibling
//! that runs the real `tick` can complete the terminal quest outright — measured
//! on `souls-bonfire`, where the batch order that reproduced it ran `verb_kill`
//! (real `tick`; starts the closing cutscene, whose camera every dummy in the
//! batch then spectates, inside the shrine's completion volume) and two `obj/door`
//! completions before `campaign`. `dw.q_trial` was then already 1, every
//! `check_q_*` in the campaign template's drive declined, and its `assert` read 0
//! on tick 0. **The check as first written saw none of this**: the template runs
//! no `tick`, so it was not even counted, and the gate is not a `tick` line, so
//! it was not in the table. The rule was right and its binding was too narrow —
//! 19 of 216 templates judged across the fixture suites, and every `campaign`
//! template among the 197.
//!
//! ## Why this is not a test, and not a rule in prose
//!
//! The suite is what proves the datapack on a server; a defect in the suite is
//! invisible to every other check the repo has, and this one presented as an
//! intermittent, which is the form that gets re-run rather than read. The rule
//! it enforces was already written down — in `packtest_batch.rs`'s module doc,
//! and in `packtest_guards`'s own comment claiming that "no template can be
//! written that opens a gate by hand and forgets one". Three templates had done
//! exactly that, each hand-rolling a different subset of the same gate: one drove
//! none of the three axes, one drove one, one drove two. A doc line is not an
//! invocation, so the rule is decided here, over the bytes that ship.
//!
//! ## Scope, and why it is drawn there
//!
//! * **`#party` only.** That is the documented batch-global progression holder.
//!   Other global sentinels (`#trig_<id> dw.sys`, a wave counter) are reached by
//!   templates through the machinery that owns them — `verb_kill` drives
//!   `#muster dw.wave` by spawning and killing a real wave, not by writing the
//!   score — so demanding a literal write of those would red correct templates.
//! * **Templates that DRIVE their own outcome.** A template drives when it
//!   dispatches a campaign function which, transitively, writes a score the
//!   template asserts or awaits *later in its own body*. `tick` is one such
//!   function and is no longer a special case. The template's own hoisted
//!   helpers (`pt_camp_drive`, `pt_camp_run_<i>`) are inlined first, because a
//!   baseline does not stop belonging to the template when the emitter moves it
//!   into a function for atomicity.
//! * **Only gates on the path from the drive to the outcome.** A gate counts
//!   when the function it dispatches records, transitively, something the
//!   template asserts *after* the drive. Both halves are load-bearing. Without
//!   "transitively" the campaign template's `campaign_complete` is invisible;
//!   without "after the drive" `verb_kill`'s wave-count assertion — taken
//!   immediately after it summons the wave itself, and long before it ticks —
//!   makes every gate that can touch a wave counter that template's business,
//!   which is a mis-attribution rather than a finding.
//! * **A drive nothing gates is driving but not judged.** It cannot be decided by
//!   a sibling, so there is nothing to own; a suite where that is true of every
//!   template reports its zero binding rather than passing silently.
//! * **Ownership, not value.** Writing the term is the whole obligation; which
//!   value opens the gate is the template's business (`verb_forbid_gate`
//!   deliberately sets a forbidden flag to 1).
//!
//! Feature-blind and decidable from the finished tree, in the same family as
//! [`crate::integrity`] and [`crate::affordance`]: it judges the commands that
//! ship, not the intent of the emitter that wrote them, so it guards templates
//! that do not exist yet.
//!
//! Determinism (ADR-0006): every set is a `BTreeSet` and every map a `BTreeMap`,
//! so the message text is a function of the tree alone.

use delvewright_dsl::DwCode;
use std::collections::{BTreeMap, BTreeSet};

/// `DW0807`: a generated PackTest template runs the campaign's real `tick` and
/// asserts on an outcome whose gate reads `#party` state the template never
/// writes.
///
/// Build-tier (exit 3). The suite still loads and the test still passes most of
/// the time — that is the failure mode, not a mitigation. A template whose
/// verdict depends on batch order is not a proof, and re-running it discards the
/// finding.
pub const DW_PACKTEST_UNOWNED_GATE: DwCode = DwCode::every_version("DW0807");

/// The batch-global progression holder every gate term is read from
/// (spec-0018). Mirrors `plan::PARTY`; kept as its own constant so this module
/// stays a pure function of the emitted tree.
const PARTY: &str = "#party";

/// A batch-state failure: a stable DW code plus the message naming every
/// template, the terms it does not own, and where they come from.
#[derive(Debug, Clone)]
pub struct BatchStateError {
    /// The stable diagnostic code.
    pub code: DwCode,
    /// Human-readable explanation, with the whole fix list.
    pub message: String,
}

/// What the check actually examined. A proof over "every template" that bound to
/// nothing is vacuous, not a pass (CLAUDE.md), so the numbers are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchStateBinding {
    /// Generated PackTest templates found in the tree.
    pub templates: usize,
    /// Of those, the ones that DRIVE the outcome they assert on — they dispatch
    /// a campaign function which, transitively, writes a score the template
    /// asserts or awaits. `tick` is one such function and no longer a special
    /// case: the campaign-playthrough template drives `complete_<objective>`
    /// directly and was invisible here while this counted `tick` alone.
    pub driving: usize,
    /// Of those, the ones whose driven outcome passes through at least one
    /// `#party`-gated dispatch — the templates this check can actually judge.
    pub judged: usize,
}

impl BatchStateBinding {
    /// The zero-binding finding: templates tick, yet none of their assertions
    /// resolved to a gated dispatch, so nothing was compared. Reported as a
    /// `DW0807` warning rather than passed over in silence.
    pub fn finding(&self) -> Option<delvewright_dsl::Diagnostic> {
        (self.driving > 0 && self.judged == 0).then(|| {
            delvewright_dsl::Diagnostic::warning(
                DW_PACKTEST_UNOWNED_GATE,
                "build",
                "packtest batch-state binding",
                format!(
                    "the batch-state ownership invariant judged 0 of {} generated PackTest \
                     template(s) that DRIVE the outcome they assert on: none of those outcomes \
                     passed through a `#party`-gated dispatch, so no template was compared \
                     against any gate. A template whose verdict depends on which sibling ran \
                     last would not have been seen",
                    self.driving
                ),
            )
        })
    }
}

/// Split an `execute` line into its condition and its payload. Everything before
/// the first ` run ` decides whether the payload happens; the payload itself is
/// not a gate. A line with no ` run ` is unconditional and has no condition.
fn condition_of(line: &str) -> &str {
    match line.find(" run ") {
        Some(i) => &line[..i],
        None => "",
    }
}

/// Every `<holder> <objective>` pair a fragment reads or writes through the
/// scoreboard, as `(holder, objective)`. Recognises the `#holder dw.objective`
/// adjacency that every emitted score reference uses, which keeps this a pure
/// text function with no command-grammar knowledge.
fn score_pairs(fragment: &str) -> BTreeSet<(String, String)> {
    let toks: Vec<&str> = fragment.split_whitespace().collect();
    toks.windows(2)
        .filter(|w| w[0].starts_with('#') && w[1].starts_with("dw."))
        .map(|w| (w[0].trim_end_matches(',').to_string(), w[1].to_string()))
        .collect()
}

/// The `#party` objectives read in a fragment.
fn party_reads(fragment: &str) -> BTreeSet<String> {
    score_pairs(fragment)
        .into_iter()
        .filter(|(h, _)| h == PARTY)
        .map(|(_, o)| o)
        .collect()
}

/// The function a line dispatches, if it names one in this campaign's namespace.
/// `function #<ns>:<tag>` names a tag, not a function, and is skipped.
fn dispatch_target(ns: &str, line: &str) -> Option<String> {
    let needle = format!("function {ns}:");
    let i = line.rfind(&needle)?;
    if line[..i].ends_with('#') {
        return None;
    }
    let rest = &line[i + needle.len()..];
    let name = rest.split_whitespace().next()?;
    (!name.is_empty()).then(|| name.to_string())
}

/// Every score a function body WRITES, at one level, as `(holder, objective)`.
/// Composed into a transitive answer by [`Tree::deep_writes`].
fn writes_of(body: &str) -> BTreeSet<(String, String)> {
    let mut w = BTreeSet::new();
    for line in body.lines() {
        let Some(i) = line.find("scoreboard players ") else {
            continue;
        };
        let rest = &line[i + "scoreboard players ".len()..];
        let mut toks = rest.split_whitespace();
        let Some(verb) = toks.next() else { continue };
        if !matches!(verb, "set" | "add" | "remove" | "reset" | "operation") {
            continue;
        }
        let (Some(h), Some(o)) = (toks.next(), toks.next()) else {
            continue;
        };
        if h.starts_with('#') && o.starts_with("dw.") {
            w.insert((h.to_string(), o.to_string()));
        }
    }
    w
}

/// The emitted function graph, resolved once per tree.
///
/// The two halves are kept apart because they answer different questions. The
/// **campaign** half is the datapack under test: its gates are the thing a
/// template can be decided by. The **helper** half is a template's own body that
/// the emitter hoisted into a function for atomicity (`pt_camp_drive`,
/// `pt_camp_run_<i>`) — a baseline does not stop belonging to the template
/// because it was moved, so those are inlined back before anything is judged.
struct Tree<'a> {
    ns: &'a str,
    campaign: BTreeMap<String, String>,
    helper: BTreeMap<String, String>,
}

/// One gated dispatch line, reduced to the two things this check needs.
#[derive(Debug, Clone)]
struct Gate {
    /// The campaign function this line runs.
    target: String,
    /// Every `#party` objective read in the line's CONDITION.
    reads: BTreeSet<String>,
}

impl Tree<'_> {
    /// Every campaign function reachable from `roots`, `roots` included.
    /// Cycle-safe: the campaign's own tick bodies re-`schedule` themselves.
    fn reachable(&self, roots: &[String]) -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = roots.to_vec();
        while let Some(f) = stack.pop() {
            if !seen.insert(f.clone()) {
                continue;
            }
            if let Some(body) = self.campaign.get(&f) {
                for line in body.lines() {
                    if let Some(t) = dispatch_target(self.ns, line) {
                        stack.push(t);
                    }
                }
            }
        }
        seen
    }

    /// Every score written anywhere below `root`, transitively.
    ///
    /// One level is not enough and the reason is the whole finding: the campaign
    /// template's outcome (`dw.campaign`) is written by `campaign_complete`,
    /// which is two dispatches below the `complete_<objective>` the template
    /// actually calls. A one-level reading sees nothing there and judges the
    /// template not to be driving its own assertion.
    fn deep_writes(&self, root: &str) -> BTreeSet<(String, String)> {
        self.reachable(std::slice::from_ref(&root.to_string()))
            .iter()
            .filter_map(|f| self.campaign.get(f))
            .flat_map(|b| writes_of(b))
            .collect()
    }

    /// Every `#party`-gated dispatch inside `fns`.
    fn gates_in(&self, fns: &BTreeSet<String>) -> Vec<Gate> {
        let mut gates = Vec::new();
        for f in fns {
            let Some(body) = self.campaign.get(f) else {
                continue;
            };
            for line in body.lines() {
                let Some(target) = dispatch_target(self.ns, line) else {
                    continue;
                };
                let reads = party_reads(condition_of(line));
                if reads.is_empty() {
                    continue;
                }
                gates.push(Gate { target, reads });
            }
        }
        gates
    }

    /// A template body with its own hoisted helper functions inlined in place.
    fn flatten(&self, body: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        self.flatten_into(body, &mut out, &mut seen);
        out
    }

    fn flatten_into(&self, body: &str, out: &mut Vec<String>, seen: &mut BTreeSet<String>) {
        for line in body.lines() {
            match dispatch_target(self.ns, line) {
                Some(t) if self.helper.contains_key(&t) && seen.insert(t.clone()) => {
                    let sub = self.helper[&t].clone();
                    self.flatten_into(&sub, out, seen);
                }
                _ => out.push(line.to_string()),
            }
        }
    }
}

/// Every score a template's verdict rests on from `after` onward: the ones it
/// `assert`s or `await`s there, closed under "and whatever decided them inside
/// this template".
///
/// The window matters. `verb_kill` asserts a wave counter immediately after
/// summoning the wave itself and the objective score after the tick; reading
/// both as outcomes of the summon makes every gate that can touch a wave
/// counter that template's business, which is not a defect but a
/// mis-attribution.
///
/// The closure is what reaches the branch-aware campaign template, whose only
/// assertion is a phase COUNTER (`#camp_phase dw.sys`) — incremented by a line
/// reading `#party <completion objective>`. Without the closure that template
/// asserts nothing the campaign writes and is silently unjudged, which is the
/// same blind spot, one indirection along.
fn asserted_scores(after: &[String], flat: &[String]) -> BTreeSet<(String, String)> {
    let mut asserted: BTreeSet<(String, String)> = after
        .iter()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("assert score ") || t.starts_with("await score ")
        })
        .flat_map(|l| score_pairs(l))
        .collect();
    loop {
        let mut grew = false;
        for line in flat {
            let cond = condition_of(line);
            if cond.is_empty() {
                continue;
            }
            let payload = &line[cond.len() + " run ".len()..];
            if writes_of(payload).is_disjoint(&asserted) {
                continue;
            }
            for p in score_pairs(cond) {
                grew |= asserted.insert(p);
            }
        }
        if !grew {
            break;
        }
    }
    asserted
}

/// Judge a whole emitted tree. `ns` is the campaign namespace.
pub fn check_tree(
    ns: &str,
    out: &BTreeMap<String, Vec<u8>>,
) -> Result<BatchStateBinding, BatchStateError> {
    let collect = |prefix: &str| -> BTreeMap<String, String> {
        out.iter()
            .filter_map(|(p, b)| {
                let name = p.strip_prefix(prefix)?.strip_suffix(".mcfunction")?;
                Some((name.to_string(), std::str::from_utf8(b).ok()?.to_string()))
            })
            .collect()
    };
    let tree = Tree {
        ns,
        campaign: collect(&format!("datapack/data/{ns}/function/")),
        helper: collect(&format!("packtest-datapack/data/{ns}/function/")),
    };
    if tree.campaign.is_empty() {
        // No campaign functions: nothing dispatches, so nothing can be gated.
        return Ok(BatchStateBinding {
            templates: 0,
            driving: 0,
            judged: 0,
        });
    }

    // What each campaign function records, transitively, read off the shipped
    // tree. Computed once for the whole tree rather than per template.
    let deep: BTreeMap<String, BTreeSet<(String, String)>> = tree
        .campaign
        .keys()
        .map(|f| (f.clone(), tree.deep_writes(f)))
        .collect();
    // Every `#party`-gated dispatch below each campaign function, likewise.
    let gates_below: BTreeMap<String, Vec<Gate>> = tree
        .campaign
        .keys()
        .map(|f| {
            (
                f.clone(),
                tree.gates_in(&tree.reachable(std::slice::from_ref(f))),
            )
        })
        .collect();

    let prefix = format!("packtest-datapack/data/{ns}/test/");
    let mut binding = BatchStateBinding {
        templates: 0,
        driving: 0,
        judged: 0,
    };
    let mut offenders: Vec<String> = Vec::new();

    for (path, bytes) in out {
        if !path.starts_with(&prefix) || !path.ends_with(".mcfunction") {
            continue;
        }
        let Ok(body) = std::str::from_utf8(bytes) else {
            continue;
        };
        binding.templates += 1;
        let flat = tree.flatten(body);

        // The DRIVE: a line that dispatches a campaign function which,
        // transitively, writes something the template asserts LATER. Everything
        // before it is the template's own init; everything from it on is the
        // campaign deciding the verdict.
        //
        // The judged drive is the first such line whose campaign function also
        // reaches a `#party`-gated dispatch that records one of those outcomes:
        // a drive nothing gates cannot be decided by a sibling, and charging a
        // template with gates that merely touch a score it asserts — rather
        // than gates on the path that produces it — demands writes to the
        // shared ledger that would themselves be the defect.
        let drives = |i: usize| -> Option<BTreeSet<(String, String)>> {
            let t = dispatch_target(ns, &flat[i])?;
            let recorded = deep.get(&t)?;
            let a = asserted_scores(&flat[i + 1..], &flat);
            (!recorded.is_disjoint(&a)).then_some(a)
        };
        if !(0..flat.len()).any(|i| drives(i).is_some()) {
            continue;
        }
        binding.driving += 1;
        let gated = |i: usize| -> Option<(usize, BTreeSet<(String, String)>)> {
            let a = drives(i)?;
            let t = dispatch_target(ns, &flat[i])?;
            gates_below
                .get(&t)?
                .iter()
                .any(|g| deep.get(&g.target).is_some_and(|w| !w.is_disjoint(&a)))
                .then_some((i, a))
        };
        let Some((drive_at, asserted)) = (0..flat.len()).find_map(gated) else {
            continue;
        };

        // Owned: every `#party` objective the template writes before it drives.
        let owned: BTreeSet<String> = flat[..drive_at]
            .iter()
            .flat_map(|l| {
                writes_of(l)
                    .into_iter()
                    .filter(|(h, _)| h == PARTY)
                    .map(|(_, o)| o)
            })
            .collect();

        binding.judged += 1;

        // Required: the gate of every `#party`-gated dispatch on the path from
        // the drive to the outcome, whose target records something asserted.
        let mut required: BTreeSet<String> = BTreeSet::new();
        for root in flat[drive_at..]
            .iter()
            .filter_map(|l| dispatch_target(ns, l))
        {
            let Some(gates) = gates_below.get(&root) else {
                continue;
            };
            for g in gates {
                if deep
                    .get(&g.target)
                    .is_some_and(|w| !w.is_disjoint(&asserted))
                {
                    required.extend(g.reads.iter().cloned());
                }
            }
        }

        let missing: Vec<String> = required.difference(&owned).cloned().collect();
        if missing.is_empty() {
            continue;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        offenders.push(format!(
            "  `{name}` does not write {} before it drives the outcome it asserts on",
            missing
                .iter()
                .map(|m| format!("`{PARTY} {m}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if offenders.is_empty() {
        return Ok(binding);
    }

    Err(BatchStateError {
        code: DW_PACKTEST_UNOWNED_GATE,
        message: format!(
            "generated PackTest template(s) assert an outcome whose gate reads `{PARTY}` state \
             they never write:\n{}\n\nThe suite runs as ONE batch on ONE shared server, in an \
             order PackTest RANDOMISES, and `{PARTY}` is batch-global progression state \
             (spec-0018). A term a template only READS is whatever the sibling that ran last left \
             there. A template in that position does not fail; it becomes a coin toss that lands \
             green most of the time, which is the shape that gets re-run instead of read.\n\nFix \
             it in the emitter that writes the template, and there are two authorities depending \
             on what the template drives. A template that opens ONE gate drives it through \
             `packtest_gate_drive`, which takes a `Gate` and therefore covers all three axes at \
             once — `requires_flags`, `forbids_flags` and `requires_state`. The \
             campaign-playthrough template drives the whole campaign, so it opens with \
             `campaign_progression_baseline` — the entire party ledger set to the campaign's start \
             state, every term of which its own drive writes again inside the same atomic \
             mcfunction. Do NOT hand-roll the terms beside the template: three sites did exactly \
             that and each dropped a different axis, which is how this arrived. And never silence \
             this by relaxing the assertion or retrying the test: an intermittent is a finding, \
             and re-running discards it",
            offenders.join("\n")
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

    /// The gallery's real shape, reduced: a trigger dispatch gated on a forbidden
    /// flag, and a template that ticks and asserts the trigger fired.
    fn gallery_like(owns_gate: bool) -> BTreeMap<String, Vec<u8>> {
        let own = if owns_gate {
            "scoreboard players set #party dw.f_hall_sealed 0\n"
        } else {
            ""
        };
        tree(&[
            (
                "datapack/data/g/function/tick.mcfunction",
                "execute unless score #trig_skip dw.sys matches 1 unless score #party \
                 dw.f_hall_sealed matches 1 if entity @e[tag=dw_trig_skip,nbt={attack:{}}] run \
                 function g:trig_skip\n",
            ),
            (
                "datapack/data/g/function/trig_skip.mcfunction",
                "scoreboard players set #trig_skip dw.sys 1\n",
            ),
            (
                "packtest-datapack/data/g/test/v04_strike_npc.mcfunction",
                &format!(
                    "#> t\n# @dummy\nscoreboard players set #trig_skip dw.sys 0\n{own}function \
                     g:tick\nassert score #trig_skip dw.sys matches 1\n"
                ),
            ),
        ])
    }

    #[test]
    fn a_template_that_owns_its_dispatch_gate_passes() {
        let b = check_tree("g", &gallery_like(true)).expect("owned gate is not a finding");
        assert_eq!((b.templates, b.driving, b.judged), (1, 1, 1));
    }

    #[test]
    fn a_template_that_reads_a_gate_term_it_never_writes_is_dw0807() {
        let e = check_tree("g", &gallery_like(false)).unwrap_err();
        assert_eq!(e.code, "DW0807");
        assert!(e.message.contains("v04_strike_npc"), "{}", e.message);
        assert!(e.message.contains("dw.f_hall_sealed"), "{}", e.message);
        // The prescription must name the one authority, and must forbid the two
        // repairs that look right and are not.
        assert!(e.message.contains("packtest_gate_drive"), "{}", e.message);
        assert!(e.message.contains("Do NOT hand-roll"), "{}", e.message);
    }

    #[test]
    fn a_template_that_drives_nothing_is_not_judged() {
        let mut t = gallery_like(false);
        t.insert(
            "packtest-datapack/data/g/test/v04_strike_npc.mcfunction".to_string(),
            b"#> t\n# @dummy\nassert score #trig_skip dw.sys matches 0\n".to_vec(),
        );
        let b = check_tree("g", &t).expect("a template that cannot be gated is not judged");
        assert_eq!((b.templates, b.driving, b.judged), (1, 0, 0));
    }

    /// The gate term is read on a dispatch whose function records nothing this
    /// template asserts, so it does not decide the verdict and is not demanded —
    /// while the gate that DOES decide it, beside it in the same `tick`, is.
    #[test]
    fn an_unrelated_gated_dispatch_is_not_required() {
        let t = tree(&[
            (
                "datapack/data/g/function/tick.mcfunction",
                "execute unless score #party dw.f_other matches 1 run function g:unrelated\n\
                 execute if score #party dw.f_mine matches 1 run function g:plain\n",
            ),
            (
                "datapack/data/g/function/unrelated.mcfunction",
                "scoreboard players set #party dw.o_other 1\n",
            ),
            (
                "datapack/data/g/function/plain.mcfunction",
                "scoreboard players set #party dw.o_mine 1\n",
            ),
            (
                "packtest-datapack/data/g/test/t.mcfunction",
                "#> t\n# @dummy\nscoreboard players set #party dw.o_mine 0\n\
                 scoreboard players set #party dw.f_mine 1\nfunction g:tick\n\
                 assert score #party dw.o_mine matches 1\n",
            ),
        ]);
        let b = check_tree("g", &t).expect("an unrelated gate is not this template's business");
        assert_eq!((b.driving, b.judged), (1, 1));
    }

    /// A campaign-playthrough-shaped tree: the template calls
    /// `complete_<objective>` DIRECTLY (it never runs `tick`), the gate that
    /// decides it lives one dispatch further down in `check_q_<quest>`, and the
    /// score it asserts is written two dispatches down in `campaign_complete`.
    /// None of that is visible to a check that reads `tick`'s own lines and one
    /// level of writes — which is how this shape shipped an intermittent.
    fn campaign_like(owns_quest_score: bool) -> BTreeMap<String, Vec<u8>> {
        let own = if owns_quest_score {
            "scoreboard players set #party dw.q_only 0\n\
             scoreboard players set #party dw.o_only 0\n"
        } else {
            ""
        };
        tree(&[
            (
                "datapack/data/g/function/tick.mcfunction",
                "say the campaign template never calls this\n",
            ),
            (
                "datapack/data/g/function/complete_o_only.mcfunction",
                "scoreboard players set #party dw.o_only 1\nfunction g:check_q_only\n",
            ),
            (
                "datapack/data/g/function/check_q_only.mcfunction",
                "execute if score #party dw.o_only matches 1 unless score #party dw.q_only \
                 matches 1 run function g:complete_q_only\n",
            ),
            (
                "datapack/data/g/function/complete_q_only.mcfunction",
                "scoreboard players set #party dw.q_only 1\nfunction g:campaign_complete\n",
            ),
            (
                "datapack/data/g/function/campaign_complete.mcfunction",
                "scoreboard players set #party dw.campaign 1\n",
            ),
            (
                "packtest-datapack/data/g/test/campaign.mcfunction",
                &format!(
                    "#> t\n# @dummy\nscoreboard players set #party dw.campaign 0\n{own}\
                     function g:pt_camp_drive\nawait score #party dw.campaign matches 1\n"
                ),
            ),
            (
                "packtest-datapack/data/g/function/pt_camp_drive.mcfunction",
                "function g:complete_o_only\n",
            ),
        ])
    }

    #[test]
    fn a_campaign_template_that_does_not_reset_the_quest_it_re_drives_is_dw0807() {
        let e = check_tree("g", &campaign_like(false)).unwrap_err();
        assert_eq!(e.code, "DW0807");
        assert!(e.message.contains("campaign.mcfunction"), "{}", e.message);
        assert!(e.message.contains("dw.q_only"), "{}", e.message);
        // The prescription for THIS shape is the whole-ledger baseline, named.
        assert!(
            e.message.contains("campaign_progression_baseline"),
            "{}",
            e.message
        );
    }

    /// The same tree with the baseline in place. The template's drive lives in
    /// the hoisted `pt_camp_drive`, so this also pins that a template's own
    /// helper function still counts as the template.
    #[test]
    fn a_campaign_template_that_re_baselines_the_ledger_passes() {
        let b = check_tree("g", &campaign_like(true)).expect("a full baseline is not a finding");
        assert_eq!((b.templates, b.driving, b.judged), (1, 1, 1));
    }

    #[test]
    fn a_suite_whose_templates_drive_nothing_reports_its_zero_binding() {
        let mut t = gallery_like(false);
        t.insert(
            "packtest-datapack/data/g/test/v04_strike_npc.mcfunction".to_string(),
            b"#> t\n# @dummy\nsay nothing\n".to_vec(),
        );
        let b = check_tree("g", &t).unwrap();
        // Nothing drives, so there is nothing to be silent about.
        assert_eq!((b.driving, b.judged), (0, 0));
        assert!(b.finding().is_none());
        // But a suite that drives and resolves no gate is a reported finding.
        let vacuous = BatchStateBinding {
            templates: 3,
            driving: 3,
            judged: 0,
        };
        assert_eq!(vacuous.finding().unwrap().code, "DW0807");
    }
}
