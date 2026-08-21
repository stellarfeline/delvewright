//! Batch-state ownership of the generated PackTest suite: a template that runs
//! the campaign's own `tick` and asserts on a gated outcome must OWN every
//! `#party` term that outcome's gate reads (`DW0807`).
//!
//! ## The defect class this closes
//!
//! The generated suite runs as ONE batch on ONE shared server. Every `# @dummy`
//! test spawns its own dummy, all the dummies coexist, and the test functions
//! execute over the same server ticks in an order the compiler does not choose.
//! Progression state is the batch-global `#party` holder (spec-0018), so the
//! contract is *own dummy, own scores, own init*: a template must establish
//! every term it depends on, because a sibling may legitimately hold that term
//! at any value.
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
//! The worked instance. The gallery's `trigger/skip-the-label` forbids
//! `flag/hall-sealed`, so its dispatch line in `tick` carries `unless score
//! #party dw.f_hall_sealed matches 1`. The campaign template's phase-0 run
//! completes `q_far_hall`, whose `complete_q_far_hall` SETS that very flag, and
//! nothing clears it again. `v04_strike_npc` — which struck the NPC, ran the
//! real `tick`, and asserted the trigger fired — therefore passed or failed on
//! nothing but whether its body happened to run before or after that. The same
//! bytes produced both verdicts, three green runs on one branch and a red on
//! another, and the emission was byte-identical across all of them.
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
//! * **Templates that invoke the campaign's real `tick`.** A template that never
//!   ticks cannot be decided by a gate.
//! * **Only the gates that actually reach an assertion.** A tick line matters to
//!   a template when the function it dispatches writes something that template
//!   asserts. Requiring every `#party` term in the whole of `tick` would demand a
//!   full progression baseline from every template, which would in turn break
//!   the campaign template's `await` — the cure being worse than the disease.
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
    /// Of those, the ones that invoke the campaign's real `tick`.
    pub ticking: usize,
    /// Of those, the ones whose assertions are reached through at least one
    /// gated `tick` line — the templates this check can actually judge.
    pub judged: usize,
}

impl BatchStateBinding {
    /// The zero-binding finding: templates tick, yet none of their assertions
    /// resolved to a gated dispatch, so nothing was compared. Reported as a
    /// `DW0807` warning rather than passed over in silence.
    pub fn finding(&self) -> Option<delvewright_dsl::Diagnostic> {
        (self.ticking > 0 && self.judged == 0).then(|| {
            delvewright_dsl::Diagnostic::warning(
                DW_PACKTEST_UNOWNED_GATE,
                "build",
                "packtest batch-state binding",
                format!(
                    "the batch-state ownership invariant judged 0 of {} generated PackTest \
                     template(s) that run the campaign's own `tick`: none of their assertions \
                     resolved to a gated dispatch line, so no template was compared against any \
                     gate. A template whose verdict depends on which sibling ran last would not \
                     have been seen",
                    self.ticking
                ),
            )
        })
    }
}

/// One `tick` dispatch line, reduced to the two things this check needs.
#[derive(Debug, Clone)]
struct Dispatch {
    /// The campaign function this line runs.
    target: String,
    /// Every `#party` objective read in the line's CONDITION.
    reads: BTreeSet<String>,
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

/// Every score a function body WRITES, at one level. One level is enough for the
/// shape this check judges: a `tick` line dispatches the function that records
/// the outcome, and that record is the write the template asserts on.
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

/// Judge a whole emitted tree. `ns` is the campaign namespace.
pub fn check_tree(
    ns: &str,
    out: &BTreeMap<String, Vec<u8>>,
) -> Result<BatchStateBinding, BatchStateError> {
    let text = |path: &str| -> Option<String> {
        out.get(path)
            .and_then(|b| std::str::from_utf8(b).ok())
            .map(str::to_string)
    };

    let tick_path = format!("datapack/data/{ns}/function/tick.mcfunction");
    let Some(tick) = text(&tick_path) else {
        // No campaign tick: nothing dispatches, so nothing can be gated.
        return Ok(BatchStateBinding {
            templates: 0,
            ticking: 0,
            judged: 0,
        });
    };

    // The dispatch table: every gated `tick` line, by the function it runs.
    let dispatches: Vec<Dispatch> = tick
        .lines()
        .filter_map(|l| {
            dispatch_target(ns, l).map(|target| Dispatch {
                target,
                reads: party_reads(condition_of(l)),
            })
        })
        .collect();

    // What each dispatched function records, read off the shipped tree.
    let mut records: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
    for d in &dispatches {
        records.entry(d.target.clone()).or_insert_with(|| {
            text(&format!(
                "datapack/data/{ns}/function/{}.mcfunction",
                d.target
            ))
            .map_or_else(BTreeSet::new, |b| writes_of(&b))
        });
    }

    let prefix = format!("packtest-datapack/data/{ns}/test/");
    let tick_call = format!("function {ns}:tick");
    let mut binding = BatchStateBinding {
        templates: 0,
        ticking: 0,
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
        let lines: Vec<&str> = body.lines().collect();
        let Some(first_tick) = lines.iter().position(|l| l.trim() == tick_call) else {
            continue;
        };
        binding.ticking += 1;

        // Owned: every `#party` objective the template writes before it ticks.
        let owned: BTreeSet<String> = lines[..first_tick]
            .iter()
            .flat_map(|l| {
                writes_of(l)
                    .into_iter()
                    .filter(|(h, _)| h == PARTY)
                    .map(|(_, o)| o)
            })
            .collect();

        // Asserted: every score the template checks once the tick has run.
        let asserted: BTreeSet<(String, String)> = lines[first_tick..]
            .iter()
            .filter(|l| l.trim_start().starts_with("assert score "))
            .flat_map(|l| score_pairs(l))
            .collect();

        // Required: the gate of every dispatch that records something asserted.
        let mut required: BTreeSet<String> = BTreeSet::new();
        let mut reached = false;
        for d in &dispatches {
            let recorded = &records[&d.target];
            if recorded.is_disjoint(&asserted) {
                continue;
            }
            reached = true;
            required.extend(d.reads.iter().cloned());
        }
        if !reached {
            continue;
        }
        binding.judged += 1;

        let missing: Vec<String> = required.difference(&owned).cloned().collect();
        if missing.is_empty() {
            continue;
        }
        let name = path.rsplit('/').next().unwrap_or(path);
        offenders.push(format!(
            "  `{name}` does not write {} before it runs `{tick_call}`",
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
             they never write:\n{}\n\nThe suite runs as ONE batch on ONE shared server, and \
             `{PARTY}` is batch-global progression state (spec-0018). A term a template only \
             READS is whatever the sibling that ran last left there — and the campaign-playthrough \
             template is not atomic: it drives the real campaign, schedules its next phase twenty \
             ticks out and awaits, so for those ticks the party ledger holds whatever the \
             campaign's own completion functions wrote, and the final phase's leavings persist to \
             the end of the run. A template in that position does not fail; it becomes a coin toss \
             that lands green most of the time, which is the shape that gets re-run instead of \
             read.\n\nFix it in the emitter that writes the template: drive the whole gate through \
             `packtest_gate_drive`, which takes a `Gate` and therefore covers all three axes at \
             once — `requires_flags`, `forbids_flags` and `requires_state`. Do NOT hand-roll the \
             terms beside the template: three sites did exactly that and each dropped a different \
             axis, which is how this arrived. Do NOT instead widen the template to a full \
             progression baseline — that would reset the scores the campaign template is awaiting \
             and trade one order-dependent verdict for another. And never silence this by relaxing \
             the assertion or retrying the test: an intermittent is a finding, and re-running \
             discards it",
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
        assert_eq!((b.templates, b.ticking, b.judged), (1, 1, 1));
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
    fn a_template_that_never_ticks_is_not_judged() {
        let mut t = gallery_like(false);
        t.insert(
            "packtest-datapack/data/g/test/v04_strike_npc.mcfunction".to_string(),
            b"#> t\n# @dummy\nassert score #trig_skip dw.sys matches 0\n".to_vec(),
        );
        let b = check_tree("g", &t).expect("a template that cannot be gated is not judged");
        assert_eq!((b.templates, b.ticking, b.judged), (1, 0, 0));
    }

    /// The gate term is read on a dispatch whose function records nothing this
    /// template asserts, so it does not decide the verdict and is not demanded.
    #[test]
    fn an_unrelated_gated_dispatch_is_not_required() {
        let t = tree(&[
            (
                "datapack/data/g/function/tick.mcfunction",
                "execute unless score #party dw.f_other matches 1 run function g:unrelated\n\
                 function g:plain\n",
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
                "#> t\n# @dummy\nscoreboard players set #party dw.o_mine 0\nfunction g:tick\n\
                 assert score #party dw.o_mine matches 1\n",
            ),
        ]);
        let b = check_tree("g", &t).expect("an unrelated gate is not this template's business");
        assert_eq!(b.judged, 1);
    }

    #[test]
    fn a_suite_whose_templates_never_tick_reports_its_zero_binding() {
        let mut t = gallery_like(false);
        t.insert(
            "packtest-datapack/data/g/test/v04_strike_npc.mcfunction".to_string(),
            b"#> t\n# @dummy\nsay nothing\n".to_vec(),
        );
        let b = check_tree("g", &t).unwrap();
        // Nothing ticks, so there is nothing to be silent about.
        assert_eq!((b.ticking, b.judged), (0, 0));
        assert!(b.finding().is_none());
        // But a suite that ticks and resolves nothing is a reported finding.
        let vacuous = BatchStateBinding {
            templates: 3,
            ticking: 3,
            judged: 0,
        };
        assert_eq!(vacuous.finding().unwrap().code, "DW0807");
    }
}
